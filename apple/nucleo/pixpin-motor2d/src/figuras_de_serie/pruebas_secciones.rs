//! Las pruebas de las secciones como datos (`secciones.rs`): que la lista de
//! fabrica dibuje lo de siempre, y que quitar, reordenar, ensanchar y
//! validar hagan lo que dicen.

use super::plantillas::{self, H, M, W};
use super::*;
use crate::elemento::Figura;

/// Una hoja de esa plantilla con su tramo de serie y esas secciones.
fn hoja(clave: &str, secs: Option<&[Seccion]>) -> FiguraGuardada {
    plantillas::hacer_con(clave, &plantillas::tramo_de_fabrica(clave), secs).unwrap()
}

fn pedida(titulo: &str) -> SeccionPedida {
    SeccionPedida {
        titulo: titulo.into(),
        ..Default::default()
    }
}

fn con(
    titulo: &str,
    tipo: Option<TipoSeccion>,
    lineas: Option<u32>,
    ancho: Option<AnchoSeccion>,
) -> SeccionPedida {
    SeccionPedida {
        titulo: titulo.into(),
        tipo,
        lineas,
        ancho,
    }
}

fn personalizar(clave: &str, pedidas: Vec<SeccionPedida>) -> Result<FiguraGuardada, String> {
    let iv = Intervalo {
        secciones: Some(pedidas),
        ..Default::default()
    };
    alargar(clave, &iv).map(|mut v| {
        assert_eq!(v.len(), 1);
        v.remove(0)
    })
}

/// Donde esta un texto (su esquina de arriba a la izquierda).
fn texto_en(f: &FiguraGuardada, s: &str) -> Option<(f32, f32)> {
    f.elementos.iter().find_map(|e| match &e.figura {
        Figura::Texto { texto, .. } if texto == s => Some((e.x, e.y)),
        _ => None,
    })
}

/// El largo de la raya de acento bajo el rotulo de una seccion.
fn raya_de(f: &FiguraGuardada, rotulo: &str) -> Option<f32> {
    let (x, y) = texto_en(f, rotulo)?;
    f.elementos.iter().find_map(|e| match &e.figura {
        Figura::Linea { puntos }
            if puntos.len() == 2
                && (puntos[0].x - x).abs() < 0.5
                && (puntos[0].y - (y + 20.0)).abs() < 0.5
                && (puntos[1].y - puntos[0].y).abs() < 0.01 =>
        {
            Some(puntos[1].x - puntos[0].x)
        }
        _ => None,
    })
}

/// Cuantas casillas de marcar (los cuadraditos de 12).
fn casillas(f: &FiguraGuardada) -> usize {
    f.elementos
        .iter()
        .filter(|e| {
            matches!(e.figura, Figura::Rectangulo)
                && (e.ancho - 12.0).abs() < 0.01
                && (e.alto - 12.0).abs() < 0.01
        })
        .count()
}

fn caja(f: &FiguraGuardada) -> (f32, f32, f32, f32) {
    biblioteca::caja_de(&f.elementos).unwrap()
}

/// Las piezas, comparables (sin ids ni semillas).
fn piezas(f: &FiguraGuardada) -> Vec<String> {
    let mut v: Vec<String> = f
        .elementos
        .iter()
        .map(|e| {
            format!(
                "{:?} {:.1} {:.1} {:.1} {:.1} {:?} {:?}",
                e.figura, e.x, e.y, e.ancho, e.alto, e.trazo, e.relleno
            )
        })
        .collect();
    v.sort();
    v
}

/// Lo que dibujaba cada plantilla antes de que sus secciones fueran datos
/// (contado en la de serie del 24-oct): piezas y caja.
const ANTES: [(&str, usize, f32, f32); 11] = [
    ("planificador-mensual", 130, 794.0, 1123.0),
    ("planificador-semanal", 142, 794.0, 1123.0),
    ("planificador-diario", 109, 794.0, 1124.0),
    ("metas", 91, 794.0, 1123.0),
    ("sesion-de-estudio", 99, 794.0, 1123.0),
    ("curso", 91, 794.0, 1123.0),
    ("planificador-de-tareas", 626, 794.0, 1123.0),
    ("viaje", 140, 794.0, 1123.0),
    ("finanzas", 120, 794.0, 1123.0),
    ("receta", 95, 794.0, 1123.0),
    ("cronograma", 96, 1123.0, 795.9),
];

