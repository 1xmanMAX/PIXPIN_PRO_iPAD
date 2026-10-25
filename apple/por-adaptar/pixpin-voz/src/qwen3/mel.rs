//! **El log-mel de Qwen3-ASR**: el de Whisper con 128 bandas y sin rellenar
//! a 30 s.
//!
//! Qwen3-ASR usa el `WhisperFeatureExtractor` de `transformers`
//! (`preprocessor_config.json`: `feature_size = 128`, `n_fft = 400`,
//! `hop_length = 160`), pero a diferencia de Whisper **no rellena hasta
//! 30 s**: el frente convolucional acepta cualquier numero de marcos. Los
//! pasos, los mismos que `crate::whisper::mel` (que se queda como esta
//! para no tocar Whisper):
//!
//! 1. STFT con Hann periodica de 400 muestras, salto de 160, centrada con
//!    relleno reflejado (`center=True`), y se tira el ultimo marco: salen
//!    `muestras / 160` marcos.
//! 2. Potencia de los 201 cubos y 128 filtros mel de Slaney normalizados por
//!    area (`mel_filter_bank(norm="slaney", mel_scale="slaney")`).
//! 3. `log10(max(x, 1e-10))`, recorte a `max - 8` sobre todo el trozo y
//!    `(x + 4) / 4`.
//!
//! Sale **marco a marco** (`[marcos, 128]`), que es la forma de la entrada
//! `input_features` del frente (`[1, marcos, 128]`).
//!
//! Aritmetica pura: se prueba entera sin modelo, contra valores de numpy.

use std::f64::consts::PI;
use std::sync::OnceLock;

pub const HERCIOS: usize = 16_000;
pub const N_FFT: usize = 400;
pub const SALTO: usize = 160;
pub const BANDAS: usize = 128;
/// Cubos de frecuencia de una FFT real de 400: 0..=200.
pub const CUBOS: usize = N_FFT / 2 + 1;

/// Hercios a mel de Slaney: lineal hasta 1 kHz y logaritmica despues.
fn hz_a_mel(f: f64) -> f64 {
    let f_sp = 200.0 / 3.0;
    if f >= 1000.0 {
        1000.0 / f_sp + (f / 1000.0).ln() / ((6.4f64).ln() / 27.0)
    } else {
        f / f_sp
    }
}

fn mel_a_hz(m: f64) -> f64 {
    let f_sp = 200.0 / 3.0;
    let min_log_mel = 1000.0 / f_sp;
    if m >= min_log_mel {
        1000.0 * (((6.4f64).ln() / 27.0) * (m - min_log_mel)).exp()
    } else {
        f_sp * m
    }
}

/// **Los 128 filtros**, `BANDAS x CUBOS`, fila a fila: 130 puntos
/// equiespaciados en mel de 0 a 8 kHz, un triangulo por banda y cada uno
/// escalado por `2 / ancho` (misma area).
pub fn filtros() -> &'static [f32] {
    static FILTROS: OnceLock<Vec<f32>> = OnceLock::new();
    FILTROS.get_or_init(|| {
        let tope = hz_a_mel(HERCIOS as f64 / 2.0);
        let puntos: Vec<f64> = (0..BANDAS + 2)
            .map(|i| mel_a_hz(tope * i as f64 / (BANDAS + 1) as f64))
            .collect();
        let frecuencia = |k: usize| k as f64 * (HERCIOS as f64 / 2.0) / (CUBOS - 1) as f64;
        let mut w = vec![0f32; BANDAS * CUBOS];
        for b in 0..BANDAS {
            let (izq, centro, der) = (puntos[b], puntos[b + 1], puntos[b + 2]);
            let norma = 2.0 / (der - izq);
            for k in 0..CUBOS {
                let f = frecuencia(k);
                let sube = (f - izq) / (centro - izq);
                let baja = (der - f) / (der - centro);
                w[b * CUBOS + k] = (sube.min(baja).max(0.0) * norma) as f32;
            }
        }
        w
    })
}

/// Coseno y seno de cada cubo por cada muestra de la ventana, con la Hann
/// ya aplicada: la DFT a mano (400 no es potencia de dos).
fn tablas() -> &'static (Vec<f32>, Vec<f32>) {
    static TABLAS: OnceLock<(Vec<f32>, Vec<f32>)> = OnceLock::new();
    TABLAS.get_or_init(|| {
        let mut cos = vec![0f32; CUBOS * N_FFT];
        let mut sin = vec![0f32; CUBOS * N_FFT];
        for n in 0..N_FFT {
            let hann = 0.5 - 0.5 * (2.0 * PI * n as f64 / N_FFT as f64).cos();
            for k in 0..CUBOS {
                let a = 2.0 * PI * ((k * n) % N_FFT) as f64 / N_FFT as f64;
                cos[k * N_FFT + n] = (hann * a.cos()) as f32;
                sin[k * N_FFT + n] = (hann * a.sin()) as f32;
            }
        }
        (cos, sin)
    })
}

/// Cuantos marcos salen de `muestras` (lo que espera el frente).
pub fn marcos(muestras: usize) -> usize {
    muestras / SALTO
}

