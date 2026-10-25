//! Pruebas del diagrama con dibujos sinteticos (con antialias, como los de
//! una captura) y tolerancias en pixeles.

use super::ajuste::dist;
use super::*;

/// Un lienzo RGBA blanco donde se pinta con antialias.
pub(crate) struct Lamina {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 3]>,
}

impl Lamina {
    pub fn nueva(w: usize, h: usize) -> Lamina {
        Lamina { w, h, px: vec![[255.0; 3]; w * h] }
    }
    fn mezclar(&mut self, x: usize, y: usize, c: [u8; 3], a: f64) {
        if a <= 0.0 || x >= self.w || y >= self.h {
            return;
        }
        let a = a.min(1.0) as f32;
        let p = &mut self.px[y * self.w + x];
        for q in 0..3 {
            p[q] = p[q] * (1.0 - a) + c[q] as f32 * a;
        }
    }
    /// Pinta con una cobertura dada por pixel (4x4 de muestras).
    fn pintar(&mut self, caja: (f64, f64, f64, f64), c: [u8; 3], cubre: impl Fn(f64, f64) -> bool) {
        let (x0, y0, x1, y1) = caja;
        for y in (y0.floor().max(0.0) as usize)..((y1.ceil() as usize).min(self.h)) {
            for x in (x0.floor().max(0.0) as usize)..((x1.ceil() as usize).min(self.w)) {
                let mut n = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        if cubre(x as f64 + (sx as f64 + 0.5) / 4.0, y as f64 + (sy as f64 + 0.5) / 4.0) {
                            n += 1;
                        }
                    }
                }
                self.mezclar(x, y, c, n as f64 / 16.0);
            }
        }
    }
    pub fn raya(&mut self, a: (f64, f64), b: (f64, f64), g: f64, c: [u8; 3]) {
        let m = g + 2.0;
        let caja = (a.0.min(b.0) - m, a.1.min(b.1) - m, a.0.max(b.0) + m, a.1.max(b.1) + m);
        self.pintar(caja, c, |x, y| ajuste::dist_a_segmento((x, y), a, b) <= g / 2.0);
    }
    pub fn discontinua(&mut self, a: (f64, f64), b: (f64, f64), g: f64, c: [u8; 3], trazo: f64, hueco: f64) {
        let l = dist(a, b);
        let d = ((b.0 - a.0) / l, (b.1 - a.1) / l);
        let mut t = 0.0;
        while t < l {
            let t1 = (t + trazo).min(l);
            self.raya((a.0 + d.0 * t, a.1 + d.1 * t), (a.0 + d.0 * t1, a.1 + d.1 * t1), g, c);
            t = t1 + hueco;
        }
    }
    /// Un arco de `desde` a `hasta` (radianes de pantalla, en sentido del reloj).
    pub fn arco(&mut self, c0: (f64, f64), r: f64, desde: f64, hasta: f64, g: f64, c: [u8; 3]) {
        let m = r + g + 2.0;
        self.pintar((c0.0 - m, c0.1 - m, c0.0 + m, c0.1 + m), c, |x, y| {
            if (dist((x, y), c0) - r).abs() > g / 2.0 {
                return false;
            }
            let a = (y - c0.1).atan2(x - c0.0).rem_euclid(std::f64::consts::TAU);
            let (d0, d1) = (desde.rem_euclid(std::f64::consts::TAU), hasta.rem_euclid(std::f64::consts::TAU));
            if d0 <= d1 { a >= d0 && a <= d1 } else { a >= d0 || a <= d1 }
        });
    }
    pub fn circunferencia(&mut self, c0: (f64, f64), r: f64, g: f64, c: [u8; 3]) {
        self.arco(c0, r, 0.0, std::f64::consts::TAU - 1e-9, g, c);
    }
    pub fn disco(&mut self, c0: (f64, f64), r: f64, c: [u8; 3]) {
        self.pintar((c0.0 - r - 1.0, c0.1 - r - 1.0, c0.0 + r + 1.0, c0.1 + r + 1.0), c, |x, y| dist((x, y), c0) <= r);
    }
    pub fn poligono(&mut self, p: &[(f64, f64)], c: [u8; 3]) {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for q in p {
            x0 = x0.min(q.0);
            y0 = y0.min(q.1);
            x1 = x1.max(q.0);
            y1 = y1.max(q.1);
        }
        let pol = p.to_vec();
        self.pintar((x0, y0, x1, y1), c, move |x, y| {
            let mut dentro = false;
            let n = pol.len();
            let mut j = n - 1;
            for i in 0..n {
                let (xi, yi) = pol[i];
                let (xj, yj) = pol[j];
                if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                    dentro = !dentro;
                }
                j = i;
            }
            dentro
        });
    }
    pub fn rgba(&self) -> Vec<u8> {
        self.px.iter().flat_map(|p| [p[0].round() as u8, p[1].round() as u8, p[2].round() as u8, 255]).collect()
    }
    pub fn medir(&self) -> Diagrama {
        vectorizar(&self.rgba(), self.w as u32, self.h as u32, &[], &Opciones::default(), None)
    }
}

