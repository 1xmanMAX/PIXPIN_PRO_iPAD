//! Pasar algo de un aparato a otro UNA vez, por la misma wifi.
//!
//! No es sincronizar: no hace falta grupo, no queda nada vinculado y al
//! terminar el codigo deja de existir. Es el puerto de `sincro/Envio.kt` de
//! PixPin Android, y por eso los rotulos viajan como alli (`t`, `nombre`,
//! `bytes`...): son el contrato con el movil, no un nombre nuestro.
//!
//! **Los dos sentidos.** En el de siempre, quien envia espera y ensena su
//! codigo. En el otro espera **quien recibe**: ensena su codigo, el que
//! envia lo escanea (o lo pega) y llama. Es el mismo protocolo con los
//! papeles cambiados, y lo unico que cambia es quien empieza el canal: **lo
//! empieza el que llama** (`inicia`).
//!
//! **Mismo cable byte a byte.** El movil escribe su JSON con
//! `encodeDefaults = false`: lo que vale lo de por defecto no viaja. Aqui se
//! hace igual (`skip_serializing_if`), en el mismo orden de campos, para que
//! un tramo de este lado sea indistinguible de uno del movil.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::canal::{Canal, ErrorCanal, TOPE_DE_TRAMO, Tipo};

/// Cuantas cifras tiene el codigo de un envio.
pub const CIFRAS: usize = 6;
/// Intentos con un codigo equivocado antes de dar el codigo por quemado
/// (`Envio.INTENTOS`): seis cifras son un millon de codigos, y sin tope se
/// podrian ir probando todos mientras la puerta esta abierta.
pub const INTENTOS: u32 = 5;
/// El tipo mDNS que anuncia quien ENVIA y espera (`Envio.TIPO`).
pub const SERVICIO: &str = crate::SERVICIO_ENVIO;
/// El tipo mDNS que anuncia quien ESPERA para recibir (`Envio.TIPO_RECIBIR`).
pub const SERVICIO_RECIBIR: &str = crate::SERVICIO_RECIBIR;

/// `tipo` de un archivo cualquiera.
pub const ARCHIVO: &str = "archivo";
/// `tipo` de un proyecto entero (un `.pixpin`).
pub const PROYECTO: &str = "proyecto";
/// `tipo` de un lienzo suelto de un proyecto.
pub const LIENZO: &str = "lienzo";

/// La clave del canal de un envio. Lleva `envio-` delante, como en Android.
pub fn clave(codigo: &str) -> [u8; 32] {
    crate::codigo::clave_cruda(&format!("envio-{codigo}"))
}

/// Lo que se anuncia en la red (el par `g`) para que quien tiene el codigo
/// encuentre a quien espera sin que el codigo se publique.
pub fn etiqueta(codigo: &str) -> String {
    crate::codigo::etiqueta(&clave(codigo))
}

/// Solo cifras, y como mucho las que caben.
pub fn limpiar(tecleado: &str) -> String {
    tecleado
        .chars()
        .filter(char::is_ascii_digit)
        .take(CIFRAS)
        .collect()
}

pub fn valido(codigo: &str) -> bool {
    codigo.len() == CIFRAS && codigo.chars().all(|c| c.is_ascii_digit())
}

/// `482 913`: en dos grupos de tres, que se dicta y se lee sin perderse.
pub fn legible(codigo: &str) -> String {
    if codigo.len() == CIFRAS {
        format!("{} {}", &codigo[..3], &codigo[3..])
    } else {
        codigo.to_string()
    }
}

/// Lo que lleva el QR de quien ENVIA (`Envio.textoDelQr`): el codigo y donde
/// esta, para conectar sin buscar.
pub fn texto_del_qr(codigo: &str, host: &str, puerto: u16) -> String {
    format!("pixpin-envio:1:{codigo}:{host}:{puerto}")
}

/// Lo que lleva el QR de quien ESPERA para recibir
/// (`Envio.textoDelQrDeRecepcion`).
pub fn texto_del_qr_de_recepcion(codigo: &str, host: &str, puerto: u16) -> String {
    format!("pixpin-recibe:1:{codigo}:{host}:{puerto}")
}

/// Lo que se saca de un QR de envio o de recepcion (`Envio.DelQr`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelQr {
    pub codigo: String,
    /// Donde esta el otro; `None` si el QR no lo dice y hay que buscarlo.
    pub host: Option<String>,
    /// Cero si no lo dice.
    pub puerto: u16,
    /// El QR es de quien espera para RECIBIR: a ese se le manda, no se le
    /// pide.
    pub espera_recibir: bool,
}

/// `Envio.leerQr`, regla por regla: cinco trozos separados por `:`, uno de
/// los dos prefijos y un codigo valido. El host vacio es «no lo se» y un
/// puerto ilegible vale cero, como alli.
pub fn leer_qr(texto: &str) -> Option<DelQr> {
    let p: Vec<&str> = texto.trim().split(':').collect();
    if p.len() < 5 {
        return None;
    }
    let espera_recibir = p[0] == "pixpin-recibe";
    if p[0] != "pixpin-envio" && !espera_recibir {
        return None;
    }
    let codigo = p[2];
    if !valido(codigo) {
        return None;
    }
    Some(DelQr {
        codigo: codigo.to_string(),
        host: (!p[3].trim().is_empty()).then(|| p[3].to_string()),
        puerto: p[4].parse().unwrap_or(0),
        espera_recibir,
    })
}

