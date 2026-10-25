//! Pruebas de la hoja al lienzo con hojas dibujadas aqui mismo: las
//! «palabras» son trazos negros dentro de su caja (lo que ve el analisis de
//! la imagen) y su texto va aparte (lo que daria el OCR). Las hojas con
//! letra de verdad, OCR de Windows y el modelo estan en el banco de la app
//! (`apps/pixpin/src/asistente/hoja_a_lienzo/banco.rs`).

use super::comparar::parecido;
use super::montaje::{self, Entrada, Leido, Via};
use super::seccion::seccionar;
use super::vision::{leer_grafica, leer_latex, leer_tipo};
use super::*;

/// Una hoja en blanco para dibujar encima.
struct Papel {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
    palabras: Vec<PalabraHoja>,
}

impl Papel {
    fn nuevo(w: u32, h: u32) -> Papel {
        Papel {
            w,
            h,
            rgba: vec![255; (w * h * 4) as usize],
            palabras: Vec::new(),
        }
    }
    fn pixel(&mut self, x: i64, y: i64, g: u8) {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        self.rgba[i] = g;
        self.rgba[i + 1] = g;
        self.rgba[i + 2] = g;
        self.rgba[i + 3] = 255;
    }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, g: u8) {
        for yy in y as i64..(y + h) as i64 {
            for xx in x as i64..(x + w) as i64 {
                self.pixel(xx, yy, g);
            }
        }
    }
    /// Una palabra: trazos verticales en su caja (y su texto, para el OCR).
    fn palabra(&mut self, texto: &str, x: f64, y: f64, alto: f64, negrita: bool) -> f64 {
        let w = (texto.chars().count() as f64 * alto * 0.5).max(alto * 0.4);
        let paso = if negrita { 3 } else { 4 };
        let grueso = if negrita { 2 } else { 1 };
        for xx in (x as i64..(x + w) as i64).step_by(paso) {
            for k in 0..grueso {
                for yy in (y + alto * 0.15) as i64..(y + alto * 0.85) as i64 {
                    self.pixel(xx + k, yy, 20);
                }
            }
        }
        self.palabras.push(PalabraHoja::nueva(texto, x, y, w, alto));
        w
    }
    /// Un renglon de palabras desde `x`; devuelve donde acabo.
    fn renglon(&mut self, texto: &str, x: f64, y: f64, alto: f64) -> f64 {
        let mut cx = x;
        for p in texto.split_whitespace() {
            let w = self.palabra(p, cx, y, alto, false);
            cx += w + alto * 0.35;
        }
        cx
    }
    /// Un parrafo de `n` renglones que llenan `ancho`.
    fn parrafo(&mut self, x: f64, y: f64, ancho: f64, n: usize, alto: f64) -> f64 {
        let mut yy = y;
        for i in 0..n {
            let mut cx = x;
            let mut k = 0;
            let fin = if i + 1 == n {
                x + ancho * 0.6
            } else {
                x + ancho
            };
            loop {
                let palabra = [
                    "lorem",
                    "ipsum",
                    "dolor",
                    "sit",
                    "amet",
                    "consectetur",
                    "adipiscing",
                    "elit",
                ][k % 8];
                let w = palabra.chars().count() as f64 * alto * 0.5;
                if cx + w > fin {
                    break;
                }
                let w = self.palabra(palabra, cx, yy, alto, false);
                cx += w + alto * 0.35;
                k += 1;
            }
            yy += alto * 1.5;
        }
        yy
    }
    fn raya_h(&mut self, x0: f64, x1: f64, y: f64, g: f64) {
        self.rect(x0, y, x1 - x0, g, 0);
    }
    fn raya_v(&mut self, x: f64, y0: f64, y1: f64, g: f64) {
        self.rect(x, y0, g, y1 - y0, 0);
    }
    /// Una «foto»: un degradado con ruido (muchos medios tonos).
    fn foto(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let mut s: u32 = 12345;
        for yy in y as i64..(y + h) as i64 {
            for xx in x as i64..(x + w) as i64 {
                s = s.wrapping_mul(1103515245).wrapping_add(12345);
                let ruido = ((s >> 16) % 40) as f64;
                let g = 70.0 + (xx as f64 - x) / w * 90.0 + (yy as f64 - y) / h * 40.0 + ruido;
                self.pixel(xx, yy, g.clamp(0.0, 255.0) as u8);
            }
        }
    }
    fn seccionar(&self) -> Seccionado {
        seccionar(&self.rgba, self.w, self.h, self.palabras.clone())
    }
}

fn tipos(s: &Seccionado) -> Vec<Tipo> {
    s.zonas.iter().map(|z| z.tipo).collect()
}

#[test]
fn una_hoja_en_blanco_no_da_zonas() {
    let p = Papel::nuevo(1000, 1400);
    let s = p.seccionar();
    assert!(s.zonas.is_empty(), "{:?}", tipos(&s));
    assert_eq!(s.columnas, 1);
    // Ni una imagen vacia rompe nada.
    let s = seccionar(&[], 0, 0, Vec::new());
    assert!(s.zonas.is_empty());
}

