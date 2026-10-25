//! **Lo reciente del buscador**: las ultimas busquedas (las fichas de
//! arriba, con su ✕ y «Borrar todas») y lo ultimo que se abrio desde aqui
//! («Abierto hace poco»). Es `buscar_todo/recientes.rs` de Windows.
//!
//! Vive en `<raiz>/buscar-recientes.json`, solo en este equipo: no se
//! sincroniza (es como el historial de un navegador). Un fichero roto o que
//! no esta es una lista vacia, nunca un error. Lo abierto se guarda como el
//! resultado entero ([`ResultadoBusqueda`]): con eso se vuelve a ensenar y a
//! abrir igual.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[cfg(test)]
use super::ClaseDeResultado;
use super::ResultadoBusqueda;

/// Cuantas busquedas se recuerdan.
pub const BUSQUEDAS: usize = 8;
/// Cuantas cosas abiertas se recuerdan.
pub const ABIERTOS: usize = 6;

pub const NOMBRE: &str = "buscar-recientes.json";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Recientes {
    #[serde(default)]
    pub busquedas: Vec<String>,
    /// Lo de Windows (`Abierto`, de su plugin) no se entiende aqui: se
    /// guarda aparte y se ignora al leer.
    #[serde(default, rename = "abiertos_mac")]
    pub abiertos: Vec<ResultadoBusqueda>,
    /// Lo que no se entiende (los `abiertos` de Windows si el fichero
    /// viniera de alli), tal cual: guardar no lo pierde.
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

impl Recientes {
    pub fn ruta(raiz: &Path) -> PathBuf {
        raiz.join(NOMBRE)
    }

    pub fn leer(raiz: &Path) -> Recientes {
        std::fs::read_to_string(Self::ruta(raiz))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn guardar(&self, raiz: &Path) -> std::io::Result<()> {
        let t = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let ruta = Self::ruta(raiz);
        let tmp = ruta.with_extension("json.tmp");
        std::fs::write(&tmp, t)?;
        std::fs::rename(&tmp, &ruta)
    }

    /// Apunta una busqueda, la primera. Se guarda limpia (sin blancos de mas)
    /// y una sola vez, sin mirar mayusculas.
    pub fn apuntar_busqueda(&mut self, consulta: &str) {
        let c = consulta.split_whitespace().collect::<Vec<_>>().join(" ");
        // Una letra sola no es una busqueda que valga la pena recordar.
        if c.chars().count() < 2 {
            return;
        }
        let baja = c.to_lowercase();
        self.busquedas.retain(|b| b.to_lowercase() != baja);
        self.busquedas.insert(0, c);
        self.busquedas.truncate(BUSQUEDAS);
    }

    pub fn quitar_busqueda(&mut self, i: usize) {
        if i < self.busquedas.len() {
            self.busquedas.remove(i);
        }
    }

    /// Apunta lo abierto, lo primero, sin repetirlo.
    pub fn apuntar_abierto(&mut self, r: &ResultadoBusqueda) {
        if !vale_como_abierto(r) {
            return;
        }
        self.abiertos.retain(|b| b.clave != r.clave);
        self.abiertos.insert(0, r.clone());
        self.abiertos.truncate(ABIERTOS);
    }
}

/// Si un resultado elegido se recuerda en «Abierto hace poco»: lo que es una
/// cosa con su clave. (Las acciones de una vez, como apuntar una tarea, las
/// hace Swift y no llegan aqui.)
pub fn vale_como_abierto(r: &ResultadoBusqueda) -> bool {
    !r.clave.is_empty()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-buscar-recientes-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn captura(n: &str) -> ResultadoBusqueda {
        let mut r = ResultadoBusqueda::de(ClaseDeResultado::Captura, n);
        r.ruta = Some(format!("/capturas/{n}"));
        r.clave = format!("captura/{n}");
        r
    }

    #[test]
    fn las_busquedas_van_primero_sin_repetirse_y_con_tope() {
        let mut r = Recientes::default();
        r.apuntar_busqueda("grieta");
        r.apuntar_busqueda("  factura   temu ");
        r.apuntar_busqueda("GRIETA");
        assert_eq!(r.busquedas, vec!["GRIETA", "factura temu"]);
        for i in 0..20 {
            r.apuntar_busqueda(&format!("cosa {i}"));
        }
        assert_eq!(r.busquedas.len(), BUSQUEDAS);
        assert_eq!(r.busquedas[0], "cosa 19");
        r.quitar_busqueda(0);
        assert_eq!(r.busquedas[0], "cosa 18");
        // Caso negativo: una letra sola o vacio no se apunta; quitar fuera
        // de rango no rompe.
        let antes = r.busquedas.clone();
        r.apuntar_busqueda("t");
        r.apuntar_busqueda("   ");
        r.quitar_busqueda(99);
        assert_eq!(r.busquedas, antes);
    }

    #[test]
    fn lo_abierto_vuelve_igual_tras_guardar_y_leer() {
        let raiz = temporal("ida-vuelta");
        let mut r = Recientes::default();
        r.apuntar_abierto(&captura("a.png"));
        r.apuntar_abierto(&captura("b.png"));
        r.apuntar_abierto(&captura("a.png"));
        r.apuntar_busqueda("muro");
        r.guardar(&raiz).unwrap();
        let leido = Recientes::leer(&raiz);
        assert_eq!(leido, r);
        let titulos: Vec<String> = leido.abiertos.iter().map(|a| a.titulo.clone()).collect();
        assert_eq!(
            titulos,
            vec!["a.png", "b.png"],
            "sin repetir y lo ultimo primero"
        );
        assert_eq!(leido.abiertos[0].ruta.as_deref(), Some("/capturas/a.png"));
        for i in 0..10 {
            r.apuntar_abierto(&captura(&format!("{i}.png")));
        }
        assert_eq!(r.abiertos.len(), ABIERTOS);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn caso_negativo_un_fichero_roto_es_una_lista_vacia_y_lo_de_windows_se_respeta() {
        let raiz = temporal("roto");
        std::fs::write(Recientes::ruta(&raiz), "{ esto no es json").unwrap();
        assert_eq!(Recientes::leer(&raiz), Recientes::default());
        // El de Windows: sus busquedas valen, sus abiertos se guardan tal cual.
        std::fs::write(
            Recientes::ruta(&raiz),
            r#"{"busquedas":["grieta"],"abiertos":[{"titulo":"x","accion":{}}]}"#,
        )
        .unwrap();
        let r = Recientes::leer(&raiz);
        assert_eq!(r.busquedas, vec!["grieta"]);
        assert!(r.abiertos.is_empty());
        r.guardar(&raiz).unwrap();
        let texto = std::fs::read_to_string(Recientes::ruta(&raiz)).unwrap();
        assert!(texto.contains("\"accion\""), "{texto}");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn lo_que_no_tiene_clave_no_se_recuerda_como_abierto() {
        let mut r = Recientes::default();
        let sin = ResultadoBusqueda::de(ClaseDeResultado::Chat, "x");
        r.apuntar_abierto(&sin);
        assert!(r.abiertos.is_empty());
        // Y lo que si es una cosa, si.
        r.apuntar_abierto(&captura("c.png"));
        assert_eq!(r.abiertos.len(), 1);
    }
}
