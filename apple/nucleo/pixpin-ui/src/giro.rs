//! **El giro del boton de sincronizar** mientras hay una vuelta en marcha
//! (25-oct-2026, el usuario: «mientras se sincroniza, que el boton gire»).
//!
//! Logica pura: cuando empezo, que angulo toca ahora y cuando hay que volver
//! a pintar. Quien pinta solo gira el icono ese angulo. Quieto no pide
//! ningun repintado: sin vuelta en marcha, el boton no gasta nada.

/// Lo que tarda una vuelta entera del icono.
pub const VUELTA_MS: u64 = 1000;
/// Cada cuanto se repinta mientras gira: treinta veces por segundo se ve
/// suave y no le quita el procesador a la sincronizacion.
pub const PASO_MS: u32 = 33;

/// Si gira y desde cuando (en ms de cualquier reloj que solo avance).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Giro {
    desde: Option<u64>,
}

impl Giro {
    /// Lo pone a girar o lo para. Devuelve si cambio (hay que pintar).
    pub fn poner(&mut self, en_marcha: bool, ahora: u64) -> bool {
        match (en_marcha, self.desde) {
            (true, None) => {
                self.desde = Some(ahora);
                true
            }
            (false, Some(_)) => {
                self.desde = None;
                true
            }
            _ => false,
        }
    }

    pub fn gira(&self) -> bool {
        self.desde.is_some()
    }

    /// El angulo de ahora, en radianes y en el sentido de las agujas del
    /// reloj; quieto, cero (el icono como siempre).
    pub fn angulo(&self, ahora: u64) -> f32 {
        match self.desde {
            None => 0.0,
            Some(d) => {
                let t = ahora.saturating_sub(d) % VUELTA_MS;
                t as f32 / VUELTA_MS as f32 * std::f32::consts::TAU
            }
        }
    }

    /// Cuando volver a pintar: solo mientras gira.
    pub fn ms_hasta(&self) -> Option<u32> {
        self.desde.map(|_| PASO_MS)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn quieto_no_gira_ni_pide_repintar() {
        let g = Giro::default();
        assert!(!g.gira());
        assert_eq!(g.angulo(12345), 0.0);
        assert_eq!(g.ms_hasta(), None);
    }

    #[test]
    fn gira_una_vuelta_por_segundo_desde_que_empieza() {
        let mut g = Giro::default();
        assert!(g.poner(true, 1000));
        assert!(!g.poner(true, 1200), "seguir girando no es un cambio");
        assert_eq!(g.angulo(1000), 0.0);
        let cuarto = g.angulo(1000 + VUELTA_MS / 4);
        assert!((cuarto - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
        // A la vuelta entera, de nuevo al principio.
        assert!(g.angulo(1000 + VUELTA_MS) < 1e-3);
        assert_eq!(g.ms_hasta(), Some(PASO_MS));
    }

    #[test]
    fn al_acabar_se_para_y_vuelve_a_cero() {
        let mut g = Giro::default();
        g.poner(true, 0);
        assert!(g.poner(false, 500));
        assert!(!g.poner(false, 600));
        assert_eq!(g.angulo(700), 0.0);
        assert_eq!(g.ms_hasta(), None);
    }
}
