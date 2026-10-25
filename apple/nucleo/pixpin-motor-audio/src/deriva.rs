//! **La deriva de los relojes**: que una hora de reunion siga a la par.
//!
//! Cada tarjeta lleva su propio cristal. El microfono USB y los altavoces
//! del portatil dicen los dos «48 000 Hz», pero uno da 48 007 y el otro
//! 47 995: 0,02 % de diferencia, que en una hora son **siete decimas de
//! segundo**. Sin corregir, al final de la clase la voz del profesor y la
//! del microfono ya no van juntas, y el video (que va con el reloj del
//! sistema) tampoco.
//!
//! La salida del motor va **al paso del reloj del sistema** (QPC): cada
//! segundo de pared son exactamente 48 000 marcos. Cada fuente se
//! remuestrea a esa salida con una relacion que se ajusta sola, en partes
//! por millon, mirando **cuanto hay esperando en su anillo**: si el anillo
//! crece, la tarjeta va rapida y hay que comer un poco mas por cada marco de
//! salida; si mengua, al reves. Mantener el anillo en su nivel es
//! exactamente mantener esa fuente a la par del reloj del sistema, asi que
//! el retraso de cada fuente queda fijo y las dos no se separan.
//!
//! ## El control
//!
//! Un PI sobre el error del anillo **en segundos** (`x`):
//!
//! ```text
//! dx/dt = d - c          d: lo que la tarjeta se aparta de su tasa
//! c = Kp·x + Ki·∫x       c: la correccion
//! ```
//!
//! que en lazo cerrado es `x'' + Kp·x' + Ki·x = 0`: un segundo orden con
//! `ωn = √Ki` y `ζ = Kp / (2·√Ki)`. Con **30 s** de constante y amortiguado
//! critico (`ζ = 1`), una tarjeta un 0,05 % rapida mueve el anillo como
//! mucho medio milisegundo antes de que la integral se la aprenda, y el
//! temblor de la medida (los paquetes llegan a golpes de 10 ms) apenas
//! mueve la relacion: unas decenas de ppm, muy por debajo de lo que se oye
//! (1 ppm de tono son 0,002 centesimas de semitono).
//!
//! La medida se suaviza antes (un segundo) y la correccion no puede cambiar
//! mas de [`PASO_MAXIMO_PPM_S`] por segundo ni pasar de [`TOPE`]: un
//! tropiezo del sistema que vacia o llena el anillo de golpe lo arregla el
//! nucleo tirando o cebando, no un cambio de tono brusco.

/// Constante de tiempo del control, en segundos.
pub const CONSTANTE_S: f64 = 30.0;
/// Lo que se suaviza la medida del anillo, en segundos.
pub const SUAVIZADO_S: f64 = 1.0;
/// Lo mas que se corrige: un 0,5 %. Ningun cristal se aparta tanto; si se
/// llega, algo va mal y es mejor no seguir.
pub const TOPE: f64 = 0.005;
/// Lo mas que cambia la correccion por segundo, en ppm.
pub const PASO_MAXIMO_PPM_S: f64 = 200.0;

/// El control de una fuente.
#[derive(Debug, Clone)]
pub struct Deriva {
    /// Nivel deseado del anillo, en marcos.
    objetivo: f64,
    /// Marcos por segundo de la fuente.
    tasa: f64,
    medida: Option<f64>,
    integral: f64,
    correccion: f64,
    kp: f64,
    ki: f64,
}

impl Deriva {
    pub fn nueva(objetivo_marcos: usize, tasa: u32) -> Deriva {
        let wn = 1.0 / CONSTANTE_S;
        Deriva {
            objetivo: objetivo_marcos as f64,
            tasa: f64::from(tasa.max(1)),
            medida: None,
            integral: 0.0,
            correccion: 0.0,
            kp: 2.0 * wn,
            ki: wn * wn,
        }
    }

    /// La correccion de ahora, en partes por millon (positiva: la tarjeta va
    /// rapida y se come mas deprisa).
    pub fn ppm(&self) -> f64 {
        self.correccion * 1e6
    }

    /// Por lo que hay que multiplicar el paso del remuestreador.
    pub fn factor(&self) -> f64 {
        1.0 + self.correccion
    }

    /// Olvida la medida (el anillo se acaba de cebar o de recortar a mano)
    /// pero **no lo aprendido**: la integral es la deriva de la tarjeta y
    /// sigue valiendo.
    pub fn olvidar_medida(&mut self) {
        self.medida = None;
    }

