//! **Editar un lienzo desde Swift**: el `Gesto` y la `Escena` de
//! `pixpin_motor2d`, los mismos que mueve la ventana de Windows
//! (`por-traducir/pixpin/src/dibujo/mano.rs`), detras de un objeto de UniFFI.
//!
//! No hay logica de dibujo nueva: el puntero se traduce a
//! [`EventoGesto`] y lo demas lo decide el motor (que nace, que se elige,
//! que se mueve, deshacer). Lo unico que se hace aqui es lo que en Windows
//! hace la mano y no el motor: **la goma** (el motor la tiene en su lista
//! pero no borra nada con ella; se borra con `borrador::alcanzados_con_grupo`,
//! como el movil) y **lo de encima** (marco de lo elegido, tiradores,
//! marquesina, cursor del texto), copiado de `dibujo::pintar::ordenes_encima`.
//!
//! # Rendimiento (§4bis del plan)
//!
//! - El puntero llega en coordenadas del MUNDO, una llamada por aviso del
//!   sistema (ya agrupados por AppKit). Devuelve la [`EditorLienzo::version`]
//!   para que Swift sepa si repintar sin una segunda llamada.
//! - Pintar es **una llamada por fotograma** ([`EditorLienzo::plano_de_vista`]):
//!   solo lo que cae en pantalla, con la geometria de cada elemento guardada
//!   en la `cache::Cache` del motor por su version y su octava de aumento.
//!   Dibujar un trazo recalcula solo ese trazo.
//! - `version()` y `cambios()` son atomicos: leerlos no espera al cerrojo.
//! - Guardar ([`EditorLienzo::guardar`]) copia la escena sin su historial con
//!   el cerrojo tomado y escribe el JSON **sin** el: el hilo principal no
//!   espera a la serializacion. Swift lo llama fuera del hilo principal.
//!
//! # El formato
//!
//! Se guarda exactamente como Windows (`ventana_chat::guardar_hoja_dibujada`):
//! `excalidraw::con_escena` sobre el lienzo tal como se abrio y
//! `excalidraw::escribir`; lo ajeno sale intacto y en su sitio, a un `.tmp` y
//! despues se renombra. Siempre contra el lienzo ABIERTO: la escena numera
//! sus elementos segun ese lienzo y `con_escena` los empareja por ese orden.
//!
//! # Lo borrado deja lapida (`isDeleted`)
//!
//! `con_escena` ya marca `isDeleted` en lo que venia en el fichero, pero lo
//! NACIDO aqui solo lo escribe mientras sigue vivo: un elemento creado,
//! guardado (y quiza ya sincronizado) y despues borrado desaparecia del
//! fichero. Sin base comun, la fusion a tres bandas de `pixpin-sincro`
//! (`fusion::json`, figura por figura) lo resucitaba desde el otro aparato.
//! Ahora el editor recuerda lo nacido que llego a escribir
//! ([`Disco::nacidos`]) y, si ya no esta vivo, lo escribe con
//! `isDeleted: true` y la version subida, como Excalidraw y el movil
//! ([`anadir_lapidas`]). Lo nacido y borrado entre dos guardados no llego a
//! ningun otro aparato y no deja nada.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use pixpin_motor2d::Escena;
use pixpin_motor2d::borrador::{self, ModoBorrador};
use pixpin_motor2d::cache::Cache;
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo};
use pixpin_motor2d::excalidraw::{self, Entrada};
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta, Peticion, Region};
use pixpin_motor2d::lazo::{Lazo, ModoLazo};
use pixpin_motor2d::pintado::{self, Orden};
use pixpin_motor2d::texto::TeclaTexto;
use pixpin_motor2d::vector::Punto2;

use crate::editor_herramientas::ManoHerramientas;
use crate::editor_imagenes::{self, Pegada};
use crate::lienzo::{CajaLienzo, ImagenDelLienzo, LienzoPlano, empaquetar};
use crate::{ErrorNucleo, Nucleo};

/// Lo que el lienzo necesita preguntar al usuario tras soltar (el cajetin
/// de calibrar y el de la cota de Windows y el movil). Swift lo recoge con
/// [`EditorLienzo::tomar_peticion`] y contesta con
/// [`EditorLienzo::calibrar`] o [`EditorLienzo::dictar_cota`]; si se
/// cancela, no se llama a nada y el lienzo queda como estaba.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum PeticionLienzo {
    /// Cuanto mide de verdad la raya que se acaba de arrastrar con
    /// `escalar` (`largo_px`, en unidades del mundo).
    Calibrar { largo_px: f32 },
    /// El largo y el angulo de la cota `id` recien trazada: lo que mide ya
    /// (en la unidad de la escala, o en pixeles sin ella) para rellenar el
    /// cajetin.
    DictarCota {
        id: u64,
        largo: f32,
        grados: f32,
        /// La unidad de la escala del lienzo; vacia si no se ha calibrado.
        unidad: String,
    },
    /// **La foto de una zona** (F8): la caja `(x0, y0, x1, y1)` del mundo
    /// que se acaba de arrastrar con `zona`. Swift pinta lo que hay dentro
    /// y la devuelve con [`EditorLienzo::poner_copia_de_zona`].
    Zona { x0: f32, y0: f32, x1: f32, y1: f32 },
}

/// Lo que se mueve el cursor sin dejar de estar «quieto» para el gesto de
/// pararse (forma rapida), en pixeles de pantalla: el temblor de la mano
/// del movil (`forma_rapida::TEMBLOR_PX`). Swift reprograma su espera solo
/// cuando el cursor se aleja mas que esto.
#[uniffi::export]
pub fn forma_rapida_temblor_px() -> f32 {
    pixpin_motor2d::forma_rapida::TEMBLOR_PX
}

/// En que momento del gesto esta el puntero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FasePuntero {
    /// Boton pulsado, lapiz apoyado.
    Pulsar,
    /// Arrastrando con el boton pulsado.
    Mover,
    /// Boton soltado.
    Soltar,
    /// Se perdio el gesto (la ventana perdio el raton, Escape): se deshace
    /// lo que iba a medias.
    Cancelar,
}

/// Bit de `modificadores`: Mayusculas (angulos redondos, proporcion fija,
/// anadir a lo elegido).
pub const MOD_MAYUSCULAS: u32 = 1;
/// Bit de `modificadores`: Opcion (Alt en Windows: atar la flecha dentro).
pub const MOD_OPCION: u32 = 2;

/// Lo que dice Swift de una herramienta y la del motor. Nombres en
/// minusculas y sin tildes; varios nombres para la misma donde se dicen de
/// dos maneras.
fn herramienta_de(nombre: &str) -> Option<Herramienta> {
    Some(match nombre {
        "seleccion" | "mano" => Herramienta::Mano,
        "lapiz" | "tinta" => Herramienta::Lapiz,
        "grafito" => Herramienta::Grafito,
        "resaltador" => Herramienta::Resaltador,
        "rectangulo" => Herramienta::Rectangulo,
        "rombo" => Herramienta::Rombo,
        "elipse" => Herramienta::Elipse,
        // El arco y el numerito de serie, como en la barra de Windows (las
        // formas y lo que nombra): el motor ya los sabe trazar.
        "arco" => Herramienta::Arco,
        "serie" => Herramienta::Serie,
        "flecha" => Herramienta::Flecha,
        "flecha_codos" => Herramienta::FlechaCodos,
        "flecha_libre" => Herramienta::FlechaLibre,
        "linea" => Herramienta::Linea,
        "texto" => Herramienta::Texto,
        "borrador" | "goma" => Herramienta::Borrador,
        "lazo" => Herramienta::Lazo,
        // Las de medir (`medida` de Windows y Android).
        "cota" => Herramienta::Cota,
        "escalar" | "calibrar" => Herramienta::Escalar,
        "escala_grafica" => Herramienta::EscalaGrafica,
        // Las cuatro de construir (`dibujo::construir` de Windows).
        "relleno" | "bote" => Herramienta::Relleno,
        "recortar" => Herramienta::Recortar,
        "extender" => Herramienta::Extender,
        "punto" => Herramienta::Punto,
        // Las que tapan o senalan, enmarcan, eligen y arreglan de la barra
        // de Windows (`BOTONES_EDITOR`): unas nacen del gesto (el mosaico, el
        // marco) y las demas las lleva `editor_herramientas`.
        "mosaico" | "pixelar" => Herramienta::Mosaico,
        "lupa" => Herramienta::Lupa,
        "foco" => Herramienta::Foco,
        "laser" => Herramienta::Laser,
        "marco" => Herramienta::Marco,
        "zona" => Herramienta::Zona,
        "bolita" => Herramienta::Bolita,
        "nudo" => Herramienta::Nudo,
        "copiar_estilo" => Herramienta::CopiarEstilo,
        _ => return None,
    })
}

