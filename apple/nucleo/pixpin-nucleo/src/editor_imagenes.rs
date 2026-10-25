//! **Imagenes pegadas en el lienzo** (⌘V o soltar una foto), guardadas
//! exactamente como Windows (`imagenes_lienzo::guardar_pegadas_en_hoja`) y
//! el movil: el PNG en `imagenes/<fileId>` de la carpeta del chat y su
//! entrada en `files` del `.excalidraw` (`excalidraw::poner_fichero`, el
//! `SceneFile` del movil: `id`, `mimeType`, `path`, `created`), con el
//! `fileId` `pc<hex>` de lo que nace en el escritorio
//! (`enlace::id_de_texto`).
//!
//! # Por que un PNG ya hecho
//!
//! Swift lo codifica con ImageIO (hardware, y ya reducido a
//! [`LADO_MAXIMO`]): aqui no se decodifica nada, solo se lee la cabecera
//! IHDR para saber lo que mide. Cruza el puente una vez.
//!
//! # Donde vive hasta guardar
//!
//! Como en Windows, lo pegado y luego borrado **no se escribe** en el
//! proyecto. Hasta guardar, el PNG espera en una carpeta temporal de la
//! sesion (no en memoria: §4bis), de donde Swift lo lee reducido para
//! pintarlo. Al guardar se copia a `imagenes/` si la escena lo sigue usando
//! y se apunta en `files`; desde entonces su entrada se queda, tambien si
//! despues se borra la figura (su lapida la sigue nombrando).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::relleno::EstiloRelleno;

use crate::ErrorNucleo;
use crate::editor::EditorLienzo;
use crate::lienzo::ImagenDelLienzo;

/// El lado mas largo que Swift deja a una imagen pegada (lo que admite
/// cualquier GPU de las de Windows, `lado_de_subida`).
pub const LADO_MAXIMO: u32 = 4096;

/// Cuanto de lo visible puede ocupar una imagen recien pegada.
const FRACCION_VISIBLE: f32 = 0.8;

/// Una imagen pegada en esta sesion, a la espera de guardarse.
#[derive(Debug, Clone)]
pub(crate) struct Pegada {
    pub id_objeto: u64,
    /// El PNG en la carpeta temporal de la sesion.
    pub temporal: PathBuf,
}

/// Lo que mide un PNG, leido de su cabecera (sin decodificar).
pub fn medidas_png(bytes: &[u8]) -> Option<(u32, u32)> {
    const FIRMA: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes.len() < 24 || bytes[..8] != FIRMA || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let ancho = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let alto = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (ancho > 0 && alto > 0).then_some((ancho, alto))
}

/// El tamano con el que entra: el natural, reducido en proporcion si no cabe
/// en lo visible con holgura; nunca se agranda (`tamano_al_pegar` de
/// Windows).
pub fn tamano_al_pegar(ancho: u32, alto: u32, vista_ancho: f32, vista_alto: f32) -> (f32, f32) {
    let (w, h) = (ancho.max(1) as f32, alto.max(1) as f32);
    let cabe_w = (vista_ancho * FRACCION_VISIBLE).max(1.0);
    let cabe_h = (vista_alto * FRACCION_VISIBLE).max(1.0);
    let factor = (cabe_w / w).min(cabe_h / h).min(1.0);
    ((w * factor).max(1.0), (h * factor).max(1.0))
}

/// La figura de una imagen (`imagenes_lienzo::elemento_imagen` de Windows):
/// sin trazo ni relleno, la imagen es su propio aspecto.
pub fn elemento_imagen(id_objeto: u64, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
    Elemento {
        id: 0,
        figura: Figura::Imagen { id_objeto },
        x,
        y,
        ancho,
        alto,
        angulo: 0.0,
        trazo: ColorRgba::opaco(0.1, 0.1, 0.1),
        relleno: None,
        estilo_relleno: EstiloRelleno::Solido,
        grosor: 0.0,
        estilo: EstiloTrazo::Solido,
        rugosidad: 0.0,
        opacidad: 1.0,
        semilla: 1,
        version: 0,
        borrado: false,
        grupos: Vec::new(),
        bloqueado: false,
        enlace: None,
        redondo: false,
        material: Default::default(),
        extras: Default::default(),
    }
}

