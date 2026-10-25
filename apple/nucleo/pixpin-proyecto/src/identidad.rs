//! Quien es este equipo: su id, su nombre y su codigo de aparato.
//!
//! El mismo fichero y el mismo formato que PixPin Android
//! (`sincro/Identidad.kt`, `IdentidadEnDisco`): `sincro/identidad.json` bajo
//! la raiz de datos. Android lo dejo escrito para «llevarlo tal cual a la
//! version de escritorio», y es lo que hace que un proyecto nacido aqui
//! lleve el codigo de este equipo (`K7Q2`) como el de cualquier telefono.
//!
//! El id se crea al azar la primera vez y no cambia nunca: renombrar el
//! equipo no puede cambiar los codigos de lo que ya viajo.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::codigos;

/// Un aparato (`Aparato` de Android).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Aparato {
    /// Al azar al instalar. Nunca se ensena.
    pub id: String,
    /// El que pone el usuario. Es lo que se ve al sincronizar.
    pub nombre: String,
    /// La letra dentro de un grupo, o nada.
    pub letra: Option<String>,
    /// Cuando entro al grupo (milisegundos desde 1970).
    pub desde: i64,
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

impl Aparato {
    /// Los cuatro signos fijos de este aparato (`Aparato.codigo`).
    pub fn codigo(&self) -> String {
        codigos::de_aparato(&self.id)
    }
}

/// `Identidad` de Android: este aparato, el grupo y sus miembros.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Identidad {
    pub yo: Aparato,
    /// El codigo del grupo tal como se teclea, o nada.
    pub codigo: Option<String>,
    pub miembros: Vec<Aparato>,
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

pub fn ruta(raiz: &Path) -> PathBuf {
    raiz.join("sincro").join("identidad.json")
}

/// Un UUID v4 en texto, como `UUID.randomUUID()` de Java. La entropia es la
/// de `codigos::nuevo` (claves aleatorias del proceso, hora y contador),
/// pasada por SHA-256.
fn uuid_v4() -> String {
    let a = codigos::sha256(codigos::nuevo().as_bytes());
    let b = codigos::sha256(codigos::nuevo().as_bytes());
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a[..8]);
    bytes[8..].copy_from_slice(&b[..8]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let h: String = bytes.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

impl Identidad {
    /// Lee la identidad de `raiz`, o la crea y la guarda si no hay (o si el
    /// fichero no se entiende: sin identidad no se puede sellar nada).
    pub fn leer_o_crear(raiz: &Path, nombre_por_omision: &str) -> std::io::Result<Identidad> {
        let fichero = ruta(raiz);
        if let Some(i) = std::fs::read_to_string(&fichero)
            .ok()
            .and_then(|t| serde_json::from_str::<Identidad>(&t).ok())
            .filter(|i| !i.yo.id.is_empty())
        {
            return Ok(i);
        }
        let nueva = Identidad {
            yo: Aparato {
                id: uuid_v4(),
                nombre: nombre_por_omision.to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        nueva.guardar(raiz)?;
        Ok(nueva)
    }

    /// Temporal y renombrado, como Android: un corte a mitad no deja un
    /// fichero a medias que haria cambiar de identidad al arrancar.
    pub fn guardar(&self, raiz: &Path) -> std::io::Result<()> {
        let fichero = ruta(raiz);
        let carpeta = fichero.parent().unwrap_or(raiz);
        std::fs::create_dir_all(carpeta)?;
        let tmp = carpeta.join("identidad.json.tmp");
        let texto = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, texto)?;
        std::fs::rename(&tmp, &fichero)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-identidad-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn la_identidad_se_crea_una_vez_y_despues_se_lee_la_misma() {
        let raiz = carpeta("una-vez");
        let a = Identidad::leer_o_crear(&raiz, "MAXBOOK").unwrap();
        let b = Identidad::leer_o_crear(&raiz, "OTRO NOMBRE").unwrap();
        assert_eq!(a, b, "arrancar otra vez no cambia de identidad");
        assert_eq!(a.yo.nombre, "MAXBOOK");
        assert_eq!(a.yo.codigo().len(), 4);
        assert!(ruta(&raiz).exists());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn lee_la_identidad_que_escribe_android_y_da_su_mismo_codigo() {
        let raiz = carpeta("android");
        std::fs::create_dir_all(raiz.join("sincro")).unwrap();
        std::fs::write(
            ruta(&raiz),
            r#"{"yo":{"id":"aparato-demo","nombre":"Max phone","letra":"a","desde":5},"codigo":"K7Q2ABCDEF","miembros":[{"id":"x","nombre":"Tablet"}],"futuro":true}"#,
        )
        .unwrap();
        let i = Identidad::leer_o_crear(&raiz, "no se usa").unwrap();
        assert_eq!(i.yo.codigo(), "9FMQ", "el mismo codigo que en el telefono");
        assert_eq!(i.miembros.len(), 1);
        // Guardar no pierde lo que no se entiende.
        i.guardar(&raiz).unwrap();
        let texto = std::fs::read_to_string(ruta(&raiz)).unwrap();
        assert!(texto.contains("futuro"));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_id_nuevo_es_un_uuid_v4_valido() {
        let u = uuid_v4();
        assert_eq!(u.len(), 36);
        assert_eq!(&u[14..15], "4");
        assert!(matches!(&u[19..20], "8" | "9" | "a" | "b"), "{u}");
        // Caso negativo: dos seguidos no se repiten.
        assert_ne!(uuid_v4(), uuid_v4());
    }
}
