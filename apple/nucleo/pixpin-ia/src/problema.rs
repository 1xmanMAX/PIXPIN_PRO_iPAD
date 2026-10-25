//! **Un problema en limpio** (`problema_en_limpio`): de la foto de un
//! ejercicio (o de un enunciado dictado) a una hoja limpia en el lienzo.
//!
//! Pedido del usuario (14-oct-2026): que el asistente ayude a los
//! estudiantes a «reescribir una foto de un problema matematico». El modelo
//! LEE (con su vision y, de apoyo, con el OCR de Windows que la app le pega
//! al mensaje: [`pista_de_ocr`]) y aqui se COMPONE, siempre igual:
//!
//! - el **enunciado** como texto, partido en renglones;
//! - las **formulas** compuestas como en un libro (las cajas de
//!   `escribir_ecuacion`, con el LaTeX que escriba el modelo);
//! - los **datos** en lista (y las incognitas);
//! - la **figura** del problema redibujada con las figuras del lienzo
//!   (`dibujar_en_lienzo`, instrumento incluido) en su recuadro;
//! - y un recuadro **«Solución»**: vacio para el estudiante, o con los pasos
//!   si pidio resolverlo.
//!
//! **Las cuentas de los pasos las hace Rust**: cada paso puede traer su
//! `calculo` (solo numeros: `5*9.8*sin(30)`), que se evalua con el
//! interprete de formulas del lienzo; si el modelo puso otro resultado, se
//! corrige y se le avisa. Los pasos (el razonamiento) los escribe el modelo.

use serde_json::{Map, Value, json};

use pixpin_motor2d::formula;

use crate::herramientas::{ErrorHerramienta, Figura, Herramienta};
use crate::lienzo::Fabrica;
use crate::pizarra::{self, Alinear, Pizarra, numero};

pub const NOMBRE: &str = "problema_en_limpio";
pub const MAX_ENUNCIADO: usize = 1500;
pub const MAX_ECUACIONES: usize = 8;
pub const MAX_DATOS: usize = 12;
pub const MAX_PASOS: usize = 12;
/// El recuadro de la figura (en el lienzo de 800 de ancho).
pub const FIGURA: (f64, f64) = (300.0, 260.0);

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

pub fn herramienta() -> Herramienta {
    Herramienta {
        nombre: NOMBRE.into(),
        descripcion: concat!(
            "Pasa en limpio al lienzo un problema (foto o texto): enunciado tal cual, formulas LaTeX, datos, figura (elementos de dibujar_en_lienzo en 300x260) y recuadro Solucion. ",
            "Solo si piden resolverlo, pasos con cada cuenta en calculo, solo numeros (la app la hace). ",
            "Ej: {\"enunciado\":\"Un bloque de 5 kg...\",\"ecuaciones\":[\"F = m a\"],\"datos\":[\"m = 5 kg\"],\"pasos\":[{\"texto\":\"Peso\",\"calculo\":\"5*9.8\"}]}"
        )
        .into(),
        parametros: json!({
            "type": "object",
            "required": ["enunciado"],
            "properties": {
                "titulo": {"type": "string"},
                "enunciado": {"type": "string", "maxLength": MAX_ENUNCIADO},
                "ecuaciones": {"type": "array", "items": {"type": "string"}, "maxItems": MAX_ECUACIONES},
                "datos": {"type": "array", "items": {"type": "string"}, "maxItems": MAX_DATOS},
                "incognitas": {"type": "array", "items": {"type": "string"}},
                "figura": {"type": "array", "items": {"type": "object"}},
                "pasos": {"type": "array", "maxItems": MAX_PASOS, "items": {"type": "object", "properties": {
                    "texto": {"type": "string"}, "calculo": {"type": "string"}, "resultado": {"type": "number"}, "unidad": {"type": "string"}
                }}},
                "respuesta": {"type": "string"}
            }
        }),
    }
}

/// Un paso de la solucion, con su cuenta ya hecha por la app.
#[derive(Debug, Clone, PartialEq)]
pub struct Paso {
    pub texto: String,
    pub calculo: Option<String>,
    pub resultado: Option<f64>,
    pub unidad: String,
}

/// Lo que se pide, ya comprobado.
#[derive(Debug, Clone, PartialEq)]
pub struct Problema {
    pub titulo: String,
    pub enunciado: String,
    pub ecuaciones: Vec<String>,
    pub datos: Vec<String>,
    pub incognitas: Vec<String>,
    pub figura: Vec<Figura>,
    pub pasos: Vec<Paso>,
    pub respuesta: String,
}

