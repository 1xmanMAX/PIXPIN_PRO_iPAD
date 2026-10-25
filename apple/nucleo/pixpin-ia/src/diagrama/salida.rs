//! **El diagrama medido, a elementos del lienzo**: las figuras del motor
//! (`Linea`, `Flecha`, `Arco`, `Elipse`, `Rectangulo`, `Punto`, `Texto`) y
//! las regiones de color (`pixpin-region`), por `lienzo::Fabrica` como todo
//! lo que dibuja el asistente.

use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pixpin_motor2d::formas::TipoPunta;
use pixpin_motor2d::geometria_exacta as ge;
use pixpin_motor2d::vector::Punto2;

use super::{Diagrama, Estilo};
use crate::hoja::Alineacion;
use crate::lienzo::Fabrica;

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn color(c: [u8; 3]) -> ColorRgba {
    ColorRgba::opaco(c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0)
}

/// Lo casi negro, negro del lienzo (el antialias lo deja gris oscuro).
fn limpio(c: [u8; 3]) -> [u8; 3] {
    let max = *c.iter().max().unwrap_or(&0) as i32;
    let min = *c.iter().min().unwrap_or(&0) as i32;
    if max < 90 && max - min < 30 { [30, 30, 30] } else { c }
}

fn estilo(e: Estilo) -> EstiloTrazo {
    match e {
        Estilo::Solido => EstiloTrazo::Solido,
        Estilo::Discontinuo => EstiloTrazo::Discontinuo,
        Estilo::Punteado => EstiloTrazo::Punteado,
    }
}

/// El grosor en el lienzo: el medido a escala, de medio en medio pixel.
fn grosor(g: f64, escala: f64) -> f32 {
    (((g * escala).max(1.0) * 2.0).round() / 2.0) as f32
}

