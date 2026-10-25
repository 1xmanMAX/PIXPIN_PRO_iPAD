//! **Qwen3-ASR sobre ONNX Runtime**: cargar los tres modelos y transcribir.
//!
//! El decodificador es el de `offline-recognizer-qwen3-asr-impl.cc` de
//! sherpa-onnx (Apache-2.0), escrito en Rust sobre [`crate::ort`], con dos
//! cambios medidos en `docs/voz-qwen3.md`:
//!
//! - **La entrada del decoder va en trozos de 64 tokens** (con la cache ya
//!   rellena de los anteriores) y no de una vez. El decoder int8 cuantiza
//!   cada activacion con una sola escala por llamada (`DynamicQuantizeLinear`
//!   por tensor): con los ~280 tokens de una tanda juntos, la escala la
//!   marcan los vectores de audio y el texto pierde precision. En FLEURS
//!   es_419 limpio: WER 7,6 % de una vez frente a 6,5 % en trozos de 64 (la
//!   referencia en PyTorch, 5,8 %). De paso, los logits de la entrada pasan
//!   de ~170 MB a ~40 MB.
//! - La cache de claves y valores mide lo justo para la tanda (guion +
//!   tope de tokens) en vez de un maximo fijo.
//!
//! Compila tambien en Linux (como `ort`), para la prueba con el modelo de
//! verdad; lo que lee ficheros de audio o baja de la red es solo Windows.
//!
//! Este modulo no tiene `unsafe`: todo pasa por [`crate::ort`].

use std::ffi::CStr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use super::fichas::{Fichas, Fusiones};
use super::guion::{self, Guion};
use super::{Reparto, carpeta, catalogo, mel, modelo_listo};
use crate::credibilidad::creible;
use crate::ort::{Runtime, Sesion, Valor};
use crate::tiempos::{Segmento, Transcripcion, con_tiempos, estado_de, lineas};
use crate::{ErrorVoz, HERCIOS};

/// Cabezas de clave/valor y su tamano en el decoder de 0,6B (`[.., 8, 128]`).
const CABEZAS_KV: usize = 8;
const DIM_CABEZA: usize = 128;
/// Lo que ocupa una posicion de la cache, por capa (claves o valores).
const POR_POSICION: usize = CABEZAS_KV * DIM_CABEZA;
/// Lo ancho de cada vector de audio que sale del encoder.
const ANCHO_AUDIO: usize = 1024;
/// Tokens por llamada al decoder al leer el guion (ver arriba).
const TROZO_DE_ENTRADA: usize = 64;

/// Los nombres de entrada que se esperan del decoder, en orden.
const ENTRADAS: [&str; 4] = [
    "input_ids",
    "audio_features",
    "attention_mask",
    "cache_position",
];

/// **El modelo cargado**: tres sesiones, el tokenizador y el guion.
pub struct Qwen3 {
    rt: &'static Runtime,
    conv: Sesion<'static>,
    encoder: Sesion<'static>,
    decoder: Sesion<'static>,
    fichas: Fichas,
    guion: Guion,
    capas: usize,
}

/// Lo que sale de una tanda.
#[derive(Debug, Clone, PartialEq)]
pub struct Oido {
    pub texto: String,
    /// El idioma que dijo el modelo, si se le dejo adivinar.
    pub lengua: Option<&'static str>,
}

fn ilegible(c: &Path) -> ErrorVoz {
    ErrorVoz::ModeloIlegible {
        donde: c.display().to_string(),
    }
}

impl Qwen3 {
    /// Carga el runtime y los seis ficheros de `<datos>/qwen3-asr/`.
    pub fn cargar(
        raiz: &Path,
        junto_al_exe: Option<&Path>,
        reparto: Reparto,
    ) -> Result<Qwen3, ErrorVoz> {
        let rt = Runtime::del_proceso(raiz, junto_al_exe)?;
        let c = carpeta(raiz);
        if !modelo_listo(raiz) {
            return Err(ErrorVoz::SinModeloQwen3 {
                donde: c.display().to_string(),
                megas: catalogo::megas(),
            });
        }
        Qwen3::cargar_de(rt, &c, reparto)
    }

