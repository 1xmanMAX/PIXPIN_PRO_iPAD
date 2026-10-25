//! **Medir el calcado** (pedido del 14-oct: «las lineas salen en escaleras»).
//!
//! No corre con `cargo test` normal (`#[ignore]`): calca figuras de las que se
//! sabe el borde exacto (circulo, elipse girada, letra S, onda con esquinas) y
//! el perrito, y apunta lo que importa para ver si un cambio mejora o empeora:
//!
//! - **error** medio y maximo de lo que pinta el lienzo frente al borde de
//!   verdad, en pixeles de la imagen (se muestrea cada medio pixel);
//! - **escalonado**: el giro total de los contornos frente al de verdad
//!   (una escalera gira 90° en cada peldano: un circulo de verdad da 1,0) y
//!   los «dientes», vertices que giran mas de 45° donde no hay esquina;
//! - puntos, elementos y tiempo.
//!
//! Y pinta por el camino del PDF del lienzo la figura entera y un trozo
//! ampliado ocho veces en una curva, para mirarlos.
//!
//! ```text
//! CALCAR_ETIQUETA=despues cargo test -p pixpin-ia --test calcar_medida -- --ignored --nocapture
//! ```
//! Salida en `CALCAR_SALIDA` (por omision `scratchpad\banco-imagenes\calcar-suave`)
//! `\<etiqueta>\`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use pixpin_ia::calcar::{self, Parametros};
use serde_json::Value;

/// **Sin pintor de PDF** (solo en Apple): en Windows lo dibujado se pasa a
/// pixeles abriendo su PDF con `pixpin_pdf::Documento` (`Windows.Data.Pdf`,
/// con el COM del hilo de `pixpin_shell`). Aqui eso lo hace PDFKit desde
/// Swift, asi que este hace sus veces y no pinta nada: el banco corre igual
/// pero no guarda las imagenes (se sacan con la copia de Windows).
mod sin_pintor {
    pub struct Documento;

    impl Documento {
        pub fn abrir(_ruta: &std::path::Path) -> Result<Documento, ()> {
            Err(())
        }

        pub fn renderizar(&self, _indice: u32, _ancho: u32) -> Result<pixpin_codec::ImagenRgba, ()> {
            Err(())
        }

        #[allow(dead_code)]
        pub fn renderizar_trozo(
            &self,
            _indice: u32,
            _trozo: (f32, f32, f32, f32),
            _ancho: u32,
            _alto: u32,
        ) -> Result<pixpin_codec::ImagenRgba, ()> {
            Err(())
        }
    }
}

const W: u32 = 768;
const H: u32 = 576;

fn salida() -> PathBuf {
    let base = std::env::var("CALCAR_SALIDA").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(
            r"C:\Users\MaxBook\AppData\Local\Temp\claude\F--THE-FORGE-PIXPIN-PC-VERSION-MAX\afe461c5-d9fa-4287-b8b5-3ab7961f6cb3\scratchpad\banco-imagenes\calcar-suave",
        )
    });
    let d = base.join(std::env::var("CALCAR_ETIQUETA").unwrap_or_else(|_| "despues".into()));
    let _ = std::fs::create_dir_all(&d);
    d
}

type Dentro = Box<dyn Fn(f64, f64) -> bool>;
type Error = Box<dyn Fn((f64, f64)) -> f64>;

struct Figura {
    nombre: &'static str,
    dentro: Dentro,
    error: Error,
    /// Giro total del borde de verdad (radianes), si se sabe.
    giro: Option<f64>,
    /// Esquinas de verdad (no cuentan como dientes).
    esquinas: Vec<(f64, f64)>,
    /// Trozo a ampliar, en pixeles de la imagen: (x, y, lado).
    lupa: (f64, f64, f64),
}

fn dist_seg(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
}

fn dist_polilinea(p: (f64, f64), pl: &[(f64, f64)]) -> f64 {
    pl.windows(2).map(|s| dist_seg(p, s[0], s[1])).fold(f64::MAX, f64::min)
}

