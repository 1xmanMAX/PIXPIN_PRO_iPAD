//! **Grabar una clase o una reunion** (Windows edd0ffa, 24-oct-2026, y el
//! video de 20162e9 y df01da9, 25-oct-2026): lo que se guarda en el chat al
//! terminar, al caracter de Windows (`ventana_chat/grabar_clase.rs`), para
//! que sincronice sin cambios.
//!
//! La ventanita (preparar, la barra de grabar con «Captura», la zona y la
//! pausa) es de Swift (`Grabar/GrabarClase.swift`): el microfono con
//! AVAudioEngine y el sonido del Mac con ScreenCaptureKit, mezclados en un
//! solo `.m4a` por la mezcla del motor de audio de Windows
//! ([`crate::mezcla_audio`]); con «Video de esta pantalla», ademas un MP4
//! (H.264 + AAC) con el mismo sonido (`Grabar/VideoDeClase.swift`, con el
//! perfil, la cadena de caidas y el ritmo del motor de video de Windows,
//! [`crate::motor_video`]).
//! Aqui entra lo ya grabado con [`Nucleo::guardar_clase`], con numeros
//! seguidos:
//!
//! 1. la nota de voz `Clase 24-10 18h05.m4a` (`clase-nombre` y
//!    `fecha_del_nombre` de Windows), con `duracionMs` y `picos`, como
//!    `guardar_la_voz`;
//! 2. detras, si se grabo, el video `Clase 24-10 18h05.mp4` (`ARCHIVO`,
//!    movido y no copiado) con `enGrabacion: {de: <id de la voz>, ms: 0}`;
//! 3. detras, cada captura como imagen `clase-<ms>-<n>.png` con el pie
//!    `⏱ 12:34 · Clase 24-10 18h05` y `enGrabacion: {de, ms}`;
//! 4. la voz lleva en `resto` `grabacion: "clase"`, `capturas: [{ms, id}]`
//!    en orden (**siempre**, aunque este vacia) y `video: {id}` si entro.
//!
//! Sin nota (no se pudo cerrar o fue un resbalon) entran igual las capturas
//! y el video, sueltos. Un video que no entra en el chat no se tira: queda
//! en la carpeta del proyecto como `Clase {fecha}.mp4`.
//!
//! La pantalla de la letra de Windows (el texto a un lado y la imagen del
//! minuto que suena al otro) sale de [`Nucleo::capturas_de_la_clase`] y
//! [`captura_de_la_clase_en`].

use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{self, Mensaje};

use crate::chats::validar_chat;
use crate::voz::{acotar_picos, marca_de_tiempo};
use crate::{ErrorNucleo, MensajeChat, Nucleo};

/// El campo de la voz con sus capturas (`[{ms, id}]`).
pub const CAMPO_CAPTURAS: &str = "capturas";
/// El campo de cada captura con su nota (`{de, ms}`).
pub const CAMPO_EN_GRABACION: &str = "enGrabacion";
/// El campo que dice que la voz es de una clase (`"clase"`).
pub const CAMPO_GRABACION: &str = "grabacion";
/// El campo de la voz con su video (`{id}`).
pub const CAMPO_VIDEO: &str = "video";

/// Las cuentas del reloj del video van en 100 ns.
const UNIDADES_POR_SEGUNDO: i64 = 10_000_000;

/// El minuto de una captura en el pie y en los avisos: `12:34` o
/// `1:02:03` (`grabar_clase::minuto`, igual que `marca_de_tiempo`).
pub fn minuto(ms: i64) -> String {
    marca_de_tiempo(ms)
}

/// La fecha del nombre de la nota, en hora local: `24-10 18h05` (sin `:`,
/// que Windows no admite en un nombre de fichero).
pub fn fecha_del_nombre(ahora_local_ms: i64) -> String {
    let dias = ahora_local_ms.div_euclid(86_400_000);
    let (_, mes, dia) = crate::automatizaciones::fecha_civil(dias);
    let del_dia = ahora_local_ms.rem_euclid(86_400_000) / 60_000;
    format!("{dia:02}-{mes:02} {:02}h{:02}", del_dia / 60, del_dia % 60)
}

