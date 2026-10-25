//! El bucle de los ojos en el agente, con un modelo guionado: `ver_lienzo`,
//! la revision con rubrica (22-oct: critico aparte, rondas, parada, volver
//! a la mejor version, comprobaciones duras, respaldo, podar imagenes), sin
//! vision y cuando la vista o el modelo fallan.
//!
//! En el guion, la nota del critico es una respuesta mas del mismo motor
//! (sin critico de respaldo, juzga el del turno) y va justo detras de cada
//! captura: dibujar, contestar, NOTA, corregir, contestar, NOTA...

use std::collections::VecDeque;

use crate::agente::{Ejecutor, Evento, Final, Opciones, REVISIONES_MAXIMAS, turno_con};
use crate::herramientas::{Llamada, catalogo};
use crate::motor::{Mensaje, Motor, MotorGuionado, Respuesta, Rol, Seguir};
use crate::ojos::rubrica::MARCA_CORRECCION;
use crate::ojos::{MARCA, Mirada, TODO_BIEN, Vista, Zona};

/// Un ejecutor que apunta lo que hace y contesta las vistas con una imagen
/// de mentira (o con un fallo); guarda versiones y da las comprobaciones
/// duras que se le digan.
#[derive(Default)]
struct ConOjos {
    hechas: Vec<Llamada>,
    miradas: Vec<Mirada>,
    cerrados: usize,
    ciego: bool,
    /// Lo hecho cuando se guardo cada version.
    versiones: Vec<usize>,
    /// A que versiones se volvio.
    vueltas: Vec<usize>,
    /// Lo que contesta `comprobar` en cada captura (vacio: no hay lint).
    duros: VecDeque<Vec<String>>,
    sin_versiones: bool,
}

impl Ejecutor for ConOjos {
    fn ejecutar(&mut self, l: &Llamada) -> Result<String, String> {
        self.hechas.push(l.clone());
        Ok("hecho: 1 elementos anadidos al lienzo\n#1 texto negro [0,0,80,20] \"Hola\"".into())
    }
    fn ver(&mut self, m: &Mirada) -> Result<Vista, String> {
        self.miradas.push(m.clone());
        if self.ciego {
            return Err("la ventana del chat no contesto".into());
        }
        Ok(Vista {
            png: vec![0x89, b'P', b'N', b'G'],
            ancho: 400,
            alto: 300,
            texto: format!(
                "{MARCA} Zona [0, 0, 400, 300]\nElementos de la imagen:\n#1 texto negro [0,0,80,20] \"Hola\""
            ),
            ms: 3,
        })
    }
    fn cerrar_turno(&mut self) {
        self.cerrados += 1;
    }
    fn guardar_version(&mut self) -> Option<usize> {
        if self.sin_versiones {
            return None;
        }
        self.versiones.push(self.hechas.len());
        Some(self.versiones.len() - 1)
    }
    fn volver_a_version(&mut self, v: usize) -> Result<(), String> {
        self.vueltas.push(v);
        Ok(())
    }
    fn comprobar(&mut self) -> Option<Vec<String>> {
        self.duros.pop_front()
    }
}

const DIBUJA: &str = r#"{"elementos":[{"tipo":"texto","x":10,"y":10,"texto":"Hola"}]}"#;
const MUEVE: &str = r##"{"que":"#1","accion":"mover","dx":40}"##;

/// La nota del critico: los cinco ejes y, si no aprueba, un problema con #1.
fn nota(e: [u8; 5]) -> Respuesta {
    let problemas = if e.iter().all(|n| *n >= 8) {
        "[]"
    } else {
        r##"[{"ids":[1],"eje":"orden","que":"#1 toca el borde","arreglo":"mover #1 40 a la derecha"}]"##
    };
    Respuesta::de_texto(format!(
        r#"{{"notas":{{"completo":{},"texto":{},"orden":{},"coherencia":{},"legibilidad":{}}},"problemas":{problemas}}}"#,
        e[0], e[1], e[2], e[3], e[4]
    ))
}