#[test]
fn las_secciones_de_fabrica_dibujan_la_hoja_de_siempre() {
    let catalogo = catalogo();
    for (clave, n, ancho, alto) in ANTES {
        assert!(se_personaliza(clave), "{clave}");
        let serie = catalogo
            .iter()
            .find(|f| f.id == format!("{PREFIJO_ID}{clave}"))
            .unwrap();
        // Mismo numero de piezas y misma caja que antes del cambio.
        assert_eq!(serie.elementos.len(), n, "{clave}");
        let (x0, y0, x1, y1) = caja(serie);
        assert!(
            x0.abs() < 0.1 && y0.abs() < 0.1 && (x1 - ancho).abs() < 0.2 && (y1 - alto).abs() < 0.2,
            "{clave}: {x1} × {y1}"
        );
        // Y la lista de fabrica pasada como datos da lo mismo pieza a pieza.
        let fabrica = secciones_de(clave).unwrap();
        assert_eq!(
            piezas(&hoja(clave, Some(&fabrica))),
            piezas(serie),
            "{clave}"
        );
        // Cada seccion de fabrica tiene su rotulo en la hoja.
        for s in &fabrica {
            assert!(
                texto_en(serie, &s.titulo.to_uppercase()).is_some(),
                "{clave}: falta «{}»",
                s.titulo
            );
        }
    }
    // El cronograma, de serie, no lleva secciones.
    assert!(secciones_de("cronograma").unwrap().is_empty());
    // Caso negativo: las que no se personalizan no tienen lista.
    assert!(secciones_de("lista-de-tareas").is_none());
    assert!(!se_personaliza("mapa-mental"));
}

#[test]
fn el_mensual_como_lo_pidio_el_usuario() {
    // «Quita prioridades, amplia los objetivos del mes a 6 lineas y a todo
    // el ancho arriba, las tareas reducelas a 4».
    let f = personalizar(
        "planificador-mensual",
        vec![
            con(
                "Objetivo del mes",
                None,
                Some(6),
                Some(AnchoSeccion::Completo),
            ),
            con("Tareas", None, Some(4), None),
        ],
    )
    .unwrap();
    assert!(f.nombre.contains("a medida"), "{}", f.nombre);
    assert!(texto_en(&f, "PRIORIDADES").is_none(), "quitada");
    let ancho_util = W - 2.0 * M;
    let raya = raya_de(&f, "OBJETIVO DEL MES").unwrap();
    assert!((raya - ancho_util).abs() < 0.5, "{raya}");
    // Las tareas, debajo del objetivo, con 4 casillas (antes 7).
    let (_, y_obj) = texto_en(&f, "OBJETIVO DEL MES").unwrap();
    let (_, y_tar) = texto_en(&f, "TAREAS").unwrap();
    assert!((y_tar - (y_obj + 20.0 + 6.0 * 30.0 + 20.0)).abs() < 0.5);
    assert_eq!(casillas(&f), 4);
    assert_eq!(casillas(&hoja("planificador-mensual", None)), 7);
    // Sigue siendo el mensual: se reconoce por su titulo.
    assert_eq!(
        cual_es(&f.elementos).as_deref(),
        Some("planificador-mensual")
    );

    // «Objetivos y tareas arriba, prioridades a todo el ancho»: los dos de
    // mitad a la par y alineados arriba, y prioridades debajo de todo.
    let f = personalizar(
        "planificador-mensual",
        vec![
            con("Objetivo del mes", None, Some(5), Some(AnchoSeccion::Mitad)),
            con("Tareas", None, None, Some(AnchoSeccion::Mitad)),
            con("Prioridades", None, None, Some(AnchoSeccion::Completo)),
        ],
    )
    .unwrap();
    let (xo, yo) = texto_en(&f, "OBJETIVO DEL MES").unwrap();
    let (xt, yt) = texto_en(&f, "TAREAS").unwrap();
    let (_, yp) = texto_en(&f, "PRIORIDADES").unwrap();
    assert!((yo - yt).abs() < 0.01 && xt > xo + 300.0);
    // Las tareas (7 × 30) son lo mas alto de la fila.
    assert!((yp - (yt + 20.0 + 7.0 * 30.0 + 20.0)).abs() < 0.5);
    assert!((raya_de(&f, "PRIORIDADES").unwrap() - (W - 2.0 * M)).abs() < 0.5);
}

