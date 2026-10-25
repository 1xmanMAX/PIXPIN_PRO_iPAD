//! **Tareas**: las listas de tareas de TODOS los chats juntas, como la
//! ventana de Tareas de Windows (`tareas.rs` de la app).
//!
//! **No hay una segunda copia de las tareas.** Cada lista es un mensaje
//! `MINIAPP` con `miniapp = "tareas"` (la mini-app del movil,
//! `pixpin_proyecto::mini`) y aqui solo se lee y se reescribe ese mensaje con
//! las mismas funciones que Windows:
//!
//! - leer: `mini::leer_tareas`, `mini::partir` (la fecha de creacion,
//!   `➕ AAAA-MM-DD` al final) y `mini::imagenes_de`;
//! - marcar: `mini::alternar` sobre el mensaje, reescrito con
//!   `cuaderno::cambiar` (cerrojo de los cuadernos, lo que no se entiende se
//!   copia tal cual). Antes se comprueba que la tarea numero `indice` sigue
//!   siendo la que se ve (su texto crudo): si la lista cambio en el chat o
//!   llego cambiada del movil, no se toca nada;
//! - apuntar: al **Inbox** de «Mensajes guardados» con la fecha de hoy
//!   (`mini::anadir_el`), creandolo si no esta.
//!
//! El formato **no tiene fecha de vencimiento**: la unica fecha de una tarea
//! es la de creacion. Lo que si puede llevar es **la hora de recordarla**
//! (`⏰ AAAA-MM-DD HH:MM`, hora local, dentro del texto: ver
//! [`RELOJ`] y [`crate::recordatorios`]). Como en Windows y en el movil, lo pendiente con fecha
//! va primero, la mas vieja arriba (lo que mas lleva esperando), y se
//! ensena cuantos dias lleva.
//!
//! # Rendimiento
//!
//! Hay que mirar todos los cuadernos, pero no se construye cada mensaje:
//! una linea que no nombra la mini-app `tareas` se salta sin leerla como
//! JSON. Lo devuelto es pequeno (solo las listas).

use std::path::Path;

// Lo que se entiende al escribir una tarea (prioridad, #etiquetas, «recordar
// a las 5») y sus chapitas, tal cual las de Windows (17-oct).
mod cambiar;
mod emoticonos;
pub(crate) mod frase;

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje, Sello};
use pixpin_proyecto::mini;

use crate::chats::{hora_unica, validar_chat};
use crate::{ErrorNucleo, Nucleo};

/// Como se llama la lista adonde va lo apuntado «asi nomas» (Windows:
/// `tareas::INBOX`). Vive en «Mensajes guardados».
pub const INBOX: &str = "Inbox";

/// Una tarea de una lista.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Tarea {
    /// Su numero en el documento (desde 0): lo que pide marcar.
    pub indice: u32,
    /// El texto tal cual esta guardado (con su fecha y sus imagenes): se
    /// devuelve al marcar para comprobar que sigue siendo esta.
    pub crudo: String,
    /// Lo que se ensena: sin la fecha, las imagenes ni la hora de recordar.
    pub texto: String,
    pub hecha: bool,
    /// La hora a la que se recuerda (`⏰`), en ms **locales** (los del reloj
    /// de la pared, sin pasar a UTC), si la lleva.
    pub recuerda: Option<i64>,
    /// El dia en que se creo, `AAAA-MM-DD`, si lo lleva.
    pub creada: Option<String>,
    /// Los dias que lleva desde que se creo (en el huso de quien mira).
    pub dias: Option<u32>,
    /// Sus imagenes, con su ruta absoluta en este aparato (las que no se
    /// pueden resolver aqui no salen).
    pub imagenes: Vec<String>,
}

/// Una lista de tareas de un chat.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ListaTareas {
    /// El id del chat.
    pub chat: String,
    pub nombre_chat: String,
    /// Si es «Mensajes guardados».
    pub guardados: bool,
    /// El id del mensaje de la lista.
    pub mensaje: String,
    /// Su titulo (o el nombre del mensaje si el documento no tiene).
    pub titulo: String,
    /// Cuando se creo el mensaje (ms desde 1970).
    pub cuando: i64,
    pub es_inbox: bool,
    /// Todas sus tareas en el orden del documento.
    pub tareas: Vec<Tarea>,
    pub pendientes: u32,
}

