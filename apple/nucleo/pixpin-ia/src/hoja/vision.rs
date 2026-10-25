//! **Lo que se le pregunta al modelo con vision**, siempre con SOLO el
//! recorte de una zona (pocos tokens, respuesta corta) y sin herramientas:
//!
//! - una formula → su LaTeX ([`pedir_latex`]);
//! - una zona dudosa → que es ([`pedir_tipo`]);
//! - una grafica → si es de funciones, cuales y su ventana
//!   ([`pedir_grafica`], que luego dibuja `graficar_funcion`).
//!
//! Las respuestas se leen con manga ancha (con o sin `$`, con bloque de
//! codigo, con una frase delante) y lo que no se entiende es un error que la
//! app convierte en el camino sin modelo (la formula como imagen).

use serde_json::{Map, Value};

use super::Tipo;
use super::montaje::Leido;
use crate::motor::{Mensaje, Motor, Peticion, Seguir};

/// Lo mas que se deja contestar (una formula larga cabe de sobra).
pub const TOKENS: u32 = 400;

const SISTEMA_LATEX: &str = "Eres un transcriptor de formulas. Recibes el recorte de una formula de una hoja escaneada. \
Responde SOLO con la formula en LaTeX, sin $ ni explicaciones ni bloques de codigo. \
Si tiene varios renglones, separalos con \\\\. Si no hay ninguna formula, responde NADA. \
Usa SOLO este LaTeX: \\frac{}{}, \\sqrt{} y \\sqrt[n]{}, ^ y _, \\int, \\sum, \\lim, \\infty, griegas, \\cdot, \\pm, \\le, \\ge, \\neq, \\to, \\sin \\cos \\tan \\ln \\log, \
\\text{} para palabras, \\measuredangle para el angulo (∢), \\overset{\\frown}{AC} para el arco AC, \\overline{AB} para un segmento, \\therefore, \\begin{bmatrix}...\\end{bmatrix} (o pmatrix, vmatrix) para matrices y \\begin{cases}...\\end{cases} para sistemas con llave. \
Nada de \\displaystyle, \\left/\\right, \\mathrm, \\operatorname ni otros paquetes. \
Si delante de la formula hay una etiqueta (a), b), 1.), copiala al principio con \\text{a)}.";

const SISTEMA_TIPO: &str = "Clasificas recortes de una hoja escaneada. Responde SOLO una palabra de esta lista: \
texto, titulo, tabla, foto, grafica, dibujo. Si es una formula (tambien un sistema de ecuaciones con su llave, una matriz), responde FORMULA: y detras la formula en LaTeX (sin $; un sistema con \\begin{cases}...\\end{cases}, una matriz con \\begin{bmatrix}...\\end{bmatrix}).";

const SISTEMA_GRAFICA: &str = "Recibes el recorte de una grafica de una hoja. Si es la grafica de una o varias funciones y = f(x), \
responde SOLO un JSON como {\"funciones\":[\"x^2\"],\"x_desde\":-3,\"x_hasta\":3,\"y_desde\":-1,\"y_hasta\":9} \
(formulas en x con ^, sqrt(), sin(), cos(), ln(), e^x; la ventana que se ve). Si no lo es (barras, datos, un dibujo), responde NO.";

/// Pregunta con una imagen y devuelve el texto de la respuesta.
pub fn preguntar(
    motor: &dyn Motor,
    sistema: &str,
    png: Vec<u8>,
    pregunta: &str,
) -> Result<String, String> {
    preguntar_con(motor, sistema, png, pregunta, TOKENS)
}

/// Lo mismo con otro tope de tokens (la lista de rotulos de una figura es
/// mas larga que una formula).
pub fn preguntar_con(
    motor: &dyn Motor,
    sistema: &str,
    png: Vec<u8>,
    pregunta: &str,
    tokens: u32,
) -> Result<String, String> {
    let mut p = Peticion::nueva(vec![
        Mensaje::sistema(sistema),
        Mensaje::usuario_con_imagen(pregunta, png, "image/png"),
    ]);
    p.max_tokens = tokens;
    p.temperatura = 0.0;
    let r = motor
        .completar(&p, &mut |_| Seguir::Si)
        .map_err(|e| e.to_string())?;
    Ok(r.texto.trim().to_string())
}

/// **La formula del recorte, en LaTeX** (una linea por renglon).
pub fn pedir_latex(motor: &dyn Motor, png: Vec<u8>) -> Result<Leido, String> {
    let t = preguntar(
        motor,
        SISTEMA_LATEX,
        png,
        "Transcribe esta formula a LaTeX.",
    )?;
    leer_latex(&t)
        .map(Leido::Latex)
        .ok_or_else(|| format!("el modelo no dio una formula: «{}»", corto(&t)))
}

