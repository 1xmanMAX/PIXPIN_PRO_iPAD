//! **Las formas del diagrama de flujo** (24-oct-2026), las de la norma
//! ISO 5807: terminador, proceso, decision, datos, documentos, conectores...
//!
//! Antes no habia ninguna en el catalogo (solo el rombo suelto del lienzo).
//! Todas van con el mismo azul y relleno claro, de un tamano parecido
//! (unos 160 × 80) para que un diagrama salga parejo, y la silueta es una
//! region rellena: se puede pinchar por dentro y escribir encima.
//!
//! Cada figura lleva sus partes con nombre ([`Lapiz::parte`]): «Forma» es
//! la silueta y las rayas de dentro van aparte («Barras laterales»,
//! «Cruz»...), para cambiarlas sin tocar la silueta.

use super::electronica::{arco_pts, bezier, region};
use super::tecnico::{Dibujo, poner};
use super::{Lapiz, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::ColorRgba;

pub(super) const CATEGORIA: &str = "Diagrama de flujo";

const TRAZO: ColorRgba = hex(0x2f5f9e);
const RELLENO: ColorRgba = hex(0xeaf1fb);
const G: f32 = 2.0;

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: &[Dibujo] = &[
        ("flujo-terminador", "Terminador (inicio / fin)", terminador),
        ("flujo-proceso", "Proceso", proceso),
        ("flujo-decision", "Decisión", decision),
        ("flujo-demora", "Demora", demora),
        ("flujo-datos", "Datos (entrada / salida)", datos),
        ("flujo-documento", "Documento", |l| {
            documento(l, 0.0, 0.0);
        }),
        (
            "flujo-multiples-documentos",
            "Múltiples documentos",
            multiples_documentos,
        ),
        (
            "flujo-subrutina",
            "Subrutina (proceso predefinido)",
            subrutina,
        ),
        ("flujo-preparacion", "Preparación", preparacion),
        ("flujo-pantalla", "Pantalla", pantalla),
        ("flujo-entrada-manual", "Entrada manual", entrada_manual),
        (
            "flujo-bucle-manual",
            "Bucle manual (operación manual)",
            bucle_manual,
        ),
        ("flujo-limite-bucle", "Límite de bucle", limite_bucle),
        (
            "flujo-datos-almacenados",
            "Datos almacenados",
            datos_almacenados,
        ),
        ("flujo-conector", "Conector", conector),
        (
            "flujo-fuera-pagina-abajo",
            "Conector fuera de página (abajo)",
            |l| fuera_de_pagina(l, 0),
        ),
        (
            "flujo-fuera-pagina-arriba",
            "Conector fuera de página (arriba)",
            |l| fuera_de_pagina(l, 1),
        ),
        (
            "flujo-fuera-pagina-izquierda",
            "Conector fuera de página (izquierda)",
            |l| fuera_de_pagina(l, 2),
        ),
        (
            "flujo-fuera-pagina-derecha",
            "Conector fuera de página (derecha)",
            |l| fuera_de_pagina(l, 3),
        ),
        ("flujo-o", "O", |l| circulo_con(l, false)),
        ("flujo-y", "Y (sumador)", |l| circulo_con(l, true)),
        ("flujo-intercalar", "Intercalar", intercalar),
        ("flujo-ordenar", "Ordenar", ordenar),
        ("flujo-fusionar", "Fusionar", fusionar),
        ("flujo-base-de-datos", "Base de datos", base_de_datos),
        (
            "flujo-almacenamiento-interno",
            "Almacenamiento interno",
            almacenamiento_interno,
        ),
    ];
    poner(v, CATEGORIA, lista);
}

/// La silueta rellena, en su parte «Forma».
fn silueta(l: &mut Lapiz, pts: &[(f32, f32)]) {
    // Parte «Forma»: la silueta del simbolo, donde va el texto.
    l.parte("Forma");
    region(l, pts, TRAZO, RELLENO, G);
}

fn raya(l: &mut Lapiz, a: (f32, f32), b: (f32, f32)) {
    l.linea(a.0, a.1, b.0, b.1, TRAZO, G);
}

/// Inicio o fin: el rectangulo con los extremos en semicirculo.
fn terminador(l: &mut Lapiz) {
    let mut p = arco_pts(125.0, 35.0, 35.0, 35.0, -90.0, 90.0);
    p.extend(arco_pts(35.0, 35.0, 35.0, 35.0, 90.0, 270.0));
    silueta(l, &p);
}

fn proceso(l: &mut Lapiz) {
    silueta(l, &[(0.0, 0.0), (160.0, 0.0), (160.0, 80.0), (0.0, 80.0)]);
}

