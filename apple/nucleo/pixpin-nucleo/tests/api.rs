//! La API del puente tal como la vera Swift: carpetas de datos de verdad en
//! un temporal, chats por paginas, el lienzo empaquetado y una vuelta de
//! sincronizar por un socket de esta maquina.

use std::path::PathBuf;
use std::sync::Arc;

use pixpin_motor2d::excalidraw;
use pixpin_motor2d::pintado::{self, Orden};
use pixpin_nucleo::lienzo::{self, decodificar};
use pixpin_nucleo::{ErrorNucleo, Lienzo, Nucleo};
use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{self, Mensaje};

/// Una carpeta temporal que se borra al terminar.
struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir =
            std::env::temp_dir().join(format!("pixpin-nucleo-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
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

#[test]
fn abrir_crea_la_identidad_y_la_conserva() {
    let t = Temporal::nueva("identidad");
    let n = t.nucleo("Mac de Max");
    let a = n.aparato().unwrap();
    assert_eq!(a.nombre, "Mac de Max");
    assert_eq!(a.codigo.chars().count(), 4);
    assert_eq!(a.grupo, None);
    // Abrirla otra vez (o con otro nombre) no cambia quien es.
    let otra = Nucleo::abrir(n.raiz(), "Otro".into()).unwrap();
    assert_eq!(otra.aparato().unwrap(), a);
}

#[test]
fn un_chat_se_lee_por_paginas() {
    let t = Temporal::nueva("paginas");
    let n = t.nucleo("Mac");
    assert!(n.chats().unwrap().is_empty());
    let obra = n.crear_chat("Obra".into()).unwrap();
    // Recien creado no tiene nada, ni carpeta.
    let vacia = n.ultimos_mensajes(obra.id.clone(), 50).unwrap();
    assert_eq!((vacia.total, vacia.mensajes.len()), (0, 0));

    for i in 0..120 {
        n.escribir_nota(obra.id.clone(), format!("nota {i}"), None)
            .unwrap();
    }
    let ultima = n.ultimos_mensajes(obra.id.clone(), 50).unwrap();
    assert_eq!(ultima.total, 120);
    assert_eq!(ultima.desde, 70);
    assert_eq!(ultima.mensajes.len(), 50);
    assert_eq!(ultima.mensajes[0].texto, "nota 70");
    assert_eq!(ultima.mensajes[49].texto, "nota 119");

    let primera = n.mensajes(obra.id.clone(), 0, 50).unwrap();
    assert_eq!(primera.mensajes[0].texto, "nota 0");
    assert_eq!(primera.mensajes[0].clase, "NOTA");
    let fin = n.mensajes(obra.id.clone(), 100, 50).unwrap();
    assert_eq!(fin.mensajes.len(), 20, "la ultima pagina va corta");
    let fuera = n.mensajes(obra.id.clone(), 500, 50).unwrap();
    assert!(fuera.mensajes.is_empty());

    // Numeros seguidos y unicos, con el codigo de este aparato.
    let codigo = n.aparato().unwrap().codigo;
    let todos: Vec<_> = (0..3)
        .flat_map(|p| n.mensajes(obra.id.clone(), p * 50, 50).unwrap().mensajes)
        .collect();
    assert_eq!(todos.len(), 120);
    for (i, m) in todos.iter().enumerate() {
        assert_eq!(
            m.codigo_chat.as_deref(),
            Some(format!("{}·{codigo}", i + 1).as_str())
        );
    }
    let ids: std::collections::BTreeSet<_> = todos.iter().map(|m| &m.id).collect();
    assert_eq!(ids.len(), 120, "dos notas seguidas no comparten id");

    // La ficha sube con la ultima linea.
    let lista = n.chats().unwrap();
    assert_eq!(lista[0].id, obra.id);
    assert_eq!(lista[0].resumen, "nota 119");

    // Y lo escrito es el cuaderno de siempre: lo lee el codigo de Windows.
    let carpeta = almacen::carpeta(std::path::Path::new(&n.raiz()), &obra.id);
    let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
    assert_eq!(c.mensajes.len(), 120);
    assert_eq!(c.siguiente_numero(Some(&obra.id)), 121);
}

#[test]
fn los_de_sistema_no_salen_y_las_lineas_rotas_se_cuentan() {
    let t = Temporal::nueva("sistema");
    let n = t.nucleo("Mac");
    let chat = n.crear_chat("Asistente de pruebas".into()).unwrap();
    n.escribir_nota(chat.id.clone(), "hola".into(), None)
        .unwrap();
    let carpeta = almacen::carpeta(std::path::Path::new(&n.raiz()), &chat.id);
    // Lo que escribe otro (la sincronizacion, el chat de Windows) por fuera.
    cuaderno::anadir(
        &carpeta,
        &Mensaje {
            id: "auto-letra-x".into(),
            numero: 40,
            texto: "no es conversacion".into(),
            ..Default::default()
        },
    )
    .unwrap();
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(carpeta.join("guardados.jsonl"))
            .unwrap();
        writeln!(f, "{{esto no es json").unwrap();
    }
    let p = n.ultimos_mensajes(chat.id.clone(), 10).unwrap();
    assert_eq!(p.total, 1);
    assert_eq!(p.lineas_rotas, 1);
    // El siguiente numero cuenta tambien los de sistema (el mayor + 1).
    let m = n
        .escribir_nota(
            chat.id.clone(),
            "otra".into(),
            Some(p.mensajes[0].id.clone()),
        )
        .unwrap();
    assert_eq!(m.codigo_chat.unwrap().split('·').next(), Some("41"));
    assert_eq!(m.responde_a, Some(p.mensajes[0].id.clone()));
    // La cache se entera de lo nuevo.
    let p = n.ultimos_mensajes(chat.id.clone(), 10).unwrap();
    assert_eq!(p.total, 2);
    assert_eq!(p.mensajes[1].texto, "otra");
}

#[test]
fn la_cache_tiene_tope_y_se_vacia() {
    let t = Temporal::nueva("cache");
    let n = t.nucleo("Mac");
    for i in 0..12 {
        let c = n.crear_chat(format!("chat {i}")).unwrap();
        n.escribir_nota(c.id.clone(), "x".into(), None).unwrap();
        n.ultimos_mensajes(c.id, 10).unwrap();
    }
    assert_eq!(n.chats_en_cache(), pixpin_nucleo::chats::TOPE_DE_CHATS);
    n.vaciar_caches();
    assert_eq!(n.chats_en_cache(), 0);
}

#[test]
fn ids_que_no_valen() {
    let t = Temporal::nueva("ids");
    let n = t.nucleo("Mac");
    assert!(matches!(
        n.escribir_nota("no-esta".into(), "x".into(), None),
        Err(ErrorNucleo::NoExiste { .. })
    ));
    for malo in ["", "..", "../fuera", "a/b", "a\\b"] {
        assert!(matches!(
            n.mensajes(malo.into(), 0, 10),
            Err(ErrorNucleo::Argumento { .. })
        ));
        assert!(matches!(
            n.abrir_lienzo(malo.into(), "d".into()),
            Err(ErrorNucleo::Argumento { .. })
        ));
    }
    assert!(matches!(
        n.abrir_lienzo("p".into(), "no-esta".into()),
        Err(ErrorNucleo::NoExiste { .. })
    ));
}

const LIENZO: &str = r##"{
  "type": "excalidraw", "version": 2, "source": "pixpin-android",
  "elements": [
    {"id":"a1","type":"rectangle","x":10,"y":20,"width":100,"height":50,
     "strokeColor":"#1e1e1e","backgroundColor":"#ffc9c9","fillStyle":"solid","strokeWidth":2,
     "strokeStyle":"dashed","roughness":1,"opacity":100,"seed":12345},
    {"id":"t","type":"freedraw","x":100,"y":200,"width":10,"height":10,
     "points":[[0,0],[5,5],[10,10],[20,4]]},
    {"id":"x","type":"text","x":0,"y":300,"width":50,"height":20,"text":"Cota ñ",
     "fontSize":20,"fontFamily":5},
    {"id":"i1","type":"image","x":5000,"y":5000,"width":160,"height":110,
     "fileId":"7xJoKC","strokeColor":"#1e1e1e"},
    {"id":"c1","type":"pixpin-gantt","x":0,"y":0,"width":80,"height":10}
  ],
  "appState": {"viewBackgroundColor": "#fafafa"},
  "files": {"7xJoKC":{"id":"7xJoKC","mimeType":"image/png","path":"imagenes/7xJoKC"}}
}"##;

