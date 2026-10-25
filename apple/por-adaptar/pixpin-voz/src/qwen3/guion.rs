//! **Lo que se le dice a Qwen3-ASR y como se lee lo que contesta**, sin
//! modelo.
//!
//! Qwen3-ASR es un modelo de chat: el audio entra como una fila de
//! `<|audio_pad|>` (uno por cada «token de audio» del encoder, unos 13 por
//! segundo) dentro de un turno de usuario, y el modelo contesta en el turno
//! del asistente. El guion es el de `offline-recognizer-qwen3-asr-impl.cc`
//! de sherpa-onnx y el de `qwen-asr`:
//!
//! ```text
//! <|im_start|>system\n<|im_end|>\n<|im_start|>user\n<|audio_start|>
//! <|audio_pad|> × n
//! <|audio_end|><|im_end|>\n<|im_start|>assistant\n
//! language Spanish<asr_text>          ← solo si se le dice el idioma
//! ```
//!
//! **El idioma se le dice** (como a Whisper): con el turno del asistente ya
//! empezado por `language Spanish<asr_text>`, el modelo solo escribe el
//! texto. Sin el, el modelo escribe primero `language X<asr_text>` y luego
//! el texto; [`Guion::sin_prefijo`] lo separa.
//!
//! Aqui viven tambien las cuentas que no necesitan ONNX: cuantos tokens de
//! audio salen de N marcos de mel, el tope de tokens por tanda, el token
//! elegido (voraz), los bucles de repeticion y por donde se parte una nota.

use std::ops::Range;

use super::fichas::{Fichas, Fusiones};
use crate::HERCIOS;

/// Los 30 idiomas de Qwen3-ASR (`support_languages` de su `config.json`),
/// con el codigo de la app. `tl` es otro nombre del filipino.
pub const IDIOMAS: [(&str, &str); 31] = [
    ("zh", "Chinese"),
    ("en", "English"),
    ("yue", "Cantonese"),
    ("ar", "Arabic"),
    ("de", "German"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("pt", "Portuguese"),
    ("id", "Indonesian"),
    ("it", "Italian"),
    ("ko", "Korean"),
    ("ru", "Russian"),
    ("th", "Thai"),
    ("vi", "Vietnamese"),
    ("ja", "Japanese"),
    ("tr", "Turkish"),
    ("hi", "Hindi"),
    ("ms", "Malay"),
    ("nl", "Dutch"),
    ("sv", "Swedish"),
    ("da", "Danish"),
    ("fi", "Finnish"),
    ("pl", "Polish"),
    ("cs", "Czech"),
    ("fil", "Filipino"),
    ("tl", "Filipino"),
    ("fa", "Persian"),
    ("el", "Greek"),
    ("ro", "Romanian"),
    ("hu", "Hungarian"),
    ("mk", "Macedonian"),
];

/// El nombre que entiende Qwen3-ASR para un codigo (`es`, `es-PE`…).
pub fn nombre_del_idioma(codigo: &str) -> Option<&'static str> {
    let lengua = codigo.split(['-', '_']).next().unwrap_or("").to_lowercase();
    IDIOMAS.iter().find(|(c, _)| *c == lengua).map(|(_, n)| *n)
}

/// La vuelta: el codigo de un nombre (`Spanish` → `es`).
pub fn codigo_del_idioma(nombre: &str) -> Option<&'static str> {
    IDIOMAS
        .iter()
        .find(|(_, n)| n.eq_ignore_ascii_case(nombre.trim()))
        .map(|(c, _)| *c)
}

/// Si Qwen3-ASR sabe el idioma.
pub fn sabe(codigo: &str) -> bool {
    nombre_del_idioma(codigo).is_some()
}

const ANTES: &str = "<|im_start|>system\n<|im_end|>\n<|im_start|>user\n<|audio_start|>";
const DESPUES: &str = "<|audio_end|><|im_end|>\n<|im_start|>assistant\n";

/// **El guion ya pasado a numeros**, una vez al cargar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guion {
    antes: Vec<i64>,
    despues: Vec<i64>,
    pub pad: i64,
    pub asr_text: i64,
    /// `<|im_end|>` y `<|endoftext|>`: cualquiera de los dos cierra.
    pub fines: Vec<i64>,
    /// `language X` de cada idioma, por nombre.
    idiomas: Vec<(&'static str, Vec<i64>)>,
}

