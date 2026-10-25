//! **Un lienzo suelto por Wi-Fi, como lo manda y lo recibe el movil**
//! (`recibir/lienzo_suelto.rs` de Windows, tal cual salvo lo de Windows).
//!
//! El usuario (26-sep-2026): «cuando paso un canvas por wifi de mi laptop a
//! mi celular aparece con la extension excalidraw y no la puedo abrir en el
//! canvas». La causa: se mandaba el fichero del lienzo
//! (`lienzos/<id>.excalidraw`) como un ARCHIVO cualquiera, y el movil guarda
//! los archivos en la conversacion general como adjunto, sin abrirlos.
//!
//! El movil manda un lienzo de otra manera (`EnviarActivity.deLienzo`):
//! tipo `lienzo`, dentro un `.pixpin` con UN proyecto de UNA hoja, la
//! identidad `lienzo:<proyecto>:<hoja>` y el proyecto del que es, para que
//! al otro lado caiga en el mismo (`Recepcion.guardarLienzo`). Aqui se hace
//! lo mismo en los dos sentidos:
//!
//! - [`empaquetar`]: lo que sale del Mac, con la forma de `deLienzo`.
//! - [`guardar`]: lo que llega, a su proyecto o a uno nuevo, en vez de a un
//!   `.excalidraw` que por dentro era un ZIP.
//!
//! **El JSON de dentro tambien tiene que ser el del movil.** Alli el lienzo
//! del paquete se lee como su `Scene` con kotlinx (sin
//! `coerceInputValues`), y un solo campo que no encaje tira la escena
//! ENTERA. Los lienzos nacidos en el PC y en el Mac llevan los puntos como
//! `[x, y]` y el movil los quiere como `{"x":…,"y":…}`.
//! [`escena_para_el_movil`] lo deja como lo espera, con las reglas de
//! `pixpin_proyecto::para_el_movil` (las mismas con que sale un lienzo al
//! sincronizar).

use std::path::{Path, PathBuf};

use pixpin_proyecto::{Hoja, Paquete, Proyecto, almacen, cuaderno, para_el_movil};
use pixpin_sincro::envio::{self, Elemento};
use serde_json::{Map, Value};

/// Lo que se hizo al pasar un lienzo a la forma del movil.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Informe {
    /// Elementos que el movil no sabe leer y se quedaron fuera: con ellos
    /// dentro no se abriria nada.
    pub quitados: usize,
    /// Fotos de `files` cuyo fichero no se encontro.
    pub fotos_perdidas: usize,
}

/// El lienzo como lo lee el `Scene` del movil, y las fotos que van en
/// `imagenes/<id>` del paquete.
///
/// `foto` da los bytes de una entrada de `files` (por su `path` o su
/// `dataURL`); sin bytes la entrada se quita, porque un `SceneFile` sin
/// `path` ni `mimeType` tambien tira la escena.
// Las firmas del cierre y de la vuelta se leen mejor en linea que tras un alias.
#[allow(clippy::type_complexity)]
pub fn escena_para_el_movil(
    json: &str,
    foto: &dyn Fn(&str, &Map<String, Value>) -> Option<Vec<u8>>,
) -> Result<(String, Vec<(String, Vec<u8>)>, Informe), String> {
    let mut raiz: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let Value::Object(mapa) = &mut raiz else {
        return Err("el lienzo no es un objeto".into());
    };
    let Some(Value::Array(elementos)) = mapa.remove("elements") else {
        return Err("el lienzo no trae «elements»".into());
    };
    let mut informe = Informe::default();
    let antes = elementos.len();
    let elementos: Vec<Value> = elementos
        .into_iter()
        .filter_map(para_el_movil::elemento)
        .collect();
    informe.quitados = antes - elementos.len();
    mapa.insert("elements".into(), Value::Array(elementos));

    // El papel: el `Scene` lo lleva arriba (`backgroundColor`); el PC, como
    // Excalidraw, en `appState.viewBackgroundColor`.
    if !mapa.get("backgroundColor").is_some_and(Value::is_string)
        && let Some(c) = mapa
            .get("appState")
            .and_then(|a| a.get("viewBackgroundColor"))
            .and_then(Value::as_str)
            .filter(|c| *c != "transparent")
    {
        mapa.insert("backgroundColor".into(), Value::String(c.to_string()));
    }
    if let Some(p) = mapa.get_mut("origenCoordenadas") {
        para_el_movil::a_punto(p);
    }

    let mut fotos = Vec::new();
    let files = match mapa.remove("files") {
        Some(Value::Object(f)) => f,
        _ => Map::new(),
    };
    let mut nuevos = Map::new();
    for (id, entrada) in files {
        let Value::Object(e) = entrada else {
            informe.fotos_perdidas += 1;
            continue;
        };
        // Un id raro no puede escribir fuera de `imagenes/`.
        if id.is_empty() || id.contains(['/', '\\']) || id.contains("..") {
            informe.fotos_perdidas += 1;
            continue;
        }
        let Some(bytes) = foto(&id, &e) else {
            informe.fotos_perdidas += 1;
            continue;
        };
        let mime = e
            .get("mimeType")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| mime_de_bytes(&bytes).to_string());
        let creado = e.get("created").and_then(Value::as_f64).unwrap_or(0.0) as i64;
        nuevos.insert(
            id.clone(),
            serde_json::json!({
                "id": id,
                "mimeType": mime,
                "path": format!("imagenes/{id}"),
                "created": creado,
            }),
        );
        fotos.push((id, bytes));
    }
    mapa.insert("files".into(), Value::Object(nuevos));
    let texto = serde_json::to_string(&raiz).map_err(|e| e.to_string())?;
    Ok((texto, fotos, informe))
}

