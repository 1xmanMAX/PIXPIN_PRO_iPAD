//! **Las cuentas de las graficas, en Rust**: raices, extremos, polos,
//! asintotas, intersecciones, areas y derivadas de una funcion `f(x)`.
//!
//! Regla del usuario: los calculos nunca los hace el modelo. El modelo pide
//! «marca las raices de x^2 - 4» y aqui se buscan muestreando el dominio y
//! afinando cada cambio de signo por biseccion; un cambio de signo donde la
//! funcion se dispara (el de `1/x` en el cero) no es una raiz sino un polo,
//! y eso es justo lo que da la asintota vertical.
//!
//! Todo es numerico y robusto antes que exacto: tolerancias relativas a la
//! escala de la funcion, nada de simbolos.

/// Cuantas muestras para buscar.
pub const MUESTRAS: usize = 4000;

fn linspace(a: f64, b: f64, n: usize) -> impl Iterator<Item = f64> {
    (0..=n).map(move |i| a + (b - a) * i as f64 / n as f64)
}

/// Biseccion de un cambio de signo de `g` en `[a, b]`.
fn biseccion(g: &dyn Fn(f64) -> f64, mut a: f64, mut b: f64) -> f64 {
    let mut ga = g(a);
    for _ in 0..90 {
        let m = 0.5 * (a + b);
        let gm = g(m);
        if !gm.is_finite() {
            // Un hueco en medio: se acerca por el lado que existe.
            b = m;
            continue;
        }
        if gm == 0.0 {
            return m;
        }
        if (gm > 0.0) == (ga > 0.0) {
            a = m;
            ga = gm;
        } else {
            b = m;
        }
    }
    0.5 * (a + b)
}

/// La escala tipica de `f` en `[a, b]` (la mediana de |f|), para que las
/// tolerancias no dependan de las unidades.
pub fn escala(f: &dyn Fn(f64) -> f64, a: f64, b: f64) -> f64 {
    let mut v: Vec<f64> = linspace(a, b, 400).map(f).filter(|y| y.is_finite()).map(f64::abs).collect();
    if v.is_empty() {
        return 1.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2].max(1e-9)
}

fn sin_repetir(v: &mut Vec<f64>, cerca: f64) {
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() <= cerca);
}

/// La derivada numerica (diferencia centrada).
pub fn derivada(f: &dyn Fn(f64) -> f64, x: f64) -> f64 {
    let h = 1e-5 * (1.0 + x.abs());
    (f(x + h) - f(x - h)) / (2.0 * h)
}

/// Si `f` es derivable en `x`: las pendientes por la izquierda y por la
/// derecha coinciden.
pub fn derivable(f: &dyn Fn(f64) -> f64, x: f64) -> bool {
    let h = 1e-4 * (1.0 + x.abs());
    let izq = (f(x) - f(x - h)) / h;
    let der = (f(x + h) - f(x)) / h;
    izq.is_finite() && der.is_finite() && (izq - der).abs() <= 1e-2 * (1.0 + izq.abs().max(der.abs()))
}

/// **Los ceros de `f` en `[a, b]`**, y aparte los «cambios de signo» que
/// resultaron ser polos (donde `f` se dispara).
pub fn ceros(f: &dyn Fn(f64) -> f64, a: f64, b: f64) -> (Vec<f64>, Vec<f64>) {
    let s = escala(f, a, b);
    let tol = 1e-6 * (1.0 + s);
    let xs: Vec<f64> = linspace(a, b, MUESTRAS).collect();
    let ys: Vec<f64> = xs.iter().map(|&x| f(x)).collect();
    let (mut raices, mut polos) = (Vec::new(), Vec::new());
    for i in 0..xs.len() {
        if ys[i] == 0.0 {
            raices.push(xs[i]);
            continue;
        }
        if i + 1 < xs.len() && ys[i].is_finite() && ys[i + 1].is_finite() && ys[i + 1] != 0.0 && (ys[i] > 0.0) != (ys[i + 1] > 0.0) {
            let r = biseccion(f, xs[i], xs[i + 1]);
            let fr = f(r);
            if fr.is_finite() && fr.abs() <= tol.max(1e-9) {
                raices.push(r);
            } else if !fr.is_finite() || fr.abs() > 1e3 * (1.0 + s) {
                polos.push(r);
            }
        }
    }
    // Los ceros que tocan sin cruzar (x^2 en el 0): un extremo sobre el eje.
    for (x, _) in extremos(f, a, b) {
        if f(x).abs() <= tol.max(1e-9) * 10.0 {
            raices.push(x);
        }
    }
    let cerca = (b - a) * 1e-6;
    sin_repetir(&mut raices, cerca.max(1e-9));
    sin_repetir(&mut polos, cerca.max(1e-9));
    (raices.into_iter().map(limpio).collect(), polos.into_iter().map(limpio).collect())
}

