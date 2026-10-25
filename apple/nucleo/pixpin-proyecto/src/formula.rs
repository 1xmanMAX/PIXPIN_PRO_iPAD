//! El calculo de las formulas de las tablas.
//!
//! `tabla.rs` guarda el texto tal cual («=SUMA(B1:B6)») y no lo interpreta.
//! Aqui se interpreta, y sin tocar la tabla: todo lo de este fichero lee y
//! devuelve, nunca escribe. Asi una rejilla se puede repintar cuantas veces
//! haga falta sin miedo, y una formula rota no puede estropear el fichero.
//!
//! # Lo que mas importa: no colgarse
//!
//! Una hoja donde `A1` es `=B1` y `B1` es `=A1` es facil de escribir sin
//! querer, y una hoja de calculo que se queda pensando para siempre —o que
//! revienta la pila— es inservible. Por eso el evaluador lleva la lista de
//! celdas por las que va pasando: si vuelve a una que ya esta en el camino,
//! eso es un ciclo y responde [`ErrorFormula::Ciclo`] en vez de seguir. La
//! misma lista limita la profundidad, que una cadena de diez mil celdas
//! encadenadas desborda la pila igual que un ciclo aunque no lo sea.
//!
//! # Las dos decisiones que sorprenden
//!
//! **Las celdas vacias valen cero al sumar, pero no cuentan.** `=A1+1` con la
//! `A1` vacia es 1; `=PROMEDIO(A1:A3)` con dos vacias divide entre uno, no
//! entre tres, porque lo que el usuario quiere saber es la media de lo que
//! escribio, no de los huecos. Es lo mismo que hacen Excel y el PixPin del
//! movil.
//!
//! **El texto se ignora dentro de una funcion y rompe una cuenta suelta.**
//! `=SUMA(B1:B6)` sobre una columna con la cabecera «Importe» dentro suma los
//! numeros y calla: es lo que el usuario esperaba al arrastrar el rango. En
//! cambio `=B1*2` con «Importe» en la `B1` no significa nada, y dar cero ahi
//! seria mentir; da [`ErrorFormula::Sintaxis`].
//!
//! # El motor (11-oct-2026)
//!
//! Una formula se trocea, se arma su arbol (`analisis`) y se calcula
//! (`motor`). Calcular recorriendo el arbol, y no mientras se lee, es lo que
//! deja a `SI` callar la rama que no toca. Las funciones, cada una con su
//! nombre en espanol y en ingles (`SUMAR.SI` / `SUMIF`), estan por familias:
//! `mates` (logicas, matematicas, recuentos), `criterio` (`CONTAR.SI`...),
//! `busqueda` (`BUSCARV`...), `texto`, `fecha` y `info`; el catalogo de
//! nombres y de cuantos argumentos admite cada una esta en `motor::CATALOGO`.
//! La guia para portarlo a Android (semantica de los casos raros y tabla de
//! pruebas) es `docs/investigacion/2026-10-11-formulas-android.md`.
//!
//! - **Operadores**: `+ - * / ^ %`, `&`, `= <> < > <= >=`, el signo de delante
//!   (`-2^2` es 4, como Excel), texto entre comillas (`"a""b"`),
//!   `VERDADERO` / `FALSO`. Separador de argumentos: `;` o `,`.
//! - **Valores**: numero, texto, logico y error. Una celda vacia es el texto
//!   vacio (vale 0 al comparar con un numero, y "" con un texto).
//! - **Errores**: ademas de los de siempre, `#N/A` (lo buscado no esta) y
//!   `#¡NUM!` (un numero que no existe o no cabe). Un ciclo nunca se tapa:
//!   `SI.ERROR`, `ESERROR` y los recuentos lo dejan pasar.
//! - **Volatiles**: `HOY`, `AHORA`, `ALEATORIO` y `ALEATORIO.ENTRE` se
//!   calculan una vez por pasada, como cualquier otra celda.

use std::collections::BTreeMap;
use std::fmt;

#[cfg(test)]
use crate::tabla::ref_de;
use crate::tabla::{Ref, Tabla};

/// Lo que vale una celda una vez calculada.
#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Numero(f64),
    /// Tambien lo que vale una celda vacia, con la cadena vacia dentro.
    Texto(String),
    /// `VERDADERO` / `FALSO`, de una comparacion o de `Y`, `O`, `NO`.
    Logico(bool),
    Error(ErrorFormula),
}

/// Por que no se pudo calcular.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorFormula {
    /// La formula se refiere a si misma, por el camino que sea.
    Ciclo,
    /// Algo con forma de celda que no es ninguna celda («A0»).
    Referencia,
    /// No se entiende lo escrito, o se opera con texto.
    Sintaxis,
    DivisionPorCero,
    /// Una funcion que no existe.
    Nombre,
    /// `#N/A`: lo buscado no esta (`BUSCARV`, `COINCIDIR`...).
    NoDisponible,
    /// `#¡NUM!`: un numero que no existe (la raiz de un negativo) o que no cabe.
    Numero,
}

/// Cuantas celdas encadenadas se admiten antes de rendirse.
///
/// No es un capricho de tamano: cada salto es una llamada recursiva, y una
/// columna entera de `=A1`, `=A2`, `=A3`... desbordaria la pila. Rendirse es
/// mucho mejor que caerse, y a partir de aqui la hoja ya no es una hoja.
const PROFUNDIDAD_MAXIMA: usize = 128;

impl fmt::Display for ErrorFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Lo que se pinta en la celda. Corto y a gritos, como en cualquier
        // hoja de calculo: ocupa poco y se ve que algo va mal sin leerlo.
        let texto = match self {
            ErrorFormula::Ciclo => "#¡CICLO!",
            ErrorFormula::Referencia => "#¡REF!",
            ErrorFormula::Sintaxis => "#¡VALOR!",
            ErrorFormula::DivisionPorCero => "#¡DIV/0!",
            ErrorFormula::Nombre => "#¿NOMBRE?",
            ErrorFormula::NoDisponible => "#N/A",
            ErrorFormula::Numero => "#¡NUM!",
        };
        f.write_str(texto)
    }
}

