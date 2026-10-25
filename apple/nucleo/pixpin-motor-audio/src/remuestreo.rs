//! **Remuestrear** cada fuente a la tasa de la salida, con la relacion que
//! va pidiendo [`crate::deriva`].
//!
//! Un seno enventanado (Kaiser, β = 7) de **48 coeficientes**, en tabla
//! polifasica de 128 fases e interpolando entre fase y fase. Es lo justo:
//!
//! - **Calidad.** Banda de paso plana hasta ~19 kHz y lo de fuera atenuado
//!   unos 70 dB. Al bajar de tasa (un microfono a 96 kHz, o 48 → 44,1) el
//!   corte se mueve con la relacion, que es el filtro anti-solapamiento: un
//!   pitido de 23 kHz no vuelve como un silbido de 21 kHz.
//! - **Coste.** 48 multiplicaciones por marco y canal: dos fuentes estereo a
//!   48 kHz son unos 9 millones por segundo, menos del 1 % de un Core i3 de
//!   tercera generacion. La tabla son 25 KB y cabe en la cache.
//! - **Relacion variable.** La fase se calcula para cada marco de salida, asi
//!   que cambiar el paso unas ppm entre un bloque y otro no mete ningun
//!   chasquido: no hay estado que dependa del paso.
//!
//! La posicion se lleva en **coma fija 32.32** relativa al principio del
//! historial, que se recorta en cada bloque. Con coma flotante, la misma
//! entrada daria muestras distintas segun como se trocease (el redondeo de
//! cada suma depende del tamano del numero), y eso no se puede probar; asi la
//! cuenta es exacta y el paso tiene una resolucion de 2^-32 marcos, mil
//! veces por debajo de una ppm.
//!
//! No reserva memoria al producir: el historial se reserva al crearlo para el
//! bloque mas grande que se le va a pedir.
//!
//! ## Al bajar mucho de tasa
//!
//! Un microfono de estudio a 96 o 192 kHz que baja a 48: con los mismos 48
//! coeficientes, la transicion del filtro (que se mide en marcos de entrada)
//! seria 2 o 4 veces mas ancha en hercios y dejaria pasar solapamiento. Como
//! hace speexdsp (`update_filter`, en `libspeexdsp/resample.c`: alarga
//! `filt_len` por la relacion al bajar), el filtro se alarga con la relacion
//! redondeada: 96 coeficientes de 96 a 48, 192 de 192 a 48. El coste por
//! marco de **salida** crece igual, pero por marco de **entrada** es el de
//! siempre.
//!
//! ## Para la CPU
//!
//! Las sumas van en [`CARRILES`] acumuladores independientes: una suma de
//! coma flotante en una sola variable es una cadena de dependencias (cada
//! suma espera a la anterior) que el compilador no puede reordenar ni
//! vectorizar; en ocho carriles se solapan y caben en registros SSE/AVX.
//! Nada de esto cambia el resultado entre una ejecucion y otra (el orden de
//! las sumas es fijo), asi que la prueba de que trocear no cambia nada sigue
//! valiendo.

/// Coeficientes del filtro a la misma tasa o subiendo (al bajar mucho se
/// multiplican, ver arriba). Multiplo de [`CARRILES`].
pub const COEFICIENTES: usize = 48;
/// Lo mas que se alarga el filtro (de 192 kHz a 48).
pub const ALARGUE_MAXIMO: usize = 4;
/// Acumuladores independientes de cada producto escalar.
pub const CARRILES: usize = 8;
/// Fases de la tabla.
pub const FASES: usize = 1 << BITS_DE_FASE;
const BITS_DE_FASE: u32 = 7;
const BETA: f64 = 7.0;
/// Donde corta el filtro, respecto a la mitad de la tasa mas baja de las
/// dos. Con 48 coeficientes la transicion mide unos 4 kHz: asi acaba justo en
/// Nyquist.
const CORTE: f64 = 0.91;

/// La funcion de Bessel modificada de orden cero, por su serie.
fn bessel_i0(x: f64) -> f64 {
    let mut suma = 1.0;
    let mut termino = 1.0;
    let y = x * x / 4.0;
    for k in 1..50 {
        termino *= y / (k * k) as f64;
        suma += termino;
        if termino < 1e-12 * suma {
            break;
        }
    }
    suma
}

