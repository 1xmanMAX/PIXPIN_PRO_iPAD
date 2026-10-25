//! **Un aparato al que le empujan el sonido**: para las plataformas que
//! entregan la captura en una llamada suya (la de datos de AAudio u Oboe,
//! el bucle de `AudioRecord.read` de Android en Kotlin, que llega por JNI)
//! en vez de dejarse leer como WASAPI.
//!
//! ```text
//! hilo de la plataforma (AAudio, JNI)        hilo de captura del motor
//!   Empujador::empujar_i16(bloque, fin) ──anillo──> Empujado (un Aparato)
//!                                                    └─> capturar() ─> Toma ─> mezcla
//! ```
//!
//! - El que empuja **nunca espera**: escribe en un anillo sin cerrojos (el
//!   de [`crate::anillo`]); si no cabe, se tira y se cuenta. Es lo que pide
//!   AAudio de su llamada de datos (no bloquear, no reservar memoria).
//! - Cada bloque va con **su fecha de fin** en el reloj comun (en Android,
//!   `CLOCK_MONOTONIC` de `AAudioStream_getTimestamp` o
//!   `AudioRecord.getTimestamp`, pasado a 100 ns): con ella la toma coloca
//!   cada paquete en su sitio como en Windows.
//! - Si el flujo se cierra (Android desconecto el aparato: unos cascos que se
//!   quitan, `AAUDIO_ERROR_DISCONNECTED`), se suelta el [`Empujador`]: el
//!   [`Empujado`] lo ve, entrega lo que quedaba y dice que se perdio, y
//!   [`crate::plataforma::capturar`] vuelve a abrir como con cualquier otro.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::Thread;
use std::time::Duration;

use crate::anillo::{Consumidor, Productor, anillo};
use crate::plataforma::{Aparato, Perdido};
use crate::reloj::duracion_de;
use crate::toma::Toma;

/// Lo que se comparten los dos lados.
#[derive(Debug, Default)]
struct Entre {
    /// El hilo que espera (el de captura), para despertarlo.
    hilo: OnceLock<Thread>,
    /// El que empuja sigue ahi.
    vivo: AtomicBool,
    /// Marcos que no cupieron.
    perdidos: AtomicU64,
}

/// El lado de la plataforma.
pub struct Empujador {
    productor: Productor,
    entre: Arc<Entre>,
    entrada: usize,
    /// Para convertir sin reservar memoria en la llamada de datos.
    conversion: Vec<f32>,
}

/// El lado del motor: un [`Aparato`].
pub struct Empujado {
    consumidor: Consumidor,
    entre: Arc<Entre>,
    tasa: u32,
    id: String,
    leido: Vec<f32>,
}

/// Un aparato empujado a `tasa`, que recibe bloques de `entrada` canales
/// y los da en `salida` (1 o 2), con sitio para `segundos` de colchon.
/// `id`: el del aparato fisico, si se sabe (ver [`Aparato::id`]). Cada
/// bloque tiene que llevar su fecha de fin (no cero).
pub fn empuje(
    tasa: u32,
    entrada: usize,
    salida: usize,
    segundos: usize,
    id: impl Into<String>,
) -> (Empujador, Empujado) {
    let tasa = tasa.max(1);
    let salida = salida.clamp(1, 2);
    let (productor, consumidor) = anillo(tasa as usize * segundos.max(1), salida);
    let entre = Arc::new(Entre {
        vivo: AtomicBool::new(true),
        ..Entre::default()
    });
    let max = tasa as usize / 10;
    (
        Empujador {
            productor,
            entre: Arc::clone(&entre),
            entrada: entrada.max(1),
            conversion: vec![0.0; max * salida],
        },
        Empujado {
            consumidor,
            entre,
            tasa,
            id: id.into(),
            leido: vec![0.0; max * salida],
        },
    )
}

impl Empujador {
    /// Marcos que no cupieron (el motor iba tarde).
    pub fn perdidos(&self) -> u64 {
        self.entre.perdidos.load(Ordering::Relaxed)
    }

    /// Un bloque de muestras de 16 bits intercaladas (lo que da
    /// `AudioRecord` con `ENCODING_PCM_16BIT`), que acaba a las `fin` (100
    /// ns del reloj comun). Devuelve los marcos que entraron.
    pub fn empujar_i16(&mut self, muestras: &[i16], fin: u64) -> usize {
        self.empujar_con(muestras, fin, |v| f32::from(v) / 32_768.0)
    }

    /// Lo mismo en coma flotante (`ENCODING_PCM_FLOAT`, o
    /// `AAUDIO_FORMAT_PCM_FLOAT`).
    pub fn empujar_f32(&mut self, muestras: &[f32], fin: u64) -> usize {
        self.empujar_con(muestras, fin, |v| if v.is_finite() { v } else { 0.0 })
    }

    fn empujar_con<T: Copy>(&mut self, muestras: &[T], fin: u64, leer: impl Fn(T) -> f32) -> usize {
        let e = self.entrada;
        let s = self.productor.canales();
        let marcos = muestras.len() / e;
        let mut hechos = 0;
        let mut escritos = 0;
        // A trozos del tamano del buffer de conversion: nada se reserva.
        let por_trozo = (self.conversion.len() / s).max(1);
        while hechos < marcos {
            let n = (marcos - hechos).min(por_trozo);
            let trozo = &muestras[hechos * e..(hechos + n) * e];
            let fuera = &mut self.conversion[..n * s];
            for (m, f) in trozo.chunks_exact(e).zip(fuera.chunks_exact_mut(s)) {
                match (e, s) {
                    (1, _) => f.fill(leer(m[0])),
                    (_, 2) => {
                        f[0] = leer(m[0]);
                        f[1] = leer(m[1]);
                    }
                    // A mono: la media de todos, como en `formato`.
                    _ => f[0] = m.iter().map(|v| leer(*v)).sum::<f32>() / e as f32,
                }
            }
            escritos += self.productor.escribir(fuera);
            hechos += n;
        }
        if escritos < marcos {
            self.entre
                .perdidos
                .fetch_add((marcos - escritos) as u64, Ordering::Relaxed);
        }
        self.productor.sellar(fin);
        if let Some(h) = self.entre.hilo.get() {
            h.unpark();
        }
        escritos
    }
}

