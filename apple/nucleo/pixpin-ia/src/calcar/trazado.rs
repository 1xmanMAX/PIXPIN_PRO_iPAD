//! **Trazar como un vectorizador**: de una mascara de pixeles (o de una
//! cadena de puntos de borde) a curvas suaves, sin escaleras.
//!
//! Pedido del usuario (14-oct-2026): «cuando le pido que calque una imagen,
//! las lineas que hace son como, en vez de una curva, escaleras». La causa
//! era triple: los contornos salian a resolucion de pixel (marching squares
//! sobre la mascara binaria, sin interpolar), Ramer-Douglas-Peucker dejaba
//! los peldanos que pasaban de su tolerancia, y la salida eran poligonos de
//! tramos rectos. Aqui, tres pasos hechos a partir de los algoritmos
//! publicados (nada copiado de potrace/autotrace, que son GPL):
//!
//! 1. **Contorno sub-pixel**: la mascara se suaviza con el nucleo binomial
//!    `[1 2 1]/4` (en las dos direcciones) y se recorre con *marching
//!    squares* interpolando linealmente donde el campo cruza 0,5. Un borde
//!    recto en el eje queda exactamente en su sitio (0,25 → 0,75 cruza en el
//!    medio) y un peldano se queda en una ondita de decimas de pixel.
//! 2. **Esquinas**: un punto es esquina si el contorno gira mucho a dos
//!    escalas (2 y 4 px) por igual —en una curva el giro crece con la escala;
//!    en una esquina no—. La esquina se afila otra vez cortando las rectas de
//!    llegada y de salida (el suavizado la achaflanaba medio pixel).
//! 3. **Curvas de Bezier cubicas** entre esquina y esquina con el algoritmo
//!    de Philip J. Schneider («An Algorithm for Automatically Fitting
//!    Digitized Curves», Graphics Gems, 1990): parametro por longitud de
//!    cuerda, minimos cuadrados para la largura de los tiradores, Newton-
//!    Raphson para reparametrizar y, si no cabe en la tolerancia, partir por
//!    el punto peor con la tangente del centro. Un tramo cuyos puntos caben
//!    en la cuerda sale **recto** (un borde recto no se curva).
//!
//! El lienzo pinta un poligono relleno (`pixpin-region`) y lineas de puntos:
//! las Bezier se aplanan con una tolerancia de pantalla ([`aplanar`]), no con
//! un numero fijo de puntos, asi una curva grande lleva los que necesita y
//! una recta lleva dos.

pub type P = (f64, f64);

fn resta(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}
fn suma(a: P, b: P) -> P {
    (a.0 + b.0, a.1 + b.1)
}
fn por(a: P, k: f64) -> P {
    (a.0 * k, a.1 * k)
}
fn punto(a: P, b: P) -> f64 {
    a.0 * b.0 + a.1 * b.1
}
fn largo(a: P) -> f64 {
    punto(a, a).sqrt()
}
fn dist(a: P, b: P) -> f64 {
    largo(resta(a, b))
}
fn unitario(a: P) -> P {
    let l = largo(a);
    if l > 1e-12 {
        por(a, 1.0 / l)
    } else {
        (0.0, 0.0)
    }
}

// ---------------------------------------------------------------------------
// 1. Contorno sub-pixel

/// La mascara suavizada con `[1 2 1]/4` en x y en y, con `borde` pixeles de
/// ceros alrededor (para que todos los anillos se cierren). Devuelve el
/// campo y su ancho y alto.
pub fn campo_suave(mascara: &[bool], w: usize, h: usize, borde: usize) -> (Vec<f32>, usize, usize) {
    let (gw, gh) = (w + 2 * borde, h + 2 * borde);
    let mut a = vec![0f32; gw * gh];
    for y in 0..h {
        for x in 0..w {
            if mascara[y * w + x] {
                a[(y + borde) * gw + x + borde] = 1.0;
            }
        }
    }
    let mut b = vec![0f32; gw * gh];
    for y in 0..gh {
        for x in 1..gw - 1 {
            let i = y * gw + x;
            b[i] = 0.25 * a[i - 1] + 0.5 * a[i] + 0.25 * a[i + 1];
        }
    }
    for y in 1..gh - 1 {
        for x in 0..gw {
            let i = y * gw + x;
            a[i] = 0.25 * b[i - gw] + 0.5 * b[i] + 0.25 * b[i + gw];
        }
    }
    (a, gw, gh)
}

