//! **Las herramientas del asistente: el protocolo comun PC/Android y la
//! validacion de lo que el modelo pide.**
//!
//! Cada herramienta se describe con nombre, descripcion y parametros en JSON
//! Schema —la forma de `tools` de la API de chat de OpenAI, que entienden
//! llama.cpp, LiteRT-LM y la plantilla de Qwen—. El movil usa **el mismo
//! JSON** ([`catalogo_json`]): asi un modelo que aprendio a llamar
//! `dibujar_en_lienzo` en el PC lo llama igual en el telefono, y el resultado
//! (elementos de Excalidraw) se abre igual en los dos.
//!
//! ## Por que una validacion estricta
//!
//! Un modelo de 2B parametros acierta casi siempre la forma de la llamada
//! (medido: 6 de 6 en este PC) pero no siempre el contenido: un texto sin
//! `texto`, una coordenada de 90 000, una grafica con mas etiquetas que
//! valores. Lo que entra al lienzo tiene que ser correcto, asi que:
//!
//! - **La estructura es estricta**: tipos, campos obligatorios, rangos y
//!   listas enumeradas. Si falla, la llamada NO se ejecuta y el modelo recibe
//!   el motivo en espanol para corregirse ([`ErrorHerramienta::para_el_modelo`]);
//!   el bucle del agente reintenta (ver `agente.rs`).
//! - **Lo cosmetico es tolerante**: un color que no se entiende pasa a negro
//!   y queda un aviso. El usuario acepta alucinaciones pequenas; no acepta un
//!   lienzo roto ni esperar otra vuelta por un color.
//! - **El JSON roto se intenta arreglar** una vez ([`reparar_json`]: vallas de
//!   Markdown, texto alrededor, comas sobrantes) antes de rendirse.

use serde_json::{Map, Value, json};

use crate::motor::LlamadaCruda;

/// Lo que mide el lienzo que se le describe al modelo. Las coordenadas que
/// devuelve son de este espacio; el lienzo real las coloca donde mira el
/// usuario (ver `lienzo.rs`).
pub const ANCHO_LIENZO: f64 = 800.0;
pub const ALTO_LIENZO: f64 = 600.0;

/// Lo mas lejos que se acepta una coordenada (un poco fuera del lienzo se
/// permite: el modelo a veces se pasa del borde).
pub const COORDENADA_MAXIMA: f64 = 2000.0;
pub const COORDENADA_MINIMA: f64 = -200.0;
pub const MAX_ELEMENTOS: usize = 60;
pub const MAX_DATOS_GRAFICA: usize = 40;
pub const MAX_TEXTO: usize = 500;

/// Una herramienta tal como se le describe al modelo.
#[derive(Debug, Clone, PartialEq)]
pub struct Herramienta {
    pub nombre: String,
    pub descripcion: String,
    /// JSON Schema de los argumentos.
    pub parametros: Value,
}

impl Herramienta {
    pub fn a_json(&self) -> Value {
        json!({"type": "function", "function": {
            "name": self.nombre, "description": self.descripcion, "parameters": self.parametros}})
    }
}

pub const DIBUJAR: &str = "dibujar_en_lienzo";
pub const GRAFICA: &str = "grafica";
pub const OCR: &str = "ocr";
pub const CALCAR: &str = "calcar_imagen";
pub const AYUDA: &str = "ayuda_app";
pub const SVG: &str = crate::svg::NOMBRE;
pub const FUNCION: &str = crate::funciones::NOMBRE;
pub const ECUACION: &str = crate::ecuacion::NOMBRE;
pub const CRONOGRAMA: &str = crate::cronograma::NOMBRE;
pub const FIGURAS: &str = crate::figuras::NOMBRE;
/// Para estudiantes (14-oct): diagramas de fisica de una tabla de
/// plantillas y problemas pasados en limpio.
pub const FISICA: &str = crate::fisica::NOMBRE;
pub const PROBLEMA: &str = crate::problema::NOMBRE;

/// Cuantos numeros como mucho en los `puntos` de una figura.
pub const MAX_NUMEROS_PUNTOS: usize = 240;

/// **Todas las herramientas**: las propias del asistente ([`catalogo_base`])
/// y las de la app, generadas de su tabla (`acciones.rs`). Al modelo no se le
/// ensenan todas a la vez: `seleccion::para_el_pedido` elige las pocas que
/// vienen a cuento en cada turno (un 2B se pierde con 30).
pub fn catalogo() -> Vec<Herramienta> {
    let mut c = catalogo_base();
    // Editar lo que ya hay en el lienzo (`edicion.rs`, tabla propia).
    c.extend(crate::edicion::herramientas());
    c.extend(crate::acciones::herramientas());
    c
}

/// **Lo que la nube sabe de mas** (banco del 15-oct): la tabla y el
/// paralelogramo de `dibujar_en_lienzo`, y que herramienta va con «pasa en
/// limpio el problema» y con «haz una tabla». Solo para los motores de la
/// nube (`agente`, con `catalogo_completo`): el local sigue con el catalogo
/// de siempre, byte a byte (decision del usuario: al local no se le toca).
/// Fijo para la misma entrada, que el catalogo es prefijo de la cache.
pub fn para_la_nube(lista: &mut [Herramienta]) {
    for h in lista.iter_mut() {
        crate::edicion::para_la_nube(h);
        crate::acciones::para_la_nube(h);
        let extra = match h.nombre.as_str() {
            DIBUJAR => {
                if let Some(t) = h.parametros.pointer_mut("/properties/elementos/items/properties/tipo/enum").and_then(Value::as_array_mut) {
                    for nuevo in ["paralelogramo", "tabla"] {
                        if !t.iter().any(|v| v == nuevo) {
                            t.push(json!(nuevo));
                        }
                    }
                }
                if let Some(p) = h.parametros.pointer_mut("/properties/elementos/items/properties").and_then(Value::as_object_mut) {
                    p.insert(
                        "filas".into(),
                        json!({"type": "array", "items": {"type": "array", "items": {"type": "string"}}, "description": "tabla: filas de celdas, la primera es la cabecera"}),
                    );
                    p.insert("rugosidad".into(), json!({"type": "number", "description": "0 trazo exacto (geometria, planos), 1 a mano"}));
                    // El texto rico del lienzo (`pixpin_motor2d::texto_rico`).
                    p.insert(
                        "texto".into(),
                        json!({"type": "string", "description": "texto rico: formulas en linea entre $...$ (LaTeX), \\color{rojo}{X}, **negrita**, *cursiva*, <u>sub</u>, renglones con \\n, \\centro al inicio de un renglon"}),
                    );
                }
                "Tambien: tabla (con «x», «y» y «filas»:[[\"Planeta\",\"UA\"],[\"Mercurio\",\"0.39\"]], la primera fila es la cabecera; es LA forma de hacer una tabla en el lienzo, no una nota) \
                 y paralelogramo (con «caja»). En un diagrama de flujo: inicio/fin elipse, proceso rectangulo, decision rombo, leer/escribir paralelogramo. \
                 En geometria, fisica o planos pon «rugosidad»:0 en cada figura (la circunferencia exacta, no a mano), calcula las coordenadas de verdad \
                 (centro y radio de una inscrita), y cada punto lleva su letra UNA vez: los textos de al lado no la repiten («incentro», no «I incentro»). \
                 Un «texto» es TEXTO RICO: formulas en linea entre $...$ con LaTeX (\\frac, ^, _, \\sqrt, \\overline{AB} segmento, \\overset{\\frown}{AC} arco, \\vec{v}, \\sphericalangle o ∢ para «m∢AOB», ^\\circ grados, \\theta, \\pi, \\therefore ∴, \\perp, \\parallel, \\cong, \\boxed{}), \
                 colores con \\color{rojo}{X} (dentro o fuera de $...$), **negrita**, *cursiva*, <u>subrayado</u>, ~~tachado~~, \\tam{1.4}{grande}; varios renglones con \\n y \\centro, \\derecha o \\justificado al inicio de un renglon. \
                 Para la resolucion de un problema, UN texto por bloque con sus formulas en linea, no ecuaciones sueltas: \
                 {\"tipo\":\"texto\",\"x\":40,\"y\":60,\"texto\":\"Nos piden $m\\\\sphericalangle COD = \\\\color{rojo}{X}$\\n• Se observa que $\\\\overline{AB}$ es diametro:\\n\\\\centro $45^\\\\circ = \\\\frac{\\\\color{azul}{2a+2b}}{2}$\"}."
            }
            PROBLEMA => {
                "Es LA herramienta para «pasa en limpio el problema/ejercicio» (no hoja_a_lienzo). El enunciado SIN las formulas; cada formula entera una sola vez en «ecuaciones» \
                 (LaTeX: \\frac, \\int_a^b, \\sqrt, matrices con \\begin{bmatrix}, sistemas con \\begin{cases}); incognitas en LaTeX; «figura» SOLO si la foto trae un dibujo \
                 necesario (triangulo, circuito, plano inclinado), nunca para integrales, matrices o ecuaciones."
            }
            "nota_nueva" => "Solo si piden una nota o un apunte: una tabla, un diagrama o una grafica van al lienzo.",
            crate::hoja::NOMBRE => "Tambien es la de «calca/pasa esta pagina al lienzo» con un PDF adjunto (calcar_imagen solo calca fotos). \
                                    NO para pasar en limpio o resolver un problema o ejercicio: eso es problema_en_limpio.",
            // Banco de documentos (15-oct): «convierte estas formulas a LaTeX»
            // acababa dibujado en el lienzo sin una linea de LaTeX en la respuesta.
            ECUACION => "Solo si piden las formulas EN EL LIENZO. Si piden el LaTeX («pasame en LaTeX», «convierte a LaTeX»), \
                         contesta el codigo en el texto, una formula por renglon entre $$...$$, sin herramienta.",
            // «Anota en la imagen los pasos» acababa en una tarea pendiente.
            "anadir_tarea" => "NO para «anota/marca/señala en el documento, la foto o el PDF»: eso es anotar_inteligente o anotar_documento.",
            crate::edicion::ESTILO => "Con «tam_letra» cambia el tamano de la letra de los textos (y los de una tabla).",
            crate::acciones_documentos::ANOTAR => "Solo PDF, Word, PowerPoint o plano, y para un texto concreto que dice el usuario. \
                                                   NUNCA para «lo importante», resumir o explicar al margen, ni varias notas a mano: eso es anotar_inteligente, en UNA llamada.",
            crate::anotacion::NOMBRE => "Es LA herramienta para «señala/marca lo importante», «resume cada seccion al margen», «explica paso a paso» o «marca los errores» \
                                         en un documento, foto o Excel del chat: lee y coloca ella sola, en una llamada (no escribas tu las notas con anotar_documento).",
            // «Haz esta tabla en el lienzo» acababa como tabla del chat.
            crate::acciones_documentos::TABLA => "Con «destino»:«lienzo» la dibuja en el lienzo IGUAL que en la foto (letra, anchos, colores de fondo y borde medidos): \
                                                  es LA forma de «haz/pasa/copia esta tabla en el lienzo», no la dibujes tu.",
            // «Marca los errores del Excel» acababa contestado en el chat.
            "leer_excel" => "Para MARCAR o explicar algo EN el Excel (errores, lo importante), anotar_inteligente: lee las celdas por su cuenta.",
            _ => continue,
        };
        if !h.descripcion.ends_with(extra) {
            h.descripcion = format!("{} {extra}", h.descripcion.trim_end());
        }
    }
}