impl Drop for Empujador {
    fn drop(&mut self) {
        self.entre.vivo.store(false, Ordering::Release);
        if let Some(h) = self.entre.hilo.get() {
            h.unpark();
        }
    }
}

impl Aparato for Empujado {
    fn tasa(&self) -> u32 {
        self.tasa
    }

    fn max_marcos(&self) -> usize {
        self.leido.len() / self.consumidor.canales()
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn esperar(&mut self, ms: u32) {
        let _ = self.entre.hilo.set(std::thread::current());
        if self.consumidor.disponibles() == 0 && self.entre.vivo.load(Ordering::Acquire) {
            std::thread::park_timeout(Duration::from_millis(u64::from(ms)));
        }
    }

    fn vaciar(&mut self, toma: &mut Toma) -> Result<(), Perdido> {
        let c = self.consumidor.canales();
        // El sello antes que la cuenta: lo que se lea puede ser algo mas
        // nuevo que el sello, nunca mas viejo.
        let vivo = self.entre.vivo.load(Ordering::Acquire);
        let fin = self.consumidor.sello();
        let mut hay = self.consumidor.disponibles();
        let ahora_hasta = fin;
        let mut desde = fin.saturating_sub(duracion_de(hay as u64, self.tasa));
        while hay > 0 {
            let n = hay.min(self.leido.len() / c);
            let leidos = self.consumidor.leer(&mut self.leido[..n * c]);
            if leidos == 0 {
                break;
            }
            let fecha = (desde != 0).then_some(desde);
            toma.paquete(
                &self.leido[..leidos * c],
                fecha,
                ahora_hasta.max(desde),
                false,
            );
            desde += duracion_de(leidos as u64, self.tasa);
            hay -= leidos;
        }
        if vivo {
            Ok(())
        } else {
            Err(Perdido {
                motivo: "la plataforma cerro el flujo".into(),
                esperado: true,
            })
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::nucleo::Compartido;
    use crate::reloj::UMBRAL_HUECO_MICROFONO;

    const FS: u32 = 48_000;
    const MS: u64 = 10_000;

    fn toma() -> (Toma, crate::anillo::Consumidor, Arc<Compartido>) {
        let (p, c) = anillo(FS as usize * 2, 1);
        let comp = Compartido::nuevo(1.0);
        (
            Toma::nueva(p, FS, UMBRAL_HUECO_MICROFONO, 0, Arc::clone(&comp)),
            c,
            comp,
        )
    }

    #[test]
    fn lo_empujado_llega_a_la_toma_con_su_fecha() {
        let (mut e, mut a) = empuje(FS, 2, 1, 1, "cascos");
        let (mut t, c, comp) = toma();
        // Diez bloques de 10 ms en estereo de 16 bits, uno con un canal
        // callado: a mono es la media.
        for k in 1..=10u64 {
            let bloque: Vec<i16> = (0..480).flat_map(|_| [16_384i16, 0]).collect();
            assert_eq!(e.empujar_i16(&bloque, k * 10 * MS), 480);
        }
        a.vaciar(&mut t).unwrap();
        assert_eq!(c.disponibles(), 4_800);
        assert_eq!(c.sello(), 100 * MS);
        assert_eq!(comp.cuentas.foto().silencio, 0);
        // Un hueco de 200 ms (la plataforma no entrego) se rellena.
        e.empujar_f32(&[0.1; 960], 310 * MS);
        a.vaciar(&mut t).unwrap();
        assert_eq!(comp.cuentas.foto().silencio, 9_600);
        assert_eq!(a.id(), "cascos");
    }

    #[test]
    fn si_no_cabe_se_tira_y_se_cuenta_sin_esperar() {
        let (mut e, _a) = empuje(FS, 1, 1, 1, "");
        assert_eq!(e.empujar_f32(&vec![0.0; 60_000], 1), 48_000);
        assert_eq!(e.perdidos(), 12_000);
    }

    #[test]
    fn cuando_la_plataforma_cierra_se_entrega_lo_que_quedaba_y_se_pierde() {
        let (mut e, mut a) = empuje(FS, 1, 2, 1, "");
        let (p, c) = anillo(FS as usize * 2, 2);
        let comp = Compartido::nuevo(1.0);
        let mut t = Toma::nueva(p, FS, UMBRAL_HUECO_MICROFONO, 0, comp);
        e.empujar_i16(&[100; 480], 10 * MS);
        drop(e);
        let perdido = a.vaciar(&mut t).unwrap_err();
        assert!(perdido.esperado);
        assert_eq!(c.disponibles(), 480, "mono a los dos lados, entero");
    }

    #[test]
    fn el_que_espera_se_despierta_al_empujar() {
        let (mut e, mut a) = empuje(FS, 1, 1, 1, "");
        // Se registra el hilo que espera.
        a.esperar(1);
        let empuja = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            e.empujar_f32(&[0.0; 480], 1);
            e
        });
        let antes = std::time::Instant::now();
        a.esperar(5_000);
        assert!(antes.elapsed() < Duration::from_millis(2_000));
        assert_eq!(a.consumidor.disponibles(), 480);
        drop(empuja.join().unwrap());
    }
}