#[test]
fn el_lienzo_cruza_en_un_bufer_y_vuelve_igual() {
    let t = Temporal::nueva("lienzo");
    let n = t.nucleo("Mac");
    let raiz = PathBuf::from(n.raiz());
    let fichero = almacen::lienzo(&raiz, "p1", "d1");
    std::fs::create_dir_all(fichero.parent().unwrap()).unwrap();
    std::fs::write(&fichero, LIENZO).unwrap();

    let l = n.abrir_lienzo("p1".into(), "d1".into()).unwrap();
    assert_eq!(l.cuantos_elementos(), 4, "lo ajeno (gantt) no se pinta");
    let plano = l.plano();
    let (cab, ordenes) = decodificar(&plano).unwrap();

    // Lo mismo que sacaria el motor directamente, orden por orden.
    let escena = excalidraw::a_escena(&excalidraw::leer(LIENZO).unwrap());
    let esperadas = pintado::ordenes_de_escena(&escena);
    assert!(!esperadas.is_empty());
    assert_eq!(ordenes, esperadas);
    assert_eq!(cab.ordenes, esperadas.len());
    assert_eq!(cab.fondo, escena.fondo);
    let caja = l.caja().unwrap();
    assert_eq!(cab.caja, Some((caja.x0, caja.y0, caja.x1, caja.y1)));
    assert!(
        ordenes
            .iter()
            .any(|o| matches!(o, Orden::Texto { texto, .. } if texto == "Cota ñ"))
    );

    // La imagen: su id en la tabla del bufer y su fichero, una vez.
    let imagenes = l.imagenes();
    assert_eq!(imagenes.len(), 1);
    assert_eq!(imagenes[0].ruta, "imagenes/7xJoKC");
    assert_eq!(plano.imagenes, vec![imagenes[0].id_objeto]);

    // Un fotograma de la vista: lo de lejos (la imagen) no entra.
    let vista = l.plano_de_vista(0.0, 0.0, 1.0, 800.0, 600.0);
    let (_, de_vista) = decodificar(&vista).unwrap();
    assert!(!de_vista.is_empty());
    assert!(!de_vista.iter().any(|o| matches!(o, Orden::Imagen { .. })));
    assert!(vista.imagenes.is_empty());

    // Y un JSON roto es un error de lienzo, no un panico.
    assert!(matches!(
        Lienzo::de_json("{".into()),
        Err(ErrorNucleo::Lienzo { .. })
    ));
    assert_eq!(lienzo::VERSION_PLANO, 1);
}

