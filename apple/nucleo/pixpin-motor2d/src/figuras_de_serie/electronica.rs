//! **Los simbolos de electronica** (24-oct-2026): fuentes, componentes
//! pasivos, semiconductores, amplificadores y compuertas logicas, con los
//! dibujos normalizados de siempre (IEC y ANSI) a trazo limpio.
//!
//! **Las patas acaban en puntos redondos** para enganchar flechas y lineas:
//! los de dos patas van en horizontal, de `x = 0` a `x = 120` sobre el eje
//! `y = 30` ([`Y`]); los de tres (transistores, potenciometro) y las
//! compuertas acaban tambien en multiplos de 10.
//!
//! Aqui tambien viven los trazos que comparten el diagrama de flujo y las
//! marcas de revision: formas rellenas ([`region`]), arcos en puntos,
//! curvas de Bezier y textos centrados.

use super::tecnico::{Dibujo, poner};
use super::{Al, Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, Figura};
use crate::texto::{self, EstiloDeTexto};

pub(super) const CATEGORIA: &str = "Electrónica";

const TINTA: ColorRgba = hex(0x2b2f3a);
const BLANCO: ColorRgba = hex(0xffffff);
/// El grosor normal de las patas y los cuerpos.
const G: f32 = 2.0;
/// El eje de los de dos patas.
const Y: f32 = 30.0;

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: &[Dibujo] = &[
        (
            "elec-fuente-corriente-ca",
            "Fuente de corriente CA",
            fuente_corriente_ca,
        ),
        (
            "elec-fuente-tension-ca",
            "Fuente de tensión CA",
            fuente_tension_ca,
        ),
        ("elec-diac", "Interruptor bilateral (DIAC)", diac),
        ("elec-resistencia", "Resistencia", |l| {
            resistencia_zigzag(l, false)
        }),
        ("elec-resistencia-iec", "Resistencia (IEC)", resistencia_iec),
        ("elec-resistencia-variable", "Resistencia variable", |l| {
            resistencia_zigzag(l, true)
        }),
        ("elec-potenciometro", "Potenciómetro", potenciometro),
        ("elec-condensador", "Condensador", |l| condensador(l, false)),
        (
            "elec-condensador-polarizado",
            "Condensador polarizado",
            condensador_polarizado,
        ),
        ("elec-condensador-variable", "Condensador variable", |l| {
            condensador(l, true)
        }),
        ("elec-pila", "Pila", |l| pila(l, 1)),
        ("elec-bateria", "Batería", |l| pila(l, 2)),
        ("elec-clavija", "Clavija macho", clavija),
        ("elec-jack", "Jack hembra", jack),
        ("elec-cristal", "Cristal de cuarzo", cristal),
        ("elec-diodo", "Diodo", |l| diodo(l, Diodo::Normal)),
        ("elec-fotodiodo", "Fotodiodo", |l| diodo(l, Diodo::Foto)),
        ("elec-led", "LED", |l| diodo(l, Diodo::Led)),
        ("elec-zener", "Diodo zener", |l| diodo(l, Diodo::Zener)),
        ("elec-fet-n", "FET de canal N", |l| fet(l, true)),
        ("elec-fet-p", "FET de canal P", |l| fet(l, false)),
        ("elec-npn", "Transistor NPN", |l| bjt(l, true)),
        ("elec-pnp", "Transistor PNP", |l| bjt(l, false)),
        ("elec-tierra", "Tierra", tierra),
        ("elec-antena-1", "Antena 1", antena_1),
        ("elec-antena-2", "Antena 2", antena_2),
        ("elec-amplificador", "Amplificador (buffer)", |l| {
            buffer(l, false)
        }),
        ("elec-amp-op", "Amplificador operacional", amp_op),
        ("elec-inversor", "Inversor (NOT)", |l| buffer(l, true)),
        ("elec-and", "Compuerta AND", |l| {
            compuerta(l, Puerta::And, false, false)
        }),
        ("elec-nand", "Compuerta NAND", |l| {
            compuerta(l, Puerta::And, true, false)
        }),
        ("elec-and-negada", "Compuerta AND negada", |l| {
            compuerta(l, Puerta::And, false, true)
        }),
        ("elec-or", "Compuerta OR", |l| {
            compuerta(l, Puerta::Or, false, false)
        }),
        ("elec-nor", "Compuerta NOR", |l| {
            compuerta(l, Puerta::Or, true, false)
        }),
        ("elec-or-negada", "Compuerta OR negada", |l| {
            compuerta(l, Puerta::Or, false, true)
        }),
        ("elec-xor", "Compuerta XOR", |l| {
            compuerta(l, Puerta::Xor, false, false)
        }),
        ("elec-xnor", "Compuerta XNOR", |l| {
            compuerta(l, Puerta::Xor, true, false)
        }),
        ("elec-bobina", "Bobina (inductor)", bobina),
        ("elec-transformador", "Transformador", transformador),
        ("elec-motor", "Motor", |l| en_circulo(l, "M", 24.0)),
        ("elec-lampara", "Lámpara", lampara),
        ("elec-fusible", "Fusible", fusible),
        ("elec-interruptor", "Interruptor", interruptor),
        ("elec-pulsador", "Pulsador", pulsador),
        ("elec-voltimetro", "Voltímetro", |l| {
            en_circulo(l, "V", 22.0)
        }),
        ("elec-amperimetro", "Amperímetro", |l| {
            en_circulo(l, "A", 22.0)
        }),
    ];
    poner(v, CATEGORIA, lista);
}

