//! **Las lecciones aprendidas**, guardadas igual que en Windows y Android
//! para que viajen al sincronizar sin tocar el protocolo
//! (`por-traducir/pixpin/src/lecciones/almacen.rs`, `LeccionesStore.kt`):
//!
//! - El archivo es `proyectos/<chat>/android/guardados/lecciones/<id>.leccion`
//!   (el JSON de `pixpin_lecciones::Leccion`, campo a campo el del movil).
//! - Un mensaje de clase `ARCHIVO` con id `lec-<id>`, nombre «💡 titulo», el
//!   resumen legible en su texto y la ruta portatil
//!   `pixpin:files/guardados/lecciones/<id>.leccion` lo senala. Por omision
//!   va a «Mensajes guardados».
//!
//! El archivo es la verdad; el mensaje, su escaparate.
//!
//! # Rendimiento (§4bis)
//!
//! Swift pide **paginas** ([`Nucleo::lecciones`]). La lista entera (son
//! fichas pequenas) se guarda en una cache que vale mientras no cambie la
//! fecha ni el tamano del indice, de ningun cuaderno ni de ninguna leccion:
//! pasar de pagina o buscar otra palabra no relee el disco, solo mira
//! fechas. Al buscar un cuaderno solo se entienden las lineas que dicen
//! `.leccion`. Se suelta con [`Nucleo::vaciar_caches`].

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use pixpin_lecciones::leccion::{EXTENSION, Nota, PREFIJO, Repaso, nuevo_id};
use pixpin_lecciones::{Indice as IndiceLeccion, Leccion, buscador, etiquetador};
use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje, Sello};
use pixpin_proyecto::vista;

use crate::chats::{hora_unica, validar_chat};
use crate::{ErrorNucleo, Nucleo};

/// Donde guarda el movil sus lecciones, relativo a su carpeta `files`.
pub const CARPETA: &str = "guardados/lecciones";
const PORTATIL: &str = "pixpin:files/";

/// Lo mas que se devuelve en una pagina.
pub const TOPE_DE_PAGINA: u32 = 200;

/// Una leccion en la lista o en la ficha.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FichaLeccion {
    pub id: String,
    pub titulo: String,
    pub que_paso: String,
    pub por_que: String,
    pub proxima: String,
    /// `leccion`, `error` o `acierto`.
    pub tipo: String,
    pub area: String,
    /// 1 leve, 2 importante, 3 grave.
    pub gravedad: i64,
    /// Las puestas a mano (o aceptadas).
    pub etiquetas: Vec<String>,
    /// Las que propuso el etiquetador y siguen ahi (se ensenan con ✨).
    pub etiquetas_auto: Vec<String>,
    /// Las automaticas que se quitaron: no vuelven a proponerse.
    pub quitadas: Vec<String>,
    pub causas: Vec<String>,
    /// Cuantas veces paso (1 + repeticiones).
    pub veces: u32,
    /// La caja del repaso (0..=6) y cuando toca, en ms desde 1970.
    pub caja: i64,
    pub repasar: i64,
    pub creada: i64,
    pub tocada: i64,
    pub en_lista: bool,
    /// El chat donde vive su mensaje.
    pub chat: String,
    /// Si ya toca repasarla.
    pub toca_repasar: bool,
    /// 💡, ⚠️ o ✅.
    pub icono: String,
    /// El resumen de varias lineas (el texto de su mensaje).
    pub resumen: String,
}

/// Que lecciones se quieren ver.
#[derive(Debug, Clone, PartialEq, Eq, Default, uniffi::Record)]
pub struct FiltroLecciones {
    /// Palabras a buscar (sin acentos, erratas perdonadas). Vacio: todas,
    /// de la mas tocada a la menos.
    pub texto: String,
    /// Solo las que llevan esta etiqueta (puesta o automatica).
    pub etiqueta: Option<String>,
    /// Solo las que toca repasar, primero las graves y las que se repiten.
    pub solo_para_repasar: bool,
}

/// Una etiqueta y en cuantas lecciones sale.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct EtiquetaContada {
    pub etiqueta: String,
    pub cuantas: u32,
}

/// Una pagina de la lista.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaLecciones {
    pub lecciones: Vec<FichaLeccion>,
    pub desde: u64,
    /// Cuantas cumplen el filtro.
    pub total: u64,
    /// Cuantas toca repasar hoy (de todas, no solo del filtro).
    pub para_repasar: u64,
    /// Todas las etiquetas, de la mas usada a la menos.
    pub etiquetas: Vec<EtiquetaContada>,
}

/// Lo que se escribe en la ficha. `id` vacio: una nueva.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LeccionEditada {
    pub id: Option<String>,
    pub titulo: String,
    pub que_paso: String,
    pub por_que: String,
    pub proxima: String,
    pub tipo: String,
    pub area: String,
    pub gravedad: i64,
    pub etiquetas: Vec<String>,
    pub etiquetas_auto: Vec<String>,
    /// Automaticas que se quitaron ahora (se anaden a las de antes).
    pub quitadas: Vec<String>,
    pub causas: Vec<String>,
    /// El chat de una nueva (ninguno: «Mensajes guardados»). En una que ya
    /// existe no se mira: no se cambia de chat.
    pub chat: Option<String>,
    /// El mensaje del que sale una nueva («Hacer lección» en su menu): se
    /// guarda en `deMensaje`, como en Windows y el movil.
    #[uniffi(default = None)]
    pub de_mensaje: Option<String>,
}

