//! **La mezcla del motor de audio para el Mac** (Windows 38204cb y 06f842c):
//! el microfono y el sonido del Mac pasan por la misma mezcla y el mismo
//! tratamiento que en Windows (`pixpin-motor-audio`), para que una nota de
//! voz o una clase suenen igual en los dos.
//!
//! Lo que es de cada sistema se queda en Swift: el microfono con
//! AVAudioEngine y el sonido del Mac con ScreenCaptureKit (el WASAPI de
//! Windows), y el `.m4a` con AVFoundation (`EscritorVoz`). Lo de en medio es
//! el motor de Windows tal cual:
//!
//! - **Por bloques, nunca por muestra**: cada fuente le da a [`MezclaDeAudio`]
//!   el bloque entero que entrega el sistema (unos 10-90 ms) con
//!   [`MezclaDeAudio::empujar`]. Se baja a los canales de la salida con el
//!   `Conversor` de Windows y entra en el anillo de esa fuente (sin cerrojos
//!   con la mezcla: cada fuente tiene el suyo).
//! - **La salida va al reloj de la pared**: cada 20 ms Swift pide
//!   [`MezclaDeAudio::mezclar`] y sale justo lo que toca desde la ultima
//!   vez. Una fuente callada (el Mac no manda nada mientras no suena) es
//!   silencio; la deriva de cada tarjeta se corrige con el remuestreador
//!   polifasico; el microfono lleva paso alto de 80 Hz y realce de la voz
//!   (lo de `pixpin-audio::Grabadora`); todo pasa por el limitador.
//! - **En pausa** lo que llega se tira y el reloj no corre.
//! - Cada bloque sale con su **posicion** (muestras desde el principio): es
//!   el reloj del video de la clase.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pixpin_motor_audio::anillo::{self, Productor};
use pixpin_motor_audio::formato::{self, Conversor, Formato, Muestra};
use pixpin_motor_audio::medidas::Medidor;
use pixpin_motor_audio::nucleo::{Compartido, Entrada, Nucleo, Papel, Tratamiento};
use pixpin_motor_audio::reloj::{duracion_de, marcos_en};
use pixpin_motor_audio::MUESTREOS;

use crate::ErrorNucleo;

/// Lo que guarda el anillo de cada fuente (`SEGUNDOS_DE_ANILLO` de
/// Windows): de sobra para un tiron de la cola de la mezcla.
pub const SEGUNDOS_DE_ANILLO: usize = 2;
/// Cada cuanto se mezcla desde Swift (el hilo de Windows va a 10 ms; en el
/// Mac un temporizador de 20 ms basta con el colchon de 50 ms del nucleo).
pub const PERIODO_MS: u32 = 20;

/// De quien es una fuente: el realce y el paso alto son solo del microfono.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PapelDeAudio {
    Microfono,
    Equipo,
}

/// Una fuente tal cual la entrega el sistema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct FuenteDeMezcla {
    pub papel: PapelDeAudio,
    /// Su muestreo (el de la tarjeta: 44 100, 48 000, 96 000...).
    pub muestreo: u32,
    /// Sus canales.
    pub canales: u16,
}

/// Lo que sale de una vuelta de la mezcla: mono `Float32` de `-1` a `1`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BloqueMezclado {
    /// Donde empieza, en muestras desde el principio de la grabacion.
    pub posicion: u64,
    pub muestras: Vec<f32>,
    /// El pico del bloque, de 0 a 1 (para el medidor).
    pub pico: f32,
}

/// Una fuente vista desde el hilo que la captura.
struct Entrante {
    productor: Productor,
    conversor: Conversor,
    tasa: u32,
    /// Lo de cada bloque, sin reservar en cada vuelta.
    crudo: Vec<u8>,
    convertido: Vec<f32>,
}

