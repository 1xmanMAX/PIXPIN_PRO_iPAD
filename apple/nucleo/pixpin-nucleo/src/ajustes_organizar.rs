//! **Ajustes → Tareas, Lienzo y Pinear** en el mismo `pixpinmax.toml` que
//! Windows (`ventana_ajustes.rs`, `Seccion::Tareas`, `Lienzo` y `Pinear`).
//!
//! Un registro aparte de [`crate::ajustes::AjustesMac`] para no tocar el del
//! Mac (que ya lo usa): las palabras de cada urgencia (`[tareas]`), el iman
//! (`[enganche]`), el suavizado del trazo (`[tinta] suavizado`, con sus dos
//! mandos de Avanzado), que herramientas salen y donde (`[herramientas]` y
//! `[herramientas.por_sitio]`) y las palabras magicas (`[pinear]`).
//!
//! Se guarda igual que `guardar_ajustes`: con `guardar_conservando` (el
//! fichero conserva sus comentarios y lo demas de Windows) y, si el que hay
//! no se entiende, no se guarda.

use std::collections::HashMap;

use pixpin_store::ajustes::{self, Suavizado};

use crate::{ErrorNucleo, Nucleo};

/// Lo de `pixpinmax.toml` de las secciones Tareas, Lienzo y Pinear.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AjustesOrganizar {
    /// Las palabras que hacen urgente una tarea (`[tareas] urgente`).
    pub tareas_urgente: Vec<String>,
    /// Las que la hacen muy urgente (`[tareas] muy_urgente`).
    pub tareas_muy_urgente: Vec<String>,
    /// El iman: al dibujar, el cursor se pega a lo que ya hay.
    pub iman_activo: bool,
    /// Esquinas de las formas y extremos de las lineas.
    pub iman_esquinas: bool,
    pub iman_medios: bool,
    pub iman_centros: bool,
    /// A cuantos pixeles de pantalla agarra (6 a 40).
    pub iman_radio_px: u32,
    /// `excalidraw` o `natural` (`[tinta] suavizado`).
    pub suavizado: String,
    /// Los mandos del filtro de 1 euro (Avanzado de Windows): `None` = los
    /// del motor. Aqui solo se leen; se guardan como esten.
    pub tinta_corte_minimo: Option<f32>,
    pub tinta_beta: Option<f32>,
    /// Las herramientas apagadas en todos los sitios
    /// (`pixpin_store::herramientas::NOMBRES`).
    pub herramientas_apagadas: Vec<String>,
    /// Las apagadas en un sitio solo (`pantalla`, `pin`, `lienzo`, `lector`).
    pub herramientas_por_sitio: HashMap<String, Vec<String>>,
    /// Si mandan las palabras magicas de aqui y no las de fabrica.
    pub palabras_propias: bool,
    /// Una por linea, `palabra=APP` (ver [`crate::magia`]).
    pub palabras_magicas: String,
}

/// Los limites del radio del iman en la ventana de Windows: por debajo de 6
/// no llega a nada; por encima de 40 agarra lo que no se miraba.
pub const RADIO_MINIMO: u32 = 6;
pub const RADIO_MAXIMO: u32 = 40;

impl From<&ajustes::Ajustes> for AjustesOrganizar {
    fn from(a: &ajustes::Ajustes) -> Self {
        AjustesOrganizar {
            tareas_urgente: a.tareas.urgente.clone(),
            tareas_muy_urgente: a.tareas.muy_urgente.clone(),
            iman_activo: a.enganche.activo,
            iman_esquinas: a.enganche.esquinas,
            iman_medios: a.enganche.medios,
            iman_centros: a.enganche.centros,
            iman_radio_px: a.enganche.radio_px.round().clamp(0.0, 1000.0) as u32,
            suavizado: match a.tinta.suavizado {
                Suavizado::Excalidraw => "excalidraw",
                Suavizado::Natural => "natural",
            }
            .into(),
            tinta_corte_minimo: a.tinta.corte_minimo,
            tinta_beta: a.tinta.beta,
            herramientas_apagadas: a.herramientas.apagadas.clone(),
            herramientas_por_sitio: a
                .herramientas
                .por_sitio
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            palabras_propias: a.pinear.palabras_propias,
            palabras_magicas: a.pinear.palabras.clone(),
        }
    }
}

fn fuera(que: &str, valor: impl std::fmt::Debug) -> ErrorNucleo {
    ErrorNucleo::Argumento {
        detalle: format!("{que} fuera de rango: {valor:?}"),
    }
}