/// Si un mensaje es una lista de tareas (`pedidos::es_lista`).
pub(crate) fn es_lista(m: &Mensaje) -> bool {
    m.clase == Some(Clase::MiniApp) && m.miniapp.as_deref() == Some(mini::TAREAS)
}

/// Como se llama una lista (`pedidos::nombre_de_lista`).
pub(crate) fn nombre_de_lista(m: &Mensaje) -> String {
    match mini::titulo(&m.texto) {
        t if t.is_empty() => m.nombre.clone(),
        t => t,
    }
}

/// Una linea que puede ser una lista de tareas: nombra la mini-app. Lo que
/// no, se salta sin leerlo como JSON.
pub(crate) fn puede_ser_lista(linea: &str) -> bool {
    linea.contains("\"miniapp\"") && linea.contains(mini::TAREAS)
}

/// El orden de las listas (`tareas::ordenar_listas`): el Inbox primero,
/// luego las que tienen algo pendiente, la mas nueva arriba; las hechas
/// enteras al final.
fn ordenar_listas(v: &mut [ListaTareas]) {
    v.sort_by(|a, b| {
        b.es_inbox
            .cmp(&a.es_inbox)
            .then((b.pendientes > 0).cmp(&(a.pendientes > 0)))
            .then(b.cuando.cmp(&a.cuando))
            .then(a.chat.cmp(&b.chat))
            .then(a.mensaje.cmp(&b.mensaje))
    });
}

impl Nucleo {
    fn lista_de(&self, ficha: &almacen::Ficha, m: &Mensaje, hoy: mini::Fecha) -> ListaTareas {
        let tareas: Vec<Tarea> = mini::leer_tareas(&m.texto)
            .into_iter()
            .enumerate()
            .map(|(i, t)| {
                let (visible, creada) = mini::partir(&t.texto);
                let (texto, enlaces) = mini::imagenes_de(visible);
                Tarea {
                    indice: i as u32,
                    recuerda: hora_de_tarea(&texto),
                    texto: sin_marca(&texto),
                    hecha: t.hecha,
                    creada: creada.map(|f| f.to_string()),
                    dias: creada.map(|f| mini::dias_desde(f, hoy)),
                    imagenes: enlaces
                        .iter()
                        .filter_map(|e| self.resolver(&ficha.id, e))
                        .map(|r| r.display().to_string())
                        .collect(),
                    crudo: t.texto,
                }
            })
            .collect();
        let titulo = nombre_de_lista(m);
        let guardados = ficha.es_guardados();
        ListaTareas {
            chat: ficha.id.clone(),
            nombre_chat: ficha.nombre.clone(),
            guardados,
            mensaje: m.id.clone(),
            es_inbox: guardados && titulo.trim().eq_ignore_ascii_case(INBOX),
            titulo,
            cuando: m.cuando,
            pendientes: tareas.iter().filter(|t| !t.hecha).count() as u32,
            tareas,
        }
    }
}

// --------------------------------------------------- los recordatorios
//
// **Una tarea puede recordarse a una hora** (Windows, 8-oct-2026: «añade los
// recordatorios a las tareas»). Android no lo tiene, asi que la hora va
// DENTRO del texto de la tarea: `Llamar al banco ⏰ 2026-10-09 10:00`, en
// hora local (`tareas::RELOJ` de Windows). Asi viaja al sincronizar sin
// inventar otro campo, y en el movil se lee como texto. Al sonar, la marca
// se quita ([`crate::recordatorios`]).

/// Lo que marca la hora en el texto de una tarea.
pub const RELOJ: char = '\u{23F0}';

/// Un dia en milisegundos.
const DIA_MS: i64 = 86_400_000;