fn a_fija(paso: f64) -> u64 {
    (paso * 4_294_967_296.0).round() as u64
}

/// Cuantos coeficientes para ir de `entrada` a `salida` marcos por segundo.
pub fn coeficientes_para(entrada: u32, salida: u32) -> usize {
    let relacion = f64::from(entrada.max(1)) / f64::from(salida.max(1));
    COEFICIENTES * (relacion.round() as usize).clamp(1, ALARGUE_MAXIMO)
}

fn tabla(corte: f64, taps: usize) -> Vec<f32> {
    let mitad = taps / 2;
    let mut t = vec![0.0f32; (FASES + 1) * taps];
    let i0b = bessel_i0(BETA);
    let mut valores = vec![0.0f64; taps];
    for p in 0..=FASES {
        let f = p as f64 / FASES as f64;
        let fila = &mut t[p * taps..(p + 1) * taps];
        let mut suma = 0.0f64;
        for (k, v) in valores.iter_mut().enumerate() {
            // Distancia del marco de entrada `k` al instante de salida.
            let d = k as f64 - (mitad as f64 - 1.0) - f;
            let x = d / mitad as f64;
            let ventana = if x.abs() >= 1.0 {
                0.0
            } else {
                bessel_i0(BETA * (1.0 - x * x).sqrt()) / i0b
            };
            let a = std::f64::consts::PI * corte * d;
            let sinc = if a.abs() < 1e-12 { 1.0 } else { a.sin() / a };
            *v = corte * sinc * ventana;
            suma += *v;
        }
        // Cada fase suma uno: un nivel constante sale igual, sea cual sea la
        // fase, y no aparece un zumbido a la frecuencia de la deriva.
        for (c, v) in fila.iter_mut().zip(&valores) {
            *c = (v / suma) as f32;
        }
    }
    t
}

/// `Σ a[i]·b[i]` en [`CARRILES`] acumuladores (ver el comentario del modulo).
/// Los dos del mismo largo, multiplo de [`CARRILES`].
#[inline]
fn producto(a: &[f32], b: &[f32]) -> f32 {
    let mut acc = [0.0f32; CARRILES];
    for (x, y) in a.chunks_exact(CARRILES).zip(b.chunks_exact(CARRILES)) {
        for k in 0..CARRILES {
            acc[k] += x[k] * y[k];
        }
    }
    let mut s = 0.0;
    for v in acc {
        s += v;
    }
    s
}

/// Un remuestreador de `canales` canales intercalados.
#[derive(Debug, Clone)]
pub struct Remuestreador {
    canales: usize,
    paso_nominal: f64,
    /// Marcos de entrada por marco de salida, en coma fija 32.32.
    paso: u64,
    tabla: Vec<f32>,
    /// Coeficientes del filtro (ver [`coeficientes_para`]) y su mitad.
    taps: usize,
    mitad: usize,
    /// Los coeficientes del marco que se esta calculando (interpolados entre
    /// dos fases), y un canal suelto cuando hay mas de dos.
    coef: Vec<f32>,
    canal: Vec<f32>,
    /// Marcos de entrada pendientes, intercalados.
    historial: Vec<f32>,
    /// Donde cae el proximo marco de salida, en marcos de `historial` y en
    /// coma fija 32.32.
    pos: u64,
    max_marcos: usize,
}

