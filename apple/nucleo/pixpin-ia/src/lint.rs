//! **La revision automatica del lienzo**: lo que se ve mal, dicho con su
//! `#id`, sin mirar la imagen.
//!
//! Queja del usuario (22-oct-2026): el asistente «hace las cosas mal y no las
//! revisa». Mirar una captura (`ojos.rs`) cuesta una vuelta y tokens de
//! imagen; muchas faltas se ven en la escena misma, en microsegundos y sin
//! dudas: dos textos encimados, un rotulo que se sale de su caja, una flecha
//! que se queda a 20 px de la caja a la que iba, algo medio fuera de su
//! marco, dos cajas de una fila a 4 px de estar alineadas, un texto que
//! quedo sin traducir, «numero» sin tilde. Va pegado al resultado de cada
//! herramienta de dibujar y editar (`edicion::aplicar`,
//! `resumen_de_lo_anadido`): una senal de fuera que el modelo recibe gratis.
//!
//! Reglas para que sirva y no estorbe:
//!
//! - **Solo lo de esta llamada**: con `tocados`, cada aviso tiene que nombrar
//!   algo tocado (salvo el idioma: el texto que quedo sin traducir es justo
//!   el que NO se toco).
//! - **Corto**: [`para_el_modelo`] da como mucho [`MAX_LINEAS`] lineas.
//! - **Sin falsos avisos en un dibujo limpio**: cada regla tiene su margen
//!   (lo casi alineado es de 1,5 a 8 px; lo que esta a mas de 60 px de una
//!   caja no «iba» a ella).
//!
//! La ortografia va detras de un rasgo ([`Ortografia`]): aqui solo hay la
//! lista curada de palabras que casi siempre llevan tilde
//! ([`OrtografiaBasica`]); la app puede poner el corrector de Windows
//! (`ISpellChecker`) con [`instalar_ortografia`] sin tocar nada de aqui.

use std::sync::RwLock;

use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::{Elemento, Figura};

use crate::edicion::textos;

/// Cuantas lineas de avisos como mucho van al modelo.
pub const MAX_LINEAS: usize = 8;

/// Con mas elementos que estos, las reglas que comparan de dos en dos solo
/// miran lo tocado contra todo (y no todo contra todo).
const MAX_PARES: usize = 400;

/// Un aviso: lo que pasa y a quien.
#[derive(Debug, Clone, PartialEq)]
pub struct Hallazgo {
    pub ids: Vec<u64>,
    pub texto: String,
    /// Avisos de idioma: valen aunque no nombren nada tocado.
    pub de_idioma: bool,
}

fn aviso(ids: Vec<u64>, texto: String) -> Hallazgo {
    Hallazgo {
        ids,
        texto,
        de_idioma: false,
    }
}

// ---------------------------------------------------------------------------
// Ortografia

/// Una palabra mal escrita y su arreglo, si se sabe.
#[derive(Debug, Clone, PartialEq)]
pub struct Falta {
    pub palabra: String,
    pub arreglo: Option<String>,
}

/// **Quien sabe de ortografia.** `idioma` es el codigo corto («es», «en»).
/// El de Windows (`ISpellChecker`) se enchufa con [`instalar_ortografia`].
pub trait Ortografia: Send + Sync {
    fn revisar(&self, texto: &str, idioma: &str) -> Vec<Falta>;
}

static ORTOGRAFIA: RwLock<Option<Box<dyn Ortografia>>> = RwLock::new(None);

/// Pone el corrector de todo el proceso (la app, al arrancar).
pub fn instalar_ortografia(o: Box<dyn Ortografia>) {
    if let Ok(mut g) = ORTOGRAFIA.write() {
        *g = Some(o);
    }
}

/// **Las tildes que casi nunca sobran**: palabras que sin tilde no existen
/// o son rarisimas (de diagramas, apuntes y laminas), mas las terminaciones
/// -cion y -sion. Las que cambian de sentido con o sin tilde (esta/está,
/// que/qué, como/cómo) solo se marcan donde no hay duda: en una pregunta, o
/// «esta» delante de un participio o gerundio.
pub struct OrtografiaBasica;

