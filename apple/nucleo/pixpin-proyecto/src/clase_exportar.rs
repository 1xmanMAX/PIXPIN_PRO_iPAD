//! **Una grabacion para sacarla fuera** (27-oct-2026, el usuario: «en las
//! grabaciones, que el menu de compartir sea el de toda la app, con los
//! mismos formatos; sobre todo el HTML, que se vea claro e intuitivo»).
//!
//! Una clase o una reunion grabada es una nota de voz (`VOZ`, con su
//! `transcripcion` en renglones `[m:ss] texto`) y, si se grabo como clase,
//! lo enlazado en su `resto`: las capturas (`capturas: [{ms, id}]`), el
//! video (`video: {id}`), los marcadores (`marcadores: [{ms, texto}]`) y las
//! banderitas del movil (`marcas: [ms]`). Las capturas y el video llevan a su
//! vez `enGrabacion: {de, ms}`.
//!
//! Aqui esta lo puro de sacarla: **[`ClaseParaExportar`]**, lo que hace
//! falta de una grabacion con sus ficheros por ruta, venga de donde venga
//! (de la nota y sus enlaces, [`ClaseParaExportar::de_la_nota`], o de la
//! carpeta de una grabacion del chat), y lo que sale de ella:
//!
//! - [`html`]: una pagina sola, sin nada de la red, con el reproductor, la
//!   barra del tiempo con los marcadores y las capturas, la transcripcion que
//!   salta al pulsar su minuto y se resalta mientras suena, las capturas en
//!   su minuto (con lupa), claro y oscuro segun el sistema, para el movil y
//!   para imprimir.
//! - [`markdown`]: el mismo orden en Markdown, con las capturas enlazadas
//!   (de ahi sale tambien el Word).
//! - [`texto`]: la transcripcion con sus minutos.
//!
//! Nada de esto toca el disco: los ficheros los lee quien llama, que dice
//! con [`Medio`] si van dentro de la pagina o al lado.

use std::path::{Path, PathBuf};

use crate::clase::{self, Marcador};
use crate::cuaderno::{Clase, Mensaje};

/// Lo que dice en `resto` que una nota de voz es de una clase grabada.
pub const GRABACION: &str = "grabacion";

/// **Una grabacion lista para sacarla**, con sus ficheros por ruta.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClaseParaExportar {
    pub titulo: String,
    /// La fecha ya escrita en el idioma de la app («25/10/2026 18:05»).
    pub fecha: String,
    pub duracion_ms: i64,
    pub audio: Option<PathBuf>,
    pub video: Option<PathBuf>,
    /// `(ms, ruta)`, en orden.
    pub capturas: Vec<(i64, PathBuf)>,
    /// Los trozos de la transcripcion: su minuto (si lo lleva) y lo que
    /// dice, ya sin marcas.
    pub lineas: Vec<(Option<i64>, String)>,
    /// Los marcadores y las banderitas (sin nota), en orden.
    pub marcadores: Vec<Marcador>,
    /// Lo demas que cuelga de la grabacion (un PDF adjunto, una foto que no
    /// es captura).
    pub adjuntos: Vec<PathBuf>,
}

/// Si un mensaje es una grabacion: una nota de voz.
pub fn es_grabacion(m: &Mensaje) -> bool {
    m.clase == Some(Clase::Voz)
}

/// Si la nota de voz se grabo como clase (con capturas o video).
pub fn es_de_clase(m: &Mensaje) -> bool {
    m.resto.get(GRABACION).and_then(|v| v.as_str()) == Some("clase")
}

/// `enGrabacion: {de, ms}` de un mensaje: de que nota cuelga y en que
/// minuto.
pub fn en_grabacion(m: &Mensaje) -> Option<(&str, i64)> {
    let en = m.resto.get("enGrabacion")?;
    Some((
        en.get("de")?.as_str()?,
        en.get("ms").and_then(|v| v.as_i64()).unwrap_or(0).max(0),
    ))
}

fn es_video(ruta: &Path) -> bool {
    matches!(
        extension(ruta).as_str(),
        "mp4" | "m4v" | "webm" | "mov" | "mkv"
    )
}

