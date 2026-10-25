use serde_json::json;

use super::colocar::{Colocador, Franja, Lado};
use super::flujo::Flujo;
use super::*;

/// Una hoja de 1400 de ancho con renglones de texto en una columna de 300 a
/// 1100 (margenes de 300: caben notas a los dos lados), letra de 26.
fn hoja(renglones: &[&str]) -> Pagina {
    let mut v = Vec::new();
    for (i, t) in renglones.iter().enumerate() {
        let y = 200.0 + i as f32 * 40.0;
        let mut x = 300.0;
        let mut palabras = Vec::new();
        for w in t.split_whitespace() {
            let ancho = w.chars().count() as f32 * 13.0;
            palabras.push(Palabra::nueva(w, [x, y, x + ancho, y + 26.0]));
            x += ancho + 8.0;
        }
        v.push(Renglon {
            palabras,
            etiqueta: None,
        });
    }
    Pagina {
        numero: 0,
        caja: [0.0, 0.0, 1400.0, 1980.0],
        renglones: v,
        letra: 20.0,
        fuera: 0.0,
        solo_derecha: false,
    }
}

const TEXTO: &[&str] = &[
    "Capitulo 2. El modelo de la viga",
    "La viga se apoya en dos columnas y soporta",
    "una carga repartida de 12 kN/m en toda su luz.",
    "El momento maximo vale M = q L^2 / 8 y aparece",
    "en el centro del vano, donde la flecha es mayor.",
    "Para el calculo se usa el modelo elastico lineal.",
];

fn pedida(clase: Clase, ancla: &str, nota: &str) -> Pedida {
    Pedida {
        clase,
        pagina: Some(0),
        ancla: ancla.into(),
        linea: None,
        nota: nota.into(),
        correccion: None,
        numero: None,
    }
}

