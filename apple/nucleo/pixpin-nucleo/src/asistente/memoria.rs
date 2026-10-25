//! **La memoria de largo plazo del asistente** (Windows,
//! `asistente/memoria.rs`, 22-oct-2026): el almacen de `pixpin_ia::memoria`
//! en una carpeta que viaja a los aparatos propios, y lo que la app hace con
//! ella (meterla en cada turno de la nube, guardar lo duradero al reiniciar
//! y al empezar una conversacion nueva).
//!
//! **Donde vive** (el mismo sitio que Windows, para que sincronice sin
//! cambios): `proyectos/<asistente>/android/guardados/asistente/memoria/`,
//! que para el movil es `pixpin:files/guardados/asistente/memoria/`.
//! **Como viaja**: un mensaje ARCHIVO por fichero con id fijo
//! `auto-memoria-<ruta>` ([`id_de`]): la sincronizacion manda solo lo que
//! cambio, dos aparatos que escriben temas distintos no se pisan, y borrar
//! un tema quita su mensaje con marca y con el el fichero en el otro
//! aparato. Esos mensajes son de sistema (`auto-`): no son burbujas.
//!
//! **Solo la nube** la usa, como en Windows: la IA local esta congelada y
//! su ventana de 8192 tokens no da para mas.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pixpin_ia::memoria::{Almacen, EnCarpeta, Memoria};
use pixpin_proyecto::cuaderno::{self, Clase, Cuaderno, Mensaje, Sello};

use crate::Nucleo;

/// Dentro de la carpeta del chat «Asistente».
pub const EN_EL_CHAT: &str = "android/guardados/asistente/memoria";
/// Lo mismo para el movil.
pub const PORTATIL: &str = "pixpin:files/guardados/asistente/memoria/";
/// El principio del id del mensaje de cada fichero.
pub const PREFIJO_ID: &str = "auto-memoria-";
/// El texto del mensaje (el de Windows).
const TEXTO: &str = "🧠 Memoria del asistente (lo que recuerda entre conversaciones; solo va a tus aparatos)";

/// El id fijo del mensaje del fichero `rel` (`PERFIL.md`, `temas/x.md`).
pub fn id_de(rel: &str) -> String {
    format!("{PREFIJO_ID}{}", pixpin_sincro::disco::limpio(&rel.to_lowercase()))
}

/// La carpeta de la memoria dentro de la carpeta del chat «Asistente».
pub fn carpeta_en(carpeta_del_chat: &Path) -> PathBuf {
    EN_EL_CHAT.split('/').fold(carpeta_del_chat.to_path_buf(), |p, s| p.join(s))
}

/// **El almacen que viaja**: la carpeta de siempre, y por cada fichero que
/// se escribe o se borra, su mensaje ARCHIVO en el chat «Asistente».
pub struct AlmacenQueViaja {
    carpeta: EnCarpeta,
    raiz: PathBuf,
    proyecto: String,
    carpeta_del_chat: PathBuf,
    aparato: String,
}

impl AlmacenQueViaja {
    pub fn nuevo(raiz: &Path, proyecto: &str, carpeta_del_chat: &Path, aparato: &str) -> AlmacenQueViaja {
        AlmacenQueViaja {
            carpeta: EnCarpeta::nueva(carpeta_en(carpeta_del_chat)),
            raiz: raiz.to_path_buf(),
            proyecto: proyecto.to_string(),
            carpeta_del_chat: carpeta_del_chat.to_path_buf(),
            aparato: aparato.to_string(),
        }
    }

    /// Pone el mensaje del fichero si no esta (el fichero cambia, el mensaje
    /// es el mismo: la sincronizacion mira el fichero).
    fn senalar(&self, rel: &str) -> std::io::Result<()> {
        let c = match Cuaderno::leer_de(&self.carpeta_del_chat) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
            Err(e) => return Err(e),
        };
        let id = id_de(rel);
        if c.mensajes.iter().any(|m| m.id == id) {
            return Ok(());
        }
        let numero = c.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
        let bytes = std::fs::metadata(self.carpeta.ruta(rel)).map(|m| m.len() as i64).unwrap_or(0);
        let nombre = rel.rsplit('/').next().unwrap_or(rel);
        let mut m = Mensaje::adjunto(
            Clase::Archivo,
            nombre,
            &format!("{PORTATIL}{rel}"),
            bytes,
            &Sello {
                cuando: crate::ahora_ms(),
                numero,
                aparato: self.aparato.clone(),
                proyecto: self.proyecto.clone(),
            },
        );
        m.id = id;
        m.texto = TEXTO.into();
        cuaderno::anadir(&self.carpeta_del_chat, &m)
    }

    /// Quita el mensaje del fichero **con su marca de borrado**: asi el otro
    /// aparato lo quita tambien, y con el su fichero.
    fn dejar_de_senalar(&self, rel: &str) -> std::io::Result<()> {
        let ids: BTreeSet<String> = [id_de(rel)].into();
        let quitados = match cuaderno::quitar(&self.carpeta_del_chat, &ids) {
            Ok(q) => q,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        if !quitados.is_empty() {
            pixpin_proyecto::vista::anotar_borrados(&self.raiz, &self.proyecto, &quitados, crate::ahora_ms())?;
        }
        Ok(())
    }
}