/// Las isolineas de `umbral` de un campo con *marching squares*,
/// interpolando en cada arista. Cada anillo va con lo de dentro (campo >
/// umbral) a su izquierda, mirando con la y hacia abajo: los bordes
/// exteriores salen con [`area`] negativa y los agujeros con positiva. Las
/// coordenadas son las de la rejilla (el nodo (i, j) esta en (i, j)). El
/// borde del campo tiene que estar por debajo del umbral.
pub fn isolineas(campo: &[f32], w: usize, h: usize, umbral: f32) -> Vec<Vec<P>> {
    if w < 2 || h < 2 {
        return Vec::new();
    }
    let horizontales = (w - 1) * h;
    let id_h = |i: usize, j: usize| j * (w - 1) + i;
    let id_v = |i: usize, j: usize| horizontales + j * w + i;
    let n_aristas = horizontales + w * (h - 1);
    const NADA: u32 = u32::MAX;
    let mut siguiente = vec![NADA; n_aristas];
    let v = |i: usize, j: usize| campo[j * w + i];
    for j in 0..h - 1 {
        for i in 0..w - 1 {
            let (a, b, c, d) = (v(i, j), v(i + 1, j), v(i + 1, j + 1), v(i, j + 1));
            let caso = (usize::from(a > umbral) << 3)
                | (usize::from(b > umbral) << 2)
                | (usize::from(c > umbral) << 1)
                | usize::from(d > umbral);
            if caso == 0 || caso == 15 {
                continue;
            }
            let (arriba, abajo, izq, der) =
                (id_h(i, j), id_h(i, j + 1), id_v(i, j), id_v(i + 1, j));
            let mut une = |de: usize, a: usize| siguiente[de] = a as u32;
            let centro_dentro = (a + b + c + d) / 4.0 > umbral;
            match caso {
                1 => une(abajo, izq),
                2 => une(der, abajo),
                3 => une(der, izq),
                4 => une(arriba, der),
                6 => une(arriba, abajo),
                7 => une(arriba, izq),
                8 => une(izq, arriba),
                9 => une(abajo, arriba),
                11 => une(der, arriba),
                12 => une(izq, der),
                13 => une(abajo, der),
                14 => une(izq, abajo),
                // Sillas: decide el centro de la celda.
                5 => {
                    if centro_dentro {
                        une(arriba, izq);
                        une(abajo, der);
                    } else {
                        une(arriba, der);
                        une(abajo, izq);
                    }
                }
                10 => {
                    if centro_dentro {
                        une(der, arriba);
                        une(izq, abajo);
                    } else {
                        une(izq, arriba);
                        une(der, abajo);
                    }
                }
                _ => {}
            }
        }
    }
    // Donde cruza cada arista.
    let posicion = |e: usize| -> P {
        let interp = |v0: f32, v1: f32| -> f64 {
            let d = v1 - v0;
            if d.abs() < 1e-9 {
                0.5
            } else {
                ((umbral - v0) / d).clamp(0.0, 1.0) as f64
            }
        };
        if e < horizontales {
            let (i, j) = (e % (w - 1), e / (w - 1));
            (i as f64 + interp(v(i, j), v(i + 1, j)), j as f64)
        } else {
            let k = e - horizontales;
            let (i, j) = (k % w, k / w);
            (i as f64, j as f64 + interp(v(i, j), v(i, j + 1)))
        }
    };
    let mut anillos = Vec::new();
    for inicio in 0..n_aristas {
        if siguiente[inicio] == NADA {
            continue;
        }
        let mut anillo = Vec::new();
        let mut e = inicio;
        loop {
            let s = siguiente[e];
            if s == NADA {
                break;
            }
            siguiente[e] = NADA;
            anillo.push(posicion(e));
            e = s as usize;
            if e == inicio {
                break;
            }
        }
        // Dos cruces seguidos en el mismo sitio (el campo justo en el
        // umbral) no aportan nada.
        anillo.dedup_by(|a, b| dist(*a, *b) < 1e-9);
        if anillo.len() >= 3 {
            anillos.push(anillo);
        }
    }
    anillos
}

/// Area con signo (formula del cordon).
pub fn area(pol: &[P]) -> f64 {
    let n = pol.len();
    if n < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..n {
        let (a, b) = (pol[i], pol[(i + 1) % n]);
        s += a.0 * b.1 - b.0 * a.1;
    }
    s / 2.0
}

// ---------------------------------------------------------------------------
// 2 y 3. Esquinas y curvas de Bezier

/// Un tramo de la curva ajustada.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tramo {
    Recta(P, P),
    /// Inicio, dos tiradores y fin.
    Cubica([P; 4]),
}

/// Longitud acumulada de una polilinea.
fn acumulada(p: &[P]) -> Vec<f64> {
    let mut s = Vec::with_capacity(p.len());
    let mut l = 0.0;
    for i in 0..p.len() {
        if i > 0 {
            l += dist(p[i - 1], p[i]);
        }
        s.push(l);
    }
    s
}

