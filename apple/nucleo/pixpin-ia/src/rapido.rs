//! **El camino rapido: ordenes simples sin modelo** (14-oct).
//!
//! Regla del usuario: «siempre reducir al maximo la latencia; que sea
//! fluido, rapido y comodo». Medido el 14-oct en el banco, una orden como
//! «abre la galería» tardaba 2-3 s con la nube y 7-20 s con el modelo local
//! (dos vueltas: la llamada y la frase de despues). Para las ordenes cortas
//! y frecuentes no hace falta pensar: este interprete, determinista y en
//! español tolerante (sin tildes, con «por favor», con o sin «la tarea»),
//! las convierte en la llamada a la herramienta de la tabla de
//! [`crate::acciones`] y el agente la hace al instante (<100 ms), con la
//! frase corta de [`crate::acciones::Accion::hecho`] como respuesta.
//!
//! **Si no esta seguro, no hace nada** y contesta el modelo: dos ordenes en
//! una frase («apunta X y dibuja Y»), una fecha que no entiende, algo que no
//! empieza como una orden conocida. **Nunca lo que borra** (quitar una tarea,
//! borrar una lista): eso siempre pasa por el modelo y su confirmacion.

use serde_json::{Map, Value, json};

use crate::acciones::normalizar;
use crate::fechas::{self, Momento};

/// Una orden entendida: que herramienta y con que argumentos.
#[derive(Debug, Clone, PartialEq)]
pub struct Orden {
    pub herramienta: &'static str,
    pub args: Map<String, Value>,
}

impl Orden {
    fn nueva(herramienta: &'static str, args: Value) -> Orden {
        Orden {
            herramienta,
            args: args.as_object().cloned().unwrap_or_default(),
        }
    }
}

/// Lo mas largo que se acepta: una orden rapida es corta.
const LARGO_MAXIMO: usize = 160;

/// Los verbos que, detras de « y », empiezan OTRA orden.
const VERBOS: &[&str] = &[
    "abre", "dibuja", "crea", "haz", "pon", "apunta", "anade", "agrega", "marca", "graba",
    "captura", "borra", "quita", "mueve", "manda", "envia", "busca", "pinea", "saca", "copia",
    "calca", "lee", "dime", "explica", "luego", "despues", "tambien", "ademas", "ensename",
    "muestra",
];

/// El texto con su version normalizada letra a letra (para cortar el
/// original donde casa lo normalizado).
struct Frase {
    original: Vec<char>,
    q: String,
}

impl Frase {
    fn nueva(texto: &str) -> Option<Frase> {
        let original: Vec<char> = texto.chars().collect();
        let q: String = original
            .iter()
            .map(|c| normalizar(&c.to_string()).chars().next().unwrap_or(*c))
            .collect();
        (q.chars().count() == original.len()).then_some(Frase { original, q })
    }

    /// Lo que queda del original tras el prefijo normalizado mas largo de
    /// `prefijos` (que tiene que acabar en palabra).
    fn tras(&self, prefijos: &[&str]) -> Option<String> {
        let mut mejor: Option<usize> = None;
        for p in prefijos {
            if self.q.starts_with(p) {
                let n = p.chars().count();
                let corta_palabra = self.q.chars().nth(n).is_some_and(|c| c.is_alphanumeric());
                if !corta_palabra && mejor.is_none_or(|m| n > m) {
                    mejor = Some(n);
                }
            }
        }
        mejor.map(|n| {
            self.original[n..]
                .iter()
                .collect::<String>()
                .trim()
                .to_string()
        })
    }

    fn es(&self, opciones: &[&str]) -> bool {
        opciones.contains(&self.q.as_str())
    }
}

/// Quita la cortesia y la puntuacion de los bordes.
fn limpiar(texto: &str) -> String {
    let mut s = texto.trim().to_string();
    loop {
        let antes = s.clone();
        let q = normalizar(&s);
        for p in [
            "@asistente",
            "asistente,",
            "oye,",
            "oye",
            "hey",
            "por favor,",
            "por favor",
            "porfa",
            "porfavor",
        ] {
            if q.starts_with(p) && q[p.len()..].starts_with(|c: char| !c.is_alphanumeric()) {
                s = s
                    .chars()
                    .skip(p.chars().count())
                    .collect::<String>()
                    .trim_start_matches([',', ' ', ':'])
                    .to_string();
                break;
            }
        }
        let q = normalizar(&s);
        for p in [
            ", por favor",
            " por favor",
            " porfa",
            ", gracias",
            " gracias",
        ] {
            if q.ends_with(p) {
                let n = s.chars().count() - p.chars().count();
                s = s.chars().take(n).collect();
                break;
            }
        }
        s = s
            .trim()
            .trim_end_matches(['.', '!', '¡'])
            .trim_start_matches(['¡', '¿'])
            .trim()
            .to_string();
        if s == antes {
            return s;
        }
    }
}

/// Si hay otra orden detras de un « y » o de un punto: entonces no es
/// simple.
fn hay_otra_orden(q: &str) -> bool {
    let partes: Vec<&str> = q
        .split(['.', ';', '\n'])
        .filter(|p| !p.trim().is_empty())
        .collect();
    if partes.len() > 1 {
        return true;
    }
    for sep in [
        " y ",
        ", y ",
        ", luego ",
        ", despues ",
        " luego ",
        " despues ",
    ] {
        for (i, _) in q.match_indices(sep) {
            let resto = q[i + sep.len()..].trim_start();
            let palabra = resto
                .split(|c: char| !c.is_alphanumeric())
                .next()
                .unwrap_or("");
            if VERBOS.iter().any(|v| palabra.starts_with(v)) {
                return true;
            }
        }
    }
    false
}

/// Quita un prefijo de conexion del principio («: », «que diga», «llamada»).
fn sin_conector(s: &str, conectores: &[&str]) -> String {
    let mut s = s
        .trim()
        .trim_start_matches([':', '-', ','])
        .trim()
        .to_string();
    let q = normalizar(&s);
    for c in conectores {
        if q.starts_with(c)
            && q[c.len()..]
                .chars()
                .next()
                .is_none_or(|x| !x.is_alphanumeric())
        {
            s = s.chars().skip(c.chars().count()).collect::<String>();
            break;
        }
    }
    s.trim()
        .trim_start_matches([':', '-', ','])
        .trim()
        .trim_matches(['"', '«', '»', '“', '”'])
        .trim()
        .to_string()
}

