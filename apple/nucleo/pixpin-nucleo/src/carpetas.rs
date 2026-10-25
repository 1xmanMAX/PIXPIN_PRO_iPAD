//! **Las carpetas de paginas del chat** (26-oct-2026 en Windows): lo que la
//! tarjeta del chat, su ventana y su PDF (`PixPinKit/Chat/Carpetas.swift`)
//! le piden al nucleo. Es la parte que no es Win32 de
//! `ventana_chat/carpetas.rs` y `compartir/carpeta.rs` de Windows.
//!
//! La carpeta es un mensaje `MINIAPP` «carpeta» con la lista ordenada de sus
//! paginas en el `texto` (`pixpin_proyecto::carpeta_de_paginas`, el mismo
//! crate que Windows): una creada en el PC se ve y se edita aqui igual, y al
//! reves. Las paginas siguen siendo mensajes normales del chat.
//!
//! - **crear** una carpeta (vacia, desde el clip; o con una pagina dentro,
//!   «Nueva carpeta con esta página»), con un nombre que no se repite
//!   («Cuaderno», «Cuaderno 2»…) y un `id` que no es el de otro mensaje;
//! - **leerla** con sus paginas vivas, en orden ([`Nucleo::carpeta_de_paginas`]);
//! - **añadirle** una hoja que ya estaba en el chat, con el titulo que le
//!   toca («Día 3» tras «Día 2»);
//! - **cambiarla**: reordenar, renombrar una pagina, quitarla;
//! - **la pagina siguiente** («Nueva página»): un lienzo nuevo con la
//!   plantilla de la ultima pagina, vacia, y el titulo siguiente;
//! - **las hojas de un lienzo para su PDF**: de la plantilla, si no de los
//!   marcos, si no todo (`exportar::hojas_de_cuaderno`).
//!
//! Lo del asistente (`anadir_a_carpeta`, `pagina_nueva_de_carpeta`) usa
//! [`anadir_por_nombre`] y [`pagina_siguiente`], sin ventana.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pixpin_motor2d::exportar::{self as ex};
use pixpin_motor2d::exportar_svg::{self, Incrustada, OpcionesSvg};
use pixpin_motor2d::{Escena, excalidraw};
use pixpin_proyecto::almacen;
use pixpin_proyecto::carpeta_de_paginas::{self as carpetas, Carpeta};
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje, Sello};

use crate::chats::validar_chat;
use crate::compartir::HojaDeDibujo;
use crate::editor_exportar::{empaquetar_hoja, mime_de};
use crate::{ErrorNucleo, MensajeChat, Nucleo};

/// El nombre con el que nace una carpeta (`carpeta-nombre-de-serie`).
pub const NOMBRE_DE_SERIE: &str = "Cuaderno";
/// El titulo de serie de una pagina sin serie («Página 4»,
/// `carpeta-pagina`).
pub const PAGINA_DE_SERIE: &str = "Página";

/// Una pagina de la carpeta, ya con su mensaje.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PaginaDeCarpeta {
    /// Su titulo en la carpeta, o el nombre del mensaje si no lleva.
    pub titulo: String,
    pub mensaje: MensajeChat,
}

/// **Una carpeta leida**: su nombre y sus paginas vivas, en orden.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CarpetaDePaginas {
    /// El `id` del mensaje de la carpeta.
    pub id: String,
    pub nombre: String,
    pub paginas: Vec<PaginaDeCarpeta>,
    /// El mensaje de la carpeta como esta en el disco (para la tarjeta).
    pub mensaje: MensajeChat,
}

/// Una carpeta del chat para el menu «Añadir a carpeta…».
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CarpetaDelChat {
    pub id: String,
    pub nombre: String,
    /// Si la pagina por la que se pregunta ya esta dentro.
    pub la_lleva: bool,
}

/// Lo que deja «Añadir a carpeta…».
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AnadidaACarpeta {
    /// El mensaje de la carpeta como quedo.
    pub carpeta: MensajeChat,
    /// El nombre de la carpeta, para el aviso.
    pub nombre: String,
    /// El titulo con el que entro la pagina (o el que ya tenia).
    pub titulo: String,
    pub paginas: u32,
    /// Si no habia ninguna carpeta con ese nombre y se creo (el asistente).
    pub creada: bool,
    /// Si ya estaba: entonces no se toco nada.
    pub ya_estaba: bool,
}

/// Lo que deja «Nueva página».
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PaginaNuevaDeCarpeta {
    /// El lienzo nuevo, ya en el chat y en la carpeta.
    pub lienzo: MensajeChat,
    /// La carpeta como quedo.
    pub carpeta: MensajeChat,
    pub titulo: String,
    /// Si lleva la plantilla de la ultima pagina (si no, en blanco).
    pub con_plantilla: bool,
}

