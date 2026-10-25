//! **El adaptador de DeepSeek** (API compatible con OpenAI, `deepseek-flash`).
//!
//! El usuario pidio «DeepSeek 4.1 flash». Segun la pagina de modelos y
//! precios de DeepSeek (api-docs.deepseek.com/quick_start/pricing, mirada el
//! 13-oct-2026) el nombre de API es **`deepseek-flash`**, que hoy sirve
//! DeepSeek-V4.1-Flash; los nombres viejos `deepseek-v4-flash` y
//! `deepseek-v4-flash-vision-exp` aun se aceptan pero sus modelos se
//! retiraron. Contexto de 1M, piensa (por omision) o no, llama herramientas
//! y **ve imagenes** (guia de vision: bloques `image_url` con URL `data:`,
//! solo en mensajes del usuario).
//!
//! Decisiones, segun sus guias:
//!
//! - **Pensar encendido** (`thinking: {type: enabled}`, esfuerzo `high`, el
//!   de fabrica) para tener el razonamiento visible en `reasoning_content`.
//!   Con herramientas su API **exige** devolver el `reasoning_content` de
//!   cada respuesta del asistente en las vueltas siguientes (400 si no): se
//!   guarda por id de llamada en la memoria del motor. Si aun asi contesta
//!   400 por el razonamiento (una conversacion vieja sin el), el motor
//!   repite la peticion **sin pensar**, que no lo pide.
//! - **Sin modo estricto**: solo existe en su URL beta y exige que TODOS los
//!   campos sean obligatorios, lo que rompe los opcionales del catalogo. En su
//!   lugar: reglas numeradas, un ejemplo de llamada y recordatorio final
//!   (`instrucciones.rs`), y la validacion estricta de siempre con reintento.
//! - `temperature`/`top_p` no se mandan: en modo pensar no tienen efecto.
//! - La conversacion va en la forma de `Mensaje::a_json` (es la de OpenAI).

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::herramientas::Herramienta;
use crate::motor::{Mensaje, Peticion, Rol};

use super::{Proveedor, instrucciones};

pub const URL: &str = "https://api.deepseek.com/chat/completions";
pub const MODELO: &str = "deepseek-flash";
pub const ESFUERZO: &str = "high";
/// Tope de salida de cada vuelta (el razonamiento cuenta dentro).
pub const MAX_TOKENS: u32 = 16_000;

/// **El cuerpo de la peticion.** `pensar`: el modo con razonamiento.
pub fn cuerpo(p: &Peticion, memoria: &HashMap<String, Value>, pensar: bool, max_tokens: u32) -> Value {
    let base = p
        .mensajes
        .first()
        .filter(|m| m.rol == Rol::Sistema)
        .map(Mensaje::texto_entero)
        .unwrap_or_default();
    let mut mensajes = vec![json!({"role": "system",
        "content": instrucciones::para(Proveedor::DeepSeek, &base, &p.herramientas)})];
    for (i, m) in p.mensajes.iter().enumerate() {
        if i == 0 && m.rol == Rol::Sistema {
            continue;
        }
        let mut j = m.a_json();
        if m.rol == Rol::Asistente {
            if !m.llamadas.is_empty() && j["content"].as_str().is_some_and(str::is_empty) {
                j["content"] = Value::Null;
            }
            if pensar
                && let Some(r) = m.llamadas.first().and_then(|l| memoria.get(&l.id))
            {
                j["reasoning_content"] = r.clone();
            }
        }
        mensajes.push(j);
    }
    let mut c = json!({
        "model": MODELO,
        "messages": mensajes,
        "max_tokens": max_tokens,
        "stream": true,
        "stream_options": {"include_usage": true},
        "thinking": {"type": if pensar { "enabled" } else { "disabled" }},
    });
    if pensar {
        c["reasoning_effort"] = json!(ESFUERZO);
    }
    if !p.herramientas.is_empty() {
        c["tools"] = Value::Array(p.herramientas.iter().map(Herramienta::a_json).collect());
        c["tool_choice"] = json!("auto");
    }
    c
}

pub fn cabeceras(clave: &str) -> Vec<(&'static str, String)> {
    vec![("Authorization", format!("Bearer {clave}"))]
}

/// La peticion minima de «Probar».
pub fn cuerpo_de_prueba() -> Value {
    json!({
        "model": MODELO,
        "max_tokens": 8,
        "thinking": {"type": "disabled"},
        "messages": [{"role": "user", "content": "Responde solo: ok"}],
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::herramientas::catalogo;
    use crate::motor::LlamadaCruda;

    #[test]
    fn el_cuerpo_lleva_modelo_pensar_y_herramientas() {
        let mut p = Peticion::nueva(vec![Mensaje::sistema("Base."), Mensaje::usuario_con_imagen("que es", vec![1], "image/jpeg")]);
        p.herramientas = catalogo();
        let c = cuerpo(&p, &HashMap::new(), true, MAX_TOKENS);
        assert_eq!(c["model"], "deepseek-flash");
        assert_eq!(c["thinking"]["type"], "enabled");
        assert_eq!(c["reasoning_effort"], "high");
        assert_eq!(c["tool_choice"], "auto");
        assert_eq!(c["tools"][0]["type"], "function");
        assert!(c.get("temperature").is_none() && c.get("chat_template_kwargs").is_none());
        let m = c["messages"].as_array().unwrap();
        assert_eq!(m.len(), 2, "un solo mensaje de sistema");
        assert!(m[0]["content"].as_str().unwrap().starts_with("Base."));
        assert_eq!(m[1]["content"][0]["type"], "image_url");
        let sin = cuerpo(&p, &HashMap::new(), false, MAX_TOKENS);
        assert_eq!(sin["thinking"]["type"], "disabled");
        assert!(sin.get("reasoning_effort").is_none());
    }

    #[test]
    fn el_razonamiento_vuelve_con_su_llamada() {
        let mut a = Mensaje::texto(Rol::Asistente, "");
        a.llamadas = vec![LlamadaCruda { id: "call_1".into(), nombre: "ocr".into(), argumentos: "{}".into() }];
        let p = Peticion::nueva(vec![Mensaje::sistema("s"), Mensaje::usuario("lee"), a, Mensaje::resultado("call_1", "hecho")]);
        let memoria = HashMap::from([("call_1".to_string(), json!("pense en leer"))]);
        let c = cuerpo(&p, &memoria, true, 100);
        let asis = &c["messages"][2];
        assert_eq!(asis["reasoning_content"], "pense en leer");
        assert!(asis["content"].is_null());
        assert_eq!(asis["tool_calls"][0]["id"], "call_1");
        assert_eq!(c["messages"][3]["role"], "tool");
        // Sin pensar no se manda.
        assert!(cuerpo(&p, &memoria, false, 100)["messages"][2].get("reasoning_content").is_none());
    }
}
