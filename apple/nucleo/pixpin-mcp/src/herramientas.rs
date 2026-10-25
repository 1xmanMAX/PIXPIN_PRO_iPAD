//! **Las herramientas MCP**: las de leer, que corren aqui, y las de la app,
//! que salen de la tabla del asistente sin copiar nada.
//!
//! Una herramienta de la app es una fila de `pixpin_ia::acciones::todas()`:
//! nombre, descripcion y JSON Schema (`acciones::herramienta`) tal cual. Se
//! quedan fuera solo tres:
//!
//! - `consultar_tareas`, que no es un pedido sino una consulta: se contesta
//!   aqui, leyendo las listas del disco, con el mismo `tareas::consultar`
//!   del asistente (asi funciona tambien con PixPin cerrada).
//! - `hoja_a_lienzo`, que dibuja en el lienzo abierto del chat del
//!   asistente: desde fuera no hay chat ni lienzo al que llevarla.
//! - `leer_documento`, que en la tabla lee un adjunto del chat: aqui la
//!   tapa la de leer del mismo nombre, que lee una ruta del disco. Vale
//!   para cualquier fila que se llame como una local: gana la local.
//!
//! Las herramientas de leer son cosas que el asistente hace dentro del chat
//! con sus adjuntos (buscar, OCR) o que no tiene (ver la pantalla, el texto
//! de un documento cualquiera), adaptadas a rutas del disco.

use serde_json::{Map, Value, json};

use pixpin_ia::acciones::{self, Accion, CONSULTA};

/// Lo que devuelve una herramienta.
#[derive(Debug, Clone, PartialEq)]
pub enum Contenido {
    Texto(String),
    /// Una imagen (contenido `image` de MCP): sus bytes y su tipo.
    Imagen { datos: Vec<u8>, mime: &'static str },
}

pub type Resultado = Result<Vec<Contenido>, String>;

/// Una herramienta que corre en este exe.
pub struct Local {
    pub nombre: &'static str,
    pub titulo: &'static str,
    pub descripcion: &'static str,
    pub esquema: fn() -> Value,
    pub hacer: fn(&Map<String, Value>) -> Resultado,
}

/// **Las filas que, ademas de las que ya preguntan (`Accion::confirmar`),
/// pregunta PixPin antes de hacerlas cuando las pide un cliente MCP**: las
/// que cambian algo que el usuario ya tenia (una tarea, un documento
/// anotado). Crear (una tarea, una nota, un mensaje) no pregunta: no se
/// pierde nada y se ve en su aviso.
pub const MODIFICA: &[&str] = &["marcar_tarea", "mover_tarea", "editar_tarea", pixpin_ia::acciones_documentos::ANOTAR];

/// Las filas de la tabla que no se ofrecen (ver la cabecera).
fn fuera(a: &Accion) -> bool {
    a.accion == CONSULTA || a.herramienta == pixpin_ia::hoja::NOMBRE || local(a.herramienta).is_some()
}

/// Las filas de la tabla que se ofrecen como herramientas de la app.
pub fn de_la_app() -> impl Iterator<Item = &'static Accion> {
    acciones::todas().filter(|a| !fuera(a))
}

/// Si PixPin pregunta al usuario antes de hacer esta fila.
pub fn pregunta(a: &Accion) -> bool {
    a.confirmar.is_some() || MODIFICA.contains(&a.herramienta)
}

fn esquema_vacio() -> Value {
    json!({"type": "object", "properties": {}})
}

fn esquema_buscar() -> Value {
    pixpin_ia::documentos::herramienta::herramienta().parametros
}

fn esquema_tareas() -> Value {
    acciones::herramienta(acciones::por_nombre("consultar_tareas").expect("esta en la tabla")).parametros
}

fn esquema_ruta_y_paginas() -> Value {
    json!({
        "type": "object",
        "required": ["ruta"],
        "properties": {
            "ruta": {"type": "string", "description": "ruta absoluta del fichero en este PC"},
            "paginas": {"type": "string", "description": "solo esas paginas u hojas, desde 1 («3», «1-4, 7»); omitir: todas"}
        }
    })
}

