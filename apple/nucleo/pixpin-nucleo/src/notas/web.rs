//! **El editor de notas de Windows, en el Mac** (`notas_md/lector_web.rs`):
//! la misma pagina (`recursos/lector-md.html`, CodeMirror con el Markdown
//! vivo, la barra flotante, el menu `/`, las tablas, las formulas, los
//! comentarios al margen, las fotos bajo su renglon) dentro de un
//! `WKWebView`. La pagina se construye en Windows
//! (`herramientas/lector-md`, `npm run build`) y aqui se copia tal cual:
//! asi las dos notas se ven y se manejan igual.
//!
//! Lo de cada sistema:
//! - El puente: la pagina habla con `chrome.webview` (WebView2). En el Mac,
//!   [`PUENTE`] lo hace pasar por `webkit.messageHandlers.pixpin`, y Swift
//!   contesta con `window.__pixpin(json)`.
//! - Las fotos se piden a `pixpin-nota://nota/archivo?r=…` (un
//!   `WKURLSchemeHandler`; WebKit no deja servir `https://`), que contesta
//!   [`SesionNota::servir`].
//! - Las teclas: CodeMirror ya usa ⌘ donde Windows usa Ctrl (`Mod-`). Los
//!   rotulos de los menus dicen ⌘, ⇧⌘ y ⌥⌘ ([`adaptar`]); ⌥⌘M comenta y
//!   ⌘-clic abre un enlace (en Windows, Ctrl+Alt+M y Ctrl+clic).
//!
//! Lo demas es el mismo protocolo que Windows ([`DeLaPagina`]): guardar a
//! los 0,8 s de dejar de escribir, lo que llega de otro aparato aparece solo
//! si aqui no hay nada sin guardar, la letra y el tamano en
//! `vista-de-notas.txt`, copia `.md`, exportar a Word, compartir, adjuntar
//! (elegir, pegar o soltar), meter un mensaje del chat y los comentarios.
//! Lo que toca ventanas y paneles lo hace Swift con lo que devuelve
//! [`SesionNota::recibir`] ([`OrdenNota`]).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use serde::Deserialize;
use serde_json::json;

use pixpin_docs::md_comentarios::Quien;
use pixpin_docs::{exportar_docx, md_vivo};
use pixpin_proyecto::{almacen, cuaderno};

use super::comentarios::{self, Comentarista};
use super::{DestinoNota, adjuntos, vista};
use crate::{ErrorNucleo, MensajeChat, Nucleo};

/// La pagina de Windows, tal cual.
const PAGINA: &str = include_str!("../../recursos/lector-md.html");

/// De donde pide la pagina sus fotos y audios.
pub const ORIGEN: &str = "pixpin-nota://nota/";

/// Donde se recuerda la letra y el tamano de las notas: al lado del
/// almacen y sin viajar (el mismo nombre que Windows).
pub const FICHERO_VISTA: &str = "vista-de-notas.txt";

/// Cuantos mensajes ofrece «Del chat»: los ultimos.
const EN_EL_MENU: usize = 40;

/// Lo que se pone delante del programa de la pagina: `chrome.webview` sobre
/// WebKit, ⌥⌘M para comentar y ⌘-clic en un enlace.
pub const PUENTE: &str = r#"(()=>{const oyentes=[];window.chrome=window.chrome||{};
window.chrome.webview={postMessage:s=>{try{window.webkit.messageHandlers.pixpin.postMessage(s)}catch(e){}},
addEventListener:(t,f)=>{if(t==='message')oyentes.push(f)}};
window.__pixpin=d=>{for(const f of oyentes)f({data:d})};
document.addEventListener('keydown',e=>{if(e.metaKey&&e.altKey&&!e.ctrlKey&&e.code==='KeyM'){e.preventDefault();e.stopImmediatePropagation();
document.dispatchEvent(new KeyboardEvent('keydown',{key:'m',code:'KeyM',ctrlKey:true,altKey:true,bubbles:true,cancelable:true}))}},true);
document.addEventListener('mousedown',e=>{if(e.isTrusted&&e.metaKey&&!e.ctrlKey&&e.target.closest&&e.target.closest('.pp-enlace')){e.preventDefault();e.stopImmediatePropagation();
e.target.dispatchEvent(new MouseEvent('mousedown',{bubbles:true,cancelable:true,ctrlKey:true,clientX:e.clientX,clientY:e.clientY,button:0,detail:e.detail,view:window}))}},true);
const marca=e=>{if(document.body)document.body.classList.toggle('ctrl',e.metaKey||e.ctrlKey)};
document.addEventListener('keydown',marca);document.addEventListener('keyup',marca)})();"#;

