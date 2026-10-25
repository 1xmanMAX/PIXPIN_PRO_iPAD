//! **El motor en marcha**: los hilos, quien es dueno de que, y como se para.
//!
//! ```text
//! pixpin-microfono ──anillo──┐
//!                            ├─> pixpin-mezcla ──PCM──> pixpin-m4a (.m4a)
//! pixpin-equipo ─────anillo──┘          │        └────> suscriptores (video, onda)
//!                                       └─ niveles, estadisticas (atomicos)
//! ```
//!
//! - Un hilo de **captura** por fuente ([`crate::sistema::captura`]), con
//!   prioridad «Pro Audio». Solo vacia la tarjeta y escribe en su anillo:
//!   nunca espera a nadie.
//! - Un hilo de **mezcla**, con prioridad «Audio», que cada 10 ms mira el
//!   reloj y saca exactamente lo que toca ([`crate::nucleo`]), y lo reparte.
//! - Un hilo para el **`.m4a`**, si se pidio, que es un suscriptor mas.
//!
//! Parar va en orden: primero las capturas (que vacian lo ultimo de la
//! tarjeta), luego la mezcla (que saca lo que quedo en los anillos, para no
//! perder la ultima palabra) y, al cerrarse las colas, el `.m4a`, que hace su
//! `Finalize`.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::ErrorMotor;
use crate::medidas::{Estadisticas, Medidor, Niveles};
use crate::nucleo::{Compartido, Entrada, Nucleo, Papel, Tratamiento};
use crate::opciones::Opciones;
use crate::pcm::{Receptor, Reparto, Suscripcion, suscripcion};
use crate::reloj::marcos_en;
use crate::sistema::captura::{self, Origen};
use crate::sistema::com::Mmcss;
use crate::sistema::m4a;

/// Cada cuanto despierta el hilo de mezcla. Con el temporizador por defecto
/// de Windows (15,6 ms) dormira algo mas; da igual, porque lo que saca lo
/// decide el reloj y no las vueltas.
const PERIODO: Duration = Duration::from_millis(10);

/// Lo que queda de una grabacion.
#[derive(Debug, Clone)]
pub struct Resumen {
    /// Marcos de salida, sin las pausas.
    pub marcos: u64,
    pub duracion_ms: i64,
    pub muestreo: u32,
    pub canales: u16,
    /// El `.m4a` escrito, si se pidio.
    pub m4a: Option<PathBuf>,
    pub estadisticas: Estadisticas,
}

/// Una grabacion en marcha. Soltarla sin [`MotorAudio::terminar`] la para
/// igual (y cierra el `.m4a`), sin devolver nada.
pub struct MotorAudio {
    parar_captura: Arc<AtomicBool>,
    parar_mezcla: Arc<AtomicBool>,
    pausa: Arc<AtomicBool>,
    capturas: Vec<JoinHandle<()>>,
    mezcla: Option<JoinHandle<()>>,
    m4a: Option<JoinHandle<Result<u64, ErrorMotor>>>,
    altas: Sender<Suscripcion>,
    microfono: Option<Arc<Compartido>>,
    equipo: Option<Arc<Compartido>>,
    medidor: Arc<Medidor>,
    marcos: Arc<AtomicU64>,
    pcm_perdidos: Arc<AtomicU64>,
    muestreo: u32,
    canales: u16,
    ruta_m4a: Option<PathBuf>,
    por_proceso: bool,
}

