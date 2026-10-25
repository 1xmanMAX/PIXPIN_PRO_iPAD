//! Pruebas de las automatizaciones (las de Windows,
//! `automatizaciones/pruebas.rs`, mas las del Mac): el formato, las recetas
//! rotas, los bucles, los disparadores con un reloj de mentira, los pasos
//! encadenados con un hacedor de mentira, el fichero que viaja y una vuelta
//! entera por el `Nucleo` con unas manos de mentira.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pixpin_ia::acciones::AccionApp;
use pixpin_proyecto::{almacen, cuaderno};
use serde_json::{Value, json};

use super::*;

fn raiz_de_prueba(nombre: &str) -> PathBuf {
    let raiz = std::env::temp_dir().join(format!("pixpin-auto-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    let obra = almacen::Ficha::nueva("Obra Miraflores", 1, "PC01");
    almacen::Indice {
        proyectos: vec![obra],
        ..Default::default()
    }
    .guardar(&raiz)
    .unwrap();
    raiz
}

const EJEMPLO: &str = r#"{"version":1,"deMas":"se guarda","automatizaciones":[
  {"id":"c1","nombre":"Capturas a la obra","activa":true,"aparato":"PC01",
   "disparador":{"tipo":"al_capturar"},
   "pasos":[{"tipo":"accion","herramienta":"escribir_en_chat","args":{"texto":"Captura {hora}"},"proyecto":"Obra","imagenes":["{captura}"]},
            {"tipo":"asistente","pedido":"Hazle OCR"}],
   "loDelMovil":42},
  {"id":"h1","nombre":"Resumen","activa":true,"aparato":"PC01",
   "disparador":{"tipo":"a_la_hora","hora":"08:00"},
   "pasos":[{"tipo":"asistente","pedido":"Resume mis tareas pendientes"}]},
  {"id":"s1","nombre":"Sincro","activa":false,"aparato":"PC01",
   "disparador":{"tipo":"al_sincronizar"},
   "pasos":[{"tipo":"accion","herramienta":"anadir_tarea","args":{"texto":"Mirar {sincro}"}}]},
  {"id":"m1","nombre":"A mano","aparato":"TEL9",
   "disparador":{"tipo":"a_mano"},
   "pasos":[{"tipo":"asistente","pedido":"Hola"}]},
  {"id":"x1","nombre":"Futura","disparador":{"tipo":"al_llegar_correo"},"pasos":[]}
]}"#;

