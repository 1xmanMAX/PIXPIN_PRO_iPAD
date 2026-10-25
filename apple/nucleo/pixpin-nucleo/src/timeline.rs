//! **El timeline**: los dias con algo y sus momentos, por paginas.
//!
//! No se inventa nada: es `pixpin_timeline` tal cual (el de Windows). Los
//! momentos viven en `<datos>/timeline/momentos.jsonl` y sus fotos y audios
//! en `<datos>/timeline/medios/` (`pixpin_timeline::almacen::Almacen`); los
//! dias se cuentan con `pixpin_timeline::dias` en el huso de quien mira (el
//! `desfase_ms` que pasa Swift: `TimeZone.current.secondsFromGMT() * 1000`).
//!
//! # Rendimiento
//!
//! Un ano de notas son pocos cientos de KB, pero no se relee en cada pagina:
//! los momentos leidos se guardan en una cache que vale mientras el fichero
//! tenga la misma fecha y el mismo tamano (`Almacen::huella`), y se suelta
//! con [`Nucleo::vaciar_caches`]. Swift pide primero los dias (una lista
//! corta) y despues los momentos de un dia, de pagina en pagina.
//!
//! # Apuntar, corregir y borrar (B5)
//!
//! Como `timeline/ventana.rs` de Windows: apuntar anade una linea al final
//! (`Almacen::anadir`) con el texto partido en titulo y descripcion por
//! `pixpin_timeline::frases`, las fotos copiadas (o movidas, si son un
//! temporal nuestro) y el audio movido a `medios/`; corregir, borrar,
//! mandar a «Momentos» y marcar como leccion reescriben el fichero entero
//! (`Almacen::guardar_todos`). Borrar devuelve la linea del momento para
//! «Deshacer» ([`Nucleo::timeline_devolver`]): sus medios se quedan.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use pixpin_timeline::Momento;
use pixpin_timeline::almacen::Almacen;
use pixpin_timeline::dias::{self, Dia, Mes};
use pixpin_timeline::html::{Opciones, Seccion};
use pixpin_timeline::{archivo, emoticonos, frases, resumen};

use crate::{ErrorNucleo, Nucleo};

/// Lo mas que se devuelve en una pagina del timeline.
pub const TOPE_DE_PAGINA_TIMELINE: u32 = 500;

/// Un dia con momentos.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DiaTimeline {
    pub anio: i32,
    pub mes: u8,
    pub dia: u8,
    /// `AAAA-MM-DD`.
    pub iso: String,
    /// Cuantos momentos tiene.
    pub cuantos: u32,
}

/// Una pagina de dias, del mas nuevo al mas viejo.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaDias {
    pub dias: Vec<DiaTimeline>,
    pub desde: u64,
    pub total: u64,
}

/// Un momento, listo para pintar su tarjeta.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MomentoTimeline {
    pub id: String,
    /// Milisegundos desde 1970 (UTC).
    pub cuando: i64,
    /// `HH:MM` en el huso de quien mira.
    pub hora: String,
    pub titulo: String,
    pub descripcion: String,
    /// Las fotos, con su ruta absoluta, en orden.
    pub fotos: Vec<String>,
    /// El audio de lo dicho, con su ruta absoluta.
    pub audio: Option<String>,
    pub duracion_ms: i64,
    /// Mandado a «Momentos».
    pub en_momentos: bool,
    /// La leccion que se hizo de este momento, si se hizo.
    pub leccion: Option<String>,
}

/// Una pagina de momentos de un dia, del mas viejo al mas nuevo (como se
/// leen en la linea de tiempo).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaMomentos {
    pub momentos: Vec<MomentoTimeline>,
    pub desde: u64,
    pub total: u64,
}

/// La huella del fichero de los momentos (`Almacen::huella`).
type Huella = Option<(SystemTime, u64)>;

/// Los momentos de la ultima lectura y la huella del fichero de entonces.
#[derive(Debug, Default)]
pub struct CacheTimeline {
    leidos: Option<(Huella, Arc<Vec<Momento>>)>,
}