/// Los rotulos de las teclas, de Windows a Mac.
const TECLAS: [(&str, &str); 7] = [
    ("\"Ctrl+May\\xFAs+", "\"\u{21e7}\u{2318}"),
    ("\"Ctrl+Alt+M\"", "\"\u{2325}\u{2318}M\""),
    ("\"Ctrl+Intro\"", "\"\u{2318}\u{21a9}\""),
    ("\"Ctrl+", "\"\u{2318}"),
    ("(Ctrl+Alt+M)", "(\u{2325}\u{2318}M)"),
    ("Ctrl + y Ctrl ", "\u{2318} + y \u{2318} "),
    ("Ctrl + and Ctrl ", "\u{2318} + and \u{2318} "),
];

/// La pagina de Windows con lo del Mac: el puente delante de todo, las
/// fotos por `pixpin-nota://` y las teclas con sus simbolos.
pub fn adaptar(html: &str) -> String {
    let mut t = html.replace("https://pixpin.nota/", ORIGEN);
    for (de, a) in TECLAS {
        t = t.replace(de, a);
    }
    t.replacen("<head>", &format!("<head>\n<script>{PUENTE}</script>"), 1)
}

/// **La pagina del editor de notas**, lista para el `WKWebView`.
#[uniffi::export]
pub fn nota_pagina() -> String {
    static P: OnceLock<String> = OnceLock::new();
    P.get_or_init(|| adaptar(PAGINA)).clone()
}

/// Lo que puede mandar la pagina (el mismo `DeLaPagina` de Windows).
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum DeLaPagina {
    Listo,
    /// Lo que encienden los botones de la barra del pin: viendo la nota
    /// (sin escribir) y el panel de los comentarios abierto (Windows, 17-oct).
    Estado {
        leyendo: bool,
        comentarios: bool,
    },
    Guardar {
        texto: String,
        version: u64,
    },
    Vista {
        cuerpo: String,
        titulos: String,
        px: u32,
        solo_esta: bool,
    },
    Titulo {
        texto: String,
    },
    AbrirEnlace {
        url: String,
    },
    AbrirPixpin {
        ruta: String,
    },
    Adjuntar {
        id: u64,
        nombre: String,
        datos: String,
    },
    Elegir {
        clase: String,
    },
    ElegirHoja {
        viva: bool,
    },
    InsertarHoja {
        clave: String,
        viva: bool,
    },
    ElegirMensaje,
    InsertarMensaje {
        clave: String,
    },
    GuardarCopia {
        texto: String,
        titulo: String,
    },
    ExportarWord {
        texto: String,
        titulo: String,
    },
    Ventana {
        accion: String,
        #[serde(default)]
        borde: String,
    },
    ComentariosPedir {
        texto: String,
    },
    ComentarioNuevo {
        texto: String,
        desde: usize,
        hasta: usize,
        cuerpo: String,
    },
    ComentarioResponder {
        texto: String,
        hilo: String,
        cuerpo: String,
    },
    ComentarioEditar {
        texto: String,
        id: String,
        cuerpo: String,
    },
    ComentarioBorrar {
        texto: String,
        id: String,
    },
    ComentarioResolver {
        texto: String,
        hilo: String,
        si: bool,
    },
    Compartir,
    Cerrar,
    CerrarListo,
    EditorAnterior,
}

/// **Lo que Swift tiene que hacer** tras un mensaje de la pagina.
#[allow(clippy::large_enum_variant)] // UniFFI no deja `Box` en un enum
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum OrdenNota {
    /// Un JSON para la pagina: `window.__pixpin(json)`.
    Mandar { json: String },
    /// Se guardo: la burbuja del chat tiene que ensenar lo nuevo.
    Guardada { mensaje: Option<MensajeChat> },
    /// Elegir ficheros con el panel del Mac (`imagenes`: solo fotos); lo
    /// elegido vuelve por [`SesionNota::adjuntar_ficheros`].
    Elegir { imagenes: bool },
    /// Guardar estos bytes donde diga el panel de guardar del Mac.
    GuardarComo {
        nombre: String,
        extension: String,
        datos: Vec<u8>,
    },
    /// `arrastrar`, `minimizar`, `maximizar` o `pantalla`.
    Ventana { accion: String },
    /// El titulo de la ventana.
    Titulo { texto: String },
    /// Una pagina o un correo, en el navegador.
    AbrirEnlace { url: String },
    /// Un fichero de la nota (una foto, un documento, un audio).
    AbrirFichero { ruta: String },
    /// Un mensaje del chat metido en la nota.
    AbrirMensaje { chat: String, codigo: String },
    /// La hoja de compartir del Mac con la nota.
    Compartir { titulo: String, texto: String },
    /// La pagina ya guardo lo ultimo: cerrar la ventana.
    Cerrar,
    /// Abrir la misma nota en el editor de antes (el de AppKit).
    EditorAnterior,
    /// Lo que encienden los botones de la barra del pin.
    EstadoBarra { leyendo: bool, comentarios: bool },
}