#[test]
fn adjuntar_copia_el_fichero_y_escribe_su_mensaje_como_windows() {
    let t = Temporal::nueva("adjuntar");
    let n = t.nucleo("Mac");
    let raiz = PathBuf::from(n.raiz());
    let obra = n.crear_chat("Obra".into()).unwrap();
    n.escribir_nota(obra.id.clone(), "antes".into(), None)
        .unwrap();

    // Una foto con el nombre en NFD, como lo da el Finder.
    let fuera = t.0.join("fuera");
    std::fs::create_dir_all(&fuera).unwrap();
    let foto = fuera.join("fachada n\u{303}.PNG");
    std::fs::write(&foto, b"\x89PNG\r\n\x1a\nfalso").unwrap();
    let m = n
        .adjuntar(obra.id.clone(), foto.display().to_string(), None)
        .unwrap();
    assert_eq!(m.clase, "IMAGEN");
    assert_eq!(m.nombre, "fachada \u{f1}.PNG", "el nombre en NFC");
    assert_eq!(m.ruta.as_deref(), Some("archivos/fachada \u{f1}.PNG"));
    assert_eq!(m.bytes, 13);
    let codigo = n.aparato().unwrap().codigo;
    assert_eq!(m.codigo_chat, Some(format!("2\u{b7}{codigo}")));
    // El fichero copiado, donde dice la ruta y con lo mismo dentro.
    let absoluta = n
        .ruta_absoluta(obra.id.clone(), m.ruta.clone().unwrap())
        .unwrap();
    assert_eq!(
        PathBuf::from(&absoluta),
        almacen::carpeta(&raiz, &obra.id).join("archivos/fachada \u{f1}.PNG")
    );
    assert_eq!(
        std::fs::read(&absoluta).unwrap(),
        std::fs::read(&foto).unwrap()
    );

    // Otro con el mismo nombre no pisa el primero; con nombre propio, ese.
    let otra = n
        .adjuntar(obra.id.clone(), foto.display().to_string(), None)
        .unwrap();
    assert_eq!(
        otra.ruta.as_deref(),
        Some("archivos/fachada \u{f1} (1).PNG")
    );
    let doc = fuera.join("x.bin");
    std::fs::write(&doc, b"hola").unwrap();
    let a = n
        .adjuntar(
            obra.id.clone(),
            doc.display().to_string(),
            Some("plano: v2.pdf".into()),
        )
        .unwrap();
    assert_eq!(a.clase, "ARCHIVO");
    assert_eq!(a.nombre, "plano: v2.pdf");
    assert_eq!(
        a.ruta.as_deref(),
        Some("archivos/plano_ v2.pdf"),
        "nombre_seguro"
    );

    // En el cuaderno, como cualquier mensaje: la pagina lo trae y el chat sube.
    let pagina = n.ultimos_mensajes(obra.id.clone(), 10).unwrap();
    assert_eq!(pagina.total, 4);
    // La pagina trae ademas su punto de proyecto: una foto solo en el chat, rojo.
    assert_eq!(pagina.mensajes[1].punto, Some(false));
    assert_eq!(
        pixpin_nucleo::chats::MensajeChat {
            punto: None,
            ..pagina.mensajes[1].clone()
        },
        m
    );
    let en_disco = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&raiz, &obra.id)).unwrap();
    let linea: &Mensaje = &en_disco.mensajes[1];
    assert_eq!(linea.clase, Some(cuaderno::Clase::Imagen));
    assert_eq!(linea.ruta.as_deref(), Some("archivos/fachada \u{f1}.PNG"));
    let ficha = n
        .chats()
        .unwrap()
        .into_iter()
        .find(|f| f.id == obra.id)
        .unwrap();
    assert_eq!(ficha.resumen, "plano: v2.pdf");

    // Lo que no vale: un chat que no esta (sin dejar fichero), un origen
    // que no esta o que es una carpeta, rutas con `..`.
    assert!(matches!(
        n.adjuntar("no-esta".into(), foto.display().to_string(), None),
        Err(ErrorNucleo::NoExiste { .. })
    ));
    assert!(!almacen::carpeta(&raiz, "no-esta").exists());
    assert!(matches!(
        n.adjuntar(
            obra.id.clone(),
            fuera.join("nada").display().to_string(),
            None
        ),
        Err(ErrorNucleo::NoExiste { .. })
    ));
    assert!(matches!(
        n.adjuntar(obra.id.clone(), fuera.display().to_string(), None),
        Err(ErrorNucleo::Argumento { .. })
    ));
    assert!(matches!(
        n.adjuntar("../x".into(), foto.display().to_string(), None),
        Err(ErrorNucleo::Argumento { .. })
    ));
    for mala in [
        "",
        "../otro/guardados.jsonl",
        "archivos/../../x",
        "/etc/passwd",
        "C:\\x",
    ] {
        assert!(
            matches!(
                n.ruta_absoluta(obra.id.clone(), mala.into()),
                Err(ErrorNucleo::Argumento { .. })
            ),
            "{mala:?}"
        );
    }
}