#[test]
fn una_columna_con_titulo_y_dos_parrafos() {
    let mut p = Papel::nuevo(1000, 1400);
    p.renglon("Informe", 380.0, 80.0, 44.0);
    let y = p.parrafo(100.0, 200.0, 800.0, 5, 20.0);
    p.parrafo(100.0, y + 30.0, 800.0, 4, 20.0);
    let s = p.seccionar();
    assert_eq!(
        tipos(&s),
        vec![Tipo::Encabezado, Tipo::Parrafo, Tipo::Parrafo],
        "{s:#?}"
    );
    assert_eq!(s.zonas[1].renglones.len(), 5);
    assert_eq!(s.zonas[2].renglones.len(), 4);
    assert_eq!(s.columnas, 1);
    assert!(s.zonas.windows(2).all(|w| w[0].orden < w[1].orden));
}

#[test]
fn dos_columnas_con_raya_en_medio_se_leen_por_columnas() {
    let mut p = Papel::nuevo(1200, 1600);
    // Titulo a todo lo ancho.
    p.renglon("Titulo del articulo", 360.0, 60.0, 40.0);
    // Columna izquierda: dos parrafos; derecha: uno, a la misma altura.
    let y = p.parrafo(80.0, 200.0, 480.0, 6, 18.0);
    p.parrafo(80.0, y + 30.0, 480.0, 5, 18.0);
    p.parrafo(640.0, 200.0, 480.0, 10, 18.0);
    p.raya_v(600.0, 190.0, 700.0, 2.0);
    let s = p.seccionar();
    let t = tipos(&s);
    assert_eq!(t.iter().filter(|x| **x == Tipo::Linea).count(), 1, "{t:?}");
    assert_eq!(
        s.columnas,
        2,
        "{:?}",
        s.zonas
            .iter()
            .map(|z| (z.tipo, z.caja.x as i32, z.caja.w as i32, z.columna))
            .collect::<Vec<_>>()
    );
    let textos: Vec<&Zona> = s.zonas.iter().filter(|z| z.tipo != Tipo::Linea).collect();
    assert_eq!(textos[0].tipo, Tipo::Encabezado, "{t:?}");
    // Orden: la izquierda entera antes que la derecha.
    let izq: Vec<usize> = textos
        .iter()
        .filter(|z| z.tipo == Tipo::Parrafo && z.caja.x < 600.0)
        .map(|z| z.orden)
        .collect();
    let der: Vec<usize> = textos
        .iter()
        .filter(|z| z.tipo == Tipo::Parrafo && z.caja.x > 600.0)
        .map(|z| z.orden)
        .collect();
    assert_eq!(izq.len(), 2, "{t:?}");
    assert_eq!(der.len(), 1, "{t:?}");
    assert!(
        izq.iter().all(|i| der.iter().all(|d| i < d)),
        "{izq:?} {der:?}"
    );
    // Ningun renglon cruza la raya.
    assert!(
        s.zonas
            .iter()
            .filter(|z| z.tipo == Tipo::Parrafo)
            .flat_map(|z| &z.renglones)
            .all(|r| r.caja.x1() < 600.0 || r.caja.x > 600.0)
    );
    assert!(
        s.zonas
            .iter()
            .filter(|z| z.tipo == Tipo::Parrafo)
            .all(|z| z.columna == if z.caja.x < 600.0 { 0 } else { 1 }),
        "{:?}",
        s.zonas
            .iter()
            .map(|z| (z.tipo, z.caja.x as i32, z.columna))
            .collect::<Vec<_>>()
    );
}

#[test]
fn dos_columnas_sin_raya_tambien() {
    let mut p = Papel::nuevo(1200, 1400);
    p.parrafo(80.0, 100.0, 480.0, 8, 18.0);
    p.parrafo(660.0, 100.0, 460.0, 8, 18.0);
    let s = p.seccionar();
    assert_eq!(tipos(&s), vec![Tipo::Parrafo, Tipo::Parrafo], "{s:#?}");
    assert!(s.zonas[0].caja.x < s.zonas[1].caja.x);
    assert_eq!(s.columnas, 2);
}

/// Una tabla de `filas x 3` con rayas o sin ellas.
fn tabla(p: &mut Papel, x: f64, y: f64, filas: usize, rayas: bool) -> Caja {
    let (cw, rh) = (180.0, 40.0);
    let datos = [
        ["Partida", "Cantidad", "Precio"],
        ["Cemento", "12", "450"],
        ["Arena", "3", "120"],
        ["Acero", "40", "2300"],
        ["Ladrillo", "900", "810"],
    ];
    for f in 0..filas {
        #[allow(clippy::needless_range_loop)] // `c` tambien da la x
        for c in 0..3 {
            p.palabra(
                datos[f % datos.len()][c],
                x + c as f64 * cw + 12.0,
                y + f as f64 * rh + 10.0,
                18.0,
                f == 0,
            );
        }
    }
    if rayas {
        for f in 0..=filas {
            p.raya_h(x, x + 3.0 * cw, y + f as f64 * rh, 2.0);
        }
        for c in 0..=3 {
            p.raya_v(x + c as f64 * cw, y, y + filas as f64 * rh, 2.0);
        }
    }
    Caja::nueva(x, y, 3.0 * cw, filas as f64 * rh)
}

