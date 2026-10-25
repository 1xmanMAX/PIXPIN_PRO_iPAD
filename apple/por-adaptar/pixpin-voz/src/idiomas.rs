//! **Que modelo hace falta, donde se pone y si ya esta.**
//!
//! El movil baja el modelo solo la primera vez
//! (`MotorVosk.asegurarModelo`, `MotorVosk.kt:60-100`). **Aqui no se baja
//! nada**, y es a proposito: son 40 MB por idioma de un servidor ajeno, y
//! una aplicacion de escritorio que se pone a descargar sin que se lo pidan
//! es exactamente lo que este proyecto no quiere ser. Este modulo dice
//! **que falta, donde va y de donde se saca**; quien pulsa «descargar» es
//! el usuario, con su navegador.
//!
//! Todo aqui es texto y rutas: se prueba sin motor y sin sonido.

use std::path::{Path, PathBuf};

/// Los modelos pequenos de Vosk, por idioma. Son **los mismos** que baja el
/// movil (`MODELOS`, `MotorVosk.kt:32-40`), version incluida: el mismo
/// audio tiene que dar el mismo texto en los dos aparatos.
pub const IDIOMAS: [(&str, &str); 7] = [
    ("es", "vosk-model-small-es-0.42"),
    ("en", "vosk-model-small-en-us-0.15"),
    ("pt", "vosk-model-small-pt-0.3"),
    ("fr", "vosk-model-small-fr-0.22"),
    ("de", "vosk-model-small-de-0.15"),
    ("it", "vosk-model-small-it-0.22"),
    ("ca", "vosk-model-small-ca-0.4"),
];

/// De donde salen. `DESCARGAS` (`MotorVosk.kt:41`).
pub const DESCARGAS: &str = "https://alphacephei.com/vosk/models/";

/// Como se llama la libreria del reconocedor.
pub const NOMBRE_DEL_MOTOR: &str = "libvosk.dll";

/// La parte de idioma de una etiqueta: `es-PE` → `es`, `ES` → `es`.
pub fn lengua_de(idioma: &str) -> String {
    idioma.split(['-', '_']).next().unwrap_or("").to_lowercase()
}

/// El nombre del modelo para un idioma, o `None` si no hay ninguno.
///
/// **Aqui se sale del movil a sabiendas.** `MotorVosk.modeloPara` (:48)
/// devuelve el de castellano ante un idioma desconocido; en un telefono, con
/// una sola pantalla y un solo idioma, pasa inadvertido, pero en el PC eso
/// seria transcribir una nota en japones con el modelo espanol y guardar el
/// disparate en el cuaderno, que luego **viaja por la sincronizacion**.
/// Mejor decir que no hay modelo.
pub fn modelo_de(idioma: &str) -> Option<&'static str> {
    let lengua = lengua_de(idioma);
    IDIOMAS
        .iter()
        .find(|(l, _)| *l == lengua)
        .map(|(_, modelo)| *modelo)
}

/// El enlace directo del `.zip` del modelo, para bajarlo con el navegador.
///
/// `enlaceDelModelo` (`MotorVosk.kt:103`). Es lo que se le ensena al
/// usuario cuando falta.
pub fn enlace_del_modelo(idioma: &str) -> Option<String> {
    modelo_de(idioma).map(|m| format!("{DESCARGAS}{m}.zip"))
}

/// Donde viven el motor y los modelos: `<datos>/vosk/`.
///
/// Mismo nombre de carpeta que en el movil (`filesDir/vosk`), para que
/// explicarlo sea igual en los dos sitios.
pub fn carpeta_de_modelos(raiz: &Path) -> PathBuf {
    raiz.join("vosk")
}

/// Si una carpeta tiene pinta de ser un modelo de Vosk de verdad.
///
/// Se mira `am` y `conf`, que es lo que lleva cualquier modelo de Kaldi. El
/// movil comprueba lo mismo al importar un zip a mano
/// (`MotorVosk.instalarZip`, :123). Sirve para no intentar cargar una
/// carpeta a medio descomprimir y que el reconocedor se caiga por dentro.
pub fn parece_modelo(dir: &Path) -> bool {
    dir.join("am").is_dir() && dir.join("conf").is_dir()
}

