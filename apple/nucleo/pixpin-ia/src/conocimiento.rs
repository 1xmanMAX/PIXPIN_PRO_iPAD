//! **Lo que el asistente sabe de PixPin**: un resumen para las instrucciones
//! y un indice corto para la herramienta `ayuda_app`.
//!
//! Pedido del usuario (13-oct-2026): «que sepa todo de la app». Un modelo de
//! 2B no la conoce y, si se le pregunta, inventa menus. Con 8192 tokens de
//! contexto no cabe un manual, asi que: un **resumen** de ~600 letras va
//! siempre en las instrucciones, y el detalle (donde esta cada cosa, que
//! atajo) se busca en este **indice local** solo cuando se pregunta.
//!
//! Sacado del codigo y de los textos de la app (`i18n/es-ES/main.ftl`,
//! `comandos.rs`, `gestos.rs`, el menu del pin y de la bandeja,
//! `docs/protocolo-pedidos.md`), comprobado atajo a atajo el 13-oct-2026.
//! Si cambia un menu o un atajo, hay que cambiarlo aqui.

use crate::acciones::normalizar;

/// Para las instrucciones de sistema: que es PixPin, en corto.
pub const RESUMEN: &str = "PixPin Max es una app de Windows que vive en la bandeja del sistema. \
Hace capturas (Ctrl+Alt+X o Alt + arrastrar) y pines flotantes con zoom, anotacion y OCR. \
Tiene un chat por proyectos (con «Mensajes guardados») con fotos, archivos, notas de voz, notas, \
lienzos y tablas, y visores de PDF, Word, PowerPoint, Excel, planos CAD y modelos BIM. \
Ademas: galeria de capturas que caducan a los 7 dias, Tareas con Inbox, Lecciones aprendidas, \
buscador, plugin de Flow Launcher («p») y sincronizacion con el movil por Wi-Fi.";

/// Una entrada: tema, palabras que la delatan y la respuesta.
pub struct Entrada {
    pub tema: &'static str,
    pub claves: &'static [&'static str],
    pub respuesta: &'static str,
}

const fn e(
    tema: &'static str,
    claves: &'static [&'static str],
    respuesta: &'static str,
) -> Entrada {
    Entrada {
        tema,
        claves,
        respuesta,
    }
}