/// **El log-mel de un trozo**, `[marcos, 128]` marco a marco. `audio` va de
/// -1 a 1 a 16 kHz. Un trozo de menos de 10 ms no da ningun marco.
pub fn log_mel(audio: &[f32]) -> Vec<f32> {
    let n_marcos = marcos(audio.len());
    if n_marcos == 0 {
        return Vec::new();
    }
    // `center=True`: 200 muestras de reflejo a cada lado (`x[-k] = x[k]`).
    // Con menos de 201 muestras no hay reflejo posible; no llega: un
    // trozo asi no da marcos (ver arriba).
    let mitad = N_FFT / 2;
    let n = audio.len();
    let mut senal = vec![0f32; n + 2 * mitad];
    senal[mitad..mitad + n].copy_from_slice(audio);
    for i in 0..mitad {
        let izq = (i + 1).min(n - 1);
        senal[mitad - 1 - i] = audio[izq];
        let der = n.saturating_sub(2 + i);
        senal[mitad + n + i] = audio[der];
    }

    let (cos, sin) = tablas();
    let w = filtros();
    let mut mel = vec![0f32; n_marcos * BANDAS];
    let mut potencia = [0f32; CUBOS];
    for m in 0..n_marcos {
        let trozo = &senal[m * SALTO..m * SALTO + N_FFT];
        if trozo.iter().all(|&x| x == 0.0) {
            potencia.fill(0.0);
        } else {
            for (k, p) in potencia.iter_mut().enumerate() {
                let c = &cos[k * N_FFT..(k + 1) * N_FFT];
                let s = &sin[k * N_FFT..(k + 1) * N_FFT];
                // Ocho sumas independientes: se vectorizan (ver whisper::mel).
                let (mut re, mut im) = ([0f32; 8], [0f32; 8]);
                for ((t, c), s) in trozo
                    .chunks_exact(8)
                    .zip(c.chunks_exact(8))
                    .zip(s.chunks_exact(8))
                {
                    for j in 0..8 {
                        re[j] += t[j] * c[j];
                        im[j] += t[j] * s[j];
                    }
                }
                let (re, im): (f32, f32) = (re.iter().sum(), im.iter().sum());
                *p = re * re + im * im;
            }
        }
        let fila = &mut mel[m * BANDAS..(m + 1) * BANDAS];
        for (b, salida) in fila.iter_mut().enumerate() {
            let f = &w[b * CUBOS..(b + 1) * CUBOS];
            let e: f32 = f.iter().zip(&potencia).map(|(a, p)| a * p).sum();
            *salida = e.max(1e-10).log10();
        }
    }
    let tope = mel.iter().cloned().fold(f32::MIN, f32::max);
    for x in &mut mel {
        *x = (x.max(tope - 8.0) + 4.0) / 4.0;
    }
    mel
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Dos tonos y un barrido, 0,5 s: la misma senal que se paso por
    /// numpy para sacar [`REFERENCIA`].
    fn senal() -> Vec<f32> {
        (0..8_000)
            .map(|i| {
                let t = i as f64 / HERCIOS as f64;
                (0.3 * (2.0 * PI * 440.0 * t).sin()
                    + 0.2 * (2.0 * PI * 3_000.0 * t).sin()
                    + 0.1 * (2.0 * PI * (100.0 + 4_000.0 * t) * t).sin()) as f32
            })
            .collect()
    }

    #[test]
    fn salen_tantos_marcos_como_saltos_y_nada_de_un_trozo_minimo() {
        assert_eq!(marcos(16_000), 100);
        assert_eq!(log_mel(&vec![0.1; 16_000]).len(), 100 * BANDAS);
        assert!(log_mel(&[0.5; 100]).is_empty());
        assert_eq!(log_mel(&[0.5; 160]).len(), BANDAS, "un marco, sin reflejo");
    }

    #[test]
    fn los_filtros_son_los_de_transformers() {
        // `mel_filter_bank(201, 128, 0, 8000, 16000, "slaney", "slaney")`
        // de transformers: la banda 0 vale 0,01237399 en el cubo 1.
        let w = filtros();
        assert_eq!(w[0], 0.0);
        assert!((w[1] - 0.012_373_99).abs() < 1e-7, "{}", w[1]);
        assert!(w.iter().all(|&x| x >= 0.0));
        for b in 0..BANDAS {
            assert!(
                w[b * CUBOS..(b + 1) * CUBOS].iter().any(|&x| x > 0.0),
                "{b}"
            );
        }
    }

    #[test]
    fn coincide_con_numpy_en_la_senal_de_prueba() {
        // `WhisperFeatureExtractor(feature_size=128)` de transformers 4.x
        // con `padding="do_not_pad"` sobre `senal()` (numpy, float64):
        // marco, banda, valor.
        let mel = log_mel(&senal());
        assert_eq!(mel.len(), 50 * BANDAS);
        let esperados: [(usize, usize, f32); 8] = REFERENCIA;
        for (m, b, v) in esperados {
            let x = mel[m * BANDAS + b];
            assert!(
                (x - v).abs() < 2e-3,
                "marco {m} banda {b}: {x} frente a {v}"
            );
        }
        let media = mel.iter().sum::<f32>() / mel.len() as f32;
        assert!((media - MEDIA_REFERENCIA).abs() < 1e-3, "{media}");
    }

    /// Ver `coincide_con_numpy_en_la_senal_de_prueba`.
    const REFERENCIA: [(usize, usize, f32); 8] = [
        (0, 0, 1.012_514),
        (0, 10, 0.864_424),
        (5, 20, 1.262_612),
        (10, 30, 0.216_630),
        (25, 64, -0.542_875),
        (40, 90, 0.768_601),
        (49, 127, -0.469_444),
        (20, 5, -0.590_586),
    ];
    const MEDIA_REFERENCIA: f32 = -0.337_133_7;

    #[test]
    fn el_silencio_entero_da_menos_uno_y_medio() {
        let mel = log_mel(&[0.0; 3_200]);
        assert!(mel.iter().all(|&x| (x + 1.5).abs() < 1e-6));
    }
}
