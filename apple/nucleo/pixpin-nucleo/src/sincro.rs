//! Sincronizar con otro aparato del grupo (Windows, Android u otro Apple).
//!
//! Es el mismo protocolo y el mismo codigo que Windows (`pixpin_sincro`:
//! `Sesion` dirige, `Respondedor` contesta) sobre el mismo almacen
//! (`pixpin_proyecto::vista::DiscoPc`). Aqui solo se pone el transporte: un
//! `TcpStream` de la biblioteca estandar, que en Mac y en iOS va sobre los
//! sockets BSD del sistema, con IPv4 e IPv6 ([`crate::red`]).
//!
//! # Lo que lleva cada vuelta (como `una_vuelta` y `responder` de Windows)
//!
//! - Los chats, con lo elegido la ultima vez con ese aparato.
//! - **La galeria de capturas** detras de los chats (`galeria`,
//!   `galeriajunta`, `damecaptura`, `poncaptura`): las capturas pasan con su
//!   fecha y se van el mismo dia en todos ([`crate::galeria_sincro`]).
//! - «Lo mio manda» si esta encendido en `pixpinmax.toml`.
//! - **Los proyectos borrados** (lapidas): como Windows, sin preguntar
//!   («con todos», la automatica) no se borra ningun proyecto, se avisa; y
//!   **preguntando** ([`Nucleo::sincronizar_preguntando`], «¿Qué
//!   sincronizar?») lo borrado en el otro se borra aqui si se deja marcado
//!   y lo borrado aqui ([`Nucleo::borrar_proyecto`]) se borra alli.
//! - **Las tareas al instante** ([`Nucleo::sincronizar_tareas`]): solo sus
//!   chats, sin galeria (`vuelta::solo_estos`).
//!
//! Y la [`Escucha`] contesta, como `responder` de Windows: la sonda, quien se
//! une al grupo, una vuelta (con galeria), **un archivo suelto** al chat o
//! lienzo que este delante ([`crate::envio`]) y **una persona de un chat
//! compartido** (`PXC1`, [`crate::compartidos`]).
//!
//! # Varias a la vez
//!
//! Swift deja **unos pocos hilos** (4) esperando en [`Escucha::atender`]
//! sobre la misma `Escucha`: `accept` es seguro desde varios hilos y cada uno
//! atiende la conexion que le toca. Asi una sonda no espera a que acabe una
//! vuelta. Lo que escribe en los chats sigue siendo de uno en uno:
//! `pixpin_sincro::protocolo::ocupar` deja una sola vuelta por carpeta de
//! datos (la segunda oye «ocupado», como en Windows) y cada cuaderno se
//! escribe con el cerrojo de `pixpin_proyecto::cuaderno`.
//!
//! # Descubrir a los demas
//!
//! Lo hace Swift (Bonjour `_pixpin._tcp`, TXT `g`/`id`/`l`/`n`): busca con
//! `NWBrowser` y anuncia con `DNSServiceRegister` sobre el puerto de la
//! [`Escucha`]. Aqui se recibe ya el `host` y el `puerto`.
//!
//! Todo es **bloqueante y sin hilos propios**: Swift lo llama desde una
//! `Task` de prioridad `.utility` o desde sus hilos de escucha.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use pixpin_proyecto::vista::DiscoPc;
use pixpin_sincro::disco::{Disco, Identidad};
use pixpin_sincro::mensajes::Aparato;
use pixpin_sincro::protocolo::{Hecho, Respondedor, Sesion};
use pixpin_sincro::{codigo, grupo, vuelta};
use pixpin_store::ajustes;

use crate::envio::{ArchivoLlegado, RecibeAlFrente};
use crate::galeria_sincro::CapturasDelMac;
use crate::red::{conectar, nonce};
use crate::{ErrorNucleo, Nucleo};

/// Lo que salio de una vuelta, para ensenarlo.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ResumenSincro {
    /// El otro aparato.
    pub otro_id: String,
    pub otro_nombre: String,
    /// Mensajes que llegaron y que se mandaron.
    pub traidos: u64,
    pub enviados: u64,
    pub archivos: u64,
    /// Capturas de la galeria que pasaron de un aparato a otro, y las que
    /// se quitaron aqui porque se quitaron a mano en el otro.
    pub capturas: u64,
    pub capturas_tiradas: u64,
    /// Mensajes borrados aqui o alli por la vuelta.
    pub borrados: u64,
    /// **Lo que cambio en los dos lados y se junto** (`Hecho::fusionados`).
    /// Una vuelta que solo junta no trae nada nuevo, pero cambia lo que se
    /// ve: tambien cuenta para releer la lista ([`ResumenSincro::trajo`]).
    pub fusionados: u64,
    /// Bytes que pasaron por la red.
    pub bytes: u64,
    pub segundos: f64,
    /// La frase de lo hecho, como la ensena Windows.
    pub texto: String,
    /// Los avisos del final: lo rescatado y lo ahorrado
    /// (`avisos_de_lo_hecho`), lo que quedo sin decidir de los proyectos
    /// borrados, el reloj desfasado...
    pub avisos: Vec<String>,
    /// **Si hay que releer la lista**: llego algo, se junto algo o se borro
    /// algo (`trajo` de Windows: `traidos > 0 || fusionados > 0`, mas lo
    /// que el Mac ya miraba: los archivos y lo borrado).
    pub trajo: bool,
    /// Se cancelo en «¿Que sincronizar?»: no se hizo nada.
    pub cancelada: bool,
}