/// « en la lista X» al final: (resto, lista).
fn lista_al_final(s: &str) -> (String, Option<String>) {
    let q = normalizar(s);
    for marca in [
        " en la lista ",
        " a la lista ",
        " en mi lista ",
        " de la lista ",
        " en la lista de ",
        " a la lista de ",
    ] {
        if let Some(i) = q.rfind(marca) {
            let antes: String = s.chars().take(q[..i].chars().count()).collect();
            let lista: String = s
                .chars()
                .skip(q[..i + marca.len()].chars().count())
                .collect();
            let lista = lista.trim().trim_matches(['"', '«', '»']).to_string();
            if !lista.is_empty() && !antes.trim().is_empty() {
                return (antes.trim().to_string(), Some(lista));
            }
        }
    }
    for marca in [" en el inbox", " al inbox", " en la bandeja"] {
        if q.ends_with(marca) {
            let n = s.chars().count() - marca.chars().count();
            return (
                s.chars().take(n).collect::<String>().trim().to_string(),
                Some("Inbox".into()),
            );
        }
    }
    (s.trim().to_string(), None)
}

/// «Compras del chat Obra» → («Compras», «Obra»): la lista y su chat.
fn partir_chat(lista: Option<String>) -> (Option<String>, Option<String>) {
    let Some(l) = lista else { return (None, None) };
    let q = normalizar(&l);
    for marca in [
        " del chat ",
        " en el chat ",
        " del proyecto ",
        " en el proyecto ",
        " de el chat ",
    ] {
        if let Some(i) = q.find(marca) {
            let lista: String = l.chars().take(q[..i].chars().count()).collect();
            let chat: String = l
                .chars()
                .skip(q[..i + marca.len()].chars().count())
                .collect();
            if !lista.trim().is_empty() && !chat.trim().is_empty() {
                return (
                    Some(lista.trim().to_string()),
                    Some(chat.trim().to_string()),
                );
            }
        }
    }
    (Some(l), None)
}

/// **Interpreta una orden simple.** `None`: no es seguro; que conteste el
/// modelo.
pub fn interpretar(texto: &str, ahora: Momento) -> Option<Orden> {
    interpretar_con(texto, ahora, false)
}

