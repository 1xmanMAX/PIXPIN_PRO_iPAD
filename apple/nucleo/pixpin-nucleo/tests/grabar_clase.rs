//! «Grabar clase o reunion» tal como lo vera Swift: lo que queda en el
//! cuaderno al terminar es lo de Windows (`grabar_clase::guardar`), campo a
//! campo, para que sincronice (tambien la carpeta de la grabacion, e015e2b);
//! y «Enviar a Anki» sin Anki deja el fichero.

use std::path::PathBuf;
use std::sync::Arc;

use pixpin_nucleo::{CapturaClase, EnvioAnki, Nucleo};
use pixpin_proyecto::{almacen, cuaderno};

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-nucleo-clase-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temporal(dir)
    }

    fn datos(&self) -> PathBuf {
        self.0.join("datos")
    }

    fn nucleo(&self) -> Arc<Nucleo> {
        Nucleo::abrir(self.datos().display().to_string(), "Mac".into()).unwrap()
    }

    fn fichero(&self, nombre: &str, bytes: &[u8]) -> String {
        let p = self.0.join(nombre);
        std::fs::write(&p, bytes).unwrap();
        p.display().to_string()
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn la_clase_entra_con_sus_capturas_enlazadas_como_en_windows() {
    let t = Temporal::nueva("guardar");
    let n = t.nucleo();
    let chat = n.crear_chat("Física".into()).unwrap().id;
    let capturas = vec![
        CapturaClase {
            ms: 65_400,
            ruta_png: t.fichero("a.png", b"png-a"),
        },
        CapturaClase {
            ms: 2_000,
            ruta_png: t.fichero("b.png", b"png-b"),
        },
        // Caso negativo: una que ya no esta se salta sin llevarse las demas.
        CapturaClase {
            ms: 9_000,
            ruta_png: t.0.join("no-esta.png").display().to_string(),
        },
    ];
    let hecho = n
        .guardar_clase(
            chat.clone(),
            Some(t.fichero("clase.m4a", b"m4a")),
            120_000,
            vec![5, 40_000],
            None,
            capturas,
            0,
        )
        .unwrap();
    let hv = hecho.voz.as_ref().unwrap();
    assert_eq!(hv.clase, "VOZ");
    assert!(hv.nombre.starts_with("Clase "), "{}", hv.nombre);
    assert!(hv.nombre.ends_with(".m4a"));
    assert_eq!(hv.duracion_ms, 120_000);
    assert_eq!(hv.picos, vec![5, 32_767]);
    assert!(hecho.video.is_none());
    assert_eq!(hecho.capturas.len(), 2);

    let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&t.datos(), &chat)).unwrap();
    assert_eq!(c.mensajes.len(), 4, "la voz, detras sus capturas y su carpeta");
    let voz = &c.mensajes[0];
    let nombre = voz.nombre.trim_end_matches(".m4a").to_string();
    assert_eq!(voz.resto.get("grabacion"), Some(&serde_json::json!("clase")));
    // Como Windows, en orden de minuto: la de 2 s primero, aunque se
    // hiciera despues.
    let b = &c.mensajes[1];
    let a = &c.mensajes[2];
    assert_eq!(b.numero, voz.numero + 1);
    assert_eq!(a.numero, voz.numero + 2, "la que no entro no deja hueco en los numeros");
    assert_eq!(a.clase, Some(cuaderno::Clase::Imagen));
    assert!(b.nombre.starts_with("clase-") && b.nombre.ends_with("-1.png"), "{}", b.nombre);
    // El fichero lleva su puesto entre las pedidas (`i + 1` de Windows).
    assert!(a.nombre.ends_with("-3.png"), "{}", a.nombre);
    assert_eq!(a.texto, format!("⏱ 1:05 · {nombre}"));
    assert_eq!(b.texto, format!("⏱ 0:02 · {nombre}"));
    assert_eq!(
        a.resto.get("enGrabacion"),
        Some(&serde_json::json!({ "de": voz.id, "ms": 65_400 }))
    );
    // En la voz, en orden de minuto.
    assert_eq!(
        voz.resto.get("capturas"),
        Some(&serde_json::json!([
            { "ms": 2_000, "id": b.id },
            { "ms": 65_400, "id": a.id },
        ]))
    );

    // La carpeta, detras de todo, con las capturas en orden de minuto.
    let carpeta = &c.mensajes[3];
    assert_eq!(carpeta.numero, a.numero + 1);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&carpeta.texto).unwrap(),
        serde_json::json!({
            "formato": 1, "tipo": "grabacion", "voz": voz.id, "duracionMs": 120_000,
            "paginas": [
                { "id": voz.id, "titulo": "", "ms": 0 },
                { "id": b.id, "titulo": "⏱ 0:02", "ms": 2_000 },
                { "id": a.id, "titulo": "⏱ 1:05", "ms": 65_400 },
            ]
        })
    );
    assert_eq!(hecho.carpeta.as_ref().map(|m| m.id.as_str()), Some(carpeta.id.as_str()));

    // La pantalla de la letra: las capturas con su fichero de aqui.
    let lista = n.capturas_de_la_clase(chat.clone(), voz.id.clone()).unwrap();
    assert_eq!(lista.iter().map(|c| c.ms).collect::<Vec<_>>(), vec![2_000, 65_400]);
    assert!(lista.iter().all(|c| c.ruta.as_ref().is_some_and(|r| std::path::Path::new(r).is_file())));
    // Caso negativo: una foto suelta no es una clase.
    assert!(n.capturas_de_la_clase(chat.clone(), a.id.clone()).unwrap().is_empty());

    // «Enviar a Anki» de la clase: sin Anki en la maquina, el fichero.
    match n.enviar_a_anki(chat, voz.id.clone(), "¿Qué se explicó aquí?".into()).unwrap() {
        EnvioAnki::Fichero { n, mazo, ruta } => {
            assert_eq!(n, 2);
            assert_eq!(mazo, "PixPin::Física");
            assert!(std::fs::read_to_string(ruta).unwrap().contains("pixpin-"));
        }
        // Con un Anki de verdad escuchando, tambien vale.
        EnvioAnki::Hecho { .. } | EnvioAnki::YaEstaban { .. } | EnvioAnki::Fallo { .. } => {}
        EnvioAnki::Nada => panic!("una clase con capturas da tarjetas"),
    }
}

