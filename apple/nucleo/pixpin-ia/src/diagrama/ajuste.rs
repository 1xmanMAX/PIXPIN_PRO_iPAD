//! **Rectas y circunferencias por minimos cuadrados**, y el troceo de una
//! cadena de pixeles en tramos rectos y tramos de arco.

use super::mascara::P;

/// Una recta ajustada: un punto, su direccion (unitaria) y lo que se aparta
/// el peor punto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recta {
    pub c: P,
    pub d: P,
    pub error: f64,
}

impl Recta {
    /// Lo que se aparta `p` de la recta (con signo).
    pub fn normal(&self, p: P) -> f64 {
        (p.0 - self.c.0) * -self.d.1 + (p.1 - self.c.1) * self.d.0
    }
    /// Donde cae `p` a lo largo de la recta.
    pub fn t(&self, p: P) -> f64 {
        (p.0 - self.c.0) * self.d.0 + (p.1 - self.c.1) * self.d.1
    }
    pub fn en(&self, t: f64) -> P {
        (self.c.0 + t * self.d.0, self.c.1 + t * self.d.1)
    }
}

/// **La recta de minimo error perpendicular** (componente principal).
pub fn recta(p: &[P]) -> Option<Recta> {
    if p.len() < 2 {
        return None;
    }
    let n = p.len() as f64;
    let (mx, my) = (
        p.iter().map(|q| q.0).sum::<f64>() / n,
        p.iter().map(|q| q.1).sum::<f64>() / n,
    );
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for q in p {
        let (dx, dy) = (q.0 - mx, q.1 - my);
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    let ang = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let d = (ang.cos(), ang.sin());
    let mut r = Recta {
        c: (mx, my),
        d,
        error: 0.0,
    };
    r.error = p.iter().map(|&q| r.normal(q).abs()).fold(0.0, f64::max);
    Some(r)
}

/// Una circunferencia ajustada y lo que se aparta el peor punto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circ {
    pub c: P,
    pub r: f64,
    pub error: f64,
}

impl Circ {
    pub fn residuo(&self, p: P) -> f64 {
        dist(p, self.c) - self.r
    }
    /// El angulo de `p` (en la pantalla: la y baja).
    pub fn angulo(&self, p: P) -> f64 {
        (p.1 - self.c.1).atan2(p.0 - self.c.0)
    }
    pub fn en(&self, a: f64) -> P {
        (self.c.0 + self.r * a.cos(), self.c.1 + self.r * a.sin())
    }
}

pub fn dist(a: P, b: P) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// **La circunferencia de Kasa** (algebraica) refinada con unas vueltas de
/// Gauss-Newton sobre la distancia geometrica.
pub fn circunferencia(p: &[P]) -> Option<Circ> {
    if p.len() < 3 {
        return None;
    }
    let n = p.len() as f64;
    let (mx, my) = (
        p.iter().map(|q| q.0).sum::<f64>() / n,
        p.iter().map(|q| q.1).sum::<f64>() / n,
    );
    // Centrado para que no se pierda precision.
    let (mut suu, mut svv, mut suv, mut suuu, mut svvv, mut suvv, mut svuu) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for q in p {
        let (u, v) = (q.0 - mx, q.1 - my);
        suu += u * u;
        svv += v * v;
        suv += u * v;
        suuu += u * u * u;
        svvv += v * v * v;
        suvv += u * v * v;
        svuu += v * u * u;
    }
    let det = suu * svv - suv * suv;
    if det.abs() < 1e-9 {
        return None;
    }
    let b1 = 0.5 * (suuu + suvv);
    let b2 = 0.5 * (svvv + svuu);
    let uc = (b1 * svv - b2 * suv) / det;
    let vc = (suu * b2 - suv * b1) / det;
    let mut c = (uc + mx, vc + my);
    let mut r = (uc * uc + vc * vc + (suu + svv) / n).sqrt();
    // Gauss-Newton: minimizar sum (|p - c| - r)^2.
    for _ in 0..6 {
        let (mut jtj, mut jtr) = ([[0f64; 3]; 3], [0f64; 3]);
        for q in p {
            let d = dist(*q, c).max(1e-9);
            let j = [-(q.0 - c.0) / d, -(q.1 - c.1) / d, -1.0];
            let res = d - r;
            for a in 0..3 {
                jtr[a] += j[a] * res;
                for b in 0..3 {
                    jtj[a][b] += j[a] * j[b];
                }
            }
        }
        let Some(delta) = resolver3(jtj, jtr) else {
            break;
        };
        c = (c.0 - delta[0], c.1 - delta[1]);
        r -= delta[2];
        if delta.iter().map(|v| v.abs()).fold(0.0, f64::max) < 1e-4 {
            break;
        }
    }
    if !(r.is_finite() && r > 0.0) {
        return None;
    }
    let mut k = Circ { c, r, error: 0.0 };
    k.error = p.iter().map(|&q| k.residuo(q).abs()).fold(0.0, f64::max);
    Some(k)
}

