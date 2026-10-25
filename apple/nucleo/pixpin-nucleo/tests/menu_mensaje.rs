//! El menu de un mensaje del chat (`menu_de_mensaje` de Windows): lo que
//! cambia en el cuaderno al fijar, etiquetar, describir, renombrar,
//! recordar, guardar del buzon, reenviar y hacer leccion. Los campos y su
//! formato son los de Windows y el movil: si cambian aqui, no viajan.

use std::path::PathBuf;
use std::sync::Arc;

use pixpin_nucleo::chats::{self, CambioMensaje};
use pixpin_nucleo::lecciones::{repartir_para_leccion, texto_para_leccion};
use pixpin_nucleo::{LeccionEditada, Nucleo};
use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje};

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-menu-mensaje-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
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

/// La linea del cuaderno de `id`, tal cual esta en el disco.
fn linea(n: &Nucleo, chat: &str, id: &str) -> serde_json::Value {
    let carpeta = almacen::carpeta(&PathBuf::from(n.raiz()), chat);
    let texto = std::fs::read_to_string(carpeta.join("guardados.jsonl")).unwrap();
    texto
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .find(|v| v["id"] == id)
        .unwrap()
}

fn mensaje(clase: Clase, ruta: Option<&str>) -> Mensaje {
    Mensaje {
        id: "1".into(),
        clase: Some(clase),
        ruta: ruta.map(str::to_string),
        ..Default::default()
    }
}

// ── Lo puro, portado de las pruebas de Windows ──

#[test]
fn conserva_la_extension() {
    // `renombrar::pruebas::conserva_la_extension` de Windows.
    assert_eq!(
        chats::con_su_extension("informe (1).pdf", "  Memoria "),
        "Memoria.pdf"
    );
    assert_eq!(
        chats::con_su_extension("informe.pdf", "Memoria.PDF"),
        "Memoria.PDF"
    );
    assert_eq!(
        chats::con_su_extension("voz_123.m4a", "Clase de física"),
        "Clase de física.m4a"
    );
    assert_eq!(
        chats::con_su_extension("Nota de voz 14:32", "Clase 3"),
        "Clase 3"
    );
    assert_eq!(chats::con_su_extension("a.pdf", "   "), "");
    assert_eq!(chats::con_su_extension("x.pdf", "a/b"), "a-b.pdf");
}

#[test]
fn que_se_puede_renombrar_y_describir() {
    // `renombrar::pruebas::que_se_puede_renombrar` y
    // `descripcion::pruebas::se_describe_una_foto_y_un_archivo`.
    assert!(chats::se_puede_renombrar(&mensaje(Clase::Voz, Some("archivos/x.m4a"))));
    assert!(chats::se_puede_renombrar(&mensaje(Clase::Imagen, Some("archivos/x.png"))));
    assert!(chats::se_puede_renombrar(&mensaje(Clase::Archivo, Some("archivos/x.pdf"))));
    assert!(chats::se_puede_describir(&mensaje(Clase::Imagen, Some("archivos/x.png"))));
    assert!(chats::se_puede_describir(&mensaje(Clase::Archivo, Some("archivos/x.pdf"))));
    // Casos negativos: una nota sin fichero, una leccion y lo del buzon.
    assert!(!chats::se_puede_renombrar(&mensaje(Clase::Nota, None)));
    assert!(!chats::se_puede_describir(&mensaje(Clase::Nota, None)));
    let leccion = mensaje(
        Clase::Archivo,
        Some("pixpin:files/guardados/lecciones/a.leccion"),
    );
    assert!(!chats::se_puede_renombrar(&leccion));
    assert!(!chats::se_puede_describir(&leccion));
    let mut buzon = mensaje(Clase::Imagen, Some("archivos/x.png"));
    buzon.en_buzon = true;
    assert!(!chats::se_puede_renombrar(&buzon));
    assert!(!chats::se_puede_describir(&buzon));
    assert!(!chats::se_puede_describir(&mensaje(Clase::Imagen, None)));
}

#[test]
fn la_descripcion_se_pone_se_cambia_y_se_quita() {
    // `descripcion::pruebas::la_descripcion_se_pone_se_cambia_y_se_quita`.
    let mut m = mensaje(Clase::Imagen, Some("archivos/foto.png"));
    let poner = CambioMensaje::Descripcion {
        texto: "  La pared norte  ".into(),
    };
    assert!(chats::aplicar(&mut m, &poner));
    assert_eq!(m.texto, "La pared norte");
    assert_eq!(m.id, "1", "es el mismo mensaje, no uno nuevo");
    // Caso negativo: lo mismo de antes no reescribe nada.
    let igual = CambioMensaje::Descripcion {
        texto: "La pared norte ".into(),
    };
    assert!(!chats::aplicar(&mut m, &igual));
    assert!(chats::aplicar(
        &mut m,
        &CambioMensaje::Descripcion { texto: "   ".into() }
    ));
    assert!(m.texto.is_empty());
}

