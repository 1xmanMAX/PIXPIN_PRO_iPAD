//! **El PC y un movil de mentira, sincronizandose de verdad.**
//!
//! El «movil» es `DiscoAndroid`: `sincro/Disco.kt` portado sobre una carpeta
//! `files` como la del telefono, con los mensajes tal como los escribe
//! kotlinx. El PC es su almacen de siempre (`proyectos/indice.json`, una
//! carpeta por proyecto) visto por `vista::DiscoPc`. Se hablan por un socket
//! de esta maquina con el protocolo y el cifrado de verdad, en los dos
//! sentidos: el PC dirige y el movil responde, y al reves.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use pixpin_proyecto::almacen::{self, Ficha, Indice};
use pixpin_proyecto::cuaderno::{self, Clase, Cuaderno, Mensaje, Sello};
use pixpin_proyecto::vista::{self, DiscoPc};
use pixpin_sincro::canonico::{self, Json};
use pixpin_sincro::disco::{Disco, GENERAL};
use pixpin_sincro::disco_android::DiscoAndroid;
use pixpin_sincro::disco_android::prueba::{Reloj, conectado, crear_grupo, presentar, sincronizar};
use pixpin_sincro::kotlin;
use pixpin_sincro::protocolo::Hecho;
use serde_json::json;

struct Par {
    dir: PathBuf,
    pc: DiscoPc,
    movil: DiscoAndroid,
    reloj: Reloj,
}

