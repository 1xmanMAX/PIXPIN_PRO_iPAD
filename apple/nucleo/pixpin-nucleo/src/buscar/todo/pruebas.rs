//! Las pruebas de «Buscar en todo», con las de `buscar_todo.rs` de Windows
//! que tienen sentido sin su ventana (capturas al buscar, la caja vacia).

use std::path::PathBuf;
use std::sync::Arc;

use super::*;
use crate::lecciones::LeccionEditada;

fn nucleo(nombre: &str) -> (Arc<Nucleo>, PathBuf) {
    let raiz = std::env::temp_dir().join(format!(
        "pixpin-buscar-todo-{nombre}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(raiz.join("capturas")).unwrap();
    let n = Nucleo::abrir(raiz.display().to_string(), "Mac".into()).unwrap();
    (n, raiz)
}

fn opciones(consulta: &str) -> OpcionesBusqueda {
    OpcionesBusqueda {
        consulta: consulta.into(),
        clases: Vec::new(),
        tope: 50,
        dias_caducidad: 7,
        desfase_ms: 0,
    }
}

fn de_clase(r: &ResultadosEnTodo, c: ClaseDeResultado) -> Vec<&ResultadoBusqueda> {
    r.resultados.iter().filter(|x| x.clase == c).collect()
}

/// Un fichero adjuntado a `chat` con ese contenido.
fn adjuntar(n: &Nucleo, raiz: &std::path::Path, chat: &str, nombre: &str, datos: &[u8]) {
    let fuera = raiz.join("fuera");
    std::fs::create_dir_all(&fuera).unwrap();
    let origen = fuera.join(nombre);
    std::fs::write(&origen, datos).unwrap();
    n.adjuntar(chat.into(), origen.display().to_string(), None)
        .unwrap();
}

#[test]
fn al_buscar_en_todo_salen_tambien_las_capturas_por_su_nombre_y_su_texto() {
    let (n, raiz) = nucleo("capturas");
    let dir = raiz.join("capturas");
    std::fs::write(dir.join("grieta-muro.png"), b"no es un png de verdad").unwrap();
    std::fs::write(dir.join("otra.png"), b"x").unwrap();
    std::fs::write(dir.join("pantallazo.png"), b"xyz").unwrap();
    // El OCR de la galeria leyo «Grieta» dentro de otra.
    n.galeria_poner_texto(
        "pantallazo.png".into(),
        "Informe:\nla Grieta del pilar".into(),
    )
    .unwrap();
    let r = n.buscar_en_todo(opciones("grieta"), None).unwrap();
    let caps = de_clase(&r, ClaseDeResultado::Captura);
    let nombres: Vec<&str> = caps.iter().map(|c| c.titulo.as_str()).collect();
    assert_eq!(nombres, vec!["grieta-muro.png", "pantallazo.png"]);
    // La del nombre gana a la del texto, que dice donde esta.
    assert!(caps[0].puntos > caps[1].puntos);
    assert!(!caps[0].dentro && caps[1].dentro);
    assert_eq!(caps[1].fragmento, "la Grieta del pilar");
    assert_eq!(caps[0].resaltado, (0..6).collect::<Vec<u32>>());
    assert!(
        caps[0]
            .ruta
            .as_deref()
            .unwrap()
            .ends_with("grieta-muro.png")
    );
    assert_eq!(caps[0].clave, "captura/grieta-muro.png");
    // Caso negativo: solo en otras clases no salen capturas.
    let mut o = opciones("grieta");
    o.clases = vec![ClaseDeResultado::Chat];
    let r = n.buscar_en_todo(o, None).unwrap();
    assert!(de_clase(&r, ClaseDeResultado::Captura).is_empty());
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn con_la_caja_vacia_salen_los_abiertos_y_la_ultima_captura() {
    let (n, raiz) = nucleo("vacia");
    // Caso negativo: sin nada en disco, la caja vacia no inventa nada mas
    // que los chats que crea el nucleo al abrirse.
    assert!(
        n.buscar_inicio(7, 0)
            .iter()
            .all(|r| r.clase == ClaseDeResultado::Chat)
    );
    std::fs::write(raiz.join("capturas").join("ultima.png"), b"x").unwrap();
    let r = n.buscar_inicio(7, 0);
    assert_eq!(r.first().map(|r| r.titulo.as_str()), Some("ultima.png"));
    // En «Capturas» con la caja vacia: las capturas, como `p capturas`.
    let mut o = opciones("");
    o.clases = vec![ClaseDeResultado::Captura];
    let r = n.buscar_en_todo(o, None).unwrap();
    assert!(r.resultados.iter().any(|r| r.titulo == "ultima.png"));
    // Lo abierto antes va primero, y no se repite.
    let chat = n.crear_chat("Obra Miraflores".into()).unwrap();
    let mut o = opciones("miraflores");
    o.clases = vec![ClaseDeResultado::Chat];
    let elegido = n.buscar_en_todo(o, None).unwrap().resultados[0].clone();
    assert_eq!(elegido.chat.as_deref(), Some(chat.id.as_str()));
    n.buscar_apuntar("miraflores".into(), Some(elegido.clone()))
        .unwrap();
    let r = n.buscar_inicio(7, 0);
    assert_eq!(r[0].clave, elegido.clave);
    assert_eq!(r.iter().filter(|x| x.clave == elegido.clave).count(), 1);
    assert!(r.len() <= ABIERTOS_A_LA_VISTA);
    assert_eq!(n.buscar_recientes().busquedas, vec!["miraflores"]);
    n.buscar_quitar_busqueda(0).unwrap();
    assert!(n.buscar_recientes().busquedas.is_empty());
    n.buscar_apuntar("otra cosa".into(), None).unwrap();
    n.buscar_borrar_busquedas().unwrap();
    assert!(n.buscar_recientes().busquedas.is_empty());
    assert_eq!(n.buscar_recientes().abiertos.len(), 1, "lo abierto sigue");
    // Una captura abierta que ya no esta no sale.
    let mut o = opciones("ultima");
    o.clases = vec![ClaseDeResultado::Captura];
    let cap = n.buscar_en_todo(o, None).unwrap().resultados[0].clone();
    n.buscar_apuntar(String::new(), Some(cap.clone())).unwrap();
    std::fs::remove_file(raiz.join("capturas").join("ultima.png")).unwrap();
    assert!(n.buscar_inicio(7, 0).iter().all(|r| r.clave != cap.clave));
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn el_chat_que_se_llama_asi_gana_y_el_titulo_al_texto_de_dentro() {
    let (n, raiz) = nucleo("orden");
    let obra = n.crear_chat("Yeso".into()).unwrap();
    n.escribir_nota(
        obra.id.clone(),
        "comprar más arena\ny luego yeso".into(),
        None,
    )
    .unwrap();
    n.escribir_nota(obra.id.clone(), "Yeso de la cocina".into(), None)
        .unwrap();
    let r = n.buscar_en_todo(opciones("yeso"), None).unwrap();
    assert_eq!(r.resultados[0].clase, ClaseDeResultado::Chat);
    let notas = de_clase(&r, ClaseDeResultado::Mensaje);
    assert_eq!(notas.len(), 2);
    assert_eq!(notas[0].titulo, "Yeso de la cocina", "el titulo gana");
    assert!(notas[0].puntos > notas[1].puntos);
    assert_eq!(notas[1].fragmento, "y luego yeso");
    assert_eq!(notas[0].nombre_chat, "Yeso");
    assert!(notas[0].posicion.is_some());
    // Sin tildes: «mas arena» encuentra «más arena».
    let r = n.buscar_en_todo(opciones("mas arena"), None).unwrap();
    assert_eq!(de_clase(&r, ClaseDeResultado::Mensaje).len(), 1);
    // Caso negativo: lo que no esta no sale.
    let r = n.buscar_en_todo(opciones("cemento"), None).unwrap();
    assert!(r.resultados.is_empty());
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn las_tareas_se_buscan_y_sin_consulta_solo_las_pendientes() {
    let (n, raiz) = nucleo("tareas");
    n.apuntar_tarea("comprar yeso".into(), 0).unwrap();
    n.apuntar_tarea("llamar al ingeniero".into(), 0).unwrap();
    let r = n.buscar_en_todo(opciones("yeso"), None).unwrap();
    let t = de_clase(&r, ClaseDeResultado::Tarea);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].titulo, "comprar yeso");
    assert_eq!(t[0].fragmento, "Inbox");
    assert!(!t[0].crudo.is_empty());
    // La marca con lo que dice el resultado.
    let hecho = n
        .marcar_tarea(
            t[0].chat.clone().unwrap(),
            t[0].mensaje.clone().unwrap(),
            t[0].tarea.unwrap(),
            t[0].crudo.clone(),
            true,
        )
        .unwrap();
    assert!(hecho);
    let mut o = opciones("");
    o.clases = vec![ClaseDeResultado::Tarea];
    let r = n.buscar_en_todo(o, None).unwrap();
    let titulos: Vec<&str> = r.resultados.iter().map(|x| x.titulo.as_str()).collect();
    assert_eq!(titulos, vec!["llamar al ingeniero"]);
    // Caso negativo: buscandola si sale, ya hecha.
    let r = n.buscar_en_todo(opciones("yeso"), None).unwrap();
    assert!(de_clase(&r, ClaseDeResultado::Tarea)[0].hecha);
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn los_documentos_se_encuentran_por_lo_que_pone_dentro() {
    let (n, raiz) = nucleo("documentos");
    let obra = n.crear_chat("Obra".into()).unwrap();
    adjuntar(
        &n,
        &raiz,
        &obra.id,
        "acta.txt",
        "Acta\nla fisura del bloque B".as_bytes(),
    );
    adjuntar(
        &n,
        &raiz,
        &obra.id,
        "planos.pdf",
        b"%PDF-1.4 sin texto de verdad",
    );
    // Antes de leerlos solo se encuentran por su nombre.
    let r = n.buscar_en_todo(opciones("fisura"), None).unwrap();
    assert!(de_clase(&r, ClaseDeResultado::Fichero).is_empty());
    let r = n.buscar_en_todo(opciones("acta"), None).unwrap();
    assert_eq!(de_clase(&r, ClaseDeResultado::Fichero).len(), 1);
    // Rust lee el .txt; el PDF se lo pasa a Swift.
    let p = n.documentos_sin_texto(10).unwrap();
    assert_eq!(p.leidos, 1);
    assert_eq!(p.pdfs.len(), 1);
    assert_eq!(p.pdfs[0].nombre, "planos.pdf");
    let r = n.buscar_en_todo(opciones("fisura bloque"), None).unwrap();
    let f = de_clase(&r, ClaseDeResultado::Fichero);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].titulo, "acta.txt");
    assert!(f[0].dentro);
    assert_eq!(f[0].fragmento, "la fisura del bloque B");
    assert!(f[0].ruta.as_deref().unwrap().ends_with("acta.txt"));
    // Swift apunta lo que leyo PDFKit.
    n.poner_texto_de_documento(p.pdfs[0].ruta.clone(), "Planta baja: pilar P7".into())
        .unwrap();
    let r = n.buscar_en_todo(opciones("pilar"), None).unwrap();
    assert_eq!(
        de_clase(&r, ClaseDeResultado::Fichero)[0].titulo,
        "planos.pdf"
    );
    // Y ya no queda nada por leer.
    let p = n.documentos_sin_texto(10).unwrap();
    assert_eq!((p.leidos, p.pdfs.len()), (0, 0));
    // Caso negativo: no se apunta el texto de algo de fuera de los datos.
    assert!(
        n.poner_texto_de_documento("/etc/hosts".into(), "x".into())
            .is_err()
    );
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn las_lecciones_salen_como_lecciones_y_no_como_mensajes() {
    let (n, raiz) = nucleo("lecciones");
    let obra = n.crear_chat("Obra".into()).unwrap();
    let f = n
        .guardar_leccion(LeccionEditada {
            id: None,
            titulo: "Sellar las grietas antes de pintar".into(),
            que_paso: "La pintura se agrieto".into(),
            por_que: "No se sello".into(),
            proxima: "Sellar primero".into(),
            tipo: String::new(),
            area: String::new(),
            gravedad: 1,
            etiquetas: vec![],
            etiquetas_auto: vec![],
            quitadas: vec![],
            causas: vec![],
            chat: Some(obra.id.clone()),
            de_mensaje: None,
        })
        .unwrap();
    let r = n.buscar_en_todo(opciones("sellar grietas"), None).unwrap();
    let l = de_clase(&r, ClaseDeResultado::Leccion);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].leccion.as_deref(), Some(f.id.as_str()));
    assert_eq!(l[0].nombre_chat, "Obra");
    assert_eq!(l[0].clave, format!("leccion/{}", f.id));
    // Su mensaje en el chat no sale otra vez como mensaje ni fichero.
    assert!(de_clase(&r, ClaseDeResultado::Mensaje).is_empty());
    assert!(de_clase(&r, ClaseDeResultado::Fichero).is_empty());
    // Sin consulta, en «Lecciones», sale.
    let mut o = opciones("");
    o.clases = vec![ClaseDeResultado::Leccion];
    assert_eq!(n.buscar_en_todo(o, None).unwrap().resultados.len(), 1);
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn los_momentos_del_timeline_y_el_tope_de_cada_clase() {
    let (n, raiz) = nucleo("tope");
    n.timeline_apuntar(
        "Se fue la luz en la cocina".into(),
        vec![],
        None,
        0,
        None,
        None,
        0,
    )
    .unwrap();
    let r = n.buscar_en_todo(opciones("luz cocina"), None).unwrap();
    let m = de_clase(&r, ClaseDeResultado::Momento);
    assert_eq!(m.len(), 1);
    assert!(m[0].momento.is_some());
    let chat = n.crear_chat("Notas".into()).unwrap();
    for i in 0..12 {
        n.escribir_nota(chat.id.clone(), format!("nota {i}"), None)
            .unwrap();
    }
    let mut o = opciones("nota");
    o.tope = 5;
    let r = n.buscar_en_todo(o, None).unwrap();
    assert_eq!(de_clase(&r, ClaseDeResultado::Mensaje).len(), 5);
    assert_eq!(r.recortadas, vec![ClaseDeResultado::Mensaje]);
    let mut o = opciones("nota");
    o.tope = 50;
    let r = n.buscar_en_todo(o, None).unwrap();
    assert_eq!(de_clase(&r, ClaseDeResultado::Mensaje).len(), 12);
    assert!(r.recortadas.is_empty());
    // Una busqueda cancelada lo dice.
    let b = Busqueda::nueva();
    b.cancelar();
    let r = n.buscar_en_todo(opciones("nota"), Some(b)).unwrap();
    assert!(r.cancelada);
    let _ = std::fs::remove_dir_all(&raiz);
}
