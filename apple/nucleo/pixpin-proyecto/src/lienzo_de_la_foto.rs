//! **El lienzo de una foto del chat, creado en el PC como lo crea el movil.**
//!
//! En el movil, anotar una foto del chat no toca la foto: la primera vez que
//! se abre en el editor se crea un lienzo propio,
//! `lienzos/<referencia o foto-<id del mensaje>>.excalidraw`
//! (`Mensaje.dibujoDeLaFoto`, `guardados/Mensajes.kt`), con la foto como
//! primer elemento (en `0,0`, bloqueada, al tamano con que el movil la carga
//! y no a sus pixeles: `DrawEditorActivity.colocarImagenInicial` +
//! `ImageStore.load`) y lo dibujado encima; y se apunta en la `referencia`
//! del mensaje (`MensajesActivity`, rama `Clase.IMAGEN`). Ese lienzo viaja al
//! sincronizar (`alcance_de`, clase `IMAGEN`).
//!
//! El PC guardaba lo dibujado sobre una foto en `<foto>.pixpin2d`, a su lado,
//! que NO viaja. Aqui se hace lo del movil: se crea ese lienzo, se apunta la
//! `referencia` y lo que ya hubiera en el `.pixpin2d` se pasa a el una vez,
//! llevado a unidades del lienzo. El `.pixpin2d` no se borra: se renombra a
//! `.adoptado`, por si algo saliera mal.

use std::io;
use std::path::{Path, PathBuf};

use pixpin_motor2d::{Elemento, Figura, Punto2};

use crate::almacen;
use crate::cuaderno::{self, Mensaje};

/// El lado mas largo con que el movil carga una foto para dibujar encima
/// (`ImageStore.MAX_DIMENSION`). La foto del lienzo mide eso, no sus pixeles.
pub const LADO_MAXIMO: u32 = 2048;

/// El id del lienzo de la foto, como el movil (`dibujoDeLaFoto`): su
/// `referencia` si la tiene (fotos de antes) y si no `foto-<id del mensaje>`.
pub fn id_del_lienzo(m: &Mensaje) -> String {
    m.referencia
        .as_deref()
        .filter(|r| !r.is_empty())
        .map_or_else(|| format!("foto-{}", m.id), str::to_string)
}

/// Donde guardaba el PC lo dibujado sobre una foto: a su lado, con el nombre
/// entero y `.pixpin2d` detras (`fachada.jpg.pixpin2d`).
pub fn pixpin2d_de(foto: &Path) -> PathBuf {
    let mut s = foto.as_os_str().to_owned();
    s.push(".pixpin2d");
    PathBuf::from(s)
}

/// Cuanto mide la foto dentro del lienzo: lo que da `BitmapFactory` con el
/// `inSampleSize` de `ImageStore.sampleFor` (se parte por dos hasta que el
/// lado largo no pasa de [`LADO_MAXIMO`]). Una foto de 4000x3000 queda en
/// 2000x1500; una de 1200x900, igual.
pub fn medida_en_el_lienzo((ancho, alto): (u32, u32)) -> (f32, f32) {
    let (ancho, alto) = (ancho.max(1), alto.max(1));
    let mut muestra = 1u32;
    while ancho.max(alto) / muestra > LADO_MAXIMO {
        muestra *= 2;
    }
    // El descodificador de JPEG redondea hacia arriba al reducir.
    (
        ancho.div_ceil(muestra) as f32,
        alto.div_ceil(muestra) as f32,
    )
}

/// Lo que hizo [`asegurar`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hecho {
    /// El id del lienzo (`lienzos/<id>.excalidraw`).
    pub id: String,
    /// Si se creo ahora (si ya estaba, el del movil o uno de antes, no).
    pub creado: bool,
    /// Cuantos elementos del `.pixpin2d` se pasaron al lienzo.
    pub adoptados: usize,
    /// Si se apunto la `referencia` en el cuaderno (el mensaje no la tenia).
    pub referencia_puesta: bool,
}

