//! Copias de seguridad de un chat antes de que nada lo pise.
//!
//! Puerto de `Copias.hacer` de `sincro/Copias.kt`: antes de sincronizar se
//! guarda como estaba el proyecto, sus mensajes y sus archivos, cada archivo
//! una vez por su contenido. Si nada cambio desde la ultima copia no se hace
//! otra, y se quedan las [POR_PROYECTO] mas nuevas. Viven en `copias/`, fuera
//! de lo que viaja: son de este aparato.
//!
//! **Volver a una copia no borra lo nuevo**: se ponen otra vez el proyecto,
//! los archivos y los mensajes de entonces, y lo que se escribio despues y no
//! estaba en la copia se queda. Y antes de volver se hace otra copia, asi que
//! volver tambien se deshace. Los ficheros llevan el mismo formato que en el
//! movil.

use std::collections::HashSet;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::canonico::Json;
use crate::disco::{Cambio, Disco, escribir_atomico, limpio, sha256_de};
use crate::kotlin;

pub const CARPETA: &str = "copias";
pub const POR_PROYECTO: usize = 30;
/// Lo que pase de esto **no se copia**, como en el movil: treinta copias de
/// un PDF de trescientos megas llenarian el disco.
///
/// Dicho sin adornos, porque es una renuncia y no una optimizacion: **de un
/// fichero asi no se puede volver atras**. La copia apunta su ruta en
/// `sin_copiar` para que la pantalla de copias lo diga, y la razon por la que
/// se aguanta es que nadie los reescribe: el PDF de un proyecto se guarda una
/// vez y lo que se dibuja encima va en otro fichero. Si algun dia algo los
/// pisara, esto tendria que cambiar antes.
pub const TOPE_POR_ARCHIVO: u64 = 40 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Copia {
    pub id: String,
    pub chat: String,
    pub cuando: i64,
    pub motivo: String,
    pub nombre: String,
    pub hojas: usize,
    pub proyecto: Option<String>,
    pub mensajes: Vec<String>,
    pub archivos: std::collections::BTreeMap<String, String>,
    pub sin_copiar: Vec<String>,
}

fn raiz<D: Disco + ?Sized>(d: &D) -> PathBuf {
    d.raiz().join(CARPETA)
}

fn carpeta_de<D: Disco + ?Sized>(d: &D, chat: &str) -> PathBuf {
    raiz(d).join("proyectos").join(limpio(chat))
}

/// Las copias de un chat, la mas nueva primero.
pub fn lista<D: Disco + ?Sized>(d: &D, chat: &str) -> Vec<Copia> {
    let mut v: Vec<Copia> = std::fs::read_dir(carpeta_de(d, chat))
        .map(|l| {
            l.flatten()
                .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
                .filter_map(|e| std::fs::read_to_string(e.path()).ok())
                .filter_map(|t| serde_json::from_str(&t).ok())
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|c| std::cmp::Reverse(c.cuando));
    v
}

/// Las copias de todos los chats, la mas nueva primero (`Copias.todas`).
pub fn todas<D: Disco + ?Sized>(d: &D) -> Vec<Copia> {
    let mut v = Vec::new();
    if let Ok(l) = std::fs::read_dir(raiz(d).join("proyectos")) {
        for e in l.flatten().filter(|e| e.path().is_dir()) {
            v.extend(
                std::fs::read_dir(e.path())
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|f| f.file_name().to_string_lossy().ends_with(".json"))
                    .filter_map(|f| std::fs::read_to_string(f.path()).ok())
                    .filter_map(|t| serde_json::from_str::<Copia>(&t).ok()),
            );
        }
    }
    v.sort_by_key(|c| std::cmp::Reverse(c.cuando));
    v
}

/// Hace la copia. `Ok(false)` si no habia nada o era igual que la ultima.
pub fn hacer<D: Disco + ?Sized>(d: &D, chat: &str, motivo: &str, ahora: i64) -> io::Result<bool> {
    hacer_con(d, chat, motivo, ahora, &[])
}

