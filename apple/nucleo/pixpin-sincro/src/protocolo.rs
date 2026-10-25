//! La conversacion entre dos aparatos: quien responde y quien dirige.
//!
//! Puerto de `sincro/Protocolo.kt` (v0.51.0, protocolo 4). Uno **dirige** —el
//! que pulso «sincronizar», con su [Sesion]— y el otro **responde** —con su
//! [Respondedor]—. Cada chat va en dos pasos: los mensajes y el proyecto, y
//! despues los archivos; lo cambiado en un solo lado pasa y lo cambiado en
//! los dos se funde. Al acabar, los dos guardan lo acordado ([Base]).
//!
//! El orden de las peticiones, lo que lleva cada una y como van los trozos
//! de un archivo son los del movil byte a byte: si algo de aqui parece
//! mejorable, hay que cambiarlo tambien alli.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::base::{Base, Mapa};
use crate::canal::{Canal, ErrorCanal, MAGIA_DE_INVITACION, TOPE_DE_TRAMO, Tipo};
use crate::compartidos::{self, AccesoDePersona, ChatCompartido, CompartidosEnDisco};
use crate::canonico::{self, Json};
use crate::diferencia::{self, Apunte, Paso};
use crate::disco::{self, Disco, Identidad, es_texto, permitida};
use crate::fusion::{self, Criterio, Cuenta};
use crate::galeria::{self, CapturasDelAparato};
use crate::grupo;
use crate::kotlin;
use crate::mensajes::{
    Aparato, ArchivoInfo, Chat, Hola, LapidaDeChat, OCUPADO, Peticion, Respuesta,
};
use crate::mezcla::Mando;

/// Lo que puede salir mal hablando con el otro.
#[derive(Debug, thiserror::Error)]
pub enum ErrorSincro {
    #[error(transparent)]
    Canal(#[from] ErrorCanal),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("JSON que no se entiende: {0}")]
    Json(#[from] serde_json::Error),
    /// Lo que contesto el otro como error, tal cual (por ejemplo
    /// [OCUPADO]).
    #[error("{0}")]
    Remoto(String),
    /// Algo que el otro hizo fuera del protocolo, o lo que dice el movil en
    /// las mismas circunstancias.
    #[error("{0}")]
    Protocolo(String),
}

impl ErrorSincro {
    /// Si la conexion se corto sin despedirse.
    pub fn es_corte(&self) -> bool {
        let io = match self {
            ErrorSincro::Canal(ErrorCanal::Io(e)) | ErrorSincro::Io(e) => e,
            _ => return false,
        };
        matches!(
            io.kind(),
            io::ErrorKind::UnexpectedEof
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::BrokenPipe
        )
    }
}

pub type Resultado<T> = Result<T, ErrorSincro>;

fn fallo(t: impl Into<String>) -> ErrorSincro {
    ErrorSincro::Protocolo(t.into())
}

// ------------------------------------------------------------ una a la vez

/// **Una sincronizacion a la vez en cada aparato** (`Protocolo.ocupar`): si
/// dos aparatos pulsan «sincronizar» a la vez uno con otro, cada uno seria a
/// la vez el que dirige y el que responde, escribiendo el mismo chat desde
/// dos hilos. Por carpeta y no por proceso: en las pruebas los dos
/// «aparatos» viven en el mismo.
static OCUPADOS: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

pub fn ocupar<D: Disco + ?Sized>(d: &D) -> bool {
    OCUPADOS
        .lock()
        .map(|mut s| s.insert(d.raiz()))
        .unwrap_or(false)
}

pub fn soltar<D: Disco + ?Sized>(d: &D) {
    if let Ok(mut s) = OCUPADOS.lock() {
        s.remove(&d.raiz());
    }
}

// ------------------------------------------------------------ el cable

fn enviar<F: Read + Write, T: serde::Serialize>(c: &mut Canal<F>, cosa: &T) -> Resultado<()> {
    c.mandar(Tipo::Json, &serde_json::to_vec(cosa)?)?;
    Ok(())
}

fn leer_peticion<F: Read + Write>(c: &mut Canal<F>) -> Resultado<Peticion> {
    let (tipo, datos) = c.recibir()?;
    if tipo != Tipo::Json {
        return Err(fallo("Se esperaba una petición"));
    }
    Ok(serde_json::from_slice(&datos)?)
}

/// La peticion y el JSON tal cual: `suelto` trae campos que `Peticion` no
/// tiene (`nombre`, `mime`, `destino`).
fn leer_peticion_cruda<F: Read + Write>(c: &mut Canal<F>) -> Resultado<(Peticion, Vec<u8>)> {
    let (tipo, datos) = c.recibir()?;
    if tipo != Tipo::Json {
        return Err(fallo("Se esperaba una petición"));
    }
    Ok((serde_json::from_slice(&datos)?, datos))
}

/// `Protocolo.leerRespuesta`: un error del otro se convierte en error aqui.
fn leer_respuesta<F: Read + Write>(c: &mut Canal<F>) -> Resultado<Respuesta> {
    let (tipo, datos) = c.recibir()?;
    if tipo != Tipo::Json {
        return Err(fallo("Se esperaba una respuesta"));
    }
    let r: Respuesta = serde_json::from_slice(&datos)?;
    if let Some(e) = r.error {
        return Err(ErrorSincro::Remoto(e));
    }
    Ok(r)
}

/// Un archivo listo para salir (`Protocolo.Salida`).
pub struct Salida {
    pub largo: i64,
    fuente: Fuente,
}

enum Fuente {
    Texto(Vec<u8>),
    Archivo(PathBuf, i64),
}

impl Salida {
    /// `Protocolo.salida`: lo de texto sale portatil y descomprimido; lo
    /// demas, tal cual y a trozos, que un PDF de 300 MB no cabe en memoria.
    pub fn de<D: Disco + ?Sized>(d: &D, chat: &str, rel: &str) -> Resultado<Salida> {
        if es_texto(rel) {
            let b = d.texto_de(chat, rel)?.into_bytes();
            return Ok(Salida {
                largo: b.len() as i64,
                fuente: Fuente::Texto(b),
            });
        }
        let f = d.ruta(chat, rel);
        let meta = std::fs::metadata(&f)?;
        Ok(Salida {
            largo: meta.len() as i64,
            fuente: Fuente::Archivo(f, disco::milis(&meta)),
        })
    }

    /// Un fichero cualquiera de este equipo, tal cual y a trozos, sin pasar
    /// por un `Disco`: es lo que manda «una foto al lienzo del movil»
    /// (`al_lienzo`), que no es de ningun chat. Mismo troceo y misma cola que
    /// un archivo de la sincronizacion, para que el otro lado lo reciba con
    /// el `recibirTrozos` de siempre.
    pub fn de_fichero(ruta: &std::path::Path) -> Resultado<Salida> {
        let meta = std::fs::metadata(ruta)?;
        Ok(Salida {
            largo: meta.len() as i64,
            fuente: Fuente::Archivo(ruta.to_path_buf(), disco::milis(&meta)),
        })
    }

    /// Manda los trozos y detras la cola con el resumen. Devuelve el resumen,
    /// o None si el archivo cambio mientras se leia (la cola dice
    /// «saltado» y el otro no lo escribe: medio viejo y medio nuevo no vale).
    pub fn mandar<F: Read + Write>(
        self,
        c: &mut Canal<F>,
        avance: &mut dyn FnMut(u64),
    ) -> Resultado<Option<String>> {
        use sha2::Digest;
        match self.fuente {
            Fuente::Texto(b) => {
                for trozo in b.chunks(TOPE_DE_TRAMO) {
                    c.mandar(Tipo::Trozo, trozo)?;
                    avance(trozo.len() as u64);
                }
                let r = canonico::sha256_hex(&b);
                enviar(
                    c,
                    &Respuesta {
                        resumen: Some(r.clone()),
                        ..Default::default()
                    },
                )?;
                Ok(Some(r))
            }
            Fuente::Archivo(ruta, fecha) => {
                let largo = self.largo as u64;
                let mut md = sha2::Sha256::new();
                let mut corto = false;
                let mut entrada = std::fs::File::open(&ruta)?;
                let mut buf = vec![0u8; TOPE_DE_TRAMO];
                let mut quedan = largo;
                while quedan > 0 {
                    let quiero = (buf.len() as u64).min(quedan) as usize;
                    let mut n = 0;
                    while n < quiero {
                        let r = entrada.read(&mut buf[n..quiero])?;
                        if r == 0 {
                            break;
                        }
                        n += r;
                    }
                    // Se acorto a medias: se rellena para cumplir lo
                    // prometido y la cola lo invalida.
                    if n < quiero {
                        corto = true;
                        buf[n..quiero].fill(0);
                    }
                    md.update(&buf[..quiero]);
                    c.mandar(Tipo::Trozo, &buf[..quiero])?;
                    quedan -= quiero as u64;
                    avance(quiero as u64);
                }
                let ahora = std::fs::metadata(&ruta).ok();
                let cambio = corto
                    || ahora
                        .as_ref()
                        .is_none_or(|m| m.len() != largo || disco::milis(m) != fecha);
                let r: String = md.finalize().iter().map(|b| format!("{b:02x}")).collect();
                enviar(
                    c,
                    &Respuesta {
                        resumen: Some(r.clone()),
                        saltado: cambio,
                        ..Default::default()
                    },
                )?;
                Ok((!cambio).then_some(r))
            }
        }
    }
}

/// `Protocolo.recibirTrozos`: `largo` bytes hacia `salida` y la cola.
/// Devuelve el resumen de lo llegado, o None si el que manda dice que no
/// vale; falla si lo llegado no es lo que se mando.
pub fn recibir_trozos<F: Read + Write>(
    c: &mut Canal<F>,
    largo: i64,
    salida: &mut dyn Write,
    avance: &mut dyn FnMut(u64),
) -> Resultado<Option<String>> {
    use sha2::Digest;
    let mut md = sha2::Sha256::new();
    let mut quedan = largo;
    while quedan > 0 {
        let (tipo, datos) = c.recibir()?;
        if tipo != Tipo::Trozo {
            return Err(fallo("Se esperaba un trozo de archivo"));
        }
        salida.write_all(&datos)?;
        md.update(&datos);
        quedan -= datos.len() as i64;
        avance(datos.len() as u64);
    }
    let cola = leer_respuesta(c)?;
    if cola.saltado {
        return Ok(None);
    }
    let r: String = md.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if cola.resumen.as_ref().is_some_and(|x| *x != r) {
        return Err(fallo("Un archivo llegó distinto de como salió"));
    }
    Ok(Some(r))
}

/// `Protocolo.leerRespuestaSinLanzar`: una respuesta con su error dentro en
/// vez de convertirlo en error aqui, para lo opcional del protocolo (la
/// galeria, que un aparato de antes no conoce).
fn leer_respuesta_sin_lanzar<F: Read + Write>(c: &mut Canal<F>) -> Resultado<Respuesta> {
    let (tipo, datos) = c.recibir()?;
    if tipo != Tipo::Json {
        return Err(fallo("Se esperaba una respuesta"));
    }
    Ok(serde_json::from_slice(&datos)?)
}

/// Un escritor que **aguanta los fallos del disco**: el primero se apunta y
/// lo que sigue se tira. Asi un disco lleno a medio recibir no deja trozos
/// sin leer en el cable (la conversacion seguiria descuadrada): se leen
/// todos y la captura queda como no recibida.
struct Aguanta<'w> {
    dentro: &'w mut dyn Write,
    fallo: bool,
}

impl Write for Aguanta<'_> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if !self.fallo && self.dentro.write_all(b).is_err() {
            self.fallo = true;
        }
        Ok(b.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.fallo && self.dentro.flush().is_err() {
            self.fallo = true;
        }
        Ok(())
    }
}

/// **Recibe una captura** de `largo` bytes que ya viene por el cable y la
/// guarda con lo que dice su entrada. `true` si quedo entera. Pase lo que
/// pase aqui, los trozos se leen todos: si no se pudo ni empezar a guardar,
/// se leen y se tiran. Solo falla si fallo la red.
fn recibir_captura<F: Read + Write>(
    canal: &mut Canal<F>,
    c: &dyn CapturasDelAparato,
    e: &galeria::Entrada,
    largo: i64,
    avance: &mut dyn FnMut(u64),
) -> Resultado<bool> {
    let mut leidos = false;
    let mut completo = false;
    let guardado = c.guardar(e, &mut |w| {
        leidos = true;
        let mut a = Aguanta {
            dentro: w,
            fallo: false,
        };
        let resumen = recibir_trozos(canal, largo, &mut a, avance)?;
        completo = true;
        Ok(resumen.is_some() && !a.fallo)
    });
    let entera = match guardado {
        Ok(b) => b,
        // A medio leer y por la red: no hay conversacion que salvar.
        Err(err) if leidos && !completo => return Err(err),
        // Lo demas (no se pudo crear el fichero, ni ponerlo en su sitio)
        // es de este aparato: la captura se queda para otra vez.
        Err(_) => false,
    };
    if !leidos {
        recibir_trozos(canal, largo, &mut io::sink(), &mut |_| {})?;
    }
    Ok(entera)
}

