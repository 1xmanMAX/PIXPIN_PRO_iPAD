//! Los chats (proyectos) y sus mensajes, **por paginas**.
//!
//! Un chat es un proyecto de `proyectos/indice.json` y sus mensajes viven en
//! `proyectos/<id>/guardados.jsonl`, una linea de JSON por mensaje
//! (`pixpin_proyecto::cuaderno`). Aqui no se inventa nada de eso: se lee y se
//! escribe con las mismas funciones que el chat de Windows.
//!
//! # Como se lee una pagina sin leer el chat entero
//!
//! La primera vez que se pide una pagina de un chat se hace su **indice de
//! lineas**: donde empieza y cuanto mide cada mensaje visible en el fichero.
//! Para eso cada linea se mira con un lector minimo ([`Cabeza`]: solo `id` y
//! `numero`), sin construir el `Mensaje` entero ni sus cadenas. Despues,
//! cada pagina lee **solo su tramo** del fichero (una lectura contigua) y
//! convierte en `Mensaje` solo esas lineas. Pasar de pagina en un chat de
//! diez mil mensajes cuesta lo que cuestan cincuenta.
//!
//! El indice se guarda en una cache con tope ([`TOPE_DE_CHATS`]) y se da por
//! bueno mientras el fichero tenga el mismo tamano y la misma fecha; si
//! cambia (otro hilo escribio, llego algo al sincronizar), se rehace. Cada
//! entrada son 12 bytes por mensaje.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::SystemTime;

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Mensaje, Sello};

use crate::{ErrorNucleo, Nucleo};

/// Cuantos indices de chat se guardan a la vez. Ocho chats son de sobra para
/// ir y volver entre los abiertos; un indice de mas solo es un fichero que
/// se vuelve a recorrer.
pub const TOPE_DE_CHATS: usize = 8;

/// Lo mas que se devuelve en una pagina, pida Swift lo que pida: una pagina
/// enorme es leer el chat entero por la puerta de atras.
pub const TOPE_DE_PAGINA: u32 = 500;

/// Un chat en la lista.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FichaChat {
    pub id: String,
    pub nombre: String,
    /// Ultima vez que se toco, en milisegundos desde 1970. Ordena la lista.
    pub tocado: i64,
    /// La ultima linea, para ensenar debajo del nombre.
    pub resumen: String,
    pub hojas: u32,
    pub sin_leer: u32,
    /// «Mensajes guardados»: va siempre primero y no se renombra.
    pub es_guardados: bool,
    /// El chat del asistente local.
    pub es_asistente: bool,
    pub archivado: bool,
}

/// Un mensaje, con lo que hace falta para pintar su burbuja.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MensajeChat {
    pub id: String,
    /// Milisegundos desde 1970.
    pub cuando: i64,
    /// `NOTA`, `IMAGEN`, `ARCHIVO`, `VOZ`, `DIBUJO`, `PAGINA`, `PROYECTO`,
    /// `MINIAPP` o la palabra que traiga una version mas nueva del movil.
    pub clase: String,
    pub texto: String,
    pub nombre: String,
    /// Fichero del mensaje, relativo a la carpeta del chat.
    pub ruta: Option<String>,
    pub bytes: i64,
    /// Lo que se ensena en una linea (la transcripcion de una nota de voz,
    /// el texto o el nombre).
    pub resumen: String,
    /// `47·K7Q2`, `47a` o nada.
    pub codigo_chat: Option<String>,
    pub responde_a: Option<String>,
    pub emoji: Option<String>,
    pub fijado: bool,
    /// El dibujo o la hoja a la que apunta (un `DIBUJO`, una `PAGINA`).
    pub referencia: Option<String>,
    pub pagina: Option<u32>,
    /// Lo que dura el audio de una nota de voz, en milisegundos (0 si no es
    /// una o no se sabe).
    pub duracion_ms: i64,
    /// El texto de una nota de voz con sus minutos (`[0:05] lo dicho`), si
    /// ya se paso a texto. Ver [`crate::voz`].
    pub transcripcion: Option<String>,
    /// Los picos de la onda de una nota de voz: enteros de 0 a 32767, como
    /// mucho 256 (el campo `picos` del movil). Vacio si no los trae.
    pub picos: Vec<i32>,
    /// Llego al buzon (caduca si no se guarda: «Guardar esto»).
    pub en_buzon: bool,
    /// La hora del recordatorio (`recuerdaEn`, UTC en milisegundos).
    pub recuerda_en: Option<i64>,
    /// De que tipo es una mini-app (`tareas`, `tabla`...).
    pub miniapp: Option<String>,
    /// Lleva una leccion (no se renombra, ni se describe, ni se hace
    /// leccion otra vez).
    pub es_leccion: bool,
    /// Si el menu ofrece «Cambiar el nombre» (`renombrar::se_puede` de
    /// Windows).
    pub se_puede_renombrar: bool,
    /// Si el menu ofrece «Añadir descripción» (`descripcion::se_puede`).
    pub se_puede_describir: bool,
    /// El punto de la burbuja (`punto_de_proyecto`): `None` sin punto (una
    /// nota, una voz), verde (`true`) si vive tambien en las hojas de un
    /// proyecto, rojo si solo en el chat. Solo en las paginas del chat; un
    /// mensaje recien escrito lo trae vacio. Ver [`crate::burbujas`].
    pub punto: Option<bool>,
    /// De una foto anotada se ensena solo la foto (`soloLaFoto` del movil,
    /// por defecto si) o el dibujo entero.
    pub solo_la_foto: bool,
    /// De donde viene (`vieneDe`, la zona de un lienzo mandada al chat): el
    /// texto que se ensena debajo y el lienzo al que lleva.
    pub viene_de: Option<String>,
    pub viene_de_dibujo: Option<String>,
    /// De quien llego por Wi-Fi (`recibidoDe`): «Max phone · K7Q2».
    pub recibido_de: Option<String>,
    /// El lienzo donde dibujo el asistente al contestar (`lienzoIa`).
    pub lienzo_ia: Option<String>,
}

