//! El bucle de los ojos en el agente, con un modelo guionado: `ver_lienzo`,
//! la revision que corrige, la que encuentra todo bien, sin vision y cuando
//! la vista o el modelo fallan.

use crate::agente::{Ejecutor, Evento, Final, Opciones, turno_con};
use crate::herramientas::{Llamada, catalogo};
use crate::motor::{Mensaje, MotorGuionado, Respuesta, Rol, Seguir};
use crate::ojos::{MARCA, Mirada, TODO_BIEN, Vista, Zona};

/// Un ejecutor que apunta lo que hace y contesta las vistas con una imagen
/// de mentira (o con un fallo).
#[derive(Default)]
struct ConOjos {
    hechas: Vec<Llamada>,
    miradas: Vec<Mirada>,
    cerrados: usize,
    ciego: bool,
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
            texto: format!("{MARCA} Zona [0, 0, 400, 300]"),
            ms: 3,
        })
    }
    fn cerrar_turno(&mut self) {
        self.cerrados += 1;
    }
}

const DIBUJA: &str = r#"{"elementos":[{"tipo":"texto","x":10,"y":10,"texto":"Hola"}]}"#;
const MUEVE: &str = r##"{"que":"#1","accion":"mover","dx":40}"##;

fn correr(guion: Vec<Respuesta>, vision: bool, revisar: usize, ej: &mut ConOjos) -> (Final, Vec<Evento>, Vec<Mensaje>, MotorGuionado) {
    let motor = MotorGuionado::nuevo(guion);
    let motor = if vision { motor.con_vision() } else { motor };
    let mut h = vec![Mensaje::sistema(crate::agente::instrucciones()), Mensaje::usuario("escribe Hola en el lienzo")];
    let mut ev = Vec::new();
    let o = Opciones { ojos: true, revisar, ..Default::default() };
    let f = turno_con(&motor, &mut h, &catalogo(), ej, &o, &mut |e| {
        ev.push(e);
        Seguir::Si
    })
    .unwrap();
    (f, ev, h, motor)
}

fn textos(ev: &[Evento]) -> String {
    ev.iter()
        .filter_map(|e| match e {
            Evento::Texto(t) => Some(t.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn la_revision_ve_lo_dibujado_y_corrige_en_el_mismo_turno() {
    let mut ej = ConOjos::default();
    let (f, ev, h, motor) = correr(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
            Respuesta::de_llamada("transformar_elementos", MUEVE),
            Respuesta::de_texto("Movi #1 para que no tape el borde."),
        ],
        true,
        1,
        &mut ej,
    );
    assert_eq!(ej.hechas.len(), 2, "dibujar y la correccion");
    assert_eq!(ej.miradas, vec![Mirada::de(Zona::Turno)]);
    assert_eq!(ej.cerrados, 1, "todo el turno, un Ctrl+Z");
    assert!(f.texto.starts_with("Escribi Hola."));
    assert!(f.texto.ends_with("Movi #1 para que no tape el borde."));
    // El usuario vio el primer texto antes que la correccion.
    let t = textos(&ev);
    assert!(t.starts_with("Escribi Hola.") && t.contains("Movi #1"), "{t}");
    // La tercera peticion lleva la imagen con el pedido de revision.
    let pedidas = motor.recibido.lock().unwrap();
    assert_eq!(pedidas.len(), 4);
    let vista = pedidas[2].mensajes.last().unwrap();
    assert_eq!(vista.rol, Rol::Usuario);
    assert!(vista.tiene_imagen());
    assert!(vista.texto_entero().contains("escribe Hola en el lienzo"), "lleva el pedido original");
    assert!(pedidas[0].herramientas.iter().any(|h| h.nombre == crate::ojos::NOMBRE));
    assert!(h.iter().any(crate::ojos::es_vista));
}

#[test]
fn si_esta_bien_no_dice_nada_ni_deja_la_imagen() {
    let mut ej = ConOjos::default();
    let (f, ev, h, motor) = correr(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
            Respuesta::de_texto(TODO_BIEN),
        ],
        true,
        2,
        &mut ej,
    );
    assert_eq!(f.texto, "Escribi Hola.");
    assert!(!textos(&ev).contains(TODO_BIEN));
    assert_eq!(ej.miradas.len(), 1, "sin cambios no hay segunda ronda");
    assert_eq!(motor.recibido.lock().unwrap().len(), 3);
    assert!(!h.iter().any(crate::ojos::es_vista), "la imagen no se queda en la conversacion");
    assert_eq!(ej.cerrados, 1);
}

#[test]
fn la_segunda_ronda_solo_si_la_primera_corrigio() {
    let mut ej = ConOjos::default();
    let (f, _, _, motor) = correr(
        vec![
            Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA),
            Respuesta::de_texto("Escribi Hola."),
            Respuesta::de_llamada("transformar_elementos", MUEVE),
            Respuesta::de_texto("Movi #1."),
            Respuesta::de_texto(TODO_BIEN),
        ],
        true,
        2,
        &mut ej,
    );
    assert_eq!(ej.miradas.len(), 2);
    assert_eq!(motor.recibido.lock().unwrap().len(), 5);
    assert_eq!(f.texto, "Escribi Hola.\n\nMovi #1.");
}

#[test]
fn sin_vision_ni_se_ofrece_ver_ni_se_revisa() {
    let mut ej = ConOjos::default();
    let (f, _, _, motor) = correr(
        vec![Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA), Respuesta::de_texto("Escribi Hola.")],
        false,
        2,
        &mut ej,
    );
    assert_eq!(f.texto, "Escribi Hola.");
    assert!(ej.miradas.is_empty());
    let pedidas = motor.recibido.lock().unwrap();
    assert_eq!(pedidas.len(), 2);
    assert!(!pedidas[0].herramientas.iter().any(|h| h.nombre == crate::ojos::NOMBRE));
}

