//! **Conexiones del asistente con servicios de archivos** (17-oct-2026).
//!
//! Pedido del usuario: «que se agregue conexion de Google Drive y otros a
//! traves del agente de IA». El asistente de la nube puede, con permiso del
//! usuario, buscar en su Drive, leer el texto de un Doc, una hoja o un PDF,
//! bajar un archivo al chat y subir uno del chat (esto ultimo preguntando
//! antes, como todo lo que cambia algo).
//!
//! ## Un componente con variables
//!
//! Nada aqui es «de Drive» salvo [`drive`]:
//!
//! - [`Servicio`]: las variables de un servicio (nombre, direcciones de
//!   OAuth y de su API, alcances, parametros extra). Anadir Dropbox o
//!   OneDrive es una constante mas en [`SERVICIOS`]...
//! - [`Almacen`]: lo que se le pide a cualquier servicio de archivos
//!   (buscar, ver uno, leer su texto, bajarlo, subir). ...y su `impl`, con
//!   una linea en [`Conexion::almacen`].
//! - [`oauth`]: el permiso (PKCE + vuelta a `127.0.0.1`), igual para todos.
//! - [`guardado`]: el ID de cliente y los tokens, cifrados con DPAPI.
//! - [`Conexion`]: un servicio conectado; pone el token en cada peticion y
//!   lo renueva solo cuando caduca (o si el servicio dice 401).
//! - [`acciones`]: las herramientas del asistente, filas de la misma tabla
//!   que el resto de la app, con un parametro `servicio`.
//!
//! Las direcciones van en [`Extremos`] (sacados del [`Servicio`]) para que
//! las pruebas apunten todo a un servidor falso en 127.0.0.1, sin red.

pub mod acciones;
pub mod calendario;
pub mod drive;
pub mod guardado;
pub mod oauth;
pub mod url;

#[cfg(test)]
mod pruebas;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;

use crate::nube::http::{self, Fallo, Respuesta};
use guardado::{Guardado, Tokens};
use oauth::FalloToken;

/// **Un servicio que se puede conectar.** Todas sus diferencias son datos.
#[derive(Debug, Clone, Copy)]
pub struct Servicio {
    /// Como se guarda y como lo nombra el modelo (`google_drive`).
    pub id: &'static str,
    /// El nombre que ve el usuario.
    pub nombre: &'static str,
    pub autorizar: &'static str,
    pub token: &'static str,
    /// Vacio si el servicio no tiene donde retirar el permiso.
    pub revocar: &'static str,
    /// La raiz de su API de archivos.
    pub api: &'static str,
    /// La raiz de sus subidas (Google las tiene en otra ruta).
    pub subida: &'static str,
    pub alcances: &'static [&'static str],
    /// Parametros de mas en la URL de permiso.
    pub extra_autorizar: &'static [(&'static str, &'static str)],
    /// Donde crea el usuario su ID de cliente.
    pub consola: &'static str,
}

/// Las direcciones de un servicio, cambiables (las pruebas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extremos {
    pub autorizar: String,
    pub token: String,
    pub revocar: String,
    pub api: String,
    pub subida: String,
}

impl Servicio {
    pub fn extremos(&self) -> Extremos {
        Extremos {
            autorizar: self.autorizar.into(),
            token: self.token.into(),
            revocar: self.revocar.into(),
            api: self.api.into(),
            subida: self.subida.into(),
        }
    }

    /// Todos hacia otro servidor (`http://127.0.0.1:puerto`), con las
    /// mismas rutas: el servidor falso de las pruebas.
    pub fn extremos_en(&self, base: &str) -> Extremos {
        let mover = |u: &str| match u.split_once("://").and_then(|(_, r)| r.find('/').map(|i| &r[i..])) {
            Some(ruta) => format!("{base}{ruta}"),
            None if u.is_empty() => String::new(),
            None => base.to_string(),
        };
        Extremos {
            autorizar: mover(self.autorizar),
            token: mover(self.token),
            revocar: mover(self.revocar),
            api: mover(self.api),
            subida: mover(self.subida),
        }
    }
}

/// **Los servicios que hay**: Google Drive (archivos) y Google Calendar
/// (solo ver, 18-oct).
pub const SERVICIOS: &[Servicio] = &[drive::GOOGLE_DRIVE, calendario::GOOGLE_CALENDAR];