/// Las propias del asistente. Las descripciones son cortas a proposito: cada
/// token del catalogo se paga en cada vuelta. La de dibujar lleva dos
/// ejemplos: con ellos un modelo pequeno acierta la forma de las figuras
/// compuestas (casa, diagrama) mucho mas que con la sola lista de campos.
pub fn catalogo_base() -> Vec<Herramienta> {
    vec![
        Herramienta {
            nombre: DIBUJAR.into(),
            descripcion: concat!(
                "Dibuja figuras en el lienzo de PixPin (800x600, origen arriba a la izquierda). ",
                "Cada elemento lleva su «color» (el que pidio el usuario) y, si va lleno, su «relleno». ",
                "Tipos: rectangulo, elipse, rombo, triangulo, estrella y onda (con «caja»:[x,y,ancho,alto]; un circulo tambien con «centro»:[x,y] y «radio»); ",
                "linea, flecha, curva (suave), poligono y lapiz (a mano alzada) (con «puntos»:[x1,y1,x2,y2,...]); texto (con «x», «y» y «texto»); ",
                "punto (con «x», «y» y su letra en «texto»); instrumento (plano cartesiano con «ejes»:2, recta con 1, espacio con 3; «caja», «unidad» en px y «tabla»:[[x,y],...] de coordenadas). ",
                "Opcionales: «grosor», «trazo» (solido, discontinuo, punteado), «opacidad» (0-100), «relleno_estilo» (solido, rayado, cruzado), ",
                "«material» del lapiz (lisa, tiza, grafito) con su «dureza» (4H, 2H, HB, 2B, 4B, 6B, 8B) y «capa» (las mayores van encima). ",
                "Una figura con «texto» lo lleva escrito dentro (o encima si es flecha). «grupo»: un nombre comun para agrupar varias. ",
                "Ejemplo, una casa: {\"elementos\":[{\"tipo\":\"rectangulo\",\"caja\":[300,280,200,160],\"color\":\"marron\"},",
                "{\"tipo\":\"triangulo\",\"caja\":[280,170,240,110],\"color\":\"rojo\",\"relleno\":\"rojo\"},",
                "{\"tipo\":\"rectangulo\",\"caja\":[380,360,40,80],\"color\":\"azul\",\"relleno\":\"azul\"}]}. ",
                "Un paso de un diagrama: {\"tipo\":\"rectangulo\",\"caja\":[60,260,160,70],\"texto\":\"Inicio\"},",
                "{\"tipo\":\"flecha\",\"puntos\":[220,295,320,295]}. ",
                "Para copiar exacta una foto adjunta usa calcar_imagen; para dibujarla con figuras o a lapiz, interpretala con estas figuras. ",
                "Para ilustraciones con curvas o mucho detalle, mejor dibujar_svg. ",
                "Para geometria exacta de lamina (arcos, semicircunferencias, cuadrantes, sectores, marcas de angulo, angulo recto, puntos con letra, degradados, raya-punto) usa geometria_precisa."
            )
            .into(),
            parametros: json!({
                "type": "object",
                "required": ["elementos"],
                "properties": {
                    "elementos": {"type": "array", "minItems": 1, "maxItems": MAX_ELEMENTOS, "items": {
                        "type": "object",
                        "required": ["tipo"],
                        "properties": {
                            "tipo": {"type": "string", "enum": ["rectangulo", "elipse", "rombo", "triangulo", "estrella", "onda", "linea", "flecha", "curva", "poligono", "lapiz", "texto", "punto", "instrumento"]},
                            "caja": {"type": "array", "items": {"type": "number"}, "description": "[x, y, ancho, alto]"},
                            "puntos": {"type": "array", "items": {"type": "number"}, "description": "[x1, y1, x2, y2, ...] de linea, flecha, curva o poligono"},
                            "centro": {"type": "array", "items": {"type": "number"}, "description": "[x, y] de un circulo"},
                            "radio": {"type": "number"},
                            "x": {"type": "number"},
                            "y": {"type": "number"},
                            "color": {"type": "string", "description": "color del trazo: rojo, azul, verde, amarillo, naranja, morado, rosa, gris, marron, negro, blanco o #rrggbb"},
                            "relleno": {"type": "string", "description": "color de relleno (los mismos nombres); omitir si va vacia"},
                            "texto": {"type": "string"},
                            "tam_letra": {"type": "number"},
                            "grupo": {"type": "string"},
                            "grosor": {"type": "number", "description": "grosor del trazo, 1 a 40"},
                            "trazo": {"type": "string", "enum": ["solido", "discontinuo", "punteado"]},
                            "opacidad": {"type": "number", "description": "0 a 100"},
                            "relleno_estilo": {"type": "string", "enum": ["solido", "rayado", "cruzado"]},
                            "material": {"type": "string", "enum": ["lisa", "tiza", "grafito", "lapiz2b", "seco"]},
                            "dureza": {"type": "string", "enum": ["4H", "2H", "HB", "2B", "4B", "6B", "8B"]},
                            "capa": {"type": "number", "description": "orden: las mayores encima"},
                            "ejes": {"type": "number", "description": "instrumento: 1, 2 o 3"},
                            "unidad": {"type": "number", "description": "instrumento: px por unidad"},
                            "tabla": {"type": "array", "items": {"type": "array", "items": {"type": "number"}}, "description": "instrumento: [[x, y], ...]"}
                        }
                    }},
                    "origen": {"type": "array", "items": {"type": "number"}, "description": "[x, y] que se suma a todas las coordenadas"}
                }
            }),
        },
        Herramienta {
            nombre: GRAFICA.into(),
            descripcion: "Dibuja en el lienzo una grafica de barras, lineas o pastel a partir de datos.".into(),
            parametros: json!({
                "type": "object",
                "required": ["tipo", "etiquetas", "valores"],
                "properties": {
                    "tipo": {"type": "string", "enum": ["barras", "lineas", "pastel"]},
                    "titulo": {"type": "string"},
                    "etiquetas": {"type": "array", "items": {"type": "string"}, "maxItems": MAX_DATOS_GRAFICA},
                    "valores": {"type": "array", "items": {"type": "number"}, "maxItems": MAX_DATOS_GRAFICA}
                }
            }),
        },
        crate::svg::herramienta(),
        crate::funciones::herramienta(),
        crate::ecuacion::herramienta(),
        crate::cronograma::herramienta(),
        crate::fisica::herramienta(),
        crate::problema::herramienta(),
        crate::figuras::herramienta(),
        Herramienta {
            nombre: OCR.into(),
            descripcion: "Reconoce el texto de un PDF escaneado o de una imagen sin texto seleccionable, con el OCR de Windows. Usala cuando el usuario quiera el texto de un documento adjunto.".into(),
            parametros: json!({
                "type": "object",
                "properties": {
                    "archivo": {"type": "string", "description": "nombre del adjunto; omitir para el ultimo adjunto"},
                    "paginas": {"type": "string", "description": "ej. 1-3,5; omitir para todas"},
                    "idioma": {"type": "string", "enum": ["es", "en", "auto"]}
                }
            }),
        },
        Herramienta {
            nombre: CALCAR.into(),
            descripcion: "Copia (calca) en el lienzo la imagen o foto adjunta, con sus formas y colores exactos. Usala siempre que pidan copiar, pasar o calcar una imagen al lienzo.".into(),
            parametros: json!({
                "type": "object",
                "properties": {
                    "detalle": {"type": "string", "enum": ["bajo", "medio", "alto"], "description": "omitir: medio"},
                    "colores": {"type": "boolean", "description": "omitir; false solo si piden blanco y negro"},
                    "solo_contornos": {"type": "boolean", "description": "omitir; true solo si piden solo los contornos o bordes"},
                    "zona": {"type": "array", "items": {"type": "number"}, "description": "[x, y, ancho, alto] en fracciones de 0 a 1, para calcar solo un trozo"},
                    "imagen": {"type": "string", "description": "nombre del adjunto; omitir para la ultima imagen"}
                }
            }),
        },
        Herramienta {
            nombre: AYUDA.into(),
            descripcion: "Busca en la ayuda de PixPin donde esta algo o como se hace (menus, atajos, ventanas). Usala ante cualquier pregunta sobre la app, antes de contestar.".into(),
            parametros: json!({
                "type": "object",
                "required": ["pregunta"],
                "properties": {"pregunta": {"type": "string"}}
            }),
        },
        // Buscar en los documentos y chats del usuario (`documentos`).
        crate::documentos::herramienta::herramienta(),
    ]
}