/// Una fila de «¿Que sincronizar?» (`vuelta::Fila`), ya con lo que se lee
/// debajo del nombre en cada pestana.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FilaElegir {
    /// El id del chat como viaja (no la ficha de este aparato).
    pub id: String,
    pub nombre: String,
    /// Si esta en este aparato y si esta en el otro.
    pub aqui: bool,
    pub alli: bool,
    /// Lo que se lee en la pestana de este aparato («Más reciente»,
    /// «Solo aquí · se copiará a…») y si va en verde.
    pub estado_aqui: String,
    pub reciente_aqui: bool,
    /// Y en la del otro.
    pub estado_alli: String,
    pub reciente_alli: bool,
    /// Como empieza la casilla: lo elegido la ultima vez con ese aparato.
    pub marcado: bool,
    /// **Borrado en el otro** y aqui sin tocar desde entonces: si se deja
    /// marcado, la vuelta lo borra AQUI (con copia, a la papelera). Empieza
    /// sin marcar.
    pub se_borra_aqui: bool,
}

/// Lo que se le pregunta al usuario antes de una vuelta (`vuelta::Pregunta`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PreguntaSincro {
    /// El nombre del otro aparato y el de este (las dos pestanas).
    pub otro: String,
    pub yo: String,
    pub filas: Vec<FilaElegir>,
    /// Los nombres de lo que se borrara en el otro porque se borro aqui.
    pub se_borran_alli: Vec<String>,
    /// Como empieza el interruptor «Juntar / Lo mío manda».
    pub lo_mio_manda: bool,
}

/// Lo que contesta el usuario (`vuelta::Eleccion`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct EleccionSincro {
    /// Los ids de [`FilaElegir::id`] marcados.
    pub elegidos: Vec<String>,
    pub lo_mio_manda: bool,
}

/// **Quien contesta «¿Que sincronizar?»**: Swift ensena la lista y devuelve
/// lo marcado, o `None` si se cancela. Se llama desde el hilo de la vuelta,
/// que se queda esperando: puede tardar lo que tarde el usuario.
#[uniffi::export(callback_interface)]
pub trait ElegirSincro: Send + Sync {
    fn elegir(&self, pregunta: PreguntaSincro) -> Option<EleccionSincro>;
}

/// Que chats entran en una vuelta (`Alcance` de Windows).
enum Alcance<'a> {
    /// Se pregunta en «¿Que sincronizar?».
    Preguntar(&'a dyn ElegirSincro),
    /// Lo elegido la ultima vez con ese aparato («con todos», la automatica).
    ComoSiempre,
    /// Solo estos chats (ids como viajan): las tareas al instante.
    Solo(std::collections::BTreeSet<String>),
}

/// La pregunta del crate, con los textos de cada pestana, para Swift.
fn pregunta_para_swift(p: &vuelta::Pregunta, otro: &str, yo: &str) -> PreguntaSincro {
    let filas = p
        .filas
        .iter()
        .map(|f| {
            // En la pestana de aqui, «el otro» es el otro aparato; en la
            // suya, este (como `pintar` de `elegir.rs`).
            let (estado_aqui, reciente_aqui) = f.estado(true, otro);
            let (estado_alli, reciente_alli) = f.estado(false, yo);
            FilaElegir {
                id: f.id.clone(),
                nombre: f.nombre.clone(),
                aqui: f.aqui.is_some(),
                alli: f.alli.is_some(),
                estado_aqui,
                reciente_aqui,
                estado_alli,
                reciente_alli,
                marcado: f.marcado,
                se_borra_aqui: f.se_borra_aqui,
            }
        })
        .collect();
    PreguntaSincro {
        otro: otro.to_string(),
        yo: yo.to_string(),
        filas,
        se_borran_alli: p.se_borran_alli.clone(),
        lo_mio_manda: p.lo_mio_manda,
    }
}

/// Si una vuelta cambio lo que se ve aqui: llego algo, se junto algo
/// (`fusionados`: una vuelta que solo junta tambien cambia los chats) o se
/// borro algo.
pub(crate) fn hecho_trajo(h: &Hecho) -> bool {
    h.traidos > 0 || h.fusionados > 0 || h.archivos > 0 || h.borrados > 0
}