/// El nombre de la nota (`clase-nombre = Clase { $fecha }`), sin extension.
pub fn nombre_de_la_clase(ahora_local_ms: i64) -> String {
    format!("Clase {}", fecha_del_nombre(ahora_local_ms))
}

/// El fichero de una nota de voz con nombre puesto
/// (`ventana_chat::nombre_de_la_voz(Some(n), _)` de Windows): ese nombre
/// (80 caracteres como mucho) con su `.m4a`, sin doblarlo. En blanco, la
/// hora (`voz_<ms>.m4a`).
pub fn nombre_de_la_voz_con(nombre: &str, ahora_ms: i64) -> String {
    let n = nombre.trim();
    if n.is_empty() {
        return crate::voz::nombre_de_la_voz(ahora_ms);
    }
    let n: String = n.chars().take(80).collect();
    if n.to_lowercase().ends_with(".m4a") {
        n
    } else {
        format!("{n}.m4a")
    }
}

/// El fichero de la captura `n` (desde 1): `clase-<ms>-<n>.png`.
pub fn fichero_de_captura(cuando_ms: i64, n: usize) -> String {
    format!("clase-{cuando_ms}-{n}.png")
}

/// El pie de una captura: `⏱ 12:34 · Clase 24-10 18h05`.
pub fn pie_de_captura(ms: i64, nombre: &str) -> String {
    format!("⏱ {} · {}", minuto(ms), nombre)
}

/// Lo que se apunta en la nota de voz: sus capturas en orden, `[{ms, id}]`.
pub fn capturas_en_json(capturas: &[(i64, String)]) -> serde_json::Value {
    let mut v: Vec<&(i64, String)> = capturas.iter().collect();
    v.sort_by_key(|(ms, _)| *ms);
    serde_json::Value::Array(
        v.into_iter()
            .map(|(ms, id)| serde_json::json!({ "ms": ms, "id": id }))
            .collect(),
    )
}

