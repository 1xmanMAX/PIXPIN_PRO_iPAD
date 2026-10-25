//! **El espectrograma log-mel de Whisper**, igual que `whisper/audio.py`.
//!
//! Whisper no oye muestras: oye 80 bandas mel cada 10 ms, 3000 marcos (30
//! segundos). El encoder se entreno con estos numeros exactos, asi que cada
//! paso se copia de `log_mel_spectrogram` de OpenAI:
//!
//! 1. El audio se rellena con ceros hasta 30 s (`pad_or_trim`).
//! 2. STFT con ventana de Hann periodica de 400 muestras (25 ms), salto de
//!    160 (10 ms), centrada con relleno reflejado (`torch.stft(center=True)`),
//!    y se tira el ultimo marco (`stft[..., :-1]`): 3000 marcos justos.
//! 3. Potencia `|X|^2` de los 201 cubos de frecuencia.
//! 4. 80 filtros mel de `librosa.filters.mel(sr=16000, n_fft=400,
//!    n_mels=80)`: escala de Slaney (lineal hasta 1 kHz, logaritmica
//!    despues) y normalizados por area (`norm="slaney"`).
//! 5. `log10(max(x, 1e-10))`, recorte a `max - 8` y `(x + 4) / 4`.
//!
//! Aritmetica pura: se prueba entero sin modelo.

use std::f64::consts::PI;
use std::sync::OnceLock;

/// Muestras por segundo que espera Whisper (las mismas de todo el crate).
pub const HERCIOS: usize = 16_000;
pub const N_FFT: usize = 400;
pub const SALTO: usize = 160;
pub const BANDAS: usize = 80;
/// Cubos de frecuencia de una FFT real de 400: 0..=200.
pub const CUBOS: usize = N_FFT / 2 + 1;
/// 30 segundos: lo que oye Whisper de una vez.
pub const MUESTRAS_30S: usize = 30 * HERCIOS;
pub const MARCOS: usize = MUESTRAS_30S / SALTO;

/// De hercios a la escala mel de Slaney (`librosa.hz_to_mel(htk=False)`):
/// lineal por debajo de 1 kHz y logaritmica por encima.
pub fn hz_a_mel(f: f64) -> f64 {
    let f_sp = 200.0 / 3.0;
    let min_log_hz = 1000.0;
    let min_log_mel = min_log_hz / f_sp;
    let paso_log = (6.4f64).ln() / 27.0;
    if f >= min_log_hz {
        min_log_mel + (f / min_log_hz).ln() / paso_log
    } else {
        f / f_sp
    }
}

/// La vuelta de [`hz_a_mel`].
pub fn mel_a_hz(m: f64) -> f64 {
    let f_sp = 200.0 / 3.0;
    let min_log_hz = 1000.0;
    let min_log_mel = min_log_hz / f_sp;
    let paso_log = (6.4f64).ln() / 27.0;
    if m >= min_log_mel {
        min_log_hz * (paso_log * (m - min_log_mel)).exp()
    } else {
        f_sp * m
    }
}