/// **Los elementos del diagrama** en la fabrica: el pixel `(x, y)` del
/// recorte cae en `origen + (x, y) * escala`. Debajo las regiones, luego
/// los trazos, los puntos y encima los rotulos.
pub(crate) fn elementos(d: &Diagrama, origen: (f64, f64), escala: f64, f: &mut Fabrica, familia: &str) {
    let a = |p: (f64, f64)| (origen.0 + p.0 * escala, origen.1 + p.1 * escala);
    let ap = |p: (f64, f64)| {
        let q = a(p);
        Punto2::nuevo(q.0 as f32, q.1 as f32)
    };
    for r in &d.regiones {
        let contorno: Vec<(f64, f64)> = r.contorno.iter().map(|&p| a(p)).collect();
        let huecos: Vec<Vec<(f64, f64)>> = r.huecos.iter().map(|h| h.iter().map(|&p| a(p)).collect()).collect();
        let c = hex(r.color);
        f.region(&contorno, &huecos, &c, &c);
        if let Some(v) = f.salida.last_mut() {
            v["roughness"] = serde_json::json!(0);
            v["strokeWidth"] = serde_json::json!(1);
        }
    }
    let mut piezas: Vec<Elemento> = Vec::new();
    let base = |c: [u8; 3], g: f64| Elemento { trazo: color(limpio(c)), grosor: grosor(g, escala), rugosidad: 0.0, ..Default::default() };
    // Con lo que estila un trazo del diagrama (color, grosor, sin temblor).
    let estilar = |mut e: Elemento, c: [u8; 3], g: f64| {
        let b = base(c, g);
        e.trazo = b.trazo;
        e.grosor = b.grosor;
        e.rugosidad = 0.0;
        e
    };
    // Las cajas y los ovalos (con su relleno, debajo de lo demas).
    for r in &d.recuadros {
        let (x0, y0) = a((r.x0, r.y0));
        let (x1, y1) = a((r.x1, r.y1));
        let mut e = base(r.color, r.grosor);
        e.figura = Figura::Rectangulo;
        e.x = x0 as f32;
        e.y = y0 as f32;
        e.ancho = (x1 - x0) as f32;
        e.alto = (y1 - y0) as f32;
        if let Some(c) = r.relleno {
            e.relleno = Some(color(c));
            e.estilo_relleno = pixpin_motor2d::relleno::EstiloRelleno::Solido;
        }
        piezas.push(e);
    }
    for o in &d.ovalos {
        let (cx, cy) = a(o.c);
        let mut e = base(o.color, o.grosor);
        e.figura = if o.rombo { Figura::Rombo } else { Figura::Elipse };
        e.x = (cx - o.rx * escala) as f32;
        e.y = (cy - o.ry * escala) as f32;
        e.ancho = (2.0 * o.rx * escala) as f32;
        e.alto = (2.0 * o.ry * escala) as f32;
        if let Some(c) = o.relleno {
            e.relleno = Some(color(c));
            e.estilo_relleno = pixpin_motor2d::relleno::EstiloRelleno::Solido;
        }
        piezas.push(e);
    }
    // Las marcas de angulo: el sector del motor comun (`geometria_exacta`),
    // relleno de su color y con su arquito.
    for mk in &d.marcas {
        let v = ap(mk.v);
        let r = (mk.r * escala) as f32;
        let p1 = ge::en_la_circunferencia(v, r, mk.inicio as f32);
        let p2 = ge::en_la_circunferencia(v, r, (mk.inicio + mk.barrido) as f32);
        let mut e = estilar(ge::marca_de_angulo(v, p1, p2, r, 1), oscuro(mk.color), 1.0);
        e.relleno = Some(color(mk.color));
        e.estilo_relleno = pixpin_motor2d::relleno::EstiloRelleno::Solido;
        piezas.push(e);
    }
    // Circunferencias y arcos: las constructoras del motor comun.
    for arco in &d.arcos {
        let c = ap(arco.c);
        let r = (arco.r * escala) as f32;
        let e = if arco.entera() { ge::circunferencia(c, r) } else { ge::arco(c, r, arco.inicio as f32, arco.barrido as f32) };
        let mut e = estilar(e, arco.color, arco.grosor);
        e.estilo = estilo(arco.estilo);
        piezas.push(e);
    }
    // La punta como en el papel: abierta (dos rayas) o rellena.
    let punta = |p: (f64, f64)| if d.puntas_abiertas.iter().any(|&q| super::ajuste::dist(p, q) <= 3.0) { TipoPunta::Flecha } else { TipoPunta::Triangulo };
    for s in &d.segmentos {
        let puntos = vec![ap(s.a), ap(s.b)];
        let mut e = base(s.color, s.grosor);
        e.x = puntos[0].x;
        e.y = puntos[0].y;
        e.ancho = (puntos[0].x - puntos[1].x).abs();
        e.alto = (puntos[0].y - puntos[1].y).abs();
        e.estilo = estilo(s.estilo);
        e.figura = if s.flecha_a || s.flecha_b {
            Figura::Flecha {
                puntos,
                punta_inicio: if s.flecha_a { punta(s.a) } else { TipoPunta::Ninguna },
                punta_fin: if s.flecha_b { punta(s.b) } else { TipoPunta::Ninguna },
                codos: false,
            }
        } else {
            Figura::Linea { puntos }
        };
        piezas.push(e);
    }
    // Las letras que nadie leyo NO se pintan a lapiz (15-oct, el usuario:
    // «nada de letras sueltas inventadas»: salian garabatos junto a la O
    // del centro y la X). Una letra se pone como texto o no se pone.
    for p in &d.puntos {
        let (x, y) = a(p.c);
        let mut e = base(p.color, (p.r / 1.6).max(1.0));
        e.grosor = ((p.r * escala / 1.6).max(1.0)) as f32;
        e.x = x as f32;
        e.y = y as f32;
        e.figura = Figura::Punto { letra: String::new(), angulo: 0.0, radio: 14.0 };
        piezas.push(e);
    }
    let k = pixpin_motor2d::texto::interlineado_de(familia).unwrap_or(pixpin_motor2d::texto::INTERLINEADO) as f64;
    for r in &d.rotulos {
        let c = crate::hoja::Caja::nueva(origen.0 + r.caja.x * escala, origen.1 + r.caja.y * escala, r.caja.w * escala, r.caja.h * escala);
        // Una letra suelta mide su alto de mayuscula (~0,7 de la letra); una
        // palabra del OCR, el renglon.
        // (Varias letras sueltas leidas juntas miden como un renglon.)
        let tam = if r.letra_suelta && r.texto.chars().count() <= 2 { c.h / 0.72 } else { c.h * 1.05 };
        let tam = ((tam * 2.0).round() / 2.0).max(6.0);
        piezas.push(crate::hoja::montaje::un_texto(&r.texto, c.x, c.cy() - tam * k / 2.0, tam, familia, r.negrita, Alineacion::Izquierda, r.color.map(limpio)));
    }
    f.de_motor(&piezas);
    for v in f.salida.iter_mut() {
        if v["type"] != "text" {
            v["roughness"] = serde_json::json!(0);
        }
    }
}

/// **Los elementos del diagrama**, en su propio grupo: `origen` y `escala`
/// llevan el recorte al lienzo.
pub fn a_elementos(d: &Diagrama, origen: (f64, f64), escala: f64, semilla: u64, familia: &str) -> Vec<serde_json::Value> {
    let mut f = Fabrica::nueva((0.0, 0.0), semilla);
    f.grupo = Some(f.nombre_de_grupo("diagrama"));
    elementos(d, origen, escala, &mut f, familia);
    f.salida
}


/// El borde de una marca: su color, mas oscuro.
fn oscuro(c: [u8; 3]) -> [u8; 3] {
    [(c[0] as f32 * 0.7) as u8, (c[1] as f32 * 0.7) as u8, (c[2] as f32 * 0.7) as u8]
}
