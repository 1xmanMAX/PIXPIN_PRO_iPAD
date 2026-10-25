//! **Los textos del plano, con su sitio, y buscar en ellos**
//! (`plano_cad::textos` de Windows, 14-oct-2026). Pedido del usuario:
//! «¿donde dice JR. ANTISUYO?» y que el visor lleve ahi.
//!
//! Lo leido para dibujar no guarda las letras como texto, solo sus trazos:
//! los textos se sacan del plano recorriendolo como al dibujarlo
//! (`pixpin_cad::convertir::textos_del_fichero`) y quedan en la cache junto
//! al plano, `<raiz>/cache/cad/<clave>-textos.json` (el mismo JSON que
//! Windows). La segunda busqueda en el mismo plano es instantanea.

use std::path::Path;

use pixpin_cad::convertir::TextoDelPlano;

use super::{Plano, cache};
use crate::ErrorNucleo;

/// Un texto del plano que coincide con lo buscado.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TextoHallado {
    /// El texto entero que lo contiene.
    pub texto: String,
    /// Su punto de insercion, en el dibujo.
    pub x: f64,
    pub y: f64,
    /// El alto de su letra, en unidades del dibujo.
    pub alto: f64,
    /// Donde mirar para verlo (`vista_de`): el centro y el ancho que se ve.
    pub centro_x: f64,
    pub centro_y: f64,
    pub ancho: f64,
}

/// `[[texto, x, y, alto], ...]`: corto, y se lee sin tipos con serde.
pub(crate) fn a_json(t: &[TextoDelPlano]) -> String {
    let v: Vec<serde_json::Value> = t
        .iter()
        .map(|t| serde_json::json!([t.texto, t.x, t.y, t.alto]))
        .collect();
    serde_json::Value::Array(v).to_string()
}

pub(crate) fn de_json(s: &str) -> Option<Vec<TextoDelPlano>> {
    let v: serde_json::Value = serde_json::from_str(s).ok()?;
    v.as_array()?
        .iter()
        .map(|e| {
            Some(TextoDelPlano {
                texto: e.get(0)?.as_str()?.to_string(),
                x: e.get(1)?.as_f64()?,
                y: e.get(2)?.as_f64()?,
                alto: e.get(3)?.as_f64()?,
            })
        })
        .collect()
}

/// **Los textos de `plano`**: de la cache o leyendolo (bloquea: un plano
/// grande tarda unos segundos la primera vez).
pub(crate) fn del_plano(raiz: &Path, plano: &Path) -> Result<Vec<TextoDelPlano>, String> {
    let carpeta = cache::carpeta(raiz);
    let clave = super::clave(plano).ok_or("no se puede leer el plano")?;
    let destino = carpeta.join(format!("{clave}-textos.json"));
    if let Some(t) = std::fs::read_to_string(&destino)
        .ok()
        .and_then(|s| de_json(&s))
    {
        return Ok(t);
    }
    let t = pixpin_cad::convertir::textos_del_fichero(plano)?;
    if std::fs::create_dir_all(&carpeta).is_ok() {
        let tmp = destino.with_extension("tmp");
        if std::fs::write(&tmp, a_json(&t)).is_ok() && std::fs::rename(&tmp, &destino).is_ok() {
            cache::podar(&carpeta);
        }
    }
    Ok(t)
}