/// La marca de una hora local: `⏰ 2026-10-09 10:00`.
pub fn marca_de_hora(local_ms: i64) -> String {
    let f = mini::Fecha::de_dias(local_ms.div_euclid(DIA_MS));
    let min = local_ms.rem_euclid(DIA_MS) / 60_000;
    format!("{RELOJ} {f} {:02}:{:02}", min / 60, min % 60)
}

/// Donde empieza la marca de la hora (en bytes), donde acaba y su hora
/// local, si el texto la lleva. Como Windows (`marca_en`), vale la ultima
/// `⏰`; un reloj suelto o una fecha imposible no son una hora.
fn marca_en(texto: &str) -> Option<(usize, usize, i64)> {
    let i = texto.rfind(RELOJ)?;
    let tras = &texto[i + RELOJ.len_utf8()..];
    let resto = tras.trim_start();
    let blancos = tras.len() - resto.len();
    let num = |a: usize, b: usize| resto.get(a..b).and_then(|s| s.parse::<i64>().ok());
    let (y, mo, d, h, mi) = (num(0, 4)?, num(5, 7)?, num(8, 10)?, num(11, 13)?, num(14, 16)?);
    let sep = resto.as_bytes();
    if sep.get(4) != Some(&b'-') || sep.get(7) != Some(&b'-') || sep.get(13) != Some(&b':') {
        return None;
    }
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 {
        return None;
    }
    let dias = mini::Fecha {
        anio: y as i32,
        mes: mo as u8,
        dia: d as u8,
    }
    .dias();
    let fin = i + RELOJ.len_utf8() + blancos + 16;
    Some((i, fin, (dias * 1440 + h * 60 + mi) * 60_000))
}

/// La hora local a la que se recuerda una tarea, si tiene.
pub fn hora_de_tarea(texto: &str) -> Option<i64> {
    marca_en(texto).map(|(_, _, t)| t)
}

/// El texto sin la marca de la hora (con el blanco de delante, que se puso
/// al marcarla). Lo que va detras (imagenes, la fecha de creacion) se
/// queda: la marca se quita, no se corta el texto.
pub fn sin_marca(texto: &str) -> String {
    match marca_en(texto) {
        Some((i, fin, _)) => {
            let antes = texto[..i].trim_end();
            let despues = texto[fin..].trim_start();
            match (antes.is_empty(), despues.is_empty()) {
                (_, true) => antes.to_string(),
                (true, _) => despues.to_string(),
                _ => format!("{antes} {despues}"),
            }
        }
        None => texto.to_string(),
    }
}

/// **Lo que se ve de una tarea** (la tarjeta, la fila, el Kanban), como
/// `tareas::frase::a_la_vista` de Windows (18-oct): el titulo limpio con la
/// hora y la nota, los emoticonos en circulos y la urgencia y las
/// `#etiquetas` en chapitas aparte.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MarcasTarea {
    /// Lo que hay que hacer, sin marcas.
    pub titulo: String,
    /// `urgente`, `muy_urgente` o nada (normal).
    pub urgencia: Option<String>,
    /// El emoticono de la urgencia (`🔼`, `⏫`), si la tiene.
    pub marca_urgencia: Option<String>,
    pub etiquetas: Vec<String>,
    /// Por que no se esta haciendo (tras `💬`).
    pub nota: Option<String>,
    /// Cuando recordarla, en ms locales.
    pub hora_ms: Option<i64>,
    /// El renglon que se ensena.
    pub vista: String,
    /// Sus emoticonos (en circulos).
    pub chapitas: Vec<String>,
}

/// Las marcas del texto de una tarea, con «hoy 17:00» segun `desfase_ms`.
#[uniffi::export]
pub fn marcas_de_tarea(texto: String, desfase_ms: i64) -> MarcasTarea {
    let m = frase::marcas(&texto);
    let v = frase::a_la_vista(&texto, Some(frase::ahora(desfase_ms)));
    let urgencia = match m.urgencia {
        frase::Urgencia::Normal => None,
        frase::Urgencia::Urgente => Some("urgente".to_string()),
        frase::Urgencia::MuyUrgente => Some("muy_urgente".to_string()),
    };
    MarcasTarea {
        titulo: m.titulo,
        urgencia,
        marca_urgencia: m.urgencia.marca().map(str::to_string),
        etiquetas: m.etiquetas,
        nota: m.nota,
        hora_ms: m.hora.map(frase::ms_de),
        vista: v.texto,
        chapitas: v.emos,
    }
}

