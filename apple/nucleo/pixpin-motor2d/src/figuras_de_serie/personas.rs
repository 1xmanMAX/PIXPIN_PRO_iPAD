//! **Personas** (24-oct-2026): gente en planta (vista desde arriba) para
//! amueblar un plano, y gente en alzado, de pie, caminando y sentada, para
//! dar la escala en secciones y alzados.
//!
//! Las de alzado son siluetas **de linea**, a mano alzada: cada miembro es
//! un trazo curvo que se estrecha (muslo, pierna, tobillo), relleno de
//! blanco para que lo de delante tape lo de detras, como en un dibujo a
//! tinta. Se dibujan de atras hacia delante (brazo y pierna lejanos,
//! torso, cabeza, brazo cercano) y con la proporcion clasica: la cabeza es
//! 1/7,5 de la altura.
//!
//! Todo va a 1 px = 1 cm, como el resto de las figuras de planta: un
//! hombre mide 172, una mujer 165 y un nino de seis o siete anos unos 118.
//!
//! Cada figura va **por partes con nombre** ([`Lapiz::parte`]): «Cabeza»,
//! «Pelo», «Torso», «Brazo de delante», «Piernas», «Ropa», «Objeto
//! (telefono)»... para poder mover o retocar una sola.

use super::tecnico::{Dibujo, poner};
use super::{Lapiz, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::ColorRgba;

pub(super) const CATEGORIA: &str = "Personas";
pub(super) const EN_PLANTA: &str = "Personas en planta";

/// La tinta de las siluetas: gris carbon, fina.
const LINEA: ColorRgba = hex(0x3a3f4b);
/// Los detalles (pliegues, costuras, pelo): mas claros y finos.
const DETALLE: ColorRgba = hex(0x7a8292);
const PAPEL: ColorRgba = hex(0xffffff);
/// El grosor del contorno y el de los detalles.
const G: f32 = 1.15;
const GD: f32 = 0.8;

// ---------------------------------------------------------------------------
// El cuerpo: un lapiz con su sitio, su escala y su espejo
// ---------------------------------------------------------------------------

/// Curva de Catmull-Rom por esos puntos, con `sub` tramos entre cada dos.
fn spline(p: &[(f32, f32)], cerrada: bool, sub: usize) -> Vec<(f32, f32)> {
    let n = p.len();
    if n < 3 {
        return p.to_vec();
    }
    let punto = |i: isize| -> (f32, f32) {
        if cerrada {
            p[i.rem_euclid(n as isize) as usize]
        } else {
            p[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let tramos = if cerrada { n } else { n - 1 };
    let mut v = Vec::with_capacity(tramos * sub + 1);
    for i in 0..tramos as isize {
        let (p0, p1, p2, p3) = (punto(i - 1), punto(i), punto(i + 1), punto(i + 2));
        for k in 0..sub {
            let t = k as f32 / sub as f32;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * (2.0 * b
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            v.push((f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    if !cerrada {
        v.push(p[n - 1]);
    }
    v
}

/// Un lapiz que dibuja una persona en `(ox, oy)`, a escala `k` y, si
/// `espejo`, mirando al otro lado. Las poses se escriben con la persona
/// mirando a la derecha, los pies en `y = 170` y el centro en `x = 0`.
pub(super) struct Cuerpo<'a> {
    pub l: &'a mut Lapiz,
    pub ox: f32,
    pub oy: f32,
    pub k: f32,
    pub espejo: bool,
    /// Si la pose dobla el brazo de delante (el derecho de frente): su
    /// codo y su muneca, para coger un telefono o un libro.
    pub brazo: Option<((f32, f32), (f32, f32))>,
}

impl<'a> Cuerpo<'a> {
    pub fn en(l: &'a mut Lapiz) -> Self {
        Self {
            l,
            ox: 0.0,
            oy: 0.0,
            k: 1.0,
            espejo: false,
            brazo: None,
        }
    }

    fn p(&self, (x, y): (f32, f32)) -> (f32, f32) {
        let x = if self.espejo { -x } else { x };
        (self.ox + x * self.k, self.oy + y * self.k)
    }

    fn todos(&self, pts: &[(f32, f32)]) -> Vec<(f32, f32)> {
        pts.iter().map(|q| self.p(*q)).collect()
    }

    pub fn parte(&mut self, nombre: &str) {
        self.l.parte(nombre);
    }

    /// Una forma cerrada y suave, rellena de papel (tapa lo de detras).
    pub fn mancha(&mut self, pts: &[(f32, f32)]) {
        self.mancha_con(pts, PAPEL);
    }

    /// Como [`Cuerpo::mancha`] con otro relleno. Con cuatro puntos es un
    /// cuadrilatero de lados rectos (asiento, rueda, toalla), sin suavizar.
    pub fn mancha_con(&mut self, pts: &[(f32, f32)], relleno: ColorRgba) {
        let suave = if pts.len() == 4 {
            pts.to_vec()
        } else {
            spline(pts, true, 6)
        };
        let mut v = self.todos(&suave);
        v.push(v[0]);
        self.l.poli(&v, LINEA, G * self.k.sqrt(), Some(relleno));
    }

    /// Una linea suave de detalle (pliegue, costura, flequillo).
    pub fn curva(&mut self, pts: &[(f32, f32)]) {
        let v = self.todos(&spline(pts, false, 6));
        self.l.poli(&v, DETALLE, GD * self.k.sqrt(), None);
    }

    /// Una linea suave con la tinta del contorno.
    pub fn trazo(&mut self, pts: &[(f32, f32)], g: f32) {
        let v = self.todos(&spline(pts, false, 6));
        self.l.poli(&v, LINEA, g * self.k.sqrt(), None);
    }

    /// Un ovalo relleno de papel.
    pub fn ovalo(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) {
        let pts: Vec<(f32, f32)> = (0..16)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 16.0;
                (cx + rx * a.cos(), cy + ry * a.sin())
            })
            .collect();
        self.mancha(&pts);
    }

    /// **Un miembro**: un trazo que pasa por `pts` (hombro, codo, muneca...)
    /// con el ancho de cada punto, cerrado con puntas redondas. Es la pieza
    /// de la que salen brazos, piernas, cuello y torso.
    pub fn miembro(&mut self, pts: &[(f32, f32)], anchos: &[f32]) {
        let sub = 6;
        let eje = spline(pts, false, sub);
        let n = eje.len();
        let ancho = |i: usize| -> f32 {
            let t = i as f32 / sub as f32;
            let a = (t.floor() as usize).min(anchos.len() - 1);
            let b = (a + 1).min(anchos.len() - 1);
            let f = t - a as f32;
            (anchos[a] * (1.0 - f) + anchos[b] * f) / 2.0
        };
        let tangente = |i: usize| -> (f32, f32) {
            let (a, b) = (eje[i.saturating_sub(1)], eje[(i + 1).min(n - 1)]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let m = (dx * dx + dy * dy).sqrt().max(1e-4);
            (dx / m, dy / m)
        };
        let mut izq = Vec::with_capacity(n);
        let mut der = Vec::with_capacity(n);
        for (i, q) in eje.iter().enumerate() {
            let (tx, ty) = tangente(i);
            let (nx, ny) = (-ty, tx);
            let w = ancho(i);
            izq.push((q.0 + nx * w, q.1 + ny * w));
            der.push((q.0 - nx * w, q.1 - ny * w));
        }
        let tapa = |q: (f32, f32), t: (f32, f32), w: f32, al_final: bool| {
            let (nx, ny) = (-t.1, t.0);
            (1..8)
                .map(|j| {
                    let a = std::f32::consts::PI * j as f32 / 8.0;
                    let (c, s) = (a.cos(), a.sin());
                    if al_final {
                        (q.0 + (nx * c + t.0 * s) * w, q.1 + (ny * c + t.1 * s) * w)
                    } else {
                        (q.0 - (nx * c + t.0 * s) * w, q.1 - (ny * c + t.1 * s) * w)
                    }
                })
                .collect::<Vec<_>>()
        };
        let mut contorno = izq.clone();
        contorno.extend(tapa(eje[n - 1], tangente(n - 1), ancho(n - 1), true));
        contorno.extend(der.iter().rev());
        contorno.extend(tapa(eje[0], tangente(0), ancho(0), false));
        let mut v = self.todos(&contorno);
        v.push(v[0]);
        self.l.poli(&v, LINEA, G * self.k.sqrt(), Some(PAPEL));
    }

    /// Un rectangulo girado (telefono, libro, bolso) por su centro.
    pub fn placa(&mut self, cx: f32, cy: f32, w: f32, h: f32, grados: f32) {
        let (s, c) = grados.to_radians().sin_cos();
        let pts: Vec<(f32, f32)> = [(-w, -h), (w, -h), (w, h), (-w, h), (-w, -h)]
            .iter()
            .map(|(x, y)| {
                let (x, y) = (x / 2.0, y / 2.0);
                (cx + x * c - y * s, cy + x * s + y * c)
            })
            .collect();
        let v = self.todos(&pts);
        self.l.poli(&v, LINEA, G * self.k.sqrt(), Some(PAPEL));
    }
}

// ---------------------------------------------------------------------------
// Las piezas del cuerpo
// ---------------------------------------------------------------------------

/// Como lleva el pelo.
#[derive(Clone, Copy, PartialEq)]
enum Pelo {
    Corto,
    Largo,
    Mono,
}

/// Desde donde se ve.
#[derive(Clone, Copy, PartialEq)]
enum Vista {
    Frente,
    Espalda,
    Lado,
    TresCuartos,
}

/// **La melena** que cae hasta los hombros. De frente, de lado y en 3/4 va
/// detras del cuello y del torso: la pose la pinta antes que ellos
/// ([`melena_detras`]); de espaldas la pinta la cabeza, por encima.
fn melena(c: &mut Cuerpo, cx: f32, cy: f32, e: f32, vista: Vista) {
    let q = |x: f32, y: f32| (cx + x * e, cy + y * e);
    c.parte("Pelo");
    let melena: Vec<(f32, f32)> = match vista {
        Vista::Espalda => vec![
            q(0.0, -13.5),
            q(10.5, -8.0),
            q(12.0, 4.0),
            q(13.0, 18.0),
            q(11.5, 27.0),
            q(0.0, 28.5),
            q(-11.5, 27.0),
            q(-13.0, 18.0),
            q(-12.0, 4.0),
            q(-10.5, -8.0),
        ],
        Vista::Frente => vec![
            q(0.0, -13.5),
            q(10.5, -8.0),
            q(12.0, 4.0),
            q(13.0, 18.0),
            q(11.5, 27.0),
            q(6.0, 26.0),
            q(0.0, 14.0),
            q(-6.0, 26.0),
            q(-11.5, 27.0),
            q(-13.0, 18.0),
            q(-12.0, 4.0),
            q(-10.5, -8.0),
        ],
        Vista::Lado | Vista::TresCuartos => vec![
            q(0.0, -13.5),
            q(6.0, -10.0),
            q(4.0, 0.0),
            q(-2.0, 12.0),
            q(-4.0, 26.0),
            q(-10.0, 27.0),
            q(-12.5, 14.0),
            q(-11.0, -4.0),
            q(-7.0, -11.5),
        ],
    };
    c.mancha(&melena);
}

/// La melena, si la hay y no es de espaldas, antes del cuello y el torso.
fn melena_detras(c: &mut Cuerpo, cx: f32, cy: f32, vista: Vista, pelo: Pelo) {
    if pelo == Pelo::Largo && vista != Vista::Espalda {
        melena(c, cx, cy, 1.0, vista);
    }
}

/// **La cabeza** con su pelo, centrada en `(cx, cy)` (22 de alto, a
/// escala `e`). De lado y en 3/4 mira a la derecha y lleva nariz y menton.
fn cabeza(c: &mut Cuerpo, cx: f32, cy: f32, e: f32, vista: Vista, pelo: Pelo) {
    let q = |x: f32, y: f32| (cx + x * e, cy + y * e);
    // De espaldas la melena va delante de la espalda: se pinta aqui.
    if vista == Vista::Espalda && pelo == Pelo::Largo {
        melena(c, cx, cy, e, vista);
    }
    // Parte «Cabeza»: el ovalo de la cara (o el perfil, con nariz y
    // barbilla).
    c.parte("Cabeza");
    let cara: Vec<(f32, f32)> = match vista {
        Vista::Frente | Vista::Espalda => vec![
            q(0.0, -11.5),
            q(6.5, -9.0),
            q(8.5, -2.0),
            q(7.5, 5.0),
            q(4.5, 9.5),
            q(0.0, 11.0),
            q(-4.5, 9.5),
            q(-7.5, 5.0),
            q(-8.5, -2.0),
            q(-6.5, -9.0),
        ],
        Vista::Lado => vec![
            q(-1.0, -11.5),
            q(5.0, -9.5),
            q(8.0, -4.0),
            q(8.2, -1.0),
            q(10.4, 2.6),
            q(8.4, 3.6),
            q(8.6, 6.0),
            q(7.6, 9.4),
            q(3.5, 11.0),
            q(-2.0, 9.0),
            q(-6.5, 5.0),
            q(-8.5, -2.0),
            q(-7.0, -8.5),
        ],
        Vista::TresCuartos => vec![
            q(0.0, -11.5),
            q(6.5, -9.0),
            q(8.6, -3.0),
            q(9.6, 1.5),
            q(8.4, 3.0),
            q(7.6, 6.5),
            q(4.5, 10.2),
            q(0.5, 11.0),
            q(-4.5, 8.5),
            q(-7.5, 3.0),
            q(-8.0, -3.0),
            q(-6.5, -9.0),
        ],
    };
    c.mancha(&cara);
    // La oreja, de lado y en 3/4.
    match vista {
        Vista::Lado => c.curva(&[q(-1.5, -1.5), q(-3.5, 0.0), q(-3.0, 3.5), q(-1.0, 4.0)]),
        Vista::TresCuartos => c.curva(&[q(-4.5, -1.0), q(-6.5, 0.5), q(-6.0, 4.0), q(-4.0, 4.5)]),
        _ => {}
    }
    // Parte «Pelo» de encima: el casquete, el flequillo o el mono.
    c.parte("Pelo");
    match (vista, pelo) {
        (Vista::Espalda, _) => {
            let casco = [
                q(0.0, -12.5),
                q(8.0, -9.0),
                q(9.2, 0.0),
                q(8.0, if pelo == Pelo::Corto { 5.0 } else { 8.0 }),
                q(0.0, if pelo == Pelo::Corto { 6.5 } else { 9.5 }),
                q(-8.0, if pelo == Pelo::Corto { 5.0 } else { 8.0 }),
                q(-9.2, 0.0),
                q(-8.0, -9.0),
            ];
            c.mancha(&casco);
            if pelo != Pelo::Corto {
                c.curva(&[q(0.0, -11.0), q(-1.0, 0.0), q(0.0, 8.0)]);
            }
        }
        (Vista::Frente, Pelo::Corto) => c.mancha(&[
            q(0.0, -12.5),
            q(7.5, -10.0),
            q(9.3, -3.0),
            q(8.4, 0.5),
            q(7.0, -4.0),
            q(2.0, -6.5),
            q(-4.0, -5.0),
            q(-7.5, -3.0),
            q(-8.6, 0.5),
            q(-9.3, -3.0),
            q(-7.5, -10.0),
        ]),
        (Vista::Frente, _) => {
            c.mancha(&[
                q(0.0, -12.5),
                q(7.5, -10.0),
                q(9.5, -2.0),
                q(9.2, 6.0),
                q(7.6, 0.0),
                q(4.0, -6.0),
                q(-1.0, -7.0),
                q(-6.0, -4.0),
                q(-7.8, 2.0),
                q(-9.2, 6.0),
                q(-9.5, -2.0),
                q(-7.5, -10.0),
            ]);
            if pelo == Pelo::Mono {
                c.ovalo(cx, cy - 13.5 * e, 5.0 * e, 3.5 * e);
            }
        }
        (Vista::Lado | Vista::TresCuartos, Pelo::Corto) => c.mancha(&[
            q(-1.5, -12.5),
            q(5.5, -10.5),
            q(8.5, -5.5),
            q(6.0, -6.0),
            q(1.5, -4.5),
            q(-2.0, -2.0),
            q(-4.5, 1.0),
            q(-7.0, 3.5),
            q(-9.0, -2.0),
            q(-7.5, -9.0),
        ]),
        (Vista::Lado | Vista::TresCuartos, _) => {
            c.mancha(&[
                q(-1.5, -12.5),
                q(5.5, -10.5),
                q(8.6, -5.0),
                q(5.5, -6.5),
                q(1.0, -5.0),
                q(-2.5, -1.0),
                q(-4.0, 4.0),
                q(-6.5, 8.0),
                q(-9.0, 1.0),
                q(-8.0, -9.0),
            ]);
            if pelo == Pelo::Mono {
                c.ovalo(cx - 7.5 * e, cy - 7.5 * e, 4.0 * e, 4.0 * e);
            }
        }
    }
}

/// **El brazo**: hombro, codo y muneca, con la mano al final.
fn brazo(c: &mut Cuerpo, hombro: (f32, f32), codo: (f32, f32), muneca: (f32, f32), w: f32) {
    c.miembro(&[hombro, codo, muneca], &[10.0 * w, 7.6 * w, 5.6 * w]);
    // La mano: un ovalo alargado en la direccion del antebrazo.
    let (dx, dy) = (muneca.0 - codo.0, muneca.1 - codo.1);
    let m = (dx * dx + dy * dy).sqrt().max(1e-3);
    let (ux, uy) = (dx / m, dy / m);
    c.miembro(
        &[
            (muneca.0 + ux * 1.0, muneca.1 + uy * 1.0),
            (muneca.0 + ux * 6.5, muneca.1 + uy * 6.5),
        ],
        &[5.6 * w, 4.6 * w],
    );
}

/// **La pierna**: cadera, rodilla y tobillo.
fn pierna(c: &mut Cuerpo, cadera: (f32, f32), rodilla: (f32, f32), tobillo: (f32, f32), w: f32) {
    c.miembro(&[cadera, rodilla, tobillo], &[16.0 * w, 10.5 * w, 7.0 * w]);
}

/// **El pie** (o el zapato) del talon a la punta.
fn pie(c: &mut Cuerpo, talon: (f32, f32), punta: (f32, f32), w: f32) {
    c.miembro(&[talon, punta], &[6.5 * w, 5.0 * w]);
}

/// **El torso**: del cuello a la entrepierna por la columna, con el ancho
/// de cada altura (cuello, hombros, pecho, cintura, cadera...).
fn torso_inclinado(c: &mut Cuerpo, columna: &[(f32, f32)], anchos: &[f32]) {
    c.miembro(columna, anchos);
}

/// **El torso de pie** (o sentado, bajado `dy`) visto desde `vista`: un
/// contorno suave por rodajas horizontales —base del cuello, caida del
/// hombro, hombro, axila, cintura y cadera— que acaba en la entrepierna.
/// Cada rodaja es `(y, centro, medio ancho detras/izquierda, medio ancho
/// delante/derecha)`. Las piernas van antes y el torso tapa su arranque.
fn torso_de(c: &mut Cuerpo, vista: Vista, mujer: bool, dy: f32) {
    let rodajas: &[(f32, f32, f32, f32)] = match (vista, mujer) {
        (Vista::Frente | Vista::Espalda, false) => &[
            (27.0, 0.0, 6.0, 6.0),
            (30.0, 0.0, 15.0, 15.0),
            (35.0, 0.0, 20.5, 20.5),
            (47.0, 0.0, 18.5, 18.5),
            (70.0, 0.0, 15.5, 15.5),
            (84.0, 0.0, 17.0, 17.0),
        ],
        (Vista::Frente | Vista::Espalda, true) => &[
            (27.0, 0.0, 5.5, 5.5),
            (30.0, 0.0, 13.0, 13.0),
            (35.0, 0.0, 17.0, 17.0),
            (47.0, 0.0, 16.0, 16.0),
            (66.0, 0.0, 12.5, 12.5),
            (84.0, 0.0, 17.5, 17.5),
        ],
        (Vista::Lado, false) => &[
            (27.0, 1.0, 5.0, 5.0),
            (31.0, 0.5, 9.0, 9.5),
            (44.0, 1.5, 11.0, 13.0),
            (62.0, 0.5, 10.0, 10.0),
            (76.0, 0.0, 11.0, 10.5),
            (86.0, 0.0, 11.5, 10.5),
        ],
        (Vista::Lado, true) => &[
            (27.0, 1.0, 4.5, 4.5),
            (31.0, 0.5, 8.0, 8.5),
            (44.0, 1.5, 9.0, 13.0),
            (62.0, 0.5, 8.5, 8.5),
            (76.0, 0.0, 11.5, 9.5),
            (86.0, 0.0, 11.5, 9.5),
        ],
        (Vista::TresCuartos, false) => &[
            (27.0, 1.0, 5.5, 5.5),
            (30.0, 1.5, 12.0, 13.5),
            (35.0, 2.0, 15.0, 18.0),
            (47.0, 2.0, 14.0, 16.5),
            (70.0, 1.0, 13.0, 13.5),
            (84.0, 1.0, 14.0, 15.5),
        ],
        (Vista::TresCuartos, true) => &[
            (27.0, 1.0, 5.0, 5.0),
            (30.0, 1.5, 10.5, 12.0),
            (35.0, 2.0, 13.0, 15.5),
            (47.0, 2.0, 12.5, 15.5),
            (66.0, 1.0, 10.0, 11.0),
            (84.0, 1.0, 14.5, 15.5),
        ],
    };
    let mut pts: Vec<(f32, f32)> = rodajas
        .iter()
        .map(|(y, xc, _, d)| (xc + d, y + dy))
        .collect();
    let ultima = rodajas[rodajas.len() - 1];
    pts.push((ultima.1 + 0.5, ultima.0 + 4.0 + dy));
    pts.extend(rodajas.iter().rev().map(|(y, xc, i, _)| (xc - i, y + dy)));
    c.mancha(&pts);
}

/// El cuello, de la nuca a los hombros.
fn cuello(c: &mut Cuerpo, arriba: (f32, f32), abajo: (f32, f32)) {
    c.miembro(&[arriba, abajo], &[8.4, 9.4]);
}

// ---------------------------------------------------------------------------
// Las poses
// ---------------------------------------------------------------------------

/// Hombre o mujer.
#[derive(Clone, Copy, PartialEq)]
enum Quien {
    Hombre,
    Mujer,
}

/// **De pie o caminando, de frente o de espaldas.** `paso` va de 0 (quieto)
/// a 1 (zancada): una rodilla se dobla, el talon sube y los brazos se
/// abren un poco.
fn frontal(c: &mut Cuerpo, quien: Quien, vista: Vista, paso: f32, pelo: Pelo) {
    let mujer = quien == Quien::Mujer;
    let w = if mujer { 0.88 } else { 1.0 };
    let hombro = if mujer { 15.5 } else { 17.5 };
    let cadera_x = if mujer { 8.0 } else { 8.5 };
    // Parte «Piernas»: dos trazos que se estrechan hasta el tobillo; al
    // caminar, la derecha se dobla y el talon se levanta.
    c.parte("Piernas");
    pierna(c, (-cadera_x, 86.0), (-8.0 + paso, 124.0), (-7.0, 161.0), w);
    pierna(
        c,
        (cadera_x, 86.0),
        (8.5 + paso * 2.0, 121.0 - paso * 2.0),
        (8.0 + paso, 157.0 - paso * 5.0),
        w,
    );
    // Parte «Pies»: los zapatos, vistos de frente (o el talon de espaldas).
    c.parte("Pies");
    pie(c, (-7.5, 163.0), (-10.0, 168.0), w);
    pie(
        c,
        (8.0 + paso, 159.0 - paso * 5.0),
        (10.0 + paso * 0.5, 165.0 - paso * 2.0),
        w,
    );
    // Parte «Torso»: cuello, hombros, pecho, cintura y cadera.
    c.parte("Torso");
    melena_detras(c, 0.0, 11.5, vista, pelo);
    cuello(c, (0.0, 18.0), (0.0, 29.0));
    torso_de(c, vista, mujer, 0.0);
    // Parte «Ropa»: cuello de la camisa, cinturon, bajo de la falda.
    c.parte("Ropa");
    if mujer {
        c.mancha(&[
            (-12.5, 66.0),
            (12.5, 66.0),
            (17.0, 92.0),
            (20.0, 116.0),
            (0.0, 118.0),
            (-20.0, 116.0),
            (-17.0, 92.0),
        ]);
        c.curva(&[(4.0, 74.0), (6.0, 96.0), (8.0, 115.0)]);
        if vista == Vista::Frente {
            c.curva(&[(-6.0, 29.0), (0.0, 36.0), (6.0, 29.0)]);
        }
    } else {
        c.curva(&[(-15.5, 74.0), (0.0, 75.5), (15.5, 74.0)]);
        if vista == Vista::Frente {
            c.curva(&[(-5.0, 28.0), (0.0, 34.0), (5.0, 28.0)]);
            c.curva(&[(0.0, 34.0), (0.0, 73.0)]);
            c.curva(&[(0.0, 77.0), (0.0, 88.0)]);
        } else {
            c.curva(&[(-5.0, 28.5), (0.0, 29.5), (5.0, 28.5)]);
            c.curva(&[(-10.0, 79.0), (-4.0, 80.0)]);
            c.curva(&[(4.0, 80.0), (10.0, 79.0)]);
        }
    }
    // Parte «Brazos»: cuelgan junto al cuerpo; al caminar se separan.
    c.parte("Brazos");
    brazo(
        c,
        (-hombro, 35.0),
        (-hombro - 3.5 - paso * 2.0, 63.0),
        (-hombro - 4.5 - paso * 3.0, 87.0),
        w,
    );
    let (codo, muneca) = c.brazo.unwrap_or((
        (hombro + 3.5 + paso, 63.0),
        (hombro + 3.0 + paso * 3.0, 86.0 - paso * 4.0),
    ));
    brazo(c, (hombro, 35.0), codo, muneca, w);
    cabeza(c, 0.0, 11.5, 1.0, vista, pelo);
}

/// **De pie o caminando, de lado** (mira a la derecha). `paso` de 0 a 1.
fn de_lado(c: &mut Cuerpo, quien: Quien, paso: f32, pelo: Pelo) {
    let mujer = quien == Quien::Mujer;
    let w = if mujer { 0.88 } else { 1.0 };
    let z = paso * 20.0;
    // Parte «Brazo de atras» y «Pierna de atras».
    c.parte("Brazo de atrás");
    brazo(
        c,
        (1.0, 36.0),
        (1.0 + z * 0.5, 63.0),
        (4.0 + z * 1.0, 87.0 - z * 0.15),
        w,
    );
    c.parte("Pierna de atrás");
    pierna(
        c,
        (-1.0, 84.0),
        (-2.0 - z * 0.3, 123.0),
        (-2.0 - z * 1.0, 160.0 - z * 0.1),
        w,
    );
    pie(
        c,
        (-5.0 - z * 1.05, 165.0 - z * 0.1),
        (8.0 - z * 0.9, 167.5 + z * 0.02),
        w,
    );
    // Parte «Torso»: ligeramente arqueado, con el pecho delante.
    c.parte("Torso");
    melena_detras(c, 2.0, 11.5, Vista::Lado, pelo);
    cuello(c, (1.5, 18.0), (1.0, 29.0));
    torso_de(c, Vista::Lado, mujer, 0.0);
    c.parte("Ropa");
    if !mujer {
        c.curva(&[(-10.0, 73.5), (0.0, 74.5), (10.5, 73.5)]);
        c.curva(&[(5.0, 28.0), (7.0, 33.0)]);
    }
    // Parte «Pierna de delante».
    c.parte("Pierna de delante");
    pierna(
        c,
        (1.0, 84.0),
        (3.0 + z * 0.6, 123.0 - z * 0.1),
        (1.0 + z * 1.0, 160.0),
        w,
    );
    pie(
        c,
        (-2.0 + z * 1.0, 165.5),
        (11.0 + z * 1.1, 167.0 - z * 0.15),
        w,
    );
    // Parte «Ropa»: la falda, por delante de las dos piernas.
    if mujer {
        c.parte("Ropa");
        c.mancha(&[
            (-8.0, 64.0),
            (9.0, 64.0),
            (13.0 + z * 0.1, 92.0),
            (16.0 + z * 0.4, 117.0),
            (0.0, 118.5),
            (-14.0 - z * 0.3, 116.0),
            (-12.0, 92.0),
        ]);
        c.curva(&[(1.0, 74.0), (2.0, 98.0), (0.0, 116.0)]);
    }
    // Parte «Brazo de delante»: al caminar va hacia atras.
    c.parte("Brazo de delante");
    let (codo, muneca) = c
        .brazo
        .unwrap_or(((-1.5 - z * 0.5, 63.0), (0.0 - z * 1.0, 87.0)));
    brazo(c, (0.0, 36.0), codo, muneca, w);
    cabeza(c, 2.0, 11.5, 1.0, Vista::Lado, pelo);
}

/// **De pie en tres cuartos** (mira a la derecha y un poco hacia
/// nosotros). `brazo_alzado` dobla el brazo derecho: `Some((codo,
/// muneca))`.
fn tres_cuartos(
    c: &mut Cuerpo,
    quien: Quien,
    pelo: Pelo,
    paso: f32,
    brazo_alzado: Option<((f32, f32), (f32, f32))>,
) {
    let mujer = quien == Quien::Mujer;
    let w = if mujer { 0.88 } else { 1.0 };
    let z = paso * 14.0;
    c.parte("Brazo de atrás");
    brazo(c, (-12.0, 36.0), (-14.0, 63.0), (-13.0 + z * 0.3, 87.0), w);
    c.parte("Piernas");
    pierna(
        c,
        (-5.5, 85.0),
        (-6.0 - z * 0.4, 123.0),
        (-6.5 - z * 0.9, 160.0),
        w,
    );
    pie(c, (-8.0 - z, 164.5), (-1.0 - z * 0.8, 168.0), w);
    pierna(
        c,
        (7.5, 85.0),
        (9.0 + z * 0.5, 123.0),
        (8.5 + z * 0.9, 160.5),
        w,
    );
    pie(c, (6.5 + z, 165.0), (15.5 + z, 167.5), w);
    c.parte("Torso");
    melena_detras(c, 3.0, 11.5, Vista::TresCuartos, pelo);
    cuello(c, (1.0, 18.0), (1.0, 29.0));
    torso_de(c, Vista::TresCuartos, mujer, 0.0);
    c.parte("Ropa");
    if mujer {
        c.mancha(&[
            (-10.0, 66.0),
            (12.0, 66.0),
            (16.0, 92.0),
            (19.0, 116.0),
            (2.0, 118.5),
            (-16.0, 115.0),
            (-14.0, 92.0),
        ]);
        c.curva(&[(6.0, 74.0), (8.0, 96.0), (9.0, 116.0)]);
    } else {
        c.curva(&[(-13.0, 74.0), (2.0, 75.5), (15.0, 73.5)]);
        c.curva(&[(1.0, 28.5), (5.0, 33.0), (8.0, 28.0)]);
        c.curva(&[(5.0, 33.0), (6.0, 73.0)]);
    }
    c.parte("Brazo de delante");
    match brazo_alzado {
        Some((codo, muneca)) => brazo(c, (15.5, 36.0), codo, muneca, w),
        None => brazo(c, (15.5, 36.0), (19.0, 63.0), (18.0 - z * 0.3, 87.0), w),
    }
    cabeza(c, 3.0, 11.5, 1.0, Vista::TresCuartos, pelo);
}

/// La silla de las personas sentadas (de lado, mirando a la derecha):
/// asiento a 45 de altura, respaldo y patas.
fn silla_de_lado(c: &mut Cuerpo, x: f32) {
    c.parte("Silla");
    c.trazo(&[(x - 18.0, 170.0), (x - 16.0, 125.0)], 1.6);
    c.trazo(&[(x + 26.0, 170.0), (x + 26.0, 125.0)], 1.6);
    c.trazo(&[(x - 20.0, 125.0), (x + 30.0, 125.0)], 2.4);
    c.trazo(&[(x - 16.0, 125.0), (x - 22.0, 76.0)], 2.0);
}

/// **Sentada de lado** en una silla (mira a la derecha). `cruzada` monta
/// una pierna sobre la otra.
fn sentada_de_lado(c: &mut Cuerpo, quien: Quien, pelo: Pelo, cruzada: bool) {
    let mujer = quien == Quien::Mujer;
    let w = if mujer { 0.88 } else { 1.0 };
    silla_de_lado(c, 0.0);
    c.parte("Brazo de atrás");
    brazo(c, (-1.0, 69.0), (3.0, 96.0), (27.0, 110.0), w);
    c.parte("Pierna de atrás");
    pierna(c, (0.0, 117.0), (40.0, 118.0), (41.0, 160.0), w);
    pie(c, (37.0, 165.0), (52.0, 167.5), w);
    c.parte("Torso");
    melena_detras(c, 0.0, 43.5, Vista::Lado, pelo);
    cuello(c, (-1.0, 50.0), (-1.5, 61.0));
    torso_de(c, Vista::Lado, mujer, 32.0);
    c.parte("Pierna de delante");
    if cruzada {
        pierna(c, (2.0, 114.0), (40.0, 108.0), (34.0, 146.0), w);
        pie(c, (32.0, 150.0), (44.0, 156.0), w);
    } else {
        pierna(c, (2.0, 116.0), (42.0, 116.0), (44.0, 160.0), w);
        pie(c, (40.0, 165.5), (55.0, 167.0), w);
    }
    c.parte("Ropa");
    if mujer {
        c.curva(&[(-11.0, 98.0), (0.0, 99.0), (10.0, 98.0)]);
    } else {
        c.curva(&[(-11.0, 105.0), (0.0, 106.0), (11.0, 105.0)]);
    }
    c.parte("Brazo de delante");
    brazo(c, (-2.0, 69.0), (0.0, 96.0), (24.0, 106.0), w);
    cabeza(c, 0.0, 43.5, 1.0, Vista::Lado, pelo);
}

/// **Sentada de frente** en un banco: los muslos vienen hacia nosotros y
/// se ven cortos.
fn sentada_de_frente(c: &mut Cuerpo, quien: Quien, pelo: Pelo) {
    let mujer = quien == Quien::Mujer;
    let w = if mujer { 0.88 } else { 1.0 };
    let hombro = if mujer { 15.5 } else { 17.5 };
    // Parte «Asiento»: el canto del banco.
    c.parte("Asiento");
    c.trazo(&[(-34.0, 124.0), (34.0, 124.0)], 2.2);
    c.trazo(&[(-30.0, 124.0), (-30.0, 170.0)], 1.6);
    c.trazo(&[(30.0, 124.0), (30.0, 170.0)], 1.6);
    c.parte("Torso");
    melena_detras(c, 0.0, 43.5, Vista::Frente, pelo);
    cuello(c, (0.0, 50.0), (0.0, 61.0));
    torso_de(c, Vista::Frente, mujer, 32.0);
    // Parte «Piernas»: muslo corto (visto de frente), rodilla y pantorrilla.
    c.parte("Piernas");
    pierna(c, (-9.0, 114.0), (-12.0, 126.0), (-13.0, 161.0), w * 1.05);
    pierna(c, (9.0, 114.0), (12.0, 126.0), (13.0, 161.0), w * 1.05);
    c.miembro(&[(-9.0, 113.0), (-11.5, 124.0)], &[17.0 * w, 15.0 * w]);
    c.miembro(&[(9.0, 113.0), (11.5, 124.0)], &[17.0 * w, 15.0 * w]);
    c.parte("Pies");
    pie(c, (-13.5, 163.0), (-16.0, 168.0), w);
    pie(c, (13.5, 163.0), (16.0, 168.0), w);
    c.parte("Ropa");
    if mujer {
        c.curva(&[(-6.0, 61.0), (0.0, 68.0), (6.0, 61.0)]);
        c.curva(&[(-19.0, 122.0), (0.0, 124.0), (19.0, 122.0)]);
    } else {
        c.curva(&[(-5.0, 60.0), (0.0, 66.0), (5.0, 60.0)]);
        c.curva(&[(0.0, 66.0), (0.0, 104.0)]);
        c.curva(&[(-15.5, 105.0), (0.0, 106.5), (15.5, 105.0)]);
    }
    // Parte «Brazos»: las manos descansan sobre las rodillas.
    c.parte("Brazos");
    brazo(c, (-hombro, 67.0), (-hombro - 5.0, 94.0), (-10.0, 113.0), w);
    brazo(c, (hombro, 67.0), (hombro + 5.0, 94.0), (10.0, 113.0), w);
    cabeza(c, 0.0, 43.5, 1.0, Vista::Frente, pelo);
}

/// **Sentada en 3/4** con las piernas cruzadas (mira a la derecha).
fn sentada_tres_cuartos(c: &mut Cuerpo, quien: Quien, pelo: Pelo) {
    let mujer = quien == Quien::Mujer;
    let w = if mujer { 0.88 } else { 1.0 };
    c.parte("Asiento");
    c.trazo(&[(-24.0, 126.0), (30.0, 122.0)], 2.2);
    c.trazo(&[(-20.0, 126.0), (-20.0, 170.0)], 1.6);
    c.trazo(&[(27.0, 122.0), (27.0, 166.0)], 1.6);
    c.parte("Brazo de atrás");
    brazo(c, (-11.0, 68.0), (-15.0, 95.0), (-12.0, 116.0), w);
    c.parte("Pierna de atrás");
    pierna(c, (-4.0, 116.0), (18.0, 124.0), (16.0, 162.0), w);
    pie(c, (14.0, 165.0), (25.0, 168.0), w);
    c.parte("Torso");
    melena_detras(c, 3.0, 43.5, Vista::TresCuartos, pelo);
    cuello(c, (1.0, 50.0), (1.0, 61.0));
    torso_de(c, Vista::TresCuartos, mujer, 32.0);
    // Parte «Pierna de delante»: cruzada sobre la otra rodilla.
    c.parte("Pierna de delante");
    pierna(c, (6.0, 113.0), (32.0, 113.0), (36.0, 148.0), w);
    pie(c, (36.0, 152.0), (44.0, 159.0), w);
    c.parte("Ropa");
    if mujer {
        c.curva(&[(-12.0, 98.0), (2.0, 99.5), (14.0, 98.0)]);
    } else {
        c.curva(&[(-12.0, 105.0), (2.0, 106.0), (14.0, 105.0)]);
        c.curva(&[(1.0, 60.5), (5.0, 65.0), (8.0, 60.0)]);
    }
    c.parte("Brazo de delante");
    brazo(c, (15.0, 68.0), (19.0, 95.0), (30.0, 110.0), w);
    cabeza(c, 3.0, 43.5, 1.0, Vista::TresCuartos, pelo);
}

// ----------------------------------------------------------- las del menu

fn hombre_caminando_frente(l: &mut Lapiz) {
    frontal(
        &mut Cuerpo::en(l),
        Quien::Hombre,
        Vista::Frente,
        1.0,
        Pelo::Corto,
    );
}

fn mujer_caminando_frente(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.k = 0.96;
    frontal(&mut c, Quien::Mujer, Vista::Frente, 1.0, Pelo::Largo);
}

fn hombre_caminando_espaldas(l: &mut Lapiz) {
    frontal(
        &mut Cuerpo::en(l),
        Quien::Hombre,
        Vista::Espalda,
        1.0,
        Pelo::Corto,
    );
}

fn hombre_frente(l: &mut Lapiz) {
    frontal(
        &mut Cuerpo::en(l),
        Quien::Hombre,
        Vista::Frente,
        0.0,
        Pelo::Corto,
    );
}

fn mujer_frente(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.k = 0.96;
    frontal(&mut c, Quien::Mujer, Vista::Frente, 0.0, Pelo::Mono);
}

fn hombre_espaldas(l: &mut Lapiz) {
    frontal(
        &mut Cuerpo::en(l),
        Quien::Hombre,
        Vista::Espalda,
        0.0,
        Pelo::Corto,
    );
}

fn mujer_espaldas(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.k = 0.96;
    frontal(&mut c, Quien::Mujer, Vista::Espalda, 0.0, Pelo::Largo);
}

fn hombre_lado(l: &mut Lapiz) {
    de_lado(&mut Cuerpo::en(l), Quien::Hombre, 0.0, Pelo::Corto);
}

fn mujer_caminando_lado(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.k = 0.96;
    de_lado(&mut c, Quien::Mujer, 1.0, Pelo::Largo);
}

fn hombre_caminando_lado(l: &mut Lapiz) {
    de_lado(&mut Cuerpo::en(l), Quien::Hombre, 1.0, Pelo::Corto);
}

fn hombre_tres_cuartos(l: &mut Lapiz) {
    tres_cuartos(&mut Cuerpo::en(l), Quien::Hombre, Pelo::Corto, 0.0, None);
}

fn hombre_tres_cuartos_telefono(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    // El brazo de delante sube el telefono a la oreja.
    tres_cuartos(
        &mut c,
        Quien::Hombre,
        Pelo::Corto,
        0.0,
        Some(((24.0, 56.0), (10.0, 22.0))),
    );
    c.parte("Objeto (teléfono)");
    c.placa(7.5, 14.5, 3.4, 13.0, 25.0);
}

fn mujer_frente_telefono(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.k = 0.96;
    // El brazo derecho, doblado, sostiene el telefono delante del pecho.
    c.brazo = Some(((17.0, 62.0), (4.0, 54.0)));
    frontal(&mut c, Quien::Mujer, Vista::Frente, 0.0, Pelo::Largo);
    c.parte("Objeto (teléfono)");
    c.placa(1.0, 47.0, 7.0, 13.0, -8.0);
}

fn hombre_lado_leyendo(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    // El brazo de delante sostiene un libro abierto a la altura del pecho.
    c.brazo = Some(((-1.0, 62.0), (18.0, 52.0)));
    de_lado(&mut c, Quien::Hombre, 0.0, Pelo::Corto);
    c.parte("Objeto (libro)");
    c.mancha(&[(14.0, 44.0), (22.0, 38.0), (28.0, 52.0), (21.0, 58.0)]);
    c.curva(&[(18.0, 41.0), (24.5, 55.0)]);
}

fn mujer_tres_cuartos_leyendo(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.k = 0.96;
    tres_cuartos(
        &mut c,
        Quien::Mujer,
        Pelo::Largo,
        0.0,
        Some(((21.0, 62.0), (14.0, 56.0))),
    );
    c.parte("Objeto (libro)");
    c.mancha(&[(6.0, 46.0), (20.0, 44.0), (22.0, 58.0), (8.0, 61.0)]);
    c.curva(&[(13.0, 45.0), (15.0, 59.5)]);
}

fn hombre_sentado_frente(l: &mut Lapiz) {
    sentada_de_frente(&mut Cuerpo::en(l), Quien::Hombre, Pelo::Corto);
}

fn mujer_sentada_lado(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.k = 0.96;
    sentada_de_lado(&mut c, Quien::Mujer, Pelo::Largo, true);
}

fn hombre_sentado_lado(l: &mut Lapiz) {
    sentada_de_lado(&mut Cuerpo::en(l), Quien::Hombre, Pelo::Corto, false);
}

fn mujer_sentada_tres_cuartos(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.k = 0.96;
    sentada_tres_cuartos(&mut c, Quien::Mujer, Pelo::Mono);
}

fn hombre_sentado_tres_cuartos(l: &mut Lapiz) {
    sentada_tres_cuartos(&mut Cuerpo::en(l), Quien::Hombre, Pelo::Corto);
}

/// **Nino corriendo** (de lado): 118 de alto, con la cabeza mas grande
/// (1/5,5 de la altura), el cuerpo inclinado y una rodilla arriba.
fn nino_corriendo(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.k = 0.66;
    let w = 0.95;
    c.parte("Brazo de atrás");
    brazo(&mut c, (6.0, 50.0), (24.0, 66.0), (34.0, 50.0), w);
    c.parte("Pierna de atrás");
    pierna(&mut c, (-2.0, 96.0), (-22.0, 120.0), (-46.0, 112.0), w);
    pie(&mut c, (-48.0, 109.0), (-50.0, 120.0), w);
    c.parte("Torso");
    cuello(&mut c, (10.0, 36.0), (8.0, 46.0));
    torso_inclinado(
        &mut c,
        &[
            (8.0, 44.0),
            (7.0, 49.0),
            (6.0, 60.0),
            (3.0, 78.0),
            (0.0, 92.0),
            (-1.0, 100.0),
        ],
        &[12.0, 25.0, 28.0, 24.0, 25.0, 24.0],
    );
    c.parte("Ropa");
    c.curva(&[(-11.0, 88.0), (0.0, 90.0), (11.0, 87.0)]);
    c.parte("Pierna de delante");
    pierna(&mut c, (2.0, 96.0), (30.0, 112.0), (22.0, 150.0), w);
    pie(&mut c, (20.0, 155.0), (35.0, 157.0), w);
    c.parte("Brazo de delante");
    brazo(&mut c, (6.0, 50.0), (-8.0, 70.0), (-20.0, 58.0), w);
    cabeza(&mut c, 14.0, 22.0, 1.35, Vista::Lado, Pelo::Corto);
}

/// **Pareja caminando** de lado: ella detras, el delante, cogidos de la
/// mano.
fn pareja_caminando(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.ox = 30.0;
    c.k = 0.96;
    de_lado(&mut c, Quien::Mujer, 0.7, Pelo::Largo);
    let mut c = Cuerpo::en(l);
    de_lado(&mut c, Quien::Hombre, 0.8, Pelo::Corto);
}

/// La silla de ruedas de lado (mira a la derecha): rueda grande, rueda
/// loca delante, asiento, respaldo con empunadura y reposapies.
fn silla_de_ruedas_lado(c: &mut Cuerpo) {
    c.parte("Silla de ruedas");
    c.trazo(&[(-14.0, 70.0), (-20.0, 74.0), (-14.0, 128.0)], 2.2);
    c.trazo(&[(-14.0, 126.0), (34.0, 126.0)], 2.4);
    c.trazo(&[(30.0, 126.0), (44.0, 160.0), (56.0, 160.0)], 2.0);
    let (cx, cy, r) = (2.0, 138.0, 31.0);
    c.trazo(
        &(0..=40)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 40.0;
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect::<Vec<_>>(),
        2.6,
    );
    c.trazo(
        &(0..=40)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 40.0;
                (cx + 26.0 * a.cos(), cy + 26.0 * a.sin())
            })
            .collect::<Vec<_>>(),
        1.2,
    );
    for i in 0..6 {
        let a = std::f32::consts::PI * i as f32 / 6.0;
        c.curva(&[
            (cx + 26.0 * a.cos(), cy + 26.0 * a.sin()),
            (cx - 26.0 * a.cos(), cy - 26.0 * a.sin()),
        ]);
    }
    c.ovalo(38.0, 163.0, 7.0, 7.0);
}

fn hombre_silla_de_ruedas(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    silla_de_ruedas_lado(&mut c);
    c.parte("Pierna de atrás");
    pierna(&mut c, (0.0, 117.0), (40.0, 118.0), (46.0, 152.0), 1.0);
    pie(&mut c, (44.0, 156.0), (58.0, 157.0), 1.0);
    c.parte("Torso");
    cuello(&mut c, (-1.0, 50.0), (-1.5, 61.0));
    torso_de(&mut c, Vista::Lado, false, 32.0);
    c.parte("Pierna de delante");
    pierna(&mut c, (2.0, 116.0), (42.0, 116.0), (48.0, 152.0), 1.0);
    pie(&mut c, (46.0, 156.0), (60.0, 156.0), 1.0);
    c.parte("Ropa");
    c.curva(&[(-11.0, 105.0), (0.0, 106.0), (11.0, 105.0)]);
    // Parte «Brazo de delante»: la mano en el aro de la rueda.
    c.parte("Brazo de delante");
    brazo(&mut c, (-2.0, 69.0), (-4.0, 96.0), (16.0, 112.0), 1.0);
    cabeza(&mut c, 0.0, 43.5, 1.0, Vista::Lado, Pelo::Corto);
}

/// Una rueda de bici: llanta, cubierta y buje (`rx` distinto de `ry` la
/// pone en 3/4).
fn rueda(c: &mut Cuerpo, cx: f32, cy: f32, rx: f32, ry: f32) {
    let aro = |k: f32| {
        (0..=48)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 48.0;
                (cx + rx * k * a.cos(), cy + ry * k * a.sin())
            })
            .collect::<Vec<_>>()
    };
    c.trazo(&aro(1.0), 2.4);
    c.trazo(&aro(0.9), 0.9);
    c.ovalo(cx, cy, 2.2, 2.2);
}

/// **Ciclista de lado** (mira a la derecha), sobre una bici de paseo.
fn ciclista_lado(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    let (tras, del, eje) = ((-50.0, 136.0), (56.0, 136.0), (-2.0, 140.0));
    let sillin = (-14.0, 86.0);
    let manillar = (40.0, 80.0);
    c.parte("Pierna de atrás");
    pierna(&mut c, (-12.0, 84.0), (10.0, 104.0), (6.0, 145.0), 1.0);
    pie(&mut c, (4.0, 150.0), (16.0, 151.0), 1.0);
    c.parte("Bicicleta");
    rueda(&mut c, tras.0, tras.1, 34.0, 34.0);
    rueda(&mut c, del.0, del.1, 34.0, 34.0);
    c.trazo(&[tras, eje, (-12.0, 94.0), tras], 2.0);
    c.trazo(&[eje, (36.0, 92.0), (-12.0, 94.0)], 2.0);
    c.trazo(&[del, (34.0, 84.0), manillar, (32.0, 82.0)], 2.0);
    c.trazo(&[(-12.0, 94.0), sillin], 2.0);
    c.trazo(&[(-22.0, 86.0), (-6.0, 85.0)], 4.0);
    c.ovalo(eje.0, eje.1, 6.0, 6.0);
    c.parte("Torso");
    cuello(&mut c, (24.0, 34.0), (18.0, 42.0));
    torso_inclinado(
        &mut c,
        &[
            (20.0, 40.0),
            (16.0, 44.0),
            (6.0, 54.0),
            (-6.0, 70.0),
            (-13.0, 84.0),
        ],
        &[11.0, 22.0, 25.0, 22.0, 24.0],
    );
    c.parte("Pierna de delante");
    pierna(&mut c, (-10.0, 82.0), (24.0, 96.0), (-2.0, 124.0), 1.0);
    pie(&mut c, (-6.0, 127.0), (6.0, 131.0), 1.0);
    c.parte("Brazo de delante");
    brazo(&mut c, (16.0, 46.0), (28.0, 66.0), (38.0, 78.0), 1.0);
    cabeza(&mut c, 29.0, 26.0, 1.0, Vista::Lado, Pelo::Corto);
}

/// **Ciclista en 3/4**: las ruedas se ven como elipses y la de delante,
/// mas cerca, algo mas grande.
fn ciclista_tres_cuartos(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    let (tras, del, eje) = ((-30.0, 128.0), (28.0, 146.0), (-2.0, 140.0));
    c.parte("Bicicleta");
    rueda(&mut c, tras.0, tras.1, 19.0, 31.0);
    c.parte("Pierna de atrás");
    pierna(&mut c, (-8.0, 82.0), (2.0, 106.0), (4.0, 140.0), 1.0);
    pie(&mut c, (2.0, 145.0), (12.0, 148.0), 1.0);
    c.parte("Bicicleta");
    rueda(&mut c, del.0, del.1, 22.0, 34.0);
    c.trazo(&[tras, eje, (-10.0, 92.0), tras], 2.0);
    c.trazo(&[eje, (20.0, 94.0), (-10.0, 92.0)], 2.0);
    c.trazo(&[del, (20.0, 92.0), (22.0, 80.0)], 2.0);
    c.trazo(&[(8.0, 78.0), (22.0, 80.0), (36.0, 83.0)], 2.2);
    c.trazo(&[(-10.0, 92.0), (-11.0, 84.0)], 2.0);
    c.trazo(&[(-17.0, 84.0), (-5.0, 83.0)], 4.0);
    c.ovalo(eje.0, eje.1, 5.0, 6.0);
    c.parte("Brazo de atrás");
    brazo(&mut c, (-2.0, 46.0), (2.0, 64.0), (10.0, 78.0), 1.0);
    c.parte("Torso");
    cuello(&mut c, (10.0, 32.0), (7.0, 42.0));
    torso_inclinado(
        &mut c,
        &[
            (8.0, 40.0),
            (6.0, 44.0),
            (2.0, 54.0),
            (-4.0, 70.0),
            (-9.0, 84.0),
        ],
        &[11.0, 28.0, 31.0, 26.0, 27.0],
    );
    c.parte("Pierna de delante");
    pierna(&mut c, (-2.0, 82.0), (16.0, 102.0), (8.0, 128.0), 1.0);
    pie(&mut c, (6.0, 132.0), (17.0, 135.0), 1.0);
    c.parte("Brazo de delante");
    brazo(&mut c, (17.0, 46.0), (26.0, 64.0), (33.0, 80.0), 1.0);
    cabeza(&mut c, 13.0, 23.0, 1.0, Vista::TresCuartos, Pelo::Corto);
}

// ---------------------------------------------------------------------------
// Personas en planta (vistas desde arriba)
// ---------------------------------------------------------------------------

/// El relleno de la ropa vista desde arriba y el del pelo.
const ROPA: ColorRgba = hex(0xe6e9ef);
const PELO: ColorRgba = hex(0xb6bdc9);

/// **Una persona vista desde arriba**, de pie en `(cx, cy)` y girada
/// `grados` (0 = mirando hacia abajo de la pantalla). `paso` adelanta un
/// pie y balancea los brazos.
fn de_pie_en_planta(c: &mut Cuerpo, paso: f32, melena: bool) {
    // Parte «Pies»: asoman por delante de los hombros.
    c.parte("Pies");
    c.miembro(
        &[(-9.0, 2.0 - paso * 6.0), (-9.5, 15.0 - paso * 8.0)],
        &[9.0, 8.0],
    );
    c.miembro(
        &[(9.0, 4.0 + paso * 8.0), (9.5, 17.0 + paso * 10.0)],
        &[9.0, 8.0],
    );
    // Parte «Brazos»: a los lados, uno adelante y otro atras al caminar.
    c.parte("Brazos");
    c.miembro(&[(-23.0, -2.0), (-25.0, 4.0 + paso * 10.0)], &[9.0, 8.0]);
    c.miembro(&[(23.0, -2.0), (25.0, 4.0 - paso * 10.0)], &[9.0, 8.0]);
    // Parte «Hombros»: el ovalo del torso visto desde arriba.
    c.parte("Hombros");
    let hombros: Vec<(f32, f32)> = (0..20)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / 20.0;
            (27.0 * a.cos(), 12.0 * a.sin() + 1.5 * (2.0 * a).cos().abs())
        })
        .collect();
    c.mancha_con(&hombros, ROPA);
    // Parte «Cabeza»: el pelo desde arriba y la nariz hacia delante.
    c.parte("Cabeza");
    c.ovalo(0.0, 1.0, 9.0, 10.0);
    c.trazo(&[(-2.0, 10.5), (0.0, 13.0), (2.0, 10.5)], 1.0);
    let pelo: Vec<(f32, f32)> = if melena {
        vec![
            (0.0, -12.0),
            (9.5, -6.0),
            (10.0, 4.0),
            (7.0, 6.0),
            (0.0, 4.0),
            (-7.0, 6.0),
            (-10.0, 4.0),
            (-9.5, -6.0),
        ]
    } else {
        vec![
            (0.0, -9.5),
            (8.5, -5.0),
            (8.6, 3.0),
            (4.0, 5.0),
            (-4.0, 5.0),
            (-8.6, 3.0),
            (-8.5, -5.0),
        ]
    };
    c.mancha_con(&pelo, PELO);
}

fn planta_de_pie(l: &mut Lapiz) {
    de_pie_en_planta(&mut Cuerpo::en(l), 0.0, false);
}

fn planta_caminando_1(l: &mut Lapiz) {
    de_pie_en_planta(&mut Cuerpo::en(l), 1.0, false);
}

fn planta_caminando_2(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.espejo = true;
    de_pie_en_planta(&mut c, 0.8, true);
    c.parte("Objeto (bolso)");
    c.placa(31.0, 2.0, 9.0, 26.0, 0.0);
}

/// Tumbada boca arriba (en una toalla o una cama), con la cabeza arriba.
fn planta_tumbada(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    // Parte «Toalla»: la tela de debajo (90 × 190), con sus rayas.
    c.parte("Toalla");
    c.mancha_con(
        &[(-45.0, -12.0), (45.0, -12.0), (45.0, 182.0), (-45.0, 182.0)],
        hex(0xf4ece0),
    );
    for y in [8.0, 160.0] {
        c.trazo(&[(-44.0, y), (44.0, y)], 0.6);
    }
    c.parte("Piernas");
    c.miembro(
        &[(-7.0, 88.0), (-9.0, 128.0), (-10.0, 166.0)],
        &[15.0, 10.5, 8.0],
    );
    c.miembro(
        &[(7.0, 88.0), (11.0, 128.0), (15.0, 166.0)],
        &[15.0, 10.5, 8.0],
    );
    c.parte("Pies");
    c.miembro(&[(-10.0, 168.0), (-12.0, 176.0)], &[8.0, 7.0]);
    c.miembro(&[(15.0, 168.0), (18.0, 175.0)], &[8.0, 7.0]);
    c.parte("Torso");
    torso_de(&mut c, Vista::Frente, false, 0.0);
    c.parte("Ropa");
    c.curva(&[(-15.5, 74.0), (0.0, 75.5), (15.5, 74.0)]);
    c.parte("Brazos");
    brazo(&mut c, (-18.0, 34.0), (-24.0, 62.0), (-26.0, 88.0), 1.0);
    brazo(&mut c, (18.0, 34.0), (30.0, 18.0), (16.0, 6.0), 1.0);
    c.parte("Cabeza");
    c.ovalo(0.0, 12.0, 9.0, 11.5);
    c.mancha_con(
        &[
            (0.0, -0.5),
            (9.0, 3.0),
            (9.6, 10.0),
            (6.0, 6.0),
            (0.0, 5.0),
            (-6.0, 6.0),
            (-9.6, 10.0),
            (-9.0, 3.0),
        ],
        PELO,
    );
}

/// La silla vista desde arriba, mirando hacia abajo.
fn silla_en_planta(c: &mut Cuerpo) {
    c.parte("Silla");
    c.mancha_con(
        &[(-22.0, -20.0), (22.0, -20.0), (22.0, 24.0), (-22.0, 24.0)],
        PAPEL,
    );
    c.mancha_con(
        &[(-23.0, -28.0), (23.0, -28.0), (23.0, -20.0), (-23.0, -20.0)],
        hex(0xc9cfd8),
    );
}

/// Sentada en una silla, vista desde arriba: hombros, cabeza y los muslos
/// hacia delante.
fn planta_sentada_1(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    silla_en_planta(&mut c);
    c.parte("Piernas");
    c.miembro(&[(-9.0, -6.0), (-10.0, 30.0)], &[15.0, 12.0]);
    c.miembro(&[(9.0, -6.0), (10.0, 30.0)], &[15.0, 12.0]);
    c.parte("Pies");
    c.miembro(&[(-10.0, 34.0), (-11.0, 44.0)], &[9.0, 8.0]);
    c.miembro(&[(10.0, 34.0), (11.0, 44.0)], &[9.0, 8.0]);
    c.parte("Brazos");
    c.miembro(
        &[(-22.0, -14.0), (-20.0, 4.0), (-12.0, 18.0)],
        &[9.0, 8.0, 6.0],
    );
    c.miembro(
        &[(22.0, -14.0), (20.0, 4.0), (12.0, 18.0)],
        &[9.0, 8.0, 6.0],
    );
    c.parte("Hombros");
    let hombros: Vec<(f32, f32)> = (0..20)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / 20.0;
            (26.0 * a.cos(), -16.0 + 11.5 * a.sin())
        })
        .collect();
    c.mancha_con(&hombros, ROPA);
    c.parte("Cabeza");
    c.ovalo(0.0, -15.0, 9.0, 10.0);
    c.mancha_con(
        &[
            (0.0, -25.5),
            (8.5, -21.0),
            (8.6, -13.0),
            (4.0, -11.0),
            (-4.0, -11.0),
            (-8.6, -13.0),
            (-8.5, -21.0),
        ],
        PELO,
    );
}