/// **El indice.**
pub const INDICE: &[Entrada] = &[
    e(
        "Capturar una zona",
        &[
            "captur",
            "screenshot",
            "pantallazo",
            "recort",
            "atajo",
            "zona",
        ],
        "El atajo de fabrica es Ctrl+Alt+X. Eliges la zona con el raton y la barra ofrece Copiar, Pinear, Al chat, Texto, Guardar y Descartar. Tambien esta en la bandeja, en «Capturar».",
    ),
    e(
        "Gestos con Alt",
        &["alt", "gesto", "arrastr", "raton", "sin atajo"],
        "Sin abrir nada: Alt + arrastrar con el boton izquierdo copia la zona, con el derecho la pinea, con el central hace un pin en vivo, y Alt + doble clic central abre el anotador de pantalla.",
    ),
    e(
        "Modos de captura",
        &[
            "ventana",
            "pantalla completa",
            "scroll",
            "larga",
            "libre",
            "repetir",
            "retardo",
            "cuenta atras",
            "gif",
        ],
        "En la pantalla de captura eliges Zona, Ventana, Pantalla, Con scroll, Grabar GIF, Pin en vivo, Texto (OCR), Libre o «Repetir la ultima zona». En la bandeja estan «Capturar tras una cuenta atras» y «Grabar una zona como GIF».",
    ),
    e(
        "Mandar una captura al chat",
        &["al chat", "mandar captura", "enviar captura", "comentario"],
        "Tras elegir la zona pulsa «Al chat», busca el proyecto, anade un comentario si quieres y pulsa «Enviar».",
    ),
    e(
        "Copiar el texto de una zona (OCR)",
        &[
            "ocr",
            "texto de",
            "leer texto",
            "extraer",
            "copiar texto",
            "reconoc",
        ],
        "En la bandeja, «Copiar el texto de una zona», o el modo «Texto (OCR)» al capturar. En un pin, su menu «Copiar el texto de la imagen» (tecla T).",
    ),
    e(
        "Anotar la pantalla",
        &[
            "anotar",
            "dibujar en pantalla",
            "marcar",
            "rotulador",
            "clic a traves",
        ],
        "En la bandeja, «Anotar la pantalla» o «Anotar una captura de la pantalla», o Alt + doble clic central. «Clic a traves» (Espacio) deja pasar el raton.",
    ),
    e(
        "Pines",
        &[
            "pin",
            "pinear",
            "fijar",
            "flotante",
            "siempre encima",
            "sacar a la pantalla",
        ],
        "En la barra de captura, «Pinear», o Alt + arrastrar con el boton derecho. En el chat, «Pinear» en el menu del mensaje. En la bandeja: «Pin en vivo» y «Pinear lo seleccionado en el Explorador».",
    ),
    e(
        "Zoom y tamano de un pin",
        &[
            "zoom",
            "acercar",
            "alejar",
            "rueda",
            "tamano original",
            "opacidad",
            "invertir",
        ],
        "La rueda del raton acerca o aleja el pin. En su menu (clic derecho): «Tamano original» (Ctrl+0), «Opacidad» e «Invertir colores» (N).",
    ),
    e(
        "Cerrar pines",
        &[
            "cerrar pin",
            "quitar pin",
            "esc",
            "ocultar pines",
            "cerrar todos",
            "recuperar pin",
        ],
        "Esc cierra el pin. Ctrl+2 oculta o muestra todos. En la bandeja: «Cerrar todos los pines» y «Devolver el ultimo pin cerrado».",
    ),
    e(
        "Anotar un pin",
        &[
            "anotar pin",
            "dibujar encima",
            "doble clic",
            "abrir en lienzo",
        ],
        "Doble clic sobre el pin, o tecla A («Anotar»). En su menu, Mas → «Abrir en lienzo» lo lleva al lienzo.",
    ),
    e(
        "Galeria de capturas",
        &[
            "galeria",
            "capturas",
            "historial",
            "recientes",
            "ultimas capturas",
        ],
        "En la bandeja, «Galeria de capturas…», o «Ver la galeria» en el panel de la bandeja. Busca tambien el texto de dentro de las capturas.",
    ),
    e(
        "Caducidad de las capturas",
        &["caduc", "dura", "7 dias", "conservar", "prorrog", "no borr"],
        "Las capturas duran 7 dias y luego van a la papelera. Clic derecho sobre una → «Conservar» (va a Mensajes guardados y no caduca) o «Dar 7 dias mas». El plazo esta en Ajustes → Captura → «Dias hasta borrar».",
    ),
    e(
        "Chat de proyectos",
        &["chat", "proyectos", "conversacion", "ventana principal"],
        "En la bandeja, «Abrir el chat de proyectos». Cada proyecto es un chat de la lista de la izquierda.",
    ),
    e(
        "Crear un proyecto",
        &[
            "nuevo proyecto",
            "crear proyecto",
            "carpeta",
            "anadir proyecto",
        ],
        "Pulsa «+» en la lista de proyectos. Con clic derecho sobre «+»: «Proyecto nuevo en una carpeta…», «Proyectos borrados…» y «Entrar en un chat compartido».",
    ),
    e(
        "Mensajes guardados",
        &[
            "mensajes guardados",
            "guardados",
            "general",
            "buzon",
            "inbox",
        ],
        "Es el chat general, el primero de la lista: ahi van las capturas conservadas, lo que llega del movil sin proyecto y la lista «Inbox» de tareas.",
    ),
    e(
        "Adjuntar y soltar ficheros",
        &[
            "adjuntar", "subir", "archivo", "arrastr", "soltar", "foto", "clip",
        ],
        "Pulsa «Adjuntar» en el chat (Foto o video, Archivo, Lienzo, Tabla, Captura, Traer del movil…) o arrastra los ficheros a la ventana. El menu ⋮ de la cabecera → «Anadir arrastrando…» saca una caja flotante.",
    ),
    e(
        "Notas de voz y dictado",
        &[
            "voz",
            "grabar",
            "audio",
            "microfono",
            "dictar",
            "transcrib",
            "pasar a texto",
        ],
        "Con la caja de escribir vacia, el boton de enviar es el microfono: clic para grabar. Clic derecho en el microfono es dictar. En el menu del mensaje, «Pasar a texto» lo transcribe (Whisper, se baja la primera vez).",
    ),
    e(
        "Menu de un mensaje",
        &[
            "fijar",
            "recordar",
            "recordatorio",
            "reenviar",
            "etiqueta",
            "compartir",
            "renombrar",
            "borrar mensaje",
        ],
        "Clic derecho sobre la burbuja: Copiar, Fijar arriba, Recordarmelo, Reenviar, Poner una etiqueta, Cambiar el nombre, Pinear, Abrir con…, Compartir…, Enviar por Wi-Fi, Borrar.",
    ),
    e(
        "Buscar en un chat",
        &["buscar mensaje", "lupa", "buscar aqui"],
        "La lupa de la cabecera del chat («Buscar aqui…») busca dentro de esa conversacion.",
    ),
    e(
        "Notas",
        &["nota", "markdown", "editor", "apuntes", "documento"],
        "Crea una con Adjuntar → «Nota», o «Abrir en el editor de notas» en una existente. Escribe / para ver opciones. Ctrl+S guarda, Ctrl+1/2/3 son titulos y «Exportar a Word…» la saca a .docx.",
    ),
    e(
        "Lienzo",
        &[
            "lienzo",
            "dibujo",
            "dibujar",
            "pizarra",
            "canvas",
            "excalidraw",
        ],
        "En el chat, Adjuntar → «Lienzo nuevo». Si se abre en «Solo mirar», Alt+R lo pasa a editar y F5 presenta.",
    ),
    e(
        "Herramientas del lienzo",
        &[
            "herramientas",
            "lapiz",
            "rectangulo",
            "flecha",
            "goma",
            "lazo",
            "cota",
            "bote",
            "relleno",
        ],
        "En la barra del lienzo: Elegir y mover (M), Lapiz, Formas, Flechas y lineas, Texto, Goma, Lazo, Cota, Bote de relleno, Lupa (Q), Zona (Z), Foco (F), Marco e Imagen. Ctrl+Z deshace y Ctrl+Y rehace.",
    ),
    e(
        "Exportar el lienzo",
        &[
            "export",
            "guardar como",
            "png",
            "svg",
            "pdf",
            "html",
            "imprimir",
        ],
        "«Exportar…» es Ctrl+Mayus+E (PNG, SVG, PDF, pagina web). «Copiar como PNG» es Ctrl+Mayus+C e «Imprimir…» Ctrl+P.",
    ),
    e(
        "Graficas en el lienzo",
        &["grafica", "funcion", "coordenadas", "ejes", "plot"],
        "En el lienzo, «Figuras» → «Grafica de una funcion…» o «Tabla de coordenadas…». El asistente tambien hace graficas de barras, lineas o pastel con datos.",
    ),
    e(
        "Excel y hojas de calculo",
        &["excel", "xlsx", "hoja de calculo", "tabla", "csv", "celda"],
        "Un Excel se abre como pin para ver; el boton «Editar» de su barra lo edita (doble clic, F2 o escribir en la celda; Esc vuelve a ver). El original no cambia. Ctrl+E exporta.",
    ),
    e(
        "Word y PowerPoint",
        &[
            "word",
            "docx",
            "powerpoint",
            "pptx",
            "presentacion",
            "diapositiva",
            "office",
        ],
        "Se abren en PixPin sin Office. A anota, Ctrl+F busca, Ctrl+P imprime, F5 presenta. Se anota encima; el documento original no cambia.",
    ),
    e(
        "PDF",
        &[
            "pdf",
            "lector",
            "leer pdf",
            "anotar pdf",
            "marcador",
            "escuchar",
        ],
        "El lector de PDF: A anota, L lee en voz alta, M pone un marcador, Ctrl+rueda acerca. «Guardar el PDF con lo anotado» exporta las anotaciones.",
    ),
    e(
        "Modo oscuro de los visores",
        &["modo oscuro", "oscuro", "noche", "luna", "tema"],
        "Con el boton luna/sol de la barra de arriba del pin-lector. Vale para todos los visores y se recuerda.",
    ),
    e(
        "Abrir documentos y vista rapida",
        &[
            "abrir documento",
            "abrir archivo",
            "vista rapida",
            "espacio",
            "explorador",
            "abrir con",
        ],
        "En la bandeja, «Abrir documento…». Con «Vista rapida con la barra espaciadora» activada en Ajustes, Espacio sobre un fichero del Explorador lo abre en PixPin.",
    ),
    e(
        "Planos CAD",
        &["cad", "autocad", "dwg", "dxf", "plano"],
        "Los DWG y DXF se abren como pin en el visor de planos. «Exportar…» (Ctrl+E) los saca a HTML por capas, PDF, PNG, JPG o DXF.",
    ),
    e(
        "Modelos BIM y 3D",
        &["bim", "ifc", "revit", "3d", "modelo"],
        "Los modelos IFC y Revit se abren en un visor 3D como pin; su menu «Exportar…» saca la vista a PDF, HTML o imagen.",
    ),
    e(
        "Tareas",
        &["tarea", "pendiente", "todo", "lista", "inbox", "checklist"],
        "Las tareas estan en la ventana de Tareas: se abre en la bandeja con «Tareas…». Escribe arriba y pulsa Intro: la tarea va al Inbox. Ctrl+T la ve en tarjetas, Ctrl+K en Kanban y Ctrl+H en el horario de clases de la semana. Las listas nuevas se crean en un chat con Adjuntar → «Lista de tareas».",
    ),
    e(
        "Marcar y mover tareas",
        &["marcar", "hecha", "completar", "desmarcar", "mover tarea"],
        "Sobre la tarea: «Marcar como hecha» / «Marcar como pendiente», «Editar», «Mover a…» y «Quitar».",
    ),
    e(
        "Lecciones aprendidas",
        &[
            "leccion",
            "lecciones",
            "aprendi",
            "repasar",
            "repaso",
            "timeline",
        ],
        "En la bandeja, «Timeline y lecciones…» o «Nueva leccion…». Escribe «paso que…, porque…, la proxima vez…» y pulsa Intro. «Repasar» hace el repaso de hoy.",
    ),
    e(
        "Sincronizar con el movil",
        &[
            "sincroniz",
            "al movil",
            "app movil",
            "movil",
            "celular",
            "android",
            "sync",
            "wifi",
            "grupo",
            "codigo",
            "telefono",
        ],
        "En la bandeja, «Sincronizar». Crea un grupo en un aparato y en los demas «Unirme con un codigo»; luego «Sincronizar con todos». Va por la misma Wi-Fi, sin internet.",
    ),
    e(
        "Enviar al movil",
        &[
            "enviar al movil",
            "pasar al movil",
            "enviar wifi",
            "recibir",
            "traer del movil",
        ],
        "En Sincronizar → «Enviar» (Enviar por Wi-Fi), o «Enviar por Wi-Fi» en el menu del mensaje. Para traer cosas: Sincronizar → «Recibir», o Adjuntar → «Traer del movil».",
    ),
    e(
        "Compartir un chat con personas",
        &["compartir chat", "otra persona", "invitar", "colabor"],
        "Clic derecho sobre el proyecto → «Compartir con personas» → «Anadir» ensena un codigo. La otra persona entra con «Entrar en un chat compartido» (clic derecho en «+»).",
    ),
    e(
        "Copias y proyectos borrados",
        &[
            "copia de seguridad",
            "recuperar",
            "papelera",
            "deshacer borrado",
            "version anterior",
        ],
        "En el menu del proyecto, «Copias de seguridad». Los proyectos borrados se recuperan con «Proyectos borrados…» (clic derecho en «+»).",
    ),
    e(
        "Ajustes",
        &[
            "ajustes",
            "configuracion",
            "opciones",
            "preferencias",
            "settings",
        ],
        "Desde el icono de PixPin en la bandeja del sistema → «Ajustes». Tiene buscador y secciones General, Captura, Lienzo y herramientas, Voz, Atajos, Sincronizar, Apps predeterminadas, Apariencia y Avanzado.",
    ),
    e(
        "Arrancar con Windows e idioma",
        &["arrancar", "inicio", "autoarranque", "idioma", "lenguaje"],
        "En Ajustes → General: «Arrancar con Windows» e «Idioma» (vale desde el proximo arranque).",
    ),
    e(
        "Cambiar los atajos",
        &["atajo", "teclas", "combinacion", "silenciar"],
        "En Ajustes → Atajos: pulsa una caja y luego la combinacion; Supr la deja sin atajo. «Silenciar los atajos» en la bandeja los apaga un rato. De fabrica solo hay Ctrl+Alt+X (capturar) y Ctrl+2 (pines).",
    ),
    e(
        "Bandeja del sistema",
        &[
            "bandeja",
            "icono",
            "menu",
            "panel",
            "barra de tareas",
            "salir",
        ],
        "Clic izquierdo en el icono de PixPin: el panel (Capturar, modos, Pines, Ultimas capturas, Tareas, Lecciones, Galeria, Abrir documento…). Clic derecho: el menu (Capturar, Pin en vivo, Anotar, Abrir el chat de proyectos, Tareas…, Sincronizar, Ajustes, Buscar en PixPin…, Salir).",
    ),
    e(
        "Buscar en PixPin",
        &["buscar", "busqueda", "encontrar", "buscador"],
        "En la bandeja, «Buscar en PixPin…». Busca archivos, tareas, lecciones y capturas (Tab cambia de pestana).",
    ),
    e(
        "Flow Launcher",
        &["flow", "lanzador", "launcher"],
        "En Flow Launcher escribe «p» y lo que buscas. «p t texto» apunta una tarea, «p n» nota, «p l» lienzo, «p g» galeria, «p c» capturar, «p u» ultima captura, «p s» sincronizar; «p proyecto > texto» escribe en un chat.",
    ),
    e(
        "El asistente: bajarlo",
        &[
            "asistente",
            "ia",
            "inteligencia",
            "modelo",
            "qwen",
            "bajar",
            "descargar",
        ],
        "En el chat fijo «Asistente», «Bajar» → «Si, bajar»; se baja una vez. El modelo se elige en Ajustes («Modelo del asistente»): Qwen3.5 2B o 0.8B (ligero).",
    ),
    e(
        "El asistente: que hace",
        &[
            "que haces",
            "que sabes",
            "puedes",
            "asistente",
            "@asistente",
        ],
        "Funciona sin internet: contesta, dibuja en el lienzo, calca imagenes, hace graficas, lee PDFs escaneados, y usa la app (notas, tareas, ventanas, capturas). En otro chat se le llama con @asistente.",
    ),
    e(
        "Reproductor de audio",
        &["audio", "musica", "reproduc", "escuchar", "cancion"],
        "Las notas de voz suenan en su burbuja. En Mensajes guardados, menu ⋮ → «Biblioteca de audio». Desde Flow, Intro sobre un audio abre el reproductor flotante (Espacio pausa, Esc cierra).",
    ),
];