    /// Lo mismo desde una carpeta cualquiera con los seis ficheros (para
    /// las pruebas).
    pub fn cargar_de(rt: &'static Runtime, c: &Path, reparto: Reparto) -> Result<Qwen3, ErrorVoz> {
        let leer = |i: usize| {
            std::fs::read_to_string(c.join(catalogo::FICHEROS[i].nombre)).map_err(|_| ilegible(c))
        };
        let fichas = Fichas::leer(&leer(catalogo::VOCAB)?, &leer(catalogo::CONFIG)?)
            .ok_or_else(|| ilegible(c))?;
        // Las fusiones solo hacen falta para el guion: se sueltan al salir.
        let guion = {
            let fusiones = Fusiones::leer(&leer(catalogo::MERGES)?);
            Guion::nuevo(&fichas, &fusiones).ok_or_else(|| ilegible(c))?
        };
        let config: &[(&CStr, &CStr)] = if reparto.preempaquetar {
            &[]
        } else {
            &[(c"session.disable_prepacking", c"1")]
        };
        tracing::info!(
            hilos = reparto.hilos,
            preempaquetar = reparto.preempaquetar,
            runtime = %rt.version,
            "Qwen3-ASR: como corre ONNX Runtime"
        );
        let abrir = |i: usize| {
            rt.abrir_con_config(
                &c.join(catalogo::FICHEROS[i].nombre),
                reparto.hilos,
                false,
                config,
            )
        };
        let conv = abrir(catalogo::CONV)?;
        let encoder = abrir(catalogo::ENCODER)?;
        let decoder = abrir(catalogo::DECODER)?;
        let capas = capas_del_decoder(&decoder).ok_or_else(|| ilegible(c))?;
        if conv.entradas.len() != 1
            || conv.salidas.len() != 1
            || encoder.entradas.len() != 2
            || encoder.salidas.len() != 1
        {
            return Err(ilegible(c));
        }
        Ok(Qwen3 {
            rt,
            conv,
            encoder,
            decoder,
            fichas,
            guion,
            capas,
        })
    }

    /// La version del runtime con la que corre (para el registro).
    pub fn version_del_runtime(&self) -> &str {
        &self.rt.version
    }

    /// **Pasa a texto un PCM de 16 bits mono a 16 kHz**, tanda a tanda.
    ///
    /// Con un idioma (o «todo en el primero») se le dice el primero. Con dos
    /// y «cada trozo como se dijo», el modelo adivina el de cada tanda; si
    /// dice uno que no es de los dos, o el texto no se sostiene en el, la
    /// tanda se rehace con el primero (la red de Whisper y del movil).
    /// `avance` de 0 a 1; `cancelado` se mira en cada token.
    pub fn transcribir_pcm(
        &self,
        muestras: &[i16],
        idiomas: &crate::Idiomas,
        avance: &mut dyn FnMut(f32),
        cancelado: &AtomicBool,
    ) -> Result<Transcripcion, ErrorVoz> {
        if muestras.is_empty() {
            return Err(ErrorVoz::AudioVacio);
        }
        let principal = idiomas.principal.as_str();
        let nombre =
            guion::nombre_del_idioma(principal).ok_or_else(|| ErrorVoz::IdiomaSinModelo {
                idioma: principal.to_string(),
            })?;
        // Un segundo idioma que Qwen no sabe es como no ponerlo.
        let adivinar = idiomas.adivinar() && idiomas.otro.as_deref().is_some_and(guion::sabe);
        let tandas = guion::tandas(muestras);
        let total = tandas.len().max(1) as f32;
        let mut segmentos = Vec::new();
        let mut avisos = 0usize;
        for (n, tanda) in tandas.iter().enumerate() {
            if cancelado.load(Ordering::Relaxed) {
                return Err(ErrorVoz::Cancelada);
            }
            let trozo = &muestras[tanda.clone()];
            if guion::callada(trozo) {
                avance((n + 1) as f32 / total);
                continue;
            }
            let audio: Vec<f32> = trozo.iter().map(|&m| m as f32 / 32_768.0).collect();
            let mut parte = |f: f32| avance((n as f32 + f.clamp(0.0, 1.0)) / total);
            let (lengua, oido) = if adivinar {
                let o = self.una_tanda(&audio, None, &mut parte, cancelado)?;
                match o.lengua {
                    Some(l) if idiomas.acepta(l, &o.texto) => (l.to_string(), o),
                    _ => (
                        principal.to_string(),
                        self.una_tanda(&audio, Some(nombre), &mut parte, cancelado)?,
                    ),
                }
            } else {
                (
                    principal.to_string(),
                    self.una_tanda(&audio, Some(nombre), &mut parte, cancelado)?,
                )
            };
            let texto = oido.texto.trim();
            if !texto.is_empty() {
                if creible(texto, &lengua) {
                    segmentos.push(Segmento::entre(
                        ms_de_muestra(tanda.start),
                        ms_de_muestra(tanda.end),
                        texto,
                    ));
                } else {
                    avisos += 1;
                }
            }
            avance((n + 1) as f32 / total);
        }
        avance(1.0);
        if segmentos.is_empty() {
            return Err(ErrorVoz::NoSeEntiendeNada);
        }
        Ok(Transcripcion {
            texto: con_tiempos(&segmentos, ""),
            estado: estado_de(segmentos.len(), avisos),
            duracion_ms: ms_de_muestra(muestras.len()),
            segmentos: lineas(&segmentos),
        })
    }

