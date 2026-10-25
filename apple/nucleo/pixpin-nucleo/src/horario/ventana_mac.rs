//! **La ventana semanal del horario para Swift** (Windows, 18-oct,
//! `horario/ventana.rs`): la lista de asignaturas y los bloques de la
//! semana ([`super::semana`]) en un objeto que Swift edita (soltar,
//! mover, alargar, borrar) y guarda en el MISMO `horario.md`, cambiando
//! solo las lineas de clases y de asignaturas.

use std::sync::{Arc, Mutex};

use pixpin_ia::fechas::Momento;

use super::semana::{self, Semana};
use crate::{ErrorNucleo, Nucleo};

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AsignaturaHorario {
    pub nombre: String,
    pub aula: Option<String>,
    /// `0xRRGGBB`.
    pub color: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BloqueHorario {
    /// 0 lunes … 6 domingo.
    pub dia: u8,
    /// Minutos del dia.
    pub inicio: u32,
    pub fin: u32,
    pub nombre: String,
    pub aula: Option<String>,
    pub color: u32,
}

/// **La ficha de una clase** (Windows: el panel de la clase elegida, con
/// aula, nota y aviso): lo que no sale en [`BloqueHorario`].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FichaDeClase {
    pub aula: Option<String>,
    /// Lo que lee la voz al contestar la llamada (las lineas `>`), una por
    /// linea.
    pub notas: Vec<String>,
    /// Minutos de antes del aviso; `None` es la del horario.
    pub antelacion: Option<u32>,
}

/// La semana abierta en la ventana.
#[derive(uniffi::Object)]
pub struct SemanaHorario {
    nucleo: Arc<Nucleo>,
    semana: Mutex<Semana>,
}

impl SemanaHorario {
    fn tomar(&self) -> std::sync::MutexGuard<'_, Semana> {
        self.semana.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[uniffi::export]
impl SemanaHorario {
    /// Lee el horario (lo crea con la plantilla si no esta).
    #[uniffi::constructor]
    pub fn abrir(nucleo: Arc<Nucleo>) -> Result<Arc<SemanaHorario>, ErrorNucleo> {
        let ruta = nucleo.fichero_del_horario()?;
        let texto = std::fs::read_to_string(&ruta)?;
        let h = super::leer(&texto, Momento::de_ms_locales(crate::ahora_ms()));
        Ok(Arc::new(SemanaHorario { nucleo, semana: Mutex::new(Semana::de_horario(&h)) }))
    }

    pub fn asignaturas(&self) -> Vec<AsignaturaHorario> {
        let s = self.tomar();
        s.asignaturas
            .iter()
            .map(|a| AsignaturaHorario { nombre: a.nombre.clone(), aula: a.aula.clone(), color: s.color_de(&a.nombre) })
            .collect()
    }

    pub fn bloques(&self) -> Vec<BloqueHorario> {
        let s = self.tomar();
        s.bloques
            .iter()
            .map(|b| BloqueHorario {
                dia: b.dia,
                inicio: b.inicio,
                fin: b.fin,
                nombre: b.nombre.clone(),
                aula: b.aula.clone(),
                color: s.color_de(&b.nombre),
            })
            .collect()
    }

    /// Una asignatura nueva («Geometría @A-12»). `false`: ya estaba.
    pub fn anadir_asignatura(&self, escrito: String) -> bool {
        matches!(self.tomar().anadir_asignatura(&escrito), Ok(Some(_)))
    }

    pub fn borrar_asignatura(&self, i: u32) {
        self.tomar().borrar_asignatura(i as usize);
    }

    pub fn cambiar_color(&self, i: u32) {
        self.tomar().cambiar_color(i as usize);
    }

    /// Suelta la asignatura `asignatura` en el dia y minuto: un bloque nuevo.
    pub fn soltar(&self, asignatura: u32, dia: u8, minuto: u32) {
        self.tomar().soltar(asignatura as usize, dia, semana::redondear_abajo(minuto));
    }

    pub fn mover(&self, i: u32, dia: u8, inicio: i64) {
        self.tomar().mover(i as usize, dia, inicio);
    }

    pub fn alargar(&self, i: u32, fin: u32) {
        self.tomar().alargar(i as usize, semana::redondear(fin));
    }

    pub fn borrar(&self, i: u32) {
        self.tomar().borrar(i as usize);
    }

