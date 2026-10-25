//! **Lo que el motor le pide a cada sistema**: capturar una fuente y escribir
//! AAC. Todo lo demas (la toma, el relleno, la deriva, la mezcla, el
//! tratamiento, el reparto del PCM, el hueco del `.m4a`) es de aqui y es lo
//! mismo en Windows y en Android.
//!
//! ```text
//!                 Sistema (uno por motor)
//!                 ├── captura(pedido) ──> Plataforma + Aparato   (en el hilo de cada fuente)
//!                 ├── salida_aac(...) ──> SalidaAac              (en el hilo del .m4a)
//!                 ├── prioridad(tarea)                           (MMCSS, o lo que haya)
//!                 └── ahora()                                    (QPC / CLOCK_MONOTONIC, 100 ns)
//! ```
//!
//! - Windows (`sistema/`): WASAPI, `IMMNotificationClient`, la captura por
//!   proceso y `IMFSinkWriter`.
//! - Android (ver `docs/motor-de-audio.md`, «Android»): AAudio u Oboe para
//!   el microfono, `AudioPlaybackCapture` para el sonido del equipo y
//!   `MediaCodec` + `MediaMuxer` para el AAC.
//! - Las pruebas: un sistema de mentira con relojes y aparatos simulados,
//!   para probar en Linux las reaperturas y el motor entero.
//!
//! ## Volver a abrir ([`capturar`])
//!
//! Lo de OBS (`WASAPISource::CaptureThread` y `ReconnectThread` en
//! `plugins/win-wasapi/win-wasapi.cpp`), con lo que alli falta:
//!
//! - Si el aparato se pierde (`AUDCLNT_E_DEVICE_INVALIDATED`, o cualquier
//!   error de la tarjeta), se cierra y se intenta abrir **en el acto** (lo
//!   normal es que ya haya otro), y luego cada vez mas espaciado: 250 ms,
//!   500 ms, 1 s, 2 s y desde ahi cada 3 s (el `RECONNECT_INTERVAL` de OBS).
//!   Solo el primer fallo se avisa en el registro (`previouslyFailed`).
//! - Si Windows dice que cambio el aparato por defecto, se vuelve a abrir
//!   **solo si es otro** (OBS compara el id en `SetDefaultDevice`: Windows
//!   repite el aviso, por ejemplo uno por papel).
//! - Lo que OBS no hace: mientras no hay aparato, **la linea de tiempo
//!   sigue** con silencio al paso del reloj ([`Toma::al_paso`]); la
//!   grabacion no se corta y lo de despues cae en su sitio.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::ErrorMotor;
use crate::medidas::Cuentas;
use crate::opciones::{Equipo, Microfono};
use crate::pcm::Receptor;
use crate::toma::Toma;

/// Lo mas que se espera al aviso de una tarjeta antes de mirar el reloj.
pub const ESPERA_MS: u32 = 10;

/// De donde lee un hilo de captura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origen {
    Microfono(Microfono),
    Equipo(Equipo),
}

impl Origen {
    /// El error de que esta fuente no exista.
    pub fn falta(&self) -> ErrorMotor {
        match self {
            Origen::Microfono(_) => ErrorMotor::SinMicrofono,
            Origen::Equipo(_) => ErrorMotor::SinSonidoDelEquipo,
        }
    }

    pub fn es_microfono(&self) -> bool {
        matches!(self, Origen::Microfono(_))
    }
}

/// Lo que se le pide a la plataforma para una fuente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pedido {
    pub origen: Origen,
    /// Los canales de la salida (1 o 2): cada aparato entrega ya en estos.
    pub canales: usize,
    /// Pedirle al sistema que quite del microfono lo que suena por los
    /// altavoces (ver [`crate::Opciones::cancelar_eco`]).
    pub cancelar_eco: bool,
}

/// Que hilo pide prioridad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tarea {
    /// Un hilo de captura: lo mas alto que haya («Pro Audio»).
    Captura,
    /// La mezcla («Audio»).
    Mezcla,
}

/// El aparato se perdio: hay que cerrarlo y volver a abrir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Perdido {
    /// Que paso, para el registro.
    pub motivo: String,
    /// Si es lo esperado (desenchufado) y no merece un aviso.
    pub esperado: bool,
}