/// Un fichero que la pagina pidio, con su tipo.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RecursoNota {
    pub tipo: String,
    pub datos: Vec<u8>,
}

/// Si una direccion se puede abrir fuera: solo paginas y correo, nunca un
/// programa ni un fichero.
pub fn es_enlace_de_fuera(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    (u.starts_with("https://") || u.starts_with("http://") || u.starts_with("mailto:"))
        && !u.starts_with(ORIGEN)
}

/// El tipo de un fichero que se le sirve a la pagina.
pub fn tipo_de(ruta: &Path) -> &'static str {
    match ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("svg") => "image/svg+xml",
        Some("avif") => "image/avif",
        Some("heic") => "image/heic",
        Some("ico") => "image/x-icon",
        Some("mp3") => "audio/mpeg",
        Some("m4a" | "aac") => "audio/mp4",
        Some("ogg" | "oga" | "opus") => "audio/ogg",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("mp4" | "m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        _ => "application/octet-stream",
    }
}

/// El renglon que entra en la nota por un fichero adjuntado:
/// `![nombre](ruta)` para todo (el movil decide por la extension si es
/// foto, audio, video o documento).
pub fn renglon_de(nombre: &str, ruta: &str) -> String {
    let alt: String = nombre
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | '\n' | '\r'))
        .collect();
    format!("![{}]({ruta})", alt.trim())
}

/// Sin lo que romperia el `[…]`: corchetes y saltos de renglon.
fn texto_limpio(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' {
                ' '
            } else {
                c
            }
        })
        .filter(|c| !matches!(c, '[' | ']'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// El renglon de un mensaje del chat (`pixpin_notas::incrustados` de
/// Windows): su primera frase como texto del enlace y sus codigos en la
/// direccion `pixpin:mensaje=<proyecto>/<mensaje>`.
pub fn renglon_de_mensaje(texto: &str, proyecto: &str, mensaje: &str) -> String {
    const LARGO: usize = 80;
    let primera = texto
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let limpia = texto_limpio(primera);
    let mut t: String = limpia.chars().take(LARGO).collect();
    if limpia.chars().count() > LARGO {
        t.push('…');
    }
    if t.is_empty() {
        t = mensaje.to_string();
    }
    format!("[{t}](pixpin:mensaje={proyecto}/{mensaje})")
}

/// El proyecto y el mensaje de un enlace `pixpin:mensaje=<proyecto>/<codigo>`.
pub fn mensaje_del_enlace(url: &str) -> Option<(String, String)> {
    let resto = url.trim().strip_prefix("pixpin:mensaje=")?;
    let (p, m) = resto.split_once('/')?;
    let bien = |s: &str| !s.is_empty() && !s.contains(['/', '\\', ' ', ')', '(', '"']);
    (bien(p) && bien(m)).then(|| (p.to_string(), m.to_string()))
}

/// Un nombre de fichero a partir del titulo (sin `<>:"/\|?*`).
pub fn nombre_de_fichero(titulo: &str, reserva: &str) -> String {
    let n: String = titulo
        .chars()
        .filter(|c| !"<>:\"/\\|?*".contains(*c) && !c.is_control())
        .collect();
    let n = n.trim().trim_end_matches('…').trim().to_string();
    if n.is_empty() { reserva.to_string() } else { n }
}

/// `%41%C3%B1` → `Añ` (lo que hace `encodeURIComponent`, al reves).
fn descodificar_url(s: &str) -> String {
    let b = s.as_bytes();
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(x) =
                u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("-"), 16)
        {
            v.push(x);
            i += 3;
        } else {
            v.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&v).into_owned()
}

/// El valor de `nombre` en `archivo?r=…&x=…`.
fn parametro(peticion: &str, nombre: &str) -> Option<String> {
    let (_, consulta) = peticion.split_once('?')?;
    consulta.split('&').find_map(|par| {
        let (k, v) = par.split_once('=')?;
        (k == nombre).then(|| descodificar_url(v))
    })
}

/// Base64 normal (lo que da `FileReader.readAsDataURL`), sin el prefijo.
fn de_base64(s: &str) -> Option<Vec<u8>> {
    fn valor(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    }
    let limpio: Vec<u8> = s
        .bytes()
        .filter(|c| !c.is_ascii_whitespace() && *c != b'=')
        .collect();
    let mut out = Vec::with_capacity(limpio.len() * 3 / 4);
    for trozo in limpio.chunks(4) {
        let mut n = 0u32;
        for (i, c) in trozo.iter().enumerate() {
            n |= valor(*c)? << (18 - 6 * i);
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        match trozo.len() {
            4 => out.extend_from_slice(&bytes),
            3 => out.extend_from_slice(&bytes[..2]),
            2 => out.push(bytes[0]),
            _ => return None,
        }
    }
    Some(out)
}

/// La huella de donde vive la nota: si cambia, alguien la toco.
fn huella(raiz: &Path, destino: &DestinoNota) -> Option<(SystemTime, u64)> {
    match destino {
        DestinoNota::Fichero { ruta } => {
            let m = std::fs::metadata(ruta).ok()?;
            Some((m.modified().ok()?, m.len()))
        }
        DestinoNota::Mensaje { chat, .. } => {
            let carpeta = almacen::carpeta(raiz, chat);
            let mut mas = std::fs::metadata(&carpeta).ok()?.modified().ok()?;
            let mut n = 0u64;
            for e in std::fs::read_dir(&carpeta).ok()?.flatten() {
                if let Ok(m) = e.metadata()
                    && m.is_file()
                {
                    n += m.len();
                    if let Ok(f) = m.modified() {
                        mas = mas.max(f);
                    }
                }
            }
            Some((mas, n))
        }
    }
}

/// El mensaje `NOTA` de la nota, leido del cuaderno.
fn mensaje_de(raiz: &Path, chat: &str, id: &str) -> Option<cuaderno::Mensaje> {
    cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, chat))
        .ok()?
        .mensajes
        .into_iter()
        .find(|m| m.id == id)
}