const NEGRO: [u8; 3] = [20, 20, 20];
const AZUL: [u8; 3] = [40, 60, 200];
const VERDE: [u8; 3] = [30, 160, 70];
const ROJO: [u8; 3] = [220, 40, 40];

fn cerca(a: (f64, f64), b: (f64, f64), tol: f64) -> bool {
    dist(a, b) <= tol
}

/// El segmento que va de `a` a `b` (en cualquier sentido), con tolerancia.
fn hay_segmento(d: &Diagrama, a: (f64, f64), b: (f64, f64), tol: f64) -> Option<&Segmento> {
    d.segmentos.iter().find(|s| (cerca(s.a, a, tol) && cerca(s.b, b, tol)) || (cerca(s.a, b, tol) && cerca(s.b, a, tol)))
}

#[test]
fn una_raya_y_una_circunferencia() {
    let mut l = Lamina::nueva(420, 320);
    l.raya((40.0, 40.0), (380.0, 90.0), 2.0, NEGRO);
    l.circunferencia((210.0, 190.0), 90.0, 2.0, NEGRO);
    let d = l.medir();
    eprintln!("{}
{:#?}
{:?}", d.resumen(), d.segmentos, d.arcos);
    assert_eq!(d.segmentos.len(), 1, "{:?}", d.segmentos);
    assert!(hay_segmento(&d, (40.0, 40.0), (380.0, 90.0), 2.5).is_some(), "{:?}", d.segmentos);
    let s = &d.segmentos[0];
    assert!((s.grosor - 2.0).abs() <= 1.0, "grosor {}", s.grosor);
    assert_eq!(d.arcos.len(), 1, "{:?}", d.arcos);
    let c = &d.arcos[0];
    assert!(c.entera());
    assert!(cerca(c.c, (210.0, 190.0), 1.0), "centro {:?}", c.c);
    assert!((c.r - 90.0).abs() <= 1.0, "radio {}", c.r);
    assert!(d.es_diagrama(), "explica {}", d.explica);
    assert!(d.ms < 2000, "tardo {} ms", d.ms);
}

#[test]
fn un_arco_de_otro_color_parte_la_circunferencia() {
    let mut l = Lamina::nueva(400, 400);
    l.circunferencia((200.0, 200.0), 120.0, 2.0, NEGRO);
    // Encima, de 0 a 90 grados (abajo a la derecha), un arco verde mas grueso.
    l.arco((200.0, 200.0), 120.0, 0.0, std::f64::consts::FRAC_PI_2, 4.0, VERDE);
    let d = l.medir();
    eprintln!("{}
{:#?}
{:?}", d.resumen(), d.segmentos, d.arcos);
    let verdes: Vec<&Arco> = d.arcos.iter().filter(|a| a.color[1] > a.color[0] + 60).collect();
    assert_eq!(verdes.len(), 1, "{:?}", d.arcos);
    let v = verdes[0];
    let (ini, fin) = (v.inicio.to_degrees(), (v.inicio + v.barrido).to_degrees());
    assert!(ini.rem_euclid(360.0) < 6.0 || ini.rem_euclid(360.0) > 354.0, "inicio {ini}");
    assert!((fin - 90.0).abs() < 6.0, "fin {fin}");
    assert!(v.grosor >= 3.0, "grosor {}", v.grosor);
    let negros: f64 = d.arcos.iter().filter(|a| a.color[1] < 90).map(|a| a.barrido.to_degrees()).sum();
    assert!((negros - 270.0).abs() < 10.0, "lo negro barre {negros}");
    for a in &d.arcos {
        assert!(cerca(a.c, (200.0, 200.0), 1.5) && (a.r - 120.0).abs() < 1.5, "{a:?}");
    }
}

#[test]
fn una_discontinua_es_discontinua_y_una_continua_no() {
    let mut l = Lamina::nueva(400, 200);
    l.discontinua((30.0, 50.0), (370.0, 60.0), 1.5, NEGRO, 9.0, 6.0);
    l.raya((30.0, 150.0), (370.0, 150.0), 1.5, NEGRO);
    let d = l.medir();
    eprintln!("{}
{:#?}
{:?}", d.resumen(), d.segmentos, d.arcos);
    assert_eq!(d.segmentos.len(), 2, "{:?}", d.segmentos);
    let disc = hay_segmento(&d, (30.0, 50.0), (370.0, 60.0), 4.0).expect("la discontinua, entera");
    assert_eq!(disc.estilo, Estilo::Discontinuo);
    let cont = hay_segmento(&d, (30.0, 150.0), (370.0, 150.0), 3.0).expect("la continua");
    assert_eq!(cont.estilo, Estilo::Solido);
}

#[test]
fn el_triangulo_inscrito_acaba_en_sus_puntos() {
    let mut l = Lamina::nueva(400, 400);
    let c = (200.0, 205.0);
    let r = 140.0;
    let ang = [200f64, 320.0, 80.0].map(|g| g.to_radians());
    let v: Vec<(f64, f64)> = ang.iter().map(|a| (c.0 + r * a.cos(), c.1 + r * a.sin())).collect();
    l.circunferencia(c, r, 1.5, NEGRO);
    for i in 0..3 {
        l.raya(v[i], v[(i + 1) % 3], 2.0, AZUL);
    }
    for &p in &v {
        l.disco(p, 4.5, NEGRO);
    }
    l.disco(c, 3.5, NEGRO);
    let d = l.medir();
    eprintln!("{}
{:#?}
{:?}", d.resumen(), d.segmentos, d.arcos);
    assert_eq!(d.puntos.len(), 4, "{:?}", d.puntos);
    for &p in &v {
        assert!(d.puntos.iter().any(|q| cerca(q.c, p, 1.0)), "falta el punto {p:?}: {:?}", d.puntos);
    }
    // Los lados acaban EXACTAMENTE en los puntos detectados.
    for i in 0..3 {
        let s = hay_segmento(&d, v[i], v[(i + 1) % 3], 2.0).unwrap_or_else(|| panic!("falta el lado {i}: {:?}", d.segmentos));
        for e in [s.a, s.b] {
            assert!(d.puntos.iter().any(|q| dist(q.c, e) < 1e-6), "el extremo {e:?} no esta pegado a un punto");
        }
        assert!(s.color[2] > s.color[0] + 80, "el lado es azul: {:?}", s.color);
    }
    let circ = d.arcos.iter().find(|a| a.entera()).expect("la circunferencia entera");
    assert!(cerca(circ.c, c, 1.5) && (circ.r - r).abs() < 1.5, "{circ:?}");
}

#[test]
fn una_flecha_rellena_es_flecha() {
    let mut l = Lamina::nueva(300, 160);
    l.raya((30.0, 80.0), (250.0, 80.0), 1.5, NEGRO);
    l.poligono(&[(270.0, 80.0), (248.0, 71.0), (248.0, 89.0)], NEGRO);
    let d = l.medir();
    eprintln!("{} {:?}", d.resumen(), d.segmentos);
    assert_eq!(d.segmentos.len(), 1);
    let s = &d.segmentos[0];
    assert!(s.flecha_a || s.flecha_b, "{s:?}");
    let punta = if s.flecha_b { s.b } else { s.a };
    assert!(cerca(punta, (270.0, 80.0), 3.0), "la punta {punta:?}");
    assert!(d.regiones.is_empty(), "la punta no es una region: {:?}", d.regiones.len());
}

#[test]
fn un_cuadrante_relleno_es_region_y_un_recuadro_es_recuadro() {
    let mut l = Lamina::nueva(400, 300);
    // Un cuadrante amarillo con borde rojo.
    let o = (60.0, 250.0);
    let mut p = vec![o];
    for k in 0..=30 {
        let a = -std::f64::consts::FRAC_PI_2 * k as f64 / 30.0;
        p.push((o.0 + 180.0 * a.cos(), o.1 + 180.0 * a.sin()));
    }
    l.poligono(&p, [250, 215, 140]);
    l.raya(o, (240.0, 250.0), 2.0, ROJO);
    l.raya(o, (60.0, 70.0), 2.0, ROJO);
    // Un recuadro aparte.
    l.raya((280.0, 30.0), (380.0, 30.0), 2.0, NEGRO);
    l.raya((380.0, 30.0), (380.0, 90.0), 2.0, NEGRO);
    l.raya((380.0, 90.0), (280.0, 90.0), 2.0, NEGRO);
    l.raya((280.0, 90.0), (280.0, 30.0), 2.0, NEGRO);
    let d = l.medir();
    eprintln!("{}
{:#?}
{:?}", d.resumen(), d.segmentos, d.arcos);
    let amarilla = d.regiones.iter().find(|r| r.color[0] > 200 && r.color[2] < 190).expect("la region amarilla");
    let esperada = std::f64::consts::PI * 180.0 * 180.0 / 4.0;
    assert!((amarilla.area - esperada).abs() / esperada < 0.08, "area {} vs {esperada}", amarilla.area);
    assert_eq!(d.recuadros.len(), 1, "{:?}", d.recuadros);
    let r = &d.recuadros[0];
    assert!((r.x0 - 280.0).abs() < 2.5 && (r.y0 - 30.0).abs() < 2.5 && (r.x1 - 380.0).abs() < 2.5 && (r.y1 - 90.0).abs() < 2.5, "{r:?}");
    assert!(hay_segmento(&d, o, (240.0, 250.0), 3.0).is_some(), "{:?}", d.segmentos);
    assert!(hay_segmento(&d, o, (60.0, 70.0), 3.0).is_some(), "{:?}", d.segmentos);
}

/// Una «foto» de paisaje con grano: no es un diagrama (va al calco).
pub(crate) fn foto(w: usize, h: usize) -> Lamina {
    let mut l = Lamina::nueva(w, h);
    let mut s: u32 = 7;
    for y in 0..h {
        for x in 0..w {
            s = s.wrapping_mul(1103515245).wrapping_add(12345);
            let ruido = ((s >> 16) % 40) as f32 - 20.0;
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let colina = 0.6 + 0.12 * (fx * 9.0).sin() + 0.05 * (fx * 23.0).cos();
            let c = if fy > colina { [60.0 + 50.0 * fy, 120.0 + 40.0 * fx, 50.0] } else { [110.0 + 80.0 * fy, 160.0 + 60.0 * fy, 230.0] };
            l.px[y * w + x] = [(c[0] + ruido).clamp(0.0, 255.0), (c[1] + ruido).clamp(0.0, 255.0), (c[2] + ruido).clamp(0.0, 255.0)];
        }
    }
    l
}

#[test]
fn una_foto_no_es_un_diagrama() {
    let d = foto(320, 240).medir();
    eprintln!("{}
{:#?}
{:?}", d.resumen(), d.segmentos, d.arcos);
    assert!(!d.es_diagrama(), "una foto no es diagrama: {}", d.resumen());
}

#[test]
fn las_letras_sueltas_se_leen_con_el_lector() {
    let mut l = Lamina::nueva(300, 200);
    l.raya((40.0, 150.0), (260.0, 150.0), 2.0, NEGRO);
    l.disco((40.0, 150.0), 4.0, NEGRO);
    // Una «A» de tres rayas, suelta, encima del punto.
    l.raya((30.0, 135.0), (37.0, 115.0), 1.6, NEGRO);
    l.raya((37.0, 115.0), (44.0, 135.0), 1.6, NEGRO);
    l.raya((33.0, 127.0), (41.0, 127.0), 1.6, NEGRO);
    static LEIDAS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let leidas = &LEIDAS;
    let lector = |_rgba: &[u8], w: u32, h: u32| {
        leidas.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // La hoja de letras: una sola, la primera celda.
        assert!(w > 0 && h > 0);
        // Lo oscuro de la hoja es la letra: alli se «lee» la A.
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        for y in 0..h {
            for x in 0..w {
                if _rgba[((y * w + x) * 4) as usize] < 128 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        vec![PalabraHoja::nueva("A", x0 as f64, y0 as f64, (x1 - x0) as f64, (y1 - y0) as f64)]
    };
    let d = vectorizar(&l.rgba(), l.w as u32, l.h as u32, &[], &Opciones::default(), Some(&lector));
    eprintln!("{} {:?}", d.resumen(), d.rotulos);
    assert_eq!(leidas.load(std::sync::atomic::Ordering::SeqCst), 1, "una sola lectura para todas las letras");
    let a = d.rotulos.iter().find(|r| r.texto == "A").expect("la A leida");
    assert!(a.caja.contiene(37.0, 125.0), "{:?}", a.caja);
    assert!(d.glifos.is_empty());
    // Y la A no se dibujo como rayas.
    assert_eq!(d.segmentos.len(), 1, "{:?}", d.segmentos);
}

#[test]
fn el_texto_del_ocr_no_se_vectoriza() {
    let mut l = Lamina::nueva(300, 200);
    l.raya((20.0, 160.0), (280.0, 160.0), 2.0, NEGRO);
    // Un «texto» de rayitas donde el OCR dijo «HOLA».
    for k in 0..4 {
        let x = 60.0 + k as f64 * 14.0;
        l.raya((x, 40.0), (x, 60.0), 2.0, NEGRO);
        l.raya((x, 40.0), (x + 8.0, 60.0), 2.0, NEGRO);
    }
    let palabras = vec![PalabraHoja::nueva("HOLA", 56.0, 38.0, 62.0, 24.0)];
    let d = vectorizar(&l.rgba(), l.w as u32, l.h as u32, &palabras, &Opciones::default(), None);
    assert_eq!(d.segmentos.len(), 1, "{:?}", d.segmentos);
    assert!(d.rotulos.iter().any(|r| r.texto == "HOLA"));
}

#[test]
fn los_elementos_salen_con_sus_figuras() {
    let mut l = Lamina::nueva(300, 300);
    l.circunferencia((150.0, 150.0), 100.0, 2.0, NEGRO);
    l.arco((150.0, 150.0), 100.0, 0.0, 1.2, 4.0, VERDE);
    l.raya((50.0, 150.0), (250.0, 150.0), 2.0, AZUL);
    l.disco((50.0, 150.0), 4.0, NEGRO);
    l.disco((250.0, 150.0), 4.0, NEGRO);
    let d = l.medir();
    let els = a_elementos(&d, (1000.0, 2000.0), 1.0, 77, "Work Sans");
    let doc = crate::lienzo::documento(els);
    let leido = pixpin_motor2d::excalidraw::leer(&doc).expect("el lienzo lo entiende");
    let figuras: Vec<String> = leido.elementos().iter().map(|e| format!("{:?}", std::mem::discriminant(&e.figura))).collect();
    let tiene = |f: fn(&pixpin_motor2d::Figura) -> bool| leido.elementos().iter().any(|e| f(&e.figura));
    assert!(tiene(|f| matches!(f, pixpin_motor2d::Figura::Arco { .. })), "{figuras:?}");
    assert!(tiene(|f| matches!(f, pixpin_motor2d::Figura::Punto { .. })), "{figuras:?}");
    assert!(tiene(|f| matches!(f, pixpin_motor2d::Figura::Linea { .. })), "{figuras:?}");
    // Desplazados al origen pedido.
    for e in leido.elementos() {
        let c = e.caja();
        assert!(c.0 >= 990.0 && c.1 >= 1990.0, "{c:?}");
    }
}

#[test]
fn el_calco_conserva_las_rayas_finas() {
    // Una raya de 1 px y una circunferencia de 1,5 px junto a una mancha:
    // reducida a 256 px, la raya fina desaparecia o salia mancha.
    let mut l = Lamina::nueva(900, 600);
    l.raya((40.0, 60.0), (860.0, 120.0), 1.0, NEGRO);
    l.circunferencia((450.0, 360.0), 160.0, 1.5, AZUL);
    l.disco((200.0, 450.0), 60.0, ROJO);
    let c = crate::calcar::calcar(&l.rgba(), l.w as u32, l.h as u32, &crate::calcar::Parametros::default());
    let rayas: Vec<&Vec<(f64, f64)>> = c
        .trazos
        .iter()
        .filter_map(|t| match t {
            crate::calcar::Trazo::Raya { puntos, .. } => Some(puntos),
            _ => None,
        })
        .collect();
    let a_imagen = |p: (f64, f64)| ((p.0 - c.desplazamiento.0) / c.escala, (p.1 - c.desplazamiento.1) / c.escala);
    // La recta: una raya de punta a punta.
    let recta = rayas.iter().find(|r| {
        let (a, b) = (a_imagen(r[0]), a_imagen(r[r.len() - 1]));
        (cerca(a, (40.0, 60.0), 4.0) && cerca(b, (860.0, 120.0), 4.0)) || (cerca(b, (40.0, 60.0), 4.0) && cerca(a, (860.0, 120.0), 4.0))
    });
    assert!(recta.is_some(), "la raya fina se perdio: {} rayas", rayas.len());
    // La circunferencia: rayas cuyos puntos caen a 160 px del centro.
    let en_circ: usize = rayas.iter().flat_map(|r| r.iter()).filter(|&&p| (dist(a_imagen(p), (450.0, 360.0)) - 160.0).abs() < 2.5).count();
    assert!(en_circ >= 20, "la circunferencia fina se perdio ({en_circ} puntos)");
    // Y la mancha roja sigue siendo mancha.
    assert!(c.trazos.iter().any(|t| matches!(t, crate::calcar::Trazo::Mancha { color, .. } if color.starts_with("#d") || color.starts_with("#e"))), "la mancha roja");
}

#[test]
fn el_calco_de_una_foto_no_saca_rayas() {
    let l = foto(400, 300);
    let c = crate::calcar::calcar(&l.rgba(), l.w as u32, l.h as u32, &crate::calcar::Parametros::default());
    assert!(!c.trazos.iter().any(|t| matches!(t, crate::calcar::Trazo::Raya { .. })), "una foto no tiene rayas finas");
}

// ---------------------------------------------------------------------------
// Geometria exacta (15-oct): arcos de verdad, restricciones, figuras con su
// propio marco, colores de nucleo y el caso negativo.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

const AMARILLO: [u8; 3] = [255, 230, 170];

fn medir_con(l: &Lamina, pistas: &[&str]) -> Diagrama {
    let o = Opciones { pistas: pistas.iter().map(|s| s.to_string()).collect(), ..Default::default() };
    vectorizar(&l.rgba(), l.w as u32, l.h as u32, &[], &o, None)
}

/// El arco mas largo del diagrama.
fn el_arco(d: &Diagrama) -> &Arco {
    d.arcos.iter().max_by(|a, b| (a.r * a.barrido).total_cmp(&(b.r * b.barrido))).expect("algun arco")
}

fn casi_angulo(a: f64, b: f64, tol_grados: f64) -> bool {
    let mut x = (a - b).rem_euclid(TAU);
    if x > PI {
        x = TAU - x;
    }
    x <= tol_grados.to_radians()
}

#[test]
fn la_semicircunferencia_es_un_arco_de_180_con_su_diametro() {
    let mut l = Lamina::nueva(460, 300);
    // Un poco torcido a proposito (como a mano): 181 grados.
    l.arco((230.0, 240.0), 170.0, PI - 0.01, TAU, 2.0, NEGRO);
    l.raya((60.0, 240.0), (400.0, 240.0), 2.0, NEGRO);
    l.disco((60.0, 240.0), 4.0, AZUL);
    l.disco((400.0, 240.0), 4.0, AZUL);
    let d = medir_con(&l, &["SEMI CIRCUNFERENCIA"]);
    let a = el_arco(&d);
    eprintln!("{:?}", d.arcos);
    assert!(!a.entera(), "un arco, no la circunferencia entera");
    // Exacta: 180 grados, centro en el punto medio de los extremos.
    assert!((a.barrido - PI).abs() < 1e-6, "barrido {}", a.barrido.to_degrees());
    let (p, q) = a.extremos();
    assert!(dist(a.c, ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0)) < 1e-6);
    assert!(cerca(a.c, (230.0, 240.0), 2.0), "centro {:?}", a.c);
    assert!((a.r - 170.0).abs() < 2.0, "radio {}", a.r);
    // Y va por arriba (de pi a 2 pi en la pantalla).
    assert!(casi_angulo(a.inicio, PI, 2.0), "inicio {}", a.inicio.to_degrees());
}

#[test]
fn el_cuadrante_tiene_90_grados_y_oa_igual_a_ob() {
    let mut l = Lamina::nueva(380, 380);
    let o = (60.0, 320.0);
    // OA vertical, OB casi horizontal (medio grado), y el arco de A a B.
    let a = (60.0, 70.0);
    let b = (310.0, 322.0);
    l.raya(o, a, 2.0, NEGRO);
    l.raya(o, b, 2.0, NEGRO);
    l.arco(o, 250.0, -FRAC_PI_2, 0.0, 2.5, NEGRO);
    for p in [o, a, b] {
        l.disco(p, 4.0, NEGRO);
    }
    let d = medir_con(&l, &["AOB es un cuadrante"]);
    let arco = el_arco(&d);
    eprintln!("{:?}\n{:?}", d.arcos, d.segmentos);
    assert!((arco.barrido - FRAC_PI_2).abs() < 1e-6, "90 exactos: {}", arco.barrido.to_degrees());
    let (pa, pb) = arco.extremos();
    assert!((dist(arco.c, pa) - dist(arco.c, pb)).abs() < 1e-6, "OA = OB");
    assert!(cerca(arco.c, o, 2.5), "centro en O: {:?}", arco.c);
    // Los lados, exactamente vertical y horizontal, y llegan a los extremos.
    let vertical = d.segmentos.iter().find(|s| (s.a.0 - s.b.0).abs() < 1e-6 && s.largo() > 200.0).expect("OA vertical exacta");
    let horizontal = d.segmentos.iter().find(|s| (s.a.1 - s.b.1).abs() < 1e-6 && s.largo() > 200.0).expect("OB horizontal exacta");
    assert!([vertical.a, vertical.b].iter().any(|&p| dist(p, pa) < 1e-6 || dist(p, pb) < 1e-6));
    assert!([horizontal.a, horizontal.b].iter().any(|&p| dist(p, pa) < 1e-6 || dist(p, pb) < 1e-6));
}

#[test]
fn el_angulo_casi_recto_y_el_punto_casi_en_la_circunferencia_salen_exactos() {
    let mut l = Lamina::nueva(420, 420);
    let c = (210.0, 210.0);
    l.circunferencia(c, 150.0, 2.0, NEGRO);
    // Un diametro AB y P sobre la circunferencia (dibujado 2 px fuera).
    let (a, b) = ((60.0, 210.0), (360.0, 210.0));
    let ang = -1.1f64;
    let p = (c.0 + 152.0 * ang.cos(), c.1 + 152.0 * ang.sin());
    l.raya(a, b, 2.0, NEGRO);
    l.raya(a, p, 2.0, AZUL);
    l.raya(p, b, 2.0, AZUL);
    for q in [a, b, p] {
        l.disco(q, 4.0, NEGRO);
    }
    let d = l.medir();
    let circ = el_arco(&d);
    eprintln!("arcos {:?}
puntos {:?}
segs {:?}", d.arcos, d.puntos, d.segmentos);
    assert!(circ.entera());
    // P, sobre la circunferencia exacta.
    let pp = d.puntos.iter().map(|q| q.c).min_by(|x, y| dist(*x, p).total_cmp(&dist(*y, p))).unwrap();
    assert!((dist(pp, circ.c) - circ.r).abs() < 1e-6, "P a {} del radio", dist(pp, circ.c) - circ.r);
    // AB es diametro: el centro, en su punto medio.
    let ab = d.segmentos.iter().max_by(|x, y| x.largo().total_cmp(&y.largo())).unwrap();
    assert!(dist(circ.c, ((ab.a.0 + ab.b.0) / 2.0, (ab.a.1 + ab.b.1) / 2.0)) < 1e-6, "centro {:?} AB {:?}", circ.c, ab);
    // Y el angulo en P (Tales), recto.
    let azules: Vec<&Segmento> = d.segmentos.iter().filter(|s| s.color[2] as i32 > s.color[0] as i32 + 40).collect();
    assert_eq!(azules.len(), 2, "{:?}", d.segmentos);
    let v = |s: &Segmento| if dist(s.a, pp) < dist(s.b, pp) { (s.b.0 - s.a.0, s.b.1 - s.a.1) } else { (s.a.0 - s.b.0, s.a.1 - s.b.1) };
    let (u, w) = (v(azules[0]), v(azules[1]));
    let coseno = (u.0 * w.0 + u.1 * w.1) / (u.0.hypot(u.1) * w.0.hypot(w.1));
    assert!(coseno.abs() < 1e-3, "recto en P: coseno {coseno}");
}

#[test]
fn dos_figuras_con_escalas_distintas_cada_una_en_su_marco() {
    let mut l = Lamina::nueva(760, 420);
    // Una semicircunferencia grande y una pequena, lejos.
    l.arco((220.0, 330.0), 180.0, PI, TAU, 2.0, NEGRO);
    l.raya((40.0, 330.0), (400.0, 330.0), 2.0, NEGRO);
    l.arco((620.0, 200.0), 60.0, PI, TAU, 1.5, ROJO);
    l.raya((560.0, 200.0), (680.0, 200.0), 1.5, ROJO);
    let d = medir_con(&l, &["semicircunferencia"]);
    let figs = restricciones::figuras(&d, 6.0);
    assert_eq!(figs.len(), 2, "dos figuras: {figs:?}");
    let escalas: Vec<f64> = figs.iter().map(|f| f.escala()).collect();
    assert!(escalas.iter().cloned().fold(0.0, f64::max) > 2.5 * escalas.iter().cloned().fold(f64::MAX, f64::min), "{escalas:?}");
    let grande = d.arcos.iter().find(|a| a.r > 150.0).expect("la grande");
    let chica = d.arcos.iter().find(|a| a.r < 80.0).expect("la chica");
    for a in [grande, chica] {
        assert!((a.barrido - PI).abs() < 1e-6, "{a:?}");
    }
    assert!(cerca(chica.c, (620.0, 200.0), 2.0) && (chica.r - 60.0).abs() < 2.0, "{chica:?}");
    assert!(cerca(grande.c, (220.0, 330.0), 2.0) && (grande.r - 180.0).abs() < 2.5, "{grande:?}");
}

#[test]
fn una_raya_roja_fina_junto_a_un_relleno_amarillo_es_roja_y_fina() {
    let mut l = Lamina::nueva(360, 300);
    // El relleno amarillo y su borde rojo de 1,5 px (como OA y OB).
    l.poligono(&[(60.0, 40.0), (300.0, 40.0), (300.0, 250.0), (60.0, 250.0)], AMARILLO);
    l.raya((60.0, 250.0), (300.0, 250.0), 1.5, ROJO);
    l.raya((60.0, 40.0), (60.0, 250.0), 1.5, ROJO);
    let d = l.medir();
    let rojas: Vec<&Segmento> = d.segmentos.iter().filter(|s| s.largo() > 150.0).collect();
    assert!(rojas.len() >= 2, "{:?}", d.segmentos);
    for s in rojas {
        let c = s.color;
        assert!(c[0] as i32 > 150 && (c[1] as i32) < 110 && (c[2] as i32) < 110, "roja, no marron: {c:?}");
        assert!(s.grosor <= 2.6, "fina (sin el amarillo de al lado): {}", s.grosor);
    }
}

#[test]
fn una_curva_libre_no_se_fuerza_a_arco() {
    let mut l = Lamina::nueva(500, 260);
    // Una onda (seno): no es ninguna circunferencia.
    let pts: Vec<(f64, f64)> = (0..=200).map(|k| (40.0 + 2.1 * k as f64, 130.0 + 70.0 * (k as f64 * 0.045).sin())).collect();
    for w in pts.windows(2) {
        l.raya(w[0], w[1], 2.0, NEGRO);
    }
    let d = l.medir();
    eprintln!("{:?}", d.arcos);
    // Ningun arco que se la quede entera (uno por joroba como mucho), ni
    // una circunferencia.
    assert!(d.arcos.iter().all(|a| !a.entera()), "{:?}", d.arcos);
    let largo_curva: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
    assert!(d.arcos.iter().all(|a| a.r * a.barrido < 0.6 * largo_curva), "{:?}", d.arcos);
}

#[test]
fn una_marca_de_angulo_palida_sale_como_sector() {
    let mut l = Lamina::nueva(460, 360);
    let v = (80.0, 300.0);
    let a = -(45f64.to_radians());
    // Primero la marca (verde palida, translucida) y encima las dos rayas.
    let mut sector = vec![v];
    for k in 0..=12 {
        let t = a * k as f64 / 12.0;
        sector.push((v.0 + 34.0 * t.cos(), v.1 + 34.0 * t.sin()));
    }
    l.poligono(&sector, [185, 232, 195]);
    l.raya(v, (420.0, 300.0), 2.0, NEGRO);
    l.raya(v, (v.0 + 330.0 * a.cos(), v.1 + 330.0 * a.sin()), 2.0, NEGRO);
    let d = l.medir();
    eprintln!("{:?}", d.marcas);
    assert_eq!(d.marcas.len(), 1, "{:?} {:?}", d.marcas, d.regiones);
    let m = &d.marcas[0];
    assert!(cerca(m.v, v, 3.0), "{m:?}");
    assert!((m.barrido - 45f64.to_radians()).abs() < 3f64.to_radians(), "{m:?}");
    assert!((m.r - 34.0).abs() < 6.0, "{m:?}");
    // Caso negativo: sin marca, ninguna.
    let mut l = Lamina::nueva(460, 360);
    l.raya(v, (420.0, 300.0), 2.0, NEGRO);
    l.raya(v, (v.0 + 330.0 * a.cos(), v.1 + 330.0 * a.sin()), 2.0, NEGRO);
    assert!(l.medir().marcas.is_empty());
}

/// **Las formas de un diagrama de flujo** (16-oct): el ovalo de «Inicio»
/// con su relleno sale como ovalo (no en arcos sueltos), la caja con su
/// relleno y el rombo de la decision como rombo.
#[test]
fn ovalo_caja_y_rombo_de_un_diagrama_de_flujo() {
    let azul = [40, 90, 170];
    let claro = [222, 236, 252];
    let mut l = Lamina::nueva(600, 640);
    // El ovalo: relleno y borde.
    let ovalo: Vec<(f64, f64)> = (0..120).map(|k| {
        let t = std::f64::consts::TAU * k as f64 / 120.0;
        (300.0 + 110.0 * t.cos(), 80.0 + 42.0 * t.sin())
    }).collect();
    l.poligono(&ovalo, claro);
    for k in 0..120 {
        l.raya(ovalo[k], ovalo[(k + 1) % 120], 2.5, azul);
    }
    // La caja.
    let caja = [(190.0, 200.0), (410.0, 200.0), (410.0, 290.0), (190.0, 290.0)];
    l.poligono(&caja, claro);
    for k in 0..4 {
        l.raya(caja[k], caja[(k + 1) % 4], 2.5, azul);
    }
    // El rombo.
    let rombo = [(300.0, 360.0), (460.0, 450.0), (300.0, 540.0), (140.0, 450.0)];
    l.poligono(&rombo, claro);
    for k in 0..4 {
        l.raya(rombo[k], rombo[(k + 1) % 4], 2.5, azul);
    }
    // Las flechas que los unen.
    l.raya((300.0, 122.0), (300.0, 200.0), 2.0, NEGRO);
    l.raya((300.0, 290.0), (300.0, 360.0), 2.0, NEGRO);
    let d = l.medir();
    for r in &d.regiones {
        let b = r.contorno.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p.0), a.1.min(p.1), a.2.max(p.0), a.3.max(p.1)));
        eprintln!("region {:?} area {:.0} caja {b:?}", r.color, r.area);
    }
    let ov = d.ovalos.iter().find(|o| !o.rombo).expect("el ovalo");
    assert!(cerca(ov.c, (300.0, 80.0), 3.0) && (ov.rx - 110.0).abs() < 4.0 && (ov.ry - 42.0).abs() < 4.0, "{ov:?}");
    assert!(ov.relleno.is_some());
    let ro = d.ovalos.iter().find(|o| o.rombo).expect("el rombo");
    assert!(cerca(ro.c, (300.0, 450.0), 3.0) && (ro.rx - 160.0).abs() < 6.0, "{ro:?}");
    let q = d.recuadros.iter().find(|q| (q.x1 - q.x0 - 220.0).abs() < 6.0).expect("la caja");
    assert!(q.relleno.is_some(), "con su relleno: {q:?}");
    // Y sin arcos sueltos del ovalo.
    assert!(d.arcos.is_empty(), "{:?}", d.arcos);
}