    /// **Una tanda**: mel → frente → encoder → decoder voraz. `nombre` es
    /// el idioma como lo entiende Qwen (`Spanish`) o `None`.
    pub fn una_tanda(
        &self,
        audio: &[f32],
        nombre: Option<&str>,
        avance: &mut dyn FnMut(f32),
        cancelado: &AtomicBool,
    ) -> Result<Oido, ErrorVoz> {
        let vacio = Oido {
            texto: String::new(),
            lengua: None,
        };
        let rt = self.rt;
        let datos_mel = mel::log_mel(audio);
        let marcos = datos_mel.len() / mel::BANDAS;
        if marcos < 2 {
            return Ok(vacio);
        }
        let entrada = rt.tensor_f32(datos_mel, &[1, marcos as i64, mel::BANDAS as i64])?;
        let conv = una_salida(self.conv.correr(&[&entrada])?, "correr el frente")?;
        let forma = conv.forma()?;
        let filas = forma.get(1).copied().unwrap_or(0).max(0) as usize;
        if forma.len() != 3 || filas == 0 {
            return Ok(vacio);
        }
        let validas = guion::tokens_de_audio(marcos).min(filas);
        let mascara = rt.tensor_bool(
            (0..filas).map(|i| i < validas).collect(),
            &[1, filas as i64],
        )?;
        let audio_v = una_salida(
            self.encoder.correr(&[&conv, &mascara])?,
            "correr el encoder",
        )?;
        drop((conv, mascara, entrada));
        let todo = audio_v.como_f32()?;
        let n = validas.min(guion::filas_con_voz(todo, ANCHO_AUDIO));
        if n == 0 || todo.len() < n * ANCHO_AUDIO {
            return Ok(vacio);
        }
        let vectores: Vec<f32> = todo[..n * ANCHO_AUDIO].to_vec();
        drop(audio_v);
        avance(0.2);

        let fuente = self.guion.fuente(n, nombre);
        let tope = guion::tope_de_tokens(audio.len());
        let largo = fuente.len() + tope;
        let mut cache = Cache::nueva(rt, self.capas, largo)?;
        // El audio entero, para las llamadas sin `<|audio_pad|>` (el grafo
        // lo pide siempre, aunque no lo use).
        let todos = rt.tensor_f32(vectores.clone(), &[1, n as i64, ANCHO_AUDIO as i64])?;

        // La entrada, en trozos: cada uno con los vectores de sus pads.
        let mut pos = 0usize;
        let mut usados = 0usize;
        let mut siguiente = 0i64;
        while pos < fuente.len() {
            if cancelado.load(Ordering::Relaxed) {
                return Err(ErrorVoz::Cancelada);
            }
            let trozo = &fuente[pos..(pos + TROZO_DE_ENTRADA).min(fuente.len())];
            let k = trozo.iter().filter(|&&t| t == self.guion.pad).count();
            let propios;
            let feats = if k > 0 {
                propios = rt.tensor_f32(
                    vectores[usados * ANCHO_AUDIO..(usados + k) * ANCHO_AUDIO].to_vec(),
                    &[1, k as i64, ANCHO_AUDIO as i64],
                )?;
                &propios
            } else {
                &todos
            };
            siguiente = self.paso(trozo, pos, feats, &mut cache)?;
            pos += trozo.len();
            usados += k;
        }
        avance(0.3);

        let esperados = (audio.len() as f32 / HERCIOS as f32 * 3.5).max(1.0);
        let mut escritos: Vec<i64> = Vec::new();
        let mut cur = fuente.len();
        while !self.guion.es_fin(siguiente) && escritos.len() < tope && cur < largo {
            if cancelado.load(Ordering::Relaxed) {
                return Err(ErrorVoz::Cancelada);
            }
            escritos.push(siguiente);
            if let Some(l) = guion::repite_al_final(&escritos) {
                escritos.truncate(l);
                break;
            }
            siguiente = self.paso(&[siguiente], cur, &todos, &mut cache)?;
            cur += 1;
            avance(0.3 + 0.7 * (escritos.len() as f32 / esperados).min(0.99));
        }
        avance(1.0);
        let (texto, lengua) = if nombre.is_some() {
            (&escritos[..], None)
        } else {
            self.guion.sin_prefijo(&escritos, &self.fichas)
        };
        Ok(Oido {
            texto: self.fichas.texto(texto).trim().to_string(),
            lengua,
        })
    }

