//! La mini-app de **gastos**: conceptos con importe, y el total. Puerto de
//! `mini/Gastos.kt` de Android.
//!
//! ## El dinero se guarda en enteros, nunca en coma flotante
//!
//! Un `f64` **no puede** representar 0,10: lo mas cerca que llega es
//! 0,1000000000000000055511151231257827…, y sumar diez de esos no da 1 sino
//! 0,9999999999999999. En una lista de gastos eso significa que el total sale
//! a un centimo de lo que suma la columna, y quien lo mira ve una cuenta que
//! no cuadra sin poder saber por que: el error no esta en ninguna linea, esta
//! en la suma. No tiene arreglo a base de redondear al final —redondear tapa
//! unos casos y deja otros—.
//!
//! Aqui cada importe es un `i64` de **unidades menores**: centimos en euros,
//! pero tambien yenes enteros o milesimas de dinar. Cuantas tiene cada moneda
//! lo dice la propia moneda ([`Moneda::decimales`]) y no un `/100` escrito a
//! mano, porque el yen no tiene centimos y el dinar tunecino tiene tres: un
//! cien fijo convertiria 500 yenes en 5.
//!
//! ## La moneda sale del aparato al crear, y se guarda con los datos
//!
//! Guardada **en el documento**, en el rotulo de la cabecera de la tabla:
//! `| Concepto | Importe (EUR) |`. Si solo se mirara la configuracion del
//! aparato, unos gastos apuntados en un viaje cambiarian de moneda al volver a
//! casa: los mismos numeros pasarian de libras a euros sin que nadie los
//! tocara, y eso no es un fallo de presentacion, es una cuenta falsa.
//!
//! ## Es una tabla de Markdown, no un formato nuevo
//!
//! ```text
//! # Viaje a Lisboa
//!
//! | Concepto | Importe (EUR) |
//! | --- | ---: |
//! | Cena | 42.50 |
//! | Tren | 18.00 |
//! | Devolución | -5.00 |
//!
//! **Total: 55,50 €**
//! ```
//!
//! ## Se escribe estricto y se lee tolerante
//!
//! Al **escribir**, el importe va siempre en forma canonica: punto decimal,
//! sin separador de miles, con tantos decimales como tenga la moneda
//! (`42.50`). Es la unica forma que significa lo mismo en toda maquina:
//! escribiendo «42,50» y leyendolo a la inglesa saldrian 4.250 €, o sea que
//! cambiar de idioma multiplicaria los gastos por cien.
//!
//! Al **leer** se acepta lo que escribiria una persona: coma o punto,
//! separadores de miles, el simbolo de la moneda al lado, espacios.
//!
//! ## El total no se guarda
//!
//! La linea `**Total:**` del final se vuelve a calcular en cada escritura y al
//! leer se **ignora**. Un total guardado es un dato que puede contradecir a
//! sus propias filas —basta editar una linea con otro programa— y entonces hay
//! dos verdades y ninguna forma de saber cual manda. Esta ahi para que el
//! documento se entienda al abrirlo por fuera, no como fuente de nada.

use super::{Resumen, en_una_linea, linea_de_titulo, titulo};

/// Una moneda: su codigo de tres letras y cuantos decimales ensena.
///
/// Es el `java.util.Currency` del movil reducido a lo unico que se usa aqui.
/// No se guarda una tabla de todas las monedas del mundo: se guardan las que
/// **no** tienen dos decimales, que son las que romperian la cuenta, y lo
/// demas son dos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moneda {
    codigo: String,
    decimales: u8,
}

/// Monedas sin decimales: un yen es un yen, no cien centimos de yen.
///
/// La lista es la de `java.util.Currency` con `defaultFractionDigits == 0`,
/// **entera**: a una moneda que falte aqui se le dan dos decimales, y
/// entonces el importe se multiplica por cien en cada ida y vuelta con el
/// movil sin que nadie vea un error. XOF, XAF, XPF, UGX y UYI faltaban.
const SIN_DECIMALES: [&str; 17] = [
    "BIF", "CLP", "DJF", "GNF", "ISK", "JPY", "KMF", "KRW", "PYG", "RWF", "UGX", "UYI", "VND",
    "VUV", "XAF", "XOF", "XPF",
];