fn mime_de_bytes(b: &[u8]) -> &'static str {
    if b.starts_with(b"\x89PNG") {
        "image/png"
    } else if b.starts_with(b"\xff\xd8") {
        "image/jpeg"
    } else if b.starts_with(b"GIF8") {
        "image/gif"
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/png"
    }
}

/// Los bytes de un `data:…;base64,…`.
fn de_data_url(url: &str) -> Option<Vec<u8>> {
    let (_, b64) = url.split_once("base64,")?;
    let mut salida = Vec::with_capacity(b64.len() * 3 / 4);
    let mut acumulado: u32 = 0;
    let mut bits = 0;
    for c in b64.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        } as u32;
        acumulado = (acumulado << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            salida.push((acumulado >> bits) as u8);
            acumulado &= (1 << bits) - 1;
        }
    }
    Some(salida)
}

// ------------------------------------------------------------ enviar

/// Si una ruta es un lienzo que se manda como lienzo y no como archivo.
pub fn es_lienzo(ruta: &Path) -> bool {
    ruta.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("excalidraw"))
}

/// De que proyecto de la lista es un fichero `proyectos/<id>/lienzos/<d>.excalidraw`.
fn proyecto_y_dibujo(raiz: &Path, ruta: &Path) -> Option<(String, String)> {
    let dentro = ruta.strip_prefix(raiz.join("proyectos")).ok()?;
    let partes: Vec<String> = dentro
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    match partes.as_slice() {
        [p, l, f] if l == "lienzos" => {
            let d = f.strip_suffix(".excalidraw")?;
            Some((p.clone(), d.to_string()))
        }
        _ => None,
    }
}

/// El nombre que se ensena de una hoja: sin el `.excalidraw` que el PC le
/// pone al mensaje, y en blanco si solo es su id (el movil pone «Lienzo»).
fn nombre_de_hoja(nombre: &str, dibujo: &str) -> String {
    let n = nombre.trim();
    let n = n.strip_suffix(".excalidraw").unwrap_or(n).trim();
    if n == dibujo {
        String::new()
    } else {
        n.to_string()
    }
}