fn nombre_de(h: Herramienta) -> &'static str {
    match h {
        Herramienta::Mano => "seleccion",
        Herramienta::Lapiz => "lapiz",
        Herramienta::Grafito => "grafito",
        Herramienta::Resaltador => "resaltador",
        Herramienta::Rectangulo => "rectangulo",
        Herramienta::Rombo => "rombo",
        Herramienta::Elipse => "elipse",
        Herramienta::Arco => "arco",
        Herramienta::Serie => "serie",
        Herramienta::Flecha => "flecha",
        Herramienta::FlechaCodos => "flecha_codos",
        Herramienta::FlechaLibre => "flecha_libre",
        Herramienta::Linea => "linea",
        Herramienta::Texto => "texto",
        Herramienta::Borrador => "borrador",
        Herramienta::Lazo => "lazo",
        Herramienta::Cota => "cota",
        Herramienta::Escalar => "escalar",
        Herramienta::EscalaGrafica => "escala_grafica",
        Herramienta::Relleno => "relleno",
        Herramienta::Recortar => "recortar",
        Herramienta::Extender => "extender",
        Herramienta::Punto => "punto",
        Herramienta::Mosaico => "mosaico",
        Herramienta::Lupa => "lupa",
        Herramienta::Foco => "foco",
        Herramienta::Laser => "laser",
        Herramienta::Marco => "marco",
        Herramienta::Zona => "zona",
        Herramienta::Bolita => "bolita",
        Herramienta::Nudo => "nudo",
        Herramienta::CopiarEstilo => "copiar_estilo",
        _ => "otra",
    }
}

/// Lo que se toca con el cerrojo: la escena, el gesto y la cache de
/// geometria.
pub(crate) struct Estado {
    pub(crate) escena: Escena,
    pub(crate) gesto: Gesto,
    cache: Cache,
    /// La goma esta apoyada: borra lo que toca hasta soltar.
    pub(crate) borrando: bool,
    /// El aumento del ultimo puntero: las tolerancias del motor son de
    /// pixeles de pantalla.
    zoom: f32,
    /// Lo que el ultimo soltar pide preguntar (calibrar, dictar la cota).
    pide: Option<Peticion>,
    /// Donde estaba el puntero en el ultimo aviso (el gesto de pararse
    /// convierte el trazo con el cursor ahi).
    ultimo: Punto2,
    /// Las imagenes pegadas en esta sesion (ver `editor_imagenes`).
    pub(crate) pegadas: Vec<Pegada>,
    /// Su carpeta temporal, creada al pegar la primera.
    pub(crate) temporal: Option<PathBuf>,
    /// Lo que recuerdan entre avisos la bolita, la zona y las lupas (ver
    /// `editor_herramientas`).
    pub(crate) mano: ManoHerramientas,
    /// El papel de fondo (un pin, una foto, una pagina de PDF) si lo hay:
    /// `(ancho, alto)` en el mundo desde (0, 0) y su PNG para el SVG (ver
    /// `editor_exportar`).
    pub(crate) papel: Option<((f32, f32), Option<Vec<u8>>)>,
    /// Lo que se ve ahora `(x0, y0, x1, y1)` en el mundo (el ultimo
    /// fotograma): lo que dibuja el asistente cae en su centro.
    pub(crate) vista: Option<(f32, f32, f32, f32)>,
    /// **Lo nuevo a la escala del plano** (`plano_cad::anotar` de Windows,
    /// `escalaDeLoNuevo` de Android): `k` (uno entre el zoom del plano) y lo
    /// ultimo visto de cada elemento (grosor y letra). `None` fuera de un
    /// plano: el lienzo de siempre.
    pub(crate) a_escala: Option<(f32, std::collections::HashMap<u64, (f32, Option<f32>)>)>,
}

/// Lo que se toca al guardar (un cerrojo aparte: guardar no frena el
/// puntero).
struct Disco {
    /// Lo ultimo escrito (o leido): si sale igual, no se escribe.
    ultimo: String,
    /// `cambios` cuando se guardo por ultima vez.
    guardados: u64,
    /// **Lo nacido en esta sesion que llego al disco**, por su id de la
    /// escena, tal como se escribio la ultima vez (vivo o ya lapida): si deja
    /// de estar vivo se escribe su lapida (ver la doc del modulo).
    nacidos: BTreeMap<u64, (Elemento, serde_json::Value)>,
    /// Las imagenes pegadas que ya tienen su fichero en `imagenes/` y su
    /// entrada en `files`: `fileId` y cuando nacio. Se quedan aunque se
    /// borre la figura (su lapida las nombra).
    ficheros: BTreeMap<String, i64>,
}

/// **Un lienzo abierto para editar.** Se usa desde varios hilos: el puntero
/// y el pintado desde el principal, guardar desde uno de fondo.
#[derive(uniffi::Object)]
pub struct EditorLienzo {
    ruta: PathBuf,
    /// Contra que carpeta van las rutas relativas de `files`
    /// (`imagenes/<fileId>`): la del chat, o la del fichero suelto.
    carpeta: PathBuf,
    /// El lienzo tal como se abrio: contra el se guarda siempre.
    abierto: excalidraw::Lienzo,
    imagenes: Vec<ImagenDelLienzo>,
    estado: Mutex<Estado>,
    disco: Mutex<Disco>,
    version: AtomicU64,
    cambios: AtomicU64,
    /// Si es la tinta de una hoja de un documento (ver
    /// [`crate::documentos`]): lo que hay que hacer ademas al guardar.
    hoja: Option<HojaDoc>,
}

/// **Lo de la tinta de una hoja de un documento** (PDF, Word o PowerPoint
/// del lector): el marco que se escribe junto a ella al guardar y, en el
/// PDF de un proyecto, el dibujo que hay que apuntarle a su hoja.
pub(crate) struct HojaDoc {
    /// `anot-<uid>-p<n>.hoja` y la hoja en unidades del lector.
    pub marco: Option<(
        pixpin_proyecto::anotado::Base,
        pixpin_sincro::anotado::MarcoDeLaHoja,
    )>,
    /// `(raiz, ficha, pagina, huella)` del PDF de un proyecto.
    pub registrar: Option<(PathBuf, String, u32, String)>,
}

impl Drop for EditorLienzo {
    fn drop(&mut self) {
        // Las pegadas que no se guardaron se van con el editor.
        let st = self.estado.get_mut().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = st.temporal.take() {
            let _ = std::fs::remove_dir_all(t);
        }
    }
}

// ---------------------------------------------------------------------------
// Los lienzos abiertos (para el asistente)

/// **Los lienzos abiertos en una ventana**: lo que dibuja o edita el
/// asistente va al abierto y no al fichero (si no, al guardar la ventana
/// pisaria lo del asistente). Como `al_lienzo::EditorAbierto` de Windows,
/// pero aqui la escena es de Rust y el turno la toca directamente, con su
/// cerrojo. Se apuntan al abrirlos ([`Nucleo::editar_lienzo`]) y se quitan
/// al cerrarse la ventana ([`EditorLienzo::cerrado`]) o al soltarse.
struct Abierto {
    editor: std::sync::Weak<EditorLienzo>,
    /// La ultima vez que su ventana paso al frente (el mas reciente gana).
    al_frente: u64,
}

static ABIERTOS: Mutex<Vec<Abierto>> = Mutex::new(Vec::new());
static AL_FRENTE: AtomicU64 = AtomicU64::new(1);

fn abiertos() -> MutexGuard<'static, Vec<Abierto>> {
    let mut g = ABIERTOS.lock().unwrap_or_else(|e| e.into_inner());
    g.retain(|a| a.editor.strong_count() > 0);
    g
}

/// Apunta un lienzo recien abierto (ya al frente).
pub(crate) fn apuntar_abierto(editor: &Arc<EditorLienzo>) {
    abiertos().push(Abierto {
        editor: Arc::downgrade(editor),
        al_frente: AL_FRENTE.fetch_add(1, Ordering::SeqCst),
    });
}

/// El editor abierto de este fichero, si lo hay.
pub(crate) fn abierto_de(ruta: &Path) -> Option<Arc<EditorLienzo>> {
    abiertos().iter().filter_map(|a| a.editor.upgrade()).find(|e| e.ruta == ruta)
}

/// **El lienzo abierto de un chat que estuvo al frente el ultimo** (no la
/// tinta de un documento), de la carpeta de datos `raiz`. `None` si no hay
/// ninguno.
pub(crate) fn abierto_al_frente(raiz: &Path) -> Option<Arc<EditorLienzo>> {
    abiertos()
        .iter()
        .filter_map(|a| Some((a.al_frente, a.editor.upgrade()?)))
        .filter(|(_, e)| e.hoja.is_none() && e.carpeta.starts_with(raiz))
        .max_by_key(|(n, _)| *n)
        .map(|(_, e)| e)
}