/// **Una leccion a partir de un mensaje** («Hacer lección»): lo que la
/// ficha nueva trae ya escrito, como `lecciones::nueva` de Windows.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LeccionDeMensaje {
    pub titulo: String,
    pub que_paso: String,
    pub por_que: String,
    pub proxima: String,
    /// Las `#etiquetas` que venian escritas.
    pub etiquetas: Vec<String>,
    pub de_mensaje: String,
    pub chat: String,
}

/// Lo que se dijo en una nota de voz sin las marcas de Markdown ni las
/// fotos (`voz::sin_marcas` de Windows).
fn sin_marcas(texto: &str) -> String {
    texto
        .lines()
        .filter(|l| !l.trim_start().starts_with("!["))
        .map(|l| l.trim_start_matches(['#', ' ']).replace("**", "").replace("__", ""))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// El texto con que nace la leccion de `m`: lo que se dijo, su texto o su
/// nombre, lo primero que haya (`Accion::HacerLeccion` de Windows).
pub fn texto_para_leccion(m: &Mensaje) -> Option<String> {
    m.transcripcion
        .as_deref()
        .map(sin_marcas)
        .filter(|t| !t.trim().is_empty())
        .or_else(|| Some(m.texto.clone()).filter(|t| !t.trim().is_empty()))
        .or_else(|| Some(m.nombre.clone()).filter(|t| !t.trim().is_empty()))
}

/// Lo escrito repartido en los campos de la ficha (`Estado::poner` de la
/// ficha de Windows): las `#etiquetas` aparte y el resto por sus marcas
/// («porque», «la proxima vez»...).
pub fn repartir_para_leccion(dicho: &str) -> (pixpin_lecciones::dictado::Campos, Vec<String>) {
    let etiquetas = etiquetador::escritas(dicho);
    let limpio = etiquetador::sin_etiquetas(dicho);
    let mut c = pixpin_lecciones::dictado::repartir(&limpio);
    if c.proxima == c.titulo {
        c.proxima = String::new();
    }
    (c, etiquetas)
}

/// Como fue el repaso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum NotaRepaso {
    Recordaba,
    AMedias,
    Olvide,
    /// Volvio a pasar: se apunta la vez y sube la gravedad.
    Repetida,
}

/// Lo que el etiquetador propone para un texto.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PropuestaLeccion {
    pub etiquetas: Vec<String>,
    pub area: Option<String>,
    pub tipo: Option<String>,
    pub causas: Vec<String>,
    pub gravedad: i64,
}

/// Una leccion con el mensaje que la lleva y el chat donde esta.
#[derive(Debug, Clone)]
struct Entrada {
    leccion: Leccion,
    mensaje: String,
    ficha: String,
    archivo: PathBuf,
}

/// La lista leida y lo que dice si sigue valiendo.
#[derive(Debug, Default)]
pub struct Cache {
    firma: Vec<(PathBuf, Option<SystemTime>, u64)>,
    entradas: Vec<Entrada>,
    indices: Vec<IndiceLeccion>,
}

fn sello_de(p: &Path) -> (PathBuf, Option<SystemTime>, u64) {
    match std::fs::metadata(p) {
        Ok(m) => (p.to_path_buf(), m.modified().ok(), m.len()),
        Err(_) => (p.to_path_buf(), None, u64::MAX),
    }
}

/// La firma de la carpeta: el indice, cada cuaderno y cada leccion
/// conocida. Solo fechas, nada se lee.
fn firma(
    raiz: &Path,
    indice: &Indice,
    entradas: &[Entrada],
) -> Vec<(PathBuf, Option<SystemTime>, u64)> {
    let mut v = vec![sello_de(&raiz.join("proyectos").join("indice.json"))];
    v.extend(
        indice
            .proyectos
            .iter()
            .map(|f| sello_de(&almacen::carpeta(raiz, &f.id).join("guardados.jsonl"))),
    );
    v.extend(entradas.iter().map(|e| sello_de(&e.archivo)));
    v
}

/// Si un mensaje lleva una leccion (`LeccionesStore.esLeccion`).
pub(crate) fn es_leccion(m: &Mensaje) -> bool {
    m.clase == Some(Clase::Archivo)
        && m.ruta
            .as_deref()
            .is_some_and(pixpin_lecciones::leccion::es_ruta_de_leccion)
}

/// La ruta con que el mensaje nombra la leccion `id`.
pub fn ruta_portatil(id: &str) -> String {
    format!("{PORTATIL}{CARPETA}/{id}{EXTENSION}")
}

