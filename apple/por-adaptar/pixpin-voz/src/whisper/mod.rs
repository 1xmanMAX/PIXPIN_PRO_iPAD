//! **Whisper sin conexion**, con el ONNX Runtime que ya trae Windows.
//!
//! Puerto de `guardados/MotorWhisper.kt` del movil: **el mismo modelo**
//! (Whisper *base* de sherpa-onnx, int8), los mismos tres ficheros bajados
//! una vez de Hugging Face, las mismas tandas de 20 s y la misma lista negra
//! contra lo que Whisper se inventa. El movil lo corre con la biblioteca de
//! sherpa-onnx; aqui no hay sherpa: el decodificador es el de su ejemplo en
//! Python (`scripts/whisper/test.py` y `offline-whisper-greedy-search-
//! decoder.cc` de `k2-fsa/sherpa-onnx`), escrito en Rust sobre [`crate::ort`].
//!
//! ## Por que Whisper y no el reconocedor de Windows
//!
//! Medido con las notas reales del usuario (2026-09-22): SAPI escribio
//! «Goleo hora de control Trieste meditacion…» donde se decia otra cosa.
//! Whisper base entiende el habla natural y escribe con mayusculas y
//! puntuacion. SAPI se queda para dictar en vivo, que Whisper no hace.
//!
//! ## Como se transcribe una nota
//!
//! 1. El audio se parte en frases y las frases en tandas de hasta 20 s
//!    ([`cortes`]); cada tanda sabe en que milisegundo empieza.
//! 2. Cada tanda pasa a log-mel de 30 s ([`mel`]) y por el **encoder**, que
//!    devuelve las claves y valores de atencion cruzada.
//! 3. El **decoder** escribe token a token, voraz (siempre el mas probable),
//!    con la cache de autoatencion que devuelve cada paso. Empieza con
//!    `[sot, idioma, transcribe, no_timestamps]`: **el idioma se le dice**,
//!    no se le deja adivinar (el movil aprendio que adivinando con un modelo
//!    pequeno sale en lenguas que nadie hablo).
//! 4. Los tokens pasan a texto ([`fichas`]) y el texto pasa por
//!    [`creible`](crate::credibilidad::creible).

pub mod bajar;
pub mod cortes;
pub mod fichas;
pub mod mel;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::credibilidad::creible;
use crate::dos_idiomas::{Idiomas, MINIMO_PARA_ADIVINAR_MS, Tarea};
use crate::motor::Transcripcion;
use crate::ort::{Runtime, Sesion, Valor};
use crate::tiempos::{Segmento, con_tiempos, estado_de};
use crate::{ErrorVoz, HERCIOS, pcm};

use fichas::Fichas;

/// El tamano elegido por el usuario: *base*, «el punto medio» del movil.
pub const MODELO: &str = "base";

/// De donde se baja, el mismo repositorio que el movil
/// (`MotorWhisper.repositorio`).
pub const REPOSITORIO: &str =
    "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base/resolve/main/";

/// Los tres ficheros y lo que pesan, en el orden en que se bajan: primero
/// el pequeno, para que un fallo de red se note en el acto. Los pesos solo
/// sirven para repartir la barra de avance; el tamano bueno lo dice el
/// servidor.
pub const FICHEROS: [(&str, u64); 3] = [
    ("base-tokens.txt", 816_730),
    ("base-encoder.int8.onnx", 29_120_534),
    ("base-decoder.int8.onnx", 130_672_026),
];

/// Lo que se ensena al usuario: «unos 160 MB».
pub const MEGAS: u64 = 160;

/// `TANDA_SEGUNDOS` del movil: cuesta casi lo mismo una tanda de 2 s que
/// una de 20 (el encoder oye siempre 30), y con mas contexto inventa menos.
const TANDA_SEGUNDOS: usize = 20;

/// Los 99 idiomas de Whisper (`MotorWhisper.IDIOMAS`). Otro cualquiera no
/// tiene token de idioma y no se puede forzar.
const IDIOMAS: &str = "en zh de es ru ko fr ja pt tr pl ca nl ar sv it id hi fi vi he uk el ms cs ro da hu ta no th ur hr bg lt la mi ml cy sk te fa lv bn sr az sl kn et mk br eu is hy ne mn bs kk sq sw gl mr pa si km sn yo so af oc ka be tg sd gu am yi lo uz fo ht ps tk nn mt sa lb my bo tl mg as tt haw ln ha ba jw su yue";

/// Si Whisper sabe el idioma (`es`, `es-PE`, `en-US`…).
pub fn sabe(idioma: &str) -> bool {
    let lengua = idioma.split('-').next().unwrap_or("").to_lowercase();
    !lengua.is_empty() && IDIOMAS.split(' ').any(|c| c == lengua)
}

/// `<datos>/whisper/base/`, como el `filesDir/whisper/base` del movil.
pub fn carpeta(raiz: &Path) -> PathBuf {
    raiz.join("whisper").join(MODELO)
}

/// Si los tres ficheros estan y no estan vacios. Un `.parte` no cuenta:
/// es una descarga a medias.
pub fn modelo_listo(raiz: &Path) -> bool {
    let c = carpeta(raiz);
    FICHEROS
        .iter()
        .all(|(n, _)| std::fs::metadata(c.join(n)).is_ok_and(|m| m.len() > 0))
}

/// Si hay un ONNX Runtime que cargar (sin cargarlo).
pub fn runtime_disponible(raiz: &Path, junto_al_exe: Option<&Path>) -> bool {
    crate::ort::buscar(raiz, junto_al_exe).is_some()
}

