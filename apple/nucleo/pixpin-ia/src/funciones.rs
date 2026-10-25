//! **Graficar funciones** (`graficar_funcion`): una sola herramienta para
//! todo lo que un estudiante grafica —`y = f(x)` (tambien a trozos),
//! **parametricas** `x(t), y(t)`, **polares** `r(θ)` y **puntos** de una
//! tabla—, varias a la vez con su leyenda, sobre el **instrumento plano
//! cartesiano** del lienzo (`pixpin_motor2d::instrumento`), y con lo que se
//! pide en clase: **marcar raices, extremos e intersecciones**, **sombrear el
//! area** entre dos curvas, la **recta tangente** en un punto y las
//! **asintotas**.
//!
//! **Las cuentas las hace Rust, no el modelo** (`calculo.rs`): el modelo solo
//! escribe la formula (`x^2 - 3sin(x)`) y lo que quiere ver; el interprete
//! del lienzo (`pixpin_motor2d::formula`, el mismo de la grafica del panel y
//! del movil) la evalua en cientos de puntos, y lo calculado (las raices, el
//! area) vuelve al modelo como aviso para que lo diga sin inventarlo.
//!
//! El instrumento tiene una sola unidad para los dos ejes y el origen en su
//! centro (`instrumento::punto`): se elige la unidad para que quepan el
//! dominio y los valores, y la curva se corta donde la funcion no existe o
//! se sale (asi `1/x` deja su hueco y `tan(x)` sale por ramas). Sin dominio,
//! se elige uno que ensene lo interesante (raices, extremos, polos).

use serde_json::{Map, Value, json};

use pixpin_motor2d::ecuacion::{self, Estilo};
use pixpin_motor2d::elemento::{ColorRgba, Elemento};
use pixpin_motor2d::formula::{self, Compilada};
use pixpin_motor2d::instrumento as ins;
use pixpin_motor2d::vector::Punto2;

use crate::calculo;
use crate::herramientas::{ErrorHerramienta, Herramienta};
use crate::lienzo::Fabrica;
use crate::pizarra::{self, Pizarra, numero};

pub const NOMBRE: &str = "graficar_funcion";
/// Cuantas curvas como mucho en una grafica.
pub const MAX_FUNCIONES: usize = 6;
/// Lo mas lejos que llega un limite.
pub const LIMITE: f64 = 1e6;
/// Cuantas muestras de `x`.
pub const MUESTRAS: usize = 600;
/// Cuantas muestras de `t` en una parametrica o polar.
pub const MUESTRAS_T: usize = 1200;
/// Cuantos puntos de datos como mucho.
pub const MAX_PUNTOS: usize = 200;
/// Cuantas marcas como mucho (que no tapen la curva).
pub const MAX_MARCAS: usize = 24;
/// La mayor diferencia de escala entre el alto y el ancho de la ventana.
const PROPORCION: f64 = 4.0;

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

pub fn herramienta() -> Herramienta {
    Herramienta {
        nombre: NOMBRE.into(),
        descripcion: concat!(
            "Grafica y = f(x) (a trozos: x^2 si x<0; x si x>=0), parametricas {\"x\":\"3cos(t)\",\"y\":\"2sin(t)\"}, polares {\"r\":\"1+cos(t)\"} (t = θ) ",
            "y puntos [[x,y]]. La app calcula: marca raices, extremos e intersecciones, sombrea el area entre curvas [i,j] (o [i] con el eje x), ",
            "traza la tangente y las asintotas. Ej: {\"funciones\":[\"x^2-4\"],\"marcar\":[\"raices\"]} · {\"funciones\":[\"x^2\",\"x+2\"],\"sombrear\":{\"entre\":[0,1]}}"
        )
        .into(),
        parametros: json!({
            "type": "object",
            "required": ["funciones"],
            "properties": {
                "funciones": {"type": "array", "maxItems": MAX_FUNCIONES, "items": {"type": ["string", "object"]}},
                "x_desde": {"type": "number"},
                "x_hasta": {"type": "number"},
                "y_desde": {"type": "number"},
                "y_hasta": {"type": "number"},
                "t_desde": {"type": "number"},
                "t_hasta": {"type": "number"},
                "puntos": {"type": "array", "items": {"type": "array", "items": {"type": "number"}}},
                "marcar": {"type": "array", "items": {"type": "string", "enum": ["raices", "extremos", "intersecciones", "corte_y"]}},
                "sombrear": {"type": "object", "properties": {"entre": {"type": "array", "items": {"type": "integer"}}, "desde": {"type": "number"}, "hasta": {"type": "number"}}},
                "tangente": {"type": "object", "properties": {"curva": {"type": "integer"}, "x": {"type": "number"}}},
                "asintotas": {"type": "boolean"},
                "titulo": {"type": "string"}
            }
        }),
    }
}

/// Una curva de la grafica.
#[derive(Debug, Clone, PartialEq)]
pub enum Curva {
    /// `y = f(x)`.
    Explicita { texto: String, f: Compilada },
    /// `x = x(t), y = y(t)`.
    Parametrica { tx: String, ty: String, x: Compilada, y: Compilada },
    /// `r = r(θ)` (en la formula, `t`).
    Polar { texto: String, r: Compilada },
    /// Una tabla de datos.
    Puntos { nombre: String, puntos: Vec<(f64, f64)>, unir: bool },
}

impl Curva {
    /// El rotulo de la leyenda, como se escribiria en la pizarra.
    pub fn rotulo(&self) -> String {
        match self {
            Curva::Explicita { texto, .. } => format!("y = {}", sin_igual(texto)),
            Curva::Parametrica { tx, ty, .. } => format!("x = {}, y = {}", sin_igual(tx), sin_igual(ty)),
            Curva::Polar { texto, .. } => format!("r = {}", sin_igual(texto).replace("theta", "θ")),
            Curva::Puntos { nombre, .. } => nombre.clone(),
        }
    }
}

/// Lo que se marca en la curva.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoMarca {
    Raiz,
    Maximo,
    Minimo,
    Interseccion,
    CorteY,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Marca {
    pub x: f64,
    pub y: f64,
    pub tipo: TipoMarca,
}

