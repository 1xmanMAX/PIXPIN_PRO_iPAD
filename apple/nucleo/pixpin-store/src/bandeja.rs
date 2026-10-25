//! **Los favoritos del panel de la bandeja** (rediseno v2): que acciones
//! salen como botones grandes debajo de «Capturar».
//!
//! Viven en el fichero de ajustes, en su propia seccion:
//!
//! ```toml
//! [bandeja]
//! favoritos = ["pinear-portapapeles", "anotar", "pinear-en-vivo"]
//! ```
//!
//! Cada nombre es el nombre estable de un comando (`comandos.rs`) o uno de
//! los de las ventanas de la bandeja (`lecciones`, `tareas`…); quien los
//! entiende es el panel, no este modulo.
//!
//! ## Por que no es un campo de [`crate::Ajustes`]
//!
//! El panel vive en su propio hilo y guarda los favoritos en cuanto se
//! eligen. La ventana de ajustes, en cambio, guarda la copia de `Ajustes`
//! que el hilo principal leyo al arrancar. Si los favoritos fueran un campo,
//! esa copia vieja los pisaria al guardar cualquier otro ajuste. Fuera de la
//! estructura, `guardar_conservando` deja la seccion como esta (lo que el
//! fichero tiene y la estructura no, se queda), y nadie pisa a nadie.

use std::fs;

use crate::ajustes::ErrorAjustes;
use crate::rutas::Ubicacion;

/// Los de fabrica: los tres que la maqueta ensena (pinear lo copiado,
/// anotar la pantalla y el pin en vivo; la voz no tiene comando todavia).
pub const DE_FABRICA: &[&str] = &["pinear-portapapeles", "anotar", "pinear-en-vivo"];

/// Cuantos caben: dos filas de cuatro, una de ellas con «Anadir».
pub const TOPE: usize = 7;

/// Los favoritos que dice `texto` (el fichero de ajustes entero). `None` si
/// no dice nada, que no es lo mismo que una lista vacia: quien los quito
/// todos los quiere sin ninguno, no con los de fabrica.
pub fn de_texto(texto: &str) -> Option<Vec<String>> {
    let doc = texto.parse::<toml_edit::DocumentMut>().ok()?;
    let lista = doc.get("bandeja")?.get("favoritos")?.as_array()?;
    let mut v: Vec<String> = Vec::new();
    for nombre in lista.iter().filter_map(|x| x.as_str()) {
        let nombre = nombre.trim();
        if !nombre.is_empty() && !v.iter().any(|y| y == nombre) {
            v.push(nombre.to_string());
        }
    }
    v.truncate(TOPE);
    Some(v)
}

/// El fichero `texto` con los favoritos puestos, y todo lo demas como
/// estaba (comentarios, orden y claves que no conocemos).
pub fn en_texto(texto: &str, favoritos: &[String]) -> String {
    let mut doc = texto.parse::<toml_edit::DocumentMut>().unwrap_or_default();
    let mut lista = toml_edit::Array::new();
    for f in favoritos.iter().take(TOPE) {
        lista.push(f.as_str());
    }
    if !doc.contains_table("bandeja") {
        doc["bandeja"] = toml_edit::Item::Table(toml_edit::Table::new());
    }
    doc["bandeja"]["favoritos"] = toml_edit::value(lista);
    doc.to_string()
}

/// Lee los favoritos; sin fichero o sin seccion, los de fabrica.
pub fn leer(ubicacion: &Ubicacion) -> Vec<String> {
    fs::read_to_string(ubicacion.fichero_ajustes())
        .ok()
        .and_then(|t| de_texto(&t))
        .unwrap_or_else(|| DE_FABRICA.iter().map(|s| s.to_string()).collect())
}

/// Guarda los favoritos tocando solo `[bandeja] favoritos`.
pub fn guardar(ubicacion: &Ubicacion, favoritos: &[String]) -> Result<(), ErrorAjustes> {
    let ruta = ubicacion.fichero_ajustes();
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre).map_err(|fuente| ErrorAjustes::Escritura {
            ruta: padre.to_path_buf(),
            fuente,
        })?;
    }
    let existente = fs::read_to_string(&ruta).unwrap_or_default();
    fs::write(&ruta, en_texto(&existente, favoritos))
        .map_err(|fuente| ErrorAjustes::Escritura { ruta, fuente })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn v(x: &[&str]) -> Vec<String> {
        x.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn se_guardan_y_se_leen_sin_tocar_lo_demas() {
        let antes = "# mis ajustes\ntema_cosmos = true\n\n[sincro]\npresencia = false # puerta\n";
        let despues = en_texto(antes, &v(&["anotar", "tareas"]));
        assert!(despues.contains("# mis ajustes"), "{despues}");
        assert!(despues.contains("presencia = false # puerta"), "{despues}");
        assert_eq!(de_texto(&despues), Some(v(&["anotar", "tareas"])));
        // Y lo que ya habia sigue leyendose como ajustes.
        let a: crate::Ajustes = toml::from_str(&despues).expect("sigue siendo un TOML de ajustes");
        assert!(a.tema_cosmos && !a.sincro.presencia);
    }

    #[test]
    fn caso_negativo_sin_seccion_no_hay_lista_y_vacia_no_es_de_fabrica() {
        assert_eq!(de_texto("idioma = \"es-ES\"\n"), None);
        assert_eq!(de_texto("esto no es toml = = ="), None);
        let vacia = en_texto("", &[]);
        assert_eq!(
            de_texto(&vacia),
            Some(Vec::new()),
            "quitar todos es una eleccion"
        );
    }

    #[test]
    fn repetidos_y_de_mas_se_quitan() {
        let t = "[bandeja]\nfavoritos = [\"a\", \"a\", \" \", \"b\", \"c\", \"d\", \"e\", \"f\", \"g\", \"h\"]\n";
        assert_eq!(de_texto(t), Some(v(&["a", "b", "c", "d", "e", "f", "g"])));
    }

    #[test]
    fn guardar_conservando_los_ajustes_no_borra_los_favoritos() {
        let dir = std::env::temp_dir().join(format!("pixpin-bandeja-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let u = Ubicacion::Portable { raiz: dir.clone() };
        guardar(&u, &v(&["tareas"])).expect("guardar favoritos");
        crate::ajustes::guardar_conservando(&u, &crate::Ajustes::default()).expect("ajustes");
        assert_eq!(leer(&u), v(&["tareas"]));
        let _ = fs::remove_dir_all(&dir);
    }
}
