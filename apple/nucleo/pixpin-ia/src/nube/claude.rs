//! **El adaptador de Claude** (Messages API, `claude-haiku-5-5`).
//!
//! Traduce la conversacion comun (`motor::Mensaje`, forma OpenAI) a la de
//! Anthropic y lee su streaming. Decisiones, segun la documentacion de la
//! API (skill `claude-api`, 13-oct-2026):
//!
//! - **Pensar**: `thinking: {type: "adaptive", display: "summarized"}` para
//!   poder ensenar el resumen del razonamiento (por omision Haiku 5.5 lo
//!   devuelve vacio). Los bloques `thinking` se devuelven **intactos** (con su
//!   firma) junto a los resultados de herramientas: se guardan por id de
//!   llamada en la memoria del motor, porque el `Mensaje` comun no los lleva.
//! - **Esfuerzo `medium`** (el de fabrica de Haiku 5.5): dibujar es colocar
//!   decenas de coordenadas con sentido y en `low` la guia avisa de que el
//!   modelo se salta comprobaciones en prompts de agente; `high` duplica los
//!   tokens de salida sin ganancia visible en pedidos de chat. Se manda
//!   explicito para que no cambie si cambia el de fabrica.
//! - **Sin `temperature`/`top_p`/`top_k`** (400 en Haiku 5.5) y **sin
//!   prefill**: la conversacion siempre acaba en un turno del usuario.
//! - **Sin `strict: true`** (medido con la API real el 13-oct, ver
//!   `grabaciones/`): con el modo estricto Haiku 5.5 dibujaba figuras sin
//!   tamano ni texto (`{"tipo":"rectangulo","x":0,"y":0}`) y numeros
//!   deformes, y el banco caia al 50 % en los dibujos; sin el, 100 %. La
//!   decodificacion restringida sigue el orden del esquema (serde_json lo
//!   escribe por orden alfabetico): tras «tipo» solo le quedaban «x» e «y».
//!   La validacion estricta la hace igual `herramientas::validar` antes de
//!   ejecutar, con el reintento corto del agente.
//! - `eager_input_streaming: true` porque la peticion va en streaming: la
//!   entrada llega sin validar del servidor y se valida aqui.
//! - `tool_choice: auto`. Todos los resultados de una vuelta en **un solo**
//!   mensaje `user`, con `is_error` en los que fallaron.
//! - **Cache**: un punto en el ultimo bloque del `system` (cachea herramientas
//!   y sistema, que no cambian) y otro en el ultimo bloque del ultimo mensaje
//!   (la conversacion que crece). 2 de los 4 permitidos.

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::ErrorIa;
use crate::herramientas::Herramienta;
use crate::motor::{Fin, LlamadaCruda, Medidas, Mensaje, Parte, Peticion, Respuesta, Rol, Trozo};

use super::{Proveedor, instrucciones};

pub const URL: &str = "https://api.anthropic.com/v1/messages";
pub const MODELO: &str = "claude-haiku-5-5";
/// **El critico de respaldo** de la revision con rubrica (23-oct): mas
/// fuerte que el del turno, solo para puntuar cuando Haiku no da una nota
/// que se lea o ya suspendio dos veces (`agente::Opciones::critico`).
pub const MODELO_CRITICO: &str = "claude-sonnet-5-5";
pub const VERSION_API: &str = "2023-06-01";
pub const ESFUERZO: &str = "medium";
/// Tope de salida de cada vuelta. El pensamiento cuenta dentro: con 2048
/// (el del modelo local) un dibujo grande despues de pensar salia cortado.
pub const MAX_TOKENS: u32 = 16_000;

/// Una herramienta en la forma de Anthropic. Sin `strict` (ver la cabecera).
pub fn herramienta(h: &Herramienta) -> Value {
    json!({
        "name": h.nombre,
        "description": h.descripcion,
        "input_schema": h.parametros,
        "eager_input_streaming": true,
    })
}

fn imagen(bytes: &[u8], mime: &str) -> Value {
    json!({"type": "image", "source": {"type": "base64", "media_type": mime,
           "data": crate::base64::codificar(bytes)}})
}

/// Los argumentos de una llamada como objeto (lo que pide `tool_use.input`).
fn como_objeto(argumentos: &str) -> Value {
    match crate::herramientas::reparar_json(argumentos) {
        Some(v @ Value::Object(_)) => v,
        _ => json!({}),
    }
}