#[test]
fn quitar_la_hora_deja_el_mensaje_como_estaba() {
    // `recordatorios`: se borra la clave, no se deja a `null`, para que el
    // resumen de sincronizar no cambie.
    let mut m = mensaje(Clase::Nota, None);
    let antes = serde_json::to_string(&m).unwrap();
    assert!(chats::aplicar(
        &mut m,
        &CambioMensaje::Recordar {
            cuando: Some(1_758_351_600_000)
        }
    ));
    assert_eq!(chats::hora_de_recordar(&m), Some(1_758_351_600_000));
    let puesta = serde_json::to_string(&m).unwrap();
    assert!(puesta.contains("\"recuerdaEn\":1758351600000"), "{puesta}");
    assert!(chats::aplicar(&mut m, &CambioMensaje::Recordar { cuando: None }));
    assert_eq!(serde_json::to_string(&m).unwrap(), antes);
    // Caso negativo: quitar lo que no hay no cambia nada; una cadena no es
    // una hora.
    assert!(!chats::aplicar(&mut m, &CambioMensaje::Recordar { cuando: None }));
    m.resto.insert("recuerdaEn".into(), "manana".into());
    assert_eq!(chats::hora_de_recordar(&m), None);
}

#[test]
fn el_nombre_de_un_lienzo_se_recorta_sin_extension() {
    let mut m = mensaje(Clase::Dibujo, None);
    m.referencia = Some("abc".into());
    m.nombre = "Planta baja".into();
    let largo = "x".repeat(200);
    assert!(chats::aplicar(&mut m, &CambioMensaje::Nombre { nombre: largo }));
    assert_eq!(m.nombre.chars().count(), 80);
    // Caso negativo: vacio no borra el nombre.
    assert!(!chats::aplicar(&mut m, &CambioMensaje::Nombre { nombre: "  ".into() }));
    assert_eq!(m.nombre.chars().count(), 80);
}

#[test]
fn las_etiquetas_son_las_del_movil() {
    assert_eq!(
        chats::ETIQUETAS,
        ["\u{2b50}", "\u{2705}", "\u{23f3}", "\u{1f4a1}", "\u{1f4b0}", "\u{1f4cd}"]
    );
    let mut m = mensaje(Clase::Nota, None);
    assert!(chats::aplicar(
        &mut m,
        &CambioMensaje::Etiqueta {
            emoji: Some("\u{2b50}".into())
        }
    ));
    assert_eq!(m.emoji.as_deref(), Some("\u{2b50}"));
    // Caso negativo: la misma otra vez no cambia; vacia es quitarla.
    assert!(!chats::aplicar(
        &mut m,
        &CambioMensaje::Etiqueta {
            emoji: Some("\u{2b50}".into())
        }
    ));
    assert!(chats::aplicar(
        &mut m,
        &CambioMensaje::Etiqueta {
            emoji: Some(" ".into())
        }
    ));
    assert_eq!(m.emoji, None);
}

#[test]
fn la_leccion_sale_de_lo_dicho_su_texto_o_su_nombre() {
    let mut m = mensaje(Clase::Voz, Some("archivos/v.m4a"));
    m.nombre = "voz_1.m4a".into();
    m.texto = "pie".into();
    m.transcripcion = Some("## **Encofrar** antes de llover".into());
    assert_eq!(texto_para_leccion(&m).as_deref(), Some("Encofrar antes de llover"));
    m.transcripcion = None;
    assert_eq!(texto_para_leccion(&m).as_deref(), Some("pie"));
    m.texto = " ".into();
    assert_eq!(texto_para_leccion(&m).as_deref(), Some("voz_1.m4a"));
    // Caso negativo: sin nada que decir no hay leccion.
    m.nombre = String::new();
    assert_eq!(texto_para_leccion(&m), None);
    let (c, etiquetas) = repartir_para_leccion("Revisar el plano #obra");
    assert_eq!(etiquetas, ["obra"]);
    assert!(!c.titulo.contains('#'), "{}", c.titulo);
}

// ── Contra el disco, por la API del puente ──

