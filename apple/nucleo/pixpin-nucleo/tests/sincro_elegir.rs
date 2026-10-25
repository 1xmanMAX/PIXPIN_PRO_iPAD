//! **«¿Qué sincronizar?» y los proyectos borrados**, de punta a punta entre
//! dos aparatos de verdad (dos carpetas de datos, la `Escucha` de TCP y el
//! protocolo de siempre), como `lanzar_vuelta` y «con todos» de Windows:
//!
//! - Un proyecto borrado en un lado deja lapida y **no vuelve** con la
//!   vuelta siguiente.
//! - Sin preguntar («con todos», la automatica) **no se borra nada** en
//!   ningun lado: se avisa y se espera a preguntar.
//! - Preguntando, lo borrado en el otro solo se borra aqui **si se deja
//!   marcado** (empieza sin marcar) y lo borrado aqui se borra alli.
//! - «Lo mío manda» vuelve a mandar lo que el otro borro.
//! - Las tareas al instante solo llevan sus chats.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pixpin_nucleo::sincro::{EleccionSincro, ElegirSincro, PreguntaSincro};
use pixpin_nucleo::{Atendido, Escucha, Nucleo};

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-nucleo-elegir-{etiqueta}-{}",
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

/// El PC con su escucha y el Mac unido a su grupo, con «Obra» y «Casa»
/// creados en el PC y ya pasados al Mac.
struct Pareja {
    _t: Temporal,
    pc: Arc<Nucleo>,
    mac: Arc<Nucleo>,
    escucha: Arc<Escucha>,
    rx: std::sync::mpsc::Receiver<Atendido>,
    puerto: u16,
}

impl Pareja {
    fn nueva(etiqueta: &str) -> Pareja {
        let t = Temporal::nueva(etiqueta);
        let pc = t.nucleo("PC");
        let mac = t.nucleo("Mac");
        let codigo = pc.crear_grupo().unwrap();
        let escucha = Arc::clone(&pc).escuchar(0).unwrap();
        let rx = pool(&escucha);
        let puerto = escucha.puerto();
        mac.unirse_a_grupo("127.0.0.1".into(), puerto, codigo)
            .unwrap();
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        for nombre in ["Obra", "Casa"] {
            let c = pc.crear_chat(nombre.into()).unwrap();
            pc.escribir_nota(c.id, format!("nota de {nombre}"), None)
                .unwrap();
        }
        let p = Pareja {
            _t: t,
            pc,
            mac,
            escucha,
            rx,
            puerto,
        };
        let r = p.con_todos();
        assert!(r.trajo, "la primera vuelta trae los chats: {r:?}");
        assert!(p.tiene(&p.mac, "Obra") && p.tiene(&p.mac, "Casa"));
        p
    }

    fn esperar(&self) {
        let a = self.rx.recv_timeout(Duration::from_secs(20)).unwrap();
        assert!(matches!(a, Atendido::Aparato { .. }), "{a:?}");
    }

    fn con_todos(&self) -> pixpin_nucleo::ResumenSincro {
        let r = self
            .mac
            .sincronizar_con("127.0.0.1".into(), self.puerto)
            .unwrap();
        self.esperar();
        r
    }

    fn preguntando(&self, quien: Arc<Contesta>) -> pixpin_nucleo::ResumenSincro {
        let r = self
            .mac
            .sincronizar_preguntando(
                "127.0.0.1".into(),
                self.puerto,
                Box::new(Comparte(Arc::clone(&quien))),
            )
            .unwrap();
        self.esperar();
        r
    }

    fn tiene(&self, n: &Nucleo, nombre: &str) -> bool {
        n.chats().unwrap().iter().any(|c| c.nombre == nombre)
    }

    fn ficha(&self, n: &Nucleo, nombre: &str) -> String {
        n.chats()
            .unwrap()
            .into_iter()
            .find(|c| c.nombre == nombre)
            .unwrap()
            .id
    }

    fn textos(&self, n: &Nucleo, nombre: &str) -> Vec<String> {
        n.ultimos_mensajes(self.ficha(n, nombre), 100)
            .unwrap()
            .mensajes
            .into_iter()
            .map(|m| m.texto)
            .collect()
    }
}

impl Drop for Pareja {
    fn drop(&mut self) {
        self.escucha.cerrar();
    }
}