/// **Baja lo que falte del modelo** a `<datos>/whisper/base/`. `avance` de
/// 0 a 1 sobre el total de los tres. Tarda: hilo de fondo.
pub fn bajar_modelo(
    raiz: &Path,
    avance: &mut dyn FnMut(f32),
    cancelado: &AtomicBool,
) -> Result<(), ErrorVoz> {
    let destino = carpeta(raiz);
    std::fs::create_dir_all(&destino).map_err(|e| ErrorVoz::Descarga {
        que: destino.display().to_string(),
        detalle: e.to_string(),
    })?;
    let total: u64 = FICHEROS.iter().map(|(_, p)| p).sum();
    let mut hecho: u64 = 0;
    for (nombre, peso) in FICHEROS {
        let fichero = destino.join(nombre);
        if std::fs::metadata(&fichero).is_ok_and(|m| m.len() > 0) {
            hecho += peso;
            continue;
        }
        let url = format!("{REPOSITORIO}{nombre}");
        let antes = hecho;
        bajar::bajar(
            &url,
            &fichero,
            &mut |leidos, _| {
                // Con el peso previsto y no con el que diga el servidor: asi
                // la barra no salta atras al empezar el fichero siguiente.
                let va = antes + leidos.min(peso);
                avance((va as f32 / total as f32).min(0.99));
            },
            cancelado,
        )?;
        hecho += peso;
    }
    avance(1.0);
    Ok(())
}

/// Los numeros del modelo que vienen en sus metadatos.
#[derive(Debug, Clone)]
struct Metadatos {
    n_text_layer: i64,
    n_text_ctx: i64,
    n_text_state: i64,
    sot: i64,
    eot: i64,
    no_timestamps: i64,
    no_speech: i64,
    blank: i64,
    transcribe: i64,
    /// `translate`: solo para «todo en el primero» con el primero en ingles
    /// (ver [`crate::dos_idiomas`]). Un modelo que no lo trae no traduce.
    translate: Option<i64>,
    codigos: String,
    tokens_de_idioma: String,
}

impl Metadatos {
    fn leer(s: &Sesion<'_>) -> Result<Metadatos, ErrorVoz> {
        let texto = |clave: &'static str| -> Result<String, ErrorVoz> {
            s.metadato(clave)?.ok_or_else(|| ErrorVoz::Onnx {
                paso: "leer los metadatos del modelo",
                detalle: format!("falta «{clave}»"),
            })
        };
        let entero = |clave: &'static str| -> Result<i64, ErrorVoz> {
            texto(clave)?.trim().parse().map_err(|_| ErrorVoz::Onnx {
                paso: "leer los metadatos del modelo",
                detalle: format!("«{clave}» no es un numero"),
            })
        };
        Ok(Metadatos {
            n_text_layer: entero("n_text_layer")?,
            n_text_ctx: entero("n_text_ctx")?,
            n_text_state: entero("n_text_state")?,
            sot: entero("sot")?,
            eot: entero("eot")?,
            no_timestamps: entero("no_timestamps")?,
            no_speech: entero("no_speech")?,
            blank: entero("blank_id")?,
            transcribe: entero("transcribe")?,
            translate: s.metadato("translate")?.and_then(|t| t.trim().parse().ok()),
            codigos: texto("all_language_codes")?,
            tokens_de_idioma: texto("all_language_tokens")?,
        })
    }

    /// `[sot, idioma, transcribe, no_timestamps]`.
    fn inicio(&self, idioma: &str) -> Option<Vec<i64>> {
        self.inicio_con(idioma, Tarea::Transcribir)
    }

    /// `[sot, idioma, tarea, no_timestamps]`. Sin token de `translate` en el
    /// modelo se transcribe: mejor el texto en su idioma que ninguno.
    fn inicio_con(&self, idioma: &str, tarea: Tarea) -> Option<Vec<i64>> {
        let lengua = fichas::token_del_idioma(&self.codigos, &self.tokens_de_idioma, idioma)?;
        let tarea = match tarea {
            Tarea::Traducir => self.translate.unwrap_or(self.transcribe),
            Tarea::Transcribir => self.transcribe,
        };
        Some(vec![self.sot, lengua, tarea, self.no_timestamps])
    }

    /// Los tokens de idioma con su codigo, en el orden de los metadatos.
    fn idiomas(&self) -> Vec<(String, i64)> {
        let tokens = fichas::enteros(&self.tokens_de_idioma).unwrap_or_default();
        self.codigos
            .split(',')
            .map(|c| c.trim().to_string())
            .zip(tokens)
            .collect()
    }
}

/// De entre los tokens de idioma, el de logit mas alto: es como adivina
/// Whisper el idioma (`detect_language` de `whisper/decoding.py` y
/// `DetectLanguage` de sherpa-onnx), mirando la fila de despues de `sot`.
fn idioma_mas_probable(fila: &[f32], idiomas: &[(String, i64)]) -> Option<String> {
    idiomas
        .iter()
        .filter_map(|(c, t)| fila.get(usize::try_from(*t).ok()?).map(|l| (c, *l)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(c, _)| c.clone())
}

/// Como corre ONNX Runtime: cuantos hilos y si usa su arena de memoria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reparto {
    pub hilos: usize,
    pub arena: bool,
}

/// **El reparto segun el equipo.** En modo cuidadoso (4 GB o menos, o
/// nivel Ligero: la misma regla que el modo cuidadoso de pdfsqueeze, que
/// decide la app) sin arena y con uno o dos hilos: pico de ~350 MB en vez de
/// ~720, aunque tarde mas. Si no, los nucleos que haya hasta ocho (pasado
/// eso el encoder de *base* ya no gana) y con arena.
pub fn reparto(cuidadoso: bool, nucleos: usize) -> Reparto {
    if cuidadoso {
        Reparto {
            hilos: nucleos.clamp(1, 2),
            arena: false,
        }
    } else {
        Reparto {
            hilos: nucleos.clamp(1, 8),
            arena: true,
        }
    }
}

