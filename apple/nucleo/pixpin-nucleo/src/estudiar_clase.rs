//! **Grabar clase en dos modos y la ventana «Estudiar la clase»** (Windows
//! 3af2647 y ce6c058, 25-oct-2026): lo que no pinta nada, para Swift.
//!
//! - **El modo de grabar** ([`ModoDeGrabar`]): «Pantalla y sonido» (el video
//!   de la pantalla con su sonido) o «Audio y capturas» (solo el sonido, y
//!   «Captura» fotografia la pantalla entera). Se recuerda en este aparato,
//!   en `voz/modo-de-grabar.txt` (no viaja: es como se graba aqui), con la
//!   palabra de Windows. Lo que se guarda en el chat es lo mismo en los dos
//!   modos ([`crate::clase`]).
//! - **De que clase es un mensaje** ([`Nucleo::de_una_clase`]): la nota
//!   (`grabacion: "clase"`) al principio; una captura o el video
//!   (`enGrabacion`), en su minuto. Es lo que pone «Estudiar la clase» en su
//!   menu (`estudiar_clase::de_una_clase` de Windows).
//! - **La clase leida** ([`Nucleo::clase_para_estudiar`]): el audio, el
//!   video, las capturas que estan en este Mac, el texto en trozos con su
//!   minuto, las senales (los marcadores con nota, `resto["marcadores"]`, y
//!   las banderitas del movil, `resto["marcas"]`) y el texto con las senales
//!   metidas en su minuto, con el minuto de cada fila para seguir el scroll.
//! - **Los marcadores**: poner uno y quitar una senal, en el cuaderno (se
//!   relee: el chat o el movil pudieron tocar la nota), con el formato de
//!   `pixpin_proyecto::clase` (el mismo crate que Windows).
//! - **Las cuentas de la ventana** (`clase_*`): que toca en cada minuto, el
//!   scroll que se sigue, la barra del tiempo y los saltos, por el puente.
//! - **Acercar lo que se ve como en un lienzo** (Windows 8d7fe93 y fe0c9c4,
//!   `encuadre_*`): `pixpin_ui::encuadre`, el mismo crate que Windows.
//! - **Anotar el fotograma del video** ([`Nucleo::guardar_fotograma_de_clase`]):
//!   se guarda como una captura mas de la clase, en su minuto
//!   (`clase::poner_captura`), y se abre en su lienzo como cualquier foto.
//! - **La carpeta de una grabacion** tambien se estudia
//!   ([`clase_que_estudia`]): su nota, desde el principio.

use pixpin_proyecto::almacen;
use pixpin_proyecto::carpeta_de_paginas::Carpeta;
use pixpin_proyecto::clase::{self as puro, Fila, Marcador};
use pixpin_proyecto::cuaderno::{self, Mensaje};
use pixpin_ui::encuadre::{self, Encuadre};

use crate::chats::validar_chat;
use crate::clase::{CAMPO_EN_GRABACION, CAMPO_GRABACION, CAMPO_VIDEO, capturas_de};
use crate::{ErrorNucleo, Nucleo};

/// Lo que saltan ←/→ (`SALTO_CORTO` de Windows).
pub const SALTO_CORTO_MS: i64 = 5_000;
/// Lo que saltan J/L y los botones de los lados (`SALTO_LARGO`).
pub const SALTO_LARGO_MS: i64 = 10_000;
/// Lo que se deja de seguir al sonido tras mover una lista a mano
/// (`A_MANO`): para poder leer.
pub const A_MANO_MS: i64 = 5_000;
/// Un clic en la barra a menos de esto (puntos) de una senal se queda en
/// ella (`PEGAR`).
pub const PEGAR_PUNTOS: f32 = 6.0;
/// El campo de las banderitas del movil (`voz::MARCAS` de Windows).
pub const MARCAS: &str = "marcas";
/// El fichero donde se recuerda el modo, dentro de la carpeta de datos.
const FICHERO_DEL_MODO: &str = "voz/modo-de-grabar.txt";

// --- El modo de grabar ---------------------------------------------------------

/// **Que se graba**: el selector de arriba de Preparar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ModoDeGrabar {
    /// El video de la pantalla con su sonido.
    Pantalla,
    /// Solo el sonido, con capturas de la pantalla entera.
    Capturas,
}

impl ModoDeGrabar {
    /// La palabra con que se recuerda (la de Windows).
    pub fn palabra(self) -> &'static str {
        match self {
            ModoDeGrabar::Pantalla => "pantalla",
            ModoDeGrabar::Capturas => "capturas",
        }
    }

    /// Lo recordado. Lo que no se entiende (o nada) es el modo de capturas:
    /// el que menos ocupa, que era el de siempre.
    pub fn de(palabra: &str) -> ModoDeGrabar {
        match palabra.trim() {
            "pantalla" => ModoDeGrabar::Pantalla,
            _ => ModoDeGrabar::Capturas,
        }
    }

    /// El otro (las flechas y el tabulador de Preparar).
    pub fn otro(self) -> ModoDeGrabar {
        match self {
            ModoDeGrabar::Pantalla => ModoDeGrabar::Capturas,
            ModoDeGrabar::Capturas => ModoDeGrabar::Pantalla,
        }
    }
}

/// El otro modo, para Swift.
#[uniffi::export]
pub fn otro_modo_de_grabar(modo: ModoDeGrabar) -> ModoDeGrabar {
    modo.otro()
}

#[uniffi::export]
impl Nucleo {
    /// **El ultimo modo con que se grabo una clase en este aparato** (sin
    /// nada recordado, el de capturas).
    pub fn modo_de_grabar(&self) -> ModoDeGrabar {
        std::fs::read_to_string(self.ruta().join(FICHERO_DEL_MODO))
            .map(|t| ModoDeGrabar::de(&t))
            .unwrap_or(ModoDeGrabar::Capturas)
    }