/// El catalogo como texto JSON (lista de `tools`), para copiarlo tal cual al
/// movil o a la documentacion.
pub fn catalogo_json() -> String {
    let lista: Vec<Value> = catalogo().iter().map(Herramienta::a_json).collect();
    serde_json::to_string_pretty(&lista).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Lo que pide el modelo, ya validado.

#[derive(Debug, Clone, PartialEq)]
pub enum Llamada {
    Dibujar(Vec<Figura>),
    Grafica(Grafica),
    Ocr(PeticionOcr),
    /// Calcar la imagen adjunta: el ejecutor, que tiene la imagen, llama a
    /// `calcar::calcar` y pone `lienzo::elementos_de_calcado`.
    Calcar(crate::calcar::Parametros),
    /// Una accion de la app: el pedido del protocolo, listo para mandar
    /// (preguntando antes si lleva `confirmar`).
    App(crate::acciones::AccionApp),
    /// Una pregunta sobre la app: se contesta con `conocimiento::responder`.
    Ayuda(String),
    /// Una ilustracion en SVG, ya leida y saneada (`svg.rs`).
    Svg(crate::svg::Svg),
    /// Funciones y = f(x) sobre el plano cartesiano (`funciones.rs`).
    Funciones(crate::funciones::Funciones),
    /// Una ecuacion compuesta (`ecuacion.rs`).
    Ecuacion(crate::ecuacion::Ecuacion),
    /// Un diagrama de Gantt (`cronograma.rs`).
    Cronograma(crate::cronograma::Cronograma),
    /// Un diagrama de fisica de su plantilla (`fisica.rs`).
    Fisica(crate::fisica::Diagrama),
    /// Un problema pasado en limpio (`problema.rs`).
    Problema(crate::problema::Problema),
    /// Guardar, poner o listar figuras de la biblioteca (`figuras.rs`): la
    /// ejecuta la app, que sabe donde vive la biblioteca y que hay elegido.
    Figuras(crate::figuras::PedidoFiguras),
    /// Buscar en los documentos y chats del usuario: el ejecutor busca en
    /// su indice (`documentos::Indice`) con el alcance del chat.
    Buscar(crate::documentos::PeticionBusqueda),
    /// Editar lo que ya hay en el lienzo (`edicion.rs`): el ejecutor la manda
    /// a quien tiene la escena, que la aplica con `edicion::aplicar`.
    Editar(crate::edicion::Edicion),
}

impl Llamada {
    pub fn nombre(&self) -> &'static str {
        match self {
            Llamada::Editar(e) => e.herramienta,
            Llamada::Dibujar(_) => DIBUJAR,
            Llamada::Grafica(_) => GRAFICA,
            Llamada::Ocr(_) => OCR,
            Llamada::Calcar(_) => CALCAR,
            Llamada::App(a) => a.herramienta,
            Llamada::Ayuda(_) => AYUDA,
            Llamada::Svg(_) => SVG,
            Llamada::Funciones(_) => FUNCION,
            Llamada::Ecuacion(_) => ECUACION,
            Llamada::Cronograma(_) => CRONOGRAMA,
            Llamada::Fisica(_) => FISICA,
            Llamada::Problema(_) => PROBLEMA,
            Llamada::Figuras(_) => FIGURAS,
            Llamada::Buscar(_) => crate::documentos::BUSCAR,
        }
    }

    /// Si lo que pide se dibuja tal cual con `lienzo::elementos` (sin mas
    /// datos que la llamada): el ejecutor lo manda al lienzo sin mirar cual
    /// es. Una herramienta de dibujo nueva solo tiene que contestar aqui.
    pub fn dibuja(&self) -> bool {
        matches!(
            self,
            Llamada::Dibujar(_)
                | Llamada::Grafica(_)
                | Llamada::Svg(_)
                | Llamada::Funciones(_)
                | Llamada::Ecuacion(_)
                | Llamada::Cronograma(_)
                | Llamada::Fisica(_)
                | Llamada::Problema(_)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoFigura {
    Rectangulo,
    Elipse,
    Rombo,
    Linea,
    Flecha,
    Texto,
    /// Cerrado; con relleno sale como mancha rellena (`pixpin-region`).
    Poligono,
    /// Linea suave que pasa por sus puntos (`line` con `roundness`).
    Curva,
    /// Trazo a mano alzada (`freedraw`), con su material: grafito con su
    /// dureza, tiza...
    Lapiz,
    /// Un punto con su letra (`pixpin-point`): la A, la B de un croquis.
    Punto,
    /// El instrumento de graficar (`pixpin-axes` y hermanos), con sus puntos
    /// de la tabla de coordenadas.
    Instrumento,
    /// **Una tabla en el lienzo** (`tabla_dibujada`, la misma de la hoja al
    /// lienzo): sus celdas van en `texto`, con tabulador entre celdas y salto
    /// entre filas. Antes no habia y «haz una tabla» acababa en una nota.
    Tabla,
}

/// Las filas de una tabla pedida (`filas` como lista de listas, o un texto
/// con `|` o tabuladores), en el formato de [`TipoFigura::Tabla`].
fn filas_de_tabla(o: &Map<String, Value>) -> Option<String> {
    let celda = |v: &Value| match v {
        Value::String(s) => s.replace(['\t', '\n'], " ").trim().to_string(),
        Value::Null => String::new(),
        otro => otro.to_string(),
    };
    let filas: Vec<Vec<String>> = match o.get("filas").or(o.get("celdas")).or(o.get("datos")).or(o.get("texto")) {
        Some(Value::Array(l)) => l
            .iter()
            .filter_map(|f| match f {
                Value::Array(c) => Some(c.iter().map(celda).collect()),
                Value::String(s) => Some(s.split(['|', '\t']).map(|c| c.trim().to_string()).collect()),
                _ => None,
            })
            .collect(),
        Some(Value::String(s)) => s
            .lines()
            .map(|l| l.trim().trim_matches('|').split(['|', '\t']).map(|c| c.trim().to_string()).collect::<Vec<_>>())
            .filter(|f: &Vec<String>| !f.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')))
            .collect(),
        _ => return None,
    };
    let filas: Vec<Vec<String>> = filas.into_iter().filter(|f| f.iter().any(|c| !c.is_empty())).collect();
    if filas.is_empty() {
        return None;
    }
    Some(filas.iter().map(|f| f.join("\t")).collect::<Vec<_>>().join("\n"))
}

/// Como se pinta una figura, mas alla de sus colores. `None`: lo de
/// siempre del lienzo. Las palabras ya van como las escribe el fichero.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EstiloFigura {
    pub grosor: Option<f64>,
    /// `solid`, `dashed` o `dotted`.
    pub trazo: Option<&'static str>,
    /// De 0 a 100.
    pub opacidad: Option<f64>,
    /// `solid`, `hachure` o `cross-hatch`.
    pub relleno: Option<&'static str>,
    pub rugosidad: Option<f64>,
    /// La palabra del `material` (`cuadritos` es el grafito).
    pub material: Option<&'static str>,
    /// La dureza del grafito: `4H` ... `8B`.
    pub dureza: Option<&'static str>,
}

/// El instrumento que se pide: sus ejes, cuanto mide la unidad y los puntos
/// de su tabla de coordenadas (en unidades del instrumento).
#[derive(Debug, Clone, PartialEq)]
pub struct PedidoInstrumento {
    pub ejes: u8,
    pub unidad: f64,
    pub tabla: Vec<(f64, f64)>,
}

/// Cuantos puntos como mucho en la tabla de un instrumento.
pub const MAX_TABLA: usize = 60;

/// Un color como `#rrggbb`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Color(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct Figura {
    pub tipo: TipoFigura,
    pub x: f64,
    pub y: f64,
    pub ancho: f64,
    pub alto: f64,
    /// Para lineas y flechas: el otro extremo.
    pub x2: f64,
    pub y2: f64,
    pub color: Color,
    pub relleno: Option<Color>,
    pub texto: String,
    pub tam_letra: f64,
    /// Si el modelo puso algun color (de trazo o de relleno). Sin el, el
    /// color se saca del pedido ([`colores_del_pedido`]).
    pub con_color: bool,
    /// Lineas, flechas, curvas y poligonos: todos sus puntos (el primero es
    /// (x, y) y el ultimo (x2, y2)). Vacio en las demas.
    pub puntos: Vec<(f64, f64)>,
    /// Las figuras con el mismo grupo se mueven juntas en el lienzo.
    pub grupo: Option<String>,
    pub estilo: EstiloFigura,
    /// El orden: las de capa mayor se pintan encima (a igual capa, el de
    /// la lista).
    pub capa: i32,
    pub instrumento: Option<PedidoInstrumento>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoGrafica {
    Barras,
    Lineas,
    Pastel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Grafica {
    pub tipo: TipoGrafica,
    pub titulo: String,
    pub etiquetas: Vec<String>,
    pub valores: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PeticionOcr {
    /// `None`: el ultimo adjunto de la conversacion.
    pub archivo: Option<String>,
    /// Rangos de paginas, de 1 en adelante, ambos incluidos. Vacio: todas.
    pub paginas: Vec<(u32, u32)>,
    /// `None`: el idioma del sistema.
    pub idioma: Option<String>,
}

/// Una llamada correcta, con los arreglos cosmeticos que se le hicieron.
#[derive(Debug, Clone, PartialEq)]
pub struct Validada {
    pub llamada: Llamada,
    pub avisos: Vec<String>,
}

/// Por que no se ejecuta una llamada.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ErrorHerramienta {
    #[error("la herramienta «{0}» no existe")]
    Desconocida(String),
    #[error("los argumentos no son JSON valido")]
    JsonRoto,
    #[error("{0}")]
    Parametro(String),
}

impl ErrorHerramienta {
    /// El mensaje que se le devuelve al modelo para que se corrija.
    pub fn para_el_modelo(&self, conocidas: &[Herramienta]) -> String {
        let nombres: Vec<&str> = conocidas.iter().map(|h| h.nombre.as_str()).collect();
        match self {
            ErrorHerramienta::Desconocida(n) => format!(
                "ERROR: la herramienta «{n}» no existe. Las que hay son: {}. Usa una de ellas o contesta con texto.",
                nombres.join(", ")
            ),
            ErrorHerramienta::JsonRoto => {
                "ERROR: los argumentos no son JSON valido. Vuelve a llamar con un objeto JSON correcto.".into()
            }
            ErrorHerramienta::Parametro(m) => {
                format!("ERROR: {m}. Corrige los argumentos y vuelve a llamar.")
            }
        }
    }
}

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

/// **Intenta arreglar un JSON roto** de los que escriben los modelos
/// pequenos: entre vallas de Markdown, con texto delante o detras, o con una
/// coma antes de `}`/`]`. Solo arreglos que no cambian el sentido.
pub fn reparar_json(texto: &str) -> Option<Value> {
    let t = texto.trim();
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        return Some(v);
    }
    let desde = t.find('{')?;
    let hasta = t.rfind('}')?;
    if hasta <= desde {
        return None;
    }
    let trozo = &t[desde..=hasta];
    if let Ok(v) = serde_json::from_str::<Value>(trozo) {
        return Some(v);
    }
    // Comas colgantes, fuera de cadenas.
    let mut limpio = String::with_capacity(trozo.len());
    let mut en_cadena = false;
    let mut escapado = false;
    let car: Vec<char> = trozo.chars().collect();
    for (i, &c) in car.iter().enumerate() {
        if en_cadena {
            limpio.push(c);
            if escapado {
                escapado = false;
            } else if c == '\\' {
                escapado = true;
            } else if c == '"' {
                en_cadena = false;
            }
            continue;
        }
        if c == '"' {
            en_cadena = true;
        }
        if c == ',' {
            let siguiente = car[i + 1..].iter().find(|c| !c.is_whitespace());
            if matches!(siguiente, Some('}') | Some(']')) {
                continue;
            }
        }
        limpio.push(c);
    }
    serde_json::from_str::<Value>(&limpio).ok()
}

/// **Valida una llamada del modelo** contra las herramientas conocidas.
pub fn validar(cruda: &LlamadaCruda, conocidas: &[Herramienta]) -> Result<Validada, ErrorHerramienta> {
    let nombre = cruda.nombre.trim();
    if !conocidas.iter().any(|h| h.nombre == nombre) {
        return Err(ErrorHerramienta::Desconocida(nombre.to_string()));
    }
    let args = if cruda.argumentos.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        reparar_json(&cruda.argumentos).ok_or(ErrorHerramienta::JsonRoto)?
    };
    let Value::Object(args) = args else {
        return Err(mal("los argumentos tienen que ser un objeto JSON"));
    };
    let mut avisos = Vec::new();
    let llamada = match nombre {
        DIBUJAR => Llamada::Dibujar(validar_dibujo(&args, &mut avisos)?),
        GRAFICA => Llamada::Grafica(validar_grafica(&args, &mut avisos)?),
        SVG => Llamada::Svg(crate::svg::validar(&args, &mut avisos)?),
        FUNCION => Llamada::Funciones(crate::funciones::validar(&args, &mut avisos)?),
        ECUACION => Llamada::Ecuacion(crate::ecuacion::validar(&args, &mut avisos)?),
        CRONOGRAMA => Llamada::Cronograma(crate::cronograma::validar(&args, &mut avisos)?),
        FISICA => Llamada::Fisica(crate::fisica::validar(&args, &mut avisos)?),
        PROBLEMA => Llamada::Problema(crate::problema::validar(&args, &mut avisos)?),
        FIGURAS => Llamada::Figuras(crate::figuras::validar(&args)?),
        OCR => Llamada::Ocr(validar_ocr(&args)?),
        CALCAR => Llamada::Calcar(validar_calcar(&args)?),
        AYUDA => Llamada::Ayuda(
            texto_en(&args, "pregunta")
                .or_else(|| args.values().find_map(|v| v.as_str().map(str::to_string)))
                .ok_or_else(|| mal("falta «pregunta»"))?,
        ),
        crate::documentos::BUSCAR => Llamada::Buscar(crate::documentos::herramienta::validar(&args)?),
        otro if crate::edicion::fila(otro).is_some() => {
            Llamada::Editar(crate::edicion::validar(crate::edicion::fila(otro).unwrap(), &args)?)
        }
        otro if crate::acciones::por_nombre(otro).is_some() => {
            Llamada::App(crate::acciones::validar(crate::acciones::por_nombre(otro).unwrap(), &args)?)
        }
        // Una herramienta del catalogo que este crate no sabe validar (la
        // anadio el chat): no se ejecuta a ciegas.
        otro => return Err(ErrorHerramienta::Desconocida(otro.to_string())),
    };
    Ok(Validada { llamada, avisos })
}

/// Un numero: acepta tambien `"120"` (los modelos pequenos lo hacen).
fn numero(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().trim_end_matches("px").trim().parse().ok(),
        _ => None,
    }
    .filter(|n: &f64| n.is_finite())
}

fn numero_en(
    o: &Map<String, Value>,
    campo: &str,
    donde: &str,
    rango: (f64, f64),
) -> Result<Option<f64>, ErrorHerramienta> {
    let Some(v) = o.get(campo) else { return Ok(None) };
    if v.is_null() {
        return Ok(None);
    }
    let n = numero(v).ok_or_else(|| mal(format!("{donde}: «{campo}» tiene que ser un numero")))?;
    if n < rango.0 || n > rango.1 {
        return Err(mal(format!(
            "{donde}: «{campo}» = {n} esta fuera de rango ({} a {})",
            rango.0, rango.1
        )));
    }
    Ok(Some(n))
}

fn texto_en(o: &Map<String, Value>, campo: &str) -> Option<String> {
    match o.get(campo)? {
        Value::String(s) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

const COORD: (f64, f64) = (COORDENADA_MINIMA, COORDENADA_MAXIMA);
const MEDIDA: (f64, f64) = (1.0, COORDENADA_MAXIMA);

pub(crate) fn validar_dibujo(args: &Map<String, Value>, avisos: &mut Vec<String>) -> Result<Vec<Figura>, ErrorHerramienta> {
    let lista = match args.get("elementos") {
        Some(Value::Array(l)) => l,
        // Un solo elemento suelto, sin la lista: se acepta.
        Some(Value::Object(_)) => std::slice::from_ref(&args["elementos"]),
        _ => return Err(mal("falta «elementos» (una lista de figuras)")),
    };
    if lista.is_empty() {
        return Err(mal("«elementos» esta vacia: pon al menos una figura"));
    }
    if lista.len() > MAX_ELEMENTOS {
        return Err(mal(format!(
            "demasiadas figuras ({}); el maximo es {MAX_ELEMENTOS}",
            lista.len()
        )));
    }
    // `origen`: coordenadas relativas, para dibujar una cosa compuesta
    // pensando desde (0, 0) y ponerla luego donde va.
    let origen = match args.get("origen") {
        None | Some(Value::Null) => (0.0, 0.0),
        Some(v) => match numeros(v).as_deref() {
            Some([x, y]) => (*x, *y),
            _ => return Err(mal("«origen» tiene que ser [x, y]")),
        },
    };
    lista
        .iter()
        .enumerate()
        .map(|(i, v)| validar_figura(i, v, origen, avisos))
        .collect()
}

/// Una lista de numeros: plana (`[1,2,3,4]`) o de pares (`[[1,2],[3,4]]`).
fn numeros(v: &Value) -> Option<Vec<f64>> {
    let Value::Array(l) = v else { return None };
    let mut out = Vec::with_capacity(l.len() * 2);
    for e in l {
        match e {
            Value::Array(par) => {
                for n in par {
                    out.push(numero(n)?);
                }
            }
            otro => out.push(numero(otro)?),
        }
    }
    Some(out)
}

fn validar_figura(
    i: usize,
    v: &Value,
    (ox, oy): (f64, f64),
    avisos: &mut Vec<String>,
) -> Result<Figura, ErrorHerramienta> {
    let donde = format!("elemento {}", i + 1);
    let Value::Object(o) = v else {
        return Err(mal(format!("{donde}: tiene que ser un objeto")));
    };
    let tipo_txt = texto_en(o, "tipo").ok_or_else(|| mal(format!("{donde}: falta «tipo»")))?;
    // Formas que se generan de su caja (al modelo le cuesta poner sus puntos).
    let mut forma: Option<&str> = None;
    let tipo = match tipo_txt.to_lowercase().as_str() {
        "rectangulo" | "rectángulo" | "rect" | "rectangle" | "cuadrado" | "square" => TipoFigura::Rectangulo,
        "elipse" | "ellipse" | "circulo" | "círculo" | "circle" | "ovalo" | "óvalo" => TipoFigura::Elipse,
        "rombo" | "diamond" => TipoFigura::Rombo,
        "linea" | "línea" | "line" => TipoFigura::Linea,
        "flecha" | "arrow" => TipoFigura::Flecha,
        "texto" | "text" => TipoFigura::Texto,
        "curva" | "curve" | "spline" => TipoFigura::Curva,
        "onda" | "wave" | "ondas" => {
            forma = Some("onda");
            TipoFigura::Curva
        }
        "poligono" | "polígono" | "polygon" => TipoFigura::Poligono,
        "lapiz" | "lápiz" | "pencil" | "freedraw" | "mano_alzada" | "trazo" | "grafito" | "boceto" => TipoFigura::Lapiz,
        "punto" | "point" => TipoFigura::Punto,
        "tabla" | "table" | "cuadro" => TipoFigura::Tabla,
        "instrumento" | "plano" | "plano_cartesiano" | "ejes" | "axes" | "recta" | "recta_numerica" | "espacio" => {
            TipoFigura::Instrumento
        }
        "triangulo" | "triángulo" | "triangle" => {
            forma = Some("triangulo");
            TipoFigura::Poligono
        }
        "estrella" | "star" => {
            forma = Some("estrella");
            TipoFigura::Poligono
        }
        "paralelogramo" | "parallelogram" | "romboide" | "entrada" | "salida" => {
            forma = Some("paralelogramo");
            TipoFigura::Poligono
        }
        otro => {
            return Err(mal(format!(
                "{donde}: «tipo» = «{otro}» no existe; usa rectangulo, elipse, rombo, triangulo, estrella, linea, flecha, curva, poligono, lapiz, texto, punto o instrumento"
            )));
        }
    };
    // La caja: «caja», o «centro» y «radio», o x, y, ancho, alto.
    let caja: Option<[f64; 4]> = match o.get("caja").filter(|v| !v.is_null()) {
        Some(v) => match numeros(v).as_deref() {
            Some([x, y, w, h]) => Some([*x, *y, *w, *h]),
            _ => return Err(mal(format!("{donde}: «caja» tiene que ser [x, y, ancho, alto]"))),
        },
        None => match o.get("centro").filter(|v| !v.is_null()) {
            Some(v) => match numeros(v).as_deref() {
                Some([cx, cy]) => {
                    let r = numero_en(o, "radio", &donde, MEDIDA)?.unwrap_or(50.0);
                    Some([cx - r, cy - r, 2.0 * r, 2.0 * r])
                }
                _ => return Err(mal(format!("{donde}: «centro» tiene que ser [x, y]"))),
            },
            None => None,
        },
    };
    if let Some([x, y, w, h]) = caja {
        for (n, campo, rango) in [(x, "x", COORD), (y, "y", COORD), (w, "ancho", MEDIDA), (h, "alto", MEDIDA)] {
            if !(rango.0..=rango.1).contains(&n) {
                return Err(mal(format!(
                    "{donde}: «caja» con {campo} = {n} fuera de rango ({} a {})",
                    rango.0, rango.1
                )));
            }
        }
    }
    // Los puntos de lineas, curvas y poligonos.
    let mut puntos: Vec<(f64, f64)> = match o.get("puntos").filter(|v| !v.is_null()) {
        Some(v) => {
            let n = numeros(v).ok_or_else(|| mal(format!("{donde}: «puntos» tiene que ser una lista de numeros")))?;
            if n.len() % 2 != 0 {
                return Err(mal(format!(
                    "{donde}: «puntos» tiene un numero impar de valores (van de dos en dos: x, y)"
                )));
            }
            if n.len() > MAX_NUMEROS_PUNTOS {
                return Err(mal(format!("{donde}: demasiados puntos (maximo {})", MAX_NUMEROS_PUNTOS / 2)));
            }
            if let Some(m) = n.iter().find(|m| !(COORD.0..=COORD.1).contains(*m)) {
                return Err(mal(format!("{donde}: un punto ({m}) esta fuera de rango")));
            }
            n.chunks(2).map(|c| (c[0], c[1])).collect()
        }
        None => Vec::new(),
    };
    let es_trazo = matches!(
        tipo,
        TipoFigura::Linea | TipoFigura::Flecha | TipoFigura::Curva | TipoFigura::Poligono | TipoFigura::Lapiz
    );
    // En una linea el ancho/alto son el desplazamiento y pueden ser negativos.
    let rango_medida = if es_trazo { (-COORDENADA_MAXIMA, COORDENADA_MAXIMA) } else { MEDIDA };
    let x_ = numero_en(o, "x", &donde, COORD)?;
    let y_ = numero_en(o, "y", &donde, COORD)?;
    let ancho_ = numero_en(o, "ancho", &donde, rango_medida)?;
    let alto_ = numero_en(o, "alto", &donde, rango_medida)?;
    let x2_ = numero_en(o, "x2", &donde, COORD)?;
    let y2_ = numero_en(o, "y2", &donde, COORD)?;
    let tam_letra = numero_en(o, "tam_letra", &donde, (6.0, 200.0))?.unwrap_or(24.0);
    // El texto tambien con otro nombre («text», «etiqueta», «contenido»...).
    let texto = if tipo == TipoFigura::Tabla {
        // La tabla viaja en el texto: celdas con tabulador, filas con salto.
        filas_de_tabla(o).ok_or_else(|| mal(format!("{donde}: la tabla necesita «filas»: [[\"Nombre\",\"UA\"],[\"Mercurio\",\"0.39\"]]")))?
    } else {
        ["texto", "text", "etiqueta", "rotulo", "contenido", "label", "titulo", "valor"]
            .iter()
            .find_map(|c| texto_en(o, c))
            .unwrap_or_default()
    };
    // Un texto sin texto se acepta aqui: `colores_del_pedido` lo saca de lo
    // que el usuario puso entre comillas («Plan 2026»), que es lo que el 2B
    // olvida (medido 13-oct: tres reintentos seguidos sin el campo). Si no
    // hay de donde sacarlo, el lienzo lo descarta.
    if texto.chars().count() > if tipo == TipoFigura::Tabla { MAX_TEXTO * 8 } else { MAX_TEXTO } {
        return Err(mal(format!("{donde}: el texto pasa de {MAX_TEXTO} caracteres")));
    }
    // Una forma con «puntos» de menos (el 2B pone [x, y] sueltos): se ignoran
    // y vale la caja. En una linea, el (x, y) es su primer punto.
    if es_trazo && !puntos.is_empty() {
        let minimo = if tipo == TipoFigura::Poligono { 3 } else { 2 };
        if puntos.len() < minimo {
            if forma.is_some() {
                puntos.clear();
            } else if let (Some(x), Some(y)) = (x_, y_) {
                puntos.insert(0, (x, y));
            }
        }
    }
    // Sin sitio ninguno: en el centro, a un tamano normal. Medido: «dibuja
    // un triangulo verde en el centro» llegaba como {"tipo":"triangulo",
    // "color":"verde"} tres veces seguidas; mejor dibujarlo que rendirse.
    let sin_sitio = caja.is_none() && x_.is_none() && y_.is_none() && puntos.is_empty();
    let caja = match (caja, forma) {
        (None, Some("onda")) if sin_sitio => Some([100.0, 220.0, 600.0, 160.0]),
        (None, _) if sin_sitio && tipo == TipoFigura::Instrumento => Some([240.0, 140.0, 320.0, 320.0]),
        (None, _) if sin_sitio && tipo == TipoFigura::Punto => Some([400.0, 300.0, 1.0, 1.0]),
        (None, _) if sin_sitio && !matches!(tipo, TipoFigura::Texto | TipoFigura::Tabla) => Some([250.0, 150.0, 300.0, 300.0]),
        (c, _) => c,
    };
    if es_trazo && puntos.is_empty() {
        if let (Some(forma), Some(c)) = (forma, caja) {
            puntos = forma_de_caja(forma, c);
        } else if let (Some(forma), Some(x), Some(y)) = (forma, x_, y_) {
            let a = ancho_.unwrap_or(100.0).abs().max(1.0);
            puntos = forma_de_caja(forma, [x, y, a, alto_.unwrap_or(a).abs().max(1.0)]);
        } else if let (TipoFigura::Curva, Some(c)) = (tipo, caja) {
            puntos = forma_de_caja("onda", c);
        } else if matches!(tipo, TipoFigura::Linea | TipoFigura::Flecha) {
            if let Some([x, y, w, h]) = caja.filter(|_| x_.is_none()) {
                puntos = vec![(x, y + h / 2.0), (x + w, y + h / 2.0)];
            } else {
                let x = x_.ok_or_else(|| mal(format!("{donde}: falta «puntos» (o «x», «y», «x2», «y2»)")))?;
                let y = y_.ok_or_else(|| mal(format!("{donde}: falta «y»")))?;
                let x2 = x2_.or(ancho_.map(|a| x + a)).unwrap_or(x + 100.0);
                let y2 = y2_.or(alto_.map(|a| y + a)).unwrap_or(y);
                puntos = vec![(x, y), (x2, y2)];
            }
        } else {
            return Err(mal(format!("{donde}: falta «puntos»: [x1, y1, x2, y2, ...]")));
        }
    }
    let minimo = match tipo {
        TipoFigura::Poligono => 3,
        TipoFigura::Linea | TipoFigura::Flecha | TipoFigura::Curva | TipoFigura::Lapiz => 2,
        _ => 0,
    };
    if puntos.len() < minimo {
        return Err(mal(format!("{donde}: hacen falta al menos {minimo} puntos")));
    }
    if es_trazo && puntos.windows(2).all(|p| p[0] == p[1]) {
        return Err(mal(format!("{donde}: la linea empieza y acaba en el mismo punto")));
    }
    for p in &mut puntos {
        p.0 += ox;
        p.1 += oy;
    }
    let (x, y, ancho, alto, x2, y2) = if es_trazo {
        let (x, y) = puntos[0];
        let (x2, y2) = puntos[puntos.len() - 1];
        (x, y, x2 - x, y2 - y, x2, y2)
    } else {
        let [x, y, a, h] = match caja {
            Some(c) => c,
            None => {
                let x = match x_ {
                    Some(x) => x,
                    None if matches!(tipo, TipoFigura::Texto | TipoFigura::Tabla) => 60.0,
                    None => return Err(mal(format!("{donde}: falta «x» (o «caja»)"))),
                };
                let y = match y_ {
                    Some(y) => y,
                    None if matches!(tipo, TipoFigura::Texto | TipoFigura::Tabla) => 40.0,
                    None => return Err(mal(format!("{donde}: falta «y» (o «caja»)"))),
                };
                let (a, h) = if tipo == TipoFigura::Texto {
                    (
                        ancho_.unwrap_or(tam_letra * 0.6 * texto.chars().count() as f64),
                        alto_.unwrap_or(tam_letra * 1.25),
                    )
                } else {
                    let a = ancho_.unwrap_or(100.0);
                    (a, alto_.unwrap_or(a))
                };
                [x, y, a, h]
            }
        };
        let (x, y) = (x + ox, y + oy);
        (x, y, a, h, x + a, y + h)
    };
    // Los modelos pequenos a veces escriben el color con otro nombre
    // («fill», «stroke», «fondo»): se aceptan los sinonimos.
    let color_txt = ["color", "colour", "stroke", "trazo"].iter().find_map(|c| texto_en(o, c));
    let relleno_txt = ["relleno", "fill", "fondo", "background"].iter().find_map(|c| texto_en(o, c));
    let con_color = color_txt.is_some() || relleno_txt.is_some();
    let trazo = match color_txt {
        // Sin color pero con relleno: el borde del mismo color (un «circulo
        // rojo» lleno no debe salir con el borde negro).
        None => relleno_txt
            .as_deref()
            .and_then(color)
            .filter(|c| c.0 != "#ffffff")
            .unwrap_or_else(|| Color("#1e1e1e".into())),
        Some(c) => color(&c).unwrap_or_else(|| {
            avisos.push(format!("{donde}: color «{c}» desconocido, se usa negro"));
            Color("#1e1e1e".into())
        }),
    };
    let relleno = match relleno_txt {
        None => None,
        Some(c) if matches!(c.to_lowercase().as_str(), "ninguno" | "none" | "transparent" | "transparente") => None,
        Some(c) => match color(&c) {
            Some(col) => Some(col),
            None => {
                avisos.push(format!("{donde}: relleno «{c}» desconocido, se deja sin relleno"));
                None
            }
        },
    };
    let grupo = texto_en(o, "grupo").map(|g| g.chars().take(40).collect());
    let estilo = validar_estilo(o, &donde, tipo, avisos)?;
    let capa = numero_en(o, "capa", &donde, (-1000.0, 1000.0))?.unwrap_or(0.0) as i32;
    let instrumento = if tipo == TipoFigura::Instrumento {
        Some(validar_instrumento(o, &donde, &tipo_txt)?)
    } else {
        None
    };
    Ok(Figura {
        tipo,
        x,
        y,
        ancho,
        alto,
        x2,
        y2,
        color: trazo,
        relleno,
        texto,
        tam_letra,
        con_color,
        puntos: if es_trazo { puntos } else { Vec::new() },
        grupo,
        estilo,
        capa,
        instrumento,
    })
}

/// El estilo de una figura: lo que no se entiende se queda en lo de
/// siempre con un aviso (es cosmetico), salvo los numeros fuera de rango.
fn validar_estilo(
    o: &Map<String, Value>,
    donde: &str,
    tipo: TipoFigura,
    avisos: &mut Vec<String>,
) -> Result<EstiloFigura, ErrorHerramienta> {
    let palabra = |campo: &str| texto_en(o, campo).map(|t| crate::acciones::normalizar(&t));
    let trazo = palabra("trazo").and_then(|t| match t.as_str() {
        "solido" | "solid" | "continuo" | "normal" => Some("solid"),
        "discontinuo" | "dashed" | "rayas" | "guiones" => Some("dashed"),
        "punteado" | "dotted" | "puntos" => Some("dotted"),
        otro => {
            avisos.push(format!("{donde}: trazo «{otro}» desconocido, se usa solido"));
            None
        }
    });
    let relleno = palabra("relleno_estilo").and_then(|t| match t.as_str() {
        "solido" | "solid" | "liso" | "lleno" => Some("solid"),
        "rayado" | "hachure" | "rayas" | "achurado" => Some("hachure"),
        "cruzado" | "cross-hatch" | "cuadriculado" => Some("cross-hatch"),
        otro => {
            avisos.push(format!("{donde}: relleno_estilo «{otro}» desconocido, se usa solido"));
            None
        }
    });
    let mut material = palabra("material").and_then(|t| match t.as_str() {
        "lisa" | "liso" | "normal" | "tinta" => None,
        "tiza" | "chalk" => Some("tiza"),
        "grafito" | "cuadritos" | "lapiz" | "graphite" => Some("cuadritos"),
        "lapiz2b" | "2b" | "carboncillo" => Some("lapiz2b"),
        "seco" | "rotulador" | "marcador" => Some("seco"),
        otro => {
            avisos.push(format!("{donde}: material «{otro}» desconocido, se usa liso"));
            None
        }
    });
    let dureza = texto_en(o, "dureza").and_then(|d| match dureza(&d) {
        Some(d) => Some(d),
        None => {
            avisos.push(format!("{donde}: dureza «{d}» desconocida, se usa HB"));
            None
        }
    });
    // «tipo: grafito» o una dureza sin material: es grafito.
    if tipo == TipoFigura::Lapiz && material.is_none() && dureza.is_some() {
        material = Some("cuadritos");
    }
    let opacidad = match numero_en(o, "opacidad", donde, (0.0, 100.0))? {
        // 0.5 es la mitad, no medio por ciento.
        Some(v) if v > 0.0 && v <= 1.0 => Some(v * 100.0),
        otro => otro,
    };
    Ok(EstiloFigura {
        grosor: numero_en(o, "grosor", donde, (0.5, 40.0))?,
        trazo,
        opacidad,
        relleno,
        rugosidad: numero_en(o, "rugosidad", donde, (0.0, 3.0))?,
        material,
        dureza,
    })
}

/// La dureza de un grafito, a una de las siete del lienzo: «6B», «hb»,
/// «3H» (la mas cercana).
pub fn dureza(texto: &str) -> Option<&'static str> {
    let t = texto.trim().to_uppercase();
    match t.as_str() {
        "HB" | "F" => return Some("HB"),
        "H" => return Some("2H"),
        "B" => return Some("2B"),
        _ => {}
    }
    let (n, letra) = t.split_at(t.len().checked_sub(1)?);
    let n: u32 = n.parse().ok()?;
    match (letra, n) {
        ("H", 1..=2) => Some("2H"),
        ("H", 3..=9) => Some("4H"),
        ("B", 1..=2) => Some("2B"),
        ("B", 3..=4) => Some("4B"),
        ("B", 5..=6) => Some("6B"),
        ("B", 7..=9) => Some("8B"),
        _ => None,
    }
}

/// Los ejes, la unidad y la tabla de un instrumento.
fn validar_instrumento(o: &Map<String, Value>, donde: &str, tipo_txt: &str) -> Result<PedidoInstrumento, ErrorHerramienta> {
    let por_nombre = match crate::acciones::normalizar(tipo_txt).as_str() {
        "recta" | "recta_numerica" => Some(1.0),
        "espacio" => Some(3.0),
        _ => None,
    };
    let ejes = match numero_en(o, "ejes", donde, (1.0, 3.0))?.or(por_nombre) {
        Some(n) => n.round() as u8,
        None => 2,
    };
    let unidad = numero_en(o, "unidad", donde, (5.0, 400.0))?.unwrap_or(40.0);
    let tabla = match o.get("tabla").filter(|v| !v.is_null()) {
        None => Vec::new(),
        Some(v) => {
            let n = numeros(v).ok_or_else(|| mal(format!("{donde}: «tabla» tiene que ser [[x, y], ...]")))?;
            if n.len() % 2 != 0 {
                return Err(mal(format!("{donde}: «tabla» tiene un numero impar de valores (van de dos en dos: x, y)")));
            }
            if n.len() / 2 > MAX_TABLA {
                return Err(mal(format!("{donde}: la tabla pasa de {MAX_TABLA} puntos")));
            }
            if n.iter().any(|v| v.abs() > 1e6) {
                return Err(mal(format!("{donde}: un valor de la tabla es enorme")));
            }
            n.chunks(2).map(|c| (c[0], c[1])).collect()
        }
    };
    Ok(PedidoInstrumento { ejes, unidad, tabla })
}

/// Los vertices de una forma dentro de su caja.
fn forma_de_caja(forma: &str, [x, y, w, h]: [f64; 4]) -> Vec<(f64, f64)> {
    match forma {
        "estrella" => {
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            (0..10)
                .map(|k| {
                    let a = -std::f64::consts::FRAC_PI_2 + k as f64 * std::f64::consts::PI / 5.0;
                    let r = if k % 2 == 0 { 1.0 } else { 0.4 };
                    (cx + r * w / 2.0 * a.cos(), cy + r * h / 2.0 * a.sin())
                })
                .collect()
        }
        // Dos ondas de seno a lo ancho de la caja.
        "onda" => (0..=16)
            .map(|k| {
                let t = k as f64 / 16.0;
                (x + t * w, y + h / 2.0 - h / 2.0 * (t * 4.0 * std::f64::consts::PI).sin())
            })
            .collect(),
        // Paralelogramo (entrada/salida de un diagrama de flujo: «Leer N»).
        "paralelogramo" => {
            let s = (w * 0.18).min(h * 0.8);
            vec![(x + s, y), (x + w, y), (x + w - s, y + h), (x, y + h)]
        }
        // Triangulo con la punta arriba.
        _ => vec![(x, y + h), (x + w / 2.0, y), (x + w, y + h)],
    }
}

fn validar_calcar(args: &Map<String, Value>) -> Result<crate::calcar::Parametros, ErrorHerramienta> {
    use crate::calcar::{Detalle, Parametros};
    let detalle = match texto_en(args, "detalle").map(|t| crate::acciones::normalizar(&t)).as_deref() {
        None | Some("medio" | "normal" | "medium") => Detalle::Medio,
        Some("bajo" | "poco" | "low" | "simple") => Detalle::Bajo,
        Some("alto" | "mucho" | "high" | "maximo") => Detalle::Alto,
        Some(otro) => return Err(mal(format!("«detalle» = «{otro}» no existe; usa bajo, medio o alto"))),
    };
    let si_no = |campo: &str, defecto: bool| -> Result<bool, ErrorHerramienta> {
        match args.get(campo) {
            None | Some(Value::Null) => Ok(defecto),
            Some(Value::Bool(b)) => Ok(*b),
            Some(Value::String(s)) => match crate::acciones::normalizar(s.trim()).as_str() {
                "true" | "si" => Ok(true),
                "false" | "no" => Ok(false),
                _ => Err(mal(format!("«{campo}» tiene que ser true o false"))),
            },
            _ => Err(mal(format!("«{campo}» tiene que ser true o false"))),
        }
    };
    let zona = match args.get("zona").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => match numeros(v).as_deref() {
            Some([x, y, w, h])
                if [*x, *y, *w, *h].iter().all(|n| (0.0..=1.0).contains(n)) && *w > 0.0 && *h > 0.0 =>
            {
                Some([*x, *y, *w, *h])
            }
            _ => return Err(mal("«zona» tiene que ser [x, y, ancho, alto] con fracciones de 0 a 1")),
        },
    };
    Ok(Parametros {
        detalle,
        colores: si_no("colores", true)?,
        solo_contornos: si_no("solo_contornos", false)?,
        zona,
        imagen: texto_en(args, "imagen"),
    })
}

/// **Los colores que el modelo olvido, sacados del pedido.**
///
/// Medido con el 2B (13-oct): a «un circulo rojo y un cuadrado azul»
/// contesta con las dos figuras bien puestas pero sin `color`, aunque la
/// descripcion se lo pida. Si el pedido dice «<figura> <color>», a cada
/// figura sin color se le pone el de su tipo, en el orden en que se
/// nombraron (y lleno, que es lo que se entiende por «un circulo rojo»).
/// Solo toca figuras sin color: nunca corrige lo que el modelo si puso.
pub fn colores_del_pedido(figuras: &mut [Figura], pedido: &str) {
    let palabras: Vec<String> = pedido
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '#')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    let tipo_de = |p: &str| -> Option<TipoFigura> {
        Some(match p {
            "circulo" | "círculo" | "circulos" | "círculos" | "elipse" | "ovalo" | "óvalo" | "sol" | "pelota" | "bola"
            | "circle" | "ellipse" => TipoFigura::Elipse,
            "cuadrado" | "cuadrados" | "rectangulo" | "rectángulo" | "caja" | "square" | "rectangle" => {
                TipoFigura::Rectangulo
            }
            "rombo" | "diamante" | "diamond" => TipoFigura::Rombo,
            "triangulo" | "triángulo" | "estrella" | "poligono" | "polígono" | "techo" | "tejado" | "triangle" | "star" => {
                TipoFigura::Poligono
            }
            "curva" | "onda" | "curve" | "wave" => TipoFigura::Curva,
            "linea" | "línea" | "raya" | "line" => TipoFigura::Linea,
            "flecha" | "arrow" => TipoFigura::Flecha,
            "titulo" | "título" | "texto" | "letrero" | "title" | "text" => TipoFigura::Texto,
            _ => return None,
        })
    };
    // (tipo, color) en el orden del pedido: el color en las 3 palabras
    // siguientes («circulo grande rojo», «cuadrado de color azul»).
    let mut pares: Vec<(TipoFigura, Color)> = Vec::new();
    for (i, p) in palabras.iter().enumerate() {
        let Some(t) = tipo_de(p) else { continue };
        if let Some(c) = palabras[i + 1..]
            .iter()
            .take(4)
            .take_while(|q| tipo_de(q).is_none())
            .find_map(|q| color(q))
        {
            pares.push((t, c));
        }
    }
    for f in figuras.iter_mut().filter(|f| !f.con_color) {
        let Some(i) = pares.iter().position(|(t, _)| *t == f.tipo) else { continue };
        let (_, c) = pares.remove(i);
        if matches!(f.tipo, TipoFigura::Rectangulo | TipoFigura::Elipse | TipoFigura::Rombo | TipoFigura::Poligono) {
            f.relleno = Some(c.clone());
        }
        f.color = c;
        f.con_color = true;
    }
    // Los textos que el modelo dejo vacios, con lo que el usuario puso entre
    // comillas, en orden.
    let mut citas = citas_del_pedido(pedido).into_iter();
    for f in figuras.iter_mut().filter(|f| f.tipo == TipoFigura::Texto && f.texto.is_empty()) {
        match citas.next() {
            Some(c) => f.texto = c,
            None => break,
        }
    }
}

