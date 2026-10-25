//! **Calcar una imagen al lienzo**: de una foto o un dibujo, sus contornos y
//! sus manchas de color como elementos del lienzo.
//!
//! Pedido del usuario (13-oct-2026): «que pueda copiar lo de una imagen en un
//! lienzo». Un modelo de 2B no puede escribir los cientos de coordenadas que
//! hacen falta para copiar una forma (medido: describe 3-6 figuras sueltas y
//! alucina el resto), asi que el trabajo lo hace aqui un **algoritmo
//! clasico**, determinista y rapido; el modelo solo elige los parametros
//! (detalle, con o sin colores, solo contornos, una zona) con la herramienta
//! `calcar_imagen`.
//!
//! ## Dos caminos
//!
//! - **Manchas de color** (lo normal): k-medias en RGB sobre la imagen
//!   reducida (media de areas) → etiqueta por pixel, alisada con un filtro
//!   de mayoria → cada mancha conexa da un campo sacado del color (cuanto
//!   de cada pixel del borde es de la mancha: el antialias y los degradados
//!   ponen el borde dentro del pixel) → contorno **sub-pixel** con
//!   *marching squares* interpolado → esquinas y **curvas de Bezier**
//!   (Schneider) → aplanadas con tolerancia de pantalla (`trazado.rs`). Sale
//!   como `pixpin-region` (el poligono relleno con agujeros del bote de
//!   pintura, que el lienzo del PC y el del movil ya pintan). La mancha de
//!   debajo se mete un poco bajo las de encima: sin rendijas. El fondo (el
//!   color que mas toca el borde) no se dibuja.
//! - **Solo contornos**: Canny (gauss, Sobel, supresion de no maximos con
//!   posicion sub-pixel de Devernay, histeresis con umbrales sacados de la
//!   propia imagen) → cadenas de pixeles → las mismas curvas. Sale como
//!   lineas (`line`), que en el lienzo se editan nodo a nodo.
//!
//! Hasta el 14-oct los contornos iban de pixel en pixel y se simplificaban
//! con Ramer-Douglas-Peucker: el usuario veia «escaleras en vez de curvas».
//! Medido con `tests/calcar_medida.rs`.
//!
//! Todo con tope: la imagen se reduce a 160/256/384 px de lado segun el
//! detalle, y salen como mucho [`Detalle::tope`] elementos, los mas grandes.
//! Una foto de 768 px se calca en decimas de segundo (prueba abajo).

mod trazado;

/// Area con signo (formula del cordon).
pub use trazado::area;

use std::collections::HashMap;

use crate::herramientas::{ALTO_LIENZO, ANCHO_LIENZO};

/// Lo que se puede apartar del ajuste la polilinea que se manda al lienzo,
/// en pixeles del lienzo.
const PLANO_EN_PANTALLA: f64 = 0.25;

/// Cuanto detalle se quiere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detalle {
    Bajo,
    Medio,
    Alto,
}

impl Detalle {
    /// Lado mayor de la imagen con la que se trabaja.
    fn lado(self) -> u32 {
        match self {
            Detalle::Bajo => 160,
            Detalle::Medio => 256,
            Detalle::Alto => 384,
        }
    }
    /// Cuantos colores busca k-medias.
    fn colores(self) -> usize {
        match self {
            Detalle::Bajo => 5,
            Detalle::Medio => 8,
            Detalle::Alto => 12,
        }
    }
    /// Lo que se puede apartar la curva ajustada del contorno sub-pixel, en
    /// pixeles de la imagen reducida.
    fn tolerancia(self) -> f64 {
        match self {
            Detalle::Bajo => 0.6,
            Detalle::Medio => 0.4,
            Detalle::Alto => 0.35,
        }
    }
    /// La mancha mas pequena que se dibuja, en fraccion de la imagen.
    fn area_minima(self) -> f64 {
        match self {
            Detalle::Bajo => 0.0015,
            Detalle::Medio => 0.0004,
            Detalle::Alto => 0.00015,
        }
    }
    /// Lo mas corto que se traza en «solo contornos», en pixeles.
    fn cadena_minima(self) -> usize {
        match self {
            Detalle::Bajo => 14,
            Detalle::Medio => 9,
            Detalle::Alto => 6,
        }
    }
    /// Cuantos elementos como mucho.
    pub fn tope(self) -> usize {
        match self {
            Detalle::Bajo => 40,
            Detalle::Medio => 90,
            Detalle::Alto => 160,
        }
    }
}

/// Lo que elige el modelo.
#[derive(Debug, Clone, PartialEq)]
pub struct Parametros {
    pub detalle: Detalle,
    /// Manchas de color (y lineas del color de la imagen); sin ello, todo en
    /// negro.
    pub colores: bool,
    /// Solo las lineas de los bordes, sin rellenos.
    pub solo_contornos: bool,
    /// Un trozo de la imagen, en fracciones `[x, y, ancho, alto]` de 0 a 1.
    pub zona: Option<[f64; 4]>,
    /// Que adjunto calcar; `None`: la ultima imagen de la conversacion.
    pub imagen: Option<String>,
}

/// **Lo que el modelo eligio sin que se lo pidieran, corregido con el
/// pedido.** Medido el 14-oct: a «dibuja este perrito de la imagen adjunta»
/// el 2B pide `solo_contornos` y sin colores, y el perrito sale en seis rayas
/// negras. Solo contornos si el pedido habla de contornos, bordes, lineas o
/// silueta; en blanco y negro solo si lo dice.
pub fn ajustar_al_pedido(p: &mut Parametros, pedido: &str) {
    let q = crate::acciones::normalizar(pedido);
    let dice = |raices: &[&str]| raices.iter().any(|r| q.contains(r));
    if p.solo_contornos
        && !dice(&[
            "contorno",
            "borde",
            "linea",
            "silueta",
            "trazo",
            "perfil",
            "solo las rayas",
        ])
    {
        p.solo_contornos = false;
    }
    if !p.colores
        && !dice(&[
            "blanco y negro",
            "sin color",
            "en negro",
            "monocrom",
            "gris",
        ])
    {
        p.colores = true;
    }
}

impl Default for Parametros {
    fn default() -> Self {
        Parametros {
            detalle: Detalle::Medio,
            colores: true,
            solo_contornos: false,
            zona: None,
            imagen: None,
        }
    }
}

/// Un elemento calcado, ya en coordenadas del lienzo de 800x600.
#[derive(Debug, Clone, PartialEq)]
pub enum Trazo {
    /// Una mancha rellena con sus agujeros.
    Mancha {
        contorno: Vec<(f64, f64)>,
        huecos: Vec<Vec<(f64, f64)>>,
        color: String,
    },
    /// Una linea (abierta o cerrada) del borde.
    Linea {
        puntos: Vec<(f64, f64)>,
        cerrada: bool,
        color: String,
    },
    /// El fondo, si no era blanco: un rectangulo lleno detras de todo.
    Fondo { caja: [f64; 4], color: String },
    /// Una raya fina del dibujo, por su linea central, con su grosor (en
    /// el lienzo).
    Raya {
        puntos: Vec<(f64, f64)>,
        color: String,
        grosor: f64,
    },
}

/// El resultado y como volver a la imagen original (para las pruebas y para
/// quien quiera colocarlo encima de la foto).
#[derive(Debug, Clone, PartialEq)]
pub struct Calcado {
    pub trazos: Vec<Trazo>,
    /// Lienzo = imagen original * `escala` + `desplazamiento`.
    pub escala: f64,
    pub desplazamiento: (f64, f64),
}

