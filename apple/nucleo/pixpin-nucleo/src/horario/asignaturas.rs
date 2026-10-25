//! **Cada asignatura es un grupo de tareas** (Windows, 18-oct,
//! `horario/asignaturas.rs`): una lista de tareas con su nombre en el chat
//! «Asignaturas», un chat como cualquier otro (viaja al movil, se ve en el
//! chat y en Tareas). El chat se reconoce por una marca en el indice; si ya
//! habia uno llamado «Asignaturas» sin marca, se adopta. Borrar una
//! asignatura del horario NO borra su lista.

use std::path::Path;

use pixpin_ia::fechas::Momento;
use pixpin_proyecto::almacen;

use crate::tareas::{ListaTareas, frase};
use crate::{ErrorNucleo, Nucleo};

/// Como se llama el chat de las asignaturas.
pub const NOMBRE_CHAT: &str = "Asignaturas";
/// Su marca en el indice (la misma de Windows).
const MARCA: &str = "asignaturas";

/// Minusculas y sin tildes, como `normalizar` de Windows.
fn plegar(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

fn tiene_marca(f: &almacen::Ficha) -> bool {
    f.resto.get(MARCA).and_then(|v| v.as_bool()) == Some(true)
}

/// El chat «Asignaturas», creandolo (o adoptando uno con ese nombre).
fn asegurar_ficha(raiz: &Path, aparato: &str) -> Result<almacen::Ficha, ErrorNucleo> {
    let mut indice = almacen::Indice::leer_si_esta(raiz)?;
    if let Some(f) = indice.proyectos.iter().find(|f| tiene_marca(f)) {
        return Ok(f.clone());
    }
    let f = match indice.proyectos.iter_mut().find(|f| f.nombre == NOMBRE_CHAT) {
        Some(f) => {
            f.resto.insert(MARCA.into(), serde_json::Value::Bool(true));
            f.clone()
        }
        None => {
            let mut f = almacen::Ficha::nueva(NOMBRE_CHAT, crate::ahora_ms(), aparato);
            f.resto.insert(MARCA.into(), serde_json::Value::Bool(true));
            indice.proyectos.push(f.clone());
            f
        }
    };
    indice.guardar(raiz)?;
    Ok(f)
}

#[uniffi::export]
impl Nucleo {
    /// Los nombres de las asignaturas del horario (la lista lateral y las de
    /// las clases), para elegir donde apuntar una tarea.
    pub fn nombres_de_asignaturas(&self, desfase_ms: i64) -> Vec<String> {
        let h = super::de_disco(self.ruta(), Momento::de_ms_locales(crate::ahora_ms() + desfase_ms));
        h.todas_las_asignaturas().into_iter().map(|a| a.nombre).collect()
    }

    /// **Apunta una tarea en la asignatura** `asignatura`: su lista en el
    /// chat «Asignaturas» (los dos se crean si no estan).
    pub fn apuntar_en_asignatura(
        &self,
        asignatura: String,
        texto: String,
        imagenes: Vec<String>,
        desfase_ms: i64,
    ) -> Result<ListaTareas, ErrorNucleo> {
        let nombre = asignatura.trim();
        if nombre.is_empty() || (pixpin_proyecto::mini::saneado(&texto).is_empty() && imagenes.is_empty()) {
            return Err(ErrorNucleo::Argumento { detalle: "falta la asignatura o la tarea".into() });
        }
        let aparato = self.identidad()?.yo.codigo();
        let ficha = asegurar_ficha(self.ruta(), &aparato)?;
        let entendida = frase::leer(&texto, frase::ahora(desfase_ms));
        let texto = if entendida.titulo.is_empty() { texto } else { entendida.texto() };
        self.apuntar_en_lista(&ficha, nombre, texto, &imagenes, desfase_ms)
    }

    /// **La lista de la asignatura** `asignatura` entre `listas` (las de
    /// Tareas): la del chat «Asignaturas» que se llama asi, sin tildes ni
    /// mayusculas (`asignaturas::de` de Windows). No crea nada: `None` si
    /// aun no tiene tareas. Lo que ensena el Horario debajo de la elegida.
    pub fn lista_de_asignatura(&self, listas: Vec<ListaTareas>, asignatura: String) -> Option<ListaTareas> {
        let indice = almacen::Indice::leer_si_esta(self.ruta()).ok()?;
        let chat = indice
            .proyectos
            .iter()
            .find(|f| tiene_marca(f))
            .or_else(|| indice.proyectos.iter().find(|f| f.nombre == NOMBRE_CHAT))?
            .id
            .clone();
        let buscado = plegar(asignatura.trim());
        listas.into_iter().find(|l| l.chat == chat && plegar(l.titulo.trim()) == buscado)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_lista_de_una_asignatura_se_encuentra_sin_tildes_y_solo_en_su_chat() {
        let d = std::env::temp_dir().join(format!("pixpin-asignaturas-lista-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        // Antes de apuntar nada: ni chat ni lista.
        assert_eq!(n.lista_de_asignatura(Vec::new(), "Geometría".into()), None);
        n.apuntar_en_asignatura("Geometría".into(), "repasar el tema 3".into(), Vec::new(), 0).unwrap();
        // Una lista con el mismo titulo en otro chat no es la de la asignatura.
        let otro = n.crear_chat("Otro".into()).unwrap();
        let indice = almacen::Indice::leer_si_esta(n.ruta()).unwrap();
        let ficha = indice.proyectos.iter().find(|f| f.id == otro.id).unwrap().clone();
        n.apuntar_en_lista(&ficha, "Geometria", "otra cosa".into(), &[], 0).unwrap();
        let listas = n.tareas(0).unwrap();
        let l = n.lista_de_asignatura(listas.clone(), "GEOMETRIA".into()).unwrap();
        assert_eq!(l.titulo, "Geometría");
        assert_ne!(l.chat, otro.id);
        assert_eq!(l.tareas.len(), 1);
        assert_eq!(n.lista_de_asignatura(listas, "Física".into()), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
