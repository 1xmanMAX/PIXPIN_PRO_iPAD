//! **La llamada secreta** (tanda E5): una nota de voz con hora suena como si
//! llamaran y, al contestar, se oye bajito. Lo de `llamada.rs` de Windows
//! que no es la ventana (esa es de Swift: `VozVentanas/LlamadaSecreta.swift`).
//!
//! **No hay campo nuevo**: es `clase = VOZ` con `recuerdaEn`, lo mismo que
//! en el movil (`LlamadaSecretaActivity.kt`) y en Windows. Al sonar un
//! recordatorio, el vigia de Swift pregunta [`Nucleo::llamada_secreta`]: si
//! el mensaje es una nota de voz y su audio esta en este Mac, suena como
//! una llamada; si no, sale como un recordatorio de siempre.
//!
//! **Quien «llama»** lo guarda el movil en sus preferencias
//! (`llamada_secreta`, clave `quien:<mensaje>`), en el aparato y fuera del
//! mensaje: no viaja. Windows y el Mac igual, en un fichero junto a los
//! datos con las mismas claves ([`FICHERO_QUIEN`]).

use std::path::{Path, PathBuf};

use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{Clase, Cuaderno, Mensaje};

use crate::chats::validar_chat;
use crate::{ErrorNucleo, Nucleo};

/// El fichero de los nombres de quien llama, en la raiz de los datos.
pub const FICHERO_QUIEN: &str = "llamada_secreta.json";

/// Una llamada que sonar: el audio, quien «llama» y de que mensaje es (para
/// volver a llamar mas tarde).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LlamadaSecreta {
    /// La ruta absoluta del audio en este Mac.
    pub audio: String,
    /// Quien llama: el nombre apuntado o el de la nota sin extension (vacio
    /// si no tiene; quien pinta pone «Llamada»).
    pub nombre: String,
    pub chat: String,
    pub mensaje: String,
}

fn clave_quien(mensaje: &str) -> String {
    format!("quien:{mensaje}")
}

