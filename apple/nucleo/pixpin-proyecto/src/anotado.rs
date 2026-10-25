//! **Donde esta en el PC lo anotado sobre un adjunto del chat** (v0.96 del
//! movil, `sincro/AnotacionesDelAdjunto.kt`).
//!
//! # Una sola verdad: los nombres del movil
//!
//! El PC guarda lo anotado sobre un adjunto **con los mismos nombres y en el
//! mismo sitio donde la sincronizacion pone lo que llega del movil**: la
//! tinta en `lienzos/anot-<uid>….excalidraw` de la carpeta del chat (lo que
//! `vista::DiscoPc::ruta` da para `pins/draw/anot-<uid>….excalidraw.gz`) y
//! lo demas (`.marcas`, `.espacios`, `.maqueta`, `.voz`, `.sitio`) en
//! `android/pins/draw/`. No hay traduccion al sincronizar: lo que el lector
//! escribe es lo que viaja, y lo que llega es lo que el lector lee. Traducir
//! pedia dos copias de lo mismo y decidir en cada vuelta cual manda.
//!
//! Solo lo que es un adjunto de un chat del almacen tiene codigo; un
//! documento abierto desde el Explorador sigue con lo suyo junto a el
//! (`<doc>.pixpin-anotado/`, `<doc>.pixpin-lectura`), como en el movil lo
//! que no es del chat sigue por la ruta.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pixpin_sincro::anotado as a;
use pixpin_sincro::disco::Disco;
use pixpin_sincro::kotlin;

use crate::almacen::{self, Indice};
use crate::vista::{self, DiscoPc};

/// El sitio de lo anotado de algo: el chat donde vive y su base
/// (`anot-<uid>`, `anot-<uid>-texto`...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base {
    pub raiz: PathBuf,
    pub chat: String,
    pub base: String,
}

impl Base {
    /// El fichero de la base con esa terminacion (`.marcas`, `.maqueta`...).
    pub fn fichero(&self, terminacion: &str) -> PathBuf {
        DiscoPc::nuevo(&self.raiz).ruta(&self.chat, &a::rel(&self.base, terminacion))
    }

    /// El fichero de la tinta de esta base (un Word o un libro).
    pub fn tinta(&self) -> PathBuf {
        self.fichero(".excalidraw.gz")
    }

    /// La base de la tinta de la hoja `pagina` (desde 0) del adjunto `x`.
    pub fn de_pagina(raiz: &Path, x: &Adjunto, pagina: u32) -> Base {
        Base {
            raiz: raiz.to_path_buf(),
            chat: x.chat.clone(),
            base: a::de_pagina(&x.uid, pagina),
        }
    }

    /// El fichero del marco de su tinta (`<base>.hoja`).
    pub fn marco(&self) -> PathBuf {
        self.fichero(a::HOJA)
    }

    /// **El resumen entero de su tinta como lo cuenta la sincronizacion**
    /// (`Disco::resumen_de_archivo`: SHA-256 del JSON canonico del texto
    /// portatil), el mismo que saca el movil de la suya. `None` si no hay
    /// tinta o no se lee. Su principio es la huella del marco
    /// (`pixpin_sincro::anotado::huella_de_tinta`).
    pub fn resumen_de_la_tinta(&self) -> Option<String> {
        if !self.tinta().is_file() {
            return None;
        }
        DiscoPc::nuevo(&self.raiz)
            .resumen_de_archivo(&self.chat, &a::rel(&self.base, ".excalidraw.gz"))
            .ok()
    }

    /// **Escribe el marco `m`** en las dos lineas de Android (`escribirMarco`),
    /// por un temporal y **sin tocar el fichero si el que hay ya dice lo
    /// mismo** (`casiIgual`, con sus decimales): una fecha nueva es un envio
    /// mas. Uno que lo dice pero trae la huella del PC del 29-sep se deja en
    /// dos lineas, con su primera tal cual, para que Android lo lea.
    pub fn escribir_marco(&self, m: &a::MarcoDeLaHoja) -> std::io::Result<()> {
        if let Some(texto) = leer(&self.marco())
            && let Some((ya, huella)) = a::MarcoDeLaHoja::de_texto_con_huella(&texto)
            && ya.casi_igual(m)
        {
            return match huella {
                Some(_) => self.quitar_la_huella(),
                None => Ok(()),
            };
        }
        escribir(&self.marco(), &m.a_texto())
    }

    /// La huella de su tinta de ahora (ver [`Base::resumen_de_la_tinta`]):
    /// con la que se comprueba un marco del PC del 29-sep que la trae.
    pub fn huella(&self) -> Option<String> {
        self.resumen_de_la_tinta()
            .map(|r| r[..a::CIFRAS_DE_LA_HUELLA].to_string())
    }

    /// **Deja el marco que ya hay en las dos lineas de Android**, con su
    /// primera linea tal cual (`MarcoDeLaHoja::sin_huella`). Si no hay marco
    /// o no se entiende, nada.
    pub fn quitar_la_huella(&self) -> std::io::Result<()> {
        let Some(texto) = leer(&self.marco()) else {
            return Ok(());
        };
        match a::MarcoDeLaHoja::sin_huella(&texto) {
            Some(nuevo) => escribir(&self.marco(), &nuevo),
            None => Ok(()),
        }
    }
}

