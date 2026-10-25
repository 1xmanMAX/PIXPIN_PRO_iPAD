//! **La valla de archivos de los chats compartidos** (Android 13-oct-2026).
//!
//! Lo que llega de una persona para un chat compartido solo puede tocar ese
//! chat: un mensaje que se hace pasar por uno de otro chat (mismo codigo
//! unico) o que nombra un archivo de otro chat (un mensaje trampa que senale
//! `pins/draw/privado…`) se tira sin decir nada; un proyecto que no es el de
//! ese chat, igual; y si una persona borra un mensaje, no se lleva un archivo
//! que tambien es de otro chat. Puerto de `Disco.soloDeEsteChat`,
//! `Disco.proyectoSeguro`, `Disco.referencias` y del `noBorrar` de
//! `Disco.aplicarMensajes`.
//!
//! «De otro chat» lo decide quien llama con `vedado(rel)`: en el PC va por
//! el fichero de verdad al que lleva la ruta (ver
//! [crate::compartidos::AccesoDePersona]).

use std::collections::{BTreeMap, HashSet};
use std::io;
use std::path::PathBuf;

use crate::canonico::Json;
use crate::disco::{Disco, GENERAL, PORTATIL, en_texto, limpio};
use crate::kotlin;

/// **Que archivos nombra un mensaje**, existan o no aqui
/// (`Disco.referencias(m)`): su adjunto, las rutas de dentro, el lienzo, la
/// tabla o el croquis que senala, sus tablas de libro y lo anotado con su
/// codigo.
pub fn referencias_de_mensaje<D: Disco + ?Sized>(
    d: &D,
    chat: &str,
    m: &Json,
    anotado: &BTreeMap<String, Vec<String>>,
) -> HashSet<String> {
    let mut salida: HashSet<String> = en_texto(&m.a_texto()).into_iter().collect();
    // La ruta del adjunto ENTERA: `en_texto` corta en `)`.
    if let Some(r) = kotlin::cadena(m, "ruta").and_then(|r| r.strip_prefix(PORTATIL)) {
        salida.insert(r.to_string());
    }
    let referencia = kotlin::cadena(m, "referencia");
    let id = kotlin::cadena(m, "id").unwrap_or_default();
    match kotlin::cadena(m, "clase").unwrap_or_default() {
        "DIBUJO" | "PAGINA" => {
            if let Some(r) = referencia {
                salida.insert(format!("pins/draw/{r}.excalidraw.gz"));
            }
        }
        "IMAGEN" => {
            let foto = format!("foto-{id}");
            salida.insert(format!(
                "pins/draw/{}.excalidraw.gz",
                referencia.unwrap_or(&foto)
            ));
        }
        "TABLA" => {
            if let Some(r) = referencia {
                salida.insert(format!("tablas/{}.json", limpio(r)));
            }
        }
        "CROQUIS" => {
            if let Some(r) = referencia {
                salida.insert(format!("croquis3d/{r}.croquis.gz"));
            }
        }
        "ARCHIVO" => {
            let mut i = 0;
            loop {
                let rel = format!("tablas/{}.json", limpio(&format!("libro-{id}-{i}")));
                if !d.ruta(chat, &rel).exists() {
                    break;
                }
                salida.insert(rel);
                i += 1;
            }
        }
        _ => {}
    }
    if let Some(l) = anotado.get(&kotlin::unico(m)) {
        salida.extend(l.iter().cloned());
    }
    salida
}

/// Lo mismo de un proyecto (`Disco.referencias(p)`): su documento, sus hojas
/// y lo anotado con su codigo o el de sus hojas.
pub fn referencias_de_proyecto(
    p: &Json,
    anotado: &BTreeMap<String, Vec<String>>,
) -> HashSet<String> {
    let mut salida: HashSet<String> = en_texto(&p.a_texto()).into_iter().collect();
    for campo in ["pdfOrigen", "pdfLimpio"] {
        if let Some(r) = kotlin::cadena(p, campo).and_then(|r| r.strip_prefix(PORTATIL)) {
            salida.insert(r.to_string());
        }
    }
    if let Some(l) = anotado.get(&kotlin::unico_de_proyecto(p)) {
        salida.extend(l.iter().cloned());
    }
    let lista = |k: &str| {
        p.como_objeto()
            .and_then(|o| o.obtener(k))
            .and_then(Json::como_lista)
            .map(<[Json]>::to_vec)
            .unwrap_or_default()
    };
    for h in lista("hojas") {
        if let Some(x) = kotlin::cadena(&h, "dibujo") {
            salida.insert(format!("pins/draw/{x}.excalidraw.gz"));
        }
        if let Some(x) = kotlin::cadena(&h, "tabla") {
            salida.insert(format!("tablas/{}.json", limpio(x)));
        }
        if let Some(x) = kotlin::cadena(&h, "croquis") {
            salida.insert(format!("croquis3d/{x}.croquis.gz"));
        }
        if let Some(l) = anotado.get(&kotlin::unico_de_hoja(&h)) {
            salida.extend(l.iter().cloned());
        }
    }
    for c in lista("croquis") {
        if let Some(x) = c.como_cadena() {
            salida.insert(format!("croquis3d/{x}.croquis.gz"));
        }
    }
    salida
}