fn figuras() -> Vec<Figura> {
    let (cx, cy) = (W as f64 / 2.0, H as f64 / 2.0);
    let mut v = Vec::new();
    // Circulo.
    let r = 200.0;
    v.push(Figura {
        nombre: "circulo",
        dentro: Box::new(move |x, y| (x - cx).powi(2) + (y - cy).powi(2) <= r * r),
        error: Box::new(move |p| (((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt() - r).abs()),
        giro: Some(std::f64::consts::TAU),
        esquinas: vec![],
        lupa: (cx - r * 0.71 - 100.0, cy - r * 0.71 - 100.0, 200.0),
    });
    // Elipse girada 20°.
    let (a, b, g) = (300.0f64, 150.0f64, 20f64.to_radians());
    let local = move |x: f64, y: f64| {
        let (dx, dy) = (x - cx, y - cy);
        (dx * g.cos() + dy * g.sin(), -dx * g.sin() + dy * g.cos())
    };
    let borde: Vec<(f64, f64)> = (0..=4000)
        .map(|i| {
            let t = i as f64 / 4000.0 * std::f64::consts::TAU;
            let (u, w) = (a * t.cos(), b * t.sin());
            (cx + u * g.cos() - w * g.sin(), cy + u * g.sin() + w * g.cos())
        })
        .collect();
    v.push(Figura {
        nombre: "elipse",
        dentro: Box::new(move |x, y| {
            let (u, w) = local(x, y);
            (u / a).powi(2) + (w / b).powi(2) <= 1.0
        }),
        error: Box::new(move |p| dist_polilinea(p, &borde)),
        giro: Some(std::f64::consts::TAU),
        esquinas: vec![],
        lupa: (cx + 60.0, cy - 200.0, 200.0),
    });
    // Letra S: un trazo de 56 px de grueso por dos arcos.
    let rr = 80.0;
    let (c1, c2) = ((cx, cy - rr), (cx, cy + rr));
    let mut centro: Vec<(f64, f64)> = Vec::new();
    for i in 0..=400 {
        let t = (20.0 + (270.0 - 20.0) * i as f64 / 400.0f64).to_radians();
        centro.push((c1.0 + rr * t.cos(), c1.1 - rr * t.sin()));
    }
    for i in 1..=400 {
        let t = (90.0 - 250.0 * i as f64 / 400.0f64).to_radians();
        centro.push((c2.0 + rr * t.cos(), c2.1 - rr * t.sin()));
    }
    let hw = 28.0;
    // Distancia exacta a los dos arcos (para pintarla rapido).
    let arco = move |p: (f64, f64), c: (f64, f64), desde: f64, hasta: f64| -> f64 {
        let ang = (-(p.1 - c.1)).atan2(p.0 - c.0).to_degrees();
        let (lo, hi) = (desde.min(hasta), desde.max(hasta));
        let mut dentro_ang = false;
        for k in [-360.0, 0.0, 360.0] {
            if ang + k >= lo && ang + k <= hi {
                dentro_ang = true;
            }
        }
        if dentro_ang {
            (((p.0 - c.0).powi(2) + (p.1 - c.1).powi(2)).sqrt() - rr).abs()
        } else {
            let e = |t: f64| {
                let t = t.to_radians();
                (c.0 + rr * t.cos(), c.1 - rr * t.sin())
            };
            let (a, b) = (e(desde), e(hasta));
            ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt().min(((p.0 - b.0).powi(2) + (p.1 - b.1).powi(2)).sqrt())
        }
    };
    let d_s = move |p: (f64, f64)| arco(p, c1, 20.0, 270.0).min(arco(p, c2, -160.0, 90.0));
    v.push(Figura {
        nombre: "letra-s",
        dentro: Box::new(move |x, y| d_s((x, y)) <= hw),
        error: Box::new(move |p| (d_s(p) - hw).abs()),
        giro: None,
        esquinas: vec![],
        lupa: (cx - rr - hw - 20.0, cy - rr - 100.0, 200.0),
    });
    // Onda: lo de debajo de una senoide, en un rectangulo (4 esquinas).
    let (x0, x1, y1) = (64.0, 704.0, 480.0);
    let f = move |x: f64| cy - 40.0 + 80.0 * (std::f64::consts::TAU * (x - x0) / 256.0).sin();
    let mut borde: Vec<(f64, f64)> = (0..=6400).map(|i| {
        let x = x0 + (x1 - x0) * i as f64 / 6400.0;
        (x, f(x))
    }).collect();
    borde.push((x1, y1));
    borde.push((x0, y1));
    borde.push(borde[0]);
    // Giro de verdad: el de la polilinea fina.
    let giro_onda = giro_total(&borde[..borde.len() - 1], true);
    let esquinas = vec![(x0, f(x0)), (x1, f(x1)), (x1, y1), (x0, y1)];
    v.push(Figura {
        nombre: "onda",
        dentro: Box::new(move |x, y| x >= x0 && x <= x1 && y <= y1 && y >= f(x)),
        error: Box::new(move |p| dist_polilinea(p, &borde)),
        giro: Some(giro_onda),
        esquinas,
        lupa: (x0 - 20.0, cy - 140.0, 200.0),
    });
    v
}

/// Pinta la figura con antialias (4x4 muestras por pixel), negra sobre blanco.
fn rasterizar(f: &Figura) -> Vec<u8> {
    let mut px = vec![255u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let mut n = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    if (f.dentro)(x as f64 + (sx as f64 + 0.5) / 4.0, y as f64 + (sy as f64 + 0.5) / 4.0) {
                        n += 1;
                    }
                }
            }
            // Azul oscuro sobre blanco, con el borde mezclado.
            let a = n as f64 / 16.0;
            let i = ((y * W + x) * 4) as usize;
            for (k, c) in [30.0, 60.0, 160.0].into_iter().enumerate() {
                px[i + k] = (255.0 * (1.0 - a) + c * a).round() as u8;
            }
            px[i + 3] = 255;
        }
    }
    px
}

