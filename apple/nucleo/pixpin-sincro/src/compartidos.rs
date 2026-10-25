//! **Chats compartidos con otras personas** (Android 13-oct-2026, v0.122.0).
//!
//! Puerto de `sincro/Compartidos.kt` de PixPin Android (commit `44c3a772`),
//! con su guia `docs/chats-compartidos.md`. Hasta aqui sincronizar era entre
//! los aparatos de uno mismo: el grupo comparte un codigo y quien lo sabe lo
//! ve todo. Un chat compartido es el chat de un proyecto que se lleva con
//! **otra persona**, que ve ese chat y nada mas, sin entrar en el grupo.
//!
//! - **Cada aparato tiene un par de claves P-256** ([Llaves]) que nace la
//!   primera vez que hace falta y no sale nunca de `sincro/persona.json`.
//! - **Un chat compartido** ([ChatCompartido]) lleva su lista de personas:
//!   de cada aparato, su id, el nombre de la persona, su codigo fijo
//!   (`K7Q2`, el que firma sus mensajes) y su clave publica.
//! - **Entrar y salir van firmados**: cada entrada la firma quien invita;
//!   cada salida, la propia persona o quien creo el chat. La lista viaja y
//!   se junta ([juntar]) sin servidor, y lo que no lleva una firma buena no
//!   se cree.
//!
//! **Todo es byte a byte lo de Android**: los textos firmados, el JSON de
//! `compartidos.json` y `persona.json` (kotlinx con `encodeDefaults = true`,
//! asi que los `null` se escriben), las claves en X.509/PKCS#8 base64 y la
//! firma `SHA256withECDSA` en DER. Si algo de aqui parece mejorable, hay que
//! cambiarlo tambien alli o dejan de entenderse.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use p256::ecdsa::signature::{Signer, Verifier};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey};
use p256::{PublicKey, SecretKey};
use serde::{Deserialize, Serialize};

use crate::disco::{Disco, escribir_atomico, permitida};

/// Lo mas largo de un nombre (`Compartidos.NOMBRE_MAX`).
pub const NOMBRE_MAX: usize = 40;

// ------------------------------------------------------------------ datos

/// Un aparato de una persona en un chat compartido (`Persona`).
///
/// El orden de los campos es el de Kotlin y no se toca: es el orden en que
/// se escribe el JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Persona {
    /// El id del aparato (`Aparato.id`).
    pub id: String,
    /// El nombre de la persona: «Ana». Lo pone ella.
    pub nombre: String,
    /// El codigo fijo de su aparato: el que llevan los mensajes que escribe.
    pub codigo: String,
    /// Su clave publica, X.509 (SPKI) DER en base64.
    pub clave: String,
    /// Cuando entro (la ultima vez).
    pub desde: i64,
    /// El id de quien la metio; ella misma si creo el chat.
    pub invito: String,
    /// La firma de `invito` sobre [texto_de_entrada].
    pub firma: String,
    /// Cuando salio o la quitaron, o 0.
    #[serde(default)]
    pub fuera: i64,
    /// Quien la saco: ella misma, o el creador.
    #[serde(default)]
    pub quito: Option<String>,
    /// La firma de `quito` sobre [texto_de_salida].
    #[serde(default, rename = "firmaFuera")]
    pub firma_fuera: Option<String>,
}

impl Persona {
    /// Esta dentro si entro despues de salir.
    pub fn dentro(&self) -> bool {
        self.fuera < self.desde
    }
}

/// **Un chat compartido**: que chat (el id del proyecto), como se llama,
/// quien lo creo y quien esta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatCompartido {
    pub chat: String,
    pub nombre: String,
    pub creador: String,
    pub creado: i64,
    #[serde(default)]
    pub personas: Vec<Persona>,
}

impl ChatCompartido {
    pub fn persona(&self, id: &str) -> Option<&Persona> {
        self.personas.iter().find(|p| p.id == id)
    }
}

/// Las listas en JSON, como `Protocolo.aTexto` (viajan como cadena).
pub fn a_texto(l: &[ChatCompartido]) -> String {
    serde_json::to_string(l).unwrap_or_else(|_| "[]".into())
}

/// `Protocolo.deTexto`: lo que no se entiende es una lista vacia.
pub fn de_texto(t: Option<&str>) -> Vec<ChatCompartido> {
    t.and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or_default()
}

/// `Protocolo.fichaATexto`.
pub fn ficha_a_texto(p: &Persona) -> String {
    serde_json::to_string(p).unwrap_or_default()
}