/// Donde esta en este aparato lo que nombra la `ruta` de un mensaje de
/// leccion (la misma regla que Windows).
fn archivo_de(raiz: &Path, ficha: &str, ruta: &str) -> Option<PathBuf> {
    let carpeta = almacen::carpeta(raiz, ficha);
    let rel = if let Some(rel) = ruta.strip_prefix(PORTATIL) {
        rel.to_string()
    } else if let Some((_, rel)) = ruta.split_once("/files/") {
        rel.to_string()
    } else {
        return vista::ruta_real(raiz, ficha, ruta);
    };
    if rel.starts_with("guardados/pc/") || rel.starts_with("pins/") {
        return vista::ruta_real(raiz, ficha, &format!("{PORTATIL}{rel}"));
    }
    let mut p = carpeta.join("android");
    for trozo in rel.split('/') {
        match trozo {
            "" | "." => {}
            ".." => return None,
            t => p.push(t),
        }
    }
    Some(p)
}

fn archivo_nuevo(raiz: &Path, ficha: &str, id: &str) -> PathBuf {
    almacen::carpeta(raiz, ficha)
        .join("android")
        .join(CARPETA)
        .join(format!("{id}{EXTENSION}"))
}

/// Las lecciones de un cuaderno: solo se entienden las lineas que dicen
/// `.leccion`.
fn mensajes_de_leccion(fichero: &Path) -> Vec<Mensaje> {
    let Ok(bytes) = std::fs::read(fichero) else {
        return Vec::new();
    };
    let aguja = EXTENSION.as_bytes();
    bytes
        .split(|&b| b == b'\n')
        .filter(|l| l.windows(aguja.len()).any(|w| w == aguja))
        .filter_map(|l| serde_json::from_slice::<Mensaje>(l).ok())
        .filter(es_leccion)
        .collect()
}

/// Lee todas las lecciones de la carpeta, de la mas tocada a la menos. Una
/// cuyo archivo no esta o no se entiende no sale.
fn leer_todas(raiz: &Path, indice: &Indice) -> Vec<Entrada> {
    let mut salida: Vec<Entrada> = Vec::new();
    for f in &indice.proyectos {
        let fichero = almacen::carpeta(raiz, &f.id).join("guardados.jsonl");
        for m in mensajes_de_leccion(&fichero) {
            let Some(archivo) = m.ruta.as_deref().and_then(|r| archivo_de(raiz, &f.id, r)) else {
                continue;
            };
            let Some(leccion) = std::fs::read_to_string(&archivo)
                .ok()
                .and_then(|t| Leccion::leer(&t))
            else {
                continue;
            };
            if salida.iter().any(|e| e.leccion.id == leccion.id) {
                continue;
            }
            salida.push(Entrada {
                leccion,
                mensaje: m.id,
                ficha: f.id.clone(),
                archivo,
            });
        }
    }
    salida.sort_by_key(|x| std::cmp::Reverse(x.leccion.tocada));
    salida
}

fn ficha_de(e: &Entrada, ahora: i64) -> FichaLeccion {
    let l = &e.leccion;
    FichaLeccion {
        id: l.id.clone(),
        titulo: l.titulo.clone(),
        que_paso: l.que_paso.clone(),
        por_que: l.por_que.clone(),
        proxima: l.proxima.clone(),
        tipo: l.tipo.clone(),
        area: l.area.clone(),
        gravedad: l.gravedad,
        etiquetas: l.etiquetas.clone(),
        etiquetas_auto: l.etiquetas_auto.clone(),
        quitadas: l.quitadas.clone(),
        causas: l.causas.clone(),
        veces: u32::try_from(l.veces_que_paso()).unwrap_or(u32::MAX),
        caja: l.caja,
        repasar: l.repasar,
        creada: l.creada,
        tocada: l.tocada,
        en_lista: l.en_lista,
        chat: e.ficha.clone(),
        toca_repasar: Repaso::toca(l, ahora),
        icono: l.icono().to_string(),
        resumen: l.resumen(),
    }
}

/// Recorta, quita vacias y repetidas.
fn limpiar(v: Vec<String>) -> Vec<String> {
    let mut salida: Vec<String> = Vec::new();
    for s in v {
        let s = s.trim().trim_start_matches('#').trim().to_string();
        if !s.is_empty() && !salida.contains(&s) {
            salida.push(s);
        }
    }
    salida
}

/// Escribe el archivo entero al lado y lo pone en su sitio de un tiron.
fn escribir_archivo(archivo: &Path, l: &Leccion) -> std::io::Result<i64> {
    if let Some(p) = archivo.parent() {
        std::fs::create_dir_all(p)?;
    }
    let texto = l.escribir();
    let tmp = archivo.with_extension("leccion.tmp");
    std::fs::write(&tmp, &texto)?;
    std::fs::rename(&tmp, archivo)?;
    Ok(texto.len() as i64)
}