#[test]
fn una_nota_de_voz_se_guarda_y_se_pasa_a_texto_como_en_windows() {
    let t = Temporal::nueva("voz");
    let n = t.nucleo("Mac");
    let raiz = PathBuf::from(n.raiz());
    let obra = n.crear_chat("Obra".into()).unwrap();
    let fuera = t.0.join("fuera");
    std::fs::create_dir_all(&fuera).unwrap();
    let audio = fuera.join("grabando.m4a");
    std::fs::write(&audio, b"no es audio de verdad").unwrap();

    // Mas picos de la cuenta y fuera de rango: se acotan como en el movil.
    let picos: Vec<i32> = (0..600).map(|i| if i == 0 { -5 } else { 40_000 }).collect();
    let m = n
        .adjuntar_voz(obra.id.clone(), audio.display().to_string(), 4200, picos)
        .unwrap();
    assert_eq!(m.clase, "VOZ");
    assert!(
        m.nombre.starts_with("voz_") && m.nombre.ends_with(".m4a"),
        "{}",
        m.nombre
    );
    assert_eq!(
        m.ruta.as_deref(),
        Some(format!("archivos/{}", m.nombre).as_str())
    );
    assert_eq!(m.duracion_ms, 4200);
    assert!(m.picos.len() <= 256 && !m.picos.is_empty());
    assert!(m.picos.iter().all(|p| (0..=32767).contains(p)));
    assert_eq!(m.transcripcion, None);
    // El origen se queda donde estaba (lo borra quien grabo).
    assert!(audio.exists());

    // En el disco: una sola linea, con `duracionMs` y `picos` del movil.
    let carpeta = almacen::carpeta(&raiz, &obra.id);
    let texto = std::fs::read_to_string(carpeta.join("guardados.jsonl")).unwrap();
    assert_eq!(texto.lines().count(), 1);
    let linea: serde_json::Value = serde_json::from_str(texto.lines().next().unwrap()).unwrap();
    assert_eq!(linea["clase"], "VOZ");
    assert_eq!(linea["duracionMs"], 4200);
    assert!(linea["picos"].as_array().unwrap().len() <= 256);

    // El texto: con sus minutos y su estado, sin tocar lo demas de la linea.
    let hecho = n
        .poner_transcripcion(
            obra.id.clone(),
            m.id.clone(),
            vec![
                pixpin_nucleo::SegmentoVoz {
                    desde_ms: 0,
                    texto: "llamar al".into(),
                    hasta_ms: 0,
                },
                pixpin_nucleo::SegmentoVoz {
                    desde_ms: 600,
                    texto: "aparejador".into(),
                    hasta_ms: 0,
                },
                pixpin_nucleo::SegmentoVoz {
                    desde_ms: 83_000,
                    texto: "por los niveles".into(),
                    hasta_ms: 0,
                },
            ],
        )
        .unwrap();
    assert_eq!(
        hecho.transcripcion.as_deref(),
        Some("[0:00] llamar al aparejador\n\n[1:23] por los niveles")
    );
    assert_eq!(hecho.resumen, hecho.transcripcion.clone().unwrap());
    assert_eq!(hecho.duracion_ms, 4200);
    assert_eq!(hecho.picos, m.picos);
    let leido = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
    let guardado = &leido.mensajes[0];
    assert_eq!(guardado.transcripcion, hecho.transcripcion);
    assert_eq!(
        guardado.resto.get("estadoDelTexto"),
        Some(&serde_json::json!("bien"))
    );
    assert!(guardado.resto.contains_key("picos"));
    // La pagina lo trae ya con su texto (la cache se rehizo).
    let pagina = n.ultimos_mensajes(obra.id.clone(), 10).unwrap();
    assert_eq!(pagina.mensajes, vec![hecho.clone()]);

    // Sin nada reconocido: texto vacio y `mal`, como Windows.
    let vacio = n
        .poner_transcripcion(obra.id.clone(), m.id.clone(), Vec::new())
        .unwrap();
    assert_eq!(vacio.transcripcion.as_deref(), Some(""));
    let leido = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
    assert_eq!(
        leido.mensajes[0].resto.get("estadoDelTexto"),
        Some(&serde_json::json!("mal"))
    );

    // Lo que no vale: un mensaje que no esta, uno que no es de voz, un
    // chat sin cuaderno.
    assert!(matches!(
        n.poner_transcripcion(obra.id.clone(), "no-esta".into(), Vec::new()),
        Err(ErrorNucleo::NoExiste { .. })
    ));
    let nota = n
        .escribir_nota(obra.id.clone(), "texto".into(), None)
        .unwrap();
    assert!(matches!(
        n.poner_transcripcion(obra.id.clone(), nota.id.clone(), Vec::new()),
        Err(ErrorNucleo::Argumento { .. })
    ));
    let otro = n.crear_chat("Vacio".into()).unwrap();
    assert!(matches!(
        n.poner_transcripcion(otro.id, m.id, Vec::new()),
        Err(ErrorNucleo::NoExiste { .. })
    ));
}

