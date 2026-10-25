// Pruebas de `lienzo_suelto` (`recibir/lienzo_suelto/pruebas.rs` de Windows, tal cual).
//! Un lienzo del PC tiene que llegar al movil como LIENZO que su editor
//! abre, y uno del movil al lienzo del PC. Lo que decide si el movil lo
//! abre esta en su Kotlin, asi que aqui hay un revisor con SUS reglas
//! ([`como_lo_lee_el_movil`]), copiadas de `motor/Element.kt`,
//! `motor/Scene.kt` y `motor/PaquetePixpin.kt` (main de 2026-09-26).

use super::*;
use pixpin_motor2d::elemento::{ColorRgba, Elemento as Pieza, Figura};
use pixpin_motor2d::formas::TipoPunta;
use pixpin_motor2d::vector::Punto2;
use pixpin_proyecto::almacen::{Ficha, Indice};

fn carpeta_temporal(etiqueta: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "pixpin-lienzo-suelto-{etiqueta}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n-una-foto-de-mentira";

// ------------------------------------------------ el revisor del movil

/// `ElementType` (`Element.kt:27-222`).
const TIPOS: &[&str] = &[
    "rectangle",
    "diamond",
    "ellipse",
    "arrow",
    "line",
    "freedraw",
    "text",
    "image",
    "pixpin-mosaic",
    "pixpin-spotlight",
    "pixpin-lupa",
    "pixpin-serial",
    "pixpin-measure",
    "pixpin-arc",
    "pixpin-region",
    "pixpin-scalebar",
    "frame",
    "pixpin-point",
    "pixpin-axes",
    "pixpin-number-line",
    "pixpin-space",
    "pixpin-solid",
    "pixpin-gantt",
];

fn entero32(v: &Value) -> bool {
    v.as_i64().is_some_and(|n| i32::try_from(n).is_ok())
}

fn es_pt(v: &Value) -> bool {
    v.get("x").is_some_and(Value::is_number) && v.get("y").is_some_and(Value::is_number)
}

/// Lo que haria `ExcalidrawJson.decodeFromString<Scene>` (kotlinx con
/// `ignoreUnknownKeys = true`, SIN `coerceInputValues`): un campo que no
/// encaja tira la escena entera. Devuelve el primer motivo.
fn como_lo_lee_el_movil(json: &str) -> Result<(), String> {
    let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let elementos = v
        .get("elements")
        .and_then(Value::as_array)
        .ok_or("sin elements")?;
    if let Some(b) = v.get("backgroundColor")
        && !b.is_string()
    {
        return Err("backgroundColor no es texto".into());
    }
    for e in elementos {
        let id = e
            .get("id")
            .and_then(Value::as_str)
            .ok_or("elemento sin id")?;
        let tipo = e.get("type").and_then(Value::as_str).ok_or("sin type")?;
        if !TIPOS.contains(&tipo) {
            return Err(format!("{id}: type «{tipo}» no existe en ElementType"));
        }
        // Sin valor por omision en `Element`: obligatorios.
        for k in ["x", "y", "width", "height"] {
            if !e.get(k).is_some_and(Value::is_number) {
                return Err(format!("{id}: falta {k}"));
            }
        }
        if !e.get("seed").is_some_and(entero32) {
            return Err(format!("{id}: seed no es Int"));
        }
        for k in [
            "roughness",
            "opacity",
            "version",
            "versionNonce",
            "fontFamily",
            "periodos",
            "oscurecer",
        ] {
            if let Some(x) = e.get(k).filter(|x| !x.is_null())
                && !entero32(x)
            {
                return Err(format!("{id}: {k}={x} no es Int"));
            }
        }
        let enums: &[(&str, &[&str])] = &[
            (
                "fillStyle",
                &["hachure", "cross-hatch", "solid", "zigzag", "pixpin-lines"],
            ),
            ("strokeStyle", &["solid", "dashed", "dotted"]),
            ("textAlign", &["left", "center", "right"]),
            ("verticalAlign", &["top", "middle", "bottom"]),
            (
                "startArrowhead",
                &[
                    "arrow",
                    "bar",
                    "circle",
                    "circle_outline",
                    "triangle",
                    "triangle_outline",
                    "diamond",
                    "diamond_outline",
                ],
            ),
            (
                "endArrowhead",
                &[
                    "arrow",
                    "bar",
                    "circle",
                    "circle_outline",
                    "triangle",
                    "triangle_outline",
                    "diamond",
                    "diamond_outline",
                ],
            ),
            (
                "material",
                &[
                    "lisa",
                    "luz",
                    "hdr",
                    "rayado",
                    "cruzado",
                    "puntos",
                    "tiza",
                    "lapiz2b",
                    "seco",
                    "trama",
                    "cuadritos",
                ],
            ),
        ];
        for (k, palabras) in enums {
            match e.get(*k) {
                None | Some(Value::Null) => {}
                Some(Value::String(s)) if palabras.contains(&s.as_str()) => {}
                Some(x) => return Err(format!("{id}: {k}={x} no es de la enumeracion")),
            }
        }
        if let Some(p) = e.get("points").filter(|p| !p.is_null()) {
            let l = p.as_array().ok_or(format!("{id}: points no es lista"))?;
            if !l.iter().all(es_pt) {
                return Err(format!("{id}: points no son Pt {{x,y}}"));
            }
            // `Renderer.sePierdeDePequeno`: un trazo, una linea o una flecha
            // con la caja por debajo de dos pixeles se pinta como la raya de
            // su primer punto al ultimo. El que se extiende no puede llegar asi.
            let eje = |k: &str| {
                l.iter()
                    .filter_map(|q| q[k].as_f64())
                    .fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v), b.max(v)))
            };
            let ((x1, x2), (y1, y2)) = (eje("x"), eje("y"));
            let num = |k: &str| e.get(k).and_then(Value::as_f64).unwrap_or(0.0);
            if ["freedraw", "line", "arrow"].contains(&tipo)
                && l.len() >= 2
                && (x2 - x1) + (y2 - y1) >= 2.0
                && num("width") + num("height") < 2.0
            {
                return Err(format!(
                    "{id}: caja a cero, se pintaria como una raya recta"
                ));
            }
        }
        if let Some(p) = e.get("lastCommittedPoint").filter(|p| !p.is_null())
            && !es_pt(p)
        {
            return Err(format!("{id}: lastCommittedPoint no es Pt"));
        }
        if let Some(Value::Array(b)) = e.get("boundElements") {
            for a in b {
                let t = a.get("type").and_then(Value::as_str).unwrap_or("");
                if !a.get("id").is_some_and(Value::is_string) || !TIPOS.contains(&t) {
                    return Err(format!("{id}: boundElements roto"));
                }
            }
        }
        if let Some(r) = e.get("roundness").filter(|r| !r.is_null())
            && !r.get("type").is_some_and(entero32)
        {
            return Err(format!("{id}: roundness sin type Int"));
        }
    }
    if let Some(Value::Object(f)) = v.get("files") {
        for (k, x) in f {
            for campo in ["id", "mimeType", "path"] {
                if !x.get(campo).is_some_and(Value::is_string) {
                    return Err(format!("files.{k}: falta {campo} (SceneFile)"));
                }
            }
        }
    }
    Ok(())
}