/// [`interpretar`], y con `nube` tambien las ordenes de las herramientas
/// solo para la nube (`acciones::SOLO_NUBE`, 15-oct): con el modelo local
/// el camino rapido sigue como estaba («la IA local, no la toques»).
pub fn interpretar_con(texto: &str, ahora: Momento, nube: bool) -> Option<Orden> {
    let limpio = limpiar(texto);
    if limpio.is_empty() || limpio.chars().count() > LARGO_MAXIMO {
        return None;
    }
    let pregunta = limpio.ends_with('?') || texto.trim_start().starts_with('¿');
    let limpio = limpio.trim_end_matches('?').trim().to_string();
    let f = Frase::nueva(&limpio)?;
    let q = f.q.as_str();
    if hay_otra_orden(q) {
        return None;
    }

    // --- **El tamaño de una plantilla** (25-oct-2026, el usuario: «le dije
    // redúcelo a solo 10 semanas y se demoro cinco minutos… y solo borro
    // las columnas»): «N semanas», «N actividades», «del X al Y» se rehacen
    // con el codigo de la plantilla, al instante. Si en el lienzo no hay
    // plantilla que cambiar, la herramienta lo dice y contesta el modelo.
    if !pregunta && let Some(o) = tamano_de_plantilla(q) {
        return Some(o);
    }

    // --- Consultar las tareas y lo hecho (lo unico que vale con «?»).
    if let Some(o) = consulta(q) {
        return Some(o);
    }
    if nube && let Some(o) = actividad(q) {
        return Some(o);
    }
    if pregunta {
        return None;
    }

    // --- 15-oct, solo con la nube: voz, pines, grabar la pantalla, PDF,
    // audio, timeline, el chat por su nombre y las ventanas nuevas.
    if nube {
        if let Some(o) = herramientas_de_la_app(&f) {
            return Some(o);
        }
        for (ventana, nombres) in [
            (
                "ajustes",
                &[
                    "ajustes",
                    "configuracion",
                    "preferencias",
                    "ajustes de pixpin",
                ][..],
            ),
            (
                "timeline",
                &["timeline", "linea de tiempo", "mi timeline"][..],
            ),
            ("buscar", &["buscador", "buscador de pixpin"][..]),
        ] {
            for v in [
                "abre",
                "abrir",
                "muestra",
                "muestrame",
                "ensename",
                "ve a",
                "ir a",
                "abreme",
            ] {
                for art in [
                    "",
                    "la ",
                    "las ",
                    "el ",
                    "los ",
                    "mis ",
                    "mi ",
                    "la ventana de ",
                    "la ventana del ",
                ] {
                    if nombres.iter().any(|n| q == format!("{v} {art}{n}")) {
                        return Some(Orden::nueva("abrir_otra_ventana", json!({"cual": ventana})));
                    }
                }
            }
        }
    }

    // --- Ventanas.
    for (ventana, nombres) in [
        (
            "galeria",
            &[
                "galeria",
                "galeria de capturas",
                "las capturas",
                "mis capturas",
            ][..],
        ),
        (
            "tareas",
            &[
                "tareas",
                "ventana de tareas",
                "mis tareas",
                "lista de tareas",
            ][..],
        ),
        (
            "lecciones",
            &["lecciones", "mis lecciones", "lecciones aprendidas"][..],
        ),
        (
            "sincronizar",
            &["sincronizar", "sincronizacion", "ventana de sincronizar"][..],
        ),
    ] {
        for v in [
            "abre",
            "abrir",
            "muestra",
            "muestrame",
            "ensename",
            "ve a",
            "ir a",
            "abreme",
        ] {
            for art in [
                "",
                "la ",
                "las ",
                "mis ",
                "la ventana de ",
                "la ventana de la ",
                "la ventana de las ",
            ] {
                for n in nombres {
                    let n = n
                        .strip_prefix("mis ")
                        .filter(|_| art == "mis ")
                        .unwrap_or(n);
                    if q == format!("{v} {art}{n}") {
                        return Some(Orden::nueva("abrir_ventana", json!({"cual": ventana})));
                    }
                }
            }
        }
    }

    // --- Capturas.
    if f.es(&[
        "captura",
        "capturar",
        "haz una captura",
        "haz una captura de pantalla",
        "captura la pantalla",
        "captura de pantalla",
        "haz una captura de una zona",
        "haz una captura de una zona de la pantalla",
        "pantallazo",
        "haz un pantallazo",
        "toma una captura",
        "saca una captura",
        "captura una zona",
        "recorta la pantalla",
    ]) {
        return Some(Orden::nueva("capturar_pantalla", json!({})));
    }
    if f.es(&[
        "pinea la ultima",
        "pinea la ultima captura",
        "saca la ultima captura",
        "saca como pin la ultima captura",
        "saca la ultima captura como pin",
        "pon la ultima captura como pin",
        "fija la ultima captura",
        "pin de la ultima captura",
        "pinear la ultima captura",
        "muestra la ultima captura como pin",
    ]) {
        return Some(Orden::nueva("pinear_ultima_captura", json!({})));
    }

    // --- Lienzo y notas.
    if let Some(resto) = f.tras(&[
        "crea un lienzo nuevo",
        "crea un lienzo",
        "crear un lienzo",
        "nuevo lienzo",
        "lienzo nuevo",
        "abre un lienzo nuevo",
        "haz un lienzo nuevo",
        "un lienzo nuevo",
    ]) {
        let nombre = sin_conector(
            &resto,
            &[
                "llamado",
                "que se llame",
                "con el nombre",
                "de nombre",
                "nuevo llamado",
            ],
        );
        let mut args = json!({});
        if !nombre.is_empty() {
            args["nombre"] = json!(nombre);
        }
        return Some(Orden::nueva("lienzo_nuevo", args));
    }
    if let Some(resto) = f.tras(&[
        "crea una nota nueva",
        "crea una nota",
        "nueva nota",
        "nota nueva",
        "escribe una nota",
        "haz una nota",
        "anota en una nota",
    ]) {
        let texto = sin_conector(
            &resto,
            &[
                "que diga",
                "que ponga",
                "con el texto",
                "con",
                "diciendo",
                "sobre",
            ],
        );
        let mut args = json!({});
        if !texto.is_empty() {
            args["texto"] = json!(texto);
        }
        return Some(Orden::nueva("nota_nueva", args));
    }

    // --- Grabar una nota de voz.
    if let Some(resto) = f.tras(&[
        "graba una nota de voz",
        "graba nota de voz",
        "grabar una nota de voz",
        "graba un audio",
        "graba audio",
        "nota de voz",
        "empieza a grabar",
        "graba un mensaje de voz",
        "graba",
    ]) {
        let nombre = sin_conector(
            &resto,
            &[
                "llamada",
                "llamado",
                "que se llame",
                "con el nombre",
                "de nombre",
                "sobre",
                "de",
            ],
        );
        // «graba la pantalla» no es una nota de voz.
        if q.starts_with("graba ")
            && !q.contains("voz")
            && !q.contains("audio")
            && !resto.is_empty()
            && !normalizar(&resto).starts_with("llamad")
            && !normalizar(&resto).starts_with("que se llame")
        {
            return None;
        }
        let nombre = if nombre.is_empty() {
            "Nota de voz".to_string()
        } else {
            nombre
        };
        return Some(Orden::nueva("grabar_voz", json!({"nombre": nombre})));
    }

    // --- Listas de tareas.
    if let Some(resto) = f.tras(&[
        "crea una lista de tareas",
        "crea una lista",
        "crea la lista",
        "nueva lista de tareas",
        "nueva lista",
        "lista nueva",
        "haz una lista de tareas",
        "haz una lista",
        "crear una lista",
    ]) {
        let titulo = sin_conector(
            &resto,
            &[
                "llamada",
                "que se llame",
                "con el nombre",
                "de nombre",
                "titulada",
                "para",
            ],
        );
        if titulo.is_empty() || titulo.chars().count() > 60 {
            return None;
        }
        return Some(Orden::nueva("lista_nueva", json!({"titulo": titulo})));
    }

    // --- Marcar (y desmarcar) una tarea, o cambiarle la fecha.
    if let Some(o) = marcar(&f) {
        return Some(o);
    }
    if let Some(o) = aplazar(&f, ahora) {
        return Some(o);
    }

    // --- Apuntar una tarea (con su lista y su fecha, si las dice).
    if let Some(resto) = f.tras(&[
        "crea una tarea",
        "crea la tarea",
        "crea tarea",
        "anade una tarea",
        "anade la tarea",
        "anade tarea",
        "agrega una tarea",
        "agrega la tarea",
        "agrega tarea",
        "apunta la tarea",
        "apunta una tarea",
        "apunta tarea",
        "anota la tarea",
        "anota una tarea",
        "nueva tarea",
        "tarea nueva",
        "pon una tarea",
        "pon la tarea",
        "recuerdame",
        "recordarme",
        "apunta",
        "anota",
        "anade",
        "agrega",
    ]) {
        let r = normalizar(&resto);
        // «apunta en el chat…» es otra cosa.
        if r.starts_with("en el chat") || r.starts_with("en una nota") || r.is_empty() {
            return None;
        }
        let texto = sin_conector(&resto, &["que diga", "de", "para", "llamada", "que"]);
        let (texto, lista) = lista_al_final(&texto);
        let (lista, chat) = partir_chat(lista);
        let (texto, fecha) = con_fecha(&texto, ahora)?;
        let texto = texto.trim_end_matches([',', ':']).trim().to_string();
        if texto.is_empty() || texto.chars().count() > 120 {
            return None;
        }
        let mut args = json!({"texto": texto});
        if let Some(l) = lista {
            args["lista"] = json!(l);
        }
        if let Some(c) = chat {
            args["chat"] = json!(c);
        }
        if let Some(f) = fecha {
            args["fecha"] = json!(f);
        }
        return Some(Orden::nueva("anadir_tarea", args));
    }
    None
}

/// Lo que se refiere a «esto» y no nombra nada (el adjunto mas reciente).
fn es_esto(s: &str) -> bool {
    matches!(
        normalizar(s.trim()).as_str(),
        "" | "esto"
            | "eso"
            | "lo"
            | "este"
            | "esta"
            | "el"
            | "la"
            | "este pdf"
            | "el pdf"
            | "esta nota"
            | "este documento"
            | "el documento"
            | "este archivo"
            | "el archivo"
            | "este audio"
            | "el audio"
            | "esta nota de voz"
            | "la nota de voz"
            | "el ultimo"
            | "la ultima"
            | "el ultimo pdf"
            | "el ultimo audio"
            | "esta grabacion"
            | "la grabacion"
    )
}

/// El archivo nombrado tras el verbo («comprime el pdf de la tesis» → «la
/// tesis»), o nada si es «esto».
fn archivo_tras(resto: &str) -> Option<String> {
    let s = sin_conector(resto, &[]);
    (!es_esto(&s) && s.chars().count() <= 80).then_some(s)
}