/// Un valor casi entero, entero (las cuentas de biseccion dejan 1.9999999).
pub fn limpio(v: f64) -> f64 {
    let r = v.round();
    if (v - r).abs() < 1e-7 * (1.0 + v.abs()) { r } else { v }
}

/// **Los extremos locales** de `f` en `[a, b]`: `(x, es_maximo)`.
pub fn extremos(f: &dyn Fn(f64) -> f64, a: f64, b: f64) -> Vec<(f64, bool)> {
    let g = |x: f64| derivada(f, x);
    let n = MUESTRAS / 2;
    let xs: Vec<f64> = linspace(a, b, n).collect();
    let gs: Vec<f64> = xs.iter().map(|&x| g(x)).collect();
    let s = escala(f, a, b);
    let mut v: Vec<(f64, bool)> = Vec::new();
    for i in 0..n {
        let (g0, g1) = (gs[i], gs[i + 1]);
        if !g0.is_finite() || !g1.is_finite() || g0 == 0.0 && g1 == 0.0 {
            continue;
        }
        if (g0 > 0.0) != (g1 > 0.0) || g1 == 0.0 {
            let r = if g1 == 0.0 { xs[i + 1] } else { biseccion(&g, xs[i], xs[i + 1]) };
            let fr = f(r);
            let paso = (b - a) / n as f64;
            // Que exista alrededor y no sea un polo.
            if !fr.is_finite() || !f(r - paso).is_finite() || !f(r + paso).is_finite() || fr.abs() > 1e4 * (1.0 + s) {
                continue;
            }
            let maximo = f(r - paso) <= fr && f(r + paso) <= fr;
            let minimo = f(r - paso) >= fr && f(r + paso) >= fr;
            if maximo != minimo {
                v.push((limpio(r), maximo));
            }
        }
    }
    v.dedup_by(|a, b| (a.0 - b.0).abs() <= (b.0.abs() + 1.0) * 1e-6);
    v
}

/// **Los polos** (asintotas verticales) de `f` en `[a, b]`: los cambios de
/// signo que se disparan y los picos de |f| que no acaban (`1/x^2`).
pub fn polos(f: &dyn Fn(f64) -> f64, a: f64, b: f64) -> Vec<f64> {
    let (_, mut v) = ceros(f, a, b);
    let s = escala(f, a, b);
    let n = MUESTRAS;
    let xs: Vec<f64> = linspace(a, b, n).collect();
    let ys: Vec<f64> = xs.iter().map(|&x| f(x).abs()).collect();
    let grande = 1e3 * (1.0 + s);
    for i in 1..n {
        let y = ys[i];
        let pico = if y.is_finite() {
            y > grande && y >= ys[i - 1] && y >= ys[i + 1]
        } else {
            // Un hueco entre dos valores enormes: el centro de un polo.
            y.is_infinite() || ys[i - 1].is_finite() && ys[i + 1].is_finite() && ys[i - 1] > grande && ys[i + 1] > grande
        };
        if !pico {
            continue;
        }
        // Busqueda ternaria del maximo de |f|.
        let (mut lo, mut hi) = (xs[i - 1], xs[i + 1]);
        for _ in 0..80 {
            let m1 = lo + (hi - lo) / 3.0;
            let m2 = hi - (hi - lo) / 3.0;
            let (f1, f2) = (f(m1).abs(), f(m2).abs());
            let (f1, f2) = (if f1.is_finite() { f1 } else { f64::MAX }, if f2.is_finite() { f2 } else { f64::MAX });
            if f1 < f2 {
                lo = m1;
            } else {
                hi = m2;
            }
        }
        let x = 0.5 * (lo + hi);
        let fx = f(x).abs();
        if !fx.is_finite() || fx > 1e6 * (1.0 + s) {
            v.push(limpio(x));
        }
    }
    sin_repetir(&mut v, ((b - a) * 1e-5).max(1e-9));
    v
}

