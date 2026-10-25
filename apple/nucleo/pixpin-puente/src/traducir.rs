//! **De un mensaje de la extension a un pedido de PixPin.**
//!
//! La extension dice QUE se eligio en el menu («tarea», «ocr»,
//! «captura_pagina»…) con lo que habia en la pagina (texto seleccionado,
//! direccion, titulo, la imagen ya leida). Aqui se decide COMO se le pide a
//! PixPin, con las acciones que ya existen (`crate::mensajero::ACCIONES`):
//! asi la extension no sabe nada del protocolo de pedidos y un cambio en el
//! se arregla en un solo sitio, en Rust y con pruebas.
//!
//! Pura: no escribe nada. Devuelve el pedido y los ficheros que hay que dejar
//! en la cache ANTES de mandarlo (la imagen, o la pagina para el movil).

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::base64;

/// Lo que manda la extension (version 1).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Mensaje {
    /// La version del mensaje; `0` (sin poner) cuenta como 1.
    pub v: u32,
    pub accion: String,
    /// El texto seleccionado.
    pub texto: String,
    /// Lo que se le pregunta al asistente sobre `texto`.
    pub pregunta: String,
    /// La direccion de la pagina y su titulo.
    pub url: String,
    pub titulo: String,
    /// La imagen elegida o la captura de la pagina, ya leida.
    pub imagen: Option<Imagen>,
    /// El `id` del movil, para `pagina_al_movil`; sin el, a todos.
    pub aparato: Option<String>,
}

/// Una imagen en base64 con su tipo MIME.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Imagen {
    pub tipo: String,
    pub base64: String,
}

/// Lo que hay que hacer: dejar `ficheros` en el disco y mandar `pedido`.
#[derive(Debug)]
pub struct Plan {
    pub ficheros: Vec<(PathBuf, Vec<u8>)>,
    pub pedido: String,
    /// El texto no cabia en un pedido (64 KB) y se corto.
    pub recortado: bool,
    /// Lo que se le dice al usuario si todo va bien.
    pub hecho: &'static str,
}

/// Las acciones que entiende el puente, con lo que dicen al acabar. Una
/// tabla (como `pixpin_ia::acciones`) y no un `match` suelto: la extension
/// tiene la suya y una prueba en Node comprueba que coinciden.
pub const ACCIONES: [(&str, &str); 12] = [
    ("tarea", "Tarea apuntada en PixPin"),
    ("nota", "Nota abierta en PixPin"),
    ("chat", "Enviado al chat de PixPin"),
    ("leccion", "Lección abierta en PixPin"),
    ("leer", "PixPin lo lee en voz alta"),
    ("asistente", "Preguntado al asistente de PixPin"),
    ("pinear_imagen", "Imagen pineada"),
    ("imagen_al_chat", "Imagen enviada al chat de PixPin"),
    ("ocr", "PixPin lee el texto de la imagen"),
    ("captura_pagina", "Página entera enviada al chat de PixPin"),
    ("enlace_al_chat", "Enlace enviado al chat de PixPin"),
    ("pagina_al_movil", "Enviando la página al móvil"),
];

/// Lo que se le pregunta al asistente si no se escribio nada.
pub const PREGUNTA_POR_DEFECTO: &str = "Explícame este texto";