/// Las capturas de una nota de voz grabada como clase, `(ms, id)` en orden.
/// Vacio si no es de una clase o si lo apuntado no se entiende.
pub fn capturas_de(m: &Mensaje) -> Vec<(i64, String)> {
    let mut v: Vec<(i64, String)> = m
        .resto
        .get(CAMPO_CAPTURAS)
        .and_then(|c| c.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|c| Some((c.get("ms")?.as_i64()?, c.get("id")?.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|(ms, _)| *ms);
    v
}

/// La captura que toca en `posicion_ms`: la ultima hecha antes (o en ese
/// mismo momento). Antes de la primera, la primera: algo hay que ensenar.
pub fn captura_en(tiempos: &[i64], posicion_ms: i64) -> Option<usize> {
    if tiempos.is_empty() {
        return None;
    }
    Some(tiempos.iter().rposition(|ms| *ms <= posicion_ms).unwrap_or(0))
}

/// **La captura del minuto que suena** entre las de una clase (`tiempos`,
/// en orden): su posicion, o `None` sin capturas.
#[uniffi::export]
pub fn captura_de_la_clase_en(tiempos: Vec<i64>, posicion_ms: i64) -> Option<u32> {
    captura_en(&tiempos, posicion_ms).map(|n| n as u32)
}

// --- El video -------------------------------------------------------------

/// **El tamano al que se graba** una pantalla de `ancho`x`alto` pixeles sin
/// pasar de `tope_ancho`x`tope_alto` (`escala::medida_de_salida`, al
/// numero): la misma forma (nunca se estira), nunca mas grande que la
/// pantalla y con los dos lados pares, que es lo unico que admite H.264.
pub fn medida_de_salida(ancho: u32, alto: u32, tope_ancho: u32, tope_alto: u32) -> (u32, u32) {
    let (ancho, alto) = (ancho.max(2), alto.max(2));
    let (tope_ancho, tope_alto) = (tope_ancho.max(2), tope_alto.max(2));
    // La escala como fraccion exacta (num/den), sin redondeos de coma
    // flotante: la menor de las dos, y nunca mas de uno.
    let (num, den) =
        if u64::from(tope_ancho) * u64::from(alto) <= u64::from(tope_alto) * u64::from(ancho) {
            (u64::from(tope_ancho), u64::from(ancho))
        } else {
            (u64::from(tope_alto), u64::from(alto))
        };
    let (num, den) = if num >= den { (1, 1) } else { (num, den) };
    let w = (u64::from(ancho) * num / den) as u32;
    let h = (u64::from(alto) * num / den) as u32;
    ((w & !1).max(2), (h & !1).max(2))
}

/// El instante (100 ns) al que se llega tras `muestras` de sonido a
/// `muestreo` (`escala::tiempo_de_muestras`): el reloj del video.
pub(crate) fn tiempo_de_muestras(muestras: u64, muestreo: u32) -> i64 {
    if muestreo == 0 {
        return 0;
    }
    (u128::from(muestras) * UNIDADES_POR_SEGUNDO as u128 / u128::from(muestreo)) as i64
}

/// El nombre del video en el chat: `Clase 24-10 18h05.mp4`.
pub fn nombre_del_video(nombre: &str) -> String {
    format!("{nombre}.mp4")
}

/// Un nombre que no pise nada en `carpeta`: `nombre`, o `tronco (n).ext`
/// (`grabar_clase::nombre_libre`).
pub fn nombre_libre(carpeta: &std::path::Path, nombre: &str) -> String {
    let (tronco, extension) = match nombre.rsplit_once('.') {
        Some((t, e)) if !t.is_empty() => (t.to_string(), format!(".{e}")),
        _ => (nombre.to_string(), String::new()),
    };
    let mut intento = nombre.to_string();
    let mut n = 1;
    while carpeta.join(&intento).exists() && n < 10_000 {
        intento = format!("{tronco} ({n}){extension}");
        n += 1;
    }
    intento
}

/// **El video que no entro en el chat** (fallo la nota, el disco) no se
/// tira: es la reunion entera. Va a la carpeta del proyecto, al lado de su
/// conversacion, como `Clase {fecha}.mp4` sin pisar nada
/// (`Estado::rescatar_video`). Devuelve donde quedo.
pub(crate) fn rescatar_video(
    carpeta: &std::path::Path,
    nombre: &str,
    video: &std::path::Path,
) -> std::path::PathBuf {
    let elegido = nombre_libre(carpeta, &almacen::nombre_seguro(&nombre_del_video(nombre)));
    let destino = carpeta.join(elegido);
    if std::fs::create_dir_all(carpeta).is_ok() && std::fs::rename(video, &destino).is_ok() {
        return destino;
    }
    if std::fs::copy(video, &destino).is_ok() {
        let _ = std::fs::remove_file(video);
        return destino;
    }
    let _ = std::fs::remove_file(&destino);
    video.to_path_buf()
}

/// Una captura hecha grabando: en que milisegundo de la nota y el PNG ya
/// escrito en un temporal (lo borra quien lo escribio).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CapturaClase {
    pub ms: i64,
    pub ruta_png: String,
}

/// Lo que dejo la clase en el chat, en el orden del cuaderno: la voz como
/// quedo (si entro), su video (si lo habia y entro) y sus capturas.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ClaseGuardada {
    pub voz: Option<MensajeChat>,
    pub video: Option<MensajeChat>,
    pub capturas: Vec<MensajeChat>,
}

/// Una captura de una clase para la pantalla de su letra (o sus tarjetas):
/// su minuto, el id de su mensaje y su fichero en este Mac (`None` si el
/// mensaje ya no esta o su ruta no se resuelve aqui).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CapturaDeLaClase {
    pub ms: i64,
    pub id: String,
    pub ruta: Option<String>,
}