/// El almacen al que pertenece un fichero: la carpeta de encima de
/// `proyectos/` que tiene su `indice.json`. `None` fuera de todo almacen.
pub fn raiz_de(fichero: &Path) -> Option<PathBuf> {
    let original = original_de(fichero);
    let fichero = original.as_deref().unwrap_or(fichero);
    fichero
        .ancestors()
        .find(|x| {
            x.file_name().is_some_and(|n| n == "proyectos") && x.join("indice.json").is_file()
        })
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}

/// Las copias que se leen en nombre de otro fichero: copia → original.
fn suplentes() -> &'static Mutex<HashMap<PathBuf, PathBuf>> {
    static S: std::sync::OnceLock<Mutex<HashMap<PathBuf, PathBuf>>> = std::sync::OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

/// **Leer `copia` en nombre de `original`** (20-oct-2026): un `.doc` o un
/// `.ppt` del chat se convierte a `.docx`/`.pptx` para leerlo en el visor
/// propio, y lo anotado sobre la copia (tinta, marcos, marcadores) tiene que
/// ser lo del adjunto original, el que viaja. Desde aqui [`raiz_de`] y
/// [`adjunto_de`] contestan por la copia lo del original.
pub fn suplir(copia: &Path, original: &Path) {
    if let Ok(mut s) = suplentes().lock() {
        s.insert(canonica(copia), original.to_path_buf());
    }
}

/// El original en cuyo nombre se lee `copia` ([`suplir`]), si lo hay.
pub fn original_de(copia: &Path) -> Option<PathBuf> {
    let s = suplentes().lock().ok()?;
    if s.is_empty() {
        return None;
    }
    s.get(&canonica(copia)).cloned()
}

/// Un adjunto del chat: de que chat es y el codigo de su mensaje.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adjunto {
    pub chat: String,
    pub uid: String,
}

fn canonica(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Lo ya encontrado: el codigo de un mensaje no cambia (`conocidos`).
fn conocidos() -> &'static Mutex<HashMap<PathBuf, Adjunto>> {
    static C: std::sync::OnceLock<Mutex<HashMap<PathBuf, Adjunto>>> = std::sync::OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// **El mensaje del chat que lleva este fichero** (`uidDe`): se busca en el
/// chat de la carpeta donde esta, por la ruta que el mensaje senala. `None`
/// si no es un adjunto del chat.
pub fn adjunto_de(raiz: &Path, doc: &Path) -> Option<Adjunto> {
    let doc = canonica(&original_de(doc).unwrap_or_else(|| doc.to_path_buf()));
    if let Some(x) = conocidos().lock().ok().and_then(|c| c.get(&doc).cloned()) {
        return Some(x);
    }
    let d = DiscoPc::nuevo(raiz);
    for (chat, f) in d.mapa() {
        let carpeta = canonica(&almacen::carpeta(raiz, &f.id));
        if !doc.starts_with(&carpeta) {
            continue;
        }
        let Ok(mensajes) = d.mensajes(&chat) else {
            continue;
        };
        for m in mensajes {
            let Some(r) = kotlin::cadena(&m, "ruta") else {
                continue;
            };
            let Some(real) = vista::ruta_real(raiz, &f.id, r) else {
                continue;
            };
            if canonica(&real) == doc {
                let x = Adjunto {
                    chat: chat.clone(),
                    uid: kotlin::unico(&m),
                };
                if let Ok(mut c) = conocidos().lock() {
                    c.insert(doc, x.clone());
                }
                return Some(x);
            }
        }
    }
    None
}

/// La base de un Word, un libro o una pagina del chat (`delDocumento`).
pub fn base_del_documento(raiz: &Path, doc: &Path) -> Option<Base> {
    let x = adjunto_de(raiz, doc)?;
    let nombre = doc.file_name()?.to_string_lossy().to_string();
    Some(Base {
        raiz: raiz.to_path_buf(),
        chat: x.chat,
        base: a::del_documento(&x.uid, &nombre),
    })
}

/// La base de los marcadores y espacios de un PDF (`CapasDelPdf.baseSuelta`):
/// el codigo del proyecto si es el PDF de uno (`ficha`), el del mensaje si
/// es un adjunto del chat, o `None`.
pub fn base_del_pdf(raiz: &Path, pdf: &Path, ficha_del_proyecto: Option<&str>) -> Option<Base> {
    if let Some(ficha) = ficha_del_proyecto {
        let chat = vista::chat_de_ficha(raiz, ficha)?;
        let p = DiscoPc::nuevo(raiz).proyecto_portatil(&chat).ok()??;
        return Some(Base {
            raiz: raiz.to_path_buf(),
            base: a::del_pdf(&kotlin::unico_de_proyecto(&p)),
            chat,
        });
    }
    let x = adjunto_de(raiz, pdf)?;
    Some(Base {
        raiz: raiz.to_path_buf(),
        chat: x.chat,
        base: a::del_pdf(&x.uid),
    })
}

/// La tinta de la hoja `i` (desde 0) de un PDF suelto del chat
/// (`anot-<uid>-p<i>`); `None` si no es un adjunto.
pub fn hoja_del_pdf(raiz: &Path, pdf: &Path, i: u32) -> Option<PathBuf> {
    let x = adjunto_de(raiz, pdf)?;
    Some(DiscoPc::nuevo(raiz).ruta(&x.chat, &a::rel(&a::de_pagina(&x.uid, i), ".excalidraw.gz")))
}

/// **El marco de la tinta de la hoja `i`** de un PDF suelto del chat
/// (`anot-<uid>-p<i>.hoja`, `pixpin_sincro::anotado::MarcoDeLaHoja`), junto
/// a lo demas del mensaje; `None` si no es un adjunto.
pub fn marco_del_pdf(raiz: &Path, pdf: &Path, i: u32) -> Option<PathBuf> {
    let x = adjunto_de(raiz, pdf)?;
    Some(DiscoPc::nuevo(raiz).ruta(&x.chat, &a::rel(&a::de_pagina(&x.uid, i), a::HOJA)))
}

/// **Los adjuntos de un chat**: el codigo de cada mensaje que senala un
/// fichero y donde esta ese fichero en este equipo (exista o no aun).
pub fn adjuntos_del_chat(raiz: &Path, chat: &str, ficha: &str) -> Vec<(Adjunto, PathBuf)> {
    let Ok(mensajes) = DiscoPc::nuevo(raiz).mensajes(chat) else {
        return Vec::new();
    };
    mensajes
        .iter()
        .filter_map(|m| {
            let real = vista::ruta_real(raiz, ficha, kotlin::cadena(m, "ruta")?)?;
            Some((
                Adjunto {
                    chat: chat.to_string(),
                    uid: kotlin::unico(m),
                },
                real,
            ))
        })
        .collect()
}

/// De que es una tinta `anot-<uid>…`, por su nombre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueTinta {
    /// `anot-<uid>-p<n>`: la hoja `n` (desde 0) de un PDF suelto del chat.
    Pagina(u32),
    /// `anot-<uid>`: un Word o un libro.
    Documento,
    /// `anot-<uid>-texto`: un PDF leido como texto (el PC no lo abre).
    Texto,
}