impl EditorLienzo {
    /// Los ids de lo elegido ahora.
    pub(crate) fn elegidos(&self) -> Vec<u64> {
        self.tomar().gesto.seleccion.ids().to_vec()
    }

    /// El fichero del lienzo.
    pub(crate) fn fichero(&self) -> &Path {
        &self.ruta
    }

    /// La carpeta del chat del lienzo.
    pub(crate) fn carpeta(&self) -> &Path {
        &self.carpeta
    }

    /// **Lo que hace el asistente en la escena abierta**: con el cerrojo,
    /// y si cerro algun paso de deshacer, cuenta como cambio (se guarda y
    /// se repinta) igual que lo hecho con el raton.
    pub(crate) fn con_la_escena<R>(&self, f: impl FnOnce(&mut Escena, Option<(f32, f32, f32, f32)>) -> R) -> R {
        let (r, cambio) = {
            let mut st = self.tomar();
            let antes = (st.escena.pasos_cerrados(), st.escena.cuantos_visibles());
            let vista = st.vista;
            let r = f(&mut st.escena, vista);
            let cambio = (st.escena.pasos_cerrados(), st.escena.cuantos_visibles()) != antes;
            (r, cambio)
        };
        if cambio {
            self.subir_cambios();
            self.subir_version();
        }
        r
    }
}

#[uniffi::export]
impl EditorLienzo {
    /// Su ventana paso al frente: lo que dibuje el asistente viene aqui.
    pub fn al_frente(&self) {
        let n = AL_FRENTE.fetch_add(1, Ordering::SeqCst);
        for a in abiertos().iter_mut() {
            if a.editor.upgrade().is_some_and(|e| std::ptr::eq(&*e, self)) {
                a.al_frente = n;
            }
        }
    }

    /// Su ventana se cerro: el asistente ya no dibuja aqui.
    pub fn cerrado(&self) {
        abiertos().retain(|a| !a.editor.upgrade().is_some_and(|e| std::ptr::eq(&*e, self)));
    }
}

impl EditorLienzo {
    /// Abre un lienzo del disco sin ventana (el turno del asistente sobre
    /// un lienzo cerrado): se toca su escena y se guarda por el camino de
    /// siempre ([`EditorLienzo::guardar`]).
    pub(crate) fn sin_ventana(ruta: PathBuf, carpeta: PathBuf, imagenes: Vec<ImagenDelLienzo>) -> Result<EditorLienzo, ErrorNucleo> {
        let texto = std::fs::read_to_string(&ruta)?;
        EditorLienzo::nuevo(ruta, carpeta, &texto, Some(imagenes))
    }

    fn nuevo(
        ruta: PathBuf,
        carpeta: PathBuf,
        texto: &str,
        imagenes: Option<Vec<ImagenDelLienzo>>,
    ) -> Result<Self, ErrorNucleo> {
        let abierto = excalidraw::leer(texto).map_err(|e| ErrorNucleo::Lienzo {
            detalle: e.to_string(),
        })?;
        let mut escena = excalidraw::a_escena(&abierto);
        escena.siguiente_id = ids_nuevos_desde(escena.siguiente_id);
        let imagenes = imagenes.unwrap_or_else(|| {
            excalidraw::ficheros(&abierto)
                .into_iter()
                .map(|(id_objeto, ruta)| ImagenDelLienzo { id_objeto, ruta })
                .collect()
        });
        let mut gesto = Gesto::nuevo();
        gesto.tomar_herramienta(Herramienta::Lapiz);
        Ok(EditorLienzo {
            ruta,
            carpeta,
            // Lo que hay en el disco, escrito por nosotros: si nadie toca
            // nada, guardar no escribe (ni cambia la fecha que mira la
            // sincronizacion).
            imagenes,
            estado: Mutex::new(Estado {
                escena,
                gesto,
                cache: Cache::nueva(),
                borrando: false,
                zoom: 1.0,
                pide: None,
                ultimo: Punto2::nuevo(0.0, 0.0),
                pegadas: Vec::new(),
                temporal: None,
                mano: ManoHerramientas::default(),
                papel: None,
                vista: None,
                a_escala: None,
            }),
            disco: Mutex::new(Disco {
                ultimo: excalidraw::escribir(&abierto),
                guardados: 0,
                nacidos: BTreeMap::new(),
                ficheros: BTreeMap::new(),
            }),
            abierto,
            version: AtomicU64::new(1),
            cambios: AtomicU64::new(0),
            hoja: None,
        })
    }

    /// **La tinta de una hoja de un documento**: el fichero `ruta` (que
    /// puede no existir aun: nace vacio y no se escribe hasta que se anote),
    /// leido en las unidades `u` y traido a las del lector (1400 de ancho
    /// desde la esquina de la hoja), con los resaltadores reconocidos, como
    /// `Capa::leer_en` de Windows.
    pub(crate) fn de_hoja(
        ruta: PathBuf,
        u: crate::unidades::Unidades,
        hoja: HojaDoc,
    ) -> Result<Self, ErrorNucleo> {
        let texto = match std::fs::read_to_string(&ruta) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                excalidraw::escribir(&excalidraw::Lienzo::vacio())
            }
            Err(e) => return Err(e.into()),
        };
        // Uno que no se entiende no impide leer el documento (Windows: «capa
        // de tinta ilegible; se empieza vacia»). Pero tampoco se pisa: sin
        // ruta valida no se guarda nada encima.
        let (texto, ruta) = match excalidraw::leer(&texto) {
            Ok(_) => (texto, ruta),
            Err(_) => (
                excalidraw::escribir(&excalidraw::Lienzo::vacio()),
                ruta.with_extension("excalidraw.ilegible-no-se-pisa"),
            ),
        };
        // Las imagenes pegadas (si alguna vez se pegan en la tinta de un
        // documento) irian junto a la capa.
        let carpeta = ruta
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();
        let mut ed = EditorLienzo::nuevo(ruta, carpeta, &texto, Some(Vec::new()))?;
        {
            let st = ed.estado.get_mut().unwrap_or_else(|e| e.into_inner());
            crate::unidades::a_las_del_lector(&mut st.escena, u);
            crate::unidades::con_resaltadores(&mut st.escena);
        }
        ed.hoja = Some(hoja);
        Ok(ed)
    }

    /// El estado, aunque un hilo muriera con el tomado: la escena sigue
    /// siendo valida entre eventos.
    pub(crate) fn tomar(&self) -> MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn subir_version(&self) -> u64 {
        self.version.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub(crate) fn subir_cambios(&self) {
        self.cambios.fetch_add(1, Ordering::SeqCst);
    }

    /// Los `id_objeto` de las fotos que ya traia el lienzo.
    pub(crate) fn imagenes_abiertas(&self) -> impl Iterator<Item = u64> + '_ {
        self.imagenes.iter().map(|i| i.id_objeto)
    }

    /// **El lazo** (`dibujo::mano` de Windows): ni el gesto ni construir lo
    /// tocan; se traza mientras se arrastra y al soltar lo que queda ENTERO
    /// dentro pasa a ser lo elegido. Un clic suelto no coge nada (como
    /// pulsar en el aire). Devuelve si hay que repintar.
    fn lazo(st: &mut Estado, fase: FasePuntero, p: Punto2) -> bool {
        match fase {
            FasePuntero::Pulsar => {
                st.gesto.lazo = Some(Lazo::empezar(p));
                true
            }
            FasePuntero::Mover => st.gesto.lazo.as_mut().is_some_and(|l| l.mover(p)),
            FasePuntero::Soltar => match st.gesto.lazo.take() {
                Some(l) => {
                    let cogidos = l.atrapados(&st.escena.elementos, ModoLazo::Encerrar);
                    st.gesto.seleccion.poner_todos(cogidos);
                    true
                }
                None => false,
            },
            FasePuntero::Cancelar => st.gesto.lazo.take().is_some(),
        }
    }

    /// Lo que pide `pide` como lo ve Swift.
    fn peticion_para_swift(escena: &Escena, pide: Peticion) -> Option<PeticionLienzo> {
        match pide {
            Peticion::Calibrar { largo_px } => Some(PeticionLienzo::Calibrar { largo_px }),
            Peticion::DictarCota { id } => {
                let e = escena.buscar(id).filter(|e| !e.borrado)?;
                let escala = escena.escala.as_ref().filter(|e| e.valida());
                Some(PeticionLienzo::DictarCota {
                    id,
                    largo: pixpin_motor2d::medida::largo_en_unidades(e, escala),
                    grados: pixpin_motor2d::medida::angulo_de(e),
                    unidad: escala.map(|e| e.unidad.clone()).unwrap_or_default(),
                })
            }
        }
    }

    /// La goma en `p`: se lleva lo que toca y su grupo (como el movil), en
    /// el paso que abrio el pulsar. Devuelve si borro algo.
    fn borrar_en(st: &mut Estado, p: Punto2) -> bool {
        let modo = if st.gesto.modo_guia() {
            ModoBorrador::Guias
        } else {
            ModoBorrador::Dibujo
        };
        let mundo = st.gesto.mundo(&st.escena);
        let ids = pixpin_motor2d::impacto::con_mundo(mundo, || {
            borrador::alcanzados_con_grupo(&st.escena.elementos, p, modo, |e| e.extras.referencia)
        });
        let mut hecho = false;
        for id in ids {
            hecho |= st.escena.borrar_apuntando(id);
        }
        hecho
    }
}