/// Lo mismo, guardando ademas los ficheros de `ademas` aunque ya no sean del
/// chat: al volver a una copia se van a escribir encima, y sin esto no habria
/// como deshacerlo.
fn hacer_con<D: Disco + ?Sized>(
    d: &D,
    chat: &str,
    motivo: &str,
    ahora: i64,
    ademas: &[String],
) -> io::Result<bool> {
    let p = d.proyecto_portatil(chat)?;
    let mensajes: Vec<String> = d.mensajes(chat)?.iter().map(|m| m.a_texto()).collect();
    if p.is_none() && mensajes.is_empty() {
        return Ok(false);
    }
    let objetos = raiz(d).join("objetos");
    std::fs::create_dir_all(&objetos)?;
    let mut archivos = std::collections::BTreeMap::new();
    let mut grandes = Vec::new();
    let mut rutas: Vec<String> = d.alcance(chat)?.into_iter().map(|(rel, _)| rel).collect();
    for extra in ademas {
        if !rutas.contains(extra) {
            rutas.push(extra.clone());
        }
    }
    for rel in rutas {
        let f = d.ruta(chat, &rel);
        let Ok(meta) = std::fs::metadata(&f) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        if meta.len() > TOPE_POR_ARCHIVO {
            grandes.push(rel);
            continue;
        }
        let resumen = sha256_de(&mut std::fs::File::open(&f)?)?;
        let destino = objetos.join(&resumen);
        if !destino.exists() {
            escribir_atomico(&destino, &std::fs::read(&f)?)?;
        }
        archivos.insert(rel, resumen);
    }
    let copia = Copia {
        id: ahora.to_string(),
        chat: chat.into(),
        cuando: ahora,
        motivo: motivo.into(),
        nombre: p
            .as_ref()
            .and_then(|p| kotlin::cadena(p, "nombre"))
            .unwrap_or("Conversación")
            .into(),
        hojas: p
            .as_ref()
            .and_then(|p| {
                p.como_objeto()?
                    .obtener("hojas")?
                    .como_lista()
                    .map(<[_]>::len)
            })
            .unwrap_or(0),
        proyecto: p.as_ref().map(|p| p.a_texto()),
        mensajes,
        archivos,
        sin_copiar: grandes,
    };
    if let Some(u) = lista(d, chat).first()
        && u.proyecto == copia.proyecto
        && u.mensajes == copia.mensajes
        && u.archivos == copia.archivos
    {
        return Ok(false);
    }
    let carpeta = carpeta_de(d, chat);
    std::fs::create_dir_all(&carpeta)?;
    let mut nombre = format!("{}.json", copia.id);
    let mut n = 1;
    while carpeta.join(&nombre).exists() {
        nombre = format!("{}-{n}.json", copia.id);
        n += 1;
    }
    let puesta = Copia {
        id: nombre.trim_end_matches(".json").into(),
        ..copia
    };
    escribir_atomico(
        &carpeta.join(&nombre),
        serde_json::to_string(&puesta)
            .map_err(io::Error::other)?
            .as_bytes(),
    )?;
    podar(d, chat);
    Ok(true)
}

/// **Vuelve a la copia** (`Copias.restaurar` del movil).
///
/// Antes guarda como esta ahora, asi que volver tambien se deshace. Despues
/// pone en su sitio los archivos de la copia, devuelve los mensajes que
/// falten y deja el proyecto como estaba **conservando al final los lienzos
/// que se anadieron despues**: volver a una copia no es borrar.
pub fn restaurar<D: Disco + ?Sized>(d: &D, c: &Copia, ahora: i64) -> io::Result<()> {
    let extra: Vec<String> = c.archivos.keys().cloned().collect();
    hacer_con(d, &c.chat, &motivo_de_vuelta(&c.motivo), ahora, &extra)?;
    let objetos = raiz(d).join("objetos");
    for (rel, resumen) in &c.archivos {
        let origen = objetos.join(resumen);
        if !origen.is_file() {
            // El objeto se recogio (ninguna copia lo senalaba ya). Lo que
            // esta en disco se queda como esta: mejor eso que vaciarlo.
            continue;
        }
        let destino = d.ruta(&c.chat, rel);
        if let Some(p) = destino.parent() {
            std::fs::create_dir_all(p)?;
        }
        // Por un temporal y `reemplazar`: el fichero que habia no deja de
        // existir mientras se escribe el de la copia.
        let tmp = crate::disco::con_sufijo(&destino, ".restaurando");
        std::fs::copy(&origen, &tmp)?;
        crate::disco::reemplazar(&tmp, &destino)?;
    }
    // Los mensajes de entonces vuelven; los de despues siguen donde estan,
    // porque `aplicar_mensajes` pone y cambia, pero sin `borrar` no quita.
    d.aplicar_mensajes(&c.chat, &c.mensajes, &[], ahora)?;
    if let Some(texto) = &c.proyecto
        && let Ok(de_entonces) = Json::analizar(texto)
    {
        d.guardar_proyecto(&con_las_hojas_de_despues(
            de_entonces,
            d.proyecto_portatil(&c.chat)?,
            ahora,
        ))?;
    }
    d.avisar(Cambio::Archivos);
    Ok(())
}

/// «Antes de volver a la copia de antes de sincronizar con Tableta».
fn motivo_de_vuelta(motivo: &str) -> String {
    let mut cs = motivo.chars();
    let minuscula = match cs.next() {
        Some(c) => c.to_lowercase().collect::<String>() + cs.as_str(),
        None => String::new(),
    };
    format!("Antes de volver a la copia de {minuscula}")
}