/// Una tinta del mensaje que aun no tiene marco: con que calcularlo.
#[derive(Debug, Clone)]
pub struct TintaSinMarco {
    pub base: Base,
    pub adjunto: Adjunto,
    pub que: QueTinta,
    /// El documento del mensaje en este equipo, si el mensaje sigue ahi.
    pub doc: Option<PathBuf>,
}

/// Lo que hizo una pasada de [`poner_marcos`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Pasada {
    /// Tintas que no tenian marco y ahora lo tienen.
    pub escritos: usize,
    /// Marcos con la huella del PC del 29-sep, de su tinta, que pasan a las
    /// dos lineas de Android (v0.98.0 rechaza un marco de tres) con sus
    /// mismos numeros.
    pub sin_huella: usize,
    /// Tintas que ya tenian su marco en dos lineas: no se tocan.
    pub ya_estaban: usize,
    /// Tintas sin marco que no se pudieron calcular (sin documento, sin
    /// columna fijada, un PDF leido como texto, o un marco que no se entiende).
    pub sin_calcular: usize,
    /// Lo que no fue bien, para el registro (este crate no registra).
    pub avisos: Vec<String>,
}

/// El nombre de una tinta de un mensaje: su base, su codigo y de que es.
fn tinta_del_nombre(rel: &str) -> Option<(String, String, QueTinta)> {
    let nombre = rel.strip_prefix(a::CARPETA)?.strip_prefix('/')?;
    let base = nombre.strip_suffix(".excalidraw.gz")?;
    let uid = a::uid_del_nombre(nombre)?;
    let resto = base.strip_prefix(a::PREFIJO)?.strip_prefix(uid)?;
    let que = match resto {
        "" => QueTinta::Documento,
        "-texto" => QueTinta::Texto,
        r => QueTinta::Pagina(r.strip_prefix("-p")?.parse().ok()?),
    };
    Some((base.to_string(), uid.to_string(), que))
}