fn extension(ruta: &Path) -> String {
    ruta.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

impl ClaseParaExportar {
    /// **De la nota de voz y lo enlazado en su `resto`**. `mensajes` es el
    /// chat entero (para encontrar las capturas y el video por su id);
    /// `ruta` da el fichero de un mensaje si esta en este equipo; `lineas`
    /// son los trozos de la transcripcion ya partidos.
    pub fn de_la_nota(
        voz: &Mensaje,
        mensajes: &[Mensaje],
        lineas: Vec<(Option<i64>, String)>,
        fecha: String,
        ruta: &dyn Fn(&Mensaje) -> Option<PathBuf>,
    ) -> ClaseParaExportar {
        let por_id = |id: &str| mensajes.iter().find(|m| m.id == id);
        let mut usados: Vec<&str> = Vec::new();
        let mut capturas: Vec<(i64, PathBuf)> = Vec::new();
        for c in voz
            .resto
            .get("capturas")
            .and_then(|c| c.as_array())
            .into_iter()
            .flatten()
        {
            let (Some(ms), Some(id)) = (
                c.get("ms").and_then(|v| v.as_i64()),
                c.get("id").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            if let Some(m) = por_id(id) {
                usados.push(&m.id);
                if let Some(r) = ruta(m) {
                    capturas.push((ms.max(0), r));
                }
            }
        }
        let mut video = voz
            .resto
            .get("video")
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .and_then(por_id)
            .and_then(|m| {
                usados.push(&m.id);
                ruta(m)
            });
        // Lo que cuelga de la nota sin estar en sus listas (lo que llega por
        // la carpeta de la grabacion): una imagen es una captura de su
        // minuto; un video, el video; lo demas, un adjunto.
        let mut adjuntos = Vec::new();
        for m in mensajes {
            if m.id == voz.id || usados.contains(&m.id.as_str()) {
                continue;
            }
            let Some((de, ms)) = en_grabacion(m) else {
                continue;
            };
            if de != voz.id {
                continue;
            }
            let Some(r) = ruta(m) else {
                continue;
            };
            if m.clase == Some(Clase::Imagen) {
                capturas.push((ms, r));
            } else if video.is_none() && es_video(&r) {
                video = Some(r);
            } else {
                adjuntos.push(r);
            }
        }
        capturas.sort_by_key(|(ms, _)| *ms);
        let mut marcadores = clase::marcadores_de(&voz.resto);
        for ms in voz
            .resto
            .get("marcas")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_i64())
        {
            if !marcadores.iter().any(|m| m.ms == ms) {
                marcadores.push(Marcador {
                    ms: ms.max(0),
                    texto: String::new(),
                });
            }
        }
        marcadores.sort_by_key(|m| m.ms);
        let titulo = voz.nombre.trim();
        let titulo = match titulo.rsplit_once('.') {
            Some((base, ext)) if !base.is_empty() && ext.len() <= 4 => base,
            _ => titulo,
        };
        ClaseParaExportar {
            titulo: if titulo.is_empty() {
                voz.texto.lines().next().unwrap_or("").trim().to_string()
            } else {
                titulo.to_string()
            },
            fecha,
            duracion_ms: voz.duracion_ms.max(0),
            audio: ruta(voz),
            video,
            capturas,
            lineas,
            marcadores,
            adjuntos,
        }
    }

    /// La duracion: la apuntada o, sin ella, la ultima senal.
    pub fn duracion(&self) -> i64 {
        if self.duracion_ms > 0 {
            return self.duracion_ms;
        }
        self.lineas
            .iter()
            .filter_map(|(ms, _)| *ms)
            .chain(self.capturas.iter().map(|(ms, _)| *ms))
            .chain(self.marcadores.iter().map(|m| m.ms))
            .max()
            .unwrap_or(0)
    }

    /// Todos los ficheros de la grabacion (para «Originales»), sin repetir.
    pub fn ficheros(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = Vec::new();
        for r in self
            .audio
            .iter()
            .chain(self.video.iter())
            .chain(self.capturas.iter().map(|(_, r)| r))
            .chain(self.adjuntos.iter())
        {
            if !v.contains(r) {
                v.push(r.clone());
            }
        }
        v
    }
}

// --- El orden de lo que se lee ---------------------------------------------

/// Una fila de lo que se lee: una linea, una captura o un marcador, por su
/// sitio en su lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fila {
    Linea(usize),
    Captura(usize),
    Marcador(usize),
}