impl Remuestreador {
    /// De `entrada` a `salida` marcos por segundo, para bloques de hasta
    /// `max_marcos` marcos (de salida, o de entrada al empujar).
    pub fn nuevo(entrada: u32, salida: u32, canales: usize, max_marcos: usize) -> Remuestreador {
        let canales = canales.max(1);
        let (entrada, salida) = (f64::from(entrada.max(1)), f64::from(salida.max(1)));
        let taps = coeficientes_para(entrada as u32, salida as u32);
        let paso = entrada / salida;
        let corte = CORTE * (salida / entrada).min(1.0);
        let max_marcos = max_marcos.max(1);
        // Lo mas que puede hacer falta: el bloque entero al paso mas rapido
        // que permite la deriva, mas el filtro.
        let hueco = (max_marcos as f64 * paso * (1.0 + 2.0 * crate::deriva::TOPE)).ceil() as usize;
        let mut r = Remuestreador {
            canales,
            paso_nominal: paso,
            paso: a_fija(paso),
            tabla: tabla(corte, taps),
            taps,
            mitad: taps / 2,
            coef: vec![0.0; taps],
            canal: if canales > 2 {
                vec![0.0; taps]
            } else {
                Vec::new()
            },
            historial: Vec::with_capacity((hueco.max(max_marcos) + 2 * taps) * canales),
            pos: 0,
            max_marcos,
        };
        r.reiniciar();
        r
    }

    pub fn canales(&self) -> usize {
        self.canales
    }

    /// Coeficientes del filtro que usa.
    pub fn coeficientes(&self) -> usize {
        self.taps
    }

    /// Multiplica el paso nominal por `factor` (lo que da la deriva).
    pub fn ajustar(&mut self, factor: f64) {
        self.paso = a_fija(self.paso_nominal * factor);
    }

    /// Vacia el historial: lo siguiente empieza de silencio.
    pub fn reiniciar(&mut self) {
        self.historial.clear();
        self.historial.resize((self.mitad - 1) * self.canales, 0.0);
        self.pos = ((self.mitad - 1) as u64) << 32;
    }

    fn marcos_en_historial(&self) -> usize {
        self.historial.len() / self.canales
    }

    /// Cuantos marcos de historial necesita el marco de salida en `pos`.
    fn necesarios(&self, pos: u64) -> usize {
        (pos >> 32) as usize + self.mitad + 1
    }

    /// Produce un marco de salida en `fuera` y avanza.
    #[inline]
    fn un_marco(&mut self, fuera: &mut [f32]) {
        let taps = self.taps;
        let c = self.canales;
        let i0 = (self.pos >> 32) as usize;
        // Los 7 bits altos de la fraccion son la fase; los 16 siguientes, lo
        // que se interpola entre ella y la siguiente.
        let fraccion = self.pos & 0xFFFF_FFFF;
        let p = (fraccion >> (32 - BITS_DE_FASE)) as usize;
        let w = ((fraccion >> (32 - BITS_DE_FASE - 16)) & 0xFFFF) as f32 / 65_536.0;
        let a = &self.tabla[p * taps..(p + 1) * taps];
        let b = &self.tabla[(p + 1) * taps..(p + 2) * taps];
        for ((k, x), y) in self.coef.iter_mut().zip(a).zip(b) {
            *k = x + (y - x) * w;
        }
        let base = (i0 + 1 - self.mitad) * c;
        let tramo = &self.historial[base..base + taps * c];
        match c {
            1 => fuera[0] = producto(tramo, &self.coef),
            2 => {
                // Intercalado: los pares son un canal y los impares el otro,
                // cada uno en sus carriles.
                let mut acc = [0.0f32; CARRILES * 2];
                for (m, k) in tramo
                    .chunks_exact(CARRILES * 2)
                    .zip(self.coef.chunks_exact(CARRILES))
                {
                    for j in 0..CARRILES {
                        acc[2 * j] += m[2 * j] * k[j];
                        acc[2 * j + 1] += m[2 * j + 1] * k[j];
                    }
                }
                let (mut i, mut d) = (0.0f32, 0.0f32);
                for par in acc.chunks_exact(2) {
                    i += par[0];
                    d += par[1];
                }
                fuera[0] = i;
                fuera[1] = d;
            }
            _ => {
                for (ch, f) in fuera.iter_mut().enumerate().take(c) {
                    for (v, x) in self.canal.iter_mut().zip(tramo[ch..].iter().step_by(c)) {
                        *v = *x;
                    }
                    *f = producto(&self.canal, &self.coef);
                }
            }
        }
        self.pos += self.paso;
    }