/// El texto de la nota como esta ahora en el disco.
pub fn leer_texto(raiz: &Path, destino: &DestinoNota) -> Option<String> {
    match destino {
        DestinoNota::Fichero { ruta } => std::fs::read(ruta)
            .ok()
            .map(|b| String::from_utf8_lossy(&b).into_owned()),
        DestinoNota::Mensaje { chat, mensaje } => mensaje_de(raiz, chat, mensaje).map(|m| m.texto),
    }
}

/// Como se llama una nota en `vista-de-notas.txt`: por su codigo unico (el
/// que no cambia aunque cambie el titulo) o por su ruta.
fn clave_de_vista(destino: &DestinoNota, codigo: &str) -> String {
    match destino {
        DestinoNota::Mensaje { .. } => format!("nota:{codigo}"),
        DestinoNota::Fichero { ruta } => format!("fichero:{ruta}"),
    }
}

struct Estado {
    destino: DestinoNota,
    /// El codigo unico del mensaje `NOTA` (vacio en un `.md` suelto).
    codigo: String,
    ultimo_conocido: String,
    firma: Option<(SystemTime, u64)>,
    comentarios: Comentarista,
    quien: Quien,
}

/// **Una nota abierta en el editor web.**
#[derive(uniffi::Object)]
pub struct SesionNota {
    nucleo: Arc<Nucleo>,
    ingles: bool,
    texto_inicial: String,
    estado: Mutex<Estado>,
}

#[uniffi::export]
impl Nucleo {
    /// **Abre una nota en el editor web**: un mensaje `NOTA` de un chat o un
    /// `.md` suelto. `NoExiste` si ya no esta (no se abre, para no pisarla).
    pub fn abrir_nota(
        self: Arc<Self>,
        destino: DestinoNota,
        ingles: bool,
    ) -> Result<Arc<SesionNota>, ErrorNucleo> {
        let raiz = self.ruta().to_path_buf();
        let (texto, codigo) = match &destino {
            DestinoNota::Mensaje { chat, mensaje } => {
                crate::chats::validar_chat(chat)?;
                let m = mensaje_de(&raiz, chat, mensaje).ok_or_else(|| ErrorNucleo::NoExiste {
                    que: mensaje.clone(),
                })?;
                if m.clase != Some(cuaderno::Clase::Nota) {
                    return Err(ErrorNucleo::Argumento {
                        detalle: format!("no es una nota: {mensaje}"),
                    });
                }
                let c = m.codigo_unico();
                (m.texto, c)
            }
            DestinoNota::Fichero { ruta } => {
                let t = leer_texto(&raiz, &destino)
                    .ok_or_else(|| ErrorNucleo::NoExiste { que: ruta.clone() })?;
                (t, String::new())
            }
        };
        let quien = match self.identidad() {
            Ok(i) => Quien {
                autor: i.yo.nombre.clone(),
                aparato: i.yo.codigo(),
            },
            Err(_) => Quien {
                autor: "Mac".into(),
                aparato: String::new(),
            },
        };
        let comentarios =
            Comentarista::abrir(comentarios::ruta(&raiz, &destino, &codigo), quien.clone());
        let firma = huella(&raiz, &destino);
        Ok(Arc::new(SesionNota {
            nucleo: self,
            ingles,
            texto_inicial: texto.clone(),
            estado: Mutex::new(Estado {
                destino,
                codigo,
                ultimo_conocido: texto,
                firma,
                comentarios,
                quien,
            }),
        }))
    }
}

