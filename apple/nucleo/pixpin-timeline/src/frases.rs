//! **Partir lo dicho en titulo y descripcion** por frases clave.
//!
//! El usuario lo pidio asi (5-oct-2026): «que inicie con "con esto paso" y lo
//! que se diga despues se ponga como titulo; luego "y asi te lo cuento" y lo
//! siguiente se ponga como descripcion». Se le propusieron estas frases y
//! siguio adelante con ellas:
//!
//! - titulo: lo que va tras «con esto paso» o «paso que»;
//! - descripcion: lo que va tras «y asi te lo cuento», «te lo cuento» o
//!   «te cuento».
//!
//! Las frases clave no se guardan. Se buscan **sin mayusculas, sin tildes y
//! sin signos**, porque Whisper escribe lo mismo de muchas maneras («Con
//! esto pasó:», «con esto paso,»…). Sin ninguna frase, la primera linea (al
//! escribir) o la primera oracion (al dictar) hace de titulo y el resto de
//! descripcion: un momento nunca se queda sin titulo.

/// Las frases que abren el titulo, la mas larga primero (asi «paso que» no
/// se come un trozo de otra mas larga que empiece igual).
pub const DEL_TITULO: &[&str] = &["con esto paso", "paso que"];

/// Las frases que abren la descripcion, la mas larga primero.
pub const DE_LA_DESCRIPCION: &[&str] = &[
    "y asi te lo cuento",
    "asi te lo cuento",
    "te lo cuento",
    "te cuento",
];

/// Lo que mide como mucho un titulo sacado de una oracion sin punto: lo
/// dicho de un tiron sin frases clave no cabe entero en un titulo.
const TITULO_LARGO: usize = 90;

/// Titulo y descripcion de `texto`.
pub fn partir(texto: &str) -> (String, String) {
    let n = Normal::de(texto);
    let t = n.buscar(DEL_TITULO, 0);
    let d = n.buscar(DE_LA_DESCRIPCION, t.map_or(0, |(_, fin)| fin));
    let byte = |i: usize| n.byte(i, texto.len());
    let (titulo, desc) = match (t, d) {
        (Some((ti, tf)), Some((di, df))) => {
            let antes = &texto[..byte(ti)];
            let titulo = &texto[byte(tf)..byte(di)];
            let desc = juntar(antes, &texto[byte(df)..]);
            if limpio(titulo).is_empty() {
                // «con esto paso, y asi te lo cuento: …»: sin titulo dicho,
                // lo saca la descripcion.
                let (t, d) = primera_frase(&desc);
                (t, d)
            } else {
                (titulo.to_string(), desc)
            }
        }
        (Some((ti, tf)), None) => {
            let antes = &texto[..byte(ti)];
            let (t, d) = primera_frase(&texto[byte(tf)..]);
            (t, juntar(antes, &d))
        }
        (None, Some((di, df))) => {
            let (t, extra) = primera_frase(&texto[..byte(di)]);
            (t, juntar(&extra, &texto[byte(df)..]))
        }
        (None, None) => primera_frase(texto),
    };
    let titulo = titulo_limpio(&titulo);
    let desc = limpio(&desc);
    if titulo.is_empty() && !desc.is_empty() {
        // Solo habia descripcion: que haga de titulo lo primero de ella.
        let (t, d) = primera_frase(&desc);
        return (titulo_limpio(&t), limpio(&d));
    }
    (titulo, desc)
}

/// La primera linea o la primera oracion, y lo demas.
fn primera_frase(s: &str) -> (String, String) {
    let s = s.trim();
    if let Some(i) = s.find('\n') {
        return (s[..i].to_string(), s[i + 1..].to_string());
    }
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if matches!(c, '.' | '!' | '?' | '…') {
            let sigue = chars.peek().map(|(_, c)| *c);
            if sigue.is_none_or(char::is_whitespace) {
                let fin = i + c.len_utf8();
                return (s[..fin].to_string(), s[fin..].to_string());
            }
        }
    }
    if s.chars().count() <= TITULO_LARGO {
        return (s.to_string(), String::new());
    }
    // Sin punto y muy largo: se corta en el ultimo blanco antes del tope.
    let tope = s
        .char_indices()
        .nth(TITULO_LARGO)
        .map_or(s.len(), |(i, _)| i);
    let corte = s[..tope].rfind(char::is_whitespace).unwrap_or(tope);
    (s[..corte].to_string(), s[corte..].to_string())
}

fn juntar(a: &str, b: &str) -> String {
    let (a, b) = (limpio(a), limpio(b));
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b,
        (_, true) => a,
        _ => format!("{a}\n{b}"),
    }
}

/// Sin blancos ni signos sueltos en los bordes (los que deja quitar la
/// frase clave: «, », «: », «- »).
fn limpio(s: &str) -> String {
    let borde = |c: char| c.is_whitespace() || matches!(c, ',' | ':' | ';' | '-' | '–' | '—');
    s.trim_matches(borde).to_string()
}

