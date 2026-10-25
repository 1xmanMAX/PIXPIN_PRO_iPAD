//! **Formas basicas y llamadas** (24-oct-2026): lo que traen todas las
//! pizarras (Visio, draw.io, Miro, FigJam, Canva) y faltaba aqui:
//! bocadillos de dialogo y de pensamiento, llamadas con guia o con flecha,
//! la nota adhesiva, la llave, las flechas gruesas (derecha, doble, curva,
//! en U, en cruz y chevron), estrella, estallido, cinta, el contenedor con
//! titulo, una tabla sencilla y los iconos de estado (bien, mal, aviso,
//! informacion y pregunta).
//!
//! Las siluetas son regiones rellenas («Forma»), con su texto aparte
//! («Texto») donde lo llevan, para escribir dentro sin tocar el contorno.

use super::Lapiz;
use super::diagramas::{
    AMARILLO, AMBAR, AZUL, BLANCO, G, MORADO, RELLENO, RELLENO_FUERTE, ROJO, TAM, TINTA, TRAZO,
    Tono, VERDE, forma, forma_de, punto, rotulo, rotulo_izq,
};
use super::electronica::{arco_pts, bezier, region};
use super::redes::nube_pts;
use super::tecnico::{Dibujo, poner};
use crate::biblioteca::FiguraGuardada;

pub(super) const CATEGORIA: &str = "Formas y llamadas";

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: &[Dibujo] = &[
        ("forma-bocadillo", "Bocadillo de diálogo", bocadillo),
        (
            "forma-bocadillo-ovalado",
            "Bocadillo ovalado",
            bocadillo_ovalado,
        ),
        ("forma-pensamiento", "Bocadillo de pensamiento", pensamiento),
        ("forma-llamada-guia", "Llamada con guía", llamada_guia),
        ("forma-llamada-flecha", "Llamada con flecha", llamada_flecha),
        ("forma-nota-adhesiva", "Nota adhesiva", nota_adhesiva),
        ("forma-llave", "Llave", llave),
        ("forma-flecha-derecha", "Flecha gruesa", flecha_derecha),
        ("forma-flecha-doble", "Flecha gruesa doble", flecha_doble),
        ("forma-flecha-curva", "Flecha gruesa curva", flecha_curva),
        ("forma-flecha-u", "Flecha en U", flecha_u),
        (
            "forma-flecha-cruz",
            "Flecha en cuatro direcciones",
            flecha_cruz,
        ),
        ("forma-chevron", "Chevron", chevron),
        ("forma-estrella", "Estrella", estrella),
        ("forma-estallido", "Estallido", estallido),
        ("forma-cinta", "Cinta (banner)", cinta),
        ("forma-contenedor", "Contenedor con título", contenedor),
        ("forma-tabla", "Tabla de 3 × 3", tabla),
        ("forma-icono-bien", "Icono Correcto", |l| {
            icono(l, Icono::Bien)
        }),
        ("forma-icono-mal", "Icono Incorrecto", |l| {
            icono(l, Icono::Mal)
        }),
        ("forma-icono-aviso", "Icono Advertencia", |l| {
            icono(l, Icono::Aviso)
        }),
        ("forma-icono-info", "Icono Información", |l| {
            icono(l, Icono::Info)
        }),
        ("forma-icono-pregunta", "Icono Pregunta", |l| {
            icono(l, Icono::Pregunta)
        }),
    ];
    poner(v, CATEGORIA, lista);
}

/// El contorno de un rectangulo redondeado, en el sentido del reloj desde
/// arriba a la izquierda; con `cola`, el pico del bocadillo en el borde
/// de abajo (sus tres puntos de derecha a izquierda).
fn redondeado(w: f32, h: f32, r: f32, cola: Option<[(f32, f32); 3]>) -> Vec<(f32, f32)> {
    let mut p = Vec::new();
    p.extend(arco_pts(w - r, r, r, r, -90.0, 0.0));
    p.extend(arco_pts(w - r, h - r, r, r, 0.0, 90.0));
    if let Some(c) = cola {
        p.extend(c);
    }
    p.extend(arco_pts(r, h - r, r, r, 90.0, 180.0));
    p.extend(arco_pts(r, r, r, r, 180.0, 270.0));
    p
}