#[test]
fn fijar_etiquetar_y_guardar_del_buzon_reescriben_el_mismo_mensaje() {
    let t = Temporal::nueva("cambiar");
    let n = t.nucleo("Mac");
    let obra = n.crear_chat("Obra".into()).unwrap();
    let a = n.escribir_nota(obra.id.clone(), "uno".into(), None).unwrap();
    let b = n.escribir_nota(obra.id.clone(), "dos".into(), None).unwrap();

    let fijado = n
        .cambiar_mensaje(obra.id.clone(), a.id.clone(), CambioMensaje::Fijar { fijado: true })
        .unwrap()
        .unwrap();
    assert!(fijado.fijado);
    assert_eq!(linea(&n, &obra.id, &a.id)["fijado"], true);
    n.cambiar_mensaje(
        obra.id.clone(),
        a.id.clone(),
        CambioMensaje::Etiqueta {
            emoji: Some("\u{2705}".into()),
        },
    )
    .unwrap();
    assert_eq!(linea(&n, &obra.id, &a.id)["emoji"], "\u{2705}");
    let puesto = n
        .cambiar_mensaje(
            obra.id.clone(),
            b.id.clone(),
            CambioMensaje::Recordar {
                cuando: Some(1_900_000_000_000),
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(puesto.recuerda_en, Some(1_900_000_000_000));
    assert_eq!(linea(&n, &obra.id, &b.id)["recuerdaEn"], 1_900_000_000_000i64);

    // Lo del buzon se queda con «Guardar esto».
    let carpeta = almacen::carpeta(&PathBuf::from(n.raiz()), &obra.id);
    cuaderno::cambiar(&carpeta, &b.id, |m| {
        m.en_buzon = true;
        true
    })
    .unwrap();
    let pagina = n.ultimos_mensajes(obra.id.clone(), 10).unwrap();
    assert!(pagina.mensajes.iter().any(|m| m.id == b.id && m.en_buzon));
    let guardado = n
        .cambiar_mensaje(obra.id.clone(), b.id.clone(), CambioMensaje::Rescatar)
        .unwrap()
        .unwrap();
    assert!(!guardado.en_buzon);
    assert_eq!(linea(&n, &obra.id, &b.id)["enBuzon"], false);

    // La pagina se relee con lo cambiado y en el mismo sitio.
    let pagina = n.ultimos_mensajes(obra.id.clone(), 10).unwrap();
    assert_eq!(pagina.total, 2);
    assert_eq!(pagina.mensajes[0].id, a.id);
    assert!(pagina.mensajes[0].fijado);

    // Casos negativos: un id que no esta y un chat que no vale.
    assert_eq!(
        n.cambiar_mensaje(obra.id.clone(), "no-esta".into(), CambioMensaje::Rescatar)
            .unwrap(),
        None
    );
    assert!(n
        .cambiar_mensaje("../x".into(), a.id.clone(), CambioMensaje::Rescatar)
        .is_err());
}

#[test]
fn renombrar_y_describir_un_adjunto_no_tocan_su_fichero() {
    let t = Temporal::nueva("renombrar");
    let n = t.nucleo("Mac");
    let obra = n.crear_chat("Obra".into()).unwrap();
    let fuera = t.0.join("fuera");
    std::fs::create_dir_all(&fuera).unwrap();
    let pdf = fuera.join("informe.pdf");
    std::fs::write(&pdf, b"%PDF-1.4 falso").unwrap();
    let m = n
        .adjuntar(obra.id.clone(), pdf.display().to_string(), None)
        .unwrap();
    assert!(m.se_puede_renombrar && m.se_puede_describir);
    let r = n
        .cambiar_mensaje(
            obra.id.clone(),
            m.id.clone(),
            CambioMensaje::Nombre {
                nombre: "Memoria".into(),
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(r.nombre, "Memoria.pdf");
    assert_eq!(r.ruta, m.ruta, "la ruta no cambia: la sincro empareja por ella");
    let d = n
        .cambiar_mensaje(
            obra.id.clone(),
            m.id.clone(),
            CambioMensaje::Descripcion {
                texto: "la buena".into(),
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(d.texto, "la buena");
    assert_eq!(linea(&n, &obra.id, &m.id)["texto"], "la buena");
    assert!(n
        .ruta_absoluta(obra.id.clone(), m.ruta.clone().unwrap())
        .map(|r| PathBuf::from(r).is_file())
        .unwrap());
}

#[test]
fn reenviar_copia_el_mensaje_y_su_fichero_al_otro_chat() {
    let t = Temporal::nueva("reenviar");
    let n = t.nucleo("Mac");
    let obra = n.crear_chat("Obra".into()).unwrap();
    let casa = n.crear_chat("Casa".into()).unwrap();
    n.escribir_nota(casa.id.clone(), "ya habia algo".into(), None)
        .unwrap();
    let nota = n.escribir_nota(obra.id.clone(), "medir la puerta".into(), None).unwrap();
    n.cambiar_mensaje(obra.id.clone(), nota.id.clone(), CambioMensaje::Fijar { fijado: true })
        .unwrap();
    let fuera = t.0.join("fuera");
    std::fs::create_dir_all(&fuera).unwrap();
    let foto = fuera.join("puerta.png");
    std::fs::write(&foto, b"\x89PNG\r\n\x1a\nfalso").unwrap();
    let f = n
        .adjuntar(obra.id.clone(), foto.display().to_string(), None)
        .unwrap();

    let hechos = n
        .reenviar_mensajes(obra.id.clone(), vec![f.id.clone(), nota.id.clone()], casa.id.clone())
        .unwrap();
    assert_eq!(hechos, 2);
    let alli = n.ultimos_mensajes(casa.id.clone(), 10).unwrap();
    assert_eq!(alli.total, 3);
    // Del mas viejo al mas nuevo: primero la nota, luego la foto.
    let (copia_nota, copia_foto) = (&alli.mensajes[1], &alli.mensajes[2]);
    assert_eq!(copia_nota.texto, "medir la puerta");
    assert!(!copia_nota.fijado, "la copia no va fijada");
    assert_ne!(copia_nota.id, nota.id);
    assert_eq!(copia_foto.clase, "IMAGEN");
    let crudo = linea(&n, &casa.id, &copia_nota.id);
    assert_eq!(crudo["proyecto"], casa.id.as_str());
    assert_eq!(crudo["numero"], 2, "sigue la cuenta del otro chat");
    let original = linea(&n, &obra.id, &nota.id);
    assert_eq!(crudo["origen"], original["uid"], "origen: el uid del de aqui");
    assert_ne!(crudo["uid"], original["uid"]);
    // El fichero se copio dentro del otro proyecto.
    let ruta = n
        .ruta_absoluta(casa.id.clone(), copia_foto.ruta.clone().unwrap())
        .unwrap();
    assert!(ruta.contains(&casa.id), "{ruta}");
    assert_eq!(std::fs::read(&ruta).unwrap(), std::fs::read(&foto).unwrap());
    // La lista sube el chat de destino con su ultima linea.
    let ficha = n.chats().unwrap().into_iter().find(|c| c.id == casa.id).unwrap();
    assert_eq!(ficha.resumen, copia_foto.resumen);
    // El de aqui sigue igual.
    assert_eq!(n.ultimos_mensajes(obra.id.clone(), 10).unwrap().total, 2);

    // Casos negativos: un destino que no existe y nada que reenviar.
    assert!(n
        .reenviar_mensajes(obra.id.clone(), vec![nota.id.clone()], "no-esta".into())
        .is_err());
    assert_eq!(
        n.reenviar_mensajes(obra.id.clone(), vec!["nada".into()], casa.id.clone())
            .unwrap(),
        0
    );
}

#[test]
fn una_leccion_hecha_de_un_mensaje_apunta_a_el() {
    let t = Temporal::nueva("leccion");
    let n = t.nucleo("Mac");
    let obra = n.crear_chat("Obra".into()).unwrap();
    let nota = n
        .escribir_nota(obra.id.clone(), "Pedir el hormigon con tiempo #obra".into(), None)
        .unwrap();
    let b = n
        .leccion_de_mensaje(obra.id.clone(), nota.id.clone())
        .unwrap()
        .unwrap();
    assert_eq!(b.de_mensaje, nota.id);
    assert_eq!(b.etiquetas, ["obra"]);
    let guardada = n
        .guardar_leccion(LeccionEditada {
            id: None,
            titulo: b.titulo.clone(),
            que_paso: b.que_paso,
            por_que: b.por_que,
            proxima: b.proxima,
            tipo: String::new(),
            area: String::new(),
            gravedad: 1,
            etiquetas: b.etiquetas,
            etiquetas_auto: vec![],
            quitadas: vec![],
            causas: vec![],
            chat: Some(obra.id.clone()),
            de_mensaje: Some(nota.id.clone()),
        })
        .unwrap();
    // El fichero de la leccion lleva `deMensaje`, como el de Windows.
    let raiz = PathBuf::from(n.raiz());
    let carpeta = almacen::carpeta(&raiz, &obra.id);
    let mut leido = None;
    for e in walk(&carpeta) {
        if e.extension().is_some_and(|x| x == "leccion") {
            leido = Some(std::fs::read_to_string(&e).unwrap());
        }
    }
    let leido = leido.expect("el fichero de la leccion");
    assert!(leido.contains(&format!("\"deMensaje\":\"{}\"", nota.id))
        || leido.contains(&format!("\"deMensaje\": \"{}\"", nota.id)), "{leido}");
    assert!(!guardada.id.is_empty());
    // Caso negativo: un mensaje que no esta.
    assert_eq!(n.leccion_de_mensaje(obra.id.clone(), "no".into()).unwrap(), None);
}

/// Todos los ficheros de debajo de `dir`.
fn walk(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                v.extend(walk(&p));
            } else {
                v.push(p);
            }
        }
    }
    v
}