impl MotorAudio {
    /// Abre lo pedido y empieza a grabar. Vuelve cuando todo esta abierto
    /// de verdad: que no haya microfono, falte el permiso o el codificador
    /// AAC se sabe aqui.
    pub fn empezar(opciones: Opciones) -> Result<MotorAudio, ErrorMotor> {
        opciones.validar()?;
        let Opciones {
            microfono,
            equipo,
            salida,
            realce_voz,
            paso_alto,
            puerta,
            ganancia_microfono,
            ganancia_equipo,
            pcm,
        } = opciones;
        let (muestreo, canales) = (salida.muestreo, salida.canales);
        let tratamiento = Tratamiento {
            realce_voz,
            paso_alto,
            puerta,
        };
        let parar_captura = Arc::new(AtomicBool::new(false));
        let mut capturas: Vec<JoinHandle<()>> = Vec::new();
        let detener = |capturas: Vec<JoinHandle<()>>| {
            parar_captura.store(true, Ordering::Release);
            for h in capturas {
                let _ = h.join();
            }
        };

        let mut entradas = Vec::new();
        let mut comp_microfono = None;
        let mut comp_equipo = None;
        let mut por_proceso = false;
        let fuentes = [
            microfono.map(|m| (Origen::Microfono(m), Papel::Microfono, ganancia_microfono)),
            equipo.map(|e| (Origen::Equipo(e), Papel::Equipo, ganancia_equipo)),
        ];
        for (origen, papel, ganancia) in fuentes.into_iter().flatten() {
            let comp = Compartido::nuevo(ganancia);
            match captura::lanzar(
                origen,
                canales,
                Arc::clone(&parar_captura),
                Arc::clone(&comp),
            ) {
                Ok((hilo, arranque)) => {
                    capturas.push(hilo);
                    entradas.push(Entrada::nueva(
                        papel,
                        arranque.consumidor,
                        arranque.tasa,
                        muestreo,
                        tratamiento,
                        Arc::clone(&comp),
                    ));
                    if papel == Papel::Microfono {
                        comp_microfono = Some(comp);
                    } else {
                        por_proceso = arranque.por_proceso;
                        comp_equipo = Some(comp);
                    }
                }
                Err(e) => {
                    detener(capturas);
                    return Err(e);
                }
            }
        }

        let mut abonados = Vec::with_capacity(pcm.len() + 1);
        let mut hilo_m4a = None;
        if let Some(ruta) = &salida.m4a {
            let (s, r) = suscripcion(m4a::CAPACIDAD);
            match m4a::lanzar(ruta.clone(), muestreo, canales, r) {
                Ok(h) => {
                    hilo_m4a = Some(h);
                    abonados.push(s);
                }
                Err(e) => {
                    detener(capturas);
                    return Err(e);
                }
            }
        }
        abonados.extend(pcm);

        let medidor = Arc::new(Medidor::default());
        let nucleo = Nucleo::nuevo(muestreo, canales, entradas, Arc::clone(&medidor));
        let parar_mezcla = Arc::new(AtomicBool::new(false));
        let pausa = Arc::new(AtomicBool::new(false));
        let marcos = Arc::new(AtomicU64::new(0));
        let pcm_perdidos = Arc::new(AtomicU64::new(0));
        let (altas, recibir_altas) = channel();
        let mezcla = {
            let parar = Arc::clone(&parar_mezcla);
            let pausa = Arc::clone(&pausa);
            let marcos = Arc::clone(&marcos);
            let perdidos = Arc::clone(&pcm_perdidos);
            std::thread::Builder::new()
                .name("pixpin-mezcla".into())
                .spawn(move || {
                    mezclar(
                        nucleo,
                        Reparto::nuevo(abonados),
                        &recibir_altas,
                        &parar,
                        &pausa,
                        &marcos,
                        &perdidos,
                    )
                })
        };
        let mezcla = match mezcla {
            Ok(h) => h,
            Err(_) => {
                // Las suscripciones iban dentro del cierre que no llego a
                // correr: ya estan soltadas y el `.m4a` se cierra solo.
                detener(capturas);
                if let Some(h) = hilo_m4a {
                    let _ = h.join();
                }
                return Err(ErrorMotor::HiloCaido);
            }
        };

        Ok(MotorAudio {
            parar_captura,
            parar_mezcla,
            pausa,
            capturas,
            mezcla: Some(mezcla),
            m4a: hilo_m4a,
            altas,
            microfono: comp_microfono,
            equipo: comp_equipo,
            medidor,
            marcos,
            pcm_perdidos,
            muestreo,
            canales,
            ruta_m4a: salida.m4a,
            por_proceso,
        })
    }

    /// Una suscripcion al PCM desde ahora (ver [`crate::pcm`]). Para recibir
    /// desde el primer marco, la suscripcion va en las [`Opciones`].
    pub fn suscribir_pcm(&self, capacidad: usize) -> Receptor {
        let (s, r) = suscripcion(capacidad);
        // Si la mezcla ya termino, la suscripcion se suelta y el receptor ve
        // la cola cerrada, que es justo lo que pasa.
        let _ = self.altas.send(s);
        r
    }

    /// Pausa: lo que suena se tira y el reloj de la grabacion no corre.
    pub fn pausar(&self) {
        self.pausa.store(true, Ordering::Release);
    }

    /// Sigue donde iba.
    pub fn seguir(&self) {
        self.pausa.store(false, Ordering::Release);
    }

    pub fn en_pausa(&self) -> bool {
        self.pausa.load(Ordering::Acquire)
    }

    /// La ganancia del microfono, en veces (se desliza sin chasquido).
    pub fn ganancia_microfono(&self, veces: f32) {
        if let Some(c) = &self.microfono {
            c.ganancia.poner(veces);
        }
    }