fn decision(l: &mut Lapiz) {
    silueta(l, &[(80.0, 0.0), (160.0, 50.0), (80.0, 100.0), (0.0, 50.0)]);
}

/// La D: lado recto a la izquierda y semicirculo a la derecha.
fn demora(l: &mut Lapiz) {
    let mut p = vec![(0.0, 0.0)];
    p.extend(arco_pts(110.0, 40.0, 40.0, 40.0, -90.0, 90.0));
    p.push((0.0, 80.0));
    silueta(l, &p);
}

/// El paralelogramo de entrada y salida de datos.
fn datos(l: &mut Lapiz) {
    silueta(l, &[(30.0, 0.0), (170.0, 0.0), (140.0, 80.0), (0.0, 80.0)]);
}

/// La hoja con el borde de abajo en onda, con su esquina en `(ox, oy)`.
fn hoja(ox: f32, oy: f32) -> Vec<(f32, f32)> {
    let mut p = vec![(ox, oy), (ox + 160.0, oy)];
    for i in 0..=24 {
        let x = 160.0 - 160.0 * i as f32 / 24.0;
        let y = 68.0 + 10.0 * (x / 160.0 * std::f32::consts::TAU).sin();
        p.push((ox + x, oy + y));
    }
    p
}

fn documento(l: &mut Lapiz, ox: f32, oy: f32) {
    silueta(l, &hoja(ox, oy));
}

/// Tres hojas en pila: las de atras asoman arriba a la derecha.
fn multiples_documentos(l: &mut Lapiz) {
    // Parte «Hojas de atrás»: las dos que asoman.
    l.parte("Hojas de atrás");
    region(l, &hoja(20.0, 0.0), TRAZO, RELLENO, G);
    region(l, &hoja(10.0, 10.0), TRAZO, RELLENO, G);
    // Parte «Forma»: la hoja de delante, donde va el texto.
    documento(l, 0.0, 20.0);
}

/// El proceso con dos barras dentro a los lados.
fn subrutina(l: &mut Lapiz) {
    proceso(l);
    // Parte «Barras laterales»: lo que la distingue de un proceso.
    l.parte("Barras laterales");
    raya(l, (14.0, 0.0), (14.0, 80.0));
    raya(l, (146.0, 0.0), (146.0, 80.0));
}

/// El hexagono alargado.
fn preparacion(l: &mut Lapiz) {
    silueta(
        l,
        &[
            (30.0, 0.0),
            (130.0, 0.0),
            (160.0, 40.0),
            (130.0, 80.0),
            (30.0, 80.0),
            (0.0, 40.0),
        ],
    );
}

/// Pico curvo a la izquierda y semicirculo a la derecha.
fn pantalla(l: &mut Lapiz) {
    let mut p = bezier((0.0, 40.0), (8.0, 10.0), (30.0, 0.0));
    p.extend(arco_pts(125.0, 40.0, 35.0, 40.0, -90.0, 90.0));
    p.extend(bezier((30.0, 80.0), (8.0, 70.0), (0.0, 40.0)));
    p.pop();
    silueta(l, &p);
}

/// El teclado visto de lado: el borde de arriba sube a la derecha.
fn entrada_manual(l: &mut Lapiz) {
    silueta(l, &[(0.0, 26.0), (160.0, 0.0), (160.0, 80.0), (0.0, 80.0)]);
}

/// El trapecio con el lado ancho arriba.
fn bucle_manual(l: &mut Lapiz) {
    silueta(l, &[(0.0, 0.0), (160.0, 0.0), (132.0, 80.0), (28.0, 80.0)]);
}

/// El rectangulo con las esquinas de arriba cortadas.
fn limite_bucle(l: &mut Lapiz) {
    silueta(
        l,
        &[
            (22.0, 0.0),
            (138.0, 0.0),
            (160.0, 22.0),
            (160.0, 80.0),
            (0.0, 80.0),
            (0.0, 22.0),
        ],
    );
}

/// Curvo por fuera a la izquierda y por dentro a la derecha.
fn datos_almacenados(l: &mut Lapiz) {
    let mut p = vec![(22.0, 0.0)];
    p.extend(bezier((160.0, 0.0), (134.0, 40.0), (160.0, 80.0)));
    p.extend(bezier((22.0, 80.0), (-8.0, 40.0), (22.0, 0.0)));
    p.pop();
    silueta(l, &p);
}

fn conector(l: &mut Lapiz) {
    silueta(l, &arco_pts(30.0, 30.0, 30.0, 30.0, 0.0, 354.0));
}