/// Como [`limpio`], sin punto final y con la primera en mayuscula.
fn titulo_limpio(s: &str) -> String {
    let t = limpio(s);
    let t = t.trim_end_matches(['.', ',']).trim_end();
    let mut c = t.chars();
    match c.next() {
        Some(p) => p.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// El texto como se comparan las frases: minusculas, sin tildes, los signos
/// hechos blancos y los blancos de uno en uno. Cada letra recuerda de que
/// bytes del original salio, para cortar el original y no la copia.
struct Normal {
    letras: Vec<char>,
    /// `(inicio, fin)` en bytes del original de cada letra de `letras`.
    origen: Vec<(usize, usize)>,
}

impl Normal {
    fn de(texto: &str) -> Normal {
        let mut letras = Vec::new();
        let mut origen = Vec::new();
        for (i, c) in texto.char_indices() {
            let fin = i + c.len_utf8();
            let c = sin_tilde(c.to_lowercase().next().unwrap_or(c));
            let c = if c.is_alphanumeric() { c } else { ' ' };
            if c == ' ' && letras.last().is_none_or(|u| *u == ' ') {
                continue;
            }
            letras.push(c);
            origen.push((i, fin));
        }
        Normal { letras, origen }
    }

    /// La primera aparicion, desde la letra `desde`, de cualquiera de las
    /// `frases` como palabras enteras: `(primera letra, letra tras la ultima)`.
    fn buscar(&self, frases: &[&str], desde: usize) -> Option<(usize, usize)> {
        let mut mejor: Option<(usize, usize)> = None;
        for f in frases {
            let f: Vec<char> = f.chars().collect();
            if f.len() > self.letras.len() {
                continue;
            }
            for i in desde..=self.letras.len() - f.len() {
                let fin = i + f.len();
                let entera = (i == 0 || self.letras[i - 1] == ' ')
                    && (fin == self.letras.len() || self.letras[fin] == ' ');
                if entera && self.letras[i..fin] == f[..] {
                    if mejor.is_none_or(|(m, _)| i < m) {
                        mejor = Some((i, fin));
                    }
                    break;
                }
            }
        }
        mejor
    }

    /// El byte del original donde empieza la letra `i` (o el final).
    fn byte(&self, i: usize, largo: usize) -> usize {
        self.origen.get(i).map_or(largo, |(b, _)| *b)
    }
}

pub(crate) fn sin_tilde(c: char) -> char {
    match c {
        'á' | 'à' | 'ä' | 'â' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        'ñ' => 'n',
        otra => otra,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn p(s: &str) -> (String, String) {
        partir(s)
    }

    #[test]
    fn las_dos_frases_dan_titulo_y_descripcion() {
        assert_eq!(
            p("con esto pasó se cayó el servidor y así te lo cuento reinicié y volvió a las 10:40"),
            (
                "Se cayó el servidor".into(),
                "reinicié y volvió a las 10:40".into()
            )
        );
    }

    #[test]
    fn da_igual_como_lo_escriba_whisper() {
        assert_eq!(
            p("Con esto pasó: llegó el pedido. Y así te lo cuento, venía roto."),
            ("Llegó el pedido".into(), "venía roto.".into())
        );
        assert_eq!(
            p("CON ESTO PASO, reunión con Juan; te cuento: todo bien"),
            ("Reunión con Juan".into(), "todo bien".into())
        );
    }

    #[test]
    fn paso_que_tambien_abre_el_titulo() {
        assert_eq!(
            p("pasó que se fue la luz te lo cuento duró diez minutos"),
            ("Se fue la luz".into(), "duró diez minutos".into())
        );
    }

    #[test]
    fn sin_frase_de_descripcion_la_primera_oracion_es_el_titulo() {
        assert_eq!(
            p("con esto pasó terminé el informe. Lo mandé a las cinco."),
            ("Terminé el informe".into(), "Lo mandé a las cinco.".into())
        );
    }

    #[test]
    fn sin_frase_de_titulo_lo_de_antes_es_el_titulo() {
        assert_eq!(
            p("Almuerzo con el equipo, te cuento: pedimos pizza"),
            ("Almuerzo con el equipo".into(), "pedimos pizza".into())
        );
    }

    #[test]
    fn sin_frases_manda_la_primera_linea_o_la_primera_oracion() {
        assert_eq!(
            p("Café\ncon leche y tostadas"),
            ("Café".into(), "con leche y tostadas".into())
        );
        assert_eq!(
            p("salí a correr. Cinco kilómetros"),
            ("Salí a correr".into(), "Cinco kilómetros".into())
        );
        assert_eq!(p("hola"), ("Hola".into(), String::new()));
    }

    #[test]
    fn las_frases_solo_cuentan_como_palabras_enteras() {
        // «repasó que» no es «pasó que»; «te cuentan» no es «te cuento».
        assert_eq!(
            p("repasó que todo estuviera bien"),
            ("Repasó que todo estuviera bien".into(), String::new())
        );
        assert_eq!(
            p("me dijeron que te cuentan todo"),
            ("Me dijeron que te cuentan todo".into(), String::new())
        );
    }

    #[test]
    fn lo_dicho_antes_de_la_frase_del_titulo_no_se_pierde() {
        assert_eq!(
            p("eh bueno con esto pasó llegué tarde te cuento había tráfico"),
            ("Llegué tarde".into(), "eh bueno\nhabía tráfico".into())
        );
    }

    #[test]
    fn titulo_vacio_lo_saca_de_la_descripcion() {
        assert_eq!(
            p("con esto pasó, y así te lo cuento: se rompió la taza. Era nueva"),
            ("Se rompió la taza".into(), "Era nueva".into())
        );
    }

    #[test]
    fn un_tiron_muy_largo_sin_punto_se_corta_en_un_blanco() {
        let largo = "palabra ".repeat(30);
        let (t, d) = p(&largo);
        assert!(t.chars().count() <= TITULO_LARGO);
        assert!(!t.ends_with(' '));
        assert!(!d.is_empty());
    }

    #[test]
    fn texto_vacio_da_vacio() {
        assert_eq!(p("   "), (String::new(), String::new()));
    }
}
