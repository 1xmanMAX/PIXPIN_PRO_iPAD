//! **El tratamiento del sonido**, en coma flotante y sin reservar memoria.
//!
//! Todo trabaja en su sitio sobre bloques intercalados de 1 o 2 canales, ya
//! a la tasa de la salida. El orden de la cadena lo pone el nucleo:
//!
//! ```text
//! microfono: paso alto 80 Hz → puerta → realce de la voz → ganancia ┐
//! equipo:                                                ganancia ──┴→ suma → limitador → i16
//! ```
//!
//! El **realce de la voz** es el de siempre (`pixpin-audio`, 2-oct: «lo capta
//! muy bajo de volumen»): un control automatico de ganancia que lleva los
//! picos de la voz a -3 dBFS, entre 0 y +24 dB, sin subir el silencio. Va
//! **solo al microfono**: el sonido del equipo ya sale a su volumen y
//! realzarlo bombearia la musica de un video.
//!
//! El **limitador** va al final, sobre la suma: dos voces altas a la vez (la
//! del usuario y la de la reunion) no pueden recortarse en seco, que es lo
//! que suena a «roto». Mira un milisegundo por delante para bajar la
//! ganancia antes de que llegue el golpe, y por si aun asi algo se pasa, una
//! curva suave lo dobla por debajo del techo. **Nunca** sale nada por encima
//! de la escala completa.

/// Por debajo de esto un estado de filtro se pone a cero: una cola que se
/// apaga acaba en numeros desnormales, que en x86 son cien veces mas lentos.
const DESNORMAL: f32 = 1.0e-15;

/// El coeficiente de un suavizado de un polo con constante de tiempo
/// `segundos` a `tasa` muestras por segundo.
pub fn coeficiente(segundos: f32, tasa: u32) -> f32 {
    let n = segundos * tasa.max(1) as f32;
    if n <= 0.0 {
        return 1.0;
    }
    1.0 - (-1.0 / n).exp()
}

