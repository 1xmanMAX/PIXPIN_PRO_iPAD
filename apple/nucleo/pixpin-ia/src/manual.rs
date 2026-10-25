//! **Como esta hecha PixPin, para el asistente** (23-oct-2026).
//!
//! El usuario (23-oct): «no le digas lo que puede hacer: ensenale a usar la
//! app y que herramientas tiene, para que cuando le pida algo no diga que no
//! sabe, lo haga, y lo haga bien supervisando su trabajo». Las
//! instrucciones (`agente::instrucciones`) dicen COMO trabajar; esto le da el
//! mapa de DONDE trabaja: los sitios de la app (chats, lienzo, documentos,
//! tareas...), como se nombra cada cosa en ellos (#id, «Lienzo destino», el
//! adjunto por su nombre) y que herramientas leen, crean o cambian cada
//! sitio. Con el mapa, un pedido que ninguna herramienta hace sola se ve
//! como una combinacion (leer aqui, pensar, escribir alli), que es lo que
//! fallaba: «no tengo herramienta para traducir la figura».
//!
//! **Por que asi** (ver `docs/investigacion/2026-10-23-asistente-que-sabe-usar-la-app.md`):
//! Anthropic recomienda describir el entorno y las relaciones entre los
//! recursos «como a alguien nuevo en el equipo», a la altura justa (ni una
//! lista de si-entonces ni algo vago), con pocos ejemplos canonicos en vez
//! de reglas para cada caso.
//!
//! **Siempre al dia**: el mapa de herramientas se genera de las tablas de
//! verdad (`herramientas::catalogo`, `acciones::todas`, `edicion::filas`).
//! Lo que se anada sin sitio sale en «Otras» y una prueba lo para.
//!
//! **Fijo** para la misma version: va en el sistema, que es prefijo de la
//! cache de la nube. Solo la nube lo lleva (el local esta congelado).

use crate::acciones::Categoria;

/// Un sitio de la app: como se llama, que es y como se nombra lo de dentro.
struct Sitio {
    nombre: &'static str,
    que_es: &'static str,
}

/// Los sitios, en el orden en que se ensenan.
const SITIOS: &[Sitio] = &[
    Sitio {
        nombre: "Chats y notas",
        que_es: "Cada proyecto es un chat (también «Mensajes guardados» y este chat «Asistente»). Sus mensajes son notas, fotos, archivos, notas de voz, lienzos y tablas; lo de un día va al timeline.",
    },
    Sitio {
        nombre: "Lienzo: elegir hoja",
        que_es: "Un chat puede tener varios lienzos (hojas de dibujo). Si no dicen cuál, el del asistente; «@lienzo:» nombra otro.",
    },
    Sitio {
        nombre: "Lienzo: crear",
        que_es: "Lo que se dibuja: figuras, textos, flechas, tablas, gráficas, ecuaciones, ilustraciones, plantillas de serie.",
    },
    Sitio {
        nombre: "Lienzo: encontrar y cambiar",
        que_es: "Cada cosa del lienzo tiene su #id y su caja [x,y,ancho,alto]; el «Lienzo destino» trae lo que hay. Se encuentra por lo que es o lo que dice y se cambia por su #id, muchas de una vez.",
    },
    Sitio {
        nombre: "Lienzo: desde una imagen",
        que_es: "Una foto o un PDF adjunto (o la imagen pegada en el lienzo) pasado al lienzo: calcado, interpretado entero o como tabla.",
    },
    Sitio {
        nombre: "Carpetas de páginas",
        que_es: "Cuadernos con nombre que juntan lienzos del chat en orden («Día 1», «Día 2»…); la página nueva repite la plantilla de la última, vacía.",
    },
    Sitio {
        nombre: "Mirar",
        que_es: "Ver cómo quedó de verdad (una imagen del lienzo o de la hoja anotada) antes de darlo por bueno.",
    },
    Sitio {
        nombre: "Documentos",
        que_es: "Los adjuntos del chat (PDF, Word, Excel, PowerPoint, planos DWG y modelos BIM) se nombran con las palabras del usuario: leer, buscar, anotar encima, abrir, exportar.",
    },
    Sitio {
        nombre: "Tareas",
        que_es: "Listas de tareas en los chats y un Inbox común; con fecha, son recordatorios. También automatizaciones y el resumen de lo hecho.",
    },
    Sitio {
        nombre: "Capturas y pines",
        que_es: "Capturar la pantalla, grabarla, pines flotantes y la galería de capturas (caducan a los 7 días si no se conservan).",
    },
    Sitio {
        nombre: "Voz y audio",
        que_es: "Grabar, transcribir, leer en voz alta y reproducir.",
    },
    Sitio {
        nombre: "Lecciones",
        que_es: "Las lecciones aprendidas del usuario: buscarlas y apuntar una nueva.",
    },
    Sitio {
        nombre: "Ventanas",
        que_es: "Abrir ventanas y ficheros de PixPin o del equipo.",
    },
    Sitio {
        nombre: "Móvil y conexiones",
        que_es: "Mandar al móvil y sincronizar; Google Drive y el calendario si el usuario los conectó.",
    },
    Sitio {
        nombre: "Saber y recordar",
        que_es: "Cómo se usa PixPin, los documentos del usuario, lo dicho antes en esta conversación, la web y tu memoria entre conversaciones.",
    },
    Sitio {
        nombre: "Hablar con el usuario",
        que_es: "Una pregunta con opciones cuando la duda cambia el resultado, y la lista de pasos del pedido que ve marcarse.",
    },
];