/// Monedas de tres decimales (milesimas).
const TRES_DECIMALES: [&str; 7] = ["BHD", "IQD", "JOD", "KWD", "LYD", "OMR", "TND"];

impl Moneda {
    /// La moneda de ese codigo, o `None` si eso no es un codigo de moneda.
    ///
    /// `None` y no un apano: un `(EURO)` mal escrito en la cabecera tiene que
    /// caer a la moneda del aparato, no inventar una que no existe.
    pub fn de_codigo(codigo: &str) -> Option<Self> {
        let limpio = codigo.trim();
        if limpio.len() != 3 || !limpio.chars().all(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        let codigo = limpio.to_uppercase();
        let decimales = if SIN_DECIMALES.contains(&codigo.as_str()) {
            0
        } else if TRES_DECIMALES.contains(&codigo.as_str()) {
            3
        } else {
            2
        };
        Some(Self { codigo, decimales })
    }

    /// El euro: lo que queda cuando el sistema no sabe decir nada. No es una
    /// eleccion, y en cuanto el documento se guarda deja de importar porque la
    /// moneda ya viaja dentro.
    pub fn euro() -> Self {
        Self {
            codigo: "EUR".to_string(),
            decimales: 2,
        }
    }

    pub fn codigo(&self) -> &str {
        &self.codigo
    }

    /// Cuantos decimales ensena esta moneda. El yen: cero.
    pub fn decimales(&self) -> u8 {
        self.decimales
    }

    /// El simbolo, o el codigo si no tiene uno conocido.
    ///
    /// La lista es corta a proposito: un codigo de tres letras se entiende
    /// siempre, y un simbolo inventado no.
    pub fn simbolo(&self) -> &str {
        match self.codigo.as_str() {
            "EUR" => "€",
            "USD" => "$",
            "GBP" => "£",
            "JPY" => "¥",
            otro => otro,
        }
    }
}

/// Una linea de la cuenta. `centimos` es en unidades menores de la moneda del
/// [`Libro`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gasto {
    pub concepto: String,
    pub centimos: i64,
}

/// Una cuenta entera: su nombre, en que moneda esta y sus lineas.
///
/// La moneda vive en el libro y no en cada gasto porque una cuenta con lineas
/// en monedas distintas **no se puede sumar**, y un total que suma libras con
/// euros es exactamente el numero que no hay que ensenar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Libro {
    pub titulo: String,
    pub moneda: Moneda,
    pub gastos: Vec<Gasto>,
}

impl Libro {
    /// La suma, exacta. Ver [`total`].
    pub fn total(&self) -> i64 {
        total(&self.gastos)
    }
}

// ---- Leer y escribir -----------------------------------------------------

/// El libro que hay escrito en `documento`.
///
/// La **primera** fila de la tabla es siempre la cabecera y no se lee como
/// gasto: es de donde sale la moneda. Las filas de guiones se saltan. De las
/// demas, el concepto es la primera celda y el importe **la ultima** —la
/// ultima y no la segunda, para que una fila con una columna de mas pegada por
/// alguien siga dando su importe—.
///
/// Un importe que no se entienda vale **cero**, y la fila se queda. Tirar la
/// fila borraria el concepto que el usuario si escribio; dejarla a cero ensena
/// el problema donde se puede arreglar, que es en la propia lista.
///
/// `por_defecto` solo se usa si el documento no dice su moneda: una tabla
/// pegada de fuera, sin el `(EUR)` en la cabecera.
pub fn leer(documento: &str, por_defecto: &Moneda) -> Libro {
    let filas: Vec<&str> = documento
        .lines()
        .map(str::trim)
        .filter(|f| f.starts_with('|'))
        .collect();
    let moneda = filas
        .first()
        .and_then(|c| codigo_en(c))
        .and_then(|c| Moneda::de_codigo(&c))
        .unwrap_or_else(|| por_defecto.clone());
    let decimales = moneda.decimales();

    let gastos = filas
        .iter()
        .skip(1)
        .filter_map(|fila| {
            let celdas = celdas(fila);
            if celdas.len() < 2 || celdas.iter().all(|c| es_guiones(c)) {
                return None;
            }
            Some(Gasto {
                concepto: en_una_linea(&celdas[0]),
                centimos: centimos_de(celdas.last()?, decimales).unwrap_or(0),
            })
        })
        .collect();
    Libro {
        titulo: titulo(documento),
        moneda,
        gastos,
    }
}

