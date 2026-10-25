//! **Un libro de hojas de calculo convertido en tablas de PixPin** (D11):
//! cada hoja, una [`Tabla`] con sus valores, sus formulas, su negrita, sus
//! fondos y sus anchos. Es `motor/ImportarHojas.kt` del movil, regla a regla.
//!
//! # Que se lee
//!
//! - **`.xlsx`/`.xlsm`** (Excel, y lo que baja Google Sheets): un ZIP de XML.
//! - **`.ods`** (LibreOffice, y Sheets tambien): igual, con otro vocabulario.
//! - **`.csv` y `.tsv`**, con el separador que traigan (`;` en la Excel
//!   espanola, `,` en la inglesa).
//! - Un **`.xls`** antiguo es binario (BIFF) y **no se lee**: se dice, en vez
//!   de ensenar basura ([`NoSeLee::XlsAntiguo`]).
//!
//! El XML se recorre **de corrido**, sin montar el arbol ([`sax`]): el movil
//! usa SAX para que un libro de cien mil celdas no reviente la memoria del
//! telefono, y aqui la maquina suelo tiene 4 GB. El lector DOM de
//! `pixpin-docs` guarda cada letra en 4 bytes y cada nodo aparte: para una
//! hoja de 20 MB serian cientos de megas.
//!
//! # Formulas que no se saben calcular
//!
//! Excel tiene cuatrocientas funciones y aqui hay pocas; y hay cosas que una
//! tabla suelta no puede tener, como `Hoja2!A1`. El libro guarda **el ultimo
//! resultado** de cada formula, asi que se calcula todo y se compara: donde
//! lo de aqui no coincide con lo que dijo Excel, la celda se queda **con su
//! valor** en vez de ensenar un error o un total distinto. Solo esa celda:
//! las que dependen de ella siguen siendo formulas y vuelven a cuadrar. Ver
//! [`con_valores_donde_no_cuadra`].

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::Path;

use crate::formula::{self, Valor};
use crate::tabla::{EstiloDeCelda, Ref, Tabla, ref_a, ref_de};

/// Hasta donde llega una tabla, los mismos topes que el movil
/// (`Celdas.MAX_COLS`, `Celdas.MAX_FILAS`): la columna `ZZ` y cien mil filas.
pub const MAX_COLS: u32 = 702;
pub const MAX_FILAS: u32 = 100_000;
/// El ancho de una columna, en pixeles de la tabla (`TablaViva.ANCHO_MIN/MAX`).
const ANCHO_MIN: u32 = 28;
const ANCHO_MAX: u32 = 600;
/// Lo que se descomprime de un libro como mucho: un ZIP de 1 MB puede
/// inflarse a gigas (la «bomba ZIP»), y aqui se lee todo en memoria.
const TOPE_DESCOMPRIMIDO: u64 = 512 * 1024 * 1024;

/// Una hoja del libro, ya tabla.
#[derive(Debug, Clone, PartialEq)]
pub struct HojaImportada {
    pub nombre: String,
    pub tabla: Tabla,
    /// Cuantas formulas se quedaron con su valor porque aqui no se sabian
    /// calcular.
    pub formulas_como_valor: usize,
}

impl HojaImportada {
    fn llamada(mut self, otro: String) -> HojaImportada {
        self.tabla.nombre = otro.clone();
        self.nombre = otro;
        self
    }
}

/// Lo que no se puede leer, con el porque. Quien lo ensena lo traduce.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NoSeLee {
    /// Un `.xls` de antes de 2007: binario, no se lee. Guardarlo como
    /// `.xlsx` lo arregla.
    #[error("los .xls antiguos no se pueden leer: guardalo como .xlsx")]
    XlsAntiguo,
    #[error("el archivo no es un libro de hojas valido")]
    NoEsUnLibro,
    #[error("el libro no tiene ninguna hoja con datos")]
    SinHojas,
    #[error("no se pudo leer el archivo: {0}")]
    Disco(String),
}

const LEGIBLES: [&str; 5] = ["xlsx", "xlsm", "ods", "csv", "tsv"];

/// La extension, en minusculas y sin el punto.
pub fn extension(nombre: &str) -> String {
    match nombre.rsplit_once('.') {
        Some((_, e)) if !e.contains(['/', '\\']) => e.to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// Si el archivo es un libro de hojas (o un `.xls`, que se reconoce para
/// decir que no se lee).
pub fn es_libro(nombre: &str) -> bool {
    let e = extension(nombre);
    LEGIBLES.contains(&e.as_str()) || e == "xls"
}

/// **Las hojas de `ruta`.** `nombre` es con el que llego (el del mensaje; el
/// fichero copiado puede no tener extension). Una sola hoja se llama como el
/// archivo; varias, «archivo · hoja».
pub fn leer(ruta: &Path, nombre: &str, ahora: i64) -> Result<Vec<HojaImportada>, NoSeLee> {
    let bytes = std::fs::read(ruta).map_err(|e| NoSeLee::Disco(e.to_string()))?;
    let de_ruta = ruta.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let nombre = if nombre.trim().is_empty() {
        de_ruta
    } else {
        nombre
    };
    leer_bytes(&bytes, nombre, de_ruta, ahora)
}

/// Lo mismo desde los bytes: `nombre` manda para saber que es, y si no
/// trae extension, `respaldo` (el nombre del fichero).
pub fn leer_bytes(
    bytes: &[u8],
    nombre: &str,
    respaldo: &str,
    ahora: i64,
) -> Result<Vec<HojaImportada>, NoSeLee> {
    let mut ext = extension(nombre);
    if ext.is_empty() {
        ext = extension(respaldo);
    }
    let base = match nombre.rsplit_once('.') {
        Some((b, _)) if !b.trim().is_empty() => b.trim().to_string(),
        _ if !nombre.trim().is_empty() && !nombre.contains('.') => nombre.trim().to_string(),
        _ => "Tabla".to_string(),
    };
    let hojas = match ext.as_str() {
        "xlsx" | "xlsm" => de_xlsx(bytes, ahora)?,
        "ods" => de_ods(bytes, ahora)?,
        "csv" | "tsv" => vec![de_csv(&texto_de(bytes), &base, ext == "tsv", ahora)],
        "xls" => return Err(NoSeLee::XlsAntiguo),
        _ => {
            if bytes.starts_with(b"PK") {
                let z = entradas(bytes)?;
                if z.contains_key("content.xml") {
                    de_ods(bytes, ahora)?
                } else {
                    de_xlsx(bytes, ahora)?
                }
            } else if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
                // La firma de los ficheros OLE: un .xls sin extension.
                return Err(NoSeLee::XlsAntiguo);
            } else {
                vec![de_csv(&texto_de(bytes), &base, false, ahora)]
            }
        }
    };
    if hojas.is_empty() || hojas.iter().all(|h| h.tabla.celdas.is_empty()) {
        return Err(NoSeLee::SinHojas);
    }
    Ok(if hojas.len() == 1 {
        hojas.into_iter().map(|h| h.llamada(base.clone())).collect()
    } else {
        hojas
            .into_iter()
            .map(|h| {
                let n = format!("{base} · {}", h.nombre);
                h.llamada(n)
            })
            .collect()
    })
}

// ---------------------------------------------------------------------------
// Lo comun

/// Los `.xml` y `.rels` del ZIP, con su texto.
fn entradas(bytes: &[u8]) -> Result<HashMap<String, String>, NoSeLee> {
    let mut zip =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| NoSeLee::NoEsUnLibro)?;
    let mut salida = HashMap::new();
    let mut total: u64 = 0;
    for i in 0..zip.len() {
        let Ok(mut e) = zip.by_index(i) else {
            continue;
        };
        let nombre = e.name().trim_start_matches('/').to_string();
        if !(nombre.ends_with(".xml") || nombre.ends_with(".rels")) {
            continue;
        }
        let mut texto = String::new();
        let queda = TOPE_DESCOMPRIMIDO.saturating_sub(total);
        let leido = (&mut e)
            .take(queda + 1)
            .read_to_string(&mut texto)
            .map_err(|_| NoSeLee::NoEsUnLibro)?;
        total += leido as u64;
        if total > TOPE_DESCOMPRIMIDO {
            return Err(NoSeLee::NoEsUnLibro);
        }
        salida.insert(nombre, texto);
    }
    Ok(salida)
}

