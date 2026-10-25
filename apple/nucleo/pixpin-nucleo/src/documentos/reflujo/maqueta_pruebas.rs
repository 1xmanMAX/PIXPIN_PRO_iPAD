//! Las pruebas de la maqueta del movil: las de `visor/maqueta_movil.rs` y
//! las de partir renglones de `pixpin_render::lectura` de Windows.

use super::*;
use pixpin_docs::tabla::FilaDeTabla;

/// Una medida de mentira: 8 unidades por letra, renglones de la letra por
/// su interlineado; el minimo es la palabra mas larga.
fn mide(texto: &str, tam: f32, ancho: f32, _t: &[Tramo], l: &Letra) -> Medida {
    let letras = texto.chars().count() as f32 * 8.0;
    let palabra = texto
        .split_whitespace()
        .map(|p| p.chars().count())
        .max()
        .unwrap_or(0) as f32
        * 8.0;
    let renglones = texto
        .split('\n')
        .map(|p| {
            ((p.chars().count() as f32 * 8.0) / ancho.max(1.0))
                .ceil()
                .max(1.0)
        })
        .sum::<f32>();
    Medida {
        ancho: letras.min(ancho),
        alto: renglones * tam * l.interlineado,
        minimo: palabra,
        renglones: Vec::new(),
    }
}

fn p(t: &str) -> Bloque {
    Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano(t)])
}

fn a384() -> lectura::Ajustes {
    lectura::Ajustes {
        columna: 384,
        tipo: 0,
        grosor: 1,
        ..lectura::Ajustes::default()
    }
}

#[test]
fn un_word_empieza_en_el_relleno_de_main_y_los_margenes_de_los_parrafos_se_solapan() {
    let doc = Documento {
        titulo: "no se ve en un Word".into(),
        bloques: vec![p("uno"), p("dos")],
        ..Documento::default()
    };
    let (c, alto) = colocar(&doc, &a384(), 384.0, Hoja::Word, &mide);
    assert_eq!(
        c.len(),
        2,
        "el movil no pone el titulo del fichero en la pagina de un Word"
    );
    // `main{padding-top:18px}` y el `p{margin:.55em}` del primero.
    assert!((c[0].y - (18.0 + 8.8)).abs() < 1e-3, "{}", c[0].y);
    // Entre dos parrafos, un solo margen (el mayor), no la suma.
    assert!((c[1].y - (c[0].y + 24.8 + 8.8)).abs() < 1e-3, "{}", c[1].y);
    // El texto empieza en el relleno de `main` (16 px) y mide la columna menos los dos.
    assert_eq!((c[0].sangria, c[0].ancho), (16.0, 352.0));
    // Abajo: el ultimo margen y el relleno de 48.
    assert!((alto - (c[1].y + 24.8 + 8.8 + 48.0)).abs() < 1e-3, "{alto}");
}

#[test]
fn en_una_columna_ancha_main_se_queda_en_sus_46_em_y_va_centrado() {
    let a = lectura::Ajustes {
        columna: 860,
        ..a384()
    };
    let (x, ancho) = caja_de_texto(Hoja::Word, &a, 860.0);
    // Medido en el Chromium: el texto mide 736 y empieza en 62.
    assert_eq!(ancho, 46.0 * 16.0);
    assert_eq!(x, (860.0 - 736.0 - 32.0) / 2.0 + 16.0);
    // Con la letra mas grande, main (en em) tambien crece.
    let grande = lectura::Ajustes { tamano: 130, ..a };
    assert_eq!(
        caja_de_texto(Hoja::Word, &grande, 860.0),
        (16.0, 860.0 - 32.0)
    );
    // Caso negativo: en una columna estrecha manda la columna.
    assert_eq!(caja_de_texto(Hoja::Word, &a384(), 384.0), (16.0, 352.0));
}

