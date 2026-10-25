//! **Que herramientas se le ensenan al modelo en cada turno.**
//!
//! Con todas las de la app (unas 33) un modelo de 2B se pierde: elige mal,
//! mezcla parametros de una con otra, y el catalogo solo ya se come ~3000
//! tokens de cada vuelta. Asi que antes de preguntarle se eligen las pocas
//! que vienen a cuento (6 como mucho) por palabras clave del pedido —las de
//! la tabla de `acciones.rs` y las de aqui para las propias—:
//!
//! - una pregunta sobre la app («¿donde…?», «¿como…?») solo lleva
//!   `ayuda_app`: que explique, no que lo haga;
//! - un saludo o una charla corta no lleva ninguna: contesta con texto;
//! - con una imagen adjunta van primero `calcar_imagen` y `dibujar_en_lienzo`;
//! - si nada encaja, las de siempre (dibujar, grafica y ayuda).

use crate::acciones::normalizar;
use crate::herramientas::{
    AYUDA, CALCAR, CRONOGRAMA, DIBUJAR, ECUACION, FIGURAS, FISICA, FUNCION, GRAFICA, Herramienta, OCR, PROBLEMA, SVG,
    catalogo_base,
};

/// Cuantas herramientas como mucho por turno.
pub const MAXIMO: usize = 6;

/// Las claves de las herramientas propias.
fn claves_base(nombre: &str) -> &'static [&'static str] {
    match nombre {
        DIBUJAR => &[
            "dibuj", "pinta", "traza", "figura", "circulo", "cuadrado", "rectangulo", "triangulo", "estrella",
            "flecha", "linea", "diagrama", "flujo", "esquema", "casa", "muneco", "curva", "onda", "poligono",
            "titulo", "lienzo", "forma", "rombo", "elipse", "organigrama", "mapa", "croquis",
        ],
        GRAFICA => &["grafic", "barras", "pastel", "torta", "estadistic", "porcentaje", "chart", "encuesta", "datos"],
        OCR => &["ocr", "escanead", "texto de", "lee el", "leer el", "extrae", "reconoce"],
        CALCAR => &["calca", "copia", "imagen", "foto", "contorno", "pasa este", "pasa esta", "replica"],
        AYUDA => &["ayuda", "explica", "funciona", "sirve", "donde", "atajo", "menu"],
        SVG => &[
            "ilustra", "svg", "detall", "isometric", "realist", "persona", "animal", "perro", "gato", "arbol", "paisaje",
            "retrato", "a lapiz", "grafito", "boceto", "logo", "icono", "pajaro", "flor", "coche", "edificio",
        ],
        FUNCION => &[
            "funcion", "funciones", "f(x)", "y=", "y =", "parabola", "seno", "coseno", "plano cartesiano", "dominio",
            "x^", "x²", "sin(", "cos(", "recta y",
            // Estudiantes (14-oct): polares, parametricas y lo que se marca.
            "polar", "parametric", "cardioide", "elipse", "raices", "tangente", "asintota", "theta", "θ",
            "r =", "area entre", "extremos", "maximos", "minimos", "interseccion",
        ],
        ECUACION => &[
            "ecuacion", "ecuaciones", "formula", "latex", "fraccion", "raiz cuadrada", "integral", "teorema", "pitagoras",
            "cuadratica", "derivada",
        ],
        CRONOGRAMA => &[
            "cronograma", "gantt", "planificacion", "programacion de obra", "hitos", "plazos", "calendario de",
            "semanas", "fases",
        ],
        FISICA => &[
            "fisica", "fuerza", "cuerpo libre", "plano inclinado", "inclinado", "tiro", "parabolic", "proyectil", "lanzad",
            "circuito", "resistencia", "paralelo", "vector", "pendulo", "atwood", "polea", "mru", "mruv", "mrua",
            "velocidad", "aceleracion", "rozamiento", "friccion", "voltaje", "voltio", "ohm", "newton", "bloque",
        ],
        PROBLEMA => &[
            "problema", "ejercicio", "enunciado", "en limpio", "a limpio", "reescrib", "transcrib", "resuelv", "resolver",
            "examen",
        ],
        FIGURAS => &[
            "guarda esto", "guardalo", "guardala", "como figura", "biblioteca", "figura guardada", "figuras guardadas",
            "mis figuras", "pon el", "pon la", "inserta el", "inserta la", "estampa", "guarda el", "guarda la",
        ],
        crate::documentos::BUSCAR => &[
            "busca en mis", "buscame", "mis notas", "mis documentos", "mis archivos", "mis cosas", "mis chats",
            "mis mensajes", "que dice", "dice la", "dice el", "segun mi", "segun la", "segun el", "tesis", "mande",
            "envie", "guarde", "apunte", "anote", "documento", "informe", "presupuesto", "plano", "memoria",
            "expediente", "contrato", "acta", "notas",
        ],
        _ => &[],
    }
}