/// Sentada en el suelo con las piernas cruzadas, vista desde arriba.
fn planta_sentada_2(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.parte("Piernas");
    c.miembro(
        &[(-8.0, 0.0), (-28.0, 22.0), (12.0, 30.0)],
        &[16.0, 12.0, 9.0],
    );
    c.miembro(
        &[(8.0, 0.0), (28.0, 24.0), (-12.0, 34.0)],
        &[16.0, 12.0, 9.0],
    );
    c.parte("Brazos");
    c.miembro(
        &[(-22.0, -6.0), (-26.0, 10.0), (-22.0, 22.0)],
        &[9.0, 8.0, 6.0],
    );
    c.miembro(
        &[(22.0, -6.0), (26.0, 10.0), (22.0, 22.0)],
        &[9.0, 8.0, 6.0],
    );
    c.parte("Hombros");
    let hombros: Vec<(f32, f32)> = (0..20)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / 20.0;
            (26.0 * a.cos(), -6.0 + 11.5 * a.sin())
        })
        .collect();
    c.mancha_con(&hombros, ROPA);
    c.parte("Cabeza");
    c.ovalo(0.0, -5.0, 9.0, 10.0);
    c.mancha_con(
        &[
            (0.0, -17.0),
            (9.5, -11.0),
            (10.0, -1.0),
            (7.0, 1.0),
            (0.0, -1.0),
            (-7.0, 1.0),
            (-10.0, -1.0),
            (-9.5, -11.0),
        ],
        PELO,
    );
}