/// La carpeta del modelo de ese idioma, si esta descomprimida.
///
/// Vale el nombre exacto del modelo recomendado y **tambien** cualquier
/// otra version del mismo idioma (`vosk-model-small-es-0.3`, o un modelo
/// grande `vosk-model-es-0.42`): el usuario descomprime lo que se baje, y
/// negarse porque el numero de version no cuadra al decimal seria pedantico.
pub fn buscar_modelo(raiz: &Path, idioma: &str) -> Option<PathBuf> {
    let modelo = modelo_de(idioma)?;
    let carpeta = carpeta_de_modelos(raiz);

    let exacta = carpeta.join(modelo);
    if parece_modelo(&exacta) {
        return Some(exacta);
    }

    // `vosk-model-small-es-0.42` → `vosk-model-small-es`. El prefijo
    // incluye el idioma, asi que `-es` no casa con `-en-us`.
    let prefijo = modelo.rsplit_once('-').map(|(p, _)| p).unwrap_or(modelo);
    let mut candidatas: Vec<PathBuf> = std::fs::read_dir(&carpeta)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefijo))
                && parece_modelo(p)
        })
        .collect();
    // En orden, para que dos versiones descomprimidas den siempre la misma
    // y el texto no cambie entre dos aperturas.
    candidatas.sort();
    candidatas.pop()
}

/// La libreria del reconocedor, si esta puesta.
///
/// Primero en `<datos>/vosk/`, que es donde se le dice al usuario que la
/// ponga, y despues junto al ejecutable, por si alguien la reparte con la
/// aplicacion. Devolver `None` no es un fallo: es el caso normal mientras
/// el usuario no la haya instalado.
pub fn buscar_motor(raiz: &Path, junto_al_exe: Option<&Path>) -> Option<PathBuf> {
    let candidatas = [
        carpeta_de_modelos(raiz).join(NOMBRE_DEL_MOTOR),
        raiz.join(NOMBRE_DEL_MOTOR),
    ];
    for c in candidatas {
        if c.is_file() {
            return Some(c);
        }
    }
    let junto = junto_al_exe?.join(NOMBRE_DEL_MOTOR);
    junto.is_file().then_some(junto)
}

/// Lo que se puede hacer hoy con ese idioma. Es lo que mira el chat antes
/// de ofrecer «Pasar a texto»: ofrecerse para algo que luego no se hace es
/// peor que no ofrecerse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disponibilidad {
    /// Todo puesto. Trae la carpeta del modelo ya resuelta.
    Listo(PathBuf),
    /// Falta `libvosk.dll`.
    SinMotor,
    /// Esta el motor pero falta el modelo del idioma. Trae el nombre de la
    /// carpeta que hay que descomprimir y el enlace de donde sacarla.
    SinModelo {
        modelo: &'static str,
        enlace: String,
    },
    /// Vosk no tiene modelo pequeno para ese idioma.
    IdiomaSinModelo,
}

