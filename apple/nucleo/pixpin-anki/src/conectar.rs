//! **AnkiConnect** (<https://foosoft.net/projects/anki-connect/>): el
//! complemento de Anki que escucha en `127.0.0.1:8765` y contesta JSON. Con
//! Anki abierto, las tarjetas entran directas en su mazo y sus imagenes en la
//! carpeta de medios del perfil.
//!
//! El cliente HTTP va a mano con `std`: es un `POST` a `localhost` y nada
//! mas, sin TLS ni redirecciones, y no merece un crate de red.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use base64::Engine;
use serde_json::{Value, json};

use crate::Tarjeta;

/// Donde escucha AnkiConnect de fabrica.
pub const DIRECCION: &str = "127.0.0.1:8765";
/// La version de su API que se habla.
const VERSION: u32 = 6;

/// Por que no se pudo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fallo {
    /// No contesta nadie: Anki cerrado o sin AnkiConnect.
    SinAnki,
    /// AnkiConnect contesto con un error (lo que dijo).
    Anki(String),
    /// La respuesta no se entiende.
    Respuesta(String),
}

impl std::fmt::Display for Fallo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fallo::SinAnki => write!(f, "Anki no esta abierto o no tiene AnkiConnect"),
            Fallo::Anki(e) => write!(f, "Anki dijo: {e}"),
            Fallo::Respuesta(e) => write!(f, "respuesta de Anki que no se entiende: {e}"),
        }
    }
}

impl std::error::Error for Fallo {}

/// **Llama a AnkiConnect**: `accion` con sus `parametros`, y devuelve su
/// `result`.
pub fn llamar(accion: &str, parametros: Value) -> Result<Value, Fallo> {
    let cuerpo = json!({ "action": accion, "version": VERSION, "params": parametros }).to_string();
    let respuesta = post(&cuerpo)?;
    let v: Value = serde_json::from_str(&respuesta).map_err(|e| Fallo::Respuesta(e.to_string()))?;
    match v.get("error") {
        Some(Value::Null) | None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
        Some(e) => Err(Fallo::Anki(
            e.as_str().unwrap_or(&e.to_string()).to_string(),
        )),
    }
}

fn post(cuerpo: &str) -> Result<String, Fallo> {
    let direccion: SocketAddr = DIRECCION.parse().map_err(|_| Fallo::SinAnki)?;
    let mut s = TcpStream::connect_timeout(&direccion, Duration::from_millis(800))
        .map_err(|_| Fallo::SinAnki)?;
    // Guardar muchas imagenes tarda: se espera de sobra.
    let _ = s.set_read_timeout(Some(Duration::from_secs(30)));
    let pedido = format!(
        "POST / HTTP/1.1\r\nHost: {DIRECCION}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
        cuerpo.len()
    );
    s.write_all(pedido.as_bytes()).map_err(|_| Fallo::SinAnki)?;
    let mut crudo = Vec::new();
    s.read_to_end(&mut crudo)
        .map_err(|e| Fallo::Respuesta(e.to_string()))?;
    cuerpo_de(&crudo)
}

/// El cuerpo de una respuesta HTTP/1.1 (con o sin `chunked`).
fn cuerpo_de(crudo: &[u8]) -> Result<String, Fallo> {
    let texto = String::from_utf8_lossy(crudo);
    let (cabeza, cuerpo) = texto
        .split_once("\r\n\r\n")
        .ok_or_else(|| Fallo::Respuesta("sin cuerpo".into()))?;
    if !cabeza.starts_with("HTTP/1.") || !cabeza.split(' ').nth(1).is_some_and(|c| c == "200") {
        return Err(Fallo::Respuesta(
            cabeza.lines().next().unwrap_or("").to_string(),
        ));
    }
    if cabeza
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        let mut salida = String::new();
        let mut resto = cuerpo;
        while let Some((tam, tras)) = resto.split_once("\r\n") {
            let n = usize::from_str_radix(tam.trim(), 16)
                .map_err(|_| Fallo::Respuesta("trozo mal medido".into()))?;
            if n == 0 || tras.len() < n {
                break;
            }
            salida.push_str(&tras[..n]);
            resto = tras[n..].trim_start_matches("\r\n");
        }
        return Ok(salida);
    }
    Ok(cuerpo.to_string())
}

