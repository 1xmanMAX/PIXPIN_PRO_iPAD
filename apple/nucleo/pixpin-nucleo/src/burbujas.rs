//! **Lo que llega al chat y hay que ensenar** (tanda B9): lo que Windows o
//! el movil ponen en un chat sincronizado y el Mac recibia sin saber verlo.
//!
//! - **El punto de proyecto** (`punto_de_proyecto.rs` de Windows,
//!   `PuntoDeProyecto` del movil): verde si lo del chat vive tambien en las
//!   hojas de algun proyecto, rojo si solo en el chat. Va en cada
//!   [`crate::chats::MensajeChat`] (`punto`).
//! - **La hoja-pagina** (`PAGINA`): una pagina del PDF del proyecto, con lo
//!   dibujado encima si ya estreno su dibujo (`pdf_en_chat.rs`:
//!   `es_pagina_sin_dibujo`, `dibujo_de_la_hoja`, `asegurar_dibujo`,
//!   `fondo_de_pagina`). Se ve en la burbuja y se abre en el lienzo con la
//!   pagina de fondo.
//! - **La foto anotada en el movil** (`foto_anotada.rs`): el lienzo propio
//!   de una foto (`lienzos/<referencia o foto-<id>>.excalidraw`), con la
//!   foto dentro y lo dibujado encima. Se ve en la burbuja y la foto se abre
//!   en ese mismo lienzo.
//!
//! Nada de esto reescribe lo que no entiende: el `Mensaje`, la `Hoja` y el
//! `Proyecto` guardan sus campos desconocidos (`resto`) y se escriben con
//! ellos.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use pixpin_motor2d::{Elemento, Figura};
use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje};
use pixpin_proyecto::{Hoja, Proyecto, vista};

use crate::chats::validar_chat;
use crate::lienzo::CajaLienzo;
use crate::{ErrorNucleo, Nucleo};

// ------------------------------------------------------ punto de proyecto

/// Si el mensaje lleva punto. Solo lo que es un archivo: una nota escrita o
/// una nota de voz no estan «en los proyectos» ni dejan de estarlo, y un
/// punto ahi seria un adorno que hay que interpretar (igual que el movil).
pub fn lleva_punto(m: &Mensaje) -> bool {
    match m.clase.as_ref() {
        Some(Clase::Imagen | Clase::Archivo | Clase::Pagina | Clase::Dibujo | Clase::Proyecto) => {
            true
        }
        Some(Clase::Otra(c)) => c == "TABLA" || c == "CROQUIS",
        _ => false,
    }
}

/// Lo que dice de cada proyecto que algo es suyo, junto para preguntar sin
/// volver a leer el disco por cada burbuja.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Senas {
    /// Ids de mensaje a los que apunta una hoja (`deMensaje`).
    de_mensaje: HashSet<String>,
    /// Codigos unicos de las hojas: una hoja que el cuaderno no tiene se
    /// ensena como mensaje con el codigo de su hoja.
    uids: HashSet<String>,
    dibujos: HashSet<String>,
    tablas: HashSet<String>,
    croquis: HashSet<String>,
    /// El PDF del que nace un proyecto: no es una hoja, es el documento.
    archivos: HashSet<String>,
    /// Los proyectos que existen, por id de carpeta y por el de dentro.
    proyectos: HashSet<String>,
}

impl Senas {
    /// Junta las senas de unos proyectos. `ids_de_carpeta` son los ids de la
    /// lista (las carpetas), que no siempre coinciden con el `id` de dentro
    /// de un proyecto que vino del movil.
    pub fn de(proyectos: &[Proyecto], ids_de_carpeta: &[String]) -> Senas {
        let mut s = Senas::default();
        s.proyectos.extend(ids_de_carpeta.iter().cloned());
        for p in proyectos {
            if !p.id.is_empty() {
                s.proyectos.insert(p.id.clone());
            }
            s.archivos.extend(p.pdf_origen.iter().cloned());
            s.archivos.extend(p.pdf_limpio.iter().cloned());
            s.croquis.extend(p.croquis.iter().cloned());
            for h in &p.hojas {
                if let Some(d) = h.resto.get("deMensaje").and_then(|v| v.as_str()) {
                    s.de_mensaje.insert(d.to_string());
                }
                s.uids.extend(h.uid.iter().cloned());
                s.dibujos.extend(h.dibujo.iter().cloned());
                s.croquis.extend(h.croquis.iter().cloned());
                if let Some(t) = h.resto.get("tabla").and_then(|v| v.as_str()) {
                    s.tablas.insert(t.to_string());
                }
            }
        }
        s
    }

