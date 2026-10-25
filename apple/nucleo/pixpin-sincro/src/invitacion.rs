//! **Anadir a una persona a un chat compartido** (`sincro/Invitacion.kt`,
//! Android 13-oct-2026): como emparejar un aparato del grupo, con un codigo
//! corto y su QR, pero quien entra **recibe ese chat y nada mas**.
//!
//! Quien invita abre una puerta en su red mientras tiene la ventana delante,
//! anunciada como `_pixpinchat._tcp.` con `g = etiqueta(clave("chat-" +
//! codigo))`, y ensena el codigo (`K7Q2-M9XP`) y el QR
//! `pixpin-chat:1:<codigo>:<ip>:<puerto>`. Quien entra lo teclea (o pega el
//! texto del QR): con la clave que sale del codigo se hablan una sola vez
//! (`PXI1`); le da su ficha (su clave publica y su nombre), quien invita la
//! firma y le devuelve la lista del chat. Desde ahi ya se reconocen por sus
//! claves, y el chat en si viaja por una conversacion entre personas
//! (`PXC1`), que no depende de lo corto del codigo.

use std::io::{Read, Write};
use std::sync::Mutex;

use rand_core::RngCore;

use crate::canal::{Canal, ErrorCanal, MAGIA_DE_INVITACION};
use crate::codigo::{
    SIGNOS, clave_cruda, etiqueta as etiqueta_de_clave, limpiar as limpiar_codigo,
};
use crate::compartidos::{self, ChatCompartido, CompartidosEnDisco};
use crate::disco::Disco;
use crate::mensajes::{Aparato, Hola, Peticion, Respuesta};
use crate::protocolo::{ErrorSincro, Resultado};

/// Ocho signos sin letras que se confundan: unos 40 bits, de sobra para lo
/// que dura abierta.
pub const LARGO: usize = 8;
/// El tipo mDNS de quien invita.
pub const TIPO: &str = "_pixpinchat._tcp.";
/// Codigos equivocados seguidos que cierran la invitacion (`Envio.INTENTOS`).
pub const INTENTOS: u32 = 5;

/// Un codigo nuevo, uniforme sobre el alfabeto del grupo.
pub fn nuevo_codigo() -> String {
    let signos: Vec<char> = SIGNOS.chars().collect();
    let n = signos.len() as u32;
    // Sin sesgo: se descarta lo que cae en el ultimo trozo incompleto.
    let tope = u32::MAX - u32::MAX % n;
    let mut azar = rand_core::OsRng;
    (0..LARGO)
        .map(|_| {
            loop {
                let x = azar.next_u32();
                if x < tope {
                    break signos[(x % n) as usize];
                }
            }
        })
        .collect()
}

/// Lo tecleado, limpio: mayusculas, solo signos del alfabeto, ocho como
/// mucho.
pub fn limpiar(tecleado: &str) -> String {
    limpiar_codigo(tecleado).chars().take(LARGO).collect()
}

pub fn valido(codigo: &str) -> bool {
    codigo.chars().count() == LARGO && codigo.chars().all(|c| SIGNOS.contains(c))
}

/// `K7Q2-M9XP`.
pub fn legible(codigo: &str) -> String {
    if codigo.chars().count() == LARGO {
        let a: String = codigo.chars().take(4).collect();
        let b: String = codigo.chars().skip(4).collect();
        format!("{a}-{b}")
    } else {
        codigo.to_string()
    }
}

/// La clave de la conversacion de la invitacion: `Grupo.clave("chat-" +
/// codigo)`, PBKDF2 tal cual (sin limpiar el guion). Se recuerda por codigo:
/// son 60 000 vueltas y cada conexion las pagaria otra vez.
pub fn clave(codigo: &str) -> [u8; 32] {
    static HECHAS: Mutex<Vec<(String, [u8; 32])>> = Mutex::new(Vec::new());
    if let Some(c) = HECHAS
        .lock()
        .ok()
        .and_then(|h| h.iter().find(|(k, _)| k == codigo).map(|(_, c)| *c))
    {
        return c;
    }
    let c = clave_cruda(&format!("chat-{codigo}"));
    if let Ok(mut h) = HECHAS.lock() {
        if h.len() > 16 {
            h.remove(0);
        }
        h.push((codigo.to_string(), c));
    }
    c
}