/// **Que es una zona dudosa.**
pub fn pedir_tipo(motor: &dyn Motor, png: Vec<u8>) -> Result<Leido, String> {
    let t = preguntar(motor, SISTEMA_TIPO, png, "¿Que es este recorte?")?;
    leer_dudosa(&t).ok_or_else(|| format!("el modelo no dijo que es: «{}»", corto(&t)))
}

/// Lee lo que dijo de una zona dudosa: un tipo, o `FORMULA: <latex>` (de
/// una vez, sin otra pregunta).
pub fn leer_dudosa(t: &str) -> Option<Leido> {
    let limpio = sin_bloque(t);
    let s = limpio.trim();
    let cabeza: String = s.chars().take(8).collect::<String>().to_lowercase();
    if cabeza.starts_with("formula") || cabeza.starts_with("fórmula") {
        if let Some((_, latex)) = s.split_once(':')
            && let Some(l) = leer_latex(latex)
        {
            return Some(Leido::Latex(l));
        }
        return Some(Leido::Tipo(Tipo::Formula));
    }
    // Con una frase delante: la primera palabra de la lista que salga.
    leer_tipo(s)
        .or_else(|| s.split(|c: char| !c.is_alphanumeric()).find_map(leer_tipo))
        .map(Leido::Tipo)
}

/// **Si una grafica es de funciones**: sus argumentos de `graficar_funcion`,
/// o `None` si no lo es.
pub fn pedir_grafica(motor: &dyn Motor, png: Vec<u8>) -> Result<Option<Leido>, String> {
    let t = preguntar(motor, SISTEMA_GRAFICA, png, "¿Es la grafica de funciones?")?;
    Ok(leer_grafica(&t).map(Leido::Funciones))
}

/// **Si la respuesta es el modelo hablando** y no lo que se le pidio («Solo
/// veo un icono de corazon... enviame el recorte correcto»): eso acabo
/// escrito en el lienzo como si fuera texto de la hoja (17-oct). Una
/// transcripcion de verdad no habla de la imagen ni pide nada.
pub fn es_charla(t: &str) -> bool {
    let q = crate::acciones::normalizar(t);
    const FRASES: &[&str] = &[
        "solo veo",
        "no veo",
        "no hay ningun",
        "no hay texto",
        "no puedo",
        "enviame",
        "envia el",
        "si quieres",
        "no contiene",
        "la imagen muestra",
        "la imagen contiene",
        "el recorte",
        "este recorte",
        "lo siento",
        "parece ser",
        "no se ve",
        "no se distingue",
        "i can't",
        "i cannot",
        "i only see",
        "the image",
    ];
    FRASES.iter().any(|f| q.contains(f))
}

fn corto(t: &str) -> String {
    t.chars().take(80).collect()
}

/// Quita un bloque de codigo de alrededor (```latex ... ```).
fn sin_bloque(t: &str) -> String {
    let t = t.trim();
    if let Some(r) = t.strip_prefix("```") {
        let r = r.split_once('\n').map_or(r, |(_, resto)| resto);
        return r.trim_end().trim_end_matches("```").trim().to_string();
    }
    t.to_string()
}

