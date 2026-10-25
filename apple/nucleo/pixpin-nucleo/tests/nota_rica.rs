//! La nota rica del chat (Windows 974d5ba): una foto pegada con texto
//! escrito va a `notas/` y a lo escrito; al enviar sale con su ruta
//! portatil (la que viaja), y la burbuja la encuentra con la misma
//! resolucion que los adjuntos y sabe su proporcion.

use std::path::PathBuf;
use std::sync::Arc;

use pixpin_nucleo::Nucleo;
use pixpin_nucleo::nota_rica::{BloqueDeNota, ClaseDeBloque};
use pixpin_proyecto::almacen;

struct Temporal(PathBuf);

impl Temporal {
    fn nueva(etiqueta: &str) -> Temporal {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-nota-rica-{etiqueta}-{}",
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

/// Un PNG de `ancho` x `alto`.
fn png(ancho: u32, alto: u32) -> Vec<u8> {
    pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
        ancho,
        alto,
        pixeles: vec![200; (ancho * alto * 4) as usize],
    })
    .unwrap()
}

#[test]
fn una_foto_pegada_viaja_con_la_nota_y_se_ve_en_su_sitio() {
    let t = Temporal::nueva("pegada");
    let n = t.nucleo("Mac");
    let chat = n.crear_chat("Obra".into()).unwrap().id;

    // Pegar con texto escrito: el fichero en `notas/` y su renglon corto.
    let renglon = n
        .foto_a_la_nota(chat.clone(), "pegado-1761400000000.png".into(), png(40, 20), "![](notas/a.png)".into(), 0)
        .unwrap();
    // La segunda foto de lo escrito (ya llevaba una), con su numero delante.
    assert!(renglon.starts_with("![foto 2](notas/") && renglon.ends_with("-pegado.png)"), "{renglon}");
    let fichero = renglon.trim_start_matches("![foto 2](notas/").trim_end_matches(')');
    let carpeta = almacen::carpeta(&PathBuf::from(n.raiz()), &chat);
    assert!(carpeta.join("notas").join(fichero).is_file());

    // Al enviar, la ruta portatil: la que la sincronizacion lleva.
    let m = n
        .escribir_nota(chat.clone(), format!("**Pendiente:** la escalera\n{renglon}"), None)
        .unwrap();
    assert!(
        m.texto.contains(&format!("](pixpin:files/guardados/pc/{chat}/notas/{fichero})")),
        "{}",
        m.texto
    );
    assert!(!m.texto.contains("](notas/"));

    // La burbuja: el texto con su negrita y la foto con su fichero y su
    // proporcion (40 x 20).
    let rica = n.nota_rica(chat.clone(), m.texto.clone()).unwrap().unwrap();
    assert!(matches!(
        &rica.bloques[0],
        BloqueDeNota::Texto { clase: ClaseDeBloque::Parrafo, marcas, .. } if marcas[0].negrita
    ));
    let BloqueDeNota::Album { fotos } = &rica.bloques[1] else {
        panic!("{:?}", rica.bloques)
    };
    let ruta = fotos[0].ruta.clone().expect("su fichero esta en este Mac");
    assert_eq!(PathBuf::from(&ruta), carpeta.join("notas").join(fichero));
    assert_eq!(
        n.ruta_absoluta(chat.clone(), fotos[0].enlace.clone()).unwrap(),
        ruta,
        "la misma resolucion que los adjuntos"
    );
    assert!((fotos[0].proporcion - 2.0).abs() < 0.001);
}

#[test]
fn una_foto_que_no_esta_no_tiene_ruta_y_una_nota_llana_no_es_rica() {
    let t = Temporal::nueva("falta");
    let n = t.nucleo("Mac");
    let chat = n.crear_chat("Obra".into()).unwrap().id;
    let rica = n
        .nota_rica(
            chat.clone(),
            "![](pixpin:files/guardados/pc/otro/notas/1-a.png)".into(),
        )
        .unwrap()
        .unwrap();
    let BloqueDeNota::Album { fotos } = &rica.bloques[0] else {
        panic!("{:?}", rica.bloques)
    };
    assert_eq!(fotos[0].ruta, None, "nunca una ruta que no lleva a nada");
    assert_eq!(fotos[0].proporcion, 0.0);
    assert_eq!(n.nota_rica(chat.clone(), "hola\nque tal".into()).unwrap(), None);
    // Una nota sin fotos pegadas no cambia al enviarse.
    let m = n.escribir_nota(chat, "mira [esto](https://x.es)".into(), None).unwrap();
    assert_eq!(m.texto, "mira [esto](https://x.es)");
}

#[test]
fn una_ruta_con_parentesis_se_ve_y_una_de_red_o_con_puntos_nunca() {
    // Windows 6f2d95a: la ruta va hasta el `)` que empareja el `(`.
    let t = Temporal::nueva("rutas");
    let n = t.nucleo("Mac");
    let chat = n.crear_chat("Obra".into()).unwrap().id;
    let carpeta = almacen::carpeta(&PathBuf::from(n.raiz()), &chat).join("notas").join("Fotos (2)");
    std::fs::create_dir_all(&carpeta).unwrap();
    let foto = carpeta.join("obra 1.png");
    std::fs::write(&foto, png(30, 30)).unwrap();
    let rica = n
        .nota_rica(chat.clone(), "**mira**\n![](notas/Fotos (2)/obra 1.png) y sigue".into())
        .unwrap()
        .unwrap();
    let BloqueDeNota::Album { fotos } = &rica.bloques[1] else {
        panic!("{:?}", rica.bloques)
    };
    assert_eq!(fotos[0].ruta.as_deref(), Some(foto.display().to_string().as_str()));
    assert!((fotos[0].proporcion - 1.0).abs() < 0.001);
    assert!(matches!(&rica.bloques[2], BloqueDeNota::Texto { texto, .. } if texto == "y sigue"));

    // Caso negativo: una ruta de red o una que sale del proyecto con `..`
    // no se abre nunca, aunque el fichero exista.
    for enlace in [
        r"\\equipo\carpeta\x.png".to_string(),
        "//equipo/notas/Fotos (2)/obra 1.png".to_string(),
        "notas/../notas/Fotos (2)/obra 1.png".to_string(),
    ] {
        let rica = n.nota_rica(chat.clone(), format!("![]({enlace})")).unwrap().unwrap();
        let BloqueDeNota::Album { fotos } = &rica.bloques[0] else {
            panic!("{:?}", rica.bloques)
        };
        assert_eq!(fotos[0].ruta, None, "{enlace}");
    }
}