/// Los codigos unicos de los mensajes de los chats que no son `chat`
/// (`Disco.unicosFueraDe`).
pub fn unicos_fuera_de<D: Disco + ?Sized>(d: &D, chat: &str) -> HashSet<String> {
    let mut chats: Vec<String> = d
        .chats()
        .map(|l| l.into_iter().map(|c| c.id).collect())
        .unwrap_or_default();
    if !chats.iter().any(|c| c == GENERAL) {
        chats.push(GENERAL.into());
    }
    chats
        .iter()
        .filter(|c| *c != chat)
        .flat_map(|c| d.mensajes(c).unwrap_or_default())
        .map(|m| kotlin::unico(&m))
        .collect()
}

/// **De lo que manda una persona para `chat`, lo que se puede poner**
/// (`Disco.soloDeEsteChat`, JSON portatil): mensajes de ese chat que no se
/// hacen pasar por uno de otro chat ni nombran nada vedado. Lo demas se tira
/// sin decir nada: un PixPin honrado nunca lo manda.
pub fn solo_de_este_chat<D: Disco + ?Sized>(
    d: &D,
    chat: &str,
    poner: &[String],
    vedado: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    if poner.is_empty() {
        return Vec::new();
    }
    let ajenos = unicos_fuera_de(d, chat);
    let lista = d.anotado(chat);
    let anotado = crate::anotado::por_uid(lista.iter().map(String::as_str));
    poner
        .iter()
        .filter(|j| {
            let Some(m) = kotlin::mensaje_de_texto(j) else {
                return false;
            };
            kotlin::chat_de(&m) == chat
                && !ajenos.contains(&kotlin::unico(&m))
                && !referencias_de_mensaje(d, chat, &m, &anotado)
                    .iter()
                    .any(|r| vedado(r))
        })
        .cloned()
        .collect()
}

/// El proyecto que manda una persona, si es el de `chat` y no nombra nada
/// vedado (`Disco.proyectoSeguro`).
pub fn proyecto_seguro<D: Disco + ?Sized>(
    d: &D,
    chat: &str,
    p: Option<Json>,
    vedado: &dyn Fn(&str) -> bool,
) -> Option<Json> {
    let p = p?;
    if kotlin::cadena(&p, "id") != Some(chat) {
        return None;
    }
    let lista = d.anotado(chat);
    let anotado = crate::anotado::por_uid(lista.iter().map(String::as_str));
    if referencias_de_proyecto(&p, &anotado)
        .iter()
        .any(|r| vedado(r))
    {
        return None;
    }
    Some(p)
}

/// **Pone y quita mensajes sin llevarse lo de otros chats**
/// (`Disco.aplicarMensajes(..., noBorrar)`): los ficheros de `no_borrar` que
/// nombran los mensajes que se van se quedan aunque el mensaje se vaya. El
/// `Disco` de cada aparato borra el adjunto a su manera; aqui se le deja
/// hacer y lo protegido se aparta antes (un enlace duro, o una copia) y se
/// devuelve a su sitio despues.
pub fn aplicar_mensajes_guardando<D: Disco + ?Sized>(
    d: &D,
    chat: &str,
    poner: &[String],
    borrar: &[String],
    ahora: i64,
    no_borrar: &HashSet<PathBuf>,
) -> io::Result<()> {
    if no_borrar.is_empty() || borrar.is_empty() {
        return d.aplicar_mensajes(chat, poner, borrar, ahora);
    }
    let quitar: HashSet<&String> = borrar.iter().collect();
    let lista = d.anotado(chat);
    let anotado = crate::anotado::por_uid(lista.iter().map(String::as_str));
    let mut apartados: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (k, m) in d.mensajes_por_clave(chat)? {
        if !quitar.contains(&k) {
            continue;
        }
        for rel in referencias_de_mensaje(d, chat, &m, &anotado) {
            let f = d.ruta(chat, &rel);
            if !no_borrar.contains(&f) || !f.is_file() || apartados.iter().any(|(x, _)| *x == f) {
                continue;
            }
            let nombre = f
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let aparte = f.with_file_name(format!(".{nombre}.de-otro-chat"));
            let _ = std::fs::remove_file(&aparte);
            if std::fs::hard_link(&f, &aparte).is_ok() || std::fs::copy(&f, &aparte).is_ok() {
                apartados.push((f, aparte));
            }
        }
    }
    let r = d.aplicar_mensajes(chat, poner, borrar, ahora);
    for (f, aparte) in apartados {
        if f.exists() {
            let _ = std::fs::remove_file(&aparte);
        } else {
            let _ = std::fs::rename(&aparte, &f);
        }
    }
    r
}