/// Si Whisper corre en modo cuidadoso. Lo fija la app al arrancar.
static CUIDADOSO: AtomicBool = AtomicBool::new(false);

/// Lo fija la app al arrancar, con la misma regla que el modo cuidadoso de
/// pdfsqueeze.
pub fn fijar_modo_cuidadoso(cuidadoso: bool) {
    CUIDADOSO.store(cuidadoso, Ordering::Relaxed);
}

/// El reparto de este equipo, ahora.
pub fn reparto_de_este_equipo() -> Reparto {
    let nucleos = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    reparto(CUIDADOSO.load(Ordering::Relaxed), nucleos)
}

/// **El modelo cargado**: encoder, decoder, tokens y metadatos.
pub struct Whisper {
    rt: &'static Runtime,
    encoder: Sesion<'static>,
    decoder: Sesion<'static>,
    fichas: Fichas,
    meta: Metadatos,
}

/// Lo que sale de una tanda.
struct Oido {
    texto: String,
    /// Probabilidad de «aqui no habla nadie» (`no_speech`) tras `sot`.
    sin_voz: f32,
    /// Media del logaritmo de la probabilidad de cada token elegido.
    confianza: f32,
}

impl Whisper {
    /// Carga el runtime y los tres ficheros de `<datos>/whisper/base/`.
    pub fn cargar(raiz: &Path, junto_al_exe: Option<&Path>) -> Result<Whisper, ErrorVoz> {
        let rt = Runtime::del_proceso(raiz, junto_al_exe)?;
        if !modelo_listo(raiz) {
            return Err(ErrorVoz::SinModeloWhisper {
                donde: carpeta(raiz).display().to_string(),
                megas: MEGAS,
            });
        }
        let c = carpeta(raiz);
        let ilegible = || ErrorVoz::ModeloIlegible {
            donde: c.display().to_string(),
        };
        let tokens = std::fs::read_to_string(c.join(FICHEROS[0].0)).map_err(|_| ilegible())?;
        let fichas = Fichas::leer(&tokens);
        if fichas.is_empty() {
            return Err(ilegible());
        }
        let r = reparto_de_este_equipo();
        tracing::info!(
            hilos = r.hilos,
            arena = r.arena,
            "Whisper: como corre ONNX Runtime"
        );
        let encoder = rt.abrir_con(&c.join(FICHEROS[1].0), r.hilos, r.arena)?;
        let decoder = rt.abrir_con(&c.join(FICHEROS[2].0), r.hilos, r.arena)?;
        if encoder.entradas.len() != 1
            || encoder.salidas.len() < 2
            || decoder.entradas.len() != 6
            || decoder.salidas.len() < 3
        {
            return Err(ilegible());
        }
        let meta = Metadatos::leer(&encoder)?;
        Ok(Whisper {
            rt,
            encoder,
            decoder,
            fichas,
            meta,
        })
    }

    /// **Pasa a texto un PCM de 16 bits mono a 16 kHz**, tanda a tanda, con
    /// un solo idioma forzado.
    pub fn transcribir_pcm(
        &self,
        muestras: &[i16],
        idioma: &str,
        avance: &mut dyn FnMut(f32),
        cancelado: &AtomicBool,
    ) -> Result<Transcripcion, ErrorVoz> {
        self.transcribir_pcm_con(muestras, &Idiomas::uno(idioma), avance, cancelado)
    }

    /// **Pasa a texto un PCM con uno o dos idiomas** (B6, ver
    /// [`crate::dos_idiomas`]). Cada tanda pasa una vez por el encoder; el
    /// decoder corre una vez, o dos si lo adivinado no se sostiene.
    pub fn transcribir_pcm_con(
        &self,
        muestras: &[i16],
        idiomas: &Idiomas,
        avance: &mut dyn FnMut(f32),
        cancelado: &AtomicBool,
    ) -> Result<Transcripcion, ErrorVoz> {
        if muestras.is_empty() {
            return Err(ErrorVoz::AudioVacio);
        }
        let principal = idiomas.principal.clone();
        let con_el_principal = self
            .meta
            .inicio_con(&principal, idiomas.tarea())
            .ok_or_else(|| ErrorVoz::IdiomaSinModelo {
                idioma: principal.clone(),
            })?;
        // Un segundo idioma sin token en el modelo es como no ponerlo.
        let idiomas = match &idiomas.otro {
            Some(o) if self.meta.inicio(o).is_none() => Idiomas::uno(&principal),
            _ => idiomas.clone(),
        };
        // Con dos idiomas, una pausa entre frases cierra la tanda: el idioma
        // se decide por tanda y el cambio suele ir entre frases. Tambien con
        // «todo en el primero»: una tanda que mezcla los dos sale solo con la
        // parte del primero (medido), y partida, la del otro sale traducida.
        let tandas = if idiomas.otro.is_some() {
            tandas_con(muestras, Some(PAUSA_ENTRE_IDIOMAS_MS))
        } else {
            tandas(muestras)
        };
        let mut segmentos = Vec::new();
        let mut avisos = 0usize;
        for (i, tanda) in tandas.iter().enumerate() {
            if cancelado.load(Ordering::Relaxed) {
                return Err(ErrorVoz::Cancelada);
            }
            let audio: Vec<f32> = muestras[tanda.clone()]
                .iter()
                .map(|&m| m as f32 / 32_768.0)
                .collect();
            let (lengua, oido) = self.una_tanda(&audio, &idiomas, &con_el_principal, cancelado)?;
            // Lo que hace Whisper con los silencios (`transcribe.py`): si
            // cree que no habla nadie y ademas no esta seguro de lo que
            // escribio, es un silencio, no una frase.
            let callado = oido.sin_voz > 0.6 && oido.confianza < -1.0;
            if !callado && !oido.texto.is_empty() {
                if creible(&oido.texto, &lengua) {
                    segmentos.push(Segmento::nuevo(pcm::ms_de_muestra(tanda.start), oido.texto));
                } else {
                    avisos += 1;
                }
            }
            avance((i + 1) as f32 / tandas.len() as f32);
        }
        avance(1.0);
        if segmentos.is_empty() {
            return Err(ErrorVoz::NoSeEntiendeNada);
        }
        Ok(Transcripcion {
            texto: con_tiempos(&segmentos, ""),
            estado: estado_de(segmentos.len(), avisos),
            duracion_ms: pcm::duracion_ms(muestras),
            segmentos,
        })
    }

