//! El editor del lienzo tal como lo usa Swift: dibujar por el puntero,
//! deshacer, guardar y volver a abrir, y que lo guardado sea lo mismo que
//! escribe Windows con el mismo motor.

use std::path::PathBuf;
use std::sync::Arc;

use pixpin_motor2d::excalidraw;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::vector::Punto2;
use pixpin_nucleo::lienzo::decodificar;
use pixpin_nucleo::{EditorLienzo, ErrorNucleo, FasePuntero, Nucleo};
use serde_json::Value;

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir =
            std::env::temp_dir().join(format!("pixpin-editor-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temporal(dir)
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Un trazo a mano de `desde` en `pasos` avisos, con presion.
fn trazo(ed: &EditorLienzo, desde: (f32, f32), pasos: usize) {
    let (x, y) = desde;
    ed.puntero(FasePuntero::Pulsar, x, y, Some(0.5), 0, 1.0);
    for i in 1..=pasos {
        let t = i as f32;
        ed.puntero(
            FasePuntero::Mover,
            x + t * 6.0,
            y + (t * 0.7).sin() * 20.0,
            Some(0.6),
            0,
            1.0,
        );
    }
    let t = pasos as f32;
    ed.puntero(
        FasePuntero::Soltar,
        x + t * 6.0,
        y + (t * 0.7).sin() * 20.0,
        None,
        0,
        1.0,
    );
}

fn arrastre(ed: &EditorLienzo, a: (f32, f32), b: (f32, f32)) {
    ed.puntero(FasePuntero::Pulsar, a.0, a.1, None, 0, 1.0);
    ed.puntero(
        FasePuntero::Mover,
        (a.0 + b.0) / 2.0,
        (a.1 + b.1) / 2.0,
        None,
        0,
        1.0,
    );
    ed.puntero(FasePuntero::Mover, b.0, b.1, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, b.0, b.1, None, 0, 1.0);
}

fn tipos(ruta: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(ruta).unwrap()).unwrap();
    v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| !e["isDeleted"].as_bool().unwrap_or(false))
        .map(|e| e["type"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn trazo_y_rectangulo_se_guardan_y_vuelven_iguales() {
    let t = Temporal::nueva("vuelta");
    let n = Nucleo::abrir(t.0.join("datos").display().to_string(), "Mac".into()).unwrap();
    let chat = n.crear_chat("Obra".into()).unwrap();
    let m = n
        .crear_lienzo(chat.id.clone(), Some("Plano".into()))
        .unwrap();
    assert_eq!(m.clase, "DIBUJO");
    assert_eq!(m.nombre, "Plano");
    let dibujo = m.referencia.clone().unwrap();
    // El mensaje esta en el chat.
    let pagina = n.ultimos_mensajes(chat.id.clone(), 10).unwrap();
    assert_eq!(
        pagina.mensajes.last().unwrap().referencia.as_deref(),
        Some(dibujo.as_str())
    );

    let ed = n.editar_lienzo(chat.id.clone(), dibujo.clone()).unwrap();
    assert_eq!(ed.cuantos_elementos(), 0);
    // Abrir y guardar sin tocar nada no escribe.
    assert!(!ed.guardar().unwrap());

    let v0 = ed.version();
    ed.herramienta("tinta".into()).unwrap();
    trazo(&ed, (10.0, 10.0), 30);
    assert!(ed.version() > v0);
    ed.herramienta("rectangulo".into()).unwrap();
    arrastre(&ed, (100.0, 100.0), (220.0, 180.0));
    assert_eq!(ed.cuantos_elementos(), 2);
    assert!(ed.sin_guardar());

    // El bufer de la vista se lee con el lector de referencia.
    let plano = ed.plano_de_vista(0.0, 0.0, 1.0, 800.0, 600.0);
    let (cab, ordenes) = decodificar(&plano).unwrap();
    assert!(!ordenes.is_empty());
    assert!(cab.caja.is_some());

    assert!(ed.guardar().unwrap());
    assert!(!ed.sin_guardar());
    // Lo mismo otra vez: nada que escribir.
    assert!(!ed.guardar().unwrap());
    assert_eq!(tipos(&ed.ruta()), vec!["freedraw", "rectangle"]);

    // Volver a abrir da el mismo dibujo, orden a orden.
    let otra = n.editar_lienzo(chat.id.clone(), dibujo).unwrap();
    assert_eq!(otra.cuantos_elementos(), 2);
    assert_eq!(
        decodificar(&otra.plano()).unwrap().1,
        decodificar(&ed.plano()).unwrap().1
    );
    // Y guardarlo sin tocar no lo cambia.
    let antes = std::fs::read_to_string(otra.ruta()).unwrap();
    assert!(!otra.guardar().unwrap());
    assert_eq!(std::fs::read_to_string(otra.ruta()).unwrap(), antes);
}

#[test]
fn deshacer_rehacer_y_borrar() {
    let t = Temporal::nueva("deshacer");
    let ruta = t.0.join("a.excalidraw").display().to_string();
    let ed = EditorLienzo::de_fichero(ruta.clone(), true).unwrap();
    assert!(std::path::Path::new(&ruta).exists());
    trazo(&ed, (0.0, 0.0), 10);
    ed.herramienta("elipse".into()).unwrap();
    arrastre(&ed, (50.0, 50.0), (120.0, 90.0));
    assert_eq!(ed.cuantos_elementos(), 2);
    assert!(ed.puede_deshacer());
    assert!(ed.deshacer());
    assert_eq!(ed.cuantos_elementos(), 1);
    assert!(ed.rehacer());
    assert_eq!(ed.cuantos_elementos(), 2);
    assert!(!ed.rehacer());

    // Elegir la elipse con la mano y borrarla.
    ed.herramienta("seleccion".into()).unwrap();
    ed.puntero(FasePuntero::Pulsar, 85.0, 50.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, 85.0, 50.0, None, 0, 1.0);
    assert!(ed.borrar_seleccion());
    assert_eq!(ed.cuantos_elementos(), 1);
    assert!(!ed.borrar_seleccion());

    // La goma se lleva el trazo, y un solo deshacer lo devuelve.
    ed.herramienta("borrador".into()).unwrap();
    ed.puntero(FasePuntero::Pulsar, 200.0, 200.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Mover, 0.0, 0.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, 0.0, 0.0, None, 0, 1.0);
    assert_eq!(ed.cuantos_elementos(), 0);
    assert!(ed.deshacer());
    assert_eq!(ed.cuantos_elementos(), 1);

    assert!(ed.guardar().unwrap());
    assert_eq!(tipos(&ruta), vec!["freedraw"]);
    assert!(matches!(
        ed.herramienta("pincel_magico".into()),
        Err(ErrorNucleo::Argumento { .. })
    ));
}

#[test]
fn el_texto_se_escribe_y_se_guarda() {
    let t = Temporal::nueva("texto");
    let ruta = t.0.join("t.excalidraw").display().to_string();
    let ed = EditorLienzo::de_fichero(ruta.clone(), true).unwrap();
    ed.herramienta("texto".into()).unwrap();
    ed.puntero(FasePuntero::Pulsar, 40.0, 40.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, 40.0, 40.0, None, 0, 1.0);
    assert!(ed.escribiendo());
    assert!(ed.teclear("Holá ñ".into()));
    assert!(ed.tecla("retroceso".into()));
    assert!(ed.terminar_texto());
    assert!(!ed.escribiendo());
    assert!(ed.guardar().unwrap());
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&ruta).unwrap()).unwrap();
    let e = &v["elements"][0];
    assert_eq!(e["type"], "text");
    assert_eq!(e["text"], "Holá ");
}

#[test]
fn lo_ajeno_se_queda_y_el_fichero_es_el_de_windows() {
    let t = Temporal::nueva("ajeno");
    let original = r##"{
  "type": "excalidraw", "version": 2, "source": "pixpin-android",
  "elements": [
    {"id":"m1","type":"pixpin-solido","x":0,"y":0,"width":10,"height":10,"loQueSea":[1,2,3]},
    {"id":"a1","type":"rectangle","x":10,"y":20,"width":100,"height":50,
     "strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":2,
     "strokeStyle":"solid","roughness":1,"opacity":100,"seed":12345,"version":3}
  ],
  "appState": {"viewBackgroundColor": "#fafafa"},
  "files": {}
}"##;
    let ruta = t.0.join("w.excalidraw");
    std::fs::write(&ruta, original).unwrap();
    let ed = EditorLienzo::de_fichero(ruta.display().to_string(), false).unwrap();
    assert_eq!(ed.cuantos_elementos(), 1);
    trazo(&ed, (300.0, 300.0), 12);
    ed.herramienta("flecha".into()).unwrap();
    arrastre(&ed, (400.0, 50.0), (500.0, 120.0));
    assert!(ed.guardar().unwrap());

    // Lo mismo, a mano con el motor, como lo guarda Windows
    // (`ventana_chat::guardar_hoja_dibujada`).
    let lienzo = excalidraw::leer(original).unwrap();
    let mut escena = excalidraw::a_escena(&lienzo);
    let mut g = Gesto::nuevo();
    let mut ev = |g: &mut Gesto, e: EventoGesto| {
        g.evento(e, &mut escena, 1.0);
    };
    g.tomar_herramienta(Herramienta::Lapiz);
    let p = |x: f32, y: f32| Punto2::nuevo(x, y);
    ev(
        &mut g,
        EventoGesto::Pulsar {
            p: p(300.0, 300.0),
            shift: false,
            alt: false,
            presion: Some(0.5),
        },
    );
    for i in 1..=12 {
        let t = i as f32;
        ev(
            &mut g,
            EventoGesto::Mover {
                p: p(300.0 + t * 6.0, 300.0 + (t * 0.7).sin() * 20.0),
                shift: false,
                alt: false,
                presion: Some(0.6),
            },
        );
    }
    ev(
        &mut g,
        EventoGesto::Soltar {
            p: p(372.0, 300.0 + (12.0f32 * 0.7).sin() * 20.0),
        },
    );
    g.tomar_herramienta(Herramienta::Flecha);
    ev(
        &mut g,
        EventoGesto::Pulsar {
            p: p(400.0, 50.0),
            shift: false,
            alt: false,
            presion: None,
        },
    );
    ev(
        &mut g,
        EventoGesto::Mover {
            p: p(450.0, 85.0),
            shift: false,
            alt: false,
            presion: None,
        },
    );
    ev(
        &mut g,
        EventoGesto::Mover {
            p: p(500.0, 120.0),
            shift: false,
            alt: false,
            presion: None,
        },
    );
    ev(&mut g, EventoGesto::Soltar { p: p(500.0, 120.0) });
    let de_windows = excalidraw::escribir(&excalidraw::con_escena(&lienzo, &escena));

    // Iguales salvo la hora (`updated`), el desempate que sale de ella y el
    // NUMERO de lo nacido aqui: el Mac salta un trecho al azar
    // (`ids_nuevos_desde`, para no repetir el `pc<n>` del PC al dibujar los
    // dos a la vez). La forma, `pc<hex>`, es la misma.
    fn sin_hora(v: &mut Value) {
        let mut nacidos = 0;
        if let Some(lista) = v["elements"].as_array_mut() {
            for e in lista {
                if let Some(m) = e.as_object_mut() {
                    m.remove("updated");
                    m.remove("versionNonce");
                    let id = m["id"].as_str().unwrap_or_default().to_string();
                    if let Some(hex) = id.strip_prefix("pc") {
                        assert!(
                            u64::from_str_radix(hex, 16).is_ok() && hex.len() <= 16,
                            "{id}"
                        );
                        nacidos += 1;
                        m.insert("id".into(), Value::String(format!("pc#{nacidos}")));
                    }
                }
            }
        }
    }
    let mut a: Value = serde_json::from_str(&std::fs::read_to_string(&ruta).unwrap()).unwrap();
    let mut b: Value = serde_json::from_str(&de_windows).unwrap();
    // Los del Mac, distintos de los que sacaria el PC desde el mismo fichero.
    let ids = |v: &Value| -> Vec<String> {
        v["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap().to_string())
            .collect()
    };
    let (de_mac, de_pc) = (ids(&a), ids(&b));
    assert!(
        de_mac
            .iter()
            .filter(|i| i.starts_with("pc"))
            .all(|i| !de_pc.contains(i)),
        "{de_mac:?} {de_pc:?}"
    );
    sin_hora(&mut a);
    sin_hora(&mut b);
    assert_eq!(a, b);
    // Lo ajeno, intacto y primero.
    assert_eq!(a["elements"][0]["type"], "pixpin-solido");
    assert_eq!(a["elements"][0]["loQueSea"], serde_json::json!([1, 2, 3]));
    assert_eq!(
        tipos(&ruta.display().to_string()),
        vec!["pixpin-solido", "rectangle", "freedraw", "arrow"]
    );
}

#[test]
fn varios_hilos_a_la_vez() {
    // Guardar desde otro hilo mientras se dibuja: ni se bloquea ni pierde nada.
    let t = Temporal::nueva("hilos");
    let ruta = t.0.join("h.excalidraw").display().to_string();
    let ed = EditorLienzo::de_fichero(ruta.clone(), true).unwrap();
    let otro = Arc::clone(&ed);
    let h = std::thread::spawn(move || {
        for _ in 0..50 {
            let _ = otro.guardar().unwrap();
        }
    });
    for i in 0..20 {
        trazo(&ed, (i as f32 * 30.0, 0.0), 8);
    }
    h.join().unwrap();
    ed.guardar().unwrap();
    assert_eq!(tipos(&ruta).len(), 20);
}