/// **La transcripcion con las capturas y los marcadores en su minuto.** Las
/// lineas conservan su orden; lo demas va delante de la primera linea con
/// minuto posterior (como [`clase::mezclar`]): una captura de las 12:05 va
/// detras de la linea que empezo a las 12:00, que es la que se estaba
/// diciendo. En el mismo minuto, el marcador antes que la captura.
pub fn filas(c: &ClaseParaExportar) -> Vec<Fila> {
    let mut senales: Vec<(i64, u8, Fila)> = c
        .marcadores
        .iter()
        .enumerate()
        .map(|(i, m)| (m.ms, 0, Fila::Marcador(i)))
        .chain(
            c.capturas
                .iter()
                .enumerate()
                .map(|(i, (ms, _))| (*ms, 1, Fila::Captura(i))),
        )
        .collect();
    senales.sort_by_key(|(ms, r, _)| (*ms, *r));
    let mut v = Vec::with_capacity(c.lineas.len() + senales.len());
    let mut s = 0;
    for (i, (t, _)) in c.lineas.iter().enumerate() {
        if let Some(t) = t {
            while s < senales.len() && senales[s].0 < *t {
                v.push(senales[s].2);
                s += 1;
            }
        }
        v.push(Fila::Linea(i));
    }
    v.extend(senales[s..].iter().map(|(_, _, f)| *f));
    v
}

/// El minuto como se lee: `12:34` o `1:02:03`.
pub fn minuto(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// El tipo MIME de un fichero de la grabacion, por su extension.
pub fn mime_de(ruta: &Path) -> &'static str {
    match extension(ruta).as_str() {
        "m4a" | "mp4a" | "aac" => "audio/mp4",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "weba" => "audio/webm",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

// --- Los rotulos -------------------------------------------------------------

/// Lo que la pagina dice, en el idioma de la app: aqui no se sabe de
/// idiomas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rotulos {
    /// `es`, `en`... para el `lang` de la pagina.
    pub idioma: String,
    pub transcripcion: String,
    pub capturas: String,
    pub captura: String,
    pub marcadores: String,
    pub marcador: String,
    pub adjuntos: String,
    pub duracion: String,
    pub sin_texto: String,
    pub buscar: String,
    pub seguir: String,
    pub reproducir: String,
    pub velocidad: String,
    pub ir_al_minuto: String,
    pub cerrar: String,
    pub audio: String,
    pub video: String,
}

impl Default for Rotulos {
    fn default() -> Self {
        Rotulos {
            idioma: "es".into(),
            transcripcion: "Transcripción".into(),
            capturas: "Capturas".into(),
            captura: "Captura".into(),
            marcadores: "Marcadores".into(),
            marcador: "Marcador".into(),
            adjuntos: "Adjuntos".into(),
            duracion: "Duración".into(),
            sin_texto: "Esta grabación aún no tiene transcripción.".into(),
            buscar: "Buscar en la transcripción".into(),
            seguir: "Seguir el audio".into(),
            reproducir: "Reproducir o pausar".into(),
            velocidad: "Velocidad".into(),
            ir_al_minuto: "Ir a este minuto".into(),
            cerrar: "Cerrar".into(),
            audio: "Audio".into(),
            video: "Vídeo".into(),
        }
    }
}

// --- Escapar -------------------------------------------------------------------

/// Texto para HTML, dentro de una etiqueta o de un atributo entre comillas.
pub fn escapar(s: &str) -> String {
    let mut v = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => v.push_str("&amp;"),
            '<' => v.push_str("&lt;"),
            '>' => v.push_str("&gt;"),
            '"' => v.push_str("&quot;"),
            '\'' => v.push_str("&#39;"),
            c => v.push(c),
        }
    }
    v
}

/// **Una ruta relativa como URL**: todo lo que no es letra, cifra, `-._~`
/// o `/` va en `%XX`. Asi un nombre con espacios o acentos se abre, y nada
/// que venga de un nombre de fichero puede volverse `http:` (los dos puntos
/// van escapados) ni salirse de la carpeta (`..` se quita).
pub fn url_relativa(ruta: &str) -> String {
    let limpia: Vec<&str> = ruta
        .split(['/', '\\'])
        .filter(|t| !t.is_empty() && *t != "." && *t != "..")
        .collect();
    let mut v = String::new();
    for (i, trozo) in limpia.iter().enumerate() {
        if i > 0 {
            v.push('/');
        }
        for b in trozo.bytes() {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                v.push(b as char);
            } else {
                v.push_str(&format!("%{b:02X}"));
            }
        }
    }
    v
}

