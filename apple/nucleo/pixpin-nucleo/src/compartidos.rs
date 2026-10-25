//! **Chats compartidos con otras personas** (Android 13-oct-2026), del lado
//! del Mac: es `sincronizar/compartidos_ui.rs` de Windows sin la ventana,
//! sobre `pixpin_sincro::compartidos` e `invitacion`, que son los mismos
//! crates (las firmas, la valla de archivos y el protocolo estan alli,
//! probados contra Android).
//!
//! - **Compartir** un chat ([`Nucleo::compartir_chat`]): la lista con una
//!   sola persona, este aparato, que lo crea.
//! - **Invitar** ([`Nucleo::invitar_a_chat`]): una puerta propia mientras la
//!   ventana esta delante ([`Invitacion`]), anunciada como `_pixpinchat._tcp`
//!   con la etiqueta del codigo, y el QR
//!   `pixpin-chat:1:<codigo>:<ip>:<puerto>` (lo pinta Swift con CoreImage).
//!   Cinco codigos malos la cierran.
//! - **Entrar** ([`Nucleo::entrar_en_chat_compartido`]): con el codigo (o el
//!   texto del QR) se entra (`PXI1`) y despues se trae el chat como persona
//!   (`PXC1`).
//! - **Sincronizar** con cada persona ([`Nucleo::sincronizar_con_persona`]):
//!   solo los chats donde estamos los dos, con **la valla** de
//!   `pixpin_sincro::valla` (ningun fichero de otro chat sale ni entra).
//!   Las personas tambien llaman a la [`crate::Escucha`] de siempre.
//! - **Quitar** a alguien (solo quien lo creo) o **salir** uno mismo.
//!
//! # Las claves
//!
//! Igual que Windows y Android: el par P-256 de este aparato vive en
//! `<datos>/sincro/persona.json` (publica X.509 y privada PKCS#8 en base64)
//! y nace la primera vez que hace falta (`CompartidosEnDisco::llaves`).
//! **Windows no usa DPAPI para esto** (DPAPI es solo para las claves de la
//! nube del asistente), asi que el Mac tampoco usa el Llavero: el fichero es
//! el mismo en los tres, y la carpeta de datos es la del usuario. Ver
//! `apple/docs/sincro.md`.
//!
//! Los ids de chat: lo que viaja es el id del chat **tal como lo ve el
//! movil** (`vista::chat_de_ficha`); Swift trabaja con el de la ficha
//! (`FichaChat.id`). Aqui se pasa de uno a otro.

use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use pixpin_proyecto::vista::DiscoPc;
use pixpin_sincro::canal::{ErrorCanal, MAGIA_DE_INVITACION, MAGIA_DE_PERSONA};
use pixpin_sincro::compartidos::{self, ChatCompartido, CompartidosEnDisco};
use pixpin_sincro::invitacion;
use pixpin_sincro::protocolo::{ErrorSincro, Respondedor};

use crate::red::{conectar, nonce};
use crate::sincro::{AnuncioBonjour, ParTxt};
use crate::{ErrorNucleo, Nucleo};

/// Una persona (un aparato de una persona) de un chat compartido.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PersonaCompartida {
    /// El id de su aparato.
    pub id: String,
    pub nombre: String,
    /// El codigo fijo de su aparato (`K7Q2`), el de sus mensajes.
    pub codigo: String,
    pub dentro: bool,
    /// Cuando entro (la ultima vez), en ms.
    pub desde: i64,
    /// Este aparato.
    pub soy_yo: bool,
    /// Si este aparato la puede quitar (o salir, si es uno mismo).
    pub puedo_quitar: bool,
}

/// Un chat compartido, para ensenarlo.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChatCompartidoMac {
    /// El id del chat tal como viaja.
    pub chat: String,
    /// La ficha de este aparato (`FichaChat.id`), si ya esta aqui.
    pub ficha: Option<String>,
    pub nombre: String,
    /// El id del aparato que lo creo.
    pub creador: String,
    pub creado: i64,
    pub personas: Vec<PersonaCompartida>,
    /// Este aparato esta dentro.
    pub estoy_dentro: bool,
}