impl Drop for Par {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn montar(etiqueta: &str) -> Par {
    let dir =
        std::env::temp_dir().join(format!("pixpin-pc-movil-{etiqueta}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let pc = DiscoPc::nuevo(&dir.join("PixPin Max"));
    let movil = DiscoAndroid::nuevo(dir.join("movil/files"), "movil");
    presentar(&pc, "id-pc", "Portátil");
    presentar(&movil, "id-movil", "Teléfono");
    let p = Par {
        dir,
        pc,
        movil,
        reloj: Reloj::nuevo(),
    };
    // El grupo lo crea el movil y el PC se une, como lo hizo el usuario.
    crear_grupo(&p.movil, "ABCDE23456", p.reloj.ahora());
    conectado(&p.pc, &p.movil, true, Some("ABCDE23456"), &p.reloj, |_| {}).unwrap();
    p
}

impl Par {
    fn raiz(&self) -> PathBuf {
        self.pc.raiz()
    }

    /// El PC dirige.
    fn desde_pc(&self, chats: &[&str]) -> Hecho {
        sincronizar(&self.pc, &self.movil, chats, &self.reloj, &|| {}).0
    }

    /// El movil dirige y el PC responde.
    fn desde_movil(&self, chats: &[&str]) -> Hecho {
        sincronizar(&self.movil, &self.pc, chats, &self.reloj, &|| {}).0
    }

    /// Una vuelta entera como la de la pantalla, sin preguntar.
    fn vuelta_desde_pc(&self) -> Hecho {
        let mut hecho = Hecho::default();
        conectado(&self.pc, &self.movil, false, None, &self.reloj, |s| {
            pixpin_sincro::vuelta::una(s, None, &mut hecho, "", &|| self.reloj.ahora(), &mut |_| {})
                .unwrap()
        })
        .unwrap();
        hecho
    }

    /// Lo mismo, devolviendo los avisos del final.
    fn vuelta_desde_pc_con_avisos(&self) -> Vec<String> {
        let mut hecho = Hecho::default();
        conectado(&self.pc, &self.movil, false, None, &self.reloj, |s| {
            pixpin_sincro::vuelta::una(s, None, &mut hecho, "", &|| self.reloj.ahora(), &mut |_| {})
                .unwrap()
                .avisos
        })
        .unwrap()
    }

    /// Una vuelta desde el PC contestando a «¿Que sincronizar?» con
    /// `marcar` (que filas se dejan marcadas) y «Lo mio manda» o no.
    fn vuelta_preguntando(
        &self,
        marcar: &dyn Fn(&pixpin_sincro::vuelta::Fila) -> bool,
        lo_mio_manda: bool,
    ) -> pixpin_sincro::vuelta::Vuelta {
        let mut hecho = Hecho::default();
        conectado(&self.pc, &self.movil, false, None, &self.reloj, |s| {
            let mut elegir = |_: &pixpin_sincro::mensajes::Aparato,
                              pr: pixpin_sincro::vuelta::Pregunta| {
                Some(pixpin_sincro::vuelta::Eleccion {
                    elegidos: pr
                        .filas
                        .iter()
                        .filter(|f| marcar(f))
                        .map(|f| f.id.clone())
                        .collect(),
                    lo_mio_manda,
                })
            };
            pixpin_sincro::vuelta::una(
                s,
                Some(&mut elegir),
                &mut hecho,
                "",
                &|| self.reloj.ahora(),
                &mut |_| {},
            )
            .unwrap()
        })
        .unwrap()
    }

    fn aparato_pc(&self) -> String {
        pixpin_proyecto::codigos::de_aparato("id-pc")
    }

    /// «Mensajes guardados» del PC.
    fn guardados(&self) -> Ficha {
        almacen::asegurar_guardados(&self.raiz(), 1, &self.aparato_pc()).unwrap()
    }

    /// Una nota escrita en el PC, como la escribe su chat.
    fn nota_pc(&self, ficha: &Ficha, texto: &str) -> Mensaje {
        let carpeta = almacen::carpeta(&self.raiz(), &ficha.id);
        let numero = Cuaderno::leer_de(&carpeta)
            .map(|c| c.mensajes.iter().map(|m| m.numero).max().unwrap_or(0))
            .unwrap_or(0)
            + 1;
        let m = Mensaje::nota(
            texto,
            &Sello {
                cuando: self.reloj.tic(),
                numero,
                aparato: self.aparato_pc(),
                proyecto: ficha.id.clone(),
            },
        );
        cuaderno::anadir(&carpeta, &m).unwrap();
        m
    }

    fn mensajes_pc(&self, ficha_id: &str) -> Vec<Mensaje> {
        Cuaderno::leer_de(&almacen::carpeta(&self.raiz(), ficha_id))
            .map(|c| c.mensajes)
            .unwrap_or_default()
    }

    fn nota_movil(&self, id: &str, texto: &str, proyecto: Option<&str>) {
        let numero = self
            .movil
            .leer_mensajes()
            .iter()
            .filter(|m| kotlin::cadena(m, "proyecto") == proyecto)
            .filter_map(|m| kotlin::numero(m, "numero"))
            .max()
            .unwrap_or(0)
            + 1;
        self.movil
            .anadir_mensaje(&Json::de_valor(&json!({
                "id": id, "cuando": self.reloj.tic(), "clase": "NOTA", "texto": texto,
                "proyecto": proyecto, "numero": numero, "letra": "a",
            })))
            .unwrap();
    }

    fn textos_movil(&self, chat: &str) -> BTreeSet<String> {
        self.movil
            .leer_mensajes()
            .iter()
            .filter(|m| kotlin::chat_de(m) == chat)
            .map(|m| kotlin::cadena(m, "texto").unwrap_or_default().to_string())
            .collect()
    }

    fn textos_pc(&self, ficha_id: &str) -> BTreeSet<String> {
        self.mensajes_pc(ficha_id)
            .into_iter()
            .map(|m| m.texto)
            .collect()
    }

    fn sello(&self, chat: &str) -> (String, String) {
        (
            self.pc.base("id-movil", chat).unwrap().sello(),
            self.movil.base("id-pc", chat).unwrap().sello(),
        )
    }

    /// Nada que hacer en una vuelta mas, dirija quien dirija.
    fn quieto(&self, chat: &str) {
        for pc_dirige in [true, false] {
            let comprobar = |s: &mut dyn FnMut(&str) -> (usize, usize)| {
                let (m, a) = s(chat);
                assert_eq!((m, a), (0, 0), "{chat}: mensajes y archivos quietos");
            };
            if pc_dirige {
                conectado(&self.pc, &self.movil, false, None, &self.reloj, |s| {
                    comprobar(&mut |c| {
                        let p = s.preparar(c).unwrap();
                        s.aplicar(&p, &mut Hecho::default()).unwrap();
                        (p.pasos.len(), s.preparar_archivos(&p).unwrap().pasos.len())
                    })
                })
                .unwrap();
            } else {
                conectado(&self.movil, &self.pc, false, None, &self.reloj, |s| {
                    comprobar(&mut |c| {
                        let p = s.preparar(c).unwrap();
                        s.aplicar(&p, &mut Hecho::default()).unwrap();
                        (p.pasos.len(), s.preparar_archivos(&p).unwrap().pasos.len())
                    })
                })
                .unwrap();
            }
        }
    }
}

fn gz(ruta: &Path, texto: &str) {
    std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
    let mut e = flate2::write::GzEncoder::new(
        std::fs::File::create(ruta).unwrap(),
        flate2::Compression::default(),
    );
    e.write_all(texto.as_bytes()).unwrap();
    e.finish().unwrap();
}

fn de_gz(ruta: &Path) -> String {
    let b = std::fs::read(ruta).unwrap();
    let mut s = String::new();
    flate2::read::GzDecoder::new(&b[..])
        .read_to_string(&mut s)
        .unwrap();
    s
}

/// Un proyecto nacido en el PC: una nota, una foto adjunta y un lienzo con
/// una imagen dentro, como los deja su chat.
fn proyecto_del_pc(p: &Par) -> Ficha {
    let raiz = p.raiz();
    let ficha = Ficha::nueva("Obra del PC", p.reloj.tic(), &p.aparato_pc());
    let mut i = Indice::leer(&raiz);
    i.proyectos.push(ficha.clone());
    i.guardar(&raiz).unwrap();
    p.nota_pc(&ficha, "medir la cocina");
    let ruta = almacen::guardar_adjunto(&raiz, &ficha.id, "captura.png", b"PNG del PC").unwrap();
    let carpeta = almacen::carpeta(&raiz, &ficha.id);
    let sello = Sello {
        cuando: p.reloj.tic(),
        numero: 2,
        aparato: p.aparato_pc(),
        proyecto: ficha.id.clone(),
    };
    cuaderno::anadir(
        &carpeta,
        &Mensaje::adjunto(Clase::Imagen, "captura.png", &ruta, 10, &sello),
    )
    .unwrap();
    std::fs::create_dir_all(carpeta.join("lienzos")).unwrap();
    std::fs::create_dir_all(carpeta.join("imagenes")).unwrap();
    std::fs::write(carpeta.join("imagenes/img1"), b"foto del lienzo").unwrap();
    std::fs::write(
        carpeta.join("lienzos/d9.excalidraw"),
        r#"{"type":"excalidraw","elements":[{"id":"A","type":"rectangle","x":1,"version":1,"versionNonce":3,"updated":1}],"files":{"img1":{"id":"img1","mimeType":"image/png","path":"imagenes/img1"}}}"#,
    )
    .unwrap();
    let mut dib = Mensaje::adjunto(
        Clase::Dibujo,
        "d9.excalidraw",
        "lienzos/d9.excalidraw",
        100,
        &Sello {
            cuando: p.reloj.tic(),
            numero: 3,
            ..sello
        },
    );
    dib.referencia = Some("d9".into());
    cuaderno::anadir(&carpeta, &dib).unwrap();
    ficha
}

/// Un proyecto del movil: un lienzo con su foto y un PDF adjunto.
fn proyecto_del_movil(p: &Par) {
    let m = &p.movil;
    let foto = m.raiz().join("pins/draw/files/foto1");
    std::fs::create_dir_all(foto.parent().unwrap()).unwrap();
    std::fs::write(&foto, [1, 2, 3]).unwrap();
    gz(
        &m.raiz().join("pins/draw/d1.excalidraw.gz"),
        &format!(
            r#"{{"elements":[{{"id":"A","type":"rectangle","x":0,"version":1,"versionNonce":7,"updated":1}}],"files":{{"foto1":{{"path":"{}"}}}}}}"#,
            m.absoluta("pins/draw/files/foto1")
        ),
    );
    let pdf = m.raiz().join("guardados/123_plano.pdf");
    std::fs::create_dir_all(pdf.parent().unwrap()).unwrap();
    std::fs::write(&pdf, b"%PDF-1.4 plano").unwrap();
    m.guardar_proyecto(
        &kotlin::normalizar_proyecto(&Json::de_valor(&json!({
            "id": "pr-1", "nombre": "Reforma", "tocado": 5,
            "hojas": [{"id": "h1", "nombre": "Planta", "dibujo": "d1"}],
            "uid": "RRRRRRRRRR", "creado": 1_726_000_000_000_i64, "aparato": "MOVI",
        })))
        .unwrap(),
    )
    .unwrap();
    for (id, clase, extra) in [
        (
            "m1",
            "DIBUJO",
            json!({"referencia": "d1", "nombre": "Planta"}),
        ),
        (
            "m2",
            "ARCHIVO",
            json!({"ruta": m.absoluta("guardados/123_plano.pdf"), "nombre": "plano.pdf"}),
        ),
    ] {
        let mut v = json!({"id": id, "cuando": p.reloj.tic(), "clase": clase, "proyecto": "pr-1",
                           "numero": if id == "m1" {1} else {2}, "letra": "a"});
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        m.anadir_mensaje(&Json::de_valor(&v)).unwrap();
    }
}

// ------------------------------------------------------------------ pruebas

#[test]
fn la_primera_vez_junta_todo_sin_pisar_y_la_segunda_no_mueve_nada() {
    let p = montar("primera");
    let g = p.guardados();
    p.nota_pc(&g, "del PC en guardados");
    p.nota_movil("n1", "del movil en la general", None);
    let obra = proyecto_del_pc(&p);
    proyecto_del_movil(&p);

    let hecho = p.vuelta_desde_pc();
    assert_eq!(hecho.fusionados, 0);

    // «Mensajes guardados» es la conversacion general del movil.
    assert_eq!(
        p.textos_movil(GENERAL),
        [
            "del PC en guardados".to_string(),
            "del movil en la general".into()
        ]
        .into()
    );
    assert_eq!(p.textos_pc(&g.id), p.textos_movil(GENERAL));

    // El proyecto del PC llega al movil con su nota, su foto y su lienzo.
    let chat_obra = vista::chat_de_ficha(&p.raiz(), &obra.id).unwrap();
    let del_movil = p
        .movil
        .leer_proyectos()
        .into_iter()
        .find(|x| kotlin::cadena(x, "id") == Some(chat_obra.as_str()))
        .expect("el proyecto del PC esta en el movil");
    assert_eq!(kotlin::cadena(&del_movil, "nombre"), Some("Obra del PC"));
    assert!(p.textos_movil(&chat_obra).contains("medir la cocina"));
    let captura = format!("guardados/pc/{chat_obra}/archivos/captura.png");
    assert_eq!(
        std::fs::read(p.movil.raiz().join(&captura)).unwrap(),
        b"PNG del PC"
    );
    let imagen = p
        .movil
        .leer_mensajes()
        .into_iter()
        .find(|m| kotlin::cadena(m, "clase") == Some("IMAGEN"))
        .unwrap();
    assert_eq!(
        kotlin::cadena(&imagen, "ruta"),
        Some(p.movil.absoluta(&captura).as_str())
    );
    let lienzo = de_gz(&p.movil.raiz().join("pins/draw/d9.excalidraw.gz"));
    let foto = format!("guardados/pc/{chat_obra}/imagenes/img1");
    assert!(lienzo.contains(&p.movil.absoluta(&foto)), "{lienzo}");
    assert_eq!(
        std::fs::read(p.movil.raiz().join(&foto)).unwrap(),
        b"foto del lienzo"
    );

    // El proyecto del movil llega al PC como una ficha mas, con su chat.
    let reforma = p.pc.ficha_de("pr-1").expect("ficha del proyecto del movil");
    assert_eq!(reforma.nombre, "Reforma");
    assert_eq!(reforma.id, "pr-1", "su carpeta se llama como su chat");
    let carpeta = almacen::carpeta(&p.raiz(), "pr-1");
    let lienzo = std::fs::read_to_string(carpeta.join("lienzos/d1.excalidraw")).unwrap();
    assert!(
        lienzo.contains("pixpin:files/pins/draw/files/foto1"),
        "{lienzo}"
    );
    assert_eq!(
        std::fs::read(carpeta.join("android/pins/draw/files/foto1")).unwrap(),
        [1, 2, 3]
    );
    let pdf = p
        .mensajes_pc("pr-1")
        .into_iter()
        .find(|m| m.clase == Some(Clase::Archivo))
        .unwrap();
    let real = vista::ruta_real(&p.raiz(), "pr-1", pdf.ruta.as_deref().unwrap()).unwrap();
    assert_eq!(std::fs::read(real).unwrap(), b"%PDF-1.4 plano");
    // El chat del PC lee el cuaderno como siempre.
    assert_eq!(p.mensajes_pc("pr-1").len(), 2);
    // Y no se inventan mensajes por sus hojas al abrirlo.
    assert_eq!(almacen::completar_hojas(&p.raiz(), "pr-1", "X").unwrap(), 0);

    // Lo acordado, con el mismo sello en los dos.
    for chat in [GENERAL, "pr-1", chat_obra.as_str()] {
        let (a, b) = p.sello(chat);
        assert_eq!(a, b, "{chat}");
        p.quieto(chat);
    }
}

#[test]
fn cuando_llama_el_movil_el_pc_responde_y_quedan_iguales() {
    let p = montar("llama-el-movil");
    let g = p.guardados();
    p.nota_pc(&g, "escrito en el PC");
    p.nota_movil("n1", "escrito en el movil", None);
    proyecto_del_movil(&p);
    p.desde_movil(&[GENERAL, "pr-1"]);
    assert_eq!(p.textos_pc(&g.id), p.textos_movil(GENERAL));
    assert_eq!(p.mensajes_pc("pr-1").len(), 2);
    assert!(
        almacen::carpeta(&p.raiz(), "pr-1")
            .join("lienzos/d1.excalidraw")
            .is_file()
    );
    let (a, b) = p.sello("pr-1");
    assert_eq!(a, b);
    p.quieto(GENERAL);
    p.quieto("pr-1");
}

#[test]
fn lo_cambiado_en_un_lado_pasa_y_en_los_dos_se_junta() {
    let p = montar("cambios");
    let g = p.guardados();
    let m = p.nota_pc(&g, "Introducción\n\nMétodo");
    p.desde_pc(&[GENERAL]);
    // Cambiado solo en el PC: pasa al movil, dirija quien dirija.
    let mut editado = m.clone();
    editado.texto = "Introducción\n\nMétodo corregido".into();
    cuaderno::reemplazar(&almacen::carpeta(&p.raiz(), &g.id), &editado).unwrap();
    assert_eq!(p.desde_movil(&[GENERAL]).fusionados, 0);
    assert_eq!(
        p.textos_movil(GENERAL),
        ["Introducción\n\nMétodo corregido".to_string()].into()
    );

    // Cambiado en los dos: se junta por parrafos.
    let mut en_pc = editado.clone();
    en_pc.texto = "Introducción ampliada\n\nMétodo corregido".into();
    cuaderno::reemplazar(&almacen::carpeta(&p.raiz(), &g.id), &en_pc).unwrap();
    let lista: Vec<Json> = p
        .movil
        .leer_mensajes()
        .into_iter()
        .map(|mut x| {
            kotlin::poner(
                &mut x,
                "texto",
                Json::cadena("Introducción\n\nMétodo corregido\n\nConclusiones"),
            );
            x
        })
        .collect();
    p.movil.escribir_mensajes(&lista).unwrap();
    assert_eq!(p.desde_pc(&[GENERAL]).fusionados, 1);
    let esperado = "Introducción ampliada\n\nMétodo corregido\n\nConclusiones".to_string();
    assert_eq!(p.textos_pc(&g.id), [esperado.clone()].into());
    assert_eq!(p.textos_movil(GENERAL), [esperado].into());
    p.quieto(GENERAL);
}

#[test]
fn lo_borrado_en_el_pc_deja_marca_y_se_borra_en_el_movil_sin_volver() {
    let p = montar("borrar-mensaje");
    let g = p.guardados();
    let a = p.nota_pc(&g, "para borrar");
    p.nota_pc(&g, "se queda");
    p.desde_pc(&[GENERAL]);
    // Como hace el chat del PC al borrar: fuera del cuaderno y su marca.
    let carpeta = almacen::carpeta(&p.raiz(), &g.id);
    let quedan: Vec<String> = std::fs::read_to_string(carpeta.join("guardados.jsonl"))
        .unwrap()
        .lines()
        .filter(|l| !l.contains("para borrar"))
        .map(str::to_string)
        .collect();
    std::fs::write(carpeta.join("guardados.jsonl"), quedan.join("\n") + "\n").unwrap();
    vista::anotar_borrados(&p.raiz(), &g.id, std::slice::from_ref(&a), p.reloj.tic()).unwrap();
    p.desde_movil(&[GENERAL]);
    assert_eq!(p.textos_movil(GENERAL), ["se queda".to_string()].into());
    p.desde_pc(&[GENERAL]);
    assert_eq!(
        p.textos_pc(&g.id),
        ["se queda".to_string()].into(),
        "no resucita"
    );
}

#[test]
fn un_proyecto_borrado_viaja_con_su_lapida_en_los_dos_sentidos() {
    let p = montar("lapidas");
    proyecto_del_movil(&p);
    let obra = proyecto_del_pc(&p);
    let chat_obra = vista::chat_de_ficha(&p.raiz(), &obra.id).unwrap();
    p.vuelta_desde_pc();
    assert_eq!(p.movil.leer_proyectos().len(), 2);

    p.reloj.saltar(1_000_000);
    // Borrado en el PC como lo borra su lista: a la papelera, con lapida.
    almacen::borrar_proyectos(&p.raiz(), std::slice::from_ref(&obra.id), p.reloj.ahora()).unwrap();
    let ids_movil = || -> Vec<String> {
        p.movil
            .leer_proyectos()
            .iter()
            .map(|x| kotlin::cadena(x, "id").unwrap().to_string())
            .collect()
    };
    // Sin preguntar no se borra en el movil: se avisa.
    let v = p.vuelta_desde_pc_con_avisos();
    assert_eq!(ids_movil().len(), 2, "sin preguntar no se borra alli");
    assert!(v.iter().any(|a| a.contains("Obra del PC")), "{v:?}");
    p.vuelta_preguntando(&|_| true, false);
    assert_eq!(
        ids_movil(),
        ["pr-1"],
        "{chat_obra} borrado tambien en el movil"
    );

    // Y al reves: borrado en el movil. Sin preguntar el PC NO lo borra.
    p.reloj.saltar(1_000_000);
    p.movil
        .borrar_chat("pr-1", "prueba", p.reloj.ahora(), "")
        .unwrap();
    p.vuelta_desde_pc();
    assert!(
        p.pc.ficha_de("pr-1").is_some(),
        "sin preguntar, sigue en el PC"
    );
    // Preguntando y sin tocar la casilla (empieza sin marcar): tampoco.
    p.vuelta_preguntando(&|f| f.marcado, false);
    assert!(
        p.pc.ficha_de("pr-1").is_some(),
        "desmarcado, sigue en el PC"
    );
    // Solo marcado se va, a la papelera.
    p.vuelta_preguntando(&|_| true, false);
    assert!(p.pc.ficha_de("pr-1").is_none());
    assert!(almacen::papelera(&p.raiz()).is_dir());
}

#[test]
fn con_lo_mio_manda_el_pc_no_borra_y_se_lo_vuelve_a_mandar_al_movil() {
    // El caso del usuario al reves: se vacio el movil y el PC lo rellena.
    let p = montar("lapida-mio-manda");
    proyecto_del_movil(&p);
    p.vuelta_desde_pc();
    p.reloj.saltar(1_000_000);
    p.movil
        .borrar_chat("pr-1", "prueba", p.reloj.ahora(), "")
        .unwrap();
    assert!(p.movil.leer_proyectos().is_empty());
    p.vuelta_preguntando(&|_| true, true);
    assert!(p.pc.ficha_de("pr-1").is_some(), "aqui no se borra nada");
    assert_eq!(p.movil.leer_proyectos().len(), 1, "y vuelve al movil");
    assert!(!p.textos_movil("pr-1").is_empty(), "con sus mensajes");
}

#[test]
fn un_proyecto_recuperado_de_la_papelera_vuelve_entero_y_la_vuelta_no_lo_borra_otra_vez() {
    // H6: borrado en el movil y aceptado aqui; luego se recupera de la
    // papelera. La siguiente vuelta tiene que devolverlo al movil con sus
    // mensajes, no volver a borrarlo ni quitarle los mensajes uno a uno.
    let p = montar("papelera-vuelve");
    proyecto_del_movil(&p);
    p.vuelta_desde_pc();
    let antes = p.textos_pc("pr-1");
    assert!(!antes.is_empty());
    p.reloj.saltar(1_000_000);
    p.movil
        .borrar_chat("pr-1", "prueba", p.reloj.ahora(), "")
        .unwrap();
    p.vuelta_preguntando(&|_| true, false);
    assert!(p.pc.ficha_de("pr-1").is_none(), "aceptado: se borro aqui");

    p.reloj.saltar(1_000_000);
    let en = almacen::en_papelera(&p.raiz());
    assert_eq!(en.len(), 1, "{en:?}");
    assert_eq!(en[0].ficha.nombre, "Reforma");
    almacen::recuperar(&p.raiz(), &en[0], p.reloj.ahora()).unwrap();
    assert!(p.pc.ficha_de("pr-1").is_some());
    assert_eq!(p.textos_pc("pr-1"), antes, "con todo lo que tenia");

    p.vuelta_desde_pc();
    assert!(
        p.pc.ficha_de("pr-1").is_some(),
        "la vuelta no lo vuelve a borrar"
    );
    assert_eq!(p.textos_pc("pr-1"), antes, "ni le quita mensajes");
    assert_eq!(p.movil.leer_proyectos().len(), 1, "y vuelve al movil");
    assert_eq!(p.textos_movil("pr-1"), antes, "entero");
}

#[test]
fn un_lienzo_cambiado_en_los_dos_se_junta_y_uno_grande_viaja_en_parche() {
    let p = montar("lienzos");
    proyecto_del_movil(&p);
    p.desde_pc(&["pr-1"]);
    let en_pc = almacen::carpeta(&p.raiz(), "pr-1").join("lienzos/d1.excalidraw");
    let texto = std::fs::read_to_string(&en_pc).unwrap();
    // El PC anade B; el movil anade C.
    let con =
        |t: &str, fig: &str| t.replacen("\"elements\":[", &format!("\"elements\":[{fig},"), 1);
    std::fs::write(
        &en_pc,
        con(
            &texto,
            r#"{"id":"B","type":"ellipse","x":5,"version":1,"versionNonce":1,"updated":2}"#,
        ),
    )
    .unwrap();
    let movil = p.movil.raiz().join("pins/draw/d1.excalidraw.gz");
    let del_movil = de_gz(&movil);
    gz(
        &movil,
        &con(
            &del_movil,
            r#"{"id":"C","type":"line","x":9,"version":1,"versionNonce":1,"updated":3}"#,
        ),
    );
    let hecho = p.desde_movil(&["pr-1"]);
    assert_eq!(hecho.fusionados, 1);
    let rel = "pins/draw/d1.excalidraw.gz";
    let a = p.pc.texto_de("pr-1", rel).unwrap();
    let b = p.movil.texto_de("pr-1", rel).unwrap();
    assert_eq!(canonico::de(&a), canonico::de(&b));
    for f in ["\"A\"", "\"B\"", "\"C\""] {
        assert!(a.contains(f), "{f} en {a}");
    }
    p.quieto("pr-1");

    // Uno grande cambiado en una figura: viaja solo el cambio.
    let muchas: Vec<String> = (0..400)
        .map(|i| format!(r#"{{"id":"f{i}","type":"rectangle","x":{i},"version":1,"versionNonce":1,"updated":1}}"#))
        .collect();
    std::fs::write(
        &en_pc,
        format!(r#"{{"elements":[{}],"files":{{}}}}"#, muchas.join(",")),
    )
    .unwrap();
    p.desde_pc(&["pr-1"]);
    let t = std::fs::read_to_string(&en_pc).unwrap();
    std::fs::write(
        &en_pc,
        t.replace(
            r#""id":"f7","type":"rectangle","x":7,"version":1"#,
            r#""id":"f7","type":"rectangle","x":777,"version":2"#,
        ),
    )
    .unwrap();
    let hecho = p.desde_pc(&["pr-1"]);
    assert!(hecho.ahorrados > 0, "{hecho:?}");
    assert!(de_gz(&movil).contains("777"));
}

#[test]
fn un_proyecto_del_movil_sale_identico_tras_recibirlo_guardarlo_y_mandarlo() {
    let p = montar("identico");
    proyecto_del_movil(&p);
    p.desde_movil(&["pr-1"]);
    let del_movil = p.movil.proyecto_portatil("pr-1").unwrap().unwrap();
    let del_pc = p.pc.proyecto_portatil("pr-1").unwrap().unwrap();
    assert_eq!(del_pc.a_texto(), del_movil.a_texto(), "byte a byte");
    // Y sus mensajes, con el mismo resumen que calcula el movil.
    let resumenes = |d: &dyn Fn() -> Vec<Json>| -> BTreeSet<String> {
        d().iter().map(kotlin::resumen_de).collect()
    };
    assert_eq!(
        resumenes(&|| p.pc.mensajes("pr-1").unwrap()),
        resumenes(&|| p.movil.mensajes("pr-1").unwrap())
    );
    let textos = |l: Vec<Json>| -> BTreeSet<String> { l.iter().map(Json::a_texto).collect() };
    assert_eq!(
        textos(p.pc.mensajes("pr-1").unwrap()),
        textos(p.movil.mensajes("pr-1").unwrap()),
        "cada mensaje, byte a byte"
    );
}

#[test]
fn un_mensaje_del_movil_da_en_el_pc_el_resumen_que_da_en_el_movil() {
    // El vector: la linea tal cual la escribe `Disco.JSON` en el movil, y el
    // resumen derivado a mano de las reglas de `Disco.resumenDe` (sin
    // recuerdaEn, uid ni aparato; canonico; SHA-256). No se puede ejecutar
    // Kotlin aqui: el vector es por razonamiento.
    let linea = r#"{"id":"1757939357123","cuando":1757939357123,"clase":"ARCHIVO","texto":"","ruta":"pixpin:files/guardados/1757939357123_plano.pdf","nombre":"plano.pdf","bytes":2048,"referencia":null,"pagina":null,"duracionMs":0,"picos":[],"miniapp":null,"proyecto":"pr-1","emoji":"📐","soloLaFoto":true,"fijado":true,"respondeA":null,"enBuzon":false,"unido":false,"transcripcion":null,"estadoDelTexto":null,"hojaDelTexto":null,"marcas":[],"turnos":[],"numero":3,"letra":null,"uid":"RVK5YHKCX7","aparato":"K7Q2","origen":null,"recibidoDe":null,"vieneDe":null,"recuerdaEn":null}"#;
    let canonico_a_mano = r#"{"bytes":2048,"clase":"ARCHIVO","cuando":1757939357123,"duracionMs":0,"emoji":"📐","enBuzon":false,"fijado":true,"id":"1757939357123","marcas":[],"nombre":"plano.pdf","numero":3,"picos":[],"proyecto":"pr-1","ruta":"pixpin:files/guardados/1757939357123_plano.pdf","soloLaFoto":true,"texto":"","turnos":[],"unido":false}"#;
    let esperado = canonico::sha256_hex(canonico_a_mano.as_bytes());

    let p = montar("vector");
    p.pc.aplicar_mensajes("pr-1", &[linea.to_string()], &[], 1)
        .unwrap();
    let en_pc = p.pc.mensajes("pr-1").unwrap();
    assert_eq!(en_pc.len(), 1);
    assert_eq!(kotlin::texto_de_base(&en_pc[0]), canonico_a_mano);
    assert_eq!(kotlin::resumen_de(&en_pc[0]), esperado);
    assert_eq!(en_pc[0].a_texto(), linea, "sale como entro");
    // Lo lee el chat del PC.
    let m = &p.mensajes_pc("pr-1")[0];
    assert_eq!(m.nombre, "plano.pdf");
    assert_eq!(m.codigo_chat().as_deref(), Some("3·K7Q2"));
}

/// Un lienzo dibujado en el PC, escrito por su motor: los puntos del trazo
/// como `[x, y]`, que es como los deja `excalidraw::escribir` en un lienzo
/// que no vino del movil.
fn lienzo_dibujado_en_el_pc() -> String {
    use pixpin_motor2d::elemento::{Elemento, Figura};
    use pixpin_motor2d::vector::Punto2;
    let mut escena = pixpin_motor2d::Escena::nueva();
    escena.anadir(Elemento {
        figura: Figura::Lapiz {
            puntos: (0..6)
                .map(|i| Punto2::nuevo(i as f32 * 4.0, i as f32 * 2.0))
                .collect(),
            presiones: Vec::new(),
            opciones: Some(Default::default()),
        },
        grosor: 3.0,
        ..Elemento::default()
    });
    let lienzo = pixpin_motor2d::excalidraw::con_escena(
        &pixpin_motor2d::excalidraw::Lienzo::vacio(),
        &escena,
    );
    pixpin_motor2d::excalidraw::escribir(&lienzo)
}

/// Los puntos de cada trazo son `Pt` (`{"x","y"}`), como los lee el movil.
fn puntos_del_movil(texto: &str) -> bool {
    let v: serde_json::Value = serde_json::from_str(texto).unwrap();
    v["elements"].as_array().unwrap().iter().all(|e| {
        e.get("points").is_none_or(|p| {
            p.as_array()
                .unwrap()
                .iter()
                .all(|q| q.get("x").is_some() && q.get("y").is_some())
        })
    })
}

#[test]
fn un_lienzo_dibujado_en_el_pc_llega_al_movil_con_puntos_que_su_scene_lee() {
    let p = montar("puntos");
    let obra = proyecto_del_pc(&p);
    let carpeta = almacen::carpeta(&p.raiz(), &obra.id);
    let texto = lienzo_dibujado_en_el_pc();
    assert!(
        !puntos_del_movil(&texto),
        "el PC escribe listas: es el fallo"
    );
    std::fs::write(carpeta.join("lienzos/d9.excalidraw"), &texto).unwrap();

    p.vuelta_desde_pc();

    let alli = de_gz(&p.movil.raiz().join("pins/draw/d9.excalidraw.gz"));
    assert!(puntos_del_movil(&alli), "{alli}");
    // En el PC el fichero no se reescribe: sigue como lo dejo su editor.
    assert_eq!(
        std::fs::read_to_string(carpeta.join("lienzos/d9.excalidraw")).unwrap(),
        texto
    );
    // Y la vuelta siguiente no lo manda otra vez: los dos lo ven igual.
    let chat = vista::chat_de_ficha(&p.raiz(), &obra.id).unwrap();
    p.quieto(&chat);
}

#[test]
fn un_lienzo_del_movil_vuelve_del_pc_sin_tocarle_un_byte() {
    // Caso negativo: el movil lo escribe ya con `Pt`, y el PC lo devuelve
    // igual aunque lo tenga guardado: otro texto seria otro resumen.
    let p = montar("puntos-movil");
    proyecto_del_movil(&p);
    let d1 = p.movil.raiz().join("pins/draw/d1.excalidraw.gz");
    let del_movil = r#"{"elements":[{"id":"A","type":"freedraw","x":0,"y":0,"width":1,"height":1,"seed":5,"version":1,"versionNonce":7,"updated":1,"points":[{"x":0.0,"y":0.0},{"x":1.0E-4,"y":1.0}]}],"files":{}}"#;
    gz(&d1, del_movil);
    p.desde_movil(&["pr-1"]);
    let rel = "pins/draw/d1.excalidraw.gz";
    assert_eq!(
        canonico::de(&p.pc.texto_de("pr-1", rel).unwrap()),
        canonico::de(&p.movil.texto_de("pr-1", rel).unwrap())
    );
    p.quieto("pr-1");
    assert_eq!(de_gz(&d1), del_movil, "el movil no recibe nada nuevo");
}

/// Una foto del chat del PC anotada en el PC: su lienzo `foto-<id>` lo crea
/// `lienzo_de_la_foto::asegurar` como el movil, con lo del `.pixpin2d` de
/// antes dentro. Datos inventados.
fn foto_anotada_en_el_pc(p: &Par) -> (Ficha, Mensaje, String) {
    let raiz = p.raiz();
    let ficha = Ficha::nueva("Fachada", p.reloj.tic(), &p.aparato_pc());
    let mut i = Indice::leer(&raiz);
    i.proyectos.push(ficha.clone());
    i.guardar(&raiz).unwrap();
    let jpg = [0xFF, 0xD8, 0xFF, 0xE0, 9, 9, 9, 9];
    let ruta = almacen::guardar_adjunto(&raiz, &ficha.id, "fachada.jpg", &jpg).unwrap();
    let carpeta = almacen::carpeta(&raiz, &ficha.id);
    let sello = Sello {
        cuando: p.reloj.tic(),
        numero: 1,
        aparato: p.aparato_pc(),
        proyecto: ficha.id.clone(),
    };
    let m = Mensaje::adjunto(
        Clase::Imagen,
        "fachada.jpg",
        &ruta,
        jpg.len() as i64,
        &sello,
    );
    cuaderno::anadir(&carpeta, &m).unwrap();
    // Lo que el PC dibujo antes, en pixeles de la foto (4000x3000).
    let foto = carpeta.join(&ruta);
    let mut escena = pixpin_motor2d::Escena::nueva();
    escena.anadir(pixpin_motor2d::Elemento {
        figura: pixpin_motor2d::Figura::Lapiz {
            puntos: (0..5)
                .map(|k| pixpin_motor2d::Punto2::nuevo(1000.0 + k as f32 * 100.0, 800.0))
                .collect(),
            presiones: Vec::new(),
            opciones: Some(Default::default()),
        },
        grosor: 6.0,
        ..pixpin_motor2d::Elemento::default()
    });
    let viejo = pixpin_proyecto::lienzo_de_la_foto::pixpin2d_de(&foto);
    pixpin_motor2d::guardar(&viejo, &escena).unwrap();
    let hecho = pixpin_proyecto::lienzo_de_la_foto::asegurar(
        &raiz,
        &ficha.id,
        &m,
        &foto,
        (4000, 3000),
        p.reloj.tic(),
    )
    .unwrap();
    assert!(hecho.creado && hecho.referencia_puesta && hecho.adoptados == 1);
    let m = p.mensajes_pc(&ficha.id).remove(0);
    (ficha, m, hecho.id)
}

#[test]
fn una_foto_anotada_en_el_pc_llega_al_movil_con_su_lienzo_su_foto_y_su_referencia() {
    let p = montar("foto-anotada");
    let (ficha, m, id) = foto_anotada_en_el_pc(&p);
    assert_eq!(id, format!("foto-{}", m.id));
    assert_eq!(m.referencia.as_deref(), Some(id.as_str()));

    p.vuelta_desde_pc();

    let chat = vista::chat_de_ficha(&p.raiz(), &ficha.id).unwrap();
    // El lienzo, donde el movil lo busca y en la forma que su `Scene` lee.
    let alli = de_gz(&p.movil.raiz().join(format!("pins/draw/{id}.excalidraw.gz")));
    assert!(puntos_del_movil(&alli), "{alli}");
    let v: serde_json::Value = serde_json::from_str(&alli).unwrap();
    let foto = &v["elements"][0];
    assert_eq!(foto["type"], "image");
    assert_eq!(foto["locked"], true);
    assert_eq!(
        foto["width"].as_f64(),
        Some(2000.0),
        "a su ancho del movil, no a sus pixeles"
    );
    assert_eq!(
        v["elements"].as_array().unwrap().len(),
        2,
        "la foto y la raya"
    );
    // Su foto viaja con el, y el lienzo la senala donde queda alli.
    let fichero = foto["fileId"].as_str().unwrap();
    let rel = format!("guardados/pc/{chat}/imagenes/{fichero}");
    assert_eq!(
        std::fs::read(p.movil.raiz().join(&rel)).unwrap(),
        [0xFF, 0xD8, 0xFF, 0xE0, 9, 9, 9, 9]
    );
    assert!(alli.contains(&p.movil.absoluta(&rel)), "{alli}");
    // Y el mensaje llega con su `referencia`: el movil abre ESE lienzo.
    let imagen = p
        .movil
        .leer_mensajes()
        .into_iter()
        .find(|x| kotlin::cadena(x, "id") == Some(m.id.as_str()))
        .expect("la foto esta en el movil");
    assert_eq!(kotlin::cadena(&imagen, "referencia"), Some(id.as_str()));
    // La vuelta siguiente no mueve nada.
    p.quieto(&chat);
}

#[test]
fn una_foto_sin_anotar_no_manda_ningun_lienzo() {
    // Caso negativo: sin lienzo en el PC, la foto viaja sola.
    let p = montar("foto-sin-anotar");
    let raiz = p.raiz();
    let ficha = Ficha::nueva("Solo foto", p.reloj.tic(), &p.aparato_pc());
    let mut i = Indice::leer(&raiz);
    i.proyectos.push(ficha.clone());
    i.guardar(&raiz).unwrap();
    let ruta = almacen::guardar_adjunto(&raiz, &ficha.id, "sola.jpg", &[0xFF, 0xD8, 1]).unwrap();
    let sello = Sello {
        cuando: p.reloj.tic(),
        numero: 1,
        aparato: p.aparato_pc(),
        proyecto: ficha.id.clone(),
    };
    let m = Mensaje::adjunto(Clase::Imagen, "sola.jpg", &ruta, 3, &sello);
    cuaderno::anadir(&almacen::carpeta(&raiz, &ficha.id), &m).unwrap();
    p.vuelta_desde_pc();
    assert!(
        !p.movil
            .raiz()
            .join(format!("pins/draw/foto-{}.excalidraw.gz", m.id))
            .exists()
    );
    let chat = vista::chat_de_ficha(&raiz, &ficha.id).unwrap();
    assert!(
        p.movil
            .raiz()
            .join(format!("guardados/pc/{chat}/{ruta}"))
            .is_file()
    );
}

// ------------------------------------------- lo anotado viaja (v0.96 del movil)

/// Un trazo del movil, con los puntos como `Pt`.
fn fig_movil(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","type":"freedraw","x":10.5,"y":20.0,"width":4,"height":4,"seed":3,"version":1,"versionNonce":1,"updated":1,"points":[{{"x":0.0,"y":0.0}},{{"x":4.0,"y":4.0}}]}}"#
    )
}

/// Un dibujo del movil en `pins/draw/<base>.excalidraw.gz` con esas figuras.
fn dibujo_movil(p: &Par, base: &str, figuras: &[&str]) {
    let figs: Vec<String> = figuras.iter().map(|f| fig_movil(f)).collect();
    gz(
        &p.movil
            .raiz()
            .join(format!("pins/draw/{base}.excalidraw.gz")),
        &format!(r#"{{"elements":[{}],"files":{{}}}}"#, figs.join(",")),
    );
}

/// Los ids de las figuras vivas de un dibujo (texto JSON).
fn figuras(texto: &str) -> BTreeSet<String> {
    let v: serde_json::Value = serde_json::from_str(texto).unwrap();
    v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["isDeleted"] != true)
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect()
}

/// Un adjunto del movil (`ARCHIVO`) en su chat general, como lo guarda su chat.
fn adjunto_movil(p: &Par, id: &str, fichero: &str, bytes: &[u8]) -> String {
    let rel = format!("guardados/{fichero}");
    let f = p.movil.raiz().join(&rel);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, bytes).unwrap();
    let m = Json::de_valor(&json!({
        "id": id, "cuando": p.reloj.tic(), "clase": "ARCHIVO", "ruta": p.movil.absoluta(&rel),
        "nombre": fichero, "numero": p.movil.leer_mensajes().len() + 1, "letra": "a",
    }));
    p.movil.anadir_mensaje(&m).unwrap();
    kotlin::unico(&m)
}

/// Escribe `texto` en un fichero del movil (`AnotacionesDelAdjunto.escribir`).
fn escribir_movil(p: &Par, rel: &str, texto: &str) {
    let f = p.movil.raiz().join(rel);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(f, texto).unwrap();
}

fn leer_movil(p: &Par, rel: &str) -> Option<String> {
    std::fs::read_to_string(p.movil.raiz().join(rel)).ok()
}

/// Deja la fecha de un fichero un minuto por delante: dos escrituras del
/// mismo largo en el mismo milisegundo se tomarian por la misma.
fn adelantar(f: &Path) {
    let t = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    std::fs::OpenOptions::new()
        .write(true)
        .open(f)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

/// Donde esta en el PC el adjunto del movil `id` (la ruta que su mensaje senala).
fn doc_en_pc(p: &Par, id: &str) -> PathBuf {
    let g = p.pc.ficha_de(GENERAL).unwrap();
    let m = p
        .mensajes_pc(&g.id)
        .into_iter()
        .find(|m| m.id == id)
        .unwrap();
    vista::ruta_real(&p.raiz(), &g.id, m.ruta.as_deref().unwrap()).unwrap()
}

#[test]
fn lo_anotado_sobre_un_pdf_un_word_y_un_libro_del_chat_viaja_con_el_codigo_del_mensaje() {
    use pixpin_proyecto::anotado;
    let p = montar("anotado");
    let u_pdf = adjunto_movil(&p, "p", "1_plano.pdf", b"%PDF-1.4");
    let u_docx = adjunto_movil(&p, "d", "2_informe.docx", b"PK docx");
    let u_epub = adjunto_movil(&p, "e", "3_libro.epub", b"PK epub");
    dibujo_movil(&p, &format!("anot-{u_pdf}-p2"), &["trazo-pdf"]);
    escribir_movil(
        &p,
        &format!("pins/draw/anot-{u_pdf}.marcas"),
        "m1:0.5:2.25:⭐",
    );
    escribir_movil(&p, &format!("pins/draw/anot-{u_pdf}.espacios"), "3");
    dibujo_movil(&p, &format!("anot-{u_docx}"), &["trazo-docx"]);
    escribir_movil(
        &p,
        &format!("pins/draw/anot-{u_docx}.maqueta"),
        "420,280,280,100,1,0",
    );
    dibujo_movil(&p, &format!("anot-{u_epub}"), &["trazo-epub"]);
    escribir_movil(&p, &format!("pins/draw/anot-{u_epub}.marcas"), "0.3:📌");
    // Un temporal a medias no viaja.
    escribir_movil(&p, &format!("pins/draw/anot-{u_pdf}.marcas.tmp"), "?");

    p.vuelta_desde_pc();

    // En el PC, el mismo mensaje con otra ruta da el mismo codigo: el lector
    // encuentra lo anotado por el documento que tiene delante.
    let raiz = p.raiz();
    let (pdf, docx, epub) = (doc_en_pc(&p, "p"), doc_en_pc(&p, "d"), doc_en_pc(&p, "e"));
    assert_eq!(anotado::adjunto_de(&raiz, &pdf).unwrap().uid, u_pdf);
    let hoja = anotado::hoja_del_pdf(&raiz, &pdf, 2).unwrap();
    assert_eq!(
        figuras(&std::fs::read_to_string(&hoja).unwrap()),
        ["trazo-pdf".to_string()].into()
    );
    let b_pdf = anotado::base_del_pdf(&raiz, &pdf, None).unwrap();
    assert_eq!(
        anotado::leer(&b_pdf.fichero(".marcas")).as_deref(),
        Some("m1:0.5:2.25:⭐")
    );
    assert_eq!(
        anotado::leer(&b_pdf.fichero(".espacios")).as_deref(),
        Some("3")
    );
    assert!(
        !b_pdf.fichero(".marcas.tmp").exists(),
        "el temporal no viaja"
    );
    let b_docx = anotado::base_del_documento(&raiz, &docx).unwrap();
    assert_eq!(
        figuras(&std::fs::read_to_string(b_docx.tinta()).unwrap()),
        ["trazo-docx".to_string()].into()
    );
    assert_eq!(
        pixpin_sincro::anotado::Maqueta::de_texto(
            &anotado::leer(&b_docx.fichero(".maqueta")).unwrap()
        ),
        pixpin_sincro::anotado::Maqueta::de_texto("420,280,280,100,1,0")
    );
    let b_epub = anotado::base_del_documento(&raiz, &epub).unwrap();
    assert_eq!(
        figuras(&std::fs::read_to_string(b_epub.tinta()).unwrap()),
        ["trazo-epub".to_string()].into()
    );
    assert_eq!(
        anotado::leer(&b_epub.fichero(".marcas")).as_deref(),
        Some("0.3:📌")
    );

    // Y de vuelta: lo que se cambia en el PC llega al movil con el mismo
    // nombre y en la forma que su `Scene` lee.
    p.reloj.saltar(10_000);
    anotado::escribir(&b_pdf.fichero(".espacios"), "1").unwrap();
    adelantar(&b_pdf.fichero(".espacios"));
    let con_otro = std::fs::read_to_string(&hoja).unwrap().replacen(
        "\"elements\":[",
        r#""elements":[{"id":"otro","type":"freedraw","x":1,"y":1,"width":2,"height":2,"seed":9,"version":1,"versionNonce":2,"updated":5,"points":[[0,0],[2,2]]},"#,
        1,
    );
    std::fs::write(&hoja, con_otro).unwrap();
    adelantar(&hoja);
    p.vuelta_desde_pc();
    assert_eq!(
        leer_movil(&p, &format!("pins/draw/anot-{u_pdf}.espacios")).as_deref(),
        Some("1")
    );
    let alli = de_gz(
        &p.movil
            .raiz()
            .join(format!("pins/draw/anot-{u_pdf}-p2.excalidraw.gz")),
    );
    assert_eq!(
        figuras(&alli),
        ["trazo-pdf".to_string(), "otro".into()].into()
    );
    assert!(puntos_del_movil(&alli), "{alli}");
    p.quieto(GENERAL);
}

#[test]
fn un_word_anotado_en_el_pc_llega_al_movil_con_los_nombres_y_textos_exactos() {
    use pixpin_proyecto::anotado;
    let p = montar("anotado-pc");
    let g = p.guardados();
    let raiz = p.raiz();
    let ruta = almacen::guardar_adjunto(&raiz, &g.id, "acta.docx", b"PK acta").unwrap();
    let sello = Sello {
        cuando: p.reloj.tic(),
        numero: 1,
        aparato: p.aparato_pc(),
        proyecto: g.id.clone(),
    };
    let m = Mensaje::adjunto(Clase::Archivo, "acta.docx", &ruta, 7, &sello);
    let carpeta = almacen::carpeta(&raiz, &g.id);
    cuaderno::anadir(&carpeta, &m).unwrap();
    let doc = carpeta.join(&ruta);
    let b = anotado::base_del_documento(&raiz, &doc).unwrap();
    // Lo que deja el lector del PC: tinta con puntos `[x, y]`, maqueta,
    // marcadores y sitio.
    std::fs::create_dir_all(b.tinta().parent().unwrap()).unwrap();
    std::fs::write(b.tinta(), lienzo_dibujado_en_el_pc()).unwrap();
    anotado::escribir(&b.fichero(".maqueta"), "860,573,573,100,1,1").unwrap();
    anotado::escribir(&b.fichero(".marcas"), "1790218080412:0.34022403:📌").unwrap();
    anotado::escribir(&b.fichero(".sitio"), "0.6239229").unwrap();
    // Caso negativo: lo de un documento que no es del chat se queda aqui.
    std::fs::write(
        carpeta.join("archivos/suelto.docx.pixpin-lectura"),
        "marcadores 1:0.1:⭐",
    )
    .unwrap();

    p.vuelta_desde_pc();

    let base = &b.base;
    assert!(
        base.starts_with("anot-") && base.len() == "anot-".len() + 10,
        "{base}"
    );
    let alli = de_gz(
        &p.movil
            .raiz()
            .join(format!("pins/draw/{base}.excalidraw.gz")),
    );
    assert!(puntos_del_movil(&alli), "{alli}");
    for (t, esperado) in [
        (".maqueta", "860,573,573,100,1,1"),
        (".marcas", "1790218080412:0.34022403:📌"),
        (".sitio", "0.6239229"),
    ] {
        assert_eq!(
            leer_movil(&p, &format!("pins/draw/{base}{t}")).as_deref(),
            Some(esperado),
            "{t}"
        );
    }
    let nombres: Vec<String> = std::fs::read_dir(p.movil.raiz().join("pins/draw"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        nombres.iter().all(|n| !n.contains("pixpin-lectura")),
        "{nombres:?}"
    );
    // Y el movil encuentra el mensaje por su codigo.
    let alla = p
        .movil
        .leer_mensajes()
        .into_iter()
        .find(|x| kotlin::cadena(x, "id") == Some(m.id.as_str()))
        .unwrap();
    assert_eq!(format!("anot-{}", kotlin::unico(&alla)), *base);
    p.quieto(GENERAL);
}

#[test]
fn los_marcadores_de_un_lienzo_la_voz_el_sitio_y_los_del_pdf_de_un_proyecto_tambien_viajan() {
    use pixpin_proyecto::anotado;
    let p = montar("anotado-lienzo");
    // Un lienzo del movil con sus marcadores al lado.
    proyecto_del_movil(&p);
    escribir_movil(&p, "pins/draw/d1.marcas", "k:10.0:20.0:🏠");
    // Un Word con el verde de la voz y el punto de lectura.
    let u_w = adjunto_movil(&p, "w", "4_acta.docx", b"PK");
    escribir_movil(&p, &format!("pins/draw/anot-{u_w}.voz"), "12:0.4");
    escribir_movil(&p, &format!("pins/draw/anot-{u_w}.sitio"), "0.37");
    p.vuelta_desde_pc();
    let raiz = p.raiz();
    let lienzo = almacen::lienzo(&raiz, "pr-1", "d1");
    let marcas = anotado::marcas_del_lienzo(&lienzo).unwrap();
    assert_eq!(anotado::leer(&marcas).as_deref(), Some("k:10.0:20.0:🏠"));
    let b = anotado::base_del_documento(&raiz, &doc_en_pc(&p, "w")).unwrap();
    assert_eq!(anotado::leer(&b.fichero(".voz")).as_deref(), Some("12:0.4"));
    assert_eq!(anotado::leer(&b.fichero(".sitio")).as_deref(), Some("0.37"));

    // El PDF de un proyecto: marcadores y espacios con el codigo del proyecto.
    let u_p = kotlin::unico_de_proyecto(&p.movil.proyecto_portatil("pr-1").unwrap().unwrap());
    assert_eq!(u_p, "RRRRRRRRRR");
    escribir_movil(&p, &format!("pins/draw/anot-{u_p}.marcas"), "m:0.1:0.5:⭐");
    escribir_movil(&p, &format!("pins/draw/anot-{u_p}.espacios"), "2");
    p.desde_movil(&["pr-1"]);
    let bp = anotado::base_del_pdf(&raiz, Path::new("no-hace-falta.pdf"), Some("pr-1")).unwrap();
    assert_eq!(bp.base, format!("anot-{u_p}"));
    assert_eq!(
        anotado::leer(&bp.fichero(".marcas")).as_deref(),
        Some("m:0.1:0.5:⭐")
    );
    assert_eq!(
        anotado::leer(&bp.fichero(".espacios")).as_deref(),
        Some("2")
    );

    // Y al reves: marcadores puestos en el lienzo del PC llegan al movil.
    p.reloj.saltar(10_000);
    anotado::escribir(&marcas, "k:10.0:20.0:🏠|z:5.0:6.0:⭐").unwrap();
    adelantar(&marcas);
    p.vuelta_desde_pc();
    assert_eq!(
        leer_movil(&p, "pins/draw/d1.marcas").as_deref(),
        Some("k:10.0:20.0:🏠|z:5.0:6.0:⭐")
    );
    p.quieto("pr-1");
    p.quieto(GENERAL);
}

/// 25-oct-2026: la vista guardada con cada marcador de un lienzo (como las
/// vistas con nombre de AutoCAD) va y vuelve con sus marcas, y quitarlas
/// todas deja `{}` en los dos lados (no hay lapidas: un fichero borrado
/// volveria).
#[test]
fn las_vistas_de_los_marcadores_de_un_lienzo_viajan_con_sus_marcas() {
    use pixpin_proyecto::anotado;
    let p = montar("vistas-lienzo");
    proyecto_del_movil(&p);
    escribir_movil(&p, "pins/draw/d1.marcas", "7:10:20:⭐");
    escribir_movil(&p, "pins/draw/d1.vistas.json", r#"{"7":"10 20 640 360"}"#);
    p.vuelta_desde_pc();
    let lienzo = almacen::lienzo(&p.raiz(), "pr-1", "d1");
    let vistas = anotado::vistas_del_lienzo(&lienzo).unwrap();
    assert_eq!(
        vistas.parent(),
        anotado::marcas_del_lienzo(&lienzo).unwrap().parent(),
        "al lado de las marcas"
    );
    assert_eq!(
        anotado::leer(&vistas).as_deref(),
        Some(r#"{"7":"10 20 640 360"}"#)
    );

    // Del PC al movil: se pone otra marca con su vista.
    p.reloj.saltar(10_000);
    let otra = r#"{"7":"10 20 640 360","8":"-5 6 200 112.5"}"#;
    anotado::escribir(&vistas, otra).unwrap();
    adelantar(&vistas);
    p.vuelta_desde_pc();
    assert_eq!(
        leer_movil(&p, "pins/draw/d1.vistas.json").as_deref(),
        Some(otra)
    );

    // Vaciar es escribir `{}`, y el `{}` tambien viaja.
    p.reloj.saltar(10_000);
    anotado::escribir(&vistas, "{}").unwrap();
    adelantar(&vistas);
    p.vuelta_desde_pc();
    assert_eq!(
        leer_movil(&p, "pins/draw/d1.vistas.json").as_deref(),
        Some("{}")
    );
    p.quieto("pr-1");
    p.quieto(GENERAL);
}

#[test]
fn borrar_el_mensaje_se_lleva_lo_anotado_en_los_dos_aparatos() {
    use pixpin_proyecto::anotado;
    let p = montar("anotado-borrar");
    let u = adjunto_movil(&p, "p", "5_libro.pdf", b"%PDF");
    let u_queda = adjunto_movil(&p, "q", "6_otro.pdf", b"%PDF-2");
    dibujo_movil(&p, &format!("anot-{u}-p0"), &["t"]);
    escribir_movil(&p, &format!("pins/draw/anot-{u}.marcas"), "x:0:0:⭐");
    escribir_movil(&p, &format!("pins/draw/anot-{u_queda}.espacios"), "1");
    p.vuelta_desde_pc();
    let raiz = p.raiz();
    let pdf = doc_en_pc(&p, "p");
    let hoja = anotado::hoja_del_pdf(&raiz, &pdf, 0).unwrap();
    let marcas = anotado::base_del_pdf(&raiz, &pdf, None)
        .unwrap()
        .fichero(".marcas");
    assert!(hoja.is_file() && marcas.is_file());

    // Borrado en el movil (lo que hace su chat): fuera el mensaje, su marca
    // y lo anotado.
    let (ido, quedan): (Vec<Json>, Vec<Json>) = p
        .movil
        .leer_mensajes()
        .into_iter()
        .partition(|m| kotlin::cadena(m, "id") == Some("p"));
    p.movil.escribir_mensajes(&quedan).unwrap();
    p.movil
        .anotar_borrados(&pixpin_sincro::disco::marcas_de(
            GENERAL,
            &ido,
            p.reloj.tic(),
        ))
        .unwrap();
    p.movil.borrar_anotado(GENERAL, &u);
    p.vuelta_desde_pc();
    assert!(!hoja.exists() && !marcas.exists(), "se fue con su mensaje");
    // Caso negativo: lo del otro mensaje sigue.
    let otro = anotado::base_del_pdf(&raiz, &doc_en_pc(&p, "q"), None).unwrap();
    assert_eq!(
        anotado::leer(&otro.fichero(".espacios")).as_deref(),
        Some("1")
    );

    // Y borrado en el PC, como lo borra su chat: llega al movil y se lleva lo suyo.
    let g = p.pc.ficha_de(GENERAL).unwrap();
    let carpeta = almacen::carpeta(&raiz, &g.id);
    let q = p
        .mensajes_pc(&g.id)
        .into_iter()
        .find(|m| m.id == "q")
        .unwrap();
    let quedan: Vec<String> = std::fs::read_to_string(carpeta.join("guardados.jsonl"))
        .unwrap()
        .lines()
        .filter(|l| !l.contains("\"id\":\"q\""))
        .map(str::to_string)
        .collect();
    std::fs::write(carpeta.join("guardados.jsonl"), quedan.join("\n") + "\n").unwrap();
    vista::anotar_borrados(&raiz, &g.id, std::slice::from_ref(&q), p.reloj.tic()).unwrap();
    assert!(
        !otro.fichero(".espacios").exists(),
        "en el PC tambien se va"
    );
    p.vuelta_desde_pc();
    assert!(
        p.movil
            .leer_mensajes()
            .iter()
            .all(|m| kotlin::cadena(m, "id") != Some("q"))
    );
    assert_eq!(
        leer_movil(&p, &format!("pins/draw/anot-{u_queda}.espacios")),
        None
    );
}

/// **El marco de la tinta** (`anot-<uid>-p<n>.hoja`, `anot-<uid>.hoja`,
/// 29-sep) viaja con su tinta en los dos sentidos, tal cual, y se va con su
/// mensaje. Lo lee el lector del PC para encajar la tinta en su hoja.
#[test]
fn el_marco_de_la_tinta_viaja_con_ella_y_se_va_con_su_mensaje() {
    use pixpin_proyecto::anotado;
    let p = montar("anotado-marco");
    let u = adjunto_movil(&p, "p", "7_plano.pdf", b"%PDF");
    let u_w = adjunto_movil(&p, "w", "8_acta.docx", b"PK");
    dibujo_movil(&p, &format!("anot-{u}-p1"), &["t"]);
    escribir_movil(
        &p,
        &format!("pins/draw/anot-{u}-p1.hoja"),
        "-1050.0,0.0,2450.0,4950.0
v1
",
    );
    dibujo_movil(&p, &format!("anot-{u_w}"), &["w"]);
    escribir_movil(
        &p,
        &format!("pins/draw/anot-{u_w}.hoja"),
        "280,0,700,420
",
    );
    // Un temporal a medias no viaja.
    escribir_movil(&p, &format!("pins/draw/anot-{u}-p1.hoja.tmp"), "?");
    p.vuelta_desde_pc();

    let raiz = p.raiz();
    let pdf = doc_en_pc(&p, "p");
    let marco = anotado::marco_del_pdf(&raiz, &pdf, 1).unwrap();
    assert_eq!(
        anotado::leer(&marco).as_deref(),
        Some(
            "-1050.0,0.0,2450.0,4950.0
v1
"
        ),
        "llega tal cual"
    );
    assert!(
        !marco.with_extension("hoja.tmp").exists(),
        "el temporal no viaja"
    );
    let b_w = anotado::base_del_documento(&raiz, &doc_en_pc(&p, "w")).unwrap();
    assert_eq!(
        anotado::leer(&b_w.fichero(".hoja")).as_deref(),
        Some(
            "280,0,700,420
"
        )
    );

    // De vuelta: el que escribe el PC llega con el mismo nombre y texto.
    p.reloj.saltar(10_000);
    anotado::escribir(
        &marco,
        "-1050,0,2450,4950
v1
",
    )
    .unwrap();
    adelantar(&marco);
    p.vuelta_desde_pc();
    assert_eq!(
        leer_movil(&p, &format!("pins/draw/anot-{u}-p1.hoja")).as_deref(),
        Some(
            "-1050,0,2450,4950
v1
"
        )
    );

    // Y se va con su mensaje; el del otro mensaje (caso negativo) se queda.
    let (ido, quedan): (Vec<Json>, Vec<Json>) = p
        .movil
        .leer_mensajes()
        .into_iter()
        .partition(|m| kotlin::cadena(m, "id") == Some("p"));
    p.movil.escribir_mensajes(&quedan).unwrap();
    p.movil
        .anotar_borrados(&pixpin_sincro::disco::marcas_de(
            GENERAL,
            &ido,
            p.reloj.tic(),
        ))
        .unwrap();
    p.movil.borrar_anotado(GENERAL, &u);
    assert_eq!(
        leer_movil(&p, &format!("pins/draw/anot-{u}-p1.hoja")),
        None,
        "en el movil se va"
    );
    p.vuelta_desde_pc();
    assert!(!marco.exists(), "y en el PC tambien");
    assert!(b_w.fichero(".hoja").is_file());
}

/// **La pasada del marco** (29-sep; formato de Android v0.98.0 el 30-sep): lo
/// que el movil anoto sin marco recibe en el PC su `.hoja` en las dos lineas
/// que lee Android (`x0,y0,x1,y1` y `v1`); viaja una vez al movil y despues
/// todo queda quieto.
#[test]
fn los_marcos_que_pone_la_pasada_viajan_una_vez_en_las_dos_lineas_de_android() {
    use pixpin_proyecto::anotado;
    use pixpin_sincro::anotado::MarcoDeLaHoja;
    let p = montar("anotado-pasada");
    let u = adjunto_movil(&p, "p", "7_plano.pdf", b"%PDF");
    dibujo_movil(&p, &format!("anot-{u}-p0"), &["t1", "t2"]);
    p.vuelta_desde_pc();
    p.quieto(GENERAL);
    let raiz = p.raiz();
    let tinta = anotado::hoja_del_pdf(&raiz, &doc_en_pc(&p, "p"), 0).unwrap();
    let antes = std::fs::read(&tinta).unwrap();

    let pasada = anotado::poner_marcos(&raiz, &mut |_| {
        Some(MarcoDeLaHoja::nuevo(-1050.0, 0.0, 2450.0, 4950.0))
    });
    assert_eq!(pasada.escritos, 1, "{pasada:?}");
    assert_eq!(std::fs::read(&tinta).unwrap(), antes, "la tinta no se toca");
    let rel = format!("pins/draw/anot-{u}-p0.hoja");
    let hecho = p.vuelta_desde_pc();
    assert!(hecho.archivos >= 1, "{hecho:?}");
    let llegado = leer_movil(&p, &rel).expect("el marco llega al movil");
    // Exactamente lo que escribe `Marco.aTexto` de Android.
    assert_eq!(llegado, "-1050,0,2450,4950\nv1\n");
    // Y una vez alla, nada mas: ni otra pasada ni otra vuelta mueven nada.
    assert_eq!(anotado::poner_marcos(&raiz, &mut |_| None).ya_estaban, 1);
    p.quieto(GENERAL);
    assert_eq!(p.vuelta_desde_pc().archivos, 0);

    // El movil v0.98 anota la hoja en sus unidades (0..1400) y reescribe su
    // marco: llega y el PC lo deja tal cual.
    p.reloj.saltar(10_000);
    dibujo_movil(&p, &format!("anot-{u}-p0"), &["t1", "t2", "t3"]);
    escribir_movil(&p, &rel, "0,0,1400,1979.899\nv1\n");
    p.vuelta_desde_pc();
    let b = anotado::Base::de_pagina(
        &raiz,
        &anotado::adjunto_de(&raiz, &doc_en_pc(&p, "p")).unwrap(),
        0,
    );
    assert_eq!(
        anotado::leer(&b.marco()).as_deref(),
        Some("0,0,1400,1979.899\nv1\n")
    );
    let otra = anotado::poner_marcos(&raiz, &mut |_| panic!("ya tiene marco"));
    assert_eq!((otra.ya_estaban, otra.escritos), (1, 0));
}

// ------------------------------- lo que el PC anota llega al movil (28-sep)

/// Lo que hace el lector del PC al soltar un trazo (`lector_tinta::Capa::guardar`):
/// lee la capa, le pone un trazo de `n` puntos tirado con el raton (muestras
/// separadas, como llegan de Windows) y la escribe entera por un temporal.
fn anotar_como_el_lector(hoja: &Path, n: usize, desde: f32) {
    use pixpin_motor2d::elemento::{Elemento, Figura};
    use pixpin_motor2d::excalidraw;
    use pixpin_motor2d::vector::Punto2;
    let lienzo = std::fs::read_to_string(hoja)
        .ok()
        .and_then(|t| excalidraw::leer(&t).ok())
        .unwrap_or_else(excalidraw::Lienzo::vacio);
    let mut escena = excalidraw::a_escena(&lienzo);
    let puntos: Vec<Punto2> = (0..n)
        .map(|i| {
            let t = i as f32 / (n - 1) as f32;
            // Media vuelta de circulo: nada que se parezca a una recta.
            Punto2::nuevo(
                desde + 200.0 * (t * std::f32::consts::PI).cos(),
                300.0 + 200.0 * (t * std::f32::consts::PI).sin(),
            )
        })
        .collect();
    // Como lo deja el gesto del raton: el origen en el primer punto.
    escena.anadir(Elemento {
        x: puntos[0].x,
        y: puntos[0].y,
        figura: Figura::Lapiz {
            puntos,
            presiones: Vec::new(),
            opciones: Some(Default::default()),
        },
        grosor: 1.0,
        ..Elemento::default()
    });
    let nuevo = excalidraw::con_escena(&lienzo, &escena);
    std::fs::create_dir_all(hoja.parent().unwrap()).unwrap();
    let tmp = hoja.with_extension("excalidraw.tmp");
    std::fs::write(&tmp, excalidraw::escribir(&nuevo)).unwrap();
    std::fs::rename(&tmp, hoja).unwrap();
}

/// Cuantos trazos vivos hay y cuantos puntos lleva cada uno. Y que cada uno
/// llega con sus puntos relativos y la caja de ellos: con la caja a cero el
/// movil lo pinta como la raya de su primer punto al ultimo
/// (`Renderer.sePierdeDePequeno`).
fn trazos(texto: &str) -> Vec<usize> {
    let v: serde_json::Value = serde_json::from_str(texto).unwrap();
    v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["isDeleted"] != true && e["type"] == "freedraw")
        .map(|e| {
            let p = e["points"].as_array().unwrap();
            let xs: Vec<f64> = p.iter().map(|q| q["x"].as_f64().unwrap()).collect();
            let ys: Vec<f64> = p.iter().map(|q| q["y"].as_f64().unwrap()).collect();
            let ancho = |l: &[f64]| {
                l.iter().cloned().fold(f64::MIN, f64::max)
                    - l.iter().cloned().fold(f64::MAX, f64::min)
            };
            assert!(
                (e["width"].as_f64().unwrap() - ancho(&xs)).abs() < 1e-3,
                "caja: {e}"
            );
            assert!(
                (e["height"].as_f64().unwrap() - ancho(&ys)).abs() < 1e-3,
                "caja: {e}"
            );
            assert_eq!((xs[0], ys[0]), (0.0, 0.0), "relativos a su x/y: {e}");
            p.len()
        })
        .collect()
}

#[test]
fn lo_que_el_pc_anota_en_un_pdf_que_el_movil_ya_tiene_le_llega_en_cada_vuelta() {
    use pixpin_proyecto::anotado;
    let p = montar("anota-pc");
    let u = adjunto_movil(&p, "p", "1_plano.pdf", b"%PDF-1.4");
    p.vuelta_desde_pc();
    let raiz = p.raiz();
    let pdf = doc_en_pc(&p, "p");
    let hoja = anotado::hoja_del_pdf(&raiz, &pdf, 0).unwrap();
    let alli = |p: &Par| {
        leer_gz_movil(p, &format!("pins/draw/anot-{u}-p0.excalidraw.gz"))
            .map(|t| trazos(&t))
            .unwrap_or_default()
    };

    // Lo que el PC ya tenia escrito como antes (caja a 0, puntos `[x, y]`):
    // en el disco del PC se queda asi y sale arreglado.
    let vieja = anotado::hoja_del_pdf(&raiz, &pdf, 1).unwrap();
    std::fs::create_dir_all(vieja.parent().unwrap()).unwrap();
    let antes = r#"{"elements":[{"id":"pc1","type":"freedraw","x":475.0,"y":1130.0,"width":0.0,"height":0.0,
        "seed":1,"version":87,"opacity":100,"roughness":1,"pressures":[],"simulatePressure":true,
        "points":[[0.0,0.0],[-16.8,-8.4],[-50.4,-14.9],[-113.9,-14.9],[-191.3,4.7]]}],"type":"excalidraw"}"#;
    std::fs::write(&vieja, antes).unwrap();

    // Primera vez: la hoja no tenia nada.
    anotar_como_el_lector(&hoja, 40, 100.0);
    p.vuelta_desde_pc();
    assert_eq!(
        alli(&p),
        vec![40],
        "la primera anotacion llega con sus 40 puntos"
    );
    let p1 = leer_gz_movil(&p, &format!("pins/draw/anot-{u}-p1.excalidraw.gz")).unwrap();
    assert_eq!(trazos(&p1), vec![5], "la de antes llega con su caja");
    assert_eq!(
        std::fs::read_to_string(&vieja).unwrap(),
        antes,
        "en el PC no se reescribe"
    );

    // Segunda, enseguida: la vuelta tiene que ver el cambio.
    anotar_como_el_lector(&hoja, 25, 500.0);
    p.vuelta_desde_pc();
    assert_eq!(alli(&p), vec![40, 25], "la segunda tambien llega");

    // El movil abre la hoja y la guarda a su manera (sus campos, compacta):
    // al PC le llega, y lo que el PC anota despues vuelve a llegar alli.
    let rel = format!("pins/draw/anot-{u}-p0.excalidraw.gz");
    let mut v: serde_json::Value = serde_json::from_str(&leer_gz_movil(&p, &rel).unwrap()).unwrap();
    for e in v["elements"].as_array_mut().unwrap() {
        e["presionFirme"] = json!(false);
    }
    gz(&p.movil.raiz().join(&rel), &v.to_string());
    p.vuelta_desde_pc();
    anotar_como_el_lector(&hoja, 30, 900.0);
    p.vuelta_desde_pc();
    assert_eq!(
        alli(&p),
        vec![40, 25, 30],
        "y la tercera, tras pasar por el movil"
    );
    p.quieto(GENERAL);
}

fn leer_gz_movil(p: &Par, rel: &str) -> Option<String> {
    let f = p.movil.raiz().join(rel);
    f.is_file().then(|| de_gz(&f))
}

// ------------------------------------- los comentarios de las notas (30-sep)

const COMENTARIOS: &str = r#"{"version":1,"comentarios":[{"id":"K7Q2-1-0","ancla":{"cita":"losa","antes":"La ","despues":" ya","pos":10},"autor":"Portátil","aparato":"K7Q2","cuando":1,"texto":"¿Cuando se cura?","resuelto":false,"respuestas":[]}]}
"#;

/// **Los comentarios de una nota del chat** (`anot-<codigo>.comentarios.json`,
/// 30-sep) llegan al movil tal cual, vuelven con lo que alli se responda y
/// se van con su nota en los dos aparatos. Los de otra nota se quedan.
#[test]
fn los_comentarios_de_una_nota_viajan_con_ella_y_se_van_con_ella() {
    use pixpin_proyecto::comentarios_de_notas as cn;
    let p = montar("comentarios-nota");
    let g = p.guardados();
    let n = p.nota_pc(&g, "# Obra\nLa losa ya esta hormigonada.");
    let otra = p.nota_pc(&g, "otra nota");
    let raiz = p.raiz();
    let f = cn::de_la_nota(&raiz, &g.id, &n.codigo_unico()).unwrap();
    let f_otra = cn::de_la_nota(&raiz, &g.id, &otra.codigo_unico()).unwrap();
    cn::escribir(&f, COMENTARIOS).unwrap();
    cn::escribir(&f_otra, "{\"version\":1,\"comentarios\":[]}\n").unwrap();
    // Un temporal a medias no viaja.
    std::fs::write(f.with_extension("json.tmp"), "?").unwrap();
    p.vuelta_desde_pc();
    let rel = format!("pins/draw/anot-{}.comentarios.json", n.codigo_unico());
    let rel_otra = format!("pins/draw/anot-{}.comentarios.json", otra.codigo_unico());
    assert_eq!(
        leer_movil(&p, &rel).as_deref(),
        Some(COMENTARIOS),
        "llega tal cual"
    );
    assert!(
        leer_movil(&p, &format!("{rel}.tmp")).is_none(),
        "el temporal no viaja"
    );

    // Respondido en el movil: vuelve al PC.
    p.reloj.saltar(10_000);
    let respondido = COMENTARIOS.replace(
        "\"respuestas\":[]",
        "\"respuestas\":[{\"id\":\"MOVI-2-0\",\"autor\":\"Teléfono\",\"aparato\":\"MOVI\",\"cuando\":2,\"texto\":\"El lunes\"}]",
    );
    escribir_movil(&p, &rel, &respondido);
    adelantar(&p.movil.raiz().join(&rel));
    p.vuelta_desde_pc();
    assert_eq!(
        cn::leer(&f).as_deref(),
        Some(respondido.as_str()),
        "la respuesta llega al PC"
    );
    let leidos: serde_json::Value = serde_json::from_str(&cn::leer(&f).unwrap()).unwrap();
    assert_eq!(
        leidos["comentarios"][0]["respuestas"][0]["texto"],
        "El lunes"
    );

    // Borrada la nota en el PC, como la borra su chat: fuera sus comentarios
    // aqui y, tras la vuelta, en el movil.
    let carpeta = almacen::carpeta(&raiz, &g.id);
    let quedan: Vec<String> = std::fs::read_to_string(carpeta.join("guardados.jsonl"))
        .unwrap()
        .lines()
        .filter(|l| !l.contains(&format!("\"id\":\"{}\"", n.id)))
        .map(str::to_string)
        .collect();
    std::fs::write(carpeta.join("guardados.jsonl"), quedan.join("\n") + "\n").unwrap();
    vista::anotar_borrados(&raiz, &g.id, std::slice::from_ref(&n), p.reloj.tic()).unwrap();
    assert!(!f.exists(), "en el PC se van con su nota");
    p.vuelta_desde_pc();
    assert_eq!(leer_movil(&p, &rel), None, "y en el movil tambien");
    // Caso negativo: los de la otra nota siguen en los dos.
    assert!(f_otra.is_file());
    assert!(leer_movil(&p, &rel_otra).is_some());
    p.quieto(GENERAL);
}

fn proyecto_con_notas_en_el_movil(
    p: &Par,
    hojas: serde_json::Value,
    quitadas: &[&str],
    tocado: i64,
) {
    p.movil
        .guardar_proyecto(
            &kotlin::normalizar_proyecto(&Json::de_valor(&json!({
                "id": "pr-n", "nombre": "Casa Lima", "tocado": tocado, "hojas": hojas, "quitadas": quitadas,
                "uid": "PPPPPPPPPP", "creado": 1_726_000_000_000_i64, "aparato": "MOVI",
            })))
            .unwrap(),
        )
        .unwrap();
}

/// **Los de una hoja `nota` del movil** van con el codigo de la hoja: llegan
/// al PC junto a ella y, si el movil quita la hoja, se van en los dos.
#[test]
fn los_comentarios_de_una_hoja_nota_del_movil_viajan_y_se_van_con_la_hoja() {
    use pixpin_proyecto::comentarios_de_notas as cn;
    let p = montar("comentarios-hoja");
    let dos = json!([
        {"id": "n-1", "uid": "NNNNNNNNNN", "nota": "# Planta\nLa losa ya esta."},
        {"id": "n-2", "uid": "OOOOOOOOOO", "nota": "otra"},
    ]);
    proyecto_con_notas_en_el_movil(&p, dos, &[], 5);
    escribir_movil(
        &p,
        "pins/draw/anot-NNNNNNNNNN.comentarios.json",
        COMENTARIOS,
    );
    escribir_movil(
        &p,
        "pins/draw/anot-OOOOOOOOOO.comentarios.json",
        "{\"version\":1,\"comentarios\":[]}\n",
    );
    p.vuelta_desde_pc();
    let raiz = p.raiz();
    let ficha = p.pc.ficha_de("pr-n").unwrap();
    let f = cn::de_la_nota(&raiz, &ficha.id, "NNNNNNNNNN").unwrap();
    let f_otra = cn::de_la_nota(&raiz, &ficha.id, "OOOOOOOOOO").unwrap();
    assert_eq!(
        cn::leer(&f).as_deref(),
        Some(COMENTARIOS),
        "llegan junto a su hoja"
    );
    assert!(f_otra.is_file());

    // El movil quita la hoja n-1 a mano (con su marca `quitadas`, como su
    // `Proyectos`): alli se van sus comentarios, y al PC le
    // llega el proyecto sin ella y los quita tambien.
    p.reloj.saltar(10_000);
    let una = json!([{"id": "n-2", "uid": "OOOOOOOOOO", "nota": "otra"}]);
    proyecto_con_notas_en_el_movil(&p, una, &["n-1"], p.reloj.tic());
    assert_eq!(
        leer_movil(&p, "pins/draw/anot-NNNNNNNNNN.comentarios.json"),
        None,
        "en el movil"
    );
    p.vuelta_desde_pc();
    assert!(!f.exists(), "en el PC tambien");
    // Caso negativo: los de la hoja que sigue no se tocan.
    assert!(f_otra.is_file());
    assert!(leer_movil(&p, "pins/draw/anot-OOOOOOOOOO.comentarios.json").is_some());
}

/// Un adjunto `.md` en el chat de un proyecto nacido en el PC, como lo deja
/// su chat al soltarlo: nombre con raya, tildes, espacios y parentesis.
fn md_en_proyecto_del_pc(p: &Par, ficha: &Ficha, nombre: &str, texto: &str) -> Mensaje {
    let raiz = p.raiz();
    let ruta = almacen::guardar_adjunto(&raiz, &ficha.id, nombre, texto.as_bytes()).unwrap();
    let carpeta = almacen::carpeta(&raiz, &ficha.id);
    let numero = p.mensajes_pc(&ficha.id).len() as i64 + 1;
    let m = Mensaje::adjunto(
        Clase::Archivo,
        nombre,
        &ruta,
        texto.len() as i64,
        &Sello {
            cuando: p.reloj.tic(),
            numero,
            aparato: p.aparato_pc(),
            proyecto: ficha.id.clone(),
        },
    );
    cuaderno::anadir(&carpeta, &m).unwrap();
    m
}

/// Lo que abre el movil al pulsar el mensaje `id`: el fichero al que apunta
/// su `ruta` (absoluta, la de su carpeta `files`).
fn abrir_en_el_movil(p: &Par, id: &str) -> Option<Vec<u8>> {
    let m = p
        .movil
        .leer_mensajes()
        .into_iter()
        .find(|m| kotlin::cadena(m, "id") == Some(id))
        .expect("el mensaje llego al movil");
    let ruta = kotlin::cadena(&m, "ruta").unwrap().to_string();
    let rel = ruta
        .strip_prefix(&p.movil.absoluta(""))
        .expect("ruta de su carpeta files");
    std::fs::read(p.movil.raiz().join(rel)).ok()
}

#[test]
fn los_md_con_parentesis_de_un_proyecto_del_pc_se_abren_en_el_movil() {
    // La queja del 1-oct-2026 («TESIS RESERCH»): el mensaje llegaba pero el
    // fichero no, y el movil decia «este archivo ya no esta». `en_texto`
    // corta la ruta en `)` (lo hace igual `Rutas.enTexto` de Android, por
    // los enlaces de Markdown), y «… (1).md» quedaba en «… (1», que no es
    // un fichero: no entraba en lo que el PC manda.
    let p = montar("md-parentesis");
    let raiz = p.raiz();
    let ficha = Ficha::nueva("TESIS RESERCH", p.reloj.tic(), &p.aparato_pc());
    let mut i = Indice::leer(&raiz);
    i.proyectos.push(ficha.clone());
    i.guardar(&raiz).unwrap();
    let uno = md_en_proyecto_del_pc(
        &p,
        &ficha,
        "Objetivos e Indicadores — Versión para la segunda asesoría (1).md",
        "# Objetivos\nuno",
    );
    let dos = md_en_proyecto_del_pc(
        &p,
        &ficha,
        "Objetivos e Indicadores — Versión para la segunda asesoría (2).md",
        "# Objetivos\ndos",
    );
    // Un tercero sin parentesis, de control.
    let tres = md_en_proyecto_del_pc(&p, &ficha, "notas.md", "# Notas");
    // Y un fichero suelto en `archivos/` que ningun mensaje senala: no viaja.
    std::fs::write(
        almacen::carpeta(&raiz, &ficha.id).join("archivos/suelto (9).md"),
        "x",
    )
    .unwrap();

    p.vuelta_desde_pc();

    assert_eq!(
        abrir_en_el_movil(&p, &uno.id).as_deref(),
        Some(&b"# Objetivos\nuno"[..])
    );
    assert_eq!(
        abrir_en_el_movil(&p, &dos.id).as_deref(),
        Some(&b"# Objetivos\ndos"[..])
    );
    assert_eq!(
        abrir_en_el_movil(&p, &tres.id).as_deref(),
        Some(&b"# Notas"[..])
    );
    let chat = vista::chat_de_ficha(&raiz, &ficha.id).unwrap();
    assert!(
        !p.movil
            .raiz()
            .join(format!("guardados/pc/{chat}/archivos/suelto (9).md"))
            .exists(),
        "lo que no senala ningun mensaje no viaja"
    );
    // Mientras el movil no liste el fichero por su ruta exacta (ver
    // docs/investigacion/2026-10-01-ficheros-de-proyectos-del-pc-android.md),
    // el PC se lo vuelve a ofrecer en cada vuelta. Molesta pero no rompe:
    // otra vuelta no borra nada en ningun lado y sigue abriendose.
    p.vuelta_desde_pc();
    assert_eq!(
        abrir_en_el_movil(&p, &uno.id).as_deref(),
        Some(&b"# Objetivos\nuno"[..])
    );
    let en_pc = vista::ruta_real(&raiz, &ficha.id, uno.ruta.as_deref().unwrap()).unwrap();
    assert_eq!(std::fs::read(en_pc).unwrap(), b"# Objetivos\nuno");
}

#[test]
fn cuando_dirige_el_movil_tambien_se_lleva_los_md_con_parentesis() {
    let p = montar("md-parentesis-movil");
    let raiz = p.raiz();
    let ficha = Ficha::nueva("Tesis", p.reloj.tic(), &p.aparato_pc());
    let mut i = Indice::leer(&raiz);
    i.proyectos.push(ficha.clone());
    i.guardar(&raiz).unwrap();
    let m = md_en_proyecto_del_pc(&p, &ficha, "capitulo (3).md", "# Tres");
    let chat = vista::chat_de_ficha(&raiz, &ficha.id).unwrap();
    p.desde_movil(&[chat.as_str()]);
    assert_eq!(
        abrir_en_el_movil(&p, &m.id).as_deref(),
        Some(&b"# Tres"[..])
    );
}

// ------------------------------------- lo que no viajaba (12-oct-2026)

/// El `archivado` del proyecto `id` en el movil.
fn archivado_en_movil(p: &Par, id: &str) -> Option<bool> {
    p.movil
        .leer_proyectos()
        .iter()
        .find(|x| kotlin::cadena(x, "id") == Some(id))
        .map(|x| {
            x.como_objeto()
                .and_then(|o| o.obtener("archivado"))
                .and_then(Json::contenido)
                == Some("true")
        })
}

/// Cambia el `archivado` de un proyecto en el movil, como
/// `Proyectos.archivado` (que tambien lo da por tocado).
fn archivar_en_movil(p: &Par, id: &str, si: bool) {
    let lista: Vec<Json> = p
        .movil
        .leer_proyectos()
        .into_iter()
        .map(|x| {
            if kotlin::cadena(&x, "id") != Some(id) {
                return x;
            }
            let mut x = x;
            kotlin::poner(&mut x, "archivado", Json::booleano(si));
            kotlin::poner(&mut x, "tocado", Json::numero(p.reloj.tic()));
            x
        })
        .collect();
    p.movil.escribir_proyectos(&lista).unwrap();
}

/// Archiva en el PC como `proyectos::archivar`: en su `proyecto.json` si lo
/// tiene, y si no con la marca de su ficha.
fn archivar_en_pc(p: &Par, ficha_id: &str, si: bool) {
    let raiz = p.raiz();
    let carpeta = almacen::carpeta(&raiz, ficha_id);
    let json = carpeta.join("proyecto.json");
    let con_json = json.is_file();
    if con_json {
        let mut pr: pixpin_proyecto::Proyecto =
            serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
        pr.archivado = si;
        pr.tocado = p.reloj.tic();
        std::fs::write(&json, serde_json::to_string(&pr).unwrap()).unwrap();
        adelantar(&json);
    }
    let mut i = Indice::leer(&raiz);
    let f = i.proyectos.iter_mut().find(|f| f.id == ficha_id).unwrap();
    if si && !con_json {
        f.resto.insert(vista::MARCA_ARCHIVADO.into(), json!(true));
    } else {
        f.resto.remove(vista::MARCA_ARCHIVADO);
    }
    i.guardar(&raiz).unwrap();
}

/// Si el PC lo ensena archivado: lo que lee su lista de Proyectos.
fn archivado_en_pc(p: &Par, ficha_id: &str) -> bool {
    let raiz = p.raiz();
    let json = almacen::carpeta(&raiz, ficha_id).join("proyecto.json");
    let en_json = std::fs::read_to_string(json)
        .ok()
        .and_then(|t| serde_json::from_str::<pixpin_proyecto::Proyecto>(&t).ok())
        .is_some_and(|pr| pr.archivado);
    let en_ficha = Indice::leer(&raiz)
        .proyectos
        .iter()
        .find(|f| f.id == ficha_id)
        .and_then(|f| {
            f.resto
                .get(vista::MARCA_ARCHIVADO)
                .and_then(|v| v.as_bool())
        })
        == Some(true);
    en_json || en_ficha
}

#[test]
fn archivar_un_proyecto_viaja_en_los_dos_sentidos_y_dos_veces() {
    let p = montar("archivar");
    let obra = proyecto_del_pc(&p);
    let chat = vista::chat_de_ficha(&p.raiz(), &obra.id).unwrap();
    p.desde_pc(&[&chat]);
    assert_eq!(
        archivado_en_movil(&p, &chat),
        Some(false),
        "sin archivar, llega sin archivar"
    );

    // 1. Archivado en el PC (proyecto nacido aqui, sin `proyecto.json`).
    p.reloj.saltar(1_000);
    archivar_en_pc(&p, &obra.id, true);
    p.desde_pc(&[&chat]);
    assert_eq!(
        archivado_en_movil(&p, &chat),
        Some(true),
        "el movil se entera"
    );
    p.quieto(&chat);

    // 2. Desarchivado en el movil: el PC deja de ensenarlo archivado.
    p.reloj.saltar(1_000);
    archivar_en_movil(&p, &chat, false);
    p.desde_movil(&[&chat]);
    assert!(
        !archivado_en_pc(&p, &obra.id),
        "ni el proyecto.json ni la ficha lo dicen"
    );
    p.quieto(&chat);

    // 3. Y otra vez en el PC, ahora ya con `proyecto.json`.
    p.reloj.saltar(1_000);
    archivar_en_pc(&p, &obra.id, true);
    p.desde_pc(&[&chat]);
    assert_eq!(
        archivado_en_movil(&p, &chat),
        Some(true),
        "la segunda vez tambien"
    );
    assert!(archivado_en_pc(&p, &obra.id));
    p.quieto(&chat);
}

#[test]
fn una_marca_de_archivado_que_no_es_si_no_archiva_nada() {
    // Caso negativo: una ficha con la marca a `false` (o con otra cosa) no
    // sale archivada, y la de otro proyecto no se contagia.
    let p = montar("archivar-no");
    let obra = proyecto_del_pc(&p);
    let otra = Ficha::nueva("Otra", p.reloj.tic(), &p.aparato_pc());
    let mut i = Indice::leer(&p.raiz());
    i.proyectos.push(otra.clone());
    i.proyectos
        .iter_mut()
        .find(|f| f.id == obra.id)
        .unwrap()
        .resto
        .insert(vista::MARCA_ARCHIVADO.into(), json!(false));
    i.proyectos
        .iter_mut()
        .find(|f| f.id == otra.id)
        .unwrap()
        .resto
        .insert(vista::MARCA_ARCHIVADO.into(), json!("si"));
    i.guardar(&p.raiz()).unwrap();
    p.nota_pc(&otra, "algo");
    let c1 = vista::chat_de_ficha(&p.raiz(), &obra.id).unwrap();
    let c2 = vista::chat_de_ficha(&p.raiz(), &otra.id).unwrap();
    p.desde_pc(&[&c1, &c2]);
    assert_eq!(archivado_en_movil(&p, &c1), Some(false));
    assert_eq!(archivado_en_movil(&p, &c2), Some(false));
}

#[test]
fn un_adjunto_borrado_en_el_movil_se_va_tambien_del_disco_del_pc() {
    let p = montar("borrar-adjunto");
    let uid = adjunto_movil(&p, "lec", "lecciones/k1.leccion", b"leccion");
    let otro = adjunto_movil(&p, "foto", "9_foto.jpg", b"JPG");
    p.desde_pc(&[GENERAL]);
    let (lec, foto) = (doc_en_pc(&p, "lec"), doc_en_pc(&p, "foto"));
    assert!(lec.is_file() && foto.is_file(), "llegaron al PC");

    // Borrado en el movil, como borra su chat: fuera, con su marca.
    p.reloj.saltar(1_000);
    p.movil
        .aplicar_mensajes(GENERAL, &[], std::slice::from_ref(&uid), p.reloj.tic())
        .unwrap();
    p.desde_movil(&[GENERAL]);
    let g = p.pc.ficha_de(GENERAL).unwrap();
    assert!(
        !p.mensajes_pc(&g.id).iter().any(|m| m.id == "lec"),
        "el mensaje se fue"
    );
    assert!(!lec.exists(), "y su fichero tambien, como en el movil");
    assert!(foto.is_file(), "lo demas no se toca");
    p.quieto(GENERAL);
    let _ = otro;
}

#[test]
fn un_fichero_que_otro_mensaje_sigue_senalando_no_se_borra() {
    // Caso negativo: dos mensajes con el mismo fichero (reenviado en el
    // chat del movil); borrar uno no deja al otro sin su adjunto en el PC.
    let p = montar("borrar-adjunto-compartido");
    let uid = adjunto_movil(&p, "a", "5_plano.pdf", b"%PDF");
    let m = Json::de_valor(&json!({
        "id": "b", "cuando": p.reloj.tic(), "clase": "ARCHIVO",
        "ruta": p.movil.absoluta("guardados/5_plano.pdf"), "nombre": "5_plano.pdf",
        "numero": 9, "letra": "a",
    }));
    p.movil.anadir_mensaje(&m).unwrap();
    p.desde_pc(&[GENERAL]);
    let doc = doc_en_pc(&p, "b");
    assert!(doc.is_file());
    p.reloj.saltar(1_000);
    // El movil lo borraria de su disco; aqui se quita solo el mensaje.
    let quedan: Vec<Json> = p
        .movil
        .leer_mensajes()
        .into_iter()
        .filter(|x| kotlin::cadena(x, "id") != Some("a"))
        .collect();
    p.movil.escribir_mensajes(&quedan).unwrap();
    p.movil
        .anotar_borrados(&[pixpin_sincro::disco::Marca {
            chat: GENERAL.into(),
            sena: String::new(),
            cuando: p.reloj.tic(),
            uid: Some(uid),
        }])
        .unwrap();
    p.desde_movil(&[GENERAL]);
    let g = p.pc.ficha_de(GENERAL).unwrap();
    assert!(!p.mensajes_pc(&g.id).iter().any(|m| m.id == "a"));
    assert!(doc.is_file(), "el otro mensaje lo sigue senalando");
}

/// Una tabla hecha en el movil (`Clase.TABLA` y `tablas/<ref>.json`).
fn tabla_movil(p: &Par, id: &str, referencia: &str, texto: &str) {
    escribir_movil(p, &format!("tablas/{referencia}.json"), texto);
    p.movil
        .anadir_mensaje(&Json::de_valor(&json!({
            "id": id, "cuando": p.reloj.tic(), "clase": "TABLA", "referencia": referencia,
            "nombre": "Metrados", "numero": p.movil.leer_mensajes().len() + 1, "letra": "a",
        })))
        .unwrap();
}

/// Cambia una celda de una tabla en su fichero, como la guarda el pin de
/// hoja del PC (`Tabla::escribir`), con un estilo que el movil no conoce.
fn celda_en_pc(archivo: &Path, celda: &str, valor: &str, cursiva: bool) {
    use pixpin_proyecto::tabla::{Tabla, ref_de};
    let mut t = Tabla::leer(&std::fs::read_to_string(archivo).unwrap()).unwrap();
    t.poner(ref_de(celda).unwrap(), valor);
    if cursiva {
        t.estilos.entry(celda.into()).or_default().k = true;
    }
    std::fs::write(archivo, t.escribir().unwrap()).unwrap();
    adelantar(archivo);
}

/// Cambia una celda en el fichero del movil, como `TablasEnDisco.guardar`
/// (su `EstiloDeCelda` solo guarda `n`, `a`, `f` y `e`).
fn celda_en_movil(p: &Par, rel: &str, celda: &str, valor: &str) {
    let f = p.movil.raiz().join(rel);
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
    v["celdas"][celda] = json!(valor);
    v["tocado"] = json!(p.reloj.tic());
    if let Some(e) = v["estilos"].as_object_mut() {
        for (_, x) in e.iter_mut() {
            if let Some(o) = x.as_object_mut() {
                o.retain(|k, _| matches!(k.as_str(), "n" | "a" | "f" | "e"));
            }
        }
    }
    std::fs::write(&f, serde_json::to_string(&v).unwrap()).unwrap();
    adelantar(&f);
}

fn celdas_movil(p: &Par, rel: &str) -> serde_json::Value {
    let v: serde_json::Value = serde_json::from_str(&leer_movil(p, rel).unwrap()).unwrap();
    v["celdas"].clone()
}

#[test]
fn una_tabla_del_movil_se_cambia_en_el_pc_y_vuelve_celda_a_celda() {
    let p = montar("tabla-movil");
    tabla_movil(
        &p,
        "t",
        "t-1",
        r#"{"nombre":"Metrados","celdas":{"A1":"1","B1":"2"},"tocado":1}"#,
    );
    p.desde_pc(&[GENERAL]);
    let g = p.pc.ficha_de(GENERAL).unwrap();
    let m = p
        .mensajes_pc(&g.id)
        .into_iter()
        .find(|m| m.id == "t")
        .unwrap();
    let archivo = vista::tabla_del_movil(&p.raiz(), &g.id, &m).expect("es una tabla del movil");
    assert!(archivo.is_file(), "su fichero llego donde el PC la busca");

    // 1. Cambiada en el PC, con formato del PC: llega al movil.
    p.reloj.saltar(1_000);
    celda_en_pc(&archivo, "A1", "10", true);
    p.desde_pc(&[GENERAL]);
    assert_eq!(celdas_movil(&p, "tablas/t-1.json")["A1"], "10");
    p.quieto(GENERAL);

    // 2. Cambiada en los dos a la vez, cada uno una celda: quedan las dos.
    p.reloj.saltar(1_000);
    celda_en_pc(&archivo, "C1", "pc", false);
    celda_en_movil(&p, "tablas/t-1.json", "D1", "movil");
    p.desde_movil(&[GENERAL]);
    let c = celdas_movil(&p, "tablas/t-1.json");
    assert_eq!(
        (c["A1"].as_str(), c["C1"].as_str(), c["D1"].as_str()),
        (Some("10"), Some("pc"), Some("movil"))
    );
    let en_pc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&archivo).unwrap()).unwrap();
    assert_eq!(en_pc["celdas"], c, "los dos quedan con la misma tabla");
    p.quieto(GENERAL);
}

#[test]
fn una_nota_o_una_tabla_sin_referencia_no_es_una_tabla_del_movil() {
    let p = montar("tabla-movil-no");
    let g = p.guardados();
    let nota = p.nota_pc(&g, "tablas/t-1.json");
    assert!(vista::tabla_del_movil(&p.raiz(), &g.id, &nota).is_none());
    let sin = Mensaje {
        clase: Some(Clase::Otra("TABLA".into())),
        referencia: None,
        ..nota
    };
    assert!(vista::tabla_del_movil(&p.raiz(), &g.id, &sin).is_none());
}

/// **Un lienzo del PC con grafito 8B y una guia, con las guias escondidas**
/// (Tanda 3): llega al movil con `dureza`, `reference` y
/// `referenciasVisibles` tal como su `Scene` los lee, y la vuelta siguiente no
/// lo manda otra vez.
#[test]
fn el_grafito_con_dureza_y_las_guias_del_pc_llegan_al_movil_como_las_lee() {
    use pixpin_motor2d::elemento::{Elemento, Extras, Figura};
    use pixpin_motor2d::vector::Punto2;
    let p = montar("dureza-guias");
    let obra = proyecto_del_pc(&p);
    let carpeta = almacen::carpeta(&p.raiz(), &obra.id);
    let mut escena = pixpin_motor2d::Escena::nueva();
    escena.anadir(Elemento {
        figura: Figura::Lapiz {
            puntos: (0..6)
                .map(|i| Punto2::nuevo(i as f32 * 4.0, i as f32 * 2.0))
                .collect(),
            presiones: Vec::new(),
            opciones: Some(Default::default()),
        },
        grosor: 3.0,
        material: pixpin_motor2d::tinta::MaterialTinta::Cuadritos,
        extras: Extras {
            dureza: Some(pixpin_motor2d::tinta::grafito::Dureza::B8),
            ..Default::default()
        },
        ..Elemento::default()
    });
    escena.anadir(Elemento {
        figura: Figura::Elipse,
        x: 50.0,
        y: 50.0,
        ancho: 40.0,
        alto: 40.0,
        grosor: 1.0,
        extras: Extras {
            referencia: true,
            ..Default::default()
        },
        ..Elemento::default()
    });
    escena.referencias_visibles = false;
    let texto = pixpin_motor2d::excalidraw::escribir(&pixpin_motor2d::excalidraw::con_escena(
        &pixpin_motor2d::excalidraw::Lienzo::vacio(),
        &escena,
    ));
    std::fs::write(carpeta.join("lienzos/d9.excalidraw"), &texto).unwrap();

    p.vuelta_desde_pc();

    let alli = de_gz(&p.movil.raiz().join("pins/draw/d9.excalidraw.gz"));
    let v: serde_json::Value = serde_json::from_str(&alli).unwrap();
    assert_eq!(v["elements"][0]["material"], "cuadritos", "{alli}");
    assert_eq!(v["elements"][0]["dureza"], "8B", "{alli}");
    assert_eq!(v["elements"][1]["reference"], true, "{alli}");
    assert_eq!(v["referenciasVisibles"], false, "{alli}");
    // Caso negativo: el trazo que no es guia no lleva `reference`, y el
    // ovalo, que no es de grafito, no lleva `dureza`.
    assert!(v["elements"][0].get("reference").is_none_or(|r| r == false));
    assert!(v["elements"][1].get("dureza").is_none());
    // Y lo que el movil leeria vuelve a ser lo mismo en el PC.
    let vuelta =
        pixpin_motor2d::excalidraw::a_escena(&pixpin_motor2d::excalidraw::leer(&alli).unwrap());
    assert_eq!(
        vuelta.elementos[0].extras.dureza,
        Some(pixpin_motor2d::tinta::grafito::Dureza::B8)
    );
    assert!(vuelta.elementos[1].extras.referencia);
    assert!(!vuelta.referencias_visibles);
    let chat = vista::chat_de_ficha(&p.raiz(), &obra.id).unwrap();
    p.quieto(&chat);
}

/// **Fotos y audios en una leccion vieja, ida y vuelta** (Tanda 7,
/// `AdjuntosDeLaLeccion.kt` v0.102): la leccion y su foto vienen del movil;
/// en el PC se le pone un audio que **le responde** y se le quita la foto,
/// como hace `lecciones::almacen::guardar_con_cambios` (adjunto en
/// `archivos/`, mensaje con `respondeA`; quitar = fuera del cuaderno con su
/// marca de borrado). El movil recibe el audio con sus claves y pierde la
/// foto, y la vuelta siguiente no la resucita.
#[test]
fn en_una_leccion_vieja_anadir_y_quitar_viaja_ida_y_vuelta() {
    let p = montar("leccion-adjuntos");
    adjunto_movil(
        &p,
        "lec-k1",
        "lecciones/k1.leccion",
        br#"{"id":"k1","titulo":"Curar el concreto"}"#,
    );
    // La foto que el movil le puso: le responde.
    let rel = "guardados/leccion_1.jpg";
    let f = p.movil.raiz().join(rel);
    std::fs::write(&f, b"JPG").unwrap();
    p.movil
        .anadir_mensaje(&Json::de_valor(&json!({
            "id": "foto-1", "cuando": p.reloj.tic(), "clase": "IMAGEN", "ruta": p.movil.absoluta(rel),
            "nombre": "Foto de la lección", "respondeA": "lec-k1", "numero": 2, "letra": "a",
        })))
        .unwrap();
    p.desde_pc(&[GENERAL]);
    let g = p.pc.ficha_de(GENERAL).unwrap();
    assert!(
        p.mensajes_pc(&g.id)
            .iter()
            .any(|m| m.id == "foto-1" && m.responde_a.as_deref() == Some("lec-k1")),
        "la foto llego respondiendo"
    );

    // En el PC: un audio nuevo que responde a la leccion...
    let carpeta = almacen::carpeta(&p.raiz(), &g.id);
    let ruta = almacen::guardar_adjunto(&p.raiz(), &g.id, "voz_1.m4a", b"M4A").unwrap();
    let numero = Cuaderno::leer_de(&carpeta)
        .unwrap()
        .mensajes
        .iter()
        .map(|m| m.numero)
        .max()
        .unwrap_or(0)
        + 1;
    let mut audio = Mensaje::adjunto(
        Clase::Voz,
        "Audio de la lección",
        &ruta,
        3,
        &Sello {
            cuando: p.reloj.tic(),
            numero,
            aparato: p.aparato_pc(),
            proyecto: g.id.clone(),
        },
    );
    audio.duracion_ms = 4_200;
    audio.responde_a = Some("lec-k1".into());
    cuaderno::anadir(&carpeta, &audio).unwrap();
    // ...y la foto fuera, con su marca.
    let quitados = cuaderno::quitar(&carpeta, &["foto-1".to_string()].into()).unwrap();
    assert_eq!(quitados.len(), 1);
    vista::anotar_borrados(&p.raiz(), &g.id, &quitados, p.reloj.tic()).unwrap();

    p.desde_pc(&[GENERAL]);
    let en_movil = p.movil.leer_mensajes();
    assert!(
        !en_movil
            .iter()
            .any(|m| kotlin::cadena(m, "id") == Some("foto-1")),
        "la foto quitada se va del movil"
    );
    let a = en_movil
        .iter()
        .find(|m| kotlin::cadena(m, "clase") == Some("VOZ"))
        .expect("el audio llega al movil");
    assert_eq!(
        kotlin::cadena(a, "respondeA"),
        Some("lec-k1"),
        "le responde a la leccion"
    );
    assert_eq!(kotlin::numero(a, "duracionMs"), Some(4_200));
    assert_eq!(kotlin::cadena(a, "nombre"), Some("Audio de la lección"));
    let ruta_movil = kotlin::cadena(a, "ruta").unwrap();
    let base = p.movil.absoluta("");
    let rel_movil = ruta_movil
        .strip_prefix(base.trim_end_matches('/'))
        .unwrap()
        .trim_start_matches('/');
    assert!(
        p.movil.raiz().join(rel_movil).is_file(),
        "con su archivo: {ruta_movil}"
    );

    // Ida y vuelta: dirija quien dirija, nada resucita ni se pierde.
    p.desde_movil(&[GENERAL]);
    let pc = p.mensajes_pc(&g.id);
    assert!(!pc.iter().any(|m| m.id == "foto-1"), "no resucita en el PC");
    let vuelto = pc.iter().find(|m| m.clase == Some(Clase::Voz)).unwrap();
    assert_eq!(vuelto.responde_a.as_deref(), Some("lec-k1"));
    assert_eq!(vuelto.duracion_ms, 4_200);
    p.quieto(GENERAL);
}

/// **El instrumento de graficar y una tabla de coordenadas del PC** (Tanda 4):
/// llegan al movil con los tipos y las claves de `Element.kt` y `Scene.kt`
/// (`pixpin-axes`, `pixpin-space` con `unidad`, `azimut`...; `tablas`,
/// `origenCoordenadas`), se leen igual de vuelta, y la vuelta siguiente no
/// los manda otra vez.
#[test]
fn el_plano_el_espacio_y_las_coordenadas_del_pc_llegan_al_movil_como_los_lee() {
    use pixpin_motor2d::instrumento::tabla::{PuntoDeTabla, TablaDeCoordenadas};
    use pixpin_motor2d::vector::Punto2;
    let p = montar("instrumentos");
    let obra = proyecto_del_pc(&p);
    let carpeta = almacen::carpeta(&p.raiz(), &obra.id);
    let negro = pixpin_motor2d::ColorRgba::opaco(0.0, 0.0, 0.0);
    let mut escena = pixpin_motor2d::Escena::nueva();
    escena.anadir(pixpin_motor2d::instrumento::elemento_de_fabrica(
        2, negro, 2.0, 1.0,
    ));
    escena.anadir(pixpin_motor2d::instrumento::elemento_de_fabrica(
        3, negro, 2.0, 1.0,
    ));
    escena.tablas = vec![TablaDeCoordenadas {
        id: "t1".into(),
        nombre: String::new(),
        color: "#e03131".into(),
        puntos: vec![PuntoDeTabla { x: 3.0, y: -2.0 }],
        visible: true,
    }];
    escena.origen_coordenadas = Some(Punto2::nuevo(160.0, 160.0));
    let texto = pixpin_motor2d::excalidraw::escribir(&pixpin_motor2d::excalidraw::con_escena(
        &pixpin_motor2d::excalidraw::Lienzo::vacio(),
        &escena,
    ));
    std::fs::write(carpeta.join("lienzos/d9.excalidraw"), &texto).unwrap();

    p.vuelta_desde_pc();

    let alli = de_gz(&p.movil.raiz().join("pins/draw/d9.excalidraw.gz"));
    let v: serde_json::Value = serde_json::from_str(&alli).unwrap();
    assert_eq!(v["elements"][0]["type"], "pixpin-axes", "{alli}");
    assert_eq!(v["elements"][0]["unidad"], 40.0, "{alli}");
    assert_eq!(v["elements"][1]["type"], "pixpin-space", "{alli}");
    assert_eq!(v["elements"][1]["elevacion"], 25.0, "{alli}");
    assert_eq!(v["tablas"][0]["puntos"][0]["y"], -2.0, "{alli}");
    assert_eq!(v["origenCoordenadas"]["x"], 160.0, "{alli}");
    // Caso negativo: el plano no lleva las claves del espacio.
    assert!(v["elements"][0].get("azimut").is_none(), "{alli}");
    let vuelta =
        pixpin_motor2d::excalidraw::a_escena(&pixpin_motor2d::excalidraw::leer(&alli).unwrap());
    assert_eq!(vuelta.tablas, escena.tablas);
    assert_eq!(vuelta.origen_coordenadas, escena.origen_coordenadas);
    assert_eq!(
        pixpin_motor2d::instrumento::de(&vuelta.elementos[1]).map(|i| i.ejes),
        Some(3)
    );
    let chat = vista::chat_de_ficha(&p.raiz(), &obra.id).unwrap();
    p.quieto(&chat);
}

// ------------------------- lo nuevo del 23 al 25-oct-2026 (todo viaja)
//
// Grabar clase (voz, video, capturas, marcadores y el texto que llega
// despues), la biblioteca de figuras, las carpetas de paginas y las fotos de
// la nota rica. El manual para Android y apple esta en
// docs/manual-sincronizar-lo-nuevo.md.

/// Un proyecto nuevo en el PC, sin nada dentro.
fn ficha_pc(p: &Par, nombre: &str) -> Ficha {
    let raiz = p.raiz();
    let ficha = Ficha::nueva(nombre, p.reloj.tic(), &p.aparato_pc());
    let mut i = Indice::leer(&raiz);
    i.proyectos.push(ficha.clone());
    i.guardar(&raiz).unwrap();
    ficha
}

fn sello_pc(p: &Par, ficha: &Ficha, numero: i64) -> Sello {
    Sello {
        cuando: p.reloj.tic(),
        numero,
        aparato: p.aparato_pc(),
        proyecto: ficha.id.clone(),
    }
}

/// El mensaje `id` del movil, como `serde_json`.
fn mensaje_movil(p: &Par, id: &str) -> serde_json::Value {
    let m = p
        .movil
        .leer_mensajes()
        .into_iter()
        .find(|m| kotlin::cadena(m, "id") == Some(id))
        .unwrap_or_else(|| panic!("el mensaje {id} no llego al movil"));
    serde_json::from_str(&m.a_texto()).unwrap()
}

/// Cambia el mensaje `id` del movil como lo haria su chat.
fn cambiar_en_movil(p: &Par, id: &str, cambio: impl FnOnce(&mut serde_json::Value)) {
    let mut lista = p.movil.leer_mensajes();
    let i = lista
        .iter()
        .position(|m| kotlin::cadena(m, "id") == Some(id))
        .unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&lista[i].a_texto()).unwrap();
    cambio(&mut v);
    lista[i] = Json::de_valor(&v);
    p.movil.escribir_mensajes(&lista).unwrap();
}

fn mensaje_pc(p: &Par, ficha: &Ficha, id: &str) -> Mensaje {
    p.mensajes_pc(&ficha.id)
        .into_iter()
        .find(|m| m.id == id)
        .unwrap_or_else(|| panic!("el mensaje {id} no esta en el PC"))
}

fn ms_de_marcadores(v: &serde_json::Value) -> Vec<i64> {
    v["marcadores"]
        .as_array()
        .map(|l| l.iter().filter_map(|m| m["ms"].as_i64()).collect())
        .unwrap_or_default()
}

/// **Una clase grabada en el PC** (`grabar_clase.rs`): la nota de voz con
/// `grabacion`, `capturas`, `video` y `marcadores`; el `.mp4` (varios
/// megas: viaja a trozos de 1 MiB) con `enGrabacion`; y una captura. Todo
/// llega al movil con sus ficheros, el texto que sale despues tambien, y un
/// marcador puesto en cada aparato antes de sincronizar deja los dos.
#[test]
fn una_clase_grabada_en_el_pc_llega_al_movil_con_voz_video_capturas_y_el_texto_de_despues() {
    use pixpin_proyecto::clase;
    let p = montar("clase-pc");
    let raiz = p.raiz();
    let ficha = ficha_pc(&p, "Estructuras");
    let carpeta = almacen::carpeta(&raiz, &ficha.id);
    let audio = vec![7u8; 300_000];
    let ruta_voz = almacen::guardar_adjunto(&raiz, &ficha.id, "Clase 25-oct.m4a", &audio).unwrap();
    let mut voz = Mensaje::adjunto(
        Clase::Voz,
        "Clase 25-oct.m4a",
        &ruta_voz,
        audio.len() as i64,
        &sello_pc(&p, &ficha, 1),
    );
    voz.duracion_ms = 90_000;
    // Mas de un tramo del cable y por encima del umbral de la cache de
    // resumenes: un video de verdad.
    let video: Vec<u8> = (0..(5usize << 20) + 123).map(|i| (i % 251) as u8).collect();
    std::fs::write(carpeta.join("archivos/Clase 25-oct.mp4"), &video).unwrap();
    let mut mp4 = Mensaje::adjunto(
        Clase::Archivo,
        "Clase 25-oct.mp4",
        "archivos/Clase 25-oct.mp4",
        video.len() as i64,
        &sello_pc(&p, &ficha, 2),
    );
    mp4.resto
        .insert("enGrabacion".into(), json!({"de": voz.id, "ms": 0}));
    let ruta_foto =
        almacen::guardar_adjunto(&raiz, &ficha.id, "clase-1-1.png", b"PNG captura").unwrap();
    let mut foto = Mensaje::adjunto(
        Clase::Imagen,
        "clase-1-1.png",
        &ruta_foto,
        11,
        &sello_pc(&p, &ficha, 3),
    );
    foto.texto = "⏱ 01:05 · Clase 25-oct".into();
    foto.resto
        .insert("enGrabacion".into(), json!({"de": voz.id, "ms": 65_000}));
    voz.resto.insert("grabacion".into(), json!("clase"));
    voz.resto
        .insert("capturas".into(), json!([{"ms": 65_000, "id": foto.id}]));
    voz.resto.insert("video".into(), json!({"id": mp4.id}));
    clase::poner_marcador(&mut voz.resto, 30_000, "ojo: examen");
    for m in [&voz, &mp4, &foto] {
        cuaderno::anadir(&carpeta, m).unwrap();
    }
    let chat = vista::chat_de_ficha(&raiz, &ficha.id).unwrap();

    p.vuelta_desde_pc();

    let v = mensaje_movil(&p, &voz.id);
    assert_eq!(v["clase"], "VOZ");
    assert_eq!(v["grabacion"], "clase");
    assert_eq!(v["capturas"][0]["id"], foto.id.as_str());
    assert_eq!(v["video"]["id"], mp4.id.as_str());
    assert_eq!(v["marcadores"][0]["texto"], "ojo: examen");
    assert_eq!(
        mensaje_movil(&p, &mp4.id)["enGrabacion"]["de"],
        voz.id.as_str()
    );
    assert_eq!(mensaje_movil(&p, &foto.id)["enGrabacion"]["ms"], 65_000);
    assert_eq!(abrir_en_el_movil(&p, &voz.id).as_deref(), Some(&audio[..]));
    assert!(
        abrir_en_el_movil(&p, &mp4.id).as_deref() == Some(&video[..]),
        "el video llega entero y byte a byte"
    );
    assert_eq!(
        abrir_en_el_movil(&p, &foto.id).as_deref(),
        Some(&b"PNG captura"[..])
    );
    p.quieto(&chat);

    // El texto sale despues (Whisper tarda): la nota cambia y viaja.
    let mut m = mensaje_pc(&p, &ficha, &voz.id);
    m.transcripcion = Some("[0:00] Hoy vemos vigas\n\n[0:40] Y columnas".into());
    m.resto.insert("estadoDelTexto".into(), json!("LISTO"));
    cuaderno::reemplazar(&carpeta, &m).unwrap();
    p.vuelta_desde_pc();
    let v = mensaje_movil(&p, &voz.id);
    assert_eq!(
        v["transcripcion"],
        "[0:00] Hoy vemos vigas\n\n[0:40] Y columnas"
    );
    assert_eq!(v["estadoDelTexto"], "LISTO");

    // Un marcador en cada aparato antes de sincronizar: quedan los dos, en
    // orden, en los dos lados. Y el texto retocado en el movil tambien.
    cambiar_en_movil(&p, &voz.id, |v| {
        v["marcadores"]
            .as_array_mut()
            .unwrap()
            .push(json!({"ms": 120_000, "texto": "desde el movil"}));
        v["transcripcion"] = json!("[0:00] Hoy vemos vigas\n\n[0:40] Y columnas de hormigon");
    });
    let mut m = mensaje_pc(&p, &ficha, &voz.id);
    clase::poner_marcador(&mut m.resto, 60_000, "desde el PC");
    cuaderno::reemplazar(&carpeta, &m).unwrap();
    p.vuelta_desde_pc();
    let v = mensaje_movil(&p, &voz.id);
    assert_eq!(ms_de_marcadores(&v), [30_000, 60_000, 120_000], "{v}");
    let en_pc = mensaje_pc(&p, &ficha, &voz.id);
    assert_eq!(
        clase::marcadores_de(&en_pc.resto)
            .iter()
            .map(|m| m.ms)
            .collect::<Vec<_>>(),
        [30_000, 60_000, 120_000]
    );
    assert_eq!(
        en_pc.transcripcion.as_deref(),
        Some("[0:00] Hoy vemos vigas\n\n[0:40] Y columnas de hormigon")
    );
    p.quieto(&chat);
}

/// **Una clase grabada en el movil** llega al PC igual: la voz en `voz/`, el
/// video en `guardados/`, los campos en `resto`, y su texto de despues.
#[test]
fn una_clase_grabada_en_el_movil_llega_al_pc_con_sus_ficheros_y_su_texto_de_despues() {
    let p = montar("clase-movil");
    let m = &p.movil;
    escribir_movil(&p, "voz/voz_1761400000000.m4a", "audio del movil");
    escribir_movil(&p, "guardados/1761400000001_clase.mp4", "video del movil");
    let (voz, mp4) = ("v-movil", "mp4-movil");
    for (id, clase, rel, extra) in [
        (
            voz,
            "VOZ",
            "voz/voz_1761400000000.m4a",
            json!({"grabacion": "clase", "capturas": [], "video": {"id": mp4},
                   "marcadores": [{"ms": 5_000, "texto": "inicio"}], "duracionMs": 60_000}),
        ),
        (
            mp4,
            "ARCHIVO",
            "guardados/1761400000001_clase.mp4",
            json!({"enGrabacion": {"de": voz, "ms": 0}, "nombre": "clase.mp4"}),
        ),
    ] {
        let mut v = json!({"id": id, "cuando": p.reloj.tic(), "clase": clase,
                           "ruta": m.absoluta(rel), "numero": m.leer_mensajes().len() + 1,
                           "letra": "a"});
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        m.anadir_mensaje(&Json::de_valor(&v)).unwrap();
    }

    p.vuelta_desde_pc();

    let g = p.pc.ficha_de(GENERAL).unwrap();
    let v = mensaje_pc(&p, &g, voz);
    assert_eq!(v.resto["grabacion"], "clase");
    assert_eq!(v.resto["video"]["id"], mp4);
    assert_eq!(
        pixpin_proyecto::clase::marcadores_de(&v.resto)[0].texto,
        "inicio"
    );
    assert_eq!(
        std::fs::read(doc_en_pc(&p, voz)).unwrap(),
        b"audio del movil"
    );
    assert_eq!(
        std::fs::read(doc_en_pc(&p, mp4)).unwrap(),
        b"video del movil"
    );
    assert_eq!(mensaje_pc(&p, &g, mp4).resto["enGrabacion"]["de"], voz);

    cambiar_en_movil(&p, voz, |v| v["transcripcion"] = json!("[0:00] hola"));
    p.vuelta_desde_pc();
    assert_eq!(
        mensaje_pc(&p, &g, voz).transcripcion.as_deref(),
        Some("[0:00] hola")
    );
    p.quieto(GENERAL);
}

fn figura_guardada(id: &str, nombre: &str) -> serde_json::Value {
    json!({"id": id, "nombre": nombre, "propia": true, "elementos": [
        {"id": format!("{id}-p"), "type": "rectangle", "x": 0, "y": 0, "width": 10,
         "height": 10, "version": 1, "versionNonce": 3, "updated": 1}
    ]})
}

fn ids_de_figuras(texto: &str) -> Vec<String> {
    serde_json::from_str::<Vec<serde_json::Value>>(texto)
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap().to_string())
        .collect()
}

