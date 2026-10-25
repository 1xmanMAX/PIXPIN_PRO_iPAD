//! **Buscar** en todo el timeline (5-oct-2026), desde la barra de arriba.
//!
//! El usuario pidio que las cuatro ventanas (galeria, lecciones, tareas y
//! timeline) tengan arriba el mismo buscador. Aqui busca en el titulo y la
//! descripcion de TODOS los momentos, no solo de lo que se ve: el archivo
//! entero es justo lo que no cabe en la pantalla.
//!
//! Se compara **sin mayusculas ni tildes**, como las frases del dictado:
//! Whisper escribe «reunion» o «reunión» segun le da, y quien busca tampoco
//! se para a poner la tilde. Cada palabra de la consulta tiene que estar,
//! en cualquier orden: «luz cocina» encuentra «Se fue la luz en la cocina».

use crate::Momento;
use crate::frases::sin_tilde;

/// `s` en minusculas y sin tildes.
pub fn normalizar(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(sin_tilde)
        .collect()
}

/// Los momentos donde esta cada palabra de `consulta`, **del mas nuevo al
/// mas viejo** (lo reciente es lo que mas se busca). Vacio si la consulta no
/// tiene ninguna palabra.
pub fn buscar<'a>(momentos: &'a [Momento], consulta: &str) -> Vec<&'a Momento> {
    let palabras: Vec<String> = normalizar(consulta)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    if palabras.is_empty() {
        return Vec::new();
    }
    let mut v: Vec<&Momento> = momentos
        .iter()
        .filter(|m| {
            let donde = normalizar(&format!("{}\n{}", m.titulo, m.descripcion));
            palabras.iter().all(|p| donde.contains(p.as_str()))
        })
        .collect();
    v.sort_by(|a, b| b.cuando.cmp(&a.cuando).then_with(|| b.id.cmp(&a.id)));
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn m(id: &str, cuando: i64, titulo: &str, desc: &str) -> Momento {
        Momento::nuevo(id.into(), cuando, titulo.into(), desc.into())
    }

    #[test]
    fn encuentra_sin_tildes_ni_mayusculas_y_en_la_descripcion() {
        let v = [
            m("a", 1, "Reunión con Juan", ""),
            m("b", 2, "Comida", "hablamos de la REUNION de mañana"),
            m("c", 3, "Paseo", "nada que ver"),
        ];
        let ids: Vec<&str> = buscar(&v, "reunion")
            .iter()
            .map(|x| x.id.as_str())
            .collect();
        // Lo mas nuevo primero.
        assert_eq!(ids, ["b", "a"]);
        let ids: Vec<&str> = buscar(&v, "MAÑANA").iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, ["b"]);
    }

    #[test]
    fn todas_las_palabras_en_cualquier_orden() {
        let v = [
            m("a", 1, "Se fue la luz en la cocina", ""),
            m("b", 2, "La luz del salon", ""),
        ];
        let ids: Vec<&str> = buscar(&v, "cocina luz")
            .iter()
            .map(|x| x.id.as_str())
            .collect();
        assert_eq!(ids, ["a"]);
    }

    #[test]
    fn caso_negativo_una_consulta_vacia_o_sin_coincidencias_no_da_nada() {
        let v = [m("a", 1, "Algo", "")];
        assert!(buscar(&v, "   ").is_empty());
        assert!(buscar(&v, "").is_empty());
        assert!(buscar(&v, "otra cosa").is_empty());
    }
}
