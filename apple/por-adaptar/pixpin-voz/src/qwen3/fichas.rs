//! **Los tokens de Qwen**: de texto a numeros (solo para el guion) y de
//! numeros a texto (lo que escribe el modelo).
//!
//! Qwen usa el BPE a nivel de byte de GPT-2: cada byte se escribe como una
//! letra imprimible (`bytes_to_unicode`: el espacio es `Ġ`, el salto de
//! linea `Ċ`) y `vocab.json` dice el numero de cada cadena de esas letras.
//! Para **leer** basta con deshacer eso: numeros → letras → bytes → UTF-8
//! (una «ñ» puede venir partida en dos tokens, asi que se juntan los bytes
//! de toda la frase y se convierte al final).
//!
//! Para **escribir** solo hacen falta unas pocas frases fijas del guion
//! (`<|im_start|>system`, `language Spanish`…), pero se hace bien: tokens
//! especiales enteros, el pretroceado de Qwen2 (la misma expresion regular
//! que `Qwen2TokenizerFast`, escrita a mano) y las fusiones de
//! `merges.txt` por orden. Las fusiones solo se cargan para eso y se
//! sueltan despues (son 150 000 pares).
//!
//! Texto puro: se prueba sin modelo (y con el de verdad, a mano).

use std::collections::HashMap;

/// `bytes_to_unicode` de GPT-2: la letra de cada byte.
pub fn letra_de_byte(b: u8) -> char {
    tabla_de_letras()[b as usize]
}

fn tabla_de_letras() -> &'static [char; 256] {
    static TABLA: std::sync::OnceLock<[char; 256]> = std::sync::OnceLock::new();
    TABLA.get_or_init(|| {
        let mut t = ['\0'; 256];
        let mut extra = 0u32;
        for b in 0..=255u8 {
            let imprimible = matches!(b, b'!'..=b'~' | 0xA1..=0xAC | 0xAE..=0xFF);
            t[b as usize] = if imprimible {
                char::from(b)
            } else {
                let c = char::from_u32(256 + extra).unwrap_or('\0');
                extra += 1;
                c
            };
        }
        t
    })
}

/// La vuelta: el byte de cada letra.
fn byte_de_letra() -> &'static HashMap<char, u8> {
    static TABLA: std::sync::OnceLock<HashMap<char, u8>> = std::sync::OnceLock::new();
    TABLA.get_or_init(|| {
        tabla_de_letras()
            .iter()
            .enumerate()
            .map(|(b, &c)| (c, b as u8))
            .collect()
    })
}