/// **La biblioteca de figuras** (`pins/draw/figuras.json`, senalada por el
/// mensaje `auto-figuras` del chat «Asistente»): viaja, y si se guardo una
/// figura en cada aparato antes de sincronizar quedan las dos en los dos
/// (antes ganaba el fichero tocado mas tarde y la otra se perdia).
#[test]
fn la_biblioteca_de_figuras_viaja_y_guardar_una_en_cada_aparato_no_pierde_ninguna() {
    let p = montar("figuras");
    let rel = "pins/draw/figuras.json";
    let serie = json!([figura_guardada("serie-norte", "Norte")]).to_string();
    escribir_movil(&p, rel, &serie);
    p.movil
        .guardar_proyecto(
            &kotlin::normalizar_proyecto(&Json::de_valor(&json!({
                "id": "asis", "nombre": "Asistente", "tocado": 5, "asistente": true,
                "uid": "AAAAAAAAAA", "creado": 1_726_000_000_000_i64, "aparato": "MOVI",
            })))
            .unwrap(),
        )
        .unwrap();
    p.movil
        .anadir_mensaje(&Json::de_valor(&json!({
            "id": "auto-figuras", "cuando": p.reloj.tic(), "clase": "ARCHIVO",
            "ruta": p.movil.absoluta(rel), "nombre": "figuras.json", "proyecto": "asis",
            "numero": 1, "letra": "a",
        })))
        .unwrap();

    p.vuelta_desde_pc();
    let en_pc = p.pc.ruta("asis", rel);
    assert_eq!(
        ids_de_figuras(&std::fs::read_to_string(&en_pc).unwrap()),
        ["serie-norte"]
    );

    // Una en cada aparato.
    p.reloj.saltar(10_000);
    let pc = json!([
        figura_guardada("serie-norte", "Norte"),
        figura_guardada("f-pc", "Sello del PC")
    ]);
    std::fs::write(&en_pc, pc.to_string()).unwrap();
    adelantar(&en_pc);
    let movil = json!([
        figura_guardada("serie-norte", "Norte"),
        figura_guardada("f-movil", "Sello del movil")
    ]);
    escribir_movil(&p, rel, &movil.to_string());

    p.vuelta_desde_pc();
    let aqui = ids_de_figuras(&std::fs::read_to_string(&en_pc).unwrap());
    let alli = ids_de_figuras(&leer_movil(&p, rel).unwrap());
    assert_eq!(aqui, ["serie-norte", "f-pc", "f-movil"]);
    assert_eq!(alli, aqui, "los dos quedan iguales");
    // Lo que sale lo lee la biblioteca del PC (y es el formato del movil).
    let leidas = pixpin_motor2d::biblioteca::desde_json(&std::fs::read_to_string(&en_pc).unwrap());
    assert_eq!(leidas.len(), 3);
    p.quieto("asis");

    // Caso negativo: borrar una figura en un solo aparato sigue borrandola
    // en los dos (no hay choque: solo cambio un lado).
    p.reloj.saltar(10_000);
    let sin = json!([
        figura_guardada("serie-norte", "Norte"),
        figura_guardada("f-movil", "Sello del movil")
    ]);
    std::fs::write(&en_pc, sin.to_string()).unwrap();
    adelantar(&en_pc);
    p.vuelta_desde_pc();
    assert_eq!(
        ids_de_figuras(&leer_movil(&p, rel).unwrap()),
        ["serie-norte", "f-movil"]
    );
}