/// El documento entero.
pub fn escribir(libro: &Libro) -> String {
    let mut d = linea_de_titulo(&libro.titulo);
    // Los rotulos de la cabecera son parte del documento, no de la interfaz:
    // se leen dentro del propio texto. Cambiarlos algun dia no rompe nada,
    // porque al leer la cabecera solo se mira buscando el codigo de la moneda.
    d.push_str("| Concepto | Importe (");
    d.push_str(libro.moneda.codigo());
    d.push_str(") |\n");
    // La columna de importes a la derecha: los numeros se comparan por su
    // ultima cifra, y alineados a la izquierda hay que leer cada uno entero
    // para ver cual es mayor.
    d.push_str("| --- | ---: |\n");
    for g in &libro.gastos {
        d.push_str("| ");
        d.push_str(&celda_segura(&g.concepto));
        d.push_str(" | ");
        d.push_str(&canonico(g.centimos, libro.moneda.decimales()));
        d.push_str(" |\n");
    }
    d.push_str("\n**Total: ");
    d.push_str(&texto_de_importe(libro.total(), &libro.moneda));
    d.push_str("**");
    d
}

// ---- Operaciones ---------------------------------------------------------

/// Anade una linea.
///
/// Un concepto vacio **si entra** si trae importe, al reves que en las tareas:
/// un gasto de doce euros que uno no sabe como llamar sigue siendo doce euros
/// que hay que sumar, y rechazarlo descuadraria el total, que es lo unico que
/// esta mini-app promete. Lo que no entra es una linea sin concepto **y** sin
/// importe, que no es nada.
pub fn anadir(libro: &Libro, concepto: &str, centimos: i64) -> Libro {
    let limpio = en_una_linea(concepto);
    if limpio.is_empty() && centimos == 0 {
        return libro.clone();
    }
    let mut nuevo = libro.clone();
    nuevo.gastos.push(Gasto {
        concepto: limpio,
        centimos: acotado(centimos),
    });
    nuevo
}

pub fn borrar(libro: &Libro, cual: usize) -> Libro {
    if cual >= libro.gastos.len() {
        return libro.clone();
    }
    let mut nuevo = libro.clone();
    nuevo.gastos.remove(cual);
    nuevo
}

pub fn cambiar(libro: &Libro, cual: usize, concepto: &str, centimos: i64) -> Libro {
    if cual >= libro.gastos.len() {
        return libro.clone();
    }
    let mut nuevo = libro.clone();
    nuevo.gastos[cual] = Gasto {
        concepto: en_una_linea(concepto),
        centimos: acotado(centimos),
    };
    nuevo
}

/// Mueve una linea. El destino se **recorta** en vez de rechazarse: arrastrar
/// mas alla del final quiere decir «al final».
pub fn mover(libro: &Libro, desde: usize, hasta: usize) -> Libro {
    if desde >= libro.gastos.len() {
        return libro.clone();
    }
    let destino = hasta.min(libro.gastos.len() - 1);
    if destino == desde {
        return libro.clone();
    }
    let mut nuevo = libro.clone();
    let g = nuevo.gastos.remove(desde);
    nuevo.gastos.insert(destino, g);
    nuevo
}

/// La suma.
///
/// Se acumula acotando en cada paso en vez de sumar y ya. Los importes de uno
/// en uno ya estan acotados, pero un documento pegado con cien mil filas del
/// maximo desbordaria el entero y el total saldria **negativo**: una cuenta de
/// gastos que dice que te deben dinero. Acotando, el total se queda en un
/// numero absurdo pero del signo correcto, que al menos se ve que es absurdo.
pub fn total(gastos: &[Gasto]) -> i64 {
    gastos.iter().fold(0i64, |acc, g| {
        acc.saturating_add(g.centimos)
            .clamp(-TOPE_TOTAL, TOPE_TOTAL)
    })
}