/// **La pasada del marco de la tinta** (29-sep, «actualizar los archivos que
/// ya estan anotados usando esta forma de marco»): cada tinta de un mensaje
/// (`anot-<uid>[-p<n>]`) de cada chat que aun no tiene su `.hoja` recibe la
/// que corresponde a como se lee hoy (la regla vieja, que calcula `calcular`
/// porque pide el PDF y la maqueta, cosas de la app). **La tinta no se
/// toca.** Es lo que hace Android al abrir (`tintaDeLaHoja` con `migrar`).
///
/// Desde el 30-sep (Android v0.98.0, marco de dos lineas): un marco en dos
/// lineas se deja; uno con la huella del PC del 29-sep, si es la de su
/// tinta, pasa a dos lineas con sus mismos numeros, y si no lo es, miente
/// (la tinta se reescribio sin el) y se calcula de nuevo como si no hubiera.
/// Uno que no se entiende (un formato nuevo del otro aparato) no se toca.
/// Escribir lo mismo no toca el fichero, asi que repetirla no cambia nada ni
/// provoca envios.
pub fn poner_marcos(
    raiz: &Path,
    calcular: &mut dyn FnMut(&TintaSinMarco) -> Option<a::MarcoDeLaHoja>,
) -> Pasada {
    let d = DiscoPc::nuevo(raiz);
    let mut p = Pasada::default();
    for (chat, ficha) in d.mapa() {
        let mut docs: Option<HashMap<String, PathBuf>> = None;
        for rel in d.anotado(&chat) {
            let Some((base, uid, que)) = tinta_del_nombre(&rel) else {
                continue;
            };
            let b = Base {
                raiz: raiz.to_path_buf(),
                chat: chat.clone(),
                base,
            };
            if let Some(texto) = leer(&b.marco()) {
                match a::MarcoDeLaHoja::de_texto_con_huella(&texto) {
                    Some((_, None)) => {
                        p.ya_estaban += 1;
                        continue;
                    }
                    Some((_, Some(h)))
                        if b.resumen_de_la_tinta()
                            .is_some_and(|r| a::huella_coincide(&h, &r)) =>
                    {
                        match b.quitar_la_huella() {
                            Ok(()) => p.sin_huella += 1,
                            Err(e) => {
                                p.avisos.push(format!(
                                    "{}: no se pudo quitar la huella al marco: {e}",
                                    b.base
                                ));
                                p.sin_calcular += 1;
                            }
                        }
                        continue;
                    }
                    // Con la huella de otra tinta: se calcula como sin marco.
                    Some((_, Some(_))) => {}
                    None => {
                        p.avisos
                            .push(format!("{}: marco que no se entiende, no se toca", b.base));
                        p.sin_calcular += 1;
                        continue;
                    }
                }
            }
            if que == QueTinta::Texto {
                p.sin_calcular += 1;
                continue;
            }
            let docs = docs.get_or_insert_with(|| {
                adjuntos_del_chat(raiz, &chat, &ficha.id)
                    .into_iter()
                    .map(|(x, r)| (x.uid, r))
                    .collect()
            });
            let t = TintaSinMarco {
                doc: docs.get(&uid).cloned(),
                adjunto: Adjunto {
                    chat: chat.clone(),
                    uid,
                },
                base: b,
                que,
            };
            match calcular(&t).filter(a::MarcoDeLaHoja::valido) {
                Some(m) => match t.base.escribir_marco(&m) {
                    Ok(()) => p.escritos += 1,
                    Err(e) => {
                        p.avisos.push(format!(
                            "{}: no se pudo escribir el marco: {e}",
                            t.base.base
                        ));
                        p.sin_calcular += 1;
                    }
                },
                None => p.sin_calcular += 1,
            }
        }
    }
    p
}

/// **Los marcadores de un lienzo del almacen, junto a su dibujo**
/// (`AnotacionesDelAdjunto.delLienzo`): para `…/<ficha>/lienzos/<d>.excalidraw`
/// es `pins/draw/<d>.marcas` de ese chat. `None` si el lienzo no es de un
/// chat del almacen.
pub fn marcas_del_lienzo(lienzo: &Path) -> Option<PathBuf> {
    junto_al_dibujo_del_movil(lienzo, ".marcas")
}

/// **La vista de cada marcador de ese lienzo** (25-oct-2026): al lado de
/// sus marcas, `pins/draw/<d>.vistas.json` (ver
/// `pixpin_sincro::anotado::vistas_junto_a`). `None` como
/// [`marcas_del_lienzo`].
pub fn vistas_del_lienzo(lienzo: &Path) -> Option<PathBuf> {
    junto_al_dibujo_del_movil(lienzo, a::VISTAS)
}

/// `pins/draw/<d><terminacion>` del chat del lienzo `…/<ficha>/lienzos/<d>.excalidraw`.
fn junto_al_dibujo_del_movil(lienzo: &Path, terminacion: &str) -> Option<PathBuf> {
    let d = lienzo.file_name()?.to_str()?.strip_suffix(".excalidraw")?;
    let carpeta = lienzo
        .parent()
        .filter(|p| p.file_name().is_some_and(|n| n == "lienzos"))?
        .parent()?;
    let ficha = carpeta.file_name()?.to_str()?;
    let raiz = raiz_de(carpeta)?;
    Indice::leer(&raiz).buscar(ficha)?;
    let chat = vista::chat_de_ficha(&raiz, ficha)?;
    Some(DiscoPc::nuevo(&raiz).ruta(&chat, &format!("{}/{d}{terminacion}", a::CARPETA)))
}

/// `leer`: el texto de un fichero de lo anotado, si esta.
pub fn leer(f: &Path) -> Option<String> {
    std::fs::read_to_string(f).ok()
}

/// `escribir`, por un temporal (la sincronizacion no debe leer nunca uno a
/// medias). **Si ya dice eso, no se toca**: una fecha nueva sin cambio haria
/// que la vuelta siguiente lo volviera a medir y a mandar.
pub fn escribir(f: &Path, texto: &str) -> std::io::Result<()> {
    if leer(f).as_deref() == Some(texto) {
        return Ok(());
    }
    pixpin_sincro::disco::escribir_atomico(f, texto.as_bytes())
}

/// **La migracion** (`copiarSiFalta`): lo viejo pasa a lo nuevo si lo nuevo
/// aun no existe. Lo viejo se queda. Devuelve si copio.
pub fn copiar_si_falta(viejo: &Path, nuevo: &Path) -> bool {
    if nuevo.exists() || !viejo.is_file() {
        return false;
    }
    std::fs::read(viejo)
        .and_then(|b| pixpin_sincro::disco::escribir_atomico(nuevo, &b))
        .is_ok()
}