#[test]
fn las_fotos_de_un_lienzo_salen_con_su_ruta_absoluta() {
    let t = Temporal::nueva("fotos-lienzo");
    let n = t.nucleo("Mac");
    let raiz = PathBuf::from(n.raiz());
    let fichero = almacen::lienzo(&raiz, "p1", "d1");
    std::fs::create_dir_all(fichero.parent().unwrap()).unwrap();
    std::fs::write(&fichero, LIENZO).unwrap();
    let l = n.abrir_lienzo("p1".into(), "d1".into()).unwrap();
    let fotos = n.imagenes_del_lienzo("p1".into(), Arc::clone(&l)).unwrap();
    assert_eq!(fotos.len(), 1);
    assert_eq!(fotos[0].id_objeto, l.imagenes()[0].id_objeto);
    assert_eq!(
        PathBuf::from(&fotos[0].ruta),
        almacen::carpeta(&raiz, "p1").join("imagenes/7xJoKC")
    );
    // Es lo mismo que resolver la relativa a mano.
    assert_eq!(
        n.ruta_absoluta("p1".into(), "imagenes/7xJoKC".into())
            .unwrap(),
        fotos[0].ruta
    );
    // Una foto con una ruta de otro disco no sale.
    let ajena = LIENZO.replace("imagenes/7xJoKC", "/sdcard/x.png");
    let l2 = Lienzo::de_json(ajena).unwrap();
    assert!(n.imagenes_del_lienzo("p1".into(), l2).unwrap().is_empty());
}