/// **Desde donde cuentan los ids de lo que nace en esta sesion** del editor.
///
/// Lo nuevo se escribe como `pc<hex>` (`excalidraw::sellar`) y
/// `excalidraw::a_escena` deja el contador en el mayor `pc<n>` del fichero
/// mas uno. Si el PC y este Mac abren el mismo lienzo y cada uno dibuja algo
/// antes de sincronizar, los dos sacan el MISMO `pc<n+1>`: dos figuras con el
/// mismo id no se juntan una a una (`fusion::con_ids`) y una tapa a la otra
/// (lo vigila `sincro_dos_aparatos`). El movil no tiene el problema: sus ids
/// son 21 letras al azar. Aqui se salta un trecho al azar (de 2^24 a 2^32)
/// por encima del contador: el PC sigue contando de uno en uno desde el
/// mayor y el Mac cae lejos, y dos Mac caen en sitios distintos. Sigue
/// siendo `pc<hex>` de 16 cifras como mucho, que es lo que entienden Windows
/// y Android: el formato no cambia.
fn ids_nuevos_desde(contador: u64) -> u64 {
    const MINIMO: u64 = 1 << 24;
    let mut b = [0u8; 4];
    let azar = match getrandom::getrandom(&mut b) {
        Ok(()) => u64::from(u32::from_le_bytes(b)),
        // Sin azar del sistema, la hora: tambien cae lejos del PC.
        Err(_) => crate::ahora_ms() as u64,
    };
    let salto = MINIMO + azar % ((1u64 << 32) - MINIMO);
    // Lejos del techo: el contador sigue sumando uno a uno despues.
    contador.max(1).saturating_add(salto).min(u64::MAX >> 2)
}

/// Una copia de la escena para escribirla, **sin el historial** (que puede
/// pesar megas y `con_escena` no lo mira).
fn copia_para_guardar(e: &Escena) -> Escena {
    let mut c = Escena::nueva();
    c.version = e.version;
    c.siguiente_id = e.siguiente_id;
    c.elementos = e.elementos.clone();
    c.escala = e.escala.clone();
    c.fondo = e.fondo;
    c.alfileres = e.alfileres.clone();
    c.referencias_visibles = e.referencias_visibles;
    c.tablas = e.tablas.clone();
    c.origen_coordenadas = e.origen_coordenadas;
    c
}

/// Si dos cajas `(x0, y0, x1, y1)` se tocan.
fn se_cruzan(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3
}

/// **Lo que se pinta encima del dibujo**: el marco de lo elegido, los
/// tiradores, el cursor del texto, la marquesina, la pista del iman, la
/// figura a la que se ataria la flecha, el lazo, los angulos en vivo y los
/// clavos; y el rectangulo de la zona, la estela del laser y la bolita. Es
/// `dibujo::pintar::ordenes_encima` de Windows.
fn ordenes_encima(gesto: &Gesto, escena: &Escena, zoom: f32) -> Vec<Orden> {
    let escala = 1.0 / zoom.max(1e-6);
    let mut v = Vec::new();
    if gesto.marco_visible() {
        for id in gesto.seleccion.ids() {
            if let Some(marco) = pintado::marco_de_seleccion(escena, *id, escala) {
                v.push(marco);
            }
        }
    }
    if let Some(tiradores) = gesto.tiradores(escena, escala) {
        v.extend(tiradores.ordenes(escala));
    }
    v.extend(gesto.cursor_de_texto(escena, zoom));
    // La zona se pinta como la marquesina (el movil la pinta con su
    // `selectionBox`).
    let zona = gesto.zona.map(|(a, b)| pixpin_motor2d::zona::caja(a, b));
    for (x0, y0, x1, y1) in gesto.marquesina().into_iter().chain(zona) {
        v.push(Orden::Polilinea {
            puntos: vec![
                Punto2::nuevo(x0, y0),
                Punto2::nuevo(x1, y0),
                Punto2::nuevo(x1, y1),
                Punto2::nuevo(x0, y1),
                Punto2::nuevo(x0, y0),
            ],
            color: ColorRgba {
                r: 0.41,
                g: 0.40,
                b: 0.84,
                a: 0.9,
            },
            grosor: escala,
            estilo: EstiloTrazo::Discontinuo,
        });
    }
    if let Some(l) = &gesto.lazo {
        if let Some(relleno) = l.relleno() {
            v.push(relleno);
        }
        v.push(l.orden(escala));
    }
    if let Some(a) = gesto.anclaje_activo {
        v.push(pixpin_motor2d::enganche::pista(&a, zoom));
    }
    v.extend(gesto.resaltado_de_union(escena, zoom));
    if let Some(puntas) = gesto.tiradores_de_punta(escena, escala) {
        v.extend(puntas.ordenes(escala));
    }
    v.extend(crate::editor_construir::angulos_en_vivo(
        escena, gesto, zoom,
    ));
    v.extend(pixpin_motor2d::nudos::ordenes_de_clavos(escena, zoom));
    v.extend(gesto.laser.ordenes(zoom));
    v.extend(gesto.bolita.orden(zoom));
    v
}

/// **Las lapidas de lo nacido aqui** (ver la doc del modulo). `salida` es lo
/// que dio `con_escena`: sus primeras `abiertas` entradas son las del
/// lienzo abierto y las demas, lo nacido aqui que sigue vivo, cuyo
/// `elemento.id` es el de la escena. Lo que se escribio en otro guardado
/// (`nacidos`) y ya no esta vivo se anade con `isDeleted` y una version
/// mayor que la escrita; una lapida ya escrita sale igual. Lo nacido queda
/// en el orden de sus ids, como lo deja `con_escena`. Devuelve lo nacido tal
/// como queda escrito ahora (vivo o lapida), para el proximo guardado.
fn anadir_lapidas(
    salida: &mut excalidraw::Lienzo,
    abiertas: usize,
    copia: &Escena,
    nacidos: &BTreeMap<u64, (Elemento, serde_json::Value)>,
) -> BTreeMap<u64, (Elemento, serde_json::Value)> {
    let vivos: std::collections::HashSet<u64> = salida.entradas
        [abiertas.min(salida.entradas.len())..]
        .iter()
        .filter_map(|en| match en {
            Entrada::Nuestro { elemento, .. } => Some(elemento.id),
            Entrada::Ajeno(_) => None,
        })
        .collect();
    for (id, (escrito, original)) in nacidos {
        if vivos.contains(id) {
            continue;
        }
        let lapida = if escrito.borrado {
            // Ya era lapida: igual que la ultima vez.
            escrito.clone()
        } else {
            let mut e = copia
                .buscar(*id)
                .cloned()
                .unwrap_or_else(|| escrito.clone());
            e.borrado = true;
            // El fichero lo escribio con `max(1, version)`: la lapida tiene
            // que ser mas nueva para que nadie la tome por la vieja.
            e.version = e.version.max(escrito.version.max(1).saturating_add(1));
            e
        };
        salida.entradas.push(Entrada::Nuestro {
            elemento: lapida,
            original: Box::new(original.clone()),
        });
    }
    let cola = abiertas.min(salida.entradas.len());
    salida.entradas[cola..].sort_by_key(|en| match en {
        Entrada::Nuestro { elemento, .. } => elemento.id,
        Entrada::Ajeno(_) => u64::MAX,
    });
    salida.entradas[cola..]
        .iter()
        .filter_map(|en| match en {
            Entrada::Nuestro { elemento, original } => {
                Some((elemento.id, (elemento.clone(), (**original).clone())))
            }
            Entrada::Ajeno(_) => None,
        })
        .collect()
}

