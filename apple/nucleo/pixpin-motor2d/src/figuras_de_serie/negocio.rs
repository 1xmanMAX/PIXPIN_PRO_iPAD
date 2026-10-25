//! **Los diagramas de estrategia y negocio** (24-oct-2026): los que mas
//! piden estudiantes y docentes de empresa y los equipos en sus pizarras
//! (DAFO/FODA, matriz 2×2, Eisenhower, embudo, piramide, ciclo PDCA,
//! Venn de 2 y 3, espina de pescado, linea de tiempo, Kanban, lienzo de
//! modelo de negocio, proceso en chevrones y pros y contras).
//!
//! Cada zona es su parte con nombre («Fortalezas», «Nivel 2», «Paso 3»...)
//! con su caja y su texto de ejemplo, para escribir en cada una, cambiarle
//! el color o moverla sin deshacer el resto.

use super::Lapiz;
use super::diagramas::{
    AMBAR, AZUL, BLANCO, G, MORADO, RELLENO, RELLENO_FUERTE, ROJO, TAM, TINTA, TRAZO, Tono, VERDE,
    caja_de, conector, forma_de, rotulo, rotulo_izq,
};
use super::electronica::arco_pts;
use super::tecnico::{Dibujo, poner};
use super::{Al, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, EstiloTrazo};
use crate::formas::TipoPunta;

pub(super) const CATEGORIA: &str = "Estrategia y negocio";

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: &[Dibujo] = &[
        ("neg-dafo", "DAFO / FODA", dafo),
        ("neg-matriz-2x2", "Matriz 2×2", matriz_2x2),
        ("neg-eisenhower", "Matriz de Eisenhower", eisenhower),
        ("neg-embudo", "Embudo", embudo),
        ("neg-piramide", "Pirámide de niveles", piramide),
        ("neg-ciclo", "Ciclo de cuatro pasos (PDCA)", ciclo),
        ("neg-venn-2", "Diagrama de Venn de 2", venn_2),
        ("neg-venn-3", "Diagrama de Venn de 3", venn_3),
        ("neg-ishikawa", "Espina de pescado (Ishikawa)", ishikawa),
        ("neg-linea-de-tiempo", "Línea de tiempo", linea_de_tiempo),
        ("neg-kanban", "Tablero Kanban", kanban),
        (
            "neg-lienzo-modelo-negocio",
            "Lienzo de modelo de negocio",
            lienzo,
        ),
        ("neg-chevrones", "Proceso en chevrones", chevrones),
        ("neg-pros-contras", "Pros y contras", pros_contras),
    ];
    poner(v, CATEGORIA, lista);
}

/// Un texto del color de su zona, a la izquierda.
fn titulo(l: &mut Lapiz, x: f32, y: f32, s: &str, c: ColorRgba) {
    l.texto(x, y, s, 17.0, WORK_SANS, c, true, Al::Izq);
}

/// Un cuadrante con su titulo y sus puntos de ejemplo.
#[allow(clippy::too_many_arguments)]
fn zona(l: &mut Lapiz, x: f32, y: f32, w: f32, h: f32, t: Tono, nombre: &str, cuerpo: &str) {
    l.parte(nombre);
    caja_de(l, x, y, w, h, t, false);
    titulo(l, x + 14.0, y + 12.0, nombre, t.trazo);
    rotulo_izq(l, x + 14.0, y + 44.0, cuerpo, 14.0, false);
}

fn dafo(l: &mut Lapiz) {
    let puntos = "• Punto 1\n• Punto 2\n• Punto 3";
    // Partes «Fortalezas», «Debilidades», «Oportunidades» y «Amenazas»:
    // los cuatro cuadrantes, lo interno arriba y lo externo abajo.
    zona(l, 0.0, 0.0, 240.0, 200.0, VERDE, "Fortalezas", puntos);
    zona(l, 240.0, 0.0, 240.0, 200.0, AMBAR, "Debilidades", puntos);
    zona(l, 0.0, 200.0, 240.0, 200.0, AZUL, "Oportunidades", puntos);
    zona(l, 240.0, 200.0, 240.0, 200.0, ROJO, "Amenazas", puntos);
}