pub fn por_id(id: &str) -> Option<&'static Servicio> {
    let id = id.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    SERVICIOS.iter().find(|s| {
        s.id == id
            || (s.id == "google_drive" && (id == "drive" || id == "google"))
            || (s.id == "google_calendar" && matches!(id.as_str(), "calendar" | "calendario" | "google_calendario"))
    })
}

/// Un archivo (o carpeta) de un servicio, lo que se le cuenta al modelo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Archivo {
    pub id: String,
    pub nombre: String,
    /// Tipo MIME del servicio.
    pub tipo: String,
    pub tamano: Option<u64>,
    /// `2026-10-12T15:04:05.000Z`, o vacio.
    pub modificado: String,
    /// Para abrirlo en el navegador.
    pub enlace: String,
    pub carpeta: bool,
}

/// Lo que sale de leer un archivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Contenido {
    /// Ya es texto (un Doc exportado, una hoja en CSV, un .txt).
    Texto(String),
    /// Hay que sacarle el texto aqui (un PDF, un Word): el lector de la app.
    Fichero { nombre: String, bytes: Vec<u8> },
}

/// Los tipos que se pueden pedir al buscar.
pub const TIPOS: &[&str] = &["cualquiera", "documento", "hoja", "presentacion", "pdf", "imagen", "carpeta"];

/// **Lo que se le pide a un servicio de archivos.** Errores ya dichos para
/// el modelo (que y por que).
pub trait Almacen {
    /// Lo que casa con `texto` (en el nombre o dentro); vacio: lo ultimo
    /// tocado. `tipo`: uno de [`TIPOS`].
    fn buscar(&self, texto: &str, tipo: &str, maximo: usize) -> Result<Vec<Archivo>, String>;
    fn archivo(&self, id: &str) -> Result<Archivo, String>;
    fn leer(&self, a: &Archivo) -> Result<Contenido, String>;
    /// El archivo entero, con el nombre con el que se guarda (un Doc de
    /// Google sale como .docx).
    fn bajar(&self, a: &Archivo) -> Result<(String, Vec<u8>), String>;
    fn subir(&self, nombre: &str, bytes: &[u8], carpeta: Option<&str>) -> Result<Archivo, String>;
    /// El correo de la cuenta.
    fn cuenta(&self) -> Result<String, String>;
}

/// Lo mas que se baja o se sube de una vez (todo pasa por la memoria).
pub const LIMITE_ARCHIVO: usize = 100 * 1024 * 1024;

/// **Un servicio conectado**: lo guardado, con el token al dia.
pub struct Conexion {
    pub servicio: &'static Servicio,
    pub extremos: Extremos,
    carpeta: PathBuf,
    estado: Mutex<Guardado>,
}

/// Lo que se dice si no esta conectado.
pub fn sin_conectar(s: &Servicio) -> String {
    format!(
        "{} no esta conectado: el usuario tiene que conectarlo en Ajustes → Asistente → Conexiones (pegar su ID de cliente y pulsar Conectar).",
        s.nombre
    )
}

impl Conexion {
    /// La de siempre (`%LOCALAPPDATA%\PixPinMax\claves`).
    pub fn abrir(s: &'static Servicio) -> Result<Conexion, String> {
        let carpeta = guardado::carpeta().ok_or("no se encuentra LOCALAPPDATA")?;
        Conexion::abrir_en(s, s.extremos(), &carpeta)
    }

    /// Con otras direcciones y otra carpeta (las pruebas).
    pub fn abrir_en(s: &'static Servicio, extremos: Extremos, carpeta: &Path) -> Result<Conexion, String> {
        let g = guardado::leer_de(carpeta, s);
        if !g.conectado() || g.cliente.is_none() {
            return Err(sin_conectar(s));
        }
        Ok(Conexion { servicio: s, extremos, carpeta: carpeta.to_path_buf(), estado: Mutex::new(g) })
    }

    fn guardar(&self, g: &Guardado) {
        if let Err(e) = guardado::guardar_en(&self.carpeta, self.servicio, g) {
            tracing::warn!("conexiones: no se pudo guardar {}: {e}", self.servicio.nombre);
        }
    }