#[test]
fn reordenar_anadir_y_crecer() {
    // Las tareas primero, a todo el ancho; luego una seccion nueva.
    let f = personalizar(
        "planificador-mensual",
        vec![
            con("Tareas", None, None, Some(AnchoSeccion::Completo)),
            // Nueva: sin ancho va de «mitad»; la de al lado, tambien.
            con("Ideas", Some(TipoSeccion::Notas), Some(4), None),
            con("Objetivo del mes", None, None, Some(AnchoSeccion::Mitad)),
        ],
    )
    .unwrap();
    let (_, yt) = texto_en(&f, "TAREAS").unwrap();
    let (_, yi) = texto_en(&f, "IDEAS").unwrap();
    let (_, yo) = texto_en(&f, "OBJETIVO DEL MES").unwrap();
    assert!(yt < yi && (yi - yo).abs() < 0.01, "{yt} {yi} {yo}");
    // Si no cabe, la hoja crece: tres secciones de 40 lineas.
    let largas = ["Uno", "Dos", "Tres"]
        .map(|t| {
            con(
                t,
                Some(TipoSeccion::Renglones),
                Some(40),
                Some(AnchoSeccion::Completo),
            )
        })
        .to_vec();
    let f = personalizar("planificador-mensual", largas).unwrap();
    assert!(caja(&f).3 > H + 3000.0, "{}", caja(&f).3);
}

#[test]
fn el_libre_baja_hasta_el_pie_y_los_bloques_propios_se_mueven() {
    // En el curso, las notas (libre, sin nada debajo) acaban en el margen.
    let f = hoja("curso", None);
    let notas = f
        .elementos
        .iter()
        .filter(|e| matches!(e.figura, Figura::Rectangulo) && e.redondo)
        .map(|e| e.y + e.alto)
        .fold(0.0_f32, f32::max);
    assert!((notas - (H - M)).abs() < 0.5, "{notas}");
    // Las entregas (bloque propio) a todo el ancho, primero, y sin datos.
    let f = personalizar(
        "curso",
        vec![
            con("Entregas", None, Some(10), Some(AnchoSeccion::Completo)),
            pedida("Fechas importantes"),
            pedida("Notas"),
        ],
    )
    .unwrap();
    assert!(texto_en(&f, "DATOS").is_none());
    assert!((raya_de(&f, "ENTREGAS").unwrap() - (W - 2.0 * M)).abs() < 0.5);
    // Las metas: con secciones y del 1 al 30, los pasos llevan los 30.
    let iv = Intervalo {
        desde: Some(1),
        hasta: Some(30),
        secciones: Some(vec![pedida("Pasos"), pedida("Mi meta")]),
        ..Default::default()
    };
    let f = &alargar("metas", &iv).unwrap()[0];
    assert!(texto_en(f, "30").is_some() && texto_en(f, "HITOS").is_none());
    assert!(f.nombre.contains("1–30 · a medida"), "{}", f.nombre);
}

#[test]
fn el_cronograma_con_su_zona_gris_abajo_o_en_el_lateral() {
    // «El cronograma de 10 semanas y abajo una zona gris donde anotar».
    let notas = || {
        vec![con(
            "Notas",
            Some(TipoSeccion::Notas),
            Some(6),
            Some(AnchoSeccion::Completo),
        )]
    };
    let gris = |f: &FiguraGuardada| {
        f.elementos
            .iter()
            .find(|e| {
                matches!(e.figura, Figura::Rectangulo)
                    && e.relleno == Some(hex(0xf1f3f6))
                    && e.trazo.a == 0.0
            })
            .map(|e| (e.x, e.y, e.ancho, e.alto))
    };
    let iv = Intervalo {
        semanas: Some((1, 10)),
        secciones: Some(notas()),
        ..Default::default()
    };
    let f = &alargar("cronograma", &iv).unwrap()[0];
    let sin = &alargar(
        "cronograma",
        &Intervalo {
            semanas: Some((1, 10)),
            ..Default::default()
        },
    )
    .unwrap()[0];
    let t = textos(f);
    assert!(t.iter().any(|s| s == "S10") && !t.iter().any(|s| s == "S11"));
    let (gx, gy, gw, gh) = gris(f).expect("la zona gris");
    assert!(gris(sin).is_none());
    // Debajo de la leyenda, a lo ancho de la tabla.
    let (_, y_leyenda) = texto_en(f, "Avance").unwrap();
    assert!(gy > y_leyenda + 20.0 && (gx - M).abs() < 0.5);
    assert!((gw - (caja(f).2 - 2.0 * M)).abs() < 0.5);
    assert!((gh - (6.0 * 30.0 - 10.0)).abs() < 0.5);
    assert_eq!(caja(f).2, caja(sin).2, "abajo no ensancha");

    // «O en el lateral»: a la derecha de la tabla; la hoja se ensancha lo justo.
    let mut lateral = notas();
    lateral[0].ancho = Some(AnchoSeccion::Lateral);
    let iv = Intervalo {
        semanas: Some((1, 10)),
        secciones: Some(lateral),
        ..Default::default()
    };
    let f = &alargar("cronograma", &iv).unwrap()[0];
    let (gx, gy, gw, gh) = gris(f).unwrap();
    let ancho_sin = caja(sin).2;
    // Baja hasta el pie de la tabla (la fila 18), no a media altura; y la
    // hoja no crece por ello.
    let (_, y20) = texto_en(f, "18").unwrap();
    assert!(gy + gh > y20, "{} <= {y20}", gy + gh);
    assert_eq!(caja(f).3, caja(sin).3, "el lateral no alarga la hoja");
    assert!(gx > ancho_sin - M, "{gx}");
    assert!((gw - plantillas::LATERAL_DEL_CRONOGRAMA).abs() < 0.5);
    assert!((caja(f).2 - (ancho_sin + 28.0 + plantillas::LATERAL_DEL_CRONOGRAMA)).abs() < 0.5);
    // Caso negativo: el mensual no tiene lateral.
    let mut v = notas();
    v[0].ancho = Some(AnchoSeccion::Lateral);
    let e = personalizar("planificador-mensual", v).unwrap_err();
    assert!(e.contains("lateral"), "{e}");
}

