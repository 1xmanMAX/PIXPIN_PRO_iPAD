//! **Cuando piensa el asistente** (15-oct-2026, el usuario: «que este
//! apagado por defecto pero que piense en tareas complejas»).
//!
//! Pensar cuesta: con el 2B de este PC, 100-240 s mas por turno (medido el
//! 14-oct); en la nube, segundos y tokens de pago. Y para «dibuja un circulo
//! rojo» no aporta nada. Asi que en «Automatico» se piensa **solo si la
//! tarea es compleja**, y eso lo decide [`clasificar`]: unas reglas fijas
//! sobre el texto, sin modelo, en microsegundos, y las mismas para el modelo
//! local, Claude y DeepSeek ([`decidir`]). «Siempre» y «Nunca» mandan sobre
//! esto, como antes.
//!
//! **Compleja** es, por orden:
//! 1. que lo pida («piensa», «razona», «paso a paso», «a fondo»); y al
//!    reves, «sin pensar» o «no pienses» la apaga aunque parezca compleja;
//! 2. resolver un problema: «resuelve», «demuestra», «despeja», «halla»...;
//!    o calcular con datos de ciencias («calcula la aceleracion...»); o un
//!    enunciado con varias cifras y una pregunta («un tren sale a 80 km/h...
//!    ¿cuando...?»);
//! 3. planificar, analizar, comparar, disenar, optimizar;
//! 4. una hoja, un plano o un documento enteros;
//! 5. varios pasos encadenados (tres ordenes, o dos con «luego» en un
//!    pedido que no es corto; o una lista numerada);
//! 6. un pedido largo (mas de 60 palabras).
//!
//! Lo demas no piensa: dibujar una figura, anotar, abrir, buscar, apuntar
//! una tarea, preguntas cortas. Las ordenes del camino rapido
//! ([`crate::rapido`]) ni llegan al modelo.

use crate::acciones::normalizar;

/// Por que una tarea es compleja (para el registro y las pruebas).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivo {
    /// Lo pidio: «piensa», «paso a paso».
    Pedido,
    /// Un problema de matematicas o de fisica que resolver.
    Problema,
    /// Planificar, analizar, comparar, disenar.
    Planificar,
    /// Una hoja, un plano o un documento enteros.
    Entero,
    /// Varios pasos encadenados.
    Pasos,
    /// Un pedido largo.
    Largo,
}

/// Mas palabras que esto es un pedido largo.
pub const LARGO: usize = 60;

/// **Si piensa este turno.** `ajuste`: lo de Ajustes → Asistente → Pensar
/// (`Some(true)` siempre, `Some(false)` nunca, `None` automatico).
pub fn decidir(ajuste: Option<bool>, pedido: &str) -> bool {
    ajuste.unwrap_or_else(|| clasificar(pedido).is_some())
}

/// **Si el pedido es una tarea compleja**, y por que. `None`: simple.
pub fn clasificar(pedido: &str) -> Option<Motivo> {
    let q = normalizar(pedido);
    let palabras: Vec<&str> = q
        .split(|c: char| !(c.is_alphanumeric() || c == '%' || c == '^'))
        .filter(|p| !p.is_empty())
        .collect();
    let tiene = |frases: &[&str]| frases.iter().any(|f| q.contains(f));
    let es = |lista: &[&str]| palabras.iter().any(|p| lista.contains(p));

    // 1. Lo dicho manda: primero el «no», que «no pienses» no es «piensa».
    if tiene(NO_PENSAR) {
        return None;
    }
    if es(PENSAR) || tiene(PENSAR_FRASES) {
        return Some(Motivo::Pedido);
    }

    // 2. Problemas.
    let cifras = palabras.iter().filter(|p| p.chars().next().is_some_and(|c| c.is_ascii_digit())).count();
    let cuentas = q.contains('=') || q.chars().any(|c| matches!(c, '^' | '√' | '∫' | '²' | '³'));
    let de_ciencias = tiene(CIENCIAS);
    if empieza_por(&palabras, RESOLVER) {
        return Some(Motivo::Problema);
    }
    if empieza_por(&palabras, CALCULAR) && (de_ciencias || cuentas || palabras.len() > 14) {
        return Some(Motivo::Problema);
    }
    let pregunta = es(&["cuanto", "cuanta", "cuantos", "cuantas", "cuando", "cual", "cuales", "donde"]) || q.contains('?');
    if cifras >= 2 && pregunta && palabras.len() >= 12 && (de_ciencias || tiene(UNIDADES)) {
        return Some(Motivo::Problema);
    }

    // 3. Planificar y analizar.
    if empieza_por(&palabras, PLANIFICAR) || tiene(PLANIFICAR_FRASES) {
        return Some(Motivo::Planificar);
    }

    // 4. Enteros.
    if tiene(ENTEROS) {
        return Some(Motivo::Entero);
    }

    // 5. Pasos encadenados.
    let ordenes = palabras.iter().filter(|p| es_orden(p)).count();
    let encadena = tiene(CONECTORES);
    let numerada = q
        .lines()
        .filter(|l| {
            let l = l.trim_start();
            l.chars().next().is_some_and(|c| c.is_ascii_digit()) && l.chars().nth(1).is_some_and(|c| matches!(c, '.' | ')' | '-'))
        })
        .count();
    if ordenes >= 3 || (ordenes >= 2 && encadena && palabras.len() >= 20) || numerada >= 3 {
        return Some(Motivo::Pasos);
    }

    // 6. Largo.
    if palabras.len() > LARGO {
        return Some(Motivo::Largo);
    }
    None
}