/// **Los mensajes** en la forma de Anthropic: roles que se alternan, los
/// resultados de herramientas juntos en un mensaje `user`, y los bloques de
/// pensamiento devueltos tal cual (de `memoria`, por id de llamada).
pub fn mensajes(lista: &[Mensaje], memoria: &HashMap<String, Value>) -> Vec<Value> {
    let mut salida: Vec<(String, Vec<Value>)> = Vec::new();
    let mut poner = |rol: &str, bloques: Vec<Value>| {
        if bloques.is_empty() {
            return;
        }
        match salida.last_mut() {
            Some((r, b)) if r == rol => b.extend(bloques),
            _ => salida.push((rol.to_string(), bloques)),
        }
    };
    for (i, m) in lista.iter().enumerate() {
        match m.rol {
            // El primero va en `system`; uno a mitad (un aviso de la app) va
            // como texto del usuario marcado, que admite cualquier modelo.
            Rol::Sistema if i == 0 => {}
            Rol::Sistema => poner(
                "user",
                vec![
                    json!({"type": "text", "text": format!("<aviso_de_la_app>{}</aviso_de_la_app>", m.texto_entero())}),
                ],
            ),
            Rol::Usuario => {
                let mut b = Vec::new();
                for p in &m.partes {
                    match p {
                        Parte::Imagen { bytes, mime } => b.push(imagen(bytes, mime)),
                        Parte::Texto(t) if !t.trim().is_empty() => {
                            b.push(json!({"type": "text", "text": t}))
                        }
                        Parte::Texto(_) => {}
                    }
                }
                poner("user", b);
            }
            Rol::Asistente => {
                let guardado = m
                    .llamadas
                    .first()
                    .and_then(|l| memoria.get(&l.id))
                    .and_then(Value::as_array);
                let bloques = match guardado {
                    Some(b) => b.clone(),
                    None => {
                        let mut b = Vec::new();
                        let t = m.texto_entero();
                        if !t.trim().is_empty() {
                            b.push(json!({"type": "text", "text": t}));
                        }
                        for l in &m.llamadas {
                            b.push(json!({"type": "tool_use", "id": l.id, "name": l.nombre,
                                          "input": como_objeto(&l.argumentos)}));
                        }
                        b
                    }
                };
                poner("assistant", bloques);
            }
            Rol::Herramienta => {
                let texto = m.texto_entero();
                let error = texto.trim_start().starts_with("ERROR");
                let mut r = json!({"type": "tool_result",
                    "tool_use_id": m.id_llamada.clone().unwrap_or_default(),
                    "content": if texto.is_empty() { "(sin texto)".to_string() } else { texto }});
                if error {
                    r["is_error"] = json!(true);
                }
                poner("user", vec![r]);
            }
        }
    }
    // Empieza y acaba con el usuario (sin prefill: Haiku 5.5 da 400).
    if salida.first().is_some_and(|(r, _)| r == "assistant") {
        salida.insert(
            0,
            (
                "user".into(),
                vec![json!({"type": "text", "text": "(sigue la conversacion de antes)"})],
            ),
        );
    }
    if salida.last().is_some_and(|(r, _)| r == "assistant") {
        salida.push((
            "user".into(),
            vec![json!({"type": "text", "text": "Sigue."})],
        ));
    }
    salida
        .into_iter()
        .map(|(rol, content)| json!({"role": rol, "content": content}))
        .collect()
}

/// **La web** (20-oct-2026, el usuario: «dale acceso a la web al agente de
/// IA»): las herramientas de servidor de Anthropic. Las ejecuta la API (no
/// la app): el modelo busca o lee una pagina y sigue en la misma vuelta.
/// Sus bloques (`server_tool_use`, `web_search_tool_result`…) se devuelven
/// intactos en la vuelta siguiente, como el pensamiento. Con tope de usos
/// por pregunta para que una duda no se coma los creditos.
pub const BUSCAR_EN_LA_WEB: &str = "web_search";
pub const LEER_LA_WEB: &str = "web_fetch";

pub fn herramientas_web() -> Vec<Value> {
    vec![
        json!({"type": "web_search_20250305", "name": BUSCAR_EN_LA_WEB, "max_uses": 5}),
        json!({"type": "web_fetch_20250910", "name": LEER_LA_WEB, "max_uses": 3}),
    ]
}

