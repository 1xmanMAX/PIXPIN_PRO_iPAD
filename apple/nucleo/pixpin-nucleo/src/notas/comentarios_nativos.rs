//! **Los comentarios al margen de una nota para un editor nativo** (el
//! `UITextView` del iPad): los mismos de `comentarios` (el fichero hermano
//! `anot-<codigo>.comentarios.json` que viaja con la nota, las cuentas de
//! `pixpin_docs::md_comentarios`), con registros de UniFFI en vez del JSON
//! de la pagina web. Todas las posiciones van en UTF-16 (`NSRange`).

use std::sync::{Arc, Mutex, MutexGuard};

use pixpin_docs::md_comentarios::Quien;
use pixpin_proyecto::{almacen, cuaderno};

use super::DestinoNota;
use super::comentarios::{self, Comentarista};
use crate::{ErrorNucleo, Nucleo};

/// Una respuesta de un hilo.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RespuestaNota {
    pub id: String,
    pub autor: String,
    pub aparato: String,
    /// Milisegundos desde 1970.
    pub cuando: i64,
    pub editado: Option<i64>,
    pub texto: String,
}

/// **Un hilo de comentarios** con donde cae ahora en el texto.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HiloNota {
    pub id: String,
    /// Donde esta la cita ahora (UTF-16); `None` si ya no se encuentra (se
    /// ensena aparte, «sin sitio»).
    pub desde: Option<u32>,
    pub hasta: Option<u32>,
    /// Lo que se comento, tal como estaba.
    pub cita: String,
    pub autor: String,
    pub aparato: String,
    pub cuando: i64,
    pub editado: Option<i64>,
    pub texto: String,
    pub resuelto: bool,
    pub respuestas: Vec<RespuestaNota>,
}

/// **Los comentarios de una nota abierta.** Cada cambio se escribe enseguida,
/// junto con lo que haya llegado al disco (otro equipo).
#[derive(uniffi::Object)]
pub struct ComentariosNota {
    ruta: Option<std::path::PathBuf>,
    quien: Quien,
    c: Mutex<Comentarista>,
}

#[uniffi::export]
impl Nucleo {
    /// **Abre los comentarios de una nota** (un mensaje `NOTA` o un `.md`
    /// suelto). `NoExiste` si el mensaje ya no esta.
    pub fn abrir_comentarios_de_nota(
        &self,
        destino: DestinoNota,
    ) -> Result<Arc<ComentariosNota>, ErrorNucleo> {
        let raiz = self.ruta().to_path_buf();
        let codigo = match &destino {
            DestinoNota::Mensaje { chat, mensaje } => {
                crate::chats::validar_chat(chat)?;
                let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&raiz, chat))?;
                let m = c
                    .mensajes
                    .iter()
                    .find(|m| &m.id == mensaje)
                    .ok_or_else(|| ErrorNucleo::NoExiste {
                        que: mensaje.clone(),
                    })?;
                m.codigo_unico()
            }
            DestinoNota::Fichero { .. } => String::new(),
        };
        let quien = match self.identidad() {
            Ok(i) => Quien {
                autor: i.yo.nombre.clone(),
                aparato: i.yo.codigo(),
            },
            Err(_) => Quien {
                autor: "iPad".into(),
                aparato: String::new(),
            },
        };
        let ruta = comentarios::ruta(&raiz, &destino, &codigo);
        let c = Comentarista::abrir(ruta.clone(), quien.clone());
        Ok(Arc::new(ComentariosNota {
            ruta,
            quien,
            c: Mutex::new(c),
        }))
    }
}

/// Las fotos de una nota para el editor nativo (`adjuntos`, el sitio y la
/// ruta portatil de Windows y el movil).
#[uniffi::export]
impl Nucleo {
    /// **Guarda una foto (o un fichero) de la nota** junto a ella y da la
    /// ruta que se escribe en el Markdown (`![](ruta)`): en la carpeta del
    /// proyecto (`notas/<hora>-<nombre>`, la que viaja) o, de un `.md`
    /// suelto, en `adjuntos/` a su lado. Bloqueante.
    pub fn adjuntar_a_nota(
        &self,
        destino: DestinoNota,
        nombre: String,
        bytes: Vec<u8>,
    ) -> Result<String, ErrorNucleo> {
        if let DestinoNota::Mensaje { chat, .. } = &destino {
            crate::chats::validar_chat(chat)?;
        }
        Ok(super::adjuntos::adjuntar_bytes(
            self.ruta(),
            &destino,
            &nombre,
            &bytes,
            crate::ahora_ms(),
        )?)
    }

    /// **Donde esta en este equipo** lo que la nota nombra con `ruta` (una
    /// foto `![](…)`), si esta.
    pub fn ruta_en_la_nota(&self, destino: DestinoNota, ruta: String) -> Option<String> {
        super::adjuntos::resolver(self.ruta(), &destino, &ruta)
            .filter(|p| p.is_file())
            .map(|p| p.display().to_string())
    }
}

fn ahora() -> i64 {
    crate::ahora_ms()
}

