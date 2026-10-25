//! **Los iconos de redes y TI** (24-oct-2026): los genericos de los
//! diagramas de red y de arquitectura de sistemas (servidor, base de
//! datos, nube, router, switch, cortafuegos, equipos, usuarios, API...),
//! sin marcas de fabricante.
//!
//! Todos caben en un cuadrado de unos 100 × 100 con su nombre debajo, en
//! dos partes: «Icono» y «Nombre», para que un diagrama de red salga
//! parejo y se pueda renombrar cada equipo sin tocar el dibujo.

use super::Lapiz;
use super::diagramas::{BLANCO, G, RELLENO, RELLENO_FUERTE, TRAZO, forma, punto, raya, rotulo};
use super::electronica::{arco_pts, region};
use super::tecnico::{Dibujo, poner};
use crate::biblioteca::FiguraGuardada;

pub(super) const CATEGORIA: &str = "Redes y TI";

/// El ancho de cada icono y donde va el nombre.
const W: f32 = 100.0;

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: &[Dibujo] = &[
        ("red-servidor", "Servidor", |l| {
            con_nombre(l, servidor, "Servidor")
        }),
        ("red-base-de-datos", "Servidor de base de datos", |l| {
            con_nombre(l, base_de_datos, "Base de datos")
        }),
        ("red-nube", "Nube (Internet)", |l| {
            con_nombre(l, nube, "Internet")
        }),
        ("red-router", "Router", |l| con_nombre(l, router, "Router")),
        ("red-switch", "Switch", |l| con_nombre(l, switch, "Switch")),
        ("red-cortafuegos", "Cortafuegos (firewall)", |l| {
            con_nombre(l, cortafuegos, "Cortafuegos")
        }),
        ("red-balanceador", "Balanceador de carga", |l| {
            con_nombre(l, balanceador, "Balanceador")
        }),
        ("red-punto-de-acceso", "Punto de acceso wifi", |l| {
            con_nombre(l, wifi, "Wifi")
        }),
        ("red-ordenador", "Ordenador de sobremesa", |l| {
            con_nombre(l, ordenador, "PC")
        }),
        ("red-portatil", "Portátil", |l| {
            con_nombre(l, portatil, "Portátil")
        }),
        ("red-movil", "Teléfono móvil", |l| {
            con_nombre(l, movil, "Móvil")
        }),
        ("red-tableta", "Tableta", |l| {
            con_nombre(l, tableta, "Tableta")
        }),
        ("red-impresora", "Impresora", |l| {
            con_nombre(l, impresora, "Impresora")
        }),
        ("red-usuario", "Usuario", |l| {
            con_nombre(l, usuario, "Usuario")
        }),
        ("red-usuarios", "Grupo de usuarios", |l| {
            con_nombre(l, usuarios, "Usuarios")
        }),
        ("red-api", "API o servicio", |l| {
            con_nombre(l, api, "Servicio")
        }),
    ];
    poner(v, CATEGORIA, lista);
}

/// El icono en su parte «Icono» y el nombre debajo, en la suya.
fn con_nombre(l: &mut Lapiz, icono: fn(&mut Lapiz), nombre: &str) {
    // Parte «Icono»: el dibujo del equipo, en un cuadrado de 100.
    l.parte("Icono");
    icono(l);
    // Parte «Nombre»: como se llama, debajo.
    l.parte("Nombre");
    rotulo(l, W / 2.0, 116.0, nombre, 14.0, false);
}

fn servidor(l: &mut Lapiz) {
    // El armario con tres bandejas, cada una con su luz y sus rejillas.
    l.rect(20.0, 0.0, 60.0, 100.0, Some(TRAZO), Some(RELLENO), G, true);
    for i in 0..3 {
        let y = 10.0 + 28.0 * i as f32;
        l.rect(28.0, y, 44.0, 22.0, Some(TRAZO), Some(BLANCO), 1.5, false);
        punto(l, 36.0, y + 11.0, 3.0, hex_verde());
        for k in 0..3 {
            let x = 48.0 + 7.0 * k as f32;
            l.linea(x, y + 6.0, x, y + 16.0, TRAZO, 1.5);
        }
    }
}

