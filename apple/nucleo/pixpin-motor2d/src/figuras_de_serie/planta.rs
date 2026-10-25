//! **Simbolos de planta** (24-oct-2026): la biblioteca para dibujar un
//! plano de vivienda entero. Amplia lo que ya habia en «Arquitectura»
//! (muro de 25, puerta abatible y doble, corredera, ventana, escaleras,
//! norte, corte) y en «Mobiliario» (camas, sofa, mesas, inodoro, lavabo,
//! ducha, banera, cocina, frigorifico) con las variantes que faltaban:
//!
//! - **Estructura**: muros delgado, medio, grueso y curvos, columnas,
//!   modulo de escalera, ascensor, linea de rotura, recorrido, acceso y
//!   alfombra.
//! - **Puertas y ventanas**: corredera empotrada, correderas que se
//!   cruzan, plegable y las de mano alzada.
//! - **Cocina y bano**: placas, fregaderos, islas, modulos, armarios,
//!   lavadora y secadora; duchas, lavabos, inodoros, urinario y accesorios.
//! - **Muebles** de dormitorio, sala, despacho y comedor.
//! - **Exterior y movilidad**.
//!
//! Todo va a **1 px = 1 cm** con medidas de catalogo (una hoja de puerta de
//! 80, un modulo de cocina de 60 de fondo, una cama litera de 90 × 190),
//! para que encajen entre si y con las de «Arquitectura» y «Mobiliario».
//!
//! Cada figura se dibuja **por partes con nombre** ([`Lapiz::parte`]):
//! «Hoja», «Arco de apertura», «Muro», «Encimera», «Seno»... para que se
//! puedan mover o retocar sueltas y el asistente sepa que es cada cosa.

use super::arquitectura::tramo;
use super::mobiliario::silla;
use super::tecnico::{BLANCO, Dibujo, FINA, GRIS, POCHE, TINTA, forma, poner, punta, rayado};
use super::{Al, Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, EstiloTrazo};

pub(super) const ESTRUCTURA: &str = "Planta: estructura";
pub(super) const PUERTAS: &str = "Planta: puertas y ventanas";
pub(super) const COCINA_Y_BANO: &str = "Planta: cocina y baño";
pub(super) const MUEBLES: &str = "Planta: muebles";
pub(super) const EXTERIOR: &str = "Planta: exterior";

const MURO: ColorRgba = hex(0xd5d9e0);
const AGUA: ColorRgba = hex(0xdcebf6);
const VIDRIO: ColorRgba = hex(0xd6e7f3);
const COJIN: ColorRgba = hex(0xe3eaf4);
const TAPIZADO: ColorRgba = hex(0xc4d1e3);
const MADERA: ColorRgba = hex(0xf1e3cf);
const ENCIMERA: ColorRgba = hex(0xf3f4f6);
const CRISTAL_NEGRO: ColorRgba = hex(0x3a3f4b);
const ACERO: ColorRgba = hex(0xe4e7ec);
const ROTULO: ColorRgba = hex(0x5d6b82);
const ALFOMBRA: ColorRgba = hex(0xf4ece0);
const EXTERIOR_MADERA: ColorRgba = hex(0xe9d8bd);

// ---------------------------------------------------------------------------
// Piezas comunes
// ---------------------------------------------------------------------------

/// Un rectangulo con contorno (sin redondear).
#[allow(clippy::too_many_arguments)]
fn caja(l: &mut Lapiz, x: f32, y: f32, w: f32, h: f32, t: ColorRgba, r: Option<ColorRgba>, g: f32) {
    l.rect(x, y, w, h, Some(t), r, g, false);
}

/// Lo ultimo que se dibujo, a trazos.
fn a_trazos(l: &mut Lapiz) {
    if let Some(e) = l.v.last_mut() {
        e.estilo = EstiloTrazo::Discontinuo;
    }
}

/// Lo ultimo que se dibujo, a puntos.
fn a_puntos(l: &mut Lapiz) {
    if let Some(e) = l.v.last_mut() {
        e.estilo = EstiloTrazo::Punteado;
    }
}

/// Un rotulo pequeno centrado.
fn rotulo(l: &mut Lapiz, x: f32, y: f32, s: &str, tam: f32) {
    l.texto(x, y, s, tam, WORK_SANS, ROTULO, true, Al::Centro);
}

/// Los dos trozos de muro (de 20) a los lados de un hueco de `ancho` que
/// empieza en `x`, en un muro de `total`.
fn muro_con_hueco(l: &mut Lapiz, x: f32, ancho: f32, total: f32) {
    l.parte("Muro");
    tramo(l, 0.0, 0.0, x, 20.0);
    tramo(l, x + ancho, 0.0, total - x - ancho, 20.0);
}