// --- HTML ------------------------------------------------------------------------

/// Como va un fichero en la pagina: dentro (base64) o al lado, por su ruta
/// relativa a la pagina.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Medio {
    Incrustado { mime: String, bytes: Vec<u8> },
    AlLado(String),
}

impl Medio {
    fn src(&self) -> String {
        match self {
            Medio::Incrustado { mime, bytes } => format!(
                "data:{};base64,{}",
                escapar(mime),
                pixpin_motor2d::exportar::base64(bytes)
            ),
            Medio::AlLado(r) => url_relativa(r),
        }
    }
}

const CSS: &str = include_str!("clase_exportar/pagina.css");
const JS: &str = include_str!("clase_exportar/pagina.js");

fn texto_con_saltos(s: &str) -> String {
    // Los saltos se quedan: la pagina los ensena (`pre-line`) y la busqueda
    // no los pierde al rehacer el texto.
    escapar(s.trim())
}

/// El nombre que se ensena de un fichero.
fn nombre(ruta: &Path) -> String {
    ruta.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Un minuto que salta: un enlace con `data-ms` (el guion lo lleva al
/// reproductor) o, sin nada que reproducir, solo el minuto.
fn minuto_html(ms: i64, con_medio: bool) -> String {
    let m = minuto(ms);
    if con_medio {
        format!("<a class=\"t\" href=\"#t-{ms}\" data-ms=\"{ms}\">{m}</a>")
    } else {
        format!("<span class=\"t\">{m}</span>")
    }
}

/// **La pagina de la grabacion**, entera en un fichero. `medio` dice como
/// va cada fichero (el audio, el video, cada captura); `None`, no va.
pub fn html(c: &ClaseParaExportar, r: &Rotulos, medio: &dyn Fn(&Path) -> Option<Medio>) -> String {
    let audio = c.audio.as_deref().and_then(|a| Some((a, medio(a)?)));
    let video = c.video.as_deref().and_then(|v| Some((v, medio(v)?)));
    let con_medio = audio.is_some() || video.is_some();
    let duracion = c.duracion();
    let capturas: Vec<Option<String>> = c
        .capturas
        .iter()
        .map(|(_, ruta)| medio(ruta).map(|m| m.src()))
        .collect();
    let n_capturas = capturas.iter().filter(|c| c.is_some()).count();
    let mut h = String::with_capacity(64 * 1024);
    h.push_str("<!doctype html>\n");
    h.push_str(&format!(
        "<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"color-scheme\" content=\"light dark\">\n\
         <meta name=\"generator\" content=\"PixPin\">\n<title>{}</title>\n<style>\n{CSS}</style>\n</head>\n",
        escapar(&r.idioma),
        escapar(&c.titulo)
    ));
    h.push_str(&format!(
        "<body data-duracion=\"{duracion}\">\n<header class=\"cab\">\n<h1>{}</h1>\n<p class=\"meta\">",
        escapar(&c.titulo)
    ));
    let mut meta = Vec::new();
    if !c.fecha.trim().is_empty() {
        meta.push(format!("<span>{}</span>", escapar(&c.fecha)));
    }
    if duracion > 0 {
        meta.push(format!(
            "<span>{} {}</span>",
            escapar(&r.duracion),
            minuto(duracion)
        ));
    }
    if n_capturas > 0 {
        meta.push(format!(
            "<span>{} {}</span>",
            n_capturas,
            escapar(&r.capturas.to_lowercase())
        ));
    }
    if !c.marcadores.is_empty() {
        meta.push(format!(
            "<span>{} {}</span>",
            c.marcadores.len(),
            escapar(&r.marcadores.to_lowercase())
        ));
    }
    h.push_str(&meta.join("<span class=\"punto\" aria-hidden=\"true\">·</span>"));
    h.push_str("</p>\n</header>\n");

    // El reproductor: el video arriba (si lo hay) y la barra que se queda
    // pegada al subir, con el audio, la barra del tiempo y la busqueda.
    if let Some((_, v)) = &video {
        h.push_str(&format!(
            "<section class=\"pantalla\"><video id=\"video\" src=\"{}\" preload=\"metadata\" playsinline{}{}></video></section>\n",
            v.src(),
            if audio.is_some() { " muted" } else { " controls" },
            if audio.is_some() { "" } else { " data-maestro" }
        ));
    }
    h.push_str("<section class=\"barra-fija\" id=\"barra-fija\">\n");
    if let Some((_, a)) = &audio {
        h.push_str(&format!(
            "<audio id=\"audio\" src=\"{}\" preload=\"metadata\" controls data-maestro></audio>\n",
            a.src()
        ));
    }
    if con_medio {
        h.push_str(&format!(
            "<div class=\"mandos\" hidden>\
             <button type=\"button\" id=\"play\" class=\"play\" aria-label=\"{0}\" title=\"{0}\"><span class=\"i-play\" aria-hidden=\"true\"></span></button>\
             <span class=\"reloj\"><span id=\"ahora\">0:00</span> / <span id=\"total\">{1}</span></span>\
             <div class=\"linea-tiempo\" id=\"linea-tiempo\" role=\"slider\" tabindex=\"0\" aria-label=\"{2}\" aria-valuemin=\"0\" aria-valuemax=\"{3}\" aria-valuenow=\"0\">\
             <div class=\"riel\"><div class=\"hecho\" id=\"hecho\"></div></div>",
            escapar(&r.reproducir),
            minuto(duracion),
            escapar(&r.duracion),
            duracion / 1000
        ));
        let por = |ms: i64| {
            if duracion > 0 {
                (ms as f64 / duracion as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            }
        };
        for (i, (ms, _)) in c.capturas.iter().enumerate() {
            if capturas[i].is_none() {
                continue;
            }
            h.push_str(&format!(
                "<a class=\"senal captura\" href=\"#captura-{n}\" data-ms=\"{ms}\" style=\"left:{p:.3}%\" title=\"{} {n} · {}\"></a>",
                escapar(&r.captura),
                minuto(*ms),
                n = i + 1,
                p = por(*ms)
            ));
        }
        for m in &c.marcadores {
            let titulo = if m.texto.trim().is_empty() {
                format!("{} · {}", r.marcador, minuto(m.ms))
            } else {
                format!("{} · {}", minuto(m.ms), m.texto.trim())
            };
            h.push_str(&format!(
                "<a class=\"senal marcador\" href=\"#t-{ms}\" data-ms=\"{ms}\" style=\"left:{p:.3}%\" title=\"{}\"></a>",
                escapar(&titulo),
                ms = m.ms,
                p = por(m.ms)
            ));
        }
        h.push_str(&format!(
            "<div class=\"globo\" id=\"globo\" hidden></div></div>\
             <button type=\"button\" id=\"velocidad\" class=\"velocidad\" title=\"{}\">1×</button>\
             </div>\n",
            escapar(&r.velocidad)
        ));
    }
    if !c.lineas.is_empty() {
        h.push_str(&format!(
            "<div class=\"busqueda\"><input type=\"search\" id=\"buscar\" placeholder=\"{0}\" aria-label=\"{0}\" autocomplete=\"off\">{1}</div>\n",
            escapar(&r.buscar),
            if con_medio {
                format!(
                    "<label class=\"seguir\"><input type=\"checkbox\" id=\"seguir\" checked> {}</label>",
                    escapar(&r.seguir)
                )
            } else {
                String::new()
            }
        ));
    }
    // Al lado: los marcadores y el indice de las capturas. Sin ellos, el
    // texto a lo ancho (una nota de voz sin mas).
    let con_lado = !c.marcadores.is_empty() || n_capturas > 0;
    h.push_str(if con_lado {
        "</section>\n<main class=\"cuerpo con-lado\">\n"
    } else {
        "</section>\n<main class=\"cuerpo\">\n"
    });
    if con_lado {
        h.push_str("<aside class=\"lado\">\n");
        if !c.marcadores.is_empty() {
            h.push_str(&format!(
                "<section class=\"bloque\"><h2>{}</h2>\n<ol class=\"lista-marcadores\">\n",
                escapar(&r.marcadores)
            ));
            for m in &c.marcadores {
                let texto = if m.texto.trim().is_empty() {
                    escapar(&r.marcador)
                } else {
                    escapar(m.texto.trim())
                };
                h.push_str(&format!(
                    "<li data-ms=\"{}\">{} <span>{texto}</span></li>\n",
                    m.ms,
                    minuto_html(m.ms, con_medio)
                ));
            }
            h.push_str("</ol></section>\n");
        }
        if n_capturas > 0 {
            h.push_str(&format!(
                "<section class=\"bloque solo-pantalla\"><h2>{}</h2>\n<div class=\"miniaturas\" id=\"miniaturas\"></div></section>\n",
                escapar(&r.capturas)
            ));
        }
        h.push_str("</aside>\n");
    }

    h.push_str(&format!(
        "<article class=\"texto\" id=\"texto\">\n<h2>{}</h2>\n",
        escapar(&r.transcripcion)
    ));
    if c.lineas.is_empty() {
        h.push_str(&format!(
            "<p class=\"vacio\">{}</p>\n",
            escapar(&r.sin_texto)
        ));
    }
    for f in filas(c) {
        match f {
            Fila::Linea(i) => {
                let (ms, texto) = &c.lineas[i];
                match ms {
                    Some(ms) => h.push_str(&format!(
                        "<p class=\"linea\" id=\"t-{ms}\" data-ms=\"{ms}\">{}<span class=\"dicho\">{}</span></p>\n",
                        minuto_html(*ms, con_medio),
                        texto_con_saltos(texto)
                    )),
                    None => h.push_str(&format!(
                        "<p class=\"linea sin-tiempo\"><span class=\"dicho\">{}</span></p>\n",
                        texto_con_saltos(texto)
                    )),
                }
            }
            Fila::Captura(i) => {
                let Some(src) = &capturas[i] else {
                    continue;
                };
                let ms = c.capturas[i].0;
                let pie = format!("{} {} · {}", r.captura, i + 1, minuto(ms));
                h.push_str(&format!(
                    "<figure class=\"captura\" id=\"captura-{n}\" data-ms=\"{ms}\" data-n=\"{n}\">\
                     <button type=\"button\" class=\"ampliar\" aria-label=\"{pie}\"><img src=\"{src}\" alt=\"{pie}\" loading=\"lazy\" decoding=\"async\"></button>\
                     <figcaption>{} {} {n}</figcaption></figure>\n",
                    minuto_html(ms, con_medio),
                    escapar(&r.captura),
                    n = i + 1,
                    pie = escapar(&pie),
                ));
            }
            Fila::Marcador(i) => {
                let m = &c.marcadores[i];
                h.push_str(&format!(
                    "<p class=\"marca\" data-ms=\"{}\">{}<span class=\"cinta\">{}</span><span class=\"dicho\">{}</span></p>\n",
                    m.ms,
                    minuto_html(m.ms, con_medio),
                    escapar(&r.marcador),
                    escapar(m.texto.trim())
                ));
            }
        }
    }
    h.push_str("</article>\n</main>\n");

    // Lo que no va dentro (el video al lado, los adjuntos): para abrirlo.
    let mut fuera: Vec<(String, String)> = Vec::new();
    if let Some((ruta, Medio::AlLado(rel))) = &video {
        fuera.push((format!("{} · {}", r.video, nombre(ruta)), rel.clone()));
    }
    if let Some((ruta, Medio::AlLado(rel))) = &audio {
        fuera.push((format!("{} · {}", r.audio, nombre(ruta)), rel.clone()));
    }
    for a in &c.adjuntos {
        if let Some(m) = medio(a) {
            let src = match &m {
                Medio::Incrustado { .. } => m.src(),
                Medio::AlLado(rel) => rel.clone(),
            };
            fuera.push((nombre(a), src));
        }
    }
    if !fuera.is_empty() {
        h.push_str(&format!(
            "<footer class=\"adjuntos\"><h2>{}</h2><ul>\n",
            escapar(&r.adjuntos)
        ));
        for (n, src) in fuera {
            let src = if src.starts_with("data:") {
                src
            } else {
                url_relativa(&src)
            };
            h.push_str(&format!(
                "<li><a href=\"{src}\" download=\"{0}\">{0}</a></li>\n",
                escapar(&n)
            ));
        }
        h.push_str("</ul></footer>\n");
    }

    h.push_str(&format!(
        "<dialog id=\"lupa\" class=\"lupa\" aria-label=\"{0}\">\
         <figure><img id=\"lupa-img\" alt=\"\"><figcaption id=\"lupa-pie\"></figcaption></figure>\
         <div class=\"lupa-mandos\">\
         <button type=\"button\" id=\"lupa-antes\" aria-label=\"‹\">‹</button>\
         {1}\
         <button type=\"button\" id=\"lupa-despues\" aria-label=\"›\">›</button>\
         <button type=\"button\" id=\"lupa-cerrar\">{2}</button></div></dialog>\n",
        escapar(&r.capturas),
        if con_medio {
            format!(
                "<button type=\"button\" id=\"lupa-ir\">{}</button>",
                escapar(&r.ir_al_minuto)
            )
        } else {
            String::new()
        },
        escapar(&r.cerrar)
    ));
    h.push_str("<script>\n");
    h.push_str(JS);
    h.push_str("</script>\n</body>\n</html>\n");
    h
}

// --- Markdown y texto -----------------------------------------------------------

fn md_escapar(s: &str) -> String {
    let mut v = String::with_capacity(s.len());
    for (i, linea) in s.trim().lines().enumerate() {
        if i > 0 {
            v.push_str("  \n");
        }
        let linea = linea.trim_end();
        // Lo que al principio de un renglon cambiaria su forma.
        if linea.starts_with(['#', '>', '-', '+', '*', '|']) {
            v.push('\\');
        }
        for c in linea.chars() {
            if "\\`*_[]<>".contains(c) {
                v.push('\\');
            }
            v.push(c);
        }
    }
    v
}

fn cabecera_texto(c: &ClaseParaExportar, r: &Rotulos) -> Vec<String> {
    let mut v = Vec::new();
    if !c.fecha.trim().is_empty() {
        v.push(c.fecha.trim().to_string());
    }
    if c.duracion() > 0 {
        v.push(format!("{} {}", r.duracion, minuto(c.duracion())));
    }
    v
}

/// **La grabacion en Markdown**: el titulo, la fecha y la duracion, los
/// marcadores, y la transcripcion con las capturas en su minuto. `enlace`
/// da la ruta relativa de cada fichero (las capturas, el audio, el video),
/// que va escapada como URL y sin `<>` (el lector de fotos de las notas, y
/// con el el Word, no las admite); `None`, no se enlaza.
pub fn markdown(
    c: &ClaseParaExportar,
    r: &Rotulos,
    enlace: &dyn Fn(&Path) -> Option<String>,
) -> String {
    let mut v = format!("# {}\n\n", md_escapar(&c.titulo));
    let cab = cabecera_texto(c, r);
    if !cab.is_empty() {
        v.push_str(&format!("*{}*\n\n", md_escapar(&cab.join(" · "))));
    }
    let mut medios = Vec::new();
    for (rotulo, ruta) in [(&r.audio, &c.audio), (&r.video, &c.video)] {
        if let Some(ruta) = ruta
            && let Some(e) = enlace(ruta)
        {
            medios.push(format!(
                "- {}: [{}]({})",
                rotulo,
                md_escapar(&nombre(ruta)),
                url_relativa(&e)
            ));
        }
    }
    for a in &c.adjuntos {
        if let Some(e) = enlace(a) {
            medios.push(format!(
                "- [{}]({})",
                md_escapar(&nombre(a)),
                url_relativa(&e)
            ));
        }
    }
    if !medios.is_empty() {
        v.push_str(&medios.join("\n"));
        v.push_str("\n\n");
    }
    if !c.marcadores.is_empty() {
        v.push_str(&format!("## {}\n\n", r.marcadores));
        for m in &c.marcadores {
            let texto = if m.texto.trim().is_empty() {
                r.marcador.clone()
            } else {
                md_escapar(&m.texto)
            };
            v.push_str(&format!("- **{}** {}\n", minuto(m.ms), texto));
        }
        v.push('\n');
    }
    v.push_str(&format!("## {}\n\n", r.transcripcion));
    if c.lineas.is_empty() {
        v.push_str(&format!("*{}*\n\n", r.sin_texto));
    }
    for f in filas(c) {
        match f {
            Fila::Linea(i) => {
                let (ms, texto) = &c.lineas[i];
                match ms {
                    Some(ms) => {
                        v.push_str(&format!("**{}** {}\n\n", minuto(*ms), md_escapar(texto)))
                    }
                    None => v.push_str(&format!("{}\n\n", md_escapar(texto))),
                }
            }
            Fila::Captura(i) => {
                let (ms, ruta) = &c.capturas[i];
                if let Some(e) = enlace(ruta) {
                    v.push_str(&format!(
                        "![{} {} · {}]({})\n\n",
                        r.captura,
                        i + 1,
                        minuto(*ms),
                        url_relativa(&e)
                    ));
                }
            }
            Fila::Marcador(i) => {
                let m = &c.marcadores[i];
                v.push_str(&format!(
                    "> **{} · {}** {}\n\n",
                    r.marcador,
                    minuto(m.ms),
                    md_escapar(&m.texto)
                ));
            }
        }
    }
    v.trim_end().to_string() + "\n"
}

/// El nombre de la captura `i` dentro de la carpeta de la grabacion:
/// `captura-01-1m05s.png`, que se ordena solo y dice su minuto.
pub fn nombre_de_captura(i: usize, ms: i64, ruta: &Path) -> String {
    let s = ms.max(0) / 1000;
    let ext = match extension(ruta).as_str() {
        "" => "png".to_string(),
        e => e.to_string(),
    };
    format!("captura-{:02}-{}m{:02}s.{ext}", i + 1, s / 60, s % 60)
}

/// **El Markdown con su carpeta, en un ZIP**: `<base>.md` y, al lado,
/// `<base>_archivos/` con las capturas (lo que exporta Notion o Obsidian).
/// `bytes` lee cada captura; la que no se lee no se enlaza.
pub fn paquete_markdown(
    c: &ClaseParaExportar,
    r: &Rotulos,
    base: &str,
    bytes: &dyn Fn(&Path) -> Option<Vec<u8>>,
) -> Result<Vec<u8>, crate::ErrorProyecto> {
    use std::io::Write;
    let carpeta = format!("{base}_archivos");
    let mut dentro: Vec<(String, Vec<u8>)> = Vec::new();
    let mut nombres: Vec<(PathBuf, String)> = Vec::new();
    for (i, (ms, ruta)) in c.capturas.iter().enumerate() {
        if let Some(b) = bytes(ruta) {
            let n = format!("{carpeta}/{}", nombre_de_captura(i, *ms, ruta));
            nombres.push((ruta.clone(), n.clone()));
            dentro.push((n, b));
        }
    }
    let md = markdown(c, r, &|ruta: &Path| {
        nombres
            .iter()
            .find(|(r, _)| r == ruta)
            .map(|(_, n)| n.clone())
    });
    let mut salida = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut salida));
        let o: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let io = |e: std::io::Error| crate::ErrorProyecto::Io(e, base.to_string());
        zip.start_file(format!("{base}.md"), o)?;
        zip.write_all(md.as_bytes()).map_err(io)?;
        for (n, b) in &dentro {
            // Una imagen ya va comprimida: guardada tal cual.
            zip.start_file(
                n.as_str(),
                o.compression_method(zip::CompressionMethod::Stored),
            )?;
            zip.write_all(b).map_err(io)?;
        }
        zip.finish()?;
    }
    Ok(salida)
}

