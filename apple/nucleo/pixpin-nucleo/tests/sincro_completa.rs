//! La sincronizacion entera del Mac, tal como la vera Swift, entre dos (o
//! tres) nucleos en carpetas temporales por la red de esta maquina: la
//! galeria de capturas en la vuelta, los archivos sueltos («Enviar a…»), el
//! envio con codigo de seis cifras en los dos sentidos, los chats
//! compartidos con personas (invitar, entrar, sincronizar y la valla),
//! varias conexiones a la vez en la misma escucha e IPv6 (`::1`).

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use pixpin_nucleo::{
    Atendido, AvisoEnvio, AvisoInvitacion, DondeLlego, Escucha, FinEnvio, FinRecibir, FrenteMac,
    Nucleo,
};

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-nucleo-sc-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temporal(dir)
    }

    fn nucleo(&self, nombre: &str) -> Arc<Nucleo> {
        Nucleo::abrir(self.0.join(nombre).display().to_string(), nombre.into()).unwrap()
    }

    fn fichero(&self, nombre: &str, datos: &[u8]) -> String {
        let r = self.0.join(nombre);
        std::fs::write(&r, datos).unwrap();
        r.display().to_string()
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Unos hilos atendiendo la escucha, como la `Puerta` de Swift: cada uno
/// vuelve a esperar al terminar y lo atendido sale por el canal.
fn pool(e: &Arc<Escucha>, hilos: usize) -> std::sync::mpsc::Receiver<Atendido> {
    let (tx, rx) = std::sync::mpsc::channel();
    for _ in 0..hilos {
        let e = Arc::clone(e);
        let tx = tx.clone();
        std::thread::spawn(move || {
            loop {
                match e.atender() {
                    Ok(Atendido::Cerrada) => return,
                    Ok(a) => {
                        let _ = tx.send(a);
                    }
                    Err(_) => {}
                }
            }
        });
    }
    rx
}

/// El PC crea un grupo y escucha; el Mac se une. Devuelve la escucha, lo
/// que va atendiendo y el puerto.
fn grupo(
    pc: &Arc<Nucleo>,
    mac: &Arc<Nucleo>,
    host: &str,
) -> (Arc<Escucha>, std::sync::mpsc::Receiver<Atendido>, u16) {
    pc.crear_grupo().unwrap();
    let e = Arc::clone(pc).escuchar(0).unwrap();
    let rx = pool(&e, pixpin_nucleo::sincro::hilos_de_escucha() as usize);
    let puerto = e.puerto();
    let codigo = pc.aparato().unwrap().grupo.unwrap();
    mac.unirse_a_grupo(host.into(), puerto, codigo).unwrap();
    // La union.
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(10)).unwrap(),
        Atendido::Aparato { .. }
    ));
    (e, rx, puerto)
}

fn escribir_captura(n: &Nucleo, nombre: &str, cuando: SystemTime) {
    let carpeta = PathBuf::from(n.carpeta_capturas());
    std::fs::create_dir_all(&carpeta).unwrap();
    let f = std::fs::File::create(carpeta.join(nombre)).unwrap();
    (&f).write_all(b"\x89PNG\r\n\x1a\n captura").unwrap();
    f.set_modified(cuando).unwrap();
}