#[uniffi::export]
impl EditorLienzo {
    /// Abre un `.excalidraw` suelto para editarlo. Si no existe y `crear`,
    /// nace vacio (y se escribe ya, como `crear_lienzo` de Windows).
    #[uniffi::constructor]
    pub fn de_fichero(ruta: String, crear: bool) -> Result<Arc<EditorLienzo>, ErrorNucleo> {
        let ruta = PathBuf::from(ruta);
        let texto = match std::fs::read_to_string(&ruta) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && crear => {
                let t = excalidraw::escribir(&excalidraw::Lienzo::vacio());
                escribir_seguro(&ruta, &t)?;
                t
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ErrorNucleo::NoExiste {
                    que: ruta.display().to_string(),
                });
            }
            Err(e) => return Err(e.into()),
        };
        // Sus fotos, al lado: `imagenes/<fileId>` junto al fichero.
        let carpeta = ruta
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Ok(Arc::new(EditorLienzo::nuevo(ruta, carpeta, &texto, None)?))
    }

    /// El fichero que edita.
    pub fn ruta(&self) -> String {
        self.ruta.display().to_string()
    }

    /// **Cambia de herramienta**: `seleccion` (o `mano`), `lapiz` (o
    /// `tinta`), `grafito`, `resaltador`, `rectangulo`, `rombo`, `elipse`,
    /// `flecha`, `flecha_codos`, `flecha_libre`, `linea`, `texto`,
    /// `borrador` (o `goma`), `lazo`; las de medir `cota`, `escalar` (o
    /// `calibrar`) y `escala_grafica`; y las cuatro de construir `relleno`
    /// (o `bote`), `recortar`, `extender` y `punto`. Cierra el texto que se
    /// estuviera escribiendo.
    pub fn herramienta(&self, nombre: String) -> Result<(), ErrorNucleo> {
        let h = herramienta_de(&nombre).ok_or_else(|| ErrorNucleo::Argumento {
            detalle: format!("herramienta desconocida: {nombre:?}"),
        })?;
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        if !st.gesto.en_reposo() {
            // A mitad de un gesto (no deberia pasar: la barra no se pulsa
            // con el boton apretado en el lienzo): se cancela lo de a medias.
            st.gesto
                .evento(EventoGesto::Escape, &mut st.escena, 1.0 / st.zoom.max(1e-6));
        }
        if st.gesto.cerrar_texto(&mut st.escena) {
            self.subir_cambios();
        }
        if h != Herramienta::Mano {
            st.gesto.seleccion.limpiar();
        }
        st.gesto.lazo = None;
        st.gesto.zona = None;
        st.pide = None;
        st.mano.zona_pedida = None;
        // El cuentagotas empieza de nuevo cada vez que se coge: lo tomado
        // antes ya no es lo que se quiere copiar.
        st.gesto.estilo_tomado = None;
        st.gesto.tomar_herramienta(h);
        drop(guarda);
        self.subir_version();
        Ok(())
    }

    /// La herramienta puesta (los nombres de [`EditorLienzo::herramienta`]).
    pub fn herramienta_actual(&self) -> String {
        nombre_de(self.tomar().gesto.herramienta).to_string()
    }

    /// **Un aviso del puntero**, en coordenadas del MUNDO. `presion` es la
    /// del lapiz o del Force Touch (0..1), `None` con raton. `modificadores`
    /// son bits ([`MOD_MAYUSCULAS`], [`MOD_OPCION`]). `zoom`: pixeles por
    /// unidad del mundo, para las tolerancias del motor (que son de
    /// pantalla). Devuelve la version: si cambio, hay que repintar.
    pub fn puntero(
        &self,
        fase: FasePuntero,
        x: f32,
        y: f32,
        presion: Option<f32>,
        modificadores: u32,
        zoom: f32,
    ) -> u64 {
        let v = self.puntero_sin_escala(fase, x, y, presion, modificadores, zoom);
        if self.poner_a_escala() { self.subir_version() } else { v }
    }

    /// **Anotando un plano**: lo que nace (o cambia de grosor o letra desde
    /// el panel) con los tamanos de fabrica se pone a `k` (uno entre el zoom
    /// con que se ve el plano): un trazo «fino» se ve fino en la pantalla,
    /// como en Windows y Android. Lo que ya estaba no se toca.
    pub fn escala_de_lo_nuevo(&self, k: f32) {
        if !(k.is_finite() && k > 0.0) {
            return;
        }
        let mut st = self.tomar();
        let visto = match st.a_escala.take() {
            Some((_, v)) => v,
            None => st.escena.elementos.iter().map(|e| (e.id, medidas_de(e))).collect(),
        };
        st.a_escala = Some((k, visto));
    }

    /// Ver [`EditorLienzo::escala_de_lo_nuevo`]. Dice si cambio algo.
    pub(crate) fn poner_a_escala(&self) -> bool {
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        let letra = st.gesto.estilo.tamano_letra;
        let Some((k, visto)) = st.a_escala.as_mut() else { return false };
        let k = *k;
        let mut cambio = false;
        for e in st.escena.elementos.iter_mut() {
            let antes = visto.get(&e.id).copied();
            let (g, t) = medidas_de(e);
            if antes.is_none_or(|(g0, _)| (g0 - g).abs() > 1e-6) && grosor_de_fabrica(g) && (k - 1.0).abs() > 1e-6 {
                e.grosor *= k;
                cambio = true;
            }
            let letra_nueva = antes.is_none_or(|(_, t0)| t0.is_none_or(|t0| t.is_some_and(|t| (t0 - t).abs() > 1e-6)));
            if letra_nueva
                && let pixpin_motor2d::Figura::Texto { tam, .. } = &mut e.figura
                && letra_de_fabrica(*tam, letra)
                && (k - 1.0).abs() > 1e-6
            {
                *tam *= k;
                e.ancho *= k;
                e.alto *= k;
                cambio = true;
            }
            visto.insert(e.id, medidas_de(e));
        }
        cambio
    }

    fn puntero_sin_escala(
        &self,
        fase: FasePuntero,
        x: f32,
        y: f32,
        presion: Option<f32>,
        modificadores: u32,
        zoom: f32,
    ) -> u64 {
        if !(x.is_finite() && y.is_finite()) {
            return self.version();
        }
        let p = Punto2::nuevo(x, y);
        let shift = modificadores & MOD_MAYUSCULAS != 0;
        let alt = modificadores & MOD_OPCION != 0;
        let presion = presion.filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 1.0));
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        if zoom.is_finite() && zoom > 0.0 {
            st.zoom = zoom;
        }
        let escala = 1.0 / st.zoom;
        st.ultimo = p;
        if fase == FasePuntero::Pulsar {
            // Lo que no se contesto antes ya no vale.
            st.pide = None;
        }

        // El lazo: tampoco pasa por el gesto.
        if st.gesto.herramienta == Herramienta::Lazo {
            return if Self::lazo(st, fase, p) {
                self.subir_version()
            } else {
                self.version()
            };
        }

        // La goma: la lleva quien tiene la escena, como en Windows.
        if st.gesto.herramienta == Herramienta::Borrador {
            let (hecho, fin) = match fase {
                FasePuntero::Pulsar => {
                    st.borrando = true;
                    st.escena.abrir_paso();
                    (Self::borrar_en(st, p), false)
                }
                FasePuntero::Mover if st.borrando => (Self::borrar_en(st, p), false),
                FasePuntero::Mover => (false, false),
                FasePuntero::Soltar if st.borrando => {
                    let h = Self::borrar_en(st, p);
                    st.escena.cerrar_paso();
                    st.borrando = false;
                    (h, true)
                }
                FasePuntero::Soltar => (false, false),
                FasePuntero::Cancelar => {
                    if st.borrando {
                        st.escena.cancelar_paso();
                        st.borrando = false;
                        (true, true)
                    } else {
                        (false, false)
                    }
                }
            };
            if fin {
                self.subir_cambios();
            }
            return if hecho || fin {
                self.subir_version()
            } else {
                self.version()
            };
        }

        // La bolita, el cuentagotas, el laser, las varitas, la zona y el
        // arrastre de lo que mira una lupa (`dibujo::mano` de Windows).
        if let Some(hecho) = st
            .mano
            .atender(fase, p, st.zoom, &mut st.gesto, &mut st.escena)
        {
            if hecho.documento {
                self.subir_cambios();
            }
            return if hecho.repintar {
                self.subir_version()
            } else {
                self.version()
            };
        }

        let ev = match fase {
            FasePuntero::Pulsar => EventoGesto::Pulsar {
                p,
                shift,
                alt,
                presion,
            },
            FasePuntero::Mover => EventoGesto::Mover {
                p,
                shift,
                alt,
                presion,
            },
            FasePuntero::Soltar => EventoGesto::Soltar { p },
            FasePuntero::Cancelar => EventoGesto::Escape,
        };
        let r = st.gesto.evento(ev, &mut st.escena, escala);
        st.mano.tras_el_gesto(fase, &mut st.gesto);
        // El resaltador del lector cubre un renglon (`resaltador_de_renglon`
        // de Windows): se le pone al nacer.
        if self.hoja.is_some()
            && fase == FasePuntero::Pulsar
            && st.gesto.herramienta == Herramienta::Resaltador
            && let Some(id) = st.gesto.trazo_en_curso()
            && let Some(e) = st.escena.buscar_mut(id)
            && matches!(
                e.figura,
                pixpin_motor2d::elemento::Figura::Resaltador { .. }
            )
        {
            e.grosor = crate::unidades::GROSOR_RESALTADOR;
        }
        // **Las cuatro de construir**, despues del gesto y solo al pulsar
        // (`dibujo::mano::al_motor` de Windows): tres cambian lo que ya hay
        // en su propio paso; la cuarta remata el punto que acaba de nacer.
        let construido = fase == FasePuntero::Pulsar
            && crate::editor_construir::al_pulsar(&mut st.escena, &st.gesto, p, st.zoom);
        if let Some(pide) = r.pide {
            st.pide = Some(pide);
        }
        // Soltar cierra el paso; pulsar con el texto abre o cierra uno.
        if matches!(fase, FasePuntero::Soltar | FasePuntero::Cancelar)
            || (fase == FasePuntero::Pulsar && st.gesto.herramienta == Herramienta::Texto)
            || construido
        {
            self.subir_cambios();
        }
        if r.region != Region::Nada || fase != FasePuntero::Mover {
            self.subir_version()
        } else {
            self.version()
        }
    }

    /// Deshace el ultimo paso (no a mitad de un gesto).
    pub fn deshacer(&self) -> bool {
        self.viajar(true)
    }

    /// Rehace lo ultimo deshecho.
    pub fn rehacer(&self) -> bool {
        self.viajar(false)
    }

    /// Si hay algo que deshacer (para la barra).
    pub fn puede_deshacer(&self) -> bool {
        self.tomar().escena.hay_que_deshacer()
    }

    /// Borra lo elegido, en un paso. `false` si no habia nada elegido.
    pub fn borrar_seleccion(&self) -> bool {
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        if !st.gesto.en_reposo()
            || st.gesto.esta_escribiendo()
            || st.gesto.seleccion.ids().is_empty()
        {
            return false;
        }
        let escala = 1.0 / st.zoom;
        st.gesto
            .evento(EventoGesto::Suprimir, &mut st.escena, escala);
        self.subir_cambios();
        self.subir_version();
        true
    }

    /// Elige todo lo que se ve.
    pub fn seleccionar_todo(&self) {
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        if !st.gesto.en_reposo() {
            return;
        }
        st.gesto.tomar_herramienta(Herramienta::Mano);
        let escala = 1.0 / st.zoom;
        st.gesto
            .evento(EventoGesto::SeleccionarTodo, &mut st.escena, escala);
        drop(guarda);
        self.subir_version();
    }

    /// Si se esta escribiendo un texto: entonces las teclas son del texto
    /// (Swift se las manda con [`EditorLienzo::teclear`] y
    /// [`EditorLienzo::tecla`]) y no atajos.
    pub fn escribiendo(&self) -> bool {
        self.tomar().gesto.esta_escribiendo()
    }

    /// Mete texto en el que se esta escribiendo (lo que da `insertText` de
    /// AppKit, ya compuesto: tildes, emoji). `false` si no se escribia.
    pub fn teclear(&self, texto: String) -> bool {
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        let mut hecho = false;
        for c in texto.chars() {
            hecho |= st.gesto.escribir(c, &mut st.escena);
        }
        drop(guarda);
        if hecho {
            self.subir_cambios();
            self.subir_version();
        }
        hecho
    }

    /// Una tecla de edicion del texto: `izquierda`, `derecha`, `inicio`,
    /// `fin`, `retroceso`, `suprimir`, `entrar`; `escape` termina de
    /// escribir y deja el texto elegido.
    pub fn tecla(&self, nombre: String) -> bool {
        let t = match nombre.as_str() {
            "izquierda" => TeclaTexto::Izquierda,
            "derecha" => TeclaTexto::Derecha,
            "inicio" => TeclaTexto::Inicio,
            "fin" => TeclaTexto::Fin,
            "retroceso" => TeclaTexto::Retroceso,
            "suprimir" => TeclaTexto::Suprimir,
            "entrar" => TeclaTexto::Entrar,
            "escape" => {
                let mut guarda = self.tomar();
                let st = &mut *guarda;
                let hecho = st.gesto.cerrar_texto_con_teclado(&mut st.escena);
                if hecho {
                    self.subir_cambios();
                    self.subir_version();
                }
                return hecho;
            }
            _ => return false,
        };
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        let hecho = st.gesto.tecla_de_texto(t, &mut st.escena);
        if hecho {
            self.subir_cambios();
            self.subir_version();
        }
        hecho
    }

    /// Termina de escribir (un clic fuera, cerrar la ventana). Un texto
    /// vacio se quita.
    pub fn terminar_texto(&self) -> bool {
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        let hecho = st.gesto.cerrar_texto(&mut st.escena);
        if hecho {
            self.subir_cambios();
            self.subir_version();
        }
        hecho
    }

    /// **Sube con todo lo que cambia lo que se ve** (dibujo, lo elegido, el
    /// cursor del texto). Swift repinta solo si cambio. Atomico: no espera.
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }

    /// Sube con cada cambio del DOCUMENTO (para guardar solo, con calma).
    pub fn cambios(&self) -> u64 {
        self.cambios.load(Ordering::SeqCst)
    }

    /// Si hay cambios sin guardar.
    pub fn sin_guardar(&self) -> bool {
        let guardados = self
            .disco
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .guardados;
        self.cambios() != guardados
    }

    /// Todo el dibujo, sin lo de encima (miniatura, exportar, pruebas).
    pub fn plano(&self) -> LienzoPlano {
        let st = self.tomar();
        empaquetar(
            &crate::grafito_plano::ordenes_de_escena(&st.escena),
            st.escena.fondo,
            st.escena.caja(),
        )
    }

    /// **Lo que se ve con la camara** en `(x, y)` (el punto del mundo en la
    /// esquina de arriba a la izquierda), a `zoom` pixeles por unidad, en una
    /// vista de `ancho_px` x `alto_px`; y encima, lo elegido, los tiradores,
    /// la marquesina y el cursor del texto. Una llamada por fotograma; la
    /// geometria de lo que no cambio sale de la cache.
    pub fn plano_de_vista(
        &self,
        x: f32,
        y: f32,
        zoom: f32,
        ancho_px: f32,
        alto_px: f32,
    ) -> LienzoPlano {
        let zoom = if zoom.is_finite() && zoom > 0.0 {
            zoom
        } else {
            1.0
        };
        let camara = Camara { x, y, zoom };
        let ventana = camara.ventana(ancho_px.max(1.0), alto_px.max(1.0));
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        st.vista = Some(ventana);
        let mut ordenes: Vec<Orden> = Vec::new();
        for e in st
            .escena
            .a_la_vista()
            .filter(|e| se_cruzan(e.caja(), ventana))
        {
            match crate::grafito_plano::orden(e) {
                Some(o) => ordenes.push(o),
                None => ordenes.extend_from_slice(st.cache.ordenes(e, zoom)),
            }
            ordenes.extend_from_slice(st.cache.medibles(
                e,
                st.escena.escala.as_ref(),
                ',',
                st.escena.fondo,
            ));
            // Lo que ensenan dentro el mosaico y la lupa (`editor_tapado`).
            ordenes.extend(crate::editor_tapado::ordenes_tras(e));
        }
        ordenes.extend(ordenes_encima(&st.gesto, &st.escena, zoom));
        // La cache no crece con lo borrado o deshecho para siempre.
        let vivos = st.escena.cuantos_visibles();
        if st.cache.sobran(vivos) {
            st.cache.podar(&st.escena);
        }
        empaquetar(&ordenes, st.escena.fondo, st.escena.caja())
    }

    /// La caja de lo dibujado, para encajar la vista. `None` si esta vacio.
    pub fn caja(&self) -> Option<CajaLienzo> {
        self.tomar()
            .escena
            .caja()
            .map(|(x0, y0, x1, y1)| CajaLienzo { x0, y0, x1, y1 })
    }

    /// Cuantos elementos visibles tiene.
    pub fn cuantos_elementos(&self) -> u64 {
        self.tomar().escena.a_la_vista().count() as u64
    }

    /// Las fotos del lienzo (abierto desde un chat: rutas absolutas; desde
    /// un fichero suelto: las relativas del `.excalidraw`).
    pub fn imagenes(&self) -> Vec<ImagenDelLienzo> {
        self.imagenes.clone()
    }

    /// **Lo que hay que preguntar tras soltar**, una vez: el cajetin de
    /// calibrar (`escalar`) o el de la cota recien trazada (`cota`). `None`
    /// si nada. Swift lo mira tras cada soltar con esas herramientas.
    pub fn tomar_peticion(&self) -> Option<PeticionLienzo> {
        let mut st = self.tomar();
        if let Some((x0, y0, x1, y1)) = st.mano.zona_pedida.take() {
            return Some(PeticionLienzo::Zona { x0, y0, x1, y1 });
        }
        let pide = st.pide.take()?;
        Self::peticion_para_swift(&st.escena, pide)
    }

    /// **Si lo de encima se mueve solo** (la estela del laser, que se apaga
    /// sola): mientras diga que si, Swift repinta a cada fotograma aunque la
    /// version no cambie.
    pub fn animando(&self) -> bool {
        self.tomar().gesto.laser.vivo()
    }

    /// **Calibra**: la raya de `largo_px` (lo que dio
    /// [`PeticionLienzo::Calibrar`]) mide `valor` `unidad` de verdad. Desde
    /// ahora todas las cotas del lienzo hablan en esa unidad. Un paso de
    /// deshacer; `false` si la medida no puede ser cierta (no toca nada).
    pub fn calibrar(&self, largo_px: f32, valor: f32, unidad: String) -> bool {
        let unidad = unidad.trim();
        if unidad.is_empty() {
            return false;
        }
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        let hecho = st.gesto.calibrar(&mut st.escena, largo_px, valor, unidad);
        drop(guarda);
        if hecho {
            self.subir_cambios();
            self.subir_version();
        }
        hecho
    }

    /// **Aplica lo dictado a la cota `id`**: el largo en la unidad de la
    /// escala (o en pixeles sin ella) y el angulo como en un plano (0 a la
    /// derecha, 90 hacia arriba). Un paso de deshacer; `false` si no cambio.
    pub fn dictar_cota(&self, id: u64, largo: f32, grados: f32) -> bool {
        let mut st = self.tomar();
        let hecho = Gesto::dictar_cota(&mut st.escena, id, largo, grados);
        drop(st);
        if hecho {
            self.subir_cambios();
            self.subir_version();
        }
        hecho
    }

    /// Las unidades que se ofrecen al calibrar, de mas a menos uso.
    pub fn unidades_de_medida(&self) -> Vec<String> {
        pixpin_motor2d::medida::UNIDADES
            .iter()
            .map(|u| u.to_string())
            .collect()
    }

    /// **Cuanto esperar al gesto de pararse** en el trazo de ahora: el medio
    /// segundo largo del lapiz o el segundo del grafito. `None` si no se
    /// esta trazando a mano (entonces no hay forma rapida). Swift lo pide al
    /// pulsar y programa una sola espera, que reprograma cuando el cursor
    /// se aleja mas que [`forma_rapida_temblor_px`] y cancela al soltar.
    pub fn espera_forma_ms(&self) -> Option<u64> {
        let st = self.tomar();
        st.gesto.trazo_en_curso()?;
        Some(st.gesto.espera_de_la_forma_ms())
    }

    /// **El gesto de pararse**: el trazo a mano quieto con el boton pulsado
    /// se convierte en compas, rectangulo, linea o elipse
    /// (`Gesto::convertir_en_forma`), y lo que se arrastre despues la
    /// ajusta. Lo llama Swift cuando vence su espera. `true` si convirtio.
    pub fn forma_rapida(&self) -> bool {
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        if st.gesto.trazo_en_curso().is_none() {
            return false;
        }
        let escala = 1.0 / st.zoom.max(1e-6);
        let hecho = st
            .gesto
            .convertir_en_forma(&mut st.escena, st.ultimo, escala)
            .is_some();
        drop(guarda);
        if hecho {
            self.subir_version();
        }
        hecho
    }

    /// **Donde esta el cursor del texto** que se escribe, en el mundo (una
    /// raya vertical del alto del renglon). Para colocar la ventanita del
    /// metodo de entrada (`firstRect(forCharacterRange:)`). `None` si no se
    /// escribe.
    pub fn caja_del_cursor(&self) -> Option<CajaLienzo> {
        let st = self.tomar();
        let orden = st.gesto.cursor_de_texto(&st.escena, st.zoom)?;
        let Orden::Polilinea { puntos, .. } = orden else {
            return None;
        };
        let mut caja: Option<CajaLienzo> = None;
        for p in puntos {
            caja = Some(match caja {
                None => CajaLienzo {
                    x0: p.x,
                    y0: p.y,
                    x1: p.x,
                    y1: p.y,
                },
                Some(c) => CajaLienzo {
                    x0: c.x0.min(p.x),
                    y0: c.y0.min(p.y),
                    x1: c.x1.max(p.x),
                    y1: c.y1.max(p.y),
                },
            });
        }
        caja
    }

    /// Cuantos elementos hay elegidos (para la barra y las pruebas).
    pub fn cuantos_elegidos(&self) -> u64 {
        self.tomar().gesto.seleccion.ids().len() as u64
    }

    /// **Guarda** el `.excalidraw` como Windows y Android (ver la doc del
    /// modulo): las imagenes pegadas que se usan, a `imagenes/` con su
    /// entrada en `files`; lo nacido y borrado, con su lapida. Devuelve si
    /// escribio algo: sin cambios, o a mitad de un gesto, no escribe.
    /// Bloqueante: Swift lo llama fuera del hilo principal.
    pub fn guardar(&self) -> Result<bool, ErrorNucleo> {
        let mut disco = self.disco.lock().unwrap_or_else(|e| e.into_inner());
        let cambios = self.cambios();
        if cambios == disco.guardados {
            return Ok(false);
        }
        let (copia, pegadas) = {
            let st = self.tomar();
            // A medias no: al soltar sube `cambios` y se guarda entonces.
            if !st.gesto.en_reposo() || st.borrando {
                return Ok(false);
            }
            let pegadas: Vec<Pegada> = editor_imagenes::en_uso(&st.escena, &st.pegadas)
                .into_iter()
                .cloned()
                .collect();
            (copia_para_guardar(&st.escena), pegadas)
        };
        // Las fotos pegadas que se usan: su fichero en `imagenes/` (una vez)
        // y desde entonces su entrada en `files`.
        for p in &pegadas {
            let clave = pixpin_motor2d::enlace::id_de_texto(p.id_objeto);
            if disco.ficheros.contains_key(&clave) {
                continue;
            }
            editor_imagenes::copiar_si_falta(
                &p.temporal,
                &self.carpeta.join("imagenes").join(&clave),
            )?;
            disco.ficheros.insert(clave, ahora_ms());
        }
        let con_ficheros;
        let base = if disco.ficheros.is_empty() {
            &self.abierto
        } else {
            let mut l = self.abierto.clone();
            for (clave, creado) in &disco.ficheros {
                excalidraw::poner_fichero(
                    &mut l,
                    clave,
                    "image/png",
                    &format!("imagenes/{clave}"),
                    *creado,
                );
            }
            con_ficheros = l;
            &con_ficheros
        };
        let mut salida = excalidraw::con_escena(base, &copia);
        let nacidos = anadir_lapidas(&mut salida, base.entradas.len(), &copia, &disco.nacidos);
        let texto = excalidraw::escribir(&salida);
        if texto == disco.ultimo {
            disco.guardados = cambios;
            return Ok(false);
        }
        // El PDF de un proyecto: su hoja recibe el dibujo antes de escribirlo
        // (`para_escribir` de Windows), o lo guardado no seria de nadie.
        if let Some((raiz, ficha, pagina, huella)) =
            self.hoja.as_ref().and_then(|h| h.registrar.as_ref())
        {
            pixpin_proyecto::capas_del_pdf::dibujo_de_la_pagina(
                raiz,
                ficha,
                *pagina,
                huella,
                true,
                crate::ahora_ms(),
            );
        }
        escribir_seguro(&self.ruta, &texto)?;
        // Primero la tinta y despues su marco, cada uno por un temporal
        // (`DondeVa::guardar_capa`): la tinta va siempre en las unidades de
        // la hoja, y el marco lo dice.
        if let Some((base, propia)) = self.hoja.as_ref().and_then(|h| h.marco.as_ref()) {
            base.escribir_marco(propia)?;
        }
        disco.ultimo = texto;
        disco.guardados = cambios;
        disco.nacidos = nacidos;
        Ok(true)
    }

    /// **El color de lo que se dibuja a partir de ahora** (0..1), y el de lo
    /// elegido si hay algo elegido. Para la tinta del lector (lapiz y
    /// resaltador de colores).
    pub fn tinta_color(&self, r: f32, g: f32, b: f32, a: f32) {
        let c = ColorRgba {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
            a: a.clamp(0.0, 1.0),
        };
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        st.gesto.estilo.trazo = c;
        let ids: Vec<u64> = st.gesto.seleccion.ids().to_vec();
        let mut cambio = false;
        if !ids.is_empty() {
            st.escena.abrir_paso();
            for id in ids {
                if let Some(e) = st.escena.buscar_mut(id) {
                    e.trazo = c;
                    e.tocar();
                    cambio = true;
                }
            }
            st.escena.cerrar_paso();
        }
        drop(guarda);
        if cambio {
            self.subir_cambios();
        }
        self.subir_version();
    }

    /// **El grosor del lapiz** (`strokeWidth` de Excalidraw: 0,5, 1, 2...).
    pub fn tinta_grosor(&self, grosor: f32) {
        if !(grosor.is_finite() && grosor > 0.0) {
            return;
        }
        self.tomar().gesto.grosor_tinta = grosor.min(64.0);
        self.subir_version();
    }

    /// Si es la tinta de una hoja de un documento (ver
    /// [`Nucleo::anotar_hoja`]).
    pub fn es_de_documento(&self) -> bool {
        self.hoja.is_some()
    }
}

