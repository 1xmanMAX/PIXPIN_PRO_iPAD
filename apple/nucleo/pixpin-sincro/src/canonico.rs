//! El JSON canonico y el resumen de un texto, como los saca Android.
//!
//! Es el puerto de `Canonico` (final de `sincro/Disco.kt`) y de la funcion
//! `sha256` de `Identidad.kt`. Dos aparatos comparan **resumenes** para saber
//! si algo cambio, y el resumen se calcula sobre el TEXTO canonico: claves
//! ordenadas, sin nulos en los objetos, y todo lo demas tal como vino. Si
//! aqui saliera un solo caracter distinto, cada vuelta de sincronizacion
//! veria «cambiado» lo que esta igual y mandaria archivos enteros.
//!
//! Por eso este modulo NO trabaja sobre `serde_json::Value`: kotlinx guarda
//! cada numero como el texto que leyo (`1.50`, `1.0E-5`) y lo vuelve a
//! escribir tal cual, mientras que serde_json lo convierte a `f64` y lo
//! imprime a su manera (`1.5`, `1e-5`). Los lienzos y los croquis estan
//! llenos de numeros con decimales, asi que la unica forma de salir byte a
//! byte igual es un arbol propio ([Json]) que conserve el literal. La otra
//! salida, la feature `arbitrary_precision` de serde_json, se descarto: se
//! propaga a todo el espacio de trabajo y rompe los `#[serde(flatten)]` y
//! `#[serde(untagged)]` que ya usa `pixpin-proyecto`.

use std::cmp::Ordering;
use std::fmt;

use sha2::{Digest, Sha256};

/// Hasta donde se anida antes de rendirse. kotlinx no tiene tope, pero
/// aqui un JSON hostil no puede tirar la pila: mas hondo que esto se toma
/// por no-JSON y `de` devuelve el texto tal cual.
const HONDURA_MAXIMA: usize = 512;

/// Un arbol JSON con la semantica de `JsonElement` de kotlinx.
///
/// - Un [Json::Literal] es lo que vino sin comillas (numero, `true`,
///   `false`) y se guarda como TEXTO: `1` y `1.0` son distintos, igual que
///   en kotlinx, y se escriben como se leyeron.
/// - Un [Json::Objeto] recuerda el orden en que llegaron sus claves, pero
///   dos objetos son iguales aunque las tengan en otro orden (kotlinx
///   compara `JsonObject` como un `Map`).
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Nulo,
    Literal(String),
    Cadena(String),
    Lista(Vec<Json>),
    Objeto(Objeto),
}

/// Un objeto JSON con el orden de llegada de sus claves y sin repetidas.
#[derive(Debug, Clone, Default)]
pub struct Objeto {
    pares: Vec<(String, Json)>,
}

impl Objeto {
    pub fn nuevo() -> Objeto {
        Objeto::default()
    }

    /// De una lista de pares. Una clave repetida se queda con el ultimo
    /// valor en el sitio del primero, como un `LinkedHashMap`.
    pub fn de(pares: Vec<(String, Json)>) -> Objeto {
        let mut o = Objeto::nuevo();
        for (k, v) in pares {
            o.poner(k, v);
        }
        o
    }

    pub fn obtener(&self, clave: &str) -> Option<&Json> {
        self.pares.iter().find(|(k, _)| k == clave).map(|(_, v)| v)
    }

    pub fn contiene(&self, clave: &str) -> bool {
        self.pares.iter().any(|(k, _)| k == clave)
    }

    /// Pone o sustituye. Si la clave ya estaba, conserva su sitio: es lo que
    /// hace `LinkedHashMap`, y de ahi sale el orden de lo que se escribe.
    pub fn poner(&mut self, clave: impl Into<String>, valor: Json) {
        let clave = clave.into();
        match self.pares.iter_mut().find(|(k, _)| *k == clave) {
            Some(par) => par.1 = valor,
            None => self.pares.push((clave, valor)),
        }
    }

    pub fn quitar(&mut self, clave: &str) -> Option<Json> {
        let i = self.pares.iter().position(|(k, _)| k == clave)?;
        Some(self.pares.remove(i).1)
    }