/// El texto crudo de una tarea con otra nota (vacia: sin nota).
#[uniffi::export]
pub fn tarea_con_nota(crudo: String, nota: String) -> String {
    frase::con_nota(&crudo, &nota)
}

/// Lo que quedaria guardado al apuntar `texto` (para ensenarlo mientras se
/// escribe).
#[uniffi::export]
pub fn tarea_entendida(texto: String, desfase_ms: i64) -> String {
    let f = frase::leer(&texto, frase::ahora(desfase_ms));
    if f.titulo.is_empty() { texto } else { f.texto() }
}

/// El texto sin la hora (el nombre de Windows, para `frase`).
pub fn sin_hora(texto: &str) -> String {
    sin_marca(texto)
}

/// El texto con la hora `local_ms` (cambiando la que tuviera).
pub fn con_hora(texto: &str, local_ms: i64) -> String {
    format!("{} {}", sin_marca(texto).trim_end(), marca_de_hora(local_ms))
}

/// El texto crudo de una tarea con la hora `local_ms` (cambiando la que
/// tuviera; `None` la quita). La marca va donde la pone Windows: detras de
/// lo que se lee y **delante** de las imagenes y de la fecha de creacion,
/// para que [`mini::partir`] siga encontrandola al final.
pub fn crudo_con_hora(crudo: &str, local_ms: Option<i64>) -> String {
    let (visible, creada) = mini::partir(crudo);
    let (texto, enlaces) = mini::imagenes_de(&sin_marca(visible));
    let texto = match local_ms {
        Some(t) if texto.is_empty() => marca_de_hora(t),
        Some(t) => format!("{texto} {}", marca_de_hora(t)),
        None => texto,
    };
    let visible = mini::con_imagenes(&texto, &enlaces);
    match creada {
        Some(f) => mini::con_fecha(&visible, f),
        None => visible,
    }
}

/// El id de agenda del recordatorio de una tarea: su lista y su hora (el de
/// Windows, `tareas::id_de_recordatorio`).
pub fn id_de_recordatorio(mensaje: &str, local_ms: i64) -> String {
    format!("tarea:{mensaje}:{local_ms}")
}

/// La lista y la hora local de un id de [`id_de_recordatorio`].
pub fn de_id_de_recordatorio(id: &str) -> Option<(&str, i64)> {
    let (m, t) = id.strip_prefix("tarea:")?.rsplit_once(':')?;
    Some((m, t.parse().ok()?))
}

/// Las listas de tareas de un cuaderno (lineas que no se entienden o que
/// estan en el buzon no cuentan, como en Windows al apuntar).
fn listas_del_cuaderno(fichero: &Path) -> std::io::Result<Vec<Mensaje>> {
    let texto = match std::fs::read_to_string(fichero) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(texto
        .lines()
        .filter(|l| puede_ser_lista(l))
        .filter_map(|l| serde_json::from_str::<Mensaje>(l).ok())
        .filter(es_lista)
        .collect())
}

#[uniffi::export]
impl Nucleo {
    /// **Todas las listas de tareas de todos los chats**, ya ordenadas (el
    /// Inbox primero, luego lo pendiente, lo mas nuevo arriba). Dentro de
    /// cada lista, las tareas van en el orden del documento; Swift agrupa
    /// lo pendiente por [`Tarea::dias`]. `desfase_ms` es el huso de quien
    /// mira (para contar los dias). Como en Windows, lo pendiente se
    /// ensena con fecha primero (la mas vieja arriba) y sin fecha detras
    /// (eso lo ordena Swift). Un chat cuyo cuaderno no se puede leer no
    /// deja sin las demas.
    pub fn tareas(&self, desfase_ms: i64) -> Result<Vec<ListaTareas>, ErrorNucleo> {
        let indice = Indice::leer_si_esta(self.ruta())?;
        let hoy = mini::Fecha::de_ms_locales(crate::ahora_ms() + desfase_ms);
        let mut listas = Vec::new();
        for ficha in &indice.proyectos {
            if validar_chat(&ficha.id).is_err() {
                continue;
            }
            let fichero = almacen::carpeta(self.ruta(), &ficha.id).join("guardados.jsonl");
            let Ok(mensajes) = listas_del_cuaderno(&fichero) else {
                continue;
            };
            listas.extend(mensajes.iter().map(|m| self.lista_de(ficha, m, hoy)));
        }
        ordenar_listas(&mut listas);
        Ok(listas)
    }