    /// La del sonido del equipo.
    pub fn ganancia_equipo(&self, veces: f32) {
        if let Some(c) = &self.equipo {
            c.ganancia.poner(veces);
        }
    }

    pub fn muestreo(&self) -> u32 {
        self.muestreo
    }

    pub fn canales(&self) -> u16 {
        self.canales
    }

    /// Marcos producidos hasta ahora (sin las pausas).
    pub fn marcos(&self) -> u64 {
        self.marcos.load(Ordering::Relaxed)
    }

    /// Lo grabado hasta ahora, en milisegundos: lo mismo que acabara
    /// diciendo la duracion, porque sale de los mismos marcos.
    pub fn llevado_ms(&self) -> i64 {
        (self.marcos() as i64 * 1000) / i64::from(self.muestreo.max(1))
    }

    /// Los niveles de ahora, para el medidor.
    pub fn niveles(&self) -> Niveles {
        Niveles {
            microfono: self.microfono.as_ref().map(|c| c.medidor.leer()),
            equipo: self.equipo.as_ref().map(|c| c.medidor.leer()),
            mezcla: self.medidor.leer(),
        }
    }

    /// Las cuentas de ahora.
    pub fn estadisticas(&self) -> Estadisticas {
        Estadisticas {
            marcos: self.marcos(),
            muestreo: self.muestreo,
            canales: self.canales,
            microfono: self.microfono.as_ref().map(|c| c.cuentas.foto()),
            equipo: self.equipo.as_ref().map(|c| c.cuentas.foto()),
            equipo_por_proceso: self.por_proceso,
            pcm_perdidos: self.pcm_perdidos.load(Ordering::Relaxed),
        }
    }

    /// Para, cierra el `.m4a` y devuelve lo que quedo.
    pub fn terminar(mut self) -> Result<Resumen, ErrorMotor> {
        let m4a = self.cerrar()?;
        let estadisticas = self.estadisticas();
        Ok(Resumen {
            marcos: estadisticas.marcos,
            duracion_ms: estadisticas.duracion_ms(),
            muestreo: self.muestreo,
            canales: self.canales,
            m4a: m4a.and(self.ruta_m4a.clone()),
            estadisticas,
        })
    }

    /// Para todos los hilos en orden. Devuelve lo que dijo el del `.m4a`.
    fn cerrar(&mut self) -> Result<Option<u64>, ErrorMotor> {
        self.parar_captura.store(true, Ordering::Release);
        let mut caido = false;
        for h in self.capturas.drain(..) {
            caido |= h.join().is_err();
        }
        self.parar_mezcla.store(true, Ordering::Release);
        if let Some(h) = self.mezcla.take() {
            caido |= h.join().is_err();
        }
        let m4a = match self.m4a.take() {
            Some(h) => Some(h.join().map_err(|_| ErrorMotor::HiloCaido)??),
            None => None,
        };
        if caido {
            return Err(ErrorMotor::HiloCaido);
        }
        Ok(m4a)
    }
}

impl Drop for MotorAudio {
    fn drop(&mut self) {
        if self.mezcla.is_some() || !self.capturas.is_empty() || self.m4a.is_some() {
            let _ = self.cerrar();
        }
    }
}

