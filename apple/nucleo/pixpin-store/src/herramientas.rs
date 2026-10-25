//! **Que herramientas de dibujo salen** (`[herramientas]` del TOML).
//!
//! Las herramientas de dibujo son las mismas en el lienzo, el lector, los
//! pines y el anotador de pantalla (ver
//! `docs/superpowers/specs/2026-09-24-herramientas-unicas-design.md`), y el
//! usuario pidio poder apagar algunas desde los ajustes generales. Una
//! apagada desaparece de la barra y de su tecla en TODOS esos sitios.
//!
//! Se guarda la lista de las APAGADAS y no una casilla por herramienta:
//!
//! - una herramienta que se anada manana nace activa sin tocar el fichero de
//!   nadie (con casillas, un fichero viejo no la nombraria y habria que
//!   decidir que significa su ausencia);
//! - un nombre que no se conoce (de una version mas nueva, o mal escrito) se
//!   ignora en vez de romper la carga de los ajustes.
//!
//! Los nombres son los estables de [`NOMBRES`]; el titulo que ve el usuario
//! sale de `herramientas-<nombre>` en los `.ftl`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Los nombres estables de las herramientas que se pueden apagar. La ventana
/// de ajustes no sigue este orden: las ensena por los grupos de la barra.
///
/// La mano no esta: sin ella no se puede ni elegir lo dibujado para moverlo
/// o borrarlo, y apagarla dejaria cualquier anfitrion a medias. Deshacer,
/// rehacer y salir tampoco, que no son herramientas sino acciones.
pub const NOMBRES: &[&str] = &[
    "lazo",
    "bolita",
    "rectangulo",
    "rombo",
    "elipse",
    "flecha",
    "flecha-codos",
    "flecha-libre",
    "linea",
    "lapiz",
    "grafito",
    "texto",
    "imagen",
    "borrador",
    "laser",
    "resaltador",
    "foco",
    "lupa",
    "mosaico",
    "arco",
    "angulo",
    "serie",
    "punto",
    "cota",
    "escalar",
    "escala-grafica",
    "marco",
    "zona",
    "relleno",
    "recortar",
    "extender",
    "nudo",
    "copiar-estilo",
    "figuras",
    // Las dos que sacan el dibujo del lienzo (el grupo «sacar» de la barra).
    "imprimir",
    "compartir",
];

/// Seccion `[herramientas]`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Herramientas {
    /// Los nombres de las apagadas. Vacia de fabrica: todas activas.
    pub apagadas: Vec<String>,
    /// Las apagadas en UN sitio (`SITIOS`): el usuario queria elegir que
    /// herramientas salen en cada situacion «para que no se muestren siempre
    /// todas y saturen». `[herramientas.por_sitio]` con `pin = ["mosaico"]`.
    /// Se suman a `apagadas`, que sigue valiendo para todos los sitios.
    pub por_sitio: BTreeMap<String, Vec<String>>,
}

/// Los sitios con barra propia, en el orden de la ventana de ajustes.
pub const SITIOS: &[&str] = &["pantalla", "pin", "lienzo", "lector"];

fn esta(lista: &[String], nombre: &str) -> bool {
    lista.iter().any(|a| a.trim().eq_ignore_ascii_case(nombre))
}

fn poner_en_lista(lista: &mut Vec<String>, nombre: &str, activa: bool) {
    lista.retain(|a| !a.trim().eq_ignore_ascii_case(nombre));
    if !activa {
        lista.push(nombre.to_string());
    }
}

impl Herramientas {
    /// Si la herramienta de ese nombre sale. Sin distinguir mayusculas: el
    /// fichero lo escribe a veces una persona.
    pub fn activa(&self, nombre: &str) -> bool {
        !esta(&self.apagadas, nombre)
    }

    /// Enciende o apaga una. Apagar dos veces no la repite en la lista, y
    /// encender una que no estaba apagada no hace nada.
    pub fn poner(&mut self, nombre: &str, activa: bool) {
        poner_en_lista(&mut self.apagadas, nombre, activa);
    }

