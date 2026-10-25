//! **Ficheros en el chat**: adjuntar uno (soltarlo o pegarlo) y saber donde
//! esta en el disco el fichero de un mensaje o una foto de un lienzo.
//!
//! No se inventa nada: es el mismo camino que Windows al soltar o pegar en
//! un chat (`ventana_chat::adjuntar_con_pie`):
//!
//! 1. el fichero se COPIA dentro del proyecto con
//!    `almacen::guardar_adjunto` (`archivos/<nombre seguro>`, con `(1)` si ya
//!    hay uno, en NFC);
//! 2. la clase sale de la extension con `cuaderno::clase_de_nombre`
//!    (`IMAGEN`, `VOZ` o `ARCHIVO`);
//! 3. el mensaje es `Mensaje::adjunto` con el siguiente numero y el codigo de
//!    este aparato, una linea al final del cuaderno;
//! 4. el chat sube en la lista.
//!
//! Lo unico distinto es que aqui el fichero NO se lee entero en memoria
//! (§4bis del plan: un video de dos gigas no puede pasar por la RAM): se le
//! pide a `guardar_adjunto` el nombre con cero bytes (eso lo reserva en el
//! disco con la misma regla de nombres) y despues se copia encima con
//! `std::fs::copy`, que en APFS es un clon sin copiar datos.
//!
//! Lo que Windows hace despues con un PDF (aligerarlo en su cola) aqui no se
//! hace: es de Swift (ImageIO + PDFKit, ver `apple/README.md`).

use std::path::{Component, Path};
use std::sync::Arc;

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Mensaje, Sello};
use pixpin_proyecto::vista;

use crate::chats::{hora_unica, validar_chat};
use crate::lienzo::{ImagenDelLienzo, Lienzo};
use crate::{ErrorNucleo, MensajeChat, Nucleo};

/// Una ruta de un mensaje que se puede resolver dentro de su chat: ni vacia
/// ni con `..` (viene del cuaderno, que puede venir de otro aparato, y no
/// puede decidir que fichero del disco se abre).
fn ruta_valida(ruta: &str) -> bool {
    !ruta.is_empty()
        && !ruta.contains('\0')
        && Path::new(ruta)
            .components()
            .all(|c| !matches!(c, Component::ParentDir))
        && !ruta.split(['/', '\\']).any(|p| p == "..")
}