#[test]
fn una_tabla_rayada_sale_con_sus_celdas() {
    let mut p = Papel::nuevo(1000, 1200);
    let y = p.parrafo(100.0, 80.0, 800.0, 3, 18.0);
    tabla(&mut p, 150.0, y + 40.0, 5, true);
    let s = p.seccionar();
    let t = s
        .zonas
        .iter()
        .find(|z| z.tipo == Tipo::Tabla)
        .unwrap_or_else(|| panic!("{:?}", tipos(&s)));
    let leida = t.tabla.as_ref().unwrap();
    assert_eq!(leida.filas.len(), 5, "{leida:?}");
    assert_eq!(leida.columnas(), 3, "{leida:?}");
    assert_eq!(leida.filas[1], vec!["Cemento", "12", "450"]);
    assert!(!t.rayas.is_empty());
    // Las rayas de la tabla no salen tambien como lineas sueltas.
    assert_eq!(s.cuantas(Tipo::Linea), 0, "{:?}", tipos(&s));
    assert_eq!(s.zonas[0].tipo, Tipo::Parrafo);
}

#[test]
fn una_tabla_sin_rayas_se_reconoce_por_sus_columnas() {
    let mut p = Papel::nuevo(1000, 1000);
    tabla(&mut p, 150.0, 200.0, 5, false);
    let s = p.seccionar();
    assert_eq!(tipos(&s), vec![Tipo::Tabla], "{s:#?}");
    let leida = s.zonas[0].tabla.as_ref().unwrap();
    assert_eq!((leida.filas.len(), leida.columnas()), (5, 3));
}

#[test]
fn la_foto_es_foto_y_lo_que_el_ocr_vio_dentro_no_es_texto() {
    let mut p = Papel::nuevo(1000, 1400);
    let y = p.parrafo(100.0, 80.0, 800.0, 3, 18.0);
    p.foto(250.0, y + 40.0, 500.0, 350.0);
    // El OCR «lee» algo dentro de la foto.
    p.palabras
        .push(PalabraHoja::nueva("xq", 400.0, y + 140.0, 30.0, 18.0));
    p.parrafo(100.0, y + 430.0, 800.0, 3, 18.0);
    let s = p.seccionar();
    assert_eq!(
        tipos(&s),
        vec![Tipo::Parrafo, Tipo::Foto, Tipo::Parrafo],
        "{s:#?}"
    );
    let f = &s.zonas[1];
    assert!(
        (f.caja.x - 250.0).abs() < 25.0 && (f.caja.w - 500.0).abs() < 50.0,
        "{:?}",
        f.caja
    );
    assert!(
        !s.zonas
            .iter()
            .flat_map(|z| &z.renglones)
            .any(|r| r.texto.contains("xq"))
    );
}

#[test]
fn una_foto_sin_texto_es_una_sola_foto() {
    let mut p = Papel::nuevo(800, 600);
    p.foto(0.0, 0.0, 800.0, 600.0);
    let s = p.seccionar();
    assert_eq!(tipos(&s), vec![Tipo::Foto], "{s:#?}");
}

#[test]
fn la_formula_centrada_es_formula() {
    let mut p = Papel::nuevo(1000, 1000);
    let y = p.parrafo(100.0, 80.0, 800.0, 3, 18.0);
    p.renglon("E = m c ^ 2", 400.0, y + 30.0, 22.0);
    p.parrafo(100.0, y + 100.0, 800.0, 3, 18.0);
    let s = p.seccionar();
    assert_eq!(
        tipos(&s),
        vec![Tipo::Parrafo, Tipo::Formula, Tipo::Parrafo],
        "{s:#?}"
    );
    // Una frase con un «=» suelto sigue siendo texto.
    let r = Caja::nueva(100.0, 0.0, 800.0, 20.0);
    assert!(
        seccion::puntuar_formula(
            "La velocidad media es v = d / t en metros por segundo",
            &r,
            &r
        ) < 0.55
    );
    assert!(
        seccion::puntuar_formula(
            "x = \u{221a}(b² − 4ac)",
            &Caja::nueva(400.0, 0.0, 200.0, 20.0),
            &r
        ) >= 0.55
    );
}

#[test]
fn una_fraccion_con_su_raya_va_junta() {
    let mut p = Papel::nuevo(1000, 800);
    let y = p.parrafo(100.0, 80.0, 800.0, 3, 18.0);
    // a + b / 2c con la raya entre medias.
    p.renglon("a + b", 460.0, y + 40.0, 20.0);
    p.raya_h(440.0, 560.0, y + 66.0, 2.0);
    p.renglon("2c", 480.0, y + 74.0, 20.0);
    p.parrafo(100.0, y + 140.0, 800.0, 2, 18.0);
    let s = p.seccionar();
    let f: Vec<&Zona> = s.zonas.iter().filter(|z| z.tipo == Tipo::Formula).collect();
    assert_eq!(f.len(), 1, "{:?}", tipos(&s));
    assert_eq!(f[0].renglones.len(), 2, "{:?}", f[0].renglones);
}