#[uniffi::export]
impl Nucleo {
    /// Lo que trae escrito la ficha de una leccion hecha desde el mensaje
    /// `id` de `chat` («Hacer lección»). `None` si el mensaje ya no esta,
    /// es una leccion o no tiene nada que decir.
    pub fn leccion_de_mensaje(
        &self,
        chat: String,
        id: String,
    ) -> Result<Option<LeccionDeMensaje>, ErrorNucleo> {
        validar_chat(&chat)?;
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(self.ruta(), &chat))?;
        let Some(m) = c.mensajes.iter().find(|m| m.id == id) else {
            return Ok(None);
        };
        if es_leccion(m) {
            return Ok(None);
        }
        let Some(texto) = texto_para_leccion(m) else {
            return Ok(None);
        };
        let (campos, etiquetas) = repartir_para_leccion(&texto);
        Ok(Some(LeccionDeMensaje {
            titulo: campos.titulo,
            que_paso: campos.que_paso,
            por_que: campos.por_que,
            proxima: campos.proxima,
            etiquetas,
            de_mensaje: m.id.clone(),
            chat,
        }))
    }

    /// **Una pagina de lecciones** con el filtro dado: `desde` es la
    /// posicion de la primera y `cuantas` el largo (hasta
    /// [`TOPE_DE_PAGINA`]).
    pub fn lecciones(
        &self,
        filtro: FiltroLecciones,
        desde: u64,
        cuantas: u32,
    ) -> Result<PaginaLecciones, ErrorNucleo> {
        let ahora = crate::ahora_ms();
        let cache = self.cache_lecciones()?;
        let todas = &cache.entradas;
        // Las que pasan el filtro, en su orden, por su posicion.
        let mut orden: Vec<usize> = if filtro.texto.trim().is_empty() {
            (0..todas.len()).collect()
        } else {
            let hallados = buscador::buscar(&cache.indices, &filtro.texto);
            hallados
                .iter()
                .filter_map(|r| todas.iter().position(|e| e.leccion.id == r.leccion.id))
                .collect()
        };
        if let Some(etq) = filtro.etiqueta.as_deref().filter(|e| !e.is_empty()) {
            orden.retain(|&i| {
                todas[i]
                    .leccion
                    .todas_las_etiquetas()
                    .iter()
                    .any(|e| e == etq)
            });
        }
        if filtro.solo_para_repasar {
            orden.retain(|&i| Repaso::toca(&todas[i].leccion, ahora));
            if filtro.texto.trim().is_empty() {
                // El orden del repaso del movil: graves y repetidas primero.
                orden.sort_by(|&a, &b| {
                    let (a, b) = (&todas[a].leccion, &todas[b].leccion);
                    b.gravedad
                        .cmp(&a.gravedad)
                        .then(b.repeticiones.len().cmp(&a.repeticiones.len()))
                        .then(a.repasar.cmp(&b.repasar))
                });
            }
        }
        let total = orden.len();
        let desde_us = (desde.min(total as u64)) as usize;
        let hasta = (desde_us + cuantas.min(TOPE_DE_PAGINA) as usize).min(total);
        let lecciones = orden[desde_us..hasta]
            .iter()
            .map(|&i| ficha_de(&todas[i], ahora))
            .collect();
        let mut cuenta: Vec<EtiquetaContada> = Vec::new();
        for e in todas {
            for etq in e.leccion.todas_las_etiquetas() {
                match cuenta.iter_mut().find(|c| c.etiqueta == etq) {
                    Some(c) => c.cuantas += 1,
                    None => cuenta.push(EtiquetaContada {
                        etiqueta: etq,
                        cuantas: 1,
                    }),
                }
            }
        }
        cuenta.sort_by(|a, b| b.cuantas.cmp(&a.cuantas).then(a.etiqueta.cmp(&b.etiqueta)));
        let para_repasar = todas
            .iter()
            .filter(|e| Repaso::toca(&e.leccion, ahora))
            .count() as u64;
        Ok(PaginaLecciones {
            lecciones,
            desde: desde_us as u64,
            total: total as u64,
            para_repasar,
            etiquetas: cuenta,
        })
    }

    /// Una leccion por su id.
    pub fn leccion(&self, id: String) -> Result<FichaLeccion, ErrorNucleo> {
        let cache = self.cache_lecciones()?;
        let e =
            cache
                .entradas
                .iter()
                .find(|e| e.leccion.id == id)
                .ok_or(ErrorNucleo::NoExiste {
                    que: format!("leccion {id}"),
                })?;
        Ok(ficha_de(e, crate::ahora_ms()))
    }

    /// Lo que propone el etiquetador para lo escrito (etiquetas, area,
    /// tipo, causas y gravedad), con lo que uno suele etiquetar.
    pub fn proponer_leccion(&self, texto: String, quitadas: Vec<String>) -> PropuestaLeccion {
        let aprendido = match self.cache_lecciones() {
            Ok(c) => {
                let todas: Vec<Leccion> = c.entradas.iter().map(|e| e.leccion.clone()).collect();
                etiquetador::aprender(&todas)
            }
            Err(_) => etiquetador::Aprendido::default(),
        };
        let p = etiquetador::proponer(&texto, &aprendido, &quitadas);
        let gravedad = etiquetador::proponer_gravedad(&texto, p.tipo.as_deref());
        PropuestaLeccion {
            etiquetas: p.etiquetas,
            area: p.area,
            tipo: p.tipo,
            causas: p.causas,
            gravedad,
        }
    }

    /// **Guarda una leccion**: la primera vez crea su archivo y su mensaje
    /// en el chat; las siguientes reescriben el archivo y ponen al dia el
    /// resumen del mensaje. Lo que no se toca aqui (repeticiones, repaso,
    /// adjuntos, campos de una version mas nueva) se conserva.
    pub fn guardar_leccion(&self, editada: LeccionEditada) -> Result<FichaLeccion, ErrorNucleo> {
        let titulo = editada.titulo.trim().to_string();
        if titulo.is_empty() {
            return Err(ErrorNucleo::Argumento {
                detalle: "una leccion necesita lo que aprendiste (el titulo)".into(),
            });
        }
        let ahora = crate::ahora_ms();
        let existente = match editada.id.as_deref().filter(|i| !i.is_empty()) {
            Some(id) => Some(
                self.cache_lecciones()?
                    .entradas
                    .iter()
                    .find(|e| e.leccion.id == id)
                    .cloned()
                    .ok_or(ErrorNucleo::NoExiste {
                        que: format!("leccion {id}"),
                    })?,
            ),
            None => None,
        };
        let mut l = match &existente {
            Some(e) => e.leccion.clone(),
            None => Leccion::nueva(&nuevo_id(ahora), ahora, &titulo),
        };
        l.titulo = titulo;
        l.que_paso = editada.que_paso.trim().to_string();
        l.por_que = editada.por_que.trim().to_string();
        l.proxima = editada.proxima.trim().to_string();
        if !editada.tipo.trim().is_empty() {
            l.tipo = editada.tipo.trim().to_string();
        }
        l.area = editada.area.trim().to_string();
        l.gravedad = editada.gravedad.clamp(1, 3);
        l.etiquetas = limpiar(editada.etiquetas);
        let mut quitadas = l.quitadas.clone();
        quitadas.extend(editada.quitadas);
        l.quitadas = limpiar(quitadas);
        l.etiquetas_auto = limpiar(editada.etiquetas_auto)
            .into_iter()
            .filter(|e| !l.etiquetas.contains(e) && !l.quitadas.contains(e))
            .collect();
        l.causas = limpiar(editada.causas);
        l.tocada = ahora;
        if existente.is_none() {
            l.de_mensaje = editada.de_mensaje.filter(|d| !d.is_empty());
        }
        let entrada = match existente {
            Some(e) => self.escribir_existente(l, e)?,
            None => self.escribir_nueva(l, editada.chat)?,
        };
        self.soltar_lecciones();
        Ok(ficha_de(&entrada, ahora))
    }

    /// Apunta como fue el repaso (o que volvio a pasar) y la guarda.
    pub fn repasar_leccion(
        &self,
        id: String,
        nota: NotaRepaso,
    ) -> Result<FichaLeccion, ErrorNucleo> {
        let ahora = crate::ahora_ms();
        let e = self
            .cache_lecciones()?
            .entradas
            .iter()
            .find(|e| e.leccion.id == id)
            .cloned()
            .ok_or(ErrorNucleo::NoExiste {
                que: format!("leccion {id}"),
            })?;
        let l = match nota {
            NotaRepaso::Recordaba => Repaso::calificar(&e.leccion, Nota::Recordaba, ahora),
            NotaRepaso::AMedias => Repaso::calificar(&e.leccion, Nota::AMedias, ahora),
            NotaRepaso::Olvide => Repaso::calificar(&e.leccion, Nota::Olvide, ahora),
            NotaRepaso::Repetida => Repaso::repetida(&e.leccion, ahora),
        };
        let entrada = self.escribir_existente(l, e)?;
        self.soltar_lecciones();
        Ok(ficha_de(&entrada, ahora))
    }

    /// **Las fotos y audios de una leccion** (`LeccionesStore.adjuntosDe`,
    /// `almacen::adjuntos_de` de Windows): los de su lista, en su orden, y
    /// **tambien los que le responden** (si dos aparatos le pusieron una
    /// foto cada uno, la lista del archivo trae la de uno, pero las dos le
    /// responden). Viven en el chat de la leccion (`FichaLeccion.chat`).
    pub fn adjuntos_de_leccion(&self, id: String) -> Result<Vec<AdjuntoLeccion>, ErrorNucleo> {
        let e = self
            .cache_lecciones()?
            .entradas
            .iter()
            .find(|e| e.leccion.id == id)
            .cloned()
            .ok_or(ErrorNucleo::NoExiste {
                que: format!("leccion {id}"),
            })?;
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(self.ruta(), &e.ficha))?;
        Ok(adjuntos_de(&e.leccion.adjuntos, &e.mensaje, &c.mensajes)
            .into_iter()
            .map(|m| {
                let fichero = m
                    .ruta
                    .as_deref()
                    .and_then(|r| self.resolver(&e.ficha, r))
                    .filter(|r| r.is_file())
                    .map(|r| r.display().to_string());
                AdjuntoLeccion {
                    mensaje: crate::chats::MensajeChat::from(m),
                    fichero,
                }
            })
            .collect())
    }

    /// **Borra una leccion** como borra el chat: su mensaje y los de sus
    /// fotos y audios salen del cuaderno con su marca de borrado (para que
    /// no vuelvan del otro aparato) y sus archivos se van.
    pub fn borrar_leccion(&self, id: String) -> Result<(), ErrorNucleo> {
        let e = self
            .cache_lecciones()?
            .entradas
            .iter()
            .find(|e| e.leccion.id == id)
            .cloned()
            .ok_or(ErrorNucleo::NoExiste {
                que: format!("leccion {id}"),
            })?;
        let raiz = self.ruta();
        let carpeta = almacen::carpeta(raiz, &e.ficha);
        let mut ids: BTreeSet<String> = e.leccion.adjuntos.iter().cloned().collect();
        ids.insert(e.mensaje.clone());
        let quitados = cuaderno::quitar(&carpeta, &ids)?;
        self.cache_tomada().quitar(&carpeta.join("guardados.jsonl"));
        vista::anotar_borrados(raiz, &e.ficha, &quitados, crate::ahora_ms())?;
        for m in quitados.iter().filter(|m| m.id != e.mensaje) {
            if let Some(r) = m
                .ruta
                .as_deref()
                .and_then(|r| vista::ruta_real(raiz, &e.ficha, r))
                && r.is_file()
            {
                let _ = std::fs::remove_file(r);
            }
        }
        if e.archivo.is_file() {
            std::fs::remove_file(&e.archivo)?;
        }
        self.soltar_lecciones();
        Ok(())
    }
}

