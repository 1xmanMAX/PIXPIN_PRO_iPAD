//! **Donde vive el timeline**: `<datos>/timeline/momentos.jsonl`, un momento
//! por linea, y sus fotos y audios en `<datos>/timeline/medios/`.
//!
//! Apuntar es anadir una linea al final (rapido, y un corte a mitad solo
//! estropea esa linea). Borrar o corregir reescribe el fichero entero por un
//! temporal y un cambio de nombre, como `indice.json`: o queda el viejo o el
//! nuevo, nunca medio. Un ano de notas son pocos cientos de KB.
//!
//! Lo borrado no se lleva sus fotos ni su audio: asi «Deshacer» lo devuelve
//! entero sin copiar nada de vuelta.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::Momento;

#[derive(Debug, Clone)]
pub struct Almacen {
    carpeta: PathBuf,
}

impl Almacen {
    /// El timeline de la carpeta de datos `raiz`.
    pub fn en(raiz: &Path) -> Almacen {
        Almacen {
            carpeta: raiz.join("timeline"),
        }
    }

    pub fn fichero(&self) -> PathBuf {
        self.carpeta.join("momentos.jsonl")
    }

    pub fn medios(&self) -> PathBuf {
        self.carpeta.join("medios")
    }

    /// Donde esta un medio por su nombre.
    pub fn ruta(&self, nombre: &str) -> PathBuf {
        self.medios().join(nombre)
    }

    /// Todos los momentos, del mas viejo al mas nuevo. Un id repetido (dos
    /// aparatos, o una copia a mano) se queda con su ultima linea.
    pub fn leer(&self) -> Vec<Momento> {
        let Ok(texto) = fs::read_to_string(self.fichero()) else {
            return Vec::new();
        };
        let mut v: Vec<Momento> = Vec::new();
        for m in texto.lines().filter_map(Momento::de_linea) {
            match v.iter_mut().find(|x| x.id == m.id) {
                Some(x) => *x = m,
                None => v.push(m),
            }
        }
        v.sort_by_key(|m| m.cuando);
        v
    }

    /// Apunta un momento al final.
    pub fn anadir(&self, m: &Momento) -> std::io::Result<()> {
        fs::create_dir_all(&self.carpeta)?;
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.fichero())?;
        writeln!(f, "{}", m.a_linea())?;
        f.flush()
    }

    /// Reescribe el fichero con `todos` (tras borrar o corregir).
    pub fn guardar_todos(&self, todos: &[Momento]) -> std::io::Result<()> {
        fs::create_dir_all(&self.carpeta)?;
        let mut texto = String::new();
        for m in todos {
            texto.push_str(&m.a_linea());
            texto.push('\n');
        }
        let tmp = self.carpeta.join("momentos.jsonl.tmp");
        fs::write(&tmp, texto)?;
        fs::rename(&tmp, self.fichero())
    }

    /// Copia `origen` a los medios con el nombre `<base>.<su extension>` y
    /// devuelve el nombre.
    pub fn copiar_medio(&self, origen: &Path, base: &str) -> std::io::Result<String> {
        fs::create_dir_all(self.medios())?;
        let ext = origen
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_else(|| "bin".into());
        let nombre = format!("{base}.{ext}");
        fs::copy(origen, self.ruta(&nombre))?;
        Ok(nombre)
    }

    /// Mueve `origen` (un temporal nuestro, como el audio recien grabado) a
    /// los medios; si no se puede mover (otro disco), lo copia.
    pub fn mover_medio(&self, origen: &Path, base: &str) -> std::io::Result<String> {
        fs::create_dir_all(self.medios())?;
        let ext = origen
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_else(|| "bin".into());
        let nombre = format!("{base}.{ext}");
        let destino = self.ruta(&nombre);
        if fs::rename(origen, &destino).is_err() {
            fs::copy(origen, &destino)?;
            let _ = fs::remove_file(origen);
        }
        Ok(nombre)
    }

    /// La huella del fichero (fecha y tamano): si cambia, alguien mas
    /// escribio (otra ventana, un pedido) y hay que releer.
    pub fn huella(&self) -> Option<(SystemTime, u64)> {
        let m = fs::metadata(self.fichero()).ok()?;
        Some((m.modified().ok()?, m.len()))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta(nombre: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("pixpin-timeline-{nombre}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn m(id: &str, cuando: i64) -> Momento {
        Momento::nuevo(id.into(), cuando, format!("t{id}"), String::new())
    }

    #[test]
    fn lo_apuntado_se_lee_en_orden_de_tiempo() {
        let raiz = carpeta("orden");
        let a = Almacen::en(&raiz);
        assert!(a.leer().is_empty(), "sin fichero, nada");
        a.anadir(&m("b", 20)).unwrap();
        a.anadir(&m("a", 10)).unwrap();
        let ids: Vec<String> = a.leer().into_iter().map(|x| x.id).collect();
        assert_eq!(ids, ["a", "b"]);
        let _ = fs::remove_dir_all(raiz);
    }

    #[test]
    fn una_linea_rota_no_se_lleva_las_demas_y_el_id_repetido_gana_la_ultima() {
        let raiz = carpeta("rota");
        let a = Almacen::en(&raiz);
        a.anadir(&m("a", 1)).unwrap();
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(a.fichero())
            .unwrap();
        writeln!(f, "{{\"id\":\"roto").unwrap();
        let mut otra = m("a", 1);
        otra.titulo = "corregido".into();
        a.anadir(&otra).unwrap();
        let v = a.leer();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].titulo, "corregido");
        let _ = fs::remove_dir_all(raiz);
    }

    #[test]
    fn guardar_todos_reescribe_sin_dejar_temporal() {
        let raiz = carpeta("todos");
        let a = Almacen::en(&raiz);
        a.anadir(&m("a", 1)).unwrap();
        a.anadir(&m("b", 2)).unwrap();
        a.guardar_todos(&[m("b", 2)]).unwrap();
        assert_eq!(a.leer().len(), 1);
        assert!(!raiz.join("timeline").join("momentos.jsonl.tmp").exists());
        let _ = fs::remove_dir_all(raiz);
    }

    #[test]
    fn los_medios_se_copian_y_se_mueven_con_su_extension() {
        let raiz = carpeta("medios");
        let a = Almacen::en(&raiz);
        fs::create_dir_all(&raiz).unwrap();
        let foto = raiz.join("Foto.PNG");
        fs::write(&foto, b"png").unwrap();
        let n = a.copiar_medio(&foto, "tl-1-0-1").unwrap();
        assert_eq!(n, "tl-1-0-1.png");
        assert!(foto.exists(), "copiar deja el original");
        assert_eq!(fs::read(a.ruta(&n)).unwrap(), b"png");
        let audio = raiz.join("dictado.m4a");
        fs::write(&audio, b"m4a").unwrap();
        let n = a.mover_medio(&audio, "tl-1-0").unwrap();
        assert_eq!(n, "tl-1-0.m4a");
        assert!(!audio.exists(), "mover se lleva el temporal");
        let _ = fs::remove_dir_all(raiz);
    }
}