    /// **Pone una tarea como hecha o pendiente** en su mensaje (pone, no
    /// alterna: repetirlo no deshace). `crudo` es el texto que se ve en
    /// pantalla ([`Tarea::crudo`]): si la tarea numero `indice` ya no es esa
    /// (la lista cambio en el chat o llego cambiada del movil), no se toca
    /// nada y se devuelve `false`: hay que releer.
    pub fn marcar_tarea(
        &self,
        chat: String,
        mensaje: String,
        indice: u32,
        crudo: String,
        hecha: bool,
    ) -> Result<bool, ErrorNucleo> {
        validar_chat(&chat)?;
        let carpeta = almacen::carpeta(self.ruta(), &chat);
        let mut era_lista = true;
        let mut sigue = false;
        let r = cuaderno::cambiar(&carpeta, &mensaje, |m| {
            if !es_lista(m) {
                era_lista = false;
                return false;
            }
            let tareas = mini::leer_tareas(&m.texto);
            let Some(t) = tareas.get(indice as usize) else {
                return false;
            };
            if t.texto != crudo {
                return false;
            }
            sigue = true;
            if t.hecha == hecha {
                return false;
            }
            // `alternar` es lo que hace la casilla del chat y del movil: el
            // documento sale reescrito igual que si se hubiera pulsado.
            m.texto = mini::alternar(&m.texto, indice as usize);
            true
        });
        let r = match r {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            otro => otro?,
        };
        if r.is_none() {
            return Err(ErrorNucleo::NoExiste { que: mensaje });
        }
        if !era_lista {
            return Err(ErrorNucleo::Argumento {
                detalle: format!("no es una lista de tareas: {mensaje}"),
            });
        }
        // Tachar deja el cuaderno del mismo tamano: el indice de la cache se
        // suelta a mano, no vaya a valer por la fecha.
        self.cache_tomada().quitar(&carpeta.join("guardados.jsonl"));
        Ok(sigue)
    }

    /// **Apunta una tarea en el Inbox** de «Mensajes guardados» con la
    /// fecha de hoy (los dos se crean si aun no estan), como la caja de la
    /// ventana de Tareas de Windows. Devuelve la lista como quedo.
    pub fn apuntar_tarea(
        &self,
        texto: String,
        desfase_ms: i64,
    ) -> Result<ListaTareas, ErrorNucleo> {
        self.apuntar_tarea_con_imagenes(texto, Vec::new(), desfase_ms)
    }

    /// **Apunta una tarea con imagenes** (pegadas o soltadas en la caja,
    /// Windows `apuntar_con`): cada una se copia a `archivos/` de «Mensajes
    /// guardados» con nombre propio y va dentro del texto como
    /// `![img 01](pixpin:files/…)`, que es lo que viaja al movil.
    pub fn apuntar_tarea_con_imagenes(
        &self,
        texto: String,
        imagenes: Vec<String>,
        desfase_ms: i64,
    ) -> Result<ListaTareas, ErrorNucleo> {
        if mini::saneado(&texto).is_empty() && imagenes.is_empty() {
            return Err(ErrorNucleo::Argumento {
                detalle: "tarea vacia".into(),
            });
        }
        // Lo reconocido al escribir (Windows, 17-oct): prioridad, #etiquetas
        // y la hora de recordar salen del titulo y van como marcas.
        // Las palabras de cada urgencia, las de Ajustes (`[tareas]`).
        if let Ok(a) = pixpin_store::ajustes::cargar(&self.ubicacion()) {
            frase::fijar(a.tareas.palabras());
        }
        let entendida = frase::leer(&texto, frase::ahora(desfase_ms));
        let texto = if entendida.titulo.is_empty() { texto } else { entendida.texto() };
        let ficha = {
            let g = self.chat_guardados()?;
            Indice::leer_si_esta(self.ruta())?
                .proyectos
                .into_iter()
                .find(|f| f.id == g.id)
                .ok_or(ErrorNucleo::NoExiste { que: g.id })?
        };
        self.apuntar_en_lista(&ficha, INBOX, texto, &imagenes, desfase_ms)
    }