#[test]
fn seccionar_es_rapido() {
    let mut p = Papel::nuevo(1700, 2200);
    p.renglon("Titulo", 700.0, 80.0, 50.0);
    let y = p.parrafo(120.0, 220.0, 680.0, 30, 22.0);
    p.parrafo(900.0, 220.0, 680.0, 30, 22.0);
    p.raya_v(850.0, 210.0, y, 3.0);
    tabla(&mut p, 300.0, y + 60.0, 5, true);
    p.foto(600.0, y + 360.0, 600.0, 400.0);
    let t = std::time::Instant::now();
    let s = p.seccionar();
    let ms = t.elapsed().as_millis();
    assert!(s.zonas.len() >= 5);
    // En debug y con otros compilando al lado: holgado.
    assert!(ms < 2500, "{ms} ms");
}

// ---------------------------------------------------------------------------
// Montaje

fn entrada<'a>(z: &'a Zona, pagina: (&'a [u8], u32, u32), o: &'a Opciones) -> Entrada<'a> {
    Entrada {
        zona: z,
        mapa: Mapa {
            escala: 0.5,
            origen: (0.0, 0.0),
        },
        opciones: o,
        pagina,
        alto_letra: 18.0,
        semilla: 77,
        lector: None,
        ajenas: &[],
        pistas: &[],
    }
}

fn leer(els: &[serde_json::Value]) -> Vec<pixpin_motor2d::Elemento> {
    pixpin_motor2d::excalidraw::leer(&crate::lienzo::documento(els.to_vec()))
        .unwrap()
        .elementos()
}

#[test]
fn cada_zona_sale_en_su_sitio_y_el_lienzo_la_lee() {
    let mut p = Papel::nuevo(1000, 1400);
    p.renglon("Informe", 380.0, 80.0, 44.0);
    let y = p.parrafo(100.0, 200.0, 800.0, 4, 20.0);
    tabla(&mut p, 150.0, y + 40.0, 4, true);
    p.foto(250.0, y + 260.0, 400.0, 300.0);
    let s = p.seccionar();
    let o = Opciones::default();
    for z in &s.zonas {
        let e = entrada(z, (&p.rgba, p.w, p.h), &o);
        let h = montaje::hacer(&e, None, Via::Editable);
        assert!(!h.elementos.is_empty(), "{:?} sin elementos", z.tipo);
        let els = leer(&h.elementos);
        assert!(!els.is_empty(), "{:?}", z.tipo);
        // Dentro de su caja destino (con holgura: la letra no es la del papel).
        let d = e.destino().crecer(60.0);
        let caja = els
            .iter()
            .map(|e| e.caja())
            .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
            .unwrap();
        assert!(
            d.contiene(caja.0 as f64, caja.1 as f64) && d.contiene(caja.2 as f64, caja.3 as f64),
            "{:?}: {caja:?} fuera de {d:?}",
            z.tipo
        );
        // Todo en un grupo.
        assert!(els.iter().all(|e| !e.grupos.is_empty()), "{:?}", z.tipo);
        match z.tipo {
            Tipo::Foto => {
                assert_eq!(h.imagenes.len(), 1);
                assert!(matches!(els[0].figura, pixpin_motor2d::Figura::Imagen { id_objeto } if id_objeto == h.imagenes[0].id));
            }
            Tipo::Parrafo => {
                let t: String = els.iter().filter_map(|e| match &e.figura {
                    pixpin_motor2d::Figura::Texto { texto, .. } => Some(texto.clone()),
                    _ => None,
                }).collect();
                assert!(t.contains("lorem"), "{t}");
            }
            Tipo::Encabezado => assert!(els.iter().any(|e| matches!(&e.figura, pixpin_motor2d::Figura::Texto { texto, tam, .. } if texto == "Informe" && *tam > 15.0)), "{els:?}"),
            _ => {}
        }
    }
}

/// Banco del 15-oct: con la misma semilla en todas las zonas los ids se
/// repetian y rehacer una se llevaba piezas de otras. Ahora ninguno se
/// repite en toda la hoja.
#[test]
fn los_ids_no_se_repiten_entre_zonas() {
    let mut p = Papel::nuevo(1000, 1400);
    p.renglon("Informe", 380.0, 80.0, 44.0);
    let y = p.parrafo(100.0, 200.0, 800.0, 4, 20.0);
    tabla(&mut p, 150.0, y + 40.0, 4, true);
    p.foto(250.0, y + 260.0, 400.0, 300.0);
    let s = p.seccionar();
    assert!(s.zonas.len() >= 3);
    let o = Opciones::default();
    let mut vistos = std::collections::HashSet::new();
    for z in &s.zonas {
        let h = montaje::hacer(&entrada(z, (&p.rgba, p.w, p.h), &o), None, Via::Editable);
        for v in &h.elementos {
            let id = v["id"].as_str().unwrap().to_string();
            assert!(vistos.insert(id.clone()), "{id} repetido ({:?})", z.tipo);
        }
    }
    // Y la misma zona rehecha da los mismos ids (es la misma pieza).
    let z = &s.zonas[0];
    let a = montaje::hacer(&entrada(z, (&p.rgba, p.w, p.h), &o), None, Via::Editable);
    let b = montaje::hacer(&entrada(z, (&p.rgba, p.w, p.h), &o), None, Via::Fiel);
    assert_eq!(a.elementos[0]["id"], b.elementos[0]["id"]);
}

