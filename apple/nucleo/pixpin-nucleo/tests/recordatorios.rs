//! Los recordatorios (tanda A4) tal como los vera Swift: carpetas de datos
//! de verdad en un temporal y los cuadernos escritos como los escriben
//! Windows y el movil. Son las pruebas de `recordatorios.rs` y de las
//! tareas con hora de Windows que no dependen del hilo del vigia (ese, en
//! el Mac, es de Swift: `RecordatoriosTests.swift`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use pixpin_nucleo::{Nucleo, TareaRecordada};
use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{self, Mensaje, Sello};
use pixpin_proyecto::mini;

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-nucleo-rec-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Temporal(dir)
    }

    fn nucleo(&self) -> Arc<Nucleo> {
        Nucleo::abrir(self.datos().display().to_string(), "Mac".into()).unwrap()
    }

    fn datos(&self) -> PathBuf {
        self.0.join("datos")
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cuaderno_de(datos: &Path, chat: &str) -> String {
    std::fs::read_to_string(almacen::carpeta(datos, chat).join("guardados.jsonl")).unwrap()
}

/// Una nota con (o sin) `recuerdaEn`, escrita como la escribe el movil.
fn nota(datos: &Path, chat: &str, id: &str, texto: &str, recuerda: Option<i64>) {
    let mut m = Mensaje {
        id: id.into(),
        texto: texto.into(),
        cuando: 1,
        ..Default::default()
    };
    if let Some(t) = recuerda {
        m.resto.insert("recuerdaEn".into(), t.into());
    }
    cuaderno::anadir(&almacen::carpeta(datos, chat), &m).unwrap();
}

/// Una lista de tareas escrita como la escribe Windows.
fn lista(datos: &Path, chat: &str, id: i64, tareas: &[(&str, bool)]) -> String {
    let tareas: Vec<mini::Tarea> = tareas
        .iter()
        .map(|(t, h)| mini::Tarea {
            texto: (*t).into(),
            hecha: *h,
        })
        .collect();
    let m = Mensaje::miniapp(
        mini::TAREAS,
        "Obra",
        &mini::escribir_tareas("Obra", &tareas),
        &Sello {
            cuando: id,
            numero: 1,
            aparato: "PC01".into(),
            proyecto: chat.into(),
        },
    );
    cuaderno::anadir(&almacen::carpeta(datos, chat), &m).unwrap();
    m.id
}

/// Una hora local (ms) de un dia y una hora.
fn local(anio: i32, mes: u8, dia: u8, h: i64, mi: i64) -> i64 {
    (mini::Fecha { anio, mes, dia }.dias() * 1440 + h * 60 + mi) * 60_000
}

#[test]
fn la_agenda_junta_las_conversaciones_ordenada_y_sin_los_que_no_tienen_hora() {
    // `la_agenda_junta_las_conversaciones_y_se_salta_las_que_no_tienen_cuaderno`
    // y `la_agenda_sale_del_cuaderno_ordenada...` de Windows.
    let tmp = Temporal::nueva("agenda");
    let n = tmp.nucleo();
    let obra = n.crear_chat("Obra".into()).unwrap();
    let casa = n.crear_chat("Casa".into()).unwrap();
    // Un tercero apuntado en el indice pero sin carpeta: no tumba la lista.
    n.crear_chat("Fantasma".into()).unwrap();
    nota(&tmp.datos(), &obra.id, "m1", "llamar al arquitecto", Some(9_000));
    nota(&tmp.datos(), &obra.id, "m2", "sin hora", None);
    nota(&tmp.datos(), &casa.id, "m3", "sacar la basura", Some(1_000));

    let v = n.recordatorios(0).unwrap();
    let ids: Vec<&str> = v.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["m3", "m1"], "del mas proximo al mas lejano, mezclados");
    assert_eq!(v[0].chat, casa.id);
    assert_eq!(v[0].nombre_chat, "Casa");
    assert_eq!(v[1].texto, "llamar al arquitecto", "el texto sale del resumen");
    assert!(v.iter().all(|r| r.tarea.is_none()));
}

