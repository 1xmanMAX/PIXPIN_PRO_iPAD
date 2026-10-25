//! Las tablas de calculo del proyecto.
//!
//! El movil tiene su hoja desde el 10-sep-2026 (`motor/TablaDeCalculo.kt`,
//! el mismo JSON: `nombre`, `celdas`, `anchos`, `estilos`, `tocado`,
//! `protegida`); esta nota decia lo contrario porque se escribio mirando un
//! clon viejo. Aqui viaja por el hueco de las mini-apps: un mensaje de clase
//! `MINIAPP` lleva el documento entero en `texto`
//! y la palabra de su tipo en `miniapp` —aqui, `"tabla"`—, y ante una
//! palabra que no conoce el movil ensena ese texto tal cual en vez de
//! romperse. No hay fichero aparte: la tabla ES el texto del mensaje.
//!
//! La forma es deliberadamente sencilla: **un mapa de celda a texto**, no
//! una matriz.
//!
//! Que sea un mapa y no una matriz no es capricho: una tabla con una celda en
//! la `A1` y otra en la `Z900` ocupa dos entradas y no ochocientas mil. Y
//! ademas es lo que hace que la fusion funcione celda a celda —un mapa se
//! junta clave por clave— sin escribir nada especial para ello.
//!
//! Aqui no se calculan formulas todavia: se guarda el texto tal cual
//! («=SUMA(B1:B6)») y se ensena. Calcular es un paso aparte, y fingir un
//! resultado seria peor que ensenar la formula.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// La palabra que va en `Mensaje.miniapp` para decir que el texto es una
/// tabla. Se escribe una vez aqui porque es un dato guardado: cambiarla
/// dejaria de reconocer las tablas ya escritas.
pub const MINIAPP: &str = "tabla";

/// Una tabla, tal como viaja.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tabla {
    pub nombre: String,
    /// De referencia (`B7`) a contenido. El contenido puede ser un numero, un
    /// texto o una formula que empieza por `=`.
    pub celdas: BTreeMap<String, String>,
    /// Ancho de una columna (`B`) en pixeles, si se cambio a mano.
    pub anchos: BTreeMap<String, u32>,
    /// El estilo de una celda: el `EstiloDeCelda` del movil.
    pub estilos: BTreeMap<String, EstiloDeCelda>,
    /// Milisegundos desde 1970 de la ultima vez que se toco.
    pub tocado: i64,
    /// Lo que no se entiende, tal cual: guardar no puede perder lo que anada
    /// una version de Android.
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

/// **El poco formato de una celda**, el `EstiloDeCelda` del movil
/// (`motor/TablaDeCalculo.kt`): negrita, alineacion y fondo, con nombres de
/// una letra porque se repiten en cada celda con estilo. Lo que no vale lo de
/// por defecto no se escribe (`encodeDefaults = false` alli).
///
/// **Lo que anade el PC** (12-oct-2026, la barra de formato del pin de
/// hoja): cursiva `k`, subrayado `s`, tachado `t`, color de letra `c`,
/// ajustar texto `w` y formato de numero `nf`. El movil (a 12-oct) solo
/// conoce `n`, `a`, `f` y `e`, y lee con `ignoreUnknownKeys = true`: estos
/// campos los IGNORA sin romperse (la celda se le ve sin ellos). Ojo: si el
/// movil vuelve a guardar esa tabla, los pierde, porque su `EstiloDeCelda`
/// no los guarda; hasta que Android los anada, viajan de ida pero no
/// sobreviven a una edicion alli.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EstiloDeCelda {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub n: bool,
    /// `i`, `c` o `d`; sin nada, la que toque por el valor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a: Option<String>,
    /// El fondo, `#rrggbb`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub f: Option<String>,
    /// Editable aunque la tabla este protegida (la pagina web lo mira).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub e: bool,
    /// Cursiva (la `k` de Ctrl+K en la Excel espanola; la `c` ya es el color).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub k: bool,
    /// Subrayado.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub s: bool,
    /// Tachado.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub t: bool,
    /// El color de la letra, `#rrggbb`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c: Option<String>,
    /// Ajustar el texto a la celda (varias lineas).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub w: bool,
    /// El formato del numero: una de [`FormatoNumero`] (`numero`, `moneda`,
    /// `porcentaje`, `fecha`); sin nada, «General».
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nf: Option<String>,
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