#[test]
fn una_formula_leida_sale_compuesta_y_sin_leer_como_imagen() {
    let mut p = Papel::nuevo(800, 400);
    p.renglon("x = a + b", 300.0, 150.0, 24.0);
    let s = p.seccionar();
    let z = s
        .zonas
        .iter()
        .find(|z| z.tipo == Tipo::Formula)
        .expect("formula");
    let o = Opciones::default();
    let e = entrada(z, (&p.rgba, p.w, p.h), &o);
    let h = montaje::hacer(
        &e,
        Some(&Leido::Latex(vec!["x = \\frac{a+b}{2}".into()])),
        Via::Editable,
    );
    let els = leer(&h.elementos);
    assert!(
        els.iter()
            .any(|e| matches!(e.figura, pixpin_motor2d::Figura::Linea { .. })),
        "la raya de la fraccion"
    );
    assert!(h.imagenes.is_empty());
    let sin = montaje::hacer(&e, None, Via::Editable);
    assert_eq!(
        sin.imagenes.len(),
        1,
        "sin LaTeX, la formula va como imagen"
    );
    // Un LaTeX roto no rompe: imagen.
    let roto = montaje::hacer(
        &e,
        Some(&Leido::Latex(vec!["\\frac{a".into()])),
        Via::Editable,
    );
    assert_eq!(roto.imagenes.len(), 1);
}

#[test]
fn una_dudosa_toma_el_tipo_que_dice_el_modelo() {
    let mut p = Papel::nuevo(600, 600);
    p.rect(100.0, 100.0, 300.0, 3.0, 0);
    p.rect(100.0, 100.0, 3.0, 300.0, 0);
    let z = Zona {
        id: 3,
        tipo: Tipo::Dudosa,
        caja: Caja::nueva(90.0, 90.0, 330.0, 330.0),
        orden: 0,
        columna: 0,
        confianza: 0.3,
        renglones: Vec::new(),
        alineacion: Alineacion::Izquierda,
        tabla: None,
        rayas: Vec::new(),
        dentro: Vec::new(),
        panel: None,
    };
    let o = Opciones::default();
    let e = entrada(&z, (&p.rgba, p.w, p.h), &o);
    // (Un dibujo de rayas sale con figuras exactas: diagrama.)
    assert!(matches!(
        montaje::hacer(&e, Some(&Leido::Tipo(Tipo::Dibujo)), Via::Editable).tipo,
        Tipo::Dibujo | Tipo::Diagrama
    ));
    assert_eq!(
        montaje::hacer(&e, None, Via::Editable).imagenes.len(),
        1,
        "sin modelo: imagen"
    );
    // Un dibujo por el que la app pregunto y que es una formula (el sistema
    // con llave escaneado, banco del 15-oct): formula, no calco.
    let mut d = z.clone();
    d.tipo = Tipo::Dibujo;
    let e = entrada(&d, (&p.rgba, p.w, p.h), &o);
    let latex = Leido::Latex(vec![
        r"\begin{cases} x + y = 1 \\ x - y = 0 \end{cases}".into(),
    ]);
    assert_eq!(
        montaje::hacer(&e, Some(&latex), Via::Editable).tipo,
        Tipo::Formula
    );
    assert!(matches!(
        montaje::hacer(&e, Some(&Leido::Tipo(Tipo::Dibujo)), Via::Editable).tipo,
        Tipo::Dibujo | Tipo::Diagrama
    ));
}

#[test]
fn el_calco_cae_en_su_caja() {
    let mut p = Papel::nuevo(400, 400);
    p.rect(100.0, 100.0, 200.0, 200.0, 30);
    let c = crate::calcar::calcar(&p.rgba, p.w, p.h, &Default::default());
    let destino = Caja::nueva(1000.0, 500.0, 200.0, 200.0);
    let els = montaje::calco_en_caja(&c, destino, 0.5, 1);
    let caja = leer(&els)
        .iter()
        .map(|e| e.caja())
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
        .unwrap();
    // El cuadrado de 200 px de la hoja a escala 0.5: 100 px desde (1050, 550).
    assert!(
        (caja.0 - 1050.0).abs() < 8.0 && (caja.1 - 550.0).abs() < 8.0,
        "{caja:?}"
    );
    assert!((caja.2 - 1150.0).abs() < 8.0, "{caja:?}");
}

#[test]
fn el_id_de_imagen_es_valido_y_distinto_por_zona() {
    let a = montaje::id_de_imagen(5, 1);
    let b = montaje::id_de_imagen(5, 2);
    assert_ne!(a, b);
    assert!(a > 1 && a < (1 << 48));
    assert_eq!(
        pixpin_motor2d::enlace::id_del_fichero(&pixpin_motor2d::enlace::id_de_texto(a)),
        a
    );
}

#[test]
fn reducir_guarda_la_proporcion() {
    let img = vec![128u8; 400 * 200 * 4];
    let (r, w, h) = montaje::reducir(&img, 400, 200, 100);
    assert_eq!((w, h), (100, 50));
    assert_eq!(r.len(), 100 * 50 * 4);
    assert!(r.iter().all(|&v| v == 128));
    let (_, w, h) = montaje::reducir(&img, 400, 200, 1000);
    assert_eq!((w, h), (400, 200), "no se agranda");
}