#[test]
fn la_galeria_de_capturas_viaja_con_su_fecha_de_irse() {
    let t = Temporal::nueva("galeria");
    let pc = t.nucleo("PC");
    let mac = t.nucleo("Mac");
    let (escucha, _rx, puerto) = grupo(&pc, &mac, "127.0.0.1");

    let hace_un_dia = SystemTime::now() - Duration::from_secs(86_400);
    escribir_captura(&pc, "PixPin_20261014_101500.png", hace_un_dia);
    escribir_captura(&mac, "captura-20261015-090000.png", SystemTime::now());

    let r = mac.sincronizar_con("127.0.0.1".into(), puerto).unwrap();
    assert!(r.capturas >= 2, "{r:?}");

    // Las dos capturas en los dos, con la fecha de cuando se hicieron y la
    // misma fecha de irse.
    let en_mac = mac.galeria(0, 50, 7, 0).unwrap();
    let en_pc = pc.galeria(0, 50, 7, 0).unwrap();
    let nombres = |p: &pixpin_nucleo::PaginaGaleria| {
        let mut v: Vec<_> = p.capturas.iter().map(|c| c.nombre.clone()).collect();
        v.sort();
        v
    };
    assert_eq!(nombres(&en_mac), nombres(&en_pc));
    assert_eq!(en_mac.total, 2);
    let de_pc_en_mac = en_mac
        .capturas
        .iter()
        .find(|c| c.nombre == "PixPin_20261014_101500.png")
        .unwrap();
    let de_pc_en_pc = en_pc
        .capturas
        .iter()
        .find(|c| c.nombre == "PixPin_20261014_101500.png")
        .unwrap();
    assert!((de_pc_en_mac.cuando - de_pc_en_pc.cuando).abs() < 2_000);
    assert!(de_pc_en_mac.se_va.is_some());
    assert_eq!(de_pc_en_mac.se_va, de_pc_en_pc.se_va, "se van el mismo dia");
    // El estado compartido, en el fichero de siempre.
    assert!(
        PathBuf::from(mac.raiz())
            .join("galeria-compartida.json")
            .is_file()
    );

    // Borrar a mano en el Mac se lleva la del PC (a su papelera).
    mac.borrar_capturas(vec!["PixPin_20261014_101500.png".into()])
        .unwrap();
    let r = mac.sincronizar_con("127.0.0.1".into(), puerto).unwrap();
    assert!(r.capturas_tiradas == 0, "las tira el otro, no este: {r:?}");
    let en_pc = pc.galeria(0, 50, 7, 0).unwrap();
    assert_eq!(en_pc.total, 1, "{:?}", nombres(&en_pc));
    assert!(
        PathBuf::from(pc.raiz())
            .join("papelera/capturas/PixPin_20261014_101500.png")
            .is_file()
    );
    escucha.cerrar();
}

#[test]
fn un_archivo_suelto_va_a_lo_que_el_otro_tiene_delante() {
    let t = Temporal::nueva("suelto");
    let pc = t.nucleo("PC");
    let mac = t.nucleo("Mac");
    let (escucha, rx, puerto) = grupo(&pc, &mac, "127.0.0.1");
    let pdf = t.fichero("informe.pdf", b"%PDF-1.4 nada");
    let png = t.fichero("foto.png", b"\x89PNG\r\n\x1a\n foto");

    // Sin nada delante: a «Mensajes guardados».
    let r = mac
        .enviar_a_aparato("127.0.0.1".into(), puerto, vec![pdf.clone()])
        .unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].donde, Some(DondeLlego::Chat), "{r:?}");
    let a = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let Atendido::Archivos {
        nombre, llegadas, ..
    } = a
    else {
        panic!("{a:?}")
    };
    assert_eq!(nombre, "Mac");
    assert_eq!(llegadas[0].nombre, "informe.pdf");
    let guardados = pc.chat_guardados().unwrap();
    let m = pc.ultimos_mensajes(guardados.id.clone(), 10).unwrap();
    assert_eq!(m.mensajes.last().unwrap().nombre, "informe.pdf");

    // Con un chat abierto: a ese chat, y el otro sabe su nombre.
    let obra = pc.crear_chat("Obra".into()).unwrap();
    pc.poner_al_frente(FrenteMac::Chat {
        id: obra.id.clone(),
        nombre: "Obra".into(),
    });
    let r = mac
        .enviar_a_aparato("127.0.0.1".into(), puerto, vec![png.clone()])
        .unwrap();
    assert_eq!(r[0].donde, Some(DondeLlego::ChatAbierto));
    assert_eq!(r[0].chat.as_deref(), Some("Obra"));
    let _ = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let m = pc.ultimos_mensajes(obra.id.clone(), 10).unwrap();
    assert_eq!(m.mensajes[0].clase, "IMAGEN");

    // Con un lienzo delante: la foto si (espera en `sincro/al-lienzo`), el
    // PDF no, y no corta lo que va detras.
    pc.poner_al_frente(FrenteMac::Lienzo {
        chat: obra.id.clone(),
    });
    let r = mac
        .enviar_a_aparato("127.0.0.1".into(), puerto, vec![pdf, png])
        .unwrap();
    assert_eq!(r[0].donde, None);
    assert!(
        r[0].error.as_deref().unwrap().contains("solo acepta fotos"),
        "{r:?}"
    );
    assert_eq!(r[1].donde, Some(DondeLlego::Lienzo));
    let Atendido::Archivos { llegadas, .. } = rx.recv_timeout(Duration::from_secs(10)).unwrap()
    else {
        panic!()
    };
    assert_eq!(llegadas.len(), 1);
    assert!(PathBuf::from(llegadas[0].ruta.clone().unwrap()).is_file());

    // Caso negativo: sin grupo no se manda nada.
    let solo = t.nucleo("Solo");
    assert!(
        solo.enviar_a_aparato("127.0.0.1".into(), puerto, vec![])
            .is_err()
    );
    escucha.cerrar();
}

