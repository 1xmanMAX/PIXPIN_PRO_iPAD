//! Pruebas de la nube contra un **servidor HTTP falso** en `127.0.0.1` que
//! imita el streaming de cada API. Pasan por el mismo WinHTTP que la app:
//! lo unico distinto de verdad es la direccion.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use super::*;
use crate::agente::{self, Ejecutor, Evento};
use crate::herramientas::{Llamada, catalogo};
use crate::motor::{Mensaje, MotorGuionado};

/// Una respuesta guionada del servidor falso.
#[derive(Clone)]
struct Falsa {
    codigo: u32,
    cabeceras: String,
    cuerpo: String,
}

fn sse(codigo: u32, eventos: &[Value]) -> Falsa {
    let mut cuerpo = String::new();
    for e in eventos {
        let tipo = e.get("type").and_then(Value::as_str).unwrap_or("x");
        cuerpo.push_str(&format!("event: {tipo}\ndata: {e}\n\n"));
    }
    Falsa {
        codigo,
        cabeceras: String::new(),
        cuerpo,
    }
}

fn crudo(codigo: u32, cuerpo: &str) -> Falsa {
    Falsa {
        codigo,
        cabeceras: String::new(),
        cuerpo: cuerpo.to_string(),
    }
}

/// Lo que recibio el servidor: cabeceras (en minusculas) y cuerpo JSON.
#[derive(Clone, Debug)]
struct Recibida {
    cabeceras: String,
    cuerpo: Value,
}

struct Servidor {
    url: String,
    recibidas: Arc<Mutex<Vec<Recibida>>>,
}

/// Arranca el servidor con su guion: una respuesta por conexion, en orden.
fn servidor(ruta: &str, guion: Vec<Falsa>) -> Servidor {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}{ruta}", l.local_addr().unwrap().port());
    let recibidas = Arc::new(Mutex::new(Vec::new()));
    let r2 = recibidas.clone();
    std::thread::spawn(move || {
        for f in guion {
            let Ok((mut s, _)) = l.accept() else { return };
            s.set_read_timeout(Some(Duration::from_secs(10))).ok();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 8192];
            let fin_cabeza = loop {
                let n = s.read(&mut tmp).unwrap_or(0);
                if n == 0 {
                    break None;
                }
                buf.extend_from_slice(&tmp[..n]);
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(i + 4);
                }
            };
            let Some(fin_cabeza) = fin_cabeza else {
                continue;
            };
            let cabeza = String::from_utf8_lossy(&buf[..fin_cabeza]).to_lowercase();
            let largo: usize = cabeza
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            while buf.len() < fin_cabeza + largo {
                let n = s.read(&mut tmp).unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            let cuerpo = serde_json::from_slice(&buf[fin_cabeza..]).unwrap_or(Value::Null);
            r2.lock().unwrap().push(Recibida {
                cabeceras: cabeza,
                cuerpo,
            });
            let cabecera = format!(
                "HTTP/1.1 {} X\r\nContent-Type: text/event-stream\r\nConnection: close\r\n{}\r\n",
                f.codigo, f.cabeceras
            );
            let _ = s.write_all(cabecera.as_bytes());
            // A pedazos, como llega de verdad.
            for trozo in f.cuerpo.as_bytes().chunks(37) {
                let _ = s.write_all(trozo);
                let _ = s.flush();
            }
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    });
    Servidor { url, recibidas }
}

fn claude(url: &str) -> MotorNube {
    MotorNube::nuevo(Proveedor::Claude, "sk-ant-api03-CLAVEFALSA0123456789")
        .con_url(url)
        .con_esperas(vec![Duration::from_millis(10), Duration::from_millis(10)])
}

fn deepseek(url: &str) -> MotorNube {
    MotorNube::nuevo(Proveedor::DeepSeek, "sk-deepseekfalsa0123456789abcdef")
        .con_url(url)
        .con_esperas(vec![Duration::from_millis(10), Duration::from_millis(10)])
}

// --- Guiones de Claude ------------------------------------------------------

fn c_inicio() -> Value {
    json!({"type": "message_start", "message": {"id": "msg_1", "type": "message", "role": "assistant",
        "usage": {"input_tokens": 50, "cache_read_input_tokens": 1500, "cache_creation_input_tokens": 0, "output_tokens": 1}}})
}

fn c_pensar(i: u32, texto: &str) -> Vec<Value> {
    vec![
        json!({"type": "content_block_start", "index": i, "content_block": {"type": "thinking", "thinking": "", "signature": ""}}),
        json!({"type": "content_block_delta", "index": i, "delta": {"type": "thinking_delta", "thinking": texto}}),
        json!({"type": "content_block_delta", "index": i, "delta": {"type": "signature_delta", "signature": "FIRMA-ORIGINAL"}}),
        json!({"type": "content_block_stop", "index": i}),
    ]
}

fn c_texto(i: u32, partes: &[&str]) -> Vec<Value> {
    let mut v = vec![
        json!({"type": "content_block_start", "index": i, "content_block": {"type": "text", "text": ""}}),
    ];
    for p in partes {
        v.push(json!({"type": "content_block_delta", "index": i, "delta": {"type": "text_delta", "text": p}}));
    }
    v.push(json!({"type": "content_block_stop", "index": i}));
    v
}

