//! **El horario de clases que llama por telefono** (17-oct-2026).
//!
//! El usuario: «en recordatorios poder ingresar de forma rapida e intuitiva
//! un horario o calendario de actividades, de forma que aparezcan en
//! pantalla como si se tratara de publicidad: el nombre de la asignatura y
//! su horario, y un mensaje de voz si lo activo, predefinido segun lo que se
//! tenia que hacer para esa clase, que grabaria durante la clase pasada; y
//! apareceria como una llamada telefonica, de forma que si o si me haga
//! recordar cosas importantes como evaluaciones con anticipacion».
//!
//! ## Meterlo: una nota de texto, una linea por clase
//!
//! El horario es `horario.md` junto a los datos, y se escribe con el editor
//! de notas de siempre (`notas_md`, el mismo motor): nada de formularios.
//!
//! ```text
//! Lun Mie 8:00-10:00 Geometría @A-12
//! > traer el compás                      ← lo que lee la voz al contestar
//! Mar Jue 10:15-12:00 Física
//! Evaluación Geometría 23/10             ← avisa 3 dias y 1 dia antes
//! avisar 15 min antes                    ← la antelacion (10 si no se dice)
//! ```
//!
//! Tambien vale **pegar una tabla** (de Excel, de la web o de Markdown): la
//! de filas («Lunes | 8:00-10:00 | Geometría | A-12») y la cuadricula de
//! los colegios (la primera fila son los dias y la primera columna las
//! horas).
//!
//! ## El aviso: la tarjeta de la llamada secreta
//!
//! Antes de cada clase sale la tarjeta de «llamada entrante» de
//! `llamada.rs` (el mismo motor, con `detalle` para la hora y el aula y en
//! el centro de la pantalla), con el tono de llamada. **Contestar** pone la
//! nota de voz de la clase (grabada con el microfono flotante del chat,
//! «Nota de voz para la próxima clase» en la bandeja) y despues la voz de
//! la app lee lo apuntado con `>`. **Volver a llamar en** la pospone.
//!
//! Los avisos van al mismo vigia que los recordatorios (`recordatorios.rs`):
//! se calculan del texto cada vez que se relee la agenda (los proximos
//! [`DIAS_A_LA_VISTA`] dias) y un hilo mira cada pocos segundos si el
//! horario cambio o cambio el dia, y entonces la relee.
//!
//! Android no tiene horario; es solo de este equipo y no viaja.
//!
//! En el Mac (18-oct): la misma lectura y los mismos avisos; el vigia y la
//! tarjeta de la llamada son los de Swift. La ventana semanal y las
//! asignaturas como listas aun no.
//!
//! ## La ventana semanal (18-oct-2026)
//!
//! El usuario: «una interfaz estilo calendario semanal en la que pueda
//! ingresar mi horario: en el lado lateral mis asignaturas, y jalarlas sobre
//! el calendario y soltarlas; que cada asignatura sea un grupo de tareas».
//! «Horario de clases…» abre ahora [`ventana`]: la lista de asignaturas a
//! la izquierda y la rejilla de la semana a la derecha. Lo que se hace alli
//! se escribe en el MISMO `horario.md` ([`semana`] reescribe solo las
//! lineas de clases y de asignaturas: los titulos, los comentarios, las
//! evaluaciones y el «avisar» se quedan donde estaban), asi que se sigue
//! pudiendo escribir a mano y los avisos salen de lo mismo. Dos cosas nuevas
//! en el texto, las dos legibles:
//!
//! ```text
//! Asignatura Geometría @A-12 #4F8EF7       ← la lista lateral (con su color)
//! Lun 8:00-9:00 Geometría @A-12 avisar 15  ← esta clase avisa 15 min antes
//! ```
//!
//! Cada asignatura es una lista de tareas en el chat «Asignaturas»
//! ([`asignaturas`]).

#![forbid(unsafe_code)]


use std::path::{Path, PathBuf};

use pixpin_ia::acciones::normalizar;
use pixpin_ia::fechas::{self, Momento};

use crate::Nucleo;

// Lo puro de la ventana semanal (Windows, 18-oct).
pub mod asignaturas;
pub(crate) mod semana;
pub mod ventana_mac;


/// La llamada de un aviso (la de `llamada.rs` de Windows, solo lo que se
/// usa aqui).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Llamada {
    pub audio: Option<PathBuf>,
    pub nombre: String,
    pub carpeta: PathBuf,
    pub mensaje: String,
    pub detalle: String,
    pub texto: Option<String>,
}

impl Llamada {
    /// Si es un aviso del horario (y no la nota de un chat).
    pub fn es_de_horario(&self) -> bool {
        self.mensaje.starts_with(PREFIJO)
    }
}


/// Lo que va delante del id de todo aviso del horario en la agenda.
pub const PREFIJO: &str = "horario:";

/// El fichero del horario, junto a los datos.
pub const FICHERO: &str = "horario.md";

/// Los minutos de antelacion si el horario no dice otra cosa.
pub const ANTELACION: u32 = 10;

/// Los dias antes de una evaluacion en que se avisa.
pub const DIAS_ANTES: [u32; 2] = [3, 1];

/// Cuantos dias por delante se ponen en la agenda.
pub const DIAS_A_LA_VISTA: i64 = 8;

const MINUTO: i64 = 60_000;
const DIA: i64 = 86_400_000;

/// Una clase de la semana.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clase {
    /// Los dias de la semana (0 lunes … 6 domingo), sin repetir y en orden.
    pub dias: Vec<u8>,
    /// Minuto del dia en que empieza y en que acaba.
    pub inicio: u32,
    pub fin: u32,
    pub nombre: String,
    pub aula: Option<String>,
    /// Lo apuntado con `>` debajo: lo que lee la voz al contestar.
    pub notas: Vec<String>,
    /// Los minutos de antes en que avisa esta clase (`avisar 15` en su
    /// linea); `None` es la antelacion de todo el horario.
    pub antelacion: Option<u32>,
}

/// Una asignatura de la lista lateral de la ventana (`Asignatura Geometría
/// @A-12 #4F8EF7`). Las clases que nombran una que no esta en la lista
/// tambien cuentan ([`Horario::todas_las_asignaturas`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asignatura {
    pub nombre: String,
    pub aula: Option<String>,
    /// Su color, `0xRRGGBB`; `None` es el de su nombre.
    pub color: Option<u32>,
}

/// Una evaluacion (examen, entrega…) con su fecha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluacion {
    /// La linea tal cual, sin la fecha: «Evaluación Geometría».
    pub que: String,
    /// La clase que nombra, si nombra alguna (su sitio en `clases`).
    pub clase: Option<usize>,
    pub cuando: Momento,
}