#[uniffi::export]
impl Nucleo {
    /// **Mete una clase grabada en `chat`**: la nota de voz de `ruta_audio`,
    /// detras el video de `ruta_video` y cada captura, enlazados, como
    /// `grabar_clase::guardar` de Windows. `desfase_ms` es el de la hora
    /// local (el nombre lleva la fecha de aqui). Bloqueante: copia las
    /// capturas y mueve el video (que ya no estara en `ruta_video`); la nota
    /// y las capturas de origen no se tocan.
    ///
    /// Sin `ruta_audio` (la nota no se pudo cerrar) entran igual el video y
    /// las capturas, sueltos. Una captura que no entra se salta sin dejar un
    /// hueco en los numeros; un video que no entra queda en la carpeta del
    /// proyecto.
    #[allow(clippy::too_many_arguments)]
    pub fn guardar_clase(
        &self,
        chat: String,
        ruta_audio: Option<String>,
        duracion_ms: i64,
        picos: Vec<i32>,
        ruta_video: Option<String>,
        capturas: Vec<CapturaClase>,
        desfase_ms: i64,
    ) -> Result<ClaseGuardada, ErrorNucleo> {
        validar_chat(&chat)?;
        let nombre = nombre_de_la_clase(crate::ahora_ms() + desfase_ms);
        let hecho = self.guardar_clase_con(
            &chat,
            &nombre,
            ruta_audio,
            duracion_ms,
            picos,
            ruta_video.as_deref(),
            capturas,
        );
        if let Some(v) = ruta_video.as_deref().map(std::path::Path::new).filter(|v| v.is_file()) {
            // No entro en el chat (fallo la nota, el disco) y no se borra.
            let carpeta = almacen::carpeta(self.ruta(), &chat);
            let _ = rescatar_video(&carpeta, &nombre, v);
        }
        hecho
    }
}

