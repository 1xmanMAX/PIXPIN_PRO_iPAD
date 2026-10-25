//! El almacen del PC visto como la carpeta `files` de PixPin Android.
//!
//! Es el `Disco` de sincronizar para este equipo. El movil guarda todos los
//! chats en un `guardados.jsonl`, los proyectos en `proyectos/proyectos.json`
//! y los lienzos en `pins/draw/`; el PC guarda un chat por proyecto, cada uno
//! en su carpeta. Aqui se traduce entre las dos cosas **sin cambiar como
//! guarda el PC**: el chat del PC sigue leyendo lo mismo de los mismos
//! ficheros, y el movil ve lo que esperaria ver de otro movil. El diseno,
//! con cada regla y su por que, esta en
//! `docs/superpowers/specs/2026-09-19-sincronizar-chats-design.md`.
//!
//! Las reglas, en corto:
//!
//! - **Chat ↔ ficha.** «Mensajes guardados» es la conversacion general del
//!   movil (`general`). Cada otra ficha es el chat de su proyecto, con el id
//!   de `resto["chat"]`, o el de su `proyecto.json` (lo que trajo un
//!   `.pixpin` del movil), o el suyo.
//! - **Rutas.** `pins/draw/<d>.excalidraw.gz` es `lienzos/<d>.excalidraw` de
//!   su carpeta; lo nacido aqui sale como `guardados/pc/<chat>/<ruta>`; lo
//!   demas del movil se guarda en `android/<ruta>` de la carpeta del chat.
//! - **Texto.** Lo que llega se guarda tal cual, con `pixpin:files/` dentro:
//!   asi sale igual que entro y su resumen no cambia. Al salir solo se
//!   reescribe lo nacido aqui con ruta relativa.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use pixpin_sincro::canonico::Json;
use pixpin_sincro::disco::{
    self, Cambio, Disco, GENERAL, Identidad, NOMBRE_GENERAL, PORTATIL, escribir_atomico, limpio,
};
use pixpin_sincro::kotlin;
use pixpin_sincro::mensajes::{Aparato, Chat};

use crate::almacen::{self, Ficha, Indice};

/// La marca de una ficha que ya se sincroniza: `completar_hojas` no le
/// inventa mensajes, que el movil no tiene y tomaria por nuevos.
pub const MARCA_SINCRO: &str = "sincro";
/// Donde se guarda el id de chat de una ficha cuando no es el suyo.
pub const MARCA_CHAT: &str = "chat";
/// **Archivado, en la ficha de un proyecto nacido aqui**, el que no tiene
/// `proyecto.json` donde apuntarlo (la pantalla de Proyectos lo escribe con
/// este nombre). Sale al movil como el `archivado` de su `Proyecto`; en
/// cuanto el proyecto tiene `proyecto.json`, la verdad es ese.
pub const MARCA_ARCHIVADO: &str = "archivado";

/// Si la ficha lleva la marca de archivado.
fn archivado_en_ficha(f: &Ficha) -> bool {
    f.resto.get(MARCA_ARCHIVADO).and_then(|v| v.as_bool()) == Some(true)
}

#[path = "vista/asistente_unico.rs"]
mod asistente_unico;

/// El `Disco` de este equipo.
pub struct DiscoPc {
    raiz: PathBuf,
    avisos: Option<Box<dyn Fn(Cambio) + Send + Sync>>,
    /// El mapa de chats, por la firma del indice (tamano y fecha): cada ruta
    /// de archivo lo necesita, y rehacerlo es leer el indice y el
    /// `proyecto.json` de cada ficha.
    mapa: std::sync::Mutex<Option<(Firma, Mapa)>>,
}

type Firma = (u64, Option<std::time::SystemTime>);
/// Cada chat con su ficha.
type Mapa = Vec<(String, Ficha)>;

impl DiscoPc {
    pub fn nuevo(raiz: &Path) -> DiscoPc {
        DiscoPc {
            raiz: raiz.to_path_buf(),
            avisos: None,
            mapa: std::sync::Mutex::new(None),
        }
    }

    /// Con quien avisar de lo que se escribe (para recargar la pantalla).
    pub fn con_avisos(mut self, f: impl Fn(Cambio) + Send + Sync + 'static) -> DiscoPc {
        self.avisos = Some(Box::new(f));
        self
    }

    /// Cada chat con su ficha, en el orden del indice. El primero que pide un
    /// id de chat se lo queda; uno repetido (el mismo `.pixpin` importado
    /// dos veces) se queda con el id de su ficha.
    pub fn mapa(&self) -> Vec<(String, Ficha)> {
        let firma = std::fs::metadata(almacen::ruta(&self.raiz))
            .map(|m| (m.len(), m.modified().ok()))
            .unwrap_or((0, None));
        if let Ok(g) = self.mapa.lock()
            && let Some((f, m)) = g.as_ref()
            && *f == firma
        {
            return m.clone();
        }
        let m = mapa_de(&self.raiz, &Indice::leer(&self.raiz));
        if let Ok(mut g) = self.mapa.lock() {
            *g = Some((firma, m.clone()));
        }
        m
    }

    /// Despues de escribir el indice o un `proyecto.json`: la firma podria no
    /// cambiar si dos escrituras caen en el mismo instante con el mismo largo.
    fn olvidar_mapa(&self) {
        if let Ok(mut g) = self.mapa.lock() {
            *g = None;
        }
    }

    pub fn ficha_de(&self, chat: &str) -> Option<Ficha> {
        self.mapa()
            .into_iter()
            .find(|(c, _)| c == chat)
            .map(|(_, f)| f)
    }

    fn carpeta_de(&self, chat: &str) -> PathBuf {
        match self.ficha_de(chat) {
            Some(f) => almacen::carpeta(&self.raiz, &f.id),
            // Todavia sin ficha: donde la creara `asegurar_ficha` si puede.
            None => almacen::carpeta(&self.raiz, &limpio(chat)),
        }
    }

    /// La ficha de un chat, creandola si no esta. «Mensajes guardados» se
    /// crea como siempre; otra, con el id del chat si es un nombre de
    /// carpeta valido y libre, para que su carpeta se reconozca a simple
    /// vista.
    fn asegurar_ficha(&self, chat: &str, nombre: &str) -> io::Result<Ficha> {
        if let Some(f) = self.ficha_de(chat) {
            return Ok(f);
        }
        let aparato = self.identidad()?.yo;
        let aparato = crate::codigos::de_aparato(&aparato.id);
        if chat == GENERAL {
            let f = almacen::asegurar_guardados(&self.raiz, ahora(), &aparato);
            self.olvidar_mapa();
            return f;
        }
        let mut indice = Indice::leer(&self.raiz);
        let valido = !chat.is_empty()
            && chat.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && indice.buscar(chat).is_none()
            && !almacen::carpeta(&self.raiz, chat).exists();
        let mut ficha = Ficha {
            id: if valido {
                chat.to_string()
            } else {
                crate::codigos::nuevo()
            },
            nombre: nombre.to_string(),
            ..Default::default()
        };
        if !valido {
            ficha
                .resto
                .insert(MARCA_CHAT.into(), serde_json::Value::String(chat.into()));
        }
        ficha
            .resto
            .insert(MARCA_SINCRO.into(), serde_json::Value::Bool(true));
        indice.proyectos.push(ficha.clone());
        indice.guardar(&self.raiz)?;
        self.olvidar_mapa();
        Ok(ficha)
    }