const CON_TILDE: &[(&str, &str)] = &[
    ("tambien", "también"),
    ("ademas", "además"),
    ("despues", "después"),
    ("mas", "más"),
    ("aqui", "aquí"),
    ("ahi", "ahí"),
    ("alli", "allí"),
    ("asi", "así"),
    ("segun", "según"),
    ("algun", "algún"),
    ("ningun", "ningún"),
    ("jamas", "jamás"),
    ("detras", "detrás"),
    ("atras", "atrás"),
    ("traves", "través"),
    ("todavia", "todavía"),
    ("quiza", "quizá"),
    ("dia", "día"),
    ("dias", "días"),
    ("numero", "número"),
    ("numeros", "números"),
    ("pagina", "página"),
    ("paginas", "páginas"),
    ("facil", "fácil"),
    ("dificil", "difícil"),
    ("rapido", "rápido"),
    ("rapida", "rápida"),
    ("ultimo", "último"),
    ("ultima", "última"),
    ("ultimos", "últimos"),
    ("ultimas", "últimas"),
    ("codigo", "código"),
    ("codigos", "códigos"),
    ("metodo", "método"),
    ("metodos", "métodos"),
    ("analisis", "análisis"),
    ("telefono", "teléfono"),
    ("musica", "música"),
    ("pais", "país"),
    ("paises", "países"),
    ("grafico", "gráfico"),
    ("graficos", "gráficos"),
    ("grafica", "gráfica"),
    ("graficas", "gráficas"),
    ("matematicas", "matemáticas"),
    ("logica", "lógica"),
    ("basico", "básico"),
    ("basica", "básica"),
    ("titulo", "título"),
    ("titulos", "títulos"),
    ("angulo", "ángulo"),
    ("angulos", "ángulos"),
    ("triangulo", "triángulo"),
    ("rectangulo", "rectángulo"),
    ("circulo", "círculo"),
    ("linea", "línea"),
    ("lineas", "líneas"),
    ("menu", "menú"),
    ("ingles", "inglés"),
    ("frances", "francés"),
    ("aleman", "alemán"),
    ("espanol", "español"),
    ("ano", "año"),
    ("anos", "años"),
    ("nino", "niño"),
    ("ninos", "niños"),
    ("manana", "mañana"),
    ("pequeno", "pequeño"),
    ("pequena", "pequeña"),
    ("diseno", "diseño"),
    ("tamano", "tamaño"),
    ("compania", "compañía"),
    ("energia", "energía"),
    ("economia", "economía"),
    ("tecnologia", "tecnología"),
    ("biologia", "biología"),
    ("geografia", "geografía"),
    ("categoria", "categoría"),
    ("categorias", "categorías"),
    ("exito", "éxito"),
    ("caracteristica", "característica"),
    ("caracteristicas", "características"),
    ("electrico", "eléctrico"),
    ("automatico", "automático"),
    ("tecnico", "técnico"),
    ("unico", "único"),
    ("unica", "única"),
    ("util", "útil"),
    ("utiles", "útiles"),
    ("movil", "móvil"),
    ("arbol", "árbol"),
    ("lapiz", "lápiz"),
    ("examenes", "exámenes"),
    ("imagenes", "imágenes"),
    ("parametro", "parámetro"),
    ("parametros", "parámetros"),
    ("perimetro", "perímetro"),
    ("diametro", "diámetro"),
    ("simbolo", "símbolo"),
    ("simbolos", "símbolos"),
    ("vehiculo", "vehículo"),
    ("articulo", "artículo"),
    ("capitulo", "capítulo"),
    ("oceano", "océano"),
    ("raiz", "raíz"),
    ("tipico", "típico"),
    ("economico", "económico"),
    ("historico", "histórico"),
    ("periodico", "periódico"),
    ("academico", "académico"),
    ("telefonos", "teléfonos"),
    ("medico", "médico"),
    ("maquina", "máquina"),
    ("maquinas", "máquinas"),
    ("busqueda", "búsqueda"),
    ("decada", "década"),
    ("epoca", "época"),
    ("indice", "índice"),
    ("sintesis", "síntesis"),
    ("hipotesis", "hipótesis"),
    ("fisica", "física"),
    ("quimica", "química"),
    ("organico", "orgánico"),
];

/// Las de pregunta: solo con tilde dentro de una pregunta, al empezarla.
const DE_PREGUNTA: &[(&str, &str)] = &[
    ("que", "qué"),
    ("como", "cómo"),
    ("cuando", "cuándo"),
    ("donde", "dónde"),
    ("quien", "quién"),
    ("quienes", "quiénes"),
    ("cual", "cuál"),
    ("cuales", "cuáles"),
    ("cuanto", "cuánto"),
    ("cuanta", "cuánta"),
    ("cuantos", "cuántos"),
    ("cuantas", "cuántas"),
];

