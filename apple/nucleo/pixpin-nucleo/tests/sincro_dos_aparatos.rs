//! **Dos aparatos de verdad, de punta a punta**: dos carpetas de datos (dos
//! `Nucleo`, como dos Mac, o un Mac y un PC: el formato en disco y el cable
//! son los mismos), un grupo creado en uno y al que se une el otro con las
//! mismas llamadas que hace Swift, la `Escucha` de TCP en la red de esta
//! maquina (IPv4 y, si la hay, IPv6) y vueltas de sincronizar en los dos
//! sentidos. Se comprueba que viajan los chats, los mensajes, una nota, un
//! lienzo y la galeria de capturas; que lo borrado no resucita (la marca de
//! borrado) y que lo cambiado a la vez en los dos se junta.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use pixpin_nucleo::{Atendido, EditorLienzo, Escucha, FasePuntero, FichaChat, MensajeChat, Nucleo};

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-nucleo-dos-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temporal(dir)
    }

    fn nucleo(&self, nombre: &str) -> Arc<Nucleo> {
        Nucleo::abrir(self.0.join(nombre).display().to_string(), nombre.into()).unwrap()
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Los hilos de la escucha, como la `Puerta` de Swift.
fn pool(e: &Arc<Escucha>) -> std::sync::mpsc::Receiver<Atendido> {
    let (tx, rx) = std::sync::mpsc::channel();
    for _ in 0..pixpin_nucleo::sincro::hilos_de_escucha() {
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

/// Un chat por su nombre (el id con que se guarda puede no ser el mismo en
/// los dos aparatos: lo que viaja es el chat de sincronizar).
fn chat(n: &Nucleo, nombre: &str) -> FichaChat {
    n.chats()
        .unwrap()
        .into_iter()
        .find(|c| c.nombre == nombre)
        .unwrap_or_else(|| panic!("{nombre:?} no esta en {}", n.raiz()))
}

fn mensajes(n: &Nucleo, nombre: &str) -> Vec<MensajeChat> {
    n.ultimos_mensajes(chat(n, nombre).id, 500)
        .unwrap()
        .mensajes
}

fn textos(n: &Nucleo, nombre: &str) -> Vec<String> {
    mensajes(n, nombre).into_iter().map(|m| m.texto).collect()
}

fn captura(n: &Nucleo, nombre: &str, cuando: SystemTime) {
    let carpeta = PathBuf::from(n.carpeta_capturas());
    std::fs::create_dir_all(&carpeta).unwrap();
    let f = std::fs::File::create(carpeta.join(nombre)).unwrap();
    (&f).write_all(b"\x89PNG\r\n\x1a\n captura").unwrap();
    f.set_modified(cuando).unwrap();
}

fn capturas(n: &Nucleo) -> Vec<String> {
    let mut v: Vec<_> = n
        .galeria(0, 100, 7, 0)
        .unwrap()
        .capturas
        .into_iter()
        .map(|c| c.nombre)
        .collect();
    v.sort();
    v
}

/// Un rectangulo de `a` a `b` en el lienzo abierto, y guardado.
fn rectangulo(ed: &EditorLienzo, a: (f32, f32), b: (f32, f32)) {
    ed.herramienta("rectangulo".into()).unwrap();
    ed.puntero(FasePuntero::Pulsar, a.0, a.1, None, 0, 1.0);
    ed.puntero(
        FasePuntero::Mover,
        (a.0 + b.0) / 2.0,
        (a.1 + b.1) / 2.0,
        None,
        0,
        1.0,
    );
    ed.puntero(FasePuntero::Mover, b.0, b.1, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, b.0, b.1, None, 0, 1.0);
    assert!(ed.guardar().unwrap(), "el rectangulo se guarda");
}

/// Lo que hay en el lienzo `Plano` del chat `nombre` de `n`.
fn figuras(n: &Nucleo, nombre: &str) -> u64 {
    let m = mensajes(n, nombre)
        .into_iter()
        .find(|m| m.clase == "DIBUJO")
        .expect("el mensaje del lienzo llego");
    n.abrir_lienzo(chat(n, nombre).id, m.referencia.unwrap())
        .unwrap()
        .cuantos_elementos()
}

/// Una vuelta pedida por `de` contra la escucha de `a`, esperando a que la
/// escucha termine de atenderla.
fn vuelta(de: &Nucleo, host: &str, puerto: u16, rx: &std::sync::mpsc::Receiver<Atendido>) {
    de.sincronizar_con(host.into(), puerto)
        .unwrap_or_else(|e| panic!("vuelta por {host}: {e:?}"));
    let a = rx.recv_timeout(Duration::from_secs(20)).unwrap();
    assert!(matches!(a, Atendido::Aparato { .. }), "{a:?}");
}

/// Todo lo de arriba entre dos aparatos que hablan por `host`.
fn de_punta_a_punta(etiqueta: &str, host: &str) {
    let t = Temporal::nueva(etiqueta);
    let pc = t.nucleo("PC");
    let mac = t.nucleo("Mac");

    // 1. El grupo: el PC lo crea y escucha; el Mac se une con el codigo.
    let codigo = pc.crear_grupo().unwrap();
    assert!(pixpin_nucleo::sincro::codigo_valido(codigo.clone()));
    let escucha = Arc::clone(&pc).escuchar(0).unwrap();
    let rx = pool(&escucha);
    let puerto = escucha.puerto();
    let unido = mac
        .unirse_a_grupo(host.into(), puerto, codigo.clone())
        .unwrap();
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(10)).unwrap(),
        Atendido::Aparato { ref nombre, .. } if nombre == "Mac"
    ));
    assert_eq!(unido.grupo.as_deref(), Some(codigo.as_str()));
    assert_eq!(pc.aparato().unwrap().grupo, unido.grupo);
    // Los dos se anuncian con la misma etiqueta y letras distintas.
    let (a_pc, a_mac) = (
        pc.anuncio().unwrap().unwrap(),
        mac.anuncio().unwrap().unwrap(),
    );
    assert_eq!(a_pc.etiqueta, a_mac.etiqueta);
    assert_ne!(a_pc.id, a_mac.id);
    let letra = |a: &pixpin_nucleo::AnuncioBonjour| {
        a.txt.iter().find(|p| p.clave == "l").unwrap().valor.clone()
    };
    assert_ne!(letra(&a_pc), letra(&a_mac));

    // 2. Cosas en los dos lados: un chat con notas y un lienzo en el PC, un
    // chat con una nota en el Mac, y una captura en cada uno.
    let obra = pc.crear_chat("Obra".into()).unwrap();
    pc.escribir_nota(obra.id.clone(), "medir la cocina".into(), None)
        .unwrap();
    let para_borrar = pc
        .escribir_nota(obra.id.clone(), "esto se borrara".into(), None)
        .unwrap();
    let nota = pc
        .escribir_nota(
            obra.id.clone(),
            "Titulo\n\nPrimero.\n\nSegundo.".into(),
            None,
        )
        .unwrap();
    let dibujo = pc
        .crear_lienzo(obra.id.clone(), Some("Plano".into()))
        .unwrap();
    let ed = pc
        .editar_lienzo(obra.id.clone(), dibujo.referencia.clone().unwrap())
        .unwrap();
    rectangulo(&ed, (0.0, 0.0), (100.0, 60.0));
    drop(ed);
    let casa = mac.crear_chat("Casa".into()).unwrap();
    mac.escribir_nota(casa.id.clone(), "desde el Mac".into(), None)
        .unwrap();
    let ayer = SystemTime::now() - Duration::from_secs(86_400);
    captura(&pc, "PixPin_20261014_101500.png", ayer);
    captura(&mac, "captura-20261015-090000.png", SystemTime::now());

    // 3. Una vuelta desde el Mac: todo llega a los dos.
    vuelta(&mac, host, puerto, &rx);
    assert_eq!(
        textos(&mac, "Obra"),
        [
            "medir la cocina",
            "esto se borrara",
            "Titulo\n\nPrimero.\n\nSegundo.",
            ""
        ],
        "las notas y el lienzo del PC, en su orden"
    );
    assert_eq!(textos(&pc, "Casa"), ["desde el Mac"]);
    assert_eq!(
        figuras(&mac, "Obra"),
        1,
        "el lienzo llega con su rectangulo"
    );
    let todas = vec![
        "PixPin_20261014_101500.png".to_string(),
        "captura-20261015-090000.png".to_string(),
    ];
    assert_eq!(capturas(&mac), todas);
    assert_eq!(capturas(&pc), todas);

    // 4. En el otro sentido: el PC dirige contra la escucha del Mac.
    let escucha_mac = Arc::clone(&mac).escuchar(0).unwrap();
    let rx_mac = pool(&escucha_mac);
    let puerto_mac = escucha_mac.puerto();
    mac.escribir_nota(chat(&mac, "Obra").id, "respuesta del Mac".into(), None)
        .unwrap();
    pc.escribir_nota(casa_en(&pc), "visto en el PC".into(), None)
        .unwrap();
    vuelta(&pc, host, puerto_mac, &rx_mac);
    assert!(textos(&pc, "Obra").contains(&"respuesta del Mac".to_string()));
    assert_eq!(textos(&mac, "Casa"), ["desde el Mac", "visto en el PC"]);

    // 5. Lo borrado no resucita: el Mac borra una nota que nacio en el PC.
    let en_mac = mensajes(&mac, "Obra")
        .into_iter()
        .find(|m| m.texto == para_borrar.texto)
        .unwrap();
    assert_eq!(
        mac.borrar_mensajes(chat(&mac, "Obra").id, vec![en_mac.id])
            .unwrap(),
        1
    );
    // Caso negativo: lo que no esta no se borra dos veces.
    assert_eq!(
        mac.borrar_mensajes(chat(&mac, "Obra").id, vec!["no-existe".into()])
            .unwrap(),
        0
    );
    vuelta(&mac, host, puerto, &rx);
    vuelta(&pc, host, puerto_mac, &rx_mac);
    vuelta(&mac, host, puerto, &rx);
    for n in [&pc, &mac] {
        assert!(
            !textos(n, "Obra").contains(&para_borrar.texto),
            "la nota borrada volvio en {}",
            n.raiz()
        );
    }

    // 6. Lo cambiado a la vez en los dos se junta: cada uno cambia un
    // parrafo distinto de la misma nota y anade una figura al mismo lienzo.
    let nota_mac = mensajes(&mac, "Obra")
        .into_iter()
        .find(|m| m.texto == nota.texto)
        .unwrap();
    pc.editar_nota(
        obra.id.clone(),
        nota.id.clone(),
        "Titulo\n\nPrimero, del PC.\n\nSegundo.".into(),
    )
    .unwrap();
    mac.editar_nota(
        chat(&mac, "Obra").id,
        nota_mac.id.clone(),
        "Titulo\n\nPrimero.\n\nSegundo, del Mac.".into(),
    )
    .unwrap();
    let ref_mac = mensajes(&mac, "Obra")
        .into_iter()
        .find(|m| m.clase == "DIBUJO")
        .unwrap()
        .referencia
        .unwrap();
    let ed = mac.editar_lienzo(chat(&mac, "Obra").id, ref_mac).unwrap();
    rectangulo(&ed, (200.0, 0.0), (260.0, 40.0));
    drop(ed);
    let ed = pc
        .editar_lienzo(obra.id.clone(), dibujo.referencia.clone().unwrap())
        .unwrap();
    rectangulo(&ed, (0.0, 200.0), (50.0, 260.0));
    drop(ed);
    // Una vuelta que solo junta tambien cuenta como traer algo
    // (`fusionados`): la lista se relee.
    let r = mac.sincronizar_con(host.into(), puerto).unwrap();
    let _ = rx.recv_timeout(Duration::from_secs(20)).unwrap();
    assert!(r.fusionados > 0 && r.trajo, "{r:?}");
    let junta = "Titulo\n\nPrimero, del PC.\n\nSegundo, del Mac.";
    for n in [&pc, &mac] {
        let t = textos(n, "Obra");
        assert!(
            t.contains(&junta.to_string()),
            "sin juntar en {}: {t:?}",
            n.raiz()
        );
        assert_eq!(figuras(n, "Obra"), 3, "las tres figuras en {}", n.raiz());
    }

    // 7. Y lo que queda igual no viaja otra vez: una vuelta mas no trae ni
    // manda mensajes.
    let r = mac.sincronizar_con(host.into(), puerto).unwrap();
    let _ = rx.recv_timeout(Duration::from_secs(20)).unwrap();
    assert_eq!((r.traidos, r.enviados), (0, 0), "{r:?}");
    assert_eq!(textos(&pc, "Obra"), textos(&mac, "Obra"));

    escucha.cerrar();
    escucha_mac.cerrar();
}

/// El id del chat «Casa» en `n`.
fn casa_en(n: &Nucleo) -> String {
    chat(n, "Casa").id
}

#[test]
fn dos_aparatos_sincronizan_de_punta_a_punta_por_ipv4() {
    de_punta_a_punta("v4", "127.0.0.1");
}

#[test]
fn dos_aparatos_sincronizan_de_punta_a_punta_por_ipv6() {
    // Sin IPv6 en esta maquina (algunos contenedores), no hay nada que probar.
    if std::net::TcpListener::bind("[::1]:0").is_err() {
        eprintln!("sin IPv6: se salta");
        return;
    }
    de_punta_a_punta("v6", "::1");
}
