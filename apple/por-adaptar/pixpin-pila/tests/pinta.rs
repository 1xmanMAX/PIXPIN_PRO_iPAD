//! Pinta el icono y el panel fuera de pantalla y los deja como PNG.
//!
//! Es la unica forma de MIRAR lo que se dibuja sin abrir la aplicacion: sin
//! esto, «el numero se sale del circulo» o «la miniatura sale estirada» no se
//! descubre hasta que el usuario lo ve. Necesitan GPU y por eso van con
//! `#[ignore]`, como el resto de las pruebas de dibujo del proyecto:
//!
//! ```text
//! cargo test -p pixpin-pila --test pinta -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! Escriben en el directorio temporal y dicen por `--nocapture` donde.

use std::path::PathBuf;
use std::rc::Rc;

use pixpin_codec::{FormatoImagen, ImagenRgba, guardar};
use pixpin_geom::Rect;
use pixpin_pila::pila::{Captura, Pila};
use pixpin_pila::ventana::{TextosPila, disponer, pintar_pila};
use pixpin_render::MotorRender;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};

fn d3d() -> ID3D11Device {
    let mut d = None;
    // SAFETY: salidas locales, constantes documentadas (el mismo patron que
    // las pruebas de dibujo de pixpin-pin).
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            Default::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut d),
            None,
            None,
        )
        .expect("GPU real");
    }
    d.expect("el dispositivo sale cuando la llamada va bien")
}

/// Una miniatura reconocible: bandas de color, para ver de un vistazo si sale
/// estirada, girada o del reves.
fn muestra(ancho: u32, alto: u32, tinte: [u8; 3]) -> ImagenRgba {
    let mut pixeles = Vec::with_capacity((ancho * alto * 4) as usize);
    for y in 0..alto {
        for x in 0..ancho {
            let banda = (y * 6 / alto.max(1)) % 2 == 0;
            let f = if banda { 1.0 } else { 0.45 };
            let claro = x * 255 / ancho.max(1);
            pixeles.push(((tinte[0] as f32 * f) as u32).min(255) as u8);
            pixeles.push(((tinte[1] as f32 * f) as u32).min(255) as u8);
            pixeles.push((((tinte[2] as u32 + claro) / 2) as f32 * f) as u8);
            pixeles.push(255);
        }
    }
    ImagenRgba {
        ancho,
        alto,
        pixeles,
    }
}

fn captura(n: usize, ancho: u32, alto: u32, elegida: bool) -> Captura {
    let tintes = [
        [220u8, 80, 70],
        [70, 180, 220],
        [120, 210, 120],
        [230, 190, 80],
        [180, 120, 220],
    ];
    Captura {
        ruta: PathBuf::from(format!(r"C:\datos\capturas\captura-{n:04}.png")),
        miniatura: muestra(ancho, alto, tintes[n % tintes.len()]),
        ancho: ancho * 4,
        alto: alto * 4,
        elegida,
    }
}

fn monitor() -> Rect {
    Rect {
        x: 0,
        y: 0,
        ancho: 1920,
        alto: 1080,
    }
}

fn textos() -> TextosPila {
    TextosPila {
        titulo: "5 capturas apiladas".into(),
        copiar_elegidas: "Copiar elegidas".into(),
        copiar_todas: "Copiar todas".into(),
        quitar: "Quitar".into(),
    }
}

/// Pinta `pila` con la disposicion pedida y guarda el PNG. Devuelve la ruta.
fn retrato(nombre: &str, pila: &Pila, abierto: bool, escala: u32) -> PathBuf {
    let d3d = d3d();
    let motor = Rc::new(MotorRender::nuevo(&d3d).expect("motor"));
    let d = disponer(abierto, pila.cuantas(), escala);
    let lienzo = FueraDePantalla::nuevo(&motor, &d3d, d.ancho, d.alto).expect("destino");

    let bitmaps: Vec<_> = pila
        .capturas()
        .iter()
        .map(|c| {
            motor
                .bitmap_desde_pixeles(c.miniatura.ancho, c.miniatura.alto, &c.miniatura.pixeles)
                .expect("miniatura")
        })
        .collect();

    motor
        .dibujar(&lienzo.destino, |p| {
            // Un tablero gris debajo: sin el, lo transparente sale negro en
            // el PNG y no se distingue del fondo oscuro de la tarjeta.
            p.limpiar(pixpin_render::Color {
                r: 0.45,
                g: 0.47,
                b: 0.50,
                a: 1.0,
            });
            pintar_pila(p, pila, &bitmaps, &d, &textos());
        })
        .expect("dibujar");
    lienzo.esperar_gpu().expect("esperar");

    let (ancho, alto, pixeles) = lienzo.leer_rgba().expect("leer");
    let ruta = std::env::temp_dir().join(format!("pixpin-pila-{nombre}.png"));
    guardar(
        &ImagenRgba {
            ancho,
            alto,
            pixeles,
        },
        &ruta,
        FormatoImagen::Png,
    )
    .expect("guardar");
    println!("PNG: {}", ruta.display());
    ruta
}

fn con(cuantas: usize, desmarcadas: &[usize]) -> Pila {
    let mut p = Pila::nueva(10);
    for i in 0..cuantas {
        // Tamanos distintos a proposito: apaisadas, verticales y cuadradas,
        // para ver si alguna sale deformada dentro de su celda.
        let (w, h) = match i % 3 {
            0 => (160u32, 90u32),
            1 => (90, 160),
            _ => (120, 120),
        };
        p.anadir(captura(i, w, h, !desmarcadas.contains(&i)), monitor(), 100);
        // Armada tras la primera: es el clic que agrupa las siguientes.
        if i == 0 {
            p.armar(true);
        }
    }
    p
}

#[test]
#[ignore = "necesita GPU; ejecutar con --ignored"]
fn el_icono_con_una_sola_captura() {
    let ruta = retrato("icono-1", &con(1, &[]), false, 150);
    assert!(ruta.exists());
}

#[test]
#[ignore = "necesita GPU; ejecutar con --ignored"]
fn el_icono_apilado_con_cinco() {
    let ruta = retrato("icono-5", &con(5, &[]), false, 150);
    assert!(ruta.exists());
}

#[test]
#[ignore = "necesita GPU; ejecutar con --ignored"]
fn el_panel_con_cinco_y_dos_desmarcadas() {
    let ruta = retrato("panel-5", &con(5, &[1, 3]), true, 150);
    assert!(ruta.exists());
}

#[test]
#[ignore = "necesita GPU; ejecutar con --ignored"]
fn el_panel_lleno() {
    let ruta = retrato("panel-lleno", &con(11, &[2]), true, 100);
    assert!(ruta.exists());
}

#[test]
#[ignore = "necesita GPU; ejecutar con --ignored"]
fn el_icono_sin_armar_no_lleva_halo() {
    let mut p = con(1, &[]);
    p.armar(false);
    let ruta = retrato("icono-1-sin-armar", &p, false, 150);
    assert!(ruta.exists());
}