/// **Calca** una imagen RGBA (`ancho * alto * 4` bytes; lo transparente
/// cuenta como blanco). Nunca falla: una imagen vacia o rara da pocos o
/// ningun trazo.
pub fn calcar(rgba: &[u8], ancho: u32, alto: u32, p: &Parametros) -> Calcado {
    let vacio = Calcado {
        trazos: Vec::new(),
        escala: 1.0,
        desplazamiento: (0.0, 0.0),
    };
    if ancho == 0 || alto == 0 || rgba.len() < (ancho as usize * alto as usize * 4) {
        return vacio;
    }
    // 1. Zona y reduccion.
    let (zx, zy, zw, zh) = match p.zona {
        Some([x, y, w, h]) if w > 0.0 && h > 0.0 => {
            let x0 = (x.clamp(0.0, 1.0) * ancho as f64) as u32;
            let y0 = (y.clamp(0.0, 1.0) * alto as f64) as u32;
            let x1 = (((x + w).clamp(0.0, 1.0)) * ancho as f64).ceil() as u32;
            let y1 = (((y + h).clamp(0.0, 1.0)) * alto as f64).ceil() as u32;
            (
                x0,
                y0,
                x1.saturating_sub(x0).max(1),
                y1.saturating_sub(y0).max(1),
            )
        }
        _ => (0, 0, ancho, alto),
    };
    let zw = zw.min(ancho - zx.min(ancho - 1));
    let zh = zh.min(alto - zy.min(alto - 1));
    let lado = p.detalle.lado();
    let f = (lado as f64 / zw.max(zh) as f64).min(1.0);
    let (w, h) = (
        ((zw as f64 * f).round() as usize).max(1),
        ((zh as f64 * f).round() as usize).max(1),
    );
    // Las rayas finas (1-3 px) se sacan antes, a toda resolucion, por su
    // linea central: reducida, una raya de un pixel se volvia una mancha
    // borrosa o desaparecia (15-oct, la lamina de geometria). Lo que queda
    // (rellenos, manchas) se calca como siempre, sin ellas.
    let (limpia, finas) = if p.solo_contornos {
        (None, Vec::new())
    } else {
        rayas_finas(
            rgba,
            ancho as usize,
            (zx as usize, zy as usize, zw as usize, zh as usize),
        )
    };
    let img = reducir(
        limpia.as_deref().unwrap_or(rgba),
        ancho as usize,
        (zx as usize, zy as usize, zw as usize, zh as usize),
        w,
        h,
    );

    // 2. A coordenadas del lienzo: cabe en 800x600 con margen y centrada.
    let margen = 20.0;
    let esc_red =
        ((ANCHO_LIENZO - 2.0 * margen) / w as f64).min((ALTO_LIENZO - 2.0 * margen) / h as f64);
    let dx = (ANCHO_LIENZO - w as f64 * esc_red) / 2.0;
    let dy = (ALTO_LIENZO - h as f64 * esc_red) / 2.0;
    let a_lienzo = |(x, y): (f64, f64)| (x * esc_red + dx, y * esc_red + dy);
    // De la imagen original al lienzo (zona y reduccion incluidas).
    let sx = w as f64 / zw as f64;
    let escala = sx * esc_red;
    let desplazamiento = (dx - zx as f64 * escala, dy - zy as f64 * escala);

    // Las curvas se aplanan para que no se aparten mas de un cuarto de pixel
    // del lienzo (a zoom 1): no se ve la quebrada y una recta sigue en dos
    // puntos.
    let plano = PLANO_EN_PANTALLA / esc_red;
    let mut trazos = if p.solo_contornos {
        contornos(&img, w, h, p, plano)
    } else {
        manchas(&img, w, h, p, plano)
    };
    // 3. Tope: los mas grandes primero (y el fondo, si lo hay, delante).
    let tope = p.detalle.tope();
    if trazos.len() > tope {
        let fondo = matches!(trazos.first(), Some(Trazo::Fondo { .. }));
        let resto = if fondo {
            trazos.split_off(1)
        } else {
            std::mem::take(&mut trazos)
        };
        let mut con_tam: Vec<(f64, Trazo)> = resto.into_iter().map(|t| (tamano(&t), t)).collect();
        con_tam.sort_by(|a, b| b.0.total_cmp(&a.0));
        con_tam.truncate(tope - usize::from(fondo));
        // Se conserva el orden de pintado (grande debajo).
        trazos.extend(con_tam.into_iter().map(|(_, t)| t));
    }
    for t in &mut trazos {
        match t {
            Trazo::Mancha {
                contorno, huecos, ..
            } => {
                contorno.iter_mut().for_each(|q| *q = a_lienzo(*q));
                huecos.iter_mut().flatten().for_each(|q| *q = a_lienzo(*q));
            }
            Trazo::Linea { puntos, .. } => puntos.iter_mut().for_each(|q| *q = a_lienzo(*q)),
            Trazo::Raya { .. } => {}
            Trazo::Fondo { caja, .. } => {
                let (x, y) = a_lienzo((0.0, 0.0));
                *caja = [x, y, w as f64 * esc_red, h as f64 * esc_red];
            }
        }
    }
    // Las rayas finas, encima de todo y ya en el lienzo.
    for (puntos, color, grosor) in finas {
        let puntos: Vec<(f64, f64)> = puntos
            .iter()
            .map(|&(x, y)| (x * escala + desplazamiento.0, y * escala + desplazamiento.1))
            .collect();
        trazos.push(Trazo::Raya {
            puntos,
            color,
            grosor: (grosor * escala).max(0.5),
        });
    }
    Calcado {
        trazos,
        escala,
        desplazamiento,
    }
}

/// Lo que «pesa» un trazo, para quedarse con los importantes.
fn tamano(t: &Trazo) -> f64 {
    match t {
        Trazo::Mancha { contorno, .. } => area(contorno).abs(),
        Trazo::Linea { puntos, .. } | Trazo::Raya { puntos, .. } => {
            puntos.windows(2).map(|p| dist(p[0], p[1])).sum::<f64>() * 3.0
        }
        Trazo::Fondo { .. } => f64::MAX,
    }
}

/// La zona reducida a `w x h` por **media de areas**, en RGB sobre blanco:
/// cada pixel de salida es la media de lo que cubre de la imagen, con los
/// pixeles de su borde pesados por la fraccion que caen dentro. Con cajas
/// enteras (de 2 o de 3 pixeles, segun cayera) un borde recto salia con un
/// vaiven de un cuarto de pixel, y el calco lo seguia.
fn reducir(
    rgba: &[u8],
    ancho: usize,
    (zx, zy, zw, zh): (usize, usize, usize, usize),
    w: usize,
    h: usize,
) -> Vec<[f32; 3]> {
    // Para cada pixel de salida, de que pixeles de la zona sale y con que
    // peso (en una direccion).
    let pesos = |n_fuera: usize, n: usize| -> Vec<Vec<(usize, f32)>> {
        let paso = n_fuera as f64 / n as f64;
        (0..n)
            .map(|i| {
                let (a, b) = (i as f64 * paso, ((i + 1) as f64 * paso).min(n_fuera as f64));
                let mut v = Vec::new();
                let mut k = a.floor() as usize;
                while (k as f64) < b && k < n_fuera {
                    let p = (b.min(k as f64 + 1.0) - a.max(k as f64)) as f32;
                    if p > 1e-6 {
                        v.push((k, p));
                    }
                    k += 1;
                }
                v
            })
            .collect()
    };
    let (px, py) = (pesos(zw, w), pesos(zh, h));
    let mut out = vec![[255f32; 3]; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut s = [0f32; 3];
            let mut n = 0f32;
            for &(yy, wy) in &py[y] {
                for &(xx, wx) in &px[x] {
                    let i = ((zy + yy) * ancho + zx + xx) * 4;
                    let Some(q) = rgba.get(i..i + 4) else {
                        continue;
                    };
                    let a = q[3] as f32 / 255.0;
                    let p = wx * wy;
                    for c in 0..3 {
                        s[c] += p * (q[c] as f32 * a + 255.0 * (1.0 - a));
                    }
                    n += p;
                }
            }
            if n > 0.0 {
                out[y * w + x] = [s[0] / n, s[1] / n, s[2] / n];
            }
        }
    }
    out
}

fn hex(c: [f32; 3]) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        c[0].round().clamp(0.0, 255.0) as u8,
        c[1].round().clamp(0.0, 255.0) as u8,
        c[2].round().clamp(0.0, 255.0) as u8
    )
}

fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

// ---------------------------------------------------------------------------
// Manchas de color

