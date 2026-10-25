//! **El anillo entre el hilo que captura y el que mezcla**: una cola de un
//! solo productor y un solo consumidor, sin cerrojos y sin reservar memoria.
//!
//! El hilo de captura no puede esperar a nadie: si el que mezcla se atasca
//! (el sistema le quita la CPU un momento) y la captura se quedara esperando
//! un cerrojo, el anillo de la propia tarjeta se llenaria y Windows tiraria
//! sonido. Aqui escribir nunca espera: si no cabe, lo que no cabe se tira y
//! se cuenta.
//!
//! Se guardan marcos enteros (todos los canales de un instante juntos): ni
//! escribir ni leer parten nunca un marco por la mitad.
//!
//! Las dos cuentas (`escritas` y `leidas`) solo crecen y dan la vuelta con
//! `wrapping`; la posicion en el anillo es la cuenta modulo la capacidad.
//! Cada lado escribe solo la suya y lee la del otro con `Acquire`, que es lo
//! que hace visibles las muestras copiadas antes del `Release` del otro.

use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Anillo {
    datos: Box<[UnsafeCell<f32>]>,
    canales: usize,
    escritas: AtomicUsize,
    leidas: AtomicUsize,
}

// SAFETY: cada posicion del anillo la toca a la vez como mucho un lado: el
// productor solo escribe en el hueco libre (entre `escritas` y `leidas +
// capacidad`) y el consumidor solo lee lo ya publicado (entre `leidas` y
// `escritas`). Las cuentas se publican con `Release` y se leen con
// `Acquire`, asi que lo copiado es visible antes de que el otro lado lo
// considere suyo. `Productor` y `Consumidor` no son `Clone`, de modo que hay
// exactamente uno de cada.
unsafe impl Sync for Anillo {}
// SAFETY: los datos son `f32` y las cuentas atomicas; nada ata el anillo a
// un hilo.
unsafe impl Send for Anillo {}

impl Anillo {
    fn capacidad(&self) -> usize {
        self.datos.len()
    }

    fn base(&self) -> *mut f32 {
        // `UnsafeCell<f32>` tiene la misma representacion que `f32`.
        self.datos.as_ptr() as *mut f32
    }
}

/// El lado que escribe (el hilo de captura).
pub struct Productor {
    anillo: Arc<Anillo>,
}

/// El lado que lee (el hilo de mezcla).
pub struct Consumidor {
    anillo: Arc<Anillo>,
}

/// Un anillo de `marcos` marcos de `canales` canales.
pub fn anillo(marcos: usize, canales: usize) -> (Productor, Consumidor) {
    let canales = canales.max(1);
    let n = marcos.max(1) * canales;
    let datos: Box<[UnsafeCell<f32>]> = (0..n).map(|_| UnsafeCell::new(0.0)).collect();
    let anillo = Arc::new(Anillo {
        datos,
        canales,
        escritas: AtomicUsize::new(0),
        leidas: AtomicUsize::new(0),
    });
    (
        Productor {
            anillo: Arc::clone(&anillo),
        },
        Consumidor { anillo },
    )
}

impl Productor {
    pub fn canales(&self) -> usize {
        self.anillo.canales
    }

    /// Marcos que caben ahora mismo.
    pub fn libres(&self) -> usize {
        let a = &self.anillo;
        let usadas = a
            .escritas
            .load(Ordering::Relaxed)
            .wrapping_sub(a.leidas.load(Ordering::Acquire));
        (a.capacidad() - usadas) / a.canales
    }

    /// Copia los marcos enteros de `muestras` que quepan. Devuelve cuantos.
    pub fn escribir(&mut self, muestras: &[f32]) -> usize {
        let marcos = (muestras.len() / self.anillo.canales).min(self.libres());
        self.copiar(marcos, |destino, desde, n| {
            // SAFETY: `destino` apunta a `n` posiciones libres del anillo
            // (ver `copiar`) y `desde + n` no pasa de `muestras.len()`
            // porque `marcos` se acoto arriba.
            unsafe { std::ptr::copy_nonoverlapping(muestras.as_ptr().add(desde), destino, n) }
        })
    }

    /// Escribe `marcos` marcos de silencio (o los que quepan).
    pub fn escribir_ceros(&mut self, marcos: usize) -> usize {
        let marcos = marcos.min(self.libres());
        self.copiar(marcos, |destino, _, n| {
            // SAFETY: `destino` apunta a `n` posiciones libres del anillo.
            unsafe { std::ptr::write_bytes(destino, 0, n) }
        })
    }