// ------------------------------------------------ lo que hay en el PC

/// Un lienzo del PC con de todo: texto, flecha, foto, dibujo a mano y lo
/// propio de PixPin (foco y cota), escrito por el mismo motor que lo guarda.
fn lienzo_del_pc() -> String {
    let mut escena = pixpin_motor2d::Escena::nueva();
    escena.anadir(Pieza {
        figura: Figura::Texto {
            texto: "Hola movil".into(),
            tam: 20.0,
            familia: "Virgil".into(),
        },
        x: 10.0,
        y: 10.0,
        ancho: 120.0,
        alto: 24.0,
        ..Pieza::default()
    });
    escena.anadir(Pieza {
        figura: Figura::Flecha {
            puntos: vec![Punto2::nuevo(10.0, 60.0), Punto2::nuevo(200.0, 90.0)],
            punta_inicio: TipoPunta::Ninguna,
            punta_fin: TipoPunta::Flecha,
            codos: false,
        },
        x: 10.0,
        y: 60.0,
        ancho: 190.0,
        alto: 30.0,
        ..Pieza::default()
    });
    escena.anadir(Pieza {
        figura: Figura::Lapiz {
            puntos: (0..12)
                .map(|i| Punto2::nuevo(20.0 + i as f32 * 5.0, 150.0 + (i % 3) as f32 * 4.0))
                .collect(),
            presiones: Vec::new(),
            opciones: Some(Default::default()),
        },
        grosor: 3.0,
        trazo: ColorRgba::opaco(0.9, 0.1, 0.1),
        ..Pieza::default()
    });
    escena.anadir(Pieza {
        figura: Figura::Foco {
            cristal: Default::default(),
        },
        x: 0.0,
        y: 0.0,
        ancho: 50.0,
        alto: 50.0,
        ..Pieza::default()
    });
    escena.anadir(Pieza {
        figura: Figura::Cota {
            puntos: vec![Punto2::nuevo(0.0, 300.0), Punto2::nuevo(100.0, 300.0)],
        },
        x: 0.0,
        y: 300.0,
        ancho: 100.0,
        alto: 0.0,
        ..Pieza::default()
    });
    let lienzo = pixpin_motor2d::excalidraw::con_escena(
        &pixpin_motor2d::excalidraw::Lienzo::vacio(),
        &escena,
    );
    let mut v: Value =
        serde_json::from_str(&pixpin_motor2d::excalidraw::escribir(&lienzo)).unwrap();
    // La foto como la apunta el PC (`pdf_en_chat`, `fusionar_paginas`): su
    // `fileId` y una entrada de `files` con la ruta dentro del proyecto. Va
    // a mano porque asi la escriben ellos: el motor no la emite desde la
    // escena (una `Figura::Imagen` nueva no sale en `escribir`).
    v["elements"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "foto1", "type": "image", "x": 220.0, "y": 20.0, "width": 100.0, "height": 80.0,
            "angle": 0, "strokeColor": "transparent", "backgroundColor": "transparent",
            "fillStyle": "solid", "strokeWidth": 1, "strokeStyle": "solid", "roughness": 0,
            "opacity": 100, "groupIds": [], "seed": 1, "version": 1, "versionNonce": 1,
            "isDeleted": false, "boundElements": null, "locked": true, "fileId": "f7",
            "status": "saved", "scale": [1, 1]
        }));
    v["files"] = serde_json::json!({"f7": {"id": "f7", "mimeType": "image/png", "path": "imagenes/f7", "created": 1}});
    // Y algo que el movil no conoce (un tipo de una version futura del PC).
    v["elements"].as_array_mut().unwrap().push(serde_json::json!({
        "id": "futuro", "type": "pixpin-tabla-del-futuro", "x": 0, "y": 0, "width": 1, "height": 1, "seed": 1
    }));
    v["appState"] = serde_json::json!({"viewBackgroundColor": "#fff8e1"});
    serde_json::to_string(&v).unwrap()
}

