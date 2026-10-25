//! **Los aparatos del grupo y las copias de seguridad**, de punta a punta
//! entre dos carpetas de datos de verdad (la `Escucha` de TCP y el protocolo
//! de siempre), como «Este aparato», «Tus aparatos» y «Copias de seguridad»
//! de Windows:
//!
//! - Renombrar este aparato llega a los demas en la vuelta siguiente.
//! - Los miembros salen con la ultima vez que se sincronizo con ellos.
//! - La sonda `PING`/`PONG` dice si una direccion recordada contesta.
//! - Salir del grupo olvida el grupo y lo acordado, pero no borra chats.
//! - Las copias que deja una vuelta se listan y se vuelve a ellas.
//! - Un proyecto borrado se recupera entero de la papelera.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use pixpin_nucleo::{Atendido, ErrorNucleo, Escucha, Nucleo};

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-nucleo-aparatos-{etiqueta}-{}",
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

/// El PC escuchando y el Mac unido a su grupo, con «Obra» ya pasado.
struct Pareja {
    t: Temporal,
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
        let c = pc.crear_chat("Obra".into()).unwrap();
        pc.escribir_nota(c.id, "la primera".into(), None).unwrap();
        let p = Pareja {
            t,
            pc,
            mac,
            escucha,
            rx,
            puerto,
        };
        p.vuelta();
        p
    }

    fn vuelta(&self) -> pixpin_nucleo::ResumenSincro {
        let r = self
            .mac
            .sincronizar_con("127.0.0.1".into(), self.puerto)
            .unwrap();
        let a = self.rx.recv_timeout(Duration::from_secs(20)).unwrap();
        assert!(matches!(a, Atendido::Aparato { .. }), "{a:?}");
        r
    }

    fn ficha(&self, n: &Nucleo, nombre: &str) -> Option<String> {
        n.chats()
            .unwrap()
            .into_iter()
            .find(|c| c.nombre == nombre)
            .map(|c| c.id)
    }
}

impl Drop for Pareja {
    fn drop(&mut self) {
        self.escucha.cerrar();
    }
}

#[test]
fn los_miembros_salen_con_la_ultima_vez_y_el_nombre_nuevo_llega_al_otro() {
    let p = Pareja::nueva("miembros");
    let pc = p.pc.aparato().unwrap();
    let del_mac = p.mac.miembros_del_grupo().unwrap();
    assert_eq!(del_mac.len(), 1, "solo el otro, no este: {del_mac:?}");
    assert_eq!(del_mac[0].id, pc.id);
    assert!(del_mac[0].ultima_vez > 0, "ya se sincronizo con el");

    let yo = p.mac.renombrar_aparato("  Mac de Max  ".into()).unwrap();
    assert_eq!(yo.nombre, "Mac de Max");
    p.vuelta();
    let del_pc = p.pc.miembros_del_grupo().unwrap();
    assert!(
        del_pc.iter().any(|m| m.nombre == "Mac de Max"),
        "el PC ve el nombre nuevo: {del_pc:?}"
    );

    // Caso negativo: vacio no vale, y el nombre se queda como estaba.
    assert!(matches!(
        p.mac.renombrar_aparato("   ".into()),
        Err(ErrorNucleo::Argumento { .. })
    ));
    assert_eq!(p.mac.aparato().unwrap().nombre, "Mac de Max");
    // Y el tope del movil.
    let largo = p.mac.renombrar_aparato("x".repeat(90)).unwrap();
    assert_eq!(largo.nombre.chars().count(), 40);
}

#[test]
fn la_direccion_recordada_se_sondea_y_se_apunta() {
    let p = Pareja::nueva("direcciones");
    let pc = p.pc.aparato().unwrap();
    assert!(p.mac.sondear_direccion("127.0.0.1".into(), p.puerto));
    assert!(
        p.mac
            .apuntar_direccion(pc.id.clone(), "127.0.0.1".into(), p.puerto)
            .unwrap()
    );
    let m = p.mac.miembros_del_grupo().unwrap();
    let d = m[0].direccion.clone().expect("con su direccion recordada");
    assert_eq!((d.host.as_str(), d.puerto), ("127.0.0.1", p.puerto));
    assert!(
        p.t.0.join("Mac/sincro/direcciones.txt").is_file(),
        "en el fichero del movil"
    );
    // Caso negativo: cerrada la escucha, la sonda dice que no.
    p.escucha.cerrar();
    std::thread::sleep(Duration::from_millis(100));
    assert!(!p.mac.sondear_direccion("127.0.0.1".into(), p.puerto));
}