    pub fn claves(&self) -> impl Iterator<Item = &str> {
        self.pares.iter().map(|(k, _)| k.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Json)> {
        self.pares.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn len(&self) -> usize {
        self.pares.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pares.is_empty()
    }
}

impl PartialEq for Objeto {
    /// Sin mirar el orden: `{"a":1,"b":2}` y `{"b":2,"a":1}` son el mismo
    /// objeto para kotlinx, y de esa igualdad dependen «no cambio nada» y
    /// «una figura hecha de las dos» en la fusion.
    fn eq(&self, otro: &Objeto) -> bool {
        self.pares.len() == otro.pares.len()
            && self
                .pares
                .iter()
                .all(|(k, v)| otro.obtener(k).is_some_and(|w| w == v))
    }
}

/// Por que un texto no es JSON.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("JSON mal formado en el byte {posicion}: {motivo}")]
pub struct ErrorDeJson {
    pub posicion: usize,
    pub motivo: &'static str,
}

impl Json {
    pub fn cadena(s: impl Into<String>) -> Json {
        Json::Cadena(s.into())
    }

    /// Un numero entero, escrito como lo escribiria kotlinx un `Long`.
    pub fn numero(n: i64) -> Json {
        Json::Literal(n.to_string())
    }

    pub fn booleano(b: bool) -> Json {
        Json::Literal(if b { "true" } else { "false" }.to_string())
    }

    pub fn como_objeto(&self) -> Option<&Objeto> {
        match self {
            Json::Objeto(o) => Some(o),
            _ => None,
        }
    }

    pub fn como_lista(&self) -> Option<&[Json]> {
        match self {
            Json::Lista(l) => Some(l),
            _ => None,
        }
    }

    pub fn como_cadena(&self) -> Option<&str> {
        match self {
            Json::Cadena(s) => Some(s),
            _ => None,
        }
    }

    /// `JsonPrimitive.content` de kotlinx: el texto de una cadena, de un
    /// literal, o `null`. Nada para listas y objetos.
    pub fn contenido(&self) -> Option<&str> {
        match self {
            Json::Nulo => Some("null"),
            Json::Literal(t) | Json::Cadena(t) => Some(t),
            _ => None,
        }
    }

    pub fn es_primitivo(&self) -> bool {
        !matches!(self, Json::Lista(_) | Json::Objeto(_))
    }

    /// Lee un texto JSON. Acepta lo que acepta `Json.parseToJsonElement`:
    /// JSON estricto, y ademas cualquier palabra sin comillas como literal,
    /// que es lo que hace el lector de arboles de kotlinx aunque no este en
    /// modo permisivo.
    pub fn analizar(texto: &str) -> Result<Json, ErrorDeJson> {
        let mut lector = Lector {
            bytes: texto.as_bytes(),
            texto,
            pos: 0,
        };
        lector.saltar_blancos();
        let valor = lector.valor(0)?;
        lector.saltar_blancos();
        if lector.pos != lector.bytes.len() {
            return Err(lector.error("sobra texto despues del valor"));
        }
        Ok(valor)
    }

    /// El texto compacto, como `JsonElement.toString()` de kotlinx.
    pub fn a_texto(&self) -> String {
        let mut salida = String::new();
        self.escribir(&mut salida);
        salida
    }

    fn escribir(&self, salida: &mut String) {
        match self {
            Json::Nulo => salida.push_str("null"),
            Json::Literal(t) => salida.push_str(t),
            Json::Cadena(s) => escribir_entrecomillada(s, salida),
            Json::Lista(l) => {
                salida.push('[');
                for (i, e) in l.iter().enumerate() {
                    if i > 0 {
                        salida.push(',');
                    }
                    e.escribir(salida);
                }
                salida.push(']');
            }
            Json::Objeto(o) => {
                salida.push('{');
                for (i, (k, v)) in o.iter().enumerate() {
                    if i > 0 {
                        salida.push(',');
                    }
                    escribir_entrecomillada(k, salida);
                    salida.push(':');
                    v.escribir(salida);
                }
                salida.push('}');
            }
        }
    }

    /// Desde un `serde_json::Value`. Los numeros salen como los imprime
    /// serde, no como se leyeron: vale para construir JSON aqui, no para
    /// resumir un archivo ajeno (para eso, [Json::analizar] sobre el texto).
    pub fn de_valor(v: &serde_json::Value) -> Json {
        match v {
            serde_json::Value::Null => Json::Nulo,
            serde_json::Value::Bool(b) => Json::booleano(*b),
            serde_json::Value::Number(n) => Json::Literal(n.to_string()),
            serde_json::Value::String(s) => Json::Cadena(s.clone()),
            serde_json::Value::Array(l) => Json::Lista(l.iter().map(Json::de_valor).collect()),
            serde_json::Value::Object(o) => Json::Objeto(Objeto::de(
                o.iter()
                    .map(|(k, v)| (k.clone(), Json::de_valor(v)))
                    .collect(),
            )),
        }
    }

    /// A `serde_json::Value`, para quien lo decodifique en una estructura.
    /// Un literal que no es numero ni booleano se entrega como cadena, que
    /// es lo unico que serde puede guardar de el.
    pub fn a_valor(&self) -> serde_json::Value {
        match self {
            Json::Nulo => serde_json::Value::Null,
            Json::Literal(t) => match t.as_str() {
                "true" => serde_json::Value::Bool(true),
                "false" => serde_json::Value::Bool(false),
                _ => t
                    .parse::<serde_json::Number>()
                    .map(serde_json::Value::Number)
                    .unwrap_or_else(|_| serde_json::Value::String(t.clone())),
            },
            Json::Cadena(s) => serde_json::Value::String(s.clone()),
            Json::Lista(l) => serde_json::Value::Array(l.iter().map(Json::a_valor).collect()),
            Json::Objeto(o) => serde_json::Value::Object(
                o.iter()
                    .map(|(k, v)| (k.to_string(), v.a_valor()))
                    .collect(),
            ),
        }
    }
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.a_texto())
    }
}