/// Si el pedido es sobre lo que el usuario tiene guardado («¿que dice la
/// tesis…?», «¿donde mande…?»): entonces un «¿donde…?» no es una pregunta
/// sobre la app y no se queda solo con la ayuda.
pub fn es_sobre_sus_documentos(q: &str) -> bool {
    let palabras: Vec<&str> = q.split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty()).collect();
    puntuar(claves_base(crate::documentos::BUSCAR), q, &palabras) >= 2
}

/// Lo que puntua un juego de claves en un pedido normalizado.
fn puntuar(claves: &[&str], q: &str, palabras: &[&str]) -> u32 {
    claves
        .iter()
        .map(|c| {
            if c.contains(' ') || c.contains(':') {
                if q.contains(c) { 3 } else { 0 }
            } else if palabras.iter().any(|p| p.starts_with(c)) {
                2
            } else {
                0
            }
        })
        .sum()
}

/// Si el pedido es una pregunta sobre como o donde (no un encargo).
pub fn es_pregunta_de_ayuda(q: &str) -> bool {
    let q = q.trim_start_matches(|c: char| !c.is_alphanumeric());
    ["como ", "donde ", "que es ", "para que ", "cual ", "en que ", "por que ", "que hace", "se puede"]
        .iter()
        .any(|p| q.starts_with(p))
}

/// Un saludo o un «gracias»: sin herramientas.
fn es_charla(q: &str, palabras: &[&str]) -> bool {
    palabras.len() <= 4
        && palabras
            .iter()
            .any(|p| ["hola", "gracias", "buenas", "adios", "chao", "ok", "vale", "genial", "perfecto"].contains(p))
        || q.contains("que tal")
}

/// Si el pedido habla de la imagen adjunta: «calcala», «copia esta imagen»,
/// «dibuja este perrito», «pasalo al lienzo». No: «dibuja un circulo».
pub fn pide_la_imagen(palabras: &[&str]) -> bool {
    const VERBOS: &[&str] = &[
        "dibuj", "copi", "calc", "pasa", "pase", "pint", "replic", "pon", "mete", "lleva", "traza", "haz", "reproduc",
        "imita", "convierte", "vectoriz",
    ];
    const ESTO: &[&str] = &[
        "imagen", "foto", "adjunt", "captura", "este", "esta", "esto", "estos", "estas", "ese", "esa", "eso", "aqui",
    ];
    let verbo = |p: &&str| VERBOS.iter().any(|v| p.starts_with(v));
    let calca = palabras.iter().any(|p| p.starts_with("calc") || p.starts_with("replic") || p.starts_with("vectoriz"));
    // «dibujalo», «copiala», «pasalos»: el pronombre pegado al verbo.
    let con_pronombre = palabras
        .iter()
        .any(|p| verbo(p) && ["lo", "la", "los", "las"].iter().any(|s| p.ends_with(s)) && p.len() > 5);
    let objeto = palabras.iter().any(|p| ESTO.iter().any(|e| p == e || (e.len() > 4 && p.starts_with(e))));
    calca || con_pronombre || (palabras.iter().any(verbo) && objeto)
}