/// Una foto o un audio de una leccion, con su fichero en este Mac (`None`
/// si aun no ha llegado).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AdjuntoLeccion {
    pub mensaje: crate::chats::MensajeChat,
    pub fichero: Option<String>,
}

/// Una foto o un audio (no del buzon): lo que puede ir con una leccion.
fn es_adjunto(m: &Mensaje) -> bool {
    matches!(m.clase, Some(Clase::Imagen) | Some(Clase::Voz)) && !m.en_buzon
}

/// Los adjuntos de la leccion del mensaje `mensaje`: los de `lista` que
/// siguen en el chat, en su orden, y luego las fotos y audios que le
/// responden y no estaban en la lista.
pub fn adjuntos_de(lista: &[String], mensaje: &str, chat: &[Mensaje]) -> Vec<Mensaje> {
    let mut v: Vec<Mensaje> = lista
        .iter()
        .filter_map(|id| chat.iter().find(|m| m.id == *id).cloned())
        .collect();
    for m in chat {
        if m.responde_a.as_deref() == Some(mensaje)
            && es_adjunto(m)
            && !v.iter().any(|x| x.id == m.id)
        {
            v.push(m.clone());
        }
    }
    v
}

impl Nucleo {
    /// La lista, releida solo si algo cambio en el disco.
    fn cache_lecciones(&self) -> Result<std::sync::MutexGuard<'_, Cache>, ErrorNucleo> {
        let raiz = self.ruta();
        let indice = Indice::leer_si_esta(raiz)?;
        let mut c = self.lecciones.lock().unwrap_or_else(|e| e.into_inner());
        let ahora = firma(raiz, &indice, &c.entradas);
        if c.firma.is_empty() || c.firma != ahora {
            let entradas = leer_todas(raiz, &indice);
            c.indices = entradas
                .iter()
                .map(|e| IndiceLeccion::nuevo(e.leccion.clone()))
                .collect();
            c.firma = firma(raiz, &indice, &entradas);
            c.entradas = entradas;
        }
        Ok(c)
    }

    /// Suelta la lista de lecciones (se rehace sola).
    pub(crate) fn soltar_lecciones(&self) {
        *self.lecciones.lock().unwrap_or_else(|e| e.into_inner()) = Cache::default();
    }

    /// Cuantas lecciones hay en la cache (para las pruebas).
    pub fn lecciones_en_cache(&self) -> usize {
        self.lecciones
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entradas
            .len()
    }

    fn escribir_nueva(&self, l: Leccion, chat: Option<String>) -> Result<Entrada, ErrorNucleo> {
        let raiz = self.ruta();
        let aparato = self.identidad()?.yo.codigo();
        let ficha = match chat.filter(|c| !c.is_empty()) {
            Some(c) => {
                validar_chat(&c)?;
                if Indice::leer_si_esta(raiz)?.buscar(&c).is_none() {
                    return Err(ErrorNucleo::NoExiste { que: c });
                }
                c
            }
            None => almacen::asegurar_guardados(raiz, crate::ahora_ms(), &aparato)?.id,
        };
        let archivo = archivo_nuevo(raiz, &ficha, &l.id);
        let bytes = escribir_archivo(&archivo, &l)?;
        let mensaje = self.anadir_mensaje(&ficha, &l, &ruta_portatil(&l.id), bytes, &aparato)?;
        Ok(Entrada {
            leccion: l,
            mensaje,
            ficha,
            archivo,
        })
    }

    fn escribir_existente(&self, l: Leccion, e: Entrada) -> Result<Entrada, ErrorNucleo> {
        let raiz = self.ruta();
        let bytes = escribir_archivo(&e.archivo, &l)?;
        let carpeta = almacen::carpeta(raiz, &e.ficha);
        let puesto = cuaderno::cambiar(&carpeta, &e.mensaje, |m| {
            m.texto = l.resumen();
            m.nombre = l.nombre();
            m.bytes = bytes;
            true
        })?;
        self.cache_tomada().quitar(&carpeta.join("guardados.jsonl"));
        let mensaje = match puesto {
            Some(m) => m.id,
            None => {
                // Su mensaje se borro mientras tanto: uno nuevo, con el
                // mismo archivo.
                let aparato = self.identidad()?.yo.codigo();
                let ruta = vista::portatil_de_ruta(raiz, &e.archivo)
                    .unwrap_or_else(|| ruta_portatil(&l.id));
                self.anadir_mensaje(&e.ficha, &l, &ruta, bytes, &aparato)?
            }
        };
        Ok(Entrada {
            leccion: l,
            mensaje,
            ..e
        })
    }

    /// El mensaje `ARCHIVO` que senala la leccion, al final del cuaderno, y
    /// el chat sube en la lista.
    fn anadir_mensaje(
        &self,
        ficha: &str,
        l: &Leccion,
        ruta: &str,
        bytes: i64,
        aparato: &str,
    ) -> Result<String, ErrorNucleo> {
        let raiz = self.ruta();
        let carpeta = almacen::carpeta(raiz, ficha);
        let fichero = carpeta.join("guardados.jsonl");
        let mut cache = self.cache_tomada();
        let numero = cache.indice(&fichero)?.map_or(0, |i| i.mayor_numero) + 1;
        let cuando = hora_unica();
        let sello = Sello {
            cuando,
            numero,
            aparato: aparato.to_string(),
            proyecto: ficha.to_string(),
        };
        let mut m = Mensaje::adjunto(Clase::Archivo, &l.nombre(), ruta, bytes, &sello);
        m.id = format!("{PREFIJO}{}", l.id);
        m.cuando = l.creada;
        m.texto = l.resumen();
        cuaderno::anadir(&carpeta, &m)?;
        cache.quitar(&fichero);
        drop(cache);
        let mut indice = Indice::leer_si_esta(raiz)?;
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == ficha) {
            f.tocado = cuando;
            f.resumen = m.nombre.clone();
            indice.guardar(raiz)?;
        }
        Ok(m.id)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::Arc;

    fn abrir(n: &str) -> (std::path::PathBuf, Arc<Nucleo>) {
        let d = std::env::temp_dir().join(format!(
            "pixpin-nucleo-lecciones-{n}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let nucleo = Nucleo::abrir(d.display().to_string(), "Mac de prueba".into()).unwrap();
        (d, nucleo)
    }

    fn editada(titulo: &str) -> LeccionEditada {
        LeccionEditada {
            id: None,
            titulo: titulo.into(),
            que_paso: String::new(),
            por_que: String::new(),
            proxima: String::new(),
            tipo: String::new(),
            area: String::new(),
            gravedad: 1,
            etiquetas: Vec::new(),
            etiquetas_auto: Vec::new(),
            quitadas: Vec::new(),
            causas: Vec::new(),
            chat: None,
            de_mensaje: None,
        }
    }

    #[test]
    fn una_leccion_nueva_queda_como_en_el_movil() {
        let (d, n) = abrir("nueva");
        let mut e = editada("Revisar el encofrado antes de vaciar");
        e.etiquetas = vec!["#obra".into(), "obra".into(), " ".into()];
        e.proxima = "Si hay vaciado, entonces reviso el encofrado".into();
        let f = n.guardar_leccion(e).unwrap();
        assert_eq!(f.etiquetas, vec!["obra".to_string()]);
        let guardados = n.chat_guardados().unwrap();
        assert_eq!(f.chat, guardados.id);
        // El archivo, en la carpeta del movil.
        let archivo = d
            .join("proyectos")
            .join(&guardados.id)
            .join("android/guardados/lecciones")
            .join(format!("{}.leccion", f.id));
        let l = Leccion::leer(&std::fs::read_to_string(&archivo).unwrap()).unwrap();
        assert_eq!(l.titulo, "Revisar el encofrado antes de vaciar");
        // El mensaje ARCHIVO que lo senala.
        let pagina = n.ultimos_mensajes(guardados.id.clone(), 10).unwrap();
        let m = pagina
            .mensajes
            .iter()
            .find(|m| m.id == format!("lec-{}", f.id))
            .expect("su mensaje");
        assert_eq!(m.clase, "ARCHIVO");
        assert_eq!(m.ruta.as_deref(), Some(ruta_portatil(&f.id).as_str()));
        assert!(m.nombre.starts_with("💡 "));
        assert!(m.texto.contains("La próxima vez"));
        // Y se lista.
        let p = n.lecciones(FiltroLecciones::default(), 0, 50).unwrap();
        assert_eq!(p.total, 1);
        assert_eq!(p.etiquetas[0].etiqueta, "obra");
    }

    #[test]
    fn se_busca_se_filtra_por_etiqueta_y_se_pagina() {
        let (_d, n) = abrir("buscar");
        for i in 0..7 {
            let mut e = editada(&format!("Leccion numero {i} sobre cosas"));
            if i % 2 == 0 {
                e.etiquetas = vec!["par".into()];
            }
            n.guardar_leccion(e).unwrap();
        }
        n.guardar_leccion(editada("Llevar paraguas cuando llueve"))
            .unwrap();
        let todas = n.lecciones(FiltroLecciones::default(), 0, 3).unwrap();
        assert_eq!(todas.total, 8);
        assert_eq!(todas.lecciones.len(), 3);
        let resto = n.lecciones(FiltroLecciones::default(), 6, 3).unwrap();
        assert_eq!(resto.lecciones.len(), 2);
        let par = n
            .lecciones(
                FiltroLecciones {
                    etiqueta: Some("par".into()),
                    ..Default::default()
                },
                0,
                50,
            )
            .unwrap();
        assert_eq!(par.total, 4);
        let busca = n
            .lecciones(
                FiltroLecciones {
                    texto: "paraguas".into(),
                    ..Default::default()
                },
                0,
                50,
            )
            .unwrap();
        assert_eq!(busca.total, 1);
        assert!(busca.lecciones[0].titulo.contains("paraguas"));
        // Nada nuevo para repasar: tocan manana.
        assert_eq!(todas.para_repasar, 0);
    }

    #[test]
    fn editar_repasar_y_borrar() {
        let (d, n) = abrir("editar");
        let f = n.guardar_leccion(editada("Primera version")).unwrap();
        let mut e = editada("Segunda version");
        e.id = Some(f.id.clone());
        e.gravedad = 9;
        e.etiquetas_auto = vec!["auto".into(), "fuera".into()];
        e.quitadas = vec!["fuera".into()];
        let g = n.guardar_leccion(e).unwrap();
        assert_eq!(g.id, f.id);
        assert_eq!(g.titulo, "Segunda version");
        assert_eq!(g.gravedad, 3);
        assert_eq!(g.etiquetas_auto, vec!["auto".to_string()]);
        assert_eq!(
            n.lecciones(FiltroLecciones::default(), 0, 10)
                .unwrap()
                .total,
            1
        );
        // El mensaje se puso al dia, no se duplico.
        let chat = n.chat_guardados().unwrap().id;
        let pagina = n.ultimos_mensajes(chat.clone(), 10).unwrap();
        assert_eq!(
            pagina
                .mensajes
                .iter()
                .filter(|m| m.id.starts_with("lec-"))
                .count(),
            1
        );
        assert!(
            pagina
                .mensajes
                .iter()
                .any(|m| m.nombre == "💡 Segunda version")
        );

        let r = n
            .repasar_leccion(f.id.clone(), NotaRepaso::Recordaba)
            .unwrap();
        assert_eq!(r.caja, 1);
        let r = n
            .repasar_leccion(f.id.clone(), NotaRepaso::Repetida)
            .unwrap();
        assert_eq!(r.veces, 2);
        assert_eq!(r.caja, 0);

        n.borrar_leccion(f.id.clone()).unwrap();
        assert_eq!(
            n.lecciones(FiltroLecciones::default(), 0, 10)
                .unwrap()
                .total,
            0
        );
        let pagina = n.ultimos_mensajes(chat.clone(), 10).unwrap();
        assert!(pagina.mensajes.iter().all(|m| !m.id.starts_with("lec-")));
        assert!(
            !d.join("proyectos")
                .join(&chat)
                .join("android/guardados/lecciones")
                .join(format!("{}.leccion", f.id))
                .exists()
        );
        assert!(matches!(n.leccion(f.id), Err(ErrorNucleo::NoExiste { .. })));
    }

    #[test]
    fn una_leccion_que_llega_de_fuera_se_ve_sin_soltar_nada() {
        let (d, n) = abrir("fuera");
        let f = n.guardar_leccion(editada("De aqui")).unwrap();
        // Guardar suelta la lista; leerla la vuelve a llenar.
        assert_eq!(n.lecciones_en_cache(), 0);
        n.lecciones(FiltroLecciones::default(), 0, 10).unwrap();
        assert_eq!(n.lecciones_en_cache(), 1);
        // Otro aparato reescribe el archivo (lo trae sincronizar).
        let archivo = d
            .join("proyectos")
            .join(&f.chat)
            .join("android/guardados/lecciones")
            .join(format!("{}.leccion", f.id));
        let mut l = Leccion::leer(&std::fs::read_to_string(&archivo).unwrap()).unwrap();
        l.titulo = "Cambiada en el movil, mas larga".into();
        std::fs::write(&archivo, l.escribir()).unwrap();
        assert_eq!(
            n.leccion(f.id).unwrap().titulo,
            "Cambiada en el movil, mas larga"
        );
        assert!(matches!(
            n.guardar_leccion(editada("  ")),
            Err(ErrorNucleo::Argumento { .. })
        ));
        let p = n.proponer_leccion(
            "Error por prisa en la obra, no revise el encofrado".into(),
            Vec::new(),
        );
        assert!(p.gravedad >= 1);
    }
}