/// Lo que se le cambia a una carpeta desde su ventana. Las posiciones son
/// las de sus paginas vivas (las que se ven).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum CambioCarpeta {
    /// Lleva la pagina `desde` al sitio `hasta` (arrastrar, ← →).
    Mover { desde: u32, hasta: u32 },
    /// Otro titulo para la pagina `pagina` (el `id` de su mensaje).
    Renombrar { pagina: String, titulo: String },
    /// Quita la pagina de la carpeta; el mensaje sigue en el chat.
    Quitar { pagina: String },
}

/// Si el mensaje es una carpeta de paginas (para Swift, que solo ve la
/// clase y la palabra).
#[uniffi::export]
pub fn es_carpeta_de_paginas(clase: String, miniapp: Option<String>) -> bool {
    clase == "MINIAPP" && miniapp.as_deref() == Some(carpetas::MINIAPP)
}

/// Los `id` de las paginas de una carpeta, en orden, de su `texto` (la
/// tarjeta del chat). Vacio si no se entiende.
#[uniffi::export]
pub fn paginas_de_carpeta(texto: String) -> Vec<String> {
    Carpeta::leer(&texto)
        .map(|c| c.paginas.into_iter().map(|p| p.id).collect())
        .unwrap_or_default()
}

/// El nombre del PDF de una carpeta (`compartir::carpeta::nombre_de_fichero`
/// de Windows): el de la carpeta y, sin el, «Carpeta.pdf».
#[uniffi::export]
pub fn nombre_del_pdf_de_carpeta(titulo: String) -> String {
    if titulo.trim().is_empty() {
        return "Carpeta.pdf".into();
    }
    let base = crate::compartir::nombre_de_fichero_compartido(titulo);
    if base.trim().is_empty() {
        "Carpeta.pdf".into()
    } else {
        format!("{base}.pdf")
    }
}

/// Si un mensaje se ofrece para «Añadir a carpeta…» (`carpetas::se_ofrece`
/// de Windows): lo que puede ser una pagina y se ve en el chat.
pub fn se_ofrece(m: &Mensaje) -> bool {
    carpetas::puede_ir(m)
}

/// Lo que se ensena de una pagina sin titulo: su nombre, «Lienzo» en un
/// lienzo sin el, o lo primero de su texto (`nombre_de_la_fila`).
pub fn nombre_visible(m: &Mensaje) -> String {
    let n = m.nombre.trim();
    if m.clase == Some(Clase::Dibujo) && (n.is_empty() || n.ends_with(".excalidraw")) {
        return "Lienzo".into();
    }
    if !n.is_empty() {
        return n.to_string();
    }
    let r = m.resumen();
    let primera = r.lines().next().unwrap_or("").trim();
    if primera.is_empty() {
        return "Sin nombre".into();
    }
    let mut v: String = primera.chars().take(48).collect();
    if primera.chars().count() > 48 {
        v.push('…');
    }
    v
}

/// El nombre con el que entra una hoja (`nombre_de_hoja`): el suyo si se lo
/// pusieron; «Lienzo» o `<id>.excalidraw` no dicen nada en un cuaderno.
fn nombre_de_hoja(m: &Mensaje) -> String {
    let n = nombre_visible(m);
    if n == "Lienzo" || n.ends_with(".excalidraw") || n == "Sin nombre" {
        String::new()
    } else {
        pixpin_docs::sin_extension(&n)
    }
}

/// Un nombre que no repite el de otra carpeta: «Cuaderno», «Cuaderno 2»…
pub fn nombre_libre(base: &str, mensajes: &[Mensaje]) -> String {
    let usados: Vec<&str> = carpetas::carpetas_de(mensajes)
        .into_iter()
        .map(|i| mensajes[i].nombre.trim())
        .collect();
    if !usados.contains(&base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|n| !usados.contains(&n.as_str()))
        .unwrap_or_else(|| base.to_string())
}