/// El giro (radianes, 0..π) en el punto `i` mirando a `escala` de
/// distancia hacia atras y hacia delante, y si llega a tanto.
fn giro_a_escala(
    p: &[P],
    s: &[f64],
    total: f64,
    cerrado: bool,
    i: usize,
    escala: f64,
) -> Option<f64> {
    let n = p.len();
    // Hacia atras.
    let mut j = i;
    let mut recorrido = 0.0;
    let atras = loop {
        let k = if j == 0 {
            if !cerrado {
                return None;
            }
            n - 1
        } else {
            j - 1
        };
        recorrido += if k + 1 == j {
            s[j] - s[k]
        } else {
            total - s[k]
        };
        j = k;
        if recorrido >= escala {
            break p[j];
        }
        if j == i {
            return None;
        }
    };
    let mut j = i;
    let mut recorrido = 0.0;
    let delante = loop {
        let k = if j + 1 == n {
            if !cerrado {
                return None;
            }
            0
        } else {
            j + 1
        };
        recorrido += if k == j + 1 {
            s[k] - s[j]
        } else {
            total - s[j]
        };
        j = k;
        if recorrido >= escala {
            break p[j];
        }
        if j == i {
            return None;
        }
    };
    let (u, v) = (resta(p[i], atras), resta(delante, p[i]));
    let (lu, lv) = (largo(u), largo(v));
    if lu < 1e-9 || lv < 1e-9 {
        return None;
    }
    Some((punto(u, v) / (lu * lv)).clamp(-1.0, 1.0).acos())
}

/// Los indices de las esquinas (y lo que giran): giro de mas de ~55° a 4 px
/// y casi el mismo a 2 px (en una curva, a la mitad de escala gira la
/// mitad), o de mas de 100° a 4 px y casi el mismo a 8 px (una esquina
/// aguda, achaflanada mas de 2 px); y el mayor de los de alrededor.
fn esquinas(p: &[P], cerrado: bool) -> Vec<(usize, f64)> {
    const GRANDE: f64 = 4.0;
    const PEQUENA: f64 = 2.0;
    let umbral = 55f64.to_radians();
    let s = acumulada(p);
    let total = s.last().copied().unwrap_or(0.0)
        + if cerrado && p.len() > 1 {
            dist(p[p.len() - 1], p[0])
        } else {
            0.0
        };
    if total < 4.0 * GRANDE {
        return Vec::new();
    }
    let giros: Vec<f64> = (0..p.len())
        .map(|i| {
            let g = giro_a_escala(p, &s, total, cerrado, i, GRANDE).unwrap_or(0.0);
            if g <= umbral {
                return 0.0;
            }
            let c = giro_a_escala(p, &s, total, cerrado, i, PEQUENA).unwrap_or(0.0);
            if c > 0.6 * g {
                return g;
            }
            // Una esquina aguda: el chaflan le come el giro a 2 px, pero a
            // doble escala gira lo mismo (una curva cerrada giraria el doble).
            let doble = giro_a_escala(p, &s, total, cerrado, i, 2.0 * GRANDE).unwrap_or(f64::MAX);
            if g > 100f64.to_radians() && doble < 1.25 * g {
                g
            } else {
                0.0
            }
        })
        .collect();
    let n = p.len();
    let mut salida = Vec::new();
    for i in 0..n {
        if giros[i] == 0.0 {
            continue;
        }
        // Maximo en ±GRANDE de arco (los empates, el primero).
        let mut es_max = true;
        for paso in [1isize, -1] {
            let mut j = i as isize;
            loop {
                j += paso;
                let k = if cerrado {
                    j.rem_euclid(n as isize) as usize
                } else if j < 0 || j >= n as isize {
                    break;
                } else {
                    j as usize
                };
                if k == i {
                    break;
                }
                let d = {
                    let ds = (s[k] - s[i]).abs();
                    if cerrado { ds.min(total - ds) } else { ds }
                };
                if d > GRANDE {
                    break;
                }
                if giros[k] > giros[i] || (giros[k] == giros[i] && paso < 0) {
                    es_max = false;
                    break;
                }
            }
            if !es_max {
                break;
            }
        }
        if es_max {
            salida.push((i, giros[i]));
        }
    }
    salida
}

/// La recta que mejor pasa por unos puntos (punto y direccion unitaria),
/// por minimos cuadrados (eje principal).
fn recta_de(p: &[P]) -> Option<(P, P)> {
    if p.len() < 2 {
        return None;
    }
    let n = p.len() as f64;
    let m = p.iter().fold((0.0, 0.0), |a, q| suma(a, *q));
    let m = por(m, 1.0 / n);
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for q in p {
        let d = resta(*q, m);
        sxx += d.0 * d.0;
        sxy += d.0 * d.1;
        syy += d.1 * d.1;
    }
    let ang = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    Some((m, (ang.cos(), ang.sin())))
}