/// El horario entero, ya leido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Horario {
    pub clases: Vec<Clase>,
    pub evaluaciones: Vec<Evaluacion>,
    /// Minutos antes de cada clase.
    pub antelacion: u32,
    /// Dias antes de cada evaluacion.
    pub dias_antes: Vec<u32>,
    /// La lista lateral, en el orden escrito.
    pub asignaturas: Vec<Asignatura>,
}

impl Horario {
    /// Las asignaturas de la lista lateral y, detras, las de las clases que
    /// no estan en ella (escritas a mano o pegadas en una tabla), sin
    /// repetir (sin mirar mayusculas ni tildes).
    pub fn todas_las_asignaturas(&self) -> Vec<Asignatura> {
        let mut v = self.asignaturas.clone();
        for c in &self.clases {
            let n = normalizar(&c.nombre);
            if !v.iter().any(|a| normalizar(&a.nombre) == n) {
                v.push(Asignatura {
                    nombre: c.nombre.clone(),
                    aula: c.aula.clone(),
                    color: None,
                });
            }
        }
        v
    }
}

impl Default for Horario {
    fn default() -> Self {
        Horario {
            clases: Vec::new(),
            evaluaciones: Vec::new(),
            antelacion: ANTELACION,
            dias_antes: DIAS_ANTES.to_vec(),
            asignaturas: Vec::new(),
        }
    }
}

// ------------------------------------------------------------- leer

/// El dia de la semana de una palabra (`lunes`, `lun`, `lu`, `L`…).
/// `corto`: si se aceptan tambien las de una y dos letras, que solo valen
/// al principio de la linea (`mi`, `do` o `sa` son palabras de un nombre).
fn dia_de(p: &str, corto: bool) -> Option<u8> {
    let q = normalizar(p.trim_matches(|c: char| !c.is_alphanumeric()));
    let largo = match q.as_str() {
        "lunes" | "lun" => Some(0),
        "martes" | "mar" => Some(1),
        "miercoles" | "mie" | "mier" => Some(2),
        "jueves" | "jue" => Some(3),
        "viernes" | "vie" => Some(4),
        "sabado" | "sab" => Some(5),
        "domingo" | "dom" => Some(6),
        _ => None,
    };
    if largo.is_some() || !corto {
        return largo;
    }
    match q.as_str() {
        "lu" | "l" => Some(0),
        "ma" | "m" => Some(1),
        "mi" | "x" => Some(2),
        "ju" | "j" => Some(3),
        "vi" | "v" => Some(4),
        "sa" | "s" => Some(5),
        "do" | "d" => Some(6),
        _ => None,
    }
}

/// Los dias de una palabra: uno, una lista (`Lun,Mie`, `L/X`) o un tramo
/// (`Lun-Vie`, `L-V`). `None` si no es eso.
fn dias_de(p: &str, corto: bool) -> Option<Vec<u8>> {
    if let Some(d) = dia_de(p, corto) {
        return Some(vec![d]);
    }
    if let Some((a, b)) = p.split_once(['-', '–']) {
        let (a, b) = (dia_de(a, corto)?, dia_de(b, corto)?);
        return Some(if a <= b { (a..=b).collect() } else { vec![a, b] });
    }
    let partes: Vec<&str> = p.split([',', '/', ';']).filter(|s| !s.is_empty()).collect();
    if partes.len() > 1 {
        return partes.iter().map(|x| dia_de(x, corto)).collect();
    }
    None
}

/// Una hora: `8`, `08`, `8:30`, `8.30`, `8h`, `8h30`, `8:30pm`. En minutos.
fn hora_de(p: &str) -> Option<u32> {
    let q = normalizar(p.trim());
    let (q, tarde) = match q.strip_suffix("pm") {
        Some(r) => (r.trim().to_string(), Some(true)),
        None => match q.strip_suffix("am") {
            Some(r) => (r.trim().to_string(), Some(false)),
            None => (q, None),
        },
    };
    let (h, m) = match q.split_once([':', '.', 'h']) {
        Some((h, m)) => (h, if m.is_empty() { "0" } else { m }),
        None => (q.as_str(), "0"),
    };
    if h.is_empty() || h.len() > 2 || m.len() > 2 {
        return None;
    }
    let mut h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    if tarde == Some(true) && h < 12 {
        h += 12;
    }
    Some(h * 60 + m)
}

/// Un tramo de horas en una palabra (`8:00-10:00`, `8-10`).
fn tramo_de(p: &str) -> Option<(u32, u32)> {
    let (a, b) = p.split_once(['-', '–', '—'])?;
    Some((hora_de(a)?, hora_de(b)?))
}

/// Deja el tramo en horas del dia: una clase «de 2 a 4» es por la tarde.
fn arreglar_tramo((mut a, mut b): (u32, u32)) -> Option<(u32, u32)> {
    if a < 7 * 60 && b <= 12 * 60 {
        a += 12 * 60;
        b += 12 * 60;
    }
    // «10-2» acaba a las 14:00; «10-8» no es nada.
    if b <= a && b < 7 * 60 {
        b += 12 * 60;
    }
    (b > a && b <= 24 * 60).then_some((a, b))
}

/// Separa el aula del nombre: `@A-12`, `aula 12`, `salón 3`, `(Lab 2)`.
fn nombre_y_aula(palabras: &[&str]) -> (String, Option<String>) {
    let mut nombre = Vec::new();
    let mut aula = None;
    let mut i = 0;
    while i < palabras.len() {
        let p = palabras[i];
        if let Some(a) = p.strip_prefix("@(") {
            // `@(Lab de física)`: un aula de varias palabras (asi la escribe
            // la ventana).
            let mut v = vec![a];
            while !v.last().is_some_and(|x| x.ends_with(')')) && i + 1 < palabras.len() {
                i += 1;
                v.push(palabras[i]);
            }
            let a = v.join(" ");
            let a = a.strip_suffix(')').unwrap_or(&a).trim();
            if !a.is_empty() {
                aula = Some(a.to_string());
            }
        } else if let Some(a) = p.strip_prefix('@').filter(|a| !a.is_empty()) {
            aula = Some(a.to_string());
        } else if matches!(normalizar(p.trim_start_matches('(')).as_str(), "aula" | "salon" | "sala" | "lab" | "laboratorio")
            && let Some(sig) = palabras.get(i + 1)
        {
            aula = Some(format!("{} {}", p.trim_start_matches('('), sig.trim_end_matches(')')));
            i += 1;
        } else {
            nombre.push(p);
        }
        i += 1;
    }
    let nombre = nombre
        .join(" ")
        .trim_matches(|c: char| c == '-' || c == '·' || c == ':' || c == '|' || c.is_whitespace())
        .to_string();
    (nombre, aula)
}