/// Los puntos de un arco (grados de pantalla).
fn puntos_de_arco(cx: f32, cy: f32, r: f32, desde: f32, hasta: f32) -> Vec<(f32, f32)> {
    let n = (((hasta - desde).abs() / 4.0).ceil() as usize).max(2);
    (0..=n)
        .map(|i| {
            let a = (desde + (hasta - desde) * i as f32 / n as f32).to_radians();
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

// ------------------------------------------------------------ a mano alzada

/// **Un trazo a mano alzada** por esos puntos: cada tramo se pasa un poco
/// de largo por los dos extremos (como cuando se dibuja con regla y prisa)
/// y se comba apenas, siempre igual (sin azar: la figura sale identica).
fn a_mano(l: &mut Lapiz, pts: &[(f32, f32)], g: f32) {
    for (i, par) in pts.windows(2).enumerate() {
        let ((x0, y0), (x1, y1)) = (par[0], par[1]);
        let (dx, dy) = (x1 - x0, y1 - y0);
        let largo = (dx * dx + dy * dy).sqrt().max(1e-3);
        let (ux, uy) = (dx / largo, dy / largo);
        let sobra = 3.0f32.min(largo * 0.08);
        let a = (x0 - ux * sobra, y0 - uy * sobra);
        let b = (x1 + ux * sobra, y1 + uy * sobra);
        let comba = largo * 0.012 * if i % 2 == 0 { 1.0 } else { -1.0 };
        let m = (
            (a.0 + b.0) / 2.0 - uy * comba,
            (a.1 + b.1) / 2.0 + ux * comba,
        );
        let curva: Vec<(f32, f32)> = (0..=10)
            .map(|k| {
                let t = k as f32 / 10.0;
                let (u, v) = ((1.0 - t) * (1.0 - t), 2.0 * (1.0 - t) * t);
                (
                    u * a.0 + v * m.0 + t * t * b.0,
                    u * a.1 + v * m.1 + t * t * b.1,
                )
            })
            .collect();
        l.poli(&curva, TINTA, g, None);
    }
}

/// Un arco a mano alzada: algo pasado de largo y con el radio que respira.
#[allow(clippy::too_many_arguments)]
fn arco_a_mano(l: &mut Lapiz, cx: f32, cy: f32, r: f32, desde: f32, hasta: f32, g: f32) {
    let n = 30;
    let (d, h) = (desde - 3.0, hasta + 3.0);
    let pts: Vec<(f32, f32)> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let a = (d + (h - d) * t).to_radians();
            let rr = r * (1.0 + 0.012 * (t * 7.0).sin());
            (cx + rr * a.cos(), cy + rr * a.sin())
        })
        .collect();
    l.poli(&pts, FINA, g, None);
}

// ---------------------------------------------------------------------------
// Estructura
// ---------------------------------------------------------------------------

/// Un tramo recto de muro de `e` de espesor y 300 de largo.
fn muro_recto(l: &mut Lapiz, e: f32) {
    l.parte("Muro");
    tramo(l, 0.0, 0.0, 300.0, e);
}

/// **Un muro curvo** de espesor `e`: la corona entre dos arcos, con su
/// relleno de muro cortado y un rayado radial fino.
fn muro_curvo(l: &mut Lapiz, r: f32, e: f32, desde: f32, hasta: f32) {
    let (cx, cy) = (0.0, 0.0);
    let fuera = puntos_de_arco(cx, cy, r + e / 2.0, desde, hasta);
    let dentro = puntos_de_arco(cx, cy, r - e / 2.0, hasta, desde);
    let mut corona = fuera.clone();
    corona.extend(dentro);
    l.parte("Muro");
    forma(l, &corona, TINTA, Some(MURO), 0.5);
    l.parte("Rayado");
    let pasos = ((hasta - desde) / 5.0).round() as usize;
    for i in 1..pasos {
        let a = (desde + (hasta - desde) * i as f32 / pasos as f32).to_radians();
        let b = a + (e * 0.8 / r);
        l.linea(
            cx + (r - e / 2.0) * a.cos(),
            cy + (r - e / 2.0) * a.sin(),
            cx + (r + e / 2.0) * b.cos(),
            cy + (r + e / 2.0) * b.sin(),
            POCHE,
            0.75,
        );
    }
    l.parte("Contorno");
    forma(l, &corona, TINTA, None, 2.0);
}

fn columna_rect(l: &mut Lapiz) {
    // Parte «Columna»: la seccion de hormigon, maciza.
    l.parte("Columna");
    caja(l, 0.0, 0.0, 30.0, 30.0, TINTA, Some(POCHE), 2.0);
}

fn columna_rect_h(l: &mut Lapiz) {
    // Parte «Recubrimiento»: el cuadrado que envuelve el perfil.
    l.parte("Recubrimiento");
    caja(l, 0.0, 0.0, 40.0, 40.0, TINTA, Some(BLANCO), 2.0);
    // Parte «Perfil H»: el perfil metalico, con sus dos alas y el alma.
    l.parte("Perfil H");
    let h = [
        (8.0, 8.0),
        (14.0, 8.0),
        (14.0, 17.5),
        (26.0, 17.5),
        (26.0, 8.0),
        (32.0, 8.0),
        (32.0, 32.0),
        (26.0, 32.0),
        (26.0, 22.5),
        (14.0, 22.5),
        (14.0, 32.0),
        (8.0, 32.0),
    ];
    forma(l, &h, TINTA, Some(POCHE), 1.25);
}

fn columna_rect_x(l: &mut Lapiz) {
    l.parte("Columna");
    caja(l, 0.0, 0.0, 40.0, 40.0, TINTA, Some(BLANCO), 2.0);
    // Parte «Aspa»: la cruz de las columnas que pasan sin cortarse.
    l.parte("Aspa");
    l.linea(0.0, 0.0, 40.0, 40.0, TINTA, 1.25);
    l.linea(40.0, 0.0, 0.0, 40.0, TINTA, 1.25);
}

fn columna_circular(l: &mut Lapiz) {
    l.parte("Columna");
    l.circulo(17.5, 17.5, 17.5, Some(TINTA), Some(POCHE), 2.0);
}

fn columna_circular_h(l: &mut Lapiz) {
    l.parte("Recubrimiento");
    l.circulo(22.5, 22.5, 22.5, Some(TINTA), Some(BLANCO), 2.0);
    l.parte("Perfil H");
    let h = [
        (11.0, 11.0),
        (16.0, 11.0),
        (16.0, 20.5),
        (29.0, 20.5),
        (29.0, 11.0),
        (34.0, 11.0),
        (34.0, 34.0),
        (29.0, 34.0),
        (29.0, 24.5),
        (16.0, 24.5),
        (16.0, 34.0),
        (11.0, 34.0),
    ];
    forma(l, &h, TINTA, Some(POCHE), 1.25);
}

/// Un tramo de cinco peldanos de 25 de huella y 100 de ancho, para ir
/// componiendo escaleras.
fn escalera_modulo(l: &mut Lapiz) {
    l.parte("Peldaños");
    caja(l, 0.0, 0.0, 100.0, 125.0, TINTA, Some(BLANCO), 2.0);
    for i in 1..5 {
        let y = 25.0 * i as f32;
        l.linea(0.0, y, 100.0, y, FINA, 1.0);
    }
    // Parte «Sentido»: la flecha de subida, del punto al final.
    l.parte("Sentido");
    l.circulo(50.0, 112.0, 3.5, Some(TINTA), Some(TINTA), 1.0);
    l.flecha(&[(50.0, 112.0), (50.0, 10.0)], TINTA, 1.25, false);
}

/// **El ascensor**: la caja de muros con el hueco de la puerta, la cabina
/// con su aspa, el contrapeso y las dos hojas telescopicas.
fn ascensor(l: &mut Lapiz) {
    let (w, h, e) = (180.0, 180.0, 15.0);
    l.parte("Caja de muros");
    tramo(l, 0.0, 0.0, w, e);
    tramo(l, 0.0, e, e, h - e);
    tramo(l, w - e, e, e, h - e);
    tramo(l, 0.0, h - e, 45.0, e);
    tramo(l, w - 45.0, h - e, 45.0, e);
    l.parte("Cabina");
    caja(l, 30.0, 25.0, 120.0, 125.0, TINTA, Some(BLANCO), 1.5);
    l.linea(30.0, 25.0, 150.0, 150.0, FINA, 1.0);
    l.linea(150.0, 25.0, 30.0, 150.0, FINA, 1.0);
    l.parte("Contrapeso");
    caja(l, 70.0, 17.0, 40.0, 6.0, TINTA, Some(GRIS), 1.0);
    l.parte("Puertas");
    caja(l, 45.0, 155.0, 46.0, 4.0, TINTA, Some(GRIS), 1.0);
    caja(l, 89.0, 160.0, 46.0, 4.0, TINTA, Some(GRIS), 1.0);
}

/// **La linea de rotura**: una recta con el quiebro en medio, para cortar
/// un elemento que sigue.
fn linea_de_rotura(l: &mut Lapiz) {
    l.parte("Línea de rotura");
    l.poli(
        &[
            (0.0, 30.0),
            (88.0, 30.0),
            (96.0, 6.0),
            (104.0, 54.0),
            (112.0, 30.0),
            (200.0, 30.0),
        ],
        TINTA,
        1.5,
        None,
    );
}

/// **El sentido de recorrido** (evacuacion, circulacion): origen, linea a
/// trazos y punta.
fn flecha_de_recorrido(l: &mut Lapiz) {
    l.parte("Origen");
    l.circulo(6.0, 20.0, 6.0, Some(TINTA), Some(TINTA), 1.0);
    l.parte("Recorrido");
    l.poli(&[(6.0, 20.0), (186.0, 20.0)], TINTA, 1.5, None);
    a_trazos(l);
    l.parte("Punta");
    punta(l, 200.0, 20.0, 0.0, 16.0, TINTA);
}

/// **El acceso** al edificio: el triangulo macizo que apunta hacia dentro
/// y su rotulo.
fn acceso(l: &mut Lapiz) {
    l.parte("Triángulo");
    forma(
        l,
        &[(40.0, 0.0), (80.0, 60.0), (0.0, 60.0)],
        TINTA,
        Some(TINTA),
        1.5,
    );
    l.parte("Rótulo");
    rotulo(l, 40.0, 68.0, "ACCESO", 13.0);
}

fn alfombra(l: &mut Lapiz) {
    let (w, h) = (200.0, 140.0);
    // Parte «Alfombra»: el pano con su cenefa.
    l.parte("Alfombra");
    caja(l, 0.0, 0.0, w, h, FINA, Some(ALFOMBRA), 1.5);
    caja(l, 10.0, 10.0, w - 20.0, h - 20.0, FINA, None, 0.75);
    caja(l, 16.0, 16.0, w - 32.0, h - 32.0, FINA, None, 0.75);
    a_trazos(l);
    // Parte «Flecos»: en los dos lados cortos.
    l.parte("Flecos");
    for i in 0..15 {
        let y = 6.0 + 9.0 * i as f32;
        l.linea(-8.0, y, 0.0, y, FINA, 0.75);
        l.linea(w, y, w + 8.0, y, FINA, 0.75);
    }
}

// ---------------------------------------------------------------------------
// Puertas y ventanas
// ---------------------------------------------------------------------------

/// **La corredera empotrada** (pocket): la hoja se esconde dentro del muro,
/// que se abre en dos pieles para alojarla.
fn puerta_empotrada(l: &mut Lapiz) {
    l.parte("Muro");
    tramo(l, 0.0, 0.0, 40.0, 20.0);
    tramo(l, 40.0, 0.0, 90.0, 6.0);
    tramo(l, 40.0, 14.0, 90.0, 6.0);
    tramo(l, 210.0, 0.0, 60.0, 20.0);
    l.parte("Hoja");
    caja(l, 50.0, 8.0, 100.0, 4.0, TINTA, Some(BLANCO), 1.25);
    l.parte("Hoja escondida");
    l.linea(44.0, 10.0, 128.0, 10.0, FINA, 0.75);
    a_trazos(l);
    l.parte("Sentido");
    l.flecha(&[(195.0, 34.0), (145.0, 34.0)], FINA, 1.0, false);
}

/// **Correderas que se cruzan** (bypass): dos hojas en dos carriles.
fn correderas_2(l: &mut Lapiz) {
    muro_con_hueco(l, 50.0, 160.0, 260.0);
    l.parte("Hojas");
    caja(l, 50.0, 5.0, 86.0, 4.0, TINTA, Some(BLANCO), 1.25);
    caja(l, 124.0, 11.0, 86.0, 4.0, TINTA, Some(BLANCO), 1.25);
    l.parte("Sentido");
    l.flecha(&[(80.0, 34.0), (120.0, 34.0)], FINA, 1.0, false);
    l.flecha(&[(180.0, 34.0), (140.0, 34.0)], FINA, 1.0, false);
}

/// Cuatro hojas: dos parejas que se abren desde el centro.
fn correderas_4(l: &mut Lapiz) {
    muro_con_hueco(l, 50.0, 240.0, 340.0);
    l.parte("Hojas");
    caja(l, 50.0, 5.0, 66.0, 4.0, TINTA, Some(BLANCO), 1.25);
    caja(l, 108.0, 11.0, 66.0, 4.0, TINTA, Some(BLANCO), 1.25);
    caja(l, 166.0, 11.0, 66.0, 4.0, TINTA, Some(BLANCO), 1.25);
    caja(l, 224.0, 5.0, 66.0, 4.0, TINTA, Some(BLANCO), 1.25);
    l.parte("Sentido");
    l.flecha(&[(160.0, 34.0), (110.0, 34.0)], FINA, 1.0, false);
    l.flecha(&[(180.0, 34.0), (230.0, 34.0)], FINA, 1.0, false);
}

/// **La plegable** (de acordeon): las hojas estrechas en zigzag.
fn plegable(l: &mut Lapiz) {
    muro_con_hueco(l, 50.0, 90.0, 190.0);
    l.parte("Hojas plegadas");
    let pts: Vec<(f32, f32)> = (0..=8)
        .map(|i| {
            let x = 52.0 + 86.0 * i as f32 / 8.0;
            (x, if i % 2 == 0 { 4.0 } else { 18.0 })
        })
        .collect();
    l.poli(&pts, TINTA, 1.5, None);
    l.parte("Guía");
    l.linea(50.0, 2.0, 140.0, 2.0, FINA, 0.75);
    a_trazos(l);
}

/// La puerta de una hoja dibujada a mano alzada.
fn puerta_mano_1(l: &mut Lapiz) {
    l.parte("Muro");
    for x in [(0.0, 40.0), (130.0, 170.0)] {
        a_mano(l, &[(x.0, 0.0), (x.1, 0.0)], 1.6);
        a_mano(l, &[(x.0, 15.0), (x.1, 15.0)], 1.6);
    }
    a_mano(l, &[(40.0, 0.0), (40.0, 15.0)], 1.6);
    a_mano(l, &[(130.0, 0.0), (130.0, 15.0)], 1.6);
    l.parte("Hoja");
    a_mano(l, &[(40.0, 15.0), (40.0, 105.0)], 2.2);
    l.parte("Arco de apertura");
    arco_a_mano(l, 40.0, 15.0, 90.0, 0.0, 90.0, 1.0);
}

/// Dos hojas a mano alzada, abiertas hacia el mismo lado.
fn puerta_mano_2(l: &mut Lapiz) {
    l.parte("Muro");
    for x in [(0.0, 40.0), (200.0, 240.0)] {
        a_mano(l, &[(x.0, 0.0), (x.1, 0.0)], 1.6);
        a_mano(l, &[(x.0, 15.0), (x.1, 15.0)], 1.6);
    }
    a_mano(l, &[(40.0, 0.0), (40.0, 15.0)], 1.6);
    a_mano(l, &[(200.0, 0.0), (200.0, 15.0)], 1.6);
    l.parte("Hojas");
    a_mano(l, &[(40.0, 15.0), (40.0, 95.0)], 2.2);
    a_mano(l, &[(200.0, 15.0), (200.0, 95.0)], 2.2);
    l.parte("Arcos de apertura");
    arco_a_mano(l, 40.0, 15.0, 80.0, 0.0, 90.0, 1.0);
    arco_a_mano(l, 200.0, 15.0, 80.0, 90.0, 180.0, 1.0);
}

/// La ventana a mano alzada: tres rayas en el hueco del muro.
fn ventana_mano(l: &mut Lapiz) {
    l.parte("Muro");
    for x in [(0.0, 40.0), (160.0, 200.0)] {
        a_mano(l, &[(x.0, 0.0), (x.1, 0.0)], 1.6);
        a_mano(l, &[(x.0, 15.0), (x.1, 15.0)], 1.6);
    }
    l.parte("Ventana");
    a_mano(l, &[(40.0, 0.0), (40.0, 15.0)], 1.6);
    a_mano(l, &[(160.0, 0.0), (160.0, 15.0)], 1.6);
    a_mano(l, &[(40.0, 0.0), (160.0, 0.0)], 1.0);
    a_mano(l, &[(40.0, 7.5), (160.0, 7.5)], 1.0);
    a_mano(l, &[(40.0, 15.0), (160.0, 15.0)], 1.0);
}

/// **La ventana corredera**: marco, dos hojas de vidrio en dos carriles y
/// el alfeizar.
fn ventana_corredera(l: &mut Lapiz) {
    muro_con_hueco(l, 50.0, 150.0, 250.0);
    l.parte("Marco");
    caja(l, 50.0, 0.0, 150.0, 20.0, TINTA, Some(BLANCO), 1.5);
    l.parte("Hojas de vidrio");
    caja(l, 52.0, 5.0, 80.0, 4.0, FINA, Some(VIDRIO), 1.0);
    caja(l, 118.0, 11.0, 80.0, 4.0, FINA, Some(VIDRIO), 1.0);
    l.parte("Alféizar");
    l.linea(44.0, 24.0, 206.0, 24.0, FINA, 1.0);
}

// ---------------------------------------------------------------------------
// Cocina
// ---------------------------------------------------------------------------

/// **La placa vitroceramica** de 60: el cristal negro, cuatro zonas y los
/// mandos tactiles delante.
fn placa_electrica(l: &mut Lapiz) {
    l.parte("Cristal");
    l.rect(
        0.0,
        0.0,
        60.0,
        52.0,
        Some(TINTA),
        Some(CRISTAL_NEGRO),
        1.5,
        true,
    );
    l.parte("Zonas de calor");
    for (cx, cy, r) in [
        (17.0, 14.0, 9.0),
        (43.0, 14.0, 7.0),
        (17.0, 35.0, 7.0),
        (43.0, 35.0, 10.0),
    ] {
        l.circulo(cx, cy, r, Some(GRIS), None, 1.0);
        l.circulo(cx, cy, r * 0.6, Some(GRIS), None, 0.5);
    }
    l.parte("Mandos");
    for i in 0..4 {
        l.circulo(
            15.0 + 10.0 * i as f32,
            48.0,
            1.5,
            Some(GRIS),
            Some(GRIS),
            0.5,
        );
    }
}

/// Un quemador de gas con su parrilla de hierro (en cruz).
fn quemador(l: &mut Lapiz, cx: f32, cy: f32, r: f32) {
    l.rect(
        cx - r - 4.0,
        cy - r - 4.0,
        2.0 * r + 8.0,
        2.0 * r + 8.0,
        Some(TINTA),
        None,
        1.0,
        true,
    );
    l.circulo(cx, cy, r, Some(TINTA), Some(ACERO), 1.0);
    l.circulo(cx, cy, r * 0.45, Some(TINTA), Some(GRIS), 0.75);
    l.linea(cx - r - 4.0, cy, cx + r + 4.0, cy, TINTA, 1.25);
    l.linea(cx, cy - r - 4.0, cx, cy + r + 4.0, TINTA, 1.25);
}

/// **La encimera de gas** de 75 con cinco fuegos y sus mandos.
fn cocina_de_gas(l: &mut Lapiz) {
    l.parte("Encimera");
    l.rect(0.0, 0.0, 75.0, 52.0, Some(TINTA), Some(ACERO), 1.5, true);
    l.parte("Quemadores");
    quemador(l, 14.0, 13.0, 6.0);
    quemador(l, 14.0, 34.0, 6.0);
    quemador(l, 37.5, 23.5, 8.5);
    quemador(l, 61.0, 13.0, 6.0);
    quemador(l, 61.0, 34.0, 6.0);
    l.parte("Mandos");
    for i in 0..5 {
        l.circulo(
            13.0 + 12.25 * i as f32,
            48.0,
            2.2,
            Some(TINTA),
            Some(BLANCO),
            0.75,
        );
    }
}

/// **La cocina con horno** (de pie, 60 × 60): cuatro fuegos, los mandos
/// en el frente y la puerta del horno abatida a trazos.
fn horno_fogones(l: &mut Lapiz) {
    l.parte("Cuerpo");
    caja(l, 0.0, 0.0, 60.0, 60.0, TINTA, Some(ACERO), 2.0);
    l.parte("Fuegos");
    for (cx, cy, r) in [
        (16.0, 15.0, 8.0),
        (44.0, 15.0, 6.5),
        (16.0, 38.0, 6.5),
        (44.0, 38.0, 8.0),
    ] {
        l.circulo(cx, cy, r, Some(TINTA), Some(BLANCO), 1.0);
        l.circulo(cx, cy, r * 0.45, Some(TINTA), Some(GRIS), 0.75);
    }
    l.parte("Mandos");
    l.linea(0.0, 50.0, 60.0, 50.0, TINTA, 1.0);
    for i in 0..5 {
        l.circulo(
            10.0 + 10.0 * i as f32,
            55.0,
            2.0,
            Some(TINTA),
            Some(BLANCO),
            0.75,
        );
    }
    l.parte("Puerta del horno (abierta)");
    caja(l, 2.0, 60.0, 56.0, 45.0, FINA, None, 0.75);
    a_trazos(l);
}

/// **El frigorifico combi** (frigorifico arriba y congelador abajo, en
/// planta solo se ve la caja y el giro de la puerta).
fn frigo_congelador(l: &mut Lapiz) {
    let s = 70.0;
    l.parte("Cuerpo");
    caja(l, 0.0, 0.0, s, 65.0, TINTA, Some(BLANCO), 2.0);
    l.parte("Puerta");
    l.linea(0.0, 59.0, s, 59.0, TINTA, 1.25);
    l.parte("Rótulo");
    rotulo(l, s / 2.0, 14.0, "FRIGO", 10.0);
    rotulo(l, s / 2.0, 30.0, "CONG.", 10.0);
    l.parte("Arco de apertura");
    l.arco(0.0, 65.0, s, 0.0, 90.0, FINA, 0.75);
    a_trazos(l);
}

/// Un seno de fregadero con su desague.
fn seno(l: &mut Lapiz, x: f32, y: f32, w: f32, h: f32) {
    l.rect(x, y, w, h, Some(FINA), Some(AGUA), 1.25, true);
    l.circulo(x + w / 2.0, y + h / 2.0, 2.5, Some(FINA), Some(FINA), 1.0);
}

/// El grifo: la base y el cano.
fn grifo(l: &mut Lapiz, x: f32, y: f32, largo: f32) {
    l.circulo(x, y, 3.0, Some(TINTA), Some(GRIS), 1.0);
    l.linea(x, y, x, y + largo, TINTA, 2.0);
}

/// **Fregadero de un seno** con escurridor (100 × 50).
fn fregadero_simple(l: &mut Lapiz) {
    l.parte("Encimera");
    l.rect(0.0, 0.0, 100.0, 50.0, Some(TINTA), Some(ACERO), 1.5, true);
    l.parte("Seno");
    seno(l, 8.0, 8.0, 44.0, 36.0);
    l.parte("Escurridor");
    for i in 0..6 {
        let y = 12.0 + 6.0 * i as f32;
        l.linea(62.0, y, 92.0, y, FINA, 0.75);
    }
    l.parte("Grifo");
    grifo(l, 30.0, 4.0, 10.0);
}

/// **Fregadero de dos senos** (120 × 50).
fn fregadero_doble(l: &mut Lapiz) {
    l.parte("Encimera");
    l.rect(0.0, 0.0, 120.0, 50.0, Some(TINTA), Some(ACERO), 1.5, true);
    l.parte("Senos");
    seno(l, 8.0, 8.0, 48.0, 36.0);
    seno(l, 64.0, 8.0, 48.0, 36.0);
    l.parte("Grifo");
    grifo(l, 60.0, 4.0, 12.0);
}

/// **Una isla de cocina** de `w × h`; el vuelo de la encimera (para comer
/// en ella) va a trazos por el lado de abajo y, si cabe, con taburetes.
fn isla(l: &mut Lapiz, w: f32, h: f32, taburetes: usize) {
    l.parte("Encimera");
    caja(l, 0.0, 0.0, w, h, TINTA, Some(ENCIMERA), 2.0);
    l.parte("Vuelo de la encimera");
    l.linea(0.0, h - 25.0, w, h - 25.0, FINA, 0.75);
    a_trazos(l);
    if taburetes > 0 {
        l.parte("Taburetes");
        let paso = w / taburetes as f32;
        for i in 0..taburetes {
            let cx = paso * (i as f32 + 0.5);
            l.circulo(cx, h + 14.0, 17.0, Some(TINTA), Some(BLANCO), 1.25);
            l.circulo(cx, h + 14.0, 11.0, Some(FINA), Some(COJIN), 0.75);
        }
    }
}

/// **Un modulo bajo de cocina** de `w` de ancho y 60 de fondo: la
/// encimera, el frente de puertas y sus tiradores.
fn modulo(l: &mut Lapiz, w: f32) {
    l.parte("Encimera");
    caja(l, 0.0, 0.0, w, 60.0, TINTA, Some(ENCIMERA), 2.0);
    l.parte("Frente");
    l.linea(0.0, 56.0, w, 56.0, FINA, 0.75);
    let puertas = (w / 60.0).round().max(1.0) as usize;
    let ancho = w / puertas as f32;
    for i in 1..puertas {
        let x = ancho * i as f32;
        l.linea(x, 56.0, x, 60.0, FINA, 0.75);
    }
    l.parte("Tiradores");
    for i in 0..puertas {
        let cx = ancho * (i as f32 + 0.5);
        l.linea(cx - 6.0, 62.0, cx + 6.0, 62.0, TINTA, 2.0);
    }
}

/// **La cajonera** (50 × 60): tres tiradores de cajon en el frente.
fn cajonera(l: &mut Lapiz) {
    l.parte("Mueble");
    caja(l, 0.0, 0.0, 50.0, 60.0, TINTA, Some(BLANCO), 2.0);
    l.parte("Cajón abierto");
    caja(l, 3.0, 60.0, 44.0, 40.0, FINA, None, 0.75);
    a_trazos(l);
    l.parte("Tiradores");
    for y in [62.0, 66.0, 70.0] {
        l.linea(17.0, y, 33.0, y, TINTA, 1.5);
    }
}

/// **Un armario bajo** de `w` con sus puertas abatibles a trazos.
fn armario_bajo(l: &mut Lapiz, w: f32) {
    l.parte("Mueble");
    caja(l, 0.0, 0.0, w, 60.0, TINTA, Some(BLANCO), 2.0);
    let hoja = w / 2.0;
    l.parte("Puertas");
    l.linea(0.0, 58.0, w, 58.0, FINA, 0.75);
    l.linea(hoja, 58.0, hoja, 60.0, FINA, 0.75);
    l.parte("Arcos de apertura");
    l.arco(0.0, 60.0, hoja, 0.0, 90.0, FINA, 0.75);
    a_trazos(l);
    l.arco(w, 60.0, hoja, 90.0, 180.0, FINA, 0.75);
    a_trazos(l);
    l.parte("Tiradores");
    l.linea(hoja - 6.0, 62.0, hoja - 6.0, 70.0, TINTA, 2.0);
    l.linea(hoja + 6.0, 62.0, hoja + 6.0, 70.0, TINTA, 2.0);
}

/// **El armario alto** (colgado, 90 × 35): por encima del plano de corte,
/// asi que va a trazos, con su aspa.
fn armario_alto(l: &mut Lapiz) {
    l.parte("Armario colgado");
    caja(l, 0.0, 0.0, 90.0, 35.0, FINA, None, 1.25);
    a_trazos(l);
    l.linea(0.0, 0.0, 90.0, 35.0, FINA, 0.75);
    a_trazos(l);
    l.linea(90.0, 0.0, 0.0, 35.0, FINA, 0.75);
    a_trazos(l);
}

/// Un electrodomestico de 60 × 60 de carga frontal: cuerpo, panel de
/// mandos al fondo y el ojo de buey.
fn de_tambor(l: &mut Lapiz, nombre: &str, secadora: bool) {
    l.parte("Cuerpo");
    caja(l, 0.0, 0.0, 60.0, 60.0, TINTA, Some(BLANCO), 2.0);
    l.parte("Panel de mandos");
    l.linea(0.0, 10.0, 60.0, 10.0, FINA, 1.0);
    l.rect(6.0, 3.0, 18.0, 4.5, Some(FINA), Some(GRIS), 0.75, false);
    l.circulo(48.0, 5.0, 2.5, Some(FINA), Some(BLANCO), 0.75);
    l.parte("Tambor");
    l.circulo(30.0, 35.0, 19.0, Some(TINTA), Some(VIDRIO), 1.25);
    l.circulo(30.0, 35.0, 13.0, Some(FINA), None, 0.75);
    if secadora {
        l.parte("Rejilla");
        for i in 0..3 {
            let y = 30.0 + 5.0 * i as f32;
            l.linea(22.0, y, 38.0, y, FINA, 0.75);
        }
    }
    l.parte("Rótulo");
    rotulo(l, 30.0, 62.0, nombre, 9.0);
}

/// **Un electrodomestico cualquiera**: la caja con el rayo.
fn electrodomestico(l: &mut Lapiz) {
    l.parte("Cuerpo");
    caja(l, 0.0, 0.0, 60.0, 60.0, TINTA, Some(BLANCO), 2.0);
    l.parte("Rayo");
    forma(
        l,
        &[
            (34.0, 8.0),
            (18.0, 34.0),
            (29.0, 34.0),
            (24.0, 52.0),
            (42.0, 25.0),
            (31.0, 25.0),
        ],
        TINTA,
        Some(hex(0xfff3c4)),
        1.25,
    );
}

// ---------------------------------------------------------------------------
// Bano
// ---------------------------------------------------------------------------

/// **Ducha a ras de suelo** (120 × 90): sin plato, el borde a trazos, las
/// pendientes hacia el sumidero lineal y la alcachofa.
fn ducha_a_ras(l: &mut Lapiz) {
    let (w, h) = (120.0, 90.0);
    l.parte("Zona de ducha");
    caja(l, 0.0, 0.0, w, h, FINA, Some(AGUA), 1.25);
    a_trazos(l);
    l.parte("Pendientes");
    for (x, y) in [(0.0, h), (w, h), (0.0, 0.0), (w, 0.0)] {
        l.linea(
            x,
            y,
            if x == 0.0 { 25.0 } else { w - 25.0 },
            12.0,
            FINA,
            0.6,
        );
    }
    l.parte("Sumidero lineal");
    caja(l, 25.0, 9.0, 70.0, 6.0, TINTA, Some(GRIS), 1.0);
    l.parte("Alcachofa");
    l.circulo(w / 2.0, 40.0, 7.0, Some(TINTA), Some(BLANCO), 1.0);
    for i in 0..6 {
        let a = (i as f32 * 60.0).to_radians();
        l.circulo(
            w / 2.0 + 3.5 * a.cos(),
            40.0 + 3.5 * a.sin(),
            0.8,
            Some(FINA),
            Some(FINA),
            0.5,
        );
    }
}

fn lavabo_circular(l: &mut Lapiz) {
    l.parte("Lavabo");
    l.circulo(25.0, 25.0, 23.0, Some(TINTA), Some(BLANCO), 1.75);
    l.circulo(25.0, 27.0, 16.0, Some(FINA), Some(AGUA), 1.0);
    l.circulo(25.0, 27.0, 2.0, Some(FINA), Some(FINA), 1.0);
    l.parte("Grifo");
    grifo(l, 25.0, 5.0, 8.0);
}

fn lavabo_eliptico(l: &mut Lapiz) {
    l.parte("Lavabo");
    l.elipse(0.0, 0.0, 60.0, 45.0, Some(TINTA), Some(BLANCO), 1.75);
    l.elipse(8.0, 9.0, 44.0, 31.0, Some(FINA), Some(AGUA), 1.0);
    l.circulo(30.0, 26.0, 2.0, Some(FINA), Some(FINA), 1.0);
    l.parte("Grifo");
    grifo(l, 30.0, 5.0, 8.0);
}

/// **Lavabo doble** sobre encimera de 140 × 55.
fn lavabo_doble(l: &mut Lapiz) {
    l.parte("Encimera");
    caja(l, 0.0, 0.0, 140.0, 55.0, TINTA, Some(ENCIMERA), 1.75);
    l.parte("Senos");
    for cx in [35.0, 105.0] {
        l.elipse(cx - 22.0, 14.0, 44.0, 32.0, Some(FINA), Some(AGUA), 1.25);
        l.circulo(cx, 30.0, 2.0, Some(FINA), Some(FINA), 1.0);
    }
    l.parte("Grifos");
    grifo(l, 35.0, 6.0, 8.0);
    grifo(l, 105.0, 6.0, 8.0);
}

/// **Lavamanos** (45 × 30), para el aseo pequeno.
fn lavabo_pequeno(l: &mut Lapiz) {
    l.parte("Lavamanos");
    l.rect(0.0, 0.0, 45.0, 30.0, Some(TINTA), Some(BLANCO), 1.75, true);
    l.rect(6.0, 8.0, 33.0, 18.0, Some(FINA), Some(AGUA), 1.0, true);
    l.circulo(22.5, 17.0, 1.8, Some(FINA), Some(FINA), 1.0);
    l.parte("Grifo");
    grifo(l, 22.5, 4.0, 5.0);
}

/// **Inodoro suspendido**: la cisterna empotrada en el muro (a trazos) y
/// la taza alargada, sin pie.
fn inodoro_suspendido(l: &mut Lapiz) {
    l.parte("Cisterna empotrada");
    caja(l, 0.0, 0.0, 50.0, 12.0, FINA, None, 1.0);
    a_trazos(l);
    l.parte("Pulsador");
    caja(l, 15.0, 12.0, 20.0, 4.0, TINTA, Some(GRIS), 1.0);
    l.parte("Taza");
    l.elipse(7.0, 14.0, 36.0, 54.0, Some(TINTA), Some(BLANCO), 1.75);
    l.elipse(12.0, 22.0, 26.0, 40.0, Some(FINA), Some(AGUA), 1.0);
}

/// **Inodoro de tanque bajo** con la taza en D (asiento redondeado
/// delante y recto detras).
fn inodoro_compacto(l: &mut Lapiz) {
    l.parte("Tanque");
    l.rect(0.0, 0.0, 46.0, 18.0, Some(TINTA), Some(BLANCO), 1.75, true);
    l.parte("Taza");
    let mut taza = vec![(5.0, 18.0), (41.0, 18.0), (41.0, 48.0)];
    taza.extend(puntos_de_arco(23.0, 48.0, 18.0, 0.0, 180.0));
    forma(l, &taza, TINTA, Some(BLANCO), 1.75);
    l.parte("Asiento");
    let mut hueco = vec![(11.0, 26.0), (35.0, 26.0), (35.0, 48.0)];
    hueco.extend(puntos_de_arco(23.0, 48.0, 12.0, 0.0, 180.0));
    forma(l, &hueco, FINA, Some(AGUA), 1.0);
}

/// **Urinario** de pared: la placa y el cuenco en D.
fn urinario(l: &mut Lapiz) {
    l.parte("Pared");
    l.linea(-6.0, 0.0, 42.0, 0.0, TINTA, 2.0);
    l.parte("Cuenco");
    let mut d = vec![(0.0, 0.0), (36.0, 0.0), (36.0, 14.0)];
    d.extend(puntos_de_arco(18.0, 14.0, 18.0, 0.0, 180.0));
    forma(l, &d, TINTA, Some(BLANCO), 1.75);
    let mut i = vec![(6.0, 5.0), (30.0, 5.0), (30.0, 14.0)];
    i.extend(puntos_de_arco(18.0, 14.0, 12.0, 0.0, 180.0));
    forma(l, &i, FINA, Some(AGUA), 1.0);
    l.circulo(18.0, 18.0, 1.8, Some(FINA), Some(FINA), 1.0);
}

/// **El sumidero** (desague de suelo): la rejilla cuadrada con sus barras.
fn desague(l: &mut Lapiz) {
    l.parte("Rejilla");
    caja(l, 0.0, 0.0, 15.0, 15.0, TINTA, Some(ACERO), 1.5);
    for i in 1..5 {
        let x = 3.0 * i as f32;
        l.linea(x, 2.5, x, 12.5, TINTA, 0.75);
    }
    l.parte("Pendiente");
    for (x, y) in [(-12.0, -12.0), (27.0, -12.0), (-12.0, 27.0), (27.0, 27.0)] {
        l.flecha(
            &[
                (x, y),
                (
                    if x < 0.0 { -2.0 } else { 17.0 },
                    if y < 0.0 { -2.0 } else { 17.0 },
                ),
            ],
            FINA,
            0.75,
            false,
        );
    }
}

/// **El toallero** de pared: la barra con sus dos soportes.
fn toallero(l: &mut Lapiz) {
    l.parte("Pared");
    l.linea(-6.0, 0.0, 66.0, 0.0, TINTA, 2.0);
    l.parte("Soportes");
    caja(l, 2.0, 0.0, 4.0, 8.0, TINTA, Some(GRIS), 1.0);
    caja(l, 54.0, 0.0, 4.0, 8.0, TINTA, Some(GRIS), 1.0);
    l.parte("Barra");
    l.rect(0.0, 8.0, 60.0, 3.0, Some(TINTA), Some(ACERO), 1.0, true);
    l.parte("Toalla");
    caja(l, 8.0, 11.0, 44.0, 4.0, FINA, Some(COJIN), 0.75);
}

/// **El colgador** (peg rail): el liston con sus colgadores.
fn colgador(l: &mut Lapiz) {
    l.parte("Listón");
    caja(l, 0.0, 0.0, 80.0, 4.0, TINTA, Some(MADERA), 1.25);
    l.parte("Colgadores");
    for i in 0..5 {
        let x = 8.0 + 16.0 * i as f32;
        l.linea(x, 4.0, x, 10.0, TINTA, 1.5);
        l.circulo(x, 11.0, 1.8, Some(TINTA), Some(TINTA), 1.0);
    }
}

/// **La barra de luz** (aplique sobre el espejo): la barra y tres focos.
fn barra_de_luz(l: &mut Lapiz) {
    l.parte("Pared");
    l.linea(-6.0, 0.0, 66.0, 0.0, TINTA, 2.0);
    l.parte("Barra");
    caja(l, 0.0, 0.0, 60.0, 5.0, TINTA, Some(GRIS), 1.0);
    l.parte("Focos");
    for x in [12.0, 30.0, 48.0] {
        l.circulo(x, 10.0, 5.0, Some(TINTA), Some(hex(0xfff3c4)), 1.0);
    }
}

// ---------------------------------------------------------------------------
// Muebles
// ---------------------------------------------------------------------------

/// **Cama litera** (90 × 190): la de abajo con su almohada, la de arriba a
/// trazos y la escalerilla a un lado.
fn litera(l: &mut Lapiz) {
    let (w, h) = (90.0, 190.0);
    l.parte("Cama de abajo");
    caja(l, 0.0, 0.0, w, h, TINTA, Some(BLANCO), 2.0);
    l.rect(
        10.0,
        10.0,
        w - 20.0,
        30.0,
        Some(FINA),
        Some(COJIN),
        1.0,
        true,
    );
    l.parte("Cama de arriba");
    caja(l, 4.0, 4.0, w - 8.0, h - 8.0, FINA, None, 0.75);
    a_trazos(l);
    l.linea(4.0, 4.0, w - 4.0, h - 4.0, FINA, 0.75);
    a_trazos(l);
    l.parte("Escalerilla");
    caja(l, w, 110.0, 12.0, 50.0, TINTA, Some(MADERA), 1.25);
    for i in 1..5 {
        let y = 110.0 + 10.0 * i as f32;
        l.linea(w, y, w + 12.0, y, TINTA, 1.0);
    }
}

fn mesita_redonda(l: &mut Lapiz) {
    l.parte("Mesita");
    l.circulo(22.5, 22.5, 22.5, Some(TINTA), Some(MADERA), 1.75);
    l.parte("Lámpara");
    lampara(l, 22.5, 22.5);
}

fn mesita_cuadrada(l: &mut Lapiz) {
    l.parte("Mesita");
    caja(l, 0.0, 0.0, 45.0, 45.0, TINTA, Some(MADERA), 1.75);
    l.parte("Lámpara");
    lampara(l, 22.5, 22.5);
}

/// La lampara vista desde arriba: la pantalla y la cruz de la bombilla.
fn lampara(l: &mut Lapiz, cx: f32, cy: f32) {
    l.circulo(cx, cy, 12.0, Some(FINA), Some(hex(0xfff3c4)), 1.0);
    l.linea(cx - 6.0, cy - 6.0, cx + 6.0, cy + 6.0, FINA, 0.75);
    l.linea(cx + 6.0, cy - 6.0, cx - 6.0, cy + 6.0, FINA, 0.75);
}

/// El interior de un armario: la barra a trazos y las perchas.
fn barra_y_perchas(l: &mut Lapiz, w: f32) {
    l.parte("Barra y perchas");
    l.linea(6.0, 30.0, w - 6.0, 30.0, FINA, 1.0);
    a_trazos(l);
    let n = (w / 18.0) as usize;
    for i in 0..n {
        let x = (w - 18.0 * (n - 1) as f32) / 2.0 + 18.0 * i as f32;
        l.linea(x - 5.0, 14.0, x + 5.0, 46.0, FINA, 1.25);
    }
}

/// **Un armario ropero** de `hojas` puertas de 60 abatibles.
fn armario_de_hojas(l: &mut Lapiz, hojas: usize) {
    let w = 60.0 * hojas as f32;
    l.parte("Mueble");
    caja(l, 0.0, 0.0, w, 60.0, TINTA, Some(BLANCO), 2.0);
    barra_y_perchas(l, w);
    l.parte("Puertas");
    for i in 0..hojas {
        let x0 = 60.0 * i as f32;
        // Las pares abren a la izquierda y las impares a la derecha.
        let (bx, desde, hasta) = if i % 2 == 0 {
            (x0, 0.0, 90.0)
        } else {
            (x0 + 60.0, 90.0, 180.0)
        };
        l.linea(bx, 60.0, bx, 120.0, TINTA, 1.5);
        l.arco(bx, 60.0, 60.0, desde, hasta, FINA, 0.75);
        a_trazos(l);
    }
}

/// **Armario de puertas correderas** (180 × 60).
fn armario_corredero(l: &mut Lapiz) {
    let w = 180.0;
    l.parte("Mueble");
    caja(l, 0.0, 0.0, w, 60.0, TINTA, Some(BLANCO), 2.0);
    barra_y_perchas(l, w);
    l.parte("Puertas correderas");
    caja(l, 2.0, 51.0, 92.0, 3.0, TINTA, Some(GRIS), 1.0);
    caja(l, 86.0, 55.0, 92.0, 3.0, TINTA, Some(GRIS), 1.0);
    l.parte("Sentido");
    l.flecha(&[(70.0, 70.0), (30.0, 70.0)], FINA, 1.0, false);
    l.flecha(&[(110.0, 70.0), (150.0, 70.0)], FINA, 1.0, false);
}

/// Un cojin de asiento.
fn cojin(l: &mut Lapiz, x: f32, y: f32, w: f32, h: f32) {
    l.rect(x, y, w, h, Some(FINA), Some(COJIN), 1.0, true);
}

/// **Divan en L** (sofa con chaise longue): 240 × 160.
fn divan(l: &mut Lapiz) {
    l.parte("Estructura");
    forma(
        l,
        &[
            (0.0, 0.0),
            (240.0, 0.0),
            (240.0, 160.0),
            (150.0, 160.0),
            (150.0, 90.0),
            (0.0, 90.0),
        ],
        TINTA,
        Some(TAPIZADO),
        2.0,
    );
    l.parte("Respaldo");
    l.rect(
        0.0,
        0.0,
        240.0,
        22.0,
        Some(TINTA),
        Some(TAPIZADO),
        1.25,
        true,
    );
    l.parte("Brazos");
    l.rect(
        0.0,
        0.0,
        20.0,
        90.0,
        Some(TINTA),
        Some(TAPIZADO),
        1.25,
        true,
    );
    l.rect(
        220.0,
        0.0,
        20.0,
        160.0,
        Some(TINTA),
        Some(TAPIZADO),
        1.25,
        true,
    );
    l.parte("Cojines");
    cojin(l, 20.0, 22.0, 65.0, 64.0);
    cojin(l, 85.0, 22.0, 65.0, 64.0);
    cojin(l, 150.0, 22.0, 70.0, 134.0);
}

/// **Sillon** (85 × 85).
fn sillon(l: &mut Lapiz) {
    l.parte("Respaldo");
    l.rect(0.0, 0.0, 85.0, 85.0, Some(TINTA), Some(TAPIZADO), 2.0, true);
    l.parte("Brazos");
    l.rect(
        0.0,
        0.0,
        16.0,
        85.0,
        Some(TINTA),
        Some(TAPIZADO),
        1.25,
        true,
    );
    l.rect(
        69.0,
        0.0,
        16.0,
        85.0,
        Some(TINTA),
        Some(TAPIZADO),
        1.25,
        true,
    );
    l.parte("Cojín");
    cojin(l, 16.0, 20.0, 53.0, 61.0);
}

fn mesa_centro_cuadrada(l: &mut Lapiz) {
    l.parte("Tablero");
    caja(l, 0.0, 0.0, 80.0, 80.0, TINTA, Some(MADERA), 2.0);
    l.parte("Bandeja de abajo");
    caja(l, 8.0, 8.0, 64.0, 64.0, FINA, None, 0.75);
    a_trazos(l);
}

fn mesa_centro_redonda(l: &mut Lapiz) {
    l.parte("Tablero");
    l.circulo(40.0, 40.0, 40.0, Some(TINTA), Some(MADERA), 2.0);
    l.parte("Bandeja de abajo");
    l.circulo(40.0, 40.0, 31.0, Some(FINA), None, 0.75);
    a_trazos(l);
}

/// **Mueble de television** (160 × 45) con la tele encima.
fn mueble_tv(l: &mut Lapiz) {
    l.parte("Mueble");
    caja(l, 0.0, 0.0, 160.0, 45.0, TINTA, Some(MADERA), 2.0);
    l.linea(53.0, 0.0, 53.0, 45.0, FINA, 0.75);
    l.linea(107.0, 0.0, 107.0, 45.0, FINA, 0.75);
    l.parte("Televisor");
    caja(l, 25.0, 14.0, 110.0, 5.0, TINTA, Some(CRISTAL_NEGRO), 1.25);
    caja(l, 70.0, 19.0, 20.0, 10.0, TINTA, Some(GRIS), 1.0);
}

/// **Estanteria** (100 × 35): los montantes y los libros vistos desde
/// arriba.
fn estanteria(l: &mut Lapiz) {
    l.parte("Mueble");
    caja(l, 0.0, 0.0, 100.0, 35.0, TINTA, Some(MADERA), 2.0);
    l.linea(50.0, 0.0, 50.0, 35.0, TINTA, 1.25);
    l.parte("Libros");
    let mut x = 4.0;
    for (i, g) in [3.0, 4.0, 2.5, 3.5, 3.0, 5.0, 2.5, 4.0, 3.0]
        .iter()
        .enumerate()
    {
        let alto = if i % 3 == 0 { 26.0 } else { 22.0 };
        caja(l, x, 4.0, *g, alto, FINA, Some(BLANCO), 0.6);
        x += g + 0.6;
    }
    let mut x = 54.0;
    for g in [4.0, 3.0, 3.0, 5.0, 2.5, 4.5] {
        caja(l, x, 4.0, g, 24.0, FINA, Some(BLANCO), 0.6);
        x += g + 0.6;
    }
}

/// **Estufa de lena**: la placa de suelo a puntos, el cuerpo y el tubo.
fn estufa(l: &mut Lapiz) {
    l.parte("Placa de suelo");
    caja(l, 0.0, 0.0, 90.0, 85.0, FINA, Some(hex(0xeceef1)), 1.0);
    a_puntos(l);
    l.parte("Estufa");
    l.rect(
        20.0,
        10.0,
        50.0,
        45.0,
        Some(TINTA),
        Some(CRISTAL_NEGRO),
        1.75,
        true,
    );
    l.parte("Tubo de humos");
    l.circulo(45.0, 26.0, 8.0, Some(GRIS), Some(GRIS), 1.0);
    l.parte("Puerta");
    l.linea(26.0, 55.0, 64.0, 55.0, TINTA, 2.0);
}

/// **Chimenea**: la fabrica (rayada) con el hogar trapecial y la solera.
fn chimenea(l: &mut Lapiz) {
    l.parte("Fábrica");
    l.rect(0.0, 0.0, 150.0, 45.0, None, Some(MURO), 1.0, false);
    rayado(l, 0.0, 0.0, 150.0, 45.0, 7.0, POCHE, 0.75);
    l.parte("Hogar");
    forma(
        l,
        &[(40.0, 10.0), (110.0, 10.0), (125.0, 45.0), (25.0, 45.0)],
        TINTA,
        Some(hex(0x6b7280)),
        1.5,
    );
    l.parte("Contorno");
    caja(l, 0.0, 0.0, 150.0, 45.0, TINTA, None, 2.0);
    l.parte("Solera");
    caja(l, 0.0, 45.0, 150.0, 35.0, FINA, Some(hex(0xeceef1)), 1.0);
    a_puntos(l);
}

/// La silla de despacho: asiento redondo y respaldo curvo.
fn silla_de_despacho(l: &mut Lapiz, cx: f32, cy: f32) {
    l.parte("Silla");
    l.circulo(cx, cy, 24.0, Some(TINTA), Some(COJIN), 1.5);
    l.arco(cx, cy, 30.0, 25.0, 155.0, TINTA, 4.0);
}

/// **Escritorio con silla** de `w × h`.
fn escritorio(l: &mut Lapiz, w: f32, h: f32) {
    l.parte("Escritorio");
    caja(l, 0.0, 0.0, w, h, TINTA, Some(MADERA), 2.0);
    l.parte("Pantalla y teclado");
    l.rect(
        w / 2.0 - 25.0,
        6.0,
        50.0,
        7.0,
        Some(TINTA),
        Some(GRIS),
        1.25,
        true,
    );
    l.rect(
        w / 2.0 - 20.0,
        h * 0.45,
        40.0,
        12.0,
        Some(FINA),
        Some(BLANCO),
        1.0,
        true,
    );
    silla_de_despacho(l, w / 2.0, h + 28.0);
}

/// **Mesa de comedor** de `w × h` con sus sillas: `por_lado` en cada lado
/// largo y una en cada cabecera si `cabeceras`.
fn comedor(l: &mut Lapiz, w: f32, h: f32, por_lado: usize, cabeceras: bool) {
    let (x0, y0) = (50.0, 50.0);
    let paso = w / por_lado as f32;
    for i in 0..por_lado {
        let x = x0 + paso * (i as f32 + 0.5);
        l.parte("Sillas");
        silla(l, x, y0 - 12.0, 0.0);
        silla(l, x, y0 + h + 12.0, 180.0);
    }
    if cabeceras {
        silla(l, x0 - 12.0, y0 + h / 2.0, -90.0);
        silla(l, x0 + w + 12.0, y0 + h / 2.0, 90.0);
    }
    l.parte("Mesa");
    caja(l, x0, y0, w, h, TINTA, Some(MADERA), 2.0);
}

/// **Mesa redonda grande** (Ø 150) con ocho sillas.
fn comedor_redondo(l: &mut Lapiz) {
    let (c, r) = (115.0, 75.0);
    l.parte("Sillas");
    for i in 0..8 {
        let g = 45.0 * i as f32;
        let a = (g - 90.0f32).to_radians();
        silla(l, c + (r + 12.0) * a.cos(), c + (r + 12.0) * a.sin(), g);
    }
    l.parte("Mesa");
    l.circulo(c, c, r, Some(TINTA), Some(MADERA), 2.0);
}

// ---------------------------------------------------------------------------
// Exterior y movilidad
// ---------------------------------------------------------------------------

/// Listones paralelos (madera de exterior) en una caja.
fn listones(l: &mut Lapiz, x: f32, y: f32, w: f32, h: f32, paso: f32) {
    let mut t = y + paso;
    while t < y + h - 0.5 {
        l.linea(x + 2.0, t, x + w - 2.0, t, FINA, 0.6);
        t += paso;
    }
}

/// **Tumbona** (70 × 195): el respaldo levantado y el resto de listones.
fn tumbona(l: &mut Lapiz) {
    l.parte("Bastidor");
    l.rect(
        0.0,
        0.0,
        70.0,
        195.0,
        Some(TINTA),
        Some(EXTERIOR_MADERA),
        2.0,
        true,
    );
    l.parte("Respaldo");
    caja(l, 5.0, 5.0, 60.0, 60.0, TINTA, Some(COJIN), 1.25);
    l.linea(5.0, 5.0, 35.0, 18.0, FINA, 0.6);
    l.linea(65.0, 5.0, 35.0, 18.0, FINA, 0.6);
    l.parte("Listones");
    listones(l, 5.0, 70.0, 60.0, 120.0, 8.0);
    l.parte("Ruedas");
    caja(l, -4.0, 8.0, 4.0, 14.0, TINTA, Some(GRIS), 1.0);
    caja(l, 70.0, 8.0, 4.0, 14.0, TINTA, Some(GRIS), 1.0);
}

/// **Sillon de exterior** (75 × 75) de listones.
fn sillon_exterior(l: &mut Lapiz) {
    l.parte("Bastidor");
    caja(l, 0.0, 0.0, 75.0, 75.0, TINTA, Some(EXTERIOR_MADERA), 2.0);
    l.parte("Brazos");
    caja(l, 0.0, 0.0, 10.0, 75.0, TINTA, Some(EXTERIOR_MADERA), 1.25);
    caja(l, 65.0, 0.0, 10.0, 75.0, TINTA, Some(EXTERIOR_MADERA), 1.25);
    l.parte("Respaldo");
    caja(l, 10.0, 0.0, 55.0, 12.0, TINTA, Some(EXTERIOR_MADERA), 1.25);
    l.parte("Listones");
    listones(l, 10.0, 12.0, 55.0, 63.0, 7.0);
}

/// **Sofa de exterior** (200 × 85): bastidor de listones y tres cojines.
fn sofa_exterior(l: &mut Lapiz) {
    l.parte("Bastidor");
    caja(l, 0.0, 0.0, 200.0, 85.0, TINTA, Some(EXTERIOR_MADERA), 2.0);
    l.parte("Brazos");
    caja(l, 0.0, 0.0, 12.0, 85.0, TINTA, Some(EXTERIOR_MADERA), 1.25);
    caja(
        l,
        188.0,
        0.0,
        12.0,
        85.0,
        TINTA,
        Some(EXTERIOR_MADERA),
        1.25,
    );
    l.parte("Respaldo");
    caja(l, 12.0, 0.0, 176.0, 16.0, TINTA, Some(COJIN), 1.25);
    l.parte("Cojines");
    for i in 0..3 {
        cojin(l, 14.0 + 58.0 * i as f32, 18.0, 56.0, 62.0);
    }
}

/// **Juego de bistro**: mesita redonda de 60 y dos sillas.
fn bistro(l: &mut Lapiz) {
    // El respaldo de cada silla, por fuera (180° la de la izquierda).
    for (cx, g) in [(25.0, 180.0f32), (135.0, 0.0)] {
        l.parte("Silla");
        l.circulo(cx, 40.0, 20.0, Some(TINTA), Some(BLANCO), 1.5);
        let a0 = g - 50.0;
        l.arco(cx, 40.0, 24.0, a0, a0 + 100.0, TINTA, 3.0);
    }
    l.parte("Mesa");
    l.circulo(80.0, 40.0, 30.0, Some(TINTA), Some(EXTERIOR_MADERA), 2.0);
}

/// **Cochecito** de bebe visto desde arriba (60 × 100).
fn cochecito(l: &mut Lapiz) {
    l.parte("Ruedas");
    for (x, y) in [(-4.0, 6.0), (60.0, 6.0), (-4.0, 74.0), (60.0, 74.0)] {
        caja(l, x, y, 4.0, 16.0, TINTA, Some(CRISTAL_NEGRO), 1.0);
    }
    l.parte("Capazo");
    l.rect(0.0, 0.0, 60.0, 96.0, Some(TINTA), Some(BLANCO), 1.75, true);
    l.parte("Capota");
    let mut capota = vec![(0.0, 40.0)];
    capota.extend(puntos_de_arco(30.0, 40.0, 30.0, 180.0, 360.0));
    capota.push((60.0, 40.0));
    forma(l, &capota, TINTA, Some(COJIN), 1.25);
    l.linea(4.0, 32.0, 56.0, 32.0, FINA, 0.6);
    l.linea(8.0, 22.0, 52.0, 22.0, FINA, 0.6);
    l.parte("Manillar");
    l.poli(
        &[(4.0, 96.0), (4.0, 112.0), (56.0, 112.0), (56.0, 96.0)],
        TINTA,
        2.5,
        None,
    );
}

/// **Bicicleta en planta** (180 de largo).
fn bicicleta_planta(l: &mut Lapiz) {
    l.parte("Ruedas");
    l.rect(
        0.0,
        26.0,
        66.0,
        6.0,
        Some(TINTA),
        Some(CRISTAL_NEGRO),
        1.0,
        true,
    );
    l.rect(
        114.0,
        26.0,
        66.0,
        6.0,
        Some(TINTA),
        Some(CRISTAL_NEGRO),
        1.0,
        true,
    );
    l.parte("Cuadro");
    l.linea(33.0, 29.0, 147.0, 29.0, TINTA, 3.0);
    l.parte("Manillar");
    l.poli(
        &[(140.0, 0.0), (146.0, 4.0), (146.0, 54.0), (140.0, 58.0)],
        TINTA,
        3.0,
        None,
    );
    l.rect(136.0, -4.0, 8.0, 6.0, Some(TINTA), Some(GRIS), 1.0, true);
    l.rect(136.0, 56.0, 8.0, 6.0, Some(TINTA), Some(GRIS), 1.0, true);
    l.parte("Sillín");
    forma(
        l,
        &[(52.0, 22.0), (78.0, 26.0), (78.0, 32.0), (52.0, 36.0)],
        TINTA,
        Some(CRISTAL_NEGRO),
        1.0,
    );
    l.parte("Pedales");
    l.rect(88.0, 14.0, 8.0, 6.0, Some(TINTA), Some(GRIS), 1.0, false);
    l.rect(96.0, 38.0, 8.0, 6.0, Some(TINTA), Some(GRIS), 1.0, false);
}

/// **Barbacoa** de bola (Ø 60) con su parrilla y las asas.
fn barbacoa(l: &mut Lapiz) {
    l.parte("Cuba");
    l.circulo(30.0, 30.0, 30.0, Some(TINTA), Some(CRISTAL_NEGRO), 2.0);
    l.parte("Parrilla");
    l.circulo(30.0, 30.0, 25.0, Some(GRIS), Some(hex(0x555b66)), 1.0);
    for i in 0..9 {
        let x = 10.0 + 5.0 * i as f32;
        let d = (25.0f32.powi(2) - (x - 30.0).powi(2)).max(0.0).sqrt();
        l.linea(x, 30.0 - d, x, 30.0 + d, GRIS, 0.75);
    }
    l.parte("Asas");
    caja(l, -8.0, 26.0, 8.0, 8.0, TINTA, Some(GRIS), 1.0);
    caja(l, 60.0, 26.0, 8.0, 8.0, TINTA, Some(GRIS), 1.0);
}

/// **Silla de ruedas en planta** (65 × 105): ruedas grandes a los lados,
/// asiento, respaldo, empunaduras, reposapies y ruedas delanteras.
fn silla_de_ruedas(l: &mut Lapiz) {
    l.parte("Ruedas grandes");
    l.rect(0.0, 12.0, 7.0, 60.0, Some(TINTA), Some(GRIS), 1.25, true);
    l.rect(58.0, 12.0, 7.0, 60.0, Some(TINTA), Some(GRIS), 1.25, true);
    l.parte("Asiento");
    l.rect(9.0, 18.0, 47.0, 46.0, Some(TINTA), Some(BLANCO), 1.5, true);
    l.parte("Respaldo y empuñaduras");
    l.linea(9.0, 14.0, 56.0, 14.0, TINTA, 2.5);
    l.linea(12.0, 14.0, 12.0, 4.0, TINTA, 2.0);
    l.linea(53.0, 14.0, 53.0, 4.0, TINTA, 2.0);
    l.parte("Ruedas delanteras");
    caja(l, 10.0, 80.0, 4.0, 10.0, TINTA, Some(GRIS), 1.0);
    caja(l, 51.0, 80.0, 4.0, 10.0, TINTA, Some(GRIS), 1.0);
    l.parte("Reposapiés");
    l.rect(16.0, 88.0, 33.0, 10.0, Some(TINTA), Some(GRIS), 1.0, true);
}

/// **Patinillo** (shaft de instalaciones): la camara de muros delgados con
/// el aspa del hueco que atraviesa los forjados.
fn patinillo(l: &mut Lapiz) {
    let (s, e) = (80.0, 8.0);
    l.parte("Muros");
    tramo(l, 0.0, 0.0, s, e);
    tramo(l, 0.0, s - e, s, e);
    tramo(l, 0.0, e, e, s - 2.0 * e);
    tramo(l, s - e, e, e, s - 2.0 * e);
    l.parte("Hueco");
    l.linea(e, e, s - e, s - e, TINTA, 1.0);
    l.linea(s - e, e, e, s - e, TINTA, 1.0);
}

// ---------------------------------------------------------------------------
// El registro
// ---------------------------------------------------------------------------

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let estructura: [Dibujo; 17] = [
        ("planta-muro-delgado", "Muro delgado (10)", |l| {
            muro_recto(l, 10.0)
        }),
        ("planta-muro-medio", "Muro medio (15)", |l| {
            muro_recto(l, 15.0)
        }),
        ("planta-muro-grueso", "Muro grueso (30)", |l| {
            muro_recto(l, 30.0)
        }),
        ("planta-muro-curvo-1", "Muro curvo (60°)", |l| {
            muro_curvo(l, 200.0, 15.0, -120.0, -60.0)
        }),
        ("planta-muro-curvo-2", "Muro curvo (90°)", |l| {
            muro_curvo(l, 150.0, 15.0, -135.0, -45.0)
        }),
        ("planta-muro-curvo-3", "Muro curvo (semicírculo)", |l| {
            muro_curvo(l, 120.0, 15.0, -180.0, 0.0)
        }),
        ("planta-columna-rect", "Columna rectangular", columna_rect),
        (
            "planta-columna-rect-h",
            "Columna rectangular con perfil H",
            columna_rect_h,
        ),
        (
            "planta-columna-rect-x",
            "Columna rectangular con aspa",
            columna_rect_x,
        ),
        ("planta-columna-circ", "Columna circular", columna_circular),
        (
            "planta-columna-circ-h",
            "Columna circular con perfil H",
            columna_circular_h,
        ),
        (
            "planta-escalera-modulo",
            "Módulo de escalera (5 peldaños)",
            escalera_modulo,
        ),
        ("planta-ascensor", "Ascensor", ascensor),
        ("planta-rotura", "Línea de rotura", linea_de_rotura),
        (
            "planta-recorrido",
            "Flecha de recorrido",
            flecha_de_recorrido,
        ),
        ("planta-acceso", "Acceso", acceso),
        ("planta-alfombra", "Alfombra", alfombra),
    ];
    let puertas: [Dibujo; 8] = [
        (
            "planta-puerta-empotrada",
            "Puerta corredera empotrada",
            puerta_empotrada,
        ),
        (
            "planta-correderas-2",
            "Correderas que se cruzan (2 hojas)",
            correderas_2,
        ),
        (
            "planta-correderas-4",
            "Correderas que se cruzan (4 hojas)",
            correderas_4,
        ),
        ("planta-plegable", "Puerta plegable (acordeón)", plegable),
        (
            "planta-puerta-mano-1",
            "Puerta 1 (mano alzada)",
            puerta_mano_1,
        ),
        (
            "planta-puerta-mano-2",
            "Puerta 2 de dos hojas (mano alzada)",
            puerta_mano_2,
        ),
        ("planta-ventana-mano", "Ventana (mano alzada)", ventana_mano),
        (
            "planta-ventana-corredera",
            "Ventana corredera",
            ventana_corredera,
        ),
    ];
    let cocina_y_bano: [Dibujo; 31] = [
        ("planta-placa-electrica", "Placa eléctrica", placa_electrica),
        ("planta-cocina-gas", "Cocina de gas", cocina_de_gas),
        ("planta-horno-fogones", "Horno con fogones", horno_fogones),
        (
            "planta-frigo-congelador",
            "Frigorífico-congelador",
            frigo_congelador,
        ),
        ("planta-fregadero", "Fregadero simple", fregadero_simple),
        ("planta-fregadero-doble", "Fregadero doble", fregadero_doble),
        ("planta-isla-120", "Isla de cocina 120 × 80", |l| {
            isla(l, 120.0, 80.0, 0)
        }),
        ("planta-isla-180", "Isla de cocina 180 × 90", |l| {
            isla(l, 180.0, 90.0, 0)
        }),
        (
            "planta-isla-240",
            "Isla de cocina 240 × 100 con taburetes",
            |l| isla(l, 240.0, 100.0, 4),
        ),
        ("planta-modulo-60", "Módulo de cocina 60", |l| {
            modulo(l, 60.0)
        }),
        ("planta-modulo-90", "Módulo de cocina 90", |l| {
            modulo(l, 90.0)
        }),
        ("planta-modulo-120", "Módulo de cocina 120", |l| {
            modulo(l, 120.0)
        }),
        ("planta-cajonera", "Cajonera", cajonera),
        ("planta-armario-doble", "Armario doble", |l| {
            armario_bajo(l, 80.0)
        }),
        ("planta-armario-grande", "Armario grande", |l| {
            armario_bajo(l, 120.0)
        }),
        (
            "planta-armario-alto",
            "Armario alto de cocina",
            armario_alto,
        ),
        ("planta-lavadora", "Lavadora", |l| {
            de_tambor(l, "LAV.", false)
        }),
        ("planta-secadora", "Secadora", |l| {
            de_tambor(l, "SEC.", true)
        }),
        (
            "planta-electrodomestico",
            "Electrodoméstico",
            electrodomestico,
        ),
        ("planta-ducha-ras", "Ducha a ras de suelo", ducha_a_ras),
        ("planta-lavabo-circular", "Lavabo circular", lavabo_circular),
        ("planta-lavabo-eliptico", "Lavabo elíptico", lavabo_eliptico),
        ("planta-lavabo-doble", "Lavabo doble", lavabo_doble),
        ("planta-lavabo-pequeno", "Lavabo pequeño", lavabo_pequeno),
        (
            "planta-inodoro-suspendido",
            "Inodoro suspendido",
            inodoro_suspendido,
        ),
        (
            "planta-inodoro-compacto",
            "Inodoro de tanque bajo",
            inodoro_compacto,
        ),
        ("planta-urinario", "Urinario", urinario),
        ("planta-desague", "Desagüe (sumidero)", desague),
        ("planta-toallero", "Toallero", toallero),
        ("planta-colgador", "Colgador (peg rail)", colgador),
        ("planta-barra-luz", "Barra de luz", barra_de_luz),
    ];
    let muebles: [Dibujo; 19] = [
        ("planta-litera", "Litera", litera),
        (
            "planta-mesita-redonda",
            "Mesita de noche redonda",
            mesita_redonda,
        ),
        (
            "planta-mesita-cuadrada",
            "Mesita de noche cuadrada",
            mesita_cuadrada,
        ),
        ("planta-armario-simple", "Armario ropero simple", |l| {
            armario_de_hojas(l, 1)
        }),
        ("planta-armario-triple", "Armario ropero triple", |l| {
            armario_de_hojas(l, 3)
        }),
        (
            "planta-armario-corredero",
            "Armario ropero corredero",
            armario_corredero,
        ),
        ("planta-divan", "Diván (sofá en L)", divan),
        ("planta-sillon", "Sillón", sillon),
        (
            "planta-mesa-centro-cuadrada",
            "Mesa de centro cuadrada",
            mesa_centro_cuadrada,
        ),
        (
            "planta-mesa-centro-redonda",
            "Mesa de centro redonda",
            mesa_centro_redonda,
        ),
        ("planta-mueble-tv", "Mueble de TV", mueble_tv),
        ("planta-estanteria", "Estantería", estanteria),
        ("planta-estufa", "Estufa", estufa),
        ("planta-chimenea", "Chimenea", chimenea),
        (
            "planta-escritorio-pequeno",
            "Escritorio pequeño con silla",
            |l| escritorio(l, 100.0, 55.0),
        ),
        (
            "planta-escritorio-grande",
            "Escritorio grande con silla",
            |l| escritorio(l, 180.0, 80.0),
        ),
        (
            "planta-comedor-4",
            "Mesa de comedor pequeña (4 sillas)",
            |l| comedor(l, 120.0, 80.0, 2, false),
        ),
        (
            "planta-comedor-8",
            "Mesa de comedor grande (8 sillas)",
            |l| comedor(l, 240.0, 100.0, 3, true),
        ),
        (
            "planta-comedor-redonda-8",
            "Mesa redonda grande (8 sillas)",
            comedor_redondo,
        ),
    ];
    let exterior: [Dibujo; 9] = [
        ("planta-tumbona", "Tumbona", tumbona),
        (
            "planta-sillon-exterior",
            "Sillón de exterior",
            sillon_exterior,
        ),
        ("planta-sofa-exterior", "Sofá de exterior", sofa_exterior),
        ("planta-bistro", "Juego de bistró", bistro),
        ("planta-cochecito", "Cochecito", cochecito),
        ("planta-bicicleta", "Bicicleta (planta)", bicicleta_planta),
        ("planta-barbacoa", "Barbacoa", barbacoa),
        (
            "planta-silla-de-ruedas",
            "Silla de ruedas (planta)",
            silla_de_ruedas,
        ),
        ("planta-patinillo", "Patinillo (shaft)", patinillo),
    ];
    poner(v, ESTRUCTURA, &estructura);
    poner(v, PUERTAS, &puertas);
    poner(v, COCINA_Y_BANO, &cocina_y_bano);
    poner(v, MUEBLES, &muebles);
    poner(v, EXTERIOR, &exterior);
}

