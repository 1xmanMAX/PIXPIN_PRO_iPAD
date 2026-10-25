//! **El PCM para quien lo quiera**: el video de una reunion, el `.m4a`, la
//! onda de una nota de voz.
//!
//! El contrato, que es el que consume el motor de video:
//!
//! - Bloques de **i16 intercalado**, a la tasa fija de la salida (44 100 o
//!   48 000) y con sus canales (1 o 2), que no cambian en toda la grabacion.
//! - Cada bloque lleva su **posicion**: marcos desde el principio. En pausa
//!   no llega nada y la posicion no avanza; sin perdidas, la posicion de un
//!   bloque es la suma de los marcos de todos los anteriores.
//! - Cola **acotada**. El hilo de mezcla nunca espera a nadie: si un
//!   suscriptor va tarde y su cola esta llena, el bloque se le tira **solo a
//!   el** y se le cuenta ([`Receptor::perdidos`]); los demas no se enteran.
//!   La posicion del siguiente bloque deja ver el hueco.
//! - Cuando la grabacion termina, la cola se cierra: [`Receptor::recibir`]
//!   devuelve `None` en cuanto se vacia.
//!
//! Para no reservar un `Vec` por bloque, quien recibe puede devolverlo con
//! [`Receptor::devolver`] y el siguiente bloque viaja en el.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::time::Duration;

/// Un trozo de sonido ya mezclado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BloquePcm {
    /// Marcos desde el principio de la grabacion hasta el primero de este.
    pub posicion: u64,
    pub muestreo: u32,
    pub canales: u16,
    /// Intercaladas: `marcos() * canales` muestras.
    pub muestras: Vec<i16>,
}

impl BloquePcm {
    pub fn marcos(&self) -> usize {
        self.muestras.len() / self.canales.max(1) as usize
    }

    /// Donde acaba: la posicion del siguiente si no se pierde nada.
    pub fn fin(&self) -> u64 {
        self.posicion + self.marcos() as u64
    }
}

/// El lado del motor de una suscripcion.
#[derive(Debug)]
pub struct Suscripcion {
    envio: SyncSender<BloquePcm>,
    devueltos: Receiver<Vec<i16>>,
    perdidos: Arc<AtomicU64>,
}

/// El lado de quien escucha.
#[derive(Debug)]
pub struct Receptor {
    llegada: Receiver<BloquePcm>,
    devolver: SyncSender<Vec<i16>>,
    perdidos: Arc<AtomicU64>,
}

/// Una suscripcion de `capacidad` bloques (cada bloque son unos 10-16 ms).
/// La [`Suscripcion`] se le da al motor (en las opciones, para recibir desde
/// el primer marco, o con `suscribir_pcm` en marcha) y el [`Receptor`] se
/// queda quien escucha.
pub fn suscripcion(capacidad: usize) -> (Suscripcion, Receptor) {
    let capacidad = capacidad.max(1);
    let (envio, llegada) = sync_channel(capacidad);
    let (devolver, devueltos) = sync_channel(capacidad);
    let perdidos = Arc::new(AtomicU64::new(0));
    (
        Suscripcion {
            envio,
            devueltos,
            perdidos: Arc::clone(&perdidos),
        },
        Receptor {
            llegada,
            devolver,
            perdidos,
        },
    )
}

/// Por que no llego un bloque a tiempo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinBloque {
    /// Aun no hay ninguno.
    Vacio,
    /// La grabacion termino y ya no llegaran mas.
    Cerrado,
}

impl Receptor {
    /// Espera al siguiente bloque. `None`: la grabacion termino.
    pub fn recibir(&self) -> Option<BloquePcm> {
        self.llegada.recv().ok()
    }

    /// Espera como mucho `tiempo`.
    pub fn recibir_hasta(&self, tiempo: Duration) -> Result<BloquePcm, SinBloque> {
        self.llegada.recv_timeout(tiempo).map_err(|e| match e {
            RecvTimeoutError::Timeout => SinBloque::Vacio,
            RecvTimeoutError::Disconnected => SinBloque::Cerrado,
        })
    }

    /// Sin esperar.
    pub fn probar(&self) -> Result<BloquePcm, SinBloque> {
        self.llegada.try_recv().map_err(|e| match e {
            TryRecvError::Empty => SinBloque::Vacio,
            TryRecvError::Disconnected => SinBloque::Cerrado,
        })
    }

    /// Bloques que se le tiraron a este receptor por ir tarde.
    pub fn perdidos(&self) -> u64 {
        self.perdidos.load(Ordering::Relaxed)
    }