    /// `estaEnLosProyectos` del movil, con las mismas preguntas en el mismo
    /// orden, mas la del codigo de la hoja (ver [`Senas::uids`]).
    pub fn esta(&self, m: &Mensaje) -> bool {
        let clase = m.clase.as_ref();
        let es = |c: &str| matches!(clase, Some(Clase::Otra(x)) if x == c);
        if clase == Some(&Clase::Archivo)
            && m.ruta.as_ref().is_some_and(|r| self.archivos.contains(r))
        {
            return true;
        }
        if clase == Some(&Clase::Proyecto) {
            // Un mensaje que ES un proyecto esta mientras ese proyecto exista.
            return m
                .referencia
                .as_ref()
                .is_some_and(|r| self.proyectos.contains(r));
        }
        if self.de_mensaje.contains(&m.id) {
            return true;
        }
        if m.uid.as_ref().is_some_and(|u| self.uids.contains(u)) {
            return true;
        }
        if es("TABLA") {
            return m
                .referencia
                .as_ref()
                .is_some_and(|r| self.tablas.contains(r));
        }
        if es("CROQUIS") {
            return m
                .referencia
                .as_ref()
                .is_some_and(|r| self.croquis.contains(r));
        }
        // El rastro viejo: una foto o un dibujo unidos antes de que existiera
        // el vinculo comparten id de dibujo con su hoja (`dibujoDeLaFoto`).
        let dibujo = m
            .referencia
            .clone()
            .unwrap_or_else(|| format!("foto-{}", m.id));
        self.dibujos.contains(&dibujo)
    }
}

/// El `proyecto.json` de un proyecto, si lo tiene y se entiende.
fn leer_proyecto(raiz: &Path, ficha: &str) -> Option<Proyecto> {
    let t = std::fs::read_to_string(almacen::carpeta(raiz, ficha).join("proyecto.json")).ok()?;
    serde_json::from_str(&t).ok()
}

/// Lee los proyectos de `raiz`: la lista y el `proyecto.json` de cada uno.
/// Un proyecto sin `proyecto.json` (uno nacido aqui sin hojas) no aporta
/// mas que su id.
pub fn leer_senas(raiz: &Path) -> Senas {
    let indice = almacen::Indice::leer(raiz);
    let ids: Vec<String> = indice.proyectos.iter().map(|f| f.id.clone()).collect();
    let proyectos: Vec<Proyecto> = ids
        .iter()
        .filter_map(|id| leer_proyecto(raiz, id))
        .collect();
    Senas::de(&proyectos, &ids)
}

/// Las fechas de los ficheros de los que salen las senas: si no cambia
/// ninguna, las senas guardadas siguen valiendo.
fn huella(raiz: &Path) -> Vec<Option<SystemTime>> {
    let fecha = |r: PathBuf| std::fs::metadata(r).and_then(|m| m.modified()).ok();
    let mut v = vec![fecha(almacen::ruta(raiz))];
    for f in almacen::Indice::leer(raiz).proyectos {
        v.push(fecha(almacen::carpeta(raiz, &f.id).join("proyecto.json")));
    }
    v
}

/// Lo que se espera entre una mirada al disco y la siguiente: una pagina de
/// cincuenta burbujas pregunta cincuenta veces seguidas.
const CADA: Duration = Duration::from_millis(250);

struct Guardado {
    raiz: PathBuf,
    mirado: Instant,
    huella: Vec<Option<SystemTime>>,
    senas: Senas,
}

/// Las senas de la ultima carpeta preguntada (Windows las guarda por hilo;
/// aqui preguntan hilos de Swift distintos, asi que van tras un cerrojo).
static SENAS: Mutex<Option<Guardado>> = Mutex::new(None);