/// **Una carpeta de paginas** (`carpeta_de_paginas.rs`, un `MINIAPP`
/// «carpeta» con su documento en `texto`): viaja, y una pagina anadida en
/// cada aparato antes de sincronizar deja las dos.
#[test]
fn una_carpeta_de_paginas_viaja_y_una_pagina_en_cada_aparato_deja_las_dos() {
    use pixpin_proyecto::carpeta_de_paginas::{self as cp, Carpeta};
    let p = montar("carpeta-paginas");
    let raiz = p.raiz();
    let ficha = ficha_pc(&p, "Diario");
    let carpeta = almacen::carpeta(&raiz, &ficha.id);
    let dia1 = p.nota_pc(&ficha, "dia 1");
    let mut c = cp::mensaje_nuevo("Diario de obra", &sello_pc(&p, &ficha, 2));
    let mut doc = Carpeta::default();
    doc.anadir(&dia1.id, "Día 1");
    c.texto = doc.escribir();
    cuaderno::anadir(&carpeta, &c).unwrap();
    let chat = vista::chat_de_ficha(&raiz, &ficha.id).unwrap();

    p.vuelta_desde_pc();
    let v = mensaje_movil(&p, &c.id);
    assert_eq!(v["clase"], "MINIAPP");
    assert_eq!(v["miniapp"], "carpeta");
    assert_eq!(v["nombre"], "Diario de obra");
    let alli = Carpeta::leer(v["texto"].as_str().unwrap()).unwrap();
    assert_eq!(alli.paginas[0].id, dia1.id);

    // El PC anade el dia 2; el movil, a la vez, una pagina suya.
    let dia2 = p.nota_pc(&ficha, "dia 2");
    let mut m = mensaje_pc(&p, &ficha, &c.id);
    let mut doc = Carpeta::leer(&m.texto).unwrap();
    doc.anadir(&dia2.id, "Día 2");
    m.texto = doc.escribir();
    cuaderno::reemplazar(&carpeta, &m).unwrap();
    cambiar_en_movil(&p, &c.id, |v| {
        let mut doc = Carpeta::leer(v["texto"].as_str().unwrap()).unwrap();
        doc.anadir("pagina-del-movil", "Foto del movil");
        v["texto"] = json!(doc.escribir());
    });

    p.vuelta_desde_pc();
    let ids = |texto: &str| -> Vec<String> {
        Carpeta::leer(texto)
            .unwrap()
            .paginas
            .into_iter()
            .map(|p| p.id)
            .collect()
    };
    let en_pc = ids(&mensaje_pc(&p, &ficha, &c.id).texto);
    assert_eq!(
        en_pc,
        [dia1.id.clone(), dia2.id.clone(), "pagina-del-movil".into()]
    );
    assert_eq!(
        ids(mensaje_movil(&p, &c.id)["texto"].as_str().unwrap()),
        en_pc
    );
    p.quieto(&chat);
}