/// Una pagina de mensajes, del mas antiguo al mas nuevo.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaMensajes {
    pub mensajes: Vec<MensajeChat>,
    /// La posicion del primero de la pagina (0 es el mas antiguo).
    pub desde: u64,
    /// Cuantos mensajes visibles tiene el chat entero.
    pub total: u64,
    /// Lineas del cuaderno que no se entienden. Se dicen, no se callan: si
    /// faltan mensajes, el usuario tiene que saberlo.
    pub lineas_rotas: u64,
}

impl From<&almacen::Ficha> for FichaChat {
    fn from(f: &almacen::Ficha) -> Self {
        FichaChat {
            id: f.id.clone(),
            nombre: f.nombre.clone(),
            tocado: f.tocado,
            resumen: f.resumen.clone(),
            hojas: f.hojas,
            sin_leer: f.sin_leer,
            es_guardados: f.es_guardados(),
            es_asistente: f.es_asistente(),
            archivado: f
                .resto
                .get(pixpin_proyecto::vista::MARCA_ARCHIVADO)
                .and_then(|v| v.as_bool())
                == Some(true),
        }
    }
}

impl From<Mensaje> for MensajeChat {
    fn from(m: Mensaje) -> Self {
        let clase = m
            .clase
            .as_ref()
            .and_then(|c| serde_json::to_value(c).ok())
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let picos = crate::voz::picos_de(&m);
        let es_leccion = crate::lecciones::es_leccion(&m);
        let (viene_de, viene_de_dibujo) = crate::burbujas::viene_de(&m).unzip();
        MensajeChat {
            punto: None,
            solo_la_foto: crate::burbujas::solo_la_foto(&m),
            viene_de,
            viene_de_dibujo: viene_de_dibujo.flatten(),
            recibido_de: crate::burbujas::recibido_de(&m),
            lienzo_ia: crate::burbujas::lienzo_ia(&m),
            en_buzon: m.en_buzon,
            recuerda_en: hora_de_recordar(&m),
            se_puede_renombrar: se_puede_renombrar(&m),
            se_puede_describir: se_puede_describir(&m),
            es_leccion,
            miniapp: m.miniapp.clone(),
            resumen: m.resumen(),
            codigo_chat: m.codigo_chat(),
            duracion_ms: m.duracion_ms,
            transcripcion: m.transcripcion,
            picos,
            id: m.id,
            cuando: m.cuando,
            clase,
            texto: m.texto,
            nombre: m.nombre,
            ruta: m.ruta,
            bytes: m.bytes,
            responde_a: m.responde_a,
            emoji: m.emoji,
            fijado: m.fijado,
            referencia: m.referencia,
            pagina: m.pagina,
        }
    }
}

/// Lo minimo de una linea para indexarla: serde salta el resto sin
/// construirlo.
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct Cabeza {
    id: String,
    numero: i64,
    cuando: i64,
}

