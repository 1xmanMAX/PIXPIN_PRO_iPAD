//! **La camara del modelo 3D** (sacada de `ventana3d` para que no
//! dependa de Windows): girar, mover, acercar y la matriz de mundo a
//! recorte. La usan la ventana de Windows (D3D11), el croquis del movil y,
//! en Mac, iPad e iPhone, el visor en Metal: [`Orbita::matriz`] da la
//! matriz por filas (`p * M`) que usa el sombreador de D3D11; el de Metal
//! tiene que usar el mismo convenio (o trasponerla).

pub(crate) const FOV: f64 = std::f64::consts::FRAC_PI_4; // 45 grados

pub(crate) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn suma(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(crate) fn por(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}
pub(crate) fn punto(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn cruz(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub(crate) fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = punto(a, a).sqrt().max(1e-30);
    por(a, 1.0 / l)
}

/// La camara: mira a `objetivo` desde `dist`, girada `rumbo` (alrededor de
/// Z) y `altura` (sobre el horizonte), en radianes. Z es arriba.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orbita {
    pub objetivo: [f64; 3],
    pub rumbo: f64,
    pub altura: f64,
    pub dist: f64,
}

pub(crate) const ALTURA_MAX: f64 = 1.5697; // casi 90 grados: la planta

impl Orbita {
    /// Desde el ojo hacia el objetivo, unitario.
    pub(crate) fn adelante(&self) -> [f64; 3] {
        let (c, s) = (self.altura.cos(), self.altura.sin());
        [-c * self.rumbo.cos(), -c * self.rumbo.sin(), -s]
    }

    pub fn ojo(&self) -> [f64; 3] {
        sub(self.objetivo, por(self.adelante(), self.dist))
    }

    /// Derecha, arriba y adelante de la pantalla.
    pub(crate) fn ejes(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let f = self.adelante();
        let mut r = cruz(f, [0.0, 0.0, 1.0]);
        if punto(r, r) < 1e-12 {
            r = [-self.rumbo.sin(), self.rumbo.cos(), 0.0];
        }
        let r = unit(r);
        let u = cruz(r, f);
        (r, u, f)
    }

    /// El modelo entero (su caja) en la ventana, visto desde el sureste y
    /// algo de arriba.
    pub fn encuadrar(caja: [f32; 6], ancho: u32, alto: u32) -> Orbita {
        let c = [(caja[0] + caja[3]) as f64 / 2.0, (caja[1] + caja[4]) as f64 / 2.0, (caja[2] + caja[5]) as f64 / 2.0];
        let d = [(caja[3] - caja[0]) as f64, (caja[4] - caja[1]) as f64, (caja[5] - caja[2]) as f64];
        let mut o = Orbita { objetivo: c, rumbo: -0.8, altura: 0.5, dist: 1.0 };
        o.dist = o.distancia_para(caja, ancho, alto).max(punto(d, d).sqrt() * 1e-3).max(1e-3);
        o
    }

    /// Lo que hay que alejarse para que las 8 esquinas de la caja quepan.
    pub(crate) fn distancia_para(&self, caja: [f32; 6], ancho: u32, alto: u32) -> f64 {
        let (r, u, f) = self.ejes();
        let ty = (FOV / 2.0).tan() * 0.94;
        let tx = ty * ancho.max(1) as f64 / alto.max(1) as f64;
        let mut d: f64 = 0.0;
        for i in 0..8 {
            let p = [
                caja[if i & 1 == 0 { 0 } else { 3 }] as f64,
                caja[if i & 2 == 0 { 1 } else { 4 }] as f64,
                caja[if i & 4 == 0 { 2 } else { 5 }] as f64,
            ];
            let q = sub(p, self.objetivo);
            let z = punto(q, f);
            d = d.max(punto(q, r).abs() / tx - z).max(punto(q, u).abs() / ty - z);
        }
        d
    }

    /// De mundo (relativo al origen del modelo) a recorte, en filas (p * M).
    pub fn matriz(&self, ancho: u32, alto: u32, radio: f64) -> [[f32; 4]; 4] {
        let (r, u, f) = self.ejes();
        let e = self.ojo();
        let cerca = (self.dist * 0.01).max(radio * 1e-5).max(1e-4);
        let lejos = self.dist + radio * 4.0 + 1.0;
        let ys = 1.0 / (FOV / 2.0).tan();
        let xs = ys * alto.max(1) as f64 / ancho.max(1) as f64;
        let q = lejos / (lejos - cerca);
        // Vista (filas): x = r·(p-e), y = u·(p-e), z = f·(p-e).
        let v = [
            [r[0], u[0], f[0], 0.0],
            [r[1], u[1], f[1], 0.0],
            [r[2], u[2], f[2], 0.0],
            [-punto(r, e), -punto(u, e), -punto(f, e), 1.0],
        ];
        let p = [[xs, 0.0, 0.0, 0.0], [0.0, ys, 0.0, 0.0], [0.0, 0.0, q, 1.0], [0.0, 0.0, -cerca * q, 0.0]];
        let mut m = [[0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                m[i][j] = (0..4).map(|k| v[i][k] * p[k][j]).sum::<f64>() as f32;
            }
        }
        m
    }

    /// La direccion (unitaria) del rayo que sale del ojo por el pixel (x, y).
    pub fn rayo(&self, x: f64, y: f64, ancho: u32, alto: u32) -> [f64; 3] {
        let (r, u, f) = self.ejes();
        let t = (FOV / 2.0).tan();
        let nx = (x / ancho.max(1) as f64 * 2.0 - 1.0) * t * ancho.max(1) as f64 / alto.max(1) as f64;
        let ny = (1.0 - y / alto.max(1) as f64 * 2.0) * t;
        unit(suma(f, suma(por(r, nx), por(u, ny))))
    }

    /// Lo que mide un pixel a la distancia del objetivo.
    pub(crate) fn por_pixel(&self, alto: u32) -> f64 {
        2.0 * self.dist * (FOV / 2.0).tan() / alto.max(1) as f64
    }

    /// Acercar `factor` (<1 acerca) dejando quieto `fijo` en la pantalla.
    pub fn zoom(&self, factor: f64, fijo: [f64; 3]) -> Orbita {
        let factor = factor.clamp(1e-3, 1e3);
        let mut o = *self;
        o.objetivo = suma(fijo, por(sub(self.objetivo, fijo), factor));
        o.dist = (self.dist * factor).max(1e-4);
        o
    }

    /// Girar alrededor de `pivote` (rumbo y altura en radianes).
    pub fn girar(&self, d_rumbo: f64, d_altura: f64, pivote: [f64; 3]) -> Orbita {
        let altura = (self.altura + d_altura).clamp(-ALTURA_MAX, ALTURA_MAX);
        let d_altura = altura - self.altura;
        let ojo = self.ojo();
        // Rumbo: alrededor de la vertical del pivote.
        let rz = |p: [f64; 3]| {
            let q = sub(p, pivote);
            let (c, s) = (d_rumbo.cos(), d_rumbo.sin());
            suma(pivote, [q[0] * c - q[1] * s, q[0] * s + q[1] * c, q[2]])
        };
        let (ojo, obj) = (rz(ojo), rz(self.objetivo));
        let mut o = Orbita { objetivo: obj, rumbo: self.rumbo + d_rumbo, altura: self.altura, dist: self.dist };
        // Altura: alrededor del eje derecho de la camara, por el pivote.
        let (r, _, _) = o.ejes();
        let rot = |p: [f64; 3]| {
            let q = sub(p, pivote);
            let (c, s) = (d_altura.cos(), d_altura.sin());
            // Rodrigues, con el signo que sube el ojo al subir la altura.
            let k = por(r, -1.0);
            let v = suma(suma(por(q, c), por(cruz(k, q), s)), por(k, punto(k, q) * (1.0 - c)));
            suma(pivote, v)
        };
        let (ojo, obj) = (rot(ojo), rot(o.objetivo));
        o.objetivo = obj;
        o.altura = altura;
        // Que el objetivo quede a `dist` delante del ojo nuevo.
        o.objetivo = suma(ojo, por(o.adelante(), self.dist));
        o
    }

    /// Mover la camara `dx`, `dy` pixeles (el modelo sigue al raton).
    pub fn mover(&self, dx: f64, dy: f64, alto: u32) -> Orbita {
        let (r, u, _) = self.ejes();
        let k = self.por_pixel(alto);
        let mut o = *self;
        o.objetivo = suma(self.objetivo, suma(por(r, -dx * k), por(u, dy * k)));
        o
    }

    /// Un paso de la animacion hacia `destino` (`k` de 0 a 1; la distancia
    /// en escala logaritmica). Publica para el visor de Swift.
    pub fn hacia(&self, destino: &Orbita, k: f64) -> Orbita {
        let mut dr = destino.rumbo - self.rumbo;
        while dr > std::f64::consts::PI {
            dr -= std::f64::consts::TAU;
        }
        while dr < -std::f64::consts::PI {
            dr += std::f64::consts::TAU;
        }
        Orbita {
            objetivo: suma(self.objetivo, por(sub(destino.objetivo, self.objetivo), k)),
            rumbo: self.rumbo + dr * k,
            altura: self.altura + (destino.altura - self.altura) * k,
            dist: (self.dist.ln() + (destino.dist.ln() - self.dist.ln()) * k).exp(),
        }
    }

    /// Si ya casi se llego a `o` (para acabar la animacion).
    pub fn cerca_de(&self, o: &Orbita) -> bool {
        let d = sub(self.objetivo, o.objetivo);
        punto(d, d).sqrt() < o.dist * 1e-3
            && (self.rumbo - o.rumbo).abs() < 1e-3
            && (self.altura - o.altura).abs() < 1e-3
            && (self.dist / o.dist - 1.0).abs() < 1e-3
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn proyectar(m: &[[f32; 4]; 4], p: [f64; 3], w: u32, h: u32) -> [f64; 2] {
        let v = [p[0] as f32, p[1] as f32, p[2] as f32, 1.0];
        let c: Vec<f32> = (0..4).map(|j| (0..4).map(|i| v[i] * m[i][j]).sum()).collect();
        [((c[0] / c[3]) as f64 + 1.0) / 2.0 * w as f64, (1.0 - (c[1] / c[3]) as f64) / 2.0 * h as f64]
    }

    #[test]
    fn el_objetivo_cae_en_el_centro_y_la_derecha_a_la_derecha() {
        let o = Orbita { objetivo: [1.0, 2.0, 3.0], rumbo: -0.8, altura: 0.5, dist: 20.0 };
        let m = o.matriz(800, 600, 10.0);
        let c = proyectar(&m, o.objetivo, 800, 600);
        assert!((c[0] - 400.0).abs() < 0.01 && (c[1] - 300.0).abs() < 0.01);
        // Lo que esta mas alto sale mas arriba (y menor en pantalla).
        let arriba = proyectar(&m, [1.0, 2.0, 5.0], 800, 600);
        assert!(arriba[1] < 300.0);
        // Sin espejo: visto desde el sur (rumbo -90°), +X sale a la derecha.
        let s = Orbita { objetivo: [0.0; 3], rumbo: -std::f64::consts::FRAC_PI_2, altura: 0.2, dist: 10.0 };
        let p = proyectar(&s.matriz(800, 600, 10.0), [1.0, 0.0, 0.0], 800, 600);
        assert!(p[0] > 400.0, "{p:?}");
    }

    #[test]
    fn el_zoom_deja_quieto_el_punto_fijo() {
        let o = Orbita { objetivo: [0.0; 3], rumbo: 0.3, altura: 0.4, dist: 50.0 };
        let fijo = [5.0, -3.0, 2.0];
        let antes = proyectar(&o.matriz(800, 600, 30.0), fijo, 800, 600);
        let z = o.zoom(0.5, fijo);
        let despues = proyectar(&z.matriz(800, 600, 30.0), fijo, 800, 600);
        assert!((antes[0] - despues[0]).abs() < 0.05 && (antes[1] - despues[1]).abs() < 0.05);
        assert!((z.dist - 25.0).abs() < 1e-9);
    }

    #[test]
    fn girar_alrededor_de_un_punto_lo_deja_en_su_sitio_de_pantalla_si_es_el_objetivo() {
        let o = Orbita { objetivo: [3.0, 1.0, 0.0], rumbo: 0.3, altura: 0.4, dist: 50.0 };
        let g = o.girar(0.5, 0.2, o.objetivo);
        assert!((g.rumbo - 0.8).abs() < 1e-9 && (g.altura - 0.6).abs() < 1e-9);
        let d = sub(g.objetivo, o.objetivo);
        assert!(punto(d, d).sqrt() < 1e-6, "{:?}", g.objetivo);
        // La altura no pasa de la planta.
        assert!(o.girar(0.0, 5.0, o.objetivo).altura <= ALTURA_MAX);
    }

    #[test]
    fn encuadrar_mete_la_caja_entera() {
        let caja = [0.0, 0.0, 0.0, 40.0, 10.0, 12.0];
        let o = Orbita::encuadrar(caja, 800, 600);
        let m = o.matriz(800, 600, 25.0);
        for i in 0..8 {
            let p = [caja[if i & 1 == 0 { 0 } else { 3 }] as f64, caja[if i & 2 == 0 { 1 } else { 4 }] as f64, caja[if i & 4 == 0 { 2 } else { 5 }] as f64];
            let q = proyectar(&m, p, 800, 600);
            assert!(q[0] >= 0.0 && q[0] <= 800.0 && q[1] >= 0.0 && q[1] <= 600.0, "{q:?}");
        }
    }
}
