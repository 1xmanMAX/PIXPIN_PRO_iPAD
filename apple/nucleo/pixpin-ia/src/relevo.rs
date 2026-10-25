//! **Varios modelos locales, uno cargado a la vez** (13-oct-2026).
//!
//! Con el modelo de mejor vista (Gemma 4 E2B, ~3,7 GB de RAM) y el de
//! siempre (Qwen3.5-2B, ~2,9 GB) bajados, los dos `llama-server` a la vez se
//! comerian 6,6 GB de un portatil de 16. El relevo lo impide: cada motor del
//! relevo, antes de contestar, **suelta a los demas** (matar su proceso
//! devuelve la memoria entera) y solo entonces carga el suyo. Y contestan de
//! uno en uno: mientras uno escribe, el otro espera su turno en vez de
//! arrancar su proceso al lado.
//!
//! Cambiar de modelo cuesta la carga del otro (~20 s), asi que quien elige
//! (`seleccion::pide_mejor_vista`) lo hace por turno, no por peticion.

use std::sync::{Arc, Mutex};

use crate::ErrorIa;
use crate::motor::{Motor, Peticion, Respuesta, Seguir, Trozo};

#[derive(Default)]
struct Dentro {
    /// Los motores del relevo, por su id (el del modelo).
    motores: Mutex<Vec<(String, Arc<dyn Motor>)>>,
    /// El que contesto el ultimo (y por tanto el que puede estar cargado).
    /// Se tiene cogido mientras uno contesta: es el turno.
    turno: Mutex<Option<String>>,
}

/// El relevo. Barato de clonar: todos los clones son el mismo.
#[derive(Clone, Default)]
pub struct Relevo {
    dentro: Arc<Dentro>,
}

impl Relevo {
    pub fn nuevo() -> Relevo {
        Relevo::default()
    }

    /// **El motor `id` del relevo**; si no estaba, se crea con `crear` (que
    /// no debe cargar nada: los motores de llama.cpp cargan en la primera
    /// peticion).
    pub fn motor(&self, id: &str, crear: impl FnOnce() -> Arc<dyn Motor>) -> Arc<dyn Motor> {
        let mut g = self.dentro.motores.lock().unwrap_or_else(|e| e.into_inner());
        let motor = match g.iter().find(|(i, _)| i == id) {
            Some((_, m)) => m.clone(),
            None => {
                let m = crear();
                g.push((id.to_string(), m.clone()));
                m
            }
        };
        Arc::new(Relevado {
            dentro: self.dentro.clone(),
            id: id.to_string(),
            motor,
        })
    }

    /// El que contesto el ultimo, si alguno.
    pub fn ultimo(&self) -> Option<String> {
        self.dentro.turno.try_lock().ok().and_then(|g| g.clone())
    }

    /// Suelta y olvida el motor `id` (el usuario cambio de modelo).
    pub fn quitar(&self, id: &str) {
        let mut g = self.dentro.motores.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = g.iter().position(|(x, _)| x == id) {
            g.remove(i).1.soltar();
        }
    }

    /// Suelta todos: la memoria vuelve entera.
    pub fn soltar_todo(&self) {
        let g = self.dentro.motores.lock().unwrap_or_else(|e| e.into_inner());
        for (_, m) in g.iter() {
            m.soltar();
        }
    }

    /// Cuantos motores hay en el relevo (cargados o no).
    pub fn cuantos(&self) -> usize {
        self.dentro.motores.lock().map(|g| g.len()).unwrap_or(0)
    }
}

/// Un motor dentro del relevo.
struct Relevado {
    dentro: Arc<Dentro>,
    id: String,
    motor: Arc<dyn Motor>,
}

impl Relevado {
    /// Hace `f` con el turno cogido: antes suelta al que estuviera cargado.
    fn con_turno<T>(&self, f: impl FnOnce() -> T) -> T {
        // El turno se tiene hasta acabar: nadie mas carga mientras tanto.
        let mut turno = self.dentro.turno.lock().unwrap_or_else(|e| e.into_inner());
        if turno.as_deref() != Some(self.id.as_str()) {
            // Solo el ultimo que contesto puede estar cargado: ese se suelta.
            if let Some(antes) = turno.as_deref() {
                tracing::info!(de = antes, a = %self.id, "asistente: cambio de modelo");
                let viejo = self
                    .dentro
                    .motores
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .iter()
                    .find(|(i, _)| i == antes)
                    .map(|(_, m)| m.clone());
                if let Some(m) = viejo {
                    m.soltar();
                }
            }
            *turno = Some(self.id.clone());
        }
        f()
    }
}

impl Motor for Relevado {
    fn completar(&self, p: &Peticion, al_trozo: &mut dyn FnMut(&Trozo) -> Seguir) -> Result<Respuesta, ErrorIa> {
        self.con_turno(|| self.motor.completar(p, al_trozo))
    }

