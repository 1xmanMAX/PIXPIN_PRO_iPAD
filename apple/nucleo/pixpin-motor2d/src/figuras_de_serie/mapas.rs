//! **Las piezas de los mapas mentales y los organigramas** (24-oct-2026):
//! el tema central, las ideas y subtemas, las ramas curvas, un mapa de
//! cuatro ramas para empezar, y las cajas de cargo con un organigrama de
//! ejemplo.
//!
//! La hoja entera de mapa mental ya esta en «Académico»; estas son las
//! piezas sueltas para hacerse el suyo o crecer uno. Cada caja lleva su
//! forma y su texto en partes con nombre.

use super::Lapiz;
use super::diagramas::{BLANCO, G, RELLENO_FUERTE, TAM, TINTA, TRAZO, caja, caja_redonda, rotulo};
use super::electronica::bezier;
use super::redes::busto;
use super::tecnico::{Dibujo, poner};
use crate::biblioteca::FiguraGuardada;

pub(super) const CATEGORIA: &str = "Mapas mentales y organigramas";

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: &[Dibujo] = &[
        ("mapa-tema-central", "Tema central", tema_central),
        ("mapa-idea-principal", "Idea principal", idea_principal),
        ("mapa-subtema", "Subtema", subtema),
        ("mapa-rama", "Rama curva", rama),
        (
            "mapa-cuatro-ramas",
            "Mapa mental de cuatro ramas",
            cuatro_ramas,
        ),
        ("org-cargo", "Caja de cargo con foto", cargo),
        ("org-cargo-simple", "Caja de cargo", cargo_simple),
        ("org-organigrama", "Organigrama de ejemplo", organigrama),
    ];
    poner(v, CATEGORIA, lista);
}

fn tema_central(l: &mut Lapiz) {
    // Parte «Forma»: la elipse grande del centro.
    l.parte("Forma");
    l.elipse(
        0.0,
        0.0,
        220.0,
        100.0,
        Some(TRAZO),
        Some(RELLENO_FUERTE),
        3.0,
    );
    // Parte «Texto»: el tema.
    l.parte("Texto");
    rotulo(l, 110.0, 50.0, "Tema central", 20.0, true);
}

fn idea_principal(l: &mut Lapiz) {
    // Parte «Forma»: la caja redondeada de una idea.
    l.parte("Forma");
    caja_redonda(l, 0.0, 0.0, 160.0, 60.0);
    // Parte «Texto»: la idea.
    l.parte("Texto");
    rotulo(l, 80.0, 30.0, "Idea principal", TAM, true);
}

fn subtema(l: &mut Lapiz) {
    // Parte «Texto»: el subtema, sobre su raya.
    l.parte("Texto");
    rotulo(l, 70.0, 14.0, "Subtema", 15.0, false);
    // Parte «Línea»: la raya donde se apoya (de ella sale la rama).
    l.parte("Línea");
    l.linea(0.0, 34.0, 140.0, 34.0, TRAZO, 3.0);
}

fn rama(l: &mut Lapiz) {
    // Parte «Rama»: la curva gruesa que sale del tema.
    l.parte("Rama");
    l.poli(
        &bezier((0.0, 100.0), (80.0, 100.0), (160.0, 20.0)),
        TRAZO,
        4.0,
        None,
    );
    // Parte «Etiqueta»: la palabra clave sobre la rama.
    l.parte("Etiqueta");
    rotulo(l, 190.0, 10.0, "Rama", 15.0, false);
}