fn c_herramienta(i: u32, id: &str, nombre: &str, json_args: &str) -> Vec<Value> {
    let mut v = vec![json!({"type": "content_block_start", "index": i,
        "content_block": {"type": "tool_use", "id": id, "name": nombre, "input": {}}})];
    // La entrada llega a pedazos (eager_input_streaming).
    let car: Vec<char> = json_args.chars().collect();
    for c in car.chunks(9) {
        let s: String = c.iter().collect();
        v.push(json!({"type": "content_block_delta", "index": i, "delta": {"type": "input_json_delta", "partial_json": s}}));
    }
    v.push(json!({"type": "content_block_stop", "index": i}));
    v
}

fn c_fin(motivo: &str) -> Vec<Value> {
    vec![
        json!({"type": "message_delta", "delta": {"stop_reason": motivo, "stop_sequence": null}, "usage": {"output_tokens": 77}}),
        json!({"type": "message_stop"}),
    ]
}

fn guion_claude(bloques: Vec<Vec<Value>>, motivo: &str) -> Falsa {
    let mut e = vec![c_inicio(), json!({"type": "ping"})];
    for b in bloques {
        e.extend(b);
    }
    e.extend(c_fin(motivo));
    sse(200, &e)
}

// --- Guiones de DeepSeek ------------------------------------------------------

fn d_trozo(delta: Value, fin: Option<&str>) -> String {
    format!(
        "data: {}\n\n",
        json!({"id": "x", "object": "chat.completion.chunk", "model": "deepseek-flash",
               "choices": [{"index": 0, "delta": delta, "finish_reason": fin}]})
    )
}

fn guion_deepseek(trozos: Vec<String>) -> Falsa {
    let mut cuerpo = String::from(": keep-alive\n\n");
    for t in trozos {
        cuerpo.push_str(&t);
    }
    cuerpo.push_str(&format!(
        "data: {}\n\n",
        json!({"choices": [], "usage": {"prompt_tokens": 900, "completion_tokens": 40, "prompt_cache_hit_tokens": 800}})
    ));
    cuerpo.push_str("data: [DONE]\n\n");
    crudo(200, &cuerpo)
}

// --- Un ejecutor que apunta -----------------------------------------------

#[derive(Default)]
struct Apunta(Vec<Llamada>);

impl Ejecutor for Apunta {
    fn ejecutar(&mut self, l: &Llamada) -> Result<String, String> {
        self.0.push(l.clone());
        Ok("dibujadas 1 figuras".into())
    }
}

fn historial(pedido: &str) -> Vec<Mensaje> {
    vec![
        Mensaje::sistema(agente::instrucciones()),
        Mensaje::usuario(pedido),
    ]
}

fn turno(
    motor: &dyn Motor,
    pedido: &str,
) -> (
    Result<agente::Final, ErrorIa>,
    Apunta,
    Vec<Evento>,
    Vec<Mensaje>,
) {
    let mut h = historial(pedido);
    let mut ej = Apunta::default();
    let mut ev = Vec::new();
    let r = agente::turno(motor, &mut h, &catalogo(), &mut ej, &mut |e| {
        ev.push(e);
        Seguir::Si
    });
    (r, ej, ev, h)
}

const DIBUJO: &str =
    r#"{"elementos":[{"tipo":"rectangulo","x":10,"y":10,"ancho":100,"alto":80,"color":"rojo"}]}"#;

// ===========================================================================
// Claude

#[test]
fn claude_texto_y_pensamiento_en_vivo() {
    let s = servidor(
        "/v1/messages",
        vec![guion_claude(
            vec![
                c_pensar(0, "El usuario saluda."),
                c_texto(1, &["Ho", "la, ¿qué ", "tal?"]),
            ],
            "end_turn",
        )],
    );
    let m = claude(&s.url);
    let mut trozos = Vec::new();
    let p = Peticion::nueva(historial("hola"));
    let r = m
        .completar(&p, &mut |t| {
            trozos.push(t.clone());
            Seguir::Si
        })
        .unwrap();
    assert_eq!(r.texto, "Hola, ¿qué tal?");
    assert_eq!(r.pensamiento, "El usuario saluda.");
    assert_eq!(r.fin, Fin::Natural);
    assert_eq!(r.medidas.tokens_entrada, 1550);
    assert_eq!(r.medidas.tokens_salida, 77);
    assert_eq!(trozos[0], Trozo::Pensando("El usuario saluda.".into()));
    assert!(trozos.contains(&Trozo::Texto("la, ¿qué ".into())));
    let rec = s.recibidas.lock().unwrap();
    assert_eq!(rec.len(), 1);
    assert!(
        rec[0]
            .cabeceras
            .contains("x-api-key: sk-ant-api03-clavefalsa0123456789")
    );
    assert!(rec[0].cabeceras.contains("anthropic-version: 2023-06-01"));
    assert_eq!(rec[0].cuerpo["model"], "claude-haiku-5-5");
    assert_eq!(rec[0].cuerpo["stream"], true);
    // El tope de la respuesta del agente, mas la reserva para pensar.
    assert_eq!(rec[0].cuerpo["max_tokens"], 1024 + RESERVA_PENSAR);
}