fn correr_con(
    guion: Vec<Respuesta>,
    vision: bool,
    revisar: usize,
    critico: Option<&dyn Motor>,
    ej: &mut ConOjos,
) -> (Final, Vec<Evento>, Vec<Mensaje>, MotorGuionado) {
    let motor = MotorGuionado::nuevo(guion);
    let motor = if vision { motor.con_vision() } else { motor };
    let mut h = vec![
        Mensaje::sistema(crate::agente::instrucciones()),
        Mensaje::usuario("escribe Hola en el lienzo"),
    ];
    let mut ev = Vec::new();
    let o = Opciones {
        ojos: true,
        revisar,
        critico,
        ..Default::default()
    };
    let f = turno_con(&motor, &mut h, &catalogo(), ej, &o, &mut |e| {
        ev.push(e);
        Seguir::Si
    })
    .unwrap();
    (f, ev, h, motor)
}

fn correr(
    guion: Vec<Respuesta>,
    vision: bool,
    revisar: usize,
    ej: &mut ConOjos,
) -> (Final, Vec<Evento>, Vec<Mensaje>, MotorGuionado) {
    correr_con(guion, vision, revisar, None, ej)
}

fn textos(ev: &[Evento]) -> String {
    ev.iter()
        .filter_map(|e| match e {
            Evento::Texto(t) => Some(t.as_str()),
            _ => None,
        })
        .collect()
}

fn revisar_turno() -> Mirada {
    Mirada::para_revisar(Zona::Turno)
}

#[test]
fn la_revision_puntua_aparte_y_corrige_en_el_mismo_turno() {
    let mut ej = ConOjos::default();
    let (f, ev, h, motor) = correr(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
            nota([9, 9, 5, 9, 8]),
            Respuesta::de_llamada("transformar_elementos", MUEVE),
            Respuesta::de_texto("Movi #1 para que no tape el borde."),
            nota([9, 9, 9, 9, 9]),
        ],
        true,
        2,
        &mut ej,
    );
    assert_eq!(ej.hechas.len(), 2, "dibujar y la correccion");
    assert_eq!(
        ej.miradas,
        vec![revisar_turno(), revisar_turno()],
        "la captura de la nota y la de comprobar la correccion"
    );
    assert_eq!(ej.miradas[0].lado, crate::ojos::LADO_REVISION);
    assert_eq!(ej.cerrados, 1, "todo el turno, un Ctrl+Z");
    assert_eq!(ej.versiones, vec![1], "se guardo lo de antes de corregir");
    assert!(ej.vueltas.is_empty(), "la correccion mejoro: se queda");
    assert!(f.texto.starts_with("Escribi Hola."));
    assert!(f.texto.ends_with("Movi #1 para que no tape el borde."));
    let t = textos(&ev);
    assert!(
        t.starts_with("Escribi Hola.") && t.contains("Movi #1"),
        "{t}"
    );
    assert!(!t.contains("notas"), "la nota no va a la burbuja: {t}");
    let pedidas = motor.recibido.lock().unwrap();
    assert_eq!(pedidas.len(), 6);
    // El critico: una peticion aparte, sin herramientas ni la conversacion,
    // con la rubrica, la imagen, el pedido y la lista de elementos.
    let c = &pedidas[2];
    assert!(c.herramientas.is_empty());
    assert_eq!(c.mensajes.len(), 2);
    assert_eq!(c.mensajes[0].rol, Rol::Sistema);
    assert!(c.mensajes[0].texto_entero().contains("legibilidad"));
    assert!(c.mensajes[1].tiene_imagen());
    let pide = c.mensajes[1].texto_entero();
    assert!(
        pide.contains("escribe Hola en el lienzo") && pide.contains("#1 texto negro"),
        "{pide}"
    );
    assert!(
        !c.mensajes
            .iter()
            .any(|m| m.texto_entero().contains("Escribi Hola.")),
        "sin lo que penso el que dibujo"
    );
    // El que corrige recibe los problemas en texto, con sus #id, sin imagen.
    let corr = pedidas[3].mensajes.last().unwrap();
    assert_eq!(corr.rol, Rol::Usuario);
    assert!(!corr.tiene_imagen());
    let corr = corr.texto_entero();
    assert!(
        corr.starts_with(MARCA_CORRECCION) && corr.contains("#1") && corr.contains("orden 5"),
        "{corr}"
    );
    assert!(
        pedidas[0]
            .herramientas
            .iter()
            .any(|h| h.nombre == crate::ojos::NOMBRE)
    );
    // Ninguna imagen se queda en la conversacion; el pedido de corregir, si.
    assert!(!h.iter().any(Mensaje::tiene_imagen));
    assert!(
        h.iter()
            .any(|m| m.texto_entero().starts_with(MARCA_CORRECCION))
    );
    // El pedido sigue siendo el del usuario (los colores salen de el).
    assert_eq!(
        crate::agente::pedido_del_usuario(&h),
        "escribe Hola en el lienzo"
    );
}