impl Entrante {
    fn formato(tasa: u32, canales: u16) -> Formato {
        let canales = canales.max(1);
        Formato {
            muestra: Muestra::F32,
            canales,
            tasa,
            alineado: canales * 4,
            mascara: formato::mascara_habitual(canales),
        }
    }

    /// Mete un bloque de `canales` canales: intercalado, o un canal detras
    /// de otro (`separadas`, como lo da AVAudioPCMBuffer). Devuelve los
    /// marcos que entraron.
    fn empujar(&mut self, muestras: &[f32], canales: u16, separadas: bool, salida: u16) -> usize {
        let canales = canales.max(1);
        if self.conversor.formato().canales != canales {
            // Otro aparato con otros canales (al mismo muestreo).
            self.conversor = Conversor::nuevo(Entrante::formato(self.tasa, canales), salida);
        }
        let c = usize::from(canales);
        let marcos = muestras.len() / c;
        if marcos == 0 {
            return 0;
        }
        self.crudo.clear();
        self.crudo.reserve(marcos * c * 4);
        if separadas && c > 1 {
            for i in 0..marcos {
                for k in 0..c {
                    self.crudo.extend_from_slice(&muestras[k * marcos + i].to_le_bytes());
                }
            }
        } else {
            for m in &muestras[..marcos * c] {
                self.crudo.extend_from_slice(&m.to_le_bytes());
            }
        }
        let fuera = marcos * self.conversor.canales_salida();
        if self.convertido.len() < fuera {
            self.convertido.resize(fuera, 0.0);
        }
        let n = self.conversor.convertir(&self.crudo, &mut self.convertido[..fuera]);
        self.productor
            .escribir(&self.convertido[..n * self.conversor.canales_salida()])
    }
}

/// El lado de la mezcla, con su reloj.
struct Reloj {
    nucleo: Nucleo,
    /// La ultima vuelta (`None` hasta la primera, que pone el cero).
    antes: Option<Instant>,
    /// Tiempo corrido sin pausas, en 100 ns.
    activo_100ns: u64,
    producidos: u64,
    cerrada: bool,
}

impl Reloj {
    /// **Una vuelta de la mezcla** tras `dt` de pared: lo que toca por el
    /// reloj (`mezclar` del hilo de Windows, al numero) y, si se esta
    /// `parando`, tambien lo que quede en los anillos.
    fn avanzar(&mut self, dt: Duration, en_pausa: bool, parando: bool) -> BloqueMezclado {
        let posicion = self.producidos;
        if en_pausa {
            self.nucleo.tirar();
            return BloqueMezclado {
                posicion,
                muestras: Vec::new(),
                pico: 0.0,
            };
        }
        let tasa = self.nucleo.tasa();
        self.activo_100ns += (dt.as_nanos() / 100) as u64;
        let toca = marcos_en(self.activo_100ns, tasa).saturating_sub(self.producidos);
        let extra = if parando {
            self.nucleo.pendiente() as u64
        } else {
            0
        };
        let mut quedan = toca + extra;
        let mut dt_s = dt.as_secs_f64();
        let canales = usize::from(self.nucleo.canales());
        let mut muestras = Vec::with_capacity(quedan as usize * canales);
        while quedan > 0 {
            let n = (quedan as usize).min(self.nucleo.max_bloque());
            let bloque = self.nucleo.mezclar(n, dt_s);
            muestras.extend(bloque.iter().map(|&m| f32::from(m) / 32_768.0));
            self.producidos += n as u64;
            quedan -= n as u64;
            dt_s = 0.0;
        }
        if extra > 0 {
            // El final sacado de mas no lo marca el reloj, pero cuenta.
            self.activo_100ns += duracion_de(extra, tasa);
        }
        let pico = muestras.iter().fold(0f32, |p, m| p.max(m.abs()));
        BloqueMezclado {
            posicion,
            muestras,
            pico,
        }
    }
}

