//! **Los marcadores del lienzo con su vista**, para Swift: lo que en Windows
//! hace `apps/pixpin/src/ventana_editor/marcas.rs` (el `Marcador`) sin lo
//! de pintar ni lo de Win32. Las cuentas son las de siempre, en
//! `pixpin_motor2d::marcas` (el orden, la linea que se guarda, la
//! [`VistaDeMarca`], el [`Vuelo`]); aqui queda donde se guarda cada cosa y
//! el estado de la ventana: la lista, las vistas y el viaje de la camara.
//!
//! # Donde se guardan (igual que Windows, para que sincronice)
//!
//! - Un lienzo de un chat (`…/<ficha>/lienzos/<d>.excalidraw`): las marcas
//!   en `pins/draw/<d>.marcas` del chat (`id:x:y:emoji|…`, la linea del
//!   movil) y la vista de cada una en `pins/draw/<d>.vistas.json`
//!   (`{"<id>": "x y ancho alto"}`, §3.8 de
//!   `docs/sincronizar-marcos-vistas-y-tinta.md`). Viajan con el dibujo.
//!   Vaciarlas deja el fichero vacio (`""` y `{}`): uno que desaparece no le
//!   dice nada al otro aparato.
//! - Un `.excalidraw` suelto: `<dibujo>.pixpin-marcas` y
//!   `<dibujo>.pixpin-vistas` al lado, que no viajan y se borran al
//!   vaciarse.
//!
//! # Lo que llega de otro aparato
//!
//! [`MarcasDelLienzo::releer`] vuelve a leer los dos ficheros (Swift lo
//! llama al traer la ventana al frente) e ir a una marca relee antes su
//! vista: una vista cambiada en el PC con Mayus+clic se respeta aqui sin
//! cerrar el lienzo.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::marcas::{self, EMOJIS, Marca, VistaDeMarca, Vuelo};
use pixpin_motor2d::vector::Punto2;
use pixpin_proyecto::anotado as an;

/// Por debajo de la barra: la marca sin vista a la que se va queda a esto
/// del borde de abajo de la barra (`AIRE_BAJO_LA_BARRA` de Windows).
pub const AIRE_BAJO_LA_BARRA: f32 = 48.0;

/// Los topes del aumento del editor del Mac (`VistaEditorLienzoNS.zoomMinimo`
/// y `zoomMaximo`, los de la rueda y el pellizco): la vista de una marca se
/// encaja con ellos y no con los de la camara (Windows 64af3c3), para que la
/// primera muesca de alejar despues de llegar no acerque de golpe.
pub const ZOOM_MINIMO_DEL_EDITOR: f32 = 0.05;
pub const ZOOM_MAXIMO_DEL_EDITOR: f32 = 30.0;

/// Una marca del lienzo como la ve Swift.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MarcaDelLienzo {
    /// Cuando se puso, en milisegundos: es su identidad.
    pub id: i64,
    /// Su sitio, en coordenadas del mundo del lienzo.
    pub x: f64,
    pub y: f64,
    pub emoji: String,
    /// Si guarda la vista que se veia al ponerla.
    pub con_vista: bool,
}

/// La camara de la vista de Swift: el punto del mundo en la esquina de
/// arriba a la izquierda y los puntos de pantalla por unidad.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct CamaraDelLienzo {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl From<CamaraDelLienzo> for Camara {
    fn from(c: CamaraDelLienzo) -> Self {
        Camara {
            x: c.x,
            y: c.y,
            zoom: c.zoom,
        }
    }
}

impl From<Camara> for CamaraDelLienzo {
    fn from(c: Camara) -> Self {
        CamaraDelLienzo {
            x: c.x,
            y: c.y,
            zoom: c.zoom,
        }
    }
}

/// El fichero de las marcas de un dibujo (`ruta_de_marcas` de Windows): en
/// un lienzo de un chat, junto a su dibujo del movil, con las de antes
/// pasadas la primera vez; si no, `<dibujo>.pixpin-marcas` al lado.
pub fn ruta_de_marcas(dibujo: &Path) -> PathBuf {
    let de_siempre = hermano(dibujo, ".pixpin-marcas");
    match an::marcas_del_lienzo(dibujo) {
        Some(nueva) => {
            an::copiar_si_falta(&de_siempre, &nueva);
            nueva
        }
        None => de_siempre,
    }
}