/// `Protocolo.fichaDeTexto`.
pub fn ficha_de_texto(t: Option<&str>) -> Option<Persona> {
    t.and_then(|t| serde_json::from_str(t).ok())
}

// ------------------------------------------------------------------ llaves

/// Lo que puede fallar con las claves.
#[derive(Debug, thiserror::Error)]
pub enum ErrorLlaves {
    #[error("clave que no se entiende")]
    ClaveRara,
    #[error("no hay azar en este equipo")]
    SinAzar,
}

/// **El par de claves de este aparato** (P-256). La privada no sale de
/// aqui: con ella se firma y se saca el secreto de cada conversacion.
#[derive(Clone)]
pub struct Llaves {
    /// La publica, X.509 (SPKI) DER: lo que viaja y por lo que se reconoce.
    publica: Vec<u8>,
    privada: SecretKey,
}

impl std::fmt::Debug for Llaves {
    // Que un `{:?}` descuidado no saque la privada a un registro.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Llaves")
            .field("publica", &b64(&self.publica))
            .finish_non_exhaustive()
    }
}

impl Llaves {
    /// Un par nuevo (`KeyPairGenerator("EC")` con `secp256r1`).
    pub fn nuevas() -> Llaves {
        let privada = SecretKey::random(&mut rand_core::OsRng);
        Llaves::de_privada(privada)
    }

    fn de_privada(privada: SecretKey) -> Llaves {
        let publica = privada
            .public_key()
            .to_public_key_der()
            .map(|d| d.as_bytes().to_vec())
            .unwrap_or_default();
        Llaves { publica, privada }
    }

    /// Las de `persona.json`: publica X.509 y privada PKCS#8, en base64.
    pub fn de(publica: &str, privada: &str) -> Result<Llaves, ErrorLlaves> {
        let der = desde_b64(privada).ok_or(ErrorLlaves::ClaveRara)?;
        let privada = SecretKey::from_pkcs8_der(&der).map_err(|_| ErrorLlaves::ClaveRara)?;
        let l = Llaves::de_privada(privada);
        // La publica guardada tiene que ser la de esa privada: si no, alguien
        // toco el fichero y no se firma con algo que no cuadra.
        let guardada = desde_b64(publica).ok_or(ErrorLlaves::ClaveRara)?;
        let suya = PublicKey::from_public_key_der(&guardada).map_err(|_| ErrorLlaves::ClaveRara)?;
        if suya != l.privada.public_key() {
            return Err(ErrorLlaves::ClaveRara);
        }
        Ok(Llaves {
            publica: guardada,
            privada: l.privada,
        })
    }

    pub fn publica(&self) -> &[u8] {
        &self.publica
    }

    /// La publica en base64: la `clave` de una [Persona].
    pub fn clave(&self) -> String {
        b64(&self.publica)
    }

    /// Para guardarla en `persona.json` (`privadaB64`). Solo la usa
    /// [CompartidosEnDisco]: la privada no sale del aparato.
    fn privada_b64(&self) -> String {
        self.privada
            .to_pkcs8_der()
            .map(|d| b64(d.as_bytes()))
            .unwrap_or_default()
    }

    /// El secreto compartido con quien tiene la privada de `suya` (ECDH, la
    /// coordenada x de 32 bytes, como `KeyAgreement("ECDH").generateSecret`).
    pub fn acordar(&self, suya: &[u8]) -> Result<[u8; 32], ErrorLlaves> {
        let otra = PublicKey::from_public_key_der(suya).map_err(|_| ErrorLlaves::ClaveRara)?;
        let s = p256::ecdh::diffie_hellman(self.privada.to_nonzero_scalar(), otra.as_affine());
        let mut salida = [0u8; 32];
        salida.copy_from_slice(s.raw_secret_bytes().as_slice());
        Ok(salida)
    }

    /// `SHA256withECDSA`, firma en DER y base64.
    pub fn firmar(&self, datos: &[u8]) -> String {
        let s: Signature = SigningKey::from(&self.privada).sign(datos);
        b64(s.to_der().as_bytes())
    }

    /// Si `firma` es de quien tiene la privada de `clave` (base64) sobre
    /// `datos`. Nunca falla: lo que no se entiende es «no».
    pub fn verificar(clave: &str, datos: &[u8], firma: &str) -> bool {
        let Some(der) = desde_b64(clave) else {
            return false;
        };
        let Ok(vk) = VerifyingKey::from_public_key_der(&der) else {
            return false;
        };
        let Some(f) = desde_b64(firma) else {
            return false;
        };
        let Ok(sig) = Signature::from_der(&f) else {
            return false;
        };
        // Java no normaliza la `s` (la mitad de sus firmas la llevan alta):
        // las dos son la misma firma, y se comprueba la baja.
        let sig = sig.normalize_s().unwrap_or(sig);
        vk.verify(datos, &sig).is_ok()
    }
}

