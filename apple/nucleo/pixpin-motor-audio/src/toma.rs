//! **La toma**: de los paquetes que da la plataforma al anillo de su fuente.
//!
//! Lo que antes hacia el hilo de captura de Windows con cada paquete, sin
//! Windows: asi se prueba en Linux y lo usa igual la captura de Android
//! (la llamada de AAudio/Oboe con cada bloque).
//!
//! Por cada paquete (ya convertido a coma flotante, en los canales de la
//! salida, a la tasa del aparato):
//!
//! 1. Lo **fecha**: con la fecha del aparato (QPC de `GetBuffer` en Windows,
//!    `AAudioStream_getTimestamp` en Android) o, si no la hay o es mala
//!    (`AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR`, cero, del futuro), con la hora
//!    de ahora menos lo que dura, como hace OBS sin `useDeviceTiming`
//!    (`WASAPISource::ProcessCaptureData`).
//! 2. Lo **coloca** por su fecha ([`Relleno`]): el silencio que falte
//!    antes, o lo que sobre recortado.
//! 3. Si el aparato va a otra tasa que el anillo (se cambio de aparato a
//!    mitad), lo **remuestrea** aqui.
//! 4. Lo escribe y **sella** el anillo con hasta cuando llega (ver
//!    [`crate::anillo::Productor::sellar`]).
//!
//! Y sin paquetes, [`Toma::al_paso`] pone silencio al paso del reloj.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::anillo::Productor;
use crate::medidas::Cuentas;
use crate::nucleo::Compartido;
use crate::reloj::{Relleno, duracion_de};
use crate::remuestreo::Remuestreador;

/// Lo mas que se reserva para un paquete, en segundos de su tasa. Hay
/// aparatos que dicen tener un anillo enorme (la captura por proceso de
/// Windows devuelve `GetBufferSize` de miles de millones de marcos, ver la
/// documentacion de `AudioClient::new_application_loopback_client` en
/// wasapi-rs): un paquete de verdad nunca pasa de unos cientos de ms.
pub const PAQUETE_MAXIMO_S: u32 = 1;

/// Marcos que se reservan para los paquetes de un aparato a `tasa` que dice
/// que su anillo tiene `dice` marcos.
pub fn marcos_de_paquete(dice: Option<u32>, tasa: u32) -> usize {
    let tope = (tasa.max(1) * PAQUETE_MAXIMO_S) as usize;
    match dice {
        Some(n) if n > 0 => (n as usize).min(tope),
        _ => tope / 2,
    }
    .max(64)
}

/// La toma de una fuente.
pub struct Toma {
    productor: Productor,
    relleno: Relleno,
    canales: usize,
    /// La tasa del anillo, fija toda la grabacion.
    tasa: u32,
    /// La del aparato abierto ahora.
    tasa_aparato: u32,
    a_anillo: Option<Remuestreador>,
    remuestreado: Vec<f32>,
    compartido: Arc<Compartido>,
    saltos_vistos: u64,
}

impl Toma {
    /// La toma de un anillo a `tasa`, que empieza en `inicio` (100 ns del
    /// reloj comun). `umbral`: desde cuanto un hueco entre paquetes es de
    /// verdad (ver [`crate::reloj::UMBRAL_HUECO_MICROFONO`]).
    pub fn nueva(
        productor: Productor,
        tasa: u32,
        umbral: u64,
        inicio: u64,
        compartido: Arc<Compartido>,
    ) -> Toma {
        compartido
            .cuentas
            .tasa_aparato
            .store(tasa, Ordering::Relaxed);
        Toma {
            canales: productor.canales(),
            productor,
            relleno: Relleno::con_umbral(tasa, inicio, umbral),
            tasa,
            tasa_aparato: tasa,
            a_anillo: None,
            remuestreado: Vec::new(),
            compartido,
            saltos_vistos: 0,
        }
    }

    pub fn tasa(&self) -> u32 {
        self.tasa
    }

    pub fn canales(&self) -> usize {
        self.canales
    }

    pub fn tasa_aparato(&self) -> u32 {
        self.tasa_aparato
    }

    fn cuentas(&self) -> &Cuentas {
        &self.compartido.cuentas
    }

