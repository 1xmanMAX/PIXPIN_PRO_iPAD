//! **Acercar y mover lo que se ve, como en un lienzo** (25-oct-2026, el
//! usuario en la ventana de estudiar una clase: «donde aparece el video,
//! permitir hacer zoom como si fuera un canvas»).
//!
//! Lo puro de un visor de una imagen (un fotograma del video, una captura):
//! encajada por defecto en su caja, la rueda acerca hacia el raton,
//! arrastrar la lleva y el doble clic la vuelve a encajar. Aqui no se pinta
//! nada: quien pinta pide [`Encuadre::destino`] y dibuja la imagen en ese
//! rectangulo (Direct2D la escala en la GPU, sin rehacer pixeles), recortado
//! a la caja.
//!
//! Tres decisiones:
//!
//! - **El aumento cuenta desde «encajada»** (1 = cabe entera), no desde los
//!   pixeles de la imagen: un video de 1920 en una caja de 900 empieza en
//!   1, no en 47 %. El tanto por ciento que se ensena si es el de los
//!   pixeles ([`Encuadre::por_ciento`]), que es lo que dice cuanto detalle
//!   hay.
//! - **No se aleja de «encajada»**: mas pequena que su caja no ensena nada
//!   mas, solo deja marco negro.
//! - **No se pierde de vista**: al moverla, la imagen acercada nunca deja
//!   hueco dentro de la caja por un lado mientras sobra por el otro; en el
//!   eje en que cabe, va centrada.

/// Un rectangulo `(x, y, ancho, alto)` en pixeles de la ventana.
pub type Caja = (f32, f32, f32, f32);

/// Lo mas que se acerca, veces «encajada».
pub const AUMENTO_MAXIMO: f32 = 12.0;
/// Lo que acerca un paso de rueda (120 de `WHEEL_DELTA`) o un boton.
pub const PASO: f32 = 1.25;

/// Como se ve la imagen en su caja: el aumento y lo movida que esta desde
/// el centro, en pixeles de la ventana.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Encuadre {
    pub aumento: f32,
    pub dx: f32,
    pub dy: f32,
}

impl Default for Encuadre {
    fn default() -> Self {
        Encuadre {
            aumento: 1.0,
            dx: 0.0,
            dy: 0.0,
        }
    }
}

/// La imagen `w x h` encajada en `caja` sin deformarla, centrada.
pub fn encajada(caja: Caja, w: f32, h: f32) -> Caja {
    let (x, y, cw, ch) = caja;
    if w <= 0.0 || h <= 0.0 || cw <= 0.0 || ch <= 0.0 {
        return caja;
    }
    let k = (cw / w).min(ch / h);
    let (dw, dh) = (w * k, h * k);
    (x + (cw - dw) / 2.0, y + (ch - dh) / 2.0, dw, dh)
}

/// **El punto de una ventana dentro de ella**: el raton en pixeles de
/// pantalla menos la esquina de la ventana DE VERDAD (la que da Windows en
/// cada vuelta, no la que se pidio al crearla).
pub fn a_la_ventana(raton: (i32, i32), esquina: (i32, i32)) -> (f32, f32) {
    ((raton.0 - esquina.0) as f32, (raton.1 - esquina.1) as f32)
}

impl Encuadre {
    /// Si esta como al abrir: encajada y en el centro.
    pub fn encajado(&self) -> bool {
        (self.aumento - 1.0).abs() < 1e-3 && self.dx.abs() < 0.5 && self.dy.abs() < 0.5
    }

    /// Vuelve a encajarla (el doble clic, el boton de encajar).
    pub fn encajar(&mut self) {
        *self = Encuadre::default();
    }

    /// **Donde se pinta la imagen** `w x h` en `caja`.
    pub fn destino(&self, caja: Caja, w: f32, h: f32) -> Caja {
        let (bx, by, bw, bh) = encajada(caja, w, h);
        let (cx, cy) = (bx + bw / 2.0, by + bh / 2.0);
        let (dw, dh) = (bw * self.aumento, bh * self.aumento);
        (cx - dw / 2.0 + self.dx, cy - dh / 2.0 + self.dy, dw, dh)
    }

    /// El aumento en pixeles de la imagen: 100 es un pixel de la imagen
    /// por pixel de la pantalla.
    pub fn por_ciento(&self, caja: Caja, w: f32, h: f32) -> u32 {
        if w <= 0.0 {
            return 100;
        }
        (self.destino(caja, w, h).2 / w * 100.0).round().max(1.0) as u32
    }

    /// Que no se salga: sin hueco dentro de la caja si la imagen es mas
    /// grande que ella; centrada en el eje en que cabe.
    pub fn acotar(&mut self, caja: Caja, w: f32, h: f32) {
        self.aumento = self.aumento.clamp(1.0, AUMENTO_MAXIMO);
        let (bx, by, bw, bh) = encajada(caja, w, h);
        let (x, y, cw, ch) = caja;
        let holgura = |base: f32, lado_base: f32, borde: f32, lado_caja: f32| {
            let lado = lado_base * self.aumento;
            if lado <= lado_caja {
                return (0.0, 0.0);
            }
            // Lo que se puede correr hacia cada lado sin dejar hueco.
            let centro = base + lado_base / 2.0;
            let min = (borde + lado_caja) - (centro + lado / 2.0);
            let max = borde - (centro - lado / 2.0);
            (min, max)
        };
        let (mx, xx) = holgura(bx, bw, x, cw);
        let (my, xy) = holgura(by, bh, y, ch);
        self.dx = self.dx.clamp(mx.min(xx), xx.max(mx));
        self.dy = self.dy.clamp(my.min(xy), xy.max(my));
    }