fn corte(a: (P, P), b: (P, P)) -> Option<P> {
    let (p, r) = a;
    let (q, s) = b;
    let den = r.0 * s.1 - r.1 * s.0;
    if den.abs() < 0.2 {
        // Casi paralelas (giro de menos de ~12°): no hay esquina que afilar.
        return None;
    }
    let t = ((q.0 - p.0) * s.1 - (q.1 - p.1) * s.0) / den;
    Some(suma(p, por(r, t)))
}

/// Los puntos entre `desde` y `hasta` (de arco desde el punto `i`, hacia
/// delante si `paso` es 1 o hacia atras si es -1).
fn puntos_a_distancia(
    p: &[P],
    cerrado: bool,
    i: usize,
    paso: isize,
    desde: f64,
    hasta: f64,
) -> Vec<P> {
    let n = p.len() as isize;
    let mut v = Vec::new();
    let mut j = i as isize;
    let mut d = 0.0;
    let mut previo = p[i];
    loop {
        j += paso;
        if !cerrado && (j < 0 || j >= n) {
            break;
        }
        let q = p[j.rem_euclid(n) as usize];
        d += dist(previo, q);
        previo = q;
        if d > hasta || j.rem_euclid(n) as usize == i {
            break;
        }
        if d >= desde {
            v.push(q);
        }
    }
    v
}

/// **Ajusta** una polilinea (un anillo si `cerrado`) con rectas y cubicas
/// que no se apartan de ella mas de `tol`. Las esquinas se respetan.
pub fn ajustar(p: &[P], cerrado: bool, tol: f64) -> Vec<Tramo> {
    let mut p: Vec<P> = p.to_vec();
    p.dedup_by(|a, b| dist(*a, *b) < 1e-9);
    if cerrado && p.len() > 1 && dist(p[0], p[p.len() - 1]) < 1e-9 {
        p.pop();
    }
    let n = p.len();
    if n < 2 {
        return Vec::new();
    }
    if n == 2 {
        return vec![Tramo::Recta(p[0], p[1])];
    }
    let tol = tol.max(0.05);
    let mut esq = esquinas(&p, cerrado);
    if cerrado && esq.is_empty() {
        // Un anillo sin esquinas: entero desde su primer punto, con la misma
        // tangente al salir y al volver (sin pico donde se cierra).
        let mut t = p.clone();
        t.push(p[0]);
        let c = tangente_centro(&p, true, 0);
        if c == (0.0, 0.0) {
            return Vec::new();
        }
        let mut tramos = Vec::new();
        ajustar_trozo(&t, Some(c), Some(por(c, -1.0)), tol, &mut tramos);
        return tramos;
    }
    // Los extremos de una linea abierta ya parten.
    esq.retain(|&(i, _)| cerrado || (i != 0 && i != n - 1));
    // Cada esquina, afilada, con su zona achaflanada; y los cortes entre
    // trozos (indice, punto, zona).
    let afiladas = esq.iter().map(|&(i, giro)| {
        let zona = zona_de_esquina(giro);
        (i, afilada_de(&p, cerrado, i, zona), zona)
    });
    let cortes: Vec<(usize, P, f64)> = if cerrado {
        afiladas.collect()
    } else {
        std::iter::once((0, p[0], 0.0))
            .chain(afiladas)
            .chain(std::iter::once((n - 1, p[n - 1], 0.0)))
            .collect()
    };
    let pares = if cerrado {
        cortes.len()
    } else {
        cortes.len() - 1
    };
    let mut tramos = Vec::new();
    for k in 0..pares {
        let (ia, xa, za) = cortes[k];
        let (ib, xb, zb) = cortes[(k + 1) % cortes.len()];
        // Los puntos de en medio, sin los de la zona achaflanada.
        let mut t = vec![xa];
        let mut j = ia;
        loop {
            j = (j + 1) % n;
            if j == ib {
                break;
            }
            let q = p[j];
            if dist(q, p[ia]) < za || dist(q, p[ib]) < zb {
                continue;
            }
            t.push(q);
        }
        t.push(xb);
        ajustar_trozo(&t, None, None, tol, &mut tramos);
    }
    tramos
}

/// Hasta donde llega el chaflan que el suavizado deja en una esquina que
/// gira `giro` radianes: medio pixel de redondeo, que en una esquina aguda
/// se aleja del vertice `0,6 / sen(angulo / 2)`.
fn zona_de_esquina(giro: f64) -> f64 {
    let angulo = (std::f64::consts::PI - giro).max(0.2);
    (0.6 / (angulo / 2.0).sin()).clamp(1.2, 4.0)
}