    /// Si sale en `sitio`: ni apagada en todos ni apagada ahi.
    pub fn activa_en(&self, sitio: &str, nombre: &str) -> bool {
        self.activa(nombre) && !self.por_sitio.get(sitio).is_some_and(|l| esta(l, nombre))
    }

    /// Enciende o apaga una en `sitio` solo. Un sitio que se queda sin
    /// apagadas desaparece del fichero.
    pub fn poner_en(&mut self, sitio: &str, nombre: &str, activa: bool) {
        let lista = self.por_sitio.entry(sitio.to_string()).or_default();
        poner_en_lista(lista, nombre, activa);
        if lista.is_empty() {
            self.por_sitio.remove(sitio);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn apagar_en_un_sitio_no_la_quita_de_los_demas_y_va_y_vuelve_por_el_toml() {
        let mut h = Herramientas::default();
        h.poner_en("pin", "mosaico", false);
        assert!(!h.activa_en("pin", "mosaico"));
        assert!(h.activa_en("lienzo", "mosaico"));
        assert!(h.activa("mosaico"), "la lista general no se toca");
        let a = crate::Ajustes {
            herramientas: h.clone(),
            ..Default::default()
        };
        let vuelta: crate::Ajustes = toml::from_str(&toml::to_string_pretty(&a).unwrap()).unwrap();
        assert_eq!(vuelta.herramientas, h);
        h.poner_en("pin", "mosaico", true);
        assert!(h.por_sitio.is_empty(), "un sitio sin apagadas desaparece");
    }

    #[test]
    fn apagada_en_todos_tampoco_sale_en_ningun_sitio() {
        let mut h = Herramientas::default();
        h.poner("lazo", false);
        for s in SITIOS {
            assert!(!h.activa_en(s, "lazo"), "{s}");
        }
    }

    #[test]
    fn de_fabrica_estan_todas_activas() {
        let h = Herramientas::default();
        for n in NOMBRES {
            assert!(h.activa(n), "{n}");
        }
    }

    #[test]
    fn apagar_una_no_apaga_las_demas_y_encenderla_la_devuelve() {
        let mut h = Herramientas::default();
        h.poner("lazo", false);
        assert!(!h.activa("lazo"));
        assert!(h.activa("lapiz"));
        h.poner("lazo", false);
        assert_eq!(h.apagadas.len(), 1, "apagar dos veces no la repite");
        h.poner("lazo", true);
        assert!(h.activa("lazo"));
        assert!(h.apagadas.is_empty());
    }

    #[test]
    fn un_nombre_escrito_a_mano_con_mayusculas_o_espacios_vale_igual() {
        let h = Herramientas {
            apagadas: vec![" Grafito ".into()],
            ..Default::default()
        };
        assert!(!h.activa("grafito"));
    }

    #[test]
    fn un_nombre_desconocido_no_rompe_nada_ni_apaga_nada() {
        let a: crate::Ajustes =
            toml::from_str("[herramientas]\napagadas = [\"teletransporte\"]").unwrap();
        for n in NOMBRES {
            assert!(a.herramientas.activa(n), "{n}");
        }
    }

    #[test]
    fn la_seccion_va_y_vuelve_por_el_toml() {
        let mut a = crate::Ajustes::default();
        a.herramientas.poner("mosaico", false);
        a.herramientas.poner("cota", false);
        let texto = toml::to_string_pretty(&a).unwrap();
        let vuelta: crate::Ajustes = toml::from_str(&texto).unwrap();
        assert_eq!(vuelta.herramientas, a.herramientas);
        // Un fichero de antes, sin la seccion, sale con todas activas.
        let viejo: crate::Ajustes = toml::from_str("idioma = \"espanol\"").unwrap_or_default();
        assert!(viejo.herramientas.apagadas.is_empty());
    }

    #[test]
    fn los_nombres_no_se_repiten() {
        let mut v: Vec<&str> = NOMBRES.to_vec();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), NOMBRES.len());
    }
}