/// **Una linea de clase**: dias al principio, el tramo de horas y el
/// nombre (con el aula). `None` si le falta algo.
fn clase_de_linea(linea: &str) -> Option<Clase> {
    let palabras: Vec<&str> = linea.split_whitespace().collect();
    let mut dias: Vec<u8> = Vec::new();
    let mut i = 0;
    // Los dias, al principio («Lun Mie», «Lunes y Miércoles», «L-V»).
    while i < palabras.len() {
        if let Some(d) = dias_de(palabras[i], true) {
            dias.extend(d);
        } else if !(normalizar(palabras[i]) == "y" && !dias.is_empty()) {
            break;
        }
        i += 1;
    }
    // (Con los dias delante, un dia de mas es del nombre: «Taller del Sabado».)
    let dias_delante = !dias.is_empty();
    // El tramo: en una palabra, o en tres («8:00 a 10:00», «8 - 10»).
    let mut tramo = None;
    let mut resto: Vec<&str> = Vec::new();
    let mut j = i;
    while j < palabras.len() {
        let p = palabras[j];
        if tramo.is_none() {
            if let Some(t) = tramo_de(p) {
                tramo = Some(t);
                j += 1;
                continue;
            }
            if let (Some(a), Some(sep), Some(b)) = (hora_de(p), palabras.get(j + 1), palabras.get(j + 2))
                && matches!(normalizar(sep).as_str(), "a" | "-" | "–" | "hasta")
                && let Some(b) = hora_de(b)
            {
                tramo = Some((a, b));
                j += 3;
                continue;
            }
            // «de 8 a 10»: el «de» no es del nombre.
            if normalizar(p) == "de" && palabras.get(j + 1).and_then(|x| hora_de(x)).is_some() {
                j += 1;
                continue;
            }
        }
        // Dias detras del nombre («Geometría lunes 8-10»): solo los largos,
        // y no si ya iban delante ni detras de «de»/«del».
        let tras_de = j > 0 && matches!(normalizar(palabras[j - 1]).as_str(), "de" | "del");
        if !dias_delante && !tras_de
            && let Some(d) = dias_de(p, false)
        {
            dias.extend(d);
        } else {
            resto.push(p);
        }
        j += 1;
    }
    let (inicio, fin) = arreglar_tramo(tramo?)?;
    let antelacion = quitar_avisar(&mut resto);
    let (nombre, aula) = nombre_y_aula(&resto);
    dias.sort_unstable();
    dias.dedup();
    if dias.is_empty() || nombre.is_empty() {
        return None;
    }
    Some(Clase {
        dias,
        inicio,
        fin,
        nombre,
        aula,
        notas: Vec::new(),
        antelacion,
    })
}

/// Saca de las palabras de una clase su «avisar 15 [min] [antes]»: los
/// minutos de antes de esa clase.
fn quitar_avisar(resto: &mut Vec<&str>) -> Option<u32> {
    let i = resto
        .iter()
        .position(|p| matches!(normalizar(p).as_str(), "avisar" | "aviso"))?;
    let n: u32 = resto.get(i + 1)?.parse().ok()?;
    let mut j = i + 2;
    if resto
        .get(j)
        .is_some_and(|p| matches!(normalizar(p).as_str(), "min" | "minutos"))
    {
        j += 1;
    }
    if resto.get(j).is_some_and(|p| normalizar(p) == "antes") {
        j += 1;
    }
    resto.drain(i..j);
    Some(n.min(24 * 60))
}

/// **Una linea de la lista lateral**: `Asignatura Geometría @A-12 #4F8EF7`.
fn asignatura_de_linea(palabras: &[&str]) -> Option<Asignatura> {
    let primera = normalizar(palabras.first()?.trim_end_matches(':'));
    if !matches!(primera.as_str(), "asignatura" | "materia") {
        return None;
    }
    let mut color = None;
    let resto: Vec<&str> = palabras[1..]
        .iter()
        .copied()
        .filter(|p| match color_de(p) {
            Some(c) => {
                color = Some(c);
                false
            }
            None => true,
        })
        .collect();
    let (nombre, aula) = nombre_y_aula(&resto);
    (!nombre.is_empty()).then_some(Asignatura {
        nombre,
        aula,
        color,
    })
}

/// Un color `#RRGGBB`.
fn color_de(p: &str) -> Option<u32> {
    let h = p.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    u32::from_str_radix(h, 16).ok()
}

/// Lo que dice que una linea es una evaluacion.
fn es_evaluacion(primera: &str) -> bool {
    matches!(
        normalizar(primera.trim_matches(|c: char| !c.is_alphanumeric())).as_str(),
        "evaluacion"
            | "examen"
            | "parcial"
            | "final"
            | "prueba"
            | "entrega"
            | "exposicion"
            | "control"
            | "test"
            | "quiz"
            | "practica"
            | "eval"
    )
}

/// Las celdas de una fila de tabla (`| a | b |` o separada por tabuladores).
fn celdas(linea: &str) -> Option<Vec<String>> {
    let t = linea.trim();
    let v: Vec<String> = if t.starts_with('|') {
        t.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect()
    } else if t.contains('\t') {
        t.split('\t').map(|c| c.trim().to_string()).collect()
    } else {
        return None;
    };
    Some(v)
}

