//! **«Enviar a Anki»** del menu de un mensaje (Windows,
//! `ventana_chat/anki.rs`, 24-oct-2026). Las tarjetas salen del mismo crate
//! que Windows (`pixpin_anki::tarjetas`): de una **nota** («pregunta ::
//! respuesta», «P: / R:» y los huecos `{{c1::…}}`) y de una **foto** (la
//! imagen delante y su descripcion detras).
//!
//! En el iPad no hay AnkiConnect (AnkiMobile no lo tiene y la app no puede
//! hablar con el Anki de otro aparato por `127.0.0.1`), asi que se hace lo
//! que Windows hace sin Anki abierto: el fichero de importar
//! (`pixpin_anki::fichero`), con el mazo `PixPin::<proyecto>` y las
//! etiquetas `pixpin` y la del proyecto. Swift lo comparte (o lo guarda en
//! Archivos) junto con las imagenes.
//!
//! Todo es puro: textos entran, textos salen.

use pixpin_anki::{Tarjeta, fichero, tarjetas};

/// Una tarjeta basica de Anki, en HTML (`pixpin_anki::Tarjeta` sin los
/// bytes de las imagenes: Swift ya tiene el fichero).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TarjetaAnki {
    pub frente: String,
    pub reverso: String,
    pub etiquetas: Vec<String>,
    /// Los nombres de las imagenes que nombran `frente` o `reverso` (como
    /// se guardan en la carpeta de medios de Anki).
    pub imagenes: Vec<String>,
}

impl From<Tarjeta> for TarjetaAnki {
    fn from(t: Tarjeta) -> Self {
        TarjetaAnki {
            frente: t.frente,
            reverso: t.reverso,
            etiquetas: t.etiquetas,
            imagenes: t.imagenes.into_iter().map(|i| i.nombre).collect(),
        }
    }
}

impl From<TarjetaAnki> for Tarjeta {
    fn from(t: TarjetaAnki) -> Self {
        Tarjeta {
            frente: t.frente,
            reverso: t.reverso,
            etiquetas: t.etiquetas,
            imagenes: Vec::new(),
        }
    }
}

/// Las etiquetas de un proyecto: `pixpin` y la del proyecto (sin espacios).
fn etiquetas_de(proyecto: &str) -> Vec<String> {
    let mut v = vec!["pixpin".to_string()];
    let p = tarjetas::etiqueta(proyecto);
    if !p.is_empty() {
        v.push(p);
    }
    v
}

/// **Las tarjetas de una nota** de `proyecto` (vacio si la nota no lleva
/// ninguna: entonces no se ofrece).
#[uniffi::export]
pub fn anki_tarjetas_de_la_nota(texto: String, proyecto: String) -> Vec<TarjetaAnki> {
    let etiquetas = etiquetas_de(&proyecto);
    tarjetas::de_la_nota(&texto)
        .into_iter()
        .map(|mut t| {
            t.etiquetas = etiquetas.clone();
            TarjetaAnki::from(t)
        })
        .collect()
}

/// **La tarjeta de una foto**: delante la imagen (`imagen`, el nombre con
/// que va a Anki); detras su descripcion, o `titulo` si no tiene.
#[uniffi::export]
pub fn anki_tarjeta_de_la_foto(
    imagen: String,
    descripcion: String,
    titulo: String,
    proyecto: String,
) -> TarjetaAnki {
    let mut t = tarjetas::de_la_foto(
        pixpin_anki::Imagen {
            nombre: imagen,
            bytes: Vec::new(),
        },
        &descripcion,
        &titulo,
    );
    t.etiquetas = etiquetas_de(&proyecto);
    TarjetaAnki::from(t)
}

/// El nombre en Anki de la imagen del mensaje `id`: unico y estable (mandar
/// dos veces la misma foto no llena Anki de copias), como Windows.
#[uniffi::export]
pub fn anki_nombre_de_imagen(id: String, ext: String) -> String {
    let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    let ext = if ext.is_empty() { "png".to_string() } else { ext };
    format!("pixpin-{id}.{ext}")
}

/// El mazo de un proyecto: `PixPin::<proyecto>` (`::` separa submazos en
/// Anki: dentro del nombre no).
#[uniffi::export]
pub fn anki_mazo_de(proyecto: String) -> String {
    format!("PixPin::{}", proyecto.trim().replace("::", ":"))
}

/// **El fichero de importar** («Archivo → Importar» de Anki) con esas
/// tarjetas en `mazo`.
#[uniffi::export]
pub fn anki_fichero(mazo: String, tarjetas: Vec<TarjetaAnki>) -> String {
    let t: Vec<Tarjeta> = tarjetas.into_iter().map(Tarjeta::from).collect();
    fichero::texto(&mazo, &t)
}

/// El nombre del fichero de importar de un mazo (sin signos que un disco
/// no admite), como Windows sin la hora.
#[uniffi::export]
pub fn anki_nombre_de_fichero(mazo: String) -> String {
    let nombre: String = mazo
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    format!("{nombre}.txt")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_nota_da_sus_tarjetas_con_las_etiquetas_del_proyecto() {
        let t = anki_tarjetas_de_la_nota(
            "Capital de Peru :: Lima\nUn parrafo suelto.".into(),
            "Clase de geografía".into(),
        );
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].frente, "Capital de Peru");
        assert_eq!(t[0].etiquetas, ["pixpin", "Clase_de_geografía"]);
        // Caso negativo: una nota sin preguntas no da tarjetas.
        assert!(anki_tarjetas_de_la_nota("Hola".into(), "x".into()).is_empty());
    }

    #[test]
    fn una_foto_lleva_su_imagen_delante_y_el_fichero_su_mazo() {
        let nombre = anki_nombre_de_imagen("123".into(), ".JPG".into());
        assert_eq!(nombre, "pixpin-123.jpg");
        let t = anki_tarjeta_de_la_foto(nombre.clone(), "".into(), "Pizarra".into(), "Obra".into());
        assert!(t.frente.contains("pixpin-123.jpg"));
        assert_eq!(t.reverso, "Pizarra");
        assert_eq!(t.imagenes, [nombre]);
        let mazo = anki_mazo_de(" A::B ".into());
        assert_eq!(mazo, "PixPin::A:B");
        let f = anki_fichero(mazo.clone(), vec![t]);
        assert!(f.starts_with("#separator:tab\n#html:true\n#deck:PixPin::A:B\n"), "{f}");
        assert!(f.trim_end().ends_with("pixpin Obra"));
        assert_eq!(anki_nombre_de_fichero(mazo), "PixPin--A-B.txt");
        // Caso negativo: sin extension, png.
        assert_eq!(anki_nombre_de_imagen("9".into(), "".into()), "pixpin-9.png");
    }
}
