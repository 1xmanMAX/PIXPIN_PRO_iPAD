//! **Las figuras de entidad-relacion** (24-oct-2026): las de Chen que se
//! ensenan en bases de datos (entidad, entidad debil, atributos, relacion,
//! especializacion) y las del modelo logico (la tabla con sus claves y las
//! terminaciones de «pata de gallo» de la cardinalidad).
//!
//! Las patas de gallo son una linea de 200 con la marca en la punta de la
//! derecha («Línea» y «Cardinalidad» aparte): se pone la marca contra la
//! tabla y se estira la linea.

use super::Lapiz;
use super::diagramas::{
    BLANCO, G, RELLENO, RELLENO_FUERTE, TAM, TRAZO, caja, forma, raya, rotulo, rotulo_izq,
    rotulo_subrayado,
};
use super::tecnico::{Dibujo, poner};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::EstiloTrazo;

pub(super) const CATEGORIA: &str = "Entidad-relación";

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: &[Dibujo] = &[
        ("er-entidad", "Entidad", |l| entidad(l, false)),
        ("er-entidad-debil", "Entidad débil", |l| entidad(l, true)),
        ("er-atributo", "Atributo", |l| atributo(l, Atributo::Normal)),
        ("er-atributo-clave", "Atributo clave", |l| {
            atributo(l, Atributo::Clave)
        }),
        ("er-atributo-multivaluado", "Atributo multivaluado", |l| {
            atributo(l, Atributo::Multivaluado)
        }),
        ("er-atributo-derivado", "Atributo derivado", |l| {
            atributo(l, Atributo::Derivado)
        }),
        ("er-relacion", "Relación", |l| relacion(l, false)),
        (
            "er-relacion-identificadora",
            "Relación identificadora",
            |l| relacion(l, true),
        ),
        (
            "er-especializacion",
            "Especialización (es un)",
            especializacion,
        ),
        ("er-tabla", "Tabla con claves", tabla),
        ("er-uno-y-solo-uno", "Cardinalidad uno y solo uno", |l| {
            pata(l, Pata::UnoSoloUno)
        }),
        ("er-cero-o-uno", "Cardinalidad cero o uno", |l| {
            pata(l, Pata::CeroOUno)
        }),
        ("er-uno-o-muchos", "Cardinalidad uno o muchos", |l| {
            pata(l, Pata::UnoOMuchos)
        }),
        ("er-cero-o-muchos", "Cardinalidad cero o muchos", |l| {
            pata(l, Pata::CeroOMuchos)
        }),
        ("er-uno-a-muchos", "Relación uno a muchos", uno_a_muchos),
    ];
    poner(v, CATEGORIA, lista);
}

fn entidad(l: &mut Lapiz, debil: bool) {
    // Parte «Forma»: el rectangulo (doble si es debil).
    l.parte("Forma");
    caja(l, 0.0, 0.0, 160.0, 70.0);
    if debil {
        l.rect(6.0, 6.0, 148.0, 58.0, Some(TRAZO), None, G, false);
    }
    // Parte «Nombre»: el nombre de la entidad, en singular.
    l.parte("Nombre");
    rotulo(
        l,
        80.0,
        35.0,
        if debil { "Detalle" } else { "Entidad" },
        TAM,
        true,
    );
}

#[derive(Clone, Copy, PartialEq)]
enum Atributo {
    Normal,
    Clave,
    Multivaluado,
    Derivado,
}

fn atributo(l: &mut Lapiz, tipo: Atributo) {
    // Parte «Forma»: la elipse (doble si es multivaluado, a trazos si es
    // derivado).
    l.parte("Forma");
    l.elipse(0.0, 0.0, 140.0, 60.0, Some(TRAZO), Some(RELLENO), G);
    match tipo {
        Atributo::Derivado => {
            if let Some(e) = l.v.last_mut() {
                e.estilo = EstiloTrazo::Discontinuo;
            }
        }
        Atributo::Multivaluado => {
            l.elipse(6.0, 6.0, 128.0, 48.0, Some(TRAZO), None, G);
        }
        _ => {}
    }
    // Parte «Nombre»: el nombre del atributo (subrayado si es clave).
    l.parte("Nombre");
    let nombre = match tipo {
        Atributo::Normal => "atributo",
        Atributo::Clave => "id",
        Atributo::Multivaluado => "teléfonos",
        Atributo::Derivado => "edad",
    };
    if tipo == Atributo::Clave {
        rotulo_subrayado(l, 70.0, 30.0, nombre, TAM, false);
    } else {
        rotulo(l, 70.0, 30.0, nombre, TAM, false);
    }
}

fn relacion(l: &mut Lapiz, identificadora: bool) {
    // Parte «Forma»: el rombo (doble si identifica a una entidad debil).
    l.parte("Forma");
    forma(l, &[(80.0, 0.0), (160.0, 45.0), (80.0, 90.0), (0.0, 45.0)]);
    if identificadora {
        l.poli(
            &[
                (80.0, 9.0),
                (144.0, 45.0),
                (80.0, 81.0),
                (16.0, 45.0),
                (80.0, 9.0),
            ],
            TRAZO,
            G,
            None,
        );
    }
    // Parte «Nombre»: el verbo de la relacion.
    l.parte("Nombre");
    rotulo(
        l,
        80.0,
        45.0,
        if identificadora {
            "contiene"
        } else {
            "relación"
        },
        15.0,
        false,
    );
}