/// **La transcripcion en texto**: el titulo, la fecha y la duracion, y cada
/// linea con su minuto; los marcadores, en su sitio.
pub fn texto(c: &ClaseParaExportar, r: &Rotulos) -> String {
    let mut v = format!("{}\n", c.titulo.trim());
    let cab = cabecera_texto(c, r);
    if !cab.is_empty() {
        v.push_str(&cab.join(" · "));
        v.push('\n');
    }
    v.push('\n');
    if c.lineas.is_empty() {
        v.push_str(&r.sin_texto);
        v.push_str("\n\n");
    }
    for f in filas(c) {
        match f {
            Fila::Linea(i) => {
                let (ms, texto) = &c.lineas[i];
                match ms {
                    Some(ms) => v.push_str(&format!("[{}] {}\n\n", minuto(*ms), texto.trim())),
                    None => v.push_str(&format!("{}\n\n", texto.trim())),
                }
            }
            Fila::Marcador(i) => {
                let m = &c.marcadores[i];
                let texto = m.texto.trim();
                if texto.is_empty() {
                    v.push_str(&format!("[{}] -- {}\n\n", minuto(m.ms), r.marcador));
                } else {
                    v.push_str(&format!(
                        "[{}] -- {}: {}\n\n",
                        minuto(m.ms),
                        r.marcador,
                        texto
                    ));
                }
            }
            Fila::Captura(_) => {}
        }
    }
    v.trim_end().to_string() + "\n"
}

#[cfg(test)]
mod pruebas;
