//! Todo lo que la sincronizacion lee y escribe del disco de un aparato.
//!
//! Puerto de `sincro/Disco.kt` partido en dos: lo que depende de como guarda
//! cada aparato sus chats, su proyecto y sus archivos va en las funciones
//! que cada uno pone (el movil lo tiene todo en `files/`, el PC en una
//! carpeta por proyecto: ver `pixpin-proyecto::vista`); lo que es igual en
//! los dos —las marcas de borrado, las lapidas, lo acordado, los objetos, la
//! cache de resumenes, las copias— va aqui en metodos por defecto, escrito
//! una sola vez para que no pueda salir distinto en un lado.
//!
//! Todo lo que cruza este trait esta **en forma portatil**: las rutas son
//! las de Android relativas a `files` (`pins/draw/d1.excalidraw.gz`), los
//! mensajes y el proyecto son el JSON que viaja, ya normalizado al esquema de
//! Kotlin (ver `kotlin`), y los textos llevan `pixpin:files/` donde el
//! aparato tenia una ruta suya.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::base::{Base, Mapa};
use crate::canonico::{self, Json};
use crate::diferencia::{Apunte, MARCA_VIEJA};
use crate::kotlin;
use crate::mensajes::{Aparato, ArchivoInfo, Chat, LapidaDeChat};

/// La conversacion general (`Disco.GENERAL`): la que no es de ningun proyecto.
pub const GENERAL: &str = "general";
/// Como se llama en la lista del movil.
pub const NOMBRE_GENERAL: &str = "Conversación general";
/// El resumen de un mensaje borrado.
pub const BORRADO: &str = "borrado";
/// Por encima de esto el resumen de un archivo se recuerda por tamano y
/// fecha: medir lo pequeno cada vez cuesta nada, un PDF de 300 MB no.
pub const UMBRAL_DE_CACHE: u64 = 4 * 1024 * 1024;
/// Las carpetas de PixPin donde se puede escribir lo que llega.
pub const CARPETAS: [&str; 7] = [
    "guardados/",
    "pins/",
    "proyectos/",
    "tablas/",
    "croquis3d/",
    "notas/",
    "voz/",
];
/// El prefijo con que viajan las rutas de un aparato (`Rutas.PORTATIL`).
pub const PORTATIL: &str = "pixpin:files/";

/// Lo que hay que recargar despues de escribir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cambio {
    Mensajes,
    Proyectos,
    Archivos,
    Identidad,
}

/// `Identidad` de Android: este aparato, el codigo del grupo y los miembros.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Identidad {
    pub yo: Aparato,
    pub codigo: Option<String>,
    pub miembros: Vec<Aparato>,
}

/// La marca que deja un mensaje borrado (`Marca`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Marca {
    pub chat: String,
    /// El codigo de chat que tenia (`47·K7Q2` o `47a`).
    pub sena: String,
    pub cuando: i64,
    /// Su codigo unico. Nulo en las de antes de los codigos.
    pub uid: Option<String>,
}

/// Lo que escribe un aparato cuando le llega un archivo: `llenar` escribe y
/// dice si lo escrito vale.
pub type Llenar<'a> = &'a mut dyn FnMut(&mut dyn Write) -> io::Result<bool>;

/// Lo que cada aparato pone.
pub trait Disco {
    /// La carpeta de datos (`files` en el movil). Tambien es la llave de
    /// «ocupado»: una sincronizacion a la vez por carpeta.
    fn raiz(&self) -> PathBuf;

    /// Donde vive lo de sincronizar: `files/sincro`.
    fn sincro(&self) -> PathBuf {
        self.raiz().join("sincro")
    }

    fn identidad(&self) -> io::Result<Identidad>;
    fn guardar_identidad(&self, i: &Identidad) -> io::Result<()>;

    /// Avisa a la aplicacion de que algo cambio en disco.
    fn avisar(&self, _c: Cambio) {}

    /// La conversacion general y una por proyecto.
    fn chats(&self) -> io::Result<Vec<Chat>>;

    /// Los mensajes de un chat, portatiles y normalizados, en el orden en que
    /// estan. Lo que no se pueda leer no sale.
    fn mensajes(&self, chat: &str) -> io::Result<Vec<Json>>;

    /// `Disco.aplicarMensajes`: `poner` en JSON portatil; lo que ya hubiera
    /// con ese codigo unico se sustituye conservando el recordatorio, que es
    /// de este aparato. `borrar` son codigos unicos y dejan marca.
    fn aplicar_mensajes(
        &self,
        chat: &str,
        poner: &[String],
        borrar: &[String],
        ahora: i64,
    ) -> io::Result<()>;

    /// `Disco.sellar`: los codigos que le falten a lo guardado.
    fn sellar(&self) -> io::Result<()>;

    /// `Disco.adoptarDocumentos`: meter dentro lo que un proyecto tenga
    /// fuera de la carpeta. En el PC no hace falta: todo vive dentro.
    fn adoptar_documentos(&self) -> io::Result<()> {
        Ok(())
    }

    /// `Disco.borrarChat`: copia, marcas de todos sus mensajes, fuera de la
    /// lista y lapida.
    fn borrar_chat(&self, chat: &str, motivo: &str, cuando: i64, aparato: &str) -> io::Result<()>;

    /// El proyecto de un chat, portatil y normalizado.
    fn proyecto_portatil(&self, chat: &str) -> io::Result<Option<Json>>;

    /// `Disco.guardarProyecto` con `Proyectos.actualizada`.
    fn guardar_proyecto(&self, p: &Json) -> io::Result<()>;

    /// `Disco.alcance`: que archivos (rutas portatiles que existen) son de un
    /// chat, y con que se nombran. Igual en todos los aparatos: sale de los
    /// mensajes y del proyecto portatiles.
    fn alcance(&self, chat: &str) -> io::Result<Vec<(String, String)>> {
        alcance_de(self, chat)
    }

    /// Donde esta en este aparato el archivo portatil `rel` de `chat`.
    fn ruta(&self, chat: &str, rel: &str) -> PathBuf;

    /// Un texto guardado, a su forma portatil (`Rutas.aPortatil`).
    fn a_portatil(&self, chat: &str, rel: &str, texto: String) -> String;

    /// Un texto que llega, a como se guarda aqui (`Rutas.aLocal`).
    fn a_local(&self, chat: &str, texto: String) -> String;