/// **La mezcla de una grabacion**: las fuentes empujan sus bloques desde
/// sus hilos y la cola de la grabacion saca lo mezclado.
#[derive(uniffi::Object)]
pub struct MezclaDeAudio {
    entrantes: Vec<Mutex<Entrante>>,
    reloj: Mutex<Reloj>,
    pausa: AtomicBool,
    producidos: AtomicU64,
    medidor: Arc<Medidor>,
    muestreo: u32,
    canales: u16,
}

impl MezclaDeAudio {
    /// La mezcla de `fuentes` hacia mono a `muestreo`, con el tratamiento
    /// de la grabadora de Windows (`Opciones::nota_de_voz` y la de la
    /// clase): realce de la voz y paso alto en el microfono, sin puerta.
    pub fn con(fuentes: &[FuenteDeMezcla], muestreo: u32) -> Result<MezclaDeAudio, ErrorNucleo> {
        if fuentes.is_empty() {
            return Err(ErrorNucleo::Argumento {
                detalle: "no se pidio ninguna fuente (ni microfono ni sonido del equipo)".into(),
            });
        }
        if !MUESTREOS.contains(&muestreo) {
            return Err(ErrorNucleo::Argumento {
                detalle: format!("la salida tiene que ir a 44 100 o 48 000 Hz (se pidio {muestreo})"),
            });
        }
        let canales: u16 = 1;
        let tratamiento = Tratamiento {
            realce_voz: true,
            paso_alto: true,
            puerta: false,
        };
        let mut entrantes = Vec::with_capacity(fuentes.len());
        let mut entradas = Vec::with_capacity(fuentes.len());
        for f in fuentes {
            if f.muestreo == 0 {
                return Err(ErrorNucleo::Argumento {
                    detalle: "una fuente sin muestreo".into(),
                });
            }
            let (productor, consumidor) =
                anillo::anillo(f.muestreo as usize * SEGUNDOS_DE_ANILLO, usize::from(canales));
            let papel = match f.papel {
                PapelDeAudio::Microfono => Papel::Microfono,
                PapelDeAudio::Equipo => Papel::Equipo,
            };
            entradas.push(Entrada::nueva(
                papel,
                consumidor,
                f.muestreo,
                muestreo,
                tratamiento,
                Compartido::nuevo(1.0),
            ));
            entrantes.push(Mutex::new(Entrante {
                productor,
                conversor: Conversor::nuevo(Entrante::formato(f.muestreo, f.canales), canales),
                tasa: f.muestreo,
                crudo: Vec::new(),
                convertido: Vec::new(),
            }));
        }
        let medidor = Arc::new(Medidor::default());
        Ok(MezclaDeAudio {
            entrantes,
            reloj: Mutex::new(Reloj {
                nucleo: Nucleo::nuevo(muestreo, canales, entradas, Arc::clone(&medidor)),
                antes: None,
                activo_100ns: 0,
                producidos: 0,
                cerrada: false,
            }),
            pausa: AtomicBool::new(false),
            producidos: AtomicU64::new(0),
            medidor,
            muestreo,
            canales,
        })
    }

    fn reloj(&self) -> std::sync::MutexGuard<'_, Reloj> {
        self.reloj.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Una vuelta con el reloj de verdad.
    fn vuelta(&self, parando: bool) -> BloqueMezclado {
        let mut r = self.reloj();
        let ahora = Instant::now();
        if r.cerrada {
            return BloqueMezclado {
                posicion: r.producidos,
                muestras: Vec::new(),
                pico: 0.0,
            };
        }
        let Some(antes) = r.antes.replace(ahora) else {
            // La primera: lo que las tarjetas entregaron mientras se abria
            // el resto es de antes de empezar (lo mismo que Windows).
            r.nucleo.tirar();
            if parando {
                r.cerrada = true;
            }
            return BloqueMezclado {
                posicion: 0,
                muestras: Vec::new(),
                pico: 0.0,
            };
        };
        let b = r.avanzar(
            ahora.saturating_duration_since(antes),
            self.pausa.load(Ordering::Acquire),
            parando,
        );
        if parando {
            r.cerrada = true;
        }
        self.producidos.store(r.producidos, Ordering::Relaxed);
        b
    }
}