/// **Un aparato abierto y en marcha.**
pub trait Aparato {
    /// Marcos por segundo que entrega.
    fn tasa(&self) -> u32;
    /// Lo mas grande que puede traer un paquete, en marcos.
    fn max_marcos(&self) -> usize;
    /// El aparato fisico (para no volver a abrir el mismo por un aviso
    /// repetido). Vacio si no se sabe o no aplica.
    fn id(&self) -> &str {
        ""
    }
    /// Si captura el sonido de un solo proceso (Windows).
    fn por_proceso(&self) -> bool {
        false
    }
    /// Si lleva la cancelacion de eco del sistema (ver [`Cuentas::eco`]).
    fn eco(&self) -> u32 {
        0
    }
    /// Si se mantiene despierto con un flujo de silencio (ver
    /// [`Cuentas::latido`]).
    fn latido(&self) -> bool {
        false
    }
    /// Espera a que tenga algo, como mucho `ms`.
    fn esperar(&mut self, ms: u32);
    /// Entrega a `toma` todo lo que tenga. `Err` si se perdio.
    fn vaciar(&mut self, toma: &mut Toma) -> Result<(), Perdido>;
}

/// **La captura de una fuente en una plataforma.** Vive en el hilo de esa
/// fuente (en Windows lleva el COM de ese hilo).
pub trait Plataforma {
    type Aparato: Aparato;
    /// La hora del reloj comun, en 100 ns.
    fn ahora(&self) -> u64;
    /// Duerme `ms` (mientras no hay aparato).
    fn dormir(&mut self, ms: u32);
    /// Vuelve a abrir el aparato, para un anillo a `tasa_anillo`.
    fn reabrir(&mut self, tasa_anillo: u32) -> Result<Self::Aparato, ErrorMotor>;
    /// Cuantos avisos de «cambio el aparato por defecto» van (0 si no
    /// vigila ninguno).
    fn avisos(&self) -> u64 {
        0
    }
    /// El id del aparato por defecto de ahora, para compararlo con el
    /// abierto. `None` si no hay ninguno.
    fn id_por_defecto(&mut self) -> Option<String> {
        None
    }
}

/// **Donde se escribe el AAC.** Vive en el hilo del `.m4a`.
pub trait SalidaAac {
    /// Un bloque intercalado de 16 bits, a continuacion de lo anterior.
    fn escribir(&mut self, muestras: &[i16]) -> Result<(), ErrorMotor>;
    /// Cierra el fichero (el indice del MPEG-4). Se llama siempre, una vez,
    /// aunque algo fallara antes.
    fn cerrar(&mut self) -> Result<(), ErrorMotor>;
}

/// **Un sistema entero**: lo que el motor necesita para arrancar sus hilos
/// (ver [`crate::MotorAudio::empezar_en`]). Se comparte entre los hilos;
/// lo que no se puede compartir (COM, los objetos de cada aparato) lo crea
/// cada hilo con estas funciones.
pub trait Sistema: Send + Sync + 'static {
    type Plataforma: Plataforma;
    type Salida: SalidaAac;
    /// Lo que mantiene la prioridad de un hilo mientras vive.
    type Prioridad;

    /// Abre la fuente de `pedido`. Se llama **en el hilo de captura**. Que
    /// no se pueda (no hay microfono, falta el permiso) se dice aqui.
    fn captura(
        &self,
        pedido: &Pedido,
    ) -> Result<(Self::Plataforma, <Self::Plataforma as Plataforma>::Aparato), ErrorMotor>;
    /// Abre el AAC. Se llama en el hilo del `.m4a`.
    fn salida_aac(
        &self,
        ruta: &std::path::Path,
        muestreo: u32,
        canales: u16,
    ) -> Result<Self::Salida, ErrorMotor>;
    /// Sube la prioridad del hilo que llama.
    fn prioridad(&self, tarea: Tarea) -> Self::Prioridad;
    /// La hora del reloj comun, en 100 ns (el mismo de las fechas de los
    /// paquetes).
    fn ahora(&self) -> u64;
}

/// **Cuando volver a intentar abrir.** Primero en el acto, luego cada vez
/// mas espaciado hasta [`ESPERAS_100NS`] su ultimo valor.
#[derive(Debug, Clone)]
pub struct Reintentos {
    siguiente: u64,
    fallos: usize,
}