fn bocadillo(l: &mut Lapiz) {
    // Parte «Forma»: el globo con su pico abajo a la izquierda.
    l.parte("Forma");
    forma(
        l,
        &redondeado(
            200.0,
            100.0,
            18.0,
            Some([(80.0, 100.0), (30.0, 140.0), (50.0, 100.0)]),
        ),
    );
    // Parte «Texto»: lo que dice.
    l.parte("Texto");
    rotulo(l, 100.0, 50.0, "¡Hola!", TAM, false);
}

fn bocadillo_ovalado(l: &mut Lapiz) {
    // Parte «Forma»: el ovalo con su pico.
    l.parte("Forma");
    let mut p = arco_pts(100.0, 55.0, 100.0, 55.0, -60.0, 110.0);
    p.push((40.0, 140.0));
    p.extend(arco_pts(100.0, 55.0, 100.0, 55.0, 125.0, 300.0));
    forma(l, &p);
    // Parte «Texto»: lo que dice.
    l.parte("Texto");
    rotulo(l, 100.0, 55.0, "Texto", TAM, false);
}

fn pensamiento(l: &mut Lapiz) {
    // Parte «Forma»: la nube del pensamiento.
    l.parte("Forma");
    forma(l, &nube_pts(0.0, 0.0, 220.0, 130.0));
    // Parte «Burbujas»: las bolitas que bajan hasta quien piensa.
    l.parte("Burbujas");
    l.circulo(50.0, 146.0, 10.0, Some(TRAZO), Some(RELLENO), G);
    l.circulo(30.0, 172.0, 6.0, Some(TRAZO), Some(RELLENO), G);
    // Parte «Texto»: lo que piensa.
    l.parte("Texto");
    rotulo(l, 112.0, 70.0, "Mmm…", TAM, false);
}

fn llamada_guia(l: &mut Lapiz) {
    // Parte «Guía»: la raya que senala, con su punto en la punta.
    l.parte("Guía");
    l.poli(&[(40.0, 80.0), (40.0, 120.0), (0.0, 150.0)], TRAZO, G, None);
    punto(l, 0.0, 150.0, 5.0, TRAZO);
    // Parte «Caja»: el recuadro del comentario.
    l.parte("Caja");
    l.rect(10.0, 0.0, 180.0, 80.0, Some(TRAZO), Some(RELLENO), G, true);
    // Parte «Texto»: el comentario.
    l.parte("Texto");
    rotulo(l, 100.0, 40.0, "Comentario", TAM, false);
}

fn llamada_flecha(l: &mut Lapiz) {
    // Parte «Forma»: la caja con su flecha gruesa hacia abajo.
    l.parte("Forma");
    forma(
        l,
        &[
            (0.0, 0.0),
            (180.0, 0.0),
            (180.0, 80.0),
            (110.0, 80.0),
            (110.0, 100.0),
            (130.0, 100.0),
            (90.0, 140.0),
            (50.0, 100.0),
            (70.0, 100.0),
            (70.0, 80.0),
            (0.0, 80.0),
        ],
    );
    // Parte «Texto»: lo que se senala.
    l.parte("Texto");
    rotulo(l, 90.0, 40.0, "Mira aquí", TAM, false);
}

fn nota_adhesiva(l: &mut Lapiz) {
    // Parte «Nota»: el cuadrado amarillo con la esquina levantada.
    l.parte("Nota");
    let sombra = crate::elemento::ColorRgba { a: 0.18, ..TINTA };
    region(
        l,
        &[(6.0, 8.0), (166.0, 8.0), (166.0, 166.0), (6.0, 166.0)],
        sombra,
        sombra,
        1.0,
    );
    forma_de(
        l,
        &[
            (0.0, 0.0),
            (160.0, 0.0),
            (160.0, 130.0),
            (130.0, 160.0),
            (0.0, 160.0),
        ],
        AMARILLO,
    );
    region(
        l,
        &[(160.0, 130.0), (130.0, 160.0), (134.0, 134.0)],
        AMARILLO.trazo,
        super::hex(0xf2dc6b),
        1.5,
    );
    // Parte «Texto»: lo que se apunta.
    l.parte("Texto");
    l.texto(
        14.0,
        14.0,
        "Idea",
        24.0,
        super::CAVEAT,
        TINTA,
        false,
        super::Al::Izq,
    );
}

