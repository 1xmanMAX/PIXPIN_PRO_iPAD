//! **La palabra magica** (Ajustes → Pinear): copiar `timer`, `todo`,
//! `gastos`, `pizarra` o `hoja` **sola** y pinearla saca la herramienta en
//! vez de un pin de texto.
//!
//! Copia de `crates/pixpin-pin/src/magia.rs` de Windows (que es el puerto de
//! `clipboard/MagicWord.kt` del movil), regla a regla, sin el estado global:
//! las palabras guardadas viajan en `[pinear]` de `pixpinmax.toml`
//! (`palabras_propias` y `palabras`, una por linea `palabra=APP`), y Swift
//! se las pasa a estas funciones (ver [`crate::ajustes_organizar`]).
//!
//! La regla de que la palabra vaya sola no es un capricho: cada palabra
//! magica es una palabra que se deja de poder pinear como texto. Por eso
//! tambien son pocas y raras en aislado.

use std::collections::BTreeMap;

/// Las herramientas, con el nombre que se guarda (el del enum del movil), en
/// el orden del movil (el del menu «Convertir en…» y de Ajustes).
pub const APPS: [&str; 9] = [
    "TIMER",
    "STOPWATCH",
    "CHECKLIST",
    "COUNTER",
    "LEDGER",
    "BOARD",
    "RULETA",
    "DRAW",
    "SHEET",
];

/// Lo mas larga que puede ser una palabra magica (`MagicWord.MAXIMO`).
pub const MAXIMO: usize = 24;

fn app_de(nombre: &str) -> Option<&'static str> {
    let n = nombre.trim().to_uppercase();
    APPS.iter().copied().find(|a| *a == n)
}

/// Las de fabrica, letra por letra las del movil (`POR_DEFECTO`).
pub fn por_defecto() -> BTreeMap<String, &'static str> {
    [
        ("time", "TIMER"),
        ("timer", "TIMER"),
        ("pomodoro", "TIMER"),
        ("temporizador", "TIMER"),
        ("crono", "STOPWATCH"),
        ("cronometro", "STOPWATCH"),
        ("cronómetro", "STOPWATCH"),
        ("stopwatch", "STOPWATCH"),
        ("todo", "CHECKLIST"),
        ("checklist", "CHECKLIST"),
        ("compras", "CHECKLIST"),
        ("tareas", "CHECKLIST"),
        ("count", "COUNTER"),
        ("contador", "COUNTER"),
        ("gastos", "LEDGER"),
        ("cuentas", "LEDGER"),
        ("money", "LEDGER"),
        ("board", "BOARD"),
        ("pizarra", "BOARD"),
        ("ruleta", "RULETA"),
        ("roulette", "RULETA"),
        ("choose", "RULETA"),
        ("sorteo", "RULETA"),
        ("sortear", "RULETA"),
        ("elegir", "RULETA"),
        ("canvas", "DRAW"),
        ("lienzo", "DRAW"),
        ("excalidraw", "DRAW"),
        ("hoja", "SHEET"),
        ("sheet", "SHEET"),
    ]
    .into_iter()
    .map(|(p, m)| (p.to_string(), m))
    .collect()
}

/// Una palabra tal como se guarda, o `None` si nunca podria dispararse
/// (vacia, con blancos o larguisima).
pub fn normalizar(palabra: &str) -> Option<String> {
    let limpia = palabra.trim().to_lowercase();
    if limpia.is_empty() || limpia.chars().count() > MAXIMO {
        return None;
    }
    if limpia.chars().any(char::is_whitespace) {
        return None;
    }
    Some(limpia)
}

/// Lee las palabras guardadas, `palabra=APP` una por linea. `None` es «no
/// lo he tocado» y mandan las de fabrica; una cadena vacia es «ninguna». Lo
/// que no se entiende se salta; si una palabra sale dos veces, la primera.
pub fn leer(guardadas: Option<&str>) -> BTreeMap<String, &'static str> {
    let Some(texto) = guardadas else {
        return por_defecto();
    };
    let mut salida = BTreeMap::new();
    for linea in texto.lines() {
        let Some(corte) = linea.find('=').filter(|c| *c > 0) else {
            continue;
        };
        let Some(palabra) = normalizar(&linea[..corte]) else {
            continue;
        };
        let Some(app) = app_de(&linea[corte + 1..]) else {
            continue;
        };
        salida.entry(palabra).or_insert(app);
    }
    salida
}

