//! **El aspecto de una historia** (5-oct-2026, noche): los degradados de
//! fondo de un momento sin foto y la fecha que se pone arriba.
//!
//! El usuario: «cuando entramos a una tarjeta, como Instagram, aparece el
//! texto en el medio […] que se pueda compartir como una tarjeta bonita,
//! como una tarjeta de las que dicen una frase». Un momento sin foto lleva
//! un degradado suave detras del texto, y **siempre el mismo** (sale de su
//! id): asi la ventana, la imagen compartida y la pagina exportada lo
//! ensenan igual, y no cambia de color cada vez que se abre.

use crate::dias::{Dia, hora};

/// Los degradados, como `(arriba a la izquierda, abajo a la derecha)` en
/// `0xRRGGBB`. Oscuros y saturados a medias: el texto blanco encima se lee
/// en todos.
pub const DEGRADADOS: [(u32, u32); 8] = [
    (0x4E2A84, 0xC2457A), // morado a frambuesa
    (0x1D4E89, 0x2A9D8F), // azul a verde agua
    (0xB4462F, 0xE9A23B), // teja a ambar
    (0x2B2D6E, 0x6A4BC4), // indigo a violeta
    (0x0F5E5A, 0x7FB069), // pino a hoja
    (0x7A1F3D, 0xE76F51), // vino a coral
    (0x23395B, 0x8E7DBE), // noche a lavanda
    (0x5C3D2E, 0xC08552), // cafe a caramelo
];

/// El degradado de un momento, por su id (siempre el mismo).
pub fn degradado_de(id: &str) -> (u32, u32) {
    // FNV-1a: cambia mucho con un caracter, y no depende de la version de
    // Rust (el `Hash` de la biblioteca no promete eso).
    let mut h: u32 = 0x811C_9DC5;
    for b in id.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    DEGRADADOS[(h % DEGRADADOS.len() as u32) as usize]
}

/// `#rrggbb` de un color, para el CSS.
pub fn css(c: u32) -> String {
    format!("#{:06x}", c & 0xFF_FFFF)
}

const MESES_CORTOS_ES: [&str; 12] = [
    "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic",
];
const MESES_CORTOS_EN: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// «oct» / «Oct»: el mes en corto (bajo el numero del mes en «Estado»).
pub fn mes_corto(mes: u8, ingles: bool) -> &'static str {
    let i = usize::from(mes.clamp(1, 12) - 1);
    if ingles { MESES_CORTOS_EN[i] } else { MESES_CORTOS_ES[i] }
}

/// «5 oct 2026 · 17:11» / «Oct 5, 2026 · 17:11»: la fecha de arriba de una
/// tarjeta compartida.
pub fn fecha_larga(utc_ms: i64, desfase: i64, ingles: bool) -> String {
    let d = Dia::de_instante(utc_ms, desfase);
    let i = usize::from(d.mes.clamp(1, 12) - 1);
    let h = hora(utc_ms, desfase);
    if ingles {
        format!("{} {}, {} · {h}", MESES_CORTOS_EN[i], d.dia, d.anio)
    } else {
        format!("{} {} {} · {h}", d.dia, MESES_CORTOS_ES[i], d.anio)
    }
}

/// «0:42» de una duracion en ms (redondeada al segundo), o vacio.
pub fn duracion(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let s = (ms + 500) / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::dias::DIA_MS;

    #[test]
    fn cada_momento_tiene_siempre_el_mismo_degradado() {
        assert_eq!(degradado_de("tl-1-1"), degradado_de("tl-1-1"));
        // Repartidos: con muchos ids salen todos los degradados.
        let distintos: std::collections::BTreeSet<(u32, u32)> =
            (0..200).map(|n| degradado_de(&format!("tl-{n}-1"))).collect();
        assert_eq!(distintos.len(), DEGRADADOS.len());
    }

    #[test]
    fn caso_negativo_dos_ids_parecidos_no_tienen_por_que_compartir_color() {
        let a = degradado_de("tl-100-1");
        let hay_otro = (101..110).any(|n| degradado_de(&format!("tl-{n}-1")) != a);
        assert!(hay_otro);
        assert_eq!(css(0x4E2A84), "#4e2a84");
    }

    #[test]
    fn la_fecha_larga_y_la_duracion() {
        let d = Dia {
            anio: 2026,
            mes: 10,
            dia: 5,
        };
        let utc = d.numero() * DIA_MS + 17 * 3_600_000 + 11 * 60_000;
        assert_eq!(fecha_larga(utc, 0, false), "5 oct 2026 · 17:11");
        assert_eq!(fecha_larga(utc, 0, true), "Oct 5, 2026 · 17:11");
        assert_eq!(mes_corto(10, false), "oct");
        assert_eq!(mes_corto(1, true), "Jan");
        assert_eq!(duracion(42_000), "0:42");
        assert_eq!(duracion(61_600), "1:02");
        // Caso negativo: sin duracion no se inventa «0:00».
        assert_eq!(duracion(0), "");
    }
}