fn llave(l: &mut Lapiz) {
    // Parte «Llave»: la llave que abre a la derecha, de 200 de alto.
    l.parte("Llave");
    let mut p = bezier((40.0, 0.0), (20.0, 0.0), (20.0, 30.0));
    p.extend(bezier((20.0, 70.0), (20.0, 100.0), (0.0, 100.0)));
    p.extend(bezier((0.0, 100.0), (20.0, 100.0), (20.0, 130.0)));
    p.extend(bezier((20.0, 170.0), (20.0, 200.0), (40.0, 200.0)));
    l.poli(&p, TRAZO, 3.0, None);
    // Parte «Texto»: lo que abarca.
    l.parte("Texto");
    rotulo_izq(l, 56.0, 88.0, "Grupo", TAM, true);
}

fn flecha_derecha(l: &mut Lapiz) {
    // Parte «Forma»: la flecha gruesa hacia la derecha.
    l.parte("Forma");
    forma(
        l,
        &[
            (0.0, 20.0),
            (110.0, 20.0),
            (110.0, 0.0),
            (160.0, 40.0),
            (110.0, 80.0),
            (110.0, 60.0),
            (0.0, 60.0),
        ],
    );
}

fn flecha_doble(l: &mut Lapiz) {
    // Parte «Forma»: la flecha gruesa de dos puntas.
    l.parte("Forma");
    forma(
        l,
        &[
            (0.0, 40.0),
            (50.0, 0.0),
            (50.0, 20.0),
            (150.0, 20.0),
            (150.0, 0.0),
            (200.0, 40.0),
            (150.0, 80.0),
            (150.0, 60.0),
            (50.0, 60.0),
            (50.0, 80.0),
        ],
    );
}

fn flecha_curva(l: &mut Lapiz) {
    // Parte «Forma»: la banda en arco que acaba en punta.
    l.parte("Forma");
    let (cx, cy) = (110.0, 120.0);
    let pol = |r: f32, a: f32| {
        let (s, c) = a.to_radians().sin_cos();
        (cx + r * c, cy + r * s)
    };
    let mut p = arco_pts(cx, cy, 110.0, 110.0, 180.0, 300.0);
    p.push(pol(130.0, 300.0));
    p.push(pol(90.0, 335.0));
    p.push(pol(50.0, 300.0));
    p.extend(arco_pts(cx, cy, 70.0, 70.0, 300.0, 180.0));
    forma(l, &p);
}

fn flecha_u(l: &mut Lapiz) {
    // Parte «Forma»: la banda que da la vuelta y vuelve con su punta.
    l.parte("Forma");
    let mut p = vec![(0.0, 0.0), (100.0, 0.0)];
    p.extend(arco_pts(100.0, 80.0, 80.0, 80.0, -90.0, 90.0));
    p.extend([
        (60.0, 160.0),
        (60.0, 180.0),
        (10.0, 140.0),
        (60.0, 100.0),
        (60.0, 120.0),
        (100.0, 120.0),
    ]);
    p.extend(arco_pts(100.0, 80.0, 40.0, 40.0, 90.0, -90.0));
    p.push((0.0, 40.0));
    forma(l, &p);
}