fn esquema_ruta() -> Value {
    json!({
        "type": "object",
        "required": ["ruta"],
        "properties": {"ruta": {"type": "string", "description": "ruta absoluta de la imagen (png, jpg, bmp, webp, tif...) o del PDF"}}
    })
}

fn esquema_pantalla() -> Value {
    json!({
        "type": "object",
        "properties": {
            "ventana": {"type": "string", "description": "un trozo del titulo de la ventana (ver listar_ventanas); omitir: la pantalla entera"},
            "monitor": {"type": "integer", "description": "numero de monitor desde 1; omitir: el principal (sin «ventana»)"},
            "ancho_maximo": {"type": "integer", "description": "reducir si es mas ancha (px); omitir: 1600"}
        }
    })
}

/// **Las de leer.**
pub const LOCALES: &[Local] = &[
    Local {
        nombre: pixpin_ia::documentos::BUSCAR,
        titulo: "Buscar en mis cosas",
        descripcion: "Busca en los PDF, Word, Excel, PowerPoint, notas y chats del usuario guardados en PixPin (el mismo indice que usa su asistente). Devuelve trozos con su origen y como abrirlos (abrir_fichero con su ruta, o abrir_mensaje con su codigo).",
        esquema: esquema_buscar,
        hacer: crate::leer::buscar,
    },
    Local {
        nombre: "consultar_tareas",
        titulo: "Consultar tareas",
        descripcion: "Lee las tareas del usuario: pendientes, hechas o vencidas, por periodo (hoy, semana, mes), lista o chat. No cambia nada.",
        esquema: esquema_tareas,
        hacer: crate::leer::consultar_tareas,
    },
    Local {
        nombre: "listar_chats",
        titulo: "Listar chats",
        descripcion: "Los chats (proyectos) del usuario en PixPin, del mas reciente al mas viejo, con sus listas de tareas.",
        esquema: esquema_vacio,
        hacer: crate::leer::listar_chats,
    },
    Local {
        nombre: "leer_documento",
        titulo: "Leer un documento",
        descripcion: "El texto de un PDF (con OCR de Windows en las paginas escaneadas), Word (.docx), PowerPoint (.pptx), Excel (.xlsx/.xlsm), EPUB, HTML, Markdown o texto del disco.",
        esquema: esquema_ruta_y_paginas,
        hacer: crate::documento::leer_documento,
    },
    Local {
        nombre: "ocr_imagen",
        titulo: "OCR de una imagen",
        descripcion: "Lee el texto de una imagen o de un PDF escaneado con el OCR de Windows, en orden de lectura.",
        esquema: esquema_ruta,
        hacer: crate::documento::ocr_imagen,
    },
    Local {
        nombre: "ver_pantalla",
        titulo: "Ver la pantalla",
        descripcion: "Captura la pantalla (o una ventana por su titulo) y la devuelve como imagen, para ver lo que el usuario tiene delante. No guarda nada.",
        esquema: esquema_pantalla,
        hacer: crate::pantalla::ver_pantalla,
    },
    Local {
        nombre: "listar_ventanas",
        titulo: "Listar ventanas",
        descripcion: "Las ventanas que se ven ahora, de delante a atras, con su titulo y su tamano (para ver_pantalla).",
        esquema: esquema_vacio,
        hacer: crate::pantalla::listar_ventanas,
    },
];

fn local(nombre: &str) -> Option<&'static Local> {
    LOCALES.iter().find(|l| l.nombre == nombre)
}

fn de_la_app_por_nombre(nombre: &str) -> Option<&'static Accion> {
    de_la_app().find(|a| a.herramienta == nombre)
}

/// Si hay una herramienta con ese nombre.
pub fn existe(nombre: &str) -> bool {
    local(nombre).is_some() || de_la_app_por_nombre(nombre).is_some()
}