/// La silla de ruedas desde arriba (mirando hacia abajo): ruedas grandes a
/// los lados, ruedas locas delante, reposapies y empunaduras.
fn silla_de_ruedas_en_planta(c: &mut Cuerpo) {
    c.parte("Silla de ruedas");
    for s in [-1.0f32, 1.0] {
        c.mancha_con(
            &[
                (s * 27.0, -32.0),
                (s * 33.0, -32.0),
                (s * 33.0, 26.0),
                (s * 27.0, 26.0),
            ],
            hex(0xc9cfd8),
        );
        c.ovalo(s * 20.0, 40.0, 3.0, 5.0);
        c.trazo(&[(s * 22.0, -36.0), (s * 22.0, -44.0)], 2.4);
    }
    c.mancha_con(
        &[(-24.0, -36.0), (24.0, -36.0), (24.0, 22.0), (-24.0, 22.0)],
        PAPEL,
    );
    c.mancha_con(
        &[(-16.0, 46.0), (16.0, 46.0), (16.0, 54.0), (-16.0, 54.0)],
        hex(0xc9cfd8),
    );
}

fn planta_silla_de_ruedas(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    silla_de_ruedas_en_planta(&mut c);
    c.parte("Piernas");
    c.miembro(
        &[(-9.0, -12.0), (-10.0, 24.0), (-10.0, 42.0)],
        &[15.0, 12.0, 9.0],
    );
    c.miembro(
        &[(9.0, -12.0), (10.0, 24.0), (10.0, 42.0)],
        &[15.0, 12.0, 9.0],
    );
    c.parte("Pies");
    c.miembro(&[(-10.0, 44.0), (-11.0, 52.0)], &[9.0, 8.0]);
    c.miembro(&[(10.0, 44.0), (11.0, 52.0)], &[9.0, 8.0]);
    c.parte("Brazos");
    c.miembro(
        &[(-22.0, -20.0), (-26.0, -2.0), (-27.0, 10.0)],
        &[9.0, 8.0, 6.0],
    );
    c.miembro(
        &[(22.0, -20.0), (26.0, -2.0), (27.0, 10.0)],
        &[9.0, 8.0, 6.0],
    );
    c.parte("Hombros");
    let hombros: Vec<(f32, f32)> = (0..20)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / 20.0;
            (25.0 * a.cos(), -22.0 + 11.0 * a.sin())
        })
        .collect();
    c.mancha_con(&hombros, ROPA);
    c.parte("Cabeza");
    c.ovalo(0.0, -21.0, 9.0, 10.0);
    c.mancha_con(
        &[
            (0.0, -31.5),
            (8.5, -27.0),
            (8.6, -19.0),
            (4.0, -17.0),
            (-4.0, -17.0),
            (-8.6, -19.0),
            (-8.5, -27.0),
        ],
        PELO,
    );
}

