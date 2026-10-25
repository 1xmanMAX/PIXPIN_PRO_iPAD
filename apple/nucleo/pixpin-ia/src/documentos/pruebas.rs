//! Pruebas del indice con un corpus sintetico: ficheros `.txt` en una
//! carpeta temporal, con `=== pagina N ===` para partir en paginas, y un
//! lector que cuenta lo que lee (para ver que la puesta al dia es de verdad
//! incremental).

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use super::*;

fn carpeta(nombre: &str) -> PathBuf {
    let c = std::env::temp_dir().join(format!("pixpin-documentos-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&c);
    std::fs::create_dir_all(&c).unwrap();
    c
}

/// Lee `.txt` con marcas de pagina; uno que empiece por `ROTO` falla.
#[derive(Default)]
struct LectorDePrueba {
    leidos: RefCell<Vec<String>>,
}

impl Lector for LectorDePrueba {
    fn leer(&self, f: &Fuente) -> Result<Vec<Unidad>, String> {
        self.leidos.borrow_mut().push(f.nombre.clone());
        let t = std::fs::read_to_string(&f.ruta).map_err(|e| e.to_string())?;
        if t.starts_with("ROTO") {
            return Err("el fichero esta danado".into());
        }
        let mut v = Vec::new();
        for (i, trozo) in t.split("=== pagina ===").enumerate() {
            v.push(Unidad {
                texto: trozo.to_string(),
                pagina: Some(i as u32 + 1),
                ..Default::default()
            });
        }
        Ok(v)
    }
}

fn fuente(dir: &Path, proyecto: &str, nombre: &str) -> Fuente {
    let ruta = dir.join(proyecto).join(nombre);
    Fuente {
        clave: format!("f:{}", ruta.display()),
        proyecto: proyecto.into(),
        proyecto_nombre: proyecto.to_uppercase(),
        nombre: nombre.into(),
        tipo: Tipo::Pdf,
        firma: Firma::de(&ruta).unwrap(),
        abrir: Abrir::Fichero {
            ruta: ruta.clone(),
            pagina: None,
        },
        ruta,
    }
}

fn escribir(dir: &Path, proyecto: &str, nombre: &str, texto: &str) {
    let d = dir.join(proyecto);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join(nombre), texto).unwrap();
}

/// Relleno que no casa con nada de lo que se pregunta.
fn relleno(n: usize) -> String {
    (0..n).map(|i| format!("palabra{} ", i % 97)).collect()
}

fn corpus(dir: &Path) -> Vec<Fuente> {
    escribir(
        dir,
        "obra",
        "Tesis.pdf",
        &format!(
            "Introduccion a la tesis. {}\n=== pagina ===\n{} El drenaje longitudinal se resuelve con cunetas \
             triangulares revestidas de concreto, con una pendiente minima del 0.5 por ciento. {}\n=== pagina ===\n\
             Conclusiones: el pavimento flexible dura mas con buen drenaje. {}",
            relleno(200),
            relleno(50),
            relleno(50),
            relleno(100)
        ),
    );
    escribir(dir, "obra", "Presupuesto.xlsx", "Partida 01 movimiento de tierras 12000 soles. Partida 02 obras de arte.");
    escribir(dir, "casa", "Receta.md", "Receta del pan: harina, agua, sal y levadura. Hornear 40 minutos.");
    escribir(dir, "carretera", "plano cunetas km 12.pdf", "Lamina 3 de 8. Escala 1:500.");
    vec![
        fuente(dir, "obra", "Tesis.pdf"),
        fuente(dir, "obra", "Presupuesto.xlsx"),
        fuente(dir, "casa", "Receta.md"),
        fuente(dir, "carretera", "plano cunetas km 12.pdf"),
    ]
}

#[test]
fn encuentra_lo_esperado_con_su_pagina() {
    let dir = carpeta("encuentra");
    let fuentes = corpus(&dir);
    let mut ix = Indice::nuevo(None);
    let inf = ix.poner_al_dia(&fuentes, &LectorDePrueba::default());
    assert_eq!(inf.leidas, 4);
    assert!(inf.rotas.is_empty());

    let r = ix.buscar("¿Qué dice la tesis sobre las cunetas?", &Alcance::Todo, 5);
    assert!(!r.is_empty());
    assert_eq!(r[0].nombre, "Tesis.pdf");
    assert_eq!(r[0].pagina, Some(2), "la cuneta esta en la pagina 2");
    assert!(r[0].fragmento.contains("cunetas"), "{}", r[0].fragmento);
    assert_eq!(
        r[0].abrir,
        Abrir::Fichero {
            ruta: dir.join("obra").join("Tesis.pdf"),
            pagina: Some(2)
        }
    );
    // El pedido es el del protocolo, con su pagina.
    let p = r[0].abrir.pedido();
    assert_eq!(p["accion"], "abrir");
    assert_eq!(p["que"]["tipo"], "fichero");
    assert_eq!(p["que"]["pagina"], 2);

    // Por el nombre del fichero: «¿donde mande el plano de cunetas?».
    let r = ix.buscar("donde mande el plano de cunetas", &Alcance::Todo, 5);
    assert_eq!(r[0].nombre, "plano cunetas km 12.pdf", "{r:#?}");

    // Otra cosa, en otro proyecto.
    let r = ix.buscar("receta del pan", &Alcance::Todo, 5);
    assert_eq!(r[0].nombre, "Receta.md");
    assert_eq!(r[0].proyecto_nombre, "CASA");

    // Lo que no esta, no sale.
    assert!(ix.buscar("helicoptero submarino", &Alcance::Todo, 5).is_empty());
}

#[test]
fn consulta_vacia_no_da_nada() {
    let dir = carpeta("vacia");
    let fuentes = corpus(&dir);
    let mut ix = Indice::nuevo(None);
    ix.poner_al_dia(&fuentes, &LectorDePrueba::default());
    assert!(ix.buscar("", &Alcance::Todo, 5).is_empty());
    assert!(ix.buscar("   ¿?  ", &Alcance::Todo, 5).is_empty());
    assert!(ix.buscar("de la que", &Alcance::Todo, 5).is_empty());
    // Y un indice vacio tampoco.
    assert!(Indice::nuevo(None).buscar("cunetas", &Alcance::Todo, 5).is_empty());
}

#[test]
fn un_proyecto_ajeno_no_se_ve() {
    let dir = carpeta("ajeno");
    let fuentes = corpus(&dir);
    let mut ix = Indice::nuevo(None);
    ix.poner_al_dia(&fuentes, &LectorDePrueba::default());
    // Desde el chat compartido «casa» no sale nada de «obra» aunque case.
    let r = ix.buscar("tesis cunetas drenaje", &Alcance::solo("casa"), 5);
    assert!(r.iter().all(|h| h.proyecto == "casa"), "{r:#?}");
    assert!(r.is_empty());
    let r = ix.buscar("pan", &Alcance::solo("casa"), 5);
    assert_eq!(r.len(), 1);
    // Ni por el empujon del nombre.
    let r = ix.buscar("tesis", &Alcance::solo("casa"), 5);
    assert!(r.is_empty(), "{r:#?}");
}

#[test]
fn la_puesta_al_dia_es_incremental_y_se_guarda() {
    let dir = carpeta("incremental");
    let cache = dir.join("cache");
    let fuentes = corpus(&dir);
    let lector = LectorDePrueba::default();
    let mut ix = Indice::cargar(&cache);
    assert_eq!(ix.tamano().0, 0);
    ix.poner_al_dia(&fuentes, &lector);
    assert_eq!(lector.leidos.borrow().len(), 4);

    // Otra vez, sin cambios: no se lee nada.
    lector.leidos.borrow_mut().clear();
    let inf = ix.poner_al_dia(&fuentes, &lector);
    assert_eq!(inf.leidas, 0);
    assert_eq!(inf.sin_cambios, 4);
    assert!(lector.leidos.borrow().is_empty());

    // Desde el disco, igual: lo leido sigue y no se relee.
    let mut ix = Indice::cargar(&cache);
    assert_eq!(ix.tamano().0, 4);
    let r = ix.buscar("cunetas pendiente", &Alcance::Todo, 5);
    assert_eq!(r[0].nombre, "Tesis.pdf");
    assert_eq!(r[0].pagina, Some(2));
    let inf = ix.poner_al_dia(&fuentes, &lector);
    assert_eq!(inf.leidas, 0);

    // Cambia uno: solo se lee ese, y se encuentra lo nuevo y no lo viejo.
    std::thread::sleep(std::time::Duration::from_millis(20));
    escribir(&dir, "casa", "Receta.md", "Receta de la sopa de quinua con zanahoria. Y una nota mas larga.");
    let fuentes = vec![
        fuente(&dir, "obra", "Tesis.pdf"),
        fuente(&dir, "obra", "Presupuesto.xlsx"),
        fuente(&dir, "casa", "Receta.md"),
        fuente(&dir, "carretera", "plano cunetas km 12.pdf"),
    ];
    let inf = ix.poner_al_dia(&fuentes, &lector);
    assert_eq!(inf.leidas, 1);
    assert_eq!(*lector.leidos.borrow(), vec!["Receta.md".to_string()]);
    assert!(ix.buscar("levadura", &Alcance::Todo, 5).is_empty());
    assert_eq!(ix.buscar("quinua", &Alcance::Todo, 5)[0].nombre, "Receta.md");

    // Se borra uno: sale del indice y del disco.
    let sin_plano: Vec<Fuente> = fuentes.iter().take(3).cloned().collect();
    let inf = ix.poner_al_dia(&sin_plano, &lector);
    assert_eq!(inf.quitadas, 1);
    assert!(ix.buscar("lamina escala", &Alcance::Todo, 5).is_empty());
    let ix = Indice::cargar(&cache);
    assert_eq!(ix.tamano().0, 3);
    assert!(ix.buscar("lamina escala", &Alcance::Todo, 5).is_empty());
    assert_eq!(ix.buscar("quinua", &Alcance::Todo, 5)[0].nombre, "Receta.md");
    assert_eq!(std::fs::read_dir(cache.join("f")).unwrap().count(), 3);
}

#[test]
fn un_fichero_roto_no_tumba_nada_y_no_se_reintenta() {
    let dir = carpeta("roto");
    let mut fuentes = corpus(&dir);
    escribir(&dir, "obra", "Danado.pdf", "ROTO y basura");
    fuentes.push(fuente(&dir, "obra", "Danado.pdf"));
    let cache = dir.join("cache");
    let lector = LectorDePrueba::default();
    let mut ix = Indice::cargar(&cache);
    let inf = ix.poner_al_dia(&fuentes, &lector);
    assert_eq!(inf.rotas.len(), 1);
    assert_eq!(inf.rotas[0].0, "Danado.pdf");
    // Lo demas se encuentra.
    let r = ix.buscar("cunetas", &Alcance::Todo, 5);
    assert_eq!(r[0].nombre, "Tesis.pdf", "{r:#?}");
    assert!(ix.buscar("basura", &Alcance::Todo, 5).is_empty());
    // Mientras no cambie, no se vuelve a intentar (ni tras cargar del disco).
    lector.leidos.borrow_mut().clear();
    let mut ix = Indice::cargar(&cache);
    assert_eq!(ix.poner_al_dia(&fuentes, &lector).leidas, 0);
    // Un fichero que ya no se puede ni abrir (borrado entre medias): error, sin panico.
    let mut fantasma = fuente(&dir, "obra", "Tesis.pdf");
    fantasma.clave = "f:fantasma".into();
    fantasma.ruta = dir.join("no-esta.pdf");
    let inf = ix.poner_al_dia(&[fantasma], &lector);
    assert_eq!(inf.rotas.len(), 1);
}

#[test]
fn una_cache_rota_se_rehace() {
    let dir = carpeta("cache-rota");
    let cache = dir.join("cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join("fuentes.json"), "{esto no es json").unwrap();
    let mut ix = Indice::cargar(&cache);
    assert_eq!(ix.tamano(), (0, 0));
    let fuentes = corpus(&dir);
    ix.poner_al_dia(&fuentes, &LectorDePrueba::default());
    // Falta el fichero de trozos de una: se vuelve a leer esa sola.
    let primero = std::fs::read_dir(cache.join("f")).unwrap().next().unwrap().unwrap().path();
    std::fs::remove_file(primero).unwrap();
    let lector = LectorDePrueba::default();
    let mut ix = Indice::cargar(&cache);
    assert_eq!(ix.tamano().0, 3);
    assert_eq!(ix.poner_al_dia(&fuentes, &lector).leidas, 1);
}

#[test]
fn los_mensajes_abren_su_mensaje_y_las_citas_se_numeran() {
    let dir = carpeta("mensajes");
    escribir(&dir, "obra", "guardados.jsonl", "x");
    struct Chat;
    impl Lector for Chat {
        fn leer(&self, _: &Fuente) -> Result<Vec<Unidad>, String> {
            Ok(vec![
                Unidad {
                    texto: "Mañana llega el camion con el cemento".into(),
                    abrir: Some(Abrir::Mensaje {
                        proyecto: Some("obra".into()),
                        codigo: "m1".into(),
                    }),
                    cuando: Some(1_790_985_600_000),
                    ..Default::default()
                },
                Unidad {
                    texto: "plano cunetas.pdf".into(),
                    seccion: Some("adjunto".into()),
                    abrir: Some(Abrir::Mensaje {
                        proyecto: Some("obra".into()),
                        codigo: "m2".into(),
                    }),
                    ..Default::default()
                },
            ])
        }
    }
    let ruta = dir.join("obra").join("guardados.jsonl");
    let f = Fuente {
        clave: "chat:obra".into(),
        proyecto: "obra".into(),
        proyecto_nombre: "Obra".into(),
        nombre: "Chat de Obra".into(),
        tipo: Tipo::Chat,
        firma: Firma::de(&ruta).unwrap(),
        abrir: Abrir::Mensaje {
            proyecto: Some("obra".into()),
            codigo: String::new(),
        },
        ruta,
    };
    let mut ix = Indice::nuevo(None);
    ix.poner_al_dia(&[f], &Chat);
    let r = ix.buscar("cemento", &Alcance::Todo, 5);
    assert_eq!(
        r[0].abrir,
        Abrir::Mensaje {
            proyecto: Some("obra".into()),
            codigo: "m1".into()
        }
    );
    let p = r[0].abrir.pedido();
    assert_eq!(p["que"]["tipo"], "mensaje");
    assert_eq!(p["que"]["codigo"], "m1");

    let mut citas = Citas::default();
    let texto = citas.anadir("cemento", r, None);
    assert!(texto.contains("[1] mensaje del chat «Obra», 3-oct-2026"), "{texto}");
    let r = ix.buscar("plano cunetas", &Alcance::Todo, 5);
    let texto = citas.anadir("plano cunetas", r, None);
    assert!(texto.contains("[2] El usuario SI mando el archivo «plano cunetas.pdf» al chat «Obra»."), "{texto}");
    assert_eq!(citas.citadas("Lo mandaste ahi [2].")[0].numero, 2);
    assert_eq!(citas.citadas("sin citar")[0].numero, 1);
    // Lo mismo otra vez conserva su numero.
    let r = ix.buscar("cemento camion", &Alcance::Todo, 5);
    assert!(citas.anadir("cemento", r, None).contains("[1]"));
    assert_eq!(citas.lista.len(), 2);
    // Nada: se dice que no hay, para que no invente.
    assert!(citas.anadir("helicoptero", vec![], None).contains("No encontre nada"));
}
