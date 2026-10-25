//! **De numeros a texto**: la tabla de tokens de Whisper y los metadatos
//! del modelo.
//!
//! `base-tokens.txt` de sherpa-onnx trae una linea por token: los **bytes**
//! del token en base64 y su numero (`IQ== 0` es «!»). Son bytes y no texto
//! porque Whisper trocea en bytes (BPE de GPT-2): una «ñ» puede venir
//! partida en dos tokens, y solo al juntar los bytes de toda la frase sale
//! UTF-8 valido. Por eso se juntan bytes y se convierte al final.
//!
//! Los metadatos (`sot`, `eot`, idiomas…) vienen como texto en el
//! `custom_metadata_map` del encoder; aqui solo se leen las listas.
//!
//! Texto puro: se prueba entero sin modelo.

use std::collections::HashMap;

/// Base64 estandar (con `+`, `/` y relleno `=`), a mano: es una tabla de
/// 64 letras y no merece un crate.
pub fn base64(texto: &str) -> Option<Vec<u8>> {
    fn valor(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    }
    let t = texto.trim().as_bytes();
    if t.is_empty() || t.len() % 4 != 0 {
        return None;
    }
    let mut v = Vec::with_capacity(t.len() / 4 * 3);
    for (n, grupo) in t.chunks(4).enumerate() {
        let ultimo = n == t.len() / 4 - 1;
        let relleno = grupo.iter().rev().take_while(|&&c| c == b'=').count();
        if relleno > 2 || (relleno > 0 && !ultimo) {
            return None;
        }
        let mut acc = 0u32;
        for &c in &grupo[..4 - relleno] {
            acc = (acc << 6) | valor(c)?;
        }
        acc <<= 6 * relleno as u32;
        let bytes = [(acc >> 16) as u8, (acc >> 8) as u8, acc as u8];
        v.extend_from_slice(&bytes[..3 - relleno]);
    }
    Some(v)
}

/// La tabla de tokens: numero a bytes.
#[derive(Debug, Default)]
pub struct Fichas {
    bytes: HashMap<i64, Vec<u8>>,
}

impl Fichas {
    /// Lee el contenido de `base-tokens.txt`. Una linea que no se entiende
    /// se salta: el fichero trae alguna rareza (`= 50256`, el fin de texto,
    /// que no es base64) y esos tokens no se escriben nunca.
    pub fn leer(texto: &str) -> Fichas {
        let mut bytes = HashMap::new();
        for linea in texto.lines() {
            let mut partes = linea.split_whitespace();
            let (Some(b64), Some(id), None) = (partes.next(), partes.next(), partes.next()) else {
                continue;
            };
            let (Ok(id), Some(b)) = (id.parse::<i64>(), base64(b64)) else {
                continue;
            };
            bytes.insert(id, b);
        }
        Fichas { bytes }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// **El texto de una lista de tokens.** Los que no estan en la tabla
    /// (los especiales: idioma, marcas de tiempo…) no se escriben.
    pub fn texto(&self, ids: &[i64]) -> String {
        let mut b = Vec::new();
        for id in ids {
            if let Some(t) = self.bytes.get(id) {
                b.extend_from_slice(t);
            }
        }
        String::from_utf8_lossy(&b).trim().to_string()
    }
}

/// Una lista de enteros separados por comas (`"50258,50259,50359"`).
pub fn enteros(texto: &str) -> Option<Vec<i64>> {
    texto
        .split(',')
        .map(|p| p.trim().parse::<i64>().ok())
        .collect()
}

/// El token de un idioma (`es`) a partir de las dos listas paralelas de
/// los metadatos: `all_language_codes` y `all_language_tokens`.
pub fn token_del_idioma(codigos: &str, tokens: &str, idioma: &str) -> Option<i64> {
    let lengua = idioma.split('-').next()?.trim().to_lowercase();
    let tokens = enteros(tokens)?;
    let codigos: Vec<&str> = codigos.split(',').map(str::trim).collect();
    if codigos.len() != tokens.len() {
        return None;
    }
    codigos.iter().position(|c| *c == lengua).map(|i| tokens[i])
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_base64_estandar_se_lee_con_y_sin_relleno() {
        assert_eq!(base64("IQ==").unwrap(), b"!");
        assert_eq!(base64("ICgn").unwrap(), b" ('");
        assert_eq!(base64("aG9sYQ==").unwrap(), b"hola");
        assert_eq!(base64("w7E=").unwrap(), "ñ".as_bytes());
        assert_eq!(base64("5Zy6").unwrap(), "场".as_bytes());
    }

    #[test]
    fn un_base64_roto_no_se_lee_a_medias() {
        assert_eq!(base64("="), None);
        assert_eq!(base64(""), None);
        assert_eq!(base64("abc"), None, "no es multiplo de cuatro");
        assert_eq!(base64("a=bc"), None, "relleno en medio");
        assert_eq!(base64("ab*c"), None, "letra fuera del alfabeto");
        assert_eq!(base64("a==="), None, "demasiado relleno");
    }

    #[test]
    fn la_tabla_de_tokens_salta_las_lineas_raras_y_junta_bytes() {
        // La «ñ» (c3 b1) partida en dos tokens, como hace el BPE de bytes.
        let tabla = Fichas::leer("IG1h 10\nww== 11\nsQ== 12\nYW5h 13\n= 50256\nbasura\nIQ== x\n");
        assert_eq!(tabla.len(), 4);
        assert_eq!(tabla.texto(&[10, 11, 12, 13]), "mañana");
    }

    #[test]
    fn los_tokens_especiales_y_los_desconocidos_no_se_escriben() {
        let tabla = Fichas::leer("IGhvbGE= 5\n");
        assert_eq!(tabla.texto(&[50258, 5, 50257, 99_999]), "hola");
        assert_eq!(tabla.texto(&[]), "");
    }

    #[test]
    fn el_idioma_se_busca_en_las_listas_paralelas_de_los_metadatos() {
        let codigos = "en,zh,de,es";
        let tokens = "50259,50260,50261,50262";
        assert_eq!(token_del_idioma(codigos, tokens, "es"), Some(50262));
        assert_eq!(token_del_idioma(codigos, tokens, "es-PE"), Some(50262));
        assert_eq!(token_del_idioma(codigos, tokens, "EN"), Some(50259));
        assert_eq!(token_del_idioma(codigos, tokens, "ja"), None);
        // Listas de distinto largo: los metadatos estan rotos, no se adivina.
        assert_eq!(token_del_idioma("en,es", "50259", "es"), None);
    }

    #[test]
    fn una_lista_de_enteros_con_algo_que_no_es_numero_no_vale() {
        assert_eq!(
            enteros("50258, 50259,50359"),
            Some(vec![50258, 50259, 50359])
        );
        assert_eq!(enteros("50258,x"), None);
    }
}
