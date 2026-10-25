//! **Un libro suelto** (un `.xlsx`, `.ods` o `.csv` fuera de un chat: el de
//! la app Archivos del iPad, el que llega por «Abrir con»): se abre con el
//! mismo editor que las hojas del chat, como el visor de hojas de Windows
//! (`visor_hoja::lanzar`). El archivo **no cambia** (ver `Origen::Suelto`):
//! lo editado sale como una copia con [`HojaDeCalculo::como_xlsx`].

use std::path::PathBuf;
use std::sync::Arc;

use pixpin_proyecto::importar_hojas;
use pixpin_proyecto::tabla_xlsx::{HojaParaVer, a_xlsx};

use super::editor::HojaEditable;
use super::{HojaDeCalculo, Origen};
use crate::{ErrorNucleo, Nucleo};

#[uniffi::export]
impl Nucleo {
    /// **Abre un libro suelto por su ruta** (Excel, OpenDocument o CSV) con
    /// todas sus hojas, para verlo y editarlo aqui. `NoExiste` si no esta;
    /// `Argumento` si no se lee (un `.xls` antiguo, uno roto). Guardar no
    /// escribe nada: el archivo no cambia.
    pub fn abrir_libro(&self, ruta: String) -> Result<Arc<HojaDeCalculo>, ErrorNucleo> {
        let r = PathBuf::from(&ruta);
        if !r.is_file() {
            return Err(ErrorNucleo::NoExiste { que: ruta });
        }
        let nombre = r
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let hojas: Vec<HojaEditable> = importar_hojas::leer(&r, &nombre, crate::ahora_ms())
            .map_err(|e| ErrorNucleo::Argumento {
                detalle: e.to_string(),
            })?
            .into_iter()
            .map(|h| HojaEditable::nueva(h.nombre, h.tabla, None))
            .collect();
        Ok(Arc::new(HojaDeCalculo::nueva(
            Origen::Suelto { ruta: r },
            hojas,
        )?))
    }
}

#[uniffi::export]
impl HojaDeCalculo {
    /// Si es un libro suelto (abierto con [`Nucleo::abrir_libro`]): lo
    /// editado no se guarda en ningun sitio, sale con [`Self::como_xlsx`].
    pub fn es_suelta(&self) -> bool {
        matches!(self.tomar().origen, Origen::Suelto { .. })
    }

    /// **El libro como un `.xlsx`** (todas sus hojas, con la activa delante):
    /// los valores ya calculados, el formato y los anchos
    /// (`pixpin_proyecto::tabla_xlsx`, el de Windows). Para compartir una
    /// copia con lo editado.
    pub fn como_xlsx(&self) -> Result<Vec<u8>, ErrorNucleo> {
        let g = self.tomar();
        let ed = &g.editor;
        let hojas: Vec<HojaParaVer> = ed
            .hojas
            .iter()
            .map(|h| HojaParaVer {
                nombre: &h.nombre,
                tabla: &h.tabla,
                valores: Some(h.valores()),
            })
            .collect();
        a_xlsx(&hojas, ed.activa).map_err(|e| ErrorNucleo::Disco {
            detalle: e.to_string(),
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::tabla::{self, Tabla};

    fn nucleo(d: &std::path::Path) -> Arc<Nucleo> {
        std::fs::create_dir_all(d).unwrap();
        Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap()
    }

    #[test]
    fn un_libro_suelto_se_edita_y_el_archivo_no_cambia() {
        let d = std::env::temp_dir().join(format!("pixpin-libro-suelto-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let mut t = Tabla::default();
        t.poner(tabla::ref_de("A1").unwrap(), "2");
        t.poner(tabla::ref_de("A2").unwrap(), "=A1*3");
        let bytes = a_xlsx(
            &[HojaParaVer {
                nombre: "Cuentas",
                tabla: &t,
                valores: None,
            }],
            0,
        )
        .unwrap();
        let libro = d.join("cuentas.xlsx");
        std::fs::write(&libro, &bytes).unwrap();
        let n = nucleo(&d.join("datos"));
        let h = n.abrir_libro(libro.display().to_string()).unwrap();
        assert!(h.es_suelta());
        assert_eq!(h.ruta_original(), Some(libro.display().to_string()));
        assert!(h.poner(0, 0, "5".into()));
        // Avisa una vez de que el archivo no cambia.
        assert!(h.aviso_al_editar());
        assert!(!h.aviso_al_editar());
        // Guardar no escribe: el archivo sigue igual.
        assert!(!h.guardar().unwrap());
        assert_eq!(std::fs::read(&libro).unwrap(), bytes);
        // La copia lleva lo editado (los valores, ya calculados).
        let copia = h.como_xlsx().unwrap();
        let otra = d.join("copia.xlsx");
        std::fs::write(&otra, &copia).unwrap();
        let h2 = n.abrir_libro(otra.display().to_string()).unwrap();
        let celdas = h2.celdas(0, 3, 0, 1);
        let a1 = celdas.iter().find(|c| c.fila == 0).unwrap();
        assert_eq!(a1.numero, Some(5.0));
        assert_eq!(h2.hojas().len(), 1);
        // Caso negativo: lo que no esta no se abre.
        assert!(n.abrir_libro(d.join("no.xlsx").display().to_string()).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