impl ComentariosNota {
    fn tomar(&self) -> MutexGuard<'_, Comentarista> {
        self.c.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[uniffi::export]
impl ComentariosNota {
    /// Los hilos, con donde caen en `texto` (el de ahora en el editor).
    pub fn hilos(&self, texto: String) -> Vec<HiloNota> {
        self.tomar()
            .hilos(&texto)
            .into_iter()
            .map(|(h, s)| HiloNota {
                id: h.id,
                desde: s.map(|s| s.0 as u32),
                hasta: s.map(|s| s.1 as u32),
                cita: h.ancla.cita,
                autor: h.autor,
                aparato: h.aparato,
                cuando: h.cuando,
                editado: h.editado,
                texto: h.texto,
                resuelto: h.resuelto,
                respuestas: h
                    .respuestas
                    .into_iter()
                    .map(|r| RespuestaNota {
                        id: r.id,
                        autor: r.autor,
                        aparato: r.aparato,
                        cuando: r.cuando,
                        editado: r.editado,
                        texto: r.texto,
                    })
                    .collect(),
            })
            .collect()
    }

    /// Cuantos hilos siguen abiertos (el numero del boton).
    pub fn abiertos(&self) -> u32 {
        self.tomar().abiertos() as u32
    }

    /// El aparato de este equipo (lo «mio» se puede editar y borrar).
    pub fn yo(&self) -> String {
        self.tomar().aparato()
    }

    /// **Un comentario nuevo** sobre `desde..hasta` de `texto` (UTF-16; si
    /// es un punto, la palabra que hay ahi). `None` si el cuerpo esta vacio
    /// o no hay que citar.
    pub fn nuevo(&self, texto: String, desde: u32, hasta: u32, cuerpo: String) -> Option<String> {
        self.tomar()
            .nuevo(&texto, desde as usize, hasta as usize, &cuerpo, ahora())
    }

    pub fn responder(&self, hilo: String, cuerpo: String) {
        self.tomar().responder(&hilo, &cuerpo, ahora());
    }

    /// Cambia el texto de un hilo o de una respuesta.
    pub fn editar(&self, id: String, cuerpo: String) {
        self.tomar().editar(&id, &cuerpo, ahora());
    }

    /// Borra un hilo o una respuesta.
    pub fn borrar(&self, id: String) {
        self.tomar().borrar(&id);
    }

    pub fn resolver(&self, hilo: String, si: bool) {
        self.tomar().resolver(&hilo, si, ahora());
    }

    /// Vuelve a leer del disco (llego algo de otro equipo). Lo de aqui ya
    /// esta escrito: cada cambio se guarda al momento.
    pub fn releer(&self) {
        *self.tomar() = Comentarista::abrir(self.ruta.clone(), self.quien.clone());
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_comentarios_de_un_md_suelto_se_guardan_y_se_reanclan() {
        let d = std::env::temp_dir().join(format!("pixpin-com-nativos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let n = Nucleo::abrir(d.join("datos").display().to_string(), "iPad".into()).unwrap();
        let md = d.join("nota.md");
        std::fs::write(&md, "Hola mundo bonito").unwrap();
        let destino = DestinoNota::Fichero {
            ruta: md.display().to_string(),
        };
        let c = n.abrir_comentarios_de_nota(destino.clone()).unwrap();
        let id = c
            .nuevo("Hola mundo bonito".into(), 5, 10, "¿seguro?".into())
            .unwrap();
        assert_eq!(c.abiertos(), 1);
        c.responder(id.clone(), "sí".into());
        // Otra vez desde el disco, con el texto movido: se reencuentra.
        let c2 = n.abrir_comentarios_de_nota(destino).unwrap();
        let h = c2.hilos("Ya: Hola mundo bonito".into());
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].id, id);
        assert_eq!((h[0].desde, h[0].hasta), (Some(9), Some(14)));
        assert_eq!(h[0].cita, "mundo");
        assert_eq!(h[0].respuestas.len(), 1);
        c2.resolver(id.clone(), true);
        assert_eq!(c2.abiertos(), 0);
        c.releer();
        assert_eq!(c.abiertos(), 0, "lo de otro se ve al releer");
        // Caso negativo: un comentario vacio no se crea.
        assert!(c2.nuevo("Hola".into(), 0, 4, "  ".into()).is_none());
        // Caso negativo: el mensaje de un chat que no esta.
        assert!(
            n.abrir_comentarios_de_nota(DestinoNota::Mensaje {
                chat: "no-esta".into(),
                mensaje: "x".into()
            })
            .is_err()
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn una_foto_de_un_md_suelto_va_a_su_lado_y_se_encuentra() {
        let d = std::env::temp_dir().join(format!("pixpin-foto-nota-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let n = Nucleo::abrir(d.join("datos").display().to_string(), "iPad".into()).unwrap();
        let md = d.join("nota.md");
        std::fs::write(&md, "Hola").unwrap();
        let destino = DestinoNota::Fichero {
            ruta: md.display().to_string(),
        };
        let ruta = n
            .adjuntar_a_nota(destino.clone(), "mi foto.png".into(), vec![1, 2, 3])
            .unwrap();
        assert!(ruta.starts_with("adjuntos/") && ruta.ends_with("-mi_foto.png"), "{ruta}");
        let real = n.ruta_en_la_nota(destino.clone(), ruta).unwrap();
        assert_eq!(std::fs::read(real).unwrap(), vec![1, 2, 3]);
        // Caso negativo: lo que no esta no sale.
        assert!(n.ruta_en_la_nota(destino, "adjuntos/no.png".into()).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