/// Si una fila es la raya de debajo de la cabecera (`|---|:--:|`).
fn es_raya(v: &[String]) -> bool {
    v.iter().all(|c| c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

/// **Una tabla pegada.** Si la cabecera tiene dos o mas dias, es la
/// cuadricula de los colegios (las columnas son los dias y la primera
/// columna las horas); si no, cada fila es una clase como una linea.
fn clases_de_tabla(filas: &[Vec<String>]) -> Vec<Clase> {
    let filas: Vec<&Vec<String>> = filas.iter().filter(|f| !es_raya(f)).collect();
    let Some(cabecera) = filas.first() else {
        return Vec::new();
    };
    let columnas: Vec<(usize, u8)> = cabecera
        .iter()
        .enumerate()
        .filter_map(|(i, c)| dia_de(c, false).map(|d| (i, d)))
        .collect();
    let mut clases: Vec<Clase> = Vec::new();
    if columnas.len() >= 2 {
        for fila in &filas[1..] {
            let Some(tramo) = fila
                .first()
                .and_then(|c| {
                    let c = c.replace(' ', "");
                    tramo_de(&c)
                })
                .and_then(arreglar_tramo)
            else {
                continue;
            };
            for &(i, d) in &columnas {
                let Some(celda) = fila.get(i).filter(|c| !c.trim().is_empty()) else {
                    continue;
                };
                let palabras: Vec<&str> = celda.split_whitespace().collect();
                let (nombre, aula) = nombre_y_aula(&palabras);
                if nombre.is_empty() {
                    continue;
                }
                match clases
                    .iter_mut()
                    .find(|c| c.nombre == nombre && c.inicio == tramo.0 && c.fin == tramo.1)
                {
                    Some(c) => {
                        if !c.dias.contains(&d) {
                            c.dias.push(d);
                            c.dias.sort_unstable();
                        }
                    }
                    None => clases.push(Clase {
                        dias: vec![d],
                        inicio: tramo.0,
                        fin: tramo.1,
                        nombre,
                        aula,
                        notas: Vec::new(),
                        antelacion: None,
                    }),
                }
            }
        }
        return clases;
    }
    for fila in &filas {
        // Lo corto con cifras detras del nombre es el aula («A-12»).
        let mut fila: Vec<String> = fila.iter().filter(|c| !c.is_empty()).cloned().collect();
        let aula = (fila.len() >= 4)
            .then(|| fila.last().cloned())
            .flatten()
            .filter(|c| c.len() <= 10 && c.chars().any(|ch| ch.is_ascii_digit()) && tramo_de(c).is_none());
        if aula.is_some() {
            fila.pop();
        }
        if let Some(mut c) = clase_de_linea(&fila.join(" ")) {
            if aula.is_some() {
                c.aula = aula;
            }
            clases.push(c);
        }
    }
    clases
}

/// **Lee el horario** escrito. `ahora`: el momento local (para las fechas
/// de las evaluaciones, que se dicen como en las tareas: «23/10», «el
/// jueves»). Lo que no se entiende se salta: es una nota, puede llevar
/// titulos y comentarios.
pub fn leer(texto: &str, ahora: Momento) -> Horario {
    leer_con_sitios(texto, ahora).0
}

/// Los dos titulos que pone la ventana encima de lo que escribe: son parte
/// de lo suyo (se quitan y se vuelven a poner al guardar).
pub(crate) const TITULO_ASIGNATURAS: &str = "## Asignaturas";
pub(crate) const TITULO_CLASES: &str = "## Clases";

/// Como [`leer`], y ademas que lineas del texto son de clases o de
/// asignaturas (las que reescribe la ventana, [`semana::reescribir`]): las
/// lineas de clase, sus `>`, las tablas que dieron clases, las de
/// `Asignatura …` y los dos titulos de la ventana. Lo demas no se toca.
pub(crate) fn leer_con_sitios(texto: &str, ahora: Momento) -> (Horario, Vec<bool>) {
    let mut h = Horario::default();
    let lineas: Vec<&str> = texto.lines().collect();
    let mut suyas = vec![false; lineas.len()];
    let mut tabla: Vec<Vec<String>> = Vec::new();
    let mut filas_tabla: Vec<usize> = Vec::new();
    let mut en_comentario = false;
    let mut sueltas: Vec<String> = Vec::new();
    let cerrar_tabla = |tabla: &mut Vec<Vec<String>>,
                        filas: &mut Vec<usize>,
                        h: &mut Horario,
                        suyas: &mut Vec<bool>| {
        if !tabla.is_empty() {
            let clases = clases_de_tabla(tabla);
            if !clases.is_empty() {
                for &i in filas.iter() {
                    suyas[i] = true;
                }
            }
            h.clases.extend(clases);
            tabla.clear();
            filas.clear();
        }
    };
    for (n, linea) in lineas.iter().enumerate() {
        let t = linea.trim();
        // Los comentarios de HTML (`<!-- … -->`): ahi va el ejemplo.
        if en_comentario {
            if t.contains("-->") {
                en_comentario = false;
            }
            continue;
        }
        if t.starts_with("<!--") {
            en_comentario = !t.contains("-->");
            continue;
        }
        if let Some(c) = celdas(linea) {
            tabla.push(c);
            filas_tabla.push(n);
            continue;
        }
        cerrar_tabla(&mut tabla, &mut filas_tabla, &mut h, &mut suyas);
        if t == TITULO_ASIGNATURAS || t == TITULO_CLASES {
            suyas[n] = true;
            continue;
        }
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        // Lo que lee la voz, para la clase de encima.
        if let Some(nota) = t.strip_prefix('>') {
            let nota = nota.trim();
            if let Some(c) = h.clases.last_mut() {
                if !nota.is_empty() {
                    c.notas.push(nota.to_string());
                }
                suyas[n] = true;
            }
            continue;
        }
        let t = t.trim_start_matches(['-', '*', '•']).trim();
        let palabras: Vec<&str> = t.split_whitespace().collect();
        let primera = normalizar(palabras.first().copied().unwrap_or(""));
        // «avisar 15 min antes», «antelación 20», «avisar evaluaciones 5 y 2 días antes».
        if matches!(primera.as_str(), "avisar" | "aviso" | "avisame" | "antelacion" | "anticipacion") {
            let numeros: Vec<u32> = palabras
                .iter()
                .filter_map(|p| p.trim_matches(|c: char| !c.is_ascii_digit()).parse().ok())
                .collect();
            let q = normalizar(t);
            if q.contains("evaluacion") || q.contains("examen") || q.contains("dia") {
                if !numeros.is_empty() {
                    h.dias_antes = numeros;
                }
            } else if let Some(&n) = numeros.first() {
                h.antelacion = if q.contains("hora") { n * 60 } else { n }.min(24 * 60);
            }
            continue;
        }
        if let Some(a) = asignatura_de_linea(&palabras) {
            suyas[n] = true;
            if !h
                .asignaturas
                .iter()
                .any(|b| normalizar(&b.nombre) == normalizar(&a.nombre))
            {
                h.asignaturas.push(a);
            }
            continue;
        }
        if palabras.first().is_some_and(|p| es_evaluacion(p)) {
            sueltas.push(t.to_string());
            continue;
        }
        if let Some(c) = clase_de_linea(t) {
            suyas[n] = true;
            h.clases.push(c);
        }
    }
    cerrar_tabla(&mut tabla, &mut filas_tabla, &mut h, &mut suyas);
    // Las evaluaciones al final: nombran clases que pueden ir despues.
    for t in sueltas {
        let Ok((que, Some(cuando))) = fechas::separar(&t, ahora) else {
            continue;
        };
        let q = normalizar(&que);
        let clase = h
            .clases
            .iter()
            .position(|c| q.contains(&normalizar(&c.nombre)));
        h.evaluaciones.push(Evaluacion { que, clase, cuando });
    }
    (h, suyas)
}

// ------------------------------------------------------------- avisos

/// Que avisa un aviso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Que {
    /// La clase `.0`, la sesion que empieza en `.1` (ms locales).
    Clase(usize, i64),
    /// La evaluacion `.0`, `.1` dias antes.
    Evaluacion(usize, u32),
}

/// Un aviso con su hora (ms locales) y su id en la agenda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aviso {
    pub id: String,
    pub cuando: i64,
    pub que: Que,
}