/// **Las herramientas de la app del 15-oct** sin modelo: grabar la
/// pantalla, los pines (no cerrarlos: eso pregunta), leer en voz alta y
/// callarla, comprimir un PDF, transcribir un audio, apuntar en el timeline
/// y abrir un chat por su nombre.
fn herramientas_de_la_app(f: &Frase) -> Option<Orden> {
    let q = f.q.as_str();
    let args = |k: &str, v: Option<String>| {
        let mut m = Map::new();
        if let Some(v) = v {
            m.insert(k.to_string(), json!(v));
        }
        Value::Object(m)
    };
    // Grabar la pantalla (antes que «graba», la nota de voz).
    if f.es(&[
        "graba la pantalla",
        "grabar la pantalla",
        "graba pantalla",
        "graba un gif",
        "graba la pantalla en gif",
        "graba un gif de la pantalla",
        "haz un gif",
        "haz un gif de la pantalla",
        "graba lo que hago",
        "graba una zona de la pantalla",
    ]) {
        return Some(Orden::nueva("grabar_pantalla", json!({})));
    }
    // Capturas con modo.
    for (modo, frases) in [
        (
            "texto",
            &[
                "copia el texto de la pantalla",
                "captura el texto",
                "captura texto",
                "copia texto de la pantalla",
                "lee el texto de la pantalla",
            ][..],
        ),
        (
            "scroll",
            &[
                "captura larga",
                "haz una captura larga",
                "captura con scroll",
                "captura con desplazamiento",
            ][..],
        ),
        (
            "repetir",
            &[
                "repite la captura",
                "repite la ultima captura",
                "captura la misma zona",
                "otra vez la misma captura",
            ][..],
        ),
        (
            "copiar",
            &[
                "captura y copia",
                "captura al portapapeles",
                "copia una captura",
            ][..],
        ),
        (
            "en_vivo",
            &["pin en vivo", "haz un pin en vivo", "captura en vivo"][..],
        ),
    ] {
        if f.es(frases) {
            return Some(Orden::nueva("capturar_con_modo", json!({ "modo": modo })));
        }
    }
    // Los pines (cerrarlos todos pregunta: va por el modelo).
    for (op, frases) in [
        (
            "ocultar_mostrar",
            &[
                "oculta los pines",
                "esconde los pines",
                "muestra los pines",
                "ensename los pines",
                "oculta todos los pines",
                "esconde todos los pines",
                "vuelve a mostrar los pines",
            ][..],
        ),
        (
            "devolver_ultimo",
            &[
                "devuelve el ultimo pin",
                "recupera el ultimo pin",
                "restaura el ultimo pin",
                "vuelve a abrir el ultimo pin",
            ][..],
        ),
        (
            "paso_de_clics",
            &[
                "que los pines dejen pasar los clics",
                "pines transparentes al clic",
            ][..],
        ),
    ] {
        if f.es(frases) {
            return Some(Orden::nueva("manejar_pines", json!({ "operacion": op })));
        }
    }
    // Callar la voz.
    if f.es(&[
        "para de leer",
        "deja de leer",
        "callate",
        "para la lectura",
        "deten la lectura",
        "para la voz",
        "deja de hablar",
        "silencio",
    ]) {
        return Some(Orden::nueva("parar_lectura", json!({})));
    }
    // «Ya esta bien, deja de leer»: lo de delante es solo el tono.
    if q.chars().count() <= 40
        && ["deja de leer", "para de leer", "callate", "deja de hablar"]
            .iter()
            .any(|f| q.ends_with(f))
    {
        return Some(Orden::nueva("parar_lectura", json!({})));
    }
    // Leer en voz alta: «lee esto en voz alta», «lee en voz alta la tesis».
    for fin in [" en voz alta", " en alto"] {
        if q.ends_with(fin) {
            let n = f.original.len() - fin.chars().count();
            let corto = Frase::nueva(&f.original[..n].iter().collect::<String>())?;
            if let Some(resto) = corto.tras(&["lee", "leeme", "leelo", "leela"]) {
                return Some(Orden::nueva(
                    "leer_en_voz_alta",
                    args("archivo", archivo_tras(&resto)),
                ));
            }
        }
    }
    if let Some(resto) = f.tras(&[
        "lee en voz alta",
        "leeme en voz alta",
        "leelo en voz alta",
        "leela en voz alta",
    ]) {
        return Some(Orden::nueva(
            "leer_en_voz_alta",
            args("archivo", archivo_tras(&resto)),
        ));
    }
    // Comprimir un PDF.
    if let Some(resto) = f.tras(&[
        "comprime",
        "comprimeme",
        "aligera",
        "aligerame",
        "reduce",
        "achica",
    ]) {
        let r = normalizar(&resto);
        if r.contains("pdf") || es_esto(&resto) {
            let resto = sin_conector(
                &resto,
                &["el pdf de", "el pdf", "este pdf", "un pdf", "pdf"],
            );
            return Some(Orden::nueva(
                "comprimir_pdf",
                args("archivo", archivo_tras(&resto)),
            ));
        }
    }
    // Pasar un audio a texto.
    if let Some(resto) = f.tras(&[
        "transcribe",
        "transcribeme",
        "pasa a texto",
        "pasame a texto",
        "convierte a texto",
    ]) {
        let resto = sin_conector(
            &resto,
            &[
                "el audio de",
                "la nota de voz de",
                "el audio",
                "la nota de voz",
                "este audio",
                "esta nota de voz",
            ],
        );
        return Some(Orden::nueva(
            "transcribir_audio",
            args("archivo", archivo_tras(&resto)),
        ));
    }
    // Apuntar en el timeline (antes que «apunta», la tarea).
    if let Some(resto) = f.tras(&[
        "apunta en el timeline",
        "anota en el timeline",
        "pon en el timeline",
        "agrega al timeline",
        "anade al timeline",
        "apunta en la linea de tiempo",
        "anota en la bitacora",
    ]) {
        let texto = sin_conector(&resto, &["que"]);
        if texto.is_empty() || texto.chars().count() > 300 {
            return None;
        }
        return Some(Orden::nueva(
            "apuntar_en_timeline",
            json!({ "texto": texto }),
        ));
    }
    // Abrir un chat por su nombre.
    if let Some(resto) = f.tras(&[
        "abre el chat",
        "ve al chat",
        "abreme el chat",
        "abre el proyecto",
        "ve al proyecto",
        "abre la conversacion",
    ]) {
        let chat = sin_conector(&resto, &["de", "del", "llamado", "que se llama"]);
        if chat.chars().count() > 60 {
            return None;
        }
        if chat.is_empty() {
            return Some(Orden::nueva("abrir_chat", json!({})));
        }
        return Some(Orden::nueva("ir_al_chat", json!({ "chat": chat })));
    }
    None
}

