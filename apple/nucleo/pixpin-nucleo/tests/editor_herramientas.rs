//! Las herramientas del editor que faltaban en Mac: el lazo, las de medir,
//! las cuatro de construir, el gesto de pararse, el estilo de la barra, las
//! imagenes pegadas (en el formato de Windows y el movil) y las lapidas de
//! lo borrado, comprobadas contra la fusion de `pixpin-sincro`.

use std::path::PathBuf;

use pixpin_motor2d::excalidraw;
use pixpin_nucleo::{
    CambioDeEstilo, ColorEstilo, EditorLienzo, FasePuntero, GrosorEstilo, Nucleo, PeticionLienzo,
};
use pixpin_sincro::canonico::Json;
use pixpin_sincro::fusion::{self, Criterio, Cuenta};
use serde_json::Value;

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-herramientas-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temporal(dir)
    }

    fn fichero(&self, nombre: &str) -> String {
        self.0.join(nombre).display().to_string()
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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

fn rectangulo(ed: &EditorLienzo, a: (f32, f32), b: (f32, f32)) {
    ed.herramienta("rectangulo".into()).unwrap();
    arrastre(ed, a, b);
}

fn json(ruta: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(ruta).unwrap()).unwrap()
}

fn elementos(ruta: &str) -> Vec<Value> {
    json(ruta)["elements"].as_array().unwrap().clone()
}

/// Los vivos de un texto de `.excalidraw`, por su tipo.
fn vivos(texto: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(texto).unwrap();
    v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| !e["isDeleted"].as_bool().unwrap_or(false))
        .map(|e| e["type"].as_str().unwrap().to_string())
        .collect()
}

/// Un lazo cerrado que pasa por `puntos`.
fn lazo(ed: &EditorLienzo, puntos: &[(f32, f32)]) {
    ed.herramienta("lazo".into()).unwrap();
    let (x, y) = puntos[0];
    ed.puntero(FasePuntero::Pulsar, x, y, None, 0, 1.0);
    for &(x, y) in &puntos[1..] {
        ed.puntero(FasePuntero::Mover, x, y, None, 0, 1.0);
    }
    ed.puntero(FasePuntero::Soltar, x, y, None, 0, 1.0);
}

#[test]
fn el_lazo_elige_solo_lo_que_encierra_entero() {
    let t = Temporal::nueva("lazo");
    let ed = EditorLienzo::de_fichero(t.fichero("a.excalidraw"), true).unwrap();
    rectangulo(&ed, (10.0, 10.0), (60.0, 60.0));
    rectangulo(&ed, (200.0, 10.0), (260.0, 60.0));
    rectangulo(&ed, (90.0, 10.0), (190.0, 60.0));
    assert_eq!(ed.cuantos_elementos(), 3);

    // Rodea el primero y solo la mitad del tercero.
    lazo(
        &ed,
        &[
            (0.0, 0.0),
            (140.0, 0.0),
            (140.0, 80.0),
            (0.0, 80.0),
            (0.0, 1.0),
        ],
    );
    assert_eq!(ed.herramienta_actual(), "lazo");
    assert_eq!(ed.cuantos_elegidos(), 1, "el que queda a medias no entra");

    // Mientras se traza, el contorno se pinta encima del dibujo.
    ed.herramienta("lazo".into()).unwrap();
    ed.puntero(FasePuntero::Pulsar, 0.0, 0.0, None, 0, 1.0);
    let sin = pixpin_nucleo::lienzo::decodificar(&ed.plano())
        .unwrap()
        .1
        .len();
    ed.puntero(FasePuntero::Mover, 300.0, 0.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Mover, 300.0, 90.0, None, 0, 1.0);
    let con = pixpin_nucleo::lienzo::decodificar(&ed.plano_de_vista(0.0, 0.0, 1.0, 800.0, 600.0))
        .unwrap()
        .1
        .len();
    assert!(con > sin, "el lazo en curso no se pintaba");
    ed.puntero(FasePuntero::Mover, 0.0, 90.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, 0.0, 90.0, None, 0, 1.0);
    assert_eq!(ed.cuantos_elegidos(), 3);

    // Lo elegido con el lazo se borra de un golpe.
    assert!(ed.borrar_seleccion());
    assert_eq!(ed.cuantos_elementos(), 0);

    // Caso negativo: un clic suelto con el lazo no elige nada.
    assert!(ed.deshacer());
    lazo(&ed, &[(30.0, 30.0)]);
    assert_eq!(ed.cuantos_elegidos(), 0);
}