/// Lo que se anuncia en la red para que quien teclea el codigo encuentre a
/// quien invita.
pub fn etiqueta(codigo: &str) -> String {
    etiqueta_de_clave(&clave(codigo))
}

pub fn texto_del_qr(codigo: &str, host: Option<&str>, puerto: u16) -> String {
    format!("pixpin-chat:1:{codigo}:{}:{puerto}", host.unwrap_or(""))
}

/// Lo que dice un QR de invitacion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelQr {
    pub codigo: String,
    pub host: Option<String>,
    pub puerto: u16,
}

pub fn leer_qr(texto: &str) -> Option<DelQr> {
    let p: Vec<&str> = texto.trim().split(':').collect();
    if p.len() < 5 || p[0] != "pixpin-chat" || !valido(p[2]) {
        return None;
    }
    Some(DelQr {
        codigo: p[2].to_string(),
        host: (!p[3].trim().is_empty()).then(|| p[3].to_string()),
        puerto: p[4].trim().parse().unwrap_or(0),
    })
}

/// Lo que sale de entrar: el chat, quien me invito y donde escucha.
#[derive(Debug, Clone)]
pub struct Entrada {
    pub chat: ChatCompartido,
    pub quien: Aparato,
    pub puerto: u32,
}

/// **Entrar con el codigo** (`Invitacion.entrar`) por una conexion ya abierta
/// con quien invita. Guarda la lista del chat y devuelve lo que se sabe; el
/// chat en si se trae despues con una vuelta entre personas.
pub fn entrar<D: Disco + ?Sized, F: Read + Write>(
    flujo: F,
    d: &D,
    codigo: &str,
    ahora: i64,
    nonce: [u8; 32],
) -> Resultado<Entrada> {
    let cd = CompartidosEnDisco::nuevo(&d.raiz());
    let yo = cd.yo_de(d)?;
    let malo = || ErrorSincro::Protocolo("El código no es el bueno".into());
    let mut canal = Canal::saludar_con(flujo, &clave(codigo), nonce, true, MAGIA_DE_INVITACION)?;
    let peticion = Peticion {
        t: "unirme".into(),
        hola: Some(Hola {
            yo: Aparato {
                id: yo.id.clone(),
                nombre: yo.nombre.clone(),
                ..Default::default()
            },
            reloj: ahora,
            ..Default::default()
        }),
        persona: Some(compartidos::ficha_a_texto(&yo.ficha(0))),
        ..Default::default()
    };
    canal.mandar(crate::canal::Tipo::Json, &serde_json::to_vec(&peticion)?)?;
    let r: Respuesta = match canal.recibir() {
        Ok((_, datos)) => serde_json::from_slice(&datos)?,
        // Quien invita no pudo descifrar el saludo (otro codigo) y colgo; o
        // lo que llega no descifra.
        Err(ErrorCanal::CodigoDistinto) => return Err(malo()),
        Err(ErrorCanal::Io(e))
            if matches!(
                e.kind(),
                std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
            ) =>
        {
            return Err(malo());
        }
        Err(e) => return Err(e.into()),
    };
    if let Some(e) = r.error {
        return Err(ErrorSincro::Remoto(e));
    }
    let hola = r
        .hola
        .ok_or_else(|| ErrorSincro::Protocolo("No saludó".into()))?;
    let mut llegan = compartidos::de_texto(r.compartidos.as_deref());
    if llegan.len() != 1 {
        return Err(ErrorSincro::Protocolo("No llegó el chat".into()));
    }
    let suyo = llegan.remove(0);
    let juntos = cd.cambiar(|l| -> std::io::Result<_> {
        Ok(compartidos::juntar_todos(
            &l,
            std::slice::from_ref(&suyo),
            |_| true,
        ))
    })?;
    let junto = juntos
        .into_iter()
        .find(|c| c.chat == suyo.chat)
        .ok_or_else(|| ErrorSincro::Protocolo("No llegó el chat".into()))?;
    // Que de verdad me haya metido, con su firma: si no, la lista no vale
    // para nada.
    if !compartidos::dentro(&junto, &yo.id) {
        return Err(ErrorSincro::Protocolo(format!(
            "No se pudo entrar en «{}»",
            suyo.nombre
        )));
    }
    let adios = Peticion::de("adios");
    if canal
        .mandar(crate::canal::Tipo::Json, &serde_json::to_vec(&adios)?)
        .is_ok()
    {
        let _ = canal.recibir();
    }
    d.avisar(crate::disco::Cambio::Identidad);
    Ok(Entrada {
        chat: junto,
        quien: hola.yo,
        puerto: hola.puerto,
    })
}