/// El total, que es la razon de existir de la mini-app.
///
/// En la burbuja va el total y no «cinco conceptos»: lo que uno quiere saber
/// de un control de gastos sin abrirlo es cuanto lleva gastado.
pub fn resumen(documento: &str, por_defecto: &Moneda) -> Resumen {
    let libro = leer(documento, por_defecto);
    Resumen {
        texto: texto_de_importe(libro.total(), &libro.moneda),
        de: libro.gastos.len(),
        vacia: libro.gastos.is_empty(),
        ..Resumen::default()
    }
}

// ---- Importes: leer, escribir y pintar -----------------------------------

/// El importe tal cual se guarda: `42.50`, `-5.00`, `1200` (yenes).
///
/// Se compone desde el entero, cifra a cifra, sin pasar por coma flotante en
/// ningun momento: no hay ningun paso donde se pueda perder un centimo.
pub fn canonico(centimos: i64, decimales: u8) -> String {
    let d = decimales.min(6) as usize;
    if d == 0 {
        return centimos.to_string();
    }
    let signo = if centimos < 0 { "-" } else { "" };
    let mut cifras = centimos.unsigned_abs().to_string();
    if cifras.len() <= d {
        cifras.insert_str(0, &"0".repeat(d + 1 - cifras.len()));
    }
    let (enteras, fraccion) = cifras.split_at(cifras.len() - d);
    format!("{signo}{enteras}.{fraccion}")
}

/// El importe como se ensena: `42,50 €`, `1.234,56 €`, `1200 JPY`.
///
/// Es **la unica parte del documento que se escribe para leerla**, y por eso
/// la unica que sigue la forma de escribir numeros de aqui —coma decimal y
/// punto de millares— en vez de la canonica. El movil la compone con la
/// configuracion regional del telefono, asi que este renglon puede salir con
/// otra puntuacion en cada aparato; da igual, porque **al leer se ignora**:
/// el total se recalcula siempre de las filas.
///
/// Lo que si se copia es el **espacio duro** (U+00A0) entre la cifra y el
/// simbolo: es el que pone CLDR para `es-ES` (`#,##0.00 ¤`), que es lo que usa
/// `NumberFormat` en el movil del usuario. Con un espacio normal el renglon
/// del total salia distinto en los dos aparatos, y el documento, que se
/// compara entero al sincronizar, cambiaba de texto sin que cambiara nada.
pub fn texto_de_importe(centimos: i64, moneda: &Moneda) -> String {
    let canon = canonico(centimos, moneda.decimales());
    let (enteras, fraccion) = match canon.split_once('.') {
        Some((e, f)) => (e, Some(f)),
        None => (canon.as_str(), None),
    };
    let negativo = enteras.starts_with('-');
    let digitos = enteras.trim_start_matches('-');
    let mut con_miles = String::new();
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i) % 3 == 0 {
            con_miles.push('.');
        }
        con_miles.push(c);
    }
    let mut salida = String::new();
    if negativo {
        salida.push('-');
    }
    salida.push_str(&con_miles);
    if let Some(f) = fraccion {
        salida.push(',');
        salida.push_str(f);
    }
    salida.push('\u{00A0}');
    salida.push_str(moneda.simbolo());
    salida
}