impl CacheTimeline {
    pub(crate) fn vaciar(&mut self) {
        self.leidos = None;
    }
}

fn a_ficha(a: &Almacen, m: &Momento, desfase_ms: i64) -> MomentoTimeline {
    MomentoTimeline {
        id: m.id.clone(),
        cuando: m.cuando,
        hora: dias::hora(m.cuando, desfase_ms),
        titulo: m.titulo.clone(),
        descripcion: m.descripcion.clone(),
        fotos: m
            .fotos
            .iter()
            .filter(|f| nombre_de_medio_valido(f))
            .map(|f| a.ruta(f).display().to_string())
            .collect(),
        audio: m
            .audio
            .as_ref()
            .filter(|f| nombre_de_medio_valido(f))
            .map(|f| a.ruta(f).display().to_string()),
        duracion_ms: m.duracion_ms,
        en_momentos: m.en_momentos,
        leccion: m.leccion.clone(),
    }
}

/// Un medio es un nombre suelto dentro de `medios/`: nada de rutas (el
/// fichero puede venir de otro aparato).
fn nombre_de_medio_valido(n: &str) -> bool {
    !n.is_empty() && n != "." && n != ".." && !n.contains(['/', '\\', '\0'])
}

impl Nucleo {
    /// Todos los momentos, de la cache si el fichero no cambio.
    pub(crate) fn momentos(&self) -> Arc<Vec<Momento>> {
        let a = Almacen::en(self.ruta());
        let huella = a.huella();
        let mut cache = self.timeline_tomada();
        if let Some((h, v)) = &cache.leidos
            && huella.is_some()
            && *h == huella
        {
            return Arc::clone(v);
        }
        let v = Arc::new(a.leer());
        cache.leidos = Some((huella, Arc::clone(&v)));
        v
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Los dias con momentos**, del mas nuevo al mas viejo, por paginas.
    /// Con `solo_en_momentos`, solo cuentan los mandados a «Momentos» (la
    /// pestana del archivo); sin el, todos (la de «Hoy» y «Estado»).
    pub fn timeline_dias(
        &self,
        desfase_ms: i64,
        solo_en_momentos: bool,
        desde: u64,
        cuantos: u32,
    ) -> Result<PaginaDias, ErrorNucleo> {
        let todos = self.momentos();
        let elegidos: Vec<Momento>;
        let lista: &[Momento] = if solo_en_momentos {
            elegidos = todos.iter().filter(|m| m.en_momentos).cloned().collect();
            &elegidos
        } else {
            &todos
        };
        let con = dias::dias_con(lista, desfase_ms);
        let total = con.len();
        let desde = (desde.min(total as u64)) as usize;
        let cuantos = cuantos.min(TOPE_DE_PAGINA_TIMELINE) as usize;
        let dias = con
            .iter()
            .rev()
            .skip(desde)
            .take(cuantos)
            .map(|(d, n)| DiaTimeline {
                anio: d.anio,
                mes: d.mes,
                dia: d.dia,
                iso: d.iso(),
                cuantos: *n as u32,
            })
            .collect();
        Ok(PaginaDias {
            dias,
            desde: desde as u64,
            total: total as u64,
        })
    }

    /// **Los momentos de un dia**, del mas viejo al mas nuevo, por paginas.
    /// Un dia que no existe (30 de febrero) es un error de argumento.
    #[allow(clippy::too_many_arguments)]
    pub fn timeline_momentos(
        &self,
        anio: i32,
        mes: u8,
        dia: u8,
        desfase_ms: i64,
        solo_en_momentos: bool,
        desde: u64,
        cuantos: u32,
    ) -> Result<PaginaMomentos, ErrorNucleo> {
        let d = Dia { anio, mes, dia };
        if !(1..=12).contains(&mes) || dia == 0 || Dia::de_numero(d.numero()) != d {
            return Err(ErrorNucleo::Argumento {
                detalle: format!("dia que no existe: {anio}-{mes}-{dia}"),
            });
        }
        let todos = self.momentos();
        let del_dia: Vec<&Momento> = dias::del_dia(&todos, d, desfase_ms)
            .into_iter()
            .filter(|m| !solo_en_momentos || m.en_momentos)
            .collect();
        Ok(pagina(self, &del_dia, desfase_ms, desde, cuantos))
    }

    /// **Hoy**: lo de las ultimas 24 horas, del mas viejo al mas nuevo (lo
    /// nuevo abajo, como en un chat), por paginas.
    pub fn timeline_ultimas_24h(
        &self,
        desfase_ms: i64,
        desde: u64,
        cuantos: u32,
    ) -> Result<PaginaMomentos, ErrorNucleo> {
        let todos = self.momentos();
        let v = dias::ultimas_24h(&todos, crate::ahora_ms());
        Ok(pagina(self, &v, desfase_ms, desde, cuantos))
    }

    /// Busca en el titulo y la descripcion de TODOS los momentos, sin
    /// mayusculas ni tildes y con cada palabra en cualquier orden
    /// (`pixpin_timeline::buscar`), del mas nuevo al mas viejo.
    pub fn timeline_buscar(
        &self,
        consulta: String,
        desfase_ms: i64,
        cuantos: u32,
    ) -> Result<Vec<MomentoTimeline>, ErrorNucleo> {
        let todos = self.momentos();
        let a = Almacen::en(self.ruta());
        Ok(pixpin_timeline::buscar::buscar(&todos, &consulta)
            .into_iter()
            .take(cuantos.min(TOPE_DE_PAGINA_TIMELINE) as usize)
            .map(|m| a_ficha(&a, m, desfase_ms))
            .collect())
    }
}

// ------------------------------------------------------------ apuntar (B5)

/// Lo que se pone si un momento no trae titulo, como Windows
/// (`timeline-sin-titulo-foto`, `timeline-sin-titulo-voz`).
pub const SIN_TITULO_FOTO: &str = "Foto";
pub const SIN_TITULO_VOZ: &str = "Nota de voz";

/// Leer, cambiar y escribir `momentos.jsonl` de uno en uno.
static CERROJO_TIMELINE: Mutex<()> = Mutex::new(());

/// Desempata los ids creados en el mismo milisegundo (`Estado::cuenta`).
static CUENTA: AtomicU32 = AtomicU32::new(0);

/// Una foto que se apunta con un momento.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FotoNueva {
    /// Su ruta absoluta.
    pub ruta: String,
    /// Un temporal nuestro (lo pegado): se mueve en vez de copiarse.
    pub temporal: bool,
}