fn json(texto: &str) -> Resultado<Json> {
    Json::analizar(texto).map_err(|e| fallo(e.to_string()))
}

fn io_de(e: ErrorSincro) -> io::Error {
    match e {
        ErrorSincro::Io(e) => e,
        otro => io::Error::other(otro.to_string()),
    }
}

// ------------------------------------------------------------ parches

/// `Parches.poner`: pone `parche` sobre la version `desde` y lo escribe si
/// sale lo que tiene que salir. false: hay que mandarlo entero.
pub fn poner_parche<D: Disco + ?Sized>(
    d: &D,
    chat: &str,
    rel: &str,
    desde: Option<&str>,
    parche: Option<&str>,
    resumen: Option<&str>,
) -> io::Result<bool> {
    let (Some(desde), Some(parche), Some(resumen)) = (desde, parche, resumen) else {
        return Ok(false);
    };
    let Some(base) = d.texto_base(chat, rel, desde) else {
        return Ok(false);
    };
    let (Ok(b), Ok(p)) = (Json::analizar(&base), Json::analizar(parche)) else {
        return Ok(false);
    };
    let Ok(Some(puesto)) = fusion::aplicar(Some(&b), Some(&p)) else {
        return Ok(false);
    };
    let texto = puesto.a_texto();
    if canonico::resumen(&texto) != resumen {
        return Ok(false);
    }
    d.escribir_texto(chat, rel, &texto)
}

/// `Parches.sacar`: lo que cambio de la version `desde` a la de ahora, y el
/// resumen de la de ahora. None si aqui no esta esa version.
pub fn sacar_parche<D: Disco + ?Sized>(
    d: &D,
    chat: &str,
    rel: &str,
    desde: Option<&str>,
) -> io::Result<Option<(String, String)>> {
    let Some(desde) = desde else {
        return Ok(None);
    };
    let Some(base) = d.texto_base(chat, rel, desde) else {
        return Ok(None);
    };
    let actual = d.texto_de(chat, rel)?;
    let parche = match (Json::analizar(&base), Json::analizar(&actual)) {
        (Ok(b), Ok(a)) => fusion::diferencia(Some(&b), Some(&a)).map(|p| p.a_texto()),
        _ => None,
    };
    Ok(Some((
        parche.unwrap_or_else(|| "null".into()),
        canonico::resumen(&actual),
    )))
}

// ------------------------------------------------------------ responder

/// Quien da, para cada archivo `suelto` que llega de `otro`, al que lo recibe.
pub type FabricaDeSueltos<'a> = dyn Fn(&Aparato) -> Box<dyn crate::al_lienzo::Recibe + 'a> + 'a;

/// **El lado que responde.** Atiende una conexion entera. Si viene alguien a
/// unirse al grupo, le da la primera letra libre.
pub struct Respondedor<'a, D: Disco + ?Sized> {
    pub disco: &'a D,
    /// Que esta pasando, en palabras, para la pantalla del que responde.
    pub estado: &'a dyn Fn(&str),
    pub ahora: &'a dyn Fn() -> i64,
    /// Donde escucha este aparato, para decirselo al otro.
    pub mi_puerto: u32,
    /// Quien vino y en que puerto escucha el.
    pub al_saludar: &'a dyn Fn(&Aparato, u32),
    /// Quien atiende un archivo `suelto` (`al_lienzo`), uno nuevo por
    /// archivo. Sin el, «No sé qué es «suelto»», como un aparato de antes.
    /// No hace falta estar libre para esto: no toca lo sincronizado.
    pub suelto: Option<&'a FabricaDeSueltos<'a>>,
}