/// Un numero uniforme por debajo de `n`, del azar del sistema (como
/// `SecureRandom.nextInt(n)` de Android), sin el sesgo del modulo.
pub(crate) fn azar_menor_que(n: usize) -> Result<usize, ErrorNucleo> {
    let n = n as u32;
    let limite = u32::MAX - u32::MAX % n;
    loop {
        let mut b = [0u8; 4];
        getrandom::getrandom(&mut b).map_err(|e| ErrorNucleo::Sincro {
            detalle: format!("sin azar del sistema: {e}"),
        })?;
        let v = u32::from_le_bytes(b);
        if v < limite {
            return Ok((v % n) as usize);
        }
    }
}

/// Un corte en el saludo es lo que hace quien no descifra: otro codigo.
pub(crate) fn explicar(e: pixpin_sincro::protocolo::ErrorSincro, host: &str) -> ErrorNucleo {
    if e.es_corte() {
        ErrorNucleo::Sincro {
            detalle: format!("{host} no aceptó la conexión: ¿tiene el mismo código de grupo?"),
        }
    } else {
        e.into()
    }
}

/// La etiqueta publica de un grupo: lo que se anuncia en el TXT `g` de
/// Bonjour para que los del mismo grupo se reconozcan **sin publicar el
/// codigo**. Son 60 000 vueltas de PBKDF2 (se recuerdan por codigo): no
/// llamarla en el hilo de la interfaz.
#[uniffi::export]
pub fn etiqueta_de_grupo(codigo: String) -> String {
    codigo::etiqueta(&codigo::clave_de_grupo(&codigo))
}

/// `K7Q2M-9XMPA`: el codigo como se lee y se dicta.
#[uniffi::export]
pub fn codigo_legible(codigo: String) -> String {
    grupo::legible(&codigo::limpiar(&codigo))
}

/// Si lo tecleado es un codigo de grupo valido (con o sin guion, en
/// minusculas o mayusculas).
#[uniffi::export]
pub fn codigo_valido(codigo: String) -> bool {
    grupo::valido(&codigo::limpiar(&codigo))
}

/// El tipo de servicio de Bonjour para sincronizar, sin el punto final:
/// `_pixpin._tcp` (como lo piden `NWBrowser` y `DNSServiceRegister`).
#[uniffi::export]
pub fn tipo_de_servicio() -> String {
    pixpin_sincro::SERVICIO.trim_end_matches('.').to_string()
}

/// El puerto que se intenta primero al escuchar (47474). Si esta tomado se
/// escucha en otro cualquiera ([`Nucleo::escuchar`] con 0) y se anuncia ese.
#[uniffi::export]
pub fn puerto_preferido() -> u16 {
    pixpin_sincro::PUERTO
}

/// Una entrada del TXT de Bonjour, en el orden en que se anuncia.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ParTxt {
    pub clave: String,
    pub valor: String,
}

/// **Lo que este aparato anuncia en la red**, igual que Windows
/// (`anunciarse`) y Android (`Presencia`): la instancia `PixPin <nombre>`
/// del tipo `_pixpin._tcp` con el TXT `g` (etiqueta del grupo), `id`, `l`
/// (letra) y `n` (nombre, 40 letras como mucho). El puerto lo pone quien
/// anuncia: el de su [`Escucha`].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AnuncioBonjour {
    /// `PixPin Mac de Max`, recortado a 63 bytes (una etiqueta DNS) sin
    /// partir una letra.
    pub instancia: String,
    /// `_pixpin._tcp`.
    pub tipo: String,
    /// `g`, `id`, `l`, `n`, en ese orden.
    pub txt: Vec<ParTxt>,
    /// La etiqueta del grupo (el valor de `g`), para comparar con lo que
    /// anuncian los demas.
    pub etiqueta: String,
    /// El id de este aparato (el valor de `id`), para no verse a si mismo.
    pub id: String,
}

/// Una etiqueta DNS mide como mucho 63 bytes (RFC 1035).
const MAXIMO_ETIQUETA: usize = 63;
/// Las letras del nombre que van en el TXT `n` (como Windows).
pub(crate) const LETRAS_DEL_NOMBRE: usize = 40;

/// Recorta a 63 bytes sin partir una letra (`recortar_etiqueta` de Windows).
pub(crate) fn recortar_etiqueta(nombre: &str) -> String {
    if nombre.len() <= MAXIMO_ETIQUETA {
        return nombre.to_string();
    }
    let mut corte = MAXIMO_ETIQUETA;
    while !nombre.is_char_boundary(corte) {
        corte -= 1;
    }
    nombre[..corte].to_string()
}

