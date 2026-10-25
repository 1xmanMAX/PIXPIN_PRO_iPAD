//! Las pruebas del modo noche.

use super::super::{
    Intervalo, PREFIJO_ID, SeccionPedida, alargar, catalogo, clave_de, cual_es, es_la_de_noche,
    hex, mes_de_la_hoja,
};
use super::*;
use crate::biblioteca::FiguraGuardada;
use crate::elemento::Elemento;

/// Cada plantilla de pagina con su gemela de noche: `(dia, noche)`.
fn parejas() -> Vec<(FiguraGuardada, FiguraGuardada)> {
    let c = catalogo();
    let v: Vec<_> = c
        .iter()
        .filter(|f| es_la_de_noche(f))
        .map(|n| {
            let id = n.id.strip_suffix(SUFIJO_NOCHE).unwrap();
            let d = c.iter().find(|f| f.id == id).expect("su gemela de dia");
            (d.clone(), n.clone())
        })
        .collect();
    assert!(v.len() >= 25, "{}", v.len());
    v
}

/// La geometria de una pieza, sin sus colores.
fn sin_color(e: &Elemento) -> Elemento {
    let mut e = e.clone();
    e.trazo = ColorRgba::opaco(0.0, 0.0, 0.0);
    e.relleno = e.relleno.map(|_| ColorRgba::opaco(0.0, 0.0, 0.0));
    e
}

#[test]
fn la_de_noche_tiene_las_mismas_piezas_y_la_misma_caja() {
    for (d, n) in parejas() {
        assert_eq!(n.nombre, format!("{} (noche)", d.nombre));
        assert_eq!(d.elementos.len(), n.elementos.len(), "{}", d.nombre);
        assert_eq!(d.tamano(), n.tamano(), "{}", d.nombre);
        for (a, b) in d.elementos.iter().zip(&n.elementos) {
            // Las mismas piezas, en su sitio y con sus partes con nombre.
            assert_eq!(sin_color(a), sin_color(b), "{}", d.nombre);
        }
        assert_ne!(d.elementos, n.elementos, "{}: no ha cambiado", d.nombre);
        // En su misma categoria.
        assert_eq!(
            super::super::categoria_de(&d.nombre).unwrap().0,
            super::super::categoria_de(&n.nombre).unwrap().0
        );
    }
}

