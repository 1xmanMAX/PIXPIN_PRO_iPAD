//! **Los marcos, las vistas y la tinta de un Excel viajan entre dos PC**
//! (20-oct-2026, el usuario: «todo lo nuevo se tiene que sincronizar»).
//!
//! Dos PixPin de PC con su carpeta de datos cada uno (`DiscoPc`), en el
//! mismo grupo y hablandose con el protocolo y el cifrado de verdad. Un
//! adjunto del chat lleva en su mensaje:
//!
//! - `anot-<uid>.marcos.json`: los marcos numerados (`marcos_doc::Marcos`);
//! - `anot-<uid>.vistas.json`: la vista exacta de cada marcador;
//! - `anot-<uid>-h<i>.excalidraw.gz`: la tinta de la hoja `i` (desde 0) de
//!   un libro de calculo (en el PC, `lienzos/anot-<uid>-h<i>.excalidraw`).
//!
//! Y una lista vaciada se queda `[]`: no vuelve lo de antes del otro PC.

use std::path::{Path, PathBuf};

use pixpin_proyecto::almacen::{self, Ficha};
use pixpin_proyecto::anotado::{self, Base};
use pixpin_proyecto::cuaderno::{self, Clase, Cuaderno, Mensaje, Sello};
use pixpin_proyecto::vista::DiscoPc;
use pixpin_sincro::anotado as a;
use pixpin_sincro::disco::{Disco, GENERAL};
use pixpin_sincro::disco_android::prueba::{Reloj, conectado, crear_grupo, presentar};
use pixpin_sincro::protocolo::Hecho;

struct Dos {
    dir: PathBuf,
    pc: DiscoPc,
    otro: DiscoPc,
    reloj: Reloj,
}