#[test]
fn si_aprueba_a_la_primera_no_dice_nada_ni_deja_la_imagen() {
    for aprobado in [nota([9, 8, 8, 10, 9]), Respuesta::de_texto(TODO_BIEN)] {
        let mut ej = ConOjos::default();
        let (f, ev, h, motor) = correr(
            vec![
                Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
                Respuesta::de_texto("Escribi Hola."),
                aprobado,
            ],
            true,
            2,
            &mut ej,
        );
        assert_eq!(f.texto, "Escribi Hola.");
        assert!(!textos(&ev).contains(TODO_BIEN));
        assert_eq!(ej.miradas.len(), 1, "aprobado: no hay segunda ronda");
        assert!(ej.versiones.is_empty());
        assert_eq!(motor.recibido.lock().unwrap().len(), 3);
        assert!(
            !h.iter().any(crate::ojos::es_de_los_ojos),
            "ni imagen ni pedido de corregir"
        );
        assert_eq!(ej.cerrados, 1);
    }
}

#[test]
fn dos_rondas_mientras_mejora_y_para_al_aprobar() {
    let mut ej = ConOjos::default();
    let (f, _, _, motor) = correr(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
            nota([5, 5, 5, 5, 5]),
            Respuesta::de_llamada("transformar_elementos", MUEVE),
            Respuesta::de_texto("Movi #1."),
            nota([7, 7, 7, 7, 7]),
            Respuesta::de_llamada("transformar_elementos", MUEVE),
            Respuesta::de_texto("Lo alinee."),
            nota([9, 9, 9, 9, 9]),
        ],
        true,
        2,
        &mut ej,
    );
    assert_eq!(ej.miradas.len(), 3);
    assert_eq!(ej.versiones.len(), 2);
    assert!(ej.vueltas.is_empty());
    assert_eq!(motor.recibido.lock().unwrap().len(), 9);
    assert_eq!(f.texto, "Escribi Hola.\n\nMovi #1.\n\nLo alinee.");
}

#[test]
fn si_casi_no_mejora_para_y_se_queda_lo_corregido() {
    let mut ej = ConOjos::default();
    let (f, _, _, motor) = correr(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
            nota([6, 6, 6, 6, 6]),
            Respuesta::de_llamada("transformar_elementos", MUEVE),
            Respuesta::de_texto("Movi #1."),
            // +0,2 de media: menos de MEJORA_MINIMA.
            nota([7, 6, 6, 6, 6]),
        ],
        true,
        2,
        &mut ej,
    );
    assert_eq!(ej.miradas.len(), 2);
    assert!(ej.vueltas.is_empty(), "no es peor: se queda");
    assert_eq!(
        motor.recibido.lock().unwrap().len(),
        6,
        "no hay segunda correccion"
    );
    assert_eq!(f.texto, "Escribi Hola.\n\nMovi #1.");
}