#[test]
fn la_cota_nace_pide_su_medida_y_obedece_lo_dictado() {
    let t = Temporal::nueva("cota");
    let ruta = t.fichero("c.excalidraw");
    let ed = EditorLienzo::de_fichero(ruta.clone(), true).unwrap();

    // Calibrar: 200 de mundo son 2 m. La raya no se queda en el dibujo.
    ed.herramienta("escalar".into()).unwrap();
    arrastre(&ed, (0.0, 0.0), (200.0, 0.0));
    assert_eq!(ed.cuantos_elementos(), 0);
    let Some(PeticionLienzo::Calibrar { largo_px }) = ed.tomar_peticion() else {
        panic!("escalar tenia que pedir la medida");
    };
    assert!((largo_px - 200.0).abs() < 0.01);
    assert_eq!(ed.tomar_peticion(), None, "se pide una sola vez");
    assert!(
        !ed.calibrar(largo_px, -3.0, "m".into()),
        "una medida imposible no vale"
    );
    assert!(ed.calibrar(largo_px, 2.0, "m".into()));

    // La cota: nace, pide su medida (ya en metros) y obedece lo dictado.
    ed.herramienta("cota".into()).unwrap();
    arrastre(&ed, (10.0, 100.0), (110.0, 100.0));
    assert_eq!(ed.cuantos_elementos(), 1);
    let Some(PeticionLienzo::DictarCota {
        id,
        largo,
        grados,
        unidad,
    }) = ed.tomar_peticion()
    else {
        panic!("la cota tenia que pedir su medida");
    };
    assert_eq!(unidad, "m");
    assert!((largo - 1.0).abs() < 0.01, "100 de mundo son 1 m: {largo}");
    assert!(grados.abs() < 0.01);
    assert!(ed.dictar_cota(id, 3.0, 90.0));

    // Y la reglita de la escala grafica.
    ed.herramienta("escala_grafica".into()).unwrap();
    arrastre(&ed, (0.0, 300.0), (150.0, 330.0));
    assert_eq!(ed.cuantos_elementos(), 2);

    assert!(ed.guardar().unwrap());
    let v = json(&ruta);
    assert_eq!(v["escala"]["unidad"], "m");
    let cota = v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == "pixpin-measure")
        .expect("la cota se guarda como en el movil");
    // Dictada a 3 m hacia arriba: 300 de mundo en vertical.
    assert!(
        (cota["height"].as_f64().unwrap().abs() - 300.0).abs() < 0.5,
        "{cota}"
    );
    assert!(
        v["elements"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["type"] == "pixpin-scalebar")
    );
}