    /// **Apunta una tarea con imagenes en la lista `mensaje` del chat
    /// `chat`** (Windows `anadir_pedida` con una lista de un proyecto): lo
    /// mismo que [`Nucleo::apuntar_tarea_con_imagenes`], pero en esa lista y
    /// no en el Inbox. Cada imagen se copia a `archivos/` de ESE chat. Si
    /// `mensaje` no es una lista de tareas, `NoExiste`.
    pub fn apuntar_tarea_con_imagenes_en(
        &self,
        chat: String,
        mensaje: String,
        texto: String,
        imagenes: Vec<String>,
        desfase_ms: i64,
    ) -> Result<ListaTareas, ErrorNucleo> {
        validar_chat(&chat)?;
        if mini::saneado(&texto).is_empty() && imagenes.is_empty() {
            return Err(ErrorNucleo::Argumento {
                detalle: "tarea vacia".into(),
            });
        }
        if let Ok(a) = pixpin_store::ajustes::cargar(&self.ubicacion()) {
            frase::fijar(a.tareas.palabras());
        }
        let entendida = frase::leer(&texto, frase::ahora(desfase_ms));
        let texto = if entendida.titulo.is_empty() { texto } else { entendida.texto() };
        let raiz = self.ruta();
        let ficha = Indice::leer_si_esta(raiz)?
            .proyectos
            .into_iter()
            .find(|f| f.id == chat)
            .ok_or(ErrorNucleo::NoExiste { que: chat.clone() })?;
        let carpeta = almacen::carpeta(raiz, &ficha.id);
        let fichero = carpeta.join("guardados.jsonl");
        if !listas_del_cuaderno(&fichero)?.iter().any(|m| m.id == mensaje) {
            return Err(ErrorNucleo::NoExiste { que: mensaje });
        }
        let mut enlaces = Vec::with_capacity(imagenes.len());
        for (i, r) in imagenes.iter().enumerate() {
            enlaces.push((i as u32 + 1, guardar_imagen_de_tarea(raiz, &ficha.id, Path::new(r))?));
        }
        let texto = if enlaces.is_empty() { texto } else { mini::fichas_a_imagenes(mini::saneado(&texto).as_str(), &enlaces) };
        let ahora = crate::ahora_ms();
        let hoy = mini::Fecha::de_ms_locales(ahora + desfase_ms);
        let nuevo = cuaderno::cambiar(&carpeta, &mensaje, |m| {
            if !es_lista(m) {
                return false;
            }
            m.texto = mini::anadir_el(&m.texto, &texto, hoy);
            true
        })?
        .ok_or(ErrorNucleo::NoExiste { que: mensaje.clone() })?;
        self.cache_tomada().quitar(&fichero);
        let mut indice = Indice::leer_si_esta(raiz)?;
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == ficha.id) {
            f.tocado = ahora;
            f.resumen = nombre_de_lista(&nuevo);
            indice.guardar(raiz)?;
        }
        Ok(self.lista_de(&ficha, &nuevo, hoy))
    }
}