/// Si el pedido habla de una tabla o un Excel («pasa esta tabla a Excel»):
/// con una foto adjunta no es calcarla, es leer sus celdas
/// (`tabla_de_imagen`).
pub fn pide_una_tabla(palabras: &[&str]) -> bool {
    palabras
        .iter()
        .any(|p| p.starts_with("tabla") || p.starts_with("excel") || *p == "xlsx" || p.starts_with("celda"))
}

/// Si el pedido nombra un documento (PDF, Word, plano, archivo...): entonces
/// resaltar o subrayar es anotarlo, no el lienzo.
pub fn habla_de_documentos(palabras: &[&str]) -> bool {
    palabras.iter().any(|p| {
        ["pdf", "documento", "plano", "archivo", "fichero", "word", "excel", "pagina", "adjunt", "docx", "informe"]
            .iter()
            .any(|r| p.starts_with(r))
    })
}

/// Si el pedido es pegar la foto en el lienzo o recortarla («pega la foto y
/// recorta la mitad izquierda»): eso es `imagen_en_lienzo`, no calcarla.
pub fn pide_pegar_o_recortar(palabras: &[&str]) -> bool {
    palabras
        .iter()
        .any(|p| p.starts_with("peg") || p.starts_with("recort") || p.starts_with("insert"))
}

/// Si el pedido sobre una imagen quiere una interpretacion (dibujarla con
/// figuras, a lapiz, con grafito, como boceto) y no una copia exacta.
pub fn pide_interpretar(q: &str) -> bool {
    [
        "con figuras", "con formas", "a lapiz", "con lapiz", "grafito", "boceto", "bosquejo", "interpret", "ilustra",
        "estilo", "caricatura", "a mano", "dibujo de", "en svg", "isometric",
    ]
    .iter()
    .any(|c| q.contains(c))
}

