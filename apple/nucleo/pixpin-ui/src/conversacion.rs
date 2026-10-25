//! **Un chat compartido como una conversacion de verdad** (25-oct-2026).
//!
//! El usuario: «cuando sea compartido, que el chat se vuelva como un chat
//! real: mis burbujas a la derecha y las de la otra persona al otro lado».
//! Un chat normal sigue siendo un cuaderno propio (todo a un lado); uno
//! compartido es una conversacion entre varias personas, como un grupo de
//! Telegram.
//!
//! Aqui va solo lo que se decide sin ventana y se prueba: **de quien es cada
//! mensaje** (por el codigo de aparato que lleva, `Mensaje.aparato`), a que
//! lado va y que numero de autor le toca para agrupar las tandas. Donde va
//! el nombre encima de la primera burbuja de cada tanda lo decide
//! `historial::colocar`, que es quien sabe que mensajes van pegados.
//!
//! **La regla de «mio»**: es mio lo que escribio cualquiera de MIS aparatos
//! (este equipo, los del grupo de sincronizar —mi movil, mi tableta— y los
//! aparatos de la lista del chat que son mios). Lo que no lleva codigo o
//! lleva uno que nadie de la lista reclama tambien es mio: el chat era mi
//! cuaderno antes de compartirlo, y lo de antes lo escribi yo. Solo es de
//! otro lo que lleva el codigo de un aparato de OTRA persona de la lista.

use std::collections::{HashMap, HashSet};

/// De quien es un mensaje.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Quien {
    /// De cualquiera de mis aparatos: a la derecha, con mi color.
    Yo,
    /// De otra persona, por su nombre: a la izquierda, con su nombre encima.
    /// Sus aparatos (su movil y su PC) son la misma persona si se llaman
    /// igual.
    Otro(String),
}

impl Quien {
    pub fn es_mio(&self) -> bool {
        matches!(self, Quien::Yo)
    }

    /// El nombre que va encima de la tanda, si no es mio.
    pub fn nombre(&self) -> Option<&str> {
        match self {
            Quien::Yo => None,
            Quien::Otro(n) => Some(n),
        }
    }
}

/// Quien es quien en un chat compartido: mis codigos de aparato y, de los
/// demas, el nombre de la persona de cada codigo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lados {
    mios: HashSet<String>,
    otros: HashMap<String, String>,
}

impl Lados {
    /// `mios`: los codigos (`K7Q2`) de mis aparatos. `personas`: de cada
    /// aparato de la lista del chat, su codigo y el nombre de su persona; los
    /// que estan tambien en `mios` se quedan como mios.
    pub fn nuevo(
        mios: impl IntoIterator<Item = String>,
        personas: impl IntoIterator<Item = (String, String)>,
    ) -> Lados {
        let mios: HashSet<String> = mios.into_iter().filter(|c| !c.is_empty()).collect();
        let otros = personas
            .into_iter()
            .filter(|(c, _)| !c.is_empty() && !mios.contains(c))
            .map(|(c, n)| {
                let n = n.trim().to_string();
                // Sin nombre se le llama por su codigo: algo hay que poner
                // encima, y el codigo es lo que lleva su chapa.
                let n = if n.is_empty() { c.clone() } else { n };
                (c, n)
            })
            .collect();
        Lados { mios, otros }
    }

    /// De quien es un mensaje con este codigo de aparato (ya corto, de cuatro
    /// signos).
    pub fn quien(&self, aparato: Option<&str>) -> Quien {
        match aparato.map(str::trim).filter(|a| !a.is_empty()) {
            Some(a) if !self.mios.contains(a) => match self.otros.get(a) {
                Some(n) => Quien::Otro(n.clone()),
                None => Quien::Yo,
            },
            _ => Quien::Yo,
        }
    }

    /// Cuantas personas distintas, aparte de mi, constan.
    pub fn otras_personas(&self) -> usize {
        self.otros.values().collect::<HashSet<_>>().len()
    }
}