fn resolver3(a: [[f64; 3]; 3], b: [f64; 3]) -> Option<[f64; 3]> {
    let det = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(a);
    if d.abs() < 1e-12 {
        return None;
    }
    let mut x = [0.0; 3];
    for (i, xi) in x.iter_mut().enumerate() {
        let mut m = a;
        for f in 0..3 {
            m[f][i] = b[f];
        }
        *xi = det(m) / d;
    }
    Some(x)
}

/// Un tramo de una cadena.
#[derive(Debug, Clone, PartialEq)]
pub enum Tramo {
    Recto { puntos: Vec<P>, recta: Recta },
    Curvo { puntos: Vec<P>, circ: Circ },
}

impl Tramo {
    pub fn puntos(&self) -> &[P] {
        match self {
            Tramo::Recto { puntos, .. } | Tramo::Curvo { puntos, .. } => puntos,
        }
    }
    pub fn largo(&self) -> f64 {
        largo(self.puntos())
    }
}

pub fn largo(p: &[P]) -> f64 {
    p.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// Lo que se puede apartar un tramo recto (pixeles).
pub const TOL_RECTA: f64 = 1.25;
/// Y uno de arco.
pub const TOL_ARCO: f64 = 1.5;

/// **Trocea una cadena** en tramos rectos y de arco: si toda ella es recta o
/// arco, entera; si no, se parte por el punto mas alejado de su cuerda y se
/// prueba con cada mitad. `radio_max`: un arco mas plano que esto es recta.
pub fn trocear(p: &[P], radio_max: f64, salida: &mut Vec<Tramo>) {
    trocear_hondo(p, radio_max, salida, 0);
}

fn trocear_hondo(p: &[P], radio_max: f64, salida: &mut Vec<Tramo>, hondo: usize) {
    if p.len() < 4 || largo(p) < 4.0 {
        // Lo muy corto no dice nada (el ruido de un cruce).
        if p.len() >= 2
            && largo(p) >= 3.0
            && let Some(r) = recta(p)
        {
            salida.push(Tramo::Recto {
                puntos: p.to_vec(),
                recta: r,
            });
        }
        return;
    }
    if let Some(r) = recta(p)
        && r.error <= TOL_RECTA
    {
        salida.push(Tramo::Recto {
            puntos: p.to_vec(),
            recta: r,
        });
        return;
    }
    let l = largo(p);
    if p.len() >= 8
        && let Some(c) = circunferencia(p)
        && c.error <= TOL_ARCO.max(c.r * 0.012)
        && c.r >= 3.0
        && c.r <= radio_max
        // Que de verdad curve: barrido de al menos ~20 grados.
        && l / c.r >= 0.33
    {
        salida.push(Tramo::Curvo {
            puntos: p.to_vec(),
            circ: c,
        });
        return;
    }
    if hondo > 24 {
        return;
    }
    // Partir por el mas alejado de la cuerda (en una cadena cerrada, por el
    // mas alejado del primero).
    let (a, b) = (p[0], p[p.len() - 1]);
    let cerrada = dist(a, b) < 2.0;
    let mut k = 0;
    let mut dmax = -1.0;
    for (i, &q) in p.iter().enumerate().take(p.len() - 1).skip(1) {
        let d = if cerrada {
            dist(q, a)
        } else {
            dist_a_segmento(q, a, b)
        };
        if d > dmax {
            dmax = d;
            k = i;
        }
    }
    if k == 0 {
        return;
    }
    trocear_hondo(&p[..=k], radio_max, salida, hondo + 1);
    trocear_hondo(&p[k..], radio_max, salida, hondo + 1);
}

pub fn dist_a_segmento(p: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-12 {
        return dist(p, a);
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
    dist(p, (a.0 + t * dx, a.1 + t * dy))
}