    /// Reparte `marcos` marcos en los (como mucho) dos tramos contiguos del
    /// anillo y publica la cuenta al final.
    fn copiar(&mut self, marcos: usize, mut poner: impl FnMut(*mut f32, usize, usize)) -> usize {
        if marcos == 0 {
            return 0;
        }
        let a = &self.anillo;
        let total = marcos * a.canales;
        let escritas = a.escritas.load(Ordering::Relaxed);
        let inicio = escritas % a.capacidad();
        let primero = total.min(a.capacidad() - inicio);
        // SAFETY: `inicio + primero <= capacidad` y el segundo tramo empieza
        // en 0 con `total - primero <= capacidad`; las posiciones son del
        // hueco libre porque `marcos <= libres()`.
        unsafe {
            poner(a.base().add(inicio), 0, primero);
            if total > primero {
                poner(a.base(), primero, total - primero);
            }
        }
        a.escritas
            .store(escritas.wrapping_add(total), Ordering::Release);
        marcos
    }
}

impl Consumidor {
    pub fn canales(&self) -> usize {
        self.anillo.canales
    }

    /// Marcos que hay para leer.
    pub fn disponibles(&self) -> usize {
        let a = &self.anillo;
        a.escritas
            .load(Ordering::Acquire)
            .wrapping_sub(a.leidas.load(Ordering::Relaxed))
            / a.canales
    }

    /// Lee hasta llenar `destino` (en marcos enteros). Devuelve cuantos.
    pub fn leer(&mut self, destino: &mut [f32]) -> usize {
        let marcos = (destino.len() / self.anillo.canales).min(self.disponibles());
        self.avanzar(marcos, Some(destino))
    }

    /// Tira `marcos` marcos sin leerlos (o los que haya).
    pub fn descartar(&mut self, marcos: usize) -> usize {
        let marcos = marcos.min(self.disponibles());
        self.avanzar(marcos, None)
    }

    fn avanzar(&mut self, marcos: usize, destino: Option<&mut [f32]>) -> usize {
        if marcos == 0 {
            return 0;
        }
        let a = &self.anillo;
        let total = marcos * a.canales;
        let leidas = a.leidas.load(Ordering::Relaxed);
        if let Some(destino) = destino {
            let inicio = leidas % a.capacidad();
            let primero = total.min(a.capacidad() - inicio);
            // SAFETY: las `total` posiciones desde `inicio` (dando la vuelta)
            // estan publicadas por el productor (`marcos <= disponibles()`,
            // leido con `Acquire`) y el productor no las toca hasta que se
            // publique `leidas`; `destino` tiene al menos `total` huecos.
            unsafe {
                std::ptr::copy_nonoverlapping(a.base().add(inicio), destino.as_mut_ptr(), primero);
                if total > primero {
                    std::ptr::copy_nonoverlapping(
                        a.base(),
                        destino.as_mut_ptr().add(primero),
                        total - primero,
                    );
                }
            }
        }
        a.leidas
            .store(leidas.wrapping_add(total), Ordering::Release);
        marcos
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lo_que_entra_sale_en_orden_y_dando_la_vuelta() {
        let (mut p, mut c) = anillo(4, 2);
        assert_eq!(p.escribir(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), 3);
        let mut d = [0.0; 4];
        assert_eq!(c.leer(&mut d), 2);
        assert_eq!(d, [1.0, 2.0, 3.0, 4.0]);
        // Ahora la escritura da la vuelta al final del anillo.
        assert_eq!(p.escribir(&[7.0, 8.0, 9.0, 10.0, 11.0, 12.0]), 3);
        let mut d = [0.0; 8];
        assert_eq!(c.leer(&mut d), 4);
        assert_eq!(d, [5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0]);
    }

    #[test]
    fn lleno_tira_lo_que_no_cabe_y_no_parte_marcos() {
        let (mut p, mut c) = anillo(2, 2);
        assert_eq!(p.escribir(&[1.0; 7]), 2, "tres marcos y medio: caben dos");
        assert_eq!(p.libres(), 0);
        assert_eq!(p.escribir(&[1.0; 2]), 0);
        assert_eq!(c.descartar(10), 2);
        assert_eq!(c.disponibles(), 0);
        assert_eq!(p.escribir_ceros(5), 2);
        let mut d = [9.0; 4];
        c.leer(&mut d);
        assert_eq!(d, [0.0; 4]);
    }

    #[test]
    fn dos_hilos_no_pierden_ni_desordenan_nada() {
        let (mut p, mut c) = anillo(257, 2);
        const N: usize = 200_000;
        let escritor = std::thread::spawn(move || {
            let mut i = 0usize;
            while i < N {
                let marco = [i as f32, -(i as f32)];
                if p.escribir(&marco) == 1 {
                    i += 1;
                } else {
                    std::thread::yield_now();
                }
            }
        });
        let mut esperado = 0usize;
        let mut d = [0.0f32; 64];
        while esperado < N {
            let n = c.leer(&mut d);
            for m in d[..n * 2].chunks_exact(2) {
                assert_eq!(m[0], esperado as f32);
                assert_eq!(m[1], -(esperado as f32));
                esperado += 1;
            }
            if n == 0 {
                std::thread::yield_now();
            }
        }
        escritor.join().unwrap();
    }
}