fn ms_de(m: Momento) -> i64 {
    (m.dia * 1440 + i64::from(m.minuto)) * MINUTO
}

/// **Cuando toca cada aviso** desde `desde` (ms locales, sin incluirlo)
/// durante `dias` dias: cada sesion de cada clase `antelacion` minutos
/// antes de empezar, y cada evaluacion los `dias_antes` dias antes a su
/// hora. Del mas proximo al mas lejano.
pub fn avisos(h: &Horario, desde: i64, dias: i64) -> Vec<Aviso> {
    let hasta = desde + dias * DIA;
    let hoy = desde.div_euclid(DIA);
    let mut v = Vec::new();
    for (i, c) in h.clases.iter().enumerate() {
        // Desde ayer: una clase de las 00:05 avisa la vispera.
        for d in hoy - 1..=hoy + dias {
            if !c.dias.contains(&(fechas::dia_de_la_semana(d) as u8)) {
                continue;
            }
            let inicio = d * DIA + i64::from(c.inicio) * MINUTO;
            let cuando = inicio - i64::from(c.antelacion.unwrap_or(h.antelacion)) * MINUTO;
            if cuando > desde && cuando <= hasta {
                v.push(Aviso {
                    id: format!("{PREFIJO}clase:{inicio}:{}", c.nombre),
                    cuando,
                    que: Que::Clase(i, inicio),
                });
            }
        }
    }
    for (j, e) in h.evaluaciones.iter().enumerate() {
        let a_las = ms_de(e.cuando);
        for &n in &h.dias_antes {
            let cuando = a_las - i64::from(n) * DIA;
            if cuando > desde && cuando <= hasta {
                v.push(Aviso {
                    id: format!("{PREFIJO}eval:{n}:{a_las}:{}", e.que),
                    cuando,
                    que: Que::Evaluacion(j, n),
                });
            }
        }
    }
    v.sort_by(|a, b| a.cuando.cmp(&b.cuando).then_with(|| a.id.cmp(&b.id)));
    v
}

/// El aviso de un id de la agenda (el de un pospuesto lleva `|<ms>` detras).
pub fn aviso_de(h: &Horario, id: &str, desde: i64) -> Option<Aviso> {
    let id = id.split_once('|').map_or(id, |(a, _)| a);
    // Se busca desde una semana antes: el aviso ya vencio (y uno pospuesto
    // puede llevar un rato).
    avisos(h, desde - 7 * DIA, 15).into_iter().find(|a| a.id == id)
}

/// Cuando empezo la sesion anterior de una clase a la que empieza en
/// `inicio` (ms locales): la nota de voz grabada antes de esa ya no es de
/// esta clase.
pub fn sesion_anterior(c: &Clase, inicio: i64) -> Option<i64> {
    let dia = inicio.div_euclid(DIA);
    (1..=7).map(|k| dia - k).find_map(|d| {
        c.dias
            .contains(&(fechas::dia_de_la_semana(d) as u8))
            .then(|| d * DIA + i64::from(c.inicio) * MINUTO)
    })
}

/// Si una nota de voz guardada a `guardada` (ms locales) es para la sesion
/// que empieza en `inicio`: grabada despues de que empezara la anterior.
pub fn nota_vigente(c: &Clase, inicio: i64, guardada: i64) -> bool {
    sesion_anterior(c, inicio).is_none_or(|antes| guardada >= antes)
}

/// **La clase a la que va una nota de voz grabada ahora**: la que esta en
/// marcha, o la ultima que acabo hoy (hace menos de 3 horas); si no, la
/// proxima. La nota es «para la proxima clase» de esa asignatura.
pub fn clase_para_grabar(h: &Horario, ahora: i64) -> Option<usize> {
    let hoy = ahora.div_euclid(DIA);
    let minuto = (ahora.rem_euclid(DIA) / MINUTO) as u32;
    let semana = fechas::dia_de_la_semana(hoy) as u8;
    let de_hoy = |c: &&Clase| c.dias.contains(&semana);
    if let Some(i) = h
        .clases
        .iter()
        .position(|c| de_hoy(&c) && c.inicio <= minuto && minuto < c.fin)
    {
        return Some(i);
    }
    let reciente = h
        .clases
        .iter()
        .enumerate()
        .filter(|(_, c)| de_hoy(c) && c.fin <= minuto && minuto - c.fin <= 180)
        .max_by_key(|(_, c)| c.fin)
        .map(|(i, _)| i);
    reciente.or_else(|| {
        let mut h2 = h.clone();
        h2.antelacion = 0;
        for c in &mut h2.clases {
            c.antelacion = None;
        }
        avisos(&h2, ahora, 7).into_iter().find_map(|a| match a.que {
            Que::Clase(i, _) => Some(i),
            Que::Evaluacion(..) => None,
        })
    })
}

/// «08:00–10:00».
fn tramo_legible(c: &Clase) -> String {
    format!(
        "{:02}:{:02}–{:02}:{:02}",
        c.inicio / 60,
        c.inicio % 60,
        c.fin / 60,
        c.fin % 60
    )
}

/// Lo que se dice de una clase: el detalle de la tarjeta y lo que lee la
/// voz al contestar. Las evaluaciones de esa clase en la proxima semana van
/// en los dos: que «si o si» se recuerden.
pub fn detalle_y_texto(h: &Horario, i: usize, inicio: i64) -> (String, Option<String>) {
    let c = &h.clases[i];
    let mut detalle = tramo_legible(c);
    if let Some(a) = &c.aula {
        detalle.push_str(" · ");
        detalle.push_str(a);
    }
    let ahora = Momento::de_ms_locales(inicio);
    let mut leer: Vec<String> = c.notas.clone();
    for e in h.evaluaciones.iter().filter(|e| e.clase == Some(i)) {
        let falta = ms_de(e.cuando) - inicio;
        if (-DIA..=7 * DIA).contains(&falta) {
            let cuando = e.cuando.legible(ahora);
            detalle.push_str(&format!("\n📝 {} · {cuando}", e.que));
            leer.push(format!("{}, {cuando}", e.que));
        }
    }
    let texto = (!leer.is_empty()).then(|| leer.join(". "));
    (detalle, texto)
}

