//! El compresor de PDF en su proceso: `pixpin-aligerar.exe`.
//!
//! La aplicacion lo lanza asi para que un PDF que haga entrar en panico a
//! pdfsqueeze no se lleve PixPin por delante (el release es
//! `panic = "abort"`), y para cancelar matando. Aqui se prueba el protocolo
//! con el ejecutable de verdad: avance por la salida estandar, codigo de
//! salida, y que se va solo cuando se cierra su entrada estandar.

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use pdfsqueeze_core::testgen::{self, ImgEnc, Synth};

const EXE: &str = env!("CARGO_BIN_EXE_pixpin-aligerar");

fn carpeta(etiqueta: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "pixpin-aligerar-proceso-{etiqueta}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Fotos de 1600x1200 en JPEG al 92 % dibujadas a unos 800 ppp.
fn pesado(paginas: usize) -> Vec<u8> {
    let mut s = Synth::new();
    for i in 0..paginas {
        let img = testgen::photo_seeded(1600, 1200, i as u64 + 3);
        s.image_page(&img, ImgEnc::Jpeg(92), 50.0, 400.0, 144.0, 108.0);
    }
    s.finish()
}

fn lanzar(entrada: &Path, salida: &Path, nivel: &str) -> Child {
    Command::new(EXE)
        .arg(entrada)
        .arg(salida)
        .args([nivel, "2", "baja"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("se lanza el ejecutable")
}

#[test]
fn el_proceso_hijo_aligera_y_va_diciendo_su_avance_y_su_memoria() {
    let d = carpeta("bien");
    let (entrada, salida) = (d.join("a.pdf"), d.join("a.ligero.pdf"));
    let original = pesado(2);
    std::fs::write(&entrada, &original).unwrap();
    let mut hijo = lanzar(&entrada, &salida, "equilibrado");
    let lineas: Vec<String> = std::io::BufReader::new(hijo.stdout.take().unwrap())
        .lines()
        .map_while(Result::ok)
        .collect();
    let estado = hijo.wait().unwrap();
    assert_eq!(estado.code(), Some(0), "salida: {lineas:?}");
    let avance: Vec<u8> = lineas
        .iter()
        .filter_map(|l| l.strip_prefix("p ")?.parse().ok())
        .collect();
    assert!(
        avance.len() >= 3,
        "tiene que ir diciendo por donde va: {lineas:?}"
    );
    assert!(
        lineas.iter().any(|l| l.starts_with("memoria ")),
        "{lineas:?}"
    );
    let ligero = std::fs::read(&salida).unwrap();
    assert!(ligero.len() < original.len() * 85 / 100);
    assert_eq!(
        std::fs::read(&entrada).unwrap(),
        original,
        "la entrada no se toca"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn matar_el_proceso_hijo_a_mitad_no_toca_la_entrada() {
    let d = carpeta("matar");
    let (entrada, salida) = (d.join("a.pdf"), d.join("a.ligero.pdf"));
    let original = pesado(10);
    std::fs::write(&entrada, &original).unwrap();
    let mut hijo = lanzar(&entrada, &salida, "equilibrado");
    let mut fuera = std::io::BufReader::new(hijo.stdout.take().unwrap());
    let mut linea = String::new();
    // Hasta que este en las fotos, que es lo largo.
    while fuera.read_line(&mut linea).unwrap_or(0) > 0 {
        if linea
            .trim()
            .strip_prefix("p ")
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| n >= 15)
        {
            break;
        }
        linea.clear();
    }
    hijo.kill().unwrap();
    let estado = hijo.wait().unwrap();
    assert_ne!(estado.code(), Some(0));
    assert_eq!(
        std::fs::read(&entrada).unwrap(),
        original,
        "el original, entero"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn sin_padre_el_proceso_hijo_se_va_solo() {
    // Cerrar su entrada estandar es lo que pasa cuando PixPin se cierra o
    // muere: el hijo no se queda comprimiendo para nadie.
    let d = carpeta("huerfano");
    let (entrada, salida) = (d.join("a.pdf"), d.join("a.ligero.pdf"));
    std::fs::write(&entrada, pesado(10)).unwrap();
    let t = std::time::Instant::now();
    let mut hijo = lanzar(&entrada, &salida, "equilibrado");
    drop(hijo.stdin.take());
    let estado = hijo.wait().unwrap();
    assert_eq!(
        estado.code(),
        Some(14),
        "se va con el codigo de «sin padre»"
    );
    assert!(t.elapsed().as_secs() < 5, "tardo {:?} en irse", t.elapsed());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn un_pdf_cifrado_sale_con_su_codigo_y_sin_escribir_nada() {
    let d = carpeta("cifrado");
    let (entrada, salida) = (d.join("a.pdf"), d.join("a.ligero.pdf"));
    let mut s = Synth::new();
    s.image_page(
        &testgen::photo(800, 600),
        ImgEnc::Jpeg(92),
        50.0,
        400.0,
        144.0,
        108.0,
    );
    let mut cifra = lopdf::Dictionary::new();
    cifra.set("Filter", "Standard");
    cifra.set("V", 1);
    cifra.set("R", 2);
    cifra.set("P", -4);
    cifra.set("O", lopdf::Object::string_literal(vec![0u8; 32]));
    cifra.set("U", lopdf::Object::string_literal(vec![0u8; 32]));
    let clave = s.doc.add_object(cifra);
    s.doc.trailer.set("Encrypt", clave);
    std::fs::write(&entrada, s.finish()).unwrap();
    let mut hijo = lanzar(&entrada, &salida, "equilibrado");
    let _ = std::io::BufReader::new(hijo.stdout.take().unwrap())
        .lines()
        .count();
    assert_eq!(hijo.wait().unwrap().code(), Some(11));
    assert!(!salida.exists());
    // Caso negativo: argumentos que no son los suyos no comprimen nada.
    let mut mal = Command::new(EXE)
        .args(["solo-uno"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    assert_eq!(mal.wait().unwrap().code(), Some(12));
    let _ = std::fs::remove_dir_all(&d);
}