fn textos(args: &Map<String, Value>, k: &str, max: usize) -> Result<Vec<String>, ErrorHerramienta> {
    let v: Vec<String> = match args.get(k) {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::String(s)) => s.split('\n').map(str::to_string).collect(),
        Some(Value::Array(l)) => l
            .iter()
            .map(|x| match x {
                Value::String(s) => s.clone(),
                otro => otro.to_string(),
            })
            .collect(),
        Some(_) => return Err(mal(format!("«{k}» es una lista de textos"))),
    };
    let v: Vec<String> = v.into_iter().map(|s| s.trim().trim_matches('$').trim().to_string()).filter(|s| !s.is_empty()).collect();
    if v.len() > max {
        return Err(mal(format!("como mucho {max} en «{k}»")));
    }
    if let Some(s) = v.iter().find(|s| s.chars().count() > 300) {
        return Err(mal(format!("«{}...» de «{k}» es demasiado largo", s.chars().take(30).collect::<String>())));
    }
    Ok(v)
}

/// La cuenta de un paso, hecha por la app: solo numeros, constantes (pi, e)
/// y funciones; los grados de `sin(30°)` se pasan a radianes.
pub fn calcular(c: &str) -> Result<f64, String> {
    let limpio = grados_a_radianes(&c.replace('×', "*").replace('·', "*").replace("\\cdot", "*").replace("\\times", "*"));
    let f = formula::compilar(&limpio).ok_or_else(|| format!("la cuenta «{c}» no se entiende: escribela solo con numeros, como 5*9.8*sin(30°)"))?;
    if let Some(v) = f.variables().into_iter().next() {
        return Err(format!("la cuenta «{c}» tiene la letra «{v}»: pon los numeros de los datos, no las letras"));
    }
    let v = f.en(0.0);
    if !v.is_finite() {
        return Err(format!("la cuenta «{c}» no da un numero (division por cero o raiz de un negativo)"));
    }
    Ok(v)
}

/// `sin(30°)` → `sin(30*pi/180)`: un numero seguido de `°`.
fn grados_a_radianes(s: &str) -> String {
    let mut salida = String::with_capacity(s.len());
    let car: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < car.len() {
        if car[i].is_ascii_digit() || car[i] == '.' {
            let ini = i;
            while i < car.len() && (car[i].is_ascii_digit() || car[i] == '.') {
                i += 1;
            }
            let n: String = car[ini..i].iter().collect();
            if car.get(i) == Some(&'°') {
                salida.push_str(&format!("({n}*pi/180)"));
                i += 1;
            } else {
                salida.push_str(&n);
            }
            continue;
        }
        salida.push(car[i]);
        i += 1;
    }
    salida
}

pub fn validar(args: &Map<String, Value>, avisos: &mut Vec<String>) -> Result<Problema, ErrorHerramienta> {
    let enunciado = ["enunciado", "texto", "problema"]
        .iter()
        .find_map(|k| args.get(*k).and_then(Value::as_str))
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let letras = enunciado.chars().filter(|c| c.is_alphabetic()).count();
    if letras < 8 {
        return Err(mal(
            "falta el «enunciado» (o no se lee): si la foto no tiene un problema legible, no lo inventes y dile al usuario que no se lee",
        ));
    }
    if enunciado.chars().count() > MAX_ENUNCIADO {
        return Err(mal(format!("el enunciado pasa de {MAX_ENUNCIADO} caracteres: resumelo")));
    }
    let ecuaciones = textos(args, "ecuaciones", MAX_ECUACIONES)?;
    for e in &ecuaciones {
        if e.matches('{').count() != e.matches('}').count() {
            return Err(mal(format!("las llaves de la ecuacion «{e}» no cuadran")));
        }
    }
    let datos = textos(args, "datos", MAX_DATOS)?;
    let incognitas = textos(args, "incognitas", MAX_DATOS)?;
    let figura = match args.get("figura") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(l)) if l.is_empty() => Vec::new(),
        Some(v) => {
            let mut m = Map::new();
            m.insert("elementos".into(), v.clone());
            crate::herramientas::validar_dibujo(&m, avisos).map_err(|e| mal(format!("en la «figura»: {e}")))?
        }
    };
    let mut pasos = Vec::new();
    let mut cuentas: Vec<String> = Vec::new();
    match args.get("pasos") {
        None | Some(Value::Null) => {}
        Some(Value::Array(l)) => {
            if l.len() > MAX_PASOS {
                return Err(mal(format!("como mucho {MAX_PASOS} pasos")));
            }
            for (k, p) in l.iter().enumerate() {
                let (texto, calculo, dicho, unidad) = match p {
                    Value::String(s) => (s.trim().to_string(), None, None, String::new()),
                    Value::Object(o) => (
                        o.get("texto").or(o.get("paso")).and_then(Value::as_str).unwrap_or("").trim().to_string(),
                        o.get("calculo").or(o.get("cuenta")).and_then(Value::as_str).map(str::trim).filter(|c| !c.is_empty()).map(str::to_string),
                        o.get("resultado").and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().replace(',', ".").parse().ok()))),
                        o.get("unidad").and_then(Value::as_str).unwrap_or("").trim().chars().take(16).collect(),
                    ),
                    _ => return Err(mal("cada paso es {\"texto\":...,\"calculo\":...}")),
                };
                if texto.is_empty() && calculo.is_none() {
                    continue;
                }
                let resultado = match &calculo {
                    Some(c) => {
                        let v = calcular(c).map_err(|e| mal(format!("paso {}: {e}", k + 1)))?;
                        match dicho {
                            Some(d) if (d - v).abs() > 1e-3 * v.abs().max(1.0) => {
                                cuentas.push(format!("paso {}: {c} da {}, no {} (corregido)", k + 1, numero(v), numero(d)));
                            }
                            _ => cuentas.push(format!("paso {}: {c} = {}", k + 1, numero(v))),
                        }
                        Some(v)
                    }
                    None => dicho,
                };
                pasos.push(Paso { texto, calculo, resultado, unidad });
            }
        }
        Some(_) => return Err(mal("«pasos» es una lista")),
    }
    if !cuentas.is_empty() {
        avisos.push(format!("calculado por la app: {}", cuentas.join("; ")));
    }
    Ok(Problema {
        titulo: args.get("titulo").and_then(Value::as_str).unwrap_or("").trim().chars().take(80).collect(),
        enunciado,
        ecuaciones,
        datos,
        incognitas,
        figura,
        pasos,
        respuesta: args.get("respuesta").and_then(Value::as_str).unwrap_or("").trim().chars().take(300).collect(),
    })
}

