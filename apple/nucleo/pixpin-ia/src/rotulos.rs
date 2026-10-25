//! **Colocar rotulos sin que se pisen** (banco del 15-oct-2026): las
//! coordenadas de las raices de una grafica salian una encima de otra
//! («(-1.732, 0)(0, 0)» montados), las fuerzas de un diagrama de fisica
//! encima del bloque y de la flecha, la «m1 = 3 kg» fuera de su caja.
//!
//! Una sola funcion para todo lo que dibuja el asistente (graficas de
//! funciones, fisica, geometria): cada rotulo tiene su **ancla** (el punto
//! al que se refiere) y su medida; se prueban sitios alrededor del ancla,
//! de mas cerca a mas lejos, y se queda el primero que no pisa ni otro
//! rotulo ya puesto ni lo `ocupado` (las curvas, los ejes, las figuras, en
//! cajas). Si ninguno queda libre, el que menos pisa. Sin estado, sin
//! pantalla: geometria pura, que es lo que se prueba.

/// Un rotulo a colocar: el punto al que se refiere y lo que mide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rotulo {
    pub ancla: (f64, f64),
    pub ancho: f64,
    pub alto: f64,
    /// El sitio que se prueba primero (su esquina), si lo hay: el de
    /// siempre de quien llama (la punta de una flecha, hacia fuera).
    pub preferida: Option<(f64, f64)>,
}

impl Rotulo {
    pub fn en(ancla: (f64, f64), ancho: f64, alto: f64) -> Rotulo {
        Rotulo { ancla, ancho, alto, preferida: None }
    }
}

/// Una caja `[x, y, ancho, alto]`.
pub type Caja = [f64; 4];

/// Cuanto se pisan dos cajas (area).
pub fn solape(a: &Caja, b: &Caja) -> f64 {
    let w = (a[0] + a[2]).min(b[0] + b[2]) - a[0].max(b[0]);
    let h = (a[1] + a[3]).min(b[1] + b[3]) - a[1].max(b[1]);
    if w > 0.0 && h > 0.0 { w * h } else { 0.0 }
}

/// Las cajas pequenas que tapan una polilinea (para que un rotulo no se
/// ponga encima de una curva o de una flecha): una por tramo, de `grosor`.
pub fn cajas_de_linea(puntos: &[(f64, f64)], grosor: f64) -> Vec<Caja> {
    let mut v = Vec::new();
    for t in puntos.windows(2) {
        let (a, b) = (t[0], t[1]);
        let largo = (b.0 - a.0).hypot(b.1 - a.1);
        // Tramos largos (una recta de dos puntos): troceados, que su caja
        // entera taparia media grafica.
        let n = (largo / (grosor * 2.0).max(4.0)).ceil().max(1.0) as usize;
        for k in 0..n {
            let (s0, s1) = (k as f64 / n as f64, (k + 1) as f64 / n as f64);
            let p = (a.0 + (b.0 - a.0) * s0, a.1 + (b.1 - a.1) * s0);
            let q = (a.0 + (b.0 - a.0) * s1, a.1 + (b.1 - a.1) * s1);
            let (x0, y0) = (p.0.min(q.0) - grosor / 2.0, p.1.min(q.1) - grosor / 2.0);
            v.push([x0, y0, (p.0 - q.0).abs() + grosor, (p.1 - q.1).abs() + grosor]);
        }
    }
    v
}

/// Los sitios que se prueban para un rotulo, en orden de preferencia: arriba
/// a la derecha (lo de siempre en una pizarra), y alrededor, cada vez mas
/// lejos.
fn candidatos(r: &Rotulo) -> Vec<Caja> {
    let (ax, ay) = r.ancla;
    let (w, h) = (r.ancho, r.alto);
    let mut v: Vec<Caja> = r.preferida.map(|(x, y)| [x, y, w, h]).into_iter().collect();
    for d in [8.0, 16.0, 28.0, 44.0, 66.0] {
        let k = d * 0.7;
        v.extend([
            [ax + k, ay - k - h, w, h],          // arriba a la derecha
            [ax - k - w, ay - k - h, w, h],      // arriba a la izquierda
            [ax + k, ay + k, w, h],              // abajo a la derecha
            [ax - k - w, ay + k, w, h],          // abajo a la izquierda
            [ax + d, ay - h / 2.0, w, h],        // a la derecha
            [ax - d - w, ay - h / 2.0, w, h],    // a la izquierda
            [ax - w / 2.0, ay - d - h, w, h],    // encima
            [ax - w / 2.0, ay + d, w, h],        // debajo
        ]);
    }
    v
}