/// El anuncio de una identidad con la etiqueta de su grupo ya calculada.
/// Logica pura, sin red ni PBKDF2.
fn anuncio_de(id: &str, nombre: &str, letra: Option<&str>, etiqueta: String) -> AnuncioBonjour {
    let corto: String = nombre.chars().take(LETRAS_DEL_NOMBRE).collect();
    let par = |clave: &str, valor: &str| ParTxt {
        clave: clave.into(),
        valor: valor.into(),
    };
    AnuncioBonjour {
        instancia: recortar_etiqueta(format!("PixPin {nombre}").trim()),
        tipo: tipo_de_servicio(),
        txt: vec![
            par("g", &etiqueta),
            par("id", id),
            par("l", letra.unwrap_or_default()),
            par("n", &corto),
        ],
        etiqueta,
        id: id.to_string(),
    }
}

#[uniffi::export]
impl Nucleo {
    /// **El anuncio de Bonjour de este aparato**, o `None` si no esta en
    /// ningun grupo (sin grupo no se anuncia: nadie sabria que hacer con el).
    /// Hay que volver a pedirlo al crear un grupo o unirse a uno, que cambian
    /// la etiqueta y la letra.
    ///
    /// La primera vez por codigo cuesta las 60 000 vueltas de PBKDF2 de la
    /// etiqueta (luego se recuerda): no llamarlo en el hilo de la interfaz.
    pub fn anuncio(&self) -> Result<Option<AnuncioBonjour>, ErrorNucleo> {
        let i = self.identidad()?;
        Ok(i.codigo.map(|codigo| {
            anuncio_de(
                &i.yo.id,
                &i.yo.nombre,
                i.yo.letra.as_deref(),
                etiqueta_de_grupo(codigo),
            )
        }))
    }

    /// La etiqueta del grupo de este aparato (el `g` que anuncian los
    /// suyos), o `None` sin grupo. Cuesta lo mismo que [`Nucleo::anuncio`].
    pub fn etiqueta(&self) -> Result<Option<String>, ErrorNucleo> {
        Ok(self.identidad()?.codigo.map(etiqueta_de_grupo))
    }

    /// **Lo que hay que anunciar para dejarse encontrar**, como la presencia
    /// de Windows: con grupo, [`Nucleo::anuncio`]; **sin grupo pero dentro
    /// de algun chat compartido con personas**, el mismo anuncio sin `g` (y
    /// con la etiqueta vacia): las personas de esos chats llaman aqui y
    /// buscan por el `id`, y sin `g` no sale como aparato propio de nadie.
    /// `None` si no hay por que escuchar.
    pub fn anuncio_de_presencia(&self) -> Result<Option<AnuncioBonjour>, ErrorNucleo> {
        if let Some(a) = self.anuncio()? {
            return Ok(Some(a));
        }
        let i = self.identidad()?;
        let cd = pixpin_sincro::compartidos::CompartidosEnDisco::nuevo(self.ruta());
        if !cd.hay_alguno(&i.yo.id) {
            return Ok(None);
        }
        let mut a = anuncio_de(&i.yo.id, &i.yo.nombre, i.yo.letra.as_deref(), String::new());
        a.txt.retain(|p| p.clave != "g");
        Ok(Some(a))
    }
}

#[uniffi::export]
impl Nucleo {
    /// Crea un grupo nuevo con este aparato dentro (letra `a`) y devuelve su
    /// codigo, para ensenarlo y que los demas se unan. Como `crearGrupo` de
    /// Android.
    pub fn crear_grupo(&self) -> Result<String, ErrorNucleo> {
        let disco = DiscoPc::nuevo(self.ruta());
        let actual = disco.identidad()?;
        if actual.codigo.is_some() {
            return Err(ErrorNucleo::Argumento {
                detalle: "este aparato ya esta en un grupo".into(),
            });
        }
        let mut fallo = None;
        let codigo = grupo::nuevo_codigo(|n| {
            azar_menor_que(n).unwrap_or_else(|e| {
                fallo = Some(e);
                0
            })
        });
        if let Some(e) = fallo {
            return Err(e);
        }
        let yo = Aparato {
            letra: Some("a".into()),
            desde: crate::ahora_ms(),
            ..actual.yo
        };
        disco.guardar_identidad(&Identidad {
            yo: yo.clone(),
            codigo: Some(codigo.clone()),
            miembros: vec![yo],
        })?;
        disco.sellar()?;
        Ok(codigo)
    }

    /// **Se une al grupo** de otro aparato que escucha en `host:puerto`, con
    /// el `codigo` que ensena el. Sale con su letra; los chats se pasan
    /// despues con [`Nucleo::sincronizar_con`], como en Windows.
    pub fn unirse_a_grupo(
        &self,
        host: String,
        puerto: u16,
        codigo: String,
    ) -> Result<crate::InfoAparato, ErrorNucleo> {
        let limpio = codigo::limpiar(&codigo);
        if !grupo::valido(&limpio) {
            return Err(ErrorNucleo::Argumento {
                detalle: "el codigo de grupo son diez signos".into(),
            });
        }
        let flujo = conectar(&host, puerto)?;
        let disco = DiscoPc::nuevo(self.ruta());
        let mut s = Sesion::conectar(
            flujo,
            &disco,
            true,
            Some(&limpio),
            crate::ahora_ms,
            self.mi_puerto(),
            nonce()?,
        )
        .map_err(|e| explicar(e, &host))?;
        s.adios();
        drop(s);
        self.aparato()
    }