/// Un `id_objeto` nuevo para una imagen (`ImagenesLienzo::id_nuevo` de
/// Windows): la hora y una muestra de los bytes, por debajo de 2^48 para
/// que el `pc<hex>` quede en doce cifras, y que no choque con `ocupado`.
pub fn id_nuevo(bytes: &[u8], ocupado: impl Fn(u64) -> bool) -> u64 {
    let ahora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mezclar = |h: &mut u64, b: u64| {
        *h ^= b;
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ ahora;
    mezclar(&mut h, bytes.len() as u64);
    let paso = (bytes.len() / 4096).max(1);
    for b in bytes.iter().step_by(paso) {
        mezclar(&mut h, *b as u64);
    }
    let mut n = 1u64;
    loop {
        mezclar(&mut h, n);
        n += 1;
        let id = h & 0x0000_ffff_ffff_ffff;
        if id != 0 && !ocupado(id) {
            return id;
        }
    }
}

/// La carpeta temporal de las pegadas de un editor: una por editor, que
/// se borra al soltarlo.
pub(crate) fn carpeta_temporal_nueva() -> PathBuf {
    static CUENTA: AtomicU64 = AtomicU64::new(0);
    let n = CUENTA.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("pixpin-pegadas-{}-{n}", std::process::id()))
}

/// Las pegadas que la escena sigue usando (visibles).
pub(crate) fn en_uso<'a>(escena: &Escena, pegadas: &'a [Pegada]) -> Vec<&'a Pegada> {
    let usadas: std::collections::HashSet<u64> = escena
        .visibles()
        .filter_map(|e| match e.figura {
            Figura::Imagen { id_objeto } => Some(id_objeto),
            _ => None,
        })
        .collect();
    pegadas
        .iter()
        .filter(|p| usadas.contains(&p.id_objeto))
        .collect()
}

/// Copia el PNG a `destino` si no estaba ya, por un `.tmp` y renombrando
/// (un corte de luz no deja media foto).
pub(crate) fn copiar_si_falta(origen: &Path, destino: &Path) -> std::io::Result<()> {
    if destino.is_file() {
        return Ok(());
    }
    if let Some(dir) = destino.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut temporal = destino.as_os_str().to_owned();
    temporal.push(".tmp");
    std::fs::copy(origen, &temporal)?;
    std::fs::rename(&temporal, destino)
}

#[uniffi::export]
impl EditorLienzo {
    /// **Pega una imagen** (⌘V o soltarla): `png` son los bytes de un PNG
    /// (Swift lo codifica con ImageIO, con el lado mayor como mucho
    /// [`LADO_MAXIMO`]); entra centrada en `(centro_x, centro_y)` del mundo
    /// y reducida si no cabe en el 80 % de lo visible (`ancho_vista` x
    /// `alto_vista`, en unidades del mundo). Nace en un paso de deshacer,
    /// queda elegida y se pasa a la mano para moverla. Devuelve su
    /// `id_objeto` y el fichero de donde pintarla hasta que se guarde.
    pub fn pegar_imagen(
        &self,
        png: Vec<u8>,
        centro_x: f32,
        centro_y: f32,
        ancho_vista: f32,
        alto_vista: f32,
    ) -> Result<ImagenDelLienzo, ErrorNucleo> {
        let (w, h) = medidas_png(&png).ok_or_else(|| ErrorNucleo::Argumento {
            detalle: "no es un PNG".into(),
        })?;
        let finito = |v: f32, si_no: f32| if v.is_finite() { v } else { si_no };
        let (ancho, alto) = tamano_al_pegar(
            w,
            h,
            finito(ancho_vista, w as f32).max(1.0),
            finito(alto_vista, h as f32).max(1.0),
        );
        let (cx, cy) = (finito(centro_x, 0.0), finito(centro_y, 0.0));
        let hecho = self.meter_png(&png, |id_objeto| {
            (
                elemento_imagen(id_objeto, cx - ancho / 2.0, cy - alto / 2.0, ancho, alto),
                true,
            )
        })?;
        Ok(hecho)
    }

    /// **La copia de una zona** (F8, `figuras::poner_copia_de_zona` de
    /// Windows): `png` es la foto que Swift saco de la caja `(x0, y0, x1,
    /// y1)` que pidio [`crate::editor::PeticionLienzo::Zona`], ya recortada
    /// en redondo. Entra del tamano de la caja, corrida
    /// `zona::CORRIMIENTO_PX` de pantalla (a `zoom`) y elegida, con el grupo
    /// que la reconoce como copia de zona. La Zona se queda en la mano: se
    /// pueden sacar varias seguidas.
    pub fn poner_copia_de_zona(
        &self,
        png: Vec<u8>,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        zoom: f32,
    ) -> Result<ImagenDelLienzo, ErrorNucleo> {
        if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) || x1 <= x0 || y1 <= y0 {
            return Err(ErrorNucleo::Argumento {
                detalle: "zona vacia".into(),
            });
        }
        medidas_png(&png).ok_or_else(|| ErrorNucleo::Argumento {
            detalle: "no es un PNG".into(),
        })?;
        let corrido = pixpin_motor2d::zona::CORRIMIENTO_PX / zoom.max(0.0001);
        let grupo = pixpin_motor2d::zona::grupo_nuevo(&self.tomar().escena);
        self.meter_png(&png, |id_objeto| {
            let mut e = elemento_imagen(id_objeto, x0 + corrido, y0 + corrido, x1 - x0, y1 - y0);
            e.grupos.push(grupo.clone());
            (e, false)
        })
    }
}