    /// **Un paso del decoder**: `ids` desde la posicion `desde`, con la
    /// cache rellena hasta ahi. Escribe sus claves y valores en su sitio y
    /// devuelve el token elegido tras el ultimo.
    fn paso(
        &self,
        ids: &[i64],
        desde: usize,
        audio: &Valor<'static>,
        cache: &mut Cache,
    ) -> Result<i64, ErrorVoz> {
        let rt = self.rt;
        let s = ids.len();
        if desde + s > cache.largo {
            return Err(ErrorVoz::Onnx {
                paso: "correr el decoder",
                detalle: "la cache se quedo corta".into(),
            });
        }
        let t_ids = rt.tensor_i64(ids.to_vec(), &[1, s as i64])?;
        let t_mascara = rt.tensor_i64(vec![1; s], &[1, s as i64])?;
        let t_pos = rt.tensor_i64((desde as i64..(desde + s) as i64).collect(), &[s as i64])?;
        let salidas = {
            let mut entradas: Vec<&Valor<'static>> = Vec::with_capacity(4 + 2 * self.capas);
            entradas.extend([&t_ids, audio, &t_mascara, &t_pos]);
            for (k, v) in &cache.capas {
                entradas.push(k);
                entradas.push(v);
            }
            self.decoder.correr(&entradas)?
        };
        if salidas.len() != 1 + 2 * self.capas {
            return Err(ErrorVoz::Onnx {
                paso: "correr el decoder",
                detalle: format!("{} salidas", salidas.len()),
            });
        }
        let mut it = salidas.into_iter();
        let logits = it.next().ok_or_else(|| ErrorVoz::Onnx {
            paso: "correr el decoder",
            detalle: "sin logits".into(),
        })?;
        for (k, v) in cache.capas.iter_mut() {
            for (destino, delta) in [(&mut *k, it.next()), (&mut *v, it.next())] {
                let delta = delta.ok_or_else(|| ErrorVoz::Onnx {
                    paso: "correr el decoder",
                    detalle: "falta un delta de la cache".into(),
                })?;
                let d = delta.como_f32()?;
                if d.len() != s * POR_POSICION {
                    return Err(ErrorVoz::Onnx {
                        paso: "correr el decoder",
                        detalle: format!("delta de {} valores para {s} tokens", d.len()),
                    });
                }
                destino.como_f32_mut()?[desde * POR_POSICION..(desde + s) * POR_POSICION]
                    .copy_from_slice(d);
            }
        }
        let forma = logits.forma()?;
        let vocab = *forma.last().unwrap_or(&0) as usize;
        let filas = logits.como_f32()?;
        if vocab == 0 || filas.len() < vocab {
            return Err(ErrorVoz::Onnx {
                paso: "correr el decoder",
                detalle: format!("logits con forma {forma:?}"),
            });
        }
        Ok(guion::elegir(&filas[filas.len() - vocab..]))
    }
}