/// Lo que dice un codigo de invitacion tecleado o su QR.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CodigoDeInvitacion {
    /// Los ocho signos (`K7Q2M9XP`).
    pub codigo: String,
    /// Donde esta quien invita, si el QR lo dice (si no, se busca en
    /// Bonjour `_pixpinchat._tcp` por [`etiqueta_de_invitacion`]).
    pub host: Option<String>,
    pub puerto: u16,
}

/// Lo que paso en una vuelta con una persona.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ResumenPersona {
    pub otro_id: String,
    pub otro_nombre: String,
    /// Donde escucha el otro (para recordarlo), o 0.
    pub puerto_del_otro: u16,
    /// Los chats que se juntaron (ids de chat tal como viajan).
    pub chats: Vec<String>,
    pub traidos: u64,
    pub enviados: u64,
    pub bytes: u64,
    /// Si llego algo nuevo (hay que releer los chats).
    pub trajo_algo: bool,
}

/// `que_entrar` de Windows: el texto de un QR de invitacion o el codigo a
/// secas. Otro QR de PixPin (o cualquier texto con `:`) no es un codigo.
#[uniffi::export]
pub fn leer_invitacion(texto: String) -> Option<CodigoDeInvitacion> {
    if let Some(q) = invitacion::leer_qr(&texto) {
        return Some(CodigoDeInvitacion {
            codigo: q.codigo,
            host: q.host,
            puerto: q.puerto,
        });
    }
    if texto.contains(':') {
        return None;
    }
    let c = invitacion::limpiar(&texto);
    invitacion::valido(&c).then_some(CodigoDeInvitacion {
        codigo: c,
        host: None,
        puerto: 0,
    })
}

/// `K7Q2-M9XP`.
#[uniffi::export]
pub fn invitacion_legible(codigo: String) -> String {
    invitacion::legible(&codigo)
}

/// La etiqueta (TXT `g`) de quien invita con ese codigo. PBKDF2: fuera del
/// hilo de la interfaz.
#[uniffi::export]
pub fn etiqueta_de_invitacion(codigo: String) -> String {
    invitacion::etiqueta(&codigo)
}

/// `_pixpinchat._tcp`.
#[uniffi::export]
pub fn tipo_de_servicio_chat() -> String {
    invitacion::TIPO.trim_end_matches('.').to_string()
}

impl Nucleo {
    fn compartidos_en_disco(&self) -> CompartidosEnDisco {
        CompartidosEnDisco::nuevo(self.ruta())
    }

    fn yo_compartido(&self) -> Result<compartidos::Yo, ErrorNucleo> {
        let i = self.identidad()?;
        Ok(self.compartidos_en_disco().yo(&i.yo.id, &i.yo.nombre)?)
    }

    /// El id de chat que viaja de una ficha de aqui (o el mismo si no hay
    /// otro).
    fn chat_de(&self, ficha: &str) -> String {
        pixpin_proyecto::vista::chat_de_ficha(self.ruta(), ficha).unwrap_or_else(|| ficha.into())
    }

    fn para_ver(&self, c: &ChatCompartido, yo: &str) -> ChatCompartidoMac {
        let ficha = DiscoPc::nuevo(self.ruta()).ficha_de(&c.chat).map(|f| f.id);
        ChatCompartidoMac {
            chat: c.chat.clone(),
            ficha,
            nombre: c.nombre.clone(),
            creador: c.creador.clone(),
            creado: c.creado,
            personas: c
                .personas
                .iter()
                .map(|p| PersonaCompartida {
                    id: p.id.clone(),
                    nombre: p.nombre.clone(),
                    codigo: p.codigo.clone(),
                    dentro: p.dentro(),
                    desde: p.desde,
                    soy_yo: p.id == yo,
                    puedo_quitar: compartidos::puede_quitar(c, yo, &p.id),
                })
                .collect(),
            estoy_dentro: compartidos::dentro(c, yo),
        }
    }
}