fn hex_verde() -> crate::elemento::ColorRgba {
    super::hex(0x2e8b57)
}

fn base_de_datos(l: &mut Lapiz) {
    // El cilindro con dos franjas, que es un disco de datos.
    let (cx, rx, ry) = (50.0, 36.0, 12.0);
    let mut p = vec![(cx - rx, 12.0)];
    p.extend(arco_pts(cx, 88.0, rx, ry, 180.0, 0.0));
    p.extend(arco_pts(cx, 12.0, rx, ry, 0.0, -180.0));
    p.pop();
    forma(l, &p);
    l.poli(&arco_pts(cx, 12.0, rx, ry, 0.0, 180.0), TRAZO, G, None);
    for y in [37.0, 62.0] {
        l.poli(&arco_pts(cx, y, rx, ry, 0.0, 180.0), TRAZO, 1.5, None);
    }
}

/// Los puntos de una nube de lobulos en la caja dada.
pub(super) fn nube_pts(x: f32, y: f32, w: f32, h: f32) -> Vec<(f32, f32)> {
    // La nube es la union de cinco circulos con la base plana: se mira
    // desde el centro en cada direccion hasta el borde mas lejano.
    let circulos = [
        (22.0, 46.0, 15.0),
        (38.0, 28.0, 17.0),
        (62.0, 24.0, 21.0),
        (82.0, 42.0, 16.0),
        (50.0, 46.0, 18.0),
    ];
    let (ox, oy, base) = (50.0_f32, 40.0_f32, 61.0_f32);
    (0..90)
        .map(|i| {
            let a = (i as f32 * 4.0).to_radians();
            let (dy, dx) = a.sin_cos();
            let mut t = 0.0_f32;
            for (cx, cy, r) in circulos {
                let (fx, fy) = (ox - cx, oy - cy);
                let b = fx * dx + fy * dy;
                let disc = b * b - (fx * fx + fy * fy - r * r);
                if disc >= 0.0 {
                    t = t.max(-b + disc.sqrt());
                }
            }
            if dy > 0.0 {
                t = t.min((base - oy) / dy);
            }
            (x + (ox + t * dx) * w / 100.0, y + (oy + t * dy) * h / 70.0)
        })
        .collect()
}

fn nube(l: &mut Lapiz) {
    region(l, &nube_pts(0.0, 10.0, W, 76.0), TRAZO, RELLENO, G);
}

/// Una flecha corta de `a` a `b` con su punta, para los iconos.
fn flechita(l: &mut Lapiz, a: (f32, f32), b: (f32, f32), c: crate::elemento::ColorRgba) {
    let ang = (b.1 - a.1).atan2(b.0 - a.0);
    let (s, co) = ang.sin_cos();
    l.linea(a.0, a.1, b.0 - co * 4.0, b.1 - s * 4.0, c, 2.5);
    let base = (b.0 - co * 9.0, b.1 - s * 9.0);
    region(
        l,
        &[
            b,
            (base.0 - s * 5.0, base.1 + co * 5.0),
            (base.0 + s * 5.0, base.1 - co * 5.0),
        ],
        c,
        c,
        1.0,
    );
}