#[test]
fn claude_llama_una_herramienta_y_devuelve_el_pensamiento_intacto() {
    let s = servidor(
        "/v1/messages",
        vec![
            guion_claude(
                vec![
                    c_pensar(0, "Un rectangulo rojo."),
                    c_texto(1, &["Lo dibujo."]),
                    c_herramienta(2, "toolu_01", "dibujar_en_lienzo", DIBUJO),
                ],
                "tool_use",
            ),
            guion_claude(
                vec![c_texto(0, &["Listo: un rectángulo rojo."])],
                "end_turn",
            ),
        ],
    );
    let m = claude(&s.url);
    let (r, ej, ev, _) = turno(&m, "dibuja un rectángulo rojo");
    let f = r.unwrap();
    assert_eq!(f.herramientas_hechas, 1);
    assert!(f.texto.contains("rectángulo rojo"));
    assert!(
        matches!(&ej.0[0], Llamada::Dibujar(figs) if figs.len() == 1 && figs[0].color.0 == "#e03131")
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, Evento::Llamando(n) if n == "dibujar_en_lienzo"))
    );
    let rec = s.recibidas.lock().unwrap();
    assert_eq!(rec.len(), 2);
    let primera = &rec[0].cuerpo;
    assert!(
        primera["tools"][0].get("strict").is_none(),
        "sin strict: con el Haiku dibujaba figuras vacias"
    );
    assert_eq!(primera["tools"][0]["eager_input_streaming"], true);
    let mensajes = rec[1].cuerpo["messages"].as_array().unwrap().clone();
    assert_eq!(mensajes.len(), 3);
    let asis = &mensajes[1];
    assert_eq!(asis["role"], "assistant");
    assert_eq!(asis["content"][0]["type"], "thinking");
    assert_eq!(asis["content"][0]["signature"], "FIRMA-ORIGINAL");
    assert_eq!(asis["content"][0]["thinking"], "Un rectangulo rojo.");
    assert_eq!(
        asis["content"][2]["input"]["elementos"][0]["tipo"],
        "rectangulo"
    );
    let res = &mensajes[2];
    assert_eq!(res["role"], "user");
    assert_eq!(res["content"][0]["type"], "tool_result");
    assert_eq!(res["content"][0]["tool_use_id"], "toolu_01");
    assert!(res["content"][0].get("is_error").is_none());
    // El prefijo estable (herramientas + sistema) es identico byte a byte.
    assert_eq!(rec[0].cuerpo["system"], rec[1].cuerpo["system"]);
    assert_eq!(rec[0].cuerpo["tools"], rec[1].cuerpo["tools"]);
}

#[test]
fn claude_entrada_invalida_no_se_ejecuta_y_se_corrige() {
    let s = servidor(
        "/v1/messages",
        vec![
            // Un tipo que no esta en la lista cerrada del esquema.
            guion_claude(
                vec![c_herramienta(
                    0,
                    "toolu_a",
                    "dibujar_en_lienzo",
                    r#"{"elementos":[{"tipo":"dragon","x":1,"y":1}]}"#,
                )],
                "tool_use",
            ),
            // JSON roto (la entrada llega sin validar del servidor).
            guion_claude(
                vec![c_herramienta(
                    0,
                    "toolu_b",
                    "dibujar_en_lienzo",
                    r#"{"elementos":[{"tipo":"#,
                )],
                "tool_use",
            ),
            guion_claude(
                vec![c_herramienta(0, "toolu_c", "dibujar_en_lienzo", DIBUJO)],
                "tool_use",
            ),
            guion_claude(vec![c_texto(0, &["Hecho."])], "end_turn"),
        ],
    );
    let m = claude(&s.url);
    let (r, ej, _, _) = turno(&m, "dibuja");
    let f = r.unwrap();
    assert_eq!(ej.0.len(), 1, "solo la buena se ejecuta");
    assert_eq!(f.reintentos, 2);
    let rec = s.recibidas.lock().unwrap();
    let ultimo_de = |i: usize| {
        rec[i].cuerpo["messages"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .clone()
    };
    let r1 = ultimo_de(1);
    assert_eq!(r1["content"][0]["is_error"], true);
    assert!(
        r1["content"][0]["content"]
            .as_str()
            .unwrap()
            .contains("dragon")
    );
    let r2 = ultimo_de(2);
    assert_eq!(r2["content"][0]["is_error"], true);
    let motivo = r2["content"][0]["content"].as_str().unwrap();
    assert!(
        motivo.contains("JSON") || motivo.contains("cortada"),
        "{motivo}"
    );
}

#[test]
fn claude_max_tokens_con_herramienta_a_medias_no_se_ejecuta() {
    let s = servidor(
        "/v1/messages",
        vec![
            guion_claude(
                vec![c_herramienta(
                    0,
                    "toolu_x",
                    "dibujar_en_lienzo",
                    r#"{"elementos":[{"tipo":"rectangulo","x":1,"y":1}"#,
                )],
                "max_tokens",
            ),
            guion_claude(
                vec![c_herramienta(0, "toolu_y", "dibujar_en_lienzo", DIBUJO)],
                "tool_use",
            ),
            guion_claude(vec![c_texto(0, &["Hecho."])], "end_turn"),
        ],
    );
    let m = claude(&s.url);
    let (r, ej, _, _) = turno(&m, "dibuja mucho");
    let f = r.unwrap();
    assert_eq!(f.reintentos, 1);
    assert_eq!(ej.0.len(), 1);
    let rec = s.recibidas.lock().unwrap();
    let segundo = rec[1].cuerpo["messages"].to_string();
    assert!(segundo.contains("salio cortada"), "{segundo}");
    assert!(
        !segundo.contains("toolu_x"),
        "la llamada cortada no queda en la conversacion"
    );
}

#[test]
fn claude_rechazo_no_ejecuta_nada_y_lo_dice() {
    let s = servidor(
        "/v1/messages",
        vec![sse(200, &{
            let mut e = vec![c_inicio()];
            e.extend(c_herramienta(
                0,
                "toolu_r",
                "dibujar_en_lienzo",
                r#"{"elementos":"#,
            ));
            e.push(json!({"type": "message_delta", "delta": {"stop_reason": "refusal", "stop_details": {"category": "cyber"}}, "usage": {"output_tokens": 3}}));
            e.push(json!({"type": "message_stop"}));
            e
        })],
    );
    let m = claude(&s.url);
    let (r, ej, ev, _) = turno(&m, "algo raro");
    let f = r.unwrap();
    assert!(ej.0.is_empty());
    assert!(
        f.texto.contains("no quiso") && f.texto.contains("filtro") && !f.texto.contains("cyber"),
        "{}",
        f.texto
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, Evento::Texto(t) if t.contains("no quiso")))
    );
}

