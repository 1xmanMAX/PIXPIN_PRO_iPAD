//! Las pruebas del editor web de notas: las de `lector_web.rs` de Windows y
//! una sesion entera contra un almacen de verdad.

use super::*;

fn nucleo(n: &str) -> (PathBuf, Arc<Nucleo>) {
    let d = std::env::temp_dir().join(format!("pixpin-nota-web-{n}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let nu = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
    (d, nu)
}

fn ordenes_mandar(v: &[OrdenNota]) -> Vec<serde_json::Value> {
    v.iter()
        .filter_map(|o| match o {
            OrdenNota::Mandar { json } => serde_json::from_str(json).ok(),
            _ => None,
        })
        .collect()
}

#[test]
fn los_mensajes_de_la_pagina_se_entienden() {
    let m: DeLaPagina =
        serde_json::from_str(r##"{"tipo":"guardar","texto":"# Hola","version":3}"##).unwrap();
    assert_eq!(
        m,
        DeLaPagina::Guardar {
            texto: "# Hola".into(),
            version: 3
        }
    );
    let v: DeLaPagina = serde_json::from_str(
        r#"{"tipo":"vista","cuerpo":"Nunito","titulos":"Fraunces","px":18,"solo_esta":true}"#,
    )
    .unwrap();
    assert!(matches!(
        v,
        DeLaPagina::Vista {
            px: 18,
            solo_esta: true,
            ..
        }
    ));
    let w: DeLaPagina = serde_json::from_str(r#"{"tipo":"ventana","accion":"minimizar"}"#).unwrap();
    assert!(matches!(w, DeLaPagina::Ventana { ref borde, .. } if borde.is_empty()));
    assert_eq!(
        serde_json::from_str::<DeLaPagina>(r#"{"tipo":"cerrar_listo"}"#).unwrap(),
        DeLaPagina::CerrarListo
    );
    // Caso negativo: un tipo que no existe no se toma por otro.
    assert!(serde_json::from_str::<DeLaPagina>(r#"{"tipo":"borrar_todo"}"#).is_err());
}

#[test]
fn solo_se_abren_fuera_paginas_y_correo() {
    assert!(es_enlace_de_fuera("https://example.com/a"));
    assert!(es_enlace_de_fuera("mailto:a@b.c"));
    // Casos negativos: un programa, un fichero o la propia pagina.
    assert!(!es_enlace_de_fuera("file:///Applications/Calculator.app"));
    assert!(!es_enlace_de_fuera("/bin/sh"));
    assert!(!es_enlace_de_fuera("pixpin-nota://nota/index.html"));
}

#[test]
fn el_renglon_adjuntado_es_un_medio_del_movil() {
    assert_eq!(
        renglon_de("plano [v2].pdf", "pixpin:files/x/notas/1-plano_v2_.pdf"),
        "![plano v2.pdf](pixpin:files/x/notas/1-plano_v2_.pdf)"
    );
    assert_eq!(tipo_de(Path::new("a/voz.M4A")), "audio/mp4");
    assert_eq!(tipo_de(Path::new("a/b.xyz")), "application/octet-stream");
}

#[test]
fn la_pagina_va_dentro_entera_y_hablando_con_webkit() {
    let t = nota_pagina();
    assert!(t.starts_with("<!doctype html>"));
    assert!(t.contains("pastilla-titulo") && t.trim_end().ends_with("</html>"));
    // El puente va antes que el programa de la pagina.
    let puente = t.find("webkit.messageHandlers.pixpin").unwrap();
    let programa = t.find("chrome.webview.postMessage(JSON.stringify").unwrap();
    assert!(puente < programa);
    // Las fotos, por el esquema del Mac; y las teclas con sus simbolos.
    assert!(t.contains("pixpin-nota://nota/archivo?r="));
    assert!(!t.contains("https://pixpin.nota/"));
    assert!(t.contains("\"\u{2318}B\"") && t.contains("\"\u{21e7}\u{2318}S\""));
    assert!(t.contains("\"\u{2325}\u{2318}M\"") && t.contains("\"\u{2318}\u{21a9}\""));
    assert!(!t.contains("\"Ctrl+"), "queda algun Ctrl+ en los menus");
    // Caso negativo: los atajos de CodeMirror (`Ctrl-…`) no se tocan.
    assert!(t.contains("\"Ctrl-ArrowUp\""));
}

#[test]
fn las_peticiones_y_lo_pegado_se_descodifican() {
    assert_eq!(
        parametro("archivo?r=pixpin%3Afiles%2Fa%20b%2F%C3%B1.png", "r").as_deref(),
        Some("pixpin:files/a b/ñ.png")
    );
    assert_eq!(parametro("archivo?x=1", "r"), None);
    assert_eq!(de_base64("SG9sYQ==").unwrap(), b"Hola");
    assert_eq!(de_base64("SG9sYSE").unwrap(), b"Hola!");
    // Caso negativo: lo que no es base64 no se inventa.
    assert!(de_base64("no*vale").is_none());
}

#[test]
fn un_mensaje_del_chat_entra_como_su_enlace() {
    assert_eq!(
        renglon_de_mensaje("\n  Losa [planta] lista\nmas", "P123", "M456"),
        "[Losa planta lista](pixpin:mensaje=P123/M456)"
    );
    assert_eq!(
        mensaje_del_enlace("pixpin:mensaje=P123/M456"),
        Some(("P123".into(), "M456".into()))
    );
    assert_eq!(mensaje_del_enlace("pixpin:mensaje=P123"), None);
    assert_eq!(renglon_de_mensaje("", "P", "M"), "[M](pixpin:mensaje=P/M)");
}

#[test]
fn una_sesion_guarda_la_nota_en_el_chat_y_ve_lo_que_llega_de_fuera() {
    let (d, n) = nucleo("sesion");
    let chat = n.crear_chat("Obra".into()).unwrap();
    let m = n
        .escribir_nota(chat.id.clone(), "# Visita\n- [ ] curado".into(), None)
        .unwrap();
    let s = n
        .clone()
        .abrir_nota(
            DestinoNota::Mensaje {
                chat: chat.id.clone(),
                mensaje: m.id.clone(),
            },
            false,
        )
        .unwrap();
    assert_eq!(s.titulo(), "Visita");
    // Al estar lista, la pagina recibe el texto y la vista de siempre.
    let abrir = ordenes_mandar(&s.recibir(r#"{"tipo":"listo"}"#.into()));
    assert_eq!(abrir[0]["tipo"], "abrir");
    assert_eq!(abrir[0]["texto"], "# Visita\n- [ ] curado");
    assert_eq!(abrir[0]["vista"]["cuerpo"], "Work Sans");
    assert_eq!(abrir[0]["capacidades"]["medios"], true);
    // Guardar escribe en el cuaderno y avisa a la burbuja.
    let r =
        s.recibir(r##"{"tipo":"guardar","texto":"# Visita\n- [x] curado","version":4}"##.into());
    assert_eq!(ordenes_mandar(&r)[0]["ok"], true);
    assert_eq!(ordenes_mandar(&r)[0]["version"], 4);
    assert!(
        r.iter().any(
            |o| matches!(o, OrdenNota::Guardada { mensaje: Some(x) } if x.texto.contains("[x]"))
        )
    );
    let pagina = n.ultimos_mensajes(chat.id.clone(), 10).unwrap();
    assert_eq!(
        pagina.mensajes.last().unwrap().texto,
        "# Visita\n- [x] curado"
    );
    // Lo guardado aqui no vuelve como «de fuera».
    assert_eq!(s.mirar_fuera(), None);
    // Otro editor (u otro aparato) la cambia: llega una vez.
    n.editar_nota(chat.id.clone(), m.id.clone(), "# Visita\nlisto".into())
        .unwrap();
    let fuera: serde_json::Value = serde_json::from_str(&s.mirar_fuera().unwrap()).unwrap();
    assert_eq!(fuera["tipo"], "texto_externo");
    assert_eq!(fuera["texto"], "# Visita\nlisto");
    assert_eq!(s.mirar_fuera(), None);
    // Cerrar: primero la pagina guarda («cerrando») y luego se cierra.
    assert_eq!(
        ordenes_mandar(&s.recibir(r#"{"tipo":"cerrar"}"#.into()))[0]["tipo"],
        "cerrando"
    );
    assert_eq!(
        s.recibir(r#"{"tipo":"cerrar_listo"}"#.into()),
        vec![OrdenNota::Cerrar]
    );
    // Caso negativo: lo que no se entiende no hace nada.
    assert!(s.recibir("{basura".into()).is_empty());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn la_letra_se_recuerda_por_nota_o_para_todas() {
    let (d, n) = nucleo("vista");
    let chat = n.crear_chat("Obra".into()).unwrap();
    let a = n.escribir_nota(chat.id.clone(), "A".into(), None).unwrap();
    let b = n.escribir_nota(chat.id.clone(), "B".into(), None).unwrap();
    let abrir = |id: &str| {
        n.clone()
            .abrir_nota(
                DestinoNota::Mensaje {
                    chat: chat.id.clone(),
                    mensaje: id.into(),
                },
                false,
            )
            .unwrap()
    };
    let sa = abrir(&a.id);
    sa.recibir(
        r#"{"tipo":"vista","cuerpo":"Nunito","titulos":"Caveat","px":99,"solo_esta":true}"#.into(),
    );
    let va =
        ordenes_mandar(&abrir(&a.id).recibir(r#"{"tipo":"listo"}"#.into()))[0]["vista"].clone();
    assert_eq!(va["cuerpo"], "Nunito");
    assert_eq!(va["px"], vista::TAMANO_MAXIMO);
    assert_eq!(va["soloEsta"], true);
    // La otra nota sigue con la de siempre.
    let vb =
        ordenes_mandar(&abrir(&b.id).recibir(r#"{"tipo":"listo"}"#.into()))[0]["vista"].clone();
    assert_eq!(vb["cuerpo"], "Work Sans");
    assert!(d.join(FICHERO_VISTA).is_file());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn lo_pegado_se_copia_junto_a_la_nota_y_se_sirve_a_la_pagina() {
    let (d, n) = nucleo("pegar");
    let chat = n.crear_chat("Obra".into()).unwrap();
    let m = n
        .escribir_nota(chat.id.clone(), "Fotos".into(), None)
        .unwrap();
    let s = n
        .clone()
        .abrir_nota(
            DestinoNota::Mensaje {
                chat: chat.id.clone(),
                mensaje: m.id,
            },
            false,
        )
        .unwrap();
    // «Hola» en base64, pegado como una captura.
    let r = ordenes_mandar(&s.recibir(
        r#"{"tipo":"adjuntar","id":7,"nombre":"Captura 1.png","datos":"SG9sYQ=="}"#.into(),
    ));
    assert_eq!(r[0]["id"], 7);
    assert_eq!(r[0]["error"], false);
    let md = r[0]["md"].as_str().unwrap().to_string();
    assert!(md.starts_with("![Captura 1.png](pixpin:files/"), "{md}");
    let ruta = md
        .trim_end_matches(')')
        .split("](")
        .nth(1)
        .unwrap()
        .to_string();
    let enc: String = ruta.bytes().map(|b| format!("%{b:02X}")).collect();
    let rec = s.servir(format!("{ORIGEN}archivo?r={enc}")).unwrap();
    assert_eq!(rec.datos, b"Hola");
    assert_eq!(rec.tipo, "image/png");
    // Elegidos con el panel: el mismo sitio.
    let origen = d.join("planta.jpg");
    std::fs::write(&origen, b"jpg").unwrap();
    let j: serde_json::Value = serde_json::from_str(
        &s.adjuntar_ficheros(vec![origen.display().to_string()])
            .unwrap(),
    )
    .unwrap();
    assert_eq!(j["tipo"], "insertar");
    assert!(
        j["texto"]
            .as_str()
            .unwrap()
            .starts_with("![planta.jpg](pixpin:files/")
    );
    // Casos negativos: otra peticion, una ruta que se sale, nada elegido.
    assert!(s.servir("index.html".into()).is_none());
    assert!(s.servir("archivo?r=..%2F..%2Fsecreto".into()).is_none());
    assert!(s.adjuntar_ficheros(vec!["/no/esta.png".into()]).is_none());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn los_comentarios_viajan_en_su_fichero_hermano() {
    let (d, n) = nucleo("comentarios");
    let chat = n.crear_chat("Obra".into()).unwrap();
    let m = n
        .escribir_nota(chat.id.clone(), "Hola mundo bonito".into(), None)
        .unwrap();
    let s = n
        .clone()
        .abrir_nota(
            DestinoNota::Mensaje {
                chat: chat.id.clone(),
                mensaje: m.id.clone(),
            },
            false,
        )
        .unwrap();
    let r = ordenes_mandar(&s.recibir(
        r#"{"tipo":"comentario_nuevo","texto":"Hola mundo bonito","desde":5,"hasta":10,"cuerpo":"¿seguro?"}"#.into(),
    ));
    assert_eq!(r[0]["tipo"], "comentarios");
    assert_eq!(r[0]["abiertos"], 1);
    assert!(r[0]["nuevo"].is_string());
    assert_eq!(s.abiertos(), 1);
    // Donde lo buscan Windows y el movil: `anot-<codigo>.comentarios.json`.
    let codigo = mensaje_de(&d, &chat.id, &m.id).unwrap().codigo_unico();
    let f = pixpin_proyecto::comentarios_de_notas::de_la_nota(&d, &chat.id, &codigo).unwrap();
    assert!(f.is_file(), "{}", f.display());
    assert!(f.display().to_string().ends_with(".comentarios.json"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn un_md_suelto_se_edita_en_su_sitio_y_exporta_a_word() {
    let (d, n) = nucleo("md");
    let md = d.join("apuntes.md");
    std::fs::write(&md, "# Obra: fase 1\n\nTexto").unwrap();
    let s = n
        .clone()
        .abrir_nota(
            DestinoNota::Fichero {
                ruta: md.display().to_string(),
            },
            false,
        )
        .unwrap();
    assert_eq!(s.fichero(), Some(md.display().to_string()));
    let abrir = ordenes_mandar(&s.recibir(r#"{"tipo":"listo"}"#.into()));
    assert_eq!(abrir[0]["nombre"], "apuntes.md");
    // Sin chat no hay «Del chat».
    assert_eq!(abrir[0]["capacidades"]["medios"], false);
    s.recibir(r##"{"tipo":"guardar","texto":"# Obra: fase 2","version":1}"##.into());
    assert_eq!(std::fs::read_to_string(&md).unwrap(), "# Obra: fase 2");
    let w = s.recibir(
        r##"{"tipo":"exportar_word","texto":"# Obra: fase 2","titulo":"Obra: fase 2"}"##.into(),
    );
    match &w[..] {
        [
            OrdenNota::GuardarComo {
                nombre,
                extension,
                datos,
            },
        ] => {
            assert_eq!(extension, "docx");
            assert_eq!(nombre, "Obra_ fase 2");
            assert!(datos.starts_with(b"PK"));
        }
        otra => panic!("se esperaba el .docx: {otra:?}"),
    }
    let c = s.recibir(r##"{"tipo":"guardar_copia","texto":"x","titulo":"Plan: <a>"}"##.into());
    assert!(
        matches!(&c[..], [OrdenNota::GuardarComo { nombre, extension, .. }] if nombre == "Plan a" && extension == "md")
    );
    // Casos negativos: un `.md` que no esta no se abre, y un enlace que
    // no es de fuera no sale.
    assert!(
        n.clone()
            .abrir_nota(
                DestinoNota::Fichero {
                    ruta: d.join("no.md").display().to_string()
                },
                false
            )
            .is_err()
    );
    assert!(
        s.recibir(r#"{"tipo":"abrir_enlace","url":"file:///etc/passwd"}"#.into())
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn del_chat_se_ofrecen_los_mensajes_y_entra_su_enlace() {
    let (d, n) = nucleo("delchat");
    let chat = n.crear_chat("Obra".into()).unwrap();
    let otro = n
        .escribir_nota(chat.id.clone(), "Comprar [cemento]\nmas".into(), None)
        .unwrap();
    let m = n
        .escribir_nota(chat.id.clone(), "La nota".into(), None)
        .unwrap();
    let s = n
        .clone()
        .abrir_nota(
            DestinoNota::Mensaje {
                chat: chat.id.clone(),
                mensaje: m.id.clone(),
            },
            false,
        )
        .unwrap();
    let lista =
        ordenes_mandar(&s.recibir(r#"{"tipo":"elegir_mensaje"}"#.into()))[0]["lista"].clone();
    let filas = lista.as_array().unwrap();
    // La propia nota no se ofrece.
    assert_eq!(filas.len(), 1);
    assert_eq!(filas[0]["rotulo"], "Comprar cemento");
    let clave = filas[0]["clave"].as_str().unwrap().to_string();
    assert_eq!(
        clave,
        mensaje_de(&d, &chat.id, &otro.id).unwrap().codigo_unico()
    );
    let ins = ordenes_mandar(&s.recibir(format!(
        r#"{{"tipo":"insertar_mensaje","clave":"{clave}"}}"#
    )));
    let t = ins[0]["texto"].as_str().unwrap();
    assert!(t.starts_with("[Comprar cemento](pixpin:mensaje="), "{t}");
    assert!(t.ends_with(&format!("/{clave})")));
    // Y ese enlace abre su mensaje en su chat.
    let url = t
        .split("](")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .to_string();
    let a = s.recibir(json!({"tipo": "abrir_pixpin", "ruta": url}).to_string());
    assert_eq!(
        a,
        vec![OrdenNota::AbrirMensaje {
            chat: chat.id.clone(),
            codigo: clave
        }]
    );
    let _ = std::fs::remove_dir_all(&d);
}
