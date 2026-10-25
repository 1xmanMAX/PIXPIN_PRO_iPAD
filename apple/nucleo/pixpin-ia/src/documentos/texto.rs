//! **Las palabras de un texto, como las cuenta el indice.**
//!
//! Buscar «cuneta» tiene que dar con «Cunetas», «CUNETA» y «cunetas,»; y
//! «drenaje» con «Drenajes». Para eso cada palabra pasa por lo mismo, en el
//! texto y en la pregunta:
//!
//! 1. minusculas y sin tildes (`ñ` pasa a `n`: en un teclado sin ella la
//!    gente escribe «disenio» o «diseno», y lo segundo es lo que se busca);
//! 2. se parte por todo lo que no es letra o numero (`plano_cunetas-v2.pdf`
//!    da `plano`, `cunetas`, `v2`, `pdf`);
//! 3. fuera las palabras vacias del espanol y del ingles (de, la, que, the…);
//! 4. una **raiz ligera**: plurales y unas pocas terminaciones. No es un
//!    lematizador (no hace falta y pesa): solo junta lo que casi siempre es
//!    lo mismo. `cunetas`→`cunet`, `drenajes`→`drenaj`, `planos`→`plan`.
//!
//! Lo mismo, igual, hara el movil (ver `mod.rs`): son unas reglas de texto,
//! sin diccionarios.

/// Las palabras que no dicen de que va un texto.
const VACIAS: &[&str] = &[
    // espanol
    "a", "al", "algo", "como", "con", "cual", "cuando", "de", "del", "desde", "donde", "el", "ella",
    "ellos", "en", "entre", "era", "es", "esa", "ese", "eso", "esta", "este", "esto", "estos",
    "estas", "fue", "ha", "han", "hay", "la", "las", "le", "les", "lo", "los", "mas", "me", "mi",
    "mis", "muy", "ni", "no", "nos", "o", "para", "pero", "por", "que", "se", "sea", "ser", "si",
    "sin", "sobre", "son", "su", "sus", "tambien", "te", "tu", "tus", "un", "una", "uno", "unos",
    "unas", "y", "ya", "yo", // ingles
    "an", "and", "are", "as", "at", "be", "by", "for", "from", "in", "is", "it", "of", "on", "or",
    "the", "this", "to", "was", "with",
];

/// Lo que se busca en la pregunta y no en los documentos: «¿que dice la
/// tesis sobre…?», «busca en mis notas…». Sin quitarlo, «dice» o «busca»
/// puntuarian los documentos que lo llevan escrito.
const DE_LA_PREGUNTA: &[&str] = &[
    "busca",
    "buscar",
    "buscame",
    "encuentra",
    "dime",
    "dice",
    "decia",
    "dicen",
    "habla",
    "hablan",
    "menciona",
    "mencionan",
    "mande",
    "mandaste",
    "envie",
    "enviaste",
    "guarde",
    "puse",
    "tengo",
    "tenia",
    "quiero",
    "saber",
    "algun",
    "alguna",
    "documento",
    "documentos",
    "cosas",
    "archivo",
    "archivos",
];

/// Una letra sin tilde y en minuscula. `None` si no es letra ni numero.
fn plana(c: char) -> Option<char> {
    let c = match c {
        'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
        'ñ' | 'Ñ' => 'n',
        'ç' | 'Ç' => 'c',
        c if c.is_alphanumeric() => return Some(c.to_lowercase().next().unwrap_or(c)),
        _ => return None,
    };
    Some(c)
}

/// El texto en minusculas y sin tildes, con lo demas tal cual (para buscar
/// donde cae una palabra dentro de un trozo).
pub fn plano(texto: &str) -> String {
    texto.chars().map(|c| plana(c).unwrap_or(c)).collect()
}