/// Palabras con -cion/-sion que se escriben asi (no hay ninguna comun: la
/// lista queda por si sale alguna).
const SIN_TILDE_CION: &[&str] = &[];

/// El arreglo con las mayusculas del original («NUMERO» -> «NÚMERO»).
fn con_mayusculas(original: &str, arreglo: &str) -> String {
    if original.chars().all(|c| !c.is_lowercase()) && original.chars().count() > 1 {
        return arreglo.to_uppercase();
    }
    if original.chars().next().is_some_and(char::is_uppercase) {
        let mut c = arreglo.chars();
        return match c.next() {
            Some(p) => p.to_uppercase().chain(c).collect(),
            None => String::new(),
        };
    }
    arreglo.to_string()
}

/// Las palabras de un texto, con su forma original.
fn palabras(texto: &str) -> Vec<&str> {
    texto
        .split(|c: char| !c.is_alphabetic())
        .filter(|p| !p.is_empty())
        .collect()
}

/// La tilde que le falta a una palabra suelta (sin mirar el contexto).
fn tilde_de(palabra_min: &str) -> Option<String> {
    if !palabra_min.is_ascii() {
        return None;
    }
    if let Some((_, b)) = CON_TILDE.iter().find(|(a, _)| *a == palabra_min) {
        return Some((*b).to_string());
    }
    for (fin, con) in [("cion", "ción"), ("sion", "sión")] {
        if palabra_min.len() > fin.len() + 1
            && palabra_min.ends_with(fin)
            && !SIN_TILDE_CION.contains(&palabra_min)
        {
            return Some(format!(
                "{}{con}",
                &palabra_min[..palabra_min.len() - fin.len()]
            ));
        }
    }
    None
}

impl Ortografia for OrtografiaBasica {
    fn revisar(&self, texto: &str, idioma: &str) -> Vec<Falta> {
        if idioma != "es" {
            return Vec::new();
        }
        let ps = palabras(texto);
        let min: Vec<String> = ps.iter().map(|p| p.to_lowercase()).collect();
        let mut faltas: Vec<Falta> = Vec::new();
        let mut poner = |p: &str, arreglo: String| {
            if !faltas.iter().any(|f| f.palabra == p) {
                faltas.push(Falta {
                    palabra: p.to_string(),
                    arreglo: Some(con_mayusculas(p, &arreglo)),
                });
            }
        };
        for (i, p) in ps.iter().enumerate() {
            if let Some(a) = tilde_de(&min[i]) {
                poner(p, a);
                continue;
            }
            // «esta» delante de un participio, un gerundio o «bien/mal/en».
            if min[i] == "esta" || min[i] == "estan" {
                let sig = min.get(i + 1).map(String::as_str).unwrap_or("");
                let verbal = [
                    "ando", "iendo", "ado", "ada", "ados", "adas", "ido", "ida", "idos", "idas",
                ]
                .iter()
                .any(|f| sig.len() > f.len() + 1 && sig.ends_with(f))
                    || [
                        "bien", "mal", "en", "listo", "lista", "aqui", "alli", "claro",
                    ]
                    .contains(&sig);
                if verbal || (sig.is_empty() && texto.trim_end().ends_with(['.', '?', '!'])) {
                    poner(
                        p,
                        if min[i] == "esta" {
                            "está".into()
                        } else {
                            "están".into()
                        },
                    );
                }
            }
        }
        // Las de pregunta: la primera palabra tras «¿» (o la primera, si
        // el texto acaba en «?» sin abrirla).
        for trozo in texto
            .split('¿')
            .skip(if texto.contains('¿') { 1 } else { 0 })
        {
            if !texto.contains('¿') && !texto.trim_end().ends_with('?') {
                break;
            }
            let ps = palabras(trozo);
            let primera = if ps.first().is_some_and(|p| p.eq_ignore_ascii_case("por")) {
                ps.get(1)
            } else {
                ps.first()
            };
            if let Some(p) = primera
                && let Some((_, b)) = DE_PREGUNTA.iter().find(|(a, _)| a.eq_ignore_ascii_case(p))
            {
                poner(p, (*b).to_string());
            }
        }
        faltas
    }
}