// ---------------------------------------------------------------------------
// Vision, comparar, solo la nube

#[test]
fn el_latex_se_lee_con_manga_ancha() {
    assert_eq!(leer_latex("$x^2$").unwrap(), vec!["x^2"]);
    assert_eq!(
        leer_latex("```latex\n\\frac{a}{b}\n```").unwrap(),
        vec!["\\frac{a}{b}"]
    );
    assert_eq!(
        leer_latex("\\[ a = b \\\\ c = d \\]").unwrap(),
        vec!["a = b", "c = d"]
    );
    assert_eq!(
        leer_latex("Aqui esta la formula: E = mc^2").unwrap(),
        vec!["E = mc^2"]
    );
    assert!(leer_latex("NADA").is_none());
    assert!(leer_latex("   ").is_none());
    assert_eq!(leer_tipo("Fórmula."), Some(Tipo::Formula));
    assert_eq!(leer_tipo("foto"), Some(Tipo::Foto));
    assert_eq!(leer_tipo("no se"), None);
    assert_eq!(
        super::vision::leer_dudosa("FORMULA: a^2 + b^2"),
        Some(Leido::Latex(vec!["a^2 + b^2".into()]))
    );
    assert_eq!(
        super::vision::leer_dudosa("Fórmula"),
        Some(Leido::Tipo(Tipo::Formula))
    );
    assert_eq!(
        super::vision::leer_dudosa("dibujo"),
        Some(Leido::Tipo(Tipo::Dibujo))
    );
    assert_eq!(super::vision::leer_dudosa("no lo se"), None);
    assert!(leer_grafica("NO").is_none());
    assert!(leer_grafica("{\"funciones\":[]}").is_none());
    let g = leer_grafica("```json\n{\"funciones\":[\"x^2\"],\"x_desde\":-2,\"x_hasta\":2}\n```")
        .unwrap();
    assert_eq!(g["funciones"][0], "x^2");
}

#[test]
fn el_parecido_distingue() {
    let mut a = Papel::nuevo(300, 200);
    a.parrafo(10.0, 10.0, 280.0, 5, 16.0);
    let b = a.rgba.clone();
    assert!(parecido((&a.rgba, 300, 200), (&b, 300, 200)) > 0.95);
    // La misma tinta a otro tamano sigue pareciendose.
    let (r, w, h) = montaje::reducir(&b, 300, 200, 150);
    assert!(parecido((&a.rgba, 300, 200), (&r, w, h)) > 0.8);
    // En blanco contra texto: nada.
    let blanco = vec![255u8; 300 * 200 * 4];
    assert!(parecido((&a.rgba, 300, 200), (&blanco, 300, 200)) < 0.1);
    assert_eq!(parecido((&blanco, 300, 200), (&blanco, 300, 200)), 1.0);
    // Los renglones en otro sitio: bastante menos.
    let mut c = Papel::nuevo(300, 200);
    c.parrafo(10.0, 120.0, 280.0, 2, 16.0);
    assert!(parecido((&a.rgba, 300, 200), (&c.rgba, 300, 200)) < 0.6);
}

#[test]
fn pasar_una_hoja_se_reconoce_y_con_el_local_se_contesta_que_no() {
    assert!(pide_hoja_a_lienzo("pasa esta hoja al lienzo"));
    assert!(pide_hoja_a_lienzo("Digitaliza esta foto"));
    assert!(pide_hoja_a_lienzo(
        "pon esta página en el lienzo, por favor"
    ));
    assert!(!pide_hoja_a_lienzo("dibuja un circulo en el lienzo"));
    assert!(!pide_hoja_a_lienzo("pasa esta tabla a Excel"));
    let mut h = vec![Mensaje::usuario("pasa esta hoja al lienzo")];
    let mut dichos = Vec::new();
    let f = solo_nube(&mut h, &mut |e| {
        dichos.push(e);
        crate::motor::Seguir::Si
    })
    .unwrap();
    assert_eq!(f.texto, SOLO_NUBE);
    assert_eq!(f.herramientas_hechas, 0);
    assert_eq!(h.last().unwrap().texto_entero(), SOLO_NUBE);
    assert!(matches!(&dichos[0], crate::agente::Evento::Texto(t) if t == SOLO_NUBE));
    let mut otro = vec![Mensaje::usuario("hola")];
    assert!(solo_nube(&mut otro, &mut |_| crate::motor::Seguir::Si).is_none());
}

#[test]
fn el_mapa_lleva_la_hoja_al_ancho_pedido() {
    let s = Seccionado {
        ancho: 1700,
        alto: 2200,
        ..Default::default()
    };
    let m = Mapa::para(&s, &Opciones::default());
    let (w, h) = m.hoja(&s);
    assert!((w - 1000.0).abs() < 1e-9);
    assert!((h - 2200.0 * 1000.0 / 1700.0).abs() < 1e-6);
    let c = m.caja(&Caja::nueva(170.0, 340.0, 850.0, 17.0));
    assert!((c.x - 100.0).abs() < 1e-9 && (c.w - 500.0).abs() < 1e-9);
    let p = Pedido::de(&serde_json::json!({"pagina": 3, "modo": "fiel", "ancho": 1200}));
    assert_eq!((p.pagina, p.fiel, p.ancho), (Some(2), true, Some(1200.0)));
    let p = Pedido::de(&serde_json::json!({"pagina": 0, "ancho": 5}));
    assert_eq!((p.pagina, p.ancho), (None, None));
}