    /// **Lo anotado sobre los adjuntos de `chat` que hay en disco**
    /// (`AnotacionesDelAdjunto.porUid`), en rutas portatiles
    /// (`pins/draw/anot-<uid>…`). Por defecto, lo que haya en la carpeta
    /// donde este aparato pone `pins/draw/` (la del movil); un aparato que
    /// reparte esa carpeta en dos (el PC: los dibujos en `lienzos/`) lo dice.
    fn anotado(&self, chat: &str) -> Vec<String> {
        let dir = self.ruta(chat, &crate::anotado::rel("x", ".marcas"));
        let Some(dir) = dir.parent() else {
            return Vec::new();
        };
        let Ok(l) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut salida: Vec<String> = l
            .flatten()
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| crate::anotado::uid_del_nombre(n).is_some())
            .map(|n| format!("{}/{n}", crate::anotado::CARPETA))
            .collect();
        salida.sort();
        salida
    }

    /// Borra lo anotado sobre el mensaje `uid` de `chat` (`todoDe(...).delete()`):
    /// sin su mensaje ya no viaja ni se abre.
    fn borrar_anotado(&self, chat: &str, uid: &str) {
        for rel in self.anotado(chat) {
            let nombre = rel.rsplit('/').next().unwrap_or_default();
            if crate::anotado::uid_del_nombre(nombre) == Some(uid) {
                let _ = std::fs::remove_file(self.ruta(chat, &rel));
            }
        }
    }

    // ------------------------------------------------------------ por defecto

    /// `Disco.apuntes`: uno por mensaje, por su codigo unico, y uno por cada
    /// marca de borrado. Una marca de antes de los codigos va por su sena.
    fn apuntes(&self, chat: &str) -> io::Result<Vec<Apunte>> {
        let mut vistos = HashSet::new();
        let mut vivos = Vec::new();
        for m in self.mensajes(chat)? {
            let clave = kotlin::unico(&m);
            if !vistos.insert(clave.clone()) {
                continue;
            }
            let cuando = kotlin::numero(&m, "cuando").unwrap_or(0);
            vivos.push(Apunte {
                sena: clave,
                creado: cuando,
                tocado: cuando,
                resumen: kotlin::resumen_de(&m),
                borrado: false,
                alias: kotlin::de_chat(&m),
            });
        }
        let senas: HashSet<String> = vivos.iter().filter_map(|a| a.alias.clone()).collect();
        let mut borrados: Vec<Apunte> = Vec::new();
        for mk in self.marcas().into_iter().filter(|mk| mk.chat == chat) {
            let sigue = match &mk.uid {
                Some(u) => !vistos.contains(u),
                None => !senas.contains(&mk.sena),
            };
            if !sigue {
                continue;
            }
            let ap = Apunte {
                sena: mk
                    .uid
                    .clone()
                    .unwrap_or_else(|| format!("{MARCA_VIEJA}{}", mk.sena)),
                creado: mk.cuando,
                tocado: mk.cuando,
                resumen: BORRADO.into(),
                borrado: true,
                alias: Some(mk.sena.clone()).filter(|s| !s.trim().is_empty()),
            };
            // `associateBy`: la ultima gana, en el sitio de la primera.
            match borrados.iter_mut().find(|a| a.sena == ap.sena) {
                Some(a) => *a = ap,
                None => borrados.push(ap),
            }
        }
        vivos.extend(borrados);
        Ok(vivos)
    }

    /// Los mensajes de un chat por su codigo unico; de uno repetido, el
    /// primero.
    fn mensajes_por_clave(&self, chat: &str) -> io::Result<Vec<(String, Json)>> {
        let mut vistos = HashSet::new();
        Ok(self
            .mensajes(chat)?
            .into_iter()
            .filter_map(|m| {
                let k = kotlin::unico(&m);
                vistos.insert(k.clone()).then_some((k, m))
            })
            .collect())
    }

    // ------------------------------------------------ marcas de borrado

    fn marcas(&self) -> Vec<Marca> {
        leer_lineas(&self.sincro().join("borrados.jsonl"))
    }

    fn anotar_borrados(&self, nuevas: &[Marca]) -> io::Result<()> {
        anadir_lineas(&self.sincro().join("borrados.jsonl"), nuevas)
    }

    fn escribir_marcas(&self, lista: &[Marca]) -> io::Result<()> {
        escribir_lineas(&self.sincro().join("borrados.jsonl"), lista)
    }

    /// Las marcas despues de poner y quitar mensajes en un chat
    /// (`aplicarMensajes`, la segunda mitad): lo que vuelve pierde su marca y
    /// lo que se va deja la suya.
    fn marcas_tras_aplicar(
        &self,
        chat: &str,
        llegan: &[Json],
        quitar: &[String],
        quitados: &[Json],
        ahora: i64,
    ) -> io::Result<()> {
        let viejas = self.marcas();
        let claves: HashSet<String> = llegan.iter().map(kotlin::unico).collect();
        let senas: HashSet<String> = llegan.iter().filter_map(kotlin::de_chat).collect();
        let mut nuevas: Vec<Marca> = viejas
            .iter()
            .filter(|mk| {
                !(mk.chat == chat
                    && match &mk.uid {
                        Some(u) => claves.contains(u),
                        None => senas.contains(&mk.sena),
                    })
            })
            .cloned()
            .collect();
        let mut vistas = HashSet::new();
        for k in quitar {
            if !vistas.insert(k.clone()) {
                continue;
            }
            if viejas
                .iter()
                .any(|mk| mk.chat == chat && mk.uid.as_deref() == Some(k))
            {
                continue;
            }
            let sena = quitados
                .iter()
                .find(|m| kotlin::unico(m) == *k)
                .and_then(kotlin::de_chat)
                .unwrap_or_default();
            nuevas.push(Marca {
                chat: chat.into(),
                sena,
                cuando: ahora,
                uid: Some(k.clone()),
            });
        }
        if nuevas != viejas {
            self.escribir_marcas(&nuevas)?;
        }
        Ok(())
    }

    // ------------------------------------------------------------ lapidas

    /// Los proyectos borrados aqui; de cada chat manda la ultima linea.
    fn lapidas(&self) -> Vec<LapidaDeChat> {
        let mut salida: Vec<LapidaDeChat> = Vec::new();
        for l in leer_lineas::<LapidaDeChat>(&self.sincro().join("chatsborrados.jsonl")) {
            match salida.iter_mut().find(|x| x.chat == l.chat) {
                Some(x) => *x = l,
                None => salida.push(l),
            }
        }
        salida
    }

    fn anotar_lapida(&self, chat: &str, cuando: i64, aparato: &str) -> io::Result<()> {
        anotar_lapida_en(&self.sincro(), chat, cuando, aparato)
    }

    /// El proyecto vuelve a estar vivo aqui.
    fn quitar_lapida(&self, chat: &str) -> io::Result<()> {
        quitar_lapida_en(&self.sincro(), chat)
    }

    // ------------------------------------------------------------ archivos

    /// El contenido de un archivo de texto, ya portatil y descomprimido.
    fn texto_de(&self, chat: &str, rel: &str) -> io::Result<String> {
        let f = self.ruta(chat, rel);
        let bytes = std::fs::read(&f)?;
        let crudo = if f.extension().is_some_and(|e| e == "gz") {
            let mut s = String::new();
            flate2::read::GzDecoder::new(&bytes[..]).read_to_string(&mut s)?;
            s
        } else {
            String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        };
        Ok(self.a_portatil(chat, rel, crudo))
    }

    /// `Disco.escribirArchivo`: primero a un temporal y se cambia al acabar.
    /// Si `llenar` dice que no vale, el de antes se queda como estaba. Lo de
    /// texto se pasa a local y se comprime si su sitio acaba en `.gz`.
    fn escribir_archivo(&self, chat: &str, rel: &str, llenar: Llenar<'_>) -> io::Result<bool> {
        if !permitida(rel) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("Ruta no permitida: {rel}"),
            ));
        }
        let destino = self.ruta(chat, rel);
        if let Some(p) = destino.parent() {
            std::fs::create_dir_all(p)?;
        }
        let tmp = con_sufijo(&destino, ".sincro");
        let hecho = (|| -> io::Result<bool> {
            let mut f = io::BufWriter::new(std::fs::File::create(&tmp)?);
            if !llenar(&mut f)? {
                return Ok(false);
            }
            f.flush()?;
            drop(f);
            if es_texto(rel) {
                let texto = String::from_utf8(std::fs::read(&tmp)?)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                let local = self.a_local(chat, texto);
                if destino.extension().is_some_and(|e| e == "gz") {
                    let mut gz = flate2::write::GzEncoder::new(
                        std::fs::File::create(&tmp)?,
                        flate2::Compression::default(),
                    );
                    gz.write_all(local.as_bytes())?;
                    gz.finish()?;
                } else {
                    std::fs::write(&tmp, local)?;
                }
            }
            // Una leccion aprendida que tambien cambio aqui no se pisa: se
            // junta con lo que llega.
            juntar_leccion(rel, &destino, &tmp)?;
            reemplazar(&tmp, &destino)?;
            Ok(true)
        })();
        if !matches!(hecho, Ok(true)) {
            let _ = std::fs::remove_file(&tmp);
        }
        if matches!(hecho, Ok(true)) {
            self.avisar(Cambio::Archivos);
        }
        hecho
    }

    fn escribir_texto(&self, chat: &str, rel: &str, portatil: &str) -> io::Result<bool> {
        self.escribir_archivo(chat, rel, &mut |s| {
            s.write_all(portatil.as_bytes())?;
            Ok(true)
        })
    }

    /// El resumen de un archivo como lo cuenta la sincronizacion: canonico si
    /// es de texto.
    fn resumen_de_archivo(&self, chat: &str, rel: &str) -> io::Result<String> {
        if es_texto(rel) {
            return Ok(canonico::resumen(&self.texto_de(chat, rel)?));
        }
        sha256_de(&mut std::fs::File::open(self.ruta(chat, rel))?)
    }

    /// `Disco.archivos`: resumen, tamano y fecha de cada archivo del chat.
    fn archivos(
        &self,
        chat: &str,
        conocidos: &HashMap<String, ArchivoInfo>,
    ) -> io::Result<Vec<ArchivoInfo>> {
        let mut cache = self.leer_cache();
        let mut tocada = false;
        let mut salida = Vec::new();
        for (rel, etiqueta) in self.alcance(chat)? {
            let f = self.ruta(chat, &rel);
            let Ok(meta) = std::fs::metadata(&f) else {
                continue;
            };
            let (largo, fecha) = (meta.len() as i64, milis(&meta));
            if let Some(c) = conocidos
                .get(&rel)
                .filter(|c| c.bytes == largo && c.tocado == fecha)
            {
                salida.push(ArchivoInfo {
                    etiqueta,
                    ..c.clone()
                });
                continue;
            }
            let sello = format!("{largo}:{fecha}");
            let guardado = cache.get(&rel).cloned();
            let info = match guardado {
                Some(g)
                    if largo as u64 > UMBRAL_DE_CACHE
                        && g.split('|').next() == Some(sello.as_str()) =>
                {
                    ArchivoInfo {
                        ruta: rel.clone(),
                        resumen: g.split_once('|').map(|x| x.1).unwrap_or("").into(),
                        bytes: largo,
                        tocado: fecha,
                        etiqueta,
                        crudo: String::new(),
                    }
                }
                _ if es_texto(&rel) => {
                    let texto = self.texto_de(chat, &rel)?;
                    let resumen = canonico::resumen(&texto);
                    cache.insert(rel.clone(), format!("{sello}|{resumen}"));
                    tocada = true;
                    ArchivoInfo {
                        ruta: rel.clone(),
                        resumen,
                        bytes: largo,
                        tocado: fecha,
                        etiqueta,
                        crudo: canonico::sha256_hex(texto.as_bytes()),
                    }
                }
                _ => {
                    let resumen = self.resumen_de_archivo(chat, &rel)?;
                    cache.insert(rel.clone(), format!("{sello}|{resumen}"));
                    tocada = true;
                    ArchivoInfo {
                        ruta: rel.clone(),
                        resumen: resumen.clone(),
                        bytes: largo,
                        tocado: fecha,
                        etiqueta,
                        crudo: resumen,
                    }
                }
            };
            salida.push(info);
        }
        if tocada {
            self.guardar_cache(&cache)?;
        }
        Ok(salida)
    }

    fn leer_cache(&self) -> HashMap<String, String> {
        std::fs::read_to_string(self.sincro().join("resumenes.txt"))
            .map(|t| {
                t.lines()
                    .filter_map(|l| l.split_once('\t'))
                    .map(|(a, b)| (a.to_string(), b.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn guardar_cache(&self, m: &HashMap<String, String>) -> io::Result<()> {
        let texto: String = m.iter().map(|(k, v)| format!("{k}\t{v}\n")).collect();
        escribir_atomico(&self.sincro().join("resumenes.txt"), texto.as_bytes())
    }

    /// `Disco.textoBase`: el texto de la version `resumen` de un archivo: lo
    /// acordado guardado, o el de ahora si es esa.
    fn texto_base(&self, chat: &str, rel: &str, resumen: &str) -> Option<String> {
        if let Some(o) = self.objeto(resumen) {
            return Some(o);
        }
        if !self.ruta(chat, rel).is_file() {
            return None;
        }
        let ahora = self.texto_de(chat, rel).ok()?;
        (canonico::resumen(&ahora) == resumen).then_some(ahora)
    }

    // ------------------------------------------------------ lo acordado

    fn archivo_de_base(&self, otro: &str, chat: &str) -> PathBuf {
        self.sincro()
            .join("base")
            .join(limpio(otro))
            .join(format!("{}.json", limpio(chat)))
    }

    fn base(&self, otro: &str, chat: &str) -> Option<Base> {
        let t = std::fs::read_to_string(self.archivo_de_base(otro, chat)).ok()?;
        serde_json::from_str(&t).ok()
    }

    /// Se guarda con el texto de Kotlin: el orden de sus mapas es el sello.
    fn guardar_base(&self, otro: &str, chat: &str, base: &Base) -> io::Result<()> {
        escribir_atomico(
            &self.archivo_de_base(otro, chat),
            base.texto_kotlin().as_bytes(),
        )
    }

    fn carpeta_de_objetos(&self) -> PathBuf {
        self.sincro().join("objetos")
    }

    /// El contenido de lo acordado, por su resumen, comprimido.
    fn objeto(&self, resumen: &str) -> Option<String> {
        if resumen.is_empty() || !resumen.chars().all(|c| c.is_ascii_alphanumeric()) {
            return None;
        }
        let bytes = std::fs::read(self.carpeta_de_objetos().join(format!("{resumen}.gz"))).ok()?;
        let mut s = String::new();
        flate2::read::GzDecoder::new(&bytes[..])
            .read_to_string(&mut s)
            .ok()?;
        Some(s)
    }

    fn hay_objeto(&self, resumen: &str) -> bool {
        self.carpeta_de_objetos()
            .join(format!("{resumen}.gz"))
            .is_file()
    }

    fn escribir_objeto(&self, resumen: &str, canonico: &str) -> io::Result<()> {
        if self.hay_objeto(resumen) {
            return Ok(());
        }
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(canonico.as_bytes())?;
        escribir_atomico(
            &self.carpeta_de_objetos().join(format!("{resumen}.gz")),
            &gz.finish()?,
        )
    }

    /// `Disco.guardarObjetosDeBase`: guarda lo acordado que siga aqui igual y
    /// tira lo que ya no senala ninguna base.
    fn guardar_objetos_de_base(&self, chat: &str, base: &Base) {
        for (rel, resumen) in base.archivos.iter() {
            if !es_texto(rel) || self.hay_objeto(resumen) || !self.ruta(chat, rel).is_file() {
                continue;
            }
            if let Ok(t) = self.texto_de(chat, rel) {
                let c = canonico::de(&t);
                if canonico::sha256_hex(c.as_bytes()) == resumen {
                    let _ = self.escribir_objeto(resumen, &c);
                }
            }
        }
        if let Ok(por_clave) = self.mensajes_por_clave(chat) {
            for (clave, resumen) in base.mensajes.iter() {
                if resumen == BORRADO || self.hay_objeto(resumen) {
                    continue;
                }
                let Some((_, m)) = por_clave.iter().find(|(k, _)| k == clave) else {
                    continue;
                };
                let t = kotlin::texto_de_base(m);
                if canonico::sha256_hex(t.as_bytes()) == resumen {
                    let _ = self.escribir_objeto(resumen, &t);
                }
            }
        }
        self.recoger_objetos();
    }

    fn recoger_objetos(&self) {
        let mut vivos = HashSet::new();
        let mut pendientes = vec![self.sincro().join("base")];
        while let Some(d) = pendientes.pop() {
            let Ok(l) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in l.flatten() {
                let p = e.path();
                if p.is_dir() {
                    pendientes.push(p);
                } else if p.extension().is_some_and(|x| x == "json")
                    && let Some(b) = std::fs::read_to_string(&p)
                        .ok()
                        .and_then(|t| serde_json::from_str::<Base>(&t).ok())
                {
                    vivos.extend(b.archivos.valores().map(str::to_string));
                    vivos.extend(b.mensajes.valores().map(str::to_string));
                }
            }
        }
        if let Ok(l) = std::fs::read_dir(self.carpeta_de_objetos()) {
            for e in l.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if let Some(r) = n.strip_suffix(".gz")
                    && !vivos.contains(r)
                {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }

    // ------------------------------------------------ elegidos y fechas

    fn elegidos(&self, otro: &str) -> Option<BTreeSet<String>> {
        let t = std::fs::read_to_string(
            self.sincro()
                .join("elegidos")
                .join(format!("{}.txt", limpio(otro))),
        )
        .ok()?;
        Some(
            t.lines()
                .filter(|l| !l.trim().is_empty())
                .map(str::to_string)
                .collect(),
        )
    }

    fn guardar_elegidos(&self, otro: &str, chats: &BTreeSet<String>) -> io::Result<()> {
        let texto = chats.iter().cloned().collect::<Vec<_>>().join("\n");
        escribir_atomico(
            &self
                .sincro()
                .join("elegidos")
                .join(format!("{}.txt", limpio(otro))),
            texto.as_bytes(),
        )
    }

    fn ultima_vez(&self, otro: &str) -> i64 {
        std::fs::read_to_string(
            self.sincro()
                .join("elegidos")
                .join(format!("{}.cuando", limpio(otro))),
        )
        .ok()
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(0)
    }

    fn apuntar_vez(&self, otro: &str, cuando: i64) -> io::Result<()> {
        escribir_atomico(
            &self
                .sincro()
                .join("elegidos")
                .join(format!("{}.cuando", limpio(otro))),
            cuando.to_string().as_bytes(),
        )
    }

    // ------------------------------------------------------------ copias

    /// `Copias.hacer`: como esta un chat ahora, antes de que nada lo pise.
    /// Devuelve si se hizo (no se repite una igual a la ultima).
    ///
    /// **Si no se pudo hacer, el error sube y lo que iba a escribir encima se
    /// para.** La copia es la unica forma de volver atras si la
    /// sincronizacion trae algo roto, y seguir sin ella es exactamente el
    /// caso que costo los lienzos de «Tesis»: escribir encima sin red.
    fn hacer_copia(&self, chat: &str, motivo: &str, ahora: i64) -> io::Result<bool> {
        crate::copias::hacer(self, chat, motivo, ahora)
    }
}

// ------------------------------------------------------------------ comun

/// `Disco.esTexto`: los lienzos, croquis y tablas viajan como texto
/// portatil.
pub fn es_texto(rel: &str) -> bool {
    rel.ends_with(".excalidraw.gz")
        || rel.ends_with(".croquis.gz")
        || (rel.starts_with("tablas/") && rel.ends_with(".json"))
}

/// **Lo que llega de una leccion aprendida, junto con lo de aqui** (mejora
/// del PC, 3-oct-2026). En el movil un `.leccion` es un archivo mas y gana
/// el tocado mas tarde; aqui, si el de este equipo tiene algo que lo que
/// llega no trae (una repeticion, un repaso, etiquetas de una edicion mas
/// nueva), se escribe en `tmp` la fusion en vez de lo que llego
/// (`pixpin_lecciones::fusion::al_llegar`). Si lo que llega ya lo trae todo,
/// `tmp` se queda como estaba, byte a byte: asi no se fabrica una diferencia
/// que volveria a viajar.
///
/// No cambia el protocolo: para el cable sigue siendo un archivo binario
/// (`es_texto` no lo incluye, igual que `Disco.kt`). Si la fusion difiere de
/// lo que tiene el otro, la vuelta siguiente se la manda.
pub fn juntar_leccion(rel: &str, destino: &Path, tmp: &Path) -> io::Result<()> {
    if !rel.ends_with(pixpin_lecciones::leccion::EXTENSION) || !destino.is_file() {
        return Ok(());
    }
    let (Ok(aqui), Ok(llega)) = (
        std::fs::read_to_string(destino),
        std::fs::read_to_string(tmp),
    ) else {
        return Ok(());
    };
    if let Some(junta) = pixpin_lecciones::fusion::al_llegar(&aqui, &llega) {
        std::fs::write(tmp, junta)?;
    }
    Ok(())
}

/// `Disco.permitida`: solo dentro de las carpetas de PixPin y nunca los
/// indices; nada de `..` ni de `\`, que en Windows tambien separa carpetas.
pub fn permitida(rel: &str) -> bool {
    if rel.trim().is_empty() || rel.starts_with('/') || rel.contains("..") || rel.contains('\\') {
        return false;
    }
    if rel.ends_with(".tmp") || rel.ends_with(".sincro") {
        return false;
    }
    if rel == "proyectos/proyectos.json" {
        return false;
    }
    CARPETAS.iter().any(|c| rel.starts_with(c))
}

/// `limpio` de Kotlin: todo lo que no sea letra, cifra, `.`, `_` o `-`, a
/// `_`, por unidad UTF-16.
pub fn limpio(s: &str) -> String {
    s.encode_utf16()
        .map(|u| match char::from_u32(u as u32) {
            Some(c) if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') => c,
            _ => '_',
        })
        .collect()
}

/// Las rutas portatiles que aparecen en un texto (`Rutas.enTexto`, solo la
/// parte de `pixpin:files/`: lo local ya se paso a portatil antes).
pub fn en_texto(texto: &str) -> Vec<String> {
    const FIN: [char; 7] = ['"', ')', '\\', '\n', '<', '>', '\''];
    let mut salida = Vec::new();
    let mut desde = 0;
    while let Some(i) = texto[desde..].find(PORTATIL) {
        let inicio = desde + i + PORTATIL.len();
        let fin = texto[inicio..]
            .find(|c: char| FIN.contains(&c))
            .map(|j| inicio + j)
            .unwrap_or(texto.len());
        if fin > inicio {
            salida.push(texto[inicio..fin].to_string());
        }
        desde = fin;
    }
    salida
}

/// `RegistroDelChat.horaEn`: la primera fecha con sentido (2020-2100) que
/// lleve un id dentro, en trece cifras seguidas.
pub fn hora_en(id: &str) -> Option<i64> {
    let b = id.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let j = (i..b.len())
                .find(|&k| !b[k].is_ascii_digit())
                .unwrap_or(b.len());
            // `Regex("\\d{13}")` sin anclas: de un numero mas largo toma los
            // trozos de trece por la izquierda.
            let mut k = i;
            while k + 13 <= j {
                if let Ok(n) = id[k..k + 13].parse::<i64>()
                    && (1_577_836_800_000..=4_102_444_800_000).contains(&n)
                {
                    return Some(n);
                }
                k += 13;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    None
}

/// `Disco.aplicarMensajes`, la parte que no toca disco: `lista` son los
/// mensajes de este aparato (de todos los chats o de uno), `llegan` los que
/// vienen para `chat` ya normalizados. Devuelve la lista nueva y los
/// quitados.
pub fn aplicar_en_lista(
    lista: Vec<Json>,
    chat: &str,
    llegan: &[Json],
    quitar: &HashSet<String>,
) -> (Vec<Json>, Vec<Json>) {
    let mut por_clave: Vec<(String, &Json)> = Vec::new();
    for m in llegan {
        let k = kotlin::unico(m);
        // `associateBy`: el ultimo que llega con esa clave gana.
        match por_clave.iter_mut().find(|(x, _)| *x == k) {
            Some(p) => p.1 = m,
            None => por_clave.push((k, m)),
        }
    }
    let mut puestos = HashSet::new();
    let mut quitados = Vec::new();
    let mut salida = Vec::with_capacity(lista.len() + llegan.len());
    for m in lista {
        let clave = (kotlin::chat_de(&m) == chat).then(|| kotlin::unico(&m));
        match clave {
            Some(k) if por_clave.iter().any(|(x, _)| *x == k) => {
                if puestos.insert(k.clone()) {
                    let nuevo = por_clave.iter().find(|(x, _)| *x == k).map(|p| p.1);
                    if let Some(n) = nuevo {
                        let mut n = n.clone();
                        let recuerda = m
                            .como_objeto()
                            .and_then(|o| o.obtener("recuerdaEn"))
                            .cloned()
                            .unwrap_or(Json::Nulo);
                        kotlin::poner(&mut n, "recuerdaEn", recuerda);
                        salida.push(n);
                    }
                }
            }
            Some(k) if quitar.contains(&k) => quitados.push(m),
            _ => salida.push(m),
        }
    }
    for (k, m) in &por_clave {
        if !puestos.contains(k) {
            let mut n = (*m).clone();
            kotlin::poner(&mut n, "recuerdaEn", Json::Nulo);
            salida.push(n);
        }
    }
    (sin_registros_repetidos(salida), quitados)
}

/// `Disco.sinRegistrosRepetidos`: dos aparatos que repararon su chat por
/// separado ponen el mismo mensaje con el mismo id y senas distintas. Queda
/// el de la sena menor, la misma eleccion en los dos lados.
pub fn sin_registros_repetidos(lista: Vec<Json>) -> Vec<Json> {
    const PREFIJO: &str = "registro-";
    let es_registro = |m: &Json| kotlin::cadena(m, "id").is_some_and(|i| i.starts_with(PREFIJO));
    if lista.iter().filter(|m| es_registro(m)).count() < 2 {
        return lista;
    }
    let clave = |m: &Json| {
        (
            kotlin::chat_de(m),
            kotlin::cadena(m, "id").unwrap_or_default().to_string(),
        )
    };
    let mut grupos: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (i, m) in lista.iter().enumerate().filter(|(_, m)| es_registro(m)) {
        grupos.entry(clave(m)).or_default().push(i);
    }
    let mut fuera = HashSet::new();
    for idx in grupos.values().filter(|v| v.len() > 1) {
        let queda = idx
            .iter()
            .copied()
            .min_by(|a, b| {
                let sa = kotlin::de_chat(&lista[*a]).unwrap_or_default();
                let sb = kotlin::de_chat(&lista[*b]).unwrap_or_default();
                canonico::comparar_como_kotlin(&sa, &sb).then(a.cmp(b))
            })
            .unwrap_or(idx[0]);
        fuera.extend(idx.iter().copied().filter(|i| *i != queda));
    }
    lista
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !fuera.contains(i))
        .map(|(_, m)| m)
        .collect()
}

/// `Codigos.sellar(m, aparato)` y el numero de `Disco.sellar`, sobre los
/// mensajes de UN chat (en el orden en que estan). Devuelve los indices que
/// cambiaron.
pub fn sellar_chat(mensajes: &mut [Json], aparato: &str) -> Vec<usize> {
    let mut orden: Vec<usize> = (0..mensajes.len()).collect();
    orden.sort_by_key(|&i| kotlin::numero(&mensajes[i], "cuando").unwrap_or(0));
    let total = mensajes.len() as i64;
    let mut mayor = mensajes
        .iter()
        .filter_map(|m| kotlin::numero(m, "numero"))
        .max()
        .unwrap_or(0);
    let mut sin_numero = 0;
    let mut vistas = HashSet::new();
    let mut cambiados = Vec::new();
    for i in orden {
        let m = &mensajes[i];
        let antes = m.clone();
        let mut s = m.clone();
        if kotlin::cadena(&s, "uid").is_none() {
            kotlin::poner(&mut s, "uid", Json::cadena(kotlin::unico(m)));
        }
        if kotlin::cadena(&s, "aparato").is_none() && kotlin::cadena(&s, "letra").is_none() {
            kotlin::poner(&mut s, "aparato", Json::cadena(aparato));
        }
        let propio = kotlin::numero(m, "numero").unwrap_or(0);
        let mut n = if propio > 0 {
            propio
        } else {
            sin_numero += 1;
            sin_numero
        };
        let quien = kotlin::cadena(&s, "aparato")
            .or(kotlin::cadena(&s, "letra"))
            .unwrap_or_default()
            .to_string();
        if vistas.contains(&format!("{n}|{quien}")) && quien == aparato {
            n = mayor.max(total) + 1;
        }
        mayor = mayor.max(n);
        vistas.insert(format!("{n}|{quien}"));
        if n != kotlin::numero(&s, "numero").unwrap_or(0) {
            kotlin::poner(&mut s, "numero", Json::numero(n));
        }
        if s != antes {
            mensajes[i] = s;
            cambiados.push(i);
        }
    }
    cambiados
}

/// `Codigos.sellar(p)` de un proyecto: el codigo unico, la fecha de creacion
/// si su id la lleva y el codigo de cada hoja.
pub fn sellar_proyecto(p: &mut Json) {
    if kotlin::cadena(p, "uid").is_none() {
        let u = kotlin::unico_de_proyecto(p);
        kotlin::poner(p, "uid", Json::cadena(u));
    }
    if kotlin::numero(p, "creado").unwrap_or(0) <= 0 {
        let h = hora_en(kotlin::cadena(p, "id").unwrap_or_default()).unwrap_or(0);
        kotlin::poner(p, "creado", Json::numero(h));
    }
    if let Json::Objeto(o) = p
        && let Some(Json::Lista(hojas)) = o.obtener("hojas").cloned()
    {
        let hojas = hojas
            .into_iter()
            .map(|mut h| {
                if kotlin::cadena(&h, "uid").is_none() {
                    let u = kotlin::unico_de_hoja(&h);
                    kotlin::poner(&mut h, "uid", Json::cadena(u));
                }
                h
            })
            .collect();
        o.poner("hojas", Json::Lista(hojas));
    }
}

/// `Proyectos.actualizada(lista, nuevo)` para un proyecto: `antes` es el que
/// habia. Sin hojas repetidas, con las marcas de quitadas de los dos y sin
/// perder los codigos que ya tenia.
pub fn actualizado(antes: Option<&Json>, nuevo: &Json) -> Json {
    let mut p = nuevo.clone();
    // `sinHojasRepetidas`: la primera de cada id.
    let hojas: Vec<Json> = lista(&p, "hojas");
    let mut vistas = HashSet::new();
    let sin: Vec<Json> = hojas
        .iter()
        .filter(|h| vistas.insert(kotlin::cadena(h, "id").unwrap_or_default().to_string()))
        .cloned()
        .collect();
    if sin.len() != hojas.len() {
        kotlin::poner(&mut p, "hojas", Json::Lista(sin.clone()));
    }
    // `conMarcasDeQuitadas`.
    let hay: HashSet<String> = sin
        .iter()
        .filter_map(|h| kotlin::cadena(h, "id").map(str::to_string))
        .collect();
    let mut marcas: Vec<String> = Vec::new();
    let mut todas: Vec<String> = cadenas(&p, "quitadas");
    if let Some(a) = antes {
        todas.extend(cadenas(a, "quitadas"));
    }
    for m in todas {
        if !hay.contains(&m) && !marcas.contains(&m) {
            marcas.push(m);
        }
    }
    let tope = crate::mezcla::MARCAS_DE_QUITADAS;
    if marcas.len() > tope {
        marcas.drain(..marcas.len() - tope);
    }
    if marcas != cadenas(&p, "quitadas") {
        kotlin::poner(
            &mut p,
            "quitadas",
            Json::Lista(marcas.into_iter().map(Json::cadena).collect()),
        );
    }
    if let Some(a) = antes {
        if kotlin::cadena(&p, "uid").is_none()
            && let Some(u) = kotlin::cadena(a, "uid")
        {
            kotlin::poner(&mut p, "uid", Json::cadena(u));
        }
        if kotlin::numero(&p, "creado").unwrap_or(0) == 0 {
            let c = kotlin::numero(a, "creado").unwrap_or(0);
            if c != 0 {
                kotlin::poner(&mut p, "creado", Json::numero(c));
            }
        }
        if kotlin::cadena(&p, "aparato").is_none()
            && let Some(x) = kotlin::cadena(a, "aparato")
        {
            kotlin::poner(&mut p, "aparato", Json::cadena(x));
        }
        let hojas = lista(&p, "hojas");
        if hojas.iter().any(|h| kotlin::cadena(h, "uid").is_none()) {
            let suyos: HashMap<String, String> = lista(a, "hojas")
                .iter()
                .filter_map(|h| {
                    Some((
                        kotlin::cadena(h, "id")?.to_string(),
                        kotlin::cadena(h, "uid")?.to_string(),
                    ))
                })
                .collect();
            if !suyos.is_empty() {
                let hojas = hojas
                    .into_iter()
                    .map(|mut h| {
                        if kotlin::cadena(&h, "uid").is_none()
                            && let Some(u) = kotlin::cadena(&h, "id").and_then(|i| suyos.get(i))
                        {
                            kotlin::poner(&mut h, "uid", Json::cadena(u.clone()));
                        }
                        h
                    })
                    .collect();
                kotlin::poner(&mut p, "hojas", Json::Lista(hojas));
            }
        }
    }
    p
}

fn lista(o: &Json, clave: &str) -> Vec<Json> {
    o.como_objeto()
        .and_then(|x| x.obtener(clave))
        .and_then(Json::como_lista)
        .map(<[Json]>::to_vec)
        .unwrap_or_default()
}

fn cadenas(o: &Json, clave: &str) -> Vec<String> {
    lista(o, clave)
        .iter()
        .filter_map(|x| x.como_cadena().map(str::to_string))
        .collect()
}

/// Las marcas de borrado de unos mensajes que se van (`Disco.borradosEntre`).
pub fn marcas_de(chat: &str, quitados: &[Json], ahora: i64) -> Vec<Marca> {
    quitados
        .iter()
        .map(|m| Marca {
            chat: chat.into(),
            sena: kotlin::de_chat(m).unwrap_or_default(),
            cuando: ahora,
            uid: Some(kotlin::unico(m)),
        })
        .collect()
}

/// Apunta una lapida en la carpeta `sincro` dada: la usa tambien quien
/// borra proyectos sin pasar por un `Disco`.
pub fn anotar_lapida_en(sincro: &Path, chat: &str, cuando: i64, aparato: &str) -> io::Result<()> {
    anadir_lineas(
        &sincro.join("chatsborrados.jsonl"),
        &[LapidaDeChat {
            chat: chat.into(),
            cuando,
            aparato: aparato.into(),
        }],
    )
}

/// Quita las lapidas de un chat en la carpeta `sincro` dada: lo usa la
/// papelera al recuperar un proyecto sin pasar por un `Disco`. Sin esto, la
/// siguiente vuelta lo volveria a borrar nada mas recuperarlo.
pub fn quitar_lapida_en(sincro: &Path, chat: &str) -> io::Result<()> {
    let f = sincro.join("chatsborrados.jsonl");
    if !f.exists() {
        return Ok(());
    }
    let quedan: Vec<LapidaDeChat> = leer_lineas::<LapidaDeChat>(&f)
        .into_iter()
        .filter(|l| l.chat != chat)
        .collect();
    escribir_lineas(&f, &quedan)
}

/// Olvida lo acordado de un chat con todos los aparatos. Lo usa la papelera
/// al recuperar un proyecto entero: con la base de antes, las marcas de
/// borrado que el otro puso al borrarlo serian «un cambio suyo» y la vuelta
/// siguiente volveria a quitar sus mensajes uno a uno. Sin base, lo vivo
/// gana a lo borrado (`diferencia::plan`), que es lo que se quiere.
pub fn olvidar_bases_en(sincro: &Path, chat: &str) -> io::Result<()> {
    let Ok(otros) = std::fs::read_dir(sincro.join("base")) else {
        return Ok(());
    };
    let nombre = format!("{}.json", limpio(chat));
    for otro in otros.flatten().filter(|e| e.path().is_dir()) {
        let f = otro.path().join(&nombre);
        if f.is_file() {
            std::fs::remove_file(f)?;
        }
    }
    Ok(())
}

/// Apunta marcas de borrado en la carpeta `sincro` dada.
pub fn anotar_marcas_en(sincro: &Path, marcas: &[Marca]) -> io::Result<()> {
    anadir_lineas(&sincro.join("borrados.jsonl"), marcas)
}

fn leer_lineas<T: for<'de> Deserialize<'de>>(f: &Path) -> Vec<T> {
    std::fs::read_to_string(f)
        .map(|t| {
            t.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn anadir_lineas<T: Serialize>(f: &Path, lineas: &[T]) -> io::Result<()> {
    if lineas.is_empty() {
        return Ok(());
    }
    if let Some(p) = f.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut texto = String::new();
    for l in lineas {
        texto.push_str(&serde_json::to_string(l).map_err(io::Error::other)?);
        texto.push('\n');
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(f)?
        .write_all(texto.as_bytes())
}

fn escribir_lineas<T: Serialize>(f: &Path, lineas: &[T]) -> io::Result<()> {
    let mut texto = String::new();
    for l in lineas {
        texto.push_str(&serde_json::to_string(l).map_err(io::Error::other)?);
        texto.push('\n');
    }
    escribir_atomico(f, texto.as_bytes())
}

/// Un nombre de fichero con algo pegado detras (`x.gz` → `x.gz.sincro`).
pub fn con_sufijo(p: &Path, sufijo: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(sufijo);
    PathBuf::from(s)
}

/// Pone `tmp` en el sitio de `destino`, **sin que el destino deje de existir
/// en ningun momento**.
///
/// El `rename` de Rust en Windows es `MoveFileExW` con
/// `MOVEFILE_REPLACE_EXISTING`: ya sustituye un fichero que existe, de un
/// golpe, y es lo que se prueba primero. Si falla es porque alguien tiene el
/// destino abierto sin compartir el borrado, y entonces se escribe ENCIMA,
/// que es lo que hace el Kotlin del movil (`copyTo(overwrite = true)`).
///
/// Lo que **no** se hace es borrar el destino y volver a probar, que es lo
/// que hacia antes: entre el borrado y el segundo intento no habia fichero,
/// y si el segundo intento tambien fallaba —o el programa se caia ahi— no
/// quedaba nada. Este es el camino por el que se escriben `guardados.jsonl`
/// y los indices de `base/`: perderlo es perder la conversacion.
pub fn reemplazar(tmp: &Path, destino: &Path) -> io::Result<()> {
    if std::fs::rename(tmp, destino).is_ok() {
        return Ok(());
    }
    // Se lee ANTES de tocar el destino: si el temporal no esta (o no se
    // puede leer), el destino se queda como estaba y el error se cuenta.
    let datos = std::fs::read(tmp)?;
    std::fs::write(destino, &datos)?;
    let _ = std::fs::remove_file(tmp);
    Ok(())
}

/// Escribe entero o nada: temporal y cambio de nombre.
pub fn escribir_atomico(destino: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(p) = destino.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = con_sufijo(destino, ".tmp");
    std::fs::write(&tmp, bytes)?;
    reemplazar(&tmp, destino)
}

/// La fecha de un fichero en milisegundos, como `File.lastModified()`.
pub fn milis(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// SHA-256 de lo que se lea, a trozos.
pub fn sha256_de(entrada: &mut dyn Read) -> io::Result<String> {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = entrada.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Para quien necesite un mapa ordenado de lo acordado.
pub fn mapa(m: &Mapa) -> std::collections::BTreeMap<String, String> {
    m.a_btree()
}

/// `Disco.etiquetaDe`: con que se nombra un archivo en la pantalla, por el
/// mensaje que lo trae (`#47·K7Q2 · Planta baja`).
pub fn etiqueta_de(m: &Json) -> String {
    let sena = kotlin::de_chat(m)
        .map(|s| format!("#{s} · "))
        .unwrap_or_default();
    let nombre = kotlin::cadena(m, "nombre").unwrap_or_default();
    let base: String = if !nombre.trim().is_empty() {
        nombre.to_string()
    } else {
        kotlin::cadena(m, "texto")
            .unwrap_or_default()
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or_default()
            .chars()
            .take(60)
            .collect()
    };
    let base = if base.trim().is_empty() {
        kotlin::cadena(m, "clase")
            .unwrap_or_default()
            .to_lowercase()
    } else {
        base
    };
    sena + &base
}

/// `Disco.alcance`: los adjuntos de sus mensajes, lo que senalan (un lienzo,
/// una tabla, un croquis), lo del proyecto (su PDF, sus hojas) y, dentro de
/// cada lienzo o croquis, las fotos que lleva. Solo lo que existe.
pub fn alcance_de<D: Disco + ?Sized>(d: &D, chat: &str) -> io::Result<Vec<(String, String)>> {
    let mut salida: Vec<(String, String)> = Vec::new();
    let poner = |rel: Option<String>, etiqueta: &str, salida: &mut Vec<(String, String)>| {
        let Some(rel) = rel else { return };
        // Los marcadores de un lienzo, con el (`marcasJuntoA`, v0.96).
        let marcas = crate::anotado::marcas_junto_a(&rel);
        for r in std::iter::once(rel).chain(marcas) {
            if salida.iter().any(|(x, _)| *x == r) || !permitida(&r) {
                continue;
            }
            if !d.ruta(chat, &r).is_file() {
                continue;
            }
            salida.push((r, etiqueta.to_string()));
        }
    };
    // Lo anotado sobre los adjuntos (PDF suelto, Word, libro), nombrado por
    // el codigo del mensaje: tinta, marcadores, espacios, maqueta, voz y
    // sitio. Una sola lectura de la carpeta por vuelta, como `porUid`.
    let lista_anotado = d.anotado(chat);
    let anotado = crate::anotado::por_uid(lista_anotado.iter().map(String::as_str));
    let dibujo = |id: Option<&str>| id.map(|i| format!("pins/draw/{i}.excalidraw.gz"));
    let croquis = |id: Option<&str>| id.map(|i| format!("croquis3d/{i}.croquis.gz"));
    let tabla = |id: Option<&str>| id.map(|i| format!("tablas/{}.json", limpio(i)));
    for m in d.mensajes(chat)? {
        let etiqueta = etiqueta_de(&m);
        for rel in en_texto(&m.a_texto()) {
            poner(Some(rel), &etiqueta, &mut salida);
        }
        // De un adjunto o, desde el 30-sep, de una nota (sus comentarios).
        if crate::anotado::lleva_anotado(&m)
            && let Some(l) = anotado.get(&kotlin::unico(&m))
        {
            for rel in l {
                poner(Some(rel.clone()), &etiqueta, &mut salida);
            }
        }
        let referencia = kotlin::cadena(&m, "referencia");
        match kotlin::cadena(&m, "clase").unwrap_or_default() {
            "DIBUJO" | "PAGINA" => poner(dibujo(referencia), &etiqueta, &mut salida),
            "IMAGEN" => {
                let foto = format!("foto-{}", kotlin::cadena(&m, "id").unwrap_or_default());
                poner(
                    dibujo(Some(referencia.unwrap_or(&foto))),
                    &etiqueta,
                    &mut salida,
                )
            }
            "TABLA" => poner(tabla(referencia), &etiqueta, &mut salida),
            "CROQUIS" => poner(croquis(referencia), &etiqueta, &mut salida),
            "ARCHIVO" => {
                let id = kotlin::cadena(&m, "id").unwrap_or_default().to_string();
                let mut i = 0;
                loop {
                    let rel = format!("tablas/{}.json", limpio(&format!("libro-{id}-{i}")));
                    if !d.ruta(chat, &rel).exists() {
                        break;
                    }
                    poner(Some(rel), &etiqueta, &mut salida);
                    i += 1;
                }
            }
            _ => {}
        }
    }
    if let Some(p) = d.proyecto_portatil(chat)? {
        let nombre = kotlin::cadena(&p, "nombre").unwrap_or_default().to_string();
        for rel in en_texto(&p.a_texto()) {
            poner(Some(rel), &nombre, &mut salida);
        }
        // Los marcadores y espacios de su PDF, con el codigo del proyecto.
        if let Some(l) = anotado.get(&kotlin::unico_de_proyecto(&p)) {
            for rel in l {
                poner(Some(rel.clone()), &nombre, &mut salida);
            }
        }
        let lista = |k: &str| {
            p.como_objeto()
                .and_then(|o| o.obtener(k))
                .and_then(Json::como_lista)
                .map(<[Json]>::to_vec)
                .unwrap_or_default()
        };
        for h in lista("hojas") {
            let n = kotlin::cadena(&h, "nombre").unwrap_or_default();
            let e = if n.trim().is_empty() {
                nombre.as_str()
            } else {
                n
            };
            poner(dibujo(kotlin::cadena(&h, "dibujo")), e, &mut salida);
            poner(tabla(kotlin::cadena(&h, "tabla")), e, &mut salida);
            poner(croquis(kotlin::cadena(&h, "croquis")), e, &mut salida);
            // Los comentarios de una hoja `nota`, con el codigo de la hoja.
            if let Some(l) = anotado.get(&kotlin::unico_de_hoja(&h)) {
                for rel in l {
                    poner(Some(rel.clone()), e, &mut salida);
                }
            }
        }
        for c in lista("croquis") {
            poner(croquis(c.como_cadena()), &nombre, &mut salida);
        }
    }
    // Lo que llevan dentro los lienzos y los croquis: sus fotos, en el orden
    // en que salieron, como la cola de Kotlin.
    let mut i = 0;
    while i < salida.len() {
        let (rel, etiqueta) = salida[i].clone();
        i += 1;
        if !es_texto(&rel) {
            continue;
        }
        let Ok(texto) = d.texto_de(chat, &rel) else {
            continue;
        };
        for dentro in en_texto(&texto) {
            poner(Some(dentro), &etiqueta, &mut salida);
        }
    }
    Ok(salida)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn solo_se_escribe_dentro_de_las_carpetas_de_pixpin() {
        assert!(!permitida("../../etc/passwd"));
        assert!(!permitida("guardados.jsonl"));
        assert!(!permitida("proyectos/proyectos.json"));
        assert!(!permitida("sincro/identidad.json"));
        assert!(!permitida(r"guardados\..\x"));
        assert!(!permitida("pins/draw/d1.excalidraw.gz.sincro"));
        assert!(permitida("guardados/1_foto.jpg"));
        assert!(permitida("pins/draw/d1.excalidraw.gz"));
    }

    #[test]
    fn las_rutas_se_encuentran_en_el_texto() {
        let t = r#"{"a":"pixpin:files/guardados/x.pdf","b":"(pixpin:files/pins/y.png)","c":"pixpin:files/"}"#;
        assert_eq!(en_texto(t), ["guardados/x.pdf", "pins/y.png"]);
    }

    #[test]
    fn la_hora_de_un_id_es_la_primera_con_sentido() {
        assert_eq!(hora_en("hoja-1726243200000"), Some(1_726_243_200_000));
        assert_eq!(
            hora_en("x-0000000000001-1726000000000"),
            Some(1_726_000_000_000)
        );
        assert_eq!(hora_en("pr-1"), None);
    }

    #[test]
    fn limpio_es_el_de_kotlin() {
        assert_eq!(limpio("../a b/ñ"), ".._a_b__");
        assert_eq!(limpio("id-tab_1.x"), "id-tab_1.x");
    }

    fn carpeta_de_prueba(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-disco-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn reemplazar_pone_lo_nuevo_en_el_sitio_de_lo_viejo() {
        let d = carpeta_de_prueba("reemplazar");
        let (tmp, destino) = (d.join("x.tmp"), d.join("x"));
        std::fs::write(&destino, b"viejo").unwrap();
        std::fs::write(&tmp, b"nuevo").unwrap();
        reemplazar(&tmp, &destino).unwrap();
        assert_eq!(std::fs::read(&destino).unwrap(), b"nuevo");
        assert!(!tmp.exists(), "el temporal no se queda por ahi");
    }

    #[test]
    fn una_leccion_que_llega_se_junta_con_la_de_aqui_y_lo_demas_no_se_toca() {
        use pixpin_lecciones::{Leccion, Repaso};
        let d = carpeta_de_prueba("leccion");
        let base = Leccion::nueva("k", 1000, "Revisar puntales");
        // Aqui: me volvio a pasar, mas tarde que lo que llega.
        let aqui = Repaso::repetida(&base, 9000);
        let llega = Leccion {
            tocada: 5000,
            etiquetas: vec!["obra".into()],
            ..base
        };
        let (tmp, destino) = (d.join("k.leccion.sincro"), d.join("k.leccion"));
        std::fs::write(&destino, aqui.escribir()).unwrap();
        std::fs::write(&tmp, llega.escribir()).unwrap();
        juntar_leccion("guardados/lecciones/k.leccion", &destino, &tmp).unwrap();
        let junta = Leccion::leer(&std::fs::read_to_string(&tmp).unwrap()).unwrap();
        assert_eq!(
            junta.repeticiones,
            vec![9000],
            "la repeticion de aqui no se pierde"
        );
        assert_eq!(junta.etiquetas, vec!["obra"], "ni la etiqueta de alli");
        // Caso negativo: un archivo que no es una leccion pasa tal cual.
        std::fs::write(&tmp, b"foto").unwrap();
        juntar_leccion("guardados/foto.jpg", &destino, &tmp).unwrap();
        assert_eq!(std::fs::read(&tmp).unwrap(), b"foto");
    }

    #[test]
    fn si_no_se_puede_sustituir_lo_que_habia_sigue_entero() {
        // Caso negativo y razon de ser del arreglo: antes, cuando el primer
        // `rename` fallaba se BORRABA el destino para volver a probar, y si
        // el segundo intento tampoco salia no quedaba nada. Este es el camino
        // por el que se escribe `guardados.jsonl`: era perder el chat entero.
        let d = carpeta_de_prueba("reemplazar-falla");
        let destino = d.join("guardados.jsonl");
        std::fs::write(&destino, b"la conversacion\n").unwrap();
        let tmp = d.join("no-esta.tmp");
        assert!(
            reemplazar(&tmp, &destino).is_err(),
            "no hay con que sustituir"
        );
        assert_eq!(
            std::fs::read(&destino).unwrap(),
            b"la conversacion\n",
            "lo que habia no se toca si no hay con que sustituirlo"
        );
    }

    #[test]
    fn escribir_atomico_no_deja_nada_a_medias_ni_temporales() {
        let d = carpeta_de_prueba("atomico");
        let f = d.join("sub").join("base.json");
        escribir_atomico(&f, b"{}").unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"{}");
        assert!(!con_sufijo(&f, ".tmp").exists());
    }
}