/// **Las piezas del motor** (todo menos la figura) y donde va la figura.
pub fn piezas(p: &Problema) -> (Vec<pixpin_motor2d::Elemento>, Option<[f64; 4]>) {
    let mut pz = Pizarra::nueva();
    let (x0, ancho) = (40.0, 720.0);
    let mut y = 20.0;
    let titulo = if p.titulo.is_empty() { "Problema" } else { p.titulo.as_str() };
    let (_, h) = pz.texto(titulo, x0, y, 26.0, pizarra::TINTA, Alinear::Izquierda);
    y += h as f64 + 4.0;
    pz.linea(&[(x0, y), (x0 + ancho, y)], pizarra::AZUL, 2.0);
    y += 14.0;
    let con_figura = !p.figura.is_empty();
    let ancho_texto = if con_figura { ancho - FIGURA.0 - 24.0 } else { ancho };
    let y_figura = y;
    // Una ecuacion que va compuesta debajo no se repite cruda en el texto.
    y += pz.parrafo(&sin_las_ecuaciones(&p.enunciado, &p.ecuaciones), x0, y, ancho_texto, 17.0, pizarra::TINTA) + 12.0;
    for e in &p.ecuaciones {
        let (_, h) = pz.ecuacion(e, x0 + 24.0, y, 24.0, pizarra::TINTA);
        y += h as f64 + 12.0;
    }
    if !p.datos.is_empty() || !p.incognitas.is_empty() {
        y += 4.0;
        let (_, h) = pz.texto("Datos", x0, y, 18.0, pizarra::AZUL, Alinear::Izquierda);
        y += h as f64 + 6.0;
        for d in p.datos.iter().filter(|d| !ya_esta(d, &p.ecuaciones)) {
            pz.texto("•", x0 + 6.0, y, 16.0, pizarra::GRIS, Alinear::Izquierda);
            // El simbolo compuesto (v_0 con su subindice) y el valor con su
            // unidad como texto: «20 m/s» no es una fraccion. Salvo que el
            // valor sea formula de verdad (una matriz, una raiz...).
            let h = match d.split_once('=') {
                Some((simbolo, valor)) if !simbolo.trim().is_empty() && simbolo.trim().chars().count() <= 12 => {
                    let (w, h) = pz.ecuacion(simbolo.trim(), x0 + 24.0, y, 18.0, pizarra::TINTA);
                    let x = x0 + 30.0 + w as f64;
                    let h2 = match valor_como_formula(valor) {
                        Some(f) => {
                            let (we, _) = pz.texto("=", x, y + 1.0, 17.0, pizarra::TINTA, Alinear::Izquierda);
                            pz.ecuacion(&f, x + we as f64 + 6.0, y, 18.0, pizarra::TINTA).1
                        }
                        None => pz.texto(&format!("= {}", a_texto(valor)), x, y + 1.0, 17.0, pizarra::TINTA, Alinear::Izquierda).1,
                    };
                    h.max(h2)
                }
                _ => pz.texto(&a_texto(d), x0 + 24.0, y, 17.0, pizarra::TINTA, Alinear::Izquierda).1,
            };
            y += (h as f64).max(20.0) + 6.0;
        }
        if !p.incognitas.is_empty() && !p.incognitas.iter().any(|i| es_formula(i)) {
            // Solo palabras: un parrafo, que puede ser largo.
            let (w, _) = pz.texto("Incógnita:", x0, y, 16.0, pizarra::AZUL, Alinear::Izquierda);
            let texto: Vec<String> = p.incognitas.iter().map(|i| a_texto(i)).collect();
            let alto = pz.parrafo(&texto.join(", "), x0 + w as f64 + 10.0, y, ancho - w as f64 - 10.0, 17.0, pizarra::TINTA);
            y += alto.max(20.0) + 6.0;
        } else if !p.incognitas.is_empty() {
            // Compuestas (A⁻¹ con su exponente), no el LaTeX crudo.
            let (w, _) = pz.texto("Incógnita:", x0, y, 16.0, pizarra::AZUL, Alinear::Izquierda);
            let mut x = x0 + w as f64 + 10.0;
            let mut alto: f64 = 20.0;
            for (k, inc) in p.incognitas.iter().enumerate() {
                if k > 0 {
                    let (wc, _) = pz.texto(",", x, y + 1.0, 17.0, pizarra::TINTA, Alinear::Izquierda);
                    x += wc as f64 + 8.0;
                }
                let (wi, hi) = if es_formula(inc) {
                    pz.ecuacion(inc, x, y, 18.0, pizarra::TINTA)
                } else {
                    pz.texto(&a_texto(inc), x, y + 1.0, 17.0, pizarra::TINTA, Alinear::Izquierda)
                };
                x += wi as f64;
                alto = alto.max(hi as f64);
            }
            y += alto + 6.0;
        }
    }
    let caja_figura = con_figura.then(|| {
        let x = x0 + ancho - FIGURA.0;
        pz.rectangulo([x - 8.0, y_figura - 8.0, FIGURA.0 + 16.0, FIGURA.1 + 16.0], pizarra::GRIS, None, 1.0);
        if let Some(e) = pz.el.last_mut() {
            e.estilo = pixpin_motor2d::EstiloTrazo::Discontinuo;
        }
        [x, y_figura, FIGURA.0, FIGURA.1]
    });
    if con_figura {
        y = y.max(y_figura + FIGURA.1 + 16.0);
    }
    // El recuadro de la solucion.
    y += 14.0;
    let y_caja = y;
    let (_, h) = pz.texto("Solución", x0 + 14.0, y + 10.0, 20.0, pizarra::AZUL, Alinear::Izquierda);
    y += 10.0 + h as f64 + 10.0;
    if p.pasos.is_empty() {
        y += 150.0;
    } else {
        for (k, paso) in p.pasos.iter().enumerate() {
            let mut linea = format!("{}. {}", k + 1, paso.texto);
            if let Some(r) = paso.resultado {
                let cuenta = paso.calculo.as_deref().map(|c| format!("{c} = ")).unwrap_or_default();
                let unidad = if paso.unidad.is_empty() { String::new() } else { format!(" {}", paso.unidad) };
                linea.push_str(&format!(": {cuenta}{}{unidad}", numero(r)));
            }
            y += pz.parrafo(&linea, x0 + 14.0, y, ancho - 28.0, 16.0, pizarra::TINTA) + 6.0;
        }
    }
    if !p.respuesta.is_empty() {
        y += 4.0;
        y += pz.parrafo(&format!("Respuesta: {}", p.respuesta), x0 + 14.0, y, ancho - 28.0, 17.0, pizarra::ROJO) + 6.0;
    }
    y += 10.0;
    pz.rectangulo([x0, y_caja, ancho, y - y_caja], pizarra::AZUL, None, 2.0);
    (pz.el, caja_figura)
}

