//! **JSON-RPC 2.0 de MCP**, a mano sobre serde_json.
//!
//! Lo que pide la especificacion (revision 2025-06-18, compatible con la
//! 2025-11-25 y las anteriores) para un servidor que solo ofrece
//! herramientas:
//!
//! - `initialize`: se contesta con la version del protocolo (la del cliente
//!   si la conocemos; si no, la mas nueva nuestra), `capabilities.tools` y
//!   `serverInfo`. Despues llega la notificacion `notifications/initialized`,
//!   que no se contesta (ninguna notificacion se contesta).
//! - `ping`: `{}`.
//! - `tools/list`: todas, sin paginar (son unas cuarenta).
//! - `tools/call`: el resultado lleva `content` (texto o imagen) e
//!   `isError`. Un fallo de la herramienta (un argumento mal, PixPin cerrada)
//!   es un resultado con `isError: true`, para que el modelo lo lea y se
//!   corrija; una herramienta que no existe es un error del protocolo
//!   (`-32602`), como dice la especificacion.
//! - Lo demas: `-32601` (metodo que no existe), `-32700` (no es JSON),
//!   `-32600` (no es una peticion; los lotes ya no existen en 2025-06-18).
//!
//! Las llamadas a herramientas pueden tardar (una pregunta de permiso espera
//! al usuario): [`Salida::Herramienta`] le dice al bucle que la haga en su
//! hilo y conteste cuando acabe, sin dejar de atender `ping`.

use serde_json::{Map, Value, json};

/// Las versiones del protocolo que se entienden, la mas nueva primero.
pub const VERSIONES: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub mod codigo {
    pub const NO_ES_JSON: i64 = -32700;
    pub const PETICION_MALA: i64 = -32600;
    pub const SIN_METODO: i64 = -32601;
    pub const PARAMETROS: i64 = -32602;
}

/// Que hacer con una linea recibida.
#[derive(Debug, Clone, PartialEq)]
pub enum Salida {
    /// Contestar esto ya.
    Responder(Value),
    /// Una notificacion (o una respuesta del cliente): nada.
    Nada,
    /// Hacer la herramienta (en otro hilo) y contestar con [`respuesta`].
    Herramienta {
        id: Value,
        nombre: String,
        argumentos: Map<String, Value>,
    },
}

/// Una respuesta buena.
pub fn respuesta(id: Value, resultado: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": resultado})
}

/// Una respuesta de error.
pub fn error(id: Value, codigo: i64, mensaje: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": codigo, "message": mensaje.into()}})
}

/// Lo que se contesta a `initialize`.
fn inicio(params: &Value) -> Value {
    let pedida = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or("");
    let version = VERSIONES
        .iter()
        .find(|v| **v == pedida)
        .copied()
        .unwrap_or(VERSIONES[0]);
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {"listChanged": false}},
        "serverInfo": {"name": crate::NOMBRE, "title": "PixPin Max", "version": crate::VERSION},
        "instructions": concat!(
            "PixPin Max es la app de capturas, pines, chats por proyecto, notas, tareas, lienzo y visores del usuario en este PC. ",
            "Las herramientas de leer (buscar_en_mis_cosas, consultar_tareas, listar_chats, leer_documento, ocr_imagen, ver_pantalla, listar_ventanas) ",
            "funcionan sin la app abierta. Las demas las hace la PixPin abierta, como si lo pidiera el usuario; ",
            "lo que borra, envia al movil o cambia lo que ya existe se lo pregunta PixPin al usuario antes. ",
            "Las tareas y listas se nombran por su texto y su titulo, y las fechas como se dicen («mañana a las 10»)."
        ),
    })
}