#[test]
fn claude_401_avisa_sin_reintentar() {
    let s = servidor(
        "/v1/messages",
        vec![crudo(
            401,
            r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#,
        )],
    );
    let m = claude(&s.url);
    let e = m
        .completar(&Peticion::nueva(historial("hola")), &mut |_| Seguir::Si)
        .unwrap_err();
    assert!(
        matches!(&e, ErrorIa::Nube(t) if t.contains("no acepta la clave") && t.contains("Ajustes")),
        "{e}"
    );
    assert_eq!(s.recibidas.lock().unwrap().len(), 1);
    // Y la clave no sale en el aviso.
    assert!(!e.to_string().contains("CLAVEFALSA"));
    // Ni codigos ni JSON en lo que ve el usuario.
    let t = crate::agente::para_el_usuario(&e);
    assert!(
        !t.contains("401") && !t.contains('{') && !t.contains("x-api-key"),
        "{t}"
    );
}

#[test]
fn claude_sin_saldo_es_un_400_con_aviso_propio() {
    let s = servidor(
        "/v1/messages",
        vec![crudo(
            400,
            r#"{"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the Anthropic API."}}"#,
        )],
    );
    let e = claude(&s.url)
        .completar(&Peticion::nueva(historial("hola")), &mut |_| Seguir::Si)
        .unwrap_err();
    assert!(e.to_string().contains("no tiene saldo"), "{e}");
}

#[test]
fn claude_429_y_luego_bien_reintenta() {
    let mut espera = crudo(
        429,
        r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#,
    );
    espera.cabeceras = "retry-after: 0\r\n".into();
    let s = servidor(
        "/v1/messages",
        vec![espera, guion_claude(vec![c_texto(0, &["ok"])], "end_turn")],
    );
    let r = claude(&s.url)
        .completar(&Peticion::nueva(historial("hola")), &mut |_| Seguir::Si)
        .unwrap();
    assert_eq!(r.texto, "ok");
    assert_eq!(s.recibidas.lock().unwrap().len(), 2);
}

#[test]
fn claude_500_siempre_se_rinde_tras_dos_reintentos() {
    let err = crudo(
        500,
        r#"{"type":"error","error":{"type":"api_error","message":"boom"}}"#,
    );
    let s = servidor(
        "/v1/messages",
        vec![err.clone(), err.clone(), err.clone(), err],
    );
    let e = claude(&s.url)
        .completar(&Peticion::nueva(historial("hola")), &mut |_| Seguir::Si)
        .unwrap_err();
    assert!(e.to_string().contains("saturado"), "{e}");
    assert_eq!(s.recibidas.lock().unwrap().len(), 3);
}

#[test]
fn claude_saturado_a_mitad_antes_de_escribir_reintenta() {
    let saturado = sse(
        200,
        &[
            c_inicio(),
            json!({"type": "error", "error": {"type": "overloaded_error", "message": "Overloaded"}}),
        ],
    );
    let s = servidor(
        "/v1/messages",
        vec![
            saturado,
            guion_claude(vec![c_texto(0, &["bien"])], "end_turn"),
        ],
    );
    let r = claude(&s.url)
        .completar(&Peticion::nueva(historial("hola")), &mut |_| Seguir::Si)
        .unwrap();
    assert_eq!(r.texto, "bien");
}