/// **El pretroceado de Qwen2**, la expresion de `Qwen2TokenizerFast`:
///
/// ```text
/// (?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?\p{L}+|\p{N}
/// | ?[^\s\p{L}\p{N}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+
/// ```
///
/// Se prueba alternativa a alternativa en cada posicion, como el motor de
/// expresiones (gana la primera que casa, no la mas larga).
pub fn pretrocear(texto: &str) -> Vec<&str> {
    let c: Vec<(usize, char)> = texto.char_indices().collect();
    let fin = texto.len();
    let byte_en = |i: usize| c.get(i).map_or(fin, |p| p.0);
    let letra = |ch: char| ch.is_alphabetic();
    let numero = |ch: char| ch.is_numeric();
    let blanco = |ch: char| ch.is_whitespace();
    let salto = |ch: char| ch == '\r' || ch == '\n';
    let mut v = Vec::new();
    let mut i = 0usize;
    while i < c.len() {
        let ch = c[i].1;
        let largo = 'casa: {
            // 1. Contracciones.
            if ch == '\'' {
                for suf in ["s", "t", "re", "ve", "m", "ll", "d"] {
                    let resto = &texto[c[i].0 + 1..];
                    if resto.len() >= suf.len()
                        && resto.is_char_boundary(suf.len())
                        && resto[..suf.len()].eq_ignore_ascii_case(suf)
                    {
                        break 'casa 1 + suf.chars().count();
                    }
                }
            }
            // 2. Un signo opcional (ni salto, ni letra, ni numero) y letras.
            {
                let mut j = i;
                if !salto(ch) && !letra(ch) && !numero(ch) {
                    j += 1;
                }
                let k = (j..c.len()).take_while(|&k| letra(c[k].1)).count();
                if k > 0 {
                    break 'casa j + k - i;
                }
            }
            // 3. Una cifra suelta.
            if numero(ch) {
                break 'casa 1;
            }
            // 4. ` ?[^\s\p{L}\p{N}]+[\r\n]*`.
            {
                let mut j = i;
                if ch == ' ' {
                    j += 1;
                }
                let k = (j..c.len())
                    .take_while(|&k| {
                        let x = c[k].1;
                        !blanco(x) && !letra(x) && !numero(x)
                    })
                    .count();
                if k > 0 {
                    let s = (j + k..c.len()).take_while(|&k| salto(c[k].1)).count();
                    break 'casa j + k + s - i;
                }
            }
            // 5. `\s*[\r\n]+`: blancos que acaban en saltos (el ultimo
            // salto del tramo de blancos manda).
            {
                let tramo = (i..c.len()).take_while(|&k| blanco(c[k].1)).count();
                if let Some(ultimo) = (i..i + tramo).rev().find(|&k| salto(c[k].1)) {
                    break 'casa ultimo + 1 - i;
                }
                // 6. `\s+(?!\S)`: todos los blancos menos el ultimo si
                // detras viene algo que no es blanco.
                if tramo > 0 {
                    let detras = i + tramo < c.len();
                    if !detras {
                        break 'casa tramo;
                    }
                    if tramo > 1 {
                        break 'casa tramo - 1;
                    }
                    // 7. `\s+`.
                    break 'casa tramo;
                }
            }
            1
        };
        v.push(&texto[c[i].0..byte_en(i + largo)]);
        i += largo;
    }
    v
}

/// **La tabla de tokens**: numero a bytes, y los especiales por nombre.
#[derive(Debug, Default)]
pub struct Fichas {
    /// Los bytes de cada token normal, por su numero.
    bytes: Vec<Vec<u8>>,
    /// Los especiales (`<|im_start|>`, `<asr_text>`…) y su numero.
    especiales: HashMap<String, i64>,
    /// La cadena de letras de cada token normal, para escribir.
    numero: HashMap<String, i64>,
}