impl fmt::Display for Valor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Como la vista General: «3» y no «3.0», «0.3» y no la cifra
            // diecisiete de 0.1+0.2. Ver `general`.
            Valor::Numero(n) => f.write_str(&general(*n)),
            Valor::Texto(t) => f.write_str(t),
            Valor::Logico(b) => f.write_str(if *b { "VERDADERO" } else { "FALSO" }),
            Valor::Error(e) => write!(f, "{e}"),
        }
    }
}

impl Valor {
    /// **Lo que se ensena en la celda**, con el separador decimal del
    /// usuario (`pixpin_sistema::entorno::separador_decimal`): en `es-ES` el
    /// total de `12,5` y `7` es `19,5`, no `19.5`. `Display` sigue con punto,
    /// que es lo que se guarda y se vuelve a leer sin dudas.
    pub fn mostrar(&self, decimal: char) -> String {
        match self {
            Valor::Numero(n) => general_con(*n, decimal),
            otro => otro.to_string(),
        }
    }
}

/// [`general`] con otro separador decimal.
pub fn general_con(d: f64, decimal: char) -> String {
    let s = general(d);
    if decimal == '.' {
        s
    } else {
        s.replace('.', &decimal.to_string())
    }
}

/// Lo que vale una celda: su contenido, o el resultado de su formula.
pub fn evaluar(tabla: &Tabla, celda: Ref) -> Valor {
    let mut m = Memoria::default();
    evaluar_camino(tabla, celda, &mut m)
}

/// Lo mismo para todas las celdas escritas, de una vez.
///
/// Solo las escritas: una tabla con una celda en la `A1` y otra en la `Z900`
/// devuelve dos entradas, no ochocientas mil. Las demas estan vacias y lo que
/// valen ya se sabe sin preguntar.
///
/// **Con memoria y por columnas**: cada formula se calcula una vez y se
/// guarda, y se recorren de arriba abajo y de izquierda a derecha. Una
/// columna de saldos (`=D1+C2`, `=D2+C3`...) de diez mil filas se calcula
/// entera: cada celda encuentra ya hecha la de encima, en vez de bajar diez
/// mil escalones (y pasar el tope de [`PROFUNDIDAD_MAXIMA`]) y repetir la
/// cuenta para cada fila, que era cuadratico. Importar un libro de Excel lo
/// necesita (ver `importar_hojas`).
pub fn evaluar_todo(tabla: &Tabla) -> BTreeMap<String, Valor> {
    let mut m = Memoria::default();
    // Cada celda se lee una vez por quien la cita, y buscarla por su nombre
    // en el mapa de la tabla (haciendo antes el nombre) es lo mas caro de
    // leerla: aqui se hace el indice por posicion de una vez.
    let mut crudas = MapaRapido::default();
    let mut refs: Vec<(Ref, &String)> = Vec::with_capacity(tabla.celdas.len());
    for (clave, contenido) in &tabla.celdas {
        // Una clave que no es una celda es basura de otra version; ensenarla
        // no se puede, pero tampoco vale la pena tirar el resto por ella.
        if let Some(r) = analisis::celda_de(clave) {
            crudas.insert(r, contenido.as_str());
            refs.push((r, clave));
        }
    }
    m.crudas = Some(crudas);
    refs.sort_by_key(|(r, _)| (r.columna, r.fila));
    let mut salida = BTreeMap::new();
    for (r, clave) in refs {
        salida.insert(clave.clone(), evaluar_camino(tabla, r, &mut m));
    }
    salida
}

/// Un mapa con una funcion de hash de una sola multiplicacion. Las claves son
/// pares de enteros que ponemos nosotros: el hash resistente a ataques de la
/// libreria estandar (SipHash) cuesta mas que lo que se busca, y se llama
/// varias veces por celda.
#[derive(Default, Clone, Copy)]
struct HashRapido(u64);

impl std::hash::Hasher for HashRapido {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.write_u32(*b as u32);
        }
    }
    fn write_u32(&mut self, n: u32) {
        self.0 = (self.0.rotate_left(5) ^ n as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

type MapaRapido<K, V> = std::collections::HashMap<K, V, std::hash::BuildHasherDefault<HashRapido>>;

/// Lo que se va sabiendo mientras se calcula: por donde se va (para cazar
/// los ciclos), lo ya calculado y las celdas escritas ordenadas por columna
/// (para recorrer un rango sin mirar la tabla entera).
#[derive(Default)]
struct Memoria<'t> {
    camino: Vec<Ref>,
    hechas: MapaRapido<Ref, Valor>,
    indice: Option<Vec<Ref>>,
    /// (columnas, filas) que llegan las celdas escritas; sale del indice.
    extension: Option<(u32, u32)>,
    /// El instante del calculo (ms desde 1970, hora local): `HOY` y `AHORA`
    /// dan lo mismo en toda la hoja aunque el calculo cruce la medianoche.
    ahora_ms: Option<i64>,
    semilla: Option<u64>,
    /// Los rangos ya leidos con sus valores (ver `Ctx::rala`).
    rangos: MapaRapido<motor::Rg, std::rc::Rc<Vec<(Ref, Valor)>>>,
    /// El contenido de cada celda escrita por su posicion (solo al calcular
    /// la tabla entera; para una celda suelta no compensa hacerlo).
    crudas: Option<MapaRapido<Ref, &'t str>>,
    /// Lo que dio ya cada `CONTAR.SI` / `SUMAR.SI`... con esos mismos rangos
    /// y criterios: una hoja los repite en cada fila con el mismo criterio.
    criterios: std::collections::HashMap<String, Valor>,
}

/// El valor de una celda sabiendo por donde se ha venido.
fn evaluar_camino<'t>(tabla: &'t Tabla, celda: Ref, m: &mut Memoria<'t>) -> Valor {
    if m.camino.contains(&celda) {
        return Valor::Error(ErrorFormula::Ciclo);
    }
    if let Some(v) = m.hechas.get(&celda) {
        return v.clone();
    }
    if m.camino.len() >= PROFUNDIDAD_MAXIMA {
        // No es un ciclo de verdad, pero se le parece en lo unico que le
        // importa al usuario: esa celda no se puede calcular y seguir
        // intentandolo tumbaria el programa.
        return Valor::Error(ErrorFormula::Ciclo);
    }

    let contenido = match &m.crudas {
        Some(c) => c.get(&celda).copied().unwrap_or(""),
        None => tabla.celda(celda),
    };
    if !Tabla::es_formula(contenido) {
        // Tambien se guarda: un rango de mil celdas sumado en mil filas lee
        // cada numero mil veces, y leer `1.234,50` no es gratis.
        let v = valor_escrito(contenido);
        m.hechas.insert(celda, v.clone());
        return v;
    }

    m.camino.push(celda);
    let valor = evaluar_formula(tabla, &contenido[1..], m);
    m.camino.pop();
    // Un «ciclo» depende de por donde se entro (el tope de profundidad
    // tambien lo da): no se guarda, que desde otra celda puede salir bien.
    if valor != Valor::Error(ErrorFormula::Ciclo) {
        m.hechas.insert(celda, valor.clone());
    }
    valor
}