/// Las palabras de una pregunta que no dicen nada.
const VACIAS: &[&str] = &[
    "el", "la", "los", "las", "un", "una", "de", "del", "en", "y", "o", "a", "al", "que", "como",
    "donde", "esta", "estan", "hago", "hace", "hacer", "puedo", "puede", "quiero", "tengo", "cual",
    "cuando", "cuanto", "se", "me", "mi", "mis", "por", "para", "con", "es", "lo", "pixpin", "una",
    "unos", "unas",
];

/// Si una palabra suelta de la pregunta toca una entrada.
fn toca(e: &Entrada, p: &str) -> bool {
    e.claves
        .iter()
        .any(|c| !c.contains(' ') && (p.starts_with(c) || (p.len() >= 4 && c.starts_with(p))))
        || (p.len() >= 4 && normalizar(e.tema).contains(p))
}

/// Lo que puntua una entrada. Cada palabra vale mas cuanto menos entradas
/// toca (una idea de IDF): «movil» dice mucho, «capturas» casi nada, que
/// sale en media ayuda. Medido 13-oct: «¿como mover capturas, notas o
/// tareas al movil?» contestaba con la galeria y las notas.
fn puntos(e: &Entrada, pregunta: &str, palabras: &[&str], cuantas: &[usize]) -> f64 {
    let mut s = 0.0;
    for c in e.claves.iter().filter(|c| c.contains(' ')) {
        if pregunta.contains(c) {
            s += 3.0;
        }
    }
    for (p, n) in palabras.iter().zip(cuantas) {
        if *n > 0 && toca(e, p) {
            s += 2.0 / (*n as f64).sqrt();
        }
    }
    s
}