    /// **El token de acceso**, renovado si caduca en menos de un minuto (o
    /// si `forzar`). Un permiso retirado borra los tokens (Ajustes ya dira
    /// «sin conectar»).
    pub fn token(&self, forzar: bool) -> Result<String, String> {
        let mut g = self.estado.lock().unwrap_or_else(|e| e.into_inner());
        let t = g.tokens.clone().unwrap_or_default();
        if !forzar && !t.acceso.is_empty() && t.caduca_ms > oauth::ahora_ms() + 60_000 {
            return Ok(t.acceso);
        }
        let cliente = g.cliente.clone().unwrap_or_default();
        match oauth::refrescar(&self.extremos, &cliente, &t.refresco) {
            Ok(nuevo) => {
                let acceso = nuevo.acceso.clone();
                g.tokens = Some(nuevo);
                self.guardar(&g);
                Ok(acceso)
            }
            Err(FalloToken::Revocado(m)) => {
                g.tokens = None;
                g.cuenta = None;
                self.guardar(&g);
                Err(format!("{}: {m} (Ajustes → Asistente → Conexiones → Conectar).", self.servicio.nombre))
            }
            Err(FalloToken::Otro(m)) => Err(format!("{}: {m}", self.servicio.nombre)),
        }
    }

    /// **Una peticion a la API** con el token; si contesta 401 se renueva
    /// el token y se repite una vez. Los fallos, ya dichos.
    pub fn pedir(
        &self,
        metodo: &str,
        url: &str,
        cabeceras: &[(&'static str, String)],
        cuerpo: &[u8],
        limite: usize,
    ) -> Result<Respuesta, String> {
        let mut forzar = false;
        loop {
            let token = self.token(forzar)?;
            let mut todas = vec![("Authorization", format!("Bearer {token}"))];
            todas.extend(cabeceras.iter().cloned());
            match http::pedir(metodo, url, &todas, cuerpo, limite) {
                Err(Fallo::Http { codigo: 401, .. }) if !forzar => forzar = true,
                Ok(r) => return Ok(r),
                Err(f) => return Err(self.decir(f)),
            }
        }
    }

    fn decir(&self, f: Fallo) -> String {
        let n = self.servicio.nombre;
        match f {
            Fallo::Red(m) => format!("sin conexion con {n}: {m}"),
            Fallo::Http { codigo, cuerpo, .. } => {
                let v: Value = serde_json::from_str(&cuerpo).unwrap_or(Value::Null);
                let detalle = v
                    .pointer("/error/message")
                    .or_else(|| v.get("error_description"))
                    .or_else(|| v.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                match codigo {
                    401 => format!("{n} rechazo el permiso; hay que volver a conectar en Ajustes → Asistente → Conexiones"),
                    403 => format!("{n} no deja hacer eso ({detalle})"),
                    404 => format!("ese archivo no existe en {n} o no tienes acceso"),
                    429 => format!("{n} pide esperar un poco (demasiadas peticiones)"),
                    c => format!("{n} contesto {c}{}", if detalle.is_empty() { String::new() } else { format!(": {detalle}") }),
                }
            }
        }
    }

    /// **El almacen de este servicio.** Con un servicio nuevo, esto pasa a
    /// ser un `match` por `self.servicio.id` con una linea por servicio.
    pub fn almacen(&self) -> Box<dyn Almacen + '_> {
        debug_assert_eq!(self.servicio.id, drive::GOOGLE_DRIVE.id);
        Box::new(drive::Drive { cx: self })
    }

    /// Apunta la cuenta (para Ajustes).
    pub fn apuntar_cuenta(&self, cuenta: &str) {
        let mut g = self.estado.lock().unwrap_or_else(|e| e.into_inner());
        g.cuenta = Some(cuenta.to_string());
        self.guardar(&g);
    }
}

/// **Conecta** un servicio: el navegador, la vuelta, los tokens (cifrados)
/// y la cuenta. Bloquea: desde un hilo de fondo.
pub fn conectar_en(
    s: &'static Servicio,
    extremos: &Extremos,
    carpeta: &Path,
    abrir: &dyn Fn(&str) -> Result<(), String>,
    parar: &std::sync::atomic::AtomicBool,
    espera: std::time::Duration,
) -> Result<String, String> {
    let mut g = guardado::leer_de(carpeta, s);
    let cliente = g
        .cliente
        .clone()
        .ok_or_else(|| format!("falta el ID de cliente de {}: pegalo arriba (se crea en {})", s.nombre, s.consola))?;
    let tokens: Tokens = oauth::autorizar(s, extremos, &cliente, abrir, parar, espera)?;
    g.tokens = Some(tokens);
    g.cuenta = None;
    guardado::guardar_en(carpeta, s, &g)?;
    let cx = Conexion::abrir_en(s, extremos.clone(), carpeta)?;
    // La cuenta es para ensenarla: si no sale, se conecto igual.
    let cuenta = match s.id {
        "google_calendar" => calendario::cuenta(&cx),
        _ => cx.almacen().cuenta(),
    }
    .unwrap_or_default();
    if !cuenta.is_empty() {
        cx.apuntar_cuenta(&cuenta);
    }
    Ok(cuenta)
}

/// **Desconecta**: borra los tokens de este PC (el ID de cliente se queda)
/// y retira el permiso en el servicio. Lo segundo puede fallar sin red: lo
/// importante ya esta hecho.
pub fn desconectar_en(s: &'static Servicio, extremos: &Extremos, carpeta: &Path) -> Result<(), String> {
    let mut g = guardado::leer_de(carpeta, s);
    let tokens = g.tokens.take();
    g.cuenta = None;
    guardado::guardar_en(carpeta, s, &g)?;
    if let Some(t) = tokens {
        let token = if t.refresco.is_empty() { t.acceso } else { t.refresco };
        if let Err(e) = oauth::revocar(extremos, &token) {
            tracing::info!("conexiones: no se pudo retirar el permiso de {} en el servicio: {e}", s.nombre);
        }
    }
    Ok(())
}

/// Guarda el ID de cliente pegado (sin tocar los tokens; si el ID cambia,
/// los tokens del anterior ya no valen y se quitan).
pub fn guardar_cliente_en(s: &'static Servicio, carpeta: &Path, pegado: &str) -> Result<guardado::Cliente, String> {
    let cliente = guardado::leer_cliente(pegado).ok_or_else(|| {
        "eso no parece un ID de cliente (algo como 1234-abc.apps.googleusercontent.com, o el JSON que da Google)".to_string()
    })?;
    let mut g = guardado::leer_de(carpeta, s);
    if g.cliente.as_ref().is_some_and(|c| c.id != cliente.id) {
        g.tokens = None;
        g.cuenta = None;
    }
    g.cliente = Some(cliente.clone());
    guardado::guardar_en(carpeta, s, &g)?;
    // (El mismo ID de cliente de Google vale para Drive y Calendar: a los
    // otros servicios de la misma cuenta que aun no tienen, se les pone.)
    for otro in SERVICIOS.iter().filter(|o| o.id != s.id && o.autorizar == s.autorizar) {
        let mut go = guardado::leer_de(carpeta, otro);
        if go.cliente.is_none() {
            go.cliente = Some(cliente.clone());
            let _ = guardado::guardar_en(carpeta, otro, &go);
        }
    }
    Ok(cliente)
}

/// «12,3 MB», «850 KB».
pub fn tamano(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)).replace('.', ",")
    } else {
        format!("{} KB", bytes.div_ceil(1024).max(1))
    }
}

/// Una linea por archivo, para el modelo.
pub fn lista_para_el_modelo(s: &Servicio, archivos: &[Archivo]) -> String {
    if archivos.is_empty() {
        return format!("No hay nada asi en {}.", s.nombre);
    }
    let mut out = format!("{} en {}:\n", archivos.len(), s.nombre);
    for a in archivos {
        let mut datos = vec![drive::tipo_leible(&a.tipo).to_string()];
        if let Some(t) = a.tamano {
            datos.push(tamano(t));
        }
        if a.modificado.len() >= 10 {
            datos.push(format!("modificado {}", &a.modificado[..10]));
        }
        out.push_str(&format!("- «{}» ({}) id: {}\n", a.nombre, datos.join(", "), a.id));
    }
    out.push_str("Para leer, bajar o subir a una carpeta usa su id.");
    out
}