    /// **Una vuelta de sincronizar** con el aparato de `host:puerto`, del
    /// mismo grupo: sin preguntar, con lo elegido la ultima vez con el (o
    /// todo la primera), y sin borrar ningun proyecto, como «con todos» de
    /// Windows. Lleva tambien **la galeria de capturas** detras de los chats
    /// y respeta «lo mio manda» de `pixpinmax.toml`. `host` puede ser IPv4,
    /// IPv6 (con su interfaz si es de enlace local: `fe80::1%en0`) o un
    /// nombre. Bloquea hasta terminar.
    pub fn sincronizar_con(&self, host: String, puerto: u16) -> Result<ResumenSincro, ErrorNucleo> {
        self.vuelta_con(&host, puerto, Alcance::ComoSiempre)
    }

    /// **Una vuelta preguntando que sincronizar** (`lanzar_vuelta` de
    /// Windows): antes de juntar nada se le pasa a `elegir` la lista de
    /// chats de los dos, con **lo borrado en el otro** (que solo se borra
    /// aqui si se deja marcado) y lo borrado aqui (que se borrara alli), y
    /// el interruptor «Lo mío manda». Si `elegir` devuelve `None` no se hace
    /// nada y el resumen sale con `cancelada`. Bloquea hasta terminar,
    /// tambien mientras el usuario decide.
    pub fn sincronizar_preguntando(
        &self,
        host: String,
        puerto: u16,
        elegir: Box<dyn ElegirSincro>,
    ) -> Result<ResumenSincro, ErrorNucleo> {
        self.vuelta_con(&host, puerto, Alcance::Preguntar(elegir.as_ref()))
    }

    /// **Las tareas, al instante** (`vuelta::solo_estos`): una vuelta sin
    /// preguntar y sin galeria **solo con los chats de `fichas`** (las
    /// carpetas de `proyectos/`, como [`crate::FichaChat::id`]), de lo
    /// elegido la ultima vez con ese aparato. Es la que sale tras tachar una
    /// casilla; lo demas lo lleva la vuelta entera de detras.
    pub fn sincronizar_tareas(
        &self,
        host: String,
        puerto: u16,
        fichas: Vec<String>,
    ) -> Result<ResumenSincro, ErrorNucleo> {
        let chats: std::collections::BTreeSet<String> = fichas
            .iter()
            .filter_map(|f| pixpin_proyecto::vista::chat_de_ficha(self.ruta(), f))
            .collect();
        if chats.is_empty() {
            return Err(ErrorNucleo::Argumento {
                detalle: "ninguna de esas fichas es un chat que viaje".into(),
            });
        }
        self.vuelta_con(&host, puerto, Alcance::Solo(chats))
    }

    /// **Borra un proyecto** (a la papelera de PixPin, como «Borrar» de la
    /// lista de proyectos de Windows) y **deja su lapida**: la proxima
    /// vuelta lo borra tambien en los demas aparatos (preguntando) en vez de
    /// traerlo entero otra vez. «Mensajes guardados» no se borra. Devuelve
    /// si se quito.
    pub fn borrar_proyecto(&self, ficha: String) -> Result<bool, ErrorNucleo> {
        let (quitados, _sin_mover) = pixpin_proyecto::almacen::borrar_proyectos(
            self.ruta(),
            std::slice::from_ref(&ficha),
            crate::ahora_ms(),
        )?;
        self.vaciar_caches();
        Ok(quitados > 0)
    }

    /// Escucha en `puerto` (0: el que de el sistema; el de siempre es
    /// 47474, [`puerto_preferido`]) para que otro aparato sincronice con
    /// este, se una a su grupo, le mande un archivo suelto o sea una persona
    /// de un chat compartido. Por IPv6 con IPv4 dentro si se puede; si no,
    /// solo IPv4. Si el puerto pedido esta tomado, falla: quien llama vuelve
    /// a probar con 0, como Windows.
    pub fn escuchar(self: Arc<Self>, puerto: u16) -> Result<Arc<Escucha>, ErrorNucleo> {
        let escucha = crate::red::escuchar(puerto)?;
        let puerto = escucha.local_addr()?.port();
        self.puerto_escucha.store(puerto, Ordering::SeqCst);
        Ok(Arc::new(Escucha {
            escucha,
            puerto,
            nucleo: self,
            cerrada: AtomicBool::new(false),
            esperando: AtomicUsize::new(0),
        }))
    }
}