/// Lo que va entre comillas en el pedido: «…», "…", “…” o '…'.
pub fn citas_del_pedido(pedido: &str) -> Vec<String> {
    let mut salida = Vec::new();
    let car: Vec<char> = pedido.chars().collect();
    let mut i = 0;
    while i < car.len() {
        let cierre = match car[i] {
            '«' => Some('»'),
            '"' => Some('"'),
            '“' => Some('”'),
            '\'' => Some('\''),
            _ => None,
        };
        if let Some(cierre) = cierre
            && let Some(fin) = car[i + 1..].iter().position(|c| *c == cierre)
        {
            let dentro: String = car[i + 1..i + 1 + fin].iter().collect();
            if !dentro.trim().is_empty() && dentro.chars().count() <= MAX_TEXTO {
                salida.push(dentro.trim().to_string());
            }
            i += fin + 2;
            continue;
        }
        i += 1;
    }
    salida
}

/// `#rgb`, `#rrggbb` o un nombre en espanol o ingles. Los nombres caen en la
/// paleta de Excalidraw, la misma del lienzo.
pub fn color(texto: &str) -> Option<Color> {
    let t = texto.trim().to_lowercase();
    if let Some(h) = t.strip_prefix('#') {
        if h.chars().all(|c| c.is_ascii_hexdigit()) {
            return match h.len() {
                6 => Some(Color(format!("#{h}"))),
                8 => Some(Color(format!("#{}", &h[..6]))),
                3 => {
                    let mut s = String::from("#");
                    for c in h.chars() {
                        s.push(c);
                        s.push(c);
                    }
                    Some(Color(s))
                }
                _ => None,
            };
        }
        return None;
    }
    // «roja», «amarillas», «azules»: el femenino y el plural del pedido.
    let mut candidatos = vec![t.clone()];
    for sufijo in ["es", "s"] {
        if let Some(r) = t.strip_suffix(sufijo) {
            candidatos.push(r.to_string());
        }
    }
    for c in candidatos.clone() {
        if let Some(r) = c.strip_suffix('a') {
            candidatos.push(format!("{r}o"));
        }
    }
    candidatos.into_iter().find_map(|c| color_por_nombre(&c)).map(|h| Color(h.into()))
}