impl Fichas {
    /// Lee `vocab.json` (`{"!": 0, ...}`) y los especiales de
    /// `tokenizer_config.json` (`added_tokens_decoder`). `None` si alguno no
    /// se entiende.
    pub fn leer(vocab: &str, config: &str) -> Option<Fichas> {
        let vocab: HashMap<String, i64> = serde_json::from_str(vocab).ok()?;
        let config: serde_json::Value = serde_json::from_str(config).ok()?;
        let mut especiales = HashMap::new();
        for (id, v) in config.get("added_tokens_decoder")?.as_object()? {
            let id: i64 = id.parse().ok()?;
            especiales.insert(v.get("content")?.as_str()?.to_string(), id);
        }
        let mas_alto = vocab.values().copied().max()?.max(0) as usize;
        let mut bytes = vec![Vec::new(); mas_alto + 1];
        let tabla = byte_de_letra();
        for (cadena, &id) in &vocab {
            if id < 0 {
                continue;
            }
            bytes[id as usize] = cadena
                .chars()
                .filter_map(|c| tabla.get(&c).copied())
                .collect();
        }
        Some(Fichas {
            bytes,
            especiales,
            numero: vocab,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// El numero de un token especial (`<asr_text>`…).
    pub fn especial(&self, nombre: &str) -> Option<i64> {
        self.especiales.get(nombre).copied()
    }

    /// **De numeros a texto**: los bytes de cada token normal, juntos, a
    /// UTF-8. Los especiales no se escriben; lo que no sea UTF-8 valido
    /// (un token partido al final) se tira.
    pub fn texto(&self, ids: &[i64]) -> String {
        let mut b = Vec::new();
        for &i in ids {
            if let Some(t) = usize::try_from(i).ok().and_then(|i| self.bytes.get(i)) {
                b.extend_from_slice(t);
            }
        }
        String::from_utf8_lossy(&b).replace('\u{FFFD}', "")
    }

    /// **De texto a numeros**, con las fusiones de `merges.txt`. Los
    /// especiales que aparezcan enteros en el texto van como un token.
    pub fn codificar(&self, texto: &str, fusiones: &Fusiones) -> Vec<i64> {
        let mut v = Vec::new();
        let mut resto = texto;
        while !resto.is_empty() {
            // El especial que empiece antes (y el mas largo si empatan).
            let siguiente = self
                .especiales
                .iter()
                .filter_map(|(n, &id)| resto.find(n.as_str()).map(|p| (p, n.len(), id)))
                .min_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
            let (hasta, especial) = match siguiente {
                Some((p, largo, id)) => (p, Some((largo, id))),
                None => (resto.len(), None),
            };
            for pieza in pretrocear(&resto[..hasta]) {
                self.bpe(pieza, fusiones, &mut v);
            }
            match especial {
                Some((largo, id)) => {
                    v.push(id);
                    resto = &resto[hasta + largo..];
                }
                None => break,
            }
        }
        v
    }

    fn bpe(&self, pieza: &str, fusiones: &Fusiones, salida: &mut Vec<i64>) {
        let mut partes: Vec<String> = pieza
            .bytes()
            .map(|b| letra_de_byte(b).to_string())
            .collect();
        loop {
            let mejor = partes
                .windows(2)
                .enumerate()
                .filter_map(|(i, p)| fusiones.rango(&p[0], &p[1]).map(|r| (r, i)))
                .min();
            let Some((_, i)) = mejor else { break };
            let junta = format!("{}{}", partes[i], partes[i + 1]);
            partes[i] = junta;
            partes.remove(i + 1);
        }
        for p in partes {
            match self.numero.get(&p) {
                Some(&id) => salida.push(id),
                // No deberia pasar con el vocabulario de Qwen (cada byte
                // suelto es un token); si pasa, byte a byte.
                None => salida.extend(
                    p.chars()
                        .filter_map(|c| self.numero.get(&c.to_string()).copied()),
                ),
            }
        }
    }
}

/// Las fusiones de `merges.txt`, con su orden (antes = se fusiona antes).
#[derive(Debug, Default)]
pub struct Fusiones {
    rango: HashMap<(String, String), usize>,
}

impl Fusiones {
    /// Lee `merges.txt`: una fusion por linea, `a b`; la primera linea es
    /// la cabecera (`#version: ...`).
    pub fn leer(texto: &str) -> Fusiones {
        let rango = texto
            .lines()
            .filter(|l| !l.starts_with("#version") && !l.is_empty())
            .filter_map(|l| l.split_once(' '))
            .enumerate()
            .map(|(i, (a, b))| ((a.to_string(), b.to_string()), i))
            .collect();
        Fusiones { rango }
    }

    fn rango(&self, a: &str, b: &str) -> Option<usize> {
        self.rango.get(&(a.to_string(), b.to_string())).copied()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_tabla_de_gpt2_deja_lo_imprimible_y_mueve_lo_demas() {
        assert_eq!(letra_de_byte(b'a'), 'a');
        assert_eq!(letra_de_byte(b' '), 'Ġ');
        assert_eq!(letra_de_byte(b'\n'), 'Ċ');
        assert_eq!(letra_de_byte(0xC3), 'Ã');
        let t = byte_de_letra();
        assert_eq!(t.len(), 256, "ninguna letra repetida");
    }

    #[test]
    fn el_pretroceado_parte_como_la_expresion_de_qwen2() {
        // Salidas de `pre_tokenizers.Split(PAT)` de `tokenizers` 0.23.
        assert_eq!(pretrocear("language Spanish"), ["language", " Spanish"]);
        assert_eq!(pretrocear("system\n"), ["system", "\n"]);
        assert_eq!(
            pretrocear("¿Qué tal? Año 2026, ñandú."),
            [
                "¿Qué", " tal", "?", " Año", " ", "2", "0", "2", "6", ",", " ñandú", "."
            ]
        );
        assert_eq!(pretrocear("hola  mundo"), ["hola", " ", " mundo"]);
        assert_eq!(pretrocear("a\n\nb  "), ["a", "\n\n", "b", "  "]);
        assert_eq!(pretrocear("it's OK"), ["it", "'s", " OK"]);
        assert_eq!(pretrocear("\n  x"), ["\n", " ", " x"]);
        assert_eq!(pretrocear("x \n \ny"), ["x", " \n \n", "y"]);
        assert_eq!(pretrocear("a, b!!\n\nc"), ["a", ",", " b", "!!\n\n", "c"]);
        assert_eq!(pretrocear(""), Vec::<&str>::new());
    }

    fn mini() -> (Fichas, Fusiones) {
        // Un vocabulario de juguete: los bytes sueltos que hacen falta y
        // dos fusiones.
        let vocab = r#"{"h":0,"o":1,"l":2,"a":3,"ho":4,"hola":5,"Ġ":6,"la":7,"Ã":8,"±":9}"#;
        let config = r#"{"added_tokens_decoder":{"100":{"content":"<|fin|>","special":true}}}"#;
        let f = Fichas::leer(vocab, config).unwrap();
        let fus = Fusiones::leer("#version: 0.2\nh o\nl a\nho la\n");
        (f, fus)
    }

    #[test]
    fn las_fusiones_van_por_orden_y_los_especiales_enteros() {
        let (f, fus) = mini();
        assert_eq!(f.codificar("hola", &fus), [5]);
        assert_eq!(f.codificar("hola<|fin|>la", &fus), [5, 100, 7]);
        assert_eq!(f.codificar("<|fin|>", &fus), [100]);
        assert_eq!(f.especial("<|fin|>"), Some(100));
        assert_eq!(f.especial("<|otro|>"), None);
    }

    #[test]
    fn los_bytes_de_varios_tokens_se_juntan_antes_de_pasar_a_utf8() {
        let (f, _) = mini();
        // «ñ» es C3 B1: «Ã» y «±» por separado.
        assert_eq!(f.texto(&[5, 6, 8, 9]), "hola ñ");
        // Un especial o un numero fuera de la tabla no se escribe.
        assert_eq!(f.texto(&[5, 100, -1]), "hola");
        // Medio caracter al final se tira.
        assert_eq!(f.texto(&[5, 8]), "hola");
    }

    /// Con el tokenizador de verdad (`PIXPIN_QWEN3`, la carpeta bajada):
    /// las frases del guion dan los mismos numeros que `tokenizers`.
    #[test]
    #[ignore = "pide los ficheros de Qwen3-ASR bajados"]
    fn el_guion_da_los_numeros_de_tokenizers() {
        let d = std::path::PathBuf::from(std::env::var("PIXPIN_QWEN3").expect("PIXPIN_QWEN3"));
        let leer = |n: &str| std::fs::read_to_string(d.join(n)).unwrap();
        let f = Fichas::leer(&leer("vocab.json"), &leer("tokenizer_config.json")).unwrap();
        let fus = Fusiones::leer(&leer("merges.txt"));
        assert_eq!(
            f.codificar(
                "<|im_start|>system\n<|im_end|>\n<|im_start|>user\n<|audio_start|>",
                &fus
            ),
            [151644, 8948, 198, 151645, 198, 151644, 872, 198, 151669]
        );
        assert_eq!(
            f.codificar(
                "<|audio_end|><|im_end|>\n<|im_start|>assistant\nlanguage Spanish<asr_text>",
                &fus
            ),
            [
                151670, 151645, 198, 151644, 77091, 198, 11528, 15154, 151704
            ]
        );
        let frase = "¿Qué tal? El año 2026 fue muy boscoso, ñandú.";
        assert_eq!(f.texto(&f.codificar(frase, &fus)), frase);
    }
}
