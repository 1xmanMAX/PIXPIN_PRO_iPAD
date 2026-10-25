use super::*;
use serde_json::json;

fn ejemplo() -> ClaseParaExportar {
    let mut c = sin_ordenar();
    c.capturas.sort_by_key(|(ms, _)| *ms);
    c
}

/// Las capturas como llegan de una lista mal guardada: al reves.
fn sin_ordenar() -> ClaseParaExportar {
    ClaseParaExportar {
        titulo: "Clase <de> \"física\" & más".into(),
        fecha: "25/10/2026 18:05".into(),
        duracion_ms: 3_725_000,
        audio: Some(PathBuf::from("/x/clase.m4a")),
        video: None,
        capturas: vec![
            (65_000, PathBuf::from("/x/c1.png")),
            (5_000, PathBuf::from("/x/c0.png")),
        ],
        lineas: vec![
            (Some(0), "Hola a <todos>".into()),
            (Some(60_000), "Vamos con la ley de Ohm".into()),
            (None, "Una línea sin minuto".into()),
            (Some(120_000), "Y la potencia".into()),
        ],
        marcadores: vec![
            Marcador {
                ms: 60_000,
                texto: "Ojo: examen".into(),
            },
            Marcador {
                ms: 3_700_000,
                texto: String::new(),
            },
        ],
        adjuntos: vec![],
    }
}

fn incrustar(_: &Path) -> Option<Medio> {
    Some(Medio::Incrustado {
        mime: "image/png".into(),
        bytes: b"png".to_vec(),
    })
}

#[test]
fn el_minuto_se_lee_como_en_la_burbuja() {
    assert_eq!(minuto(0), "0:00");
    assert_eq!(minuto(65_400), "1:05");
    assert_eq!(minuto(3_725_000), "1:02:05");
    assert_eq!(minuto(-5), "0:00");
}

#[test]
fn las_capturas_y_los_marcadores_caen_en_su_minuto() {
    let c = ejemplo();
    // c0 (0:05) despues de la linea de 0:00; el marcador de 1:00 y la
    // captura de 1:05 detras de la linea de 1:00 (y de la que no lleva
    // minuto, que no mueve a nadie); el de 1:01:40, al final.
    assert_eq!(
        filas(&c),
        vec![
            Fila::Linea(0),
            Fila::Captura(0),
            Fila::Linea(1),
            Fila::Linea(2),
            Fila::Marcador(0),
            Fila::Captura(1),
            Fila::Linea(3),
            Fila::Marcador(1),
        ]
    );
}

#[test]
fn las_capturas_desordenadas_salen_igual_en_su_minuto() {
    let c = sin_ordenar();
    // La de 0:05 (la segunda de la lista) sigue cayendo tras la linea de 0:00.
    assert_eq!(filas(&c)[1], Fila::Captura(1));
}

#[test]
fn el_html_escapa_todo_lo_que_viene_del_usuario() {
    let h = html(&ejemplo(), &Rotulos::default(), &incrustar);
    assert!(h.contains("<title>Clase &lt;de&gt; &quot;física&quot; &amp; más</title>"));
    assert!(h.contains("Hola a &lt;todos&gt;"));
    assert!(!h.contains("<todos>"));
    assert!(!h.contains("<de>"));
    let r = Rotulos {
        idioma: "en\"><script>".into(),
        ..Rotulos::default()
    };
    let h = html(&ejemplo(), &r, &incrustar);
    assert!(h.contains("lang=\"en&quot;&gt;&lt;script&gt;\""));
}

#[test]
fn cada_minuto_lleva_a_donde_salta() {
    let h = html(&ejemplo(), &Rotulos::default(), &incrustar);
    assert!(h.contains(
        "<p class=\"linea\" id=\"t-60000\" data-ms=\"60000\"><a class=\"t\" href=\"#t-60000\" data-ms=\"60000\">1:00</a>"
    ));
    // La linea sin minuto no salta.
    assert!(h.contains("<p class=\"linea sin-tiempo\"><span class=\"dicho\">Una línea sin minuto"));
    // Las senales de la barra, en su sitio (1:00 de 1:02:05).
    assert!(h.contains(
        "class=\"senal marcador\" href=\"#t-60000\" data-ms=\"60000\" style=\"left:1.611%\""
    ));
    assert!(h.contains("class=\"senal captura\" href=\"#captura-1\""));
    // El audio va dentro, y manda.
    assert!(h.contains("<audio id=\"audio\" src=\"data:image/png;base64,cG5n\""));
    assert!(h.contains("data-maestro"));
    // El orden de la transcripcion: la captura de 0:05 antes de 1:00.
    let c0 = h.find("id=\"captura-1\" data-ms=\"5000\"").unwrap();
    let l1 = h.find("id=\"t-60000\"").unwrap();
    let c1 = h.find("id=\"captura-2\" data-ms=\"65000\"").unwrap();
    assert!(c0 < l1 && l1 < c1);
}