/// Si un texto es formula (lleva LaTeX o exponentes/indices) y se compone.
pub fn es_formula(s: &str) -> bool {
    s.contains('\\') || s.contains('^') || s.contains('_')
}

/// **El valor de un dato, si es formula de verdad**: una matriz escrita como
/// lista (`[[2,-1,0],[1,3,2]]`, como la manda el modelo) pasa a `bmatrix`, y
/// lo que lleva fracciones, raices o entornos se compone. Lo demas (`20 m/s`)
/// sigue como texto.
pub fn valor_como_formula(valor: &str) -> Option<String> {
    let v = valor.trim().trim_matches('$').trim();
    if let Some(m) = matriz_de_lista(v) {
        return Some(m);
    }
    ["\\frac", "\\sqrt", "\\begin", "\\int", "\\sum", "\\lim", "\\vec"].iter().any(|k| v.contains(k)).then(|| v.to_string())
}

/// `[[2,-1,0],[1,3,2]]` → `\begin{bmatrix} 2 & -1 & 0 \\ 1 & 3 & 2 \end{bmatrix}`.
pub fn matriz_de_lista(s: &str) -> Option<String> {
    let t = s.trim();
    if !t.starts_with("[[") {
        return None;
    }
    let v: Value = serde_json::from_str(t).ok().or_else(|| {
        // Con letras sueltas (`[[a, b], [c, d]]`): cada celda entre comillas.
        let mut out = String::new();
        let mut celda = String::new();
        for c in t.chars() {
            if matches!(c, '[' | ']' | ',') {
                if !celda.trim().is_empty() {
                    out.push_str(&format!("\"{}\"", celda.trim().replace('"', "")));
                }
                celda.clear();
                out.push(c);
            } else {
                celda.push(c);
            }
        }
        serde_json::from_str(&out).ok()
    })?;
    let filas: Vec<Vec<String>> = v
        .as_array()?
        .iter()
        .map(|f| {
            f.as_array().map(|c| {
                c.iter()
                    .map(|x| match x {
                        Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), numero),
                        Value::String(s) => s.trim().to_string(),
                        otro => otro.to_string(),
                    })
                    .collect()
            })
        })
        .collect::<Option<_>>()?;
    if filas.is_empty() || filas.iter().any(Vec::is_empty) {
        return None;
    }
    let cuerpo: Vec<String> = filas.iter().map(|f| f.join(" & ")).collect();
    Some(format!("\\begin{{bmatrix}} {} \\end{{bmatrix}}", cuerpo.join(" \\\\ ")))
}