/// Un proyecto del PC con ese lienzo en su chat, como lo deja `crear_lienzo`.
fn proyecto_del_pc(raiz: &Path) -> (String, PathBuf) {
    let ficha = Ficha::nueva("Obra", 1_758_000_000_000, "6ARJ");
    let mut i = Indice::leer(raiz);
    i.proyectos.push(ficha.clone());
    i.guardar(raiz).unwrap();
    let dibujo = "d-pc-1";
    let ruta = almacen::lienzo(raiz, &ficha.id, dibujo);
    std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
    std::fs::write(&ruta, lienzo_del_pc()).unwrap();
    let carpeta = almacen::carpeta(raiz, &ficha.id);
    std::fs::create_dir_all(carpeta.join("imagenes")).unwrap();
    std::fs::write(carpeta.join("imagenes/f7"), PNG).unwrap();
    let mut m = cuaderno::Mensaje::adjunto(
        cuaderno::Clase::Dibujo,
        &format!("{dibujo}.excalidraw"),
        &format!("lienzos/{dibujo}.excalidraw"),
        10,
        &cuaderno::Sello {
            cuando: 1_758_000_000_500,
            numero: 3,
            aparato: "6ARJ".into(),
            proyecto: ficha.id.clone(),
        },
    );
    m.referencia = Some(dibujo.into());
    cuaderno::anadir(&carpeta, &m).unwrap();
    (ficha.id, ruta)
}