/// El indice de lineas de un cuaderno (ver la doc del modulo).
#[derive(Debug, Default)]
pub(crate) struct IndiceCuaderno {
    largo: u64,
    modificado: Option<SystemTime>,
    /// Inicio y largo en bytes de cada mensaje visible, **por su hora**
    /// (`cuando`), como los ensenan Windows y Android: el fichero se escribe
    /// anadiendo, pero un cuaderno que viajo entre aparatos puede venir con
    /// las lineas mezcladas.
    lineas: Vec<(u64, u32)>,
    /// El mayor `numero` de todos (tambien los de sistema): el siguiente
    /// mensaje lleva este mas uno (`Cuaderno::siguiente_numero`).
    pub(crate) mayor_numero: i64,
    lineas_rotas: u64,
}

impl IndiceCuaderno {
    fn de_bytes(texto: &[u8], largo: u64, modificado: Option<SystemTime>) -> IndiceCuaderno {
        let mut ind = IndiceCuaderno {
            largo,
            modificado,
            ..Default::default()
        };
        let mut horas: Vec<i64> = Vec::new();
        let mut inicio = 0usize;
        while inicio < texto.len() {
            let fin = texto[inicio..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(texto.len(), |p| inicio + p);
            let linea = &texto[inicio..fin];
            if !linea.iter().all(u8::is_ascii_whitespace) {
                match serde_json::from_slice::<Cabeza>(linea) {
                    Ok(c) => {
                        ind.mayor_numero = ind.mayor_numero.max(c.numero);
                        // Los de sistema (`auto-...`) existen y viajan, pero
                        // no son conversacion: no salen como burbujas.
                        if !c.id.starts_with(cuaderno::PREFIJO_DE_SISTEMA) {
                            ind.lineas.push((inicio as u64, linea.len() as u32));
                            horas.push(c.cuando);
                        }
                    }
                    Err(_) => ind.lineas_rotas += 1,
                }
            }
            inicio = fin + 1;
        }
        // Lo mas viejo arriba, como `ventana_chat` de Windows
        // (`mensajes.sort_by_key(|m| m.cuando)`, estable: a la misma hora,
        // el orden del fichero). Lo ya ordenado no se toca.
        if horas.windows(2).any(|w| w[0] > w[1]) {
            let mut orden: Vec<usize> = (0..ind.lineas.len()).collect();
            orden.sort_by_key(|&i| horas[i]);
            ind.lineas = orden.into_iter().map(|i| ind.lineas[i]).collect();
        }
        ind
    }
}

/// Los indices de los ultimos chats leidos, el mas reciente al final.
#[derive(Debug, Default)]
pub struct Cache {
    cuadernos: Vec<(PathBuf, Arc<IndiceCuaderno>)>,
}

impl Cache {
    pub(crate) fn vaciar(&mut self) {
        self.cuadernos = Vec::new();
    }

    pub(crate) fn quitar(&mut self, fichero: &Path) {
        self.cuadernos.retain(|(r, _)| r != fichero);
    }

    /// Cuantos indices hay guardados (para las pruebas del tope).
    pub fn cuantos(&self) -> usize {
        self.cuadernos.len()
    }

    /// El indice de `fichero`, rehecho si cambio. `None` si no existe.
    pub(crate) fn indice(
        &mut self,
        fichero: &Path,
    ) -> std::io::Result<Option<Arc<IndiceCuaderno>>> {
        let meta = match std::fs::metadata(fichero) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.quitar(fichero);
                return Ok(None);
            }
            Err(e) => return Err(e),
        };
        let modificado = meta.modified().ok();
        if let Some(n) = self.cuadernos.iter().position(|(r, _)| r == fichero) {
            let (_, ind) = &self.cuadernos[n];
            if ind.largo == meta.len() && ind.modificado == modificado {
                let par = self.cuadernos.remove(n);
                let ind = Arc::clone(&par.1);
                self.cuadernos.push(par);
                return Ok(Some(ind));
            }
            self.cuadernos.remove(n);
        }
        let texto = std::fs::read(fichero)?;
        let ind = Arc::new(IndiceCuaderno::de_bytes(
            &texto,
            texto.len() as u64,
            modificado,
        ));
        if self.cuadernos.len() >= TOPE_DE_CHATS {
            self.cuadernos.remove(0);
        }
        self.cuadernos
            .push((fichero.to_path_buf(), Arc::clone(&ind)));
        Ok(Some(ind))
    }
}

/// Un id de chat que no puede salir de `proyectos/`: viene de Swift, y un
/// `../` no puede decidir donde se lee o se escribe.
pub(crate) fn validar_chat(chat: &str) -> Result<(), ErrorNucleo> {
    if chat.is_empty() || chat == "." || chat == ".." || chat.contains(['/', '\\', '\0']) {
        return Err(ErrorNucleo::Argumento {
            detalle: format!("id de chat no valido: {chat:?}"),
        });
    }
    Ok(())
}