#[test]
fn sin_capturas_la_voz_dice_igual_que_es_de_una_clase() {
    let t = Temporal::nueva("sola");
    let n = t.nucleo();
    let chat = n.crear_chat("Obra".into()).unwrap().id;
    let hecho = n
        .guardar_clase(
            chat.clone(),
            Some(t.fichero("x.m4a", b"m4a")),
            3_000,
            vec![],
            None,
            vec![],
            3_600_000,
        )
        .unwrap();
    assert!(hecho.capturas.is_empty());
    let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&t.datos(), &chat)).unwrap();
    assert_eq!(c.mensajes.len(), 2, "la voz y su carpeta");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&c.mensajes[1].texto).unwrap()["paginas"],
        serde_json::json!([{ "id": c.mensajes[0].id, "titulo": "", "ms": 0 }])
    );
    // Como Windows (df01da9): `capturas` va siempre, aunque este vacia.
    assert_eq!(c.mensajes[0].resto.get("capturas"), Some(&serde_json::json!([])));
    assert_eq!(c.mensajes[0].resto.get("grabacion"), Some(&serde_json::json!("clase")));
    // Caso negativo: sin video no hay campo `video`.
    assert!(c.mensajes[0].resto.get("video").is_none());
    // Y una clase sin capturas no da tarjetas de Anki, ni una nota suelta.
    assert_eq!(
        n.enviar_a_anki(chat.clone(), c.mensajes[0].id.clone(), "?".into()).unwrap(),
        EnvioAnki::Nada
    );
    let nota = n.escribir_nota(chat.clone(), "un parrafo suelto".into(), None).unwrap();
    assert_eq!(n.enviar_a_anki(chat, nota.id, "?".into()).unwrap(), EnvioAnki::Nada);
}

