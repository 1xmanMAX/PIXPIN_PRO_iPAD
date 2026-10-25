//! **El motor en marcha**: los hilos, quien es dueno de que, y como se para.
//!
//! ```text
//! pixpin-microfono ──anillo──┐
//!                            ├─> pixpin-mezcla ──PCM──> pixpin-m4a (.m4a)
//! pixpin-equipo ─────anillo──┘          │        └────> suscriptores (video, onda)
//!                                       └─ niveles, estadisticas (atomicos)
//! ```
//!
//! - Un hilo de **captura** por fuente, con la prioridad mas alta que de el
//!   sistema. Solo vacia la tarjeta y escribe en su anillo
//!   ([`crate::plataforma::capturar`]): nunca espera a nadie.
//! - Un hilo de **mezcla** que cada 10 ms mira el reloj y saca exactamente lo
//!   que toca ([`crate::nucleo`]), y lo reparte.
//! - Un hilo para el **`.m4a`**, si se pidio, que es un suscriptor mas.
//!
//! Nada de esto es de Windows: lo que toca el sistema lo da un
//! [`Sistema`] ([`MotorAudio::empezar`] usa el de Windows; Android daria el
//! suyo, ver `docs/motor-de-audio.md`). Asi el motor entero se prueba en
//! Linux con un sistema de mentira.
//!
//! Parar va en orden: primero las capturas (que vacian lo ultimo de la
//! tarjeta), luego la mezcla (que saca lo que quedo en los anillos, para no
//! perder la ultima palabra) y, al cerrarse las colas, el `.m4a`, que se
//! cierra (`Finalize`).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::ErrorMotor;
use crate::anillo::{Consumidor, anillo};
use crate::medidas::{Estadisticas, Medidor, Niveles};
use crate::nucleo::{Compartido, Entrada, Nucleo, Papel, Tratamiento};
use crate::opciones::Opciones;
use crate::pcm::{Receptor, Reparto, Suscripcion, suscripcion};
use crate::plataforma::{
    Aparato, Origen, Pedido, Plataforma, Sistema, Tarea, capturar, escribir_aac,
};
use crate::reloj::{
    CIEN_NS_POR_SEGUNDO, UMBRAL_HUECO, UMBRAL_HUECO_MICROFONO, avance_de_mezcla, duracion_de,
    marcos_en,
};
use crate::toma::Toma;

/// Cada cuanto despierta el hilo de mezcla. Con el temporizador por defecto
/// de Windows (15,6 ms) dormira algo mas; da igual, porque lo que saca lo
/// decide el reloj y no las vueltas.
const PERIODO: Duration = Duration::from_millis(10);

/// Segundos que caben en el anillo de cada fuente hacia la mezcla: de sobra
/// para que un atasco del hilo de mezcla no pierda nada.
pub const SEGUNDOS_DE_ANILLO: usize = 2;

/// Bloques en cola hacia el codificador AAC: unos 30-45 s.
pub const CAPACIDAD_AAC: usize = 3_000;

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

/// Lo que devuelve un hilo de captura que arranco bien.
struct Arranque {
    consumidor: Consumidor,
    tasa: u32,
    por_proceso: bool,
}

impl MotorAudio {
    /// Abre lo pedido y empieza a grabar, con WASAPI y Media Foundation.
    /// Vuelve cuando todo esta abierto de verdad: que no haya microfono,
    /// falte el permiso o el codificador AAC se sabe aqui.
    #[cfg(windows)]
    pub fn empezar(opciones: Opciones) -> Result<MotorAudio, ErrorMotor> {
        MotorAudio::empezar_en(Arc::new(crate::sistema::Windows), opciones)
    }

