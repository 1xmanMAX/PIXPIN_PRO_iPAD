//! **Diagramas de fisica** (`diagrama_fisica`): plantillas con variables.
//!
//! Regla del usuario: un solo componente con variables, no una herramienta
//! a mano por diagrama. Aqui hay **una tabla** ([`PLANTILLAS`]): cada fila es
//! un tipo de diagrama (vectores, cuerpo libre, plano inclinado, tiro
//! parabolico, MRU/MRUV, circuito, pendulo, maquina de Atwood) con sus
//! parametros —nombre, unidad, valor por omision y **rango fisico**— y la
//! funcion que calcula y dibuja. De la misma tabla salen:
//!
//! - el esquema y la descripcion que ve el modelo (`herramienta`): la lista
//!   de tipos y la firma de cada uno, `plano_inclinado(angulo°, masa kg=10…)`;
//! - la validacion: lo que falta, lo que no es un numero y lo **fisicamente
//!   imposible** (una masa negativa, un plano de 95°, una resistencia de 0 Ω)
//!   se devuelve al modelo con el motivo, sin dibujar nada;
//! - los resultados: las cuentas (la altura maxima, el alcance, la
//!   aceleracion, la resistencia equivalente) las hace **Rust**, nunca el
//!   modelo, y vuelven al modelo como aviso para que las diga tal cual.
//!
//! Anadir un diagrama es anadir una fila y su funcion. Se dibuja con la
//! pizarra (`pizarra.rs`): elementos del motor del lienzo, editables.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::herramientas::{ErrorHerramienta, Herramienta};
use crate::lienzo::Fabrica;
use crate::pizarra::{self, Alinear, Ejes, Pizarra, numero};

pub const NOMBRE: &str = "diagrama_fisica";
/// La gravedad por omision (m/s²).
pub const G: f64 = 9.8;
/// Cuantas fuerzas, vectores o resistencias como mucho.
pub const MAX_LISTA: usize = 8;

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

/// De que clase es un parametro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clase {
    Numero,
    /// Una lista de numeros (las resistencias).
    Lista,
    /// Una lista de vectores con nombre, modulo y angulo.
    Vectores,
    /// Una palabra de una lista cerrada (en `ayuda`, separadas por `|`).
    Opcion,
}

/// Un parametro de una plantilla.
#[derive(Debug, Clone, Copy)]
pub struct Parametro {
    pub nombre: &'static str,
    pub clase: Clase,
    pub unidad: &'static str,
    /// `None`: obligatorio.
    pub por_omision: Option<f64>,
    /// El rango fisico; `abierto`: el minimo no se alcanza (> en vez de >=).
    pub min: f64,
    pub max: f64,
    pub abierto: bool,
    /// Para las opciones, los valores validos separados por `|`.
    pub ayuda: &'static str,
}

const fn num(nombre: &'static str, unidad: &'static str, por_omision: Option<f64>, min: f64, max: f64, abierto: bool) -> Parametro {
    Parametro { nombre, clase: Clase::Numero, unidad, por_omision, min, max, abierto, ayuda: "" }
}

/// Lo que dibuja y calcula una plantilla: con `None` solo calcula (para
/// validar); devuelve los resultados para el modelo, o el motivo fisico de
/// que no se pueda.
type Hacer = fn(&Valores, Option<&mut Pizarra>) -> Result<Vec<String>, String>;

/// Una fila de la tabla.
pub struct Plantilla {
    pub tipo: &'static str,
    pub ayuda: &'static str,
    pub parametros: &'static [Parametro],
    pub hacer: Hacer,
}

const GRAVEDAD: Parametro = num("g", "m/s²", Some(G), 0.0, 100.0, true);
const MASA: Parametro = num("masa", "kg", Some(10.0), 0.0, 1e6, true);
const VECTORES: Parametro = Parametro {
    nombre: "vectores",
    clase: Clase::Vectores,
    unidad: "",
    por_omision: None,
    min: 0.0,
    max: 1e6,
    abierto: false,
    ayuda: "[{nombre, modulo, angulo°}]",
};
const FUERZAS: Parametro = Parametro { nombre: "fuerzas", ayuda: "[{nombre, modulo N, angulo°}]", ..VECTORES };

/// **La tabla de plantillas.**
pub const PLANTILLAS: &[Plantilla] = &[
    Plantilla {
        tipo: "vectores",
        ayuda: "suma de vectores (paralelogramo o poligono)",
        parametros: &[
            VECTORES,
            Parametro { nombre: "metodo", clase: Clase::Opcion, unidad: "", por_omision: None, min: 0.0, max: 0.0, abierto: false, ayuda: "paralelogramo|poligono" },
        ],
        hacer: vectores,
    },
    Plantilla {
        tipo: "cuerpo_libre",
        ayuda: "diagrama de cuerpo libre de un bloque",
        parametros: &[FUERZAS, Parametro { por_omision: Some(0.0), abierto: false, ..MASA }],
        hacer: cuerpo_libre,
    },
    Plantilla {
        tipo: "plano_inclinado",
        ayuda: "bloque en un plano inclinado con el peso descompuesto y rozamiento",
        parametros: &[num("angulo", "°", None, 0.0, 90.0, true), MASA, num("mu", "", Some(0.0), 0.0, 2.0, false), GRAVEDAD],
        hacer: plano_inclinado,
    },
    Plantilla {
        tipo: "tiro_parabolico",
        ayuda: "trayectoria, altura maxima y alcance",
        parametros: &[num("v0", "m/s", None, 0.0, 2000.0, true), num("angulo", "°", None, 0.0, 90.0, true), num("altura", "m", Some(0.0), 0.0, 1e5, false), GRAVEDAD],
        hacer: tiro_parabolico,
    },
    Plantilla {
        tipo: "mru",
        ayuda: "graficas x-t, v-t y a-t a velocidad constante",
        parametros: &[num("x0", "m", Some(0.0), -1e6, 1e6, false), num("v", "m/s", None, -1e5, 1e5, false), num("t", "s", Some(10.0), 0.0, 1e5, true)],
        hacer: cinematica,
    },
    Plantilla {
        tipo: "mruv",
        ayuda: "graficas x-t, v-t y a-t con aceleracion constante",
        parametros: &[
            num("x0", "m", Some(0.0), -1e6, 1e6, false),
            num("v0", "m/s", Some(0.0), -1e5, 1e5, false),
            num("a", "m/s²", None, -1e4, 1e4, false),
            num("t", "s", Some(10.0), 0.0, 1e5, true),
        ],
        hacer: cinematica,
    },
    Plantilla {
        tipo: "circuito",
        ayuda: "circuito con fuente y resistencias en serie o en paralelo",
        parametros: &[
            num("voltaje", "V", None, 0.0, 1e6, true),
            Parametro { nombre: "resistencias", clase: Clase::Lista, unidad: "Ω", por_omision: None, min: 0.0, max: 1e9, abierto: true, ayuda: "[R1, R2, ...]" },
            Parametro { nombre: "conexion", clase: Clase::Opcion, unidad: "", por_omision: None, min: 0.0, max: 0.0, abierto: false, ayuda: "serie|paralelo" },
        ],
        hacer: circuito,
    },
    Plantilla {
        tipo: "pendulo",
        ayuda: "pendulo simple con sus fuerzas y su periodo",
        parametros: &[num("longitud", "m", None, 0.0, 1e4, true), num("angulo", "°", Some(20.0), 0.0, 90.0, true), Parametro { por_omision: Some(1.0), ..MASA }, GRAVEDAD],
        hacer: pendulo,
    },
    Plantilla {
        tipo: "maquina_atwood",
        ayuda: "dos masas colgando de una polea",
        parametros: &[num("m1", "kg", None, 0.0, 1e6, true), num("m2", "kg", None, 0.0, 1e6, true), GRAVEDAD],
        hacer: atwood,
    },
];