fn color_por_nombre(t: &str) -> Option<&'static str> {
    let hex = match t {
        "negro" | "black" => "#1e1e1e",
        "blanco" | "white" => "#ffffff",
        "rojo" | "red" => "#e03131",
        "verde" | "green" => "#2f9e44",
        "azul" | "blue" => "#1971c2",
        "amarillo" | "yellow" => "#f08c00",
        "naranja" | "orange" => "#e8590c",
        "morado" | "violeta" | "purple" | "violet" => "#6741d9",
        "rosa" | "pink" => "#c2255c",
        "gris" | "gray" | "grey" => "#868e96",
        "marron" | "marrón" | "cafe" | "café" | "brown" => "#8b5a2b",
        "celeste" | "cyan" | "cian" => "#0c8599",
        _ => return None,
    };
    Some(hex)
}

fn validar_grafica(args: &Map<String, Value>, avisos: &mut Vec<String>) -> Result<Grafica, ErrorHerramienta> {
    let tipo = match texto_en(args, "tipo").map(|t| t.to_lowercase()).as_deref() {
        Some("barras" | "barra" | "bar" | "columnas") => TipoGrafica::Barras,
        Some("lineas" | "líneas" | "linea" | "línea" | "line") => TipoGrafica::Lineas,
        Some("pastel" | "torta" | "pie" | "circular") => TipoGrafica::Pastel,
        Some(otro) => return Err(mal(format!("«tipo» = «{otro}» no existe; usa barras, lineas o pastel"))),
        None => return Err(mal("falta «tipo» (barras, lineas o pastel)")),
    };
    let valores: Vec<f64> = match args.get("valores") {
        Some(Value::Array(l)) => l
            .iter()
            .enumerate()
            .map(|(i, v)| {
                numero(v)
                    .filter(|n| n.abs() < 1e12)
                    .ok_or_else(|| mal(format!("«valores»[{}] no es un numero valido", i + 1)))
            })
            .collect::<Result<_, _>>()?,
        _ => return Err(mal("falta «valores» (una lista de numeros)")),
    };
    if valores.is_empty() || valores.len() > MAX_DATOS_GRAFICA {
        return Err(mal(format!(
            "«valores» tiene {} datos; tiene que tener de 1 a {MAX_DATOS_GRAFICA}",
            valores.len()
        )));
    }
    let etiquetas: Vec<String> = match args.get("etiquetas") {
        Some(Value::Array(l)) => l
            .iter()
            .map(|v| match v {
                Value::String(s) => s.trim().chars().take(60).collect(),
                otro => otro.to_string(),
            })
            .collect(),
        None | Some(Value::Null) => {
            avisos.push("sin «etiquetas»: se numeran".into());
            (1..=valores.len()).map(|i| i.to_string()).collect()
        }
        _ => return Err(mal("«etiquetas» tiene que ser una lista de textos")),
    };
    if etiquetas.len() != valores.len() {
        return Err(mal(format!(
            "hay {} etiquetas y {} valores; tiene que haber el mismo numero",
            etiquetas.len(),
            valores.len()
        )));
    }
    if tipo == TipoGrafica::Pastel {
        if valores.iter().any(|v| *v < 0.0) {
            return Err(mal("un pastel no admite valores negativos"));
        }
        if valores.iter().sum::<f64>() <= 0.0 {
            return Err(mal("un pastel necesita algun valor mayor que cero"));
        }
    }
    let titulo = texto_en(args, "titulo").unwrap_or_default().chars().take(120).collect();
    Ok(Grafica {
        tipo,
        titulo,
        etiquetas,
        valores,
    })
}