/// **La tinta de antes, junto a la que ya trae el mensaje.**
///
/// `copiar_si_falta` es la regla del movil y alli basta: lo viejo y lo nuevo
/// son del mismo aparato. En el PC no: si el movil ya mando su tinta de ese
/// mensaje antes de que el lector del PC abriera el documento, el fichero
/// nuevo ya existe y lo anotado aqui antes (junto al documento) no pasaba
/// nunca: ni se veia en el lector ni viajaba (queja del 28-sep-2026, «lo
/// que se modifica aqui no se envia»; en los datos del usuario, seis trazos
/// del Word de la bibliografia).
///
/// Devuelve el lienzo `nuevo` con los elementos de `viejo` que no tiene:
/// uno esta si alli hay otro con su mismo `id` y sus mismos `points` (asi
/// lo ya copiado, movido o borrado despues no vuelve). Los que entran se
/// corren `dx` (la tinta de un Word del PC cuenta desde el borde de la
/// columna y la del mensaje desde el de la pagina) y, si su `id` ya lo usa
/// otro, se les da uno libre. `None` si no hay nada que anadir o si alguno
/// no se entiende.
pub fn juntar_tinta(viejo: &str, nuevo: &str, dx: f64) -> Option<String> {
    use serde_json::Value;
    let viejo: Value = serde_json::from_str(viejo).ok()?;
    let mut nuevo: Value = serde_json::from_str(nuevo).ok()?;
    let de_antes = viejo.get("elements")?.as_array()?;
    let ya = nuevo.get_mut("elements")?.as_array_mut()?;
    // Los puntos se comparan por su valor: lo copiado que dio la vuelta por
    // el movil vuelve con `{"x","y"}` donde aqui habia `[x, y]`.
    let puntos = |e: &Value| -> Vec<(f64, f64)> {
        e.get("points")
            .and_then(Value::as_array)
            .map(|l| {
                l.iter()
                    .filter_map(|p| match p {
                        Value::Array(a) => Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?)),
                        Value::Object(o) => Some((o.get("x")?.as_f64()?, o.get("y")?.as_f64()?)),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let mismos = |a: &[(f64, f64)], b: &[(f64, f64)]| {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(p, q)| (p.0 - q.0).abs() < 1e-4 && (p.1 - q.1).abs() < 1e-4)
    };
    // Lo que no lleva puntos (un texto, un ovalo) se reconoce por su tipo,
    // su tamano y su texto; la x no, que en un Word va corrida.
    let igual = |a: &Value, b: &Value, k: &str| match (a.get(k), b.get(k)) {
        (Some(x), Some(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) => (x - y).abs() < 1e-4,
            _ => x == y,
        },
        (x, y) => x == y,
    };
    let esta = |e: &Value, ya: &[Value]| {
        let p = puntos(e);
        ya.iter().any(|x| {
            x.get("id") == e.get("id")
                && igual(x, e, "type")
                && if p.is_empty() {
                    ["width", "height", "text"].iter().all(|k| igual(x, e, k))
                } else {
                    mismos(&puntos(x), &p)
                }
        })
    };
    let mut anadidos = 0;
    for e in de_antes {
        if e.get("isDeleted").and_then(Value::as_bool) == Some(true) || esta(e, ya) {
            continue;
        }
        let mut e = e.clone();
        let Some(id) = e.get("id").and_then(Value::as_str).map(str::to_string) else {
            continue;
        };
        if ya
            .iter()
            .any(|x| x.get("id").and_then(Value::as_str) == Some(id.as_str()))
        {
            let libre = (1..).map(|n| format!("{id}-antes{n}")).find(|c| {
                !ya.iter()
                    .any(|x| x.get("id").and_then(Value::as_str) == Some(c.as_str()))
            })?;
            e["id"] = Value::String(libre);
        }
        if dx != 0.0 {
            let x = e.get("x").and_then(Value::as_f64).unwrap_or(0.0);
            e["x"] = Value::from(x + dx);
        }
        ya.push(e);
        anadidos += 1;
    }
    (anadidos > 0)
        .then(|| serde_json::to_string(&nuevo).ok())
        .flatten()
}

/// La marca de que la tinta de `viejo` ya se paso a su mensaje: una vez
/// pasada, lo que se borre despues en el lector no puede volver.
fn marca_de_pasada(viejo: &Path) -> PathBuf {
    let mut n = viejo.as_os_str().to_owned();
    n.push(".pasada");
    PathBuf::from(n)
}

/// **Pasa la tinta de antes a la del mensaje, una sola vez.** Si `nuevo` no
/// existe no hace nada (eso es `copiar_si_falta`, con su corrida donde la
/// haya): solo junta cuando ya existe (ver [juntar_tinta]). En los dos casos
/// deja la marca, para que despues de copiar tampoco se junte encima.
/// Devuelve si anadio algo.
pub fn pasar_tinta_de_antes(viejo: &Path, nuevo: &Path, dx: f64) -> bool {
    let marca = marca_de_pasada(viejo);
    if marca.exists() || !viejo.is_file() || !nuevo.is_file() {
        return false;
    }
    let (Some(v), Some(n)) = (leer(viejo), leer(nuevo)) else {
        return false;
    };
    let hecho = match juntar_tinta(&v, &n, dx) {
        Some(junto) => pixpin_sincro::disco::escribir_atomico(nuevo, junto.as_bytes()).is_ok(),
        None => false,
    };
    let _ = std::fs::write(&marca, b"");
    hecho
}

/// Deja la marca de pasada tras copiar (`copiar_si_falta`): lo copiado ya
/// esta en el mensaje y juntarlo otra vez lo repetiria.
pub fn marcar_tinta_pasada(viejo: &Path) {
    if viejo.is_file() {
        let _ = std::fs::write(marca_de_pasada(viejo), b"");
    }
}

/// Lo mismo con un texto (`ponerSiFalta`).
pub fn poner_si_falta(nuevo: &Path, viejo: Option<&str>) -> bool {
    let Some(v) = viejo else {
        return false;
    };
    if nuevo.exists() {
        return false;
    }
    escribir(nuevo, v).is_ok()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::almacen::Ficha;
    use crate::cuaderno::{self, Clase, Mensaje, Sello};

    fn raiz(etiqueta: &str) -> PathBuf {
        let r =
            std::env::temp_dir().join(format!("pixpin-anotado-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    /// Un proyecto con un Word adjunto en su chat.
    fn con_un_word(r: &Path) -> (Ficha, PathBuf, Mensaje) {
        let ficha = Ficha::nueva("Tesis", 5, "PC01");
        let mut i = Indice::default();
        i.proyectos.push(ficha.clone());
        i.guardar(r).unwrap();
        let ruta = almacen::guardar_adjunto(r, &ficha.id, "informe.docx", b"PK").unwrap();
        let m = Mensaje::adjunto(
            Clase::Archivo,
            "informe.docx",
            &ruta,
            2,
            &Sello {
                cuando: 10,
                numero: 1,
                aparato: "PC01".into(),
                proyecto: ficha.id.clone(),
            },
        );
        let carpeta = almacen::carpeta(r, &ficha.id);
        cuaderno::anadir(&carpeta, &m).unwrap();
        (ficha.clone(), carpeta.join(&ruta), m)
    }

    #[test]
    fn un_word_del_chat_se_anota_con_el_codigo_de_su_mensaje_donde_la_sincronizacion_lo_pone() {
        let r = raiz("word");
        let (ficha, doc, m) = con_un_word(&r);
        let b = base_del_documento(&r, &doc).expect("es un adjunto");
        let uid = kotlin::unico(&pixpin_sincro::canonico::Json::de_valor(
            &serde_json::to_value(&m).unwrap(),
        ));
        assert_eq!(b.base, format!("anot-{uid}"));
        let carpeta = almacen::carpeta(&r, &ficha.id);
        assert_eq!(
            b.tinta(),
            carpeta.join(format!("lienzos/anot-{uid}.excalidraw"))
        );
        assert_eq!(
            b.fichero(".maqueta"),
            carpeta.join(format!("android/pins/draw/anot-{uid}.maqueta"))
        );
        // Lo escrito ahi es lo que el sincronizar manda con el mensaje.
        escribir(&b.fichero(".marcas"), "1:0.5:⭐").unwrap();
        let chat = vista::chat_de_ficha(&r, &ficha.id).unwrap();
        let alcance = DiscoPc::nuevo(&r).alcance(&chat).unwrap();
        assert!(
            alcance
                .iter()
                .any(|(x, _)| *x == format!("pins/draw/anot-{uid}.marcas")),
            "{alcance:?}"
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn lo_que_no_es_un_adjunto_del_chat_no_tiene_codigo() {
        let r = raiz("suelto");
        let (ficha, _, _) = con_un_word(&r);
        // En la carpeta del proyecto pero sin mensaje que lo senale.
        let otro = almacen::carpeta(&r, &ficha.id).join("archivos/otro.docx");
        std::fs::write(&otro, b"PK").unwrap();
        assert_eq!(base_del_documento(&r, &otro), None);
        // Fuera del almacen.
        let fuera = std::env::temp_dir().join("pixpin-anotado-fuera.docx");
        assert_eq!(raiz_de(&fuera), None);
        assert_eq!(adjunto_de(&r, &fuera), None);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn los_marcadores_de_un_lienzo_del_almacen_van_junto_a_su_dibujo_del_movil() {
        let r = raiz("lienzo");
        let (ficha, _, _) = con_un_word(&r);
        let lienzo = almacen::lienzo(&r, &ficha.id, "d9");
        assert_eq!(
            marcas_del_lienzo(&lienzo),
            Some(almacen::carpeta(&r, &ficha.id).join("android/pins/draw/d9.marcas"))
        );
        // Y sus vistas, al lado (25-oct-2026).
        assert_eq!(
            vistas_del_lienzo(&lienzo),
            Some(almacen::carpeta(&r, &ficha.id).join("android/pins/draw/d9.vistas.json"))
        );
        assert_eq!(
            vistas_del_lienzo(Path::new("C:/dibujos/d9.excalidraw")),
            None
        );
        // Caso negativo: un dibujo fuera del almacen, o de una ficha que no esta.
        assert_eq!(
            marcas_del_lienzo(Path::new("C:/dibujos/d9.excalidraw")),
            None
        );
        assert_eq!(
            marcas_del_lienzo(&almacen::lienzo(&r, "no-esta", "d9")),
            None
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_migracion_copia_una_vez_y_no_pisa_lo_nuevo() {
        let r = raiz("migrar");
        let viejo = r.join("viejo.excalidraw");
        let nuevo = r.join("n/nuevo.excalidraw");
        std::fs::write(&viejo, "de antes").unwrap();
        assert!(copiar_si_falta(&viejo, &nuevo));
        assert!(viejo.exists(), "lo viejo no se borra");
        std::fs::write(&viejo, "cambiado despues").unwrap();
        assert!(!copiar_si_falta(&viejo, &nuevo), "lo nuevo ya existe");
        assert_eq!(leer(&nuevo).as_deref(), Some("de antes"));
        let marcas = r.join("n/x.marcas");
        assert!(poner_si_falta(&marcas, Some("x:0:0:⭐")));
        assert!(!poner_si_falta(&marcas, Some("otra")));
        assert!(!poner_si_falta(&r.join("n/y.espacios"), None));
        assert!(!copiar_si_falta(&r.join("no-esta"), &r.join("n/z")));
        let _ = std::fs::remove_dir_all(&r);
    }

    /// La tinta de antes del Word de la bibliografia (datos del usuario): seis
    /// trazos del PC, en las unidades de su columna.
    const DE_ANTES: &str = r#"{"type":"excalidraw","elements":[
      {"id":"pc1","type":"freedraw","x":10,"y":40,"width":0,"height":0,"points":[[0,0],[2,2],[9,1]]},
      {"id":"pc2","type":"freedraw","x":30,"y":90,"width":0,"height":0,"points":[[0,0],[5,5]]},
      {"id":"pc3","type":"freedraw","x":1,"y":1,"isDeleted":true,"points":[[0,0],[1,1]]}]}"#;

    #[test]
    fn la_tinta_de_antes_se_junta_con_la_que_ya_mando_el_movil_corrida_y_sin_pisar_ids() {
        // El movil ya mando su tinta del mensaje, con un «pc1» que no es el de aqui.
        let del_movil = r#"{"elements":[
          {"id":"Xa9","type":"freedraw","x":300,"y":10278,"width":4,"height":4,"points":[{"x":0.0,"y":0.0},{"x":4.0,"y":4.0}]},
          {"id":"pc1","type":"freedraw","x":7,"y":7,"width":3,"height":3,"points":[{"x":0.0,"y":0.0},{"x":3.0,"y":3.0}]}],"files":{}}"#;
        let junto = juntar_tinta(DE_ANTES, del_movil, 256.0).expect("habia que juntar");
        let v: serde_json::Value = serde_json::from_str(&junto).unwrap();
        let els = v["elements"].as_array().unwrap();
        let ids: Vec<&str> = els.iter().map(|e| e["id"].as_str().unwrap()).collect();
        assert_eq!(
            ids,
            ["Xa9", "pc1", "pc1-antes1", "pc2"],
            "lo del movil primero y ninguno pisado"
        );
        assert_eq!(
            els[2]["x"].as_f64(),
            Some(266.0),
            "corrido al borde de la pagina"
        );
        assert_eq!(els[3]["x"].as_f64(), Some(286.0));
        assert_eq!(els[2]["points"].as_array().unwrap().len(), 3);
        assert!(
            v.get("files").is_some(),
            "lo demas del lienzo del movil se queda"
        );
    }

    #[test]
    fn lo_ya_copiado_movido_o_que_dio_la_vuelta_por_el_movil_no_se_repite() {
        // Copiado antes (sin marca) y vuelto del movil con los puntos como
        // objetos y la x movida: ya esta, no se anade nada.
        let copiado = r#"{"elements":[
          {"id":"pc1","type":"freedraw","x":99,"y":40,"width":9,"height":2,"points":[{"x":0.0,"y":0.0},{"x":2.0,"y":2.0},{"x":9.0,"y":1.0}]},
          {"id":"pc2","type":"freedraw","x":30,"y":90,"width":5,"height":5,"isDeleted":true,"points":[{"x":0.0,"y":0.0},{"x":5.0,"y":5.0}]}]}"#;
        assert_eq!(juntar_tinta(DE_ANTES, copiado, 0.0), None);
        // Casos negativos: lo que no se entiende no se toca.
        assert_eq!(juntar_tinta("no es json", copiado, 0.0), None);
        assert_eq!(juntar_tinta(DE_ANTES, "{}", 0.0), None);
    }

    #[test]
    fn la_tinta_de_antes_se_pasa_una_sola_vez() {
        let r = raiz("pasar");
        let viejo = r.join("doc.docx.pixpin-anotado/capa.excalidraw");
        let nuevo = r.join("lienzos/anot-ABCDE23456.excalidraw");
        std::fs::create_dir_all(viejo.parent().unwrap()).unwrap();
        std::fs::create_dir_all(nuevo.parent().unwrap()).unwrap();
        std::fs::write(&viejo, DE_ANTES).unwrap();
        // Sin el nuevo no hace nada (eso es copiar, con su corrida).
        assert!(!pasar_tinta_de_antes(&viejo, &nuevo, 0.0));
        std::fs::write(&nuevo, r#"{"elements":[]}"#).unwrap();
        assert!(pasar_tinta_de_antes(&viejo, &nuevo, 0.0));
        let dos = leer(&nuevo).unwrap();
        // Borrado despues en el lector: la segunda vez no vuelve.
        std::fs::write(&nuevo, r#"{"elements":[]}"#).unwrap();
        assert!(!pasar_tinta_de_antes(&viejo, &nuevo, 0.0));
        assert_eq!(leer(&nuevo).as_deref(), Some(r#"{"elements":[]}"#));
        assert!(dos.contains("pc2"));
        // Tras copiar tambien se marca: no se junta encima de lo copiado.
        let otro = r.join("b.pixpin-anotado/hoja-1.excalidraw");
        std::fs::create_dir_all(otro.parent().unwrap()).unwrap();
        std::fs::write(&otro, DE_ANTES).unwrap();
        marcar_tinta_pasada(&otro);
        std::fs::write(&nuevo, r#"{"elements":[]}"#).unwrap();
        assert!(!pasar_tinta_de_antes(&otro, &nuevo, 0.0));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_pasada_pone_a_cada_tinta_sin_marco_el_suyo_en_dos_lineas_y_no_toca_la_tinta() {
        let r = raiz("pasada");
        let (_, doc, _) = con_un_word(&r);
        let b = base_del_documento(&r, &doc).unwrap();
        std::fs::create_dir_all(b.tinta().parent().unwrap()).unwrap();
        let tinta = r#"{"elements":[{"id":"m1","type":"freedraw","x":300,"y":10,"width":4,"height":4,"points":[{"x":0.0,"y":0.0},{"x":4.0,"y":4.0}]}],"files":{}}"#;
        std::fs::write(b.tinta(), tinta).unwrap();
        // Lo de un PDF leido como texto: el PC no lo abre, no se inventa.
        let texto = Base {
            base: format!("{}-texto", b.base),
            ..b.clone()
        };
        std::fs::write(texto.tinta(), tinta).unwrap();
        let regla_vieja = a::MarcoDeLaHoja::nuevo(256.0, 0.0, 640.0, 384.0);
        let mut vistos = Vec::new();
        let p = poner_marcos(&r, &mut |t| {
            vistos.push((t.que, t.doc.clone()));
            Some(regla_vieja)
        });
        assert_eq!(
            (p.escritos, p.sin_calcular, p.ya_estaban),
            (1, 1, 0),
            "{p:?}"
        );
        assert_eq!(vistos, vec![(QueTinta::Documento, Some(doc.clone()))]);
        // Dos lineas, como Android: una tercera la ignoraria.
        assert_eq!(leer(&b.marco()).as_deref(), Some("256,0,640,384\nv1\n"));
        assert_eq!(
            std::fs::read_to_string(b.tinta()).unwrap(),
            tinta,
            "la tinta, byte a byte"
        );
        assert!(!texto.marco().exists());
        // Otra vez: nada que hacer, ni la fecha del marco cambia.
        let fecha = std::fs::metadata(b.marco()).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let p = poner_marcos(&r, &mut |_| panic!("ya tiene marco"));
        assert_eq!((p.escritos, p.sin_huella, p.ya_estaban), (0, 0, 1));
        assert_eq!(
            std::fs::metadata(b.marco()).unwrap().modified().unwrap(),
            fecha
        );
        // Uno con la huella del PC del 29-sep, de esta tinta: pasa a dos
        // lineas con sus mismos numeros.
        let h = a::huella_de_tinta(tinta);
        escribir(
            &b.marco(),
            &format!("-1050.0,0.0,2450.0,4950.0\nv1\ntinta {h}\n"),
        )
        .unwrap();
        let p = poner_marcos(&r, &mut |_| panic!("tiene marco que vale"));
        assert_eq!(p.sin_huella, 1);
        assert_eq!(
            leer(&b.marco()).as_deref(),
            Some("-1050.0,0.0,2450.0,4950.0\nv1\n")
        );
        // Caso negativo: con la huella de otra tinta miente, y se calcula.
        escribir(
            &b.marco(),
            "-1050,0,2450,4950\nv1\ntinta 0123456789abcdef\n",
        )
        .unwrap();
        let p = poner_marcos(&r, &mut |_| Some(regla_vieja));
        assert_eq!((p.escritos, p.sin_huella), (1, 0), "{p:?}");
        assert_eq!(leer(&b.marco()).as_deref(), Some("256,0,640,384\nv1\n"));
        // Casos negativos: uno que no se entiende no se toca, y si no se
        // puede calcular no se escribe nada.
        escribir(&b.marco(), "0,0,1,1\nv7\n").unwrap();
        let p = poner_marcos(&r, &mut |_| None);
        assert_eq!(p.sin_calcular, 2);
        assert_eq!(leer(&b.marco()).as_deref(), Some("0,0,1,1\nv7\n"));
        std::fs::remove_file(b.marco()).unwrap();
        assert_eq!(poner_marcos(&r, &mut |_| None).escritos, 0);
        assert!(!b.marco().exists());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_huella_es_la_del_resumen_con_que_se_sincroniza_la_tinta() {
        let r = raiz("huella");
        let (ficha, doc, _) = con_un_word(&r);
        let b = base_del_documento(&r, &doc).unwrap();
        assert_eq!(b.resumen_de_la_tinta(), None, "sin tinta no hay huella");
        std::fs::create_dir_all(b.tinta().parent().unwrap()).unwrap();
        std::fs::write(b.tinta(), r#"{"elements":[],"files":{}}"#).unwrap();
        let chat = vista::chat_de_ficha(&r, &ficha.id).unwrap();
        let d = DiscoPc::nuevo(&r);
        let rel = a::rel(&b.base, ".excalidraw.gz");
        assert_eq!(
            b.resumen_de_la_tinta(),
            d.resumen_de_archivo(&chat, &rel).ok()
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn escribir_lo_mismo_no_cambia_la_fecha_del_fichero() {
        let r = raiz("fecha");
        let f = r.join("a.sitio");
        escribir(&f, "0.5").unwrap();
        let antes = std::fs::metadata(&f).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        escribir(&f, "0.5").unwrap();
        assert_eq!(std::fs::metadata(&f).unwrap().modified().unwrap(), antes);
        escribir(&f, "0.6").unwrap();
        assert_eq!(leer(&f).as_deref(), Some("0.6"));
        assert!(!r.join("a.sitio.tmp").exists());
        let _ = std::fs::remove_dir_all(&r);
    }
}