/// Lo que va en el circulo de un dia de «Estado» (`estado_mes::Circulo`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum CirculoDia {
    /// Sus emoticonos distintos, en orden.
    Emoticonos { lista: Vec<String> },
    /// La primera foto del dia, con su ruta absoluta.
    Foto { ruta: String },
    /// Ni emoticono ni foto: la inicial de su primer titulo.
    Inicial { letra: String },
}

/// Un dia de la tarjeta de un mes de «Estado».
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DiaEstado {
    pub dia: u8,
    /// `AAAA-MM-DD`.
    pub iso: String,
    pub cuantos: u32,
    /// `None` si no tiene nada (un puntito).
    pub circulo: Option<CirculoDia>,
}

/// La tarjeta de un mes de «Estado»: sus dias del mas reciente al 1.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MesEstado {
    pub anio: i32,
    pub mes: u8,
    /// «octubre».
    pub nombre: String,
    /// Lo que mas se repitio («🍕») y cuantas veces, si hay.
    pub lo_mas: Option<String>,
    pub lo_mas_veces: u32,
    pub dias: Vec<DiaEstado>,
}

/// Un dia de «Momentos» (el archivo): sus momentos en el orden en que
/// pasaron.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DiaArchivo {
    pub dia: DiaTimeline,
    /// «lun».
    pub dia_corto: String,
    /// «octubre».
    pub nombre_mes: String,
    pub momentos: Vec<MomentoTimeline>,
}

/// Una pagina de «Momentos», del dia mas nuevo al mas viejo.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaArchivo {
    pub dias: Vec<DiaArchivo>,
    pub desde: u64,
    pub total: u64,
}