/// La cache de claves y valores de todas las capas, `[1, largo, 8, 128]`.
struct Cache {
    capas: Vec<(Valor<'static>, Valor<'static>)>,
    largo: usize,
}

impl Cache {
    fn nueva(rt: &'static Runtime, capas: usize, largo: usize) -> Result<Cache, ErrorVoz> {
        let forma = [1, largo as i64, CABEZAS_KV as i64, DIM_CABEZA as i64];
        let celdas = largo * POR_POSICION;
        let mut v = Vec::with_capacity(capas);
        for _ in 0..capas {
            v.push((
                rt.tensor_f32(vec![0.0; celdas], &forma)?,
                rt.tensor_f32(vec![0.0; celdas], &forma)?,
            ));
        }
        Ok(Cache { capas: v, largo })
    }
}

/// La unica salida de una sesion.
fn una_salida(v: Vec<Valor<'static>>, paso: &'static str) -> Result<Valor<'static>, ErrorVoz> {
    v.into_iter().next().ok_or_else(|| ErrorVoz::Onnx {
        paso,
        detalle: "no devolvio nada".into(),
    })
}

/// **Cuantas capas tiene el decoder**, comprobando que sus entradas y
/// salidas son las de la exportacion de sherpa-onnx y estan en orden
/// (`cache_key_0`, `cache_value_0`…; `logits`, `key_delta_0`…): se le
/// pasan por posicion.
fn capas_del_decoder(d: &Sesion<'_>) -> Option<usize> {
    capas_de(
        &d.entradas
            .iter()
            .map(|c| c.to_string_lossy())
            .collect::<Vec<_>>(),
        &d.salidas
            .iter()
            .map(|c| c.to_string_lossy())
            .collect::<Vec<_>>(),
    )
}

fn capas_de<S: AsRef<str>>(entradas: &[S], salidas: &[S]) -> Option<usize> {
    if entradas.len() < 6 || (entradas.len() - 4) % 2 != 0 {
        return None;
    }
    let capas = (entradas.len() - 4) / 2;
    if salidas.len() != 1 + 2 * capas || salidas[0].as_ref() != "logits" {
        return None;
    }
    let bien = ENTRADAS.iter().zip(entradas).all(|(a, b)| *a == b.as_ref())
        && (0..capas).all(|i| {
            entradas[4 + 2 * i].as_ref() == format!("cache_key_{i}")
                && entradas[5 + 2 * i].as_ref() == format!("cache_value_{i}")
                && salidas[1 + 2 * i].as_ref() == format!("key_delta_{i}")
                && salidas[2 + 2 * i].as_ref() == format!("value_delta_{i}")
        });
    bien.then_some(capas)
}

fn ms_de_muestra(m: usize) -> i64 {
    m as i64 * 1000 / HERCIOS as i64
}

// --- Lo que es solo de Windows: ficheros de audio y red -------------------

/// Si Qwen3-ASR corre en modo cuidadoso. Lo fija la app al arrancar (la
/// misma regla que Whisper y pdfsqueeze).
static CUIDADOSO: AtomicBool = AtomicBool::new(false);

pub fn fijar_modo_cuidadoso(cuidadoso: bool) {
    CUIDADOSO.store(cuidadoso, Ordering::Relaxed);
}

/// El reparto de este equipo, ahora.
pub fn reparto_de_este_equipo() -> Reparto {
    let nucleos = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    super::reparto(CUIDADOSO.load(Ordering::Relaxed), nucleos)
}

