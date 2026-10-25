//! **Los pasos del pedido** (25-oct-2026, «que sea un agente como Claude
//! Code»): la lista de tareas que el propio modelo escribe al empezar un
//! pedido de varias partes y va marcando, como el `TodoWrite` de Claude
//! Code. Sirve para dos cosas:
//!
//! - **Que no se le olvide nada ni haga de mas**: los pasos salen solo de lo
//!   que pidio el usuario y se ven en la burbuja, marcandose
//!   ([`crate::agente::Evento::Tareas`], la misma lista que la del jefe).
//! - **Que no acabe con pasos sin hacer**: si contesta sin herramientas con
//!   pasos pendientes, el turno le devuelve [`empujon`] con cuales faltan (la
//!   cuenta de empujones es la de [`crate::guardas::EMPUJONES`]).
//!
//! Cada llamada manda la lista ENTERA (reemplaza la anterior): asi el modelo
//! nunca tiene que recordar ids, y lo que ve en el resultado es lo que hay.
//! Lo contesta el propio turno, sin el ejecutor (como `memoria`).

use serde_json::{Value, json};

use crate::herramientas::Herramienta;
use crate::orquesta::{EstadoTarea, TareaVisible};

/// El nombre de la herramienta.
pub const NOMBRE: &str = "pasos_del_pedido";

/// Cuantos pasos como mucho: mas es partir demasiado (y el jefe, para lo
/// grande, ya reparte en hasta ocho tareas).
pub const MAX_PASOS: usize = 10;

/// Lo mas largo de cada paso.
const LARGO_PASO: usize = 120;

/// La herramienta para el modelo.
pub fn herramienta() -> Herramienta {
    Herramienta {
        nombre: NOMBRE.into(),
        descripcion: "Tu lista de pasos para ESTE pedido (como una lista de tareas que vas marcando). Úsala cuando el pedido tenga \
3 o más pasos o varias partes: al empezar, escribe los pasos (solo lo que pidió el usuario, sin añadir nada), y cada vez que acabes uno, \
vuelve a mandar la lista entera con su estado. Un solo paso en_curso a la vez. Antes de terminar, todos deben estar hecho o no_se_pudo \
(con su nota de por qué). No la uses para un pedido de un solo paso. \
Ejemplo: {\"pasos\":[{\"paso\":\"Agrandar el cronograma a 30 actividades\",\"estado\":\"hecho\"},{\"paso\":\"Agrandarlo a 30 semanas\",\"estado\":\"en_curso\"}]}"
            .into(),
        parametros: json!({
            "type": "object",
            "required": ["pasos"],
            "properties": {
                "pasos": {
                    "type": "array",
                    "maxItems": MAX_PASOS,
                    "items": {
                        "type": "object",
                        "required": ["paso", "estado"],
                        "properties": {
                            "paso": {"type": "string", "description": "lo que hay que hacer, corto"},
                            "estado": {"type": "string", "enum": ["pendiente", "en_curso", "hecho", "no_se_pudo"]},
                            "nota": {"type": "string", "description": "no_se_pudo: por qué"}
                        }
                    }
                }
            }
        }),
    }
}

/// Un paso de la lista.
#[derive(Debug, Clone, PartialEq)]
pub struct Paso {
    pub paso: String,
    pub estado: EstadoTarea,
    pub nota: String,
}

impl Paso {
    /// Si aun queda por hacer.
    pub fn abierto(&self) -> bool {
        matches!(
            self.estado,
            EstadoTarea::Pendiente | EstadoTarea::EnCurso | EstadoTarea::Corrigiendo
        )
    }
}

fn estado_de(s: &str) -> Option<EstadoTarea> {
    match crate::acciones::normalizar(s.trim())
        .replace([' ', '-'], "_")
        .as_str()
    {
        "pendiente" => Some(EstadoTarea::Pendiente),
        "en_curso" | "haciendo" => Some(EstadoTarea::EnCurso),
        "hecho" | "hecha" | "completado" | "completada" => Some(EstadoTarea::Hecha),
        "no_se_pudo" | "no_se_puede" | "imposible" => Some(EstadoTarea::NoSePudo),
        _ => None,
    }
}

/// **Lee la lista** que manda el modelo. `Err` con lo que tiene que
/// corregir (vuelve como ERROR, como las demas herramientas).
pub fn leer(args: &Value) -> Result<Vec<Paso>, String> {
    let lista = args
        .get("pasos")
        .and_then(Value::as_array)
        .ok_or("falta «pasos»: una lista de {\"paso\":\"...\",\"estado\":\"pendiente|en_curso|hecho|no_se_pudo\"}")?;
    if lista.is_empty() {
        return Err("«pasos» está vacía: escribe los pasos del pedido".into());
    }
    if lista.len() > MAX_PASOS {
        return Err(format!("como mucho {MAX_PASOS} pasos: junta los parecidos"));
    }
    let mut pasos = Vec::new();
    for (i, v) in lista.iter().enumerate() {
        let paso = v
            .get("paso")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("el paso {} no tiene «paso»", i + 1))?;
        let estado = v
            .get("estado")
            .and_then(Value::as_str)
            .and_then(estado_de)
            .ok_or_else(|| {
                format!(
                    "el paso {} («{paso}»): «estado» tiene que ser pendiente, en_curso, hecho o no_se_pudo",
                    i + 1
                )
            })?;
        pasos.push(Paso {
            paso: paso.chars().take(LARGO_PASO).collect(),
            estado,
            nota: v
                .get("nota")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string(),
        });
    }
    Ok(pasos)
}

