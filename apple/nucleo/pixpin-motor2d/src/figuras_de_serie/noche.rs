//! **El modo noche de las plantillas** (25-oct-2026): las mismas hojas
//! (cronograma, planificadores, diario, habitos...) en oscuro, para
//! rellenarlas en el lienzo de noche sin deslumbrarse.
//!
//! No es una inversion de colores: cada color de dia tiene su pareja de
//! noche elegida a mano ([`PALETA`]). El papel es un azul noche
//! ([`FONDO`]), la tinta un blanco roto, las rayas y los bordes tenues pero
//! visibles, los acentos de cada tema algo mas luminosos para que se lean
//! sobre lo oscuro, los fondos «suaves» una version oscura del acento y la
//! zona de «notas» un gris neutro distinto del papel. Todo el texto pasa el
//! contraste AA de la WCAG sobre lo que tiene detras (ver las pruebas).
//!
//! Se aplica **al final**, sobre la hoja ya hecha ([`pintar`]): ninguna
//! plantilla se dibuja dos veces, las piezas, su caja y sus partes con
//! nombre (`parte:<Nombre>`) son las mismas, y lo que se alarga o se
//! personaliza sale de noche con solo pedirlo (`Intervalo::noche`). En el
//! catalogo, cada plantilla de pagina tiene su gemela `serie-<clave>-noche`
//! («… (noche)»), en su misma categoria.
//!
//! La hoja lleva su propio rectangulo de papel oscuro: el SVG o el PDF que
//! pongan blanco detras no se notan.

use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, Elemento, Figura};

/// El final del id de la gemela de noche (`serie-cronograma-noche`).
pub const SUFIJO_NOCHE: &str = "-noche";

/// El papel de noche: un azul muy oscuro, no negro.
pub(super) const FONDO: u32 = 0x1b2030;

/// **Cada color de dia y su pareja de noche.** Los de las plantillas
/// (`plantillas.rs`, `secciones.rs`); uno nuevo sin pareja lo dice la
/// prueba `cada_color_de_dia_tiene_su_pareja_de_noche`.
pub(super) const PALETA: [(u32, u32); 30] = [
    // El papel, su canto y lo que va en blanco (casillas, letras sobre el acento).
    (0xffffff, FONDO),
    (0xdde3ec, 0x2e3649),
    // Tinta, rotulos, rayas y bordes.
    (0x22304a, 0xe6eaf2),
    (0x5d6b82, 0xa3adc0),
    (0xcfd7e3, 0x3b455d),
    (0xb4c0d3, 0x535e7a),
    // Filas alternas, la cuadricula, los puntos y lo apenas sombreado.
    (0xf7f9fc, 0x20263a),
    (0xe1e7f0, 0x38415a),
    (0xb7c2d2, 0x4d5872),
    (0xfcfcfd, 0x1f2434),
    (0x8f9bb0, 0x6f7b94),
    // El mensual: los dias de fuera del mes, hundidos; el fin de semana, apenas mas claro.
    (0xf3f5f9, 0x161a26),
    (0xf6f8fc, 0x222a3d),
    // La zona de «notas»: gris neutro, distinto del papel azulado.
    (0xf1f3f6, 0x2a2d35),
    // El diario: su tinta calida.
    (0x4a3f48, 0xeadfe6),
    // El hito del cronograma.
    (0xd9822b, 0xf6bb7c),
    (0xf0a04b, 0xe39248),
    // Planificacion: acento y suave.
    (0x3f6db3, 0x82a8ee),
    (0xe8eff9, 0x263352),
    // Academico.
    (0x2c8a7f, 0x5fc7b8),
    (0xe3f3f0, 0x1d3a3b),
    // Creativo.
    (0xc0692b, 0xeb9d62),
    (0xfbeee3, 0x3a2b26),
    // Diario.
    (0xbf5878, 0xee90ad),
    (0xfbe9ef, 0x312439),
    // Los blocs y el storyboard (por si una hoja los lleva).
    (0xb9cde6, 0x3d5170),
    (0xe8a0a8, 0x9c5a64),
    (0xd3d9e2, 0x3a4255),
    (0xaab4c3, 0x5a6580),
    (0xc47b7b, 0x8f5a5a),
];

/// **Las letras en un color de raya o de borde** (los dias de la semana
/// apagados de los habitos): de noche, el borde no se lee como letra, asi
/// que van en un gris azulado tenue que si pasa el contraste.
const TEXTO_TENUE: [(u32, u32); 2] = [(0xb4c0d3, 0x8c97ad), (0xcfd7e3, 0x8c97ad)];

/// Un color en `0xrrggbb` (sin la transparencia).
pub(super) fn a_hex(c: ColorRgba) -> u32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (b(c.r) << 16) | (b(c.g) << 8) | b(c.b)
}

/// **La pareja de noche de un color**: la de la paleta o, si no esta, el
/// mismo tono con la claridad del reves (en OKLab, que respeta el tono).
/// La transparencia no cambia.
pub(super) fn de_noche(c: ColorRgba) -> ColorRgba {
    let h = a_hex(c);
    let n = PALETA
        .iter()
        .find(|(d, _)| *d == h)
        .map_or_else(|| claridad_del_reves(h), |(_, n)| *n);
    ColorRgba {
        a: c.a,
        ..super::hex(n)
    }
}