/// Las comillas y los escapes de kotlinx (`printQuoted`): `"`, `\`, `\n`,
/// `\r`, `\t`, `\b` y `\f` con su letra, el resto de los controles como
/// `\u00XX` en minusculas, y todo lo demas tal cual, incluidos `/`, el
/// DEL y cualquier letra fuera del ASCII.
fn escribir_entrecomillada(s: &str, salida: &mut String) {
    salida.push('"');
    for c in s.chars() {
        match c {
            '"' => salida.push_str("\\\""),
            '\\' => salida.push_str("\\\\"),
            '\n' => salida.push_str("\\n"),
            '\r' => salida.push_str("\\r"),
            '\t' => salida.push_str("\\t"),
            '\u{8}' => salida.push_str("\\b"),
            '\u{c}' => salida.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                salida.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => salida.push(c),
        }
    }
    salida.push('"');
}

struct Lector<'a> {
    bytes: &'a [u8],
    texto: &'a str,
    pos: usize,
}

impl Lector<'_> {
    fn error(&self, motivo: &'static str) -> ErrorDeJson {
        ErrorDeJson {
            posicion: self.pos,
            motivo,
        }
    }

    fn saltar_blancos(&mut self) {
        while let Some(&b) = self.bytes.get(self.pos) {
            if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn valor(&mut self, hondura: usize) -> Result<Json, ErrorDeJson> {
        if hondura > HONDURA_MAXIMA {
            return Err(self.error("demasiado anidado"));
        }
        match self.bytes.get(self.pos) {
            None => Err(self.error("se acabo el texto donde iba un valor")),
            Some(b'{') => self.objeto(hondura),
            Some(b'[') => self.lista(hondura),
            Some(b'"') => Ok(Json::Cadena(self.cadena()?)),
            Some(_) => self.literal(),
        }
    }

    fn objeto(&mut self, hondura: usize) -> Result<Json, ErrorDeJson> {
        self.pos += 1;
        let mut o = Objeto::nuevo();
        self.saltar_blancos();
        if self.bytes.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(Json::Objeto(o));
        }
        loop {
            self.saltar_blancos();
            if self.bytes.get(self.pos) != Some(&b'"') {
                return Err(self.error("una clave tiene que ir entre comillas"));
            }
            let clave = self.cadena()?;
            self.saltar_blancos();
            if self.bytes.get(self.pos) != Some(&b':') {
                return Err(self.error("falta el ':' tras la clave"));
            }
            self.pos += 1;
            self.saltar_blancos();
            let v = self.valor(hondura + 1)?;
            o.poner(clave, v);
            self.saltar_blancos();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Objeto(o));
                }
                _ => return Err(self.error("falta ',' o '}' en el objeto")),
            }
        }
    }

    fn lista(&mut self, hondura: usize) -> Result<Json, ErrorDeJson> {
        self.pos += 1;
        let mut l = Vec::new();
        self.saltar_blancos();
        if self.bytes.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Ok(Json::Lista(l));
        }
        loop {
            self.saltar_blancos();
            l.push(self.valor(hondura + 1)?);
            self.saltar_blancos();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Lista(l));
                }
                _ => return Err(self.error("falta ',' o ']' en la lista")),
            }
        }
    }

    /// Una cadena entre comillas, con sus escapes resueltos. Los controles
    /// sin escapar se aceptan, como hace kotlinx.
    fn cadena(&mut self) -> Result<String, ErrorDeJson> {
        self.pos += 1;
        let mut s = String::new();
        loop {
            let inicio = self.pos;
            while let Some(&b) = self.bytes.get(self.pos) {
                if b == b'"' || b == b'\\' {
                    break;
                }
                self.pos += 1;
            }
            // Entre escapes se copia de golpe: los limites caen siempre en
            // ASCII, asi que el tramo es UTF-8 valido.
            s.push_str(&self.texto[inicio..self.pos]);
            match self.bytes.get(self.pos) {
                None => return Err(self.error("cadena sin cerrar")),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(s);
                }
                Some(_) => {
                    self.pos += 1;
                    self.escape(&mut s)?;
                }
            }
        }
    }

    fn escape(&mut self, s: &mut String) -> Result<(), ErrorDeJson> {
        let Some(&b) = self.bytes.get(self.pos) else {
            return Err(self.error("escape sin terminar"));
        };
        self.pos += 1;
        match b {
            b'"' => s.push('"'),
            b'\\' => s.push('\\'),
            b'/' => s.push('/'),
            b'b' => s.push('\u{8}'),
            b'f' => s.push('\u{c}'),
            b'n' => s.push('\n'),
            b'r' => s.push('\r'),
            b't' => s.push('\t'),
            b'u' => {
                let alto = self.cuatro_hex()?;
                let c = match alto {
                    0xD800..=0xDBFF => {
                        // Un par sustituto. Si no viene la segunda mitad,
                        // Java escribiria '?' al pasar a UTF-8: se imita
                        // para que el resumen sea el mismo.
                        if self.bytes.get(self.pos) == Some(&b'\\')
                            && self.bytes.get(self.pos + 1) == Some(&b'u')
                        {
                            let guardado = self.pos;
                            self.pos += 2;
                            let bajo = self.cuatro_hex()?;
                            if (0xDC00..=0xDFFF).contains(&bajo) {
                                let punto = 0x10000 + ((alto - 0xD800) << 10) + (bajo - 0xDC00);
                                char::from_u32(punto).unwrap_or('?')
                            } else {
                                self.pos = guardado;
                                '?'
                            }
                        } else {
                            '?'
                        }
                    }
                    0xDC00..=0xDFFF => '?',
                    _ => char::from_u32(alto).unwrap_or('?'),
                };
                s.push(c);
            }
            _ => return Err(self.error("escape desconocido")),
        }
        Ok(())
    }

    fn cuatro_hex(&mut self) -> Result<u32, ErrorDeJson> {
        let fin = self.pos + 4;
        let trozo = self
            .bytes
            .get(self.pos..fin)
            .ok_or_else(|| self.error("\\u sin sus cuatro cifras"))?;
        let mut n = 0u32;
        for &b in trozo {
            let d = (b as char)
                .to_digit(16)
                .ok_or_else(|| self.error("\\u con una cifra que no es hexadecimal"))?;
            n = (n << 4) | d;
        }
        self.pos = fin;
        Ok(n)
    }

    /// Una palabra sin comillas: hasta el siguiente signo de puntuacion o
    /// blanco. `null` es el nulo; lo demas, un literal con su texto.
    fn literal(&mut self) -> Result<Json, ErrorDeJson> {
        let inicio = self.pos;
        while let Some(&b) = self.bytes.get(self.pos) {
            if matches!(
                b,
                b' ' | b'\t'
                    | b'\n'
                    | b'\r'
                    | b','
                    | b':'
                    | b'{'
                    | b'}'
                    | b'['
                    | b']'
                    | b'"'
                    | b'\\'
            ) {
                break;
            }
            self.pos += 1;
        }
        if self.pos == inicio {
            return Err(self.error("se esperaba un valor"));
        }
        let palabra = &self.texto[inicio..self.pos];
        Ok(if palabra == "null" {
            Json::Nulo
        } else {
            Json::Literal(palabra.to_string())
        })
    }
}