fn matriz_2x2(l: &mut Lapiz) {
    // El area de los cuadrantes: de (40, 40) a (440, 440).
    let (x0, y0, lado) = (40.0, 40.0, 200.0);
    for (i, (t, nombre)) in [
        (AZUL, "Cuadrante 2"),
        (VERDE, "Cuadrante 1"),
        (ROJO, "Cuadrante 3"),
        (AMBAR, "Cuadrante 4"),
    ]
    .into_iter()
    .enumerate()
    {
        let x = x0 + lado * (i % 2) as f32;
        let y = y0 + lado * (i / 2) as f32;
        // Parte «Cuadrante N»: la zona con su nombre.
        l.parte(nombre);
        caja_de(l, x, y, lado, lado, t, false);
        rotulo(l, x + lado / 2.0, y + lado / 2.0, nombre, TAM, true);
    }
    // Parte «Ejes»: las dos flechas con lo que mide cada una.
    l.parte("Ejes");
    conector(
        l,
        &[(x0, y0 + 2.0 * lado), (x0, 0.0)],
        TipoPunta::Ninguna,
        TipoPunta::Flecha,
        EstiloTrazo::Solido,
    );
    conector(
        l,
        &[
            (x0, y0 + 2.0 * lado),
            (x0 + 2.0 * lado + 40.0, y0 + 2.0 * lado),
        ],
        TipoPunta::Ninguna,
        TipoPunta::Flecha,
        EstiloTrazo::Solido,
    );
    rotulo_izq(l, x0 + 10.0, 0.0, "Eje Y (impacto)", 14.0, true);
    l.texto(
        x0 + 2.0 * lado + 40.0,
        y0 + 2.0 * lado + 10.0,
        "Eje X (esfuerzo)",
        14.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Der,
    );
}

fn eisenhower(l: &mut Lapiz) {
    // Parte «Cabeceras»: urgente / no urgente arriba e importante / no
    // importante a la izquierda.
    l.parte("Cabeceras");
    for (i, s) in ["Urgente", "No urgente"].into_iter().enumerate() {
        let x = 120.0 + 200.0 * i as f32;
        l.rect(
            x,
            0.0,
            200.0,
            40.0,
            Some(TRAZO),
            Some(RELLENO_FUERTE),
            G,
            false,
        );
        rotulo(l, x + 100.0, 20.0, s, 15.0, true);
    }
    for (i, s) in ["Importante", "No\nimportante"].into_iter().enumerate() {
        let y = 40.0 + 160.0 * i as f32;
        l.rect(
            0.0,
            y,
            120.0,
            160.0,
            Some(TRAZO),
            Some(RELLENO_FUERTE),
            G,
            false,
        );
        rotulo(l, 60.0, y + 80.0, s, 15.0, true);
    }
    // Partes «Hacer», «Planificar», «Delegar» y «Eliminar»: que hacer con
    // cada tarea segun donde cae.
    zona(l, 120.0, 40.0, 200.0, 160.0, VERDE, "Hacer", "• Tarea");
    zona(l, 320.0, 40.0, 200.0, 160.0, AZUL, "Planificar", "• Tarea");
    zona(l, 120.0, 200.0, 200.0, 160.0, AMBAR, "Delegar", "• Tarea");
    zona(l, 320.0, 200.0, 200.0, 160.0, ROJO, "Eliminar", "• Tarea");
}

/// Los azules de los niveles, del mas claro al mas fuerte.
const NIVELES: [ColorRgba; 4] = [hex(0xeaf1fb), hex(0xd3e3f7), hex(0xbcd4f2), hex(0xa5c5ed)];

fn embudo(l: &mut Lapiz) {
    for (i, s) in ["Atracción", "Interés", "Deseo", "Acción"]
        .into_iter()
        .enumerate()
    {
        let (y0, y1) = (80.0 * i as f32, 80.0 * (i + 1) as f32);
        let (a0, a1) = (200.0 - 40.0 * i as f32, 200.0 - 40.0 * (i + 1) as f32);
        // Parte «Nivel N»: un tramo del embudo con su etapa.
        l.parte(&format!("Nivel {}", i + 1));
        super::electronica::region(
            l,
            &[
                (200.0 - a0, y0),
                (200.0 + a0, y0),
                (200.0 + a1, y1),
                (200.0 - a1, y1),
            ],
            TRAZO,
            NIVELES[i],
            G,
        );
        rotulo(l, 200.0, y0 + 40.0, s, TAM, true);
    }
}