/// **Los 80 filtros mel**, `BANDAS x CUBOS`, fila a fila.
///
/// Es `librosa.filters.mel` paso a paso: 82 puntos equiespaciados en mel
/// entre 0 y 8 kHz, un triangulo por banda entre sus vecinos, y cada
/// triangulo escalado por `2 / (ancho en Hz)` para que todas las bandas
/// tengan la misma area. Se calcula en `f64` y se guarda en `f32`, como el
/// `mel_filters.npz` de OpenAI.
pub fn filtros() -> &'static [f32] {
    static FILTROS: OnceLock<Vec<f32>> = OnceLock::new();
    FILTROS.get_or_init(|| {
        let tope = hz_a_mel(HERCIOS as f64 / 2.0);
        let puntos: Vec<f64> = (0..BANDAS + 2)
            .map(|i| mel_a_hz(tope * i as f64 / (BANDAS + 1) as f64))
            .collect();
        let frecuencia = |k: usize| k as f64 * HERCIOS as f64 / N_FFT as f64;
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

/// Coseno y seno de cada cubo y cada muestra de la ventana, ya
/// multiplicados por la ventana de Hann. Es la DFT a mano: 400 no es
/// potencia de dos y una FFT de radix mixto no merece su codigo para lo que
/// cuesta esto frente al encoder.
fn tablas() -> &'static (Vec<f32>, Vec<f32>) {
    static TABLAS: OnceLock<(Vec<f32>, Vec<f32>)> = OnceLock::new();
    TABLAS.get_or_init(|| {
        let mut cos = vec![0f32; CUBOS * N_FFT];
        let mut sin = vec![0f32; CUBOS * N_FFT];
        for n in 0..N_FFT {
            // Hann periodica (`torch.hann_window(400)`): divide por N, no
            // por N - 1.
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

/// **El log-mel de hasta 30 s de audio**, `BANDAS x MARCOS`, banda a banda
/// (la forma `[1, 80, 3000]` que espera el encoder).
///
/// `audio` son muestras de -1 a 1 a 16 kHz; lo que pase de 30 s se ignora
/// (quien llama trocea antes: ver `cortes`).
pub fn log_mel(audio: &[f32]) -> Vec<f32> {
    let n = audio.len().min(MUESTRAS_30S);
    // Relleno a 30 s y, alrededor, el reflejo de `center=True`: 200
    // muestras a cada lado, `x[-k] = x[k]` sin repetir el borde.
    let mitad = N_FFT / 2;
    let mut senal = vec![0f32; MUESTRAS_30S + 2 * mitad];
    senal[mitad..mitad + n].copy_from_slice(&audio[..n]);
    for i in 0..mitad {
        senal[mitad - 1 - i] = senal[mitad + i + 1];
        senal[mitad + MUESTRAS_30S + i] = senal[mitad + MUESTRAS_30S - 2 - i];
    }

    let (cos, sin) = tablas();
    let w = filtros();
    let mut mel = vec![0f32; BANDAS * MARCOS];
    let mut potencia = [0f32; CUBOS];
    for m in 0..MARCOS {
        let trozo = &senal[m * SALTO..m * SALTO + N_FFT];
        // Un marco de puro relleno da potencia cero en todos los cubos: no
        // hace falta la DFT. Con una tanda de 5 s son 5 de cada 6 marcos.
        if trozo.iter().all(|&x| x == 0.0) {
            potencia.fill(0.0);
        } else {
            for (k, p) in potencia.iter_mut().enumerate() {
                let c = &cos[k * N_FFT..(k + 1) * N_FFT];
                let s = &sin[k * N_FFT..(k + 1) * N_FFT];
                // Ocho sumas a la vez: una suma unica en `f32` no se puede
                // vectorizar (cambiaria el orden de redondeo), ocho
                // independientes si, y el marco va cuatro veces mas rapido.
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
        for b in 0..BANDAS {
            let fila = &w[b * CUBOS..(b + 1) * CUBOS];
            let e: f32 = fila.iter().zip(&potencia).map(|(a, p)| a * p).sum();
            mel[b * MARCOS + m] = e.max(1e-10).log10();
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

    #[test]
    fn la_escala_mel_de_slaney_es_lineal_hasta_mil_hercios_y_va_y_vuelve() {
        assert!((hz_a_mel(1000.0) - 15.0).abs() < 1e-9);
        assert!((hz_a_mel(200.0) - 3.0).abs() < 1e-9);
        for f in [0.0, 37.0, 999.0, 1000.0, 4321.0, 8000.0] {
            assert!((mel_a_hz(hz_a_mel(f)) - f).abs() < 1e-6, "{f}");
        }
    }

    #[test]
    fn el_primer_filtro_vale_lo_mismo_que_en_el_npz_de_openai() {
        // `mel_filters.npz["mel_80"][0][1]` = 0.02486259; el resto de la
        // primera fila, cero pasado el segundo cubo.
        let w = filtros();
        assert!((w[1] - 0.024_862_59).abs() < 1e-7, "{}", w[1]);
        assert_eq!(w[0], 0.0, "el cubo de 0 Hz cae en el borde del triangulo");
        assert!(w[3..CUBOS].iter().all(|&x| x == 0.0));
    }

    #[test]
    fn ningun_filtro_es_negativo_y_todos_tocan_algun_cubo() {
        let w = filtros();
        assert!(w.iter().all(|&x| x >= 0.0));
        for b in 0..BANDAS {
            let fila = &w[b * CUBOS..(b + 1) * CUBOS];
            assert!(fila.iter().any(|&x| x > 0.0), "la banda {b} esta vacia");
        }
    }

    #[test]
    fn el_silencio_da_menos_uno_y_medio_en_todo_el_espectrograma() {
        // log10(1e-10) = -10, recortado a -10 y (-10 + 4) / 4 = -1.5: el
        // valor que Whisper conoce como «nada».
        let mel = log_mel(&[]);
        assert_eq!(mel.len(), BANDAS * MARCOS);
        assert!(mel.iter().all(|&x| (x + 1.5).abs() < 1e-6));
    }

    #[test]
    fn un_pitido_de_mil_hercios_cae_en_la_banda_de_mil_hercios() {
        let audio: Vec<f32> = (0..HERCIOS)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / HERCIOS as f32).sin())
            .collect();
        let mel = log_mel(&audio);
        // Un marco de en medio del pitido.
        let m = 50;
        let banda = (0..BANDAS)
            .max_by(|&a, &b| mel[a * MARCOS + m].total_cmp(&mel[b * MARCOS + m]))
            .unwrap();
        // 1 kHz son 15 mel; los centros estan cada 45,25/81 mel: la banda
        // 26 (centro en el punto 27).
        assert!((25..=27).contains(&banda), "salio la banda {banda}");
        // Pasado el segundo de audio, el relleno queda por debajo del pitido
        // y no mas de 8 decadas por debajo del maximo.
        let tope = mel.iter().cloned().fold(f32::MIN, f32::max);
        let despues = mel[banda * MARCOS + 500];
        assert!(despues < tope);
        assert!((despues - (tope - 2.0)).abs() < 1e-5, "recorte a max - 8");
    }

    #[test]
    fn lo_que_pasa_de_treinta_segundos_no_desborda() {
        let largo = vec![0.1f32; MUESTRAS_30S + 5_000];
        assert_eq!(log_mel(&largo).len(), BANDAS * MARCOS);
    }
}