#[test]
fn las_cuatro_de_construir_trabajan_sobre_lo_que_hay() {
    let t = Temporal::nueva("construir");
    let ed = EditorLienzo::de_fichero(t.fichero("k.excalidraw"), true).unwrap();
    rectangulo(&ed, (0.0, 0.0), (200.0, 120.0));
    // El bote dentro de una sola figura le pone su fondo, sin mancha aparte.
    ed.herramienta("bote".into()).unwrap();
    let cambios = ed.cambios();
    ed.puntero(FasePuntero::Pulsar, 100.0, 60.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, 100.0, 60.0, None, 0, 1.0);
    assert!(ed.cambios() > cambios);
    assert_eq!(ed.cuantos_elementos(), 1);
    assert!(
        ed.estilo().fondo.is_none(),
        "sin nada elegido, el estilo es el actual"
    );

    // Recortar y extender sobre rayas.
    ed.herramienta("linea".into()).unwrap();
    arrastre(&ed, (300.0, 50.0), (600.0, 50.0));
    arrastre(&ed, (400.0, 0.0), (400.0, 100.0));
    arrastre(&ed, (500.0, 0.0), (500.0, 100.0));
    assert_eq!(ed.cuantos_elementos(), 4);
    ed.herramienta("recortar".into()).unwrap();
    ed.puntero(FasePuntero::Pulsar, 450.0, 50.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, 450.0, 50.0, None, 0, 1.0);
    assert_eq!(ed.cuantos_elementos(), 5, "la raya se parte en dos");
    assert!(ed.deshacer());
    assert_eq!(ed.cuantos_elementos(), 4);

    ed.herramienta("linea".into()).unwrap();
    arrastre(&ed, (300.0, 200.0), (350.0, 200.0));
    arrastre(&ed, (400.0, 150.0), (400.0, 250.0));
    ed.herramienta("extender".into()).unwrap();
    ed.puntero(FasePuntero::Pulsar, 350.0, 200.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, 350.0, 200.0, None, 0, 1.0);
    let caja = ed.caja().unwrap();
    assert!(caja.x1 >= 600.0);

    // El punto se pega al cruce y lleva su letra; en el vacio no nace.
    ed.herramienta("punto".into()).unwrap();
    let antes = ed.cuantos_elementos();
    ed.puntero(FasePuntero::Pulsar, 402.0, 199.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, 402.0, 199.0, None, 0, 1.0);
    assert_eq!(ed.cuantos_elementos(), antes + 1);
    ed.puntero(FasePuntero::Pulsar, 900.0, 900.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, 900.0, 900.0, None, 0, 1.0);
    assert_eq!(
        ed.cuantos_elementos(),
        antes + 1,
        "un punto en el vacio no nace"
    );
    assert_eq!(ed.herramienta_actual(), "punto");
}

#[test]
fn quieto_dibujando_a_mano_el_trazo_se_vuelve_figura() {
    let t = Temporal::nueva("forma");
    let ruta = t.fichero("f.excalidraw");
    let ed = EditorLienzo::de_fichero(ruta.clone(), true).unwrap();
    ed.herramienta("lapiz".into()).unwrap();
    assert_eq!(ed.espera_forma_ms(), None, "sin trazar no hay espera");
    ed.puntero(FasePuntero::Pulsar, 0.0, 0.0, Some(0.5), 0, 1.0);
    assert_eq!(
        ed.espera_forma_ms(),
        Some(pixpin_motor2d::forma_rapida::ESPERA_PARA_LA_FORMA_MS)
    );
    for i in 1..=40 {
        ed.puntero(FasePuntero::Mover, i as f32 * 5.0, 0.0, Some(0.5), 0, 1.0);
    }
    assert!(ed.forma_rapida(), "una raya recta quieta se vuelve linea");
    ed.puntero(FasePuntero::Soltar, 200.0, 0.0, None, 0, 1.0);
    assert!(!ed.forma_rapida(), "soltado ya no hay nada que convertir");
    assert!(ed.guardar().unwrap());
    assert_eq!(
        vivos(&std::fs::read_to_string(&ruta).unwrap()),
        vec!["line"]
    );
    assert_eq!(pixpin_nucleo::editor::forma_rapida_temblor_px(), 8.0);

    // Caso negativo: con el rectangulo no hay gesto de pararse.
    ed.herramienta("rectangulo".into()).unwrap();
    ed.puntero(FasePuntero::Pulsar, 0.0, 100.0, None, 0, 1.0);
    assert_eq!(ed.espera_forma_ms(), None);
    assert!(!ed.forma_rapida());
    ed.puntero(FasePuntero::Cancelar, 0.0, 100.0, None, 0, 1.0);
}