/// Otros nombres para los parametros (los modelos pequenos mezclan ingles y
/// espanol: `voltage`, medido con el 2B el 14-oct).
const ALIAS: &[(&str, &str)] = &[
    ("voltage", "voltaje"),
    ("v", "voltaje"),
    ("tension", "voltaje"),
    ("fem", "voltaje"),
    ("resistances", "resistencias"),
    ("resistencia", "resistencias"),
    ("r", "resistencias"),
    ("connection", "conexion"),
    ("tipo_conexion", "conexion"),
    ("configuracion", "conexion"),
    ("velocidad", "v0"),
    ("velocidad_inicial", "v0"),
    ("vo", "v0"),
    ("v_0", "v0"),
    ("velocidad", "v"),
    ("aceleracion", "a"),
    ("tiempo", "t"),
    ("angle", "angulo"),
    ("theta", "angulo"),
    ("inclinacion", "angulo"),
    ("mass", "masa"),
    ("m", "masa"),
    ("coeficiente", "mu"),
    ("rozamiento", "mu"),
    ("friccion", "mu"),
    ("μ", "mu"),
    ("l", "longitud"),
    ("length", "longitud"),
    ("forces", "fuerzas"),
    ("vectors", "vectores"),
    ("altura_inicial", "altura"),
    ("h", "altura"),
    ("h0", "altura"),
];

/// La plantilla de un tipo (con alias: «mrua» es «mruv», «fuerzas» es
/// «cuerpo_libre»...).
pub fn plantilla(tipo: &str) -> Option<&'static Plantilla> {
    let t = crate::acciones::normalizar(tipo).replace([' ', '-'], "_");
    let t = match t.as_str() {
        "mrua" | "mruv" | "movimiento_uniformemente_acelerado" | "mua" => "mruv",
        "mu" | "movimiento_rectilineo_uniforme" => "mru",
        "fuerzas" | "dcl" | "diagrama_de_cuerpo_libre" | "cuerpo" => "cuerpo_libre",
        "plano" | "rampa" => "plano_inclinado",
        "tiro" | "proyectil" | "parabolico" | "lanzamiento" => "tiro_parabolico",
        "suma_de_vectores" | "vector" => "vectores",
        "electrico" | "circuito_electrico" | "resistencias" => "circuito",
        "polea" | "atwood" => "maquina_atwood",
        otro => otro,
    };
    PLANTILLAS.iter().find(|p| p.tipo == t)
}

/// La firma compacta de una plantilla para el modelo:
/// `tipo(param unidad=omision, ...)`. Corta a proposito: va en cada vuelta.
fn firma(p: &Plantilla) -> String {
    let ps: Vec<String> = p
        .parametros
        .iter()
        .filter(|q| q.nombre != "g")
        .map(|q| {
            let mut s = q.nombre.to_string();
            match q.clase {
                Clase::Opcion => s.push_str(&format!(":{}", q.ayuda)),
                Clase::Lista => s.push_str(&format!(":[{}]", q.unidad)),
                Clase::Vectores => s.push_str(":[{nombre,modulo,angulo}]"),
                Clase::Numero if !q.unidad.is_empty() => s.push_str(&format!(" {}", q.unidad)),
                Clase::Numero => {}
            }
            if let Some(v) = q.por_omision {
                s.push_str(&format!("={}", numero(v)));
            }
            s
        })
        .collect();
    format!("{}({})", p.tipo, ps.join(", "))
}

pub fn herramienta() -> Herramienta {
    let tipos: Vec<&str> = PLANTILLAS.iter().map(|p| p.tipo).collect();
    let firmas: Vec<String> = PLANTILLAS.iter().map(firma).collect();
    Herramienta {
        nombre: NOMBRE.into(),
        descripcion: format!(
            "Diagrama de fisica de una plantilla; la app calcula los resultados. Angulos en grados (0 derecha, 90 arriba); lo que lleva = se puede omitir; un bloque en un plano (tambien su cuerpo libre) es plano_inclinado. {}. \
             Ejemplo: {{\"tipo\":\"plano_inclinado\",\"parametros\":{{\"angulo\":30,\"masa\":5,\"mu\":0.2}}}}",
            firmas.join("; ")
        ),
        parametros: json!({
            "type": "object",
            "required": ["tipo", "parametros"],
            "properties": {
                "tipo": {"type": "string", "enum": tipos},
                "parametros": {"type": "object"},
                "titulo": {"type": "string"}
            }
        }),
    }
}

/// Un vector o una fuerza.
#[derive(Debug, Clone, PartialEq)]
pub struct Vector {
    pub nombre: String,
    pub modulo: f64,
    /// En grados, desde el eje x, antihorario.
    pub angulo: f64,
}

impl Vector {
    fn x(&self) -> f64 {
        self.modulo * self.angulo.to_radians().cos()
    }
    fn y(&self) -> f64 {
        self.modulo * self.angulo.to_radians().sin()
    }
}

/// Los valores ya comprobados.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Valores {
    pub numeros: BTreeMap<String, f64>,
    pub listas: BTreeMap<String, Vec<f64>>,
    pub vectores: Vec<Vector>,
    pub opciones: BTreeMap<String, String>,
}

impl Valores {
    fn n(&self, k: &str) -> f64 {
        self.numeros.get(k).copied().unwrap_or(0.0)
    }
    fn tiene(&self, k: &str) -> bool {
        self.numeros.contains_key(k)
    }
}

/// Lo que se pide, ya comprobado y calculado.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagrama {
    pub tipo: String,
    pub valores: Valores,
    pub titulo: String,
    pub resultados: Vec<String>,
}

fn un_numero(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            // «20 m/s», «30°», «0,2»: el numero de delante.
            let t: String = s.trim().replace(',', ".").chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e')).collect();
            t.parse().ok()
        }
        _ => None,
    }
    .filter(|v: &f64| v.is_finite())
}

fn vectores_de(v: &Value, q: &Parametro) -> Result<Vec<Vector>, String> {
    let Value::Array(l) = v else { return Err(format!("«{}» es una lista {}", q.nombre, q.ayuda)) };
    if l.is_empty() || l.len() > MAX_LISTA {
        return Err(format!("«{}» lleva de 1 a {MAX_LISTA}", q.nombre));
    }
    let letras = ["A", "B", "C", "D", "E", "F", "G", "H"];
    let mut salida = Vec::new();
    for (k, x) in l.iter().enumerate() {
        let (nombre, modulo, angulo) = match x {
            Value::Object(o) => (
                o.get("nombre").and_then(Value::as_str).map(str::to_string),
                ["modulo", "valor", "magnitud", "fuerza", "f"].iter().find_map(|k| o.get(*k)).and_then(un_numero),
                ["angulo", "direccion", "grados"].iter().find_map(|k| o.get(*k)).and_then(un_numero),
            ),
            Value::Array(a) if a.len() == 2 => (None, un_numero(&a[0]), un_numero(&a[1])),
            Value::Array(a) if a.len() == 3 => (a[0].as_str().map(str::to_string), un_numero(&a[1]), un_numero(&a[2])),
            _ => (None, None, None),
        };
        let modulo = modulo.ok_or_else(|| format!("el {} de «{}» no tiene «modulo»", k + 1, q.nombre))?;
        let angulo = angulo.ok_or_else(|| format!("el {} de «{}» no tiene «angulo» (en grados)", k + 1, q.nombre))?;
        if !(0.0..=q.max).contains(&modulo) {
            return Err(format!("el modulo {} de «{}» no tiene sentido: va de 0 a {}", numero(modulo), q.nombre, numero(q.max)));
        }
        let nombre = nombre.map(|n| n.trim().chars().take(12).collect::<String>()).filter(|n| !n.is_empty());
        salida.push(Vector { nombre: nombre.unwrap_or_else(|| letras[k].to_string()), modulo, angulo: angulo.rem_euclid(360.0) });
    }
    Ok(salida)
}