/// En bicicleta, vista desde arriba (va hacia abajo de la pantalla).
fn planta_bicicleta(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.parte("Bicicleta");
    c.mancha_con(
        &[(-3.0, -100.0), (3.0, -100.0), (3.0, -38.0), (-3.0, -38.0)],
        hex(0x6f7a8b),
    );
    c.mancha_con(
        &[(-3.0, 30.0), (3.0, 30.0), (3.0, 92.0), (-3.0, 92.0)],
        hex(0x6f7a8b),
    );
    c.trazo(&[(0.0, -60.0), (0.0, 50.0)], 3.0);
    c.trazo(
        &[(-28.0, 32.0), (-20.0, 28.0), (20.0, 28.0), (28.0, 32.0)],
        2.6,
    );
    c.mancha_con(
        &[(-6.0, -42.0), (6.0, -42.0), (4.0, -22.0), (-4.0, -22.0)],
        hex(0x3a3f4b),
    );
    c.parte("Piernas");
    c.miembro(
        &[(-8.0, -30.0), (-14.0, -6.0), (-9.0, 8.0)],
        &[13.0, 10.0, 7.0],
    );
    c.miembro(
        &[(8.0, -30.0), (14.0, -6.0), (9.0, 8.0)],
        &[13.0, 10.0, 7.0],
    );
    c.parte("Brazos");
    c.miembro(
        &[(-21.0, -8.0), (-25.0, 12.0), (-24.0, 28.0)],
        &[9.0, 8.0, 6.0],
    );
    c.miembro(
        &[(21.0, -8.0), (25.0, 12.0), (24.0, 28.0)],
        &[9.0, 8.0, 6.0],
    );
    c.parte("Hombros");
    let hombros: Vec<(f32, f32)> = (0..20)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / 20.0;
            (25.0 * a.cos(), -12.0 + 11.0 * a.sin())
        })
        .collect();
    c.mancha_con(&hombros, ROPA);
    c.parte("Cabeza");
    c.ovalo(0.0, -6.0, 9.0, 10.0);
    c.mancha_con(
        &[
            (0.0, -17.0),
            (8.5, -12.0),
            (8.6, -4.0),
            (4.0, -2.0),
            (-4.0, -2.0),
            (-8.6, -4.0),
            (-8.5, -12.0),
        ],
        PELO,
    );
}

