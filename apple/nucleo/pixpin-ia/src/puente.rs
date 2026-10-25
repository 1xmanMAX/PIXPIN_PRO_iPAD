//! **Las herramientas para cualquier motor, en JSON de ida y vuelta.**
//!
//! En Mac, iPad e iPhone el asistente por omision sera **Foundation Models**
//! (el modelo de Apple en el aparato, con *tool calling*), llamado desde
//! Swift; llama.cpp con Metal queda de respaldo. Foundation Models no pasa
//! por [`crate::Motor`] ni por el bucle de [`crate::agente`]: Swift declara
//! las herramientas (con `DynamicGenerationSchema` a partir del JSON Schema
//! de aqui) y, cuando el modelo llama una, le pasa a Rust el nombre y los
//! argumentos como texto JSON (`GeneratedContent.jsonString`).
//!
//! Por eso este modulo es **solo funciones puras de texto JSON a texto
//! JSON**, sin tipos de Rust en la frontera (cruzan el puente de UniFFI tal
//! cual, una llamada por herramienta, no una por figura: §4bis del plan):
//!
//! - [`catalogo`] / [`catalogo_para`]: las herramientas, en la forma de
//!   `tools` de OpenAI (la misma que ven llama.cpp, Claude y Android).
//! - [`llamar`]: valida la llamada con las mismas reglas que el resto
//!   (`herramientas::validar`) y, si dibuja, devuelve ya los elementos de
//!   Excalidraw para el lienzo. Si falla, el texto que hay que devolverle al
//!   modelo para que se corrija.

use serde_json::{Value, json};

use crate::herramientas::{self, Herramienta};
use crate::motor::LlamadaCruda;

fn lista(h: &[Herramienta]) -> String {
    Value::Array(h.iter().map(Herramienta::a_json).collect()).to_string()
}

/// Todas las herramientas (lista de `tools` de OpenAI), en una linea.
pub fn catalogo() -> String {
    lista(&herramientas::catalogo())
}

/// Las pocas que vienen a cuento para este pedido (`seleccion`): un modelo
/// pequeno se pierde con el catalogo entero.
pub fn catalogo_para(pedido: &str, con_imagen: bool) -> String {
    lista(&crate::seleccion::para_el_pedido(pedido, con_imagen))
}

/// **Una llamada del modelo, de cualquier motor.** `argumentos` es el texto
/// JSON tal como llego (puede venir roto: se intenta arreglar una vez).
/// `origen` es donde cae el (0,0) del modelo en el lienzo real y `semilla`,
/// cualquier numero distinto en cada llamada.
///
/// Devuelve un objeto JSON:
///
/// - `{"ok": true, "herramienta", "avisos": [..], "elementos": [..],
///   "argumentos": {..}}`: `elementos` son los de Excalidraw si la
///   herramienta dibuja (si no, vacio: OCR, acciones de la app, buscar...,
///   que hace la app con `argumentos`, ya reparados).
/// - `{"ok": false, "para_el_modelo": "ERROR: ..."}`: no se ejecuta nada y
///   ese texto vuelve al modelo como resultado de la herramienta.
pub fn llamar(nombre: &str, argumentos: &str, origen: (f64, f64), semilla: u64) -> String {
    let conocidas = herramientas::catalogo();
    let cruda = LlamadaCruda {
        id: String::new(),
        nombre: nombre.to_string(),
        argumentos: argumentos.to_string(),
    };
    match herramientas::validar(&cruda, &conocidas) {
        Ok(v) => {
            let elementos = crate::lienzo::elementos(&v.llamada, origen, semilla);
            let argumentos = herramientas::reparar_json(argumentos).unwrap_or_else(|| json!({}));
            json!({
                "ok": true,
                "herramienta": nombre.trim(),
                "avisos": v.avisos,
                "elementos": elementos,
                "argumentos": argumentos,
            })
        }
        Err(e) => json!({"ok": false, "para_el_modelo": e.para_el_modelo(&conocidas)}),
    }
    .to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn leer(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn el_catalogo_es_la_lista_de_tools_de_openai() {
        let v = leer(&catalogo());
        let l = v.as_array().unwrap();
        assert!(l.len() > 5);
        assert!(l.iter().all(|t| t["type"] == "function" && t["function"]["parameters"].is_object()));
        let pocas = leer(&catalogo_para("dibuja un circulo rojo", false));
        assert!(pocas.as_array().unwrap().len() <= crate::seleccion::MAXIMO);
    }

    #[test]
    fn una_llamada_buena_da_elementos_del_lienzo() {
        let r = leer(&llamar(
            herramientas::DIBUJAR,
            r#"{"elementos":[{"tipo":"elipse","x":10,"y":10,"ancho":100,"alto":100,"color":"rojo"}]}"#,
            (0.0, 0.0),
            7,
        ));
        assert_eq!(r["ok"], true, "{r}");
        assert!(!r["elementos"].as_array().unwrap().is_empty());
        assert_eq!(r["herramienta"], herramientas::DIBUJAR);
        // Lo dibujado se lee en el lienzo como propio.
        let doc = crate::lienzo::documento(r["elementos"].as_array().unwrap().clone());
        assert!(pixpin_motor2d::excalidraw::leer(&doc).is_ok());
    }

    #[test]
    fn una_llamada_mala_vuelve_al_modelo_sin_ejecutarse() {
        let r = leer(&llamar("borrar_todo", "{}", (0.0, 0.0), 1));
        assert_eq!(r["ok"], false);
        assert!(r["para_el_modelo"].as_str().unwrap().starts_with("ERROR"));
        let r = leer(&llamar(herramientas::DIBUJAR, "{esto no es json", (0.0, 0.0), 1));
        assert_eq!(r["ok"], false);
    }

    #[test]
    fn lo_que_no_dibuja_lo_hace_la_app_con_los_argumentos() {
        let r = leer(&llamar(herramientas::OCR, "```json\n{\"idioma\": \"es\"}\n```", (0.0, 0.0), 1));
        assert_eq!(r["ok"], true, "{r}");
        assert!(r["elementos"].as_array().unwrap().is_empty());
        assert_eq!(r["argumentos"]["idioma"], "es");
    }
}