/// El nombre de fichero de la nota de voz de una clase.
pub fn fichero_de_voz(nombre: &str) -> String {
    let mut s: String = normalizar(nombre)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    format!("{}.m4a", s.trim_matches('-'))
}

/// **La llamada de un aviso.** `voz` es la carpeta de las notas de voz de
/// las clases y `guardada` dice cuando se grabo (ms locales) cada una: la
/// nota solo suena si es para esta sesion ([`nota_vigente`]).
pub fn llamada_de(
    h: &Horario,
    a: &Aviso,
    voz: &Path,
    guardada: impl Fn(&Path) -> Option<i64>,
) -> Llamada {
    match a.que {
        Que::Clase(i, inicio) => {
            let c = &h.clases[i];
            let (detalle, texto) = detalle_y_texto(h, i, inicio);
            let ruta = voz.join(fichero_de_voz(&c.nombre));
            let audio = guardada(&ruta)
                .filter(|&g| nota_vigente(c, inicio, g))
                .map(|_| ruta);
            Llamada {
                audio,
                nombre: c.nombre.clone(),
                carpeta: PathBuf::new(),
                mensaje: a.id.clone(),
                detalle,
                texto,
            }
        }
        Que::Evaluacion(j, n) => {
            let e = &h.evaluaciones[j];
            let cuando = e.cuando.legible(Momento::de_ms_locales(a.cuando));
            let en = if n == 1 {
                "mañana".to_string()
            } else {
                format!("en {n} días")
            };
            Llamada {
                audio: None,
                nombre: format!("📝 {}", e.que),
                carpeta: PathBuf::new(),
                mensaje: a.id.clone(),
                detalle: format!("{en} · {cuando}"),
                texto: Some(format!("Recuerda: {}, {en}, {cuando}", e.que)),
            }
        }
    }
}

// ------------------------------------------------------------- el disco

/// Donde vive el horario.
pub fn ruta(raiz: &Path) -> PathBuf {
    raiz.join(FICHERO)
}

/// Donde van las notas de voz de las clases.
pub fn carpeta_de_voz(raiz: &Path) -> PathBuf {
    raiz.join("horario").join("voz")
}

/// Con lo que nace el horario: el ejemplo va comentado, para que no llame.
pub const PLANTILLA: &str = "# Horario de clases

Una línea por clase: los días, la hora y la asignatura (el aula con @).
Debajo de una clase, lo que empieza por > lo lee la voz al contestar.
También puedes pegar una tabla (de Excel o de la web).

<!-- Ejemplo (quita estas dos líneas de comentario para usarlo):
Lun Mie 8:00-10:00 Geometría @A-12
> traer el compás
Mar Jue 10:15-12:00 Física
Evaluación Geometría 23/10
-->

avisar 10 min antes
";

/// Lee el horario del disco (vacio si no hay).
pub fn de_disco(raiz: &Path, ahora: Momento) -> Horario {
    std::fs::read_to_string(ruta(raiz))
        .map(|t| leer(&t, ahora))
        .unwrap_or_default()
}

// ------------------------------------------------------------- el Mac
//
// En Windows el vigia de los recordatorios lee estos avisos y `llamada.rs`
// pinta la tarjeta. En el Mac lo hace Swift (`Vigia`, `LlamadaSecreta`):
// aqui solo se dan los avisos ya hechos, con su llamada.

/// **Un aviso del horario** para el vigia del Mac: cuando suena (UTC) y la
/// llamada que sale.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AvisoHorario {
    /// El id en la agenda (`horario:clase:…`, `horario:eval:…`).
    pub id: String,
    pub cuando_utc_ms: i64,
    /// Quien «llama»: la asignatura, o «📝 Evaluación…».
    pub nombre: String,
    /// La hora y el aula, y las evaluaciones cercanas.
    pub detalle: String,
    /// Lo que lee la voz al contestar (lo apuntado con `>`).
    pub texto: Option<String>,
    /// La nota de voz de la clase grabada para esta sesion, si la hay.
    pub audio: Option<String>,
}

/// Cuando se grabo un fichero, en ms locales.
fn guardada_en(ruta: &Path, desfase_ms: i64) -> Option<i64> {
    let t = std::fs::metadata(ruta).ok()?.modified().ok()?;
    let utc = t.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64;
    Some(utc + desfase_ms)
}

