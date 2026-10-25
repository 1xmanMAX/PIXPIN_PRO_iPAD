//! **Dos personas de verdad, un chat compartido** (puerto de
//! `ChatsCompartidosDeVerdadTest.kt`, Android 13-oct-2026).
//!
//! Cada «aparato» es una carpeta `files` como la del movil ([DiscoAndroid])
//! y se hablan por un socket de esta maquina con el protocolo y el cifrado
//! de verdad. Max y Ana **no** estan en el mismo grupo: no comparten codigo.
//! Lo que se comprueba es lo que pidio el usuario —que el chat compartido
//! viaje y nada mas— y que no se pueda pedir, escribir ni borrar lo que no
//! toca.

use std::collections::HashSet;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;

use crate::canal::{Canal, MAGIA_DE_PERSONA, Tipo};
use crate::canonico::Json;
use crate::compartidos::{self, ChatCompartido, CompartidosEnDisco, Persona};
use crate::disco::{Disco, GENERAL};
use crate::disco_android::DiscoAndroid;
use crate::disco_android::prueba::{Reloj, conectado, crear_grupo, presentar};
use crate::invitacion::{self, ConPersona, Puerta};
use crate::kotlin;
use crate::mensajes::{Aparato, Hola, Peticion, Respuesta};
use crate::protocolo::{ErrorSincro, Hecho, Respondedor, Resultado, Salida, Sesion};

const CODIGO: &str = "K7Q2M9XP";

/// El reloj de la puerta de la invitacion (un `fn`, no un cierre): va por
/// delante de lo que escriben las pruebas.
fn reloj_de_la_puerta() -> i64 {
    5_000_000
}

struct Gente {
    dir: PathBuf,
    max: DiscoAndroid,
    ana: DiscoAndroid,
    eva: DiscoAndroid,
    reloj: Reloj,
}