impl SesionNota {
    fn estado(&self) -> std::sync::MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn raiz(&self) -> PathBuf {
        self.nucleo.ruta().to_path_buf()
    }

    fn insertar(texto: String) -> OrdenNota {
        OrdenNota::Mandar {
            json: json!({"tipo": "insertar", "bloque": true, "texto": texto}).to_string(),
        }
    }

    fn leer_vista(&self, e: &Estado) -> (vista::Vista, bool) {
        let contenido =
            std::fs::read_to_string(self.raiz().join(FICHERO_VISTA)).unwrap_or_default();
        vista::leer(&contenido, Some(&clave_de_vista(&e.destino, &e.codigo)))
    }

    fn guardar_vista(&self, e: &Estado, v: &vista::Vista, solo_esta: bool) {
        let ruta = self.raiz().join(FICHERO_VISTA);
        let contenido = std::fs::read_to_string(&ruta).unwrap_or_default();
        let clave = clave_de_vista(&e.destino, &e.codigo);
        let nuevo = if solo_esta {
            vista::escribir(&contenido, Some(&clave), v)
        } else {
            // La general; y si esta nota tenia la suya, se olvida.
            let sin = vista::olvidar(&contenido, &clave);
            vista::escribir(&sin, None, v)
        };
        let _ = pixpin_sincro::disco::escribir_atomico(&ruta, nuevo.as_bytes());
    }

    /// El primer mensaje para la pagina: el texto, el idioma y la vista.
    fn abrir_json(&self, e: &Estado) -> String {
        let (v, solo_esta) = self.leer_vista(e);
        let nombre = match &e.destino {
            DestinoNota::Fichero { ruta } => Path::new(ruta)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            DestinoNota::Mensaje { .. } => String::new(),
        };
        let de_chat = matches!(e.destino, DestinoNota::Mensaje { .. });
        json!({
            "tipo": "abrir",
            "idioma": if self.ingles { "en" } else { "es" },
            "texto": e.ultimo_conocido,
            "nombre": nombre,
            "vista": {"cuerpo": v.cuerpo, "titulos": v.titulos, "px": v.px, "soloEsta": solo_esta},
            // Las paginas vivas y los enlaces a hojas aun no: necesitan
            // pintar la hoja (`paginas_vivas` de Windows).
            "capacidades": {"hojas": false, "medios": de_chat},
            // La cabecera propia no: la barra es la comun de los visores
            // (Windows, 17-oct: el lector de notas como pin).
            "enPin": true,
        })
        .to_string()
    }

    fn guardar(&self, e: &mut Estado, texto: &str) -> Result<Option<MensajeChat>, ErrorNucleo> {
        match &e.destino {
            DestinoNota::Fichero { ruta } => {
                pixpin_sincro::disco::escribir_atomico(Path::new(ruta), texto.as_bytes())?;
                Ok(None)
            }
            DestinoNota::Mensaje { chat, mensaje } => self
                .nucleo
                .editar_nota(chat.clone(), mensaje.clone(), texto.to_string())
                .map(Some),
        }
    }

    /// Los mensajes del chat de la nota que se pueden meter en ella.
    fn mensajes(&self, e: &Estado) -> serde_json::Value {
        let DestinoNota::Mensaje { chat, mensaje } = &e.destino else {
            return json!({"tipo": "mensajes", "lista": []});
        };
        let mut v: Vec<cuaderno::Mensaje> =
            cuaderno::Cuaderno::leer_de(&almacen::carpeta(&self.raiz(), chat))
                .map(|c| c.mensajes)
                .unwrap_or_default()
                .into_iter()
                .filter(|m| !m.en_buzon && !m.id.is_empty() && m.id != *mensaje)
                .collect();
        v.sort_by_key(|m| std::cmp::Reverse(m.cuando));
        let lista: Vec<serde_json::Value> = v
            .iter()
            .take(EN_EL_MENU)
            .map(|m| {
                let que: String = match m.clase {
                    Some(cuaderno::Clase::Voz) => format!("\u{1f3a4} {}", texto_limpio(&m.resumen())),
                    _ => texto_limpio(m.resumen().lines().next().unwrap_or("")),
                };
                json!({"clave": m.codigo_unico(), "rotulo": que.chars().take(60).collect::<String>()})
            })
            .collect();
        json!({"tipo": "mensajes", "lista": lista})
    }