/// [`Senas::esta`] contra los proyectos de `raiz`, leidos solo cuando
/// cambian.
pub fn esta(raiz: &Path, m: &Mensaje) -> bool {
    let mut c = SENAS.lock().unwrap_or_else(|e| e.into_inner());
    let fresco = c
        .as_ref()
        .is_some_and(|g| g.raiz == raiz && g.mirado.elapsed() < CADA);
    if !fresco {
        let h = huella(raiz);
        let igual = c.as_ref().is_some_and(|g| g.raiz == raiz && g.huella == h);
        if igual {
            if let Some(g) = c.as_mut() {
                g.mirado = Instant::now();
            }
        } else {
            *c = Some(Guardado {
                raiz: raiz.to_path_buf(),
                mirado: Instant::now(),
                huella: h,
                senas: leer_senas(raiz),
            });
        }
    }
    c.as_ref().is_some_and(|g| g.senas.esta(m))
}

/// El punto de la burbuja: `None` si no lleva, `Some(true)` verde (tambien
/// en un proyecto), `Some(false)` rojo (solo en el chat).
pub fn punto(raiz: &Path, m: &Mensaje) -> Option<bool> {
    lleva_punto(m).then(|| esta(raiz, m))
}

// ------------------------------------------------------- campos del movil

/// Un texto de `resto`, sin espacios alrededor y no vacio.
fn texto_de_resto(m: &Mensaje, campo: &str) -> Option<String> {
    m.resto
        .get(campo)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Si de esta foto anotada se ensena solo la foto (`soloLaFoto` del movil).
/// **Por defecto si**: dibujando fuera del borde, encuadrarlo todo encoge la
/// foto y la rodea de blanco. Windows lo escribe a veces como `"no"`.
pub fn solo_la_foto(m: &Mensaje) -> bool {
    match m.resto.get("soloLaFoto") {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => !s.trim().eq_ignore_ascii_case("no"),
        _ => true,
    }
}

/// **De donde viene**, si lo dice (`vieneDe` del movil, la zona de un lienzo
/// mandada al chat): el texto que se ensena y el lienzo al que lleva.
pub fn viene_de(m: &Mensaje) -> Option<(String, Option<String>)> {
    let v = m.resto.get("vieneDe")?.as_object()?;
    let texto = v.get("texto")?.as_str()?.trim().to_string();
    if texto.is_empty() {
        return None;
    }
    let dibujo = v
        .get("dibujo")
        .and_then(|d| d.as_str())
        .filter(|d| !d.is_empty())
        .map(str::to_string);
    Some((texto, dibujo))
}

/// **De quien llego**, si vino de otro aparato: «Max phone · K7Q2».
pub fn recibido_de(m: &Mensaje) -> Option<String> {
    texto_de_resto(m, "recibidoDe")
}

/// El lienzo donde dibujo el asistente en una respuesta suya (`lienzoIa`).
pub fn lienzo_ia(m: &Mensaje) -> Option<String> {
    texto_de_resto(m, "lienzoIa")
}

// -------------------------------------------------------- la hoja-pagina

/// Una pagina del PDF del proyecto que aun no estreno dibujo: sin fichero
/// propio y sin referencia.
pub fn es_pagina_sin_dibujo(m: &Mensaje) -> bool {
    m.clase == Some(Clase::Pagina)
        && m.pagina.is_some()
        && m.ruta.as_deref().is_none_or(str::is_empty)
        && m.referencia.as_deref().is_none_or(str::is_empty)
}

/// La hoja del `proyecto.json` que representa ese mensaje: por su codigo
/// unico y, si el mensaje no lo tiene, por la pagina, como el movil al
/// volver del editor (`it.pagina == paginaDeFondo`).
fn hoja_del_mensaje<'a>(p: &'a mut Proyecto, m: &Mensaje) -> Option<&'a mut Hoja> {
    if let Some(uid) = m.uid.as_deref() {
        return p.hojas.iter_mut().find(|h| h.uid.as_deref() == Some(uid));
    }
    let pagina = m.pagina?;
    p.hojas
        .iter_mut()
        .find(|h| h.pagina == Some(pagina) && h.nota.is_none() && h.croquis.is_none())
}