impl Drop for Gente {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn montar(etiqueta: &str) -> Gente {
    let dir = std::env::temp_dir().join(format!(
        "pixpin-compartidos-{etiqueta}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let g = Gente {
        max: DiscoAndroid::nuevo(dir.join("max/files"), "max"),
        ana: DiscoAndroid::nuevo(dir.join("ana-otra-ruta/data/files"), "ana"),
        eva: DiscoAndroid::nuevo(dir.join("eva/files"), "eva"),
        dir,
        reloj: Reloj::nuevo(),
    };
    presentar(&g.max, "id-max", "Teléfono de Max");
    presentar(&g.ana, "id-ana", "Móvil de Ana");
    presentar(&g.eva, "id-eva", "Móvil de Eva");
    // Max tiene ademas su grupo de siempre: no tiene nada que ver con Ana.
    crear_grupo(&g.max, "ABCDE23456", g.reloj.ahora());
    cd(&g.max).poner_nombre("Max").unwrap();
    cd(&g.ana).poner_nombre("Ana").unwrap();
    g
}

fn cd(d: &DiscoAndroid) -> CompartidosEnDisco {
    CompartidosEnDisco::nuevo(&d.raiz())
}

fn otra_vez(d: &DiscoAndroid, nombre: &str) -> DiscoAndroid {
    DiscoAndroid::nuevo(d.raiz(), nombre)
}

#[allow(clippy::too_many_arguments)]
fn mensaje(
    d: &DiscoAndroid,
    reloj: &Reloj,
    id: &str,
    texto: &str,
    proyecto: Option<&str>,
    clase: &str,
    ruta: Option<String>,
    referencia: Option<&str>,
    uid: Option<&str>,
) {
    let numero = d
        .leer_mensajes()
        .iter()
        .filter(|m| kotlin::cadena(m, "proyecto") == proyecto)
        .filter_map(|m| kotlin::numero(m, "numero"))
        .max()
        .unwrap_or(0)
        + 1;
    let aparato = crate::grupo::codigo_de_aparato(&d.identidad().unwrap().yo.id);
    let m = Json::de_valor(&json!({
        "id": id, "cuando": reloj.tic(), "clase": clase, "texto": texto,
        "proyecto": proyecto, "ruta": ruta, "referencia": referencia,
        "numero": numero, "uid": uid, "aparato": aparato,
    }));
    d.anadir_mensaje(&m).unwrap();
}

fn nota(d: &DiscoAndroid, reloj: &Reloj, id: &str, texto: &str, proyecto: Option<&str>) {
    mensaje(d, reloj, id, texto, proyecto, "NOTA", None, None, None);
}

fn lienzo(d: &DiscoAndroid, id: &str, texto: &str) {
    let f = d.raiz().join(format!("pins/draw/{id}.excalidraw.gz"));
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    let mut e = flate2::write::GzEncoder::new(
        std::fs::File::create(&f).unwrap(),
        flate2::Compression::default(),
    );
    write!(
        e,
        r#"{{"elements":[{{"id":"e1","type":"text","text":"{texto}","version":1,"versionNonce":1,"updated":1}}],"files":{{}}}}"#
    )
    .unwrap();
    e.finish().unwrap();
}

fn proyecto(d: &DiscoAndroid, p: serde_json::Value) {
    d.guardar_proyecto(&kotlin::normalizar_proyecto(&Json::de_valor(&p)).unwrap())
        .unwrap();
}

fn existe(d: &DiscoAndroid, rel: &str) -> bool {
    d.raiz().join(rel).is_file()
}

fn textos(d: &DiscoAndroid, chat: &str) -> HashSet<String> {
    d.leer_mensajes()
        .iter()
        .filter(|m| kotlin::chat_de(m) == chat)
        .map(|m| kotlin::cadena(m, "texto").unwrap_or_default().to_string())
        .collect()
}

fn ids_de_proyectos(d: &DiscoAndroid) -> Vec<String> {
    d.leer_proyectos()
        .iter()
        .filter_map(|p| kotlin::cadena(p, "id").map(str::to_string))
        .collect()
}

/// Max con dos proyectos: «Tesis» (p1), que comparte, y «Privado» (p2), que
/// no. Y algo en la conversacion general.
fn montar_max(g: &Gente) {
    let (max, r) = (&g.max, &g.reloj);
    lienzo(max, "tesis", "planta de la tesis");
    lienzo(max, "privado", "mis cuentas");
    proyecto(
        max,
        json!({"id":"p1","nombre":"Tesis","hojas":[{"id":"h1","nombre":"Planta","dibujo":"tesis"}],"tocado":5}),
    );
    proyecto(
        max,
        json!({"id":"p2","nombre":"Privado","hojas":[{"id":"h2","nombre":"Cuentas","dibujo":"privado"}],"tocado":5}),
    );
    std::fs::create_dir_all(max.raiz().join("guardados")).unwrap();
    std::fs::write(max.raiz().join("guardados/memoria.pdf"), "%PDF memoria").unwrap();
    nota(max, r, "m1", "hola equipo", Some("p1"));
    mensaje(
        max,
        r,
        "m2",
        "",
        Some("p1"),
        "ARCHIVO",
        Some(max.absoluta("guardados/memoria.pdf")),
        None,
        Some("ADJUNTA234"),
    );
    // Lo anotado sobre el adjunto, que viaja con el.
    std::fs::write(max.raiz().join("pins/draw/anot-ADJUNTA234.hoja"), "0,0,1,1").unwrap();
    nota(max, r, "m3", "secreto del privado", Some("p2"));
    nota(max, r, "m4", "nota de la general", None);
    std::fs::write(max.raiz().join("guardados/nomina.pdf"), "%PDF nómina").unwrap();
    mensaje(
        max,
        r,
        "m5",
        "",
        Some("p2"),
        "ARCHIVO",
        Some(max.absoluta("guardados/nomina.pdf")),
        None,
        None,
    );
    let c = cd(max);
    let yo = c.yo_de(max).unwrap();
    c.poner(&compartidos::crear("p1", "Tesis", &yo, r.tic())).unwrap();
}

/// Atiende una conexion como lo hace la escucha de siempre: mira la magia y
/// atiende a una persona o al grupo.
fn atender(d: &DiscoAndroid, reloj: &Reloj, flujo: TcpStream) -> Resultado<()> {
    let r = Respondedor {
        disco: d,
        estado: &|_| {},
        ahora: &|| reloj.ahora(),
        mi_puerto: 0,
        al_saludar: &|_, _| {},
        suelto: None,
    };
    let mut magia = [0u8; 4];
    let _ = flujo.peek(&mut magia);
    if &magia == MAGIA_DE_PERSONA {
        r.atender_persona(flujo, [9u8; 32])
    } else {
        r.atender(flujo, [9u8; 32])
    }
}

/// Uno escucha en otro hilo y `uso` habla con el aqui.
fn con_escucha<T>(hacia: &DiscoAndroid, reloj: &Reloj, uso: impl FnOnce(TcpStream) -> T) -> T {
    let escucha = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let puerto = escucha.local_addr().unwrap().port();
    std::thread::scope(|h| {
        h.spawn(|| {
            if let Ok((f, _)) = escucha.accept() {
                let _ = atender(hacia, reloj, f);
            }
        });
        uso(TcpStream::connect(("127.0.0.1", puerto)).unwrap())
    })
}

/// Una vuelta normal entre dos personas: `desde` llama a `hacia`.
fn vuelta(desde: &DiscoAndroid, hacia: &DiscoAndroid, reloj: &Reloj) -> Resultado<ConPersona> {
    con_escucha(hacia, reloj, |f| {
        invitacion::vuelta_con_persona(
            f,
            desde,
            None,
            None,
            &|| reloj.ahora(),
            0,
            [3u8; 32],
            &mut |_| {},
        )
    })
}

/// Max abre la puerta de la invitacion de «Tesis».
fn puerta(g: &Gente, codigo: &str) -> Puerta {
    Puerta::abrir_con(
        Arc::new(otra_vez(&g.max, "max")),
        "p1",
        codigo,
        ("127.0.0.1", 0),
        0,
        reloj_de_la_puerta,
        |_| {},
    )
    .unwrap()
}

/// Ana entra con el codigo que le ensena Max y se trae el chat, como en la
/// ventana: primero la invitacion y luego una vuelta por la misma puerta.
fn ana_entra(g: &Gente) -> ConPersona {
    let p = puerta(g, CODIGO);
    let f = TcpStream::connect(("127.0.0.1", p.puerto)).unwrap();
    let e = invitacion::entrar(f, &g.ana, CODIGO, g.reloj.ahora(), [5u8; 32]).unwrap();
    assert_eq!(e.chat.chat, "p1");
    assert_eq!(e.quien.id, "id-max");
    let f = TcpStream::connect(("127.0.0.1", p.puerto)).unwrap();
    invitacion::vuelta_con_persona(
        f,
        &g.ana,
        Some("id-max"),
        None,
        &|| g.reloj.ahora(),
        0,
        [6u8; 32],
        &mut |_| {},
    )
    .unwrap()
}

// ------------------------------------------------------------------ pruebas

#[test]
fn el_chat_compartido_viaja_a_la_otra_persona_y_nada_mas() {
    let g = montar("viaja");
    montar_max(&g);
    let r = ana_entra(&g);
    assert_eq!(r.chats, ["p1"]);
    // Lo del chat compartido: sus mensajes, su proyecto, su lienzo, el
    // adjunto y lo anotado.
    assert_eq!(textos(&g.ana, "p1"), ["hola equipo", ""].map(String::from).into());
    assert_eq!(ids_de_proyectos(&g.ana), ["p1"]);
    assert!(existe(&g.ana, "pins/draw/tesis.excalidraw.gz"));
    assert!(existe(&g.ana, "guardados/memoria.pdf"));
    assert!(existe(&g.ana, "pins/draw/anot-ADJUNTA234.hoja"));
    // Y nada de lo demas.
    assert!(textos(&g.ana, "p2").is_empty());
    assert!(textos(&g.ana, GENERAL).is_empty());
    assert!(!existe(&g.ana, "pins/draw/privado.excalidraw.gz"));
    assert!(!existe(&g.ana, "guardados/nomina.pdf"));
    // Ana no entro en el grupo de Max ni sabe su codigo.
    let id = g.ana.identidad().unwrap();
    assert!(id.codigo.is_none());
    assert!(id.miembros.is_empty());
    // Los dos se ven dentro del chat.
    let en_ana = cd(&g.ana).de("p1").unwrap();
    assert_eq!(
        en_ana
            .personas
            .iter()
            .filter(|p| p.dentro())
            .map(|p| p.id.clone())
            .collect::<HashSet<_>>(),
        ["id-max", "id-ana"].map(String::from).into()
    );
    assert_eq!(cd(&g.max).de("p1").unwrap().persona("id-ana").unwrap().nombre, "Ana");
}

#[test]
fn lo_que_escribe_ana_llega_a_max_con_su_nombre_y_al_reves() {
    let g = montar("al-reves");
    montar_max(&g);
    ana_entra(&g);
    nota(&g.ana, &g.reloj, "a1", "he revisado la planta", Some("p1"));
    nota(&g.max, &g.reloj, "m6", "gracias", Some("p1"));
    vuelta(&g.ana, &g.max, &g.reloj).unwrap();
    assert!(textos(&g.max, "p1").contains("he revisado la planta"));
    assert!(textos(&g.ana, "p1").contains("gracias"));
    // Quien lo escribio: por el codigo de su aparato.
    let autores = compartidos::autores(&cd(&g.max).de("p1").unwrap());
    let de_ana = g
        .max
        .leer_mensajes()
        .into_iter()
        .find(|m| kotlin::cadena(m, "texto") == Some("he revisado la planta"))
        .unwrap();
    let aparato = kotlin::cadena(&de_ana, "aparato").unwrap().to_string();
    assert_eq!(autores.get(&aparato).map(String::as_str), Some("Ana"));
    // Y al reves (Max llama a Ana) tambien va.
    nota(&g.max, &g.reloj, "m7", "otra más", Some("p1"));
    vuelta(&g.max, &g.ana, &g.reloj).unwrap();
    assert!(textos(&g.ana, "p1").contains("otra más"));
    assert!(textos(&g.ana, "p2").is_empty());
    // Borrar tambien viaja: Ana borra lo suyo y a Max se le va.
    let antes = g.ana.leer_mensajes();
    let sin: Vec<Json> = antes
        .iter()
        .filter(|m| kotlin::cadena(m, "id") != Some("a1"))
        .cloned()
        .collect();
    g.ana.escribir_mensajes(&sin).unwrap();
    let borrado = antes
        .iter()
        .find(|m| kotlin::cadena(m, "id") == Some("a1"))
        .unwrap();
    g.ana
        .anotar_borrados(&[crate::disco::Marca {
            chat: "p1".into(),
            sena: kotlin::de_chat(borrado).unwrap_or_default(),
            cuando: g.reloj.tic(),
            uid: Some(kotlin::unico(borrado)),
        }])
        .unwrap();
    vuelta(&g.ana, &g.max, &g.reloj).unwrap();
    assert!(!textos(&g.max, "p1").contains("he revisado la planta"));
}

#[test]
fn un_chat_no_compartido_no_viaja_aunque_se_pida() {
    let g = montar("no-viaja");
    montar_max(&g);
    ana_entra(&g);
    con_escucha(&g.max, &g.reloj, |f| {
        let mut s = Sesion::conectar_con_persona(f, &g.ana, None, || 0, 0, [1u8; 32]).unwrap();
        // El catalogo solo ensena el compartido.
        let ids: Vec<String> = s.catalogo().unwrap().into_iter().map(|c| c.id).collect();
        assert_eq!(ids, ["p1"]);
        // Pedir el otro proyecto por su id: no.
        assert!(matches!(
            s.info_de("p2", "pins/draw/privado.excalidraw.gz"),
            Err(ErrorSincro::Remoto(_))
        ));
        // Ni la general.
        assert!(matches!(
            s.info_de(GENERAL, "guardados/nomina.pdf"),
            Err(ErrorSincro::Remoto(_))
        ));
        // Ni colarlo por el chat compartido, a mano.
        assert!(s.info_de("p1", "pins/draw/privado.excalidraw.gz").unwrap().is_none());
        // Ni las lapidas del aparato.
        assert!(s.lapidas().unwrap().is_empty());
        // Ni preparar aqui un chat que no compartimos.
        assert!(s.preparar("p2").is_err());
        s.adios();
    });
}

fn pedir(c: &mut Canal<TcpStream>, p: &Peticion) -> Respuesta {
    c.mandar(Tipo::Json, &serde_json::to_vec(p).unwrap()).unwrap();
    let (_, datos) = c.recibir().unwrap();
    serde_json::from_slice(&datos).unwrap()
}

#[test]
fn pedir_a_mano_un_archivo_de_otro_chat_se_rechaza() {
    let g = montar("a-mano");
    montar_max(&g);
    ana_entra(&g);
    con_escucha(&g.max, &g.reloj, |f| {
        let llaves = cd(&g.ana).llaves().unwrap();
        let (mut c, _) = Canal::entre_personas(f, &llaves, [2u8; 32], true, |_| true).unwrap();
        let hola = Peticion {
            hola: Some(Hola {
                yo: Aparato {
                    id: "id-ana".into(),
                    nombre: "Ana".into(),
                    ..Default::default()
                },
                reloj: 1,
                ..Default::default()
            }),
            ..Peticion::de("hola")
        };
        assert!(pedir(&mut c, &hola).hola.is_some());
        for rel in [
            "pins/draw/privado.excalidraw.gz",
            "guardados/nomina.pdf",
            "guardados/memoria.pdf",
        ] {
            let r = pedir(
                &mut c,
                &Peticion {
                    ruta: Some(rel.into()),
                    ..Peticion::sobre("dame", "p1")
                },
            );
            if rel == "guardados/memoria.pdf" {
                // El del chat compartido si: la puerta funciona y no esta
                // cerrada a todo.
                assert!(r.error.is_none(), "{:?}", r.error);
                crate::protocolo::recibir_trozos(&mut c, r.bytes, &mut std::io::sink(), &mut |_| {})
                    .unwrap();
            } else {
                assert!(r.error.is_some(), "se dio {rel}");
            }
        }
        // Pedirlo por el otro chat, tampoco.
        let r = pedir(
            &mut c,
            &Peticion {
                ruta: Some("guardados/nomina.pdf".into()),
                ..Peticion::sobre("dame", "p2")
            },
        );
        assert!(r.error.is_some());
        // Escribir encima de un archivo de otro chat, tampoco.
        let basura = g.dir.join("basura.bin");
        std::fs::write(&basura, b"basura").unwrap();
        let sale = Salida::de_fichero(&basura).unwrap();
        let pon = Peticion {
            ruta: Some("pins/draw/privado.excalidraw.gz".into()),
            bytes: sale.largo,
            ..Peticion::sobre("pon", "p1")
        };
        c.mandar(Tipo::Json, &serde_json::to_vec(&pon).unwrap()).unwrap();
        sale.mandar(&mut c, &mut |_| {}).unwrap();
        let (_, datos) = c.recibir().unwrap();
        let r: Respuesta = serde_json::from_slice(&datos).unwrap();
        assert!(r.error.is_some());
        // Ni escribir en el otro chat por su id.
        let r = pedir(
            &mut c,
            &Peticion {
                poner: vec![r#"{"id":"x","cuando":1,"clase":"NOTA","texto":"colada","proyecto":"p2","numero":9}"#.into()],
                ..Peticion::sobre("aplicar", "p2")
            },
        );
        assert!(r.error.is_some());
        // Ni borrar proyectos, ni la galeria, ni las listas del grupo, ni
        // un archivo suelto.
        for t in ["borrarchat", "galeria", "compartidos", "suelto"] {
            let r = pedir(&mut c, &Peticion::sobre(t, "p2"));
            assert!(r.error.is_some(), "{t}");
        }
        assert!(pedir(&mut c, &Peticion::de("adios")).error.is_none());
    });
    assert!(existe(&g.max, "pins/draw/privado.excalidraw.gz"));
    assert_eq!(ids_de_proyectos(&g.max), ["p1", "p2"]);
    assert!(textos(&g.max, "p2").contains("secreto del privado"));
    assert!(!textos(&g.max, "p2").contains("colada"));
}

#[test]
fn quien_no_es_miembro_es_rechazado() {
    let g = montar("no-miembro");
    montar_max(&g);
    ana_entra(&g);
    // Eva se ha hecho con la lista del chat (con la clave de Max) y se ha
    // puesto ella misma, con una firma inventada.
    let eva_c = cd(&g.eva);
    let yo_eva = eva_c.yo_de(&g.eva).unwrap();
    let mut robada = cd(&g.max).de("p1").unwrap();
    robada.personas.push(Persona {
        invito: "id-max".into(),
        firma: yo_eva.llaves.firmar(b"x"),
        ..yo_eva.ficha(g.reloj.ahora())
    });
    eva_c.guardar(&[robada]).unwrap();
    let r = vuelta(&g.eva, &g.max, &g.reloj);
    assert!(r.is_err(), "Eva no deberia poder hablar con Max");
    assert!(g.eva.leer_mensajes().is_empty());
    assert!(g.eva.leer_proyectos().is_empty());
    assert!(cd(&g.max).de("p1").unwrap().persona("id-eva").is_none());
    // Ni haciendose pasar por Ana (su id, otra clave).
    let eva_como_ana = DiscoAndroid::nuevo(g.dir.join("eva2/files"), "eva2");
    presentar(&eva_como_ana, "id-ana", "Ana");
    cd(&eva_como_ana).guardar(&cd(&g.ana).leer()).unwrap();
    let r = vuelta(&eva_como_ana, &g.max, &g.reloj);
    assert!(r.is_err(), "con otra clave no deberia pasar por Ana");
    assert!(eva_como_ana.leer_mensajes().is_empty());
    // Ni presentandose con la clave de Ana sin tener su privada: no
    // descifra ni el primer tramo.
    con_escucha(&g.max, &g.reloj, |f| {
        let mentira = crate::compartidos::Llaves::nuevas();
        let r = Canal::entre_personas(f, &mentira, [4u8; 32], true, |_| true);
        // Max no conoce esta clave: corta antes de hablar.
        if let Ok((mut c, _)) = r {
            let _ = c.mandar(Tipo::Json, b"{\"t\":\"catalogo\"}");
            assert!(c.recibir().is_err());
        }
    });
}

#[test]
fn una_invitacion_con_otro_codigo_no_entra() {
    let g = montar("otro-codigo");
    montar_max(&g);
    let p = puerta(&g, CODIGO);
    let f = TcpStream::connect(("127.0.0.1", p.puerto)).unwrap();
    let r = invitacion::entrar(f, &g.ana, "ZZZZ9999", 1, [5u8; 32]);
    assert!(
        matches!(&r, Err(ErrorSincro::Protocolo(m)) if m.contains("código")),
        "{r:?}"
    );
    assert!(cd(&g.ana).de("p1").is_none());
    assert!(cd(&g.max).de("p1").unwrap().persona("id-ana").is_none());
}

#[test]
fn cinco_codigos_equivocados_cierran_la_invitacion() {
    let g = montar("cinco");
    montar_max(&g);
    let p = puerta(&g, CODIGO);
    for i in 0..invitacion::INTENTOS {
        let f = TcpStream::connect(("127.0.0.1", p.puerto)).unwrap();
        assert!(invitacion::entrar(f, &g.ana, "ZZZZ9999", 1, [i as u8; 32]).is_err());
    }
    // Dejarle un momento para cerrar.
    for _ in 0..50 {
        if !p.abierta() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!p.abierta());
    // Ya ni con el bueno.
    let f = TcpStream::connect(("127.0.0.1", p.puerto));
    if let Ok(f) = f {
        let _ = f.set_read_timeout(Some(std::time::Duration::from_secs(2)));
        assert!(invitacion::entrar(f, &g.ana, CODIGO, 1, [9u8; 32]).is_err());
    }
    assert!(cd(&g.ana).de("p1").is_none());
}

#[test]
fn una_invitacion_caducada_no_entra() {
    let g = montar("caducada");
    montar_max(&g);
    // La puerta se cerro (la ventana ya no esta delante): no hay a quien
    // llamar.
    let mut p = puerta(&g, CODIGO);
    let puerto = p.puerto;
    p.cerrar();
    assert!(
        TcpStream::connect(("127.0.0.1", puerto))
            .ok()
            .map(|f| invitacion::entrar(f, &g.ana, CODIGO, 1, [5u8; 32]).is_err())
            .unwrap_or(true)
    );
    // Y si el chat dejo de compartirse con la puerta abierta, tampoco.
    let p = puerta(&g, CODIGO);
    cd(&g.max).guardar(&[]).unwrap();
    let f = TcpStream::connect(("127.0.0.1", p.puerto)).unwrap();
    let r = invitacion::entrar(f, &g.ana, CODIGO, 1, [5u8; 32]);
    assert!(matches!(&r, Err(ErrorSincro::Remoto(m)) if m.contains("ya no")), "{r:?}");
    assert!(cd(&g.ana).leer().is_empty());
}

#[test]
fn un_mensaje_que_nombra_un_archivo_de_otro_chat_no_lo_saca() {
    let g = montar("trampa");
    montar_max(&g);
    ana_entra(&g);
    // Ana mete en el chat compartido mensajes que senalan el lienzo y el
    // adjunto privados de Max.
    let r = &g.reloj;
    mensaje(&g.ana, r, "trampa1", "mira", Some("p1"), "DIBUJO", None, Some("privado"), None);
    mensaje(
        &g.ana,
        r,
        "trampa2",
        "",
        Some("p1"),
        "ARCHIVO",
        Some(g.ana.absoluta("guardados/nomina.pdf")),
        None,
        None,
    );
    // Y uno que se hace pasar por un mensaje de otro chat (su codigo unico).
    let uid_privado = kotlin::unico(
        &g.max
            .leer_mensajes()
            .into_iter()
            .find(|m| kotlin::cadena(m, "id") == Some("m3"))
            .unwrap(),
    );
    mensaje(&g.ana, r, "trampa3", "suplantado", Some("p1"), "NOTA", None, None, Some(&uid_privado));
    nota(&g.ana, r, "a1", "esto sí", Some("p1"));
    vuelta(&g.ana, &g.max, r).unwrap();
    vuelta(&g.max, &g.ana, r).unwrap();
    assert!(!existe(&g.ana, "pins/draw/privado.excalidraw.gz"));
    assert!(!existe(&g.ana, "guardados/nomina.pdf"));
    // Max no se los quedo; lo honrado, si.
    assert!(textos(&g.max, "p1").contains("esto sí"));
    let ids: Vec<String> = g
        .max
        .leer_mensajes()
        .iter()
        .filter_map(|m| kotlin::cadena(m, "id").map(str::to_string))
        .collect();
    for t in ["trampa1", "trampa2", "trampa3"] {
        assert!(!ids.contains(&t.to_string()), "{t}");
    }
    assert!(existe(&g.max, "guardados/nomina.pdf"));
    assert!(textos(&g.max, "p2").contains("secreto del privado"));
}

#[test]
fn borrar_un_mensaje_compartido_no_se_lleva_un_adjunto_de_otro_chat() {
    let g = montar("borrar-ajeno");
    montar_max(&g);
    // Max reenvio el adjunto privado al chat compartido... y luego se
    // arrepiente: lo quita del privado. Lo que importa es que el fichero, que
    // tambien es de «Privado», no se pierda si Ana borra su mensaje.
    let r = &g.reloj;
    mensaje(
        &g.max,
        r,
        "m8",
        "",
        Some("p1"),
        "ARCHIVO",
        Some(g.max.absoluta("guardados/nomina.pdf")),
        None,
        Some("REENVIA234"),
    );
    ana_entra(&g);
    // El adjunto que tambien es de otro chat no salio de casa de Max.
    assert!(!existe(&g.ana, "guardados/nomina.pdf"));
    // Ana borra el mensaje (lo tiene: los mensajes viajan) y se lo cuenta.
    let antes = g.ana.leer_mensajes();
    let m8 = antes
        .iter()
        .find(|m| kotlin::cadena(m, "id") == Some("m8"))
        .cloned()
        .expect("el mensaje llega aunque su adjunto no");
    g.ana
        .escribir_mensajes(
            &antes
                .iter()
                .filter(|m| kotlin::cadena(m, "id") != Some("m8"))
                .cloned()
                .collect::<Vec<_>>(),
        )
        .unwrap();
    g.ana
        .anotar_borrados(&[crate::disco::Marca {
            chat: "p1".into(),
            sena: kotlin::de_chat(&m8).unwrap_or_default(),
            cuando: r.tic(),
            uid: Some(kotlin::unico(&m8)),
        }])
        .unwrap();
    vuelta(&g.ana, &g.max, r).unwrap();
    // El mensaje se fue de casa de Max, pero el PDF del privado sigue.
    assert!(
        !g.max
            .leer_mensajes()
            .iter()
            .any(|m| kotlin::cadena(m, "id") == Some("m8"))
    );
    assert!(existe(&g.max, "guardados/nomina.pdf"));
}

#[test]
fn quitar_a_una_persona_corta_la_sincronizacion_con_ella() {
    let g = montar("quitar");
    montar_max(&g);
    ana_entra(&g);
    let c = cd(&g.max);
    let yo = c.yo_de(&g.max).unwrap();
    c.poner(&compartidos::quitar(&c.de("p1").unwrap(), &yo, "id-ana", g.reloj_de_quitar()).unwrap())
        .unwrap();
    nota(&g.max, &g.reloj, "m9", "ya no lo ves", Some("p1"));
    nota(&g.ana, &g.reloj, "a9", "ni tú esto", Some("p1"));
    let r = vuelta(&g.ana, &g.max, &g.reloj).unwrap();
    // Ana se entera de que ya no esta, y no pasa nada mas.
    assert!(r.chats.is_empty());
    assert!(!compartidos::dentro(&cd(&g.ana).de("p1").unwrap(), "id-ana"));
    assert!(!textos(&g.ana, "p1").contains("ya no lo ves"));
    assert!(!textos(&g.max, "p1").contains("ni tú esto"));
    // Lo que ya tenia se le queda.
    assert!(textos(&g.ana, "p1").contains("hola equipo"));
}

impl Gente {
    /// La hora de quitar: despues de la entrada que firmo la puerta.
    fn reloj_de_quitar(&self) -> i64 {
        reloj_de_la_puerta() + 10
    }
}

#[test]
fn quien_sale_deja_de_recibir_y_los_demas_se_enteran() {
    let g = montar("sale");
    montar_max(&g);
    ana_entra(&g);
    let c = cd(&g.ana);
    let yo = c.yo_de(&g.ana).unwrap();
    c.poner(&compartidos::quitar(&c.de("p1").unwrap(), &yo, "id-ana", g.reloj_de_quitar()).unwrap())
        .unwrap();
    nota(&g.max, &g.reloj, "m9", "después de irse", Some("p1"));
    vuelta(&g.ana, &g.max, &g.reloj).unwrap();
    assert!(!compartidos::dentro(&cd(&g.max).de("p1").unwrap(), "id-ana"));
    assert!(!textos(&g.ana, "p1").contains("después de irse"));
}

#[test]
fn un_miembro_no_echa_a_otro_ni_por_la_red() {
    let g = montar("echar");
    montar_max(&g);
    ana_entra(&g);
    // Max mete tambien a Eva (la invita el, firmado).
    let c = cd(&g.max);
    let yo_max = c.yo_de(&g.max).unwrap();
    let yo_eva = cd(&g.eva).yo_de(&g.eva).unwrap();
    c.poner(&compartidos::invitar(&c.de("p1").unwrap(), &yo_max, &yo_eva.ficha(0), reloj_de_la_puerta() + 1).unwrap())
        .unwrap();
    vuelta(&g.ana, &g.max, &g.reloj).unwrap();
    assert!(compartidos::dentro(&cd(&g.ana).de("p1").unwrap(), "id-eva"));
    // Ana (que no creo el chat) «quita» a Eva en su lista, firmandolo ella
    // pero diciendo que fue Max. En su aparato no cuela ni al guardarlo, y
    // por la red Max tampoco se lo cree.
    let ca = cd(&g.ana);
    let mut l = ca.de("p1").unwrap();
    let eva = l.persona("id-eva").unwrap().clone();
    let mut falsa = Persona {
        fuera: reloj_de_la_puerta() + 50,
        quito: Some("id-max".into()),
        firma_fuera: None,
        ..eva
    };
    let yo_ana = ca.yo_de(&g.ana).unwrap();
    falsa.firma_fuera = Some(yo_ana.llaves.firmar(&compartidos::texto_de_salida("p1", &falsa)));
    for p in &mut l.personas {
        if p.id == "id-eva" {
            *p = falsa.clone();
        }
    }
    ca.guardar(&[l]).unwrap();
    vuelta(&g.ana, &g.max, &g.reloj).unwrap();
    assert!(compartidos::dentro(&cd(&g.max).de("p1").unwrap(), "id-eva"));
    // Y Ana tampoco puede quitarla por las buenas.
    let l = ca.de("p1").unwrap();
    assert!(compartidos::quitar(&l, &yo_ana, "id-eva", reloj_de_la_puerta() + 60).is_err());
}

#[test]
fn el_grupo_de_max_lleva_el_chat_compartido_a_su_tableta_y_ana_la_acepta() {
    let g = montar("tableta");
    montar_max(&g);
    ana_entra(&g);
    let tableta = DiscoAndroid::nuevo(g.dir.join("tableta-de-max/files"), "tab");
    presentar(&tableta, "id-max-tab", "Tableta de Max");
    conectado(&tableta, &g.max, true, Some("ABCDE23456"), &g.reloj, |_| {}).unwrap();
    conectado(&tableta, &g.max, false, None, &g.reloj, |s| {
        assert!(s.compartidos_del_grupo(g.reloj.ahora()).unwrap());
        let mut hecho = Hecho::default();
        let prep = s.preparar("p1").unwrap();
        s.aplicar(&prep, &mut hecho).unwrap();
        let arch = s.preparar_archivos(&prep).unwrap();
        s.aplicar_archivos(&arch, &mut hecho, &mut |_| {}).unwrap();
        s.cerrar(&prep, g.reloj.ahora()).unwrap();
    })
    .unwrap();
    let en_tableta: ChatCompartido = cd(&tableta).de("p1").unwrap();
    assert!(compartidos::dentro(&en_tableta, "id-max-tab"));
    assert!(compartidos::dentro(&cd(&g.max).de("p1").unwrap(), "id-max-tab"));
    // El telefono le cuenta a Ana que la tableta esta; luego la tableta
    // habla con Ana directamente.
    vuelta(&g.max, &g.ana, &g.reloj).unwrap();
    assert!(compartidos::dentro(&cd(&g.ana).de("p1").unwrap(), "id-max-tab"));
    nota(&tableta, &g.reloj, "t1", "desde la tableta", Some("p1"));
    vuelta(&tableta, &g.ana, &g.reloj).unwrap();
    assert!(textos(&g.ana, "p1").contains("desde la tableta"));
    assert!(textos(&g.ana, "p2").is_empty());
}