fn especializacion(l: &mut Lapiz) {
    // Parte «Superclase»: la linea que sube a la entidad general.
    l.parte("Superclase");
    raya(l, (100.0, 0.0), (100.0, 20.0));
    // Parte «Forma»: el triangulo «es un».
    l.parte("Forma");
    forma(l, &[(100.0, 20.0), (150.0, 80.0), (50.0, 80.0)]);
    rotulo(l, 100.0, 60.0, "es un", 13.0, false);
    // Parte «Subclases»: las lineas que bajan a las entidades concretas.
    l.parte("Subclases");
    l.poli(
        &[(0.0, 120.0), (0.0, 100.0), (200.0, 100.0), (200.0, 120.0)],
        TRAZO,
        G,
        None,
    );
    raya(l, (100.0, 80.0), (100.0, 100.0));
}

fn tabla(l: &mut Lapiz) {
    // Parte «Nombre»: la cabecera con el nombre de la tabla.
    l.parte("Nombre");
    l.rect(
        0.0,
        0.0,
        220.0,
        40.0,
        Some(TRAZO),
        Some(RELLENO_FUERTE),
        G,
        false,
    );
    rotulo(l, 110.0, 20.0, "Pedido", TAM, true);
    // Parte «Claves»: la columna PK / FK.
    l.parte("Claves");
    l.rect(0.0, 40.0, 50.0, 120.0, Some(TRAZO), Some(RELLENO), G, false);
    for (i, k) in ["PK", "FK", "", ""].into_iter().enumerate() {
        if !k.is_empty() {
            rotulo(l, 25.0, 55.0 + 30.0 * i as f32, k, 13.0, true);
        }
    }
    // Parte «Campos»: los campos con su tipo, uno por fila.
    l.parte("Campos");
    l.rect(
        50.0,
        40.0,
        170.0,
        120.0,
        Some(TRAZO),
        Some(BLANCO),
        G,
        false,
    );
    for y in [70.0, 100.0, 130.0] {
        l.linea(0.0, y, 220.0, y, TRAZO, 1.0);
    }
    for (i, campo) in [
        "id: entero",
        "cliente_id: entero",
        "fecha: fecha",
        "total: decimal",
    ]
    .into_iter()
    .enumerate()
    {
        let (_, h) = crate::texto::medida(campo, 14.0, super::WORK_SANS, Default::default());
        rotulo_izq(
            l,
            60.0,
            55.0 + 30.0 * i as f32 - h / 2.0,
            campo,
            14.0,
            false,
        );
    }
}

#[derive(Clone, Copy)]
enum Pata {
    UnoSoloUno,
    CeroOUno,
    UnoOMuchos,
    CeroOMuchos,
}

/// La marca de cardinalidad con la punta en `x` (la entidad) y la linea
/// llegando desde el lado `dir` (-1, desde la izquierda; 1, desde la
/// derecha).
fn marca(l: &mut Lapiz, x: f32, y: f32, dir: f32, tipo: Pata) {
    let en = |d: f32| x + dir * d;
    // La raya cerca de la entidad: «uno» (o la pata de gallo: «muchos»).
    match tipo {
        Pata::UnoOMuchos | Pata::CeroOMuchos => {
            l.poli(
                &[(en(0.0), y - 12.0), (en(20.0), y), (en(0.0), y + 12.0)],
                TRAZO,
                G,
                None,
            );
        }
        _ => raya(l, (en(12.0), y - 12.0), (en(12.0), y + 12.0)),
    }
    // La de mas afuera: «uno» o «cero».
    match tipo {
        Pata::UnoSoloUno | Pata::UnoOMuchos => raya(l, (en(26.0), y - 12.0), (en(26.0), y + 12.0)),
        Pata::CeroOUno | Pata::CeroOMuchos => {
            l.circulo(en(34.0), y, 7.0, Some(TRAZO), Some(BLANCO), G);
        }
    }
}

fn pata(l: &mut Lapiz, tipo: Pata) {
    // Parte «Etiqueta»: lo que significa, en palabras.
    l.parte("Etiqueta");
    let s = match tipo {
        Pata::UnoSoloUno => "1..1",
        Pata::CeroOUno => "0..1",
        Pata::UnoOMuchos => "1..*",
        Pata::CeroOMuchos => "0..*",
    };
    rotulo(l, 80.0, 10.0, s, 14.0, false);
    // Parte «Línea»: la linea de la relacion.
    l.parte("Línea");
    raya(l, (0.0, 40.0), (200.0, 40.0));
    // Parte «Cardinalidad»: la marca de pata de gallo, contra la entidad.
    l.parte("Cardinalidad");
    marca(l, 200.0, 40.0, -1.0, tipo);
}

fn uno_a_muchos(l: &mut Lapiz) {
    // Parte «Etiqueta»: el verbo de la relacion.
    l.parte("Etiqueta");
    rotulo(l, 120.0, 10.0, "hace", 14.0, false);
    // Parte «Línea»: la linea entre las dos entidades.
    l.parte("Línea");
    raya(l, (0.0, 40.0), (240.0, 40.0));
    // Parte «Cardinalidad origen»: «uno y solo uno», a la izquierda.
    l.parte("Cardinalidad origen");
    marca(l, 0.0, 40.0, 1.0, Pata::UnoSoloUno);
    // Parte «Cardinalidad destino»: «cero o muchos», a la derecha.
    l.parte("Cardinalidad destino");
    marca(l, 240.0, 40.0, -1.0, Pata::CeroOMuchos);
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::figuras_de_serie::diagramas::pruebas::{comprobar, nombres, partes_estampada};

    #[test]
    fn estan_las_de_entidad_relacion_y_cada_pieza_tiene_su_parte() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(v.len(), 15);
        comprobar(&v, "serie-er-");
    }

    #[test]
    fn la_tabla_estampada_conserva_sus_partes() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(
            partes_estampada(&v, "serie-er-tabla"),
            nombres(&["Nombre", "Claves", "Campos"])
        );
    }
}