#[test]
fn un_titulo_lleva_su_margen_en_em_de_su_letra_y_su_interlineado_de_titulo() {
    let doc = Documento {
        bloques: vec![
            p("antes"),
            Bloque::nuevo(Clase::Titulo(1), vec![Trozo::llano("Titulo")]),
            p("despues"),
        ],
        ..Documento::default()
    };
    let (c, _) = colocar(&doc, &a384(), 384.0, Hoja::Word, &mide);
    let tam = 16.0 * 1.7;
    assert_eq!(c[1].tam, tam);
    assert_eq!(c[1].letra.peso, 700);
    // El margen de arriba del h1 (1.2 em de 27.2) gana al del parrafo.
    assert!((c[1].y - (c[0].y + 24.8 + tam * 1.2)).abs() < 1e-3);
    assert!((c[1].alto - tam * 1.25).abs() < 1e-3);
    // Debajo, el .5em del h1 (13.6) gana al .55em del parrafo (8.8).
    assert!((c[2].y - (c[1].y + tam * 1.25 + tam * 0.5)).abs() < 1e-3);
}

#[test]
fn un_parrafo_vacio_de_word_ocupa_su_renglon_como_el_nbsp_del_movil() {
    let doc = Documento {
        bloques: vec![p("a"), p(""), p("b")],
        ..Documento::default()
    };
    let (c, _) = colocar(&doc, &a384(), 384.0, Hoja::Word, &mide);
    assert_eq!(c.len(), 3);
    assert!(c[1].texto.is_empty());
    assert!((c[2].y - c[0].y - 2.0 * (24.8 + 8.8)).abs() < 1e-3);
}

#[test]
fn la_letra_y_el_grosor_son_los_cuatro_del_movil() {
    for (tipo, familia) in [
        (0u8, "Noto Serif"),
        (1, "Roboto"),
        (2, "Courier New"),
        (3, "Caveat"),
    ] {
        let a = lectura::Ajustes { tipo, ..a384() };
        assert_eq!(Letra::del_cuerpo(Hoja::Word, &a).familia, familia);
    }
    let a = lectura::Ajustes {
        grosor: 3,
        ..a384()
    };
    assert_eq!(Letra::del_cuerpo(Hoja::Word, &a).peso, 800);
    assert_eq!(Letra::del_cuerpo(Hoja::Libro, &a).interlineado, 1.6);
}

#[test]
fn el_enlace_no_cambia_la_letra_y_el_tabulador_es_un_espacio_eme() {
    let enlace = Trozo {
        texto: "https://x.pe".into(),
        estilo: pixpin_docs::documento::Estilo {
            enlace: true,
            ..Default::default()
        },
    };
    let (t, tramos) = texto_de(&[Trozo::llano("a\tb "), enlace]);
    assert_eq!(t, "a\u{2003}b https://x.pe");
    assert!(
        tramos.is_empty(),
        "el movil solo le cambia el color: {tramos:?}"
    );
}

#[test]
fn el_reparto_de_la_tabla_da_el_maximo_si_cabe_y_el_minimo_si_no_cabe_ni_eso() {
    assert_eq!(
        repartir_columnas(&[20.0, 30.0], &[50.0, 60.0], 300.0),
        vec![50.0, 60.0]
    );
    assert_eq!(
        repartir_columnas(&[200.0, 300.0], &[500.0, 600.0], 300.0),
        vec![200.0, 300.0]
    );
    // Entre medias, lo que sobra en proporcion a lo que falta.
    let r = repartir_columnas(&[20.0, 20.0], &[120.0, 40.0], 100.0);
    assert!((r[0] + r[1] - 100.0).abs() < 1e-3);
    assert!((r[0] - (20.0 + 100.0 * 60.0 / 120.0)).abs() < 1e-3, "{r:?}");
}

fn fila(celdas: Vec<Celda>) -> Bloque {
    let mut b = Bloque::nuevo(Clase::Fila, vec![Trozo::llano("x")]);
    b.fila = Some(FilaDeTabla {
        tabla: 0,
        rejilla: vec![100, 9000],
        pagina: 10_000,
        celdas,
    });
    b
}