    /// Una medida del anillo (`ocupado`, en marcos) pasados `dt` segundos
    /// desde la anterior. Devuelve el factor nuevo.
    pub fn observar(&mut self, ocupado: f64, dt: f64) -> f64 {
        let dt = dt.clamp(0.0, 0.5);
        let m = match self.medida {
            None => ocupado,
            Some(m) => m + (ocupado - m) * (1.0 - (-dt / SUAVIZADO_S).exp()),
        };
        self.medida = Some(m);
        let x = (m - self.objetivo) / self.tasa;
        self.integral = (self.integral + self.ki * x * dt).clamp(-TOPE, TOPE);
        let quiere = (self.kp * x + self.integral).clamp(-TOPE, TOPE);
        let paso = PASO_MAXIMO_PPM_S * 1e-6 * dt;
        self.correccion += (quiere - self.correccion).clamp(-paso, paso);
        self.factor()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Simula una fuente con el cristal desviado `ppm` contra un consumidor
    /// que va al paso de la pared, durante `segundos`, con los golpes de
    /// verdad: la tarjeta entrega cada 10 ms y el que mezcla se despierta
    /// cada 15,6 ms (el temporizador por defecto de Windows). Devuelve el
    /// peor apartamiento del anillo respecto a su nivel, en ms, pasado el
    /// primer minuto, y la correccion final.
    fn simular(ppm: f64, segundos: f64) -> (f64, f64) {
        let tasa = 48_000.0;
        let objetivo = 2_400.0; // 50 ms
        let mut d = Deriva::nueva(objetivo as usize, 48_000);
        let real = tasa * (1.0 + ppm * 1e-6);
        let mut producido = objetivo; // se arranca cebado
        let mut consumido = 0.0f64;
        let mut proximo_paquete = 0.0;
        let tick = 0.015_6;
        let mut t = 0.0;
        let mut peor: f64 = 0.0;
        while t < segundos {
            t += tick;
            // Los paquetes que han llegado hasta ahora: 480 marcos de la
            // tarjeta, que a su tasa de verdad duran algo menos o algo mas
            // de 10 ms de pared.
            while proximo_paquete <= t {
                producido += 480.0;
                proximo_paquete += 480.0 / real;
            }
            let ocupado = producido - consumido;
            let f = d.observar(ocupado, tick);
            consumido += tasa * tick * f;
            if t > 60.0 {
                // Incluye el diente de sierra de los paquetes (hasta 10 ms).
                peor = peor.max(((ocupado - objetivo) / tasa * 1000.0).abs());
            }
        }
        (peor, d.ppm())
    }

    #[test]
    fn una_hora_con_el_cristal_rapido_o_lento_no_se_separa() {
        for ppm in [-500.0, -150.0, 0.0, 150.0, 500.0] {
            let (peor, aprendido) = simular(ppm, 3_600.0);
            assert!(peor < 20.0, "{ppm} ppm: se aparto {peor:.1} ms");
            // Y la correccion acaba siendo la deriva de la tarjeta.
            assert!(
                (aprendido - ppm).abs() < 60.0,
                "{ppm} ppm: aprendio {aprendido:.0}"
            );
        }
    }

    #[test]
    fn la_correccion_no_pasa_del_tope_ni_cambia_de_golpe() {
        let mut d = Deriva::nueva(2_400, 48_000);
        let mut antes = d.ppm();
        for _ in 0..10_000 {
            // Un anillo absurdamente lleno: el control empuja, pero despacio.
            d.observar(1.0e9, 0.01);
            assert!((d.ppm() - antes).abs() <= PASO_MAXIMO_PPM_S * 0.01 + 1e-9);
            antes = d.ppm();
        }
        assert!(d.ppm() <= TOPE * 1e6 + 1e-9);
    }

    #[test]
    fn olvidar_la_medida_no_olvida_la_deriva() {
        let mut d = Deriva::nueva(2_400, 48_000);
        for _ in 0..30_000 {
            d.observar(2_500.0, 0.01);
        }
        d.olvidar_medida();
        // Ya en su nivel: lo que queda es lo aprendido, no cero.
        for _ in 0..1_000 {
            d.observar(2_400.0, 0.01);
        }
        assert!(d.ppm() > 500.0, "{}", d.ppm());
    }
}