/// Lo que vale lo escrito en una celda que no es una formula
/// (`Calculadora.literal` del movil): lo usa quien importa un libro para
/// comparar lo calculado aqui con lo que dijo Excel.
pub fn literal(contenido: &str) -> Valor {
    valor_escrito(contenido)
}

/// Lo que vale una celda que no es una formula.
fn valor_escrito(contenido: &str) -> Valor {
    // El apostrofo de delante es la marca de «texto tal cual» del movil y de
    // Excel (`ImportarHojas.textoComoLiteral`): un 007 que no es un 7, o un
    // «=hola» que no es formula. Se ensena sin el.
    if let Some(literal) = contenido.strip_prefix('\'') {
        return Valor::Texto(literal.to_string());
    }
    let limpio = contenido.trim();
    if limpio.is_empty() {
        return Valor::Texto(String::new());
    }
    match numero_escrito(limpio) {
        Some(n) => Valor::Numero(n),
        None => Valor::Texto(limpio.to_string()),
    }
}

/// **Un numero escrito a mano**, como lo escribio quien lo tecleo o lo pego
/// de una hoja en espanol o en ingles: `1.234,50`, `1,234.50`, `S/ 120`,
/// `15%`, `(300)`. `None` si no lo es. Es `CalculoFormato.numero` del movil,
/// regla a regla: con los dos separadores el ultimo es el decimal; con uno
/// solo repetido, de miles; una sola coma seguida de tres cifras tambien es
/// de miles (`1,500` son mil quinientos, como en Peru), y si no, decimal.
///
/// Solo cifras: «inf» y «NaN» (que `parse` si acepta) son texto, que un
/// infinito colado aqui contagiaria toda la columna.
pub fn numero_escrito(texto: &str) -> Option<f64> {
    // Los casos de todos los dias, sin pasar por el reconocedor entero:
    // enteros y decimales con punto («12», «-3.5»), y palabras corrientes
    // que se descartan por su primera letra. Dan lo mismo que el camino
    // largo; es solo que el 90 % de las celdas de una hoja son de estos.
    if let Some(n) = numero_corriente(texto) {
        return n;
    }
    let mut t = texto.trim();
    if t.is_empty() {
        return None;
    }
    let mut negativo = false;
    if t.len() > 2 && t.starts_with('(') && t.ends_with(')') {
        negativo = true;
        t = t[1..t.len() - 1].trim();
    }
    if let Some(r) = t.strip_prefix('-') {
        negativo = !negativo;
        t = r.trim();
    } else if let Some(r) = t.strip_prefix('+') {
        t = r.trim();
    }
    for moneda in ["S/.", "S/", "US$", "$", "€", "£"] {
        if let Some(r) = t.strip_prefix(moneda) {
            t = r.trim_start();
            break;
        }
    }
    let mut por_ciento = false;
    if let Some(r) = t.strip_suffix('%') {
        por_ciento = true;
        t = r.trim();
    }
    let mut s: String = t
        .chars()
        .filter(|c| !matches!(c, ' ' | '\u{a0}' | '\u{202f}'))
        .collect();
    if let Some(r) = s.strip_prefix('-') {
        negativo = !negativo;
        s = r.to_string();
    }
    // `^[0-9][0-9.,]*([eE][+-]?[0-9]+)?$|^[.,][0-9]+([eE][+-]?[0-9]+)?$`
    let (cuerpo, exponente) = match s.find(['e', 'E']) {
        Some(i) => (s[..i].to_string(), s[i..].to_string()),
        None => (s.clone(), String::new()),
    };
    let exp_bien = exponente.is_empty() || {
        let r = exponente[1..].trim_start_matches(['+', '-']);
        !r.is_empty()
            && r.chars().all(|c| c.is_ascii_digit())
            && exponente[1..].len() - r.len() <= 1
    };
    let mut cs = cuerpo.chars();
    let cifras_bien = match cs.next() {
        Some(c) if c.is_ascii_digit() => cuerpo
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == ','),
        Some('.' | ',') => {
            let resto: String = cs.collect();
            !resto.is_empty() && resto.chars().all(|c| c.is_ascii_digit())
        }
        _ => false,
    };
    if !cifras_bien || !exp_bien {
        return None;
    }
    let puntos = cuerpo.matches('.').count();
    let comas = cuerpo.matches(',').count();
    let mut cuerpo = if puntos > 0 && comas > 0 {
        if cuerpo.rfind('.') > cuerpo.rfind(',') {
            if puntos > 1 {
                return None;
            }
            cuerpo.replace(',', "")
        } else {
            if comas > 1 {
                return None;
            }
            cuerpo.replace('.', "").replace(',', ".")
        }
    } else if comas > 0 {
        if comas > 1 {
            cuerpo.replace(',', "")
        } else {
            let antes = cuerpo.find(',').unwrap_or(0);
            let despues = cuerpo.len() - antes - 1;
            if despues == 3 && (1..=3).contains(&antes) && !cuerpo.starts_with('0') {
                cuerpo.replace(',', "")
            } else {
                cuerpo.replace(',', ".")
            }
        }
    } else if puntos > 1 {
        cuerpo.replace('.', "")
    } else {
        cuerpo
    };
    if cuerpo.starts_with('.') {
        cuerpo.insert(0, '0');
    }
    if cuerpo.ends_with('.') {
        cuerpo.pop();
    }
    if cuerpo.is_empty() {
        return None;
    }
    let v: f64 = format!("{cuerpo}{exponente}").parse().ok()?;
    let mut r = if por_ciento { v / 100.0 } else { v };
    if negativo {
        r = -r;
    }
    r.is_finite().then_some(r)
}