/// UTF-8, y si no lo es, Latin-1: el CSV de la Excel de Windows.
fn texto_de(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(t) => t.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// Un texto que, escrito tal cual, se leeria como numero o formula, va con
/// `'` delante (`textoComoLiteral`): el `00123` de un codigo sigue siendo
/// `00123`.
fn texto_como_literal(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    if s.starts_with('=') || s.starts_with('\'') || !matches!(formula::literal(s), Valor::Texto(_))
    {
        Some(format!("'{s}"))
    } else {
        Some(s.to_string())
    }
}

/// Un numero guardado con sus diecisiete cifras (`0.30000000000000004`), a
/// quince, sin ceros de cola y sin notacion cientifica.
fn limpio(d: f64) -> String {
    if d == 0.0 || !d.is_finite() {
        return "0".into();
    }
    let redondo: f64 = format!("{d:.14e}").parse().unwrap_or(d);
    // `Display` de Rust da el mas corto que vuelve al mismo numero, y nunca
    // en notacion cientifica: justo el `toPlainString` del movil.
    format!("{redondo}")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TipoNumero {
    General,
    Fecha,
    Porcentaje,
}

/// Dias del 30-dic-1899 (el cero de Excel) al 1-ene-1970.
const DIAS_HASTA_1970: i64 = 25569;

/// Ano, mes y dia de un dia contado desde 1970 (el algoritmo de Hinnant).
fn civil(dias: i64) -> (i64, u32, u32) {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `dd/mm/aaaa`, y la hora si la lleva (`CalculoFormato.textoDeFecha`).
pub fn texto_de_fecha(serial: f64) -> String {
    if !serial.is_finite() || !(-657_434.0..=2_958_465.0).contains(&serial) {
        return "#¡NUM!".into();
    }
    let (a, m, d) = civil(serial.floor() as i64 - DIAS_HASTA_1970);
    let base = format!("{d:02}/{m:02}/{a}");
    let minutos = ((serial - serial.floor()) * 1440.0).round() as i64;
    if minutos <= 0 || minutos >= 1440 {
        return base;
    }
    format!("{base} {:02}:{:02}", minutos / 60, minutos % 60)
}

fn numero_como_literal(v: &str, tipo: TipoNumero, fecha1904: bool) -> Option<String> {
    let Ok(d) = v.trim().parse::<f64>() else {
        return (!v.trim().is_empty()).then(|| v.to_string());
    };
    Some(match tipo {
        TipoNumero::Fecha => {
            let serial = if fecha1904 { d + 1462.0 } else { d };
            if serial > 0.0 && serial < 2_958_466.0 {
                texto_de_fecha(serial)
            } else {
                limpio(d)
            }
        }
        TipoNumero::Porcentaje => format!("{}%", formula::general(d * 100.0)),
        TipoNumero::General => limpio(d),
    })
}

/// Las funciones que dan otra cosa cada vez: su resultado guardado no tiene
/// por que coincidir, y no por eso estan mal.
fn es_volatil(formula: &str) -> bool {
    let alto = formula.to_uppercase();
    [
        "RAND(",
        "RANDBETWEEN(",
        "NOW(",
        "TODAY(",
        "ALEATORIO(",
        "ALEATORIO.ENTRE(",
        "HOY(",
        "AHORA(",
    ]
    .iter()
    .any(|f| {
        alto.match_indices(f).any(|(i, _)| {
            i == 0
                || !alto[..i]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '.' || c == '_')
        })
    })
}

/// Si lo calculado aqui es lo mismo que el resultado que traia el libro.
fn cuadra(v: &Valor, guardado: &str) -> bool {
    let esperado = if guardado.is_empty() {
        Valor::Texto(String::new())
    } else {
        formula::literal(guardado)
    };
    match (v, &esperado) {
        (Valor::Numero(a), Valor::Numero(b)) => (a - b).abs() <= 1e-9 * b.abs().max(1.0),
        (Valor::Texto(a), Valor::Texto(b)) => a == b,
        // Un error aqui y otro alli: la celda esta rota en los dos sitios, y
        // eso es cuadrar (los textos de los errores cambian de un idioma a
        // otro, `#DIV/0!` y `#¡DIV/0!`).
        (Valor::Error(_), Valor::Texto(b)) => b.starts_with('#'),
        _ => v.to_string() == esperado.to_string(),
    }
}

/// Un rango citado en una formula: `(col1, fila1, col2, fila2)`, todo
/// incluido.
type Rango = (u32, u32, u32, u32);

/// **Donde lo de aqui no da lo que dio Excel, se deja el valor.** Se cambian
/// primero las celdas que no dependen de otras que tampoco cuadran —la raiz
/// del problema—, se vuelve a calcular y se repite: asi `=D5+1` sigue siendo
/// formula aunque `D5` usara una funcion que aqui no hay. Devuelve cuantas
/// celdas se quedaron con su valor.
fn con_valores_donde_no_cuadra(
    celdas: &mut BTreeMap<String, String>,
    guardados: &BTreeMap<String, String>,
) -> usize {
    if guardados.is_empty() {
        return 0;
    }
    let mut tabla = Tabla {
        celdas: std::mem::take(celdas),
        ..Default::default()
    };
    let mut cambiadas = 0;
    // Seis vueltas, como el movil: cada una arregla un escalon de la cadena
    // de celdas que no cuadran.
    for _ in 0..6 {
        // Toda la hoja de una vez y con memoria: una columna de saldos de
        // diez mil filas no cabe en el tope de profundidad celda a celda.
        let valores = formula::evaluar_todo(&tabla);
        let mut malas: Vec<(String, Ref)> = Vec::new();
        for (dir, guardado) in guardados {
            let Some(crudo) = tabla.celdas.get(dir) else {
                continue;
            };
            if !Tabla::es_formula(crudo) {
                continue;
            }
            let Some(r) = ref_de(dir) else {
                continue;
            };
            let Some(v) = valores.get(dir) else {
                continue;
            };
            // Una volatil que aqui si se calcula da otra cosa y esta bien;
            // una que aqui no existe, error: esa si se queda con su valor.
            if es_volatil(crudo) && !matches!(v, Valor::Error(_)) {
                continue;
            }
            if !cuadra(v, guardado) {
                malas.push((dir.clone(), r));
            }
        }
        if malas.is_empty() {
            break;
        }
        let raices: Vec<usize> = if malas.len() > 2000 {
            (0..malas.len()).collect()
        } else {
            (0..malas.len())
                .filter(|&i| {
                    let crudo = tabla.celdas.get(&malas[i].0).map_or("", String::as_str);
                    !referencias(crudo).iter().any(|&(c1, f1, c2, f2)| {
                        malas.iter().any(|(_, m)| {
                            (c1..=c2).contains(&m.columna) && (f1..=f2).contains(&m.fila)
                        })
                    })
                })
                .collect()
        };
        let raices = if raices.is_empty() {
            (0..malas.len()).collect()
        } else {
            raices
        };
        for i in raices {
            let dir = &malas[i].0;
            let guardado = guardados.get(dir).cloned().unwrap_or_default();
            if guardado.is_empty() {
                tabla.celdas.remove(dir);
            } else {
                tabla.celdas.insert(dir.clone(), guardado);
            }
            cambiadas += 1;
        }
    }
    *celdas = tabla.celdas;
    cambiadas
}

/// Una referencia escrita en una formula: donde esta y que celda es.
pub(crate) struct RefEnTexto {
    pub(crate) desde: usize,
    pub(crate) hasta: usize,
    pub(crate) columna: u32,
    pub(crate) fila: u32,
    pub(crate) columna_fija: bool,
    pub(crate) fila_fija: bool,
}

/// Las referencias `A1`, `$A$1` de una formula (sin el `=`), en orden. Se
/// salta lo que va entre comillas y los nombres de funcion (`LOG10(`).
pub(crate) fn refs_en(cuerpo: &str) -> Vec<RefEnTexto> {
    let b = cuerpo.as_bytes();
    let mut salida = Vec::new();
    let mut i = 0;
    let es_palabra = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c == b'$';
    while i < b.len() {
        let c = b[i];
        if c == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                i += 1;
            }
            i += 1;
            continue;
        }
        if !(c == b'$' || c.is_ascii_alphabetic()) || (i > 0 && es_palabra(b[i - 1])) {
            i += 1;
            continue;
        }
        let desde = i;
        let mut j = i;
        let columna_fija = b[j] == b'$';
        if columna_fija {
            j += 1;
        }
        let l0 = j;
        while j < b.len() && b[j].is_ascii_alphabetic() {
            j += 1;
        }
        let letras = &cuerpo[l0..j];
        let fila_fija = j < b.len() && b[j] == b'$';
        if fila_fija {
            j += 1;
        }
        let n0 = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        let numeros = &cuerpo[n0..j];
        let sigue_palabra = j < b.len() && (es_palabra(b[j]) || b[j] == b'(');
        if (1..=3).contains(&letras.len())
            && (1..=6).contains(&numeros.len())
            && !sigue_palabra
            && let Some(r) = ref_de(&format!("{letras}{numeros}"))
        {
            salida.push(RefEnTexto {
                desde,
                hasta: j,
                columna: r.columna,
                fila: r.fila,
                columna_fija,
                fila_fija,
            });
            i = j;
            continue;
        }
        // No era una celda: se salta la palabra entera.
        i = j.max(i + 1);
        while i < b.len() && es_palabra(b[i]) {
            i += 1;
        }
    }
    salida
}