#[test]
fn los_enlaces_y_los_colores_se_conservan() {
    assert_eq!(
        montaje::enlace_de("Clic en https://mega.nz/folder/abc ya").as_deref(),
        Some("https://mega.nz/folder/abc")
    );
    assert_eq!(
        montaje::enlace_de("visita www.pixpin.pe").as_deref(),
        Some("https://www.pixpin.pe")
    );
    assert!(montaje::enlace_de("un texto sin enlace").is_none());
    // Tinta roja: se mide; negra: nada (es la de siempre).
    let mut p = Papel::nuevo(100, 40);
    for y in 10..30 {
        for x in 10..40 {
            let i = ((y * 100 + x) * 4) as usize;
            p.rgba[i..i + 3].copy_from_slice(&[220, 20, 30]);
        }
        for x in 60..90 {
            let i = ((y * 100 + x) * 4) as usize;
            p.rgba[i..i + 3].copy_from_slice(&[25, 25, 25]);
        }
    }
    let rojo =
        imagen::color_de_tinta(&p.rgba, 100, 40, &Caja::nueva(5.0, 5.0, 40.0, 30.0), 158).unwrap();
    assert!(rojo[0] > 200 && rojo[1] < 60, "{rojo:?}");
    assert!(
        imagen::color_de_tinta(&p.rgba, 100, 40, &Caja::nueva(55.0, 5.0, 40.0, 30.0), 158)
            .is_none()
    );
    assert_eq!(imagen::hex([255, 0, 16]), "#ff0010");
}

// ---------------------------------------------------------------------------
// Lo que no se dibuja, recortado (15-oct)

impl Papel {
    /// Una circunferencia con un diametro (un diagrama sencillo), a mano
    /// pixel a pixel con algo de antialias.
    fn diagrama(&mut self, cx: f64, cy: f64, r: f64) {
        for y in (cy - r - 4.0) as i64..(cy + r + 4.0) as i64 {
            for x in (cx - r - 4.0) as i64..(cx + r + 4.0) as i64 {
                let d = ((x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2)).sqrt();
                let en_arco = (d - r).abs();
                let en_diametro = if (x as f64 - cx).abs() <= r {
                    (y as f64 + 0.5 - cy).abs()
                } else {
                    99.0
                };
                // Y un triangulo inscrito (de los extremos del diametro a lo alto).
                let tope = (cx, cy - r);
                let lado = |a: (f64, f64), b: (f64, f64)| {
                    crate::diagrama::ajuste::dist_a_segmento((x as f64 + 0.5, y as f64 + 0.5), a, b)
                };
                let en_diametro = en_diametro
                    .min(lado((cx - r, cy), tope))
                    .min(lado(tope, (cx + r, cy)));
                let t = en_arco.min(en_diametro);
                if t < 1.6 {
                    let g = (20.0 + 200.0 * (t / 1.6)).min(255.0) as u8;
                    self.pixel(x, y, g);
                }
            }
        }
    }
}

/// **La clasificacion**: una foto (textura con muchos tonos) no se dibuja,
/// va recortada de la hoja con el recorte del lienzo (`crop`), en su sitio y
/// a su escala. Caso negativo: un diagrama sencillo NO se recorta, sale con
/// figuras exactas.
#[test]
fn lo_que_no_se_dibuja_va_recortado_y_un_diagrama_no() {
    let o = Opciones::default();
    // La foto.
    let mut p = Papel::nuevo(900, 700);
    p.foto(120.0, 100.0, 420.0, 300.0);
    let s = p.seccionar();
    let z = s
        .zonas
        .iter()
        .find(|z| matches!(z.tipo, Tipo::Foto | Tipo::Dibujo))
        .expect("la zona de la foto");
    let e = entrada(z, (&p.rgba, p.w, p.h), &o);
    let h = montaje::hacer(&e, None, Via::Editable);
    assert_eq!(h.imagenes.len(), 1, "{}", h.nota);
    let v = &h.elementos[0];
    assert_eq!(v["type"], "image");
    let crop = &v["crop"];
    assert!(crop.is_object(), "con el recorte del lienzo: {v}");
    // El recorte es la zona (con 2 px de aire) sobre la hoja entera.
    let img = &h.imagenes[0];
    let k = img.ancho as f64 / p.w as f64;
    assert!(
        (crop["x"].as_f64().unwrap() / k - (z.caja.x - 2.0)).abs() < 1.5,
        "{crop}"
    );
    assert!(
        (crop["width"].as_f64().unwrap() / k - (z.caja.w + 4.0)).abs() < 2.5,
        "{crop}"
    );
    assert_eq!(crop["naturalWidth"].as_u64().unwrap(), img.ancho as u64);
    // Y en su sitio y a su escala (la del mapa).
    let d = e.mapa.caja(&z.caja.crecer(2.0));
    assert!(
        (v["x"].as_f64().unwrap() - d.x).abs() < 0.5
            && (v["width"].as_f64().unwrap() - d.w).abs() < 0.5,
        "{v}"
    );
    let els = leer(&h.elementos);
    assert!(matches!(
        &els[0].figura,
        pixpin_motor2d::Figura::Imagen { .. }
    ));
    assert!(els[0].extras.recorte.is_some(), "el lienzo lee el recorte");

    // Un diagrama sencillo: figuras exactas, sin recorte.
    let mut p = Papel::nuevo(900, 700);
    p.diagrama(450.0, 380.0, 200.0);
    let s = p.seccionar();
    let z = s
        .zonas
        .iter()
        .filter(|z| matches!(z.tipo, Tipo::Dibujo | Tipo::Grafica | Tipo::Foto))
        .max_by(|a, b| a.caja.area().total_cmp(&b.caja.area()))
        .expect("la zona del dibujo");
    let h = montaje::hacer(&entrada(z, (&p.rgba, p.w, p.h), &o), None, Via::Editable);
    {
        let (rgba, w, h) = super::imagen::recortar(&p.rgba, p.w, p.h, &z.caja.crecer(12.0));
        let d = crate::diagrama::vectorizar(
            &rgba,
            w,
            h,
            &[],
            &crate::diagrama::Opciones {
                alto_letra: 18.0,
                ..Default::default()
            },
            None,
        );
        eprintln!("zona {:?} {:?}: {}", z.tipo, z.caja, d.resumen());
    }
    assert_eq!(h.tipo, Tipo::Diagrama, "{}", h.nota);
    assert!(
        h.imagenes.is_empty(),
        "un diagrama no se recorta: {}",
        h.nota
    );
}