#[uniffi::export]
impl Nucleo {
    /// Todos los chats compartidos que conoce este aparato.
    pub fn chats_compartidos(&self) -> Result<Vec<ChatCompartidoMac>, ErrorNucleo> {
        let yo = self.identidad()?.yo.id;
        Ok(self
            .compartidos_en_disco()
            .leer()
            .iter()
            .map(|c| self.para_ver(c, &yo))
            .collect())
    }

    /// El chat compartido de la ficha `ficha` (`FichaChat.id`), si lo esta.
    pub fn chat_compartido(&self, ficha: String) -> Result<Option<ChatCompartidoMac>, ErrorNucleo> {
        let yo = self.identidad()?.yo.id;
        let chat = self.chat_de(&ficha);
        Ok(self
            .compartidos_en_disco()
            .de(&chat)
            .map(|c| self.para_ver(&c, &yo)))
    }

    /// **Comparte el chat** de la ficha `ficha` con personas (si ya lo
    /// estaba, no cambia nada).
    pub fn compartir_chat(&self, ficha: String) -> Result<ChatCompartidoMac, ErrorNucleo> {
        crate::chats::validar_chat(&ficha)?;
        let nombre = self
            .chats()?
            .into_iter()
            .find(|c| c.id == ficha)
            .map(|c| c.nombre)
            .ok_or_else(|| ErrorNucleo::NoExiste { que: ficha.clone() })?;
        let chat = self.chat_de(&ficha);
        let yo = self.yo_compartido()?;
        let cd = self.compartidos_en_disco();
        let lista = cd.cambiar(|mut l| -> std::io::Result<_> {
            if !l.iter().any(|c| c.chat == chat) {
                l.push(compartidos::crear(&chat, &nombre, &yo, crate::ahora_ms()));
            }
            Ok(l)
        })?;
        let c = lista
            .iter()
            .find(|c| c.chat == chat)
            .ok_or_else(|| ErrorNucleo::NoExiste { que: chat.clone() })?;
        Ok(self.para_ver(c, &yo.id))
    }

    /// **Quita a `id`** del chat compartido `chat` (el id que viaja), o
    /// **sale** si `id` es este aparato. Solo quien lo creo quita a otros.
    pub fn quitar_de_chat_compartido(
        &self,
        chat: String,
        id: String,
    ) -> Result<ChatCompartidoMac, ErrorNucleo> {
        let yo = self.yo_compartido()?;
        let ahora = crate::ahora_ms();
        let lista = self
            .compartidos_en_disco()
            .cambiar(|l| -> Result<_, ErrorNucleo> {
                l.into_iter()
                    .map(|c| {
                        if c.chat == chat {
                            compartidos::quitar(&c, &yo, &id, ahora)
                                .map_err(|detalle| ErrorNucleo::Argumento { detalle })
                        } else {
                            Ok(c)
                        }
                    })
                    .collect()
            })?;
        let c = lista
            .iter()
            .find(|c| c.chat == chat)
            .ok_or(ErrorNucleo::NoExiste { que: chat })?;
        Ok(self.para_ver(c, &yo.id))
    }

    /// El nombre con el que me ven en los chats compartidos (el del aparato
    /// si no puse otro).
    pub fn nombre_en_compartidos(&self) -> Result<String, ErrorNucleo> {
        Ok(self.yo_compartido()?.nombre)
    }

    /// Pone ese nombre (40 letras como mucho).
    pub fn poner_nombre_en_compartidos(&self, nombre: String) -> Result<(), ErrorNucleo> {
        if nombre.trim().is_empty() {
            return Err(ErrorNucleo::Argumento {
                detalle: "el nombre no puede quedar vacio".into(),
            });
        }
        Ok(self.compartidos_en_disco().poner_nombre(&nombre)?)
    }