#[test]
fn si_la_correccion_empeora_vuelve_a_la_mejor_version() {
    let mut ej = ConOjos::default();
    let (f, ev, _, motor) = correr(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
            nota([8, 8, 6, 8, 8]),
            Respuesta::de_llamada("transformar_elementos", MUEVE),
            Respuesta::de_texto("Movi #1."),
            nota([6, 6, 5, 6, 6]),
        ],
        true,
        2,
        &mut ej,
    );
    assert_eq!(
        ej.vueltas,
        vec![0],
        "vuelta a la version de antes de corregir"
    );
    assert_eq!(motor.recibido.lock().unwrap().len(), 6, "y no se sigue");
    assert!(
        !f.texto.contains("Movi #1."),
        "lo dicho de una correccion deshecha no se dice: {}",
        f.texto
    );
    assert!(f.texto.contains("versión que se veía mejor"), "{}", f.texto);
    assert!(!textos(&ev).contains("Movi #1."));
    assert_eq!(ej.cerrados, 1, "la vuelta va en el mismo Ctrl+Z del turno");
}

#[test]
fn una_ronda_que_rompe_una_comprobacion_dura_no_se_acepta() {
    let mut ej = ConOjos {
        duros: VecDeque::from(vec![
            vec!["#1 toca el borde".to_string()],
            vec![
                "#1 toca el borde".to_string(),
                "#1 se sale del lienzo".to_string(),
            ],
        ]),
        ..Default::default()
    };
    let (f, _, _, motor) = correr(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
            nota([9, 9, 9, 9, 9]),
            Respuesta::de_llamada("transformar_elementos", MUEVE),
            Respuesta::de_texto("Movi #1."),
            // El critico la puntua mejor, pero el lint dice que se sale.
            nota([10, 10, 10, 10, 10]),
        ],
        true,
        2,
        &mut ej,
    );
    // Aprobada por el critico pero con un fallo duro: se corrige igual, y
    // el fallo duro va al critico y al que corrige.
    let pedidas = motor.recibido.lock().unwrap();
    assert!(
        pedidas[2].mensajes[1]
            .texto_entero()
            .contains("- #1 toca el borde")
    );
    assert!(
        pedidas[3]
            .mensajes
            .last()
            .unwrap()
            .texto_entero()
            .contains("(comprobación) #1 toca el borde")
    );
    assert_eq!(
        ej.vueltas,
        vec![0],
        "un fallo duro nuevo: vuelta a la mejor"
    );
    assert!(!f.texto.contains("Movi #1."));
}

#[test]
fn sin_versiones_la_correccion_peor_se_queda_pero_para() {
    let mut ej = ConOjos {
        sin_versiones: true,
        ..Default::default()
    };
    let (f, _, _, motor) = correr(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
            nota([7, 7, 7, 7, 7]),
            Respuesta::de_llamada("transformar_elementos", MUEVE),
            Respuesta::de_texto("Movi #1."),
            nota([5, 5, 5, 5, 5]),
        ],
        true,
        2,
        &mut ej,
    );
    assert!(ej.vueltas.is_empty());
    assert_eq!(motor.recibido.lock().unwrap().len(), 6);
    assert_eq!(f.texto, "Escribi Hola.\n\nMovi #1.");
}

#[test]
fn las_rondas_no_pasan_del_tope() {
    let mut ej = ConOjos::default();
    let mut guion = vec![
        Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
        Respuesta::de_texto("Escribi Hola."),
    ];
    for n in [2, 4, 6] {
        guion.push(nota([n; 5]));
        guion.push(Respuesta::de_llamada("transformar_elementos", MUEVE));
        guion.push(Respuesta::de_texto("Movi #1."));
    }
    guion.push(nota([7; 5]));
    let (_, _, _, motor) = correr(guion, true, 99, &mut ej);
    assert_eq!(ej.miradas.len(), REVISIONES_MAXIMAS + 1);
    assert_eq!(ej.versiones.len(), REVISIONES_MAXIMAS);
    assert_eq!(
        motor.recibido.lock().unwrap().len(),
        2 + 3 * REVISIONES_MAXIMAS + 1,
        "el guion entero, ni una mas"
    );
}

#[test]
fn el_respaldo_puntua_si_la_nota_no_se_lee() {
    let respaldo = MotorGuionado::nuevo(vec![nota([9; 5])]).con_vision();
    let mut ej = ConOjos::default();
    let (f, _, _, motor) = correr_con(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
            Respuesta::de_texto("Se ve bastante bien, la verdad."),
        ],
        true,
        2,
        Some(&respaldo),
        &mut ej,
    );
    assert_eq!(f.texto, "Escribi Hola.");
    assert_eq!(motor.recibido.lock().unwrap().len(), 3);
    let r = respaldo.recibido.lock().unwrap();
    assert_eq!(r.len(), 1, "el respaldo puntuo esa captura");
    assert!(r[0].mensajes[1].tiene_imagen());
}