#[test]
fn el_estilo_de_la_barra_va_a_lo_proximo_y_a_lo_elegido() {
    let t = Temporal::nueva("estilo");
    let ruta = t.fichero("e.excalidraw");
    let ed = EditorLienzo::de_fichero(ruta.clone(), true).unwrap();
    let rojo = ColorEstilo {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    // Sin nada elegido: solo el «actual», el dibujo no cambia.
    assert!(!ed.cambiar_estilo(CambioDeEstilo::Trazo { color: rojo }));
    assert_eq!(ed.estilo().trazo, rojo);
    rectangulo(&ed, (0.0, 0.0), (100.0, 100.0));
    assert!(ed.guardar().unwrap());
    assert_eq!(elementos(&ruta)[0]["strokeColor"], "#ff0000");

    // Elegido: se le aplica en un paso de deshacer.
    ed.seleccionar_todo();
    assert!(ed.cambiar_estilo(CambioDeEstilo::Grosor {
        grosor: GrosorEstilo::Grueso
    }));
    assert_eq!(ed.estilo().grosor, GrosorEstilo::Grueso);
    assert!(ed.cambiar_estilo(CambioDeEstilo::Fondo { color: Some(rojo) }));
    assert!(ed.guardar().unwrap());
    let e = &elementos(&ruta)[0];
    assert_eq!(e["strokeWidth"], 4.0);
    assert_eq!(e["backgroundColor"], "#ff0000");
    assert!(ed.deshacer());
    // Deshacer suelta lo elegido: la barra vuelve al «actual», que se queda
    // con el fondo puesto; la figura, en cambio, vuelve a no tenerlo.
    assert_eq!(ed.estilo().fondo, Some(rojo));
    ed.seleccionar_todo();
    assert_eq!(ed.estilo().fondo, None);
    // Pulsar lo que ya tiene no ensucia el historial.
    assert!(!ed.cambiar_estilo(CambioDeEstilo::Trazo { color: rojo }));
}

/// Un PNG de `ancho` x `alto` (solo la cabecera y un final: Rust no lo
/// decodifica, lo copia).
fn png(ancho: u32, alto: u32) -> Vec<u8> {
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13];
    v.extend_from_slice(b"IHDR");
    v.extend_from_slice(&ancho.to_be_bytes());
    v.extend_from_slice(&alto.to_be_bytes());
    v.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    v.extend_from_slice(&[0, 0, 0, 0]);
    v.extend_from_slice(b"IEND");
    v.extend_from_slice(&[0xae, 0x42, 0x60, 0x82]);
    v
}