impl<D: Disco + ?Sized> Respondedor<'_, D> {
    /// Un `suelto` ya leido: lo atiende quien diga [`Respondedor::suelto`].
    /// Solo devuelve error si la red se corto.
    fn atender_suelto<F: Read + Write>(
        &self,
        canal: &mut Canal<F>,
        otro: &Aparato,
        cruda: &[u8],
    ) -> Resultado<()> {
        let Some(fabrica) = self.suelto else {
            return enviar(
                canal,
                &Respuesta {
                    error: Some(format!("No sé qué es «{}»", crate::al_lienzo::SUELTO)),
                    ..Default::default()
                },
            );
        };
        let p: crate::al_lienzo::Suelto = serde_json::from_slice(cruda)?;
        let carpeta = std::env::temp_dir().join("pixpin-suelto");
        let mut quien = fabrica(otro);
        crate::al_lienzo::responder(canal, &p, &carpeta, &mut *quien).map_err(|e| match e {
            crate::al_lienzo::ErrorAlLienzo::Canal(c) => ErrorSincro::Canal(c),
            crate::al_lienzo::ErrorAlLienzo::Io(i) => ErrorSincro::Io(i),
            otro => fallo(otro.to_string()),
        })
    }

    pub fn atender<F: Read + Write>(&self, flujo: F, nonce: [u8; 32]) -> Resultado<()> {
        self.atender_con_galeria(flujo, nonce, None)
    }

    /// [`Respondedor::atender`] con la galeria de capturas de este aparato
    /// (`galeria`). Sin ella, las peticiones de la galeria se contestan «No
    /// sé qué es», como un aparato de antes. Va aparte y no como un campo
    /// mas para no romper a quien ya construye el `Respondedor` (el PC
    /// simulado de las pruebas de Android, entre otros).
    pub fn atender_con_galeria<F: Read + Write>(
        &self,
        flujo: F,
        nonce: [u8; 32],
        galeria: Option<&dyn CapturasDelAparato>,
    ) -> Resultado<()> {
        let d = self.disco;
        let id = d.identidad()?;
        let codigo = id
            .codigo
            .clone()
            .ok_or_else(|| fallo("Este aparato no está en un grupo"))?;
        let clave = crate::codigo::clave_de_grupo(&codigo);
        let mut canal = Canal::saludar(flujo, &clave, nonce, false)?;
        let primera = leer_peticion(&mut canal)?;
        let hola = primera.hola.ok_or_else(|| fallo("Faltó el saludo"))?;
        let ahora = (self.ahora)();
        let actual = d.identidad()?;
        let base = miembros_o_yo(&actual);
        let otro;
        if hola.unirme {
            let miembros = grupo::juntar(&base, &hola.miembros, None);
            // Si vuelve alguien que ya estuvo, recupera su letra: lo que
            // sello con ella sigue siendo suyo.
            let suya = miembros
                .iter()
                .find(|x| x.id == hola.yo.id)
                .and_then(|x| x.letra.as_ref())
                .and_then(|l| l.chars().next());
            let ocupadas: Vec<char> = miembros
                .iter()
                .filter(|x| x.id != hola.yo.id)
                .filter_map(|x| x.letra.as_ref().and_then(|l| l.chars().next()))
                .collect();
            let libre = suya
                .or_else(|| grupo::sena::libre(&ocupadas))
                .ok_or_else(|| fallo("El grupo ya tiene 26 aparatos"))?;
            let nuevo = Aparato {
                letra: Some(libre.to_string()),
                desde: ahora,
                ..hola.yo.clone()
            };
            let mut juntos = grupo::juntar(
                &miembros
                    .into_iter()
                    .filter(|x| x.id != nuevo.id)
                    .collect::<Vec<_>>(),
                &[],
                None,
            );
            juntos.push(nuevo.clone());
            d.guardar_identidad(&Identidad {
                miembros: juntos.clone(),
                ..actual.clone()
            })?;
            enviar(
                &mut canal,
                &respuesta_hola(&actual, juntos, ahora, self.mi_puerto),
            )?;
            d.avisar(disco::Cambio::Identidad);
            (self.estado)(&format!(
                "{} se unió al grupo con la letra {libre}",
                nuevo.nombre
            ));
            otro = nuevo;
        } else {
            let juntos = grupo::juntar(&base, &hola.miembros, Some(&hola.yo));
            d.guardar_identidad(&Identidad {
                miembros: juntos.clone(),
                ..actual.clone()
            })?;
            d.avisar(disco::Cambio::Identidad);
            otro = hola.yo.clone();
            enviar(
                &mut canal,
                &respuesta_hola(&actual, juntos, ahora, self.mi_puerto),
            )?;
        }
        (self.al_saludar)(&otro, hola.puerto);
        if !ocupar(d) {
            // Un archivo suelto no toca lo sincronizado: se atiende aunque
            // este aparato este ocupado con otro.
            return self.contestar_ocupado(&mut canal, &otro, true);
        }
        let hecho = self.responder(&mut canal, &otro, galeria, None);
        soltar(d);
        hecho
    }

    fn responder<F: Read + Write>(
        &self,
        canal: &mut Canal<F>,
        otro: &Aparato,
        galeria: Option<&dyn CapturasDelAparato>,
        acceso: Option<&AccesoDePersona<'_, D>>,
    ) -> Resultado<()> {
        // Lo que es de cada chat, calculado al preguntar por sus archivos:
        // sin esto, cada archivo pedido volveria a recorrer el chat entero.
        let mut alcance: Option<(String, HashSet<String>)> = None;
        // Los resumenes ya calculados en esta conexion.
        let mut conocidos: HashMap<String, ArchivoInfo> = HashMap::new();
        loop {
            let (p, cruda) = match leer_peticion_cruda(canal) {
                Ok(p) => p,
                Err(e) if e.es_corte() => return Ok(()),
                Err(e) => return Err(e),
            };
            // **A una persona de un chat compartido, lo del aparato entero no
            // se le contesta**: como un PixPin de antes (`SOLO_DEL_GRUPO`).
            // (Un `suelto` espera el «vale» antes de mandar los trozos: con
            // el «no», no llega ninguno y la conversacion sigue sana.)
            if acceso.is_some() && solo_del_grupo(&p.t) {
                enviar(
                    canal,
                    &Respuesta {
                        error: Some(format!("No sé qué es «{}»", p.t)),
                        ..Default::default()
                    },
                )?;
                continue;
            }
            if p.t == crate::al_lienzo::SUELTO {
                self.atender_suelto(canal, otro, &cruda)?;
                continue;
            }
            if p.t == "adios" {
                enviar(canal, &Respuesta::default())?;
                canal.vaciar()?;
                return Ok(());
            }
            // **La galeria** (8-oct-2026). Sin capturas en este aparato,
            // como uno de antes: «No sé qué es».
            let hecho = if galeria::es_de_galeria(&p.t) {
                match galeria {
                    Some(c) => self.responder_galeria(canal, &p, c),
                    None => Err(fallo(format!("No sé qué es «{}»", p.t))),
                }
            } else {
                self.una(canal, otro, &p, &mut alcance, &mut conocidos, acceso)
            };
            match hecho {
                Ok(()) => {}
                // Lo de la red corta: el otro ya no esta.
                Err(e @ ErrorSincro::Canal(_)) => return Err(e),
                // Lo demas se le cuenta al otro, que lo ensena tal cual.
                Err(e) => enviar(
                    canal,
                    &Respuesta {
                        error: Some(e.to_string()),
                        ..Default::default()
                    },
                )?,
            }
        }
    }

    /// **Las cuatro peticiones de la galeria** (`Respondedor.responderGaleria`).
    /// Solo devuelve un error de red si la conversacion ya no se puede
    /// seguir; lo demas va al otro como `{"error":…}`.
    fn responder_galeria<F: Read + Write>(
        &self,
        canal: &mut Canal<F>,
        p: &Peticion,
        c: &dyn CapturasDelAparato,
    ) -> Resultado<()> {
        let ahora = (self.ahora)();
        match p.t.as_str() {
            galeria::PETICION => {
                (self.estado)("Comparando la galería…");
                let yo = self.disco.identidad()?.yo.id;
                let e = galeria::poner_al_dia(c, &yo, ahora)?;
                enviar(
                    canal,
                    &Respuesta {
                        galeria: Some(galeria::entradas_a_texto(&e.entradas)),
                        tengo: e.tenia.into_iter().collect(),
                        ..Default::default()
                    },
                )
            }
            galeria::JUNTA => {
                let juntas = galeria::entradas_de_texto(p.parche.as_deref().unwrap_or("[]"))?;
                galeria::aplicar_juntas(c, &juntas, ahora)?;
                c.avisar();
                enviar(canal, &Respuesta::default())
            }
            galeria::DAME => {
                let n = galeria::nombre_valido(p.ruta.as_deref()).map_err(fallo)?;
                let sale = c
                    .abrir(&n)
                    .and_then(|r| Salida::de_fichero(&r).ok())
                    .ok_or_else(|| fallo(format!("No tengo «{n}»")))?;
                (self.estado)(&format!("Mandando «{n}»"));
                enviar(
                    canal,
                    &Respuesta {
                        bytes: sale.largo,
                        ..Default::default()
                    },
                )?;
                sale.mandar(canal, &mut |_| {})?;
                Ok(())
            }
            galeria::PON => {
                let n = match galeria::nombre_valido(p.ruta.as_deref()) {
                    Ok(n) => n,
                    Err(m) => {
                        // Los trozos ya vienen de camino: se leen y se tiran,
                        // o la conversacion se descuadra.
                        recibir_trozos(canal, p.bytes, &mut io::sink(), &mut |_| {})?;
                        return Err(fallo(m));
                    }
                };
                // Lo que dice de ella: lo juntado, que llego justo antes con
                // `galeriajunta`.
                let e = galeria::leer(&c.raiz())
                    .entradas
                    .into_iter()
                    .find(|x| x.nombre == n)
                    .unwrap_or_else(|| galeria::Entrada::nueva(&n, ahora));
                (self.estado)(&format!("Recibiendo «{n}»"));
                let entera = recibir_captura(canal, c, &e, p.bytes, &mut |_| {})?;
                if entera {
                    galeria::apuntar_que_llego(c, &n)?;
                    c.avisar();
                }
                enviar(
                    canal,
                    &Respuesta {
                        saltado: !entera,
                        ..Default::default()
                    },
                )
            }
            otra => Err(fallo(format!("No sé qué es «{otra}»"))),
        }
    }

    fn una<F: Read + Write>(
        &self,
        canal: &mut Canal<F>,
        otro: &Aparato,
        p: &Peticion,
        alcance: &mut Option<(String, HashSet<String>)>,
        conocidos: &mut HashMap<String, ArchivoInfo>,
        acceso: Option<&AccesoDePersona<'_, D>>,
    ) -> Resultado<()> {
        let d = self.disco;
        let ahora = (self.ahora)();
        // `chatPermitido`: con una persona, cualquier peticion con un chat
        // que no compartimos los dos es un error, sea cual sea.
        let chat = || -> Resultado<String> {
            let c = p.chat.clone().ok_or_else(|| fallo("Falta el chat"))?;
            if let Some(a) = acceso
                && !a.puede(&c)
            {
                return Err(fallo("No es un chat compartido contigo"));
            }
            Ok(c)
        };
        let ruta = || p.ruta.clone().ok_or_else(|| fallo("Falta la ruta"));
        let nombre = |rel: &str| rel.rsplit('/').next().unwrap_or(rel).to_string();
        match p.t.as_str() {
            // **Va TODO el catalogo a proposito, no es un descuido.**
            //
            // Comprobado contra el movil: `Protocolo.kt:290` (rama `main`)
            // es esta misma linea, `"catalogo" -> enviar(Respuesta(chats =
            // disco.chats()))`, sin mirar ningun campo de la peticion; y su
            // `Disco.chats()` (`Disco.kt:156`) devuelve la general mas todos
            // los proyectos. Quien recorta es **el que inicia la vuelta**,
            // con los chats que el usuario marco para ese aparato
            // (`SincronizarActivity.kt:404-423`, aqui `vuelta::una`).
            //
            // Filtrar aqui exigiria un campo nuevo en `Peticion`, y un
            // movil que no lo manda se quedaria sin catalogo: seria romper
            // el alambre para adelantar un recorte que el otro lado ya hace.
            //
            // A una persona de un chat compartido, eso si: solo los chats
            // donde estamos dentro los dos.
            "catalogo" => enviar(
                canal,
                &Respuesta {
                    chats: d
                        .chats()?
                        .into_iter()
                        .filter(|c| acceso.is_none_or(|a| a.puede(&c.id)))
                        .collect(),
                    ..Default::default()
                },
            ),
            // Que proyectos se borraron aqui, para que el otro no los
            // devuelva. A una persona, nada: borrar un chat compartido es
            // salir de el, no borrarselo a ella.
            "lapidas" => enviar(
                canal,
                &Respuesta {
                    lapidas: if acceso.is_none() {
                        d.lapidas()
                    } else {
                        Vec::new()
                    },
                    ..Default::default()
                },
            ),
            // Borralo tu tambien: alli se borro y aqui no se ha tocado desde
            // entonces.
            "borrarchat" => {
                if acceso.is_some() {
                    return Err(fallo("Una persona no borra proyectos"));
                }
                let c = chat()?;
                (self.estado)(&format!(
                    "Borrando «{}», borrado en {}…",
                    nombre_de(d, &c),
                    otro.nombre
                ));
                d.borrar_chat(
                    &c,
                    &format!("Antes de borrarlo, borrado en {}", otro.nombre),
                    ahora,
                    &otro.id,
                )?;
                enviar(canal, &Respuesta::default())
            }
            "inventario" => {
                let c = chat()?;
                (self.estado)(&format!(
                    "Comparando «{}» con {}…",
                    nombre_de(d, &c),
                    otro.nombre
                ));
                // Antes de tocar nada, como estaba. Si la copia no se pudo
                // hacer, la vuelta se para aqui: sin ella no hay con que
                // volver si lo que llega viene roto.
                d.hacer_copia(
                    &c,
                    &format!("Antes de sincronizar con {}", otro.nombre),
                    ahora,
                )?;
                d.sellar()?;
                d.adoptar_documentos()?;
                enviar(
                    canal,
                    &Respuesta {
                        apuntes: d.apuntes(&c)?,
                        proyecto: d.proyecto_portatil(&c)?.map(|p| p.a_texto()),
                        sello_de_base: d.base(&otro.id, &c).map(|b| b.sello()),
                        ..Default::default()
                    },
                )
            }
            "mensajes" => {
                let suyos = d.mensajes_por_clave(&chat()?)?;
                let mensajes = p
                    .senas
                    .iter()
                    .filter_map(|s| suyos.iter().find(|(k, _)| k == s).map(|(_, m)| m.a_texto()))
                    .collect();
                enviar(
                    canal,
                    &Respuesta {
                        mensajes,
                        ..Default::default()
                    },
                )
            }
            "aplicar" => {
                let c = chat()?;
                if let Some(a) = acceso {
                    // **De una persona, solo lo de este chat**: ni mensajes
                    // que nombren archivos de otros chats, ni otro proyecto;
                    // y al borrar, nada de otros chats.
                    let vedado = |rel: &str| a.vedado(&c, rel);
                    let poner = crate::valla::solo_de_este_chat(d, &c, &p.poner, &vedado);
                    crate::valla::aplicar_mensajes_guardando(
                        d,
                        &c,
                        &poner,
                        &p.borrar,
                        ahora,
                        &a.prohibidos(),
                    )?;
                    let llega = p.proyecto.as_deref().and_then(kotlin::proyecto_de_texto);
                    if let Some(pr) = crate::valla::proyecto_seguro(d, &c, llega, &vedado) {
                        d.guardar_proyecto(&pr)?;
                    }
                    return enviar(canal, &Respuesta::default());
                }
                d.aplicar_mensajes(&c, &p.poner, &p.borrar, ahora)?;
                if let Some(pr) = p.proyecto.as_deref().and_then(kotlin::proyecto_de_texto) {
                    // Si llega el proyecto es que alli decidieron que sigue
                    // vivo: se levanta la lapida o la vuelta siguiente lo
                    // borraria otra vez.
                    d.quitar_lapida(&c)?;
                    d.guardar_proyecto(&pr)?;
                }
                enviar(canal, &Respuesta::default())
            }
            "archivos" => {
                let c = chat()?;
                let mut lista = d.archivos(&c, conocidos)?;
                if let Some(a) = acceso {
                    lista.retain(|x| !a.vedado(&c, &x.ruta));
                }
                for a in &lista {
                    conocidos.insert(a.ruta.clone(), a.clone());
                }
                *alcance = Some((c, lista.iter().map(|a| a.ruta.clone()).collect()));
                enviar(
                    canal,
                    &Respuesta {
                        archivos: lista,
                        ..Default::default()
                    },
                )
            }
            "pon" => {
                let rel = ruta()?;
                // **De una persona, solo lo de este chat**: los trozos ya
                // vienen por el cable; se leen y se tiran, o la conversacion
                // se descuadra. Y se dice que no.
                if let Some(a) = acceso
                    && !p.chat.as_deref().is_some_and(|c| a.puede_escribir(c, &rel))
                {
                    recibir_trozos(canal, p.bytes, &mut io::sink(), &mut |_| {})?;
                    return Err(fallo(format!("No es de este chat: {rel}")));
                }
                // Sin `chat` y sin un «archivos» previo que lo diga no se
                // sabe DE QUE conversacion es esto. Antes caia a la cadena
                // vacia, que no es un chat: el fichero se escribia con un id
                // que no existe y quedaba fuera de toda conversacion. Se
                // rechaza como una ruta no permitida, leyendo los trozos que
                // ya vienen para no perder el paso.
                let Some(c) = p
                    .chat
                    .clone()
                    .or_else(|| alcance.as_ref().map(|a| a.0.clone()))
                    .filter(|c| !c.trim().is_empty())
                else {
                    recibir_trozos(canal, p.bytes, &mut io::sink(), &mut |_| {})?;
                    return Err(fallo(format!("«pon» sin chat para {rel}")));
                };
                (self.estado)(&format!("Recibiendo {}", nombre(&rel)));
                conocidos.remove(&rel);
                if !permitida(&rel) {
                    // Los trozos ya vienen: se leen y se tiran para no
                    // perder el paso de la conversacion.
                    recibir_trozos(canal, p.bytes, &mut io::sink(), &mut |_| {})?;
                    return Err(fallo(format!("Ruta no permitida: {rel}")));
                }
                let mut roto = None;
                let entero = d.escribir_archivo(&c, &rel, &mut |s| match recibir_trozos(
                    canal,
                    p.bytes,
                    s,
                    &mut |_| {},
                ) {
                    Ok(r) => Ok(r.is_some()),
                    Err(e) => {
                        let texto = e.to_string();
                        roto = Some(e);
                        Err(io::Error::other(texto))
                    }
                });
                if let Some(e) = roto {
                    return Err(e);
                }
                enviar(
                    canal,
                    &Respuesta {
                        saltado: !entero?,
                        ..Default::default()
                    },
                )
            }
            "parche" => {
                let rel = ruta()?;
                let c = p.chat.clone().unwrap_or_default();
                if !(permitida(&rel) && es_texto(&rel)) {
                    return Err(fallo(format!("No se puede parchear: {rel}")));
                }
                if let Some(a) = acceso {
                    let c = chat()?;
                    if !a.puede_escribir(&c, &rel) {
                        return Err(fallo(format!("No es de este chat: {rel}")));
                    }
                }
                (self.estado)(&format!("Recibiendo cambios de {}", nombre(&rel)));
                conocidos.remove(&rel);
                let puesto = poner_parche(
                    d,
                    &c,
                    &rel,
                    p.desde.as_deref(),
                    p.parche.as_deref(),
                    p.resumen.as_deref(),
                )?;
                enviar(
                    canal,
                    &Respuesta {
                        falta_base: !puesto,
                        ..Default::default()
                    },
                )
            }
            "damecambios" => {
                let rel = ruta()?;
                let c = chat()?;
                // De una persona, ademas, nada que sea tambien de otro chat.
                let es_suyo = suyo(d, alcance, &c, &rel)?
                    && acceso.is_none_or(|a| !a.vedado(&c, &rel));
                if !(permitida(&rel) && es_texto(&rel) && es_suyo) {
                    return Err(fallo(format!("No es de este chat: {rel}")));
                }
                (self.estado)(&format!("Mandando cambios de {}", nombre(&rel)));
                let r = match sacar_parche(d, &c, &rel, p.desde.as_deref())? {
                    None => Respuesta {
                        falta_base: true,
                        ..Default::default()
                    },
                    Some((parche, resumen)) => Respuesta {
                        parche: Some(parche),
                        resumen: Some(resumen),
                        ..Default::default()
                    },
                };
                enviar(canal, &r)
            }
            "dame" => {
                let rel = ruta()?;
                let c = chat()?;
                // De una persona, ademas, nada que sea tambien de otro chat.
                let es_suyo = suyo(d, alcance, &c, &rel)?
                    && acceso.is_none_or(|a| !a.vedado(&c, &rel));
                if !(permitida(&rel) && es_suyo) {
                    return Err(fallo(format!("No es de este chat: {rel}")));
                }
                (self.estado)(&format!("Mandando {}", nombre(&rel)));
                let sale = Salida::de(d, &c, &rel)?;
                enviar(
                    canal,
                    &Respuesta {
                        bytes: sale.largo,
                        ..Default::default()
                    },
                )?;
                sale.mandar(canal, &mut |_| {})?;
                Ok(())
            }
            "base" => {
                let c = chat()?;
                let base = p.base.clone().unwrap_or_default();
                d.guardar_base(&otro.id, &c, &base)?;
                d.guardar_objetos_de_base(&c, &base);
                d.apuntar_vez(&otro.id, ahora)?;
                enviar(canal, &Respuesta::default())?;
                (self.estado)(&format!(
                    "«{}» al día con {}",
                    nombre_de(d, &c),
                    otro.nombre
                ));
                Ok(())
            }
            // **Las listas de los chats compartidos, entre aparatos del
            // grupo** (13-oct-2026): se juntan enteras —son mias— y el otro
            // aparato entra en los que estoy yo.
            "compartidos" if acceso.is_none() => {
                let cd = CompartidosEnDisco::nuevo(&d.raiz());
                let yo = cd.yo_de(d)?;
                let su_ficha = compartidos::ficha_de_texto(p.persona.as_deref())
                    .filter(|f| f.id == otro.id);
                let suyos = compartidos::de_texto(p.compartidos.as_deref());
                let juntos = cd.cambiar(|mios| -> io::Result<_> {
                    Ok(compartidos::juntar_del_grupo(
                        &mios,
                        &suyos,
                        &yo,
                        su_ficha.as_ref(),
                        ahora,
                    ))
                })?;
                d.avisar(disco::Cambio::Identidad);
                enviar(
                    canal,
                    &Respuesta {
                        compartidos: Some(compartidos::a_texto(&juntos)),
                        persona: Some(compartidos::ficha_a_texto(&yo.ficha(0))),
                        ..Default::default()
                    },
                )
            }
            // **Las listas, con una persona**: solo las de los chats donde
            // constamos los dos, y sin admitir chats nuevos. Asi se entera
            // cada uno de quien entro y quien salio.
            "miembros" => {
                let Some(a) = acceso else {
                    return Err(fallo("No sé qué es «miembros»"));
                };
                let cd = CompartidosEnDisco::nuevo(&d.raiz());
                let rastro = compartidos::con_rastro(&cd.leer(), &a.yo, &otro.id);
                let suyos: Vec<ChatCompartido> = compartidos::de_texto(p.compartidos.as_deref())
                    .into_iter()
                    .filter(|c| rastro.contains(&c.chat))
                    .collect();
                let juntos = cd.cambiar(|mios| -> io::Result<_> {
                    Ok(compartidos::juntar_todos(&mios, &suyos, |_| false))
                })?;
                let mios: Vec<ChatCompartido> = juntos
                    .into_iter()
                    .filter(|c| rastro.contains(&c.chat))
                    .collect();
                enviar(
                    canal,
                    &Respuesta {
                        compartidos: Some(compartidos::a_texto(&mios)),
                        ..Default::default()
                    },
                )
            }
            otra => Err(fallo(format!("No sé qué es «{otra}»"))),
        }
    }

    /// Se sigue contestando «ocupado» hasta que el otro se despida o corte.
    /// Cerrar aqui mismo, como el movil, en Windows manda un RST si su
    /// siguiente peticion ya viene de camino, y ese RST se lleva el «ocupado»
    /// sin leer.
    fn contestar_ocupado<F: Read + Write>(
        &self,
        canal: &mut Canal<F>,
        otro: &Aparato,
        con_sueltos: bool,
    ) -> Resultado<()> {
        let ocupado = Respuesta {
            error: Some(OCUPADO.into()),
            ..Default::default()
        };
        enviar(canal, &ocupado)?;
        canal.vaciar()?;
        while let Ok((p, cruda)) = leer_peticion_cruda(canal) {
            if con_sueltos && p.t == crate::al_lienzo::SUELTO {
                if self.atender_suelto(canal, otro, &cruda).is_err() {
                    break;
                }
                continue;
            }
            let adios = p.t == "adios";
            let r = if adios {
                Respuesta::default()
            } else {
                ocupado.clone()
            };
            if enviar(canal, &r).is_err() || adios {
                break;
            }
        }
        Ok(())
    }

    /// **Atiende a una persona de un chat compartido**
    /// (`Respondedor.atenderPersona`, 13-oct-2026): la conversacion empieza
    /// con `PXC1` y no con el codigo del grupo. Solo se habla con quien esta
    /// en la lista de algun chat compartido conmigo, y solo de los chats
    /// donde estamos dentro los dos ([AccesoDePersona]). Lo del aparato
    /// entero (la galeria, borrar proyectos, archivos sueltos, las listas del
    /// grupo) se le contesta como un PixPin de antes.
    pub fn atender_persona<F: Read + Write>(&self, flujo: F, nonce: [u8; 32]) -> Resultado<()> {
        let d = self.disco;
        let cd = CompartidosEnDisco::nuevo(&d.raiz());
        let yo = cd.yo_de(d)?;
        let lista = cd.leer();
        let mut quien: Option<compartidos::Persona> = None;
        let (mut canal, _) = Canal::entre_personas(flujo, &yo.llaves, nonce, false, |clave| {
            quien = compartidos::por_clave(&lista, &yo.id, &compartidos::b64(clave));
            quien.is_some()
        })?;
        let persona = quien.ok_or(ErrorSincro::Canal(ErrorCanal::NoEsMiembro))?;
        let hola = leer_peticion(&mut canal)?
            .hola
            .ok_or_else(|| fallo("Faltó el saludo"))?;
        // La clave manda: el id que dice tener tiene que ser el de la ficha
        // de esa clave.
        if hola.yo.id != persona.id {
            return Err(fallo("No es quien dice ser"));
        }
        let otro = Aparato {
            id: persona.id.clone(),
            nombre: persona.nombre.clone(),
            ..Default::default()
        };
        enviar(
            &mut canal,
            &Respuesta {
                hola: Some(Hola {
                    yo: Aparato {
                        id: yo.id.clone(),
                        nombre: yo.nombre.clone(),
                        ..Default::default()
                    },
                    reloj: (self.ahora)(),
                    puerto: self.mi_puerto,
                    ..Default::default()
                }),
                ..Default::default()
            },
        )?;
        (self.al_saludar)(&otro, hola.puerto);
        if !ocupar(d) {
            return self.contestar_ocupado(&mut canal, &otro, false);
        }
        let acceso = AccesoDePersona::nuevo(d, &yo.id, &persona.id);
        let hecho = self.responder(&mut canal, &otro, None, Some(&acceso));
        soltar(d);
        hecho
    }

    /// **Alguien viene con una invitacion** (`Respondedor.atenderInvitacion`)
    /// al chat `chat`: el canal va con la clave del codigo de la invitacion
    /// (`PXI1`). Se le mete en la lista, firmado por este aparato, y se le da
    /// la lista entera; el chat en si lo pide despues como persona, ya con su
    /// clave ([Respondedor::atender_persona]). Devuelve su ficha, para decir
    /// quien entro.
    pub fn atender_invitacion<F: Read + Write>(
        &self,
        flujo: F,
        nonce: [u8; 32],
        codigo: &str,
        chat: &str,
    ) -> Resultado<compartidos::Persona> {
        let d = self.disco;
        let clave = crate::invitacion::clave(codigo);
        let mut canal = Canal::saludar_con(flujo, &clave, nonce, false, MAGIA_DE_INVITACION)?;
        let p = leer_peticion(&mut canal)?;
        if p.t != "unirme" {
            return Err(fallo("Se esperaba «unirme»"));
        }
        let hola = p.hola.ok_or_else(|| fallo("Faltó el saludo"))?;
        let ficha = compartidos::ficha_de_texto(p.persona.as_deref())
            .filter(|f| f.id == hola.yo.id && !f.clave.trim().is_empty())
            .ok_or_else(|| fallo("Faltó su ficha"))?;
        let cd = CompartidosEnDisco::nuevo(&d.raiz());
        let yo = cd.yo_de(d)?;
        let ahora = (self.ahora)();
        let hecho = cd.cambiar(|l| -> Resultado<_> {
            let c = l
                .iter()
                .find(|c| c.chat == chat)
                .ok_or_else(|| fallo("Este chat ya no está compartido"))?;
            let nuevo = compartidos::invitar(c, &yo, &ficha, ahora).map_err(fallo)?;
            Ok(l.into_iter()
                .map(|x| if x.chat == chat { nuevo.clone() } else { x })
                .collect())
        });
        let junto = match hecho {
            Ok(l) => l
                .into_iter()
                .find(|c| c.chat == chat)
                .ok_or_else(|| fallo("Este chat ya no está compartido"))?,
            Err(e) => {
                let _ = enviar(
                    &mut canal,
                    &Respuesta {
                        error: Some(e.to_string()),
                        ..Default::default()
                    },
                );
                let _ = canal.vaciar();
                return Err(e);
            }
        };
        enviar(
            &mut canal,
            &Respuesta {
                hola: Some(Hola {
                    yo: Aparato {
                        id: yo.id.clone(),
                        nombre: yo.nombre.clone(),
                        ..Default::default()
                    },
                    reloj: ahora,
                    puerto: self.mi_puerto,
                    ..Default::default()
                }),
                compartidos: Some(compartidos::a_texto(std::slice::from_ref(&junto))),
                ..Default::default()
            },
        )?;
        (self.al_saludar)(
            &Aparato {
                id: ficha.id.clone(),
                nombre: ficha.nombre.clone(),
                ..Default::default()
            },
            hola.puerto,
        );
        // El «adios»: si no llega, lo hecho hecho esta.
        if leer_peticion(&mut canal).is_ok() {
            let _ = enviar(&mut canal, &Respuesta::default());
            let _ = canal.vaciar();
        }
        d.avisar(disco::Cambio::Identidad);
        (self.estado)(&format!("{} entró en «{}»", ficha.nombre, junto.nombre));
        Ok(ficha)
    }
}

