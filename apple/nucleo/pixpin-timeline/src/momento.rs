//! **Un momento** del timeline: cuando paso, su titulo, lo que se conto y lo
//! que lo acompana (fotos, el audio de lo dicho).
//!
//! Se guarda como JSON de una linea. Los campos vacios no se escriben, para
//! que el fichero de un dia de notas cortas se lea a simple vista.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Momento {
    /// Unico y estable: `tl-<ms>-<n>`.
    pub id: String,
    /// Milisegundos desde 1970, en UTC (como las horas del chat). Se pinta
    /// corrido al huso de quien mira.
    pub cuando: i64,
    pub titulo: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub descripcion: String,
    /// Los nombres de las fotos en la carpeta de medios, en orden.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fotos: Vec<String>,
    /// El nombre del audio de lo dicho, si se dicto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(default, rename = "duracionMs", skip_serializing_if = "es_cero")]
    pub duracion_ms: i64,
    /// Lo dicho tal cual salio de la transcripcion, antes de partirlo: si el
    /// reparto sale mal, aqui esta el original.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dicho: Option<String>,
    /// El id de la leccion aprendida que se hizo de este momento con el
    /// boton «Leccion» (5-oct-2026): el usuario pidio juntar timeline y
    /// lecciones, y que una tarjeta pase a leccion con un solo boton y se
    /// quede marcada. La leccion vive en su almacen de siempre (viaja al
    /// movil y entra en el repaso); aqui solo se recuerda cual es.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leccion: Option<String>,
    /// La leccion que tuvo y se desvinculo (desmarcar no la borra). Si se
    /// vuelve a marcar, se reutiliza esa en vez de crear otra: un revisor
    /// vio que marcar, desmarcar y marcar dejaba dos lecciones iguales.
    #[serde(default, rename = "leccionPrevia", skip_serializing_if = "Option::is_none")]
    pub leccion_previa: Option<String>,
    /// Mandado a «Momentos» (8-oct-2026, el usuario: «que vaya a momentos
    /// solo lo de hoy que me parecio relevante y decida mandar a momentos»).
    /// «Hoy» lo ensena todo; «Momentos», solo lo que lleva esto.
    #[serde(default, rename = "enMomentos", skip_serializing_if = "no")]
    pub en_momentos: bool,
}

fn no(v: &bool) -> bool {
    !*v
}

fn es_cero(v: &i64) -> bool {
    *v == 0
}

impl Momento {
    /// Un momento nuevo, sin nada mas que su texto.
    pub fn nuevo(id: String, cuando: i64, titulo: String, descripcion: String) -> Momento {
        Momento {
            id,
            cuando,
            titulo,
            descripcion,
            fotos: Vec::new(),
            audio: None,
            duracion_ms: 0,
            dicho: None,
            leccion: None,
            leccion_previa: None,
            en_momentos: false,
        }
    }

    /// Su linea en el fichero.
    pub fn a_linea(&self) -> String {
        // Un struct sin mapas ni flotantes no puede fallar al escribirse.
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Lee una linea del fichero. `None` si no es un momento: una linea
    /// rota (un corte de luz a mitad de escribir) no tira las demas.
    pub fn de_linea(linea: &str) -> Option<Momento> {
        let l = linea.trim();
        if l.is_empty() {
            return None;
        }
        serde_json::from_str(l).ok()
    }

    /// Si no lleva nada que ensenar.
    pub fn vacio(&self) -> bool {
        self.titulo.trim().is_empty()
            && self.descripcion.trim().is_empty()
            && self.fotos.is_empty()
            && self.audio.is_none()
    }
}

/// Un id nuevo para el momento creado en `cuando`; `n` lo desempata dentro
/// del mismo milisegundo.
pub fn id_nuevo(cuando: i64, n: u32) -> String {
    format!("tl-{cuando}-{n}")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_momento_va_y_vuelve_por_su_linea() {
        let mut m = Momento::nuevo("tl-1-0".into(), 1_000, "Titulo".into(), "Desc".into());
        m.fotos.push("tl-1-0-1.png".into());
        m.audio = Some("tl-1-0.m4a".into());
        m.duracion_ms = 4_200;
        m.dicho = Some("con esto paso titulo".into());
        m.leccion = Some("lec-1".into());
        m.leccion_previa = Some("lec-0".into());
        assert_eq!(Momento::de_linea(&m.a_linea()), Some(m));
    }

    #[test]
    fn los_campos_vacios_no_se_escriben() {
        let m = Momento::nuevo("tl-1-0".into(), 1, "Solo titulo".into(), String::new());
        let l = m.a_linea();
        assert!(!l.contains("descripcion"));
        assert!(!l.contains("fotos"));
        assert!(!l.contains("audio"));
        assert!(!l.contains("duracionMs"));
        assert!(!l.contains("leccion"));
        assert!(!l.contains("leccionPrevia"));
    }

    #[test]
    fn una_linea_rota_no_es_un_momento() {
        assert_eq!(Momento::de_linea("{\"id\":\"tl-1"), None);
        assert_eq!(Momento::de_linea("   "), None);
    }

    #[test]
    fn vacio_solo_si_no_hay_nada() {
        let mut m = Momento::nuevo("a".into(), 0, " ".into(), String::new());
        assert!(m.vacio());
        m.fotos.push("f.png".into());
        assert!(!m.vacio());
    }
}