/// **El cuerpo de la peticion.**
pub fn cuerpo(p: &Peticion, memoria: &HashMap<String, Value>, max_tokens: u32) -> Value {
    let base = p
        .mensajes
        .first()
        .filter(|m| m.rol == Rol::Sistema)
        .map(Mensaje::texto_entero)
        .unwrap_or_default();
    // Dos bloques: lo fijo con su punto de cache (herramientas + reglas, que
    // no cambian de una pregunta a otra) y lo que cambia cada turno (la hoja
    // de trabajo, la peticion mejorada) detras, sin punto: si fuera dentro
    // del mismo bloque, cada pregunta nueva volveria a escribir la cache.
    let (fijo, variable) = instrucciones::en_dos(Proveedor::Claude, &base, &p.herramientas);
    let mut sistema =
        vec![json!({"type": "text", "text": fijo, "cache_control": {"type": "ephemeral"}})];
    if !variable.is_empty() {
        sistema.push(json!({"type": "text", "text": variable}));
    }
    let mut mensajes = mensajes(&p.mensajes, memoria);
    // El punto de cache de la conversacion: el ultimo bloque del ultimo
    // mensaje (que siempre es del usuario).
    if let Some(ultimo) = mensajes.last_mut()
        && let Some(Value::Array(b)) = ultimo.get_mut("content")
        && let Some(Value::Object(bloque)) = b.last_mut()
    {
        bloque.insert("cache_control".into(), json!({"type": "ephemeral"}));
    }
    let mut c = json!({
        "model": MODELO,
        "max_tokens": max_tokens,
        "stream": true,
        "system": sistema,
        "messages": mensajes,
        "thinking": {"type": "adaptive", "display": "summarized"},
        "output_config": {"effort": ESFUERZO},
    });
    // Las de la app y, siempre, las de la web (las ejecuta la API).
    let mut tools: Vec<Value> = p.herramientas.iter().map(herramienta).collect();
    tools.extend(herramientas_web());
    c["tools"] = Value::Array(tools);
    c["tool_choice"] = json!({"type": "auto"});
    c
}

/// Las cabeceras (la clave va aqui y en ningun otro sitio).
pub fn cabeceras(clave: &str) -> Vec<(&'static str, String)> {
    vec![
        ("x-api-key", clave.to_string()),
        ("anthropic-version", VERSION_API.to_string()),
    ]
}

/// La peticion minima de «Probar»: sin streaming, sin pensar, 16 tokens.
pub fn cuerpo_de_prueba() -> Value {
    json!({
        "model": MODELO,
        "max_tokens": 16,
        "thinking": {"type": "disabled"},
        "messages": [{"role": "user", "content": "Responde solo: ok"}],
    })
}

// ---------------------------------------------------------------------------
// El streaming

#[derive(Debug, Clone)]
enum Bloque {
    Texto(String),
    Pensando {
        texto: String,
        firma: String,
    },
    /// `redacted_thinking` y lo que no se conozca: se devuelve tal cual.
    Tal(Value),
    Herramienta {
        id: String,
        nombre: String,
        json: String,
    },
    /// `server_tool_use` (la web): lo ejecuta la API; solo se guarda para
    /// devolverlo intacto, con su entrada juntada de los trozos.
    Servidor {
        bloque: Value,
        json: String,
    },
}

/// Lo que dejo una respuesta entera.
#[derive(Debug, Clone)]
pub struct Salida {
    pub respuesta: Respuesta,
    /// Los bloques para devolverlos intactos en la vuelta siguiente.
    pub bloques: Vec<Value>,
    /// `stop_reason` tal cual (`end_turn`, `tool_use`, `max_tokens`,
    /// `refusal`…), si llego.
    pub motivo: Option<String>,
    /// `stop_details.category` de un rechazo.
    pub categoria: Option<String>,
}

/// Un error que llego dentro del flujo (`event: error`).
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorDelFlujo {
    pub tipo: String,
    pub mensaje: String,
}

#[derive(Default)]
pub struct Lector {
    pendiente: Vec<u8>,
    bloques: Vec<Bloque>,
    motivo: Option<String>,
    categoria: Option<String>,
    medidas: Medidas,
    pub acabado: bool,
    pub error: Option<ErrorDelFlujo>,
}