/// Lo que es del aparato entero y no de un chat: a una persona no se le
/// contesta (`SOLO_DEL_GRUPO`).
fn solo_del_grupo(t: &str) -> bool {
    t == crate::al_lienzo::SUELTO
        || galeria::es_de_galeria(t)
        || t == "compartidos"
        || t == "borrarchat"
}

fn suyo<D: Disco + ?Sized>(
    d: &D,
    alcance: &Option<(String, HashSet<String>)>,
    chat: &str,
    rel: &str,
) -> Resultado<bool> {
    Ok(match alcance {
        Some((c, rutas)) if c == chat => rutas.contains(rel),
        _ => d.alcance(chat)?.iter().any(|(r, _)| r == rel),
    })
}

fn nombre_de<D: Disco + ?Sized>(d: &D, chat: &str) -> String {
    d.chats()
        .ok()
        .and_then(|l| l.into_iter().find(|c| c.id == chat))
        .map(|c| c.nombre)
        .unwrap_or_else(|| chat.to_string())
}

fn miembros_o_yo(id: &Identidad) -> Vec<Aparato> {
    if id.miembros.is_empty() {
        vec![id.yo.clone()]
    } else {
        id.miembros.clone()
    }
}

fn respuesta_hola(yo: &Identidad, miembros: Vec<Aparato>, ahora: i64, puerto: u32) -> Respuesta {
    Respuesta {
        hola: Some(Hola {
            yo: yo.yo.clone(),
            miembros,
            reloj: ahora,
            puerto,
            ..Default::default()
        }),
        ..Default::default()
    }
}

// ------------------------------------------------------------ dirigir

/// Lo que paso en una vuelta, para contarlo al acabar (`Sesion.Hecho`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hecho {
    pub traidos: usize,
    pub enviados: usize,
    pub borrados: usize,
    pub archivos: usize,
    /// Lo que cambio en los dos aparatos y se junto.
    pub fusionados: usize,
    /// Lo borrado en un lado que se quedo porque en el otro se cambio.
    pub rescatados: usize,
    /// Bytes que no hizo falta mandar porque viajaron solo los cambios.
    pub ahorrados: i64,
    /// Lo que se estaba guardando mientras se mandaba.
    pub saltados: Vec<String>,
    /// Capturas de la galeria que pasaron de un aparato a otro.
    pub capturas: usize,
    /// Las quitadas aqui por quitarse a mano en el otro.
    pub capturas_tiradas: usize,
}

/// El paso 1 preparado: que pasa con los mensajes de un chat.
#[derive(Debug, Clone)]
pub struct Preparado {
    pub chat: String,
    pub pasos: Vec<Paso>,
    pub base: Base,
    suyo_proyecto: Option<String>,
    mios: HashMap<String, Apunte>,
    suyos: HashMap<String, Apunte>,
}

/// El paso 2 preparado: que archivos hay que mover.
#[derive(Debug, Clone)]
pub struct PreparadoArchivos {
    pub chat: String,
    pub pasos: Vec<Paso>,
    acordado: BTreeMap<String, String>,
    mios: HashMap<String, ArchivoInfo>,
    suyos: HashMap<String, ArchivoInfo>,
}