/// Lo que se espera despues de cada fallo seguido, en 100 ns.
pub const ESPERAS_100NS: [u64; 5] = [2_500_000, 5_000_000, 10_000_000, 20_000_000, 30_000_000];

impl Reintentos {
    /// Para un aparato que se acaba de perder a las `ahora`: se intenta ya.
    pub fn desde(ahora: u64) -> Reintentos {
        Reintentos {
            siguiente: ahora,
            fallos: 0,
        }
    }

    /// Si toca intentarlo a las `ahora`.
    pub fn toca(&self, ahora: u64) -> bool {
        ahora >= self.siguiente
    }

    /// Fallo el intento de las `ahora`. Devuelve si es el primero seguido
    /// (el unico que merece un aviso en el registro).
    pub fn fallo(&mut self, ahora: u64) -> bool {
        let espera = ESPERAS_100NS[self.fallos.min(ESPERAS_100NS.len() - 1)];
        self.siguiente = ahora + espera;
        self.fallos += 1;
        self.fallos == 1
    }

    /// Fallos seguidos.
    pub fn fallos(&self) -> usize {
        self.fallos
    }
}

/// **El bucle de un hilo de captura**, hasta `parar`: vacia el aparato,
/// pone silencio al paso del reloj y vuelve a abrir lo que se pierda (ver
/// el comentario del modulo).
pub fn capturar<P: Plataforma>(
    plataforma: &mut P,
    primero: P::Aparato,
    toma: &mut Toma,
    cuentas: &Cuentas,
    parar: &AtomicBool,
) {
    let mut abierto = Some(primero);
    let mut reintentos = Reintentos::desde(0);
    let mut visto = plataforma.avisos();
    while !parar.load(Ordering::Acquire) {
        match abierto.as_mut() {
            Some(a) => {
                a.esperar(ESPERA_MS);
                if let Err(p) = a.vaciar(toma) {
                    if p.esperado {
                        tracing::info!(motivo = %p.motivo, "el aparato de sonido se fue; se volvera a abrir");
                    } else {
                        tracing::warn!(motivo = %p.motivo, "fallo leyendo la tarjeta; se volvera a abrir");
                    }
                    abierto = None;
                    reintentos = Reintentos::desde(plataforma.ahora());
                }
            }
            None => {
                plataforma.dormir(ESPERA_MS);
                let ahora = plataforma.ahora();
                if reintentos.toca(ahora) {
                    match plataforma.reabrir(toma.tasa()) {
                        Ok(a) => {
                            if reintentos.fallos() > 0 {
                                tracing::info!(
                                    fallos = reintentos.fallos(),
                                    "el aparato de sonido volvio"
                                );
                            }
                            Cuentas::sumar(&cuentas.reaperturas, 1);
                            cuentas.eco.store(a.eco(), Ordering::Relaxed);
                            cuentas.latido.store(a.latido(), Ordering::Relaxed);
                            toma.aparato(a.tasa(), a.max_marcos());
                            abierto = Some(a);
                        }
                        Err(e) => {
                            if reintentos.fallo(ahora) {
                                tracing::warn!(?e, "aun no se puede volver a abrir el aparato");
                            } else {
                                tracing::debug!(?e, "aun no se puede volver a abrir el aparato");
                            }
                        }
                    }
                }
            }
        }
        let avisos = plataforma.avisos();
        if avisos != visto {
            visto = avisos;
            let nuevo = plataforma.id_por_defecto();
            let mismo = match (&abierto, &nuevo) {
                (Some(a), Some(n)) => !a.id().is_empty() && a.id() == n,
                _ => false,
            };
            if !mismo {
                tracing::info!("cambio el aparato de sonido por defecto; se pasa al nuevo");
                abierto = None;
                reintentos = Reintentos::desde(plataforma.ahora());
            }
        }
        toma.al_paso(plataforma.ahora());
    }
    // El aparato se suelta aqui (antes que la plataforma y su COM).
    drop(abierto);
}

/// Lo mas que se escribe de silencio de una vez al tapar un hueco.
const TROZO_DE_SILENCIO: u64 = 48_000;