/// Lo mismo desde los argumentos en texto, como llegan del modelo.
pub fn leer_texto(argumentos: &str) -> Result<Vec<Paso>, String> {
    let v: Value = serde_json::from_str(argumentos).map_err(|_| {
        "los argumentos no son un JSON que se lea: manda {\"pasos\":[...]}".to_string()
    })?;
    leer(&v)
}

/// Como se ensena en la burbuja.
pub fn visibles(pasos: &[Paso]) -> Vec<TareaVisible> {
    pasos
        .iter()
        .map(|p| TareaVisible {
            titulo: p.paso.clone(),
            estado: p.estado,
        })
        .collect()
}

/// Lo que vuelve al modelo: cuantos quedan y cuales (lo que ve es lo que
/// hay, sin tener que recordarlo).
pub fn resultado(pasos: &[Paso]) -> String {
    let abiertos: Vec<&Paso> = pasos.iter().filter(|p| p.abierto()).collect();
    if abiertos.is_empty() {
        return format!("hecho: los {} pasos están cerrados.", pasos.len());
    }
    format!(
        "hecho: {} de {} pasos cerrados; quedan: {}.",
        pasos.len() - abiertos.len(),
        pasos.len(),
        nombres(&abiertos)
    )
}

fn nombres(pasos: &[&Paso]) -> String {
    pasos
        .iter()
        .map(|p| format!("«{}»", p.paso))
        .collect::<Vec<_>>()
        .join(", ")
}

/// **El empujon** si va a acabar con pasos abiertos: `None` si no queda
/// ninguno (o no hay lista).
pub fn empujon(pasos: &[Paso]) -> Option<String> {
    let abiertos: Vec<&Paso> = pasos.iter().filter(|p| p.abierto()).collect();
    if abiertos.is_empty() {
        return None;
    }
    Some(format!(
        "[Aviso del turno] Vas a terminar con pasos sin cerrar en {NOMBRE}: {}. Hazlos ahora con las herramientas y márcalos hecho; \
si alguno de verdad no se puede, márcalo no_se_pudo con su nota y dilo en la respuesta. No anuncies lo que vas a hacer: hazlo.",
        nombres(&abiertos)
    ))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_lista_se_lee_y_dice_lo_que_queda() {
        let p = leer(&json!({"pasos": [
            {"paso": "Agrandar a 30 actividades", "estado": "hecho"},
            {"paso": "Agrandar a 30 semanas", "estado": "en curso"},
            {"paso": "Numerar", "estado": "Pendiente"}
        ]}))
        .unwrap();
        assert_eq!(p.len(), 3);
        assert_eq!(p[1].estado, EstadoTarea::EnCurso);
        let r = resultado(&p);
        assert!(r.contains("1 de 3") && r.contains("«Numerar»"), "{r}");
        let e = empujon(&p).expect("quedan dos");
        assert!(
            e.contains("«Agrandar a 30 semanas»") && !e.contains("actividades»"),
            "{e}"
        );
        assert_eq!(visibles(&p)[0].estado, EstadoTarea::Hecha);
    }

    #[test]
    fn cerrada_no_empuja() {
        let p = leer(&json!({"pasos": [
            {"paso": "Agrandar", "estado": "hecho"},
            {"paso": "Colorear", "estado": "no_se_pudo", "nota": "no hay colores en la plantilla"}
        ]}))
        .unwrap();
        assert!(empujon(&p).is_none());
        assert!(resultado(&p).contains("los 2 pasos"));
        assert!(empujon(&[]).is_none(), "sin lista no hay empujon");
    }

    #[test]
    fn una_lista_mal_hecha_se_corrige() {
        assert!(leer(&json!({})).is_err());
        assert!(leer_texto("{roto").is_err());
        assert!(leer_texto(r#"{"pasos":[{"paso":"a","estado":"hecho"}]}"#).is_ok());
        assert!(leer(&json!({"pasos": []})).is_err());
        assert!(leer(&json!({"pasos": [{"paso": "x", "estado": "quizas"}]})).is_err());
        assert!(leer(&json!({"pasos": [{"paso": "  ", "estado": "hecho"}]})).is_err());
        let muchos: Vec<Value> = (0..MAX_PASOS + 1)
            .map(|i| json!({"paso": format!("p{i}"), "estado": "pendiente"}))
            .collect();
        assert!(leer(&json!({ "pasos": muchos })).is_err());
    }
}