/// **Atiende una linea.** Pura: no hace las herramientas (ver
/// [`Salida::Herramienta`]), asi se prueba sin PixPin ni hilos.
pub fn atender(linea: &str) -> Salida {
    let v: Value = match serde_json::from_str(linea) {
        Ok(v) => v,
        Err(e) => {
            return Salida::Responder(error(
                Value::Null,
                codigo::NO_ES_JSON,
                format!("no es JSON: {e}"),
            ));
        }
    };
    let Value::Object(m) = v else {
        return Salida::Responder(error(
            Value::Null,
            codigo::PETICION_MALA,
            "se espera un objeto JSON-RPC (sin lotes)",
        ));
    };
    let id = m.get("id").cloned();
    let Some(metodo) = m.get("method").and_then(Value::as_str) else {
        // Una respuesta del cliente (no le pedimos nada) o basura sin metodo.
        return match id {
            Some(id) if !m.contains_key("result") && !m.contains_key("error") => {
                Salida::Responder(error(id, codigo::PETICION_MALA, "falta «method»"))
            }
            _ => Salida::Nada,
        };
    };
    // Sin id es una notificacion: nunca se contesta.
    let Some(id) = id else { return Salida::Nada };
    let params = m.get("params").cloned().unwrap_or(Value::Null);
    match metodo {
        "initialize" => Salida::Responder(respuesta(id, inicio(&params))),
        "ping" => Salida::Responder(respuesta(id, json!({}))),
        "tools/list" => Salida::Responder(respuesta(
            id,
            json!({"tools": crate::herramientas::lista()}),
        )),
        "tools/call" => {
            let Some(nombre) = params.get("name").and_then(Value::as_str) else {
                return Salida::Responder(error(id, codigo::PARAMETROS, "falta «name»"));
            };
            if !crate::herramientas::existe(nombre) {
                return Salida::Responder(error(
                    id,
                    codigo::PARAMETROS,
                    format!("Herramienta desconocida: {nombre}"),
                ));
            }
            let argumentos = match params.get("arguments") {
                None | Some(Value::Null) => Map::new(),
                Some(Value::Object(a)) => a.clone(),
                Some(_) => {
                    return Salida::Responder(error(
                        id,
                        codigo::PARAMETROS,
                        "«arguments» tiene que ser un objeto",
                    ));
                }
            };
            Salida::Herramienta {
                id,
                nombre: nombre.to_string(),
                argumentos,
            }
        }
        // Sin recursos ni plantillas: listas vacias en vez de un error, que
        // algunos clientes las piden aunque no se anuncien.
        "resources/list" => Salida::Responder(respuesta(id, json!({"resources": []}))),
        "resources/templates/list" => {
            Salida::Responder(respuesta(id, json!({"resourceTemplates": []})))
        }
        "prompts/list" => Salida::Responder(respuesta(id, json!({"prompts": []}))),
        otro => Salida::Responder(error(
            id,
            codigo::SIN_METODO,
            format!("metodo desconocido: {otro}"),
        )),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn contestar(l: &str) -> Value {
        match atender(l) {
            Salida::Responder(v) => v,
            otra => panic!("{otra:?}"),
        }
    }

    #[test]
    fn el_saludo_dice_version_y_capacidades() {
        let r = contestar(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"x","version":"1"}}}"#,
        );
        assert_eq!(r["id"], 1);
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert!(r["result"]["capabilities"]["tools"].is_object());
        assert_eq!(r["result"]["serverInfo"]["name"], "pixpin");
        // Una version que no se conoce: la nuestra mas nueva.
        let r = contestar(
            r#"{"jsonrpc":"2.0","id":"a","method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#,
        );
        assert_eq!(r["result"]["protocolVersion"], VERSIONES[0]);
    }

    #[test]
    fn las_notificaciones_no_se_contestan() {
        assert_eq!(
            atender(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
            Salida::Nada
        );
        assert_eq!(
            atender(
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":3}}"#
            ),
            Salida::Nada
        );
        // Una respuesta del cliente tampoco.
        assert_eq!(
            atender(r#"{"jsonrpc":"2.0","id":9,"result":{}}"#),
            Salida::Nada
        );
    }

    #[test]
    fn lo_roto_da_su_codigo() {
        assert_eq!(contestar("no json")["error"]["code"], codigo::NO_ES_JSON);
        assert_eq!(contestar("[1,2]")["error"]["code"], codigo::PETICION_MALA);
        assert_eq!(
            contestar(r#"{"jsonrpc":"2.0","id":2,"method":"cosas/raras"}"#)["error"]["code"],
            codigo::SIN_METODO
        );
        let r = contestar(
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"no_existe","arguments":{}}}"#,
        );
        assert_eq!(r["error"]["code"], codigo::PARAMETROS);
        assert!(
            r["error"]["message"]
                .as_str()
                .unwrap()
                .contains("no_existe")
        );
        let r = contestar(
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"consultar_tareas","arguments":[1]}}"#,
        );
        assert_eq!(r["error"]["code"], codigo::PARAMETROS);
    }

    #[test]
    fn una_llamada_buena_se_hace_aparte() {
        match atender(
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"consultar_tareas"}}"#,
        ) {
            Salida::Herramienta {
                id,
                nombre,
                argumentos,
            } => {
                assert_eq!(id, 5);
                assert_eq!(nombre, "consultar_tareas");
                assert!(argumentos.is_empty());
            }
            otra => panic!("{otra:?}"),
        }
    }
}
