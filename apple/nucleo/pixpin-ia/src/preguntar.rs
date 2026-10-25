//! **Preguntar al usuario y esperar** (22-oct-2026, hoja de ruta punto 4).
//!
//! El fallo que mas se mide en los modelos de hoy es preguntar de menos:
//! eligen una lectura y la hacen (CLAM, AskBench). Pero preguntar de mas
//! tambien cuesta, asi que la herramienta es estrecha, como el
//! `AskUserQuestion` de Claude Code: UNA pregunta concreta (dos como mucho
//! por llamada), de 2 a 4 opciones con etiqueta corta y descripcion, la
//! recomendada primero, y la app anade siempre «Otro…» para que escriba lo
//! suyo. El bucle (`agente::vueltas`) se para en la llamada: el
//! [`crate::agente::Ejecutor::preguntar`] ensena la tarjeta en la burbuja y
//! no vuelve hasta que el usuario elige, escribe o se acaba el tiempo; lo
//! contestado vuelve al modelo como resultado de la herramienta.
//!
//! Cuando preguntar lo dicen las instrucciones (`agente::instrucciones`) y
//! la descripcion de aqui: solo si la respuesta cambia el resultado y no es
//! barato deshacerlo; si no, decidir algo razonable y decirlo en una linea.
//! Lo que borra o envia NO pasa por aqui: eso es la confirmacion de su
//! herramienta (`acciones`, «¿Sí o No?»).

use serde_json::{Value, json};

use crate::herramientas::Herramienta;

pub const NOMBRE: &str = "preguntar_usuario";

/// Cuantas preguntas por llamada como mucho (una es lo normal: el usuario
/// contesta mejor una cosa cada vez).
pub const MAX_PREGUNTAS: usize = 2;
/// Opciones por pregunta: menos de dos no es elegir; mas de cuatro ya no se
/// lee de un vistazo en la burbuja.
pub const MIN_OPCIONES: usize = 2;
pub const MAX_OPCIONES: usize = 4;
/// Lo largo de cada cosa (letras): la tarjeta es estrecha.
pub const LARGO_PREGUNTA: usize = 200;
pub const LARGO_ETIQUETA: usize = 40;
pub const LARGO_DESCRIPCION: usize = 120;

/// Una opcion de la tarjeta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opcion {
    /// Lo que pone el boton («Solo los títulos»).
    pub etiqueta: String,
    /// Que pasa si se elige (una frase corta, debajo del boton).
    pub descripcion: String,
}

/// Una pregunta con sus opciones (la primera es la recomendada).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pregunta {
    pub texto: String,
    pub opciones: Vec<Opcion>,
}

/// Lo que contesto el usuario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Contestada {
    /// Pulso la opcion `n` (desde 0).
    Opcion(usize),
    /// Escribio lo suyo («Otro…», o contesto escribiendo en el chat).
    Texto(String),
    /// Nadie contesto: paso el tiempo, se paro el turno o aqui no hay a
    /// quien preguntar (Flow Launcher, una automatizacion).
    SinRespuesta,
}

/// **La herramienta como la ve el modelo.**
pub fn herramienta() -> Herramienta {
    Herramienta {
        nombre: NOMBRE.into(),
        descripcion: concat!(
            "Hace al usuario una pregunta con opciones y espera su respuesta, que te llega como resultado. ",
            "Úsala solo cuando el pedido admite lecturas que dan resultados distintos, equivocarse cuesta (un trabajo largo, sobrescribir lo que hay, gastar) ",
            "y ni el lienzo, ni la conversación, ni los adjuntos lo aclaran. ",
            "No la uses para lo que se deshace fácil (decide y di lo que asumiste), si el usuario dijo «tú decide», ni para confirmar algo que borra o envía: eso lo pregunta su propia herramienta. ",
            "Una pregunta concreta, de 2 a 4 opciones cortas con la recomendada primero; la app añade «Otro…» para que escriba lo suyo. ",
            "Ejemplo: {\"preguntas\":[{\"pregunta\":\"¿Traduzco todos los textos del lienzo o solo los de la figura elegida?\",",
            "\"opciones\":[{\"etiqueta\":\"Solo la figura\",\"descripcion\":\"Los 6 textos de lo elegido\"},",
            "{\"etiqueta\":\"Todo el lienzo\",\"descripcion\":\"Los 23 textos\"}]}]}"
        )
        .into(),
        parametros: json!({
            "type": "object",
            "required": ["preguntas"],
            "properties": {
                "preguntas": {"type": "array", "minItems": 1, "maxItems": MAX_PREGUNTAS, "items": {
                    "type": "object",
                    "required": ["pregunta", "opciones"],
                    "properties": {
                        "pregunta": {"type": "string", "maxLength": LARGO_PREGUNTA, "description": "una sola cosa, concreta, terminada en ?"},
                        "opciones": {"type": "array", "minItems": MIN_OPCIONES, "maxItems": MAX_OPCIONES, "items": {
                            "type": "object",
                            "required": ["etiqueta"],
                            "properties": {
                                "etiqueta": {"type": "string", "maxLength": LARGO_ETIQUETA, "description": "de 1 a 5 palabras"},
                                "descripcion": {"type": "string", "maxLength": LARGO_DESCRIPCION, "description": "qué pasa si la elige"}
                            }
                        }}
                    }
                }}
            }
        }),
    }
}