/// **Deja la foto `m` con su lienzo, como el movil**, y devuelve su id.
///
/// - Si el lienzo no existe, lo crea: la foto copiada a `imagenes/<id>` de
///   la carpeta del proyecto (el movil tambien la copia, `guardarImagen`:
///   asi el lienzo no depende del mensaje) y puesta como primer elemento.
/// - Si ya existe (llego del movil) no se crea otro.
/// - Si hay un `.pixpin2d` junto a la foto, sus elementos pasan al lienzo
///   (de pixeles de la foto a unidades del lienzo) y el fichero se renombra
///   a `.adoptado`: pasa una sola vez, porque la vez siguiente ya no esta.
/// - Si el mensaje no tenia `referencia`, se le apunta en el cuaderno.
///
/// El lienzo va ANTES que la `referencia`: un mensaje nunca apunta a un
/// lienzo que no esta. `medidas` son los pixeles de la foto (lo que usan las
/// coordenadas del `.pixpin2d`).
pub fn asegurar(
    raiz: &Path,
    proyecto: &str,
    m: &Mensaje,
    foto: &Path,
    medidas: (u32, u32),
    ahora: i64,
) -> io::Result<Hecho> {
    let id = id_del_lienzo(m);
    let ruta = almacen::lienzo(raiz, proyecto, &id);
    let creado = if ruta.is_file() {
        false
    } else {
        crear(raiz, proyecto, &ruta, foto, medidas, ahora)?;
        true
    };
    let adoptados = adoptar(&ruta, foto, medidas, ahora)?;
    let referencia_puesta = if m.referencia.as_deref().is_none_or(str::is_empty) {
        let mut copia = m.clone();
        copia.referencia = Some(id.clone());
        // Si no se puede apuntar, el lienzo se sigue encontrando por
        // `foto-<id>`: no es motivo para no abrirlo.
        cuaderno::reemplazar(&almacen::carpeta(raiz, proyecto), &copia).unwrap_or(false)
    } else {
        false
    };
    Ok(Hecho {
        id,
        creado,
        adoptados,
        referencia_puesta,
    })
}

/// **Una foto nueva con lo dibujado encima ya puesto**: lo de [`asegurar`]
/// (el lienzo con la foto bloqueada debajo, a la medida del movil, y la
/// `referencia` apuntada) y encima `dibujados`, que vienen en PIXELES de la
/// foto, llevados a unidades del lienzo como elementos sueltos y editables.
///
/// Es lo que guarda el anotador de pantalla (2026-09-28): la captura de la
/// pantalla sin la tinta es la foto, y la tinta, sus trazos. Al abrir la
/// burbuja se ve y se sigue editando igual que una foto anotada en el movil.
/// Si el lienzo ya existia no se le vuelve a poner nada: una foto recien
/// guardada no puede tener otro, y duplicar la tinta seria peor que perderla.
pub fn con_dibujo(
    raiz: &Path,
    proyecto: &str,
    m: &Mensaje,
    foto: &Path,
    medidas: (u32, u32),
    dibujados: &[Elemento],
    ahora: i64,
) -> io::Result<Hecho> {
    let mut hecho = asegurar(raiz, proyecto, m, foto, medidas, ahora)?;
    if !hecho.creado || dibujados.is_empty() {
        return Ok(hecho);
    }
    let ruta = almacen::lienzo(raiz, proyecto, &hecho.id);
    let texto = std::fs::read_to_string(&ruta)?;
    let lienzo = pixpin_motor2d::excalidraw::leer(&texto).map_err(io::Error::other)?;
    let mut destino = pixpin_motor2d::excalidraw::a_escena(&lienzo);
    let visibles: Vec<Elemento> = dibujados.iter().filter(|e| !e.borrado).cloned().collect();
    for e in a_unidades_del_lienzo(&lienzo.elementos(), &visibles, medidas) {
        destino.anadir(e);
    }
    let nuevo = pixpin_motor2d::excalidraw::con_escena(&lienzo, &destino);
    escribir_atomico(&ruta, &pixpin_motor2d::excalidraw::escribir(&nuevo))?;
    hecho.adoptados += visibles
        .iter()
        .filter(|e| !matches!(e.figura, Figura::Imagen { .. }))
        .count();
    Ok(hecho)
}

/// **La misma pantalla anotada, con la tinta de ahora** (2026-09-29): el
/// anotador de pantalla guarda solo al salir, y si ya se guardo en esa
/// sesion (con el boton) y luego se siguio dibujando, lo que toca es poner
/// al dia EL MISMO lienzo, no mandar otro mensaje con otra captura.
///
/// Se queda la foto (la primera imagen bloqueada, la que puso
/// [`con_dibujo`]) y lo demas se cambia por `dibujados` (en pixeles de la
/// captura, `medidas`), llevado a unidades del lienzo como alli. Lo de antes
/// no se quita del fichero: se marca `isDeleted` (`con_escena`), que es lo
/// que le dice al movil «esto se borro» al sincronizar. Devuelve cuantos
/// trazos quedaron encima. Sin el lienzo, error: no se inventa uno sin foto.
pub fn cambiar_dibujo(
    raiz: &Path,
    proyecto: &str,
    id_lienzo: &str,
    medidas: (u32, u32),
    dibujados: &[Elemento],
) -> io::Result<usize> {
    let ruta = almacen::lienzo(raiz, proyecto, id_lienzo);
    let texto = std::fs::read_to_string(&ruta)?;
    let lienzo = pixpin_motor2d::excalidraw::leer(&texto).map_err(io::Error::other)?;
    let mut destino = pixpin_motor2d::excalidraw::a_escena(&lienzo);
    let la_foto = destino
        .visibles()
        .find(|e| e.bloqueado && matches!(e.figura, Figura::Imagen { .. }))
        .map(|e| e.id);
    let viejos: Vec<u64> = destino
        .visibles()
        .map(|e| e.id)
        .filter(|id| Some(*id) != la_foto)
        .collect();
    for id in viejos {
        destino.borrar(id);
    }
    let visibles: Vec<Elemento> = dibujados
        .iter()
        .filter(|e| !e.borrado && !matches!(e.figura, Figura::Imagen { .. }))
        .cloned()
        .collect();
    for e in a_unidades_del_lienzo(&lienzo.elementos(), &visibles, medidas) {
        destino.anadir(e);
    }
    let nuevo = pixpin_motor2d::excalidraw::con_escena(&lienzo, &destino);
    escribir_atomico(&ruta, &pixpin_motor2d::excalidraw::escribir(&nuevo))?;
    Ok(visibles.len())
}