/// **Las herramientas para este pedido.** `con_imagen`: si el mensaje trae
/// una foto (para calcarla).
///
/// Con una imagen y un pedido que habla de ella, **solo `calcar_imagen`**:
/// medido el 14-oct, con «Dibuja este perrito de la imagen adjunta» y las
/// dos ofrecidas, el 2B dibujaba 4 triangulos con `dibujar_en_lienzo` en vez
/// de calcarlo. Sin hablar de ella («dibuja un circulo» con una foto
/// cualquiera en el chat), calcar no tiene premio.
pub fn para_el_pedido(pedido: &str, con_imagen: bool) -> Vec<Herramienta> {
    let base = catalogo_base();
    let de = |n: &str| base.iter().find(|h| h.nombre == n).cloned();
    let q = normalizar(pedido);
    let palabras: Vec<&str> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .collect();
    // La foto de un problema («pasalo en limpio», «resuelve este
    // ejercicio»): no es calcarla ni una pregunta sobre la app; se pasa en
    // limpio, y por si su figura es de fisica o una grafica, esas dos.
    if con_imagen && crate::problema::pide_problema(pedido) {
        return [PROBLEMA, FISICA, FUNCION].iter().filter_map(|n| de(n)).collect();
    }
    if es_pregunta_de_ayuda(&q) {
        // «¿Donde dice JR. ANTISUYO en el plano?» es buscar DENTRO de un
        // documento y llevar ahi (`acciones_documentos`), no la ayuda.
        let dentro: Vec<Herramienta> = crate::acciones_documentos::para_una_pregunta(&q)
            .into_iter()
            .map(crate::acciones::herramienta)
            .collect();
        // «¿Donde mande el plano?» es sobre sus cosas, no sobre la app:
        // primero buscar, y la ayuda por si acaso.
        if es_sobre_sus_documentos(&q) || !dentro.is_empty() {
            let mut v = dentro;
            v.extend([crate::documentos::BUSCAR, AYUDA].iter().filter_map(|n| de(n)));
            v.truncate(MAXIMO);
            return v;
        }
        return de(AYUDA).into_iter().collect();
    }
    // «Pega la foto y recorta la mitad»: es editar el lienzo con ella, no
    // calcarla (`edicion.rs`).
    if con_imagen && pide_la_imagen(&palabras) && !pide_una_tabla(&palabras) && !pide_pegar_o_recortar(&palabras) {
        // «Dibujala con figuras», «a lapiz», «con grafito»: interpretarla,
        // no calcarla. El modelo la ve y la redibuja con las herramientas
        // del lienzo (o en SVG, que da curvas).
        if pide_interpretar(&q) {
            return [SVG, DIBUJAR].iter().filter_map(|n| de(n)).collect();
        }
        return de(CALCAR).into_iter().collect();
    }
    if es_charla(&q, &palabras) {
        return Vec::new();
    }
    let mut puntuadas: Vec<(u32, Herramienta)> = Vec::new();
    for h in &base {
        let s = puntuar(claves_base(&h.nombre), &q, &palabras);
        if h.nombre == CALCAR && !con_imagen {
            // Sin imagen no hay nada que calcar.
            continue;
        }
        puntuadas.push((s, h.clone()));
    }
    // La hoja al lienzo es solo para la nube (decision del usuario, 14-oct):
    // esta seleccion es la del modelo local. Lo mismo las de la app del
    // 15-oct (`acciones::SOLO_NUBE`: «la IA local, no la toques»).
    for a in crate::acciones::todas()
        .filter(|a| a.herramienta != crate::hoja::NOMBRE && !crate::acciones::es_solo_nube(a.herramienta))
    {
        puntuadas.push((puntuar(a.claves, &q, &palabras), crate::acciones::herramienta(a)));
    }
    // Editar lo que ya hay en el lienzo (tabla de `edicion.rs`).
    for f in crate::edicion::filas() {
        puntuadas.push((puntuar(f.claves, &q, &palabras), crate::edicion::herramienta(f)));
    }
    // Estable: a igual puntuacion, el orden del catalogo.
    puntuadas.sort_by(|a, b| b.0.cmp(&a.0));
    // «Resalta el titulo», «subraya el resultado»: sin nombrar un documento,
    // es el lienzo. Con las dos a mano el 2B anotaba un PDF que no habia
    // (medido el 14-oct con `anotar_documento`).
    let edita = puntuadas.iter().any(|(s, h)| *s >= 2 && crate::edicion::fila(&h.nombre).is_some());
    if edita && !habla_de_documentos(&palabras) {
        puntuadas.retain(|(_, h)| !crate::acciones_documentos::es_de_documentos(&h.nombre));
    }
    let mut elegidas: Vec<(u32, Herramienta)> = puntuadas.into_iter().filter(|(s, _)| *s >= 2).take(MAXIMO).collect();
    // «Saca las tareas de esta acta»: con las dos a mano, el 2B las apuntaba
    // una a una en el Inbox y no creaba la lista pedida (medido, 14-oct).
    if let Some(varias) = elegidas.iter().find(|(_, h)| h.nombre == "anadir_tareas").map(|x| x.0) {
        elegidas.retain(|(s, h)| !(h.nombre == "anadir_tarea" && *s <= varias));
    }
    // «Dibuja el tiro parabolico...»: con dibujar a mano, el 2B dibujaba
    // cajas con rotulos en vez de la plantilla (medido, 14-oct). Si una de
    // estudiantes encaja de lleno, la de dibujar a mano sobra.
    if elegidas.iter().any(|(s, h)| *s >= 4 && [FISICA, FUNCION, PROBLEMA].contains(&h.nombre.as_str())) {
        elegidas.retain(|(s, h)| !(h.nombre == DIBUJAR && *s <= 2));
    }
    let elegidas: Vec<Herramienta> = elegidas.into_iter().map(|(_, h)| h).collect();
    if elegidas.is_empty() {
        return [DIBUJAR, GRAFICA, AYUDA].iter().filter_map(|n| de(n)).collect();
    }
    elegidas
}