    /// Lo que se escribe en la nota por el mensaje `clave` de su chat: su
    /// audio si es una nota de voz con su fichero, o el enlace que se pinta
    /// como su burbuja.
    fn bloque_de_mensaje(&self, e: &Estado, clave: &str) -> Option<String> {
        let DestinoNota::Mensaje { chat, .. } = &e.destino else {
            return None;
        };
        let raiz = self.raiz();
        let m = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&raiz, chat))
            .ok()?
            .mensajes
            .into_iter()
            .find(|m| m.codigo_unico() == clave)?;
        if m.clase == Some(cuaderno::Clase::Voz)
            && let Some(r) = m.ruta.as_deref().filter(|r| !r.is_empty())
            && let Some(real) = pixpin_proyecto::vista::ruta_real(&raiz, chat, r)
            && real.is_file()
        {
            let ruta = if r.starts_with("pixpin:files/") {
                r.to_string()
            } else {
                pixpin_proyecto::vista::portatil_de_ruta(&raiz, &real)
                    .unwrap_or_else(|| r.to_string())
            };
            let nombre = match texto_limpio(&m.nombre) {
                n if n.is_empty() => if self.ingles {
                    "Voice note"
                } else {
                    "Nota de voz"
                }
                .to_string(),
                n => n,
            };
            return Some(format!("![{nombre}]({ruta})"));
        }
        let indice = almacen::Indice::leer(&raiz);
        let proyecto = indice
            .buscar(chat)
            .map(|f| f.codigo_unico())
            .unwrap_or_else(|| chat.clone());
        Some(renglon_de_mensaje(
            &m.resumen(),
            &proyecto,
            &m.codigo_unico(),
        ))
    }

    fn word(&self, e: &Estado, texto: &str, titulo: &str) -> Option<OrdenNota> {
        let (v, _) = self.leer_vista(e);
        let raiz = self.raiz();
        let destino = e.destino.clone();
        let fotos = |ruta: &str| {
            let p = adjuntos::resolver(&raiz, &destino, ruta)?;
            let b = std::fs::read(p).ok()?;
            exportar_docx::imagen::leer(&b).is_some().then_some(b)
        };
        let nueva = if self.ingles {
            "New note"
        } else {
            "Nota nueva"
        };
        let t = match md_vivo::titulo(texto) {
            t if t.trim().is_empty() => nueva.to_string(),
            t => t,
        };
        let bytes = exportar_docx::exportar(&exportar_docx::Nota {
            markdown: texto,
            titulo: &t,
            letra: exportar_docx::Letra {
                cuerpo: v.cuerpo,
                titulos: v.titulos,
                px: v.px,
            },
            comentarios: None,
            imagen: &fotos,
            autor: &e.quien.autor,
            ahora_ms: crate::ahora_ms(),
        })
        .ok()?;
        let sugerido = exportar_docx::nombre_de_fichero(&t);
        let sugerido = sugerido
            .strip_suffix(".docx")
            .unwrap_or(&sugerido)
            .to_string();
        Some(OrdenNota::GuardarComo {
            nombre: nombre_de_fichero(
                if sugerido.is_empty() {
                    titulo
                } else {
                    &sugerido
                },
                "Nota",
            ),
            extension: "docx".into(),
            datos: bytes,
        })
    }

    /// Que abre un `pixpin:` o una ruta de la nota.
    fn abrir_ruta(&self, e: &Estado, ruta: &str) -> Vec<OrdenNota> {
        if let Some((p, c)) = mensaje_del_enlace(ruta) {
            let indice = almacen::Indice::leer(&self.raiz());
            let chat = indice.buscar(&p).map(|f| f.id.clone()).or_else(|| {
                indice
                    .proyectos
                    .iter()
                    .find(|f| f.codigo_unico() == p)
                    .map(|f| f.id.clone())
            });
            return chat
                .map(|chat| vec![OrdenNota::AbrirMensaje { chat, codigo: c }])
                .unwrap_or_default();
        }
        match adjuntos::resolver(&self.raiz(), &e.destino, ruta).filter(|r| r.is_file()) {
            Some(r) => vec![OrdenNota::AbrirFichero {
                ruta: r.display().to_string(),
            }],
            None => Vec::new(),
        }
    }
}

#[uniffi::export]
impl SesionNota {
    /// El titulo con que nace la ventana.
    pub fn titulo(&self) -> String {
        let t = md_vivo::titulo(&self.texto_inicial);
        if t.trim().is_empty() {
            if self.ingles {
                "New note"
            } else {
                "Nota nueva"
            }
            .to_string()
        } else {
            t
        }
    }

