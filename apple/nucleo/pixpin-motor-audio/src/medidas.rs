//! **Lo que se ve desde fuera mientras se graba**: los niveles para el
//! medidor de la interfaz y las cuentas para saber si algo fue mal.
//!
//! Todo atomicos sueltos: el hilo de mezcla escribe y quien quiera lee, sin
//! cerrojos. Un nivel es un `f32` guardado por sus bits en un `AtomicU32`.
//! Leer dos atomicos seguidos puede mezclar dos bloques distintos (el pico de
//! uno y el RMS del siguiente); para pintar una barra da igual.

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};

/// El nivel de un bloque, lineal de 0 a 1.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Nivel {
    pub pico: f32,
    pub rms: f32,
}

impl Nivel {
    /// El pico en dBFS (con suelo en -100).
    pub fn pico_db(&self) -> f32 {
        20.0 * self.pico.max(1e-5).log10()
    }
}

/// Los niveles de ahora: de cada fuente (despues de su ganancia, antes de
/// sumar) y de la mezcla (lo que se escribe).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Niveles {
    pub microfono: Option<Nivel>,
    pub equipo: Option<Nivel>,
    pub mezcla: Nivel,
}

/// Un nivel que se puede escribir desde un hilo y leer desde otro.
#[derive(Debug, Default)]
pub struct Medidor {
    pico: AtomicU32,
    rms: AtomicU32,
}

impl Medidor {
    pub fn publicar(&self, nivel: Nivel) {
        self.pico.store(nivel.pico.to_bits(), Ordering::Relaxed);
        self.rms.store(nivel.rms.to_bits(), Ordering::Relaxed);
    }

    pub fn leer(&self) -> Nivel {
        Nivel {
            pico: f32::from_bits(self.pico.load(Ordering::Relaxed)),
            rms: f32::from_bits(self.rms.load(Ordering::Relaxed)),
        }
    }
}

/// Las cuentas de una fuente, en marcos de su anillo salvo donde se dice.
#[derive(Debug, Default)]
pub struct Cuentas {
    /// Lo que entrego la tarjeta.
    pub capturados: AtomicU64,
    /// Silencio puesto porque no llegaba nada (el equipo callado, un
    /// aparato que se fue).
    pub silencio: AtomicU64,
    /// Silencio que se habia puesto de mas y se quito de lo que llego.
    pub recortados: AtomicU64,
    /// Paquetes con `AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY`: Windows tiro
    /// algo porque no se leyo a tiempo.
    pub discontinuidades: AtomicU64,
    /// Lo que no cupo en el anillo (el hilo de mezcla no daba abasto).
    pub perdidos: AtomicU64,
    /// Lo que el nucleo tiro porque sobraba (un atasco lleno el anillo).
    pub tirados: AtomicU64,
    /// Lo que falto al mezclar y salio como silencio (marcos de entrada).
    pub faltaron: AtomicU64,
    /// Veces que se volvio a abrir el aparato (desenchufado, cambio del
    /// aparato por defecto).
    pub reaperturas: AtomicU64,
    /// Saltos del reloj (el equipo se suspendio o el hilo estuvo congelado
    /// mas de 2 s): ni se rellenaron ni se cuentan como grabado.
    pub saltos: AtomicU64,
    /// Si el microfono lleva la cancelacion de eco del sistema (ver
    /// [`crate::Opciones::cancelar_eco`]): 0 no, 1 la categoria de
    /// comunicaciones, 2 ademas con la referencia de los altavoces puesta.
    pub eco: AtomicU32,
    /// Si el sonido del equipo se mantiene despierto con un flujo de
    /// silencio (ver `sistema/captura.rs`, `Latido`).
    pub latido: AtomicBool,
    /// La correccion de deriva, en milesimas de ppm.
    pub milippm: AtomicI64,
    /// Marcos por segundo del anillo.
    pub tasa: AtomicU32,
    /// Marcos por segundo del aparato abierto ahora (puede no ser el del
    /// anillo despues de un cambio de aparato).
    pub tasa_aparato: AtomicU32,
}

impl Cuentas {
    pub fn sumar(c: &AtomicU64, n: u64) {
        if n > 0 {
            c.fetch_add(n, Ordering::Relaxed);
        }
    }

    pub fn foto(&self) -> EstadisticasFuente {
        let l = |a: &AtomicU64| a.load(Ordering::Relaxed);
        EstadisticasFuente {
            capturados: l(&self.capturados),
            silencio: l(&self.silencio),
            recortados: l(&self.recortados),
            discontinuidades: l(&self.discontinuidades),
            perdidos: l(&self.perdidos),
            tirados: l(&self.tirados),
            faltaron: l(&self.faltaron),
            reaperturas: l(&self.reaperturas),
            saltos: l(&self.saltos),
            eco: self.eco.load(Ordering::Relaxed),
            latido: self.latido.load(Ordering::Relaxed),
            ppm: self.milippm.load(Ordering::Relaxed) as f64 / 1000.0,
            tasa: self.tasa.load(Ordering::Relaxed),
            tasa_aparato: self.tasa_aparato.load(Ordering::Relaxed),
        }
    }
}

/// Lo que paso con una fuente (ver [`Cuentas`] para cada campo).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EstadisticasFuente {
    pub capturados: u64,
    pub silencio: u64,
    pub recortados: u64,
    pub discontinuidades: u64,
    pub perdidos: u64,
    pub tirados: u64,
    pub faltaron: u64,
    pub reaperturas: u64,
    pub saltos: u64,
    pub eco: u32,
    pub latido: bool,
    pub ppm: f64,
    pub tasa: u32,
    pub tasa_aparato: u32,
}

/// Todas las cuentas del motor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Estadisticas {
    /// Marcos de salida producidos (sin contar las pausas).
    pub marcos: u64,
    pub muestreo: u32,
    pub canales: u16,
    pub microfono: Option<EstadisticasFuente>,
    pub equipo: Option<EstadisticasFuente>,
    /// Si el sonido del equipo es solo el de un proceso (y no todo).
    pub equipo_por_proceso: bool,
    /// Bloques que no llegaron a algun suscriptor del PCM porque iba tarde.
    pub pcm_perdidos: u64,
}

impl Estadisticas {
    /// Lo grabado, en milisegundos (hacia abajo).
    pub fn duracion_ms(&self) -> i64 {
        if self.muestreo == 0 {
            return 0;
        }
        (self.marcos as i64 * 1000) / i64::from(self.muestreo)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_nivel_sobrevive_al_viaje_por_los_bits() {
        let m = Medidor::default();
        assert_eq!(m.leer(), Nivel::default());
        let n = Nivel {
            pico: 0.5,
            rms: 0.125,
        };
        m.publicar(n);
        assert_eq!(m.leer(), n);
        assert!((n.pico_db() + 6.02).abs() < 0.01);
        assert_eq!(Nivel::default().pico_db(), -100.0);
    }

    #[test]
    fn la_duracion_sale_de_los_marcos() {
        let e = Estadisticas {
            marcos: 72_000,
            muestreo: 48_000,
            ..Default::default()
        };
        assert_eq!(e.duracion_ms(), 1_500);
        assert_eq!(Estadisticas::default().duracion_ms(), 0);
    }
}