impl Guion {
    /// Codifica el guion con el tokenizador. `None` si falta algun
    /// especial (no es el tokenizador de Qwen3-ASR).
    pub fn nuevo(f: &Fichas, fusiones: &Fusiones) -> Option<Guion> {
        let pad = f.especial("<|audio_pad|>")?;
        let asr_text = f.especial("<asr_text>")?;
        let fines = vec![f.especial("<|im_end|>")?, f.especial("<|endoftext|>")?];
        f.especial("<|audio_start|>")?;
        f.especial("<|audio_end|>")?;
        let mut idiomas: Vec<(&'static str, Vec<i64>)> = Vec::new();
        for (_, n) in IDIOMAS {
            if !idiomas.iter().any(|(m, _)| *m == n) {
                idiomas.push((n, f.codificar(&format!("language {n}"), fusiones)));
            }
        }
        Some(Guion {
            antes: f.codificar(ANTES, fusiones),
            despues: f.codificar(DESPUES, fusiones),
            pad,
            asr_text,
            fines,
            idiomas,
        })
    }

    /// **La entrada del decoder** para `audio` tokens de audio, con el
    /// idioma dicho (`Some("Spanish")`) o para que lo adivine (`None`).
    pub fn fuente(&self, audio: usize, idioma: Option<&str>) -> Vec<i64> {
        let mut v = Vec::with_capacity(self.antes.len() + audio + self.despues.len() + 4);
        v.extend_from_slice(&self.antes);
        v.extend(std::iter::repeat_n(self.pad, audio));
        v.extend_from_slice(&self.despues);
        if let Some(ids) = idioma.and_then(|n| self.idiomas.iter().find(|(m, _)| *m == n)) {
            v.extend_from_slice(&ids.1);
            v.push(self.asr_text);
        }
        v
    }

    pub fn es_fin(&self, token: i64) -> bool {
        self.fines.contains(&token)
    }

    /// **Lo escrito sin el `language X<asr_text>` del principio** (cuando
    /// se le dejo adivinar el idioma), y el codigo del idioma que dijo.
    /// Solo se mira en los 16 primeros tokens, como sherpa-onnx.
    pub fn sin_prefijo<'a>(&self, ids: &'a [i64], f: &Fichas) -> (&'a [i64], Option<&'static str>) {
        let ventana = ids.len().min(16);
        let Some(p) = ids[..ventana].iter().position(|&t| t == self.asr_text) else {
            return (ids, None);
        };
        let dicho = f.texto(&ids[..p]);
        let codigo = dicho
            .trim()
            .strip_prefix("language")
            .and_then(codigo_del_idioma);
        (&ids[p + 1..], codigo)
    }
}

/// Los marcos de mel que agrupa el frente en cada «trozo» (1 s).
const TROZO_DEL_FRENTE: usize = 100;

/// **Cuantos tokens de audio salen de `marcos` de mel**: tres
/// convoluciones de paso 2 por cada trozo de 100 marcos (13 tokens por
/// segundo), y lo que sobre, lo suyo (`FeatToAudioTokensLen` de sherpa-onnx).
pub fn tokens_de_audio(marcos: usize) -> usize {
    let tres = |n: usize| {
        let x = n.div_ceil(2);
        let x = x.div_ceil(2);
        x.div_ceil(2)
    };
    let tras = |x: usize| {
        if x == 0 {
            return 0;
        }
        let x = (x - 1) / 2 + 1;
        let x = (x - 1) / 2 + 1;
        (x - 1) / 2 + 1
    };
    let llenos = marcos / TROZO_DEL_FRENTE;
    let resto = marcos % TROZO_DEL_FRENTE;
    llenos * tres(TROZO_DEL_FRENTE) + tras(resto)
}

