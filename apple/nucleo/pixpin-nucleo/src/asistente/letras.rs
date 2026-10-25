//! **Las letras para maquetar Word y PowerPoint fuera del lector** (el
//! asistente, E1): el mismo `pixpin-office` y las mismas letras de Core Text
//! que el lector (`office.rs`), para que lo que el asistente busca o anota
//! caiga en el mismo sitio que se ve.
//!
//! Swift instala su medidor una vez al arrancar
//! ([`instalar_medidor_de_letras`]); sin el (las pruebas) se mide con
//! `MedidorFijo`, que da las mismas paginas en cualquier equipo.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use pixpin_office::{Documento, Medidor, MedidorFijo, Pagina};

use crate::office::{MedidorConCache, MedidorLetras};

static MEDIDOR: OnceLock<MedidorConCache> = OnceLock::new();

/// **Instala el medidor de Core Text** para el asistente (una vez; las
/// demas no hacen nada).
#[uniffi::export]
pub fn instalar_medidor_de_letras(medidor: Box<dyn MedidorLetras>) {
    let _ = MEDIDOR.set(MedidorConCache::nuevo(medidor));
}

/// Un Word o un PowerPoint abierto para leerlo por paginas.
pub struct OfficeAbierto {
    documento: Mutex<Documento>,
}

/// Abre `ruta` como Office.
pub fn abrir_office(ruta: &Path) -> Result<OfficeAbierto, String> {
    let d = Documento::abrir(ruta).map_err(|e| format!("no se abre como Office: {e}"))?;
    Ok(OfficeAbierto { documento: Mutex::new(d) })
}

impl OfficeAbierto {
    /// Maqueta la pagina `i` (desde 0) y se la da a `f` con el medidor.
    /// `None` si no existe.
    pub fn con_pagina<R>(&self, i: usize, f: impl FnOnce(&Pagina, &dyn Medidor) -> R) -> Option<R> {
        let mut d = self.documento.lock().unwrap_or_else(|e| e.into_inner());
        match MEDIDOR.get() {
            Some(m) => {
                let p = d.pagina(i, m)?;
                Some(f(&p, m))
            }
            None => {
                let m = MedidorFijo::default();
                let p = d.pagina(i, &m)?;
                Some(f(&p, &m))
            }
        }
    }
}
