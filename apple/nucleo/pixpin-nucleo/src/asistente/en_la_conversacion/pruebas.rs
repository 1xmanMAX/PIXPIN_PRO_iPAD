//! El chat con el asistente de mentira (`MotorGuionado`, nunca la red): la
//! respuesta que nace y crece en el cuaderno, el dibujo que acaba en un
//! lienzo del chat, la mencion, parar, la pregunta en la burbuja, lo pensado
//! guardado con su nota y la hoja que pasa al turno siguiente (las de
//! `ventana_chat/asistente/pruebas{,_pensado}.rs` de Windows).

use std::sync::Mutex;

use pixpin_ia::motor::{MotorGuionado, Respuesta};
use pixpin_proyecto::cuaderno::Clase;
use serde_json::Value;

use super::*;

/// Las manos de la app, apuntadas.
#[derive(Default)]
struct Manos {
    si: bool,
    preguntas: Mutex<Vec<String>>,
    pedidos: Mutex<Vec<Value>>,
    vistas: Mutex<Vec<crate::asistente::ojos::VistaParaPintar>>,
}

impl AppIa for Manos {
    fn confirmar(&self, pregunta: String) -> bool {
        self.preguntas.lock().unwrap().push(pregunta);
        self.si
    }
    fn pedido(&self, json: String) -> Option<String> {
        self.pedidos.lock().unwrap().push(serde_json::from_str(&json).unwrap());
        None
    }
    fn leer_texto(&self, _ruta: String, _paginas: Vec<RangoPaginas>) -> Result<String, ErrorNucleo> {
        Ok("Clausula primera: el plazo es de 30 dias.".into())
    }
    fn pintar_vista(&self, vista: crate::asistente::ojos::VistaParaPintar) -> Option<Vec<u8>> {
        self.vistas.lock().unwrap().push(vista);
        Some(b"\x89PNG de mentira".to_vec())
    }
    fn paginas_de_pdf(&self, _ruta: String) -> Vec<crate::asistente::turno::MedidaPagina> {
        Vec::new()
    }
    fn pintar_pagina(&self, _ruta: String, _pagina: u32, _ancho: u32) -> Option<Vec<u8>> {
        None
    }
    fn palabras_de_imagen(&self, _imagen: Vec<u8>) -> Vec<crate::asistente::turno::PalabraLeida> {
        Vec::new()
    }
    fn exportar(&self, _ruta: String, _formato: String, _carpeta: String) -> Result<Vec<String>, ErrorNucleo> {
        Err(ErrorNucleo::Argumento { detalle: "sin exportar en las pruebas".into() })
    }
}

/// Lo que ve la burbuja de Swift.
#[derive(Default)]
struct Ve {
    vistos: Mutex<Vec<VivoIa>>,
}

impl OyenteChatIa for Ve {
    fn vivo(&self, v: VivoIa) {
        self.vistos.lock().unwrap().push(v);
    }
}