/// **Cuantos tokens puede escribir como mucho** una tanda de `muestras`.
/// En castellano salen ~3,3 por segundo (medido con FLEURS); se dejan diez
/// y un margen, como con Whisper: cortar una frase buena es peor que dejar
/// correr una mala (esa la paran [`repite_al_final`] o `creible`).
pub fn tope_de_tokens(muestras: usize) -> usize {
    (muestras as f32 / HERCIOS as f32 * 10.0).ceil() as usize + 16
}

/// El token mas probable (voraz). Los valores no finitos no compiten.
pub fn elegir(fila: &[f32]) -> i64 {
    let mut mejor = 0usize;
    let mut valor = f32::NEG_INFINITY;
    for (i, &l) in fila.iter().enumerate() {
        if l.is_finite() && l > valor {
            valor = l;
            mejor = i;
        }
    }
    mejor as i64
}

/// **Si el final se ha metido en un bucle**, el largo que hay que dejar.
///
/// Dos reglas: la de Whisper (cuatro veces seguidas el mismo bloque de 1 a
/// 16 tokens: se deja uno) y la de sherpa-onnx para Qwen3-ASR (los ultimos
/// 64 tokens son solo 2 o 3 distintos: se quita la cola entera).
pub fn repite_al_final(t: &[i64]) -> Option<usize> {
    const VECES: usize = 4;
    for p in 1..=16 {
        if t.len() < VECES * p {
            break;
        }
        let ultimo = &t[t.len() - p..];
        if (2..=VECES).all(|k| &t[t.len() - k * p..t.len() - (k - 1) * p] == ultimo) {
            return Some(t.len() - (VECES - 1) * p);
        }
    }
    const VENTANA: usize = 64;
    const DISTINTOS: usize = 3;
    if t.len() >= VENTANA {
        let cola = &t[t.len() - VENTANA..];
        let mut vistos: Vec<i64> = Vec::new();
        for &x in cola {
            if !vistos.contains(&x) {
                vistos.push(x);
                if vistos.len() > DISTINTOS {
                    return None;
                }
            }
        }
        let quitar = cola.iter().rev().take_while(|x| vistos.contains(x)).count();
        return Some(t.len() - quitar);
    }
    None
}

/// **Cuantas filas del encoder tienen voz**: las del final que salen a
/// cero (relleno o silencio puro) se quitan, como `TrimAudioFeatures` de
/// sherpa-onnx. `ancho` es lo largo de cada fila.
pub fn filas_con_voz(datos: &[f32], ancho: usize) -> usize {
    if ancho == 0 {
        return 0;
    }
    let filas = datos.len() / ancho;
    (0..filas)
        .rev()
        .find(|&f| {
            datos[f * ancho..(f + 1) * ancho]
                .iter()
                .any(|x| x.abs() > 1e-6)
        })
        .map_or(0, |f| f + 1)
}

/// Lo que se junta en una tanda: frases seguidas hasta 20 s. El frente
/// aguanta mas, pero cada segundo son 13 tokens de audio en la cache del
/// decoder y en la memoria del paso de entrada.
pub const TANDA_SEGUNDOS: usize = 20;

/// Ninguna tanda pasa de 30 s (lo que el modelo vio entrenando).
pub const TOPE_SEGUNDOS: usize = 30;

/// **Por donde se parte una nota**: en frases ([`crate::cortes`], las
/// mismas que con Whisper) juntadas hasta 20 s, sin solapar, cortando
/// siempre en un silencio o en el punto mas callado. Cada tanda sabe en
/// que muestra empieza: de ahi sale su `[m:ss]`.
pub fn tandas(muestras: &[i16]) -> Vec<Range<usize>> {
    let tope = TOPE_SEGUNDOS * HERCIOS as usize;
    let frases = crate::cortes::cortes(muestras);
    crate::cortes::agrupar(&frases, TANDA_SEGUNDOS)
        .into_iter()
        .flat_map(|r| {
            (r.start..r.end)
                .step_by(tope)
                .map(move |a| a..(a + tope).min(r.end))
        })
        .collect()
}