/// Un lienzo del PC como lo manda el movil (`EnviarActivity.deLienzo`):
/// el elemento de la oferta y los bytes del `.pixpin` de una hoja.
pub fn empaquetar(raiz: &Path, ruta: &Path, ahora: i64) -> Result<(Elemento, Vec<u8>), String> {
    let json = std::fs::read_to_string(ruta).map_err(|e| e.to_string())?;
    let en_proyecto = proyecto_y_dibujo(raiz, ruta);

    let (mut proyecto, hoja, mensaje, fotos_desde): (Proyecto, Hoja, Option<cuaderno::Mensaje>, _) =
        match &en_proyecto {
            Some((pid, dibujo)) => {
                let carpeta = almacen::carpeta(raiz, pid);
                let ficha = almacen::Indice::leer(raiz).buscar(pid).cloned();
                let base: Proyecto = std::fs::read(carpeta.join("proyecto.json"))
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok())
                    .unwrap_or_else(|| Proyecto {
                        id: pid.clone(),
                        ..Default::default()
                    });
                let mensaje = cuaderno::Cuaderno::leer_de(&carpeta).ok().and_then(|c| {
                    c.mensajes.into_iter().find(|m| {
                        m.clase == Some(cuaderno::Clase::Dibujo)
                            && (m.referencia.as_deref() == Some(dibujo)
                                || m.ruta.as_deref()
                                    == Some(&format!("lienzos/{dibujo}.excalidraw")))
                    })
                });
                let hoja = base
                    .hojas
                    .iter()
                    .find(|h| h.dibujo.as_deref() == Some(dibujo))
                    .cloned()
                    .unwrap_or_else(|| Hoja {
                        id: dibujo.clone(),
                        nombre: mensaje
                            .as_ref()
                            .map(|m| nombre_de_hoja(&m.nombre, dibujo))
                            .unwrap_or_default(),
                        dibujo: Some(dibujo.clone()),
                        ..Default::default()
                    });
                let mut p = base;
                if let Some(f) = &ficha {
                    p.nombre = f.nombre.clone();
                    p.uid = f.uid.clone().or(p.uid);
                    if f.creado > 0 {
                        p.creado = f.creado;
                    }
                    p.aparato = f.aparato.clone().or(p.aparato);
                    p.tocado = p.tocado.max(f.tocado);
                }
                let aparato = ficha.as_ref().and_then(|f| f.aparato.clone());
                p.sellar(aparato.as_deref());
                (p, hoja, mensaje, Some(pid.clone()))
            }
            None => {
                // Un `.excalidraw` suelto (el de «Compartir», uno de fuera):
                // al otro lado sera un proyecto nuevo con su nombre.
                let nombre = ruta
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Lienzo".into());
                let id = pixpin_proyecto::codigos::nuevo();
                let dibujo = pixpin_proyecto::codigos::nuevo();
                let mut p = Proyecto {
                    id,
                    nombre: nombre.clone(),
                    tocado: ahora,
                    creado: ahora,
                    ..Default::default()
                };
                p.sellar(None);
                let hoja = Hoja {
                    id: dibujo.clone(),
                    nombre,
                    dibujo: Some(dibujo),
                    ..Default::default()
                };
                (p, hoja, None, None)
            }
        };

    let dibujo = hoja.dibujo.clone().unwrap_or_else(|| hoja.id.clone());
    let al_lado = ruta.parent().map(Path::to_path_buf).unwrap_or_default();
    let foto = |_: &str, e: &Map<String, Value>| -> Option<Vec<u8>> {
        if let Some(url) = e.get("dataURL").and_then(Value::as_str) {
            return de_data_url(url);
        }
        let rel = e.get("path").and_then(Value::as_str)?;
        let real = match &fotos_desde {
            Some(pid) => pixpin_proyecto::vista::ruta_real(raiz, pid, rel)?,
            None => {
                if Path::new(rel).is_absolute() || rel.contains("..") || rel.contains(':') {
                    return None;
                }
                al_lado.join(rel)
            }
        };
        std::fs::read(real).ok()
    };
    let (escena, fotos, informe) = escena_para_el_movil(&json, &foto)?;
    // Lo que no viaja (un tipo que el movil no conoce, una foto sin
    // fichero) se queda fuera: con ello dentro no se abriria nada alli.
    let _ = informe;

    // Una pagina de un PDF necesita su PDF, como en `deLienzo`.
    let pdf = match (&fotos_desde, hoja.pagina) {
        (Some(pid), Some(_)) => pixpin_proyecto::vista::documento_del_proyecto(raiz, pid)
            .and_then(|d| std::fs::read(d).ok()),
        _ => None,
    };
    let mut hoja = hoja;
    hoja.dibujo = Some(dibujo.clone());
    if hoja.uid.is_none() {
        hoja.uid = Some(hoja.codigo_unico());
    }
    proyecto.hojas = vec![hoja.clone()];
    proyecto.croquis = Vec::new();
    proyecto.pdf_origen = None;
    proyecto.pdf_limpio = None;

    let manifiesto = pixpin_proyecto::Manifiesto {
        escrito: ahora,
        proyecto: proyecto.nombre.clone(),
        ..Default::default()
    };
    let mut paquete = Paquete::nuevo(manifiesto, proyecto.clone());
    paquete.poner_entrada(&format!("lienzos/{dibujo}.excalidraw"), escena.into_bytes());
    for (id, bytes) in fotos {
        paquete.poner_entrada(&format!("imagenes/{id}"), bytes);
    }
    if let Some(b) = pdf {
        paquete.poner_entrada("documento.pdf", b);
    }
    let bytes = paquete.a_bytes().map_err(|e| e.to_string())?;

    // La identidad, la de `Recepcion.identidadDe`: el origen si vino de
    // otro sitio, su id si nacio aqui. Igual para la hoja.
    let suya_p = proyecto
        .resto
        .get("origen")
        .and_then(Value::as_str)
        .unwrap_or(&proyecto.id)
        .to_string();
    let suya_h = hoja
        .resto
        .get("origen")
        .and_then(Value::as_str)
        .unwrap_or(&hoja.id)
        .to_string();
    let nombre = if hoja.nombre.trim().is_empty() {
        match hoja.pagina {
            Some(n) => format!("Página {}", n + 1),
            None => "Lienzo".to_string(),
        }
    } else {
        hoja.nombre.clone()
    };
    Ok((
        Elemento {
            tipo: envio::LIENZO.into(),
            nombre,
            bytes: bytes.len() as i64,
            mime: Some("application/zip".into()),
            identidad: format!("lienzo:{suya_p}:{suya_h}"),
            proyecto: Some(format!("proyecto:{suya_p}")),
            proyecto_nombre: Some(proyecto.nombre.clone()),
            creado: mensaje.as_ref().map(|m| m.cuando).unwrap_or(0),
            uid: hoja.uid.clone(),
            codigo_de_chat: mensaje.as_ref().and_then(|m| m.codigo_chat()),
            aparato: None,
        },
        bytes,
    ))
}