/// Milisegundos desde 1970, como fecha Excalidraw lo suyo.
fn ahora_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl EditorLienzo {
    /// Deshacer (`atras`) o rehacer, como `Gesto::evento` con
    /// `Deshacer`/`Rehacer` pero sabiendo si habia algo.
    fn viajar(&self, atras: bool) -> bool {
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        if !st.gesto.en_reposo() || st.borrando {
            return false;
        }
        // Escribiendo, deshacer cierra antes el texto (como un clic fuera).
        let cerrado = st.gesto.cerrar_texto(&mut st.escena);
        let hecho = if atras {
            let h = st.escena.deshacer();
            // Lo elegido puede ya no existir.
            st.gesto.seleccion.limpiar();
            h
        } else {
            st.escena.rehacer()
        };
        drop(guarda);
        if hecho || cerrado {
            self.subir_cambios();
            self.subir_version();
        }
        hecho
    }
}

/// Escribe a un `.tmp` al lado y lo pone encima: si se va la luz a media
/// escritura, queda el dibujo viejo entero (igual que Windows).
fn escribir_seguro(ruta: &Path, texto: &str) -> Result<(), ErrorNucleo> {
    if let Some(padre) = ruta.parent() {
        std::fs::create_dir_all(padre)?;
    }
    let temporal = ruta.with_extension("excalidraw.tmp");
    std::fs::write(&temporal, texto)?;
    std::fs::rename(&temporal, ruta)?;
    Ok(())
}