    /// Una tanda con las reglas del movil (`MotorWhisper.kt:197-234`).
    /// Devuelve en que idioma quedo y lo oido.
    fn una_tanda(
        &self,
        audio: &[f32],
        idiomas: &Idiomas,
        con_el_principal: &[i64],
        cancelado: &AtomicBool,
    ) -> Result<(String, Oido), ErrorVoz> {
        let cruz = self.encoder(audio)?;
        let principal = idiomas.principal.clone();
        let dura_ms = pcm::ms_de_muestra(audio.len());
        let mut lengua = principal.clone();
        let mut oido = None;
        if idiomas.adivinar() && dura_ms >= MINIMO_PARA_ADIVINAR_MS {
            // Solo se decodifica en lo adivinado si es uno de los dos: si
            // Whisper dice otra lengua, el movil lo tira y rehace con el
            // primero; aqui se va directo al primero y se ahorra la vuelta.
            let adivinado = self
                .adivinar(&cruz)?
                .filter(|c| *c == principal || idiomas.otro.as_deref() == Some(c.as_str()));
            if let Some(c) = adivinado
                && c != principal
                && let Some(inicio) = self.meta.inicio(&c)
            {
                let o = self.decodificar(&cruz, audio.len(), &inicio, cancelado)?;
                if idiomas.acepta(&c, &o.texto) {
                    lengua = c;
                    oido = Some(o);
                }
            }
        }
        let mut oido = match oido {
            Some(o) => o,
            None => self.decodificar(&cruz, audio.len(), con_el_principal, cancelado)?,
        };
        // **Y si aun asi no se sostiene**, una vez mas con el primero antes
        // de tirarlo. Con lo de arriba no deberia pasar (lo del otro ya se
        // comprobo), pero es la red del movil y no cuesta nada.
        if idiomas.reintentar_con_el_primero(&lengua, &oido.texto) {
            oido = self.decodificar(&cruz, audio.len(), con_el_principal, cancelado)?;
            lengua = principal;
        }
        Ok((lengua, oido))
    }

    /// El encoder: log-mel de 30 s y las claves y valores de atencion
    /// cruzada.
    fn encoder(&self, audio: &[f32]) -> Result<(Valor<'static>, Valor<'static>), ErrorVoz> {
        let mel = self.rt.tensor_f32(
            mel::log_mel(audio),
            &[1, mel::BANDAS as i64, mel::MARCOS as i64],
        )?;
        let mut cruz = self.encoder.correr(&[&mel])?.into_iter();
        let (Some(cruz_k), Some(cruz_v)) = (cruz.next(), cruz.next()) else {
            return Err(ErrorVoz::Onnx {
                paso: "correr el encoder",
                detalle: "no devolvio las dos salidas".into(),
            });
        };
        Ok((cruz_k, cruz_v))
    }

    /// Las caches de autoatencion vacias del primer paso del decoder.
    fn caches_vacias(&self) -> Result<(Valor<'static>, Valor<'static>), ErrorVoz> {
        let m = &self.meta;
        let forma = [m.n_text_layer, 1, m.n_text_ctx, m.n_text_state];
        let celdas = forma.iter().product::<i64>().max(0) as usize;
        Ok((
            self.rt.tensor_f32(vec![0.0; celdas], &forma)?,
            self.rt.tensor_f32(vec![0.0; celdas], &forma)?,
        ))
    }

    /// **Que idioma oye Whisper** en esta tanda: un paso del decoder con
    /// solo `sot` y el token de idioma mas probable.
    fn adivinar(
        &self,
        cruz: &(Valor<'static>, Valor<'static>),
    ) -> Result<Option<String>, ErrorVoz> {
        let (cache_k, cache_v) = self.caches_vacias()?;
        let tokens = self.rt.tensor_i64(vec![self.meta.sot], &[1, 1])?;
        let offset = self.rt.tensor_i64(vec![0], &[1])?;
        let salidas = self
            .decoder
            .correr(&[&tokens, &cache_k, &cache_v, &cruz.0, &cruz.1, &offset])?;
        let Some(logits) = salidas.into_iter().next() else {
            return Ok(None);
        };
        let vocab = *logits.forma()?.last().unwrap_or(&0) as usize;
        let filas = logits.como_f32()?;
        if vocab == 0 || filas.len() < vocab {
            return Ok(None);
        }
        let oido = idioma_mas_probable(&filas[filas.len() - vocab..], &self.meta.idiomas());
        Ok(oido.map(|c| crate::dos_idiomas::limpiar_codigo(&c)))
    }