/// Si una tanda es silencio (ni un tramo de 20 ms por encima del ruido de
/// fondo): no se le da al modelo, que con silencio puede inventar.
pub fn callada(trozo: &[i16]) -> bool {
    const VENTANA: usize = 320;
    const RUIDO_DE_FONDO: f64 = 120.0;
    !trozo.chunks(VENTANA).any(|v| {
        let e = v.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / v.len().max(1) as f64;
        e.sqrt() >= RUIDO_DE_FONDO
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn fichas() -> (Fichas, Fusiones) {
        // Un tokenizador de juguete con los especiales de Qwen y cada letra
        // suelta (sin fusiones): basta para ver la forma del guion.
        let mut vocab = serde_json::Map::new();
        for (i, b) in (0u8..=255).enumerate() {
            vocab.insert(
                super::super::fichas::letra_de_byte(b).to_string(),
                serde_json::Value::from(i as i64),
            );
        }
        let vocab = serde_json::Value::Object(vocab).to_string();
        let mut config = serde_json::Map::new();
        for (i, n) in [
            "<|endoftext|>",
            "<|im_start|>",
            "<|im_end|>",
            "<|audio_start|>",
            "<|audio_end|>",
            "<|audio_pad|>",
            "<asr_text>",
        ]
        .iter()
        .enumerate()
        {
            config.insert(
                (1000 + i).to_string(),
                serde_json::json!({"content": n, "special": true}),
            );
        }
        let config = serde_json::json!({ "added_tokens_decoder": config }).to_string();
        (Fichas::leer(&vocab, &config).unwrap(), Fusiones::default())
    }

    #[test]
    fn el_guion_lleva_el_audio_en_medio_y_el_idioma_al_final() {
        let (f, fus) = fichas();
        let g = Guion::nuevo(&f, &fus).unwrap();
        let v = g.fuente(3, Some("Spanish"));
        let pads: Vec<usize> = (0..v.len()).filter(|&i| v[i] == g.pad).collect();
        assert_eq!(pads.len(), 3);
        assert!(pads.windows(2).all(|p| p[1] == p[0] + 1), "seguidos");
        assert_eq!(v[pads[0] - 1], 1003, "<|audio_start|> justo antes");
        assert_eq!(v[pads[2] + 1], 1004, "<|audio_end|> justo despues");
        assert_eq!(*v.last().unwrap(), g.asr_text);
        assert!(f.texto(&v).ends_with("assistant\nlanguage Spanish"));
        // Sin idioma acaba en el turno del asistente, sin `<asr_text>`.
        let w = g.fuente(3, None);
        assert!(!w.contains(&g.asr_text));
        assert!(f.texto(&w).ends_with("assistant\n"));
        assert_eq!(g.fines, [1002, 1000]);
    }

    #[test]
    fn lo_adivinado_se_separa_del_texto_y_dice_el_idioma() {
        let (f, fus) = fichas();
        let g = Guion::nuevo(&f, &fus).unwrap();
        // Con el juguete cada letra es un token: un nombre corto, para que
        // `<asr_text>` caiga dentro de los 16 primeros (con el de verdad,
        // `language English` son dos tokens).
        let mut ids = f.codificar("language Thai", &fus);
        ids.push(g.asr_text);
        let texto = f.codificar("Hello", &fus);
        ids.extend(&texto);
        let (resto, idioma) = g.sin_prefijo(&ids, &f);
        assert_eq!(resto, &texto[..]);
        assert_eq!(idioma, Some("th"));
        // Sin prefijo, todo es texto.
        assert_eq!(g.sin_prefijo(&texto, &f), (&texto[..], None));
    }

    #[test]
    fn los_idiomas_de_la_app_tienen_su_nombre_en_qwen() {
        assert_eq!(nombre_del_idioma("es"), Some("Spanish"));
        assert_eq!(nombre_del_idioma("es-PE"), Some("Spanish"));
        assert_eq!(nombre_del_idioma("EN_us"), Some("English"));
        assert_eq!(nombre_del_idioma("pt"), Some("Portuguese"));
        assert_eq!(nombre_del_idioma("tl"), Some("Filipino"));
        assert_eq!(nombre_del_idioma("eu"), None, "el euskera no esta");
        assert_eq!(codigo_del_idioma(" spanish"), Some("es"));
        assert!(sabe("es") && !sabe(""));
    }

    #[test]
    fn de_los_marcos_salen_los_tokens_de_audio_de_sherpa() {
        // `FeatToAudioTokensLen(n, 100)`: 13 por segundo lleno.
        assert_eq!(tokens_de_audio(0), 0);
        assert_eq!(tokens_de_audio(100), 13);
        assert_eq!(tokens_de_audio(1), 1);
        assert_eq!(tokens_de_audio(150), 13 + 7);
        assert_eq!(tokens_de_audio(2_000), 260);
        assert_eq!(tokens_de_audio(1_999), 247 + 13);
    }

    #[test]
    fn el_tope_crece_con_lo_largo_de_la_tanda() {
        assert_eq!(tope_de_tokens(0), 16);
        assert_eq!(tope_de_tokens(20 * HERCIOS as usize), 216);
    }

    #[test]
    fn se_elige_el_mas_alto_y_lo_que_no_es_numero_no_cuenta() {
        assert_eq!(elegir(&[0.0, 3.0, f32::NAN, 2.0]), 1);
        assert_eq!(elegir(&[f32::INFINITY, -1.0]), 1);
        assert_eq!(elegir(&[]), 0);
    }

    #[test]
    fn un_bucle_se_corta_y_una_frase_normal_no() {
        assert_eq!(repite_al_final(&[1, 5, 6, 5, 6, 5, 6, 5, 6]), Some(3));
        assert_eq!(repite_al_final(&[1, 5, 6, 5, 6, 5, 6]), None);
        let frase: Vec<i64> = (0..100).collect();
        assert_eq!(repite_al_final(&frase), None);
        // La regla de sherpa: 64 tokens de solo tres distintos, sin bloque
        // exacto que se repita cuatro veces.
        let mut t: Vec<i64> = (100..110).collect();
        let ciclo = [1, 2, 1, 3, 1, 2, 2, 3, 1];
        for i in 0..70 {
            t.push(ciclo[i % ciclo.len()]);
        }
        // El ciclo de 9 se repite exacto: tambien vale la regla de Whisper,
        // que deja una vuelta. En cualquier caso se queda lo de antes.
        let largo = repite_al_final(&t).unwrap();
        assert!(largo >= 10 && largo < t.len(), "{largo}");
        assert_eq!(&t[..10], &(100..110).collect::<Vec<_>>()[..]);
    }

    #[test]
    fn las_filas_a_cero_del_final_no_son_voz() {
        let d = [1.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0];
        assert_eq!(filas_con_voz(&d, 2), 2);
        assert_eq!(filas_con_voz(&[0.0; 6], 3), 0);
        assert_eq!(filas_con_voz(&d, 0), 0);
    }

    fn tono(segundos: f32) -> Vec<i16> {
        (0..(segundos * HERCIOS as f32) as usize)
            .map(|i| ((i as f32 * 0.3).sin() * 6_000.0) as i16)
            .collect()
    }

    #[test]
    fn una_nota_larga_se_parte_en_tandas_de_hasta_treinta_segundos_sin_perder_nada() {
        // Un minuto de habla continua (sin pausas): se parte por los puntos
        // mas callados y ninguna tanda pasa del tope.
        let pcm = tono(65.0);
        let t = tandas(&pcm);
        assert!(t.len() >= 3, "{t:?}");
        assert!(
            t.iter()
                .all(|r| r.len() <= TOPE_SEGUNDOS * HERCIOS as usize)
        );
        assert_eq!(t[0].start, 0);
        assert_eq!(t.last().unwrap().end, pcm.len());
        assert!(t.windows(2).all(|p| p[0].end <= p[1].start), "sin solapar");
    }

    #[test]
    fn dos_frases_con_una_pausa_van_juntas_si_caben() {
        let mut pcm = tono(5.0);
        pcm.extend(vec![0i16; HERCIOS as usize]);
        pcm.extend(tono(5.0));
        assert_eq!(tandas(&pcm).len(), 1);
    }

    #[test]
    fn el_silencio_no_se_le_da_al_modelo() {
        assert!(callada(&[0i16; 16_000]));
        assert!(callada(&[50i16; 16_000]), "ruido de fondo");
        assert!(!callada(&tono(1.0)));
        assert!(callada(&[]));
    }
}