/// La esquina afilada: donde se cortan la recta de llegada y la de salida
/// (sacadas de los puntos de despues del chaflan), si cae cerca.
fn afilada_de(p: &[P], cerrado: bool, i: usize, zona: f64) -> P {
    let antes = puntos_a_distancia(p, cerrado, i, -1, zona, zona + 4.0);
    let despues = puntos_a_distancia(p, cerrado, i, 1, zona, zona + 4.0);
    match (recta_de(&antes), recta_de(&despues)) {
        (Some(a), Some(b)) if antes.len() >= 2 && despues.len() >= 2 => corte(a, b)
            .filter(|x| dist(*x, p[i]) < zona * 1.5 + 0.5)
            .unwrap_or(p[i]),
        _ => p[i],
    }
}

/// Tangente en el punto `i` de un trozo: de la media de los puntos de
/// detras (hasta 2 px) a la de los de delante.
fn tangente_centro(p: &[P], cerrado: bool, i: usize) -> P {
    let atras = puntos_a_distancia(p, cerrado, i, -1, 0.0, 2.5);
    let delante = puntos_a_distancia(p, cerrado, i, 1, 0.0, 2.5);
    let media = |v: &[P]| {
        if v.is_empty() {
            p[i]
        } else {
            por(
                v.iter().fold((0.0, 0.0), |a, q| suma(a, *q)),
                1.0 / v.len() as f64,
            )
        }
    };
    unitario(resta(media(&delante), media(&atras)))
}

/// Tangente al salir de `p[0]` hacia dentro del trozo (media de la
/// direccion a los puntos de los primeros ~3 px).
fn tangente_extremo(p: &[P]) -> P {
    let mut d = (0.0, 0.0);
    let mut l = 0.0;
    for k in 1..p.len() {
        l += dist(p[k - 1], p[k]);
        d = suma(d, unitario(resta(p[k], p[0])));
        if l > 3.0 {
            break;
        }
    }
    unitario(d)
}

/// Un trozo sin esquinas: recto si cabe en la cuerda; si no, Schneider.
fn ajustar_trozo(t: &[P], t1: Option<P>, t2: Option<P>, tol: f64, salida: &mut Vec<Tramo>) {
    let n = t.len();
    if n < 2 {
        return;
    }
    let (a, b) = (t[0], t[n - 1]);
    // Recto: cabe en la cuerda con media tolerancia, o el ruido va a los
    // dos lados de una recta (la de minimos cuadrados) sin combarse.
    let recto = || {
        let cuerda = t.iter().map(|q| dist_a_recta(*q, a, b)).fold(0.0, f64::max);
        cuerda <= tol * 0.5
            || (cuerda <= tol
                && recta_de(t).is_some_and(|(m, d)| {
                    t.iter()
                        .all(|q| dist_a_recta(*q, m, suma(m, d)) <= tol * 0.35)
                }))
    };
    if n == 2 || (t1.is_none() && recto()) {
        if dist(a, b) > 1e-9 {
            salida.push(Tramo::Recta(a, b));
        }
        return;
    }
    let t1 = t1.unwrap_or_else(|| tangente_extremo(t));
    let t2 = t2.unwrap_or_else(|| {
        let r: Vec<P> = t.iter().rev().copied().collect();
        tangente_extremo(&r)
    });
    schneider(t, t1, t2, tol, salida, 0);
}

fn dist_a_recta(p: P, a: P, b: P) -> f64 {
    let d = resta(b, a);
    let l = largo(d);
    if l < 1e-12 {
        return dist(p, a);
    }
    ((p.0 - a.0) * d.1 - (p.1 - a.1) * d.0).abs() / l
}

fn bezier(c: &[P; 4], u: f64) -> P {
    let v = 1.0 - u;
    let (b0, b1, b2, b3) = (v * v * v, 3.0 * v * v * u, 3.0 * v * u * u, u * u * u);
    (
        c[0].0 * b0 + c[1].0 * b1 + c[2].0 * b2 + c[3].0 * b3,
        c[0].1 * b0 + c[1].1 * b1 + c[2].1 * b2 + c[3].1 * b3,
    )
}

/// Parametro por longitud de cuerda.
fn cuerda(t: &[P]) -> Vec<f64> {
    let s = acumulada(t);
    let total = *s.last().unwrap_or(&0.0);
    if total <= 0.0 {
        return vec![0.0; t.len()];
    }
    s.iter().map(|x| x / total).collect()
}