#[test]
fn el_video_entra_detras_de_la_voz_enlazado_como_en_windows() {
    let t = Temporal::nueva("video");
    let n = t.nucleo();
    let chat = n.crear_chat("Reunión".into()).unwrap().id;
    let video = t.fichero("temporal.video.mp4", b"mp4-de-la-reunion");
    let hecho = n
        .guardar_clase(
            chat.clone(),
            Some(t.fichero("voz.m4a", b"m4a")),
            90_000,
            vec![7],
            Some(video.clone()),
            vec![CapturaClase {
                ms: 30_000,
                ruta_png: t.fichero("c.png", b"png"),
            }],
            0,
        )
        .unwrap();
    // El video se movio, no se copio: una hora son cientos de megas.
    assert!(!std::path::Path::new(&video).exists());
    let hv = hecho.video.as_ref().unwrap();
    assert_eq!(hv.clase, "ARCHIVO");

    let carpeta = almacen::carpeta(&t.datos(), &chat);
    let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
    assert_eq!(c.mensajes.len(), 4, "la voz, el video, la captura y su carpeta");
    let (voz, v, foto) = (&c.mensajes[0], &c.mensajes[1], &c.mensajes[2]);
    let nombre = voz.nombre.trim_end_matches(".m4a").to_string();
    // Numeros seguidos: voz, video, capturas.
    assert_eq!(v.numero, voz.numero + 1);
    assert_eq!(foto.numero, voz.numero + 2);
    // El video, campo a campo.
    assert_eq!(v.clase, Some(cuaderno::Clase::Archivo));
    assert_eq!(v.nombre, format!("{nombre}.mp4"));
    assert_eq!(v.ruta.as_deref(), Some(format!("archivos/{nombre}.mp4").as_str()));
    assert_eq!(v.bytes, b"mp4-de-la-reunion".len() as i64);
    assert_eq!(v.id, hv.id);
    assert_eq!(
        v.resto.get("enGrabacion"),
        Some(&serde_json::json!({ "de": voz.id, "ms": 0 }))
    );
    assert_eq!(
        std::fs::read(carpeta.join(v.ruta.as_deref().unwrap())).unwrap(),
        b"mp4-de-la-reunion"
    );
    // La captura, detras del video y con su minuto.
    assert_eq!(foto.clase, Some(cuaderno::Clase::Imagen));
    assert_eq!(foto.texto, format!("⏱ 0:30 · {nombre}"));
    assert_eq!(
        foto.resto.get("enGrabacion"),
        Some(&serde_json::json!({ "de": voz.id, "ms": 30_000 }))
    );
    // La voz, con sus tres campos.
    assert_eq!(voz.resto.get("grabacion"), Some(&serde_json::json!("clase")));
    assert_eq!(
        voz.resto.get("capturas"),
        Some(&serde_json::json!([{ "ms": 30_000, "id": foto.id }]))
    );
    assert_eq!(voz.resto.get("video"), Some(&serde_json::json!({ "id": v.id })));
    assert_eq!(hecho.voz.as_ref().unwrap().id, voz.id);

    // **La carpeta de la grabacion**, campo a campo como la de Windows
    // (`manual-sincronizar-lo-nuevo.md` §5.1): un `MINIAPP` «carpeta» con
    // el nombre de la clase, detras de lo grabado, y en su `texto` el
    // documento en una linea con las claves en el orden de Windows.
    let cm = &c.mensajes[3];
    assert_eq!(cm.clase, Some(cuaderno::Clase::MiniApp));
    assert_eq!(cm.miniapp.as_deref(), Some("carpeta"));
    assert_eq!(cm.nombre, nombre, "el nombre de la grabacion");
    assert_eq!(cm.numero, foto.numero + 1, "numeros seguidos");
    assert_eq!(cm.aparato, voz.aparato);
    assert_eq!(cm.proyecto.as_deref(), Some(chat.as_str()));
    assert!(cm.cuando >= voz.cuando);
    assert!(![&voz.id, &v.id, &foto.id].contains(&&cm.id), "su id no es el de lo que lleva");
    assert_eq!(
        cm.texto,
        format!(
            concat!(
                r#"{{"formato":1,"tipo":"grabacion","voz":"{voz}","video":"{video}","duracionMs":90000,"#,
                r#""paginas":[{{"id":"{voz}","titulo":"","ms":0}},{{"id":"{video}","titulo":"","ms":0}},"#,
                r#"{{"id":"{foto}","titulo":"⏱ 0:30","ms":30000}}]}}"#
            ),
            voz = voz.id,
            video = v.id,
            foto = foto.id
        )
    );
    // La linea del cuaderno, con lo de un mensaje de mini-app (sin `ruta`).
    let linea = std::fs::read_to_string(carpeta.join("guardados.jsonl"))
        .unwrap()
        .lines()
        .last()
        .unwrap()
        .to_string();
    let l: serde_json::Value = serde_json::from_str(&linea).unwrap();
    assert_eq!(l["clase"], "MINIAPP");
    assert_eq!(l["miniapp"], "carpeta");
    assert_eq!(l["id"], cm.id.as_str());
    assert!(l["ruta"].is_null(), "una carpeta no lleva fichero");
    // La lee el mismo crate que Windows: es la carpeta de una grabacion,
    // con su nota, su video y una captura, sin adjuntos.
    let doc = pixpin_proyecto::carpeta_de_paginas::Carpeta::de_mensaje(cm).unwrap();
    assert!(doc.es_grabacion());
    assert_eq!(doc.capturas().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec![foto.id.as_str()]);
    assert_eq!(doc.adjuntos().count(), 0);
    let devuelta = hecho.carpeta.as_ref().unwrap();
    assert_eq!((devuelta.id.as_str(), devuelta.texto.as_str()), (cm.id.as_str(), cm.texto.as_str()));
    // El chat sube en la lista con el nombre de la clase.
    let ficha = n.chats().unwrap().into_iter().find(|f| f.id == chat).unwrap();
    assert_eq!(ficha.resumen, nombre);
    // Lo de dentro de la carpeta no sale suelto en el chat.
    let dentro = pixpin_nucleo::carpetas::dentro_de_carpetas(vec![cm.texto.clone()]);
    assert!(dentro.contains(&voz.id) && dentro.contains(&v.id) && dentro.contains(&foto.id));
    // Y se estudia: desde la carpeta, su nota al principio.
    let en = n.de_una_clase(chat.clone(), cm.id.clone()).unwrap().unwrap();
    assert_eq!((en.voz.as_str(), en.ms), (voz.id.as_str(), 0));

    // Caso negativo: el video no es una captura de la clase (la letra no lo
    // ensena como imagen).
    let lista = n.capturas_de_la_clase(chat, voz.id.clone()).unwrap();
    assert_eq!(lista.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), vec![foto.id.as_str()]);
}