    /// Devuelve la memoria de un bloque ya usado para que viaje en el
    /// siguiente. Opcional.
    pub fn devolver(&self, bloque: BloquePcm) {
        let _ = self.devolver.try_send(bloque.muestras);
    }
}

/// **El reparto**: el motor tiene uno y le pasa cada bloque mezclado.
#[derive(Debug, Default)]
pub struct Reparto {
    abonados: Vec<Suscripcion>,
}

impl Reparto {
    pub fn nuevo(abonados: Vec<Suscripcion>) -> Reparto {
        Reparto { abonados }
    }

    pub fn alta(&mut self, s: Suscripcion) {
        self.abonados.push(s);
    }

    pub fn cuantos(&self) -> usize {
        self.abonados.len()
    }

    /// Los perdidos de todos, sumados.
    pub fn perdidos(&self) -> u64 {
        self.abonados
            .iter()
            .map(|a| a.perdidos.load(Ordering::Relaxed))
            .sum()
    }

    /// Manda una copia de `muestras` a cada uno, sin esperar a ninguno.
    pub fn repartir(&mut self, posicion: u64, muestreo: u32, canales: u16, muestras: &[i16]) {
        if muestras.is_empty() {
            return;
        }
        let mut i = 0;
        while i < self.abonados.len() {
            let a = &self.abonados[i];
            let mut v = a.devueltos.try_recv().unwrap_or_default();
            v.clear();
            v.extend_from_slice(muestras);
            let bloque = BloquePcm {
                posicion,
                muestreo,
                canales,
                muestras: v,
            };
            match a.envio.try_send(bloque) {
                Ok(()) => i += 1,
                Err(TrySendError::Full(_)) => {
                    a.perdidos.fetch_add(1, Ordering::Relaxed);
                    i += 1;
                }
                // Quien escuchaba ya no esta: se le da de baja y el resto
                // sigue igual.
                Err(TrySendError::Disconnected(_)) => {
                    self.abonados.swap_remove(i);
                }
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_bloque_llega_con_su_posicion_y_al_cerrar_se_acaba() {
        let (s, r) = suscripcion(8);
        let mut rep = Reparto::nuevo(vec![s]);
        rep.repartir(0, 48_000, 2, &[1, 2, 3, 4]);
        rep.repartir(2, 48_000, 2, &[5, 6]);
        let a = r.recibir().unwrap();
        assert_eq!((a.posicion, a.marcos(), a.fin()), (0, 2, 2));
        let b = r.recibir().unwrap();
        assert_eq!((b.posicion, b.muestras.clone()), (2, vec![5, 6]));
        assert_eq!(r.probar(), Err(SinBloque::Vacio));
        drop(rep);
        assert_eq!(r.recibir(), None);
        assert_eq!(
            r.recibir_hasta(Duration::from_millis(1)),
            Err(SinBloque::Cerrado)
        );
    }

    #[test]
    fn el_que_va_tarde_pierde_solo_lo_suyo() {
        let (lento, r_lento) = suscripcion(1);
        let (rapido, r_rapido) = suscripcion(16);
        let mut rep = Reparto::nuevo(vec![lento, rapido]);
        for i in 0..5u64 {
            rep.repartir(i * 10, 48_000, 1, &[i as i16; 10]);
            // El rapido lee al momento.
            assert_eq!(r_rapido.recibir().unwrap().posicion, i * 10);
        }
        assert_eq!(r_lento.perdidos(), 4);
        assert_eq!(r_rapido.perdidos(), 0);
        assert_eq!(rep.perdidos(), 4);
        // Lo que si le llego al lento es el primero, y el hueco se ve en la
        // posicion del siguiente.
        assert_eq!(r_lento.recibir().unwrap().posicion, 0);
        rep.repartir(50, 48_000, 1, &[0; 10]);
        assert_eq!(r_lento.recibir().unwrap().posicion, 50);
    }

    #[test]
    fn el_que_se_va_se_da_de_baja_y_la_memoria_se_reutiliza() {
        let (a, ra) = suscripcion(4);
        let (b, rb) = suscripcion(4);
        let mut rep = Reparto::nuevo(vec![a, b]);
        drop(ra);
        rep.repartir(0, 48_000, 1, &[1; 100]);
        assert_eq!(rep.cuantos(), 1);
        let bloque = rb.recibir().unwrap();
        let puntero = bloque.muestras.as_ptr();
        rb.devolver(bloque);
        rep.repartir(100, 48_000, 1, &[2; 50]);
        let otro = rb.recibir().unwrap();
        assert_eq!(otro.muestras.as_ptr(), puntero, "viajo en el mismo Vec");
        assert_eq!(otro.muestras, vec![2; 50]);
    }
}