impl EstiloDeCelda {
    /// Si no cambia nada: no vale la pena guardarlo.
    pub fn vacio(&self) -> bool {
        !self.e && self.resto.is_empty() && self.sin_formato()
    }

    /// Si no lleva formato que se vea (lo que quita «Borrar formato»; la
    /// marca de editable `e` no es formato).
    pub fn sin_formato(&self) -> bool {
        !self.n && self.a.is_none() && self.f.is_none() && !self.k && !self.s && !self.t && self.c.is_none() && !self.w && self.nf.is_none()
    }

    /// **Borrar formato**: quita todo lo que se ve y deja la marca de
    /// editable y lo que no se entiende (puede no ser formato).
    pub fn borrar_formato(&mut self) {
        *self = EstiloDeCelda { e: self.e, resto: std::mem::take(&mut self.resto), ..Default::default() };
    }

    /// El formato de numero, si es uno de los que se conocen.
    pub fn formato_numero(&self) -> Option<FormatoNumero> {
        self.nf.as_deref().and_then(FormatoNumero::de)
    }
}

/// **Los formatos de numero** de la barra del pin (el `nf` de un estilo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatoNumero {
    /// Con miles y dos decimales.
    Numero,
    /// Con el simbolo de la moneda del equipo, miles y dos decimales.
    Moneda,
    /// Por cien, con `%` y dos decimales.
    Porcentaje,
    /// La fecha corta (el numero es el serial de Excel).
    Fecha,
}

impl FormatoNumero {
    pub const TODOS: [FormatoNumero; 4] = [FormatoNumero::Numero, FormatoNumero::Moneda, FormatoNumero::Porcentaje, FormatoNumero::Fecha];

    /// La palabra que se guarda.
    pub fn palabra(self) -> &'static str {
        match self {
            FormatoNumero::Numero => "numero",
            FormatoNumero::Moneda => "moneda",
            FormatoNumero::Porcentaje => "porcentaje",
            FormatoNumero::Fecha => "fecha",
        }
    }

    /// De la palabra guardada; una que no se conoce (de una version mas
    /// nueva) es `None`: se ve como «General».
    pub fn de(palabra: &str) -> Option<FormatoNumero> {
        FormatoNumero::TODOS.into_iter().find(|f| f.palabra() == palabra)
    }
}

/// Una celda por su sitio: columna y fila, las dos desde cero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ref {
    pub columna: u32,
    pub fila: u32,
}

/// De `B7` a (1, 6). `None` si no tiene esa forma.
///
/// Las columnas van como en cualquier hoja de calculo: tras la `Z` viene la
/// `AA`, que es la **27**, no la 26; por eso se suma uno a cada letra antes
/// de multiplicar, y se resta uno al final.
pub fn ref_de(texto: &str) -> Option<Ref> {
    let texto = texto.trim();
    if texto.is_empty() {
        return None;
    }
    let letras: String = texto
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let numeros = &texto[letras.len()..];
    if letras.is_empty() || numeros.is_empty() {
        return None;
    }
    // Una referencia con mas de siete letras no cabe en un u32, y no existe
    // hoja con tantas columnas: es basura, no una celda lejana.
    if letras.len() > 7 || !numeros.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut columna: u32 = 0;
    for c in letras.chars() {
        let valor = c.to_ascii_uppercase() as u32 - 'A' as u32 + 1;
        columna = columna.checked_mul(26)?.checked_add(valor)?;
    }
    let fila: u32 = numeros.parse().ok()?;
    // La fila 0 no existe: las hojas empiezan en la 1.
    Some(Ref {
        columna: columna.checked_sub(1)?,
        fila: fila.checked_sub(1)?,
    })
}

/// De (1, 6) a `B7`.
pub fn ref_a(r: Ref) -> String {
    let mut letras = Vec::new();
    let mut n = r.columna as u64 + 1;
    while n > 0 {
        let resto = ((n - 1) % 26) as u8;
        letras.push((b'A' + resto) as char);
        n = (n - 1) / 26;
    }
    letras.reverse();
    let letras: String = letras.into_iter().collect();
    format!("{letras}{}", r.fila + 1)
}