/// Lo que se exporta: lo que se ve en la ventana.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ExportarTimeline {
    /// «Hoy»: las ultimas 24 horas.
    Ultimas24h,
    /// Unos dias (`AAAA-MM-DD`): el abierto, o los elegidos en «Estado».
    Dias { isos: Vec<String> },
    /// Lo encontrado con el buscador.
    Buscar { consulta: String },
}

/// La pagina exportada y el nombre que se propone para el fichero (sin
/// extension).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaTimeline {
    pub nombre: String,
    pub html: String,
    /// Cuantos momentos lleva.
    pub cuantos: u32,
}

/// El dia de `iso`, comprobado.
fn dia_de_iso(iso: &str) -> Result<Dia, ErrorNucleo> {
    let mal = || ErrorNucleo::Argumento {
        detalle: format!("dia que no existe: {iso}"),
    };
    let mut p = iso.split('-');
    let (Some(a), Some(m), Some(d), None) = (p.next(), p.next(), p.next(), p.next()) else {
        return Err(mal());
    };
    let (Ok(anio), Ok(mes), Ok(dia)) = (a.parse::<i32>(), m.parse::<u8>(), d.parse::<u8>()) else {
        return Err(mal());
    };
    let x = Dia { anio, mes, dia };
    if !(1..=12).contains(&mes) || dia == 0 || Dia::de_numero(x.numero()) != x {
        return Err(mal());
    }
    Ok(x)
}

/// El circulo de un dia de «Estado», como `estado_mes::circulo`.
fn circulo(a: &Almacen, v: &[&Momento]) -> CirculoDia {
    let mut emos: Vec<String> = Vec::new();
    for m in v {
        for t in [&m.titulo, &m.descripcion] {
            for x in emoticonos::emoticonos(t) {
                if !emos.iter().any(|y| emoticonos::clave(y) == emoticonos::clave(x)) {
                    emos.push(x.to_string());
                }
            }
        }
    }
    if !emos.is_empty() {
        return CirculoDia::Emoticonos { lista: emos };
    }
    for m in v {
        if let Some(f) = m.fotos.iter().find(|f| nombre_de_medio_valido(f)) {
            return CirculoDia::Foto {
                ruta: a.ruta(f).display().to_string(),
            };
        }
    }
    let letra = v
        .first()
        .and_then(|m| m.titulo.chars().find(|c| c.is_alphanumeric()))
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default();
    CirculoDia::Inicial { letra }
}

impl Nucleo {
    /// Lee, cambia y escribe todos los momentos con el cerrojo cogido.
    /// Si `f` da un error, no se escribe nada.
    fn cambiar_momentos<T>(
        &self,
        f: impl FnOnce(&Almacen, &mut Vec<Momento>) -> Result<T, ErrorNucleo>,
    ) -> Result<T, ErrorNucleo> {
        let _c = CERROJO_TIMELINE.lock().unwrap_or_else(|e| e.into_inner());
        let a = Almacen::en(self.ruta());
        let mut todos = a.leer();
        let r = f(&a, &mut todos)?;
        todos.sort_by_key(|m| m.cuando);
        a.guardar_todos(&todos)?;
        self.timeline_tomada().vaciar();
        Ok(r)
    }

    /// La posicion del momento `id`, o un error «no existe».
    fn indice_de_momento(todos: &[Momento], id: &str) -> Result<usize, ErrorNucleo> {
        todos
            .iter()
            .position(|m| m.id == id)
            .ok_or_else(|| ErrorNucleo::NoExiste {
                que: format!("momento {id}"),
            })
    }