#[test]
fn el_papel_es_oscuro_y_la_tinta_clara() {
    for (d, n) in parejas() {
        assert!(!es_de_noche(&d.elementos), "{}", d.nombre);
        assert!(es_de_noche(&n.elementos), "{}", n.nombre);
        // El papel: el rectangulo mas grande, oscuro y azulado.
        let papel = n
            .elementos
            .iter()
            .filter(|e| matches!(e.figura, Figura::Rectangulo))
            .max_by(|a, b| (a.ancho * a.alto).total_cmp(&(b.ancho * b.alto)))
            .and_then(|e| e.relleno)
            .unwrap();
        assert!(luminancia(papel) < 0.03, "{}", n.nombre);
        assert!(papel.b > papel.r, "{}: el papel es azul noche", n.nombre);
        // Y el titulo (su texto mas grande), claro.
        let titulo = n
            .elementos
            .iter()
            .filter_map(|e| match e.figura {
                Figura::Texto { tam, .. } => Some((e.trazo, tam)),
                _ => None,
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap()
            .0;
        assert!(luminancia(titulo) > 0.3, "{}", n.nombre);
    }
}

#[test]
fn todo_el_texto_de_noche_pasa_el_contraste_aa() {
    for (_, n) in parejas() {
        let (t, r, minima) = peor_contraste(&n.elementos).expect("lleva textos");
        assert!(
            r >= minima,
            "{}: «{t}» tiene {r:.2} y pide {minima}",
            n.nombre
        );
    }
    // Y lo que se alarga o se personaliza de noche, tambien.
    let noche = |iv: Intervalo| Intervalo {
        noche: Some(true),
        ..iv
    };
    for (clave, iv) in [
        (
            "planificador-mensual",
            noche(Intervalo {
                mes: Some((2026, 11)),
                ..Default::default()
            }),
        ),
        (
            "habitos",
            noche(Intervalo {
                mes: Some((2028, 2)),
                ..Default::default()
            }),
        ),
        (
            "cronograma",
            noche(Intervalo {
                secciones: Some(vec![SeccionPedida {
                    titulo: "Notas".into(),
                    tipo: Some(super::super::TipoSeccion::Notas),
                    lineas: Some(5),
                    ancho: Some(super::super::AnchoSeccion::Completo),
                }]),
                ..Default::default()
            }),
        ),
    ] {
        for h in alargar(clave, &iv).unwrap() {
            let (t, r, minima) = peor_contraste(&h.elementos).unwrap();
            assert!(r >= minima, "{}: «{t}» {r:.2} < {minima}", h.nombre);
        }
    }
}

#[test]
fn el_contraste_pillado_en_falta() {
    // Una hoja de noche con un texto gris sobre el papel: no pasa.
    let mut l = super::super::Lapiz::default();
    l.rect(0.0, 0.0, 200.0, 100.0, None, Some(hex(FONDO)), 1.0, false);
    l.texto(
        10.0,
        10.0,
        "poco",
        11.0,
        super::super::WORK_SANS,
        hex(0x4a5266),
        false,
        super::super::Al::Izq,
    );
    let (t, r, minima) = peor_contraste(&l.v).unwrap();
    assert_eq!(t, "poco");
    assert!(r < minima, "{r}");
    // Y la tinta de dia sobre el papel de noche, tampoco.
    l.v[1].trazo = hex(0x22304a);
    let (_, r, minima) = peor_contraste(&l.v).unwrap();
    assert!(r < minima, "{r}");
    // Con la de noche, si.
    pintar(&mut l.v[1..]);
    let (_, r, minima) = peor_contraste(&l.v).unwrap();
    assert!(r >= minima, "{r}");
    // Una hoja de dia no es de noche, ni una sin papel.
    assert!(es_de_noche(&l.v));
    assert!(!es_de_noche(&l.v[1..]));
}

#[test]
fn cada_color_de_dia_tiene_su_pareja_de_noche() {
    for (d, _) in parejas() {
        for e in &d.elementos {
            for c in std::iter::once(e.trazo).chain(e.relleno) {
                if c.a > 0.0 {
                    let h = a_hex(c);
                    assert!(
                        PALETA.iter().any(|(dia, _)| *dia == h),
                        "{}: #{h:06x} sin pareja de noche",
                        d.nombre
                    );
                }
            }
        }
    }
    // Lo que no esta en la paleta tambien se oscurece (o se aclara).
    assert!(luminancia(de_noche(hex(0xfafafa))) < 0.05);
    assert!(luminancia(de_noche(hex(0x202020))) > 0.4);
}

#[test]
fn alargar_una_de_noche_sigue_de_noche() {
    let iv = Intervalo {
        desde: Some(1),
        hasta: Some(30),
        semanas: Some((1, 20)),
        noche: Some(true),
        ..Default::default()
    };
    let h = alargar("cronograma", &iv).unwrap();
    assert_eq!(h.len(), 1);
    assert!(es_de_noche(&h[0].elementos));
    assert!(h[0].nombre.ends_with("· noche)"), "{}", h[0].nombre);
    assert_eq!(cual_es(&h[0].elementos).as_deref(), Some("cronograma"));
    // Partida en hojas, todas de noche.
    let partidas = alargar(
        "lista-de-tareas",
        &Intervalo {
            desde: Some(1),
            hasta: Some(40),
            de_a: Some(10),
            noche: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(partidas.len(), 4);
    assert!(partidas.iter().all(|f| es_de_noche(&f.elementos)));
    // Sin el modo, de dia: el caso de antes no cambia.
    let de_dia = alargar(
        "cronograma",
        &Intervalo {
            noche: None,
            ..iv.clone()
        },
    )
    .unwrap();
    assert!(!es_de_noche(&de_dia[0].elementos));
    // De noche, las mismas piezas que de dia.
    assert_eq!(de_dia[0].elementos.len(), h[0].elementos.len());
}

#[test]
fn solo_el_modo_rehace_la_de_serie_en_el_otro() {
    // Tambien las que no se alargan ni se personalizan (la gratitud).
    for clave in [
        "gratitud",
        "planificador-semanal",
        "planificador-mensual",
        "habitos",
    ] {
        let n = alargar(
            clave,
            &Intervalo {
                noche: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(n.len(), 1);
        assert!(es_de_noche(&n[0].elementos), "{clave}");
        assert_eq!(cual_es(&n[0].elementos).as_deref(), Some(clave));
        let d = alargar(
            clave,
            &Intervalo {
                noche: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!es_de_noche(&d[0].elementos), "{clave}");
        assert_eq!(d[0].elementos.len(), n[0].elementos.len());
    }
    // Un simbolo no es una plantilla de pagina.
    assert!(
        alargar(
            "claqueta",
            &Intervalo {
                noche: Some(true),
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn cual_es_y_clave_de_reconocen_la_de_noche() {
    for (d, n) in parejas() {
        assert_eq!(clave_de(&n), clave_de(&d), "{}", n.id);
        assert!(n.id.starts_with(PREFIJO_ID));
        assert_eq!(cual_es(&n.elementos), cual_es(&d.elementos), "{}", n.nombre);
    }
    let c = catalogo();
    let n = c.iter().find(|f| f.id == "serie-cronograma-noche").unwrap();
    assert_eq!(clave_de(n), Some("cronograma"));
    assert_eq!(cual_es(&n.elementos).as_deref(), Some("cronograma"));
    assert!(es_la_de_noche(n));
    // Lo que no es de noche no lo parece.
    let d = c.iter().find(|f| f.id == "serie-cronograma").unwrap();
    assert!(!es_la_de_noche(d));
    let s = c.iter().find(|f| f.id == "serie-claqueta").unwrap();
    assert!(!es_la_de_noche(s) && cual_es(&s.elementos).is_none());
}

#[test]
fn el_mes_se_lee_de_la_hoja() {
    let mes = |clave: &str, m| {
        alargar(
            clave,
            &Intervalo {
                mes: Some(m),
                noche: Some(true),
                ..Default::default()
            },
        )
        .unwrap()
        .remove(0)
    };
    assert_eq!(
        mes_de_la_hoja(&mes("planificador-mensual", (2026, 11)).elementos),
        Some((2026, 11))
    );
    assert_eq!(
        mes_de_la_hoja(&mes("habitos", (2028, 2)).elementos),
        Some((2028, 2))
    );
    let c = catalogo();
    let de_serie = c
        .iter()
        .find(|f| f.id == "serie-planificador-mensual")
        .unwrap();
    assert_eq!(mes_de_la_hoja(&de_serie.elementos), None);
}