fn piramide(l: &mut Lapiz) {
    // El vertice arriba en (200, 0) y la base de 400 en y = 320.
    for i in 0..4 {
        let (y0, y1) = (80.0 * i as f32, 80.0 * (i + 1) as f32);
        let (a0, a1) = (50.0 * i as f32, 50.0 * (i + 1) as f32);
        // Parte «Nivel N»: una franja de la piramide (el 1, la punta).
        l.parte(&format!("Nivel {}", i + 1));
        let mut p = vec![(200.0 - a0, y0)];
        if a0 > 0.0 {
            p.push((200.0 + a0, y0));
        }
        p.extend([(200.0 + a1, y1), (200.0 - a1, y1)]);
        super::electronica::region(l, &p, TRAZO, NIVELES[3 - i], G);
        rotulo(
            l,
            200.0,
            y0 + if i == 0 { 60.0 } else { 40.0 },
            &format!("Nivel {}", i + 1),
            if i == 0 { 14.0 } else { TAM },
            true,
        );
    }
}

fn ciclo(l: &mut Lapiz) {
    let (cx, cy, r) = (230.0, 200.0, 150.0);
    let pasos = [
        ("Planificar", -90.0, VERDE),
        ("Hacer", 0.0, AZUL),
        ("Verificar", 90.0, AMBAR),
        ("Actuar", 180.0, MORADO),
    ];
    // Parte «Flechas»: los cuatro arcos que van de un paso al siguiente.
    l.parte("Flechas");
    for (_, a, _) in pasos {
        let p = arco_pts(cx, cy, r, r, a + 28.0, a + 62.0);
        conector(
            l,
            &p,
            TipoPunta::Ninguna,
            TipoPunta::Triangulo,
            EstiloTrazo::Solido,
        );
        if let Some(e) = l.v.last_mut() {
            e.grosor = 3.0;
        }
    }
    for (i, (s, a, t)) in pasos.into_iter().enumerate() {
        let (sn, cs) = a.to_radians().sin_cos();
        let (px, py) = (cx + r * cs, cy + r * sn);
        // Parte «Paso N»: la caja de cada paso del ciclo.
        l.parte(&format!("Paso {}", i + 1));
        caja_de(l, px - 70.0, py - 30.0, 140.0, 60.0, t, true);
        rotulo(l, px, py, s, TAM, true);
    }
}

/// Un relleno transparente del tono, para que se vean los cruces.
fn velado(t: Tono) -> ColorRgba {
    ColorRgba {
        a: 0.45,
        ..t.relleno
    }
}

fn conjunto(l: &mut Lapiz, nombre: &str, cx: f32, cy: f32, t: Tono, letra: (f32, f32)) {
    // Parte «Conjunto X»: un circulo y su nombre.
    l.parte(nombre);
    l.circulo(cx, cy, 120.0, Some(t.trazo), Some(velado(t)), G);
    rotulo(
        l,
        letra.0,
        letra.1,
        &nombre["Conjunto ".len()..],
        20.0,
        true,
    );
}

fn venn_2(l: &mut Lapiz) {
    conjunto(l, "Conjunto A", 120.0, 120.0, AZUL, (70.0, 120.0));
    conjunto(l, "Conjunto B", 280.0, 120.0, VERDE, (330.0, 120.0));
    // Parte «Intersección»: lo que tienen en comun.
    l.parte("Intersección");
    rotulo(l, 200.0, 120.0, "A y B", 15.0, false);
}

fn venn_3(l: &mut Lapiz) {
    conjunto(l, "Conjunto A", 120.0, 120.0, AZUL, (90.0, 90.0));
    conjunto(l, "Conjunto B", 280.0, 120.0, VERDE, (310.0, 90.0));
    conjunto(l, "Conjunto C", 200.0, 250.0, AMBAR, (200.0, 310.0));
    // Parte «Intersección»: lo que tienen en comun los tres.
    l.parte("Intersección");
    rotulo(l, 200.0, 165.0, "Todos", 14.0, true);
}

