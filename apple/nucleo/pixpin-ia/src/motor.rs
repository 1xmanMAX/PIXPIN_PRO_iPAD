//! **El contrato con el motor**: lo unico que el resto de PixPin ve del
//! modelo.
//!
//! La conversacion sigue la forma de la API de chat de OpenAI (mensajes con
//! rol, partes de texto e imagen, llamadas a herramientas con `id`), porque es
//! la que hablan `llama-server`, LiteRT-LM y casi todo lo demas: cambiar de
//! motor —por ejemplo a uno en el mismo proceso, o al de Android— no toca ni
//! el bucle del agente ni el chat.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde_json::{Value, json};

use crate::ErrorIa;
use crate::herramientas::Herramienta;

/// Quien habla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rol {
    Sistema,
    Usuario,
    Asistente,
    /// El resultado de una herramienta, devuelto al modelo.
    Herramienta,
}

impl Rol {
    pub fn nombre(self) -> &'static str {
        match self {
            Rol::Sistema => "system",
            Rol::Usuario => "user",
            Rol::Asistente => "assistant",
            Rol::Herramienta => "tool",
        }
    }
}

/// Un trozo de un mensaje.
#[derive(Debug, Clone, PartialEq)]
pub enum Parte {
    Texto(String),
    /// Una imagen ya codificada (PNG o JPEG) y su tipo MIME. El chat la
    /// reduce antes (ver el documento: ~768 px de lado bastan y ahorran
    /// segundos de vision).
    Imagen {
        bytes: Vec<u8>,
        mime: String,
    },
}

/// Una llamada a herramienta tal como la escribio el modelo, sin validar.
#[derive(Debug, Clone, PartialEq)]
pub struct LlamadaCruda {
    pub id: String,
    pub nombre: String,
    /// El JSON de los argumentos, como texto: puede venir roto.
    pub argumentos: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mensaje {
    pub rol: Rol,
    pub partes: Vec<Parte>,
    /// Solo en mensajes del asistente: las herramientas que pidio.
    pub llamadas: Vec<LlamadaCruda>,
    /// Solo en mensajes de herramienta: a que llamada responde.
    pub id_llamada: Option<String>,
}

impl Mensaje {
    pub fn texto(rol: Rol, texto: impl Into<String>) -> Mensaje {
        Mensaje {
            rol,
            partes: vec![Parte::Texto(texto.into())],
            llamadas: Vec::new(),
            id_llamada: None,
        }
    }

    pub fn sistema(texto: impl Into<String>) -> Mensaje {
        Mensaje::texto(Rol::Sistema, texto)
    }

    pub fn usuario(texto: impl Into<String>) -> Mensaje {
        Mensaje::texto(Rol::Usuario, texto)
    }

    /// Un mensaje del usuario con una imagen adjunta (la imagen va primero,
    /// como recomienda Qwen).
    pub fn usuario_con_imagen(texto: impl Into<String>, bytes: Vec<u8>, mime: &str) -> Mensaje {
        Mensaje {
            rol: Rol::Usuario,
            partes: vec![
                Parte::Imagen {
                    bytes,
                    mime: mime.to_string(),
                },
                Parte::Texto(texto.into()),
            ],
            llamadas: Vec::new(),
            id_llamada: None,
        }
    }

    pub fn resultado(id_llamada: &str, texto: impl Into<String>) -> Mensaje {
        Mensaje {
            rol: Rol::Herramienta,
            partes: vec![Parte::Texto(texto.into())],
            llamadas: Vec::new(),
            id_llamada: Some(id_llamada.to_string()),
        }
    }

    /// Todo el texto del mensaje junto.
    pub fn texto_entero(&self) -> String {
        self.partes
            .iter()
            .filter_map(|p| match p {
                Parte::Texto(t) => Some(t.as_str()),
                Parte::Imagen { .. } => None,
            })
            .collect()
    }

    pub fn tiene_imagen(&self) -> bool {
        self.partes
            .iter()
            .any(|p| matches!(p, Parte::Imagen { .. }))
    }