/// Una asintota horizontal u oblicua hacia `lado` (+1 o -1): `(m, b)` de
/// `y = m x + b` (m = 0 es horizontal).
pub fn asintota_al_infinito(f: &dyn Fn(f64) -> f64, lado: f64) -> Option<(f64, f64)> {
    let (l1, l2, l3) = (lado * 1e5, lado * 1e6, lado * 1e7);
    let (f1, f2, f3) = (f(l1), f(l2), f(l3));
    if !(f1.is_finite() && f2.is_finite() && f3.is_finite()) {
        return None;
    }
    let m = (f3 - f2) / (l3 - l2);
    let b = f3 - m * l3;
    let pred = m * l1 + b;
    if (pred - f1).abs() > 1e-3 * (1.0 + f1.abs().min(1e9)) {
        return None;
    }
    let m = if m.abs() < 1e-6 { 0.0 } else { limpio((m * 1e4).round() / 1e4) };
    let b = if m == 0.0 { f3 } else { b };
    let b = limpio((b * 1e4).round() / 1e4);
    Some((m, if b == 0.0 { 0.0 } else { b }))
}

/// Si `f` es (numericamente) la recta `y = m x + b` en `[a, b]`.
pub fn es_la_recta(f: &dyn Fn(f64) -> f64, (m, q): (f64, f64), a: f64, b: f64) -> bool {
    linspace(a, b, 50).all(|x| {
        let y = f(x);
        !y.is_finite() || (y - (m * x + q)).abs() < 1e-6 * (1.0 + y.abs())
    })
}

/// **El area entre `f` y `g`** de `a` a `b` (Simpson sobre |f - g|), o
/// `None` si no es finita (un polo en medio).
pub fn area_entre(f: &dyn Fn(f64) -> f64, g: &dyn Fn(f64) -> f64, a: f64, b: f64) -> Option<f64> {
    let n = 4000;
    let h = (b - a) / n as f64;
    let mut s = 0.0;
    for i in 0..=n {
        let x = a + h * i as f64;
        let d = (f(x) - g(x)).abs();
        if !d.is_finite() {
            return None;
        }
        let peso = if i == 0 || i == n { 1.0 } else if i % 2 == 1 { 4.0 } else { 2.0 };
        s += peso * d;
    }
    let area = s * h / 3.0;
    // Un polo entre muestras deja un area enorme pero finita.
    (area.is_finite() && area < 1e9).then_some(area)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn raices_de_una_parabola_y_la_que_toca() {
        let (r, p) = ceros(&|x| x * x - 4.0, -5.0, 5.0);
        assert_eq!(r, vec![-2.0, 2.0]);
        assert!(p.is_empty());
        let (r, _) = ceros(&|x| x * x, -3.0, 3.0);
        assert_eq!(r, vec![0.0], "la que toca sin cruzar");
    }

    #[test]
    fn un_polo_no_es_una_raiz() {
        let (r, p) = ceros(&|x| 1.0 / x, -4.0, 4.1);
        assert!(r.is_empty(), "{r:?}");
        assert_eq!(p.len(), 1);
        assert!(p[0].abs() < 1e-6);
        assert_eq!(polos(&|x| 1.0 / x, -4.0, 4.0), vec![0.0], "con el cero en una muestra");
        let v = polos(&|x| 1.0 / (x * x), -3.0, 3.0);
        assert_eq!(v.len(), 1, "{v:?}");
        let v = polos(&|x: f64| x.tan(), -3.0, 3.0);
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(polos(&|x| x * x, -3.0, 3.0).is_empty());
        assert!(polos(&|x: f64| x.sqrt(), -3.0, 3.0).is_empty(), "un hueco no es un polo");
    }

    #[test]
    fn extremos_con_su_clase() {
        let e = extremos(&|x| x * x * x - 3.0 * x, -3.0, 3.0);
        assert_eq!(e, vec![(-1.0, true), (1.0, false)]);
        assert!(extremos(&|x| 1.0 / (x * x), -3.0, 3.0).is_empty(), "el polo no es un maximo");
    }

    #[test]
    fn asintotas_y_area() {
        assert_eq!(asintota_al_infinito(&|x| (2.0 * x + 1.0) / (x - 1.0), 1.0), Some((0.0, 2.0)));
        assert_eq!(asintota_al_infinito(&|x| x + 1.0 / x, 1.0), Some((1.0, 0.0)));
        assert_eq!(asintota_al_infinito(&|x| x * x, 1.0), None);
        assert!(es_la_recta(&|x| 2.0 * x + 1.0, (2.0, 1.0), -5.0, 5.0));
        let a = area_entre(&|x| x + 2.0, &|x| x * x, -1.0, 2.0).unwrap();
        assert!((a - 4.5).abs() < 1e-6, "{a}");
        assert!(area_entre(&|x| 1.0 / x, &|_| 0.0, -1.0, 1.0).is_none());
    }

    #[test]
    fn derivada_y_derivable() {
        assert!((derivada(&|x| x * x, 3.0) - 6.0).abs() < 1e-6);
        assert!(derivable(&|x| x * x, 1.0));
        assert!(!derivable(&|x: f64| x.abs(), 0.0));
    }
}