#[cfg(test)]
mod pruebas {
    use super::super::{PREFIJO_PARTE, catalogo};
    use crate::biblioteca;
    use crate::vector::Punto2;

    /// Las figuras de planta y las personas van por partes con nombre, y
    /// las partes sobreviven al estampado como subgrupos dentro del grupo
    /// de la figura (y propios de esa estampacion).
    #[test]
    fn las_partes_viajan_como_subgrupos_al_estampar() {
        let c = catalogo();
        for f in c
            .iter()
            .filter(|f| f.id.starts_with("serie-planta-") || f.id.starts_with("serie-pers-"))
        {
            assert!(
                f.elementos
                    .iter()
                    .all(|e| e.grupos.iter().any(|g| g.starts_with(PREFIJO_PARTE))),
                "{}: hay piezas sin parte",
                f.nombre
            );
        }
        let cama = c.iter().find(|f| f.id == "serie-planta-litera").unwrap();
        let a = biblioteca::estampar(cama, Punto2::nuevo(0.0, 0.0), 10, "g10");
        let b = biblioteca::estampar(cama, Punto2::nuevo(300.0, 0.0), 100, "g100");
        assert!(a.iter().all(|e| e.grupos.last().unwrap() == "g10"));
        assert!(
            a.iter()
                .any(|e| e.grupos.contains(&"parte:Escalerilla@g10".to_string()))
        );
        assert!(
            b.iter()
                .any(|e| e.grupos.contains(&"parte:Escalerilla@g100".to_string()))
        );
        assert!(
            a.iter()
                .all(|e| e.grupos.iter().all(|g| !g.ends_with("@g100")))
        );
    }

    /// Las de planta van a 1 px = 1 cm: medidas de catalogo.
    #[test]
    fn la_escala_de_planta_es_la_de_verdad() {
        let c = catalogo();
        let medida = |id: &str| {
            let f = c.iter().find(|f| f.id == id).unwrap();
            let (x0, y0, x1, y1) = biblioteca::caja_de(&f.elementos).unwrap();
            (x1 - x0, y1 - y0)
        };
        let (w, h) = medida("serie-planta-litera");
        assert!((100.0..=106.0).contains(&w) && (189.0..=193.0).contains(&h));
        let (w, _) = medida("serie-planta-modulo-60");
        assert!((59.0..=62.0).contains(&w), "{w}");
        // Las personas de pie, entre 160 y 175 de alto.
        let (_, h) = medida("serie-pers-hombre-frente");
        assert!((165.0..=176.0).contains(&h), "{h}");
        let (_, h) = medida("serie-pers-mujer-frente");
        assert!((158.0..=172.0).contains(&h), "{h}");
    }
}