fn cortar(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
    t.push('…');
    t
}

/// Una opcion: el objeto de la forma buena o, por si un modelo la simplifica,
/// una cadena sola (la etiqueta).
fn opcion(v: &Value) -> Option<Opcion> {
    let (etiqueta, descripcion) = match v {
        Value::String(s) => (s.as_str(), ""),
        Value::Object(o) => (
            o.get("etiqueta")
                .or_else(|| o.get("label"))
                .and_then(Value::as_str)
                .unwrap_or(""),
            o.get("descripcion")
                .or_else(|| o.get("description"))
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
        _ => return None,
    };
    let etiqueta = cortar(etiqueta, LARGO_ETIQUETA);
    (!etiqueta.is_empty()).then(|| Opcion {
        etiqueta,
        descripcion: cortar(descripcion, LARGO_DESCRIPCION),
    })
}

fn pregunta(v: &Value) -> Result<Pregunta, String> {
    let texto = v
        .get("pregunta")
        .or_else(|| v.get("question"))
        .and_then(Value::as_str)
        .map(|s| cortar(s, LARGO_PREGUNTA))
        .filter(|s| !s.is_empty())
        .ok_or("falta «pregunta»: una sola pregunta concreta")?;
    let mut opciones: Vec<Opcion> = v
        .get("opciones")
        .or_else(|| v.get("options"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(opcion).collect())
        .unwrap_or_default();
    // «Otro» lo pone la app: si el modelo lo anade, sobra.
    opciones.retain(|o| {
        !matches!(
            crate::acciones::normalizar(&o.etiqueta).trim_end_matches(['.', '…']),
            "otro" | "otra" | "otra cosa"
        )
    });
    let mut vistas = Vec::new();
    opciones.retain(|o| {
        let k = crate::acciones::normalizar(&o.etiqueta);
        let nueva = !vistas.contains(&k);
        vistas.push(k);
        nueva
    });
    if opciones.len() < MIN_OPCIONES {
        return Err(format!(
            "«{texto}» necesita de {MIN_OPCIONES} a {MAX_OPCIONES} opciones distintas con su «etiqueta» (sin «Otro»: lo pone la app). Si no hay opciones claras, no preguntes: decide lo más razonable y dilo."
        ));
    }
    if opciones.len() > MAX_OPCIONES {
        return Err(format!(
            "«{texto}» tiene {} opciones: deja las {MAX_OPCIONES} que más cambian el resultado, la recomendada primero",
            opciones.len()
        ));
    }
    Ok(Pregunta { texto, opciones })
}

/// **Lee los argumentos de la llamada.** Acepta tambien una pregunta sola
/// sin la lista (`{"pregunta":…, "opciones":…}`), que es lo que escribe un
/// modelo con prisa. El `Err` es el motivo, para que el modelo lo corrija.
pub fn leer(argumentos: &str) -> Result<Vec<Pregunta>, String> {
    let v = crate::herramientas::reparar_json(argumentos)
        .ok_or("los argumentos de preguntar_usuario no son JSON")?;
    let lista: Vec<Value> = match v.get("preguntas").or_else(|| v.get("questions")) {
        Some(Value::Array(a)) => a.clone(),
        Some(otro @ Value::Object(_)) => vec![otro.clone()],
        _ if v.get("pregunta").is_some() || v.get("question").is_some() => vec![v.clone()],
        _ => return Err(
            "falta «preguntas»: [{\"pregunta\": \"…?\", \"opciones\": [{\"etiqueta\": \"…\"}, …]}]"
                .into(),
        ),
    };
    if lista.is_empty() {
        return Err("«preguntas» esta vacia: pon una pregunta".into());
    }
    if lista.len() > MAX_PREGUNTAS {
        return Err(format!(
            "como mucho {MAX_PREGUNTAS} preguntas por llamada; mejor una: la que más cambia el resultado"
        ));
    }
    lista.iter().map(pregunta).collect()
}

/// **Lo que vuelve al modelo** de una pregunta contestada.
pub fn para_el_modelo(p: &Pregunta, c: &Contestada) -> String {
    match c {
        Contestada::Opcion(i) => match p.opciones.get(*i) {
            Some(o) if o.descripcion.is_empty() => format!(
                "«{}» → el usuario eligió «{}». Sigue con eso.",
                p.texto, o.etiqueta
            ),
            Some(o) => format!(
                "«{}» → el usuario eligió «{}» ({}). Sigue con eso.",
                p.texto, o.etiqueta, o.descripcion
            ),
            None => para_el_modelo(p, &Contestada::SinRespuesta),
        },
        Contestada::Texto(t) => format!(
            "«{}» → el usuario contestó con sus palabras: «{}». Sigue con eso.",
            p.texto,
            t.trim()
        ),
        Contestada::SinRespuesta => format!(
            "«{}» → el usuario no contestó. No vuelvas a preguntarlo: sigue con la recomendada («{}») y dilo en una línea al final.",
            p.texto,
            p.opciones
                .first()
                .map(|o| o.etiqueta.as_str())
                .unwrap_or("lo más razonable")
        ),
    }
}

/// Lo que queda escrito en la burbuja tras contestar: «❓ ¿…? → ✅ Solo la
/// figura».
pub fn linea_contestada(p: &Pregunta, c: &Contestada) -> String {
    let fin = match c {
        Contestada::Opcion(i) => match p.opciones.get(*i) {
            Some(o) => format!("✅ {}", o.etiqueta),
            None => "⌛ sin respuesta".to_string(),
        },
        Contestada::Texto(t) => format!("✍️ {}", t.trim()),
        Contestada::SinRespuesta => format!(
            "⌛ sin respuesta: sigo con «{}»",
            p.opciones
                .first()
                .map(|o| o.etiqueta.as_str())
                .unwrap_or("lo más razonable")
        ),
    };
    format!("❓ {} → {fin}", p.texto)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lee_la_forma_buena_y_la_simplificada() {
        let p = leer(r#"{"preguntas":[{"pregunta":"¿Qué idioma?","opciones":[{"etiqueta":"Inglés","descripcion":"todo"},{"etiqueta":"Francés"}]}]}"#).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].texto, "¿Qué idioma?");
        assert_eq!(
            p[0].opciones[0],
            Opcion {
                etiqueta: "Inglés".into(),
                descripcion: "todo".into()
            }
        );
        // Sin la lista y con opciones en cadena (un modelo con prisa).
        let p = leer(r#"{"pregunta":"¿Cuál?","opciones":["A","B","Otro"]}"#).unwrap();
        assert_eq!(
            p[0].opciones
                .iter()
                .map(|o| o.etiqueta.as_str())
                .collect::<Vec<_>>(),
            ["A", "B"],
            "«Otro» lo pone la app"
        );
    }

    #[test]
    fn rechaza_con_un_motivo_que_se_puede_corregir() {
        // Negativos: una sola opcion, demasiadas, demasiadas preguntas, sin texto.
        let e = leer(r#"{"pregunta":"¿Sigo?","opciones":["Sí"]}"#).unwrap_err();
        assert!(e.contains("de 2 a 4") && e.contains("no preguntes"), "{e}");
        assert!(
            leer(r#"{"pregunta":"¿?","opciones":["a","b","c","d","e"]}"#)
                .unwrap_err()
                .contains("deja las 4")
        );
        let tres = r#"{"preguntas":[{"pregunta":"1","opciones":["a","b"]},{"pregunta":"2","opciones":["a","b"]},{"pregunta":"3","opciones":["a","b"]}]}"#;
        assert!(leer(tres).unwrap_err().contains("como mucho 2"));
        assert!(leer(r#"{"opciones":["a","b"]}"#).is_err());
        assert!(leer("no es json").is_err());
        // Repetidas cuentan una vez.
        assert!(leer(r#"{"pregunta":"¿?","opciones":["Sí","si"]}"#).is_err());
    }

    #[test]
    fn lo_contestado_vuelve_al_modelo_y_a_la_burbuja() {
        let p = leer(r#"{"pregunta":"¿Qué idioma?","opciones":[{"etiqueta":"Inglés","descripcion":"todo el lienzo"},"Francés"]}"#).unwrap().remove(0);
        assert!(
            para_el_modelo(&p, &Contestada::Opcion(0)).contains("eligió «Inglés» (todo el lienzo)")
        );
        assert!(para_el_modelo(&p, &Contestada::Texto(" alemán ".into())).contains("«alemán»"));
        let sin = para_el_modelo(&p, &Contestada::SinRespuesta);
        assert!(
            sin.contains("no contestó")
                && sin.contains("«Inglés»")
                && sin.contains("No vuelvas a preguntarlo")
        );
        // Una opcion que no existe es como no contestar.
        assert_eq!(para_el_modelo(&p, &Contestada::Opcion(9)), sin);
        assert_eq!(
            linea_contestada(&p, &Contestada::Opcion(1)),
            "❓ ¿Qué idioma? → ✅ Francés"
        );
    }

    #[test]
    fn el_esquema_es_el_que_lee() {
        let h = herramienta();
        assert_eq!(h.nombre, NOMBRE);
        let ejemplo = h.descripcion.split("Ejemplo: ").nth(1).unwrap();
        assert_eq!(
            leer(ejemplo).unwrap()[0].opciones.len(),
            2,
            "el ejemplo de la descripcion vale"
        );
        assert_eq!(
            h.parametros["properties"]["preguntas"]["maxItems"],
            MAX_PREGUNTAS
        );
    }
}