pub fn validar(args: &Map<String, Value>, avisos: &mut Vec<String>) -> Result<Diagrama, ErrorHerramienta> {
    let tipo_txt = args.get("tipo").and_then(Value::as_str).unwrap_or("").trim();
    let tipos: Vec<&str> = PLANTILLAS.iter().map(|p| p.tipo).collect();
    if tipo_txt.is_empty() {
        return Err(mal(format!("falta «tipo»: {}", tipos.join(", "))));
    }
    let p = plantilla(tipo_txt).ok_or_else(|| mal(format!("no hay plantilla «{tipo_txt}»; las que hay: {}", tipos.join(", "))))?;
    // Los parametros, dentro de «parametros» o sueltos al lado del tipo.
    let mut dados: Map<String, Value> = args
        .iter()
        .filter(|(k, _)| !matches!(k.as_str(), "tipo" | "parametros" | "titulo"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if let Some(Value::Object(o)) = args.get("parametros") {
        dados.extend(o.clone());
    }
    let dados: Map<String, Value> = dados.into_iter().map(|(k, v)| (crate::acciones::normalizar(&k).replace(' ', "_"), v)).collect();
    // «El cuerpo libre de un bloque en un plano inclinado» con el angulo del
    // plano y sin fuerzas sueltas es la plantilla del plano (el 2B pedia
    // `cuerpo_libre` con `angulo` y `mu`, medido el 14-oct).
    let p = if p.tipo == "cuerpo_libre" && dados.contains_key("angulo") && !dados.contains_key("fuerzas") && !dados.contains_key("vectores") {
        avisos.push("con el angulo de un plano y sin fuerzas sueltas, se dibuja el plano inclinado".into());
        plantilla("plano_inclinado").expect("esta en la tabla")
    } else {
        p
    };
    // Los nombres de otros (en ingles, con otra palabra): al de la tabla,
    // si la plantilla lo tiene y no vino ya con su nombre.
    let nombres: Vec<&str> = p.parametros.iter().map(|q| q.nombre).collect();
    let mut dados = dados;
    for (alias, nombre) in ALIAS {
        if nombres.contains(nombre)
            && !dados.contains_key(*nombre)
            && let Some(v) = dados.remove(*alias)
        {
            dados.insert(nombre.to_string(), v);
        }
    }
    let mut val = Valores::default();
    for q in p.parametros {
        let v = dados.get(q.nombre).filter(|v| !v.is_null());
        match q.clase {
            Clase::Numero => {
                let x = match v {
                    Some(v) => un_numero(v).ok_or_else(|| mal(format!("«{}» tiene que ser un numero (en {})", q.nombre, q.unidad)))?,
                    None => q.por_omision.ok_or_else(|| mal(format!("falta «{}» ({}) para {}", q.nombre, q.unidad, p.tipo)))?,
                };
                let bajo = if q.abierto { x <= q.min } else { x < q.min };
                if bajo || x > q.max {
                    return Err(mal(format!(
                        "«{}» = {} {} no tiene sentido fisico en {}: tiene que estar entre {} {} y {} {}",
                        q.nombre,
                        numero(x),
                        q.unidad,
                        p.tipo,
                        numero(q.min),
                        if q.abierto { "(sin llegar)" } else { "" },
                        numero(q.max),
                        q.unidad
                    )
                    .replace("  ", " ")));
                }
                if v.is_some() || q.por_omision.is_some() {
                    val.numeros.insert(q.nombre.into(), x);
                }
            }
            Clase::Lista => {
                let l: Vec<f64> = match v {
                    Some(Value::Array(l)) => l.iter().map(un_numero).collect::<Option<Vec<f64>>>().ok_or_else(|| mal(format!("«{}» es una lista de numeros", q.nombre)))?,
                    Some(otro) => vec![un_numero(otro).ok_or_else(|| mal(format!("«{}» es una lista de numeros", q.nombre)))?],
                    None => return Err(mal(format!("falta «{}» {}", q.nombre, q.ayuda))),
                };
                if l.is_empty() || l.len() > MAX_LISTA {
                    return Err(mal(format!("«{}» lleva de 1 a {MAX_LISTA} valores", q.nombre)));
                }
                if let Some(x) = l.iter().find(|x| (q.abierto && **x <= q.min) || **x < q.min || **x > q.max) {
                    return Err(mal(format!(
                        "una resistencia de {} {} no tiene sentido fisico: tiene que ser mayor que {} y como mucho {}",
                        numero(*x),
                        q.unidad,
                        numero(q.min),
                        numero(q.max)
                    )));
                }
                val.listas.insert(q.nombre.into(), l);
            }
            Clase::Vectores => {
                let v = v.or_else(|| dados.get("vectores")).or_else(|| dados.get("fuerzas"));
                let v = v.ok_or_else(|| mal(format!("falta «{}» {}", q.nombre, q.ayuda)))?;
                val.vectores = vectores_de(v, q).map_err(mal)?;
            }
            Clase::Opcion => {
                let validas: Vec<&str> = q.ayuda.split('|').collect();
                if let Some(v) = v {
                    let s = crate::acciones::normalizar(v.as_str().unwrap_or(""));
                    let elegida = validas
                        .iter()
                        .find(|o| s.starts_with(&o[..4.min(o.len())]))
                        .ok_or_else(|| mal(format!("«{}» es {}", q.nombre, validas.join(" o "))))?;
                    val.opciones.insert(q.nombre.into(), elegida.to_string());
                }
            }
        }
    }
    let conocidos: Vec<&str> = p.parametros.iter().map(|q| q.nombre).collect();
    let sobran: Vec<&String> = dados.keys().filter(|k| !conocidos.contains(&k.as_str()) && !matches!(k.as_str(), "vectores" | "fuerzas")).collect();
    if !sobran.is_empty() {
        avisos.push(format!(
            "no se usan {} (los de {} son: {})",
            sobran.iter().map(|s| format!("«{s}»")).collect::<Vec<_>>().join(", "),
            p.tipo,
            conocidos.join(", ")
        ));
    }
    let resultados = (p.hacer)(&val, None).map_err(mal)?;
    if !resultados.is_empty() {
        avisos.push(format!("calculado por la app: {}", resultados.join("; ")));
    }
    let titulo = args.get("titulo").and_then(Value::as_str).unwrap_or("").trim().chars().take(100).collect();
    Ok(Diagrama { tipo: p.tipo.to_string(), valores: val, titulo, resultados })
}

/// Las piezas del motor del diagrama, en el lienzo de 800x600.
pub fn piezas(d: &Diagrama) -> Vec<pixpin_motor2d::Elemento> {
    let mut pz = Pizarra::nueva();
    if let Some(p) = plantilla(&d.tipo) {
        let _ = (p.hacer)(&d.valores, Some(&mut pz));
    }
    // Los rotulos de las fuerzas, ya con todo dibujado: que no pisen el
    // bloque, el plano ni otra flecha (banco del 15-oct).
    pz.colocar_rotulos();
    if !d.titulo.is_empty() {
        // El titulo arriba de todo: lo demas, un poco mas abajo. Con μ, θ,
        // x² y no «mu», «theta», «x^2».
        pz.mover_desde(0, 0.0, 36.0);
        pz.texto(&pizarra::a_pizarra(&d.titulo, false), 400.0, 0.0, 24.0, pizarra::TINTA, Alinear::Centro);
    }
    pz.el
}

pub(crate) fn elementos(f: &mut Fabrica, d: &Diagrama) {
    let piezas = piezas(d);
    let antes = f.grupo.clone();
    f.grupo = Some(f.nombre_de_grupo("fisica"));
    f.de_motor(&piezas);
    f.grupo = antes;
}

/// Los resultados escritos en una columna, a partir de `(x, y)`.
fn columna(pz: &mut Pizarra, x: f64, mut y: f64, lineas: &[String]) {
    for l in lineas {
        let (_, h) = pz.texto(l, x, y, 16.0, pizarra::TINTA, Alinear::Izquierda);
        y += h as f64 + 6.0;
    }
}

fn con_unidad(v: f64, u: &str) -> String {
    format!("{} {u}", numero(v))
}

// ---------------------------------------------------------------------------
// Las plantillas.

fn suma(vs: &[Vector]) -> (f64, f64) {
    vs.iter().fold((0.0, 0.0), |(x, y), v| (x + v.x(), y + v.y()))
}

fn polar_de((x, y): (f64, f64)) -> (f64, f64) {
    let m = (x * x + y * y).sqrt();
    let a = if m < 1e-12 { 0.0 } else { y.atan2(x).to_degrees().rem_euclid(360.0) };
    (m, a)
}

fn vectores(v: &Valores, pz: Option<&mut Pizarra>) -> Result<Vec<String>, String> {
    let vs = &v.vectores;
    let (rx, ry) = suma(vs);
    let (r, ang) = polar_de((rx, ry));
    let resultados = vec![
        format!("Rx = {}", numero(rx)),
        format!("Ry = {}", numero(ry)),
        format!("R = {} a {}°", numero(r), numero(ang)),
    ];
    let Some(pz) = pz else { return Ok(resultados) };
    let poligono = v.opciones.get("metodo").map(String::as_str) == Some("poligono") || vs.len() != 2;
    // Los puntos que se dibujan, para la escala.
    let mut cadena = vec![(0.0, 0.0)];
    let mut punta = (0.0, 0.0);
    for x in vs {
        punta = (punta.0 + x.x(), punta.1 + x.y());
        cadena.push(punta);
    }
    let mut todos: Vec<(f64, f64)> = cadena.clone();
    todos.extend(vs.iter().map(|x| (x.x(), x.y())));
    let (x0, x1) = todos.iter().fold((0.0f64, 0.0f64), |(a, b), p| (a.min(p.0), b.max(p.0)));
    let (y0, y1) = todos.iter().fold((0.0f64, 0.0f64), |(a, b), p| (a.min(p.1), b.max(p.1)));
    let s = (460.0 / (x1 - x0).max(1e-9)).min(420.0 / (y1 - y0).max(1e-9));
    let s = if s.is_finite() { s } else { 1.0 };
    let o = (60.0 + (-x0) * s + (460.0 - (x1 - x0) * s) / 2.0, 70.0 + y1 * s + (420.0 - (y1 - y0) * s) / 2.0);
    let a_lienzo = |(x, y): (f64, f64)| (o.0 + x * s, o.1 - y * s);
    // Los ejes de referencia, finos.
    pz.discontinua(&[(o.0 - 30.0, o.1), (560.0, o.1)], pizarra::GRIS, 1.0);
    pz.discontinua(&[(o.0, o.1 + 30.0), (o.0, 40.0)], pizarra::GRIS, 1.0);
    for (k, x) in vs.iter().enumerate() {
        let c = pizarra::color_de_serie(k + 1);
        let fin = a_lienzo((x.x(), x.y()));
        pz.flecha(o, fin, c, 2.5);
        pz.rotulo_en(fin, x.angulo.to_radians(), &x.nombre, c);
    }
    if poligono {
        // Cabeza con cola: cada vector, desde la punta del anterior.
        for (k, _) in vs.iter().enumerate().skip(1) {
            let c = pizarra::color_de_serie(k + 1);
            pz.flecha_discontinua(a_lienzo(cadena[k]), a_lienzo(cadena[k + 1]), c, 2.0);
        }
    } else {
        // El paralelogramo: las paralelas a trazos.
        let (a, b) = (&vs[0], &vs[1]);
        let fin = a_lienzo((rx, ry));
        pz.discontinua(&[a_lienzo((a.x(), a.y())), fin], pizarra::GRIS, 1.5);
        pz.discontinua(&[a_lienzo((b.x(), b.y())), fin], pizarra::GRIS, 1.5);
    }
    let fin = a_lienzo((rx, ry));
    pz.flecha(o, fin, pizarra::ROJO, 3.0);
    pz.rotulo_en(fin, ang.to_radians(), "R", pizarra::ROJO);
    let mut lineas: Vec<String> = vs.iter().map(|x| format!("{} = {} a {}°", x.nombre, numero(x.modulo), numero(x.angulo))).collect();
    lineas.push(String::new());
    lineas.extend(resultados.iter().cloned());
    lineas.push(format!("({})", if poligono { "metodo del poligono" } else { "metodo del paralelogramo" }));
    columna(pz, 590.0, 80.0, &lineas);
    Ok(resultados)
}

fn cuerpo_libre(v: &Valores, pz: Option<&mut Pizarra>) -> Result<Vec<String>, String> {
    let fs = &v.vectores;
    let (fx, fy) = suma(fs);
    let (r, ang) = polar_de((fx, fy));
    let m = v.n("masa");
    let mut resultados = vec![format!("ΣFx = {} N", numero(fx)), format!("ΣFy = {} N", numero(fy)), format!("F neta = {} N a {}°", numero(r), numero(ang))];
    if m > 0.0 {
        resultados.push(format!("a = F/m = {} m/s²", numero(r / m)));
    }
    if r < 1e-9 * (1.0 + fs.iter().map(|f| f.modulo).sum::<f64>()) {
        resultados.push("las fuerzas se equilibran: el bloque esta en reposo o a velocidad constante".into());
    }
    let Some(pz) = pz else { return Ok(resultados) };
    let c = (300.0, 300.0);
    pz.rectangulo([c.0 - 45.0, c.1 - 45.0, 90.0, 90.0], pizarra::TINTA, Some(pixpin_motor2d::ColorRgba::opaco(0.93, 0.93, 0.95)), 2.0);
    if m > 0.0 {
        pz.texto(&format!("{} kg", numero(m)), c.0, c.1 - 10.0, 16.0, pizarra::TINTA, Alinear::Centro);
    }
    let mayor = fs.iter().map(|f| f.modulo).fold(0.0, f64::max).max(1e-9);
    for (k, f) in fs.iter().enumerate() {
        let largo = 60.0 + 110.0 * f.modulo / mayor;
        pz.vector(c, largo, f.angulo, &format!("{} = {} N", f.nombre, numero(f.modulo)), pizarra::color_de_serie(k + 1), false);
    }
    if r > 1e-9 {
        pz.vector(c, 60.0 + 110.0 * (r / mayor).min(1.5), ang, "F neta", pizarra::ROJO, true);
    }
    columna(pz, 560.0, 120.0, &resultados);
    Ok(resultados)
}

fn plano_inclinado(v: &Valores, pz: Option<&mut Pizarra>) -> Result<Vec<String>, String> {
    let (th, m, mu, g) = (v.n("angulo"), v.n("masa"), v.n("mu"), v.n("g"));
    let t = th.to_radians();
    let peso = m * g;
    let (px, py) = (peso * t.sin(), peso * t.cos());
    let normal = py;
    let desliza = mu < t.tan();
    let (fr, a) = if desliza { (mu * normal, g * (t.sin() - mu * t.cos())) } else { (px, 0.0) };
    let mut resultados = vec![
        format!("P = m·g = {}", con_unidad(peso, "N")),
        format!("Px = P·sen {}° = {}", numero(th), con_unidad(px, "N")),
        format!("Py = P·cos {}° = {}", numero(th), con_unidad(py, "N")),
        format!("N = Py = {}", con_unidad(normal, "N")),
    ];
    if mu > 0.0 {
        resultados.push(if desliza {
            format!("Fr = μ·N = {}", con_unidad(fr, "N"))
        } else {
            format!("Fr estatica = Px = {} (μ ≥ tan {}°: no desliza)", con_unidad(fr, "N"), numero(th))
        });
    }
    resultados.push(format!("a = {}", con_unidad(a, "m/s²")));
    let Some(pz) = pz else { return Ok(resultados) };
    // El plano: de abajo a la izquierda hacia arriba a la derecha.
    let base = (450.0f64).min(300.0 / t.tan().max(1e-6));
    let a0 = (40.0, 540.0);
    let b0 = (a0.0 + base, a0.1);
    let c0 = (a0.0 + base, a0.1 - base * t.tan());
    pz.poligono(&[a0, b0, c0], pizarra::TINTA, 2.0);
    pz.arco(a0, 50.0, 0.0, th, pizarra::TINTA, 1.5);
    pz.texto(&format!("{}°", numero(th)), a0.0 + 58.0, a0.1 - 24.0, 15.0, pizarra::TINTA, Alinear::Izquierda);
    // El bloque, a media cuesta.
    let largo = base / t.cos();
    let u = (t.cos(), -t.sin());
    let n = (-t.sin(), -t.cos());
    let lado = 64.0;
    let mpt = (a0.0 + u.0 * largo * 0.55, a0.1 + u.1 * largo * 0.55);
    let esq = |a: f64, b: f64| (mpt.0 + u.0 * a + n.0 * b, mpt.1 + u.1 * a + n.1 * b);
    pz.poligono(&[esq(-lado / 2.0, 0.0), esq(lado / 2.0, 0.0), esq(lado / 2.0, lado), esq(-lado / 2.0, lado)], pizarra::TINTA, 2.0);
    let cg = esq(0.0, lado / 2.0);
    let escala = 120.0 / peso.max(1e-9);
    // Los angulos de cada fuerza, en grados de clase.
    let abajo = 270.0;
    let cuesta_arriba = th;
    let cuesta_abajo = 180.0 + th;
    let normal_ang = 90.0 + th;
    pz.vector(cg, peso * escala, abajo, &format!("P = {}", con_unidad(peso, "N")), pizarra::ROJO, false);
    pz.vector(cg, normal * escala, normal_ang, &format!("N = {}", con_unidad(normal, "N")), pizarra::AZUL, false);
    pz.vector(cg, px * escala, cuesta_abajo, &format!("Px = {}", con_unidad(px, "N")), pizarra::NARANJA, true);
    pz.vector(cg, py * escala, normal_ang + 180.0, &format!("Py = {}", con_unidad(py, "N")), pizarra::NARANJA, true);
    if mu > 0.0 && fr > 0.0 {
        let contacto = esq(-lado / 2.0 + 6.0, 0.0);
        pz.vector(contacto, (fr * escala).max(30.0), cuesta_arriba, &format!("Fr = {}", con_unidad(fr, "N")), pizarra::VERDE, false);
    }
    let mut lineas = vec![format!("m = {} kg, μ = {}, g = {} m/s²", numero(m), numero(mu), numero(g)), String::new()];
    lineas.extend(resultados.iter().cloned());
    columna(pz, 60.0, 30.0, &lineas[..1]);
    columna(pz, 540.0, 70.0, &lineas[2..]);
    Ok(resultados)
}

fn tiro_parabolico(v: &Valores, pz: Option<&mut Pizarra>) -> Result<Vec<String>, String> {
    let (v0, th, h0, g) = (v.n("v0"), v.n("angulo"), v.n("altura"), v.n("g"));
    let t = th.to_radians();
    let (vx, vy) = (v0 * t.cos(), v0 * t.sin());
    let vx = if vx.abs() < 1e-9 { 0.0 } else { vx };
    let t_sub = vy / g;
    let h_max = h0 + vy * vy / (2.0 * g);
    let t_vuelo = (vy + (vy * vy + 2.0 * g * h0).sqrt()) / g;
    let alcance = vx * t_vuelo;
    let resultados = vec![
        format!("v0x = {} m/s, v0y = {} m/s", numero(vx), numero(vy)),
        format!("altura maxima = {} m (a los {} s)", numero(h_max), numero(t_sub)),
        format!("tiempo de vuelo = {} s", numero(t_vuelo)),
        format!("alcance = {} m", numero(alcance)),
    ];
    let Some(pz) = pz else { return Ok(resultados) };
    // Misma escala en los dos ejes: la parabola con su forma de verdad.
    let (ancho, alto) = ((alcance * 1.08).max(h_max * 0.3).max(1.0), h_max * 1.15);
    let s = (560.0 / ancho).min(380.0 / alto);
    let caja = [70.0, 70.0 + 380.0 - alto * s, ancho * s, alto * s];
    let ejes = Ejes { caja, x0: 0.0, x1: ancho, y0: 0.0, y1: alto };
    ejes.dibujar(pz, "x (m)", "y (m)");
    let y_de = |x: f64| if vx == 0.0 { f64::NAN } else { h0 + x * vy / vx - g * x * x / (2.0 * vx * vx) };
    if vx == 0.0 {
        pz.linea(&[ejes.a_lienzo(0.0, h0), ejes.a_lienzo(0.0, h_max)], pizarra::AZUL, 2.5);
    } else {
        let n = 200;
        let pts: Vec<(f64, f64)> = (0..=n).map(|k| alcance * k as f64 / n as f64).map(|x| ejes.a_lienzo(x, y_de(x).max(0.0))).collect();
        pz.linea(&pts, pizarra::AZUL, 2.5);
    }
    let x_max = vx * t_sub;
    let (hx, hy) = ejes.a_lienzo(x_max, h_max);
    pz.discontinua(&[ejes.a_lienzo(x_max, 0.0), (hx, hy)], pizarra::GRIS, 1.5);
    pz.punto(hx, hy, &format!("H = {} m", numero(h_max)), pizarra::ROJO);
    let (rx, ry) = ejes.a_lienzo(alcance, 0.0);
    pz.punto_con(rx, ry, &format!("R = {} m", numero(alcance)), pizarra::ROJO, -2.4);
    let salida = ejes.a_lienzo(0.0, h0);
    pz.vector(salida, 70.0, th, &format!("v0 = {} m/s", numero(v0)), pizarra::VERDE, false);
    pz.arco(salida, 34.0, 0.0, th, pizarra::VERDE, 1.5);
    pz.texto(&format!("{}°", numero(th)), salida.0 + 40.0, salida.1 - 22.0, 14.0, pizarra::VERDE, Alinear::Izquierda);
    columna(pz, 70.0, 480.0, &resultados);
    Ok(resultados)
}

fn cinematica(v: &Valores, pz: Option<&mut Pizarra>) -> Result<Vec<String>, String> {
    let x0 = v.n("x0");
    let v0 = if v.tiene("v") { v.n("v") } else { v.n("v0") };
    let a = v.n("a");
    let tf = v.n("t");
    let x = |t: f64| x0 + v0 * t + 0.5 * a * t * t;
    let vel = |t: f64| v0 + a * t;
    let mut resultados = vec![
        format!("x(t) = {}", polinomio(&[(x0, ""), (v0, "·t"), (a / 2.0, "·t²")])),
        format!("v(t) = {}", polinomio(&[(v0, ""), (a, "·t")])),
        format!("a los {} s: x = {} m, v = {} m/s", numero(tf), numero(x(tf)), numero(vel(tf))),
        format!("desplazamiento = {} m", numero(x(tf) - x0)),
    ];
    if a != 0.0 && v0 * a < 0.0 && -v0 / a < tf {
        resultados.push(format!("se detiene y da la vuelta en t = {} s, en x = {} m", numero(-v0 / a), numero(x(-v0 / a))));
    }
    let Some(pz) = pz else { return Ok(resultados) };
    let paneles: [(&str, &str, &dyn Fn(f64) -> f64, pixpin_motor2d::ColorRgba); 3] = [
        ("x–t", "x (m)", &x, pizarra::AZUL),
        ("v–t", "v (m/s)", &vel, pizarra::ROJO),
        ("a–t", "a (m/s²)", &|_| a, pizarra::VERDE),
    ];
    for (k, (titulo, eje, f, color)) in paneles.into_iter().enumerate() {
        let x_caja = 50.0 + 255.0 * k as f64;
        let (lo, hi) = (0..=100).map(|i| f(tf * i as f64 / 100.0)).fold((f64::MAX, f64::MIN), |(a, b), y| (a.min(y), b.max(y)));
        let ejes = Ejes::nuevos([x_caja, 130.0, 190.0, 200.0], (0.0, tf), (lo, hi));
        pz.texto(titulo, x_caja + 95.0, 70.0, 18.0, pizarra::TINTA, Alinear::Centro);
        ejes.dibujar(pz, "t", eje);
        ejes.curva(pz, f, color, 2.5);
    }
    columna(pz, 50.0, 400.0, &resultados);
    Ok(resultados)
}

/// `3 + 2·t - 0.5·t²` sin los terminos nulos ni el «+ -».
fn polinomio(terminos: &[(f64, &str)]) -> String {
    let mut s = String::new();
    for &(c, var) in terminos {
        if c == 0.0 {
            continue;
        }
        let cuerpo = if c.abs() == 1.0 && !var.is_empty() { var.trim_start_matches('·').to_string() } else { format!("{}{var}", numero(c.abs())) };
        if s.is_empty() {
            s = if c < 0.0 { format!("-{cuerpo}") } else { cuerpo };
        } else {
            s.push_str(&format!(" {} {cuerpo}", if c < 0.0 { "-" } else { "+" }));
        }
    }
    if s.is_empty() { "0".into() } else { s }
}

/// Una resistencia en zigzag de `a` a `b` (horizontal o vertical).
fn resistencia(pz: &mut Pizarra, a: (f64, f64), b: (f64, f64), color: pixpin_motor2d::ColorRgba) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let largo = (dx * dx + dy * dy).sqrt();
    let (ux, uy) = (dx / largo, dy / largo);
    let (nx, ny) = (-uy, ux);
    let zig = 50.0f64.min(largo * 0.8);
    let ini = (largo - zig) / 2.0;
    let mut pts = vec![a, (a.0 + ux * ini, a.1 + uy * ini)];
    for k in 1..=6 {
        let d = ini + zig * (k as f64 - 0.5) / 6.0;
        let lado = if k % 2 == 0 { -9.0 } else { 9.0 };
        pts.push((a.0 + ux * d + nx * lado, a.1 + uy * d + ny * lado));
    }
    pts.push((a.0 + ux * (ini + zig), a.1 + uy * (ini + zig)));
    pts.push(b);
    pz.linea(&pts, color, 2.0);
}

/// La fuente: dos rayas, la larga es el +.
fn pila(pz: &mut Pizarra, centro: (f64, f64), voltaje: f64) {
    let (x, y) = centro;
    pz.linea(&[(x - 22.0, y - 6.0), (x + 22.0, y - 6.0)], pizarra::TINTA, 2.0);
    pz.linea(&[(x - 12.0, y + 6.0), (x + 12.0, y + 6.0)], pizarra::TINTA, 4.0);
    pz.texto("+", x + 26.0, y - 20.0, 14.0, pizarra::TINTA, Alinear::Izquierda);
    pz.texto(&format!("{} V", numero(voltaje)), x - 30.0, y - 9.0, 16.0, pizarra::TINTA, Alinear::Derecha);
}

fn circuito(v: &Valores, pz: Option<&mut Pizarra>) -> Result<Vec<String>, String> {
    let volt = v.n("voltaje");
    let rs = v.listas.get("resistencias").cloned().unwrap_or_default();
    let paralelo = match v.opciones.get("conexion").map(String::as_str) {
        Some("paralelo") => true,
        Some(_) => false,
        None if rs.len() == 1 => false,
        None => return Err("falta «conexion»: serie o paralelo".into()),
    };
    let req = if paralelo { 1.0 / rs.iter().map(|r| 1.0 / r).sum::<f64>() } else { rs.iter().sum() };
    let i = volt / req;
    let mut resultados = vec![format!("R equivalente = {} Ω", numero(req)), format!("I total = {} A", numero(i)), format!("P = V·I = {} W", numero(volt * i))];
    for (k, r) in rs.iter().enumerate() {
        resultados.push(if paralelo {
            format!("R{} = {} Ω: V = {} V, I = {} A", k + 1, numero(*r), numero(volt), numero(volt / r))
        } else {
            format!("R{} = {} Ω: V = {} V, I = {} A", k + 1, numero(*r), numero(i * r), numero(i))
        });
    }
    let Some(pz) = pz else { return Ok(resultados) };
    let (x0, x1, y0, y1) = (120.0, 560.0, 110.0, 330.0);
    pila(pz, (x0, (y0 + y1) / 2.0), volt);
    // El lado de la pila, partido por ella.
    pz.linea(&[(x0, y0), (x0, (y0 + y1) / 2.0 - 6.0)], pizarra::TINTA, 2.0);
    pz.linea(&[(x0, (y0 + y1) / 2.0 + 6.0), (x0, y1)], pizarra::TINTA, 2.0);
    let n = rs.len() as f64;
    if paralelo {
        pz.linea(&[(x0, y0), (x1, y0)], pizarra::TINTA, 2.0);
        pz.linea(&[(x0, y1), (x1, y1)], pizarra::TINTA, 2.0);
        for (k, r) in rs.iter().enumerate() {
            // Cada rama a su paso; la ultima cierra el circuito por la derecha.
            let x = x0 + (x1 - x0) * (k as f64 + 1.0) / n;
            resistencia(pz, (x, y0), (x, y1), pizarra::color_de_serie(k + 1));
            pz.texto(&format!("R{} = {} Ω", k + 1, numero(*r)), x + 14.0, (y0 + y1) / 2.0 - 10.0, 15.0, pizarra::TINTA, Alinear::Izquierda);
        }
    } else {
        // Las resistencias repartidas por arriba; la derecha y abajo, cable.
        let paso = (x1 - x0) / n;
        let mut x = x0;
        for (k, r) in rs.iter().enumerate() {
            let (a, b) = (x + paso * 0.15, x + paso * 0.85);
            pz.linea(&[(x, y0), (a, y0)], pizarra::TINTA, 2.0);
            resistencia(pz, (a, y0), (b, y0), pizarra::color_de_serie(k + 1));
            pz.linea(&[(b, y0), (x + paso, y0)], pizarra::TINTA, 2.0);
            pz.texto(&format!("R{} = {} Ω", k + 1, numero(*r)), (a + b) / 2.0, y0 - 34.0, 15.0, pizarra::TINTA, Alinear::Centro);
            x += paso;
        }
        pz.linea(&[(x1, y0), (x1, y1), (x0, y1)], pizarra::TINTA, 2.0);
    }
    // El sentido de la corriente, en el cable de abajo.
    // Sale por el + (arriba) y vuelve por abajo hacia la pila.
    let x_flecha = if paralelo { x0 + (x1 - x0) / n * 0.5 } else { (x0 + x1) / 2.0 };
    pz.flecha((x_flecha + 35.0, y1), (x_flecha - 35.0, y1), pizarra::ROJO, 2.5);
    pz.texto(&format!("I = {} A", numero(i)), x_flecha, y1 + 8.0, 15.0, pizarra::ROJO, Alinear::Centro);
    let mut lineas = vec![format!("Conexion en {}", if paralelo { "paralelo" } else { "serie" })];
    lineas.extend(resultados.iter().cloned());
    columna(pz, 120.0, 380.0, &lineas);
    Ok(resultados)
}

fn pendulo(v: &Valores, pz: Option<&mut Pizarra>) -> Result<Vec<String>, String> {
    let (l, th, m, g) = (v.n("longitud"), v.n("angulo"), v.n("masa"), v.n("g"));
    let t = th.to_radians();
    let periodo = 2.0 * std::f64::consts::PI * (l / g).sqrt();
    let peso = m * g;
    let tension = peso * t.cos();
    let v_max = (2.0 * g * l * (1.0 - t.cos())).sqrt();
    let mut resultados = vec![
        format!("periodo T = 2π·√(L/g) = {} s", numero(periodo)),
        format!("P = m·g = {} N", numero(peso)),
        format!("en el extremo: tension = P·cos {}° = {} N, fuerza recuperadora = P·sen {}° = {} N", numero(th), numero(tension), numero(th), numero(peso * t.sin())),
        format!("velocidad maxima (abajo) = {} m/s", numero(v_max)),
    ];
    if th > 15.0 {
        resultados.push("el periodo es el de oscilaciones pequenas: con angulos grandes es algo mayor".into());
    }
    let Some(pz) = pz else { return Ok(resultados) };
    let pivote = (300.0, 70.0);
    let largo = 300.0;
    pz.linea(&[(pivote.0 - 50.0, pivote.1), (pivote.0 + 50.0, pivote.1)], pizarra::TINTA, 3.0);
    pz.discontinua(&[pivote, (pivote.0, pivote.1 + largo + 20.0)], pizarra::GRIS, 1.0);
    let masa = (pivote.0 + largo * t.sin(), pivote.1 + largo * t.cos());
    pz.linea(&[pivote, masa], pizarra::TINTA, 2.0);
    pz.arco(pivote, 70.0, 270.0, 270.0 + th, pizarra::TINTA, 1.5);
    pz.texto(&format!("{}°", numero(th)), pivote.0 + 18.0, pivote.1 + 76.0, 14.0, pizarra::TINTA, Alinear::Izquierda);
    pz.texto(&format!("L = {} m", numero(l)), (pivote.0 + masa.0) / 2.0 + 12.0, (pivote.1 + masa.1) / 2.0 - 20.0, 15.0, pizarra::TINTA, Alinear::Izquierda);
    // El recorrido, a trazos.
    let recorrido: Vec<(f64, f64)> = (0..=40)
        .map(|k| -t + 2.0 * t * k as f64 / 40.0)
        .map(|a| (pivote.0 + largo * a.sin(), pivote.1 + largo * a.cos()))
        .collect();
    pz.discontinua(&recorrido, pizarra::GRIS, 1.0);
    pz.elipse([masa.0 - 18.0, masa.1 - 18.0, 36.0, 36.0], pizarra::TINTA, Some(pizarra::AZUL), 2.0);
    let escala = 110.0 / peso.max(1e-9);
    pz.vector(masa, peso * escala, 270.0, &format!("P = {} N", numero(peso)), pizarra::ROJO, false);
    pz.vector(masa, tension * escala, 90.0 + th, &format!("T = {} N", numero(tension)), pizarra::AZUL, false);
    pz.vector(masa, peso * t.sin() * escala, 180.0 + th, "P·sen θ", pizarra::NARANJA, true);
    pz.vector(masa, peso * t.cos() * escala, 270.0 + th, "P·cos θ", pizarra::NARANJA, true);
    columna(pz, 470.0, 120.0, &resultados);
    Ok(resultados)
}

fn atwood(v: &Valores, pz: Option<&mut Pizarra>) -> Result<Vec<String>, String> {
    let (m1, m2, g) = (v.n("m1"), v.n("m2"), v.n("g"));
    let a = (m1 - m2).abs() * g / (m1 + m2);
    let tension = 2.0 * m1 * m2 * g / (m1 + m2);
    let resultados = vec![
        format!("a = |m1 − m2|·g/(m1 + m2) = {} m/s²", numero(a)),
        format!("T = 2·m1·m2·g/(m1 + m2) = {} N", numero(tension)),
        if m1 > m2 {
            "baja m1 y sube m2".into()
        } else if m2 > m1 {
            "baja m2 y sube m1".into()
        } else {
            "masas iguales: equilibrio".into()
        },
    ];
    let Some(pz) = pz else { return Ok(resultados) };
    let c = (300.0, 110.0);
    let r = 40.0;
    pz.linea(&[(c.0, 40.0), c], pizarra::TINTA, 3.0);
    pz.elipse([c.0 - r, c.1 - r, 2.0 * r, 2.0 * r], pizarra::TINTA, None, 2.5);
    let mayor = m1.max(m2);
    for (k, (m, lado)) in [(m1, -1.0), (m2, 1.0)].into_iter().enumerate() {
        let x = c.0 + lado * r;
        let lado_caja = 56.0 + 30.0 * (m / mayor);
        let y = 300.0 + if (m > m1.min(m2)) && m1 != m2 { 60.0 } else { 0.0 };
        pz.linea(&[(x, c.1), (x, y)], pizarra::TINTA, 2.0);
        pz.rectangulo([x - lado_caja / 2.0, y, lado_caja, lado_caja], pizarra::TINTA, Some(pixpin_motor2d::ColorRgba::opaco(0.93, 0.93, 0.95)), 2.0);
        // El nombre de la masa, que quepa en su caja (salia «m1 = 3 kg»
        // por los lados): en dos renglones y con la letra que entre.
        let (nombre, valor) = (format!("m{}", k + 1), format!("{} kg", numero(m)));
        let mut tam = 14.0f32;
        while tam > 8.0 && pizarra::medir(&valor, tam).0 as f64 > lado_caja - 10.0 {
            tam -= 1.0;
        }
        pz.texto(&nombre, x, y + lado_caja / 2.0 - tam as f64 * 1.3, tam, pizarra::TINTA, Alinear::Centro);
        pz.texto(&valor, x, y + lado_caja / 2.0, tam, pizarra::TINTA, Alinear::Centro);
        // Las fuerzas, saliendo del bloque: el peso desde abajo y la tension
        // desde arriba, por la cuerda (antes iban sueltas al lado).
        let _ = lado;
        pz.vector((x, y + lado_caja), 60.0 + 40.0 * m / mayor, 270.0, &format!("P{} = {} N", k + 1, numero(m * g)), pizarra::ROJO, false);
        pz.vector((x, y), 60.0 + 40.0 * tension / (mayor * g), 90.0, "T", pizarra::AZUL, false);
    }
    columna(pz, 470.0, 140.0, &resultados);
    Ok(resultados)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::elemento::Figura;

    fn validar_(v: Value) -> Result<(Diagrama, String), ErrorHerramienta> {
        let Value::Object(m) = v else { panic!() };
        let mut avisos = Vec::new();
        validar(&m, &mut avisos).map(|d| (d, avisos.join(" | ")))
    }

    #[test]
    fn la_descripcion_sale_de_la_tabla() {
        let h = herramienta();
        for p in PLANTILLAS {
            assert!(h.descripcion.contains(p.tipo), "{}", p.tipo);
        }
        assert!(h.descripcion.contains("plano_inclinado(angulo °, masa kg=10, mu=0)"), "{}", h.descripcion);
        assert!(h.descripcion.contains("circuito(voltaje V, resistencias:[Ω], conexion:serie|paralelo)"), "{}", h.descripcion);
        assert_eq!(h.parametros["properties"]["tipo"]["enum"].as_array().unwrap().len(), PLANTILLAS.len());
    }

    #[test]
    fn plano_inclinado_de_30_con_rozamiento() {
        let (d, avisos) = validar_(json!({"tipo": "plano_inclinado", "parametros": {"angulo": 30, "masa": 5, "mu": 0.2}})).unwrap();
        // P = 49, Px = 24.5, N = 42.435, Fr = 8.487, a = 3.203.
        assert!(avisos.contains("P = m·g = 49 N"), "{avisos}");
        assert!(avisos.contains("Px = P·sen 30° = 24.5 N"), "{avisos}");
        assert!(avisos.contains("Fr = μ·N = 8.487 N"), "{avisos}");
        assert!(avisos.contains("a = 3.203 m/s²"), "{avisos}");
        let els = piezas(&d);
        let flechas = els.iter().filter(|e| matches!(e.figura, Figura::Flecha { .. })).count();
        assert!(flechas >= 5, "P, N, Px, Py y Fr: {flechas}");
        let mut f = Fabrica::nueva((0.0, 0.0), 9);
        elementos(&mut f, &d);
        let l = pixpin_motor2d::excalidraw::leer(&crate::lienzo::documento(f.salida)).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0);
        // Con mucho rozamiento no desliza.
        let (_, avisos) = validar_(json!({"tipo": "plano", "angulo": 20, "mu": 0.9})).unwrap();
        assert!(avisos.contains("no desliza") && avisos.contains("a = 0 m/s²"), "{avisos}");
        // Lo que pidio el 2B: cuerpo libre con el angulo del plano y sin fuerzas.
        let (d, avisos) = validar_(json!({"tipo": "cuerpo_libre", "parametros": {"angulo": 30, "mu": 0.2}})).unwrap();
        assert_eq!(d.tipo, "plano_inclinado", "{avisos}");
    }

    #[test]
    fn tiro_parabolico_a_20_y_45() {
        let (d, avisos) = validar_(json!({"tipo": "tiro_parabolico", "parametros": {"v0": 20, "angulo": 45}})).unwrap();
        // H = v²sen²θ/2g = 10.204; R = v²sen2θ/g = 40.816; T = 2.886.
        assert!(avisos.contains("altura maxima = 10.204 m"), "{avisos}");
        assert!(avisos.contains("alcance = 40.816 m"), "{avisos}");
        assert!(avisos.contains("tiempo de vuelo = 2.886 s"), "{avisos}");
        let els = piezas(&d);
        assert!(els.iter().any(|e| matches!(&e.figura, Figura::Linea { puntos } if puntos.len() > 150)), "la trayectoria");
        assert!(els.iter().any(|e| matches!(&e.figura, Figura::Punto { letra, .. } if letra.contains("40.816"))));
    }

    #[test]
    fn circuitos_en_serie_y_en_paralelo() {
        let (d, avisos) = validar_(json!({"tipo": "circuito", "parametros": {"voltaje": 12, "resistencias": [6, 3], "conexion": "paralelo"}})).unwrap();
        assert!(avisos.contains("R equivalente = 2 Ω") && avisos.contains("I total = 6 A"), "{avisos}");
        assert!(piezas(&d).len() > 10);
        let (_, avisos) = validar_(json!({"tipo": "circuito", "voltaje": "12 V", "resistencias": [6, 3], "conexion": "serie"})).unwrap();
        assert!(avisos.contains("R equivalente = 9 Ω") && avisos.contains("R1 = 6 Ω: V = 8 V"), "{avisos}");
        // Los nombres en ingles del 2B (`voltage`) valen.
        let (_, avisos) = validar_(json!({"tipo": "circuito", "parametros": {"voltage": 24, "resistances": [10, 10], "connection": "paralelo"}})).unwrap();
        assert!(avisos.contains("R equivalente = 5 Ω"), "{avisos}");
    }

    #[test]
    fn vectores_cuerpo_libre_cinematica_pendulo_y_atwood() {
        let (d, avisos) = validar_(json!({"tipo": "vectores", "parametros": {"vectores": [{"nombre": "A", "modulo": 3, "angulo": 0}, {"nombre": "B", "modulo": 4, "angulo": 90}]}})).unwrap();
        assert!(avisos.contains("R = 5 a 53.13°"), "{avisos}");
        assert!(!piezas(&d).is_empty());
        let (_, avisos) = validar_(json!({"tipo": "cuerpo_libre", "parametros": {"masa": 2, "fuerzas": [["N", 19.6, 90], ["P", 19.6, 270], ["F", 10, 0]]}})).unwrap();
        assert!(avisos.contains("ΣFx = 10 N") && avisos.contains("a = F/m = 5 m/s²"), "{avisos}");
        let (d, avisos) = validar_(json!({"tipo": "mruv", "parametros": {"v0": 2, "a": 3, "t": 4}})).unwrap();
        assert!(avisos.contains("x = 32 m, v = 14 m/s"), "{avisos}");
        assert!(piezas(&d).iter().filter(|e| matches!(&e.figura, Figura::Linea { puntos } if puntos.len() > 100)).count() >= 3);
        let (_, avisos) = validar_(json!({"tipo": "mru", "parametros": {"v": 5}})).unwrap();
        assert!(avisos.contains("x = 50 m"), "{avisos}");
        let (_, avisos) = validar_(json!({"tipo": "pendulo", "parametros": {"longitud": 1}})).unwrap();
        assert!(avisos.contains("T = 2π·√(L/g) = 2.007 s"), "{avisos}");
        let (d, avisos) = validar_(json!({"tipo": "atwood", "parametros": {"m1": 3, "m2": 1}})).unwrap();
        assert!(avisos.contains("= 4.9 m/s²") && avisos.contains("= 14.7 N"), "{avisos}");
        assert!(!piezas(&d).is_empty());
    }

    #[test]
    fn lo_fisicamente_imposible_se_rechaza() {
        for (v, motivo) in [
            (json!({"tipo": "plano_inclinado", "parametros": {"angulo": 95}}), "no tiene sentido fisico"),
            (json!({"tipo": "plano_inclinado", "parametros": {"angulo": 0}}), "sin llegar"),
            (json!({"tipo": "plano_inclinado", "parametros": {"angulo": 30, "masa": -2}}), "«masa»"),
            (json!({"tipo": "plano_inclinado", "parametros": {"angulo": 30, "mu": -0.1}}), "«mu»"),
            (json!({"tipo": "plano_inclinado", "parametros": {}}), "falta «angulo»"),
            (json!({"tipo": "tiro_parabolico", "parametros": {"v0": -20, "angulo": 45}}), "«v0»"),
            (json!({"tipo": "tiro_parabolico", "parametros": {"v0": 20, "angulo": 120}}), "«angulo»"),
            (json!({"tipo": "circuito", "parametros": {"voltaje": 12, "resistencias": [6, 0], "conexion": "serie"}}), "resistencia de 0"),
            (json!({"tipo": "circuito", "parametros": {"voltaje": 12, "resistencias": [6, 3]}}), "conexion"),
            (json!({"tipo": "circuito", "parametros": {"voltaje": 12, "resistencias": [6, 3], "conexion": "triangulo"}}), "serie o paralelo"),
            (json!({"tipo": "pendulo", "parametros": {"longitud": 0}}), "«longitud»"),
            (json!({"tipo": "vectores", "parametros": {"vectores": [{"modulo": 3}]}}), "angulo"),
            (json!({"tipo": "vectores", "parametros": {"vectores": [{"modulo": -3, "angulo": 0}]}}), "no tiene sentido"),
            (json!({"tipo": "mruv", "parametros": {"a": 2, "t": -1}}), "«t»"),
            (json!({"tipo": "agujero_negro"}), "no hay plantilla"),
            (json!({}), "falta «tipo»"),
        ] {
            let e = validar_(v.clone()).unwrap_err().to_string();
            assert!(e.contains(motivo), "{v}: {e}");
        }
    }
}
