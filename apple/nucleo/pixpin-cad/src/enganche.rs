//! **Enganchar y escribir medidas** (sacado de `ventana` para que
//! no dependa de Windows): los puntos del plano donde se engancha una cota
//! y como se escribe una medida con su unidad. Lo usan la regla, la pagina
//! web y las ventanas de Windows; en Mac, iPad e iPhone, el visor de Swift.

use crate::modelo::Modelo;

/// Los puntos donde se engancha la cota: los vertices de lo dibujado y los
/// centros y extremos de los circulos y arcos. Ordenados por celda para
/// buscar solo cerca del cursor.
pub struct Enganches {
    celda: f64,
    claves: Vec<(u64, [f32; 2])>,
}

fn clave(x: f64, y: f64, celda: f64) -> u64 {
    let cx = ((x / celda).floor() as i64 + (1 << 30)) as u64 & 0xffff_ffff;
    let cy = ((y / celda).floor() as i64 + (1 << 30)) as u64 & 0xffff_ffff;
    cx << 32 | cy
}

impl Enganches {
    #[allow(clippy::approx_constant)]
    pub fn de(m: &Modelo) -> Enganches {
        let lado = ((m.caja[2] - m.caja[0]).max(m.caja[3] - m.caja[1]) as f64).max(1e-6);
        let celda = lado / 256.0;
        let mut puntos: Vec<[f32; 2]> = m.vertices.iter().map(|v| [v.x, v.y]).collect();
        for a in &m.arcos {
            puntos.push(a.centro);
            if a.barrido < 6.28 {
                for ang in [a.inicio, a.inicio + a.barrido] {
                    puntos.push([a.centro[0] + a.radio * ang.cos(), a.centro[1] + a.radio * ang.sin()]);
                }
            } else {
                for k in 0..4 {
                    let ang = k as f32 * std::f32::consts::FRAC_PI_2;
                    puntos.push([a.centro[0] + a.radio * ang.cos(), a.centro[1] + a.radio * ang.sin()]);
                }
            }
        }
        let mut claves: Vec<(u64, [f32; 2])> = puntos.into_iter().map(|p| (clave(p[0] as f64, p[1] as f64, celda), p)).collect();
        claves.sort_unstable_by_key(|c| c.0);
        claves.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
        Enganches { celda, claves }
    }

    /// El punto mas cercano a `p` a menos de `radio` (unidades del plano).
    pub fn cerca(&self, p: [f64; 2], radio: f64) -> Option<[f64; 2]> {
        let n = (radio / self.celda).ceil() as i64;
        if n > 8 {
            return None; // muy de lejos: no vale la pena
        }
        let (cx, cy) = ((p[0] / self.celda).floor() as i64, (p[1] / self.celda).floor() as i64);
        let mut mejor: Option<([f64; 2], f64)> = None;
        for i in cx - n..=cx + n {
            for j in cy - n..=cy + n {
                let k = ((i + (1 << 30)) as u64 & 0xffff_ffff) << 32 | ((j + (1 << 30)) as u64 & 0xffff_ffff);
                let desde = self.claves.partition_point(|c| c.0 < k);
                for (kk, q) in &self.claves[desde..] {
                    if *kk != k {
                        break;
                    }
                    let d = (q[0] as f64 - p[0]).hypot(q[1] as f64 - p[1]);
                    if d <= radio && mejor.is_none_or(|(_, m)| d < m) {
                        mejor = Some(([q[0] as f64, q[1] as f64], d));
                    }
                }
            }
        }
        mejor.map(|(q, _)| q)
    }
}

/// Las unidades del plano para escribir una medida.
pub fn sufijo(unidades: u32) -> &'static str {
    match unidades {
        1 => " in",
        2 => " ft",
        4 => " mm",
        5 => " cm",
        6 => " m",
        7 => " km",
        14 => " dm",
        _ => "",
    }
}

/// Una medida con sus decimales justos y su unidad.
pub fn medida(d: f64, unidades: u32) -> String {
    let dec = if d >= 1000.0 {
        1
    } else if d >= 100.0 {
        2
    } else {
        3
    };
    format!("{:.*}{}", dec, d, sufijo(unidades))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::modelo::Constructor;

    #[test]
    fn la_cota_se_engancha_al_vertice_cercano_y_no_al_lejano() {
        let mut c = Constructor::nuevo();
        c.polilinea(&[[0.0, 0.0], [100.0, 0.0], [100.0, 50.0]], 0, None);
        c.arco([50.0, 25.0], 10.0, 0.0, std::f64::consts::TAU, 0);
        let m = c.terminar();
        let g = Enganches::de(&m);
        let o = m.origen;
        // Cerca de (100,0) del plano (en coordenadas del modelo, relativas).
        let p = g.cerca([100.0 - o[0] + 0.3, 0.0 - o[1] - 0.2], 1.0).unwrap();
        assert!((p[0] - (100.0 - o[0])).abs() < 1e-4);
        // El centro del circulo tambien engancha.
        assert!(g.cerca([50.0 - o[0] + 0.5, 25.0 - o[1]], 1.0).is_some());
        // Caso negativo: lejos de todo, nada.
        assert!(g.cerca([30.0 - o[0], 40.0 - o[1]], 1.0).is_none());
    }

    #[test]
    fn las_medidas_llevan_sus_decimales_y_su_unidad() {
        assert_eq!(medida(12.34567, 6), "12.346 m");
        assert_eq!(medida(123.456, 6), "123.46 m");
        assert_eq!(medida(1234.5, 4), "1234.5 mm");
        assert_eq!(medida(2.0, 0), "2.000");
    }
}