/// Las lineas `[desde, desde+cuantos)` del indice, ya como mensajes: una sola
/// lectura del tramo del fichero que las contiene. Cada uno con su punto de
/// proyecto (las senas de `raiz`, leidas una vez para toda la pagina).
fn leer_tramo(
    raiz: &Path,
    fichero: &Path,
    ind: &IndiceCuaderno,
    desde: usize,
    hasta: usize,
) -> std::io::Result<Vec<MensajeChat>> {
    if desde >= hasta {
        return Ok(Vec::new());
    }
    // Las lineas van por hora y no por sitio en el fichero: el tramo es el
    // que las abarca a todas (el de siempre si el fichero ya va en orden).
    let paso = &ind.lineas[desde..hasta];
    let inicio = paso.iter().map(|&(i, _)| i).min().unwrap_or(0);
    let fin = paso
        .iter()
        .map(|&(i, l)| i + u64::from(l))
        .max()
        .unwrap_or(inicio);
    let mut f = std::fs::File::open(fichero)?;
    f.seek(SeekFrom::Start(inicio))?;
    let mut tramo = vec![0u8; (fin - inicio) as usize];
    f.read_exact(&mut tramo)?;
    Ok(paso
        .iter()
        .filter_map(|&(i, l)| {
            let a = (i - inicio) as usize;
            serde_json::from_slice::<Mensaje>(&tramo[a..a + l as usize]).ok()
        })
        .map(|m| {
            let punto = crate::burbujas::punto(raiz, &m);
            MensajeChat {
                punto,
                ..MensajeChat::from(m)
            }
        })
        .collect())
}

/// La hora de un mensaje nuevo: la de ahora, pero siempre mayor que la del
/// anterior de este proceso. El id de una nota ES su hora en milisegundos
/// (`Mensaje::nota`), y dos notas en el mismo milisegundo chocarian.
pub(crate) fn hora_unica() -> i64 {
    static ULTIMA: AtomicI64 = AtomicI64::new(0);
    let ahora = crate::ahora_ms();
    let mut previa = ULTIMA.load(Ordering::SeqCst);
    loop {
        let nueva = ahora.max(previa + 1);
        match ULTIMA.compare_exchange(previa, nueva, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return nueva,
            Err(otra) => previa = otra,
        }
    }
}

#[uniffi::export]
impl Nucleo {
    /// Los chats, en el orden de la lista (lo ultimo tocado arriba y
    /// «Mensajes guardados» primero). Es el indice, pequeno: no se abre
    /// ningun proyecto.
    pub fn chats(&self) -> Result<Vec<FichaChat>, ErrorNucleo> {
        let indice = Indice::leer_si_esta(self.ruta())?;
        Ok(indice
            .ordenadas()
            .into_iter()
            .map(FichaChat::from)
            .collect())
    }

    /// Crea un chat vacio. No crea su carpeta: eso lo hace el primer
    /// mensaje, como en Windows.
    pub fn crear_chat(&self, nombre: String) -> Result<FichaChat, ErrorNucleo> {
        let aparato = self.identidad()?.yo.codigo();
        let ficha = almacen::Ficha::nueva(&nombre, crate::ahora_ms(), &aparato);
        // `leer_si_esta`: se reescribe la lista entera, y una que no se pudo
        // leer no puede pasar por vacia.
        let mut indice = Indice::leer_si_esta(self.ruta())?;
        indice.proyectos.push(ficha.clone());
        indice.guardar(self.ruta())?;
        Ok(FichaChat::from(&ficha))
    }

    /// «Mensajes guardados», creandolo si no esta.
    pub fn chat_guardados(&self) -> Result<FichaChat, ErrorNucleo> {
        let aparato = self.identidad()?.yo.codigo();
        let f = almacen::asegurar_guardados(self.ruta(), crate::ahora_ms(), &aparato)?;
        Ok(FichaChat::from(&f))
    }

    /// Una pagina de mensajes contando desde el mas antiguo: `desde` es la
    /// posicion del primero (0 el mas antiguo) y `cuantos` el largo (hasta
    /// [`TOPE_DE_PAGINA`]). Un chat sin nada escrito da una pagina vacia.
    pub fn mensajes(
        &self,
        chat: String,
        desde: u64,
        cuantos: u32,
    ) -> Result<PaginaMensajes, ErrorNucleo> {
        self.pagina(&chat, Some(desde), cuantos)
    }