#[test]
fn salir_del_grupo_olvida_el_grupo_pero_no_los_chats() {
    let p = Pareja::nueva("salir");
    assert!(p.ficha(&p.mac, "Obra").is_some());
    let base = p.t.0.join("Mac/sincro/base");
    assert!(base.is_dir(), "la vuelta dejo lo acordado");
    let yo = p.mac.salir_del_grupo().unwrap();
    assert!(yo.grupo.is_none() && yo.letra.is_none());
    assert!(p.mac.miembros_del_grupo().unwrap().is_empty());
    assert!(
        p.mac.anuncio().unwrap().is_none(),
        "ya no se anuncia en el grupo"
    );
    assert!(!base.exists(), "lo acordado se olvida");
    assert!(p.ficha(&p.mac, "Obra").is_some(), "no se borra ningun chat");
    // Caso negativo: el id de este aparato no cambia al salir.
    assert_eq!(yo.id, p.mac.aparato().unwrap().id);
}

#[test]
fn las_copias_de_una_vuelta_se_listan_y_se_vuelve_a_ellas() {
    let p = Pareja::nueva("copias");
    let obra = p.ficha(&p.mac, "Obra").unwrap();
    // El PC escribe; la vuelta que lo trae deja copia de como estaba aqui.
    let en_pc = p.ficha(&p.pc, "Obra").unwrap();
    p.pc.escribir_nota(en_pc, "la segunda".into(), None)
        .unwrap();
    p.vuelta();
    let copias = p.mac.copias_de_seguridad();
    assert!(!copias.is_empty(), "una vuelta deja copia");
    let c = &copias[0];
    assert_eq!(c.nombre, "Obra");
    assert!(c.mensajes >= 1);
    assert!(
        copias.windows(2).all(|w| w[0].cuando >= w[1].cuando),
        "la mas nueva primero"
    );
    let nombre = p.mac.volver_a_copia(c.chat.clone(), c.id.clone()).unwrap();
    assert_eq!(nombre, "Obra");
    assert!(p.ficha(&p.mac, "Obra").is_some_and(|f| f == obra));
    // Caso negativo: una copia que no esta no restaura otra cualquiera.
    assert!(matches!(
        p.mac.volver_a_copia(c.chat.clone(), "no-esta".into()),
        Err(ErrorNucleo::NoExiste { .. })
    ));
}

#[test]
fn un_proyecto_borrado_se_recupera_entero() {
    let t = Temporal::nueva("papelera");
    let n = t.nucleo("Mac");
    let mut fichas = Vec::new();
    for nombre in ["Tesis", "Casa"] {
        let c = n.crear_chat(nombre.into()).unwrap();
        n.escribir_nota(c.id.clone(), format!("nota de {nombre}"), None)
            .unwrap();
        fichas.push(c.id);
    }
    assert!(n.proyectos_borrados().is_empty());
    assert!(n.borrar_proyecto(fichas[0].clone()).unwrap());
    let borrados = n.proyectos_borrados();
    assert_eq!(borrados.len(), 1);
    assert_eq!(borrados[0].nombre, "Tesis");
    assert_eq!(borrados[0].mensajes, 1);
    assert!(borrados[0].borrado > 0);

    // Caso negativo: lo que no esta en la papelera no se recupera.
    assert!(n.recuperar_proyecto("otra-cosa".into()).is_err());
    assert!(n.recuperar_proyecto("../Casa".into()).is_err());

    let nombre = n.recuperar_proyecto(borrados[0].carpeta.clone()).unwrap();
    assert_eq!(nombre, "Tesis");
    assert!(n.proyectos_borrados().is_empty());
    assert!(n.chats().unwrap().iter().any(|c| c.nombre == "Tesis"));

    // «Recuperar todos».
    for f in &fichas {
        n.borrar_proyecto(f.clone()).unwrap();
    }
    let r = n.recuperar_todos();
    assert_eq!((r.n, r.total), (2, 2));
    assert_eq!(
        n.chats()
            .unwrap()
            .iter()
            .filter(|c| c.nombre != "Mensajes guardados")
            .count(),
        2
    );
}