    /// El `.md` suelto que se edita, si no es una nota de un chat.
    pub fn fichero(&self) -> Option<String> {
        match &self.estado().destino {
            DestinoNota::Fichero { ruta } => Some(ruta.clone()),
            DestinoNota::Mensaje { .. } => None,
        }
    }

    /// **Un mensaje de la pagina** (el JSON tal cual) y lo que hay que hacer.
    /// Lo que no se entiende no hace nada. Bloqueante (guarda en el disco).
    pub fn recibir(&self, mensaje: String) -> Vec<OrdenNota> {
        let Ok(pedido) = serde_json::from_str::<DeLaPagina>(&mensaje) else {
            return Vec::new();
        };
        let mandar = |v: serde_json::Value| OrdenNota::Mandar {
            json: v.to_string(),
        };
        let ahora = crate::ahora_ms();
        let mut e = self.estado();
        match pedido {
            DeLaPagina::Listo => vec![OrdenNota::Mandar {
                json: self.abrir_json(&e),
            }],
            DeLaPagina::Estado { leyendo, comentarios } => vec![OrdenNota::EstadoBarra { leyendo, comentarios }],
            DeLaPagina::Guardar { texto, version } => {
                let r = self.guardar(&mut e, &texto);
                let ok = r.is_ok();
                if ok {
                    e.ultimo_conocido = texto;
                    e.firma = huella(&self.raiz(), &e.destino);
                }
                let mut v = vec![mandar(
                    json!({"tipo": "guardado", "version": version, "ok": ok}),
                )];
                if let Ok(m) = r {
                    v.push(OrdenNota::Guardada { mensaje: m });
                }
                v
            }
            DeLaPagina::Vista {
                cuerpo,
                titulos,
                px,
                solo_esta,
            } => {
                let v = vista::Vista {
                    cuerpo,
                    titulos,
                    px: px.clamp(vista::TAMANO_MINIMO, vista::TAMANO_MAXIMO),
                };
                self.guardar_vista(&e, &v, solo_esta);
                Vec::new()
            }
            DeLaPagina::Titulo { texto } => vec![OrdenNota::Titulo { texto }],
            DeLaPagina::AbrirEnlace { url } => {
                if es_enlace_de_fuera(&url) {
                    vec![OrdenNota::AbrirEnlace { url }]
                } else {
                    Vec::new()
                }
            }
            DeLaPagina::AbrirPixpin { ruta } => self.abrir_ruta(&e, &ruta),
            DeLaPagina::Adjuntar { id, nombre, datos } => {
                let md = de_base64(&datos).and_then(|b| {
                    adjuntos::adjuntar_bytes(&self.raiz(), &e.destino, &nombre, &b, ahora)
                        .ok()
                        .map(|ruta| renglon_de(&nombre, &ruta))
                });
                vec![mandar(
                    json!({"tipo": "adjuntado", "id": id, "md": md, "error": md.is_none()}),
                )]
            }
            DeLaPagina::Elegir { clase } => vec![OrdenNota::Elegir {
                imagenes: clase == "imagen",
            }],
            DeLaPagina::ElegirHoja { viva } => {
                vec![mandar(json!({"tipo": "hojas", "viva": viva, "grupos": []}))]
            }
            DeLaPagina::InsertarHoja { .. } => Vec::new(),
            DeLaPagina::ElegirMensaje => vec![mandar(self.mensajes(&e))],
            DeLaPagina::InsertarMensaje { clave } => self
                .bloque_de_mensaje(&e, &clave)
                .map(Self::insertar)
                .into_iter()
                .collect(),
            DeLaPagina::GuardarCopia { texto, titulo } => {
                let nueva = if self.ingles {
                    "New note"
                } else {
                    "Nota nueva"
                };
                vec![OrdenNota::GuardarComo {
                    nombre: nombre_de_fichero(&titulo, nueva),
                    extension: "md".into(),
                    datos: texto.into_bytes(),
                }]
            }
            DeLaPagina::ExportarWord { texto, titulo } => {
                self.word(&e, &texto, &titulo).into_iter().collect()
            }
            DeLaPagina::ComentariosPedir { texto } => vec![mandar(e.comentarios.json(&texto))],
            DeLaPagina::ComentarioNuevo {
                texto,
                desde,
                hasta,
                cuerpo,
            } => {
                let id = e.comentarios.nuevo(&texto, desde, hasta, &cuerpo, ahora);
                let mut j = e.comentarios.json(&texto);
                j["nuevo"] = json!(id);
                vec![mandar(j)]
            }
            DeLaPagina::ComentarioResponder {
                texto,
                hilo,
                cuerpo,
            } => {
                e.comentarios.responder(&hilo, &cuerpo, ahora);
                vec![mandar(e.comentarios.json(&texto))]
            }
            DeLaPagina::ComentarioEditar { texto, id, cuerpo } => {
                e.comentarios.editar(&id, &cuerpo, ahora);
                vec![mandar(e.comentarios.json(&texto))]
            }
            DeLaPagina::ComentarioBorrar { texto, id } => {
                e.comentarios.borrar(&id);
                vec![mandar(e.comentarios.json(&texto))]
            }
            DeLaPagina::ComentarioResolver { texto, hilo, si } => {
                e.comentarios.resolver(&hilo, si, ahora);
                vec![mandar(e.comentarios.json(&texto))]
            }
            // El borde lo estira el propio marco del Mac.
            DeLaPagina::Ventana { accion, .. } if accion == "estirar" => Vec::new(),
            DeLaPagina::Ventana { accion, .. } => vec![OrdenNota::Ventana { accion }],
            DeLaPagina::Compartir => {
                let texto = leer_texto(&self.raiz(), &e.destino)
                    .unwrap_or_else(|| e.ultimo_conocido.clone());
                let nueva = if self.ingles {
                    "New note"
                } else {
                    "Nota nueva"
                };
                let titulo = match md_vivo::titulo(&texto) {
                    t if t.trim().is_empty() => nueva.to_string(),
                    t => t,
                };
                vec![OrdenNota::Compartir { titulo, texto }]
            }
            DeLaPagina::Cerrar => vec![mandar(json!({"tipo": "cerrando"}))],
            DeLaPagina::CerrarListo => vec![OrdenNota::Cerrar],
            DeLaPagina::EditorAnterior => vec![OrdenNota::EditorAnterior],
        }
    }