#[test]
fn el_envio_con_codigo_va_y_vuelve() {
    let t = Temporal::nueva("envio");
    let pc = t.nucleo("PC");
    let mac = t.nucleo("Mac");
    let notas = t.fichero("notas.txt", b"hola desde el PC");

    // 1. El PC envia y espera; el Mac llama con el codigo. El PC aprueba.
    let puerta = pc.abrir_envio(vec![notas.clone()]).unwrap();
    assert_eq!(puerta.codigo().len(), 6);
    assert!(
        puerta
            .texto_del_qr(Some("192.168.1.4".into()))
            .starts_with("pixpin-envio:1:")
    );
    assert!(
        puerta
            .texto_del_qr(Some("fe80::1%en0".into()))
            .contains("::")
    );
    let p = Arc::clone(&puerta);
    let atiende = std::thread::spawn(move || p.atender().unwrap());
    let p = Arc::clone(&puerta);
    let aprueba = std::thread::spawn(move || {
        loop {
            match p.siguiente_aviso() {
                AvisoEnvio::Llega { llamada, nombre } => {
                    assert_eq!(nombre, "Mac");
                    p.aprobar(llamada, true);
                }
                AvisoEnvio::Fin { fin, .. } => return fin,
                AvisoEnvio::Cerrada => return FinEnvio::Cerrada,
                AvisoEnvio::Avance { .. } => {}
            }
        }
    });
    // Quien solo mira donde esta (Swift resolviendo el servicio de Bonjour)
    // y cuelga sin hablar no cuenta como llamada.
    drop(std::net::TcpStream::connect(("127.0.0.1", puerta.puerto())).unwrap());
    let llegado = mac
        .recibir_envio("127.0.0.1".into(), puerta.puerto(), puerta.codigo())
        .unwrap();
    assert_eq!(
        atiende.join().unwrap(),
        FinEnvio::Enviado {
            nombre: "Mac".into()
        }
    );
    assert_eq!(
        aprueba.join().unwrap(),
        FinEnvio::Enviado {
            nombre: "Mac".into()
        }
    );
    assert_eq!(llegado.len(), 1);
    assert_eq!(llegado[0].donde, "guardados");
    let m = mac.ultimos_mensajes(llegado[0].chat.clone(), 10).unwrap();
    assert_eq!(m.mensajes.last().unwrap().nombre, "notas.txt");

    // Caso negativo: un codigo equivocado cuenta, y no llega nada.
    let p = Arc::clone(&puerta);
    let atiende = std::thread::spawn(move || p.atender().unwrap());
    let malo = if puerta.codigo() == "000000" {
        "111111"
    } else {
        "000000"
    };
    assert!(
        mac.recibir_envio("127.0.0.1".into(), puerta.puerto(), malo.into())
            .is_err()
    );
    assert_eq!(
        atiende.join().unwrap(),
        FinEnvio::CodigoMalo { intentos: 1 }
    );
    puerta.cerrar();
    assert_eq!(puerta.atender().unwrap(), FinEnvio::Cerrada);

    // 2. Al reves: el Mac espera para recibir y el PC le manda.
    let espera = Arc::clone(&mac).esperar_envios().unwrap();
    assert!(espera.texto_del_qr(None).starts_with("pixpin-recibe:1:"));
    let e = Arc::clone(&espera);
    let atiende = std::thread::spawn(move || e.atender().unwrap());
    let foto = t.fichero("plano.png", b"\x89PNG\r\n\x1a\n plano");
    let fin = pc
        .mandar_a_quien_espera(
            "127.0.0.1".into(),
            espera.puerto(),
            espera.codigo(),
            vec![foto],
        )
        .unwrap();
    assert_eq!(
        fin,
        FinEnvio::Enviado {
            nombre: "Mac".into()
        }
    );
    let FinRecibir::Recibido { de, cosas } = atiende.join().unwrap() else {
        panic!()
    };
    assert_eq!(de, "PC");
    assert_eq!(cosas[0].nombre, "plano.png");
    let m = mac.ultimos_mensajes(cosas[0].chat.clone(), 10).unwrap();
    assert_eq!(m.mensajes.last().unwrap().clase, "IMAGEN");
    espera.cerrar();
}