    /// Si este aparato esta dentro de algun chat compartido: entonces hay
    /// que escuchar y anunciarse aunque no haya grupo
    /// ([`Nucleo::anuncio_de_presencia`]).
    pub fn hay_chats_compartidos(&self) -> Result<bool, ErrorNucleo> {
        let yo = self.identidad()?.yo.id;
        Ok(self.compartidos_en_disco().hay_alguno(&yo))
    }

    /// **Abre la puerta para invitar** al chat de la ficha `ficha` (que
    /// tiene que estar compartido): un codigo nuevo y un puerto propio
    /// (IPv6 con IPv4 dentro) mientras la ventana este delante.
    pub fn invitar_a_chat(self: Arc<Self>, ficha: String) -> Result<Arc<Invitacion>, ErrorNucleo> {
        let chat = self.chat_de(&ficha);
        let yo = self.identidad()?.yo.id;
        match self.compartidos_en_disco().de(&chat) {
            Some(c) if compartidos::dentro(&c, &yo) => {}
            Some(_) => {
                return Err(ErrorNucleo::Argumento {
                    detalle: "ya no estás en este chat".into(),
                });
            }
            None => {
                return Err(ErrorNucleo::Argumento {
                    detalle: "este chat aún no está compartido".into(),
                });
            }
        }
        let escucha = crate::red::escuchar(0)?;
        let puerto = escucha.local_addr()?.port();
        Ok(Arc::new(Invitacion {
            escucha,
            puerto,
            codigo: invitacion::nuevo_codigo(),
            chat,
            nucleo: self,
            cerrada: AtomicBool::new(false),
            malos: AtomicU32::new(0),
            esperando: AtomicUsize::new(0),
        }))
    }

    /// **Entra en un chat compartido** con el `codigo` de quien invita, que
    /// escucha en `host:puerto` (del QR, o buscado en Bonjour por la
    /// etiqueta), y **se trae el chat**. Bloquea hasta terminar.
    pub fn entrar_en_chat_compartido(
        &self,
        codigo: String,
        host: String,
        puerto: u16,
    ) -> Result<ChatCompartidoMac, ErrorNucleo> {
        let codigo = invitacion::limpiar(&codigo);
        if !invitacion::valido(&codigo) {
            return Err(ErrorNucleo::Argumento {
                detalle: "el código de la invitación son ocho signos".into(),
            });
        }
        let d = DiscoPc::nuevo(self.ruta());
        let flujo = conectar(&host, puerto)?;
        let e = invitacion::entrar(flujo, &d, &codigo, crate::ahora_ms(), nonce()?)?;
        // Ya dentro: el chat en si, como persona.
        let flujo = conectar(&host, puerto)?;
        invitacion::vuelta_con_persona(
            flujo,
            &d,
            Some(&e.quien.id),
            None,
            &crate::ahora_ms,
            self.mi_puerto(),
            nonce()?,
            &mut |_| {},
        )?;
        self.vaciar_caches();
        let yo = self.identidad()?.yo.id;
        let c = self
            .compartidos_en_disco()
            .de(&e.chat.chat)
            .unwrap_or(e.chat);
        Ok(self.para_ver(&c, &yo))
    }

    /// **Una vuelta con una persona** que escucha en `host:puerto`: se
    /// juntan las listas de los chats que compartimos y cada chat donde
    /// estamos los dos (nada mas: la valla). `id`, el de su aparato si se
    /// sabe (comprueba que contesta quien debe); `chats`, solo esos (ids que
    /// viajan) o todos los comunes si va vacio.
    pub fn sincronizar_con_persona(
        &self,
        host: String,
        puerto: u16,
        id: Option<String>,
        chats: Vec<String>,
    ) -> Result<ResumenPersona, ErrorNucleo> {
        let d = DiscoPc::nuevo(self.ruta());
        let flujo = conectar(&host, puerto)?;
        let solo = (!chats.is_empty()).then_some(chats.as_slice());
        let r = invitacion::vuelta_con_persona(
            flujo,
            &d,
            id.as_deref(),
            solo,
            &crate::ahora_ms,
            self.mi_puerto(),
            nonce()?,
            &mut |_| {},
        )
        .map_err(|e| match e {
            ErrorSincro::Canal(ErrorCanal::NoEsMiembro) => ErrorNucleo::Sincro {
                detalle: format!("{host} no comparte ningún chat con este Mac"),
            },
            otro => otro.into(),
        })?;
        self.vaciar_caches();
        Ok(ResumenPersona {
            trajo_algo: r.trajo_algo(),
            otro_id: r.quien.id,
            otro_nombre: r.quien.nombre,
            puerto_del_otro: u16::try_from(r.puerto).unwrap_or(0),
            chats: r.chats,
            traidos: r.hecho.traidos as u64,
            enviados: r.hecho.enviados as u64,
            bytes: r.bytes,
        })
    }
}

