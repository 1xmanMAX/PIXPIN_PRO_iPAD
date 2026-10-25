//! **El grafito como en Windows y el movil**: su mapa de casillas
//! (`pixpin_motor2d::tinta::grafito`), no una raya lisa. Va en el bufer
//! plano como una orden de imagen con un id que tiene el bit alto puesto
//! (`BIT`, el 61: el 63 y el 62 son del mosaico y la lupa): quien pinta pide sus pixeles con [`mapa_de_grafito`] una vez por
//! mapa (el id cambia cuando el mapa cambia).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::pintado::{self, Orden};
use pixpin_motor2d::tinta::grafito;
use pixpin_motor2d::Elemento;

/// El bit que dice «esto es un mapa de grafito» en el id de una imagen.
pub const BIT: u64 = 1 << 61;

/// Lo que puede pesar lo guardado antes de vaciarlo (los mapas se piden una
/// vez y se quedan en la cache de quien pinta).
const TOPE: usize = 192 << 20;

/// Los pixeles de un mapa de grafito, RGBA sin premultiplicar.
#[derive(Debug, Clone, uniffi::Record)]
pub struct MapaGrafito {
    pub rgba: Vec<u8>,
    pub ancho: u32,
    pub alto: u32,
}

struct Almacen {
    mapas: HashMap<u64, Arc<MapaGrafito>>,
    peso: usize,
}

fn almacen() -> &'static Mutex<Almacen> {
    static A: OnceLock<Mutex<Almacen>> = OnceLock::new();
    A.get_or_init(|| Mutex::new(Almacen { mapas: HashMap::new(), peso: 0 }))
}

/// Un numero para cada mapa distinto en todo el programa (la generacion del
/// horno es por hilo).
fn id_de(c: &grafito::Cocido) -> u64 {
    static VISTOS: OnceLock<Mutex<HashMap<(u64, u64, usize), u64>>> = OnceLock::new();
    static SIGUIENTE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let hilo = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        std::thread::current().id().hash(&mut h);
        h.finish()
    };
    let clave = (hilo, c.generacion, c.rgba.as_ptr() as usize);
    let mut v = VISTOS.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap_or_else(|e| e.into_inner());
    if v.len() > 4096 {
        v.clear();
    }
    *v.entry(clave).or_insert_with(|| SIGUIENTE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) | BIT
}

/// **La orden de un elemento de grafito**: su mapa como imagen, en su sitio
/// y con su giro. `None` si no es de grafito (va con sus ordenes de siempre).
pub fn orden(e: &Elemento) -> Option<Orden> {
    let c = grafito::cocer(e)?;
    let id = id_de(&c);
    {
        let mut a = almacen().lock().unwrap_or_else(|e| e.into_inner());
        if !a.mapas.contains_key(&id) {
            if a.peso > TOPE {
                a.mapas.clear();
                a.peso = 0;
            }
            a.peso += c.rgba.len();
            a.mapas.insert(id, Arc::new(MapaGrafito { rgba: c.rgba.clone(), ancho: c.ancho, alto: c.alto }));
        }
    }
    let (x0, y0, w, h) = c.caja();
    // El mapa gira alrededor de `centro`; la orden, alrededor del centro de
    // su caja: se lleva la caja a donde cae su centro ya girado.
    let (s, k) = c.angulo.sin_cos();
    let (mx, my) = (x0 + w / 2.0 - c.centro.x, y0 + h / 2.0 - c.centro.y);
    let (cx, cy) = (c.centro.x + mx * k - my * s, c.centro.y + mx * s + my * k);
    Some(Orden::Imagen {
        id_objeto: id,
        x: cx - w / 2.0,
        y: cy - h / 2.0,
        ancho: w,
        alto: h,
        opacidad: e.trazo.a * e.opacidad.clamp(0.0, 1.0),
        recorte: None,
        angulo: c.angulo,
    })
}

/// Las ordenes de una escena con el grafito como su mapa.
pub fn ordenes_de_escena(escena: &Escena) -> Vec<Orden> {
    let mut v = Vec::new();
    for e in escena.a_la_vista() {
        match orden(e) {
            Some(o) => v.push(o),
            None => v.extend(pintado::ordenes(e)),
        }
        v.extend(pintado::ordenes_medibles(e, escena.escala.as_ref(), ',', escena.fondo));
    }
    v
}

/// **Los pixeles de un mapa de grafito** (el id de su orden de imagen).
#[uniffi::export]
pub fn mapa_de_grafito(id: u64) -> Option<MapaGrafito> {
    almacen().lock().ok()?.mapas.get(&id).map(|m| (**m).clone())
}