/// Quien contesta «¿Que sincronizar?» en las pruebas: apunta lo preguntado
/// y contesta lo que diga `decide`.
type Decide = Box<dyn Fn(&PreguntaSincro) -> Option<EleccionSincro> + Send + Sync>;

struct Contesta {
    vistas: Mutex<Vec<PreguntaSincro>>,
    decide: Decide,
}

impl Contesta {
    fn nuevo(
        decide: impl Fn(&PreguntaSincro) -> Option<EleccionSincro> + Send + Sync + 'static,
    ) -> Arc<Contesta> {
        Arc::new(Contesta {
            vistas: Mutex::new(Vec::new()),
            decide: Box::new(decide),
        })
    }

    fn ultima(&self) -> PreguntaSincro {
        self.vistas.lock().unwrap().last().cloned().unwrap()
    }
}

struct Comparte(Arc<Contesta>);

impl ElegirSincro for Comparte {
    fn elegir(&self, pregunta: PreguntaSincro) -> Option<EleccionSincro> {
        let r = (self.0.decide)(&pregunta);
        self.0.vistas.lock().unwrap().push(pregunta);
        r
    }
}

/// Todo marcado, tambien lo que se borraria aqui.
fn todo(p: &PreguntaSincro, lo_mio_manda: bool) -> Option<EleccionSincro> {
    Some(EleccionSincro {
        elegidos: p.filas.iter().map(|f| f.id.clone()).collect(),
        lo_mio_manda,
    })
}

/// Lo marcado como venia (lo de la ultima vez; lo borrado alli, sin marcar).
fn como_viene(p: &PreguntaSincro) -> Option<EleccionSincro> {
    Some(EleccionSincro {
        elegidos: p
            .filas
            .iter()
            .filter(|f| f.marcado)
            .map(|f| f.id.clone())
            .collect(),
        lo_mio_manda: false,
    })
}

#[test]
fn lo_borrado_en_el_pc_no_vuelve_y_solo_se_borra_en_el_mac_si_se_deja_marcado() {
    let p = Pareja::nueva("borrado-alli");
    assert!(p.pc.borrar_proyecto(p.ficha(&p.pc, "Obra")).unwrap());
    assert!(!p.tiene(&p.pc, "Obra"));

    // 1. Con todos (la automatica): no se borra nada en ningun lado, no
    // vuelve al PC y se avisa.
    let r = p.con_todos();
    assert!(p.tiene(&p.mac, "Obra"), "sin preguntar no se borra aqui");
    assert!(!p.tiene(&p.pc, "Obra"), "y no resucita en el PC");
    assert!(
        r.avisos
            .iter()
            .any(|a| a.contains("No se ha borrado «Obra» aquí")),
        "{:?}",
        r.avisos
    );

    // 2. Preguntando y cancelando: nada.
    let cancela = Contesta::nuevo(|_| None);
    let r = p.preguntando(Arc::clone(&cancela));
    assert!(r.cancelada && !r.trajo, "{r:?}");
    assert!(p.tiene(&p.mac, "Obra"));
    let q = cancela.ultima();
    assert_eq!(q.otro, "PC");
    assert_eq!(q.yo, "Mac");
    let obra = q.filas.iter().find(|f| f.nombre == "Obra").unwrap();
    assert!(obra.se_borra_aqui, "{obra:?}");
    assert!(!obra.marcado, "lo que se borraria aqui empieza sin marcar");
    assert!(obra.aqui && !obra.alli);
    let casa = q.filas.iter().find(|f| f.nombre == "Casa").unwrap();
    assert!(!casa.se_borra_aqui && casa.marcado);
    assert_eq!(casa.estado_aqui, "Igual en los dos");

    // 3. Preguntando y dejandolo como viene (sin marcar): se queda.
    let r = p.preguntando(Contesta::nuevo(como_viene));
    assert!(!r.cancelada);
    assert!(
        p.tiene(&p.mac, "Obra"),
        "caso negativo: sin marcar no se borra"
    );
    assert!(!p.tiene(&p.pc, "Obra"));

    // 4. Marcado: se borra aqui tambien.
    let r = p.preguntando(Contesta::nuevo(|q| todo(q, false)));
    assert!(r.trajo, "borrar un proyecto cambia la lista: {r:?}");
    assert!(!p.tiene(&p.mac, "Obra"), "marcado, se borra en el Mac");
    assert!(!p.tiene(&p.pc, "Obra"));
    assert!(p.tiene(&p.mac, "Casa") && p.tiene(&p.pc, "Casa"));

    // 5. Y ya no sale en la lista ni vuelve.
    let quien = Contesta::nuevo(como_viene);
    p.preguntando(Arc::clone(&quien));
    assert!(quien.ultima().filas.iter().all(|f| f.nombre != "Obra"));
    p.con_todos();
    assert!(!p.tiene(&p.mac, "Obra") && !p.tiene(&p.pc, "Obra"));
}