    /// El decoder voraz desde `inicio`, sobre lo que dio el encoder.
    /// `muestras` es lo largo de la tanda (para el tope de tokens).
    fn decodificar(
        &self,
        cruz: &(Valor<'static>, Valor<'static>),
        muestras: usize,
        inicio: &[i64],
        cancelado: &AtomicBool,
    ) -> Result<Oido, ErrorVoz> {
        let rt = self.rt;
        let m = &self.meta;
        let (cruz_k, cruz_v) = cruz;
        let (mut cache_k, mut cache_v) = self.caches_vacias()?;

        // Cuantos tokens como mucho: el movil (sherpa) supone seis por
        // segundo de audio; aqui diez, porque el castellano rapido pasa de
        // seis y cortar una frase buena es peor que dejar correr una mala
        // (esa la para [`repite_al_final`] o la tira `creible`). Nunca mas de
        // media ventana de texto, como sherpa.
        let segundos = muestras as f32 / HERCIOS as f32;
        let tope = ((segundos * 10.0).ceil() as i64 + 8).min(m.n_text_ctx / 2) as usize;

        let mut tokens = rt.tensor_i64(inicio.to_vec(), &[1, inicio.len() as i64])?;
        let mut desplazamiento: i64 = 0;
        let mut elegidos: Vec<i64> = Vec::new();
        let mut suma_log = 0f32;
        let mut sin_voz = 0f32;
        let mut primero = true;
        loop {
            if cancelado.load(Ordering::Relaxed) {
                return Err(ErrorVoz::Cancelada);
            }
            let offset = rt.tensor_i64(vec![desplazamiento], &[1])?;
            let salidas = self
                .decoder
                .correr(&[&tokens, &cache_k, &cache_v, cruz_k, cruz_v, &offset])?;
            let mut it = salidas.into_iter();
            let (Some(logits), Some(k), Some(v)) = (it.next(), it.next(), it.next()) else {
                return Err(ErrorVoz::Onnx {
                    paso: "correr el decoder",
                    detalle: "no devolvio logits y cache".into(),
                });
            };
            let forma = logits.forma()?;
            let vocab = *forma.last().unwrap_or(&0) as usize;
            let filas = logits.como_f32()?;
            if vocab == 0 || filas.len() < vocab {
                return Err(ErrorVoz::Onnx {
                    paso: "correr el decoder",
                    detalle: format!("logits con forma {forma:?}"),
                });
            }
            if primero {
                // La fila de despues de `sot` es donde Whisper dice si hay
                // voz: la probabilidad de `no_speech` ahi.
                sin_voz = probabilidad(&filas[..vocab], m.no_speech);
            }
            let ultima = &filas[filas.len() - vocab..];
            let (token, log) = elegir(ultima, m, primero);
            desplazamiento += if primero { inicio.len() as i64 } else { 1 };
            primero = false;
            cache_k = k;
            cache_v = v;
            if token == m.eot || elegidos.len() >= tope || desplazamiento >= m.n_text_ctx - 1 {
                break;
            }
            elegidos.push(token);
            suma_log += log;
            if let Some(largo) = repite_al_final(&elegidos) {
                elegidos.truncate(largo);
                break;
            }
            tokens = rt.tensor_i64(vec![token], &[1, 1])?;
        }
        let confianza = if elegidos.is_empty() {
            0.0
        } else {
            suma_log / elegidos.len() as f32
        };
        Ok(Oido {
            texto: self.fichas.texto(&elegidos),
            sin_voz,
            confianza,
        })
    }
}

/// Las tandas de una nota: frases juntas hasta 20 s, y ninguna de mas de
/// 30 (lo que oye el encoder; con cortes de 8 s no deberia pasar, pero una
/// tanda mas larga perderia su final sin decir nada).
fn tandas(muestras: &[i16]) -> Vec<std::ops::Range<usize>> {
    tandas_con(muestras, None)
}

/// Lo que separa dos frases que pueden ir en idiomas distintos (B6): mas
/// que una coma o un respiro, menos que un punto y aparte.
const PAUSA_ENTRE_IDIOMAS_MS: usize = 700;

/// Las tandas, cortando ademas en las pausas de `pausa_ms` si se da.
fn tandas_con(muestras: &[i16], pausa_ms: Option<usize>) -> Vec<std::ops::Range<usize>> {
    let tope = mel::MUESTRAS_30S;
    let frases = cortes::cortes(muestras);
    match pausa_ms {
        Some(p) => {
            cortes::agrupar_por_frases(&frases, TANDA_SEGUNDOS, &cortes::pausas_largas(muestras, p))
        }
        None => cortes::agrupar(&frases, TANDA_SEGUNDOS),
    }
    .into_iter()
    .flat_map(|r| {
        (r.start..r.end)
            .step_by(tope)
            .map(move |a| a..(a + tope).min(r.end))
    })
    .collect()
}

/// El token mas probable que se puede escribir, y el logaritmo de su
/// probabilidad.
///
/// Solo compiten el texto (`0..eot`) y el fin (`eot`): los especiales de
/// detras (idiomas, tareas, marcas de tiempo, `no_speech`) no son texto y
/// sherpa los suprime uno a uno; aqui se suprimen todos de una vez. En el
/// primer paso tampoco valen el fin ni el blanco suelto: una tanda no
/// empieza terminando (`suppress_tokens(is_initial=True)`).
fn elegir(logits: &[f32], m: &Metadatos, primero: bool) -> (i64, f32) {
    let hasta = (m.eot as usize).min(logits.len().saturating_sub(1));
    let mut mejor = 0usize;
    let mut valor = f32::NEG_INFINITY;
    for (i, &l) in logits[..=hasta].iter().enumerate() {
        let i64_ = i as i64;
        if primero && (i64_ == m.eot || i64_ == m.blank) {
            continue;
        }
        if l > valor {
            valor = l;
            mejor = i;
        }
    }
    (mejor as i64, valor - log_suma_exp(logits))
}