/// 24-oct-2026, el OCR de Windows de la CI: «m = Δy/Δx = (480 − 120)/6» con
/// el «m =» y los «=» sin leer. Quedan dos fracciones lado a lado, numerador
/// sobre denominador con su raya en medio, que parecian una tabla sin rayas
/// de 2 × 2 («Ay | 480- 120» sobre «Ax | 6»). Son una formula.
#[test]
fn dos_fracciones_lado_a_lado_no_son_una_tabla() {
    let mut p = Papel::nuevo(1400, 900);
    p.renglon(
        "El grupo salio a las ocho y llego al mirador a mediodia.",
        83.0,
        192.0,
        36.0,
    );
    p.renglon(
        "La pendiente media del camino se calcula asi:",
        83.0,
        246.0,
        36.0,
    );
    // «m =», sin texto del OCR: solo su tinta.
    p.rect(130.0, 368.0, 40.0, 4.0, 20);
    p.rect(185.0, 372.0, 22.0, 3.0, 20);
    p.rect(185.0, 382.0, 22.0, 3.0, 20);
    // Δy / Δx
    p.renglon("Ay", 240.0, 332.0, 40.0);
    p.raya_h(232.0, 300.0, 382.0, 3.0);
    p.renglon("Ax", 240.0, 398.0, 40.0);
    // «=», sin texto del OCR.
    p.rect(318.0, 372.0, 24.0, 3.0, 20);
    p.rect(318.0, 382.0, 24.0, 3.0, 20);
    // (480 − 120) / 6
    p.renglon("480- 120", 380.0, 332.0, 40.0);
    p.raya_h(354.0, 558.0, 382.0, 3.0);
    p.renglon("6", 450.0, 398.0, 40.0);
    let s = p.seccionar();
    assert!(
        !tipos(&s).contains(&Tipo::Tabla),
        "{:?}",
        s.zonas
            .iter()
            .map(|z| (z.tipo, z.caja, z.texto()))
            .collect::<Vec<_>>()
    );
}

/// La misma, como la vio el OCR de la CI: la raya de la primera fraccion
/// fundida con el «m =» y el «=» en una sola mancha ancha y baja.
#[test]
fn la_raya_fundida_con_los_iguales_tambien_es_una_fraccion() {
    let mut p = Papel::nuevo(1400, 900);
    p.renglon(
        "El grupo salio a las ocho y llego al mirador a mediodia.",
        83.0,
        192.0,
        36.0,
    );
    p.renglon(
        "La pendiente media del camino se calcula asi:",
        83.0,
        246.0,
        36.0,
    );
    p.renglon("Ay", 240.0, 332.0, 40.0);
    // «m =», la raya y el «=», todo junto: 196 x 26.
    p.rect(124.0, 374.0, 196.0, 4.0, 20);
    p.rect(124.0, 374.0, 6.0, 26.0, 20);
    p.rect(150.0, 384.0, 170.0, 4.0, 20);
    p.rect(150.0, 394.0, 170.0, 4.0, 20);
    p.renglon("Ax", 240.0, 406.0, 32.0);
    p.renglon("480- 120", 363.0, 332.0, 32.0);
    p.raya_h(354.0, 558.0, 382.0, 4.0);
    p.renglon("6", 445.0, 406.0, 32.0);
    let s = p.seccionar();
    assert!(
        !tipos(&s).contains(&Tipo::Tabla),
        "{:?}",
        s.zonas
            .iter()
            .map(|z| (z.tipo, z.caja, z.texto()))
            .collect::<Vec<_>>()
    );
}