#[test]
fn la_imagen_pegada_se_guarda_como_en_windows_y_el_movil() {
    let t = Temporal::nueva("pegar");
    let n = Nucleo::abrir(t.0.join("datos").display().to_string(), "Mac".into()).unwrap();
    let chat = n.crear_chat("Obra".into()).unwrap();
    let dibujo = n
        .crear_lienzo(chat.id.clone(), None)
        .unwrap()
        .referencia
        .unwrap();
    let ed = n.editar_lienzo(chat.id.clone(), dibujo.clone()).unwrap();

    // Una que se pega y se borra antes de guardar: no deja nada.
    let borrada = ed.pegar_imagen(png(8, 8), 0.0, 0.0, 800.0, 600.0).unwrap();
    assert!(ed.borrar_seleccion());
    // Y la que se queda: grande, entra reducida al 80 % de lo visible.
    let bytes = png(2000, 1000);
    let foto = ed
        .pegar_imagen(bytes.clone(), 400.0, 300.0, 800.0, 600.0)
        .unwrap();
    assert!(
        std::path::Path::new(&foto.ruta).is_file(),
        "se pinta desde ahi"
    );
    assert_eq!(ed.herramienta_actual(), "seleccion");
    assert_eq!(ed.cuantos_elegidos(), 1);
    assert!(ed.guardar().unwrap());

    let clave = pixpin_motor2d::enlace::id_de_texto(foto.id_objeto);
    let v = json(&ed.ruta());
    let e = v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == "image")
        .expect("la figura de la imagen");
    assert_eq!(e["fileId"], clave.as_str());
    assert_eq!(e["status"], "saved");
    assert_eq!(e["isDeleted"], false);
    assert!((e["width"].as_f64().unwrap() - 640.0).abs() < 0.01);
    assert!((e["height"].as_f64().unwrap() - 320.0).abs() < 0.01);
    assert!((e["x"].as_f64().unwrap() - 80.0).abs() < 0.01);
    let f = &v["files"][clave.as_str()];
    assert_eq!(f["id"], clave.as_str());
    assert_eq!(f["mimeType"], "image/png");
    assert_eq!(f["path"], format!("imagenes/{clave}"));
    assert!(f["created"].as_i64().unwrap() > 0);
    assert_eq!(
        v["files"].as_object().unwrap().len(),
        1,
        "la borrada no se apunta"
    );

    // El fichero, en `imagenes/` de la carpeta del chat, con los mismos bytes.
    let carpeta = pixpin_proyecto::almacen::carpeta(&t.0.join("datos"), &chat.id);
    let escrito = carpeta.join("imagenes").join(&clave);
    assert_eq!(std::fs::read(&escrito).unwrap(), bytes);
    assert!(
        !carpeta
            .join("imagenes")
            .join(pixpin_motor2d::enlace::id_de_texto(borrada.id_objeto))
            .exists()
    );

    // Al reabrir, el editor encuentra la foto por el id de su figura.
    let otra = n.editar_lienzo(chat.id.clone(), dibujo).unwrap();
    assert_eq!(otra.cuantos_elementos(), 1);
    let fotos = otra.imagenes();
    assert_eq!(fotos.len(), 1);
    assert_eq!(fotos[0].id_objeto, foto.id_objeto);
    assert_eq!(PathBuf::from(&fotos[0].ruta), escrito);
    // Y lo lee igual el excalidraw del motor (el de Windows).
    let leido = excalidraw::leer(&std::fs::read_to_string(otra.ruta()).unwrap()).unwrap();
    assert_eq!(
        excalidraw::ficheros(&leido),
        vec![(foto.id_objeto, format!("imagenes/{clave}"))]
    );

    // Caso negativo: lo que no es PNG no se pega.
    assert!(
        ed.pegar_imagen(b"GIF89a".to_vec(), 0.0, 0.0, 10.0, 10.0)
            .is_err()
    );
}

/// La fusion de `pixpin-sincro` (`fusion::json`, figura por figura) de dos
/// textos de lienzo sobre una base.
fn juntar(base: Option<&str>, mio: &str, suyo: &str) -> String {
    let b = base.map(|b| Json::analizar(b).unwrap());
    let m = Json::analizar(mio).unwrap();
    let s = Json::analizar(suyo).unwrap();
    let mut cuenta = Cuenta::default();
    fusion::json(
        b.as_ref(),
        Some(&m),
        Some(&s),
        &Criterio::default(),
        &mut cuenta,
    )
    .unwrap()
    .a_texto()
}