/// Corre `f` con el corrector instalado, o con el basico.
fn con_ortografia<R>(f: impl FnOnce(&dyn Ortografia) -> R) -> R {
    match ORTOGRAFIA.read() {
        Ok(g) => match g.as_deref() {
            Some(o) => f(o),
            None => f(&OrtografiaBasica),
        },
        Err(_) => f(&OrtografiaBasica),
    }
}

// ---------------------------------------------------------------------------
// Idioma

/// **En que idioma esta un texto**, si se nota: «zh», «ja», «ko» por su
/// escritura; «es» o «en» por sus palabras de siempre (y la ñ, las tildes,
/// -cion frente a -tion). `None` si es corto o no se sabe («A», «x²», «42»).
pub fn idioma_de(texto: &str) -> Option<&'static str> {
    let (mut han, mut kana, mut hangul, mut letras) = (0usize, 0usize, 0usize, 0usize);
    for c in texto.chars() {
        match c as u32 {
            0x4E00..=0x9FFF | 0x3400..=0x4DBF => han += 1,
            0x3040..=0x30FF => kana += 1,
            0xAC00..=0xD7AF => hangul += 1,
            _ if c.is_alphabetic() => letras += 1,
            _ => {}
        }
    }
    let cjk = han + kana + hangul;
    if cjk > 0 && cjk * 3 >= letras + cjk {
        return Some(if kana > 0 {
            "ja"
        } else if hangul > han {
            "ko"
        } else {
            "zh"
        });
    }
    const ES: &[&str] = &[
        "el",
        "la",
        "los",
        "las",
        "de",
        "del",
        "que",
        "y",
        "en",
        "un",
        "una",
        "es",
        "por",
        "con",
        "para",
        "se",
        "no",
        "al",
        "lo",
        "su",
        "sus",
        "como",
        "pero",
        "este",
        "esta",
        "entre",
        "cuando",
        "muy",
        "sin",
        "sobre",
        "hay",
        "donde",
        "desde",
        "todo",
        "todos",
        "otro",
        "otra",
        "son",
        "fin",
        "inicio",
        "si",
        "datos",
        "resultado",
        "paso",
    ];
    const EN: &[&str] = &[
        "the", "of", "and", "to", "in", "is", "it", "that", "for", "on", "with", "as", "was",
        "are", "be", "this", "by", "at", "from", "or", "an", "not", "have", "has", "which", "you",
        "we", "they", "can", "will", "all", "end", "start", "step", "data", "result",
    ];
    let mut es = 0i32;
    let mut en = 0i32;
    for p in palabras(texto) {
        let m = p.to_lowercase();
        if m.contains(['ñ', 'á', 'é', 'í', 'ó', 'ú', '¿', '¡']) {
            es += 2;
        }
        if ES.contains(&m.as_str()) {
            es += 1;
        }
        if EN.contains(&m.as_str()) {
            en += 1;
        }
        if m.len() > 5 && (m.ends_with("cion") || m.ends_with("ción") || m.ends_with("ciones")) {
            es += 1;
        }
        if m.len() > 5 && (m.ends_with("tion") || m.ends_with("tions") || m.ends_with("ing")) {
            en += 1;
        }
        // Las de la lista de tildes son espanolas (menos las que tambien
        // son inglesas).
        if tilde_de(&m).is_some() && !["menu", "area", "radio"].contains(&m.as_str()) {
            es += 1;
        }
    }
    if texto.contains(['¿', '¡']) {
        es += 2;
    }
    match es.cmp(&en) {
        std::cmp::Ordering::Greater if es >= 1 => Some("es"),
        std::cmp::Ordering::Less if en >= 1 => Some("en"),
        _ => None,
    }
}

fn nombre_de_idioma(i: &str) -> &'static str {
    match i {
        "es" => "espanol",
        "en" => "ingles",
        "zh" => "chino",
        "ja" => "japones",
        "ko" => "coreano",
        _ => "otro idioma",
    }
}

// ---------------------------------------------------------------------------
// La revision

type Caja = (f32, f32, f32, f32);

fn area(c: Caja) -> f32 {
    (c.2 - c.0).max(0.0) * (c.3 - c.1).max(0.0)
}

fn cruce(a: Caja, b: Caja) -> Caja {
    (a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3))
}