fn aviso_mac(h: &Horario, a: &Aviso, raiz: &Path, desfase_ms: i64) -> AvisoHorario {
    let l = llamada_de(h, a, &carpeta_de_voz(raiz), |r| guardada_en(r, desfase_ms));
    AvisoHorario {
        id: a.id.clone(),
        cuando_utc_ms: a.cuando - desfase_ms,
        nombre: l.nombre,
        detalle: l.detalle,
        texto: l.texto,
        audio: l.audio.map(|p| p.display().to_string()),
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Los avisos del horario** de los proximos [`DIAS_A_LA_VISTA`] dias,
    /// del mas proximo al mas lejano. `desfase_ms`: hora local menos UTC.
    pub fn avisos_del_horario(&self, desfase_ms: i64) -> Vec<AvisoHorario> {
        let ahora = crate::ahora_ms() + desfase_ms;
        let h = de_disco(self.ruta(), Momento::de_ms_locales(ahora));
        let mut v: Vec<AvisoHorario> = avisos(&h, ahora, DIAS_A_LA_VISTA)
            .iter()
            .map(|a| aviso_mac(&h, a, self.ruta(), desfase_ms))
            .collect();
        // Los eventos de Google Calendar (Windows, 18-oct), con la misma
        // antelacion que las clases. Los de todo el dia no llaman.
        if crate::calendario_google::conectado() {
            let ahora_utc = crate::ahora_ms();
            for e in crate::calendario_google::eventos_entre(self.ruta(), ahora_utc, ahora_utc + DIAS_A_LA_VISTA * DIA) {
                let cuando = e.inicio_ms - i64::from(h.antelacion) * MINUTO;
                if e.todo_el_dia || cuando <= ahora_utc {
                    continue;
                }
                v.push(aviso_de_evento(&e, cuando, desfase_ms));
            }
            v.sort_by_key(|a| a.cuando_utc_ms);
        }
        v
    }

    /// El aviso de un id (al sonar, o uno pospuesto con `|<ms>` detras),
    /// leido otra vez del horario: si cambio, sale lo de ahora. `None` si ya
    /// no esta.
    pub fn aviso_del_horario(&self, id: String, desfase_ms: i64) -> Option<AvisoHorario> {
        // Uno de Google Calendar: `horario:gcal:<id>:<titulo>`.
        if let Some(resto) = id.strip_prefix(PREFIJO).and_then(|x| x.strip_prefix("gcal:")) {
            let ev = resto.split_once(':').map_or(resto, |(i, _)| i);
            let ev = ev.split('|').next().unwrap_or(ev);
            let e = crate::calendario_google::evento(self.ruta(), ev)?;
            return Some(aviso_de_evento(&e, e.inicio_ms, desfase_ms));
        }
        let ahora = crate::ahora_ms() + desfase_ms;
        let h = de_disco(self.ruta(), Momento::de_ms_locales(ahora));
        aviso_de(&h, &id, ahora).map(|a| aviso_mac(&h, &a, self.ruta(), desfase_ms))
    }

    /// Donde esta (o estara) el horario, sin crearlo.
    pub fn ruta_del_horario(&self) -> String {
        ruta(self.ruta()).display().to_string()
    }

    /// Donde esta el horario (`horario.md` junto a los datos); si aun no
    /// existe, se crea con la plantilla (el ejemplo comentado, que no llama).
    pub fn fichero_del_horario(&self) -> Result<String, crate::ErrorNucleo> {
        let r = ruta(self.ruta());
        if !r.is_file() {
            std::fs::write(&r, PLANTILLA)?;
        }
        Ok(r.display().to_string())
    }

    /// **«Nota de voz para la próxima clase»**: donde grabarla (la de la
    /// clase en marcha, o la ultima de hoy, o la proxima) y de que clase
    /// es. `None` si el horario no tiene clases.
    pub fn nota_de_voz_de_clase(&self, desfase_ms: i64) -> Option<NotaDeClase> {
        let ahora = crate::ahora_ms() + desfase_ms;
        let h = de_disco(self.ruta(), Momento::de_ms_locales(ahora));
        let i = clase_para_grabar(&h, ahora)?;
        let nombre = h.clases[i].nombre.clone();
        let carpeta = carpeta_de_voz(self.ruta());
        let _ = std::fs::create_dir_all(&carpeta);
        Some(NotaDeClase {
            ruta: carpeta.join(fichero_de_voz(&nombre)).display().to_string(),
            clase: nombre,
        })
    }
}

/// El aviso de un evento de Google Calendar: su titulo, su hora, su lugar
/// y su calendario.
fn aviso_de_evento(e: &pixpin_ia::conexiones::calendario::Evento, cuando_utc_ms: i64, desfase_ms: i64) -> AvisoHorario {
    let detalle = [crate::calendario_google::tramo_legible(e, desfase_ms), e.lugar.clone(), e.calendario.clone()]
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    AvisoHorario {
        id: format!("{PREFIJO}gcal:{}:{}", e.id, e.titulo),
        cuando_utc_ms,
        nombre: format!("📅 {}", e.titulo),
        detalle,
        texto: None,
        audio: None,
    }
}

/// Donde va la nota de voz de una clase.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NotaDeClase {
    pub ruta: String,
    pub clase: String,
}