    /// Las lineas de un chat: las que se entienden como objeto JSON (con el
    /// `proyecto` del chat puesto, que es por lo que se reconocen) y las que
    /// no, tal cual, para no perderlas al reescribir.
    ///
    /// **Un fallo de lectura no es «no hay mensajes»**: lo que se lee aqui se
    /// vuelve a escribir entero unas lineas mas abajo, asi que un error
    /// pasajero —el fichero abierto por otro, el disco ocupado— se convertiria
    /// en borrar la conversacion. Solo un fichero que de verdad NO EXISTE
    /// (un chat recien creado) cuenta como vacio; cualquier otro error sube y
    /// quien iba a escribir se para.
    fn lineas(&self, chat: &str) -> io::Result<(Vec<Json>, Vec<String>)> {
        let texto = match std::fs::read_to_string(self.carpeta_de(chat).join("guardados.jsonl")) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        let mut buenas = Vec::new();
        let mut crudas = Vec::new();
        for l in texto.lines().filter(|l| !l.trim().is_empty()) {
            match Json::analizar(l) {
                Ok(mut j @ Json::Objeto(_)) => {
                    kotlin::poner(&mut j, "proyecto", proyecto_de(chat));
                    buenas.push(j)
                }
                _ => crudas.push(l.to_string()),
            }
        }
        Ok((buenas, crudas))
    }

    fn escribir_lineas(&self, chat: &str, buenas: &[Json], crudas: &[String]) -> io::Result<()> {
        let mut texto = String::new();
        for j in buenas {
            texto.push_str(&j.a_texto());
            texto.push('\n');
        }
        for l in crudas {
            texto.push_str(l);
            texto.push('\n');
        }
        escribir_atomico(
            &self.carpeta_de(chat).join("guardados.jsonl"),
            texto.as_bytes(),
        )
    }

    /// Un mensaje guardado aqui, a como viaja.
    fn portatil_de(&self, chat: &str, m: &Json) -> Option<Json> {
        let mut o = m.clone();
        kotlin::poner(&mut o, "proyecto", proyecto_de(chat));
        // El `Mensaje` del PC puede no llevar clase: una nota, para el movil.
        if kotlin::cadena(&o, "clase").is_none() {
            kotlin::poner(&mut o, "clase", Json::cadena("NOTA"));
        }
        if let Some(r) = kotlin::cadena(&o, "ruta").filter(|r| es_relativa(r)) {
            let v = format!("{PORTATIL}{}", virtual_de(chat, r));
            kotlin::poner(&mut o, "ruta", Json::cadena(v));
        }
        kotlin::normalizar_mensaje(&o)
    }

    /// El proyecto de una ficha tal como lo tiene aqui, antes de hacerlo
    /// portatil.
    fn proyecto_local(&self, chat: &str, f: &Ficha) -> Json {
        let dir = almacen::carpeta(&self.raiz, &f.id);
        let mut p = std::fs::read_to_string(dir.join("proyecto.json"))
            .ok()
            .and_then(|t| Json::analizar(&t).ok())
            .filter(|j| j.como_objeto().is_some())
            .unwrap_or_else(|| {
                // Nacido aqui: se le hace uno con lo que dice la ficha. Con la
                // fecha de creacion de tocado, que no se mueve con cada
                // mensaje: si no, cada nota escrita aqui haria viajar el
                // proyecto entero.
                Json::de_valor(&serde_json::json!({"tocado": f.creado.max(0)}))
            });
        // El documento del proyecto se apunta aqui RELATIVO a su carpeta
        // (`archivos/doc-<t>.pdf`), que sobrevive a mover la carpeta. Al
        // movil le llega como ruta portatil, que es lo que su
        // `pdfOrigen` sabe traducir; y asi `alcance` lo encuentra y el PDF
        // viaja con el proyecto.
        for campo in ["pdfOrigen", "pdfLimpio"] {
            if let Some(r) = kotlin::cadena(&p, campo)
                .filter(|r| es_relativa(r))
                .map(str::to_string)
            {
                let v = format!("{PORTATIL}{}", virtual_de(chat, &r));
                kotlin::poner(&mut p, campo, Json::cadena(v));
            }
        }
        // Archivado en el PC un proyecto sin `proyecto.json`: lo dice la
        // marca de su ficha. Sin esto el movil no se enteraba (12-oct-2026).
        if archivado_en_ficha(f) {
            kotlin::poner(&mut p, "archivado", Json::booleano(true));
        }
        kotlin::poner(&mut p, "id", Json::cadena(chat));
        kotlin::poner(&mut p, "nombre", Json::cadena(f.nombre.clone()));
        if let Some(u) = &f.uid {
            kotlin::poner(&mut p, "uid", Json::cadena(u.clone()));
        }
        if f.creado > 0 {
            kotlin::poner(&mut p, "creado", Json::numero(f.creado));
        }
        if let Some(a) = &f.aparato {
            kotlin::poner(&mut p, "aparato", Json::cadena(a.clone()));
        }
        // El chat del asistente lo dice como el movil (`Proyecto.asistente`):
        // sin esto el otro aparato creaba el suyo (22-oct-2026).
        if f.es_asistente() {
            kotlin::poner(&mut p, "asistente", Json::booleano(true));
        }
        p
    }
}

/// El `proyecto` de un mensaje de ese chat: nulo en la general.
fn proyecto_de(chat: &str) -> Json {
    if chat == GENERAL {
        Json::Nulo
    } else {
        Json::cadena(chat)
    }
}

/// Una ruta relativa de este equipo (`archivos/x.png`), no una del movil ni
/// una absoluta.
fn es_relativa(r: &str) -> bool {
    !r.is_empty()
        && !r.starts_with(PORTATIL)
        && !r.starts_with('/')
        && !r.contains(':')
        && !r.contains('\\')
}

/// La ruta con que viaja un fichero nacido aqui, relativo a su carpeta.
pub fn virtual_de(chat: &str, local: &str) -> String {
    if let Some(d) = local
        .strip_prefix("lienzos/")
        .and_then(|x| x.strip_suffix(".excalidraw"))
        .filter(|d| !d.contains('/'))
    {
        return format!("pins/draw/{d}.excalidraw.gz");
    }
    if let Some(x) = local.strip_prefix("android/") {
        return x.to_string();
    }
    format!("guardados/pc/{}/{local}", limpio(chat))
}

