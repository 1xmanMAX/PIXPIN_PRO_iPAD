//! **Que se graba y como sale**: lo que se le pide al motor al empezar.

use std::path::PathBuf;

use crate::ErrorMotor;
use crate::pcm::Suscripcion;

/// Las dos tasas que admite el codificador AAC de Windows (y que espera el
/// video): 44 100 o 48 000.
pub const MUESTREOS: [u32; 2] = [44_100, 48_000];

/// Que microfono.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Microfono {
    /// El de por defecto de Windows (consola). Si el usuario cambia el de
    /// por defecto a mitad, se pasa al nuevo.
    Defecto,
    /// Uno concreto, por su identificador de Windows (ver
    /// `microfonos()`). Si se desenchufa y no vuelve a abrirse, se sigue con
    /// el de por defecto.
    Aparato(String),
}

/// Que sonido del equipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Equipo {
    /// Todo lo que suena por los altavoces o cascos por defecto.
    Todo,
    /// Solo lo que suena desde un proceso y sus hijos (el navegador de una
    /// reunion de Meet, Zoom): asi no se cuelan las notificaciones ni la
    /// musica de otra ventana. Necesita Windows 10 2004 o posterior; si no
    /// se puede, se graba todo y [`crate::Estadisticas::equipo_por_proceso`]
    /// lo dice.
    Proceso(u32),
}

/// Como sale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Salida {
    /// Si se escribe un `.m4a` (AAC-LC en MPEG-4), donde.
    pub m4a: Option<PathBuf>,
    /// 1 o 2.
    pub canales: u16,
    /// 44 100 o 48 000.
    pub muestreo: u32,
}

impl Default for Salida {
    fn default() -> Salida {
        Salida {
            m4a: None,
            canales: 1,
            muestreo: 48_000,
        }
    }
}

/// Todo lo que se le pide al motor.
#[derive(Debug)]
pub struct Opciones {
    pub microfono: Option<Microfono>,
    pub equipo: Option<Equipo>,
    pub salida: Salida,
    /// El realce de la voz (control automatico de ganancia) en el microfono.
    pub realce_voz: bool,
    /// El paso alto de 80 Hz en el microfono.
    pub paso_alto: bool,
    /// La puerta de ruido en el microfono.
    pub puerta: bool,
    /// Ganancia de cada fuente al empezar, en veces (se puede cambiar en
    /// marcha).
    pub ganancia_microfono: f32,
    pub ganancia_equipo: f32,
    /// **Quitar del microfono lo que suena por los altavoces** (el eco: en
    /// una reunion sin cascos, la otra parte se oiria dos veces, por el
    /// sonido del equipo y colada por el microfono). Se le pide al sistema:
    /// en Windows, el microfono se abre como de comunicaciones
    /// (`AudioCategory_Communications`), que activa el cancelador de eco del
    /// controlador (o el de Windows 11 24H2, Voice Clarity), y en Windows 11 22H2+
    /// se le dice que la referencia son los altavoces por defecto
    /// (`IAcousticEchoCancellationControl`); sin que Windows baje el volumen
    /// de lo demas (`SetDuckingPreference`). Cambia el sonido del microfono
    /// (el procesado de llamadas suele llevar tambien supresion de ruido), por
    /// eso va apagado salvo que se pida. Si no hay cancelador, no pasa nada:
    /// [`crate::EstadisticasFuente::eco`] lo dice.
    pub cancelar_eco: bool,
    /// Suscripciones al PCM que reciben **desde el primer marco** (ver
    /// [`crate::pcm`]).
    pub pcm: Vec<Suscripcion>,
}

impl Default for Opciones {
    fn default() -> Opciones {
        Opciones {
            microfono: None,
            equipo: None,
            salida: Salida::default(),
            realce_voz: false,
            paso_alto: false,
            puerta: false,
            ganancia_microfono: 1.0,
            ganancia_equipo: 1.0,
            cancelar_eco: false,
            pcm: Vec::new(),
        }
    }
}

impl Opciones {
    /// Una nota de voz: el microfono por defecto con su realce, en mono.
    pub fn nota_de_voz(m4a: Option<PathBuf>) -> Opciones {
        Opciones {
            microfono: Some(Microfono::Defecto),
            salida: Salida {
                m4a,
                ..Salida::default()
            },
            realce_voz: true,
            paso_alto: true,
            ..Opciones::default()
        }
    }

    /// Comprueba que lo pedido tiene sentido antes de abrir nada.
    pub fn validar(&self) -> Result<(), ErrorMotor> {
        if self.microfono.is_none() && self.equipo.is_none() {
            return Err(ErrorMotor::SinFuentes);
        }
        if !(1..=2).contains(&self.salida.canales) || !MUESTREOS.contains(&self.salida.muestreo) {
            return Err(ErrorMotor::SalidaNoValida {
                canales: self.salida.canales,
                muestreo: self.salida.muestreo,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_fuentes_o_con_una_salida_rara_no_se_empieza() {
        assert!(matches!(
            Opciones::default().validar(),
            Err(ErrorMotor::SinFuentes)
        ));
        let mut o = Opciones::nota_de_voz(None);
        assert!(o.validar().is_ok());
        o.salida.muestreo = 22_050;
        assert!(matches!(
            o.validar(),
            Err(ErrorMotor::SalidaNoValida { .. })
        ));
        o.salida.muestreo = 44_100;
        o.salida.canales = 6;
        assert!(o.validar().is_err());
    }
}