impl Nucleo {
    /// `una_vuelta` de Windows: conecta, saluda, resuelve lo borrado y junta
    /// lo que diga `alcance`.
    fn vuelta_con(
        &self,
        host: &str,
        puerto: u16,
        alcance: Alcance<'_>,
    ) -> Result<ResumenSincro, ErrorNucleo> {
        let inicio = Instant::now();
        let flujo = conectar(host, puerto)?;
        let disco = DiscoPc::nuevo(self.ruta());
        // Las capturas de este aparato, para la galeria que viaja (antes que
        // la sesion: tienen que vivir mas que ella).
        let capturas = CapturasDelMac::nuevas(self.ruta());
        let yo = disco
            .identidad()
            .map(|i| i.yo.nombre)
            .ok()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| "Este Mac".into());
        let mut s = Sesion::conectar(
            flujo,
            &disco,
            false,
            None,
            crate::ahora_ms,
            self.mi_puerto(),
            nonce()?,
        )
        .map_err(|e| explicar(e, host))?;
        s.con_galeria(&capturas);
        // «Lo mio manda»: se lee en cada vuelta, como Windows. Preguntando,
        // es solo como empieza el interruptor; lo que se elija es lo que vale.
        if ajustes::cargar(&self.ubicacion())
            .map(|a| a.sincro.lo_mio_manda)
            .unwrap_or(false)
        {
            s.quien_manda(pixpin_sincro::mezcla::Mando::LoMioManda);
        }
        let otro = s.otro.clone();
        let mut hecho = Hecho::default();
        let r = match &alcance {
            Alcance::Preguntar(quien) => {
                let mut elegir = |o: &Aparato, p: vuelta::Pregunta| {
                    quien
                        .elegir(pregunta_para_swift(&p, &o.nombre, &yo))
                        .map(|e| vuelta::Eleccion {
                            elegidos: e.elegidos.into_iter().collect(),
                            lo_mio_manda: e.lo_mio_manda,
                        })
                };
                vuelta::una(
                    &mut s,
                    Some(&mut elegir),
                    &mut hecho,
                    "",
                    &crate::ahora_ms,
                    &mut |_| {},
                )
            }
            Alcance::ComoSiempre => {
                vuelta::una(&mut s, None, &mut hecho, "", &crate::ahora_ms, &mut |_| {})
            }
            Alcance::Solo(chats) => {
                vuelta::solo_estos(&mut s, chats, &mut hecho, "", &crate::ahora_ms, &mut |_| {})
            }
        };
        let bytes = s.enviados() + s.recibidos();
        if r.is_err() {
            s.soltar();
        }
        drop(s);
        // Lo que llego cambia los cuadernos: los indices de la cache, fuera.
        self.vaciar_caches();
        let v = r?;
        let segundos = inicio.elapsed().as_secs_f64();
        let mut avisos = vuelta::avisos_de_lo_hecho(&hecho);
        avisos.extend(v.avisos);
        Ok(ResumenSincro {
            otro_id: otro.id,
            otro_nombre: otro.nombre,
            traidos: hecho.traidos as u64,
            enviados: hecho.enviados as u64,
            archivos: hecho.archivos as u64,
            capturas: hecho.capturas as u64,
            capturas_tiradas: hecho.capturas_tiradas as u64,
            borrados: hecho.borrados as u64,
            fusionados: hecho.fusionados as u64,
            bytes,
            segundos,
            texto: if v.cancelada {
                String::new()
            } else {
                vuelta::contar_lo_hecho(&hecho, bytes, segundos)
            },
            avisos,
            // Preguntando se pudo borrar o levantar un proyecto entero: la
            // lista se relee siempre.
            trajo: hecho_trajo(&hecho)
                || (matches!(alcance, Alcance::Preguntar(_)) && !v.cancelada),
            cancelada: v.cancelada,
        })
    }

    /// El puerto de la ultima [`Escucha`] abierta, para decirselo al otro en
    /// el saludo (`hola.puerto`), o 0.
    pub(crate) fn mi_puerto(&self) -> u32 {
        u32::from(self.puerto_escucha.load(Ordering::SeqCst))
    }
}

/// **Un puerto escuchando.** Swift deja unos pocos hilos (4) esperando en
/// [`Escucha::atender`] (bloquea sin gastar nada hasta que alguien llama) y
/// cada uno la vuelve a llamar al terminar. Lo publica en Bonjour con
/// [`Escucha::puerto`].
#[derive(uniffi::Object)]
pub struct Escucha {
    escucha: TcpListener,
    puerto: u16,
    nucleo: Arc<Nucleo>,
    cerrada: AtomicBool,
    /// Cuantos hilos esperan ahora en `accept` (para despertarlos a todos
    /// al cerrar).
    esperando: AtomicUsize,
}

/// Cuenta a quien espera mientras vive (tambien si `accept` falla).
struct EnEspera<'a>(&'a AtomicUsize);