fn ishikawa(l: &mut Lapiz) {
    let ey = 210.0;
    // Parte «Espina»: la raya central que lleva al problema.
    l.parte("Espina");
    conector(
        l,
        &[(0.0, ey), (620.0, ey)],
        TipoPunta::Ninguna,
        TipoPunta::Triangulo,
        EstiloTrazo::Solido,
    );
    if let Some(e) = l.v.last_mut() {
        e.grosor = 4.0;
    }
    // Parte «Problema»: la cabeza del pescado, el efecto.
    l.parte("Problema");
    caja_de(l, 620.0, ey - 50.0, 160.0, 100.0, ROJO, true);
    rotulo(l, 700.0, ey, "Problema", 18.0, true);
    let cats = [
        "Método",
        "Máquina",
        "Material",
        "Mano de obra",
        "Medición",
        "Medio ambiente",
    ];
    for (i, cat) in cats.into_iter().enumerate() {
        let arriba = i < 3;
        let x = 200.0 + 180.0 * (i % 3) as f32;
        let (x0, y0) = if arriba {
            (x - 100.0, 40.0)
        } else {
            (x - 100.0, 2.0 * ey - 40.0)
        };
        // Parte «Categoría N»: la espina de una familia de causas, con su
        // nombre en la punta y una causa de ejemplo.
        l.parte(&format!("Categoría {}", i + 1));
        l.linea(x0, y0, x, ey, TRAZO, 2.5);
        let ty = if arriba { y0 - 30.0 } else { y0 + 8.0 };
        l.texto(x0, ty, cat, 15.0, WORK_SANS, TRAZO, true, Al::Centro);
        let ym = (y0 + ey) / 2.0;
        let xm = (x0 + x) / 2.0;
        l.linea(xm - 70.0, ym, xm, ym, TRAZO, 1.5);
        l.texto(
            xm - 70.0,
            ym - 20.0,
            "Causa",
            13.0,
            WORK_SANS,
            TINTA,
            false,
            Al::Izq,
        );
    }
}

fn linea_de_tiempo(l: &mut Lapiz) {
    let ey = 110.0;
    // Parte «Eje»: la flecha del tiempo.
    l.parte("Eje");
    conector(
        l,
        &[(0.0, ey), (760.0, ey)],
        TipoPunta::Ninguna,
        TipoPunta::Triangulo,
        EstiloTrazo::Solido,
    );
    if let Some(e) = l.v.last_mut() {
        e.grosor = 4.0;
    }
    for i in 0..5 {
        let x = 70.0 + 150.0 * i as f32;
        let arriba = i % 2 == 0;
        // Parte «Hito N»: el punto, su raya y su fecha con lo que paso.
        l.parte(&format!("Hito {}", i + 1));
        let (y_raya, y_txt) = if arriba {
            (ey - 30.0, ey - 100.0)
        } else {
            (ey + 30.0, ey + 40.0)
        };
        l.linea(x, ey, x, y_raya, TRAZO, 1.5);
        l.circulo(x, ey, 10.0, Some(TRAZO), Some(BLANCO), 3.0);
        l.texto(
            x,
            y_txt,
            &format!("{}", 2022 + i),
            17.0,
            WORK_SANS,
            TRAZO,
            true,
            Al::Centro,
        );
        l.texto(
            x,
            y_txt + 24.0,
            "Hito",
            14.0,
            WORK_SANS,
            TINTA,
            false,
            Al::Centro,
        );
    }
}

fn kanban(l: &mut Lapiz) {
    let cols = [
        ("Por hacer", AMBAR, 3),
        ("En curso", AZUL, 2),
        ("Hecho", VERDE, 1),
    ];
    let mut n = 0;
    for (i, (s, t, tarjetas)) in cols.into_iter().enumerate() {
        let x = 240.0 * i as f32;
        // Parte «Columna …»: la columna con su cabecera.
        l.parte(&format!("Columna {s}"));
        l.rect(
            x,
            0.0,
            220.0,
            400.0,
            Some(t.trazo),
            Some(hex(0xf6f7f9)),
            G,
            true,
        );
        l.rect(x, 0.0, 220.0, 44.0, Some(t.trazo), Some(t.relleno), G, true);
        l.texto(x + 16.0, 12.0, s, 17.0, WORK_SANS, t.trazo, true, Al::Izq);
        for k in 0..tarjetas {
            n += 1;
            // Parte «Tarjeta N»: una tarea, para moverla de columna.
            l.parte(&format!("Tarjeta {n}"));
            let y = 60.0 + 80.0 * k as f32;
            l.rect(
                x + 14.0,
                y,
                192.0,
                64.0,
                Some(hex(0xc9ced8)),
                Some(BLANCO),
                1.5,
                true,
            );
            l.rect(
                x + 14.0,
                y,
                6.0,
                64.0,
                Some(t.trazo),
                Some(t.trazo),
                1.0,
                false,
            );
            rotulo_izq(l, x + 30.0, y + 10.0, &format!("Tarea {n}"), 15.0, true);
            rotulo_izq(l, x + 30.0, y + 34.0, "Detalle", 13.0, false);
        }
    }
}