impl PreparadoArchivos {
    /// Cuantos bytes hay que mover como mucho, para la barra.
    pub fn bytes(&self) -> u64 {
        self.pasos
            .iter()
            .map(|p| {
                let m = self.mios.get(p.sena()).map_or(0, |a| a.bytes);
                let s = self.suyos.get(p.sena()).map_or(0, |a| a.bytes);
                m.max(s).max(0) as u64
            })
            .sum()
    }

    fn nombre_de(&self, rel: &str) -> String {
        let e = self
            .mios
            .get(rel)
            .map(|a| a.etiqueta.clone())
            .or_else(|| self.suyos.get(rel).map(|a| a.etiqueta.clone()))
            .unwrap_or_default();
        if e.trim().is_empty() {
            rel.rsplit('/').next().unwrap_or(rel).to_string()
        } else {
            e
        }
    }
}

/// **El lado que dirige**, paso a paso: entre `preparar` y `aplicar` puede
/// haber que preguntarle al usuario.
pub struct Sesion<'a, D: Disco + ?Sized, F: Read + Write> {
    canal: Canal<F>,
    disco: &'a D,
    ahora: Box<dyn Fn() -> i64 + 'a>,
    /// El otro aparato, tal como se presento.
    pub otro: Aparato,
    /// Cuanto va adelantado el reloj del otro, en milisegundos.
    pub desfase: i64,
    /// En que puerto escucha el otro.
    pub puerto_del_otro: u32,
    /// Lo que paso de un lado a otro en esta vuelta, con su resumen:
    /// `m:<chat>:<clave>` y `f:<chat>:<ruta>`.
    pasados: HashMap<String, String>,
    conocidos: HashMap<String, ArchivoInfo>,
    chat_de_la_vuelta: String,
    terminada: bool,
    /// Quien decide cuando los dos lados no dicen lo mismo. Se pone desde la
    /// pantalla antes de `preparar`; ver [Sesion::quien_manda].
    mando: Mando,
    /// Las capturas de este aparato, si la vuelta lleva galeria. Ver
    /// [`Sesion::con_galeria`].
    capturas: Option<&'a dyn CapturasDelAparato>,
    /// **Con una persona de un chat compartido** y no con un aparato del
    /// grupo (ver [Sesion::conectar_con_persona]): solo se trabaja en los
    /// chats que compartimos y nada de los demas sale ni se pisa aqui,
    /// aunque lo que llega lo nombre. Nulo con el grupo.
    acceso: Option<AccesoDePersona<'a, D>>,
}

impl<D: Disco + ?Sized, F: Read + Write> Drop for Sesion<'_, D, F> {
    fn drop(&mut self) {
        if !self.terminada {
            self.terminada = true;
            soltar(self.disco);
        }
    }
}