/// **Si este pedido merece el modelo de mejor vista** (Gemma 4 E2B, ver
/// `modelos::GEMMA4_E2B`): hay una foto y lo que se pide es MIRARLA
/// —describirla, leer lo que pone, entender una escena— y no hacer algo con
/// ella. Para dibujar, calcar o mandar algo a la app sigue el de siempre, que
/// llama herramientas mejor (documento §4.2: Gemma partio un dibujo en
/// varias llamadas) y no hace esperar a cargar 4 GB.
///
/// Cambiar de modelo cuesta la carga del otro (~20 s), asi que la regla es
/// conservadora: sin palabras de mirar, no.
pub fn pide_mejor_vista(pedido: &str, con_foto: bool) -> bool {
    if !con_foto {
        return false;
    }
    let q = normalizar(pedido);
    let palabras: Vec<&str> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .collect();
    let empieza = |raices: &[&str]| palabras.iter().any(|p| raices.iter().any(|r| p.starts_with(r)));
    // Hacer algo con la foto: el de siempre.
    if empieza(&[
        "dibuj", "calca", "copia", "pasa", "traza", "grafic", "lienzo", "anota", "tarea", "lista", "borra",
        "envia", "manda", "comparte", "guarda", "pinea", "recorta",
    ]) {
        return false;
    }
    let frases = [
        "que ves", "que hay", "que es esto", "que es eso", "que dice", "que pone", "que significa", "que marca",
        "de que color", "donde esta", "quien es", "quienes son", "que se ve", "que aparece", "que sale",
    ];
    frases.iter().any(|f| q.contains(f))
        || empieza(&[
            "describ", "lee", "leer", "texto", "escena", "identific", "reconoc", "cuant", "analiz", "mira",
            "explica", "detall", "ocurr", "sucede", "letrero", "cartel", "etiqueta", "matricula", "placa",
        ])
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_mejor_vista_solo_para_mirar_una_foto() {
        for (pedido, foto, debe) in [
            ("Te mando esta imagen. Descríbela en una o dos frases", true, true),
            ("¿Qué pone en el cartel?", true, true),
            ("léeme lo que dice la etiqueta", true, true),
            ("¿Qué está ocurriendo en esta escena?", true, true),
            ("¿cuántas personas hay?", true, true),
            // Negativos: sin foto, o pidiendo HACER algo con ella.
            ("Descríbeme un gato", false, false),
            ("dibuja eso en el lienzo", true, false),
            ("calca esta foto", true, false),
            ("lee el texto y pásalo a una lista", true, false),
            ("hola", true, false),
            ("gracias", true, false),
        ] {
            assert_eq!(pide_mejor_vista(pedido, foto), debe, "{pedido}");
        }
    }

    fn nombres(pedido: &str, img: bool) -> Vec<String> {
        para_el_pedido(pedido, img).into_iter().map(|h| h.nombre).collect()
    }

    #[test]
    fn los_pedidos_de_editar_llevan_su_herramienta() {
        use crate::edicion::{CORTAR, EXTENDER, GEOMETRIA, IMAGEN, TRANSFORMAR, TRAZAR};
        for (pedido, img, debe) in [
            ("dibuja dos rectas que se crucen y corta la de abajo en el cruce", false, CORTAR),
            ("pega la foto y recorta la mitad izquierda", true, IMAGEN),
            ("resalta el titulo", false, TRAZAR),
            ("subraya el resultado con el lapiz rojo", false, TRAZAR),
            ("alarga la recta hasta que toque el circulo", false, EXTENDER),
            ("traza una perpendicular a la recta por su punto medio", false, GEOMETRIA),
            ("mueve el circulo rojo 50 a la derecha", false, TRANSFORMAR),
        ] {
            let n = nombres(pedido, img);
            assert!(n.iter().any(|x| x == debe), "{pedido}: {n:?}");
            assert!(n.len() <= MAXIMO);
        }
        // Sin nombrar un documento, resaltar es del lienzo; nombrandolo, del
        // documento.
        assert!(!nombres("resalta el titulo", false).iter().any(|x| x == "anotar_documento"));
        assert!(nombres("resalta en el PDF el total", false).iter().any(|x| x == "anotar_documento"));
        // Negativo: con una foto, «copia esta imagen» sigue siendo calcarla.
        assert_eq!(nombres("Copia esta imagen en el lienzo", true), vec![CALCAR.to_string()]);
    }

    #[test]
    fn elige_pocas_y_las_que_vienen_a_cuento() {
        for (pedido, img, debe) in [
            ("Dibuja un circulo rojo y un cuadrado azul", false, "dibujar_en_lienzo"),
            ("Hazme una grafica de barras de ventas: enero 120", false, "grafica"),
            ("Copia esta imagen en el lienzo", true, "calcar_imagen"),
            ("Apunta la tarea: llamar al ingeniero", false, "anadir_tarea"),
            ("Crea una lista de tareas que se llame Obra", false, "lista_nueva"),
            ("Abre la ventana de tareas", false, "abrir_ventana"),
            ("Enseñame la galeria de capturas", false, "abrir_ventana"),
            ("Haz una captura de una zona de la pantalla", false, "capturar_pantalla"),
            ("Saca como pin la ultima captura", false, "pinear_ultima_captura"),
            ("Crea un lienzo nuevo llamado Croquis", false, "lienzo_nuevo"),
            ("Graba una nota de voz que se llame reunion", false, "grabar_voz"),
            (r"Abre el archivo C:\Users\yo\plano.pdf", false, "abrir_fichero"),
            ("Busca en mis lecciones aprendidas las de encofrado", false, "buscar_lecciones"),
            ("Crea una nota nueva que diga: comprar cemento", false, "nota_nueva"),
            ("Sincroniza con el movil", false, "sincronizar_movil"),
            ("Lee el texto de este pdf escaneado", false, "ocr"),
            ("Saca las tareas de esta acta y ponlas en una lista nueva", false, "anadir_tareas"),
            ("¿Qué tengo pendiente esta semana?", false, "consultar_tareas"),
            ("Aplaza la tarea llamar al banco al viernes", false, "editar_tarea"),
        ] {
            let n = nombres(pedido, img);
            assert!(n.iter().any(|x| x == debe), "«{pedido}» → {n:?}");
            assert!(n.len() <= MAXIMO, "«{pedido}» → {n:?}");
        }
    }

    /// 15-oct: las herramientas de la app solo para la nube no llegan nunca
    /// al modelo local («la IA local, no la toques»).
    #[test]
    fn las_de_solo_la_nube_no_llegan_al_local() {
        for pedido in [
            "Graba la pantalla en un gif",
            "Cierra todos los pines",
            "Léeme en voz alta el informe",
            "Comprime el PDF de la tesis que pesa mucho",
            "Transcribe el audio de la reunión",
            "¿Cuánto suma la columna C del Excel?",
            "Apunta en el timeline que llegó el camión",
            "Dime cuánto hice hoy",
            "Abre los ajustes",
        ] {
            let n = nombres(pedido, false);
            assert!(!n.iter().any(|x| crate::acciones::es_solo_nube(x)), "«{pedido}» → {n:?}");
        }
    }

    #[test]
    fn sacar_las_tareas_de_un_acta_no_ofrece_apuntarlas_de_una_en_una() {
        let n = nombres("Saca las tareas de esta acta y ponlas en una lista nueva llamada Acta", false);
        assert!(n.contains(&"anadir_tareas".to_string()) && !n.contains(&"anadir_tarea".to_string()), "{n:?}");
        // Una sola tarea sigue con la suya.
        assert!(nombres("Apunta la tarea comprar pan", false).contains(&"anadir_tarea".to_string()));
    }

    #[test]
    fn una_pregunta_solo_lleva_la_ayuda() {
        assert_eq!(nombres("¿Cómo hago una captura de pantalla?", false), vec!["ayuda_app"]);
        assert_eq!(nombres("¿Donde estan los ajustes de PixPin?", false), vec!["ayuda_app"]);
        // Pedir algo con una pregunta de cortesia no es pedir ayuda.
        assert!(nombres("¿Puedes abrir la galeria?", false).contains(&"abrir_ventana".to_string()));
    }

    #[test]
    fn un_saludo_no_lleva_herramientas_y_lo_raro_lleva_las_de_siempre() {
        assert!(nombres("Hola, ¿que tal?", false).is_empty());
        assert!(nombres("gracias", false).is_empty());
        assert_eq!(nombres("hazme algo bonito", false), vec!["dibujar_en_lienzo", "grafica", "ayuda_app"]);
    }

    #[test]
    fn las_preguntas_sobre_sus_documentos_llevan_buscar() {
        let b = crate::documentos::BUSCAR;
        assert_eq!(nombres("¿Qué dice la tesis sobre las cunetas?", false)[0], b);
        assert_eq!(nombres("busca en mis notas lo del presupuesto", false)[0], b);
        assert_eq!(nombres("¿Dónde mandé el plano de cunetas?", false), vec![b, "ayuda_app"]);
        // Lo de la app sigue siendo solo ayuda.
        assert_eq!(nombres("¿Donde estan los ajustes de PixPin?", false), vec!["ayuda_app"]);
        assert!(!nombres("Dibuja un circulo rojo", false).contains(&b.to_string()));
    }

    #[test]
    fn sin_imagen_no_se_ofrece_calcar() {
        assert!(!nombres("copia el circulo", false).contains(&"calcar_imagen".to_string()));
        // Un saludo con una foto no es pedir que se calque.
        assert!(nombres("hola", true).is_empty());
    }

    #[test]
    fn con_la_imagen_y_un_pedido_sobre_ella_solo_se_ofrece_calcar() {
        for pedido in [
            "Dibuja este perrito de la imagen adjunta dentro del lienzo.",
            "Copia esta imagen en el lienzo",
            "Calca solo los contornos",
            "dibujalo en el lienzo",
            "Pasa este dibujo al lienzo con sus colores",
            "pon la foto en el lienzo",
        ] {
            assert_eq!(nombres(pedido, true), vec!["calcar_imagen"], "«{pedido}»");
        }
    }

    #[test]
    fn las_herramientas_nuevas_salen_cuando_vienen_a_cuento() {
        for (pedido, debe) in [
            ("Dibuja un arbol en isometrico para arquitectura", "dibujar_svg"),
            ("Grafica la funcion y = x^2 entre -3 y 3", "graficar_funcion"),
            ("Escribe la formula cuadratica en el lienzo", "escribir_ecuacion"),
            ("Hazme un cronograma de la obra con sus hitos", "cronograma"),
            ("Guarda esto como arbol isometrico", "figuras_guardadas"),
            ("Pon el arbol isometrico en el lienzo", "figuras_guardadas"),
        ] {
            let n = nombres(pedido, false);
            assert!(n.iter().any(|x| x == debe), "«{pedido}» → {n:?}");
        }
    }

    /// Estudiantes (14-oct): graficas de clase, fisica y la foto de un
    /// problema.
    #[test]
    fn los_pedidos_de_estudiantes_traen_su_herramienta() {
        for (pedido, img, debe) in [
            ("Grafica r = 1 + cos θ", false, "graficar_funcion"),
            ("Grafica la parametrica de una elipse", false, "graficar_funcion"),
            ("Grafica x²-4 y marca sus raices", false, "graficar_funcion"),
            ("Diagrama de cuerpo libre de un bloque en un plano inclinado de 30° con rozamiento", false, "diagrama_fisica"),
            ("Tiro parabolico a 20 m/s y 45°", false, "diagrama_fisica"),
            ("Dibuja un circuito con dos resistencias en paralelo", false, "diagrama_fisica"),
            ("Pasa en limpio este problema", true, "problema_en_limpio"),
            ("¿Como resuelvo este ejercicio?", true, "problema_en_limpio"),
            ("Resuelve el problema: un tren sale a 60 km/h...", false, "problema_en_limpio"),
        ] {
            let n = nombres(pedido, img);
            assert!(n.iter().any(|x| x == debe), "«{pedido}» → {n:?}");
            assert!(!n.contains(&"calcar_imagen".to_string()), "«{pedido}» → {n:?}");
        }
        // Con la plantilla de lleno, dibujar a mano sobra (el 2B la elegia).
        let n = nombres("Dibuja el tiro parabolico de un proyectil lanzado a 20 m/s y 45°", false);
        assert!(!n.contains(&"dibujar_en_lienzo".to_string()), "{n:?}");
        // Pero dibujar algo cualquiera sigue siendo dibujar.
        assert!(nombres("Dibuja una casa con un arbol", false).contains(&"dibujar_en_lienzo".to_string()));
        // Negativo: copiar una foto sigue siendo calcarla.
        assert_eq!(nombres("Copia esta imagen en el lienzo", true), vec!["calcar_imagen"]);
    }

    #[test]
    fn una_foto_a_lapiz_o_con_figuras_se_interpreta_no_se_calca() {
        for pedido in [
            "Dibuja esta persona con figuras",
            "dibuja este perro a lapiz",
            "Haz un boceto con grafito de esta foto",
        ] {
            let n = nombres(pedido, true);
            assert!(!n.contains(&"calcar_imagen".to_string()), "«{pedido}» → {n:?}");
            assert!(n.contains(&"dibujar_svg".to_string()) && n.contains(&"dibujar_en_lienzo".to_string()), "«{pedido}» → {n:?}");
        }
        assert_eq!(nombres("Copia esta imagen en el lienzo", true), vec!["calcar_imagen"]);
    }

    #[test]
    fn dibujar_otra_cosa_con_una_imagen_en_el_chat_no_calca() {
        for pedido in ["Dibuja un circulo rojo", "dibuja una casa con un arbol", "haz una grafica de barras: 1, 2, 3"] {
            let n = nombres(pedido, true);
            assert!(!n.contains(&"calcar_imagen".to_string()), "«{pedido}» → {n:?}");
            assert!(!n.is_empty(), "«{pedido}»");
        }
        // Sin imagen, aunque hable de «esta imagen», no hay nada que calcar.
        assert!(!nombres("copia esta imagen", false).contains(&"calcar_imagen".to_string()));
    }

    /// Los documentos (14-oct): cada pedido trae su herramienta, tambien
    /// las preguntas «¿donde dice...?» y la foto de una tabla.
    #[test]
    fn los_pedidos_de_documentos_traen_su_herramienta() {
        use crate::acciones_documentos::{ABRIR, ANOTAR, BUSCAR_EN, EXPORTAR, TABLA};
        for (pedido, img, debe) in [
            ("Convierte esta tabla escaneada a Excel", true, TABLA),
            ("Pasa la tabla del PDF a una tabla del chat", false, TABLA),
            ("Resalta JR. ANTISUYO en el PDF", false, ANOTAR),
            ("Encierra en rojo la leyenda del plano", false, ANOTAR),
            ("¿Dónde dice JR. ANTISUYO en el plano?", false, BUSCAR_EN),
            ("Llevame a donde dice Av. Tahuantinsuyo", false, BUSCAR_EN),
            ("Abre el plano de señalización de Tahuantinsuyo", false, ABRIR),
            ("Exporta el plano a PDF", false, EXPORTAR),
        ] {
            let n = nombres(pedido, img);
            assert!(n.iter().any(|x| x == debe), "«{pedido}» → {n:?}");
            assert!(n.len() <= MAXIMO, "«{pedido}» → {n:?}");
            assert!(!n.contains(&"calcar_imagen".to_string()), "«{pedido}» → {n:?}");
        }
        // Caso negativo: una pregunta sobre la app sigue siendo de la ayuda.
        assert_eq!(nombres("¿Donde estan los ajustes de PixPin?", false), vec!["ayuda_app"]);
    }
}