#[test]
fn el_respaldo_entra_tras_dos_suspensos_y_no_si_no_ve() {
    let respaldo = MotorGuionado::nuevo(vec![nota([9; 5])]).con_vision();
    let mut ej = ConOjos::default();
    let guion = vec![
        Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
        Respuesta::de_texto("Escribi Hola."),
        nota([4; 5]),
        Respuesta::de_llamada("transformar_elementos", MUEVE),
        Respuesta::de_texto("Movi #1."),
        nota([6; 5]),
        Respuesta::de_llamada("transformar_elementos", MUEVE),
        Respuesta::de_texto("Lo alinee."),
    ];
    let (f, _, _, motor) = correr_con(guion, true, 2, Some(&respaldo), &mut ej);
    assert_eq!(
        motor.recibido.lock().unwrap().len(),
        8,
        "la tercera nota no es del motor del turno"
    );
    assert_eq!(respaldo.recibido.lock().unwrap().len(), 1);
    assert_eq!(f.texto, "Escribi Hola.\n\nMovi #1.\n\nLo alinee.");
    // Caso negativo: un respaldo que no ve no se usa.
    let ciego = MotorGuionado::nuevo(vec![nota([9; 5])]);
    let mut ej = ConOjos::default();
    let (f, _, _, _) = correr_con(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
            Respuesta::de_texto("bonito"),
        ],
        true,
        2,
        Some(&ciego),
        &mut ej,
    );
    assert_eq!(f.texto, "Escribi Hola.");
    assert!(ciego.recibido.lock().unwrap().is_empty());
}

#[test]
fn la_revision_poda_las_imagenes_de_las_vistas_del_turno() {
    let mut ej = ConOjos::default();
    let mut dos = Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA);
    dos.llamadas.push(crate::motor::LlamadaCruda {
        id: "v1".into(),
        nombre: crate::ojos::NOMBRE.into(),
        argumentos: "{}".into(),
    });
    let (_, _, h, motor) = correr(
        vec![dos, Respuesta::de_texto("Escribi Hola."), nota([9; 5])],
        true,
        2,
        &mut ej,
    );
    // La vista que pidio el modelo iba con su imagen en la segunda vuelta...
    assert!(
        motor.recibido.lock().unwrap()[1]
            .mensajes
            .last()
            .unwrap()
            .tiene_imagen()
    );
    // ...y tras la revision solo queda su texto.
    let vistas: Vec<&Mensaje> = h.iter().filter(|m| crate::ojos::es_vista(m)).collect();
    assert_eq!(vistas.len(), 1);
    assert!(!vistas[0].tiene_imagen());
    assert!(vistas[0].texto_entero().contains("Zona [0, 0, 400, 300]"));
}

#[test]
fn sin_vision_ni_se_ofrece_ver_ni_se_revisa() {
    let mut ej = ConOjos::default();
    let (f, _, _, motor) = correr(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
        ],
        false,
        2,
        &mut ej,
    );
    assert_eq!(f.texto, "Escribi Hola.");
    assert!(ej.miradas.is_empty());
    let pedidas = motor.recibido.lock().unwrap();
    assert_eq!(pedidas.len(), 2);
    assert!(
        !pedidas[0]
            .herramientas
            .iter()
            .any(|h| h.nombre == crate::ojos::NOMBRE)
    );
}

#[test]
fn un_turno_que_no_dibuja_no_se_revisa() {
    let mut ej = ConOjos::default();
    let (f, _, _, motor) = correr(
        vec![Respuesta::de_texto("Hola, ¿que dibujo?")],
        true,
        2,
        &mut ej,
    );
    assert_eq!(f.texto, "Hola, ¿que dibujo?");
    assert!(ej.miradas.is_empty());
    assert_eq!(ej.cerrados, 0);
    assert_eq!(motor.recibido.lock().unwrap().len(), 1);
}