// ------------------------------------------------------------ pruebas

#[test]
fn el_lienzo_tal_cual_lo_guarda_el_pc_no_lo_leeria_el_movil() {
    // El por que del fallo, medido: los puntos van como `[x, y]` y hay un
    // tipo que alla no existe. Cualquiera de los dos tira la escena entera.
    let crudo = lienzo_del_pc();
    assert!(como_lo_lee_el_movil(&crudo).is_err());
}

#[test]
fn un_lienzo_del_pc_sale_como_lienzo_de_una_hoja_y_no_como_archivo() {
    let raiz = carpeta_temporal("sale");
    let (fid, ruta) = proyecto_del_pc(&raiz);
    assert!(es_lienzo(&ruta));
    let (e, bytes) = empaquetar(&raiz, &ruta, 1_758_000_001_000).unwrap();

    // La oferta, como la de `EnviarActivity.deLienzo`.
    assert_eq!(e.tipo, envio::LIENZO);
    assert_eq!(e.mime.as_deref(), Some("application/zip"));
    assert_eq!(e.identidad, format!("lienzo:{fid}:d-pc-1"));
    assert_eq!(
        e.proyecto.as_deref(),
        Some(format!("proyecto:{fid}").as_str())
    );
    assert_eq!(e.proyecto_nombre.as_deref(), Some("Obra"));
    assert_eq!(e.nombre, "Lienzo", "el id no se ensena como nombre");
    assert_eq!(e.creado, 1_758_000_000_500, "la hora de su mensaje");
    assert!(
        e.uid.is_some() && e.codigo_de_chat.is_some(),
        "con sus codigos"
    );
    assert_eq!(e.bytes, bytes.len() as i64);
    assert!(nombre_del_paquete(&e).ends_with(".pixpin"));
    assert!(!nombre_del_paquete(&e).contains(".excalidraw"));

    // El paquete, como lo abre `PaquetePixpin.abrir`.
    let p = Paquete::desde_bytes(&bytes).unwrap();
    let manifiesto = String::from_utf8_lossy(p.entrada("manifest.json").unwrap()).into_owned();
    assert!(
        manifiesto.contains("\"pixpin\""),
        "sin esto abrir() da null: {manifiesto}"
    );
    assert_eq!(p.proyecto.hojas.len(), 1);
    let hoja = &p.proyecto.hojas[0];
    assert_eq!(hoja.dibujo.as_deref(), Some("d-pc-1"));
    assert_eq!(p.proyecto.nombre, "Obra");

    // Y la escena de dentro, como la lee su `Scene`.
    let escena =
        String::from_utf8_lossy(p.entrada("lienzos/d-pc-1.excalidraw").unwrap()).into_owned();
    como_lo_lee_el_movil(&escena).unwrap();
    let v: Value = serde_json::from_str(&escena).unwrap();
    let tipos: Vec<&str> = v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["type"].as_str().unwrap())
        .collect();
    for t in [
        "text",
        "arrow",
        "freedraw",
        "image",
        "pixpin-spotlight",
        "pixpin-measure",
    ] {
        assert!(tipos.contains(&t), "falta {t} en {tipos:?}");
    }
    assert!(
        !tipos.contains(&"pixpin-tabla-del-futuro"),
        "lo que alla no existe se queda fuera"
    );
    assert_eq!(
        v["backgroundColor"], "#fff8e1",
        "el papel donde lo lee el movil"
    );
    assert_eq!(v["files"]["f7"]["path"], "imagenes/f7");
    assert_eq!(p.entrada("imagenes/f7"), Some(PNG), "la foto viaja dentro");
}