impl Tabla {
    /// Lo que hay en una celda, o cadena vacia.
    pub fn celda(&self, r: Ref) -> &str {
        self.celdas.get(&ref_a(r)).map_or("", |s| s.as_str())
    }

    /// Escribe una celda. Escribir vacio la QUITA en vez de guardar una
    /// cadena vacia: si no, borrar una celda dejaria basura que viajaria por
    /// la red y contaria como diferencia sin serlo.
    pub fn poner(&mut self, r: Ref, contenido: &str) {
        let clave = ref_a(r);
        if contenido.is_empty() {
            self.celdas.remove(&clave);
        } else {
            self.celdas.insert(clave, contenido.to_string());
        }
    }

    /// Hasta donde llega la tabla: cuantas columnas y cuantas filas hay que
    /// ensenar para que se vea todo lo escrito.
    ///
    /// Se mira lo que hay, no un tamano guardado: una tabla es tan grande
    /// como su celda mas lejana.
    pub fn tamano(&self) -> (u32, u32) {
        let mut columnas = 0;
        let mut filas = 0;
        for clave in self.celdas.keys() {
            if let Some(r) = ref_de(clave) {
                columnas = columnas.max(r.columna + 1);
                filas = filas.max(r.fila + 1);
            }
        }
        (columnas, filas)
    }

    /// Si el contenido de una celda es una formula.
    pub fn es_formula(contenido: &str) -> bool {
        contenido.starts_with('=')
    }

    /// **Lee una tabla**, sea de esta version, de Android o de una vieja.
    ///
    /// Primero tal cual; si no encaja pero es un objeto JSON, se rescata
    /// campo a campo ([`Tabla::rescatar`]): una tabla escrita por una
    /// version anterior (los estilos eran palabras, `"B1": "negrita"`) o
    /// por otra herramienta (numeros en vez de textos en `celdas`) se abre
    /// igual en vez de quedar sin poder editarse. Lo que no se puede
    /// convertir NO se tira: va a `resto` bajo `viejo_<campo>` y vuelve al
    /// guardar. Solo lo que ni siquiera es un objeto JSON es un error, y
    /// entonces quien lee no debe guardar encima (ver `hoja_pin`).
    pub fn leer(texto: &str) -> Result<Tabla, serde_json::Error> {
        match serde_json::from_str(texto) {
            Ok(t) => Ok(t),
            Err(e) => match serde_json::from_str::<serde_json::Value>(texto) {
                Ok(serde_json::Value::Object(o)) => Ok(Tabla::rescatar(o)),
                _ => Err(e),
            },
        }
    }

    /// Una tabla sacada de un objeto JSON que no tiene la forma de ahora.
    /// Ver [`Tabla::leer`].
    pub fn rescatar(mut o: serde_json::Map<String, serde_json::Value>) -> Tabla {
        use serde_json::{Map, Value};
        let mut t = Tabla::default();
        let mut viejos = Map::new();
        if let Some(v) = o.remove("nombre") {
            match v {
                Value::String(s) => t.nombre = s,
                Value::Null => {}
                otro => {
                    viejos.insert("viejo_nombre".into(), otro);
                }
            }
        }
        // Un mapa de celda a algo: lo que `convertir` entiende entra; lo
        // demas se queda aparte, tal cual.
        let mut mapa = |o: &mut Map<String, Value>, clave: &str, mut convertir: Box<dyn FnMut(String, &Value) -> bool + '_>| {
            let Some(v) = o.remove(clave) else { return };
            match v {
                Value::Object(m) => {
                    let malas: Map<String, Value> = m.into_iter().filter(|(k, v)| !convertir(k.clone(), v)).collect();
                    if !malas.is_empty() {
                        viejos.insert(format!("viejo_{clave}"), Value::Object(malas));
                    }
                }
                Value::Null => {}
                otro => {
                    viejos.insert(format!("viejo_{clave}"), otro);
                }
            }
        };
        let celdas = &mut t.celdas;
        mapa(
            &mut o,
            "celdas",
            Box::new(|k, v| match texto_de_celda(v) {
                Some(s) => {
                    if !s.is_empty() {
                        celdas.insert(k, s);
                    }
                    true
                }
                None => false,
            }),
        );
        let anchos = &mut t.anchos;
        mapa(
            &mut o,
            "anchos",
            Box::new(|k, v| {
                let n = match v {
                    Value::Number(n) => n.as_f64(),
                    Value::String(s) => s.trim().parse::<f64>().ok(),
                    _ => None,
                };
                match n.filter(|n| n.is_finite() && *n >= 0.0 && *n <= u32::MAX as f64) {
                    Some(n) => {
                        anchos.insert(k, n.round() as u32);
                        true
                    }
                    None => false,
                }
            }),
        );
        let estilos = &mut t.estilos;
        mapa(
            &mut o,
            "estilos",
            Box::new(|k, v| match estilo_de(v) {
                Some(e) => {
                    if !e.vacio() {
                        estilos.insert(k, e);
                    }
                    true
                }
                None => false,
            }),
        );
        if let Some(v) = o.remove("tocado") {
            let n = match &v {
                Value::Number(n) => n.as_i64().or_else(|| n.as_f64().filter(|f| f.is_finite()).map(|f| f as i64)),
                Value::String(s) => s.trim().parse().ok(),
                _ => None,
            };
            match n {
                Some(n) => t.tocado = n,
                None if v.is_null() => {}
                None => {
                    viejos.insert("viejo_tocado".into(), v);
                }
            }
        }
        // Lo demas (`protegida`, lo de una version mas nueva...) tal cual.
        t.resto = o;
        t.resto.extend(viejos);
        t
    }

    pub fn escribir(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// El texto de una celda escrita de otra forma: un numero o un si/no pasan
/// a texto (`12`, `true`); `null` es una celda vacia. Una lista o un objeto
/// no son una celda: `None`.
fn texto_de_celda(v: &serde_json::Value) -> Option<String> {
    use serde_json::Value;
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b { "VERDADERO" } else { "FALSO" }.into()),
        Value::Null => Some(String::new()),
        _ => None,
    }
}