pub fn b64(b: &[u8]) -> String {
    B64.encode(b)
}

pub fn desde_b64(s: &str) -> Option<Vec<u8>> {
    B64.decode(s.trim()).ok()
}

/// **Este aparato como persona**: lo que hace falta para firmar y
/// presentarse (`Yo`).
#[derive(Debug, Clone)]
pub struct Yo {
    pub id: String,
    pub nombre: String,
    pub codigo: String,
    pub llaves: Llaves,
}

impl Yo {
    pub fn clave(&self) -> String {
        self.llaves.clave()
    }

    /// La ficha sin firmar que se le da a quien invita.
    pub fn ficha(&self, ahora: i64) -> Persona {
        Persona {
            id: self.id.clone(),
            nombre: self.nombre.clone(),
            codigo: self.codigo.clone(),
            clave: self.clave(),
            desde: ahora,
            invito: String::new(),
            firma: String::new(),
            fuera: 0,
            quito: None,
            firma_fuera: None,
        }
    }
}

// ------------------------------------------------------------------ firmas

/// Lo que firma quien mete a `p` en `chat`: todo lo que dice de ella.
pub fn texto_de_entrada(chat: &str, p: &Persona) -> Vec<u8> {
    format!(
        "pixpin-chat|entra|{chat}|{}|{}|{}|{}|{}|{}",
        p.id, p.clave, p.desde, p.invito, p.nombre, p.codigo
    )
    .into_bytes()
}

/// Lo que firma quien saca a `p`. Un `quito` nulo se escribe `null`, como
/// la plantilla de Kotlin.
pub fn texto_de_salida(chat: &str, p: &Persona) -> Vec<u8> {
    format!(
        "pixpin-chat|sale|{chat}|{}|{}|{}",
        p.id,
        p.fuera,
        p.quito.as_deref().unwrap_or("null")
    )
    .into_bytes()
}

/// Si la entrada de `p` esta firmada por quien dice: el creador se firma a
/// si mismo; los demas, quien los invito, que tiene que estar ya en
/// `conocidas`.
pub fn entrada_valida(c: &ChatCompartido, p: &Persona, conocidas: &[Persona]) -> bool {
    let firmante = if p.invito == p.id {
        if p.id == c.creador {
            p.clave.as_str()
        } else {
            return false;
        }
    } else {
        match conocidas.iter().find(|x| x.id == p.invito) {
            Some(x) => x.clave.as_str(),
            None => return false,
        }
    };
    Llaves::verificar(firmante, &texto_de_entrada(&c.chat, p), &p.firma)
}

/// Si la salida de `p` (si la tiene) esta firmada por ella misma o por el
/// creador.
pub fn salida_valida(c: &ChatCompartido, p: &Persona, conocidas: &[Persona]) -> bool {
    if p.fuera <= 0 {
        return true;
    }
    let Some(firma) = p.firma_fuera.as_deref() else {
        return false;
    };
    let firmante = match p.quito.as_deref() {
        Some(q) if q == p.id => p.clave.as_str(),
        Some(q) if q == c.creador => match conocidas.iter().find(|x| x.id == c.creador) {
            Some(x) => x.clave.as_str(),
            None => return false,
        },
        _ => return false,
    };
    Llaves::verificar(firmante, &texto_de_salida(&c.chat, p), firma)
}

// ------------------------------------------------------------------ juntar