/// **El dibujo que ya tiene la hoja de ese mensaje**, sin crear nada: el
/// suyo (`referencia`) o el que se le puso a su hoja en el `proyecto.json`
/// (al abrirla en otro aparato).
pub fn dibujo_de_la_hoja(raiz: &Path, proyecto: &str, m: &Mensaje) -> Option<String> {
    if let Some(r) = m.referencia.as_deref().filter(|r| !r.is_empty()) {
        return Some(r.to_string());
    }
    m.pagina?;
    let mut p = leer_proyecto(raiz, proyecto)?;
    hoja_del_mensaje(&mut p, m)?.dibujo.clone()
}

/// Guarda el `proyecto.json` al lado y luego de un tiron: un corte a mitad
/// no deja el proyecto sin sus hojas.
fn guardar_proyecto(raiz: &Path, ficha: &str, p: &Proyecto) -> std::io::Result<()> {
    let carpeta = almacen::carpeta(raiz, ficha);
    let texto = serde_json::to_string(p)?;
    std::fs::create_dir_all(&carpeta)?;
    let temporal = carpeta.join("proyecto.json.pagina.tmp");
    std::fs::write(&temporal, texto)?;
    std::fs::rename(&temporal, carpeta.join("proyecto.json"))
}

/// **El dibujo de una hoja-pagina, creandolo la primera vez** (el
/// `Proyectos.conDibujo` del movil al abrir una pagina). Si la hoja ya tiene
/// `dibujo`, se reutiliza (y si su fichero no esta, se crea vacio: el
/// `.excalidraw` lo puede traer luego la sincronizacion). Si no, se le pone
/// `dib-<ahora>`, se escribe su `.excalidraw` vacio y se guarda el
/// `proyecto.json`. El fichero va ANTES que el `proyecto.json`: una hoja
/// nunca apunta a un dibujo que no esta.
///
/// `None` si el mensaje no es de una pagina, si el proyecto no tiene
/// `proyecto.json` o si no se pudo escribir.
pub fn asegurar_dibujo(raiz: &Path, proyecto: &str, m: &Mensaje, ahora: i64) -> Option<String> {
    if let Some(r) = m.referencia.as_deref().filter(|r| !r.is_empty()) {
        return Some(r.to_string());
    }
    m.pagina?;
    let mut p = leer_proyecto(raiz, proyecto)?;
    let hoja = hoja_del_mensaje(&mut p, m)?;
    let crear_fichero = |id: &str| -> Option<()> {
        let ruta = almacen::lienzo(raiz, proyecto, id);
        if ruta.is_file() {
            return Some(());
        }
        std::fs::create_dir_all(ruta.parent()?).ok()?;
        let vacio =
            pixpin_motor2d::excalidraw::escribir(&pixpin_motor2d::excalidraw::Lienzo::vacio());
        std::fs::write(&ruta, vacio).ok()
    };
    if let Some(d) = hoja.dibujo.clone() {
        crear_fichero(&d)?;
        return Some(d);
    }
    // Un id que ya este en disco no se pisa.
    let mut t = ahora;
    let id = loop {
        let id = format!("dib-{t}");
        if !almacen::lienzo(raiz, proyecto, &id).exists() {
            break id;
        }
        t += 1;
    };
    crear_fichero(&id)?;
    hoja.dibujo = Some(id.clone());
    p.tocado = p.tocado.max(ahora);
    guardar_proyecto(raiz, proyecto, &p).ok()?;
    Some(id)
}

/// Lo que hace falta para ensenar y abrir una hoja-pagina.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HojaDePagina {
    /// El PDF del proyecto en este Mac, si esta.
    pub pdf: Option<String>,
    /// La pagina, desde 0.
    pub pagina: u32,
    /// Sin PDF: la pagina que una version vieja extrajo como imagen (la
    /// `ruta` del mensaje o `archivos/pagina-NN.png`). Un pixel por unidad.
    pub imagen: Option<String>,
    /// El dibujo de encima (`lienzos/<id>.excalidraw`), si ya lo tiene y su
    /// fichero esta en este Mac.
    pub dibujo: Option<String>,
}