    /// Recuerda `modo` para la proxima vez. Si no se puede, se sigue: es
    /// una comodidad.
    pub fn recordar_modo_de_grabar(&self, modo: ModoDeGrabar) {
        let f = self.ruta().join(FICHERO_DEL_MODO);
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(&f, modo.palabra());
    }
}

// --- De que clase es un mensaje ------------------------------------------------

/// **Si un mensaje es de una clase grabada**: `(id de su nota de voz,
/// minuto)`. La nota misma va al principio; una captura o el video de la
/// clase, a su minuto.
pub fn de_una_clase(m: &Mensaje) -> Option<(String, i64)> {
    if m.resto.get(CAMPO_GRABACION).and_then(|v| v.as_str()) == Some("clase") {
        return Some((m.id.clone(), 0));
    }
    let en = m.resto.get(CAMPO_EN_GRABACION)?;
    let de = en.get("de")?.as_str()?.to_string();
    Some((de, en.get("ms").and_then(|v| v.as_i64()).unwrap_or(0)))
}

/// **La clase que estudia un mensaje** (`clase_que_estudia` de Windows):
/// la nota, una captura o el video de una clase ([`de_una_clase`]), o la
/// carpeta de una grabacion (su nota, desde el principio; sin nota, nada).
pub fn clase_que_estudia(m: &Mensaje) -> Option<(String, i64)> {
    de_una_clase(m).or_else(|| {
        Carpeta::de_mensaje(m)
            .filter(|c| c.es_grabacion() && !c.voz.is_empty())
            .map(|c| (c.voz, 0))
    })
}

/// La clase a la que pertenece un mensaje: su nota de voz y donde empezar.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct EnUnaClase {
    /// El id de la nota de voz de la clase.
    pub voz: String,
    /// El minuto donde abrirla (el de la captura; 0 la nota y el video).
    pub ms: i64,
}

// --- La clase leida -------------------------------------------------------------

/// Un trozo del texto de la clase: su minuto (si lo lleva) y lo que dice.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LineaDeClase {
    pub ms: Option<i64>,
    pub texto: String,
}

/// Una senal de la barra y de la lista: un marcador (con su nota) o una
/// banderita del movil (sin ella).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SenalDeClase {
    pub ms: i64,
    pub texto: String,
    pub banderita: bool,
}

/// Una fila de la columna del texto: una linea (`linea`) o una senal
/// (`senal`), por su sitio en su lista, con el minuto con que se sigue el
/// scroll (`tiempos_de_filas`: el suyo o el de la anterior, nunca baja).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FilaDeClase {
    pub linea: Option<u32>,
    pub senal: Option<u32>,
    pub ms: i64,
}

/// Una captura de la clase que esta en este Mac.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CapturaParaEstudiar {
    pub ms: i64,
    pub id: String,
    pub ruta: String,
    /// El id de su lienzo propio (`lienzo_de_la_foto::id_del_lienzo`: su
    /// `referencia` o `foto-<id>`), para ensenar encima lo anotado.
    pub lienzo: String,
}

/// **Todo lo de una clase**, leido del cuaderno (la `Clase` de Windows).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ClaseParaEstudiar {
    pub chat: String,
    /// El id de la nota de voz (aunque se abriera desde una captura).
    pub id: String,
    pub nombre: String,
    pub duracion_ms: i64,
    /// La nota de voz y el video en este Mac, si estan.
    pub audio: Option<String>,
    pub video: Option<String>,
    /// En orden; las que ya no estan en el disco, fuera.
    pub capturas: Vec<CapturaParaEstudiar>,
    pub lineas: Vec<LineaDeClase>,
    pub senales: Vec<SenalDeClase>,
    /// El texto con las senales en su minuto.
    pub filas: Vec<FilaDeClase>,
}

/// El milisegundo de un `[1:23] texto` y el texto sin la marca
/// (`pixpin_voz::tiempo_de` de Windows): `m:ss` o `h:mm:ss`, con dos cifras
/// exactas en los minutos y segundos de detras.
fn tiempo_de(linea: &str) -> Option<(i64, &str)> {
    let resto = linea.trim_start().strip_prefix('[')?;
    let (dentro, despues) = resto.split_once(']')?;
    let numero = |t: &str| -> Option<i64> {
        if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        t.parse().ok()
    };
    let dos = |t: &str| if t.len() == 2 { numero(t) } else { None };
    let mut partes = dentro.split(':');
    let a = partes.next()?;
    let b = partes.next()?;
    let c = partes.next();
    if partes.next().is_some() {
        return None;
    }
    let (h, m, s) = match c {
        Some(c) => (numero(a)?, dos(b)?, dos(c)?),
        None => (0, numero(a)?, dos(b)?),
    };
    if m > 59 || s > 59 {
        return None;
    }
    Some((
        (h * 3600 + m * 60 + s) * 1000,
        despues.strip_prefix(' ').unwrap_or(despues),
    ))
}