    /// Copia junto a la nota los ficheros elegidos con el panel del Mac y
    /// da el JSON que los mete en ella (`None` si no entro ninguno).
    pub fn adjuntar_ficheros(&self, rutas: Vec<String>) -> Option<String> {
        let e = self.estado();
        let raiz = self.raiz();
        let renglones: Vec<String> = rutas
            .iter()
            .filter_map(|r| {
                let p = Path::new(r);
                let ruta = adjuntos::adjuntar(&raiz, &e.destino, p, crate::ahora_ms()).ok()?;
                let nombre = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Some(renglon_de(&nombre, &ruta))
            })
            .collect();
        (!renglones.is_empty()).then(|| {
            json!({"tipo": "insertar", "bloque": true, "texto": renglones.join("\n")}).to_string()
        })
    }

    /// **Lo que la pagina pide bajo `pixpin-nota://nota/`**: las fotos,
    /// audios y videos de la nota, por la misma ruta con que van en el
    /// Markdown (`archivo?r=<ruta>`).
    pub fn servir(&self, peticion: String) -> Option<RecursoNota> {
        let resto = peticion.strip_prefix(ORIGEN).unwrap_or(&peticion);
        if !resto.starts_with("archivo?") {
            return None;
        }
        let ruta = parametro(resto, "r")?;
        let e = self.estado();
        let fichero = adjuntos::resolver(&self.raiz(), &e.destino, &ruta)?;
        drop(e);
        let datos = std::fs::read(&fichero).ok()?;
        Some(RecursoNota {
            tipo: tipo_de(&fichero).to_string(),
            datos,
        })
    }

    /// **Lo que llego de fuera** (otro aparato, otra ventana): el JSON
    /// `texto_externo` si la nota cambio en el disco desde lo ultimo que se
    /// vio. La pagina lo pone solo si no tiene nada sin guardar. Para
    /// llamarlo cada 2 s.
    pub fn mirar_fuera(&self) -> Option<String> {
        let mut e = self.estado();
        let raiz = self.raiz();
        let ahora = huella(&raiz, &e.destino);
        if ahora == e.firma {
            return None;
        }
        e.firma = ahora;
        let t = leer_texto(&raiz, &e.destino)?;
        if t == e.ultimo_conocido {
            return None;
        }
        e.ultimo_conocido = t.clone();
        Some(json!({"tipo": "texto_externo", "texto": t}).to_string())
    }
}

impl SesionNota {
    /// El JSON con los comentarios (pruebas).
    #[cfg(test)]
    fn abiertos(&self) -> usize {
        let e = self.estado();
        let ruta = comentarios::ruta(&self.raiz(), &e.destino, &e.codigo);
        ruta.and_then(|r| std::fs::read_to_string(r).ok())
            .and_then(|t| pixpin_docs::md_comentarios::leer(&t).ok())
            .map(|c| c.abiertos())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod pruebas;