impl Nucleo {
    /// Lo de [`Nucleo::guardar_clase`] sin el rescate del video.
    #[allow(clippy::too_many_arguments)]
    fn guardar_clase_con(
        &self,
        chat: &str,
        nombre: &str,
        ruta_audio: Option<String>,
        duracion_ms: i64,
        picos: Vec<i32>,
        ruta_video: Option<&str>,
        capturas: Vec<CapturaClase>,
    ) -> Result<ClaseGuardada, ErrorNucleo> {
        let ahora = crate::ahora_ms();
        let voz = match ruta_audio {
            Some(ruta) => {
                let picos = acotar_picos(&picos);
                Some(self.adjuntar_con(
                    chat.to_string(),
                    ruta,
                    Some(nombre_de_la_voz_con(nombre, ahora)),
                    move |m| {
                        m.duracion_ms = duracion_ms.max(0);
                        m.resto
                            .insert("picos".into(), serde_json::Value::from(picos));
                    },
                )?)
            }
            None => None,
        };
        let de = voz.as_ref().map(|v| v.id.clone());
        let enlazar = |m: &mut Mensaje, ms: i64| {
            if let Some(de) = &de {
                m.resto.insert(
                    CAMPO_EN_GRABACION.into(),
                    serde_json::json!({ "de": de, "ms": ms }),
                );
            }
        };

        // El video, justo detras de la nota: es la misma reunion.
        let video = ruta_video.and_then(|ruta| {
            self.adjuntar_moviendo(
                chat.to_string(),
                ruta.to_string(),
                Some(nombre_del_video(nombre)),
                |m| enlazar(m, 0),
            )
            // Como Windows: el que no entra se queda en la carpeta.
            .ok()
        });

        let mut capturas = capturas;
        capturas.sort_by_key(|c| c.ms);
        let mut hechas = Vec::new();
        for (i, c) in capturas.into_iter().enumerate() {
            let fichero = fichero_de_captura(crate::ahora_ms(), i + 1);
            let pie = pie_de_captura(c.ms, nombre);
            let ms = c.ms;
            match self.adjuntar_con(chat.to_string(), c.ruta_png, Some(fichero), |m| {
                m.texto = pie.trim().to_string();
                enlazar(m, ms);
            }) {
                Ok(m) => hechas.push((ms, m)),
                // Como Windows: la que no entra se salta.
                Err(_) => continue,
            }
        }
        let Some(voz) = voz else {
            return Ok(ClaseGuardada {
                voz: None,
                video,
                capturas: hechas.into_iter().map(|(_, m)| m).collect(),
            });
        };
        let enlazadas: Vec<(i64, String)> =
            hechas.iter().map(|(ms, m)| (*ms, m.id.clone())).collect();
        let video_id = video.as_ref().map(|v| v.id.clone());
        let carpeta = almacen::carpeta(self.ruta(), chat);
        let fichero = carpeta.join("guardados.jsonl");
        let mut cache = self.cache_tomada();
        // Siempre, aunque no haya capturas: es lo que dice que la nota es de
        // una clase y no una nota de voz cualquiera.
        let cambiada = cuaderno::cambiar(&carpeta, &voz.id, |m| {
            m.resto
                .insert(CAMPO_GRABACION.into(), serde_json::json!("clase"));
            m.resto
                .insert(CAMPO_CAPTURAS.into(), capturas_en_json(&enlazadas));
            if let Some(id) = &video_id {
                m.resto
                    .insert(CAMPO_VIDEO.into(), serde_json::json!({ "id": id }));
            }
            true
        });
        cache.quitar(&fichero);
        drop(cache);
        // La nota y sus adjuntos ya estan dentro; si solo falta el enlace,
        // se devuelve igual (Windows lo apunta y sigue).
        let voz = match cambiada {
            Ok(Some(m)) => MensajeChat::from(m),
            _ => voz,
        };
        Ok(ClaseGuardada {
            voz: Some(voz),
            video,
            capturas: hechas.into_iter().map(|(_, m)| m).collect(),
        })
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Las capturas de una nota de voz grabada como clase** (`mensaje` es
    /// su id), en orden, con su fichero en este Mac. Vacio si no es de una
    /// clase. Bloqueante (lee el cuaderno): Swift lo guarda por mensaje.
    pub fn capturas_de_la_clase(
        &self,
        chat: String,
        mensaje: String,
    ) -> Result<Vec<CapturaDeLaClase>, ErrorNucleo> {
        validar_chat(&chat)?;
        let carpeta = almacen::carpeta(self.ruta(), &chat);
        let c = match cuaderno::Cuaderno::leer_de(&carpeta) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let Some(voz) = c.mensajes.iter().find(|m| m.id == mensaje) else {
            return Ok(Vec::new());
        };
        Ok(capturas_de(voz)
            .into_iter()
            .map(|(ms, id)| {
                let ruta = c
                    .mensajes
                    .iter()
                    .find(|m| m.id == id)
                    .and_then(|m| m.ruta.as_deref())
                    .and_then(|r| self.resolver(&chat, r))
                    .map(|r| r.display().to_string());
                CapturaDeLaClase { ms, id, ruta }
            })
            .collect())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_minuto_de_una_captura_se_lee_como_un_reloj() {
        assert_eq!(minuto(0), "0:00");
        assert_eq!(minuto(65_400), "1:05");
        assert_eq!(minuto(3_723_000), "1:02:03");
        // Caso negativo: un reloj que se fue atras no da minutos negativos.
        assert_eq!(minuto(-5), "0:00");
    }

    #[test]
    fn la_fecha_del_nombre_no_lleva_dos_puntos() {
        // 24-oct-2026 18:05 en hora local.
        let dias = 20_750; // 2026-10-24
        let ms = dias * 86_400_000 + (18 * 60 + 5) * 60_000;
        let f = fecha_del_nombre(ms);
        assert_eq!(f, "24-10 18h05");
        assert!(!f.contains(':'));
        assert_eq!(nombre_de_la_clase(ms), "Clase 24-10 18h05");
    }

    #[test]
    fn el_nombre_de_la_voz_lleva_su_m4a_una_vez() {
        assert_eq!(nombre_de_la_voz_con("Clase 24-10 18h05", 1), "Clase 24-10 18h05.m4a");
        assert_eq!(nombre_de_la_voz_con("ya.M4A", 1), "ya.M4A");
        let largo = "x".repeat(100);
        assert_eq!(nombre_de_la_voz_con(&largo, 1).chars().count(), 84);
        // Caso negativo: en blanco no hay nombre, va la hora.
        assert_eq!(nombre_de_la_voz_con("  ", 7), "voz_7.m4a");
    }

    #[test]
    fn el_pie_y_el_fichero_de_cada_captura_son_los_de_windows() {
        assert_eq!(pie_de_captura(65_400, "Clase 24-10 18h05"), "⏱ 1:05 · Clase 24-10 18h05");
        assert_eq!(fichero_de_captura(1_700, 2), "clase-1700-2.png");
    }

    #[test]
    fn las_capturas_van_y_vuelven_en_orden() {
        let mut m = Mensaje::default();
        m.resto.insert(
            CAMPO_CAPTURAS.into(),
            capturas_en_json(&[(9_000, "b".into()), (2_000, "a".into())]),
        );
        let v = capturas_de(&m);
        assert_eq!(v, vec![(2_000, "a".to_string()), (9_000, "b".to_string())]);
        // Caso negativo: una nota sin capturas no tiene ninguna, y una
        // entrada rota se salta sin llevarse las demas.
        assert!(capturas_de(&Mensaje::default()).is_empty());
        m.resto.insert(
            CAMPO_CAPTURAS.into(),
            serde_json::json!([{ "ms": "x" }, { "ms": 5, "id": "c" }]),
        );
        assert_eq!(capturas_de(&m), vec![(5, "c".to_string())]);
    }

    #[test]
    fn la_captura_del_momento_es_la_ultima_hecha_antes() {
        let c = vec![2_000, 9_000];
        assert_eq!(captura_en(&c, 500), Some(0), "antes de la primera, la primera");
        assert_eq!(captura_en(&c, 2_000), Some(0));
        assert_eq!(captura_en(&c, 8_999), Some(0));
        assert_eq!(captura_en(&c, 9_000), Some(1));
        assert_eq!(captura_de_la_clase_en(c, 99_000), Some(1));
        assert_eq!(captura_en(&[], 1_000), None);
    }

    #[test]
    fn el_video_se_encoge_sin_estirarse() {
        // La pantalla de siempre cabe justa en 720p.
        assert_eq!(medida_de_salida(1920, 1080, 1280, 720), (1280, 720));
        let (w, h) = medida_de_salida(1366, 768, 1280, 720);
        assert_eq!((w % 2, h % 2), (0, 0));
        assert_eq!(w, 1280);
        // Una pantalla Retina de 2880x1800 (16:10) manda el alto.
        assert_eq!(medida_de_salida(2880, 1800, 1280, 720), (1152, 720));
        // Una mas pequena que el tope no se agranda; vertical, manda el alto.
        assert_eq!(medida_de_salida(800, 600, 1280, 720), (800, 600));
        assert_eq!(medida_de_salida(1080, 1920, 1280, 720), (404, 720));
        // Caso negativo: una pantalla rota (0x0) no da un video de cero.
        assert_eq!(medida_de_salida(0, 0, 1280, 720), (2, 2));
    }

    #[test]
    fn el_reloj_del_video_sigue_al_sonido() {
        assert_eq!(tiempo_de_muestras(48_000, 48_000), UNIDADES_POR_SEGUNDO);
        // Una hora de sonido, sin desbordarse ni atrasarse.
        assert_eq!(tiempo_de_muestras(48_000 * 3600, 48_000), 3600 * UNIDADES_POR_SEGUNDO);
        // Caso negativo: sin muestreo no corre el reloj.
        assert_eq!(tiempo_de_muestras(48_000, 0), 0);
    }

    #[test]
    fn el_video_no_pisa_otro_con_el_mismo_nombre() {
        let dir = std::env::temp_dir().join(format!("pixpin-nucleo-clase-nombre-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(nombre_del_video("Clase 1"), "Clase 1.mp4");
        assert_eq!(nombre_libre(&dir, "Clase 1.mp4"), "Clase 1.mp4");
        std::fs::write(dir.join("Clase 1.mp4"), b"x").unwrap();
        assert_eq!(nombre_libre(&dir, "Clase 1.mp4"), "Clase 1 (1).mp4");
        std::fs::write(dir.join("Clase 1 (1).mp4"), b"x").unwrap();
        assert_eq!(nombre_libre(&dir, "Clase 1.mp4"), "Clase 1 (2).mp4");

        // El rescate: el video va a la carpeta con el nombre de la clase.
        let suelto = dir.join("temporal.video.mp4");
        std::fs::write(&suelto, b"mp4").unwrap();
        let carpeta = dir.join("proyecto");
        let r = rescatar_video(&carpeta, "Clase 25-10 09h00", &suelto);
        assert_eq!(r, carpeta.join("Clase 25-10 09h00.mp4"));
        assert_eq!(std::fs::read(&r).unwrap(), b"mp4");
        // Caso negativo: no pisa el que ya estaba, ni deja el temporal.
        assert!(!suelto.exists());
        std::fs::write(&suelto, b"otro").unwrap();
        let r2 = rescatar_video(&carpeta, "Clase 25-10 09h00", &suelto);
        assert_eq!(r2, carpeta.join("Clase 25-10 09h00 (1).mp4"));
        assert_eq!(std::fs::read(&r).unwrap(), b"mp4");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
