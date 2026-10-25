//! Las pruebas del visor reflujado: las de Word y libro de
//! `anotado_del_adjunto.rs` de Windows y las de `visor.rs` que no son de su
//! ventana.

use super::*;
use crate::editor::FasePuntero;
use pixpin_proyecto::almacen::{self, Ficha, Indice};
use pixpin_proyecto::cuaderno::{self, Clase as ClaseMensaje, Mensaje, Sello};

fn raiz(etiqueta: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!(
        "pixpin-nucleo-reflujo-{etiqueta}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&r);
    std::fs::create_dir_all(&r).unwrap();
    r
}

/// Un almacen con un adjunto `nombre` en el chat «Tesis» (la receta de
/// `anotado_del_adjunto::pruebas::con_adjunto` de Windows).
fn con_adjunto(r: &Path, nombre: &str, bytes: &[u8]) -> PathBuf {
    let ficha = Ficha::nueva("Tesis", 5, "PC01");
    let mut i = Indice::default();
    i.proyectos.push(ficha.clone());
    i.guardar(r).unwrap();
    let ruta = almacen::guardar_adjunto(r, &ficha.id, nombre, bytes).unwrap();
    let sello = Sello {
        cuando: 10,
        numero: 1,
        aparato: "PC01".into(),
        proyecto: ficha.id.clone(),
    };
    let carpeta = almacen::carpeta(r, &ficha.id);
    cuaderno::anadir(
        &carpeta,
        &Mensaje::adjunto(ClaseMensaje::Archivo, nombre, &ruta, 2, &sello),
    )
    .unwrap();
    carpeta.join(&ruta)
}

fn base(r: &Path, doc: &Path) -> Base {
    base_de(r, doc).expect("es un adjunto del chat")
}

fn nucleo(r: &Path) -> Arc<Nucleo> {
    Nucleo::abrir(r.display().to_string(), "Mac".into()).unwrap()
}

fn trazar(ed: &EditorLienzo, desde: (f32, f32), hasta: (f32, f32)) {
    ed.herramienta("lapiz".into()).unwrap();
    ed.puntero(FasePuntero::Pulsar, desde.0, desde.1, None, 0, 1.0);
    for k in 1..=8 {
        let t = k as f32 / 8.0;
        ed.puntero(
            FasePuntero::Mover,
            desde.0 + (hasta.0 - desde.0) * t,
            desde.1 + (hasta.1 - desde.1) * t,
            None,
            0,
            1.0,
        );
    }
    ed.puntero(FasePuntero::Soltar, hasta.0, hasta.1, None, 0, 1.0);
}

#[test]
fn la_letra_del_movil_es_la_misma_en_el_pc_y_vuelve_igual() {
    // K16: las cuatro letras y los cuatro grosores del movil, tal cual.
    assert_eq!([0, 1, 2, 3].map(grosor_del_movil), [0, 1, 2, 3]);
    assert_eq!([0, 1, 2, 3].map(tipo_del_movil), [0, 1, 2, 3]);
    // Caso negativo: lo que no existe cae en el extremo mas cercano.
    assert_eq!([-1, 9].map(tipo_del_movil), [0, 3]);
    assert_eq!([-5, 7].map(grosor_del_movil), [0, 3]);
}

#[test]
fn los_espacios_de_cada_lado_y_el_verde_de_la_voz_viajan_como_en_el_movil() {
    let r = raiz("lados-y-voz");
    let doc = con_adjunto(&r, "libro.epub", b"PK");
    let b = base(&r, &doc);
    // Del movil: un solo paso a la izquierda y dos a la derecha, y su verde.
    an::escribir(&b.fichero(".maqueta"), "420,140,280,100,1,0").unwrap();
    an::escribir(&b.fichero(".voz"), "12:0.25").unwrap();
    let mut a = leer_ajustes(&r, &doc);
    assert_eq!(a.lados, Some((140, 280)));
    assert_eq!(a.voz, Some((12, 0.25)));
    // Sin cambios no se reescribe nada.
    escribir_ajustes(&r, &doc, &a).unwrap();
    assert_eq!(
        an::leer(&b.fichero(".maqueta")).as_deref(),
        Some("420,140,280,100,1,0")
    );
    assert_eq!(an::leer(&b.fichero(".voz")).as_deref(), Some("12:0.25"));
    // Aqui se quita el paso de la izquierda y el verde avanza.
    a.lados = Some((0, 280));
    a.voz = Some((13, 0.3));
    escribir_ajustes(&r, &doc, &a).unwrap();
    assert_eq!(
        an::leer(&b.fichero(".maqueta")).as_deref(),
        Some("420,0,280,100,1,0")
    );
    assert_eq!(an::leer(&b.fichero(".voz")).as_deref(), Some("13:0.3"));
    // Acabado el documento: vacio, no borrado (borrado seria «no se sabe»).
    a.voz = None;
    escribir_ajustes(&r, &doc, &a).unwrap();
    assert_eq!(an::leer(&b.fichero(".voz")).as_deref(), Some(""));
    assert_eq!(leer_ajustes(&r, &doc).voz, None);
    // Un espacio fuera de lo que cabe se acota al leer.
    an::escribir(&b.fichero(".maqueta"), "420,900,280,100,1,0").unwrap();
    assert_eq!(leer_ajustes(&r, &doc).lados, Some((280, 280)));
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn un_documento_sin_verde_no_crea_su_fichero() {
    let r = raiz("sin-verde");
    let doc = con_adjunto(&r, "acta.docx", b"PK");
    let b = base(&r, &doc);
    escribir_ajustes(&r, &doc, &leer_ajustes(&r, &doc)).unwrap();
    for t in [".voz", ".marcas", ".maqueta", ".sitio"] {
        assert!(
            !b.fichero(t).exists(),
            "{t}: abrir y guardar no crea nada que viaje"
        );
    }
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn lo_que_llega_del_movil_sobre_un_word_se_lee_encima_de_lo_de_aqui() {
    let r = raiz("leer-word");
    let doc = con_adjunto(&r, "tesis.docx", b"PK");
    lectura::escribir(
        &doc,
        &lectura::Ajustes {
            columna: 860,
            zoom: 0.8,
            marcadores: lectura::marcadores_de_texto("7:0.1:X"),
            ..lectura::Ajustes::default()
        },
    )
    .unwrap();
    let b = base(&r, &doc);
    an::escribir(&b.fichero(".maqueta"), "384,256,256,115,3,2").unwrap();
    an::escribir(&b.fichero(".marcas"), "1790218080412:0.34022403:P").unwrap();
    an::escribir(&b.fichero(".sitio"), "0.7799783").unwrap();
    let n = nucleo(&r);
    let l = n.lectura_de_reflujo(doc.display().to_string());
    assert_eq!(l.donde, DondeAnota::Chat);
    assert_eq!(
        (l.columna, l.tamano, l.grosor, l.tipo),
        (384, 115, 3, 2),
        "la columna y la letra del movil"
    );
    assert_eq!(l.marcadores.len(), 1);
    assert_eq!(l.marcadores[0].emoji, "P");
    assert!((l.sitio - 0.7799783).abs() < 1e-6);
    assert_eq!(l.zoom, 0.8, "lo de esta pantalla sigue siendo de aqui");
    // Guardar sin cambiar la letra no reescribe la maqueta del movil: sus
    // margenes, su peso negro y su letra de ancho fijo se quedan.
    n.guardar_lectura_de_reflujo(doc.display().to_string(), l.clone())
        .unwrap();
    assert_eq!(
        an::leer(&b.fichero(".maqueta")).as_deref(),
        Some("384,256,256,115,3,2")
    );
    assert_eq!(
        an::leer(&b.fichero(".marcas")).as_deref(),
        Some("1790218080412:0.34022403:P")
    );
    assert_eq!(an::leer(&b.fichero(".sitio")).as_deref(), Some("0.7799783"));
    // Cambiada aqui, se reescribe con los margenes que tenia.
    let grande = LecturaReflujo {
        tamano: 130,
        ..l.clone()
    };
    n.guardar_lectura_de_reflujo(doc.display().to_string(), grande)
        .unwrap();
    assert_eq!(
        an::leer(&b.fichero(".maqueta")).as_deref(),
        Some("384,256,256,130,3,2")
    );
    // Y sin columna (se quito toda la tinta), vacia, no borrada.
    let suelta = LecturaReflujo {
        columna: 0,
        lados: None,
        ..l
    };
    n.guardar_lectura_de_reflujo(doc.display().to_string(), suelta)
        .unwrap();
    assert_eq!(an::leer(&b.fichero(".maqueta")).as_deref(), Some(""));
    assert_eq!(n.lectura_de_reflujo(doc.display().to_string()).columna, 0);
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn lo_de_antes_de_un_word_pasa_una_vez_a_su_mensaje_y_no_se_borra() {
    let r = raiz("migrar-word");
    let doc = con_adjunto(&r, "acta.docx", b"PK");
    let antes = lectura::Ajustes {
        columna: 860,
        sitio: 0.6239229,
        marcadores: lectura::marcadores_de_texto("1790218080412:0.34022403:P"),
        ..lectura::Ajustes::default()
    };
    lectura::escribir(&doc, &antes).unwrap();
    let _ = leer_ajustes(&r, &doc);
    let b = base(&r, &doc);
    assert_eq!(
        an::leer(&b.fichero(".maqueta")).as_deref(),
        Some("860,573,573,100,1,0")
    );
    assert_eq!(
        an::leer(&b.fichero(".marcas")).as_deref(),
        Some("1790218080412:0.34022403:P")
    );
    assert_eq!(an::leer(&b.fichero(".sitio")).as_deref(), Some("0.6239229"));
    assert!(
        lectura::ruta_de_ajustes(&doc).is_file(),
        "lo viejo se queda"
    );
    // La segunda vez no se pisa lo que ya tiene el mensaje.
    an::escribir(&b.fichero(".sitio"), "0.5").unwrap();
    let _ = leer_ajustes(&r, &doc);
    assert_eq!(an::leer(&b.fichero(".sitio")).as_deref(), Some("0.5"));
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn la_tinta_del_movil_sin_marco_se_corre_su_margen_y_se_le_apunta_el_marco() {
    let r = raiz("corrida");
    let doc = con_adjunto(&r, "libro.epub", b"PK");
    let b = base(&r, &doc);
    an::escribir(&b.fichero(".maqueta"), "384,256,256,100,1,0").unwrap();
    std::fs::create_dir_all(b.tinta().parent().unwrap()).unwrap();
    std::fs::write(
        b.tinta(),
        r#"{"elements":[{"id":"m1","type":"freedraw","x":266,"y":10278.29,"width":4,"height":4,"seed":1,"version":1,"versionNonce":1,"updated":1,"points":[{"x":0.0,"y":0.0},{"x":4.0,"y":4.0}]}],"files":{}}"#,
    )
    .unwrap();
    let n = nucleo(&r);
    let ed = n.anotar_reflujo(doc.display().to_string()).unwrap();
    let caja = ed.caja().expect("el trazo del movil se ve");
    assert!(
        (caja.x0 - 10.0).abs() < 5.0,
        "corrida su izq de 256: {}",
        caja.x0
    );
    // Al abrirla se le apunta el marco de la regla de antes, sin tocar la tinta.
    assert_eq!(an::leer(&b.marco()).as_deref(), Some("256,0,640,384\nv1\n"));
    // Un trazo nuevo en el margen de la izquierda: todo pasa a las unidades
    // del lector y el marco dice que la columna esta en el cero.
    trazar(&ed, (-100.0, 50.0), (-40.0, 50.0));
    assert!(ed.guardar().unwrap());
    assert_eq!(an::leer(&b.marco()).as_deref(), Some("0,0,384,384\nv1\n"));
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(b.tinta()).unwrap()).unwrap();
    let xs: Vec<f64> = v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["x"].as_f64().unwrap())
        .collect();
    assert_eq!(xs.len(), 2);
    assert!((xs[0] - 10.0).abs() < 1.0 && xs[1] < -30.0, "{xs:?}");
    // Y otro `izq` en la maqueta ya no la corre: manda el marco.
    an::escribir(&b.fichero(".maqueta"), "384,128,256,100,1,0").unwrap();
    let otra = n.anotar_reflujo(doc.display().to_string()).unwrap();
    assert!((otra.caja().unwrap().x0 - xs[1] as f32).abs() < 3.0);
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn la_tinta_de_un_word_con_su_marco_cae_en_la_columna_aunque_la_maqueta_diga_otro_margen() {
    let r = raiz("marco-word");
    let doc = con_adjunto(&r, "acta.docx", b"PK");
    let b = base(&r, &doc);
    // La maqueta dice columna 420 con 280 de margen; el marco, que la
    // columna esta a 560 y al doble (otra escala de la capa).
    an::escribir(&b.fichero(".maqueta"), "420,280,280,100,1,0").unwrap();
    an::escribir(&b.marco(), "560,0,1400,840\nv1\n").unwrap();
    std::fs::create_dir_all(b.tinta().parent().unwrap()).unwrap();
    let tinta = r#"{"elements":[{"id":"w1","type":"freedraw","x":580,"y":100,"width":200,"height":0,"strokeWidth":2,"version":1,"points":[{"x":0.0,"y":0.0},{"x":200.0,"y":0.0}]}],"files":{}}"#;
    std::fs::write(b.tinta(), tinta).unwrap();
    let ed = abrir_tinta(&r, &doc).unwrap();
    let caja = ed.caja().unwrap();
    // (la caja lleva medio grosor del trazo de mas a cada lado)
    assert!(
        (caja.x0 - 10.0).abs() < 3.0 && (caja.y0 - 50.0).abs() < 3.0,
        "{caja:?}"
    );
    assert!((caja.x1 - 110.0).abs() < 3.0, "{caja:?}");
    // Abrir no escribe nada: ni la tinta ni su marco.
    assert!(!ed.guardar().unwrap());
    assert_eq!(std::fs::read_to_string(b.tinta()).unwrap(), tinta);
    assert_eq!(
        an::leer(&b.marco()).as_deref(),
        Some("560,0,1400,840\nv1\n")
    );
    // Caso negativo: sin marco, la regla de antes (la columna corrida `izq`).
    std::fs::remove_file(b.marco()).unwrap();
    let ed = abrir_tinta(&r, &doc).unwrap();
    let x0 = ed.caja().unwrap().x0;
    assert!((x0 - 300.0).abs() < 5.0, "{x0}");
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn sin_columna_no_se_anota_y_fuera_del_almacen_va_junto_al_documento() {
    let r = raiz("fuera");
    let otro = raiz("fuera-doc");
    let doc = otro.join("suelto.docx");
    std::fs::write(&doc, b"PK").unwrap();
    let n = nucleo(&r);
    let ruta = doc.display().to_string();
    let l = n.lectura_de_reflujo(ruta.clone());
    assert_eq!(l.donde, DondeAnota::JuntoAlDocumento);
    assert_eq!(l.columna, 0);
    assert!(
        n.anotar_reflujo(ruta.clone()).is_err(),
        "sin columna fijada no"
    );
    // Se fija la columna (al empezar a anotar) y ya se anota, sin correr.
    n.guardar_lectura_de_reflujo(ruta.clone(), LecturaReflujo { columna: 500, ..l })
        .unwrap();
    assert!(!n.hay_tinta_de_reflujo(ruta.clone()));
    let ed = n.anotar_reflujo(ruta.clone()).unwrap();
    trazar(&ed, (-50.0, 0.0), (90.0, 90.0));
    assert!(ed.guardar().unwrap());
    let capa = otro.join("suelto.docx.pixpin-anotado/capa.excalidraw");
    assert!(capa.is_file());
    assert!(n.hay_tinta_de_reflujo(ruta));
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&capa).unwrap()).unwrap();
    assert!(
        v["elements"][0]["x"].as_f64().unwrap() < -40.0,
        "sin correr"
    );
    let _ = std::fs::remove_dir_all(&r);
    let _ = std::fs::remove_dir_all(&otro);
}

#[test]
fn el_verde_no_se_guarda_como_marcador_y_los_marcadores_van_en_orden() {
    let r = raiz("marcadores");
    let doc = con_adjunto(&r, "acta.docx", b"PK");
    let n = nucleo(&r);
    let ruta = doc.display().to_string();
    let l = n.lectura_de_reflujo(ruta.clone());
    let m = con_marcador_de_lectura(Vec::new(), 0.6, "⭐".into(), 2000);
    let m = con_marcador_de_lectura(m, 0.2, "🔖".into(), 1000);
    assert_eq!(
        m.iter().map(|x| x.emoji.as_str()).collect::<Vec<_>>(),
        ["🔖", "⭐"]
    );
    let mut con_verde = m.clone();
    con_verde.push(MarcadorReflujo {
        id: u64::MAX,
        fraccion: 0.4,
        emoji: voz_alta::EMOJI_DE_VOZ.into(),
    });
    let l = LecturaReflujo {
        marcadores: con_verde,
        voz: Some(VozReflujo {
            parrafo: 3,
            fraccion: 0.4,
        }),
        ..l
    };
    n.guardar_lectura_de_reflujo(ruta.clone(), l).unwrap();
    let b = base(&r, &doc);
    assert_eq!(
        an::leer(&b.fichero(".marcas")).as_deref(),
        Some("1000:0.2:🔖|2000:0.6:⭐")
    );
    assert_eq!(an::leer(&b.fichero(".voz")).as_deref(), Some("3:0.4"));
    let otra = n.lectura_de_reflujo(ruta);
    assert_eq!(otra.marcadores.len(), 2);
    assert_eq!(otra.voz.map(|v| v.parrafo), Some(3));
    let _ = std::fs::remove_dir_all(&r);
}

// ------------------------------------------------- las cuentas de `visor.rs`

#[test]
fn la_columna_de_lectura_no_se_estira_en_un_monitor_ancho() {
    assert_eq!(columna_de_lectura_que_cabe(2560.0), 860.0);
    assert_eq!(columna_de_lectura_que_cabe(600.0), 504.0);
    // Caso negativo: en una ventana minima, la columna minima.
    assert_eq!(columna_de_lectura_que_cabe(100.0), 240.0);
    assert_eq!(columna_de_lectura_que_cabe(f32::NAN), 860.0);
}

fn lectura_vacia() -> LecturaReflujo {
    LecturaReflujo {
        tamano: 100,
        tipo: 0,
        grosor: 1,
        columna: 0,
        lados: None,
        sin_lado: false,
        sitio: 0.0,
        zoom: 1.0,
        marcadores: Vec::new(),
        voz: None,
        donde: DondeAnota::JuntoAlDocumento,
    }
}

#[test]
fn los_mandos_abren_espacio_a_tercios_y_el_candado_bloquea_el_lado() {
    // Sin columna fijada, se fija con la medida y sin espacio salvo el pedido.
    let l = otro_espacio_de_lado(lectura_vacia(), 420.4, true, true).unwrap();
    assert_eq!(l.columna, 420);
    assert_eq!(l.lados, Some(LadosReflujo { izq: 140, der: 0 }));
    let l = otro_espacio_de_lado(l, 420.0, true, true).unwrap();
    assert_eq!(l.lados.unwrap().izq, 280);
    // Caso negativo: en el tope (dos tercios) no hay mas, y en cero no hay menos.
    assert!(otro_espacio_de_lado(l.clone(), 420.0, true, true).is_none());
    assert!(otro_espacio_de_lado(l.clone(), 420.0, false, false).is_none());
    assert_eq!(lados_de_la_columna(l.clone(), 420.0), vec![280.0, 0.0]);
    // Sin columna fijada, los dos tercios de siempre.
    assert_eq!(
        lados_de_la_columna(lectura_vacia(), 600.0),
        vec![400.0, 400.0]
    );
}

#[test]
fn el_tamano_de_la_letra_va_por_los_puntos_del_movil_y_el_aumento_tiene_topes() {
    assert_eq!(tamano_de_letra_vecino(100, true), 115);
    assert_eq!(tamano_de_letra_vecino(100, false), 90);
    assert_eq!(tamano_de_letra_vecino(230, true), 230);
    // Alejar hasta ver la columna con sus dos margenes, nunca mas de 1.
    assert!((aumento_minimo_de_lectura(1400.0, 700.0) - 0.5).abs() < 1e-6);
    assert_eq!(aumento_minimo_de_lectura(400.0, 700.0), 1.0);
    assert!(aumento_maximo_de_lectura() > 1.0);
}

#[test]
fn se_empieza_por_el_verde_y_si_no_cuadra_por_el_mas_cercano() {
    let f = vec![0.0, 0.2, 0.5, 0.9];
    let verde = |parrafo, fraccion| VozReflujo { parrafo, fraccion };
    assert_eq!(parrafo_del_verde(f.clone(), verde(2, 0.5)), Some(2));
    // Contado por otro aparato: por la fraccion.
    assert_eq!(parrafo_del_verde(f.clone(), verde(40, 0.85)), Some(3));
    // Caso negativo: sin parrafos no hay por donde.
    assert_eq!(parrafo_del_verde(Vec::new(), verde(0, 0.0)), None);
}

/// Un medidor de mentira para cruzar el puente: 8 por letra.
struct OchoPorLetra;

impl MedidorReflujo for OchoPorLetra {
    fn avances(
        &self,
        texto: String,
        _f: String,
        _p: u16,
        _t: f32,
        _tr: Vec<TramoReflujo>,
    ) -> Vec<f32> {
        vec![8.0; texto.encode_utf16().count()]
    }
}

#[test]
fn un_word_se_coloca_se_busca_sin_tildes_y_su_indice_lleva_a_su_bloque() {
    let r = raiz("colocar");
    let doc = r.join("acta.html");
    std::fs::write(
        &doc,
        "<html><body><h1>Árbol</h1><p>El arbol del patio.</p><h2>Raices</h2><p>Una raiz honda.</p></body></html>",
    )
    .unwrap();
    let d = DocumentoReflujo::abrir(doc.display().to_string()).unwrap();
    assert!(!d.es_libro());
    let m = d.colocar(384.0, 100, 0, 1, Box::new(OchoPorLetra));
    assert_eq!(m.columna, 384.0);
    assert!(m.alto > 0.0);
    // Cada colocado con sus renglones de dos en dos y la letra del movil.
    let p = m
        .colocados
        .iter()
        .find(|c| c.texto.contains("patio"))
        .unwrap();
    assert_eq!(p.familia, "Noto Serif");
    assert_eq!(p.renglones.len() % 2, 0);
    assert!(!p.renglones.is_empty());
    assert_eq!(
        m.para_leer.len(),
        m.colocados
            .iter()
            .filter(|c| !c.texto.trim().is_empty())
            .count()
    );
    // Buscar «arbol» encuentra tambien «Árbol», en UTF-16.
    let k = d.buscar("arbol".into());
    assert_eq!(k.len(), 2);
    assert_eq!((k[0].inicio, k[0].largo), (0, 5));
    // El indice lleva al bloque de su titulo.
    let i = d.indice();
    assert!(i.iter().any(|e| e.titulo.contains("Raices")));
    assert!(m.colocados.iter().any(|c| c.bloque == Some(i[0].bloque)));
    // Caso negativo: lo que no se lee no se abre, y buscar nada no encuentra.
    assert!(DocumentoReflujo::abrir(r.join("no-esta.docx").display().to_string()).is_err());
    assert!(d.buscar("   ".into()).is_empty());
    assert!(se_abre_en_el_visor("libro.epub".into()));
    assert!(!se_abre_en_el_visor("plano.pdf".into()));
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn las_letras_del_movil_vienen_dentro() {
    let l = letras_de_lectura();
    assert!(
        l.iter()
            .any(|x| x.familia == "Noto Serif" && x.negrita && x.cursiva)
    );
    assert!(
        l.iter()
            .any(|x| x.familia == "Roboto" && !x.negrita && !x.cursiva)
    );
    assert!(l.iter().all(|x| x.datos.starts_with(b"wOF2")), "woff2");
}