/// Si AnkiConnect contesta.
pub fn hay_anki() -> bool {
    llamar("version", json!({})).is_ok()
}

/// **El tipo de nota basico** de este Anki y sus dos campos: el nombre
/// cambia con el idioma con que se creo el perfil («Basic», «Básico»,
/// «Basique»…), asi que se busca entre los que hay uno de dos campos.
fn modelo_basico() -> Result<(String, String, String), Fallo> {
    let nombres: Vec<String> = serde_json::from_value(llamar("modelNames", json!({}))?)
        .map_err(|e| Fallo::Respuesta(e.to_string()))?;
    let preferidos = ["Basic", "Básico", "Basico", "Basique", "Einfach", "Base"];
    let mut orden: Vec<&String> = nombres
        .iter()
        .filter(|n| preferidos.contains(&n.as_str()))
        .collect();
    orden.extend(nombres.iter().filter(|n| !preferidos.contains(&n.as_str())));
    for n in orden {
        let campos: Vec<String> =
            serde_json::from_value(llamar("modelFieldNames", json!({ "modelName": n }))?)
                .map_err(|e| Fallo::Respuesta(e.to_string()))?;
        if campos.len() == 2 {
            return Ok((n.clone(), campos[0].clone(), campos[1].clone()));
        }
    }
    Err(Fallo::Anki("no hay ningun tipo de nota basico".into()))
}

/// **Mete las tarjetas en `mazo`** (lo crea si no esta), con sus imagenes.
/// Devuelve cuantas entraron; las repetidas (mismo frente en el mazo) se
/// saltan sin fallar.
pub fn anadir(mazo: &str, tarjetas: &[Tarjeta]) -> Result<usize, Fallo> {
    if tarjetas.is_empty() {
        return Ok(0);
    }
    llamar("createDeck", json!({ "deck": mazo }))?;
    for t in tarjetas {
        for i in &t.imagenes {
            let datos = base64::engine::general_purpose::STANDARD.encode(&i.bytes);
            llamar(
                "storeMediaFile",
                json!({ "filename": i.nombre, "data": datos }),
            )?;
        }
    }
    let (modelo, frente, reverso) = modelo_basico()?;
    let notas: Vec<Value> = tarjetas
        .iter()
        .map(|t| {
            json!({
                "deckName": mazo,
                "modelName": modelo,
                "fields": { frente.clone(): t.frente, reverso.clone(): t.reverso },
                "tags": t.etiquetas,
                "options": { "allowDuplicate": false, "duplicateScope": "deck" },
            })
        })
        .collect();
    // `canAddNotes` primero: `addNotes` falla entero si una es repetida.
    let se_pueden: Vec<bool> =
        serde_json::from_value(llamar("canAddNotes", json!({ "notes": notas }))?)
            .map_err(|e| Fallo::Respuesta(e.to_string()))?;
    let nuevas: Vec<Value> = notas
        .into_iter()
        .zip(se_pueden)
        .filter_map(|(n, si)| si.then_some(n))
        .collect();
    if nuevas.is_empty() {
        return Ok(0);
    }
    let ids = llamar("addNotes", json!({ "notes": nuevas }))?;
    Ok(ids
        .as_array()
        .map_or(0, |v| v.iter().filter(|i| !i.is_null()).count()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_cuerpo_de_una_respuesta_entera_y_por_trozos() {
        let r = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"result\": 6, \"error\": null}";
        assert_eq!(cuerpo_de(r).unwrap(), "{\"result\": 6, \"error\": null}");
        let t = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhola \r\n5\r\nmundo\r\n0\r\n\r\n";
        assert_eq!(cuerpo_de(t).unwrap(), "hola mundo");
        // Caso negativo: un 403 (AnkiConnect con otra direccion permitida)
        // es un fallo, no un cuerpo vacio.
        assert!(cuerpo_de(b"HTTP/1.1 403 Forbidden\r\n\r\n").is_err());
    }

    #[test]
    fn sin_nadie_escuchando_se_dice_que_no_hay_anki() {
        // En la CI no hay Anki: tiene que contestar rapido y sin colgarse.
        if !hay_anki() {
            assert_eq!(llamar("version", json!({})), Err(Fallo::SinAnki));
        }
    }
}