/// Si alguna palabra empieza por una de `raices` (con lo que se le pegue:
/// «resuelvelo», «demuestrame»).
fn empieza_por(palabras: &[&str], raices: &[&str]) -> bool {
    palabras.iter().any(|p| raices.iter().any(|r| p.starts_with(r)))
}

/// Una orden: el verbo tal cual o con un pronombre pegado («ponle»,
/// «dibujalo»). Por igualdad y no por raiz: «crea» no es «creativo».
fn es_orden(p: &str) -> bool {
    const PEGADOS: [&str; 10] = ["", "lo", "la", "los", "las", "le", "les", "me", "nos", "te"];
    ORDENES.iter().any(|v| p.strip_prefix(v).is_some_and(|resto| PEGADOS.contains(&resto)))
}

const NO_PENSAR: &[&str] = &["sin pensar", "no pienses", "no lo pienses", "no razones", "contesta rapido", "responde rapido", "rapidito"];

const PENSAR: &[&str] = &[
    "piensa", "piensalo", "pensalo", "piensatelo", "razona", "razonalo", "razonando", "reflexiona", "think",
];

const PENSAR_FRASES: &[&str] = &[
    "paso a paso", "pasito a pasito", "en profundidad", "detenidamente", "con calma",
    "step by step", "think hard", "piensa bien",
];

/// Verbos de resolver: con ellos ya es un problema.
const RESOLVER: &[&str] = &["resuelv", "resolver", "demuestr", "demostrar", "despej", "halla", "deduc", "determina"];

/// Verbos de calcular: un problema solo con algo de ciencias, cuentas o
/// un pedido que no es corto («calcula el 15 % de 80» no piensa).
const CALCULAR: &[&str] = &["calcul", "encuentra", "obten", "simplific", "factoriz", "deriva", "integra"];

const CIENCIAS: &[&str] = &[
    "fuerza", "aceleracion", "velocidad", "masa", "energia", "potencia", "friccion", "rozamiento", "inclinado",
    "proyectil", "pendulo", "polea", "circuito", "resistencia", "voltaje", "corriente", "presion", "caudal",
    "momento", "torque", "integral", "derivada", "limite", "ecuacion", "sistema de", "matriz", "vector",
    "probabilidad", "logaritmo", "volumen", "perimetro", " area ", "hipotenusa", "angulo", "trigonometr",
    "interes compuesto", "estadistic", "promedio", "varianza", "viga", "esfuerzo", "deflexion",
];

const UNIDADES: &[&str] = &[
    "km/h", "m/s", " km", " kg", " cm", " mm", " m ", " m.", " m,", " s ", "segundo", "minuto", " hora", "metro",
    "kilo", "litro", "newton", "julio", "watt", "voltio", "ohm", "grados", "°", "%", "soles", "euros", "dolares",
];

const PLANIFICAR: &[&str] = &[
    "planific", "planea", "analiz", "compara", "disena", "optimiz", "evalua", "justifica",
];

const PLANIFICAR_FRASES: &[&str] = &[
    "haz un plan", "hazme un plan", "un plan de", "plan para", "estrategia", "organiza mi", "organizame", "pros y contras",
    "ventajas y desventajas",
];

const ENTEROS: &[&str] = &[
    "toda la hoja", "hoja entera", "hoja completa", "todas las hojas", "todo el plano", "plano entero",
    "plano completo", "todo el lienzo", "lienzo entero", "todo el documento", "documento entero",
    "documento completo", "todas las paginas", "toda la pagina", "pagina entera", "todo el pdf", "pdf entero",
    "todo el libro", "libro entero", "todo el excel", "toda la tabla", "tabla entera",
];

const CONECTORES: &[&str] = &[
    " luego ", " despues ", "a continuacion", "por ultimo", "finalmente", "primero ", "y entonces", "al final",
];