/// Una lista de palabras limpia: sin blancos alrededor, sin vacias y sin
/// repetidas (sin mirar mayusculas), en su orden.
fn limpias(v: &[String]) -> Vec<String> {
    let mut sal: Vec<String> = Vec::new();
    for p in v {
        let p = p.trim();
        if !p.is_empty() && !sal.iter().any(|q| q.to_lowercase() == p.to_lowercase()) {
            sal.push(p.to_string());
        }
    }
    sal
}

impl AjustesOrganizar {
    fn aplicar(&self, a: &mut ajustes::Ajustes) -> Result<(), ErrorNucleo> {
        if !(RADIO_MINIMO..=RADIO_MAXIMO).contains(&self.iman_radio_px) {
            return Err(fuera("radio del iman", self.iman_radio_px));
        }
        let suavizado = match self.suavizado.as_str() {
            "excalidraw" => Suavizado::Excalidraw,
            "natural" => Suavizado::Natural,
            otro => return Err(fuera("suavizado", otro)),
        };
        if let Some(s) = self
            .herramientas_por_sitio
            .keys()
            .find(|s| !pixpin_store::herramientas::SITIOS.contains(&s.as_str()))
        {
            return Err(fuera("sitio de las herramientas", s));
        }
        a.tareas.urgente = limpias(&self.tareas_urgente);
        a.tareas.muy_urgente = limpias(&self.tareas_muy_urgente);
        a.enganche.activo = self.iman_activo;
        a.enganche.esquinas = self.iman_esquinas;
        a.enganche.medios = self.iman_medios;
        a.enganche.centros = self.iman_centros;
        a.enganche.radio_px = self.iman_radio_px as f32;
        a.tinta.suavizado = suavizado;
        a.tinta.corte_minimo = self.tinta_corte_minimo.filter(|v| v.is_finite() && *v > 0.0);
        a.tinta.beta = self.tinta_beta.filter(|v| v.is_finite() && *v >= 0.0);
        a.herramientas.apagadas = limpias(&self.herramientas_apagadas);
        // Un sitio que se queda sin apagadas se escribe vacio y no se quita:
        // el guardado que conserva el fichero no borra lo que ya no esta, y
        // la lista vieja volveria a mandar.
        for (sitio, lista) in &self.herramientas_por_sitio {
            a.herramientas.por_sitio.insert(sitio.clone(), limpias(lista));
        }
        a.pinear.palabras_propias = self.palabras_propias;
        a.pinear.palabras = self.palabras_magicas.clone();
        Ok(())
    }

    /// Lo guardado de las palabras magicas como lo leen las funciones de
    /// [`crate::magia`]: `None` es «las de fabrica».
    pub fn palabras_guardadas(&self) -> Option<String> {
        self.palabras_propias.then(|| self.palabras_magicas.clone())
    }
}

#[uniffi::export]
impl Nucleo {
    /// Tareas, Lienzo y Pinear de `pixpinmax.toml` (los de fabrica si no
    /// hay fichero). De paso fija las palabras de urgencia con que se
    /// entienden las tareas.
    pub fn ajustes_organizar(&self) -> Result<AjustesOrganizar, ErrorNucleo> {
        let a = ajustes::cargar(&self.ubicacion())?;
        crate::tareas::frase::fijar(a.tareas.palabras());
        Ok(AjustesOrganizar::from(&a))
    }

    /// Los de fabrica, los de Windows.
    pub fn ajustes_organizar_de_fabrica(&self) -> AjustesOrganizar {
        AjustesOrganizar::from(&ajustes::Ajustes::default())
    }

    /// Guarda Tareas, Lienzo y Pinear sin tocar lo demas del fichero, y
    /// fija las palabras de urgencia nuevas.
    pub fn guardar_ajustes_organizar(&self, nuevos: AjustesOrganizar) -> Result<(), ErrorNucleo> {
        let u = self.ubicacion();
        let mut a = ajustes::cargar(&u)?;
        nuevos.aplicar(&mut a)?;
        ajustes::guardar_conservando(&u, &a)?;
        crate::tareas::frase::fijar(a.tareas.palabras());
        Ok(())
    }

    /// **La herramienta que abre `texto` pineado** con las palabras magicas
    /// de Ajustes → Pinear (`TIMER`, `CHECKLIST`, `DRAW`…), o `None` si es
    /// texto corriente o el fichero no se lee.
    pub fn palabra_magica_de(&self, texto: String) -> Option<String> {
        let a = ajustes::cargar(&self.ubicacion()).ok()?;
        crate::magia::palabra_magica(texto, a.pinear.guardadas().map(str::to_string))
    }
}

