//! Pruebas del turno con ajustes: con un motor guionado (nunca la red) y
//! unas manos de la app falsas.

use std::sync::Mutex;

use pixpin_ia::herramientas;
use pixpin_ia::motor::{MotorGuionado, Respuesta};

use super::*;
use crate::asistente::pruebas::Apunta;

/// Las manos de la app, apuntadas.
#[derive(Default)]
struct Manos {
    si: bool,
    preguntas: Mutex<Vec<String>>,
    pedidos: Mutex<Vec<Value>>,
    leidos: Mutex<Vec<(String, Vec<RangoPaginas>)>>,
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
    fn leer_texto(&self, ruta: String, paginas: Vec<RangoPaginas>) -> Result<String, ErrorNucleo> {
        self.leidos.lock().unwrap().push((ruta, paginas));
        Ok("ACTA DE LA REUNION: se aprueba el presupuesto".into())
    }
    fn pintar_vista(&self, _vista: crate::asistente::ojos::VistaParaPintar) -> Option<Vec<u8>> {
        None
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

fn entorno(nube: bool, pensar: bool) -> Entorno {
    Entorno {
        nombre: "guionado".into(),
        nube,
        pensar,
        ventana: 8192,
        cada: Duration::ZERO,
        guardar_con: None,
    }
}

fn peticion(pedido: &str) -> PeticionIa {
    PeticionIa {
        historial: vec![
            MensajeIa {
                del_usuario: true,
                texto: "hola".into(),
            },
            MensajeIa {
                del_usuario: false,
                texto: "Hola, ¿en qué te ayudo?".into(),
            },
        ],
        pedido: pedido.into(),
        foto: None,
        adjuntos: Vec::new(),
        hoja: String::new(),
        desfase_ms: 0,
    }
}

fn carpeta() -> PathBuf {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("pixpin-asistente-{}-{n}-{}", std::process::id(), crate::ahora_ms()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn una_accion_de_la_app_sale_como_pedido_del_protocolo() {
    let motor = MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("anadir_tarea", r#"{"texto":"comprar pan"}"#),
        Respuesta::de_texto("Apuntado: comprar pan."),
    ]);
    let manos = Arc::new(Manos::default());
    let mut ej = EjecutorDelMac::nuevo(Vec::new(), 0).con_app(manos.clone());
    let oyente = Apunta::default();
    // «guarda en mis tareas» no es una orden simple: va al modelo.
    let r = turno_con(&motor, &entorno(false, false), peticion("guarda en mis tareas que tengo que comprar pan por favor"), &mut ej, &oyente, &CorteIa::nuevo()).unwrap();
    let pedidos = manos.pedidos.lock().unwrap();
    assert_eq!(pedidos.len(), 1, "{pedidos:?}");
    assert_eq!(pedidos[0]["accion"], "anadir_tarea");
    assert_eq!(pedidos[0]["pixpin"], 1);
    assert!(r.herramientas.iter().any(|h| h.nombre == "anadir_tarea" && h.ok), "{:?}", r.herramientas);
    assert_eq!(r.texto, "Apuntado: comprar pan.");
    assert_eq!(oyente.tandas.lock().unwrap().concat(), r.texto);
    assert_eq!(r.motor, "guionado");
    // La hoja de trabajo vuelve reescrita para el turno siguiente.
    let hoja = HojaDeTrabajo::de_json(&serde_json::from_str(&r.hoja).unwrap()).unwrap();
    assert!(!hoja.esta_vacia());
}

#[test]
fn lo_que_borra_pregunta_y_un_no_no_manda_nada() {
    let d = carpeta();
    let nucleo = Nucleo::abrir(d.display().to_string(), "Mac de pruebas".into()).unwrap();
    nucleo.apuntar_tarea("comprar pan".into(), 0).unwrap();
    let motor = MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("borrar_lista", r#"{"lista":"Inbox"}"#),
        Respuesta::de_texto("Vale, no la borro."),
    ]);
    let manos = Arc::new(Manos::default());
    let mut ej = EjecutorDelMac::nuevo(Vec::new(), 0).con_app(manos.clone()).con_nucleo(Some(nucleo.clone()));
    let r = turno_con(&motor, &entorno(true, false), peticion("borra la lista inbox entera"), &mut ej, &Apunta::default(), &CorteIa::nuevo()).unwrap();
    let preguntas = manos.preguntas.lock().unwrap();
    assert_eq!(preguntas.len(), 1);
    assert!(preguntas[0].contains("Inbox"), "{preguntas:?}");
    assert!(manos.pedidos.lock().unwrap().is_empty(), "con un no no sale nada");
    assert!(r.herramientas.iter().any(|h| h.nombre == "borrar_lista"));
    // Con un si, el pedido lleva el id de verdad de la lista.
    let motor = MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("borrar_lista", r#"{"lista":"Inbox"}"#),
        Respuesta::de_texto("Borrada."),
    ]);
    let manos = Arc::new(Manos { si: true, ..Default::default() });
    let mut ej = EjecutorDelMac::nuevo(Vec::new(), 0).con_app(manos.clone()).con_nucleo(Some(nucleo.clone()));
    turno_con(&motor, &entorno(true, false), peticion("borra la lista inbox entera"), &mut ej, &Apunta::default(), &CorteIa::nuevo()).unwrap();
    let pedidos = manos.pedidos.lock().unwrap();
    let listas = nucleo.tareas(0).unwrap();
    assert_eq!(pedidos.len(), 1);
    assert_eq!(pedidos[0]["accion"], "borrar_lista");
    assert_eq!(pedidos[0]["codigo"], listas[0].mensaje.as_str());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn leer_un_adjunto_pide_el_texto_a_la_app_por_su_nombre() {
    let motor = MotorGuionado::nuevo(vec![
        Respuesta::de_llamada(herramientas::OCR, r#"{"archivo":"acta","paginas":"1-2"}"#),
        Respuesta::de_texto("El acta aprueba el presupuesto."),
    ]);
    let manos = Arc::new(Manos::default());
    let adjuntos = vec![
        AdjuntoIa {
            nombre: "foto.png".into(),
            ruta: "/tmp/foto.png".into(),
        },
        AdjuntoIa {
            nombre: "Acta.pdf".into(),
            ruta: "/tmp/Acta.pdf".into(),
        },
    ];
    let mut p = peticion("¿qué dice el acta?");
    p.adjuntos = adjuntos.clone();
    let mut ej = EjecutorDelMac::nuevo(adjuntos, 0).con_app(manos.clone());
    let r = turno_con(&motor, &entorno(false, false), p, &mut ej, &Apunta::default(), &CorteIa::nuevo()).unwrap();
    let leidos = manos.leidos.lock().unwrap();
    assert_eq!(leidos.len(), 1, "{:?}", r.herramientas);
    assert_eq!(leidos[0].0, "/tmp/Acta.pdf");
    assert!(r.herramientas.iter().any(|h| h.ok && h.resumen.contains("presupuesto")), "{:?}", r.herramientas);
    // Sin adjuntos, se le dice al modelo que pida uno.
    let ej = EjecutorDelMac::nuevo(Vec::new(), 0);
    assert!(ej.elegir(None, false).unwrap_err().contains("adjunte"));
}

#[test]
fn lo_que_piensa_llega_aparte_y_se_guarda_con_la_respuesta() {
    let mut pensada = Respuesta::de_texto("ENTIENDO: dibujar una casa\nHARE: dibujar_en_lienzo");
    pensada.pensamiento = "Primero la pared, luego el tejado.".into();
    let motor = MotorGuionado::nuevo(vec![
        pensada,
        Respuesta::de_llamada(
            herramientas::DIBUJAR,
            r#"{"elementos":[{"tipo":"rectangulo","x":0,"y":40,"ancho":80,"alto":60,"color":"rojo"}]}"#,
        ),
        Respuesta::de_texto("Listo."),
    ]);
    let oyente = Apunta::default();
    let mut ej = EjecutorDelMac::nuevo(Vec::new(), 0);
    let r = turno_con(&motor, &entorno(false, true), peticion("dibuja una casa"), &mut ej, &oyente, &CorteIa::nuevo()).unwrap();
    assert!(r.pensamiento.contains("tejado"), "{:?}", r.pensamiento);
    assert_eq!(*oyente.pensado.lock().unwrap(), r.pensamiento);
    assert!(r.lienzo.is_some());
    assert!(!oyente.llamadas.lock().unwrap().is_empty() || !r.herramientas.is_empty());
}

#[test]
fn la_nube_ve_el_catalogo_sin_lo_que_el_mac_aun_no_hace() {
    let nube = herramientas_del_turno("hola", false, false, true);
    let nombres: Vec<&str> = nube.iter().map(|h| h.nombre.as_str()).collect();
    assert!(nombres.contains(&"anadir_tarea") && nombres.contains(&herramientas::DIBUJAR));
    for no in [pixpin_ia::hoja::NOMBRE, pixpin_ia::figuras::NOMBRE, pixpin_ia::anotacion::NOMBRE] {
        assert!(!nombres.contains(&no), "{no}");
    }
    assert!(pixpin_ia::edicion::herramientas().iter().all(|h| !nombres.contains(&h.nombre.as_str())));
    // El local, solo las pocas que vienen a cuento; con un adjunto, leerlo.
    let local = herramientas_del_turno("¿qué dice?", false, true, false);
    assert!(local.len() <= pixpin_ia::seleccion::MAXIMO + 1);
    assert!(local.iter().any(|h| h.nombre == herramientas::OCR));
    // «Cópiame esto» con una foto: solo calcar.
    let calcar = herramientas_del_turno("dibuja esta imagen", true, true, true);
    assert!(calcar.iter().all(|h| h.nombre != "anadir_tarea"), "{:?}", calcar.iter().map(|h| &h.nombre).collect::<Vec<_>>());
}

#[test]
fn sin_clave_la_nube_lo_dice_y_sin_modelo_el_local_tambien() {
    let d = carpeta();
    let mut a = AjustesIa {
        motor: "deepseek".into(),
        modelo: String::new(),
        pensar: None,
        respaldo_local: true,
        mejor_vista: false,
        documentos_a_la_nube: String::new(),
        carpeta_modelos: d.display().to_string(),
        puerto_usuario: 9,
    };
    // Sin Llavero instalado en las pruebas: no hay clave.
    match ia_que_motor(a.clone()) {
        MotorIa::Nube { proveedor, modelo, .. } => {
            assert_eq!(proveedor, "deepseek");
            assert_eq!(modelo, pixpin_ia::nube::deepseek::MODELO);
        }
        otro => panic!("{otro:?}"),
    }
    let e = motor_del_turno(&a, "hola", false).err().expect("sin clave no hay nube");
    assert!(e.to_string().contains("Falta la clave"), "{e}");
    a.motor = "local".into();
    assert_eq!(ia_que_motor(a.clone()), MotorIa::Ninguno);
    let e = motor_del_turno(&a, "hola", false).err().expect("sin modelo no hay local");
    assert!(e.to_string().contains("modelo local"), "{e}");
    // Caso negativo: un pedido vacio ni llega al motor.
    let r = ia_turno(a, peticion("  "), None, Arc::new(Manos::default()), Arc::new(Apunta::default()), CorteIa::nuevo());
    assert!(matches!(r, Err(ErrorNucleo::Argumento { .. })));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn el_contexto_lleno_se_reinicia_y_lo_viejo_se_busca() {
    let mut p = peticion("¿y el color que te dije al principio?");
    p.historial = (0..12)
        .map(|i| MensajeIa {
            del_usuario: i % 2 == 0,
            texto: format!("mensaje {i}: {}", "palabra ".repeat(180)),
        })
        .collect();
    let motor = MotorGuionado::nuevo(vec![Respuesta::de_texto("Era azul.")]);
    let mut ej = EjecutorDelMac::nuevo(Vec::new(), 0);
    let mut e = entorno(false, false);
    e.ventana = 2048;
    let r = turno_con(&motor, &e, p, &mut ej, &Apunta::default(), &CorteIa::nuevo()).unwrap();
    let hoja = HojaDeTrabajo::de_json(&serde_json::from_str(&r.hoja).unwrap()).unwrap();
    assert!(hoja.reinicios >= 1, "se apunto el reinicio");
    // Al motor no le llego la conversacion entera.
    let recibido = motor.recibido.lock().unwrap();
    assert!(recibido[0].mensajes.len() < 14, "{}", recibido[0].mensajes.len());
}

#[test]
fn las_fechas_de_las_tareas_se_leen() {
    assert_eq!(dia_de("1970-01-02"), Some(1));
    assert_eq!(dia_de("2026-02-30"), None);
    assert_eq!(dia_de("roto"), None);
    assert_eq!(recortar("abc", 5), "abc");
    assert!(recortar("abcdef", 3).starts_with("abc\n"));
}

#[test]
fn buscar_en_mis_cosas_encuentra_y_cita_y_la_nube_pide_permiso() {
    let d = carpeta();
    let nucleo = Nucleo::abrir(d.display().to_string(), "Mac de pruebas".into()).unwrap();
    let obra = nucleo.crear_chat("Obra Carretera Sur".into()).unwrap();
    nucleo
        .escribir_nota(obra.id.clone(), "El presupuesto de la cuneta es de 4500 soles, aprobado el lunes.".into(), None)
        .unwrap();
    let guion = || {
        MotorGuionado::nuevo(vec![
            Respuesta::de_llamada(pixpin_ia::documentos::BUSCAR, r#"{"consulta":"presupuesto cuneta"}"#),
            Respuesta::de_texto("Son 4500 soles [1]."),
        ])
    };
    // El local busca sin preguntar.
    let manos = Arc::new(Manos::default());
    let mut ej = EjecutorDelMac::nuevo(Vec::new(), 0).con_app(manos.clone()).con_nucleo(Some(nucleo.clone()));
    let r = turno_con(&guion(), &entorno(false, false), peticion("¿cuánto era el presupuesto de la cuneta?"), &mut ej, &Apunta::default(), &CorteIa::nuevo()).unwrap();
    assert!(manos.preguntas.lock().unwrap().is_empty());
    assert!(r.herramientas.iter().any(|h| h.ok && h.resumen.contains("4500")), "{:?}", r.herramientas);
    assert_eq!(r.citas.len(), 1, "{:?}", r.citas);
    let pedido: Value = serde_json::from_str(&r.citas[0].pedido).unwrap();
    assert_eq!(pedido["accion"], "abrir");
    assert_eq!(pedido["que"]["tipo"], "mensaje");
    // La nube pregunta; un no se recuerda y no busca.
    let mut ej = EjecutorDelMac::nuevo(Vec::new(), 0).con_app(manos.clone()).con_nucleo(Some(nucleo.clone())).en_la_nube(true);
    let r = turno_con(&guion(), &entorno(true, false), peticion("¿cuánto era el presupuesto de la cuneta?"), &mut ej, &Apunta::default(), &CorteIa::nuevo()).unwrap();
    assert_eq!(manos.preguntas.lock().unwrap().len(), 1);
    assert!(r.herramientas.iter().all(|h| !h.ok), "{:?}", r.herramientas);
    assert!(r.citas.is_empty());
    assert_eq!(nucleo.ajustes().unwrap().asistente_documentos_a_la_nube, "no");
    // Ya no se vuelve a preguntar.
    let mut ej = EjecutorDelMac::nuevo(Vec::new(), 0).con_app(manos.clone()).con_nucleo(Some(nucleo.clone())).en_la_nube(true);
    turno_con(&guion(), &entorno(true, false), peticion("¿cuánto era el presupuesto de la cuneta?"), &mut ej, &Apunta::default(), &CorteIa::nuevo()).unwrap();
    assert_eq!(manos.preguntas.lock().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn una_herramienta_pedida_desde_fuera_va_por_el_camino_del_asistente() {
    let d = carpeta();
    let nucleo = Nucleo::abrir(d.display().to_string(), "Mac de pruebas".into()).unwrap();
    let manos = Arc::new(Manos::default());
    let r = ia_hacer_herramienta(nucleo.clone(), "escribir_en_chat".into(), r#"{"texto":"hola desde Claude"}"#.into(), 0, manos.clone())
        .unwrap();
    assert!(!r.is_empty());
    let p = manos.pedidos.lock().unwrap();
    assert_eq!(p[0]["accion"], "chat");
    assert_eq!(p[0]["texto"], "hola desde Claude");
    drop(p);
    // Lo que borra pregunta; con un no, no se manda nada.
    nucleo.apuntar_tarea("comprar pan".into(), 0).unwrap();
    let antes = manos.pedidos.lock().unwrap().len();
    let _ = ia_hacer_herramienta(nucleo.clone(), "borrar_lista".into(), r#"{"lista":"Inbox"}"#.into(), 0, manos.clone());
    assert!(!manos.preguntas.lock().unwrap().is_empty());
    assert_eq!(manos.pedidos.lock().unwrap().len(), antes);
    // Casos negativos: una que no existe y unos argumentos que faltan.
    assert!(ia_hacer_herramienta(nucleo.clone(), "volar".into(), "{}".into(), 0, manos.clone()).is_err());
    assert!(ia_hacer_herramienta(nucleo, "escribir_en_chat".into(), "{}".into(), 0, manos).is_err());
}
