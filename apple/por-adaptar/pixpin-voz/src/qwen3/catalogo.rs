//! **Que se baja para Qwen3-ASR-0.6B**, de donde, cuanto pesa y su sha256.
//!
//! Es la exportacion a ONNX int8 que publica sherpa-onnx (k2-fsa) en
//! `csukuangfj2/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25`, fijada a su
//! commit (un commit no cambia): tres modelos —el frente convolucional, el
//! encoder de audio y el decoder (el modelo de lenguaje, con su cache de
//! claves y valores)— y el tokenizador de Qwen (`vocab.json`, `merges.txt` y
//! la lista de tokens especiales). Comprobados con `sha256sum` el
//! 2026-10-25.
//!
//! Licencia: los pesos de Qwen3-ASR-0.6B son **Apache-2.0** (Qwen/Alibaba);
//! la exportacion es una conversion de esos mismos pesos. Ver
//! `docs/voz-qwen3.md`.

/// Commit del repositorio de Hugging Face con la exportacion.
pub const REVISION: &str = "68818b2313fe77bd06f6a7c5068ff3ef59d02b8a";

/// La licencia de los pesos, para ensenarla antes de bajar.
pub const LICENCIA: &str = "Apache-2.0";

/// Donde se lee la licencia (la ficha del modelo original).
pub const ENLACE_LICENCIA: &str = "https://huggingface.co/Qwen/Qwen3-ASR-0.6B";

/// Un fichero que se baja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fichero {
    /// Como se guarda en `<datos>/qwen3-asr/`.
    pub nombre: &'static str,
    pub url: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

macro_rules! sherpa {
    ($nombre:literal, $ruta:literal, $b:expr, $s:literal) => {
        Fichero {
            nombre: $nombre,
            url: concat!(
                "https://huggingface.co/csukuangfj2/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25/resolve/68818b2313fe77bd06f6a7c5068ff3ef59d02b8a/",
                $ruta
            ),
            bytes: $b,
            sha256: $s,
        }
    };
}

/// Los seis ficheros, por orden de bajada: primero lo pequeno, para que un
/// fallo de red se note en el acto.
pub const FICHEROS: [Fichero; 6] = [
    sherpa!(
        "tokenizer_config.json",
        "tokenizer/tokenizer_config.json",
        12_487,
        "4942d005604266809309cabc9f4e9cb89ce855d59b14681fdc0e1cc62ea26c4c"
    ),
    sherpa!(
        "merges.txt",
        "tokenizer/merges.txt",
        1_671_853,
        "8831e4f1a044471340f7c0a83d7bd71306a5b867e95fd870f74d0c5308a904d5"
    ),
    sherpa!(
        "vocab.json",
        "tokenizer/vocab.json",
        2_776_833,
        "ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910"
    ),
    sherpa!(
        "conv_frontend.onnx",
        "conv_frontend.onnx",
        44_148_281,
        "d22dc4423e0940e49884e903d2ea2f7e5567c14fc1aed97e4e26d6b8f208ef9e"
    ),
    sherpa!(
        "encoder.int8.onnx",
        "encoder.int8.onnx",
        182_491_662,
        "60748d3e6744a57c9c91e1b17424a6c2990567e8adceb0783940c03ed98fa9d9"
    ),
    sherpa!(
        "decoder.int8.onnx",
        "decoder.int8.onnx",
        755_914_231,
        "4f6885be5959ae26af3089d38ee7972c5fafbeeb1cf8d5e76eab6d8b61ca5771"
    ),
];

/// Indices en [`FICHEROS`], para no escribir los nombres dos veces.
pub const CONFIG: usize = 0;
pub const MERGES: usize = 1;
pub const VOCAB: usize = 2;
pub const CONV: usize = 3;
pub const ENCODER: usize = 4;
pub const DECODER: usize = 5;

/// Lo que se ensena al usuario: «unos 988 MB».
pub fn megas() -> u64 {
    FICHEROS
        .iter()
        .map(|f| f.bytes)
        .sum::<u64>()
        .div_ceil(1_000_000)
}

/// **Memoria que pide mientras transcribe**, en MB, medida en el Xeon de 4
/// hilos (ver `docs/voz-qwen3.md`): pico del proceso con el runtime ya
/// cargado, con los pesos preempaquetados (lo rapido) y sin ellos (el modo
/// cuidadoso).
pub const MEGAS_DE_RAM: u64 = 1_650;
pub const MEGAS_DE_RAM_CUIDADOSO: u64 = 1_450;

/// Por debajo de esto no se ofrece: con menos de 4 GB no cabe con Windows
/// y la propia app al lado.
pub const RAM_MINIMA_MB: u64 = 3_500;

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_ficheros_estan_fijados_al_commit_y_traen_su_sha() {
        for f in FICHEROS {
            assert_eq!(f.sha256.len(), 64, "{}", f.nombre);
            assert!(f.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(f.url.starts_with("https://huggingface.co/"));
            assert!(f.url.contains(REVISION), "{}", f.nombre);
            assert!(f.url.ends_with(f.nombre), "{}", f.nombre);
            assert!(f.bytes > 0);
        }
        assert_eq!(FICHEROS[CONV].nombre, "conv_frontend.onnx");
        assert_eq!(FICHEROS[ENCODER].nombre, "encoder.int8.onnx");
        assert_eq!(FICHEROS[DECODER].nombre, "decoder.int8.onnx");
        assert_eq!(FICHEROS[VOCAB].nombre, "vocab.json");
        assert_eq!(FICHEROS[MERGES].nombre, "merges.txt");
        assert_eq!(FICHEROS[CONFIG].nombre, "tokenizer_config.json");
    }

    #[test]
    fn pesa_casi_un_giga_y_lo_pequeno_va_primero() {
        assert_eq!(megas(), 988);
        assert!(FICHEROS.windows(2).all(|p| p[0].bytes <= p[1].bytes));
    }
}
