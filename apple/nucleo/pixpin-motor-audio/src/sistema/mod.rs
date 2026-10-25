//! Lo que toca Windows: COM, WASAPI y Media Foundation, con su `unsafe`
//! acotado y comentado. Nada de aqui decide nada de sonido; eso esta en los
//! modulos puros del crate.

pub(crate) mod aparatos;
pub(crate) mod captura;
pub(crate) mod com;
pub(crate) mod m4a;
pub(crate) mod proceso;

use std::path::Path;

use crate::ErrorMotor;
use crate::plataforma::{Pedido, Sistema, Tarea};

/// **El sistema Windows**: WASAPI para capturar, Media Foundation para el
/// AAC, MMCSS para la prioridad y el QPC como reloj.
pub(crate) struct Windows;

impl Sistema for Windows {
    type Plataforma = captura::PlataformaWindows;
    type Salida = m4a::Escritor;
    type Prioridad = com::Mmcss;

    fn captura(
        &self,
        pedido: &Pedido,
    ) -> Result<(captura::PlataformaWindows, captura::Abierta), ErrorMotor> {
        captura::abrir_fuente(pedido)
    }

    fn salida_aac(
        &self,
        ruta: &Path,
        muestreo: u32,
        canales: u16,
    ) -> Result<m4a::Escritor, ErrorMotor> {
        m4a::Escritor::abrir(ruta, muestreo, canales)
    }

    fn prioridad(&self, tarea: Tarea) -> com::Mmcss {
        match tarea {
            Tarea::Captura => com::Mmcss::pro_audio(),
            Tarea::Mezcla => com::Mmcss::audio(),
        }
    }

    fn ahora(&self) -> u64 {
        com::qpc_100ns()
    }
}