impl<'a> EnEspera<'a> {
    fn nueva(n: &'a AtomicUsize) -> Self {
        n.fetch_add(1, Ordering::SeqCst);
        EnEspera(n)
    }
}

impl Drop for EnEspera<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Los hilos que Swift deja esperando en una [`Escucha`]: los que hacen
/// falta para que una sonda o un archivo suelto no esperen a una vuelta, y
/// pocos (cada uno es un hilo dormido en `accept`, que no gasta nada).
#[uniffi::export]
pub fn hilos_de_escucha() -> u32 {
    HILOS_DE_ESCUCHA
}

/// Ver [`hilos_de_escucha`].
pub const HILOS_DE_ESCUCHA: u32 = 4;

#[uniffi::export]
impl Escucha {
    /// El puerto donde escucha de verdad (el que dio el sistema si se pidio 0).
    pub fn puerto(&self) -> u16 {
        self.puerto
    }

    /// Espera a que llegue alguien y lo atiende entero: una sonda (`PING`),
    /// alguien que se une al grupo o una vuelta de sincronizar. Devuelve
    /// `false` si se cerro con [`Escucha::cerrar`] en vez de llegar nadie.
    pub fn atender_una(&self) -> Result<bool, ErrorNucleo> {
        Ok(self.atender()? != Atendido::Cerrada)
    }

    /// Como [`Escucha::atender_una`], pero dice **a quien** se atendio: una
    /// sonda (no cambia nada: no hay que releer los chats), un aparato que
    /// saludo (sincronizo o se unio: puede haber chats nuevos), una persona
    /// de un chat compartido o archivos sueltos que llegaron. Asi Swift no
    /// relee la lista cada vez que la presencia de Windows pregunta.
    ///
    /// Se puede llamar desde varios hilos a la vez sobre la misma
    /// `Escucha`: cada uno atiende una conexion.
    pub fn atender(&self) -> Result<Atendido, ErrorNucleo> {
        if self.cerrada.load(Ordering::SeqCst) {
            return Ok(Atendido::Cerrada);
        }
        let flujo = {
            let _espera = EnEspera::nueva(&self.esperando);
            self.escucha.accept()?.0
        };
        if self.cerrada.load(Ordering::SeqCst) {
            return Ok(Atendido::Cerrada);
        }
        let r = atender(flujo, &self.nucleo, self.puerto);
        if matches!(
            r,
            Ok(Atendido::Aparato { .. } | Atendido::Persona { .. } | Atendido::Archivos { .. })
        ) {
            // Lo que llego cambia los cuadernos y la galeria.
            self.nucleo.vaciar_caches();
        }
        r
    }