    /// La precarga tambien pasa por el relevo (14-oct): suelta al otro
    /// modelo antes de cargar este, y espera si otro esta contestando.
    fn precalentar(&self) {
        self.con_turno(|| self.motor.precalentar())
    }

    fn soltar(&self) {
        self.motor.soltar();
    }

    fn catalogo_completo(&self) -> bool {
        self.motor.catalogo_completo()
    }

    fn a_la_vez(&self) -> usize {
        self.motor.a_la_vez()
    }
}

#[cfg(test)]
mod pruebas {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;
    use crate::motor::Mensaje;

    /// Un motor que cuenta cuantas veces se le solto y cuantos contestan a
    /// la vez en todo el relevo.
    struct Contador {
        soltado: AtomicUsize,
        a_la_vez: Arc<AtomicUsize>,
        maximo: Arc<AtomicUsize>,
    }

    impl Motor for Contador {
        fn completar(&self, _: &Peticion, _: &mut dyn FnMut(&Trozo) -> Seguir) -> Result<Respuesta, ErrorIa> {
            let n = self.a_la_vez.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximo.fetch_max(n, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(15));
            self.a_la_vez.fetch_sub(1, Ordering::SeqCst);
            Ok(Respuesta::de_texto("ok"))
        }
        fn soltar(&self) {
            self.soltado.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn par() -> (Relevo, Arc<Contador>, Arc<Contador>, Arc<AtomicUsize>) {
        let a_la_vez = Arc::new(AtomicUsize::new(0));
        let maximo = Arc::new(AtomicUsize::new(0));
        let nuevo = || {
            Arc::new(Contador {
                soltado: AtomicUsize::new(0),
                a_la_vez: a_la_vez.clone(),
                maximo: maximo.clone(),
            })
        };
        (Relevo::nuevo(), nuevo(), nuevo(), maximo)
    }

    fn pedir(m: &Arc<dyn Motor>) {
        m.completar(&Peticion::nueva(vec![Mensaje::usuario("hola")]), &mut |_| Seguir::Si)
            .unwrap();
    }

    #[test]
    fn al_cambiar_de_modelo_se_suelta_el_otro_y_no_antes() {
        let (r, qwen, gemma, _) = par();
        let (q, g) = (qwen.clone(), gemma.clone());
        let mq = r.motor("qwen", move || q);
        let mg = r.motor("gemma", move || g);
        pedir(&mq);
        pedir(&mq);
        // Con el mismo modelo no se suelta nada (no se recarga).
        assert_eq!(qwen.soltado.load(Ordering::SeqCst), 0);
        assert_eq!(r.ultimo().as_deref(), Some("qwen"));
        pedir(&mg);
        assert_eq!(qwen.soltado.load(Ordering::SeqCst), 1, "Qwen fuera antes de cargar Gemma");
        assert_eq!(gemma.soltado.load(Ordering::SeqCst), 0);
        pedir(&mq);
        assert_eq!(gemma.soltado.load(Ordering::SeqCst), 1);
        // Pedir el mismo id otra vez da el mismo motor, no uno nuevo.
        let _ = r.motor("qwen", || panic!("no debe crear otro"));
        assert_eq!(r.cuantos(), 2);
        r.quitar("gemma");
        assert_eq!(r.cuantos(), 1);
        assert_eq!(gemma.soltado.load(Ordering::SeqCst), 2);
    }

    /// Precargar (al abrir el chat «Asistente») tambien releva: el otro
    /// modelo se suelta antes de cargar este.
    #[test]
    fn precargar_tambien_suelta_al_otro() {
        let (r, qwen, gemma, _) = par();
        let (q, g) = (qwen.clone(), gemma.clone());
        let mq = r.motor("qwen", move || q);
        let mg = r.motor("gemma", move || g);
        pedir(&mg);
        mq.precalentar();
        assert_eq!(gemma.soltado.load(Ordering::SeqCst), 1);
        assert_eq!(r.ultimo().as_deref(), Some("qwen"));
        mq.precalentar();
        assert_eq!(qwen.soltado.load(Ordering::SeqCst), 0, "precargar el mismo no lo recarga");
    }

    #[test]
    fn nunca_contestan_dos_a_la_vez() {
        let (r, qwen, gemma, maximo) = par();
        let mq = r.motor("qwen", move || qwen);
        let mg = r.motor("gemma", move || gemma);
        let hilos: Vec<_> = (0..6)
            .map(|i| {
                let m = if i % 2 == 0 { mq.clone() } else { mg.clone() };
                std::thread::spawn(move || pedir(&m))
            })
            .collect();
        for h in hilos {
            h.join().unwrap();
        }
        assert_eq!(maximo.load(Ordering::SeqCst), 1);
    }
}