    /// Tira del historial lo que ya no va a hacer falta.
    fn compactar(&mut self) {
        let quitar = ((self.pos >> 32) as usize + 1).saturating_sub(self.mitad);
        let quitar = quitar.min(self.marcos_en_historial());
        if quitar > 0 {
            self.historial.drain(..quitar * self.canales);
            self.pos -= (quitar as u64) << 32;
        }
    }

    /// **Tirando de la fuente**: llena `salida` entera (en marcos) pidiendo a
    /// `leer` los marcos de entrada que hagan falta. Si `leer` no tiene
    /// bastantes, lo que falta es silencio. Devuelve cuantos marcos de
    /// entrada faltaron.
    pub fn producir(
        &mut self,
        salida: &mut [f32],
        mut leer: impl FnMut(&mut [f32]) -> usize,
    ) -> usize {
        let c = self.canales;
        let mut faltaron = 0;
        for trozo in salida.chunks_mut(self.max_marcos * c) {
            let n = trozo.len() / c;
            if n == 0 {
                continue;
            }
            let ultimo = self.pos + self.paso * (n - 1) as u64;
            let hace_falta = self.necesarios(ultimo);
            let hay = self.marcos_en_historial();
            if hace_falta > hay {
                let pedir = hace_falta - hay;
                let antes = self.historial.len();
                self.historial.resize(antes + pedir * c, 0.0);
                let llegaron = leer(&mut self.historial[antes..]).min(pedir);
                faltaron += pedir - llegaron;
            }
            for fuera in trozo.chunks_exact_mut(c) {
                self.un_marco(fuera);
            }
            self.compactar();
        }
        faltaron
    }

    /// **Empujando**: mete marcos de entrada para sacarlos luego con
    /// [`Remuestreador::sacar`].
    pub fn meter(&mut self, entrada: &[f32]) {
        let n = entrada.len() / self.canales * self.canales;
        self.historial.extend_from_slice(&entrada[..n]);
    }