/// El lienzo nuevo, con la foto dentro y nada mas.
fn crear(
    raiz: &Path,
    proyecto: &str,
    ruta: &Path,
    foto: &Path,
    medidas: (u32, u32),
    ahora: i64,
) -> io::Result<()> {
    let bytes = std::fs::read(foto)?;
    let carpeta = almacen::carpeta(raiz, proyecto);
    let imagenes = carpeta.join("imagenes");
    std::fs::create_dir_all(&imagenes)?;
    // Un id que ya este (dos fotos en el mismo milisegundo) no se pisa.
    let mut t = ahora;
    let fichero = loop {
        let f = format!("foto{t}");
        if !imagenes.join(&f).exists() {
            break f;
        }
        t += 1;
    };
    std::fs::write(imagenes.join(&fichero), &bytes)?;
    let texto = lienzo_con_foto(
        &fichero,
        tipo_de(&bytes, foto),
        medida_en_el_lienzo(medidas),
        ahora,
    );
    if let Some(dir) = ruta.parent() {
        std::fs::create_dir_all(dir)?;
    }
    escribir_atomico(ruta, &texto)
}

/// El `.excalidraw` de la foto: la `Scene` del movil, con la foto como la
/// deja el editor alli (un `Element` de imagen con sus valores de fabrica,
/// en `0,0` y `locked`) y su fichero en `files` por ruta, no en base64.
fn lienzo_con_foto(fichero: &str, mime: &str, (ancho, alto): (f32, f32), ahora: i64) -> String {
    serde_json::json!({
        "elements": [{
            "id": format!("foto-{ahora}-0"),
            "type": "image",
            "x": 0.0, "y": 0.0,
            "width": ancho, "height": alto,
            "angle": 0.0,
            "strokeColor": "#1e1e1e",
            "backgroundColor": "transparent",
            "fillStyle": "solid",
            "strokeWidth": 2.0,
            "strokeStyle": "solid",
            "roughness": 1.0,
            "opacity": 100.0,
            "seed": 1,
            "version": 1,
            "versionNonce": 0,
            "isDeleted": false,
            "groupIds": [],
            "updated": ahora,
            "locked": true,
            "fileId": fichero,
            "scale": [1.0, 1.0]
        }],
        "files": {
            fichero: {"id": fichero, "mimeType": mime, "path": format!("imagenes/{fichero}"), "created": ahora}
        },
        "backgroundColor": "#ffffff"
    })
    .to_string()
}

/// El tipo de la foto por sus primeros bytes y, si no se reconoce, por su
/// extension (`mimeDe` del movil).
fn tipo_de(bytes: &[u8], foto: &Path) -> &'static str {
    match bytes {
        [0xFF, 0xD8, ..] => "image/jpeg",
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [b'G', b'I', b'F', b'8', ..] => "image/gif",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "image/webp",
        [b'B', b'M', ..] => "image/bmp",
        _ => match foto
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("png") => "image/png",
            Some("webp") => "image/webp",
            Some("gif") => "image/gif",
            Some("bmp") => "image/bmp",
            _ => "image/jpeg",
        },
    }
}