impl<'a, D: Disco + ?Sized, F: Read + Write> Sesion<'a, D, F> {
    /// Se presenta al otro aparato. Con `unirme`, este aun no esta en el
    /// grupo: llega con el `codigo` tecleado y sale con su letra.
    pub fn conectar(
        flujo: F,
        disco: &'a D,
        unirme: bool,
        codigo: Option<&str>,
        ahora: impl Fn() -> i64 + 'a,
        mi_puerto: u32,
        nonce: [u8; 32],
    ) -> Resultado<Sesion<'a, D, F>> {
        let id = disco.identidad()?;
        let el_codigo = codigo
            .map(str::to_string)
            .or(id.codigo.clone())
            .ok_or_else(|| fallo("Este aparato no está en un grupo"))?;
        if !ocupar(disco) {
            return Err(ErrorSincro::Remoto(OCUPADO.into()));
        }
        let ahora: Box<dyn Fn() -> i64 + 'a> = Box::new(ahora);
        let abierta = (|| -> Resultado<Sesion<'a, D, F>> {
            let clave = crate::codigo::clave_de_grupo(&el_codigo);
            let mut canal = Canal::saludar(flujo, &clave, nonce, true)?;
            let antes = ahora();
            enviar(
                &mut canal,
                &Peticion {
                    t: "hola".into(),
                    hola: Some(Hola {
                        yo: id.yo.clone(),
                        miembros: id.miembros.clone(),
                        reloj: antes,
                        unirme,
                        puerto: mi_puerto,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )?;
            let r = leer_respuesta(&mut canal)?;
            let despues = ahora();
            let hola = r.hola.ok_or_else(|| fallo("El otro aparato no saludó"))?;
            if hola.version != crate::VERSION {
                return Err(fallo(
                    "El otro aparato tiene otra versión de PixPin: actualiza los dos",
                ));
            }
            let desfase = hola.reloj - (antes + despues) / 2;
            if unirme {
                let yo = hola
                    .miembros
                    .iter()
                    .find(|x| x.id == id.yo.id)
                    .cloned()
                    .ok_or_else(|| fallo("El otro aparato no me dio letra"))?;
                disco.guardar_identidad(&Identidad {
                    yo,
                    codigo: Some(el_codigo.clone()),
                    miembros: hola.miembros.clone(),
                })?;
                disco.sellar()?;
            } else {
                let juntos = grupo::juntar(&miembros_o_yo(&id), &hola.miembros, Some(&hola.yo));
                disco.guardar_identidad(&Identidad {
                    miembros: juntos,
                    ..id.clone()
                })?;
            }
            disco.avisar(disco::Cambio::Identidad);
            Ok(Sesion {
                canal,
                disco,
                ahora: Box::new(|| 0),
                otro: hola.yo,
                desfase,
                puerto_del_otro: hola.puerto,
                pasados: HashMap::new(),
                conocidos: HashMap::new(),
                chat_de_la_vuelta: String::new(),
                terminada: false,
                mando: Mando::default(),
                capturas: None,
                acceso: None,
            })
        })();
        match abierta {
            Ok(mut s) => {
                s.ahora = ahora;
                Ok(s)
            }
            Err(e) => {
                soltar(disco);
                Err(e)
            }
        }
    }

    /// **Se presenta a una persona de un chat compartido**
    /// (`Sesion.conectarConPersona`, 13-oct-2026), con las claves de este
    /// aparato y no con el codigo del grupo. Solo se habla con alguien que
    /// esta en la lista de un chat donde tambien estoy yo; con `a_quien`,
    /// ademas, tiene que ser ese aparato. La sesion queda limitada a lo que
    /// compartimos ([Sesion::acceso]).
    pub fn conectar_con_persona(
        flujo: F,
        disco: &'a D,
        a_quien: Option<&str>,
        ahora: impl Fn() -> i64 + 'a,
        mi_puerto: u32,
        nonce: [u8; 32],
    ) -> Resultado<Sesion<'a, D, F>> {
        if !ocupar(disco) {
            return Err(ErrorSincro::Remoto(OCUPADO.into()));
        }
        let ahora: Box<dyn Fn() -> i64 + 'a> = Box::new(ahora);
        let abierta = (|| -> Resultado<Sesion<'a, D, F>> {
            let cd = CompartidosEnDisco::nuevo(&disco.raiz());
            let yo = cd.yo_de(disco)?;
            let lista = cd.leer();
            let mut quien: Option<compartidos::Persona> = None;
            let (mut canal, _) = Canal::entre_personas(flujo, &yo.llaves, nonce, true, |clave| {
                quien = compartidos::por_clave(&lista, &yo.id, &compartidos::b64(clave))
                    .filter(|p| a_quien.is_none_or(|q| q == p.id));
                quien.is_some()
            })?;
            let persona = quien.ok_or(ErrorSincro::Canal(ErrorCanal::NoEsMiembro))?;
            let antes = ahora();
            enviar(
                &mut canal,
                &Peticion {
                    t: "hola".into(),
                    hola: Some(Hola {
                        yo: Aparato {
                            id: yo.id.clone(),
                            nombre: yo.nombre.clone(),
                            ..Default::default()
                        },
                        reloj: antes,
                        puerto: mi_puerto,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )?;
            let r = leer_respuesta(&mut canal)?;
            let despues = ahora();
            let hola = r
                .hola
                .ok_or_else(|| fallo(format!("{} no saludó", persona.nombre)))?;
            if hola.version != crate::VERSION {
                return Err(fallo(format!(
                    "{} tiene otra versión de PixPin: actualizad los dos",
                    persona.nombre
                )));
            }
            if hola.yo.id != persona.id {
                return Err(fallo("No es quien dice ser"));
            }
            Ok(Sesion {
                canal,
                disco,
                ahora: Box::new(|| 0),
                otro: Aparato {
                    id: persona.id.clone(),
                    nombre: persona.nombre.clone(),
                    ..Default::default()
                },
                desfase: hola.reloj - (antes + despues) / 2,
                puerto_del_otro: hola.puerto,
                pasados: HashMap::new(),
                conocidos: HashMap::new(),
                chat_de_la_vuelta: String::new(),
                terminada: false,
                mando: Mando::default(),
                capturas: None,
                acceso: Some(AccesoDePersona::nuevo(disco, &yo.id, &persona.id)),
            })
        })();
        match abierta {
            Ok(mut s) => {
                s.ahora = ahora;
                Ok(s)
            }
            Err(e) => {
                soltar(disco);
                Err(e)
            }
        }
    }

    /// Con una persona, lo que se puede tocar; con el grupo, nada.
    pub fn acceso(&self) -> Option<&AccesoDePersona<'a, D>> {
        self.acceso.as_ref()
    }

    /// Si `rel` de `chat` es de otro chat (solo con una persona).
    fn vedado(&self, chat: &str, rel: &str) -> bool {
        self.acceso.as_ref().is_some_and(|a| a.vedado(chat, rel))
    }

    /// **Las listas de los chats compartidos con otro aparato del grupo**
    /// (`Sesion.compartidosDelGrupo`): se juntan enteras, y cada uno mete al
    /// otro en los chats compartidos donde esta el (son la misma persona).
    /// Dos idas: en la primera el otro me mete a mi; en la segunda le llega
    /// lo que yo hice con el. Devuelve `false` si el otro no sabe de esto (un
    /// PixPin de antes): no pasa nada.
    pub fn compartidos_del_grupo(&mut self, ahora: i64) -> Resultado<bool> {
        if self.acceso.is_some() {
            return Ok(false);
        }
        let cd = CompartidosEnDisco::nuevo(&self.disco.raiz());
        let yo = cd.yo_de(self.disco)?;
        for _ in 0..2 {
            enviar(
                &mut self.canal,
                &Peticion {
                    compartidos: Some(compartidos::a_texto(&cd.leer())),
                    persona: Some(compartidos::ficha_a_texto(&yo.ficha(0))),
                    ..Peticion::de("compartidos")
                },
            )?;
            let r = leer_respuesta_sin_lanzar(&mut self.canal)?;
            if r.error.is_some() {
                return Ok(false);
            }
            let su_ficha = compartidos::ficha_de_texto(r.persona.as_deref())
                .filter(|f| f.id == self.otro.id);
            let suyos = compartidos::de_texto(r.compartidos.as_deref());
            cd.cambiar(|mios| -> io::Result<_> {
                Ok(compartidos::juntar_del_grupo(
                    &mios,
                    &suyos,
                    &yo,
                    su_ficha.as_ref(),
                    ahora,
                ))
            })?;
        }
        self.disco.avisar(disco::Cambio::Identidad);
        Ok(true)
    }

    /// **Las listas de los chats que compartimos, con una persona**
    /// (`Sesion.juntarMiembros`): se juntan las de los chats donde constamos
    /// los dos. Va antes que los chats: si me quitaron, me entero aqui y ya
    /// no se pide nada mas.
    pub fn juntar_miembros(&mut self) -> Resultado<()> {
        let Some(a) = &self.acceso else {
            return Ok(());
        };
        let yo = a.yo.clone();
        let cd = CompartidosEnDisco::nuevo(&self.disco.raiz());
        let rastro = compartidos::con_rastro(&cd.leer(), &yo, &self.otro.id);
        let mios: Vec<ChatCompartido> = cd
            .leer()
            .into_iter()
            .filter(|c| rastro.contains(&c.chat))
            .collect();
        let r = self.pedir(&Peticion {
            compartidos: Some(compartidos::a_texto(&mios)),
            ..Peticion::de("miembros")
        })?;
        let suyos: Vec<ChatCompartido> = compartidos::de_texto(r.compartidos.as_deref())
            .into_iter()
            .filter(|c| rastro.contains(&c.chat))
            .collect();
        cd.cambiar(|mios| -> io::Result<_> {
            Ok(compartidos::juntar_todos(&mios, &suyos, |_| false))
        })?;
        self.disco.avisar(disco::Cambio::Identidad);
        Ok(())
    }

    /// **Lo mio manda** (`Sesion.loMioManda`, `sincro/Protocolo.kt:404-412`):
    /// sincronizar en una sola direccion, para los casos raros. El usuario
    /// vacio su portatil creyendo que el telefono lo volveria a llenar, y
    /// juntar —que es lo normal— hizo lo contrario: los borrados del portatil
    /// eran lo mas reciente y ganaron. Con [Mando::LoMioManda] aqui no se
    /// borra ni se cambia nada: mis mensajes vivos vuelven al otro aunque
    /// alli esten borrados, lo cambiado en los dos queda como aqui, el
    /// proyecto es el mio y mis archivos pisan a los suyos. Lo que solo tiene
    /// el otro se conserva y se trae.
    ///
    /// Los proyectos borrados (lapidas) se deciden antes, en la vuelta y
    /// preguntando: ver `vuelta::decidir_borrados`.
    pub fn quien_manda(&mut self, mando: Mando) {
        self.mando = mando;
    }

    /// Quien manda ahora mismo. Recien conectada es [Mando::Acordar].
    pub fn mando(&self) -> Mando {
        self.mando
    }

    pub fn enviados(&self) -> u64 {
        self.canal.enviados()
    }

    pub fn recibidos(&self) -> u64 {
        self.canal.recibidos()
    }

    fn pedir(&mut self, p: &Peticion) -> Resultado<Respuesta> {
        enviar(&mut self.canal, p)?;
        leer_respuesta(&mut self.canal)
    }

    fn sobre(&self, t: &str) -> Peticion {
        Peticion::sobre(t, &self.chat_de_la_vuelta)
    }

    /// Los proyectos que el otro tiene borrados.
    pub fn lapidas(&mut self) -> Resultado<Vec<LapidaDeChat>> {
        Ok(self.pedir(&Peticion::de("lapidas"))?.lapidas)
    }

    /// Le dice al otro que borre ese proyecto.
    pub fn borrar_alla(&mut self, chat: &str) -> Resultado<()> {
        self.pedir(&Peticion::sobre("borrarchat", chat))?;
        Ok(())
    }

    pub fn catalogo(&mut self) -> Resultado<Vec<Chat>> {
        Ok(self.pedir(&Peticion::de("catalogo"))?.chats)
    }

    /// Paso 1: que pasa con los mensajes de `chat`.
    pub fn preparar(&mut self, chat: &str) -> Resultado<Preparado> {
        let d = self.disco;
        if let Some(a) = &self.acceso
            && !a.puede(chat)
        {
            return Err(fallo(format!(
                "No es un chat compartido con {}",
                self.otro.nombre
            )));
        }
        // Sin copia no se sincroniza: ver `Disco::hacer_copia`.
        d.hacer_copia(
            chat,
            &format!("Antes de sincronizar con {}", self.otro.nombre),
            (self.ahora)(),
        )?;
        d.sellar()?;
        d.adoptar_documentos()?;
        self.conocidos.clear();
        self.chat_de_la_vuelta = chat.to_string();
        let r = self.pedir(&Peticion::sobre("inventario", chat))?;
        let mia = d.base(&self.otro.id, chat);
        // Lo acordado solo vale si los dos recuerdan lo mismo: si una vuelta
        // se corto a medias uno lo guardo y el otro no, y se hace como la
        // primera vez, que junta todo y nunca pisa nada.
        let base = match mia {
            Some(b) if Some(b.sello()) == r.sello_de_base => b,
            _ => Base::default(),
        };
        let de_aqui = d.apuntes(chat)?;
        let por_sena = |l: Vec<Apunte>| -> HashMap<String, Apunte> {
            l.into_iter().map(|a| (a.sena.clone(), a)).collect()
        };
        let mios = por_sena(diferencia::con_marcas_viejas(&de_aqui, &r.apuntes));
        let suyos = por_sena(diferencia::con_marcas_viejas(&r.apuntes, &de_aqui));
        let lista_mia: Vec<Apunte> = mios.values().cloned().collect();
        let lista_suya: Vec<Apunte> = suyos.values().cloned().collect();
        let pasos = diferencia::plan(&lista_mia, &lista_suya, &base.mensajes.a_btree());
        Ok(Preparado {
            chat: chat.into(),
            pasos,
            base,
            suyo_proyecto: r.proyecto,
            mios,
            suyos,
        })
    }

    fn pedir_mensajes(&mut self, chat: &str, claves: &[String]) -> Resultado<Vec<String>> {
        let mut salida = Vec::new();
        for tanda in claves.chunks(crate::MENSAJES_POR_TANDA) {
            let r = self.pedir(&Peticion {
                senas: tanda.to_vec(),
                ..Peticion::sobre("mensajes", chat)
            })?;
            salida.extend(r.mensajes);
        }
        Ok(salida)
    }

    fn mis_archivos(&mut self, chat: &str) -> Resultado<BTreeMap<String, ArchivoInfo>> {
        let lista = self.disco.archivos(chat, &self.conocidos)?;
        let mut salida = BTreeMap::new();
        for a in lista {
            self.conocidos.insert(a.ruta.clone(), a.clone());
            salida.insert(a.ruta.clone(), a);
        }
        Ok(salida)
    }

    /// Aplica el paso 1: trae, manda, borra y **fusiona** mensajes, y junta
    /// el proyecto.
    pub fn aplicar(&mut self, prep: &Preparado, hecho: &mut Hecho) -> Resultado<()> {
        let d = self.disco;
        let chat = prep.chat.as_str();
        let (mut traer, mut mandar, mut fusionar) = (Vec::new(), Vec::new(), Vec::new());
        for p in &prep.pasos {
            match p {
                Paso::Traer(s) => traer.push(s.clone()),
                Paso::Mandar(s) => mandar.push(s.clone()),
                Paso::Fusionar(s) => fusionar.push(s.clone()),
            }
        }
        let borrado = |m: &HashMap<String, Apunte>, k: &str| m.get(k).map(|a| a.borrado);
        // Lo mio manda: lo cambiado en los dos no se junta, se manda; y lo
        // borrado alli que aqui sigue vivo no se borra aqui, vuelve alli.
        // Ver [Sesion::quien_manda] (`Protocolo.kt:520-524`).
        if self.mando == Mando::LoMioManda {
            mandar.append(&mut fusionar);
            let vuelven: Vec<String> = traer
                .iter()
                .filter(|k| {
                    borrado(&prep.suyos, k) == Some(true) && borrado(&prep.mios, k) == Some(false)
                })
                .cloned()
                .collect();
            traer.retain(|k| !vuelven.contains(k));
            mandar.extend(vuelven);
        }
        let traer_vivos: Vec<String> = traer
            .iter()
            .filter(|k| borrado(&prep.suyos, k) == Some(false))
            .cloned()
            .collect();
        let borrar_aqui: Vec<String> = traer
            .iter()
            .filter(|k| borrado(&prep.suyos, k) == Some(true))
            .cloned()
            .collect();
        let mut llegan = self.pedir_mensajes(chat, &traer_vivos)?;
        // De una persona, solo lo que es de este chat y no nombra nada de los
        // otros (`Disco.soloDeEsteChat`).
        let valla = self.acceso.as_ref().map(AccesoDePersona::valla);
        let vedado = |rel: &str| valla.as_ref().is_some_and(|v| v.vedado(chat, rel));
        if self.acceso.is_some() {
            llegan = crate::valla::solo_de_este_chat(d, chat, &llegan, &vedado);
        }

        // Lo cambiado en los dos, junto campo a campo, y el texto por
        // parrafos.
        let mut juntos: Vec<String> = Vec::new();
        if !fusionar.is_empty() {
            let suyos: Vec<(String, Json)> = self
                .pedir_mensajes(chat, &fusionar)?
                .iter()
                .filter_map(|j| {
                    let crudo = Json::analizar(j).ok()?;
                    let n = kotlin::normalizar_mensaje(&crudo)?;
                    Some((kotlin::unico(&n), crudo))
                })
                .collect();
            let mios = d.mensajes_por_clave(chat)?;
            let mut cuenta = Cuenta::default();
            let criterio = Criterio {
                mio_mas_nuevo: true,
                desfase: self.desfase,
            };
            for clave in &fusionar {
                let Some((_, mio)) = mios.iter().find(|(k, _)| k == clave) else {
                    continue;
                };
                let Some((_, suyo)) = suyos.iter().find(|(k, _)| k == clave) else {
                    continue;
                };
                let acordado = prep.base.mensajes.obtener(clave).or_else(|| {
                    prep.mios
                        .get(clave)
                        .and_then(|a| a.alias.as_deref())
                        .and_then(|al| prep.base.mensajes.obtener(al))
                });
                let base = acordado
                    .and_then(|r| d.objeto(r))
                    .and_then(|t| Json::analizar(&t).ok());
                if let Some(junto) =
                    fusion::json(base.as_ref(), Some(mio), Some(suyo), &criterio, &mut cuenta)
                {
                    juntos.push(junto.a_texto());
                }
            }
            if self.acceso.is_some() {
                juntos = crate::valla::solo_de_este_chat(d, chat, &juntos, &vedado);
            }
            hecho.fusionados += juntos.len();
            hecho.rescatados += cuenta.rescatados as usize;
        }
        let mut poner_aqui = llegan.clone();
        poner_aqui.extend(juntos.iter().cloned());
        match &self.acceso {
            // Lo que se borra por una persona no se lleva nada de otro chat.
            Some(a) => crate::valla::aplicar_mensajes_guardando(
                d,
                chat,
                &poner_aqui,
                &borrar_aqui,
                (self.ahora)(),
                &a.prohibidos(),
            )?,
            None => d.aplicar_mensajes(chat, &poner_aqui, &borrar_aqui, (self.ahora)())?,
        }
        hecho.traidos += llegan.len();
        hecho.borrados += borrar_aqui.len();

        // Lo que mando.
        let mios = d.mensajes_por_clave(chat)?;
        let mut poner: Vec<String> = mandar
            .iter()
            .filter(|k| borrado(&prep.mios, k) == Some(false))
            .filter_map(|k| mios.iter().find(|(x, _)| x == k).map(|(_, m)| m.a_texto()))
            .collect();
        poner.extend(juntos.iter().cloned());
        let borrar_alli: Vec<String> = mandar
            .iter()
            .filter(|k| borrado(&prep.mios, k) == Some(true))
            .cloned()
            .collect();

        // El proyecto, juntado. Para saber que hojas se cambiaron en cada
        // lado hacen falta los resumenes de sus archivos.
        let mio = d.proyecto_portatil(chat)?;
        let mut suyo = prep
            .suyo_proyecto
            .as_deref()
            .and_then(kotlin::proyecto_de_texto);
        if self.acceso.is_some() {
            suyo = crate::valla::proyecto_seguro(d, chat, suyo, &vedado);
        }
        let mut junto = mio.clone().or(suyo.clone());
        // Con `LoMioManda` ni se pregunta por los archivos del otro: el
        // proyecto de aqui queda entero y solo viaja el suyo si aqui no hay
        // ninguno (`Protocolo.kt:563-575`).
        if let (Some(m), Some(s)) = (&mio, &suyo)
            && m != s
            && self.mando == Mando::Acordar
        {
            let alli: HashMap<String, ArchivoInfo> = self
                .pedir(&Peticion::sobre("archivos", chat))?
                .archivos
                .into_iter()
                .map(|a| (a.ruta.clone(), a))
                .collect();
            let aqui: HashMap<String, ArchivoInfo> = self.mis_archivos(chat)?.into_iter().collect();
            let acordado = traducir(&prep.base.archivos, &aqui, &alli);
            let resumenes = |m: &HashMap<String, ArchivoInfo>| -> BTreeMap<String, String> {
                m.iter()
                    .map(|(k, a)| (k.clone(), a.resumen.clone()))
                    .collect()
            };
            let (de_aqui, de_alli) = (resumenes(&aqui), resumenes(&alli));
            let base = prep
                .base
                .proyecto
                .as_deref()
                .and_then(kotlin::proyecto_de_texto);
            junto = crate::mezcla::proyecto_con(
                Some(m),
                Some(s),
                base.as_ref(),
                &crate::mezcla::cambiadas(Some(m), &de_aqui, &acordado),
                &crate::mezcla::cambiadas(Some(s), &de_alli, &acordado),
                self.desfase,
                self.mando,
            )
            .and_then(|j| kotlin::normalizar_proyecto(&j));
        }
        if let Some(j) = &junto
            && Some(j) != mio.as_ref()
        {
            d.guardar_proyecto(j)?;
        }
        let para_alla = d
            .proyecto_portatil(chat)?
            .filter(|p| Some(p) != suyo.as_ref())
            .map(|p| p.a_texto());

        let tandas: Vec<&[String]> = poner.chunks(crate::MENSAJES_POR_TANDA).collect();
        if tandas.len() > 1 {
            for t in &tandas[..tandas.len() - 1] {
                self.pedir(&Peticion {
                    poner: t.to_vec(),
                    ..Peticion::sobre("aplicar", chat)
                })?;
            }
        }
        if !poner.is_empty() || !borrar_alli.is_empty() || para_alla.is_some() {
            self.pedir(&Peticion {
                poner: tandas.last().map(|t| t.to_vec()).unwrap_or_default(),
                borrar: borrar_alli.clone(),
                proyecto: para_alla,
                ..Peticion::sobre("aplicar", chat)
            })?;
        }
        hecho.enviados += poner.len() - juntos.len();
        hecho.borrados += borrar_alli.len();
        let ahora_aqui: HashMap<String, String> = d
            .apuntes(chat)?
            .into_iter()
            .map(|a| (a.sena, a.resumen))
            .collect();
        for clave in traer.iter().chain(&mandar).chain(&fusionar) {
            if let Some(r) = ahora_aqui.get(clave) {
                self.pasados.insert(format!("m:{chat}:{clave}"), r.clone());
            }
        }
        Ok(())
    }

    /// Paso 2: con los chats ya iguales, que archivos hay que mover.
    pub fn preparar_archivos(&mut self, prep: &Preparado) -> Resultado<PreparadoArchivos> {
        let chat = prep.chat.clone();
        self.chat_de_la_vuelta = chat.clone();
        let suyos: HashMap<String, ArchivoInfo> = self
            .pedir(&Peticion::sobre("archivos", &chat))?
            .archivos
            .into_iter()
            .map(|a| (a.ruta.clone(), a))
            .collect();
        let mios: HashMap<String, ArchivoInfo> = self.mis_archivos(&chat)?.into_iter().collect();
        let acordado = traducir(&prep.base.archivos, &mios, &suyos);
        Ok(self.planear_archivos(chat, acordado, mios, suyos))
    }

    fn planear_archivos(
        &self,
        chat: String,
        acordado: BTreeMap<String, String>,
        mios: HashMap<String, ArchivoInfo>,
        suyos: HashMap<String, ArchivoInfo>,
    ) -> PreparadoArchivos {
        let apunte = |a: &ArchivoInfo| Apunte {
            sena: a.ruta.clone(),
            creado: a.tocado,
            tocado: a.tocado,
            resumen: a.resumen.clone(),
            borrado: false,
            alias: None,
        };
        let m: Vec<Apunte> = mios.values().map(apunte).collect();
        let s: Vec<Apunte> = suyos.values().map(apunte).collect();
        let pasos = diferencia::plan(&m, &s, &acordado)
            .into_iter()
            // Con una persona, lo de los demas chats no entra en ningun plan.
            .filter(|p| permitida(p.sena()) && !self.vedado(&chat, p.sena()))
            .collect();
        PreparadoArchivos {
            chat,
            pasos,
            acordado,
            mios,
            suyos,
        }
    }

    /// **Solo este archivo** (`Sesion.soloEsteArchivo`).
    pub fn solo_este_archivo(&mut self, chat: &str, rel: &str) -> Resultado<PreparadoArchivos> {
        self.chat_de_la_vuelta = chat.to_string();
        let suyos: HashMap<String, ArchivoInfo> = self
            .pedir(&Peticion::sobre("archivos", chat))?
            .archivos
            .into_iter()
            .filter(|a| a.ruta == rel)
            .map(|a| (a.ruta.clone(), a))
            .collect();
        let mios: HashMap<String, ArchivoInfo> = self
            .mis_archivos(chat)?
            .into_iter()
            .filter(|(k, _)| k == rel)
            .collect();
        let base = self.disco.base(&self.otro.id, chat).unwrap_or_default();
        let solo: Mapa = base
            .archivos
            .iter()
            .filter(|(k, _)| *k == rel)
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let acordado = traducir(&solo, &mios, &suyos);
        Ok(self.planear_archivos(chat.into(), acordado, mios, suyos))
    }

    pub fn aplicar_archivos(
        &mut self,
        prep: &PreparadoArchivos,
        hecho: &mut Hecho,
        avance: &mut dyn FnMut(u64),
    ) -> Resultado<()> {
        self.chat_de_la_vuelta = prep.chat.clone();
        let mut cuenta = Cuenta::default();
        for p in &prep.pasos {
            let rel = p.sena();
            let texto = es_texto(rel);
            let mio = prep.mios.get(rel);
            let suyo = prep.suyos.get(rel);
            let acordado = prep.acordado.get(rel).map(String::as_str);
            let intento: Resultado<Option<String>> = match p {
                // Lo mio manda: lo que tengo yo, va; solo se trae lo que aqui
                // no existe (`Protocolo.kt:680`).
                _ if self.mando == Mando::LoMioManda && mio.is_some() => {
                    self.mandar_archivo(rel, suyo, acordado, avance, hecho)
                }
                Paso::Fusionar(_) if texto && mio.is_some() && suyo.is_some() => self
                    .fusionar_archivo(
                        rel,
                        mio.unwrap_or(&ArchivoInfo::default()),
                        suyo.unwrap_or(&ArchivoInfo::default()),
                        acordado,
                        &mut cuenta,
                        hecho,
                    ),
                // Una leccion aprendida cambiada en los dos se junta (mejora
                // del PC): se trae la suya, se une con la de aqui y la union
                // va de vuelta, para que los dos queden iguales.
                Paso::Fusionar(_)
                    if rel.ends_with(pixpin_lecciones::leccion::EXTENSION)
                        && mio.is_some()
                        && suyo.is_some() =>
                {
                    self.fusionar_leccion(rel, suyo, acordado, avance, hecho)
                }
                // Un PDF o una foto no se juntan: se queda la version tocada
                // mas tarde (la otra sigue en la copia).
                Paso::Fusionar(_) => {
                    let tm = mio.map_or(0, |a| a.tocado);
                    let ts = suyo.map_or(0, |a| a.tocado);
                    if tm >= ts - self.desfase {
                        self.mandar_archivo(rel, suyo, acordado, avance, hecho)
                    } else {
                        self.traer_archivo(rel, mio, acordado, avance, hecho)
                    }
                }
                Paso::Mandar(_) => self.mandar_archivo(rel, suyo, acordado, avance, hecho),
                Paso::Traer(_) => self.traer_archivo(rel, mio, acordado, avance, hecho),
            };
            let puesto = match intento {
                Ok(x) => x,
                // **Una persona dijo que no** a ese archivo: en su aparato
                // tambien es de otro chat suyo (un adjunto reenviado). Se
                // deja y se sigue; con el grupo, como siempre.
                Err(ErrorSincro::Remoto(_)) if self.acceso.is_some() => None,
                Err(e) => return Err(e),
            };
            let Some(r) = puesto else {
                hecho.saltados.push(prep.nombre_de(rel));
                continue;
            };
            self.conocidos.remove(rel);
            self.pasados.insert(format!("f:{}:{rel}", prep.chat), r);
            hecho.archivos += 1;
        }
        hecho.rescatados += cuenta.rescatados as usize;
        Ok(())
    }

    /// Manda mi version: solo los cambios si el otro tiene lo acordado, o
    /// entera. Devuelve el resumen puesto.
    fn mandar_archivo(
        &mut self,
        rel: &str,
        suyo: Option<&ArchivoInfo>,
        acordado: Option<&str>,
        avance: &mut dyn FnMut(u64),
        hecho: &mut Hecho,
    ) -> Resultado<Option<String>> {
        let d = self.disco;
        let chat = self.chat_de_la_vuelta.clone();
        if es_texto(rel)
            && suyo.is_some()
            && let Some(ac) = acordado
            && let Some(base) = d.texto_base(&chat, rel, ac)
        {
            let actual = d.texto_de(&chat, rel)?;
            let resumen = canonico::resumen(&actual);
            let parche = fusion::diferencia(Some(&json(&base)?), Some(&json(&actual)?))
                .map(|p| p.a_texto())
                .unwrap_or_else(|| "null".into());
            if self.mandar_parche(rel, ac, &parche, &resumen)? {
                hecho.ahorrados += (actual.len() as i64 - parche.len() as i64).max(0);
                return Ok(Some(resumen));
            }
        }
        let sale = Salida::de(d, &chat, rel)?;
        let pon = Peticion {
            ruta: Some(rel.into()),
            bytes: sale.largo,
            ..self.sobre("pon")
        };
        enviar(&mut self.canal, &pon)?;
        let resumen = sale.mandar(&mut self.canal, avance)?;
        if leer_respuesta(&mut self.canal)?.saltado || resumen.is_none() {
            return Ok(None);
        }
        Ok(Some(if es_texto(rel) {
            d.resumen_de_archivo(&chat, rel)?
        } else {
            resumen.unwrap_or_default()
        }))
    }

    fn mandar_parche(
        &mut self,
        rel: &str,
        desde: &str,
        parche: &str,
        resumen: &str,
    ) -> Resultado<bool> {
        let r = self.pedir(&Peticion {
            ruta: Some(rel.into()),
            desde: Some(desde.into()),
            parche: Some(parche.into()),
            resumen: Some(resumen.into()),
            ..self.sobre("parche")
        })?;
        Ok(!r.falta_base)
    }

    /// Me traigo la suya: solo los cambios si tengo lo acordado, o entera.
    fn traer_archivo(
        &mut self,
        rel: &str,
        mio: Option<&ArchivoInfo>,
        acordado: Option<&str>,
        avance: &mut dyn FnMut(u64),
        hecho: &mut Hecho,
    ) -> Resultado<Option<String>> {
        let d = self.disco;
        let chat = self.chat_de_la_vuelta.clone();
        if es_texto(rel)
            && mio.is_some()
            && let Some(ac) = acordado
            && d.texto_base(&chat, rel, ac).is_some()
            && let Some((suyo, resumen)) = self.pedir_cambios(rel, ac)?
            && canonico::resumen(&suyo) == resumen
        {
            d.escribir_texto(&chat, rel, &suyo)?;
            hecho.ahorrados += suyo.len() as i64;
            return Ok(Some(resumen));
        }
        let cabecera = self.pedir(&Peticion {
            ruta: Some(rel.into()),
            ..self.sobre("dame")
        })?;
        let canal = &mut self.canal;
        let entero = d
            .escribir_archivo(&chat, rel, &mut |s| {
                recibir_trozos(canal, cabecera.bytes, s, avance)
                    .map(|r| r.is_some())
                    .map_err(io_de)
            })
            .map_err(ErrorSincro::Io)?;
        Ok(if entero {
            Some(d.resumen_de_archivo(&chat, rel)?)
        } else {
            None
        })
    }

    /// Su version, reconstruida con lo acordado y los cambios que manda.
    fn pedir_cambios(&mut self, rel: &str, desde: &str) -> Resultado<Option<(String, String)>> {
        let Some(base) = self
            .disco
            .texto_base(&self.chat_de_la_vuelta.clone(), rel, desde)
        else {
            return Ok(None);
        };
        let r = self.pedir(&Peticion {
            ruta: Some(rel.into()),
            desde: Some(desde.into()),
            ..self.sobre("damecambios")
        })?;
        let Some(resumen) = r.resumen.filter(|_| !r.falta_base) else {
            return Ok(None);
        };
        let parche = match r.parche.as_deref() {
            Some(p) => Some(json(p)?),
            None => None,
        };
        match fusion::aplicar(Some(&json(&base)?), parche.as_ref()) {
            Ok(Some(j)) => Ok(Some((j.a_texto(), resumen))),
            _ => Ok(None),
        }
    }

    /// **Una leccion cambiada en los dos lados**: la suya a memoria, la union
    /// con la mia (`pixpin_lecciones::fusion::en_choque`) escrita aqui y
    /// mandada alli. Si alguna no se lee como leccion, se hace lo de
    /// cualquier archivo: gana la mas reciente (aqui, la suya: la mia sigue
    /// en la copia).
    fn fusionar_leccion(
        &mut self,
        rel: &str,
        suyo: Option<&ArchivoInfo>,
        acordado: Option<&str>,
        avance: &mut dyn FnMut(u64),
        hecho: &mut Hecho,
    ) -> Resultado<Option<String>> {
        let d = self.disco;
        let chat = self.chat_de_la_vuelta.clone();
        let Some(de_alli) = self.traer_entero(rel)? else {
            return Ok(None);
        };
        let aqui = std::fs::read_to_string(d.ruta(&chat, rel)).unwrap_or_default();
        let junta = pixpin_lecciones::fusion::en_choque(&aqui, &de_alli).unwrap_or(de_alli);
        d.escribir_texto(&chat, rel, &junta)?;
        self.mandar_archivo(rel, suyo, acordado, avance, hecho)
    }

    /// Su version entera, a memoria.
    fn traer_entero(&mut self, rel: &str) -> Resultado<Option<String>> {
        let cabecera = self.pedir(&Peticion {
            ruta: Some(rel.into()),
            ..self.sobre("dame")
        })?;
        let mut bytes = Vec::new();
        if recibir_trozos(&mut self.canal, cabecera.bytes, &mut bytes, &mut |_| {})?.is_none() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
    }

    /// **Un lienzo, tabla o croquis cambiado en los dos: se junta** con lo
    /// acordado y queda igual en los dos.
    fn fusionar_archivo(
        &mut self,
        rel: &str,
        mio: &ArchivoInfo,
        suyo: &ArchivoInfo,
        acordado: Option<&str>,
        cuenta: &mut Cuenta,
        hecho: &mut Hecho,
    ) -> Resultado<Option<String>> {
        let d = self.disco;
        let chat = self.chat_de_la_vuelta.clone();
        let por_cambios = match acordado {
            Some(a) => self.pedir_cambios(rel, a)?.map(|x| x.0),
            None => None,
        };
        let de_alli = match por_cambios {
            Some(t) => t,
            None => match self.traer_entero(rel)? {
                Some(t) => t,
                None => return Ok(None),
            },
        };
        let base = acordado
            .and_then(|a| d.texto_base(&chat, rel, a))
            .and_then(|t| Json::analizar(&t).ok());
        let aqui = d.texto_de(&chat, rel)?;
        let criterio = Criterio {
            mio_mas_nuevo: mio.tocado >= suyo.tocado - self.desfase,
            desfase: self.desfase,
        };
        let alli = json(&de_alli)?;
        let Some(junto) = fusion::json(
            base.as_ref(),
            Some(&json(&aqui)?),
            Some(&alli),
            &criterio,
            cuenta,
        ) else {
            return Ok(None);
        };
        let texto_junto = junto.a_texto();
        let resumen = canonico::resumen(&texto_junto);
        d.escribir_texto(&chat, rel, &texto_junto)?;
        let suyo_ahora = canonico::resumen(&de_alli);
        if resumen != suyo_ahora {
            let parche = fusion::diferencia(Some(&alli), Some(&junto))
                .map(|p| p.a_texto())
                .unwrap_or_else(|| "null".into());
            if !self.mandar_parche(rel, &suyo_ahora, &parche, &resumen)?
                && self
                    .mandar_archivo(rel, None, None, &mut |_| {}, hecho)?
                    .is_none()
            {
                return Ok(None);
            }
        }
        hecho.fusionados += 1;
        Ok(Some(resumen))
    }

    /// **Qué tiene el otro de un solo archivo**, sin mover nada.
    pub fn info_de(&mut self, chat: &str, rel: &str) -> Resultado<Option<ArchivoInfo>> {
        Ok(self
            .pedir(&Peticion::sobre("archivos", chat))?
            .archivos
            .into_iter()
            .find(|a| a.ruta == rel))
    }

    /// **Guarda lo acordado en los dos lados**, mirando otra vez como
    /// quedaron. Solo se apunta lo que de verdad esta igual en los dos; lo
    /// demas se queda con lo acordado antes.
    pub fn cerrar(&mut self, prep: &Preparado, ahora: i64) -> Resultado<()> {
        let d = self.disco;
        let chat = prep.chat.as_str();
        let antes = &prep.base;
        let inv = self.pedir(&Peticion::sobre("inventario", chat))?;
        let suyos: Vec<(String, String)> = self
            .pedir(&Peticion::sobre("archivos", chat))?
            .archivos
            .into_iter()
            .map(|a| (a.ruta, a.resumen))
            .collect();
        self.conocidos.clear();
        let mios: Vec<(String, String)> = self
            .mis_archivos(chat)?
            .into_iter()
            .map(|(k, a)| (k, a.resumen))
            .collect();
        let mis_apuntes = d.apuntes(chat)?;
        let alias: HashSet<String> = mis_apuntes
            .iter()
            .chain(&inv.apuntes)
            .filter_map(|a| a.alias.clone())
            .collect();
        let asociar = |l: &[Apunte]| -> Vec<(String, String)> {
            let mut m: Vec<(String, String)> = Vec::new();
            for a in l {
                match m.iter_mut().find(|(k, _)| *k == a.sena) {
                    Some(x) => x.1 = a.resumen.clone(),
                    None => m.push((a.sena.clone(), a.resumen.clone())),
                }
            }
            m
        };
        let mensajes = self.acordado(
            "m",
            chat,
            &asociar(&mis_apuntes),
            &asociar(&inv.apuntes),
            &antes.mensajes,
            &alias,
        );
        let archivos = self.acordado("f", chat, &mios, &suyos, &antes.archivos, &alias);
        let mi_proyecto = d.proyecto_portatil(chat)?.map(|p| p.a_texto());
        let proyecto = match (&mi_proyecto, &inv.proyecto) {
            (Some(m), Some(s)) if canonico::de(m) == canonico::de(s) => Some(m.clone()),
            _ => antes.proyecto.clone(),
        };
        let base = Base {
            mensajes,
            archivos,
            proyecto,
        };
        self.pedir(&Peticion {
            base: Some(base.clone()),
            ..Peticion::sobre("base", chat)
        })?;
        d.guardar_base(&self.otro.id, chat, &base)?;
        d.guardar_objetos_de_base(chat, &base);
        d.apuntar_vez(&self.otro.id, ahora)?;
        Ok(())
    }

    fn acordado(
        &self,
        tipo: &str,
        chat: &str,
        m: &[(String, String)],
        s: &[(String, String)],
        viejo: &Mapa,
        alias: &HashSet<String>,
    ) -> Mapa {
        let busca = |l: &[(String, String)], k: &str| {
            l.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone())
        };
        let mut claves: Vec<String> = Vec::new();
        let mut vistas = HashSet::new();
        for k in m
            .iter()
            .map(|x| x.0.clone())
            .chain(s.iter().map(|x| x.0.clone()))
            .chain(viejo.claves().map(str::to_string))
        {
            if vistas.insert(k.clone()) {
                claves.push(k);
            }
        }
        let mut salida = Mapa::nuevo();
        for k in claves {
            let a = busca(m, &k);
            let b = busca(s, &k);
            let pasado = self.pasados.get(&format!("{tipo}:{chat}:{k}"));
            if a.is_some() && a == b {
                salida.poner(k, a.unwrap_or_default());
            } else if let Some(p) =
                pasado.filter(|p| a.as_ref() == Some(p) || b.as_ref() == Some(p))
            {
                salida.poner(k, p.clone());
            } else if let Some(v) = viejo.obtener(&k)
                && !(tipo == "m" && alias.contains(&k))
            {
                salida.poner(k, v);
            }
        }
        salida
    }