    /// Deja de escuchar: **todas** las `atender` que esten esperando vuelven
    /// con `Cerrada` (o `false`). Se despiertan conectandose a si mismo, una
    /// vez por hilo que espera, que es lo unico que saca a un `accept` de su
    /// espera sin sondear.
    pub fn cerrar(&self) {
        self.cerrada.store(true, Ordering::SeqCst);
        for _ in 0..50 {
            let n = self.esperando.load(Ordering::SeqCst);
            if n == 0 {
                break;
            }
            for _ in 0..n {
                crate::red::despertar(self.puerto);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Lo que hizo [`Escucha::atender`].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Atendido {
    /// Se cerro con [`Escucha::cerrar`]: no llego nadie.
    Cerrada,
    /// Una sonda (`PING`): alguien mira si este aparato esta. No cambia nada.
    Sonda,
    /// Un aparato saludo y se hablo con el (una vuelta o una union).
    Aparato { id: String, nombre: String },
    /// Una persona de un chat compartido se junto con este aparato (`PXC1`):
    /// puede haber mensajes nuevos en esos chats.
    Persona { id: String, nombre: String },
    /// Un aparato del grupo mando **archivos sueltos** (y quiza sincronizo
    /// en la misma conexion): donde quedo cada uno.
    Archivos {
        id: String,
        nombre: String,
        llegadas: Vec<ArchivoLlegado>,
    },
}

/// Los cuatro primeros bytes de una conexion, sin sacarlos: dicen a quien
/// se atiende. Quien llama los manda nada mas conectar, pero pueden no
/// llegar juntos: se espera un poco (1 s) a que esten.
fn mirar_cuatro(flujo: &TcpStream) -> Result<(usize, [u8; 4]), ErrorNucleo> {
    let mut cuatro = [0u8; 4];
    let mut n = flujo.peek(&mut cuatro)?;
    let tope = Instant::now() + Duration::from_secs(1);
    while n > 0 && n < 4 && Instant::now() < tope {
        std::thread::sleep(Duration::from_millis(25));
        n = flujo.peek(&mut cuatro)?;
    }
    Ok((n, cuatro))
}

/// Atiende una conexion entrante: `responder` de Windows entero (la sonda,
/// el grupo con su galeria, los archivos sueltos y las personas de los
/// chats compartidos).
fn atender(mut flujo: TcpStream, nucleo: &Nucleo, mi_puerto: u16) -> Result<Atendido, ErrorNucleo> {
    // Corto para los cuatro primeros y la sonda.
    flujo.set_read_timeout(Some(Duration::from_secs(5)))?;
    let (n, cuatro) = mirar_cuatro(&flujo)?;
    if n == 4 && &cuatro == pixpin_sincro::SONDA {
        flujo.read_exact(&mut [0u8; 4])?;
        flujo.write_all(pixpin_sincro::SONDA_RESPUESTA)?;
        return Ok(Atendido::Sonda);
    }
    // Largo, como `Red.escuchar` del movil: mientras el otro decide que
    // sincronizar, aqui no llega nada.
    flujo.set_read_timeout(Some(Duration::from_secs(30 * 60)))?;
    let raiz = nucleo.ruta();
    let disco = DiscoPc::nuevo(raiz);
    let quien: std::cell::RefCell<Option<Aparato>> = std::cell::RefCell::new(None);
    let llegadas: std::cell::RefCell<Vec<ArchivoLlegado>> = std::cell::RefCell::new(Vec::new());
    let yo = disco
        .identidad()
        .map(|i| i.yo.nombre)
        .ok()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Este Mac".into());
    // Un archivo suelto de otro aparato: al chat o lienzo que este delante.
    let fabrica = |_otro: &Aparato| {
        Box::new(RecibeAlFrente::nuevo(nucleo, &yo, &llegadas))
            as Box<dyn pixpin_sincro::al_lienzo::Recibe + '_>
    };
    let al_saludar = |otro: &Aparato, _: u32| *quien.borrow_mut() = Some(otro.clone());
    let estado = |_: &str| {};
    let r = Respondedor {
        disco: &disco,
        estado: &estado,
        ahora: &crate::ahora_ms,
        mi_puerto: u32::from(mi_puerto),
        al_saludar: &al_saludar,
        suelto: Some(&fabrica),
    };
    // **Quien llama dice con los cuatro primeros bytes** si es del grupo
    // (`PXS1`) o una persona de un chat compartido (`PXC1`): a esa solo se
    // le deja tocar los chats que compartimos (la valla).
    let persona = n == 4 && &cuatro == pixpin_sincro::canal::MAGIA_DE_PERSONA;
    if persona {
        r.atender_persona(flujo, nonce()?)?;
    } else {
        // Con la galeria de capturas: si el otro la pide, la tiene.
        let capturas = CapturasDelMac::nuevas(raiz);
        r.atender_con_galeria(flujo, nonce()?, Some(&capturas))?;
    }
    let otro = quien.take().unwrap_or_default();
    let llegadas = llegadas.take();
    Ok(if !llegadas.is_empty() {
        Atendido::Archivos {
            id: otro.id,
            nombre: otro.nombre,
            llegadas,
        }
    } else if persona {
        Atendido::Persona {
            id: otro.id,
            nombre: otro.nombre,
        }
    } else {
        Atendido::Aparato {
            id: otro.id,
            nombre: otro.nombre,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_anuncio_es_el_de_windows() {
        let a = anuncio_de(
            "id-del-mac",
            "Mac de Max",
            Some("c"),
            "0123456789abcdef".into(),
        );
        assert_eq!(a.instancia, "PixPin Mac de Max");
        assert_eq!(a.tipo, "_pixpin._tcp");
        let txt: Vec<_> = a
            .txt
            .iter()
            .map(|p| (p.clave.as_str(), p.valor.as_str()))
            .collect();
        assert_eq!(
            txt,
            [
                ("g", "0123456789abcdef"),
                ("id", "id-del-mac"),
                ("l", "c"),
                ("n", "Mac de Max"),
            ]
        );
        assert_eq!(a.etiqueta, "0123456789abcdef");
        assert_eq!(a.id, "id-del-mac");
    }

    #[test]
    fn sin_letra_va_vacia_y_el_nombre_largo_se_recorta() {
        let largo = "ñ".repeat(50);
        let a = anuncio_de("id-del-mac", &largo, None, "e".into());
        // 63 bytes como mucho y sin partir la ñ (2 bytes): «PixPin » son 7.
        assert!(a.instancia.len() <= MAXIMO_ETIQUETA);
        assert_eq!(a.instancia, format!("PixPin {}", "ñ".repeat(28)));
        let valor = |c: &str| a.txt.iter().find(|p| p.clave == c).unwrap().valor.clone();
        assert_eq!(valor("l"), "");
        assert_eq!(valor("n").chars().count(), LETRAS_DEL_NOMBRE);
    }

    #[test]
    fn tipo_y_puerto_son_los_de_pixpin_sincro() {
        assert_eq!(tipo_de_servicio(), "_pixpin._tcp");
        assert_eq!(puerto_preferido(), 47474);
    }
}