/// Pasa lo del `.pixpin2d` de la foto al lienzo `ruta`, y lo renombra.
/// Devuelve cuantos elementos paso. Sin `.pixpin2d`, nada. Uno que no se
/// entiende se deja donde esta: mejor que perderlo.
fn adoptar(ruta: &Path, foto: &Path, medidas: (u32, u32), ahora: i64) -> io::Result<usize> {
    let viejo = pixpin2d_de(foto);
    if !viejo.is_file() {
        return Ok(0);
    }
    let Ok(escena) = pixpin_motor2d::cargar(&viejo) else {
        return Ok(0);
    };
    let dibujados: Vec<Elemento> = escena.visibles().cloned().collect();
    let cuantos = if dibujados.is_empty() {
        0
    } else {
        let texto = std::fs::read_to_string(ruta)?;
        let lienzo = pixpin_motor2d::excalidraw::leer(&texto).map_err(io::Error::other)?;
        let mut destino = pixpin_motor2d::excalidraw::a_escena(&lienzo);
        let llevados = a_unidades_del_lienzo(&lienzo.elementos(), &dibujados, medidas);
        // Las fotos pegadas en el `.pixpin2d` no tienen fichero que viaje:
        // `con_escena` no las escribe, asi que no se cuentan.
        let cuantos = llevados
            .iter()
            .filter(|e| !matches!(e.figura, Figura::Imagen { .. }))
            .count();
        for e in llevados {
            destino.anadir(e);
        }
        let nuevo = pixpin_motor2d::excalidraw::con_escena(&lienzo, &destino);
        escribir_atomico(ruta, &pixpin_motor2d::excalidraw::escribir(&nuevo))?;
        cuantos
    };
    // Renombrado y no borrado. Si ya hubo uno adoptado, este no lo pisa.
    let mut guardado = viejo.as_os_str().to_owned();
    guardado.push(".adoptado");
    let mut guardado = PathBuf::from(guardado);
    if guardado.exists() {
        let mut otro = guardado.into_os_string();
        otro.push(format!("-{ahora}"));
        guardado = PathBuf::from(otro);
    }
    std::fs::rename(&viejo, &guardado)?;
    Ok(cuantos)
}

/// Lo dibujado en pixeles de la foto, llevado a unidades del lienzo: la
/// cuenta al reves de la que hace la burbuja del chat para ensenarlo. La
/// foto del lienzo es la primera imagen (la que se puso al crearlo); sin
/// ella, se deja tal cual.
fn a_unidades_del_lienzo(
    del_lienzo: &[Elemento],
    dibujados: &[Elemento],
    (w, h): (u32, u32),
) -> Vec<Elemento> {
    let la_foto = del_lienzo
        .iter()
        .find(|e| !e.borrado && matches!(e.figura, Figura::Imagen { .. }));
    let (ox, oy, sx, sy) = match la_foto {
        Some(f) if f.ancho > 0.0 && f.alto > 0.0 && w > 0 && h > 0 => {
            (f.x, f.y, f.ancho / w as f32, f.alto / h as f32)
        }
        _ => (0.0, 0.0, 1.0, 1.0),
    };
    // `escalar_desde` no toca lo bloqueado; aqui todo tiene que ir a su sitio.
    let sueltos: Vec<Elemento> = dibujados
        .iter()
        .map(|e| Elemento {
            bloqueado: false,
            ..e.clone()
        })
        .collect();
    let s = sx.min(sy);
    pixpin_motor2d::estirar_bloque::escalar_desde(
        &sueltos,
        Punto2::nuevo(0.0, 0.0),
        sx,
        sy,
        Punto2::nuevo(ox, oy),
    )
    .into_iter()
    .zip(dibujados)
    .map(|(mut e, original)| {
        e.bloqueado = original.bloqueado;
        // El grosor, con la misma escala: la raya se ve igual de gorda
        // sobre la foto aqui y alli.
        e.grosor *= s;
        e
    })
    .collect()
}

/// Al lado y luego de un tiron: un corte a mitad no deja el lienzo roto.
fn escribir_atomico(ruta: &Path, texto: &str) -> io::Result<()> {
    let mut temporal = ruta.as_os_str().to_owned();
    temporal.push(".foto.tmp");
    let temporal = PathBuf::from(temporal);
    std::fs::write(&temporal, texto)?;
    std::fs::rename(&temporal, ruta)
}