/// Traduce. `carpeta` es donde van los ficheros (`<raiz>\cache\navegador`)
/// y `sello` un nombre unico para ellos (milisegundos y proceso).
pub fn traducir(m: &Mensaje, carpeta: &Path, sello: &str) -> Result<Plan, String> {
    if m.v > 1 {
        return Err("Esta versión de PixPin no entiende la extensión: actualiza PixPin".into());
    }
    let Some(&(_, hecho)) = ACCIONES.iter().find(|(a, _)| *a == m.accion) else {
        return Err(format!("Acción desconocida: «{}»", m.accion));
    };
    let texto = m.texto.trim();
    let url = m.url.trim();
    let titulo = una_linea(&m.titulo);
    let pide_texto = matches!(
        m.accion.as_str(),
        "tarea" | "nota" | "chat" | "leccion" | "leer" | "asistente"
    );
    if pide_texto && texto.is_empty() {
        return Err("No hay texto seleccionado".into());
    }
    let mut ficheros: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    let mut imagen = || -> Result<PathBuf, String> {
        let img = m.imagen.as_ref().ok_or("No llegó la imagen")?;
        let (bytes, ext) = imagen_valida(img)?;
        let ruta = carpeta.join(format!("nav-{sello}.{ext}"));
        ficheros.push((ruta.clone(), bytes));
        Ok(ruta)
    };
    let mut otro_fichero: Option<(PathBuf, Vec<u8>)> = None;
    let fuente = fuente(&titulo, url);
    let pedido = match m.accion.as_str() {
        "tarea" => {
            let mut t = una_linea(texto);
            if !url.is_empty() {
                t = format!("{t} — {url}");
            }
            json!({"accion": "anadir_tarea", "texto": t})
        }
        "nota" => {
            json!({"accion": "nota_nueva", "texto": con_pie(texto, &markdown_de(&titulo, url))})
        }
        "chat" => json!({"accion": "chat", "texto": con_pie(texto, &fuente)}),
        "leccion" => {
            let pie = if fuente.is_empty() {
                String::new()
            } else {
                format!("Fuente: {}", fuente.replace('\n', " "))
            };
            json!({"accion": "leccion_nueva", "texto": con_pie(texto, &pie)})
        }
        "leer" => json!({"accion": "leer_en_voz", "texto": texto, "titulo": titulo}),
        "asistente" => {
            let pregunta = match m.pregunta.trim() {
                "" => PREGUNTA_POR_DEFECTO,
                p => p,
            };
            let de = match (titulo.is_empty(), url.is_empty()) {
                (true, true) => String::new(),
                (false, true) => format!("(De «{titulo}»)"),
                (true, false) => format!("(De {url})"),
                (false, false) => format!("(De «{titulo}», {url})"),
            };
            json!({"accion": "asistente", "texto": con_pie(&format!("{pregunta}\n\n«{texto}»"), &de)})
        }
        "pinear_imagen" => json!({"accion": "pinear", "ruta": imagen()?}),
        "ocr" => json!({"accion": "ocr", "ruta": imagen()?}),
        "imagen_al_chat" | "captura_pagina" => {
            json!({"accion": "chat", "texto": fuente, "imagenes": [imagen()?]})
        }
        "enlace_al_chat" => {
            if fuente.is_empty() {
                return Err("La página no tiene dirección".into());
            }
            json!({"accion": "chat", "texto": fuente})
        }
        "pagina_al_movil" => {
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return Err("Solo se mandan al móvil páginas http o https".into());
            }
            // El movil recibe ficheros, no enlaces: una pagina diminuta con
            // el nombre de la pagina, que su visor de HTML abre y lleva a la
            // direccion (y que se puede tocar si no salta sola).
            let nombre = nombre_de_fichero(if titulo.is_empty() { url } else { &titulo });
            let ruta = carpeta
                .join(format!("nav-{sello}"))
                .join(format!("{nombre}.html"));
            otro_fichero = Some((ruta.clone(), pagina_de_enlace(&titulo, url).into_bytes()));
            let aparato = m
                .aparato
                .as_deref()
                .map(str::trim)
                .filter(|a| !a.is_empty())
                .unwrap_or("todos");
            json!({"accion": "enviar_al_movil", "aparato": aparato, "archivos": [ruta]})
        }
        _ => unreachable!("la tabla de arriba ya filtro la accion"),
    };
    ficheros.extend(otro_fichero);
    let (pedido, recortado) = a_texto(pedido)?;
    Ok(Plan {
        ficheros,
        pedido,
        recortado,
        hecho,
    })
}