#[uniffi::export]
impl MezclaDeAudio {
    /// **La mezcla de una grabacion** de `fuentes` (una o dos) hacia mono a
    /// `muestreo` (44 100 o 48 000).
    #[uniffi::constructor]
    pub fn nueva(fuentes: Vec<FuenteDeMezcla>, muestreo: u32) -> Result<Arc<MezclaDeAudio>, ErrorNucleo> {
        MezclaDeAudio::con(&fuentes, muestreo).map(Arc::new)
    }

    /// **Un bloque de la fuente `fuente`** (su posicion en la lista), de
    /// `canales` canales: intercalado, o un canal entero detras de otro si
    /// `separadas`. Lo llama el hilo de esa fuente con cada bloque del
    /// sistema; no espera a la mezcla. Devuelve los marcos que entraron
    /// (si el anillo estaba lleno, lo que no cabe se tira).
    pub fn empujar(&self, fuente: u32, muestras: Vec<f32>, canales: u16, separadas: bool) -> u32 {
        let Some(e) = self.entrantes.get(fuente as usize) else {
            return 0;
        };
        let mut e = e.lock().unwrap_or_else(|e| e.into_inner());
        e.empujar(&muestras, canales, separadas, self.canales) as u32
    }

    /// **Lo que toca desde la ultima vez**, por el reloj. La primera vez
    /// pone el cero y no da nada: se llama justo al empezar.
    pub fn mezclar(&self) -> BloqueMezclado {
        self.vuelta(false)
    }

    /// **Al parar**: lo que toque y lo que quede en los anillos. Despues no
    /// sale nada mas.
    pub fn terminar(&self) -> BloqueMezclado {
        self.vuelta(true)
    }

    /// Pausa o sigue: en pausa lo que llega se tira y el reloj no corre.
    pub fn pausar(&self, pausa: bool) {
        self.pausa.store(pausa, Ordering::Release);
    }

    pub fn en_pausa(&self) -> bool {
        self.pausa.load(Ordering::Acquire)
    }

    /// Lo grabado, sin las pausas (el minuto de una captura).
    pub fn llevado_ms(&self) -> i64 {
        (self.producidos.load(Ordering::Relaxed) * 1000 / u64::from(self.muestreo)) as i64
    }

    /// Las muestras que ya salieron.
    pub fn muestras(&self) -> u64 {
        self.producidos.load(Ordering::Relaxed)
    }

    /// El pico de lo ultimo mezclado, de 0 a 1 (cero en pausa).
    pub fn nivel(&self) -> f32 {
        if self.en_pausa() {
            0.0
        } else {
            self.medidor.leer().pico
        }
    }

    pub fn muestreo(&self) -> u32 {
        self.muestreo
    }
}