fn solapan(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

// ---------------------------------------------------------------------------
// Anclar

#[test]
fn ancla_el_trozo_exacto_sin_mirar_mayusculas_tildes_ni_signos() {
    let p = hoja(TEXTO);
    let a = ancla::anclar(&p, "carga repartida de 12 kN/m", None).unwrap();
    assert!(a.exacto);
    assert_eq!((a.renglon, a.ultimo), (2, 2));
    // Empieza en «carga» (la segunda palabra del renglon) y no en «una».
    let carga = p.renglones[2].palabras[1].caja;
    assert_eq!(a.cajas[0][0], carga[0]);
    // Con espacios distintos (el modelo separa la formula) tambien.
    let f = ancla::anclar(&p, "M = q L^2/8", None).unwrap();
    assert_eq!(f.renglon, 3);
    // Y en dos renglones: una caja por renglon.
    let dos = ancla::anclar(&p, "aparece en el centro del vano", None).unwrap();
    assert_eq!((dos.renglon, dos.ultimo, dos.cajas.len()), (3, 4, 2));
}

#[test]
fn una_palabra_corta_no_se_ancla_dentro_de_otra() {
    let p = hoja(&["el cuaderno de notas"]);
    // «de» es su propia palabra, no el trozo de «cuaderno».
    let a = ancla::anclar(&p, "de", None).unwrap();
    assert_eq!(a.cajas[0][0], p.renglones[0].palabras[2].caja[0]);
    assert!(
        ancla::anclar(&p, "uad", None).is_none(),
        "un trozo de palabra no es un ancla"
    );
}

#[test]
fn con_un_error_de_ocr_se_ancla_por_parecido_y_lo_inventado_no() {
    let mut p = hoja(TEXTO);
    // El OCR leyo «rnodelo» por «modelo» en el ultimo renglon.
    p.renglones[5].palabras[5].texto = "rnodelo".into();
    let a = ancla::anclar(&p, "se usa el modelo elastico lineal", None).unwrap();
    assert!(!a.exacto);
    assert_eq!(a.renglon, 5);
    // Lo que no esta en la hoja no se ancla en ninguna parte.
    assert!(ancla::anclar(&p, "la resistencia del hormigon armado", None).is_none());
}

#[test]
fn con_dos_sitios_iguales_manda_el_renglon_que_dijo_el_modelo() {
    let p = hoja(&[
        "el modelo uno",
        "otra cosa",
        "el modelo dos",
        "el modelo tres",
    ]);
    assert_eq!(ancla::anclar(&p, "el modelo", None).unwrap().renglon, 0);
    assert_eq!(ancla::anclar(&p, "el modelo", Some(3)).unwrap().renglon, 2);
    assert_eq!(ancla::anclar(&p, "el modelo", Some(4)).unwrap().renglon, 3);
}

#[test]
fn en_una_hoja_de_calculo_se_ancla_a_la_celda_o_al_rango() {
    let mut p = hoja(&[]);
    for (k, (etq, t)) in [
        ("A1", "Precio"),
        ("B1", "120"),
        ("A2", "Cantidad"),
        ("B2", "3"),
        ("B3", "360"),
    ]
    .iter()
    .enumerate()
    {
        let (f, c) = ancla::celda(etq).unwrap();
        let caja = [
            100.0 + c as f32 * 80.0,
            100.0 + f as f32 * 20.0,
            180.0 + c as f32 * 80.0,
            120.0 + f as f32 * 20.0,
        ];
        p.renglones.push(Renglon {
            palabras: vec![Palabra::nueva(t, caja)],
            etiqueta: Some(etq.to_string()),
        });
        assert_eq!(p.renglones.len(), k + 1);
    }
    assert_eq!(ancla::celda("B3"), Some((2, 1)));
    assert_eq!(ancla::celda("AA10"), Some((9, 26)));
    assert_eq!(ancla::celda("3B"), None);
    let una = ancla::anclar(&p, "B3", None).unwrap();
    assert_eq!(una.union(), [180.0, 140.0, 260.0, 160.0]);
    let rango = ancla::anclar(&p, "B1:B3", None).unwrap();
    assert_eq!(rango.union(), [180.0, 100.0, 260.0, 160.0]);
    // Un rango sin nada escrito dentro no es ancla... pero el texto si.
    assert!(ancla::anclar(&p, "D10:E12", None).is_none());
    assert_eq!(ancla::anclar(&p, "Cantidad", None).unwrap().renglon, 2);
}

// ---------------------------------------------------------------------------
// Colocar

#[test]
fn las_notas_no_se_solapan_y_cambian_de_lado_cuando_un_margen_se_llena() {
    let franjas = vec![
        Franja {
            x0: 20.0,
            x1: 280.0,
            y0: 0.0,
            y1: 600.0,
            lado: Lado::Izquierda,
            fuera: false,
        },
        Franja {
            x0: 1120.0,
            x1: 1380.0,
            y0: 0.0,
            y1: 600.0,
            lado: Lado::Derecha,
            fuera: false,
        },
    ];
    let mut c = Colocador::nuevo(franjas, 10.0);
    let mut puestas = Vec::new();
    // Diez notas de 110 de alto senalando todas el mismo renglon, a la
    // derecha del texto: no caben diez en una franja de 600.
    for _ in 0..10 {
        if let Some(p) = c.colocar([900.0, 300.0, 1000.0, 326.0], &|_| 110.0, 400.0) {
            puestas.push(p);
        }
    }
    for (i, a) in puestas.iter().enumerate() {
        for b in &puestas[i + 1..] {
            assert!(!solapan(a.caja, b.caja), "{a:?} pisa {b:?}");
        }
        assert!(
            a.caja[1] >= 0.0 && a.caja[3] <= 600.0,
            "fuera de la franja: {a:?}"
        );
    }
    // La primera, a la altura de lo senalado y en su lado.
    assert_eq!(puestas[0].franja.lado, Lado::Derecha);
    assert!((puestas[0].caja[1] - 295.0).abs() < 1.0, "{:?}", puestas[0]);
    // Lleno el derecho, se pasa al izquierdo.
    assert!(puestas.iter().any(|p| p.franja.lado == Lado::Izquierda));
    // Y caben las que caben: como mucho 5 por franja (5*110 + 4*10 = 590);
    // llegando de una en una, la primera parte la franja y quedan 4.
    assert!((8..=10).contains(&puestas.len()), "{}", puestas.len());
    assert!(
        c.colocar([900.0, 300.0, 1000.0, 326.0], &|_| 110.0, 400.0)
            .is_none(),
        "sin sitio no se pone encima"
    );
}

#[test]
fn salir_de_la_hoja_solo_si_dentro_no_hay_sitio() {
    let mut p = hoja(TEXTO);
    p.fuera = 1050.0;
    let fs = p.franjas();
    assert_eq!(fs.iter().filter(|f| f.fuera).count(), 2);
    let mut c = Colocador::nuevo(fs, 10.0);
    let primera = c
        .colocar([900.0, 300.0, 1000.0, 326.0], &|_| 100.0, 360.0)
        .unwrap();
    assert!(!primera.franja.fuera, "con margen dentro, dentro");
    // Una hoja sin margenes (texto de borde a borde): fuera, y si no se
    // puede salir, ningun sitio.
    let mut ancha = hoja(&["texto de borde a borde de la hoja entera sin margen ninguno"]);
    for w in &mut ancha.renglones[0].palabras {
        w.caja[0] = 10.0;
        w.caja[2] = 1390.0;
    }
    assert!(ancha.franjas().is_empty());
    ancha.fuera = 1050.0;
    assert!(ancha.franjas().iter().all(|f| f.fuera));
}

// ---------------------------------------------------------------------------
// Lo que llega del modelo

#[test]
fn el_flujo_suelta_cada_objeto_al_cerrarse_aunque_llegue_a_trozos() {
    let mut f = Flujo::nuevo();
    let texto = "```json\n{\"tipo\":\"importante\",\"ancla\":\"la {viga}\",\"nota\":\"dice \\\"hola\\\"\"}\n{\"tipo\":\"paso\",\"ancla\":\"x\",\"n\":2}\n```";
    let mut salen = Vec::new();
    let mut cuando = Vec::new();
    for (i, c) in texto.chars().enumerate() {
        let v = f.empujar(&c.to_string());
        if !v.is_empty() {
            cuando.push(i);
        }
        salen.extend(v);
    }
    assert_eq!(salen.len(), 2);
    assert_eq!(
        salen[0]["ancla"], "la {viga}",
        "las llaves entre comillas no cuentan"
    );
    assert_eq!(salen[0]["nota"], "dice \"hola\"");
    // El primero sale en cuanto se cierra, no al final.
    assert!(cuando[0] < texto.find("paso").unwrap());
    assert_eq!(f.rotos, 0);
    assert!(f.empujar("{\"tipo\": roto}").is_empty());
    assert_eq!(f.rotos, 1);
}

#[test]
fn se_leen_las_anotaciones_y_sus_sinonimos() {
    let a = Leida::de(
        &json!({"tipo":"error","pag":2,"ancla":"12 kN/m","l":"L3","nota":"la unidad","corr":"12 kN/m²"}),
    );
    let Leida::Anotacion(p) = a else {
        panic!("{a:?}")
    };
    assert_eq!(
        (p.clase, p.pagina, p.linea),
        (Clase::Error, Some(1), Some(3))
    );
    assert_eq!(p.correccion.as_deref(), Some("12 kN/m²"));
    assert!(matches!(
        Leida::de(&json!({"tipo":"glosario","ancla":"luz","nota":"luz: distancia"})),
        Leida::Anotacion(Pedida {
            clase: Clase::Definicion,
            ..
        })
    ));
    assert!(matches!(
        Leida::de(&json!({"tipo":"nada","nota":"pagina en blanco"})),
        Leida::Nada(_)
    ));
    assert!(matches!(
        Leida::de(&json!({"tipo":"importante","nota":"sin ancla"})),
        Leida::Mal(_)
    ));
    assert!(matches!(
        Leida::de(&json!({"tipo":"dibujo","ancla":"x","nota":"y"})),
        Leida::Mal(_)
    ));
}

// ---------------------------------------------------------------------------
// De punta a punta

#[test]
fn el_anotador_pone_cada_nota_en_el_margen_sin_pisar_texto_ni_otra_nota() {
    let col = hoja(TEXTO).columna().unwrap();
    let mut a = Anotador::nuevo(vec![hoja(TEXTO)], "t1");
    let pedidas = [
        pedida(
            Clase::Importante,
            "carga repartida de 12 kN/m",
            "Es la carga de diseño: entra en todas las fórmulas del capítulo.",
        ),
        pedida(
            Clase::Explicacion,
            "M = q L^2 / 8",
            "M: momento máximo; q: carga por metro; L: luz entre apoyos.",
        ),
        Pedida {
            numero: Some(1),
            ..pedida(
                Clase::Paso,
                "La viga se apoya en dos columnas",
                "Identifica los apoyos.",
            )
        },
        Pedida {
            correccion: Some("modelo elástico lineal".into()),
            ..pedida(Clase::Error, "modelo elastico lineal", "Falta la tilde.")
        },
        pedida(Clase::Definicion, "vano", "vano: tramo entre dos apoyos."),
        pedida(
            Clase::Pregunta,
            "donde la flecha es mayor",
            "¿Por qué la flecha máxima está en el centro?",
        ),
        pedida(
            Clase::Resumen,
            "",
            "Viga biapoyada con carga repartida: momento máximo en el centro.",
        ),
    ];
    let mut notas = Vec::new();
    for p in &pedidas {
        let puesta = a.poner(p).unwrap_or_else(|e| panic!("{p:?}: {e}"));
        assert!(!puesta.fuera);
        // Todo en el grupo de su llamada y en el del turno.
        assert!(
            puesta
                .elementos
                .iter()
                .all(|e| e.grupos.contains(&"asistente-anotado:t1".to_string()))
        );
        // La nota: el ultimo rectangulo relleno de pastel, en un margen.
        let papel = puesta
            .elementos
            .iter()
            .rev()
            .find(|e| {
                matches!(e.figura, pixpin_motor2d::Figura::Rectangulo)
                    && e.relleno.is_some()
                    && e.opacidad > 0.9
            })
            .unwrap();
        let caja = [
            papel.x,
            papel.y,
            papel.x + papel.ancho,
            papel.y + papel.alto,
        ];
        assert!(
            caja[2] <= col[0] || caja[0] >= col[2],
            "la nota tapa la columna: {caja:?} {col:?}"
        );
        notas.push(caja);
    }
    for (i, x) in notas.iter().enumerate() {
        for y in &notas[i + 1..] {
            assert!(!solapan(*x, *y), "{x:?} pisa {y:?}");
        }
    }
    assert_eq!(a.cuantas(), 7);
    // La misma otra vez no se repite; lo que no esta no se pone.
    assert!(a.poner(&pedidas[0]).is_err());
    assert!(
        a.poner(&pedida(Clase::Importante, "hormigon pretensado", "x"))
            .is_err()
    );
    let r = a.resumen();
    assert!(
        r.starts_with("7 anotaciones") && r.contains("hormigon pretensado"),
        "{r}"
    );
}

#[test]
fn la_linea_guia_no_tapa_ninguna_palabra() {
    let p = hoja(TEXTO);
    let palabras: Vec<[f32; 4]> = p
        .renglones
        .iter()
        .flat_map(|r| r.palabras.iter().map(|w| w.caja))
        .collect();
    let mut a = Anotador::nuevo(vec![p], "t2");
    // Lo senalado esta en medio del renglon: la recta cruzaria el resto.
    let puesta = a
        .poner(&pedida(Clase::Importante, "dos columnas", "Los apoyos."))
        .unwrap();
    let marca = puesta
        .elementos
        .iter()
        .find(|e| matches!(e.figura, pixpin_motor2d::Figura::Rectangulo))
        .unwrap();
    let dentro_de_la_marca = |c: &[f32; 4]| {
        c[0] >= marca.x
            && c[2] <= marca.x + marca.ancho
            && c[1] >= marca.y
            && c[3] <= marca.y + marca.alto
    };
    for e in &puesta.elementos {
        if let pixpin_motor2d::Figura::Linea { puntos } = &e.figura {
            for w in puntos.windows(2) {
                for k in 0..=40 {
                    let t = k as f32 / 40.0;
                    let (x, y) = (
                        w[0].x + (w[1].x - w[0].x) * t,
                        w[0].y + (w[1].y - w[0].y) * t,
                    );
                    for c in palabras.iter().filter(|c| !dentro_de_la_marca(c)) {
                        assert!(
                            !(x > c[0] + 2.0 && x < c[2] - 2.0 && y > c[1] + 2.0 && y < c[3] - 2.0),
                            "la guia pasa por {c:?} en ({x}, {y})"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn una_pagina_vacia_no_da_nada_que_anclar() {
    let vacia = Pagina {
        numero: 0,
        caja: [0.0, 0.0, 1400.0, 1980.0],
        renglones: Vec::new(),
        letra: 20.0,
        fuera: 0.0,
        solo_derecha: false,
    };
    assert!(vacia.vacia());
    assert!(vacia.columna().is_none());
    let mut a = Anotador::nuevo(vec![vacia], "t3");
    assert!(a.poner(&pedida(Clase::Importante, "algo", "nota")).is_err());
    assert!(a.poner(&pedida(Clase::Resumen, "", "nota")).is_err());
    assert!(
        Anotador::nuevo(Vec::new(), "t4")
            .poner(&pedida(Clase::Importante, "x", "y"))
            .is_err()
    );
}

#[test]
fn el_mensaje_numera_los_renglones_y_se_recorta() {
    let p = hoja(TEXTO);
    let m = mensaje("señala lo importante", std::slice::from_ref(&p), MAX_LETRAS);
    assert!(m.contains("=== Pagina 1 ===") && m.contains("L3: una carga repartida"));
    let corto = mensaje("x", &[p], 120);
    assert!(corto.contains("no cabe") && corto.chars().count() < 200);
    assert!(instrucciones(true).contains("B4:D9"));
    assert!(!instrucciones(false).contains("B4:D9"));
}