/// «¿Cuánto hice hoy?», «resumen de la semana», «mi actividad de este mes».
fn actividad(q: &str) -> Option<Orden> {
    const EMPIEZAN: &[&str] = &[
        "cuanto hice",
        "cuanto he hecho",
        "cuanto trabaje",
        "cuanto he trabajado",
        "resumen de hoy",
        "resumen del dia",
        "resumen de la semana",
        "resumen del mes",
        "mi actividad",
        "actividad de hoy",
        "que tal fue mi dia",
        "estadisticas",
        "mis estadisticas",
        "cuantas capturas hice",
        "cuantos mensajes",
    ];
    let e = EMPIEZAN
        .iter()
        .filter(|e| q.starts_with(*e))
        .max_by_key(|e| e.len())?;
    let resto = &q[e.len()..];
    let periodo = if q.contains("semana") {
        "semana"
    } else if q.contains(" mes") {
        "mes"
    } else {
        "hoy"
    };
    let sobra = resto.split(|c: char| !c.is_alphanumeric()).any(|p| {
        !p.is_empty()
            && !matches!(
                p,
                "hoy"
                    | "de"
                    | "del"
                    | "esta"
                    | "este"
                    | "la"
                    | "el"
                    | "semana"
                    | "mes"
                    | "dia"
                    | "en"
                    | "pixpin"
                    | "con"
                    | "escribi"
            )
    });
    (!sobra).then(|| Orden::nueva("resumen_de_actividad", json!({ "periodo": periodo })))
}

/// La fecha del final de una frase, como la lee `tareas::resolver`: la marca
/// `AAAA-MM-DD HH:MM` si se entiende, o **la cola tal cual** si parece una
/// fecha pero no existe o ya paso (el resolver dira por que, sin modelo).
/// `None`: algo raro; que conteste el modelo.
fn con_fecha(texto: &str, ahora: Momento) -> Option<(String, Option<String>)> {
    match fechas::separar(texto, ahora) {
        Ok((t, Some(m))) => {
            let (y, mo, d) = fechas::fecha_de(m.dia);
            Some((
                t,
                Some(format!(
                    "{y:04}-{mo:02}-{d:02} {:02}:{:02}",
                    m.minuto / 60,
                    m.minuto % 60
                )),
            ))
        }
        Ok((t, None)) => Some((t, None)),
        Err(fechas::ErrorFecha::NoExiste(cola) | fechas::ErrorFecha::Pasada(cola)) => {
            let t = texto.split_whitespace().collect::<Vec<_>>().join(" ");
            let t = t.trim_end_matches(['.', ',', ';']).trim_end();
            let antes = t.strip_suffix(cola.as_str())?.trim_end().to_string();
            Some((antes, Some(cola)))
        }
        Err(fechas::ErrorFecha::NoEntiendo(_)) => None,
    }
}

/// «aplaza la tarea X al viernes», «pasa la tarea X a mañana», «cambia la
/// fecha de X al 20/10»: otra fecha para una tarea. Sin fecha detras («pasa
/// la tarea X a la lista Obra») no es esto: al modelo.
fn aplazar(f: &Frase, ahora: Momento) -> Option<Orden> {
    let resto = f.tras(&[
        "aplaza la tarea",
        "aplaza",
        "posterga la tarea",
        "posterga",
        "pospon la tarea",
        "pospon",
        "cambia la fecha de la tarea",
        "cambia la fecha de",
        "pasa la tarea",
        "mueve la tarea",
        "retrasa la tarea",
        "retrasa",
        "adelanta la tarea",
        "adelanta",
    ])?;
    let resto = sin_conector(&resto, &["la tarea"]);
    let (tarea, fecha) = con_fecha(&resto, ahora)?;
    let fecha = fecha?;
    let tarea = tarea.trim_end_matches([',', ':']).trim().to_string();
    if tarea.is_empty() || tarea.chars().count() > 120 {
        return None;
    }
    Some(Orden::nueva(
        "editar_tarea",
        json!({"tarea": tarea, "fecha": fecha}),
    ))
}

/// «marca como hecha la tarea X», «tacha X», «he terminado X», «desmarca X»,
/// «marca X como pendiente».
fn marcar(f: &Frase) -> Option<Orden> {
    let (resto, mut hecha) = if let Some(r) = f.tras(&[
        "marca como hecha la tarea",
        "marca como hecha",
        "marca como completada la tarea",
        "marca como completada",
        "marca como terminada",
        "marca la tarea",
        "marca",
        "tacha la tarea",
        "tacha",
        "he terminado la tarea",
        "he terminado",
        "ya termine",
        "termine la tarea",
        "termine",
        "ya hice",
        "he hecho",
        "completa la tarea",
        "completada la tarea",
    ]) {
        (r, true)
    } else {
        let r = f.tras(&[
            "desmarca la tarea",
            "desmarca",
            "marca como pendiente la tarea",
            "marca como pendiente",
        ])?;
        (r, false)
    };
    let mut resto = sin_conector(&resto, &["la tarea", "que"]);
    let q = normalizar(&resto);
    let mut explicito = !f.q.starts_with("marca la tarea")
        && !(f.q.starts_with("marca ") && !f.q.starts_with("marca como"));
    for (fin, h) in [
        (" como hecha", true),
        (" como completada", true),
        (" como terminada", true),
        (" de hecha", true),
        (" hecha", true),
        (" como pendiente", false),
        (" como no hecha", false),
    ] {
        if q.ends_with(fin) {
            let n = resto.chars().count() - fin.chars().count();
            resto = resto.chars().take(n).collect::<String>().trim().to_string();
            hecha = h;
            explicito = true;
            break;
        }
    }
    // «marca la tarea X» a secas no dice si hecha o pendiente: al modelo.
    if !explicito {
        return None;
    }
    let (tarea, lista) = lista_al_final(&resto);
    let (lista, chat) = partir_chat(lista);
    if tarea.is_empty() || tarea.chars().count() > 120 {
        return None;
    }
    let mut args = json!({"tarea": tarea, "hecha": hecha});
    if let Some(l) = lista {
        args["lista"] = json!(l);
    }
    if let Some(c) = chat {
        args["chat"] = json!(c);
    }
    Some(Orden::nueva("marcar_tarea", args))
}