impl Nucleo {
    /// Apunta `texto` (ya entendido) con sus `imagenes` en la lista de
    /// tareas `nombre_lista` del chat `ficha` (sin mirar mayusculas ni
    /// tildes), creandola vacia si no esta. Devuelve la lista como quedo.
    pub(crate) fn apuntar_en_lista(
        &self,
        ficha: &almacen::Ficha,
        nombre_lista: &str,
        texto: String,
        imagenes: &[String],
        desfase_ms: i64,
    ) -> Result<ListaTareas, ErrorNucleo> {
        let raiz = self.ruta();
        let mut enlaces = Vec::with_capacity(imagenes.len());
        for (i, r) in imagenes.iter().enumerate() {
            enlaces.push((i as u32 + 1, guardar_imagen_de_tarea(raiz, &ficha.id, Path::new(r))?));
        }
        let texto = if enlaces.is_empty() { texto } else { mini::fichas_a_imagenes(mini::saneado(&texto).as_str(), &enlaces) };
        let carpeta = almacen::carpeta(raiz, &ficha.id);
        let fichero = carpeta.join("guardados.jsonl");
        let ahora = crate::ahora_ms();
        let hoy = mini::Fecha::de_ms_locales(ahora + desfase_ms);
        let buscada = pixpin_ia::acciones::normalizar(nombre_lista.trim());
        let inbox = listas_del_cuaderno(&fichero)?
            .into_iter()
            .find(|m| !m.en_buzon && pixpin_ia::acciones::normalizar(nombre_de_lista(m).trim()) == buscada);
        let inbox = match inbox {
            Some(m) => m,
            None => {
                // Una lista nueva y vacia, como `pedidos::nueva_lista`.
                let aparato = self.identidad()?.yo.codigo();
                let documento =
                    mini::documento_nuevo(mini::TAREAS, nombre_lista, &mini::gastos::Moneda::euro())
                        .unwrap_or_default();
                let mut cache = self.cache_tomada();
                let numero = cache.indice(&fichero)?.map_or(0, |i| i.mayor_numero) + 1;
                let m = Mensaje::miniapp(
                    mini::TAREAS,
                    nombre_lista,
                    &documento,
                    &Sello {
                        cuando: hora_unica(),
                        numero,
                        aparato,
                        proyecto: ficha.id.clone(),
                    },
                );
                cuaderno::anadir(&carpeta, &m)?;
                cache.quitar(&fichero);
                m
            }
        };
        let nuevo = cuaderno::cambiar(&carpeta, &inbox.id, |m| {
            m.texto = mini::anadir_el(&m.texto, &texto, hoy);
            true
        })?
        .ok_or(ErrorNucleo::NoExiste {
            que: inbox.id.clone(),
        })?;
        self.cache_tomada().quitar(&fichero);
        // El chat sube en la lista, como `subir_en_la_lista`.
        let mut indice = Indice::leer_si_esta(raiz)?;
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == ficha.id) {
            f.tocado = ahora;
            f.resumen = nombre_lista.to_string();
            indice.guardar(raiz)?;
        }
        Ok(self.lista_de(ficha, &nuevo, hoy))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn solo_las_lineas_que_nombran_la_miniapp_se_leen() {
        assert!(puede_ser_lista(
            r#"{"id":"1","miniapp":"tareas","texto":"- [ ] a"}"#
        ));
        assert!(!puede_ser_lista(
            r#"{"id":"1","clase":"NOTA","texto":"tareas"}"#
        ));
    }

    /// Las 10:30 del 9-oct-2026, en hora local.
    fn diez_y_media() -> i64 {
        let dias = mini::Fecha {
            anio: 2026,
            mes: 10,
            dia: 9,
        }
        .dias();
        (dias * 1440 + 10 * 60 + 30) * 60_000
    }

    #[test]
    fn la_hora_de_una_tarea_va_y_vuelve_por_su_texto() {
        // `la_hora_de_una_tarea_va_y_vuelve_por_su_texto` de Windows.
        let t = diez_y_media();
        assert_eq!(marca_de_hora(t), "⏰ 2026-10-09 10:30");
        let con = crudo_con_hora("Llamar al banco", Some(t));
        assert_eq!(con, "Llamar al banco ⏰ 2026-10-09 10:30");
        assert_eq!(hora_de_tarea(&con), Some(t));
        assert_eq!(sin_marca(&con), "Llamar al banco");
        // Cambiarla no la repite.
        let otra = crudo_con_hora(&con, Some(t + 60_000));
        assert_eq!(otra.matches(RELOJ).count(), 1);
        assert_eq!(hora_de_tarea(&otra), Some(t + 60_000));
        // Casos negativos: un reloj suelto o una fecha imposible no son hora.
        assert_eq!(hora_de_tarea("despertar ⏰ pronto"), None);
        assert_eq!(hora_de_tarea("x ⏰ 2026-13-01 10:00"), None);
        assert_eq!(hora_de_tarea("x ⏰ 2026-10-01 24:00"), None);
        assert_eq!(sin_marca("x ⏰ pronto"), "x ⏰ pronto");
    }