    /// Se abrio otro aparato (al volver a abrir), a `tasa_aparato` y con
    /// paquetes de hasta `max_marcos`. Si no va a la tasa del anillo, lo que
    /// entregue se remuestrea aqui. El primer paquete viene despues de un
    /// corte: su hueco se pone desde 1 ms.
    pub fn aparato(&mut self, tasa_aparato: u32, max_marcos: usize) {
        let tasa_aparato = tasa_aparato.max(1);
        self.tasa_aparato = tasa_aparato;
        self.cuentas()
            .tasa_aparato
            .store(tasa_aparato, Ordering::Relaxed);
        self.relleno.discontinuidad();
        if tasa_aparato == self.tasa {
            self.a_anillo = None;
            self.remuestreado = Vec::new();
            return;
        }
        let max_marcos = max_marcos.max(64);
        self.a_anillo = Some(Remuestreador::nuevo(
            tasa_aparato,
            self.tasa,
            self.canales,
            max_marcos,
        ));
        // Lo que da un paquete entero a la tasa del anillo, con margen: si
        // fuera menos, lo que no cupiera se iria quedando en el remuestreador
        // y creceria sin parar.
        let salen = (max_marcos as u64 * u64::from(self.tasa)).div_ceil(u64::from(tasa_aparato));
        self.remuestreado = vec![0.0; (salen as usize * 2 + 64) * self.canales];
    }

    /// Un paquete: `muestras` intercaladas en los canales de la salida, a la
    /// tasa del aparato. `fecha`: cuando se capturo su primer marco (100 ns
    /// del reloj comun), si el aparato la sabe. `ahora`: la hora de ahora.
    /// `discontinuidad`: el aparato dice que tiro algo antes de este.
    pub fn paquete(
        &mut self,
        muestras: &[f32],
        fecha: Option<u64>,
        ahora: u64,
        discontinuidad: bool,
    ) {
        let c = self.canales;
        let n = muestras.len() / c;
        if n == 0 {
            return;
        }
        let muestras = &muestras[..n * c];
        Cuentas::sumar(&self.cuentas().capturados, n as u64);
        if discontinuidad {
            Cuentas::sumar(&self.cuentas().discontinuidades, 1);
            self.relleno.discontinuidad();
        }
        let dura = duracion_de(n as u64, self.tasa_aparato);
        let inicio = match fecha {
            Some(f) if f != 0 && f <= ahora => f,
            _ => ahora.saturating_sub(dura),
        };
        let hueco = self.relleno.paquete(inicio, dura);
        self.contar_saltos();
        if hueco > 0 {
            let puestos = self.productor.escribir_ceros(hueco as usize);
            Cuentas::sumar(&self.compartido.cuentas.silencio, puestos as u64);
            Cuentas::sumar(&self.compartido.cuentas.perdidos, hueco - puestos as u64);
        }
        let bloque: &[f32] = match self.a_anillo.as_mut() {
            Some(r) => {
                let necesita = ((n as u64 * u64::from(self.tasa))
                    .div_ceil(u64::from(self.tasa_aparato))
                    as usize
                    + 64)
                    * c;
                if self.remuestreado.len() < necesita {
                    // Un paquete mas grande de lo que dijo el aparato: fuera de
                    // lo normal, se hace sitio.
                    self.remuestreado.resize(necesita, 0.0);
                }
                r.meter(muestras);
                let k = r.sacar(&mut self.remuestreado);
                &self.remuestreado[..k * c]
            }
            None => muestras,
        };
        let total = bloque.len() / c;
        let saltar = self.relleno.recortar(total as u64) as usize;
        Cuentas::sumar(&self.compartido.cuentas.recortados, saltar as u64);
        let escritos = self.productor.escribir(&bloque[saltar * c..]);
        Cuentas::sumar(
            &self.compartido.cuentas.perdidos,
            (total - saltar - escritos) as u64,
        );
        self.productor.sellar(self.relleno.hasta());
    }

    /// Sin paquete nuevo, a las `ahora`: el silencio que toque.
    pub fn al_paso(&mut self, ahora: u64) {
        let n = self.relleno.espera(ahora);
        self.contar_saltos();
        if n > 0 {
            let puestos = self.productor.escribir_ceros(n as usize);
            Cuentas::sumar(&self.compartido.cuentas.silencio, puestos as u64);
            Cuentas::sumar(&self.compartido.cuentas.perdidos, n - puestos as u64);
            self.productor.sellar(self.relleno.hasta());
        }
    }