/// Lo que paso en una conexion a una [`Invitacion`].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AvisoInvitacion {
    /// Se cerro: no llego nadie.
    Cerrada,
    /// Alguien entro con el codigo.
    Entro { id: String, nombre: String },
    /// Una persona se trajo el chat (o lo junto) por esta puerta.
    Junto { id: String, nombre: String },
    /// Alguien probo otro codigo (`intentos` seguidos).
    CodigoMalo { intentos: u32 },
    /// Cinco codigos malos: la invitacion se cerro sola.
    Quemada,
    /// Algo salio mal con esa conexion; la puerta sigue abierta.
    Fallo { detalle: String },
}

/// **La puerta de una invitacion** (`invitacion::Puerta` de
/// `pixpin_sincro`, sin su hilo que sondea cada 100 ms: aqui `accept`
/// bloquea en un hilo de Swift y se despierta al cerrar). Atiende la
/// invitacion (`PXI1`) y, detras, la conversacion de persona (`PXC1`) con
/// la que el otro se trae el chat.
#[derive(uniffi::Object)]
pub struct Invitacion {
    escucha: TcpListener,
    puerto: u16,
    codigo: String,
    /// El chat (id que viaja).
    chat: String,
    nucleo: Arc<Nucleo>,
    cerrada: AtomicBool,
    malos: AtomicU32,
    esperando: AtomicUsize,
}

#[uniffi::export]
impl Invitacion {
    /// Los ocho signos (para ensenar: [`invitacion_legible`]).
    pub fn codigo(&self) -> String {
        self.codigo.clone()
    }

    pub fn puerto(&self) -> u16 {
        self.puerto
    }

    /// El texto del QR, `pixpin-chat:1:<codigo>:<ip>:<puerto>`, con la IPv4
    /// de este Mac que da Swift (una IPv6 no cabe: va vacio y el otro busca
    /// en Bonjour).
    pub fn texto_del_qr(&self, host: Option<String>) -> String {
        let h = host
            .filter(|h| crate::red::es_ipv4(h))
            .map(|h| crate::red::limpiar_host(&h).to_string());
        invitacion::texto_del_qr(&self.codigo, h.as_deref(), self.puerto)
    }

    /// Lo que hay que anunciar en Bonjour: `PixPin <nombre>` del tipo
    /// `_pixpinchat._tcp` con el TXT `g` (como Windows). PBKDF2: fuera del
    /// hilo de la interfaz.
    pub fn anuncio(&self, nombre: String) -> AnuncioBonjour {
        let etiqueta = invitacion::etiqueta(&self.codigo);
        AnuncioBonjour {
            instancia: crate::sincro::recortar_etiqueta(format!("PixPin {nombre}").trim()),
            tipo: tipo_de_servicio_chat(),
            txt: vec![ParTxt {
                clave: "g".into(),
                valor: etiqueta.clone(),
            }],
            etiqueta,
            id: String::new(),
        }
    }

    /// Espera a alguien y lo atiende entero. Se puede llamar desde varios
    /// hilos.
    pub fn atender(&self) -> Result<AvisoInvitacion, ErrorNucleo> {
        match crate::envio::aceptar(&self.escucha, &self.esperando, &self.cerrada)? {
            Some(flujo) => Ok(self.una(flujo)),
            None => Ok(AvisoInvitacion::Cerrada),
        }
    }