#[test]
fn un_excalidraw_suelto_con_la_foto_incrustada_sale_como_proyecto_nuevo() {
    let raiz = carpeta_temporal("suelto");
    let ruta = raiz.join("Plano de casa.excalidraw");
    // Como lo exporta excalidraw.com (o `ExcalidrawStore.exportar`): la foto
    // en `dataURL`, sin ruta.
    std::fs::write(
        &ruta,
        r##"{"type":"excalidraw","version":2,"elements":[
            {"id":"i1","type":"image","x":0,"y":0,"width":10,"height":10,"seed":3,"fileId":"f1","roughness":1.0}],
           "appState":{"viewBackgroundColor":"#ffffff"},
           "files":{"f1":{"id":"f1","mimeType":"image/png","dataURL":"data:image/png;base64,aGVsbG8="},
                    "roto":{"id":"roto","mimeType":"image/png"}}}"##,
    )
    .unwrap();
    let (e, bytes) = empaquetar(&raiz, &ruta, 5).unwrap();
    assert_eq!(e.tipo, envio::LIENZO);
    assert_eq!(e.proyecto_nombre.as_deref(), Some("Plano de casa"));
    let p = Paquete::desde_bytes(&bytes).unwrap();
    let d = p.proyecto.hojas[0].dibujo.clone().unwrap();
    let escena = String::from_utf8_lossy(p.entrada(&format!("lienzos/{d}.excalidraw")).unwrap())
        .into_owned();
    como_lo_lee_el_movil(&escena).unwrap();
    assert_eq!(p.entrada("imagenes/f1"), Some(&b"hello"[..]));
    // Caso negativo: una foto sin bytes no llega como `SceneFile` a medias.
    assert!(!escena.contains("\"roto\""));
}

#[test]
fn lo_que_no_es_un_lienzo_sigue_saliendo_como_archivo() {
    for n in [
        "foto.png",
        "notas.md",
        "proyecto.pixpin",
        "dibujo.pixpin2d",
        "sin-extension",
    ] {
        assert!(!es_lienzo(Path::new(n)), "{n}");
    }
    assert!(es_lienzo(Path::new("x/Plano.EXCALIDRAW")));
}

/// Un `.pixpin` de una hoja como el que manda el movil (`deLienzo`).
fn paquete_del_movil(hoja: &str, dibujo: &str, nombre: &str) -> (Elemento, Vec<u8>) {
    let proyecto: Proyecto = serde_json::from_value(serde_json::json!({
        "id": "pr-movil", "nombre": "Tesis", "archivado": false, "tocado": 1,
        "hojas": [{"id": hoja, "nombre": nombre, "dibujo": dibujo, "uid": format!("U{hoja}")}],
        "croquis": [], "uid": "PPPPPPPPPP", "creado": 1_757_000_000_000i64, "aparato": "K7Q2"
    }))
    .unwrap();
    let mut p = Paquete::nuevo(
        pixpin_proyecto::Manifiesto {
            aplicacion: "pixpin-android".into(),
            ..Default::default()
        },
        proyecto,
    );
    // El `Scene` del movil: papel arriba, puntos como objetos, foto por ruta.
    p.poner_entrada(
        &format!("lienzos/{dibujo}.excalidraw"),
        serde_json::to_vec(&serde_json::json!({
            "elements": [
                {"id": "t1", "type": "freedraw", "x": 5.0, "y": 5.0, "width": 20.0, "height": 10.0, "seed": 9,
                 "points": [{"x": 0.0, "y": 0.0}, {"x": 20.0, "y": 10.0}], "pressures": []},
                {"id": "i1", "type": "image", "x": 50.0, "y": 0.0, "width": 10.0, "height": 10.0, "seed": 2, "fileId": "f1"}
            ],
            "files": {"f1": {"id": "f1", "mimeType": "image/png", "path": "imagenes/f1", "created": 0}},
            "backgroundColor": "#e8f5e9"
        }))
        .unwrap(),
    );
    p.poner_entrada("imagenes/f1", PNG.to_vec());
    p.poner_entrada("chat/mensajes.jsonl", b"{}".to_vec());
    let bytes = p.a_bytes().unwrap();
    let e = Elemento {
        tipo: envio::LIENZO.into(),
        nombre: nombre.into(),
        bytes: bytes.len() as i64,
        mime: Some("application/zip".into()),
        identidad: format!("lienzo:pr-movil:{hoja}"),
        proyecto: Some("proyecto:pr-movil".into()),
        proyecto_nombre: Some("Tesis".into()),
        creado: 1_757_000_000_100,
        uid: Some(format!("U{hoja}")),
        codigo_de_chat: Some("4·K7Q2".into()),
        aparato: None,
    };
    (e, bytes)
}