/// El nombre del `.pixpin` que sale, el de `deLienzo`: «Proyecto - Hoja».
pub fn nombre_del_paquete(e: &Elemento) -> String {
    let proyecto = e.proyecto_nombre.as_deref().unwrap_or("Proyecto");
    format!(
        "{}.pixpin",
        envio::nombre_sano(&format!("{proyecto} - {}", e.nombre))
    )
}

/// Deja el paquete de un lienzo en `carpeta` y devuelve lo que se ofrece.
pub fn preparar_envio(
    raiz: &Path,
    ruta: &Path,
    carpeta: &Path,
    ahora: i64,
) -> Result<(Elemento, PathBuf), String> {
    let (e, bytes) = empaquetar(raiz, ruta, ahora)?;
    // Uno por carpeta, como `l-<hora>` del movil: dos lienzos que se llamen
    // igual no se pisan.
    let carpeta = carpeta.join(pixpin_proyecto::codigos::nuevo());
    std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    let destino = carpeta.join(nombre_del_paquete(&e));
    std::fs::write(&destino, bytes).map_err(|e| e.to_string())?;
    Ok((e, destino))
}

// ------------------------------------------------------------ recibir

/// `lienzo:<proyecto>:<hoja>` → (proyecto, hoja), como `partesDelLienzo`.
fn partes(identidad: &str) -> Option<(String, String)> {
    let mut p = identidad.splitn(3, ':');
    match (p.next(), p.next(), p.next()) {
        (Some("lienzo"), Some(a), Some(b)) => Some((a.to_string(), b.to_string())),
        _ => None,
    }
}

/// El proyecto de aqui que es ese del otro lado: su `proyecto.json` dice
/// ese id o ese origen, o la ficha tiene ese id.
fn proyecto_de_aqui(raiz: &Path, suya: &str) -> Option<String> {
    let indice = almacen::Indice::leer(raiz);
    indice
        .proyectos
        .iter()
        .find(|f| {
            if f.id == suya {
                return true;
            }
            std::fs::read(almacen::carpeta(raiz, &f.id).join("proyecto.json"))
                .ok()
                .and_then(|b| serde_json::from_slice::<Proyecto>(&b).ok())
                .is_some_and(|p| {
                    p.id == suya || p.resto.get("origen").and_then(Value::as_str) == Some(suya)
                })
        })
        .map(|f| f.id.clone())
}