    fn contar_saltos(&mut self) {
        let s = self.relleno.saltos();
        if s != self.saltos_vistos {
            Cuentas::sumar(&self.compartido.cuentas.saltos, s - self.saltos_vistos);
            self.saltos_vistos = s;
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::anillo::{Consumidor, anillo};
    use crate::reloj::{UMBRAL_HUECO, marcos_en};

    const FS: u32 = 48_000;
    const MS: u64 = 10_000;

    fn toma(canales: usize) -> (Toma, Consumidor, Arc<Compartido>) {
        let (p, c) = anillo(FS as usize * 2, canales);
        let comp = Compartido::nuevo(1.0);
        (
            Toma::nueva(p, FS, UMBRAL_HUECO, 0, Arc::clone(&comp)),
            c,
            comp,
        )
    }

    #[test]
    fn los_paquetes_seguidos_entran_tal_cual_y_sellan() {
        let (mut t, mut c, comp) = toma(1);
        let mut f = 0;
        for _ in 0..10 {
            t.paquete(&[0.5; 480], Some(f), f + 10 * MS, false);
            f += 10 * MS;
        }
        assert_eq!(c.disponibles(), 4_800);
        assert_eq!(c.sello(), 100 * MS);
        let mut v = vec![0.0; 4_800];
        c.leer(&mut v);
        assert!(v.iter().all(|x| *x == 0.5));
        assert_eq!(comp.cuentas.foto().capturados, 4_800);
        assert_eq!(comp.cuentas.foto().silencio, 0);
    }

    #[test]
    fn sin_fecha_se_fecha_con_la_hora_y_un_hueco_se_rellena() {
        let (mut t, c, comp) = toma(2);
        t.paquete(&[0.1; 960], None, 10 * MS, false);
        // Fecha del futuro (un controlador roto): como si no la hubiera.
        t.paquete(&[0.1; 960], Some(u64::MAX), 20 * MS, false);
        assert_eq!(comp.cuentas.foto().silencio, 0);
        // 100 ms sin nada y luego un paquete fechado donde toca.
        t.paquete(&[0.1; 960], Some(120 * MS), 130 * MS, false);
        assert_eq!(comp.cuentas.foto().silencio, 4_800);
        assert_eq!(c.disponibles(), 480 * 3 + 4_800);
    }

    #[test]
    fn el_equipo_callado_avanza_al_paso_y_sella_lo_puesto() {
        let (mut t, c, comp) = toma(1);
        let mut ahora = 0;
        while ahora < 1_000 * MS {
            ahora += 10 * MS;
            t.al_paso(ahora);
        }
        let puesto = comp.cuentas.foto().silencio;
        assert_eq!(
            puesto,
            marcos_en(1_000 * MS - crate::reloj::MARGEN_ESPERA, FS)
        );
        assert_eq!(c.disponibles() as u64, puesto);
        assert_eq!(c.sello(), duracion_de(puesto, FS));
    }

    #[test]
    fn un_aparato_a_otra_tasa_se_remuestrea_a_la_del_anillo() {
        let (mut t, c, comp) = toma(1);
        t.aparato(44_100, 441);
        assert_eq!(comp.cuentas.foto().tasa_aparato, 44_100);
        let mut f = 0;
        for _ in 0..100 {
            t.paquete(&[0.25; 441], Some(f), f + 10 * MS, false);
            f += 10 * MS;
        }
        // Un segundo de 44,1 kHz son 48 000 marcos del anillo (menos lo que
        // se queda dentro del filtro).
        let hay = c.disponibles() as i64;
        assert!((hay - 48_000).abs() < 100, "{hay}");
        // Y de vuelta a la misma tasa, sin remuestrear.
        t.aparato(48_000, 480);
        assert!(t.a_anillo.is_none());
    }

    #[test]
    fn una_discontinuidad_se_cuenta_y_su_hueco_se_cree() {
        let (p, c) = anillo(FS as usize * 2, 1);
        let comp = Compartido::nuevo(1.0);
        let mut t = Toma::nueva(
            p,
            FS,
            crate::reloj::UMBRAL_HUECO_MICROFONO,
            0,
            Arc::clone(&comp),
        );
        t.paquete(&[0.0; 480], Some(0), 10 * MS, false);
        // 40 ms de hueco: sin marca, en un microfono, es temblor.
        t.paquete(&[0.0; 480], Some(50 * MS), 60 * MS, false);
        assert_eq!(comp.cuentas.foto().silencio, 0);
        // Con marca, se pone.
        t.paquete(&[0.0; 480], Some(100 * MS), 110 * MS, true);
        let e = comp.cuentas.foto();
        assert_eq!((e.silencio, e.discontinuidades), (1_920, 1));
        assert_eq!(c.disponibles(), 480 * 3 + 1_920);
    }

    #[test]
    fn el_tamano_de_paquete_que_dice_el_aparato_se_acota() {
        assert_eq!(marcos_de_paquete(Some(9_600), FS), 9_600);
        // La captura por proceso dice 3 131 961 357 marcos.
        assert_eq!(marcos_de_paquete(Some(3_131_961_357), FS), 48_000);
        assert_eq!(marcos_de_paquete(None, FS), 24_000);
        assert_eq!(marcos_de_paquete(Some(0), 8_000), 4_000);
        assert_eq!(marcos_de_paquete(Some(10), FS), 64);
    }

    #[test]
    fn una_suspension_cuenta_un_salto_y_no_llena_el_anillo() {
        let (mut t, c, comp) = toma(1);
        t.paquete(&[0.0; 480], Some(0), 10 * MS, false);
        t.al_paso(3_600 * 1_000 * MS);
        assert_eq!(comp.cuentas.foto().saltos, 1);
        assert_eq!(c.disponibles(), 480);
    }
}
