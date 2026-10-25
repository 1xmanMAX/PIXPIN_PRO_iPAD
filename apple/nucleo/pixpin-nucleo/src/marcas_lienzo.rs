//! **Los marcadores del lienzo** (F5; `ventana_editor/marcas.rs` de
//! Windows): sitios del dibujo con su emoticono, en el riel de la derecha.
//! Se guardan donde Windows: junto al dibujo del movil si es un lienzo de un
//! chat (`pins/draw/<dibujo>.marcas`, que viaja al sincronizar; los de antes
//! se pasan la primera vez) y, si no, en `<nombre>.excalidraw.pixpin-marcas`
//! al lado. La linea es la del movil (`id:x:y:emoji|…`, `marcas::a_texto`).

use std::path::{Path, PathBuf};

use pixpin_motor2d::marcas;
use pixpin_proyecto::anotado as an;

use crate::editor::EditorLienzo;
use crate::{ErrorNucleo, MarcaDocumento};

/// El fichero de las marcas del dibujo `dibujo` (`ruta_de_marcas`).
fn ruta_de_marcas(dibujo: &Path) -> PathBuf {
    let mut nombre = dibujo
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "dibujo".into());
    nombre.push_str(".pixpin-marcas");
    let de_siempre = dibujo.with_file_name(nombre);
    match an::marcas_del_lienzo(dibujo) {
        Some(nueva) => {
            an::copiar_si_falta(&de_siempre, &nueva);
            nueva
        }
        None => de_siempre,
    }
}

fn a_marca(m: &MarcaDocumento) -> Option<marcas::Marca> {
    (m.x.is_finite() && m.y.is_finite()).then(|| marcas::Marca {
        id: m.id,
        emoji: m.emoji.replace(['|', '\n', '\r'], ""),
        x: m.x,
        y: m.y,
    })
}

#[uniffi::export]
impl EditorLienzo {
    /// **Los marcadores de este lienzo**, en el orden del riel (de arriba
    /// abajo). Sin fichero, ninguno. `x` e `y` son del mundo del lienzo.
    pub fn marcas(&self) -> Vec<MarcaDocumento> {
        std::fs::read_to_string(ruta_de_marcas(self.fichero()))
            .map(|t| marcas::de_texto(t.trim()))
            .unwrap_or_default()
            .into_iter()
            .map(|m| MarcaDocumento { id: m.id, x: m.x, y: m.y, emoji: m.emoji })
            .collect()
    }

    /// **Un marcador mas** en `(x, y)` del mundo (si ya hay 24, se va el mas
    /// viejo). Devuelve la lista como queda.
    pub fn poner_marca(&self, x: f64, y: f64, emoji: String) -> Result<Vec<MarcaDocumento>, ErrorNucleo> {
        let antes: Vec<marcas::Marca> = self.marcas().iter().filter_map(a_marca).collect();
        let lista = marcas::con(&antes, x, y, &emoji, crate::ahora_ms());
        self.escribir_marcas(&lista)?;
        Ok(self.marcas())
    }

    /// Mueve el marcador `id` a `(x, y)`.
    pub fn mover_marca(&self, id: i64, x: f64, y: f64) -> Result<Vec<MarcaDocumento>, ErrorNucleo> {
        let antes: Vec<marcas::Marca> = self.marcas().iter().filter_map(a_marca).collect();
        self.escribir_marcas(&marcas::movida(&antes, id, x, y))?;
        Ok(self.marcas())
    }

    /// Quita el marcador `id`.
    pub fn quitar_marca(&self, id: i64) -> Result<Vec<MarcaDocumento>, ErrorNucleo> {
        let antes: Vec<marcas::Marca> = self.marcas().iter().filter_map(a_marca).collect();
        self.escribir_marcas(&marcas::sin(&antes, id))?;
        Ok(self.marcas())
    }
}

impl EditorLienzo {
    /// Las guarda. Sin marcas, el fichero de al lado se borra; el que viaja
    /// se deja vacio, como el movil (uno que desaparece no le dice nada al
    /// otro aparato).
    fn escribir_marcas(&self, lista: &[marcas::Marca]) -> Result<(), ErrorNucleo> {
        let ruta = ruta_de_marcas(self.fichero());
        if !ruta.to_string_lossy().ends_with(".pixpin-marcas") {
            an::escribir(&ruta, &marcas::a_texto(lista))?;
            return Ok(());
        }
        if lista.is_empty() {
            return match std::fs::remove_file(&ruta) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
                _ => Ok(()),
            };
        }
        let temporal = ruta.with_extension("pixpin-marcas.tmp");
        std::fs::write(&temporal, marcas::a_texto(lista))?;
        std::fs::rename(&temporal, &ruta)?;
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_ponen_se_mueven_y_se_quitan_junto_al_dibujo() {
        let d = std::env::temp_dir().join(format!("pixpin-nucleo-marcas-lienzo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let e = EditorLienzo::de_fichero(d.join("dib.excalidraw").display().to_string(), true).unwrap();
        assert!(e.marcas().is_empty());
        let l = e.poner_marca(10.0, 200.0, "⭐".into()).unwrap();
        let l = {
            std::thread::sleep(std::time::Duration::from_millis(2));
            let _ = l;
            e.poner_marca(5.0, 50.0, "🔖".into()).unwrap()
        };
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].emoji, "🔖", "en el orden del riel: de arriba abajo");
        assert!(d.join("dib.excalidraw.pixpin-marcas").exists());
        let id = l[0].id;
        let l = e.mover_marca(id, 5.0, 900.0).unwrap();
        assert_eq!(l[1].id, id);
        let l = e.quitar_marca(id).unwrap();
        assert_eq!(l.len(), 1);
        let id = l[0].id;
        assert!(e.quitar_marca(id).unwrap().is_empty());
        assert!(!d.join("dib.excalidraw.pixpin-marcas").exists());
    }
}