#[test]
fn un_recordatorio_puesto_sobrevive_a_cerrar_y_abrir_y_al_sonar_se_olvida() {
    let tmp = Temporal::nueva("sobrevive");
    let n = tmp.nucleo();
    let chat = n.crear_chat("Obra".into()).unwrap().id;
    nota(&tmp.datos(), &chat, "m1", "llamar al arquitecto", None);
    nota(&tmp.datos(), &chat, "m2", "otra cosa", None);

    assert!(n.volver_a_recordar(chat.clone(), "m1".into(), 1_758_351_600_000).unwrap());
    // «Cerrar y abrir»: otro Nucleo, nada en memoria.
    let otro = tmp.nucleo();
    let v = otro.recordatorios(0).unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].id, "m1");
    assert_eq!(v[0].cuando_utc_ms, 1_758_351_600_000);
    assert!(cuaderno_de(&tmp.datos(), &chat).contains("\"recuerdaEn\":1758351600000"));

    // Al saltar se le quita la hora, o volveria a sonar al arrancar.
    assert!(otro.olvidar_recordatorio(chat.clone(), "m1".into()).unwrap());
    assert!(otro.recordatorios(0).unwrap().is_empty());
    assert!(!cuaderno_de(&tmp.datos(), &chat).contains("recuerdaEn"), "sin nulo colgando");
    // Caso negativo: olvidar dos veces dice que no.
    assert!(!otro.olvidar_recordatorio(chat.clone(), "m1".into()).unwrap());
}

#[test]
fn guardar_la_hora_no_se_lleva_por_delante_lo_que_escribio_el_movil() {
    let tmp = Temporal::nueva("conserva");
    let n = tmp.nucleo();
    let chat = n.crear_chat("Obra".into()).unwrap().id;
    let carpeta = almacen::carpeta(&tmp.datos(), &chat);
    std::fs::create_dir_all(&carpeta).unwrap();
    std::fs::write(
        carpeta.join("guardados.jsonl"),
        "{\"id\":\"m1\",\"texto\":\"hola\",\"turnos\":[{\"quien\":\"Ana\"}]}\n",
    )
    .unwrap();
    assert!(n.volver_a_recordar(chat.clone(), "m1".into(), 4_242).unwrap());
    let escrito = cuaderno_de(&tmp.datos(), &chat);
    assert!(escrito.contains("\"turnos\""), "se perdio un campo ajeno: {escrito}");
    assert!(escrito.contains("\"recuerdaEn\":4242"));
}

#[test]
fn ponerle_hora_a_un_mensaje_que_no_existe_dice_que_no_en_vez_de_inventarlo() {
    let tmp = Temporal::nueva("nohay");
    let n = tmp.nucleo();
    let chat = n.crear_chat("Obra".into()).unwrap().id;
    nota(&tmp.datos(), &chat, "m1", "hola", None);
    assert!(!n.volver_a_recordar(chat.clone(), "m9".into(), 1).unwrap());
    assert_eq!(cuaderno_de(&tmp.datos(), &chat).lines().count(), 1);
    // Un chat sin cuaderno tampoco revienta.
    let vacio = n.crear_chat("Vacio".into()).unwrap().id;
    assert!(!n.volver_a_recordar(vacio, "m1".into(), 1).unwrap());
    // Ni un id de chat que se sale de la carpeta.
    assert!(n.volver_a_recordar("../x".into(), "m1".into(), 1).is_err());
}

#[test]
fn poner_dos_veces_la_hora_al_mismo_mensaje_no_lo_duplica() {
    let tmp = Temporal::nueva("dosveces");
    let n = tmp.nucleo();
    let chat = n.crear_chat("Obra".into()).unwrap().id;
    nota(&tmp.datos(), &chat, "m1", "llamar", None);
    assert!(n.volver_a_recordar(chat.clone(), "m1".into(), 9_000).unwrap());
    assert!(n.volver_a_recordar(chat.clone(), "m1".into(), 4_000).unwrap());
    let v = n.recordatorios(0).unwrap();
    assert_eq!(v.len(), 1, "cambiar la hora no anade otra");
    assert_eq!(v[0].cuando_utc_ms, 4_000, "vale la ultima");
}