/// Un estilo escrito de otra forma. El PC de antes del 12-oct guardaba una
/// palabra (`"negrita"`); una version rara puede traer `"n": 1` o
/// `"n": "true"`. Lo que no se entiende se queda en `resto` (con la palabra
/// entera en `viejo` si era una palabra). `None`: ni objeto ni palabra.
fn estilo_de(v: &serde_json::Value) -> Option<EstiloDeCelda> {
    use serde_json::Value;
    let si = |v: &Value| match v {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => n.as_f64().map(|x| x != 0.0),
        Value::String(s) => match s.trim().to_lowercase().as_str() {
            "true" | "1" | "si" => Some(true),
            "false" | "0" | "no" | "" => Some(false),
            _ => None,
        },
        Value::Null => Some(false),
        _ => None,
    };
    match v {
        Value::Object(m) => {
            if let Ok(e) = serde_json::from_value::<EstiloDeCelda>(v.clone()) {
                return Some(e);
            }
            let mut e = EstiloDeCelda::default();
            for (k, x) in m {
                let hecho = match k.as_str() {
                    "n" => si(x).map(|b| e.n = b).is_some(),
                    "e" => si(x).map(|b| e.e = b).is_some(),
                    "k" => si(x).map(|b| e.k = b).is_some(),
                    "s" => si(x).map(|b| e.s = b).is_some(),
                    "t" => si(x).map(|b| e.t = b).is_some(),
                    "w" => si(x).map(|b| e.w = b).is_some(),
                    "a" | "f" | "c" | "nf" => match x {
                        Value::String(s) => {
                            let s = Some(s.clone());
                            match k.as_str() {
                                "a" => e.a = s,
                                "f" => e.f = s,
                                "c" => e.c = s,
                                _ => e.nf = s,
                            }
                            true
                        }
                        Value::Null => true,
                        _ => false,
                    },
                    _ => false,
                };
                // Un campo conocido que no se entiende va a `resto` con su
                // nombre: el suyo no se escribe (vale lo de por defecto),
                // asi que no sale dos veces.
                if !hecho {
                    e.resto.insert(k.clone(), x.clone());
                }
            }
            Some(e)
        }
        Value::String(palabras) => {
            let mut e = EstiloDeCelda::default();
            let mut raras = false;
            for p in palabras.split([' ', ',', ';', '+']).map(str::trim).filter(|p| !p.is_empty()) {
                match p.to_lowercase().as_str() {
                    "negrita" | "bold" | "b" => e.n = true,
                    "cursiva" | "italic" | "italica" => e.k = true,
                    "subrayado" | "underline" => e.s = true,
                    "tachado" | "strike" => e.t = true,
                    "ajustar" | "wrap" => e.w = true,
                    "izquierda" | "left" => e.a = Some("i".into()),
                    "centro" | "centrado" | "center" => e.a = Some("c".into()),
                    "derecha" | "right" => e.a = Some("d".into()),
                    _ => raras = true,
                }
            }
            if raras {
                e.resto.insert("viejo".into(), v.clone());
            }
            Some(e)
        }
        Value::Null => Some(EstiloDeCelda::default()),
        _ => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_referencias_van_y_vuelven() {
        for (texto, columna, fila) in [
            ("A1", 0, 0),
            ("B7", 1, 6),
            ("Z1", 25, 0),
            // Tras la Z viene la AA, que es la 27; el fallo clasico es que
            // salga la 26 por no sumar uno a cada letra.
            ("AA1", 26, 0),
            ("AB1", 27, 0),
            ("BA1", 52, 0),
            ("ZZ100", 701, 99),
            ("AAA1", 702, 0),
        ] {
            let r = ref_de(texto).unwrap_or_else(|| panic!("{texto}"));
            assert_eq!((r.columna, r.fila), (columna, fila), "{texto}");
            assert_eq!(ref_a(r), texto, "de vuelta");
        }
    }

    #[test]
    fn lo_que_no_es_una_celda_no_lo_parece() {
        // Casos negativos: sin numero, sin letra, del reves, la fila cero
        // (las hojas empiezan en la 1) y basura.
        for malo in ["", "A", "1", "1A", "A0", "-", "A-1", "A1.5", "ABCDEFGH1"] {
            assert!(ref_de(malo).is_none(), "{malo} no es una celda");
        }
        // Las minusculas si valen: se teclean asi a menudo.
        assert_eq!(ref_de("b7"), ref_de("B7"));
    }

    #[test]
    fn una_celda_vacia_se_quita_en_vez_de_guardarse_vacia() {
        let mut t = Tabla::default();
        let b7 = ref_de("B7").unwrap();
        t.poner(b7, "12");
        assert_eq!(t.celda(b7), "12");
        assert_eq!(t.celdas.len(), 1);
        // Borrarla la quita del mapa: una cadena vacia viajaria por la red y
        // contaria como diferencia sin serlo.
        t.poner(b7, "");
        assert_eq!(t.celda(b7), "");
        assert!(t.celdas.is_empty());
    }

    #[test]
    fn la_tabla_es_tan_grande_como_su_celda_mas_lejana() {
        let mut t = Tabla::default();
        assert_eq!(t.tamano(), (0, 0), "una tabla vacia no ocupa nada");
        t.poner(ref_de("A1").unwrap(), "1");
        assert_eq!(t.tamano(), (1, 1));
        t.poner(ref_de("C10").unwrap(), "2");
        assert_eq!(t.tamano(), (3, 10));
        // Una celda suelta lejos no cuesta nada: el mapa tiene dos entradas,
        // no treinta.
        assert_eq!(t.celdas.len(), 2);
    }

    #[test]
    fn una_tabla_de_android_se_lee_y_se_devuelve_sin_perder_nada() {
        let original = r##"{
            "nombre": "Gastos",
            "celdas": {"A1": "Concepto", "B1": "Importe", "B7": "=SUMA(B2:B6)"},
            "anchos": {"A": 180},
            "estilos": {"B1": {"n": true, "f": "#fff2cc"}, "C1": {"a": "d", "x": 7}},
            "tocado": 1789500000000,
            "congeladas": 1,
            "futuro": {"x": true}
        }"##;
        let t = Tabla::leer(original).unwrap();
        assert_eq!(t.nombre, "Gastos");
        assert_eq!(t.celda(ref_de("A1").unwrap()), "Concepto");
        assert_eq!(t.anchos.get("A"), Some(&180));
        // El estilo es el `EstiloDeCelda` del movil (`n`, `a`, `f`, `e`), no
        // una palabra: con una cadena aqui, una tabla con negritas del movil
        // no se podia ni abrir.
        let b1 = &t.estilos["B1"];
        assert!(b1.n);
        assert_eq!(b1.f.as_deref(), Some("#fff2cc"));
        assert_eq!(t.estilos["C1"].a.as_deref(), Some("d"));
        assert!(Tabla::es_formula(t.celda(ref_de("B7").unwrap())));
        assert!(!Tabla::es_formula(t.celda(ref_de("A1").unwrap())));

        // Y lo que aqui no se entiende vuelve tal cual: guardar desde Windows
        // no puede tirar campos de una version mas nueva del movil.
        let vuelta: serde_json::Value = serde_json::from_str(&t.escribir().unwrap()).unwrap();
        assert_eq!(vuelta["congeladas"], 1);
        assert_eq!(vuelta["futuro"]["x"], true);
        assert_eq!(vuelta["celdas"]["B7"], "=SUMA(B2:B6)");
        assert_eq!(
            vuelta["estilos"]["C1"]["x"], 7,
            "lo que no se entiende de un estilo tambien vuelve"
        );
        // Y sin los valores por defecto, como `encodeDefaults = false` del
        // movil: mil celdas con estilo no repiten mil veces `"n":false`.
        assert!(vuelta["estilos"]["C1"].get("n").is_none());
    }

    #[test]
    fn el_formato_nuevo_del_pc_viaja_y_lo_que_el_movil_no_conoce_no_estorba() {
        let mut e = EstiloDeCelda { n: true, k: true, s: true, t: true, w: true, e: true, ..Default::default() };
        e.c = Some("#c00000".into());
        e.nf = Some(FormatoNumero::Moneda.palabra().into());
        e.resto.insert("futuro".into(), serde_json::Value::Bool(true));
        let mut t = Tabla::default();
        t.estilos.insert("A1".into(), e.clone());
        let texto = t.escribir().unwrap();
        let v: serde_json::Value = serde_json::from_str(&texto).unwrap();
        let a1 = &v["estilos"]["A1"];
        for campo in ["n", "k", "s", "t", "w", "e"] {
            assert_eq!(a1[campo], true, "{campo}");
        }
        assert_eq!(a1["c"], "#c00000");
        assert_eq!(a1["nf"], "moneda");
        assert_eq!(Tabla::leer(&texto).unwrap().estilos["A1"], e, "ida y vuelta sin perder nada");
        assert_eq!(e.formato_numero(), Some(FormatoNumero::Moneda));
        // Una tabla del movil (sin los campos nuevos) se lee igual que antes.
        let movil = Tabla::leer(r##"{"estilos":{"B1":{"n":true,"f":"#fff3bf"}}}"##).unwrap();
        let b1 = &movil.estilos["B1"];
        assert!(b1.n && !b1.k && !b1.s && !b1.t && !b1.w && b1.c.is_none() && b1.nf.is_none());
        assert!(b1.resto.is_empty(), "nada va a parar a `resto`");
        // Borrar formato deja la marca de editable y lo que no se entiende.
        e.borrar_formato();
        assert!(e.sin_formato() && e.e && e.resto.contains_key("futuro"));
        assert!(!e.vacio());
        // Casos negativos: una palabra que no se conoce no es un formato, y
        // sin nada mas el estilo queda vacio.
        assert_eq!(FormatoNumero::de("rarisimo"), None);
        let mut x = EstiloDeCelda { nf: Some("rarisimo".into()), ..Default::default() };
        assert_eq!(x.formato_numero(), None);
        assert!(!x.vacio(), "se guarda tal cual: puede ser de una version nueva");
        x.borrar_formato();
        assert!(x.vacio());
    }

    /// **Ida y vuelta del formato unico**: lo que escribe el PC se lee igual,
    /// y lo escrito por Android (compacto, sin lo de por defecto) tambien.
    #[test]
    fn el_formato_unico_va_y_vuelve_igual_desde_el_pc_y_desde_android() {
        let mut t = Tabla { nombre: "Metrados".into(), tocado: 1_789_000_000_123, ..Default::default() };
        t.poner(ref_de("A1").unwrap(), "Partida");
        t.poner(ref_de("B2").unwrap(), "=SUMA(B3:B9)");
        t.anchos.insert("A".into(), 180);
        t.estilos.insert("A1".into(), EstiloDeCelda { n: true, f: Some("#fff2cc".into()), nf: Some("moneda".into()), ..Default::default() });
        t.resto.insert("protegida".into(), serde_json::Value::Bool(true));
        let texto = t.escribir().unwrap();
        assert_eq!(Tabla::leer(&texto).unwrap(), t);
        // Lo mismo escrito por el movil (`TablaDeCalculo.aJson`): sin
        // espacios y sin lo que vale lo de por defecto.
        let movil = r##"{"nombre":"Metrados","celdas":{"A1":"Partida","B2":"=SUMA(B3:B9)"},"anchos":{"A":180},"estilos":{"A1":{"n":true,"f":"#fff2cc","nf":"moneda"}},"tocado":1789000000123,"protegida":true}"##;
        assert_eq!(Tabla::leer(movil).unwrap(), t);
        // Y la tabla vacia del movil (`{}`) es una tabla vacia.
        assert_eq!(Tabla::leer("{}").unwrap(), Tabla::default());
    }

    /// **Lo viejo se migra al leer**: estilos que eran palabras (el PC antes
    /// del 12-oct), numeros en vez de textos, anchos con decimales, la hora
    /// como texto. Lo que no se puede convertir no se pierde: `viejo_*`.
    #[test]
    fn una_tabla_vieja_o_rara_se_rescata_sin_perder_nada() {
        let vieja = r##"{
            "nombre": "Cuentas",
            "celdas": {"A1": "Total", "B1": 12.5, "C1": true, "D1": null, "E1": [1, 2]},
            "anchos": {"A": 120.4, "B": "90", "C": "ancho"},
            "estilos": {"A1": "negrita", "B1": "derecha cursiva", "C1": "brillante", "D1": {"n": 1, "k": "true", "f": 7}, "E1": 3},
            "tocado": "1789000000000",
            "protegida": true
        }"##;
        let t = Tabla::leer(vieja).expect("un objeto JSON siempre se abre");
        assert_eq!(t.nombre, "Cuentas");
        assert_eq!(t.celda(ref_de("A1").unwrap()), "Total");
        assert_eq!(t.celda(ref_de("B1").unwrap()), "12.5", "un numero pasa a texto y suma igual");
        assert_eq!(t.celda(ref_de("C1").unwrap()), "VERDADERO");
        assert!(!t.celdas.contains_key("D1"), "null es una celda vacia");
        assert_eq!(t.anchos["A"], 120);
        assert_eq!(t.anchos["B"], 90);
        assert!(t.estilos["A1"].n);
        assert_eq!(t.estilos["B1"].a.as_deref(), Some("d"));
        assert!(t.estilos["B1"].k);
        assert_eq!(t.estilos["C1"].resto["viejo"], "brillante", "la palabra que no se conoce se queda");
        let d1 = &t.estilos["D1"];
        assert!(d1.n && d1.k);
        assert_eq!(d1.resto["f"], 7, "el campo conocido que no se entiende se queda con su nombre");
        assert_eq!(t.tocado, 1_789_000_000_000);
        assert_eq!(t.resto["protegida"], true);
        assert_eq!(t.resto["viejo_celdas"]["E1"], serde_json::json!([1, 2]));
        assert_eq!(t.resto["viejo_anchos"]["C"], "ancho");
        assert_eq!(t.resto["viejo_estilos"]["E1"], 3);

        // Al guardar sale el formato de ahora (los estilos son objetos) y,
        // releido, da lo mismo: la migracion es estable.
        let texto = t.escribir().unwrap();
        let v: serde_json::Value = serde_json::from_str(&texto).unwrap();
        assert_eq!(v["estilos"]["A1"], serde_json::json!({"n": true}));
        assert_eq!(v["celdas"]["B1"], "12.5");
        let otra = Tabla::leer(&texto).unwrap();
        assert_eq!(otra, t, "leer lo migrado da lo mismo");
        // Ninguna clave sale dos veces (el `f` raro de D1 va solo en su resto).
        assert_eq!(texto.matches("\"f\"").count(), 1);
    }

    /// Caso negativo: lo que ni es un objeto JSON es un error (y entonces
    /// nadie guarda encima), no una tabla vacia.
    #[test]
    fn lo_que_no_es_un_objeto_json_no_se_toma_por_una_tabla() {
        for malo in ["", "no es json", "[1,2]", "\"tabla\"", "12", "null", "{\"nombre\": \"a\""] {
            assert!(Tabla::leer(malo).is_err(), "{malo:?}");
        }
    }
}