/// **La hoja entera, de noche**: el trazo y el relleno de cada pieza. Nada
/// mas cambia (ni la geometria, ni los grupos, ni las partes).
pub(super) fn pintar(elementos: &mut [Elemento]) {
    for e in elementos {
        let tenue = matches!(e.figura, Figura::Texto { .. })
            .then(|| TEXTO_TENUE.iter().find(|(d, _)| *d == a_hex(e.trazo)))
            .flatten();
        e.trazo = match tenue {
            Some((_, n)) => ColorRgba {
                a: e.trazo.a,
                ..super::hex(*n)
            },
            None => de_noche(e.trazo),
        };
        e.relleno = e.relleno.map(de_noche);
    }
}

/// **La gemela de noche** de una plantilla del catalogo: su id con
/// [`SUFIJO_NOCHE`] y «(noche)» en el nombre.
pub(super) fn gemela(mut f: FiguraGuardada) -> FiguraGuardada {
    pintar(&mut f.elementos);
    f.id.push_str(SUFIJO_NOCHE);
    f.nombre.push_str(" (noche)");
    f
}

/// La luminancia relativa de la WCAG.
pub(super) fn luminancia(c: ColorRgba) -> f32 {
    let l = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * l(c.r) + 0.7152 * l(c.g) + 0.0722 * l(c.b)
}

/// **Si una hoja es de noche**: su papel (el rectangulo relleno mas
/// grande) es oscuro.
pub fn es_de_noche(elementos: &[Elemento]) -> bool {
    elementos
        .iter()
        .filter(|e| !e.borrado && matches!(e.figura, Figura::Rectangulo))
        .filter_map(|e| Some((e.relleno?, e.ancho * e.alto)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .is_some_and(|(c, _)| c.a > 0.5 && luminancia(c) < 0.2)
}

// ------------------------------------------------- lo que no esta en la paleta

fn lineal(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// El mismo tono con la claridad del reves: lo claro queda cerca del
/// papel de noche y lo oscuro se aclara; el color se apaga un poco al
/// oscurecerse para no chillar sobre el fondo.
fn claridad_del_reves(h: u32) -> u32 {
    let c = super::hex(h);
    let (r, g, b) = (lineal(c.r), lineal(c.g), lineal(c.b));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let ll = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let aa = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let bb = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    // El papel de noche tiene L ≈ 0,24 en OKLab.
    let ll2 = 0.24 + (1.0 - ll) * 0.72;
    let k = if ll2 < ll { 0.7 } else { 1.0 };
    let (aa, bb) = (aa * k, bb * k);
    let l = (ll2 + 0.396_337_78 * aa + 0.215_803_76 * bb).powi(3);
    let m = (ll2 - 0.105_561_346 * aa - 0.063_854_17 * bb).powi(3);
    let s = (ll2 - 0.089_484_18 * aa - 1.291_485_5 * bb).powi(3);
    let r = 4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s;
    let g = -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s;
    let b = -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s;
    let q = |v: f32| (srgb(v) * 255.0).round() as u32;
    (q(r) << 16) | (q(g) << 8) | q(b)
}

// ------------------------------------------------------------- el contraste

#[cfg(test)]
/// La razon de contraste de la WCAG entre dos colores (1 a 21).
pub(super) fn contraste(a: ColorRgba, b: ColorRgba) -> f32 {
    let (x, y) = (luminancia(a), luminancia(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[cfg(test)]
/// **El peor texto de una hoja**: para cada texto, su color frente a lo que
/// tiene detras (la ultima pieza rellena dibujada antes que lo cubre en su
/// centro), y la razon minima que pide la WCAG AA (4,5; 3 para el texto
/// grande: 24 px, o 18,66 en negrita; lo que no lleva letras ni cifras
/// es adorno y no cuenta). Devuelve el texto con menos margen:
/// `(texto, razon, minima)`.
pub(super) fn peor_contraste(elementos: &[Elemento]) -> Option<(String, f32, f32)> {
    let mut peor: Option<(String, f32, f32)> = None;
    for (i, e) in elementos.iter().enumerate() {
        let Figura::Texto { texto, tam, .. } = &e.figura else {
            continue;
        };
        // Lo decorativo (la «/» de una fecha) no es texto que leer.
        if e.borrado || !texto.chars().any(char::is_alphanumeric) {
            continue;
        }
        let (cx, cy) = (e.x + e.ancho / 2.0, e.y + e.alto / 2.0);
        let detras = elementos[..i]
            .iter()
            .rev()
            .filter(|o| !o.borrado)
            .filter(|o| !matches!(o.figura, Figura::Texto { .. }))
            .filter_map(|o| Some((o, o.relleno.filter(|c| c.a > 0.5)?)))
            .find(|(o, _)| cubre(o, cx, cy))
            .map(|(_, c)| c);
        let Some(fondo) = detras else { continue };
        let grande = *tam >= 24.0 || (e.extras.negrita && *tam >= 18.66);
        let minima = if grande { 3.0 } else { 4.5 };
        let r = contraste(e.trazo, fondo);
        if peor.as_ref().is_none_or(|(_, pr, pm)| r / minima < pr / pm) {
            peor = Some((texto.clone(), r, minima));
        }
    }
    peor
}

#[cfg(test)]
/// Si la pieza rellena cubre ese punto (la elipse, por dentro de verdad).
fn cubre(o: &Elemento, x: f32, y: f32) -> bool {
    let (x0, y0, x1, y1) = o.caja();
    if x < x0 || x > x1 || y < y0 || y > y1 {
        return false;
    }
    match o.figura {
        Figura::Elipse => {
            let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
            let (dx, dy) = ((x - x0 - rx) / rx.max(0.01), (y - y0 - ry) / ry.max(0.01));
            dx * dx + dy * dy <= 1.0
        }
        _ => true,
    }
}

#[cfg(test)]
#[path = "pruebas_noche.rs"]
mod pruebas;
