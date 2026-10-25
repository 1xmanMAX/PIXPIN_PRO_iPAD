//! **El lienzo a pantalla completa o en ventana** (F11 de Windows,
//! `ventana_editor/modo_ventana.rs`): la ultima eleccion, una para todos los
//! lienzos y entre sesiones, en `estado.toml` (`lienzo_pantalla_completa`),
//! como en Windows.
//!
//! Lo que el Mac hace distinto: el sitio y el tamano de la ventana los
//! recuerda AppKit (`setFrameAutosaveName`), no `lienzo_ventana`; y un
//! lienzo que nunca se toco abre en ventana y no a pantalla completa, porque
//! en el Mac la pantalla completa es otro escritorio entero (en Windows es la
//! ventana sin marco tapando la pantalla).

use pixpin_store::estado;

use crate::{ErrorNucleo, Nucleo};

#[uniffi::export]
impl Nucleo {
    /// Si el ultimo lienzo se dejo a pantalla completa (`None`: nunca se
    /// toco).
    pub fn lienzo_pantalla_completa(&self) -> Option<bool> {
        estado::cargar(&self.ubicacion()).lienzo_pantalla_completa
    }

    /// Apunta como se dejo el lienzo, sin tocar lo demas del estado. Si no
    /// cambio, no escribe.
    pub fn recordar_lienzo_pantalla_completa(&self, si: bool) -> Result<(), ErrorNucleo> {
        let u = self.ubicacion();
        let mut e = estado::cargar(&u);
        if e.lienzo_pantalla_completa == Some(si) {
            return Ok(());
        }
        e.lienzo_pantalla_completa = Some(si);
        estado::guardar(&u, &e)?;
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_pantalla_completa_del_lienzo_se_recuerda_entre_sesiones() {
        let raiz = std::env::temp_dir().join(format!("pixpin-modo-lienzo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let n = Nucleo::abrir(raiz.display().to_string(), "Mac".into()).unwrap();
        assert_eq!(n.lienzo_pantalla_completa(), None, "nunca se toco");
        n.recordar_lienzo_pantalla_completa(true).unwrap();
        let otra = Nucleo::abrir(raiz.display().to_string(), "Mac".into()).unwrap();
        assert_eq!(otra.lienzo_pantalla_completa(), Some(true));
        otra.recordar_lienzo_pantalla_completa(false).unwrap();
        assert_eq!(n.lienzo_pantalla_completa(), Some(false));
        let _ = std::fs::remove_dir_all(raiz);
    }
}
