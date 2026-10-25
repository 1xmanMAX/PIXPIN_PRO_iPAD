//! **El envio por codigo completo** (A6), de punta a punta entre dos
//! carpetas de datos por 127.0.0.1, como «Enviar por Wi-Fi» y «Recibir por
//! Wi-Fi» de Windows:
//!
//! - Un proyecto entero entra en la lista del otro; si vuelve, se ofrece
//!   ponerlo al dia (y se deja copia) o crearlo como nuevo.
//! - Un lienzo sale como lienzo y cae en su proyecto al otro lado, no como
//!   un `.excalidraw` suelto en «Mensajes guardados».
//! - «No, gracias» no trae nada; quien espera con su puerta tambien
//!   pregunta.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pixpin_nucleo::envio::{DecidirOferta, FinEnvio, FinRecibir, OfertaRecibida};
use pixpin_nucleo::Nucleo;

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-nucleo-envio-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temporal(dir)
    }

    fn nucleo(&self, nombre: &str) -> Arc<Nucleo> {
        Nucleo::abrir(self.0.join(nombre).display().to_string(), nombre.into()).unwrap()
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Contesta siempre lo mismo y apunta lo que vio.
struct Contesta {
    vistas: Arc<Mutex<Vec<OfertaRecibida>>>,
    nuevos: Option<Vec<String>>,
}

impl DecidirOferta for Contesta {
    fn decidir(&self, oferta: OfertaRecibida) -> Option<Vec<String>> {
        self.vistas.lock().unwrap().push(oferta);
        self.nuevos.clone()
    }
}

fn contesta(nuevos: Option<Vec<String>>) -> (Box<Contesta>, Arc<Mutex<Vec<OfertaRecibida>>>) {
    let vistas = Arc::new(Mutex::new(Vec::new()));
    (
        Box::new(Contesta {
            vistas: Arc::clone(&vistas),
            nuevos,
        }),
        vistas,
    )
}

/// Abre la puerta de envio de `pc` con `abrir`, la atiende en un hilo y
/// recibe en `mac` preguntando a `decide`.
fn por_codigo(
    pc: &Nucleo,
    abrir: impl FnOnce(&Nucleo) -> Arc<pixpin_nucleo::PuertaEnvio>,
    mac: &Nucleo,
    decide: Box<Contesta>,
) -> Option<Vec<pixpin_nucleo::RecibidoEnvio>> {
    let p = abrir(pc);
    p.aprobar_todos();
    let atiende = {
        let p = Arc::clone(&p);
        std::thread::spawn(move || p.atender().unwrap())
    };
    let r = mac
        .recibir_envio_preguntando("127.0.0.1".into(), p.puerto(), p.codigo(), decide)
        .unwrap();
    let fin = atiende.join().unwrap();
    p.cerrar();
    if r.is_some() {
        assert!(matches!(fin, FinEnvio::Enviado { .. }), "{fin:?}");
    }
    r
}

#[test]
fn un_proyecto_entero_llega_y_al_volver_se_pone_al_dia_o_entra_como_nuevo() {
    let t = Temporal::nueva("proyecto");
    let pc = t.nucleo("PC");
    let mac = t.nucleo("Mac");
    let tesis = pc.crear_chat("Tesis".into()).unwrap();
    pc.escribir_nota(tesis.id.clone(), "capitulo 1".into(), None)
        .unwrap();
    let cuantas = |n: &Nucleo| {
        n.chats()
            .unwrap()
            .iter()
            .filter(|c| c.nombre == "Tesis")
            .count()
    };

    let (d, vistas) = contesta(Some(vec![]));
    let r = por_codigo(
        &pc,
        |n| n.abrir_envio_de_proyecto(tesis.id.clone()).unwrap(),
        &mac,
        d,
    )
    .unwrap();
    assert_eq!(r[0].donde, "proyecto");
    let o = vistas.lock().unwrap()[0].clone();
    assert_eq!(o.de, "PC");
    assert_eq!(o.cosas[0].tipo, "proyecto");
    assert_eq!(o.cosas[0].ya_tienes, None);
    assert_eq!(cuantas(&mac), 1);
    assert!(
        !t.0.join("PC/envio").read_dir().is_ok_and(|mut d| d.next().is_some()),
        "lo empaquetado se borra al cerrar la puerta"
    );

    // Vuelve: se ofrece «Actualizar el que tengo», y se pone al dia.
    pc.escribir_nota(tesis.id.clone(), "capitulo 2".into(), None)
        .unwrap();
    let (d, vistas) = contesta(Some(vec![]));
    let r = por_codigo(
        &pc,
        |n| n.abrir_envio_de_proyecto(tesis.id.clone()).unwrap(),
        &mac,
        d,
    )
    .unwrap();
    assert_eq!(
        vistas.lock().unwrap()[0].cosas[0].ya_tienes.as_deref(),
        Some("Tesis")
    );
    assert_eq!(r[0].donde, "proyecto_al_dia");
    assert_eq!(cuantas(&mac), 1, "no duplica");
    let textos: Vec<String> = mac
        .ultimos_mensajes(r[0].chat.clone(), 10)
        .unwrap()
        .mensajes
        .into_iter()
        .map(|m| m.texto)
        .collect();
    assert!(textos.contains(&"capitulo 2".to_string()), "{textos:?}");
    assert!(!mac.copias_de_seguridad().is_empty(), "copia de antes");

    // «Crear como nuevo»: aparte.
    let identidad = vistas.lock().unwrap()[0].cosas[0].identidad.clone();
    let (d, _) = contesta(Some(vec![identidad]));
    let r = por_codigo(
        &pc,
        |n| n.abrir_envio_de_proyecto(tesis.id.clone()).unwrap(),
        &mac,
        d,
    )
    .unwrap();
    assert_eq!(r[0].donde, "proyecto");
    assert_eq!(cuantas(&mac), 2);

    // Caso negativo: «No, gracias» no trae nada.
    let (d, _) = contesta(None);
    let r = por_codigo(
        &pc,
        |n| n.abrir_envio_de_proyecto(tesis.id.clone()).unwrap(),
        &mac,
        d,
    );
    assert!(r.is_none());
    assert_eq!(cuantas(&mac), 2);
}

#[test]
fn un_proyecto_que_no_esta_no_se_manda() {
    let t = Temporal::nueva("no-esta");
    let pc = t.nucleo("PC");
    assert!(pc.abrir_envio_de_proyecto("no-esta".into()).is_err());
    assert!(
        !t.0.join("PC/envio").read_dir().is_ok_and(|mut d| d.next().is_some()),
        "sin restos"
    );
}

#[test]
fn un_lienzo_sale_como_lienzo_y_cae_en_su_proyecto() {
    let t = Temporal::nueva("lienzo");
    let pc = t.nucleo("PC");
    let mac = t.nucleo("Mac");
    let obra = pc.crear_chat("Obra".into()).unwrap();
    pc.escribir_nota(obra.id.clone(), "hola".into(), None).unwrap();
    // Un lienzo del proyecto, como lo deja el editor.
    let lienzos = t.0.join("PC/proyectos").join(&obra.id).join("lienzos");
    std::fs::create_dir_all(&lienzos).unwrap();
    let ruta = lienzos.join("d-1.excalidraw");
    std::fs::write(
        &ruta,
        r##"{"type":"excalidraw","version":2,"elements":[
            {"id":"t1","type":"freedraw","x":0,"y":0,"width":20,"height":10,"seed":3,
             "points":[[0,0],[20,10]],"pressures":[]}],
           "appState":{"viewBackgroundColor":"#fff8e1"},"files":{}}"##,
    )
    .unwrap();

    let (d, vistas) = contesta(Some(vec![]));
    let rutas = vec![ruta.display().to_string()];
    let r = por_codigo(&pc, |n| n.abrir_envio(rutas.clone()).unwrap(), &mac, d).unwrap();
    let o = vistas.lock().unwrap()[0].clone();
    assert_eq!(o.cosas[0].tipo, "lienzo", "no como archivo");
    assert_eq!(o.cosas[0].proyecto_nombre.as_deref(), Some("Obra"));
    assert_eq!(r[0].donde, "proyecto");
    // Cae en un proyecto «Obra», no en «Mensajes guardados».
    let suyo = mac
        .chats()
        .unwrap()
        .into_iter()
        .find(|c| c.id == r[0].chat)
        .unwrap();
    assert_eq!(suyo.nombre, "Obra");
    let m = mac.ultimos_mensajes(suyo.id.clone(), 10).unwrap().mensajes;
    assert!(m.iter().any(|m| m.clase == "DIBUJO"), "{m:?}");

    // Otra vez el mismo: se pone al dia en su sitio.
    let (d, _) = contesta(Some(vec![]));
    let r = por_codigo(&pc, |n| n.abrir_envio(rutas.clone()).unwrap(), &mac, d).unwrap();
    assert_eq!(r[0].donde, "proyecto_al_dia");
    let m = mac.ultimos_mensajes(suyo.id, 10).unwrap().mensajes;
    assert_eq!(m.iter().filter(|m| m.clase == "DIBUJO").count(), 1);
}

#[test]
fn quien_espera_con_su_puerta_tambien_pregunta() {
    let t = Temporal::nueva("espera");
    let pc = t.nucleo("PC");
    let mac = t.nucleo("Mac");
    let notas = t.0.join("notas.txt");
    std::fs::write(&notas, "hola").unwrap();

    let puerta = Arc::clone(&mac).esperar_envios().unwrap();
    let (d, vistas) = contesta(None);
    puerta.preguntar_con(d);
    let atiende = {
        let p = Arc::clone(&puerta);
        std::thread::spawn(move || p.atender().unwrap())
    };
    let fin = pc
        .mandar_a_quien_espera(
            "127.0.0.1".into(),
            puerta.puerto(),
            puerta.codigo(),
            vec![notas.display().to_string()],
        )
        .unwrap();
    assert!(matches!(fin, FinEnvio::Rechazado { .. }), "{fin:?}");
    assert!(matches!(
        atiende.join().unwrap(),
        FinRecibir::Rechazado { .. }
    ));
    assert_eq!(vistas.lock().unwrap()[0].cosas[0].nombre, "notas.txt");
    puerta.cerrar();
}
