//! **La biblioteca de figuras desde el asistente** (`figuras_guardadas`):
//! «guarda esto como arbol isometrico», «pon el arbol isometrico», «¿que
//! figuras tengo?».
//!
//! Es la MISMA biblioteca del panel de figuras del lienzo
//! (`pixpin_motor2d::biblioteca`, fichero `figuras.json` en el formato del
//! movil): lo que guarda el asistente sale en el menu del lienzo y al
//! reves. Esta herramienta solo dice que se pide; la ejecuta la app, que
//! sabe donde vive el fichero y que hay elegido en el lienzo abierto.

use serde_json::{Map, Value, json};

use pixpin_motor2d::biblioteca::FiguraGuardada;

use crate::herramientas::{ErrorHerramienta, Herramienta};

pub const NOMBRE: &str = "figuras_guardadas";

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

pub fn herramienta() -> Herramienta {
    Herramienta {
        nombre: NOMBRE.into(),
        descripcion: concat!(
            "La biblioteca de figuras propias del lienzo. «accion»: guardar (lo elegido en el lienzo, o si no lo ultimo que dibujaste, ",
            "con el «nombre» que diga el usuario), poner (estampa en el lienzo la figura de ese «nombre») o lista (las que hay). ",
            "Ejemplo: {\"accion\":\"poner\",\"nombre\":\"arbol isometrico\"}"
        )
        .into(),
        parametros: json!({
            "type": "object",
            "required": ["accion"],
            "properties": {
                "accion": {"type": "string", "enum": ["guardar", "poner", "lista"]},
                "nombre": {"type": "string"}
            }
        }),
    }
}

/// Lo que se pide a la biblioteca.
#[derive(Debug, Clone, PartialEq)]
pub enum PedidoFiguras {
    Guardar { nombre: String },
    Poner { nombre: String },
    Lista,
}

pub fn validar(args: &Map<String, Value>) -> Result<PedidoFiguras, ErrorHerramienta> {
    let nombre = ["nombre", "figura", "como"]
        .iter()
        .find_map(|k| args.get(*k).and_then(Value::as_str))
        .map(|s| s.trim().trim_matches(['«', '»', '"', '\'']).trim().chars().take(80).collect::<String>())
        .filter(|s| !s.is_empty());
    match args.get("accion").and_then(Value::as_str).map(crate::acciones::normalizar).as_deref() {
        Some("guardar" | "guarda" | "salvar" | "save") => Ok(PedidoFiguras::Guardar {
            nombre: nombre.ok_or_else(|| mal("para guardar hace falta el «nombre» de la figura"))?,
        }),
        Some("poner" | "pon" | "insertar" | "estampar" | "usar" | "put" | "insert") => Ok(PedidoFiguras::Poner {
            nombre: nombre.ok_or_else(|| mal("falta el «nombre» de la figura que poner"))?,
        }),
        Some("lista" | "listar" | "ver" | "cuales" | "list") => Ok(PedidoFiguras::Lista),
        None if nombre.is_none() => Ok(PedidoFiguras::Lista),
        Some(otra) => Err(mal(format!("«accion» = «{otra}» no existe; usa guardar, poner o lista"))),
        None => Err(mal("falta «accion»: guardar, poner o lista")),
    }
}

/// **La figura que mejor encaja con el nombre pedido**: igual sin tildes ni
/// mayusculas; si no, la que contiene todas las palabras pedidas; si no, la
/// que mas comparte. `None` si no se parece ninguna.
pub fn buscar<'a>(figuras: &'a [FiguraGuardada], pedido: &str) -> Option<&'a FiguraGuardada> {
    let q = crate::acciones::normalizar(pedido.trim());
    let palabras = |s: &str| -> Vec<String> {
        crate::acciones::normalizar(s)
            .split(|c: char| !c.is_alphanumeric())
            .filter(|p| p.len() > 2 || p.chars().all(|c| c.is_ascii_digit()))
            // El plural no cuenta: «arboles» es «arbol».
            .map(|p| match p.strip_suffix("es").filter(|r| r.len() > 3) {
                Some(r) => r.to_string(),
                None => p.strip_suffix('s').unwrap_or(p).to_string(),
            })
            .filter(|p| !p.is_empty())
            .collect()
    };
    if let Some(f) = figuras.iter().find(|f| crate::acciones::normalizar(&f.nombre) == q) {
        return Some(f);
    }
    let pq = palabras(pedido);
    if pq.is_empty() {
        return None;
    }
    let comun = |f: &FiguraGuardada| {
        let pf = palabras(&f.nombre);
        pq.iter().filter(|p| pf.contains(p)).count()
    };
    if let Some(f) = figuras.iter().find(|f| comun(f) == pq.len()) {
        return Some(f);
    }
    figuras.iter().filter(|f| comun(f) > 0).max_by_key(|f| comun(f))
}

/// La lista para el modelo (y el usuario).
pub fn lista(figuras: &[FiguraGuardada]) -> String {
    if figuras.is_empty() {
        return "no hay figuras guardadas todavia".into();
    }
    let nombres: Vec<&str> = figuras.iter().map(|f| f.nombre.as_str()).collect();
    format!("{} figuras guardadas: {}", figuras.len(), nombres.join(", "))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::Elemento;

    fn f(nombre: &str) -> FiguraGuardada {
        FiguraGuardada { id: nombre.into(), nombre: nombre.into(), elementos: vec![Elemento::default()] }
    }

    #[test]
    fn se_entienden_las_tres_acciones() {
        let v = |j: Value| validar(j.as_object().unwrap());
        assert_eq!(v(json!({"accion": "guardar", "nombre": "«Árbol isométrico»"})).unwrap(), PedidoFiguras::Guardar { nombre: "Árbol isométrico".into() });
        assert_eq!(v(json!({"accion": "pon", "nombre": "arbol"})).unwrap(), PedidoFiguras::Poner { nombre: "arbol".into() });
        assert_eq!(v(json!({"accion": "lista"})).unwrap(), PedidoFiguras::Lista);
        assert_eq!(v(json!({})).unwrap(), PedidoFiguras::Lista);
        // Casos negativos.
        assert!(v(json!({"accion": "guardar"})).is_err());
        assert!(v(json!({"accion": "poner", "nombre": "  "})).is_err());
        assert!(v(json!({"accion": "borrar_todo", "nombre": "x"})).is_err());
        assert!(v(json!({"nombre": "x"})).is_err());
    }

    #[test]
    fn busca_por_nombre_aunque_no_sea_exacto() {
        let l = vec![f("Árbol isométrico"), f("Casa"), f("Arbol de navidad"), f("Norte")];
        assert_eq!(buscar(&l, "arbol isometrico").unwrap().nombre, "Árbol isométrico");
        assert_eq!(buscar(&l, "el árbol isométrico").unwrap().nombre, "Árbol isométrico");
        assert_eq!(buscar(&l, "CASA").unwrap().nombre, "Casa");
        assert_eq!(buscar(&l, "arboles de navidad").unwrap().nombre, "Arbol de navidad");
        assert!(buscar(&l, "perro").is_none());
        assert!(buscar(&[], "casa").is_none());
        assert!(lista(&l).contains("4 figuras"));
        assert!(lista(&[]).contains("no hay"));
    }
}