#[test]
fn claude_saturado_tras_escribir_no_duplica() {
    let mut e = vec![c_inicio()];
    e.extend(c_texto(0, &["medio"]));
    e.push(
        json!({"type": "error", "error": {"type": "overloaded_error", "message": "Overloaded"}}),
    );
    let s = servidor(
        "/v1/messages",
        vec![
            sse(200, &e),
            guion_claude(vec![c_texto(0, &["otra"])], "end_turn"),
        ],
    );
    let r = claude(&s.url).completar(&Peticion::nueva(historial("hola")), &mut |_| Seguir::Si);
    assert!(r.is_err());
    assert_eq!(s.recibidas.lock().unwrap().len(), 1);
}

#[test]
fn claude_400_de_esquema_no_se_reintenta() {
    let s = servidor(
        "/v1/messages",
        vec![
            crudo(
                400,
                r#"{"type":"error","error":{"type":"invalid_request_error","message":"tools.0.input_schema: bad schema"}}"#,
            ),
            guion_claude(vec![c_texto(0, &["ok"])], "end_turn"),
        ],
    );
    let mut p = Peticion::nueva(historial("hola"));
    p.herramientas = catalogo();
    let e = claude(&s.url)
        .completar(&p, &mut |_| Seguir::Si)
        .unwrap_err();
    assert!(!e.to_string().contains("schema"), "nada tecnico: {e}");
    assert_eq!(s.recibidas.lock().unwrap().len(), 1);
}

#[test]
fn claude_flujo_ilegible_es_error_de_protocolo() {
    let s = servidor(
        "/v1/messages",
        vec![crudo(200, "event: x\ndata: {roto\n\n")],
    );
    let e = claude(&s.url)
        .completar(&Peticion::nueva(historial("hola")), &mut |_| Seguir::Si)
        .unwrap_err();
    assert!(matches!(e, ErrorIa::Protocolo(_)));
}

#[test]
fn parar_corta_y_no_ejecuta() {
    let s = servidor(
        "/v1/messages",
        vec![guion_claude(
            vec![c_texto(0, &["uno ", "dos ", "tres"])],
            "end_turn",
        )],
    );
    let r = claude(&s.url)
        .completar(&Peticion::nueva(historial("hola")), &mut |_| Seguir::Parar)
        .unwrap();
    assert_eq!(r.fin, Fin::Cortado);
}

// ===========================================================================
// DeepSeek

#[test]
fn deepseek_razona_llama_y_devuelve_el_razonamiento() {
    let s = servidor(
        "/chat/completions",
        vec![
            guion_deepseek(vec![
                d_trozo(
                    json!({"role": "assistant", "reasoning_content": "Pide un "}),
                    None,
                ),
                d_trozo(json!({"reasoning_content": "rectangulo."}), None),
                d_trozo(
                    json!({"tool_calls": [{"index": 0, "id": "call_9", "type": "function",
                    "function": {"name": "dibujar_en_lienzo", "arguments": "{\"elementos\":[{\"tipo\":\"rectangulo\","}}]}),
                    None,
                ),
                d_trozo(
                    json!({"tool_calls": [{"index": 0, "function": {"arguments": "\"x\":5,\"y\":5,\"color\":\"azul\"}]}"}}]}),
                    Some("tool_calls"),
                ),
            ]),
            guion_deepseek(vec![d_trozo(json!({"content": "Dibujado."}), Some("stop"))]),
        ],
    );
    let m = deepseek(&s.url);
    let (r, ej, _, _) = turno(&m, "un rectangulo azul");
    let f = r.unwrap();
    assert_eq!(f.texto, "Dibujado.");
    assert_eq!(ej.0.len(), 1);
    let rec = s.recibidas.lock().unwrap();
    assert!(
        rec[0]
            .cabeceras
            .contains("authorization: bearer sk-deepseekfalsa")
    );
    assert_eq!(rec[0].cuerpo["model"], "deepseek-flash");
    let asis = &rec[1].cuerpo["messages"][2];
    assert_eq!(asis["role"], "assistant");
    assert_eq!(asis["reasoning_content"], "Pide un rectangulo.");
    assert_eq!(asis["tool_calls"][0]["id"], "call_9");
    assert_eq!(rec[1].cuerpo["messages"][3]["role"], "tool");
}

#[test]
fn deepseek_400_por_razonamiento_sigue_sin_pensar() {
    let s = servidor(
        "/chat/completions",
        vec![
            crudo(
                400,
                r#"{"error":{"message":"Missing reasoning_content field in the assistant message","type":"invalid_request_error"}}"#,
            ),
            guion_deepseek(vec![d_trozo(json!({"content": "ok"}), Some("stop"))]),
        ],
    );
    let r = deepseek(&s.url)
        .completar(&Peticion::nueva(historial("hola")), &mut |_| Seguir::Si)
        .unwrap();
    assert_eq!(r.texto, "ok");
    let rec = s.recibidas.lock().unwrap();
    assert_eq!(rec[0].cuerpo["thinking"]["type"], "enabled");
    assert_eq!(rec[1].cuerpo["thinking"]["type"], "disabled");
}