    /// Crea la leccion de un momento en «Mensajes guardados», como
    /// `lecciones::crear` de Windows: lo que propone el etiquetador y la
    /// gravedad al menos «importante» (si se marco, algo enseno).
    fn leccion_de_momento(&self, m: &Momento) -> Result<String, ErrorNucleo> {
        let texto = format!("{}\n{}", m.titulo, m.descripcion);
        let p = self.proponer_leccion(texto, Vec::new());
        let ficha = self.guardar_leccion(crate::lecciones::LeccionEditada {
            id: None,
            titulo: m.titulo.clone(),
            que_paso: m.descripcion.clone(),
            por_que: String::new(),
            proxima: String::new(),
            tipo: p.tipo.unwrap_or_default(),
            area: p.area.unwrap_or_default(),
            gravedad: p.gravedad.max(2),
            etiquetas: Vec::new(),
            etiquetas_auto: p.etiquetas,
            quitadas: Vec::new(),
            causas: p.causas,
            chat: None,
            de_mensaje: None,
        })?;
        Ok(ficha.id)
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Apunta un momento** (Intro en la caja de «Hoy»): `texto` se parte
    /// en titulo y descripcion (`frases::partir`), las fotos se copian a
    /// `medios/` (las temporales se mueven) y el audio se mueve. Sin titulo,
    /// «Nota de voz» o «Foto»; sin nada, error de argumento. `cuando` es el
    /// principio de lo dicho, o ahora.
    #[allow(clippy::too_many_arguments)]
    pub fn timeline_apuntar(
        &self,
        texto: String,
        fotos: Vec<FotoNueva>,
        audio: Option<String>,
        duracion_ms: i64,
        dicho: Option<String>,
        cuando: Option<i64>,
        desfase_ms: i64,
    ) -> Result<MomentoTimeline, ErrorNucleo> {
        let cuando = cuando.unwrap_or_else(crate::ahora_ms);
        let (mut titulo, descripcion) = frases::partir(texto.trim());
        if titulo.is_empty() {
            titulo = if audio.is_some() {
                SIN_TITULO_VOZ.into()
            } else if !fotos.is_empty() {
                SIN_TITULO_FOTO.into()
            } else {
                return Err(ErrorNucleo::Argumento {
                    detalle: "un momento sin texto, foto ni voz".into(),
                });
            };
        }
        let _c = CERROJO_TIMELINE.lock().unwrap_or_else(|e| e.into_inner());
        let a = Almacen::en(self.ruta());
        // Otro aparato pudo usar el mismo numero en el mismo milisegundo.
        let ya: Vec<String> = a.leer().into_iter().map(|m| m.id).collect();
        let mut n = CUENTA.fetch_add(1, Ordering::Relaxed) + 1;
        let mut id = pixpin_timeline::momento::id_nuevo(cuando, n);
        while ya.contains(&id) {
            n += 1;
            id = pixpin_timeline::momento::id_nuevo(cuando, n);
        }
        let mut m = Momento::nuevo(id.clone(), cuando, titulo, descripcion);
        for (k, f) in fotos.iter().enumerate() {
            let base = format!("{id}-{}", k + 1);
            let origen = Path::new(&f.ruta);
            let hecho = if f.temporal {
                a.mover_medio(origen, &base)
            } else {
                a.copiar_medio(origen, &base)
            };
            m.fotos.push(hecho?);
        }
        if let Some(audio) = audio {
            m.audio = Some(a.mover_medio(Path::new(&audio), &id)?);
            m.duracion_ms = duracion_ms.max(0);
        }
        m.dicho = dicho.filter(|d| !d.trim().is_empty());
        a.anadir(&m)?;
        self.timeline_tomada().vaciar();
        Ok(a_ficha(&a, &m, desfase_ms))
    }

    /// **Corrige un momento** (↑ con la caja vacia, o «Corregir»): el texto
    /// nuevo se parte como al apuntar. Sin titulo se queda el de antes,
    /// salvo que no tenga foto ni voz: entonces es un error («escribe algo
    /// para el titulo»).
    pub fn timeline_corregir(
        &self,
        id: String,
        texto: String,
        desfase_ms: i64,
    ) -> Result<MomentoTimeline, ErrorNucleo> {
        let (titulo, descripcion) = frases::partir(texto.trim());
        self.cambiar_momentos(|a, todos| {
            let i = Nucleo::indice_de_momento(todos, &id)?;
            let m = &mut todos[i];
            if titulo.is_empty() && m.fotos.is_empty() && m.audio.is_none() {
                return Err(ErrorNucleo::Argumento {
                    detalle: "escribe algo para el titulo".into(),
                });
            }
            if !titulo.is_empty() {
                m.titulo = titulo;
            }
            m.descripcion = descripcion;
            Ok(a_ficha(a, m, desfase_ms))
        })
    }

    /// **Borra un momento** y devuelve su linea, para «Deshacer»
    /// ([`Nucleo::timeline_devolver`]). Sus fotos y su audio se quedan.
    pub fn timeline_borrar(&self, id: String) -> Result<String, ErrorNucleo> {
        self.cambiar_momentos(|_, todos| {
            let i = Nucleo::indice_de_momento(todos, &id)?;
            Ok(todos.remove(i).a_linea())
        })
    }

    /// **Deshace un borrado**: vuelve el momento de la `linea` que dio
    /// [`Nucleo::timeline_borrar`]. Si ya estaba, no hace nada.
    pub fn timeline_devolver(&self, linea: String) -> Result<(), ErrorNucleo> {
        let m = Momento::de_linea(&linea).ok_or_else(|| ErrorNucleo::Argumento {
            detalle: "no es la linea de un momento".into(),
        })?;
        self.cambiar_momentos(|_, todos| {
            if !todos.iter().any(|x| x.id == m.id) {
                todos.push(m);
            }
            Ok(())
        })
    }

    /// **Lo manda a «Momentos», o lo saca** (el marcador de la tarjeta).
    /// Devuelve si queda dentro.
    pub fn timeline_a_momentos(&self, id: String) -> Result<bool, ErrorNucleo> {
        self.cambiar_momentos(|_, todos| {
            let i = Nucleo::indice_de_momento(todos, &id)?;
            todos[i].en_momentos = !todos[i].en_momentos;
            Ok(todos[i].en_momentos)
        })
    }

    /// **La bombilla «Lección»** (`lecciones::alternar` de Windows): marca
    /// el momento haciendo su leccion en «Mensajes guardados» (o reenlaza la
    /// que tuvo, si sigue existiendo), o lo desmarca sin borrar la leccion.
    /// Devuelve el id de la leccion si queda marcado.
    pub fn timeline_leccion(&self, id: String) -> Result<Option<String>, ErrorNucleo> {
        let m = {
            let todos = Almacen::en(self.ruta()).leer();
            todos[Nucleo::indice_de_momento(&todos, &id)?].clone()
        };
        if m.leccion.is_some() {
            return self.cambiar_momentos(|_, todos| {
                let i = Nucleo::indice_de_momento(todos, &id)?;
                let x = &mut todos[i];
                x.leccion_previa = x.leccion.take();
                Ok(None)
            });
        }
        let reusar = m
            .leccion_previa
            .clone()
            .filter(|l| self.leccion(l.clone()).is_ok());
        let leccion = match reusar {
            Some(l) => l,
            None => self.leccion_de_momento(&m)?,
        };
        self.cambiar_momentos(|_, todos| {
            let i = Nucleo::indice_de_momento(todos, &id)?;
            let x = &mut todos[i];
            x.leccion = Some(leccion.clone());
            x.leccion_previa = None;
            Ok(Some(leccion))
        })
    }

    /// **«Momentos»**, el archivo: los dias con algo mandado a «Momentos»,
    /// del mas nuevo al mas viejo, cada uno con sus momentos en orden, por
    /// paginas de dias (`archivo::agrupar`).
    pub fn timeline_archivo(
        &self,
        desfase_ms: i64,
        desde: u64,
        cuantos: u32,
    ) -> Result<PaginaArchivo, ErrorNucleo> {
        let todos = self.momentos();
        let elegidos: Vec<Momento> = todos.iter().filter(|m| m.en_momentos).cloned().collect();
        let a = Almacen::en(self.ruta());
        let anios = archivo::agrupar(&elegidos, desfase_ms);
        let lista: Vec<&archivo::DiaDelArchivo<'_>> = anios
            .iter()
            .flat_map(|x| &x.meses)
            .flat_map(|m| &m.dias)
            .collect();
        let total = lista.len();
        let desde = (desde.min(total as u64)) as usize;
        let cuantos = cuantos.min(TOPE_DE_PAGINA_TIMELINE) as usize;
        Ok(PaginaArchivo {
            dias: lista
                .iter()
                .skip(desde)
                .take(cuantos)
                .map(|d| DiaArchivo {
                    dia: DiaTimeline {
                        anio: d.dia.anio,
                        mes: d.dia.mes,
                        dia: d.dia.dia,
                        iso: d.dia.iso(),
                        cuantos: d.momentos.len() as u32,
                    },
                    dia_corto: dias::dia_corto(d.dia, false).into(),
                    nombre_mes: dias::solo_el_mes(d.dia.mes, false).into(),
                    momentos: d
                        .momentos
                        .iter()
                        .map(|m| a_ficha(&a, m, desfase_ms))
                        .collect(),
                })
                .collect(),
            desde: desde as u64,
            total: total as u64,
        })
    }

    /// **«Estado»**: una tarjeta por mes con algo (y la del mes de hoy
    /// siempre), del mas nuevo al mas viejo, con sus dias al reves y el
    /// circulo de cada dia con algo (`archivo::meses_del_estado`).
    pub fn timeline_estado(&self, desfase_ms: i64) -> Result<Vec<MesEstado>, ErrorNucleo> {
        let todos = self.momentos();
        let a = Almacen::en(self.ruta());
        let hoy = Dia::de_instante(crate::ahora_ms(), desfase_ms);
        let mut por_dia: BTreeMap<Dia, Vec<&Momento>> = BTreeMap::new();
        for m in todos.iter() {
            por_dia
                .entry(Dia::de_instante(m.cuando, desfase_ms))
                .or_default()
                .push(m);
        }
        Ok(archivo::meses_del_estado(&todos, desfase_ms, hoy)
            .into_iter()
            .map(|mes| {
                let del_mes: Vec<&Momento> = por_dia
                    .iter()
                    .filter(|(d, _)| Mes::de(**d) == mes)
                    .flat_map(|(_, v)| v.iter().copied())
                    .collect();
                let lo_mas = resumen::lo_mas_con_cuenta(&del_mes);
                MesEstado {
                    anio: mes.anio,
                    mes: mes.mes,
                    nombre: dias::solo_el_mes(mes.mes, false).into(),
                    lo_mas_veces: lo_mas.as_ref().map_or(0, |(_, n)| *n as u32),
                    lo_mas: lo_mas.map(|(l, _)| l.texto().to_string()),
                    dias: archivo::dias_al_reves(mes, hoy)
                        .into_iter()
                        .map(|d| {
                            let v = por_dia.get(&d);
                            DiaEstado {
                                dia: d.dia,
                                iso: d.iso(),
                                cuantos: v.map_or(0, |v| v.len() as u32),
                                circulo: v.map(|v| circulo(&a, v)),
                            }
                        })
                        .collect(),
                }
            })
            .collect())
    }

    /// **Exportar lo que se ve** (`exportar_lo_que_se_ve` de Windows): la
    /// pagina de linea de tiempo de `pixpin_timeline::html`, con las fotos
    /// y los audios dentro. Swift la guarda como HTML o la imprime a PDF.
    /// Sin nada que exportar, error de argumento.
    pub fn timeline_exportar(
        &self,
        que: ExportarTimeline,
        desfase_ms: i64,
    ) -> Result<PaginaTimeline, ErrorNucleo> {
        let todos = self.momentos();
        let a = Almacen::en(self.ruta());
        let ahora = crate::ahora_ms();
        let hoy = Dia::de_instante(ahora, desfase_ms);
        let nada = || ErrorNucleo::Argumento {
            detalle: "no hay nada que exportar ahi".into(),
        };
        let (nombre, subtitulo, partes): (String, String, Vec<(Dia, Vec<&Momento>)>) = match &que {
            ExportarTimeline::Ultimas24h => (
                format!("timeline-24h-{}", hoy.iso()),
                "Últimas 24 horas".into(),
                partir_por_dias(dias::ultimas_24h(&todos, ahora), desfase_ms),
            ),
            ExportarTimeline::Buscar { consulta } => {
                let mut v = pixpin_timeline::buscar::buscar(&todos, consulta);
                // En la pagina, siempre de lo viejo a lo nuevo.
                v.sort_by_key(|m| (m.cuando, m.id.clone()));
                (
                    format!("timeline-busqueda-{}", hoy.iso()),
                    format!("Búsqueda: «{}»", consulta.trim()),
                    partir_por_dias(v, desfase_ms),
                )
            }
            ExportarTimeline::Dias { isos } => {
                let mut ds = isos
                    .iter()
                    .map(|i| dia_de_iso(i))
                    .collect::<Result<Vec<_>, _>>()?;
                ds.sort();
                ds.dedup();
                let (Some(p), Some(u)) = (ds.first().copied(), ds.last().copied()) else {
                    return Err(nada());
                };
                let (nombre, sub) = if p == u {
                    (format!("timeline-{}", p.iso()), dias::nombre_del_dia(p, false))
                } else {
                    (
                        format!("timeline-{}_{}", p.iso(), u.iso()),
                        format!(
                            "{} – {}",
                            dias::nombre_del_dia(p, false),
                            dias::nombre_del_dia(u, false)
                        ),
                    )
                };
                let partes = ds
                    .iter()
                    .map(|d| (*d, dias::del_dia(&todos, *d, desfase_ms)))
                    .collect();
                (nombre, sub, partes)
            }
        };
        let secciones: Vec<Seccion<'_>> = partes
            .into_iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(d, momentos)| Seccion {
                rotulo: dias::nombre_del_dia(d, false),
                momentos,
            })
            .collect();
        let cuantos: usize = secciones.iter().map(|s| s.momentos.len()).sum();
        if cuantos == 0 {
            return Err(nada());
        }
        let op = Opciones {
            titulo: "Timeline".into(),
            subtitulo,
            pie: format!(
                "Exportado con PixPin Max · {}",
                dias::nombre_del_dia(hoy, false)
            ),
            nota_de_voz: SIN_TITULO_VOZ.into(),
            desfase: desfase_ms,
            ingles: false,
            pestanas: ["Línea".into(), "Momentos".into(), "Calendario".into()],
            sobre_todo: "Sobre todo".into(),
        };
        let dentro = |n: &str| {
            if !nombre_de_medio_valido(n) {
                return None;
            }
            let bytes = std::fs::read(a.ruta(n)).ok()?;
            Some(pixpin_timeline::html::data_uri(n, &bytes))
        };
        Ok(PaginaTimeline {
            nombre,
            html: pixpin_timeline::html::pagina(&op, &secciones, &dentro, &dentro),
            cuantos: cuantos as u32,
        })
    }
}