/// **Junta la lista de `suyo` con la mia** (`Compartidos.juntar`). Por id de
/// aparato: de la entrada se queda la mas reciente bien firmada, y de la
/// salida igual. Lo que no cuadra se ignora: una firma mala, alguien de
/// quien no se conoce a quien lo invito, o una clave distinta para un
/// aparato que ya se conoce. El creador y el nombre del chat son los que ya
/// habia; si `mio` es nulo, los de `suyo`.
pub fn juntar(mio: Option<&ChatCompartido>, suyo: &ChatCompartido) -> ChatCompartido {
    if let Some(m) = mio
        && m.chat != suyo.chat
    {
        return m.clone();
    }
    let base = match mio {
        Some(m) => m.clone(),
        None => ChatCompartido {
            personas: Vec::new(),
            ..suyo.clone()
        },
    };
    // Un `LinkedHashMap` de Kotlin: el orden de llegada, una fila por id.
    let mut personas: Vec<Persona> = Vec::new();
    for p in mio.map(|m| m.personas.as_slice()).unwrap_or(&[]) {
        match personas.iter_mut().find(|x| x.id == p.id) {
            Some(x) => *x = p.clone(),
            None => personas.push(p.clone()),
        }
    }
    let mut pendientes: Vec<Persona> = suyo.personas.clone();
    // Por vueltas: alguien invitado por otro que viene detras en la lista se
    // valida en la siguiente.
    while !pendientes.is_empty() {
        let mut quedan = Vec::new();
        let mut cambio = false;
        for p in &pendientes {
            let actual = personas.iter().find(|x| x.id == p.id).cloned();
            if let Some(a) = &actual
                && a.clave != p.clave
            {
                continue;
            }
            let entra = entrada_valida(&base, p, &personas);
            if actual.is_none() && !entra {
                quedan.push(p.clone());
                continue;
            }
            let sale = if actual.is_none() {
                let mut con = personas.clone();
                con.push(p.clone());
                salida_valida(&base, p, &con)
            } else {
                salida_valida(&base, p, &personas)
            };
            let mut r = actual.clone();
            if entra && r.as_ref().is_none_or(|r| p.desde > r.desde) {
                let previa = r.unwrap_or_else(|| Persona {
                    fuera: 0,
                    quito: None,
                    firma_fuera: None,
                    ..p.clone()
                });
                r = Some(Persona {
                    nombre: p.nombre.clone(),
                    codigo: p.codigo.clone(),
                    desde: p.desde,
                    invito: p.invito.clone(),
                    firma: p.firma.clone(),
                    ..previa
                });
            }
            if let Some(x) = &mut r
                && sale
                && p.fuera > x.fuera
            {
                x.fuera = p.fuera;
                x.quito = p.quito.clone();
                x.firma_fuera = p.firma_fuera.clone();
            }
            if let Some(x) = r
                && Some(&x) != actual.as_ref()
            {
                match personas.iter_mut().find(|y| y.id == x.id) {
                    Some(y) => *y = x,
                    None => personas.push(x),
                }
                cambio = true;
            }
        }
        if !cambio || quedan.len() == pendientes.len() {
            break;
        }
        pendientes = quedan;
    }
    ChatCompartido { personas, ..base }
}

/// Junta listas enteras, chat por chat. Lo de chats que no tengo solo entra
/// con `admitir_nuevos`.
pub fn juntar_todos(
    mios: &[ChatCompartido],
    suyos: &[ChatCompartido],
    admitir_nuevos: impl Fn(&ChatCompartido) -> bool,
) -> Vec<ChatCompartido> {
    let mut por_chat: Vec<ChatCompartido> = Vec::new();
    for m in mios {
        match por_chat.iter_mut().find(|x| x.chat == m.chat) {
            Some(x) => *x = m.clone(),
            None => por_chat.push(m.clone()),
        }
    }
    for s in suyos {
        let i = por_chat.iter().position(|x| x.chat == s.chat);
        if i.is_none() && !admitir_nuevos(s) {
            continue;
        }
        let junto = juntar(i.map(|i| &por_chat[i]), s);
        match i {
            Some(i) => por_chat[i] = junto,
            None => por_chat.push(junto),
        }
    }
    por_chat
}