/// «¿que tengo pendiente?», «¿que vencio esta semana?», «mis tareas de hoy».
fn consulta(q: &str) -> Option<Orden> {
    const EMPIEZAN: &[&str] = &[
        "que tengo pendiente",
        "que tengo por hacer",
        "que tengo que hacer",
        "que tareas tengo",
        "que me falta",
        "que me queda",
        "mis tareas",
        "tareas pendientes",
        "mis pendientes",
        "lo pendiente",
        "que hay pendiente",
        "que tareas me quedan",
        "que tareas hay",
        "que vencio",
        "que vencieron",
        "que ha vencido",
        "que han vencido",
        "que tareas vencieron",
        "que tareas han vencido",
        "que tareas vencen",
        "que vence",
        "tareas vencidas",
        "que tengo vencido",
        "que tareas me vencieron",
        "que tareas se me vencieron",
        "que se me vencio",
        "que me vencio",
        "que se me paso",
        "que tengo atrasado",
        "tareas atrasadas",
        "que hice",
        "que he hecho",
        "tareas hechas",
        "que tareas hice",
        "cuantas tareas tengo",
        "dime mis tareas",
        "dime que tengo pendiente",
        "lista mis tareas",
    ];
    if !EMPIEZAN.iter().any(|e| q.starts_with(e)) {
        return None;
    }
    let estado = if q.contains("venc") || q.contains("atrasad") {
        "vencidas"
    } else if q.contains("hice") || q.contains("hecho") || q.contains("hechas") {
        "hechas"
    } else {
        "pendientes"
    };
    let periodo = if q.contains("hoy") {
        "hoy"
    } else if q.contains("semana") {
        "semana"
    } else if q.contains(" mes") {
        "mes"
    } else {
        "todo"
    };
    let largo = EMPIEZAN
        .iter()
        .filter(|e| q.starts_with(*e))
        .map(|e| e.len())
        .max()
        .unwrap_or(0);
    let mut resto = format!(" {}", &q[largo..]);
    // «en el chat Obra», «de la lista Compras»: el filtro (por su nombre; el
    // resolver lo busca sin tildes ni mayusculas). Del final hacia atras.
    let mut args = json!({"estado": estado, "periodo": periodo});
    loop {
        let marca = [
            (" en el chat ", "chat"),
            (" del chat ", "chat"),
            (" en el proyecto ", "chat"),
            (" del proyecto ", "chat"),
            (" en la lista ", "lista"),
            (" de la lista ", "lista"),
        ]
        .into_iter()
        .filter_map(|(m, campo)| resto.rfind(m).map(|i| (i, m, campo)))
        .max_by_key(|x| x.0);
        let Some((i, m, campo)) = marca else { break };
        let nombre = resto[i + m.len()..].trim().to_string();
        if nombre.is_empty() || args.get(campo).is_some() {
            return None;
        }
        args[campo] = json!(nombre);
        resto.truncate(i);
    }
    // Lo demas que diga de mas no lo entiende el camino rapido: si hay algo
    // mas que el periodo, al modelo.
    let sobra: Vec<&str> = resto
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| {
            !p.is_empty()
                && !matches!(
                    *p,
                    "hoy"
                        | "para"
                        | "esta"
                        | "este"
                        | "semana"
                        | "mes"
                        | "de"
                        | "que"
                        | "pendientes"
                        | "pendiente"
                        | "por"
                        | "hacer"
                        | "tengo"
                        | "aun"
                        | "todavia"
                        | "ya"
                        | "me"
                        | "las"
                        | "tareas"
                        | "en"
                        | "la"
                        | "el"
                        | "vencidas"
                        | "vencido"
                )
        })
        .collect();
    if !sobra.is_empty() {
        return None;
    }
    Some(Orden::nueva("consultar_tareas", args))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ahora() -> Momento {
        crate::tareas::pruebas::ahora()
    }

    fn orden(t: &str) -> Option<(String, Value)> {
        interpretar(t, ahora()).map(|o| (o.herramienta.to_string(), Value::Object(o.args)))
    }

    fn es(t: &str, herramienta: &str, args: Value) {
        assert_eq!(orden(t), Some((herramienta.to_string(), args)), "«{t}»");
    }

    #[test]
    fn las_ordenes_simples_salen_sin_modelo() {
        es(
            "Apunta la tarea: llamar al ingeniero mañana.",
            "anadir_tarea",
            json!({"texto": "llamar al ingeniero", "fecha": "2026-10-15 09:00"}),
        );
        es(
            "crea una tarea comprar pan",
            "anadir_tarea",
            json!({"texto": "comprar pan"}),
        );
        es(
            "Añade la tarea revisar planos para el viernes a las 10",
            "anadir_tarea",
            json!({"texto": "revisar planos", "fecha": "2026-10-16 10:00"}),
        );
        es(
            "añade comprar clavos a la lista Compras",
            "anadir_tarea",
            json!({"texto": "comprar clavos", "lista": "Compras"}),
        );
        es(
            "Añade revisar grietas a la lista Compras del chat Casa Lima.",
            "anadir_tarea",
            json!({"texto": "revisar grietas", "lista": "Compras", "chat": "Casa Lima"}),
        );
        es(
            "recuérdame llamar a Juan mañana a las 5 de la tarde",
            "anadir_tarea",
            json!({"texto": "llamar a Juan", "fecha": "2026-10-15 17:00"}),
        );
        es(
            "por favor, apunta comprar pan y leche",
            "anadir_tarea",
            json!({"texto": "comprar pan y leche"}),
        );
        es(
            "Crea una lista de tareas que se llame Obra.",
            "lista_nueva",
            json!({"titulo": "Obra"}),
        );
        es(
            "Crea una nota nueva que diga: comprar cemento y arena.",
            "nota_nueva",
            json!({"texto": "comprar cemento y arena"}),
        );
        es("nueva nota", "nota_nueva", json!({}));
        es(
            "Crea un lienzo nuevo llamado Croquis.",
            "lienzo_nuevo",
            json!({"nombre": "Croquis"}),
        );
        es("nuevo lienzo", "lienzo_nuevo", json!({}));
        es(
            "Abre la ventana de tareas.",
            "abrir_ventana",
            json!({"cual": "tareas"}),
        );
        es(
            "abre la galería",
            "abrir_ventana",
            json!({"cual": "galeria"}),
        );
        es(
            "muéstrame mis lecciones",
            "abrir_ventana",
            json!({"cual": "lecciones"}),
        );
        es(
            "Haz una captura de una zona de la pantalla.",
            "capturar_pantalla",
            json!({}),
        );
        es("captura", "capturar_pantalla", json!({}));
        es(
            "Saca como pin la ultima captura.",
            "pinear_ultima_captura",
            json!({}),
        );
        es("pinea la última", "pinear_ultima_captura", json!({}));
        es(
            "Graba una nota de voz que se llame reunion.",
            "grabar_voz",
            json!({"nombre": "reunion"}),
        );
        es("graba", "grabar_voz", json!({"nombre": "Nota de voz"}));
        es(
            "marca como hecha la tarea comprar pan",
            "marcar_tarea",
            json!({"tarea": "comprar pan", "hecha": true}),
        );
        es(
            "tacha revisar encofrado",
            "marcar_tarea",
            json!({"tarea": "revisar encofrado", "hecha": true}),
        );
        es(
            "marca comprar pan como hecha",
            "marcar_tarea",
            json!({"tarea": "comprar pan", "hecha": true}),
        );
        es(
            "desmarca pedir cemento",
            "marcar_tarea",
            json!({"tarea": "pedir cemento", "hecha": false}),
        );
        es(
            "¿Qué tengo pendiente?",
            "consultar_tareas",
            json!({"estado": "pendientes", "periodo": "todo"}),
        );
        es(
            "¿qué tareas me vencieron esta semana?",
            "consultar_tareas",
            json!({"estado": "vencidas", "periodo": "semana"}),
        );
        es(
            "¿qué venció esta semana?",
            "consultar_tareas",
            json!({"estado": "vencidas", "periodo": "semana"}),
        );
        es(
            "mis tareas de hoy",
            "consultar_tareas",
            json!({"estado": "pendientes", "periodo": "hoy"}),
        );
        es(
            "¿Qué tareas tengo en el chat Casa Lima?",
            "consultar_tareas",
            json!({"estado": "pendientes", "periodo": "todo", "chat": "casa lima"}),
        );
        es(
            "¿qué tengo pendiente en la lista Compras del chat Obra?",
            "consultar_tareas",
            json!({"estado": "pendientes", "periodo": "todo", "lista": "compras", "chat": "obra"}),
        );
        es(
            "Aplaza la tarea llamar al banco al viernes.",
            "editar_tarea",
            json!({"tarea": "llamar al banco", "fecha": "2026-10-16 09:00"}),
        );
        es(
            "pasa la tarea comprar pan a mañana a las 12",
            "editar_tarea",
            json!({"tarea": "comprar pan", "fecha": "2026-10-15 12:00"}),
        );
        // Una fecha que no existe va tal cual: el resolver dice por que no.
        es(
            "Añade la tarea pagar alquiler para el 31 de febrero",
            "anadir_tarea",
            json!({"texto": "pagar alquiler", "fecha": "para el 31 de febrero"}),
        );
    }

    #[test]
    fn si_no_esta_seguro_contesta_el_modelo() {
        for t in [
            // Dos ordenes en una.
            "Apunta la tarea comprar yeso y dibuja un círculo rojo",
            "crea un lienzo y luego dibuja una casa",
            "abre la galería. Después captura",
            // Lo que borra, nunca.
            "Borra la lista de tareas con codigo m-123.",
            "quita la tarea comprar pan",
            // Preguntas y cosas que no son ordenes simples.
            "¿Cómo hago una captura de pantalla?",
            "¿Dónde veo mis tareas pendientes?",
            "Hola, ¿qué tal?",
            "Dibuja un circulo rojo",
            "Busca en mis lecciones aprendidas las de encofrado.",
            r"Abre el archivo C:\Users\yo\Desktop\plano.pdf",
            // «marca la tarea X» no dice si hecha o pendiente.
            "marca la tarea comprar pan",
            // Un filtro que no entiende, o «pasa la tarea» sin fecha (es mover).
            "¿qué tengo pendiente con prisa?",
            "pasa la tarea comprar pan a la lista Compras",
            "graba la pantalla",
            "apunta en el chat que mañana no vengo",
            "crea una lista",
        ] {
            assert_eq!(orden(t), None, "«{t}» no deberia ir por el camino rapido");
        }
    }

    /// Con la nube (las herramientas de `acciones::SOLO_NUBE`).
    fn en(t: &str, herramienta: &str, args: Value) {
        let o = interpretar_con(t, ahora(), true)
            .map(|o| (o.herramienta.to_string(), Value::Object(o.args)));
        assert_eq!(o, Some((herramienta.to_string(), args)), "«{t}»");
    }

    /// Con el modelo local el camino rapido sigue como en main, y con la
    /// nube lo que pregunta o no se entiende va al modelo.
    #[test]
    fn las_ordenes_nuevas_son_solo_con_la_nube() {
        for t in [
            "Graba la pantalla",
            "Lee esto en voz alta",
            "Comprime este PDF",
            "abre los ajustes",
            "¿Cuánto hice hoy?",
            "abre el chat Obra",
        ] {
            assert_eq!(orden(t), None, "«{t}» con el local");
            assert!(
                interpretar_con(t, ahora(), true).is_some(),
                "«{t}» con la nube"
            );
        }
        // Lo de siempre, igual con los dos.
        for t in ["apunta comprar pan", "abre la galería", "graba"] {
            assert_eq!(
                interpretar(t, ahora()),
                interpretar_con(t, ahora(), true),
                "«{t}»"
            );
        }
        for t in [
            "cierra todos los pines",
            "comprime la carpeta de fotos",
            "¿cuánto hice hoy con el perro?",
        ] {
            assert_eq!(interpretar_con(t, ahora(), true), None, "«{t}»");
        }
    }

    /// 15-oct: las ordenes nuevas de la app, sin modelo.
    #[test]
    fn las_ordenes_de_la_app_salen_sin_modelo() {
        en("Graba la pantalla", "grabar_pantalla", json!({}));
        en("haz un gif de la pantalla", "grabar_pantalla", json!({}));
        en(
            "copia el texto de la pantalla",
            "capturar_con_modo",
            json!({"modo": "texto"}),
        );
        en(
            "haz una captura larga",
            "capturar_con_modo",
            json!({"modo": "scroll"}),
        );
        en(
            "repite la última captura",
            "capturar_con_modo",
            json!({"modo": "repetir"}),
        );
        en(
            "oculta los pines",
            "manejar_pines",
            json!({"operacion": "ocultar_mostrar"}),
        );
        en(
            "devuelve el último pin",
            "manejar_pines",
            json!({"operacion": "devolver_ultimo"}),
        );
        en("Lee esto en voz alta", "leer_en_voz_alta", json!({}));
        en(
            "léeme en voz alta la tesis",
            "leer_en_voz_alta",
            json!({"archivo": "la tesis"}),
        );
        en(
            "lee el informe de cunetas en voz alta",
            "leer_en_voz_alta",
            json!({"archivo": "el informe de cunetas"}),
        );
        en("para de leer", "parar_lectura", json!({}));
        en("Ya está bien, deja de leer.", "parar_lectura", json!({}));
        en("Comprime este PDF", "comprimir_pdf", json!({}));
        en(
            "comprime el pdf de la tesis",
            "comprimir_pdf",
            json!({"archivo": "la tesis"}),
        );
        en("transcribe este audio", "transcribir_audio", json!({}));
        en(
            "pasa a texto la nota de voz de la reunión",
            "transcribir_audio",
            json!({"archivo": "la reunión"}),
        );
        en(
            "Apunta en el timeline que llegó el camión de cemento",
            "apuntar_en_timeline",
            json!({"texto": "llegó el camión de cemento"}),
        );
        en(
            "abre el chat Obra Miraflores",
            "ir_al_chat",
            json!({"chat": "Obra Miraflores"}),
        );
        en("abre el chat", "abrir_chat", json!({}));
        en(
            "abre los ajustes",
            "abrir_otra_ventana",
            json!({"cual": "ajustes"}),
        );
        en(
            "abre el timeline",
            "abrir_otra_ventana",
            json!({"cual": "timeline"}),
        );
        en(
            "¿Cuánto hice hoy?",
            "resumen_de_actividad",
            json!({"periodo": "hoy"}),
        );
        en(
            "resumen de la semana",
            "resumen_de_actividad",
            json!({"periodo": "semana"}),
        );
        // Las de antes no cambian.
        en("graba", "grabar_voz", json!({"nombre": "Nota de voz"}));
        en(
            "apunta comprar pan",
            "anadir_tarea",
            json!({"texto": "comprar pan"}),
        );
    }

    /// Cada orden rapida es una llamada que la tabla valida (nombre y
    /// argumentos), y nada de lo que pregunta antes sale por aqui.
    #[test]
    fn las_ordenes_rapidas_son_llamadas_validas() {
        for t in [
            "Graba la pantalla",
            "copia el texto de la pantalla",
            "oculta los pines",
            "Lee esto en voz alta",
            "para de leer",
            "Comprime este PDF",
            "transcribe este audio",
            "Apunta en el timeline que llegó el camión",
            "abre el chat Obra",
            "abre los ajustes",
            "¿Cuánto hice hoy?",
        ] {
            let o = interpretar_con(t, ahora(), true).unwrap();
            let fila = crate::acciones::por_nombre(o.herramienta)
                .unwrap_or_else(|| panic!("{t}: {}", o.herramienta));
            let v = crate::acciones::validar(fila, &o.args).unwrap_or_else(|e| panic!("{t}: {e}"));
            assert!(v.confirmar.is_none(), "{t}");
        }
    }
}