/// El papel que el movil escribe arriba (`backgroundColor`), tambien donde lo
/// lee el PC (`appState.viewBackgroundColor`).
fn escena_para_el_pc(json: &[u8]) -> Vec<u8> {
    let Ok(Value::Object(mut m)) = serde_json::from_slice::<Value>(json) else {
        return json.to_vec();
    };
    let Some(papel) = m
        .get("backgroundColor")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return json.to_vec();
    };
    let estado = m
        .entry("appState")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(a) = estado
        && !a.contains_key("viewBackgroundColor")
    {
        a.insert("viewBackgroundColor".into(), Value::String(papel));
    }
    serde_json::to_vec(&Value::Object(m)).unwrap_or_else(|_| json.to_vec())
}

/// Lo que paso con un lienzo que llego.
#[derive(Debug, PartialEq, Eq)]
pub enum Llegada {
    /// Se creo un proyecto nuevo con el (no se tenia el suyo).
    ProyectoNuevo(String),
    /// Se anadio al proyecto que ya se tenia.
    Anadido(String),
    /// Ya estaba y se puso al dia en su sitio.
    AlDia(String),
}

/// **Un lienzo que llega, a su proyecto** (`Recepcion.guardarLienzo`): si ya
/// se tiene ese proyecto, el lienzo se anade o pone al dia el mismo; si no,
/// se crea el proyecto con su nombre y solo ese lienzo.
///
/// `antes_de_tocar` hace la copia de seguridad del proyecto que se va a
/// tocar; si falla, no se escribe.
pub fn guardar(
    raiz: &Path,
    e: &Elemento,
    ruta: &Path,
    aparato: &str,
    como_nuevo: bool,
    ahora: i64,
    antes_de_tocar: &dyn Fn(&str) -> std::io::Result<()>,
) -> std::io::Result<Llegada> {
    let paquete = Paquete::abrir(ruta).map_err(std::io::Error::other)?;
    let hoja = paquete
        .proyecto
        .hojas
        .first()
        .cloned()
        .ok_or_else(|| std::io::Error::other("el lienzo llego sin hoja"))?;
    let suya_p = partes(&e.identidad)
        .map(|(p, _)| p)
        .unwrap_or_else(|| paquete.proyecto.id.clone());
    let Some(fid) = (!como_nuevo)
        .then(|| proyecto_de_aqui(raiz, &suya_p))
        .flatten()
    else {
        let mut p = if como_nuevo {
            almacen::renovar(&paquete, ahora)
        } else {
            paquete
        };
        if let Some(n) = e
            .proyecto_nombre
            .as_deref()
            .filter(|n| !n.trim().is_empty())
        {
            p.proyecto.nombre = n.to_string();
        }
        // El papel donde lo lee el PC, en cada lienzo del paquete.
        let lienzos: Vec<String> = p
            .nombres()
            .filter(|n| n.starts_with("lienzos/"))
            .map(str::to_string)
            .collect();
        for n in lienzos {
            if let Some(b) = p.entrada(&n).map(escena_para_el_pc) {
                p.poner_entrada(&n, b);
            }
        }
        let ficha = almacen::importar_paquete(raiz, &p, aparato)?;
        return Ok(Llegada::ProyectoNuevo(ficha.id));
    };

    antes_de_tocar(&fid)?;
    let carpeta = almacen::carpeta(raiz, &fid);
    let dibujo_suyo = hoja
        .dibujo
        .clone()
        .ok_or_else(|| std::io::Error::other("la hoja que llego no trae lienzo"))?;
    let lienzo = paquete
        .entrada(&format!("lienzos/{dibujo_suyo}.excalidraw"))
        .ok_or_else(|| std::io::Error::other("falta el lienzo dentro del paquete"))?;
    let mut cuaderno_de_aqui = cuaderno::Cuaderno::leer_de(&carpeta).unwrap_or_default();
    // **El mismo lienzo que ya llego antes**: por su identidad o su codigo
    // unico. Ese se escribe en su sitio y el chat no cambia de burbuja.
    let repetido = cuaderno_de_aqui
        .mensajes
        .iter()
        .find(|m| {
            m.clase == Some(cuaderno::Clase::Dibujo)
                && (m.origen.as_deref() == Some(e.identidad.as_str())
                    || (e.uid.is_some() && m.uid == e.uid))
        })
        .cloned();
    let dibujo = match repetido.as_ref().and_then(|m| m.referencia.clone()) {
        Some(d) => d,
        None => {
            let libre = !almacen::lienzo(raiz, &fid, &dibujo_suyo).exists();
            if libre {
                dibujo_suyo.clone()
            } else {
                format!("{dibujo_suyo}-{ahora}")
            }
        }
    };
    let destino = almacen::lienzo(raiz, &fid, &dibujo);
    if let Some(padre) = destino.parent() {
        std::fs::create_dir_all(padre)?;
    }
    let lienzo = escena_para_el_pc(lienzo);
    std::fs::write(&destino, &lienzo)?;
    for n in paquete.nombres() {
        let Some(id) = n.strip_prefix("imagenes/") else {
            continue;
        };
        if id.is_empty() || id.contains(['/', '\\']) || id.contains("..") {
            continue;
        }
        let r = carpeta.join("imagenes").join(id);
        if !r.exists()
            && let Some(b) = paquete.entrada(n)
        {
            std::fs::create_dir_all(carpeta.join("imagenes"))?;
            std::fs::write(r, b)?;
        }
    }
    // El `proyecto.json`, si lo tiene, con la hoja: es lo que se manda
    // despues entero y lo que lee el movil al sincronizar.
    let pj = carpeta.join("proyecto.json");
    if let Some(mut p) = std::fs::read(&pj)
        .ok()
        .and_then(|b| serde_json::from_slice::<Proyecto>(&b).ok())
    {
        let mut h = hoja.clone();
        h.dibujo = Some(dibujo.clone());
        match p
            .hojas
            .iter_mut()
            .find(|x| x.dibujo.as_deref() == Some(dibujo.as_str()))
        {
            Some(x) => {
                *x = Hoja {
                    id: x.id.clone(),
                    ..h
                }
            }
            None => {
                if p.hojas.iter().any(|x| x.id == h.id) {
                    h.id = format!("{}-{ahora}", h.id);
                }
                p.hojas.push(h);
            }
        }
        p.tocado = ahora;
        if let Ok(t) = serde_json::to_vec_pretty(&p) {
            std::fs::write(&pj, t)?;
        }
    }
    let nombre = if hoja.nombre.trim().is_empty() {
        e.nombre.clone()
    } else {
        hoja.nombre.clone()
    };
    let relativa = format!("lienzos/{dibujo}.excalidraw");
    let llegada = match repetido {
        Some(m) => {
            let puesto = cuaderno::Mensaje {
                ruta: Some(relativa),
                bytes: lienzo.len() as i64,
                ..m
            };
            cuaderno::reemplazar(&carpeta, &puesto)?;
            Llegada::AlDia(fid.clone())
        }
        None => {
            let numero = cuaderno_de_aqui
                .mensajes
                .iter()
                .map(|m| m.numero)
                .max()
                .unwrap_or(0)
                + 1;
            let mut m = cuaderno::Mensaje::adjunto(
                cuaderno::Clase::Dibujo,
                &nombre,
                &relativa,
                lienzo.len() as i64,
                &cuaderno::Sello {
                    cuando: if e.creado > 0 { e.creado } else { ahora },
                    numero,
                    aparato: aparato.to_string(),
                    proyecto: fid.clone(),
                },
            );
            m.referencia = Some(dibujo);
            m.origen = Some(e.identidad.clone());
            if e.uid.is_some() {
                m.uid = e.uid.clone();
            }
            cuaderno::anadir(&carpeta, &m)?;
            cuaderno_de_aqui.mensajes.push(m);
            Llegada::Anadido(fid.clone())
        }
    };
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == fid) {
        f.tocado = ahora;
        let _ = indice.guardar(raiz);
    }
    Ok(llegada)
}

#[cfg(test)]
#[path = "lienzo_suelto_pruebas.rs"]
mod pruebas;