impl Drop for Dos {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn montar(etiqueta: &str) -> Dos {
    let dir = std::env::temp_dir().join(format!("pixpin-dos-pc-{etiqueta}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let d = Dos {
        pc: DiscoPc::nuevo(&dir.join("pc-1/PixPinMax")),
        otro: DiscoPc::nuevo(&dir.join("pc-2/PixPinMax")),
        dir,
        reloj: Reloj::nuevo(),
    };
    presentar(&d.pc, "id-pc", "Portátil");
    presentar(&d.otro, "id-pc2", "Sobremesa");
    crear_grupo(&d.pc, "ABCDE23456", d.reloj.ahora());
    conectado(&d.otro, &d.pc, true, Some("ABCDE23456"), &d.reloj, |_| {}).unwrap();
    d
}

/// Una vuelta entera desde `de` hacia `a`.
fn vuelta(d: &Dos, de: &DiscoPc, a: &DiscoPc) {
    conectado(de, a, false, None, &d.reloj, |s| {
        pixpin_sincro::vuelta::una(
            s,
            None,
            &mut Hecho::default(),
            "",
            &|| d.reloj.ahora(),
            &mut |_| {},
        )
        .unwrap()
    })
    .unwrap();
}

fn aparato() -> String {
    pixpin_proyecto::codigos::de_aparato("id-pc")
}

/// Un adjunto en «Mensajes guardados» del primer PC, como lo guarda su chat:
/// su ficha, el fichero y el codigo de su mensaje.
fn adjunto_pc(d: &Dos, nombre: &str, bytes: &[u8]) -> (Ficha, PathBuf, String) {
    let raiz = d.pc.raiz();
    let g = almacen::asegurar_guardados(&raiz, 1, &aparato()).unwrap();
    let ruta = almacen::guardar_adjunto(&raiz, &g.id, nombre, bytes).unwrap();
    let carpeta = almacen::carpeta(&raiz, &g.id);
    let numero = Cuaderno::leer_de(&carpeta)
        .map(|c| c.mensajes.len() as i64)
        .unwrap_or(0)
        + 1;
    let sello = Sello {
        cuando: d.reloj.tic(),
        numero,
        aparato: aparato(),
        proyecto: g.id.clone(),
    };
    cuaderno::anadir(
        &carpeta,
        &Mensaje::adjunto(Clase::Archivo, nombre, &ruta, bytes.len() as i64, &sello),
    )
    .unwrap();
    let doc = carpeta.join(&ruta);
    let uid = anotado::adjunto_de(&raiz, &doc)
        .expect("es un adjunto del chat")
        .uid;
    (g, doc, uid)
}

/// La base `anot-<uid>…` de «Mensajes guardados» en un PC.
fn base(raiz: &Path, base: String) -> Base {
    Base {
        raiz: raiz.to_path_buf(),
        chat: GENERAL.to_string(),
        base,
    }
}

/// Deja la fecha de un fichero un minuto por delante: dos escrituras del
/// mismo largo en el mismo milisegundo se tomarian por la misma.
fn adelantar(f: &Path) {
    let t = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    std::fs::OpenOptions::new()
        .write(true)
        .open(f)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

const MARCOS: &str = "[\n  {\n    \"caja\": [\n      10.0,\n      20.0,\n      300.0,\n      200.0\n    ],\n    \"parte\": 1\n  }\n]";
const VISTAS: &str = "{\n  \"1790218080412\": \"1.5 0 3400.5\"\n}";
const TINTA: &str = r##"{"type":"excalidraw","version":2,"source":"pixpin","elements":[{"id":"trazo-hoja","type":"freedraw","x":10.5,"y":20.0,"width":4,"height":4,"angle":0,"strokeColor":"#e03131","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":2,"strokeStyle":"solid","roughness":0,"opacity":100,"seed":3,"version":1,"versionNonce":1,"isDeleted":false,"updated":1,"points":[[0,0],[4,4]]}],"appState":{},"files":{}}"##;

fn ids(texto: &str) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(texto).unwrap();
    v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["isDeleted"] != true)
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn los_marcos_las_vistas_y_la_tinta_de_un_excel_del_chat_viajan_y_vaciados_no_vuelven() {
    let d = montar("marcos");
    let (_g, _doc, uid) = adjunto_pc(&d, "cuentas.xlsx", b"PK libro");
    let (r1, r2) = (d.pc.raiz(), d.otro.raiz());
    let aqui = base(&r1, a::del_pdf(&uid));
    let hoja = base(&r1, format!("{}-h1", a::del_pdf(&uid)));
    anotado::escribir(&aqui.fichero(a::MARCOS), MARCOS).unwrap();
    anotado::escribir(&aqui.fichero(a::VISTAS), VISTAS).unwrap();
    std::fs::create_dir_all(hoja.tinta().parent().unwrap()).unwrap();
    std::fs::write(hoja.tinta(), TINTA).unwrap();
    // Donde caen en el PC: lo de texto en `android/pins/draw/`, la tinta en
    // `lienzos/` (lo de cualquier dibujo del chat).
    let f = aqui.fichero(a::MARCOS);
    assert!(
        f.to_string_lossy()
            .replace('\\', "/")
            .ends_with(&format!("android/pins/draw/anot-{uid}.marcos.json")),
        "{}",
        f.display()
    );
    assert!(
        hoja.tinta()
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with(&format!("lienzos/anot-{uid}-h1.excalidraw"))
    );
    // Son del mensaje para la sincronizacion.
    let rels = d.pc.anotado(GENERAL);
    for t in [
        format!("pins/draw/anot-{uid}.marcos.json"),
        format!("pins/draw/anot-{uid}.vistas.json"),
        format!("pins/draw/anot-{uid}-h1.excalidraw.gz"),
    ] {
        assert!(rels.contains(&t), "{t} en {rels:?}");
    }

    vuelta(&d, &d.pc, &d.otro);

    let alli = base(&r2, a::del_pdf(&uid));
    let hoja_alli = base(&r2, format!("{}-h1", a::del_pdf(&uid)));
    assert_eq!(
        anotado::leer(&alli.fichero(a::MARCOS)).as_deref(),
        Some(MARCOS),
        "los marcos, tal cual"
    );
    assert_eq!(
        anotado::leer(&alli.fichero(a::VISTAS)).as_deref(),
        Some(VISTAS),
        "las vistas, tal cual"
    );
    let tinta_alli = std::fs::read_to_string(hoja_alli.tinta()).expect("la tinta de la hoja llega");
    assert_eq!(ids(&tinta_alli), vec!["trazo-hoja".to_string()]);

    // Vaciados en el primero: `[]` y `{}`, nunca borrados.
    d.reloj.saltar(10_000);
    anotado::escribir(&aqui.fichero(a::MARCOS), "[]").unwrap();
    adelantar(&aqui.fichero(a::MARCOS));
    anotado::escribir(&aqui.fichero(a::VISTAS), "{}").unwrap();
    adelantar(&aqui.fichero(a::VISTAS));
    vuelta(&d, &d.pc, &d.otro);
    assert_eq!(
        anotado::leer(&alli.fichero(a::MARCOS)).as_deref(),
        Some("[]")
    );
    assert_eq!(
        anotado::leer(&alli.fichero(a::VISTAS)).as_deref(),
        Some("{}")
    );
    // Y lo de antes no vuelve, dirija quien dirija.
    vuelta(&d, &d.otro, &d.pc);
    vuelta(&d, &d.pc, &d.otro);
    for r in [&aqui, &alli] {
        assert_eq!(
            anotado::leer(&r.fichero(a::MARCOS)).as_deref(),
            Some("[]"),
            "{}",
            r.raiz.display()
        );
        assert_eq!(anotado::leer(&r.fichero(a::VISTAS)).as_deref(), Some("{}"));
    }
}

#[test]
fn borrar_el_fichero_no_es_vaciarlo_vuelve_del_otro_pc() {
    // Por que se escribe `[]`: la sincronizacion no lleva borrados de
    // ficheros sueltos de lo anotado, y uno borrado se vuelve a pedir.
    let d = montar("borrado");
    let (_g, _doc, uid) = adjunto_pc(&d, "plano.pdf", b"%PDF-1.4");
    let aqui = base(&d.pc.raiz(), a::del_pdf(&uid));
    anotado::escribir(&aqui.fichero(a::MARCOS), MARCOS).unwrap();
    vuelta(&d, &d.pc, &d.otro);
    std::fs::remove_file(aqui.fichero(a::MARCOS)).unwrap();
    vuelta(&d, &d.pc, &d.otro);
    assert_eq!(
        anotado::leer(&aqui.fichero(a::MARCOS)).as_deref(),
        Some(MARCOS),
        "lo borrado vuelve"
    );
}

#[test]
fn los_marcos_de_una_nota_viajan_con_el_codigo_de_la_nota() {
    let d = montar("nota");
    let raiz = d.pc.raiz();
    let g = almacen::asegurar_guardados(&raiz, 1, &aparato()).unwrap();
    let carpeta = almacen::carpeta(&raiz, &g.id);
    let m = Mensaje::nota(
        "# Obra",
        &Sello {
            cuando: d.reloj.tic(),
            numero: 1,
            aparato: aparato(),
            proyecto: g.id.clone(),
        },
    );
    cuaderno::anadir(&carpeta, &m).unwrap();
    let codigo = m.codigo_unico();
    let aqui = base(&raiz, format!("{}{codigo}", a::PREFIJO));
    anotado::escribir(&aqui.fichero(a::MARCOS), MARCOS).unwrap();
    anotado::escribir(&aqui.fichero(".marcas"), "7:0.25:⭐").unwrap();
    vuelta(&d, &d.pc, &d.otro);
    let alli = base(&d.otro.raiz(), format!("{}{codigo}", a::PREFIJO));
    assert_eq!(
        anotado::leer(&alli.fichero(a::MARCOS)).as_deref(),
        Some(MARCOS)
    );
    assert_eq!(
        anotado::leer(&alli.fichero(".marcas")).as_deref(),
        Some("7:0.25:⭐")
    );
    // Caso negativo: lo de otro codigo no aparece.
    assert!(
        !base(&d.otro.raiz(), "anot-ZZZZZ22222".into())
            .fichero(a::MARCOS)
            .exists()
    );
}