#[uniffi::export]
impl Nucleo {
    /// **Abre para editar** el lienzo `dibujo` de un chat (la `referencia`
    /// de un mensaje `DIBUJO`). Sus fotos salen ya con la ruta absoluta.
    pub fn editar_lienzo(
        &self,
        chat: String,
        dibujo: String,
    ) -> Result<Arc<EditorLienzo>, ErrorNucleo> {
        validar_nombre(&chat)?;
        validar_nombre(&dibujo)?;
        let ruta = pixpin_proyecto::almacen::lienzo(self.ruta(), &chat, &dibujo);
        let texto = match std::fs::read_to_string(&ruta) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ErrorNucleo::NoExiste {
                    que: ruta.display().to_string(),
                });
            }
            Err(e) => return Err(e.into()),
        };
        let leido = excalidraw::leer(&texto).map_err(|e| ErrorNucleo::Lienzo {
            detalle: e.to_string(),
        })?;
        let imagenes = excalidraw::ficheros(&leido)
            .into_iter()
            .filter_map(|(id_objeto, rel)| {
                let r = self.resolver(&chat, &rel)?;
                Some(ImagenDelLienzo {
                    id_objeto,
                    ruta: r.display().to_string(),
                })
            })
            .collect();
        let carpeta = pixpin_proyecto::almacen::carpeta(self.ruta(), &chat);
        let editor = Arc::new(EditorLienzo::nuevo(
            ruta,
            carpeta,
            &texto,
            Some(imagenes),
        )?);
        apuntar_abierto(&editor);
        Ok(editor)
    }

    /// **Un lienzo nuevo en un chat**: el `.excalidraw` vacio en
    /// `lienzos/<codigo>.excalidraw` y su mensaje `DIBUJO` al final del
    /// cuaderno, como `ventana_chat::escribir_lienzo` de Windows. Se abre
    /// despues con [`Nucleo::editar_lienzo`] y la `referencia` del mensaje.
    pub fn crear_lienzo(
        &self,
        chat: String,
        nombre: Option<String>,
    ) -> Result<crate::MensajeChat, ErrorNucleo> {
        self.crear_lienzo_con(chat, nombre, |_| {})
            .map(crate::MensajeChat::from)
    }
}

