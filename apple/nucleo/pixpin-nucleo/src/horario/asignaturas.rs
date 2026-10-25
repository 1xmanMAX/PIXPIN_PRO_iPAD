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
}