#[test]
fn sin_nada_que_oir_los_minutos_no_saltan() {
    let h = html(&ejemplo(), &Rotulos::default(), &|r: &Path| {
        (extension(r) == "png").then(|| Medio::Incrustado {
            mime: "image/png".into(),
            bytes: vec![1],
        })
    });
    assert!(!h.contains("<audio"));
    assert!(!h.contains("href=\"#t-"));
    assert!(h.contains("<span class=\"t\">1:00</span>"));
    assert!(!h.contains("class=\"mandos\""));
}

#[test]
fn la_pagina_no_pide_nada_a_la_red() {
    let mut c = ejemplo();
    c.video = Some(PathBuf::from("/x/Clase 25-10: vídeo.mp4"));
    c.adjuntos = vec![PathBuf::from("/x/apuntes.pdf")];
    let h = html(&c, &Rotulos::default(), &|r: &Path| {
        Some(match extension(r).as_str() {
            "png" | "m4a" => incrustar(r)?,
            _ => Medio::AlLado(format!("../../{}", nombre(r))),
        })
    });
    for prohibido in [
        "http:",
        "https:",
        "src=\"//",
        "href=\"//",
        "@import",
        "url(",
    ] {
        assert!(!h.contains(prohibido), "la pagina lleva {prohibido}");
    }
    // El video, al lado y con su nombre escapado como URL.
    assert!(h.contains("<video id=\"video\" src=\"Clase%2025-10%3A%20v%C3%ADdeo.mp4\""));
    assert!(h.contains("href=\"apuntes.pdf\""));
    // Con audio, el video lo sigue sin sonido.
    assert!(h.contains("preload=\"metadata\" playsinline muted>"));
}

#[test]
fn las_urls_relativas_no_se_salen_ni_cambian_de_esquema() {
    assert_eq!(url_relativa("a b/ñ.png"), "a%20b/%C3%B1.png");
    assert_eq!(url_relativa("../../etc/x"), "etc/x");
    assert_eq!(
        url_relativa("javascript:alert(1)"),
        "javascript%3Aalert%281%29"
    );
    assert_eq!(url_relativa("C:\\x\\y.mp4"), "C%3A/x/y.mp4");
}

#[test]
fn el_markdown_lleva_las_capturas_en_su_minuto() {
    let c = ejemplo();
    let md = markdown(&c, &Rotulos::default(), &|r: &Path| {
        Some(format!("Clase x_archivos/{}", nombre(r)))
    });
    assert!(md.starts_with("# Clase \\<de\\> \"física\" & más\n"));
    assert!(md.contains("**1:00** Vamos con la ley de Ohm"));
    assert!(md.contains("![Captura 2 · 1:05](Clase%20x_archivos/c1.png)"));
    assert!(md.contains("- Audio: [clase.m4a](Clase%20x_archivos/clase.m4a)"));
    assert!(md.contains("## Marcadores\n\n- **1:00** Ojo: examen\n- **1:01:40** Marcador\n"));
    let c1 = md.find("c1.png)\n").unwrap();
    assert!(md.find("**1:00** Vamos").unwrap() < c1);
    assert!(c1 < md.find("**2:00** Y la potencia").unwrap());
}

#[test]
fn el_texto_es_la_transcripcion_con_sus_minutos() {
    let t = texto(&ejemplo(), &Rotulos::default());
    assert!(t.starts_with("Clase <de> \"física\" & más\n25/10/2026 18:05 · Duración 1:02:05\n\n"));
    assert!(t.contains("[0:00] Hola a <todos>\n\n[1:00] Vamos con la ley de Ohm\n\nUna línea sin minuto\n\n[1:00] -- Marcador: Ojo: examen\n\n[2:00] Y la potencia\n\n[1:01:40] -- Marcador\n"));
}