impl Nucleo {
    /// La ruta absoluta de `ruta` (la de un mensaje o la de una foto de un
    /// lienzo) dentro de `chat`, igual que Windows (`vista::ruta_real`): lo
    /// relativo cuelga de la carpeta del chat y lo `pixpin:files/...` que
    /// llego del movil, de donde lo dejo la sincronizacion. `None` si no se
    /// puede resolver en este aparato (una ruta absoluta de otro disco).
    pub(crate) fn resolver(&self, chat: &str, ruta: &str) -> Option<std::path::PathBuf> {
        if !ruta_valida(ruta) {
            return None;
        }
        let r = vista::ruta_real(self.ruta(), chat, ruta)?;
        if r.exists() || !ruta.starts_with("pixpin:files/") {
            return Some(r);
        }
        // Una foto de un lienzo hecho en el PC cuya ruta volvio del movil
        // como `pixpin:files/…/imagenes/<id>`: el fichero sigue en
        // `imagenes/<id>` del chat (sin esto, un recuadro gris).
        let nombre = std::path::Path::new(ruta).file_name()?;
        let aqui = pixpin_proyecto::almacen::carpeta(self.ruta(), chat).join("imagenes").join(nombre);
        Some(if aqui.is_file() { aqui } else { r })
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Adjunta un fichero a un chat**: lo copia dentro del proyecto y
    /// escribe su mensaje (`IMAGEN` si es una foto, `VOZ` si es audio,
    /// `ARCHIVO` lo demas), como al soltarlo en el chat de Windows.
    ///
    /// `nombre` es el que se ensena y con el que se guarda; sin el, el del
    /// fichero de origen. Es bloqueante (copia el fichero): Swift lo llama
    /// fuera del hilo principal.
    pub fn adjuntar(
        &self,
        chat: String,
        ruta_origen: String,
        nombre: Option<String>,
    ) -> Result<MensajeChat, ErrorNucleo> {
        self.adjuntar_con(chat, ruta_origen, nombre, |_| {})
    }

    /// **Adjunta un proyecto entero** al chat `chat` (la hoja «+» de
    /// Windows, `AdjProyecto`): no se copia, es la puerta a lo que ya existe
    /// (un mensaje `PROYECTO` con `referencia` = el id del otro chat).
    pub fn adjuntar_proyecto(&self, chat: String, proyecto: String) -> Result<MensajeChat, ErrorNucleo> {
        validar_chat(&chat)?;
        validar_chat(&proyecto)?;
        let raiz = self.ruta();
        let indice = Indice::leer_si_esta(raiz)?;
        if indice.buscar(&chat).is_none() {
            return Err(ErrorNucleo::NoExiste { que: chat });
        }
        let nombre = indice
            .buscar(&proyecto)
            .map(|f| f.nombre.clone())
            .ok_or_else(|| ErrorNucleo::NoExiste { que: proyecto.clone() })?;
        let aparato = self.identidad()?.yo.codigo();
        let carpeta = almacen::carpeta(raiz, &chat);
        let fichero = carpeta.join("guardados.jsonl");
        let mut cache = self.cache_tomada();
        let numero = cache.indice(&fichero)?.map_or(0, |i| i.mayor_numero) + 1;
        let mut m = Mensaje::adjunto(
            cuaderno::Clase::Proyecto,
            &nombre,
            "",
            0,
            &Sello { cuando: hora_unica(), numero, aparato, proyecto: chat.clone() },
        );
        m.ruta = None;
        m.referencia = Some(proyecto);
        cuaderno::anadir(&carpeta, &m)?;
        cache.quitar(&fichero);
        Ok(MensajeChat::from(m))
    }

    /// La ruta absoluta del fichero de un mensaje (su `ruta`) o de una foto
    /// de un lienzo, para ensenarlo o abrirlo. No comprueba que exista:
    /// para eso basta mirar el disco desde Swift.
    pub fn ruta_absoluta(
        &self,
        chat: String,
        ruta_relativa: String,
    ) -> Result<String, ErrorNucleo> {
        validar_chat(&chat)?;
        self.resolver(&chat, &ruta_relativa)
            .map(|r| r.display().to_string())
            .ok_or(ErrorNucleo::Argumento {
                detalle: format!("ruta que no se resuelve en este aparato: {ruta_relativa:?}"),
            })
    }

    /// Las fotos de un lienzo del chat con su ruta **absoluta**, todas en
    /// una llamada (no una por foto). Las que no se pueden resolver en este
    /// aparato no salen.
    pub fn imagenes_del_lienzo(
        &self,
        chat: String,
        lienzo: Arc<Lienzo>,
    ) -> Result<Vec<ImagenDelLienzo>, ErrorNucleo> {
        validar_chat(&chat)?;
        Ok(lienzo
            .imagenes()
            .into_iter()
            .filter_map(|i| {
                let r = self.resolver(&chat, &i.ruta)?;
                Some(ImagenDelLienzo {
                    id_objeto: i.id_objeto,
                    ruta: r.display().to_string(),
                })
            })
            .collect())
    }
}

impl Nucleo {
    /// Lo de [`Nucleo::adjuntar`], con un `retoque` del mensaje ANTES de
    /// escribir su linea (la duracion y los picos de una nota de voz): asi
    /// se escribe una sola linea y no una que luego se reescribe.
    pub(crate) fn adjuntar_con(
        &self,
        chat: String,
        ruta_origen: String,
        nombre: Option<String>,
        retoque: impl FnOnce(&mut Mensaje),
    ) -> Result<MensajeChat, ErrorNucleo> {
        use unicode_normalization::UnicodeNormalization;
        validar_chat(&chat)?;
        let origen = Path::new(&ruta_origen);
        let meta = match std::fs::metadata(origen) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ErrorNucleo::NoExiste { que: ruta_origen });
            }
            Err(e) => return Err(e.into()),
        };
        if !meta.is_file() {
            return Err(ErrorNucleo::Argumento {
                detalle: format!("no es un fichero: {ruta_origen}"),
            });
        }
        // El nombre en NFC: el Finder los da en NFD y Windows y Android en
        // NFC; la misma foto tiene que llamarse igual en todos.
        let nombre: String = nombre
            .filter(|n| !n.trim().is_empty())
            .or_else(|| origen.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "archivo".to_string())
            .nfc()
            .collect();

        let raiz = self.ruta();
        let mut indice = Indice::leer_si_esta(raiz)?;
        if indice.buscar(&chat).is_none() {
            return Err(ErrorNucleo::NoExiste { que: chat });
        }
        let aparato = self.identidad()?.yo.codigo();
        let carpeta = almacen::carpeta(raiz, &chat);
        let cuaderno_fichero = carpeta.join("guardados.jsonl");

        // Reservar el nombre (cero bytes) y copiar encima, sin leerlo entero.
        let relativa = almacen::guardar_adjunto(raiz, &chat, &nombre, &[])?;
        let destino = carpeta.join(&relativa);
        let bytes = match std::fs::copy(origen, &destino) {
            Ok(b) => b,
            Err(e) => {
                let _ = std::fs::remove_file(&destino);
                return Err(e.into());
            }
        };

        // Con la cache tomada de principio a fin, como `escribir_nota`: dos
        // mensajes a la vez no pueden sacar el mismo numero.
        let mut cache = self.cache_tomada();
        let numero = cache
            .indice(&cuaderno_fichero)?
            .map_or(0, |i| i.mayor_numero)
            + 1;
        let cuando = hora_unica();
        let mut m = Mensaje::adjunto(
            cuaderno::clase_de_nombre(&nombre),
            &nombre,
            &relativa,
            i64::try_from(bytes).unwrap_or(i64::MAX),
            &Sello {
                cuando,
                numero,
                aparato,
                proyecto: chat.clone(),
            },
        );
        retoque(&mut m);
        if let Err(e) = cuaderno::anadir(&carpeta, &m) {
            // Sin mensaje, el fichero copiado no lo veria nadie.
            let _ = std::fs::remove_file(&destino);
            return Err(e.into());
        }
        cache.quitar(&cuaderno_fichero);
        drop(cache);
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == chat) {
            f.tocado = cuando;
            f.resumen = m.resumen();
        }
        indice.guardar(raiz)?;
        Ok(MensajeChat::from(m))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_rutas_con_dos_puntos_no_valen() {
        assert!(ruta_valida("archivos/foto.png"));
        assert!(ruta_valida("pixpin:files/fotos/a.jpg"));
        assert!(!ruta_valida(""));
        assert!(!ruta_valida("../otro/guardados.jsonl"));
        assert!(!ruta_valida("archivos/../../x"));
        assert!(!ruta_valida("archivos\\..\\x"));
        assert!(!ruta_valida("a\0b"));
    }
}