#[test]
fn el_formato_va_y_vuelve_sin_perder_lo_que_no_entiende() {
    let l = Lista::de_json(EJEMPLO).unwrap();
    assert_eq!(l.entradas.len(), 5);
    assert_eq!(l.buenas().count(), 4);
    let c1 = l.buscar("c1").unwrap();
    assert_eq!(c1.disparador, Disparador::AlCapturar);
    assert_eq!(c1.resto.get("loDelMovil"), Some(&json!(42)));
    assert!(
        matches!(&l.entradas[4], Entrada::Rota { motivo, .. } if motivo == "«al_llegar_correo» es de una versión más nueva")
    );
    // Ida y vuelta: lo de mas y la rota siguen ahi, tal cual.
    let otra = Lista::de_json(&l.a_json()).unwrap();
    assert_eq!(otra, l);
    let v: Value = serde_json::from_str(&l.a_json()).unwrap();
    assert_eq!(v["deMas"], "se guarda");
    assert_eq!(v["version"], 1);
    assert_eq!(v["automatizaciones"][4]["disparador"]["tipo"], "al_llegar_correo");
    assert_eq!(v["automatizaciones"][0]["pasos"][0]["tipo"], "accion");
    // Caso negativo: lo que no es JSON no se pisa.
    assert!(Lista::de_json("no es json").is_err());
    assert!(Lista::de_json(r#"{"automatizaciones":3}"#).is_err());
}

#[test]
fn una_receta_rota_o_con_un_paso_desconocido_no_entra() {
    let crear = |r: Value| de_receta(&r, "a".into(), "PC01", 1);
    let bien = crear(json!({"nombre":"Resumen","disparador":{"tipo":"a_la_hora","hora":"8:00"},
                            "pasos":"[\"Resume mis tareas pendientes\"]"}))
    .unwrap();
    assert_eq!(bien.pasos.len(), 1);
    assert_eq!(bien.aparato.as_deref(), Some("PC01"));
    // Un texto suelto es un pedido al asistente.
    let suelto = crear(json!({"nombre":"x","disparador":"a_mano","pasos":"dime la hora"})).unwrap();
    assert!(matches!(&suelto.pasos[0], Paso::Asistente { pedido, .. } if pedido == "dime la hora"));
    // Objetos con y sin `tipo`.
    let mixto = crear(json!({"nombre":"x","disparador":{"tipo":"al_capturar"},"pasos":[
        {"herramienta":"escribir_en_chat","args":{"texto":"{captura}"},"proyecto":"Obra"},
        {"asistente":"hazle OCR","imagenes":["{captura}"]},
        {"tipo":"accion","herramienta":"anadir_tarea","args":{"texto":"{resultado}"}}]}))
    .unwrap();
    assert_eq!(mixto.pasos.len(), 3);

    let mal = |r: Value| crear(r).unwrap_err();
    assert!(
        mal(json!({"nombre":"x","disparador":"a_mano","pasos":[{"herramienta":"volar","args":{}}]}))
            .contains("no hay ninguna acción «volar»")
    );
    assert!(mal(json!({"nombre":"x","disparador":"a_mano","pasos":[{"herramienta":"anadir_tarea"}]})).contains("texto"));
    assert!(mal(json!({"nombre":"x","disparador":{"tipo":"a_la_hora","hora":"25:00"},"pasos":"hola"})).contains("hora"));
    assert!(mal(json!({"nombre":"x","disparador":"cada_luna","pasos":"hola"})).contains("disparador"));
    assert!(mal(json!({"nombre":"x","disparador":"a_mano","pasos":[]})).contains("no tiene pasos"));
    assert!(mal(json!({"nombre":"","disparador":"a_mano","pasos":"hola"})).contains("nombre"));
    assert!(mal(json!({"nombre":"x","disparador":"a_mano","pasos":[3]})).contains("paso 1"));
    assert!(
        mal(json!({"nombre":"x","disparador":"a_mano","pasos":[{"herramienta":"capturar_pantalla","imagenes":["{captura}"]}]}))
            .contains("no lleva imágenes")
    );
    let muchos: Vec<Value> = (0..11).map(|i| json!(format!("paso {i}"))).collect();
    assert!(mal(json!({"nombre":"x","disparador":"a_mano","pasos":muchos})).contains("como mucho"));
    // Lo mismo antes de mandar el pedido (para que el modelo lo arregle).
    let pedido = json!({"pixpin":1,"accion":"automatizacion","operacion":"crear",
                        "receta":{"nombre":"x","disparador":{"tipo":"a_mano"},"pasos":"[{\"herramienta\":\"volar\"}]"}});
    assert!(revisar_pedido(&pedido).unwrap_err().contains("volar"));
    assert!(revisar_pedido(&json!({"pixpin":1,"accion":"capturar"})).is_ok());
}

#[test]
fn un_bucle_de_disparadores_se_rechaza_al_crearla_y_se_bloquea_al_correr() {
    let crear = |d: &str, h: &str| {
        de_receta(
            &json!({"nombre":"x","disparador":{"tipo":d},"pasos":[{"herramienta":h}]}),
            "a".into(),
            "PC01",
            1,
        )
    };
    assert!(crear("al_capturar", "capturar_pantalla").unwrap_err().contains("bucle"));
    assert!(crear("al_sincronizar", "sincronizar_movil").unwrap_err().contains("bucle"));
    // Capturar a mano o sincronizar cada dia si vale.
    assert!(crear("a_mano", "capturar_pantalla").is_ok());
    assert!(crear("a_mano", "sincronizar_movil").is_ok());
    // Ninguna crea automatizaciones (una que crea otra que crea otra…).
    let anidada = de_receta(
        &json!({"nombre":"x","disparador":"a_mano","pasos":[{"herramienta":"crear_automatizacion",
            "args":{"nombre":"y","disparador":"a_mano","pasos":"hola"}}]}),
        "a".into(),
        "PC01",
        1,
    );
    assert!(anidada.unwrap_err().contains("no puede crear"));
    // Dentro de un paso del asistente (lo que mira `HacerPedidos`).
    assert!(automatizacion_bloqueada(Some("al_capturar".into()), "capturar".into()).unwrap().contains("bucle"));
    assert!(automatizacion_bloqueada(Some("a_mano".into()), "automatizacion".into()).is_some());
    assert!(automatizacion_bloqueada(Some("al_capturar".into()), "sincronizar".into()).is_none());
    // Caso negativo: sin automatizacion corriendo, todo pasa.
    assert!(automatizacion_bloqueada(None, "automatizacion".into()).is_none());
}

/// 13-oct-2026 (un martes), a las `h:m` hora local, en ms.
fn a_las(dia: i64, h: i64, m: i64) -> i64 {
    // 2026-10-13 son 20 739 dias desde 1970.
    (20_739 + dia) * 86_400_000 + (h * 60 + m) * 60_000
}

#[test]
fn a_la_hora_con_un_reloj_de_mentira() {
    assert_eq!(dia_de_la_semana(a_las(0, 12, 0)), 2, "el 13-oct-2026 es martes");
    assert_eq!(Vars::default().con_hora(a_las(0, 8, 5)).fecha, "2026-10-13");
    assert_eq!(Vars::default().con_hora(a_las(0, 8, 5)).hora, "08:05");
    let ocho = Disparador::ALaHora {
        hora: "08:00".into(),
        dias: vec![],
    };
    assert!(!toca_a_la_hora(&ocho, a_las(0, 7, 59), None));
    assert!(toca_a_la_hora(&ocho, a_las(0, 8, 0), None));
    // Ya corrio hoy: no otra vez; manana si.
    assert!(!toca_a_la_hora(&ocho, a_las(0, 8, 30), Some(a_las(0, 8, 0))));
    assert!(toca_a_la_hora(&ocho, a_las(1, 8, 0), Some(a_las(0, 8, 0))));
    // Con el Mac apagado a las 8: a las 9 se recupera; a las 15, ya no.
    assert!(toca_a_la_hora(&ocho, a_las(0, 9, 0), Some(a_las(-1, 8, 0))));
    assert!(!toca_a_la_hora(&ocho, a_las(0, 15, 0), Some(a_las(-1, 8, 0))));
    // Solo los lunes: el martes no.
    let lunes = Disparador::ALaHora {
        hora: "08:00".into(),
        dias: vec![1],
    };
    assert!(!toca_a_la_hora(&lunes, a_las(0, 8, 0), None));
    assert!(toca_a_la_hora(&lunes, a_las(6, 8, 0), None));
    // Caso negativo: otro disparador u hora rota, nunca.
    assert!(!toca_a_la_hora(&Disparador::AMano, a_las(0, 8, 0), None));
    let rota = Disparador::ALaHora {
        hora: "ocho".into(),
        dias: vec![],
    };
    assert!(!toca_a_la_hora(&rota, a_las(0, 8, 0), None));
    assert_eq!(minuto_del_dia("8:05"), Some(485));
    assert_eq!(minuto_del_dia("24:00"), None);
}

#[test]
fn cada_disparo_elige_las_suyas() {
    let l = Lista::de_json(EJEMPLO).unwrap();
    let ids = |d: &Disparo, ahora: i64, ultimas: &BTreeMap<String, i64>| -> Vec<String> {
        elegir(&l, d, "PC01", ahora, ultimas).into_iter().map(|(a, _)| a.id).collect()
    };
    let nada = Default::default();
    let captura = Disparo::Captura(PathBuf::from("/c/captura-1.png"));
    assert_eq!(ids(&captura, a_las(0, 12, 0), &nada), ["c1"]);
    let (_, vars) = elegir(&l, &captura, "PC01", a_las(0, 12, 0), &nada).remove(0);
    assert_eq!(vars.captura.as_deref(), Some(Path::new("/c/captura-1.png")));
    assert_eq!(vars.hora, "12:00");
    // La de la sincro esta apagada.
    assert!(ids(&Disparo::Sincro("Al día".into()), a_las(0, 12, 0), &nada).is_empty());
    assert_eq!(ids(&Disparo::Reloj, a_las(0, 8, 1), &nada), ["h1"]);
    assert!(ids(&Disparo::Reloj, a_las(0, 7, 0), &nada).is_empty());
    let hecha = [("h1".to_string(), a_las(0, 8, 1))].into_iter().collect();
    assert!(ids(&Disparo::Reloj, a_las(0, 8, 2), &hecha).is_empty());
    // A mano: la pedida, aunque este apagada o sea de otro aparato.
    assert_eq!(ids(&Disparo::AMano("s1".into()), 0, &nada), ["s1"]);
    assert_eq!(ids(&Disparo::AMano("m1".into()), 0, &nada), ["m1"]);
    assert!(ids(&Disparo::AMano("no-esta".into()), 0, &nada).is_empty());
    // Caso negativo: desde otro aparato, la de captura de PC01 no corre.
    assert!(elegir(&l, &captura, "TEL9", 0, &nada).is_empty());
}

#[test]
fn la_guardia_corta_un_bucle_que_no_se_vio() {
    let mut g = Guardia::default();
    assert!(g.puede("a", 0).is_ok());
    assert!(g.puede("a", 1_000).is_err(), "dos veces en un segundo");
    assert!(g.puede("b", 1_000).is_ok(), "otra si puede");
    let mut t = 10_000;
    for _ in 1..POR_HORA {
        assert!(g.puede("a", t).is_ok());
        t += 10_000;
    }
    assert!(g.puede("a", t).unwrap_err().contains("veces"));
    // Pasada la hora, vuelve a poder.
    assert!(g.puede("a", 3_700_000 + t).is_ok());
}

/// Un hacedor de mentira: apunta lo que se le pide.
#[derive(Default)]
struct Espia {
    acciones: Vec<Value>,
    preguntas: Vec<(String, Option<String>, Vec<PathBuf>, Tipo)>,
    respuesta: Option<Result<String, String>>,
}

impl Hacedor for Espia {
    fn accion(&mut self, a: &AccionApp) -> Result<String, String> {
        self.acciones.push(a.pedido.clone());
        Ok(format!("hecho: {}", a.herramienta))
    }
    fn asistente(&mut self, pedido: &str, proyecto: Option<&str>, imagenes: &[PathBuf], bloquear: Tipo) -> Result<String, String> {
        self.preguntas
            .push((pedido.to_string(), proyecto.map(str::to_string), imagenes.to_vec(), bloquear));
        self.respuesta.clone().unwrap_or_else(|| Ok("Texto leído: CIMIENTOS".into()))
    }
    fn proyecto(&self, nombre: &str) -> Option<String> {
        (acciones::normalizar(nombre.trim()) == "obra").then(|| "pr-obra".into())
    }
}

#[test]
fn los_pasos_se_encadenan_y_el_que_falla_para_los_demas() {
    let a = de_receta(
        &json!({"nombre":"Capturas a la obra","disparador":"al_capturar","pasos":[
            {"herramienta":"escribir_en_chat","args":{"texto":"Captura de las {hora}"},"proyecto":"Obra","imagenes":["{captura}"]},
            {"asistente":"Hazle OCR"},
            {"herramienta":"escribir_en_chat","args":{"texto":"{resultado}"},"proyecto":"Obra"}]}),
        "a".into(),
        "PC01",
        1,
    )
    .unwrap();
    let vars = Vars {
        captura: Some(PathBuf::from("/c/captura-7.png")),
        ..Vars::default().con_hora(a_las(0, 9, 30))
    };
    let mut h = Espia::default();
    let inf = ejecutar(&a, vars.clone(), &mut h);
    assert!(inf.bien(), "{inf:?}");
    assert_eq!(h.acciones.len(), 2);
    assert_eq!(h.acciones[0]["accion"], "chat");
    assert_eq!(h.acciones[0]["texto"], "Captura de las 09:30");
    assert_eq!(h.acciones[0]["proyecto"], "pr-obra");
    assert_eq!(h.acciones[0]["imagenes"], json!(["/c/captura-7.png"]));
    // Al capturar, la captura va al asistente aunque el paso no la nombre.
    assert_eq!(h.preguntas[0].0, "Hazle OCR");
    assert_eq!(h.preguntas[0].2, [PathBuf::from("/c/captura-7.png")]);
    assert_eq!(h.preguntas[0].3, Tipo::AlCapturar);
    // Lo que contesto el asistente es el texto del paso siguiente.
    assert_eq!(h.acciones[1]["texto"], "Texto leído: CIMIENTOS");
    assert_eq!(inf.resumen(3), "hecho: escribir_en_chat");

    // Caso negativo: si el asistente falla, el tercer paso no se hace.
    let mut h = Espia {
        respuesta: Some(Err("sin modelo".into())),
        ..Default::default()
    };
    let inf = ejecutar(&a, vars.clone(), &mut h);
    assert!(!inf.bien());
    assert_eq!(inf.pasos.len(), 2);
    assert_eq!(h.acciones.len(), 1);
    assert!(inf.resumen(3).contains("Paso 2 de 3: sin modelo"), "{}", inf.resumen(3));
    // Y un chat que no existe para en ese paso.
    let mut otra = a.clone();
    if let Paso::Accion { proyecto, .. } = &mut otra.pasos[0] {
        *proyecto = Some("Casa".into());
    }
    let mut h = Espia::default();
    let inf = ejecutar(&otra, vars, &mut h);
    assert_eq!(inf.pasos.len(), 1);
    assert!(inf.pasos[0].as_ref().unwrap_err().contains("«Casa»"));
    assert!(h.acciones.is_empty());
}

#[test]
fn guardar_crea_el_fichero_y_un_solo_mensaje_que_lo_hace_viajar() {
    let raiz = raiz_de_prueba("guardar");
    let a = de_receta(&json!({"nombre":"Resumen","disparador":"a_mano","pasos":"hola"}), "auto-1".into(), "PC01", 5).unwrap();
    cambiar(&raiz, "PC01", 5, |l| l.anadir(a.clone())).unwrap();
    // Repetir el id no entra.
    assert!(cambiar(&raiz, "PC01", 6, |l| l.anadir(a.clone())).unwrap_err().contains("ya hay"));
    let b = de_receta(&json!({"nombre":"Otra","disparador":"a_mano","pasos":"adios"}), "auto-2".into(), "PC01", 7).unwrap();
    cambiar(&raiz, "PC01", 7, |l| l.anadir(b)).unwrap();
    let (asistente, carpeta) = chat_del_asistente(&raiz, "PC01", 8).unwrap();
    assert!(fichero_en(&carpeta).ends_with("android/guardados/asistente/automatizaciones.json"));
    let l = leer_si_hay(&raiz).unwrap();
    assert_eq!(l.buenas().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["auto-1", "auto-2"]);
    let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
    let senal: Vec<_> = c.mensajes.iter().filter(|m| m.id == ID_MENSAJE).collect();
    assert_eq!(senal.len(), 1, "un solo mensaje aunque se guarde dos veces");
    assert_eq!(senal[0].ruta.as_deref(), Some(RUTA_PORTATIL));
    assert_eq!(senal[0].clase, Some(cuaderno::Clase::Archivo));
    assert!(cuaderno::es_de_sistema(senal[0]), "no sale como burbuja");
    // La ruta portatil cae en el fichero de verdad (asi viaja).
    let real = pixpin_proyecto::vista::ruta_real(&raiz, &asistente, RUTA_PORTATIL).unwrap();
    assert_eq!(real, fichero_en(&carpeta));
    // Apagar y borrar.
    cambiar(&raiz, "PC01", 9, |l| Ok(l.activar("auto-1", false, 9))).unwrap();
    assert!(!leer_si_hay(&raiz).unwrap().buscar("auto-1").unwrap().activa);
    assert!(cambiar(&raiz, "PC01", 10, |l| Ok(l.borrar("auto-1"))).unwrap());
    assert!(!cambiar(&raiz, "PC01", 11, |l| Ok(l.borrar("auto-1"))).unwrap());
    assert_eq!(leer_si_hay(&raiz).unwrap().buenas().count(), 1);
    // Caso negativo: un fichero roto no se pisa.
    std::fs::write(fichero_en(&carpeta), "{roto").unwrap();
    assert!(cambiar(&raiz, "PC01", 12, |l| Ok(l.borrar("auto-2"))).is_err());
    assert_eq!(std::fs::read_to_string(fichero_en(&carpeta)).unwrap(), "{roto");
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn poner_cambia_en_su_sitio_sin_perder_lo_del_movil() {
    let mut l = Lista::de_json(EJEMPLO).unwrap();
    let mut c1 = l.buscar("c1").unwrap().clone();
    c1.nombre = "Capturas a la obra (cambiada)".into();
    c1.resto.clear();
    c1.aparato = None;
    l.poner(c1, 99).unwrap();
    let c1 = l.buscar("c1").unwrap();
    assert_eq!(c1.nombre, "Capturas a la obra (cambiada)");
    assert_eq!(c1.resto.get("loDelMovil"), Some(&json!(42)), "lo del movil sigue");
    assert_eq!(c1.aparato.as_deref(), Some("PC01"), "el aparato no lo cambia el editor");
    assert_eq!(c1.tocada, 99);
    assert!(matches!(&l.entradas[0], Entrada::Buena(a) if a.id == "c1"), "en su sitio");
    // Caso negativo: una que no vale no entra y la lista no cambia.
    let antes = l.clone();
    let mut mala = l.buscar("h1").unwrap().clone();
    mala.pasos.clear();
    assert!(l.poner(mala, 100).unwrap_err().contains("no tiene pasos"));
    assert_eq!(l, antes);
}

#[test]
fn la_ficha_del_editor_va_y_vuelve() {
    let l = Lista::de_json(EJEMPLO).unwrap();
    let ultimas = [("h1".to_string(), 7)].into_iter().collect();
    let f = fichas(&l, "PC01", &ultimas);
    assert_eq!(f.len(), 5);
    assert_eq!(f[0].disparador, "al_capturar");
    assert_eq!(f[0].pasos[0].args[0].valor, "Captura {hora}");
    assert_eq!(f[0].detalle, "Al capturar · escribir_en_chat en «Obra» → «Hazle OCR»");
    assert_eq!(f[1].ultima_vez, Some(7));
    assert_eq!(f[3].otro_aparato.as_deref(), Some("TEL9"));
    assert!(f[4].rota.as_deref().unwrap().contains("al_llegar_correo"));
    // De vuelta: igual que la del fichero (sin lo que no toca el editor).
    let c1 = de_ficha(&f[0], "c1".into()).unwrap();
    let original = l.buscar("c1").unwrap();
    assert_eq!(c1.pasos, original.pasos);
    assert_eq!(c1.disparador, original.disparador);
    // Una lista escrita como tal es una lista; un texto, texto.
    assert_eq!(de_texto(r#"["pan","leche"]"#), json!(["pan", "leche"]));
    assert_eq!(de_texto("[pan"), json!("[pan"));
    // Caso negativo: un disparador que no existe.
    let mut rara = f[0].clone();
    rara.disparador = "cada_luna".into();
    assert!(de_ficha(&rara, "z".into()).unwrap_err().contains("cada_luna"));
}

#[test]
fn las_herramientas_del_editor_no_crean_automatizaciones() {
    let h = automatizacion_herramientas();
    assert!(h.iter().any(|x| x.nombre == "escribir_en_chat" && x.con_proyecto && x.con_imagenes));
    assert!(h.iter().any(|x| x.nombre == "capturar_pantalla" && !x.con_imagenes));
    assert!(!h.iter().any(|x| x.nombre == "crear_automatizacion"));
}

/// Lo que se le pidio al asistente: pedido, chat, imagenes y bloqueo.
type Pregunta = (String, Option<String>, Vec<String>, String);

/// Unas manos de mentira: apuntan lo que se les pide.
#[derive(Default)]
struct Manos {
    pedidos: Mutex<Vec<Value>>,
    asistente: Mutex<Vec<Pregunta>>,
    empezadas: Mutex<Vec<String>>,
    acabadas: Mutex<Vec<InformeAutomatizacion>>,
}

impl ManosAutomatizacion for Manos {
    fn empieza(&self, id: String, _nombre: String) {
        self.empezadas.lock().unwrap().push(id);
    }
    fn confirmar(&self, _pregunta: String) -> bool {
        false
    }
    fn pedido(&self, json: String) -> Option<String> {
        self.pedidos.lock().unwrap().push(serde_json::from_str(&json).unwrap());
        None
    }
    fn asistente(&self, pedido: String, proyecto: Option<String>, imagenes: Vec<String>, bloquear: String) -> RespuestaPaso {
        self.asistente.lock().unwrap().push((pedido, proyecto, imagenes, bloquear));
        RespuestaPaso {
            bien: true,
            texto: "3 tareas pendientes".into(),
        }
    }
    fn acabo(&self, informe: InformeAutomatizacion) {
        self.acabadas.lock().unwrap().push(informe);
    }
}

#[test]
fn una_vuelta_entera_por_el_nucleo() {
    let raiz = raiz_de_prueba("nucleo");
    let n = Nucleo::abrir(raiz.display().to_string(), "Mac".into()).unwrap();
    // Sin chat «Asistente»: nada, y no se crea solo por mirar.
    assert!(n.automatizaciones().unwrap().is_empty());
    assert!(!almacen::Indice::leer(&raiz).proyectos.iter().any(|f| f.es_asistente()));
    let manos = Arc::new(Manos::default());
    let m: Arc<dyn ManosAutomatizacion> = manos.clone();
    assert!(n.automatizaciones_disparar(DisparoAutomatizacion::Reloj, 0, m.clone()).unwrap().is_empty());

    // Una del editor: pregunta al asistente y apunta lo que dijo en la obra.
    let ficha = FichaAutomatizacion {
        id: String::new(),
        nombre: "Pendientes a la obra".into(),
        activa: true,
        disparador: "a_mano".into(),
        hora: String::new(),
        dias: Vec::new(),
        pasos: vec![
            PasoAutomatizacion {
                es_asistente: true,
                herramienta: String::new(),
                args: Vec::new(),
                pedido: "Resume mis tareas del {fecha}".into(),
                proyecto: None,
                imagenes: Vec::new(),
            },
            PasoAutomatizacion {
                es_asistente: false,
                herramienta: "escribir_en_chat".into(),
                args: vec![ArgAutomatizacion {
                    nombre: "texto".into(),
                    valor: "{nombre}: {resultado}".into(),
                }],
                pedido: String::new(),
                proyecto: Some("Obra".into()),
                imagenes: Vec::new(),
            },
        ],
        detalle: String::new(),
        otro_aparato: None,
        rota: None,
        ultima_vez: None,
    };
    assert_eq!(n.automatizacion_revisar(ficha.clone()), None);
    let id = n.automatizacion_guardar(ficha.clone()).unwrap();
    let lista = n.automatizaciones().unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].id, id);
    assert_eq!(lista[0].otro_aparato, None, "nace con este Mac como su aparato");

    let informes = n
        .automatizaciones_disparar(DisparoAutomatizacion::AMano { id: id.clone() }, 0, m.clone())
        .unwrap();
    assert_eq!(informes.len(), 1);
    assert!(informes[0].bien, "{:?}", informes[0]);
    assert_eq!(*manos.empezadas.lock().unwrap(), std::slice::from_ref(&id));
    assert_eq!(manos.acabadas.lock().unwrap().len(), 1);
    let pregunta = manos.asistente.lock().unwrap()[0].clone();
    assert!(pregunta.0.starts_with("Resume mis tareas del 20"), "{}", pregunta.0);
    assert_eq!(pregunta.3, "a_mano");
    let p = manos.pedidos.lock().unwrap()[0].clone();
    assert_eq!(p["accion"], "chat");
    assert_eq!(p["texto"], "Pendientes a la obra: 3 tareas pendientes");
    let obra = almacen::Indice::leer(&raiz)
        .proyectos
        .into_iter()
        .find(|f| f.nombre == "Obra Miraflores")
        .unwrap()
        .id;
    assert_eq!(p["proyecto"], json!(obra));
    // Queda en el historial y en lo que ya corrio.
    let h = n.automatizaciones_historial();
    assert_eq!(h[0].id, id);
    assert!(h[0].bien);
    assert_eq!(h[0].disparo, "a_mano");
    assert!(n.automatizaciones().unwrap()[0].ultima_vez.is_some());
    // La guardia: otra vez en el acto, se salta.
    let otra = n
        .automatizaciones_disparar(DisparoAutomatizacion::AMano { id: id.clone() }, 0, m.clone())
        .unwrap();
    assert!(otra[0].saltada.is_some());
    assert_eq!(manos.empezadas.lock().unwrap().len(), 1);

    // Del pedido `automatizacion`: crear, apagar y borrar.
    let nombre = n
        .automatizacion_de_receta(r#"{"nombre":"Resumen","disparador":{"tipo":"a_la_hora","hora":"08:00"},"pasos":"hola"}"#.into())
        .unwrap();
    assert_eq!(nombre, "Resumen");
    let resumen = n.automatizaciones().unwrap().into_iter().find(|f| f.nombre == "Resumen").unwrap();
    assert_eq!(n.automatizacion_activar(resumen.id.clone(), false).unwrap().as_deref(), Some("Resumen"));
    assert!(!n.automatizaciones().unwrap().iter().find(|f| f.id == resumen.id).unwrap().activa);
    assert_eq!(n.automatizacion_borrar(resumen.id.clone()).unwrap().as_deref(), Some("Resumen"));
    assert_eq!(n.automatizacion_borrar(resumen.id).unwrap(), None);
    // Casos negativos: una receta rota no entra; editar sin pasos tampoco.
    assert!(n.automatizacion_de_receta(r#"{"nombre":"x","disparador":"a_mano","pasos":[]}"#.into()).is_err());
    let mut sin_pasos = ficha;
    sin_pasos.id = id;
    sin_pasos.pasos.clear();
    assert!(n.automatizacion_revisar(sin_pasos.clone()).unwrap().contains("no tiene pasos"));
    assert!(n.automatizacion_guardar(sin_pasos).is_err());
    assert_eq!(n.automatizaciones().unwrap()[0].pasos.len(), 2, "no se piso");
    let _ = std::fs::remove_dir_all(&raiz);
}