#[test]
fn una_tabla_del_word_ignora_la_rejilla_y_cada_parrafo_de_celda_lleva_su_margen() {
    let doc = Documento {
        bloques: vec![
            p("antes"),
            fila(vec![
                Celda::llana(vec![Trozo::llano("N")]),
                Celda::llana(vec![Trozo::llano("uno\n\ndos")]),
            ]),
            p("despues"),
        ],
        ..Documento::default()
    };
    let (c, _) = colocar(&doc, &a384(), 384.0, Hoja::Word, &mide);
    let celdas: Vec<&Colocado> = c.iter().filter(|x| x.clase == Clase::Fila).collect();
    // Tres parrafos en la segunda celda (el vacio en medio cuenta).
    assert_eq!(celdas.len(), 4);
    let k = celdas[0].caja.expect("la primera lleva la caja");
    // La columna mide lo que su texto (1 letra) mas su relleno, no la rejilla de Word.
    assert!((k.ancho - (8.0 + 17.0 + 1.0)).abs() < 1e-3, "{k:?}");
    // Alto de la fila: 3 renglones, 4 margenes de .2em, relleno y raya.
    let contenido = 3.0 * 24.8 + 4.0 * 3.2;
    assert!((k.alto - (contenido + 8.0 + 2.0)).abs() < 1e-3, "{k:?}");
    // El parrafo de despues, tras la tabla y su margen de .8em (la fila con
    // su raya de abajo; la de arriba es media fuera, como en el Chromium).
    let despues = c.last().unwrap();
    assert!(
        (despues.y - (k.y + contenido + 8.0 + 1.0 + 12.8)).abs() < 1e-3,
        "{} {k:?}",
        despues.y
    );
}

#[test]
fn un_libro_lleva_su_cabecera_y_la_raya_de_cada_capitulo() {
    let doc = Documento {
        titulo: "El extranjero".into(),
        autor: "Camus".into(),
        bloques: vec![
            Bloque::nuevo(Clase::Capitulo, vec![]),
            p("Hoy ha muerto mama."),
        ],
        imagenes: Vec::new(),
    };
    let a = lectura::Ajustes {
        tipo: 0,
        ..lectura::Ajustes::default()
    };
    let (c, _) = colocar(&doc, &a, 600.0, Hoja::Libro, &mide);
    assert_eq!(
        c[0].bloque, None,
        "la cabecera no es un bloque del documento"
    );
    assert_eq!(c[0].letra.alineacion, Alineacion::Centro);
    // 18 de relleno y 2 em de margen de la cabecera.
    assert!((c[0].y - (18.0 + 34.0)).abs() < 1e-3);
    let raya = c.iter().find(|x| x.clase == Clase::Capitulo).expect("raya");
    let texto = c.last().unwrap();
    // Tras la raya, su relleno de 2 em sin solaparse con el .7em del parrafo.
    assert!(
        (texto.y - (raya.y + 1.0 + 34.0 + 17.0 * 0.7)).abs() < 1e-3,
        "{} {}",
        texto.y,
        raya.y
    );
}

#[test]
fn las_medidas_de_una_imagen_salen_de_su_cabecera() {
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    png.extend_from_slice(&640u32.to_be_bytes());
    png.extend_from_slice(&480u32.to_be_bytes());
    assert_eq!(medidas_de_imagen(&png), Some((640, 480)));
    let gif = [b"GIF89a".as_slice(), &[0x20, 0x01, 0x10, 0x00]].concat();
    assert_eq!(medidas_de_imagen(&gif), Some((288, 16)));
    let jpg = [
        0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xC0, 0, 11, 8, 0x01, 0xE0, 0x02, 0x80,
    ];
    assert_eq!(medidas_de_imagen(&jpg), Some((640, 480)));
    // Casos negativos: lo que no es una imagen, o esta cortado.
    assert_eq!(medidas_de_imagen(b"hola"), None);
    assert_eq!(medidas_de_imagen(&png[..18]), None);
}

#[test]
fn una_imagen_ancha_se_encoge_al_texto_y_ocupa_su_alto() {
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    png.extend_from_slice(&704u32.to_be_bytes());
    png.extend_from_slice(&352u32.to_be_bytes());
    let doc = Documento {
        bloques: vec![Bloque::nota(MARCA_IMAGEN), p("pie")],
        imagenes: vec![pixpin_docs::documento::Imagen {
            mime: "image/png".into(),
            datos: png,
        }],
        ..Documento::default()
    };
    let (c, _) = colocar(&doc, &a384(), 384.0, Hoja::Word, &mide);
    assert!(c[0].recuadro);
    assert_eq!(c[0].imagen, Some(0));
    assert_eq!((c[0].ancho, c[0].alto), (352.0, 176.0));
    assert!(c[1].y > c[0].y + 176.0 + 8.8);
    // La imagen no se lee en voz alta; el pie si.
    assert_eq!(para_leer(&c), vec![1]);
}