/// El numero de autor de cada mensaje, para `historial::Entrada::autor`:
/// `0` lo mio, y a cada otra persona el suyo por orden de aparicion. Dos
/// mensajes seguidos de la misma persona se agrupan; de dos distintas, no,
/// aunque los dos vayan a la izquierda.
pub fn numerar(quienes: &[Quien]) -> Vec<u32> {
    let mut vistos: HashMap<&str, u32> = HashMap::new();
    quienes
        .iter()
        .map(|q| match q {
            Quien::Yo => 0,
            Quien::Otro(n) => {
                let siguiente = vistos.len() as u32 + 1;
                *vistos.entry(n.as_str()).or_insert(siguiente)
            }
        })
        .collect()
}

/// La inicial del circulo de una persona: la primera letra, en mayuscula.
pub fn inicial(nombre: &str) -> String {
    nombre
        .trim()
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default()
}

/// Que color de la paleta le toca a una persona (de `cuantos`): siempre el
/// mismo para el mismo nombre, en cualquier aparato, como en Telegram.
pub fn tono(nombre: &str, cuantos: usize) -> usize {
    if cuantos == 0 {
        return 0;
    }
    // FNV-1a: corto, estable entre versiones y entre plataformas (el hash
    // de la biblioteca estandar no promete ninguna de las dos cosas).
    let mut h: u32 = 0x811c_9dc5;
    for b in nombre.trim().as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h as usize % cuantos
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn lados() -> Lados {
        Lados::nuevo(
            ["PC01".to_string(), "MOV1".to_string()],
            [
                ("PC01".to_string(), "Max".to_string()),
                ("MOV1".to_string(), "Max".to_string()),
                ("ANA1".to_string(), "Ana".to_string()),
                ("ANA2".to_string(), "Ana".to_string()),
                ("LUI1".to_string(), "Luis".to_string()),
                ("SIN1".to_string(), "  ".to_string()),
            ],
        )
    }

    #[test]
    fn lo_de_mis_aparatos_es_mio_tambien_lo_del_movil() {
        let l = lados();
        assert_eq!(l.quien(Some("PC01")), Quien::Yo);
        // Mi movil esta en la lista del chat con mi nombre, pero es mio: si
        // fuera «de otro» saldria a la izquierda con mi propio nombre.
        assert_eq!(l.quien(Some("MOV1")), Quien::Yo);
    }

    #[test]
    fn lo_de_otra_persona_va_con_su_nombre_desde_cualquiera_de_sus_aparatos() {
        let l = lados();
        assert_eq!(l.quien(Some("ANA1")), Quien::Otro("Ana".into()));
        assert_eq!(l.quien(Some("ANA2")), Quien::Otro("Ana".into()));
        assert_eq!(l.quien(Some("LUI1")), Quien::Otro("Luis".into()));
        assert_eq!(l.otras_personas(), 3, "Ana, Luis y el sin nombre");
    }

    #[test]
    fn sin_codigo_o_con_uno_que_nadie_reclama_es_mio() {
        let l = lados();
        // Lo escrito antes de compartir, o en un aparato viejo: mi cuaderno.
        assert_eq!(l.quien(None), Quien::Yo);
        assert_eq!(l.quien(Some("")), Quien::Yo);
        assert_eq!(l.quien(Some("ZZZZ")), Quien::Yo);
    }

    #[test]
    fn sin_nombre_se_le_llama_por_su_codigo() {
        assert_eq!(lados().quien(Some("SIN1")), Quien::Otro("SIN1".into()));
    }

    #[test]
    fn cada_persona_tiene_su_numero_y_lo_mio_el_cero() {
        let q = [
            Quien::Yo,
            Quien::Otro("Ana".into()),
            Quien::Otro("Luis".into()),
            Quien::Otro("Ana".into()),
            Quien::Yo,
        ];
        assert_eq!(numerar(&q), vec![0, 1, 2, 1, 0]);
    }

    #[test]
    fn la_inicial_y_el_tono_son_estables() {
        assert_eq!(inicial(" ana"), "A");
        assert_eq!(inicial(""), "");
        assert_eq!(tono("Ana", 7), tono("Ana", 7));
        assert!(tono("Luis", 7) < 7);
        assert_eq!(tono("x", 0), 0);
    }
}