/// Lo que se ensena de un trozo: sin las marcas de Markdown que no aportan
/// leyendo y sin las imagenes (`voz::sin_marcas` de Windows).
fn sin_marcas(texto: &str) -> String {
    texto
        .lines()
        .filter(|l| !l.trim_start().starts_with("!["))
        .map(|l| {
            l.trim_start_matches(['#', ' '])
                .replace("**", "")
                .replace("__", "")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// **Los trozos del texto de una nota** (`trozos_de_la_letra` de Windows):
/// la transcripcion partida por renglones en blanco, cada trozo con su
/// minuto fuera del texto y sin marcas; los vacios, fuera.
pub fn trozos_de_la_letra(transcripcion: &str) -> Vec<(Option<i64>, String)> {
    let texto = transcripcion.trim().replace("\r\n", "\n");
    let mut v = Vec::new();
    let mut bloque = String::new();
    let cerrar = |bloque: &mut String, v: &mut Vec<(Option<i64>, String)>| {
        let limpio = bloque.trim();
        if !limpio.is_empty() {
            let (ms, t) = match tiempo_de(limpio) {
                Some((ms, resto)) => (Some(ms), resto.trim()),
                None => (None, limpio),
            };
            let t = sin_marcas(t);
            if !t.is_empty() {
                v.push((ms, t));
            }
        }
        bloque.clear();
    };
    for linea in texto.split('\n') {
        if linea.trim().is_empty() {
            cerrar(&mut bloque, &mut v);
        } else {
            if !bloque.is_empty() {
                bloque.push('\n');
            }
            bloque.push_str(linea);
        }
    }
    cerrar(&mut bloque, &mut v);
    v
}

/// Las banderitas del movil de una nota, en orden y sin repetir.
pub fn banderitas_de(m: &Mensaje) -> Vec<i64> {
    let mut v: Vec<i64> = m
        .resto
        .get(MARCAS)
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
        .unwrap_or_default();
    v.sort_unstable();
    v.dedup();
    v
}

/// Quita la banderita de `ms` (`voz::quitar_marca`). Vacias, la clave se
/// quita. Devuelve si habia alguna.
pub fn quitar_banderita(m: &mut Mensaje, ms: i64) -> bool {
    let mut v = banderitas_de(m);
    let antes = v.len();
    v.retain(|x| *x != ms);
    if v.len() == antes {
        return false;
    }
    if v.is_empty() {
        m.resto.remove(MARCAS);
    } else {
        m.resto.insert(
            MARCAS.into(),
            serde_json::Value::Array(v.into_iter().map(serde_json::Value::from).collect()),
        );
    }
    true
}

/// **Los marcadores y las banderitas de una nota**, en orden: una banderita
/// en el mismo milisegundo que un marcador no se repite (`senales_de`).
pub fn senales_de(m: &Mensaje) -> Vec<SenalDeClase> {
    let mut v: Vec<SenalDeClase> = puro::marcadores_de(&m.resto)
        .into_iter()
        .map(|Marcador { ms, texto }| SenalDeClase {
            ms,
            texto,
            banderita: false,
        })
        .collect();
    for ms in banderitas_de(m) {
        if !v.iter().any(|s| s.ms == ms) {
            v.push(SenalDeClase {
                ms,
                texto: String::new(),
                banderita: true,
            });
        }
    }
    v.sort_by_key(|s| s.ms);
    v
}

/// El texto con las senales en su minuto, y el minuto de cada fila.
pub fn filas_de(lineas: &[LineaDeClase], senales: &[SenalDeClase]) -> Vec<FilaDeClase> {
    let l: Vec<Option<i64>> = lineas.iter().map(|x| x.ms).collect();
    let s: Vec<i64> = senales.iter().map(|x| x.ms).collect();
    let filas = puro::mezclar(&l, &s);
    let tiempos = puro::tiempos_de_filas(&filas, &l, &s);
    filas
        .into_iter()
        .zip(tiempos)
        .map(|(f, ms)| match f {
            Fila::Linea(i) => FilaDeClase {
                linea: Some(i as u32),
                senal: None,
                ms,
            },
            Fila::Marcador(j) => FilaDeClase {
                linea: None,
                senal: Some(j as u32),
                ms,
            },
        })
        .collect()
}

impl Nucleo {
    /// Lee la clase de `mensaje` (la nota, o una captura o el video: se
    /// busca su nota) en `chat`.
    fn leer_clase(&self, chat: &str, mensaje: &str) -> Result<ClaseParaEstudiar, ErrorNucleo> {
        let carpeta = almacen::carpeta(self.ruta(), chat);
        let mensajes = cuaderno::Cuaderno::leer_de(&carpeta)?.mensajes;
        let no_esta = || ErrorNucleo::NoExiste {
            que: "la clase ya no esta en el chat".into(),
        };
        let m = mensajes
            .iter()
            .find(|m| m.id == mensaje)
            .ok_or_else(no_esta)?;
        let voz = match clase_que_estudia(m) {
            Some((de, _)) if de != m.id => {
                mensajes.iter().find(|v| v.id == de).ok_or_else(no_esta)?
            }
            _ => m,
        };
        let ruta_de = |m: &Mensaje| {
            m.ruta
                .as_deref()
                .and_then(|r| self.resolver(chat, r))
                .filter(|r| r.is_file())
                .map(|r| r.display().to_string())
        };
        let ruta_del_id = |id: &str| mensajes.iter().find(|c| c.id == id).and_then(ruta_de);
        let capturas = capturas_de(voz)
            .into_iter()
            .filter_map(|(ms, id)| {
                let m = mensajes.iter().find(|c| c.id == id)?;
                Some(CapturaParaEstudiar {
                    ms,
                    ruta: ruta_de(m)?,
                    lienzo: pixpin_proyecto::lienzo_de_la_foto::id_del_lienzo(m),
                    id,
                })
            })
            .collect();
        let video = voz
            .resto
            .get(CAMPO_VIDEO)
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .and_then(ruta_del_id);
        let lineas: Vec<LineaDeClase> =
            trozos_de_la_letra(voz.transcripcion.as_deref().unwrap_or(""))
                .into_iter()
                .map(|(ms, texto)| LineaDeClase { ms, texto })
                .collect();
        let senales = senales_de(voz);
        let filas = filas_de(&lineas, &senales);
        Ok(ClaseParaEstudiar {
            chat: chat.to_string(),
            id: voz.id.clone(),
            nombre: if voz.nombre.trim().is_empty() {
                voz.texto.clone()
            } else {
                voz.nombre.trim_end_matches(".m4a").to_string()
            },
            duracion_ms: voz.duracion_ms.max(0),
            audio: ruta_de(voz),
            video,
            capturas,
            lineas,
            senales,
            filas,
        })
    }

    /// Cambia la nota de la clase en el cuaderno (relee: el chat o el movil
    /// pudieron tocarla) y la devuelve leida otra vez.
    fn cambiar_la_nota(
        &self,
        chat: &str,
        voz: &str,
        cambio: impl FnOnce(&mut Mensaje) -> bool,
    ) -> Result<ClaseParaEstudiar, ErrorNucleo> {
        let carpeta = almacen::carpeta(self.ruta(), chat);
        let fichero = carpeta.join("guardados.jsonl");
        let mut cache = self.cache_tomada();
        let hecho = cuaderno::cambiar(&carpeta, voz, cambio);
        cache.quitar(&fichero);
        drop(cache);
        if hecho?.is_none() {
            return Err(ErrorNucleo::NoExiste {
                que: "la nota de la clase ya no esta".into(),
            });
        }
        self.leer_clase(chat, voz)
    }
}

#[uniffi::export]
impl Nucleo {
    /// **De que clase es `mensaje`** de `chat`, si lo es: su nota de voz y
    /// el minuto donde abrirla. Bloqueante (lee el cuaderno): Swift lo
    /// guarda por mensaje.
    pub fn de_una_clase(
        &self,
        chat: String,
        mensaje: String,
    ) -> Result<Option<EnUnaClase>, ErrorNucleo> {
        validar_chat(&chat)?;
        let carpeta = almacen::carpeta(self.ruta(), &chat);
        let c = match cuaderno::Cuaderno::leer_de(&carpeta) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        Ok(c.mensajes
            .iter()
            .find(|m| m.id == mensaje)
            .and_then(clase_que_estudia)
            .map(|(voz, ms)| EnUnaClase { voz, ms }))
    }

    /// **Cuando cambio por ultima vez el cuaderno de `chat`** (ms desde
    /// 1970; 0 si no hay): la ventana de estudiar lo mira cada poco y, si
    /// cambio (llego el texto, un marcador del movil), relee la clase. Solo
    /// mira la fecha del fichero.
    pub fn cambio_del_cuaderno_ms(&self, chat: String) -> i64 {
        if validar_chat(&chat).is_err() {
            return 0;
        }
        std::fs::metadata(almacen::carpeta(self.ruta(), &chat).join("guardados.jsonl"))
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_millis() as i64)
    }

    /// **La clase de `mensaje` para estudiarla** (la nota, una captura o el
    /// video). Bloqueante.
    pub fn clase_para_estudiar(
        &self,
        chat: String,
        mensaje: String,
    ) -> Result<ClaseParaEstudiar, ErrorNucleo> {
        validar_chat(&chat)?;
        self.leer_clase(&chat, &mensaje)
    }

    /// **Pone un marcador** en `ms` de la nota `voz` con `texto` (recortado
    /// y en una linea; en el mismo milisegundo, cambia la nota del que
    /// habia). Devuelve la clase como quedo.
    pub fn poner_marcador_de_clase(
        &self,
        chat: String,
        voz: String,
        ms: i64,
        texto: String,
    ) -> Result<ClaseParaEstudiar, ErrorNucleo> {
        validar_chat(&chat)?;
        self.cambiar_la_nota(&chat, &voz, |m| {
            puro::poner_marcador(&mut m.resto, ms, &texto);
            true
        })
    }

    /// **Guarda el fotograma que se ve** (el PNG de `ruta_png`, escrito por
    /// Swift) como una captura mas de la clase `voz` en `ms`
    /// (`estudiar_clase::guardar_fotograma` de Windows): una imagen en el
    /// chat `clase-<ahora>-<ms>.png` con el pie `⏱ 1:05 · Clase …`,
    /// enlazada a la nota (`enGrabacion`) y en la lista de capturas de la
    /// nota. Devuelve la imagen (para abrirla en su lienzo) y la clase como
    /// quedo. Bloqueante.
    pub fn guardar_fotograma_de_clase(
        &self,
        chat: String,
        voz: String,
        ms: i64,
        ruta_png: String,
    ) -> Result<FotogramaDeClase, ErrorNucleo> {
        validar_chat(&chat)?;
        let carpeta = almacen::carpeta(self.ruta(), &chat);
        let nota = cuaderno::Cuaderno::leer_de(&carpeta)?
            .mensajes
            .into_iter()
            .find(|m| m.id == voz)
            .ok_or_else(|| ErrorNucleo::NoExiste {
                que: "la nota de la clase ya no esta".into(),
            })?;
        let ms = ms.max(0);
        let fichero = format!("clase-{}-{ms}.png", crate::ahora_ms());
        let pie = format!(
            "⏱ {} · {}",
            crate::voz::marca_de_tiempo(ms),
            nombre_de_la_nota(&nota)
        );
        let de = voz.clone();
        let imagen = self.adjuntar_con(chat.clone(), ruta_png, Some(fichero), |m| {
            m.texto = pie;
            m.resto.insert(
                crate::clase::CAMPO_EN_GRABACION.into(),
                serde_json::json!({ "de": de, "ms": ms }),
            );
        })?;
        let id = imagen.id.clone();
        let clase = self.cambiar_la_nota(&chat, &voz, |n| {
            puro::poner_captura(&mut n.resto, ms, &id);
            true
        })?;
        Ok(FotogramaDeClase { imagen, clase })
    }

    /// **Quita la senal de `ms`**: la banderita del movil si lo es, o el
    /// marcador, cada una de su campo. Devuelve la clase como quedo.
    pub fn quitar_senal_de_clase(
        &self,
        chat: String,
        voz: String,
        ms: i64,
    ) -> Result<ClaseParaEstudiar, ErrorNucleo> {
        validar_chat(&chat)?;
        self.cambiar_la_nota(&chat, &voz, |m| {
            let banderita = senales_de(m).iter().any(|s| s.ms == ms && s.banderita);
            if banderita {
                quitar_banderita(m, ms)
            } else {
                puro::quitar_marcador(&mut m.resto, ms)
            }
        })
    }
}

/// Lo que deja guardar un fotograma para anotarlo.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FotogramaDeClase {
    /// La imagen nueva del chat (la captura).
    pub imagen: crate::MensajeChat,
    /// La clase como quedo, con la captura en su minuto.
    pub clase: ClaseParaEstudiar,
}

/// El nombre de una nota para el pie de sus capturas (el de
/// `ClaseParaEstudiar::nombre`).
fn nombre_de_la_nota(voz: &Mensaje) -> String {
    if voz.nombre.trim().is_empty() {
        voz.texto.clone()
    } else {
        voz.nombre.trim_end_matches(".m4a").to_string()
    }
}

// --- Acercar lo que se ve (`pixpin_ui::encuadre`) --------------------------------

/// **Como se ve la imagen en su caja**: el aumento (1 = encajada) y lo
/// movida que esta desde el centro, en puntos de la vista.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct EncuadreDeClase {
    pub aumento: f32,
    pub dx: f32,
    pub dy: f32,
}