/// **`tools/list`.**
pub fn lista() -> Vec<Value> {
    let mut v: Vec<Value> = LOCALES
        .iter()
        .map(|l| {
            json!({
                "name": l.nombre,
                "title": l.titulo,
                "description": l.descripcion,
                "inputSchema": (l.esquema)(),
                "annotations": {"readOnlyHint": true, "openWorldHint": false},
            })
        })
        .collect();
    for a in de_la_app() {
        let h = acciones::herramienta(a);
        let mut descripcion = h.descripcion;
        descripcion.push_str(" Lo hace la PixPin abierta.");
        if pregunta(a) {
            descripcion.push_str(" PixPin pide permiso al usuario antes.");
        }
        v.push(json!({
            "name": h.nombre,
            "description": descripcion,
            "inputSchema": h.parametros,
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": pregunta(a),
                "openWorldHint": false,
            },
        }));
    }
    v
}

/// El `result` de `tools/call`.
pub fn resultado(r: Resultado) -> Value {
    match r {
        Ok(contenido) => {
            let c: Vec<Value> = contenido
                .into_iter()
                .map(|c| match c {
                    Contenido::Texto(t) => json!({"type": "text", "text": t}),
                    Contenido::Imagen { datos, mime } => {
                        json!({"type": "image", "data": pixpin_ia::base64::codificar(&datos), "mimeType": mime})
                    }
                })
                .collect();
            json!({"content": c, "isError": false})
        }
        Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
    }
}

/// **Hace una herramienta** (`None`: no existe). Puede tardar: la llama el
/// bucle en su propio hilo.
pub fn llamar(nombre: &str, args: &Map<String, Value>) -> Option<Value> {
    if let Some(l) = local(nombre) {
        return Some(resultado((l.hacer)(args)));
    }
    let a = de_la_app_por_nombre(nombre)?;
    Some(resultado(crate::escribir::hacer(a, args).map(|t| vec![Contenido::Texto(t)])))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_lista_sale_de_la_tabla_sin_repetir_nombres() {
        let l = lista();
        let mut nombres: Vec<&str> = l.iter().map(|h| h["name"].as_str().unwrap()).collect();
        let n = nombres.len();
        nombres.sort();
        nombres.dedup();
        assert_eq!(nombres.len(), n, "un nombre repetido");
        // Todas las filas de la tabla menos las tres de fuera, y las locales.
        assert_eq!(n, acciones::todas().count() - 3 + LOCALES.len());
        for h in &l {
            assert_eq!(h["inputSchema"]["type"], "object", "{}", h["name"]);
            assert!(!h["description"].as_str().unwrap().is_empty());
        }
        // El esquema es el de la tabla, no una copia.
        let abrir = l.iter().find(|h| h["name"] == "abrir_ventana").unwrap();
        assert_eq!(abrir["inputSchema"], acciones::herramienta(acciones::por_nombre("abrir_ventana").unwrap()).parametros);
        assert!(!existe("hoja_a_lienzo") && existe("consultar_tareas") && existe("anadir_tarea"));
    }

    #[test]
    fn lo_que_borra_o_cambia_va_marcado() {
        let l = lista();
        let marcado = |n: &str| l.iter().find(|h| h["name"] == n).unwrap()["annotations"]["destructiveHint"] == true;
        for n in ["borrar_lista", "quitar_tarea", "borrar_captura", "enviar_al_movil", "editar_tarea", "anotar_documento"] {
            assert!(marcado(n), "{n}");
        }
        for n in ["anadir_tarea", "nota_nueva", "escribir_en_chat"] {
            assert!(!marcado(n), "{n}");
        }
        let leer = l.iter().find(|h| h["name"] == "ver_pantalla").unwrap();
        assert_eq!(leer["annotations"]["readOnlyHint"], true);
    }

    #[test]
    fn el_resultado_lleva_texto_imagen_o_error() {
        let r = resultado(Ok(vec![Contenido::Texto("hola".into()), Contenido::Imagen { datos: vec![1, 2, 3], mime: "image/png" }]));
        assert_eq!(r["isError"], false);
        assert_eq!(r["content"][0]["text"], "hola");
        assert_eq!(r["content"][1]["type"], "image");
        assert_eq!(r["content"][1]["data"], "AQID");
        let e = resultado(Err("mal".into()));
        assert_eq!(e["isError"], true);
        assert_eq!(e["content"][0]["text"], "mal");
    }
}