/// Que falta, si falta algo.
pub fn disponibilidad(raiz: &Path, junto_al_exe: Option<&Path>, idioma: &str) -> Disponibilidad {
    let Some(modelo) = modelo_de(idioma) else {
        return Disponibilidad::IdiomaSinModelo;
    };
    if buscar_motor(raiz, junto_al_exe).is_none() {
        return Disponibilidad::SinMotor;
    }
    match buscar_modelo(raiz, idioma) {
        Some(carpeta) => Disponibilidad::Listo(carpeta),
        None => Disponibilidad::SinModelo {
            modelo,
            enlace: format!("{DESCARGAS}{modelo}.zip"),
        },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una carpeta de usar y tirar. No se trae `tempfile` para esto: son
    /// cuatro lineas y el crate no tiene ninguna dependencia de pruebas.
    struct Prestada(PathBuf);

    impl Prestada {
        fn nueva(nombre: &str) -> Prestada {
            let p = std::env::temp_dir().join(format!(
                "pixpin-voz-{nombre}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).expect("se puede escribir en el temporal");
            Prestada(p)
        }

        /// Deja una carpeta con pinta de modelo dentro de `<raiz>/vosk/`.
        fn con_modelo(&self, nombre: &str) {
            let d = carpeta_de_modelos(&self.0).join(nombre);
            std::fs::create_dir_all(d.join("am")).unwrap();
            std::fs::create_dir_all(d.join("conf")).unwrap();
        }

        fn con_motor(&self) {
            let c = carpeta_de_modelos(&self.0);
            std::fs::create_dir_all(&c).unwrap();
            std::fs::write(c.join(NOMBRE_DEL_MOTOR), b"no es una dll de verdad").unwrap();
        }
    }

    impl Drop for Prestada {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn la_etiqueta_del_sistema_se_queda_en_su_idioma() {
        assert_eq!(lengua_de("es-PE"), "es");
        assert_eq!(lengua_de("es_419"), "es");
        assert_eq!(lengua_de("EN-us"), "en");
        assert_eq!(lengua_de(""), "");
    }

    #[test]
    fn cada_idioma_conocido_tiene_su_modelo_y_su_enlace() {
        assert_eq!(modelo_de("es-ES"), Some("vosk-model-small-es-0.42"));
        assert_eq!(modelo_de("en"), Some("vosk-model-small-en-us-0.15"));
        assert_eq!(
            enlace_del_modelo("es").as_deref(),
            Some("https://alphacephei.com/vosk/models/vosk-model-small-es-0.42.zip")
        );
    }

    #[test]
    fn un_idioma_sin_modelo_no_se_transcribe_con_el_castellano() {
        // El movil devolveria el espanol aqui; ver el comentario de
        // `modelo_de`.
        assert_eq!(modelo_de("ja"), None);
        assert_eq!(modelo_de("zh-CN"), None);
        assert_eq!(enlace_del_modelo("ja"), None);
    }

    #[test]
    fn sin_motor_instalado_no_se_ofrece_pasar_a_texto() {
        let t = Prestada::nueva("sin-motor");
        t.con_modelo("vosk-model-small-es-0.42");
        assert_eq!(disponibilidad(&t.0, None, "es"), Disponibilidad::SinMotor);
    }

    #[test]
    fn con_motor_pero_sin_el_modelo_del_idioma_se_dice_cual_falta() {
        let t = Prestada::nueva("sin-modelo");
        t.con_motor();
        t.con_modelo("vosk-model-small-es-0.42");
        match disponibilidad(&t.0, None, "fr") {
            Disponibilidad::SinModelo { modelo, enlace } => {
                assert_eq!(modelo, "vosk-model-small-fr-0.22");
                assert!(enlace.ends_with("vosk-model-small-fr-0.22.zip"));
            }
            otra => panic!("se esperaba que faltara el modelo frances, y salio {otra:?}"),
        }
    }

    #[test]
    fn un_idioma_que_vosk_no_cubre_se_dice_aparte_de_que_falte_algo() {
        let t = Prestada::nueva("idioma-raro");
        t.con_motor();
        assert_eq!(
            disponibilidad(&t.0, None, "ja"),
            Disponibilidad::IdiomaSinModelo
        );
    }

    #[test]
    fn con_todo_puesto_se_devuelve_la_carpeta_del_modelo() {
        let t = Prestada::nueva("listo");
        t.con_motor();
        t.con_modelo("vosk-model-small-es-0.42");
        let esperada = carpeta_de_modelos(&t.0).join("vosk-model-small-es-0.42");
        assert_eq!(
            disponibilidad(&t.0, None, "es"),
            Disponibilidad::Listo(esperada)
        );
    }

    #[test]
    fn vale_otra_version_del_mismo_idioma_pero_no_la_de_otro() {
        let t = Prestada::nueva("otra-version");
        t.con_modelo("vosk-model-small-es-0.3");
        t.con_modelo("vosk-model-small-en-us-0.15");
        assert_eq!(
            buscar_modelo(&t.0, "es"),
            Some(carpeta_de_modelos(&t.0).join("vosk-model-small-es-0.3"))
        );
        assert_eq!(buscar_modelo(&t.0, "fr"), None);
    }

    #[test]
    fn una_carpeta_a_medio_descomprimir_no_cuenta_como_modelo() {
        let t = Prestada::nueva("a-medias");
        let d = carpeta_de_modelos(&t.0).join("vosk-model-small-es-0.42");
        std::fs::create_dir_all(d.join("am")).unwrap();
        assert!(!parece_modelo(&d));
        assert_eq!(buscar_modelo(&t.0, "es"), None);
    }
}