    /// **Acerca (`factor` > 1) o aleja hacia `punto`**: lo que hay bajo el
    /// raton se queda bajo el raton.
    pub fn acercar(&mut self, caja: Caja, w: f32, h: f32, factor: f32, punto: (f32, f32)) {
        let antes = self.destino(caja, w, h);
        let nuevo = (self.aumento * factor).clamp(1.0, AUMENTO_MAXIMO);
        if antes.2 <= 0.0 || (nuevo - self.aumento).abs() < 1e-6 {
            self.acotar(caja, w, h);
            return;
        }
        // El punto en fraccion de la imagen, antes de acercar.
        let fx = (punto.0 - antes.0) / antes.2;
        let fy = (punto.1 - antes.1) / antes.3;
        self.aumento = nuevo;
        let despues = self.destino(caja, w, h);
        self.dx += punto.0 - (despues.0 + fx * despues.2);
        self.dy += punto.1 - (despues.1 + fy * despues.3);
        self.acotar(caja, w, h);
    }

    /// Acerca un paso por cada muesca de rueda (`delta` de 120 en 120,
    /// positivo hacia arriba = acercar).
    pub fn rueda(&mut self, caja: Caja, w: f32, h: f32, delta: i32, punto: (f32, f32)) {
        let factor = PASO.powf(delta as f32 / 120.0);
        self.acercar(caja, w, h, factor, punto);
    }

    /// La lleva con el raton (arrastrar).
    pub fn mover(&mut self, caja: Caja, w: f32, h: f32, dx: f32, dy: f32) {
        self.dx += dx;
        self.dy += dy;
        self.acotar(caja, w, h);
    }

    /// **El punto de la imagen** (en sus pixeles) bajo `punto` de la
    /// ventana; `None` fuera de ella.
    pub fn a_la_imagen(&self, caja: Caja, w: f32, h: f32, punto: (f32, f32)) -> Option<(f32, f32)> {
        let (x, y, dw, dh) = self.destino(caja, w, h);
        if dw <= 0.0 || dh <= 0.0 {
            return None;
        }
        let (u, v) = ((punto.0 - x) / dw, (punto.1 - y) / dh);
        ((0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v)).then_some((u * w, v * h))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const CAJA: Caja = (100.0, 50.0, 800.0, 450.0);

    fn cerca(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn al_abrir_cabe_entera_y_centrada() {
        let e = Encuadre::default();
        // Una captura cuadrada en una caja apaisada: alto completo.
        let d = e.destino(CAJA, 1000.0, 1000.0);
        assert_eq!(d, (275.0, 50.0, 450.0, 450.0));
        assert!(e.encajado());
        assert_eq!(e.por_ciento(CAJA, 1000.0, 1000.0), 45);
        // Caso negativo: una imagen vacia se queda con la caja.
        assert_eq!(encajada(CAJA, 0.0, 10.0), CAJA);
    }

    #[test]
    fn la_rueda_acerca_hacia_el_raton() {
        let mut e = Encuadre::default();
        let raton = (300.0, 120.0);
        let antes = e.a_la_imagen(CAJA, 1600.0, 900.0, raton).unwrap();
        e.rueda(CAJA, 1600.0, 900.0, 240, raton);
        assert!(e.aumento > 1.5 && e.aumento < 1.6, "{}", e.aumento);
        let despues = e.a_la_imagen(CAJA, 1600.0, 900.0, raton).unwrap();
        assert!(cerca(antes.0, despues.0) && cerca(antes.1, despues.1));
        // Y alejar del todo vuelve a encajarla.
        e.rueda(CAJA, 1600.0, 900.0, -2400, raton);
        assert!(e.encajado(), "{e:?}");
    }

    #[test]
    fn arrastrar_la_mueve_sin_dejar_hueco() {
        let mut e = Encuadre::default();
        e.acercar(CAJA, 1600.0, 900.0, 2.0, (500.0, 275.0));
        // Llevarla muy a la derecha: su borde izquierdo se queda en el de la
        // caja, no mas alla.
        e.mover(CAJA, 1600.0, 900.0, 5_000.0, 0.0);
        let d = e.destino(CAJA, 1600.0, 900.0);
        assert!(cerca(d.0, CAJA.0), "{d:?}");
        e.mover(CAJA, 1600.0, 900.0, -10_000.0, 0.0);
        let d = e.destino(CAJA, 1600.0, 900.0);
        assert!(cerca(d.0 + d.2, CAJA.0 + CAJA.2), "{d:?}");
        // Caso negativo: encajada no se mueve (no hay nada fuera que ver).
        let mut quieta = Encuadre::default();
        quieta.mover(CAJA, 1600.0, 900.0, 300.0, 200.0);
        assert!(quieta.encajado());
        quieta.encajar();
        assert_eq!(quieta, Encuadre::default());
    }

    #[test]
    fn no_pasa_del_maximo_ni_baja_de_encajada() {
        let mut e = Encuadre::default();
        for _ in 0..100 {
            e.rueda(CAJA, 640.0, 360.0, 120, (500.0, 275.0));
        }
        assert!(cerca(e.aumento, AUMENTO_MAXIMO));
        e.acercar(CAJA, 640.0, 360.0, 0.0001, (500.0, 275.0));
        assert!(cerca(e.aumento, 1.0));
        // Fuera de la imagen no hay punto de la imagen.
        assert_eq!(e.a_la_imagen(CAJA, 640.0, 360.0, (0.0, 0.0)), None);
    }

    #[test]
    fn el_raton_se_cuenta_desde_la_esquina_de_verdad() {
        assert_eq!(a_la_ventana((1250, 700), (1000, 600)), (250.0, 100.0));
        // Caso negativo: con la esquina de antes de moverla, otro sitio.
        assert_ne!(a_la_ventana((1250, 700), (1060, 600)), (250.0, 100.0));
    }
}