fn dentro(a: Caja, b: Caja, tol: f32) -> bool {
    a.0 >= b.0 - tol && a.1 >= b.1 - tol && a.2 <= b.2 + tol && a.3 <= b.3 + tol
}

fn distancia_a_caja(p: (f32, f32), c: Caja) -> f32 {
    let dx = (c.0 - p.0).max(0.0).max(p.0 - c.2);
    let dy = (c.1 - p.1).max(0.0).max(p.1 - c.3);
    (dx * dx + dy * dy).sqrt()
}

fn es_texto(e: &Elemento) -> bool {
    matches!(e.figura, Figura::Texto { .. })
}

fn es_caja(e: &Elemento) -> bool {
    matches!(
        e.figura,
        Figura::Rectangulo | Figura::Elipse | Figura::Rombo
    )
}

fn es_marco(e: &Elemento) -> bool {
    matches!(e.figura, Figura::Marco { .. })
}

fn texto_de(e: &Elemento) -> &str {
    match &e.figura {
        Figura::Texto { texto, .. } => texto,
        _ => "",
    }
}

/// **Revisa la escena con el corrector instalado** (o el basico). Con
/// `tocados`, solo lo que nombra algo de ellos (y el idioma, si se tocaron
/// textos).
pub fn revisar(escena: &Escena, tocados: Option<&[u64]>) -> Vec<Hallazgo> {
    con_ortografia(|o| revisar_con(escena, tocados, o))
}