/// **Escribe en `salida` todo lo que llegue por `receptor`** hasta que se
/// cierre la cola, y la cierra. Devuelve los marcos escritos.
///
/// Un hueco (la cola se lleno y se tiro algo) se escribe como silencio, para
/// que lo de despues caiga en su sitio y la duracion sea la buena. Si
/// escribir falla (el disco se lleno), se deja de escribir pero se sigue
/// vaciando la cola (para que la mezcla no cuente perdidas que no son) y
/// **se cierra igual**: lo escrito hasta ahi queda legible.
pub fn escribir_aac<S: SalidaAac>(
    salida: &mut S,
    receptor: &Receptor,
    canales: u16,
) -> Result<u64, ErrorMotor> {
    let canales = u64::from(canales.max(1));
    let mut escritos = 0u64;
    let mut ceros: Vec<i16> = Vec::new();
    let mut fallo = None;
    while let Some(bloque) = receptor.recibir() {
        if fallo.is_some() {
            receptor.devolver(bloque);
            continue;
        }
        while bloque.posicion > escritos && fallo.is_none() {
            let n = (bloque.posicion - escritos).min(TROZO_DE_SILENCIO);
            ceros.clear();
            ceros.resize((n * canales) as usize, 0);
            match salida.escribir(&ceros) {
                Ok(()) => escritos += n,
                Err(e) => fallo = Some(e),
            }
        }
        if fallo.is_none() {
            match salida.escribir(&bloque.muestras) {
                Ok(()) => escritos += bloque.marcos() as u64,
                Err(e) => fallo = Some(e),
            }
        }
        receptor.devolver(bloque);
    }
    let cerrado = salida.cerrar();
    if let Some(e) = fallo {
        return Err(e);
    }
    cerrado?;
    Ok(escritos)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Arc;

    use crate::anillo::anillo;
    use crate::nucleo::Compartido;
    use crate::pcm::{Reparto, suscripcion};
    use crate::reloj::{UMBRAL_HUECO, marcos_en};

    const FS: u32 = 48_000;
    const MS: u64 = 10_000;

    /// El reloj simulado y cuando se para el bucle.
    #[derive(Clone)]
    struct Reloj {
        t: Rc<Cell<u64>>,
        fin: u64,
        parar: Arc<AtomicBool>,
    }

    impl Reloj {
        fn nuevo(fin_ms: u64) -> Reloj {
            Reloj {
                t: Rc::new(Cell::new(0)),
                fin: fin_ms * MS,
                parar: Arc::new(AtomicBool::new(false)),
            }
        }

        fn pasar(&self, ms: u32) {
            self.t.set(self.t.get() + u64::from(ms) * MS);
            if self.t.get() >= self.fin {
                self.parar.store(true, Ordering::Release);
            }
        }
    }

    /// Un aparato simulado que entrega 10 ms por cada 10 ms del reloj, hasta
    /// que se va.
    struct Simulado {
        reloj: Reloj,
        entregado: u64,
        tasa: u32,
        id: String,
        se_va_a: Option<u64>,
    }

    impl Aparato for Simulado {
        fn tasa(&self) -> u32 {
            self.tasa
        }
        fn max_marcos(&self) -> usize {
            self.tasa as usize / 100
        }
        fn id(&self) -> &str {
            &self.id
        }
        fn esperar(&mut self, ms: u32) {
            self.reloj.pasar(ms);
        }
        fn vaciar(&mut self, toma: &mut Toma) -> Result<(), Perdido> {
            let ahora = self.reloj.t.get();
            if self.se_va_a.is_some_and(|t| ahora >= t) {
                return Err(Perdido {
                    motivo: "desenchufado".into(),
                    esperado: true,
                });
            }
            let v = vec![0.5f32; self.max_marcos()];
            while self.entregado + 10 * MS <= ahora {
                toma.paquete(&v, Some(self.entregado), ahora, false);
                self.entregado += 10 * MS;
            }
            Ok(())
        }
    }

    /// Una plataforma simulada: reabrir falla `fallos` veces y luego da un
    /// aparato a `tasa`; los avisos de cambio llegan cuando dice `avisos`
    /// (a que hora, y que aparato queda por defecto).
    struct Mentira {
        reloj: Reloj,
        intentos: Vec<u64>,
        fallos: usize,
        tasa: u32,
        defecto: String,
        avisos: Vec<(u64, String)>,
    }

    impl Mentira {
        fn vistos(&self) -> impl Iterator<Item = &(u64, String)> {
            let t = self.reloj.t.get();
            self.avisos.iter().filter(move |(en, _)| *en <= t)
        }
    }

    impl Plataforma for Mentira {
        type Aparato = Simulado;
        fn ahora(&self) -> u64 {
            self.reloj.t.get()
        }
        fn dormir(&mut self, ms: u32) {
            self.reloj.pasar(ms);
        }
        fn reabrir(&mut self, _: u32) -> Result<Simulado, ErrorMotor> {
            self.intentos.push(self.reloj.t.get());
            if self.fallos > 0 {
                self.fallos -= 1;
                return Err(ErrorMotor::SinMicrofono);
            }
            Ok(Simulado {
                reloj: self.reloj.clone(),
                entregado: self.reloj.t.get(),
                tasa: self.tasa,
                id: self.id_por_defecto().unwrap_or_default(),
                se_va_a: None,
            })
        }
        fn avisos(&self) -> u64 {
            self.vistos().count() as u64
        }
        fn id_por_defecto(&mut self) -> Option<String> {
            Some(
                self.vistos()
                    .last()
                    .map_or_else(|| self.defecto.clone(), |(_, id)| id.clone()),
            )
        }
    }

    #[test]
    fn los_reintentos_se_espacian_hasta_tres_segundos() {
        let mut r = Reintentos::desde(0);
        assert!(r.toca(0));
        assert!(r.fallo(0), "el primero se avisa");
        assert!(!r.toca(2_499_999));
        assert!(r.toca(2_500_000));
        assert!(!r.fallo(2_500_000), "los siguientes no");
        let mut t = 2_500_000;
        let mut esperas = Vec::new();
        while !r.toca(t) {
            t += MS;
        }
        for _ in 0..6 {
            r.fallo(t);
            let antes = t;
            while !r.toca(t) {
                t += MS;
            }
            esperas.push((t - antes) / MS);
        }
        assert_eq!(esperas, [1_000, 2_000, 3_000, 3_000, 3_000, 3_000]);
    }

    /// Un microfono que se desenchufa al segundo, tarda en volver (tres
    /// intentos fallidos) y vuelve a otra tasa: la grabacion sigue, la linea
    /// de tiempo no se rompe y se cuenta una reapertura.
    #[test]
    fn un_aparato_que_se_va_vuelve_sin_romper_la_linea_de_tiempo() {
        let r = Reloj::nuevo(5_000);
        let (p, mut c) = anillo(FS as usize * 20, 1);
        let comp = Compartido::nuevo(1.0);
        let mut toma = Toma::nueva(p, FS, UMBRAL_HUECO, 0, Arc::clone(&comp));
        let mut plataforma = Mentira {
            reloj: r.clone(),
            intentos: Vec::new(),
            fallos: 3,
            tasa: 44_100,
            defecto: "nuevo".into(),
            avisos: Vec::new(),
        };
        let primero = Simulado {
            reloj: r.clone(),
            entregado: 0,
            tasa: FS,
            id: "viejo".into(),
            se_va_a: Some(1_000 * MS),
        };
        capturar(&mut plataforma, primero, &mut toma, &comp.cuentas, &r.parar);
        let e = comp.cuentas.foto();
        assert_eq!(e.reaperturas, 1);
        assert_eq!(e.tasa_aparato, 44_100);
        // Se intento en el acto y luego a los 250 ms, 500 ms y 1 s.
        let i: Vec<u64> = plataforma.intentos.iter().map(|t| t / MS).collect();
        assert_eq!(i.len(), 4, "{i:?}");
        assert!(i[0] - 1_000 <= 20, "{i:?}");
        assert!((i[1] - i[0]).abs_diff(250) <= 20, "{i:?}");
        assert!((i[2] - i[1]).abs_diff(500) <= 20, "{i:?}");
        assert!((i[3] - i[2]).abs_diff(1_000) <= 20, "{i:?}");
        // Cinco segundos de pared son cinco segundos de anillo (menos lo que
        // aun puede estar de camino y lo que se queda en el filtro).
        let hay = c.disponibles() as i64;
        assert!((hay - 240_000).abs() < 3_000, "{hay}");
        assert!(e.silencio > marcos_en(1_500 * MS, FS), "{e:?}");
        let mut v = vec![0.0; hay as usize];
        c.leer(&mut v);
        // Lo del final es sonido del aparato nuevo, en su sitio.
        let cola = &v[hay as usize - 1_000..];
        assert!(cola.iter().all(|x| (x - 0.5).abs() < 0.01));
    }

    /// Un aviso repetido del mismo aparato por defecto no lo vuelve a abrir;
    /// uno de otro aparato, si.
    #[test]
    fn un_aviso_repetido_no_reabre_y_uno_de_verdad_si() {
        let r = Reloj::nuevo(3_000);
        let (p, _c) = anillo(FS as usize * 20, 1);
        let comp = Compartido::nuevo(1.0);
        let mut toma = Toma::nueva(p, FS, UMBRAL_HUECO, 0, Arc::clone(&comp));
        let mut plataforma = Mentira {
            reloj: r.clone(),
            intentos: Vec::new(),
            fallos: 0,
            tasa: FS,
            defecto: "el-mismo".into(),
            avisos: vec![
                (1_000 * MS, "el-mismo".into()),
                (1_500 * MS, "el-mismo".into()),
                (2_000 * MS, "otro".into()),
            ],
        };
        let primero = Simulado {
            reloj: r.clone(),
            entregado: 0,
            tasa: FS,
            id: "el-mismo".into(),
            se_va_a: None,
        };
        capturar(&mut plataforma, primero, &mut toma, &comp.cuentas, &r.parar);
        assert_eq!(plataforma.intentos.len(), 1, "{:?}", plataforma.intentos);
        assert!(plataforma.intentos[0] >= 2_000 * MS);
        assert_eq!(comp.cuentas.foto().reaperturas, 1);
    }

    /// Una salida que guarda lo escrito y puede fallar a partir de un punto.
    #[derive(Default)]
    struct Fichero {
        muestras: Vec<i16>,
        cerrado: u32,
        falla_desde: Option<usize>,
    }

    impl SalidaAac for Fichero {
        fn escribir(&mut self, m: &[i16]) -> Result<(), ErrorMotor> {
            if self.falla_desde.is_some_and(|n| self.muestras.len() >= n) {
                return Err(ErrorMotor::Fichero {
                    ruta: "lleno".into(),
                });
            }
            self.muestras.extend_from_slice(m);
            Ok(())
        }
        fn cerrar(&mut self) -> Result<(), ErrorMotor> {
            self.cerrado += 1;
            Ok(())
        }
    }

    #[test]
    fn un_hueco_en_la_cola_del_aac_se_escribe_como_silencio() {
        let (s, r) = suscripcion(16);
        let mut rep = Reparto::nuevo(vec![s]);
        rep.repartir(0, FS, 2, &[1; 960]);
        // Se perdieron 100 000 marcos (mas de un trozo de silencio).
        rep.repartir(100_480, FS, 2, &[2; 960]);
        drop(rep);
        let mut f = Fichero::default();
        let n = escribir_aac(&mut f, &r, 2).unwrap();
        assert_eq!(n, 100_960);
        assert_eq!(f.muestras.len(), 100_960 * 2);
        assert!(f.muestras[960..960 + 200_000].iter().all(|v| *v == 0));
        assert_eq!(f.muestras[f.muestras.len() - 1], 2);
        assert_eq!(f.cerrado, 1);
    }

    #[test]
    fn si_escribir_falla_se_cierra_igual_y_se_dice() {
        let (s, r) = suscripcion(16);
        let mut rep = Reparto::nuevo(vec![s]);
        for k in 0..10 {
            rep.repartir(k * 480, FS, 2, &[3; 960]);
        }
        drop(rep);
        let mut f = Fichero {
            falla_desde: Some(960 * 3),
            ..Fichero::default()
        };
        assert!(escribir_aac(&mut f, &r, 2).is_err());
        assert_eq!(f.cerrado, 1, "el indice se escribe aunque falle");
        assert_eq!(f.muestras.len(), 960 * 3);
    }
}