/// **La raiz ligera** de una palabra ya plana.
pub fn raiz(p: &str) -> String {
    let n = p.chars().count();
    if n <= 3 || p.chars().any(|c| c.is_ascii_digit()) {
        return p.to_string();
    }
    // Dos pasos, para que singular y plural acaben igual («notas» y «nota»
    // en `not`): primero una terminacion (de la mas larga a la mas corta),
    // luego la vocal final. Solo si queda una raiz de 3 letras o mas.
    let quitar = |p: &str, fines: &[&str]| -> String {
        for fin in fines {
            if let Some(r) = p.strip_suffix(fin)
                && r.chars().count() >= 3
            {
                return r.to_string();
            }
        }
        p.to_string()
    };
    let p = quitar(
        p,
        &[
            "aciones", "amientos", "imientos", "amiento", "imiento", "acion", "ciones", "mente",
            "idades", "idad", "ables", "able", "ibles", "ible", "es", "s",
        ],
    );
    quitar(&p, &["a", "o", "e"])
}

/// Las palabras de un texto para el indice: planas, sin vacias y en raiz.
pub fn palabras(texto: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut actual = String::new();
    let cerrar = |actual: &mut String, v: &mut Vec<String>| {
        if !actual.is_empty() {
            let p = std::mem::take(actual);
            let largo = p.chars().count();
            let numero = p.chars().all(|c| c.is_ascii_digit());
            // Una letra suelta no dice nada; un numero suelto si (`2`, el
            // capitulo 2).
            if (largo >= 2 || numero) && largo <= 40 && !VACIAS.contains(&p.as_str()) {
                v.push(raiz(&p));
            }
        }
    };
    for c in texto.chars() {
        match plana(c) {
            Some(c) => actual.push(c),
            None => cerrar(&mut actual, &mut v),
        }
    }
    cerrar(&mut actual, &mut v);
    v
}

/// Las palabras de una **pregunta**: como [`palabras`], sin las que solo
/// son de preguntar y sin repetidas.
pub fn de_la_consulta(consulta: &str) -> Vec<String> {
    let crudas: Vec<String> = {
        let mut v = Vec::new();
        let mut actual = String::new();
        for c in consulta.chars() {
            match plana(c) {
                Some(c) => actual.push(c),
                None => {
                    if !actual.is_empty() {
                        v.push(std::mem::take(&mut actual));
                    }
                }
            }
        }
        if !actual.is_empty() {
            v.push(actual);
        }
        v
    };
    let mut fuera: Vec<String> = Vec::new();
    for p in crudas {
        if DE_LA_PREGUNTA.contains(&p.as_str()) {
            continue;
        }
        for r in palabras(&p) {
            if !fuera.contains(&r) {
                fuera.push(r);
            }
        }
    }
    fuera
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn plurales_tildes_y_mayusculas_dan_lo_mismo() {
        assert_eq!(palabras("Cunetas"), palabras("cuneta"));
        assert_eq!(palabras("DRENAJES"), palabras("drenaje"));
        assert_eq!(palabras("Diseño"), palabras("diseno"));
        assert_eq!(palabras("Pavimentación"), palabras("pavimentacion"));
        assert_eq!(
            palabras("plano_cunetas-v2.pdf"),
            vec!["plan", "cunet", "v2", "pdf"]
        );
    }

    #[test]
    fn la_pregunta_pierde_lo_que_es_de_preguntar() {
        assert_eq!(
            de_la_consulta("¿Qué dice la tesis sobre las cunetas?"),
            vec!["tesi", "cunet"]
        );
        assert_eq!(
            de_la_consulta("busca en mis notas el presupuesto"),
            vec!["not", "presupuest"]
        );
        assert!(de_la_consulta("   ").is_empty());
        assert!(de_la_consulta("¿qué dice?").is_empty());
    }

    #[test]
    fn los_numeros_se_quedan() {
        assert_eq!(
            palabras("capítulo 2, tabla 4.1"),
            vec!["capitul", "2", "tabl", "4", "1"]
        );
    }
}