fn mapa_de(raiz: &Path, indice: &Indice) -> Vec<(String, Ficha)> {
    let mut salida: Vec<(String, Ficha)> = Vec::new();
    let mut usados = HashSet::new();
    for f in &indice.proyectos {
        let pedido = if f.es_guardados() {
            GENERAL.to_string()
        } else if let Some(c) = f.resto.get(MARCA_CHAT).and_then(|v| v.as_str()) {
            c.to_string()
        } else {
            std::fs::read_to_string(almacen::carpeta(raiz, &f.id).join("proyecto.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|v| v.get("id")?.as_str().map(str::to_string))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| f.id.clone())
        };
        let chat = if usados.contains(&pedido) {
            f.id.clone()
        } else {
            pedido
        };
        if usados.insert(chat.clone()) {
            salida.push((chat, f.clone()));
        }
    }
    salida
}

/// Los chats que hay ahora en la lista. La papelera lo mira para no
/// recuperar un proyecto que ya volvio por otro lado (llego otra vez del
/// movil): seria tenerlo dos veces.
pub fn chats_vivos(raiz: &Path) -> HashSet<String> {
    mapa_de(raiz, &Indice::leer(raiz))
        .into_iter()
        .map(|(c, _)| c)
        .collect()
}

/// El id de chat de una ficha, para quien borra o marca desde fuera.
pub fn chat_de_ficha(raiz: &Path, ficha_id: &str) -> Option<String> {
    mapa_de(raiz, &Indice::leer(raiz))
        .into_iter()
        .find(|(_, f)| f.id == ficha_id)
        .map(|(c, _)| c)
}

/// Donde esta en este equipo lo que un mensaje o un lienzo de `ficha_id`
/// senala: una ruta relativa a su carpeta, o una `pixpin:files/…` que llego
/// del movil. None si es de otro aparato (absoluta).
pub fn ruta_real(raiz: &Path, ficha_id: &str, ruta: &str) -> Option<PathBuf> {
    if ruta.is_empty() {
        return None;
    }
    if let Some(rel) = ruta.strip_prefix(PORTATIL) {
        if !disco::permitida(rel) {
            return None;
        }
        let chat = chat_de_ficha(raiz, ficha_id).unwrap_or_else(|| ficha_id.to_string());
        return Some(DiscoPc::nuevo(raiz).ruta(&chat, rel));
    }
    if Path::new(ruta).is_absolute() || ruta.starts_with('/') || ruta.contains(':') {
        return None;
    }
    Some(almacen::carpeta(raiz, ficha_id).join(ruta))
}

/// **Las letras de los planos que viajan** (14-oct-2026): la ruta portatil
/// de una letra es `pixpin:files/guardados/fuentes-shx/<nombre>`, senalada
/// por un mensaje ARCHIVO del chat «Asistente» (solo aparatos propios). En
/// `guardados/` porque es donde los dos aparatos dejan viajar un fichero y
/// donde borrar el mensaje borra tambien el fichero (`adjuntosFuera`).
pub const LETRAS_PORTATIL: &str = "guardados/fuentes-shx/";
/// La carpeta de letras del usuario en la raiz del PC (`pixpin_cad::letras::CARPETA`).
pub const CARPETA_DE_LETRAS: &str = "fuentes-shx";

/// La clase de un mensaje con una tabla del movil (`Clase.TABLA`).
pub const CLASE_TABLA: &str = "TABLA";

/// **El fichero de una tabla hecha en el movil** (`Clase.TABLA` con su
/// `referencia`): `tablas/<referencia>.json` alli (`TablasEnDisco.archivo`),
/// que la sincronizacion trae a `android/tablas/` de la carpeta del chat.
/// Es la verdad de la tabla: se lee y se guarda AHI, con el mismo JSON
/// (`TablaDeCalculo`), y viaja como cualquier tabla del movil, celda a
/// celda. `None` si el mensaje no es una tabla del movil.
pub fn tabla_del_movil(
    raiz: &Path,
    ficha_id: &str,
    m: &crate::cuaderno::Mensaje,
) -> Option<PathBuf> {
    if m.clase != Some(crate::cuaderno::Clase::Otra(CLASE_TABLA.into())) {
        return None;
    }
    let referencia = m.referencia.as_deref().filter(|r| !r.trim().is_empty())?;
    let rel = format!("tablas/{}.json", limpio(referencia));
    let chat = chat_de_ficha(raiz, ficha_id).unwrap_or_else(|| ficha_id.to_string());
    Some(DiscoPc::nuevo(raiz).ruta(&chat, &rel))
}

/// **Las hojas de un Excel que el movil editaba antes del 13-oct-2026**:
/// entonces las guardaba solo en `tablas/libro-<mensaje>-<hoja>.json`
/// (`LibroDelChat`), y la sincronizacion las sigue trayendo a
/// `android/tablas/`. Hoy las hojas editadas son mensajes `tabla` del chat
/// con esa referencia (el formato unico); estos ficheros solo se LEEN, para
/// no perder lo que se edito alli, y al guardar en el PC pasan a mensajes
/// (`hoja_pin::Editable::guardar`). No se escriben ni se borran nunca.
/// En orden de hoja, hasta el primero que falte (como el movil).
pub fn tablas_viejas_del_libro(raiz: &Path, ficha_id: &str, libro_id: &str) -> Vec<PathBuf> {
    if libro_id.trim().is_empty() {
        return Vec::new();
    }
    let chat = chat_de_ficha(raiz, ficha_id).unwrap_or_else(|| ficha_id.to_string());
    let disco = DiscoPc::nuevo(raiz);
    (0..10_000)
        .map(|i| {
            disco.ruta(
                &chat,
                &format!("tablas/{}.json", limpio(&format!("libro-{libro_id}-{i}"))),
            )
        })
        .take_while(|r| r.is_file())
        .collect()
}

/// Guarda una tabla del movil en su fichero ([tabla_del_movil]), por un
/// temporal: a medias no queda nada que la sincronizacion pueda mandar.
pub fn guardar_tabla_del_movil(archivo: &Path, texto: &str) -> io::Result<()> {
    escribir_atomico(archivo, texto.as_bytes())
}

/// Al reves que [ruta_real]: la ruta portatil (`pixpin:files/…`) de un
/// fichero de este equipo que esta dentro de la carpeta de algun chat, como
/// la veria el movil. `None` si esta fuera de todas (un PDF abierto desde el
/// Explorador): no hay forma de que el movil lo encuentre.
pub fn portatil_de_ruta(raiz: &Path, fichero: &Path) -> Option<String> {
    let fichero = std::fs::canonicalize(fichero).unwrap_or_else(|_| fichero.to_path_buf());
    for (chat, f) in mapa_de(raiz, &Indice::leer(raiz)) {
        let carpeta = almacen::carpeta(raiz, &f.id);
        let carpeta = std::fs::canonicalize(&carpeta).unwrap_or(carpeta);
        let Ok(rel) = fichero.strip_prefix(&carpeta) else {
            continue;
        };
        let rel: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        let rel = rel.join("/");
        if rel.is_empty() {
            continue;
        }
        let v = virtual_de(&chat, &rel);
        return disco::permitida(&v).then(|| format!("{PORTATIL}{v}"));
    }
    None
}

/// **El PDF del proyecto en este equipo**: el que dice su `pdfOrigen`
/// (`archivos/doc-<t>.pdf` si se unio aqui, `pixpin:files/…` si llego del
/// movil) o, en un proyecto que entro por un `.pixpin`, su `documento.pdf`.
/// `None` si no tiene documento o si su fichero no esta aqui.
pub fn documento_del_proyecto(raiz: &Path, ficha_id: &str) -> Option<PathBuf> {
    let carpeta = almacen::carpeta(raiz, ficha_id);
    let origen = std::fs::read_to_string(carpeta.join("proyecto.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<crate::Proyecto>(&t).ok())
        .and_then(|p| p.pdf_origen);
    if let Some(r) = origen.and_then(|o| ruta_real(raiz, ficha_id, &o))
        && r.is_file()
    {
        return Some(r);
    }
    Some(carpeta.join("documento.pdf")).filter(|r| r.is_file())
}

/// Deja marca de unos mensajes que se quitaron del chat de `ficha_id`: sin
/// ella, la vuelta siguiente los traeria otra vez del otro aparato.
pub fn anotar_borrados(
    raiz: &Path,
    ficha_id: &str,
    quitados: &[crate::cuaderno::Mensaje],
    cuando: i64,
) -> io::Result<()> {
    let Some(chat) = chat_de_ficha(raiz, ficha_id) else {
        return Ok(());
    };
    let d = DiscoPc::nuevo(raiz);
    let portatiles: Vec<Json> = quitados
        .iter()
        .filter_map(|m| serde_json::to_value(m).ok())
        .filter_map(|v| d.portatil_de(&chat, &Json::de_valor(&v)))
        .collect();
    // Lo anotado sobre ellos se va con ellos (`MensajesStore.borrarAdjunto(m)`, v0.96).
    // De una nota, sus comentarios (30-sep).
    for m in portatiles
        .iter()
        .filter(|m| pixpin_sincro::anotado::lleva_anotado(m))
    {
        d.borrar_anotado(&chat, &kotlin::unico(m));
    }
    disco::anotar_marcas_en(&d.sincro(), &disco::marcas_de(&chat, &portatiles, cuando))
}

/// La huella de un cuaderno: cuando se toco y cuanto pesa.
type Huella = (std::time::SystemTime, u64);

/// Los cuadernos que quedaron sellados, con el aparato con que se sellaron y
/// su huella de entonces. Del proceso y no de un `DiscoPc` (cada vuelta hace
/// el suyo).
static SELLADOS: std::sync::Mutex<Vec<(PathBuf, String, Huella)>> =
    std::sync::Mutex::new(Vec::new());

fn huella_de(fichero: &Path) -> Option<Huella> {
    let m = std::fs::metadata(fichero).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// Si `fichero` sigue como quedo al sellarlo con `aparato`.
fn ya_sellado(fichero: &Path, aparato: &str) -> bool {
    let Some(h) = huella_de(fichero) else {
        return false;
    };
    SELLADOS
        .lock()
        .map(|v| {
            v.iter()
                .any(|(f, a, x)| f == fichero && a == aparato && *x == h)
        })
        .unwrap_or(false)
}

/// Apunta que `fichero` queda sellado con `aparato`, con su huella de ahora.
fn apuntar_sellado(fichero: &Path, aparato: &str) {
    let Some(h) = huella_de(fichero) else {
        return;
    };
    if let Ok(mut v) = SELLADOS.lock() {
        v.retain(|(f, _, _)| f != fichero);
        v.push((fichero.to_path_buf(), aparato.to_string(), h));
    }
}

fn ahora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn nombre_del_equipo() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PixPin Max".into())
}

fn al_cable(a: &crate::identidad::Aparato) -> Aparato {
    Aparato {
        id: a.id.clone(),
        nombre: a.nombre.clone(),
        letra: a.letra.clone(),
        desde: a.desde,
    }
}

impl Disco for DiscoPc {
    fn raiz(&self) -> PathBuf {
        self.raiz.clone()
    }

    fn identidad(&self) -> io::Result<Identidad> {
        let i = crate::identidad::Identidad::leer_o_crear(&self.raiz, &nombre_del_equipo())?;
        Ok(Identidad {
            yo: al_cable(&i.yo),
            codigo: i.codigo.clone(),
            miembros: i.miembros.iter().map(al_cable).collect(),
        })
    }

    /// Lo que no conoce el cable (el `resto` de la identidad y de este
    /// aparato) se conserva.
    fn guardar_identidad(&self, i: &Identidad) -> io::Result<()> {
        let actual = crate::identidad::Identidad::leer_o_crear(&self.raiz, &nombre_del_equipo())?;
        let del_cable = |a: &Aparato, resto: serde_json::Map<String, serde_json::Value>| {
            crate::identidad::Aparato {
                id: a.id.clone(),
                nombre: a.nombre.clone(),
                letra: a.letra.clone(),
                desde: a.desde,
                resto,
            }
        };
        crate::identidad::Identidad {
            yo: del_cable(&i.yo, actual.yo.resto.clone()),
            codigo: i.codigo.clone(),
            miembros: i
                .miembros
                .iter()
                .map(|m| {
                    let resto = actual
                        .miembros
                        .iter()
                        .find(|x| x.id == m.id)
                        .map(|x| x.resto.clone())
                        .unwrap_or_default();
                    del_cable(m, resto)
                })
                .collect(),
            resto: actual.resto,
        }
        .guardar(&self.raiz)
    }

    fn avisar(&self, c: Cambio) {
        if let Some(f) = &self.avisos {
            f(c);
        }
    }

    fn chats(&self) -> io::Result<Vec<Chat>> {
        let mapa = self.mapa();
        let resumen = |chat: &str| -> io::Result<(u32, i64)> {
            let (l, _) = self.lineas(chat)?;
            let ultimo = l
                .iter()
                .filter_map(|m| kotlin::numero(m, "cuando"))
                .max()
                .unwrap_or(0)
                .max(0);
            Ok((l.len() as u32, ultimo))
        };
        let (n, ultimo) = if mapa.iter().any(|(c, _)| c == GENERAL) {
            resumen(GENERAL)?
        } else {
            (0, 0)
        };
        let mut v = vec![Chat {
            id: GENERAL.into(),
            nombre: NOMBRE_GENERAL.into(),
            mensajes: n,
            tocado: ultimo,
        }];
        for (chat, f) in mapa.iter().filter(|(c, _)| c != GENERAL) {
            let (n, ultimo) = resumen(chat)?;
            v.push(Chat {
                id: chat.clone(),
                nombre: f.nombre.clone(),
                mensajes: n,
                tocado: f.tocado.max(ultimo),
            });
        }
        Ok(v)
    }

    fn mensajes(&self, chat: &str) -> io::Result<Vec<Json>> {
        if self.ficha_de(chat).is_none() {
            return Ok(Vec::new());
        }
        let (l, _) = self.lineas(chat)?;
        Ok(l.iter().filter_map(|m| self.portatil_de(chat, m)).collect())
    }

    /// `Disco.alcance` y, ademas, la `ruta` EXACTA de cada mensaje y el
    /// documento del proyecto.
    ///
    /// `en_texto` (como `Rutas.enTexto` del movil) corta una ruta en `)`
    /// para no comerse el cierre de un enlace de Markdown, y aqui los
    /// adjuntos guardan su nombre de siempre: «… asesoria (1).md» se quedaba
    /// en «… asesoria (1», que no es un fichero, y el adjunto no viajaba. El
    /// movil recibia el mensaje y, al pulsarlo, «este archivo ya no esta»
    /// (1-oct-2026). El propio `guardar_adjunto` pone « (1)» al repetir
    /// nombre, asi que pasa con cualquier fichero soltado dos veces.
    fn alcance(&self, chat: &str) -> io::Result<Vec<(String, String)>> {
        let mut salida = disco::alcance_de(self, chat)?;
        let mut poner = |rel: &str, etiqueta: String| {
            if disco::permitida(rel)
                && !salida.iter().any(|(r, _)| r == rel)
                && self.ruta(chat, rel).is_file()
            {
                salida.push((rel.to_string(), etiqueta));
            }
        };
        for m in self.mensajes(chat)? {
            if let Some(rel) = kotlin::cadena(&m, "ruta").and_then(|r| r.strip_prefix(PORTATIL)) {
                poner(rel, disco::etiqueta_de(&m));
            }
        }
        if let Some(p) = self.proyecto_portatil(chat)? {
            let nombre = kotlin::cadena(&p, "nombre").unwrap_or_default().to_string();
            for campo in ["pdfOrigen", "pdfLimpio"] {
                if let Some(rel) = kotlin::cadena(&p, campo).and_then(|r| r.strip_prefix(PORTATIL))
                {
                    poner(rel, nombre.clone());
                }
            }
        }
        Ok(salida)
    }

    fn aplicar_mensajes(
        &self,
        chat: &str,
        poner: &[String],
        borrar: &[String],
        ahora: i64,
    ) -> io::Result<()> {
        if poner.is_empty() && borrar.is_empty() {
            return Ok(());
        }
        let quitar: HashSet<String> = borrar.iter().cloned().collect();
        // Lo que llega se guarda tal cual viene: con `pixpin:files/` dentro,
        // que es lo que hace que salga igual y su resumen no cambie.
        let llegan: Vec<Json> = poner
            .iter()
            .filter_map(|t| kotlin::mensaje_de_texto(t))
            .filter(|m| kotlin::chat_de(m) == chat)
            .collect();
        let ficha = self.asegurar_ficha(chat, chat)?;
        // El cerrojo solo mientras se lee y se reescribe: lo de despues avisa
        // a otros, y uno que anadiera un mensaje en este hilo se quedaria
        // esperando para siempre.
        let (salida, quitados) = {
            let _cerrojo = crate::cuaderno::cerrojo();
            let (buenas, crudas) = self.lineas(chat)?;
            let (salida, quitados) = disco::aplicar_en_lista(buenas, chat, &llegan, &quitar);
            self.escribir_lineas(chat, &salida, &crudas)?;
            (salida, quitados)
        };
        self.marcas_tras_aplicar(chat, &llegan, borrar, &quitados, ahora)?;
        // Lo anotado sobre lo que se fue, con el (`Disco.aplicarMensajes`, v0.96).
        for m in quitados
            .iter()
            .filter(|m| pixpin_sincro::anotado::lleva_anotado(m))
        {
            self.borrar_anotado(chat, &kotlin::unico(m));
        }
        // Y su adjunto, como el movil (`adjuntosFuera`) y como borra el chat
        // de aqui: sin esto, lo borrado en el telefono se quedaba en el disco
        // del PC (una leccion seguia en su carpeta).
        self.borrar_adjuntos(chat, &quitados, &salida);
        // La ficha, al dia: la lista del chat ensena lo ultimo.
        let mut indice = Indice::leer(&self.raiz);
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == ficha.id) {
            let ultimo = salida
                .iter()
                .filter_map(|m| {
                    let c = kotlin::numero(m, "cuando")?;
                    let v = serde_json::from_str::<crate::cuaderno::Mensaje>(&m.a_texto()).ok()?;
                    // Una letra o las figuras que llegan no son «lo ultimo».
                    (!crate::cuaderno::es_de_sistema(&v)).then(|| (c, v.resumen()))
                })
                .max_by_key(|(c, _)| *c);
            if let Some((c, r)) = ultimo {
                f.tocado = f.tocado.max(c);
                f.resumen = r;
            }
            f.resto
                .insert(MARCA_SINCRO.into(), serde_json::Value::Bool(true));
            indice.guardar(&self.raiz)?;
            self.olvidar_mapa();
        }
        self.avisar(Cambio::Mensajes);
        Ok(())
    }

    fn sellar(&self) -> io::Result<()> {
        // Antes de mirar que hay: si llego otro chat del asistente, se junta
        // con el de aqui y el que sobra se va como un chat borrado.
        self.unificar_asistentes()?;
        let aparato = crate::codigos::de_aparato(&self.identidad()?.yo.id);
        for (chat, _) in self.mapa() {
            let _cerrojo = crate::cuaderno::cerrojo();
            // **Lo ya sellado no se vuelve a leer** (10-oct-2026): se sella en
            // cada `inventario`, dos veces por chat y por lado en cada vuelta,
            // y recorrer TODOS los cuadernos cada vez era lo que mas tardaba
            // en la vuelta de un solo chat de las tareas al instante. Un
            // cuaderno que no se ha tocado desde que quedo sellado (misma
            // fecha y tamano, mismo aparato) sigue sellado.
            let fichero = self.carpeta_de(&chat).join("guardados.jsonl");
            if ya_sellado(&fichero, &aparato) {
                continue;
            }
            let (mut buenas, crudas) = self.lineas(&chat)?;
            if !disco::sellar_chat(&mut buenas, &aparato).is_empty() {
                self.escribir_lineas(&chat, &buenas, &crudas)?;
            }
            apuntar_sellado(&fichero, &aparato);
        }
        Ok(())
    }

    fn borrar_chat(&self, chat: &str, motivo: &str, cuando: i64, aparato: &str) -> io::Result<()> {
        // La conversacion general no es un proyecto: no se borra entera.
        if chat == GENERAL {
            return Ok(());
        }
        let Some(f) = self.ficha_de(chat) else {
            return self.anotar_lapida(chat, cuando, aparato);
        };
        self.hacer_copia(chat, motivo, cuando)?;
        // Marca de cada mensaje, como `aplicarMensajes`: sin ellas volverian
        // solos en la vuelta siguiente. El cuaderno no se vacia: va entero a
        // la papelera con su carpeta, por si hay que volver.
        let todos: Vec<(String, Json)> = self.mensajes_por_clave(chat)?;
        let claves: Vec<String> = todos.iter().map(|(k, _)| k.clone()).collect();
        let mensajes: Vec<Json> = todos.into_iter().map(|(_, m)| m).collect();
        self.marcas_tras_aplicar(chat, &[], &claves, &mensajes, cuando)?;
        almacen::borrar_proyectos(&self.raiz, std::slice::from_ref(&f.id), cuando)?;
        self.olvidar_mapa();
        // `borrar_proyectos` ya dejo lapida; esta dice ademas quien lo borro.
        self.anotar_lapida(chat, cuando, aparato)?;
        self.avisar(Cambio::Proyectos);
        Ok(())
    }

    fn proyecto_portatil(&self, chat: &str) -> io::Result<Option<Json>> {
        if chat == GENERAL {
            return Ok(None);
        }
        let Some(f) = self.ficha_de(chat) else {
            return Ok(None);
        };
        let mut p = self.proyecto_local(chat, &f);
        disco::sellar_proyecto(&mut p);
        Ok(kotlin::normalizar_proyecto(&p))
    }

    fn guardar_proyecto(&self, p: &Json) -> io::Result<()> {
        let Some(nuevo) = kotlin::normalizar_proyecto(p) else {
            return Ok(());
        };
        let chat = kotlin::cadena(&nuevo, "id").unwrap_or_default().to_string();
        if chat.is_empty() || chat == GENERAL {
            return Ok(());
        }
        let nombre = kotlin::cadena(&nuevo, "nombre")
            .unwrap_or_default()
            .to_string();
        let ficha = self.asegurar_ficha(&chat, &nombre)?;
        let antes = self.proyecto_portatil(&chat)?;
        let puesto = disco::actualizado(antes.as_ref(), &nuevo);
        let dir = almacen::carpeta(&self.raiz, &ficha.id);
        escribir_atomico(&dir.join("proyecto.json"), puesto.a_texto().as_bytes())?;
        // Una hoja que ya no esta se lleva sus comentarios (30-sep).
        for uid in pixpin_sincro::anotado::hojas_quitadas(antes.as_ref(), &puesto) {
            self.borrar_anotado(&chat, &uid);
        }
        self.olvidar_mapa();
        let mut indice = Indice::leer(&self.raiz);
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == ficha.id) {
            f.nombre = nombre;
            f.uid = kotlin::cadena(&puesto, "uid").map(str::to_string);
            f.creado = kotlin::numero(&puesto, "creado").unwrap_or(0);
            f.aparato = kotlin::cadena(&puesto, "aparato").map(str::to_string);
            f.tocado = f.tocado.max(kotlin::numero(&puesto, "tocado").unwrap_or(0));
            f.hojas = puesto
                .como_objeto()
                .and_then(|o| o.obtener("hojas"))
                .and_then(Json::como_lista)
                .map_or(0, |l| l.len() as u32);
            f.resto
                .insert(MARCA_SINCRO.into(), serde_json::Value::Bool(true));
            // Ya tiene `proyecto.json`, que dice si esta archivado: la marca
            // de la ficha diria lo contrario si el movil lo desarchivo.
            f.resto.remove(MARCA_ARCHIVADO);
            indice.guardar(&self.raiz)?;
            self.olvidar_mapa();
        }
        self.avisar(Cambio::Proyectos);
        Ok(())
    }

    fn ruta(&self, chat: &str, rel: &str) -> PathBuf {
        let p = self.carpeta_de(chat);
        if let Some(d) = rel
            .strip_prefix("pins/draw/")
            .and_then(|x| x.strip_suffix(".excalidraw.gz"))
            .filter(|d| !d.contains('/'))
        {
            return p.join("lienzos").join(format!("{d}.excalidraw"));
        }
        if let Some((c, resto)) = rel
            .strip_prefix("guardados/pc/")
            .and_then(|x| x.split_once('/'))
            && let Some((_, f)) = self.mapa().into_iter().find(|(ch, _)| limpio(ch) == c)
        {
            return almacen::carpeta(&self.raiz, &f.id).join(resto);
        }
        // Las letras de los planos del chat «Asistente» son la carpeta de
        // letras del usuario (`<raiz>/fuentes-shx`, el boton «Aa»): asi lo
        // que llega de otro aparato propio se usa al momento, y lo que se
        // borra alli se borra aqui. De otro chat, a su carpeta: inerte.
        if let Some(nombre) = rel
            .strip_prefix(LETRAS_PORTATIL)
            .filter(|n| !n.is_empty() && !n.contains('/'))
            && self.ficha_de(chat).is_some_and(|f| f.es_asistente())
        {
            return self.raiz.join(CARPETA_DE_LETRAS).join(nombre);
        }
        p.join("android").join(rel)
    }

    /// Lo que llega se guarda tal cual: `pixpin:files/` se queda dentro y
    /// quien lo lee aqui lo resuelve con [ruta_real].
    fn a_local(&self, _chat: &str, texto: String) -> String {
        texto
    }

    /// Lo anotado de este chat: la tinta en `lienzos/anot-…excalidraw`
    /// (`pins/draw/anot-….excalidraw.gz`, como cualquier dibujo) y el resto
    /// en `android/pins/draw/`, donde [Disco::ruta] pone lo del movil.
    fn anotado(&self, chat: &str) -> Vec<String> {
        let carpeta = self.carpeta_de(chat);
        let mut salida = Vec::new();
        let nombres = |dir: PathBuf| -> Vec<String> {
            std::fs::read_dir(dir)
                .map(|l| {
                    l.flatten()
                        .filter(|e| e.path().is_file())
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect()
                })
                .unwrap_or_default()
        };
        for n in nombres(carpeta.join("lienzos")) {
            if let Some(d) = n.strip_suffix(".excalidraw") {
                let rel = pixpin_sincro::anotado::rel(d, ".excalidraw.gz");
                if pixpin_sincro::anotado::uid_del_nombre(&format!("{d}.excalidraw.gz")).is_some() {
                    salida.push(rel);
                }
            }
        }
        for n in nombres(
            carpeta
                .join("android")
                .join(pixpin_sincro::anotado::CARPETA),
        ) {
            if !n.ends_with(".excalidraw.gz")
                && pixpin_sincro::anotado::uid_del_nombre(&n).is_some()
            {
                salida.push(format!("{}/{n}", pixpin_sincro::anotado::CARPETA));
            }
        }
        salida.sort();
        salida
    }

    /// Un lienzo nacido aqui lleva sus fotos como `imagenes/<id>`, relativas
    /// a su carpeta: al salir se ponen como las veria el movil. Lo que ya
    /// venia del movil no se toca, ni se reescribe el texto si no hace falta.
    ///
    /// Y sale **en la forma que lee el `Scene` del movil**
    /// ([`crate::para_el_movil`]): un lienzo nacido aqui lleva los puntos
    /// como `[x, y]`, y alli eso tiraba la escena entera. Lo que ya se lee
    /// alli sale tal cual.
    fn a_portatil(&self, chat: &str, rel: &str, texto: String) -> String {
        if !rel.ends_with(".excalidraw.gz") {
            return texto;
        }
        let texto = self.fotos_a_portatil(chat, texto);
        crate::para_el_movil::lienzo_legible(&texto).unwrap_or(texto)
    }
}

impl DiscoPc {
    /// **El fichero adjunto de unos mensajes borrados en el otro aparato**
    /// (`Disco.aplicarMensajes`: `adjuntosFuera`, solo lo de `guardados/`).
    ///
    /// Mas prudente que el movil en dos cosas, porque aqui un mismo fichero
    /// puede tener dos usos: solo los adjuntos de verdad (archivo, imagen,
    /// voz; un lienzo o una pagina son hojas del proyecto, como dice el
    /// borrar del chat), y nunca uno que otro mensaje que se queda, o el
    /// documento del proyecto, siguen senalando.
    fn borrar_adjuntos(&self, chat: &str, quitados: &[Json], quedan: &[Json]) {
        let rel_de = |m: &Json| -> Option<String> {
            let r = kotlin::cadena(m, "ruta")?;
            match r.strip_prefix(PORTATIL) {
                Some(rel) => Some(rel.to_string()),
                None if es_relativa(r) => Some(virtual_de(chat, r)),
                None => None,
            }
        };
        let mut en_uso: HashSet<String> = quedan.iter().filter_map(rel_de).collect();
        if let Ok(Some(p)) = self.proyecto_portatil(chat) {
            for campo in ["pdfOrigen", "pdfLimpio"] {
                if let Some(rel) = kotlin::cadena(&p, campo).and_then(|r| r.strip_prefix(PORTATIL))
                {
                    en_uso.insert(rel.to_string());
                }
            }
        }
        for m in quitados {
            let adjunto = matches!(
                kotlin::cadena(m, "clase"),
                Some("ARCHIVO" | "IMAGEN" | "VOZ")
            );
            let Some(rel) = rel_de(m).filter(|_| adjunto) else {
                continue;
            };
            if !rel.starts_with("guardados/") || !disco::permitida(&rel) || en_uso.contains(&rel) {
                continue;
            }
            // Como el movil (`File.delete()`): si no se deja, se queda; el
            // mensaje ya se fue y nada lo vuelve a mandar.
            let _ = std::fs::remove_file(self.ruta(chat, &rel));
        }
    }

    /// Las fotos de un lienzo nacido aqui (`imagenes/<id>`), como las veria
    /// el movil.
    fn fotos_a_portatil(&self, chat: &str, texto: String) -> String {
        if !texto.contains("\"imagenes/") {
            return texto;
        }
        let Ok(mut j) = Json::analizar(&texto) else {
            return texto;
        };
        let Some(Json::Objeto(files)) = j.como_objeto().and_then(|o| o.obtener("files")).cloned()
        else {
            return texto;
        };
        let mut nuevos = files.clone();
        let mut cambio = false;
        for (k, v) in files.iter() {
            if let Some(ruta) = kotlin::cadena(v, "path").filter(|r| r.starts_with("imagenes/")) {
                let mut v = v.clone();
                let portatil = format!("{PORTATIL}{}", virtual_de(chat, ruta));
                kotlin::poner(&mut v, "path", Json::cadena(portatil));
                nuevos.poner(k, v);
                cambio = true;
            }
        }
        if !cambio {
            return texto;
        }
        kotlin::poner(&mut j, "files", Json::Objeto(nuevos));
        j.a_texto()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_pdf_unido_aqui_viaja_al_movil_con_el_nombre_de_campo_y_la_ruta_que_entiende() {
        let raiz = std::env::temp_dir().join(format!("pixpin-vista-doc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let ficha = Ficha::nueva("Obra", 5, "PC01");
        let mut indice = Indice::default();
        indice.proyectos.push(ficha.clone());
        indice.guardar(&raiz).unwrap();
        let carpeta = almacen::carpeta(&raiz, &ficha.id);
        std::fs::create_dir_all(carpeta.join("archivos")).unwrap();
        std::fs::write(carpeta.join("archivos/doc-7.pdf"), b"%PDF-1.4\n").unwrap();
        std::fs::write(
            carpeta.join("proyecto.json"),
            r#"{"id":"x","nombre":"Obra","hojas":[{"id":"h-7-0","nombre":"","pagina":0,"deMensaje":"m1"}],"pdfOrigen":"archivos/doc-7.pdf"}"#,
        )
        .unwrap();
        let d = DiscoPc::nuevo(&raiz);
        let chat = chat_de_ficha(&raiz, &ficha.id).unwrap();
        let p = d.proyecto_portatil(&chat).unwrap().unwrap();
        let origen = kotlin::cadena(&p, "pdfOrigen").unwrap().to_string();
        assert_eq!(
            origen,
            format!(
                "{PORTATIL}guardados/pc/{}/archivos/doc-7.pdf",
                limpio(&chat)
            )
        );
        // Y el fichero entra en lo que se manda con el chat.
        let alcance = d.alcance(&chat).unwrap();
        assert!(
            alcance
                .iter()
                .any(|(r, _)| r.ends_with("archivos/doc-7.pdf"))
        );
        // La hoja conserva su pagina y su mensaje, con los nombres del movil.
        let texto = p.a_texto();
        assert!(texto.contains("\"pagina\":0") && texto.contains("\"deMensaje\":\"m1\""));
        // Aqui se encuentra por la ruta relativa, y tambien por la portatil.
        assert_eq!(
            documento_del_proyecto(&raiz, &ficha.id),
            Some(carpeta.join("archivos/doc-7.pdf"))
        );
        assert!(ruta_real(&raiz, &ficha.id, &origen).is_some_and(|r| r.is_file()));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn las_fotos_de_una_nota_rica_viajan_con_ella() {
        // La nota rica del chat (25-oct-2026): una NOTA cuyo Markdown nombra
        // sus fotos de `notas/`. Si no viajaran, en el movil la burbuja
        // saldria con los recuadros vacios.
        let raiz = std::env::temp_dir().join(format!("pixpin-vista-nota-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let ficha = Ficha::nueva("Obra", 5, "PC01");
        let mut indice = Indice::default();
        indice.proyectos.push(ficha.clone());
        indice.guardar(&raiz).unwrap();
        let carpeta = almacen::carpeta(&raiz, &ficha.id);
        std::fs::create_dir_all(carpeta.join("notas")).unwrap();
        std::fs::write(carpeta.join("notas/1-a.png"), b"png").unwrap();
        std::fs::write(carpeta.join("notas/2-b.png"), b"png").unwrap();
        let notas = portatil_de_ruta(&raiz, &carpeta.join("notas")).unwrap();
        let chat = chat_de_ficha(&raiz, &ficha.id).unwrap();
        assert_eq!(
            notas,
            format!("{PORTATIL}guardados/pc/{}/notas", limpio(&chat))
        );
        let texto =
            format!("hola\n![]({notas}/1-a.png)\n![b]({notas}/2-b.png)\n![](notas/no-esta.png)");
        let m = crate::cuaderno::Mensaje::nota(
            &texto,
            &crate::cuaderno::Sello {
                cuando: 1_760_000_000_000,
                numero: 1,
                aparato: "PC01".into(),
                proyecto: ficha.id.clone(),
            },
        );
        crate::cuaderno::anadir(&carpeta, &m).unwrap();
        let d = DiscoPc::nuevo(&raiz);
        let alcance = d.alcance(&chat).unwrap();
        for f in ["notas/1-a.png", "notas/2-b.png"] {
            let rel = format!("guardados/pc/{}/{f}", limpio(&chat));
            assert!(
                alcance.iter().any(|(r, _)| *r == rel),
                "{rel} no viaja: {alcance:?}"
            );
            // Y lo que llega del otro aparato cae en el mismo sitio.
            assert_eq!(d.ruta(&chat, &rel), carpeta.join(f));
        }
        // Caso negativo: lo que no se nombra con `pixpin:files/` no viaja.
        assert!(!alcance.iter().any(|(r, _)| r.ends_with("no-esta.png")));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_proyecto_sin_documento_no_se_inventa_uno() {
        // Caso negativo: sin `pdfOrigen` ni `documento.pdf` no hay documento,
        // y una ruta absoluta de otro aparato tampoco cuenta.
        let raiz = std::env::temp_dir().join(format!("pixpin-vista-sin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let carpeta = almacen::carpeta(&raiz, "p1");
        std::fs::create_dir_all(&carpeta).unwrap();
        assert_eq!(documento_del_proyecto(&raiz, "p1"), None);
        std::fs::write(
            carpeta.join("proyecto.json"),
            r#"{"id":"p1","nombre":"","pdfOrigen":"/data/user/0/com.forge.pixpin/files/proyectos/doc-1.pdf"}"#,
        )
        .unwrap();
        assert_eq!(documento_del_proyecto(&raiz, "p1"), None);
        // El de un `.pixpin` viejo si vale.
        std::fs::write(carpeta.join("documento.pdf"), b"%PDF-1.4\n").unwrap();
        assert_eq!(
            documento_del_proyecto(&raiz, "p1"),
            Some(carpeta.join("documento.pdf"))
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_lienzo_nacido_aqui_sale_hacia_el_movil_con_los_puntos_que_su_scene_lee() {
        // El trazo del PC lleva los puntos como `[x, y]`: tal cual, el movil
        // tiraba la escena entera y el lienzo se abria en blanco alli.
        let d = DiscoPc::nuevo(Path::new("C:/no-hace-falta"));
        let del_pc = r#"{"type":"excalidraw","elements":[{"id":"t","type":"freedraw","x":0,"y":0,"width":2,"height":2,"seed":3,"points":[[0,0],[2,2]]}]}"#;
        let sale = d.a_portatil("pr-1", "pins/draw/d1.excalidraw.gz", del_pc.to_string());
        let v: serde_json::Value = serde_json::from_str(&sale).unwrap();
        assert_eq!(
            v["elements"][0]["points"][1],
            serde_json::json!({"x": 2, "y": 2})
        );
        // Lo que no es un lienzo no se toca aunque lo parezca.
        let tabla = r#"{"elements":[{"id":"t","type":"x","points":[[0,0]]}]}"#;
        assert_eq!(
            d.a_portatil("pr-1", "tablas/t.json", tabla.to_string()),
            tabla
        );
    }

    #[test]
    fn un_lienzo_del_movil_vuelve_a_salir_byte_a_byte() {
        // Caso negativo: lo que ya lee el movil sale igual que entro, o su
        // resumen cambiaria y se mandaria en cada vuelta.
        let d = DiscoPc::nuevo(Path::new("C:/no-hace-falta"));
        let del_movil = r#"{"elements":[{"id":"t","type":"freedraw","x":1.10,"y":0,"width":2,"height":2,"seed":3,"points":[{"x":0.0,"y":0.0}]}],"files":{"f":{"id":"f","mimeType":"image/png","path":"pixpin:files/pins/draw/files/f"}}}"#;
        assert_eq!(
            d.a_portatil("pr-1", "pins/draw/d1.excalidraw.gz", del_movil.to_string()),
            del_movil
        );
    }

    #[test]
    fn las_rutas_nacidas_aqui_llevan_su_chat_y_los_lienzos_su_sitio_del_movil() {
        assert_eq!(
            virtual_de("pr-1", "lienzos/d9.excalidraw"),
            "pins/draw/d9.excalidraw.gz"
        );
        assert_eq!(
            virtual_de("pr-1", "archivos/captura.png"),
            "guardados/pc/pr-1/archivos/captura.png"
        );
        assert_eq!(
            virtual_de("general", "android/guardados/x.pdf"),
            "guardados/x.pdf"
        );
        // Y lo que no es relativo de aqui no se toca.
        assert!(!es_relativa("pixpin:files/guardados/x.pdf"));
        assert!(!es_relativa("/storage/emulated/0/x.pdf"));
        assert!(!es_relativa(r"C:\Users\x.pdf"));
        assert!(es_relativa("archivos/x.pdf"));
    }
}

/// **El fichero de un mensaje con la extension de su nombre** (8-oct-2026).
///
/// Algunos ficheros llegan del movil guardados como `.bin` aunque el mensaje
/// diga `informe.pdf` (el usuario: «me dice que no se puede abrir porque es
/// un archivo .bin a pesar de que en su formato diga PDF»). El PC decide por
/// la extension del disco como se abre, asi que un PDF de verdad acababa en
/// el «¿Con que aplicacion?» de Windows.
///
/// Si `fichero` es `.bin` y `nombre` tiene otra extension, se devuelve un
/// enlace (o una copia, si el enlace no se puede) con la extension buena en
/// `<raiz>/cache/con-extension/`. Fuera de la carpeta sincronizada a
/// proposito: un fichero de mas ahi viajaria al movil. En cualquier otro
/// caso, el mismo `fichero`.
pub fn con_la_extension_del_nombre(raiz: &Path, fichero: PathBuf, nombre: &str) -> PathBuf {
    let es_bin = fichero
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("bin"));
    let buena = Path::new(nombre)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .filter(|e| !e.is_empty() && e != "bin" && e.chars().all(|c| c.is_ascii_alphanumeric()));
    let (true, Some(ext), Some(tallo)) = (es_bin, buena, fichero.file_stem()) else {
        return fichero;
    };
    let Ok(meta) = std::fs::metadata(&fichero) else {
        return fichero;
    };
    let carpeta = raiz.join("cache").join("con-extension");
    let destino = carpeta.join(format!("{}.{ext}", tallo.to_string_lossy()));
    if std::fs::metadata(&destino).is_ok_and(|d| d.len() == meta.len()) {
        return destino;
    }
    let _ = std::fs::remove_file(&destino);
    let hecho = std::fs::create_dir_all(&carpeta).and_then(|()| {
        std::fs::hard_link(&fichero, &destino)
            .or_else(|_| std::fs::copy(&fichero, &destino).map(|_| ()))
    });
    // Si no se pudo, el de siempre: peor abrirlo como .bin que no ofrecerlo.
    if hecho.is_ok() { destino } else { fichero }
}

#[cfg(test)]
mod pruebas_con_extension {
    use super::*;

    #[test]
    fn un_pdf_guardado_como_bin_se_abre_como_pdf_y_lo_demas_no_se_toca() {
        let raiz = std::env::temp_dir().join(format!("pp-bin-{}", std::process::id()));
        std::fs::create_dir_all(&raiz).unwrap();
        let bin = raiz.join("123_abc.bin");
        std::fs::write(&bin, b"%PDF-1.3").unwrap();
        let bueno = con_la_extension_del_nombre(&raiz, bin.clone(), "manual.pdf");
        assert_eq!(bueno.extension().unwrap(), "pdf");
        assert_eq!(std::fs::read(&bueno).unwrap(), b"%PDF-1.3");
        // Fuera de la carpeta del chat, en la cache.
        assert!(bueno.starts_with(raiz.join("cache")));
        // Una segunda vez, el mismo.
        assert_eq!(
            con_la_extension_del_nombre(&raiz, bin.clone(), "manual.pdf"),
            bueno
        );
        // Casos negativos: con su extension ya buena, o sin nombre que diga
        // otra cosa, el fichero tal cual.
        let png = raiz.join("foto.png");
        std::fs::write(&png, b"x").unwrap();
        assert_eq!(
            con_la_extension_del_nombre(&raiz, png.clone(), "foto.pdf"),
            png
        );
        assert_eq!(
            con_la_extension_del_nombre(&raiz, bin.clone(), "sin extension"),
            bin
        );
        assert_eq!(
            con_la_extension_del_nombre(&raiz, bin.clone(), "otro.bin"),
            bin
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }
}

#[cfg(test)]
mod pruebas_de_sellar {
    use super::*;

    fn uids(carpeta: &Path) -> Vec<bool> {
        std::fs::read_to_string(carpeta.join("guardados.jsonl"))
            .unwrap()
            .lines()
            .map(|l| l.contains("\"uid\""))
            .collect()
    }

    #[test]
    fn lo_sellado_no_se_relee_pero_lo_escrito_despues_si_se_sella() {
        let raiz = std::env::temp_dir().join(format!("pixpin-vista-sellar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let ficha = Ficha::nueva("Obra", 5, "PC01");
        let mut indice = Indice::default();
        indice.proyectos.push(ficha.clone());
        indice.guardar(&raiz).unwrap();
        let d = DiscoPc::nuevo(&raiz);
        d.guardar_identidad(&pixpin_sincro::disco::Identidad {
            yo: Aparato {
                id: "id-pc".into(),
                nombre: "PC".into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        let carpeta = almacen::carpeta(&raiz, &ficha.id);
        std::fs::create_dir_all(&carpeta).unwrap();
        let linea = |id: &str, cuando: i64| {
            format!(
                "{{\"id\":\"{id}\",\"cuando\":{cuando},\"clase\":\"NOTA\",\"texto\":\"x\",\"numero\":{cuando}}}\n"
            )
        };
        std::fs::write(carpeta.join("guardados.jsonl"), linea("a", 1)).unwrap();
        d.sellar().unwrap();
        assert_eq!(uids(&carpeta), [true]);
        let sellado = std::fs::read(carpeta.join("guardados.jsonl")).unwrap();
        // Sin tocar: la segunda no reescribe nada.
        d.sellar().unwrap();
        assert_eq!(
            std::fs::read(carpeta.join("guardados.jsonl")).unwrap(),
            sellado
        );
        // Caso negativo: lo anadido despues (otro tamano) se sella igual.
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(carpeta.join("guardados.jsonl"))
            .unwrap();
        std::io::Write::write_all(&mut f, linea("b", 2).as_bytes()).unwrap();
        drop(f);
        d.sellar().unwrap();
        assert_eq!(uids(&carpeta), [true, true]);
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