#[test]
fn un_chat_compartido_se_invita_se_entra_y_se_sincroniza_con_su_valla() {
    let t = Temporal::nueva("compartido");
    // Ana y Beto no estan en ningun grupo: solo comparten un chat.
    let ana = t.nucleo("Ana");
    let beto = t.nucleo("Beto");
    let tesis = ana.crear_chat("Tesis".into()).unwrap();
    ana.escribir_nota(tesis.id.clone(), "capitulo 1".into(), None)
        .unwrap();
    let privado = ana.crear_chat("Privado".into()).unwrap();
    ana.escribir_nota(privado.id.clone(), "secreto".into(), None)
        .unwrap();

    // Sin compartir no se invita.
    assert!(Arc::clone(&ana).invitar_a_chat(tesis.id.clone()).is_err());
    assert!(!ana.hay_chats_compartidos().unwrap());
    let c = ana.compartir_chat(tesis.id.clone()).unwrap();
    assert!(c.estoy_dentro);
    assert_eq!(c.personas.len(), 1);
    assert!(ana.hay_chats_compartidos().unwrap());
    // Sin grupo, pero con un chat compartido, se anuncia (sin `g`).
    let a = ana.anuncio_de_presencia().unwrap().unwrap();
    assert!(a.txt.iter().all(|p| p.clave != "g"));
    assert!(ana.anuncio().unwrap().is_none());

    // La invitacion: Beto entra con el codigo y se trae el chat.
    let inv = Arc::clone(&ana).invitar_a_chat(tesis.id.clone()).unwrap();
    let qr = inv.texto_del_qr(Some("192.168.1.5".into()));
    let leido = pixpin_nucleo::compartidos::leer_invitacion(qr).unwrap();
    assert_eq!(leido.codigo, inv.codigo());
    let i = Arc::clone(&inv);
    let puerta = std::thread::spawn(move || (i.atender().unwrap(), i.atender().unwrap()));
    let dentro = beto
        .entrar_en_chat_compartido(
            pixpin_nucleo::compartidos::invitacion_legible(inv.codigo()),
            "127.0.0.1".into(),
            inv.puerto(),
        )
        .unwrap();
    let (entro, junto) = puerta.join().unwrap();
    assert!(matches!(entro, AvisoInvitacion::Entro { ref nombre, .. } if nombre == "Beto"));
    assert!(matches!(junto, AvisoInvitacion::Junto { .. }), "{junto:?}");
    assert!(dentro.estoy_dentro);
    assert_eq!(dentro.personas.iter().filter(|p| p.dentro).count(), 2);
    inv.cerrar();

    // Beto tiene la Tesis con su nota y NADA del chat privado (la valla).
    let suyos = beto.chats().unwrap();
    let suya = suyos.iter().find(|c| c.nombre == "Tesis").expect("llego");
    assert!(suyos.iter().all(|c| c.nombre != "Privado"));
    let p = beto.ultimos_mensajes(suya.id.clone(), 10).unwrap();
    assert_eq!(p.mensajes[0].texto, "capitulo 1");

    // Despues, por la escucha de siempre de Ana: Beto escribe y sincroniza.
    beto.escribir_nota(suya.id.clone(), "comentario de Beto".into(), None)
        .unwrap();
    let escucha = Arc::clone(&ana).escuchar(0).unwrap();
    let rx = pool(&escucha, 2);
    let r = beto
        .sincronizar_con_persona(
            "127.0.0.1".into(),
            escucha.puerto(),
            Some(ana.aparato().unwrap().id),
            vec![],
        )
        .unwrap();
    assert_eq!(r.otro_nombre, "Ana");
    assert_eq!(r.chats.len(), 1);
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(10)).unwrap(),
        Atendido::Persona { ref nombre, .. } if nombre == "Beto"
    ));
    let p = ana.ultimos_mensajes(tesis.id.clone(), 10).unwrap();
    assert!(p.mensajes.iter().any(|m| m.texto == "comentario de Beto"));

    // Ana (la que lo creo) quita a Beto; Beto ya no puede quitar a nadie.
    let beto_id = beto.aparato().unwrap().id;
    let c = ana
        .quitar_de_chat_compartido(dentro.chat.clone(), beto_id)
        .unwrap();
    assert_eq!(c.personas.iter().filter(|p| p.dentro).count(), 1);
    assert!(
        beto.quitar_de_chat_compartido(dentro.chat.clone(), ana.aparato().unwrap().id)
            .is_err()
    );

    // Caso negativo: un tercero sin chats compartidos no habla con Ana.
    let eva = t.nucleo("Eva");
    assert!(
        eva.sincronizar_con_persona("127.0.0.1".into(), escucha.puerto(), None, vec![])
            .is_err()
    );
    escucha.cerrar();
}