/// Minusculas, sin tildes ni puntos y con los espacios de uno en uno: «JR.
/// ANTISUYO» encuentra «Jr Antisuyo».
pub(crate) fn plegar(s: &str) -> String {
    let t: String = pixpin_ia::acciones::normalizar(s)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// **Busca `consulta` en los textos**: primero los que la contienen entera;
/// si no hay, los que tienen todas sus palabras. Los mas grandes antes (el
/// nombre de una calle rotulado en grande es lo que se busca, no la misma
/// palabra en una nota al pie). Como mucho `max`.
pub(crate) fn buscar(textos: &[TextoDelPlano], consulta: &str, max: usize) -> Vec<TextoHallado> {
    let q = plegar(consulta);
    if q.is_empty() {
        return Vec::new();
    }
    let palabras: Vec<&str> = q.split(' ').collect();
    let mut enteras: Vec<&TextoDelPlano> = Vec::new();
    let mut sueltas: Vec<&TextoDelPlano> = Vec::new();
    for t in textos {
        let p = plegar(&t.texto);
        if p.contains(&q) {
            enteras.push(t);
        } else if palabras.iter().all(|w| {
            p.split(' ')
                .any(|x| x == *w || (w.len() >= 4 && x.starts_with(w)))
        }) {
            sueltas.push(t);
        }
    }
    let mut v = if enteras.is_empty() { sueltas } else { enteras };
    v.sort_by(|a, b| b.alto.total_cmp(&a.alto));
    v.into_iter().take(max).map(hallado).collect()
}

/// Lo hallado con su vista: centrada en el texto y con unas 40 letras de
/// ancho (se lee y se ve lo de alrededor), `vista_de` de Windows.
fn hallado(t: &TextoDelPlano) -> TextoHallado {
    let largo = t.texto.chars().count().max(4) as f64;
    let ancho = (t.alto * 40.0).max(t.alto * largo * 1.6).max(1e-6);
    TextoHallado {
        texto: t.texto.clone(),
        x: t.x,
        y: t.y,
        alto: t.alto,
        centro_x: t.x + t.alto * largo * 0.35,
        centro_y: t.y + t.alto * 0.5,
        ancho,
    }
}

#[uniffi::export]
impl Plano {
    /// **Busca en los textos del plano** (sin mirar mayusculas, tildes ni
    /// puntos), los mas grandes antes, como mucho `max`. La primera vez lee
    /// el plano (bloquea: Swift lo llama desde una `Task`).
    pub fn buscar_textos(
        &self,
        consulta: String,
        max: u32,
    ) -> Result<Vec<TextoHallado>, ErrorNucleo> {
        let textos = del_plano(&self.raiz, &self.ruta)
            .map_err(|detalle| ErrorNucleo::Argumento { detalle })?;
        Ok(buscar(&textos, &consulta, max as usize))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn t(texto: &str, x: f64, alto: f64) -> TextoDelPlano {
        TextoDelPlano {
            texto: texto.into(),
            x,
            y: 10.0,
            alto,
        }
    }

    #[test]
    fn se_encuentra_sin_mirar_mayusculas_tildes_ni_puntos_y_el_grande_primero() {
        let ts = vec![
            t("ver Jr Antisuyo en nota", 1.0, 1.0),
            t("JR. ANTISUYO", 50.0, 5.0),
            t("Av. Tahuantinsuyo", 9.0, 5.0),
        ];
        let h = buscar(&ts, "jr antisuyo", 5);
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].texto, "JR. ANTISUYO");
        // Por palabras sueltas, si no esta entera.
        let h = buscar(&[t("ANTISUYO (JR.)", 0.0, 2.0)], "Jr. Antisuyo", 5);
        assert_eq!(h.len(), 1);
        // Las tildes no cuentan.
        assert_eq!(
            buscar(&[t("Pasaje Atahualpa", 0.0, 1.0)], "pasáje", 5).len(),
            1
        );
        // Caso negativo.
        assert!(buscar(&ts, "Collasuyo", 5).is_empty());
        assert!(buscar(&ts, "  ", 5).is_empty());
    }

    #[test]
    fn el_json_de_la_cache_va_y_vuelve() {
        let ts = vec![t("Señal «R-1»", 1.5, 2.25)];
        assert_eq!(de_json(&a_json(&ts)).unwrap(), ts);
        assert!(de_json("[[1,2]]").is_none());
        let v = hallado(&TextoDelPlano {
            texto: "JR. ANTISUYO".into(),
            x: 100.0,
            y: 50.0,
            alto: 2.0,
        });
        assert!(v.ancho >= 80.0 && v.centro_x > 100.0);
    }
}