/// El orden de `String.compareTo` de Kotlin: por unidades UTF-16, no por
/// puntos de codigo. Solo se nota con letras fuera del plano basico (un
/// emoji frente a un caracter de la zona privada), pero ahi Rust y Kotlin
/// ordenan al reves y el resumen saldria distinto.
pub fn comparar_como_kotlin(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// La forma canonica de un JSON: claves ordenadas y sin nulos en los
/// objetos. Los nulos de una lista se quedan (son posiciones). Si el texto
/// no es JSON se devuelve tal cual, como hace Android.
pub fn de(texto: &str) -> String {
    match Json::analizar(texto) {
        Ok(j) => ordenar(j).a_texto(),
        Err(_) => texto.to_string(),
    }
}

fn ordenar(e: Json) -> Json {
    match e {
        Json::Objeto(o) => {
            let mut pares: Vec<(String, Json)> = o
                .pares
                .into_iter()
                .filter(|(_, v)| !matches!(v, Json::Nulo))
                .map(|(k, v)| (k, ordenar(v)))
                .collect();
            // Estable, como `sortedBy`; las claves ya son unicas.
            pares.sort_by(|a, b| comparar_como_kotlin(&a.0, &b.0));
            Json::Objeto(Objeto { pares })
        }
        Json::Lista(l) => Json::Lista(l.into_iter().map(ordenar).collect()),
        otro => otro,
    }
}

/// SHA-256 en hexadecimal minuscula, como `sha256(bytes)` de Android.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// El resumen de un texto: SHA-256 de su forma canonica en UTF-8. Es lo que
/// Android apunta de cada lienzo, tabla, croquis y mensaje.
pub fn resumen(texto: &str) -> String {
    sha256_hex(de(texto).as_bytes())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn ordena_las_claves_y_quita_los_nulos_de_los_objetos() {
        let texto = r#"{"b": 1, "a": {"z": null, "y": [1.50, "x", null]}, "c": "é\n"}"#;
        // Los nulos de la lista se quedan: son posiciones. El 1.50 no se
        // toca: kotlinx escribe el numero tal como lo leyo.
        assert_eq!(
            de(texto),
            "{\"a\":{\"y\":[1.50,\"x\",null]},\"b\":1,\"c\":\"é\\n\"}"
        );
    }

    #[test]
    fn el_mismo_json_escrito_de_dos_maneras_da_el_mismo_resumen() {
        let a = r#"{"elements":[{"id":"a","x":1.0}],"files":{}}"#;
        let b = "{ \"files\" : { } ,\n \"elements\" : [ { \"x\" : 1.0 , \"id\" : \"a\" } ] }";
        assert_eq!(de(a), de(b));
        assert_eq!(resumen(a), resumen(b));
        // Caso negativo: `1.0` y `1` NO son lo mismo para kotlinx, y aqui
        // tampoco; si lo fueran, el resumen del movil y este no cuadrarian.
        let c = r#"{"elements":[{"id":"a","x":1}],"files":{}}"#;
        assert_ne!(resumen(a), resumen(c));
    }

    #[test]
    fn los_numeros_conservan_su_texto() {
        for n in [
            "1.50",
            "1.0E-5",
            "-0",
            "1e21",
            "100000000000000000000000",
            "007",
        ] {
            assert_eq!(de(&format!("[{n}]")), format!("[{n}]"), "{n}");
        }
    }

    #[test]
    fn lo_que_no_es_json_sale_tal_cual() {
        assert_eq!(de("hola que tal"), "hola que tal");
        assert_eq!(de(""), "");
        assert_eq!(de("{\"a\":1"), "{\"a\":1");
        assert_eq!(de("[1,]"), "[1,]");
        assert_eq!(de("{\"a\":1} x"), "{\"a\":1} x");
    }

    #[test]
    fn una_palabra_sin_comillas_es_un_literal_como_en_kotlinx() {
        // El lector de arboles de kotlinx acepta `abc` como literal aunque
        // no este en modo permisivo; se imita para resumir lo mismo.
        assert_eq!(Json::analizar("abc"), Ok(Json::Literal("abc".into())));
        assert_eq!(de("{\"a\":NaN}"), "{\"a\":NaN}");
        assert_eq!(Json::analizar("null"), Ok(Json::Nulo));
        assert_eq!(Json::analizar("true"), Ok(Json::Literal("true".into())));
    }

    #[test]
    fn los_escapes_se_leen_y_se_escriben_como_kotlinx() {
        let leido = Json::analizar(r#""a\/bé😀\t\u0001\u007f""#).unwrap();
        assert_eq!(leido, Json::Cadena("a/bé😀\t\u{1}\u{7f}".into()));
        // Al escribir: la barra sin escapar, el emoji y el DEL tal cual, el
        // tabulador con su letra y el control como \u0001 en minusculas.
        assert_eq!(leido.a_texto(), "\"a/bé😀\\t\\u0001\u{7f}\"");
        assert_eq!(
            Json::cadena("\"\\\n\r\u{8}\u{c}\u{1f}").a_texto(),
            r#""\"\\\n\r\b\f\u001f""#
        );
        // Un sustituto suelto se vuelve '?', que es lo que Java escribe al
        // pasar a UTF-8.
        assert_eq!(
            Json::analizar(r#""\ud83dx""#).unwrap(),
            Json::Cadena("?x".into())
        );
    }

    #[test]
    fn un_objeto_es_igual_a_otro_aunque_cambie_el_orden_de_sus_claves() {
        let a = Json::analizar(r#"{"a":1,"b":{"c":[1,2]}}"#).unwrap();
        let b = Json::analizar(r#"{"b":{"c":[1,2]},"a":1}"#).unwrap();
        assert_eq!(a, b);
        // Pero una lista si tiene orden.
        assert_ne!(
            Json::analizar("[1,2]").unwrap(),
            Json::analizar("[2,1]").unwrap()
        );
        // Y una clave repetida se queda con el ultimo valor en el primer sitio.
        assert_eq!(
            Json::analizar(r#"{"a":1,"b":2,"a":3}"#).unwrap().a_texto(),
            r#"{"a":3,"b":2}"#
        );
    }

    #[test]
    fn las_claves_se_ordenan_por_unidades_utf16() {
        // U+FFFF va DESPUES de U+1F600 en Kotlin (sustitutos D83D DE00, y
        // D83D < FFFF) y antes en el orden de Rust por puntos de codigo.
        assert_eq!(comparar_como_kotlin("\u{ffff}", "😀"), Ordering::Greater);
        assert_eq!("\u{ffff}".cmp("😀"), Ordering::Less);
        assert_eq!(de("{\"\u{ffff}\":2,\"😀\":1}"), "{\"😀\":1,\"\u{ffff}\":2}");
    }

    #[test]
    fn el_sha256_es_el_de_libro() {
        // Vectores del NIST (FIPS 180-2).
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn el_resumen_es_el_sha256_del_texto_canonico_en_utf8() {
        assert_eq!(
            resumen("{\"b\":1,\"a\":\"é\"}"),
            sha256_hex("{\"a\":\"é\",\"b\":1}".as_bytes())
        );
        // Y de lo que no es JSON, el del texto tal cual.
        assert_eq!(resumen("hola"), sha256_hex(b"hola"));
    }

    #[test]
    fn un_json_demasiado_hondo_no_tira_la_pila() {
        let hondo = "[".repeat(100_000) + &"]".repeat(100_000);
        assert!(Json::analizar(&hondo).is_err());
        assert_eq!(de(&hondo), hondo);
    }

    #[test]
    fn va_y_vuelve_de_serde_json() {
        let v: serde_json::Value =
            serde_json::from_str(r#"{"a":[1,true,null,"x"],"b":2.5}"#).unwrap();
        let j = Json::de_valor(&v);
        assert_eq!(j.a_texto(), r#"{"a":[1,true,null,"x"],"b":2.5}"#);
        assert_eq!(j.a_valor(), v);
        // Un literal que serde no sabe guardar se entrega como cadena.
        assert_eq!(
            Json::Literal("NaN".into()).a_valor(),
            serde_json::Value::String("NaN".into())
        );
    }

    #[test]
    fn un_objeto_pone_y_quita_conservando_el_sitio() {
        let mut o = Objeto::de(vec![
            ("a".into(), Json::numero(1)),
            ("b".into(), Json::numero(2)),
        ]);
        o.poner("a", Json::numero(9));
        o.poner("c", Json::Nulo);
        assert_eq!(
            Json::Objeto(o.clone()).a_texto(),
            r#"{"a":9,"b":2,"c":null}"#
        );
        assert_eq!(o.quitar("b"), Some(Json::numero(2)));
        assert_eq!(o.quitar("b"), None);
        assert_eq!(o.len(), 2);
        assert!(!o.contiene("b"));
    }
}