/// k-medias determinista: el primer centro es el color mas comun; los
/// siguientes, el pixel mas lejos de los que ya hay (asi un color pequeno
/// pero distinto, una puerta azul, tiene su centro).
fn k_medias(img: &[[f32; 3]], k: usize) -> (Vec<[f32; 3]>, Vec<u8>) {
    let paso = (img.len() / 20_000).max(1);
    let muestra: Vec<[f32; 3]> = img.iter().step_by(paso).copied().collect();
    // Histograma de 16 niveles por canal, con la media de cada cubo: los
    // centros salen de los cubos con bastantes pixeles. Asi un color
    // concentrado (una raya verde) gana a los de mezcla de los bordes, que
    // son muchos pero repartidos en cubos de pocos pixeles cada uno.
    let mut cubos: HashMap<[u8; 3], [f64; 4]> = HashMap::new();
    for c in &muestra {
        let e = cubos
            .entry([
                (c[0] / 16.0) as u8,
                (c[1] / 16.0) as u8,
                (c[2] / 16.0) as u8,
            ])
            .or_default();
        for j in 0..3 {
            e[j] += c[j] as f64;
        }
        e[3] += 1.0;
    }
    let minimo = (muestra.len() as f64 / 400.0).max(3.0);
    let mut candidatos: Vec<([f32; 3], f64)> = cubos
        .values()
        .map(|s| {
            (
                [
                    (s[0] / s[3]) as f32,
                    (s[1] / s[3]) as f32,
                    (s[2] / s[3]) as f32,
                ],
                s[3],
            )
        })
        .collect();
    candidatos.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0[0].total_cmp(&b.0[0])));
    let mut centros = vec![candidatos.first().map(|c| c.0).unwrap_or([255.0; 3])];
    let fuertes: Vec<[f32; 3]> = candidatos
        .iter()
        .filter(|c| c.1 >= minimo)
        .map(|c| c.0)
        .collect();
    while centros.len() < k {
        let Some((mejor, dist)) = fuertes
            .iter()
            .map(|c| {
                (
                    c,
                    centros.iter().map(|z| d2(*c, *z)).fold(f32::MAX, f32::min),
                )
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
        else {
            break;
        };
        // Ya no hay colores distintos (menos de ~20 de distancia).
        if dist < 400.0 {
            break;
        }
        centros.push(*mejor);
    }
    for _ in 0..8 {
        let mut suma = vec![[0f64; 4]; centros.len()];
        for c in &muestra {
            let i = cercano(&centros, *c);
            for j in 0..3 {
                suma[i][j] += c[j] as f64;
            }
            suma[i][3] += 1.0;
        }
        for (z, s) in centros.iter_mut().zip(&suma) {
            if s[3] > 0.0 {
                *z = [
                    (s[0] / s[3]) as f32,
                    (s[1] / s[3]) as f32,
                    (s[2] / s[3]) as f32,
                ];
            }
        }
    }
    let etiquetas = img.iter().map(|c| cercano(&centros, *c) as u8).collect();
    (centros, etiquetas)
}

fn cercano(centros: &[[f32; 3]], c: [f32; 3]) -> usize {
    let mut mejor = 0;
    let mut md = f32::MAX;
    for (i, z) in centros.iter().enumerate() {
        let d = d2(c, *z);
        if d < md {
            md = d;
            mejor = i;
        }
    }
    mejor
}

/// Filtro de mayoria 3x3: quita el «sal y pimienta» de los bordes
/// suavizados (antialias) sin mover los bordes de verdad.
fn mayoria(et: &[u8], w: usize, h: usize, k: usize) -> Vec<u8> {
    let mut out = et.to_vec();
    let mut cuenta = vec![0u8; k];
    for y in 0..h {
        for x in 0..w {
            cuenta.iter_mut().for_each(|c| *c = 0);
            for yy in y.saturating_sub(1)..(y + 2).min(h) {
                for xx in x.saturating_sub(1)..(x + 2).min(w) {
                    cuenta[et[yy * w + xx] as usize] += 1;
                }
            }
            let propio = et[y * w + x] as usize;
            let (mejor, n) = cuenta
                .iter()
                .enumerate()
                .max_by_key(|(_, n)| **n)
                .map(|(i, n)| (i, *n))
                .unwrap();
            if n >= 6 && mejor != propio {
                out[y * w + x] = mejor as u8;
            }
        }
    }
    out
}

/// **Quita los colores de mezcla**: al reducir la imagen, el borde entre
/// rojo y blanco sale rosa, y k-medias le da su propio color; calcado, eso es
/// un anillo rosa alrededor de cada figura. Un grupo cuyo color esta entre
/// otros dos y que es casi todo borde (sus pixeles tocan otro color) se
/// reparte entre esos dos. Una mancha morada de verdad, maciza, no es borde
/// y se queda.
fn fusionar_mezclas(centros: &[[f32; 3]], et: &mut [u8], w: usize, h: usize) {
    let k = centros.len();
    for _ in 0..k {
        let mut cuenta = vec![0usize; k];
        let mut de_borde = vec![0usize; k];
        for y in 0..h {
            for x in 0..w {
                let e = et[y * w + x];
                cuenta[e as usize] += 1;
                let distinto = (x > 0 && et[y * w + x - 1] != e)
                    || (x + 1 < w && et[y * w + x + 1] != e)
                    || (y > 0 && et[(y - 1) * w + x] != e)
                    || (y + 1 < h && et[(y + 1) * w + x] != e);
                if distinto {
                    de_borde[e as usize] += 1;
                }
            }
        }
        let mut cambio = false;
        for c in 0..k {
            if cuenta[c] == 0 || (de_borde[c] as f64) < 0.5 * cuenta[c] as f64 {
                continue;
            }
            // Los dos colores entre los que esta.
            let mut mejor: Option<(usize, usize, f32)> = None;
            for a in 0..k {
                for b in a + 1..k {
                    if a == c || b == c || cuenta[a] == 0 || cuenta[b] == 0 {
                        continue;
                    }
                    let (pa, pb, pc) = (centros[a], centros[b], centros[c]);
                    let ab = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
                    let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
                    if l2 <= 0.0 {
                        continue;
                    }
                    let t = ((pc[0] - pa[0]) * ab[0]
                        + (pc[1] - pa[1]) * ab[1]
                        + (pc[2] - pa[2]) * ab[2])
                        / l2;
                    if !(0.05..=0.95).contains(&t) {
                        continue;
                    }
                    let q = [pa[0] + t * ab[0], pa[1] + t * ab[1], pa[2] + t * ab[2]];
                    let d = d2(pc, q);
                    if d < 45.0 * 45.0 && mejor.is_none_or(|m| d < m.2) {
                        mejor = Some((a, b, d));
                    }
                }
            }
            if let Some((a, b, _)) = mejor {
                // Cada pixel, al de los dos que mas tenga alrededor; si no
                // toca ninguno, al color mas cercano de los dos.
                let copia = et.to_vec();
                for y in 0..h {
                    for x in 0..w {
                        let i = y * w + x;
                        if copia[i] as usize != c {
                            continue;
                        }
                        let mut na = 0;
                        let mut nb = 0;
                        for yy in y.saturating_sub(1)..(y + 2).min(h) {
                            for xx in x.saturating_sub(1)..(x + 2).min(w) {
                                let v = copia[yy * w + xx] as usize;
                                na += usize::from(v == a);
                                nb += usize::from(v == b);
                            }
                        }
                        et[i] = if na > nb {
                            a as u8
                        } else if nb > na {
                            b as u8
                        } else if d2(centros[c], centros[a]) <= d2(centros[c], centros[b]) {
                            a as u8
                        } else {
                            b as u8
                        };
                    }
                }
                cambio = true;
                break;
            }
        }
        if !cambio {
            break;
        }
    }
}

/// La caja de una mancha en pixeles: `(x0, y0, x1, y1)`, incluidos.
type CajaPx = (usize, usize, usize, usize);

/// Las manchas conexas (4-vecindad) de la etiqueta `l`: sus pixeles y su
/// caja `(x0, y0, x1, y1)` (x1, y1 incluidos).
fn componentes(et: &[u8], w: usize, h: usize, l: u8) -> Vec<(Vec<usize>, CajaPx)> {
    let mut visto = vec![false; w * h];
    let mut salida = Vec::new();
    for inicio in 0..w * h {
        if visto[inicio] || et[inicio] != l {
            continue;
        }
        let mut pila = vec![inicio];
        visto[inicio] = true;
        let mut pixeles = Vec::new();
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        while let Some(i) = pila.pop() {
            pixeles.push(i);
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            let vecinos = [
                (x > 0).then(|| i - 1),
                (x + 1 < w).then(|| i + 1),
                (y > 0).then(|| i - w),
                (y + 1 < h).then(|| i + w),
            ];
            for j in vecinos.into_iter().flatten() {
                if !visto[j] && et[j] == l {
                    visto[j] = true;
                    pila.push(j);
                }
            }
        }
        salida.push((pixeles, (x0, y0, x1, y1)));
    }
    salida
}

fn manchas(img: &[[f32; 3]], w: usize, h: usize, p: &Parametros, plano: f64) -> Vec<Trazo> {
    let k = if p.colores { p.detalle.colores() } else { 2 };
    let (centros, et) = k_medias(img, k);
    let mut et = mayoria(&et, w, h, centros.len());
    fusionar_mezclas(&centros, &mut et, w, h);
    // El fondo: el color que mas toca el borde.
    let mut borde = vec![0usize; centros.len()];
    for x in 0..w {
        borde[et[x] as usize] += 1;
        borde[et[(h - 1) * w + x] as usize] += 1;
    }
    for y in 0..h {
        borde[et[y * w] as usize] += 1;
        borde[et[y * w + w - 1] as usize] += 1;
    }
    let fondo = borde
        .iter()
        .enumerate()
        .max_by_key(|(_, n)| **n)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let mut trazos = Vec::new();
    let blanco = centros[fondo].iter().all(|c| *c > 225.0);
    if p.colores && !blanco {
        trazos.push(Trazo::Fondo {
            caja: [0.0; 4],
            color: hex(centros[fondo]),
        });
    }
    let area_min = (p.detalle.area_minima() * (w * h) as f64).max(4.0);
    let tol = p.detalle.tolerancia();
    // Todas las manchas de todos los colores (menos el fondo) y su turno de
    // pintado: la grande debajo.
    let mut lista: Vec<(usize, Vec<usize>, CajaPx)> = Vec::new();
    for l in 0..centros.len() {
        if l != fondo {
            lista.extend(
                componentes(&et, w, h, l as u8)
                    .into_iter()
                    .map(|(px, caja)| (l, px, caja)),
            );
        }
    }
    lista.sort_by_key(|l| std::cmp::Reverse(l.1.len()));
    let mut turno = vec![usize::MAX; w * h];
    for (k, (_, pixeles, _)) in lista.iter().enumerate() {
        for &i in pixeles {
            turno[i] = k;
        }
    }
    let mut todas: Vec<(usize, Trazo)> = Vec::new();
    // Mancha a mancha, y solo las grandes: el ruido (miles de manchas de un
    // pixel) no llega a recorrerse.
    for (k, (l, pixeles, (x0, y0, x1, y1))) in lista.iter().enumerate() {
        if (pixeles.len() as f64) < area_min {
            break;
        }
        let l = *l;
        let color = if p.colores {
            hex(centros[l])
        } else {
            "#1e1e1e".to_string()
        };
        // La caja, con sitio para meterse SOLAPE pixeles bajo las vecinas.
        let (x0, y0) = (x0.saturating_sub(SOLAPE), y0.saturating_sub(SOLAPE));
        let (x1, y1) = ((x1 + SOLAPE).min(w - 1), (y1 + SOLAPE).min(h - 1));
        let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut mascara = vec![false; bw * bh];
        for &i in pixeles {
            mascara[(i / w - y0) * bw + (i % w - x0)] = true;
        }
        // Por debajo solo de lo que va encima, y sin asomarse al otro lado:
        // ni a SOLAPE pixeles del fondo ni de lo que se pinto antes (que quedaria
        // tapado por el solape en vez de por la de encima).
        let encima = |i: usize| et[i] as usize != fondo && turno[i] > k;
        let debajo = solape(&mascara, bw, bh, |x, y| {
            let (gx, gy) = (x + x0, y + y0);
            encima(gy * w + gx)
                && (gy.saturating_sub(SOLAPE)..(gy + SOLAPE + 1).min(h)).all(|yy| {
                    (gx.saturating_sub(SOLAPE)..(gx + SOLAPE + 1).min(w)).all(|xx| {
                        let j = yy * w + xx;
                        turno[j] == k || encima(j)
                    })
                })
        });
        let mover = |v: &[(f64, f64)]| -> Vec<(f64, f64)> {
            v.iter()
                .map(|&(x, y)| (x + x0 as f64, y + y0 as f64))
                .collect()
        };
        let (campo, gw, gh) = campo_de_color(
            img,
            w,
            &et,
            &centros,
            l,
            (x0, y0, bw, bh),
            &mascara,
            &debajo,
        );
        let entera: Vec<bool> = mascara.iter().zip(&debajo).map(|(a, b)| *a || *b).collect();
        for (contorno, huecos) in regiones_de_campo(&campo, gw, gh, area_min) {
            let contorno = curva(&mover(&contorno), true, tol, plano);
            if contorno.len() < 3 {
                continue;
            }
            // Solo se cortan los agujeros por los que asoma el fondo: los
            // que tapa otra mancha (que se pinta encima, por ser menor)
            // dejaban rendijas blancas entre los dos colores.
            let huecos = huecos
                .iter()
                .filter(|hq| {
                    hueco_de_fondo(hq, &entera, bw, bh, |x, y| {
                        et[(y + y0) * w + (x + x0)] as usize == fondo
                    })
                })
                .map(|hq| curva(&mover(hq), true, tol, plano))
                .filter(|hq| hq.len() >= 3)
                .collect();
            todas.push((
                k,
                Trazo::Mancha {
                    contorno,
                    huecos,
                    color: color.clone(),
                },
            ));
        }
    }
    // Lo grande debajo (ya van en su turno).
    trazos.extend(todas.into_iter().map(|(_, t)| t));
    trazos
}

/// Cuanto se mete cada mancha por debajo de las que se pintan encima.
const SOLAPE: usize = 2;

/// **Sin rendijas entre manchas vecinas.** Cada mancha se calca por su
/// cuenta, y entre dos vecinas el ajuste de una puede irse medio pixel
/// para un lado y el de la otra para el otro: por la rendija asomaba el
/// papel (medido: 1,2 ‰ del perrito en blanco). Como las manchas se pintan
/// de la grande a la pequena, la de debajo se alarga [`SOLAPE`] pixeles por
/// dentro de las que van encima (`encima(x, y)`): su borde con ellas queda
/// tapado y el que se ve es el de la de encima. Con el fondo no se solapa.
fn solape(
    mascara: &[bool],
    bw: usize,
    bh: usize,
    encima: impl Fn(usize, usize) -> bool,
) -> Vec<bool> {
    // Dilatacion cuadrada de SOLAPE (en x y luego en y).
    let mut fila = vec![false; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            let (a, b) = (x.saturating_sub(SOLAPE), (x + SOLAPE).min(bw - 1));
            fila[y * bw + x] = (a..=b).any(|xx| mascara[y * bw + xx]);
        }
    }
    let mut salida = vec![false; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            let i = y * bw + x;
            if mascara[i] {
                continue;
            }
            let (a, b) = (y.saturating_sub(SOLAPE), (y + SOLAPE).min(bh - 1));
            salida[i] = (a..=b).any(|yy| fila[yy * bw + x]) && encima(x, y);
        }
    }
    salida
}