    #[test]
    fn la_marca_va_delante_de_las_imagenes_y_de_la_fecha_de_creacion() {
        // Asi la escribe Windows (`con_hora` + `corregir`): `partir` tiene que
        // seguir encontrando la fecha al final y las imagenes no se pierden.
        let t = diez_y_media();
        let crudo = "yeso ![img 01](pixpin:files/a.png) ➕ 2026-10-01";
        let con = crudo_con_hora(crudo, Some(t));
        assert_eq!(
            con,
            "yeso ⏰ 2026-10-09 10:30 ![img 01](pixpin:files/a.png) ➕ 2026-10-01"
        );
        let (visible, creada) = mini::partir(&con);
        assert_eq!(creada.map(|f| f.to_string()).as_deref(), Some("2026-10-01"));
        let (texto, enlaces) = mini::imagenes_de(visible);
        assert_eq!(enlaces, ["pixpin:files/a.png"]);
        assert_eq!(hora_de_tarea(&texto), Some(t));
        assert_eq!(sin_marca(&texto), "yeso");
        // Quitarla deja la tarea como estaba.
        assert_eq!(crudo_con_hora(&con, None), crudo);
        // Lo que la marca deja detras se queda (no se corta el texto).
        assert_eq!(sin_marca("a ⏰ 2026-10-09 10:30 b"), "a b");
    }

    #[test]
    fn el_id_de_agenda_de_una_tarea_va_y_vuelve() {
        let id = id_de_recordatorio("L1", 123);
        assert_eq!(id, "tarea:L1:123");
        assert_eq!(de_id_de_recordatorio(&id), Some(("L1", 123)));
        // Caso negativo: un id de mensaje no es de tarea.
        assert_eq!(de_id_de_recordatorio("m7"), None);
        assert_eq!(de_id_de_recordatorio("tarea:L1:mañana"), None);
    }
}

/// Copia una imagen a `archivos/` del chat y devuelve su enlace portatil
/// (`pixpin:files/…/archivos/tarea-<ms>-<n>.<ext>`), como `guardar_imagen`
/// de Windows: con nombre propio, porque el enlace va dentro de un `![](…)`
/// y la sincronizacion corta la ruta en el primer `)` o blanco.
fn guardar_imagen_de_tarea(raiz: &Path, proyecto: &str, origen: &Path) -> Result<String, ErrorNucleo> {
    let ext = origen.extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
    if !origen.is_file() || !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "bmp" | "tif" | "tiff") {
        return Err(ErrorNucleo::Argumento { detalle: format!("no es una imagen: {}", origen.display()) });
    }
    let carpeta = almacen::carpeta(raiz, proyecto).join("archivos");
    std::fs::create_dir_all(&carpeta)?;
    let ahora = crate::ahora_ms();
    let nombre = (1..10_000)
        .map(|n| format!("tarea-{ahora}-{n:02}.{ext}"))
        .find(|n| !carpeta.join(n).exists())
        .ok_or_else(|| ErrorNucleo::Argumento { detalle: "demasiadas imagenes a la vez".into() })?;
    std::fs::copy(origen, carpeta.join(&nombre))?;
    let chat = pixpin_proyecto::vista::chat_de_ficha(raiz, proyecto).unwrap_or_else(|| proyecto.to_string());
    Ok(format!(
        "{}{}",
        pixpin_sincro::disco::PORTATIL,
        pixpin_proyecto::vista::virtual_de(&chat, &format!("archivos/{nombre}"))
    ))
}