/// **Entre aparatos del grupo** (los de una misma persona): se juntan todas
/// las listas —tambien las de chats que aqui no habia— y, si llega la ficha
/// del `otro` aparato, lo meto yo en los chats compartidos donde estoy
/// dentro y el aun no consta. Si consta y salio, no: se fue a proposito.
pub fn juntar_del_grupo(
    mios: &[ChatCompartido],
    suyos: &[ChatCompartido],
    yo: &Yo,
    otro: Option<&Persona>,
    ahora: i64,
) -> Vec<ChatCompartido> {
    let juntos = juntar_todos(mios, suyos, |_| true);
    let Some(otro) = otro.filter(|o| !o.clave.trim().is_empty() && o.id != yo.id) else {
        return juntos;
    };
    juntos
        .into_iter()
        .map(|c| {
            if dentro(&c, &yo.id) && c.persona(&otro.id).is_none() {
                invitar(&c, yo, otro, ahora).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

// ---------------------------------------------------- crear, invitar, quitar

/// **Compartir un chat**: la lista con una sola persona, quien lo comparte,
/// que es su creador.
pub fn crear(chat: &str, nombre: &str, yo: &Yo, ahora: i64) -> ChatCompartido {
    let mut p = Persona {
        invito: yo.id.clone(),
        ..yo.ficha(ahora)
    };
    p.firma = yo.llaves.firmar(&texto_de_entrada(chat, &p));
    ChatCompartido {
        chat: chat.into(),
        nombre: nombre.into(),
        creador: yo.id.clone(),
        creado: ahora,
        personas: vec![p],
    }
}

/// **`yo` mete a `nueva` en el chat**, firmando su entrada. Si ya estaba
/// (con su misma clave) y se habia ido, vuelve; si esta dentro, no cambia
/// nada. Solo invita quien esta dentro.
pub fn invitar(
    c: &ChatCompartido,
    yo: &Yo,
    nueva: &Persona,
    ahora: i64,
) -> Result<ChatCompartido, String> {
    if !dentro(c, &yo.id) {
        return Err("Ya no estás en este chat".into());
    }
    let antes = c.persona(&nueva.id);
    if antes.is_some_and(|a| a.clave != nueva.clave) {
        return Err("Ese aparato ya está con otra clave".into());
    }
    if antes.is_some_and(Persona::dentro) {
        return Ok(c.clone());
    }
    let desde = ahora.max(antes.map_or(0, |a| a.fuera) + 1);
    let nombre = if nueva.nombre.trim().is_empty() {
        "Sin nombre".to_string()
    } else {
        nueva.nombre.chars().take(NOMBRE_MAX).collect()
    };
    let mut p = Persona {
        id: nueva.id.clone(),
        nombre,
        codigo: nueva.codigo.clone(),
        clave: nueva.clave.clone(),
        desde,
        invito: yo.id.clone(),
        firma: String::new(),
        fuera: antes.map_or(0, |a| a.fuera),
        quito: antes.and_then(|a| a.quito.clone()),
        firma_fuera: antes.and_then(|a| a.firma_fuera.clone()),
    };
    p.firma = yo.llaves.firmar(&texto_de_entrada(&c.chat, &p));
    let mut personas: Vec<Persona> = c
        .personas
        .iter()
        .filter(|x| x.id != p.id)
        .cloned()
        .collect();
    personas.push(p);
    Ok(ChatCompartido {
        personas,
        ..c.clone()
    })
}

/// Si `yo` puede sacar a `id`: a si mismo siempre (es irse), a otro solo el
/// creador.
pub fn puede_quitar(c: &ChatCompartido, yo: &str, id: &str) -> bool {
    dentro(c, id) && (yo == id || (yo == c.creador && dentro(c, yo)))
}

/// **`yo` saca a `id`** (o se va, si es el mismo), firmandolo.
pub fn quitar(c: &ChatCompartido, yo: &Yo, id: &str, ahora: i64) -> Result<ChatCompartido, String> {
    if !puede_quitar(c, &yo.id, id) {
        return Err("Solo quien creó el chat puede quitar a otros".into());
    }
    let p = c.persona(id).ok_or("No está en este chat")?;
    let mut sin = Persona {
        fuera: ahora.max(p.desde + 1),
        quito: Some(yo.id.clone()),
        firma_fuera: None,
        ..p.clone()
    };
    sin.firma_fuera = Some(yo.llaves.firmar(&texto_de_salida(&c.chat, &sin)));
    Ok(ChatCompartido {
        personas: c
            .personas
            .iter()
            .map(|x| if x.id == id { sin.clone() } else { x.clone() })
            .collect(),
        ..c.clone()
    })
}

// ------------------------------------------------------------------ preguntas

pub fn dentro(c: &ChatCompartido, id: &str) -> bool {
    c.personas.iter().any(|p| p.id == id && p.dentro())
}

/// Los chats en los que **los dos** estan dentro: los unicos cuyo contenido
/// viaja entre ellos.
pub fn en_comun(lista: &[ChatCompartido], yo: &str, otro: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for c in lista.iter().filter(|c| dentro(c, yo) && dentro(c, otro)) {
        if !v.contains(&c.chat) {
            v.push(c.chat.clone());
        }
    }
    v
}

/// Los chats en los que los dos **constan**, dentro o no: de esos se junta
/// la lista, para que se entere de una salida tambien quien estaba lejos.
pub fn con_rastro(lista: &[ChatCompartido], yo: &str, otro: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for c in lista
        .iter()
        .filter(|c| c.persona(yo).is_some() && c.persona(otro).is_some())
    {
        if !v.contains(&c.chat) {
            v.push(c.chat.clone());
        }
    }
    v
}

/// La persona que tiene esa clave publica, en cualquiera de los chats donde
/// tambien estoy yo.
pub fn por_clave(lista: &[ChatCompartido], yo: &str, clave: &str) -> Option<Persona> {
    lista
        .iter()
        .filter(|c| c.persona(yo).is_some())
        .flat_map(|c| c.personas.iter())
        .find(|p| p.clave == clave && p.id != yo)
        .cloned()
}

/// **Quien escribio cada mensaje**: del codigo del aparato (`aparato` del
/// mensaje) al nombre de la persona.
pub fn autores(c: &ChatCompartido) -> HashMap<String, String> {
    let mut v: Vec<&Persona> = c.personas.iter().collect();
    v.sort_by_key(|p| p.desde);
    v.into_iter()
        .map(|p| (p.codigo.clone(), p.nombre.clone()))
        .collect()
}

/// Las personas (no los aparatos) que estan dentro, cada una una vez.
pub fn nombres_dentro(c: &ChatCompartido) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for p in c.personas.iter().filter(|p| p.dentro()) {
        if !v.contains(&p.nombre) {
            v.push(p.nombre.clone());
        }
    }
    v
}

// ------------------------------------------------------------------ disco

/// Un cerrojo para todo el proceso, como `CompartidosEnDisco.CERROJO`: la
/// pantalla, la escucha y la sincronizacion escriben la misma lista.
static CERROJO: Mutex<()> = Mutex::new(());
/// Sube cada vez que se guarda algo: las pantallas lo miran para releer.
static VERSION: AtomicU64 = AtomicU64::new(0);

/// `CompartidosEnDisco.version`.
pub fn version() -> u64 {
    VERSION.load(Ordering::SeqCst)
}

/// `persona.json`: la ficha de este aparato.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MiFicha {
    publica: String,
    privada: String,
    #[serde(default)]
    nombre: Option<String>,
}

/// **Lo guardado de los chats compartidos** (`CompartidosEnDisco`): la lista
/// en `<raiz>/sincro/compartidos.json` y las claves y el nombre de este
/// aparato en `<raiz>/sincro/persona.json`.
#[derive(Debug, Clone)]
pub struct CompartidosEnDisco {
    raiz: PathBuf,
}

impl CompartidosEnDisco {
    /// `raiz` es la de los datos (`files` en el movil), no `sincro`.
    pub fn nuevo(raiz: &Path) -> CompartidosEnDisco {
        CompartidosEnDisco {
            raiz: raiz.to_path_buf(),
        }
    }

    fn carpeta(&self) -> PathBuf {
        self.raiz.join("sincro")
    }

    fn archivo(&self) -> PathBuf {
        self.carpeta().join("compartidos.json")
    }

    fn archivo_de_persona(&self) -> PathBuf {
        self.carpeta().join("persona.json")
    }

    fn leer_sin_cerrojo(&self) -> Vec<ChatCompartido> {
        std::fs::read_to_string(self.archivo())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn leer(&self) -> Vec<ChatCompartido> {
        let _g = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
        self.leer_sin_cerrojo()
    }

    pub fn de(&self, chat: &str) -> Option<ChatCompartido> {
        self.leer().into_iter().find(|c| c.chat == chat)
    }

    fn guardar_sin_cerrojo(&self, lista: &[ChatCompartido]) -> io::Result<()> {
        escribir_atomico(&self.archivo(), a_texto(lista).as_bytes())?;
        VERSION.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn guardar(&self, lista: &[ChatCompartido]) -> io::Result<()> {
        let _g = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
        self.guardar_sin_cerrojo(lista)
    }

    /// Lee, cambia y guarda de una vez, sin que otro hilo escriba en medio.
    /// Devuelve lo guardado. Si `f` falla no se guarda nada.
    pub fn cambiar<E>(
        &self,
        f: impl FnOnce(Vec<ChatCompartido>) -> Result<Vec<ChatCompartido>, E>,
    ) -> Result<Vec<ChatCompartido>, E>
    where
        E: From<io::Error>,
    {
        let _g = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
        let antes = self.leer_sin_cerrojo();
        let despues = f(antes.clone())?;
        if despues != antes {
            self.guardar_sin_cerrojo(&despues)?;
        }
        Ok(despues)
    }

    /// Pone (o sustituye) la lista de un chat.
    pub fn poner(&self, c: &ChatCompartido) -> io::Result<()> {
        self.cambiar(|l| -> io::Result<_> {
            let mut l: Vec<ChatCompartido> = l.into_iter().filter(|x| x.chat != c.chat).collect();
            l.push(c.clone());
            Ok(l)
        })
        .map(|_| ())
    }

    /// Si este aparato esta dentro de algun chat compartido: entonces hay que
    /// escuchar en la Wi-Fi aunque no haya grupo.
    pub fn hay_alguno(&self, yo: &str) -> bool {
        self.leer().iter().any(|c| dentro(c, yo))
    }

    fn ficha(&self) -> io::Result<MiFicha> {
        if let Some(f) = std::fs::read_to_string(self.archivo_de_persona())
            .ok()
            .and_then(|t| serde_json::from_str::<MiFicha>(&t).ok())
            .filter(|f| Llaves::de(&f.publica, &f.privada).is_ok())
        {
            return Ok(f);
        }
        let l = Llaves::nuevas();
        let nueva = MiFicha {
            publica: l.clave(),
            privada: l.privada_b64(),
            nombre: None,
        };
        self.guardar_ficha(&nueva)?;
        Ok(nueva)
    }

    fn guardar_ficha(&self, f: &MiFicha) -> io::Result<()> {
        let texto = serde_json::to_string(f).map_err(io::Error::other)?;
        escribir_atomico(&self.archivo_de_persona(), texto.as_bytes())
    }

    pub fn llaves(&self) -> io::Result<Llaves> {
        let _g = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
        let f = self.ficha()?;
        Llaves::de(&f.publica, &f.privada).map_err(io::Error::other)
    }

    /// El nombre con el que me ven en los chats compartidos, si lo puse.
    pub fn nombre(&self) -> Option<String> {
        let _g = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
        self.ficha().ok().and_then(|f| f.nombre)
    }

    pub fn poner_nombre(&self, nombre: &str) -> io::Result<()> {
        let _g = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
        let mut f = self.ficha()?;
        let limpio: String = nombre.trim().chars().take(NOMBRE_MAX).collect();
        f.nombre = (!limpio.is_empty()).then_some(limpio);
        self.guardar_ficha(&f)?;
        VERSION.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Yo, para firmar y presentarme: el id y el codigo del aparato, mi
    /// nombre y mis claves.
    pub fn yo(&self, id: &str, nombre_del_aparato: &str) -> io::Result<Yo> {
        Ok(Yo {
            id: id.into(),
            nombre: self.nombre().unwrap_or_else(|| nombre_del_aparato.into()),
            codigo: crate::grupo::codigo_de_aparato(id),
            llaves: self.llaves()?,
        })
    }

    /// [CompartidosEnDisco::yo] con la identidad de un [Disco].
    pub fn yo_de<D: Disco + ?Sized>(&self, d: &D) -> io::Result<Yo> {
        let i = d.identidad()?;
        self.yo(&i.yo.id, &i.yo.nombre)
    }
}

// ------------------------------------------------------------------ la valla

/// **Lo que una persona puede tocar en una conversacion** con este aparato
/// (`AccesoDePersona`): los chats en los que estamos dentro los dos, y la
/// valla de archivos de todos los demas. Se vuelve a mirar a cada pregunta:
/// si a mitad de conversacion la lista cambia, deja de valer al momento.
///
/// **En el PC la valla va por sitio en disco y no por ruta portatil**: aqui
/// cada proyecto tiene su carpeta y `pins/draw/x.excalidraw.gz` de un chat
/// y de otro son ficheros distintos. Lo que cuenta es a que fichero de
/// verdad llevaria la ruta (`Disco::ruta`), que tambien atrapa una ruta que
/// salta a la carpeta de otro proyecto (`guardados/pc/<otro>/…`). En el
/// movil (un solo `files`) sitio y ruta son lo mismo y queda como alli.
pub struct AccesoDePersona<'a, D: Disco + ?Sized> {
    disco: &'a D,
    compartidos: CompartidosEnDisco,
    pub yo: String,
    pub otro: String,
    prohibidos_de: Mutex<Option<(Vec<String>, HashSet<PathBuf>)>>,
}

impl<'a, D: Disco + ?Sized> AccesoDePersona<'a, D> {
    pub fn nuevo(disco: &'a D, yo: &str, otro: &str) -> Self {
        AccesoDePersona {
            disco,
            compartidos: CompartidosEnDisco::nuevo(&disco.raiz()),
            yo: yo.into(),
            otro: otro.into(),
            prohibidos_de: Mutex::new(None),
        }
    }

    pub fn chats(&self) -> Vec<String> {
        en_comun(&self.compartidos.leer(), &self.yo, &self.otro)
    }

    /// Los ficheros de los chats que no compartimos, calculados una vez por
    /// conversacion (y por lista).
    pub fn prohibidos(&self) -> HashSet<PathBuf> {
        let c = self.chats();
        let mut g = self.prohibidos_de.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((de, p)) = g.as_ref()
            && *de == c
        {
            return p.clone();
        }
        let p = prohibidos_para(self.disco, &c);
        *g = Some((c, p.clone()));
        p
    }

    pub fn puede(&self, chat: &str) -> bool {
        self.chats().iter().any(|c| c == chat)
    }

    /// Si `rel` de `chat` lleva a un fichero de otro chat.
    pub fn vedado(&self, chat: &str, rel: &str) -> bool {
        self.valla().vedado(chat, rel)
    }

    /// La valla de ahora, suelta: se puede usar mientras se habla con el
    /// otro sin tener prestado el acceso.
    pub fn valla(&self) -> Valla<'a, D> {
        Valla {
            disco: self.disco,
            prohibidos: self.prohibidos(),
        }
    }

    /// Si un archivo se puede leer: es de ese chat y de ninguno mas.
    pub fn puede_leer(&self, chat: &str, rel: &str, de_este_chat: &HashSet<String>) -> bool {
        self.puede(chat) && permitida(rel) && de_este_chat.contains(rel) && !self.vedado(chat, rel)
    }

    /// Si un archivo se puede escribir: no es de otro chat y, si ya existe,
    /// es de este. Lo nuevo (un adjunto que acaba de llegar) si.
    pub fn puede_escribir(&self, chat: &str, rel: &str) -> bool {
        self.puede(chat)
            && permitida(rel)
            && !self.vedado(chat, rel)
            && (!self.disco.ruta(chat, rel).exists()
                || self
                    .disco
                    .alcance(chat)
                    .map(|a| a.iter().any(|(r, _)| r == rel))
                    .unwrap_or(false))
    }
}

/// Los ficheros de los otros chats, ya calculados (ver
/// [AccesoDePersona::valla]).
pub struct Valla<'a, D: Disco + ?Sized> {
    disco: &'a D,
    prohibidos: HashSet<PathBuf>,
}

impl<D: Disco + ?Sized> Valla<'_, D> {
    /// Si `rel` de `chat` lleva a un fichero de otro chat.
    pub fn vedado(&self, chat: &str, rel: &str) -> bool {
        salta_de_carpeta(chat, rel) || self.prohibidos.contains(&self.disco.ruta(chat, rel))
    }
}

/// `guardados/pc/<carpeta>/…` es, en el PC, la carpeta de un proyecto: de
/// una persona solo vale la del chat que comparte. (En el movil no existe
/// esa ruta y nunca salta.)
fn salta_de_carpeta(chat: &str, rel: &str) -> bool {
    match rel
        .strip_prefix("guardados/pc/")
        .and_then(|x| x.split_once('/'))
    {
        Some((c, _)) => c != crate::disco::limpio(chat),
        None => false,
    }
}

/// **Lo que no es de `permitidos`** (`Disco.prohibidosPara`): los ficheros
/// de todos los demas chats, la conversacion general incluida. Lo que esta
/// en los dos (un adjunto reenviado) se queda fuera: ante la duda, no se da.
pub fn prohibidos_para<D: Disco + ?Sized>(d: &D, permitidos: &[String]) -> HashSet<PathBuf> {
    let mut chats: Vec<String> = d
        .chats()
        .map(|l| l.into_iter().map(|c| c.id).collect())
        .unwrap_or_default();
    if !chats.iter().any(|c| c == crate::disco::GENERAL) {
        chats.push(crate::disco::GENERAL.into());
    }
    let mut salida = HashSet::new();
    for c in chats.iter().filter(|c| !permitidos.contains(c)) {
        for (rel, _) in d.alcance(c).unwrap_or_default() {
            salida.insert(d.ruta(c, &rel));
        }
    }
    salida
}

#[cfg(test)]
#[path = "compartidos/pruebas.rs"]
mod pruebas;

/// Unas llaves que ensenan la publica de `publica` y firman con la privada
/// de `privada`: el impostor de las pruebas del canal.
#[cfg(test)]
pub(crate) fn mezcladas(publica: &Llaves, privada: &Llaves) -> Llaves {
    Llaves {
        publica: publica.publica.clone(),
        privada: privada.privada.clone(),
    }
}