/// Lo que se escribe o se pega en el ordenador, que no tiene camara: el
/// texto entero de un QR, o solo las seis cifras.
///
/// Si lleva `:` y no es un QR, no se rescatan cifras de dentro: de
/// `pixpin-recibe:1:482913:192.168.1.4:47474` mal copiado saldria un
/// `148291` que parece un codigo y no lo es.
pub fn leer_tecleado(texto: &str) -> Option<DelQr> {
    if let Some(q) = leer_qr(texto) {
        return Some(q);
    }
    if texto.contains(':') {
        return None;
    }
    let codigo = limpiar(texto);
    valido(&codigo).then_some(DelQr {
        codigo,
        host: None,
        puerto: 0,
        espera_recibir: false,
    })
}

fn es_cero(v: &i64) -> bool {
    *v == 0
}

fn es_falso(v: &bool) -> bool {
    !*v
}

/// Una cosa que se manda. Los nombres de los campos son los del movil.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Elemento {
    /// `archivo`, `proyecto` (un `.pixpin`) o `lienzo`.
    pub tipo: String,
    pub nombre: String,
    pub bytes: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    /// Su sena entre envios: la misma cosa mandada otra vez lleva la misma.
    #[serde(default)]
    pub identidad: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proyecto: Option<String>,
    #[serde(
        rename = "proyectoNombre",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub proyecto_nombre: Option<String>,
    #[serde(default, skip_serializing_if = "es_cero")]
    pub creado: i64,
    /// Los tres codigos (ver `pixpin_proyecto::codigos`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(
        rename = "codigoDeChat",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub codigo_de_chat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aparato: Option<String>,
}

/// Lo que ofrece quien envia.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Oferta {
    #[serde(default)]
    pub de: String,
    #[serde(rename = "deId", default)]
    pub de_id: String,
    #[serde(default)]
    pub elementos: Vec<Elemento>,
    #[serde(rename = "deCodigo", default, skip_serializing_if = "String::is_empty")]
    pub de_codigo: String,
    /// Quien envia no aprobo a este aparato: lo dice en vez de cortar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rechazado: Option<String>,
}