/// Lo que pinta el lienzo de cada elemento: polilineas en coordenadas del
/// lienzo (con la curva del motor si la linea lleva `roundness`).
fn polilineas(els: &[Value]) -> Vec<(Vec<(f64, f64)>, bool)> {
    let mut v = Vec::new();
    for e in els {
        let tipo = e["type"].as_str().unwrap_or("");
        let (x, y) = (e["x"].as_f64().unwrap_or(0.0), e["y"].as_f64().unwrap_or(0.0));
        let pts = |a: &Value| -> Vec<(f64, f64)> {
            a.as_array()
                .map(|l| l.iter().map(|p| (x + p[0].as_f64().unwrap_or(0.0), y + p[1].as_f64().unwrap_or(0.0))).collect())
                .unwrap_or_default()
        };
        match tipo {
            "pixpin-region" => {
                v.push((pts(&e["points"]), true));
                for h in e["huecos"].as_array().into_iter().flatten() {
                    v.push((pts(h), true));
                }
            }
            "line" => {
                let p = pts(&e["points"]);
                let curva = !e["roundness"].is_null();
                let p = if curva && p.len() >= 2 {
                    let q: Vec<_> = p.iter().map(|&(a, b)| pixpin_motor2d::vector::Punto2::nuevo(a as f32, b as f32)).collect();
                    pixpin_motor2d::curva::muestrear(&q).into_iter().map(|q| (q.x as f64, q.y as f64)).collect()
                } else {
                    p
                };
                let cerrada = p.len() > 2 && {
                    let (a, b) = (p[0], p[p.len() - 1]);
                    (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
                };
                let p = if cerrada { p[..p.len() - 1].to_vec() } else { p };
                v.push((p, cerrada));
            }
            _ => {}
        }
    }
    v
}

fn giro_total(p: &[(f64, f64)], cerrada: bool) -> f64 {
    giros(p, cerrada).iter().map(|g| g.1.abs()).sum()
}

/// (vertice, giro con signo) de cada vertice.
fn giros(p: &[(f64, f64)], cerrada: bool) -> Vec<((f64, f64), f64)> {
    let n = p.len();
    if n < 3 {
        return Vec::new();
    }
    let rango: Vec<usize> = if cerrada { (0..n).collect() } else { (1..n - 1).collect() };
    let mut v = Vec::new();
    for i in rango {
        let (a, b, c) = (p[(i + n - 1) % n], p[i], p[(i + 1) % n]);
        let (u, w) = ((b.0 - a.0, b.1 - a.1), (c.0 - b.0, c.1 - b.1));
        if (u.0 == 0.0 && u.1 == 0.0) || (w.0 == 0.0 && w.1 == 0.0) {
            continue;
        }
        let g = (u.0 * w.1 - u.1 * w.0).atan2(u.0 * w.0 + u.1 * w.1);
        v.push((b, g));
    }
    v
}

struct Medida {
    elementos: usize,
    puntos: usize,
    err_medio: f64,
    err_max: f64,
    giro: f64,
    dientes: usize,
    ms: f64,
    /// Por mil (solo con manchas).
    rendijas: f64,
}

fn medir(f: &Figura, px: &[u8], p: &Parametros) -> (Medida, Vec<Value>, calcar::Calcado) {
    let t0 = Instant::now();
    let c = calcar::calcar(px, W, H, p);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    let els = pixpin_ia::lienzo::elementos_de_calcado(&c, (0.0, 0.0), 1);
    let a_img = |(x, y): (f64, f64)| ((x - c.desplazamiento.0) / c.escala, (y - c.desplazamiento.1) / c.escala);
    let pls = polilineas(&els);
    let puntos = els
        .iter()
        .map(|e| e["points"].as_array().map_or(0, |a| a.len()) + e["huecos"].as_array().map_or(0, |h| h.iter().map(|x| x.as_array().map_or(0, |a| a.len())).sum()))
        .sum();
    let (mut suma, mut n, mut max) = (0.0, 0usize, 0.0f64);
    let mut giro = 0.0;
    let mut dientes = 0;
    for (pl, cerrada) in &pls {
        let img: Vec<(f64, f64)> = pl.iter().map(|q| a_img(*q)).collect();
        let m = img.len();
        let tramos = if *cerrada { m } else { m.saturating_sub(1) };
        for i in 0..tramos {
            let (a, b) = (img[i], img[(i + 1) % m]);
            let l = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            let k = (l / 0.5).ceil().max(1.0) as usize;
            for j in 0..k {
                let t = j as f64 / k as f64;
                let e = (f.error)((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
                suma += e;
                n += 1;
                max = max.max(e);
            }
        }
        for (q, g) in giros(&img, *cerrada) {
            giro += g.abs();
            let esquina = f.esquinas.iter().any(|e| ((e.0 - q.0).powi(2) + (e.1 - q.1).powi(2)).sqrt() < 6.0);
            if g.abs() > 45f64.to_radians() && !esquina {
                dientes += 1;
            }
        }
    }
    (
        Medida {
            elementos: els.len(),
            puntos,
            err_medio: suma / n.max(1) as f64,
            err_max: max,
            giro: f.giro.map_or(giro, |g| giro / g),
            dientes,
            ms,
            rendijas: 0.0,
        },
        els,
        c,
    )
}

/// Pinta los elementos sobre un papel de 800x600 (entero, y un trozo
/// ampliado si se pide: en fracciones del papel).
fn pintar(els: &[Value], ruta: &Path, lupa: Option<(f64, f64, f64)>) -> Option<pixpin_codec::imagen::ImagenRgba> {
    let doc = pixpin_ia::lienzo::documento(els.to_vec());
    let Ok(lienzo) = pixpin_motor2d::excalidraw::leer(&doc) else { return None };
    let escena = pixpin_motor2d::excalidraw::a_escena(&lienzo);
    let hojas = pixpin_motor2d::exportar::hojas(&escena, pixpin_motor2d::exportar::Alcance::Todo, &[], Some((800.0, 600.0)));
    let blanco = pixpin_motor2d::ColorRgba::opaco(1.0, 1.0, 1.0);
    let Some(pdf) = pixpin_pdf::escribir::de_hojas(&hojas, Some(blanco), &|_| None) else { return None };
    let tmp = ruta.with_extension("pdf");
    if std::fs::write(&tmp, pdf).is_err() {
        return None;
    }
    let img = crate::sin_pintor::Documento::abrir(&tmp).ok().and_then(|d| match lupa {
        None => d.renderizar(0, 1000).ok(),
        Some((x, y, l)) => d.renderizar_trozo(0, ((x / 800.0) as f32, (y / 600.0) as f32, (l / 800.0) as f32, (l / 600.0) as f32), 800, 800).ok(),
    });
    let _ = std::fs::remove_file(&tmp);
    let img = img?;
    if let Ok(png) = pixpin_codec::imagen::codificar_png(&img) {
        let _ = std::fs::write(ruta, png);
    }
    Some(img)
}

/// **Rendijas**: de los pixeles pintados (papel de 800x600 entero) que caen
/// bien dentro de la figura de verdad (a mas de 3 pixeles pintados de su
/// borde), los que salen blancos, por mil. Entre dos manchas vecinas
/// calcadas por separado puede quedar una rendija por la que asoma el papel.
fn rendijas(img: &pixpin_codec::imagen::ImagenRgba, c: &calcar::Calcado, els: &[Value], lleno: &dyn Fn(f64, f64) -> bool) -> f64 {
    // Del pixel pintado al lienzo: la caja de lo pintado frente a la de los
    // elementos (con su borde de 2: 1 por fuera).
    let mut caja = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for (pl, _) in polilineas(els) {
        for q in pl {
            caja = [caja[0].min(q.0 - 1.0), caja[1].min(q.1 - 1.0), caja[2].max(q.0 + 1.0), caja[3].max(q.1 + 1.0)];
        }
    }
    let mut pintada = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for v in 0..img.alto {
        for u in 0..img.ancho {
            let i = ((v * img.ancho + u) * 4) as usize;
            if img.pixeles[i..i + 3].iter().any(|&k| k < 200) {
                pintada = [pintada[0].min(u as f64), pintada[1].min(v as f64), pintada[2].max(u as f64 + 1.0), pintada[3].max(v as f64 + 1.0)];
            }
        }
    }
    if pintada[0] >= pintada[2] || caja[0] >= caja[2] {
        return 0.0;
    }
    let sx = (caja[2] - caja[0]) / (pintada[2] - pintada[0]);
    let sy = (caja[3] - caja[1]) / (pintada[3] - pintada[1]);
    let a_img = |u: f64, v: f64| {
        let (x, y) = (caja[0] + (u - pintada[0]) * sx, caja[1] + (v - pintada[1]) * sy);
        ((x - c.desplazamiento.0) / c.escala, (y - c.desplazamiento.1) / c.escala)
    };
    let r = 3.0;
    let (mut dentro, mut blancos) = (0usize, 0usize);
    for v in 0..img.alto {
        for u in 0..img.ancho {
            let (fu, fv) = (u as f64 + 0.5, v as f64 + 0.5);
            let todo_dentro = [(0.0, 0.0), (r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r), (r, r), (-r, -r), (r, -r), (-r, r)]
                .iter()
                .all(|(du, dv)| {
                    let (x, y) = a_img(fu + du, fv + dv);
                    lleno(x, y)
                });
            if !todo_dentro {
                continue;
            }
            dentro += 1;
            let i = ((v * img.ancho + u) * 4) as usize;
            if img.pixeles[i..i + 3].iter().all(|&k| k >= 245) {
                blancos += 1;
            }
        }
    }
    blancos as f64 * 1000.0 / dentro.max(1) as f64
}

fn linea(nombre: &str, modo: &str, m: &Medida) -> String {
    format!(
        "| {nombre} | {modo} | {:.2} | {:.2} | {:.2} | {} | {:.1} | {} | {} | {:.0} |",
        m.err_medio, m.err_max, m.giro, m.dientes, m.rendijas, m.elementos, m.puntos, m.ms
    )
}

#[test]
#[ignore = "medida a mano: ver la cabecera"]
fn medir_calcado() {
    let dir = salida();
    let mut tabla = vec![
        "| figura | modo | error medio px | error max px | giro / verdad | dientes | rendijas ‰ | elementos | puntos | ms |".to_string(),
        "|---|---|---|---|---|---|---|---|---|---|".to_string(),
    ];
    let modos = [
        ("manchas", Parametros::default()),
        ("contornos", Parametros { solo_contornos: true, ..Default::default() }),
    ];
    for f in figuras() {
        let px = rasterizar(&f);
        for (modo, p) in &modos {
            let (mut m, els, c) = medir(&f, &px, p);
            let img = pintar(&els, &dir.join(format!("{}-{modo}.png", f.nombre)), None);
            if !p.solo_contornos {
                m.rendijas = img.map_or(0.0, |i| rendijas(&i, &c, &els, &*f.dentro));
            }
            tabla.push(linea(f.nombre, modo, &m));

            let (lx, ly, ll) = f.lupa;
            let (x, y) = (lx * c.escala + c.desplazamiento.0, ly * c.escala + c.desplazamiento.1);
            pintar(&els, &dir.join(format!("{}-{modo}-lupa.png", f.nombre)), Some((x, y, ll * c.escala)));
        }
    }
    // El perrito, a 768 px (lo que tarda) y como es.
    let ruta = PathBuf::from(
        r"C:\Users\MaxBook\AppData\Local\Temp\claude\F--THE-FORGE-PIXPIN-PC-VERSION-MAX\afe461c5-d9fa-4287-b8b5-3ab7961f6cb3\scratchpad\perrito\perrito.png",
    );
    if let Ok(img) = image::open(&ruta) {
        let img = image::imageops::resize(&img.to_rgba8(), 768, 768, image::imageops::FilterType::CatmullRom);
        let nada = Figura {
            nombre: "perrito",
            dentro: Box::new(|_, _| false),
            error: Box::new(|_| 0.0),
            giro: None,
            esquinas: vec![],
            lupa: (0.0, 0.0, 0.0),
        };
        for (modo, p) in modos.iter().chain([("manchas-alto", Parametros { detalle: calcar::Detalle::Alto, ..Default::default() })].iter()) {
            let t0 = Instant::now();
            let c = calcar::calcar(img.as_raw(), 768, 768, p);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            let els = pixpin_ia::lienzo::elementos_de_calcado(&c, (0.0, 0.0), 1);
            let pls = polilineas(&els);
            let mut dientes = 0;
            let mut giro = 0.0;
            for (pl, cerrada) in &pls {
                let img: Vec<(f64, f64)> = pl.iter().map(|q| ((q.0 - c.desplazamiento.0) / c.escala, (q.1 - c.desplazamiento.1) / c.escala)).collect();
                for (_, g) in giros(&img, *cerrada) {
                    giro += g.abs();
                    dientes += usize::from(g.abs() > 45f64.to_radians());
                }
            }
            let puntos: usize = pls.iter().map(|p| p.0.len()).sum();
            let mut m = Medida { elementos: els.len(), puntos, err_medio: 0.0, err_max: 0.0, giro: giro / std::f64::consts::TAU / pls.len().max(1) as f64, dientes, ms, rendijas: 0.0 };

            let pintado = pintar(&els, &dir.join(format!("perrito-{modo}.png")), None);
            if !p.solo_contornos {
                let lleno = |x: f64, y: f64| {
                    if x < 0.0 || y < 0.0 || x >= 768.0 || y >= 768.0 {
                        return false;
                    }
                    let q = img.get_pixel(x as u32, y as u32).0;
                    q[3] > 128 && !q[..3].iter().all(|&k| k > 235)
                };
                m.rendijas = pintado.map_or(0.0, |i| rendijas(&i, &c, &els, &lleno));
            }
            tabla.push(linea(nada.nombre, modo, &m));
            // La oreja izquierda y el hocico, ampliados.
            pintar(&els, &dir.join(format!("perrito-{modo}-lupa-oreja.png")), Some((215.0, 140.0, 90.0)));
            pintar(&els, &dir.join(format!("perrito-{modo}-lupa-boca.png")), Some((350.0, 350.0, 100.0)));
            pintar(&els, &dir.join(format!("perrito-{modo}-lupa-frente.png")), Some((430.0, 250.0, 130.0)));
        }
    }
    let texto = tabla.join("\n");
    println!("\n{texto}\n");
    let _ = std::fs::write(dir.join("medidas.md"), &texto);
}

/// Lo que tarda calcar el perrito a 768 px (mediana de 5), en cada modo.
/// Para medirlo como en la app, con `pixpin-ia` optimizado:
/// `cargo test -p pixpin-ia --test calcar_medida --config profile.dev.package.pixpin-ia.opt-level=3 -- --ignored --nocapture tiempo`
#[test]
#[ignore = "medida a mano: ver la cabecera"]
fn tiempo_del_perrito() {
    let ruta = PathBuf::from(
        r"C:\Users\MaxBook\AppData\Local\Temp\claude\F--THE-FORGE-PIXPIN-PC-VERSION-MAX\afe461c5-d9fa-4287-b8b5-3ab7961f6cb3\scratchpad\perrito\perrito.png",
    );
    let Ok(img) = image::open(&ruta) else { return };
    let img = image::imageops::resize(&img.to_rgba8(), 768, 768, image::imageops::FilterType::CatmullRom);
    for (modo, p) in [
        ("manchas", Parametros::default()),
        ("contornos", Parametros { solo_contornos: true, ..Default::default() }),
        ("manchas-alto", Parametros { detalle: calcar::Detalle::Alto, ..Default::default() }),
    ] {
        let mut t: Vec<f64> = (0..5)
            .map(|_| {
                let t0 = Instant::now();
                let _ = calcar::calcar(img.as_raw(), 768, 768, &p);
                t0.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        t.sort_by(f64::total_cmp);
        println!("TIEMPO {modo}: {:.0} ms", t[2]);
    }
}