#[test]
fn dos_aparatos_se_unen_y_sincronizan_por_la_red() {
    let t = Temporal::nueva("sincro");
    let pc = t.nucleo("PC");
    let mac = t.nucleo("Mac");

    let codigo = pc.crear_grupo().unwrap();
    assert!(pixpin_nucleo::sincro::codigo_valido(codigo.clone()));
    assert!(matches!(
        pc.crear_grupo(),
        Err(ErrorNucleo::Argumento { .. })
    ));
    assert_eq!(
        pixpin_nucleo::sincro::etiqueta_de_grupo(codigo.clone()).len(),
        16
    );

    let escucha = Arc::clone(&pc).escuchar(0).unwrap();
    let puerto = escucha.puerto();
    let atender = |e: Arc<pixpin_nucleo::Escucha>| std::thread::spawn(move || e.atender_una());

    // 1. El Mac se une con el codigo (con guion y en minusculas, como se
    //    dicta).
    let hilo = atender(Arc::clone(&escucha));
    let dictado = pixpin_nucleo::sincro::codigo_legible(codigo.clone()).to_lowercase();
    let yo = mac
        .unirse_a_grupo("127.0.0.1".into(), puerto, dictado)
        .unwrap();
    assert!(hilo.join().unwrap().unwrap());
    assert_eq!(yo.grupo.as_deref(), Some(codigo.as_str()));
    assert!(yo.letra.is_some());

    // 2. Escribe en un chat y sincroniza.
    let obra = mac.crear_chat("Obra".into()).unwrap();
    mac.escribir_nota(obra.id.clone(), "desde el Mac".into(), None)
        .unwrap();
    // (Con `atender`, que dice quien vino: el Mac.)
    let e = Arc::clone(&escucha);
    let hilo = std::thread::spawn(move || e.atender());
    let r = mac.sincronizar_con("127.0.0.1".into(), puerto).unwrap();
    assert_eq!(
        hilo.join().unwrap().unwrap(),
        pixpin_nucleo::Atendido::Aparato {
            id: mac.aparato().unwrap().id,
            nombre: "Mac".into()
        }
    );
    assert_eq!(r.otro_nombre, "PC");
    assert!(r.enviados >= 1, "{r:?}");

    // En el PC esta el chat con su nota.
    let alli = pc.chats().unwrap();
    let suya = alli
        .iter()
        .find(|c| c.nombre == "Obra")
        .expect("el chat llego");
    let p = pc.ultimos_mensajes(suya.id.clone(), 10).unwrap();
    assert_eq!(p.mensajes.len(), 1);
    assert_eq!(p.mensajes[0].texto, "desde el Mac");

    // 3. Cerrar despierta a quien espera, sin que llegue nadie.
    let hilo = atender(Arc::clone(&escucha));
    std::thread::sleep(std::time::Duration::from_millis(50));
    escucha.cerrar();
    assert!(!hilo.join().unwrap().unwrap());

    // Un codigo mal escrito no llega a la red.
    assert!(matches!(
        mac.unirse_a_grupo("127.0.0.1".into(), puerto, "abc".into()),
        Err(ErrorNucleo::Argumento { .. })
    ));
}

/// Cuenta los avisos de [`pixpin_nucleo::sincro::AvisoDeVuelta`].
#[derive(Default)]
struct Avisos {
    empiezan: std::sync::atomic::AtomicU32,
    bien: std::sync::atomic::AtomicU32,
    mal: std::sync::atomic::AtomicU32,
}

struct Aviso(Arc<Avisos>);