/// La cubica de tangentes dadas que mejor pasa por los puntos (minimos
/// cuadrados para la largura de los dos tiradores).
fn generar(t: &[P], u: &[f64], t1: P, t2: P) -> [P; 4] {
    let (p0, p3) = (t[0], t[t.len() - 1]);
    let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (q, &ui) in t.iter().zip(u) {
        let v = 1.0 - ui;
        let (b0, b1, b2, b3) = (v * v * v, 3.0 * v * v * ui, 3.0 * v * ui * ui, ui * ui * ui);
        let (a1, a2) = (por(t1, b1), por(t2, b2));
        c00 += punto(a1, a1);
        c01 += punto(a1, a2);
        c11 += punto(a2, a2);
        let resto = resta(*q, suma(por(p0, b0 + b1), por(p3, b2 + b3)));
        x0 += punto(a1, resto);
        x1 += punto(a2, resto);
    }
    let det = c00 * c11 - c01 * c01;
    let seg = dist(p0, p3);
    let (mut al, mut ar) = if det.abs() > 1e-12 {
        ((x0 * c11 - x1 * c01) / det, (c00 * x1 - c01 * x0) / det)
    } else {
        (0.0, 0.0)
    };
    // Tiradores al reves o diminutos: la heuristica de Wu y Barsky (un
    // tercio de la cuerda). Larguisimos: se recortan (el error lo vigila).
    let eps = 1e-6 * seg;
    if al < eps || ar < eps {
        al = seg / 3.0;
        ar = seg / 3.0;
    }
    al = al.min(seg * 2.0);
    ar = ar.min(seg * 2.0);
    [p0, suma(p0, por(t1, al)), suma(p3, por(t2, ar)), p3]
}

/// El mayor alejamiento (en pixeles) y donde.
fn error_maximo(t: &[P], c: &[P; 4], u: &[f64]) -> (f64, usize) {
    let mut peor = (0.0, t.len() / 2);
    for i in 1..t.len() - 1 {
        let d = dist(bezier(c, u[i]), t[i]);
        if d > peor.0 {
            peor = (d, i);
        }
    }
    peor
}

/// Un paso de Newton-Raphson por punto para que su parametro sea el del
/// punto de la curva mas cercano.
fn reparametrizar(t: &[P], c: &[P; 4], u: &[f64]) -> Vec<f64> {
    let d1 = [
        por(resta(c[1], c[0]), 3.0),
        por(resta(c[2], c[1]), 3.0),
        por(resta(c[3], c[2]), 3.0),
    ];
    let d2 = [por(resta(d1[1], d1[0]), 2.0), por(resta(d1[2], d1[1]), 2.0)];
    t.iter()
        .zip(u)
        .map(|(q, &ui)| {
            let v = 1.0 - ui;
            let b = bezier(c, ui);
            let b1 = suma(
                suma(por(d1[0], v * v), por(d1[1], 2.0 * v * ui)),
                por(d1[2], ui * ui),
            );
            let b2 = suma(por(d2[0], v), por(d2[1], ui));
            let dif = resta(b, *q);
            let num = punto(dif, b1);
            let den = punto(b1, b1) + punto(dif, b2);
            if den.abs() < 1e-12 {
                ui
            } else {
                (ui - num / den).clamp(0.0, 1.0)
            }
        })
        .collect()
}

fn schneider(t: &[P], t1: P, t2: P, tol: f64, salida: &mut Vec<Tramo>, hondo: usize) {
    let n = t.len();
    if n == 2 {
        let l = dist(t[0], t[1]) / 3.0;
        salida.push(recta_o_cubica(
            [t[0], suma(t[0], por(t1, l)), suma(t[1], por(t2, l)), t[1]],
            tol,
        ));
        return;
    }
    let mut u = cuerda(t);
    let mut c = generar(t, &u, t1, t2);
    let (mut err, mut peor) = error_maximo(t, &c, &u);
    if err <= tol {
        salida.push(recta_o_cubica(c, tol));
        return;
    }
    if err < tol * 4.0 {
        for _ in 0..4 {
            u = reparametrizar(t, &c, &u);
            c = generar(t, &u, t1, t2);
            (err, peor) = error_maximo(t, &c, &u);
            if err <= tol {
                salida.push(recta_o_cubica(c, tol));
                return;
            }
        }
    }
    if hondo > 40 {
        salida.push(recta_o_cubica(c, tol));
        return;
    }
    // Partir por el peor punto, con la tangente del centro.
    let peor = peor.clamp(1, n - 2);
    let mut tc = tangente_centro(t, false, peor);
    if tc == (0.0, 0.0) {
        tc = unitario(resta(t[peor + 1], t[peor - 1]));
    }
    schneider(&t[..=peor], t1, por(tc, -1.0), tol, salida, hondo + 1);
    schneider(&t[peor..], tc, t2, tol, salida, hondo + 1);
}

/// Una cubica con los tiradores en su cuerda es una recta.
fn recta_o_cubica(c: [P; 4], tol: f64) -> Tramo {
    let plano = (tol * 0.1).min(0.05);
    if dist_a_recta(c[1], c[0], c[3]) <= plano && dist_a_recta(c[2], c[0], c[3]) <= plano {
        let d = resta(c[3], c[0]);
        let dentro = |q: P| {
            let k = punto(resta(q, c[0]), d);
            k >= 0.0 && k <= punto(d, d)
        };
        if dentro(c[1]) && dentro(c[2]) {
            return Tramo::Recta(c[0], c[3]);
        }
    }
    Tramo::Cubica(c)
}

