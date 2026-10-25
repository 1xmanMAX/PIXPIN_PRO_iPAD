//! **Las conexiones como herramientas del asistente**: filas de la misma
//! tabla que el resto de la app ([`Accion`]), con el mismo generador de
//! JSON Schema y la misma validacion.
//!
//! Son cuatro para TODOS los servicios: el parametro `servicio` (por ahora
//! solo `google_drive`) elige cual. Solo para la nube (la IA local esta
//! congelada): sin claves de seleccion y en `acciones::SOLO_NUBE`.
//!
//! Las hace [`hacer`], con lo que pone la app detras del rasgo [`Chat`]
//! (elegir un archivo del chat, sacar el texto de un PDF, dejar un archivo
//! en el chat, preguntar «¿Sí?»). Asi la logica entera se prueba aqui con
//! un servidor falso, y la app solo pone sus piezas de siempre.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{
    Almacen, Archivo, Conexion, Contenido, Servicio, TIPOS, lista_para_el_modelo, por_id, tamano,
};
use crate::acciones::{Accion, AccionApp, Categoria, Param, Tipo};

/// La `accion` de estas filas: ni un pedido del protocolo ni del ejecutor
/// del chat de siempre; las hace [`hacer`].
pub const CONEXION: &str = "(conexion)";

pub const BUSCAR: &str = "buscar_en_conexion";
pub const LEER: &str = "leer_de_conexion";
pub const BAJAR: &str = "bajar_de_conexion";
pub const SUBIR: &str = "subir_a_conexion";
/// Los eventos de Google Calendar (18-oct): la contesta la app con lo que
/// ya tiene leido (`calendario_google`), sin esperar a la red.
pub const VER_CALENDARIO: &str = "ver_calendario";

/// Los ids de los servicios, para el `enum` del esquema.
const IDS: &[&str] = &["google_drive"];

const fn p(
    nombre: &'static str,
    tipo: Tipo,
    obligatorio: bool,
    descripcion: &'static str,
) -> Param {
    Param {
        nombre,
        destino: "",
        tipo,
        obligatorio,
        descripcion,
        defecto: None,
        largo: None,
    }
}

const fn d(defecto: &'static str, mut p: Param) -> Param {
    p.defecto = Some(defecto);
    p
}

const SERVICIO: Param = d(
    r#""google_drive""#,
    p(
        "servicio",
        Tipo::Opciones(IDS),
        false,
        "el servicio conectado; omitir: google_drive",
    ),
);
const ARCHIVO: &str = "id del archivo (el que da buscar_en_conexion) o su nombre exacto";