/// `Some(resultado)` si `texto` es de los casos que se resuelven de un vistazo
/// (`None`: hay que mirarlo con calma).
fn numero_corriente(texto: &str) -> Option<Option<f64>> {
    let b = texto.as_bytes();
    let (&primero, _) = b.split_first()?;
    if primero.is_ascii_alphabetic() {
        // Letra de entrada: solo las monedas «S/» y «US$» (y los «inf»/«NaN»
        // que nunca fueron numeros) merecen la mirada larga; el resto es texto.
        return if matches!(primero, b'S' | b'U' | b's' | b'u') {
            None
        } else {
            Some(None)
        };
    }
    let cuerpo = if primero == b'-' { &b[1..] } else { b };
    let puntos = cuerpo.iter().filter(|c| **c == b'.').count();
    let solo_cifras = !cuerpo.is_empty()
        && cuerpo.len() <= 15
        && puntos <= 1
        && cuerpo.iter().all(|c| c.is_ascii_digit() || *c == b'.')
        && cuerpo[0].is_ascii_digit()
        && cuerpo[cuerpo.len() - 1].is_ascii_digit();
    if !solo_cifras {
        return None;
    }
    texto.parse::<f64>().ok().map(Some)
}

/// **Un numero como lo ensena la vista «General» de Excel** (y el movil,
/// `CalculoFormato.general`): diez cifras significativas sin ceros de cola,
/// los enteros enteros hasta 10^15 y en notacion cientifica lo que no cabe.
/// Asi `0.1+0.2` ensena `0.3`, y la tabla de la pantalla, la de la pagina
/// web y el CSV dicen lo mismo al digito.
pub fn general(d: f64) -> String {
    if !d.is_finite() {
        return "#¡NUM!".into();
    }
    if d == 0.0 {
        return "0".into();
    }
    let a = d.abs();
    let signo = if d < 0.0 { "-" } else { "" };
    if a < 1e15 && a == a.floor() {
        return format!("{signo}{}", a as i64);
    }
    if !(1e-9..1e15).contains(&a) {
        // Seis cifras y el exponente con dos digitos: `1.5E-12`.
        let s = format!("{a:.5e}");
        let (mantisa, exp) = s.split_once('e').unwrap_or((&s, "0"));
        let mantisa = if mantisa.contains('.') {
            mantisa.trim_end_matches('0').trim_end_matches('.')
        } else {
            mantisa
        };
        let e: i32 = exp.parse().unwrap_or(0);
        return format!(
            "{signo}{mantisa}E{}{:02}",
            if e < 0 { '-' } else { '+' },
            e.abs()
        );
    }
    let s = format!("{a:.9e}");
    let e: i32 = s
        .split_once('e')
        .and_then(|(_, e)| e.parse().ok())
        .unwrap_or(0);
    let decimales = (9 - e).max(0) as usize;
    let mut plano = format!("{a:.decimales$}");
    if plano.contains('.') {
        plano = plano
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();
    }
    format!("{signo}{plano}")
}

mod analisis;
mod busqueda;
mod criterio;
mod fecha;
mod formato;
mod info;
mod mates;
mod motor;
mod texto;

#[cfg(test)]
mod pruebas_funciones;

/// Calcula una formula ya sin el `=` de delante.
fn evaluar_formula<'t>(tabla: &'t Tabla, texto: &str, m: &mut Memoria<'t>) -> Valor {
    let arbol = match analisis::trocear(texto).and_then(analisis::analizar) {
        Ok(a) => a,
        Err(e) => return Valor::Error(e),
    };
    let mut ctx = motor::Ctx { tabla, m };
    ctx.valor(&arbol)
}