/// Si la herramienta `nombre` (de `pixpin_store::herramientas::NOMBRES`)
/// sale en `sitio` con estos ajustes (`sitio` vacio: en todos).
#[uniffi::export]
pub fn herramienta_activa_en(ajustes: AjustesOrganizar, sitio: String, nombre: String) -> bool {
    let esta = |l: &[String]| l.iter().any(|a| a.trim().eq_ignore_ascii_case(&nombre));
    !esta(&ajustes.herramientas_apagadas)
        && (sitio.is_empty() || !ajustes.herramientas_por_sitio.get(&sitio).is_some_and(|l| esta(l)))
}

/// Los nombres de las herramientas que se pueden apagar, en el orden de
/// `pixpin_store::herramientas::NOMBRES`.
#[uniffi::export]
pub fn nombres_de_herramientas() -> Vec<String> {
    pixpin_store::herramientas::NOMBRES.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(n: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-nucleo-organizar-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn va_y_vuelve_con_las_claves_de_windows() {
        let d = temporal("ida");
        let n = Nucleo::abrir(d.display().to_string(), "iPad".into()).unwrap();
        let f = n.ajustes_organizar_de_fabrica();
        assert_eq!(n.ajustes_organizar().unwrap(), f);
        assert!(f.iman_activo);
        assert_eq!(f.iman_radio_px, 14);
        assert_eq!(f.suavizado, "excalidraw");
        assert!(!f.palabras_propias);
        let mut por_sitio = HashMap::new();
        por_sitio.insert("lienzo".to_string(), vec!["mosaico".to_string()]);
        let nuevos = AjustesOrganizar {
            tareas_urgente: vec!["ya".into(), " ".into(), "YA".into()],
            iman_medios: false,
            iman_radio_px: 20,
            suavizado: "natural".into(),
            herramientas_apagadas: vec!["lazo".into()],
            herramientas_por_sitio: por_sitio,
            palabras_propias: true,
            palabras_magicas: "reloj=TIMER".into(),
            ..f.clone()
        };
        n.guardar_ajustes_organizar(nuevos).unwrap();
        let b = n.ajustes_organizar().unwrap();
        assert_eq!(b.tareas_urgente, vec!["ya"]);
        assert!(!b.iman_medios);
        assert_eq!(b.iman_radio_px, 20);
        assert_eq!(b.suavizado, "natural");
        assert!(!herramienta_activa_en(b.clone(), "lienzo".into(), "mosaico".into()));
        assert!(herramienta_activa_en(b.clone(), "pin".into(), "mosaico".into()));
        assert!(!herramienta_activa_en(b.clone(), String::new(), "LAZO".into()));
        assert_eq!(n.palabra_magica_de("reloj".into()).as_deref(), Some("TIMER"));
        assert_eq!(n.palabra_magica_de("timer".into()), None);
        let w = ajustes::cargar(&n.ubicacion()).unwrap();
        assert_eq!(w.tinta.suavizado, Suavizado::Natural);
        assert_eq!(w.enganche.radio_px, 20.0);
        assert!(!w.herramientas.activa_en("lienzo", "mosaico"));
        assert_eq!(w.pinear.guardadas(), Some("reloj=TIMER"));
        // El sitio que se queda sin apagadas no vuelve a mandar.
        let mut vacio = HashMap::new();
        vacio.insert("lienzo".to_string(), Vec::new());
        n.guardar_ajustes_organizar(AjustesOrganizar { herramientas_por_sitio: vacio, ..b }).unwrap();
        let c = n.ajustes_organizar().unwrap();
        assert!(herramienta_activa_en(c.clone(), "lienzo".into(), "mosaico".into()));
    }

    #[test]
    fn lo_fuera_de_rango_no_se_guarda() {
        let d = temporal("rango");
        let n = Nucleo::abrir(d.display().to_string(), "iPad".into()).unwrap();
        let f = n.ajustes_organizar_de_fabrica();
        let mut raro = HashMap::new();
        raro.insert("cocina".to_string(), vec!["lazo".to_string()]);
        for m in [
            AjustesOrganizar { iman_radio_px: 2, ..f.clone() },
            AjustesOrganizar { iman_radio_px: 99, ..f.clone() },
            AjustesOrganizar { suavizado: "seda".into(), ..f.clone() },
            AjustesOrganizar { herramientas_por_sitio: raro, ..f.clone() },
        ] {
            assert!(matches!(n.guardar_ajustes_organizar(m), Err(ErrorNucleo::Argumento { .. })));
        }
        assert!(!d.join("pixpinmax.toml").exists());
    }
}