#[test]
fn una_tarea_con_hora_entra_en_la_agenda_y_al_sonar_pierde_su_marca() {
    // `una_tarea_con_hora_entra_en_la_agenda_y_al_sonar_pierde_su_marca` de
    // Windows: la hora va en el texto, en hora local.
    let tmp = Temporal::nueva("tarea");
    let n = tmp.nucleo();
    let chat = n.crear_chat("Obra".into()).unwrap().id;
    let t = local(2030, 1, 2, 9, 0);
    let id = lista(
        &tmp.datos(),
        &chat,
        1_000,
        &[
            ("yeso ⏰ 2030-01-02 09:00 ➕ 2029-12-30", false),
            ("arena", false),
            ("cemento ⏰ 2030-01-03 09:00", true),
        ],
    );
    // Lima: cinco horas detras de UTC.
    let lima = -5 * 3_600_000;
    let v = n.recordatorios(lima).unwrap();
    assert_eq!(v.len(), 1, "solo la pendiente que tiene hora");
    let r = &v[0];
    assert_eq!(r.id, format!("tarea:{id}:{t}"));
    assert_eq!(r.cuando_utc_ms, t - lima, "la hora local pasa a UTC");
    assert_eq!(r.texto, "☑ yeso");
    assert_eq!(
        r.tarea,
        Some(TareaRecordada {
            lista: id.clone(),
            indice: 0
        }),
        "la lista y el numero, para «Hecha» y «+10 min»"
    );

    // La ventana de Tareas la ve sin la marca y con su hora.
    let listas = n.tareas(0).unwrap();
    let tarea = &listas[0].tareas[0];
    assert_eq!(tarea.texto, "yeso");
    assert_eq!(tarea.recuerda, Some(t));
    assert_eq!(tarea.creada.as_deref(), Some("2029-12-30"));
    assert_eq!(listas[0].tareas[1].recuerda, None);

    assert!(n.olvidar_recordatorio(chat.clone(), r.id.clone()).unwrap());
    assert!(n.recordatorios(lima).unwrap().is_empty());
    let listas = n.tareas(0).unwrap();
    let textos: Vec<&str> = listas[0].tareas.iter().map(|t| t.crudo.as_str()).collect();
    assert_eq!(
        textos,
        ["yeso ➕ 2029-12-30", "arena", "cemento ⏰ 2030-01-03 09:00"],
        "solo pierde la marca la que sono; la fecha de creacion se queda"
    );
    // Caso negativo: un id que no es de nada no toca nada.
    assert!(!n.olvidar_recordatorio(chat.clone(), "tarea:otra:1".into()).unwrap());
    assert!(!n.olvidar_recordatorio(chat, "otra-cosa".into()).unwrap());
}

