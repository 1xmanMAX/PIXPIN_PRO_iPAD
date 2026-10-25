//! Un contador: un numero que sube y baja de uno en uno, o de lo que se diga.
//! Puerto de `mini/Contador.kt` de Android.
//!
//! ## Por que merece ser una mini-app
//!
//! Porque contar a mano es justo lo que se hace mal. Cajas descargadas,
//! viajes de un camion, personas que entran, series de un ejercicio: son
//! cosas que se cuentan mientras se hace otra cosa, y ahi la cabeza pierde el
//! numero en cuanto alguien pregunta la hora. Un boton grande y un numero
//! grande es toda la aplicacion.
//!
//! ## El paso
//!
//! De uno casi siempre, pero se guarda **con el contador** y no en los
//! ajustes: quien cuenta cajas de doce quiere que ese contador suba de doce, y
//! el de al lado, de uno. Un ajuste general obligaria a cambiarlo cada vez que
//! se pasa de una cuenta a la otra.

use super::{Resumen, documento_de_claves, otras_claves, valor, valores};

/// Mas alla de esto no es una cuenta, es un desbordamiento esperando.
pub const TOPE: i64 = 1_000_000_000;

/// Un contador y su paso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cuenta {
    pub valor: i64,
    /// Cuanto sube o baja cada toque. Nunca cero: un paso de cero no cuenta.
    pub paso: i64,
    /// Las claves que este PC no entiende, para devolverlas al guardar.
    pub otros: Vec<(String, String)>,
}

impl Default for Cuenta {
    fn default() -> Self {
        Self {
            valor: 0,
            paso: 1,
            otros: Vec::new(),
        }
    }
}

const CLAVES: [&str; 2] = ["valor", "paso"];

pub fn leer(documento: &str) -> Cuenta {
    let v = valores(documento);
    Cuenta {
        valor: valor(&v, "valor")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
            .clamp(-TOPE, TOPE),
        // Un paso de cero o negativo dejaria un contador que no cuenta o que
        // baja al sumar. Se corrige **al leer**, no al escribir: el documento
        // puede venir tocado a mano, y quien lo lee es quien tiene que
        // aguantar lo que encuentre.
        paso: valor(&v, "paso")
            .and_then(|s| s.parse::<i64>().ok())
            .filter(|p| *p > 0)
            .unwrap_or(1),
        otros: otras_claves(&v, &CLAVES),
    }
}

pub fn escribir(titulo: &str, c: &Cuenta) -> String {
    let mut pares = vec![
        ("valor".to_string(), c.valor.to_string()),
        ("paso".to_string(), c.paso.to_string()),
    ];
    pares.extend(c.otros.iter().cloned());
    documento_de_claves(titulo, &pares)
}

pub fn mas(c: &Cuenta) -> Cuenta {
    Cuenta {
        valor: c.valor.saturating_add(c.paso).clamp(-TOPE, TOPE),
        ..c.clone()
    }
}

pub fn menos(c: &Cuenta) -> Cuenta {
    Cuenta {
        valor: c.valor.saturating_sub(c.paso).clamp(-TOPE, TOPE),
        ..c.clone()
    }
}

/// Reiniciar deja el paso como estaba: lo que se pone a cero es la cuenta.
pub fn reiniciar(c: &Cuenta) -> Cuenta {
    Cuenta {
        valor: 0,
        ..c.clone()
    }
}

pub fn con_paso(c: &Cuenta, paso: i64) -> Cuenta {
    Cuenta {
        paso: paso.max(1),
        ..c.clone()
    }
}

/// El numero, que es toda la mini-app.
pub fn resumen(documento: &str) -> Resumen {
    let c = leer(documento);
    Resumen {
        texto: c.valor.to_string(),
        vacia: c.valor == 0,
        ..Resumen::default()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_documento_del_contador_se_escribe_como_en_el_movil() {
        let c = Cuenta {
            valor: 144,
            paso: 12,
            otros: Vec::new(),
        };
        let d = escribir("Cajas descargadas", &c);
        assert_eq!(d, "# Cajas descargadas\n\n- valor: 144\n- paso: 12");
        assert_eq!(leer(&d), c, "ida y vuelta");
    }

    #[test]
    fn nace_a_cero_y_de_uno_en_uno() {
        let d = escribir("C", &Cuenta::default());
        assert_eq!(d, "# C\n\n- valor: 0\n- paso: 1");
        assert!(resumen(&d).vacia);
    }

    #[test]
    fn sube_y_baja_con_su_paso_y_no_se_desborda() {
        let c = con_paso(&Cuenta::default(), 12);
        let c = mas(&mas(&c));
        assert_eq!(c.valor, 24);
        assert_eq!(menos(&c).valor, 12);
        assert_eq!(
            reiniciar(&c),
            con_paso(&Cuenta::default(), 12),
            "el paso se queda"
        );
        // El tope no se pasa ni sumando desde el borde.
        let alto = Cuenta {
            valor: TOPE,
            paso: TOPE,
            otros: Vec::new(),
        };
        assert_eq!(mas(&alto).valor, TOPE);
        assert_eq!(menos(&alto).valor, 0);
    }

    #[test]
    fn un_documento_tocado_a_mano_no_deja_un_contador_que_no_cuenta() {
        let c = leer("# C\n\n- valor: 5\n- paso: 0");
        assert_eq!(c.paso, 1, "un paso de cero no contaria nada");
        assert_eq!(leer("- valor: 5\n- paso: -3").paso, 1);
        assert_eq!(con_paso(&Cuenta::default(), 0).paso, 1);
        // Y un valor imposible se acota en vez de caerse.
        assert_eq!(leer("- valor: 99999999999999").valor, TOPE);
        assert_eq!(leer("- valor: hola").valor, 0);
    }

    #[test]
    fn una_clave_de_una_version_futura_sobrevive_a_tocar_el_boton() {
        let c = leer("# C\n\n- valor: 3\n- paso: 1\n- color: rojo");
        assert_eq!(c.otros, vec![("color".to_string(), "rojo".to_string())]);
        let d = escribir("C", &mas(&c));
        assert_eq!(d, "# C\n\n- valor: 4\n- paso: 1\n- color: rojo");
    }

    #[test]
    fn una_clave_repetida_dice_lo_mismo_que_en_kotlin() {
        // En Kotlin estas lineas acaban en un `toMap()`: manda la ultima.
        assert_eq!(leer("- valor: 1\n- valor: 9").valor, 9);
    }
}