/// **Lee el LaTeX** de la respuesta: sin `$`, sin `\[ \]`, sin bloque de
/// codigo; renglones por `\\` o salto de linea. `None` si no hay formula.
pub fn leer_latex(t: &str) -> Option<Vec<String>> {
    let t = sin_bloque(t);
    let t = t.trim().trim_matches('$').trim();
    let t = t.trim_start_matches("\\[").trim_end_matches("\\]").trim();
    let t = t.trim_start_matches("\\(").trim_end_matches("\\)").trim();
    // (Ni una respuesta que habla de su formato: «answer = LaTeX only... Output».)
    let bajo = t.to_lowercase();
    let habla = ["answer", "latex only", "output", "formula is", "the image"]
        .iter()
        .any(|p| bajo.contains(p));
    if t.is_empty()
        || t.eq_ignore_ascii_case("nada")
        || t.len() > crate::ecuacion::MAX_LARGO
        || (es_charla(t) && !t.contains('\\'))
        || habla
    {
        return None;
    }
    // Una frase de mas («Aqui esta: ...»): lo de despues de los dos puntos.
    let t = match t.split_once(": ") {
        Some((antes, despues))
            if antes.split_whitespace().count() >= 2
                && !antes.contains('\\')
                && !antes.contains('=') =>
        {
            despues
        }
        _ => t,
    };
    // Los renglones, sin partir las matrices (`\\` dentro de un entorno es
    // una fila suya, no un renglon).
    let lineas: Vec<String> = crate::ecuacion::partir_lineas(t)
        .into_iter()
        .map(|l| l.trim_matches('$').trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    (!lineas.is_empty()).then_some(lineas)
}

/// Lee el tipo de una palabra (con tildes, mayusculas o punto).
pub fn leer_tipo(t: &str) -> Option<Tipo> {
    let q = crate::acciones::normalizar(t);
    let p = q
        .split(|c: char| !c.is_alphanumeric())
        .find(|p| !p.is_empty())?;
    Some(match p {
        "texto" | "parrafo" => Tipo::Parrafo,
        "titulo" | "encabezado" => Tipo::Encabezado,
        "formula" | "ecuacion" => Tipo::Formula,
        "tabla" => Tipo::Tabla,
        "foto" | "fotografia" | "imagen" => Tipo::Foto,
        "grafica" | "grafico" => Tipo::Grafica,
        "dibujo" | "diagrama" | "esquema" => Tipo::Dibujo,
        _ => return None,
    })
}

/// Lee el JSON de una grafica de funciones (con o sin bloque de codigo).
/// `None` si dice que no o no trae funciones.
pub fn leer_grafica(t: &str) -> Option<Map<String, Value>> {
    let t = sin_bloque(t);
    let (a, b) = (t.find('{')?, t.rfind('}')?);
    let v: Value = serde_json::from_str(t.get(a..=b)?).ok()?;
    let m = v.as_object()?.clone();
    let n = m
        .get("funciones")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    (n > 0).then_some(m)
}

const SISTEMA_ROTULOS: &str = "Recibes el recorte de una figura geometrica o un diagrama. Lista TODOS sus rotulos de texto: \
letras de puntos (A, B, O, P...), medidas y angulos (2a, 45°, θ, x), palabras y formulas cortas. \
Responde SOLO un JSON: [{\"t\":\"A\",\"x\":0.12,\"y\":0.40}, ...] con el texto tal cual se ve (griegas en Unicode, grados con °) \
y el centro de cada rotulo en fracciones del ancho y del alto del recorte (0 a 1). \
Despues, en la misma lista, los HECHOS geometricos que se ven o que dice el texto, uno por objeto: \
{\"h\":\"semicircunferencia AB\"}, {\"h\":\"AB es diametro\"}, {\"h\":\"AOB es un cuadrante\"}, {\"h\":\"centro O\"}, {\"h\":\"m∢APB=90°\"} \
(angulos con ∢ y tres letras, vertice en medio). Sin explicaciones.";

const SISTEMA_RICO: &str = "Recibes el recorte de un bloque de texto de una lamina o una hoja (puede llevar formulas). \
Transcribelo EXACTO, renglon a renglon, en este formato de texto rico: \
formulas en linea entre $...$ con LaTeX (\\sphericalangle para el angulo ∢, \\overline{AB} para la raya, \\overset{\\frown}{AC} para el arco, \\frac{a}{b}, 90^\\circ, \\therefore); \
*cursiva*, **negrita**, <u>subrayado</u>; el color de un tramo que no es negro con \\color{rojo}{...} (rojo, verde, azul, naranja, morado, gris o #rrggbb), tambien dentro de una formula; \
un renglon centrado empieza con \\centro (sin llaves, seguido del texto); las vinetas con •. \
MUY IMPORTANTE: los colores. Mira el color de cada letra: lo rojo va con \\color{rojo}{...}, lo verde con \\color{verde}{...}, lo azul con \\color{azul}{...} (dentro de una formula tambien: $X=90^\\circ - \\color{rojo}{(a+b)}$). \
Los renglones que en el papel van centrados o sangrados bajo una vineta (las formulas de debajo) empiezan con \\centro. \
Fuera de $...$ no uses otros mandatos (ni \\text ni \\boxed: los recuadros ya estan dibujados). \
Copia las letras tal cual (con sus tildes), sin corregir ni resumir. \
Responde SOLO el texto, sin explicaciones ni bloques de codigo.";

const SISTEMA_RENGLONES: &str = "Recibes el recorte de un trozo de una hoja (una lamina, un apunte, una captura de pantalla) \
y lo que el OCR leyo en el, renglon a renglon y numerado. El OCR se equivoca: pierde tildes, lee ° como 0, θ como o, ∢ como <, la raya de AB, las griegas. \
Devuelve SOLO un JSON: una lista con EXACTAMENTE el mismo numero de renglones y en el mismo orden, cada uno corregido mirando la imagen. \
El texto esta en espanol: pon TODAS las tildes y la ñ como deben ir (intencion → intención, ingenieria → ingeniería) y los signos ¿ ¡. \
Los EMOJIS y simbolos que se ven en la imagen (📋 ✨ 🔗 📝 ✅ • ★ →) van tal cual en su sitio: el OCR los lee mal («+4-», «9») o no los lee. \
Formulas en linea entre $...$ con LaTeX (\\sphericalangle para ∢, \\overline{AB}, \\overset{\\frown}{AC}, 90^\\circ, \\theta); \
las palabras que en la imagen estan en NEGRITA van entre **...** (un tramo o el renglon entero, solo lo que es negrita); un tramo en cursiva entre *...*. NO pongas colores (\\color): la app ya sabe el color de cada renglon. \
No juntes, partas, anadas ni quites renglones; no resumas; si un renglon esta bien, copialo igual. Sin explicaciones.";

/// **Los renglones de una zona, corregidos** (17-oct): el OCR de cada renglon
/// va con el recorte, y vuelve la misma lista arreglada. Cada renglon se
/// queda en su sitio; un renglon que no se parece a lo leido (el modelo
/// inventando o hablando) se queda como lo leyo el OCR.
pub fn pedir_renglones(
    motor: &dyn Motor,
    png: Vec<u8>,
    ocr: &[String],
    contexto: &str,
) -> Result<Leido, String> {
    let lista: String = ocr
        .iter()
        .enumerate()
        .map(|(i, r)| format!("{}. {r}\n", i + 1))
        .collect();
    let contexto = if contexto.is_empty() {
        String::new()
    } else {
        format!("{contexto}\n")
    };
    let t = preguntar_con(
        motor,
        SISTEMA_RENGLONES,
        png,
        &format!(
            "{contexto}Lo que leyo el OCR ({} renglones):\n{lista}",
            ocr.len()
        ),
        2500,
    )?;
    leer_renglones(&t, ocr)
        .or_else(|| renglones_libres(&t, ocr))
        .map(Leido::Renglones)
        .ok_or_else(|| format!("el modelo no devolvio los renglones: «{}»", corto(&t)))
}

/// **Los renglones de la nube aunque no casen con los del OCR** (18-oct,
/// Polinomios: el OCR partia «M = (x+2)/(y−1)» en tres y leia basura; la
/// nube daba la lista bien, pero con otro numero de renglones, y se tiraba
/// para poner el OCR crudo). Se aceptan si, en conjunto, dicen lo mismo:
/// las letras de fuera de las formulas, parecidas a las del OCR. Quien los
/// pinta los reparte por el alto de la zona.
pub fn renglones_libres(t: &str, ocr: &[String]) -> Option<Vec<String>> {
    let t = sin_bloque(t);
    let (a, b) = (t.find('[')?, t.rfind(']')?);
    let v: Vec<String> = serde_json::from_str::<Vec<Value>>(t.get(a..=b)?)
        .ok()?
        .into_iter()
        .filter_map(|x| match x {
            Value::String(s) => Some(s),
            Value::Object(m) => ["texto", "text", "t"]
                .iter()
                .find_map(|k| m.get(*k).and_then(Value::as_str))
                .map(str::to_string),
            _ => None,
        })
        .map(|s| pixpin_motor2d::texto_rico::sin_barras_dobles(s.trim()))
        .filter(|s| !s.is_empty())
        .collect();
    if v.is_empty() || v.iter().any(|s| es_charla(s)) {
        return None;
    }
    let todo_nuevo = v.join(" ");
    let todo_viejo = ocr.join(" ");
    let (n, o) = (
        cuantas_letras(&todo_nuevo) as f64,
        cuantas_letras(&todo_viejo).max(1) as f64,
    );
    let parecido = parecido_de_letras(&todo_nuevo, &todo_viejo).max(parecido_de_letras(
        &fuera_de_formulas(&todo_nuevo),
        &todo_viejo,
    ));
    if parecido < 0.45 || n < o * 0.5 || n > o * 2.5 + 10.0 {
        return None;
    }
    Some(v.into_iter().map(|s| leer_rico(&s).unwrap_or(s)).collect())
}

/// Lee la lista corregida: tiene que traer los mismos renglones; cada uno se
/// acepta si se parece al del OCR (`parecido_de_letras` >= 0.5) y no es el
/// modelo hablando.
pub fn leer_renglones(t: &str, ocr: &[String]) -> Option<Vec<String>> {
    let t = sin_bloque(t);
    let (a, b) = (t.find('[')?, t.rfind(']')?);
    // (Haiku a veces devuelve objetos, `{"renglon":1,"texto":"..."}`: su texto.)
    let v: Vec<String> = serde_json::from_str::<Vec<Value>>(t.get(a..=b)?)
        .ok()?
        .into_iter()
        .map(|x| match x {
            Value::String(s) => s,
            Value::Object(m) => ["texto", "text", "t", "renglon_corregido"]
                .iter()
                .find_map(|k| m.get(*k).and_then(Value::as_str))
                .or_else(|| m.values().find_map(Value::as_str))
                .unwrap_or_default()
                .to_string(),
            otro => otro.to_string(),
        })
        .collect();
    // **Menos renglones que el OCR** (18-oct, Polinomios: la fraccion
    // «M = (x+2)/(y-1)» eran tres renglones del OCR y uno de verdad): cada
    // renglon de la nube va, en orden, al del OCR que mas se le parece; los
    // demas quedan vacios (lo suyo ya va en el otro).
    let v = if v.len() < ocr.len() && !v.is_empty() {
        let mut out = vec![String::new(); ocr.len()];
        let mut desde = 0;
        for (k, nuevo) in v.iter().enumerate() {
            // (Que queden sitios para los que faltan.)
            let hasta = ocr.len() - (v.len() - k - 1);
            let mejor = (desde..hasta).max_by(|&i, &j| {
                parecido_de_letras(nuevo, &ocr[i])
                    .total_cmp(&parecido_de_letras(nuevo, &ocr[j]))
                    .then(j.cmp(&i))
            })?;
            // (Que se parezca de verdad: si no, no es esa hoja.)
            let p = parecido_de_letras(nuevo, &ocr[mejor])
                .max(parecido_de_letras(&fuera_de_formulas(nuevo), &ocr[mejor]));
            if p < 0.4 {
                return None;
            }
            out[mejor] = nuevo.clone();
            desde = mejor + 1;
        }
        return Some(
            out.into_iter()
                .map(|nuevo| {
                    if nuevo.trim().is_empty() {
                        return String::new();
                    }
                    let n = pixpin_motor2d::texto_rico::sin_barras_dobles(nuevo.trim());
                    leer_rico(&n).unwrap_or(n)
                })
                .collect(),
        );
    } else if v.len() != ocr.len() {
        return None;
    } else {
        v
    };
    let mut hecho: Vec<String> = v
        .into_iter()
        .zip(ocr)
        .map(|(nuevo, viejo)| {
            let nuevo = sin_numero_inventado(
                &pixpin_motor2d::texto_rico::sin_barras_dobles(nuevo.trim()),
                viejo,
            );
            // (Ni uno mucho mas largo o mas corto: la frase entera para un
            // renglon que era un trozo salia encima de la otra mitad.)
            // (Lo de dentro de `$...$` no cuenta: el OCR no lee las
            // formulas, «Sea» era «Sea $P_{(x)}=x^3+3x^2+3x+3$».)
            // (Para lo de mas, solo cuenta lo de fuera; para lo de menos, todo.)
            let (n, v) = (
                cuantas_letras(&nuevo) as f64,
                cuantas_letras(viejo).max(1) as f64,
            );
            let n_fuera = cuantas_letras(&fuera_de_formulas(&nuevo)) as f64;
            if nuevo.is_empty()
                || es_charla(&nuevo)
                || parecido_de_letras(&nuevo, viejo)
                    .max(parecido_de_letras(&fuera_de_formulas(&nuevo), viejo))
                    < 0.5
                || n_fuera > v * 1.35 + 2.0
                || n < v * 0.65 - 2.0
            {
                viejo.clone()
            } else {
                // (LaTeX sin `$` —«16,5^\circ»— se envuelve, como en `leer_rico`.)
                leer_rico(&nuevo).unwrap_or(nuevo)
            }
        })
        .collect();
    sin_lo_ya_dicho(&mut hecho, ocr);
    Some(hecho)
}

/// **Lo que ya dice otro renglon** (19-oct, TikTok: el OCR partia «2 dia(s)
/// | Responder» en dos y la nube daba «2 día(s) Responder» en uno; el otro
/// se quedaba con lo del OCR y salia dos veces). Un renglon que se quedo con
/// lo del OCR y cuyas letras ya van dentro del corregido de otro, vacio.
fn sin_lo_ya_dicho(hecho: &mut [String], ocr: &[String]) {
    let plano = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .map(|c| match c {
                'á' => 'a',
                'é' => 'e',
                'í' => 'i',
                'ó' => 'o',
                'ú' => 'u',
                _ => c,
            })
            .collect()
    };
    let n = hecho.len().min(ocr.len());
    for j in 0..n {
        let pj = plano(&ocr[j]);
        if pj.chars().count() < 3 || hecho[j] != ocr[j] {
            continue;
        }
        let dicho = (0..n).any(|i| {
            i != j
                && hecho[i] != ocr[i]
                && plano(&hecho[i]).contains(&pj)
                && !plano(&ocr[i]).contains(&pj)
        });
        if dicho {
            hecho[j].clear();
        }
    }
}