#[test]
fn si_no_se_puede_mirar_se_queda_lo_de_antes() {
    let mut ej = ConOjos {
        ciego: true,
        ..Default::default()
    };
    let (f, _, h, motor) = correr(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
        ],
        true,
        1,
        &mut ej,
    );
    assert_eq!(f.texto, "Escribi Hola.");
    assert_eq!(ej.miradas.len(), 1);
    assert_eq!(
        motor.recibido.lock().unwrap().len(),
        2,
        "sin vista no se pregunta"
    );
    assert!(!h.iter().any(crate::ojos::es_vista));
}

#[test]
fn si_el_critico_falla_se_queda_lo_de_antes() {
    let mut ej = ConOjos::default();
    // El guion se acaba justo en la nota: el motor da error.
    let (f, _, h, _) = correr(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
        ],
        true,
        1,
        &mut ej,
    );
    assert_eq!(f.texto, "Escribi Hola.");
    assert!(!f.cortado);
    assert!(!h.iter().any(crate::ojos::es_de_los_ojos));
    assert_eq!(ej.cerrados, 1);
}

#[test]
fn si_el_modelo_falla_al_corregir_se_queda_lo_de_antes() {
    let mut ej = ConOjos::default();
    // El guion se acaba en la correccion.
    let (f, _, h, _) = correr(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
            nota([5; 5]),
        ],
        true,
        1,
        &mut ej,
    );
    assert_eq!(f.texto, "Escribi Hola.");
    assert!(!f.cortado);
    assert!(
        !h.iter().any(crate::ojos::es_de_los_ojos),
        "sin correccion no se queda el pedido de corregir"
    );
    assert_eq!(ej.cerrados, 1);
}

#[test]
fn ver_lienzo_pinta_despues_de_dibujar_y_la_imagen_va_detras_de_los_resultados() {
    let mut ej = ConOjos::default();
    let mut dos = Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA);
    dos.llamadas.push(crate::motor::LlamadaCruda {
        id: "v1".into(),
        nombre: crate::ojos::NOMBRE.into(),
        argumentos: r#"{"zona":"ultimo"}"#.into(),
    });
    let (f, ev, _, motor) = correr(
        vec![dos, Respuesta::de_texto("Listo y comprobado.")],
        true,
        0,
        &mut ej,
    );
    assert_eq!(f.texto, "Listo y comprobado.");
    assert_eq!(ej.miradas, vec![Mirada::de(Zona::Ultimo)]);
    assert!(ev.iter().any(
        |e| matches!(e, Evento::Resultado { nombre, ok: true, .. } if nombre == crate::ojos::NOMBRE)
    ));
    let pedidas = motor.recibido.lock().unwrap();
    let m = &pedidas[1].mensajes;
    let n = m.len();
    assert_eq!(m[n - 3].rol, Rol::Herramienta);
    assert_eq!(m[n - 2].rol, Rol::Herramienta);
    assert!(
        m[n - 2]
            .texto_entero()
            .contains("imagen va en el mensaje siguiente")
    );
    assert!(m[n - 1].tiene_imagen() && crate::ojos::es_vista(&m[n - 1]));
    // La vista no es el pedido: los colores del pedido siguen saliendo de el.
    assert_eq!(
        crate::agente::pedido_del_usuario(m),
        "escribe Hola en el lienzo"
    );
}

#[test]
fn una_zona_mal_dicha_se_reintenta() {
    let mut ej = ConOjos::default();
    let (f, _, _, motor) = correr(
        vec![
            Respuesta::de_llamada(crate::ojos::NOMBRE, r#"{"zona":"cielo"}"#),
            Respuesta::de_texto("Vale."),
        ],
        true,
        0,
        &mut ej,
    );
    assert_eq!(f.reintentos, 1);
    assert!(ej.miradas.is_empty());
    assert!(
        motor.recibido.lock().unwrap()[1]
            .mensajes
            .last()
            .unwrap()
            .texto_entero()
            .starts_with("ERROR")
    );
}