/// Un rectangulo en puntos de la vista (`encuadre::Caja`).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct CajaDeClase {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

impl From<EncuadreDeClase> for Encuadre {
    fn from(e: EncuadreDeClase) -> Encuadre {
        Encuadre {
            aumento: e.aumento,
            dx: e.dx,
            dy: e.dy,
        }
    }
}

impl From<Encuadre> for EncuadreDeClase {
    fn from(e: Encuadre) -> EncuadreDeClase {
        EncuadreDeClase {
            aumento: e.aumento,
            dx: e.dx,
            dy: e.dy,
        }
    }
}

fn caja(c: CajaDeClase) -> encuadre::Caja {
    (c.x, c.y, c.ancho, c.alto)
}

/// Encajada y en el centro, como al abrir.
#[uniffi::export]
pub fn encuadre_encajado() -> EncuadreDeClase {
    Encuadre::default().into()
}

/// Si esta como al abrir (⎋ primero encaja; si ya lo esta, cierra).
#[uniffi::export]
pub fn encuadre_esta_encajado(e: EncuadreDeClase) -> bool {
    Encuadre::from(e).encajado()
}

/// Lo que acerca un paso de rueda o un boton (`encuadre::PASO`).
#[uniffi::export]
pub fn encuadre_paso() -> f32 {
    encuadre::PASO
}