#[test]
fn deepseek_sin_saldo_y_clave_mala() {
    let s = servidor(
        "/chat/completions",
        vec![
            crudo(
                402,
                r#"{"error":{"message":"Insufficient Balance","type":"unknown_error"}}"#,
            ),
            crudo(
                401,
                r#"{"error":{"message":"Authentication Fails","type":"authentication_error"}}"#,
            ),
        ],
    );
    let m = deepseek(&s.url);
    let e1 = m
        .completar(&Peticion::nueva(historial("a")), &mut |_| Seguir::Si)
        .unwrap_err();
    assert!(e1.to_string().contains("no tiene saldo"), "{e1}");
    let e2 = m
        .completar(&Peticion::nueva(historial("a")), &mut |_| Seguir::Si)
        .unwrap_err();
    assert!(e2.to_string().contains("no acepta la clave"), "{e2}");
}

#[test]
fn deepseek_503_y_luego_bien() {
    let s = servidor(
        "/chat/completions",
        vec![
            crudo(503, "{}"),
            guion_deepseek(vec![d_trozo(json!({"content": "ya"}), Some("stop"))]),
        ],
    );
    let r = deepseek(&s.url)
        .completar(&Peticion::nueva(historial("a")), &mut |_| Seguir::Si)
        .unwrap();
    assert_eq!(r.texto, "ya");
}

// ===========================================================================
// Probar la clave, respaldo y sin red

#[test]
fn probar_la_clave() {
    let s = servidor(
        "/v1/messages",
        vec![
            crudo(
                200,
                r#"{"id":"msg","type":"message","model":"claude-haiku-5-5","content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn"}"#,
            ),
            crudo(
                401,
                r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#,
            ),
        ],
    );
    let ok = probar_en(Proveedor::Claude, "sk-ant-buena-0123456789", &s.url).unwrap();
    assert!(ok.contains("funciona"));
    let mal = probar_en(Proveedor::Claude, "sk-ant-mala-0123456789", &s.url).unwrap_err();
    assert!(mal.contains("no acepta la clave"));
    assert!(probar_en(Proveedor::Claude, "  ", &s.url).is_err());
    let rec = s.recibidas.lock().unwrap();
    assert_eq!(rec[0].cuerpo["max_tokens"], 16);
}

#[test]
fn sin_red_avisa_y_no_se_queda_colgado() {
    // Un puerto donde no escucha nadie.
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "http://127.0.0.1:{}/v1/messages",
        l.local_addr().unwrap().port()
    );
    drop(l);
    let e = claude(&url)
        .completar(&Peticion::nueva(historial("hola")), &mut |_| Seguir::Si)
        .unwrap_err();
    assert!(e.to_string().contains("No hay conexion con Claude"), "{e}");
}

#[test]
fn sin_clave_lo_dice() {
    let e = MotorNube::nuevo(Proveedor::DeepSeek, "  ")
        .completar(&Peticion::nueva(historial("hola")), &mut |_| Seguir::Si)
        .unwrap_err();
    assert!(e.to_string().contains("Falta la clave de DeepSeek"));
}

#[test]
fn si_la_nube_falla_sigue_el_local() {
    let s = servidor(
        "/v1/messages",
        vec![crudo(
            401,
            r#"{"type":"error","error":{"type":"authentication_error","message":"x"}}"#,
        )],
    );
    let local: Arc<dyn Motor> = Arc::new(MotorGuionado::nuevo(vec![
        crate::motor::Respuesta::de_texto("respuesta local"),
        crate::motor::Respuesta::de_texto("otra local"),
    ]));
    let m = MotorConRespaldo::nuevo(Arc::new(claude(&s.url)), Some(local), "Claude Haiku 5.5");
    let mut visto = String::new();
    let r = m
        .completar(&Peticion::nueva(historial("hola")), &mut |t| {
            if let Trozo::Texto(x) = t {
                visto.push_str(x);
            }
            Seguir::Si
        })
        .unwrap();
    assert_eq!(r.texto, "respuesta local");
    assert!(
        visto.contains("no acepta la clave") && visto.contains("Sigo con el asistente local"),
        "{visto}"
    );
    // La siguiente va directa al local, sin otra peticion ni otro aviso.
    let r2 = m
        .completar(&Peticion::nueva(historial("hola")), &mut |_| Seguir::Si)
        .unwrap();
    assert_eq!(r2.texto, "otra local");
    assert_eq!(s.recibidas.lock().unwrap().len(), 1);
    // Sin local, el error llega tal cual.
    let solo = MotorConRespaldo::nuevo(
        Arc::new(MotorNube::nuevo(Proveedor::Claude, "")),
        None,
        "Claude",
    );
    assert!(matches!(
        solo.completar(&Peticion::nueva(historial("a")), &mut |_| Seguir::Si),
        Err(ErrorIa::Nube(_))
    ));
}

#[test]
fn la_clave_no_sale_por_debug() {
    let m = claude("http://127.0.0.1:1/");
    let d = format!("{m:?}");
    assert!(!d.contains("CLAVEFALSA0123456789"));
    assert!(d.contains("6789") && d.contains("sk-ant-"));
}

#[test]
fn proveedores_por_id() {
    for p in Proveedor::TODOS {
        assert_eq!(Proveedor::por_id(p.id()), Some(p));
    }
    assert_eq!(Proveedor::por_id("local"), None);
    assert_eq!(Proveedor::Claude.modelo(), "claude-haiku-5-5");
    assert_eq!(Proveedor::DeepSeek.modelo(), "deepseek-flash");
}