#[test]
fn lo_borrado_en_el_mac_se_borra_en_el_pc_preguntando_y_no_sin_preguntar() {
    let p = Pareja::nueva("borrado-aqui");
    assert!(p.mac.borrar_proyecto(p.ficha(&p.mac, "Obra")).unwrap());

    // Sin preguntar: ni vuelve al Mac ni se borra en el PC.
    let r = p.con_todos();
    assert!(!p.tiene(&p.mac, "Obra"), "no resucita en el Mac");
    assert!(p.tiene(&p.pc, "Obra"), "sin preguntar no se borra alli");
    assert!(
        r.avisos
            .iter()
            .any(|a| a.contains("se borró aquí y sigue en PC")),
        "{:?}",
        r.avisos
    );

    // Preguntando: la lista lo dice en rojo y se borra alli.
    let quien = Contesta::nuevo(como_viene);
    p.preguntando(Arc::clone(&quien));
    assert_eq!(quien.ultima().se_borran_alli, ["Obra"]);
    assert!(!p.tiene(&p.pc, "Obra"), "preguntando, se borra en el PC");
    assert!(!p.tiene(&p.mac, "Obra"));
}

#[test]
fn lo_mio_manda_le_vuelve_a_mandar_al_pc_lo_que_borro() {
    let p = Pareja::nueva("lo-mio-manda");
    assert!(p.pc.borrar_proyecto(p.ficha(&p.pc, "Obra")).unwrap());
    p.preguntando(Contesta::nuevo(|q| todo(q, true)));
    assert!(p.tiene(&p.mac, "Obra"), "con lo mio manda aqui no se borra");
    assert!(p.tiene(&p.pc, "Obra"), "y al PC le vuelve");
    assert_eq!(p.textos(&p.pc, "Obra"), ["nota de Obra"]);
}

#[test]
fn las_tareas_al_instante_solo_llevan_sus_chats() {
    let p = Pareja::nueva("tareas");
    let (obra, casa) = (p.ficha(&p.mac, "Obra"), p.ficha(&p.mac, "Casa"));
    p.mac
        .escribir_nota(obra.clone(), "nueva en obra".into(), None)
        .unwrap();
    p.mac
        .escribir_nota(casa, "nueva en casa".into(), None)
        .unwrap();
    let r = p
        .mac
        .sincronizar_tareas("127.0.0.1".into(), p.puerto, vec![obra])
        .unwrap();
    p.esperar();
    assert_eq!(r.enviados, 1, "{r:?}");
    assert_eq!(p.textos(&p.pc, "Obra"), ["nota de Obra", "nueva en obra"]);
    assert_eq!(
        p.textos(&p.pc, "Casa"),
        ["nota de Casa"],
        "caso negativo: los otros chats esperan a la vuelta entera"
    );
    // Sin fichas que viajen no se llama a nadie.
    assert!(
        p.mac
            .sincronizar_tareas("127.0.0.1".into(), p.puerto, vec!["no-existe".into()])
            .is_err()
    );
    // Y la entera lleva lo demas.
    p.con_todos();
    assert_eq!(p.textos(&p.pc, "Casa"), ["nota de Casa", "nueva en casa"]);
}

#[test]
fn mensajes_guardados_no_se_borra_como_proyecto() {
    let p = Pareja::nueva("guardados");
    let g = p.mac.chats().unwrap().into_iter().find(|c| c.es_guardados);
    if let Some(g) = g {
        assert!(!p.mac.borrar_proyecto(g.id).unwrap());
    }
    assert!(!p.mac.borrar_proyecto("no-existe".into()).unwrap());
}