/// Los numeros de la frase con la palabra que los sigue: «10 semanas» →
/// `(10, "semanas")`.
fn numeros_con_unidad(q: &str) -> Vec<(u32, String)> {
    let palabras: Vec<&str> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .collect();
    palabras
        .windows(2)
        .filter_map(|w| Some((w[0].parse::<u32>().ok()?, w[1].to_string())))
        .collect()
}

/// **«Redúcelo a 10 semanas», «que tenga 30 actividades», «amplíalo del 1 al
/// 40»**: la llamada a `figuras_guardadas` «alargar» con lo pedido (lo que
/// no se dice se queda como esta en el lienzo: lo completa `poner_plantilla`).
fn tamano_de_plantilla(q: &str) -> Option<Orden> {
    let verbo = [
        "reduc",
        "acort",
        "achic",
        "disminu",
        "ampli",
        "agrand",
        "alarg",
        "extiend",
        "aument",
        "hazlo",
        "hazla",
        "ponle",
        "dejalo",
        "dejala",
        "cambia",
        "que tenga",
        "que sean",
    ]
    .iter()
    .any(|v| q.contains(v));
    if !verbo {
        return None;
    }
    // **Si pide algo mas** que el tamaño («10 semanas y abajo una zona gris
    // para anotaciones», 24-oct-2026), lo hace el modelo: aqui se perderia.
    let mas = [
        "zona",
        "anot",
        "nota",
        "gris",
        "color",
        "lateral",
        "costado",
        "lado",
        "abajo",
        "arriba",
        "debajo",
        "encima",
        "agrega",
        "anade",
        "pon ",
        "pone",
        "quita",
        "borra",
        "mueve",
        "sube",
        "baja",
        "seccion",
        "espacio",
        "recuadro",
        "cuadro",
        "titulo",
        "escrib",
        "llena",
        "rellena",
        "actividades:",
        "leyenda",
        "fecha",
    ];
    if mas.iter().any(|m| q.contains(m)) {
        return None;
    }
    let mut args = Map::new();
    args.insert("accion".into(), json!("alargar"));
    for (n, unidad) in numeros_con_unidad(q) {
        if !(1..=999).contains(&n) {
            return None;
        }
        if unidad.starts_with("semana") {
            args.insert("semanas_desde".into(), json!(1));
            args.insert("semanas_hasta".into(), json!(n));
        } else if [
            "actividad",
            "fila",
            "tarea",
            "renglon",
            "linea",
            "hora",
            "dia",
            "punto",
            "vineta",
            "paso",
            "item",
        ]
        .iter()
        .any(|u| unidad.starts_with(u))
        {
            args.insert("desde".into(), json!(1));
            args.insert("hasta".into(), json!(n));
        }
    }
    // «del 5 al 40».
    if let Some(i) = q.find("del ") {
        let resto: Vec<&str> = q[i + 4..].split_whitespace().collect();
        if let (Some(a), Some("al"), Some(b)) = (
            resto.first().and_then(|x| x.parse::<u32>().ok()),
            resto.get(1).copied(),
            resto.get(2).and_then(|x| {
                x.trim_matches(|c: char| !c.is_ascii_digit())
                    .parse::<u32>()
                    .ok()
            }),
        ) {
            args.insert("desde".into(), json!(a));
            args.insert("hasta".into(), json!(b));
        }
    }
    // «de 10 en 10» es partir en hojas: eso lo decide el modelo.
    if q.contains(" en ") && q.contains(" de ") && numeros_con_unidad(q).is_empty() {
        return None;
    }
    let algo = ["semanas_hasta", "hasta"]
        .iter()
        .any(|k| args.contains_key(*k));
    algo.then(|| Orden::nueva(crate::figuras::NOMBRE, Value::Object(args)))
}