fn lienzos_de(raiz: &Path, fid: &str) -> Vec<cuaderno::Mensaje> {
    cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, fid))
        .unwrap()
        .mensajes
        .into_iter()
        .filter(|m| m.clase == Some(cuaderno::Clase::Dibujo))
        .collect()
}

fn sin_copia(_: &str) -> std::io::Result<()> {
    Ok(())
}

#[test]
fn un_lienzo_del_movil_crea_su_proyecto_y_se_abre_en_el_lienzo_del_pc() {
    let raiz = carpeta_temporal("llega");
    let (e, bytes) = paquete_del_movil("h1", "d1", "Plano");
    let r = raiz.join("llego.pixpin");
    std::fs::write(&r, bytes).unwrap();
    let Llegada::ProyectoNuevo(fid) =
        guardar(&raiz, &e, &r, "6ARJ", false, 10, &sin_copia).unwrap()
    else {
        panic!("tenia que crear el proyecto");
    };
    assert_eq!(Indice::leer(&raiz).buscar(&fid).unwrap().nombre, "Tesis");
    let m = lienzos_de(&raiz, &fid);
    assert_eq!(m.len(), 1, "un mensaje de lienzo en su chat");
    // Lo abre el lector del lienzo del PC, con su foto y su papel.
    let texto =
        std::fs::read_to_string(almacen::carpeta(&raiz, &fid).join(m[0].ruta.as_deref().unwrap()))
            .unwrap();
    let l = pixpin_motor2d::excalidraw::leer(&texto).unwrap();
    assert_eq!(l.elementos().len(), 2);
    let papel = pixpin_motor2d::excalidraw::fondo(&l);
    assert!(
        (papel.g - 0xf5 as f32 / 255.0).abs() < 0.01,
        "el papel del movil: {papel:?}"
    );
    assert!(almacen::carpeta(&raiz, &fid).join("imagenes/f1").is_file());
}

#[test]
fn el_segundo_lienzo_del_mismo_proyecto_cae_en_el_mismo_y_repetirlo_lo_pone_al_dia() {
    let raiz = carpeta_temporal("mismo");
    let (e1, b1) = paquete_del_movil("h1", "d1", "Plano");
    let r1 = raiz.join("1.pixpin");
    std::fs::write(&r1, b1).unwrap();
    let Llegada::ProyectoNuevo(fid) =
        guardar(&raiz, &e1, &r1, "6ARJ", false, 10, &sin_copia).unwrap()
    else {
        panic!()
    };

    let (e2, b2) = paquete_del_movil("h2", "d2", "Alzado");
    let r2 = raiz.join("2.pixpin");
    std::fs::write(&r2, &b2).unwrap();
    let copias = std::cell::Cell::new(0);
    let contar = |_: &str| {
        copias.set(copias.get() + 1);
        Ok(())
    };
    assert_eq!(
        guardar(&raiz, &e2, &r2, "6ARJ", false, 11, &contar).unwrap(),
        Llegada::Anadido(fid.clone())
    );
    assert_eq!(copias.get(), 1, "copia antes de tocar el que ya habia");
    assert_eq!(lienzos_de(&raiz, &fid).len(), 2);
    assert_eq!(
        Indice::leer(&raiz).proyectos.len(),
        1,
        "no crea otro proyecto"
    );

    // El mismo otra vez: en su sitio, sin otra burbuja.
    std::fs::write(&r2, &b2).unwrap();
    assert_eq!(
        guardar(&raiz, &e2, &r2, "6ARJ", false, 12, &sin_copia).unwrap(),
        Llegada::AlDia(fid.clone())
    );
    assert_eq!(lienzos_de(&raiz, &fid).len(), 2);

    // Caso negativo: «crear como nuevo» no toca el que habia.
    std::fs::write(&r2, &b2).unwrap();
    let Llegada::ProyectoNuevo(otro) =
        guardar(&raiz, &e2, &r2, "6ARJ", true, 13, &sin_copia).unwrap()
    else {
        panic!("como nuevo va aparte")
    };
    assert_ne!(otro, fid);
    assert_eq!(lienzos_de(&raiz, &fid).len(), 2);
}