/// Una region: su borde exterior y sus agujeros, en coordenadas continuas.
pub type Region = (Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>);

/// Los contornos **sub-pixel** de una mascara (`trazado`: la mascara
/// suavizada y *marching squares* interpolado en 0,5): cada anillo es el
/// borde exterior de una mancha o el de uno de sus agujeros. Devuelve
/// (contorno, agujeros) de las manchas de al menos `area_min` pixeles, en
/// coordenadas continuas (el pixel (x, y) ocupa [x, x+1] x [y, y+1]).
pub fn regiones(mascara: &[bool], w: usize, h: usize, area_min: f64) -> Vec<Region> {
    if w == 0 || h == 0 || mascara.len() < w * h {
        return Vec::new();
    }
    let (campo, gw, gh) = trazado::campo_suave(mascara, w, h, BORDE);
    regiones_de_campo(&campo, gw, gh, area_min)
}

/// Los ceros alrededor del campo (para que todo anillo se cierre).
const BORDE: usize = 2;

/// Como [`regiones`], de un campo ya hecho (con [`BORDE`] alrededor):
/// isolineas de 0,5, separadas en bordes exteriores y agujeros.
fn regiones_de_campo(campo: &[f32], gw: usize, gh: usize, area_min: f64) -> Vec<Region> {
    // El nodo (i, j) de la rejilla es el centro del pixel (i - 2, j - 2).
    let a_pixel = BORDE as f64 - 0.5;
    let mut exteriores = Vec::new();
    let mut agujeros = Vec::new();
    for anillo in trazado::isolineas(campo, gw, gh, 0.5) {
        let anillo: Vec<(f64, f64)> = anillo
            .into_iter()
            .map(|(x, y)| (x - a_pixel, y - a_pixel))
            .collect();
        let a = area(&anillo);
        if a.abs() < area_min {
            continue;
        }
        if a < 0.0 {
            exteriores.push(anillo);
        } else {
            agujeros.push(anillo);
        }
    }
    let mut salida: Vec<Region> = exteriores.into_iter().map(|e| (e, Vec::new())).collect();
    for a in agujeros {
        // Al exterior mas pequeno que lo contiene.
        let p = a[0];
        if let Some((_, (_, hs))) = salida
            .iter_mut()
            .map(|r| (area(&r.0).abs(), r))
            .filter(|(_, r)| dentro(&r.0, p))
            .min_by(|x, y| x.0.total_cmp(&y.0))
        {
            hs.push(a);
        }
    }
    salida
}

/// **El campo de una mancha sacado del color**, para que su borde caiga
/// donde cae en la imagen y no en la frontera de pixeles. En cada pixel del
/// borde, cuanto de su color es el de la mancha y cuanto el del vecino de
/// fuera (la proyeccion de su color en el segmento entre los dos centros de
/// k-medias): en un borde con antialias es la fraccion de pixel que cubre
/// la mancha, y en un degradado partido en bandas, el punto medio entre
/// las dos bandas (una frontera lisa en vez del zigzag de las etiquetas).
///
/// Lo de dentro de la mascara empieza por encima de 0,5 y lo de fuera por
/// debajo: la forma (y sus agujeros) son los de la mascara y el color solo
/// mueve el borde dentro del pixel. Luego un suavizado `[1 2 1]` quita el
/// temblor de pixel a pixel (en una foto, el color tiembla), y con el se
/// van los salientes de un pixel que dejan las etiquetas.
#[allow(clippy::too_many_arguments)]
fn campo_de_color(
    img: &[[f32; 3]],
    w: usize,
    et: &[u8],
    centros: &[[f32; 3]],
    l: usize,
    (x0, y0, bw, bh): (usize, usize, usize, usize),
    mascara: &[bool],
    debajo: &[bool],
) -> (Vec<f32>, usize, usize) {
    let h = et.len() / w;
    let (gw, gh) = (bw + 2 * BORDE, bh + 2 * BORDE);
    let mut campo = vec![0f32; gw * gh];
    let proyeccion = |c: [f32; 3], fuera: [f32; 3]| -> f32 {
        let d = [
            centros[l][0] - fuera[0],
            centros[l][1] - fuera[1],
            centros[l][2] - fuera[2],
        ];
        let n = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        if n < 1e-3 {
            return 0.5;
        }
        (((c[0] - fuera[0]) * d[0] + (c[1] - fuera[1]) * d[1] + (c[2] - fuera[2]) * d[2]) / n)
            .clamp(0.0, 1.0)
    };
    let dentro_de = |m: &[bool], x: isize, y: isize| {
        x >= 0
            && y >= 0
            && (x as usize) < bw
            && (y as usize) < bh
            && m[y as usize * bw + x as usize]
    };
    let en_mascara = |x: isize, y: isize| dentro_de(mascara, x, y);
    let en_debajo = |x: isize, y: isize| dentro_de(debajo, x, y);
    let lados = [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)];
    // Dentro, de 0,5 para arriba; fuera, para abajo.
    const CASI: f32 = 1e-3;
    for y in 0..bh + 2 {
        for x in 0..bw + 2 {
            // (x, y) de la caja con un pixel de margen: (x - 1, y - 1).
            let (bx, by) = (x as isize - 1, y as isize - 1);
            let (ix, iy) = (x0 as isize + bx, y0 as isize + by);
            if ix < 0 || iy < 0 || ix as usize >= w || iy as usize >= h {
                continue;
            }
            let i = iy as usize * w + ix as usize;
            let v = if en_debajo(bx, by) {
                // Lo que se mete por debajo de las de encima: lleno.
                1.0
            } else if en_mascara(bx, by) {
                // El primer vecino de fuera (de los 4 de lado).
                let fuera = lados.iter().find_map(|&(dx, dy)| {
                    let (nx, ny) = (ix + dx, iy + dy);
                    let fuera = !en_mascara(bx + dx, by + dy) && !en_debajo(bx + dx, by + dy);
                    (fuera && nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h)
                        .then(|| et[ny as usize * w + nx as usize] as usize)
                });
                match fuera {
                    Some(k) if k != l => proyeccion(img[i], centros[k]).max(0.5 + CASI),
                    _ => 1.0,
                }
            } else {
                let k = et[i] as usize;
                // Junto a lo solapado, vacio sin mas: ese borde lo tapa la
                // mancha de encima.
                let junto_al_solape = lados.iter().any(|&(dx, dy)| en_debajo(bx + dx, by + dy));
                if k == l || junto_al_solape {
                    0.0
                } else {
                    proyeccion(img[i], centros[k]).min(0.5 - CASI)
                }
            };
            campo[(y + BORDE - 1) * gw + x + BORDE - 1] = v;
        }
    }
    // Suavizado [1 2 1] para el temblor de pixel a pixel. Sin volver a
    // forzar cada pixel a su lado de 0,5: medido en el perrito, forzarlo
    // devolvia los dientes de las etiquetas (115 vertices de mas de 45° y
    // un 30 % mas de giro, frente a 83 sin forzar), y en las figuras daba
    // lo mismo.
    let mut tmp = campo.clone();
    for y in 0..gh {
        for x in 1..gw - 1 {
            let i = y * gw + x;
            tmp[i] = 0.25 * campo[i - 1] + 0.5 * campo[i] + 0.25 * campo[i + 1];
        }
    }
    let mut suave = vec![0f32; gw * gh];
    // La primera y la ultima fila y columna se quedan en cero: todo anillo
    // se cierra.
    for y in 1..gh - 1 {
        for x in 1..gw - 1 {
            let i = y * gw + x;
            suave[i] = 0.25 * tmp[i - gw] + 0.5 * tmp[i] + 0.25 * tmp[i + gw];
        }
    }
    (suave, gw, gh)
}

/// Si por un agujero (en coordenadas de la caja de la mancha) asoma el
/// fondo: se mira la etiqueta de los pixeles de fuera de la mancha que
/// quedan dentro del agujero; con que la mitad sea fondo, se corta.
fn hueco_de_fondo(
    hueco: &[(f64, f64)],
    mascara: &[bool],
    bw: usize,
    bh: usize,
    es_fondo: impl Fn(usize, usize) -> bool,
) -> bool {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in hueco {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let (mut fondo, mut total) = (0usize, 0usize);
    for y in (y0.max(0.0) as usize)..(y1.ceil() as usize).min(bh) {
        for x in (x0.max(0.0) as usize)..(x1.ceil() as usize).min(bw) {
            if mascara[y * bw + x] || !dentro(hueco, (x as f64 + 0.5, y as f64 + 0.5)) {
                continue;
            }
            total += 1;
            fondo += usize::from(es_fondo(x, y));
        }
    }
    total > 0 && fondo * 2 >= total
}

/// Punto dentro de poligono (rayo hacia la derecha).
fn dentro(pol: &[(f64, f64)], (px, py): (f64, f64)) -> bool {
    let mut c = false;
    let n = pol.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = pol[i];
        let (xj, yj) = pol[j];
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            c = !c;
        }
        j = i;
    }
    c
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