    /// La ultima pagina: los `cuantos` mensajes mas nuevos. Es la que se pide
    /// al abrir un chat; las anteriores, con [`Nucleo::mensajes`] y el
    /// `desde` de esta.
    pub fn ultimos_mensajes(
        &self,
        chat: String,
        cuantos: u32,
    ) -> Result<PaginaMensajes, ErrorNucleo> {
        self.pagina(&chat, None, cuantos)
    }

    /// Escribe una nota en un chat: **una linea al final** de su cuaderno,
    /// sellada con el codigo de este aparato y el siguiente numero, y sube
    /// el chat en la lista. Igual que la caja de escribir de Windows
    /// (`ventana_chat::escribir_nota`).
    pub fn escribir_nota(
        &self,
        chat: String,
        texto: String,
        responde_a: Option<String>,
    ) -> Result<MensajeChat, ErrorNucleo> {
        self.escribir_nota_con(chat, texto, responde_a, |_| {})
            .map(MensajeChat::from)
    }
}

impl Nucleo {
    /// Lo de [`Nucleo::escribir_nota`], con un `retoque` del mensaje antes de
    /// escribir su linea (las respuestas del asistente: `aparato = "IA"`).
    pub(crate) fn escribir_nota_con(
        &self,
        chat: String,
        texto: String,
        responde_a: Option<String>,
        retoque: impl FnOnce(&mut Mensaje),
    ) -> Result<Mensaje, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta();
        let mut indice = Indice::leer_si_esta(raiz)?;
        if indice.buscar(&chat).is_none() {
            return Err(ErrorNucleo::NoExiste { que: chat });
        }
        let aparato = self.identidad()?.yo.codigo();
        let carpeta = almacen::carpeta(raiz, &chat);
        let fichero = carpeta.join("guardados.jsonl");
        // Con la cache tomada de principio a fin: dos notas a la vez en este
        // Nucleo no pueden sacar el mismo numero.
        let mut cache = self.cache_tomada();
        let numero = cache.indice(&fichero)?.map_or(0, |i| i.mayor_numero) + 1;
        let cuando = hora_unica();
        let mut m = Mensaje::nota(
            &texto,
            &Sello {
                cuando,
                numero,
                aparato,
                proyecto: chat.clone(),
            },
        );
        m.responde_a = responde_a;
        retoque(&mut m);
        cuaderno::anadir(&carpeta, &m)?;
        cache.quitar(&fichero);
        drop(cache);
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == chat) {
            f.tocado = cuando;
            f.resumen = m.resumen();
        }
        indice.guardar(raiz)?;
        Ok(m)
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Borra mensajes de un chat** (`ids`), como `borrar_mensajes` de
    /// Windows y `borrarAdjunto` del movil: se quitan del cuaderno, se deja
    /// **la marca de cada uno** para sincronizar (sin ella, la vuelta
    /// siguiente los traeria otra vez del otro aparato) y se borra el
    /// fichero de los que lo llevan (archivo, foto, voz). Un lienzo o una
    /// pagina NO se borran: son hojas del proyecto y otras cosas pueden
    /// apuntar a ellas. Devuelve cuantos se quitaron. Bloqueante.
    pub fn borrar_mensajes(&self, chat: String, ids: Vec<String>) -> Result<u32, ErrorNucleo> {
        use pixpin_proyecto::cuaderno::Clase;
        validar_chat(&chat)?;
        let raiz = self.ruta();
        let carpeta = almacen::carpeta(raiz, &chat);
        let fichero = carpeta.join("guardados.jsonl");
        if !fichero.exists() {
            return Ok(0);
        }
        let ids: std::collections::BTreeSet<String> = ids.into_iter().collect();
        let mut cache = self.cache_tomada();
        let quitados = cuaderno::quitar(&carpeta, &ids)?;
        cache.quitar(&fichero);
        drop(cache);
        if quitados.is_empty() {
            return Ok(0);
        }
        // Con la hora del reloj (UTC) y no la local: se compara con la del
        // movil.
        pixpin_proyecto::vista::anotar_borrados(raiz, &chat, &quitados, crate::ahora_ms())?;
        for m in &quitados {
            if !matches!(m.clase, Some(Clase::Archivo | Clase::Imagen | Clase::Voz)) {
                continue;
            }
            if let Some(r) = m.ruta.as_deref().and_then(|r| self.resolver(&chat, r))
                && r.is_file()
            {
                let _ = std::fs::remove_file(&r);
            }
        }
        // La ultima linea de la lista, la de lo que queda.
        let mut indice = Indice::leer(raiz);
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == chat) {
            let queda = cuaderno::Cuaderno::leer_de(&carpeta).unwrap_or_default();
            f.resumen = queda
                .mensajes
                .last()
                .map(Mensaje::resumen)
                .unwrap_or_default();
            indice.guardar(raiz)?;
        }
        Ok(quitados.len() as u32)
    }
}