#[test]
fn un_turno_que_no_dibuja_no_se_revisa() {
    let mut ej = ConOjos::default();
    let (f, _, _, motor) = correr(vec![Respuesta::de_texto("Hola, ¿que dibujo?")], true, 2, &mut ej);
    assert_eq!(f.texto, "Hola, ¿que dibujo?");
    assert!(ej.miradas.is_empty());
    assert_eq!(ej.cerrados, 0);
    assert_eq!(motor.recibido.lock().unwrap().len(), 1);
}

#[test]
fn si_no_se_puede_mirar_se_queda_lo_de_antes() {
    let mut ej = ConOjos { ciego: true, ..Default::default() };
    let (f, _, h, motor) = correr(
        vec![Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA), Respuesta::de_texto("Escribi Hola.")],
        true,
        1,
        &mut ej,
    );
    assert_eq!(f.texto, "Escribi Hola.");
    assert_eq!(ej.miradas.len(), 1);
    assert_eq!(motor.recibido.lock().unwrap().len(), 2, "sin vista no se pregunta");
    assert!(!h.iter().any(crate::ojos::es_vista));
}

#[test]
fn si_el_modelo_falla_al_revisar_se_queda_lo_de_antes() {
    let mut ej = ConOjos::default();
    // El guion se acaba justo en la revision: el motor da error.
    let (f, _, h, _) = correr(
        vec![Respuesta::de_llamada("dibujar_en_lienzo", DIBUJA), Respuesta::de_texto("Escribi Hola.")],
        true,
        1,
        &mut ej,
    );
    assert_eq!(f.texto, "Escribi Hola.");
    assert!(!f.cortado);
    assert!(!h.iter().any(crate::ojos::es_vista));
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
    let (f, ev, _, motor) = correr(vec![dos, Respuesta::de_texto("Listo y comprobado.")], true, 0, &mut ej);
    assert_eq!(f.texto, "Listo y comprobado.");
    assert_eq!(ej.miradas, vec![Mirada::de(Zona::Ultimo)]);
    assert!(ev.iter().any(|e| matches!(e, Evento::Resultado { nombre, ok: true, .. } if nombre == crate::ojos::NOMBRE)));
    let pedidas = motor.recibido.lock().unwrap();
    let m = &pedidas[1].mensajes;
    let n = m.len();
    assert_eq!(m[n - 3].rol, Rol::Herramienta);
    assert_eq!(m[n - 2].rol, Rol::Herramienta);
    assert!(m[n - 2].texto_entero().contains("imagen va en el mensaje siguiente"));
    assert!(m[n - 1].tiene_imagen() && crate::ojos::es_vista(&m[n - 1]));
    // La vista no es el pedido: los colores del pedido siguen saliendo de el.
    assert_eq!(crate::agente::pedido_del_usuario(m), "escribe Hola en el lienzo");
}

#[test]
fn una_zona_mal_dicha_se_reintenta() {
    let mut ej = ConOjos::default();
    let (f, _, _, motor) = correr(
        vec![Respuesta::de_llamada(crate::ojos::NOMBRE, r#"{"zona":"cielo"}"#), Respuesta::de_texto("Vale.")],
        true,
        0,
        &mut ej,
    );
    assert_eq!(f.reintentos, 1);
    assert!(ej.miradas.is_empty());
    assert!(motor.recibido.lock().unwrap()[1].mensajes.last().unwrap().texto_entero().starts_with("ERROR"));
}