#[cfg(test)]
mod pruebas_nombres_con_dia {
    #[test]
    fn un_dia_en_el_nombre_no_se_pierde() {
        let c = super::clase_de_linea("Lun Mié 8:00-10:00 Taller del Sábado @A-12").unwrap();
        assert_eq!(c.nombre, "Taller del Sábado");
        assert_eq!(c.dias, vec![0, 2]);
        let c = super::clase_de_linea("Geometría lunes 8-10").unwrap();
        assert_eq!((c.nombre.as_str(), c.dias.clone()), ("Geometría", vec![0]));
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Miercoles 14 de octubre de 2026, 10:30.
    fn ahora() -> Momento {
        Momento {
            dia: fechas::dias_de(2026, 10, 14),
            minuto: 10 * 60 + 30,
        }
    }

    fn ms(d: i64, h: i64, m: i64) -> i64 {
        (fechas::dias_de(2026, 10, d) * 1440 + h * 60 + m) * MINUTO
    }

    const EJEMPLO: &str = "# Horario

Lun Mie 8:00-10:00 Geometría @A-12
> traer el compás
> repasar el tema 3
Martes y Jueves 10:15 a 12:00 Física (Aula 7)
L-V 14-16 Inglés
- Evaluación Geometría 23/10
Examen de Física el jueves
avisar 15 min antes
<!-- comentario
Lun 8-9 No soy clase
-->
Esto no es nada
";

    #[test]
    fn una_linea_por_clase_con_dias_horas_y_aula() {
        let h = leer(EJEMPLO, ahora());
        assert_eq!(h.clases.len(), 3, "{:#?}", h.clases);
        let g = &h.clases[0];
        assert_eq!((g.dias.as_slice(), g.inicio, g.fin), (&[0u8, 2][..], 480, 600));
        assert_eq!((g.nombre.as_str(), g.aula.as_deref()), ("Geometría", Some("A-12")));
        assert_eq!(g.notas, ["traer el compás", "repasar el tema 3"]);
        let f = &h.clases[1];
        assert_eq!((f.dias.as_slice(), f.inicio, f.fin), (&[1u8, 3][..], 615, 720));
        assert_eq!((f.nombre.as_str(), f.aula.as_deref()), ("Física", Some("Aula 7")));
        let i = &h.clases[2];
        assert_eq!(i.dias, [0, 1, 2, 3, 4]);
        assert_eq!((i.inicio, i.fin), (14 * 60, 16 * 60), "14-16");
        assert_eq!(h.antelacion, 15);
        assert_eq!(h.dias_antes, DIAS_ANTES);
    }

    #[test]
    fn las_evaluaciones_llevan_su_fecha_y_su_clase() {
        let h = leer(EJEMPLO, ahora());
        assert_eq!(h.evaluaciones.len(), 2, "{:#?}", h.evaluaciones);
        let e = &h.evaluaciones[0];
        assert_eq!(e.que, "Evaluación Geometría");
        assert_eq!(e.clase, Some(0));
        assert_eq!(ms_de(e.cuando), ms(23, 9, 0));
        assert_eq!(h.evaluaciones[1].clase, Some(1), "Examen de Física");
        assert_eq!(ms_de(h.evaluaciones[1].cuando), ms(15, 9, 0), "el jueves");
    }

    #[test]
    fn caso_negativo_lo_que_no_es_clase_no_llama() {
        for t in [
            "Geometría",                  // sin dias ni horas
            "Lun Geometría",              // sin horas
            "8:00-10:00 Geometría",       // sin dias
            "Lun 10:00-8:00 Geometría",   // al reves
            "Lun 25:00-26:00 Geometría",  // horas que no existen
            "Lun 8-10",                   // sin nombre
            "Evaluación sin fecha",
            "Evaluación Física 1/1/2020", // ya paso
        ] {
            let h = leer(t, ahora());
            assert!(h.clases.is_empty() && h.evaluaciones.is_empty(), "{t}: {h:#?}");
        }
        // La plantilla no llama: el ejemplo va comentado.
        let p = leer(PLANTILLA, ahora());
        assert!(p.clases.is_empty() && p.evaluaciones.is_empty(), "{p:#?}");
        assert_eq!(p.antelacion, 10);
    }

    #[test]
    fn una_tabla_pegada_de_filas_o_cuadricula() {
        let filas = "| Día | Hora | Curso | Aula |\n|---|---|---|---|\n| Lunes | 8:00-10:00 | Geometría | A-12 |\n| Mie | 8:00-10:00 | Geometría | A-12 |";
        let h = leer(filas, ahora());
        assert_eq!(h.clases.len(), 2, "{:#?}", h.clases);
        assert_eq!((h.clases[0].nombre.as_str(), h.clases[0].aula.as_deref()), ("Geometría", Some("A-12")));
        // La de los colegios, pegada de Excel (tabuladores).
        let cuadricula = "Hora\tLunes\tMartes\tMiércoles\n8:00-10:00\tGeometría\t\tGeometría\n10:15-12:00\t\tFísica @B2\t";
        let h = leer(cuadricula, ahora());
        assert_eq!(h.clases.len(), 2, "{:#?}", h.clases);
        assert_eq!((h.clases[0].nombre.as_str(), h.clases[0].dias.as_slice()), ("Geometría", &[0u8, 2][..]));
        assert_eq!((h.clases[1].dias.as_slice(), h.clases[1].aula.as_deref()), (&[1u8][..], Some("B2")));
    }

    #[test]
    fn cada_aviso_toca_su_antelacion_antes_y_las_evaluaciones_3_y_1_dia_antes() {
        let h = leer(EJEMPLO, ahora());
        // Desde el miercoles 14 a las 10:30, dos dias por delante.
        let v = avisos(&h, ms(14, 10, 30), 2);
        let horas: Vec<(i64, String)> = v
            .iter()
            .map(|a| {
                (
                    a.cuando,
                    match a.que {
                        Que::Clase(i, _) => h.clases[i].nombre.clone(),
                        Que::Evaluacion(j, n) => format!("{}-{n}", h.evaluaciones[j].que),
                    },
                )
            })
            .collect();
        assert_eq!(
            horas,
            vec![
                (ms(14, 13, 45), "Inglés".to_string()),
                (ms(15, 10, 0), "Física".to_string()),
                (ms(15, 13, 45), "Inglés".to_string()),
            ],
            "la Geometría de hoy (7:45) y el aviso del examen del jueves (hoy 9:00) ya \
             pasaron; el Inglés del viernes cae fuera de los dos dias"
        );
        // La evaluacion del 23: el 20 y el 22 a las 9.
        let v = avisos(&h, ms(19, 0, 0), 4);
        let de_eval: Vec<i64> = v
            .iter()
            .filter(|a| matches!(a.que, Que::Evaluacion(0, _)))
            .map(|a| a.cuando)
            .collect();
        assert_eq!(de_eval, [ms(20, 9, 0), ms(22, 9, 0)]);
        // Y el id lleva al mismo aviso aunque ya haya vencido.
        let a = &v[0];
        assert_eq!(aviso_de(&h, &a.id, a.cuando + 60_000).as_ref(), Some(a));
        assert_eq!(aviso_de(&h, &format!("{}|123", a.id), a.cuando).as_ref(), Some(a));
        assert_eq!(aviso_de(&h, "horario:clase:1:Nada", a.cuando), None);
    }

    #[test]
    fn la_nota_de_voz_es_de_esta_clase_si_se_grabo_tras_la_anterior() {
        let h = leer(EJEMPLO, ahora());
        let g = &h.clases[0]; // lunes y miercoles a las 8
        let lunes_19 = ms(19, 8, 0);
        assert_eq!(sesion_anterior(g, lunes_19), Some(ms(14, 8, 0)), "el miercoles de antes");
        assert!(nota_vigente(g, lunes_19, ms(14, 9, 30)), "grabada en la clase del miercoles");
        assert!(!nota_vigente(g, lunes_19, ms(12, 9, 0)), "la del lunes pasado ya no");
        // La llamada: con nota, nombre, hora, aula y lo que lee la voz.
        let a = avisos(&h, ms(18, 12, 0), 2)
            .into_iter()
            .find(|a| matches!(a.que, Que::Clase(0, _)))
            .unwrap();
        let voz = Path::new("C:/voz");
        let l = llamada_de(&h, &a, voz, |_| Some(ms(14, 9, 30)));
        assert_eq!(l.nombre, "Geometría");
        assert_eq!(l.audio, Some(voz.join("geometria.m4a")));
        assert!(l.detalle.starts_with("08:00–10:00 · A-12"), "{}", l.detalle);
        assert!(l.detalle.contains("📝 Evaluación Geometría"), "la del 23, en esa semana: {}", l.detalle);
        assert!(l.texto.as_deref().unwrap().starts_with("traer el compás. repasar el tema 3"));
        assert!(l.es_de_horario());
        // Sin nota vigente, solo la voz de la app.
        let l = llamada_de(&h, &a, voz, |_| Some(ms(1, 9, 0)));
        assert_eq!(l.audio, None);
        let l = llamada_de(&h, &a, voz, |_| None);
        assert_eq!(l.audio, None);
    }

    #[test]
    fn la_nota_se_graba_para_la_clase_en_marcha_o_la_ultima_o_la_proxima() {
        let h = leer(EJEMPLO, ahora());
        assert_eq!(clase_para_grabar(&h, ms(14, 9, 0)), Some(0), "en Geometría");
        assert_eq!(clase_para_grabar(&h, ms(14, 11, 0)), Some(0), "acaba de salir");
        assert_eq!(clase_para_grabar(&h, ms(15, 11, 0)), Some(1), "jueves, en Física");
        assert_eq!(clase_para_grabar(&h, ms(17, 9, 0)), Some(0), "sabado: la proxima es Geometría del lunes");
        assert_eq!(clase_para_grabar(&Horario::default(), ms(14, 9, 0)), None);
        assert_eq!(fichero_de_voz("Geometría Analítica II"), "geometria-analitica-ii.m4a");
    }
}