/// El pentagono que lleva a otra pagina: 0 abajo, 1 arriba, 2 izquierda y
/// 3 derecha (hacia donde apunta el pico).
fn fuera_de_pagina(l: &mut Lapiz, hacia: usize) {
    let p: &[(f32, f32)] = match hacia {
        0 => &[
            (0.0, 0.0),
            (80.0, 0.0),
            (80.0, 60.0),
            (40.0, 90.0),
            (0.0, 60.0),
        ],
        1 => &[
            (40.0, 0.0),
            (80.0, 30.0),
            (80.0, 90.0),
            (0.0, 90.0),
            (0.0, 30.0),
        ],
        2 => &[
            (0.0, 40.0),
            (30.0, 0.0),
            (90.0, 0.0),
            (90.0, 80.0),
            (30.0, 80.0),
        ],
        _ => &[
            (0.0, 0.0),
            (60.0, 0.0),
            (90.0, 40.0),
            (60.0, 80.0),
            (0.0, 80.0),
        ],
    };
    silueta(l, p);
}

/// El circulo con una cruz (O, «or») o con un aspa (Y, «suma»).
fn circulo_con(l: &mut Lapiz, aspa: bool) {
    silueta(l, &arco_pts(35.0, 35.0, 35.0, 35.0, 0.0, 354.0));
    if aspa {
        // Parte «Aspa»: la X del sumador.
        l.parte("Aspa");
        let d = 35.0 * std::f32::consts::FRAC_1_SQRT_2;
        raya(l, (35.0 - d, 35.0 - d), (35.0 + d, 35.0 + d));
        raya(l, (35.0 - d, 35.0 + d), (35.0 + d, 35.0 - d));
    } else {
        // Parte «Cruz»: la + del O.
        l.parte("Cruz");
        raya(l, (0.0, 35.0), (70.0, 35.0));
        raya(l, (35.0, 0.0), (35.0, 70.0));
    }
}

/// El reloj de arena: dos triangulos punta con punta.
fn intercalar(l: &mut Lapiz) {
    silueta(
        l,
        &[
            (0.0, 0.0),
            (100.0, 0.0),
            (50.0, 50.0),
            (100.0, 100.0),
            (0.0, 100.0),
            (50.0, 50.0),
        ],
    );
}

/// El rombo partido por la mitad.
fn ordenar(l: &mut Lapiz) {
    silueta(l, &[(50.0, 0.0), (100.0, 50.0), (50.0, 100.0), (0.0, 50.0)]);
    // Parte «División»: la raya del medio.
    l.parte("División");
    raya(l, (0.0, 50.0), (100.0, 50.0));
}

/// El triangulo con la punta abajo.
fn fusionar(l: &mut Lapiz) {
    silueta(l, &[(0.0, 0.0), (100.0, 0.0), (50.0, 84.0)]);
}

/// El cilindro.
fn base_de_datos(l: &mut Lapiz) {
    let mut p = vec![(0.0, 14.0)];
    p.extend(arco_pts(50.0, 96.0, 50.0, 14.0, 180.0, 0.0));
    p.extend(arco_pts(50.0, 14.0, 50.0, 14.0, 0.0, -180.0));
    p.pop();
    silueta(l, &p);
    // Parte «Tapa»: el borde de delante de la tapa de arriba.
    l.parte("Tapa");
    l.poli(
        &arco_pts(50.0, 14.0, 50.0, 14.0, 0.0, 180.0),
        TRAZO,
        G,
        None,
    );
}

/// El rectangulo con una raya arriba y otra a la izquierda.
fn almacenamiento_interno(l: &mut Lapiz) {
    silueta(l, &[(0.0, 0.0), (130.0, 0.0), (130.0, 100.0), (0.0, 100.0)]);
    // Parte «Rayas de memoria»: las dos que lo distinguen de un proceso.
    l.parte("Rayas de memoria");
    raya(l, (0.0, 16.0), (130.0, 16.0));
    raya(l, (16.0, 0.0), (16.0, 100.0));
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::biblioteca::nombre_de_parte;

    #[test]
    fn estan_las_formas_de_la_norma_y_cada_pieza_tiene_su_parte() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(v.len(), 26);
        for f in &v {
            assert!(f.id.starts_with("serie-flujo-"), "{}", f.id);
            assert!(
                f.elementos
                    .iter()
                    .any(|e| e.grupos.iter().any(|g| nombre_de_parte(g) == Some("Forma"))),
                "{} sin «Forma»",
                f.nombre
            );
            for e in &f.elementos {
                assert_eq!(
                    e.grupos.iter().filter_map(|g| nombre_de_parte(g)).count(),
                    1,
                    "{}",
                    f.nombre
                );
            }
        }
    }
}