/// Verbos de orden (en imperativo, sin tildes).
const ORDENES: &[&str] = &[
    "dibuja", "pinta", "pon", "anade", "agrega", "escribe", "calcula", "haz", "crea", "abre", "busca", "cambia",
    "mueve", "borra", "quita", "colorea", "traza", "marca", "resume", "traduce", "convierte", "guarda", "envia",
    "manda", "copia", "ordena", "genera", "rellena", "grafica", "exporta", "inserta", "dime", "explica", "lee",
    "rota", "gira", "agranda", "achica", "alinea", "une", "separa", "duplica", "renombra",
];

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Lo simple: nunca piensa.
    const SIMPLES: &[&str] = &[
        "dibuja un circulo rojo",
        "Dibuja una casa con un árbol",
        "anota esto: llamar a Juan",
        "abre la galería",
        "busca mis notas de la tesis",
        "apunta comprar pan mañana",
        "hola",
        "¿qué hora es?",
        "¿cómo se cambia el color de fondo?",
        "pon un título que diga Plano de la casa",
        "dibuja una flecha de A a B y pon un texto",
        "calcula el 15% de 80",
        "pasa en limpio este problema",
        "haz una gráfica de barras con 3, 5 y 7",
        "dibuja un diagrama de cuerpo libre",
        "¿qué piensas de este color?",
        "crea una tarea para el lunes",
        "traduce hola al inglés",
        "resume este mensaje",
        "busca el problema 3 en mis notas",
    ];

    /// Lo complejo: piensa, y por que.
    const COMPLEJOS: &[(&str, Motivo)] = &[
        ("piensa bien cómo distribuir los muebles", Motivo::Pedido),
        ("explícame paso a paso la regla de la cadena", Motivo::Pedido),
        ("razona la respuesta", Motivo::Pedido),
        ("resuelve x^2 - 5x + 6 = 0", Motivo::Problema),
        ("resuélvelo", Motivo::Problema),
        ("Demuestra que la raíz de 2 es irracional", Motivo::Problema),
        ("calcula la aceleración de un bloque de 5 kg en un plano inclinado de 30°", Motivo::Problema),
        (
            "Un tren sale a las 8 a 80 km/h y otro a las 9 a 100 km/h por la misma vía, ¿cuándo lo alcanza?",
            Motivo::Problema,
        ),
        ("halla la derivada de sin(x)·x²", Motivo::Problema),
        ("planifica la obra de la casa en 12 semanas", Motivo::Planificar),
        ("compara estas dos vigas", Motivo::Planificar),
        ("hazme un plan de estudio para el examen", Motivo::Planificar),
        ("pasa toda la hoja al lienzo", Motivo::Entero),
        ("revisa el plano entero y marca los errores", Motivo::Entero),
        ("dibuja un rectángulo, ponle un título y colorea el fondo de azul", Motivo::Pasos),
        (
            "primero dibuja el plano de la planta baja con sus cuatro habitaciones y luego pon las cotas de cada pared en metros",
            Motivo::Pasos,
        ),
        ("1. abre el plano\n2. marca las puertas\n3. exporta a PDF", Motivo::Pasos),
    ];

    #[test]
    fn lo_simple_no_piensa() {
        for s in SIMPLES {
            assert_eq!(clasificar(s), None, "«{s}» no deberia pensar");
            assert!(!decidir(None, s));
        }
    }

    #[test]
    fn lo_complejo_piensa_y_dice_por_que() {
        for (s, m) in COMPLEJOS {
            assert_eq!(clasificar(s), Some(*m), "«{s}»");
            assert!(decidir(None, s));
        }
    }

    #[test]
    fn un_pedido_largo_piensa() {
        let largo = "quiero que me cuentes ".repeat(16);
        assert!(largo.split_whitespace().count() > LARGO);
        assert_eq!(clasificar(&largo), Some(Motivo::Largo));
        // Caso negativo: el mismo, corto, no.
        assert_eq!(clasificar("quiero que me cuentes"), None);
    }

    #[test]
    fn decir_que_no_piense_manda_sobre_lo_complejo() {
        assert_eq!(clasificar("resuelve x+1=2 sin pensar"), None);
        assert_eq!(clasificar("no pienses, dibuja una casa"), None);
        // «no pienses» no es «piensa».
        assert_eq!(clasificar("no pienses mucho"), None);
    }

    #[test]
    fn siempre_y_nunca_mandan_sobre_el_clasificador() {
        assert!(decidir(Some(true), "hola"));
        assert!(!decidir(Some(false), "resuelve x^2 = 4 paso a paso"));
        // Automatico: el clasificador.
        assert!(!decidir(None, "hola"));
        assert!(decidir(None, "resuelve x^2 = 4"));
    }

    #[test]
    fn las_ordenes_no_se_confunden_con_palabras_parecidas() {
        assert!(es_orden("ponle") && es_orden("dibujalo") && es_orden("crea"));
        assert!(!es_orden("creativo") && !es_orden("pone") && !es_orden("dibujante"));
    }

    #[test]
    fn clasificar_cuesta_menos_de_un_milisegundo() {
        let todos: Vec<&str> = SIMPLES.iter().copied().chain(COMPLEJOS.iter().map(|(s, _)| *s)).collect();
        let vueltas = 200;
        let t = std::time::Instant::now();
        let mut n = 0;
        for _ in 0..vueltas {
            for s in &todos {
                n += usize::from(clasificar(std::hint::black_box(s)).is_some());
            }
        }
        let por_pedido = t.elapsed() / (vueltas * todos.len()) as u32;
        assert_eq!(n, vueltas * COMPLEJOS.len());
        println!("clasificar: {por_pedido:?} por pedido (sin optimizar)");
        assert!(por_pedido < std::time::Duration::from_millis(1), "{por_pedido:?}");
    }
}