/// **Aplana** los tramos a una polilinea que no se aparta de las curvas mas
/// de `plano`: cada cubica en `n` rectas con `n` sacado de su segunda
/// derivada (el alejamiento de la cuerda es como mucho `M / (8 n²)`), cada
/// recta en sus dos extremos. Un anillo (`cerrado`) no repite el primero.
pub fn aplanar(tramos: &[Tramo], cerrado: bool, plano: f64) -> Vec<P> {
    let plano = plano.max(1e-3);
    let mut v: Vec<P> = Vec::new();
    for tr in tramos {
        match tr {
            Tramo::Recta(a, _) => v.push(*a),
            Tramo::Cubica(c) => {
                let dd = |a: P, b: P, cc: P| largo(suma(resta(a, por(b, 2.0)), cc));
                let m = 6.0 * dd(c[0], c[1], c[2]).max(dd(c[1], c[2], c[3]));
                let n = ((m / (8.0 * plano)).sqrt().ceil() as usize).clamp(1, 64);
                for k in 0..n {
                    v.push(bezier(c, k as f64 / n as f64));
                }
            }
        }
    }
    if let Some(ultimo) = tramos.last() {
        let fin = match ultimo {
            Tramo::Recta(_, b) => *b,
            Tramo::Cubica(c) => c[3],
        };
        if !cerrado {
            v.push(fin);
        }
    }
    v.dedup_by(|a, b| dist(*a, *b) < 1e-9);
    if cerrado && v.len() > 1 && dist(v[0], v[v.len() - 1]) < 1e-9 {
        v.pop();
    }
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn disco(w: usize, h: usize, cx: f64, cy: f64, r: f64) -> Vec<bool> {
        (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f64 + 0.5, (i / w) as f64 + 0.5);
                (x - cx).powi(2) + (y - cy).powi(2) <= r * r
            })
            .collect()
    }

    #[test]
    fn el_contorno_de_un_disco_es_subpixel_y_redondo() {
        let (w, h) = (60, 60);
        let m = disco(w, h, 30.0, 30.0, 20.0);
        let (c, gw, gh) = campo_suave(&m, w, h, 2);
        let a = isolineas(&c, gw, gh, 0.5);
        assert_eq!(a.len(), 1);
        // Exterior: area negativa (lo de dentro a la izquierda, y abajo).
        assert!(area(&a[0]) < 0.0, "{}", area(&a[0]));
        // Nodo (i, j) = centro del pixel (i - 2, j - 2): sumar -1,5.
        let err: f64 = a[0]
            .iter()
            .map(|p| ((p.0 - 1.5 - 30.0).powi(2) + (p.1 - 1.5 - 30.0).powi(2)).sqrt() - 20.0)
            .map(f64::abs)
            .fold(0.0, f64::max);
        assert!(err < 0.6, "error del contorno {err}");
        // Nada de peldanos: ningun giro de 90°.
        let n = a[0].len();
        for i in 0..n {
            let (p, q, r) = (a[0][(i + n - 1) % n], a[0][i], a[0][(i + 1) % n]);
            let (u, v) = (resta(q, p), resta(r, q));
            let cos = punto(u, v) / (largo(u) * largo(v));
            assert!(cos > 0.5, "giro brusco en {q:?}");
        }
    }

    #[test]
    fn un_agujero_va_al_reves() {
        let (w, h) = (40, 40);
        let m: Vec<bool> = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f64 + 0.5, (i / w) as f64 + 0.5);
                let d = ((x - 20.0).powi(2) + (y - 20.0).powi(2)).sqrt();
                (6.0..=15.0).contains(&d)
            })
            .collect();
        let (c, gw, gh) = campo_suave(&m, w, h, 2);
        let mut a: Vec<f64> = isolineas(&c, gw, gh, 0.5).iter().map(|x| area(x)).collect();
        a.sort_by(f64::total_cmp);
        assert_eq!(a.len(), 2);
        assert!(a[0] < 0.0 && a[1] > 0.0, "{a:?}");
    }

    #[test]
    fn campo_vacio_o_diminuto_no_da_nada() {
        assert!(isolineas(&[], 0, 0, 0.5).is_empty());
        assert!(isolineas(&[0.0; 4], 2, 2, 0.5).is_empty());
        // Un pixel suelto se queda por debajo de 0,5 al suavizar.
        let (c, gw, gh) = campo_suave(&[true], 1, 1, 2);
        assert!(isolineas(&c, gw, gh, 0.5).is_empty());
        assert!(ajustar(&[], true, 0.5).is_empty());
        assert!(ajustar(&[(1.0, 1.0)], false, 0.5).is_empty());
    }

    #[test]
    fn un_circulo_sale_en_pocas_cubicas_y_fiel() {
        let pts: Vec<P> = (0..200)
            .map(|i| {
                let t = i as f64 / 200.0 * std::f64::consts::TAU;
                (50.0 + 30.0 * t.cos(), 50.0 + 30.0 * t.sin())
            })
            .collect();
        let tr = ajustar(&pts, true, 0.3);
        assert!(tr.len() >= 4 && tr.len() <= 10, "{} tramos", tr.len());
        assert!(
            tr.iter().all(|t| matches!(t, Tramo::Cubica(_))),
            "un circulo sin rectas"
        );
        let v = aplanar(&tr, true, 0.2);
        let e = v
            .iter()
            .map(|p| (dist(*p, (50.0, 50.0)) - 30.0).abs())
            .fold(0.0, f64::max);
        assert!(e < 0.5, "{e}");
    }

    #[test]
    fn un_borde_recto_con_ruido_no_se_curva() {
        // Una recta con dientes de 0,2 px: un solo tramo recto.
        let pts: Vec<P> = (0..80)
            .map(|i| (i as f64, 10.0 + if i % 2 == 0 { 0.2 } else { -0.2 }))
            .collect();
        let tr = ajustar(&pts, false, 0.7);
        assert_eq!(tr.len(), 1, "{tr:?}");
        assert!(matches!(tr[0], Tramo::Recta(..)));
        assert_eq!(aplanar(&tr, false, 0.1).len(), 2);
    }

    #[test]
    fn una_esquina_de_90_grados_queda_esquina() {
        // Un cuadrado de 40 px con las esquinas achaflanadas medio pixel
        // (como sale del suavizado): 4 rectas y las esquinas en su sitio.
        let (w, h) = (60, 60);
        let m: Vec<bool> = (0..w * h)
            .map(|i| (10..50).contains(&(i % w)) && (10..50).contains(&(i / w)))
            .collect();
        let (c, gw, gh) = campo_suave(&m, w, h, 2);
        let a = isolineas(&c, gw, gh, 0.5);
        assert_eq!(a.len(), 1);
        let tr = ajustar(&a[0], true, 0.6);
        assert_eq!(tr.len(), 4, "{tr:?}");
        assert!(tr.iter().all(|t| matches!(t, Tramo::Recta(..))), "{tr:?}");
        let v = aplanar(&tr, true, 0.2);
        assert_eq!(v.len(), 4);
        // En coordenadas de la rejilla, el cuadrado va de 11,5 a 51,5.
        for esquina in [(11.5, 11.5), (51.5, 11.5), (51.5, 51.5), (11.5, 51.5)] {
            let d = v.iter().map(|p| dist(*p, esquina)).fold(f64::MAX, f64::min);
            assert!(d < 0.15, "esquina {esquina:?} a {d}");
        }
    }

    #[test]
    fn una_curva_con_esquina_conserva_la_esquina_y_la_curva() {
        // Media luna: un arco y una cuerda. Dos esquinas donde se juntan.
        let mut pts: Vec<P> = (0..=100)
            .map(|i| {
                let t = std::f64::consts::PI * i as f64 / 100.0;
                (50.0 + 30.0 * t.cos(), 50.0 - 30.0 * t.sin())
            })
            .collect();
        for i in 1..60 {
            pts.push((20.0 + i as f64, 50.0));
        }
        let tr = ajustar(&pts, true, 0.3);
        let rectas = tr.iter().filter(|t| matches!(t, Tramo::Recta(..))).count();
        assert_eq!(rectas, 1, "{tr:?}");
        let v = aplanar(&tr, true, 0.2);
        for e in [(20.0, 50.0), (80.0, 50.0)] {
            let d = v.iter().map(|p| dist(*p, e)).fold(f64::MAX, f64::min);
            assert!(d < 0.2, "{e:?} {d}");
        }
    }

    #[test]
    fn aplanar_respeta_la_tolerancia_de_pantalla() {
        let c = [(0.0, 0.0), (0.0, 100.0), (100.0, 100.0), (100.0, 0.0)];
        let fino = aplanar(&[Tramo::Cubica(c)], false, 0.1);
        let grueso = aplanar(&[Tramo::Cubica(c)], false, 2.0);
        assert!(fino.len() > grueso.len());
        for (pl, plano) in [(&fino, 0.1), (&grueso, 2.0)] {
            for k in 0..=400 {
                let q = bezier(&c, k as f64 / 400.0);
                let d = pl
                    .windows(2)
                    .map(|s| dist_a_segmento(q, s[0], s[1]))
                    .fold(f64::MAX, f64::min);
                assert!(d <= plano * 1.01, "{d} > {plano}");
            }
        }
    }

    fn dist_a_segmento(p: P, a: P, b: P) -> f64 {
        let d = resta(b, a);
        let l2 = punto(d, d);
        let t = if l2 > 0.0 {
            (punto(resta(p, a), d) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        dist(p, suma(a, por(d, t)))
    }
}
