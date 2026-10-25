//! Las pruebas del puente: el protocolo del navegador, la traduccion a
//! pedidos y una vuelta entera con un cartero de mentira (nunca la PixPin
//! del usuario).

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

use crate::protocolo::{ErrorLectura, TOPE_ENTRADA, escribir_mensaje, leer_mensaje};
use crate::traducir::{self, Mensaje, traducir};
use crate::*;

/// Un mensaje con su largo delante, como lo manda Chrome.
fn con_largo(json: &[u8]) -> Vec<u8> {
    let mut v = (json.len() as u32).to_ne_bytes().to_vec();
    v.extend_from_slice(json);
    v
}

fn carpeta(etiqueta: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pixpin-puente-{etiqueta}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// Un PNG de 1×1 de verdad.
const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

fn mensaje(v: Value) -> Mensaje {
    serde_json::from_value(v).unwrap()
}

fn pedido(plan: &traducir::Plan) -> Value {
    serde_json::from_str(&plan.pedido).unwrap()
}

// ------------------------------------------------------------ protocolo

#[test]
fn un_mensaje_va_y_vuelve_con_su_largo() {
    let json = br#"{"accion":"tarea","texto":"comprar yeso \u00f1"}"#;
    let mut salida = Vec::new();
    escribir_mensaje(&mut salida, json).unwrap();
    assert_eq!(&salida[..4], &(json.len() as u32).to_ne_bytes());
    assert_eq!(leer_mensaje(&mut Cursor::new(salida), 1024).unwrap(), json);
}

#[test]
fn caso_negativo_entrada_vacia_corta_o_de_largo_cero() {
    assert_eq!(
        leer_mensaje(&mut Cursor::new(vec![]), 1024),
        Err(ErrorLectura::Fin)
    );
    assert_eq!(
        leer_mensaje(&mut Cursor::new(vec![5, 0]), 1024),
        Err(ErrorLectura::Corto {
            esperados: 4,
            llegados: 2
        })
    );
    let mut corto = con_largo(b"{\"a\":1}");
    corto.truncate(7);
    assert_eq!(
        leer_mensaje(&mut Cursor::new(corto), 1024),
        Err(ErrorLectura::Corto {
            esperados: 7,
            llegados: 3
        })
    );
    assert_eq!(
        leer_mensaje(&mut Cursor::new(vec![0, 0, 0, 0]), 1024),
        Err(ErrorLectura::Vacio)
    );
}

#[test]
fn caso_negativo_un_mensaje_enorme_se_rechaza_sin_leerlo() {
    // Un largo de 4 GB con solo 4 bytes detras: si se reservara antes de
    // mirar el tope, la prueba se quedaria sin memoria.
    let r = leer_mensaje(
        &mut Cursor::new(u32::MAX.to_ne_bytes().to_vec()),
        TOPE_ENTRADA,
    );
    assert_eq!(r, Err(ErrorLectura::Enorme(u32::MAX as usize)));
    let justo = (TOPE_ENTRADA as u32 + 1).to_ne_bytes().to_vec();
    assert!(matches!(
        leer_mensaje(&mut Cursor::new(justo), TOPE_ENTRADA),
        Err(ErrorLectura::Enorme(_))
    ));
}

#[test]
fn caso_negativo_una_respuesta_de_mas_de_un_mega_no_se_escribe() {
    let mut salida = Vec::new();
    assert!(escribir_mensaje(&mut salida, &vec![b'a'; protocolo::TOPE_SALIDA + 1]).is_err());
    assert!(salida.is_empty());
}

// ------------------------------------------------------------ traducir

#[test]
fn la_seleccion_va_a_tarea_nota_chat_leccion_voz_y_asistente() {
    let c = Path::new(r"C:\cache\navegador");
    let base = json!({"texto": "  Comprar   yeso\n y arena ", "url": "https://obra.pe/a",
                      "titulo": "Obra  nueva"});
    let con = |accion: &str| {
        let mut v = base.clone();
        v["accion"] = json!(accion);
        let plan = traducir(&mensaje(v), c, "1").unwrap();
        assert!(plan.ficheros.is_empty(), "{accion}");
        assert_eq!(
            crate::mensajero::validar_pedido(&plan.pedido),
            crate::mensajero::respuesta::ACEPTADO,
            "{accion}"
        );
        pedido(&plan)
    };
    assert_eq!(
        con("tarea"),
        json!({"pixpin":1,"accion":"anadir_tarea","texto":"Comprar yeso y arena — https://obra.pe/a"})
    );
    assert_eq!(
        con("nota"),
        json!({"pixpin":1,"accion":"nota_nueva",
               "texto":"Comprar   yeso\n y arena\n\n[Obra nueva](https://obra.pe/a)"})
    );
    assert_eq!(
        con("chat"),
        json!({"pixpin":1,"accion":"chat","texto":"Comprar   yeso\n y arena\n\nObra nueva\nhttps://obra.pe/a"})
    );
    assert_eq!(
        con("leccion")["texto"],
        "Comprar   yeso\n y arena\n\nFuente: Obra nueva https://obra.pe/a"
    );
    assert_eq!(con("leccion")["accion"], "leccion_nueva");
    assert_eq!(
        con("leer"),
        json!({"pixpin":1,"accion":"leer_en_voz","texto":"Comprar   yeso\n y arena","titulo":"Obra nueva"})
    );
    let a = con("asistente");
    assert_eq!(a["accion"], "asistente");
    assert_eq!(
        a["texto"],
        "Explícame este texto\n\n«Comprar   yeso\n y arena»\n\n(De «Obra nueva», https://obra.pe/a)"
    );
    // Con pregunta escrita, va esa.
    let mut v = base.clone();
    v["accion"] = json!("asistente");
    v["pregunta"] = json!("¿Cuánto cuesta?");
    let p = pedido(&traducir(&mensaje(v), c, "1").unwrap());
    assert!(
        p["texto"]
            .as_str()
            .unwrap()
            .starts_with("¿Cuánto cuesta?\n\n«")
    );
}

#[test]
fn caso_negativo_sin_seleccion_accion_desconocida_o_version_nueva() {
    let c = Path::new("c");
    for accion in ["tarea", "nota", "chat", "leccion", "leer", "asistente"] {
        let r = traducir(&mensaje(json!({"accion": accion, "texto": "  \n"})), c, "1");
        assert_eq!(r.unwrap_err(), "No hay texto seleccionado", "{accion}");
    }
    let r = traducir(
        &mensaje(json!({"accion": "borrar_todo", "texto": "x"})),
        c,
        "1",
    );
    assert!(r.unwrap_err().contains("borrar_todo"));
    let r = traducir(
        &mensaje(json!({"v": 2, "accion": "tarea", "texto": "x"})),
        c,
        "1",
    );
    assert!(r.unwrap_err().contains("actualiza"));
}

#[test]
fn una_imagen_se_guarda_en_la_cache_y_se_pinea_va_al_chat_o_al_ocr() {
    let c = Path::new(r"C:\cache\navegador");
    let img = json!({"tipo": "image/png", "base64": PNG_1X1});
    let con = |accion: &str| {
        traducir(
            &mensaje(
                json!({"accion": accion, "imagen": img, "url": "https://x.pe/p", "titulo": "P"}),
            ),
            c,
            "77-1",
        )
        .unwrap()
    };
    let ruta = c.join("nav-77-1.png");
    let plan = con("pinear_imagen");
    assert_eq!(plan.ficheros.len(), 1);
    assert_eq!(plan.ficheros[0].0, ruta);
    assert!(plan.ficheros[0].1.starts_with(b"\x89PNG"));
    assert_eq!(
        pedido(&plan),
        json!({"pixpin":1,"accion":"pinear","ruta":ruta})
    );
    assert_eq!(
        pedido(&con("ocr")),
        json!({"pixpin":1,"accion":"ocr","ruta":ruta})
    );
    for accion in ["imagen_al_chat", "captura_pagina"] {
        assert_eq!(
            pedido(&con(accion)),
            json!({"pixpin":1,"accion":"chat","texto":"P\nhttps://x.pe/p","imagenes":[ruta]})
        );
    }
}

#[test]
fn caso_negativo_imagen_que_falta_no_es_lo_que_dice_o_no_se_admite() {
    let c = Path::new("c");
    let con = |img: Value| {
        traducir(
            &mensaje(json!({"accion": "pinear_imagen", "imagen": img})),
            c,
            "1",
        )
    };
    assert_eq!(
        traducir(&mensaje(json!({"accion": "ocr"})), c, "1").unwrap_err(),
        "No llegó la imagen"
    );
    // Un «PNG» que en realidad es un ejecutable.
    let exe = "TVqQAAMAAAAEAAAA";
    assert!(
        con(json!({"tipo": "image/png", "base64": exe}))
            .unwrap_err()
            .contains("de verdad")
    );
    assert!(
        con(json!({"tipo": "image/svg+xml", "base64": "PHN2Zz4="}))
            .unwrap_err()
            .contains("no admitido")
    );
    assert!(
        con(json!({"tipo": "image/png", "base64": "no es base64!"}))
            .unwrap_err()
            .contains("rota")
    );
}

#[test]
fn la_pagina_va_al_chat_como_enlace_y_al_movil_como_pagina_html() {
    let c = Path::new(r"C:\cache\navegador");
    let p = traducir(
        &mensaje(json!({"accion": "enlace_al_chat", "url": "https://a.pe", "titulo": "A"})),
        c,
        "1",
    )
    .unwrap();
    assert_eq!(
        pedido(&p),
        json!({"pixpin":1,"accion":"chat","texto":"A\nhttps://a.pe"})
    );

    let p = traducir(
        &mensaje(
            json!({"accion": "pagina_al_movil", "url": "https://a.pe/x?q=1&b=2",
                        "titulo": "Planos: «obra» <nueva>?"}),
        ),
        c,
        "9",
    )
    .unwrap();
    let ruta = c.join("nav-9").join("Planos «obra» nueva.html");
    assert_eq!(
        pedido(&p),
        json!({"pixpin":1,"accion":"enviar_al_movil","aparato":"todos","archivos":[ruta]})
    );
    let html = String::from_utf8(p.ficheros[0].1.clone()).unwrap();
    assert!(
        html.contains("content=\"0;url=https://a.pe/x?q=1&amp;b=2\""),
        "{html}"
    );
    assert!(html.contains("Planos: «obra» &lt;nueva&gt;?"), "{html}");

    // Con un movil elegido, a ese.
    let p = traducir(
        &mensaje(json!({"accion": "pagina_al_movil", "url": "http://a.pe", "aparato": "tel-1"})),
        c,
        "9",
    )
    .unwrap();
    assert_eq!(pedido(&p)["aparato"], "tel-1");
}

#[test]
fn caso_negativo_al_movil_solo_paginas_web_y_enlace_sin_direccion() {
    let c = Path::new("c");
    for url in [
        "javascript:alert(1)",
        "file:///C:/x.html",
        "chrome://settings",
        "",
    ] {
        let r = traducir(
            &mensaje(json!({"accion": "pagina_al_movil", "url": url})),
            c,
            "1",
        );
        assert!(r.is_err(), "{url}");
    }
    assert!(traducir(&mensaje(json!({"accion": "enlace_al_chat"})), c, "1").is_err());
}

#[test]
fn los_nombres_de_fichero_son_validos_en_windows() {
    assert_eq!(
        traducir::nombre_de_fichero("a/b\\c:d*e?f\"g<h>i|j"),
        "a b c d e f g h i j"
    );
    assert_eq!(traducir::nombre_de_fichero("  Título.  "), "Título");
    assert_eq!(traducir::nombre_de_fichero("CON"), "pagina");
    assert_eq!(traducir::nombre_de_fichero("com1.txt"), "pagina");
    assert_eq!(traducir::nombre_de_fichero("???"), "pagina");
    assert_eq!(
        traducir::nombre_de_fichero(&"x".repeat(300))
            .chars()
            .count(),
        80
    );
}

#[test]
fn un_texto_enorme_se_recorta_hasta_caber_en_un_pedido() {
    let largo = "palabra \"con comillas\"\n".repeat(10_000);
    let plan = traducir(
        &mensaje(json!({"accion": "leer", "texto": largo})),
        Path::new("c"),
        "1",
    )
    .unwrap();
    assert!(plan.recortado);
    assert!(plan.pedido.len() <= crate::mensajero::TOPE_PEDIDO);
    let t = pedido(&plan)["texto"].as_str().unwrap().to_string();
    assert!(t.ends_with('…') && t.starts_with("palabra"));
    // Caso negativo: uno corto no se toca.
    let plan = traducir(
        &mensaje(json!({"accion": "leer", "texto": "hola"})),
        Path::new("c"),
        "1",
    )
    .unwrap();
    assert!(!plan.recortado);
}

#[test]
fn todas_las_acciones_del_puente_dan_pedidos_que_la_app_acepta() {
    // La tabla del puente contra la de la app: un pedido que la ventana no
    // conoce se contestaria «no se entiende» y el usuario no sabria por que.
    let img = json!({"tipo": "image/png", "base64": PNG_1X1});
    for (accion, _) in traducir::ACCIONES {
        let m =
            mensaje(json!({"accion": accion, "texto": "t", "url": "https://a.pe", "imagen": img}));
        let plan = traducir(&m, Path::new("c"), "1").unwrap_or_else(|e| panic!("{accion}: {e}"));
        assert_eq!(
            crate::mensajero::validar_pedido(&plan.pedido),
            crate::mensajero::respuesta::ACEPTADO,
            "{accion}"
        );
    }
}

// ------------------------------------------------------------ una vuelta

/// Un cartero que apunta lo que le mandan y contesta lo que se le diga.
struct DeMentira {
    mandados: Vec<String>,
    respuestas: Vec<isize>,
    arrancada: bool,
}

impl Cartero for DeMentira {
    fn mandar(&mut self, json: &str) -> isize {
        self.mandados.push(json.to_string());
        if self.respuestas.is_empty() {
            1
        } else {
            self.respuestas.remove(0)
        }
    }
    fn arrancar(&mut self) -> Result<(), String> {
        self.arrancada = true;
        Ok(())
    }
    fn esperar(&mut self, _: Duration) {}
}

fn entorno(etiqueta: &str) -> Entorno {
    Entorno {
        raiz: carpeta(etiqueta),
        sello: "5-5".into(),
        ensenar_pedido: false,
    }
}

fn vuelta(entrada: Vec<u8>, e: &Entorno, cartero: &mut DeMentira) -> Value {
    let mut salida = Vec::new();
    assert_eq!(
        correr(&mut Cursor::new(entrada), &mut salida, e, cartero),
        0
    );
    serde_json::from_slice(&leer_mensaje(&mut Cursor::new(salida), 1 << 20).unwrap()).unwrap()
}

#[test]
fn una_vuelta_guarda_la_imagen_manda_el_pedido_y_contesta() {
    let e = entorno("vuelta");
    let mut cartero = DeMentira {
        mandados: vec![],
        respuestas: vec![],
        arrancada: false,
    };
    let m = json!({"accion": "pinear_imagen", "imagen": {"tipo": "image/png", "base64": PNG_1X1}});
    let r = vuelta(con_largo(m.to_string().as_bytes()), &e, &mut cartero);
    assert_eq!(r, json!({"ok": true, "aviso": "Imagen pineada"}));
    let ruta = e.carpeta().join("nav-5-5.png");
    assert!(ruta.is_file());
    assert_eq!(cartero.mandados.len(), 1);
    assert!(cartero.mandados[0].contains("\"pinear\""));
    assert!(!cartero.arrancada);
    let _ = std::fs::remove_dir_all(&e.raiz);
}

#[test]
fn sin_pixpin_abierta_se_arranca_y_se_reintenta() {
    let e = entorno("arranque");
    let mut cartero = DeMentira {
        mandados: vec![],
        respuestas: vec![0, 0, 0, 1],
        arrancada: false,
    };
    let r = vuelta(
        con_largo(br#"{"accion":"tarea","texto":"x"}"#),
        &e,
        &mut cartero,
    );
    assert_eq!(r["ok"], true);
    assert!(cartero.arrancada);
    assert_eq!(cartero.mandados.len(), 4);
}

#[test]
fn caso_negativo_json_roto_enorme_o_app_vieja_contestan_el_error() {
    let e = entorno("errores");
    let mut cartero = DeMentira {
        mandados: vec![],
        respuestas: vec![],
        arrancada: false,
    };
    let r = vuelta(con_largo(b"{esto no es json"), &e, &mut cartero);
    assert_eq!(r["ok"], false);
    assert!(
        r["error"].as_str().unwrap().starts_with("Mensaje roto"),
        "{r}"
    );

    let r = vuelta(
        ((TOPE_ENTRADA + 1) as u32).to_ne_bytes().to_vec(),
        &e,
        &mut cartero,
    );
    assert!(
        r["error"].as_str().unwrap().contains("Demasiado grande"),
        "{r}"
    );
    assert!(cartero.mandados.is_empty());

    // La app no conoce la accion (una PixPin anterior al `ocr`): se dice y
    // la imagen escrita se borra.
    let mut vieja = DeMentira {
        mandados: vec![],
        respuestas: vec![3],
        arrancada: false,
    };
    let m = json!({"accion": "ocr", "imagen": {"tipo": "image/png", "base64": PNG_1X1}});
    let r = vuelta(con_largo(m.to_string().as_bytes()), &e, &mut vieja);
    assert!(r["error"].as_str().unwrap().contains("actualízala"), "{r}");
    assert!(!e.carpeta().join("nav-5-5.png").exists());

    // Sin nada en la entrada, ni se contesta.
    let mut salida = Vec::new();
    assert_eq!(
        correr(&mut Cursor::new(vec![]), &mut salida, &e, &mut cartero),
        0
    );
    assert!(salida.is_empty());
    let _ = std::fs::remove_dir_all(&e.raiz);
}

#[test]
fn caso_negativo_si_pixpin_no_aparece_se_dice() {
    struct Nadie;
    impl Cartero for Nadie {
        fn mandar(&mut self, _: &str) -> isize {
            0
        }
        fn arrancar(&mut self) -> Result<(), String> {
            Err("No se encuentra pixpinmax.exe".into())
        }
    }
    assert_eq!(
        entregar(&mut Nadie, "{}").unwrap_err(),
        "No se encuentra pixpinmax.exe"
    );
}

#[test]
fn la_cache_vieja_se_limpia_y_lo_ajeno_no_se_toca() {
    let d = carpeta("limpiar");
    std::fs::create_dir_all(d.join("nav-1")).unwrap();
    std::fs::write(d.join("nav-1").join("x.html"), b"x").unwrap();
    std::fs::write(d.join("nav-2.png"), b"x").unwrap();
    std::fs::write(d.join("otro.txt"), b"x").unwrap();
    // Con 0 dias, todo lo del puente es viejo... tras un instante.
    std::thread::sleep(Duration::from_millis(20));
    limpiar_viejos(&d, 0);
    assert!(!d.join("nav-1").exists() && !d.join("nav-2.png").exists());
    assert!(d.join("otro.txt").exists());
    // Y con 7, lo recien hecho se queda.
    std::fs::write(d.join("nav-3.png"), b"x").unwrap();
    limpiar_viejos(&d, DIAS_EN_CACHE);
    assert!(d.join("nav-3.png").exists());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn la_raiz_es_la_del_mac_y_la_app_la_de_su_paquete() {
    let d = carpeta("raiz");
    std::fs::create_dir_all(&d).unwrap();
    let e = Entorno::del_equipo(Some(&d), None).unwrap();
    assert!(e.raiz.ends_with("Library/Application Support/PixPinMax"), "{:?}", e.raiz);
    // Dentro de PixPin.app, la app es esa; suelto, la de /Applications.
    let dentro = PathBuf::from("/x/PixPin.app/Contents/MacOS");
    assert_eq!(exe_de_pixpin(Some(&dentro)), Some(PathBuf::from("/x/PixPin.app")));
    assert_eq!(exe_de_pixpin(Some(&d)), Some(PathBuf::from("/Applications/PixPin.app")));
    assert_eq!(
        Entorno::del_equipo(Some(&d), Some(PathBuf::from("z")))
            .unwrap()
            .raiz,
        PathBuf::from("z")
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn lo_que_tarda_el_puente_por_dentro() {
    // Sin el arranque del proceso (que es de Windows: lo mismo tarda
    // `whoami.exe`, ver tests/exe.rs): leer, traducir, escribir y contestar.
    let e = entorno("tiempo");
    let mut cartero = DeMentira {
        mandados: vec![],
        respuestas: vec![],
        arrancada: false,
    };
    let tarea = con_largo(br#"{"accion":"tarea","texto":"comprar yeso","url":"https://a.pe"}"#);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.resize(1_100_000, 7);
    let b64 = {
        // Lo mismo que hace la extension (base64 estandar).
        const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for t in png.chunks(3) {
            let n = (u32::from(t[0]) << 16)
                | (u32::from(*t.get(1).unwrap_or(&0)) << 8)
                | u32::from(*t.get(2).unwrap_or(&0));
            for i in 0..4 {
                s.push(if i <= t.len() {
                    A[(n >> (18 - 6 * i) & 63) as usize] as char
                } else {
                    '='
                });
            }
        }
        s
    };
    let imagen = con_largo(
        json!({"accion":"captura_pagina","url":"https://a.pe","imagen":{"tipo":"image/png","base64":b64}})
            .to_string()
            .as_bytes(),
    );
    let medir = |entrada: &Vec<u8>, cartero: &mut DeMentira| {
        let mut t: Vec<Duration> = (0..5)
            .map(|_| {
                let inicio = std::time::Instant::now();
                vuelta(entrada.clone(), &e, cartero);
                inicio.elapsed()
            })
            .collect();
        t.sort();
        t[2]
    };
    let t_tarea = medir(&tarea, &mut cartero);
    let t_imagen = medir(&imagen, &mut cartero);
    eprintln!("por dentro (depuracion): tarea {t_tarea:?}, imagen de 1,1 MB {t_imagen:?}");
    assert!(t_tarea < Duration::from_millis(50), "{t_tarea:?}");
    let _ = std::fs::remove_dir_all(&e.raiz);
}