/// Los rangos que cita una formula (`CalculoLexico.referencias`): una celda
/// suelta es un rango de una.
fn referencias(formula: &str) -> Vec<Rango> {
    let Some(cuerpo) = formula.strip_prefix('=') else {
        return Vec::new();
    };
    let refs = refs_en(cuerpo);
    let mut salida = Vec::new();
    let mut k = 0;
    while k < refs.len() {
        let x = &refs[k];
        if let Some(y) = refs.get(k + 1)
            && cuerpo[x.hasta..y.desde].trim() == ":"
        {
            salida.push((
                x.columna.min(y.columna),
                x.fila.min(y.fila),
                x.columna.max(y.columna),
                x.fila.max(y.fila),
            ));
            k += 2;
            continue;
        }
        salida.push((x.columna, x.fila, x.columna, x.fila));
        k += 1;
    }
    salida
}

/// **La formula copiada `df` filas y `dc` columnas mas alla**
/// (`CalculoLexico.desplazar`): lo que no lleva `$` se mueve con ella, como
/// al copiar en Excel. Lo que se sale de la hoja queda en `#¡REF!`.
pub fn desplazar(formula: &str, df: i64, dc: i64) -> String {
    if df == 0 && dc == 0 {
        return formula.to_string();
    }
    let Some(cuerpo) = formula.strip_prefix('=') else {
        return formula.to_string();
    };
    let mut s = String::with_capacity(formula.len() + 8);
    s.push('=');
    let mut ultimo = 0;
    for x in refs_en(cuerpo) {
        let f = if x.fila_fija {
            x.fila as i64
        } else {
            x.fila as i64 + df
        };
        let c = if x.columna_fija {
            x.columna as i64
        } else {
            x.columna as i64 + dc
        };
        s.push_str(&cuerpo[ultimo..x.desde]);
        if f < 0 || f >= MAX_FILAS as i64 || c < 0 || c >= MAX_COLS as i64 {
            s.push_str("#¡REF!");
        } else {
            let dir = ref_a(Ref {
                columna: c as u32,
                fila: f as u32,
            });
            let (letras, numeros) = dir.split_at(
                dir.find(|ch: char| ch.is_ascii_digit())
                    .unwrap_or(dir.len()),
            );
            if x.columna_fija {
                s.push('$');
            }
            s.push_str(letras);
            if x.fila_fija {
                s.push('$');
            }
            s.push_str(numeros);
        }
        ultimo = x.hasta;
    }
    s.push_str(&cuerpo[ultimo..]);
    s
}