/// Lo que cuenta la puerta de una invitacion mientras esta abierta.
#[derive(Debug, Clone)]
pub enum AvisoDePuerta {
    /// Alguien entro con el codigo: su ficha.
    Entro(compartidos::Persona),
    /// Una persona se trajo el chat (o lo junto) por esta puerta.
    Junto(String),
    /// Algo salio mal con una conexion; la puerta sigue abierta.
    Fallo(String),
    /// Demasiados codigos equivocados: la puerta se cerro sola.
    Cerrada,
    /// Lo que esta pasando, en palabras.
    Estado(String),
}

/// **La puerta de una invitacion** (`abrirInvitacion` de
/// `ChatCompartidoActivity`): un puerto propio mientras la ventana esta
/// delante, que atiende la invitacion (`PXI1`) y, detras, la conversacion de
/// persona (`PXC1`) con la que el otro se trae el chat. Cinco codigos
/// equivocados la cierran. Se cierra al soltarla.
pub struct Puerta {
    pub codigo: String,
    pub puerto: u16,
    vivo: std::sync::Arc<std::sync::atomic::AtomicBool>,
    hilo: Option<std::thread::JoinHandle<()>>,
}

impl Puerta {
    /// Abre la puerta para `chat` con un codigo nuevo. `mi_puerto` es el de
    /// la escucha de siempre, para que el otro lo apunte.
    pub fn abrir<D: Disco + Send + Sync + 'static>(
        disco: std::sync::Arc<D>,
        chat: &str,
        mi_puerto: u32,
        ahora: fn() -> i64,
        avisar: impl Fn(AvisoDePuerta) + Send + 'static,
    ) -> std::io::Result<Puerta> {
        Puerta::abrir_con(
            disco,
            chat,
            &nuevo_codigo(),
            ("0.0.0.0", 0),
            mi_puerto,
            ahora,
            avisar,
        )
    }

    /// [Puerta::abrir] con el codigo y la direccion dados (las pruebas).
    pub fn abrir_con<D: Disco + Send + Sync + 'static>(
        disco: std::sync::Arc<D>,
        chat: &str,
        codigo: &str,
        donde: (&str, u16),
        mi_puerto: u32,
        ahora: fn() -> i64,
        avisar: impl Fn(AvisoDePuerta) + Send + 'static,
    ) -> std::io::Result<Puerta> {
        use std::sync::atomic::Ordering;
        let escucha = std::net::TcpListener::bind(donde)?;
        escucha.set_nonblocking(true)?;
        let puerto = escucha.local_addr()?.port();
        let vivo = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let (v, cod, chat) = (vivo.clone(), codigo.to_string(), chat.to_string());
        let hilo = std::thread::Builder::new()
            .name("chat-invitacion".into())
            .spawn(move || {
                let mut fallos = 0;
                while v.load(Ordering::SeqCst) {
                    let flujo = match escucha.accept() {
                        Ok((f, _)) => f,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            continue;
                        }
                        Err(_) => break,
                    };
                    let de = flujo.peer_addr().ok();
                    let _ = flujo.set_nonblocking(false);
                    let _ = flujo.set_read_timeout(Some(std::time::Duration::from_secs(120)));
                    let mut magia = [0u8; 4];
                    if !matches!(flujo.peek(&mut magia), Ok(4)) {
                        continue;
                    }
                    let estado = |t: &str| avisar(AvisoDePuerta::Estado(t.to_string()));
                    let reloj = || ahora();
                    let saludo = |_: &Aparato, _: u32| {};
                    let r = crate::protocolo::Respondedor {
                        disco: &*disco,
                        estado: &estado,
                        ahora: &reloj,
                        mi_puerto,
                        al_saludar: &saludo,
                        suelto: None,
                    };
                    let mut azar = [0u8; 32];
                    rand_core::OsRng.fill_bytes(&mut azar);
                    if &magia == MAGIA_DE_INVITACION {
                        match r.atender_invitacion(flujo, azar, &cod, &chat) {
                            Ok(p) => {
                                fallos = 0;
                                avisar(AvisoDePuerta::Entro(p));
                            }
                            Err(ErrorSincro::Canal(ErrorCanal::CodigoDistinto)) => {
                                // Alguien probando codigos: a los pocos
                                // fallos, la invitacion se cierra.
                                fallos += 1;
                                if fallos >= INTENTOS {
                                    avisar(AvisoDePuerta::Cerrada);
                                    break;
                                }
                                avisar(AvisoDePuerta::Fallo("El código no es el bueno".into()));
                            }
                            Err(e) => avisar(AvisoDePuerta::Fallo(e.to_string())),
                        }
                    } else if &magia == crate::canal::MAGIA_DE_PERSONA {
                        match r.atender_persona(flujo, azar) {
                            Ok(()) => avisar(AvisoDePuerta::Junto(
                                de.map(|d| d.ip().to_string()).unwrap_or_default(),
                            )),
                            Err(e) => avisar(AvisoDePuerta::Fallo(e.to_string())),
                        }
                    }
                }
                v.store(false, Ordering::SeqCst);
            })?;
        Ok(Puerta {
            codigo: codigo.to_string(),
            puerto,
            vivo,
            hilo: Some(hilo),
        })
    }

    /// Si sigue abierta (cinco codigos malos la cierran sola).
    pub fn abierta(&self) -> bool {
        self.vivo.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn cerrar(&mut self) {
        self.vivo.store(false, std::sync::atomic::Ordering::SeqCst);
        if let Some(h) = self.hilo.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Puerta {
    fn drop(&mut self) {
        self.cerrar();
    }
}

/// Lo que paso en una vuelta con una persona (`VueltaConPersona.Resultado`).
#[derive(Debug, Clone, Default)]
pub struct ConPersona {
    pub quien: Aparato,
    pub puerto: u32,
    /// Los chats que se juntaron (los compartidos donde estamos los dos).
    pub chats: Vec<String>,
    pub hecho: crate::protocolo::Hecho,
    pub bytes: u64,
}

impl ConPersona {
    pub fn trajo_algo(&self) -> bool {
        self.hecho.traidos > 0 || self.hecho.fusionados > 0 || self.hecho.borrados > 0
    }
}

/// **Una vuelta con una persona** (`VueltaConPersona.con`): se juntan las
/// listas de los chats que compartimos y luego cada chat donde estamos los
/// dos, igual que con un aparato del grupo. Sin preguntas ni borrados de
/// proyectos: lo de una persona se junta siempre. `a_quien`: el id del
/// aparato al que se llama, si se sabe; `solo_estos`: solo esos chats.
#[allow(clippy::too_many_arguments)]
pub fn vuelta_con_persona<D: Disco + ?Sized, F: Read + Write>(
    flujo: F,
    d: &D,
    a_quien: Option<&str>,
    solo_estos: Option<&[String]>,
    ahora: &dyn Fn() -> i64,
    mi_puerto: u32,
    nonce: [u8; 32],
    estado: &mut dyn FnMut(&str),
) -> Resultado<ConPersona> {
    let mut s =
        crate::protocolo::Sesion::conectar_con_persona(flujo, d, a_quien, ahora, mi_puerto, nonce)?;
    estado("Mirando quién está en cada chat…");
    s.juntar_miembros()?;
    let chats: Vec<String> = s
        .acceso()
        .map(|a| a.chats())
        .unwrap_or_default()
        .into_iter()
        .filter(|c| solo_estos.is_none_or(|l| l.contains(c)))
        .collect();
    let nombres: Vec<(String, String)> = s
        .catalogo()?
        .into_iter()
        .map(|c| (c.id, c.nombre))
        .collect();
    let mut hecho = crate::protocolo::Hecho::default();
    for chat in &chats {
        let nombre = nombres
            .iter()
            .find(|(id, _)| id == chat)
            .map_or(chat.as_str(), |(_, n)| n.as_str());
        estado(&format!("Juntando «{nombre}» con {}…", s.otro.nombre));
        let prep = s.preparar(chat)?;
        s.aplicar(&prep, &mut hecho)?;
        let arch = s.preparar_archivos(&prep)?;
        s.aplicar_archivos(&arch, &mut hecho, &mut |_| {})?;
        s.cerrar(&prep, ahora())?;
    }
    let bytes = s.enviados() + s.recibidos();
    let quien = s.otro.clone();
    let puerto = s.puerto_del_otro;
    s.adios();
    Ok(ConPersona {
        quien,
        puerto,
        chats,
        hecho,
        bytes,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn invitaciones_se_leen_del_qr_y_el_codigo_se_limpia() {
        let q = leer_qr(&texto_del_qr("K7Q2M9XP", Some("192.168.1.5"), 4000)).unwrap();
        assert_eq!(q.codigo, "K7Q2M9XP");
        assert_eq!(q.host.as_deref(), Some("192.168.1.5"));
        assert_eq!(q.puerto, 4000);
        assert_eq!(
            texto_del_qr("K7Q2M9XP", None, 4000),
            "pixpin-chat:1:K7Q2M9XP::4000"
        );
        assert!(
            leer_qr("pixpin-chat:1:K7Q2M9XP::4000")
                .unwrap()
                .host
                .is_none()
        );
        // Casos negativos: otro QR de PixPin, un codigo malo, poco texto.
        assert!(leer_qr("pixpin-envio:1:123456::0").is_none());
        assert!(leer_qr("pixpin-chat:1:K7Q2M9X0:h:1").is_none());
        assert!(leer_qr("pixpin-chat:1:K7Q2M9XP").is_none());
        assert_eq!(limpiar("k7q2-m9xp"), "K7Q2M9XP");
        assert_eq!(limpiar("k7q2-m9xp-zz"), "K7Q2M9XP");
        assert_eq!(legible("K7Q2M9XP"), "K7Q2-M9XP");
        for _ in 0..50 {
            assert!(valido(&nuevo_codigo()));
        }
        assert!(!valido("K7Q2M9X"));
        assert!(!valido("K7Q2M9XI"));
    }

    #[test]
    fn la_clave_es_la_del_grupo_con_chat_delante() {
        // `Grupo.clave("chat-K7Q2M9XP")`: PBKDF2 del texto entero, con su
        // guion; no la del codigo limpio.
        assert_eq!(clave("K7Q2M9XP"), clave_cruda("chat-K7Q2M9XP"));
        assert_ne!(clave("K7Q2M9XP"), crate::codigo::clave_de_grupo("K7Q2M9XP"));
        assert_ne!(clave("K7Q2M9XP"), clave("K7Q2M9XQ"));
        assert_eq!(etiqueta("K7Q2M9XP").len(), 16);
    }
}