fn router(l: &mut Lapiz) {
    // El disco visto de lado con las cuatro flechas en cruz encima.
    l.elipse(5.0, 30.0, 90.0, 56.0, Some(TRAZO), Some(RELLENO), G);
    region(
        l,
        &{
            let mut p = vec![(5.0, 46.0)];
            p.extend(arco_pts(50.0, 58.0, 45.0, 28.0, 180.0, 0.0));
            p.push((95.0, 46.0));
            p.extend(arco_pts(50.0, 46.0, 45.0, 28.0, 0.0, 180.0));
            p
        },
        TRAZO,
        RELLENO_FUERTE,
        G,
    );
    l.elipse(5.0, 18.0, 90.0, 56.0, Some(TRAZO), Some(RELLENO), G);
    let (cx, cy) = (50.0, 46.0);
    flechita(l, (cx - 10.0, cy - 6.0), (cx - 32.0, cy - 18.0), TRAZO);
    flechita(l, (cx + 10.0, cy + 6.0), (cx + 32.0, cy + 18.0), TRAZO);
    flechita(l, (cx + 32.0, cy - 18.0), (cx + 10.0, cy - 6.0), TRAZO);
    flechita(l, (cx - 32.0, cy + 18.0), (cx - 10.0, cy + 6.0), TRAZO);
}

fn switch(l: &mut Lapiz) {
    // La caja plana con las flechas que van y vienen.
    l.rect(0.0, 26.0, W, 54.0, Some(TRAZO), Some(RELLENO), G, true);
    flechita(l, (14.0, 37.0), (58.0, 37.0), TRAZO);
    flechita(l, (42.0, 49.0), (86.0, 49.0), TRAZO);
    flechita(l, (58.0, 59.0), (14.0, 59.0), TRAZO);
    flechita(l, (86.0, 70.0), (42.0, 70.0), TRAZO);
}

fn cortafuegos(l: &mut Lapiz) {
    // El muro de ladrillos, de cuatro hileras.
    let ladrillo = super::hex(0xfbe3d6);
    let borde = super::hex(0xb5523b);
    for fila in 0..4 {
        let y = 12.0 + 20.0 * fila as f32;
        let desfase = if fila % 2 == 0 { 0.0 } else { 15.0 };
        let mut x = -desfase;
        while x < W {
            let x0 = x.max(0.0);
            let x1 = (x + 30.0).min(W);
            l.rect(
                x0,
                y,
                x1 - x0,
                20.0,
                Some(borde),
                Some(ladrillo),
                1.5,
                false,
            );
            x += 30.0;
        }
    }
}

fn balanceador(l: &mut Lapiz) {
    // El circulo con una entrada que se reparte en tres.
    l.circulo(50.0, 50.0, 44.0, Some(TRAZO), Some(RELLENO), G);
    l.linea(14.0, 50.0, 44.0, 50.0, TRAZO, 2.5);
    for y in [24.0, 50.0, 76.0] {
        flechita(l, (44.0, 50.0), (82.0, y), TRAZO);
    }
}

fn wifi(l: &mut Lapiz) {
    // La caja con su antena y las ondas.
    l.rect(15.0, 64.0, 70.0, 26.0, Some(TRAZO), Some(RELLENO), G, true);
    punto(l, 30.0, 77.0, 3.0, hex_verde());
    raya(l, (50.0, 64.0), (50.0, 40.0));
    for r in [12.0, 22.0, 32.0] {
        l.poli(&arco_pts(50.0, 40.0, r, r, -140.0, -40.0), TRAZO, G, None);
    }
}

fn ordenador(l: &mut Lapiz) {
    // El monitor, el pie y la base.
    l.rect(4.0, 4.0, 92.0, 62.0, Some(TRAZO), Some(RELLENO), G, true);
    l.rect(
        12.0,
        12.0,
        76.0,
        46.0,
        Some(TRAZO),
        Some(BLANCO),
        1.5,
        false,
    );
    forma(l, &[(42.0, 66.0), (58.0, 66.0), (62.0, 86.0), (38.0, 86.0)]);
    l.rect(26.0, 86.0, 48.0, 8.0, Some(TRAZO), Some(RELLENO), G, true);
}