#[test]
fn lo_nacido_y_borrado_deja_lapida_y_la_sincronizacion_no_lo_resucita() {
    let t = Temporal::nueva("lapida");
    let ruta = t.fichero("l.excalidraw");
    let ed = EditorLienzo::de_fichero(ruta.clone(), true).unwrap();

    // 1. Se crea en el Mac y se guarda.
    rectangulo(&ed, (0.0, 0.0), (100.0, 80.0));
    ed.herramienta("elipse".into()).unwrap();
    arrastre(&ed, (200.0, 0.0), (260.0, 50.0));
    assert!(ed.guardar().unwrap());
    // 2. Se sincroniza: esto es la base comun y lo que tiene el otro, que no
    // lo toca.
    let base = std::fs::read_to_string(&ruta).unwrap();
    assert_eq!(vivos(&base), vec!["rectangle", "ellipse"]);
    let id_rect = elementos(&ruta)[0]["id"].as_str().unwrap().to_string();
    let version_antes = elementos(&ruta)[0]["version"].as_i64().unwrap();

    // 3. Se borra el rectangulo en el Mac y se guarda: lapida, no hueco.
    ed.herramienta("seleccion".into()).unwrap();
    ed.puntero(FasePuntero::Pulsar, 50.0, 0.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, 50.0, 0.0, None, 0, 1.0);
    assert!(ed.borrar_seleccion());
    assert!(ed.guardar().unwrap());
    let mio = std::fs::read_to_string(&ruta).unwrap();
    let lapida = elementos(&ruta)
        .into_iter()
        .find(|e| e["id"] == id_rect.as_str())
        .expect("el borrado sigue en el fichero");
    assert_eq!(lapida["isDeleted"], true);
    assert_eq!(lapida["type"], "rectangle");
    assert!(lapida["version"].as_i64().unwrap() > version_antes);
    assert_eq!(vivos(&mio), vec!["ellipse"]);
    // Y sigue ahi en el guardado siguiente.
    ed.herramienta("linea".into()).unwrap();
    arrastre(&ed, (0.0, 200.0), (50.0, 200.0));
    assert!(ed.guardar().unwrap());
    let mio_despues = std::fs::read_to_string(&ruta).unwrap();
    assert!(
        elementos(&ruta)
            .iter()
            .any(|e| e["id"] == id_rect.as_str() && e["isDeleted"] == true)
    );

    // 4. La fusion con el otro aparato, que no toco nada: borrado en los dos
    // lados, se mire desde donde se mire...
    assert_eq!(vivos(&juntar(Some(&base), &mio, &base)), vec!["ellipse"]);
    assert_eq!(vivos(&juntar(Some(&base), &base, &mio)), vec!["ellipse"]);
    assert_eq!(
        vivos(&juntar(Some(&base), &base, &mio_despues)),
        vec!["ellipse", "line"]
    );
    // ...y tambien sin base comun (la primera vez, que junta todo): la
    // lapida es mas nueva que la copia viva del otro y gana.
    assert_eq!(vivos(&juntar(None, &mio, &base)), vec!["ellipse"]);
    assert_eq!(vivos(&juntar(None, &base, &mio)), vec!["ellipse"]);

    // 5. Reabrir el fichero: lo borrado no se ve y la lapida se conserva.
    let otra = EditorLienzo::de_fichero(ruta.clone(), false).unwrap();
    assert_eq!(otra.cuantos_elementos(), 2);
    otra.herramienta("rectangulo".into()).unwrap();
    arrastre(&otra, (300.0, 300.0), (350.0, 350.0));
    assert!(otra.guardar().unwrap());
    assert!(
        elementos(&ruta)
            .iter()
            .any(|e| e["id"] == id_rect.as_str() && e["isDeleted"] == true)
    );

    // Caso negativo: lo nacido y borrado ANTES de guardar nunca llego a
    // nadie y no deja lapida.
    let ruta2 = t.fichero("m.excalidraw");
    let ed2 = EditorLienzo::de_fichero(ruta2.clone(), true).unwrap();
    rectangulo(&ed2, (0.0, 0.0), (10.0, 10.0));
    ed2.seleccionar_todo();
    assert!(ed2.borrar_seleccion());
    ed2.herramienta("elipse".into()).unwrap();
    arrastre(&ed2, (50.0, 50.0), (90.0, 90.0));
    assert!(ed2.guardar().unwrap());
    assert_eq!(elementos(&ruta2).len(), 1);
}