/// Lo que ha tecleado una persona, en unidades menores. `None` si ahi no hay
/// un numero.
///
/// ## Como se decide que separador es el decimal
///
/// Se mira **el ultimo** punto o coma del texto: si detras lleva entre una y
/// `decimales` cifras, es el separador decimal; si lleva mas, es un separador
/// de miles y el numero es entero. Con esa regla salen bien las cuatro formas
/// que escribe la gente —`1234.5`, `1.234,56`, `1,234.56`, `1234`— sin
/// preguntarle a nadie en que idioma esta pensando.
///
/// No es adivinacion perfecta y no puede serlo: `1.005` es mil cinco para
/// media Europa y un euro con medio centimo para la otra media. Se lee como
/// mil cinco, que es lo que dice la regla, y no importa demasiado porque
/// **esto solo se usa con lo que teclea el usuario y con lo editado a mano**:
/// lo que escribe la aplicacion es siempre canonico y cae en el caso facil.
///
/// Por eso mismo no hay redondeo en ningun sitio: cuando detras del separador
/// hay mas cifras de las que tiene la moneda, la regla ya ha decidido que ese
/// separador era de millares, y el numero entra entero. `0,999` con euros son
/// novecientos noventa y nueve euros, no un euro. Es raro escrito asi, pero es
/// lo que hace el movil y **tiene que dar lo mismo en los dos aparatos**.
pub fn centimos_de(texto: &str, decimales: u8) -> Option<i64> {
    let d = decimales.min(6) as usize;
    // El menos de teclado y el menos tipografico: el segundo llega copiando de
    // una hoja de calculo o de una web, y sin tratarlo un reembolso se
    // guardaria como un gasto.
    let limpio = texto.replace('−', "-");
    let limpio = limpio.trim();
    let pos = limpio.find(|c: char| c.is_ascii_digit())?;
    let negativo = limpio[..pos].contains('-');

    let corte = limpio.rfind(['.', ',']);
    let es_decimal = corte.is_some_and(|c| {
        let detras = limpio[c + 1..].chars().filter(char::is_ascii_digit).count();
        (1..=d).contains(&detras)
    });

    let solo_cifras = |t: &str| -> String { t.chars().filter(char::is_ascii_digit).collect() };
    let (enteras, fraccion) = match corte {
        Some(c) if es_decimal => (solo_cifras(&limpio[..c]), solo_cifras(&limpio[c + 1..])),
        _ => (solo_cifras(limpio), String::new()),
    };

    let mut junto = enteras;
    junto.push_str(&fraccion);
    // Lo que falte para los decimales de la moneda, a ceros: «12,5 €» son
    // doce con cincuenta, no doce con cinco centimos.
    for _ in fraccion.len()..d {
        junto.push('0');
    }
    // Un numero que no cabe se rechaza entero: doscientas cifras pegadas no
    // son un importe, son un accidente, y mas vale no aceptarlo que aceptar su
    // resto.
    let sin_ceros = junto.trim_start_matches('0');
    let valor: i64 = if sin_ceros.is_empty() {
        0
    } else {
        sin_ceros.parse().ok()?
    };
    Some(acotado(if negativo { -valor } else { valor }))
}

/// El importe recortado a algo que sea un importe.
///
/// No es paranoia: el campo de texto acepta lo que sea y pegar una tira de
/// cifras es un segundo de trabajo. El tope son mil billones de unidades
/// menores —diez billones de euros—, muy por encima de cualquier cuenta real y
/// muy por debajo de donde un entero de 64 bits empieza a dar problemas al
/// sumar.
pub fn acotado(centimos: i64) -> i64 {
    centimos.clamp(-TOPE_IMPORTE, TOPE_IMPORTE)
}

const TOPE_IMPORTE: i64 = 1_000_000_000_000_000;
const TOPE_TOTAL: i64 = 100_000_000_000_000_000;

// ---- Celdas --------------------------------------------------------------

/// Un concepto listo para meter en una celda.
///
/// La barra vertical **parte la fila**: un gasto llamado «pan | leche» se
/// guardaria como tres columnas y al releerlo el importe seria «leche». Se
/// escapa como `\|`, que es lo que desescapa cualquier lector de tablas de
/// Markdown.
///
/// La barra invertida tambien se escapa. Sin eso, un concepto que ya acabara
/// en `\` se comeria la barra siguiente al releerlo y la fila se partiria
/// igual: el escape solo funciona si el caracter de escape tambien se escapa.
pub fn celda_segura(texto: &str) -> String {
    en_una_linea(texto)
        .replace('\\', "\\\\")
        .replace('|', "\\|")
}

/// Las celdas de una fila, deshaciendo el escape de [`celda_segura`].
fn celdas(fila: &str) -> Vec<String> {
    let cuerpo = fila.trim();
    let cuerpo = cuerpo.strip_prefix('|').unwrap_or(cuerpo);
    let cuerpo = cuerpo.strip_suffix('|').unwrap_or(cuerpo);
    let mut salida = Vec::new();
    let mut actual = String::new();
    let mut cs = cuerpo.chars().peekable();
    while let Some(c) = cs.next() {
        match c {
            '\\' if cs.peek().is_some() => actual.push(cs.next().unwrap_or('\\')),
            '|' => {
                salida.push(actual.trim().to_string());
                actual.clear();
            }
            otro => actual.push(otro),
        }
    }
    salida.push(actual.trim().to_string());
    salida
}