/// Las celdas escritas que caen dentro de un rango.
///
/// Se recorren las celdas que hay, no el rectangulo: `A1:ZZ9999` son
/// diecisiete millones de huecos y dos numeros, y recorrer los huecos colgaria
/// el programa por una formula que el usuario escribio con toda la razon.
fn celdas_del_rango(tabla: &Tabla, a: Ref, b: Ref, indice: &mut Option<Vec<Ref>>) -> Vec<Ref> {
    // Al reves (`B6:B1`) es el mismo rango: se arrastra el raton hacia arriba
    // tan a menudo como hacia abajo.
    let (c1, c2) = (a.columna.min(b.columna), a.columna.max(b.columna));
    let (f1, f2) = (a.fila.min(b.fila), a.fila.max(b.fila));
    // Las escritas, ordenadas por columna y fila una sola vez por calculo:
    // un rango se busca con dos saltos por columna en vez de mirar la
    // tabla entera por cada `SUMA` de cada fila.
    let indice = indice.get_or_insert_with(|| {
        let mut v: Vec<Ref> = tabla
            .celdas
            .keys()
            .filter_map(|clave| analisis::celda_de(clave))
            .collect();
        v.sort_by_key(|r| (r.columna, r.fila));
        v
    });
    let mut salida = Vec::new();
    let mut i = indice.partition_point(|r| (r.columna, r.fila) < (c1, f1));
    while let Some(r) = indice.get(i) {
        if r.columna > c2 {
            break;
        }
        if r.fila < f1 {
            i = indice.partition_point(|x| (x.columna, x.fila) < (r.columna, f1));
            continue;
        }
        if r.fila > f2 {
            // Lo que queda de esta columna cae fuera: a la siguiente.
            let siguiente = r.columna.saturating_add(1);
            if siguiente == r.columna {
                break;
            }
            i = indice.partition_point(|x| (x.columna, x.fila) < (siguiente, f1));
            continue;
        }
        salida.push(*r);
        i += 1;
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una tabla escrita de un tiron: `("B7", "=SUMA(B1:B6)")`.
    fn tabla(celdas: &[(&str, &str)]) -> Tabla {
        let mut t = Tabla::default();
        for (clave, contenido) in celdas {
            t.poner(ref_de(clave).unwrap(), contenido);
        }
        t
    }

    fn valor(t: &Tabla, celda: &str) -> Valor {
        evaluar(t, ref_de(celda).unwrap())
    }

    /// Lo que sale en una celda, para no escribir el `Valor::Numero` entero.
    fn numero(t: &Tabla, celda: &str) -> f64 {
        match valor(t, celda) {
            Valor::Numero(n) => n,
            otro => panic!("{celda} no dio un numero sino {otro:?}"),
        }
    }

    #[test]
    fn lo_escrito_a_mano_vale_por_si_mismo() {
        let t = tabla(&[("A1", "12"), ("A2", "-3.5"), ("A3", "Concepto"), ("A4", "")]);
        assert_eq!(valor(&t, "A1"), Valor::Numero(12.0));
        assert_eq!(valor(&t, "A2"), Valor::Numero(-3.5));
        assert_eq!(valor(&t, "A3"), Valor::Texto("Concepto".into()));
        // Y una celda en la que nunca se escribio nada esta vacia, no rota.
        assert_eq!(valor(&t, "Z90"), Valor::Texto(String::new()));
        // Caso negativo: «inf» y «NaN» se escriben con letras y son texto; un
        // infinito colado aqui contagiaria toda la columna.
        let t = tabla(&[("A1", "inf"), ("A2", "NaN")]);
        assert_eq!(valor(&t, "A1"), Valor::Texto("inf".into()));
        assert_eq!(valor(&t, "A2"), Valor::Texto("NaN".into()));
    }

    #[test]
    fn los_numeros_se_leen_como_los_escribe_una_hoja_en_espanol_o_en_ingles() {
        // `CalculoFormato.numero` del movil: con los dos separadores el
        // ultimo es el decimal; una coma seguida de tres cifras es de miles.
        for (texto, esperado) in [
            ("1.234,50", 1234.5),
            ("1,234.50", 1234.5),
            ("3,20", 3.2),
            ("1,500", 1500.0),
            ("0,500", 0.5),
            ("12,5", 12.5),
            ("15%", 0.15),
            ("(300)", -300.0),
            ("S/ 120", 120.0),
            ("-€ 4", -4.0),
            (".5", 0.5),
            ("1e3", 1000.0),
            ("2.000.000", 2_000_000.0),
        ] {
            assert_eq!(numero_escrito(texto), Some(esperado), "{texto}");
        }
        // Casos negativos: nada de esto es un numero.
        for malo in [
            "",
            "abc",
            "12a",
            "e5",
            "1e",
            "inf",
            "NaN",
            "--",
            "1.234,5,6",
        ] {
            assert_eq!(numero_escrito(malo), None, "{malo}");
        }
        let t = tabla(&[("B2", "1.234,50"), ("B3", "3,20"), ("B4", "=SUMA(B2:B3)")]);
        assert_eq!(valor(&t, "B4").to_string(), "1237.7");
    }

    #[test]
    fn los_numeros_se_ensenan_como_la_vista_general_de_excel() {
        // Diez cifras significativas y sin ceros de cola: 0.1+0.2 no ensena
        // la cifra diecisiete que el usuario nunca escribio.
        assert_eq!(general(0.1 + 0.2), "0.3");
        assert_eq!(general(1.0 / 3.0), "0.3333333333");
        assert_eq!(general(-2.5), "-2.5");
        assert_eq!(general(12.0), "12");
        assert_eq!(general(0.0), "0");
        assert_eq!(general(1e20), "1E+20");
        assert_eq!(general(1.5e-12), "1.5E-12");
        assert_eq!(general(f64::NAN), "#¡NUM!");
    }

    #[test]
    fn una_columna_de_saldos_de_mil_filas_se_calcula_entera_y_deprisa() {
        // Cada fila suma la de encima: mil escalones, mas que el tope de
        // profundidad. Con memoria, cada celda encuentra hecha la anterior.
        let mut t = Tabla::default();
        for f in 0..1000u32 {
            t.poner(
                Ref {
                    columna: 0,
                    fila: f,
                },
                "1",
            );
            let saldo = if f == 0 {
                "=A1".to_string()
            } else {
                format!("=B{}+A{}", f, f + 1)
            };
            t.poner(
                Ref {
                    columna: 1,
                    fila: f,
                },
                &saldo,
            );
            // Y un total que suma todo lo de encima, fila a fila.
            t.poner(
                Ref {
                    columna: 2,
                    fila: f,
                },
                &format!("=SUMA($A$1:A{})", f + 1),
            );
        }
        let t0 = std::time::Instant::now();
        let v = evaluar_todo(&t);
        assert_eq!(v["B1000"], Valor::Numero(1000.0));
        assert_eq!(v["C1000"], Valor::Numero(1000.0));
        assert!(t0.elapsed().as_secs() < 5, "tardo {:?}", t0.elapsed());
        // Caso negativo: un ciclo sigue siendo un ciclo aunque haya memoria.
        t.poner(ref_de("A1").unwrap(), "=B1000");
        assert_eq!(evaluar_todo(&t)["A1"], Valor::Error(ErrorFormula::Ciclo));
    }

    #[test]
    fn un_rango_encuentra_sus_celdas_por_columnas_sin_colarse_otras() {
        let t = tabla(&[
            ("A1", "1"),
            ("A5", "100"),
            ("B2", "10"),
            ("B3", "20"),
            ("C2", "1000"),
            ("D1", "=SUMA(A2:B4)"),
            ("D2", "=SUMA(B3:A2)"),
        ]);
        assert_eq!(numero(&t, "D1"), 30.0);
        assert_eq!(numero(&t, "D2"), 30.0, "al reves es el mismo rango");
    }

    #[test]
    fn el_resultado_se_ensena_con_la_coma_del_usuario_y_se_guarda_con_punto() {
        let t = tabla(&[
            ("B2", "12,5"),
            ("B3", "7"),
            ("B4", "=SUMA(B2:B3)"),
            ("B5", "=1/0"),
            ("B6", "=B4*100000000000000000000"),
        ]);
        let total = valor(&t, "B4");
        assert_eq!(total.mostrar(','), "19,5", "es-ES");
        assert_eq!(total.mostrar('.'), "19.5", "en-US");
        assert_eq!(total.to_string(), "19.5", "lo que se guarda no cambia");
        assert_eq!(valor(&t, "B6").mostrar(','), "1,95E+21");
        // Casos negativos: los errores y los enteros no llevan separador.
        assert_eq!(valor(&t, "B5").mostrar(','), "#¡DIV/0!");
        assert_eq!(valor(&t, "B3").mostrar(','), "7");
        // Y lo ensenado se vuelve a leer igual.
        assert_eq!(numero_escrito(&total.mostrar(',')), Some(19.5));
    }

    #[test]
    fn un_apostrofo_delante_hace_texto_lo_que_pareceria_numero_o_formula() {
        // Es la marca del movil (y de Excel) para «esto es texto tal cual»:
        // el codigo postal 007 no es un 7, ni «=hola» una formula. Se ensena
        // sin el apostrofo.
        let t = tabla(&[
            ("A1", "'007"),
            ("A2", "'=hola"),
            ("A3", "=SUMA(A1:A2)"),
            ("A4", "'"),
        ]);
        assert_eq!(valor(&t, "A1"), Valor::Texto("007".into()));
        assert_eq!(valor(&t, "A2"), Valor::Texto("=hola".into()));
        // Y como texto no suma.
        assert_eq!(numero(&t, "A3"), 0.0);
        // Caso negativo: un apostrofo solo es una celda vacia de verdad.
        assert_eq!(valor(&t, "A4"), Valor::Texto(String::new()));
    }

    #[test]
    fn las_referencias_con_dolar_de_excel_valen_igual() {
        // `$A$1` fija la celda al copiar la formula; al calcular es la A1. Un
        // libro de Excel las trae a cada paso, y sin esto cada una daba error.
        let t = tabla(&[
            ("A1", "2"),
            ("A2", "3"),
            ("B1", "=$A$1*A$2+$A2"),
            ("B2", "=SUMA($A$1:$A2)"),
        ]);
        assert_eq!(numero(&t, "B1"), 9.0);
        assert_eq!(numero(&t, "B2"), 5.0);
        // Caso negativo: un dolar suelto no es nada.
        let t = tabla(&[("A1", "=$+1"), ("A2", "=A$")]);
        assert!(matches!(valor(&t, "A1"), Valor::Error(_)));
        assert!(matches!(valor(&t, "A2"), Valor::Error(_)));
    }

    #[test]
    fn las_cuentas_con_numeros_y_celdas_salen() {
        let t = tabla(&[
            ("A1", "10"),
            ("A2", "4"),
            ("B1", "=A1+A2"),
            ("B2", "=A1-A2*2"),
            ("B3", "=-A2"),
            ("B4", "=A1/A2"),
            ("B5", "=2.5+1.5"),
        ]);
        assert_eq!(numero(&t, "B1"), 14.0);
        assert_eq!(numero(&t, "B2"), 2.0);
        assert_eq!(numero(&t, "B3"), -4.0);
        assert_eq!(numero(&t, "B4"), 2.5);
        assert_eq!(numero(&t, "B5"), 4.0);
    }

    #[test]
    fn multiplicar_va_antes_que_sumar_y_los_parentesis_antes_que_todo() {
        // El fallo clasico de un evaluador escrito de izquierda a derecha es
        // que esto de 20.
        let t = tabla(&[("A1", "=2+3*4"), ("A2", "=(2+3)*4"), ("A3", "=2-3-4")]);
        assert_eq!(numero(&t, "A1"), 14.0);
        assert_eq!(numero(&t, "A2"), 20.0);
        // Y las restas van de izquierda a derecha: -5, no 3.
        assert_eq!(numero(&t, "A3"), -5.0);
    }

    #[test]
    fn un_ciclo_directo_no_cuelga() {
        // El requisito duro. Si esto se cuelga, la tabla es inservible.
        let t = tabla(&[("A1", "=B1"), ("B1", "=A1")]);
        assert_eq!(valor(&t, "A1"), Valor::Error(ErrorFormula::Ciclo));
        assert_eq!(valor(&t, "B1"), Valor::Error(ErrorFormula::Ciclo));
    }

    #[test]
    fn un_ciclo_de_tres_celdas_tampoco() {
        let t = tabla(&[("A1", "=B1+1"), ("B1", "=C1+1"), ("C1", "=A1+1")]);
        for celda in ["A1", "B1", "C1"] {
            assert_eq!(
                valor(&t, celda),
                Valor::Error(ErrorFormula::Ciclo),
                "{celda}"
            );
        }
        // Y el ciclo no envenena a quien no esta dentro: una celda sana que
        // mire a otra sana sigue calculando.
        let t = tabla(&[("A1", "=B1"), ("B1", "=A1"), ("C1", "5"), ("D1", "=C1*2")]);
        assert_eq!(numero(&t, "D1"), 10.0);
    }

    #[test]
    fn una_cadena_larguisima_de_celdas_no_desborda_la_pila() {
        // No es un ciclo, pero cada salto es una llamada recursiva: sin el
        // limite de profundidad esto tumbaria el programa entero.
        let mut t = Tabla::default();
        t.poner(ref_de("A1").unwrap(), "1");
        for fila in 2..=2000 {
            t.poner(
                ref_de(&format!("A{fila}")).unwrap(),
                &format!("=A{}+1", fila - 1),
            );
        }
        assert_eq!(valor(&t, "A2000"), Valor::Error(ErrorFormula::Ciclo));
        // Lo que queda dentro del limite si sale bien.
        assert_eq!(numero(&t, "A10"), 10.0);
    }

    #[test]
    fn un_ciclo_dentro_de_un_rango_tampoco_cuelga() {
        // El ciclo entra por la funcion, no por el operador: es otro camino
        // hacia el mismo desastre.
        let t = tabla(&[("A1", "=SUMA(A1:A3)"), ("A2", "2"), ("A3", "3")]);
        assert_eq!(valor(&t, "A1"), Valor::Error(ErrorFormula::Ciclo));
    }

    #[test]
    fn las_funciones_suman_promedian_y_cuentan() {
        let t = tabla(&[
            ("B1", "10"),
            ("B2", "20"),
            ("B3", "30"),
            ("C1", "=SUMA(B1:B3)"),
            ("C2", "=PROMEDIO(B1:B3)"),
            ("C3", "=MIN(B1:B3)"),
            ("C4", "=MAX(B1:B3)"),
            ("C5", "=CONTAR(B1:B3)"),
            ("C6", "=SUMA(B1:B3; 40)"),
            ("C7", "=SUMA(B1, B3)"),
            // Al reves es el mismo rango: se arrastra el raton hacia arriba
            // tan a menudo como hacia abajo.
            ("C8", "=SUMA(B3:B1)"),
        ]);
        assert_eq!(numero(&t, "C1"), 60.0);
        assert_eq!(numero(&t, "C2"), 20.0);
        assert_eq!(numero(&t, "C3"), 10.0);
        assert_eq!(numero(&t, "C4"), 30.0);
        assert_eq!(numero(&t, "C5"), 3.0);
        assert_eq!(numero(&t, "C6"), 100.0);
        assert_eq!(numero(&t, "C7"), 40.0);
        assert_eq!(numero(&t, "C8"), 60.0);
    }

    #[test]
    fn el_maximo_de_una_columna_de_negativos_no_es_cero() {
        // Caso negativo en los dos sentidos: partir de cero al plegar daria 0,
        // un numero que no escribio nadie.
        let t = tabla(&[
            ("A1", "-5"),
            ("A2", "-3"),
            ("B1", "=MAX(A1:A2)"),
            ("B2", "=MIN(A1:A2)"),
            // Sin nada escrito si valen cero: es una columna a medio rellenar.
            ("B3", "=MAX(C1:C9)"),
        ]);
        assert_eq!(numero(&t, "B1"), -3.0);
        assert_eq!(numero(&t, "B2"), -5.0);
        assert_eq!(numero(&t, "B3"), 0.0);
    }

    #[test]
    fn los_nombres_ingleses_y_las_minusculas_valen_igual() {
        let t = tabla(&[
            ("B1", "10"),
            ("B2", "20"),
            ("C1", "=SUM(B1:B2)"),
            ("C2", "=average(b1:b2)"),
            ("C3", "=Count(B1:B2)"),
        ]);
        assert_eq!(numero(&t, "C1"), 30.0);
        assert_eq!(numero(&t, "C2"), 15.0);
        assert_eq!(numero(&t, "C3"), 2.0);
    }

    #[test]
    fn una_celda_vacia_vale_cero_al_sumar() {
        let t = tabla(&[("A1", "7"), ("B1", "=A1+A2"), ("B2", "=A2*3")]);
        assert_eq!(numero(&t, "B1"), 7.0);
        assert_eq!(numero(&t, "B2"), 0.0);
    }

    #[test]
    fn una_celda_vacia_no_cuenta_en_el_promedio() {
        // Aqui es donde mas se falla: promediar tres celdas de las que dos
        // estan vacias es dividir entre uno, no entre tres.
        let t = tabla(&[("A1", "9"), ("B1", "=PROMEDIO(A1:A3)")]);
        assert_eq!(numero(&t, "B1"), 9.0);
        let t = tabla(&[("A1", "9"), ("A3", "3"), ("B1", "=PROMEDIO(A1:A3)")]);
        assert_eq!(numero(&t, "B1"), 6.0);
    }

    #[test]
    fn una_celda_vacia_no_cuenta_en_contar() {
        let t = tabla(&[("A1", "1"), ("A4", "2"), ("B1", "=CONTAR(A1:A6)")]);
        assert_eq!(numero(&t, "B1"), 2.0);
        // Caso negativo: contar un rango sin nada escrito da cero, no seis.
        let t = tabla(&[("B1", "=CONTAR(A1:A6)")]);
        assert_eq!(numero(&t, "B1"), 0.0);
    }

    #[test]
    fn promediar_donde_no_hay_nada_no_divide_entre_cero() {
        // Decir cero seria inventarse un dato: no hay nada que promediar.
        let t = tabla(&[("B1", "=PROMEDIO(A1:A6)")]);
        assert_eq!(valor(&t, "B1"), Valor::Error(ErrorFormula::DivisionPorCero));
    }

    #[test]
    fn el_texto_se_ignora_en_una_funcion_y_rompe_una_cuenta_suelta() {
        // Arrastrar el rango incluyendo la cabecera es lo normal, y ahi lo que
        // el usuario quiere es la suma de los numeros.
        let t = tabla(&[
            ("B1", "Importe"),
            ("B2", "10"),
            ("B3", "20"),
            ("C1", "=SUMA(B1:B3)"),
            ("C2", "=CONTAR(B1:B3)"),
            ("C3", "=PROMEDIO(B1:B3)"),
            // En cambio esto no significa nada, y dar 0 seria mentir con cara
            // de resultado.
            ("C4", "=B1*2"),
            ("C5", "=B1+B2"),
        ]);
        assert_eq!(numero(&t, "C1"), 30.0);
        assert_eq!(numero(&t, "C2"), 2.0, "el texto no cuenta");
        assert_eq!(numero(&t, "C3"), 15.0, "ni divide");
        assert_eq!(valor(&t, "C4"), Valor::Error(ErrorFormula::Sintaxis));
        assert_eq!(valor(&t, "C5"), Valor::Error(ErrorFormula::Sintaxis));
    }

    #[test]
    fn dividir_entre_cero_da_error_y_no_infinito() {
        let t = tabla(&[("A1", "5"), ("A2", "0"), ("B1", "=A1/A2"), ("B2", "=A1/A9")]);
        assert_eq!(valor(&t, "B1"), Valor::Error(ErrorFormula::DivisionPorCero));
        // Y dividir entre una celda vacia es dividir entre cero, no entre uno.
        assert_eq!(valor(&t, "B2"), Valor::Error(ErrorFormula::DivisionPorCero));
    }

    #[test]
    fn un_error_no_se_disimula_por_el_camino() {
        // Una suma que se salta la celda rota daria un total creible y falso,
        // que es lo peor que puede pasar en una hoja de gastos.
        let t = tabla(&[
            ("A1", "10"),
            ("A2", "=1/0"),
            ("A3", "=SUMA(A1:A2)"),
            ("A4", "=A2+1"),
        ]);
        assert_eq!(valor(&t, "A3"), Valor::Error(ErrorFormula::DivisionPorCero));
        assert_eq!(valor(&t, "A4"), Valor::Error(ErrorFormula::DivisionPorCero));
    }

    #[test]
    fn una_funcion_que_no_existe_se_dice_por_su_nombre() {
        let t = tabla(&[("A1", "=NOEXISTE(A3)"), ("A2", "=SUMA(A3:A4")]);
        assert_eq!(valor(&t, "A1"), Valor::Error(ErrorFormula::Nombre));
        // Y un parentesis sin cerrar no es un nombre malo, es sintaxis.
        assert_eq!(valor(&t, "A2"), Valor::Error(ErrorFormula::Sintaxis));
    }

    #[test]
    fn lo_que_parece_una_celda_y_no_lo_es_da_error_de_referencia() {
        // Las hojas empiezan en la fila 1: la A0 no existe.
        let t = tabla(&[("B1", "=A0+1"), ("B2", "=SUMA(A0:A3)")]);
        assert_eq!(valor(&t, "B1"), Valor::Error(ErrorFormula::Referencia));
        assert_eq!(valor(&t, "B2"), Valor::Error(ErrorFormula::Referencia));
    }

    #[test]
    fn una_formula_a_medias_no_se_calcula_a_medias() {
        // Casos negativos: quedarse con el 1 de «=1 2» callando seria peor que
        // decir que no se entiende.
        for mala in [
            "=",
            "=1+",
            "=1 2",
            "=*3",
            "=(1+2",
            "=1)",
            "=1,,2",
            "=B1:B6*2",
            "=1.2.3+1",
            "=#",
            "=SUMA",
            "=SUMA(1;)",
        ] {
            let t = tabla(&[("A1", mala)]);
            assert!(
                matches!(valor(&t, "A1"), Valor::Error(_)),
                "«{mala}» tendria que ser un error"
            );
        }
    }

    #[test]
    fn evaluar_todo_da_una_celda_por_cada_una_escrita() {
        let t = tabla(&[
            ("A1", "Gasto"),
            ("B1", "10"),
            ("B2", "20"),
            ("B3", "=SUMA(B1:B2)"),
        ]);
        let todo = evaluar_todo(&t);
        assert_eq!(todo.len(), 4, "ni una de mas ni una de menos");
        assert_eq!(todo["B3"], Valor::Numero(30.0));
        assert_eq!(todo["A1"], Valor::Texto("Gasto".into()));
        // Las celdas vacias no aparecen: una tabla con una celda en la A1 y
        // otra en la Z900 son dos entradas, no ochocientas mil.
        assert!(!todo.contains_key("A2"));
    }

    #[test]
    fn un_rango_enorme_se_calcula_al_momento() {
        // Se recorren las celdas escritas, no el rectangulo: recorrer los
        // diecisiete millones de huecos colgaria el programa por una formula
        // que el usuario escribio con toda la razon.
        // La formula se queda fuera del rango a proposito: dentro seria un
        // ciclo de verdad, que es otra prueba.
        let t = tabla(&[("A1", "2"), ("ZY9999", "3"), ("ZZ1", "=SUMA(A1:ZY9999)")]);
        assert_eq!(numero(&t, "ZZ1"), 5.0);
    }

    #[test]
    fn los_errores_se_pintan_cortos_y_a_gritos() {
        assert_eq!(Valor::Numero(3.0).to_string(), "3", "sin coma de mas");
        assert_eq!(Valor::Numero(2.5).to_string(), "2.5");
        assert_eq!(Valor::Error(ErrorFormula::Ciclo).to_string(), "#¡CICLO!");
        assert_eq!(Valor::Texto(String::new()).to_string(), "");
    }
}