fn validar_ocr(args: &Map<String, Value>) -> Result<PeticionOcr, ErrorHerramienta> {
    let archivo = texto_en(args, "archivo");
    let paginas = match texto_en(args, "paginas") {
        None => Vec::new(),
        Some(p) => leer_paginas(&p)?,
    };
    let idioma = match texto_en(args, "idioma").map(|i| i.to_lowercase()).as_deref() {
        None | Some("auto") => None,
        Some(i @ ("es" | "en")) => Some(i.to_string()),
        Some("espanol" | "español" | "spanish") => Some("es".into()),
        Some("ingles" | "inglés" | "english") => Some("en".into()),
        Some(otro) => return Err(mal(format!("«idioma» = «{otro}» no existe; usa es, en o auto"))),
    };
    Ok(PeticionOcr {
        archivo,
        paginas,
        idioma,
    })
}

/// `"1-3,5"` → `[(1,3),(5,5)]`. Paginas de 1 a 10 000.
pub fn leer_paginas(texto: &str) -> Result<Vec<(u32, u32)>, ErrorHerramienta> {
    let t = texto.trim().to_lowercase();
    if t.is_empty() || t == "todas" || t == "all" {
        return Ok(Vec::new());
    }
    let mut rangos = Vec::new();
    for trozo in t.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (a, b) = match trozo.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (trozo, trozo),
        };
        let a: u32 = a.parse().map_err(|_| mal(format!("«paginas»: «{trozo}» no se entiende")))?;
        let b: u32 = b.parse().map_err(|_| mal(format!("«paginas»: «{trozo}» no se entiende")))?;
        if a == 0 || b < a || b > 10_000 {
            return Err(mal(format!("«paginas»: «{trozo}» esta fuera de rango")));
        }
        rangos.push((a, b));
    }
    Ok(rangos)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cruda(nombre: &str, args: &str) -> LlamadaCruda {
        LlamadaCruda {
            id: "1".into(),
            nombre: nombre.into(),
            argumentos: args.into(),
        }
    }

    fn validar_(nombre: &str, args: &str) -> Result<Validada, ErrorHerramienta> {
        validar(&cruda(nombre, args), &catalogo())
    }

    #[test]
    fn la_llamada_medida_del_modelo_2b_pasa() {
        // Copiada de la medida real (Qwen3.5-2B, ver el documento).
        let v = validar_(
            DIBUJAR,
            r#"{"elementos":[{"y": 200, "x": 100, "tipo": "elipse", "alto": 150, "ancho": 150}, {"y": 250, "x": 600, "tipo": "rectangulo", "alto": 100, "ancho": 150}]}"#,
        )
        .unwrap();
        let Llamada::Dibujar(f) = v.llamada else { panic!() };
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].tipo, TipoFigura::Elipse);
        assert_eq!(f[1].x2, 750.0);
        assert!(v.avisos.is_empty());
    }

    #[test]
    fn los_colores_con_otro_nombre_tambien_valen() {
        let v = validar_(
            DIBUJAR,
            r#"{"elementos":[
                {"tipo":"elipse","x":1,"y":1,"fill":"red"},
                {"tipo":"rectangulo","x":1,"y":1,"stroke":"azul","fondo":"amarillo"},
                {"tipo":"rombo","x":1,"y":1,"relleno":"blanco"}]}"#,
        )
        .unwrap();
        let Llamada::Dibujar(f) = v.llamada else { panic!() };
        // Lleno de rojo y sin «color»: el borde, rojo tambien.
        assert_eq!(f[0].color.0, "#e03131");
        assert_eq!(f[0].relleno.as_ref().unwrap().0, "#e03131");
        assert_eq!(f[1].color.0, "#1971c2");
        assert_eq!(f[1].relleno.as_ref().unwrap().0, "#f08c00");
        // Un relleno blanco no deja el borde blanco (no se veria).
        assert_eq!(f[2].color.0, "#1e1e1e");
    }

    #[test]
    fn los_colores_olvidados_salen_del_pedido() {
        // La llamada real del 2B (13-oct): las figuras sin color.
        let v = validar_(
            DIBUJAR,
            r#"{"elementos":[{"tipo":"elipse","x":100,"y":150,"ancho":100,"alto":100},
                             {"tipo":"rectangulo","x":600,"y":150,"ancho":100,"alto":100},
                             {"tipo":"texto","x":300,"y":20,"texto":"CASA"},
                             {"tipo":"rectangulo","x":1,"y":1,"color":"verde"}]}"#,
        )
        .unwrap();
        let Llamada::Dibujar(mut f) = v.llamada else { panic!() };
        colores_del_pedido(
            &mut f,
            "Dibuja un circulo rojo a la izquierda y un cuadrado azul a la derecha, con el titulo CASA arriba.",
        );
        assert_eq!(f[0].color.0, "#e03131");
        assert_eq!(f[0].relleno.as_ref().unwrap().0, "#e03131");
        assert_eq!(f[1].color.0, "#1971c2");
        // Sin color en el pedido: negro, como estaba.
        assert_eq!(f[2].color.0, "#1e1e1e");
        // Lo que el modelo si puso no se toca.
        assert_eq!(f[3].color.0, "#2f9e44");
        // Un pedido sin colores no cambia nada.
        let antes = f.clone();
        colores_del_pedido(&mut f, "dibuja una casa");
        assert_eq!(f, antes);
    }

    #[test]
    fn la_descripcion_pide_el_color_siempre() {
        let d = &catalogo()[0];
        assert!(d.descripcion.contains("Cada elemento lleva su «color»"));
        assert!(d.parametros.to_string().contains("rojo, azul"));
    }

    #[test]
    fn un_texto_sin_texto_lo_toma_de_las_comillas_del_pedido() {
        // Lo que hizo el 0.8B en la medida, y el 2B con «Plan 2026» (13-oct).
        let mut f = figuras(r##"{"elementos":[{"y":150,"x":300,"tipo":"texto","color":"#ffffff"},{"tipo":"texto"}]}"##);
        colores_del_pedido(&mut f, "Escribe el titulo «Plan 2026» y debajo \"version 2\"");
        assert_eq!(f[0].texto, "Plan 2026");
        assert_eq!(f[1].texto, "version 2");
        assert_eq!((f[1].x, f[1].y), (60.0, 40.0));
        // Sin comillas se queda vacio y el lienzo no lo pinta.
        let mut f = figuras(r#"{"elementos":[{"tipo":"texto","x":1,"y":1}]}"#);
        colores_del_pedido(&mut f, "pon un titulo");
        assert!(f[0].texto.is_empty());
        assert!(crate::lienzo::elementos(&Llamada::Dibujar(f), (0.0, 0.0), 1).is_empty());
        assert_eq!(citas_del_pedido("a «b» c 'd' “e”"), vec!["b", "d", "e"]);
        assert!(citas_del_pedido("sin «cerrar").is_empty());
    }

    #[test]
    fn una_forma_sin_sitio_va_al_centro_y_los_puntos_de_menos_se_arreglan() {
        let f = figuras(r#"{"elementos":[{"tipo":"triangulo","color":"verde"},{"tipo":"estrella","puntos":[100,50],"x":200,"y":300},{"tipo":"curva","puntos":[0,50],"x":160,"y":300},{"tipo":"onda"}]}"#);
        assert_eq!(f[0].puntos, vec![(250.0, 450.0), (400.0, 150.0), (550.0, 450.0)]);
        assert_eq!(f[1].puntos.len(), 10);
        assert_eq!(f[2].puntos, vec![(160.0, 300.0), (0.0, 50.0)]);
        assert_eq!(f[3].puntos.len(), 17);
        // Un poligono sin nada sigue siendo un error (no hay forma que inventar).
        assert!(validar_(DIBUJAR, r#"{"elementos":[{"tipo":"poligono","puntos":[1,2]}]}"#).is_err());
    }

    #[test]
    fn herramienta_desconocida_dice_las_que_hay() {
        let e = validar_("borrar_disco", "{}").unwrap_err();
        assert_eq!(e, ErrorHerramienta::Desconocida("borrar_disco".into()));
        let m = e.para_el_modelo(&catalogo());
        assert!(m.contains("dibujar_en_lienzo") && m.contains("grafica") && m.contains("ocr"));
    }

    #[test]
    fn json_roto_sin_arreglo_se_rechaza() {
        assert_eq!(validar_(GRAFICA, r#"{"tipo": "barras", "valores": [1, 2"#).unwrap_err(), ErrorHerramienta::JsonRoto);
        assert_eq!(validar_(GRAFICA, "no es json").unwrap_err(), ErrorHerramienta::JsonRoto);
    }

    #[test]
    fn json_con_vallas_y_comas_colgantes_se_arregla() {
        let v = validar_(
            GRAFICA,
            "```json\n{\"tipo\": \"barras\", \"etiquetas\": [\"a\", \"b\",], \"valores\": [1, 2,],}\n```",
        )
        .unwrap();
        let Llamada::Grafica(g) = v.llamada else { panic!() };
        assert_eq!(g.valores, vec![1.0, 2.0]);
        // Una coma dentro de una cadena no se toca.
        let v = reparar_json(r#"{"t": "a, }", }"#).unwrap();
        assert_eq!(v["t"], "a, }");
    }

    #[test]
    fn coordenadas_fuera_de_rango_se_rechazan() {
        let e = validar_(DIBUJAR, r#"{"elementos":[{"tipo":"rectangulo","x":90000,"y":10}]}"#).unwrap_err();
        assert!(e.to_string().contains("fuera de rango"), "{e}");
        let e = validar_(DIBUJAR, r#"{"elementos":[{"tipo":"rectangulo","x":1,"y":1,"ancho":-5}]}"#).unwrap_err();
        assert!(e.to_string().contains("«ancho»"), "{e}");
        let e = validar_(DIBUJAR, r#"{"elementos":[{"tipo":"texto","x":1,"y":1,"texto":"a","tam_letra":900}]}"#).unwrap_err();
        assert!(e.to_string().contains("tam_letra"), "{e}");
    }

    #[test]
    fn demasiadas_figuras_o_ninguna_se_rechazan() {
        let una = r#"{"tipo":"elipse","x":1,"y":1}"#;
        let muchas = vec![una; MAX_ELEMENTOS + 1].join(",");
        assert!(validar_(DIBUJAR, &format!(r#"{{"elementos":[{muchas}]}}"#)).is_err());
        assert!(validar_(DIBUJAR, r#"{"elementos":[]}"#).is_err());
        assert!(validar_(DIBUJAR, r#"{}"#).is_err());
    }

    #[test]
    fn tipo_inventado_se_rechaza() {
        let e = validar_(DIBUJAR, r#"{"elementos":[{"tipo":"dragon","x":1,"y":1}]}"#).unwrap_err();
        assert!(e.to_string().contains("dragon"));
    }

    #[test]
    fn los_colores_raros_no_rompen_la_llamada() {
        let v = validar_(
            DIBUJAR,
            r##"{"elementos":[{"tipo":"rectangulo","x":"10","y":"20px","color":"azul","relleno":"#F00"},{"tipo":"elipse","x":1,"y":1,"color":"color-de-ilusion"}]}"##,
        )
        .unwrap();
        let Llamada::Dibujar(f) = v.llamada else { panic!() };
        assert_eq!(f[0].x, 10.0);
        assert_eq!(f[0].y, 20.0);
        assert_eq!(f[0].color, Color("#1971c2".into()));
        assert_eq!(f[0].relleno, Some(Color("#ff0000".into())));
        assert_eq!(f[1].color, Color("#1e1e1e".into()));
        assert_eq!(v.avisos.len(), 1);
    }

    #[test]
    fn una_linea_acepta_x2_y2_o_ancho_alto() {
        let v = validar_(DIBUJAR, r#"{"elementos":[{"tipo":"linea","x":10,"y":10,"x2":110,"y2":60},{"tipo":"flecha","x":0,"y":100,"ancho":-50,"alto":0}]}"#).unwrap();
        let Llamada::Dibujar(f) = v.llamada else { panic!() };
        assert_eq!((f[0].ancho, f[0].alto), (100.0, 50.0));
        assert_eq!((f[1].x2, f[1].y2), (-50.0, 100.0));
        assert!(validar_(DIBUJAR, r#"{"elementos":[{"tipo":"linea","x":5,"y":5,"x2":5,"y2":5}]}"#).is_err());
    }

    #[test]
    fn la_grafica_medida_pasa_y_la_descuadrada_no() {
        let v = validar_(GRAFICA, r#"{"tipo":"barras","etiquetas":["Enero", "Febrero", "Marzo"],"valores":[120, 95, 140]}"#).unwrap();
        let Llamada::Grafica(g) = v.llamada else { panic!() };
        assert_eq!(g.tipo, TipoGrafica::Barras);
        let e = validar_(GRAFICA, r#"{"tipo":"barras","etiquetas":["a"],"valores":[1,2]}"#).unwrap_err();
        assert!(e.to_string().contains("mismo numero"));
        assert!(validar_(GRAFICA, r#"{"tipo":"pastel","etiquetas":["a","b"],"valores":[-1,2]}"#).is_err());
        assert!(validar_(GRAFICA, r#"{"tipo":"radar","etiquetas":["a"],"valores":[1]}"#).is_err());
        assert!(validar_(GRAFICA, r#"{"tipo":"barras","etiquetas":["a"],"valores":["mucho"]}"#).is_err());
    }

    #[test]
    fn ocr_lee_paginas_e_idioma() {
        let v = validar_(OCR, r#"{"archivo":"tesis.pdf","paginas":"1-3, 5","idioma":"español"}"#).unwrap();
        assert_eq!(
            v.llamada,
            Llamada::Ocr(PeticionOcr {
                archivo: Some("tesis.pdf".into()),
                paginas: vec![(1, 3), (5, 5)],
                idioma: Some("es".into())
            })
        );
        // Sin argumentos: el ultimo adjunto, todas las paginas.
        assert!(validar_(OCR, "").is_ok());
        assert!(validar_(OCR, r#"{"paginas":"0-2"}"#).is_err());
        assert!(validar_(OCR, r#"{"paginas":"5-2"}"#).is_err());
        assert!(validar_(OCR, r#"{"paginas":"uno"}"#).is_err());
        assert!(validar_(OCR, r#"{"idioma":"klingon"}"#).is_err());
    }

    #[test]
    fn los_argumentos_que_no_son_objeto_se_rechazan() {
        assert!(matches!(validar_(GRAFICA, "[1,2,3]"), Err(ErrorHerramienta::Parametro(_)) | Err(ErrorHerramienta::JsonRoto)));
    }

    #[test]
    fn el_catalogo_es_json_de_herramientas_valido() {
        let v: Value = serde_json::from_str(&catalogo_json()).unwrap();
        let lista = v.as_array().unwrap();
        assert_eq!(lista.len(), 13 + crate::edicion::FILAS.len() + crate::acciones::todas().count());
        for h in lista {
            assert_eq!(h["type"], "function");
            assert_eq!(h["function"]["parameters"]["type"], "object");
        }
    }

    fn figuras(args: &str) -> Vec<Figura> {
        match validar_(DIBUJAR, args).unwrap().llamada {
            Llamada::Dibujar(f) => f,
            otra => panic!("{otra:?}"),
        }
    }

    #[test]
    fn figuras_ricas_con_coordenadas_compactas() {
        let f = figuras(
            r#"{"origen":[100,50],"elementos":[
                {"tipo":"rectangulo","caja":[0,0,200,100],"texto":"Inicio","grupo":"a"},
                {"tipo":"poligono","puntos":[0,0,50,0,25,40],"relleno":"rojo"},
                {"tipo":"curva","puntos":[[0,0],[10,20],[20,0],[30,20]],"color":"azul"},
                {"tipo":"triangulo","caja":[0,0,100,80]},
                {"tipo":"estrella","centro":[50,50],"radio":40},
                {"tipo":"elipse","centro":[50,50],"radio":20},
                {"tipo":"flecha","puntos":[0,0,100,0],"texto":"si"}]}"#,
        );
        assert_eq!((f[0].x, f[0].y, f[0].ancho, f[0].alto), (100.0, 50.0, 200.0, 100.0));
        assert_eq!(f[0].texto, "Inicio");
        assert_eq!(f[0].grupo.as_deref(), Some("a"));
        assert_eq!(f[1].tipo, TipoFigura::Poligono);
        assert_eq!(f[1].puntos, vec![(100.0, 50.0), (150.0, 50.0), (125.0, 90.0)]);
        assert_eq!(f[1].color.0, "#e03131");
        assert_eq!(f[2].tipo, TipoFigura::Curva);
        assert_eq!(f[2].puntos.len(), 4);
        assert_eq!(f[3].puntos, vec![(100.0, 130.0), (150.0, 50.0), (200.0, 130.0)]);
        assert_eq!(f[4].puntos.len(), 10);
        assert_eq!((f[5].x, f[5].ancho), (130.0, 40.0));
        assert_eq!(f[6].tipo, TipoFigura::Flecha);
        assert_eq!((f[6].x2, f[6].y2), (200.0, 50.0));
    }

    #[test]
    fn puntos_mal_puestos_se_rechazan() {
        for (args, motivo) in [
            (r#"{"elementos":[{"tipo":"poligono","puntos":[1,2,3]}]}"#, "impar"),
            (r#"{"elementos":[{"tipo":"poligono","puntos":[1,2,3,4]}]}"#, "al menos 3"),
            (r#"{"elementos":[{"tipo":"poligono"}]}"#, "falta «puntos»"),
            (r#"{"elementos":[{"tipo":"poligono","puntos":[1,2,3,4,99999,1]}]}"#, "fuera de rango"),
            (r#"{"elementos":[{"tipo":"rectangulo","caja":[1,2,3]}]}"#, "«caja»"),
            (r#"{"elementos":[{"tipo":"rectangulo","caja":[1,2,-3,4]}]}"#, "fuera de rango"),
            (r#"{"origen":"aqui","elementos":[{"tipo":"rectangulo","caja":[1,2,3,4]}]}"#, "«origen»"),
        ] {
            let e = validar_(DIBUJAR, args).unwrap_err().to_string();
            assert!(e.contains(motivo), "{args}: {e}");
        }
        let muchos: Vec<String> = (0..MAX_NUMEROS_PUNTOS + 2).map(|i| (i % 500).to_string()).collect();
        let e = validar_(DIBUJAR, &format!(r#"{{"elementos":[{{"tipo":"curva","puntos":[{}]}}]}}"#, muchos.join(",")))
            .unwrap_err();
        assert!(e.to_string().contains("demasiados"));
    }

    #[test]
    fn colores_en_femenino_y_plural() {
        assert_eq!(color("roja"), color("rojo"));
        assert_eq!(color("amarillas"), color("amarillo"));
        assert_eq!(color("azules"), color("azul"));
        assert_eq!(color("rosa").unwrap().0, "#c2255c");
        assert!(color("casa").is_none());
        let mut f = figuras(r#"{"elementos":[{"tipo":"estrella","caja":[1,1,50,50]}]}"#);
        colores_del_pedido(&mut f, "Dibuja una estrella de cinco puntas amarilla");
        assert_eq!(f[0].relleno, color("amarillo"));
    }

    #[test]
    fn lapiz_grafito_instrumento_y_estilos() {
        let f = figuras(
            r#"{"elementos":[
                {"tipo":"lapiz","puntos":[10,10,20,30,40,35],"material":"grafito","dureza":"6B","grosor":3},
                {"tipo":"grafito","puntos":[0,0,5,5],"dureza":"3h"},
                {"tipo":"instrumento","ejes":2,"caja":[100,100,320,320],"unidad":40,"tabla":[[1,2],[-3,1.5]]},
                {"tipo":"recta","caja":[0,0,300,60]},
                {"tipo":"punto","x":10,"y":20,"texto":"A"},
                {"tipo":"rectangulo","caja":[0,0,10,10],"trazo":"discontinuo","opacidad":0.5,"relleno_estilo":"rayado","capa":-2}]}"#,
        );
        assert_eq!(f[0].tipo, TipoFigura::Lapiz);
        assert_eq!(f[0].puntos.len(), 3);
        assert_eq!((f[0].estilo.material, f[0].estilo.dureza), (Some("cuadritos"), Some("6B")));
        assert_eq!(f[0].estilo.grosor, Some(3.0));
        // Una dureza sin material es grafito; 3H se lleva a la mas cercana.
        assert_eq!((f[1].estilo.material, f[1].estilo.dureza), (Some("cuadritos"), Some("4H")));
        let i = f[2].instrumento.as_ref().unwrap();
        assert_eq!((i.ejes, i.unidad, i.tabla.len()), (2, 40.0, 2));
        assert_eq!(f[3].instrumento.as_ref().unwrap().ejes, 1);
        assert_eq!(f[4].texto, "A");
        assert_eq!(f[5].estilo.trazo, Some("dashed"));
        assert_eq!(f[5].estilo.opacidad, Some(50.0));
        assert_eq!(f[5].estilo.relleno, Some("hachure"));
        assert_eq!(f[5].capa, -2);
        assert_eq!(dureza("8b"), Some("8B"));
        assert_eq!(dureza("HB"), Some("HB"));
        assert_eq!(dureza("12Z"), None);
        // Casos negativos: tabla impar, enorme o con demasiados puntos.
        for (args, motivo) in [
            (r#"{"elementos":[{"tipo":"instrumento","tabla":[1,2,3]}]}"#, "impar"),
            (r#"{"elementos":[{"tipo":"instrumento","tabla":[[1,1e9]]}]}"#, "enorme"),
            (r#"{"elementos":[{"tipo":"instrumento","ejes":7}]}"#, "«ejes»"),
            (r#"{"elementos":[{"tipo":"lapiz","puntos":[1,1]}]}"#, "al menos 2"),
            (r#"{"elementos":[{"tipo":"rectangulo","caja":[0,0,9,9],"grosor":500}]}"#, "«grosor»"),
        ] {
            let e = validar_(DIBUJAR, args).unwrap_err().to_string();
            assert!(e.contains(motivo), "{args}: {e}");
        }
        let muchos: Vec<String> = (0..=MAX_TABLA).map(|i| format!("[{i},1]")).collect();
        assert!(validar_(DIBUJAR, &format!(r#"{{"elementos":[{{"tipo":"instrumento","tabla":[{}]}}]}}"#, muchos.join(","))).is_err());
        // Lo cosmetico raro no rompe: aviso y lo de siempre.
        let v = validar_(DIBUJAR, r#"{"elementos":[{"tipo":"lapiz","puntos":[0,0,9,9],"material":"oro","dureza":"ZZ"}]}"#).unwrap();
        assert_eq!(v.avisos.len(), 2);
    }

    #[test]
    fn calcar_y_ayuda_y_acciones_se_validan() {
        let v = validar_(CALCAR, r#"{"detalle":"alto","solo_contornos":true,"zona":[0,0,0.5,0.5]}"#).unwrap();
        let Llamada::Calcar(p) = v.llamada else { panic!() };
        assert_eq!(p.detalle, crate::calcar::Detalle::Alto);
        assert!(p.solo_contornos && p.colores);
        assert_eq!(p.zona, Some([0.0, 0.0, 0.5, 0.5]));
        assert!(validar_(CALCAR, "").is_ok());
        assert!(validar_(CALCAR, r#"{"zona":[0,0,2,1]}"#).is_err());
        assert!(validar_(CALCAR, r#"{"detalle":"extremo"}"#).is_err());
        let v = validar_(AYUDA, r#"{"pregunta":"donde estan los ajustes"}"#).unwrap();
        assert_eq!(v.llamada, Llamada::Ayuda("donde estan los ajustes".into()));
        assert!(validar_(AYUDA, "{}").is_err());
        let v = validar_("anadir_tarea", r#"{"texto":"llamar"}"#).unwrap();
        assert_eq!(v.llamada.nombre(), "anadir_tarea");
        let Llamada::App(a) = v.llamada else { panic!() };
        assert_eq!(a.pedido["accion"], "anadir_tarea");
        // Una de la app que no se enseno en este turno: no existe para el modelo.
        let e = validar(
            &crate::motor::LlamadaCruda { id: "1".into(), nombre: "anadir_tarea".into(), argumentos: "{}".into() },
            &catalogo_base(),
        )
        .unwrap_err();
        assert_eq!(e, ErrorHerramienta::Desconocida("anadir_tarea".into()));
    }
    /// El local no cambia (decision del usuario, 15-oct): la tabla y las
    /// guias de eleccion son solo para la nube.
    #[test]
    fn lo_de_la_nube_no_toca_el_catalogo_del_local() {
        let local = catalogo();
        let mut nube = catalogo();
        para_la_nube(&mut nube);
        let de = |l: &[Herramienta], n: &str| l.iter().find(|h| h.nombre == n).unwrap().clone();
        let (dl, dn) = (de(&local, DIBUJAR), de(&nube, DIBUJAR));
        assert!(!dl.descripcion.contains("tabla (con") && !dl.parametros.to_string().contains("\"filas\""));
        assert!(dn.descripcion.contains("tabla (con") && dn.parametros.to_string().contains("\"filas\""));
        assert!(dn.parametros.to_string().contains("\"paralelogramo\""));
        assert!(de(&nube, PROBLEMA).descripcion.contains("no hoja_a_lienzo"));
        assert!(!de(&local, PROBLEMA).descripcion.contains("no hoja_a_lienzo"));
        // Banco de documentos (15-oct): el LaTeX en el texto, la tabla al
        // lienzo y el tamano de letra; el local, igual que antes.
        assert!(de(&nube, ECUACION).descripcion.contains("LaTeX"));
        assert!(!de(&local, ECUACION).descripcion.contains("sin herramienta"));
        assert!(de(&nube, crate::edicion::ESTILO).parametros["properties"].get("tam_letra").is_some());
        assert!(de(&local, crate::edicion::ESTILO).parametros["properties"].get("tam_letra").is_none());
        // Dos veces da lo mismo (es prefijo de la cache).
        let mut otra = nube.clone();
        para_la_nube(&mut otra);
        assert_eq!(otra.iter().map(|h| h.descripcion.clone()).collect::<Vec<_>>(), nube.iter().map(|h| h.descripcion.clone()).collect::<Vec<_>>());
    }
}