fn log_suma_exp(v: &[f32]) -> f32 {
    let tope = v.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    if !tope.is_finite() {
        return tope;
    }
    tope + v.iter().map(|&x| (x - tope).exp()).sum::<f32>().ln()
}

/// La probabilidad (softmax) de un token en una fila de logits.
fn probabilidad(fila: &[f32], token: i64) -> f32 {
    match fila.get(token as usize) {
        Some(&l) if token >= 0 => (l - log_suma_exp(fila)).exp(),
        _ => 0.0,
    }
}

/// **Si el final se ha metido en un bucle**, el largo que hay que dejar.
///
/// Whisper voraz, en cuanto duda, repite la misma coletilla hasta el tope
/// («y luego y luego y luego…»). Cuatro repeticiones seguidas de un mismo
/// bloque de 1 a 12 tokens ya no son habla: se deja una y se para.
fn repite_al_final(t: &[i64]) -> Option<usize> {
    const VECES: usize = 4;
    for p in 1..=12 {
        if t.len() < VECES * p {
            break;
        }
        let ultimo = &t[t.len() - p..];
        if (2..=VECES).all(|k| &t[t.len() - k * p..t.len() - (k - 1) * p] == ultimo) {
            return Some(t.len() - (VECES - 1) * p);
        }
    }
    None
}

/// **Pasa una nota de voz a texto con Whisper**, de principio a fin.
pub fn transcribir(
    audio: &Path,
    raiz: &Path,
    junto_al_exe: Option<&Path>,
    idioma: &str,
    avance: &mut dyn FnMut(f32),
    cancelado: &AtomicBool,
) -> Result<Transcripcion, ErrorVoz> {
    // Primero el modelo: si falta, se dice en el acto y no tras decodificar.
    let w = Whisper::cargar(raiz, junto_al_exe)?;
    let muestras = pcm::decodificar(audio)?;
    if cancelado.load(Ordering::Relaxed) {
        return Err(ErrorVoz::Cancelada);
    }
    w.transcribir_pcm(&muestras, idioma, avance, cancelado)
}