fn flecha_cruz(l: &mut Lapiz) {
    // Parte «Forma»: las cuatro flechas en cruz.
    l.parte("Forma");
    forma(
        l,
        &[
            (80.0, 0.0),
            (110.0, 30.0),
            (95.0, 30.0),
            (95.0, 65.0),
            (130.0, 65.0),
            (130.0, 50.0),
            (160.0, 80.0),
            (130.0, 110.0),
            (130.0, 95.0),
            (95.0, 95.0),
            (95.0, 130.0),
            (110.0, 130.0),
            (80.0, 160.0),
            (50.0, 130.0),
            (65.0, 130.0),
            (65.0, 95.0),
            (30.0, 95.0),
            (30.0, 110.0),
            (0.0, 80.0),
            (30.0, 50.0),
            (30.0, 65.0),
            (65.0, 65.0),
            (65.0, 30.0),
            (50.0, 30.0),
        ],
    );
}

fn chevron(l: &mut Lapiz) {
    // Parte «Forma»: el chevron.
    l.parte("Forma");
    forma(
        l,
        &[
            (0.0, 0.0),
            (120.0, 0.0),
            (160.0, 40.0),
            (120.0, 80.0),
            (0.0, 80.0),
            (40.0, 40.0),
        ],
    );
}

/// Los puntos de una estrella de `n` picos.
fn picos(cx: f32, cy: f32, n: usize, r_fuera: f32, r_dentro: f32) -> Vec<(f32, f32)> {
    (0..2 * n)
        .map(|i| {
            let r = if i % 2 == 0 { r_fuera } else { r_dentro };
            let a = (-90.0 + 180.0 * i as f32 / n as f32).to_radians();
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

fn estrella(l: &mut Lapiz) {
    // Parte «Forma»: la estrella de cinco puntas.
    l.parte("Forma");
    forma_de(l, &picos(80.0, 84.0, 5, 80.0, 34.0), AMARILLO);
}

fn estallido(l: &mut Lapiz) {
    // Parte «Forma»: el estallido de dieciseis picos.
    l.parte("Forma");
    forma_de(
        l,
        &picos(100.0, 100.0, 16, 100.0, 74.0),
        Tono {
            trazo: ROJO.trazo,
            relleno: AMARILLO.relleno,
        },
    );
    // Parte «Texto»: lo que se anuncia.
    l.parte("Texto");
    texto_de(l, 100.0, 100.0, "¡Nuevo!", 22.0, ROJO.trazo);
}

/// Un texto en negrita de un color, centrado.
fn texto_de(l: &mut Lapiz, cx: f32, cy: f32, s: &str, tam: f32, c: crate::elemento::ColorRgba) {
    super::electronica::texto_en(l, cx, cy, s, tam, c, true);
}

fn cinta(l: &mut Lapiz) {
    // Parte «Puntas»: los extremos con su muesca, por detras.
    l.parte("Puntas");
    forma_de(
        l,
        &[
            (0.0, 20.0),
            (60.0, 20.0),
            (60.0, 80.0),
            (0.0, 80.0),
            (20.0, 50.0),
        ],
        AZUL,
    );
    forma_de(
        l,
        &[
            (240.0, 20.0),
            (300.0, 20.0),
            (280.0, 50.0),
            (300.0, 80.0),
            (240.0, 80.0),
        ],
        AZUL,
    );
    region(
        l,
        &[(40.0, 60.0), (60.0, 60.0), (60.0, 80.0)],
        TRAZO,
        TRAZO,
        1.0,
    );
    region(
        l,
        &[(240.0, 60.0), (260.0, 60.0), (240.0, 80.0)],
        TRAZO,
        TRAZO,
        1.0,
    );
    // Parte «Banda»: la franja de delante, donde va el titulo.
    l.parte("Banda");
    l.rect(
        40.0,
        0.0,
        220.0,
        60.0,
        Some(TRAZO),
        Some(RELLENO_FUERTE),
        G,
        false,
    );
    // Parte «Texto»: el titulo.
    l.parte("Texto");
    rotulo(l, 150.0, 30.0, "Título", 20.0, true);
}

fn contenedor(l: &mut Lapiz) {
    // Parte «Marco»: la caja que agrupa.
    l.parte("Marco");
    l.rect(0.0, 0.0, 300.0, 200.0, Some(TRAZO), Some(BLANCO), G, false);
    // Parte «Cabecera»: la franja de arriba con el titulo.
    l.parte("Cabecera");
    l.rect(
        0.0,
        0.0,
        300.0,
        40.0,
        Some(TRAZO),
        Some(RELLENO_FUERTE),
        G,
        false,
    );
    rotulo_izq(l, 14.0, 10.0, "Título", TAM, true);
}

fn tabla(l: &mut Lapiz) {
    // Parte «Cabecera»: la fila de arriba con los nombres de columna.
    l.parte("Cabecera");
    l.rect(
        0.0,
        0.0,
        360.0,
        40.0,
        Some(TRAZO),
        Some(RELLENO_FUERTE),
        G,
        false,
    );
    for c in 0..3 {
        let x = 120.0 * c as f32;
        rotulo(l, x + 60.0, 20.0, &format!("Columna {}", c + 1), 15.0, true);
    }
    // Parte «Filas»: la rejilla con los datos.
    l.parte("Filas");
    l.rect(0.0, 40.0, 360.0, 120.0, Some(TRAZO), Some(BLANCO), G, false);
    for f in 1..3 {
        let y = 40.0 + 40.0 * f as f32;
        l.linea(0.0, y, 360.0, y, TRAZO, 1.0);
    }
    for c in 1..3 {
        let x = 120.0 * c as f32;
        l.linea(x, 0.0, x, 160.0, TRAZO, 1.0);
    }
    for f in 0..3 {
        for c in 0..3 {
            rotulo(
                l,
                60.0 + 120.0 * c as f32,
                60.0 + 40.0 * f as f32,
                "Dato",
                14.0,
                false,
            );
        }
    }
}

#[derive(Clone, Copy)]
enum Icono {
    Bien,
    Mal,
    Aviso,
    Info,
    Pregunta,
}

fn icono(l: &mut Lapiz, tipo: Icono) {
    let (t, r) = match tipo {
        Icono::Bien => (VERDE, 30.0),
        Icono::Mal => (ROJO, 30.0),
        Icono::Aviso => (AMBAR, 30.0),
        Icono::Info => (AZUL, 30.0),
        Icono::Pregunta => (MORADO, 30.0),
    };
    // Parte «Fondo»: el circulo (o el triangulo del aviso) de color.
    l.parte("Fondo");
    match tipo {
        Icono::Aviso => region(
            l,
            &[(35.0, 0.0), (70.0, 60.0), (0.0, 60.0)],
            t.trazo,
            t.trazo,
            G,
        ),
        _ => l.circulo(r, r, r, Some(t.trazo), Some(t.trazo), G),
    }
    // Parte «Símbolo»: la marca blanca de dentro.
    l.parte("Símbolo");
    match tipo {
        Icono::Bien => l.poli(
            &[(16.0, 31.0), (26.0, 41.0), (45.0, 20.0)],
            BLANCO,
            6.0,
            None,
        ),
        Icono::Mal => {
            l.linea(19.0, 19.0, 41.0, 41.0, BLANCO, 6.0);
            l.linea(19.0, 41.0, 41.0, 19.0, BLANCO, 6.0);
        }
        Icono::Aviso => texto_de(l, 35.0, 38.0, "!", 30.0, BLANCO),
        Icono::Info => texto_de(l, 30.0, 30.0, "i", 32.0, BLANCO),
        Icono::Pregunta => texto_de(l, 30.0, 30.0, "?", 32.0, BLANCO),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::figuras_de_serie::diagramas::pruebas::{comprobar, nombres, partes_estampada};

    #[test]
    fn estan_las_formas_y_llamadas_y_cada_pieza_tiene_su_parte() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(v.len(), 23);
        comprobar(&v, "serie-forma-");
    }

    #[test]
    fn el_bocadillo_estampado_conserva_sus_partes() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(
            partes_estampada(&v, "serie-forma-bocadillo"),
            nombres(&["Forma", "Texto"])
        );
        assert_eq!(
            partes_estampada(&v, "serie-forma-icono-aviso"),
            nombres(&["Fondo", "Símbolo"])
        );
    }
}