    /// El mensaje en la forma de la API de chat de OpenAI.
    pub fn a_json(&self) -> Value {
        let mut m = serde_json::Map::new();
        m.insert("role".into(), json!(self.rol.nombre()));
        let contenido = if self.tiene_imagen() {
            Value::Array(
                self.partes
                    .iter()
                    .map(|p| match p {
                        Parte::Texto(t) => json!({"type": "text", "text": t}),
                        Parte::Imagen { bytes, mime } => json!({
                            "type": "image_url",
                            "image_url": {"url": format!("data:{mime};base64,{}", crate::base64::codificar(bytes))}
                        }),
                    })
                    .collect(),
            )
        } else {
            json!(self.texto_entero())
        };
        m.insert("content".into(), contenido);
        if !self.llamadas.is_empty() {
            m.insert(
                "tool_calls".into(),
                Value::Array(
                    self.llamadas
                        .iter()
                        .map(|l| {
                            json!({"id": l.id, "type": "function",
                                   "function": {"name": l.nombre, "arguments": l.argumentos}})
                        })
                        .collect(),
                ),
            );
        }
        if let Some(id) = &self.id_llamada {
            m.insert("tool_call_id".into(), json!(id));
        }
        Value::Object(m)
    }
}

/// Lo que se le pide al motor en una vuelta.
#[derive(Debug, Clone)]
pub struct Peticion {
    pub mensajes: Vec<Mensaje>,
    pub herramientas: Vec<Herramienta>,
    pub max_tokens: u32,
    pub temperatura: f32,
    /// Qwen3.5 piensa en voz alta si se le deja; en los modelos pequenos eso
    /// gasta tokens y a veces entra en bucle (lo avisa su propia ficha). Por
    /// omision, apagado.
    pub pensar: bool,
}

impl Peticion {
    pub fn nueva(mensajes: Vec<Mensaje>) -> Peticion {
        Peticion {
            mensajes,
            herramientas: Vec::new(),
            max_tokens: 1024,
            temperatura: 0.7,
            pensar: false,
        }
    }

    /// El cuerpo JSON para `/v1/chat/completions`, con los ajustes que
    /// recomienda la ficha de Qwen3.5 para el modo sin pensar.
    pub fn a_json(&self, en_vivo: bool) -> Value {
        let mut c = json!({
            "messages": self.mensajes.iter().map(Mensaje::a_json).collect::<Vec<_>>(),
            "max_tokens": self.max_tokens,
            "temperature": self.temperatura,
            "top_p": 0.8,
            "top_k": 20,
            "presence_penalty": 1.5,
            "stream": en_vivo,
            // Que la ranura guarde lo leido: la vuelta siguiente del mismo
            // turno (y la siguiente pregunta) solo lee lo nuevo.
            "cache_prompt": true,
            "chat_template_kwargs": {"enable_thinking": self.pensar},
        });
        if en_vivo {
            c["stream_options"] = json!({"include_usage": true});
        }
        if !self.herramientas.is_empty() {
            c["tools"] = Value::Array(self.herramientas.iter().map(Herramienta::a_json).collect());
        }
        c
    }
}

/// Un trozo de la respuesta mientras llega.
#[derive(Debug, Clone, PartialEq)]
pub enum Trozo {
    Texto(String),
    /// El razonamiento, si se pidio pensar. El chat puede plegarlo.
    Pensando(String),
    /// Empezo a escribir una llamada a herramienta (para ensenar «dibujando…»).
    Llamando {
        nombre: String,
    },
}

/// Si seguir escuchando o cortar (el usuario pulso «parar»).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seguir {
    Si,
    Parar,
}

