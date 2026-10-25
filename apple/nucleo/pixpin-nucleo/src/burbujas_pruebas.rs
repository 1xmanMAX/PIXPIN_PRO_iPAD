// Pruebas de `burbujas` (B9): las de `punto_de_proyecto.rs`, `croquis3d.rs`,
// `pdf_en_chat.rs`, `foto_anotada.rs`, `tarjetas.rs` y `ventana_chat.rs` de
// Windows que tocan lo mismo, mas las de la parte que solo tiene el Mac.

use super::*;
use pixpin_proyecto::Hoja;
use serde_json::{Value, json};

fn mensaje(id: &str, clase: Clase) -> Mensaje {
    Mensaje {
        id: id.into(),
        clase: Some(clase),
        ..Default::default()
    }
}

fn carpeta_temporal(etiqueta: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pixpin-burbujas-{etiqueta}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ------------------------------------------------------ punto de proyecto

fn hoja_de(de_mensaje: &str) -> Hoja {
    let mut h = Hoja {
        id: format!("h-{de_mensaje}"),
        ..Default::default()
    };
    h.resto
        .insert("deMensaje".into(), Value::String(de_mensaje.into()));
    h
}

#[test]
fn un_md_mandado_al_chat_de_un_proyecto_sin_unir_sale_rojo() {
    let mut m = mensaje("1790825714183", Clase::Archivo);
    m.ruta = Some("archivos/Objetivos (1).md".into());
    m.nombre = "Objetivos (1).md".into();
    let senas = Senas::de(&[], &["5QX7DBS6BV".into()]);
    assert!(lleva_punto(&m));
    assert!(!senas.esta(&m), "mandarlo al chat no lo une");
}

#[test]
fn lo_que_una_hoja_senala_con_de_mensaje_sale_verde() {
    let p = Proyecto {
        hojas: vec![hoja_de("m1")],
        ..Default::default()
    };
    let senas = Senas::de(&[p], &[]);
    assert!(senas.esta(&mensaje("m1", Clase::Imagen)));
    assert!(
        !senas.esta(&mensaje("m2", Clase::Imagen)),
        "otro mensaje no"
    );
}

#[test]
fn el_mismo_nombre_de_fichero_no_basta_para_salir_verde() {
    let p = Proyecto {
        hojas: vec![Hoja {
            nombre: "plano.pdf".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut m = mensaje("m9", Clase::Archivo);
    m.nombre = "plano.pdf".into();
    m.ruta = Some("archivos/plano.pdf".into());
    assert!(!Senas::de(&[p], &[]).esta(&m));
}

#[test]
fn el_pdf_del_que_nace_un_proyecto_sale_verde_por_su_ruta_exacta() {
    let p = Proyecto {
        pdf_origen: Some("archivos/obra.pdf".into()),
        ..Default::default()
    };
    let senas = Senas::de(&[p], &[]);
    let mut m = mensaje("m1", Clase::Archivo);
    m.ruta = Some("archivos/obra.pdf".into());
    assert!(senas.esta(&m));
    m.ruta = Some("archivos/otra/obra.pdf".into());
    assert!(!senas.esta(&m), "otra ruta con el mismo nombre no");
}

#[test]
fn un_mensaje_que_es_un_proyecto_sale_verde_mientras_el_proyecto_exista() {
    let mut m = mensaje("m1", Clase::Proyecto);
    m.referencia = Some("pr-1".into());
    assert!(Senas::de(&[], &["pr-1".into()]).esta(&m));
    assert!(!Senas::de(&[], &["pr-2".into()]).esta(&m), "borrado: rojo");
}

#[test]
fn una_foto_unida_antes_del_vinculo_se_reconoce_por_su_dibujo() {
    let p = Proyecto {
        hojas: vec![
            Hoja {
                dibujo: Some("foto-m1".into()),
                ..Default::default()
            },
            Hoja {
                dibujo: Some("lienzo-7".into()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let senas = Senas::de(&[p], &[]);
    assert!(
        senas.esta(&mensaje("m1", Clase::Imagen)),
        "sin referencia: foto-<id>"
    );
    let mut d = mensaje("m2", Clase::Dibujo);
    d.referencia = Some("lienzo-7".into());
    assert!(senas.esta(&d), "con referencia: la referencia");
    assert!(!senas.esta(&mensaje("m3", Clase::Imagen)));
}

#[test]
fn una_hoja_que_se_ensena_como_mensaje_sale_verde_por_su_codigo() {
    let p = Proyecto {
        hojas: vec![Hoja {
            uid: Some("H1".into()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut m = mensaje("sintetico", Clase::Pagina);
    m.uid = Some("H1".into());
    assert!(Senas::de(std::slice::from_ref(&p), &[]).esta(&m));
    m.uid = Some("H2".into());
    assert!(!Senas::de(&[p], &[]).esta(&m));
}

#[test]
fn un_croquis_y_una_tabla_salen_verdes_por_su_referencia() {
    let mut h = Hoja {
        croquis: Some("cr-1".into()),
        ..Default::default()
    };
    h.resto.insert("tabla".into(), json!("tb-1"));
    let p = Proyecto {
        hojas: vec![h],
        ..Default::default()
    };
    let senas = Senas::de(&[p], &[]);
    let mut c = mensaje("c", Clase::Otra("CROQUIS".into()));
    c.referencia = Some("cr-1".into());
    assert!(senas.esta(&c));
    let mut t = mensaje("t", Clase::Otra("TABLA".into()));
    t.referencia = Some("tb-1".into());
    assert!(senas.esta(&t));
    t.referencia = Some("tb-2".into());
    assert!(!senas.esta(&t));
}

#[test]
fn la_nota_y_la_voz_no_llevan_punto() {
    assert!(!lleva_punto(&mensaje("n", Clase::Nota)));
    assert!(!lleva_punto(&mensaje("v", Clase::Voz)));
    assert!(!lleva_punto(&mensaje("a", Clase::MiniApp)));
    assert!(lleva_punto(&mensaje("t", Clase::Otra("TABLA".into()))));
    assert!(lleva_punto(&mensaje("c", Clase::Otra("CROQUIS".into()))));
    assert!(!lleva_punto(&mensaje(
        "x",
        Clase::Otra("COSA_NUEVA".into())
    )));
}

#[test]
fn leer_mira_las_hojas_de_todos_los_proyectos_del_disco() {
    let dir = carpeta_temporal("punto");
    std::fs::create_dir_all(dir.join("proyectos/A")).unwrap();
    std::fs::create_dir_all(dir.join("proyectos/B")).unwrap();
    std::fs::write(
        dir.join("proyectos/indice.json"),
        json!({"proyectos": [{"id": "A"}, {"id": "B"}]}).to_string(),
    )
    .unwrap();
    std::fs::write(
        dir.join("proyectos/A/proyecto.json"),
        json!({"id": "A", "hojas": [{"id": "h", "deMensaje": "de-guardados"}]}).to_string(),
    )
    .unwrap();
    assert!(esta(&dir, &mensaje("de-guardados", Clase::Imagen)));
    assert!(!esta(&dir, &mensaje("suelto", Clase::Archivo)));
    assert_eq!(
        punto(&dir, &mensaje("de-guardados", Clase::Imagen)),
        Some(true)
    );
    assert_eq!(punto(&dir, &mensaje("suelto", Clase::Archivo)), Some(false));
    assert_eq!(
        punto(&dir, &mensaje("n", Clase::Nota)),
        None,
        "una nota no lleva"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------- campos del movil

#[test]
fn viene_de_se_lee_de_lo_que_escribe_el_movil() {
    let m: Mensaje = serde_json::from_str(
        r#"{"id":"1","cuando":1,"vieneDe":{"texto":"PDF «GE_Sem16» → página 5","dibujo":"ABC","pagina":4}}"#,
    )
    .unwrap();
    let (texto, dibujo) = viene_de(&m).expect("lo trae");
    assert_eq!(texto, "PDF «GE_Sem16» → página 5");
    assert_eq!(dibujo.as_deref(), Some("ABC"));
}

#[test]
fn sin_viene_de_o_vacio_no_hay_tarjeta() {
    let m: Mensaje = serde_json::from_str(r#"{"id":"1","cuando":1}"#).unwrap();
    assert!(viene_de(&m).is_none());
    let m: Mensaje =
        serde_json::from_str(r#"{"id":"1","cuando":1,"vieneDe":{"texto":"  "}}"#).unwrap();
    assert!(viene_de(&m).is_none());
}

#[test]
fn recibido_de_solo_si_es_un_texto_con_algo() {
    let m: Mensaje = serde_json::from_str(r#"{"id":"1","recibidoDe":"Max phone · K7Q2"}"#).unwrap();
    assert_eq!(recibido_de(&m).as_deref(), Some("Max phone · K7Q2"));
    let vacio: Mensaje = serde_json::from_str(r#"{"id":"1","recibidoDe":"  "}"#).unwrap();
    assert!(recibido_de(&vacio).is_none());
    let raro: Mensaje = serde_json::from_str(r#"{"id":"1","recibidoDe":3}"#).unwrap();
    assert!(recibido_de(&raro).is_none());
}

#[test]
fn solo_la_foto_es_si_salvo_que_diga_que_no() {
    let leer = |t: &str| serde_json::from_str::<Mensaje>(t).unwrap();
    assert!(
        solo_la_foto(&leer(r#"{"id":"1"}"#)),
        "sin el campo: solo la foto"
    );
    assert!(!solo_la_foto(&leer(r#"{"id":"1","soloLaFoto":false}"#)));
    assert!(
        !solo_la_foto(&leer(r#"{"id":"1","soloLaFoto":"no"}"#)),
        "como lo escribe Windows"
    );
    assert!(solo_la_foto(&leer(r#"{"id":"1","soloLaFoto":true}"#)));
}

#[test]
fn los_campos_del_movil_pasan_a_la_burbuja_y_no_se_pierden() {
    let texto = r#"{"id":"1","cuando":5,"clase":"IMAGEN","recibidoDe":"Tel","lienzoIa":"dib-9","soloLaFoto":false,"vieneDe":{"texto":"Lienzo","dibujo":"D1"},"cosaNueva":{"a":1}}"#;
    let m: Mensaje = serde_json::from_str(texto).unwrap();
    let c = crate::chats::MensajeChat::from(m.clone());
    assert_eq!(c.recibido_de.as_deref(), Some("Tel"));
    assert_eq!(c.lienzo_ia.as_deref(), Some("dib-9"));
    assert!(!c.solo_la_foto);
    assert_eq!(c.viene_de.as_deref(), Some("Lienzo"));
    assert_eq!(c.viene_de_dibujo.as_deref(), Some("D1"));
    assert_eq!(c.punto, None, "fuera de una pagina no se mira el disco");
    // Lo que no se entiende se vuelve a escribir tal cual.
    let otra = serde_json::to_value(&m).unwrap();
    assert_eq!(otra["cosaNueva"], json!({"a": 1}));
    assert_eq!(otra["vieneDe"]["dibujo"], "D1");
}

// -------------------------------------------------------- la hoja-pagina

/// Un proyecto `P` con su indice, su `proyecto.json` y un cuaderno.
fn proyecto_de_prueba(etiqueta: &str, hojas: Value, mensajes: &[Value]) -> PathBuf {
    let raiz = carpeta_temporal(etiqueta);
    let carpeta = raiz.join("proyectos/P");
    std::fs::create_dir_all(&carpeta).unwrap();
    std::fs::write(
        raiz.join("proyectos/indice.json"),
        json!({"proyectos": [{"id": "P", "nombre": "Obra"}]}).to_string(),
    )
    .unwrap();
    std::fs::write(
        carpeta.join("proyecto.json"),
        json!({"id": "P", "nombre": "Obra", "hojas": hojas, "cosaDelMovil": 7}).to_string(),
    )
    .unwrap();
    let lineas: Vec<String> = mensajes.iter().map(Value::to_string).collect();
    std::fs::write(carpeta.join("guardados.jsonl"), lineas.join("\n") + "\n").unwrap();
    raiz
}

#[test]
fn una_pagina_sin_dibujo_es_la_que_no_tiene_ni_fichero_ni_referencia() {
    let mut m = mensaje("p", Clase::Pagina);
    m.pagina = Some(3);
    assert!(es_pagina_sin_dibujo(&m));
    m.referencia = Some("dib-1".into());
    assert!(!es_pagina_sin_dibujo(&m));
    m.referencia = None;
    m.ruta = Some("archivos/pagina-04.png".into());
    assert!(!es_pagina_sin_dibujo(&m));
    assert!(
        !es_pagina_sin_dibujo(&mensaje("p", Clase::Pagina)),
        "sin pagina no"
    );
    let mut d = mensaje("d", Clase::Dibujo);
    d.pagina = Some(1);
    assert!(!es_pagina_sin_dibujo(&d));
}

#[test]
fn el_dibujo_de_la_hoja_sale_por_su_codigo_o_por_su_pagina() {
    let raiz = proyecto_de_prueba(
        "dibujo-hoja",
        json!([
            {"id": "h1", "pagina": 2, "dibujo": "dib-2"},
            {"id": "h2", "pagina": 4, "dibujo": "dib-4", "uid": "U4"},
            {"id": "h3", "pagina": 5, "nota": "texto", "dibujo": "dib-nota"}
        ]),
        &[],
    );
    let mut m = mensaje("p", Clase::Pagina);
    m.pagina = Some(2);
    assert_eq!(dibujo_de_la_hoja(&raiz, "P", &m).as_deref(), Some("dib-2"));
    m.uid = Some("U4".into());
    assert_eq!(
        dibujo_de_la_hoja(&raiz, "P", &m).as_deref(),
        Some("dib-4"),
        "manda el codigo"
    );
    m.uid = None;
    m.pagina = Some(5);
    assert_eq!(
        dibujo_de_la_hoja(&raiz, "P", &m),
        None,
        "una hoja nota no es la pagina"
    );
    m.referencia = Some("propio".into());
    assert_eq!(dibujo_de_la_hoja(&raiz, "P", &m).as_deref(), Some("propio"));
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn estrenar_el_dibujo_de_una_pagina_lo_apunta_en_su_hoja_sin_perder_nada() {
    let raiz = proyecto_de_prueba(
        "asegurar",
        json!([{"id": "h1", "pagina": 0, "otro": true}]),
        &[],
    );
    let mut m = mensaje("p", Clase::Pagina);
    m.pagina = Some(0);
    let d = asegurar_dibujo(&raiz, "P", &m, 1_000).expect("se estrena");
    assert_eq!(d, "dib-1000");
    assert!(
        almacen::lienzo(&raiz, "P", &d).is_file(),
        "el fichero va antes"
    );
    let p: Value = serde_json::from_str(
        &std::fs::read_to_string(raiz.join("proyectos/P/proyecto.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(p["hojas"][0]["dibujo"], "dib-1000");
    assert_eq!(
        p["hojas"][0]["otro"], true,
        "lo de la hoja que no se entiende sigue"
    );
    assert_eq!(p["cosaDelMovil"], 7, "lo del proyecto tambien");
    // La segunda vez, el mismo.
    assert_eq!(
        asegurar_dibujo(&raiz, "P", &m, 2_000).as_deref(),
        Some("dib-1000")
    );
    // Una pagina sin hoja no se estrena: no habria donde guardarla.
    m.pagina = Some(9);
    assert_eq!(asegurar_dibujo(&raiz, "P", &m, 3_000), None);
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn la_hoja_de_pagina_dice_su_fondo_y_su_dibujo() {
    let raiz = proyecto_de_prueba(
        "hoja-pagina",
        json!([{"id": "h1", "pagina": 1, "dibujo": "dib-1"}]),
        &[json!({"id": "p1", "cuando": 1, "clase": "PAGINA", "pagina": 1, "nombre": "Plano · 2"})],
    );
    let nucleo = Nucleo::abrir(raiz.display().to_string(), "Mac".into()).unwrap();
    // Sin PDF ni imagen ni dibujo en disco: no hay nada que ensenar.
    assert_eq!(
        nucleo.hoja_de_pagina("P".into(), "p1".into()).unwrap(),
        None
    );
    // La imagen que extraia una version vieja.
    let png = raiz.join("proyectos/P/archivos/pagina-02.png");
    std::fs::create_dir_all(png.parent().unwrap()).unwrap();
    std::fs::write(&png, b"png").unwrap();
    let h = nucleo
        .hoja_de_pagina("P".into(), "p1".into())
        .unwrap()
        .unwrap();
    assert_eq!(
        h.imagen.as_deref(),
        Some(png.display().to_string().as_str())
    );
    assert_eq!(h.pagina, 1);
    assert_eq!(h.dibujo, None, "su dibujo aun no ha llegado");
    // Llega el dibujo y el documento: la imagen vieja sigue mandando.
    std::fs::create_dir_all(raiz.join("proyectos/P/lienzos")).unwrap();
    std::fs::write(almacen::lienzo(&raiz, "P", "dib-1"), "{}").unwrap();
    std::fs::remove_file(&png).unwrap();
    std::fs::write(raiz.join("proyectos/P/documento.pdf"), b"%PDF").unwrap();
    let h = nucleo
        .hoja_de_pagina("P".into(), "p1".into())
        .unwrap()
        .unwrap();
    assert_eq!(h.dibujo.as_deref(), Some("dib-1"));
    assert!(
        h.pdf
            .as_deref()
            .is_some_and(|p| p.ends_with("documento.pdf"))
    );
    assert_eq!(h.imagen, None);
    // Abrirla para dibujar apunta el dibujo en su mensaje.
    let d = nucleo
        .estrenar_dibujo_de_pagina("P".into(), "p1".into())
        .unwrap();
    assert_eq!(d.as_deref(), Some("dib-1"));
    let c = cuaderno::Cuaderno::leer_de(&raiz.join("proyectos/P")).unwrap();
    assert_eq!(c.mensajes[0].referencia.as_deref(), Some("dib-1"));
    // Lo que no es una pagina no se toca.
    assert_eq!(
        nucleo.hoja_de_pagina("P".into(), "no-esta".into()).unwrap(),
        None
    );
    let _ = std::fs::remove_dir_all(&raiz);
}

// --------------------------------------------------------- la foto anotada

fn lienzo_con(elementos: Value) -> String {
    json!({"type": "excalidraw", "elements": elementos, "files": {}}).to_string()
}

#[test]
fn la_foto_anotada_dice_donde_esta_la_foto_dentro_del_lienzo() {
    let raiz = proyecto_de_prueba("foto", json!([]), &[]);
    std::fs::create_dir_all(raiz.join("proyectos/P/lienzos")).unwrap();
    let foto = json!({"id": "foto-1-0", "type": "image", "x": 10, "y": 20, "width": 1600, "height": 1200, "fileId": "F"});
    let raya = json!({"id": "r", "type": "line", "x": 0, "y": 0, "width": 100, "height": 0,
                      "points": [[0, 0], [100, 0]], "strokeColor": "#ff0000"});
    // Solo la foto: nada que ensenar encima.
    std::fs::write(
        almacen::lienzo(&raiz, "P", "foto-m1"),
        lienzo_con(json!([foto.clone()])),
    )
    .unwrap();
    assert_eq!(foto_anotada(&raiz, "P", "foto-m1"), None);
    // Con una raya: la caja de la foto.
    std::fs::write(
        almacen::lienzo(&raiz, "P", "foto-m1"),
        lienzo_con(json!([raya, foto])),
    )
    .unwrap();
    let f = foto_anotada(&raiz, "P", "foto-m1").expect("anotada");
    assert_eq!(f.lienzo, "foto-m1");
    let c = f.foto.expect("lleva la foto");
    assert_eq!((c.x0, c.y0, c.x1, c.y1), (10.0, 20.0, 1610.0, 1220.0));
    // Un lienzo que no ha llegado.
    assert_eq!(foto_anotada(&raiz, "P", "foto-otra"), None);
    let nucleo = Nucleo::abrir(raiz.display().to_string(), "Mac".into()).unwrap();
    assert!(
        nucleo.foto_anotada("P".into(), "../x".into()).is_err(),
        "no se sale de su carpeta"
    );
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn asegurar_el_lienzo_de_una_foto_lo_crea_y_apunta_su_referencia() {
    let raiz = proyecto_de_prueba(
        "foto-asegurar",
        json!([]),
        &[
            json!({"id": "m1", "cuando": 1, "clase": "IMAGEN", "ruta": "archivos/f.png", "nombre": "f.png", "extra": 1}),
        ],
    );
    let png = raiz.join("proyectos/P/archivos/f.png");
    std::fs::create_dir_all(png.parent().unwrap()).unwrap();
    let im = pixpin_codec::imagen::ImagenRgba {
        ancho: 4,
        alto: 2,
        pixeles: vec![200; 32],
    };
    std::fs::write(&png, pixpin_codec::imagen::codificar_png(&im).unwrap()).unwrap();
    let nucleo = Nucleo::abrir(raiz.display().to_string(), "Mac".into()).unwrap();
    let id = nucleo
        .asegurar_lienzo_de_foto("P".into(), "m1".into())
        .unwrap();
    assert_eq!(id, "foto-m1");
    assert!(almacen::lienzo(&raiz, "P", &id).is_file());
    let c = cuaderno::Cuaderno::leer_de(&raiz.join("proyectos/P")).unwrap();
    assert_eq!(c.mensajes[0].referencia.as_deref(), Some("foto-m1"));
    assert_eq!(
        c.mensajes[0].resto["extra"], 1,
        "lo que no se entiende sigue"
    );
    // La segunda vez no se crea otro.
    assert_eq!(
        nucleo
            .asegurar_lienzo_de_foto("P".into(), "m1".into())
            .unwrap(),
        "foto-m1"
    );
    let _ = std::fs::remove_dir_all(&raiz);
}

// ------------------------------------------------------------- croquis

#[test]
fn el_croquis_se_busca_donde_lo_deja_la_sincronizacion() {
    use crate::modelos3d::fichero_del_croquis;
    let raiz = proyecto_de_prueba("croquis", json!([]), &[]);
    assert!(
        fichero_del_croquis(&raiz, "P", "cr-1").is_none(),
        "aun no ha llegado"
    );
    let donde = vista::ruta_real(&raiz, "P", "pixpin:files/croquis3d/cr-1.croquis.gz").unwrap();
    std::fs::create_dir_all(donde.parent().unwrap()).unwrap();
    std::fs::write(
        &donde,
        br##"{"colorDelFondo":"#202830","trazos":[{"id":"t","puntos":[{"x":1,"y":2,"z":3},{"x":5,"y":2,"z":3}],"color":"#ffffff","grosor":4}]}"##,
    )
    .unwrap();
    assert_eq!(fichero_del_croquis(&raiz, "P", "cr-1"), Some(donde.clone()));
    assert!(fichero_del_croquis(&raiz, "P", "../../x").is_none());
    assert!(fichero_del_croquis(&raiz, "P", "a/b").is_none());
    assert!(fichero_del_croquis(&raiz, "P", "  ").is_none());
    let nucleo = Nucleo::abrir(raiz.display().to_string(), "Mac".into()).unwrap();
    assert!(nucleo.croquis_del_chat("P".into(), "cr-1".into()).is_some());
    let m = nucleo.abrir_croquis("P".into(), "cr-1".into()).unwrap();
    assert!(!m.resumen().vacio);
    let x = m.extras_croquis().expect("es un croquis");
    assert!(x.fondo.is_some());
    assert!(!x.claro, "fondo oscuro, abre oscuro");
    let e = x.encuadre.expect("hay algo dibujado");
    assert!(
        (e.x1 - e.x0 - 4.0).abs() < 1e-3,
        "la caja de lo dibujado: {e:?}"
    );
    assert!(nucleo.abrir_croquis("P".into(), "no-esta".into()).is_err());
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn encuadrar_desde_una_orientacion_mete_la_caja_y_respeta_el_giro() {
    use crate::modelos3d::{Caja3d, camara_encuadrar_desde, camara_matriz};
    let caja = Caja3d {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        x1: 40.0,
        y1: 10.0,
        z1: 12.0,
    };
    let c = camara_encuadrar_desde(caja, 800, 600, 1.0, 0.3);
    assert_eq!((c.rumbo, c.altura), (1.0, 0.3));
    let m = camara_matriz(c, 800, 600, 25.0);
    for i in 0..8 {
        let p = [
            if i & 1 == 0 { 0.0 } else { 40.0 },
            if i & 2 == 0 { 0.0 } else { 10.0 },
            if i & 4 == 0 { 0.0 } else { 12.0 },
        ];
        // `p * M` por filas, como `camara_matriz`.
        let q: Vec<f32> = (0..4)
            .map(|k| p[0] * m[k] + p[1] * m[4 + k] + p[2] * m[8 + k] + m[12 + k])
            .collect();
        let (x, y) = (q[0] / q[3], q[1] / q[3]);
        assert!(
            x.abs() <= 1.0 && y.abs() <= 1.0,
            "esquina {i} fuera: {x} {y}"
        );
    }
    assert_eq!(
        camara_encuadrar_desde(caja, 800, 600, 0.0, 9.0).altura,
        1.5697,
        "no pasa de la planta"
    );
}

// ------------------------------------------------------------- lecciones

#[test]
fn los_adjuntos_de_una_leccion_son_los_de_su_lista_y_los_que_le_responden() {
    use crate::lecciones::adjuntos_de;
    let leer = |t: Value| serde_json::from_value::<Mensaje>(t).unwrap();
    let chat = vec![
        leer(json!({"id": "lec-1", "clase": "ARCHIVO"})),
        leer(json!({"id": "f1", "clase": "IMAGEN"})),
        leer(json!({"id": "f2", "clase": "IMAGEN", "respondeA": "lec-1"})),
        leer(json!({"id": "v1", "clase": "VOZ", "respondeA": "lec-1"})),
        leer(json!({"id": "n1", "clase": "NOTA", "respondeA": "lec-1"})),
        leer(json!({"id": "b1", "clase": "IMAGEN", "respondeA": "lec-1", "enBuzon": true})),
        leer(json!({"id": "otra", "clase": "IMAGEN", "respondeA": "lec-2"})),
    ];
    let ids: Vec<String> = adjuntos_de(&["f1".into(), "f2".into(), "ya-no".into()], "lec-1", &chat)
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(
        ids,
        ["f1", "f2", "v1"],
        "en su orden, sin repetir, sin notas ni buzon"
    );
}