    /// Saca todos los marcos de salida que ya se pueden calcular con lo
    /// metido, hasta llenar `salida`. Devuelve cuantos.
    pub fn sacar(&mut self, salida: &mut [f32]) -> usize {
        let c = self.canales;
        let mut n = 0;
        for fuera in salida.chunks_exact_mut(c) {
            if self.necesarios(self.pos) > self.marcos_en_historial() {
                break;
            }
            self.un_marco(fuera);
            n += 1;
        }
        self.compactar();
        n
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::f64::consts::PI;

    fn seno(hz: f64, tasa: f64, n: usize, amplitud: f64) -> Vec<f32> {
        (0..n)
            .map(|i| (amplitud * (2.0 * PI * hz * i as f64 / tasa).sin()) as f32)
            .collect()
    }

    /// Remuestrea `entrada` (mono) tirando de ella en el orden de llegada.
    fn de_una(r: &mut Remuestreador, entrada: &[f32], marcos: usize, trozo: usize) -> Vec<f32> {
        let mut leido = 0;
        let mut salida = vec![0.0; marcos];
        for t in salida.chunks_mut(trozo) {
            r.producir(t, |destino| {
                let n = destino.len().min(entrada.len() - leido);
                destino[..n].copy_from_slice(&entrada[leido..leido + n]);
                leido += n;
                n
            });
        }
        salida
    }

    /// El error de `salida` contra el seno ideal a la tasa de salida, en dB
    /// respecto a la amplitud, sin el arranque ni el final.
    fn error_db(salida: &[f32], hz: f64, tasa: f64, amplitud: f64) -> f64 {
        let mut peor: f64 = 0.0;
        for (j, v) in salida.iter().enumerate().skip(200).take(salida.len() - 400) {
            let ideal = amplitud * (2.0 * PI * hz * j as f64 / tasa).sin();
            peor = peor.max((f64::from(*v) - ideal).abs());
        }
        20.0 * (peor / amplitud).log10()
    }

    #[test]
    fn de_44100_a_48000_un_barrido_de_senos_sale_limpio() {
        for hz in [100.0, 1_000.0, 5_000.0, 10_000.0, 15_000.0] {
            let entrada = seno(hz, 44_100.0, 44_100, 0.5);
            let mut r = Remuestreador::nuevo(44_100, 48_000, 1, 1_024);
            let salida = de_una(&mut r, &entrada, 47_000, 480);
            let e = error_db(&salida, hz, 48_000.0, 0.5);
            assert!(e < -60.0, "{hz} Hz: error de {e:.1} dB");
        }
    }

    #[test]
    fn de_96_y_192_khz_a_48_lo_ultrasonico_no_vuelve() {
        for (tasa, coef) in [(96_000u32, 96), (192_000, 192)] {
            let fs = f64::from(tasa);
            // 30 kHz no cabe en 48 kHz: sin un filtro mas largo volvia como
            // un silbido de 18 kHz.
            let entrada = seno(30_000.0, fs, tasa as usize, 0.9);
            let mut r = Remuestreador::nuevo(tasa, 48_000, 1, 1_024);
            assert_eq!(r.coeficientes(), coef);
            let salida = de_una(&mut r, &entrada, 40_000, 480);
            let pico = salida[400..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
            let db = 20.0 * (f64::from(pico) / 0.9).log10();
            assert!(db < -60.0, "{tasa}: se colo a {db:.1} dB");
            // Lo que si cabe pasa entero.
            let entrada = seno(5_000.0, fs, tasa as usize, 0.5);
            let mut r = Remuestreador::nuevo(tasa, 48_000, 1, 1_024);
            let salida = de_una(&mut r, &entrada, 40_000, 480);
            let e = error_db(&salida, 5_000.0, 48_000.0, 0.5);
            assert!(e < -60.0, "{tasa}: 5 kHz con error de {e:.1} dB");
        }
        // Subir o quedarse no alarga nada.
        assert_eq!(coeficientes_para(44_100, 48_000), COEFICIENTES);
        assert_eq!(coeficientes_para(48_000, 44_100), COEFICIENTES);
        assert_eq!(
            coeficientes_para(384_000, 48_000),
            COEFICIENTES * ALARGUE_MAXIMO
        );
    }

    #[test]
    fn seis_canales_salen_cada_uno_por_su_lado() {
        // El camino general (mas de dos canales) da lo mismo que el mono.
        let base = seno(700.0, 48_000.0, 6_000, 0.5);
        let entrada: Vec<f32> = base
            .iter()
            .flat_map(|v| (0..6).map(move |k| *v * (k as f32 + 1.0) / 6.0))
            .collect();
        let mut r6 = Remuestreador::nuevo(48_000, 44_100, 6, 1_024);
        let mut r1 = Remuestreador::nuevo(48_000, 44_100, 1, 1_024);
        let mut leido = 0;
        let mut s6 = vec![0.0; 6 * 4_000];
        r6.producir(&mut s6, |d| {
            let n = d.len().min(entrada.len() - leido);
            d[..n].copy_from_slice(&entrada[leido..leido + n]);
            leido += n;
            n / 6
        });
        let s1 = de_una(&mut r1, &base, 4_000, 4_000);
        for (m, v) in s6.chunks_exact(6).zip(&s1) {
            for (k, x) in m.iter().enumerate() {
                assert!((x - v * (k as f32 + 1.0) / 6.0).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn a_la_misma_tasa_la_voz_pasa_igual() {
        let entrada = seno(1_000.0, 48_000.0, 48_000, 0.8);
        let mut r = Remuestreador::nuevo(48_000, 48_000, 1, 512);
        let salida = de_una(&mut r, &entrada, 47_000, 333);
        assert!(error_db(&salida, 1_000.0, 48_000.0, 0.8) < -70.0);
    }

    #[test]
    fn al_bajar_de_tasa_lo_que_no_cabe_no_vuelve_como_un_silbido() {
        // 23 kHz a 48 kHz no cabe en 44,1 kHz (Nyquist en 22 050): tiene que
        // desaparecer, no reflejarse a 21 kHz.
        let entrada = seno(23_000.0, 48_000.0, 48_000, 0.9);
        let mut r = Remuestreador::nuevo(48_000, 44_100, 1, 1_024);
        let salida = de_una(&mut r, &entrada, 40_000, 441);
        let pico = salida[200..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let db = 20.0 * (f64::from(pico) / 0.9).log10();
        assert!(db < -50.0, "se colo a {db:.1} dB");
        // Caso negativo: lo que si cabe pasa entero.
        let entrada = seno(1_000.0, 48_000.0, 48_000, 0.9);
        let mut r = Remuestreador::nuevo(48_000, 44_100, 1, 1_024);
        let salida = de_una(&mut r, &entrada, 40_000, 441);
        assert!(error_db(&salida, 1_000.0, 44_100.0, 0.9) < -60.0);
    }

    #[test]
    fn el_resultado_no_depende_de_como_se_trocee() {
        let entrada = seno(3_000.0, 44_100.0, 20_000, 0.5);
        let mut a = Remuestreador::nuevo(44_100, 48_000, 1, 2_048);
        let mut b = Remuestreador::nuevo(44_100, 48_000, 1, 2_048);
        let x = de_una(&mut a, &entrada, 20_000, 2_000);
        let y = de_una(&mut b, &entrada, 20_000, 77);
        assert_eq!(x, y);
        // Y empujando se obtiene lo mismo que tirando.
        let mut c = Remuestreador::nuevo(44_100, 48_000, 1, 2_048);
        let mut z = Vec::new();
        let mut fuera = vec![0.0; 4_096];
        for t in entrada.chunks(441) {
            c.meter(t);
            let n = c.sacar(&mut fuera);
            z.extend_from_slice(&fuera[..n]);
        }
        assert_eq!(&x[..z.len().min(x.len())], &z[..z.len().min(x.len())]);
        assert!(z.len() > 20_000, "{}", z.len());
    }

    #[test]
    fn estereo_lleva_cada_canal_por_su_lado() {
        let izq = seno(500.0, 48_000.0, 10_000, 0.5);
        let entrada: Vec<f32> = izq.iter().flat_map(|v| [*v, -*v]).collect();
        let mut r = Remuestreador::nuevo(48_000, 44_100, 2, 1_024);
        let mut leido = 0;
        let mut salida = vec![0.0; 2 * 8_000];
        r.producir(&mut salida, |d| {
            let n = d.len().min(entrada.len() - leido);
            d[..n].copy_from_slice(&entrada[leido..leido + n]);
            leido += n;
            n / 2
        });
        for m in salida.chunks_exact(2) {
            assert!((m[0] + m[1]).abs() < 1e-5);
        }
        let solo_izq: Vec<f32> = salida.chunks_exact(2).map(|m| m[0]).collect();
        assert!(error_db(&solo_izq, 500.0, 44_100.0, 0.5) < -60.0);
    }

    #[test]
    fn si_la_fuente_se_queda_corta_se_cuenta_y_sale_silencio() {
        let mut r = Remuestreador::nuevo(48_000, 48_000, 1, 1_024);
        let mut salida = vec![1.0; 480];
        let faltaron = r.producir(&mut salida, |_| 0);
        assert!(faltaron >= 480, "{faltaron}");
        assert!(salida.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn la_relacion_se_ajusta_en_marcha_sin_saltos() {
        // Cambiar el paso 500 ppm a mitad de un seno no mete un escalon: la
        // diferencia entre muestras vecinas no pasa de lo que da el seno.
        let entrada = seno(1_000.0, 48_000.0, 48_000, 0.5);
        let mut r = Remuestreador::nuevo(48_000, 48_000, 1, 1_024);
        let mut leido = 0;
        let mut salida = Vec::new();
        for i in 0..90 {
            r.ajustar(if i % 2 == 0 { 1.0 } else { 1.0005 });
            let mut t = vec![0.0; 480];
            r.producir(&mut t, |d| {
                let n = d.len().min(entrada.len() - leido);
                d[..n].copy_from_slice(&entrada[leido..leido + n]);
                leido += n;
                n
            });
            salida.extend(t);
        }
        let salto_maximo = 2.0 * PI * 1_000.0 / 48_000.0 * 0.5 * 1.01;
        for v in salida[100..].windows(2) {
            assert!(f64::from((v[1] - v[0]).abs()) <= salto_maximo);
        }
    }
}