impl EditorLienzo {
    /// Guarda `png` en la carpeta temporal y mete la figura que da
    /// `figura(id_objeto)` en un paso de deshacer, elegida; el `bool` dice si
    /// se pasa a la mano para moverla.
    /// **Registra un PNG con su `id_objeto`** (las fotos recortadas de una
    /// hoja pasada al lienzo, `hoja_a_lienzo`): sin figura ni paso de
    /// deshacer; la figura que lo nombra llega luego con su documento y al
    /// guardar se copia a `imagenes/` como lo pegado.
    pub(crate) fn registrar_png(&self, id_objeto: u64, png: &[u8]) -> Result<(), ErrorNucleo> {
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        let carpeta = st.temporal.get_or_insert_with(carpeta_temporal_nueva).clone();
        std::fs::create_dir_all(&carpeta)?;
        let temporal = carpeta.join(pixpin_motor2d::enlace::id_de_texto(id_objeto));
        std::fs::write(&temporal, png)?;
        st.pegadas.retain(|p| p.id_objeto != id_objeto);
        st.pegadas.push(Pegada { id_objeto, temporal });
        Ok(())
    }

    fn meter_png(
        &self,
        png: &[u8],
        figura: impl FnOnce(u64) -> (Elemento, bool),
    ) -> Result<ImagenDelLienzo, ErrorNucleo> {
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        if !st.gesto.en_reposo() || st.borrando {
            return Err(ErrorNucleo::Argumento {
                detalle: "a mitad de un gesto".into(),
            });
        }
        let ya: std::collections::HashSet<u64> = self
            .imagenes_abiertas()
            .chain(st.pegadas.iter().map(|p| p.id_objeto))
            .collect();
        let id_objeto = id_nuevo(png, |id| ya.contains(&id));
        let carpeta = st
            .temporal
            .get_or_insert_with(carpeta_temporal_nueva)
            .clone();
        std::fs::create_dir_all(&carpeta)?;
        let temporal = carpeta.join(pixpin_motor2d::enlace::id_de_texto(id_objeto));
        std::fs::write(&temporal, png)?;
        st.gesto.cerrar_texto(&mut st.escena);
        let (e, a_la_mano) = figura(id_objeto);
        st.escena.abrir_paso();
        let id = st.escena.anadir(e);
        st.escena.cerrar_paso();
        st.pegadas.push(Pegada {
            id_objeto,
            temporal: temporal.clone(),
        });
        if a_la_mano {
            st.gesto
                .tomar_herramienta(pixpin_motor2d::gesto::Herramienta::Mano);
        }
        st.gesto.seleccion.poner_todos([id]);
        drop(guarda);
        self.subir_cambios();
        self.subir_version();
        Ok(ImagenDelLienzo {
            id_objeto,
            ruta: temporal.display().to_string(),
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_cabecera_del_png_dice_lo_que_mide() {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13];
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&640u32.to_be_bytes());
        png.extend_from_slice(&480u32.to_be_bytes());
        assert_eq!(medidas_png(&png), Some((640, 480)));
        // Caso negativo: un JPEG no se toma por PNG.
        assert_eq!(
            medidas_png(&[
                0xff, 0xd8, 0xff, 0xe0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
            ]),
            None
        );
    }

    #[test]
    fn al_pegar_se_reduce_si_no_cabe_y_nunca_se_agranda() {
        assert_eq!(tamano_al_pegar(100, 50, 1000.0, 1000.0), (100.0, 50.0));
        let (w, h) = tamano_al_pegar(4000, 2000, 1000.0, 800.0);
        assert!((w - 800.0).abs() < 0.01 && (h - 400.0).abs() < 0.01);
    }

    #[test]
    fn el_id_nuevo_cabe_en_doce_cifras_y_no_choca() {
        let a = id_nuevo(b"abc", |_| false);
        assert!(a < 1 << 48 && a != 0);
        let b = id_nuevo(b"abc", |id| id == a);
        assert_ne!(a, b);
    }
}