/// La escena con la que se escribe un `.pixpin2d` en las pruebas.
#[cfg(test)]
fn escena_con(elementos: Vec<Elemento>) -> pixpin_motor2d::Escena {
    let mut e = pixpin_motor2d::Escena::nueva();
    for x in elementos {
        e.anadir(x);
    }
    e
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::cuaderno::{Clase, Sello};

    fn raiz(etiqueta: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!(
            "pixpin-lienzo-foto-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(almacen::carpeta(&r, "p1").join("archivos")).unwrap();
        r
    }

    /// Una foto «inventada»: la cabecera de un JPEG y relleno. No se
    /// descodifica: las medidas se pasan aparte.
    fn poner_foto(r: &Path, nombre: &str) -> PathBuf {
        let ruta = almacen::carpeta(r, "p1").join("archivos").join(nombre);
        std::fs::write(&ruta, [0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3, 4]).unwrap();
        ruta
    }

    /// El mensaje de la foto, apuntado en el cuaderno del proyecto.
    fn mensaje(r: &Path, id: &str, referencia: Option<&str>) -> Mensaje {
        let sello = Sello {
            cuando: 1_790_000_000_000,
            numero: 1,
            aparato: "PC01".into(),
            proyecto: "p1".into(),
        };
        let mut m = Mensaje::adjunto(Clase::Imagen, "foto.jpg", "archivos/foto.jpg", 8, &sello);
        m.id = id.into();
        m.referencia = referencia.map(str::to_string);
        cuaderno::anadir(&almacen::carpeta(r, "p1"), &m).unwrap();
        m
    }

    fn cuaderno_de(r: &Path) -> Vec<Mensaje> {
        let t = std::fs::read_to_string(almacen::carpeta(r, "p1").join("guardados.jsonl")).unwrap();
        cuaderno::Cuaderno::leer(&t).mensajes
    }

    fn json_del_lienzo(r: &Path, id: &str) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(almacen::lienzo(r, "p1", id)).unwrap())
            .unwrap()
    }

    fn raya(puntos: &[(f32, f32)], grosor: f32) -> Elemento {
        Elemento {
            figura: Figura::Lapiz {
                puntos: puntos.iter().map(|&(x, y)| Punto2::nuevo(x, y)).collect(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
            grosor,
            ..Elemento::default()
        }
    }

    #[test]
    fn una_foto_grande_entra_en_el_lienzo_partida_por_dos_hasta_caber_como_en_el_movil() {
        assert_eq!(medida_en_el_lienzo((4000, 3000)), (2000.0, 1500.0));
        assert_eq!(medida_en_el_lienzo((4096, 3072)), (2048.0, 1536.0));
        assert_eq!(medida_en_el_lienzo((3201, 2400)), (1601.0, 1200.0));
        assert_eq!(medida_en_el_lienzo((9000, 100)), (1125.0, 13.0));
        // Caso negativo: una foto que ya cabe no cambia, ni una de cero peta.
        assert_eq!(medida_en_el_lienzo((1200, 900)), (1200.0, 900.0));
        assert_eq!(medida_en_el_lienzo((0, 0)), (1.0, 1.0));
    }

    #[test]
    fn una_foto_nueva_anotada_estrena_su_lienzo_con_la_foto_dentro_y_apunta_la_referencia() {
        let r = raiz("nueva");
        let foto = poner_foto(&r, "foto.jpg");
        let m = mensaje(&r, "m1", None);
        let h = asegurar(&r, "p1", &m, &foto, (4000, 3000), 1_790_000_000_500).unwrap();
        assert_eq!(
            h,
            Hecho {
                id: "foto-m1".into(),
                creado: true,
                adoptados: 0,
                referencia_puesta: true
            }
        );
        let v = json_del_lienzo(&r, "foto-m1");
        let e = &v["elements"][0];
        assert_eq!(v["elements"].as_array().unwrap().len(), 1);
        assert_eq!(e["type"], "image");
        assert_eq!(e["locked"], true);
        assert_eq!((e["x"].as_f64(), e["y"].as_f64()), (Some(0.0), Some(0.0)));
        assert_eq!(
            (e["width"].as_f64(), e["height"].as_f64()),
            (Some(2000.0), Some(1500.0))
        );
        let fichero = e["fileId"].as_str().unwrap();
        let f = &v["files"][fichero];
        assert_eq!(f["mimeType"], "image/jpeg");
        assert_eq!(f["path"], format!("imagenes/{fichero}"));
        assert_eq!(v["backgroundColor"], "#ffffff");
        // La foto, copiada byte a byte donde el lienzo la busca.
        let copia = almacen::carpeta(&r, "p1").join("imagenes").join(fichero);
        assert_eq!(std::fs::read(copia).unwrap(), std::fs::read(&foto).unwrap());
        // Y el mensaje apunta a su lienzo, como el del movil.
        assert_eq!(cuaderno_de(&r)[0].referencia.as_deref(), Some("foto-m1"));
        // Lo lee el lector del PC.
        let l = pixpin_motor2d::excalidraw::leer(
            &std::fs::read_to_string(almacen::lienzo(&r, "p1", "foto-m1")).unwrap(),
        )
        .unwrap();
        assert!(matches!(l.elementos()[0].figura, Figura::Imagen { .. }));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn lo_dibujado_antes_en_el_pixpin2d_pasa_al_lienzo_a_su_escala_una_sola_vez() {
        let r = raiz("adopta");
        let foto = poner_foto(&r, "foto.jpg");
        // Una raya en pixeles de una foto de 4000x3000: en el lienzo, a la mitad.
        let viejo = pixpin2d_de(&foto);
        pixpin_motor2d::guardar(
            &viejo,
            &escena_con(vec![raya(&[(1000.0, 800.0), (3000.0, 2200.0)], 8.0)]),
        )
        .unwrap();
        let m = mensaje(&r, "m2", None);
        let h = asegurar(&r, "p1", &m, &foto, (4000, 3000), 1_790_000_000_600).unwrap();
        assert_eq!(h.adoptados, 1);
        let l = pixpin_motor2d::excalidraw::leer(
            &std::fs::read_to_string(almacen::lienzo(&r, "p1", "foto-m2")).unwrap(),
        )
        .unwrap();
        let els = l.elementos();
        assert_eq!(els.len(), 2, "la foto y la raya");
        assert!(
            matches!(els[0].figura, Figura::Imagen { .. }),
            "la foto sigue debajo"
        );
        let Figura::Lapiz { puntos, .. } = &els[1].figura else {
            panic!("la raya: {:?}", els[1].figura)
        };
        let (a, b) = (puntos.first().unwrap(), puntos.last().unwrap());
        assert!(
            (a.x - 500.0).abs() < 1.0 && (a.y - 400.0).abs() < 1.0,
            "{a:?}"
        );
        assert!(
            (b.x - 1500.0).abs() < 1.0 && (b.y - 1100.0).abs() < 1.0,
            "{b:?}"
        );
        assert!(
            (els[1].grosor - 4.0).abs() < 1e-3,
            "el grosor, a la misma escala: {}",
            els[1].grosor
        );
        // El `.pixpin2d` no se borra: se aparta.
        assert!(!viejo.exists());
        let mut apartado = viejo.clone().into_os_string();
        apartado.push(".adoptado");
        assert!(PathBuf::from(apartado).is_file());
        // Caso negativo: la vez siguiente no vuelve a pasar nada ni se duplica.
        let m = cuaderno_de(&r).remove(0);
        let h2 = asegurar(&r, "p1", &m, &foto, (4000, 3000), 1_790_000_000_700).unwrap();
        assert_eq!(
            (h2.creado, h2.adoptados, h2.referencia_puesta),
            (false, 0, false)
        );
        let l = pixpin_motor2d::excalidraw::leer(
            &std::fs::read_to_string(almacen::lienzo(&r, "p1", "foto-m2")).unwrap(),
        )
        .unwrap();
        assert_eq!(l.elementos().len(), 2);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_pantalla_anotada_guarda_la_foto_debajo_y_la_tinta_encima_editable_a_su_escala() {
        let r = raiz("pantalla");
        let foto = poner_foto(&r, "pantalla.png");
        let m = mensaje(&r, "m7", None);
        // Dos monitores de 1920x1080 lado a lado: 3840x1080, que el movil
        // carga partida por dos. Una raya en pixeles de la captura y otra ya
        // borrada (deshecha) que no debe llegar.
        let mut borrada = raya(&[(0.0, 0.0), (10.0, 10.0)], 4.0);
        borrada.borrado = true;
        let tinta = vec![raya(&[(2000.0, 100.0), (3800.0, 1000.0)], 6.0), borrada];
        let h = con_dibujo(&r, "p1", &m, &foto, (3840, 1080), &tinta, 11).unwrap();
        assert_eq!(
            (h.id.as_str(), h.creado, h.adoptados, h.referencia_puesta),
            ("foto-m7", true, 1, true)
        );
        let l = pixpin_motor2d::excalidraw::leer(
            &std::fs::read_to_string(almacen::lienzo(&r, "p1", "foto-m7")).unwrap(),
        )
        .unwrap();
        let els = l.elementos();
        assert_eq!(els.len(), 2, "la foto y la raya viva");
        assert!(matches!(els[0].figura, Figura::Imagen { .. }));
        assert!(els[0].bloqueado, "la foto, bloqueada debajo");
        assert_eq!((els[0].ancho, els[0].alto), (1920.0, 540.0));
        assert!(!els[1].bloqueado, "la tinta, editable");
        let Figura::Lapiz { puntos, .. } = &els[1].figura else {
            panic!("{:?}", els[1].figura)
        };
        let (a, b) = (puntos.first().unwrap(), puntos.last().unwrap());
        assert!(
            (a.x - 1000.0).abs() < 1.0 && (a.y - 50.0).abs() < 1.0,
            "{a:?}"
        );
        assert!(
            (b.x - 1900.0).abs() < 1.0 && (b.y - 500.0).abs() < 1.0,
            "{b:?}"
        );
        assert!((els[1].grosor - 3.0).abs() < 1e-3);
        assert_eq!(cuaderno_de(&r)[0].referencia.as_deref(), Some("foto-m7"));
        // Caso negativo: si el lienzo ya estaba, no se le duplica la tinta.
        let h2 = con_dibujo(
            &r,
            "p1",
            &cuaderno_de(&r)[0],
            &foto,
            (3840, 1080),
            &tinta,
            12,
        )
        .unwrap();
        assert_eq!((h2.creado, h2.adoptados), (false, 0));
        let l = pixpin_motor2d::excalidraw::leer(
            &std::fs::read_to_string(almacen::lienzo(&r, "p1", "foto-m7")).unwrap(),
        )
        .unwrap();
        assert_eq!(l.elementos().len(), 2);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn seguir_dibujando_tras_guardar_pone_al_dia_el_mismo_lienzo_sin_tocar_la_foto() {
        let r = raiz("pantalla-al-dia");
        let foto = poner_foto(&r, "pantalla.png");
        let m = mensaje(&r, "m9", None);
        let primera = vec![raya(&[(100.0, 100.0), (200.0, 200.0)], 4.0)];
        con_dibujo(&r, "p1", &m, &foto, (1920, 1080), &primera, 20).unwrap();
        // La sesion sigue: la raya de antes y dos mas.
        let mut ahora = primera.clone();
        ahora.push(raya(&[(300.0, 300.0), (400.0, 350.0)], 4.0));
        ahora.push(raya(&[(10.0, 900.0), (900.0, 10.0)], 4.0));
        assert_eq!(
            cambiar_dibujo(&r, "p1", "foto-m9", (1920, 1080), &ahora).unwrap(),
            3
        );
        let l = pixpin_motor2d::excalidraw::leer(
            &std::fs::read_to_string(almacen::lienzo(&r, "p1", "foto-m9")).unwrap(),
        )
        .unwrap();
        let els = l.elementos();
        assert_eq!(
            els.len(),
            4,
            "la foto y las tres rayas, sin duplicar la primera"
        );
        assert!(matches!(els[0].figura, Figura::Imagen { .. }) && els[0].bloqueado);
        assert!(
            els[1..]
                .iter()
                .all(|e| !e.bloqueado && matches!(e.figura, Figura::Lapiz { .. }))
        );
        // Lo de antes queda en el fichero marcado como borrado (para el movil).
        let json = json_del_lienzo(&r, "foto-m9");
        let borrados = json["elements"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["isDeleted"] == true)
            .count();
        assert_eq!(borrados, 1);
        // Y la foto es la misma de antes: mismo fichero.
        assert_eq!(
            json["elements"][0]["fileId"],
            json_del_lienzo(&r, "foto-m9")["elements"][0]["fileId"]
        );
        assert_eq!(json["files"].as_object().unwrap().len(), 1);
        // Limpiar todo y salir deja la foto sola.
        assert_eq!(
            cambiar_dibujo(&r, "p1", "foto-m9", (1920, 1080), &[]).unwrap(),
            0
        );
        let l = pixpin_motor2d::excalidraw::leer(
            &std::fs::read_to_string(almacen::lienzo(&r, "p1", "foto-m9")).unwrap(),
        )
        .unwrap();
        assert_eq!(l.elementos().len(), 1);
        // Un solo mensaje en el cuaderno: poner al dia no manda otro.
        assert_eq!(cuaderno_de(&r).len(), 1);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn poner_al_dia_un_lienzo_que_no_esta_falla_sin_crear_nada() {
        let r = raiz("pantalla-sin-lienzo");
        let tinta = vec![raya(&[(1.0, 1.0), (2.0, 2.0)], 1.0)];
        assert!(cambiar_dibujo(&r, "p1", "foto-nada", (10, 10), &tinta).is_err());
        assert!(!almacen::lienzo(&r, "p1", "foto-nada").exists());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_pantalla_guardada_sin_tinta_deja_el_lienzo_con_la_foto_sola() {
        let r = raiz("pantalla-sola");
        let foto = poner_foto(&r, "pantalla.png");
        let m = mensaje(&r, "m8", None);
        let h = con_dibujo(&r, "p1", &m, &foto, (1920, 1080), &[], 13).unwrap();
        assert_eq!((h.creado, h.adoptados), (true, 0));
        assert_eq!(
            json_del_lienzo(&r, "foto-m8")["elements"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn una_foto_con_lienzo_del_movil_no_estrena_otro_ni_toca_la_foto_de_alli() {
        let r = raiz("movil");
        let foto = poner_foto(&r, "foto.jpg");
        let del_movil = r##"{"elements":[{"id":"foto-1-0","type":"image","x":0,"y":0,"width":1600,"height":1200,"seed":1,"version":1,"isDeleted":false,"locked":true,"fileId":"AbC"}],"files":{"AbC":{"id":"AbC","mimeType":"image/jpeg","path":"pixpin:files/pins/draw/files/AbC","created":1}},"backgroundColor":"#14213d"}"##;
        std::fs::create_dir_all(almacen::carpeta(&r, "p1").join("lienzos")).unwrap();
        std::fs::write(almacen::lienzo(&r, "p1", "foto-m3"), del_movil).unwrap();
        let m = mensaje(&r, "m3", None);
        let h = asegurar(&r, "p1", &m, &foto, (3200, 2400), 5).unwrap();
        assert_eq!(
            (h.id.as_str(), h.creado, h.adoptados),
            ("foto-m3", false, 0)
        );
        assert_eq!(
            std::fs::read_to_string(almacen::lienzo(&r, "p1", "foto-m3")).unwrap(),
            del_movil,
            "sin nada que pasar, el lienzo del movil no cambia ni un byte"
        );
        assert!(
            !almacen::carpeta(&r, "p1").join("imagenes").exists(),
            "ni se copia la foto"
        );
        // La referencia se apunta igual, como hace el movil al abrirla.
        assert!(h.referencia_puesta);
        assert_eq!(cuaderno_de(&r)[0].referencia.as_deref(), Some("foto-m3"));
        // Y con un `.pixpin2d` viejo, lo suyo va encima de lo del movil, a la
        // escala de SU foto (1600 de 3200: la mitad).
        pixpin_motor2d::guardar(
            &pixpin2d_de(&foto),
            &escena_con(vec![raya(&[(200.0, 200.0), (400.0, 600.0)], 2.0)]),
        )
        .unwrap();
        let h = asegurar(&r, "p1", &cuaderno_de(&r)[0], &foto, (3200, 2400), 6).unwrap();
        assert_eq!((h.creado, h.adoptados), (false, 1));
        let v = json_del_lienzo(&r, "foto-m3");
        assert_eq!(v["elements"].as_array().unwrap().len(), 2);
        assert_eq!(v["elements"][0]["fileId"], "AbC");
        assert_eq!(
            v["backgroundColor"], "#14213d",
            "el papel del movil se queda"
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_referencia_de_una_foto_vieja_manda_y_no_se_reescribe() {
        let r = raiz("referencia");
        let foto = poner_foto(&r, "foto.png");
        let m = mensaje(&r, "m4", Some("dib-antiguo"));
        let antes =
            std::fs::read_to_string(almacen::carpeta(&r, "p1").join("guardados.jsonl")).unwrap();
        let h = asegurar(&r, "p1", &m, &foto, (800, 600), 7).unwrap();
        assert_eq!(
            (h.id.as_str(), h.creado, h.referencia_puesta),
            ("dib-antiguo", true, false)
        );
        assert!(almacen::lienzo(&r, "p1", "dib-antiguo").is_file());
        assert!(!almacen::lienzo(&r, "p1", "foto-m4").exists());
        assert_eq!(
            std::fs::read_to_string(almacen::carpeta(&r, "p1").join("guardados.jsonl")).unwrap(),
            antes,
            "el cuaderno no se toca"
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn sin_la_foto_en_disco_no_se_crea_nada_ni_se_apunta_nada() {
        // Caso negativo: la foto no llego (se borro o sigue en el movil).
        let r = raiz("sinfoto");
        let m = mensaje(&r, "m5", None);
        let falta = almacen::carpeta(&r, "p1")
            .join("archivos")
            .join("no-esta.jpg");
        assert!(asegurar(&r, "p1", &m, &falta, (800, 600), 8).is_err());
        assert!(!almacen::lienzo(&r, "p1", "foto-m5").exists());
        assert_eq!(cuaderno_de(&r)[0].referencia, None);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_pixpin2d_roto_se_deja_donde_esta_y_uno_vacio_se_aparta_sin_anadir_nada() {
        let r = raiz("roto");
        let foto = poner_foto(&r, "foto.jpg");
        let viejo = pixpin2d_de(&foto);
        std::fs::write(&viejo, "{no es json").unwrap();
        let m = mensaje(&r, "m6", None);
        let h = asegurar(&r, "p1", &m, &foto, (800, 600), 9).unwrap();
        assert_eq!((h.creado, h.adoptados), (true, 0));
        assert_eq!(
            std::fs::read_to_string(&viejo).unwrap(),
            "{no es json",
            "no se pierde"
        );
        // Uno sin nada visible (todo borrado) se aparta y el lienzo sigue con la foto sola.
        pixpin_motor2d::guardar(&viejo, &pixpin_motor2d::Escena::nueva()).unwrap();
        let h = asegurar(&r, "p1", &cuaderno_de(&r)[0], &foto, (800, 600), 10).unwrap();
        assert_eq!(h.adoptados, 0);
        assert!(!viejo.exists());
        assert_eq!(
            json_del_lienzo(&r, "foto-m6")["elements"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn el_tipo_de_la_foto_sale_de_sus_bytes_antes_que_de_su_nombre() {
        let p = Path::new("x.jpg");
        assert_eq!(tipo_de(&[0x89, b'P', b'N', b'G', 0], p), "image/png");
        assert_eq!(tipo_de(b"RIFF\0\0\0\0WEBPVP8", p), "image/webp");
        assert_eq!(
            tipo_de(&[0xFF, 0xD8, 0xFF], Path::new("x.png")),
            "image/jpeg"
        );
        // Caso negativo: bytes que no se reconocen, manda la extension.
        assert_eq!(tipo_de(b"????", Path::new("x.PNG")), "image/png");
        assert_eq!(tipo_de(b"????", Path::new("x")), "image/jpeg");
    }
}