    /// Se despide y deja libre el aparato. Se puede llamar mas de una vez.
    pub fn adios(&mut self) {
        if self.terminada {
            return;
        }
        self.terminada = true;
        if enviar(&mut self.canal, &Peticion::de("adios")).is_ok() {
            let _ = leer_respuesta(&mut self.canal);
        }
        soltar(self.disco);
    }

    /// Suelta el aparato sin despedirse, cuando la conexion ya se rompio.
    pub fn soltar(&mut self) {
        if self.terminada {
            return;
        }
        self.terminada = true;
        soltar(self.disco);
    }

    /// **Con galeria**: la vuelta pasa tambien la galeria de capturas de este
    /// aparato ([`Sesion::galeria`]), detras de los chats.
    pub fn con_galeria(&mut self, capturas: &'a dyn CapturasDelAparato) {
        self.capturas = Some(capturas);
    }

    /// Si esta vuelta lleva galeria.
    pub fn tiene_galeria(&self) -> bool {
        self.capturas.is_some()
    }

    /// Un `poncaptura` con el nombre que se quiera, para probar que el otro
    /// rechaza lo que no es un nombre sin descuadrar la conversacion.
    #[cfg(test)]
    pub(crate) fn poncaptura_de_prueba(
        &mut self,
        nombre: &str,
        fichero: &std::path::Path,
        _hecho: &mut Hecho,
    ) -> Resultado<Respuesta> {
        let sale = Salida::de_fichero(fichero)?;
        enviar(
            &mut self.canal,
            &Peticion {
                t: galeria::PON.into(),
                ruta: Some(nombre.into()),
                bytes: sale.largo,
                ..Default::default()
            },
        )?;
        sale.mandar(&mut self.canal, &mut |_| {})?;
        leer_respuesta(&mut self.canal)
    }