// ---------------------------------------------------------------------------
// De contorno a curva

/// **El contorno hecho curva**: ajustado con rectas y Bezier cubicas que no
/// se apartan mas de `tol` (pixeles de la imagen reducida) y aplanado para
/// el lienzo con `plano` de tolerancia (tambien en pixeles de la imagen
/// reducida: el que llama lo saca de lo que mide un pixel de pantalla).
pub fn curva(p: &[(f64, f64)], cerrado: bool, tol: f64, plano: f64) -> Vec<(f64, f64)> {
    trazado::aplanar(&trazado::ajustar(p, cerrado, tol), cerrado, plano)
}

// ---------------------------------------------------------------------------
// Solo contornos: Canny y cadenas

fn contornos(img: &[[f32; 3]], w: usize, h: usize, p: &Parametros, plano: f64) -> Vec<Trazo> {
    if w < 3 || h < 3 {
        return Vec::new();
    }
    let gris: Vec<f32> = img
        .iter()
        .map(|c| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2])
        .collect();
    let suave = gauss(&gris, w, h);
    // Sobel.
    let mut mag = vec![0f32; w * h];
    let mut dir = vec![0u8; w * h];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let g = |dx: isize, dy: isize| {
                suave[((y as isize + dy) as usize) * w + (x as isize + dx) as usize]
            };
            let gx = g(1, -1) + 2.0 * g(1, 0) + g(1, 1) - g(-1, -1) - 2.0 * g(-1, 0) - g(-1, 1);
            let gy = g(-1, 1) + 2.0 * g(0, 1) + g(1, 1) - g(-1, -1) - 2.0 * g(0, -1) - g(1, -1);
            mag[y * w + x] = (gx * gx + gy * gy).sqrt();
            let a = gy.atan2(gx).to_degrees().rem_euclid(180.0);
            dir[y * w + x] = if !(22.5..157.5).contains(&a) {
                0
            } else if a < 67.5 {
                1
            } else if a < 112.5 {
                2
            } else {
                3
            };
        }
    }
    // Supresion de no maximos, y donde cae de verdad el borde dentro del
    // pixel (Devernay, «A non-maximum suppression method for edge detection
    // with sub-pixel accuracy», 1995: la parabola por la magnitud del
    // gradiente en el pixel y sus dos vecinos en la direccion del
    // gradiente). Sin esto la cadena va de centro a centro de pixel: en
    // escalera.
    let mut nms = vec![0f32; w * h];
    let mut fino = vec![(0f32, 0f32); w * h];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let ((a, b), paso) = match dir[i] {
                0 => ((i - 1, i + 1), (1.0, 0.0)),
                1 => ((i - w - 1, i + w + 1), (1.0, 1.0)),
                2 => ((i - w, i + w), (0.0, 1.0)),
                _ => ((i - w + 1, i + w - 1), (-1.0, 1.0)),
            };
            if mag[i] >= mag[a] && mag[i] >= mag[b] {
                nms[i] = mag[i];
                let den = mag[a] - 2.0 * mag[i] + mag[b];
                let eta = if den < -1e-6 {
                    ((mag[a] - mag[b]) / (2.0 * den)).clamp(-0.5, 0.5)
                } else {
                    0.0
                };
                fino[i] = (eta * paso.0, eta * paso.1);
            }
        }
    }
    // Umbrales de la propia imagen.
    let maximo = nms.iter().copied().fold(0.0, f32::max);
    if maximo < 8.0 {
        return Vec::new();
    }
    let mut vivos: Vec<f32> = nms.iter().copied().filter(|m| *m > maximo * 0.02).collect();
    vivos.sort_by(|a, b| a.total_cmp(b));
    let percentil = match p.detalle {
        Detalle::Bajo => 0.85,
        Detalle::Medio => 0.7,
        Detalle::Alto => 0.55,
    };
    // Tope a media altura del maximo: en un dibujo limpio el percentil cae
    // en los bordes mas fuertes y dejaba fuera figuras enteras de menos
    // contraste (un circulo rojo junto a un cuadrado azul).
    let alto = vivos[((vivos.len() - 1) as f32 * percentil) as usize]
        .min(maximo * 0.45)
        .max(maximo * 0.1)
        .max(20.0);
    let bajo = alto * 0.4;
    // Histeresis.
    let mut borde = vec![false; w * h];
    let mut pila: Vec<usize> = (0..w * h).filter(|&i| nms[i] >= alto).collect();
    for &i in &pila {
        borde[i] = true;
    }
    while let Some(i) = pila.pop() {
        let (x, y) = (i % w, i / w);
        for yy in y.saturating_sub(1)..(y + 2).min(h) {
            for xx in x.saturating_sub(1)..(x + 2).min(w) {
                let j = yy * w + xx;
                if !borde[j] && nms[j] >= bajo {
                    borde[j] = true;
                    pila.push(j);
                }
            }
        }
    }
    // Fondo (para el color de las lineas): la media del borde de la imagen.
    let fondo = {
        let mut s = [0f32; 3];
        let mut n = 0.0;
        for x in 0..w {
            for c in [img[x], img[(h - 1) * w + x]] {
                (0..3).for_each(|k| s[k] += c[k]);
                n += 1.0;
            }
        }
        [s[0] / n, s[1] / n, s[2] / n]
    };
    let tol = p.detalle.tolerancia();
    let mut salida = Vec::new();
    for cadena in cadenas(&mut borde, w, h) {
        if cadena.len() < p.detalle.cadena_minima() {
            continue;
        }
        let cerrada = cadena.len() > 4 && {
            let (a, b) = (cadena[0], cadena[cadena.len() - 1]);
            (a.0 as i64 - b.0 as i64).abs() <= 1 && (a.1 as i64 - b.1 as i64).abs() <= 1
        };
        let color = if p.colores {
            // El lado de la linea que no es fondo, de media.
            let mut s = [0f32; 3];
            let mut n = 0.0;
            for &(x, y) in cadena.iter().step_by(2) {
                let mut mejor = img[y * w + x];
                let mut md = d2(mejor, fondo);
                for (dx, dy) in [(-2i64, 0i64), (2, 0), (0, -2), (0, 2)] {
                    let (xx, yy) = (x as i64 + dx, y as i64 + dy);
                    if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < h {
                        let c = img[yy as usize * w + xx as usize];
                        if d2(c, fondo) > md {
                            md = d2(c, fondo);
                            mejor = c;
                        }
                    }
                }
                (0..3).for_each(|k| s[k] += mejor[k]);
                n += 1.0;
            }
            hex([s[0] / n, s[1] / n, s[2] / n])
        } else {
            "#1e1e1e".to_string()
        };
        // Cada pixel, en el punto sub-pixel de su borde.
        let puntos: Vec<(f64, f64)> = cadena
            .iter()
            .map(|&(x, y)| {
                let d = fino[y * w + x];
                (x as f64 + 0.5 + d.0 as f64, y as f64 + 0.5 + d.1 as f64)
            })
            .collect();
        let mut puntos = curva(&puntos, cerrada, tol, plano);
        if cerrada && puntos.len() >= 3 {
            puntos.push(puntos[0]);
        }
        if puntos.len() >= 2 {
            salida.push(Trazo::Linea {
                puntos,
                cerrada,
                color,
            });
        }
    }
    salida
}

fn gauss(g: &[f32], w: usize, h: usize) -> Vec<f32> {
    const K: [f32; 5] = [1.0 / 16.0, 4.0 / 16.0, 6.0 / 16.0, 4.0 / 16.0, 1.0 / 16.0];
    let mut tmp = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for (k, kv) in K.iter().enumerate() {
                let xx = (x as isize + k as isize - 2).clamp(0, w as isize - 1) as usize;
                s += kv * g[y * w + xx];
            }
            tmp[y * w + x] = s;
        }
    }
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for (k, kv) in K.iter().enumerate() {
                let yy = (y as isize + k as isize - 2).clamp(0, h as isize - 1) as usize;
                s += kv * tmp[yy * w + x];
            }
            out[y * w + x] = s;
        }
    }
    out
}

/// Las cadenas de pixeles de borde: primero desde los extremos (las
/// abiertas), luego los lazos que quedan. Se consumen los pixeles.
fn cadenas(borde: &mut [bool], w: usize, h: usize) -> Vec<Vec<(usize, usize)>> {
    let vecinos = |borde: &[bool], x: usize, y: usize| -> Vec<(usize, usize)> {
        let mut v = Vec::with_capacity(8);
        // Primero los 4 de lado: la cadena sale mas fiel.
        for (dx, dy) in [
            (1i64, 0i64),
            (0, 1),
            (-1, 0),
            (0, -1),
            (1, 1),
            (-1, 1),
            (-1, -1),
            (1, -1),
        ] {
            let (xx, yy) = (x as i64 + dx, y as i64 + dy);
            if xx >= 0
                && yy >= 0
                && (xx as usize) < w
                && (yy as usize) < h
                && borde[yy as usize * w + xx as usize]
            {
                v.push((xx as usize, yy as usize));
            }
        }
        v
    };
    let mut salida = Vec::new();
    for pasada in 0..2 {
        for y in 0..h {
            for x in 0..w {
                if !borde[y * w + x] {
                    continue;
                }
                if pasada == 0 && vecinos(borde, x, y).len() != 1 {
                    continue;
                }
                let mut cadena = vec![(x, y)];
                borde[y * w + x] = false;
                let (mut cx, mut cy) = (x, y);
                while let Some(&(nx, ny)) = vecinos(borde, cx, cy).first() {
                    borde[ny * w + nx] = false;
                    cadena.push((nx, ny));
                    (cx, cy) = (nx, ny);
                }
                salida.push(cadena);
            }
        }
    }
    salida
}