fn lienzo(l: &mut Lapiz) {
    // Las nueve zonas del lienzo, en su sitio de siempre.
    let bloques: [(&str, &str, f32, f32, f32, f32); 9] = [
        ("Socios clave", "Socios\nclave", 0.0, 0.0, 200.0, 400.0),
        (
            "Actividades clave",
            "Actividades\nclave",
            200.0,
            0.0,
            200.0,
            200.0,
        ),
        (
            "Recursos clave",
            "Recursos\nclave",
            200.0,
            200.0,
            200.0,
            200.0,
        ),
        (
            "Propuesta de valor",
            "Propuesta\nde valor",
            400.0,
            0.0,
            200.0,
            400.0,
        ),
        (
            "Relaciones con clientes",
            "Relaciones\ncon clientes",
            600.0,
            0.0,
            200.0,
            200.0,
        ),
        ("Canales", "Canales", 600.0, 200.0, 200.0, 200.0),
        (
            "Segmentos de clientes",
            "Segmentos\nde clientes",
            800.0,
            0.0,
            200.0,
            400.0,
        ),
        (
            "Estructura de costes",
            "Estructura de costes",
            0.0,
            400.0,
            500.0,
            160.0,
        ),
        (
            "Fuentes de ingresos",
            "Fuentes de ingresos",
            500.0,
            400.0,
            500.0,
            160.0,
        ),
    ];
    for (parte, titulo_s, x, y, w, h) in bloques {
        // Parte «…»: cada zona del lienzo con su titulo.
        l.parte(parte);
        let fondo = if parte == "Propuesta de valor" {
            RELLENO_FUERTE
        } else {
            RELLENO
        };
        l.rect(x, y, w, h, Some(TRAZO), Some(fondo), G, false);
        titulo(l, x + 12.0, y + 10.0, titulo_s, TRAZO);
    }
}

fn chevrones(l: &mut Lapiz) {
    for i in 0..5 {
        let x = 150.0 * i as f32;
        let t = [AZUL, VERDE, AMBAR, ROJO, MORADO][i];
        // Parte «Paso N»: un chevron con su paso.
        l.parte(&format!("Paso {}", i + 1));
        let entrada = if i == 0 { 0.0 } else { 30.0 };
        forma_de(
            l,
            &[
                (x, 0.0),
                (x + 140.0, 0.0),
                (x + 170.0, 40.0),
                (x + 140.0, 80.0),
                (x, 80.0),
                (x + entrada, 40.0),
            ],
            t,
        );
        rotulo(
            l,
            x + 85.0 + entrada / 4.0,
            40.0,
            &format!("Paso {}", i + 1),
            15.0,
            true,
        );
    }
}

fn pros_contras(l: &mut Lapiz) {
    let puntos = "• Uno\n• Otro\n• Otro más";
    for (i, (s, t)) in [("Pros", VERDE), ("Contras", ROJO)].into_iter().enumerate() {
        let x = 240.0 * i as f32;
        // Parte «Pros» / «Contras»: la columna con su cabecera.
        l.parte(s);
        l.rect(
            x,
            0.0,
            240.0,
            50.0,
            Some(t.trazo),
            Some(t.relleno),
            G,
            false,
        );
        rotulo(l, x + 120.0, 25.0, s, 18.0, true);
        l.rect(x, 50.0, 240.0, 250.0, Some(t.trazo), Some(BLANCO), G, false);
        rotulo_izq(l, x + 16.0, 66.0, puntos, 15.0, false);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::figuras_de_serie::diagramas::pruebas::{comprobar, nombres, partes_estampada};

    #[test]
    fn estan_las_de_negocio_y_cada_pieza_tiene_su_parte() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(v.len(), 14);
        comprobar(&v, "serie-neg-");
    }

    #[test]
    fn el_dafo_estampado_conserva_sus_cuadrantes() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(
            partes_estampada(&v, "serie-neg-dafo"),
            nombres(&["Fortalezas", "Debilidades", "Oportunidades", "Amenazas"])
        );
        assert_eq!(
            partes_estampada(&v, "serie-neg-lienzo-modelo-negocio").len(),
            9
        );
    }
}