impl Lector {
    pub fn nuevo() -> Lector {
        Lector::default()
    }

    /// Mete bytes y devuelve los trozos nuevos.
    pub fn meter(&mut self, bytes: &[u8]) -> Result<Vec<Trozo>, ErrorIa> {
        self.pendiente.extend_from_slice(bytes);
        let mut trozos = Vec::new();
        while let Some(fin) = self.pendiente.iter().position(|&b| b == b'\n') {
            let linea: Vec<u8> = self.pendiente.drain(..=fin).collect();
            let linea = String::from_utf8_lossy(&linea);
            let linea = linea.trim_end_matches(['\r', '\n']);
            // `event:` repite el `type` del JSON: basta con `data:`.
            if let Some(datos) = linea.strip_prefix("data:") {
                self.evento(datos.trim(), &mut trozos)?;
            }
        }
        Ok(trozos)
    }

    fn evento(&mut self, datos: &str, trozos: &mut Vec<Trozo>) -> Result<(), ErrorIa> {
        if datos.is_empty() {
            return Ok(());
        }
        let v: Value = serde_json::from_str(datos)
            .map_err(|e| ErrorIa::Protocolo(format!("evento de Claude ilegible ({e})")))?;
        let tipo = v.get("type").and_then(Value::as_str).unwrap_or("");
        match tipo {
            "message_start" => {
                if let Some(u) = v.pointer("/message/usage") {
                    self.uso(u);
                }
            }
            "content_block_start" => {
                let b = v.get("content_block").cloned().unwrap_or(Value::Null);
                let bloque = match b.get("type").and_then(Value::as_str) {
                    Some("text") => Bloque::Texto(
                        b.get("text")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    ),
                    Some("thinking") => Bloque::Pensando {
                        texto: b
                            .get("thinking")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        firma: b
                            .get("signature")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    },
                    Some("tool_use") => {
                        let nombre = b
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        trozos.push(Trozo::Llamando {
                            nombre: nombre.clone(),
                        });
                        Bloque::Herramienta {
                            id: b
                                .get("id")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                            nombre,
                            json: String::new(),
                        }
                    }
                    Some("server_tool_use") => {
                        let nombre = b
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        trozos.push(Trozo::Llamando { nombre });
                        Bloque::Servidor {
                            bloque: b,
                            json: String::new(),
                        }
                    }
                    _ => Bloque::Tal(b),
                };
                let i = v
                    .get("index")
                    .and_then(Value::as_u64)
                    .map_or(self.bloques.len(), |i| i as usize);
                if i > 256 {
                    return Err(ErrorIa::Protocolo(
                        "demasiados bloques en la respuesta".into(),
                    ));
                }
                while self.bloques.len() < i {
                    self.bloques.push(Bloque::Tal(Value::Null));
                }
                if i == self.bloques.len() {
                    self.bloques.push(bloque);
                } else {
                    self.bloques[i] = bloque;
                }
            }
            "content_block_delta" => {
                let i = v.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let d = v.get("delta").cloned().unwrap_or(Value::Null);
                let Some(b) = self.bloques.get_mut(i) else {
                    return Err(ErrorIa::Protocolo(
                        "trozo de un bloque que no empezo".into(),
                    ));
                };
                let s = |k: &str| d.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                match (d.get("type").and_then(Value::as_str), b) {
                    (Some("text_delta"), Bloque::Texto(t)) => {
                        let x = s("text");
                        if !x.is_empty() {
                            t.push_str(&x);
                            trozos.push(Trozo::Texto(x));
                        }
                    }
                    (Some("thinking_delta"), Bloque::Pensando { texto, .. }) => {
                        let x = s("thinking");
                        if !x.is_empty() {
                            texto.push_str(&x);
                            trozos.push(Trozo::Pensando(x));
                        }
                    }
                    (Some("signature_delta"), Bloque::Pensando { firma, .. }) => {
                        firma.push_str(&s("signature"))
                    }
                    (
                        Some("input_json_delta"),
                        Bloque::Herramienta { json, .. } | Bloque::Servidor { json, .. },
                    ) => json.push_str(&s("partial_json")),
                    // Citas y lo que venga nuevo: no se usa.
                    _ => {}
                }
            }
            "message_delta" => {
                if let Some(m) = v.pointer("/delta/stop_reason").and_then(Value::as_str) {
                    self.motivo = Some(m.to_string());
                }
                if let Some(c) = v
                    .pointer("/delta/stop_details/category")
                    .and_then(Value::as_str)
                {
                    self.categoria = Some(c.to_string());
                }
                if let Some(u) = v.get("usage") {
                    self.uso(u);
                }
            }
            "message_stop" => self.acabado = true,
            "error" => {
                let tipo = v
                    .pointer("/error/type")
                    .and_then(Value::as_str)
                    .unwrap_or("error")
                    .to_string();
                let mensaje = v
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                self.error = Some(ErrorDelFlujo { tipo, mensaje });
                self.acabado = true;
            }
            // `ping`, `content_block_stop` y lo que se anada: nada que hacer.
            _ => {}
        }
        Ok(())
    }