/// **Baja lo que falte** a `<datos>/qwen3-asr/`, comprobando el sha256 de
/// cada fichero (uno que no cuadra se borra y es un error). `avance` de 0
/// a 1 sobre el total. Tarda: hilo de fondo.
#[cfg(windows)]
pub fn bajar_modelo(
    raiz: &Path,
    avance: &mut dyn FnMut(f32),
    cancelado: &AtomicBool,
) -> Result<(), ErrorVoz> {
    let destino = carpeta(raiz);
    let fallo = |que: &str, e: &dyn std::fmt::Display| ErrorVoz::Descarga {
        que: que.to_string(),
        detalle: e.to_string(),
    };
    std::fs::create_dir_all(&destino).map_err(|e| fallo(&destino.display().to_string(), &e))?;
    let total: u64 = catalogo::FICHEROS.iter().map(|f| f.bytes).sum();
    let mut hecho = 0u64;
    for f in catalogo::FICHEROS {
        let p = destino.join(f.nombre);
        if !std::fs::metadata(&p).is_ok_and(|m| m.len() == f.bytes) {
            let antes = hecho;
            crate::whisper::bajar::bajar(
                f.url,
                &p,
                &mut |leidos, _| {
                    avance(((antes + leidos.min(f.bytes)) as f32 / total as f32).min(0.99))
                },
                cancelado,
            )?;
            let sha = super::sha256_de(&p).map_err(|e| fallo(f.nombre, &e))?;
            if sha != f.sha256 {
                let _ = std::fs::remove_file(&p);
                return Err(fallo(f.nombre, &format!("el sha256 no cuadra ({sha})")));
            }
        }
        hecho += f.bytes;
    }
    avance(1.0);
    tracing::info!(carpeta = %destino.display(), "Qwen3-ASR bajado");
    Ok(())
}