/// **La carpeta de una clase grabada** (25-oct-2026): viaja con su tipo,
/// su nota, su video, lo que duro y el minuto de cada pieza; un adjunto a
/// la clase en cada aparato deja los dos, marcados como adjuntos y aparte
/// de lo grabado, que no cambia.
#[test]
fn una_carpeta_de_grabacion_viaja_y_un_adjunto_en_cada_aparato_deja_los_dos() {
    use pixpin_proyecto::carpeta_de_paginas::{self as cp, Carpeta};
    let p = montar("carpeta-grabacion");
    let raiz = p.raiz();
    let ficha = ficha_pc(&p, "Clases");
    let carpeta = almacen::carpeta(&raiz, &ficha.id);
    // La nota, el video y una captura (aqui notas: lo que importa es la
    // carpeta, los adjuntos de una clase ya se prueban arriba).
    let voz = p.nota_pc(&ficha, "la nota de la clase");
    let video = p.nota_pc(&ficha, "el video");
    let foto = p.nota_pc(&ficha, "la captura");
    let mut doc = Carpeta::grabacion(&voz.id, 5_400_000);
    doc.video = video.id.clone();
    doc.anadir_al_minuto(&voz.id, "", 0);
    doc.anadir_al_minuto(&video.id, "", 0);
    doc.anadir_al_minuto(&foto.id, "⏱ 1:05", 65_000);
    let c = cp::mensaje_de_grabacion("Clase 25-10 10h58", &doc, &sello_pc(&p, &ficha, 4));
    cuaderno::anadir(&carpeta, &c).unwrap();
    let chat = vista::chat_de_ficha(&raiz, &ficha.id).unwrap();

    p.vuelta_desde_pc();
    let v = mensaje_movil(&p, &c.id);
    assert_eq!(v["clase"], "MINIAPP");
    assert_eq!(v["miniapp"], "carpeta");
    assert_eq!(v["nombre"], "Clase 25-10 10h58");
    let alli = Carpeta::leer(v["texto"].as_str().unwrap()).unwrap();
    assert_eq!(alli, doc, "igual en el movil, campo a campo");
    assert!(alli.es_grabacion());
    assert_eq!(alli.paginas[2].ms, Some(65_000));

    // El PC le adjunta unos apuntes; el movil, a la vez, una foto suya.
    let apuntes = p.nota_pc(&ficha, "apuntes del profesor");
    let mut m = mensaje_pc(&p, &ficha, &c.id);
    let mut d = Carpeta::leer(&m.texto).unwrap();
    d.adjuntar(&apuntes.id, "Apuntes");
    m.texto = d.escribir();
    cuaderno::reemplazar(&carpeta, &m).unwrap();
    cambiar_en_movil(&p, &c.id, |v| {
        let mut d = Carpeta::leer(v["texto"].as_str().unwrap()).unwrap();
        d.adjuntar("foto-del-movil", "Pizarra");
        v["texto"] = json!(d.escribir());
    });

    p.vuelta_desde_pc();
    let en_pc = Carpeta::leer(&mensaje_pc(&p, &ficha, &c.id).texto).unwrap();
    let ids = |v: Vec<&cp::Pagina>| v.into_iter().map(|p| p.id.clone()).collect::<Vec<_>>();
    assert!(en_pc.es_grabacion());
    assert_eq!(
        (en_pc.voz.as_str(), en_pc.duracion_ms),
        (voz.id.as_str(), 5_400_000)
    );
    assert_eq!(
        ids(en_pc.de_la_grabacion().collect()),
        [voz.id.clone(), video.id.clone(), foto.id.clone()],
        "lo grabado no cambia"
    );
    let mut adjuntos = ids(en_pc.adjuntos().collect());
    adjuntos.sort();
    let mut esperados = vec![apuntes.id.clone(), "foto-del-movil".to_string()];
    esperados.sort();
    assert_eq!(adjuntos, esperados, "los dos adjuntos, marcados");
    assert_eq!(en_pc.capturas().count(), 1, "un adjunto no es una captura");
    let en_movil = Carpeta::leer(mensaje_movil(&p, &c.id)["texto"].as_str().unwrap()).unwrap();
    assert_eq!(en_movil, en_pc);
    p.quieto(&chat);
}