// ---------------------------------------------------------------------------
// Rayas finas, a toda resolucion

/// Lo mas grande (en pixeles) que se mira a toda resolucion buscando rayas
/// finas; por encima, se reduce primero a esto.
const LADO_FINAS: usize = 1600;

/// **Las rayas finas** de la zona `(zx, zy, zw, zh)`: la tinta que no
/// aguanta una erosion de 2 px (lo de 1-4 px de ancho), por su linea
/// central (el esqueleto, alisado con las curvas de `trazado`), con su color
/// y su grosor. Devuelve tambien la imagen sin ellas (pintadas del color
/// del papel) para calcar el resto. En coordenadas de la imagen original.
#[allow(clippy::type_complexity)]
fn rayas_finas(
    rgba: &[u8],
    ancho: usize,
    (zx, zy, zw, zh): (usize, usize, usize, usize),
) -> (Option<Vec<u8>>, Vec<(Vec<(f64, f64)>, String, f64)>) {
    use crate::diagrama::mascara;
    if zw.max(zh) > LADO_FINAS || zw < 8 || zh < 8 {
        return (None, Vec::new());
    }
    let mut sub = vec![0u8; zw * zh * 4];
    for y in 0..zh {
        let o = ((zy + y) * ancho + zx) * 4;
        let Some(fila) = rgba.get(o..o + zw * 4) else {
            return (None, Vec::new());
        };
        sub[y * zw * 4..(y + 1) * zw * 4].copy_from_slice(fila);
    }
    let img = mascara::Img::de_rgba(&sub, zw, zh);
    let papel = mascara::fondo(&img);
    let dpap: Vec<f32> = img
        .px
        .iter()
        .map(|&c| mascara::dist_color(c, papel))
        .collect();
    let tinta: Vec<bool> = dpap.iter().map(|&d| d > 40.0).collect();
    // Una foto (sin papel liso) no tiene «rayas» que sacar: su textura son
    // mil rayitas.
    let lisas = dpap.iter().filter(|&&d| d < 12.0).count();
    if (lisas as f64) < 0.3 * (zw * zh) as f64 {
        return (None, Vec::new());
    }
    let nucleo = mascara::erosionar(&tinta, zw, zh, 2.0);
    let grueso = mascara::dilatar(&nucleo, zw, zh, 2.6);
    let fino: Vec<bool> = (0..zw * zh).map(|i| tinta[i] && !grueso[i]).collect();
    let minimo = 12f64.max(0.02 * zw.max(zh) as f64);
    let mut salida: Vec<(Vec<(f64, f64)>, String, f64)> = Vec::new();
    let mut quitar = vec![false; zw * zh];
    for comp in mascara::componentes(&fino, zw, zh) {
        if comp.len() < 10 {
            continue;
        }
        let (x0, y0, x1, y1) = mascara::caja_de(&comp, zw);
        let (ox, oy) = (x0.saturating_sub(2), y0.saturating_sub(2));
        let (gw, gh) = ((x1 + 3).min(zw) - ox, (y1 + 3).min(zh) - oy);
        let mut m = vec![false; gw * gh];
        for &i in &comp {
            m[(i / zw - oy) * gw + (i % zw - ox)] = true;
        }
        let esq = mascara::esqueleto(&m, gw, gh);
        let cadenas: Vec<Vec<(f64, f64)>> = mascara::cadenas(&esq, gw, gh)
            .into_iter()
            .filter(|c| c.len() >= 3)
            .collect();
        let largo: f64 = cadenas
            .iter()
            .map(|c| c.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>())
            .sum();
        if largo < minimo {
            continue;
        }
        // Fina de verdad: lo que pesa entre lo que mide.
        let peso: f64 = comp
            .iter()
            .map(|&i| (dpap[i] as f64 / 180.0).min(1.0))
            .sum();
        let grosor = peso / largo.max(1.0);
        if grosor > 4.0 {
            continue;
        }
        let mut cs: Vec<(f32, [f32; 3])> = comp.iter().map(|&i| (dpap[i], img.px[i])).collect();
        cs.sort_by(|a, b| b.0.total_cmp(&a.0));
        cs.truncate((cs.len() / 2).max(1));
        let media = mascara::media(cs.iter().map(|c| c.1));
        let color = hex(media);
        for c in cadenas {
            let puntos: Vec<(f64, f64)> = c
                .iter()
                .map(|&(x, y)| (x + (ox + zx) as f64, y + (oy + zy) as f64))
                .collect();
            let lisa = curva(&puntos, false, 0.6, 0.3);
            if lisa.len() >= 2 {
                salida.push((lisa, color.clone(), grosor.max(1.0)));
            }
        }
        for &i in &comp {
            quitar[i] = true;
        }
    }
    if salida.is_empty() {
        return (None, Vec::new());
    }
    // Lo que se quito, del color del papel (y su halo de antialias).
    let quitar = mascara::dilatar(&quitar, zw, zh, 1.0);
    let mut limpia = rgba.to_vec();
    for y in 0..zh {
        for x in 0..zw {
            if quitar[y * zw + x] && !grueso[y * zw + x] {
                let o = ((zy + y) * ancho + zx + x) * 4;
                limpia[o] = papel[0] as u8;
                limpia[o + 1] = papel[1] as u8;
                limpia[o + 2] = papel[2] as u8;
                limpia[o + 3] = 255;
            }
        }
    }
    salida.sort_by_key(|s| std::cmp::Reverse(s.0.len()));
    salida.truncate(400);
    (Some(limpia), salida)
}

/// **El texto de una imagen que se calca, como texto** (15-oct): lo que
/// leyo el OCR (palabras con caja en pixeles de la imagen) se tapa con el
/// color del papel antes de calcar ([`tapar_texto`]) y va al lienzo como
/// texto editable, en su sitio, con su tamano y su color.
pub fn tapar_texto(rgba: &mut [u8], ancho: u32, alto: u32, palabras: &[crate::hoja::PalabraHoja]) {
    let (w, h) = (ancho as usize, alto as usize);
    let img = crate::diagrama::mascara::Img::de_rgba(rgba, w, h);
    let papel = crate::diagrama::mascara::fondo(&img);
    for p in palabras {
        let c = p.caja.crecer(2.0).dentro(w as f64, h as f64);
        for y in c.y.floor() as usize..(c.y1().ceil() as usize).min(h) {
            for x in c.x.floor() as usize..(c.x1().ceil() as usize).min(w) {
                let i = (y * w + x) * 4;
                rgba[i] = papel[0] as u8;
                rgba[i + 1] = papel[1] as u8;
                rgba[i + 2] = papel[2] as u8;
                rgba[i + 3] = 255;
            }
        }
    }
}