    /// Lo mismo, con el [`Sistema`] que se de (el de Windows, el de
    /// Android, uno de pruebas).
    pub fn empezar_en<S: Sistema>(
        sistema: Arc<S>,
        opciones: Opciones,
    ) -> Result<MotorAudio, ErrorMotor> {
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
            cancelar_eco,
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
            let pedido = Pedido {
                origen,
                canales: canales.clamp(1, 2) as usize,
                cancelar_eco,
            };
            match lanzar_captura(
                Arc::clone(&sistema),
                pedido,
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
            let (s, r) = suscripcion(CAPACIDAD_AAC);
            match lanzar_aac(Arc::clone(&sistema), ruta.clone(), muestreo, canales, r) {
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
            let sistema = Arc::clone(&sistema);
            std::thread::Builder::new()
                .name("pixpin-mezcla".into())
                .spawn(move || {
                    let _prioridad = sistema.prioridad(Tarea::Mezcla);
                    mezclar(
                        nucleo,
                        Reparto::nuevo(abonados),
                        &Hilo {
                            altas: &recibir_altas,
                            parar: &parar,
                            pausa: &pausa,
                            marcos: &marcos,
                            perdidos: &perdidos,
                        },
                        || sistema.ahora(),
                        || std::thread::sleep(PERIODO),
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

/// Arranca el hilo de captura de `pedido` y espera a que el aparato este
/// abierto de verdad: que no se pueda grabar (no hay microfono, falta el
/// permiso) se sabe aqui, no a mitad.
fn lanzar_captura<S: Sistema>(
    sistema: Arc<S>,
    pedido: Pedido,
    parar: Arc<AtomicBool>,
    compartido: Arc<Compartido>,
) -> Result<(JoinHandle<()>, Arranque), ErrorMotor> {
    let (avisar, arranque) = sync_channel(1);
    let nombre = match pedido.origen {
        Origen::Microfono(_) => "pixpin-microfono",
        Origen::Equipo(_) => "pixpin-equipo",
    };
    let hilo = std::thread::Builder::new()
        .name(nombre.into())
        .spawn(move || hilo_de_captura(&*sistema, &pedido, &parar, &compartido, avisar))
        .map_err(|_| ErrorMotor::HiloCaido)?;
    match arranque.recv() {
        Ok(Ok(a)) => Ok((hilo, a)),
        Ok(Err(e)) => {
            let _ = hilo.join();
            Err(e)
        }
        Err(_) => {
            let _ = hilo.join();
            Err(ErrorMotor::HiloCaido)
        }
    }
}

fn hilo_de_captura<S: Sistema>(
    sistema: &S,
    pedido: &Pedido,
    parar: &AtomicBool,
    compartido: &Arc<Compartido>,
    avisar: SyncSender<Result<Arranque, ErrorMotor>>,
) {
    // La primera: se suelta la ultima.
    let _prioridad = sistema.prioridad(Tarea::Captura);
    let (mut plataforma, aparato) = match sistema.captura(pedido) {
        Ok(p) => p,
        Err(e) => {
            let _ = avisar.send(Err(e));
            return;
        }
    };
    let tasa = aparato.tasa();
    let (productor, consumidor) = anillo(tasa as usize * SEGUNDOS_DE_ANILLO, pedido.canales);
    let umbral = if pedido.origen.es_microfono() {
        UMBRAL_HUECO_MICROFONO
    } else {
        UMBRAL_HUECO
    };
    let cuentas = &compartido.cuentas;
    cuentas.eco.store(aparato.eco(), Ordering::Relaxed);
    cuentas.latido.store(aparato.latido(), Ordering::Relaxed);
    let mut toma = Toma::nueva(
        productor,
        tasa,
        umbral,
        plataforma.ahora(),
        Arc::clone(compartido),
    );
    let _ = avisar.send(Ok(Arranque {
        consumidor,
        tasa,
        por_proceso: aparato.por_proceso(),
    }));
    drop(avisar);
    capturar(&mut plataforma, aparato, &mut toma, cuentas, parar);
    // `capturar` ya solto el aparato; la plataforma (y su COM) va despues.
    drop(plataforma);
}

/// Arranca el hilo que escribe `ruta` con lo que llegue por `receptor`, y
/// espera a que el fichero este abierto (que falte el codificador se sabe
/// aqui). El hilo devuelve los marcos escritos.
fn lanzar_aac<S: Sistema>(
    sistema: Arc<S>,
    ruta: PathBuf,
    muestreo: u32,
    canales: u16,
    receptor: Receptor,
) -> Result<JoinHandle<Result<u64, ErrorMotor>>, ErrorMotor> {
    let (avisar, abierto) = sync_channel::<Result<(), ErrorMotor>>(1);
    let hilo = std::thread::Builder::new()
        .name("pixpin-m4a".into())
        .spawn(move || {
            let mut salida = match sistema.salida_aac(&ruta, muestreo, canales) {
                Ok(s) => {
                    let _ = avisar.send(Ok(()));
                    s
                }
                Err(e) => {
                    let _ = avisar.send(Err(e.clone()));
                    return Err(e);
                }
            };
            drop(avisar);
            escribir_aac(&mut salida, &receptor, canales)
        })
        .map_err(|_| ErrorMotor::HiloCaido)?;
    match abierto.recv() {
        Ok(Ok(())) => Ok(hilo),
        Ok(Err(e)) => {
            let _ = hilo.join();
            Err(e)
        }
        Err(_) => {
            let _ = hilo.join();
            Err(ErrorMotor::HiloCaido)
        }
    }
}

/// Lo que el hilo de mezcla comparte con el resto.
struct Hilo<'a> {
    altas: &'a Receiver<Suscripcion>,
    parar: &'a AtomicBool,
    pausa: &'a AtomicBool,
    marcos: &'a AtomicU64,
    perdidos: &'a AtomicU64,
}

/// El cuerpo del hilo de mezcla. `ahora`: el reloj comun (100 ns); `dormir`:
/// hasta la vuelta siguiente.
fn mezclar(
    mut nucleo: Nucleo,
    mut reparto: Reparto,
    h: &Hilo,
    ahora: impl Fn() -> u64,
    dormir: impl Fn(),
) {
    // Lo que las tarjetas entregaron mientras se abria el resto (el `.m4a`
    // tarda medio segundo la primera vez) es de antes de empezar: fuera.
    nucleo.tirar();
    let tasa = nucleo.tasa();
    let canales = nucleo.canales();
    let mut activo_100ns: u64 = 0;
    let mut producidos: u64 = 0;
    let mut antes = ahora();
    loop {
        // Se lee antes de mezclar: si se pidio parar, esta vuelta es la
        // ultima y saca tambien lo que quede en los anillos.
        let parando = h.parar.load(Ordering::Acquire);
        while let Ok(s) = h.altas.try_recv() {
            reparto.alta(s);
        }
        let t = ahora();
        let dt = t.saturating_sub(antes);
        antes = t;
        let en_pausa = h.pausa.load(Ordering::Acquire);
        match avance_de_mezcla(dt) {
            _ if en_pausa => nucleo.tirar(),
            None => {
                // El equipo se suspendio (o este hilo estuvo congelado): no
                // se saca de golpe lo que estuvo dormido; es una pausa.
                tracing::info!(
                    segundos = dt / CIEN_NS_POR_SEGUNDO,
                    "salto del reloj; se sigue como tras una pausa"
                );
                nucleo.tirar();
            }
            Some(dt) => {
                activo_100ns += dt;
                let toca = marcos_en(activo_100ns, tasa).saturating_sub(producidos);
                let extra = if parando {
                    nucleo.pendiente() as u64
                } else {
                    0
                };
                let mut quedan = toca + extra;
                let mut dt_s = dt as f64 / CIEN_NS_POR_SEGUNDO as f64;
                while quedan > 0 {
                    let n = (quedan as usize).min(nucleo.max_bloque());
                    let bloque = nucleo.mezclar_en(n, dt_s, Some(t));
                    reparto.repartir(producidos, tasa, canales, bloque);
                    producidos += n as u64;
                    quedan -= n as u64;
                    dt_s = 0.0;
                }
                if extra > 0 {
                    // El final sacado de mas no lo marca el reloj, pero cuenta.
                    activo_100ns += duracion_de(extra, tasa);
                }
                h.marcos.store(producidos, Ordering::Relaxed);
            }
        }
        h.perdidos.store(reparto.perdidos(), Ordering::Relaxed);
        if parando {
            break;
        }
        dormir();
    }
    // Al soltar el reparto se cierran las colas: el `.m4a` y los
    // suscriptores saben que se acabo.
    drop(reparto);
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::Mutex;
    use std::time::Instant;

    use crate::opciones::{Equipo, Microfono, Salida};
    use crate::plataforma::Perdido;

    /// **Un sistema de mentira** con relojes de verdad: el microfono entrega
    /// un tono cada 10 ms de pared (a 44,1 kHz, con el cristal 300 ppm
    /// rapido); el equipo, nada (callado, como el loopback sin sonido); y el
    /// AAC guarda lo escrito. Puede hacer que el microfono se vaya.
    struct DeMentira {
        cero: Instant,
        se_va_a_ms: Option<u64>,
        escrito: Arc<Mutex<Vec<i16>>>,
        cerrado: Arc<AtomicU64>,
        sin_microfono: bool,
    }

    impl DeMentira {
        fn nuevo() -> DeMentira {
            DeMentira {
                cero: Instant::now(),
                se_va_a_ms: None,
                escrito: Arc::default(),
                cerrado: Arc::default(),
                sin_microfono: false,
            }
        }

        fn reloj(&self) -> u64 {
            (self.cero.elapsed().as_nanos() / 100) as u64
        }
    }

    struct Fuente {
        cero: Instant,
        tasa: u32,
        tono: bool,
        entregado: u64,
        abierto_en: u64,
        se_va_a: Option<u64>,
        canales: usize,
    }

    impl Fuente {
        fn ahora(&self) -> u64 {
            (self.cero.elapsed().as_nanos() / 100) as u64
        }
    }

    impl Aparato for Fuente {
        fn tasa(&self) -> u32 {
            self.tasa
        }
        fn max_marcos(&self) -> usize {
            self.tasa as usize / 50
        }
        fn esperar(&mut self, ms: u32) {
            std::thread::sleep(Duration::from_millis(u64::from(ms)));
        }
        fn vaciar(&mut self, toma: &mut Toma) -> Result<(), Perdido> {
            let ahora = self.ahora();
            if self.se_va_a.is_some_and(|t| ahora >= t) {
                return Err(Perdido {
                    motivo: "desenchufado".into(),
                    esperado: true,
                });
            }
            if !self.tono {
                return Ok(());
            }
            // Cada 10 ms de pared, 10 ms de *su* reloj (300 ppm rapido).
            let real = f64::from(self.tasa) * 1.000_3;
            let deberia = ((ahora - self.abierto_en) as f64 / 1e7 * real) as u64;
            let paquete = self.tasa as u64 / 100;
            let mut v = vec![0.0f32; paquete as usize * self.canales];
            while self.entregado + paquete <= deberia {
                for (k, m) in v.chunks_exact_mut(self.canales).enumerate() {
                    let n = self.entregado + k as u64;
                    let x = (n as f32 * 2.0 * std::f32::consts::PI * 440.0 / self.tasa as f32)
                        .sin()
                        * 0.3;
                    m.fill(x);
                }
                let fecha = self.abierto_en + (self.entregado as f64 / real * 1e7) as u64;
                toma.paquete(&v, Some(fecha), ahora, false);
                self.entregado += paquete;
            }
            Ok(())
        }
    }

    struct Plat {
        cero: Instant,
        tono: bool,
        canales: usize,
    }

    impl Plataforma for Plat {
        type Aparato = Fuente;
        fn ahora(&self) -> u64 {
            (self.cero.elapsed().as_nanos() / 100) as u64
        }
        fn dormir(&mut self, ms: u32) {
            std::thread::sleep(Duration::from_millis(u64::from(ms)));
        }
        fn reabrir(&mut self, _: u32) -> Result<Fuente, ErrorMotor> {
            Ok(Fuente {
                cero: self.cero,
                tasa: 48_000,
                tono: self.tono,
                entregado: 0,
                abierto_en: self.ahora(),
                se_va_a: None,
                canales: self.canales,
            })
        }
    }

    struct Fichero {
        escrito: Arc<Mutex<Vec<i16>>>,
        cerrado: Arc<AtomicU64>,
    }

    impl crate::plataforma::SalidaAac for Fichero {
        fn escribir(&mut self, m: &[i16]) -> Result<(), ErrorMotor> {
            self.escrito.lock().unwrap().extend_from_slice(m);
            Ok(())
        }
        fn cerrar(&mut self) -> Result<(), ErrorMotor> {
            self.cerrado.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    impl Sistema for DeMentira {
        type Plataforma = Plat;
        type Salida = Fichero;
        type Prioridad = ();

        fn captura(&self, pedido: &Pedido) -> Result<(Plat, Fuente), ErrorMotor> {
            let microfono = pedido.origen.es_microfono();
            if microfono && self.sin_microfono {
                return Err(ErrorMotor::SinMicrofono);
            }
            let p = Plat {
                cero: self.cero,
                tono: microfono,
                canales: pedido.canales,
            };
            let ahora = p.ahora();
            let f = Fuente {
                cero: self.cero,
                tasa: if microfono { 44_100 } else { 48_000 },
                tono: microfono,
                entregado: 0,
                abierto_en: ahora,
                se_va_a: if microfono {
                    self.se_va_a_ms.map(|ms| ahora + ms * 10_000)
                } else {
                    None
                },
                canales: pedido.canales,
            };
            Ok((p, f))
        }

        fn salida_aac(&self, _: &std::path::Path, _: u32, _: u16) -> Result<Fichero, ErrorMotor> {
            Ok(Fichero {
                escrito: Arc::clone(&self.escrito),
                cerrado: Arc::clone(&self.cerrado),
            })
        }

        fn prioridad(&self, _: Tarea) {}

        fn ahora(&self) -> u64 {
            self.reloj()
        }
    }

    fn opciones(pcm: Vec<Suscripcion>) -> Opciones {
        Opciones {
            microfono: Some(Microfono::Defecto),
            equipo: Some(Equipo::Todo),
            salida: Salida {
                m4a: Some(PathBuf::from("mentira.m4a")),
                canales: 2,
                muestreo: 48_000,
            },
            realce_voz: true,
            paso_alto: true,
            pcm,
            ..Opciones::default()
        }
    }

    /// El motor entero, con sus hilos, en Linux: microfono a 44,1 kHz y el
    /// equipo callado, con una pausa. La duracion es la de pared sin la
    /// pausa, el PCM llega sin huecos y el AAC tiene lo mismo, cerrado una
    /// vez.
    #[test]
    fn el_motor_entero_graba_pausa_y_cierra() {
        let sistema = Arc::new(DeMentira::nuevo());
        let (s, r) = suscripcion(1_000);
        let motor = MotorAudio::empezar_en(Arc::clone(&sistema), opciones(vec![s])).unwrap();
        std::thread::sleep(Duration::from_millis(600));
        motor.pausar();
        std::thread::sleep(Duration::from_millis(300));
        let en_pausa = motor.marcos();
        std::thread::sleep(Duration::from_millis(200));
        assert!(motor.marcos() - en_pausa < 1_000, "en pausa no corre");
        motor.seguir();
        std::thread::sleep(Duration::from_millis(600));
        let n = motor.niveles();
        assert!(n.microfono.unwrap().pico > 0.1, "{n:?}");
        assert_eq!(n.equipo.unwrap().pico, 0.0);
        let resumen = motor.terminar().unwrap();
        // 1,2 s de los 1,7 de pared (mas o menos lo que tarden los hilos).
        assert!(
            (1_100..1_500).contains(&resumen.duracion_ms),
            "{}",
            resumen.duracion_ms
        );
        let mut fin = 0;
        while let Some(b) = r.recibir() {
            assert_eq!(b.posicion, fin, "hueco en el PCM");
            fin = b.fin();
        }
        assert_eq!(fin, resumen.marcos);
        assert_eq!(
            sistema.escrito.lock().unwrap().len() as u64,
            resumen.marcos * 2
        );
        assert_eq!(sistema.cerrado.load(Ordering::Relaxed), 1);
        let e = resumen.estadisticas;
        let m = e.microfono.unwrap();
        assert_eq!((m.tasa, m.reaperturas, m.saltos), (44_100, 0, 0));
        let q = e.equipo.unwrap();
        assert!(q.silencio > 0, "el equipo callado se relleno: {q:?}");
        assert_eq!(e.pcm_perdidos, 0);
    }

    /// El microfono se va a los 300 ms y vuelve en el acto (a 48 kHz): una
    /// reapertura, y la grabacion sigue igual de larga.
    #[test]
    fn el_motor_sigue_aunque_el_microfono_se_vaya() {
        let mut s = DeMentira::nuevo();
        s.se_va_a_ms = Some(300);
        let sistema = Arc::new(s);
        let motor = MotorAudio::empezar_en(Arc::clone(&sistema), opciones(Vec::new())).unwrap();
        std::thread::sleep(Duration::from_millis(1_000));
        let resumen = motor.terminar().unwrap();
        let m = resumen.estadisticas.microfono.unwrap();
        assert_eq!(m.reaperturas, 1, "{m:?}");
        assert_eq!(m.tasa_aparato, 48_000);
        assert!(
            (850..1_200).contains(&resumen.duracion_ms),
            "{}",
            resumen.duracion_ms
        );
    }

    #[test]
    fn sin_microfono_no_se_empieza_y_no_queda_nada_abierto() {
        let mut s = DeMentira::nuevo();
        s.sin_microfono = true;
        let sistema = Arc::new(s);
        let r = MotorAudio::empezar_en(Arc::clone(&sistema), opciones(Vec::new()));
        assert!(matches!(r, Err(ErrorMotor::SinMicrofono)));
        assert_eq!(sistema.cerrado.load(Ordering::Relaxed), 0);
    }

    /// La mezcla con un reloj que salta una hora (el portatil se cerro):
    /// no saca una hora de sonido de golpe.
    #[test]
    fn un_salto_del_reloj_en_la_mezcla_es_una_pausa() {
        let nucleo = Nucleo::nuevo(48_000, 1, Vec::new(), Arc::new(Medidor::default()));
        let (s, r) = suscripcion(10_000);
        let (_altas, recibir) = channel();
        let parar = AtomicBool::new(false);
        let pausa = AtomicBool::new(false);
        let marcos = AtomicU64::new(0);
        let perdidos = AtomicU64::new(0);
        let t = std::cell::Cell::new(0u64);
        let vueltas = std::cell::Cell::new(0u32);
        mezclar(
            nucleo,
            Reparto::nuevo(vec![s]),
            &Hilo {
                altas: &recibir,
                parar: &parar,
                pausa: &pausa,
                marcos: &marcos,
                perdidos: &perdidos,
            },
            || t.get(),
            || {
                vueltas.set(vueltas.get() + 1);
                // Diez vueltas de 10 ms, una hora de golpe, otras diez.
                let paso = if vueltas.get() == 10 {
                    3_600 * CIEN_NS_POR_SEGUNDO
                } else {
                    100_000
                };
                t.set(t.get() + paso);
                if vueltas.get() == 20 {
                    parar.store(true, Ordering::Release);
                }
            },
        );
        // 19 vueltas de 10 ms: 9 120 marcos, no 172 millones.
        assert_eq!(marcos.load(Ordering::Relaxed), 9_120);
        let mut total = 0;
        while let Some(b) = r.recibir() {
            total += b.marcos();
        }
        assert_eq!(total, 9_120);
    }

    /// **Como seria en Android**: la plataforma empuja el sonido desde un
    /// hilo suyo (la llamada de datos de AAudio, o `AudioRecord.read` por
    /// JNI) con su fecha de `CLOCK_MONOTONIC`, en bloques de 20 ms de 16
    /// bits a 48 kHz mono, y el motor hace lo mismo que en Windows.
    struct ComoAndroid {
        cero: Instant,
        parar: Arc<AtomicBool>,
        hilos: Mutex<Vec<JoinHandle<()>>>,
    }

    struct PlatAndroid {
        cero: Instant,
    }

    impl Plataforma for PlatAndroid {
        type Aparato = crate::empuje::Empujado;
        fn ahora(&self) -> u64 {
            (self.cero.elapsed().as_nanos() / 100) as u64
        }
        fn dormir(&mut self, ms: u32) {
            std::thread::sleep(Duration::from_millis(u64::from(ms)));
        }
        fn reabrir(&mut self, _: u32) -> Result<crate::empuje::Empujado, ErrorMotor> {
            Err(ErrorMotor::SinMicrofono)
        }
    }

    impl Sistema for ComoAndroid {
        type Plataforma = PlatAndroid;
        type Salida = Fichero;
        type Prioridad = ();

        fn captura(
            &self,
            pedido: &Pedido,
        ) -> Result<(PlatAndroid, crate::empuje::Empujado), ErrorMotor> {
            let (mut empujador, empujado) =
                crate::empuje::empuje(48_000, 1, pedido.canales, 2, "microfono");
            let (cero, parar) = (self.cero, Arc::clone(&self.parar));
            let tono = pedido.origen.es_microfono();
            self.hilos.lock().unwrap().push(std::thread::spawn(move || {
                // Una onda cuadrada de 500 Hz (el paso alto se comeria una
                // continua).
                let amplitud = if tono { 8_000 } else { 0 };
                let bloque: Vec<i16> = (0..960)
                    .map(|i| if i % 96 < 48 { amplitud } else { -amplitud })
                    .collect();
                let empezo = cero.elapsed();
                let mut fin = empezo;
                while !parar.load(Ordering::Acquire) {
                    fin += Duration::from_millis(20);
                    if let Some(falta) = fin.checked_sub(cero.elapsed()) {
                        std::thread::sleep(falta);
                    }
                    empujador.empujar_i16(&bloque, (fin.as_nanos() / 100) as u64);
                }
            }));
            Ok((PlatAndroid { cero: self.cero }, empujado))
        }

        fn salida_aac(&self, _: &std::path::Path, _: u32, _: u16) -> Result<Fichero, ErrorMotor> {
            Ok(Fichero {
                escrito: Arc::default(),
                cerrado: Arc::default(),
            })
        }

        fn prioridad(&self, _: Tarea) {}

        fn ahora(&self) -> u64 {
            (self.cero.elapsed().as_nanos() / 100) as u64
        }
    }

    #[test]
    fn un_sistema_que_empuja_como_android_graba_igual() {
        let sistema = Arc::new(ComoAndroid {
            cero: Instant::now(),
            parar: Arc::default(),
            hilos: Mutex::default(),
        });
        let (s, r) = suscripcion(1_000);
        let motor = MotorAudio::empezar_en(Arc::clone(&sistema), opciones(vec![s])).unwrap();
        std::thread::sleep(Duration::from_millis(1_000));
        let n = motor.niveles();
        let resumen = motor.terminar().unwrap();
        sistema.parar.store(true, Ordering::Release);
        for h in sistema.hilos.lock().unwrap().drain(..) {
            h.join().unwrap();
        }
        assert!(n.microfono.unwrap().pico > 0.2, "{n:?}");
        assert!(
            (850..1_200).contains(&resumen.duracion_ms),
            "{}",
            resumen.duracion_ms
        );
        let m = resumen.estadisticas.microfono.unwrap();
        assert!(m.capturados > 40_000, "{m:?}");
        // Sin quedarse corto en marcha; al parar, lo que el equipo tenia de
        // mas que el microfono sale con el microfono en silencio (menos de
        // un paquete).
        assert!(m.faltaron < 1_000, "{m:?}");
        let mut fin = 0;
        while let Some(b) = r.recibir() {
            assert_eq!(b.posicion, fin);
            fin = b.fin();
        }
        assert_eq!(fin, resumen.marcos);
    }
}

#[cfg(all(test, windows))]
mod en_windows {
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

    /// Con la cancelacion de eco pedida: abre igual, dice que nivel consiguio
    /// y el sonido del equipo lleva su latido.
    #[test]
    #[ignore = "necesita microfono y altavoces de verdad"]
    fn con_cancelar_eco_se_abre_y_dice_lo_que_hay() {
        let motor = MotorAudio::empezar(Opciones {
            microfono: Some(Microfono::Defecto),
            equipo: Some(Equipo::Todo),
            cancelar_eco: true,
            ..Opciones::default()
        })
        .expect("no se pudo empezar");
        std::thread::sleep(Duration::from_millis(1_500));
        let r = motor.terminar().unwrap();
        eprintln!("{r:#?}");
        let m = r.estadisticas.microfono.unwrap();
        assert!(m.eco >= 1, "{m:?}");
        assert!(r.estadisticas.equipo.unwrap().latido);
    }

    /// El sonido de este mismo proceso (que no suena): la captura por proceso
    /// se abre (Windows 10 2004+) o cae a todo el equipo, y la linea de
    /// tiempo avanza igual.
    #[test]
    #[ignore = "necesita altavoces de verdad"]
    fn el_sonido_de_un_proceso_avanza_aunque_calle() {
        let motor = MotorAudio::empezar(Opciones {
            equipo: Some(Equipo::Proceso(std::process::id())),
            ..Opciones::default()
        })
        .expect("no se pudo empezar");
        std::thread::sleep(Duration::from_millis(2_000));
        let r = motor.terminar().unwrap();
        eprintln!("{r:#?}");
        assert!((1_800..2_300).contains(&r.duracion_ms), "{}", r.duracion_ms);
    }
}
