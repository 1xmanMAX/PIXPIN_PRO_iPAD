//! Las ventanitas de la voz (tanda E5) tal como las vera Swift: una carpeta
//! de datos de verdad en un temporal. Lo grabado en el telepronter, en
//! Pronunciar y en la conversacion entra en el chat como en Windows; una
//! nota de voz con hora es una llamada secreta, y «Volver a llamar en» le
//! cambia la hora.

use std::path::PathBuf;
use std::sync::Arc;

use pixpin_nucleo::{GrabacionVoz, Nucleo, SegmentoVoz, TurnoVoz};

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-nucleo-voz-ventanas-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temporal(dir)
    }

    fn nucleo(&self) -> Arc<Nucleo> {
        Nucleo::abrir(self.0.join("datos").display().to_string(), "Mac".into()).unwrap()
    }

    /// Un `.m4a` de mentira con ese nombre (el nucleo no lo decodifica).
    fn audio(&self, nombre: &str) -> String {
        let p = self.0.join(nombre);
        std::fs::write(&p, b"m4a").unwrap();
        p.display().to_string()
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn grabacion(ruta: String) -> GrabacionVoz {
    GrabacionVoz {
        ruta_origen: ruta,
        duracion_ms: 9_000,
        picos: vec![1, 2, 3],
        texto: None,
        turnos: vec![],
    }
}

#[test]
fn la_lectura_entra_con_su_nombre_y_su_texto_ya_puesto() {
    let t = Temporal::nueva("lectura");
    let n = t.nucleo();
    let chat = n.crear_chat("Obra".into()).unwrap().id;
    let mut g = grabacion(t.audio("lectura-1758351600000.m4a"));
    g.texto = Some("[0:00] primero\n\n[0:07] segundo".into());
    let m = n.adjuntar_grabacion(chat.clone(), g).unwrap();
    assert_eq!(m.clase, "VOZ");
    assert_eq!(m.nombre, "lectura-1758351600000.m4a");
    assert_eq!(m.duracion_ms, 9_000);
    assert_eq!(m.picos, vec![1, 2, 3]);
    assert_eq!(
        m.transcripcion.as_deref(),
        Some("[0:00] primero\n\n[0:07] segundo")
    );
    // Caso negativo: sin fichero no hay nota.
    assert!(
        n.adjuntar_grabacion(
            chat,
            grabacion(t.0.join("no-esta.m4a").display().to_string())
        )
        .is_err()
    );
}

#[test]
fn la_conversacion_se_pasa_a_texto_con_los_nombres_de_sus_turnos() {
    let t = Temporal::nueva("conversacion");
    let n = t.nucleo();
    let chat = n.crear_chat("Reunion".into()).unwrap().id;
    let mut g = grabacion(t.audio("conversacion-1.m4a"));
    g.turnos = vec![
        TurnoVoz::nuevo("Ana", 0, 4_000),
        TurnoVoz::nuevo("Luis", 4_000, 9_000),
    ];
    let m = n.adjuntar_grabacion(chat.clone(), g).unwrap();
    assert!(
        m.transcripcion.is_none(),
        "el texto lo pone la transcripcion"
    );
    let hecho = n
        .poner_transcripcion(
            chat,
            m.id,
            vec![
                SegmentoVoz {
                    desde_ms: 300,
                    texto: "buenos dias".into(),
                },
                SegmentoVoz {
                    desde_ms: 4_200,
                    texto: "hola Ana".into(),
                },
            ],
        )
        .unwrap();
    assert_eq!(
        hecho.transcripcion.as_deref(),
        Some("[0:00] **Ana:** buenos dias\n\n[0:04] **Luis:** hola Ana")
    );
}

#[test]
fn una_nota_de_voz_es_una_llamada_secreta_con_quien_llama_y_vuelve_mas_tarde() {
    let t = Temporal::nueva("llamada");
    let n = t.nucleo();
    let chat = n.crear_chat("Casa".into()).unwrap().id;
    let voz = n
        .adjuntar_grabacion(chat.clone(), grabacion(t.audio("recado.m4a")))
        .unwrap();
    let l = n
        .llamada_secreta(chat.clone(), voz.id.clone())
        .unwrap()
        .expect("una nota de voz con su audio llama");
    assert_eq!(l.nombre, "recado");
    assert!(std::path::Path::new(&l.audio).is_file());

    n.poner_quien_llama(voz.id.clone(), "Mama".into()).unwrap();
    assert_eq!(n.quien_llama(voz.id.clone()).as_deref(), Some("Mama"));
    let l = n
        .llamada_secreta(chat.clone(), voz.id.clone())
        .unwrap()
        .unwrap();
    assert_eq!(l.nombre, "Mama");

    // «Volver a llamar en 15 min»: la misma alarma a otra hora.
    assert!(
        n.volver_a_recordar(chat.clone(), voz.id.clone(), 1_000 + 15 * 60_000)
            .unwrap()
    );
    let puestos = n.recordatorios(0).unwrap();
    assert!(
        puestos
            .iter()
            .any(|r| r.id == voz.id && r.cuando_utc_ms == 1_000 + 15 * 60_000)
    );

    // Casos negativos: una nota escrita no llama, ni un mensaje que no esta.
    let nota = n
        .escribir_nota(chat.clone(), "a las diez".into(), None)
        .unwrap();
    assert!(n.llamada_secreta(chat.clone(), nota.id).unwrap().is_none());
    assert!(n.llamada_secreta(chat, "no-esta".into()).unwrap().is_none());
}
