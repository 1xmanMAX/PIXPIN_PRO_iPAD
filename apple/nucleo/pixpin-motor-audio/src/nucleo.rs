//! **El nucleo de la mezcla**: de los anillos de cada fuente a un bloque de
//! salida, sin Windows.
//!
//! El hilo de mezcla se despierta cada pocos milisegundos, mira el reloj del
//! sistema y le pide al nucleo **exactamente** los marcos que tocan: la
//! salida va al paso de la pared, no al de ninguna tarjeta. Para cada fuente
//! el nucleo:
//!
//! 1. **Ceba**: al empezar (o despues de quedarse sin nada) espera a que su
//!    anillo tenga [`OBJETIVO_MS`] antes de leer; mientras, esa fuente es
//!    silencio. Ese colchon es lo que absorbe que las tarjetas entreguen a
//!    golpes de 10 ms y el hilo de mezcla despierte cuando puede.
//! 2. **Tira** lo que sobra si un atasco le dejo el anillo por encima de
//!    [`OBJETIVO_MS`] + [`SOBRA_MS`]: mejor perder un trozo que ir medio
//!    segundo tarde el resto de la grabacion.
//! 3. **Corrige la deriva** ([`crate::deriva`]) con lo que hay en el anillo,
//!    y remuestrea a la salida con ese paso.
//! 4. Le pasa su cadena (paso alto, puerta, realce, ganancia) y mide su
//!    nivel.
//!
//! Luego suma, limita y pasa a 16 bits. Nada de esto reserva memoria: los
//! bloques se reservan al crear el nucleo para el mas grande posible
//! ([`MAX_BLOQUE_MS`]).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::anillo::Consumidor;
use crate::deriva::Deriva;
use crate::dsp::{self, Ganancia, Limitador, PasoAlto, Puerta, Realce};
use crate::medidas::{Cuentas, Medidor, Nivel};
use crate::remuestreo::Remuestreador;

/// El colchon de cada anillo.
pub const OBJETIVO_MS: u32 = 50;
/// Lo que se le tolera por encima antes de tirar.
pub const SOBRA_MS: u32 = 200;
/// El bloque mas grande que se pide de una vez.
pub const MAX_BLOQUE_MS: u32 = 100;
/// La frecuencia de corte del paso alto del microfono.
pub const CORTE_PASO_ALTO_HZ: f32 = 80.0;

/// De quien es una fuente: el realce, la puerta y el paso alto son solo del
/// microfono.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Papel {
    Microfono,
    Equipo,
}

/// El tratamiento que se pide para el microfono.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tratamiento {
    pub realce_voz: bool,
    pub paso_alto: bool,
    pub puerta: bool,
}

/// Una ganancia que se cambia desde otro hilo (un `f32` en sus bits).
#[derive(Debug)]
pub struct GananciaCompartida(AtomicU32);