    fn uso(&mut self, u: &Value) {
        let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        let entrada =
            n("input_tokens") + n("cache_read_input_tokens") + n("cache_creation_input_tokens");
        if entrada > 0 {
            self.medidas.tokens_entrada = entrada;
        }
        if n("cache_read_input_tokens") > 0 {
            self.medidas.tokens_cache_leidos = n("cache_read_input_tokens");
        }
        if n("cache_creation_input_tokens") > 0 {
            self.medidas.tokens_cache_escritos = n("cache_creation_input_tokens");
        }
        if n("output_tokens") > 0 {
            self.medidas.tokens_salida = n("output_tokens");
        }
    }

    /// La respuesta entera. `cortado`: el usuario paro.
    pub fn acabar(self, cortado: bool) -> Salida {
        let mut texto = String::new();
        let mut pensamiento = String::new();
        let mut llamadas = Vec::new();
        let mut bloques = Vec::new();
        for b in self.bloques {
            match b {
                Bloque::Texto(t) => {
                    texto.push_str(&t);
                    if !t.is_empty() {
                        bloques.push(json!({"type": "text", "text": t}));
                    }
                }
                Bloque::Pensando { texto: t, firma } => {
                    pensamiento.push_str(&t);
                    // Se guarda aunque el texto venga vacio: la firma es lo
                    // que importa.
                    bloques.push(json!({"type": "thinking", "thinking": t, "signature": firma}));
                }
                Bloque::Tal(Value::Null) => {}
                Bloque::Tal(v) => bloques.push(v),
                Bloque::Servidor { mut bloque, json } => {
                    if !json.trim().is_empty() {
                        bloque["input"] = como_objeto(&json);
                    }
                    bloques.push(bloque);
                }
                Bloque::Herramienta { id, nombre, json } => {
                    let argumentos = if json.trim().is_empty() {
                        "{}".to_string()
                    } else {
                        json
                    };
                    bloques.push(json!({"type": "tool_use", "id": id, "name": nombre, "input": como_objeto(&argumentos)}));
                    llamadas.push(LlamadaCruda {
                        id,
                        nombre,
                        argumentos,
                    });
                }
            }
        }
        let fin = if cortado {
            Fin::Cortado
        } else {
            match self.motivo.as_deref() {
                Some("tool_use") => Fin::Herramientas,
                Some("max_tokens") | Some("model_context_window_exceeded") => Fin::Longitud,
                _ if !llamadas.is_empty() => Fin::Herramientas,
                _ => Fin::Natural,
            }
        };
        Salida {
            respuesta: Respuesta {
                texto,
                pensamiento,
                llamadas,
                fin,
                medidas: self.medidas,
            },
            bloques,
            motivo: self.motivo,
            categoria: self.categoria,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::herramientas::catalogo;

    fn por_nombre(n: &str) -> Herramienta {
        catalogo().into_iter().find(|h| h.nombre == n).unwrap()
    }

    #[test]
    fn ninguna_herramienta_va_estricta_y_el_esquema_va_tal_cual() {
        // Con `strict` Haiku 5.5 dibujaba figuras vacias (ver la cabecera).
        let h = por_nombre(crate::herramientas::DIBUJAR);
        let t = herramienta(&h);
        assert!(t.get("strict").is_none());
        assert_eq!(t["eager_input_streaming"], true);
        assert_eq!(t["input_schema"], h.parametros);
    }

    /// Lo que el modelo escribio, juntando los `partial_json` del SSE a mano.
    fn juntar_a_mano(sse: &str) -> String {
        sse.lines()
            .filter_map(|l| l.strip_prefix("data: "))
            .filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok())
            .filter_map(|v| {
                v.pointer("/delta/partial_json")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect()
    }

    fn leer_grabada(sse: &str) -> Salida {
        let mut l = Lector::nuevo();
        // A pedazos que cortan lineas, como llega por la red.
        for c in sse.as_bytes().chunks(53) {
            l.meter(c).unwrap();
        }
        assert!(l.acabado);
        l.acabar(false)
    }

    #[test]
    fn el_sse_real_se_junta_byte_a_byte() {
        // Grabadas con la API real (13-oct, `PIXPIN_IA_GRABAR`): la misma
        // casa pedida con y sin `strict`. Lo que se junta es exactamente lo
        // que mando el modelo: la deformacion no era nuestra.
        for sse in [
            include_str!("grabaciones/casa-sin-strict.sse"),
            include_str!("grabaciones/casa-con-strict.sse"),
        ] {
            let s = leer_grabada(sse);
            assert_eq!(s.respuesta.llamadas.len(), 1);
            assert_eq!(s.respuesta.llamadas[0].argumentos, juntar_a_mano(sse));
            assert_eq!(s.respuesta.fin, Fin::Herramientas);
        }
    }

    #[test]
    fn la_casa_sin_strict_trae_tamanos_y_colores_y_la_de_strict_no() {
        let validar = |sse: &str| {
            let s = leer_grabada(sse);
            crate::herramientas::validar(&s.respuesta.llamadas[0], &catalogo())
        };
        let buena = validar(include_str!("grabaciones/casa-sin-strict.sse")).unwrap();
        let crate::herramientas::Llamada::Dibujar(figuras) = &buena.llamada else {
            panic!()
        };
        assert_eq!(figuras.len(), 3);
        assert!(figuras.iter().all(|f| f.con_color), "{figuras:?}");
        assert!(
            figuras
                .iter()
                .filter(|f| f.ancho > 0.0 && f.alto > 0.0)
                .count()
                >= 2,
            "{figuras:?}"
        );
        // Caso negativo: lo que salia con `strict` (`{"tipo":"rectangulo",
        // "x":0}`, sin tamano, color ni «y») no pasa la validacion.
        assert!(validar(include_str!("grabaciones/casa-con-strict.sse")).is_err());
    }

    #[test]
    fn el_cuerpo_sigue_las_reglas_de_haiku_5_5() {
        let mut p = Peticion::nueva(vec![
            Mensaje::sistema("Eres el asistente."),
            Mensaje::usuario("hola"),
        ]);
        p.herramientas = catalogo();
        let c = cuerpo(&p, &HashMap::new(), MAX_TOKENS);
        assert_eq!(c["model"], "claude-haiku-5-5");
        assert_eq!(
            c["thinking"],
            json!({"type": "adaptive", "display": "summarized"})
        );
        assert_eq!(c["output_config"]["effort"], "medium");
        assert_eq!(c["tool_choice"], json!({"type": "auto"}));
        for prohibido in [
            "temperature",
            "top_p",
            "top_k",
            "chat_template_kwargs",
            "presence_penalty",
        ] {
            assert!(c.get(prohibido).is_none(), "{prohibido}");
        }
        assert_eq!(c["system"][0]["cache_control"]["type"], "ephemeral");
        assert!(
            c["system"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Eres el asistente.")
        );
        let m = c["messages"].as_array().unwrap();
        assert_eq!(m.len(), 1, "el sistema no va en messages");
        assert_eq!(m[0]["content"][0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn los_resultados_van_juntos_y_el_pensamiento_vuelve_intacto() {
        let mut asis = Mensaje::texto(Rol::Asistente, "voy");
        asis.llamadas = vec![
            LlamadaCruda {
                id: "toolu_1".into(),
                nombre: "ocr".into(),
                argumentos: "{}".into(),
            },
            LlamadaCruda {
                id: "toolu_2".into(),
                nombre: "grafica".into(),
                argumentos: "{roto".into(),
            },
        ];
        let lista = vec![
            Mensaje::sistema("s"),
            Mensaje::usuario("lee y grafica"),
            asis,
            Mensaje::resultado("toolu_1", "texto leido"),
            Mensaje::resultado("toolu_2", "ERROR: los argumentos no son JSON valido."),
            Mensaje::usuario("y ademas…"),
        ];
        // Sin memoria: se rehace con tool_use y la entrada rota pasa a {}.
        let m = mensajes(&lista, &HashMap::new());
        assert_eq!(m.len(), 3);
        assert_eq!(m[1]["content"][1]["input"], json!({}));
        assert_eq!(m[1]["content"][2]["input"], json!({}));
        let r = m[2]["content"].as_array().unwrap();
        assert_eq!(r.len(), 3, "dos resultados y el texto en UN mensaje");
        assert_eq!(r[0]["type"], "tool_result");
        assert!(r[0].get("is_error").is_none());
        assert_eq!(r[1]["is_error"], true);
        assert_eq!(r[2]["type"], "text");
        // Con memoria: los bloques guardados, firma incluida.
        let guardado = json!([{"type": "thinking", "thinking": "", "signature": "FIRMA"},
                              {"type": "tool_use", "id": "toolu_1", "name": "ocr", "input": {}}]);
        let memoria = HashMap::from([("toolu_1".to_string(), guardado.clone())]);
        let m = mensajes(&lista, &memoria);
        assert_eq!(m[1]["content"], guardado);
    }

    #[test]
    fn nunca_acaba_en_el_asistente_ni_empieza_por_el() {
        let lista = vec![Mensaje::texto(Rol::Asistente, "hola")];
        let m = mensajes(&lista, &HashMap::new());
        assert_eq!(m.first().unwrap()["role"], "user");
        assert_eq!(m.last().unwrap()["role"], "user");
    }

    #[test]
    fn la_imagen_va_en_base64_antes_del_texto() {
        let m = mensajes(
            &[Mensaje::usuario_con_imagen(
                "dibujala",
                vec![1, 2, 3],
                "image/png",
            )],
            &HashMap::new(),
        );
        assert_eq!(
            m[0]["content"][0]["source"],
            json!({"type": "base64", "media_type": "image/png", "data": "AQID"})
        );
        assert_eq!(m[0]["content"][1]["text"], "dibujala");
    }

    #[test]
    fn la_prueba_de_clave_es_minima() {
        let c = cuerpo_de_prueba();
        assert_eq!(c["max_tokens"], 16);
        assert!(c.get("stream").is_none() && c.get("tools").is_none());
    }

    #[test]
    fn la_web_va_siempre_y_la_busqueda_se_devuelve_intacta() {
        let c = cuerpo(
            &Peticion::nueva(vec![Mensaje::usuario("hola")]),
            &HashMap::new(),
            100,
        );
        let tipos: Vec<&str> = c["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["type"].as_str())
            .collect();
        assert!(
            tipos.contains(&"web_search_20250305") && tipos.contains(&"web_fetch_20250910"),
            "{tipos:?}"
        );
        let sse = [
            r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"server_tool_use","id":"srvtoolu_1","name":"web_search","input":{}}}"#,
            r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"query\": \"pixpin\"}"}}"#,
            r#"data: {"type":"content_block_start","index":1,"content_block":{"type":"web_search_tool_result","tool_use_id":"srvtoolu_1","content":[{"type":"web_search_result","url":"https://x","title":"X","encrypted_content":"E"}]}}"#,
            r#"data: {"type":"content_block_start","index":2,"content_block":{"type":"text","text":""}}"#,
            r#"data: {"type":"content_block_delta","index":2,"delta":{"type":"text_delta","text":"Hallado."}}"#,
            r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":5}}"#,
            r#"data: {"type":"message_stop"}"#,
        ]
        .join("\n")
            + "\n";
        let mut l = Lector::nuevo();
        let trozos = l.meter(sse.as_bytes()).unwrap();
        assert!(
            trozos
                .iter()
                .any(|t| matches!(t, Trozo::Llamando { nombre } if nombre == BUSCAR_EN_LA_WEB))
        );
        let s = l.acabar(false);
        assert_eq!(s.respuesta.texto, "Hallado.");
        assert!(
            s.respuesta.llamadas.is_empty(),
            "la web no es una llamada de la app"
        );
        assert_eq!(s.respuesta.fin, Fin::Natural);
        assert_eq!(s.bloques[0]["input"], json!({"query": "pixpin"}));
        assert_eq!(s.bloques[1]["type"], "web_search_tool_result");
    }
}