fn portatil(l: &mut Lapiz) {
    // La pantalla abierta y el teclado en perspectiva.
    l.rect(14.0, 10.0, 72.0, 52.0, Some(TRAZO), Some(RELLENO), G, true);
    l.rect(
        21.0,
        17.0,
        58.0,
        38.0,
        Some(TRAZO),
        Some(BLANCO),
        1.5,
        false,
    );
    forma(l, &[(10.0, 66.0), (90.0, 66.0), (100.0, 84.0), (0.0, 84.0)]);
    l.linea(40.0, 78.0, 60.0, 78.0, TRAZO, 1.5);
}

fn movil(l: &mut Lapiz) {
    // El telefono de pie con su pantalla y el boton.
    l.rect(28.0, 0.0, 44.0, 96.0, Some(TRAZO), Some(RELLENO), G, true);
    l.rect(
        32.0,
        10.0,
        36.0,
        68.0,
        Some(TRAZO),
        Some(BLANCO),
        1.5,
        false,
    );
    l.circulo(50.0, 87.0, 4.0, Some(TRAZO), None, 1.5);
}

fn tableta(l: &mut Lapiz) {
    l.rect(14.0, 0.0, 72.0, 96.0, Some(TRAZO), Some(RELLENO), G, true);
    l.rect(20.0, 8.0, 60.0, 74.0, Some(TRAZO), Some(BLANCO), 1.5, false);
    l.circulo(50.0, 89.0, 3.5, Some(TRAZO), None, 1.5);
}

fn impresora(l: &mut Lapiz) {
    // El papel que entra, el cuerpo y la hoja que sale.
    l.rect(26.0, 4.0, 48.0, 30.0, Some(TRAZO), Some(BLANCO), 1.5, false);
    l.rect(4.0, 30.0, 92.0, 44.0, Some(TRAZO), Some(RELLENO), G, true);
    punto(l, 82.0, 42.0, 3.0, hex_verde());
    l.rect(
        22.0,
        62.0,
        56.0,
        32.0,
        Some(TRAZO),
        Some(BLANCO),
        1.5,
        false,
    );
    for y in [72.0, 80.0] {
        l.linea(30.0, y, 70.0, y, TRAZO, 1.0);
    }
}

/// Un busto (cabeza y hombros) centrado en `cx` con la cabeza en `cy`.
pub(super) fn busto(l: &mut Lapiz, cx: f32, cy: f32, r: f32) {
    let mut hombros = arco_pts(cx, cy + 3.2 * r, 2.0 * r, 1.6 * r, 180.0, 360.0);
    hombros.push((cx + 2.0 * r, cy + 3.4 * r));
    hombros.push((cx - 2.0 * r, cy + 3.4 * r));
    region(l, &hombros, TRAZO, RELLENO, G);
    l.circulo(cx, cy, r, Some(TRAZO), Some(RELLENO), G);
}

fn usuario(l: &mut Lapiz) {
    busto(l, 50.0, 24.0, 20.0);
}

fn usuarios(l: &mut Lapiz) {
    busto(l, 26.0, 34.0, 14.0);
    busto(l, 74.0, 34.0, 14.0);
    busto(l, 50.0, 26.0, 17.0);
}

fn api(l: &mut Lapiz) {
    // El hexagono con «API» dentro.
    let p: Vec<(f32, f32)> = (0..6)
        .map(|i| {
            let a = (60.0 * i as f32).to_radians();
            (50.0 + 50.0 * a.cos(), 48.0 + 44.0 * a.sin())
        })
        .collect();
    forma(l, &p);
    rotulo(l, 50.0, 48.0, "API", 20.0, true);
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::figuras_de_serie::diagramas::pruebas::{comprobar, nombres, partes_estampada};

    #[test]
    fn estan_las_de_redes_y_cada_pieza_tiene_su_parte() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(v.len(), 16);
        comprobar(&v, "serie-red-");
    }

    #[test]
    fn el_servidor_estampado_conserva_sus_partes() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(
            partes_estampada(&v, "serie-red-servidor"),
            nombres(&["Icono", "Nombre"])
        );
    }
}