impl Almacen for AlmacenQueViaja {
    fn leer(&self, rel: &str) -> std::io::Result<Option<String>> {
        self.carpeta.leer(rel)
    }

    fn escribir(&self, rel: &str, texto: &str) -> std::io::Result<()> {
        self.carpeta.escribir(rel, texto)?;
        // Sin su mensaje el fichero no viaja, pero se queda escrito.
        let _ = self.senalar(rel);
        Ok(())
    }

    fn borrar(&self, rel: &str) -> std::io::Result<()> {
        self.carpeta.borrar(rel)?;
        let _ = self.dejar_de_senalar(rel);
        Ok(())
    }

    fn listar(&self) -> std::io::Result<Vec<String>> {
        self.carpeta.listar()
    }
}

/// El dia de hoy en la hora del usuario: la fecha de lo que se guarda.
pub fn hoy(desfase_ms: i64) -> String {
    pixpin_ia::memoria::fecha_de_ms(crate::ahora_ms() + desfase_ms)
}

/// **La memoria de este nucleo** (creando el chat «Asistente» si no esta),
/// con `quien` como procedencia de lo que se escriba. `None` si el chat no
/// se puede crear: el turno sigue sin memoria.
pub fn de(nucleo: &Nucleo, quien: &str, desfase_ms: i64) -> Option<Memoria> {
    let aparato = nucleo.identidad().ok()?.yo.codigo();
    let (proyecto, carpeta) = crate::automatizaciones::chat_del_asistente(nucleo.ruta(), &aparato, crate::ahora_ms())
        .ok()?;
    Some(en(nucleo.ruta(), &proyecto, &carpeta, &aparato, quien, &hoy(desfase_ms)))
}

/// Lo mismo con todo dado (las pruebas).
pub fn en(raiz: &Path, proyecto: &str, carpeta_del_chat: &Path, aparato: &str, quien: &str, hoy: &str) -> Memoria {
    Memoria::nueva(Arc::new(AlmacenQueViaja::nuevo(raiz, proyecto, carpeta_del_chat, aparato)), quien, hoy)
}

/// **El bloque de la memoria para el sistema** (perfil e indice), vacio si
/// no hay nada o no hay chat.
pub fn bloque(m: Option<&Memoria>) -> String {
    m.map(|m| m.bloque_para_el_sistema(pixpin_ia::memoria::PRESUPUESTO_SISTEMA))
        .unwrap_or_default()
}

/// **Guarda lo duradero en segundo plano** con este motor: lo de
/// `conversacion` que se va a dejar de ver. No espera: ni el turno ni
/// «Conversación nueva» anaden latencia.
pub fn guardar_en_segundo_plano(motor: Arc<dyn pixpin_ia::Motor>, memoria: Memoria, conversacion: Vec<(bool, String)>) {
    if !conversacion.iter().any(|(u, t)| *u && !t.trim().is_empty()) {
        return;
    }
    // Si falla, la memoria sigue como estaba: no hay nadie a quien decirlo.
    let _ = std::thread::Builder::new().name("asistente-memoria".into()).spawn(move || {
        let t = pixpin_ia::memoria::transcripcion(&conversacion);
        let _ = pixpin_ia::memoria::guardar_lo_duradero(motor.as_ref(), &memoria, &t);
    });
}