/// El fichero de la vista de cada marca (`ruta_de_vistas` de Windows).
pub fn ruta_de_vistas(dibujo: &Path) -> PathBuf {
    an::vistas_del_lienzo(dibujo).unwrap_or_else(|| hermano(dibujo, ".pixpin-vistas"))
}

fn hermano(dibujo: &Path, terminacion: &str) -> PathBuf {
    let mut nombre = dibujo
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "dibujo".into());
    nombre.push_str(terminacion);
    dibujo.with_file_name(nombre)
}

/// Las marcas de ese fichero. Sin fichero, o roto, ninguna.
fn leer_marcas(ruta: &Path) -> Vec<Marca> {
    std::fs::read_to_string(ruta)
        .map(|t| marcas::de_texto(t.trim()))
        .unwrap_or_default()
}

/// Las guarda como Windows (`escribir`): la que viaja se deja vacia al
/// vaciarla; la de al lado se borra.
fn escribir_marcas(ruta: &Path, lista: &[Marca]) -> std::io::Result<()> {
    if !ruta.to_string_lossy().ends_with(".pixpin-marcas") {
        return an::escribir(ruta, &marcas::a_texto(lista));
    }
    if lista.is_empty() {
        return match std::fs::remove_file(ruta) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    an::escribir(ruta, &marcas::a_texto(lista))
}

/// Las vistas de un `.vistas.json`, por id. Lo que no se entiende (un id
/// que no es un numero, un texto que no es una vista) se salta: esa marca
/// vuelve como antes, sin vista.
pub fn vistas_de_texto(t: &str) -> BTreeMap<i64, VistaDeMarca> {
    serde_json::from_str::<BTreeMap<String, serde_json::Value>>(t)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(id, v)| {
            Some((
                id.trim().parse::<i64>().ok()?,
                VistaDeMarca::de_texto(v.as_str()?)?,
            ))
        })
        .collect()
}

/// Y de vuelta, exactamente como lo escribe Windows: las claves en texto y
/// ordenadas como texto, con `to_string_pretty`.
pub fn vistas_a_texto(vistas: &BTreeMap<i64, VistaDeMarca>) -> String {
    let de: BTreeMap<String, String> = vistas
        .iter()
        .map(|(id, v)| (id.to_string(), v.a_texto()))
        .collect();
    serde_json::to_string_pretty(&de).unwrap_or_else(|_| "{}".into())
}