#[test]
fn si_la_copia_de_seguridad_falla_no_se_escribe_nada() {
    let raiz = carpeta_temporal("sin-copia");
    let (e1, b1) = paquete_del_movil("h1", "d1", "Plano");
    let r = raiz.join("1.pixpin");
    std::fs::write(&r, b1).unwrap();
    let Llegada::ProyectoNuevo(fid) =
        guardar(&raiz, &e1, &r, "6ARJ", false, 10, &sin_copia).unwrap()
    else {
        panic!()
    };
    let (e2, b2) = paquete_del_movil("h2", "d2", "Alzado");
    std::fs::write(&r, b2).unwrap();
    let falla = |_: &str| Err(std::io::Error::other("disco lleno"));
    assert!(guardar(&raiz, &e2, &r, "6ARJ", false, 11, &falla).is_err());
    assert_eq!(lienzos_de(&raiz, &fid).len(), 1);
}

#[test]
fn lo_que_no_es_un_paquete_no_se_da_por_lienzo() {
    let raiz = carpeta_temporal("roto");
    let (e, _) = paquete_del_movil("h1", "d1", "Plano");
    let r = raiz.join("roto.pixpin");
    std::fs::write(&r, b"{\"elements\":[]}").unwrap();
    assert!(guardar(&raiz, &e, &r, "6ARJ", false, 10, &sin_copia).is_err());
    assert!(Indice::leer(&raiz).proyectos.is_empty());
}

#[test]
fn un_lienzo_del_pc_vuelve_al_pc_y_se_abre_igual() {
    // Ida y vuelta entre dos PixPin de escritorio con el mismo paquete.
    let origen = carpeta_temporal("ida");
    let (_, ruta) = proyecto_del_pc(&origen);
    let (e, bytes) = empaquetar(&origen, &ruta, 1).unwrap();
    let destino = carpeta_temporal("vuelta");
    let r = destino.join("l.pixpin");
    std::fs::write(&r, bytes).unwrap();
    let Llegada::ProyectoNuevo(fid) =
        guardar(&destino, &e, &r, "ZZZZ", false, 2, &sin_copia).unwrap()
    else {
        panic!()
    };
    let m = lienzos_de(&destino, &fid);
    assert_eq!(m.len(), 1);
    let texto = std::fs::read_to_string(
        almacen::carpeta(&destino, &fid).join(m[0].ruta.as_deref().unwrap()),
    )
    .unwrap();
    let l = pixpin_motor2d::excalidraw::leer(&texto).unwrap();
    assert_eq!(
        l.elementos().len(),
        6,
        "texto, flecha, trazo, foto, foco y cota"
    );
}

#[test]
fn los_puntos_raros_y_las_palabras_que_no_existen_alla_se_arreglan_o_se_quitan() {
    let json = r#"{"elements":[
        {"id":"a","type":"arrow","x":0,"y":0,"width":1,"height":1,"seed":5000000000,
         "roughness":1.5,"opacity":100.0,"endArrowhead":"dot","fillStyle":"acuarela",
         "points":[[0,0],[1,1],"basura"],"lastCommittedPoint":[1,1],
         "boundElements":[{"id":"t","type":"text"},{"id":"z","type":"otro"}],
         "roundness":{"type":2.0},"startBinding":{"focus":0}}],
       "files":{}}"#;
    let (out, _, informe) = escena_para_el_movil(json, &|_, _| None).unwrap();
    como_lo_lee_el_movil(&out).unwrap();
    assert_eq!(informe, Informe::default());
    let v: Value = serde_json::from_str(&out).unwrap();
    let a = &v["elements"][0];
    assert_eq!(
        a["points"].as_array().unwrap().len(),
        2,
        "lo que no es punto se quita"
    );
    assert!(a.get("endArrowhead").is_none() && a.get("fillStyle").is_none());
    assert_eq!(a["boundElements"].as_array().unwrap().len(), 1);
    assert!(a.get("startBinding").is_none());
    // Caso negativo: lo que no es un lienzo no se inventa.
    assert!(escena_para_el_movil("[]", &|_, _| None).is_err());
    assert!(escena_para_el_movil("{\"type\":\"excalidraw\"}", &|_, _| None).is_err());
}
