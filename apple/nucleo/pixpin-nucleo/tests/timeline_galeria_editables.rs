//! El timeline editable (B5) y el buscador de la galeria (B6), tal como los
//! usa Swift: carpetas de datos de verdad en un temporal y los ficheros
//! escritos como los escribe Windows.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use pixpin_nucleo::timeline::{CirculoDia, ExportarTimeline, FotoNueva};
use pixpin_nucleo::{ErrorNucleo, Nucleo};
use pixpin_timeline::almacen::Almacen;

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-nucleo-tge-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temporal(dir)
    }

    fn nucleo(&self) -> Arc<Nucleo> {
        Nucleo::abrir(self.datos().display().to_string(), "Mac".into()).unwrap()
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

fn fichero(dir: &Path, nombre: &str, bytes: &[u8]) -> String {
    let r = dir.join(nombre);
    std::fs::write(&r, bytes).unwrap();
    r.display().to_string()
}

#[test]
fn apuntar_corregir_borrar_y_deshacer_un_momento() {
    let t = Temporal::nueva("apuntar");
    let n = t.nucleo();
    let foto = fichero(&t.0, "foto.PNG", b"png");
    let pegada = fichero(&t.0, "pegada.png", b"png2");
    let audio = fichero(&t.0, "voz.m4a", b"m4a");

    // Primer renglon titulo, lo demas descripcion.
    let m = n
        .timeline_apuntar(
            "Llego el yeso\nY faltaban dos sacos".into(),
            vec![
                FotoNueva { ruta: foto.clone(), temporal: false },
                FotoNueva { ruta: pegada.clone(), temporal: true },
            ],
            None,
            0,
            None,
            Some(1_000),
            0,
        )
        .unwrap();
    assert_eq!(m.titulo, "Llego el yeso");
    assert_eq!(m.descripcion, "Y faltaban dos sacos");
    assert_eq!(m.fotos.len(), 2);
    assert!(m.fotos[0].ends_with(&format!("{}-1.png", m.id)), "extension en minusculas");
    assert!(Path::new(&foto).exists(), "la elegida se copia");
    assert!(!Path::new(&pegada).exists(), "la pegada (temporal) se mueve");

    // Solo voz: «Nota de voz», con su audio movido y lo dicho guardado.
    let v = n
        .timeline_apuntar(String::new(), vec![], Some(audio.clone()), 4_200, Some("hola".into()), Some(2_000), 0)
        .unwrap();
    assert_eq!(v.titulo, "Nota de voz");
    assert_eq!(v.duracion_ms, 4_200);
    assert!(v.audio.as_deref().unwrap().ends_with(".m4a"));
    assert!(!Path::new(&audio).exists());
    let lineas = std::fs::read_to_string(t.datos().join("timeline/momentos.jsonl")).unwrap();
    assert!(lineas.contains("\"dicho\":\"hola\""), "{lineas}");

    // Caso negativo: nada que apuntar.
    assert!(matches!(
        n.timeline_apuntar("  ".into(), vec![], None, 0, None, None, 0),
        Err(ErrorNucleo::Argumento { .. })
    ));

    // Corregir: el texto nuevo se parte igual.
    let c = n
        .timeline_corregir(m.id.clone(), "Llegaron 10 sacos\nTodo bien".into(), 0)
        .unwrap();
    assert_eq!((c.titulo.as_str(), c.descripcion.as_str()), ("Llegaron 10 sacos", "Todo bien"));
    assert_eq!(c.fotos.len(), 2, "corregir no toca las fotos");
    // Sin titulo y sin medios no se puede dejar.
    let solo = n
        .timeline_apuntar("Solo texto".into(), vec![], None, 0, None, Some(3_000), 0)
        .unwrap();
    assert!(matches!(
        n.timeline_corregir(solo.id.clone(), String::new(), 0),
        Err(ErrorNucleo::Argumento { .. })
    ));
    // Con foto, sin titulo, se queda el de antes.
    assert_eq!(n.timeline_corregir(m.id.clone(), String::new(), 0).unwrap().titulo, "Llegaron 10 sacos");
    assert!(matches!(
        n.timeline_corregir("tl-no".into(), "x".into(), 0),
        Err(ErrorNucleo::NoExiste { .. })
    ));

    // Borrar y deshacer: vuelve entero, con sus fotos.
    let linea = n.timeline_borrar(m.id.clone()).unwrap();
    let todos = Almacen::en(&t.datos()).leer();
    assert_eq!(todos.len(), 2);
    assert!(Path::new(&c.fotos[0]).exists(), "las fotos se quedan para deshacer");
    n.timeline_devolver(linea.clone()).unwrap();
    n.timeline_devolver(linea).unwrap();
    let todos = Almacen::en(&t.datos()).leer();
    assert_eq!(todos.len(), 3, "devolver dos veces no lo duplica");
    assert_eq!(todos[0].id, m.id, "en su sitio por la hora");
    assert!(matches!(
        n.timeline_devolver("no es json".into()),
        Err(ErrorNucleo::Argumento { .. })
    ));
}

#[test]
fn momentos_leccion_estado_y_exportar() {
    let t = Temporal::nueva("momentos");
    let n = t.nucleo();
    let ahora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let a = n
        .timeline_apuntar("Pizza con el equipo 🍕".into(), vec![], None, 0, None, Some(ahora - 60_000), 0)
        .unwrap();
    let b = n
        .timeline_apuntar("Se fue la luz en la cocina".into(), vec![], None, 0, None, Some(ahora - 30_000), 0)
        .unwrap();

    // Mandar a «Momentos» y sacar.
    assert!(n.timeline_a_momentos(a.id.clone()).unwrap());
    let archivo = n.timeline_archivo(0, 0, 10).unwrap();
    assert_eq!(archivo.total, 1);
    assert_eq!(archivo.dias[0].momentos[0].id, a.id);
    assert!(archivo.dias[0].momentos[0].en_momentos);
    assert!(!n.timeline_a_momentos(a.id.clone()).unwrap());
    assert_eq!(n.timeline_archivo(0, 0, 10).unwrap().total, 0);

    // La bombilla: hace la leccion, la desmarca y al volver reusa la misma.
    let l = n.timeline_leccion(b.id.clone()).unwrap().expect("marcado");
    assert_eq!(n.leccion(l.clone()).unwrap().titulo, "Se fue la luz en la cocina");
    assert_eq!(n.timeline_leccion(b.id.clone()).unwrap(), None);
    assert!(n.leccion(l.clone()).is_ok(), "desmarcar no borra la leccion");
    assert_eq!(n.timeline_leccion(b.id.clone()).unwrap(), Some(l));

    // «Estado»: el mes de hoy, con el emoticono del dia.
    let estado = n.timeline_estado(0).unwrap();
    let hoy = estado[0].dias.iter().find(|d| d.cuantos > 0).unwrap();
    assert_eq!(hoy.cuantos, 2);
    assert_eq!(hoy.circulo, Some(CirculoDia::Emoticonos { lista: vec!["🍕".into()] }));
    assert_eq!(estado[0].lo_mas.as_deref(), Some("🍕"));

    // Exportar: la pagina con lo de hoy; lo buscado; nada, error.
    let p = n.timeline_exportar(ExportarTimeline::Ultimas24h, 0).unwrap();
    assert_eq!(p.cuantos, 2);
    assert!(p.html.contains("Se fue la luz"));
    assert!(p.nombre.starts_with("timeline-24h-"));
    let p = n
        .timeline_exportar(ExportarTimeline::Buscar { consulta: "cocina".into() }, 0)
        .unwrap();
    assert_eq!(p.cuantos, 1);
    let p = n
        .timeline_exportar(ExportarTimeline::Dias { isos: vec![hoy.iso.clone()] }, 0)
        .unwrap();
    assert_eq!(p.cuantos, 2);
    assert!(matches!(
        n.timeline_exportar(ExportarTimeline::Dias { isos: vec!["2001-01-01".into()] }, 0),
        Err(ErrorNucleo::Argumento { .. })
    ));
    assert!(matches!(
        n.timeline_exportar(ExportarTimeline::Dias { isos: vec!["2026-02-30".into()] }, 0),
        Err(ErrorNucleo::Argumento { .. })
    ));
}

#[test]
fn la_galeria_busca_por_nombre_fecha_y_texto_leido() {
    let t = Temporal::nueva("buscar");
    let n = t.nucleo();
    let dir = t.datos().join("capturas");
    std::fs::create_dir_all(&dir).unwrap();
    fichero(&dir, "captura-factura.png", b"a");
    fichero(&dir, "captura-2.png", b"bb");
    fichero(&dir, "grabacion.mp4", b"ccc");

    assert_eq!(n.galeria_buscar(String::new(), 0, 10, 7, 0).unwrap().total, 3);
    let r = n.galeria_buscar("FACTURA".into(), 0, 10, 7, 0).unwrap();
    assert_eq!(r.total, 1);
    assert_eq!(r.capturas[0].nombre, "captura-factura.png");
    // La fecha: «hoy».
    assert_eq!(n.galeria_buscar("hoy".into(), 0, 10, 7, 0).unwrap().total, 3);

    // El texto de dentro: los videos no se leen.
    let sin = n.galeria_sin_texto(10);
    assert_eq!(sin.len(), 2);
    assert!(sin.iter().all(|c| !c.es_video));
    n.galeria_poner_texto("captura-2.png".into(), "Total a pagar: 30 €\nAvión".into())
        .unwrap();
    assert_eq!(n.galeria_sin_texto(10).len(), 1);
    let r = n.galeria_buscar("avion pagar".into(), 0, 10, 7, 0).unwrap();
    assert_eq!(r.total, 1, "sin tildes y en cualquier orden");
    assert_eq!(r.capturas[0].nombre, "captura-2.png");
    // Se lee del disco tras soltar las caches.
    n.vaciar_caches();
    assert_eq!(n.galeria_buscar("avion".into(), 0, 10, 7, 0).unwrap().total, 1);
    // Caso negativo: nada encaja; una captura que no esta no es un error.
    assert_eq!(n.galeria_buscar("zzz".into(), 0, 10, 7, 0).unwrap().total, 0);
    n.galeria_poner_texto("no-esta.png".into(), "x".into()).unwrap();
    n.galeria_poner_texto("../fuera.png".into(), "x".into()).unwrap();
    assert!(!t.datos().join("cache/galeria").join("fuera").exists());
}