impl pixpin_nucleo::sincro::AvisoDeVuelta for Aviso {
    fn empieza(&self) {
        self.0.empiezan.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    fn acaba(&self, bien: bool) {
        let n = if bien { &self.0.bien } else { &self.0.mal };
        n.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn una_llamada_de_fuera_avisa_al_empezar_y_al_acabar_y_una_sonda_no() {
    use std::io::{Read, Write};
    use std::sync::atomic::Ordering;
    let t = Temporal::nueva("avisos-de-vuelta");
    let pc = t.nucleo("PC");
    let escucha = Arc::clone(&pc).escuchar(0).unwrap();
    let puerto = escucha.puerto();
    let avisos = Arc::new(Avisos::default());

    // Una sonda no es una vuelta: el boton no gira por ella.
    let (e, a) = (Arc::clone(&escucha), Arc::clone(&avisos));
    let hilo = std::thread::spawn(move || e.atender_avisando(Box::new(Aviso(a))));
    let mut s = std::net::TcpStream::connect(("127.0.0.1", puerto)).unwrap();
    s.write_all(pixpin_sincro::SONDA).unwrap();
    let mut r = [0u8; 4];
    s.read_exact(&mut r).unwrap();
    assert_eq!(hilo.join().unwrap().unwrap(), pixpin_nucleo::Atendido::Sonda);
    assert_eq!(avisos.empiezan.load(Ordering::SeqCst), 0);

    // Una llamada que se corta: empieza y acaba mal (la marca roja).
    let (e, a) = (Arc::clone(&escucha), Arc::clone(&avisos));
    let hilo = std::thread::spawn(move || e.atender_avisando(Box::new(Aviso(a))));
    let mut s = std::net::TcpStream::connect(("127.0.0.1", puerto)).unwrap();
    s.write_all(b"PXS1 esto no es un saludo").unwrap();
    drop(s);
    assert!(hilo.join().unwrap().is_err());
    assert_eq!(avisos.empiezan.load(Ordering::SeqCst), 1);
    assert_eq!(avisos.mal.load(Ordering::SeqCst), 1);
    assert_eq!(avisos.bien.load(Ordering::SeqCst), 0);
}

#[test]
fn una_sonda_se_contesta_sin_sincronizar() {
    use std::io::{Read, Write};
    let t = Temporal::nueva("sonda");
    let pc = t.nucleo("PC");
    let escucha = Arc::clone(&pc).escuchar(0).unwrap();
    let puerto = escucha.puerto();
    let e = Arc::clone(&escucha);
    let hilo = std::thread::spawn(move || e.atender());
    let mut s = std::net::TcpStream::connect(("127.0.0.1", puerto)).unwrap();
    s.write_all(pixpin_sincro::SONDA).unwrap();
    let mut r = [0u8; 4];
    s.read_exact(&mut r).unwrap();
    assert_eq!(&r, pixpin_sincro::SONDA_RESPUESTA);
    assert_eq!(
        hilo.join().unwrap().unwrap(),
        pixpin_nucleo::Atendido::Sonda
    );
}

#[test]
fn el_anuncio_de_bonjour_sale_con_el_grupo() {
    let t = Temporal::nueva("anuncio");
    let pc = t.nucleo("PC de casa");
    // Sin grupo no se anuncia nada.
    assert_eq!(pc.anuncio().unwrap(), None);
    assert_eq!(pc.etiqueta().unwrap(), None);

    let codigo = pc.crear_grupo().unwrap();
    let a = pc.anuncio().unwrap().expect("con grupo hay anuncio");
    let yo = pc.aparato().unwrap();
    assert_eq!(a.instancia, "PixPin PC de casa");
    assert_eq!(a.tipo, pixpin_nucleo::sincro::tipo_de_servicio());
    assert_eq!(a.etiqueta, pixpin_nucleo::sincro::etiqueta_de_grupo(codigo));
    assert_eq!(pc.etiqueta().unwrap().as_deref(), Some(a.etiqueta.as_str()));
    assert_eq!(a.id, yo.id);
    let claves: Vec<_> = a.txt.iter().map(|p| p.clave.as_str()).collect();
    assert_eq!(claves, ["g", "id", "l", "n"]);
    assert_eq!(a.txt[0].valor, a.etiqueta);
    assert_eq!(a.txt[2].valor, "a", "quien crea el grupo es la a");
}