// ----------------------------------------------- partir (pixpin_render::lectura)

fn u(t: &str) -> Vec<u16> {
    t.encode_utf16().collect()
}

/// Diez de ancho cada letra.
fn diez(t: &str) -> (Vec<u16>, Vec<f32>) {
    let v = u(t);
    let n = v.len();
    (v, vec![10.0; n])
}

#[test]
fn se_parte_por_los_espacios_y_el_espacio_del_final_cuelga() {
    let (t, a) = diez("uno dos tres");
    let (r, ancho, minimo) = partir(&t, &a, 75.0);
    assert_eq!(r, vec![(0, 7), (8, 12)], "«uno dos» (70) cabe; «tres» baja");
    assert_eq!(ancho, 70.0);
    assert_eq!(minimo, 40.0, "la palabra mas ancha");
    // A lo ancho de todo, un renglon.
    assert_eq!(partir(&t, &a, 1.0e6).0, vec![(0, 12)]);
}

#[test]
fn como_el_movil_no_se_parte_un_enlace_por_su_barra_ni_una_letra_de_su_grado() {
    assert!(!se_parte_tras(u16::from(b'/'), u16::from(b'd')));
    assert!(!se_parte_tras(u16::from(b'.'), u16::from(b'o')));
    assert!(!se_parte_tras(u16::from(b'N'), 0xB0));
    assert!(
        !se_parte_tras(0xA0, u16::from(b'a')),
        "el espacio duro no parte"
    );
    // Si tras un guion entre letras o cifras, y alrededor de una raya larga.
    assert!(se_parte_tras(u16::from(b'-'), u16::from(b'4')));
    assert!(se_parte_tras(u16::from(b'a'), 0x2014));
    assert!(se_parte_tras(0x2014, u16::from(b'a')));
    assert!(!se_parte_tras(u16::from(b'-'), u16::from(b' ')));
    let (t, a) = diez("https://doi.org/10.1080/0144619005002489");
    let (_, _, minimo) = partir(&t, &a, 100.0);
    assert_eq!(minimo, 400.0, "el enlace entero es el minimo");
}

#[test]
fn una_palabra_que_no_cabe_sola_se_parte_por_donde_llega_y_un_salto_parte_siempre() {
    let (t, a) = diez("ab abcdefgh");
    let (r, _, _) = partir(&t, &a, 50.0);
    assert_eq!(r, vec![(0, 2), (3, 8), (8, 11)]);
    let (t, a) = diez("uno\ndos");
    assert_eq!(partir(&t, &a, 1.0e6).0, vec![(0, 3), (4, 7)]);
    // Caso negativo: un texto vacio es un renglon (el del `&nbsp;`).
    assert_eq!(partir(&[], &[], 100.0).0, vec![(0, 0)]);
}

#[test]
fn el_alto_de_un_parrafo_son_sus_renglones_por_el_interlineado_de_css() {
    let letra = Letra::del_cuerpo(Hoja::Word, &a384());
    let texto = "Valentin Mendez, J. C. (2026). Herramientas de inteligencia artificial";
    let avances = vec![9.0; texto.encode_utf16().count()];
    let m = medir_con_avances(texto, &avances, 16.0, 352.0, &letra);
    assert!(m.renglones.len() >= 2, "{m:?}");
    assert!(
        (m.alto - m.renglones.len() as f32 * 24.8).abs() < 0.01,
        "cada renglon 16 x 1,55"
    );
    assert!(m.ancho <= 352.0 + 0.01);
    // Lo que el medidor no contesto cuenta a media eme, no a cero.
    let corto = medir_con_avances(texto, &[], 16.0, 1.0e6, &letra);
    assert!((corto.ancho - 8.0 * texto.encode_utf16().count() as f32).abs() < 0.01);
}