/// El fondo y el dibujo de una hoja-pagina (`fondo_de_pagina` y
/// `pagina_del_pdf` de Windows). `None` si no es una pagina o no hay de
/// donde sacar su fondo.
pub fn hoja_de_pagina(raiz: &Path, proyecto: &str, m: &Mensaje) -> Option<HojaDePagina> {
    if m.clase != Some(Clase::Pagina) {
        return None;
    }
    let pagina = m.pagina.unwrap_or(0);
    let carpeta = almacen::carpeta(raiz, proyecto);
    let dibujo = dibujo_de_la_hoja(raiz, proyecto, m)
        .filter(|d| almacen::lienzo(raiz, proyecto, d).is_file());
    // Las paginas que se extraian antes como PNG llevan su imagen en `ruta`.
    let propia = m
        .ruta
        .as_deref()
        .filter(|r| !r.is_empty())
        .and_then(|r| vista::ruta_real(raiz, proyecto, r))
        .filter(|r| r.is_file() && !r.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")));
    let vieja = Some(
        carpeta
            .join("archivos")
            .join(format!("pagina-{:02}.png", pagina + 1)),
    )
    .filter(|r| r.is_file());
    let pdf = vista::documento_del_proyecto(raiz, proyecto);
    let imagen = propia.or(vieja);
    if pdf.is_none() && imagen.is_none() && dibujo.is_none() {
        return None;
    }
    let texto = |r: PathBuf| r.display().to_string();
    Some(HojaDePagina {
        // Con su imagen propia, la pagina se ve de ella, sin tocar el PDF.
        pdf: if imagen.is_some() {
            None
        } else {
            pdf.map(texto)
        },
        pagina,
        imagen: imagen.map(texto),
        dibujo,
    })
}

// --------------------------------------------------------- la foto anotada

/// El lienzo propio de una foto del chat (`foto_anotada.rs`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FotoAnotada {
    /// El id del lienzo (`lienzos/<id>.excalidraw`).
    pub lienzo: String,
    /// Donde esta la foto dentro del lienzo, para recortar a ella
    /// (`soloLaFoto`). `None` si el lienzo no la lleva.
    pub foto: Option<CajaLienzo>,
}

/// La foto del lienzo: la imagen que el movil puso primero al crearlo
/// (`foto-<hora>-0`) y, si el id no lo dice, la primera imagen.
fn la_foto(visibles: &[&Elemento]) -> Option<usize> {
    let es_foto = |e: &&Elemento| matches!(e.figura, Figura::Imagen { .. });
    visibles
        .iter()
        .position(|e| {
            es_foto(e)
                && e.extras
                    .id_de_fichero
                    .as_deref()
                    .is_some_and(|i| i.starts_with("foto-"))
        })
        .or_else(|| visibles.iter().position(es_foto))
}

/// **Lo dibujado sobre la foto**, si su lienzo esta y tiene algo mas que la
/// foto (sin trazos, la burbuja ensena la foto sola, como Windows).
pub fn foto_anotada(raiz: &Path, proyecto: &str, id: &str) -> Option<FotoAnotada> {
    let ruta = almacen::lienzo(raiz, proyecto, id);
    let texto = std::fs::read_to_string(&ruta).ok()?;
    let lienzo = pixpin_motor2d::excalidraw::leer(&texto).ok()?;
    let elementos = lienzo.elementos();
    let visibles: Vec<&Elemento> = elementos.iter().filter(|e| !e.borrado).collect();
    let foto = la_foto(&visibles);
    if visibles.len() <= usize::from(foto.is_some()) {
        return None;
    }
    Some(FotoAnotada {
        lienzo: id.to_string(),
        foto: foto.map(|i| {
            let f = visibles[i];
            CajaLienzo {
                x0: f.x,
                y0: f.y,
                x1: f.x + f.ancho,
                y1: f.y + f.alto,
            }
        }),
    })
}

// ------------------------------------------------------------- para Swift

/// Un id de lienzo o de mensaje que viene de Swift no puede salir de su
/// carpeta.
fn validar_nombre(n: &str) -> Result<(), ErrorNucleo> {
    if n.is_empty() || n == "." || n == ".." || n.contains(['/', '\\', '\0']) {
        return Err(ErrorNucleo::Argumento {
            detalle: format!("nombre no valido: {n:?}"),
        });
    }
    Ok(())
}

/// El mensaje `id` del chat, leido del cuaderno. Solo para lo que pide un
/// clic o una burbuja rara (una pagina): leer el cuaderno entero por cada
/// foto seria leerlo cincuenta veces por pagina.
fn mensaje_del_chat(raiz: &Path, chat: &str, id: &str) -> Result<Option<Mensaje>, ErrorNucleo> {
    let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, chat))?;
    Ok(c.mensajes.into_iter().find(|m| m.id == id))
}