#[cfg(test)]
mod pruebas_plantilla {
    use super::*;

    fn args(q: &str) -> Option<Map<String, Value>> {
        tamano_de_plantilla(&normalizar(q)).map(|o| o.args)
    }

    #[test]
    fn si_pide_algo_mas_que_el_tamano_lo_hace_el_modelo() {
        assert!(args("hazlo de 10 semanas y abajo una zona gris para anotaciones").is_none());
        assert!(args("redúcelo a 10 semanas y en el lateral pon un espacio para notas").is_none());
        assert!(args("amplíalo a 30 actividades y agrega una leyenda").is_none());
    }

    #[test]
    fn el_tamano_de_una_plantilla_va_directo_y_sin_modelo() {
        let a = args("Redúcelo a solo 10 semanas").unwrap();
        assert_eq!(a["semanas_hasta"], json!(10));
        assert!(
            !a.contains_key("hasta"),
            "las actividades se quedan como estan"
        );
        let a = args("amplía el cronograma a 30 semanas y 20 actividades").unwrap();
        assert_eq!(
            (a["semanas_hasta"].clone(), a["hasta"].clone()),
            (json!(30), json!(20))
        );
        let a = args("hazlo del 1 al 40").unwrap();
        assert_eq!(
            (a["desde"].clone(), a["hasta"].clone()),
            (json!(1), json!(40))
        );
        // Caso negativo: sin verbo de tamaño, o sin cantidad, contesta el
        // modelo («dibuja 3 arboles» no es cambiar una plantilla).
        assert!(args("dibuja 3 arboles").is_none());
        assert!(args("reduce el cronograma").is_none());
    }
}