/// Lo de `v` (en orden) cortado por dias, como `partir_por_dias` de Windows.
fn partir_por_dias(v: Vec<&Momento>, desfase_ms: i64) -> Vec<(Dia, Vec<&Momento>)> {
    let mut out: Vec<(Dia, Vec<&Momento>)> = Vec::new();
    for m in v {
        let d = Dia::de_instante(m.cuando, desfase_ms);
        match out.last_mut() {
            Some((x, l)) if *x == d => l.push(m),
            _ => out.push((d, vec![m])),
        }
    }
    out
}

fn pagina(n: &Nucleo, v: &[&Momento], desfase_ms: i64, desde: u64, cuantos: u32) -> PaginaMomentos {
    let a = Almacen::en(n.ruta());
    let total = v.len();
    let desde = (desde.min(total as u64)) as usize;
    let hasta = (desde + cuantos.min(TOPE_DE_PAGINA_TIMELINE) as usize).min(total);
    PaginaMomentos {
        momentos: v[desde..hasta]
            .iter()
            .map(|m| a_ficha(&a, m, desfase_ms))
            .collect(),
        desde: desde as u64,
        total: total as u64,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_medio_con_rutas_no_vale() {
        assert!(nombre_de_medio_valido("tl-1-0-1.png"));
        assert!(!nombre_de_medio_valido("../indice.json"));
        assert!(!nombre_de_medio_valido("a/b.png"));
        assert!(!nombre_de_medio_valido(""));
    }
}
