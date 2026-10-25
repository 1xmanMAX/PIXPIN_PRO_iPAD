//! **Si lo que dijo el reconocedor parece de verdad.**
//!
//! Puerto de `Transcriptor.creible` (`Transcriptor.kt:298-324`). Nacio
//! contra Whisper, que cuando le llega ruido o silencio se saca frases de
//! la nada —en otro alfabeto, o los creditos de subtitulos con que se
//! entreno—, pero se conserva entero aunque aqui el motor sea Vosk por tres
//! razones: un modelo pequeno tambien suelta palabras sueltas donde solo
//! hay ruido de obra; el filtro no tira nada que sea texto normal; y el dia
//! que se anada otro motor ya esta puesto.
//!
//! Texto puro: se prueba entero.

/// Lo que Whisper suelta cuando no oye nada. `JUNK` (`Transcriptor.kt:281-286`).
const BASURA: [&str; 9] = [
    "subtítulos realizados por",
    "subtitulos realizados por",
    "amara.org",
    "subtítulos por",
    "gracias por ver",
    "suscríbete",
    "suscribete",
    "thanks for watching",
    "thank you for watching",
];

/// Idiomas de alfabeto latino: en los demas no se puede mirar el alfabeto,
/// porque el alfabeto «raro» es el bueno. `LATINOS` (`Transcriptor.kt:326`).
pub const LATINOS: [&str; 22] = [
    "es", "en", "pt", "fr", "de", "it", "ca", "nl", "pl", "ro", "sv", "da", "no", "fi", "cs", "hu",
    "tr", "id", "ms", "vi", "eu", "gl",
];

/// Cuantas veces seguidas puede repetirse la misma palabra antes de que
/// deje de parecer una frase. Ocho, como en el movil.
const REPETICIONES: usize = 8;

/// Una letra del alfabeto latino, con o sin tilde.
///
/// El movil pregunta `Character.UnicodeScript.of(c)`; aqui se miran los
/// bloques a mano para no arrastrar una tabla Unicode entera a un crate de
/// cimientos. Cubre lo mismo que hace falta: latino basico, latino con
/// tildes (suplemento y extendidos A y B), fonetico y el extendido
/// adicional del vietnamita.
fn es_latina(c: char) -> bool {
    matches!(c,
        'A'..='Z' | 'a'..='z'
        | '\u{00C0}'..='\u{024F}'
        | '\u{0250}'..='\u{02AF}'
        | '\u{1E00}'..='\u{1EFF}')
}

/// Si un trozo de texto parece de verdad y no una invencion del modelo.
///
/// `idioma` es la etiqueta del usuario (`es`, `es-PE`, `en-US`): solo se
/// mira la parte de delante.
pub fn creible(texto: &str, idioma: &str) -> bool {
    let t = texto.trim();
    if t.is_empty() {
        return false;
    }

    let lengua = idioma.split('-').next().unwrap_or("").to_lowercase();
    if LATINOS.contains(&lengua.as_str()) {
        let mut letras = 0usize;
        let mut raras = 0usize;
        for c in t.chars() {
            if !c.is_alphabetic() {
                continue;
            }
            letras += 1;
            if !es_latina(c) {
                raras += 1;
            }
        }
        // Una letra suelta de otro alfabeto puede ser un nombre propio; a
        // partir de un quinto del texto ya no es un nombre, es otra lengua.
        if raras > 0 && raras * 5 >= letras {
            return false;
        }
    }

    let bajo = t.to_lowercase();
    if BASURA.iter().any(|b| bajo.contains(b)) {
        return false;
    }

    // «no no no no no no no no»: la misma palabra ocho veces seguidas.
    let palabras: Vec<&str> = bajo.split(' ').filter(|p| !p.is_empty()).collect();
    if palabras.len() >= REPETICIONES {
        let mut seguidas = 1usize;
        for par in palabras.windows(2) {
            seguidas = if par[0] == par[1] { seguidas + 1 } else { 1 };
            if seguidas >= REPETICIONES {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_frase_normal_en_castellano_pasa() {
        assert!(creible("llamar al aparejador el lunes", "es"));
        assert!(creible("mañana subimos el encofrado", "es-PE"));
    }

    #[test]
    fn un_texto_vacio_o_solo_blancos_no_es_creible() {
        assert!(!creible("", "es"));
        assert!(!creible("   \n\t ", "es"));
    }

    #[test]
    fn un_audio_que_no_se_entiende_y_sale_en_otro_alfabeto_se_tira() {
        assert!(!creible("这是一个测试", "es"));
        assert!(!creible("привет как дела", "en"));
    }

    #[test]
    fn un_nombre_propio_de_otro_alfabeto_dentro_de_una_frase_larga_sobrevive() {
        // Una letra rara entre muchas latinas no llega al quinto.
        assert!(creible(
            "el proveedor se llama Ω y trae el material el martes por la manana",
            "es"
        ));
    }

    #[test]
    fn en_un_idioma_que_no_es_latino_no_se_mira_el_alfabeto() {
        assert!(creible("привет как дела", "ru"));
    }

    #[test]
    fn los_creditos_de_subtitulos_con_que_se_entreno_el_modelo_se_tiran() {
        assert!(!creible(
            "Subtítulos realizados por la comunidad de Amara.org",
            "es"
        ));
        assert!(!creible("Thanks for watching!", "en"));
        // Y aunque vengan pegados a algo mas.
        assert!(!creible("hola suscríbete al canal", "es"));
    }

    #[test]
    fn la_misma_palabra_ocho_veces_seguidas_no_es_una_frase() {
        assert!(!creible("no no no no no no no no", "es"));
        // Siete si lo es: el corte esta en ocho, como en el movil.
        assert!(creible("no no no no no no no", "es"));
    }
}