/// **Sin una numeracion que no estaba** (18-oct: la nube devolvia «1. Si le
/// asignamos…», «2. tica y…» para un parrafo corrido). Si el renglon del OCR
/// no empieza por una cifra, el «1. » de delante se quita.
fn sin_numero_inventado(nuevo: &str, viejo: &str) -> String {
    let n = nuevo.chars().take_while(|c| c.is_ascii_digit()).count();
    let viejo_numerado = viejo.trim_start().starts_with(|c: char| c.is_ascii_digit());
    if n > 0 && n <= 2 && !viejo_numerado {
        let resto = &nuevo[n..];
        if let Some(r) = resto
            .strip_prefix(". ")
            .or_else(|| resto.strip_prefix(") "))
        {
            return r.to_string();
        }
    }
    nuevo.to_string()
}

/// El texto sin lo que va entre `$...$`.
fn fuera_de_formulas(s: &str) -> String {
    let mut fuera = String::new();
    let mut dentro = false;
    for c in s.chars() {
        if c == '$' {
            dentro = !dentro;
        } else if !dentro {
            fuera.push(c);
        }
    }
    fuera
}

/// Las letras y cifras de un texto, sin los mandatos de LaTeX.
fn cuantas_letras(s: &str) -> usize {
    let mut n = 0;
    let mut c = s.chars().peekable();
    while let Some(ch) = c.next() {
        if ch == '\\' {
            while c.peek().is_some_and(|x| x.is_ascii_alphabetic()) {
                c.next();
            }
        } else if ch.is_alphanumeric() {
            n += 1;
        }
    }
    n
}