/// **Busca en el indice** lo que responde a la pregunta: las 2 entradas mas
/// a tono, con su tema. Sin nada que valga, lo dice.
pub fn responder(pregunta: &str) -> String {
    let q = normalizar(pregunta);
    let palabras: Vec<&str> = q
        .split(|c: char| !c.is_alphanumeric() && c != '+')
        .filter(|p| !p.is_empty() && !VACIAS.contains(p))
        .collect();
    let cuantas: Vec<usize> = palabras
        .iter()
        .map(|p| INDICE.iter().filter(|e| toca(e, p)).count())
        .collect();
    let mut puntuadas: Vec<(f64, &Entrada)> = INDICE
        .iter()
        .map(|e| (puntos(e, &q, &palabras, &cuantas), e))
        .filter(|(s, _)| *s > 0.0)
        .collect();
    puntuadas.sort_by(|a, b| b.0.total_cmp(&a.0));
    if puntuadas.is_empty() {
        return "No encuentro eso en la ayuda de PixPin. Di que no lo sabes; no inventes menus."
            .into();
    }
    let mejor = puntuadas[0].0;
    puntuadas
        .iter()
        .take(2)
        .filter(|(s, _)| *s >= mejor * 0.6)
        .map(|(_, e)| format!("{}: {}", e.tema, e.respuesta))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn contesta_lo_tipico() {
        for (pregunta, debe) in [
            ("¿Dónde están los ajustes?", "bandeja"),
            ("¿Cómo hago una captura de pantalla?", "Ctrl+Alt+X"),
            ("como sincronizo con el movil", "Sincronizar"),
            ("donde veo mis tareas pendientes", "Inbox"),
            ("como exporto el lienzo a pdf", "Ctrl+Mayus+E"),
            ("cuanto duran las capturas", "7 dias"),
            (
                "Cómo mover las capturas, notas o tareas a una app móvil de PixPin?",
                "Sincronizar",
            ),
            (
                "dónde están las tareas pendientes en PixPin",
                "ventana de Tareas",
            ),
            ("como pongo el modo oscuro", "luna"),
        ] {
            let r = responder(pregunta);
            assert!(r.contains(debe), "«{pregunta}» → {r}");
        }
    }

    #[test]
    fn lo_que_no_esta_lo_dice_sin_inventar() {
        let r = responder("¿como se hace una tortilla de patatas?");
        assert!(r.contains("No encuentro"), "{r}");
        assert!(responder("").contains("No encuentro"));
    }

    #[test]
    fn el_resumen_cabe() {
        assert!(RESUMEN.chars().count() < 700);
        let total: usize = INDICE
            .iter()
            .map(|e| e.respuesta.len() + e.tema.len())
            .sum();
        // El indice no va al modelo entero, pero que no se desmadre.
        assert!(total < 12_000, "{total}");
    }
}