#[uniffi::export]
impl Nucleo {
    /// **La hoja-pagina `id` del chat** para su burbuja: el PDF o la imagen
    /// de fondo y el dibujo de encima. `None` si no es una pagina o no hay
    /// fondo en este Mac. Bloqueante (lee el cuaderno).
    pub fn hoja_de_pagina(
        &self,
        chat: String,
        id: String,
    ) -> Result<Option<HojaDePagina>, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta();
        Ok(mensaje_del_chat(raiz, &chat, &id)?.and_then(|m| hoja_de_pagina(raiz, &chat, &m)))
    }

    /// **Abrir una hoja-pagina para dibujar**: su dibujo, estrenandolo si
    /// no tiene (`asegurar_dibujo`), y apuntado en la `referencia` de su
    /// mensaje (la proxima vez se abre ese). `None` si no se puede (sin
    /// `proyecto.json` o sin su hoja): mejor no abrir que abrir un lienzo
    /// que luego no se guarda en ningun sitio. Bloqueante.
    pub fn estrenar_dibujo_de_pagina(
        &self,
        chat: String,
        id: String,
    ) -> Result<Option<String>, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta();
        let Some(m) = mensaje_del_chat(raiz, &chat, &id)? else {
            return Ok(None);
        };
        if m.clase != Some(Clase::Pagina) {
            return Ok(None);
        }
        let Some(d) = asegurar_dibujo(raiz, &chat, &m, crate::ahora_ms()) else {
            return Ok(None);
        };
        if es_pagina_sin_dibujo(&m) {
            let carpeta = almacen::carpeta(raiz, &chat);
            cuaderno::cambiar(&carpeta, &id, |m| {
                m.referencia = Some(d.clone());
                true
            })?;
            self.cache_tomada().quitar(&carpeta.join("guardados.jsonl"));
        }
        Ok(Some(d))
    }

    /// **El lienzo de una foto anotada en el movil**: `lienzo` es su id
    /// (`referencia` del mensaje, o `foto-<id>`). `None` si no ha llegado o
    /// solo lleva la foto. Bloqueante (lee el `.excalidraw`).
    pub fn foto_anotada(
        &self,
        chat: String,
        lienzo: String,
    ) -> Result<Option<FotoAnotada>, ErrorNucleo> {
        validar_chat(&chat)?;
        validar_nombre(&lienzo)?;
        Ok(foto_anotada(self.ruta(), &chat, &lienzo))
    }

    /// **Deja la foto `id` con su lienzo, como el movil**
    /// (`lienzo_de_la_foto::asegurar`): lo crea con la foto dentro si no lo
    /// tiene, pasa a el lo dibujado aqui antes (`.pixpin2d`) y apunta la
    /// `referencia`. Devuelve el id del lienzo, para abrirlo. Bloqueante.
    pub fn asegurar_lienzo_de_foto(&self, chat: String, id: String) -> Result<String, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta();
        let m = mensaje_del_chat(raiz, &chat, &id)?.ok_or(ErrorNucleo::NoExiste {
            que: format!("mensaje {id}"),
        })?;
        let foto = m
            .ruta
            .as_deref()
            .filter(|r| !r.is_empty())
            .and_then(|r| vista::ruta_real(raiz, &chat, r))
            .filter(|r| r.is_file())
            .ok_or(ErrorNucleo::NoExiste {
                que: format!("la foto de {id}"),
            })?;
        let medidas = pixpin_codec::imagen::medidas(&foto).map_err(|e| ErrorNucleo::Argumento {
            detalle: e.to_string(),
        })?;
        let hecho = pixpin_proyecto::lienzo_de_la_foto::asegurar(
            raiz,
            &chat,
            &m,
            &foto,
            medidas,
            crate::ahora_ms(),
        )?;
        if hecho.referencia_puesta {
            self.cache_tomada()
                .quitar(&almacen::carpeta(raiz, &chat).join("guardados.jsonl"));
        }
        Ok(hecho.id)
    }
}

#[cfg(test)]
#[path = "burbujas_pruebas.rs"]
mod pruebas;