/// Las herramientas que no salen de una tabla con su sitio, por su nombre.
const POR_NOMBRE: &[(&str, &str)] = &[
    ("dibujar_en_lienzo", "Lienzo: crear"),
    ("dibujar_svg", "Lienzo: crear"),
    ("grafica", "Lienzo: crear"),
    ("graficar_funcion", "Lienzo: crear"),
    ("escribir_ecuacion", "Lienzo: crear"),
    ("cronograma", "Lienzo: crear"),
    ("diagrama_fisica", "Lienzo: crear"),
    ("problema_en_limpio", "Lienzo: crear"),
    ("figuras_guardadas", "Lienzo: crear"),
    ("calcar_imagen", "Lienzo: desde una imagen"),
    ("hoja_a_lienzo", "Lienzo: desde una imagen"),
    ("tabla_de_imagen", "Lienzo: desde una imagen"),
    ("seleccionar_en_lienzo", "Lienzo: encontrar y cambiar"),
    ("lienzos_del_chat", "Lienzo: elegir hoja"),
    ("usar_lienzo", "Lienzo: elegir hoja"),
    ("crear_lienzo", "Lienzo: elegir hoja"),
    ("anadir_a_carpeta", "Carpetas de páginas"),
    ("pagina_nueva_de_carpeta", "Carpetas de páginas"),
    ("lienzo_nuevo", "Lienzo: elegir hoja"),
    ("abrir_hoja", "Lienzo: elegir hoja"),
    ("ver_lienzo", "Mirar"),
    ("ocr", "Documentos"),
    ("leer_documento", "Documentos"),
    ("leer_excel", "Documentos"),
    ("anotar_inteligente", "Documentos"),
    ("comprimir_pdf", "Documentos"),
    ("ir_a_pagina", "Documentos"),
    ("abrir_fichero", "Ventanas"),
    ("ver_calendario", "Tareas"),
    ("ayuda_app", "Saber y recordar"),
    ("buscar_en_mis_cosas", "Saber y recordar"),
    ("buscar_historial", "Saber y recordar"),
    ("memoria", "Saber y recordar"),
    ("buscar_lecciones", "Lecciones"),
    ("web_search", "Saber y recordar"),
    ("web_fetch", "Saber y recordar"),
    ("preguntar_usuario", "Hablar con el usuario"),
    // La lista de pasos que el usuario ve marcarse (`pasos.rs`).
    (crate::pasos::NOMBRE, "Hablar con el usuario"),
];

/// Las que no estan en `herramientas::catalogo` pero el turno de la nube
/// puede anadir (la pregunta, los ojos, la memoria, el historial, los
/// documentos del usuario, las conexiones) o ejecuta la propia API (la web).
pub const FUERA_DEL_CATALOGO: &[&str] = &[
    "preguntar_usuario",
    crate::pasos::NOMBRE,
    "ver_lienzo",
    "memoria",
    "buscar_historial",
    "buscar_en_mis_cosas",
    "web_search",
    "web_fetch",
];

/// El sitio de una fila de acciones de la app por su categoria.
fn sitio_de_categoria(c: Categoria) -> &'static str {
    match c {
        Categoria::Abrir => "Ventanas",
        Categoria::Notas => "Chats y notas",
        Categoria::Tareas => "Tareas",
        Categoria::Capturas => "Capturas y pines",
        Categoria::Audio => "Voz y audio",
        Categoria::Lecciones => "Lecciones",
        Categoria::Movil => "Móvil y conexiones",
    }
}

/// **El sitio de una herramienta**, si se sabe.
pub fn sitio_de(nombre: &str) -> Option<&'static str> {
    if let Some((_, s)) = POR_NOMBRE.iter().find(|(n, _)| *n == nombre) {
        return Some(s);
    }
    if crate::edicion::fila(nombre).is_some() {
        return Some("Lienzo: encontrar y cambiar");
    }
    if crate::conexiones::acciones::NOMBRES.contains(&nombre) {
        return Some("Móvil y conexiones");
    }
    if crate::acciones_documentos::ACCIONES_DOCUMENTOS
        .iter()
        .any(|a| a.herramienta == nombre)
    {
        return Some("Documentos");
    }
    crate::acciones::por_nombre(nombre).map(|a| sitio_de_categoria(a.categoria))
}

/// Todos los nombres que el asistente de la nube puede ver, en orden.
fn todos_los_nombres() -> Vec<String> {
    let mut v: Vec<String> = crate::herramientas::catalogo()
        .into_iter()
        .map(|h| h.nombre)
        .collect();
    for n in FUERA_DEL_CATALOGO {
        if !v.iter().any(|x| x == n) {
            v.push((*n).to_string());
        }
    }
    v
}