/// **La tabla de las conexiones.**
pub const ACCIONES_CONEXIONES: &[Accion] = &[
    Accion {
        herramienta: BUSCAR,
        accion: CONEXION,
        descripcion: "Busca archivos en el Google Drive del usuario (por nombre o por lo que dicen dentro) y da su nombre, tipo, fecha e id. \
                      Sin texto, lo ultimo modificado. Usala para «busca en mi Drive...», «¿tengo en Drive el contrato...?».",
        categoria: Categoria::Abrir,
        claves: &[],
        fijos: "{}",
        params: &[
            p(
                "texto",
                Tipo::Texto,
                false,
                "lo que se busca; omitir: lo mas reciente",
            ),
            d(
                r#""cualquiera""#,
                p("tipo", Tipo::Opciones(TIPOS), false, "solo de este tipo"),
            ),
            d(
                "10",
                p("maximo", Tipo::Entero, false, "cuantos como mucho (1-25)"),
            ),
            SERVICIO,
        ],
        confirmar: None,
        hecho: "Hecho.",
    },
    Accion {
        herramienta: LEER,
        accion: CONEXION,
        descripcion: "Lee el texto de un archivo del Google Drive del usuario: un Documento o Presentacion de Google, una Hoja de calculo (en CSV), \
                      un PDF, Word o texto. Usala para resumirlo o contestar sobre el sin bajarlo.",
        categoria: Categoria::Notas,
        claves: &[],
        fijos: "{}",
        params: &[
            p("archivo", Tipo::Texto, true, ARCHIVO),
            d(
                "1",
                p(
                    "parte",
                    Tipo::Entero,
                    false,
                    "si el texto es largo, que trozo (1, 2, 3...)",
                ),
            ),
            SERVICIO,
        ],
        confirmar: None,
        hecho: "Hecho.",
    },
    Accion {
        herramienta: BAJAR,
        accion: CONEXION,
        descripcion: "Baja un archivo del Google Drive del usuario y lo deja en este chat (un Doc de Google sale como Word, una Hoja como Excel). \
                      Para trabajar despues con el en PixPin (anotarlo, abrirlo, pasarlo al lienzo).",
        categoria: Categoria::Abrir,
        claves: &[],
        fijos: "{}",
        params: &[p("archivo", Tipo::Texto, true, ARCHIVO), SERVICIO],
        confirmar: None,
        hecho: "Hecho.",
    },
    Accion {
        herramienta: SUBIR,
        accion: CONEXION,
        descripcion: "Sube un archivo de este chat al Google Drive del usuario (pide permiso antes). Opcional: la carpeta de Drive y otro nombre.",
        categoria: Categoria::Abrir,
        claves: &[],
        fijos: "{}",
        params: &[
            p(
                "archivo",
                Tipo::Texto,
                false,
                "nombre o descripcion del archivo del chat; omitir: el mas reciente",
            ),
            p(
                "carpeta",
                Tipo::Texto,
                false,
                "carpeta de Drive (su id o su nombre); omitir: la raiz de Mi unidad",
            ),
            p(
                "nombre",
                Tipo::Texto,
                false,
                "nombre con el que se guarda en Drive; omitir: el mismo",
            ),
            SERVICIO,
        ],
        confirmar: Some("¿Subo[ «{archivo}»] a {servicio}?"),
        hecho: "Hecho.",
    },
    Accion {
        herramienta: VER_CALENDARIO,
        accion: CONEXION,
        descripcion: "Lee los eventos del Google Calendar del usuario (todos sus calendarios visibles): titulo, dia, hora, lugar.                       Usala para «¿que tengo hoy?», «¿tengo algo el jueves?», «mi agenda de esta semana». Solo lee: no crea ni cambia eventos.",
        categoria: Categoria::Abrir,
        claves: &[],
        fijos: "{}",
        params: &[
            d(
                r#""hoy""#,
                p(
                    "desde",
                    Tipo::Texto,
                    false,
                    "el primer dia: hoy, mañana, un dia de la semana (jueves), semana (esta semana) o una fecha AAAA-MM-DD",
                ),
            ),
            d(
                "1",
                p(
                    "dias",
                    Tipo::Entero,
                    false,
                    "cuantos dias desde ese (1-31); con desde=semana, 7",
                ),
            ),
        ],
        confirmar: None,
        hecho: "Hecho.",
    },
];

/// Si la herramienta es de las conexiones.
pub fn es_de_conexiones(herramienta: &str) -> bool {
    ACCIONES_CONEXIONES
        .iter()
        .any(|a| a.herramienta == herramienta)
}

/// Los nombres, para `acciones::SOLO_NUBE`.
pub const NOMBRES: &[&str] = &[BUSCAR, LEER, BAJAR, SUBIR, VER_CALENDARIO];

/// Cuanto texto se le da al modelo de una vez (lo mismo que `leer_documento`).
pub const LETRAS: usize = 40_000;

/// **Lo que pone la app**: sus piezas de siempre.
pub trait Chat {
    /// Un archivo del chat por su nombre o descripcion (`None`: el mas
    /// reciente): (nombre, ruta).
    fn archivo(&self, descripcion: Option<&str>) -> Result<(String, PathBuf), String>;
    /// El texto de un fichero con el lector de la app (PDF, Word, OCR...).
    fn texto_de(&self, ruta: &Path) -> Result<String, String>;
    /// Deja un fichero en el chat, con una frase.
    fn al_chat(&self, ruta: &Path, frase: &str) -> Result<(), String>;
    /// Pregunta al usuario («¿Sí?»).
    fn confirmar(&self, pregunta: &str) -> bool;
    /// Donde dejar lo bajado.
    fn carpeta(&self) -> PathBuf;
}