/// Una fila de guiones: la que separa la cabecera del cuerpo en una tabla de
/// Markdown, con o sin los dos puntos de la alineacion.
fn es_guiones(celda: &str) -> bool {
    let s = celda.strip_prefix(':').unwrap_or(celda);
    let s = s.strip_suffix(':').unwrap_or(s);
    s.len() >= 3 && s.chars().all(|c| c == '-')
}

/// `| Concepto | Importe (EUR) |` → `EUR`. Vacio si la cabecera no lo dice.
fn codigo_en(fila: &str) -> Option<String> {
    let cs: Vec<char> = fila.chars().collect();
    for (i, c) in cs.iter().enumerate() {
        if *c != '(' {
            continue;
        }
        let mut j = i + 1;
        while cs.get(j).is_some_and(|c| c.is_whitespace()) {
            j += 1;
        }
        let Some(letras) = cs.get(j..j + 3) else {
            continue;
        };
        if !letras.iter().all(|c| c.is_ascii_alphabetic()) {
            continue;
        }
        let mut k = j + 3;
        while cs.get(k).is_some_and(|c| c.is_whitespace()) {
            k += 1;
        }
        if cs.get(k) == Some(&')') {
            return Some(letras.iter().collect());
        }
    }
    None
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn libro(gastos: &[(&str, i64)]) -> Libro {
        Libro {
            titulo: "Viaje a Lisboa".to_string(),
            moneda: Moneda::euro(),
            gastos: gastos
                .iter()
                .map(|(c, v)| Gasto {
                    concepto: c.to_string(),
                    centimos: *v,
                })
                .collect(),
        }
    }

    #[test]
    fn el_documento_de_gastos_se_escribe_como_en_el_movil() {
        let d = escribir(&libro(&[
            ("Cena", 4250),
            ("Tren", 1800),
            ("Devolución", -500),
        ]));
        assert_eq!(
            d,
            "# Viaje a Lisboa\n\n\
             | Concepto | Importe (EUR) |\n\
             | --- | ---: |\n\
             | Cena | 42.50 |\n\
             | Tren | 18.00 |\n\
             | Devolución | -5.00 |\n\
             \n**Total: 55,50\u{a0}€**"
        );
        let vuelta = leer(&d, &Moneda::euro());
        assert_eq!(
            vuelta,
            libro(&[("Cena", 4250), ("Tren", 1800), ("Devolución", -500)])
        );
        assert_eq!(vuelta.total(), 5550);
    }

    #[test]
    fn nace_con_la_tabla_puesta_y_sin_filas() {
        let d = escribir(&Libro {
            titulo: "Obra".to_string(),
            moneda: Moneda::euro(),
            gastos: Vec::new(),
        });
        assert_eq!(
            d,
            "# Obra\n\n| Concepto | Importe (EUR) |\n| --- | ---: |\n\n**Total: 0,00\u{a0}€**"
        );
        assert!(resumen(&d, &Moneda::euro()).vacia);
    }

    #[test]
    fn la_moneda_viaja_dentro_del_documento() {
        let yenes = Libro {
            titulo: "Tokio".to_string(),
            moneda: Moneda::de_codigo("jpy").expect("JPY existe"),
            gastos: vec![Gasto {
                concepto: "Ramen".to_string(),
                centimos: 1200,
            }],
        };
        let d = escribir(&yenes);
        assert!(d.contains("| Importe (JPY) |"), "{d}");
        assert!(d.contains("| Ramen | 1200 |"), "el yen no tiene centimos");
        assert!(d.ends_with("**Total: 1.200\u{a0}¥**"), "{d}");
        // Y al releerlo con un PC en euros, sigue siendo en yenes.
        let vuelta = leer(&d, &Moneda::euro());
        assert_eq!(vuelta.moneda.codigo(), "JPY");
        assert_eq!(vuelta.gastos[0].centimos, 1200);
    }

    #[test]
    fn una_tabla_pegada_sin_moneda_usa_la_del_aparato() {
        let d = "| Concepto | Importe |\n| --- | ---: |\n| Cena | 42,50 |";
        let l = leer(d, &Moneda::euro());
        assert_eq!(l.moneda.codigo(), "EUR");
        assert_eq!(l.gastos[0].centimos, 4250, "la coma tambien se lee");
        assert_eq!(l.titulo, "", "sin titulo no se inventa uno");
        // Y un codigo que no es un codigo tampoco inventa moneda.
        assert_eq!(Moneda::de_codigo("EURO"), None);
        assert_eq!(Moneda::de_codigo("E1R"), None);
    }

    #[test]
    fn un_importe_ilegible_vale_cero_pero_no_borra_su_concepto() {
        let d = "| Concepto | Importe (EUR) |\n| --- | ---: |\n| Fontanero | lo que sea |\n| Cena | 10 |";
        let l = leer(d, &Moneda::euro());
        assert_eq!(l.gastos.len(), 2, "la fila se queda: {:?}", l.gastos);
        assert_eq!(l.gastos[0].concepto, "Fontanero");
        assert_eq!(l.gastos[0].centimos, 0);
        assert_eq!(l.gastos[1].centimos, 1000, "«10» son diez euros");
    }

    #[test]
    fn una_fila_con_una_columna_de_mas_sigue_dando_su_importe() {
        let d = "| Concepto | Nota | Importe (EUR) |\n| :--- | --- | ---: |\n| Cena | con Ana | 42.50 |";
        let l = leer(d, &Moneda::euro());
        assert_eq!(l.gastos.len(), 1, "la fila de guiones no cuenta");
        assert_eq!(l.gastos[0].concepto, "Cena");
        assert_eq!(l.gastos[0].centimos, 4250, "el importe es la ultima celda");
    }

    #[test]
    fn una_barra_en_el_concepto_no_parte_la_fila() {
        let l = libro(&[("pan | leche", 100), ("copia\\", 200)]);
        let d = escribir(&l);
        assert!(d.contains(r"| pan \| leche | 1.00 |"), "{d}");
        assert_eq!(leer(&d, &Moneda::euro()).gastos, l.gastos, "ida y vuelta");
    }

    #[test]
    fn los_importes_se_leen_como_los_escribe_la_gente() {
        assert_eq!(centimos_de("1234.5", 2), Some(123_450));
        assert_eq!(centimos_de("1.234,56", 2), Some(123_456));
        assert_eq!(centimos_de("1,234.56", 2), Some(123_456));
        assert_eq!(centimos_de("1234", 2), Some(123_400));
        assert_eq!(centimos_de("42,50 €", 2), Some(4250));
        assert_eq!(centimos_de("−5", 2), Some(-500), "el menos tipografico");
        assert_eq!(centimos_de("- 5,00", 2), Some(-500));
        assert_eq!(
            centimos_de("1.005", 2),
            Some(100_500),
            "la regla dice mil cinco"
        );
        assert_eq!(
            centimos_de("1,234", 0),
            Some(1234),
            "el yen no tiene decimales"
        );
        assert_eq!(centimos_de("12,5", 2), Some(1250), "doce con cincuenta");
        // Y la cara fea de la regla, que es la del movil y se deja igual:
        // tres cifras detras no caben como decimales de euro, asi que la coma
        // pasa a ser de millares y esto son novecientos noventa y nueve euros.
        assert_eq!(centimos_de("0,999", 2), Some(99_900));
    }

    /// Una moneda sin decimales que se creyera de dos multiplica el importe
    /// por cien en cada ida y vuelta con el movil, y en pantalla se lee bien:
    /// nadie ve el error hasta que el total es cien veces mayor.
    #[test]
    fn las_monedas_sin_decimales_son_todas_las_de_java_y_no_solo_el_yen() {
        for codigo in [
            "BIF", "CLP", "DJF", "GNF", "ISK", "JPY", "KMF", "KRW", "PYG", "RWF", "UGX", "UYI",
            "VND", "VUV", "XAF", "XOF", "XPF",
        ] {
            let m = Moneda::de_codigo(codigo).unwrap_or_else(|| panic!("{codigo} no es moneda"));
            assert_eq!(m.decimales(), 0, "{codigo} tiene que ir sin decimales");
            // Ida y vuelta: mil francos CFA siguen siendo mil, no cien mil.
            assert_eq!(
                centimos_de(&texto_de_importe(1000, &m), m.decimales()),
                Some(1000),
                "{codigo} cambia de valor al ir y volver"
            );
        }
        // Caso negativo: el euro y el dinar no entran en la lista por estar
        // cerca; la lista es exacta, no «las raras».
        assert_eq!(Moneda::de_codigo("EUR").unwrap().decimales(), 2);
        assert_eq!(Moneda::de_codigo("KWD").unwrap().decimales(), 3);
        // Y «XOF» mal escrito no se cuela como moneda sin decimales.
        assert_eq!(Moneda::de_codigo("XO"), None);
    }

    #[test]
    fn lo_que_no_es_un_numero_no_es_un_importe() {
        assert_eq!(centimos_de("", 2), None);
        assert_eq!(centimos_de("lo que sea", 2), None);
        assert_eq!(centimos_de("€", 2), None);
        // Doscientas cifras no son un importe grande, son otra cosa.
        assert_eq!(centimos_de(&"9".repeat(200), 2), None);
        assert_eq!(centimos_de("0", 2), Some(0));
    }

    #[test]
    fn el_canonico_no_pasa_por_coma_flotante() {
        assert_eq!(canonico(4250, 2), "42.50");
        assert_eq!(canonico(-500, 2), "-5.00");
        assert_eq!(canonico(5, 2), "0.05");
        assert_eq!(canonico(-5, 2), "-0.05");
        assert_eq!(canonico(1200, 0), "1200");
        assert_eq!(canonico(1, 3), "0.001");
        // Diez veces diez centimos suman exactamente un euro.
        let diez = (0..10)
            .map(|_| Gasto {
                concepto: String::new(),
                centimos: 10,
            })
            .collect::<Vec<_>>();
        assert_eq!(canonico(total(&diez), 2), "1.00");
    }

    #[test]
    fn el_total_no_se_desborda_ni_cambia_de_signo() {
        let muchos = (0..200)
            .map(|_| Gasto {
                concepto: String::new(),
                centimos: TOPE_IMPORTE,
            })
            .collect::<Vec<_>>();
        assert_eq!(total(&muchos), TOPE_TOTAL, "absurdo, pero positivo");
        assert_eq!(acotado(i64::MIN), -TOPE_IMPORTE);
    }

    #[test]
    fn las_operaciones_respetan_lo_que_hay() {
        let l = libro(&[("a", 100), ("b", 200)]);
        assert_eq!(anadir(&l, "  ", 0), l, "ni concepto ni importe no es nada");
        assert_eq!(
            anadir(&l, "  ", 500).gastos[2].centimos,
            500,
            "sin nombre si"
        );
        assert_eq!(borrar(&l, 9), l, "una fila que no existe");
        assert_eq!(cambiar(&l, 9, "x", 1), l);
        assert_eq!(mover(&l, 9, 0), l);
        assert_eq!(
            mover(&l, 0, 9).gastos[1].concepto,
            "a",
            "el destino se recorta"
        );
        assert_eq!(borrar(&l, 0).gastos.len(), 1);
    }

    #[test]
    fn el_total_guardado_no_manda_sobre_las_filas() {
        // Un total mentiroso, como el que deja editar el documento a mano.
        let d =
            "| Concepto | Importe (EUR) |\n| --- | ---: |\n| Cena | 10.00 |\n\n**Total: 999,00 €**";
        let l = leer(d, &Moneda::euro());
        assert_eq!(l.gastos.len(), 1, "la linea del total no es una fila");
        assert_eq!(l.total(), 1000);
        assert!(escribir(&l).ends_with("**Total: 10,00\u{a0}€**"));
    }

    #[test]
    fn el_total_se_lee_con_sus_millares() {
        assert_eq!(
            texto_de_importe(123_456, &Moneda::euro()),
            "1.234,56\u{a0}€"
        );
        assert_eq!(texto_de_importe(-500, &Moneda::euro()), "-5,00\u{a0}€");
        assert_eq!(texto_de_importe(0, &Moneda::euro()), "0,00\u{a0}€");
        let kwd = Moneda::de_codigo("KWD").expect("KWD existe");
        assert_eq!(
            texto_de_importe(1234, &kwd),
            "1,234\u{a0}KWD",
            "tres decimales"
        );
    }
}