/// Una formula «normalizada» para comparar: sin espacios, `$` ni marcas que
/// no cambian lo que dice.
fn normal(s: &str) -> String {
    s.replace("\\left", "").replace("\\right", "").replace("\\,", "").replace("\\displaystyle", "").chars().filter(|c| !c.is_whitespace() && *c != '$').collect()
}

/// Si un dato es una de las ecuaciones (el modelo a veces la repite).
fn ya_esta(d: &str, ecuaciones: &[String]) -> bool {
    let n = normal(d);
    !n.is_empty() && ecuaciones.iter().any(|e| normal(e) == n)
}

/// **El enunciado sin las ecuaciones que van compuestas debajo**: el modelo
/// las escribia en el texto (crudas, `$x^2 + 3x = 0$`) y otra vez como
/// ecuacion, y salian dos veces. Se quita cada trozo entre `$` que sea una de
/// ellas, y cada una que aparezca tal cual; lo demas no se toca.
pub fn sin_las_ecuaciones(enunciado: &str, ecuaciones: &[String]) -> String {
    if ecuaciones.is_empty() {
        return enunciado.to_string();
    }
    let normales: Vec<String> = ecuaciones.iter().map(|e| normal(e)).filter(|e| e.chars().count() >= 3).collect();
    // Los trozos entre `$...$`.
    let mut t = String::new();
    let mut resto = enunciado;
    while let Some(a) = resto.find('$') {
        let Some(b) = resto[a + 1..].find('$').map(|b| a + 1 + b) else { break };
        let dentro = &resto[a + 1..b];
        t.push_str(&resto[..a]);
        let n = normal(dentro);
        if !normales.iter().any(|e| *e == n) {
            t.push_str(&resto[a..=b]);
        }
        resto = &resto[b + 1..];
    }
    t.push_str(resto);
    // Y escritas tal cual.
    for e in ecuaciones {
        let e = e.trim();
        if e.chars().count() >= 5 && t.contains(e) {
            t = t.replacen(e, "", 1);
        }
    }
    // Los huecos que quedan: espacios dobles, renglones vacios, « :» suelto.
    let renglones: Vec<String> = t
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ").replace(" .", ".").replace(" ,", ","))
        .filter(|l| !l.is_empty())
        .collect();
    let limpio = renglones.join("\n");
    if limpio.chars().filter(|c| c.is_alphabetic()).count() < 4 { enunciado.to_string() } else { limpio }
}

/// Un trozo de LaTeX de bolsillo como texto llano: `20\ m/s^2` → `20 m/s²`,
/// `\text{desde el suelo}` → `desde el suelo`.
pub fn a_texto(s: &str) -> String {
    let mut t = s.trim().to_string();
    for (de, a) in [
        ("\\text{", ""), ("\\mathrm{", ""), ("\\,", " "), ("\\ ", " "), ("\\;", " "), ("^{\\circ}", "°"), ("^\\circ", "°"),
        ("\\circ", "°"), ("\\cdot", "·"), ("\\times", "×"), ("\\theta", "θ"), ("\\alpha", "α"), ("\\mu", "μ"),
        ("\\Omega", "Ω"), ("\\pi", "π"), ("\\Delta", "Δ"), ("^2", "²"), ("^3", "³"), ("_0", "₀"), ("_1", "₁"), ("_2", "₂"),
        ("{", ""), ("}", ""), ("$", ""),
    ] {
        t = t.replace(de, a);
    }
    t.replace('\\', "")
}