/// Las palabras como textos del lienzo, colocadas como el calco.
pub fn textos_del_calco(
    c: &Calcado,
    palabras: &[crate::hoja::PalabraHoja],
    semilla: u64,
) -> Vec<serde_json::Value> {
    let familia = "Work Sans";
    let k = pixpin_motor2d::texto::interlineado_de(familia)
        .unwrap_or(pixpin_motor2d::texto::INTERLINEADO) as f64;
    let piezas: Vec<pixpin_motor2d::Elemento> = palabras
        .iter()
        .map(|p| {
            let (x, y) = (
                p.caja.x * c.escala + c.desplazamiento.0,
                p.caja.y * c.escala + c.desplazamiento.1,
            );
            let alto = p.caja.h * c.escala;
            let tam = ((alto * 1.05 * 2.0).round() / 2.0).max(5.0);
            crate::hoja::montaje::un_texto(
                &p.texto,
                x,
                y + alto / 2.0 - tam * k / 2.0,
                tam,
                familia,
                p.negrita,
                crate::hoja::Alineacion::Izquierda,
                p.color,
            )
        })
        .collect();
    let mut f = crate::lienzo::Fabrica::nueva((0.0, 0.0), semilla ^ 0x7e57);
    f.grupo = Some(format!("ia-{semilla:x}-calco"));
    f.de_motor(&piezas);
    f.salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Lienzo blanco de `w x h` con un circulo rojo (centro, radio) y un
    /// cuadrado azul (x, y, lado).
    pub(super) fn circulo_y_cuadrado(
        w: u32,
        h: u32,
        (cx, cy, r): (f64, f64, f64),
        (qx, qy, ql): (f64, f64, f64),
    ) -> Vec<u8> {
        let mut px = vec![255u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                if (fx - cx).powi(2) + (fy - cy).powi(2) <= r * r {
                    px[i..i + 3].copy_from_slice(&[220, 30, 30]);
                }
                if fx >= qx && fx <= qx + ql && fy >= qy && fy <= qy + ql {
                    px[i..i + 3].copy_from_slice(&[30, 60, 210]);
                }
            }
        }
        px
    }

    /// Del lienzo a la imagen original.
    fn a_imagen(c: &Calcado, (x, y): (f64, f64)) -> (f64, f64) {
        (
            (x - c.desplazamiento.0) / c.escala,
            (y - c.desplazamiento.1) / c.escala,
        )
    }

    fn error_circulo(c: &Calcado, pts: &[(f64, f64)], (cx, cy, r): (f64, f64, f64)) -> f64 {
        pts.iter()
            .map(|p| {
                let (x, y) = a_imagen(c, *p);
                (((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - r).abs()
            })
            .sum::<f64>()
            / pts.len() as f64
    }

    #[test]
    fn circulo_y_cuadrado_salen_como_manchas_cerradas_y_fieles() {
        let (w, h) = (400, 300);
        let circ = (110.0, 150.0, 70.0);
        let px = circulo_y_cuadrado(w, h, circ, (230.0, 80.0, 120.0));
        let t0 = std::time::Instant::now();
        let c = calcar(&px, w, h, &Parametros::default());
        assert!(t0.elapsed().as_millis() < 2000);
        let manchas: Vec<_> = c
            .trazos
            .iter()
            .filter_map(|t| match t {
                Trazo::Mancha {
                    contorno, color, ..
                } => Some((contorno, color)),
                _ => None,
            })
            .collect();
        assert_eq!(manchas.len(), 2, "{:?}", c.trazos.len());
        let rojo = manchas
            .iter()
            .find(|(_, col)| col.starts_with("#d") || col.starts_with("#e"))
            .expect("mancha roja");
        let azul = manchas
            .iter()
            .find(|(_, col)| col.ends_with("d2") || col.ends_with("d1") || col.ends_with("d3"))
            .expect("mancha azul");
        // El circulo: radio medio con error de menos de 1,5 px (de 70).
        let e = error_circulo(&c, rojo.0, circ);
        assert!(e < 1.5, "error del circulo {e}");
        assert!(
            rojo.0.len() >= 12,
            "un circulo de pocos puntos: {}",
            rojo.0.len()
        );
        // El cuadrado: 4 esquinas bien puestas (se conservan, no se redondean).
        let pts: Vec<_> = azul.0.iter().map(|p| a_imagen(&c, *p)).collect();
        for esquina in [(230.0, 80.0), (350.0, 80.0), (350.0, 200.0), (230.0, 200.0)] {
            let d = pts
                .iter()
                .map(|p| dist(*p, esquina))
                .fold(f64::MAX, f64::min);
            // La esquina, en su sitio: ni achaflanada ni redondeada.
            assert!(d < 1.5, "esquina {esquina:?} a {d}");
        }
        assert!(azul.0.len() <= 5, "un cuadrado con {} puntos", azul.0.len());
        // Y cada vertice, pegado al borde del cuadrado.
        for p in &pts {
            let al_borde = [
                (p.0 - 230.0).abs(),
                (p.0 - 350.0).abs(),
                (p.1 - 80.0).abs(),
                (p.1 - 200.0).abs(),
            ]
            .into_iter()
            .fold(f64::MAX, f64::min);
            assert!(al_borde < 2.0, "vertice {p:?} a {al_borde} del borde");
        }
    }

    #[test]
    fn solo_contornos_da_lineas_cerradas_fieles() {
        let (w, h) = (400, 300);
        let circ = (110.0, 150.0, 70.0);
        let px = circulo_y_cuadrado(w, h, circ, (230.0, 80.0, 120.0));
        let c = calcar(
            &px,
            w,
            h,
            &Parametros {
                solo_contornos: true,
                ..Default::default()
            },
        );
        let lineas: Vec<_> = c
            .trazos
            .iter()
            .filter_map(|t| match t {
                Trazo::Linea {
                    puntos,
                    cerrada,
                    color,
                } => Some((puntos, *cerrada, color)),
                _ => None,
            })
            .collect();
        assert!(
            lineas.len() >= 2 && lineas.len() <= 4,
            "{} lineas",
            lineas.len()
        );
        let cerradas: Vec<_> = lineas.iter().filter(|l| l.1).collect();
        assert!(cerradas.len() >= 2, "contornos sin cerrar");
        // La del circulo: la de centro cerca de (110,150).
        let del_circulo = cerradas
            .iter()
            .find(|l| {
                let (sx, sy) = l.0.iter().fold((0.0, 0.0), |s, p| (s.0 + p.0, s.1 + p.1));
                let m = a_imagen(&c, (sx / l.0.len() as f64, sy / l.0.len() as f64));
                dist(m, (110.0, 150.0)) < 10.0
            })
            .expect("contorno del circulo");
        let e = error_circulo(&c, del_circulo.0, circ);
        assert!(e < 2.0, "error del contorno del circulo {e}");
        // Con colores: la linea del circulo es roja.
        assert!(
            del_circulo.2.starts_with("#d") || del_circulo.2.starts_with("#e"),
            "{}",
            del_circulo.2
        );
    }

    #[test]
    fn un_agujero_sale_como_hueco() {
        // Un anillo: mancha con un agujero.
        let (w, h) = (100usize, 100usize);
        let mascara: Vec<bool> = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f64 + 0.5, (i / w) as f64 + 0.5);
                let d = ((x - 50.0).powi(2) + (y - 50.0).powi(2)).sqrt();
                (20.0..=40.0).contains(&d)
            })
            .collect();
        let r = regiones(&mascara, w, h, 4.0);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1.len(), 1, "falta el agujero");
    }

    #[test]
    fn imagen_vacia_o_rara_no_cuelga_ni_dibuja_de_mas() {
        // Blanca entera.
        let blanco = vec![255u8; 300 * 200 * 4];
        assert!(
            calcar(&blanco, 300, 200, &Parametros::default())
                .trazos
                .is_empty()
        );
        assert!(
            calcar(
                &blanco,
                300,
                200,
                &Parametros {
                    solo_contornos: true,
                    ..Default::default()
                }
            )
            .trazos
            .is_empty()
        );
        // Sin pixeles, de tamano cero o con menos bytes de los que dice.
        assert!(calcar(&[], 0, 0, &Parametros::default()).trazos.is_empty());
        assert!(
            calcar(&[1, 2, 3], 10, 10, &Parametros::default())
                .trazos
                .is_empty()
        );
        // Un pixel.
        let _ = calcar(&[0, 0, 0, 255], 1, 1, &Parametros::default());
        let _ = calcar(
            &[0, 0, 0, 255],
            1,
            1,
            &Parametros {
                solo_contornos: true,
                ..Default::default()
            },
        );
        // Ruido puro: termina rapido y respeta el tope.
        let mut s = 12345u32;
        let ruido: Vec<u8> = (0..768 * 576 * 4)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (s >> 16) as u8
            })
            .collect();
        for p in [
            Parametros::default(),
            Parametros {
                solo_contornos: true,
                ..Default::default()
            },
            Parametros {
                detalle: Detalle::Alto,
                ..Default::default()
            },
        ] {
            let t0 = std::time::Instant::now();
            let c = calcar(&ruido, 768, 576, &p);
            assert!(c.trazos.len() <= p.detalle.tope(), "{}", c.trazos.len());
            // En depuracion; en la app (optimizada) es mucho menos.
            assert!(t0.elapsed().as_secs_f32() < 10.0, "{:?}", t0.elapsed());
        }
    }

    #[test]
    fn una_zona_calca_solo_ese_trozo() {
        let (w, h) = (400, 300);
        let px = circulo_y_cuadrado(w, h, (110.0, 150.0, 70.0), (230.0, 80.0, 120.0));
        // La mitad derecha: solo el cuadrado.
        let c = calcar(
            &px,
            w,
            h,
            &Parametros {
                zona: Some([0.5, 0.0, 0.5, 1.0]),
                ..Default::default()
            },
        );
        assert_eq!(c.trazos.len(), 1, "{:?}", c.trazos);
        let Trazo::Mancha { contorno, .. } = &c.trazos[0] else {
            panic!()
        };
        let esquina = a_imagen(&c, contorno[0]);
        assert!(esquina.0 > 200.0, "{esquina:?}");
    }

    /// Un emoji de perrito de mentira, con degradados como los de verdad:
    /// cara con degradado radial, orejas oscuras, lengua rosa, fondo
    /// transparente.
    fn emoji_con_degradados(w: u32) -> Vec<u8> {
        let mut px = vec![0u8; (w * w * 4) as usize];
        let c = w as f64 / 2.0;
        for y in 0..w {
            for x in 0..w {
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let i = ((y * w + x) * 4) as usize;
                let d = ((fx - c).powi(2) + (fy - c).powi(2)).sqrt() / (w as f64 * 0.38);
                let mut col: Option<[f64; 3]> = None;
                if d <= 1.0 {
                    // Del melocoton claro del centro al marron del borde.
                    col = Some([250.0 - 50.0 * d, 200.0 - 70.0 * d, 160.0 - 70.0 * d]);
                }
                for ox in [0.2, 0.8] {
                    let e = ((fx - ox * w as f64) / (w as f64 * 0.1)).powi(2)
                        + ((fy - 0.3 * w as f64) / (w as f64 * 0.18)).powi(2);
                    if e <= 1.0 {
                        col = Some([110.0 - 20.0 * e, 70.0 - 15.0 * e, 60.0 - 10.0 * e]);
                    }
                }
                let lengua = ((fx - c) / (w as f64 * 0.07)).powi(2)
                    + ((fy - 0.75 * w as f64) / (w as f64 * 0.09)).powi(2);
                if lengua <= 1.0 {
                    col = Some([215.0, 50.0 + 20.0 * lengua, 120.0]);
                }
                if let Some(k) = col {
                    px[i..i + 4].copy_from_slice(&[k[0] as u8, k[1] as u8, k[2] as u8, 255]);
                }
            }
        }
        px
    }

    #[test]
    fn un_emoji_con_degradados_se_parece() {
        let w = 384;
        let px = emoji_con_degradados(w);
        for detalle in [Detalle::Medio, Detalle::Alto] {
            let c = calcar(
                &px,
                w,
                w,
                &Parametros {
                    detalle,
                    ..Default::default()
                },
            );
            // El fondo transparente (blanco) no se dibuja.
            assert!(!c.trazos.iter().any(|t| matches!(t, Trazo::Fondo { .. })));
            let manchas: Vec<(&Vec<(f64, f64)>, &String)> = c
                .trazos
                .iter()
                .filter_map(|t| match t {
                    Trazo::Mancha {
                        contorno, color, ..
                    } => Some((contorno, color)),
                    _ => None,
                })
                .collect();
            // Unas pocas bandas de color, no cientos de manchitas.
            assert!(
                manchas.len() >= 4 && manchas.len() <= detalle.tope(),
                "{detalle:?}: {}",
                manchas.len()
            );
            // Lo calcado ocupa lo mismo que el emoji: de la oreja izquierda a
            // la derecha y de arriba de la cara a abajo.
            let pts: Vec<(f64, f64)> = manchas
                .iter()
                .flat_map(|m| m.0.iter())
                .map(|p| {
                    (
                        (p.0 - c.desplazamiento.0) / c.escala,
                        (p.1 - c.desplazamiento.1) / c.escala,
                    )
                })
                .collect();
            let caja = pts
                .iter()
                .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
                    [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)]
                });
            let esperada = [0.1, 0.12, 0.9, 0.88].map(|f| f * w as f64);
            for (a, b) in caja.iter().zip(esperada) {
                assert!(
                    (a - b).abs() < 0.04 * w as f64,
                    "{detalle:?}: caja {caja:?} y no {esperada:?}"
                );
            }
            let rgb = |h: &str| {
                let v = |i| u8::from_str_radix(&h[i..i + 2], 16).unwrap() as i32;
                (v(1), v(3), v(5))
            };
            assert!(
                manchas.iter().any(|m| {
                    let (r, g, b) = rgb(m.1);
                    r > 180 && g < 110 && b > 90
                }),
                "sin la lengua rosa"
            );
            assert!(
                manchas.iter().any(|m| {
                    let (r, g, b) = rgb(m.1);
                    r < 130 && g < 90 && b < 80
                }),
                "sin las orejas oscuras"
            );
            // Ningun agujero tapado por otra mancha (dejaria una rendija).
            for t in &c.trazos {
                if let Trazo::Mancha { huecos, .. } = t {
                    assert!(
                        huecos.is_empty(),
                        "{detalle:?}: un agujero sobre otra mancha"
                    );
                }
            }
        }
    }

    #[test]
    fn lo_que_no_se_pidio_se_corrige_con_el_pedido() {
        let mal = Parametros {
            solo_contornos: true,
            colores: false,
            ..Default::default()
        };
        let mut p = mal.clone();
        ajustar_al_pedido(
            &mut p,
            "Dibuja este perrito de la imagen adjunta dentro del lienzo.",
        );
        assert!(!p.solo_contornos && p.colores);
        let mut p = mal.clone();
        ajustar_al_pedido(&mut p, "Calca solo los contornos, en blanco y negro");
        assert_eq!(p, mal, "lo pedido se respeta");
    }

    /// Una imagen con antialias (4x4 muestras por pixel) de lo que diga
    /// `color` en cada punto (None: blanco).
    fn con_antialias(w: u32, h: u32, color: impl Fn(f64, f64) -> Option<[f64; 3]>) -> Vec<u8> {
        let mut px = vec![255u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let mut s = [0.0; 3];
                for k in 0..16 {
                    let (fx, fy) = (
                        x as f64 + (k % 4) as f64 / 4.0 + 0.125,
                        y as f64 + (k / 4) as f64 / 4.0 + 0.125,
                    );
                    let c = color(fx, fy).unwrap_or([255.0; 3]);
                    (0..3).for_each(|j| s[j] += c[j] / 16.0);
                }
                let i = ((y * w + x) * 4) as usize;
                for j in 0..3 {
                    px[i + j] = s[j].round() as u8;
                }
            }
        }
        px
    }

    /// El mayor giro de un vertice (grados), en un anillo o una polilinea.
    fn giro_maximo(p: &[(f64, f64)], cerrado: bool) -> f64 {
        let n = p.len();
        let rango: Vec<usize> = if cerrado {
            (0..n).collect()
        } else {
            (1..n.saturating_sub(1)).collect()
        };
        rango
            .into_iter()
            .map(|i| {
                let (a, b, c) = (p[(i + n - 1) % n], p[i], p[(i + 1) % n]);
                let (u, v) = ((b.0 - a.0, b.1 - a.1), (c.0 - b.0, c.1 - b.1));
                (u.0 * v.1 - u.1 * v.0)
                    .atan2(u.0 * v.0 + u.1 * v.1)
                    .abs()
                    .to_degrees()
            })
            .fold(0.0, f64::max)
    }

    /// **Lo que pidio el usuario**: un circulo calcado es una curva, no una
    /// escalera. Antes (contornos de pixel + RDP) el contorno giraba 90° en
    /// cada peldano; ahora ningun vertice gira mas de unos grados, y el
    /// borde cae a menos de medio pixel del de verdad.
    #[test]
    fn un_circulo_calcado_es_una_curva_y_no_una_escalera() {
        let (w, h) = (768u32, 576u32);
        let (cx, cy, r) = (384.0, 288.0, 200.0);
        let px = con_antialias(w, h, |x, y| {
            ((x - cx).powi(2) + (y - cy).powi(2) <= r * r).then_some([30.0, 60.0, 160.0])
        });
        for solo_contornos in [false, true] {
            let c = calcar(
                &px,
                w,
                h,
                &Parametros {
                    solo_contornos,
                    ..Default::default()
                },
            );
            assert_eq!(c.trazos.len(), 1, "{solo_contornos}: {:?}", c.trazos.len());
            let (pts, cerrado) = match &c.trazos[0] {
                Trazo::Mancha { contorno, .. } => (contorno.clone(), true),
                Trazo::Linea { puntos, .. } => (puntos[..puntos.len() - 1].to_vec(), true),
                t => panic!("{t:?}"),
            };
            let g = giro_maximo(&pts, cerrado);
            assert!(g < 12.0, "{solo_contornos}: un vertice gira {g}°");
            let e = error_circulo(&c, &pts, (cx, cy, r));
            assert!(e < 0.6, "{solo_contornos}: error medio {e}");
            // Ni pocos puntos (poligono a la vista) ni miles.
            assert!(
                pts.len() >= 40 && pts.len() <= 200,
                "{solo_contornos}: {} puntos",
                pts.len()
            );
        }
    }

    /// Un borde recto e inclinado no se curva, y una esquina de 90° queda
    /// esquina (no se redondea ni se achaflana).
    #[test]
    fn lo_recto_queda_recto_y_las_esquinas_esquinas() {
        let (w, h) = (600u32, 450u32);
        // Un cuadrado girado 20° (lado 240) en el centro.
        let (cx, cy, l, g) = (300.0f64, 225.0f64, 240.0f64, 20f64.to_radians());
        let px = con_antialias(w, h, |x, y| {
            let (dx, dy) = (x - cx, y - cy);
            let (u, v) = (dx * g.cos() + dy * g.sin(), -dx * g.sin() + dy * g.cos());
            (u.abs() <= l / 2.0 && v.abs() <= l / 2.0).then_some([200.0, 40.0, 40.0])
        });
        let esquinas: Vec<(f64, f64)> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .iter()
            .map(|(a, b)| {
                let (u, v) = (a * l / 2.0, b * l / 2.0);
                (
                    cx + u * g.cos() - v * g.sin(),
                    cy + u * g.sin() + v * g.cos(),
                )
            })
            .collect();
        for solo_contornos in [false, true] {
            let c = calcar(
                &px,
                w,
                h,
                &Parametros {
                    solo_contornos,
                    ..Default::default()
                },
            );
            let pts: Vec<(f64, f64)> = match &c.trazos[0] {
                Trazo::Mancha { contorno, .. } => contorno.clone(),
                Trazo::Linea { puntos, .. } => puntos[..puntos.len() - 1].to_vec(),
                t => panic!("{t:?}"),
            };
            let pts: Vec<(f64, f64)> = pts.iter().map(|p| a_imagen(&c, *p)).collect();
            // Cuatro lados rectos: cuatro vertices (y si acaso alguno de mas
            // en Canny, que corta las esquinas), cada uno en una esquina.
            assert!(
                pts.len() >= 4 && pts.len() <= 6,
                "{solo_contornos}: {} puntos: {pts:?}",
                pts.len()
            );
            for e in &esquinas {
                let d = pts.iter().map(|p| dist(*p, *e)).fold(f64::MAX, f64::min);
                let tope = if solo_contornos { 3.5 } else { 1.5 };
                assert!(d < tope, "{solo_contornos}: esquina {e:?} a {d}");
            }
        }
    }

    /// Dos manchas vecinas no dejan rendija: la grande (debajo) se mete por
    /// debajo de la pequena, y su borde con ella no se ve.
    #[test]
    fn dos_manchas_vecinas_no_dejan_rendija() {
        let (w, h) = (400u32, 300u32);
        // Un disco rojo partido: la mitad derecha, azul y mas pequena.
        let px = con_antialias(w, h, |x, y| {
            let d = (x - 200.0).powi(2) + (y - 150.0).powi(2);
            if d > 120.0f64.powi(2) {
                None
            } else if x > 230.0 + (y - 150.0) * 0.3 {
                Some([30.0, 60.0, 210.0])
            } else {
                Some([220.0, 30.0, 30.0])
            }
        });
        let c = calcar(&px, w, h, &Parametros::default());
        let manchas: Vec<&Vec<(f64, f64)>> = c
            .trazos
            .iter()
            .filter_map(|t| match t {
                Trazo::Mancha { contorno, .. } => Some(contorno),
                _ => None,
            })
            .collect();
        assert_eq!(manchas.len(), 2);
        // La primera (debajo) es la grande, la roja; cubre una franja de
        // la azul junto a la frontera.
        let roja: Vec<(f64, f64)> = manchas[0].iter().map(|p| a_imagen(&c, *p)).collect();
        for y in [80.0, 150.0, 220.0] {
            let frontera = 230.0 + (y - 150.0) * 0.3;
            assert!(
                dentro(&roja, (frontera + 1.5, y)),
                "la roja no se mete bajo la azul en y={y}"
            );
            // Pero no asoma por el otro lado de la azul.
            assert!(
                !dentro(&roja, (frontera + 30.0, y)),
                "la roja tapa la azul en y={y}"
            );
        }
    }

    #[test]
    fn una_recta_queda_recta_y_un_cuadrado_con_sus_esquinas() {
        let linea: Vec<(f64, f64)> = (0..50)
            .map(|i| (i as f64, (i as f64 * 0.01).sin() * 0.1))
            .collect();
        assert_eq!(curva(&linea, false, 0.5, 0.1).len(), 2);
        let mut cuadrado = Vec::new();
        for i in 0..40 {
            cuadrado.push((i as f64 / 4.0, 0.0));
        }
        for i in 0..40 {
            cuadrado.push((10.0, i as f64 / 4.0));
        }
        for i in 0..40 {
            cuadrado.push((10.0 - i as f64 / 4.0, 10.0));
        }
        for i in 0..40 {
            cuadrado.push((0.0, 10.0 - i as f64 / 4.0));
        }
        let c = curva(&cuadrado, true, 0.5, 0.1);
        assert_eq!(c.len(), 4, "{c:?}");
        assert!((area(&c).abs() - 100.0).abs() < 1e-6);
    }
}