fn textos(f: &FiguraGuardada) -> Vec<String> {
    f.elementos
        .iter()
        .filter_map(|e| match &e.figura {
            Figura::Texto { texto, .. } => Some(texto.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn lo_que_no_vale_se_dice_en_palabras() {
    let m = "planificador-mensual";
    let err = |v: Vec<SeccionPedida>| personalizar(m, v).unwrap_err();
    assert!(err(vec![]).contains("al menos una"));
    assert!(err(vec![con("Tareas", None, Some(0), None)]).contains("de 1 a 40"));
    assert!(err(vec![con("Tareas", None, Some(41), None)]).contains("de 1 a 40"));
    assert!(err(vec![pedida("   ")]).contains("sin título"));
    assert!(err((0..13).map(|i| pedida(&format!("S{i}"))).collect()).contains("como mucho 12"));
    assert!(err(vec![con("Hitos", Some(TipoSeccion::Propia), None, None)]).contains("propia"));
    // En las metas «Hitos» si es propio; uno inventado no.
    assert!(
        personalizar(
            "metas",
            vec![con("Hitos", Some(TipoSeccion::Propia), Some(3), None)]
        )
        .is_ok()
    );
    let e = personalizar(
        "metas",
        vec![con("Gráfica", Some(TipoSeccion::Propia), None, None)],
    )
    .unwrap_err();
    assert!(e.contains("Hitos"), "{e}");
    // Las que no se personalizan.
    let e = personalizar("lista-de-tareas", vec![pedida("Pendientes")]).unwrap_err();
    assert!(
        e.contains("no tiene secciones") && e.contains("Planificador mensual"),
        "{e}"
    );
    // Las que no se alargan pero se personalizan: solo secciones.
    let e = alargar(
        "curso",
        &Intervalo {
            hasta: Some(10),
            secciones: Some(vec![pedida("Notas")]),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(e.contains("solo se cambian las secciones"), "{e}");
    // Lo bueno sigue valiendo, con el titulo sin tildes ni mayusculas.
    let f = personalizar(m, vec![pedida("objetivo del MES")]).unwrap();
    assert!(texto_en(&f, "OBJETIVO DEL MES").is_some());
}

#[test]
fn las_palabras_y_el_json_para_el_modelo() {
    let s = guia_de("planificador-mensual").unwrap();
    assert!(
        s.contains("calendario") && s.contains("\"titulo\":\"Prioridades\""),
        "{s}"
    );
    assert!(s.contains("\"ancho\":\"izquierda\""));
    // Cada seccion, explicada en su linea.
    assert!(
        s.contains("«Prioridades»: renglones numerados") && s.contains("ancho izquierda"),
        "{s}"
    );
    let s = guia_de("metas").unwrap();
    assert!(s.contains("«Hitos»") && s.contains("Progreso"), "{s}");
    assert!(guia_de("cronograma").unwrap().contains("lateral"));
    assert!(guia_de("lista-numerada").is_none());
    assert!(las_que_se_personalizan().contains("Sesión de estudio"));
    for t in TipoSeccion::TODOS {
        assert_eq!(TipoSeccion::de_palabra(t.nombre()), Some(t));
    }
    assert_eq!(
        AnchoSeccion::de_palabra("Todo el ancho"),
        Some(AnchoSeccion::Completo)
    );
    assert_eq!(
        TipoSeccion::de_palabra("zona gris"),
        Some(TipoSeccion::Notas)
    );
    assert_eq!(TipoSeccion::de_palabra("tarta"), None);
}