/// **Donde se pinta la imagen** `w x h` en `caja`.
#[uniffi::export]
pub fn encuadre_destino(e: EncuadreDeClase, caja_vista: CajaDeClase, w: f32, h: f32) -> CajaDeClase {
    let (x, y, ancho, alto) = Encuadre::from(e).destino(caja(caja_vista), w, h);
    CajaDeClase { x, y, ancho, alto }
}

/// El tanto por ciento de la pastilla: 100 es un pixel de la imagen por
/// punto de la vista.
#[uniffi::export]
pub fn encuadre_por_ciento(e: EncuadreDeClase, caja_vista: CajaDeClase, w: f32, h: f32) -> u32 {
    Encuadre::from(e).por_ciento(caja(caja_vista), w, h)
}

/// **Acerca (`factor` > 1) o aleja hacia el punto** (`px`, `py`): lo que
/// hay bajo el raton se queda bajo el raton.
#[uniffi::export]
pub fn encuadre_acercar(
    e: EncuadreDeClase,
    caja_vista: CajaDeClase,
    w: f32,
    h: f32,
    factor: f32,
    px: f32,
    py: f32,
) -> EncuadreDeClase {
    let mut n = Encuadre::from(e);
    n.acercar(caja(caja_vista), w, h, factor, (px, py));
    n.into()
}

/// La lleva con el raton (arrastrar), sin perderla de vista.
#[uniffi::export]
pub fn encuadre_mover(
    e: EncuadreDeClase,
    caja_vista: CajaDeClase,
    w: f32,
    h: f32,
    dx: f32,
    dy: f32,
) -> EncuadreDeClase {
    let mut n = Encuadre::from(e);
    n.mover(caja(caja_vista), w, h, dx, dy);
    n.into()
}

// --- Las cuentas de la ventana, para Swift --------------------------------------

/// Lo que se ensena como posicion mientras se arrastra la barra
/// (`clase::posicion_a_la_vista`): donde esta el raton, no donde dice el
/// video (que cae en el fotograma clave mas cercano).
#[uniffi::export]
pub fn clase_posicion_a_la_vista(medio_ms: i64, arrastrando: Option<i64>) -> i64 {
    puro::posicion_a_la_vista(medio_ms, arrastrando)
}

/// La linea del texto que suena en `ms` (la ultima con minuto que ya paso).
#[uniffi::export]
pub fn clase_linea_en(lineas: Vec<Option<i64>>, ms: i64) -> Option<u32> {
    puro::linea_en(&lineas, ms).map(|i| i as u32)
}

/// El ultimo de `tiempos` que ya paso en `ms` (antes del primero, el
/// primero): la captura del minuto, la senal por la que va.
#[uniffi::export]
pub fn clase_ultimo_antes(tiempos: Vec<i64>, ms: i64) -> Option<u32> {
    puro::ultimo_antes(&tiempos, ms).map(|i| i as u32)
}

/// El scroll de una lista para que `ms` quede en su ancla (a un tercio).
#[uniffi::export]
pub fn clase_scroll_para(
    tiempos: Vec<i64>,
    altos: Vec<f32>,
    alto_vista: f32,
    ms: i64,
    suave: bool,
) -> f32 {
    puro::scroll_para(&tiempos, &altos, alto_vista, ms, suave)
}

/// El milisegundo que se esta leyendo en una lista con `scroll`.
#[uniffi::export]
pub fn clase_ms_leido(
    tiempos: Vec<i64>,
    altos: Vec<f32>,
    alto_vista: f32,
    scroll: f32,
) -> Option<i64> {
    puro::ms_leido(&tiempos, &altos, alto_vista, scroll)
}