fn leer_quien(raiz: &Path) -> serde_json::Map<String, serde_json::Value> {
    std::fs::read_to_string(raiz.join(FICHERO_QUIEN))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// El nombre que el usuario le puso a la llamada de la nota `mensaje`, o
/// `None` si no le puso ninguno (entonces sale el nombre de la nota).
pub fn quien_llama(raiz: &Path, mensaje: &str) -> Option<String> {
    leer_quien(raiz)
        .get(&clave_quien(mensaje))?
        .as_str()
        .filter(|t| !t.trim().is_empty())
        .map(str::to_string)
}

/// Apunta (o, en blanco, olvida) quien llama en la nota `mensaje`
/// (`ponerQuienLlama`). Va por un temporal: un corte a medias no puede dejar
/// los nombres de todas las llamadas en un fichero roto.
pub fn poner_quien_llama(raiz: &Path, mensaje: &str, nombre: &str) -> std::io::Result<()> {
    let mut todos = leer_quien(raiz);
    let nombre = nombre.trim();
    if nombre.is_empty() {
        todos.remove(&clave_quien(mensaje));
    } else {
        todos.insert(clave_quien(mensaje), serde_json::Value::from(nombre));
    }
    std::fs::create_dir_all(raiz)?;
    let destino = raiz.join(FICHERO_QUIEN);
    let temporal = destino.with_extension("json.tmp");
    std::fs::write(&temporal, serde_json::to_vec_pretty(&todos)?)?;
    std::fs::rename(&temporal, &destino)
}

/// «Quien llama»: el nombre del fichero sin extension
/// (`nombre.substringBeforeLast('.')`), o vacio si no tiene.
pub fn nombre_de_la_llamada(nombre: &str) -> String {
    match nombre.rsplit_once('.') {
        Some((antes, _)) => antes.to_string(),
        None => nombre.to_string(),
    }
}

/// Quien llama en `m`: el que se le puso al programarla y, si no, el nombre
/// de la nota sin extension.
pub fn quien_llama_en(raiz: &Path, m: &Mensaje) -> String {
    quien_llama(raiz, &m.id).unwrap_or_else(|| nombre_de_la_llamada(&m.nombre))
}

/// **Si un recordatorio vencido es una llamada secreta**: el mensaje `id`
/// de `c` es una nota de voz y su audio esta en este aparato. `audio`
/// resuelve la ruta del mensaje (como el chat, tambien las `pixpin:files/…`).
pub fn de_un_cuaderno(
    c: &Cuaderno,
    raiz: &Path,
    chat: &str,
    id: &str,
    audio: impl Fn(&str) -> Option<PathBuf>,
) -> Option<LlamadaSecreta> {
    let m = c.mensajes.iter().find(|m| m.id == id)?;
    if m.clase != Some(Clase::Voz) {
        return None;
    }
    let ruta = audio(m.ruta.as_deref()?)?;
    if !ruta.is_file() {
        return None;
    }
    Some(LlamadaSecreta {
        audio: ruta.display().to_string(),
        nombre: quien_llama_en(raiz, m),
        chat: chat.to_string(),
        mensaje: m.id.clone(),
    })
}

#[uniffi::export]
impl Nucleo {
    /// **La llamada secreta del mensaje `mensaje`**, si lo es: una nota de
    /// voz cuyo audio esta en este Mac. `None` si no (sale como un
    /// recordatorio de siempre). Bloqueante (lee el cuaderno).
    pub fn llamada_secreta(
        &self,
        chat: String,
        mensaje: String,
    ) -> Result<Option<LlamadaSecreta>, ErrorNucleo> {
        validar_chat(&chat)?;
        let carpeta = almacen::carpeta(self.ruta(), &chat);
        let c = match Cuaderno::leer_de(&carpeta) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        Ok(de_un_cuaderno(&c, self.ruta(), &chat, &mensaje, |r| {
            self.resolver(&chat, r)
        }))
    }

    /// El nombre apuntado de quien llama en la nota `mensaje`, o `None` si
    /// no se le puso ninguno. Lee un fichero pequeno.
    pub fn quien_llama(&self, mensaje: String) -> Option<String> {
        quien_llama(self.ruta(), &mensaje)
    }

    /// Apunta quien llama en la nota `mensaje`; en blanco, lo olvida y vuelve
    /// el nombre de la nota. No viaja (como en el movil).
    pub fn poner_quien_llama(&self, mensaje: String, nombre: String) -> Result<(), ErrorNucleo> {
        poner_quien_llama(self.ruta(), &mensaje, &nombre)?;
        Ok(())
    }
}

/// «Quien llama» por defecto de una nota: su nombre sin extension.
#[uniffi::export]
pub fn nombre_de_llamada(nombre: String) -> String {
    nombre_de_la_llamada(&nombre)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una conversacion propia de cada prueba: el fichero de quien llama
    /// vive en la raiz y no puede pisarse entre pruebas.
    fn raiz_propia(etiqueta: &str) -> PathBuf {
        let raiz = std::env::temp_dir().join(format!(
            "pixpin-nucleo-llamada-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(raiz.join("proyectos").join("p1").join("archivos")).unwrap();
        raiz
    }

    fn voz(id: &str, nombre: &str) -> Mensaje {
        Mensaje {
            id: id.into(),
            clase: Some(Clase::Voz),
            ruta: Some("archivos/recado.m4a".into()),
            nombre: nombre.into(),
            ..Mensaje::default()
        }
    }

    fn audio(raiz: &Path) -> impl Fn(&str) -> Option<PathBuf> + '_ {
        move |r| Some(raiz.join("proyectos").join("p1").join(r))
    }

    #[test]
    fn una_nota_de_voz_con_su_audio_es_una_llamada_con_el_nombre_sin_extension() {
        let raiz = raiz_propia("es");
        std::fs::write(raiz.join("proyectos/p1/archivos/recado.m4a"), b"x").unwrap();
        let cu = Cuaderno {
            mensajes: vec![voz("m1", "recado.m4a")],
            ..Cuaderno::default()
        };
        let l = de_un_cuaderno(&cu, &raiz, "p1", "m1", audio(&raiz)).expect("es una llamada");
        assert_eq!(l.nombre, "recado");
        assert_eq!(
            l.audio,
            raiz.join("proyectos/p1/archivos/recado.m4a")
                .display()
                .to_string()
        );
        assert_eq!((l.chat.as_str(), l.mensaje.as_str()), ("p1", "m1"));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn una_nota_escrita_o_una_voz_sin_su_audio_no_son_llamadas() {
        let raiz = raiz_propia("no");
        let cu = Cuaderno {
            mensajes: vec![
                Mensaje {
                    id: "texto".into(),
                    clase: Some(Clase::Nota),
                    texto: "a las diez".into(),
                    ..Mensaje::default()
                },
                Mensaje {
                    id: "sin-audio".into(),
                    clase: Some(Clase::Voz),
                    ruta: Some("archivos/no-esta.m4a".into()),
                    ..Mensaje::default()
                },
            ],
            ..Cuaderno::default()
        };
        assert!(de_un_cuaderno(&cu, &raiz, "p1", "texto", audio(&raiz)).is_none());
        assert!(
            de_un_cuaderno(&cu, &raiz, "p1", "sin-audio", audio(&raiz)).is_none(),
            "el audio no llego a este equipo"
        );
        assert!(de_un_cuaderno(&cu, &raiz, "p1", "no-existe", audio(&raiz)).is_none());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_nombre_de_quien_llama_es_el_del_fichero_sin_extension() {
        assert_eq!(nombre_de_la_llamada("recado.largo.m4a"), "recado.largo");
        assert_eq!(nombre_de_la_llamada("sin_extension"), "sin_extension");
        assert_eq!(nombre_de_la_llamada(""), "");
    }

    #[test]
    fn quien_llama_es_el_nombre_que_se_le_puso_y_si_no_el_de_la_nota() {
        let raiz = raiz_propia("quien");
        std::fs::write(raiz.join("proyectos/p1/archivos/recado.m4a"), b"x").unwrap();
        let cu = Cuaderno {
            mensajes: vec![voz("m1", "recado.m4a"), voz("m2", "otra.m4a")],
            ..Cuaderno::default()
        };
        assert_eq!(quien_llama(&raiz, "m1"), None, "sin poner, nada");
        poner_quien_llama(&raiz, "m1", "  Mama  ").unwrap();
        assert_eq!(quien_llama(&raiz, "m1").as_deref(), Some("Mama"));
        let l = de_un_cuaderno(&cu, &raiz, "p1", "m1", audio(&raiz)).unwrap();
        assert_eq!(l.nombre, "Mama");
        // Caso negativo: la otra nota sigue con su nombre.
        assert_eq!(
            de_un_cuaderno(&cu, &raiz, "p1", "m2", audio(&raiz))
                .unwrap()
                .nombre,
            "otra"
        );
        // En blanco se olvida y vuelve el de la nota.
        poner_quien_llama(&raiz, "m1", "   ").unwrap();
        assert_eq!(quien_llama(&raiz, "m1"), None);
        assert_eq!(
            de_un_cuaderno(&cu, &raiz, "p1", "m1", audio(&raiz))
                .unwrap()
                .nombre,
            "recado"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_fichero_de_nombres_roto_se_lee_como_si_no_hubiera_ninguno() {
        let raiz = raiz_propia("roto");
        std::fs::write(raiz.join(FICHERO_QUIEN), b"{no es json").unwrap();
        assert_eq!(quien_llama(&raiz, "m1"), None);
        // Y poner uno lo deja sano otra vez.
        poner_quien_llama(&raiz, "m1", "Jefe").unwrap();
        assert_eq!(quien_llama(&raiz, "m1").as_deref(), Some("Jefe"));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_fichero_lleva_las_claves_del_movil() {
        let raiz = raiz_propia("claves");
        poner_quien_llama(&raiz, "m7", "Pepe").unwrap();
        let t = std::fs::read_to_string(raiz.join(FICHERO_QUIEN)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&t).unwrap();
        assert_eq!(v["quien:m7"], "Pepe");
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