/// Con cochecito: la persona empuja el carro que va delante.
fn planta_cochecito(l: &mut Lapiz) {
    let mut c = Cuerpo::en(l);
    c.parte("Cochecito");
    for s in [-1.0f32, 1.0] {
        c.mancha_con(
            &[
                (s * 26.0, 40.0),
                (s * 30.0, 40.0),
                (s * 30.0, 54.0),
                (s * 26.0, 54.0),
            ],
            hex(0x6f7a8b),
        );
        c.mancha_con(
            &[
                (s * 26.0, 100.0),
                (s * 30.0, 100.0),
                (s * 30.0, 114.0),
                (s * 26.0, 114.0),
            ],
            hex(0x6f7a8b),
        );
    }
    c.mancha_con(
        &[(-24.0, 36.0), (24.0, 36.0), (24.0, 116.0), (-24.0, 116.0)],
        PAPEL,
    );
    c.mancha_con(
        &[
            (-24.0, 76.0),
            (-20.0, 92.0),
            (0.0, 100.0),
            (20.0, 92.0),
            (24.0, 76.0),
        ],
        hex(0xdde3ec),
    );
    c.trazo(&[(-22.0, 24.0), (22.0, 24.0)], 3.0);
    c.trazo(&[(-20.0, 24.0), (-22.0, 36.0)], 2.0);
    c.trazo(&[(20.0, 24.0), (22.0, 36.0)], 2.0);
    c.parte("Persona");
    de_pie_en_planta(&mut c, 0.6, true);
    c.parte("Brazos (empujando)");
    c.miembro(
        &[(-22.0, -2.0), (-20.0, 12.0), (-18.0, 24.0)],
        &[9.0, 8.0, 6.0],
    );
    c.miembro(
        &[(22.0, -2.0), (20.0, 12.0), (18.0, 24.0)],
        &[9.0, 8.0, 6.0],
    );
}