// ── El menu de un mensaje (`menu_de_mensaje` de Windows) ──
//
// Lo que cambia un mensaje sin crear otro: fijarlo, ponerle etiqueta,
// describirlo, cambiarle el nombre, la hora de recordar y «Guardar esto».
// Todo se escribe con `cuaderno::cambiar`, que lee, cambia y reescribe con
// el cerrojo tomado y copia tal cual las lineas que no entiende: los mismos
// campos y el mismo formato que Windows y el movil, para que viajen.

/// El campo del recordatorio. Es el del movil y **no se traduce**: ya viaja
/// entre aparatos (`recordatorios::CAMPO` de Windows).
pub const CAMPO_RECORDAR: &str = "recuerdaEn";

/// Lo mas largo de un nombre (`renombrar::TOPE` de Windows, el del movil).
const TOPE_DE_NOMBRE: usize = 120;

/// Lo mas largo del nombre de un lienzo (`guardar_nombre_de_mensaje`).
const TOPE_DE_NOMBRE_LIENZO: usize = 80;

/// Las etiquetas que se le pueden poner a un mensaje, las del movil
/// (`ETIQUETAS` de `MensajesActivity.kt`) y en su orden: ⭐ ✅ ⏳ 💡 💰 📍.
pub const ETIQUETAS: [&str; 6] = [
    "\u{2b50}",
    "\u{2705}",
    "\u{23f3}",
    "\u{1f4a1}",
    "\u{1f4b0}",
    "\u{1f4cd}",
];

/// Lo que se le cambia a un mensaje desde su menu.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum CambioMensaje {
    /// «Fijar arriba» / «Quitar de arriba» (`fijado`).
    Fijar { fijado: bool },
    /// La etiqueta de la fila de emojis (`emoji`); `None` la quita.
    Etiqueta { emoji: Option<String> },
    /// La descripcion de una foto o un archivo (`texto`); vacia la quita.
    Descripcion { texto: String },
    /// El nombre que se ensena (`nombre`). El fichero no se toca: la
    /// sincronizacion empareja por su ruta.
    Nombre { nombre: String },
    /// La hora de avisar (`recuerdaEn`, UTC en ms); `None` la quita.
    Recordar { cuando: Option<i64> },
    /// «Guardar esto»: lo que llego al buzon se queda (`enBuzon = false`).
    Rescatar,
}

/// La hora de recordar que lleva un mensaje. Lo que no sea un entero (una
/// cadena, un `null` de otro aparato) es «no hay hora».
pub fn hora_de_recordar(m: &Mensaje) -> Option<i64> {
    m.resto.get(CAMPO_RECORDAR)?.as_i64()
}

/// Si a `m` se le puede cambiar el nombre (`renombrar::se_puede`): un
/// archivo con fichero o con hoja, que no sea una leccion ni del buzon.
pub fn se_puede_renombrar(m: &Mensaje) -> bool {
    use pixpin_proyecto::cuaderno::Clase;
    matches!(
        m.clase,
        Some(Clase::Imagen | Clase::Archivo | Clase::Voz | Clase::Dibujo | Clase::Pagina)
    ) && !m.en_buzon
        && (m.ruta.is_some() || m.referencia.is_some())
        && !crate::lecciones::es_leccion(m)
}

/// Si a `m` se le puede poner descripcion (`descripcion::se_puede`): una
/// foto o un archivo con su fichero, que no sea leccion ni del buzon.
pub fn se_puede_describir(m: &Mensaje) -> bool {
    use pixpin_proyecto::cuaderno::Clase;
    matches!(m.clase, Some(Clase::Imagen | Clase::Archivo))
        && !m.en_buzon
        && m.ruta.is_some()
        && !crate::lecciones::es_leccion(m)
}

/// `nuevo` limpio y **con la extension que tenia** `viejo` si no se la puso
/// (`renombrar::con_su_extension` de Windows): «informe.pdf» renombrado a
/// «Memoria» queda «Memoria.pdf». Vacio si no queda nada.
pub fn con_su_extension(viejo: &str, nuevo: &str) -> String {
    let limpio: String = nuevo
        .replace('\n', " ")
        .replace('/', "-")
        .trim()
        .chars()
        .take(TOPE_DE_NOMBRE)
        .collect();
    if limpio.is_empty() {
        return String::new();
    }
    let Some(ext) = extension(viejo) else {
        return limpio;
    };
    if limpio.to_lowercase().ends_with(&ext.to_lowercase()) {
        limpio
    } else {
        limpio + ext
    }
}