/// De decibelios a veces.
pub fn de_db(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// **Paso alto** de Butterworth (Q = 1/√2) del «Audio EQ Cookbook»: quita el
/// retumbe de la mesa, el zumbido grave y la continua de algunos microfonos
/// USB, que si no se comeria margen del realce.
#[derive(Debug, Clone)]
pub struct PasoAlto {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    /// `[x1, x2, y1, y2]` de cada canal.
    estado: [[f32; 4]; 2],
}

impl PasoAlto {
    pub fn nuevo(corte_hz: f32, tasa: u32) -> PasoAlto {
        let fs = tasa.max(1) as f32;
        let w0 = 2.0 * std::f32::consts::PI * (corte_hz / fs).min(0.49);
        let (seno, coseno) = w0.sin_cos();
        let alfa = seno / (2.0 * std::f32::consts::FRAC_1_SQRT_2);
        let a0 = 1.0 + alfa;
        PasoAlto {
            b0: (1.0 + coseno) / 2.0 / a0,
            b1: -(1.0 + coseno) / a0,
            b2: (1.0 + coseno) / 2.0 / a0,
            a1: -2.0 * coseno / a0,
            a2: (1.0 - alfa) / a0,
            estado: [[0.0; 4]; 2],
        }
    }

    pub fn procesar(&mut self, bloque: &mut [f32], canales: usize) {
        let canales = canales.clamp(1, 2);
        for marco in bloque.chunks_exact_mut(canales) {
            for (x, e) in marco.iter_mut().zip(self.estado.iter_mut()) {
                let mut y = self.b0 * *x + self.b1 * e[0] + self.b2 * e[1]
                    - self.a1 * e[2]
                    - self.a2 * e[3];
                if y.abs() < DESNORMAL {
                    y = 0.0;
                }
                e[1] = e[0];
                e[0] = *x;
                e[3] = e[2];
                e[2] = y;
                *x = y;
            }
        }
    }
}

/// **Puerta de ruido** del microfono: por debajo de un umbral (el ventilador,
/// el teclado lejano) baja el microfono a -30 dB; no a cero, que el corte
/// entre palabras se oiria como un hueco.
///
/// Con histeresis (abre a -50 dBFS, cierra a -56) y una espera de 150 ms
/// antes de cerrar: si no, el final de cada palabra se cortaria.
#[derive(Debug, Clone)]
pub struct Puerta {
    abre: f32,
    cierra: f32,
    suelo: f32,
    envolvente: f32,
    abierta: bool,
    espera: u32,
    espera_total: u32,
    ganancia: f32,
    ataque_env: f32,
    suelta_env: f32,
    ataque: f32,
    suelta: f32,
}

impl Puerta {
    pub fn nueva(tasa: u32) -> Puerta {
        Puerta {
            abre: de_db(-50.0),
            cierra: de_db(-56.0),
            suelo: de_db(-30.0),
            envolvente: 0.0,
            abierta: false,
            espera: 0,
            espera_total: (0.150 * tasa as f32) as u32,
            ganancia: de_db(-30.0),
            ataque_env: coeficiente(0.001, tasa),
            suelta_env: coeficiente(0.050, tasa),
            ataque: coeficiente(0.002, tasa),
            suelta: coeficiente(0.100, tasa),
        }
    }

    pub fn abierta(&self) -> bool {
        self.abierta
    }

    pub fn procesar(&mut self, bloque: &mut [f32], canales: usize) {
        let canales = canales.clamp(1, 2);
        for marco in bloque.chunks_exact_mut(canales) {
            let a = marco.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            let k = if a > self.envolvente {
                self.ataque_env
            } else {
                self.suelta_env
            };
            self.envolvente += (a - self.envolvente) * k;
            if self.envolvente > self.abre {
                self.abierta = true;
                self.espera = self.espera_total;
            } else if self.envolvente < self.cierra {
                if self.espera > 0 {
                    self.espera -= 1;
                } else {
                    self.abierta = false;
                }
            }
            let (quiere, k) = if self.abierta {
                (1.0, self.ataque)
            } else {
                (self.suelo, self.suelta)
            };
            self.ganancia += (quiere - self.ganancia) * k;
            for v in marco.iter_mut() {
                *v *= self.ganancia;
            }
        }
    }
}

/// A donde lleva el realce los picos de la voz: -3 dBFS.
pub const OBJETIVO_REALCE: f32 = 0.707_946;
/// Lo mas que sube: +24 dB.
pub const REALCE_MAXIMO: f32 = 15.848_932;
/// Por debajo de esto (-45 dBFS) no es voz y la ganancia no sube.
pub const PISO_REALCE: f32 = 0.005_623;

/// **El realce de la voz**: el control automatico de ganancia de
/// `pixpin-audio` (ataque 10 ms, suelta 300 ms, de 0 a +24 dB hacia -3 dBFS,
/// y en silencio vuelve despacio a 0 dB en vez de subir el ruido). El
/// limitador que llevaba detras ahora es el del final de la mezcla.
#[derive(Debug, Clone)]
pub struct Realce {
    ataque: f32,
    suelta: f32,
    vuelta: f32,
    envolvente: f32,
    ganancia: f32,
}

impl Realce {
    pub fn nuevo(tasa: u32) -> Realce {
        Realce {
            ataque: coeficiente(0.010, tasa),
            suelta: coeficiente(0.300, tasa),
            vuelta: coeficiente(1.5, tasa),
            envolvente: 0.0,
            ganancia: 1.0,
        }
    }

    /// La ganancia de ahora, en veces.
    pub fn ganancia(&self) -> f32 {
        self.ganancia
    }

    pub fn procesar(&mut self, bloque: &mut [f32], canales: usize) {
        let canales = canales.clamp(1, 2);
        for marco in bloque.chunks_exact_mut(canales) {
            let a = marco.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            let k = if a > self.envolvente {
                self.ataque
            } else {
                self.suelta
            };
            self.envolvente += (a - self.envolvente) * k;
            if self.envolvente < PISO_REALCE {
                self.ganancia += (1.0 - self.ganancia) * self.vuelta;
            } else {
                let quiere = (OBJETIVO_REALCE / self.envolvente).clamp(1.0, REALCE_MAXIMO);
                // Bajar es urgente (lo que viene se saldria); subir, no.
                let k = if quiere < self.ganancia {
                    self.ataque
                } else {
                    self.suelta
                };
                self.ganancia += (quiere - self.ganancia) * k;
            }
            for v in marco.iter_mut() {
                *v *= self.ganancia;
            }
        }
    }
}

/// Una **ganancia** fija que se puede cambiar en marcha sin chasquido: se
/// desliza hacia la nueva en unos 20 ms.
#[derive(Debug, Clone)]
pub struct Ganancia {
    actual: f32,
    k: f32,
}

impl Ganancia {
    pub fn nueva(valor: f32, tasa: u32) -> Ganancia {
        Ganancia {
            actual: valor,
            k: coeficiente(0.020, tasa),
        }
    }

    pub fn procesar(&mut self, bloque: &mut [f32], canales: usize, quiere: f32) {
        let canales = canales.clamp(1, 2);
        if (quiere - self.actual).abs() < 1e-6 {
            self.actual = quiere;
            if quiere != 1.0 {
                for v in bloque.iter_mut() {
                    *v *= quiere;
                }
            }
            return;
        }
        for marco in bloque.chunks_exact_mut(canales) {
            self.actual += (quiere - self.actual) * self.k;
            for v in marco.iter_mut() {
                *v *= self.actual;
            }
        }
    }
}

/// Desde donde actua el limitador: -1 dBFS.
pub const UMBRAL_LIMITADOR: f32 = 0.891;
/// Desde donde dobla la curva de seguridad.
const RODILLA: f32 = 0.93;
/// El techo: un pelo por debajo de la escala completa, para que al pasar a
/// 16 bits no se toque nunca el 32767.
pub const TECHO: f32 = 0.999_9;
/// Lo que mira por delante, en marcos: 1 ms a 48 kHz.
const MIRADA_MAXIMA: usize = 96;

/// La curva de seguridad: lineal hasta [`RODILLA`] y por encima una tangente
/// hiperbolica que se acerca al techo sin tocarlo. Simetrica.
pub fn doblar(x: f32) -> f32 {
    let a = x.abs();
    if a <= RODILLA {
        return x;
    }
    let margen = TECHO - RODILLA;
    (RODILLA + margen * ((a - RODILLA) / margen).tanh())
        .min(TECHO)
        .copysign(x)
}

/// **El limitador** del final, con un milisegundo de mirada.
///
/// La ganancia que hace falta se calcula con la muestra que **entra**, y se
/// aplica a la que **sale**, que es la de hace un milisegundo: para cuando el
/// golpe llega a la salida, la ganancia ya ha bajado (suavemente, en ese
/// milisegundo). Despues vuelve en 100 ms. Lo poco que se pueda colar en el
/// primer instante lo dobla [`doblar`].
#[derive(Debug, Clone)]
pub struct Limitador {
    canales: usize,
    retraso: usize,
    linea: [f32; MIRADA_MAXIMA * 2],
    pos: usize,
    objetivo: f32,
    ganancia: f32,
    ataque: f32,
    suelta: f32,
}

impl Limitador {
    pub fn nuevo(tasa: u32, canales: usize) -> Limitador {
        let retraso = ((tasa as usize) / 1000).clamp(1, MIRADA_MAXIMA);
        Limitador {
            canales: canales.clamp(1, 2),
            retraso,
            linea: [0.0; MIRADA_MAXIMA * 2],
            pos: 0,
            objetivo: 1.0,
            ganancia: 1.0,
            // Llega al 98 % del objetivo dentro de la mirada.
            ataque: 1.0 - (-4.0 / retraso as f32).exp(),
            suelta: coeficiente(0.100, tasa),
        }
    }

    /// Cuantos marcos retrasa la salida.
    pub fn retraso(&self) -> usize {
        self.retraso
    }

    /// La ganancia de ahora (1 = no esta actuando).
    pub fn ganancia(&self) -> f32 {
        self.ganancia
    }

    pub fn procesar(&mut self, bloque: &mut [f32]) {
        let c = self.canales;
        for marco in bloque.chunks_exact_mut(c) {
            let pico = marco.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            let necesita = if pico > UMBRAL_LIMITADOR {
                UMBRAL_LIMITADOR / pico
            } else {
                1.0
            };
            // El objetivo baja en el acto y vuelve despacio; la ganancia lo
            // sigue bajando en la mirada y subiendo con el.
            self.objetivo = necesita.min(self.objetivo + (1.0 - self.objetivo) * self.suelta);
            if self.objetivo < self.ganancia {
                self.ganancia += (self.objetivo - self.ganancia) * self.ataque;
            } else {
                self.ganancia = self.objetivo;
            }
            let i = self.pos * c;
            for (ch, v) in marco.iter_mut().enumerate() {
                let entra = if v.is_finite() { *v } else { 0.0 };
                let sale = self.linea[i + ch];
                self.linea[i + ch] = entra;
                *v = doblar(sale * self.ganancia);
            }
            // Sin division por marco: la vuelta es una comparacion.
            self.pos += 1;
            if self.pos == self.retraso {
                self.pos = 0;
            }
        }
    }
}

/// Pasa a 16 bits. Lo que entra ya viene limitado; se acota igual por si.
///
/// Redondea al mas cercano sumando medio con el signo y truncando, que es lo
/// mismo que `f32::round` pero sin llamada: con el procesador de referencia
/// (`target-cpu=x86-64`, sin SSE4.1) `round` es una funcion de la libreria
/// por muestra; asi es una suma y una conversion, y el bucle se vectoriza.
pub fn a_i16(entrada: &[f32], salida: &mut [i16]) {
    for (s, v) in salida.iter_mut().zip(entrada) {
        let y = v.clamp(-TECHO, TECHO) * 32_767.0;
        *s = (y + 0.5f32.copysign(y)) as i16;
    }
}

/// El pico y el RMS de un bloque.
pub fn medir(bloque: &[f32]) -> (f32, f32) {
    if bloque.is_empty() {
        return (0.0, 0.0);
    }
    let mut pico = 0.0f32;
    let mut cuadrados = 0.0f32;
    for v in bloque {
        pico = pico.max(v.abs());
        cuadrados += v * v;
    }
    (pico, (cuadrados / bloque.len() as f32).sqrt())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const FS: u32 = 48_000;

    fn seno(amplitud: f32, hz: f32, segundos: f32) -> Vec<f32> {
        let n = (segundos * FS as f32) as usize;
        (0..n)
            .map(|i| amplitud * (2.0 * std::f32::consts::PI * hz * i as f32 / FS as f32).sin())
            .collect()
    }

    fn pico(v: &[f32]) -> f32 {
        v.iter().fold(0.0, |m, x| m.max(x.abs()))
    }

    #[test]
    fn el_limitador_nunca_pasa_de_la_escala_completa() {
        // Hasta +20 dB por encima del techo, con golpes secos y senos.
        let mut l = Limitador::nuevo(FS, 2);
        for amplitud in [0.5f32, 0.95, 1.0, 2.0, 10.0] {
            let mut s: Vec<f32> = seno(amplitud, 440.0, 0.5)
                .into_iter()
                .flat_map(|v| [v, -v])
                .collect();
            // Un golpe de una sola muestra en medio.
            s[24_000] = amplitud * 1.5;
            l.procesar(&mut s);
            assert!(pico(&s) <= TECHO, "{amplitud}: {}", pico(&s));
        }
        assert_eq!(doblar(f32::MAX), TECHO);
        assert_eq!(doblar(-1.0e30), -TECHO);
    }

    #[test]
    fn el_limitador_no_toca_lo_que_no_se_pasa() {
        let mut l = Limitador::nuevo(FS, 1);
        let entrada = seno(0.5, 440.0, 0.2);
        let mut s = entrada.clone();
        l.procesar(&mut s);
        let r = l.retraso();
        for (a, b) in entrada.iter().zip(&s[r..]) {
            assert!((a - b).abs() < 1e-6);
        }
        assert_eq!(l.ganancia(), 1.0);
    }

    #[test]
    fn el_limitador_no_recorta_en_seco_una_voz_alta() {
        // +6 dB por encima: sale alta, sin tramos planos pegados al techo.
        let mut l = Limitador::nuevo(FS, 1);
        let mut s = seno(2.0, 300.0, 1.0);
        l.procesar(&mut s);
        let cola = &s[FS as usize / 2..];
        assert!(pico(cola) > 0.85 && pico(cola) <= TECHO);
        let pegadas = cola.iter().filter(|v| v.abs() > 0.999).count();
        assert_eq!(pegadas, 0);
    }

    #[test]
    fn una_voz_baja_sale_mas_alta_y_el_silencio_no_sube() {
        let mut r = Realce::nuevo(FS);
        let mut s = seno(0.02, 440.0, 2.0);
        for t in s.chunks_mut(480) {
            r.procesar(t, 1);
        }
        let cola = &s[FS as usize..];
        assert!(pico(cola) > 10.0 * 0.02, "{}", pico(cola));
        assert!(pico(cola) <= 0.02 * REALCE_MAXIMO * 1.05);
        let mut r = Realce::nuevo(FS);
        let mut ruido = vec![0.000_3f32; FS as usize];
        r.procesar(&mut ruido, 1);
        assert!(pico(&ruido) < 0.001, "{}", pico(&ruido));
    }

    #[test]
    fn el_paso_alto_quita_la_continua_y_deja_la_voz() {
        let mut p = PasoAlto::nuevo(80.0, FS);
        let mut s = vec![0.25f32; FS as usize];
        p.procesar(&mut s, 1);
        assert!(pico(&s[FS as usize / 2..]) < 1e-3);
        let mut p = PasoAlto::nuevo(80.0, FS);
        let mut v = seno(0.5, 1_000.0, 0.5);
        p.procesar(&mut v, 1);
        assert!((pico(&v[FS as usize / 4..]) - 0.5).abs() < 0.01);
        // Estereo: cada canal con su estado.
        let mut p = PasoAlto::nuevo(80.0, FS);
        let mut e: Vec<f32> = (0..FS).flat_map(|_| [0.25f32, 0.0]).collect();
        p.procesar(&mut e, 2);
        assert!(e.chunks_exact(2).all(|m| m[1] == 0.0));
    }

    #[test]
    fn la_puerta_baja_el_ventilador_y_deja_pasar_la_voz() {
        let mut g = Puerta::nueva(FS);
        let mut ruido = seno(0.000_5, 200.0, 1.0);
        g.procesar(&mut ruido, 1);
        assert!(!g.abierta());
        assert!(pico(&ruido[FS as usize / 2..]) < 0.000_5 * 0.05);
        let mut voz = seno(0.3, 300.0, 0.5);
        g.procesar(&mut voz, 1);
        assert!(g.abierta());
        assert!(pico(&voz[FS as usize / 4..]) > 0.29);
    }

    #[test]
    fn la_ganancia_cambia_sin_escalon() {
        let mut g = Ganancia::nueva(1.0, FS);
        let mut s = vec![0.5f32; 4_800];
        g.procesar(&mut s, 1, 0.0);
        assert!(s[0] > 0.49, "el primer marco casi igual");
        assert!(s.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01));
        assert!(s[4_799] < 0.01);
    }

    #[test]
    fn a_dieciseis_bits_no_toca_el_tope() {
        let mut fuera = [0i16; 3];
        a_i16(&[1.0, -1.0, 0.0], &mut fuera);
        assert_eq!(fuera, [32_764, -32_764, 0]);
    }

    #[test]
    fn pasar_a_16_bits_redondea_como_round() {
        let mut v = Vec::new();
        let mut x = -1.2f32;
        while x < 1.2 {
            v.push(x);
            x += 0.000_37;
        }
        v.extend([0.0, -0.0, 0.5 / 32_767.0, -0.5 / 32_767.0, 1.5 / 32_767.0]);
        let mut rapido = vec![0i16; v.len()];
        a_i16(&v, &mut rapido);
        for (a, x) in rapido.iter().zip(&v) {
            let esperado = (x.clamp(-TECHO, TECHO) * 32_767.0).round() as i16;
            assert_eq!(*a, esperado, "{x}");
        }
    }
}