    /// La ficha del bloque `i` (aula, notas y aviso), o nada si no esta.
    pub fn ficha_de_clase(&self, i: u32) -> Option<FichaDeClase> {
        let s = self.tomar();
        s.bloques.get(i as usize).map(|b| FichaDeClase {
            aula: b.aula.clone(),
            notas: b.notas.clone(),
            antelacion: b.antelacion,
        })
    }

    /// Cambia la ficha del bloque `i` como la de Windows al guardarla: el
    /// aula vacia es ninguna, las notas sin lineas en blanco y el aviso
    /// como mucho de un dia.
    pub fn poner_ficha_de_clase(&self, i: u32, ficha: FichaDeClase) {
        let mut s = self.tomar();
        let Some(b) = s.bloques.get_mut(i as usize) else {
            return;
        };
        b.aula = ficha.aula.map(|a| a.trim().to_string()).filter(|a| !a.is_empty());
        b.notas = ficha
            .notas
            .iter()
            .flat_map(|n| n.lines())
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        b.antelacion = ficha.antelacion.map(|n| n.min(24 * 60));
    }

    /// Cambia el aula de la asignatura `i` (y de sus clases que tenian la
    /// de antes).
    pub fn poner_aula_de_asignatura(&self, i: u32, aula: Option<String>) {
        let aula = aula.map(|a| a.trim().to_string()).filter(|a| !a.is_empty());
        self.tomar().poner_aula_de_asignatura(i as usize, aula);
    }

    /// Los minutos de antes que avisa una clase sin aviso propio (los del
    /// horario).
    pub fn antelacion_del_horario(&self) -> u32 {
        self.nucleo
            .fichero_del_horario()
            .ok()
            .and_then(|r| std::fs::read_to_string(r).ok())
            .map(|t| super::leer(&t, Momento::de_ms_locales(crate::ahora_ms())).antelacion)
            .unwrap_or(super::ANTELACION)
    }

    /// **Guarda** en `horario.md` (solo las lineas de clases y asignaturas).
    pub fn guardar(&self) -> Result<(), ErrorNucleo> {
        let ruta = self.nucleo.fichero_del_horario()?;
        let original = std::fs::read_to_string(&ruta)?;
        let nuevo = semana::reescribir(&original, &self.tomar());
        if nuevo != original {
            let tmp = format!("{ruta}.parte");
            std::fs::write(&tmp, nuevo)?;
            std::fs::rename(&tmp, &ruta)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn soltar_una_asignatura_la_escribe_en_el_horario_y_avisa() {
        let d = std::env::temp_dir().join(format!("pixpin-semana-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        let s = SemanaHorario::abrir(n.clone()).unwrap();
        assert!(s.anadir_asignatura("Geometría @A-12".into()));
        assert!(!s.anadir_asignatura("geometria".into()), "ya estaba");
        for dia in 0..7 {
            s.soltar(0, dia, 8 * 60 + 7);
        }
        assert_eq!(s.bloques().len(), 7);
        assert_eq!(s.bloques()[0].inicio, 8 * 60, "a los 15 min de abajo");
        s.guardar().unwrap();
        let texto = std::fs::read_to_string(n.fichero_del_horario().unwrap()).unwrap();
        assert!(texto.contains("Geometría"), "{texto}");
        assert!(texto.contains("# Horario de clases"), "lo del usuario se queda");
        assert!(!n.avisos_del_horario(0).is_empty(), "y ya avisa");
        // La ficha de una clase: aula, notas y aviso, y vuelven al leer.
        s.poner_ficha_de_clase(
            1,
            FichaDeClase { aula: Some(" B-3 ".into()), notas: vec!["llevar la escuadra\n\n".into()], antelacion: Some(5000) },
        );
        assert_eq!(
            s.ficha_de_clase(1),
            Some(FichaDeClase { aula: Some("B-3".into()), notas: vec!["llevar la escuadra".into()], antelacion: Some(24 * 60) })
        );
        assert_eq!(s.ficha_de_clase(99), None);
        s.guardar().unwrap();
        let otra = SemanaHorario::abrir(n.clone()).unwrap();
        assert_eq!(otra.ficha_de_clase(1).unwrap().notas, ["llevar la escuadra"]);
        assert!(otra.antelacion_del_horario() > 0);
        // Borrar un bloque.
        s.borrar(0);
        assert_eq!(s.bloques().len(), 6);
        let _ = std::fs::remove_dir_all(&d);
    }
}