/// La extension de `nombre` con su punto: de una a cinco letras o cifras al
/// final (`\.[A-Za-z0-9]{1,5}$` en el movil).
fn extension(nombre: &str) -> Option<&str> {
    let punto = nombre.rfind('.')?;
    let ext = &nombre[punto + 1..];
    ((1..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric()))
        .then(|| &nombre[punto..])
}

/// Aplica `cambio` a `m`. `false` si no cambia nada (no se reescribe).
pub fn aplicar(m: &mut Mensaje, cambio: &CambioMensaje) -> bool {
    use pixpin_proyecto::cuaderno::Clase;
    match cambio {
        CambioMensaje::Fijar { fijado } => {
            let antes = m.fijado;
            m.fijado = *fijado;
            antes != *fijado
        }
        CambioMensaje::Etiqueta { emoji } => {
            let emoji = emoji.clone().filter(|e| !e.trim().is_empty());
            let antes = std::mem::replace(&mut m.emoji, emoji);
            antes != m.emoji
        }
        CambioMensaje::Descripcion { texto } => {
            // `descripcion::con_descripcion`: lo mismo de antes no reescribe.
            let limpio = texto.trim();
            if limpio == m.texto.trim() {
                return false;
            }
            m.texto = limpio.to_string();
            true
        }
        CambioMensaje::Nombre { nombre } => {
            // `guardar_nombre_de_mensaje`: un lienzo, recortado; lo demas,
            // con su extension.
            let nuevo = if m.clase == Some(Clase::Dibujo) {
                nombre.trim().chars().take(TOPE_DE_NOMBRE_LIENZO).collect()
            } else {
                con_su_extension(&m.nombre, nombre)
            };
            if nuevo.is_empty() || nuevo == m.nombre {
                return false;
            }
            m.nombre = nuevo;
            true
        }
        CambioMensaje::Recordar { cuando } => {
            let antes = hora_de_recordar(m);
            match cuando {
                Some(t) => {
                    m.resto
                        .insert(CAMPO_RECORDAR.to_string(), serde_json::Value::from(*t));
                }
                // Se BORRA la clave (no `null`): el mensaje vuelve a ser
                // byte a byte el de antes y no viaja entero en cada vuelta
                // (`recordatorios::quitar_hora`).
                None => {
                    m.resto.remove(CAMPO_RECORDAR);
                }
            }
            antes != *cuando
        }
        CambioMensaje::Rescatar => std::mem::replace(&mut m.en_buzon, false),
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Cambia un mensaje desde su menu** (fijar, etiqueta, descripcion,
    /// nombre, recordar, guardar del buzon), como `reescribir` del chat de
    /// Windows. Devuelve el mensaje como quedo, o `None` si ya no esta. Si
    /// no cambia nada, no se escribe. Bloqueante.
    pub fn cambiar_mensaje(
        &self,
        chat: String,
        id: String,
        cambio: CambioMensaje,
    ) -> Result<Option<MensajeChat>, ErrorNucleo> {
        validar_chat(&chat)?;
        let carpeta = almacen::carpeta(self.ruta(), &chat);
        let fichero = carpeta.join("guardados.jsonl");
        if !fichero.exists() {
            return Ok(None);
        }
        let mut cache = self.cache_tomada();
        let hecho = cuaderno::cambiar(&carpeta, &id, |m| aplicar(m, &cambio))?;
        cache.quitar(&fichero);
        Ok(hecho.map(MensajeChat::from))
    }

    /// **Reenvia mensajes a otro chat** (`reenviar` de Windows): una copia
    /// de cada uno al final de `destino`, del mas viejo al mas nuevo, con id
    /// y hora nuevos, su numero alli, un `uid` nuevo, este aparato, y
    /// `origen` el `uid` (o el id) del de aqui. Sin fijar, sin buzon y sin
    /// responder a nada. El fichero se copia a `destino` (lo que no esta en
    /// este equipo va sin el, como en el movil) y un lienzo lleva su dibujo.
    /// Devuelve cuantos se copiaron. Bloqueante.
    pub fn reenviar_mensajes(
        &self,
        chat: String,
        ids: Vec<String>,
        destino: String,
    ) -> Result<u32, ErrorNucleo> {
        use pixpin_proyecto::cuaderno::Clase;
        validar_chat(&chat)?;
        validar_chat(&destino)?;
        let raiz = self.ruta();
        let mut indice = Indice::leer_si_esta(raiz)?;
        if indice.buscar(&destino).is_none() {
            return Err(ErrorNucleo::NoExiste { que: destino });
        }
        let aparato = self.identidad()?.yo.codigo();
        let origen = cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, &chat))?;
        let quiero: std::collections::BTreeSet<&str> = ids.iter().map(String::as_str).collect();
        let mut elegidos: Vec<&Mensaje> = origen
            .mensajes
            .iter()
            .filter(|m| quiero.contains(m.id.as_str()))
            .collect();
        elegidos.sort_by_key(|m| m.cuando);
        if elegidos.is_empty() {
            return Ok(0);
        }
        let carpeta = almacen::carpeta(raiz, &destino);
        let fichero = carpeta.join("guardados.jsonl");
        let mut cache = self.cache_tomada();
        let mut numero = cache.indice(&fichero)?.map_or(0, |i| i.mayor_numero) + 1;
        let mut hechos = 0u32;
        let mut ultimo = (0, String::new());
        for m in elegidos {
            let cuando = hora_unica();
            let mut copia = m.clone();
            copia.id = format!("{cuando}");
            copia.cuando = cuando;
            copia.numero = numero;
            copia.uid = Some(pixpin_proyecto::codigos::nuevo());
            copia.aparato = Some(aparato.clone());
            copia.letra = None;
            copia.origen = m.uid.clone().or_else(|| Some(m.id.clone()));
            copia.proyecto = Some(destino.clone());
            copia.fijado = false;
            copia.en_buzon = false;
            copia.responde_a = None;
            if let Some(ruta) = m.ruta.as_deref() {
                match self.resolver(&chat, ruta).filter(|r| r.is_file()) {
                    Some(r) => {
                        let bytes = std::fs::read(&r)?;
                        let nombre = if m.nombre.trim().is_empty() {
                            r.file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "archivo".into())
                        } else {
                            m.nombre.clone()
                        };
                        copia.ruta = Some(almacen::guardar_adjunto(raiz, &destino, &nombre, &bytes)?);
                    }
                    None => copia.ruta = None,
                }
            }
            if m.clase == Some(Clase::Dibujo)
                && let Some(id) = m.referencia.as_deref().filter(|r| !r.is_empty())
            {
                let de = almacen::lienzo(raiz, &chat, id);
                let a = almacen::lienzo(raiz, &destino, id);
                if de.is_file() && !a.exists() {
                    if let Some(padre) = a.parent() {
                        std::fs::create_dir_all(padre)?;
                    }
                    std::fs::copy(&de, &a)?;
                }
            }
            cuaderno::anadir(&carpeta, &copia)?;
            ultimo = (cuando, copia.resumen());
            numero += 1;
            hechos += 1;
        }
        cache.quitar(&fichero);
        drop(cache);
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == destino) {
            f.tocado = ultimo.0;
            f.resumen = ultimo.1;
            indice.guardar(raiz)?;
        }
        Ok(hechos)
    }

    /// Las etiquetas de la fila de emojis, en su orden.
    pub fn etiquetas_de_mensaje(&self) -> Vec<String> {
        ETIQUETAS.iter().map(|e| e.to_string()).collect()
    }
}

impl Nucleo {
    fn pagina(
        &self,
        chat: &str,
        desde: Option<u64>,
        cuantos: u32,
    ) -> Result<PaginaMensajes, ErrorNucleo> {
        validar_chat(chat)?;
        let fichero = almacen::carpeta(self.ruta(), chat).join("guardados.jsonl");
        let ind = self.cache_tomada().indice(&fichero)?;
        let Some(ind) = ind else {
            return Ok(PaginaMensajes {
                mensajes: Vec::new(),
                desde: 0,
                total: 0,
                lineas_rotas: 0,
            });
        };
        let total = ind.lineas.len();
        let cuantos = cuantos.min(TOPE_DE_PAGINA) as usize;
        let desde = match desde {
            Some(d) => (d.min(total as u64)) as usize,
            None => total.saturating_sub(cuantos),
        };
        let hasta = (desde + cuantos).min(total);
        let mensajes = leer_tramo(self.ruta(), &fichero, &ind, desde, hasta)?;
        Ok(PaginaMensajes {
            mensajes,
            desde: desde as u64,
            total: total as u64,
            lineas_rotas: ind.lineas_rotas,
        })
    }
}