#[test]
fn de_la_nota_y_sus_enlaces() {
    let voz = Mensaje {
        id: "voz".into(),
        clase: Some(Clase::Voz),
        nombre: "Clase 25-10 18h05.m4a".into(),
        ruta: Some("clase.m4a".into()),
        duracion_ms: 90_000,
        resto: json!({
            "grabacion": "clase",
            "capturas": [{"ms": 30_000, "id": "c2"}, {"ms": 1_000, "id": "c1"}, {"ms": 5, "id": "no-esta"}],
            "video": {"id": "v"},
            "marcadores": [{"ms": 40_000, "texto": "importante"}],
            "marcas": [10_000, 40_000],
        })
        .as_object()
        .unwrap()
        .clone(),
        ..Default::default()
    };
    assert!(es_grabacion(&voz) && es_de_clase(&voz));
    let hijo = |id: &str, clase: Clase, ruta: &str, ms: i64| Mensaje {
        id: id.into(),
        clase: Some(clase),
        ruta: Some(ruta.into()),
        resto: json!({"enGrabacion": {"de": "voz", "ms": ms}})
            .as_object()
            .unwrap()
            .clone(),
        ..Default::default()
    };
    let mensajes = vec![
        voz.clone(),
        hijo("c1", Clase::Imagen, "c1.png", 1_000),
        hijo("c2", Clase::Imagen, "c2.png", 30_000),
        hijo("v", Clase::Archivo, "v.mp4", 0),
        // Llegada por la carpeta de la grabacion, sin estar en sus listas.
        hijo("c3", Clase::Imagen, "c3.png", 50_000),
        hijo("pdf", Clase::Archivo, "apuntes.pdf", 0),
        Mensaje {
            id: "otro".into(),
            clase: Some(Clase::Imagen),
            ruta: Some("otro.png".into()),
            ..Default::default()
        },
    ];
    let ruta = |m: &Mensaje| m.ruta.as_deref().map(|r| PathBuf::from("/p").join(r));
    let c = ClaseParaExportar::de_la_nota(
        &voz,
        &mensajes,
        vec![(Some(0), "hola".into())],
        "hoy".into(),
        &ruta,
    );
    assert_eq!(c.titulo, "Clase 25-10 18h05");
    assert_eq!(c.audio, Some(PathBuf::from("/p/clase.m4a")));
    assert_eq!(c.video, Some(PathBuf::from("/p/v.mp4")));
    assert_eq!(
        c.capturas,
        vec![
            (1_000, PathBuf::from("/p/c1.png")),
            (30_000, PathBuf::from("/p/c2.png")),
            (50_000, PathBuf::from("/p/c3.png")),
        ]
    );
    assert_eq!(c.adjuntos, vec![PathBuf::from("/p/apuntes.pdf")]);
    // Las banderitas se suman a los marcadores sin repetir minuto.
    assert_eq!(
        c.marcadores.iter().map(|m| m.ms).collect::<Vec<_>>(),
        vec![10_000, 40_000]
    );
    assert_eq!(c.marcadores[1].texto, "importante");
    assert_eq!(c.ficheros().len(), 6);
    assert_eq!(c.duracion(), 90_000);
}

#[test]
fn el_markdown_va_en_un_zip_con_su_carpeta() {
    let c = ejemplo();
    let zip = paquete_markdown(&c, &Rotulos::default(), "Clase 1", &|r: &Path| {
        (nombre(r) == "c1.png").then(|| b"png".to_vec())
    })
    .unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let mut nombres: Vec<String> = z.file_names().map(String::from).collect();
    nombres.sort();
    assert_eq!(
        nombres,
        vec!["Clase 1.md", "Clase 1_archivos/captura-02-1m05s.png"]
    );
    let mut md = String::new();
    std::io::Read::read_to_string(&mut z.by_name("Clase 1.md").unwrap(), &mut md).unwrap();
    assert!(md.contains("![Captura 2 · 1:05](Clase%201_archivos/captura-02-1m05s.png)"));
    // La que no se pudo leer no se enlaza.
    assert!(!md.contains("Captura 1 ·"));
}
