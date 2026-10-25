//! **Donde estan en el PC los comentarios de una nota Markdown** (H12,
//! 30-sep-2026). El formato es `pixpin_docs::md_comentarios`; aqui solo el
//! sitio, que es el que hace que viajen con la nota.
//!
//! - Una nota de un proyecto —un mensaje `NOTA` del chat o una hoja `nota`
//!   de `proyecto.json`, que tienen el mismo codigo unico— lleva sus
//!   comentarios en `pins/draw/anot-<codigo>.comentarios.json`, junto a lo
//!   anotado sobre los adjuntos (`pixpin_sincro::anotado`). Asi viajan con
//!   su nota sin nada nuevo en la sincronizacion y se borran con ella.
//! - Un `.md` adjunto del chat, igual, con el codigo de su mensaje.
//! - Un `.md` suelto (abierto desde el Explorador), a su lado:
//!   `apuntes.md` → `apuntes.comentarios.json`. No viaja: tampoco el `.md`.

use std::path::{Path, PathBuf};

use pixpin_sincro::anotado as a;
use pixpin_sincro::disco::Disco;

use crate::vista::{self, DiscoPc};

fn rel_de(codigo: &str) -> String {
    a::rel(&format!("{}{codigo}", a::PREFIJO), a::COMENTARIOS)
}

/// Los de una nota del proyecto `ficha` por su codigo unico. `None` si el
/// proyecto no esta en el almacen.
pub fn de_la_nota(raiz: &Path, ficha: &str, codigo: &str) -> Option<PathBuf> {
    let chat = vista::chat_de_ficha(raiz, ficha)?;
    Some(DiscoPc::nuevo(raiz).ruta(&chat, &rel_de(codigo)))
}

/// Los de un `.md`: con el codigo de su mensaje si es un adjunto del chat,
/// a su lado si no.
pub fn del_md(raiz: &Path, md: &Path) -> PathBuf {
    match crate::anotado::adjunto_de(raiz, md) {
        Some(x) => DiscoPc::nuevo(raiz).ruta(&x.chat, &rel_de(&x.uid)),
        None => junto_a(md),
    }
}

/// `apuntes.md` → `apuntes.comentarios.json`.
pub fn junto_a(md: &Path) -> PathBuf {
    md.with_extension(a::COMENTARIOS.trim_start_matches('.'))
}

/// El texto del fichero; uno que no esta es «sin comentarios» (cadena vacia).
/// `None` si esta pero no se lee: quien llama no debe escribir encima.
pub fn leer(f: &Path) -> Option<String> {
    match std::fs::read(f) {
        Ok(b) => Some(String::from_utf8_lossy(&b).into_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(String::new()),
        Err(_) => None,
    }
}

/// Lo escribe por un temporal: a medias no se queda nunca. El temporal
/// (`….comentarios.json.tmp`) no es de lo anotado y no viaja.
pub fn escribir(f: &Path, texto: &str) -> std::io::Result<()> {
    if let Some(p) = f.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut t = f.as_os_str().to_owned();
    t.push(".tmp");
    let t = PathBuf::from(t);
    std::fs::write(&t, texto.as_bytes())?;
    std::fs::rename(&t, f)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_md_suelto_lleva_los_comentarios_a_su_lado() {
        assert_eq!(
            junto_a(Path::new(r"C:\a\apuntes.md")),
            PathBuf::from(r"C:\a\apuntes.comentarios.json")
        );
        assert_eq!(
            junto_a(Path::new(r"C:\a\v1.2.md")),
            PathBuf::from(r"C:\a\v1.2.comentarios.json")
        );
        let raiz =
            std::env::temp_dir().join(format!("pixpin-coment-suelto-{}", std::process::id()));
        // Fuera de todo almacen: a su lado.
        let md = raiz.join("fuera").join("x.md");
        assert_eq!(
            del_md(&raiz, &md),
            raiz.join("fuera").join("x.comentarios.json")
        );
    }

    #[test]
    fn una_nota_de_un_proyecto_que_no_esta_no_tiene_sitio() {
        let raiz = std::env::temp_dir().join(format!("pixpin-coment-nada-{}", std::process::id()));
        assert_eq!(de_la_nota(&raiz, "no-existe", "ABCDE23456"), None);
    }

    #[test]
    fn leer_lo_que_no_esta_es_vacio_y_escribir_no_deja_temporales() {
        let d = std::env::temp_dir().join(format!("pixpin-coment-io-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let f = d.join("sub").join("anot-ABCDE23456.comentarios.json");
        assert_eq!(leer(&f).as_deref(), Some(""));
        escribir(&f, "{}\n").unwrap();
        assert_eq!(leer(&f).as_deref(), Some("{}\n"));
        assert!(
            !d.join("sub")
                .join("anot-ABCDE23456.comentarios.json.tmp")
                .exists()
        );
        // Una carpeta en su sitio no se lee como vacio: no se pisaria.
        let carpeta = d.join("carpeta.comentarios.json");
        std::fs::create_dir_all(&carpeta).unwrap();
        assert_eq!(leer(&carpeta), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