/// El cuerpo del hilo de mezcla.
fn mezclar(
    mut nucleo: Nucleo,
    mut reparto: Reparto,
    altas: &Receiver<Suscripcion>,
    parar: &AtomicBool,
    pausa: &AtomicBool,
    marcos: &AtomicU64,
    perdidos: &AtomicU64,
) {
    let _prioridad = Mmcss::audio();
    // Lo que las tarjetas entregaron mientras se abria el resto (el `.m4a`
    // tarda medio segundo la primera vez) es de antes de empezar: fuera.
    nucleo.tirar();
    let tasa = nucleo.tasa();
    let canales = nucleo.canales();
    let mut activo_100ns: u64 = 0;
    let mut producidos: u64 = 0;
    let mut antes = Instant::now();
    loop {
        // Se lee antes de mezclar: si se pidio parar, esta vuelta es la
        // ultima y saca tambien lo que quede en los anillos.
        let parando = parar.load(Ordering::Acquire);
        while let Ok(s) = altas.try_recv() {
            reparto.alta(s);
        }
        let ahora = Instant::now();
        let dt = ahora - antes;
        antes = ahora;
        let en_pausa = pausa.load(Ordering::Acquire);
        if en_pausa {
            nucleo.tirar();
        } else {
            activo_100ns += (dt.as_nanos() / 100) as u64;
            let toca = marcos_en(activo_100ns, tasa).saturating_sub(producidos);
            let extra = if parando {
                nucleo.pendiente() as u64
            } else {
                0
            };
            let mut quedan = toca + extra;
            let mut dt_s = dt.as_secs_f64();
            while quedan > 0 {
                let n = (quedan as usize).min(nucleo.max_bloque());
                let bloque = nucleo.mezclar(n, dt_s);
                reparto.repartir(producidos, tasa, canales, bloque);
                producidos += n as u64;
                quedan -= n as u64;
                dt_s = 0.0;
            }
            if extra > 0 {
                // El final sacado de mas no lo marca el reloj, pero cuenta.
                activo_100ns += crate::reloj::duracion_de(extra, tasa);
            }
            marcos.store(producidos, Ordering::Relaxed);
        }
        perdidos.store(reparto.perdidos(), Ordering::Relaxed);
        if parando {
            break;
        }
        std::thread::sleep(PERIODO);
    }
    // Al soltar el reparto se cierran las colas: el `.m4a` y los
    // suscriptores saben que se acabo.
    drop(reparto);
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::opciones::{Equipo, Microfono, Salida};

    /// Necesita **microfono y altavoces de verdad**, y que suene algo en el
    /// equipo para ver el nivel. Se ejecuta a mano en Windows con:
    /// `cargo test -p pixpin-motor-audio -- --ignored --nocapture`
    #[test]
    #[ignore = "necesita microfono y altavoces de verdad"]
    fn tres_segundos_de_microfono_y_equipo() {
        let ruta = std::env::temp_dir().join("pixpin-motor-audio-prueba.m4a");
        let _ = std::fs::remove_file(&ruta);
        let (s, r) = suscripcion(1_000);
        let motor = MotorAudio::empezar(Opciones {
            microfono: Some(Microfono::Defecto),
            equipo: Some(Equipo::Todo),
            salida: Salida {
                m4a: Some(ruta.clone()),
                canales: 2,
                muestreo: 48_000,
            },
            realce_voz: true,
            paso_alto: true,
            pcm: vec![s],
            ..Opciones::default()
        })
        .expect("no se pudo empezar");
        std::thread::sleep(Duration::from_millis(1_500));
        eprintln!("a mitad: {:?}", motor.niveles());
        motor.pausar();
        std::thread::sleep(Duration::from_millis(500));
        let en_pausa = motor.marcos();
        std::thread::sleep(Duration::from_millis(300));
        assert!(motor.marcos() - en_pausa < 1_000, "en pausa no corre");
        motor.seguir();
        std::thread::sleep(Duration::from_millis(1_500));
        let resumen = motor.terminar().expect("no se pudo terminar");
        eprintln!("{resumen:#?}");
        // Unos 3 s de los 3,8 de pared (sin la pausa), mas el final.
        assert!(
            (2_700..3_400).contains(&resumen.duracion_ms),
            "{}",
            resumen.duracion_ms
        );
        assert!(std::fs::metadata(&ruta).unwrap().len() > 10_000);
        // El PCM llego entero y sin huecos.
        let mut fin = 0;
        while let Some(b) = r.recibir() {
            assert_eq!(b.posicion, fin, "hueco en el PCM");
            assert_eq!((b.muestreo, b.canales), (48_000, 2));
            fin = b.fin();
        }
        assert_eq!(fin, resumen.marcos);
        let _ = std::fs::remove_file(&ruta);
    }

    /// Solo el sonido del equipo, sin que suene nada: la linea de tiempo
    /// tiene que avanzar igual (el silencio sale del reloj).
    #[test]
    #[ignore = "necesita altavoces de verdad (y que no suene nada)"]
    fn el_equipo_callado_avanza_igual() {
        let motor = MotorAudio::empezar(Opciones {
            equipo: Some(Equipo::Todo),
            ..Opciones::default()
        })
        .expect("no se pudo empezar");
        std::thread::sleep(Duration::from_millis(3_000));
        let r = motor.terminar().unwrap();
        eprintln!("{r:#?}");
        assert!((2_800..3_300).contains(&r.duracion_ms), "{}", r.duracion_ms);
    }

    #[test]
    #[ignore = "necesita un microfono de verdad"]
    fn la_lista_de_microfonos_tiene_el_de_por_defecto() {
        let lista = crate::microfonos().unwrap();
        eprintln!("{lista:#?}");
        assert!(lista.iter().filter(|a| a.defecto).count() <= 1);
        assert!(lista.iter().all(|a| !a.id.is_empty()));
    }
}