// ----------------------------------------------------- trazos compartidos

/// **Una forma cerrada y rellena** (sin repetir el primer punto): va como
/// region, que es lo que el lienzo rellena; una linea cerrada no.
pub(super) fn region(
    l: &mut Lapiz,
    pts: &[(f32, f32)],
    trazo: ColorRgba,
    relleno: ColorRgba,
    g: f32,
) {
    l.poli(pts, trazo, g, Some(relleno));
    if let Some(e) = l.v.last_mut()
        && let Figura::Linea { puntos } = &mut e.figura
    {
        let contorno = std::mem::take(puntos);
        e.figura = Figura::Region {
            contorno,
            huecos: Vec::new(),
        };
    }
}

/// Los puntos de un arco de elipse, con sus dos extremos (grados, 0 a la
/// derecha y creciendo hacia abajo, como la pantalla).
pub(super) fn arco_pts(
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    desde: f32,
    hasta: f32,
) -> Vec<(f32, f32)> {
    let n = (((hasta - desde).abs() / 6.0).ceil() as usize).max(2);
    (0..=n)
        .map(|i| {
            let a = (desde + (hasta - desde) * i as f32 / n as f32).to_radians();
            (cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}

/// Una curva de Bezier cuadratica de `a` a `b` tirando hacia `c`.
pub(super) fn bezier(a: (f32, f32), c: (f32, f32), b: (f32, f32)) -> Vec<(f32, f32)> {
    let n = 16;
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            (
                u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0,
                u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1,
            )
        })
        .collect()
}

/// Un texto centrado en `(cx, cy)`.
pub(super) fn texto_en(
    l: &mut Lapiz,
    cx: f32,
    cy: f32,
    s: &str,
    tam: f32,
    c: ColorRgba,
    negrita: bool,
) {
    let estilo = EstiloDeTexto {
        negrita,
        ..Default::default()
    };
    let (_, h) = texto::medida(s, tam, WORK_SANS, estilo);
    l.texto(cx, cy - h / 2.0, s, tam, WORK_SANS, c, negrita, Al::Centro);
}

// ------------------------------------------------------- trazos de aqui

fn raya(l: &mut Lapiz, a: (f32, f32), b: (f32, f32)) {
    l.linea(a.0, a.1, b.0, b.1, TINTA, G);
}

/// Una punta rellena con el pico en `tip` apuntando hacia `grados`.
fn punta(l: &mut Lapiz, tip: (f32, f32), grados: f32, largo: f32, ancho: f32) {
    let (s, c) = grados.to_radians().sin_cos();
    let base = (tip.0 - c * largo, tip.1 - s * largo);
    let (nx, ny) = (-s * ancho / 2.0, c * ancho / 2.0);
    region(
        l,
        &[tip, (base.0 + nx, base.1 + ny), (base.0 - nx, base.1 - ny)],
        TINTA,
        TINTA,
        1.0,
    );
}

/// Una flecha fina de `a` a `b` con la punta rellena (las de luz y las de
/// los componentes variables).
fn flechita(l: &mut Lapiz, a: (f32, f32), b: (f32, f32), largo: f32, g: f32) {
    let ang = (b.1 - a.1).atan2(b.0 - a.0);
    let fin = (b.0 - ang.cos() * largo * 0.8, b.1 - ang.sin() * largo * 0.8);
    l.linea(a.0, a.1, fin.0, fin.1, TINTA, g);
    punta(l, b, ang.to_degrees(), largo, largo * 0.8);
}

/// La bolita de negacion de las compuertas.
fn burbuja(l: &mut Lapiz, cx: f32, cy: f32) {
    l.circulo(cx, cy, 6.0, Some(TINTA), Some(BLANCO), G);
}

/// Las dos patas de un componente horizontal: de 0 a `a` y de `b` a 120,
/// cada una su parte.
fn patas(l: &mut Lapiz, a: f32, b: f32) {
    // Parte «Terminal izquierdo»: la pata de la izquierda, acaba en x = 0.
    l.parte("Terminal izquierdo");
    raya(l, (0.0, Y), (a, Y));
    // Parte «Terminal derecho»: la pata de la derecha, acaba en x = 120.
    l.parte("Terminal derecho");
    raya(l, (b, Y), (120.0, Y));
}

// ---------------------------------------------------------------- fuentes

/// El circulo de las fuentes, de pie: patas arriba (40, 0) y abajo (40, 88).
fn fuente(l: &mut Lapiz) {
    // Parte «Terminal superior» y «Terminal inferior»: las patas, en (40, 0)
    // y (40, 88).
    l.parte("Terminal superior");
    raya(l, (40.0, 0.0), (40.0, 20.0));
    l.parte("Terminal inferior");
    raya(l, (40.0, 68.0), (40.0, 88.0));
    // Parte «Cuerpo»: el circulo de la fuente.
    l.parte("Cuerpo");
    l.circulo(40.0, 44.0, 24.0, Some(TINTA), Some(BLANCO), G);
}

/// Una onda senoidal de una vuelta de `x0` a `x1` en `cy`.
fn seno(l: &mut Lapiz, x0: f32, x1: f32, cy: f32, amp: f32, g: f32) {
    let pts: Vec<(f32, f32)> = (0..=24)
        .map(|i| {
            let t = i as f32 / 24.0;
            (
                x0 + (x1 - x0) * t,
                cy - amp * (t * std::f32::consts::TAU).sin(),
            )
        })
        .collect();
    l.poli(&pts, TINTA, g, None);
}

fn fuente_tension_ca(l: &mut Lapiz) {
    fuente(l);
    // Parte «Onda alterna»: la senoide que dice que es de alterna.
    l.parte("Onda alterna");
    seno(l, 27.0, 53.0, 44.0, 8.0, G);
}

fn fuente_corriente_ca(l: &mut Lapiz) {
    fuente(l);
    // Parte «Sentido de la corriente»: la flecha de las fuentes de corriente.
    l.parte("Sentido de la corriente");
    flechita(l, (34.0, 60.0), (34.0, 28.0), 9.0, G);
    // Parte «Onda alterna»: la senoide pequena junto a la flecha.
    l.parte("Onda alterna");
    seno(l, 41.0, 55.0, 44.0, 4.5, 1.6);
}

/// El DIAC, de pie: dos diodos en antiparalelo entre dos placas.
fn diac(l: &mut Lapiz) {
    // Parte «Terminal superior» / «Terminal inferior»: patas en (40, 0) y
    // (40, 90).
    l.parte("Terminal superior");
    raya(l, (40.0, 0.0), (40.0, 28.0));
    l.parte("Terminal inferior");
    raya(l, (40.0, 62.0), (40.0, 90.0));
    // Parte «Placas»: los dos electrodos.
    l.parte("Placas");
    l.linea(10.0, 28.0, 70.0, 28.0, TINTA, 2.5);
    l.linea(10.0, 62.0, 70.0, 62.0, TINTA, 2.5);
    // Parte «Diodo hacia abajo» y «Diodo hacia arriba»: los dos sentidos
    // en que conduce cuando se dispara.
    l.parte("Diodo hacia abajo");
    region(
        l,
        &[(14.0, 28.0), (39.0, 28.0), (26.5, 62.0)],
        TINTA,
        TINTA,
        1.0,
    );
    l.parte("Diodo hacia arriba");
    region(
        l,
        &[(41.0, 62.0), (66.0, 62.0), (53.5, 28.0)],
        TINTA,
        TINTA,
        1.0,
    );
}

// ---------------------------------------------------------------- pasivos

/// La flecha cruzada de los componentes variables.
fn ajuste(l: &mut Lapiz) {
    // Parte «Ajuste»: la flecha en diagonal que dice que el valor cambia.
    l.parte("Ajuste");
    flechita(l, (36.0, Y + 26.0), (86.0, Y - 26.0), 10.0, 1.6);
}

fn resistencia_zigzag(l: &mut Lapiz, variable: bool) {
    patas(l, 30.0, 90.0);
    // Parte «Cuerpo»: el zigzag (norma ANSI), de x = 30 a x = 90.
    l.parte("Cuerpo");
    let mut pts = vec![(30.0, Y)];
    for i in 0..6 {
        let s = if i % 2 == 0 { -12.0 } else { 12.0 };
        pts.push((35.0 + 10.0 * i as f32, Y + s));
    }
    pts.push((90.0, Y));
    l.poli(&pts, TINTA, G, None);
    if variable {
        ajuste(l);
    }
}

fn resistencia_iec(l: &mut Lapiz) {
    patas(l, 30.0, 90.0);
    // Parte «Cuerpo»: el rectangulo de la norma IEC.
    l.parte("Cuerpo");
    l.rect(
        30.0,
        Y - 12.0,
        60.0,
        24.0,
        Some(TINTA),
        Some(BLANCO),
        G,
        false,
    );
}

/// Con la tercera pata (el cursor) abajo, en (60, 74).
fn potenciometro(l: &mut Lapiz) {
    resistencia_zigzag(l, false);
    // Parte «Cursor»: la tercera pata, con la punta que toca la resistencia;
    // acaba en (60, 74).
    l.parte("Cursor");
    raya(l, (60.0, Y + 44.0), (60.0, Y + 22.0));
    punta(l, (60.0, Y + 13.0), -90.0, 11.0, 10.0);
}

fn condensador(l: &mut Lapiz, variable: bool) {
    patas(l, 54.0, 66.0);
    // Parte «Placas»: las dos armaduras paralelas.
    l.parte("Placas");
    l.linea(54.0, Y - 20.0, 54.0, Y + 20.0, TINTA, 2.5);
    l.linea(66.0, Y - 20.0, 66.0, Y + 20.0, TINTA, 2.5);
    if variable {
        ajuste(l);
    }
}

/// El signo `+` pequeno en `(cx, cy)`, en su parte «Polaridad».
fn mas(l: &mut Lapiz, cx: f32, cy: f32) {
    // Parte «Polaridad»: el `+` que marca el lado positivo.
    l.parte("Polaridad");
    l.linea(cx - 4.0, cy, cx + 4.0, cy, TINTA, 1.5);
    l.linea(cx, cy - 4.0, cx, cy + 4.0, TINTA, 1.5);
}

fn condensador_polarizado(l: &mut Lapiz) {
    patas(l, 54.0, 66.0);
    // Parte «Placa positiva»: la recta.
    l.parte("Placa positiva");
    l.linea(54.0, Y - 20.0, 54.0, Y + 20.0, TINTA, 2.5);
    // Parte «Placa negativa»: la curva.
    l.parte("Placa negativa");
    let curva = arco_pts(92.0, Y, 26.0, 26.0, 140.0, 220.0);
    l.poli(&curva, TINTA, 2.5, None);
    mas(l, 44.0, Y - 16.0);
}

/// Una pila (`celdas` = 1) o una bateria: placa larga (+) y corta (−).
fn pila(l: &mut Lapiz, celdas: usize) {
    let ancho = 12.0 + 22.0 * (celdas as f32 - 1.0);
    let x0 = 60.0 - ancho / 2.0;
    patas(l, x0, x0 + ancho);
    for i in 0..celdas {
        // Parte «Celda n»: placa larga (positivo) y corta y gruesa (negativo).
        l.parte(&if celdas == 1 {
            "Celda".to_string()
        } else {
            format!("Celda {}", i + 1)
        });
        let x = x0 + 22.0 * i as f32;
        l.linea(x, Y - 20.0, x, Y + 20.0, TINTA, 2.0);
        l.linea(x + 12.0, Y - 9.0, x + 12.0, Y + 9.0, TINTA, 5.0);
    }
    mas(l, x0 - 10.0, Y - 16.0);
}

/// La clavija macho: la pata en `x = 0` y la punta abierta al final.
fn clavija(l: &mut Lapiz) {
    // Parte «Terminal»: el cable, acaba en x = 0.
    l.parte("Terminal");
    raya(l, (0.0, Y), (70.0, Y));
    // Parte «Punta»: el macho que entra en el jack.
    l.parte("Punta");
    l.poli(
        &[(56.0, Y - 14.0), (71.0, Y), (56.0, Y + 14.0)],
        TINTA,
        G,
        None,
    );
}

/// El jack hembra: la boca en V a la izquierda y la pata en `x = 80`.
fn jack(l: &mut Lapiz) {
    // Parte «Boca»: la V donde entra la clavija.
    l.parte("Boca");
    l.poli(
        &[(0.0, Y - 14.0), (14.0, Y), (0.0, Y + 14.0)],
        TINTA,
        G,
        None,
    );
    // Parte «Terminal»: el cable, acaba en x = 80.
    l.parte("Terminal");
    raya(l, (14.0, Y), (80.0, Y));
}

fn cristal(l: &mut Lapiz) {
    patas(l, 44.0, 76.0);
    // Parte «Electrodos»: las dos placas a los lados.
    l.parte("Electrodos");
    l.linea(44.0, Y - 16.0, 44.0, Y + 16.0, TINTA, 2.5);
    l.linea(76.0, Y - 16.0, 76.0, Y + 16.0, TINTA, 2.5);
    // Parte «Cristal»: la lamina de cuarzo entre ellas.
    l.parte("Cristal");
    l.rect(
        51.0,
        Y - 20.0,
        18.0,
        40.0,
        Some(TINTA),
        Some(BLANCO),
        G,
        false,
    );
}

fn bobina(l: &mut Lapiz) {
    // Parte «Terminal izquierdo» / «Terminal derecho»: patas en x = 0 y 120.
    l.parte("Terminal izquierdo");
    raya(l, (0.0, Y), (12.0, Y));
    l.parte("Terminal derecho");
    raya(l, (108.0, Y), (120.0, Y));
    // Parte «Devanado»: las cuatro espiras.
    l.parte("Devanado");
    let mut pts = Vec::new();
    for i in 0..4 {
        let cx = 24.0 + 24.0 * i as f32;
        pts.extend(arco_pts(cx, Y, 12.0, 21.0, 180.0, 360.0));
    }
    pts.dedup();
    l.poli(&pts, TINTA, G, None);
}

/// Un devanado de pie de `y = 12` a `y = 108` en `x`, con las jorobas
/// hacia la derecha (`dir = 1`) o la izquierda (`-1`).
fn devanado(l: &mut Lapiz, x: f32, dir: f32) {
    let mut pts = Vec::new();
    for i in 0..4 {
        let cy = 24.0 + 24.0 * i as f32;
        let (a, b) = if dir > 0.0 {
            (-90.0, 90.0)
        } else {
            (270.0, 90.0)
        };
        pts.extend(arco_pts(x, cy, 21.0, 12.0, a, b));
    }
    pts.dedup();
    l.poli(&pts, TINTA, G, None);
}

/// Primario a la izquierda (patas en x = 0) y secundario a la derecha
/// (x = 146), con el nucleo de hierro en medio.
fn transformador(l: &mut Lapiz) {
    // Parte «Primario»: el devanado de la izquierda con sus dos patas.
    l.parte("Primario");
    devanado(l, 40.0, 1.0);
    raya(l, (0.0, 12.0), (40.0, 12.0));
    raya(l, (0.0, 108.0), (40.0, 108.0));
    // Parte «Secundario»: el de la derecha con las suyas.
    l.parte("Secundario");
    devanado(l, 106.0, -1.0);
    raya(l, (106.0, 12.0), (146.0, 12.0));
    raya(l, (106.0, 108.0), (146.0, 108.0));
    // Parte «Núcleo»: las dos rayas del hierro.
    l.parte("Núcleo");
    l.linea(70.0, 6.0, 70.0, 114.0, TINTA, 2.5);
    l.linea(76.0, 6.0, 76.0, 114.0, TINTA, 2.5);
}

/// Un circulo con su letra entre dos patas (motor, voltimetro...).
fn en_circulo(l: &mut Lapiz, letra: &str, tam: f32) {
    let r = if letra == "M" { 24.0 } else { 22.0 };
    patas(l, 60.0 - r, 60.0 + r);
    // Parte «Cuerpo»: el circulo.
    l.parte("Cuerpo");
    l.circulo(60.0, Y, r, Some(TINTA), Some(BLANCO), G);
    // Parte «Letra»: M de motor, V de voltimetro, A de amperimetro.
    l.parte("Letra");
    texto_en(l, 60.0, Y, letra, tam, TINTA, true);
}

fn lampara(l: &mut Lapiz) {
    patas(l, 40.0, 80.0);
    // Parte «Ampolla»: el circulo con la cruz (norma IEC).
    l.parte("Ampolla");
    l.circulo(60.0, Y, 20.0, Some(TINTA), Some(BLANCO), G);
    let d = 20.0 * std::f32::consts::FRAC_1_SQRT_2;
    raya(l, (60.0 - d, Y - d), (60.0 + d, Y + d));
    raya(l, (60.0 - d, Y + d), (60.0 + d, Y - d));
}

fn fusible(l: &mut Lapiz) {
    // Parte «Cuerpo»: el rectangulo del cartucho.
    l.parte("Cuerpo");
    l.rect(
        34.0,
        Y - 11.0,
        52.0,
        22.0,
        Some(TINTA),
        Some(BLANCO),
        G,
        false,
    );
    // Parte «Filamento»: el hilo que lo cruza de pata a pata (x = 0 a 120).
    l.parte("Filamento");
    raya(l, (0.0, Y), (120.0, Y));
}

/// Los dos contactos de un interruptor, en x = 40 y x = 80.
fn contactos(l: &mut Lapiz) {
    patas(l, 36.0, 84.0);
    // Parte «Contactos»: los dos bornes donde cierra.
    l.parte("Contactos");
    l.circulo(40.0, Y, 4.0, Some(TINTA), Some(BLANCO), G);
    l.circulo(80.0, Y, 4.0, Some(TINTA), Some(BLANCO), G);
}

fn interruptor(l: &mut Lapiz) {
    contactos(l);
    // Parte «Palanca»: la cuchilla abierta.
    l.parte("Palanca");
    raya(l, (43.0, Y - 2.0), (80.0, Y - 22.0));
}

fn pulsador(l: &mut Lapiz) {
    contactos(l);
    // Parte «Puente»: la barra que cierra los contactos al pulsar.
    l.parte("Puente");
    raya(l, (34.0, Y - 12.0), (86.0, Y - 12.0));
    // Parte «Boton»: el vastago y la cabeza que se aprietan.
    l.parte("Botón");
    raya(l, (60.0, Y - 12.0), (60.0, Y - 30.0));
    raya(l, (50.0, Y - 30.0), (70.0, Y - 30.0));
}

// ------------------------------------------------------ semiconductores

#[derive(Clone, Copy, PartialEq)]
enum Diodo {
    Normal,
    Foto,
    Led,
    Zener,
}

/// El anodo a la izquierda (x = 0) y el catodo a la derecha (x = 120).
fn diodo(l: &mut Lapiz, tipo: Diodo) {
    // Parte «Ánodo» y «Cátodo»: las patas, en x = 0 y x = 120.
    l.parte("Ánodo");
    raya(l, (0.0, Y), (45.0, Y));
    l.parte("Cátodo");
    raya(l, (75.0, Y), (120.0, Y));
    // Parte «Cuerpo»: el triangulo que apunta en el sentido de conduccion.
    l.parte("Cuerpo");
    region(
        l,
        &[(45.0, Y - 15.0), (45.0, Y + 15.0), (75.0, Y)],
        TINTA,
        TINTA,
        1.0,
    );
    // Parte «Barra»: la raya del catodo (con las alas en Z en el zener).
    l.parte("Barra");
    if tipo == Diodo::Zener {
        l.poli(
            &[
                (68.0, Y - 20.0),
                (75.0, Y - 15.0),
                (75.0, Y + 15.0),
                (82.0, Y + 20.0),
            ],
            TINTA,
            2.5,
            None,
        );
    } else {
        l.linea(75.0, Y - 15.0, 75.0, Y + 15.0, TINTA, 2.5);
    }
    match tipo {
        Diodo::Foto => {
            // Parte «Luz que entra»: las dos flechas hacia el diodo.
            l.parte("Luz que entra");
            flechita(l, (26.0, Y - 44.0), (42.0, Y - 24.0), 8.0, 1.5);
            flechita(l, (40.0, Y - 48.0), (56.0, Y - 28.0), 8.0, 1.5);
        }
        Diodo::Led => {
            // Parte «Luz que sale»: las dos flechas hacia fuera.
            l.parte("Luz que sale");
            flechita(l, (60.0, Y - 22.0), (76.0, Y - 42.0), 8.0, 1.5);
            flechita(l, (74.0, Y - 18.0), (90.0, Y - 38.0), 8.0, 1.5);
        }
        _ => {}
    }
}

/// El FET (de union): puerta a la izquierda (10, 40), drenador arriba
/// (80, 0) y fuente abajo (80, 80). La flecha de la puerta entra al canal
/// en el de canal N y sale en el de canal P.
fn fet(l: &mut Lapiz, canal_n: bool) {
    // Parte «Cápsula»: el circulo.
    l.parte("Cápsula");
    l.circulo(60.0, 40.0, 26.0, Some(TINTA), Some(BLANCO), G);
    // Parte «Canal»: la barra por donde pasa la corriente.
    l.parte("Canal");
    l.linea(66.0, 22.0, 66.0, 58.0, TINTA, 3.0);
    // Parte «Drenador» (arriba, acaba en (80, 0)) y «Fuente» (abajo, en
    // (80, 80)).
    l.parte("Drenador");
    l.poli(&[(66.0, 28.0), (80.0, 28.0), (80.0, 0.0)], TINTA, G, None);
    l.parte("Fuente");
    l.poli(&[(66.0, 52.0), (80.0, 52.0), (80.0, 80.0)], TINTA, G, None);
    // Parte «Puerta»: la pata de control, acaba en (10, 40), con su flecha.
    l.parte("Puerta");
    raya(l, (10.0, 40.0), (66.0, 40.0));
    if canal_n {
        punta(l, (65.0, 40.0), 0.0, 11.0, 10.0);
    } else {
        punta(l, (44.0, 40.0), 180.0, 11.0, 10.0);
    }
}

/// El transistor bipolar: base a la izquierda (10, 40), colector arriba
/// (74, 0) y emisor abajo (74, 80). La flecha del emisor sale en el NPN y
/// entra en el PNP.
fn bjt(l: &mut Lapiz, npn: bool) {
    // Parte «Cápsula»: el circulo.
    l.parte("Cápsula");
    l.circulo(62.0, 40.0, 28.0, Some(TINTA), Some(BLANCO), G);
    // Parte «Base»: la barra y su pata, acaba en (10, 40).
    l.parte("Base");
    l.linea(50.0, 22.0, 50.0, 58.0, TINTA, 3.5);
    raya(l, (10.0, 40.0), (50.0, 40.0));
    // Parte «Colector»: arriba, acaba en (74, 0).
    l.parte("Colector");
    l.poli(&[(50.0, 32.0), (74.0, 16.0), (74.0, 0.0)], TINTA, G, None);
    // Parte «Emisor»: abajo, acaba en (74, 80), con la flecha del tipo.
    l.parte("Emisor");
    l.poli(&[(50.0, 48.0), (74.0, 64.0), (74.0, 80.0)], TINTA, G, None);
    let ang = 16f32.atan2(24.0).to_degrees();
    if npn {
        punta(l, (72.0, 62.7), ang, 11.0, 10.0);
    } else {
        punta(l, (53.0, 50.0), ang + 180.0, 11.0, 10.0);
    }
}

// ------------------------------------------------------ tierra y antenas

fn tierra(l: &mut Lapiz) {
    // Parte «Terminal»: la pata, acaba arriba en (30, 0).
    l.parte("Terminal");
    raya(l, (30.0, 0.0), (30.0, 26.0));
    // Parte «Placas de tierra»: las tres rayas que menguan.
    l.parte("Placas de tierra");
    l.linea(8.0, 26.0, 52.0, 26.0, TINTA, 2.5);
    l.linea(16.0, 34.0, 44.0, 34.0, TINTA, 2.5);
    l.linea(24.0, 42.0, 36.0, 42.0, TINTA, 2.5);
}

fn antena_1(l: &mut Lapiz) {
    // Parte «Antena»: el triangulo invertido de arriba.
    l.parte("Antena");
    l.poli(
        &[(30.0, 36.0), (0.0, 0.0), (60.0, 0.0), (30.0, 36.0)],
        TINTA,
        G,
        None,
    );
    // Parte «Mástil»: la pata, acaba abajo en (30, 76).
    l.parte("Mástil");
    raya(l, (30.0, 0.0), (30.0, 76.0));
}

fn antena_2(l: &mut Lapiz) {
    // Parte «Antena»: los dos brazos en V.
    l.parte("Antena");
    raya(l, (30.0, 32.0), (4.0, 0.0));
    raya(l, (30.0, 32.0), (56.0, 0.0));
    // Parte «Mástil»: la pata, acaba abajo en (30, 76).
    l.parte("Mástil");
    raya(l, (30.0, 0.0), (30.0, 76.0));
}

// ------------------------------------------------- amplificadores y logica

/// El buffer (y con bolita, el inversor): entrada en (0, 30).
fn buffer(l: &mut Lapiz, negado: bool) {
    // Parte «Entrada»: acaba en (0, 30).
    l.parte("Entrada");
    raya(l, (0.0, 30.0), (30.0, 30.0));
    // Parte «Cuerpo»: el triangulo del amplificador.
    l.parte("Cuerpo");
    region(
        l,
        &[(30.0, 0.0), (82.0, 30.0), (30.0, 60.0)],
        TINTA,
        BLANCO,
        G,
    );
    if negado {
        // Parte «Burbuja de negación»: la bolita que invierte la salida.
        l.parte("Burbuja de negación");
        burbuja(l, 88.0, 30.0);
        // Parte «Salida»: acaba en (120, 30).
        l.parte("Salida");
        raya(l, (94.0, 30.0), (120.0, 30.0));
    } else {
        // Parte «Salida»: acaba en (110, 30).
        l.parte("Salida");
        raya(l, (82.0, 30.0), (110.0, 30.0));
    }
}

/// El operacional: entradas − (0, 25) y + (0, 65), salida (140, 45).
fn amp_op(l: &mut Lapiz) {
    // Parte «Entrada inversora» (−) y «Entrada no inversora» (+), con su
    // signo dentro del triangulo.
    l.parte("Entrada inversora");
    raya(l, (0.0, 25.0), (30.0, 25.0));
    l.parte("Entrada no inversora");
    raya(l, (0.0, 65.0), (30.0, 65.0));
    // Parte «Cuerpo»: el triangulo.
    l.parte("Cuerpo");
    region(
        l,
        &[(30.0, 0.0), (110.0, 45.0), (30.0, 90.0)],
        TINTA,
        BLANCO,
        G,
    );
    // Parte «Signos»: el − y el + junto a cada entrada.
    l.parte("Signos");
    l.linea(37.0, 25.0, 47.0, 25.0, TINTA, 1.8);
    l.linea(37.0, 65.0, 47.0, 65.0, TINTA, 1.8);
    l.linea(42.0, 60.0, 42.0, 70.0, TINTA, 1.8);
    // Parte «Salida»: acaba en (140, 45).
    l.parte("Salida");
    raya(l, (110.0, 45.0), (140.0, 45.0));
}

#[derive(Clone, Copy, PartialEq)]
enum Puerta {
    And,
    Or,
    Xor,
}

/// **Una compuerta de dos entradas** (en y = 15 y y = 45, desde x = 0) y
/// una salida en y = 30. `salida_negada` pone la bolita a la salida (NAND,
/// NOR, XNOR) y `entradas_negadas` en las entradas (AND y OR negadas).
fn compuerta(l: &mut Lapiz, tipo: Puerta, salida_negada: bool, entradas_negadas: bool) {
    let (cuerpo, punta_x, entrada_x) = match tipo {
        Puerta::And => {
            // La D: lomo recto en x = 30 y frente en semicirculo.
            let mut p = vec![(30.0, 0.0)];
            p.extend(arco_pts(55.0, 30.0, 30.0, 30.0, -90.0, 90.0));
            p.push((30.0, 60.0));
            (p, 85.0, 30.0)
        }
        Puerta::Or | Puerta::Xor => {
            // La ojiva: dos curvas que se juntan en la punta y el lomo
            // concavo.
            let mut p = bezier((26.0, 0.0), (64.0, 0.0), (90.0, 30.0));
            p.extend(
                bezier((90.0, 30.0), (64.0, 60.0), (26.0, 60.0))
                    .into_iter()
                    .skip(1),
            );
            let mut atras = bezier((26.0, 60.0), (42.0, 30.0), (26.0, 0.0));
            atras.pop();
            p.extend(atras.into_iter().skip(1));
            // El lomo pasa por x = 32 a la altura de las entradas; en la XOR
            // las entradas llegan a la raya de mas atras (x = 24).
            (p, 90.0, if tipo == Puerta::Xor { 24.0 } else { 32.0 })
        }
    };
    for (nombre, y) in [("Entrada A", 15.0), ("Entrada B", 45.0)] {
        // Parte «Entrada A» / «Entrada B»: acaban en x = 0 (y = 15 y 45);
        // en las negadas llevan su bolita junto al cuerpo.
        l.parte(nombre);
        if entradas_negadas {
            raya(l, (0.0, y), (entrada_x - 12.0, y));
            burbuja(l, entrada_x - 6.0, y);
        } else {
            raya(l, (0.0, y), (entrada_x, y));
        }
    }
    // Parte «Cuerpo»: la forma de la compuerta (D para AND, ojiva para OR).
    l.parte("Cuerpo");
    region(l, &cuerpo, TINTA, BLANCO, G);
    if tipo == Puerta::Xor {
        // Parte «Lomo exclusivo»: la segunda curva de la XOR.
        l.parte("Lomo exclusivo");
        let extra = bezier((18.0, 0.0), (34.0, 30.0), (18.0, 60.0));
        l.poli(&extra, TINTA, G, None);
    }
    if salida_negada {
        // Parte «Burbuja de negación»: la bolita de NAND, NOR y XNOR.
        l.parte("Burbuja de negación");
        burbuja(l, punta_x + 6.0, 30.0);
        // Parte «Salida»: acaba en (punta + 40, 30).
        l.parte("Salida");
        raya(l, (punta_x + 12.0, 30.0), (punta_x + 40.0, 30.0));
    } else {
        // Parte «Salida»: acaba en (punta + 30, 30).
        l.parte("Salida");
        raya(l, (punta_x, 30.0), (punta_x + 30.0, 30.0));
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::biblioteca::{self, nombre_de_parte};
    use crate::vector::Punto2;
    use std::collections::HashSet;

    fn mias() -> Vec<FiguraGuardada> {
        let mut v = Vec::new();
        todas(&mut v);
        v
    }

    fn partes(piezas: &[crate::elemento::Elemento]) -> HashSet<String> {
        piezas
            .iter()
            .flat_map(|e| e.grupos.iter())
            .filter_map(|g| nombre_de_parte(g).map(str::to_string))
            .collect()
    }

    #[test]
    fn estan_todos_los_simbolos_pedidos() {
        let v = mias();
        assert!(v.len() >= 45, "{}", v.len());
        for clave in [
            "elec-fuente-corriente-ca",
            "elec-diac",
            "elec-resistencia-iec",
            "elec-potenciometro",
            "elec-condensador-polarizado",
            "elec-bateria",
            "elec-jack",
            "elec-cristal",
            "elec-zener",
            "elec-fet-p",
            "elec-pnp",
            "elec-antena-2",
            "elec-amp-op",
            "elec-and-negada",
            "elec-or-negada",
            "elec-xnor",
        ] {
            assert!(
                v.iter().any(|f| f.id == format!("serie-{clave}")),
                "falta {clave}"
            );
        }
    }

    #[test]
    fn cada_pieza_va_en_una_parte_con_nombre() {
        for f in mias() {
            for e in &f.elementos {
                let n = e.grupos.iter().filter_map(|g| nombre_de_parte(g)).count();
                assert_eq!(n, 1, "{}: pieza sin parte (o con dos)", f.nombre);
            }
        }
    }

    #[test]
    fn la_nand_estampada_conserva_sus_partes() {
        let nand = mias()
            .into_iter()
            .find(|f| f.id == "serie-elec-nand")
            .unwrap();
        let e = biblioteca::estampar(&nand, Punto2::nuevo(100.0, 100.0), 1, "g1");
        let esperadas: HashSet<String> = [
            "Entrada A",
            "Entrada B",
            "Cuerpo",
            "Burbuja de negación",
            "Salida",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        assert_eq!(partes(&e), esperadas);
        // Cada pieza: su parte dentro y el grupo de la figura por fuera.
        for p in &e {
            assert_eq!(p.grupos.len(), 2);
            assert!(p.grupos[0].starts_with("parte:") && p.grupos[0].ends_with("@g1"));
            assert_eq!(p.grupos[1], "g1");
        }
    }

    #[test]
    fn las_patas_de_dos_terminales_acaban_en_x_0_y_120() {
        let v = mias();
        let r = v.iter().find(|f| f.id == "serie-elec-resistencia").unwrap();
        let (x0, _, x1, _) = biblioteca::caja_de(&r.elementos).unwrap();
        // 120 de pata a pata, mas el grueso del trazo.
        assert!((x1 - x0 - 120.0).abs() <= G + 0.01, "{x0} {x1}");
    }
}