impl GananciaCompartida {
    pub fn nueva(v: f32) -> GananciaCompartida {
        GananciaCompartida(AtomicU32::new(v.to_bits()))
    }
    pub fn poner(&self, v: f32) {
        let v = if v.is_finite() {
            v.clamp(0.0, 16.0)
        } else {
            1.0
        };
        self.0.store(v.to_bits(), Ordering::Relaxed);
    }
    pub fn leer(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

/// Lo que se comparte de una fuente con el resto del motor.
#[derive(Debug)]
pub struct Compartido {
    pub ganancia: GananciaCompartida,
    pub medidor: Medidor,
    pub cuentas: Cuentas,
}

impl Compartido {
    pub fn nuevo(ganancia: f32) -> Arc<Compartido> {
        Arc::new(Compartido {
            ganancia: GananciaCompartida::nueva(ganancia),
            medidor: Medidor::default(),
            cuentas: Cuentas::default(),
        })
    }
}

/// Una fuente vista desde la mezcla.
pub struct Entrada {
    papel: Papel,
    anillo: Consumidor,
    tasa: u32,
    canales: usize,
    remuestreador: Remuestreador,
    deriva: Deriva,
    cebada: bool,
    objetivo: usize,
    sobra: usize,
    paso_alto: Option<PasoAlto>,
    puerta: Option<Puerta>,
    realce: Option<Realce>,
    ganancia: Ganancia,
    compartido: Arc<Compartido>,
    bloque: Vec<f32>,
}

impl Entrada {
    /// `anillo` a `tasa` marcos por segundo, en los canales de la salida.
    pub fn nueva(
        papel: Papel,
        anillo: Consumidor,
        tasa: u32,
        tasa_salida: u32,
        tratamiento: Tratamiento,
        compartido: Arc<Compartido>,
    ) -> Entrada {
        let canales = anillo.canales();
        let max = max_bloque(tasa_salida);
        let objetivo = (tasa as usize * OBJETIVO_MS as usize) / 1000;
        let microfono = papel == Papel::Microfono;
        compartido.cuentas.tasa.store(tasa, Ordering::Relaxed);
        Entrada {
            papel,
            tasa,
            canales,
            remuestreador: Remuestreador::nuevo(tasa, tasa_salida, canales, max),
            deriva: Deriva::nueva(objetivo, tasa),
            cebada: false,
            objetivo,
            sobra: (tasa as usize * SOBRA_MS as usize) / 1000,
            paso_alto: (microfono && tratamiento.paso_alto)
                .then(|| PasoAlto::nuevo(CORTE_PASO_ALTO_HZ, tasa_salida)),
            puerta: (microfono && tratamiento.puerta).then(|| Puerta::nueva(tasa_salida)),
            realce: (microfono && tratamiento.realce_voz).then(|| Realce::nuevo(tasa_salida)),
            ganancia: Ganancia::nueva(compartido.ganancia.leer(), tasa_salida),
            compartido,
            anillo,
            bloque: vec![0.0; max * canales],
        }
    }

    pub fn papel(&self) -> Papel {
        self.papel
    }

    /// Lo que espera en el anillo, ya en marcos de salida.
    fn pendiente(&self, tasa_salida: u32) -> usize {
        (self.anillo.disponibles() as u64 * u64::from(tasa_salida) / u64::from(self.tasa.max(1)))
            as usize
    }

    /// Los `marcos` siguientes de esta fuente, ya tratados.
    fn producir(&mut self, marcos: usize, dt: f64) -> &[f32] {
        let c = self.canales;
        let fuera = &mut self.bloque[..marcos * c];
        let cuentas = &self.compartido.cuentas;
        let mut hay = self.anillo.disponibles();
        if !self.cebada {
            if hay < self.objetivo {
                fuera.fill(0.0);
                self.compartido.medidor.publicar(Nivel::default());
                return fuera;
            }
            self.cebada = true;
            self.deriva.olvidar_medida();
        }
        if hay > self.objetivo + self.sobra {
            let tirados = self.anillo.descartar(hay - self.objetivo);
            Cuentas::sumar(&cuentas.tirados, tirados as u64);
            hay -= tirados;
            self.deriva.olvidar_medida();
        }
        if dt > 0.0 {
            let factor = self.deriva.observar(hay as f64, dt);
            self.remuestreador.ajustar(factor);
            cuentas
                .milippm
                .store((self.deriva.ppm() * 1000.0) as i64, Ordering::Relaxed);
        }
        let anillo = &mut self.anillo;
        let faltaron = self.remuestreador.producir(fuera, |d| anillo.leer(d));
        if faltaron > 0 {
            // Se quedo sin nada: lo que falto ya es silencio, y hasta que
            // vuelva a tener su colchon esta fuente calla.
            Cuentas::sumar(&cuentas.faltaron, faltaron as u64);
            self.cebada = false;
        }
        if let Some(p) = &mut self.paso_alto {
            p.procesar(fuera, c);
        }
        if let Some(p) = &mut self.puerta {
            p.procesar(fuera, c);
        }
        if let Some(r) = &mut self.realce {
            r.procesar(fuera, c);
        }
        self.ganancia
            .procesar(fuera, c, self.compartido.ganancia.leer());
        let (pico, rms) = dsp::medir(fuera);
        self.compartido.medidor.publicar(Nivel { pico, rms });
        fuera
    }

    /// En pausa: todo lo que llega se tira y al seguir se vuelve a cebar.
    fn tirar(&mut self) {
        self.anillo.descartar(usize::MAX);
        self.cebada = false;
        self.remuestreador.reiniciar();
        self.deriva.olvidar_medida();
        self.compartido.medidor.publicar(Nivel::default());
    }
}

/// Marcos del bloque mas grande a `tasa`.
pub fn max_bloque(tasa: u32) -> usize {
    (tasa as usize * MAX_BLOQUE_MS as usize) / 1000
}

/// La mezcla entera.
pub struct Nucleo {
    entradas: Vec<Entrada>,
    tasa: u32,
    canales: usize,
    suma: Vec<f32>,
    enteros: Vec<i16>,
    limitador: Limitador,
    medidor: Arc<Medidor>,
}

impl Nucleo {
    pub fn nuevo(tasa: u32, canales: u16, entradas: Vec<Entrada>, medidor: Arc<Medidor>) -> Nucleo {
        let canales = canales.clamp(1, 2) as usize;
        let max = max_bloque(tasa);
        Nucleo {
            entradas,
            tasa,
            canales,
            suma: vec![0.0; max * canales],
            enteros: vec![0; max * canales],
            limitador: Limitador::nuevo(tasa, canales),
            medidor,
        }
    }

    pub fn tasa(&self) -> u32 {
        self.tasa
    }

    pub fn canales(&self) -> u16 {
        self.canales as u16
    }

    /// El bloque mas grande que acepta [`Nucleo::mezclar`].
    pub fn max_bloque(&self) -> usize {
        max_bloque(self.tasa)
    }

    /// Lo que queda en los anillos, en marcos de salida (el que mas tiene):
    /// lo que hay que sacar al terminar para no perder el final.
    pub fn pendiente(&self) -> usize {
        self.entradas
            .iter()
            .map(|e| e.pendiente(self.tasa))
            .max()
            .unwrap_or(0)
    }

    /// Mezcla los `marcos` siguientes (como mucho [`Nucleo::max_bloque`]).
    /// `dt`: segundos de pared desde la medida anterior de la deriva (cero si
    /// es otro trozo de la misma vuelta).
    pub fn mezclar(&mut self, marcos: usize, dt: f64) -> &[i16] {
        let marcos = marcos.min(self.max_bloque());
        let n = marcos * self.canales;
        let suma = &mut self.suma[..n];
        suma.fill(0.0);
        for e in &mut self.entradas {
            let b = e.producir(marcos, dt);
            for (s, v) in suma.iter_mut().zip(b) {
                *s += v;
            }
        }
        self.limitador.procesar(suma);
        let (pico, rms) = dsp::medir(suma);
        self.medidor.publicar(Nivel { pico, rms });
        dsp::a_i16(suma, &mut self.enteros[..n]);
        &self.enteros[..n]
    }

    /// En pausa: tira lo que llega.
    pub fn tirar(&mut self) {
        for e in &mut self.entradas {
            e.tirar();
        }
        self.medidor.publicar(Nivel::default());
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::anillo::{Productor, anillo};

    const FS: u32 = 48_000;

    /// Una tarjeta simulada: entrega paquetes de 10 ms de **su** reloj, que
    /// va `ppm` partes por millon rapido o lento respecto a la pared.
    struct Tarjeta {
        productor: Productor,
        tasa_real: f64,
        siguiente: f64,
        marco: u64,
        senal: Box<dyn FnMut(u64, f64) -> f32>,
        callada: bool,
    }

    impl Tarjeta {
        fn nueva(
            productor: Productor,
            ppm: f64,
            senal: Box<dyn FnMut(u64, f64) -> f32>,
        ) -> Tarjeta {
            Tarjeta {
                productor,
                tasa_real: f64::from(FS) * (1.0 + ppm * 1e-6),
                siguiente: 0.0,
                marco: 0,
                senal,
                callada: false,
            }
        }

        /// Entrega lo que su reloj haya capturado hasta `t`.
        fn hasta(&mut self, t: f64) {
            let c = self.productor.canales();
            let mut paquete = vec![0.0f32; 480 * c];
            while self.siguiente + 480.0 / self.tasa_real <= t {
                for m in paquete.chunks_exact_mut(c) {
                    let instante = self.marco as f64 / self.tasa_real;
                    let v = (self.senal)(self.marco, instante);
                    m.fill(v);
                    self.marco += 1;
                }
                if !self.callada {
                    self.productor.escribir(&paquete);
                }
                self.siguiente += 480.0 / self.tasa_real;
            }
        }
    }

    fn entrada(
        papel: Papel,
        ppm: f64,
        tratamiento: Tratamiento,
        senal: Box<dyn FnMut(u64, f64) -> f32>,
        canales: usize,
    ) -> (Entrada, Tarjeta, Arc<Compartido>) {
        let (p, c) = anillo(FS as usize * 2, canales);
        let comp = Compartido::nuevo(1.0);
        let e = Entrada::nueva(papel, c, FS, FS, tratamiento, Arc::clone(&comp));
        (e, Tarjeta::nueva(p, ppm, senal), comp)
    }

    /// Hace correr el reloj de pared `segundos` con el hilo de mezcla
    /// despertando cada 15,6 ms, y devuelve la salida.
    fn correr(
        nucleo: &mut Nucleo,
        tarjetas: &mut [Tarjeta],
        segundos: f64,
        mut cada: impl FnMut(&[i16], u64),
    ) -> u64 {
        let tick = 0.015_6;
        let mut t = 0.0;
        let mut producidos = 0u64;
        while t < segundos {
            t += tick;
            for k in tarjetas.iter_mut() {
                k.hasta(t);
            }
            let toca = (t * f64::from(FS)) as u64 - producidos;
            let mut dt = tick;
            let mut quedan = toca as usize;
            while quedan > 0 {
                let n = quedan.min(nucleo.max_bloque());
                let b = nucleo.mezclar(n, dt);
                cada(b, producidos);
                producidos += n as u64;
                quedan -= n;
                dt = 0.0;
            }
        }
        producidos
    }

    /// Un chasquido cada 5 s de pared, del reloj de la tarjeta.
    fn chasquidos() -> Box<dyn FnMut(u64, f64) -> f32> {
        let mut ultimo = -1i64;
        Box::new(move |_, instante| {
            let k = (instante / 5.0) as i64;
            if k != ultimo && instante - k as f64 * 5.0 < 1.0 / 48_000.0 * 1.01 {
                ultimo = k;
                0.5
            } else {
                0.0
            }
        })
    }

    /// El retraso de cada chasquido en la salida respecto a cuando sono, en
    /// ms.
    fn retrasos(ppm: f64, segundos: f64) -> Vec<f64> {
        let (e, k, _) = entrada(Papel::Equipo, ppm, Tratamiento::default(), chasquidos(), 1);
        let mut nucleo = Nucleo::nuevo(FS, 1, vec![e], Arc::new(Medidor::default()));
        let mut tarjetas = [k];
        let mut picos: Vec<(u64, i16)> = Vec::new();
        correr(&mut nucleo, &mut tarjetas, segundos, |b, desde| {
            for (i, v) in b.iter().enumerate() {
                if *v > 3_000 {
                    let j = desde + i as u64;
                    match picos.last_mut() {
                        Some((pj, pv)) if j - *pj < 100 => {
                            if *v > *pv {
                                *pj = j;
                                *pv = *v;
                            }
                        }
                        _ => picos.push((j, *v)),
                    }
                }
            }
        });
        picos
            .iter()
            .map(|(j, _)| {
                let salida = *j as f64 / f64::from(FS);
                let k = (salida / 5.0).floor();
                (salida - k * 5.0) * 1000.0
            })
            .collect()
    }

    #[test]
    fn diez_minutos_con_relojes_separados_siguen_a_la_par() {
        // +-400 ppm durante 10 minutos son 0,24 s de deriva cada una sin
        // corregir (0,48 s entre las dos): mas que una hora a 70 ppm.
        let rapida = retrasos(400.0, 600.0);
        let lenta = retrasos(-400.0, 600.0);
        assert!(
            rapida.len() >= 110 && lenta.len() >= 110,
            "{} {}",
            rapida.len(),
            lenta.len()
        );
        // Pasado el primer minuto, cada una va con un retraso fijo...
        for r in [&rapida, &lenta] {
            let cola = &r[12..];
            let (min, max) = cola
                .iter()
                .fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
            assert!(
                max - min < 5.0,
                "el retraso se movio de {min:.1} a {max:.1} ms"
            );
            // ...del orden del colchon.
            assert!((30.0..90.0).contains(&min), "{min}");
        }
        // ...y las dos, el mismo: la diferencia es menor de 20 ms.
        for (a, b) in rapida[12..].iter().zip(&lenta[12..]) {
            assert!((a - b).abs() < 20.0, "{a:.1} contra {b:.1}");
        }
    }

    #[test]
    fn el_equipo_callado_no_para_la_salida_y_vuelve_a_su_sitio() {
        let (e, k, comp) = entrada(
            Papel::Equipo,
            0.0,
            Tratamiento::default(),
            Box::new(|_, _| 0.25),
            1,
        );
        let mut nucleo = Nucleo::nuevo(FS, 1, vec![e], Arc::new(Medidor::default()));
        let mut tarjetas = [k];
        let total = correr(&mut nucleo, &mut tarjetas, 1.0, |_, _| {});
        tarjetas[0].callada = true;
        let mut ceros = 0u64;
        let parte = correr_desde(&mut nucleo, &mut tarjetas, 1.0, 2.0, total, |b| {
            ceros += b.iter().filter(|v| **v == 0).count() as u64;
        });
        // La salida siguio al paso de la pared aunque no llegara nada.
        assert!((parte as i64 - 48_000).abs() < 800, "{parte}");
        assert!(ceros > 40_000);
        assert!(comp.cuentas.faltaron.load(Ordering::Relaxed) > 0);
        tarjetas[0].callada = false;
        let mut sonando = 0u64;
        correr_desde(&mut nucleo, &mut tarjetas, 2.0, 3.0, total + parte, |b| {
            sonando += b.iter().filter(|v| **v > 8_000).count() as u64;
        });
        assert!(sonando > 40_000, "{sonando}");
    }

    /// Como `correr`, pero siguiendo desde `t0` con `producidos` ya hechos.
    fn correr_desde(
        nucleo: &mut Nucleo,
        tarjetas: &mut [Tarjeta],
        t0: f64,
        t1: f64,
        mut producidos: u64,
        mut cada: impl FnMut(&[i16]),
    ) -> u64 {
        let antes = producidos;
        let tick = 0.015_6;
        let mut t = t0;
        while t < t1 {
            t += tick;
            for k in tarjetas.iter_mut() {
                k.hasta(t);
            }
            let toca = ((t * f64::from(FS)) as u64).saturating_sub(producidos) as usize;
            let mut quedan = toca;
            let mut dt = tick;
            while quedan > 0 {
                let n = quedan.min(nucleo.max_bloque());
                cada(nucleo.mezclar(n, dt));
                producidos += n as u64;
                quedan -= n;
                dt = 0.0;
            }
        }
        producidos - antes
    }

    #[test]
    fn dos_fuentes_al_tope_se_limitan_y_no_dan_la_vuelta() {
        let (a, ka, _) = entrada(
            Papel::Microfono,
            0.0,
            Tratamiento::default(),
            Box::new(|n, _| if n % 96 < 48 { 0.99 } else { -0.99 }),
            2,
        );
        let (b, kb, _) = entrada(
            Papel::Equipo,
            0.0,
            Tratamiento::default(),
            Box::new(|n, _| if n % 96 < 48 { 0.99 } else { -0.99 }),
            2,
        );
        let mut nucleo = Nucleo::nuevo(FS, 2, vec![a, b], Arc::new(Medidor::default()));
        let mut tarjetas = [ka, kb];
        let mut tope = 0i32;
        let mut positivos = 0u64;
        correr(&mut nucleo, &mut tarjetas, 1.0, |b, _| {
            for v in b {
                tope = tope.max(i32::from(*v).abs());
                positivos += u64::from(*v > 20_000);
            }
        });
        assert!(tope < 32_767, "{tope}");
        assert!(positivos > 10_000, "sigue sonando alto");
    }

    #[test]
    fn el_realce_es_solo_del_microfono() {
        let quieto =
            |_: u64, t: f64| (0.02 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()) as f32;
        let todo = Tratamiento {
            realce_voz: true,
            paso_alto: true,
            puerta: false,
        };
        let (m, km, cm) = entrada(Papel::Microfono, 0.0, todo, Box::new(quieto), 1);
        let (e, ke, ce) = entrada(Papel::Equipo, 0.0, todo, Box::new(quieto), 1);
        let mut nucleo = Nucleo::nuevo(FS, 1, vec![m, e], Arc::new(Medidor::default()));
        correr(&mut nucleo, &mut [km, ke], 2.0, |_, _| {});
        let (pm, pe) = (cm.medidor.leer().pico, ce.medidor.leer().pico);
        assert!(pm > 0.2, "microfono realzado: {pm}");
        assert!((pe - 0.02).abs() < 0.002, "equipo tal cual: {pe}");
    }

    #[test]
    fn en_pausa_se_tira_lo_que_llega() {
        let (e, k, comp) = entrada(
            Papel::Microfono,
            0.0,
            Tratamiento::default(),
            Box::new(|_, _| 0.5),
            1,
        );
        let mut nucleo = Nucleo::nuevo(FS, 1, vec![e], Arc::new(Medidor::default()));
        let mut tarjetas = [k];
        tarjetas[0].hasta(0.5);
        assert!(nucleo.pendiente() > 20_000);
        nucleo.tirar();
        assert_eq!(nucleo.pendiente(), 0);
        assert_eq!(comp.medidor.leer(), Nivel::default());
    }

    #[test]
    fn la_ganancia_se_cambia_en_marcha() {
        let (e, k, comp) = entrada(
            Papel::Equipo,
            0.0,
            Tratamiento::default(),
            Box::new(|_, _| 0.5),
            1,
        );
        let mut nucleo = Nucleo::nuevo(FS, 1, vec![e], Arc::new(Medidor::default()));
        let mut tarjetas = [k];
        let hechos = correr(&mut nucleo, &mut tarjetas, 0.5, |_, _| {});
        comp.ganancia.poner(0.5);
        correr_desde(&mut nucleo, &mut tarjetas, 0.5, 1.0, hechos, |_| {});
        assert!((comp.medidor.leer().pico - 0.25).abs() < 0.01);
        comp.ganancia.poner(f32::NAN);
        assert_eq!(comp.ganancia.leer(), 1.0);
    }
}