/// Los avisos cortos del dialogo: `hola`, `acepto`, `no`, `archivo`, `listo`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Aviso {
    pub t: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nombre: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "es_cero")]
    pub bytes: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// La cola que va detras de los trozos de un archivo: su resumen.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cola {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resumen: Option<String>,
    /// El que manda dice que ese archivo no vale: cambio mientras lo leia.
    #[serde(default, skip_serializing_if = "es_falso")]
    pub saltado: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorEnvio {
    #[error(transparent)]
    Canal(#[from] ErrorCanal),
    #[error("no se entendio lo que mando el otro aparato: {0}")]
    Formato(#[from] serde_json::Error),
    #[error("se esperaba {que} y llego otra cosa")]
    FueraDeOrden { que: &'static str },
    #[error("el otro aparato dijo: {0}")]
    Dijo(String),
    #[error("«{0}» llego distinto de como salio")]
    Corrupto(String),
    #[error("no se pudo escribir lo que llego: {0}")]
    Escritura(#[from] std::io::Error),
    #[error("«{0}» cambió mientras se mandaba. Vuelve a intentarlo.")]
    CambioAlMandar(String),
}

impl ErrorEnvio {
    /// El otro deriva otra clave: tecleo otro codigo. Es lo que se cuenta
    /// para quemar el codigo tras `INTENTOS`, y no un fallo de red.
    pub fn es_codigo_distinto(&self) -> bool {
        matches!(self, ErrorEnvio::Canal(ErrorCanal::CodigoDistinto))
    }
}

/// Un aviso, y si trae `error`, el error: es `Envio.leer` del movil.
fn leer_aviso<F: Read + Write>(c: &mut Canal<F>, que: &'static str) -> Result<Aviso, ErrorEnvio> {
    let datos = leer_json(c, que)?;
    let a: Aviso = serde_json::from_slice(&datos)?;
    if let Some(e) = a.error {
        return Err(ErrorEnvio::Dijo(e));
    }
    Ok(a)
}

fn a_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Como termino un envio para quien manda (`Envio.Final`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Final {
    Enviado,
    /// El otro aparato dijo que no.
    Rechazado,
    /// Este aparato no aprobo al otro: ni se le ofrecio.
    SinAprobar,
}

/// Quien manda, tal como se presenta en la oferta.
#[derive(Debug, Clone, Default)]
pub struct Remitente {
    pub nombre: String,
    pub id: String,
    /// El codigo corto del aparato (`K7Q2`).
    pub codigo: String,
}

/// **Quien envia** (`Envio.Emisor`). Atiende una conexion: lee el saludo,
/// dice que manda y, si le aceptan, lo manda.
pub struct Emisor<'a> {
    yo: &'a Remitente,
    cosas: &'a [(Elemento, PathBuf)],
}

impl<'a> Emisor<'a> {
    pub fn nuevo(yo: &'a Remitente, cosas: &'a [(Elemento, PathBuf)]) -> Emisor<'a> {
        Emisor { yo, cosas }
    }

    /// `al_conocer` recibe el nombre de quien se conecto y **decide si se le
    /// manda**: puede quedarse esperando a que el usuario apruebe. Si dice
    /// que no, se le contesta con una oferta `rechazado`, que asi sabe que
    /// no le aprobaron y no cree que se cayo la red.
    ///
    /// `inicia` es cierto cuando es ESTE aparato el que llama, porque quien
    /// recibe estaba esperando.
    pub fn atender<F: Read + Write>(
        &self,
        flujo: F,
        codigo: &str,
        nonce: [u8; crate::canal::NONCE],
        inicia: bool,
        mut al_conocer: impl FnMut(&str) -> bool,
        mut avance: impl FnMut(u64, u64),
    ) -> Result<Final, ErrorEnvio> {
        let mut canal = Canal::saludar(flujo, &clave(codigo), nonce, inicia)?;
        let hola = leer_aviso(&mut canal, "el saludo")?;
        if hola.t != "hola" {
            return Err(ErrorEnvio::FueraDeOrden { que: "el saludo" });
        }
        if !al_conocer(&hola.nombre) {
            let no = Oferta {
                de: self.yo.nombre.clone(),
                de_id: self.yo.id.clone(),
                elementos: Vec::new(),
                de_codigo: self.yo.codigo.clone(),
                rechazado: Some(format!(
                    "{} no aprobó el envío a este aparato.",
                    self.yo.nombre
                )),
            };
            canal.mandar(Tipo::Json, &serde_json::to_vec(&no)?)?;
            canal.vaciar()?;
            return Ok(Final::SinAprobar);
        }
        let oferta = Oferta {
            de: self.yo.nombre.clone(),
            de_id: self.yo.id.clone(),
            elementos: self.cosas.iter().map(|(e, _)| e.clone()).collect(),
            de_codigo: self.yo.codigo.clone(),
            rechazado: None,
        };
        canal.mandar(Tipo::Json, &serde_json::to_vec(&oferta)?)?;
        if leer_aviso(&mut canal, "la respuesta")?.t != "acepto" {
            canal.vaciar()?;
            return Ok(Final::Rechazado);
        }
        // El total sale del disco y no de la oferta: es lo que de verdad va
        // a salir por el cable, y asi la barra llega justo al final.
        let total: u64 = self
            .cosas
            .iter()
            .map(|(_, r)| std::fs::metadata(r).map(|m| m.len()).unwrap_or(0))
            .sum();
        let mut hechos = 0u64;
        for (e, ruta) in self.cosas {
            mandar_archivo(&mut canal, &e.nombre, ruta, |n| {
                hechos += n;
                avance(hechos, total);
            })?;
        }
        if leer_aviso(&mut canal, "la confirmacion")?.t != "listo" {
            return Err(ErrorEnvio::FueraDeOrden {
                que: "la confirmacion",
            });
        }
        Ok(Final::Enviado)
    }
}

/// Un archivo tal cual, a trozos y con su cola (`Protocolo.salidaDeArchivo`).
///
/// Si el archivo cambia mientras se lee —un programa terminando de
/// guardarlo—, lo que salio puede ser medio viejo y medio nuevo: la cola lo
/// dice (`saltado`) para que el otro no lo escriba, y aqui se corta el envio.
/// Si se acorta a medias se rellena con ceros hasta lo prometido, porque el
/// otro cuenta bytes y sin eso se quedaria esperando.
fn mandar_archivo<F: Read + Write>(
    canal: &mut Canal<F>,
    nombre: &str,
    ruta: &Path,
    mut avance: impl FnMut(u64),
) -> Result<(), ErrorEnvio> {
    let antes = std::fs::metadata(ruta)?;
    let largo = antes.len();
    mandar_aviso(
        canal,
        &Aviso {
            t: "archivo".into(),
            nombre: nombre.to_string(),
            bytes: largo as i64,
            ..Default::default()
        },
    )?;
    let mut fichero = std::fs::File::open(ruta)?;
    let mut md = Sha256::new();
    let mut buf = vec![0u8; TOPE_DE_TRAMO];
    let mut quedan = largo;
    let mut corto = false;
    while quedan > 0 {
        let quiero = (buf.len() as u64).min(quedan) as usize;
        let mut n = 0;
        while n < quiero {
            match fichero.read(&mut buf[n..quiero])? {
                0 => break,
                r => n += r,
            }
        }
        if n < quiero {
            corto = true;
            buf[n..quiero].fill(0);
        }
        md.update(&buf[..quiero]);
        canal.mandar(Tipo::Trozo, &buf[..quiero])?;
        quedan -= quiero as u64;
        avance(quiero as u64);
    }
    let despues = std::fs::metadata(ruta)?;
    let cambio =
        corto || despues.len() != largo || despues.modified().ok() != antes.modified().ok();
    let cola = Cola {
        resumen: Some(a_hex(&md.finalize())),
        saltado: cambio,
    };
    canal.mandar(Tipo::Json, &serde_json::to_vec(&cola)?)?;
    if cambio {
        return Err(ErrorEnvio::CambioAlMandar(nombre.to_string()));
    }
    Ok(())
}

fn mandar_aviso<F: std::io::Read + std::io::Write>(
    c: &mut Canal<F>,
    a: &Aviso,
) -> Result<(), ErrorEnvio> {
    c.mandar(Tipo::Json, &serde_json::to_vec(a)?)?;
    Ok(())
}

fn leer_json<F: std::io::Read + std::io::Write>(
    c: &mut Canal<F>,
    que: &'static str,
) -> Result<Vec<u8>, ErrorEnvio> {
    let (tipo, datos) = c.recibir()?;
    if tipo != Tipo::Json {
        return Err(ErrorEnvio::FueraDeOrden { que });
    }
    Ok(datos)
}

/// El lado que RECIBE, que aqui es el ordenador.
pub struct Receptor<F: std::io::Read + std::io::Write> {
    canal: Canal<F>,
    pub oferta: Oferta,
}

impl<F: std::io::Read + std::io::Write> Receptor<F> {
    /// Se presenta y trae la oferta. `inicia` dice si es ESTE aparato el que
    /// llama: falso cuando el ordenador esperaba y el movil llamo.
    pub fn conectar(
        flujo: F,
        codigo: &str,
        nombre: &str,
        id: &str,
        nonce: [u8; crate::canal::NONCE],
        inicia: bool,
    ) -> Result<Self, ErrorEnvio> {
        let mut canal = Canal::saludar(flujo, &clave(codigo), nonce, inicia)?;
        mandar_aviso(
            &mut canal,
            &Aviso {
                t: "hola".into(),
                nombre: nombre.to_string(),
                id: id.to_string(),
                ..Default::default()
            },
        )?;
        let datos = leer_json(&mut canal, "la oferta")?;
        let oferta: Oferta = serde_json::from_slice(&datos)?;
        if let Some(motivo) = oferta.rechazado.clone() {
            return Err(ErrorEnvio::Dijo(motivo));
        }
        Ok(Receptor { canal, oferta })
    }

    /// Dice que no. Lo que falle aqui da igual: ya no se recibe nada.
    pub fn rechazar(&mut self) {
        let _ = mandar_aviso(
            &mut self.canal,
            &Aviso {
                t: "no".into(),
                ..Default::default()
            },
        );
        let _ = self.canal.vaciar();
    }

    /// Acepta y escribe cada cosa en `carpeta`. Devuelve donde quedo cada una.
    ///
    /// `avance` recibe los bytes que van llegando y el total, para poder
    /// ensenarlo mientras dura.
    pub fn aceptar(
        &mut self,
        carpeta: &std::path::Path,
        mut avance: impl FnMut(u64, u64),
    ) -> Result<Vec<(Elemento, std::path::PathBuf)>, ErrorEnvio> {
        mandar_aviso(
            &mut self.canal,
            &Aviso {
                t: "acepto".into(),
                ..Default::default()
            },
        )?;
        std::fs::create_dir_all(carpeta)?;
        let total: u64 = self
            .oferta
            .elementos
            .iter()
            .map(|e| e.bytes.max(0) as u64)
            .sum();
        let mut hechos = 0u64;
        let mut salida = Vec::new();
        for e in self.oferta.elementos.clone() {
            let datos = leer_json(&mut self.canal, "un archivo")?;
            let cabecera: Aviso = serde_json::from_slice(&datos)?;
            if let Some(error) = cabecera.error {
                return Err(ErrorEnvio::Dijo(error));
            }
            if cabecera.t != "archivo" {
                return Err(ErrorEnvio::FueraDeOrden { que: "un archivo" });
            }
            // **Mientras llega se llama `<nombre>.parte`** y solo al final se
            // le pone su nombre. Asi un corte —la wifi, el movil que se
            // apaga, PixPin que se cierra— no deja un fichero a medias con
            // cara de entero: lo que se quede con `.parte` detras se ve que
            // no vale, y nada que se llame como lo que se pidio esta roto.
            let destino = ruta_libre(carpeta, &nombre_sano(&e.nombre));
            let parte = crate::disco::con_sufijo(&destino, ".parte");
            let mut fichero = std::fs::File::create(&parte)?;
            let bien = self.recibir_trozos(cabecera.bytes.max(0) as u64, &mut fichero, |n| {
                hechos += n;
                avance(hechos, total);
            });
            // El fichero se cierra pase lo que pase: en Windows no se puede
            // borrar ni renombrar lo que sigue abierto.
            drop(fichero);
            let bien = match bien {
                Ok(b) => b,
                Err(x) => {
                    let _ = std::fs::remove_file(&parte);
                    return Err(x);
                }
            };
            if !bien {
                // Lo que llego a medias no se queda: mejor nada que un
                // fichero roto con un nombre que promete estar entero.
                let _ = std::fs::remove_file(&parte);
                return Err(ErrorEnvio::Corrupto(e.nombre.clone()));
            }
            crate::disco::reemplazar(&parte, &destino)?;
            salida.push((e, destino));
        }
        mandar_aviso(
            &mut self.canal,
            &Aviso {
                t: "listo".into(),
                ..Default::default()
            },
        )?;
        self.canal.vaciar()?;
        Ok(salida)
    }

    /// Los trozos de un archivo y su cola. `false` si el que manda dice que
    /// no vale, o si lo que llego no es lo que salio.
    fn recibir_trozos(
        &mut self,
        largo: u64,
        salida: &mut impl std::io::Write,
        mut avance: impl FnMut(u64),
    ) -> Result<bool, ErrorEnvio> {
        let mut md = Sha256::new();
        let mut quedan = largo;
        while quedan > 0 {
            let (tipo, datos) = self.canal.recibir()?;
            if tipo != Tipo::Trozo {
                return Err(ErrorEnvio::FueraDeOrden { que: "un trozo" });
            }
            salida.write_all(&datos)?;
            md.update(&datos);
            quedan = quedan.saturating_sub(datos.len() as u64);
            avance(datos.len() as u64);
        }
        let datos = leer_json(&mut self.canal, "la cola del archivo")?;
        let cola: Cola = serde_json::from_slice(&datos)?;
        if cola.saltado {
            return Ok(false);
        }
        let resumen: String = md.finalize().iter().map(|b| format!("{b:02x}")).collect();
        match cola.resumen {
            Some(suyo) if suyo != resumen => Ok(false),
            _ => Ok(true),
        }
    }
}

/// Un nombre que Windows admita: sin los signos prohibidos y sin pasarse de
/// largo. El mismo criterio que el movil, para que el fichero se llame igual
/// en los dos aparatos.
pub fn nombre_sano(nombre: &str) -> String {
    let limpio: String = nombre
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .take(120)
        .collect();
    let limpio = limpio.trim().to_string();
    if limpio.is_empty() {
        "archivo".to_string()
    } else {
        limpio
    }
}

/// Las extensiones que se reconocen (`EXTENSIONES_CONOCIDAS` de
/// `guardados/Mensajes.kt`). `bin` NO esta: es «no se que es».
const EXTENSIONES_CONOCIDAS: [&str; 46] = [
    "pdf", "jpg", "jpeg", "png", "webp", "gif", "heic", "bmp", "svg", "m4a", "mp3", "ogg", "oga",
    "opus", "wav", "flac", "aac", "amr", "3gp", "mp4", "mkv", "mov", "webm", "txt", "md", "csv",
    "json", "xml", "html", "htm", "zip", "doc", "docx", "xls", "xlsx", "xlsm", "tsv", "ppt",
    "pptx", "odt", "ods", "dxf", "dwg", "pixpin", "excalidraw", "epub",
];

/// Si `nombre` acaba en una extension de las que se reconocen
/// (`tieneExtensionConocida`).
pub fn tiene_extension_conocida(nombre: &str) -> bool {
    nombre.rsplit_once('.').is_some_and(|(_, e)| {
        EXTENSIONES_CONOCIDAS
            .iter()
            .any(|x| x.eq_ignore_ascii_case(e))
    })
}

/// **La extension por lo que hay dentro** (`extensionPorContenido`, Android
/// v0.108.0): los primeros bytes de un PDF, PNG, JPEG, GIF o WebP no
/// enganan. `None` si no es ninguno.
pub fn extension_por_contenido(cabeza: &[u8]) -> Option<&'static str> {
    let empieza = |b: &[u8]| cabeza.starts_with(b);
    if empieza(b"%PDF") {
        Some("pdf")
    } else if empieza(&[0x89, 0x50, 0x4E, 0x47]) {
        Some("png")
    } else if empieza(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if empieza(b"GIF8") {
        Some("gif")
    } else if cabeza.len() >= 12 && empieza(b"RIFF") && &cabeza[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// **El nombre que le toca a un adjunto por lo que lleva dentro.** Hasta la
/// v0.108.0 el movil pegaba `bin` a lo que llegaba como
/// `application/octet-stream` (muchas apps comparten asi un PDF) y el PC no
/// sabia abrir `informe.bin`; desde entonces lo manda sin extension. Con una
/// extension que se reconoce, el nombre se queda; con `.bin` o sin ninguna,
/// se mira la cabeza: si es un PDF o una imagen, `informe.bin` pasa a
/// `informe.pdf` (y `informe` a `informe.pdf`). Si no se sabe que es, tal
/// cual.
pub fn nombre_por_contenido(nombre: &str, cabeza: &[u8]) -> String {
    if tiene_extension_conocida(nombre) {
        return nombre.to_string();
    }
    let Some(ext) = extension_por_contenido(cabeza) else {
        return nombre.to_string();
    };
    let base = match nombre.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() && e.eq_ignore_ascii_case("bin") => b,
        _ => nombre,
    };
    format!("{base}.{ext}")
}

/// Los primeros bytes de un fichero, los que mira
/// [`extension_por_contenido`]. Vacio si no se puede leer.
pub fn cabeza_de(ruta: &std::path::Path) -> Vec<u8> {
    use std::io::Read;
    let mut b = [0u8; 16];
    let Ok(mut f) = std::fs::File::open(ruta) else {
        return Vec::new();
    };
    let mut n = 0;
    while n < b.len() {
        match f.read(&mut b[n..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => n += k,
        }
    }
    b[..n].to_vec()
}

/// Un nombre que no pise nada: `plano (2).pdf` si `plano.pdf` ya esta.
pub fn ruta_libre(carpeta: &std::path::Path, nombre: &str) -> std::path::PathBuf {
    let mut destino = carpeta.join(nombre);
    let (base, ext) = match nombre.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() => (b.to_string(), format!(".{e}")),
        _ => (nombre.to_string(), String::new()),
    };
    let mut n = 2;
    while destino.exists() {
        destino = carpeta.join(format!("{base} ({n}){ext}"));
        n += 1;
    }
    destino
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_codigo_se_limpia_y_se_lee_como_en_el_movil() {
        assert_eq!(limpiar("48-29 13x"), "482913");
        assert_eq!(limpiar("4829137"), "482913", "no pasa de seis cifras");
        assert!(valido("482913"));
        assert!(!valido("48291"), "cinco cifras no valen");
        assert!(!valido("48291a"), "ni una letra colada");
        assert_eq!(legible("482913"), "482 913");
    }

    #[test]
    fn el_qr_dice_donde_esperar() {
        assert_eq!(
            texto_del_qr_de_recepcion("482913", "192.168.1.40", 47474),
            "pixpin-recibe:1:482913:192.168.1.40:47474"
        );
        assert_eq!(
            texto_del_qr("482913", "192.168.1.40", 47474),
            "pixpin-envio:1:482913:192.168.1.40:47474"
        );
    }

    #[test]
    fn los_qr_se_leen_como_los_lee_el_movil() {
        assert_eq!(
            leer_qr(" pixpin-envio:1:482913:192.168.1.40:47474 "),
            Some(DelQr {
                codigo: "482913".into(),
                host: Some("192.168.1.40".into()),
                puerto: 47474,
                espera_recibir: false,
            })
        );
        let recibe = leer_qr("pixpin-recibe:1:482913::0").expect("sin host tambien vale");
        assert!(recibe.espera_recibir);
        assert_eq!((recibe.host, recibe.puerto), (None, 0));
        assert_eq!(
            leer_qr("pixpin-envio:1:482913:1.2.3.4:abc").map(|q| q.puerto),
            Some(0),
            "un puerto ilegible vale cero, como `toIntOrNull() ?: 0`"
        );
    }

    #[test]
    fn un_qr_ajeno_corto_o_con_codigo_malo_no_se_lee() {
        assert_eq!(
            leer_qr("pixpin-envio:1:482913:1.2.3.4"),
            None,
            "faltan trozos"
        );
        assert_eq!(leer_qr("otra-cosa:1:482913:1.2.3.4:5"), None);
        assert_eq!(leer_qr("pixpin-envio:1:48291:1.2.3.4:5"), None);
        assert_eq!(leer_qr("pixpin-envio:1:48291a:1.2.3.4:5"), None);
        assert_eq!(leer_qr(""), None);
    }

    #[test]
    fn lo_tecleado_vale_con_seis_cifras_o_con_el_texto_de_un_qr() {
        assert_eq!(
            leer_tecleado("482 913").map(|q| (q.codigo, q.host)),
            Some(("482913".into(), None))
        );
        assert!(
            leer_tecleado("pixpin-recibe:1:482913:192.168.1.4:47474")
                .is_some_and(|q| q.espera_recibir)
        );
        assert_eq!(leer_tecleado("48 29"), None, "cuatro cifras no bastan");
        // Caso negativo que importa: de un QR roto no se rescatan cifras.
        assert_eq!(leer_tecleado("pixpin-recibe:1:482913:192.168.1.4"), None);
    }

    #[test]
    fn lo_que_vale_lo_de_por_defecto_no_viaja_como_en_el_movil() {
        // `encodeDefaults = false`: estos son, letra por letra, los tramos
        // que escribe el movil.
        let acepto = Aviso {
            t: "acepto".into(),
            ..Default::default()
        };
        assert_eq!(serde_json::to_string(&acepto).unwrap(), r#"{"t":"acepto"}"#);
        let archivo = Aviso {
            t: "archivo".into(),
            nombre: "a.txt".into(),
            bytes: 3,
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_string(&archivo).unwrap(),
            r#"{"t":"archivo","nombre":"a.txt","bytes":3}"#
        );
        let cola = Cola {
            resumen: Some("ab".into()),
            saltado: false,
        };
        assert_eq!(serde_json::to_string(&cola).unwrap(), r#"{"resumen":"ab"}"#);
        let oferta = Oferta {
            de: "PC".into(),
            de_id: "x".into(),
            elementos: vec![Elemento {
                tipo: ARCHIVO.into(),
                nombre: "a.txt".into(),
                bytes: 3,
                mime: None,
                identidad: "archivo:x:a.txt".into(),
                proyecto: None,
                proyecto_nombre: None,
                creado: 0,
                uid: None,
                codigo_de_chat: None,
                aparato: None,
            }],
            de_codigo: String::new(),
            rechazado: None,
        };
        assert_eq!(
            serde_json::to_string(&oferta).unwrap(),
            r#"{"de":"PC","deId":"x","elementos":[{"tipo":"archivo","nombre":"a.txt","bytes":3,"identidad":"archivo:x:a.txt"}]}"#
        );
    }

    // ---- Emisor contra Receptor, por un par de sockets de verdad

    fn carpeta_de_prueba(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-envio-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn cosas_de_prueba(d: &Path) -> Vec<(Elemento, PathBuf)> {
        // Uno de mas de un tramo, para que viaje en varios trozos.
        let grande: Vec<u8> = (0..TOPE_DE_TRAMO + 1234).map(|i| (i % 251) as u8).collect();
        let mut v = Vec::new();
        for (nombre, datos) in [("plano.pdf", grande), ("nota.txt", b"hola".to_vec())] {
            let ruta = d.join(nombre);
            std::fs::write(&ruta, &datos).unwrap();
            v.push((
                Elemento {
                    tipo: ARCHIVO.into(),
                    nombre: nombre.into(),
                    bytes: datos.len() as i64,
                    mime: None,
                    identidad: format!("archivo:pc:{nombre}"),
                    proyecto: None,
                    proyecto_nombre: None,
                    creado: 0,
                    uid: None,
                    codigo_de_chat: None,
                    aparato: None,
                },
                ruta,
            ));
        }
        v
    }

    fn yo() -> Remitente {
        Remitente {
            nombre: "Portatil".into(),
            id: "pc".into(),
            codigo: "K7Q2".into(),
        }
    }

    /// Quien espera en un hilo y quien llama en este; devuelve lo de los dos.
    fn enfrentar<A: Send + 'static, B>(
        espera: impl FnOnce(std::net::TcpStream) -> A + Send + 'static,
        llama: impl FnOnce(std::net::TcpStream) -> B,
    ) -> (A, B) {
        let escucha = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let puerto = escucha.local_addr().unwrap().port();
        let hilo = std::thread::spawn(move || espera(escucha.accept().unwrap().0));
        let b = llama(std::net::TcpStream::connect(("127.0.0.1", puerto)).unwrap());
        (hilo.join().expect("el hilo que espera no revienta"), b)
    }

    #[test]
    fn lo_que_manda_el_emisor_llega_entero_al_receptor() {
        let origen = carpeta_de_prueba("sale");
        let destino = carpeta_de_prueba("llega");
        let cosas = cosas_de_prueba(&origen);
        let para_hilo = cosas.clone();
        let (final_, (conocido, llegados)) = enfrentar(
            move |s| {
                let yo = yo();
                let mut quien = String::new();
                let f = Emisor::nuevo(&yo, &para_hilo)
                    .atender(
                        s,
                        "482913",
                        [1; 32],
                        false,
                        |n| {
                            quien = n.to_string();
                            true
                        },
                        |_, _| {},
                    )
                    .unwrap();
                (f, quien)
            },
            |s| {
                let mut r = Receptor::conectar(s, "482913", "Movil", "m1", [2; 32], true).unwrap();
                assert_eq!(r.oferta.de, "Portatil");
                assert_eq!(r.oferta.de_codigo, "K7Q2");
                assert_eq!(r.oferta.elementos.len(), 2);
                let mut ultimo = (0, 0);
                let llegados = r.aceptar(&destino, |h, t| ultimo = (h, t)).unwrap();
                assert_eq!(ultimo.0, ultimo.1, "la barra acaba llena");
                ("Movil", llegados)
            },
        );
        assert_eq!(final_.0, Final::Enviado);
        assert_eq!(final_.1, conocido);
        for ((_, sale), (_, llega)) in cosas.iter().zip(&llegados) {
            assert_eq!(std::fs::read(sale).unwrap(), std::fs::read(llega).unwrap());
        }
    }

    #[test]
    fn con_los_papeles_cambiados_llama_quien_envia_y_tambien_llega() {
        // Quien recibe espera (su QR `pixpin-recibe`) y el emisor llama:
        // el canal lo empieza el emisor.
        let origen = carpeta_de_prueba("sale-2");
        let destino = carpeta_de_prueba("llega-2");
        let cosas = cosas_de_prueba(&origen);
        let (llegados, final_) = enfrentar(
            move |s| {
                let mut r =
                    Receptor::conectar(s, "111222", "Portatil", "pc", [3; 32], false).unwrap();
                r.aceptar(&destino, |_, _| {}).unwrap().len()
            },
            |s| {
                let yo = yo();
                Emisor::nuevo(&yo, &cosas)
                    .atender(s, "111222", [4; 32], true, |_| true, |_, _| {})
                    .unwrap()
            },
        );
        assert_eq!((llegados, final_), (2, Final::Enviado));
    }

    #[test]
    fn si_quien_recibe_dice_que_no_no_se_manda_nada() {
        let origen = carpeta_de_prueba("no");
        let cosas = cosas_de_prueba(&origen);
        let (final_, ()) = enfrentar(
            move |s| {
                let yo = yo();
                Emisor::nuevo(&yo, &cosas)
                    .atender(s, "482913", [1; 32], false, |_| true, |_, _| {})
                    .unwrap()
            },
            |s| {
                let mut r = Receptor::conectar(s, "482913", "Movil", "m1", [2; 32], true).unwrap();
                r.rechazar();
            },
        );
        assert_eq!(final_, Final::Rechazado);
    }

    #[test]
    fn a_quien_no_se_aprueba_se_le_dice_en_vez_de_cortarle() {
        let origen = carpeta_de_prueba("sin-aprobar");
        let cosas = cosas_de_prueba(&origen);
        let (final_, error) = enfrentar(
            move |s| {
                let yo = yo();
                Emisor::nuevo(&yo, &cosas)
                    .atender(s, "482913", [1; 32], false, |_| false, |_, _| {})
                    .unwrap()
            },
            |s| {
                Receptor::conectar(s, "482913", "Movil", "m1", [2; 32], true)
                    .err()
                    .map(|e| e.to_string())
            },
        );
        assert_eq!(final_, Final::SinAprobar);
        assert!(
            error.is_some_and(|e| e.contains("Portatil no aprobó")),
            "el receptor sabe que no le aprobaron"
        );
    }

    #[test]
    fn con_otro_codigo_el_emisor_lo_cuenta_como_codigo_equivocado() {
        let origen = carpeta_de_prueba("malo");
        let cosas = cosas_de_prueba(&origen);
        let (error, _) = enfrentar(
            move |s| {
                let yo = yo();
                Emisor::nuevo(&yo, &cosas)
                    .atender(s, "482913", [1; 32], false, |_| true, |_, _| {})
                    .unwrap_err()
            },
            |s| Receptor::conectar(s, "000000", "Movil", "m1", [2; 32], true).is_err(),
        );
        assert!(error.es_codigo_distinto(), "{error}");
    }

    #[test]
    fn un_envio_cortado_a_medias_no_deja_un_fichero_con_cara_de_entero() {
        // Lo que llega se escribe como `<nombre>.parte` y solo al final se
        // le pone su nombre. Aqui quien envia corta el socket a mitad del
        // primer archivo: al otro lado no puede quedar ningun `plano.pdf`
        // —que pareceria bueno y se abriria roto— y el `.parte` se limpia.
        let destino = carpeta_de_prueba("corte-llega");
        let ((), error) = enfrentar(
            move |s| {
                // Un emisor de mentira que promete un archivo largo, manda un
                // trozo y suelta el socket: es lo que hace una wifi que se cae.
                let mut canal = Canal::saludar(s, &clave("482913"), [1; 32], false).unwrap();
                leer_aviso(&mut canal, "el saludo").unwrap();
                let oferta = Oferta {
                    de: "Portatil".into(),
                    de_id: "pc".into(),
                    elementos: vec![Elemento {
                        tipo: ARCHIVO.into(),
                        nombre: "plano.pdf".into(),
                        bytes: 4096,
                        mime: None,
                        identidad: "archivo:pc:plano.pdf".into(),
                        proyecto: None,
                        proyecto_nombre: None,
                        creado: 0,
                        uid: None,
                        codigo_de_chat: None,
                        aparato: None,
                    }],
                    de_codigo: "K7Q2".into(),
                    rechazado: None,
                };
                canal
                    .mandar(Tipo::Json, &serde_json::to_vec(&oferta).unwrap())
                    .unwrap();
                leer_aviso(&mut canal, "la respuesta").unwrap();
                mandar_aviso(
                    &mut canal,
                    &Aviso {
                        t: "archivo".into(),
                        nombre: "plano.pdf".into(),
                        bytes: 4096,
                        ..Default::default()
                    },
                )
                .unwrap();
                canal.mandar(Tipo::Trozo, &[7u8; 100]).unwrap();
                // Y aqui se corta: el socket se suelta a medio archivo.
                drop(canal);
            },
            |s| {
                let mut r = Receptor::conectar(s, "482913", "Movil", "m1", [2; 32], true).unwrap();
                r.aceptar(&destino, |_, _| {}).err().map(|e| e.to_string())
            },
        );
        assert!(error.is_some(), "el corte se cuenta como fallo");
        assert!(
            !destino.join("plano.pdf").exists(),
            "nada con el nombre definitivo: estaria a medias y se abriria roto"
        );
    }

    #[test]
    fn lo_que_llega_entero_no_deja_ningun_resto_a_medias() {
        let origen = carpeta_de_prueba("sin-restos-sale");
        let destino = carpeta_de_prueba("sin-restos-llega");
        let cosas = cosas_de_prueba(&origen);
        let (_, llegados) = enfrentar(
            move |s| {
                let yo = yo();
                Emisor::nuevo(&yo, &cosas)
                    .atender(s, "482913", [1; 32], false, |_| true, |_, _| {})
                    .unwrap()
            },
            |s| {
                let mut r = Receptor::conectar(s, "482913", "Movil", "m1", [2; 32], true).unwrap();
                r.aceptar(&destino, |_, _| {}).unwrap().len()
            },
        );
        assert_eq!(llegados, 2);
        let restos: Vec<String> = std::fs::read_dir(&destino)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".parte"))
            .collect();
        assert!(restos.is_empty(), "quedaron a medias: {restos:?}");
    }

    #[test]
    fn la_clave_del_envio_lleva_su_prefijo() {
        // Android: `Grupo.clave("envio-$codigo")`. Sin el prefijo, un codigo
        // de envio abriria un grupo, que es otra cosa y con otra vida.
        assert_eq!(clave("482913"), crate::codigo::clave_cruda("envio-482913"));
        assert_ne!(clave("482913"), crate::codigo::clave_cruda("482913"));
    }

    #[test]
    fn un_nombre_del_movil_no_escribe_fuera_de_su_carpeta() {
        // Caso negativo que importa: un nombre con rutas dentro no puede
        // sacar el fichero de la carpeta elegida.
        assert_eq!(
            nombre_sano("../../windows/system32/a.dll"),
            ".._.._windows_system32_a.dll"
        );
        assert_eq!(nombre_sano("  "), "archivo");
        assert_eq!(nombre_sano("plano:1.pdf"), "plano_1.pdf");
    }

    #[test]
    fn dos_ficheros_con_el_mismo_nombre_no_se_pisan() {
        let d = std::env::temp_dir().join(format!("pixpin-envio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("plano.pdf"), b"x").unwrap();
        assert_eq!(ruta_libre(&d, "plano.pdf"), d.join("plano (2).pdf"));
        assert_eq!(ruta_libre(&d, "otro.pdf"), d.join("otro.pdf"));
    }

    #[test]
    fn un_bin_que_es_un_pdf_o_una_imagen_se_llama_por_lo_que_es() {
        assert_eq!(extension_por_contenido(b"%PDF-1.7\n"), Some("pdf"));
        assert_eq!(extension_por_contenido(&[0x89, b'P', b'N', b'G', 13, 10]), Some("png"));
        assert_eq!(extension_por_contenido(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(extension_por_contenido(b"GIF89a"), Some("gif"));
        assert_eq!(extension_por_contenido(b"RIFF\x10\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(nombre_por_contenido("informe.bin", b"%PDF-1.4"), "informe.pdf");
        assert_eq!(nombre_por_contenido("informe.BIN", b"%PDF-1.4"), "informe.pdf");
        assert_eq!(nombre_por_contenido("Plano de la nave", b"%PDF-1.4"), "Plano de la nave.pdf");
        assert_eq!(nombre_por_contenido("Plano v1.2", &[0xFF, 0xD8, 0xFF]), "Plano v1.2.jpg");
    }

    #[test]
    fn caso_negativo_lo_que_ya_tiene_extension_o_no_se_reconoce_no_se_toca() {
        // Una extension conocida manda, aunque dentro haya otra cosa.
        assert_eq!(nombre_por_contenido("foto.png", b"%PDF-1.4"), "foto.png");
        // Un RIFF que no es WebP (un WAV) no es una imagen.
        assert_eq!(extension_por_contenido(b"RIFF\x10\0\0\0WAVEfmt "), None);
        // Corto o desconocido: tal cual, con su `.bin` si lo traia.
        assert_eq!(extension_por_contenido(b"%PD"), None);
        assert_eq!(nombre_por_contenido("datos.bin", b"\0\0\0\0"), "datos.bin");
        assert_eq!(nombre_por_contenido("datos", b""), "datos");
        // `.bin` solo no es un nombre con extension que quitar.
        assert_eq!(nombre_por_contenido(".bin", b"%PDF"), ".bin.pdf");
    }

    #[test]
    fn la_cabeza_de_un_fichero_son_sus_primeros_bytes() {
        let d = std::env::temp_dir().join(format!("pixpin-cabeza-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("x.bin");
        std::fs::write(&f, b"%PDF-1.4 y mucho mas que dieciseis bytes").unwrap();
        assert_eq!(cabeza_de(&f), b"%PDF-1.4 y mucho".to_vec());
        assert!(cabeza_de(&d.join("no-esta")).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