fn texto<'a>(v: &'a Value, campo: &str) -> Option<&'a str> {
    v.get(campo)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Si parece un id de Drive (y no un nombre).
fn parece_id(s: &str) -> bool {
    s.len() >= 15
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// **Elige un archivo** por su id o su nombre. Varios parecidos y ninguno
/// exacto: se le dicen al modelo para que pregunte o use el id.
pub fn elegir(
    s: &Servicio,
    al: &dyn Almacen,
    archivo: &str,
    tipo: &str,
) -> Result<Archivo, String> {
    let archivo = archivo.trim().trim_matches(['"', '\'', '«', '»']);
    if parece_id(archivo)
        && let Ok(a) = al.archivo(archivo)
    {
        return Ok(a);
    }
    let hallados = al.buscar(archivo, tipo, 10)?;
    let sin_ext = |n: &str| n.rsplit_once('.').map_or(n, |(a, _)| a).to_lowercase();
    let buscado = archivo.to_lowercase();
    if let Some(a) = hallados
        .iter()
        .find(|a| a.nombre.to_lowercase() == buscado || sin_ext(&a.nombre) == buscado)
    {
        return Ok(a.clone());
    }
    match hallados.len() {
        0 => Err(format!(
            "no encontre «{archivo}» en {}; prueba buscar_en_conexion con otras palabras",
            s.nombre
        )),
        1 => Ok(hallados[0].clone()),
        _ => Err(format!(
            "hay varios parecidos; pregunta cual o usa su id.\n{}",
            lista_para_el_modelo(s, &hallados)
        )),
    }
}

/// Un nombre que vale como fichero de Windows.
fn nombre_de_fichero(n: &str) -> String {
    let limpio: String = n
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let limpio = limpio.trim().trim_end_matches('.').to_string();
    if limpio.is_empty() {
        "archivo".into()
    } else {
        limpio
    }
}

/// **Hace una herramienta de las conexiones.** `abrir`: la conexion del
/// servicio (la de siempre, o la de las pruebas). `Ok` lleva lo que se le
/// cuenta al modelo.
pub fn hacer(
    a: &AccionApp,
    abrir: &dyn Fn(&'static Servicio) -> Result<Conexion, String>,
    chat: &dyn Chat,
) -> Result<String, String> {
    let args = &a.pedido;
    if a.herramienta == VER_CALENDARIO {
        return Err("los eventos de Google Calendar los da la app".into());
    }
    let s = por_id(texto(args, "servicio").unwrap_or("google_drive"))
        .ok_or("ese servicio no se puede conectar todavia")?;
    if s.id == super::calendario::GOOGLE_CALENDAR.id {
        return Err("Google Calendar no es un servicio de archivos: para sus eventos usa la herramienta calendario".into());
    }
    // En subir, el archivo es el del chat de verdad (se elige primero) y la
    // pregunta lleva su nombre y su tamano; sin el «Sí» no sale ni un byte.
    let subir = a.herramienta == SUBIR;
    let elegido = if subir {
        Some(chat.archivo(texto(args, "archivo"))?)
    } else {
        None
    };
    let cx = abrir(s)?;
    let al = cx.almacen();
    match a.herramienta {
        BUSCAR => {
            let max = args
                .get("maximo")
                .and_then(Value::as_u64)
                .unwrap_or(10)
                .clamp(1, 25) as usize;
            let hallados = al.buscar(
                texto(args, "texto").unwrap_or(""),
                texto(args, "tipo").unwrap_or("cualquiera"),
                max,
            )?;
            Ok(lista_para_el_modelo(s, &hallados))
        }
        LEER => {
            let f = elegir(
                s,
                al.as_ref(),
                texto(args, "archivo").unwrap_or(""),
                "cualquiera",
            )?;
            let entero = match al.leer(&f)? {
                Contenido::Texto(t) => t,
                Contenido::Fichero { nombre, bytes } => {
                    let dir = chat.carpeta();
                    std::fs::create_dir_all(&dir)
                        .map_err(|e| format!("sin sitio para leerlo: {e}"))?;
                    let ruta = dir.join(nombre_de_fichero(&nombre));
                    std::fs::write(&ruta, &bytes)
                        .map_err(|e| format!("no se pudo guardar para leerlo: {e}"))?;
                    let t = chat.texto_de(&ruta);
                    let _ = std::fs::remove_file(&ruta);
                    t?
                }
            };
            let letras: Vec<char> = entero.chars().collect();
            let partes = letras.len().div_ceil(LETRAS).max(1);
            let parte =
                (args.get("parte").and_then(Value::as_u64).unwrap_or(1) as usize).clamp(1, partes);
            let trozo: String = letras
                .iter()
                .skip((parte - 1) * LETRAS)
                .take(LETRAS)
                .collect();
            if trozo.trim().is_empty() {
                return Ok(format!("«{}» no tiene texto.", f.nombre));
            }
            let mut out = format!("«{}» de {} ({} letras", f.nombre, s.nombre, letras.len());
            if partes > 1 {
                out.push_str(&format!(", parte {parte} de {partes}"));
            }
            out.push_str("):\n");
            out.push_str(&trozo);
            if parte < partes {
                out.push_str(&format!("\n[...sigue: pide «parte»: {}]", parte + 1));
            }
            Ok(out)
        }
        BAJAR => {
            let f = elegir(
                s,
                al.as_ref(),
                texto(args, "archivo").unwrap_or(""),
                "cualquiera",
            )?;
            let (nombre, bytes) = al.bajar(&f)?;
            let dir = chat.carpeta();
            std::fs::create_dir_all(&dir).map_err(|e| format!("sin sitio para bajarlo: {e}"))?;
            let ruta = dir.join(nombre_de_fichero(&nombre));
            std::fs::write(&ruta, &bytes).map_err(|e| format!("no se pudo guardar: {e}"))?;
            chat.al_chat(&ruta, &format!("De {}: «{nombre}»", s.nombre))?;
            Ok(format!(
                "«{nombre}» ({}) bajado de {} y ya esta en el chat.",
                tamano(bytes.len() as u64),
                s.nombre
            ))
        }
        SUBIR => {
            let (nombre_local, ruta) = elegido.unwrap_or_default();
            let nombre = texto(args, "nombre")
                .map(str::to_string)
                .unwrap_or_else(|| nombre_local.clone());
            let carpeta = match texto(args, "carpeta") {
                None => None,
                Some(c) => Some(elegir(s, al.as_ref(), c, "carpeta").and_then(|f| {
                    if f.carpeta {
                        Ok(f)
                    } else {
                        Err(format!("«{}» no es una carpeta", f.nombre))
                    }
                })?),
            };
            let bytes = std::fs::read(&ruta)
                .map_err(|e| format!("no se pudo leer «{nombre_local}»: {e}"))?;
            let donde = carpeta
                .as_ref()
                .map(|c| format!(" en la carpeta «{}»", c.nombre))
                .unwrap_or_default();
            let como = if nombre != nombre_local {
                format!(" como «{nombre}»")
            } else {
                String::new()
            };
            let pregunta = format!(
                "¿Subo «{nombre_local}» ({}) a tu {}{donde}{como}?",
                tamano(bytes.len() as u64),
                s.nombre
            );
            if !chat.confirmar(&pregunta) {
                return Ok(format!(
                    "El usuario dijo que no: «{nombre_local}» no se subio."
                ));
            }
            let f = al.subir(&nombre, &bytes, carpeta.as_ref().map(|c| c.id.as_str()))?;
            let enlace = if f.enlace.is_empty() {
                String::new()
            } else {
                format!(" Enlace: {}", f.enlace)
            };
            Ok(format!(
                "«{}» subido a {}{donde} (id: {}).{enlace}",
                f.nombre, s.nombre, f.id
            ))
        }
        otra => Err(format!("la herramienta «{otra}» no es de las conexiones")),
    }
}