// ---------------------------------------------------------------------------
// El registro
// ---------------------------------------------------------------------------

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let alzado: [Dibujo; 23] = [
        (
            "pers-hombre-caminando-frente",
            "Hombre caminando de frente",
            hombre_caminando_frente,
        ),
        (
            "pers-mujer-caminando-frente",
            "Mujer caminando de frente",
            mujer_caminando_frente,
        ),
        (
            "pers-hombre-caminando-espaldas",
            "Hombre caminando de espaldas",
            hombre_caminando_espaldas,
        ),
        (
            "pers-hombre-frente",
            "Hombre de pie de frente",
            hombre_frente,
        ),
        ("pers-mujer-frente", "Mujer de pie de frente", mujer_frente),
        (
            "pers-hombre-espaldas",
            "Hombre de pie de espaldas",
            hombre_espaldas,
        ),
        (
            "pers-mujer-espaldas",
            "Mujer de pie de espaldas",
            mujer_espaldas,
        ),
        ("pers-hombre-lado", "Hombre de pie de lado", hombre_lado),
        (
            "pers-hombre-caminando-lado",
            "Hombre caminando de lado",
            hombre_caminando_lado,
        ),
        (
            "pers-mujer-caminando-lado",
            "Mujer caminando de lado",
            mujer_caminando_lado,
        ),
        (
            "pers-hombre-tres-cuartos",
            "Hombre de pie en 3/4",
            hombre_tres_cuartos,
        ),
        (
            "pers-hombre-telefono",
            "Hombre en 3/4 con teléfono",
            hombre_tres_cuartos_telefono,
        ),
        (
            "pers-mujer-telefono",
            "Mujer de frente con teléfono",
            mujer_frente_telefono,
        ),
        (
            "pers-hombre-leyendo",
            "Hombre de lado leyendo",
            hombre_lado_leyendo,
        ),
        (
            "pers-mujer-leyendo",
            "Mujer en 3/4 leyendo",
            mujer_tres_cuartos_leyendo,
        ),
        (
            "pers-hombre-sentado-frente",
            "Hombre sentado de frente",
            hombre_sentado_frente,
        ),
        (
            "pers-hombre-sentado-lado",
            "Hombre sentado de lado",
            hombre_sentado_lado,
        ),
        (
            "pers-mujer-sentada-lado",
            "Mujer sentada de lado",
            mujer_sentada_lado,
        ),
        (
            "pers-hombre-sentado-tres-cuartos",
            "Hombre sentado en 3/4",
            hombre_sentado_tres_cuartos,
        ),
        (
            "pers-mujer-sentada-tres-cuartos",
            "Mujer sentada en 3/4",
            mujer_sentada_tres_cuartos,
        ),
        ("pers-nino-corriendo", "Niño corriendo", nino_corriendo),
        (
            "pers-pareja-caminando",
            "Pareja caminando",
            pareja_caminando,
        ),
        (
            "pers-silla-de-ruedas",
            "Hombre en silla de ruedas",
            hombre_silla_de_ruedas,
        ),
    ];
    let ciclistas: [Dibujo; 2] = [
        ("pers-ciclista-lado", "Ciclista de lado", ciclista_lado),
        (
            "pers-ciclista-tres-cuartos",
            "Ciclista en 3/4",
            ciclista_tres_cuartos,
        ),
    ];
    let planta: [Dibujo; 9] = [
        ("pers-planta-de-pie", "De pie", planta_de_pie),
        ("pers-planta-caminando-1", "Caminando 1", planta_caminando_1),
        (
            "pers-planta-caminando-2",
            "Caminando 2 (con bolso)",
            planta_caminando_2,
        ),
        ("pers-planta-tumbada", "Tumbada", planta_tumbada),
        (
            "pers-planta-sentada-1",
            "Sentada en una silla",
            planta_sentada_1,
        ),
        (
            "pers-planta-sentada-2",
            "Sentada en el suelo",
            planta_sentada_2,
        ),
        (
            "pers-planta-silla-de-ruedas",
            "En silla de ruedas",
            planta_silla_de_ruedas,
        ),
        ("pers-planta-bicicleta", "En bicicleta", planta_bicicleta),
        ("pers-planta-cochecito", "Con cochecito", planta_cochecito),
    ];
    poner(v, CATEGORIA, &alzado);
    poner(v, CATEGORIA, &ciclistas);
    poner(v, EN_PLANTA, &planta);
}