/// **Pasa una nota a texto con uno o dos idiomas y, si los trae, por
/// turnos** (B6 y B10). Sin turnos es [`Whisper::transcribir_pcm_con`] de la
/// nota entera; con turnos, cada turno por su cuenta y con su nombre
/// delante (ver [`crate::turnos`]).
pub fn transcribir_con(
    audio: &Path,
    raiz: &Path,
    junto_al_exe: Option<&Path>,
    idiomas: &Idiomas,
    turnos: &[crate::turnos::Turno],
    avance: &mut dyn FnMut(f32),
    cancelado: &AtomicBool,
) -> Result<Transcripcion, ErrorVoz> {
    let w = Whisper::cargar(raiz, junto_al_exe)?;
    let muestras = pcm::decodificar(audio)?;
    if cancelado.load(Ordering::Relaxed) {
        return Err(ErrorVoz::Cancelada);
    }
    let rangos = crate::turnos::rangos(turnos, muestras.len(), HERCIOS);
    if rangos.is_empty() {
        return w.transcribir_pcm_con(&muestras, idiomas, avance, cancelado);
    }
    let mut oidos = Vec::with_capacity(rangos.len());
    let total = rangos.len() as f32;
    for (n, (i, r)) in rangos.iter().enumerate() {
        let desde = pcm::ms_de_muestra(r.start);
        let mut parte = |f: f32| avance((n as f32 + f) / total);
        let segmentos =
            match w.transcribir_pcm_con(&muestras[r.clone()], idiomas, &mut parte, cancelado) {
                Ok(t) => t
                    .segmentos
                    .into_iter()
                    .map(|s| Segmento::nuevo(desde + s.desde_ms, s.texto))
                    .collect(),
                // Un turno en el que no se entiende nada no tumba los demas.
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
        duracion_ms: pcm::duracion_ms(&muestras),
        segmentos: oidos.into_iter().flat_map(|o| o.segmentos).collect(),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn meta() -> Metadatos {
        Metadatos {
            n_text_layer: 6,
            n_text_ctx: 448,
            n_text_state: 512,
            sot: 50258,
            eot: 50257,
            no_timestamps: 50363,
            no_speech: 50362,
            blank: 220,
            transcribe: 50359,
            translate: Some(50358),
            codigos: "en,zh,de,es".into(),
            tokens_de_idioma: "50259,50260,50261,50262".into(),
        }
    }

    #[test]
    fn en_modo_cuidadoso_sin_arena_y_con_uno_o_dos_hilos() {
        assert_eq!(
            reparto(true, 8),
            Reparto {
                hilos: 2,
                arena: false
            }
        );
        assert_eq!(
            reparto(true, 1),
            Reparto {
                hilos: 1,
                arena: false
            }
        );
        assert_eq!(
            reparto(false, 4),
            Reparto {
                hilos: 4,
                arena: true
            }
        );
        assert_eq!(
            reparto(false, 32),
            Reparto {
                hilos: 8,
                arena: true
            }
        );
    }

    #[test]
    fn traducir_pone_la_tarea_translate_y_sin_ella_en_el_modelo_se_transcribe() {
        assert_eq!(
            meta().inicio_con("en", Tarea::Traducir).unwrap(),
            [50258, 50259, 50358, 50363]
        );
        let mut sin = meta();
        sin.translate = None;
        assert_eq!(
            sin.inicio_con("en", Tarea::Traducir).unwrap(),
            [50258, 50259, 50359, 50363]
        );
    }

    #[test]
    fn el_idioma_adivinado_es_el_token_de_idioma_de_logit_mas_alto() {
        let m = meta();
        let mut fila = vec![0f32; 51_865];
        fila[50_300] = 99.0; // no es de idioma: no cuenta
        fila[50_262] = 5.0; // es
        fila[50_259] = 7.0; // en
        assert_eq!(
            idioma_mas_probable(&fila, &m.idiomas()).as_deref(),
            Some("en")
        );
        // Una fila corta (logits raros) no revienta: solo cuenta lo que cabe.
        assert_eq!(idioma_mas_probable(&[0.0; 10], &m.idiomas()), None);
    }

    #[test]
    fn con_dos_idiomas_la_pausa_entre_frases_parte_la_tanda() {
        // Dos frases de 5 s con 1,5 s de silencio: con un idioma van en una
        // tanda; con dos, cada una en la suya.
        let tono = |s: f32| -> Vec<i16> {
            (0..(s * HERCIOS as f32) as usize)
                .map(|i| ((i as f32 * 0.3).sin() * 6_000.0) as i16)
                .collect()
        };
        let mut pcm = tono(5.0);
        pcm.extend(vec![0i16; (1.5 * HERCIOS as f32) as usize]);
        pcm.extend(tono(5.0));
        assert_eq!(tandas(&pcm).len(), 1);
        assert_eq!(tandas_con(&pcm, Some(PAUSA_ENTRE_IDIOMAS_MS)).len(), 2);
    }

    #[test]
    fn el_inicio_fuerza_el_idioma_la_tarea_y_sin_marcas_de_tiempo() {
        assert_eq!(meta().inicio("es").unwrap(), [50258, 50262, 50359, 50363]);
        assert_eq!(
            meta().inicio("en-US").unwrap(),
            [50258, 50259, 50359, 50363]
        );
        assert!(meta().inicio("xx").is_none());
    }

    #[test]
    fn whisper_sabe_los_idiomas_del_movil_y_no_otros() {
        assert!(sabe("es"));
        assert!(sabe("es-PE"));
        assert!(sabe("EN"));
        assert!(sabe("haw"));
        assert!(!sabe("xx"));
        assert!(!sabe(""));
    }

    #[test]
    fn solo_compiten_el_texto_y_el_fin_y_al_principio_ni_el_fin_ni_el_blanco() {
        let m = meta();
        let mut l = vec![0f32; 51_865];
        l[50_300] = 100.0; // un especial: no compite
        l[m.eot as usize] = 50.0;
        l[m.blank as usize] = 40.0;
        l[7] = 30.0;
        assert_eq!(elegir(&l, &m, false).0, m.eot);
        assert_eq!(elegir(&l, &m, true).0, 7);
        // El logaritmo de la probabilidad es negativo o cero.
        assert!(elegir(&l, &m, true).1 <= 0.0);
    }

    #[test]
    fn la_probabilidad_de_no_hablar_sale_del_softmax_de_la_fila() {
        let fila = [0.0f32, 0.0, 0.0, 0.0];
        assert!((probabilidad(&fila, 2) - 0.25).abs() < 1e-6);
        assert_eq!(probabilidad(&fila, 9), 0.0, "fuera de la fila");
        assert_eq!(probabilidad(&fila, -1), 0.0);
    }

    #[test]
    fn un_bucle_de_coletillas_se_corta_dejando_una() {
        // «y luego» = [5, 6], cuatro veces seguidas tras «hola» = [1].
        let t = [1, 5, 6, 5, 6, 5, 6, 5, 6];
        assert_eq!(repite_al_final(&t), Some(3));
        // La misma palabra cuatro veces.
        assert_eq!(repite_al_final(&[9, 9, 9, 9]), Some(1));
    }

    #[test]
    fn tres_repeticiones_o_una_frase_normal_no_son_un_bucle() {
        assert_eq!(repite_al_final(&[1, 5, 6, 5, 6, 5, 6]), None);
        assert_eq!(repite_al_final(&[1, 2, 3, 4, 5, 6, 7, 8]), None);
        assert_eq!(repite_al_final(&[]), None);
    }

    #[test]
    fn ninguna_tanda_pasa_de_treinta_segundos_ni_deja_audio_fuera_del_principio_al_fin() {
        // 25 s de ruido continuo: frases de 8 s, tandas de hasta 20.
        let pcm: Vec<i16> = (0..25 * HERCIOS as usize)
            .map(|i| ((i as f32 * 0.3).sin() * 6_000.0) as i16)
            .collect();
        let t = tandas(&pcm);
        assert!(t.len() >= 2, "{t:?}");
        assert!(t.iter().all(|r| r.len() <= mel::MUESTRAS_30S));
        assert_eq!(t[0].start, 0);
        assert_eq!(t.last().unwrap().end, pcm.len());
    }

    #[test]
    fn el_modelo_esta_listo_solo_con_los_tres_ficheros_y_ninguno_vacio() {
        let raiz = std::env::temp_dir().join(format!("pixpin-voz-whisper-{}", std::process::id()));
        let c = carpeta(&raiz);
        std::fs::create_dir_all(&c).unwrap();
        assert!(!modelo_listo(&raiz));
        for (n, _) in FICHEROS {
            std::fs::write(c.join(n), b"x").unwrap();
        }
        assert!(modelo_listo(&raiz));
        // Uno vacio (una descarga que fallo al abrir) no vale.
        std::fs::write(c.join(FICHEROS[2].0), b"").unwrap();
        assert!(!modelo_listo(&raiz));
        // Y uno a medias con `.parte` tampoco.
        std::fs::remove_file(c.join(FICHEROS[2].0)).unwrap();
        std::fs::write(c.join(format!("{}.parte", FICHEROS[2].0)), b"x").unwrap();
        assert!(!modelo_listo(&raiz));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn sin_modelo_se_dice_que_hay_que_bajarlo_y_donde() {
        let raiz = std::env::temp_dir().join("pixpin-voz-whisper-sin-modelo");
        match Whisper::cargar(&raiz, None).err() {
            Some(ErrorVoz::SinModeloWhisper { donde, megas }) => {
                assert!(donde.ends_with("base"), "{donde}");
                assert_eq!(megas, MEGAS);
            }
            // Un Windows sin ONNX Runtime: tambien es decir lo que falta.
            Some(ErrorVoz::SinOnnxRuntime { .. }) => {}
            otra => panic!("se esperaba que faltara el modelo, y salio {otra:?}"),
        }
    }

    /// Baja el modelo de verdad (unos 160 MB) a `PIXPIN_VOZ_RAIZ`.
    #[test]
    #[ignore = "baja 160 MB de Hugging Face"]
    fn baja_el_modelo_de_verdad() {
        let raiz = std::env::var("PIXPIN_VOZ_RAIZ").expect("PIXPIN_VOZ_RAIZ");
        let t0 = std::time::Instant::now();
        let mut ultimo = 0f32;
        let mut marcha_atras = false;
        bajar_modelo(
            Path::new(&raiz),
            &mut |f| {
                marcha_atras |= f < ultimo;
                ultimo = f;
            },
            &AtomicBool::new(false),
        )
        .expect("baja el modelo");
        eprintln!("bajado en {:.1} s", t0.elapsed().as_secs_f32());
        assert!(modelo_listo(Path::new(&raiz)));
        assert_eq!(ultimo, 1.0);
        assert!(!marcha_atras, "la barra no retrocede");
    }

    /// Dos idiomas de verdad (B6): `PIXPIN_VOZ_RAIZ`, `PIXPIN_VOZ_AUDIO`,
    /// `PIXPIN_VOZ_SEGUNDO` (p. ej. `en`) y `PIXPIN_VOZ_MODO`
    /// (`cada_uno`/`todo_en_uno`). Imprime el texto y el tiempo.
    #[test]
    #[ignore = "pide el modelo Whisper bajado y un audio de verdad"]
    fn whisper_dos_idiomas_de_verdad() {
        let raiz = std::env::var("PIXPIN_VOZ_RAIZ").expect("PIXPIN_VOZ_RAIZ");
        let audio = std::env::var("PIXPIN_VOZ_AUDIO").expect("PIXPIN_VOZ_AUDIO");
        let segundo = std::env::var("PIXPIN_VOZ_SEGUNDO").unwrap_or_default();
        let modo = crate::dos_idiomas::ModoDeIdiomas::de(
            &std::env::var("PIXPIN_VOZ_MODO").unwrap_or_default(),
        );
        // SAFETY: COM por hilo para Media Foundation, como en la app.
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            )
        };
        let w = Whisper::cargar(Path::new(&raiz), None).expect("carga el modelo");
        let muestras = pcm::decodificar(Path::new(&audio)).expect("decodifica");
        let t0 = std::time::Instant::now();
        let t = w
            .transcribir_pcm_con(
                &muestras,
                &Idiomas::con("es", &segundo, modo),
                &mut |_| {},
                &AtomicBool::new(false),
            )
            .expect("transcribe");
        eprintln!(
            "audio {:.1} s | transcribir {:.2} s | modo {modo:?} segundo «{segundo}»\n{}",
            muestras.len() as f32 / HERCIOS as f32,
            t0.elapsed().as_secs_f32(),
            t.texto
        );
        assert!(!t.texto.is_empty());
    }

    /// Necesita el modelo bajado y un audio de verdad:
    /// `PIXPIN_VOZ_RAIZ` (la carpeta con `whisper/base/`) y
    /// `PIXPIN_VOZ_AUDIO` (un `.m4a`). Se corre a mano con
    /// `cargo test -p pixpin-voz --release -- --ignored whisper_transcribe`.
    #[test]
    #[ignore = "pide el modelo Whisper bajado y un .m4a de verdad"]
    fn whisper_transcribe_una_nota_real() {
        let raiz = std::env::var("PIXPIN_VOZ_RAIZ").expect("PIXPIN_VOZ_RAIZ");
        let audio = std::env::var("PIXPIN_VOZ_AUDIO").expect("PIXPIN_VOZ_AUDIO");
        let idioma = std::env::var("PIXPIN_VOZ_IDIOMA").unwrap_or_else(|_| "es".into());
        // SAFETY: COM por hilo para Media Foundation, como hace la app en su
        // hilo de transcribir; el hilo de la prueba muere al acabar.
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            )
        };
        let t0 = std::time::Instant::now();
        let w = Whisper::cargar(Path::new(&raiz), None).expect("carga el modelo");
        let carga = t0.elapsed();
        let muestras = pcm::decodificar(Path::new(&audio)).expect("decodifica");
        let t1 = std::time::Instant::now();
        let t = w
            .transcribir_pcm(&muestras, &idioma, &mut |_| {}, &AtomicBool::new(false))
            .expect("transcribe");
        eprintln!(
            "audio {:.1} s | carga {:.2} s | transcribir {:.2} s | runtime {}\n{}",
            muestras.len() as f32 / HERCIOS as f32,
            carga.as_secs_f32(),
            t1.elapsed().as_secs_f32(),
            w.rt.version,
            t.texto
        );
        assert!(!t.texto.is_empty());
    }
}