/// El area sombreada: entre la curva `curva` y la `otra` (o el eje x).
#[derive(Debug, Clone, PartialEq)]
pub struct Sombra {
    pub curva: usize,
    pub otra: Option<usize>,
    pub desde: f64,
    pub hasta: f64,
    pub area: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tangente {
    pub curva: usize,
    pub x: f64,
    pub y: f64,
    pub m: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Asintota {
    Vertical(f64),
    /// `y = m x + b` (m = 0: horizontal).
    Recta(f64, f64),
}

/// Lo que se pide, ya comprobado y calculado.
#[derive(Debug, Clone, PartialEq)]
pub struct Funciones {
    pub curvas: Vec<Curva>,
    /// El dominio de las `y = f(x)` y lo que se ve de x.
    pub x_desde: f64,
    pub x_hasta: f64,
    pub y_desde: f64,
    pub y_hasta: f64,
    pub t_desde: f64,
    pub t_hasta: f64,
    pub titulo: String,
    pub marcas: Vec<Marca>,
    pub sombra: Option<Sombra>,
    pub tangente: Option<Tangente>,
    pub asintotas: Vec<Asintota>,
}

impl Funciones {
    /// Las formulas `y = f(x)` (para quien solo quiere esas).
    pub fn formulas(&self) -> Vec<String> {
        self.curvas
            .iter()
            .filter_map(|c| match c {
                Curva::Explicita { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect()
    }
}

fn numero_en(v: Option<&Value>, k: &str) -> Result<Option<f64>, ErrorHerramienta> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_f64().filter(|v| v.is_finite()).map(Some).ok_or_else(|| mal(format!("«{k}» no es un numero"))),
        Some(Value::String(s)) => {
            let t = s.trim().replace(',', ".");
            // «2pi», «π/2»: con el interprete de formulas.
            t.parse::<f64>()
                .ok()
                .or_else(|| formula::compilar(&preparar(&t)).filter(|c| c.variables().is_empty()).map(|c| c.en(0.0)))
                .filter(|v| v.is_finite())
                .map(Some)
                .ok_or_else(|| mal(format!("«{k}» = «{s}» no es un numero")))
        }
        Some(_) => Err(mal(format!("«{k}» tiene que ser un numero"))),
    }
}

fn numero_arg(args: &Map<String, Value>, k: &str) -> Result<Option<f64>, ErrorHerramienta> {
    numero_en(args.get(k), k)
}

/// La formula lista para el interprete: el angulo θ es `t`, y los signos de
/// la pizarra (·, \cdot) los de siempre.
fn preparar(f: &str) -> String {
    f.replace(['θ', 'ϑ', 'φ'], "t")
        .replace("theta", "t")
        .replace("\\cdot", "*")
        .replace('\\', "")
        .trim()
        .to_string()
}

/// `y = x^2` → `x^2`, para el rotulo.
fn sin_igual(f: &str) -> String {
    let t = f.trim();
    let mut c = t.chars();
    if let Some(a) = c.next()
        && a.is_alphabetic()
    {
        let resto: String = c.collect();
        let resto = resto.trim_start();
        let resto = if resto.starts_with('(') { resto.split_once(')').map_or(resto, |(_, r)| r).trim_start() } else { resto };
        if let Some(r) = resto.strip_prefix('=')
            && !r.starts_with('=')
        {
            return r.trim().to_string();
        }
    }
    t.to_string()
}

fn compilar_en(f: &str, variable: char) -> Result<Compilada, ErrorHerramienta> {
    if f.chars().count() > 160 {
        return Err(mal(format!("la formula «{}...» es demasiado larga", f.chars().take(30).collect::<String>())));
    }
    let c = formula::compilar(&preparar(f))
        .ok_or_else(|| mal(format!("la formula «{f}» no se entiende; escribela como x^2 + 3x - 1 o sin(x)/x")))?;
    if let Some(v) = c.variables().into_iter().find(|v| *v != variable) {
        return Err(mal(match variable {
            'x' if v == 't' => format!("«{f}» usa t: para una parametrica pon {{\"x\":\"...\",\"y\":\"...\"}} y para una polar {{\"r\":\"...\"}}"),
            _ => format!("«{f}» usa la variable «{v}»; aqui solo vale {variable}"),
        }));
    }
    Ok(c)
}

/// Parte por comas o punto y coma de primer nivel (fuera de parentesis).
fn partir_arriba(s: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut nivel = 0i32;
    let mut actual = String::new();
    for ch in s.chars() {
        match ch {
            '(' | '[' | '{' => nivel += 1,
            ')' | ']' | '}' => nivel -= 1,
            ',' | ';' if nivel == 0 => {
                v.push(std::mem::take(&mut actual));
                continue;
            }
            _ => {}
        }
        actual.push(ch);
    }
    v.push(actual);
    v.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// Lo que va antes del igual de `x = ...` (una palabra), y lo de detras.
fn lado_izquierdo(s: &str) -> Option<(String, String)> {
    // «r = ...» y tambien «r: ...» (lo escribe asi el 2B).
    let (izq, der) = s.split_once('=').or_else(|| s.split_once(':').filter(|(i, _)| i.trim().chars().count() <= 2))?;
    let izq = izq.trim().to_lowercase();
    let nombre: String = izq.chars().take_while(|c| c.is_alphabetic()).collect();
    (!nombre.is_empty() && !der.starts_with('=')).then(|| (nombre, der.trim().to_string()))
}

/// Una curva escrita como texto: `x^2`, `y = x^2`, `r = 1 + cos θ`,
/// `x = 3cos(t), y = 2sin(t)` o `(3cos(t), 2sin(t))`.
fn curva_de_texto(s: &str) -> Result<Curva, ErrorHerramienta> {
    let t = s.trim();
    // Parametrica entre parentesis: (f(t), g(t)).
    if t.starts_with('(') && t.ends_with(')') {
        let dentro = partir_arriba(&t[1..t.len() - 1]);
        if dentro.len() == 2 && t.contains('t') {
            return parametrica(&dentro[0], &dentro[1]);
        }
    }
    // Las partes de una por partes van con «si»: no son una parametrica.
    if !t.contains(" si ") {
        let trozos = partir_arriba(t);
        if trozos.len() == 2
            && let (Some((na, fa)), Some((nb, fb))) = (lado_izquierdo(&trozos[0]), lado_izquierdo(&trozos[1]))
        {
            if na == "x" && nb == "y" {
                return parametrica(&fa, &fb);
            }
            if na == "y" && nb == "x" {
                return parametrica(&fb, &fa);
            }
        }
    }
    if let Some((n, der)) = lado_izquierdo(t)
        && (n == "r" || n == "ρ")
    {
        return polar(&der);
    }
    // Sin «r =» pero con θ y sin x: una polar escrita a secas.
    let usa_angulo = ['θ', 'ϑ', 'φ'].iter().any(|c| t.contains(*c)) || t.contains("theta");
    if usa_angulo && !t.contains('x') {
        return polar(&sin_igual(t));
    }
    let f = compilar_en(t, 'x')?;
    Ok(Curva::Explicita { texto: t.to_string(), f })
}

fn parametrica(fx: &str, fy: &str) -> Result<Curva, ErrorHerramienta> {
    Ok(Curva::Parametrica { tx: fx.to_string(), ty: fy.to_string(), x: compilar_en(fx, 't')?, y: compilar_en(fy, 't')? })
}

fn polar(f: &str) -> Result<Curva, ErrorHerramienta> {
    Ok(Curva::Polar { texto: f.to_string(), r: compilar_en(f, 't')? })
}

fn tabla(v: &Value, que: &str) -> Result<Vec<(f64, f64)>, ErrorHerramienta> {
    let Value::Array(l) = v else { return Err(mal(format!("«{que}» tiene que ser una lista [[x, y], ...]"))) };
    if l.len() > MAX_PUNTOS {
        return Err(mal(format!("como mucho {MAX_PUNTOS} puntos")));
    }
    let mut v = Vec::new();
    for p in l {
        let par: Option<(f64, f64)> = match p {
            Value::Array(xy) if xy.len() == 2 => xy[0].as_f64().zip(xy[1].as_f64()),
            Value::Object(o) => o.get("x").and_then(Value::as_f64).zip(o.get("y").and_then(Value::as_f64)),
            _ => None,
        };
        let (x, y) = par.ok_or_else(|| mal(format!("cada punto de «{que}» es [x, y]")))?;
        if !(x.is_finite() && y.is_finite()) || x.abs() > LIMITE || y.abs() > LIMITE {
            return Err(mal(format!("el punto ({x}, {y}) es enorme o no es un numero")));
        }
        v.push((x, y));
    }
    if v.is_empty() {
        return Err(mal(format!("«{que}» esta vacia")));
    }
    Ok(v)
}

fn curva_de_objeto(o: &Map<String, Value>) -> Result<Curva, ErrorHerramienta> {
    let s = |k: &str| o.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    if let Some(p) = o.get("puntos") {
        let nombre = s("nombre").unwrap_or("datos").chars().take(40).collect();
        let unir = o.get("unir").and_then(Value::as_bool).unwrap_or(false);
        return Ok(Curva::Puntos { nombre, puntos: tabla(p, "puntos")?, unir });
    }
    if let Some(r) = s("r") {
        return polar(r);
    }
    match (s("x"), s("y")) {
        (Some(x), Some(y)) => parametrica(x, y),
        (None, Some(y)) => curva_de_texto(y),
        (Some(x), None) => Err(mal(format!("falta «y» para la parametrica de x = {x}"))),
        (None, None) => match s("formula").or(s("funcion")).or(s("f")) {
            Some(f) => curva_de_texto(f),
            None => Err(mal("cada curva es una formula, {\"x\":..,\"y\":..}, {\"r\":..} o {\"puntos\":[[x,y],...]}")),
        },
    }
}

/// Las muestras de una curva no explicita, con un hueco (`None`) donde no
/// existe.
fn muestras_t(c: &Curva, t0: f64, t1: f64) -> Vec<Option<(f64, f64)>> {
    let punto = |t: f64| -> Option<(f64, f64)> {
        let (x, y) = match c {
            Curva::Parametrica { x, y, .. } => (x.evaluar(0.0, 0.0, t), y.evaluar(0.0, 0.0, t)),
            Curva::Polar { r, .. } => {
                let r = r.evaluar(0.0, 0.0, t);
                (r * t.cos(), r * t.sin())
            }
            _ => return None,
        };
        (x.is_finite() && y.is_finite() && x.abs() <= LIMITE && y.abs() <= LIMITE).then_some((x, y))
    };
    match c {
        Curva::Puntos { puntos, .. } => puntos.iter().map(|&p| Some(p)).collect(),
        Curva::Explicita { .. } => Vec::new(),
        _ => (0..=MUESTRAS_T).map(|i| punto(t0 + (t1 - t0) * i as f64 / MUESTRAS_T as f64)).collect(),
    }
}

fn indices(v: &Value) -> Vec<usize> {
    match v {
        Value::Array(l) => l.iter().filter_map(|x| x.as_f64()).map(|x| x.max(0.0) as usize).collect(),
        Value::Number(n) => n.as_f64().map(|x| vec![x.max(0.0) as usize]).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Un dominio que ensene lo interesante de las `y = f(x)`: sus raices,
/// extremos y polos, con el origen dentro. Las periodicas (muchas cosas
/// que ensenar), dos vueltas.
fn dominio_automatico(fs: &[&Compilada]) -> (f64, f64) {
    let mut puntos: Vec<f64> = Vec::new();
    for f in fs {
        let g = |x: f64| f.en(x);
        let (r, p) = calculo::ceros(&g, -50.0, 50.0);
        puntos.extend(r);
        puntos.extend(p);
        puntos.extend(calculo::extremos(&g, -50.0, 50.0).into_iter().map(|e| e.0));
    }
    if puntos.len() > 12 * fs.len().max(1) {
        let dos_pi = 2.0 * std::f64::consts::PI;
        return (-dos_pi, dos_pi);
    }
    if puntos.is_empty() {
        return (-5.0, 5.0);
    }
    let lo = puntos.iter().copied().fold(f64::MAX, f64::min);
    let hi = puntos.iter().copied().fold(f64::MIN, f64::max);
    let margen = ((hi - lo) * 0.25).max(1.0);
    // Lo interesante con un margen, el origen dentro y al menos 4 de ancho
    // (el instrumento es simetrico: un dominio de mas estira la y).
    let (mut a, mut b) = ((lo - margen).min(0.0).floor(), (hi + margen).max(0.0).ceil());
    if b - a < 4.0 {
        let falta = (4.0 - (b - a)) / 2.0;
        a -= falta;
        b += falta;
    }
    (a, b)
}

/// Lo que mandan los modelos pequenos para una parametrica, arreglado:
/// las claves con tilde (`{"función": ...}`) pasan a texto, y un «x = f(t)»
/// seguido de un «y = g(t)» sueltos son UNA curva (medido con el 2B el
/// 14-oct: `[{"función":"x = 3cos(t)"},{"función":"y = 2sin(t)"}]`).
fn juntar_parametricas(lista: Vec<Value>) -> Vec<Value> {
    let texto_de = |v: &Value| -> Option<String> {
        match v {
            Value::String(s) => Some(s.trim().to_string()),
            Value::Object(o) if o.len() <= 2 => o
                .iter()
                .find(|(k, _)| matches!(crate::acciones::normalizar(k).as_str(), "funcion" | "formula" | "f" | "expresion" | "ecuacion"))
                .and_then(|(_, v)| v.as_str())
                .map(|s| s.trim().to_string()),
            _ => None,
        }
    };
    let mut salida = Vec::new();
    let mut i = 0;
    while i < lista.len() {
        let a = texto_de(&lista[i]);
        let b = lista.get(i + 1).and_then(texto_de);
        if let (Some(a), Some(b)) = (&a, &b)
            && let (Some((na, fa)), Some((nb, fb))) = (lado_izquierdo(a), lado_izquierdo(b))
            && na == "x"
            && nb == "y"
            && (fa.contains('t') || fb.contains('t'))
        {
            salida.push(json!({"x": fa, "y": fb}));
            i += 2;
            continue;
        }
        salida.push(match a {
            Some(t) if !lista[i].is_string() => Value::String(t),
            _ => lista[i].clone(),
        });
        i += 1;
    }
    salida
}

pub fn validar(args: &Map<String, Value>, avisos: &mut Vec<String>) -> Result<Funciones, ErrorHerramienta> {
    let lista: Vec<Value> = match args.get("funciones").or(args.get("funcion")).or(args.get("formula")).or(args.get("curvas")) {
        Some(Value::String(s)) => s.split(['\n', '|']).map(|s| Value::String(s.to_string())).collect(),
        Some(Value::Array(l)) => l.clone(),
        Some(o @ Value::Object(_)) => vec![o.clone()],
        None if args.get("puntos").is_some() => Vec::new(),
        _ => return Err(mal("falta «funciones» (una lista de formulas en x, como \"x^2\")")),
    };
    // Un objeto escrito como texto (`"{\"x\":\"3cos(t)\",...}"`, del 2B): el objeto.
    let lista: Vec<Value> = lista
        .into_iter()
        .map(|v| {
            let objeto = v.as_str().filter(|s| s.trim_start().starts_with('{')).and_then(|s| serde_json::from_str::<Value>(s.trim()).ok()).filter(Value::is_object);
            objeto.unwrap_or(v)
        })
        .collect();
    let lista = juntar_parametricas(lista);
    let mut curvas = Vec::new();
    for v in &lista {
        match v {
            Value::String(s) if s.trim().is_empty() => {}
            Value::String(s) => curvas.push(curva_de_texto(s)?),
            Value::Object(o) => curvas.push(curva_de_objeto(o)?),
            _ => return Err(mal("cada curva de «funciones» es un texto o un objeto")),
        }
    }
    if let Some(p) = args.get("puntos") {
        let nombre = args.get("nombre_puntos").and_then(Value::as_str).unwrap_or("datos").chars().take(40).collect();
        let unir = args.get("unir").and_then(Value::as_bool).unwrap_or(false);
        curvas.push(Curva::Puntos { nombre, puntos: tabla(p, "puntos")?, unir });
    }
    if curvas.is_empty() {
        return Err(mal("«funciones» esta vacia"));
    }
    if curvas.len() > MAX_FUNCIONES {
        return Err(mal(format!("como mucho {MAX_FUNCIONES} funciones por grafica")));
    }
    let explicitas: Vec<&Compilada> = curvas
        .iter()
        .filter_map(|c| match c {
            Curva::Explicita { f, .. } => Some(f),
            _ => None,
        })
        .collect();
    let (x_desde, x_hasta) = match (numero_arg(args, "x_desde")?, numero_arg(args, "x_hasta")?) {
        (None, None) if !explicitas.is_empty() => dominio_automatico(&explicitas),
        (None, None) => (f64::NAN, f64::NAN),
        (Some(a), Some(b)) => (a, b),
        _ => return Err(mal("pon los dos limites del dominio: «x_desde» y «x_hasta»")),
    };
    let x_dado = x_desde.is_finite();
    if x_dado {
        if !(x_desde < x_hasta) {
            return Err(mal(format!("el dominio esta al reves o vacio: x_desde = {x_desde} tiene que ser menor que x_hasta = {x_hasta}")));
        }
        if x_desde.abs() > LIMITE || x_hasta.abs() > LIMITE || x_hasta - x_desde < 1e-9 {
            return Err(mal(format!("el dominio tiene que estar entre -{LIMITE} y {LIMITE} y no ser un punto")));
        }
    }
    let (t_desde, t_hasta) = match (numero_arg(args, "t_desde")?, numero_arg(args, "t_hasta")?) {
        (None, None) => (0.0, 2.0 * std::f64::consts::PI),
        (Some(a), None) => (a, a + 2.0 * std::f64::consts::PI),
        (None, Some(b)) => (0.0_f64.min(b - 1e-3), b),
        (Some(a), Some(b)) => (a, b),
    };
    if !(t_desde < t_hasta) || t_desde.abs() > 1e4 || t_hasta.abs() > 1e4 {
        return Err(mal(format!("el intervalo de t esta al reves o es enorme: t_desde = {t_desde}, t_hasta = {t_hasta}")));
    }
    // Las muestras, para ver si hay curva y ajustar la ventana.
    let mut ys: Vec<f64> = Vec::new();
    let mut xs_otras: Vec<f64> = Vec::new();
    let mut ys_otras: Vec<f64> = Vec::new();
    for c in &curvas {
        match c {
            Curva::Explicita { texto, f } => {
                let antes = ys.len();
                for i in 0..=MUESTRAS {
                    let x = x_desde + (x_hasta - x_desde) * i as f64 / MUESTRAS as f64;
                    let y = f.en(x);
                    if y.is_finite() && y.abs() <= LIMITE {
                        ys.push(y);
                    }
                }
                if ys.len() - antes < 2 {
                    return Err(mal(format!("«{texto}» no tiene valores reales entre x = {x_desde} y x = {x_hasta}: cambia el dominio")));
                }
            }
            otra => {
                let m: Vec<(f64, f64)> = muestras_t(otra, t_desde, t_hasta).into_iter().flatten().collect();
                if m.is_empty() || (!matches!(otra, Curva::Puntos { .. }) && m.len() < 2) {
                    return Err(mal(format!("«{}» no tiene valores reales entre t = {t_desde} y t = {t_hasta}", otra.rotulo())));
                }
                xs_otras.extend(m.iter().map(|p| p.0));
                ys_otras.extend(m.iter().map(|p| p.1));
            }
        }
    }
    let ventana_de = |v: &[f64]| -> (f64, f64) {
        let lo = v.iter().copied().fold(f64::MAX, f64::min);
        let hi = v.iter().copied().fold(f64::MIN, f64::max);
        let m = ((hi - lo) * 0.1).max(0.5);
        (lo - m, hi + m)
    };
    let (x_desde, x_hasta) = if x_dado {
        (x_desde, x_hasta)
    } else {
        let (a, b) = ventana_de(&xs_otras);
        (a.min(-1.0), b.max(1.0))
    };
    let (mut y_auto_lo, mut y_auto_hi) = if ys.is_empty() { (f64::MAX, f64::MIN) } else { ventana(&mut ys) };
    if !ys_otras.is_empty() {
        let (a, b) = ventana_de(&ys_otras);
        y_auto_lo = y_auto_lo.min(a);
        y_auto_hi = y_auto_hi.max(b);
    }
    let (y_desde, y_hasta) = match (numero_arg(args, "y_desde")?, numero_arg(args, "y_hasta")?) {
        (Some(a), Some(b)) if a < b && a.abs() <= LIMITE && b.abs() <= LIMITE => (a, b),
        (Some(a), Some(b)) => return Err(mal(format!("la ventana de y esta al reves o fuera de rango: y_desde = {a}, y_hasta = {b}"))),
        (None, None) => (y_auto_lo, y_auto_hi),
        (Some(a), None) => (a, y_auto_hi.max(a + 1.0)),
        (None, Some(b)) => (y_auto_lo.min(b - 1.0), b),
    };
    // Una sola unidad para los dos ejes: si la y se dispara, la curva se
    // corta arriba y abajo en vez de dejar la x en un hilo.
    let ax = x_desde.abs().max(x_hasta.abs());
    let ay = y_desde.abs().max(y_hasta.abs());
    let (y_desde, y_hasta) = if ay > ax * PROPORCION && !explicitas.is_empty() {
        avisos.push(format!(
            "la curva sube mucho mas de lo que mide el dominio: se ensena y entre {} y {} (pide y_desde/y_hasta para otra ventana)",
            ins::escrito(y_desde.max(-ax * PROPORCION)),
            ins::escrito(y_hasta.min(ax * PROPORCION))
        ));
        (y_desde.max(-ax * PROPORCION), y_hasta.min(ax * PROPORCION))
    } else {
        (y_desde, y_hasta)
    };
    let titulo = args.get("titulo").and_then(Value::as_str).unwrap_or("").trim().chars().take(120).collect();
    let mut p = Funciones {
        curvas,
        x_desde,
        x_hasta,
        y_desde,
        y_hasta,
        t_desde,
        t_hasta,
        titulo,
        marcas: Vec::new(),
        sombra: None,
        tangente: None,
        asintotas: Vec::new(),
    };
    analizar(&mut p, args, avisos)?;
    Ok(p)
}

/// La `y = f(x)` numero `k` de la lista, o el motivo.
fn explicita(p: &Funciones, k: usize, para: &str) -> Result<Compilada, ErrorHerramienta> {
    match p.curvas.get(k) {
        Some(Curva::Explicita { f, .. }) => Ok(f.clone()),
        Some(_) => Err(mal(format!("{para} solo se hace con curvas y = f(x); la curva {k} no lo es"))),
        None => Err(mal(format!("{para}: no hay curva {k} (se cuentan desde 0; hay {})", p.curvas.len()))),
    }
}

/// **Las cuentas**: marcas, area, tangente y asintotas, y lo calculado
/// dicho al modelo (en `avisos`).
fn analizar(p: &mut Funciones, args: &Map<String, Value>, avisos: &mut Vec<String>) -> Result<(), ErrorHerramienta> {
    let (a, b) = (p.x_desde, p.x_hasta);
    let explicitas: Vec<(usize, Compilada)> = p
        .curvas
        .iter()
        .enumerate()
        .filter_map(|(k, c)| match c {
            Curva::Explicita { f, .. } => Some((k, f.clone())),
            _ => None,
        })
        .collect();
    // Que marcar.
    let pedidas: Vec<String> = match args.get("marcar") {
        Some(Value::String(s)) => s.split([',', ' ']).map(crate::acciones::normalizar).collect(),
        Some(Value::Array(l)) => l.iter().filter_map(Value::as_str).map(crate::acciones::normalizar).collect(),
        Some(Value::Bool(true)) => vec!["todo".into()],
        _ => Vec::new(),
    };
    let pedidas: Vec<String> = pedidas.into_iter().filter(|s| !s.is_empty()).collect();
    let quiere = |raiz: &[&str]| pedidas.iter().any(|q| q == "todo" || raiz.iter().any(|r| q.starts_with(r)));
    if !pedidas.is_empty() && explicitas.is_empty() {
        return Err(mal("marcar raices, extremos o intersecciones solo se hace con curvas y = f(x)"));
    }
    let mut dichos: Vec<String> = Vec::new();
    for (k, f) in &explicitas {
        let g = |x: f64| f.en(x);
        let nombre = p.curvas[*k].rotulo();
        if quiere(&["raic", "raiz", "cero"]) {
            let (r, _) = calculo::ceros(&g, a, b);
            dichos.push(if r.is_empty() {
                format!("{nombre} no corta el eje x entre {} y {}", numero(a), numero(b))
            } else {
                format!("raices de {nombre}: {}", r.iter().map(|x| format!("x = {}", numero(*x))).collect::<Vec<_>>().join(", "))
            });
            p.marcas.extend(r.into_iter().map(|x| Marca { x, y: 0.0, tipo: TipoMarca::Raiz }));
        }
        if quiere(&["extrem", "maxim", "minim", "vertic"]) {
            let e = calculo::extremos(&g, a, b);
            if e.is_empty() {
                dichos.push(format!("{nombre} no tiene maximos ni minimos locales entre {} y {}", numero(a), numero(b)));
            }
            for (x, es_max) in e {
                let y = calculo::limpio(g(x));
                dichos.push(format!("{} de {nombre} en ({}, {})", if es_max { "maximo" } else { "minimo" }, numero(x), numero(y)));
                p.marcas.push(Marca { x, y, tipo: if es_max { TipoMarca::Maximo } else { TipoMarca::Minimo } });
            }
        }
        if quiere(&["corte_y", "corte y", "ordenada"]) && a <= 0.0 && 0.0 <= b && g(0.0).is_finite() {
            let y = calculo::limpio(g(0.0));
            dichos.push(format!("{nombre} corta el eje y en (0, {})", numero(y)));
            p.marcas.push(Marca { x: 0.0, y, tipo: TipoMarca::CorteY });
        }
    }
    if quiere(&["inters", "cortes"]) && explicitas.len() >= 2 {
        for i in 0..explicitas.len() {
            for j in i + 1..explicitas.len() {
                let (fi, fj) = (&explicitas[i].1, &explicitas[j].1);
                let h = |x: f64| fi.en(x) - fj.en(x);
                let (r, _) = calculo::ceros(&h, a, b);
                for x in r {
                    let y = calculo::limpio(fi.en(x));
                    if y.is_finite() {
                        dichos.push(format!("se cortan en ({}, {})", numero(x), numero(y)));
                        p.marcas.push(Marca { x, y, tipo: TipoMarca::Interseccion });
                    }
                }
            }
        }
    }
    p.marcas.truncate(MAX_MARCAS);
    // El area.
    if let Some(s) = args.get("sombrear").or(args.get("area")) {
        let (entre, desde, hasta) = match s {
            Value::Object(o) => (
                o.get("entre").or(o.get("curvas")).or(o.get("curva")).map(indices).unwrap_or_else(|| vec![0]),
                numero_en(o.get("desde"), "desde")?,
                numero_en(o.get("hasta"), "hasta")?,
            ),
            Value::Bool(true) => (vec![0, 1], None, None),
            otro => (indices(otro), None, None),
        };
        let entre = if entre.is_empty() { vec![0] } else { entre };
        let f = explicita(p, entre[0], "sombrear un area")?;
        let g: Option<Compilada> = match entre.get(1) {
            Some(&j) if j != entre[0] => Some(explicita(p, j, "sombrear un area")?),
            _ => None,
        };
        let dif = |x: f64| f.en(x) - g.as_ref().map_or(0.0, |g| g.en(x));
        let (desde, hasta) = match (desde, hasta) {
            (Some(d), Some(h)) => (d, h),
            _ => {
                // Entre los cortes de las dos (o con el eje), si los hay.
                let (r, _) = calculo::ceros(&dif, a, b);
                if r.len() >= 2 { (r[0], r[r.len() - 1]) } else { (desde.unwrap_or(a), hasta.unwrap_or(b)) }
            }
        };
        if !(desde < hasta) {
            return Err(mal(format!("el area va de «desde» = {desde} a «hasta» = {hasta}: al reves o vacia")));
        }
        let area = calculo::area_entre(&|x| f.en(x), &|x| g.as_ref().map_or(0.0, |g| g.en(x)), desde, hasta).ok_or_else(|| {
            mal(format!("el area entre x = {} y x = {} no es finita (hay un polo o un hueco en medio)", numero(desde), numero(hasta)))
        })?;
        dichos.push(format!("area sombreada entre x = {} y x = {}: {}", numero(desde), numero(hasta), numero(area)));
        p.sombra = Some(Sombra { curva: entre[0], otra: g.as_ref().map(|_| entre[1]), desde, hasta, area });
    }
    // La tangente.
    if let Some(t) = args.get("tangente") {
        let (k, x0) = match t {
            Value::Object(o) => (
                o.get("curva").and_then(Value::as_f64).unwrap_or(0.0).max(0.0) as usize,
                numero_en(o.get("x").or(o.get("en")), "x")?.ok_or_else(|| mal("la tangente necesita «x»: {\"curva\":0,\"x\":1}"))?,
            ),
            Value::Number(n) => (0, n.as_f64().unwrap_or(0.0)),
            _ => return Err(mal("«tangente» es {\"curva\":0,\"x\":1}")),
        };
        let f = explicita(p, k, "la tangente")?;
        let g = |x: f64| f.en(x);
        let y0 = g(x0);
        if !y0.is_finite() {
            return Err(mal(format!("la curva {k} no existe en x = {}: no hay tangente", numero(x0))));
        }
        if !calculo::derivable(&g, x0) {
            return Err(mal(format!("la curva {k} no es derivable en x = {} (tiene un pico): no hay tangente", numero(x0))));
        }
        let m = calculo::limpio((calculo::derivada(&g, x0) * 1e6).round() / 1e6);
        let y0 = calculo::limpio(y0);
        dichos.push(format!("tangente en x = {}: pendiente {}, recta y = {}", numero(x0), numero(m), recta(m, calculo::limpio(y0 - m * x0))));
        p.tangente = Some(Tangente { curva: k, x: x0, y: y0, m });
    }
    // Las asintotas: las verticales siempre que haya polos; las demas si se
    // piden o si la curva tiene polos (una racional).
    let pide = args.get("asintotas").and_then(Value::as_bool);
    if pide != Some(false) {
        for (k, f) in &explicitas {
            let g = |x: f64| f.en(x);
            let polos = calculo::polos(&g, a, b);
            for &x in &polos {
                dichos.push(format!("asintota vertical x = {} ({})", numero(x), p.curvas[*k].rotulo()));
                p.asintotas.push(Asintota::Vertical(x));
            }
            if pide == Some(true) || !polos.is_empty() {
                let mut vistas: Vec<(f64, f64)> = Vec::new();
                for lado in [1.0, -1.0] {
                    if let Some((m, q)) = calculo::asintota_al_infinito(&g, lado)
                        && !calculo::es_la_recta(&g, (m, q), a, b)
                        && !vistas.contains(&(m, q))
                    {
                        vistas.push((m, q));
                        dichos.push(format!(
                            "asintota {} y = {} ({})",
                            if m == 0.0 { "horizontal" } else { "oblicua" },
                            recta(m, q),
                            p.curvas[*k].rotulo()
                        ));
                        p.asintotas.push(Asintota::Recta(m, q));
                    }
                }
            }
        }
        if pide == Some(true) && p.asintotas.is_empty() {
            dichos.push("no hay asintotas en lo que se ve".into());
        }
    }
    if !dichos.is_empty() {
        avisos.push(format!("calculado por la app: {}", dichos.join("; ")));
    }
    Ok(())
}

/// `y = m x + b` escrito bonito.
fn recta(m: f64, b: f64) -> String {
    let m_txt = match m {
        m if m == 0.0 => String::new(),
        m if m == 1.0 => "x".into(),
        m if m == -1.0 => "-x".into(),
        m => format!("{}x", numero(m)),
    };
    match (m_txt.is_empty(), b) {
        (true, b) => numero(b),
        (false, b) if b == 0.0 => m_txt,
        (false, b) if b < 0.0 => format!("{m_txt} - {}", numero(-b)),
        (false, b) => format!("{m_txt} + {}", numero(b)),
    }
}

/// La ventana de la y que deja ver la curva: lo que hay entre los
/// percentiles 2 y 98 (un polo como el de `1/x` no aplasta lo demas), con
/// un margen, y el cero dentro si cae cerca.
fn ventana(ys: &mut [f64]) -> (f64, f64) {
    ys.sort_by(f64::total_cmp);
    let n = ys.len();
    let (mut a, mut b) = (ys[n * 2 / 100], ys[(n * 98 / 100).min(n - 1)]);
    if b - a < 1e-9 {
        a -= 1.0;
        b += 1.0;
    }
    let margen = (b - a) * 0.08;
    let (mut a, mut b) = (a - margen, b + margen);
    if a > 0.0 && a < (b - a) * 0.5 {
        a = 0.0;
    }
    if b < 0.0 && -b < (b - a) * 0.5 {
        b = 0.0;
    }
    (a, b)
}

/// Los colores de las curvas (los de la grafica del lienzo).
fn color_de_curva(k: usize) -> ColorRgba {
    pixpin_motor2d::grafica::color_de(pixpin_motor2d::grafica::COLORES_DE_CURVAS[k % pixpin_motor2d::grafica::COLORES_DE_CURVAS.len()])
}

/// **Los elementos**: el instrumento, las curvas por ramas, la leyenda con
/// la ecuacion de cada una y el titulo, todo en un grupo.
pub(crate) fn elementos(f: &mut Fabrica, p: &Funciones) {
    let piezas = piezas(p, (60.0, 40.0, 680.0, 520.0));
    let antes = f.grupo.clone();
    f.grupo = Some(f.nombre_de_grupo("funciones"));
    f.de_motor(&piezas);
    f.grupo = antes;
}

/// Lo que ocupan unos elementos colocados desde el (0, 0).
fn medida(els: &[Elemento]) -> (f32, f32) {
    let (mut x1, mut y1) = (0.0f32, 0.0f32);
    for e in els {
        let (_, _, a, b) = e.caja();
        x1 = x1.max(a);
        y1 = y1.max(b);
    }
    (x1, y1)
}

/// Sobre que se dibuja la grafica.
enum Marco {
    /// El instrumento plano cartesiano (una unidad, el origen en el centro,
    /// como `Plano.kt` de Android): editable y medible en el lienzo.
    Instrumento(Elemento, ins::Instrumento),
    /// Ejes con una escala por eje y el rango pedido (`pizarra::Ejes`).
    Ejes(pizarra::Ejes),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarcoElegido {
    Instrumento,
    Ejes,
}

/// **Instrumento o ejes.** El instrumento ensena siempre de -a a a con la
/// misma unidad en los dos ejes; vale cuando la ventana es casi simetrica y
/// cabe sin quedarse en un hilo. Si no (banco del 15-oct: «sin x entre 0 y
/// π» salia de -π a π; la tangente de x³ - 3x, en una tira estrecha), los
/// ejes de la pizarra: el rango pedido tal cual y una escala por eje.
fn marco_de(p: &Funciones) -> MarcoElegido {
    let ax = p.x_desde.abs().max(p.x_hasta.abs());
    let ay = p.y_desde.abs().max(p.y_hasta.abs());
    let simetrica_x = p.x_desde < 0.0 && p.x_hasta > 0.0 && (p.x_hasta - p.x_desde) >= 1.5 * ax;
    let simetrica_y = (p.y_hasta - p.y_desde) >= 1.0 * ay;
    // Con una sola unidad, una y que sube mucho mas que la x deja la grafica
    // en un hilo (la tangente de x³ - 3x: ±12 frente a ±3).
    let llena = ay / ax <= 3.5 && ax / ay <= 3.5;
    if simetrica_x && simetrica_y && llena { MarcoElegido::Instrumento } else { MarcoElegido::Ejes }
}

/// Las piezas del motor dentro de la caja `(x, y, ancho, alto)` del lienzo
/// de 800x600.
pub fn piezas(p: &Funciones, (cx, cy, cw, ch): (f64, f64, f64, f64)) -> Vec<Elemento> {
    let negro = pizarra::TINTA;
    let familia = "Fraunces";
    let medir = |t: &str, tam: f32| crate::lienzo::medir_con(t, tam, familia);
    let estilo = Estilo { color: negro, tam: 18.0, opacidad: 1.0, familia: familia.into() };
    // La leyenda, para saber cuanto sitio deja: la ecuacion compuesta de
    // cada y = f(x) y el rotulo de las demas.
    let leyenda: Vec<Vec<Elemento>> = p
        .curvas
        .iter()
        .enumerate()
        .map(|(k, c)| {
            let color = color_de_curva(k);
            match c {
                Curva::Explicita { f, .. } => {
                    ecuacion::caja_de_la_ecuacion("y", f, estilo.tam, &medir).elementos(0.0, 0.0, &estilo.con_color(color))
                }
                Curva::Puntos { nombre, .. } => {
                    let mut pz = Pizarra::nueva();
                    pz.punto(4.0, 10.0, "", color);
                    pz.texto(nombre, 16.0, 0.0, estilo.tam * 0.9, color, pizarra::Alinear::Izquierda);
                    pz.el
                }
                // La parametrica, en dos renglones (x arriba, y abajo).
                // La polar, con θ (la formula la escribe con t) y sin los `*`
                // de la cuenta: «r = 2 sin(3θ)», no «r = 2·sin3·t».
                otra => pizarra::caja_de_ecuacion(
                    &pizarra::para_ver(&otra.rotulo().replace(", y = ", "\ny = "), matches!(otra, Curva::Polar { .. })),
                    0.0,
                    0.0,
                    estilo.tam * 0.9,
                    color,
                )
                .0,
            }
        })
        .collect();
    let mut extra_leyenda: Vec<String> = Vec::new();
    if let Some(s) = &p.sombra {
        extra_leyenda.push(format!("Área ≈ {}", numero(s.area)));
    }
    if let Some(t) = &p.tangente {
        extra_leyenda.push(format!("tangente: y = {}", recta(t.m, calculo::limpio(t.y - t.m * t.x))));
    }
    let ancho_leyenda = leyenda
        .iter()
        .map(|l| medida(l).0 as f64)
        .chain(extra_leyenda.iter().map(|t| medir(t, 15.0).0 as f64))
        .fold(0.0, f64::max)
        .min(cw * 0.4);
    let alto_titulo = if p.titulo.is_empty() { 0.0 } else { 40.0 };
    let (w, h) = (cw - ancho_leyenda - 24.0, ch - alto_titulo);
    let ax = p.x_desde.abs().max(p.x_hasta.abs());
    let ay = p.y_desde.abs().max(p.y_hasta.abs());
    // La unidad: que quepan el dominio y la ventana de la y.
    let u = (w / (2.0 * ax)).min(h / (2.0 * ay)).clamp(0.05, 400.0);
    let (lado_x, lado_y) = (2.0 * ax * u, 2.0 * ay * u);
    let marco = match marco_de(p) {
        MarcoElegido::Instrumento => {
            let mut e = ins::elemento_de_fabrica(2, negro, 1.5, 1.0);
            if let pixpin_motor2d::elemento::Figura::Instrumento(i) = &mut e.figura {
                i.unidad = Some(u);
                // Numeros cada 1, 2, 5, 10...: ni amontonados ni sueltos.
                let paso = pixpin_motor2d::grafica::paso_bonito(2.0 * ax.max(ay)).max(ins::paso_util(1.0, u, 34.0).min(1e9));
                i.paso_numeros = Some(paso);
                i.paso_cuadros = Some(paso / if paso >= 2.0 { 2.0 } else { 1.0 });
            }
            e.ancho = lado_x as f32;
            e.alto = lado_y as f32;
            e.x = (cx + (w - lado_x) / 2.0) as f32;
            e.y = (cy + alto_titulo + (h - lado_y) / 2.0) as f32;
            let i = *ins::de(&e).expect("es un instrumento");
            Marco::Instrumento(e, i)
        }
        // El rango pedido tal cual (0 a π no es -π a π) y una escala por eje
        // (la tangente de x^3 ya no sale en un hilo): los ejes de la pizarra.
        MarcoElegido::Ejes => Marco::Ejes(pizarra::Ejes {
            caja: [cx + 34.0, cy + alto_titulo + 14.0, (w - 54.0).max(80.0), (h - 40.0).max(80.0)],
            x0: p.x_desde,
            x1: p.x_hasta,
            y0: p.y_desde,
            y1: p.y_hasta,
        }),
    };
    let en = |x: f64, y: f64| -> (f64, f64) {
        match &marco {
            Marco::Instrumento(e, i) => {
                let q = ins::punto(e, i, x, y, 0.0);
                (q.x as f64, q.y as f64)
            }
            Marco::Ejes(j) => j.a_lienzo(x, y),
        }
    };
    let pt = |x: f64, y: f64| -> Punto2 {
        let (a, b) = en(x, y);
        Punto2::nuevo(a as f32, b as f32)
    };
    // Lo que se ve de x.
    let (xa, xb) = match &marco {
        Marco::Instrumento(..) => (-ax, ax),
        Marco::Ejes(_) => (p.x_desde, p.x_hasta),
    };
    let caja_marco: [f64; 4] = match &marco {
        Marco::Instrumento(e, _) => [e.x as f64, e.y as f64, e.ancho as f64, e.alto as f64],
        Marco::Ejes(j) => j.caja,
    };
    let dentro = |x: f64, y: f64| x >= xa - 1e-9 && x <= xb + 1e-9 && y >= p.y_desde - 1e-9 && y <= p.y_hasta + 1e-9;
    let mut salida = match &marco {
        Marco::Instrumento(e, _) => vec![e.clone()],
        Marco::Ejes(j) => {
            let mut pz = Pizarra::nueva();
            j.dibujar(&mut pz, "x", "y");
            pz.el
        }
    };
    let mut pz = Pizarra::nueva();
    // El area, debajo de todo.
    if let Some(s) = &p.sombra
        && let Some(Curva::Explicita { f, .. }) = p.curvas.get(s.curva)
    {
        let otra = s.otra.and_then(|j| p.curvas.get(j));
        let g = |x: f64| match otra {
            Some(Curva::Explicita { f, .. }) => f.en(x),
            _ => 0.0,
        };
        let n = 200;
        let recorte = |y: f64| y.clamp(p.y_desde, p.y_hasta);
        let xk = |k: usize| s.desde + (s.hasta - s.desde) * k as f64 / n as f64;
        let ida = (0..=n).map(|k| en(xk(k), recorte(f.en(xk(k)))));
        let vuelta = (0..=n).rev().map(|k| en(xk(k), recorte(g(xk(k)))));
        let contorno: Vec<(f64, f64)> = ida.chain(vuelta).collect();
        pz.region(&contorno, color_de_curva(s.curva), 0.25);
    }
    // Las asintotas, a trazos y grises.
    for a in &p.asintotas {
        match *a {
            Asintota::Vertical(x) if x >= xa && x <= xb => {
                pz.discontinua(&[en(x, p.y_desde), en(x, p.y_hasta)], pizarra::GRIS, 1.5);
            }
            Asintota::Recta(m, q) => {
                let tramo: Vec<(f64, f64)> = (0..=100)
                    .map(|k| xa + (xb - xa) * k as f64 / 100.0)
                    .filter(|&x| dentro(x, m * x + q))
                    .map(|x| en(x, m * x + q))
                    .collect();
                // Recta: solo sus dos puntas (con cien tramos, a trazos se
                // veia como una raya entera).
                if let (Some(&a), Some(&b)) = (tramo.first(), tramo.last()) {
                    pz.discontinua(&[a, b], pizarra::GRIS, 1.5);
                }
            }
            _ => {}
        }
    }
    salida.append(&mut pz.el);
    for (k, c) in p.curvas.iter().enumerate() {
        let de_curva = estilo.con_color(color_de_curva(k));
        let mut rama: Vec<Punto2> = Vec::new();
        let cerrar = |rama: &mut Vec<Punto2>, salida: &mut Vec<Elemento>| {
            if rama.len() >= 2 {
                salida.push(ecuacion::elemento_linea(std::mem::take(rama), 2.5, &de_curva));
            }
            rama.clear();
        };
        match c {
            Curva::Explicita { f, .. } => {
                let mut anterior: Option<f64> = None;
                for s in 0..=MUESTRAS {
                    let x = p.x_desde + (p.x_hasta - p.x_desde) * s as f64 / MUESTRAS as f64;
                    let y = f.en(x);
                    // Un salto enorme entre dos muestras es un polo: se corta.
                    let salto = anterior.is_some_and(|a| (y - a).abs() > (p.y_hasta - p.y_desde) * 0.9);
                    if !y.is_finite() || y < p.y_desde || y > p.y_hasta || salto {
                        cerrar(&mut rama, &mut salida);
                        anterior = y.is_finite().then_some(y);
                        if !y.is_finite() || y < p.y_desde || y > p.y_hasta {
                            continue;
                        }
                    }
                    anterior = Some(y);
                    rama.push(pt(x, y));
                }
                cerrar(&mut rama, &mut salida);
            }
            Curva::Puntos { puntos, unir, .. } => {
                let mut pz = Pizarra::nueva();
                let dentro_p: Vec<(f64, f64)> = puntos.iter().copied().filter(|&(x, y)| dentro(x, y)).collect();
                if *unir {
                    pz.linea(&dentro_p.iter().map(|&(x, y)| en(x, y)).collect::<Vec<_>>(), color_de_curva(k), 2.0);
                }
                for &(x, y) in &dentro_p {
                    let (px, py) = en(x, y);
                    pz.punto(px, py, "", color_de_curva(k));
                }
                salida.append(&mut pz.el);
            }
            otra => {
                let mut anterior: Option<(f64, f64)> = None;
                let (sx, sy) = (xb - xa, p.y_hasta - p.y_desde);
                for m in muestras_t(otra, p.t_desde, p.t_hasta) {
                    match m {
                        Some((x, y)) if dentro(x, y) => {
                            let salto = anterior.is_some_and(|(a, b)| (x - a).abs() > sx * 0.5 || (y - b).abs() > sy * 0.5);
                            if salto {
                                cerrar(&mut rama, &mut salida);
                            }
                            anterior = Some((x, y));
                            rama.push(pt(x, y));
                        }
                        _ => {
                            cerrar(&mut rama, &mut salida);
                            anterior = None;
                        }
                    }
                }
                cerrar(&mut rama, &mut salida);
            }
        }
    }
    // La tangente y las marcas, encima.
    let mut pz = Pizarra::nueva();
    if let Some(t) = &p.tangente {
        let tramo: Vec<(f64, f64)> = (0..=100)
            .map(|k| xa + (xb - xa) * k as f64 / 100.0)
            .filter(|&x| dentro(x, t.y + t.m * (x - t.x)))
            .map(|x| en(x, t.y + t.m * (x - t.x)))
            .collect();
        pz.linea(&tramo, pizarra::MORADO, 2.0);
    }
    // Los puntos marcados con sus coordenadas, que no se pisen entre si ni
    // con las curvas y los ejes (`rotulos::colocar`): antes iban todos
    // arriba a la derecha y en las raices juntas salian montados.
    let mut marcados: Vec<((f64, f64), String, ColorRgba)> = Vec::new();
    if let Some(t) = &p.tangente {
        marcados.push((en(t.x, t.y), format!("({}, {})", numero(t.x), numero(t.y)), pizarra::MORADO));
    }
    for m in p.marcas.iter().filter(|m| dentro(m.x, m.y)) {
        let rotulo = format!("({}, {})", numero(m.x), numero(m.y));
        // El mismo punto por dos motivos (raiz y corte con el eje y): una vez.
        if !marcados.iter().any(|(_, r, _)| *r == rotulo) {
            marcados.push((en(m.x, m.y), rotulo, pizarra::TINTA));
        }
    }
    if !marcados.is_empty() {
        let tam = 13.0f32;
        let mut ocupado: Vec<crate::rotulos::Caja> = Vec::new();
        for el in salida.iter().chain(&pz.el) {
            match &el.figura {
                pixpin_motor2d::Figura::Linea { puntos } | pixpin_motor2d::Figura::Flecha { puntos, .. } => {
                    let v: Vec<(f64, f64)> = puntos.iter().map(|q| (q.x as f64, q.y as f64)).collect();
                    ocupado.extend(crate::rotulos::cajas_de_linea(&v, 5.0));
                }
                pixpin_motor2d::Figura::Texto { .. } => {
                    let (x0, y0, x1, y1) = el.caja();
                    ocupado.push([x0 as f64, y0 as f64, (x1 - x0) as f64, (y1 - y0) as f64]);
                }
                _ => {}
            }
        }
        // Los ejes del instrumento (no son lineas sueltas).
        if let Marco::Instrumento(..) = &marco {
            ocupado.extend(crate::rotulos::cajas_de_linea(&[en(xa, 0.0), en(xb, 0.0)], 4.0));
            ocupado.extend(crate::rotulos::cajas_de_linea(&[en(0.0, p.y_desde), en(0.0, p.y_hasta)], 4.0));
        }
        let pedidos: Vec<crate::rotulos::Rotulo> = marcados
            .iter()
            .map(|(a, t, _)| {
                let (w, h) = pizarra::medir(t, tam);
                crate::rotulos::Rotulo::en(*a, w as f64, h as f64)
            })
            .collect();
        let sitios = crate::rotulos::colocar(&pedidos, &ocupado, Some(caja_marco));
        for ((ancla, t, color), (x, y)) in marcados.iter().zip(sitios) {
            pz.punto(ancla.0, ancla.1, "", *color);
            pz.texto(t, x, y, tam, *color, pizarra::Alinear::Izquierda);
        }
    }
    salida.append(&mut pz.el);
    // La leyenda, a la derecha, del color de cada curva.
    let mut y_ley = caja_marco[1] + 8.0;
    let x_ley = caja_marco[0] + caja_marco[2] + 24.0;
    for l in leyenda {
        let (_, alto) = medida(&l);
        for mut el in l {
            el.mover(x_ley as f32, y_ley as f32);
            salida.push(el);
        }
        y_ley += alto as f64 + 12.0;
    }
    let mut pz = Pizarra::nueva();
    for t in &extra_leyenda {
        let (_, alto) = pz.texto(t, x_ley, y_ley, 15.0, pizarra::GRIS, pizarra::Alinear::Izquierda);
        y_ley += alto as f64 + 8.0;
    }
    salida.append(&mut pz.el);
    if !p.titulo.is_empty() {
        let tam = 26.0;
        // Con x³, π y θ, no «x^3», «pi» ni la t de las polares.
        let titulo = pizarra::a_pizarra(&p.titulo, p.curvas.iter().any(|c| matches!(c, Curva::Polar { .. })));
        let (aw, ah) = medir(&titulo, tam);
        salida.push(ecuacion::elemento_texto(&titulo, (cx + cw / 2.0) as f32 - aw / 2.0, cy as f32, (aw, ah), tam, false, &estilo));
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::elemento::Figura;

    fn validar_(v: Value) -> Result<Funciones, ErrorHerramienta> {
        let Value::Object(m) = v else { panic!() };
        validar(&m, &mut Vec::new())
    }

    fn con_avisos(v: Value) -> (Funciones, String) {
        let Value::Object(m) = v else { panic!() };
        let mut avisos = Vec::new();
        let p = validar(&m, &mut avisos).unwrap();
        (p, avisos.join(" | "))
    }

    fn curvas_largas(els: &[Elemento], n: usize) -> usize {
        els.iter().filter(|e| matches!(&e.figura, Figura::Linea { puntos } if puntos.len() > n)).count()
    }

    #[test]
    fn dos_curvas_sobre_el_instrumento() {
        let p = validar_(json!({"funciones": ["x^2", "y = 2x + 1"], "x_desde": -3, "x_hasta": 3})).unwrap();
        assert_eq!(p.formulas().len(), 2);
        let els = piezas(&p, (60.0, 40.0, 680.0, 520.0));
        assert!(matches!(els[0].figura, Figura::Instrumento(_)));
        let curvas: Vec<_> = els.iter().filter(|e| matches!(&e.figura, Figura::Linea { puntos } if puntos.len() > 50)).collect();
        assert_eq!(curvas.len(), 2);
        // El (2, 4) de la parabola cae donde el instrumento dice.
        let i = *ins::de(&els[0]).unwrap();
        let p24 = ins::punto(&els[0], &i, 2.0, 4.0, 0.0);
        let pasa = match &curvas[0].figura {
            Figura::Linea { puntos } => puntos.iter().any(|q| (q.x - p24.x).abs() < 2.0 && (q.y - p24.y).abs() < 2.0),
            _ => false,
        };
        assert!(pasa, "la curva no pasa por (2, 4)");
        // Lo lee el lienzo, todo editable y en un grupo.
        let mut f = Fabrica::nueva((0.0, 0.0), 5);
        elementos(&mut f, &p);
        let l = pixpin_motor2d::excalidraw::leer(&crate::lienzo::documento(f.salida.clone())).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0);
        assert_eq!(f.salida[0]["type"], "pixpin-axes");
        let leidos = l.elementos();
        assert!(leidos.iter().all(|e| e.grupos.last() == leidos[0].grupos.last() && !e.grupos.is_empty()));
    }

    #[test]
    fn un_polo_corta_la_curva_y_da_su_asintota() {
        let (p, avisos) = con_avisos(json!({"funciones": ["1/x"], "x_desde": -4, "x_hasta": 4}));
        let els = piezas(&p, (0.0, 0.0, 800.0, 600.0));
        let ramas = els
            .iter()
            .filter(|e| e.estilo == pixpin_motor2d::EstiloTrazo::Solido && matches!(&e.figura, Figura::Linea { puntos } if puntos.len() > 20))
            .count();
        assert_eq!(ramas, 2, "1/x son dos ramas");
        assert!(p.asintotas.contains(&Asintota::Vertical(0.0)), "{:?}", p.asintotas);
        assert!(p.asintotas.contains(&Asintota::Recta(0.0, 0.0)), "y la horizontal: {:?}", p.asintotas);
        assert!(avisos.contains("asintota vertical x = 0"), "{avisos}");
        assert!(els.iter().any(|e| e.estilo == pixpin_motor2d::EstiloTrazo::Discontinuo));
    }

    #[test]
    fn marca_las_raices_de_x2_menos_4() {
        let (p, avisos) = con_avisos(json!({"funciones": ["x^2-4"], "marcar": ["raices", "extremos"]}));
        let raices: Vec<f64> = p.marcas.iter().filter(|m| m.tipo == TipoMarca::Raiz).map(|m| m.x).collect();
        assert_eq!(raices, vec![-2.0, 2.0]);
        assert!(p.marcas.iter().any(|m| m.tipo == TipoMarca::Minimo && m.x == 0.0 && m.y == -4.0));
        assert!(avisos.contains("x = -2, x = 2"), "{avisos}");
        // El dominio solo: el de siempre, con las raices dentro.
        assert!(p.x_desde <= -3.0 && p.x_hasta >= 3.0, "{} {}", p.x_desde, p.x_hasta);
        let els = piezas(&p, (60.0, 40.0, 680.0, 520.0));
        // Los rotulos van aparte del punto (colocados sin pisarse).
        let puntos: Vec<String> = els
            .iter()
            .filter_map(|e| match &e.figura {
                Figura::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect();
        assert!(puntos.contains(&"(-2, 0)".to_string()) && puntos.contains(&"(0, -4)".to_string()), "{puntos:?}");
    }

    fn caja_de(e: &Elemento) -> crate::rotulos::Caja {
        let (x0, y0, x1, y1) = e.caja();
        [x0 as f64, y0 as f64, (x1 - x0) as f64, (y1 - y0) as f64]
    }

    /// Banco del 15-oct: las coordenadas de las raices de x³ - 3x salian
    /// unas encima de otras.
    #[test]
    fn los_rotulos_de_los_puntos_no_se_pisan() {
        let p = validar_(json!({"funciones": ["x^3 - 3x"], "x_desde": -3, "x_hasta": 3, "marcar": ["raices", "extremos"], "tangente": {"x": 2}})).unwrap();
        let els = piezas(&p, (60.0, 40.0, 680.0, 520.0));
        let rotulos: Vec<crate::rotulos::Caja> = els
            .iter()
            .filter(|e| matches!(&e.figura, Figura::Texto { texto, .. } if texto.starts_with('(')))
            .map(caja_de)
            .collect();
        assert_eq!(rotulos.len(), 6, "3 raices, 2 extremos y el punto de la tangente");
        for i in 0..rotulos.len() {
            for j in i + 1..rotulos.len() {
                assert!(crate::rotulos::solape(&rotulos[i], &rotulos[j]) < 1.0, "{:?} pisa {:?}", rotulos[i], rotulos[j]);
            }
        }
    }

    /// Banco del 15-oct: «sin x entre 0 y π» salia de -π a π (el
    /// instrumento es simetrico) y la tangente de x³ - 3x en una tira.
    #[test]
    fn el_rango_pedido_se_respeta_y_la_grafica_no_sale_en_un_hilo() {
        let p = validar_(json!({"funciones": ["sin(x)"], "x_desde": 0, "x_hasta": "pi", "sombrear": {"entre": [0]}, "titulo": "y = sin x en [0, pi]"})).unwrap();
        let els = piezas(&p, (60.0, 40.0, 680.0, 520.0));
        assert!(!els.iter().any(|e| matches!(e.figura, Figura::Instrumento(_))), "con ejes propios");
        let numeros: Vec<String> = els
            .iter()
            .filter_map(|e| match &e.figura {
                Figura::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect();
        // Ninguna marca negativa en el eje x.
        assert!(!numeros.iter().any(|t| t.parse::<f64>().is_ok_and(|v| v < -0.5)), "{numeros:?}");
        assert!(numeros.iter().any(|t| t == "y = sin x en [0, π]"), "titulo compuesto: {numeros:?}");
        // La curva llena casi todo el ancho.
        let ancho_de = |els: &[Elemento]| {
            els.iter()
                .filter(|e| matches!(&e.figura, Figura::Linea { puntos } if puntos.len() > 100))
                .map(caja_de)
                .map(|c| c[2])
                .fold(0.0, f64::max)
        };
        assert!(ancho_de(&els) > 400.0, "{}", ancho_de(&els));
        let p = validar_(json!({"funciones": ["x^3 - 3x"], "x_desde": -3, "x_hasta": 3, "tangente": {"x": 2}})).unwrap();
        let els = piezas(&p, (60.0, 40.0, 680.0, 520.0));
        assert!(ancho_de(&els) > 350.0, "la cubica no en un hilo: {}", ancho_de(&els));
        // Una ventana simetrica sigue en el instrumento.
        let p = validar_(json!({"funciones": ["x^2", "2x+1"], "x_desde": -3, "x_hasta": 3})).unwrap();
        assert!(matches!(piezas(&p, (60.0, 40.0, 680.0, 520.0))[0].figura, Figura::Instrumento(_)));
    }

    #[test]
    fn polar_y_parametrica() {
        let p = validar_(json!({"funciones": ["r = 1 + cos θ"]})).unwrap();
        assert!(matches!(p.curvas[0], Curva::Polar { .. }));
        assert_eq!(p.curvas[0].rotulo(), "r = 1 + cos θ");
        // La cardioide va de x = -0.25 a 2: la ventana la abarca.
        assert!(p.x_hasta >= 2.0 && p.x_desde <= -0.25, "{} {}", p.x_desde, p.x_hasta);
        let els = piezas(&p, (60.0, 40.0, 680.0, 520.0));
        assert!(curvas_largas(&els, 500) >= 1, "la cardioide entera");
        let p = validar_(json!({"funciones": [{"x": "3cos(t)", "y": "2sin(t)"}]})).unwrap();
        assert!(matches!(p.curvas[0], Curva::Parametrica { .. }));
        assert!(p.x_hasta >= 3.0 && p.y_hasta >= 2.0);
        // Escrita como texto, tambien.
        let p = validar_(json!({"funciones": ["x = 3cos(t), y = 2sin(t)"]})).unwrap();
        assert!(matches!(p.curvas[0], Curva::Parametrica { .. }));
        let p = validar_(json!({"funciones": ["(cos(t), sin(t))"], "t_desde": 0, "t_hasta": "pi"})).unwrap();
        assert!((p.t_hasta - std::f64::consts::PI).abs() < 1e-12);
        // Lo que mando el 2B: «r:» en vez de «r =», y un objeto escrito
        // como texto.
        assert!(matches!(validar_(json!({"funciones": ["r: 1 + cos(t)"]})).unwrap().curvas[0], Curva::Polar { .. }));
        let p = validar_(json!({"funciones": ["{\"x\":\"3cos(t)\", \"y\":\"2sin(t)\"}"]})).unwrap();
        assert!(matches!(p.curvas[0], Curva::Parametrica { .. }));
        // Y dos objetos con tilde, uno por coordenada.
        let p = validar_(json!({"funciones": [{"función": "x = 3cos(t)", "t_hasta": 10}, {"función": "y = 2sin(t)"}]})).unwrap();
        assert_eq!(p.curvas.len(), 1);
        assert!(matches!(p.curvas[0], Curva::Parametrica { .. }));
        // Una a trozos no es una parametrica.
        let p = validar_(json!({"funciones": ["x^2 si x<0; x si x>=0"]})).unwrap();
        assert!(matches!(p.curvas[0], Curva::Explicita { .. }));
    }

    #[test]
    fn puntos_de_una_tabla_con_su_leyenda() {
        let p = validar_(json!({"funciones": ["2x"], "puntos": [[0, 0.1], [1, 2.2], [2, 3.9]], "nombre_puntos": "medidas"})).unwrap();
        assert_eq!(p.curvas.len(), 2);
        let els = piezas(&p, (60.0, 40.0, 680.0, 520.0));
        assert!(els.iter().filter(|e| matches!(e.figura, Figura::Punto { .. })).count() >= 3);
        assert!(els.iter().any(|e| matches!(&e.figura, Figura::Texto { texto, .. } if texto == "medidas")));
    }

    #[test]
    fn area_entre_curvas_tangente_e_intersecciones() {
        let (p, avisos) = con_avisos(
            json!({"funciones": ["x+2", "x^2"], "x_desde": -3, "x_hasta": 3, "sombrear": {"entre": [0, 1]}, "marcar": ["intersecciones"]}),
        );
        let s = p.sombra.as_ref().unwrap();
        assert_eq!((s.desde, s.hasta), (-1.0, 2.0));
        assert!((s.area - 4.5).abs() < 1e-6, "{}", s.area);
        assert!(avisos.contains("4.5"), "{avisos}");
        assert_eq!(p.marcas.iter().filter(|m| m.tipo == TipoMarca::Interseccion).count(), 2);
        let els = piezas(&p, (60.0, 40.0, 680.0, 520.0));
        assert!(els.iter().any(|e| matches!(e.figura, Figura::Region { .. })));
        let (p, avisos) = con_avisos(json!({"funciones": ["x^3-3x"], "tangente": {"curva": 0, "x": 2}}));
        let t = p.tangente.unwrap();
        assert_eq!((t.x, t.y, t.m), (2.0, 2.0, 9.0));
        assert!(avisos.contains("y = 9x - 16"), "{avisos}");
    }

    #[test]
    fn dominios_y_formulas_malas_se_rechazan() {
        for (v, motivo) in [
            (json!({"funciones": ["x^2"], "x_desde": 5, "x_hasta": -5}), "al reves"),
            (json!({"funciones": ["x^2"], "x_desde": 1, "x_hasta": 1}), "al reves"),
            (json!({"funciones": ["x^2"], "x_desde": 0}), "los dos limites"),
            (json!({"funciones": ["x^2"], "x_desde": -1e9, "x_hasta": 1}), "entre"),
            (json!({"funciones": ["sqrt(x)"], "x_desde": -5, "x_hasta": -1}), "valores reales"),
            (json!({"funciones": ["x +* )"]}), "no se entiende"),
            (json!({"funciones": ["x + t"]}), "parametrica"),
            (json!({"funciones": ["x + z"]}), "no se entiende"),
            (json!({"funciones": []}), "vacia"),
            (json!({}), "falta"),
            (json!({"funciones": ["x"], "y_desde": 3, "y_hasta": 1}), "al reves"),
            (json!({"funciones": ["x", "x", "x", "x", "x", "x", "x"]}), "como mucho"),
            (json!({"funciones": ["x"], "x_desde": "mucho", "x_hasta": 1}), "no es un numero"),
            // Negativos de lo nuevo.
            (json!({"funciones": [{"x": "cos(t)"}]}), "falta «y»"),
            (json!({"funciones": [{"r": "sqrt(-1-cos(t)-1)"}]}), "valores reales"),
            (json!({"funciones": ["x^2"], "t_desde": 5, "t_hasta": 1}), "al reves"),
            (json!({"funciones": ["1/x"], "x_desde": -1, "x_hasta": 1, "sombrear": {"entre": [0]}}), "no es finita"),
            (json!({"funciones": ["abs(x)"], "tangente": {"x": 0}}), "no es derivable"),
            (json!({"funciones": ["1/x"], "tangente": {"x": 0}}), "no existe"),
            (json!({"funciones": ["x"], "tangente": {"curva": 3, "x": 0}}), "no hay curva 3"),
            (json!({"funciones": [{"r": "1"}], "marcar": ["raices"]}), "y = f(x)"),
            (json!({"funciones": ["x"], "puntos": [[1, 2, 3]]}), "[x, y]"),
        ] {
            let e = validar_(v.clone()).unwrap_err().to_string();
            assert!(e.contains(motivo), "{v}: {e}");
        }
    }

    #[test]
    fn la_ventana_se_ajusta_y_no_deja_la_x_en_un_hilo() {
        let mut avisos = Vec::new();
        let Value::Object(m) = json!({"funciones": ["x^3"], "x_desde": -10, "x_hasta": 10}) else { panic!() };
        let p = validar(&m, &mut avisos).unwrap();
        assert!(p.y_hasta <= 40.0 + 1e-9, "{}", p.y_hasta);
        assert_eq!(avisos.len(), 1);
        let p = validar_(json!({"funciones": ["sin(x)"], "x_desde": -6.28, "x_hasta": 6.28})).unwrap();
        assert!(p.y_desde < -0.9 && p.y_hasta > 0.9 && p.y_hasta < 2.0);
        // Sin dominio, una periodica sale con dos vueltas.
        let p = validar_(json!({"funciones": ["sin(x)"]})).unwrap();
        assert!((p.x_hasta - 2.0 * std::f64::consts::PI).abs() < 1e-9, "{}", p.x_hasta);
    }
}