/// **Al cerrar una conversacion** («Conversación nueva»): lo duradero de la
/// que se cierra, con Claude Haiku y solo si el usuario usa la nube y tiene
/// su clave (Windows). Sin clave no se hace nada: la memoria sigue como
/// estaba.
pub fn al_cerrar(nucleo: &Nucleo, conversacion: Vec<(bool, String)>, desfase_ms: i64) {
    use pixpin_ia::nube::{MotorConRespaldo, MotorNube, Proveedor, claves};
    let Ok(a) = pixpin_store::ajustes::cargar(&nucleo.ubicacion()) else { return };
    if Proveedor::por_id(&a.asistente.motor).is_none() || !pixpin_ia::nube::http::hay_transporte() {
        return;
    }
    let Some(clave) = claves::leer(Proveedor::Claude) else { return };
    let nube: Arc<dyn pixpin_ia::Motor> = Arc::new(MotorNube::nuevo(Proveedor::Claude, clave).pensando(false));
    let motor: Arc<dyn pixpin_ia::Motor> = Arc::new(MotorConRespaldo::nuevo(nube, None, Proveedor::Claude.nombre()));
    let Some(m) = de(nucleo, Proveedor::Claude.nombre(), desfase_ms) else { return };
    guardar_en_segundo_plano(motor, m, conversacion);
}

#[uniffi::export]
impl Nucleo {
    /// **La carpeta de la memoria del asistente** (la franja del chat
    /// «Asistente» de Windows: ahi el usuario la lee y la cambia con
    /// cualquier editor), creandola si no esta.
    pub fn ia_carpeta_de_memoria(&self) -> Result<String, crate::ErrorNucleo> {
        let aparato = self.identidad()?.yo.codigo();
        let (_, carpeta) = crate::automatizaciones::chat_del_asistente(self.ruta(), &aparato, crate::ahora_ms())?;
        let dir = carpeta_en(&carpeta);
        std::fs::create_dir_all(&dir)?;
        Ok(dir.display().to_string())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::almacen;

    fn raiz_de_prueba(nombre: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("pixpin-memoria-mac-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    #[test]
    fn cada_fichero_de_memoria_viaja_con_su_mensaje_y_se_va_con_el() {
        let raiz = raiz_de_prueba("viaja");
        let f = almacen::asegurar_asistente(&raiz, 1, "MAC01").unwrap();
        let carpeta = almacen::carpeta(&raiz, &f.id);
        let m = en(&raiz, &f.id, &carpeta, "MAC01", "Claude Haiku 5.5", "2026-10-22");
        let r = m.ejecutar(r#"{"command":"create","path":"/memoria/PERFIL.md","file_text":"Habla espanol."}"#);
        assert!(r.contains("creado"), "{r}");
        m.ejecutar(r##"{"command":"create","path":"/memoria/temas/obra.md","file_text":"# La obra de la calle Mayor"}"##);
        m.ejecutar(r#"{"command":"str_replace","path":"/memoria/PERFIL.md","old_str":"espanol","new_str":"espanol, corto"}"#);
        let c = Cuaderno::leer_de(&carpeta).unwrap();
        let ids: Vec<&str> = c.mensajes.iter().map(|m| m.id.as_str()).filter(|i| i.starts_with(PREFIJO_ID)).collect();
        // PERFIL, el tema y el indice: uno cada uno aunque se escriba dos veces.
        assert_eq!(ids.len(), 3, "{ids:?}");
        assert!(c.mensajes.iter().all(|m| !m.id.starts_with(PREFIJO_ID) || cuaderno::es_de_sistema(m)));
        // La ruta portatil es el fichero de verdad: asi lo lleva la sincro
        // (la misma que en Windows).
        let perfil = c.mensajes.iter().find(|m| m.id == id_de("PERFIL.md")).unwrap();
        assert_eq!(perfil.ruta.as_deref(), Some("pixpin:files/guardados/asistente/memoria/PERFIL.md"));
        let real = pixpin_proyecto::vista::ruta_real(&raiz, &f.id, perfil.ruta.as_deref().unwrap()).unwrap();
        assert_eq!(real, carpeta_en(&carpeta).join("PERFIL.md"));
        assert!(std::fs::read_to_string(&real).unwrap().contains("corto"));
        let tema = c.mensajes.iter().find(|m| m.id == id_de("temas/obra.md")).unwrap();
        assert!(pixpin_sincro::disco::permitida(tema.ruta.as_deref().unwrap().strip_prefix("pixpin:files/").unwrap()));
        // Borrar el tema quita su mensaje (con marca) y el fichero.
        assert!(m.ejecutar(r#"{"command":"delete","path":"/memoria/temas/obra.md"}"#).contains("borrado"));
        let c = Cuaderno::leer_de(&carpeta).unwrap();
        assert!(!c.mensajes.iter().any(|m| m.id == id_de("temas/obra.md")));
        assert!(!carpeta_en(&carpeta).join("temas").exists());
        // Y lo que va al sistema lo lleva.
        assert!(bloque(Some(&m)).contains("espanol, corto"));
        assert_eq!(bloque(None), "");
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