/// [`revisar`] con un corrector dado (para las pruebas y para quien tenga
/// el suyo a mano).
pub fn revisar_con(
    escena: &Escena,
    tocados: Option<&[u64]>,
    orto: &dyn Ortografia,
) -> Vec<Hallazgo> {
    let vivos: Vec<&Elemento> = escena
        .a_la_vista()
        .filter(|e| !e.extras.referencia)
        .collect();
    let toca = |id: u64| tocados.is_none_or(|t| t.contains(&id));
    let mut avisos: Vec<Hallazgo> = Vec::new();

    // Lo que se compara de dos en dos: textos y cajas (los trazos a mano y
    // las fotos se pisan con todo y es a proposito).
    let piezas: Vec<&Elemento> = vivos
        .iter()
        .copied()
        .filter(|e| es_texto(e) || es_caja(e))
        .collect();
    let todos_contra_todos = piezas.len() <= MAX_PARES;
    let par_vale = |a: &Elemento, b: &Elemento| {
        (toca(a.id) || toca(b.id)) && (todos_contra_todos || toca(a.id))
    };

    // 1. Encimados: dos textos que se pisan, o dos cajas que se pisan a
    //    medias (una dentro de otra es a proposito).
    for (i, a) in piezas.iter().enumerate() {
        for b in &piezas[i + 1..] {
            if !par_vale(a, b) {
                continue;
            }
            let (ca, cb) = (a.caja(), b.caja());
            let comun = area(cruce(ca, cb));
            if comun <= 0.0 {
                continue;
            }
            let menor = area(ca).min(area(cb)).max(1.0);
            if es_texto(a) && es_texto(b) && comun > 0.2 * menor {
                avisos.push(aviso(
                    vec![a.id, b.id],
                    format!("#{} y #{}: textos encimados", a.id, b.id),
                ));
            } else if es_caja(a) && es_caja(b) && comun > 0.08 * menor && !dentro(ca, cb, 2.0) && !dentro(cb, ca, 2.0)
                // Dos ovalos que se cortan suelen ser un Venn.
                && !(matches!(a.figura, Figura::Elipse) && matches!(b.figura, Figura::Elipse))
            {
                avisos.push(aviso(
                    vec![a.id, b.id],
                    format!("#{} y #{}: figuras que se pisan a medias", a.id, b.id),
                ));
            }
        }
    }

    // 2. Textos que no caben en su caja (atados o encima de ella).
    for t in vivos.iter().filter(|e| es_texto(e)) {
        if let Some((w, h, cw, ch, c)) = textos::no_cabe(escena, t)
            && (toca(t.id) || toca(c))
        {
            let que = if w > cw + 1.0 {
                format!("sobra {} px a lo ancho", (w - cw).round())
            } else {
                format!("sobra {} px a lo alto", (h - ch).round())
            };
            avisos.push(aviso(
                vec![t.id, c],
                format!("#{} se sale de #{c} ({que}; editar_texto con «ajustar»: «auto», o agranda #{c})", t.id),
            ));
        }
    }

    // 3. Flechas con una punta suelta cerca de algo: iba a ello y no llego.
    for f in vivos
        .iter()
        .filter(|e| matches!(e.figura, Figura::Flecha { .. }))
    {
        let Some(p) = f.puntos().filter(|p| p.len() >= 2) else {
            continue;
        };
        let puntas = [
            ("inicio", p[0], &f.extras.enganche_inicio),
            ("fin", p[p.len() - 1], &f.extras.enganche_fin),
        ];
        for (nombre, punta, enganche) in puntas {
            if let Some(en) = enganche {
                let esta = vivos
                    .iter()
                    .any(|e| pixpin_motor2d::enlace::id_de_texto_de(e) == en.elemento);
                if !esta && toca(f.id) {
                    avisos.push(aviso(
                        vec![f.id],
                        format!("#{}: el {nombre} apunta a una figura que ya no esta", f.id),
                    ));
                }
                continue;
            }
            let cerca = vivos
                .iter()
                .filter(|e| {
                    e.id != f.id
                        && (es_caja(e) || es_texto(e) || matches!(e.figura, Figura::Imagen { .. }))
                })
                .map(|e| (e, distancia_a_caja((punta.x, punta.y), e.caja())))
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            if let Some((e, d)) = cerca
                && d > 6.0
                && d <= 60.0
                && (toca(f.id) || toca(e.id))
            {
                avisos.push(aviso(
                    vec![f.id, e.id],
                    format!(
                        "#{}: el {nombre} de la flecha se queda a {} px de #{} sin tocarlo",
                        f.id,
                        d.round(),
                        e.id
                    ),
                ));
            }
        }
    }

    // 4. Medio fuera de un marco.
    for m in vivos.iter().filter(|e| es_marco(e)) {
        let cm = m.caja();
        for e in vivos.iter().filter(|e| e.id != m.id && !es_marco(e)) {
            let ce = e.caja();
            if area(cruce(ce, cm)) > 0.0 && !dentro(ce, cm, 2.0) && (toca(e.id) || toca(m.id)) {
                avisos.push(aviso(
                    vec![e.id, m.id],
                    format!("#{} se sale a medias del marco #{}", e.id, m.id),
                ));
            }
        }
    }

    // 5. Casi alineados: dos piezas de la misma talla, una al lado de la
    //    otra, que se quedan a unos px de la misma fila (o columna).
    let sueltas: Vec<&Elemento> = piezas
        .iter()
        .copied()
        .filter(|e| {
            es_caja(e)
                || (e.extras.contenedor.is_none() && textos::caja_que_lo_rodea(escena, e).is_none())
        })
        .collect();
    let mut ya_alineado: Vec<u64> = Vec::new();
    for (i, a) in sueltas.iter().enumerate() {
        for b in &sueltas[i + 1..] {
            if !par_vale(a, b)
                || es_caja(a) != es_caja(b)
                || ya_alineado.contains(&a.id)
                || ya_alineado.contains(&b.id)
            {
                continue;
            }
            let (ca, cb) = (a.caja(), b.caja());
            let (ha, hb) = (ca.3 - ca.1, cb.3 - cb.1);
            let (wa, wb) = (ca.2 - ca.0, cb.2 - cb.0);
            let parecida = |x: f32, y: f32| x.min(y) >= 0.8 * x.max(y);
            let separadas_x = ca.2 <= cb.0 || cb.2 <= ca.0;
            let separadas_y = ca.3 <= cb.1 || cb.3 <= ca.1;
            // Alineados por un borde o por el centro, el que mas se acerque:
            // dos textos pegados a la izquierda tienen centros distintos.
            let menor = |a0: f32, a1: f32, b0: f32, b1: f32| {
                [
                    (a0 - b0).abs(),
                    ((a0 + a1) - (b0 + b1)).abs() / 2.0,
                    (a1 - b1).abs(),
                ]
                .into_iter()
                .fold(f32::MAX, f32::min)
            };
            let dy = menor(ca.1, ca.3, cb.1, cb.3);
            let dx = menor(ca.0, ca.2, cb.0, cb.2);
            let casi = |d: f32| d > 1.5 && d <= 8.0;
            if parecida(ha, hb) && separadas_x && casi(dy) {
                avisos.push(aviso(
                    vec![a.id, b.id],
                    format!(
                        "#{} y #{}: casi en la misma fila ({} px de diferencia en y)",
                        a.id,
                        b.id,
                        dy.round()
                    ),
                ));
                ya_alineado.push(b.id);
            } else if parecida(wa, wb) && separadas_y && casi(dx) {
                avisos.push(aviso(
                    vec![a.id, b.id],
                    format!(
                        "#{} y #{}: casi en la misma columna ({} px de diferencia en x)",
                        a.id,
                        b.id,
                        dx.round()
                    ),
                ));
                ya_alineado.push(b.id);
            }
        }
    }

    // 6. Idioma: los textos en un idioma distinto del de casi todos (lo que
    //    quedo sin traducir), y las faltas de ortografia.
    let con_idioma: Vec<(&Elemento, Option<&'static str>)> = vivos
        .iter()
        .copied()
        .filter(|e| es_texto(e))
        .map(|e| (e, idioma_de(texto_de(e))))
        .collect();
    let mut cuenta: Vec<(&str, usize)> = Vec::new();
    for (_, i) in &con_idioma {
        if let Some(i) = i {
            match cuenta.iter_mut().find(|(x, _)| x == i) {
                Some(c) => c.1 += 1,
                None => cuenta.push((i, 1)),
            }
        }
    }
    let sabidos: usize = cuenta.iter().map(|c| c.1).sum();
    let dominante = cuenta
        .iter()
        .max_by_key(|c| c.1)
        .filter(|c| sabidos >= 2 && c.1 * 10 >= sabidos * 6 && c.1 < sabidos)
        .map(|c| c.0);
    let se_tocaron_textos =
        tocados.is_none_or(|t| t.iter().any(|id| escena.buscar(*id).is_some_and(es_texto)));
    if let Some(dom) = dominante
        && se_tocaron_textos
    {
        let otros: Vec<u64> = con_idioma
            .iter()
            .filter(|(_, i)| i.is_some_and(|i| i != dom))
            .map(|(e, _)| e.id)
            .collect();
        if !otros.is_empty() {
            let primero = con_idioma
                .iter()
                .find(|(e, _)| e.id == otros[0])
                .and_then(|x| x.1)
                .unwrap_or("?");
            let ids: Vec<String> = otros.iter().take(8).map(|i| format!("#{i}")).collect();
            avisos.push(Hallazgo {
                ids: otros.clone(),
                texto: format!(
                    "{} sigue{} en {} y el resto esta en {}",
                    ids.join(" "),
                    if otros.len() == 1 { "" } else { "n" },
                    nombre_de_idioma(primero),
                    nombre_de_idioma(dom)
                ),
                de_idioma: true,
            });
        }
    }
    let todo_es = cuenta.iter().max_by_key(|c| c.1).map(|c| c.0);
    for (e, i) in &con_idioma {
        if !toca(e.id) {
            continue;
        }
        let idioma = i.or(todo_es).unwrap_or("");
        if idioma.is_empty() {
            continue;
        }
        let faltas = orto.revisar(texto_de(e), idioma);
        if faltas.is_empty() {
            continue;
        }
        let lista: Vec<String> = faltas
            .iter()
            .take(4)
            .map(|f| match &f.arreglo {
                Some(a) => format!("«{}»→«{a}»", f.palabra),
                None => format!("«{}»", f.palabra),
            })
            .collect();
        avisos.push(aviso(
            vec![e.id],
            format!("#{} ortografia: {}", e.id, lista.join(", ")),
        ));
    }

    if let Some(t) = tocados {
        avisos.retain(|a| a.de_idioma || a.ids.iter().any(|id| t.contains(id)));
    }
    avisos
}

/// **Los avisos en pocas lineas** para pegarlos al resultado de una
/// herramienta. Vacio si no hay nada.
pub fn para_el_modelo(avisos: &[Hallazgo]) -> String {
    if avisos.is_empty() {
        return String::new();
    }
    let mut s = String::from("revision automatica (arreglalo, o ignoralo si es a proposito):");
    for a in avisos.iter().take(MAX_LINEAS - 1) {
        s.push_str("\n- ");
        s.push_str(&a.texto);
    }
    if avisos.len() > MAX_LINEAS - 1 {
        s.push_str(&format!(
            "\n(+{} avisos mas)",
            avisos.len() - (MAX_LINEAS - 1)
        ));
    }
    s
}

#[cfg(test)]
mod pruebas;