/// Cada cuanto se pide [`MezclaDeAudio::mezclar`], en milisegundos.
#[uniffi::export]
pub fn periodo_de_la_mezcla_ms() -> u32 {
    PERIODO_MS
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const FS: u32 = 48_000;

    fn mic(muestreo: u32, canales: u16) -> FuenteDeMezcla {
        FuenteDeMezcla {
            papel: PapelDeAudio::Microfono,
            muestreo,
            canales,
        }
    }

    fn equipo(muestreo: u32, canales: u16) -> FuenteDeMezcla {
        FuenteDeMezcla {
            papel: PapelDeAudio::Equipo,
            muestreo,
            canales,
        }
    }

    /// `ms` milisegundos de un seno de 440 Hz a `tasa`, `canales` canales
    /// separados (como AVAudioPCMBuffer), desde la muestra `desde`.
    fn seno(tasa: u32, canales: u16, ms: u32, desde: u64, amplitud: f32) -> Vec<f32> {
        let n = (tasa * ms / 1000) as usize;
        let mut v = Vec::with_capacity(n * usize::from(canales));
        for _ in 0..canales {
            for i in 0..n {
                let t = (desde + i as u64) as f32 / tasa as f32;
                v.push(amplitud * (2.0 * std::f32::consts::PI * 440.0 * t).sin());
            }
        }
        v
    }

    /// Unas cuantas vueltas de 20 ms con el reloj de mentira, empujando lo
    /// que `cada` de en cada una. Devuelve lo que salio.
    fn grabar(
        m: &MezclaDeAudio,
        vueltas: usize,
        mut cada: impl FnMut(&MezclaDeAudio, usize),
    ) -> Vec<BloqueMezclado> {
        let mut r = m.reloj();
        r.nucleo.tirar();
        r.antes = Some(Instant::now());
        drop(r);
        let mut fuera = Vec::new();
        for i in 0..vueltas {
            cada(m, i);
            let pausa = m.en_pausa();
            let b = m.reloj().avanzar(Duration::from_millis(20), pausa, false);
            fuera.push(b);
        }
        fuera
    }

    #[test]
    fn la_salida_va_al_reloj_de_la_pared() {
        // Un microfono estereo a 44 100 que entrega a golpes de 20 ms.
        let m = MezclaDeAudio::con(&[mic(44_100, 2)], FS).unwrap();
        let mut dadas = 0u64;
        let salio = grabar(&m, 100, |m, _| {
            let v = seno(44_100, 2, 20, dadas, 0.3);
            dadas += 882;
            assert_eq!(m.empujar(0, v, 2, true), 882);
        });
        let total: usize = salio.iter().map(|b| b.muestras.len()).sum();
        // Dos segundos de pared son dos segundos de nota, a 48 000.
        assert_eq!(total, 2 * FS as usize);
        // Sin huecos: cada bloque empieza donde acabo el anterior.
        let mut fin = 0;
        for b in &salio {
            assert_eq!(b.posicion, fin);
            fin += b.muestras.len() as u64;
        }
        // Y con sonido de verdad, no silencio (ya cebado y remuestreado).
        assert!(salio.last().unwrap().pico > 0.05, "{}", salio.last().unwrap().pico);
    }

    #[test]
    fn el_sonido_del_mac_callado_no_para_al_microfono() {
        let m = MezclaDeAudio::con(&[mic(FS, 1), equipo(FS, 2)], FS).unwrap();
        let mut dadas = 0u64;
        let salio = grabar(&m, 50, |m, _| {
            m.empujar(0, seno(FS, 1, 20, dadas, 0.3), 1, true);
            dadas += 960;
        });
        let total: usize = salio.iter().map(|b| b.muestras.len()).sum();
        assert_eq!(total, FS as usize);
        assert!(salio.last().unwrap().pico > 0.05);
        // Caso negativo: sin nada de ninguna fuente sale silencio, pero sale.
        let m = MezclaDeAudio::con(&[mic(FS, 1), equipo(FS, 2)], FS).unwrap();
        let salio = grabar(&m, 10, |_, _| {});
        let total: usize = salio.iter().map(|b| b.muestras.len()).sum();
        assert_eq!(total, FS as usize / 5);
        assert!(salio.iter().all(|b| b.pico == 0.0));
    }

    #[test]
    fn dos_fuentes_altas_no_pasan_del_techo() {
        let m = MezclaDeAudio::con(&[mic(FS, 1), equipo(FS, 2)], FS).unwrap();
        let mut dadas = 0u64;
        let salio = grabar(&m, 50, |m, _| {
            m.empujar(0, seno(FS, 1, 20, dadas, 0.95), 1, true);
            m.empujar(1, seno(FS, 2, 20, dadas, 0.95), 2, true);
            dadas += 960;
        });
        for b in &salio {
            assert!(b.muestras.iter().all(|s| s.abs() <= 1.0));
        }
    }

    #[test]
    fn en_pausa_no_corre_el_reloj_y_lo_que_llega_se_tira() {
        let m = MezclaDeAudio::con(&[mic(FS, 1)], FS).unwrap();
        let mut dadas = 0u64;
        let salio = grabar(&m, 30, |m, i| {
            m.pausar((10..20).contains(&i));
            m.empujar(0, seno(FS, 1, 20, dadas, 0.3), 1, true);
            dadas += 960;
        });
        let total: usize = salio.iter().map(|b| b.muestras.len()).sum();
        // Veinte vueltas sin pausa: 400 ms.
        assert_eq!(total, 20 * 960);
        assert!(salio[10..20].iter().all(|b| b.muestras.is_empty()));
        assert_eq!(salio[20].posicion, 10 * 960);
        assert!(m.nivel() > 0.0);
        m.pausar(true);
        assert_eq!(m.nivel(), 0.0, "en pausa el medidor no se mueve");
    }

    #[test]
    fn al_terminar_sale_lo_que_quedaba_y_despues_nada() {
        let m = MezclaDeAudio::con(&[mic(FS, 1)], FS).unwrap();
        let mut dadas = 0u64;
        grabar(&m, 10, |m, _| {
            m.empujar(0, seno(FS, 1, 20, dadas, 0.3), 1, true);
            dadas += 960;
        });
        // Llega un bloque mas justo antes de parar: no se pierde.
        m.empujar(0, seno(FS, 1, 100, dadas, 0.3), 1, true);
        let resto = m.reloj().avanzar(Duration::ZERO, false, true);
        assert!(resto.muestras.len() >= 4_800, "{}", resto.muestras.len());
        m.reloj().cerrada = true;
        // Caso negativo: despues de terminar no sale nada mas.
        assert!(m.terminar().muestras.is_empty());
        assert!(m.mezclar().muestras.is_empty());
    }

    #[test]
    fn la_primera_vuelta_pone_el_cero() {
        let m = MezclaDeAudio::nueva(vec![mic(FS, 1)], FS).unwrap();
        m.empujar(0, seno(FS, 1, 200, 0, 0.3), 1, true);
        let b = m.mezclar();
        // Lo que llego antes de empezar es de antes de empezar.
        assert_eq!((b.posicion, b.muestras.len()), (0, 0));
        assert_eq!(m.llevado_ms(), 0);
    }

    #[test]
    fn un_bloque_intercalado_y_uno_separado_suenan_igual() {
        let separado = seno(FS, 2, 20, 0, 0.5);
        let n = separado.len() / 2;
        let mut intercalado = Vec::with_capacity(separado.len());
        for i in 0..n {
            intercalado.push(separado[i]);
            intercalado.push(separado[n + i]);
        }
        let a = MezclaDeAudio::con(&[equipo(FS, 2)], FS).unwrap();
        let b = MezclaDeAudio::con(&[equipo(FS, 2)], FS).unwrap();
        assert_eq!(a.empujar(0, separado, 2, true), b.empujar(0, intercalado, 2, false));
        // Caso negativo: una fuente que no existe no entra.
        assert_eq!(a.empujar(5, vec![0.0; 10], 1, false), 0);
        // Otro aparato con otros canales: se reajusta y entra.
        assert_eq!(a.empujar(0, vec![0.1; 480], 1, false), 480);
    }

    #[test]
    fn lo_que_no_vale_se_dice_al_crear() {
        assert!(MezclaDeAudio::con(&[], FS).is_err());
        assert!(MezclaDeAudio::con(&[mic(FS, 1)], 32_000).is_err());
        assert!(MezclaDeAudio::con(&[mic(0, 1)], FS).is_err());
        assert!(MezclaDeAudio::con(&[mic(96_000, 4)], 44_100).is_ok());
    }
}