/// Como se guardan. Ver [`leer`].
pub fn escribir(palabras: &BTreeMap<String, &'static str>) -> String {
    palabras
        .iter()
        .map(|(p, m)| format!("{p}={m}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Las palabras de una herramienta, en orden alfabetico.
pub fn de_la_app(palabras: &BTreeMap<String, &'static str>, app: &str) -> Vec<String> {
    palabras
        .iter()
        .filter(|(_, m)| **m == app)
        .map(|(p, _)| p.clone())
        .collect()
}

/// Cambia de golpe las palabras de una herramienta y deja las demas. Una
/// palabra solo abre una cosa: darsela a esta se la quita a la que la
/// tuviera; lo que no podria dispararse se cae.
pub fn con_las_de(
    palabras: &BTreeMap<String, &'static str>,
    app: &'static str,
    nuevas: &[&str],
) -> BTreeMap<String, &'static str> {
    let limpias: Vec<String> = nuevas.iter().filter_map(|p| normalizar(p)).collect();
    let mut salida: BTreeMap<String, &'static str> = palabras
        .iter()
        .filter(|(p, m)| **m != app && !limpias.contains(p))
        .map(|(p, m)| (p.clone(), *m))
        .collect();
    for p in limpias {
        salida.insert(p, app);
    }
    salida
}

/// La herramienta que abre `texto`, o `None` si es texto corriente: sola,
/// sin blancos por dentro; mayusculas y espacios alrededor dan igual.
pub fn detectar(texto: &str, palabras: &BTreeMap<String, &'static str>) -> Option<&'static str> {
    let t = texto.trim();
    if t.is_empty() || palabras.is_empty() {
        return None;
    }
    let mas_larga = palabras.keys().map(|k| k.chars().count()).max()?;
    if t.chars().count() > mas_larga || t.chars().any(char::is_whitespace) {
        return None;
    }
    palabras.get(&t.to_lowercase()).copied()
}

// ---------------------------------------------------------------------------
// Para Swift

/// Las herramientas que abre una palabra magica (`TIMER`, `STOPWATCH`…), en
/// el orden de Ajustes → Pinear.
#[uniffi::export]
pub fn apps_de_palabras_magicas() -> Vec<String> {
    APPS.iter().map(|s| s.to_string()).collect()
}

/// Las palabras que abren `app` con lo guardado (`None`: las de fabrica).
#[uniffi::export]
pub fn palabras_magicas_de(guardadas: Option<String>, app: String) -> Vec<String> {
    match app_de(&app) {
        Some(a) => de_la_app(&leer(guardadas.as_deref()), a),
        None => Vec::new(),
    }
}

/// Lo que hay que guardar (`[pinear] palabras`) tras cambiar las de `app`
/// por lo escrito en su caja (separadas por comas o renglones).
#[uniffi::export]
pub fn palabras_magicas_cambiadas(guardadas: Option<String>, app: String, escrito: String) -> String {
    let palabras = leer(guardadas.as_deref());
    let Some(a) = app_de(&app) else {
        return escribir(&palabras);
    };
    let nuevas: Vec<&str> = escrito.split([',', '\n']).collect();
    escribir(&con_las_de(&palabras, a, &nuevas))
}

/// **La herramienta que abre `texto` pineado**, con lo guardado (`None`:
/// las de fabrica). `None` si es texto corriente.
#[uniffi::export]
pub fn palabra_magica(texto: String, guardadas: Option<String>) -> Option<String> {
    detectar(&texto, &leer(guardadas.as_deref())).map(str::to_string)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sola_y_sin_blancos_abre_la_herramienta() {
        assert_eq!(palabra_magica("  Timer ".into(), None).as_deref(), Some("TIMER"));
        assert_eq!(palabra_magica("el time es oro".into(), None), None);
        assert_eq!(palabra_magica("".into(), None), None);
        // Vacia y propia: ninguna.
        assert_eq!(palabra_magica("timer".into(), Some(String::new())), None);
    }

    #[test]
    fn cambiar_las_de_una_quita_la_palabra_a_la_otra() {
        let g = palabras_magicas_cambiadas(None, "DRAW".into(), "hoja, dibujo".into());
        assert_eq!(palabras_magicas_de(Some(g.clone()), "DRAW".into()), vec!["dibujo", "hoja"]);
        assert!(!palabras_magicas_de(Some(g.clone()), "SHEET".into()).contains(&"hoja".to_string()));
        assert_eq!(palabra_magica("Dibujo".into(), Some(g)).as_deref(), Some("DRAW"));
    }

    #[test]
    fn lo_roto_se_salta_y_va_y_vuelve() {
        let m = leer(Some("reloj=TIMER\n=DRAW\nmal\nuna palabra=DRAW\nx=NADA\nreloj=DRAW"));
        assert_eq!(m.len(), 1);
        assert_eq!(m.get("reloj"), Some(&"TIMER"));
        assert_eq!(leer(Some(&escribir(&por_defecto()))), por_defecto());
        assert_eq!(apps_de_palabras_magicas().len(), 9);
    }
}