// ===========================================================================
// En vivo (solo a mano, con la clave en el entorno; nunca se pide)

fn en_vivo(p: Proveedor, var: &str) {
    let Ok(clave) = std::env::var(var) else {
        eprintln!("sin {var}: se salta");
        return;
    };
    println!("{}", probar(p, &clave).unwrap());
    let m = MotorNube::nuevo(p, clave);
    let (r, ej, _, h) = turno(
        &m,
        "Dibuja una casa sencilla con tejado rojo y puerta marron.",
    );
    let f = r.unwrap();
    println!(
        "{}: {} herramientas, {} reintentos, texto: {}",
        p.nombre(),
        f.herramientas_hechas,
        f.reintentos,
        f.texto
    );
    println!("mensajes: {}", h.len());
    assert!(!ej.0.is_empty(), "no dibujo nada");
}

#[test]
#[ignore = "en vivo: necesita ANTHROPIC_API_KEY"]
fn en_vivo_claude() {
    en_vivo(Proveedor::Claude, "ANTHROPIC_API_KEY");
}

#[test]
#[ignore = "en vivo: necesita DEEPSEEK_API_KEY"]
fn en_vivo_deepseek() {
    en_vivo(Proveedor::DeepSeek, "DEEPSEEK_API_KEY");
}

#[test]
fn la_nube_recibe_el_catalogo_entero_y_el_respaldo_solo_lo_que_toca() {
    assert!(MotorNube::nuevo(Proveedor::Claude, "x").catalogo_completo());
    assert!(!MotorGuionado::nuevo(vec![]).catalogo_completo());
    let r = MotorConRespaldo::nuevo(
        Arc::new(MotorNube::nuevo(Proveedor::DeepSeek, "x")),
        None,
        "DeepSeek",
    );
    assert!(r.catalogo_completo());
    let mut p = Peticion::nueva(historial("dibuja una estrella roja"));
    p.herramientas = catalogo();
    assert!(p.herramientas.len() > crate::seleccion::MAXIMO);
    let q = para_el_local(&p);
    assert!(q.herramientas.len() <= crate::seleccion::MAXIMO);
    assert!(
        q.herramientas
            .iter()
            .any(|h| h.nombre == crate::herramientas::DIBUJAR)
    );
}

#[test]
fn ninguna_va_estricta_y_los_nombres_valen() {
    let mut p = Peticion::nueva(historial("hola"));
    p.herramientas = catalogo();
    let c = claude::cuerpo(&p, &std::collections::HashMap::new(), 100);
    let tools = c["tools"].as_array().unwrap();
    // Las de la app y las dos de la web (`claude::herramientas_web`).
    assert_eq!(
        tools.len(),
        catalogo().len() + claude::herramientas_web().len()
    );
    assert!(tools.iter().all(|t| t.get("strict").is_none()));
    for t in tools {
        let n = t["name"].as_str().unwrap();
        assert!(
            n.len() <= 64
                && n.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "{n}"
        );
    }
}

// ===========================================================================
// Pensar, peticion mejorada y hoja de trabajo con la nube (ia-pensar, 13-oct)

const PLAN_NUBE: &str = "ENTIENDO: Dibujar un rectangulo azul.\nHARE: usar dibujar_en_lienzo.\nHERRAMIENTAS: dibujar_en_lienzo\nDATOS: azul";

fn turno_pensando(motor: &dyn Motor, pedido: &str) -> (agente::Final, Vec<Evento>) {
    let hoja = crate::hoja_trabajo::HojaDeTrabajo {
        objetivo: "Hacer el logo de Ana".into(),
        ..Default::default()
    };
    let o = agente::Opciones {
        planificar: true,
        pensar: true,
        hoja: Some(&hoja),
        ventana: 64_000,
        ..Default::default()
    };
    let mut h = historial(pedido);
    let mut ev = Vec::new();
    let f = agente::turno_con(
        motor,
        &mut h,
        &catalogo(),
        &mut Apunta::default(),
        &o,
        &mut |e| {
            ev.push(e);
            Seguir::Si
        },
    )
    .unwrap();
    (f, ev)
}

fn pensado_y_plan(ev: &[Evento]) -> (String, crate::pensar::Plan, usize) {
    let pensado: String = ev
        .iter()
        .filter_map(|e| {
            if let Evento::Pensando(s) = e {
                Some(s.as_str())
            } else {
                None
            }
        })
        .collect();
    let plan = ev
        .iter()
        .find_map(|e| {
            if let Evento::Peticion(p) = e {
                Some(p.clone())
            } else {
                None
            }
        })
        .expect("peticion mejorada");
    let listos = ev
        .iter()
        .filter(|e| matches!(e, Evento::PensoListo { .. }))
        .count();
    (pensado, plan, listos)
}