#[test]
fn poner_y_quitar_la_hora_de_una_tarea_y_posponerla_y_tacharla() {
    let tmp = Temporal::nueva("ponerhora");
    let n = tmp.nucleo();
    let chat = n.crear_chat("Obra".into()).unwrap().id;
    let id = lista(&tmp.datos(), &chat, 1_000, &[("pan ➕ 2026-10-01", false), ("leche", false)]);
    let t = local(2026, 10, 9, 10, 30);

    // Desde la ventana de Tareas: con el crudo que se ve.
    assert!(n
        .poner_hora_tarea(chat.clone(), id.clone(), 0, Some("pan ➕ 2026-10-01".into()), Some(t))
        .unwrap());
    let crudo = n.tareas(0).unwrap()[0].tareas[0].crudo.clone();
    assert_eq!(crudo, "pan ⏰ 2026-10-09 10:30 ➕ 2026-10-01");
    // Caso negativo: con un crudo que ya no es, no se toca.
    assert!(!n
        .poner_hora_tarea(chat.clone(), id.clone(), 0, Some("pan".into()), None)
        .unwrap());
    assert_eq!(n.tareas(0).unwrap()[0].tareas[0].crudo, crudo);

    // «+10 min» de la notificacion: solo con el numero, cambia la hora.
    assert!(n
        .poner_hora_tarea(chat.clone(), id.clone(), 0, None, Some(t + 600_000))
        .unwrap());
    assert_eq!(n.tareas(0).unwrap()[0].tareas[0].recuerda, Some(t + 600_000));
    assert_eq!(n.recordatorios(0).unwrap().len(), 1, "cambiarla no la repite");

    // Quitarla la deja como estaba.
    assert!(n
        .poner_hora_tarea(chat.clone(), id.clone(), 0, None, None)
        .unwrap());
    assert_eq!(n.tareas(0).unwrap()[0].tareas[0].crudo, "pan ➕ 2026-10-01");
    // Una tarea que no esta, o una lista que no esta, dicen que no.
    assert!(!n.poner_hora_tarea(chat.clone(), id.clone(), 9, None, Some(t)).unwrap());
    assert!(!n.poner_hora_tarea(chat.clone(), "nada".into(), 0, None, Some(t)).unwrap());

    // «Hecha»: se tacha por su numero; otra vez, ya estaba.
    assert!(n.tachar_tarea_recordada(chat.clone(), id.clone(), 1).unwrap());
    assert!(n.tareas(0).unwrap()[0].tareas[1].hecha);
    assert!(!n.tachar_tarea_recordada(chat.clone(), id.clone(), 1).unwrap());
    assert!(!n.tachar_tarea_recordada(chat, id, 7).unwrap());
}

#[test]
fn la_posicion_de_un_mensaje_es_la_de_las_paginas_del_chat() {
    let tmp = Temporal::nueva("posicion");
    let n = tmp.nucleo();
    let chat = n.crear_chat("Obra".into()).unwrap().id;
    nota(&tmp.datos(), &chat, "a", "uno", None);
    // Los de sistema no cuentan, como en el indice de paginas.
    nota(&tmp.datos(), &chat, "auto-x", "sistema", None);
    nota(&tmp.datos(), &chat, "b", "dos", Some(5));
    assert_eq!(n.posicion_de_mensaje(chat.clone(), "b".into()).unwrap(), Some(1));
    let pagina = n.mensajes(chat.clone(), 1, 1).unwrap();
    assert_eq!(pagina.mensajes[0].id, "b");
    assert_eq!(n.posicion_de_mensaje(chat, "zz".into()).unwrap(), None);
}

#[test]
fn las_capturas_que_se_van_en_un_dia_se_avisan() {
    use std::time::{Duration, SystemTime};
    let tmp = Temporal::nueva("capturas");
    let n = tmp.nucleo();
    let dir = tmp.datos().join("capturas");
    std::fs::create_dir_all(&dir).unwrap();
    // El registro nace con `desde` ahora: lo de antes cuenta desde hoy, asi
    // que se crea primero y luego se le pone una fecha vieja.
    assert!(n.capturas_por_avisar(7).is_empty());
    let registro = tmp.datos().join("capturas-caducidad.json");
    std::fs::write(&registro, r#"{"desde":0}"#).unwrap();
    for (nombre, hace_dias) in [("captura-pronto.png", 6.5), ("captura-luego.png", 2.0)] {
        let ruta = dir.join(nombre);
        std::fs::write(&ruta, b"\x89PNG").unwrap();
        let cuando = SystemTime::now() - Duration::from_secs_f64(hace_dias * 86_400.0);
        std::fs::File::options()
            .write(true)
            .open(&ruta)
            .unwrap()
            .set_modified(cuando)
            .unwrap();
    }
    n.vaciar_caches();
    let v = n.capturas_por_avisar(7);
    let nombres: Vec<&str> = v.iter().map(|c| c.nombre.as_str()).collect();
    assert_eq!(nombres, ["captura-pronto.png"]);
    // Con la caducidad apagada no se avisa de nada.
    assert!(n.capturas_por_avisar(0).is_empty());
}