/// **El scroll de la otra lista** cuando el usuario mueve una.
#[allow(clippy::too_many_arguments)]
#[uniffi::export]
pub fn clase_seguir(
    de_tiempos: Vec<i64>,
    de_altos: Vec<f32>,
    de_vista: f32,
    de_scroll: f32,
    a_tiempos: Vec<i64>,
    a_altos: Vec<f32>,
    a_vista: f32,
) -> Option<f32> {
    puro::seguir(
        &de_tiempos,
        &de_altos,
        de_vista,
        de_scroll,
        &a_tiempos,
        &a_altos,
        a_vista,
    )
}

/// Lo mas abajo que se puede bajar una lista.
#[uniffi::export]
pub fn clase_tope(altos: Vec<f32>, alto_vista: f32) -> f32 {
    puro::tope(&altos, alto_vista)
}

/// Acerca `actual` a `destino` un trozo cada fotograma.
#[uniffi::export]
pub fn clase_acercar(actual: f32, destino: f32, k: f32) -> f32 {
    puro::acercar(actual, destino, k)
}

/// La x de un milisegundo en la barra del tiempo.
#[uniffi::export]
pub fn clase_x_en_la_barra(ms: i64, ancho: f32, duracion_ms: i64) -> f32 {
    puro::x_en_la_barra(ms, ancho, duracion_ms)
}

/// Un clic en la barra en `x`: el milisegundo, o el de la senal si cae a
/// menos de `tolerancia`.
#[uniffi::export]
pub fn clase_pegar_a_senal(
    x: f32,
    ancho: f32,
    duracion_ms: i64,
    senales: Vec<i64>,
    tolerancia: f32,
) -> i64 {
    puro::pegar_a_senal(x, ancho, duracion_ms, &senales, tolerancia)
}

/// Salta `delta` desde `ms` sin salirse de la nota.
#[uniffi::export]
pub fn clase_saltar(ms: i64, delta: i64, duracion_ms: i64) -> i64 {
    puro::saltar(ms, delta, duracion_ms)
}

