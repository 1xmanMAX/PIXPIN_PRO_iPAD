//! Los marcadores de un lienzo de un chat con su vista, ida y vuelta con
//! Windows: lo que escribe el PC (`ventana_editor/marcas.rs`) se lee aqui
//! igual y lo que se escribe aqui es, letra por letra, lo que escribiria el
//! PC (§3.8 de `docs/sincronizar-marcos-vistas-y-tinta.md`).

use std::path::{Path, PathBuf};

use pixpin_nucleo::{CamaraDelLienzo, MarcasDelLienzo, Nucleo};

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-marcas-lienzo-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temporal(dir)
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Un chat con un lienzo: la ruta de su dibujo.
fn lienzo_de_un_chat(t: &Temporal) -> PathBuf {
    let n = Nucleo::abrir(t.0.join("datos").display().to_string(), "Mac".into()).unwrap();
    let chat = n.crear_chat("Obra".into()).unwrap();
    let m = n
        .crear_lienzo(chat.id.clone(), Some("Plano".into()))
        .unwrap();
    let ed = n
        .editar_lienzo(chat.id.clone(), m.referencia.clone().unwrap())
        .unwrap();
    PathBuf::from(ed.ruta())
}

fn leer(r: &Path) -> String {
    std::fs::read_to_string(r).unwrap()
}

#[test]
fn las_vistas_de_windows_se_leen_se_respetan_y_vuelven_como_las_escribe_windows() {
    let t = Temporal::nueva("windows");
    let dibujo = lienzo_de_un_chat(&t);
    let m = MarcasDelLienzo::junto_al_dibujo(dibujo.display().to_string());
    let (ruta_marcas, ruta_vistas) = m.rutas();
    // Junto al dibujo del movil, en el chat: viajan con el.
    let d = dibujo.file_stem().unwrap().to_string_lossy().to_string();
    assert!(ruta_marcas.ends_with(format!("android/pins/draw/{d}.marcas")));
    assert!(ruta_vistas.ends_with(format!("android/pins/draw/{d}.vistas.json")));

    // Lo que deja el PC: una marca con su vista, escrita con
    // `serde_json::to_string_pretty` como hace `vistas_a_texto` de Windows.
    std::fs::create_dir_all(ruta_marcas.parent().unwrap()).unwrap();
    std::fs::write(&ruta_marcas, "1792000000000:320.5:1040:⭐").unwrap();
    std::fs::write(
        &ruta_vistas,
        "{\n  \"1792000000000\": \"320.5 1040 800 450\"\n}",
    )
    .unwrap();
    assert!(m.releer());
    assert!(!m.releer());
    let lista = m.lista();
    assert_eq!(lista.len(), 1);
    assert!(lista[0].con_vista);

    // Ir a ella en un hueco de 1600 x 900 da la camara de la formula:
    // zoom = min(1600/800, 900/450) = 2, esquina en (320,5 - 400, 1040 - 225).
    let aqui = CamaraDelLienzo {
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
    };
    assert_eq!(
        m.destino(1792000000000, aqui, 1600.0, 900.0, 60.0),
        Some(CamaraDelLienzo {
            x: -79.5,
            y: 815.0,
            zoom: 2.0
        })
    );

    // El PC la cambia con Mayus+clic mientras el lienzo esta abierto aqui:
    // ir a ella usa la nueva sin cerrar nada.
    std::fs::write(
        &ruta_vistas,
        "{\n  \"1792000000000\": \"0 0 1600 900\"\n}",
    )
    .unwrap();
    assert!(m.ir_a(1792000000000, aqui, 1600.0, 900.0, 60.0));
    let mut c = aqui;
    while let Some(n) = m.avanzar(c, 1.0 / 60.0) {
        c = n;
        if !m.volando() {
            break;
        }
    }
    assert_eq!(
        c,
        CamaraDelLienzo {
            x: -800.0,
            y: -450.0,
            zoom: 1.0
        }
    );

    // Aqui se planta otra, mas abajo: la linea de `.marcas` y el
    // `.vistas.json` salen como los escribiria Windows.
    let camara = CamaraDelLienzo {
        x: 10.0,
        y: 20.0,
        zoom: 2.0,
    };
    let nueva = m.poner(50.25, 2000.0, "🔥".into(), camara, 1000.0, 500.0);
    assert_eq!(
        leer(&ruta_marcas),
        format!("1792000000000:320.5:1040:⭐|{nueva}:50.25:2000:🔥")
    );
    // ancho = 1000/2 = 500, alto = 250, centro (10 + 250, 20 + 125).
    assert_eq!(
        leer(&ruta_vistas),
        format!(
            "{{\n  \"1792000000000\": \"0 0 1600 900\",\n  \"{nueva}\": \"260 145 500 250\"\n}}"
        )
    );

    // Otra ventana (o Windows al leerlo) ve lo mismo.
    let otra = MarcasDelLienzo::junto_al_dibujo(dibujo.display().to_string());
    assert_eq!(otra.lista(), m.lista());

    // Quitarlas todas deja los ficheros vacios, no los borra: la
    // sincronizacion no lleva borrados.
    assert!(m.quitar(nueva));
    assert!(m.quitar(1792000000000));
    assert_eq!(leer(&ruta_marcas), "");
    assert_eq!(leer(&ruta_vistas), "{}");
}

#[test]
fn las_marcas_de_antes_junto_al_dibujo_pasan_al_chat_la_primera_vez() {
    let t = Temporal::nueva("antes");
    let dibujo = lienzo_de_un_chat(&t);
    let mut viejo = dibujo.clone().into_os_string();
    viejo.push(".pixpin-marcas");
    std::fs::write(&viejo, "5:1:2:❓").unwrap();
    let m = MarcasDelLienzo::junto_al_dibujo(dibujo.display().to_string());
    assert_eq!(m.lista().len(), 1);
    assert_eq!(leer(&m.rutas().0), "5:1:2:❓");
    // Sin vistas no se escribe ningun `.vistas.json`.
    assert!(!m.rutas().1.exists());
}