/// Cuanto se parecen las letras y cifras de dos textos (0-1), sin los
/// mandatos de LaTeX ni las marcas del texto rico: «16,5°» y «16,50», si;
/// «Solo veo un icono» y «2293», no.
pub fn parecido_de_letras(a: &str, b: &str) -> f64 {
    let letras = |s: &str| -> Vec<char> {
        let mut v = Vec::new();
        let mut c = s.chars().peekable();
        while let Some(ch) = c.next() {
            if ch == '\\' {
                while c.peek().is_some_and(|x| x.is_ascii_alphabetic()) {
                    c.next();
                }
            } else if ch.is_alphanumeric() {
                v.extend(ch.to_lowercase());
            }
        }
        v
    };
    let (x, y) = (letras(a), letras(b));
    if x.is_empty() && y.is_empty() {
        return 1.0;
    }
    let mut hay: std::collections::HashMap<char, usize> = Default::default();
    for c in &y {
        *hay.entry(*c).or_default() += 1;
    }
    let mut comun = 0;
    for c in &x {
        if let Some(n) = hay.get_mut(c).filter(|n| **n > 0) {
            *n -= 1;
            comun += 1;
        }
    }
    comun as f64 / x.len().max(y.len()) as f64
}

/// **Un bloque de texto en texto rico del lienzo** (`pixpin_motor2d::texto_rico`):
/// la columna «RESOLUCION» de una lamina con sus formulas en linea, sus
/// colores por tramo, su cursiva y su subrayado. El OCR de Windows leia
/// «Äb» por «AB» con su raya y «m<COD» por «m∢COD».
pub fn pedir_rico(motor: &dyn Motor, png: Vec<u8>, pista: &str) -> Result<Leido, String> {
    // Con lo que el OCR vio de color (Haiku se olvidaba de los colores).
    let pregunta = if pista.is_empty() {
        "Transcribe este bloque.".to_string()
    } else {
        format!(
            "Transcribe este bloque. Colores que se ven (del OCR, puede leer mal las letras): {pista}"
        )
    };
    let t = preguntar_con(motor, SISTEMA_RICO, png, &pregunta, 3000)?;
    leer_rico(&t)
        .map(Leido::Rico)
        .ok_or_else(|| format!("el modelo no dio el texto: «{}»", corto(&t)))
}