#[test]
fn sin_nota_el_video_y_las_capturas_entran_sueltos() {
    let t = Temporal::nueva("sin-nota");
    let n = t.nucleo();
    let chat = n.crear_chat("Taller".into()).unwrap().id;
    let hecho = n
        .guardar_clase(
            chat.clone(),
            None,
            0,
            vec![],
            Some(t.fichero("v.mp4", b"mp4")),
            vec![CapturaClase {
                ms: 1_000,
                ruta_png: t.fichero("c.png", b"png"),
            }],
            0,
        )
        .unwrap();
    assert!(hecho.voz.is_none());
    assert!(hecho.video.is_some());
    let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&t.datos(), &chat)).unwrap();
    assert_eq!(c.mensajes.len(), 3, "el video, la captura y su carpeta");
    assert_eq!(c.mensajes[1].numero, c.mensajes[0].numero + 1);
    // La carpeta, como la de Windows sin nota: sin `voz` ni `duracionMs`.
    assert_eq!(
        c.mensajes[2].texto,
        format!(
            r#"{{"formato":1,"tipo":"grabacion","video":"{}","paginas":[{{"id":"{}","titulo":"","ms":0}},{{"id":"{}","titulo":"⏱ 0:01","ms":1000}}]}}"#,
            c.mensajes[0].id, c.mensajes[0].id, c.mensajes[1].id
        )
    );
    // Caso negativo: sin nota no hay clase que estudiar desde la carpeta.
    assert!(n.de_una_clase(chat.clone(), c.mensajes[2].id.clone()).unwrap().is_none());
    assert!(c.mensajes[0].nombre.starts_with("Clase ") && c.mensajes[0].nombre.ends_with(".mp4"));
    // Caso negativo: sin nota no hay a quien enlazarlos.
    assert!(c.mensajes.iter().all(|m| m.resto.get("enGrabacion").is_none()));
    assert!(c.mensajes[1].texto.starts_with("⏱ 0:01 · Clase "));
}

#[test]
fn el_video_que_no_entra_queda_en_la_carpeta_del_proyecto() {
    let t = Temporal::nueva("rescate");
    let n = t.nucleo();
    let chat = n.crear_chat("Clase".into()).unwrap().id;
    let video = t.fichero("v.mp4", b"mp4");
    // La nota no esta: no entra nada y el chat queda como estaba.
    let fallo = n.guardar_clase(
        chat.clone(),
        Some(t.0.join("no-esta.m4a").display().to_string()),
        5_000,
        vec![],
        Some(video.clone()),
        vec![],
        0,
    );
    assert!(fallo.is_err());
    let carpeta = almacen::carpeta(&t.datos(), &chat);
    let rescatados: Vec<String> = std::fs::read_dir(&carpeta)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("Clase ") && n.ends_with(".mp4"))
        .collect();
    assert_eq!(rescatados.len(), 1, "{rescatados:?}");
    assert_eq!(std::fs::read(carpeta.join(&rescatados[0])).unwrap(), b"mp4");
    // Caso negativo: el temporal ya no esta (no hay dos copias de una hora).
    assert!(!std::path::Path::new(&video).exists());
}