/// Los estilos que caen dentro de lo escrito: una fila entera pintada de
/// amarillo hasta la columna XFD no es parte de la tabla.
fn estilos_dentro(
    celdas: &BTreeMap<String, String>,
    estilos: BTreeMap<String, EstiloDeCelda>,
) -> BTreeMap<String, EstiloDeCelda> {
    let refs: Vec<Ref> = celdas.keys().filter_map(|k| ref_de(k)).collect();
    let (Some(c1), Some(c2), Some(f1), Some(f2)) = (
        refs.iter().map(|r| r.columna).min(),
        refs.iter().map(|r| r.columna).max(),
        refs.iter().map(|r| r.fila).min(),
        refs.iter().map(|r| r.fila).max(),
    ) else {
        return BTreeMap::new();
    };
    estilos
        .into_iter()
        .filter(|(d, _)| {
            ref_de(d).is_some_and(|r| (c1..=c2).contains(&r.columna) && (f1..=f2).contains(&r.fila))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// El XML, de corrido

/// Lo que va saliendo al recorrer un XML.
enum Ev<'a> {
    /// Una etiqueta que abre, por su nombre local (`x:sheet` es `sheet`), con
    /// sus atributos tambien por nombre local.
    Abre(&'a str, &'a [(&'a str, String)]),
    Cierra(&'a str),
    Texto(&'a str),
}

fn local(nombre: &str) -> &str {
    nombre.rsplit_once(':').map_or(nombre, |(_, l)| l)
}

fn attr<'a>(a: &'a [(&str, String)], nombre: &str) -> Option<&'a str> {
    a.iter()
        .find(|(k, _)| *k == nombre)
        .map(|(_, v)| v.as_str())
}

/// Las cinco entidades de siempre y las numericas. Las que declare el
/// documento no existen aqui: un libro que llega de fuera no puede pedir que
/// se lean otros ficheros (`disallow-doctype-decl` en el movil).
fn descodificar(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.contains('&') {
        return std::borrow::Cow::Borrowed(s);
    }
    let mut salida = String::with_capacity(s.len());
    let mut resto = s;
    while let Some(i) = resto.find('&') {
        salida.push_str(&resto[..i]);
        let tras = &resto[i..];
        let Some(fin) = tras[..tras.len().min(12)].find(';') else {
            salida.push('&');
            resto = &tras[1..];
            continue;
        };
        let nombre = &tras[1..fin];
        let c = match nombre {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if nombre.starts_with("#x") || nombre.starts_with("#X") => {
                u32::from_str_radix(&nombre[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if nombre.starts_with('#') => nombre[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                salida.push(c);
                resto = &tras[fin + 1..];
            }
            None => {
                salida.push('&');
                resto = &tras[1..];
            }
        }
    }
    salida.push_str(resto);
    std::borrow::Cow::Owned(salida)
}

/// **Recorre un XML de corrido**, sin montar el arbol. Una etiqueta que se
/// cierra sola (`<c r="A1"/>`) da su `Abre` y su `Cierra`, para que quien
/// escucha no tenga que distinguirlas. El `DOCTYPE`, los comentarios y las
/// instrucciones se saltan sin mirarlos.
fn sax(xml: &str, f: &mut dyn FnMut(Ev<'_>)) {
    let b = xml.as_bytes();
    let mut i = 0;
    let mut atributos: Vec<(&str, String)> = Vec::new();
    while i < b.len() {
        if b[i] != b'<' {
            let fin = xml[i..].find('<').map_or(b.len(), |k| i + k);
            let t = descodificar(&xml[i..fin]);
            f(Ev::Texto(&t));
            i = fin;
            continue;
        }
        let tras = &xml[i..];
        if tras.starts_with("<!--") {
            i = tras.find("-->").map_or(b.len(), |k| i + k + 3);
            continue;
        }
        if tras.starts_with("<![CDATA[") {
            let fin = tras.find("]]>").map_or(b.len(), |k| i + k);
            f(Ev::Texto(&xml[i + 9..fin.max(i + 9)]));
            i = (fin + 3).min(b.len());
            continue;
        }
        if tras.starts_with("<!") {
            // DOCTYPE con su `[...]`: fuera entero, ni se mira.
            let mut j = i + 2;
            let mut dentro = false;
            while j < b.len() {
                match b[j] {
                    b'[' => dentro = true,
                    b']' => dentro = false,
                    b'>' if !dentro => break,
                    _ => {}
                }
                j += 1;
            }
            i = j + 1;
            continue;
        }
        if tras.starts_with("<?") {
            i = tras.find("?>").map_or(b.len(), |k| i + k + 2);
            continue;
        }
        let fin = tras.find('>').map_or(b.len(), |k| i + k);
        let dentro = &xml[i + 1..fin.min(b.len())];
        i = fin + 1;
        if let Some(nombre) = dentro.strip_prefix('/') {
            f(Ev::Cierra(local(nombre.trim())));
            continue;
        }
        let suelta = dentro.ends_with('/');
        let dentro = dentro.trim_end_matches('/');
        let corte = dentro
            .find(|c: char| c.is_whitespace())
            .unwrap_or(dentro.len());
        let nombre = local(&dentro[..corte]);
        atributos.clear();
        let mut resto = &dentro[corte..];
        while let Some(igual) = resto.find('=') {
            let clave = local(resto[..igual].trim());
            let tras_igual = resto[igual + 1..].trim_start();
            let Some(comilla) = tras_igual
                .chars()
                .next()
                .filter(|c| *c == '"' || *c == '\'')
            else {
                break;
            };
            let Some(cierre) = tras_igual[1..].find(comilla) else {
                break;
            };
            atributos.push((clave, descodificar(&tras_igual[1..1 + cierre]).into_owned()));
            resto = &tras_igual[1 + cierre + 1..];
        }
        f(Ev::Abre(nombre, &atributos));
        if suelta {
            f(Ev::Cierra(nombre));
        }
    }
}

// ---------------------------------------------------------------------------
// XLSX

#[derive(Clone, Default)]
struct EstiloXlsx {
    negrita: bool,
    fondo: Option<String>,
    alineacion: Option<String>,
    tipo: Option<TipoNumero>,
}

fn de_xlsx(bytes: &[u8], ahora: i64) -> Result<Vec<HojaImportada>, NoSeLee> {
    let z = entradas(bytes)?;
    let libro = z.get("xl/workbook.xml").ok_or(NoSeLee::NoEsUnLibro)?;
    // (nombre, id de la relacion, oculta)
    let mut hojas: Vec<(String, String, bool)> = Vec::new();
    let mut fecha1904 = false;
    sax(libro, &mut |ev| {
        if let Ev::Abre(n, a) = ev {
            match n {
                "sheet" => hojas.push((
                    attr(a, "name").unwrap_or("Hoja").to_string(),
                    attr(a, "id").unwrap_or("").to_string(),
                    matches!(attr(a, "state"), Some("hidden" | "veryHidden")),
                )),
                "workbookPr" => fecha1904 = matches!(attr(a, "date1904"), Some("1" | "true")),
                _ => {}
            }
        }
    });
    let mut rels: HashMap<String, String> = HashMap::new();
    if let Some(r) = z.get("xl/_rels/workbook.xml.rels") {
        sax(r, &mut |ev| {
            if let Ev::Abre("Relationship", a) = ev
                && let (Some(id), Some(destino)) = (attr(a, "Id"), attr(a, "Target"))
            {
                rels.insert(id.to_string(), destino.to_string());
            }
        });
    }
    let compartidas = z
        .get("xl/sharedStrings.xml")
        .map(|t| compartidas_xlsx(t))
        .unwrap_or_default();
    let estilos = z
        .get("xl/styles.xml")
        .map(|t| estilos_xlsx(t))
        .unwrap_or_default();
    let mut salida = Vec::new();
    for (nombre, rid, oculta) in hojas {
        if oculta {
            continue;
        }
        let Some(destino) = rels.get(&rid) else {
            continue;
        };
        let ruta = match destino.strip_prefix('/') {
            Some(r) => r.to_string(),
            None => format!("xl/{destino}"),
        };
        let Some(xml) = z.get(&ruta) else {
            continue;
        };
        let h = hoja_xlsx(&nombre, xml, &compartidas, &estilos, fecha1904, ahora);
        if !h.tabla.celdas.is_empty() {
            salida.push(h);
        }
    }
    Ok(salida)
}

/// Los textos compartidos, sin la guia fonetica (`rPh`) del japones.
fn compartidas_xlsx(xml: &str) -> Vec<String> {
    let mut salida = Vec::new();
    let mut actual = String::new();
    let mut en_t = false;
    let mut fonetica = false;
    sax(xml, &mut |ev| match ev {
        Ev::Abre(n, _) => match n {
            "si" => actual.clear(),
            "rPh" => fonetica = true,
            "t" => en_t = !fonetica,
            _ => {}
        },
        Ev::Cierra(n) => match n {
            "t" => en_t = false,
            "rPh" => fonetica = false,
            "si" => salida.push(std::mem::take(&mut actual)),
            _ => {}
        },
        Ev::Texto(t) => {
            if en_t {
                actual.push_str(t);
            }
        }
    });
    salida
}

/// Que clase de numero pinta un formato: fecha, porcentaje o de los demas.
fn tipo_de_formato(id: u32, codigo: Option<&str>) -> TipoNumero {
    if id == 9 || id == 10 {
        return TipoNumero::Porcentaje;
    }
    if (14..=17).contains(&id) || id == 22 || (27..=36).contains(&id) || (50..=58).contains(&id) {
        return TipoNumero::Fecha;
    }
    let Some(codigo) = codigo else {
        return TipoNumero::General;
    };
    // Lo que va entre comillas o corchetes no cuenta: `0.00 "dias"` no es
    // una fecha por llevar una «d».
    let mut c = String::new();
    let mut fuera = None;
    for ch in codigo.chars() {
        match (fuera, ch) {
            (None, '"') => fuera = Some('"'),
            (None, '[') => fuera = Some(']'),
            (Some(cierra), x) if x == cierra => fuera = None,
            (None, x) => c.extend(x.to_lowercase()),
            _ => {}
        }
    }
    if c.contains('%') {
        return TipoNumero::Porcentaje;
    }
    if c.contains(['y', 'd'])
        || (c.contains('m') && !c.contains('h') && !c.contains('s') && !c.contains('0'))
    {
        TipoNumero::Fecha
    } else {
        TipoNumero::General
    }
}

fn estilos_xlsx(xml: &str) -> Vec<EstiloXlsx> {
    let mut formatos: HashMap<u32, String> = HashMap::new();
    let mut fuentes: Vec<bool> = Vec::new();
    let mut fondos: Vec<Option<String>> = Vec::new();
    let mut xfs: Vec<EstiloXlsx> = Vec::new();
    let mut seccion = String::new();
    let mut negrita = false;
    let mut patron: Option<String> = None;
    let mut color: Option<String> = None;
    let mut xf = (0u32, 0usize, 0usize, None::<String>);
    let secciones = ["numFmts", "fonts", "fills", "cellXfs", "cellStyleXfs"];
    sax(xml, &mut |ev| match ev {
        Ev::Abre(n, a) => match n {
            _ if secciones.contains(&n) => seccion = n.to_string(),
            "numFmt" => {
                if let Some(id) = attr(a, "numFmtId").and_then(|v| v.parse().ok()) {
                    formatos.insert(id, attr(a, "formatCode").unwrap_or("").to_string());
                }
            }
            "font" => negrita = false,
            "b" if seccion == "fonts" => {
                negrita = matches!(attr(a, "val"), None | Some("1" | "true"))
            }
            "fill" => {
                patron = None;
                color = None;
            }
            "patternFill" => patron = attr(a, "patternType").map(str::to_string),
            "fgColor" if seccion == "fills" => {
                color = attr(a, "rgb")
                    .filter(|r| r.len() >= 6)
                    .map(|r| format!("#{}", r[r.len() - 6..].to_ascii_lowercase()));
            }
            "xf" if seccion == "cellXfs" => {
                xf = (
                    attr(a, "numFmtId")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0),
                    attr(a, "fontId").and_then(|v| v.parse().ok()).unwrap_or(0),
                    attr(a, "fillId").and_then(|v| v.parse().ok()).unwrap_or(0),
                    None,
                );
            }
            "alignment" if seccion == "cellXfs" => {
                xf.3 = match attr(a, "horizontal") {
                    Some("center" | "centerContinuous") => Some("c".into()),
                    Some("right") => Some("d".into()),
                    Some("left") => Some("i".into()),
                    _ => None,
                };
            }
            _ => {}
        },
        Ev::Cierra(n) => match n {
            _ if secciones.contains(&n) => {
                if seccion == n {
                    seccion.clear();
                }
            }
            "font" if seccion == "fonts" => fuentes.push(negrita),
            "fill" if seccion == "fills" => fondos.push(if patron.as_deref() == Some("solid") {
                color.clone()
            } else {
                None
            }),
            "xf" if seccion == "cellXfs" => xfs.push(EstiloXlsx {
                negrita: fuentes.get(xf.1).copied().unwrap_or(false),
                fondo: fondos.get(xf.2).cloned().flatten(),
                alineacion: xf.3.clone(),
                tipo: Some(tipo_de_formato(
                    xf.0,
                    formatos.get(&xf.0).map(String::as_str),
                )),
            }),
            _ => {}
        },
        Ev::Texto(_) => {}
    });
    xfs
}

/// Quita los prefijos con que Excel guarda las funciones nuevas
/// (`_xlfn.XLOOKUP`): en la formula escrita no van.
fn sin_prefijos(f: &str) -> String {
    f.replace("_xlfn._xlws.", "")
        .replace("_xlfn.", "")
        .replace("_xlws.", "")
}

fn hoja_xlsx(
    nombre: &str,
    xml: &str,
    compartidas: &[String],
    estilos: &[EstiloXlsx],
    fecha1904: bool,
    ahora: i64,
) -> HojaImportada {
    let mut celdas: BTreeMap<String, String> = BTreeMap::new();
    let mut guardados: BTreeMap<String, String> = BTreeMap::new();
    let mut formatos: BTreeMap<String, EstiloDeCelda> = BTreeMap::new();
    let mut anchos: BTreeMap<String, u32> = BTreeMap::new();
    // La formula compartida `si`: su texto y en que celda se escribio.
    let mut base: HashMap<String, (String, u32, u32)> = HashMap::new();

    let mut fila: i64 = -1;
    let mut siguiente: u32 = 0;
    let (mut f, mut c) = (0u32, 0u32);
    let mut tipo: Option<String> = None;
    let mut estilo: Option<usize> = None;
    let mut f_tipo: Option<String> = None;
    let mut f_si: Option<String> = None;
    let mut formula = String::new();
    let mut valor = String::new();
    let mut en_linea = String::new();
    // 1 formula, 2 valor, 3 texto en linea
    let mut dentro = 0u8;

    sax(xml, &mut |ev| match ev {
        Ev::Abre(n, a) => match n {
            "row" => {
                fila = attr(a, "r")
                    .and_then(|v| v.parse::<i64>().ok())
                    .map_or(fila + 1, |r| r - 1);
                siguiente = 0;
            }
            "c" => {
                let r = attr(a, "r").and_then(ref_de);
                f = r.map_or(fila.max(0) as u32, |r| r.fila);
                c = r.map_or(siguiente, |r| r.columna);
                siguiente = c + 1;
                tipo = attr(a, "t").map(str::to_string);
                estilo = attr(a, "s").and_then(|v| v.parse().ok());
                f_tipo = None;
                f_si = None;
                formula.clear();
                valor.clear();
                en_linea.clear();
            }
            "f" => {
                dentro = 1;
                f_tipo = attr(a, "t").map(str::to_string);
                f_si = attr(a, "si").map(str::to_string);
            }
            "v" => dentro = 2,
            "t" if tipo.as_deref() == Some("inlineStr") => dentro = 3,
            "col" => {
                let Some(desde) = attr(a, "min").and_then(|v| v.parse::<u32>().ok()) else {
                    return;
                };
                let hasta = attr(a, "max")
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(desde)
                    .min(MAX_COLS)
                    .min(desde + 700);
                let Some(ancho) = attr(a, "width").and_then(|v| v.parse::<f64>().ok()) else {
                    return;
                };
                if attr(a, "hidden") == Some("1") {
                    return;
                }
                let px = ((ancho * 7.0 + 5.0).round() as i64)
                    .clamp(ANCHO_MIN as i64, ANCHO_MAX as i64) as u32;
                for col in desde.saturating_sub(1)..hasta {
                    let letras = ref_a(Ref {
                        columna: col,
                        fila: 0,
                    });
                    anchos.insert(letras.trim_end_matches('1').to_string(), px);
                }
            }
            _ => {}
        },
        Ev::Texto(t) => match dentro {
            1 => formula.push_str(t),
            2 => valor.push_str(t),
            3 => en_linea.push_str(t),
            _ => {}
        },
        Ev::Cierra(n) => match n {
            "f" | "v" | "t" => dentro = 0,
            "c" => {
                if f >= MAX_FILAS || c >= MAX_COLS {
                    return;
                }
                let e = estilo.and_then(|i| estilos.get(i));
                let mut escrita: Option<String> = None;
                if !formula.trim().is_empty() {
                    let texto = format!("={}", sin_prefijos(&formula));
                    if f_tipo.as_deref() == Some("shared")
                        && let Some(si) = &f_si
                    {
                        base.insert(si.clone(), (texto.clone(), f, c));
                    }
                    escrita = Some(texto);
                } else if f_tipo.as_deref() == Some("shared")
                    && let Some((b, f0, c0)) = f_si.as_ref().and_then(|si| base.get(si))
                {
                    escrita = Some(desplazar(b, f as i64 - *f0 as i64, c as i64 - *c0 as i64));
                }
                let literal = match tipo.as_deref() {
                    Some("s") => valor
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .and_then(|i| compartidas.get(i))
                        .and_then(|s| texto_como_literal(s)),
                    Some("inlineStr") => texto_como_literal(&en_linea),
                    Some("str") => texto_como_literal(&valor),
                    Some("b") => Some(
                        if valor.trim() == "1" {
                            "VERDADERO"
                        } else {
                            "FALSO"
                        }
                        .to_string(),
                    ),
                    Some("e") => (!valor.trim().is_empty()).then(|| valor.clone()),
                    _ => numero_como_literal(
                        &valor,
                        e.and_then(|e| e.tipo).unwrap_or(TipoNumero::General),
                        fecha1904,
                    ),
                };
                let dir = ref_a(Ref {
                    columna: c,
                    fila: f,
                });
                if let Some(fin) = escrita.clone().or_else(|| literal.clone())
                    && !fin.is_empty()
                {
                    celdas.insert(dir.clone(), fin);
                }
                if escrita.is_some() {
                    guardados.insert(dir.clone(), literal.unwrap_or_default());
                }
                if let Some(e) = e {
                    let propio = EstiloDeCelda {
                        n: e.negrita,
                        a: e.alineacion.clone(),
                        f: e.fondo.clone(),
                        ..Default::default()
                    };
                    if !propio.vacio() {
                        formatos.insert(dir, propio);
                    }
                }
            }
            _ => {}
        },
    });
    let como_valor = con_valores_donde_no_cuadra(&mut celdas, &guardados);
    let estilos = estilos_dentro(&celdas, formatos);
    HojaImportada {
        nombre: nombre.to_string(),
        tabla: Tabla {
            nombre: nombre.to_string(),
            celdas,
            anchos,
            estilos,
            tocado: ahora,
            ..Default::default()
        },
        formulas_como_valor: como_valor,
    }
}

// ---------------------------------------------------------------------------
// ODS

/// `of:=SUM([.A1:.B3])` a `=SUM(A1:B3)`. Las de otra hoja quedan en
/// `#REF!` y no cuadran: se quedaran con su valor.
fn formula_odf(f: &str) -> Option<String> {
    let i = f.find('=')?;
    let cuerpo = &f[i + 1..];
    let mut s = String::with_capacity(cuerpo.len());
    let mut resto = cuerpo;
    while let Some(a) = resto.find('[') {
        s.push_str(&resto[..a]);
        let Some(b) = resto[a..].find(']') else {
            s.push_str(&resto[a..]);
            resto = "";
            break;
        };
        let r = &resto[a + 1..a + b];
        if r.starts_with('.') || r.starts_with("$.") {
            s.push_str(
                &r.replace("$.", "")
                    .trim_start_matches('.')
                    .replace(":.", ":"),
            );
        } else {
            s.push_str("#REF!");
        }
        resto = &resto[a + b + 1..];
    }
    s.push_str(resto);
    Some(format!("={s}"))
}

fn de_ods(bytes: &[u8], ahora: i64) -> Result<Vec<HojaImportada>, NoSeLee> {
    let z = entradas(bytes)?;
    let contenido = z.get("content.xml").ok_or(NoSeLee::NoEsUnLibro)?;
    let mut salida = Vec::new();

    let mut nombre = String::new();
    let mut celdas: BTreeMap<String, String> = BTreeMap::new();
    let mut guardados: BTreeMap<String, String> = BTreeMap::new();
    let mut fila: u32 = 0;
    let mut col: u32 = 0;
    let mut repite_fila: u32 = 1;
    let mut de_la_fila: Vec<(u32, String)> = Vec::new();
    let mut en_celda = false;
    let mut repite_col: u32 = 1;
    let mut tipo: Option<String> = None;
    let mut valor: Option<String> = None;
    let mut fecha: Option<String> = None;
    let mut logico: Option<String> = None;
    let mut formula: Option<String> = None;
    let mut texto = String::new();
    let mut parrafos = 0;

    sax(contenido, &mut |ev| match ev {
        Ev::Abre(n, a) => match n {
            "table" => {
                nombre = attr(a, "name").unwrap_or("Hoja").to_string();
                celdas.clear();
                guardados.clear();
                fila = 0;
            }
            "table-row" => {
                repite_fila = attr(a, "number-rows-repeated")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1)
                    .max(1);
                col = 0;
                de_la_fila.clear();
            }
            "table-cell" | "covered-table-cell" => {
                en_celda = true;
                repite_col = attr(a, "number-columns-repeated")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1)
                    .max(1);
                tipo = attr(a, "value-type").map(str::to_string);
                valor = attr(a, "value").map(str::to_string);
                fecha = attr(a, "date-value").map(str::to_string);
                logico = attr(a, "boolean-value").map(str::to_string);
                formula = attr(a, "formula").map(str::to_string);
                texto.clear();
                parrafos = 0;
            }
            "p" if en_celda => {
                if parrafos > 0 {
                    texto.push('\n');
                }
                parrafos += 1;
            }
            "s" if en_celda => {
                let n: usize = attr(a, "c").and_then(|v| v.parse().ok()).unwrap_or(1);
                texto.extend(std::iter::repeat_n(' ', n.min(100)));
            }
            "line-break" if en_celda => texto.push('\n'),
            _ => {}
        },
        Ev::Texto(t) => {
            if en_celda {
                texto.push_str(t);
            }
        }
        Ev::Cierra(n) => match n {
            "table-cell" | "covered-table-cell" => {
                en_celda = false;
                let literal = match tipo.as_deref() {
                    Some("float" | "currency") => valor
                        .as_deref()
                        .and_then(|v| v.parse::<f64>().ok())
                        .map(limpio),
                    Some("percentage") => valor
                        .as_deref()
                        .and_then(|v| v.parse::<f64>().ok())
                        .map(|d| format!("{}%", formula::general(d * 100.0))),
                    Some("date") => fecha.as_deref().and_then(fecha_iso),
                    Some("boolean") => Some(
                        if logico.as_deref() == Some("true") {
                            "VERDADERO"
                        } else {
                            "FALSO"
                        }
                        .into(),
                    ),
                    None => None,
                    Some(_) => texto_como_literal(&texto),
                };
                let escrita = formula.as_deref().and_then(formula_odf);
                if let Some(fin) = escrita.clone().or_else(|| literal.clone())
                    && !fin.is_empty()
                    && fila < MAX_FILAS
                {
                    for k in 0..repite_col.min(1000) {
                        let c = col + k;
                        if c >= MAX_COLS {
                            break;
                        }
                        let dir = ref_a(Ref { columna: c, fila });
                        celdas.insert(dir.clone(), fin.clone());
                        if escrita.is_some() {
                            guardados.insert(dir, literal.clone().unwrap_or_default());
                        }
                        de_la_fila.push((c, fin.clone()));
                    }
                }
                col = col.saturating_add(repite_col);
            }
            "table-row" => {
                // Una fila repetida con algo dentro se copia; las vacias
                // solo avanzan (un millon de filas vacias al final de la
                // hoja es lo normal en un .ods).
                if !de_la_fila.is_empty() {
                    for k in 1..repite_fila.min(1000) {
                        if fila + k >= MAX_FILAS {
                            break;
                        }
                        for (c, v) in &de_la_fila {
                            celdas.insert(
                                ref_a(Ref {
                                    columna: *c,
                                    fila: fila + k,
                                }),
                                v.clone(),
                            );
                        }
                    }
                }
                fila = fila.saturating_add(repite_fila);
            }
            "table" if !celdas.is_empty() => {
                let mut propias = std::mem::take(&mut celdas);
                let como_valor = con_valores_donde_no_cuadra(&mut propias, &guardados);
                salida.push(HojaImportada {
                    nombre: nombre.clone(),
                    tabla: Tabla {
                        nombre: nombre.clone(),
                        celdas: propias,
                        tocado: ahora,
                        ..Default::default()
                    },
                    formulas_como_valor: como_valor,
                });
            }
            _ => {}
        },
    });
    Ok(salida)
}

/// `2026-09-10` (y lo que siga, la hora) a `10/09/2026`.
fn fecha_iso(s: &str) -> Option<String> {
    let s = s.get(..10)?;
    let mut partes = s.split('-');
    let a: i64 = partes.next()?.parse().ok()?;
    let m: u32 = partes.next()?.parse().ok()?;
    let d: u32 = partes.next()?.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then(|| format!("{d:02}/{m:02}/{a}"))
}

// ---------------------------------------------------------------------------
// CSV

/// **Un texto separado** (CSV, TSV o lo que se pega desde Excel) en filas
/// de celdas: `PortapapelesDeTabla.deSeparado` del movil. Las comillas solo
/// cuentan al empezar la celda y si la que cierra va seguida del separador o
/// del final; si no, son texto que empieza por comillas.
pub fn de_separado(texto: &str, separador: char) -> Vec<Vec<String>> {
    let cs: Vec<char> = texto.chars().collect();
    let n = cs.len();
    let mut filas = Vec::new();
    let mut fila = Vec::new();
    let mut celda = String::new();
    let mut i = 0;
    let mut al_empezar = true;
    while i < n {
        let c = cs[i];
        if al_empezar && c == '"' {
            let mut j = i + 1;
            let mut dentro = String::new();
            let mut cerrada = None;
            while j < n {
                if cs[j] == '"' {
                    if j + 1 < n && cs[j + 1] == '"' {
                        dentro.push('"');
                        j += 2;
                        continue;
                    }
                    cerrada = Some(j);
                    break;
                }
                dentro.push(cs[j]);
                j += 1;
            }
            if let Some(k) = cerrada {
                let tras = cs.get(k + 1).copied().unwrap_or('\n');
                if tras == separador || tras == '\n' || tras == '\r' {
                    celda.push_str(&dentro);
                    i = k + 1;
                    al_empezar = false;
                    continue;
                }
            }
        }
        al_empezar = false;
        if c == separador {
            fila.push(std::mem::take(&mut celda));
            al_empezar = true;
        } else if c == '\r' || c == '\n' {
            if c == '\r' && cs.get(i + 1) == Some(&'\n') {
                i += 1;
            }
            fila.push(std::mem::take(&mut celda));
            filas.push(std::mem::take(&mut fila));
            al_empezar = true;
        } else {
            celda.push(c);
        }
        i += 1;
    }
    if !celda.is_empty() || !fila.is_empty() {
        fila.push(celda);
        filas.push(fila);
    }
    filas
}

/// **Un CSV**: el separador es el que mas sale en la primera linea (`\t`,
/// `;` o `,`, en ese orden si empatan).
pub fn de_csv(texto: &str, nombre: &str, tabuladores: bool, ahora: i64) -> HojaImportada {
    let limpio = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    let primera = limpio.lines().next().unwrap_or("");
    let separador = if tabuladores {
        '\t'
    } else {
        let mut mejor = (',', 0);
        for s in ['\t', ';', ','] {
            let n = primera.matches(s).count();
            if n > mejor.1 {
                mejor = (s, n);
            }
        }
        mejor.0
    };
    let mut celdas = BTreeMap::new();
    for (f, fila) in de_separado(limpio, separador).into_iter().enumerate() {
        if f as u32 >= MAX_FILAS {
            break;
        }
        for (c, v) in fila.into_iter().enumerate() {
            if c as u32 >= MAX_COLS {
                break;
            }
            if !v.is_empty() {
                celdas.insert(
                    ref_a(Ref {
                        columna: c as u32,
                        fila: f as u32,
                    }),
                    v,
                );
            }
        }
    }
    HojaImportada {
        nombre: nombre.to_string(),
        tabla: Tabla {
            nombre: nombre.to_string(),
            celdas,
            tocado: ahora,
            ..Default::default()
        },
        formulas_como_valor: 0,
    }
}

#[cfg(test)]
mod pruebas;