/// **La esquina de arriba a la izquierda de cada rotulo**, en el orden en
/// que llegan (los primeros eligen antes). `dentro`: si se da, los sitios
/// que se salen de esa caja se dejan para el final.
pub fn colocar(rotulos: &[Rotulo], ocupado: &[Caja], dentro: Option<Caja>) -> Vec<(f64, f64)> {
    let mut puestos: Vec<Caja> = Vec::new();
    // Los anclas de todos tapan un poco: un rotulo no se pone encima del
    // punto de otro.
    let anclas: Vec<Caja> = rotulos.iter().map(|r| [r.ancla.0 - 5.0, r.ancla.1 - 5.0, 10.0, 10.0]).collect();
    let mut salida = Vec::with_capacity(rotulos.len());
    for r in rotulos {
        let mut mejor: Option<(f64, Caja)> = None;
        for c in candidatos(r) {
            let fuera = dentro.is_some_and(|d| c[0] < d[0] || c[1] < d[1] || c[0] + c[2] > d[0] + d[2] || c[1] + c[3] > d[1] + d[3]);
            let pisa: f64 = puestos.iter().chain(&anclas).chain(ocupado).map(|o| solape(&c, o)).sum::<f64>()
                + if fuera { c[2] * c[3] } else { 0.0 };
            if pisa <= 1e-6 {
                mejor = Some((0.0, c));
                break;
            }
            if mejor.is_none_or(|(m, _)| pisa < m) {
                mejor = Some((pisa, c));
            }
        }
        let c = mejor.map_or([r.ancla.0, r.ancla.1, r.ancho, r.alto], |(_, c)| c);
        puestos.push(c);
        salida.push((c[0], c[1]));
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn rotulos_juntos_no_se_pisan() {
        // Tres raices pegadas en el eje x (como x^3 - 3x con zoom).
        let rs: Vec<Rotulo> = [0.0, 30.0, 60.0].iter().map(|&x| Rotulo::en((x, 100.0), 70.0, 16.0)).collect();
        let eje = cajas_de_linea(&[(-100.0, 100.0), (200.0, 100.0)], 2.0);
        let p = colocar(&rs, &eje, None);
        let cajas: Vec<Caja> = p.iter().zip(&rs).map(|(&(x, y), r)| [x, y, r.ancho, r.alto]).collect();
        for i in 0..cajas.len() {
            for j in i + 1..cajas.len() {
                assert_eq!(solape(&cajas[i], &cajas[j]), 0.0, "{:?} {:?}", cajas[i], cajas[j]);
            }
            for e in &eje {
                assert_eq!(solape(&cajas[i], e), 0.0, "pisa el eje: {:?}", cajas[i]);
            }
        }
    }

    #[test]
    fn sin_estorbos_va_arriba_a_la_derecha_y_cerca() {
        let r = Rotulo::en((50.0, 50.0), 20.0, 10.0);
        let (x, y) = colocar(&[r], &[], None)[0];
        assert!(x > 50.0 && y + 10.0 < 50.0 && x < 60.0, "{x},{y}");
    }

    #[test]
    fn respeta_la_caja_de_dentro() {
        let r = Rotulo::en((95.0, 5.0), 30.0, 10.0);
        let (x, y) = colocar(&[r], &[], Some([0.0, 0.0, 100.0, 100.0]))[0];
        assert!(x >= 0.0 && y >= 0.0 && x + 30.0 <= 100.0, "{x},{y}");
    }
}