/// **Pasa una nota a texto con Qwen3-ASR**, con uno o dos idiomas (ver
/// [`Qwen3::transcribir_pcm`]) y, si los trae, por turnos (como
/// `whisper::transcribir_con`).
#[cfg(windows)]
pub fn transcribir_con(
    audio: &Path,
    raiz: &Path,
    junto_al_exe: Option<&Path>,
    idiomas: &crate::Idiomas,
    turnos: &[crate::turnos::Turno],
    avance: &mut dyn FnMut(f32),
    cancelado: &AtomicBool,
) -> Result<Transcripcion, ErrorVoz> {
    let q = Qwen3::cargar(raiz, junto_al_exe, reparto_de_este_equipo())?;
    let muestras = crate::pcm::decodificar(audio)?;
    if cancelado.load(Ordering::Relaxed) {
        return Err(ErrorVoz::Cancelada);
    }
    let rangos = crate::turnos::rangos(turnos, muestras.len(), HERCIOS);
    if rangos.is_empty() {
        return q.transcribir_pcm(&muestras, idiomas, avance, cancelado);
    }
    let mut oidos = Vec::with_capacity(rangos.len());
    let total = rangos.len() as f32;
    for (n, (i, r)) in rangos.iter().enumerate() {
        let desde = ms_de_muestra(r.start);
        let mut parte = |f: f32| avance((n as f32 + f) / total);
        let segmentos =
            match q.transcribir_pcm(&muestras[r.clone()], idiomas, &mut parte, cancelado) {
                Ok(t) => t.segmentos.into_iter().map(|s| s.corrido(desde)).collect(),
                Err(ErrorVoz::NoSeEntiendeNada | ErrorVoz::AudioVacio) => Vec::new(),
                Err(e) => return Err(e),
            };
        oidos.push(crate::turnos::Oido {
            turno: &turnos[*i],
            segmentos,
        });
    }
    avance(1.0);
    let con_texto = oidos.iter().filter(|o| !o.segmentos.is_empty()).count();
    if con_texto == 0 {
        return Err(ErrorVoz::NoSeEntiendeNada);
    }
    Ok(Transcripcion {
        texto: crate::turnos::dialogo(&oidos),
        estado: crate::turnos::estado(con_texto, oidos.len()),
        duracion_ms: ms_de_muestra(muestras.len()),
        segmentos: oidos.into_iter().flat_map(|o| o.segmentos).collect(),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn nombres(capas: usize) -> (Vec<String>, Vec<String>) {
        let mut e: Vec<String> = ENTRADAS.iter().map(|s| s.to_string()).collect();
        let mut s = vec!["logits".to_string()];
        for i in 0..capas {
            e.push(format!("cache_key_{i}"));
            e.push(format!("cache_value_{i}"));
            s.push(format!("key_delta_{i}"));
            s.push(format!("value_delta_{i}"));
        }
        (e, s)
    }

    #[test]
    fn las_capas_salen_de_los_nombres_si_estan_en_orden() {
        let (e, s) = nombres(28);
        assert_eq!(capas_de(&e, &s), Some(28));
        let (mut e2, s2) = nombres(28);
        e2.swap(4, 5);
        assert_eq!(capas_de(&e2, &s2), None, "clave y valor cambiados");
        let (e3, mut s3) = nombres(28);
        s3.pop();
        assert_eq!(capas_de(&e3, &s3), None);
        assert_eq!(capas_de::<&str>(&[], &[]), None);
    }

    /// **Con el modelo de verdad, en Linux o Windows**: `PIXPIN_QWEN3` (la
    /// carpeta con los seis ficheros), `PIXPIN_ORT` (la carpeta con
    /// `libonnxruntime.so` / `onnxruntime.dll`) y `PIXPIN_CLIPS` (una
    /// carpeta con `uNN.wav` PCM de 16 bits a 16 kHz y su `meta.json`).
    /// Escribe lo oido a `PIXPIN_SALIDA` (JSON) y el tiempo de cada clip.
    /// `cargo test -p pixpin-voz --release -- --ignored qwen3_de_verdad`.
    #[test]
    #[ignore = "pide el modelo Qwen3-ASR bajado y ONNX Runtime"]
    fn qwen3_de_verdad() {
        let modelo = std::path::PathBuf::from(std::env::var("PIXPIN_QWEN3").expect("PIXPIN_QWEN3"));
        let ort = std::path::PathBuf::from(std::env::var("PIXPIN_ORT").expect("PIXPIN_ORT"));
        let clips = std::path::PathBuf::from(std::env::var("PIXPIN_CLIPS").expect("PIXPIN_CLIPS"));
        let n: usize = std::env::var("PIXPIN_N")
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(44);
        let cuidadoso = std::env::var("PIXPIN_CUIDADOSO").is_ok();
        let vacia = std::env::temp_dir().join("pixpin-voz-qwen3-raiz-vacia");
        let rt = Runtime::del_proceso(&vacia, Some(&ort)).expect("runtime");
        let t0 = std::time::Instant::now();
        let q = Qwen3::cargar_de(rt, &modelo, super::super::reparto(cuidadoso, 4)).expect("carga");
        let carga = t0.elapsed().as_secs_f32();
        let meta: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(clips.join("meta.json")).unwrap())
                .unwrap();
        let mut salida = Vec::new();
        let (mut audio_s, mut calculo_s) = (0f32, 0f32);
        for m in meta.as_array().unwrap().iter().take(n) {
            let uid = m["uid"].as_str().unwrap();
            let muestras = leer_wav(&clips.join(format!("{uid}.wav")));
            let t = std::time::Instant::now();
            let r = q.transcribir_pcm(
                &muestras,
                &crate::Idiomas::uno("es"),
                &mut |_| {},
                &AtomicBool::new(false),
            );
            let dt = t.elapsed().as_secs_f32();
            let dur = muestras.len() as f32 / HERCIOS as f32;
            audio_s += dur;
            calculo_s += dt;
            let texto = match r {
                Ok(t) => t
                    .segmentos
                    .iter()
                    .map(|s| s.texto.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
                Err(e) => format!("<{e}>"),
            };
            eprintln!("{uid} rtf {:.3} {}", dt / dur, texto);
            salida.push(serde_json::json!({"uid": uid, "hyp": texto, "sec": dt, "dur": dur}));
        }
        // El pico de memoria del proceso (Linux: `VmHWM`).
        let pico = std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("VmHWM"))
                    .map(|l| l.to_string())
            })
            .unwrap_or_default();
        eprintln!(
            "carga {carga:.1} s | RTF {:.3} | runtime {} | {pico}",
            calculo_s / audio_s,
            q.version_del_runtime()
        );
        if let Ok(p) = std::env::var("PIXPIN_SALIDA") {
            let j =
                serde_json::json!({"items": salida, "rtf": calculo_s / audio_s, "carga_s": carga});
            std::fs::write(p, j.to_string()).unwrap();
        }
    }

    /// Un WAV PCM de 16 bits mono (lo que escribe el guion de los clips).
    fn leer_wav(p: &Path) -> Vec<i16> {
        let b = std::fs::read(p).unwrap();
        let mut i = 12;
        while i + 8 <= b.len() {
            let largo = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
            if &b[i..i + 4] == b"data" {
                return b[i + 8..(i + 8 + largo).min(b.len())]
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();
            }
            i += 8 + largo + (largo & 1);
        }
        panic!("sin datos en {}", p.display());
    }
}