/// Encoge y coloca unos elementos de Excalidraw dentro de `caja`.
fn encajar(els: &mut [Value], [bx, by, bw, bh]: [f64; 4]) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for e in els.iter() {
        let (x, y) = (e["x"].as_f64().unwrap_or(0.0), e["y"].as_f64().unwrap_or(0.0));
        let (w, h) = (e["width"].as_f64().unwrap_or(0.0), e["height"].as_f64().unwrap_or(0.0));
        let (mut ex0, mut ey0, mut ex1, mut ey1) = (x, y, x + w, y + h);
        if let Some(pts) = e["points"].as_array() {
            for p in pts {
                let (px, py) = (x + p[0].as_f64().unwrap_or(0.0), y + p[1].as_f64().unwrap_or(0.0));
                ex0 = ex0.min(px);
                ey0 = ey0.min(py);
                ex1 = ex1.max(px);
                ey1 = ey1.max(py);
            }
        }
        x0 = x0.min(ex0);
        y0 = y0.min(ey0);
        x1 = x1.max(ex1);
        y1 = y1.max(ey1);
    }
    if x0 > x1 {
        return;
    }
    let s = (bw / (x1 - x0).max(1.0)).min(bh / (y1 - y0).max(1.0)).min(1.0);
    let (dx, dy) = (bx + (bw - (x1 - x0) * s) / 2.0, by + (bh - (y1 - y0) * s) / 2.0);
    for e in els.iter_mut() {
        let x = e["x"].as_f64().unwrap_or(0.0);
        let y = e["y"].as_f64().unwrap_or(0.0);
        e["x"] = json!(dx + (x - x0) * s);
        e["y"] = json!(dy + (y - y0) * s);
        for k in ["width", "height", "fontSize", "unidad"] {
            if let Some(v) = e[k].as_f64() {
                e[k] = json!(v * s);
            }
        }
        if let Some(pts) = e["points"].as_array_mut() {
            for p in pts.iter_mut() {
                if let (Some(a), Some(b)) = (p[0].as_f64(), p[1].as_f64()) {
                    *p = json!([a * s, b * s]);
                }
            }
        }
    }
}

pub(crate) fn elementos(f: &mut Fabrica, p: &Problema) {
    let (piezas, caja) = piezas(p);
    let antes = f.grupo.clone();
    let grupo = f.nombre_de_grupo("problema");
    f.grupo = Some(grupo.clone());
    f.de_motor(&piezas);
    f.grupo = antes;
    if let Some(caja) = caja {
        // La figura, con sus figuras de siempre, encajada en su recuadro.
        let mut otra = Fabrica::nueva((0.0, 0.0), f.semilla.wrapping_add(0x9e37));
        for fig in &p.figura {
            crate::lienzo::figura(&mut otra, fig);
        }
        let mut els = otra.salida;
        encajar(&mut els, caja);
        for e in els.iter_mut() {
            e["x"] = json!(e["x"].as_f64().unwrap_or(0.0) + f.ox);
            e["y"] = json!(e["y"].as_f64().unwrap_or(0.0) + f.oy);
        }
        f.salida.extend(els);
    }
}

/// **Si el pedido es pasar en limpio o resolver un problema** (con una foto
/// adjunta: entonces la app le pega el OCR al mensaje, [`pista_de_ocr`]).
pub fn pide_problema(pedido: &str) -> bool {
    let q = crate::acciones::normalizar(pedido);
    [
        "problema", "ejercicio", "enunciado", "en limpio", "a limpio", "reescrib", "transcrib", "resuelve", "resolver", "resuelvelo",
        "tarea de mate", "examen",
    ]
    .iter()
    .any(|c| q.contains(c))
}