/// `escribir_vistas` de Windows: sin ninguna, la de al lado se borra y la
/// que viaja se deja en `{}` (si ya existia).
fn escribir_vistas(ruta: &Path, vistas: &BTreeMap<i64, VistaDeMarca>) -> std::io::Result<()> {
    if vistas.is_empty() {
        if ruta.to_string_lossy().ends_with(".pixpin-vistas") {
            return match std::fs::remove_file(ruta) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        if !ruta.exists() {
            return Ok(());
        }
    }
    an::escribir(ruta, &vistas_a_texto(vistas))
}

fn leer_vistas(ruta: &Path) -> BTreeMap<i64, VistaDeMarca> {
    std::fs::read_to_string(ruta)
        .map(|t| vistas_de_texto(&t))
        .unwrap_or_default()
}

fn ahora_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

struct Estado {
    marcas: Vec<Marca>,
    vistas: BTreeMap<i64, VistaDeMarca>,
    /// El viaje y el aumento en que lo dejo su ultimo paso.
    vuelo: Option<(Vuelo, f32)>,
}

/// **Las marcas de un lienzo abierto**, con sus vistas y el viaje de la
/// camara (el `Marcador` de Windows). Una por ventana.
#[derive(uniffi::Object)]
pub struct MarcasDelLienzo {
    ruta_marcas: PathBuf,
    ruta_vistas: PathBuf,
    estado: Mutex<Estado>,
}

impl MarcasDelLienzo {
    fn estado(&self) -> MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn guardar(&self, e: &Estado) {
        if let Err(err) = escribir_marcas(&self.ruta_marcas, &e.marcas) {
            // Un dibujo en un sitio de solo lectura no estropea el lienzo:
            // las marcas siguen en memoria.
            eprintln!(
                "PixPin: no se pudieron guardar las marcas en {}: {err}",
                self.ruta_marcas.display()
            );
        }
    }

    /// Guarda las vistas, sin las de marcas que ya no estan (quitadas, o
    /// idas por el tope de [`marcas::MAXIMO`]).
    fn guardar_vistas(&self, e: &mut Estado) {
        let Estado { marcas, vistas, .. } = e;
        vistas.retain(|id, _| marcas.iter().any(|m| m.id == *id));
        if let Err(err) = escribir_vistas(&self.ruta_vistas, vistas) {
            eprintln!(
                "PixPin: no se pudieron guardar las vistas de las marcas en {}: {err}",
                self.ruta_vistas.display()
            );
        }
    }

    fn fijar(&self, e: &mut Estado, id: i64, camara: CamaraDelLienzo, w: f32, h: f32) -> bool {
        if !e.marcas.iter().any(|m| m.id == id) {
            return false;
        }
        let Some(v) = VistaDeMarca::de_camara(&camara.into(), w, h) else {
            return false;
        };
        e.vistas.insert(id, v);
        self.guardar_vistas(e);
        true
    }

    /// Para las pruebas: donde guarda cada cosa.
    pub fn rutas(&self) -> (PathBuf, PathBuf) {
        (self.ruta_marcas.clone(), self.ruta_vistas.clone())
    }
}

#[uniffi::export]
impl MarcasDelLienzo {
    /// Las marcas (y sus vistas) del dibujo `ruta` (`EditorLienzo::ruta`).
    #[uniffi::constructor]
    pub fn junto_al_dibujo(ruta: String) -> Arc<Self> {
        let dibujo = PathBuf::from(ruta);
        let ruta_marcas = ruta_de_marcas(&dibujo);
        let ruta_vistas = ruta_de_vistas(&dibujo);
        let estado = Estado {
            marcas: leer_marcas(&ruta_marcas),
            vistas: leer_vistas(&ruta_vistas),
            vuelo: None,
        };
        Arc::new(Self {
            ruta_marcas,
            ruta_vistas,
            estado: Mutex::new(estado),
        })
    }

    /// Los emoticonos de la tira, en su orden (1-9 y 0 eligen los diez
    /// primeros).
    pub fn emojis(&self) -> Vec<String> {
        EMOJIS.iter().map(|e| e.to_string()).collect()
    }

    /// Las marcas en el orden del riel (de arriba abajo y de izquierda a
    /// derecha): ⌘1…9 van a las nueve primeras.
    pub fn lista(&self) -> Vec<MarcaDelLienzo> {
        let e = self.estado();
        e.marcas
            .iter()
            .map(|m| MarcaDelLienzo {
                id: m.id,
                x: m.x,
                y: m.y,
                emoji: m.emoji.clone(),
                con_vista: e.vistas.contains_key(&m.id),
            })
            .collect()
    }

    /// **Vuelve a leer** las marcas y sus vistas (lo que llego de otro
    /// aparato). `true` si algo cambio.
    pub fn releer(&self) -> bool {
        let marcas = leer_marcas(&self.ruta_marcas);
        let vistas = leer_vistas(&self.ruta_vistas);
        let mut e = self.estado();
        if e.marcas == marcas && e.vistas == vistas {
            return false;
        }
        e.marcas = marcas;
        e.vistas = vistas;
        true
    }

    /// **Planta una marca** en `(x, y)` del mundo con `emoji` y le guarda lo
    /// que se ve ahora (la `camara` en un hueco de `ancho_px` × `alto_px`
    /// puntos): es adonde se volvera. Devuelve su id.
    pub fn poner(
        &self,
        x: f64,
        y: f64,
        emoji: String,
        camara: CamaraDelLienzo,
        ancho_px: f32,
        alto_px: f32,
    ) -> i64 {
        let mut e = self.estado();
        let antes: Vec<i64> = e.marcas.iter().map(|m| m.id).collect();
        e.marcas = marcas::con(&e.marcas, x, y, &emoji, ahora_ms());
        self.guardar(&e);
        let nueva = e
            .marcas
            .iter()
            .map(|m| m.id)
            .find(|id| !antes.contains(id))
            .unwrap_or_default();
        if !self.fijar(&mut e, nueva, camara, ancho_px, alto_px) {
            // Sin vista (un hueco que no mide nada): al menos se quitan las
            // de las marcas idas por el tope.
            self.guardar_vistas(&mut e);
        }
        nueva
    }

    /// La marca `id`, movida a `(x, y)` del mundo (con la mano). Su vista
    /// no cambia.
    pub fn mover(&self, id: i64, x: f64, y: f64) {
        let mut e = self.estado();
        e.marcas = marcas::movida(&e.marcas, id, x, y);
        self.guardar(&e);
    }

    /// Sin la marca `id`; su vista se va con ella. `true` si estaba.
    pub fn quitar(&self, id: i64) -> bool {
        let mut e = self.estado();
        if !e.marcas.iter().any(|m| m.id == id) {
            return false;
        }
        e.marcas = marcas::sin(&e.marcas, id);
        self.guardar(&e);
        if e.vistas.contains_key(&id) {
            self.guardar_vistas(&mut e);
        }
        true
    }

    /// **La vista de la marca `id` pasa a ser la de ahora** (Mayus+clic, o
    /// ⌘⇧1…9). `false` si no esta o el hueco no mide nada.
    pub fn fijar_vista(&self, id: i64, camara: CamaraDelLienzo, ancho_px: f32, alto_px: f32) -> bool {
        let mut e = self.estado();
        self.fijar(&mut e, id, camara, ancho_px, alto_px)
    }

    /// La marca bajo el punto `(px, py)` de la vista (en puntos), con su
    /// redondel de `radio`: la ultima de la lista si se tapan.
    pub fn marca_en(&self, camara: CamaraDelLienzo, px: f32, py: f32, radio: f32) -> Option<i64> {
        let e = self.estado();
        marcas::marca_en(&e.marcas, &camara.into(), Punto2::nuevo(px, py), radio)
    }

    /// Adonde lleva ir a la marca `id` en un hueco de `ancho_px` ×
    /// `alto_px`: a su vista si la tiene y, si no, a dejarla arriba y
    /// centrada a `bajo_la_barra` + [`AIRE_BAJO_LA_BARRA`] del borde, con el
    /// aumento de ahora.
    pub fn destino(
        &self,
        id: i64,
        camara: CamaraDelLienzo,
        ancho_px: f32,
        alto_px: f32,
        bajo_la_barra: f32,
    ) -> Option<CamaraDelLienzo> {
        let e = self.estado();
        let m = e.marcas.iter().find(|m| m.id == id)?;
        Some(match e.vistas.get(&id) {
            // Con los topes del editor y no los de la camara.
            Some(v) => v
                .camara_entre(ancho_px, alto_px, ZOOM_MINIMO_DEL_EDITOR, ZOOM_MAXIMO_DEL_EDITOR)
                .into(),
            None => marcas::camara_con_la_marca_arriba(
                m,
                camara.zoom,
                ancho_px,
                bajo_la_barra + AIRE_BAJO_LA_BARRA,
            )
            .into(),
        })
    }

    /// **Echa a volar la camara** hacia la marca `id` (ver
    /// [`MarcasDelLienzo::destino`]). Antes relee las vistas: la que se haya
    /// cambiado en otro aparato manda. `false` si la marca no esta o si no
    /// hay hueco (la ventana minimizada, o a medio crear): la camara saldria
    /// para una ventana de un pixel (Windows 64af3c3).
    pub fn ir_a(
        &self,
        id: i64,
        camara: CamaraDelLienzo,
        ancho_px: f32,
        alto_px: f32,
        bajo_la_barra: f32,
    ) -> bool {
        if !(ancho_px >= 1.0 && alto_px >= 1.0) {
            return false;
        }
        let vistas = leer_vistas(&self.ruta_vistas);
        {
            let mut e = self.estado();
            if std::fs::metadata(&self.ruta_vistas).is_ok() {
                e.vistas = vistas;
            }
        }
        let Some(destino) = self.destino(id, camara, ancho_px, alto_px, bajo_la_barra) else {
            return false;
        };
        let con_vista = self.estado().vistas.contains_key(&id);
        let vuelo = if con_vista {
            Vuelo::hacia_en(destino.into(), ancho_px, alto_px)
        } else {
            Vuelo::hacia(destino.into())
        };
        self.estado().vuelo = Some((vuelo, camara.zoom));
        true
    }

    /// **Un paso del viaje** de `dt` segundos desde `camara`: la camara
    /// nueva, o `None` si no hay viaje (o se acaba de parar porque alguien
    /// cambio el aumento a mitad, como la rueda en Windows). El ultimo paso
    /// deja la camara exactamente en el destino.
    pub fn avanzar(&self, camara: CamaraDelLienzo, dt: f32) -> Option<CamaraDelLienzo> {
        let mut e = self.estado();
        let (vuelo, dejado) = e.vuelo?;
        // Se compara con el aumento que dejo el propio viaje: con una vista
        // guardada el viaje tambien acerca o aleja.
        if (camara.zoom - dejado).abs() > 1e-4 * dejado.max(1.0) {
            e.vuelo = None;
            return None;
        }
        let mut c: Camara = camara.into();
        let sigue = vuelo.avanzar(&mut c, dt);
        e.vuelo = sigue.then_some((vuelo, c.zoom));
        Some(c.into())
    }

    /// Si la camara esta de viaje.
    pub fn volando(&self) -> bool {
        self.estado().vuelo.is_some()
    }

    /// Para el viaje (un clic, la rueda: la mano vuelve a mandar).
    pub fn parar(&self) {
        self.estado().vuelo = None;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-marcas-lienzo-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const CAM: CamaraDelLienzo = CamaraDelLienzo {
        x: 100.0,
        y: 200.0,
        zoom: 2.0,
    };

    fn volar(m: &MarcasDelLienzo, mut c: CamaraDelLienzo) -> CamaraDelLienzo {
        let mut vueltas = 0;
        while let Some(n) = m.avanzar(c, 1.0 / 60.0) {
            c = n;
            vueltas += 1;
            assert!(vueltas < 2000);
            if !m.volando() {
                break;
            }
        }
        c
    }

    #[test]
    fn un_dibujo_suelto_las_guarda_al_lado_y_las_borra_al_vaciarse() {
        let d = carpeta("suelto");
        let dibujo = d.join("plano.excalidraw");
        let m = MarcasDelLienzo::junto_al_dibujo(dibujo.display().to_string());
        assert_eq!(
            m.rutas(),
            (
                d.join("plano.excalidraw.pixpin-marcas"),
                d.join("plano.excalidraw.pixpin-vistas")
            )
        );
        let id = m.poner(10.0, 20.0, "⭐".into(), CAM, 800.0, 600.0);
        assert!(m.rutas().0.is_file() && m.rutas().1.is_file());
        // Otra ventana del mismo dibujo la ve igual, con su vista.
        let otra = MarcasDelLienzo::junto_al_dibujo(dibujo.display().to_string());
        assert_eq!(otra.lista(), m.lista());
        assert!(otra.lista()[0].con_vista);
        assert!(m.quitar(id));
        assert!(!m.rutas().0.exists() && !m.rutas().1.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn plantar_guarda_la_vista_y_volver_la_devuelve_con_su_aumento() {
        let d = carpeta("vuelta");
        let m = MarcasDelLienzo::junto_al_dibujo(d.join("a.excalidraw").display().to_string());
        let id = m.poner(500.0, 400.0, "🔖".into(), CAM, 800.0, 600.0);
        // Se va lejos y con otro aumento...
        let lejos = CamaraDelLienzo {
            x: -3000.0,
            y: 9000.0,
            zoom: 0.3,
        };
        assert_eq!(m.destino(id, lejos, 800.0, 600.0, 60.0), Some(CAM));
        assert!(m.ir_a(id, lejos, 800.0, 600.0, 60.0));
        let llegada = volar(&m, lejos);
        // ...y vuelve exactamente a la camara de antes.
        assert_eq!(llegada, CAM);
        assert!(!m.volando());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn una_marca_sin_vista_vuelve_arriba_con_el_aumento_de_ahora() {
        let d = carpeta("sin-vista");
        let dibujo = d.join("a.excalidraw");
        std::fs::write(hermano(&dibujo, ".pixpin-marcas"), "7:300:900:❤️").unwrap();
        let m = MarcasDelLienzo::junto_al_dibujo(dibujo.display().to_string());
        assert!(!m.lista()[0].con_vista);
        let c = m.destino(7, CAM, 800.0, 600.0, 52.0).unwrap();
        assert_eq!(c.zoom, CAM.zoom);
        // Centrada a lo ancho y a la barra mas 48 del borde de arriba.
        assert!((c.x - (300.0 - 800.0 / 4.0)).abs() < 1e-3);
        assert!((c.y - (900.0 - 100.0 / 2.0)).abs() < 1e-3);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn mayus_clic_cambia_la_vista_y_quitar_se_la_lleva() {
        let d = carpeta("fijar");
        let m = MarcasDelLienzo::junto_al_dibujo(d.join("a.excalidraw").display().to_string());
        let a = m.poner(1.0, 1.0, "⭐".into(), CAM, 800.0, 600.0);
        let b = m.poner(2.0, 2.0, "❗".into(), CAM, 800.0, 600.0);
        let otra = CamaraDelLienzo {
            x: 0.0,
            y: 0.0,
            zoom: 4.0,
        };
        assert!(m.fijar_vista(a, otra, 800.0, 600.0));
        assert_eq!(m.destino(a, CAM, 800.0, 600.0, 0.0), Some(otra));
        assert!(!m.fijar_vista(99, otra, 800.0, 600.0));
        assert!(!m.fijar_vista(a, otra, 0.0, 600.0));
        assert!(m.quitar(a));
        let vistas = leer_vistas(&m.rutas().1);
        assert_eq!(vistas.keys().copied().collect::<Vec<_>>(), vec![b]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn lejos_del_origen_el_vuelo_aterriza_y_sin_hueco_no_despega() {
        // Windows 64af3c3: un plano en milimetros, coordenadas de millones y
        // el aumento al maximo; el vuelo acaba exactamente en la vista.
        let d = carpeta("lejos");
        let m = MarcasDelLienzo::junto_al_dibujo(d.join("a.excalidraw").display().to_string());
        let alli = CamaraDelLienzo {
            x: 2_003_000.5,
            y: -1_499_000.5,
            zoom: ZOOM_MAXIMO_DEL_EDITOR,
        };
        let id = m.poner(2_003_010.0, -1_498_990.0, "⭐".into(), alli, 1600.0, 900.0);
        let aqui = CamaraDelLienzo {
            x: 2_000_000.0,
            y: -1_500_000.0,
            zoom: ZOOM_MAXIMO_DEL_EDITOR,
        };
        let destino = m.destino(id, aqui, 1600.0, 900.0, 0.0).unwrap();
        assert!(m.ir_a(id, aqui, 1600.0, 900.0, 0.0));
        assert_eq!(volar(&m, aqui), destino);
        assert!(!m.volando());
        // Una vista enorme no baja del minimo del editor.
        let enorme = m.poner(0.0, 0.0, "❗".into(), CAM, 800.0, 600.0);
        let lejisimos = CamaraDelLienzo {
            x: -1e6,
            y: -1e6,
            zoom: 0.0001,
        };
        assert!(m.fijar_vista(enorme, lejisimos, 800.0, 600.0));
        let c = m.destino(enorme, CAM, 800.0, 600.0, 0.0).unwrap();
        assert_eq!(c.zoom, ZOOM_MINIMO_DEL_EDITOR);
        // Caso negativo: sin hueco (la ventana minimizada) no se echa a volar.
        assert!(!m.ir_a(id, aqui, 0.0, 900.0, 0.0));
        assert!(!m.volando());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_rueda_a_mitad_de_viaje_para_el_viaje() {
        let d = carpeta("rueda");
        let m = MarcasDelLienzo::junto_al_dibujo(d.join("a.excalidraw").display().to_string());
        let id = m.poner(1.0, 1.0, "⭐".into(), CAM, 800.0, 600.0);
        let lejos = CamaraDelLienzo {
            x: 5000.0,
            y: 5000.0,
            zoom: 0.5,
        };
        m.ir_a(id, lejos, 800.0, 600.0, 0.0);
        let paso = m.avanzar(lejos, 1.0 / 60.0).unwrap();
        // Alguien acerca con la rueda: el viaje se para.
        let tocada = CamaraDelLienzo {
            zoom: paso.zoom * 1.5,
            ..paso
        };
        assert_eq!(m.avanzar(tocada, 1.0 / 60.0), None);
        assert!(!m.volando());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_marca_bajo_el_raton_es_la_de_su_redondel() {
        let d = carpeta("bajo");
        let m = MarcasDelLienzo::junto_al_dibujo(d.join("a.excalidraw").display().to_string());
        let id = m.poner(150.0, 250.0, "⭐".into(), CAM, 800.0, 600.0);
        // (150, 250) del mundo cae en (100, 100) de la vista al doble.
        assert_eq!(m.marca_en(CAM, 110.0, 95.0, 18.0), Some(id));
        assert_eq!(m.marca_en(CAM, 140.0, 100.0, 18.0), None);
        m.mover(id, 400.0, 400.0);
        assert_eq!(m.lista()[0].x, 400.0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn las_vistas_se_leen_por_id_y_lo_roto_se_salta() {
        let v = vistas_de_texto(
            r#"{"12": "1 2 3 4", "x": "1 2 3 4", "13": "1 2 0 4", "14": 5, "15": "1 2 3"}"#,
        );
        assert_eq!(v.len(), 1);
        assert_eq!(
            v[&12],
            VistaDeMarca {
                x: 1.0,
                y: 2.0,
                ancho: 3.0,
                alto: 4.0
            }
        );
        assert!(vistas_de_texto("no es json").is_empty());
        assert_eq!(vistas_a_texto(&BTreeMap::new()), "{}");
    }
}