/// Minusculas y sin tildes, para buscar una carpeta por su nombre.
fn plegar(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

/// **La carpeta del chat que se llama `nombre`**: igual sin tildes ni
/// mayusculas, o si no la que lo lleva dentro. La mas reciente si hay dos.
pub fn carpeta_por_nombre(mensajes: &[Mensaje], nombre: &str) -> Option<usize> {
    let buscado = plegar(nombre.trim());
    if buscado.is_empty() {
        return None;
    }
    let todas = carpetas::carpetas_de(mensajes);
    let nombre_de = |i: &usize| plegar(mensajes[*i].nombre.trim());
    todas
        .iter()
        .rev()
        .find(|i| nombre_de(i) == buscado)
        .or_else(|| todas.iter().rev().find(|i| nombre_de(i).contains(&buscado)))
        .copied()
}

/// Un `id` (la hora) que no es el de ningun mensaje: el lienzo recien hecho
/// puede ser del mismo milisegundo y la carpeta se confundiria con el.
fn hora_libre(mensajes: &[Mensaje]) -> i64 {
    let mut cuando = crate::chats::hora_unica();
    while mensajes.iter().any(|m| m.id == cuando.to_string()) {
        cuando += 1;
    }
    cuando
}

/// El cuaderno de un chat, del mas viejo al mas nuevo.
fn mensajes_de(dir: &Path) -> Result<Vec<Mensaje>, ErrorNucleo> {
    let mut v = cuaderno::Cuaderno::leer_de(dir)?.mensajes;
    v.sort_by_key(|m| m.cuando);
    Ok(v)
}

fn no_existe(que: &str) -> ErrorNucleo {
    ErrorNucleo::NoExiste { que: que.into() }
}

impl Nucleo {
    fn dir_de(&self, chat: &str) -> Result<PathBuf, ErrorNucleo> {
        validar_chat(chat)?;
        let indice = almacen::Indice::leer_si_esta(self.ruta())?;
        if indice.buscar(chat).is_none() {
            return Err(no_existe(chat));
        }
        Ok(almacen::carpeta(self.ruta(), chat))
    }

    /// Cambia la carpeta en el disco con el cerrojo y suelta el indice del
    /// cuaderno de la cache.
    fn cambiar_en_disco(
        &self,
        dir: &Path,
        id: &str,
        cambio: impl FnOnce(&mut Carpeta) -> bool,
    ) -> Result<Option<Mensaje>, ErrorNucleo> {
        let hecha = carpetas::cambiar_en_disco(dir, id, cambio)?;
        self.cache_tomada().quitar(&dir.join("guardados.jsonl"));
        if hecha.is_none() {
            return Ok(None);
        }
        Ok(mensajes_de(dir)?.into_iter().find(|m| m.id == id))
    }

    /// Escribe un mensaje nuevo al final y sube el chat en la lista con
    /// `resumen`.
    fn anadir_carpeta_al_chat(
        &self,
        chat: &str,
        dir: &Path,
        m: &Mensaje,
        resumen: &str,
    ) -> Result<(), ErrorNucleo> {
        cuaderno::anadir(dir, m)?;
        self.cache_tomada().quitar(&dir.join("guardados.jsonl"));
        let mut indice = almacen::Indice::leer_si_esta(self.ruta())?;
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == chat) {
            f.tocado = m.cuando;
            f.resumen = resumen.to_string();
        }
        indice.guardar(self.ruta())?;
        Ok(())
    }

    /// Una carpeta nueva al final del chat, con `nombre` (o el de serie que
    /// no se repita) y `primera` dentro si se da y puede ir.
    pub(crate) fn carpeta_nueva(
        &self,
        chat: &str,
        nombre: Option<&str>,
        primera: Option<&str>,
    ) -> Result<Mensaje, ErrorNucleo> {
        let dir = self.dir_de(chat)?;
        let mensajes = mensajes_de(&dir)?;
        let nombre = match nombre.map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => n.to_string(),
            None => nombre_libre(NOMBRE_DE_SERIE, &mensajes),
        };
        let numero = {
            let mut cache = self.cache_tomada();
            cache
                .indice(&dir.join("guardados.jsonl"))?
                .map_or(0, |i| i.mayor_numero)
                + 1
        };
        let mut m = carpetas::mensaje_nuevo(
            &nombre,
            &Sello {
                cuando: hora_libre(&mensajes),
                numero,
                aparato: self.identidad()?.yo.codigo(),
                proyecto: chat.to_string(),
            },
        );
        if let Some(p) = primera
            .and_then(|id| mensajes.iter().find(|x| x.id == id))
            .filter(|p| se_ofrece(p))
        {
            let mut c = Carpeta::default();
            let titulo = c.titulo_al_anadir(&nombre_de_hoja(p), PAGINA_DE_SERIE);
            c.anadir(&p.id, &titulo);
            m.texto = c.escribir();
        }
        self.anadir_carpeta_al_chat(chat, &dir, &m, &m.nombre.clone())?;
        Ok(m)
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Una carpeta de paginas nueva** al final del chat («Carpeta de
    /// páginas» del clip; con `primera`, «Nueva carpeta con esta página»).
    /// Se llama «Cuaderno» (o «Cuaderno 2»…): quien llama ofrece cambiarle
    /// el nombre enseguida, como Windows. Bloqueante.
    pub fn crear_carpeta_de_paginas(
        &self,
        chat: String,
        primera: Option<String>,
    ) -> Result<MensajeChat, ErrorNucleo> {
        self.carpeta_nueva(&chat, None, primera.as_deref())
            .map(MensajeChat::from)
    }

    /// **Una carpeta con sus paginas vivas**, en orden. `None` si el
    /// mensaje ya no esta o no es una carpeta que se entienda. No escribe:
    /// lo borrado se salta aqui y se poda al cambiarla. Bloqueante.
    pub fn carpeta_de_paginas(
        &self,
        chat: String,
        id: String,
    ) -> Result<Option<CarpetaDePaginas>, ErrorNucleo> {
        let dir = self.dir_de(&chat)?;
        let mensajes = mensajes_de(&dir)?;
        let Some(m) = mensajes.iter().find(|m| m.id == id) else {
            return Ok(None);
        };
        let Some(c) = Carpeta::de_mensaje(m) else {
            return Ok(None);
        };
        let paginas = c
            .vivas(&mensajes)
            .into_iter()
            .map(|(i, p)| PaginaDeCarpeta {
                titulo: if p.titulo.trim().is_empty() {
                    nombre_visible(&mensajes[i])
                } else {
                    p.titulo.clone()
                },
                mensaje: MensajeChat::from(mensajes[i].clone()),
            })
            .collect();
        Ok(Some(CarpetaDePaginas {
            id: m.id.clone(),
            nombre: nombre_visible(m),
            paginas,
            mensaje: MensajeChat::from(m.clone()),
        }))
    }

    /// **Las carpetas del chat** para «Añadir a carpeta…» de `pagina`, la
    /// mas vieja primero, con la marca de si ya la llevan.
    pub fn carpetas_del_chat(
        &self,
        chat: String,
        pagina: String,
    ) -> Result<Vec<CarpetaDelChat>, ErrorNucleo> {
        let dir = self.dir_de(&chat)?;
        let mensajes = mensajes_de(&dir)?;
        Ok(carpetas::carpetas_de(&mensajes)
            .into_iter()
            .map(|i| CarpetaDelChat {
                id: mensajes[i].id.clone(),
                nombre: nombre_visible(&mensajes[i]),
                la_lleva: Carpeta::de_mensaje(&mensajes[i]).is_some_and(|c| c.contiene(&pagina)),
            })
            .collect())
    }

    /// Si el mensaje `id` se ofrece para «Añadir a carpeta…».
    pub fn se_puede_anadir_a_carpeta(&self, chat: String, id: String) -> bool {
        let Ok(dir) = self.dir_de(&chat) else {
            return false;
        };
        mensajes_de(&dir)
            .ok()
            .and_then(|v| v.into_iter().find(|m| m.id == id))
            .is_some_and(|m| se_ofrece(&m))
    }

    /// **Mete el mensaje `pagina` en la carpeta `carpeta`**, al final y con
    /// el titulo que le toca («Día 3» si la carpeta lleva una serie; si no,
    /// el nombre de la hoja). Si ya estaba no se toca. `None` si la carpeta
    /// ya no esta. Bloqueante.
    pub fn anadir_a_carpeta(
        &self,
        chat: String,
        carpeta: String,
        pagina: String,
    ) -> Result<Option<AnadidaACarpeta>, ErrorNucleo> {
        let dir = self.dir_de(&chat)?;
        let mensajes = mensajes_de(&dir)?;
        let Some(m) = mensajes
            .iter()
            .find(|m| m.id == pagina)
            .filter(|m| se_ofrece(m))
        else {
            return Err(no_existe(&pagina));
        };
        let nombre = nombre_de_hoja(m);
        let (mut titulo, mut ya) = (String::new(), false);
        let hecha = self.cambiar_en_disco(&dir, &carpeta, |c| {
            if let Some(i) = c.posicion(&pagina) {
                ya = true;
                titulo = c.paginas[i].titulo.clone();
                return false;
            }
            titulo = c.titulo_al_anadir(&nombre, PAGINA_DE_SERIE);
            c.anadir(&pagina, &titulo)
        })?;
        Ok(hecha.map(|m| AnadidaACarpeta {
            nombre: nombre_visible(&m),
            paginas: Carpeta::de_mensaje(&m).map_or(0, |c| c.len() as u32),
            carpeta: MensajeChat::from(m),
            titulo,
            creada: false,
            ya_estaba: ya,
        }))
    }

    /// **Cambia una carpeta** desde su ventana (reordenar, renombrar una
    /// pagina, quitarla). Devuelve la carpeta como quedo, o `None` si ya no
    /// esta. Lo borrado del chat se poda de paso. Bloqueante.
    pub fn cambiar_carpeta(
        &self,
        chat: String,
        carpeta: String,
        cambio: CambioCarpeta,
    ) -> Result<Option<MensajeChat>, ErrorNucleo> {
        let dir = self.dir_de(&chat)?;
        let hecha = self.cambiar_en_disco(&dir, &carpeta, |c| match &cambio {
            CambioCarpeta::Mover { desde, hasta } => c.mover(*desde as usize, *hasta as usize),
            CambioCarpeta::Renombrar { pagina, titulo } => {
                !titulo.trim().is_empty()
                    && c.posicion(pagina).is_some_and(|i| c.renombrar(i, titulo))
            }
            CambioCarpeta::Quitar { pagina } => c.quitar(pagina),
        })?;
        Ok(hecha.map(MensajeChat::from))
    }

    /// **«Nueva página»**: un lienzo nuevo en el chat con la plantilla de la
    /// ultima pagina que sea un lienzo, vacia, o en blanco si no tiene; con
    /// el titulo siguiente («Día 3» tras «Día 2»), y al final de la
    /// carpeta. Bloqueante.
    pub fn pagina_siguiente_de_carpeta(
        &self,
        chat: String,
        carpeta: String,
    ) -> Result<PaginaNuevaDeCarpeta, ErrorNucleo> {
        pagina_siguiente(self, &chat, &carpeta)
    }

    /// **Las hojas de un lienzo para el PDF de una carpeta**
    /// (`exportar::hojas_de_cuaderno`): una por plantilla, en orden de
    /// lectura; sin plantilla, cada marco; sin marcos, todo. Con sus ordenes
    /// para pintarlas y su SVG. Vacio si no hay nada. Bloqueante.
    pub fn hojas_de_carpeta(
        &self,
        chat: String,
        dibujo: String,
    ) -> Result<Vec<HojaDeDibujo>, ErrorNucleo> {
        validar_chat(&chat)?;
        let lienzo = self.abrir_lienzo(chat.clone(), dibujo)?;
        let fotos: HashMap<u64, PathBuf> = self
            .imagenes_del_lienzo(chat, lienzo.clone())?
            .into_iter()
            .map(|i| (i.id_objeto, PathBuf::from(i.ruta)))
            .collect();
        Ok(hojas_de_cuaderno(lienzo.escena(), &fotos))
    }
}

/// Las hojas de una escena con la regla del cuaderno, listas para Swift.
pub(crate) fn hojas_de_cuaderno(
    escena: &Escena,
    fotos: &HashMap<u64, PathBuf>,
) -> Vec<HojaDeDibujo> {
    let incrustada = |id: u64| -> Option<Incrustada> {
        let bytes = std::fs::read(fotos.get(&id)?).ok()?;
        Some(Incrustada {
            mime: mime_de(&bytes)?,
            bytes,
        })
    };
    let fondo = ex::hex(escena.fondo);
    ex::hojas_de_cuaderno(escena, None)
        .into_iter()
        .map(|h| {
            let o = empaquetar_hoja(escena, &h);
            HojaDeDibujo {
                nombre: h.nombre.clone(),
                es_marco: false,
                x0: o.x0,
                y0: o.y0,
                x1: o.x1,
                y1: o.y1,
                plano: o.plano,
                svg: exportar_svg::svg(
                    &h,
                    OpcionesSvg {
                        fondo: Some(escena.fondo),
                        marcos: false,
                    },
                    &incrustada,
                ),
                fondo: fondo.clone(),
            }
        })
        .collect()
}

/// **La pagina siguiente de una carpeta**, sin ventana (la usan la ventana
/// y el asistente), como `carpetas::pagina_siguiente` de Windows.
pub fn pagina_siguiente(
    n: &Nucleo,
    chat: &str,
    carpeta: &str,
) -> Result<PaginaNuevaDeCarpeta, ErrorNucleo> {
    let dir = n.dir_de(chat)?;
    let mensajes = mensajes_de(&dir)?;
    let mut c = mensajes
        .iter()
        .find(|m| m.id == carpeta)
        .and_then(Carpeta::de_mensaje)
        .ok_or_else(|| no_existe(carpeta))?;
    c.podar(|id| mensajes.iter().any(|m| m.id == id));
    let raiz = n.ruta();
    let plantilla = c
        .vivas(&mensajes)
        .into_iter()
        .rev()
        .map(|(i, _)| &mensajes[i])
        .find(|m| m.clase == Some(Clase::Dibujo))
        .and_then(|m| m.referencia.clone())
        .and_then(|r| {
            let texto = std::fs::read_to_string(almacen::lienzo(raiz, chat, &r)).ok()?;
            let lienzo = excalidraw::leer(&texto).ok()?;
            pixpin_motor2d::hoja_de_plantilla::lienzo_siguiente(&excalidraw::a_escena(&lienzo))
        });
    let titulo = c.titulo_siguiente(PAGINA_DE_SERIE);
    let m = n.crear_lienzo_con(chat.to_string(), Some(titulo.clone()), |_| {})?;
    if let (Some(escena), Some(r)) = (&plantilla, m.referencia.as_deref()) {
        let lienzo = excalidraw::con_escena(&excalidraw::Lienzo::vacio(), escena);
        std::fs::write(
            almacen::lienzo(raiz, chat, r),
            excalidraw::escribir(&lienzo),
        )?;
    }
    let hecha = n
        .cambiar_en_disco(&dir, carpeta, |c| c.anadir(&m.id, &titulo))?
        .ok_or_else(|| no_existe(carpeta))?;
    Ok(PaginaNuevaDeCarpeta {
        lienzo: MensajeChat::from(m),
        carpeta: MensajeChat::from(hecha),
        titulo,
        con_plantilla: plantilla.is_some(),
    })
}

/// **Para el asistente**: mete el mensaje `pagina` en la carpeta `nombre`
/// de este chat (por su nombre, sin tildes ni mayusculas), al final y con
/// el titulo que le toca; si no hay ninguna con ese nombre, la crea
/// (`anadir_por_nombre` de Windows).
pub fn anadir_por_nombre(
    n: &Nucleo,
    chat: &str,
    nombre: &str,
    pagina: &str,
) -> Result<AnadidaACarpeta, ErrorNucleo> {
    let dir = n.dir_de(chat)?;
    let mensajes = mensajes_de(&dir)?;
    if !mensajes.iter().any(|m| m.id == pagina) {
        return Err(no_existe(pagina));
    }
    let (id, creada) = match carpeta_por_nombre(&mensajes, nombre) {
        Some(i) => (mensajes[i].id.clone(), false),
        None => (n.carpeta_nueva(chat, Some(nombre), None)?.id, true),
    };
    let mut hecha = n
        .anadir_a_carpeta(chat.to_string(), id.clone(), pagina.to_string())?
        .ok_or_else(|| no_existe(&id))?;
    hecha.creada = creada;
    Ok(hecha)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::Arc;

    fn nucleo(etiqueta: &str) -> (Arc<Nucleo>, PathBuf, String) {
        let d =
            std::env::temp_dir().join(format!("pixpin-carpetas-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        let chat = n.crear_chat("Diario".into()).unwrap();
        (n, d, chat.id)
    }

    fn diario() -> pixpin_motor2d::biblioteca::FiguraGuardada {
        pixpin_motor2d::figuras_de_serie::catalogo()
            .into_iter()
            .find(|f| f.id == "serie-diario")
            .unwrap()
    }

    /// Escribe en el lienzo `referencia` el diario de serie y algo encima.
    fn con_diario(d: &Path, chat: &str, referencia: &str) -> usize {
        let f = diario();
        let (w, h) = f.tamano();
        let mut escena = Escena::nueva();
        pixpin_motor2d::biblioteca::estampar_en_escena(
            &mut escena,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(w / 2.0, h / 2.0),
        );
        escena.anadir(pixpin_motor2d::Elemento {
            x: 100.0,
            y: 300.0,
            ancho: 50.0,
            alto: 20.0,
            figura: pixpin_motor2d::Figura::Rectangulo,
            ..Default::default()
        });
        // Algo lejos de la hoja: no sale en el PDF.
        escena.anadir(pixpin_motor2d::Elemento {
            x: 9000.0,
            y: 9000.0,
            ancho: 30.0,
            alto: 30.0,
            figura: pixpin_motor2d::Figura::Rectangulo,
            ..Default::default()
        });
        let l = excalidraw::con_escena(&excalidraw::Lienzo::vacio(), &escena);
        std::fs::write(
            almacen::lienzo(d, chat, referencia),
            excalidraw::escribir(&l),
        )
        .unwrap();
        f.elementos.iter().filter(|e| !e.borrado).count()
    }

    #[test]
    fn una_carpeta_nueva_lleva_la_pagina_y_su_nombre_no_se_repite() {
        let (n, _d, chat) = nucleo("crear");
        let lienzo = n.crear_lienzo(chat.clone(), None).unwrap();
        let c = n
            .crear_carpeta_de_paginas(chat.clone(), Some(lienzo.id.clone()))
            .unwrap();
        assert_eq!(c.clase, "MINIAPP");
        assert_eq!(c.miniapp.as_deref(), Some("carpeta"));
        assert!(es_carpeta_de_paginas(c.clase.clone(), c.miniapp.clone()));
        assert_eq!(c.nombre, "Cuaderno");
        assert_ne!(c.id, lienzo.id, "el id no es el del lienzo recien hecho");
        assert!(c.se_puede_renombrar, "se renombra como un lienzo");
        assert_eq!(c.resumen, "Cuaderno", "el resumen es el nombre, no el JSON");
        assert_eq!(paginas_de_carpeta(c.texto.clone()), [lienzo.id.as_str()]);
        // Un lienzo sin nombre entra como «Página 1», no como «Lienzo».
        let leida = n
            .carpeta_de_paginas(chat.clone(), c.id.clone())
            .unwrap()
            .unwrap();
        assert_eq!(leida.paginas.len(), 1);
        assert_eq!(leida.paginas[0].titulo, "Página 1");
        // La segunda no se llama igual; y una vacia no lleva nada.
        let otra = n.crear_carpeta_de_paginas(chat.clone(), None).unwrap();
        assert_eq!(otra.nombre, "Cuaderno 2");
        assert!(paginas_de_carpeta(otra.texto).is_empty());
        // Renombrarla, como un lienzo: sin extension.
        let r = n
            .cambiar_mensaje(
                chat.clone(),
                c.id.clone(),
                crate::chats::CambioMensaje::Nombre {
                    nombre: "Diario de obra".into(),
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(r.nombre, "Diario de obra");
        // Casos negativos: un chat que no esta, un mensaje que no es carpeta.
        assert!(n.crear_carpeta_de_paginas("noesta".into(), None).is_err());
        assert!(
            n.carpeta_de_paginas(chat.clone(), lienzo.id)
                .unwrap()
                .is_none()
        );
        assert!(paginas_de_carpeta("no es json".into()).is_empty());
    }

    #[test]
    fn anadir_reordenar_renombrar_y_quitar() {
        let (n, _d, chat) = nucleo("cambiar");
        let a = n.crear_lienzo(chat.clone(), Some("Plano".into())).unwrap();
        let b = n.crear_lienzo(chat.clone(), None).unwrap();
        let nota = n
            .escribir_nota(chat.clone(), "Lo de en medio".into(), None)
            .unwrap();
        let c = n.crear_carpeta_de_paginas(chat.clone(), None).unwrap();
        let menu = n.carpetas_del_chat(chat.clone(), a.id.clone()).unwrap();
        assert_eq!(menu.len(), 1);
        assert!(!menu[0].la_lleva);

        let x = n
            .anadir_a_carpeta(chat.clone(), c.id.clone(), a.id.clone())
            .unwrap()
            .unwrap();
        assert_eq!(x.titulo, "Plano", "una hoja con nombre entra con el suyo");
        assert!(!x.ya_estaba);
        assert_eq!(x.nombre, "Cuaderno");
        let x = n
            .anadir_a_carpeta(chat.clone(), c.id.clone(), b.id.clone())
            .unwrap()
            .unwrap();
        assert_eq!(x.titulo, "Página 2");
        let x = n
            .anadir_a_carpeta(chat.clone(), c.id.clone(), nota.id.clone())
            .unwrap()
            .unwrap();
        assert_eq!(x.paginas, 3);
        // Otra vez la misma: ya esta, y no se toca.
        let x = n
            .anadir_a_carpeta(chat.clone(), c.id.clone(), a.id.clone())
            .unwrap()
            .unwrap();
        assert!(x.ya_estaba);
        assert_eq!(x.paginas, 3);
        assert!(n.carpetas_del_chat(chat.clone(), a.id.clone()).unwrap()[0].la_lleva);

        let ids = |n: &Nucleo| {
            n.carpeta_de_paginas(chat.clone(), c.id.clone())
                .unwrap()
                .unwrap()
                .paginas
                .into_iter()
                .map(|p| p.mensaje.id)
                .collect::<Vec<_>>()
        };
        n.cambiar_carpeta(
            chat.clone(),
            c.id.clone(),
            CambioCarpeta::Mover { desde: 2, hasta: 0 },
        )
        .unwrap()
        .unwrap();
        assert_eq!(ids(&n), [nota.id.clone(), a.id.clone(), b.id.clone()]);
        n.cambiar_carpeta(
            chat.clone(),
            c.id.clone(),
            CambioCarpeta::Renombrar {
                pagina: a.id.clone(),
                titulo: "Día 1".into(),
            },
        )
        .unwrap();
        let leida = n
            .carpeta_de_paginas(chat.clone(), c.id.clone())
            .unwrap()
            .unwrap();
        assert_eq!(leida.paginas[1].titulo, "Día 1");
        n.cambiar_carpeta(
            chat.clone(),
            c.id.clone(),
            CambioCarpeta::Quitar {
                pagina: nota.id.clone(),
            },
        )
        .unwrap();
        assert_eq!(ids(&n), [a.id.clone(), b.id.clone()]);
        // Lo borrado del chat desaparece de la carpeta sin romperla.
        n.borrar_mensajes(chat.clone(), vec![b.id.clone()]).unwrap();
        assert_eq!(ids(&n), [a.id.as_str()]);
        // Casos negativos: una carpeta en otra, una carpeta que no esta.
        assert!(!n.se_puede_anadir_a_carpeta(chat.clone(), c.id.clone()));
        assert!(n.se_puede_anadir_a_carpeta(chat.clone(), a.id.clone()));
        assert!(
            n.anadir_a_carpeta(chat.clone(), c.id.clone(), c.id.clone())
                .is_err()
        );
        assert!(
            n.anadir_a_carpeta(chat.clone(), "nada".into(), a.id.clone())
                .unwrap()
                .is_none()
        );
        assert!(
            n.cambiar_carpeta(
                chat.clone(),
                "nada".into(),
                CambioCarpeta::Quitar { pagina: a.id }
            )
            .unwrap()
            .is_none()
        );
    }

    /// «Nueva página»: el Día 2 nace con la plantilla del Día 1, vacia, y
    /// entra al final de la carpeta; sin plantilla, en blanco.
    #[test]
    fn la_pagina_siguiente_repite_la_plantilla_vacia_con_el_titulo_siguiente() {
        let (n, d, chat) = nucleo("siguiente");
        let dia1 = n.crear_lienzo(chat.clone(), Some("Día 1".into())).unwrap();
        let piezas = con_diario(&d, &chat, dia1.referencia.as_deref().unwrap());
        let c = n
            .crear_carpeta_de_paginas(chat.clone(), Some(dia1.id.clone()))
            .unwrap();
        assert_eq!(
            n.carpeta_de_paginas(chat.clone(), c.id.clone())
                .unwrap()
                .unwrap()
                .paginas[0]
                .titulo,
            "Día 1"
        );
        let p = n
            .pagina_siguiente_de_carpeta(chat.clone(), c.id.clone())
            .unwrap();
        assert_eq!(p.titulo, "Día 2");
        assert!(p.con_plantilla);
        assert_eq!(
            p.lienzo.nombre, "Día 2",
            "el lienzo se llama como su pagina"
        );
        assert_eq!(
            paginas_de_carpeta(p.carpeta.texto),
            [dia1.id.clone(), p.lienzo.id.clone()]
        );
        let texto = std::fs::read_to_string(almacen::lienzo(
            &d,
            &chat,
            p.lienzo.referencia.as_deref().unwrap(),
        ))
        .unwrap();
        let nueva = excalidraw::a_escena(&excalidraw::leer(&texto).unwrap());
        assert_eq!(
            nueva.cuantos_visibles(),
            piezas,
            "solo la plantilla, sin lo escrito"
        );

        // Sin plantilla: en blanco, «Página 2».
        let blanco = n.crear_lienzo(chat.clone(), None).unwrap();
        let otra = n
            .crear_carpeta_de_paginas(chat.clone(), Some(blanco.id))
            .unwrap();
        let p = n
            .pagina_siguiente_de_carpeta(chat.clone(), otra.id)
            .unwrap();
        assert!(!p.con_plantilla);
        assert_eq!(p.titulo, "Página 2");
        // Caso negativo: una carpeta que no esta es un error.
        assert!(
            n.pagina_siguiente_de_carpeta(chat.clone(), "nada".into())
                .is_err()
        );
    }

    #[test]
    fn el_pdf_saca_de_cada_lienzo_su_plantilla() {
        let (n, d, chat) = nucleo("hojas");
        let dia1 = n.crear_lienzo(chat.clone(), None).unwrap();
        con_diario(&d, &chat, dia1.referencia.as_deref().unwrap());
        let hojas = n
            .hojas_de_carpeta(chat.clone(), dia1.referencia.clone().unwrap())
            .unwrap();
        assert_eq!(hojas.len(), 1, "una hoja: la del diario");
        let (w, h) = diario().tamano();
        assert!(
            hojas[0].x1 - hojas[0].x0 < w + 200.0 && hojas[0].y1 - hojas[0].y0 < h + 200.0,
            "sin lo de lejos"
        );
        assert!(hojas[0].svg.contains("<svg") && !hojas[0].plano.datos.is_empty());
        // Caso negativo: un lienzo vacio no da hojas.
        let vacio = n.crear_lienzo(chat.clone(), None).unwrap();
        assert!(
            n.hojas_de_carpeta(chat.clone(), vacio.referencia.unwrap())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            nombre_del_pdf_de_carpeta("Diario de campo".into()),
            "Diario de campo.pdf"
        );
        assert_eq!(nombre_del_pdf_de_carpeta("  ".into()), "Carpeta.pdf");
    }

    #[test]
    fn el_asistente_anade_por_nombre_y_crea_si_no_esta() {
        let (n, _d, chat) = nucleo("asistente");
        let a = n.crear_lienzo(chat.clone(), None).unwrap();
        let x = anadir_por_nombre(&n, &chat, "Diario", &a.id).unwrap();
        assert!(x.creada);
        assert_eq!(x.nombre, "Diario");
        let b = n.crear_lienzo(chat.clone(), None).unwrap();
        // Sin tildes ni mayusculas: la misma.
        let y = anadir_por_nombre(&n, &chat, "diário", &b.id).unwrap();
        assert!(!y.creada);
        assert_eq!(y.paginas, 2);
        assert!(anadir_por_nombre(&n, &chat, "Diario", "nada").is_err());
    }

    #[test]
    fn las_carpetas_por_nombre_y_el_nombre_libre() {
        let s = |n: i64| Sello {
            cuando: 1000 + n,
            numero: n,
            aparato: "K7Q2".into(),
            proyecto: "p".into(),
        };
        let mut v = vec![carpetas::mensaje_nuevo("Cuaderno", &s(1))];
        assert_eq!(nombre_libre("Cuaderno", &v), "Cuaderno 2");
        v.push(carpetas::mensaje_nuevo("Diario de Campo", &s(2)));
        assert_eq!(carpeta_por_nombre(&v, "diario de campo"), Some(1));
        assert_eq!(carpeta_por_nombre(&v, "campo"), Some(1));
        assert_eq!(carpeta_por_nombre(&v, "obra"), None);
        assert_eq!(carpeta_por_nombre(&v, "  "), None);
        assert_eq!(nombre_libre("Diario", &v), "Diario");
    }
}