/// El proyecto de la copia con las hojas que se anadieron despues pegadas al
/// final: volver atras no puede llevarse por delante lo que se hizo luego.
fn con_las_hojas_de_despues(de_entonces: Json, ahora_p: Option<Json>, ahora: i64) -> Json {
    let mut puesto = de_entonces;
    let suyas: Vec<Json> = puesto
        .como_objeto()
        .and_then(|o| o.obtener("hojas"))
        .and_then(Json::como_lista)
        .map(<[Json]>::to_vec)
        .unwrap_or_default();
    let ids: HashSet<String> = suyas
        .iter()
        .filter_map(|h| kotlin::cadena(h, "id").map(str::to_string))
        .collect();
    let despues: Vec<Json> = ahora_p
        .as_ref()
        .and_then(|p| p.como_objeto()?.obtener("hojas")?.como_lista())
        .map(|l| {
            l.iter()
                .filter(|h| !kotlin::cadena(h, "id").is_some_and(|i| ids.contains(i)))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let mut todas = suyas;
    todas.extend(despues);
    kotlin::poner(&mut puesto, "hojas", Json::Lista(todas));
    kotlin::poner(&mut puesto, "tocado", Json::numero(ahora));
    puesto
}

fn podar<D: Disco + ?Sized>(d: &D, chat: &str) {
    let sobran: Vec<Copia> = lista(d, chat).into_iter().skip(POR_PROYECTO).collect();
    if sobran.is_empty() {
        return;
    }
    for c in &sobran {
        let _ = std::fs::remove_file(carpeta_de(d, chat).join(format!("{}.json", c.id)));
    }
    // Los objetos que ya no senala ninguna copia de ningun chat.
    let mut vivos = HashSet::new();
    if let Ok(l) = std::fs::read_dir(raiz(d).join("proyectos")) {
        for e in l.flatten() {
            let chat = e.file_name().to_string_lossy().to_string();
            for c in lista(d, &chat) {
                vivos.extend(c.archivos.into_values());
            }
        }
    }
    if let Ok(l) = std::fs::read_dir(raiz(d).join("objetos")) {
        for e in l.flatten() {
            if !vivos.contains(&e.file_name().to_string_lossy().to_string()) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::disco_android::DiscoAndroid;
    use crate::disco_android::prueba::presentar;
    use serde_json::json;

    /// Un «aparato» con su carpeta, como en `de_verdad`.
    struct Aparato {
        dir: PathBuf,
        d: DiscoAndroid,
    }

    impl Drop for Aparato {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn montar(etiqueta: &str) -> Aparato {
        let dir =
            std::env::temp_dir().join(format!("pixpin-copias-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let d = DiscoAndroid::nuevo(dir.join("files"), "tel");
        presentar(&d, "id-tel", "Teléfono");
        Aparato { dir, d }
    }

    /// Un proyecto con sus hojas, sus lienzos en disco y un mensaje.
    fn poner(a: &Aparato, hojas: &[(&str, &str)], texto: &str) {
        let hs: Vec<serde_json::Value> = hojas
            .iter()
            .map(|(id, dibujo)| json!({"id": id, "nombre": id, "dibujo": dibujo, "uid": null}))
            .collect();
        a.d.escribir_proyectos(&[Json::de_valor(&json!({
            "id": "p1", "nombre": "Tesis", "hojas": hs, "tocado": 10,
        }))])
        .unwrap();
        for (_, dibujo) in hojas {
            let ruta = a.d.ruta("p1", &format!("pins/draw/{dibujo}.excalidraw.gz"));
            std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
            std::fs::write(&ruta, texto.as_bytes()).unwrap();
        }
    }

    fn mensaje(a: &Aparato, id: &str, texto: &str, cuando: i64) {
        a.d.anadir_mensaje(&Json::de_valor(&json!({
            "id": id, "cuando": cuando, "clase": "NOTA", "texto": texto,
            "proyecto": "p1", "numero": cuando, "letra": "a",
        })))
        .unwrap();
    }

    fn hojas_de(a: &Aparato) -> Vec<String> {
        a.d.proyecto_portatil("p1")
            .unwrap()
            .and_then(|p| {
                p.como_objeto()?
                    .obtener("hojas")?
                    .como_lista()
                    .map(<[Json]>::to_vec)
            })
            .unwrap_or_default()
            .iter()
            .filter_map(|h| kotlin::cadena(h, "id").map(str::to_string))
            .collect()
    }

    #[test]
    fn volver_a_una_copia_devuelve_el_lienzo_pisado_y_no_se_lleva_lo_de_despues() {
        // Es el caso de «Tesis», entero: se copia, luego algo pisa un lienzo
        // y quita otro, y volver atras tiene que devolver lo pisado sin
        // borrar el lienzo y el mensaje que se anadieron despues.
        let a = montar("volver");
        poner(&a, &[("h1", "d1"), ("h2", "d2")], "lo bueno");
        mensaje(&a, "m1", "el de antes", 100);
        assert!(hacer(&a.d, "p1", "Antes de sincronizar con Tableta", 1_000).unwrap());
        let copia = lista(&a.d, "p1").remove(0);

        // Y ahora lo de siempre: un envio pisa `d1`, se lleva `h2` por
        // delante y se anade una hoja nueva y un mensaje nuevo.
        poner(&a, &[("h1", "d1"), ("h3", "d3")], "lo pisado");
        mensaje(&a, "m2", "el de despues", 200);

        restaurar(&a.d, &copia, 2_000).unwrap();

        assert_eq!(
            std::fs::read_to_string(a.d.ruta("p1", "pins/draw/d1.excalidraw.gz")).unwrap(),
            "lo bueno",
            "el lienzo pisado vuelve a ser el de la copia"
        );
        let hs = hojas_de(&a);
        assert!(
            hs.contains(&"h2".to_string()),
            "la hoja que se perdio vuelve: {hs:?}"
        );
        assert!(
            hs.contains(&"h3".to_string()),
            "y la de despues se queda: volver no es borrar ({hs:?})"
        );
        let textos: Vec<String> =
            a.d.mensajes("p1")
                .unwrap()
                .iter()
                .filter_map(|m| kotlin::cadena(m, "texto").map(str::to_string))
                .collect();
        assert!(textos.contains(&"el de antes".to_string()), "{textos:?}");
        assert!(
            textos.contains(&"el de despues".to_string()),
            "los mensajes de despues no se borran: {textos:?}"
        );
    }

    #[test]
    fn volver_atras_tambien_se_deshace() {
        // Antes de volver se guarda como estaba: si la vuelta no era lo que
        // se queria, se vuelve de la vuelta.
        let a = montar("deshacer");
        poner(&a, &[("h1", "d1")], "version uno");
        mensaje(&a, "m1", "uno", 100);
        assert!(hacer(&a.d, "p1", "Antes de sincronizar con Tableta", 1_000).unwrap());
        let primera = lista(&a.d, "p1").remove(0);

        poner(&a, &[("h1", "d1")], "version dos");
        restaurar(&a.d, &primera, 2_000).unwrap();
        let despues = lista(&a.d, "p1");
        assert_eq!(despues.len(), 2, "volver dejo su propia copia");
        assert!(
            despues[0]
                .motivo
                .starts_with("Antes de volver a la copia de antes de sincronizar"),
            "{:?}",
            despues[0].motivo
        );

        restaurar(&a.d, &despues[0].clone(), 3_000).unwrap();
        assert_eq!(
            std::fs::read_to_string(a.d.ruta("p1", "pins/draw/d1.excalidraw.gz")).unwrap(),
            "version dos",
            "deshacer la vuelta devuelve lo que habia antes de volver"
        );
    }

    #[test]
    fn una_copia_de_un_chat_vacio_no_se_hace_y_dos_iguales_seguidas_tampoco() {
        // Casos negativos: sin nada que copiar no hay copia, y sincronizar
        // cinco veces seguidas no llena la lista de copias identicas.
        let a = montar("vacia");
        assert!(!hacer(&a.d, "p1", "Antes de nada", 1_000).unwrap());
        poner(&a, &[("h1", "d1")], "algo");
        assert!(hacer(&a.d, "p1", "Antes de sincronizar", 1_100).unwrap());
        assert!(
            !hacer(&a.d, "p1", "Antes de sincronizar", 1_200).unwrap(),
            "nada cambio: la de antes ya sirve"
        );
        assert_eq!(lista(&a.d, "p1").len(), 1);
    }

    #[test]
    fn todas_junta_las_de_todos_los_chats_con_la_mas_nueva_delante() {
        let a = montar("todas");
        poner(&a, &[("h1", "d1")], "algo");
        mensaje(&a, "m1", "uno", 100);
        assert!(hacer(&a.d, "p1", "Antes de sincronizar", 1_100).unwrap());
        a.d.anadir_mensaje(&Json::de_valor(&json!({
            "id": "g1", "cuando": 50, "clase": "NOTA", "texto": "general", "numero": 1, "letra": "a",
        })))
        .unwrap();
        assert!(hacer(&a.d, crate::disco::GENERAL, "Antes de recibir", 1_500).unwrap());
        let t = todas(&a.d);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].cuando, 1_500, "la mas nueva primero");
    }
}