fn cuatro_ramas(l: &mut Lapiz) {
    let (cx, cy) = (300.0, 160.0);
    // Parte «Ramas»: las cuatro curvas que salen del centro.
    l.parte("Ramas");
    let fines = [(110.0, 40.0), (490.0, 40.0), (110.0, 280.0), (490.0, 280.0)];
    for (fx, fy) in fines {
        let control = (cx + (fx - cx) * 0.2, fy);
        l.poli(&bezier((cx, cy), control, (fx, fy)), TRAZO, 4.0, None);
    }
    // Parte «Tema central»: la elipse del centro y su texto.
    l.parte("Tema central");
    l.elipse(
        cx - 100.0,
        cy - 40.0,
        200.0,
        80.0,
        Some(TRAZO),
        Some(RELLENO_FUERTE),
        3.0,
    );
    rotulo(l, cx, cy, "Tema central", 18.0, true);
    for (i, (fx, fy)) in fines.into_iter().enumerate() {
        // Parte «Idea N»: la caja de cada rama con su idea.
        l.parte(&format!("Idea {}", i + 1));
        let x = if fx < cx { fx - 110.0 } else { fx };
        caja_redonda(l, x, fy - 25.0, 110.0, 50.0);
        rotulo(l, x + 55.0, fy, &format!("Idea {}", i + 1), 15.0, false);
    }
}

fn cargo(l: &mut Lapiz) {
    // Parte «Caja»: el rectangulo de la persona.
    l.parte("Caja");
    caja_redonda(l, 0.0, 0.0, 240.0, 80.0);
    // Parte «Foto»: el circulo para la foto.
    l.parte("Foto");
    l.circulo(40.0, 40.0, 28.0, Some(TRAZO), Some(BLANCO), G);
    busto(l, 40.0, 30.0, 8.0);
    // Parte «Nombre»: quien es.
    l.parte("Nombre");
    super::diagramas::rotulo_izq(l, 80.0, 18.0, "Nombre Apellido", TAM, true);
    // Parte «Cargo»: que hace.
    l.parte("Cargo");
    l.texto(
        80.0,
        44.0,
        "Cargo",
        14.0,
        super::WORK_SANS,
        TRAZO,
        false,
        super::Al::Izq,
    );
}

/// Una caja de cargo sencilla en `(x, y)`, con la franja de color arriba.
fn caja_cargo(l: &mut Lapiz, x: f32, y: f32, cargo: &str, nombre: &str) {
    caja(l, x, y, 180.0, 70.0);
    l.rect(x, y, 180.0, 8.0, Some(TRAZO), Some(TRAZO), 1.0, false);
    rotulo(l, x + 90.0, y + 30.0, cargo, 15.0, true);
    l.texto(
        x + 90.0,
        y + 44.0,
        nombre,
        13.0,
        super::WORK_SANS,
        TINTA,
        false,
        super::Al::Centro,
    );
}

fn cargo_simple(l: &mut Lapiz) {
    // Parte «Cargo»: la caja con el cargo y el nombre.
    l.parte("Cargo");
    caja_cargo(l, 0.0, 0.0, "Dirección", "Nombre Apellido");
}

fn organigrama(l: &mut Lapiz) {
    // Parte «Líneas»: las rayas en codo que bajan de la direccion.
    l.parte("Líneas");
    l.linea(320.0, 70.0, 320.0, 110.0, TRAZO, G);
    l.linea(90.0, 110.0, 550.0, 110.0, TRAZO, G);
    for x in [90.0, 320.0, 550.0] {
        l.linea(x, 110.0, x, 150.0, TRAZO, G);
    }
    // Parte «Dirección»: la caja de arriba.
    l.parte("Dirección");
    caja_cargo(l, 230.0, 0.0, "Dirección", "Nombre Apellido");
    for (i, area) in ["Área 1", "Área 2", "Área 3"].into_iter().enumerate() {
        // Parte «Área N»: cada caja de abajo.
        l.parte(area);
        caja_cargo(l, 230.0 * i as f32, 150.0, area, "Responsable");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::figuras_de_serie::diagramas::pruebas::{comprobar, nombres, partes_estampada};

    #[test]
    fn estan_las_de_mapas_y_organigramas_y_cada_pieza_tiene_su_parte() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(v.len(), 8);
        for f in &v {
            assert!(
                f.id.starts_with("serie-mapa-") || f.id.starts_with("serie-org-"),
                "{}",
                f.id
            );
        }
        comprobar(&v, "serie-");
    }

    #[test]
    fn el_organigrama_estampado_conserva_sus_partes() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(
            partes_estampada(&v, "serie-org-organigrama"),
            nombres(&["Líneas", "Dirección", "Área 1", "Área 2", "Área 3"])
        );
    }
}