/// Por que acabo la respuesta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fin {
    Natural,
    Herramientas,
    /// Se acabaron los `max_tokens`.
    Longitud,
    Cortado,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Medidas {
    pub tokens_entrada: u32,
    pub tokens_salida: u32,
    pub tokens_por_segundo_lectura: f32,
    pub tokens_por_segundo_escritura: f32,
    /// Tokens de la entrada que salieron de la cache (Claude
    /// `cache_read_input_tokens`, DeepSeek `prompt_cache_hit_tokens`,
    /// llama.cpp `timings.cache_n`): lo que NO hubo que volver a leer.
    pub tokens_cache_leidos: u32,
    /// Tokens que se escribieron en la cache en esta peticion (Claude).
    pub tokens_cache_escritos: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Respuesta {
    pub texto: String,
    pub pensamiento: String,
    pub llamadas: Vec<LlamadaCruda>,
    pub fin: Fin,
    pub medidas: Medidas,
}

impl Respuesta {
    pub fn de_texto(texto: impl Into<String>) -> Respuesta {
        Respuesta {
            texto: texto.into(),
            pensamiento: String::new(),
            llamadas: Vec::new(),
            fin: Fin::Natural,
            medidas: Medidas::default(),
        }
    }

    pub fn de_llamada(nombre: &str, argumentos: &str) -> Respuesta {
        Respuesta {
            texto: String::new(),
            pensamiento: String::new(),
            llamadas: vec![LlamadaCruda {
                id: format!("llamada-{nombre}"),
                nombre: nombre.to_string(),
                argumentos: argumentos.to_string(),
            }],
            fin: Fin::Herramientas,
            medidas: Medidas::default(),
        }
    }
}

/// **El motor.** Bloquea hasta acabar: se llama desde un hilo de fondo, y
/// `al_trozo` recibe la respuesta a medida que sale (en ese mismo hilo; el
/// chat la reenvia a su ventana con un mensaje).
pub trait Motor: Send + Sync {
    fn completar(
        &self,
        peticion: &Peticion,
        al_trozo: &mut dyn FnMut(&Trozo) -> Seguir,
    ) -> Result<Respuesta, ErrorIa>;

    /// Soltar la memoria ya (cerrar el modelo). Se vuelve a cargar solo en la
    /// siguiente peticion.
    fn soltar(&self) {}

    /// Dejarlo listo para contestar sin esperar (cargar el modelo local).
    /// Bloquea: se llama en un hilo de fondo. Por omision no hace nada (la
    /// nube no tiene nada que cargar).
    fn precalentar(&self) {}

    /// Si este motor recibe el catalogo ENTERO en cada turno en vez de las
    /// pocas herramientas de `seleccion::para_el_pedido`. Los modelos de la
    /// nube eligen bien entre todas, y un catalogo que no cambia de un turno
    /// a otro es un prefijo que se cachea (ver `nube`). Los locales, no.
    fn catalogo_completo(&self) -> bool {
        false
    }

    /// **Si ve imagenes** (15-oct, los ojos del lienzo: `ojos.rs`). Claude
    /// y DeepSeek si; el local, por ahora no (el usuario, 15-oct: primero la
    /// nube, el local no se toca). Sin vision, el turno no ofrece
    /// `ver_lienzo` ni revisa lo dibujado.
    fn ve_imagenes(&self) -> bool {
        false
    }

    /// **Si es el modelo local** (llama.cpp en este PC). Esta congelado por
    /// decision del usuario (15-oct): lo que se le anada a la nube (la
    /// revision con rubrica, 22-oct) no se le enciende aunque un dia vea
    /// imagenes. Por omision, no.
    fn es_local(&self) -> bool {
        false
    }

    /// **Cuantas peticiones a la vez** aguanta (15-oct: una lamina se
    /// secciona y cada zona va a su trabajador en paralelo). Claude, 8;
    /// DeepSeek, 3 (pocos creditos y limites mas bajos); el local, 1.
    fn a_la_vez(&self) -> usize {
        1
    }
}

/// Un motor de mentira que contesta lo que se le dice, en orden. Para las
/// pruebas del agente y del chat sin bajar nada.
pub struct MotorGuionado {
    guion: Mutex<VecDeque<Respuesta>>,
    /// Lo que se le fue pidiendo, para comprobarlo despues.
    pub recibido: Mutex<Vec<Peticion>>,
    /// Si dice que ve imagenes ([`Motor::ve_imagenes`]).
    pub vision: bool,
    /// Si hace de motor de la nube ([`Motor::catalogo_completo`]): las
    /// pruebas de lo que solo tiene la nube (preguntar al usuario).
    pub nube: bool,
}

impl MotorGuionado {
    pub fn nuevo(guion: Vec<Respuesta>) -> MotorGuionado {
        MotorGuionado {
            guion: Mutex::new(guion.into()),
            recibido: Mutex::new(Vec::new()),
            vision: false,
            nube: false,
        }
    }

    /// El mismo, como si fuera de la nube.
    pub fn como_nube(mut self) -> MotorGuionado {
        self.nube = true;
        self
    }

    /// El mismo, diciendo que ve imagenes.
    pub fn con_vision(mut self) -> MotorGuionado {
        self.vision = true;
        self
    }
}

impl Motor for MotorGuionado {
    fn catalogo_completo(&self) -> bool {
        self.nube
    }

    fn ve_imagenes(&self) -> bool {
        self.vision
    }

    fn completar(
        &self,
        peticion: &Peticion,
        al_trozo: &mut dyn FnMut(&Trozo) -> Seguir,
    ) -> Result<Respuesta, ErrorIa> {
        self.recibido
            .lock()
            .map_err(|_| ErrorIa::Protocolo("candado envenenado".into()))?
            .push(peticion.clone());
        let r = self
            .guion
            .lock()
            .map_err(|_| ErrorIa::Protocolo("candado envenenado".into()))?
            .pop_front()
            .ok_or_else(|| ErrorIa::Protocolo("el guion se acabo".into()))?;
        // Lo pensado sale antes que lo dicho, a trozos, como de verdad.
        for palabra in r.pensamiento.split_inclusive(' ') {
            if al_trozo(&Trozo::Pensando(palabra.to_string())) == Seguir::Parar {
                return Ok(Respuesta {
                    fin: Fin::Cortado,
                    ..r
                });
            }
        }
        for palabra in r.texto.split_inclusive(' ') {
            if al_trozo(&Trozo::Texto(palabra.to_string())) == Seguir::Parar {
                return Ok(Respuesta {
                    fin: Fin::Cortado,
                    ..r
                });
            }
        }
        for l in &r.llamadas {
            al_trozo(&Trozo::Llamando {
                nombre: l.nombre.clone(),
            });
        }
        Ok(r)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_mensaje_con_imagen_va_como_lista_de_partes() {
        let m = Mensaje::usuario_con_imagen("dibujala", vec![1, 2, 3], "image/png");
        let j = m.a_json();
        assert_eq!(j["role"], "user");
        assert_eq!(j["content"][0]["type"], "image_url");
        assert_eq!(
            j["content"][0]["image_url"]["url"],
            "data:image/png;base64,AQID"
        );
        assert_eq!(j["content"][1]["text"], "dibujala");
    }

    #[test]
    fn un_mensaje_de_texto_va_como_cadena() {
        assert_eq!(Mensaje::usuario("hola").a_json()["content"], "hola");
    }

    #[test]
    fn el_resultado_de_una_herramienta_lleva_su_id() {
        let j = Mensaje::resultado("abc", "hecho").a_json();
        assert_eq!(j["role"], "tool");
        assert_eq!(j["tool_call_id"], "abc");
    }

    #[test]
    fn la_peticion_apaga_el_pensar_y_pide_el_uso_en_vivo() {
        let p = Peticion::nueva(vec![Mensaje::usuario("hola")]);
        let j = p.a_json(true);
        assert_eq!(j["chat_template_kwargs"]["enable_thinking"], false);
        assert_eq!(j["stream_options"]["include_usage"], true);
        assert!(j.get("tools").is_none());
    }
}