/// Lee la transcripcion: sin bloque de codigo, sin barras dobles de mas,
/// sin renglones vacios al principio y al final. `None` si no hay nada.
pub fn leer_rico(t: &str) -> Option<String> {
    let t = sin_bloque(t);
    let t = pixpin_motor2d::texto_rico::sin_barras_dobles(&t);
    let mut t = t
        .trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ')
        .replace("\r\n", "\n");
    // Un renglon en blanco entre parrafos se queda (es el aire del papel);
    // mas de uno, no.
    while t.contains("\n\n\n") {
        t = t.replace("\n\n\n", "\n\n");
    }
    // Un renglon con LaTeX y sin `$` («θ=90^\circ»): es una formula entera.
    // Y un `$` que no se cerro (la respuesta cortada), se cierra.
    let t = t
        .split('\n')
        .map(|l| {
            let (pre, resto) = match l.strip_prefix("\\centro") {
                Some(r) => ("\\centro ", r.trim_start()),
                None => ("", l),
            };
            let n = resto.matches('$').count();
            // (No si lleva palabras: «Grafica de f(x) = x^2» como formula entera
            // perdia sus espacios.)
            let sin_mandatos: String = {
                let mut s = String::new();
                let mut c = resto.chars().peekable();
                while let Some(ch) = c.next() {
                    if ch == '\\' {
                        while c.peek().is_some_and(|x| x.is_ascii_alphabetic()) {
                            c.next();
                        }
                        s.push(' ');
                    } else {
                        s.push(ch);
                    }
                }
                s
            };
            let con_palabras = sin_mandatos
                .split(|c: char| !c.is_alphabetic())
                .any(|p| p.chars().count() >= 4);
            if n == 0 && !con_palabras && (resto.contains('\\') || resto.contains('^')) {
                format!("{pre}${resto}$")
            } else if n == 0 && (resto.contains('\\') || resto.contains('^')) {
                // Con palabras: solo los trozos de formula («x^2»).
                let t: Vec<String> = resto
                    .split(' ')
                    .map(|p| {
                        if p.contains('\\') || p.contains('^') {
                            format!("${p}$")
                        } else {
                            p.to_string()
                        }
                    })
                    .collect();
                format!("{pre}{}", t.join(" "))
            } else if n % 2 == 1 {
                format!("{pre}{resto}$")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    (!t.is_empty() && !t.eq_ignore_ascii_case("nada") && !es_charla(&t)).then_some(t)
}

#[cfg(test)]
mod pruebas_rico {
    #[test]
    fn el_texto_rico_de_la_nube_se_arregla() {
        assert_eq!(super::leer_rico("θ=90^\\circ").unwrap(), "$θ=90^\\circ$");
        assert_eq!(
            super::leer_rico("\\centro X=90^\\circ").unwrap(),
            "\\centro $X=90^\\circ$"
        );
        assert_eq!(
            super::leer_rico("Por ∢ interior:\n$45^\\circ=\\frac{2a").unwrap(),
            "Por ∢ interior:\n$45^\\circ=\\frac{2a$"
        );
        assert_eq!(
            super::leer_rico("```\nNos piden $X$\n\n\n\nY\n```").unwrap(),
            "Nos piden $X$\n\nY"
        );
        assert!(super::leer_rico("NADA").is_none());
        // El modelo hablando no es texto de la hoja (17-oct, salia en el lienzo).
        assert!(super::leer_rico("Solo veo un ícono de corazón con el número 2293, sin bloque de texto ni fórmulas. Si quieres transcribir una lámina, envíame el recorte correcto.").is_none());
        assert!(super::leer_latex("No veo ninguna formula en la imagen").is_none());
        assert!(super::leer_rico("Nos piden $m\\sphericalangle COD = X$").is_some());
        // Los renglones corregidos: los mismos, y el que no se parece se queda como el OCR.
        let ocr = vec!["A) 16,50".to_string(), "2293".to_string()];
        let r = super::leer_renglones(
            "[\"A) $16{,}5^\\\\circ$\", \"Solo veo un icono de corazon\"]",
            &ocr,
        )
        .unwrap();
        assert_eq!(r[1], "2293");
        assert!(r[0].contains("circ"), "{r:?}");
        assert!(
            super::leer_renglones("[\"uno\"]", &ocr).is_none(),
            "otro numero de renglones"
        );
        let r = super::leer_renglones(
            "[{\"renglon\":1,\"texto\":\"A) 16,5°\"},{\"n\":2,\"texto\":\"2293\"}]",
            &ocr,
        )
        .unwrap();
        assert_eq!(r, vec!["A) 16,5°".to_string(), "2293".to_string()]);
        let r = super::leer_renglones(r#"["A) 16,5^\\circ", "2293"]"#, &ocr).unwrap();
        assert!(r[0].contains('$'), "{r:?}");
        // La frase entera para un trozo: se queda el trozo.
        let o2 = vec!["Se tiene un".to_string()];
        assert_eq!(
            super::leer_renglones("[\"Se tiene un paralelogramo ABCD y se traza AP\"]", &o2)
                .unwrap(),
            o2
        );
        assert_eq!(
            super::leer_rico("Grafica de f(x) = x^2 - 2").unwrap(),
            "Grafica de f(x) = $x^2$ - 2"
        );
    }
}

/// **Los rotulos de una figura** (letras de los puntos, medidas, angulos)
/// con su centro en fracciones del recorte: la nube corrige al OCR en lo
/// que lee mal o no ve (una letra suelta, una griega).
pub fn pedir_rotulos(motor: &dyn Motor, png: Vec<u8>) -> Result<Leido, String> {
    let t = preguntar_con(
        motor,
        SISTEMA_ROTULOS,
        png,
        "Lista los rotulos de esta figura.",
        1500,
    )?;
    let hechos = leer_hechos(&t);
    leer_rotulos(&t)
        .map(|r| Leido::Rotulos(r, hechos))
        .ok_or_else(|| format!("el modelo no dio los rotulos: «{}»", corto(&t)))
}

/// Los hechos de la lista (`{"h":"AB es diametro"}`), tal cual.
pub fn leer_hechos(t: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut resto = t;
    while let Some(a) = resto.find('{') {
        let Some(b) = resto[a..].find('}') else { break };
        if let Ok(r) = serde_json::from_str::<Value>(&resto[a..a + b + 1])
            && let Some(h) = r.get("h").and_then(Value::as_str)
            && !h.trim().is_empty()
        {
            v.push(h.trim().to_string());
        }
        resto = &resto[a + b + 1..];
    }
    v
}

/// Lee la lista de rotulos (`[{"t":..,"x":..,"y":..}]`, con o sin bloque).
pub fn leer_rotulos(t: &str) -> Option<Vec<(String, f64, f64)>> {
    let t = sin_bloque(t);
    // Objeto a objeto: una lista cortada por el tope de tokens vale hasta
    // donde llego (banco del 15-oct: la de la lamina llegaba a medias).
    let mut lista = Vec::new();
    let mut resto = t.as_str();
    let mut alguno = false;
    while let Some(a) = resto.find('{') {
        let Some(b) = resto[a..].find('}') else { break };
        alguno = true;
        if let Ok(r) = serde_json::from_str::<Value>(&resto[a..a + b + 1]) {
            let texto = r
                .get("t")
                .or_else(|| r.get("texto"))
                .and_then(Value::as_str)
                .map(|s| s.trim().to_string());
            let x = r.get("x").and_then(Value::as_f64);
            let y = r.get("y").and_then(Value::as_f64);
            if let (Some(texto), Some(x), Some(y)) = (texto, x, y)
                && !texto.is_empty()
                && (0.0..=1.0).contains(&x)
                && (0.0..=1.0).contains(&y)
            {
                lista.push((texto, x, y));
            }
        }
        resto = &resto[a + b + 1..];
    }
    (alguno || t.contains('[')).then_some(lista)
}

#[cfg(test)]
mod pruebas_rotulos {
    use super::*;

    #[test]
    fn los_rotulos_se_leen_aunque_lleguen_cortados() {
        let t = "```json\n[{\"t\":\"A\",\"x\":0.27,\"y\":0.12},{\"t\":\"2a\",\"x\":0.37,\"y\":0.04},{\"t\":\"B\",\"x\":0.8";
        let v = leer_rotulos(t).unwrap();
        assert_eq!(
            v,
            vec![
                ("A".to_string(), 0.27, 0.12),
                ("2a".to_string(), 0.37, 0.04)
            ]
        );
        assert_eq!(leer_rotulos("[]"), Some(Vec::new()));
        assert!(leer_rotulos("No veo rotulos").is_none());
        // Fuera de 0-1: no.
        assert!(
            leer_rotulos("[{\"t\":\"A\",\"x\":3,\"y\":0.1}]")
                .unwrap()
                .is_empty()
        );
    }
}

#[cfg(test)]
mod pruebas_numeracion {
    #[test]
    fn la_numeracion_inventada_se_quita() {
        assert_eq!(
            super::sin_numero_inventado("1. Si le asignamos valores", "Si Ie asignamos valores"),
            "Si le asignamos valores"
        );
        assert_eq!(
            super::sin_numero_inventado("1. Número de RFIs", "1. Nümero de RFIs"),
            "1. Número de RFIs"
        );
        assert_eq!(
            super::sin_numero_inventado("2026 fue el año", "2026 fue"),
            "2026 fue el año"
        );
    }
}

#[cfg(test)]
mod pruebas_formula_que_el_ocr_no_leyo {
    #[test]
    fn lo_de_dentro_de_las_formulas_no_cuenta_como_de_mas() {
        let ocr = vec!["Sea".to_string()];
        let r = super::leer_renglones(r#"["Sea $P_{(x)}=x^{3}+3x^{2}+3x+3$."]"#, &ocr).unwrap();
        assert!(r[0].contains("x^{3}"), "{r:?}");
        // Pero una frase entera de texto para un trozo, no.
        let r = super::leer_renglones(
            r#"["Sea la funcion que se define en todo el dominio real"]"#,
            &ocr,
        )
        .unwrap();
        assert_eq!(r[0], "Sea");
    }
}

#[cfg(test)]
mod pruebas_lo_ya_dicho {
    use super::*;

    #[test]
    fn un_trozo_que_ya_va_en_otro_renglon_no_sale_dos_veces() {
        let ocr = vec!["Responder".to_string(), "2 dia(s)".to_string()];
        let r = leer_renglones(r#"["2 día(s) Responder", "zz"]"#, &ocr).unwrap();
        assert_eq!(r, vec!["2 día(s) Responder".to_string(), String::new()]);
        // (Si cada uno dice lo suyo, los dos se quedan.)
        let r = leer_renglones(r#"["Responder", "2 día(s)"]"#, &ocr).unwrap();
        assert_eq!(r, vec!["Responder".to_string(), "2 día(s)".to_string()]);
    }
}