#[test]
fn una_sonda_no_espera_a_una_vuelta() {
    let t = Temporal::nueva("varias");
    let pc = t.nucleo("PC");
    let mac = t.nucleo("Mac");
    let (escucha, _rx, puerto) = grupo(&pc, &mac, "127.0.0.1");

    // Una conexion que saluda y se queda callada: ocupa un hilo de la
    // escucha como una vuelta larga (el otro pensando que sincronizar).
    let mut lenta = std::net::TcpStream::connect(("127.0.0.1", puerto)).unwrap();
    lenta.write_all(b"PXS1").unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // La sonda contesta al momento.
    let inicio = Instant::now();
    let mut s = std::net::TcpStream::connect(("127.0.0.1", puerto)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    s.write_all(pixpin_sincro::SONDA).unwrap();
    let mut r = [0u8; 4];
    s.read_exact(&mut r).unwrap();
    assert_eq!(&r, pixpin_sincro::SONDA_RESPUESTA);
    assert!(inicio.elapsed() < Duration::from_secs(2));

    // Y una vuelta entera tambien, a la vez, con otra sonda durante ella.
    let obra = mac.crear_chat("Obra".into()).unwrap();
    for i in 0..30 {
        mac.escribir_nota(obra.id.clone(), format!("nota {i}"), None)
            .unwrap();
    }
    let m = Arc::clone(&mac);
    let vuelta = std::thread::spawn(move || m.sincronizar_con("127.0.0.1".into(), puerto));
    let mut s = std::net::TcpStream::connect(("127.0.0.1", puerto)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    s.write_all(pixpin_sincro::SONDA).unwrap();
    s.read_exact(&mut r).unwrap();
    assert_eq!(&r, pixpin_sincro::SONDA_RESPUESTA);
    let hecho = vuelta.join().unwrap().unwrap();
    assert_eq!(hecho.enviados, 30);
    drop(lenta);
    escucha.cerrar();
}

#[test]
fn se_sincroniza_por_ipv6() {
    // Sin IPv6 en esta maquina (algunos contenedores), no hay nada que probar.
    if std::net::TcpListener::bind("[::1]:0").is_err() {
        eprintln!("sin IPv6: se salta");
        return;
    }
    let t = Temporal::nueva("ipv6");
    let pc = t.nucleo("PC");
    let mac = t.nucleo("Mac");
    let (escucha, _rx, puerto) = grupo(&pc, &mac, "::1");
    let obra = mac.crear_chat("Obra".into()).unwrap();
    mac.escribir_nota(obra.id.clone(), "por IPv6".into(), None)
        .unwrap();
    // Con corchetes, como se escribe con puerto, y sin ellos.
    let r = mac.sincronizar_con("[::1]".into(), puerto).unwrap();
    assert_eq!(r.otro_nombre, "PC");
    let suya = pc.chats().unwrap();
    let suya = suya.iter().find(|c| c.nombre == "Obra").unwrap();
    let p = pc.ultimos_mensajes(suya.id.clone(), 10).unwrap();
    assert_eq!(p.mensajes[0].texto, "por IPv6");
    // La misma escucha atiende tambien por IPv4.
    assert!(mac.sincronizar_con("127.0.0.1".into(), puerto).is_ok());
    escucha.cerrar();
}
