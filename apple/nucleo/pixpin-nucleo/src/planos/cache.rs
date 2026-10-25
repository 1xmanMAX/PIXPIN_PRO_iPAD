//! **El plano ya leido, en la cache** (`plano_cad::cargar` de Windows): leer
//! un DWG grande tarda segundos; abrir otra vez el mismo plano lee lo que se
//! guardo la primera vez y es casi instantaneo.
//!
//! Va en `<raiz>/cache/cad/<clave>-l1[-<huella>].pxcad`, con la clave de
//! Windows (FNV-1a de la ruta, el tamano y la fecha: si el plano cambia,
//! cambia la clave) y la huella de las letras del usuario (`fuentes-shx`:
//! con una letra nueva el plano se vuelve a leer con ella). El visor de Mac
//! lee **por capas** (se encienden y apagan), asi que lo guardado es el
//! plano por capas (`pixpin_cad::web::a_bytes`, lo mismo que guarda
//! Windows para exportar, `.pxcapas`), no el modelo entero de una pieza del
//! `.pxcad` de Windows. Solo vive en este Mac: no viaja.
//!
//! Las letras que el plano pide y no estan se apuntan en el `sin_dibujar`
//! del primer grupo (`FUENTE_QUE_FALTA`, como Android y Windows), para que
//! lo guardado las siga diciendo.
//!
//! Se guardan los [`EN_CACHE`] mas recientes (abrir uno lo toca). Lo demas
//! de la carpeta (la capa de un plano que no es de ningun chat, sus textos)
//! no se poda con los planos.
//!
//! En Windows la lectura va en otro proceso por si un DWG roto tumbara el
//! programa; aqui va en el mismo (la libreria de DWG se defiende con
//! `catch_unwind`, y el nucleo no tiene un ejecutable propio que lanzar).

use std::path::{Path, PathBuf};

use pixpin_cad::convertir::PlanoPorCapas;
use pixpin_cad::letras;

/// Cuantos planos leidos se guardan (los mas recientes).
pub(crate) const EN_CACHE: usize = 24;

/// Cambia cuando cambia lo que se guarda del plano leido (la de Windows: la
/// `l1` trae las letras que faltan).
const REVISION_DE_LETRAS: &str = "l1";

/// La carpeta de la cache de planos.
pub(crate) fn carpeta(raiz: &Path) -> PathBuf {
    raiz.join("cache").join("cad")
}

/// El nombre del plano leido en la cache (`nombre_en_cache` de Windows, con
/// la terminacion de lo que guarda el Mac).
pub(crate) fn nombre_en_cache(clave: &str, huella: &str) -> String {
    if huella.is_empty() {
        format!("{clave}-{REVISION_DE_LETRAS}.pxcad")
    } else {
        format!("{clave}-{REVISION_DE_LETRAS}-{huella}.pxcad")
    }
}

/// Deja los `EN_CACHE` de cada clase mas recientes: los planos leidos
/// (`.pxcad`) y sus textos (`-textos.json`). Lo anotado no se toca.
pub(crate) fn podar(carpeta: &Path) {
    let Ok(d) = std::fs::read_dir(carpeta) else {
        return;
    };
    let mut planos = Vec::new();
    let mut textos = Vec::new();
    for e in d.flatten() {
        let nombre = e.file_name().to_string_lossy().into_owned();
        let Some(fecha) = e.metadata().ok().and_then(|m| m.modified().ok()) else {
            continue;
        };
        if nombre.ends_with(".pxcad") {
            planos.push((fecha, e.path()));
        } else if nombre.ends_with("-textos.json") {
            textos.push((fecha, e.path()));
        }
    }
    for mut v in [planos, textos] {
        v.sort_by_key(|x| std::cmp::Reverse(x.0));
        for (_, p) in v.into_iter().skip(EN_CACHE) {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Lo toca: es de los recientes.
fn tocar(ruta: &Path) {
    if let Ok(f) = std::fs::File::options().append(true).open(ruta) {
        let _ = f.set_modified(std::time::SystemTime::now());
    }
}

/// Escribe por un temporal: quien lea a la vez nunca ve uno a medias.
fn escribir(destino: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = destino.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, destino)
}

/// **Lee el plano por capas** con las letras del usuario y apunta las que
/// pide y no estan (`convertir_con_letras` de Windows, por capas).
pub(crate) fn convertir(ruta: &Path, letras_del_usuario: &Path) -> Result<PlanoPorCapas, String> {
    letras::poner_carpeta(Some(letras_del_usuario.to_path_buf()));
    letras::sacar_lo_que_falta();
    let mut p = pixpin_cad::convertir::convertir_fichero_por_capas(ruta)?;
    let faltan = letras::sacar_lo_que_falta();
    if let Some(g) = p.grupos.first_mut() {
        for f in faltan {
            g.modelo
                .sin_dibujar
                .push((format!("{}{f}", letras::FUENTE_QUE_FALTA), 1));
        }
    }
    Ok(p)
}

/// **El plano por capas**: de la cache o leyendolo (y guardandolo). Bloquea.
pub(crate) fn leer_por_capas(raiz: &Path, ruta: &Path) -> Result<PlanoPorCapas, String> {
    let clave = super::clave(ruta).ok_or("no se puede leer el fichero")?;
    let c = carpeta(raiz);
    let mias = letras::carpeta_en(raiz);
    let destino = c.join(nombre_en_cache(&clave, &letras::huella(&mias)));
    if let Ok(b) = std::fs::read(&destino)
        && let Some(p) = pixpin_cad::web::de_bytes(&b)
    {
        tocar(&destino);
        return Ok(p);
    }
    let p = convertir(ruta, &mias)?;
    // Sin cache se sigue: solo tardara otra vez la proxima.
    if std::fs::create_dir_all(&c).is_ok()
        && escribir(&destino, &pixpin_cad::web::a_bytes(&p)).is_ok()
    {
        podar(&c);
    }
    Ok(p)
}