fn nucleo(nombre: &str) -> (Arc<Nucleo>, PathBuf) {
    let d = std::env::temp_dir().join(format!("pixpin-chat-ia-{nombre}-{}-{}", std::process::id(), crate::ahora_ms()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    (Nucleo::abrir(d.display().to_string(), "Mac de pruebas".into()).unwrap(), d)
}

fn entorno(pensar: bool) -> Entorno {
    Entorno {
        nombre: "guionado".into(),
        nube: false,
        pensar,
        ventana: 8192,
        cada: Duration::ZERO,
    }
}

/// Escribe `texto` en el chat y, si es para el asistente, contesta con
/// `motor`. Devuelve como acabo (o `None` si no era para el).
fn preguntar(n: &Arc<Nucleo>, chat: &str, texto: &str, motor: &MotorGuionado, manos: Arc<Manos>, ve: Arc<Ve>, corte: &CorteIa) -> Option<FinChatIa> {
    let m = n.escribir_nota(chat.into(), texto.into(), None).unwrap();
    contestar(n, chat, &m.id, motor, manos, ve, corte)
}

fn contestar(n: &Arc<Nucleo>, chat: &str, id: &str, motor: &MotorGuionado, manos: Arc<Manos>, ve: Arc<Ve>, corte: &CorteIa) -> Option<FinChatIa> {
    let inicio = n.ia_al_enviar(chat.into(), id.into()).unwrap()?;
    assert_eq!(inicio.respuesta.texto, ia_chat::ESTADO_INICIAL, "nace pensando");
    Some(
        turno_en_chat_con(motor, &entorno(false), n.clone(), chat.into(), inicio, 0, manos, ve, corte, Duration::ZERO)
            .unwrap(),
    )
}

fn lienzos(n: &Nucleo, chat: &str) -> Vec<Nota> {
    n.notas_de(chat).into_iter().filter(|m| m.clase == Some(Clase::Dibujo)).collect()
}

fn elementos(n: &Nucleo, chat: &str, m: &Nota) -> usize {
    let ruta = almacen::lienzo(n.ruta(), chat, m.referencia.as_deref().unwrap());
    pixpin_motor2d::excalidraw::leer(&std::fs::read_to_string(ruta).unwrap())
        .unwrap()
        .elementos()
        .len()
}

const CIRCULO: &str = r#"{"elementos":[{"tipo":"elipse","x":10,"y":10,"ancho":80,"alto":80,"color":"rojo","relleno":"rojo"}]}"#;

#[test]
fn el_chat_asistente_existe_una_vez_y_va_detras_de_guardados() {
    let (n, d) = nucleo("fijo");
    n.chat_guardados().unwrap();
    let a1 = n.chat_asistente().unwrap();
    let a2 = n.chat_asistente().unwrap();
    assert_eq!(a1.id, a2.id);
    assert!(a1.es_asistente && !a1.es_guardados);
    n.crear_chat("nuevo".into()).unwrap();
    let orden: Vec<String> = n.chats().unwrap().into_iter().map(|f| f.nombre).collect();
    assert_eq!(orden, vec!["Mensajes guardados", "Asistente", "nuevo"]);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn un_dibujo_pedido_en_el_chat_acaba_en_un_lienzo_nuevo_y_luego_en_el_mismo() {
    let (n, d) = nucleo("dibujo");
    let chat = n.chat_asistente().unwrap().id;
    let motor = MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("dibujar_en_lienzo", CIRCULO),
        Respuesta::de_texto("Hecho: un circulo rojo."),
        Respuesta::de_llamada("dibujar_en_lienzo", CIRCULO),
        Respuesta::de_texto("Otro mas."),
    ]);
    let ve = Arc::new(Ve::default());
    let fin = preguntar(&n, &chat, "dibuja un circulo rojo", &motor, Arc::default(), ve.clone(), &CorteIa::nuevo()).unwrap();
    let r = &fin.respuesta;
    assert!(r.texto.starts_with("Hecho: un circulo rojo."), "{}", r.texto);
    assert_eq!(r.codigo_chat.as_deref().map(|c| c.ends_with("IA")), Some(true), "{:?}", r.codigo_chat);
    // Un lienzo nuevo en el chat, suyo, con el circulo dentro y en rojo.
    let ls = lienzos(&n, &chat);
    assert_eq!(ls.len(), 1);
    assert_eq!(ls[0].nombre, super::super::al_lienzo::NOMBRE_LIENZO_PROPIO);
    assert_eq!(ls[0].aparato.as_deref(), Some(ia_chat::APARATO));
    assert_eq!(elementos(&n, &chat, &ls[0]), 1);
    let json = std::fs::read_to_string(almacen::lienzo(n.ruta(), &chat, ls[0].referencia.as_deref().unwrap())).unwrap();
    assert!(json.contains("#e03131"), "{json}");
    // La respuesta lleva su «Abrir lienzo» y lo escrito esta en el disco.
    assert_eq!(r.lienzo_ia.as_deref(), ls[0].referencia.as_deref());
    assert!(n.notas_de(&chat).iter().any(|m| m.texto.starts_with("Hecho: un circulo rojo.")));
    // Swift lo vio crecer y acabar.
    let vistos = ve.vistos.lock().unwrap();
    assert!(vistos.iter().any(|v| v.texto.contains("dibujando")), "{:?}", vistos.iter().map(|v| &v.texto).collect::<Vec<_>>());
    assert!(vistos.last().unwrap().acabado);
    drop(vistos);
    // Sin pedirlo, no se abre solo.
    assert_eq!(fin.abrir_lienzo, None);

    // La segunda vez, al mismo lienzo (a la derecha), sin crear otro.
    preguntar(&n, &chat, "otro", &motor, Arc::default(), Arc::default(), &CorteIa::nuevo()).unwrap();
    let ls = lienzos(&n, &chat);
    assert_eq!(ls.len(), 1);
    assert_eq!(elementos(&n, &chat, &ls[0]), 2);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn en_otro_chat_solo_contesta_con_la_mencion_y_cuelga_de_la_pregunta() {
    let (n, d) = nucleo("mencion");
    let chat = n.crear_chat("obra".into()).unwrap().id;
    let motor = MotorGuionado::nuevo(vec![Respuesta::de_texto("Dicen hola.")]);
    // Sin mencion: no es para el.
    assert!(preguntar(&n, &chat, "hola a todos", &motor, Arc::default(), Arc::default(), &CorteIa::nuevo()).is_none());
    let pregunta = n.escribir_nota(chat.clone(), "@asistente resume esto".into(), None).unwrap();
    let fin = contestar(&n, &chat, &pregunta.id, &motor, Arc::default(), Arc::default(), &CorteIa::nuevo()).unwrap();
    assert_eq!(fin.respuesta.texto, "Dicen hola.");
    assert_eq!(fin.respuesta.responde_a.as_deref(), Some(pregunta.id.as_str()));
    // Y el modelo vio lo de antes del chat, y la pregunta sin la mencion.
    let pedidas = motor.recibido.lock().unwrap();
    assert!(pedidas[0].mensajes.iter().any(|m| m.texto_entero() == "hola a todos"));
    assert_eq!(pedidas[0].mensajes.last().unwrap().texto_entero(), "resume esto");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn parar_a_mitad_deja_lo_escrito_y_lo_dice() {
    let (n, d) = nucleo("parar");
    let chat = n.chat_asistente().unwrap().id;
    let largo: String = (0..400).map(|i| format!("p{i} ")).collect();
    let motor = MotorGuionado::nuevo(vec![Respuesta::de_texto(largo)]);
    let corte = CorteIa::nuevo();
    // Para al primer trozo que vea Swift con texto.
    struct Para(Arc<CorteIa>);
    impl OyenteChatIa for Para {
        fn vivo(&self, v: VivoIa) {
            if v.texto.contains("p1") {
                self.0.cancelar();
            }
        }
    }
    let m = n.escribir_nota(chat.clone(), "cuenta".into(), None).unwrap();
    let inicio = n.ia_al_enviar(chat.clone(), m.id).unwrap().unwrap();
    let fin = turno_en_chat_con(&motor, &entorno(false), n.clone(), chat.clone(), inicio, 0, Arc::<Manos>::default(), Arc::new(Para(corte.clone())), &corte, Duration::ZERO).unwrap();
    let t = &fin.respuesta.texto;
    assert!(t.ends_with("⏹ (parado)"), "{t}");
    assert!(!t.contains("p399"), "no paro");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn sin_motor_la_respuesta_lo_dice() {
    let (n, d) = nucleo("sin-motor");
    let chat = n.chat_asistente().unwrap().id;
    let m = n.escribir_nota(chat.clone(), "hola".into(), None).unwrap();
    let inicio = n.ia_al_enviar(chat.clone(), m.id).unwrap().unwrap();
    // La nube sin clave en el Llavero (un proveedor que no esta).
    let ajustes = AjustesIa {
        motor: "local".into(),
        modelo: String::new(),
        pensar: None,
        respaldo_local: false,
        mejor_vista: false,
        documentos_a_la_nube: String::new(),
        carpeta_modelos: d.join("modelos").display().to_string(),
        puerto_usuario: 0,
    };
    let fin = ia_turno_en_chat(ajustes, n.clone(), chat.clone(), inicio, 0, Arc::<Manos>::default(), Arc::<Ve>::default(), CorteIa::nuevo()).unwrap();
    assert!(fin.respuesta.texto.starts_with("⚠️"), "{}", fin.respuesta.texto);
    assert!(fin.respuesta.texto.contains("modelo local"), "{}", fin.respuesta.texto);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn el_texto_leido_queda_en_el_chat_como_nota_y_como_txt() {
    let (n, d) = nucleo("leido");
    let chat = n.chat_asistente().unwrap().id;
    let pdf = d.join("Contrato.pdf");
    std::fs::write(&pdf, b"%PDF-1.4\n").unwrap();
    n.adjuntar(chat.clone(), pdf.display().to_string(), None).unwrap();
    let motor = MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("ocr", r#"{}"#),
        Respuesta::de_texto("El plazo es de 30 dias."),
    ]);
    let fin = preguntar(&n, &chat, "¿qué plazo pone el contrato?", &motor, Arc::default(), Arc::default(), &CorteIa::nuevo()).unwrap();
    assert_eq!(fin.respuesta.texto, "El plazo es de 30 dias.");
    let notas = n.notas_de(&chat);
    let k = notas.len();
    assert!(notas[k - 2].texto.contains("Clausula primera"), "{:?} {:?}", notas.iter().map(|m| &m.texto).collect::<Vec<_>>(), fin);
    assert_eq!(notas[k - 2].aparato.as_deref(), Some(ia_chat::APARATO));
    let txt = &notas[k - 1];
    assert_eq!(txt.nombre, "Contrato (texto).txt");
    let ruta = n.resolver(&chat, txt.ruta.as_deref().unwrap()).unwrap();
    assert_eq!(std::fs::read_to_string(ruta).unwrap(), "Clausula primera: el plazo es de 30 dias.");
    let _ = std::fs::remove_dir_all(d);
}

fn png_de_prueba(d: &Path, nombre: &str, lado: u32) -> PathBuf {
    let img = pixpin_codec::ImagenRgba {
        ancho: lado,
        alto: lado,
        pixeles: vec![120; (lado * lado * 4) as usize],
    };
    let r = d.join(nombre);
    std::fs::write(&r, pixpin_codec::imagen::codificar_png(&img).unwrap()).unwrap();
    r
}

#[test]
fn una_foto_al_chat_del_asistente_es_pregunta_y_en_otro_chat_no() {
    let (n, d) = nucleo("foto");
    let chat = n.chat_asistente().unwrap().id;
    let foto = n.adjuntar(chat.clone(), png_de_prueba(&d, "plano.png", 64).display().to_string(), None).unwrap();
    let motor = MotorGuionado::nuevo(vec![
        Respuesta::de_texto("Veo un cuadrado gris. ¿Qué quieres que haga?"),
        Respuesta::de_llamada(
            "dibujar_en_lienzo",
            r#"{"elementos":[{"tipo":"rectangulo","x":10,"y":10,"ancho":80,"alto":80,"color":"gris"}]}"#,
        ),
        Respuesta::de_texto("Listo."),
    ]);
    let fin = contestar(&n, &chat, &foto.id, &motor, Arc::default(), Arc::default(), &CorteIa::nuevo()).unwrap();
    assert!(fin.respuesta.texto.starts_with("Veo un cuadrado"), "{}", fin.respuesta.texto);
    // «dibuja eso» la sigue viendo.
    preguntar(&n, &chat, "dibuja eso", &motor, Arc::default(), Arc::default(), &CorteIa::nuevo()).unwrap();
    let pedidas = motor.recibido.lock().unwrap();
    assert!(pedidas[0].mensajes.last().unwrap().tiene_imagen(), "la foto sola llega");
    assert_eq!(pedidas[0].mensajes.last().unwrap().texto_entero(), ia_chat::PREGUNTA_SOLO_FOTO);
    assert!(pedidas[1].mensajes.last().unwrap().tiene_imagen(), "y sigue al pedir dibujarla");
    drop(pedidas);
    // En otro chat, una foto sin mencion no es para el.
    let otro = n.crear_chat("obra".into()).unwrap().id;
    let f2 = n.adjuntar(otro.clone(), png_de_prueba(&d, "otra.png", 16).display().to_string(), None).unwrap();
    assert!(n.ia_al_enviar(otro, f2.id).unwrap().is_none());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn con_mencion_dibuja_en_ese_lienzo_y_si_se_pide_lo_abre() {
    let (n, d) = nucleo("mencionado");
    let chat = n.chat_asistente().unwrap().id;
    let plano = n.crear_lienzo(chat.clone(), Some("Plano casa".into())).unwrap();
    let fachada = n.crear_lienzo(chat.clone(), Some("Fachada".into())).unwrap();
    let motor = MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("dibujar_en_lienzo", CIRCULO),
        Respuesta::de_texto("Hecho en el plano."),
    ]);
    let fin = preguntar(&n, &chat, "dibuja un circulo en @lienzo:Plano_casa y ábrelo", &motor, Arc::default(), Arc::default(), &CorteIa::nuevo()).unwrap();
    let notas = n.notas_de(&chat);
    let de = |id: &str| notas.iter().find(|m| m.id == id).unwrap().clone();
    assert_eq!(elementos(&n, &chat, &de(&plano.id)), 1);
    assert_eq!(elementos(&n, &chat, &de(&fachada.id)), 0);
    assert!(ia_chat::lienzo_propio(&notas).is_none(), "no crea uno propio");
    // Al modelo le llega el nombre, no la mencion en crudo.
    let pedidas = motor.recibido.lock().unwrap();
    assert!(pedidas[0].mensajes.last().unwrap().texto_entero().contains("el lienzo «Plano casa»"));
    // «…y ábrelo».
    assert_eq!(fin.abrir_lienzo, plano.referencia);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn con_un_lienzo_abierto_va_al_abierto_y_no_al_chat() {
    let (n, d) = nucleo("abierto");
    let chat = n.chat_asistente().unwrap().id;
    let mio = n.crear_lienzo(chat.clone(), Some("Mio".into())).unwrap();
    let editor = n.editar_lienzo(chat.clone(), mio.referencia.clone().unwrap()).unwrap();
    // Lo que se ve: el dibujo cae en su centro.
    let _ = editor.plano_de_vista(1000.0, 1000.0, 1.0, 400.0, 300.0);
    let motor = MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("dibujar_en_lienzo", CIRCULO),
        Respuesta::de_texto("Hecho."),
    ]);
    let version = editor.version();
    let fin = preguntar(&n, &chat, "dibuja", &motor, Arc::default(), Arc::default(), &CorteIa::nuevo()).unwrap();
    assert_eq!(editor.cuantos_elementos(), 1);
    assert!(editor.version() > version, "la ventana se entera y repinta");
    assert!(editor.sin_guardar(), "se guarda como lo hecho con el raton");
    let c = editor.caja().unwrap();
    assert!(((c.x0 + c.x1) / 2.0 - 1200.0).abs() < 2.0, "{c:?}");
    assert!(ia_chat::lienzo_propio(&n.notas_de(&chat)).is_none(), "no crea el suyo");
    assert_eq!(fin.respuesta.lienzo_ia, mio.referencia, "es de este chat: su «Abrir lienzo»");
    // Cerrada su ventana, ya no va alli.
    editor.cerrado();
    preguntar(&n, &chat, "dibuja", &MotorGuionado::nuevo(vec![Respuesta::de_llamada("dibujar_en_lienzo", CIRCULO), Respuesta::de_texto("Hecho.")]), Arc::default(), Arc::default(), &CorteIa::nuevo()).unwrap();
    assert_eq!(editor.cuantos_elementos(), 1);
    assert!(ia_chat::lienzo_propio(&n.notas_de(&chat)).is_some());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn editar_lo_dibujado_en_el_lienzo_del_chat() {
    let (n, d) = nucleo("editar");
    let chat = n.chat_asistente().unwrap().id;
    let motor = MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("dibujar_en_lienzo", CIRCULO),
        Respuesta::de_texto("Hecho."),
        Respuesta::de_llamada("transformar_elementos", r#"{"que":"todo","accion":"borrar"}"#),
        Respuesta::de_texto("Borrado."),
    ]);
    preguntar(&n, &chat, "dibuja un circulo", &motor, Arc::default(), Arc::default(), &CorteIa::nuevo()).unwrap();
    let fin = preguntar(&n, &chat, "borra el circulo del lienzo", &motor, Arc::default(), Arc::default(), &CorteIa::nuevo()).unwrap();
    // El modelo recibio el resumen del lienzo con lo que hay.
    let pedidas = motor.recibido.lock().unwrap();
    let ultimo = pedidas[2].mensajes.last().unwrap().texto_entero();
    assert!(ultimo.contains("#"), "el resumen con sus #id: {ultimo}");
    drop(pedidas);
    let usadas = &fin.respuesta.texto;
    assert_eq!(usadas, "Borrado.");
    let l = ia_chat::lienzo_propio(&n.notas_de(&chat)).unwrap();
    assert_eq!(elementos_vivos(&n, &chat, &n.notas_de(&chat)[l]), 0);
    let _ = std::fs::remove_dir_all(d);
}

fn elementos_vivos(n: &Nucleo, chat: &str, m: &Nota) -> usize {
    let ruta = almacen::lienzo(n.ruta(), chat, m.referencia.as_deref().unwrap());
    let l = pixpin_motor2d::excalidraw::leer(&std::fs::read_to_string(ruta).unwrap()).unwrap();
    pixpin_motor2d::excalidraw::a_escena(&l).cuantos_visibles()
}

#[test]
fn lo_que_borra_pregunta_en_la_burbuja_y_queda_escrito() {
    let (n, d) = nucleo("confirmar");
    n.apuntar_tarea("comprar pan".into(), 0).unwrap();
    let chat = n.chat_asistente().unwrap().id;
    for (si, fin_esperado) in [(true, "→ ✅ Sí"), (false, "→ ✖️ No")] {
        let motor = MotorGuionado::nuevo(vec![
            Respuesta::de_llamada("borrar_lista", r#"{"lista":"Inbox"}"#),
            Respuesta::de_texto("Listo."),
        ]);
        let manos = Arc::new(Manos { si, ..Default::default() });
        let ve = Arc::new(Ve::default());
        let fin = preguntar(&n, &chat, "borra la lista inbox entera", &motor, manos.clone(), ve.clone(), &CorteIa::nuevo()).unwrap();
        // La burbuja enseno la pregunta mientras esperaba.
        let vistos = ve.vistos.lock().unwrap();
        let con_pregunta = vistos.iter().find(|v| v.pregunta.is_some()).expect("la pregunta en la burbuja");
        assert!(con_pregunta.pregunta.as_deref().unwrap().contains("Inbox"));
        assert!(con_pregunta.texto.contains("esperando tu respuesta"), "{}", con_pregunta.texto);
        // Y queda en el historial, en el disco.
        assert!(fin.respuesta.texto.contains("❓") && fin.respuesta.texto.contains(fin_esperado), "{}", fin.respuesta.texto);
        assert_eq!(manos.pedidos.lock().unwrap().len(), usize::from(si));
    }
    let _ = std::fs::remove_dir_all(d);
}

const PLAN_FIESTA: &str = "ENTIENDO: Apuntar la fiesta de Lucia: sabado 18 a las 17:00 en el parque Kennedy.\nHARE: confirmarlo.\nHERRAMIENTAS: ninguna\nDATOS: sabado 18; 17:00; parque Kennedy";

#[test]
fn lo_pensado_se_guarda_con_la_respuesta_y_la_hoja_pasa_al_turno_siguiente() {
    let (n, d) = nucleo("pensado");
    let chat = n.chat_asistente().unwrap().id;
    let motor = MotorGuionado::nuevo(vec![
        Respuesta {
            pensamiento: "Hay que recordar la fecha, la hora y el sitio.".into(),
            ..Respuesta::de_texto(PLAN_FIESTA)
        },
        Respuesta::de_texto("Apuntado."),
        Respuesta::de_texto("ENTIENDO: Decir la hora de la fiesta de Lucia.\nHARE: mirar la hoja."),
        Respuesta::de_texto("A las 17:00."),
    ]);
    let m = n
        .escribir_nota(chat.clone(), "La fiesta de Lucia es el sabado 18 a las 17:00 en el parque Kennedy, tenlo en cuenta".into(), None)
        .unwrap();
    let inicio = n.ia_al_enviar(chat.clone(), m.id).unwrap().unwrap();
    let ve = Arc::new(Ve::default());
    let fin = turno_en_chat_con(&motor, &entorno(true), n.clone(), chat.clone(), inicio, 0, Arc::<Manos>::default(), ve.clone(), &CorteIa::nuevo(), Duration::ZERO).unwrap();
    assert_eq!(fin.respuesta.texto, "Apuntado.");
    // Swift lo vio pensar en vivo.
    assert!(ve.vistos.lock().unwrap().iter().any(|v| v.pensado.pensando));
    // En el disco, junto a la nota, con su codigo unico.
    let nota = n.notas_de(&chat).into_iter().find(|x| x.id == fin.respuesta.id).unwrap();
    let ruta = ruta_del_registro(n.ruta(), &chat, &nota.codigo_unico());
    assert!(ruta.to_string_lossy().contains(".asistente.json"), "{}", ruta.display());
    let reg = Registro::leer(&ruta).unwrap().expect("registro guardado");
    assert_eq!(reg.pensamiento, "Hay que recordar la fecha, la hora y el sitio.");
    assert!(reg.hoja.objetivo.contains("parque Kennedy"));
    // La seccion de la burbuja sale del registro (como al volver a abrir).
    let p = n.ia_pensado(chat.clone(), fin.respuesta.id.clone()).expect("pensado");
    assert!(p.segundos.is_some() && !p.pensando);
    assert!(p.entiendo.contains("Kennedy"));
    // El sincronizador lo reconoce como lo anotado de esa nota.
    let nombre = ruta.file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(pixpin_sincro::anotado::uid_del_nombre(&nombre), Some(nota.codigo_unico().as_str()));

    // Turno siguiente: la vuelta de planear lee la hoja.
    let m = n.escribir_nota(chat.clone(), "¿a que hora era?".into(), None).unwrap();
    let inicio = n.ia_al_enviar(chat.clone(), m.id.clone()).unwrap().unwrap();
    let fin = turno_en_chat_con(&motor, &entorno(true), n.clone(), chat.clone(), inicio, 0, Arc::<Manos>::default(), Arc::<Ve>::default(), &CorteIa::nuevo(), Duration::ZERO).unwrap();
    assert_eq!(fin.respuesta.texto, "A las 17:00.");
    let pedidas = motor.recibido.lock().unwrap();
    let planear = pedidas[2].mensajes[1].texto_entero();
    assert!(planear.contains("Objetivo: Apuntar la fiesta de Lucia"), "{planear}");
    // Una nota sin registro (la del usuario) no tiene seccion.
    assert!(n.ia_pensado(chat.clone(), m.id.clone()).is_none());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn una_hoja_rota_no_rompe_el_chat_y_se_empieza_otra() {
    let (n, d) = nucleo("hoja-rota");
    let chat = n.chat_asistente().unwrap().id;
    let motor = MotorGuionado::nuevo(vec![Respuesta::de_texto("Uno."), Respuesta::de_texto("Dos.")]);
    let fin = preguntar(&n, &chat, "hola", &motor, Arc::default(), Arc::default(), &CorteIa::nuevo()).unwrap();
    let nota = n.notas_de(&chat).into_iter().find(|x| x.id == fin.respuesta.id).unwrap();
    std::fs::write(ruta_del_registro(n.ruta(), &chat, &nota.codigo_unico()), "{roto").unwrap();
    assert!(hoja_del_chat(n.ruta(), &chat, &n.notas_de(&chat)).esta_vacia());
    let fin = preguntar(&n, &chat, "otra", &motor, Arc::default(), Arc::default(), &CorteIa::nuevo()).unwrap();
    assert_eq!(fin.respuesta.texto, "Dos.");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn la_lista_de_arroba_y_la_consulta() {
    let (n, d) = nucleo("arroba");
    let chat = n.chat_asistente().unwrap().id;
    n.crear_lienzo(chat.clone(), Some("Plano casa".into())).unwrap();
    n.crear_lienzo(chat.clone(), Some("Fachada".into())).unwrap();
    assert_eq!(ia_consulta_de_mencion("dibuja en @pla".into()).as_deref(), Some("pla"));
    assert_eq!(ia_consulta_de_mencion("correo@pla".into()), None);
    assert_eq!(ia_consulta_de_mencion("@pla casa".into()), None);
    let o = n.ia_opciones_mencion(chat.clone(), "pla".into());
    assert_eq!(o.len(), 1);
    assert_eq!(o[0].mencion, "@lienzo:Plano_casa");
    // En su chat no sale el asistente; en otro, si y el primero.
    assert!(n.ia_opciones_mencion(chat, String::new()).iter().all(|o| o.mencion != "@asistente"));
    let otro = n.crear_chat("obra".into()).unwrap().id;
    assert_eq!(n.ia_opciones_mencion(otro, String::new())[0].mencion, "@asistente");
    assert!(ia_menciona_al_asistente("oye @Asistente, ¿qué es?".into()));
    assert!(!ia_menciona_al_asistente("correo@asistente.com".into()));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn apple_intelligence_escribe_su_respuesta_y_su_dibujo() {
    let (n, d) = nucleo("apple");
    let chat = n.chat_asistente().unwrap().id;
    let m = n.escribir_nota(chat.clone(), "dibuja un circulo".into(), None).unwrap();
    let inicio = n.ia_al_enviar(chat.clone(), m.id).unwrap().unwrap();
    let doc = pixpin_ia::lienzo::documento(vec![serde_json::json!({
        "id": "a", "type": "ellipse", "x": 0.0, "y": 0.0, "width": 50.0, "height": 50.0,
        "strokeColor": "#1e1e1e", "version": 1, "isDeleted": false
    })]);
    let r = n.ia_responder(chat.clone(), inicio.respuesta.id, "Ahí lo tienes.".into(), Some(doc)).unwrap();
    assert_eq!(r.texto, "Ahí lo tienes.");
    let l = ia_chat::lienzo_propio(&n.notas_de(&chat)).unwrap();
    assert_eq!(r.lienzo_ia, n.notas_de(&chat)[l].referencia);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn medidas_reducidas_nunca_amplian() {
    assert_eq!(medidas_reducidas(4000, 3000, 768), (768, 576));
    assert_eq!(medidas_reducidas(300, 200, 768), (300, 200));
    assert_eq!(medidas_reducidas(100, 5000, 768), (15, 768));
}

#[test]
fn con_ojos_revisar_lleva_la_vista_y_ver_lienzo_la_pinta_swift() {
    let (n, d) = nucleo("ojos");
    let chat = n.chat_asistente().unwrap().id;
    let motor = MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("dibujar_en_lienzo", CIRCULO),
        Respuesta::de_texto("Hecho."),
        Respuesta::de_llamada("ver_lienzo", r#"{"zona":"todo"}"#),
        Respuesta::de_texto("Esta bien centrado."),
    ])
    .con_vision();
    preguntar(&n, &chat, "dibuja un circulo", &motor, Arc::default(), Arc::default(), &CorteIa::nuevo()).unwrap();
    let manos = Arc::new(Manos::default());
    let fin = preguntar(&n, &chat, "revisa el lienzo y corrige lo que falte", &motor, manos.clone(), Arc::default(), &CorteIa::nuevo()).unwrap();
    assert_eq!(fin.respuesta.texto, "Esta bien centrado.");
    // Al empezar, la vista va pegada al pedido; y `ver_lienzo` pinta otra.
    let vistas = manos.vistas.lock().unwrap();
    assert_eq!(vistas.len(), 2, "la del principio y la de ver_lienzo");
    assert!(vistas[0].rotulos.iter().any(|r| r.etiqueta && r.texto.starts_with('#')));
    let pedidas = motor.recibido.lock().unwrap();
    let pedido = pedidas[2].mensajes.iter().rev().find(|m| m.rol == pixpin_ia::motor::Rol::Usuario).unwrap();
    assert!(pedido.tiene_imagen(), "la vista del lienzo va con el pedido");
    assert!(pedido.texto_entero().contains("vista del lienzo"), "{}", pedido.texto_entero());
    drop(vistas);
    // Sin vision no se mira (caso negativo).
    let ciego = MotorGuionado::nuevo(vec![Respuesta::de_texto("Vale.")]);
    let manos2 = Arc::new(Manos::default());
    preguntar(&n, &chat, "revisa el lienzo", &ciego, manos2.clone(), Arc::default(), &CorteIa::nuevo()).unwrap();
    assert!(manos2.vistas.lock().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn una_conversacion_nueva_corta_lo_que_se_le_manda_al_modelo_pero_no_lo_borra() {
    let (n, d) = nucleo("conversacion-nueva");
    let chat = n.chat_asistente().unwrap().id;
    n.escribir_nota(chat.clone(), "lo de antes".into(), None).unwrap();
    let corte = n.ia_conversacion_nueva(chat.clone()).unwrap();
    assert_eq!(corte.texto, ia_chat::TEXTO_NUEVA_CONVERSACION);
    n.escribir_nota(chat.clone(), "lo de ahora".into(), None).unwrap();
    let notas = n.notas_de(&chat);
    assert_eq!(notas.len(), 3, "lo de antes sigue a la vista");
    assert!(ia_chat::es_corte(&notas[1]));
    let c = ia_chat::conversacion(&notas);
    assert_eq!(c, vec![(true, "lo de ahora".to_string())]);
    // Caso negativo: sin corte va todo.
    assert_eq!(ia_chat::conversacion(&notas[..1]).len(), 1);
    let _ = std::fs::remove_dir_all(&d);
}