#[test]
fn claude_piensa_planea_y_actua_con_la_hoja() {
    let s = servidor(
        "/v1/messages",
        vec![
            guion_claude(
                vec![
                    c_pensar(0, "Quiere un rectangulo azul."),
                    c_texto(1, &[PLAN_NUBE]),
                ],
                "end_turn",
            ),
            guion_claude(
                vec![c_pensar(0, "Lo dibujo ya."), c_texto(1, &["Listo."])],
                "end_turn",
            ),
        ],
    );
    let (f, ev) = turno_pensando(&claude(&s.url), "un rectangulo azul");
    assert_eq!(f.texto, "Listo.");
    let (pensado, plan, listos) = pensado_y_plan(&ev);
    assert!(
        pensado.contains("Quiere un rectangulo azul.") && pensado.contains("Lo dibujo ya."),
        "{pensado}"
    );
    assert_eq!(plan.entiendo, "Dibujar un rectangulo azul.");
    assert!(plan.del_modelo);
    assert_eq!(listos, 2, "se pliega tras cada pensamiento");
    // El texto del plan no salio como respuesta.
    assert!(
        !ev.iter()
            .any(|e| matches!(e, Evento::Texto(t) if t.contains("ENTIENDO")))
    );
    let rec = s.recibidas.lock().unwrap();
    let sistema = rec[1].cuerpo["system"].to_string();
    assert!(
        sistema.contains("Hacer el logo de Ana") && sistema.contains("Peticion mejorada"),
        "{sistema}"
    );
}

#[test]
fn deepseek_razona_planea_y_actua_con_la_hoja() {
    let s = servidor(
        "/chat/completions",
        vec![
            guion_deepseek(vec![
                d_trozo(
                    json!({"role": "assistant", "reasoning_content": "Quiere un "}),
                    None,
                ),
                d_trozo(json!({"reasoning_content": "rectangulo azul."}), None),
                d_trozo(json!({"content": PLAN_NUBE}), Some("stop")),
            ]),
            guion_deepseek(vec![d_trozo(json!({"content": "Listo."}), Some("stop"))]),
        ],
    );
    let (f, ev) = turno_pensando(&deepseek(&s.url), "un rectangulo azul");
    assert_eq!(f.texto, "Listo.");
    let (pensado, plan, listos) = pensado_y_plan(&ev);
    assert_eq!(pensado, "Quiere un rectangulo azul.");
    assert_eq!(plan.entiendo, "Dibujar un rectangulo azul.");
    assert_eq!(listos, 1);
    let rec = s.recibidas.lock().unwrap();
    let sistema = rec[1].cuerpo["messages"][0]["content"].to_string();
    assert!(
        sistema.contains("Hacer el logo de Ana") && sistema.contains("Peticion mejorada"),
        "{sistema}"
    );
}

/// **Un turno simple no piensa en la nube** (15-oct, `crate::complejidad`):
/// Claude va con `thinking: disabled` y DeepSeek tambien, sin la reserva de
/// tokens del pensamiento; uno complejo, como siempre.
#[test]
fn un_turno_simple_no_piensa_en_la_nube() {
    let mut p = crate::motor::Peticion::nueva(vec![Mensaje::usuario("dibuja un circulo")]);
    p.max_tokens = 1000;
    for pensar in [false, true] {
        let c = claude("http://127.0.0.1:1").pensando(pensar).cuerpo(&p);
        let d = deepseek("http://127.0.0.1:1").pensando(pensar).cuerpo(&p);
        if pensar {
            assert_eq!(c["thinking"]["type"], "adaptive");
            assert_eq!(d["thinking"]["type"], "enabled");
            assert_eq!(c["max_tokens"], json!(1000 + RESERVA_PENSAR));
        } else {
            assert_eq!(c["thinking"]["type"], "disabled");
            assert_eq!(d["thinking"]["type"], "disabled");
            assert!(d.get("reasoning_effort").is_none());
            assert_eq!(c["max_tokens"], json!(1000), "sin reserva para pensar");
            assert_eq!(d["max_tokens"], json!(1000));
            // El esfuerzo no baja: eso es `ligero`.
            assert_ne!(c["output_config"]["effort"], "low");
        }
    }
}

/// **La web en vivo** (20-oct-2026): con la clave guardada por la app
/// (DPAPI; nunca se imprime) y Haiku, una pregunta que solo se contesta
/// buscando. Dice si la API acepto las herramientas web y si busco.
#[test]
#[ignore = "en vivo: usa la clave de Claude guardada en este equipo"]
fn en_vivo_claude_busca_en_la_web() {
    let Some(clave) = claves::leer(Proveedor::Claude) else {
        eprintln!("sin clave de Claude guardada: se salta");
        return;
    };
    let m = MotorNube::nuevo(Proveedor::Claude, clave).pensando(false);
    let mut buscadas = Vec::new();
    let p = Peticion::nueva(vec![Mensaje::usuario(
        "Busca en la web: ¿cual es la ultima version publicada de Rust estable? Responde en una linea con la fuente.",
    )]);
    let r = m
        .completar(&p, &mut |t| {
            if let Trozo::Llamando { nombre } = t {
                buscadas.push(nombre.clone());
            }
            Seguir::Si
        })
        .expect("la API acepto la peticion con la web");
    println!("herramientas de servidor usadas: {buscadas:?}");
    println!("respuesta: {}", r.texto);
    if web_disponible() {
        assert!(
            buscadas.iter().any(|n| n == claude::BUSCAR_EN_LA_WEB),
            "no busco en la web"
        );
    } else {
        println!("la cuenta no tiene la web activada: contesto sin ella");
    }
    assert!(!r.texto.trim().is_empty());
}
