use super::*;
use crate::almacen::{Indice, poner_marca_asistente};

/// Una raiz con tres chats del asistente, como la del usuario el 21-oct: el
/// del PC (marcado, el mas antiguo), el del movil (`"asistente": true` en su
/// proyecto, con una foto) y el de otro PC (solo llamado «Asistente»). Y un
/// chat normal que no se toca.
fn montaje(nombre: &str) -> std::path::PathBuf {
    let raiz = std::env::temp_dir().join(format!(
        "pixpin-asistente-unico-{nombre}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&raiz);
    let mut pc = Ficha::nueva("Asistente", 100, "6ARJ");
    pc.id = "PCASIS".into();
    poner_marca_asistente(&mut pc);
    let mut movil = Ficha::nueva("Asistente", 200, "5NM3");
    movil.id = "pr-200".into();
    let mut otro = Ficha::nueva("Asistente", 300, "A256");
    otro.id = "OTROPC".into();
    let mut normal = Ficha::nueva("Tesis", 50, "6ARJ");
    normal.id = "TESIS".into();
    let mut indice = Indice::default();
    indice.proyectos.extend([pc, movil, otro, normal]);
    indice.guardar(&raiz).unwrap();
    let linea = |chat: &str,
                 id: &str,
                 cuando: i64,
                 numero: i64,
                 texto: &str,
                 aparato: &str,
                 ruta: Option<&str>| {
        let ruta = ruta.map_or("null".to_string(), |r| format!("\"{r}\""));
        format!(
            r#"{{"id":"{id}","cuando":{cuando},"clase":"NOTA","texto":"{texto}","ruta":{ruta},"numero":{numero},"aparato":"{aparato}","proyecto":"{chat}"}}"#
        )
    };
    let escribir = |id: &str, lineas: Vec<String>| {
        let c = almacen::carpeta(&raiz, id);
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(c.join("guardados.jsonl"), lineas.join("\n") + "\n").unwrap();
    };
    escribir(
        "PCASIS",
        vec![
            linea("PCASIS", "auto-figuras", 110, 1, "figuras", "6ARJ", None),
            linea("PCASIS", "u1", 1000, 2, "hola desde el PC", "6ARJ", None),
            linea("PCASIS", "r1", 1001, 3, "respuesta", "IA", None),
        ],
    );
    escribir(
        "pr-200",
        vec![
            linea("pr-200", "auto-figuras", 120, 1, "figuras", "6ARJ", None),
            linea(
                "pr-200",
                "u2",
                1500,
                1,
                "hola desde el movil",
                "5NM3",
                Some("pixpin:files/fotos/x.png"),
            ),
        ],
    );
    let movil_dir = almacen::carpeta(&raiz, "pr-200");
    std::fs::write(
        movil_dir.join("proyecto.json"),
        r#"{"id":"pr-200","nombre":"Asistente","asistente":true}"#,
    )
    .unwrap();
    std::fs::create_dir_all(movil_dir.join("android/fotos")).unwrap();
    std::fs::write(movil_dir.join("android/fotos/x.png"), b"png").unwrap();
    escribir(
        "OTROPC",
        vec![linea("OTROPC", "u3", 900, 1, "hola", "A256", None)],
    );
    escribir(
        "TESIS",
        vec![linea("TESIS", "t1", 10, 1, "capitulo 1", "6ARJ", None)],
    );
    raiz
}

#[test]
fn los_tres_chats_del_asistente_quedan_en_el_mas_antiguo_con_todo_en_orden() {
    let raiz = montaje("tres");
    let d = DiscoPc::nuevo(&raiz);
    assert_eq!(d.unificar_asistentes().unwrap(), 2);
    let chats: Vec<String> = d.mapa().into_iter().map(|(c, _)| c).collect();
    assert_eq!(chats, ["PCASIS", "TESIS"]);
    assert!(d.ficha_de("PCASIS").unwrap().es_asistente());
    let (m, _) = d.lineas("PCASIS").unwrap();
    let textos: Vec<&str> = m
        .iter()
        .map(|m| kotlin::cadena(m, "texto").unwrap())
        .collect();
    // El mensaje fijo del PC no se repite; lo demas, por fecha.
    assert_eq!(
        textos,
        [
            "figuras",
            "hola",
            "hola desde el PC",
            "respuesta",
            "hola desde el movil"
        ]
    );
    let numeros: HashSet<i64> = m
        .iter()
        .filter_map(|m| kotlin::numero(m, "numero"))
        .collect();
    assert_eq!(numeros.len(), m.len(), "sin numeros repetidos");
    // La foto del movil viene con su mensaje.
    assert!(
        almacen::carpeta(&raiz, "PCASIS")
            .join("android/fotos/x.png")
            .is_file()
    );
    // El chat normal, intacto.
    assert_eq!(d.lineas("TESIS").unwrap().0.len(), 1);
    // Y una segunda vez no hace nada.
    assert_eq!(d.unificar_asistentes().unwrap(), 0);
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn la_marca_viaja_en_el_proyecto_del_chat_del_asistente() {
    let raiz = montaje("marca");
    let d = DiscoPc::nuevo(&raiz);
    let p = d.proyecto_portatil("PCASIS").unwrap().unwrap();
    assert!(
        p.a_texto().contains("\"asistente\":true"),
        "{}",
        p.a_texto()
    );
    let t = d.proyecto_portatil("TESIS").unwrap().unwrap();
    assert!(!t.a_texto().contains("asistente"));
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn un_solo_chat_llamado_asistente_sin_marca_se_queda_con_ella() {
    // El de un PC de antes, solo: no hay nada que juntar, pero pasa a ser
    // el del asistente (y sale con la marca al movil).
    let raiz = std::env::temp_dir().join(format!(
        "pixpin-asistente-unico-solo-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&raiz);
    let mut f = Ficha::nueva("Asistente", 5, "A256");
    f.id = "SOLO".into();
    let mut indice = Indice::default();
    indice.proyectos.push(f);
    indice.guardar(&raiz).unwrap();
    let d = DiscoPc::nuevo(&raiz);
    assert_eq!(d.unificar_asistentes().unwrap(), 0);
    assert!(d.ficha_de("SOLO").unwrap().es_asistente());
    let _ = std::fs::remove_dir_all(&raiz);
}