impl Nucleo {
    /// Lo de [`Nucleo::crear_lienzo`], con un `retoque` del mensaje antes de
    /// escribirlo (el lienzo propio del asistente: `aparato = "IA"`).
    pub(crate) fn crear_lienzo_con(
        &self,
        chat: String,
        nombre: Option<String>,
        retoque: impl FnOnce(&mut pixpin_proyecto::cuaderno::Mensaje),
    ) -> Result<pixpin_proyecto::cuaderno::Mensaje, ErrorNucleo> {
        use pixpin_proyecto::{almacen, cuaderno};
        crate::chats::validar_chat(&chat)?;
        let raiz = self.ruta();
        let mut indice = almacen::Indice::leer_si_esta(raiz)?;
        if indice.buscar(&chat).is_none() {
            return Err(ErrorNucleo::NoExiste { que: chat });
        }
        let aparato = self.identidad()?.yo.codigo();
        let id = pixpin_proyecto::codigos::nuevo();
        let ruta = almacen::lienzo(raiz, &chat, &id);
        let json = excalidraw::escribir(&excalidraw::Lienzo::vacio());
        escribir_seguro(&ruta, &json)?;

        let carpeta = almacen::carpeta(raiz, &chat);
        let fichero_cuaderno = carpeta.join("guardados.jsonl");
        let mut cache = self.cache_tomada();
        let numero = cache
            .indice(&fichero_cuaderno)?
            .map_or(0, |i| i.mayor_numero)
            + 1;
        let cuando = crate::chats::hora_unica();
        let fichero = format!("{id}.excalidraw");
        let mut m = cuaderno::Mensaje::adjunto(
            cuaderno::Clase::Dibujo,
            &fichero,
            &format!("lienzos/{fichero}"),
            json.len() as i64,
            &cuaderno::Sello {
                cuando,
                numero,
                aparato,
                proyecto: chat.clone(),
            },
        );
        m.referencia = Some(id);
        // El mismo tope que al renombrar a mano en Windows.
        if let Some(n) = nombre.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            m.nombre = n.chars().take(80).collect();
        }
        retoque(&mut m);
        if let Err(e) = cuaderno::anadir(&carpeta, &m) {
            let _ = std::fs::remove_file(&ruta);
            return Err(e.into());
        }
        cache.quitar(&fichero_cuaderno);
        drop(cache);
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == chat) {
            f.tocado = cuando;
            f.resumen = m.resumen();
        }
        indice.guardar(raiz)?;
        Ok(m)
    }
}

/// Un nombre de chat o de dibujo que no puede salir de su carpeta.
fn validar_nombre(parte: &str) -> Result<(), ErrorNucleo> {
    if parte.is_empty() || parte == "." || parte == ".." || parte.contains(['/', '\\', '\0']) {
        return Err(ErrorNucleo::Argumento {
            detalle: format!("nombre no valido: {parte:?}"),
        });
    }
    Ok(())
}

/// Los grosores del panel (fino, medio y grueso de formas, lapiz y
/// resaltador): solo esos se ponen a la escala del plano.
fn grosor_de_fabrica(g: f32) -> bool {
    use pixpin_motor2d::estilo::NivelGrosor as N;
    [N::Fino, N::Medio, N::Grueso].iter().any(|n| [n.de_forma(), n.de_tinta(), n.de_resaltador()].iter().any(|v| (v - g).abs() < 1e-4))
}

/// Lo mismo con la letra: los tamanos del panel y el elegido.
fn letra_de_fabrica(t: f32, la_elegida: f32) -> bool {
    (t - la_elegida).abs() < 1e-4 || [16.0f32, 20.0, 28.0, 36.0].iter().any(|v| (v - t).abs() < 1e-4)
}

fn medidas_de(e: &pixpin_motor2d::Elemento) -> (f32, Option<f32>) {
    let letra = match &e.figura {
        pixpin_motor2d::Figura::Texto { tam, .. } => Some(*tam),
        _ => None,
    };
    (e.grosor, letra)
}