    /// **La galeria de capturas** (`Sesion.galeria`, ver `galeria`): se junta
    /// lo de los dos, cada uno se queda con las capturas vivas que le faltan
    /// y se tira lo quitado a mano en el otro. Devuelve `false` si el otro no
    /// sabe de galerias (un aparato de antes) o aqui no hay: entonces no se
    /// hace nada y la conversacion sigue sana. `avance` cuenta bytes, como
    /// los archivos de los chats.
    pub fn galeria(
        &mut self,
        hecho: &mut Hecho,
        ahora: i64,
        avance: &mut dyn FnMut(u64),
    ) -> Resultado<bool> {
        let Some(c) = self.capturas else {
            return Ok(false);
        };
        enviar(&mut self.canal, &Peticion::de(galeria::PETICION))?;
        let suya = leer_respuesta_sin_lanzar(&mut self.canal)?;
        // «No sé qué es»: un aparato de antes, o sin galeria.
        let (None, Some(sus_entradas)) = (suya.error, suya.galeria) else {
            return Ok(false);
        };
        let yo = self.disco.identidad()?.yo.id;
        let mia = galeria::poner_al_dia(c, &yo, ahora)?;
        let juntas = galeria::juntar(&mia.entradas, &galeria::entradas_de_texto(&sus_entradas)?);
        let mias = mia.tenia;
        let suyas: BTreeSet<String> = suya.tengo.into_iter().collect();
        // Primero lo acordado, alli y aqui: asi el otro sabe de cada captura
        // que le llega (su fecha, su tipo) y lo quitado a mano se va antes de
        // pasar nada.
        enviar(
            &mut self.canal,
            &Peticion {
                t: galeria::JUNTA.into(),
                parche: Some(galeria::entradas_a_texto(&juntas)),
                ..Default::default()
            },
        )?;
        leer_respuesta(&mut self.canal)?;
        hecho.capturas_tiradas += galeria::aplicar_juntas(c, &juntas, ahora)?;
        for e in galeria::que_traer(&juntas, &mias, &suyas, ahora) {
            enviar(
                &mut self.canal,
                &Peticion {
                    t: galeria::DAME.into(),
                    ruta: Some(e.nombre.clone()),
                    ..Default::default()
                },
            )?;
            let cabecera = leer_respuesta_sin_lanzar(&mut self.canal)?;
            if cabecera.error.is_some() {
                // Ya no la tiene: se quedara para otra vez.
                continue;
            }
            let entera = recibir_captura(&mut self.canal, c, &e, cabecera.bytes, avance)?;
            if entera {
                galeria::apuntar_que_llego(c, &e.nombre)?;
                hecho.capturas += 1;
            } else {
                hecho.saltados.push(e.nombre.clone());
            }
        }
        for e in galeria::que_mandar(&juntas, &mias, &suyas, ahora) {
            let Some(sale) = c.abrir(&e.nombre).and_then(|r| Salida::de_fichero(&r).ok()) else {
                continue;
            };
            enviar(
                &mut self.canal,
                &Peticion {
                    t: galeria::PON.into(),
                    ruta: Some(e.nombre.clone()),
                    bytes: sale.largo,
                    ..Default::default()
                },
            )?;
            let resumen = sale.mandar(&mut self.canal, avance)?;
            if leer_respuesta(&mut self.canal)?.saltado || resumen.is_none() {
                hecho.saltados.push(e.nombre.clone());
            } else {
                hecho.capturas += 1;
            }
        }
        c.avisar();
        Ok(true)
    }

    /// El disco de este lado, para quien lleva la vuelta.
    pub fn disco(&self) -> &'a D {
        self.disco
    }
}

/// `Sesion.traducir`: lo acordado antes del 15-sep-2026 se apunto con el
/// resumen de los bytes tal cual; si lo acordado es el crudo de un lado, se
/// cambia por su canonico.
fn traducir(
    acordado: &Mapa,
    mios: &HashMap<String, ArchivoInfo>,
    suyos: &HashMap<String, ArchivoInfo>,
) -> BTreeMap<String, String> {
    acordado
        .iter()
        .map(|(rel, r)| {
            let v = match (mios.get(rel), suyos.get(rel)) {
                (Some(m), _) if m.crudo == r => m.resumen.clone(),
                (_, Some(s)) if s.crudo == r => s.resumen.clone(),
                _ => r.to_string(),
            };
            (rel.to_string(), v)
        })
        .collect()
}