/// **La nota rica** (Markdown con fotos en `notas/`): llegan la nota y sus
/// fotos, y en el movil la ruta de cada foto es un fichero que existe.
#[test]
fn una_nota_rica_del_pc_llega_al_movil_con_sus_fotos() {
    let p = montar("nota-rica");
    let raiz = p.raiz();
    let ficha = ficha_pc(&p, "Visita");
    let carpeta = almacen::carpeta(&raiz, &ficha.id);
    std::fs::create_dir_all(carpeta.join("notas")).unwrap();
    std::fs::write(carpeta.join("notas/1761400000000-pegado.png"), b"png 1").unwrap();
    std::fs::write(carpeta.join("notas/1761400000001-planta.jpg"), b"jpg 2").unwrap();
    let notas = vista::portatil_de_ruta(&raiz, &carpeta.join("notas")).unwrap();
    let nota = p.nota_pc(
        &ficha,
        &format!(
            "Visita\n\n**Pendiente:** la escalera\n![]({notas}/1761400000000-pegado.png)\n![planta|300]({notas}/1761400000001-planta.jpg)"
        ),
    );
    let chat = vista::chat_de_ficha(&raiz, &ficha.id).unwrap();

    p.vuelta_desde_pc();

    let v = mensaje_movil(&p, &nota.id);
    let texto = v["texto"].as_str().unwrap();
    assert!(texto.contains("**Pendiente:**"), "{texto}");
    for (f, bytes) in [
        ("1761400000000-pegado.png", &b"png 1"[..]),
        ("1761400000001-planta.jpg", &b"jpg 2"[..]),
    ] {
        let rel = format!(
            "guardados/pc/{}/notas/{f}",
            pixpin_sincro::disco::limpio(&chat)
        );
        assert_eq!(
            std::fs::read(p.movil.raiz().join(&rel)).ok().as_deref(),
            Some(bytes),
            "{rel}"
        );
    }
    p.quieto(&chat);
}