/// El JSON del pedido con `"pixpin": 1`, cortando `texto` hasta que quepa en
/// [`crate::mensajero::TOPE_PEDIDO`]. Una seleccion de un libro
/// entero no deberia dejar al usuario sin nada: se manda lo que cabe y se
/// dice.
fn a_texto(mut pedido: Value) -> Result<(String, bool), String> {
    pedido["pixpin"] = json!(crate::mensajero::VERSION_PEDIDO);
    let mut recortado = false;
    loop {
        let s = pedido.to_string();
        if s.len() <= crate::mensajero::TOPE_PEDIDO {
            debug_assert_eq!(
                crate::mensajero::validar_pedido(&s),
                crate::mensajero::respuesta::ACEPTADO
            );
            return Ok((s, recortado));
        }
        let Some(t) = pedido.get("texto").and_then(Value::as_str) else {
            return Err("El pedido no cabe".into());
        };
        let cuantos = t.chars().count();
        if cuantos < 16 {
            return Err("El pedido no cabe".into());
        }
        let mut nuevo: String = t.chars().take(cuantos * 3 / 4).collect();
        nuevo.push('…');
        pedido["texto"] = json!(nuevo);
        recortado = true;
    }
}

/// Los blancos juntados en una sola linea.
fn una_linea(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// «Titulo\nurl», sin lo que falte.
fn fuente(titulo: &str, url: &str) -> String {
    [titulo, url]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

/// El enlace en Markdown para una nota (las notas de PixPin lo son).
fn markdown_de(titulo: &str, url: &str) -> String {
    match (titulo.is_empty(), url.is_empty()) {
        (_, true) => titulo.to_string(),
        (true, false) => format!("<{url}>"),
        (false, false) => format!("[{}]({url})", titulo.replace(['[', ']'], "")),
    }
}

/// El texto y, si hay, su pie separado por una linea en blanco.
fn con_pie(texto: &str, pie: &str) -> String {
    if pie.is_empty() {
        texto.to_string()
    } else {
        format!("{texto}\n\n{pie}")
    }
}

/// Los bytes de la imagen y su extension, si de verdad es lo que dice. Se
/// miran los primeros bytes: lo que se escribe en el disco del usuario tiene
/// que ser una imagen, no lo que una pagina quiera colar con otro nombre.
fn imagen_valida(img: &Imagen) -> Result<(Vec<u8>, &'static str), String> {
    let ext = match img.tipo.trim().to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        otro => return Err(format!("Formato de imagen no admitido: {otro}")),
    };
    let bytes = base64::decodificar(&img.base64).map_err(|e| format!("Imagen rota: {e}"))?;
    let cuadra = match ext {
        "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "webp" => bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        _ => bytes.starts_with(b"BM"),
    };
    if !cuadra {
        return Err(format!(
            "La imagen no es un {} de verdad",
            ext.to_uppercase()
        ));
    }
    Ok((bytes, ext))
}

/// Un nombre de fichero de Windows a partir de un titulo: sin los
/// caracteres prohibidos, sin puntos ni blancos al final y no muy largo.
pub fn nombre_de_fichero(titulo: &str) -> String {
    let limpio: String = titulo
        .chars()
        .map(|c| {
            if c.is_control() || r#"<>:"/\|?*"#.contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let limpio = una_linea(&limpio);
    let corto: String = limpio.chars().take(80).collect();
    let corto = corto.trim_end_matches(['.', ' ']).to_string();
    // Los nombres reservados de Windows (CON, NUL, COM1…) no se pueden crear.
    let reservado = {
        let base = corto.split('.').next().unwrap_or("").to_ascii_uppercase();
        matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ((base.starts_with("COM") || base.starts_with("LPT"))
                && base.len() == 4
                && base.as_bytes()[3].is_ascii_digit())
    };
    if corto.is_empty() || reservado {
        "pagina".into()
    } else {
        corto
    }
}

/// La pagina que se manda al movil: lleva a `url` sola y, si no, con un toque.
pub fn pagina_de_enlace(titulo: &str, url: &str) -> String {
    let t = escapar(if titulo.is_empty() { url } else { titulo });
    let u = escapar(url);
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width\">\
         <meta http-equiv=\"refresh\" content=\"0;url={u}\"><title>{t}</title></head>\n\
         <body style=\"font-family:sans-serif;padding:1em\"><p><a href=\"{u}\">{t}</a></p>\
         <p style=\"color:#666\">{u}</p></body></html>\n"
    )
}

fn escapar(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