/// **El trozo anterior o el siguiente** al que suena en `ms` (↑/↓,
/// `Vista::otro_trozo` de Windows): adelante, el primero que empieza mas de
/// 200 ms despues; atras, el anterior si se acaba de empezar este (menos de
/// 1,5 s), o el principio de este.
#[uniffi::export]
pub fn clase_otro_trozo(tiempos: Vec<i64>, ms: i64, adelante: bool) -> Option<i64> {
    if adelante {
        return tiempos.iter().find(|t| **t > ms + 200).copied();
    }
    match tiempos.iter().rposition(|t| *t <= ms) {
        Some(i) if ms - tiempos[i] < 1_500 && i > 0 => Some(tiempos[i - 1]),
        Some(i) => Some(tiempos[i]),
        None => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn nucleo(nombre: &str) -> (std::path::PathBuf, std::sync::Arc<Nucleo>) {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-nucleo-estudiar-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let n = Nucleo::abrir(dir.display().to_string(), "Mac".into()).unwrap();
        (dir, n)
    }

    #[test]
    fn el_modo_se_recuerda_por_su_palabra() {
        for m in [ModoDeGrabar::Pantalla, ModoDeGrabar::Capturas] {
            assert_eq!(ModoDeGrabar::de(m.palabra()), m);
            assert_eq!(m.otro().otro(), m);
            assert_ne!(otro_modo_de_grabar(m), m);
        }
        assert_eq!(
            ModoDeGrabar::de("pantalla\n"),
            ModoDeGrabar::Pantalla,
            "con su salto de linea"
        );
        // Caso negativo: lo que no se entiende es el modo que menos ocupa.
        assert_eq!(ModoDeGrabar::de(""), ModoDeGrabar::Capturas);
        assert_eq!(ModoDeGrabar::de("video"), ModoDeGrabar::Capturas);
    }

    #[test]
    fn el_modo_queda_en_su_fichero_de_windows() {
        let (dir, n) = nucleo("modo");
        assert_eq!(
            n.modo_de_grabar(),
            ModoDeGrabar::Capturas,
            "sin nada, el de capturas"
        );
        n.recordar_modo_de_grabar(ModoDeGrabar::Pantalla);
        assert_eq!(
            std::fs::read_to_string(dir.join("voz/modo-de-grabar.txt")).unwrap(),
            "pantalla"
        );
        assert_eq!(n.modo_de_grabar(), ModoDeGrabar::Pantalla);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_nota_de_una_clase_y_lo_suyo_abren_la_clase() {
        let mut voz = Mensaje {
            id: "v".into(),
            ..Default::default()
        };
        assert_eq!(de_una_clase(&voz), None, "una nota de voz cualquiera no");
        voz.resto
            .insert("grabacion".into(), serde_json::json!("clase"));
        assert_eq!(de_una_clase(&voz), Some(("v".into(), 0)));
        let mut foto = Mensaje {
            id: "f".into(),
            ..Default::default()
        };
        foto.resto.insert(
            "enGrabacion".into(),
            serde_json::json!({ "de": "v", "ms": 65_000 }),
        );
        assert_eq!(de_una_clase(&foto), Some(("v".into(), 65_000)));
        // Caso negativo: un enlace roto (sin `de`) no es de ninguna clase.
        foto.resto
            .insert("enGrabacion".into(), serde_json::json!({ "ms": 1 }));
        assert_eq!(de_una_clase(&foto), None);
    }

    #[test]
    fn las_senales_juntan_marcadores_y_banderitas_sin_repetir() {
        let mut m = Mensaje::default();
        puro::poner_marcador(&mut m.resto, 5_000, "ojo");
        m.resto
            .insert(MARCAS.into(), serde_json::json!([5_000, 1_000, 1_000]));
        let s = senales_de(&m);
        assert_eq!(s.len(), 2, "la banderita del mismo minuto no se repite");
        assert!(s[0].banderita && s[0].ms == 1_000);
        assert!(!s[1].banderita && s[1].texto == "ojo");
        // Quitar la ultima banderita quita la clave, como el movil.
        assert!(quitar_banderita(&mut m, 1_000));
        assert!(quitar_banderita(&mut m, 5_000));
        assert!(!m.resto.contains_key(MARCAS));
        // Caso negativo: quitar una que no esta no cambia nada.
        assert!(!quitar_banderita(&mut m, 7));
    }

    #[test]
    fn el_texto_se_parte_en_trozos_con_su_minuto_y_sin_marcas() {
        let t = trozos_de_la_letra(
            "[0:00] **llamar** al aparejador\n\n[1:02:03] y pedir\nel presupuesto\n\n\n![audio](x.m4a)\n\nsuelto",
        );
        assert_eq!(
            t,
            vec![
                (Some(0), "llamar al aparejador".to_string()),
                (Some(3_723_000), "y pedir\nel presupuesto".to_string()),
                (None, "suelto".to_string()),
            ]
        );
        // Caso negativo: `[1:5]` no es un minuto, y nada no es nada.
        assert_eq!(
            trozos_de_la_letra("[1:5] cita"),
            vec![(None, "[1:5] cita".to_string())]
        );
        assert!(trozos_de_la_letra("  \n\n ").is_empty());
    }

    #[test]
    fn las_filas_llevan_las_senales_en_su_minuto() {
        let lineas = vec![
            LineaDeClase {
                ms: Some(0),
                texto: "a".into(),
            },
            LineaDeClase {
                ms: Some(10_000),
                texto: "b".into(),
            },
        ];
        let senales = vec![SenalDeClase {
            ms: 3_000,
            texto: String::new(),
            banderita: true,
        }];
        let f = filas_de(&lineas, &senales);
        assert_eq!(
            f.iter()
                .map(|f| (f.linea, f.senal, f.ms))
                .collect::<Vec<_>>(),
            vec![
                (Some(0), None, 0),
                (None, Some(0), 3_000),
                (Some(1), None, 10_000)
            ]
        );
        // Caso negativo: sin nada, nada.
        assert!(filas_de(&[], &[]).is_empty());
    }

    #[test]
    fn el_trozo_de_al_lado() {
        let t = vec![0, 10_000, 20_000];
        assert_eq!(
            clase_otro_trozo(t.clone(), 10_100, true),
            Some(20_000),
            "a menos de 200 ms, el siguiente"
        );
        assert_eq!(clase_otro_trozo(t.clone(), 9_700, true), Some(10_000));
        assert_eq!(
            clase_otro_trozo(t.clone(), 10_500, false),
            Some(0),
            "recien empezado, el anterior"
        );
        assert_eq!(
            clase_otro_trozo(t.clone(), 15_000, false),
            Some(10_000),
            "si no, el principio de este"
        );
        // Caso negativo: despues del ultimo no hay siguiente, y sin texto nada.
        assert_eq!(clase_otro_trozo(t, 25_000, true), None);
        assert_eq!(clase_otro_trozo(vec![], 0, false), None);
    }

    #[test]
    fn la_clase_se_lee_y_sus_marcadores_se_guardan_en_la_nota() {
        let (dir, n) = nucleo("leer");
        let chat = n.chat_guardados().unwrap().id;
        let audio = dir.join("clase.m4a");
        std::fs::write(&audio, b"m4a").unwrap();
        let png = dir.join("a.png");
        std::fs::write(&png, b"png").unwrap();
        let hecho = n
            .guardar_clase(
                chat.clone(),
                Some(audio.display().to_string()),
                60_000,
                vec![1, 2],
                None,
                vec![crate::CapturaClase {
                    ms: 9_000,
                    ruta_png: png.display().to_string(),
                }],
                0,
            )
            .unwrap();
        let voz = hecho.voz.unwrap();
        let foto = &hecho.capturas[0];
        assert!(n.cambio_del_cuaderno_ms(chat.clone()) > 0);
        assert_eq!(
            n.cambio_del_cuaderno_ms("../fuera".into()),
            0,
            "un chat que no vale no se mira"
        );
        // Desde la captura: su nota, en el minuto de la captura.
        assert_eq!(
            n.de_una_clase(chat.clone(), foto.id.clone()).unwrap(),
            Some(EnUnaClase {
                voz: voz.id.clone(),
                ms: 9_000
            })
        );
        let c = n
            .clase_para_estudiar(chat.clone(), foto.id.clone())
            .unwrap();
        assert_eq!(c.id, voz.id);
        assert_eq!(c.duracion_ms, 60_000);
        assert!(c.audio.is_some() && c.video.is_none());
        assert_eq!(
            c.capturas.iter().map(|c| c.ms).collect::<Vec<_>>(),
            vec![9_000]
        );
        assert!(c.lineas.is_empty() && c.senales.is_empty());
        // Un marcador con nota, en el formato de Windows.
        let c = n
            .poner_marcador_de_clase(
                chat.clone(),
                voz.id.clone(),
                4_000,
                "  ojo\ncon esto ".into(),
            )
            .unwrap();
        assert_eq!(
            c.senales,
            vec![SenalDeClase {
                ms: 4_000,
                texto: "ojo con esto".into(),
                banderita: false
            }]
        );
        let linea =
            std::fs::read_to_string(almacen::carpeta(&dir, &chat).join("guardados.jsonl")).unwrap();
        assert!(
            linea.contains(r#""marcadores":[{"ms":4000,"texto":"ojo con esto"}]"#),
            "{linea}"
        );
        // Quitarlo quita la clave.
        let c = n
            .quitar_senal_de_clase(chat.clone(), voz.id.clone(), 4_000)
            .unwrap();
        assert!(c.senales.is_empty());
        let linea =
            std::fs::read_to_string(almacen::carpeta(&dir, &chat).join("guardados.jsonl")).unwrap();
        assert!(!linea.contains("marcadores"));
        // Caso negativo: un mensaje que no es de una clase no abre nada, y
        // uno que no esta da error.
        let nota = n.escribir_nota(chat.clone(), "hola".into(), None).unwrap();
        assert_eq!(n.de_una_clase(chat.clone(), nota.id).unwrap(), None);
        assert!(n.clase_para_estudiar(chat, "nadie".into()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_fotograma_anotado_entra_como_una_captura_de_la_clase_en_su_minuto() {
        let (dir, n) = nucleo("fotograma");
        let chat = n.chat_guardados().unwrap().id;
        let audio = dir.join("clase.m4a");
        std::fs::write(&audio, b"m4a").unwrap();
        let png = dir.join("a.png");
        std::fs::write(&png, b"png").unwrap();
        let hecho = n
            .guardar_clase(
                chat.clone(),
                Some(audio.display().to_string()),
                90_000,
                vec![],
                None,
                vec![crate::CapturaClase {
                    ms: 9_000,
                    ruta_png: png.display().to_string(),
                }],
                0,
            )
            .unwrap();
        let voz = hecho.voz.unwrap();
        let fotograma = dir.join("fotograma.png");
        std::fs::write(&fotograma, b"png-del-video").unwrap();
        let f = n
            .guardar_fotograma_de_clase(
                chat.clone(),
                voz.id.clone(),
                65_000,
                fotograma.display().to_string(),
            )
            .unwrap();
        // La imagen, como la de Windows: su fichero, su pie y su enlace.
        assert_eq!(f.imagen.clase, "IMAGEN");
        assert!(
            f.imagen.nombre.starts_with("clase-") && f.imagen.nombre.ends_with("-65000.png"),
            "{}",
            f.imagen.nombre
        );
        let nombre = voz.nombre.trim_end_matches(".m4a").to_string();
        assert_eq!(f.imagen.texto, format!("⏱ 1:05 · {nombre}"));
        let carpeta = almacen::carpeta(&dir, &chat);
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        let m = c.mensajes.iter().find(|m| m.id == f.imagen.id).unwrap();
        assert_eq!(
            m.resto.get(CAMPO_EN_GRABACION),
            Some(&serde_json::json!({ "de": voz.id, "ms": 65_000 }))
        );
        // En la nota, en su minuto entre las que habia.
        let nota = c.mensajes.iter().find(|m| m.id == voz.id).unwrap();
        assert_eq!(
            capturas_de(nota).into_iter().map(|(ms, _)| ms).collect::<Vec<_>>(),
            vec![9_000, 65_000]
        );
        assert_eq!(f.clase.capturas.len(), 2);
        // Caso negativo: sin nota no hay clase a la que sumarlo.
        assert!(
            n.guardar_fotograma_de_clase(chat, "nadie".into(), 0, fotograma.display().to_string())
                .is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_carpeta_de_una_grabacion_se_estudia_desde_su_nota() {
        let s = cuaderno::Sello {
            cuando: 1_761_400_000_004,
            numero: 4,
            aparato: "PC".into(),
            proyecto: "p".into(),
        };
        let mut c = Carpeta::grabacion("v", 65_000);
        c.anadir_al_minuto("v", "", 0);
        let m = pixpin_proyecto::carpeta_de_paginas::mensaje_de_grabacion("Clase", &c, &s);
        assert_eq!(clase_que_estudia(&m), Some(("v".into(), 0)));
        // Caso negativo: sin nota (solo video y capturas), ni una de paginas.
        let sin = pixpin_proyecto::carpeta_de_paginas::mensaje_de_grabacion(
            "Clase",
            &Carpeta::grabacion("", 0),
            &s,
        );
        assert_eq!(clase_que_estudia(&sin), None);
        let normal = pixpin_proyecto::carpeta_de_paginas::mensaje_nuevo("Diario", &s);
        assert_eq!(clase_que_estudia(&normal), None);
    }

    #[test]
    fn lo_que_se_ve_se_acerca_como_un_lienzo() {
        let caja = CajaDeClase {
            x: 0.0,
            y: 0.0,
            ancho: 800.0,
            alto: 450.0,
        };
        let e = encuadre_encajado();
        assert!(encuadre_esta_encajado(e));
        // Encajada, un video de 1600x900 se ve entero a la mitad.
        assert_eq!(encuadre_destino(e, caja, 1600.0, 900.0).ancho, 800.0);
        assert_eq!(encuadre_por_ciento(e, caja, 1600.0, 900.0), 50);
        // Acercar hacia una esquina deja esa esquina donde estaba.
        let a = encuadre_acercar(e, caja, 1600.0, 900.0, 2.0, 0.0, 0.0);
        let d = encuadre_destino(a, caja, 1600.0, 900.0);
        assert!((d.x, d.y) == (0.0, 0.0) && d.ancho == 1600.0, "{d:?}");
        assert!(!encuadre_esta_encajado(a));
        // Caso negativo: no se aleja de encajada ni se pierde de vista.
        let lejos = encuadre_acercar(e, caja, 1600.0, 900.0, 0.5, 400.0, 225.0);
        assert!(encuadre_esta_encajado(lejos));
        let movida = encuadre_mover(a, caja, 1600.0, 900.0, 5_000.0, 0.0);
        assert!(encuadre_destino(movida, caja, 1600.0, 900.0).x <= 0.0);
        assert!(encuadre_paso() > 1.0);
        assert_eq!(clase_posicion_a_la_vista(27_000, Some(4_000)), 4_000);
        assert_eq!(clase_posicion_a_la_vista(27_000, None), 27_000);
    }
}
