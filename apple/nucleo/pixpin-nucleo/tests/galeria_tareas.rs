//! La galeria de capturas, el timeline, las tareas y buscar en todo, tal
//! como los vera Swift: carpetas de datos de verdad en un temporal y los
//! ficheros escritos como los escribe Windows.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use pixpin_nucleo::{ClaseDeResultado, ErrorNucleo, Nucleo};
use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{self, Mensaje, Sello};
use pixpin_proyecto::mini;
use pixpin_sincro::galeria::{Caducidad, DIA_MS};
use pixpin_timeline::Momento;
use pixpin_timeline::almacen::Almacen;

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-nucleo-gt-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Temporal(dir)
    }

    fn nucleo(&self) -> Arc<Nucleo> {
        Nucleo::abrir(self.0.join("datos").display().to_string(), "Mac".into()).unwrap()
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

fn ahora() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Una captura en `capturas/` con su fecha de modificacion.
fn captura(datos: &Path, nombre: &str, hace_ms: i64) {
    let dir = datos.join("capturas");
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join(nombre);
    std::fs::write(&ruta, b"\x89PNG falso").unwrap();
    let cuando = SystemTime::now() - Duration::from_millis(hace_ms as u64);
    std::fs::File::options()
        .write(true)
        .open(&ruta)
        .unwrap()
        .set_modified(cuando)
        .unwrap();
}

fn registro(datos: &Path) -> Caducidad {
    serde_json::from_str(&std::fs::read_to_string(datos.join("capturas-caducidad.json")).unwrap())
        .unwrap()
}

#[test]
fn la_galeria_lista_por_paginas_con_su_caducidad() {
    let t = Temporal::nueva("lista");
    let n = t.nucleo();
    // Sin carpeta: vacia, no un error.
    assert_eq!(n.galeria(0, 50, 7, 0).unwrap().total, 0);
    // El registro nace con `desde` ahora: lo de antes cuenta desde hoy.
    captura(&t.datos(), "captura-vieja.png", 30 * DIA_MS);
    captura(&t.datos(), "captura-a.png", 3 * DIA_MS);
    captura(&t.datos(), "captura-b.jpg", DIA_MS);
    captura(&t.datos(), "video.mp4", 1000);
    captura(&t.datos(), "notas.txt", 0);
    std::fs::write(t.datos().join("capturas").join("vacia.png"), b"").unwrap();

    let p = n.galeria(0, 2, 7, 0).unwrap();
    assert_eq!(p.total, 4, "ni el txt ni la vacia");
    let nombres: Vec<&str> = p.capturas.iter().map(|c| c.nombre.as_str()).collect();
    assert_eq!(
        nombres,
        ["video.mp4", "captura-b.jpg"],
        "la mas nueva primero"
    );
    assert!(p.capturas[0].es_video);
    let siguiente = n.galeria(2, 2, 7, 0).unwrap();
    let nombres: Vec<&str> = siguiente
        .capturas
        .iter()
        .map(|c| c.nombre.as_str())
        .collect();
    assert_eq!(nombres, ["captura-a.png", "captura-vieja.png"]);
    assert!(Path::new(&siguiente.capturas[0].ruta).is_file());

    // El registro nacio con la primera mirada: todo cuenta desde entonces.
    let r = registro(&t.datos());
    let vieja = &siguiente.capturas[1];
    assert_eq!(vieja.se_va, Some(r.desde + 7 * DIA_MS), "no se va de golpe");
    assert_eq!(siguiente.capturas[0].dias_que_quedan, Some(7));
    // Con un estreno de hace tiempo, cada una cuenta desde que se hizo.
    let viejo = Caducidad { desde: 0, ..r };
    std::fs::write(
        t.datos().join("capturas-caducidad.json"),
        serde_json::to_vec(&viejo).unwrap(),
    )
    .unwrap();
    let siguiente = n.galeria(2, 2, 7, 0).unwrap();
    let a = &siguiente.capturas[0];
    assert_eq!(a.se_va, Some(a.cuando + 7 * DIA_MS));
    assert_eq!(a.dias_que_quedan, Some(4));
    assert_eq!(siguiente.capturas[1].dias_que_quedan, Some(0), "ya le toca");
    // Con la caducidad apagada no se va ninguna.
    assert!(
        n.galeria(0, 10, 0, 0)
            .unwrap()
            .capturas
            .iter()
            .all(|c| c.se_va.is_none())
    );
    // Pasado el final, pagina vacia.
    assert!(n.galeria(99, 10, 7, 0).unwrap().capturas.is_empty());
}

#[test]
fn conservar_la_manda_a_guardados_y_ya_no_se_va() {
    let t = Temporal::nueva("conservar");
    let n = t.nucleo();
    captura(&t.datos(), "captura-1.png", 0);
    let m = n.conservar_captura("captura-1.png".into()).unwrap();
    assert_eq!(m.clase, "IMAGEN");
    assert_eq!(m.nombre, "captura-1.png");
    let guardados = n.chat_guardados().unwrap();
    let ruta = n
        .ruta_absoluta(guardados.id.clone(), m.ruta.clone().unwrap())
        .unwrap();
    assert_eq!(std::fs::read(ruta).unwrap(), b"\x89PNG falso");
    assert!(registro(&t.datos()).conservadas.contains("captura-1.png"));
    let c = &n.galeria(0, 10, 7, 0).unwrap().capturas[0];
    assert!(c.conservada);
    assert_eq!((c.se_va, c.dias_que_quedan), (None, None));
    // Caso negativo: dos veces no la manda dos veces.
    assert!(matches!(
        n.conservar_captura("captura-1.png".into()),
        Err(ErrorNucleo::Argumento { .. })
    ));
    assert_eq!(n.ultimos_mensajes(guardados.id, 10).unwrap().total, 1);
    // Ni un nombre con rutas.
    assert!(matches!(
        n.conservar_captura("../indice.json".into()),
        Err(ErrorNucleo::Argumento { .. })
    ));
}

#[test]
fn siete_dias_mas_alarga_y_borrar_se_deshace() {
    let t = Temporal::nueva("borrar");
    let n = t.nucleo();
    captura(&t.datos(), "a.png", 0);
    captura(&t.datos(), "b.png", 0);
    let antes = n.galeria(0, 10, 7, 0).unwrap().capturas[0].se_va.unwrap();
    let nueva = n.prorrogar_captura("a.png".into(), 7).unwrap().unwrap();
    assert_eq!(nueva, antes.max(ahora()) + 7 * DIA_MS);
    assert_eq!(registro(&t.datos()).prorrogadas.get("a.png"), Some(&nueva));

    let borradas = n
        .borrar_capturas(vec!["a.png".into(), "b.png".into()])
        .unwrap();
    assert_eq!(borradas.len(), 2);
    assert_eq!(borradas[0].prorroga, Some(nueva));
    assert!(t.datos().join("papelera/capturas/a.png").is_file());
    assert_eq!(n.galeria(0, 10, 7, 0).unwrap().total, 0);
    assert!(registro(&t.datos()).prorrogadas.is_empty(), "se olvida");

    // Otra con el mismo nombre va a la papelera con la hora delante.
    captura(&t.datos(), "b.png", 0);
    let otra = n.borrar_capturas(vec!["b.png".into()]).unwrap();
    assert!(!otra[0].destino.ends_with("/b.png"));

    assert_eq!(n.devolver_capturas(borradas).unwrap(), 2);
    assert_eq!(n.galeria(0, 10, 7, 0).unwrap().total, 2);
    assert_eq!(registro(&t.datos()).prorrogadas.get("a.png"), Some(&nueva));
    // Caso negativo: un destino fuera de la papelera no se mueve.
    let falsa = pixpin_nucleo::CapturaBorrada {
        nombre: "x.png".into(),
        destino: t.datos().join("indice.json").display().to_string(),
        conservada: false,
        prorroga: None,
    };
    assert_eq!(n.devolver_capturas(vec![falsa]).unwrap(), 0);
    // Y borrar lo que no esta es un error.
    assert!(n.borrar_capturas(vec!["no-esta.png".into()]).is_err());
}

fn momento(a: &Almacen, id: &str, cuando: i64, titulo: &str, en_momentos: bool) {
    let mut m = Momento::nuevo(id.into(), cuando, titulo.into(), String::new());
    m.en_momentos = en_momentos;
    m.fotos.push(format!("{id}-1.png"));
    m.fotos.push("../fuera.png".into());
    a.anadir(&m).unwrap();
}

#[test]
fn el_timeline_da_los_dias_y_sus_momentos_por_paginas() {
    let t = Temporal::nueva("timeline");
    let n = t.nucleo();
    assert_eq!(n.timeline_dias(0, false, 0, 10).unwrap().total, 0);
    let a = Almacen::en(&t.datos());
    // 3 de octubre de 2026 a las 10:00 UTC y dos el 5.
    let d3 = 20_729 * DIA_MS + 10 * 3_600_000;
    let d5 = d3 + 2 * DIA_MS;
    momento(&a, "tl-1-0", d3, "Llego el yeso", false);
    momento(&a, "tl-2-0", d5, "Reunión de obra", true);
    momento(&a, "tl-3-0", d5 + 60_000, "Comida", false);

    let dias = n.timeline_dias(0, false, 0, 10).unwrap();
    assert_eq!(dias.total, 2);
    assert_eq!(dias.dias[0].iso, "2026-10-05", "el mas nuevo primero");
    assert_eq!(dias.dias[0].cuantos, 2);
    assert_eq!(
        n.timeline_dias(0, false, 1, 10).unwrap().dias[0].iso,
        "2026-10-03"
    );
    assert_eq!(n.timeline_dias(0, true, 0, 10).unwrap().total, 1);
    // En Lima (-5 h) el de las 10:00 UTC sigue siendo del 3.
    let lima = -5 * 3_600_000;
    let m = n.timeline_momentos(2026, 10, 5, lima, false, 0, 1).unwrap();
    assert_eq!((m.total, m.momentos.len()), (2, 1));
    assert_eq!(m.momentos[0].titulo, "Reunión de obra");
    assert_eq!(m.momentos[0].hora, "05:00");
    assert_eq!(m.momentos[0].fotos.len(), 1, "un medio con rutas no sale");
    assert!(m.momentos[0].fotos[0].ends_with("timeline/medios/tl-2-0-1.png"));
    let segunda = n.timeline_momentos(2026, 10, 5, lima, false, 1, 5).unwrap();
    assert_eq!(segunda.momentos[0].titulo, "Comida");
    // Caso negativo: un dia que no existe.
    assert!(matches!(
        n.timeline_momentos(2026, 2, 30, 0, false, 0, 5),
        Err(ErrorNucleo::Argumento { .. })
    ));
    let hallados = n.timeline_buscar("REUNION".into(), 0, 10).unwrap();
    assert_eq!(hallados.len(), 1);

    // Lo escrito despues se ve (la cache mira la huella del fichero).
    momento(
        &a,
        "tl-4-0",
        d5 + 120_000,
        "Cierre del dia con mas texto",
        false,
    );
    assert_eq!(
        n.timeline_momentos(2026, 10, 5, 0, false, 0, 10)
            .unwrap()
            .total,
        3
    );
}

/// Una lista de tareas escrita en un chat como la escribe Windows.
fn lista(n: &Nucleo, datos: &Path, chat: &str, titulo: &str, tareas: &[mini::Tarea]) -> String {
    let doc = mini::escribir_tareas(titulo, tareas);
    let numero = n.ultimos_mensajes(chat.into(), 1).unwrap().total as i64 + 1;
    let m = Mensaje::miniapp(
        mini::TAREAS,
        titulo,
        &doc,
        &Sello {
            cuando: ahora() + numero,
            numero,
            aparato: "PC01".into(),
            proyecto: chat.into(),
        },
    );
    cuaderno::anadir(&almacen::carpeta(datos, chat), &m).unwrap();
    m.id
}

fn t(texto: &str, hecha: bool) -> mini::Tarea {
    mini::Tarea {
        texto: texto.into(),
        hecha,
    }
}

#[test]
fn las_tareas_de_todos_los_chats_se_marcan_en_su_mensaje() {
    let tmp = Temporal::nueva("tareas");
    let n = tmp.nucleo();
    let obra = n.crear_chat("Obra".into()).unwrap();
    n.escribir_nota(obra.id.clone(), "una nota que dice tareas".into(), None)
        .unwrap();
    let id = lista(
        &n,
        &tmp.datos(),
        &obra.id,
        "Compras",
        &[t("comprar yeso ➕ 2026-10-01", false), t("arena", true)],
    );
    let listas = n.tareas(0).unwrap();
    assert_eq!(listas.len(), 1);
    let l = &listas[0];
    assert_eq!(
        (l.titulo.as_str(), l.nombre_chat.as_str()),
        ("Compras", "Obra")
    );
    assert_eq!(l.pendientes, 1);
    assert_eq!(l.tareas[0].texto, "comprar yeso");
    assert_eq!(l.tareas[0].creada.as_deref(), Some("2026-10-01"));
    assert!(l.tareas[0].dias.is_some());

    // Marcar la reescribe con `alternar`, como la casilla del chat.
    let crudo = l.tareas[0].crudo.clone();
    assert!(
        n.marcar_tarea(obra.id.clone(), id.clone(), 0, crudo.clone(), true)
            .unwrap()
    );
    let doc = n
        .ultimos_mensajes(obra.id.clone(), 10)
        .unwrap()
        .mensajes
        .into_iter()
        .find(|m| m.id == id)
        .unwrap()
        .texto;
    assert_eq!(
        doc,
        "# Compras\n\n- [x] comprar yeso ➕ 2026-10-01\n- [x] arena"
    );
    // Repetirlo no deshace.
    assert!(
        n.marcar_tarea(obra.id.clone(), id.clone(), 0, crudo, true)
            .unwrap()
    );
    assert_eq!(n.tareas(0).unwrap()[0].pendientes, 0);
    // Caso negativo: si la tarea numero 1 ya no es la que se ve, no se toca.
    assert!(
        !n.marcar_tarea(obra.id.clone(), id.clone(), 1, "otra".into(), false)
            .unwrap()
    );
    assert!(matches!(
        n.marcar_tarea(obra.id.clone(), "no-esta".into(), 0, "x".into(), true),
        Err(ErrorNucleo::NoExiste { .. })
    ));

    // Apuntar va al Inbox de «Mensajes guardados», que se crea solo.
    let inbox = n.apuntar_tarea("llamar al fontanero".into(), 0).unwrap();
    assert!(inbox.es_inbox && inbox.guardados);
    assert_eq!(inbox.tareas.len(), 1);
    assert!(inbox.tareas[0].crudo.starts_with("llamar al fontanero ➕ "));
    let otra = n.apuntar_tarea("pagar luz".into(), 0).unwrap();
    assert_eq!(otra.mensaje, inbox.mensaje, "la misma lista");
    assert_eq!(otra.tareas.len(), 2);
    let listas = n.tareas(0).unwrap();
    assert!(listas[0].es_inbox, "el Inbox primero");
    assert!(n.apuntar_tarea("   ".into(), 0).is_err());

    // Lo que se entiende al escribir (Windows, 17-oct): prioridad y #etiqueta
    // van como marcas detras del titulo, y la vista las ensena.
    let con = n.apuntar_tarea("llamar al banco prioridad alta #casa".into(), 0).unwrap();
    let crudo = &con.tareas[2].crudo;
    assert!(crudo.starts_with("llamar al banco 🔼 #casa ➕ "), "{crudo}");
    let m = pixpin_nucleo::marcas_de_tarea(crudo.clone(), 0);
    assert_eq!(m.titulo, "llamar al banco");
    assert_eq!(m.urgencia.as_deref(), Some("urgente"));
    assert_eq!(m.etiquetas, ["casa"]);
    assert_eq!(m.marca_urgencia.as_deref(), Some("🔼"));
    assert_eq!(m.vista, "llamar al banco", "las #etiquetas van en chapitas aparte");
    // Caso negativo: lo que no se entiende se queda en el titulo.
    let raro = n.apuntar_tarea("comprar pan #".into(), 0).unwrap();
    assert!(raro.tareas[3].crudo.starts_with("comprar pan #"), "{}", raro.tareas[3].crudo);
}

#[test]
fn buscar_en_todo_da_chats_mensajes_tareas_y_momentos() {
    let tmp = Temporal::nueva("buscar");
    let n = tmp.nucleo();
    let obra = n.crear_chat("Obra del yeso".into()).unwrap();
    for i in 0..30 {
        n.escribir_nota(obra.id.clone(), format!("nota {i}"), None)
            .unwrap();
    }
    let nota = n
        .escribir_nota(obra.id.clone(), "Falta el YESO de la cocina".into(), None)
        .unwrap();
    n.escribir_nota(obra.id.clone(), "nada".into(), None)
        .unwrap();
    lista(
        &n,
        &tmp.datos(),
        &obra.id,
        "Compras",
        &[t("comprar yeso", false)],
    );
    let a = Almacen::en(&tmp.datos());
    momento(&a, "tl-9-0", ahora(), "Llego el yeso", false);

    let p = n.buscar("yeso".into(), 0, 50, None).unwrap();
    let clases: Vec<ClaseDeResultado> = p.resultados.iter().map(|r| r.clase).collect();
    assert_eq!(
        clases,
        [
            ClaseDeResultado::Chat,
            ClaseDeResultado::Tarea,
            ClaseDeResultado::Mensaje,
            ClaseDeResultado::Momento
        ]
    );
    let m = &p.resultados[2];
    assert_eq!(m.mensaje.as_deref(), Some(nota.id.as_str()));
    // La posicion lleva a la pagina donde esta el mensaje.
    let pos = m.posicion.unwrap();
    let pagina = n.mensajes(obra.id.clone(), pos, 1).unwrap();
    assert_eq!(pagina.mensajes[0].id, nota.id);
    assert_eq!(m.fragmento, "Falta el YESO de la cocina");
    assert!(!p.hay_mas);

    // Por paginas.
    let p1 = n.buscar("yeso".into(), 0, 2, None).unwrap();
    assert!(p1.hay_mas);
    let p2 = n.buscar("yeso".into(), 2, 2, None).unwrap();
    assert_eq!(p2.resultados[0].clase, ClaseDeResultado::Mensaje);
    assert!(!p2.hay_mas);
    // Cada palabra, sin tildes.
    assert_eq!(
        n.buscar("cocina falta".into(), 0, 10, None)
            .unwrap()
            .resultados
            .len(),
        1
    );
    // Caso negativo: sin palabras, nada; y cancelada antes de empezar.
    assert!(
        n.buscar("  ".into(), 0, 10, None)
            .unwrap()
            .resultados
            .is_empty()
    );
    let b = pixpin_nucleo::Busqueda::nueva();
    b.cancelar();
    let c = n.buscar("nota".into(), 0, 10, Some(b)).unwrap();
    assert!(c.cancelada);
    assert!(c.resultados.is_empty());
}

#[test]
fn editar_borrar_deshacer_y_mover_tareas() {
    let tmp = Temporal::nueva("cambiar-tareas");
    let n = tmp.nucleo();
    let inbox = n.apuntar_tarea("pagar luz".into(), 0).unwrap();
    n.apuntar_tarea("comprar pan".into(), 0).unwrap();
    let (chat, lista) = (inbox.chat.clone(), inbox.mensaje.clone());
    let leer = || {
        n.tareas(0)
            .unwrap()
            .into_iter()
            .find(|l| l.mensaje == lista)
            .unwrap()
            .tareas
    };
    let t = leer();
    // Corregir: el texto nuevo, entendido, con su fecha de creacion.
    assert!(n
        .editar_tarea(chat.clone(), lista.clone(), 0, t[0].crudo.clone(), "pagar la luz #casa".into(), 0)
        .unwrap());
    let t = leer();
    assert!(t[0].crudo.starts_with("pagar la luz #casa ➕ "), "{}", t[0].crudo);
    // Caso negativo: con el texto viejo ya no se toca.
    assert!(!n
        .editar_tarea(chat.clone(), lista.clone(), 0, "pagar luz".into(), "x".into(), 0)
        .unwrap());
    // Borrar y deshacer: vuelve a su sitio.
    let crudo = t[0].crudo.clone();
    assert!(n.borrar_tarea(chat.clone(), lista.clone(), 0, crudo.clone()).unwrap());
    assert_eq!(leer().len(), 1);
    n.devolver_tarea(chat.clone(), lista.clone(), 0, crudo.clone(), false).unwrap();
    assert_eq!(leer()[0].crudo, crudo);
    // Mover a una lista de otro chat: sale de aqui y entra alli.
    let obra = n.crear_chat("Obra".into()).unwrap();
    let otra = self::lista(&n, &tmp.datos(), &obra.id, "Compras", &[]);
    assert!(n
        .mover_tarea(chat.clone(), lista.clone(), 1, leer()[1].crudo.clone(), obra.id.clone(), otra.clone())
        .unwrap());
    assert_eq!(leer().len(), 1);
    let alli = n.tareas(0).unwrap().into_iter().find(|l| l.mensaje == otra).unwrap();
    assert!(alli.tareas[0].crudo.starts_with("comprar pan"));
    // Caso negativo: mover algo que ya no esta no deja copia en el destino.
    assert!(!n
        .mover_tarea(chat.clone(), lista.clone(), 0, "no esta".into(), obra.id.clone(), otra.clone())
        .unwrap());
    let alli = n.tareas(0).unwrap().into_iter().find(|l| l.mensaje == otra).unwrap();
    assert_eq!(alli.tareas.len(), 1);
}

#[test]
fn el_pin_de_una_tarea_la_encuentra_por_su_titulo_y_le_pone_la_nota() {
    let tmp = Temporal::nueva("pin-tarea");
    let n = tmp.nucleo();
    let l = n.apuntar_tarea("llamar al banco prioridad alta".into(), 0).unwrap();
    let (chat, lista) = (l.chat.clone(), l.mensaje.clone());
    let t = n.tarea_por_titulo(chat.clone(), lista.clone(), "llamar al banco".into()).unwrap().unwrap();
    assert_eq!(t.indice, 0);
    let crudo = n
        .poner_nota_tarea(chat.clone(), lista.clone(), "llamar al banco".into(), "no abren hasta el lunes".into())
        .unwrap()
        .unwrap();
    assert!(crudo.contains("🔼 💬 no abren hasta el lunes ➕ "), "{crudo}");
    let m = pixpin_nucleo::marcas_de_tarea(crudo, 0);
    assert_eq!(m.nota.as_deref(), Some("no abren hasta el lunes"));
    assert_eq!(m.titulo, "llamar al banco", "la nota no cambia el titulo");
    // Quitar la nota.
    let sin = n.poner_nota_tarea(chat.clone(), lista.clone(), "llamar al banco".into(), "".into()).unwrap().unwrap();
    assert!(!sin.contains('💬'));
    // Caso negativo: un titulo que no esta.
    assert!(n.tarea_por_titulo(chat.clone(), lista.clone(), "otra".into()).unwrap().is_none());
    assert!(n.poner_nota_tarea(chat, lista, "otra".into(), "x".into()).unwrap().is_none());
}

#[test]
fn el_horario_nace_con_la_plantilla_que_no_llama_y_despues_avisa() {
    let tmp = Temporal::nueva("horario");
    let n = tmp.nucleo();
    let ruta = n.fichero_del_horario().unwrap();
    assert!(std::fs::read_to_string(&ruta).unwrap().contains("# Horario de clases"));
    // La plantilla trae el ejemplo comentado: no llama.
    assert!(n.avisos_del_horario(0).is_empty());
    assert!(n.nota_de_voz_de_clase(0).is_none(), "sin clases no hay donde grabar");
    // Una clase todos los dias: siempre hay un aviso en los proximos dias.
    std::fs::write(&ruta, "Lun Mar Mie Jue Vie Sab Dom 9:00-10:00 Geometría @A-12\n> traer el compás\n").unwrap();
    let a = n.avisos_del_horario(0);
    assert!(a.len() >= 7, "{a:?}");
    assert_eq!(a[0].nombre, "Geometría");
    assert!(a[0].detalle.starts_with("09:00–10:00 · A-12"), "{}", a[0].detalle);
    assert_eq!(a[0].texto.as_deref(), Some("traer el compás"));
    assert!(a[0].audio.is_none(), "sin nota de voz grabada");
    // 10 min antes por omision.
    assert_eq!(a[0].cuando_utc_ms.rem_euclid(86_400_000), (8 * 60 + 50) * 60_000);
    let otra_vez = n.aviso_del_horario(a[0].id.clone(), 0).unwrap();
    assert_eq!(otra_vez.id, a[0].id);
    let nota = n.nota_de_voz_de_clase(0).unwrap();
    assert_eq!(nota.clase, "Geometría");
    assert!(nota.ruta.ends_with("horario/voz/geometria.m4a"), "{}", nota.ruta);
    assert!(n.aviso_del_horario("horario:clase:0:otra".into(), 0).is_none());
}

#[test]
fn un_proyecto_entero_se_adjunta_como_puerta_sin_copiarlo() {
    let tmp = Temporal::nueva("adj-proyecto");
    let n = tmp.nucleo();
    let a = n.crear_chat("Casa".into()).unwrap();
    let b = n.crear_chat("Obra".into()).unwrap();
    let m = n.adjuntar_proyecto(a.id.clone(), b.id.clone()).unwrap();
    assert_eq!(m.clase, "PROYECTO");
    assert_eq!(m.referencia.as_deref(), Some(b.id.as_str()));
    assert_eq!(m.nombre, "Obra");
    // Caso negativo: un proyecto que no esta.
    assert!(n.adjuntar_proyecto(a.id.clone(), "no-esta".into()).is_err());
}

#[test]
fn una_tarea_con_imagen_la_copia_al_chat_y_la_lleva_en_su_texto() {
    let tmp = Temporal::nueva("tarea-imagen");
    let n = tmp.nucleo();
    let foto = tmp.0.join("foto (1).png");
    std::fs::write(&foto, b"\x89PNG de mentira").unwrap();
    let l = n
        .apuntar_tarea_con_imagenes("revisar yeso".into(), vec![foto.display().to_string()], 0)
        .unwrap();
    let t = &l.tareas[0];
    assert!(t.crudo.contains("![img 01](pixpin:files/"), "{}", t.crudo);
    assert!(t.crudo.contains("/archivos/tarea-"), "{}", t.crudo);
    assert_eq!(t.imagenes.len(), 1, "se ve como imagen de la tarea");
    // Casos negativos: algo que no es imagen, y nada de nada.
    let txt = tmp.0.join("nota.txt");
    std::fs::write(&txt, b"x").unwrap();
    assert!(n.apuntar_tarea_con_imagenes("x".into(), vec![txt.display().to_string()], 0).is_err());
    assert!(n.apuntar_tarea_con_imagenes("  ".into(), vec![], 0).is_err());
}

#[test]
fn una_tarea_de_una_asignatura_va_a_su_lista_del_chat_asignaturas() {
    let tmp = Temporal::nueva("asignaturas");
    let n = tmp.nucleo();
    let l = n.apuntar_en_asignatura("Geometría".into(), "repasar ángulos".into(), vec![], 0).unwrap();
    assert_eq!(l.titulo, "Geometría");
    assert!(l.tareas[0].crudo.starts_with("repasar ángulos"));
    let otra = n.apuntar_en_asignatura("geometria".into(), "traer compás".into(), vec![], 0).unwrap();
    assert_eq!(otra.mensaje, l.mensaje, "la misma lista sin mirar tildes");
    let chats = n.chats().unwrap();
    assert_eq!(chats.iter().filter(|c| c.nombre == "Asignaturas").count(), 1);
    // Casos negativos.
    assert!(n.apuntar_en_asignatura("".into(), "x".into(), vec![], 0).is_err());
    assert!(n.apuntar_en_asignatura("Física".into(), " ".into(), vec![], 0).is_err());
}

#[test]
fn una_tarea_con_imagen_va_a_una_lista_de_un_proyecto() {
    let tmp = Temporal::nueva("tarea-imagen-lista");
    let n = tmp.nucleo();
    let obra = n.crear_chat("Obra".into()).unwrap();
    let id = lista(&n, &tmp.datos(), &obra.id, "Pendientes", &[t("medir", false)]);
    let foto = tmp.0.join("grieta.png");
    std::fs::write(&foto, b"\x89PNG de mentira").unwrap();
    let l = n
        .apuntar_tarea_con_imagenes_en(obra.id.clone(), id.clone(), "revisar grieta".into(), vec![foto.display().to_string()], 0)
        .unwrap();
    assert_eq!(l.mensaje, id, "en esa lista, no en el Inbox");
    assert_eq!(l.tareas.len(), 2);
    let t = &l.tareas[1];
    assert!(t.crudo.starts_with("revisar grieta"), "{}", t.crudo);
    assert_eq!(t.imagenes.len(), 1, "con su imagen");
    assert!(t.imagenes[0].contains(&obra.id) || t.crudo.contains("/archivos/tarea-"), "{:?}", t.imagenes);
    // Casos negativos: una lista que no esta y una tarea vacia.
    assert!(n.apuntar_tarea_con_imagenes_en(obra.id.clone(), "no-esta".into(), "x".into(), vec![], 0).is_err());
    assert!(n.apuntar_tarea_con_imagenes_en(obra.id.clone(), id, " ".into(), vec![], 0).is_err());
}