/// **Lo que el OCR de Windows leyo en la foto**, para pegarlo al mensaje del
/// usuario: el modelo pequeno lee mal la letra menuda y el OCR mal las
/// formulas, y juntos aciertan mas. Sin texto, se le dice: asi no inventa
/// un enunciado que la foto no tiene.
pub fn pista_de_ocr(lineas: &[String]) -> String {
    let texto: Vec<&str> = lineas.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    let letras: usize = texto.iter().map(|l| l.chars().filter(|c| c.is_alphanumeric()).count()).sum();
    if letras < 8 {
        return "\n\n[OCR de la foto: no se encontro texto legible. Si no ves un problema en la imagen, dilo; no lo inventes.]".into();
    }
    let mut t = texto.join("\n");
    if t.chars().count() > 2000 {
        t = t.chars().take(2000).collect();
    }
    format!("\n\n[Texto de la foto leido por OCR (puede tener errores; las formulas, mejor leelas de la imagen):]\n{t}")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::elemento::Figura as F;

    fn validar_(v: Value) -> Result<(Problema, String), ErrorHerramienta> {
        let Value::Object(m) = v else { panic!() };
        let mut avisos = Vec::new();
        validar(&m, &mut avisos).map(|p| (p, avisos.join(" | ")))
    }

    fn ejemplo() -> Value {
        json!({
            "titulo": "Problema 3",
            "enunciado": "Un bloque de 5 kg baja por un plano inclinado de 30° sin rozamiento. ¿Cual es su aceleracion?",
            "ecuaciones": ["a = g \\sin\\theta", "\\frac{1}{2} m v^2"],
            "datos": ["m = 5\\ kg", "\\theta = 30°", "g = 9.8\\ m/s^2"],
            "incognitas": ["a"],
            "figura": [
                {"tipo": "poligono", "puntos": [10, 250, 290, 250, 290, 90], "color": "negro"},
                {"tipo": "rectangulo", "x": 140, "y": 140, "ancho": 50, "alto": 40, "color": "azul"}
            ],
            "pasos": [
                {"texto": "La aceleracion es g por el seno del angulo", "calculo": "9.8*sin(30°)", "resultado": 5.1, "unidad": "m/s²"},
                "Sale la mitad de g"
            ],
            "respuesta": "a = 4.9 m/s²"
        })
    }

    #[test]
    fn un_problema_completo_en_limpio() {
        let (p, avisos) = validar_(ejemplo()).unwrap();
        // La cuenta la hace la app y corrige al modelo.
        assert!((p.pasos[0].resultado.unwrap() - 4.9).abs() < 1e-9);
        assert!(avisos.contains("da 4.9, no 5.1 (corregido)"), "{avisos}");
        let (els, caja) = piezas(&p);
        assert!(caja.is_some());
        let textos: Vec<String> = els
            .iter()
            .filter_map(|e| match &e.figura {
                F::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect();
        assert!(textos.iter().any(|t| t == "Solución"), "{textos:?}");
        assert!(textos.iter().any(|t| t.contains("Un bloque de 5 kg")), "{textos:?}");
        assert!(textos.iter().any(|t| t.contains("4.9")), "{textos:?}");
        // La fraccion de la segunda ecuacion lleva su raya.
        assert!(els.iter().filter(|e| matches!(e.figura, F::Linea { .. })).count() >= 2);
        let mut f = Fabrica::nueva((0.0, 0.0), 3);
        elementos(&mut f, &p);
        let l = pixpin_motor2d::excalidraw::leer(&crate::lienzo::documento(f.salida.clone())).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0);
        // La figura cae dentro de su recuadro.
        let [bx, by, bw, bh] = caja.unwrap();
        let rect = f.salida.iter().rev().find(|e| e["type"] == "rectangle").unwrap();
        let (x, y) = (rect["x"].as_f64().unwrap(), rect["y"].as_f64().unwrap());
        assert!(x >= bx - 1.0 && x <= bx + bw && y >= by - 1.0 && y <= by + bh, "({x}, {y}) fuera de {:?}", caja);
    }

    #[test]
    fn sin_pasos_el_recuadro_queda_para_el_estudiante() {
        let (p, avisos) = validar_(json!({"enunciado": "Halla las raices de x^2 - 5x + 6 = 0", "ecuaciones": ["x^2 - 5x + 6 = 0"]})).unwrap();
        assert!(avisos.is_empty());
        let (els, caja) = piezas(&p);
        assert!(caja.is_none());
        // El recuadro de la solucion es alto aunque este vacio.
        let alto = els.iter().filter(|e| matches!(e.figura, F::Rectangulo)).map(|e| e.alto).fold(0.0, f32::max);
        assert!(alto >= 150.0, "{alto}");
    }

    #[test]
    fn las_cuentas_las_hace_rust() {
        assert!((calcular("5*9.8").unwrap() - 49.0).abs() < 1e-12);
        assert!((calcular("2·3^2").unwrap() - 18.0).abs() < 1e-12);
        assert!((calcular("20^2*sin(2*45°)/9.8").unwrap() - 40.816326530612).abs() < 1e-9);
        assert!((calcular("sqrt(16) + pi").unwrap() - (4.0 + std::f64::consts::PI)).abs() < 1e-12);
        assert!(calcular("2*x").unwrap_err().contains("letra"));
        assert!(calcular("m*g").is_err());
        assert!(calcular("1/0").unwrap_err().contains("no da un numero"));
        assert!(calcular("5 +* )").unwrap_err().contains("no se entiende"));
    }

    #[test]
    fn una_foto_sin_texto_no_se_inventa() {
        // Sin enunciado (el modelo no leyo nada): no se dibuja.
        for v in [json!({}), json!({"enunciado": "   "}), json!({"enunciado": "?? 3 ..."})] {
            let e = validar_(v).unwrap_err().to_string();
            assert!(e.contains("no lo inventes"), "{e}");
        }
        assert!(pista_de_ocr(&[]).contains("no se encontro texto"));
        assert!(pista_de_ocr(&["  ".into(), "1".into()]).contains("no se encontro texto"));
        let p = pista_de_ocr(&["Un tren sale a las 8".into(), "a 60 km/h".into()]);
        assert!(p.contains("OCR") && p.contains("a 60 km/h"), "{p}");
    }

    #[test]
    fn lo_roto_se_rechaza() {
        for (v, motivo) in [
            (json!({"enunciado": "Un problema de verdad", "ecuaciones": ["\\frac{1}{2"]}), "llaves"),
            (json!({"enunciado": "Un problema de verdad", "pasos": [{"texto": "x", "calculo": "m*g"}]}), "paso 1"),
            (json!({"enunciado": "Un problema de verdad", "pasos": "uno"}), "lista"),
            (json!({"enunciado": "Un problema de verdad", "figura": [{"tipo": "dragon"}]}), "figura"),
            (json!({"enunciado": "x".repeat(MAX_ENUNCIADO + 1)}), "pasa de"),
        ] {
            let e = validar_(v.clone()).unwrap_err().to_string();
            assert!(e.contains(motivo), "{v}: {e}");
        }
    }

    #[test]
    fn los_datos_con_unidad_no_son_fracciones() {
        assert_eq!(a_texto("20\\ m/s"), "20 m/s");
        assert_eq!(a_texto("9.8\\,m/s^2"), "9.8 m/s²");
        assert_eq!(a_texto("\\text{Lanzamiento desde el suelo}"), "Lanzamiento desde el suelo");
        assert_eq!(a_texto("45^\\circ"), "45°");
        let (p, _) = validar_(json!({"enunciado": "Un proyectil se lanza a 20 m/s", "datos": ["v_0 = 20 m/s", "desde el suelo"]})).unwrap();
        let (els, _) = piezas(&p);
        let textos: Vec<String> = els
            .iter()
            .filter_map(|e| match &e.figura {
                F::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect();
        assert!(textos.iter().any(|t| t == "= 20 m/s"), "{textos:?}");
        assert!(textos.iter().any(|t| t == "desde el suelo"), "{textos:?}");
    }

    fn textos_de(els: &[pixpin_motor2d::Elemento]) -> Vec<String> {
        els.iter()
            .filter_map(|e| match &e.figura {
                F::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect()
    }

    /// Banco del 15-oct (hoja de la matriz): la matriz salia como lista
    /// «[[2,-1,0],...]» y las incognitas en LaTeX crudo «A^{-1}».
    #[test]
    fn la_matriz_y_las_incognitas_salen_compuestas() {
        assert_eq!(
            matriz_de_lista("[[2, -1, 0], [1, 3, 2]]").unwrap(),
            r"\begin{bmatrix} 2 & -1 & 0 \\ 1 & 3 & 2 \end{bmatrix}"
        );
        assert!(matriz_de_lista("[[a, b], [c, d]]").unwrap().contains("a & b"));
        assert!(matriz_de_lista("[1, 2]").is_none());
        assert!(valor_como_formula("20 m/s").is_none());
        let (p, _) = validar_(json!({
            "enunciado": "Dada la matriz A, calcula su determinante y su inversa.",
            "datos": ["A = [[2, -1, 0], [1, 3, 2], [0, 1, 4]]"],
            "incognitas": ["\\det A", "A^{-1}", "A \\cdot A^{-1} = I"]
        }))
        .unwrap();
        let (els, _) = piezas(&p);
        let t = textos_de(&els);
        assert!(t.iter().all(|x| !x.contains('[') && !x.contains('\\') && !x.contains('^') && !x.contains('{')), "{t:?}");
        // Las nueve cifras, sueltas en su celda.
        assert!(t.iter().filter(|x| ["2", "-1", "0", "1", "3", "4"].contains(&x.trim())).count() >= 9, "{t:?}");
    }

    /// Banco del 15-oct: la integral se quedaba en «a» (lo de detras del
    /// «a)» se perdia) y la ecuacion salia en el texto y compuesta.
    #[test]
    fn las_ecuaciones_salen_enteras_y_una_sola_vez() {
        let (p, _) = validar_(json!({
            "enunciado": "Calcula las integrales: $\\int_0^2 (x^2+1)\\,dx$ y la otra.",
            "ecuaciones": ["a) \\int_0^2 (x^2+1)\\,dx", "\\int_0^2 (x^2+1)\\,dx"],
            "datos": ["\\int_0^2 (x^2+1)\\,dx"]
        }))
        .unwrap();
        let (els, _) = piezas(&p);
        let t = textos_de(&els);
        assert!(t.iter().any(|x| x.contains('∫')), "la integral entera: {t:?}");
        assert!(t.iter().all(|x| !x.contains('$') && !x.contains("\\int")), "{t:?}");
        assert!(t.iter().any(|x| x.contains("Calcula las integrales")), "{t:?}");
        assert_eq!(sin_las_ecuaciones("Resuelve x^2 - 5x + 6 = 0 por factorizacion.", &["x^2 - 5x + 6 = 0".into()]), "Resuelve por factorizacion.");
        // Un enunciado que es SOLO la ecuacion se queda como estaba.
        assert_eq!(sin_las_ecuaciones("x^2 = 4", &["x^2 = 4".into()]), "x^2 = 4");
    }

    #[test]
    fn se_reconoce_el_pedido() {
        assert!(pide_problema("Pasa en limpio este problema"));
        assert!(pide_problema("resuelve el ejercicio de la foto"));
        assert!(!pide_problema("dibuja un gato"));
    }
}
