//! Donde viven los ajustes.
//!
//! El modo portable no se activa con una bandera ni con un instalador
//! distinto: **lo decide la presencia del fichero de ajustes junto al
//! ejecutable**. Asi copiar la carpeta a un USB y llevarsela funciona sin que
//! el usuario tenga que saber nada, y una instalacion normal nunca escribe
//! junto al .exe (donde probablemente no tendria permisos).

use std::path::{Path, PathBuf};

/// Nombre del fichero de ajustes, igual en los dos modos.
pub const NOMBRE_AJUSTES: &str = "pixpinmax.toml";

/// Carpeta dentro de APPDATA en el modo instalado.
const CARPETA_APPDATA: &str = "PixPinMax";

/// Donde estan los datos de la aplicacion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ubicacion {
    /// Todo junto al ejecutable. Cero rastro en el registro.
    Portable { raiz: PathBuf },
    /// Bajo `%APPDATA%\PixPinMax`.
    Instalado { raiz: PathBuf },
}

impl Ubicacion {
    /// Raiz de datos, sea cual sea el modo.
    pub fn raiz(&self) -> &Path {
        match self {
            Ubicacion::Portable { raiz } | Ubicacion::Instalado { raiz } => raiz,
        }
    }

    /// Ruta completa del fichero de ajustes.
    pub fn fichero_ajustes(&self) -> PathBuf {
        self.raiz().join(NOMBRE_AJUSTES)
    }

    pub fn es_portable(&self) -> bool {
        matches!(self, Ubicacion::Portable { .. })
    }
}

/// Decide el modo a partir del directorio del ejecutable y de APPDATA.
///
/// Ambas rutas se pasan como parametro en vez de consultarlas aqui para que
/// esto sea logica pura y se pueda probar sin tocar el entorno real.
pub fn resolver(dir_exe: &Path, appdata: &Path) -> Ubicacion {
    if dir_exe.join(NOMBRE_AJUSTES).is_file() {
        Ubicacion::Portable {
            raiz: dir_exe.to_path_buf(),
        }
    } else {
        Ubicacion::Instalado {
            raiz: appdata.join(CARPETA_APPDATA),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    /// Crea un directorio temporal propio del test, sin dependencias externas.
    fn temporal(etiqueta: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pixpin-pruebas-{etiqueta}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn con_fichero_junto_al_exe_es_portable() {
        let exe = temporal("portable");
        fs::write(exe.join(NOMBRE_AJUSTES), "").unwrap();

        let u = resolver(&exe, Path::new(r"C:\NoUsado"));

        assert_eq!(u, Ubicacion::Portable { raiz: exe.clone() });
        assert!(u.es_portable());
        assert_eq!(u.fichero_ajustes(), exe.join(NOMBRE_AJUSTES));
    }

    #[test]
    fn sin_fichero_junto_al_exe_es_instalado() {
        let exe = temporal("instalado");
        let appdata = temporal("appdata");

        let u = resolver(&exe, &appdata);

        assert_eq!(
            u,
            Ubicacion::Instalado {
                raiz: appdata.join("PixPinMax")
            }
        );
        assert!(!u.es_portable());
        assert_eq!(
            u.fichero_ajustes(),
            appdata.join("PixPinMax").join(NOMBRE_AJUSTES)
        );
    }

    #[test]
    fn un_directorio_con_ese_nombre_no_cuenta_como_portable() {
        // Si alguien crea por error una CARPETA llamada pixpinmax.toml, no debe
        // activarse el modo portable: se buscaria un fichero de ajustes que no
        // existe y la app arrancaria con un estado incoherente.
        let exe = temporal("directorio");
        fs::create_dir(exe.join(NOMBRE_AJUSTES)).unwrap();
        let appdata = temporal("appdata2");

        let u = resolver(&exe, &appdata);

        assert!(
            !u.es_portable(),
            "una carpeta no debe activar el modo portable"
        );
    }
}
