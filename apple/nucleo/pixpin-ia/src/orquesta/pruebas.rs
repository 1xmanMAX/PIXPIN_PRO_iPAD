//! Pruebas del jefe y los trabajadores, con motores guionados.

use super::*;
use crate::agente::{Ejecutor, Opciones, turno_con};
use crate::herramientas::{Llamada, catalogo};
use crate::motor::{MotorGuionado, Respuesta};

struct Hace(usize);

impl Ejecutor for Hace {
    fn ejecutar(&mut self, _: &Llamada) -> Result<String, String> {
        self.0 += 1;
        Ok("hecho".into())
    }
}

const AYUDA: &str = r#"{"pregunta":"como se exporta"}"#;
const PLAN: &str = r#"Aqui va: ```json
{"entendido":"dos cosas","tareas":[
 {"titulo":"Buscar","instrucciones":"usa ayuda_app","hecho_si":"hay respuesta"},
 {"titulo":"Resumir","instrucciones":"usa ayuda_app otra vez","hecho_si":"hay resumen"}]}
```"#;

fn correr(
    jefe: Vec<Respuesta>,
    trabajador: Vec<Respuesta>,
    pedido: &str,
) -> (Final, Vec<Evento>, usize, MotorGuionado, MotorGuionado) {
    let jefe = MotorGuionado::nuevo(jefe).como_nube();
    let trabajador = MotorGuionado::nuevo(trabajador).como_nube();
    let mut h = vec![
        Mensaje::sistema(agente::instrucciones()),
        Mensaje::usuario(pedido),
    ];
    let mut ej = Hace(0);
    let mut eventos = Vec::new();
    let o = Opciones {
        jefe: Some(&jefe),
        ..Default::default()
    };
    let f = turno_con(&trabajador, &mut h, &catalogo(), &mut ej, &o, &mut |e| {
        eventos.push(e);
        Seguir::Si
    })
    .unwrap();
    (f, eventos, ej.0, jefe, trabajador)
}

/// El bucle entero: el jefe planea dos tareas, la segunda la devuelve una
/// vez para corregir, revisa el conjunto y contesta.
#[test]
fn el_jefe_planea_reparte_corrige_y_contesta() {
    let (f, eventos, hechas, jefe, trabajador) = correr(
        vec![
            Respuesta::de_texto(PLAN),
            Respuesta::de_texto(r#"{"veredicto":"bien","motivo":"ok"}"#),
            Respuesta::de_texto(
                r#"{"veredicto":"corregir","motivo":"falta","correccion":"añade los atajos"}"#,
            ),
            Respuesta::de_texto(r#"{"veredicto":"bien"}"#),
            Respuesta::de_texto(
                r#"{"faltan":[],"respuesta":"Busqué cómo se exporta y lo resumí."}"#,
            ),
        ],
        vec![
            Respuesta::de_llamada("ayuda_app", AYUDA),
            Respuesta::de_texto("Encontrado."),
            Respuesta::de_llamada("ayuda_app", AYUDA),
            Respuesta::de_texto("Resumido."),
            Respuesta::de_llamada("ayuda_app", r#"{"pregunta":"atajos"}"#),
            Respuesta::de_texto("Con los atajos."),
        ],
        "busca como se exporta, mira los ajustes y resume los atajos",
    );
    assert_eq!(f.texto, "Busqué cómo se exporta y lo resumí.");
    assert_eq!((hechas, f.herramientas_hechas), (3, 3));
    assert_eq!(jefe.recibido.lock().unwrap().len(), 5);
    assert_eq!(trabajador.recibido.lock().unwrap().len(), 6);
    // La correccion llega al trabajador con lo que pidio el jefe.
    let r = trabajador.recibido.lock().unwrap();
    assert!(
        r[4].mensajes
            .iter()
            .any(|m| m.texto_entero().contains("añade los atajos"))
    );
    // Lo que dicen los trabajadores no va a la burbuja; lo del jefe, si.
    let textos: String = eventos
        .iter()
        .filter_map(|e| {
            if let Evento::Texto(t) = e {
                Some(t.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(textos, "Busqué cómo se exporta y lo resumí.");
    // El plan se ve y acaba con las dos hechas.
    let ultimo = eventos
        .iter()
        .rev()
        .find_map(|e| {
            if let Evento::Tareas(t) = e {
                Some(t.clone())
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(ultimo.len(), 2);
    assert!(ultimo.iter().all(|t| t.estado == EstadoTarea::Hecha));
    assert!(eventos.iter().any(
        |e| matches!(e, Evento::Tareas(t) if t.iter().any(|x| x.estado == EstadoTarea::Corrigiendo))
    ));
}

/// Lo que falta al revisar el conjunto se hace en una ronda mas.
#[test]
fn lo_que_falta_al_final_se_hace_en_otra_ronda() {
    let plan = r#"{"entendido":"x","tareas":[{"titulo":"Una","instrucciones":"ayuda_app","hecho_si":"ok"}]}"#;
    let (f, _, hechas, jefe, _) = correr(
        vec![
            Respuesta::de_texto(plan),
            Respuesta::de_texto(r#"{"veredicto":"bien"}"#),
            Respuesta::de_texto(
                r#"{"faltan":[{"titulo":"Otra","instrucciones":"ayuda_app","hecho_si":"ok"}],"respuesta":""}"#,
            ),
            Respuesta::de_texto(r#"{"veredicto":"bien"}"#),
            Respuesta::de_texto(r#"{"faltan":[],"respuesta":"Listo, las dos."}"#),
        ],
        vec![
            Respuesta::de_llamada("ayuda_app", AYUDA),
            Respuesta::de_texto("Una."),
            Respuesta::de_llamada("ayuda_app", AYUDA),
            Respuesta::de_texto("Otra."),
        ],
        "busca como se exporta, mira los ajustes y resume los atajos",
    );
    assert_eq!(hechas, 2);
    assert_eq!(f.texto, "Listo, las dos.");
    assert_eq!(jefe.recibido.lock().unwrap().len(), 5);
}

/// Casos negativos: si el jefe no da un plan que se entienda, el turno de
/// siempre; y lo simple ni le pregunta.
#[test]
fn sin_plan_o_con_un_pedido_simple_sigue_el_turno_de_siempre() {
    let (f, _, hechas, jefe, _) = correr(
        vec![Respuesta::de_texto("no se")],
        vec![
            Respuesta::de_llamada("ayuda_app", AYUDA),
            Respuesta::de_texto("Hecho solo."),
        ],
        "busca como se exporta, mira los ajustes y resume los atajos",
    );
    assert_eq!((f.texto.as_str(), hechas), ("Hecho solo.", 1));
    assert_eq!(jefe.recibido.lock().unwrap().len(), 1);
    let (f, _, _, jefe, _) = correr(vec![], vec![Respuesta::de_texto("Hola.")], "hola");
    assert_eq!(f.texto, "Hola.");
    assert_eq!(
        jefe.recibido.lock().unwrap().len(),
        0,
        "lo simple no llama al jefe"
    );
}

#[test]
fn se_entiende_lo_que_contesta_el_jefe() {
    let p = leer_plan(PLAN).unwrap();
    assert_eq!(p.tareas.len(), 2);
    assert_eq!(p.tareas[1].titulo, "Resumir");
    assert!(leer_plan(r#"{"tareas":[]}"#).is_none());
    assert!(leer_plan("sin json").is_none());
    let muchas = format!(
        r#"{{"tareas":[{}]}}"#,
        vec![r#"{"titulo":"t","instrucciones":"i","hecho_si":"h"}"#; 20].join(",")
    );
    assert_eq!(leer_plan(&muchas).unwrap().tareas.len(), MAX_TAREAS);
    assert_eq!(
        leer_veredicto(r#"{"veredicto":"corregir","correccion":"x"}"#),
        Veredicto::Corregir("x".into())
    );
    assert_eq!(
        leer_veredicto(r#"{"veredicto":"no_se_puede","motivo":"m"}"#),
        Veredicto::NoSePuede("m".into())
    );
    assert_eq!(leer_veredicto("raro"), Veredicto::Bien);
    let c = leer_cierre(r#"{"faltan":[],"respuesta":"Hecho."}"#);
    assert!(c.faltan.is_empty() && c.respuesta == "Hecho.");
    assert!(conviene(
        "traduce la figura, pon los títulos en azul y alinéalos"
    ));
    assert!(!conviene("hola"));
}

/// 25-oct-2026 (el caso del usuario): la revision final escribia «todavia
/// no esta listo… ahora voy a limpiar el resto» y AHI acababa el turno. Lo
/// anunciado se hace en otra ronda.
#[test]
fn la_revision_que_anuncia_trabajo_lo_hace_en_otra_ronda() {
    let plan = r#"{"entendido":"x","tareas":[{"titulo":"Una","instrucciones":"ayuda_app","hecho_si":"ok"}]}"#;
    let (f, _, hechas, jefe, trabajador) = correr(
        vec![
            Respuesta::de_texto(plan),
            Respuesta::de_texto(r#"{"veredicto":"bien"}"#),
            Respuesta::de_texto(
                r#"{"faltan":[],"respuesta":"Todavía no está listo: quedó a medias y desordenado… ahora voy a limpiar el resto y hacerlo por completo."}"#,
            ),
            Respuesta::de_texto(r#"{"veredicto":"bien"}"#),
            Respuesta::de_texto(r#"{"faltan":[],"respuesta":"Listo: completo y alineado."}"#),
        ],
        vec![
            Respuesta::de_llamada("ayuda_app", AYUDA),
            Respuesta::de_texto("Una."),
            Respuesta::de_llamada("ayuda_app", AYUDA),
            Respuesta::de_texto("Limpio."),
        ],
        "busca como se exporta, mira los ajustes y resume los atajos",
    );
    assert_eq!(hechas, 2, "lo anunciado se hizo");
    assert_eq!(f.texto, "Listo: completo y alineado.");
    assert_eq!(jefe.recibido.lock().unwrap().len(), 5);
    // El trabajador de la ronda nueva recibio lo que se anuncio.
    let r = trabajador.recibido.lock().unwrap();
    assert!(
        r[2].mensajes
            .iter()
            .any(|m| m.texto_entero().contains("ahora voy a limpiar el resto"))
    );
}

/// Sin rondas que gastar, la respuesta no promete: dice que no quedo
/// terminado. Negativo: lo acabado queda como esta y no pide otra ronda.
#[test]
fn sin_rondas_la_respuesta_no_promete() {
    let anuncia = Cierre {
        faltan: vec![],
        respuesta: "Ahora voy a limpiar el resto.".into(),
    };
    assert!(sin_promesas(&anuncia).starts_with("No llegué a terminarlo"));
    assert_eq!(faltan_del_cierre(&anuncia).len(), 1);
    let con_faltan = Cierre {
        faltan: vec![Tarea {
            titulo: "Numerar las semanas".into(),
            instrucciones: String::new(),
            hecho_si: String::new(),
        }],
        respuesta: "Agrandado.".into(),
    };
    assert!(sin_promesas(&con_faltan).contains("No llegué a terminar: Numerar las semanas"));
    let listo = Cierre {
        faltan: vec![],
        respuesta: "Listo, las dos.".into(),
    };
    assert_eq!(sin_promesas(&listo), "Listo, las dos.");
    assert!(faltan_del_cierre(&listo).is_empty());
}