#[test]
fn deshacer_el_borrado_tras_guardar_resucita_la_figura() {
    let t = Temporal::nueva("resucita");
    let ruta = t.fichero("r.excalidraw");
    let ed = EditorLienzo::de_fichero(ruta.clone(), true).unwrap();
    rectangulo(&ed, (0.0, 0.0), (100.0, 80.0));
    assert!(ed.guardar().unwrap());
    ed.seleccionar_todo();
    assert!(ed.borrar_seleccion());
    assert!(ed.guardar().unwrap());
    assert!(vivos(&std::fs::read_to_string(&ruta).unwrap()).is_empty());
    assert!(ed.deshacer());
    assert!(ed.guardar().unwrap());
    let v = elementos(&ruta);
    assert_eq!(v.len(), 1, "la misma figura, no una copia: {v:?}");
    assert_eq!(v[0]["isDeleted"], false);
}

#[test]
fn las_herramientas_nuevas_tienen_nombre_de_vuelta() {
    let t = Temporal::nueva("nombres");
    let ed = EditorLienzo::de_fichero(t.fichero("n.excalidraw"), true).unwrap();
    for (pedido, puesto) in [
        ("lazo", "lazo"),
        ("cota", "cota"),
        ("calibrar", "escalar"),
        ("escala_grafica", "escala_grafica"),
        ("bote", "relleno"),
        ("recortar", "recortar"),
        ("extender", "extender"),
        ("punto", "punto"),
        ("arco", "arco"),
        ("serie", "serie"),
        ("grafito", "grafito"),
        ("flecha_codos", "flecha_codos"),
        ("flecha_libre", "flecha_libre"),
    ] {
        ed.herramienta(pedido.into()).unwrap();
        assert_eq!(ed.herramienta_actual(), puesto);
    }
    assert!(ed.unidades_de_medida().contains(&"cm".to_string()));
}

#[test]
fn el_cursor_del_texto_dice_donde_poner_la_ventanita_del_metodo_de_entrada() {
    let t = Temporal::nueva("cursor");
    let ed = EditorLienzo::de_fichero(t.fichero("t.excalidraw"), true).unwrap();
    assert!(ed.caja_del_cursor().is_none());
    ed.herramienta("texto".into()).unwrap();
    ed.puntero(FasePuntero::Pulsar, 100.0, 50.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, 100.0, 50.0, None, 0, 1.0);
    let c0 = ed.caja_del_cursor().expect("escribiendo hay cursor");
    assert!(c0.y1 > c0.y0);
    assert!(ed.teclear("Hola".into()));
    let c1 = ed.caja_del_cursor().unwrap();
    assert!(c1.x0 > c0.x0, "el cursor avanza con lo escrito");
    assert!(ed.terminar_texto());
    assert!(ed.caja_del_cursor().is_none());
}

#[test]
fn el_arco_y_la_serie_de_la_barra_de_windows_dibujan_y_se_guardan() {
    let t = Temporal::nueva("arco-serie");
    let ruta = t.fichero("a.excalidraw");
    let ed = EditorLienzo::de_fichero(ruta.clone(), true).unwrap();
    ed.herramienta("arco".into()).unwrap();
    arrastre(&ed, (10.0, 10.0), (120.0, 90.0));
    ed.herramienta("serie".into()).unwrap();
    ed.puntero(FasePuntero::Pulsar, 200.0, 50.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, 200.0, 50.0, None, 0, 1.0);
    assert!(ed.guardar().unwrap());
    let v = elementos(&ruta);
    assert_eq!(v.len(), 2, "el arco y el numerito: {v:?}");
    // Y se vuelven a leer: no se pierden al reabrir el lienzo.
    let otra = EditorLienzo::de_fichero(ruta.clone(), false).unwrap();
    assert!(otra.caja().is_some());
    // Caso negativo: un nombre que no existe sigue siendo un error.
    assert!(ed.herramienta("arco_iris".into()).is_err());
}