    pub fn cerrar(&self) {
        if self.cerrada.swap(true, Ordering::SeqCst) {
            return;
        }
        for _ in 0..50 {
            let n = self.esperando.load(Ordering::SeqCst);
            if n == 0 {
                break;
            }
            for _ in 0..n {
                crate::red::despertar(self.puerto);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

impl Invitacion {
    fn una(&self, flujo: TcpStream) -> AvisoInvitacion {
        let _ = flujo.set_read_timeout(Some(crate::red::ESPERA_AL_LEER));
        let mut magia = [0u8; 4];
        if !matches!(flujo.peek(&mut magia), Ok(4)) {
            return AvisoInvitacion::Fallo {
                detalle: "llamada sin saludo".into(),
            };
        }
        let disco = DiscoPc::nuevo(self.nucleo.ruta());
        let quien = std::cell::RefCell::new((String::new(), String::new()));
        let r = Respondedor {
            disco: &disco,
            estado: &|_| {},
            ahora: &crate::ahora_ms,
            mi_puerto: self.nucleo.mi_puerto(),
            al_saludar: &|otro, _| *quien.borrow_mut() = (otro.id.clone(), otro.nombre.clone()),
            suelto: None,
        };
        let Ok(azar) = nonce() else {
            return AvisoInvitacion::Fallo {
                detalle: "sin azar del sistema".into(),
            };
        };
        if &magia == MAGIA_DE_INVITACION {
            match r.atender_invitacion(flujo, azar, &self.codigo, &self.chat) {
                Ok(p) => {
                    self.malos.store(0, Ordering::SeqCst);
                    AvisoInvitacion::Entro {
                        id: p.id,
                        nombre: p.nombre,
                    }
                }
                Err(ErrorSincro::Canal(ErrorCanal::CodigoDistinto)) => {
                    // Alguien probando codigos: a los pocos fallos, se cierra.
                    let malos = self.malos.fetch_add(1, Ordering::SeqCst) + 1;
                    if malos >= invitacion::INTENTOS {
                        self.cerrar();
                        AvisoInvitacion::Quemada
                    } else {
                        AvisoInvitacion::CodigoMalo { intentos: malos }
                    }
                }
                Err(e) => AvisoInvitacion::Fallo {
                    detalle: e.to_string(),
                },
            }
        } else if &magia == MAGIA_DE_PERSONA {
            match r.atender_persona(flujo, azar) {
                Ok(()) => {
                    self.nucleo.vaciar_caches();
                    let (id, nombre) = quien.into_inner();
                    AvisoInvitacion::Junto { id, nombre }
                }
                Err(e) => AvisoInvitacion::Fallo {
                    detalle: e.to_string(),
                },
            }
        } else {
            AvisoInvitacion::Fallo {
                detalle: "llamada que no es de una invitación".into(),
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_entra_con_el_codigo_tecleado_o_con_el_texto_del_qr() {
        let q = leer_invitacion("pixpin-chat:1:K7Q2M9XP:192.168.1.5:4000".into()).unwrap();
        assert_eq!(q.codigo, "K7Q2M9XP");
        assert_eq!(q.host.as_deref(), Some("192.168.1.5"));
        assert_eq!(q.puerto, 4000);
        let q = leer_invitacion("k7q2-m9xp".into()).unwrap();
        assert_eq!((q.codigo.as_str(), q.host, q.puerto), ("K7Q2M9XP", None, 0));
        // Casos negativos: otro QR de PixPin, un codigo corto.
        assert!(leer_invitacion("pixpin-envio:1:123456::0".into()).is_none());
        assert!(leer_invitacion("k7q2".into()).is_none());
        assert_eq!(invitacion_legible("K7Q2M9XP".into()), "K7Q2-M9XP");
        assert_eq!(tipo_de_servicio_chat(), "_pixpinchat._tcp");
    }
}