/// **El mapa**: cada sitio, que es y sus herramientas. Lo que no tenga
/// sitio sale al final en «Otras» (una prueba lo impide).
pub fn mapa() -> String {
    let nombres = todos_los_nombres();
    let mut s = String::new();
    for sitio in SITIOS {
        let suyas: Vec<&str> = nombres
            .iter()
            .filter(|n| sitio_de(n) == Some(sitio.nombre))
            .map(String::as_str)
            .collect();
        if suyas.is_empty() {
            continue;
        }
        s.push_str(&format!(
            "- {}: {} → {}\n",
            sitio.nombre,
            sitio.que_es,
            suyas.join(", ")
        ));
    }
    let otras: Vec<&str> = nombres
        .iter()
        .filter(|n| sitio_de(n).is_none())
        .map(String::as_str)
        .collect();
    if !otras.is_empty() {
        s.push_str(&format!("- Otras: {}\n", otras.join(", ")));
    }
    s
}

/// **Como combinar y como comprobar**: los patrones de siempre, en pocas
/// lineas (no una regla por caso).
pub const COMBINAR: &str = "Cómo se combinan (casi todo pedido es uno de estos):\n\
- Leer → pensar tú → escribir: el texto de un sitio (un documento, la nota, los textos del lienzo) lo transformas tú (traducir, resumir, corregir, sacar tareas) y lo escribes con la herramienta de otro sitio.\n\
- Encontrar → cambiar: busca los #id (el «Lienzo destino» o seleccionar_en_lienzo) y cambia todos los que toca en una llamada.\n\
- Crear → mirar → corregir: lo que se ve (un diagrama, una lámina, lo anotado), míralo y arregla lo que se pise, se salga o falte.\n\
- Para lo que no ves: ayuda_app dice cómo se hace en la app; buscar_en_mis_cosas, lo que el usuario guardó; la web, lo de fuera.\n\n\
Antes de decir que algo no se puede: busca en este mapa el sitio que toca y qué lo lee y lo escribe; si hay una combinación, hazla. \
Si de verdad falta una pieza, haz el resto y di en una línea qué falta.\n\n\
Cómo compruebas tu trabajo: cada herramienta te devuelve lo que hizo (los #id, los avisos, un ERROR con su motivo); \
léelo antes de seguir, corrige lo que falló y no digas que hiciste nada que ningún resultado confirme. \
Lo que se ve, míralo antes de terminar. Al acabar, repasa el pedido punto por punto.\n";

/// **El manual entero** para el sistema de la nube.
pub fn manual() -> String {
    format!(
        "Cómo está hecha la app (sitios → herramientas):\n{}\n{}",
        mapa(),
        COMBINAR
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Cada herramienta que puede ver la nube tiene su sitio: una nueva sin
    /// sitio la para aqui (y no sale en «Otras»).
    #[test]
    fn cada_herramienta_tiene_su_sitio() {
        let sin: Vec<String> = todos_los_nombres()
            .into_iter()
            .filter(|n| sitio_de(n).is_none())
            .collect();
        assert!(sin.is_empty(), "sin sitio en el manual: {sin:?}");
        assert!(!mapa().contains("- Otras:"));
    }

    /// Lo de la tabla por nombre existe (en el catalogo o fuera de el): un
    /// nombre cambiado no deja una entrada muerta.
    #[test]
    fn la_tabla_no_nombra_herramientas_que_no_existen() {
        let todos = todos_los_nombres();
        let muertas: Vec<&str> = POR_NOMBRE
            .iter()
            .map(|(n, _)| *n)
            .filter(|n| !todos.iter().any(|t| t == n) && crate::acciones::por_nombre(n).is_none())
            .collect();
        assert!(muertas.is_empty(), "no existen: {muertas:?}");
        for (_, s) in POR_NOMBRE {
            assert!(
                SITIOS.iter().any(|x| x.nombre == *s),
                "sitio desconocido: {s}"
            );
        }
    }

    /// El manual ensena los sitios con sus herramientas y como combinarlas;
    /// es fijo (prefijo de la cache) y de un tamano razonable.
    #[test]
    fn el_manual_ensena_sitios_herramientas_y_como_comprobar() {
        let m = manual();
        for parte in [
            "Lienzo: encontrar y cambiar",
            "editar_texto",
            "Documentos",
            "anotar_inteligente",
            "Tareas",
            "Leer → pensar tú → escribir",
            "Cómo compruebas",
            "Antes de decir que algo no se puede",
        ] {
            assert!(m.contains(parte), "falta «{parte}»");
        }
        assert_eq!(m, manual(), "fijo");
        assert!(m.chars().count() < 6000, "{} letras", m.chars().count());
        // Negativo: el mapa ensena, no prohibe.
        assert!(!m.contains("Nunca uses") && !m.contains("No puedes"));
    }
}
