//! **Como se ve una tabla de una foto**: la rejilla (donde caen sus rayas),
//! el fondo de cada celda, el color y grosor del borde, el color y el tamano
//! de la letra y el aire a la izquierda del texto, medidos en los pixeles
//! (16-oct-2026, «que la tabla quede casi identica en el lienzo y que los
//! colores de la cabecera pasen al Excel»).
//!
//! Lo que dicen las celdas lo saca [`crate::tabla_escaneada`]; aqui solo el
//! aspecto, para que el dibujo (`pixpin_motor2d::tabla_dibujada::Aspecto`) y
//! el Excel (los fondos y anchos de `EstiloDeCelda`) lo copien. Es un
//! algoritmo clasico, sin modelo: rapido y sin inventar.
//!
//! ## Como
//!
//! - **El papel**: la mediana de la luz de la imagen.
//! - **Las rayas**: una fila de pixeles es raya horizontal si tiene un tramo
//!   oscuro largo (una cuarta parte del ancho); las filas seguidas son la
//!   misma raya (su grosor). Entre la primera y la ultima, una columna de
//!   pixeles es raya vertical si su tramo oscuro cubre casi toda la altura.
//!   Una tabla sin rayas (o torcida en la foto: las rayas se rompen) se
//!   queda sin rejilla y solo con la letra.
//! - **Los fondos**: dentro de cada celda, sin su texto ni sus rayas, la
//!   mediana del color; si no es el del papel, es su fondo.
//! - **La letra**: la mediana del alto de las palabras del OCR, pasada a
//!   tamano de letra ([`ALTO_A_TAM`]).

use crate::tabla_escaneada::Palabra;

/// Del alto de la caja de una palabra (lo que da el OCR de Windows: de lo
/// mas alto a lo mas bajo de sus letras) al tamano de la letra. Medido con
/// Segoe UI a varios tamanos en el banco de documentos (16-oct).
pub const ALTO_A_TAM: f64 = 1.3;

/// Lo medido, en pixeles de la imagen.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Medida {
    /// Donde caen las rayas verticales y horizontales (bordes de columnas y
    /// filas), de izquierda a derecha y de arriba abajo. Vacias si no hay
    /// rejilla.
    pub columnas: Vec<f64>,
    pub filas: Vec<f64>,
    /// El fondo de cada celda de la rejilla (`None`: el del papel).
    pub fondos: Vec<Vec<Option<[u8; 3]>>>,
    /// Color y grosor de las rayas.
    pub borde: Option<([u8; 3], f64)>,
    pub texto: [u8; 3],
    pub tam_letra: f64,
    /// Lo que va del borde izquierdo de la celda al texto.
    pub aire: Option<f64>,
    /// Lo que dice cada celda de la rejilla (las palabras de dentro).
    pub celdas: Vec<Vec<String>>,
    /// Los grados que estaba torcida la foto (0: derecha).
    pub inclinacion: f64,
    /// El texto de la imagen fuera de la tabla (el titulo, una nota), por
    /// renglones, donde esta.
    pub fuera: Vec<Renglon>,
}

/// Un renglon de texto de fuera de la tabla: su esquina y su letra, en
/// pixeles de la imagen (de la enderezada, si estaba torcida).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Renglon {
    pub texto: String,
    pub x: f64,
    pub y: f64,
    pub tam: f64,
}

impl Medida {
    /// Si trae rejilla de `filas` x `columnas` celdas.
    pub fn cuadra(&self, filas: usize, columnas: usize) -> bool {
        self.filas.len() == filas + 1 && self.columnas.len() == columnas + 1
    }
    pub fn anchos(&self) -> Vec<f64> {
        self.columnas.windows(2).map(|w| w[1] - w[0]).collect()
    }
    pub fn altos(&self) -> Vec<f64> {
        self.filas.windows(2).map(|w| w[1] - w[0]).collect()
    }
}

/// Palabras que el OCR de Windows deja sin su tilde, con la buena. Las de
/// obra, de clase y de oficina que salen en tablas; sin las que cambian de
/// sentido sin tilde («esta», «el», «solo», «area» de «áreas»…).
const SIN_TILDE: &[(&str, &str)] = &[
    ("ferreteria", "ferretería"),
    ("dias", "días"),
    ("guia", "guía"),
    ("practica", "práctica"),
    ("compresion", "compresión"),
    ("tecnologia", "tecnología"),
    ("ubicacion", "ubicación"),
    ("metodo", "método"),
    ("numero", "número"),
    ("codigo", "código"),
    ("telefono", "teléfono"),
    ("direccion", "dirección"),
    ("descripcion", "descripción"),
    ("observacion", "observación"),
    ("informacion", "información"),
    ("almacen", "almacén"),
    ("construccion", "construcción"),
    ("revision", "revisión"),
    ("seccion", "sección"),
    ("conclusion", "conclusión"),
    ("calculo", "cálculo"),
    ("tecnico", "técnico"),
    ("tecnica", "técnica"),
    ("analisis", "análisis"),
    ("ingenieria", "ingeniería"),
    ("energia", "energía"),
    ("electrico", "eléctrico"),
    ("electrica", "eléctrica"),
    ("mecanica", "mecánica"),
    ("grafica", "gráfica"),
    ("basico", "básico"),
    ("unico", "único"),
    ("minimo", "mínimo"),
    ("maximo", "máximo"),
    ("latex", "látex"),
    ("metrico", "métrico"),
    ("cubico", "cúbico"),
    ("camion", "camión"),
    ("excavacion", "excavación"),
    ("instalacion", "instalación"),
    ("tuberia", "tubería"),
    ("via", "vía"),
    ("pais", "país"),
    ("razon", "razón"),
    ("region", "región"),
    ("periodo", "período"),
    ("fisica", "física"),
    ("quimica", "química"),
    ("matematica", "matemática"),
    ("geometria", "geometría"),
    ("biologia", "biología"),
];

/// **Una pasada de correccion del OCR**, sin modelo: las tildes que se pierden
/// (`SIN_TILDE`, con su mayuscula) y el «N.°» que sale «N.0» o «N.o».
pub fn corregir_ocr(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    let palabras: Vec<&str> = t.split(' ').collect();
    for (i, p) in palabras.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        // «N.0 16», «N.o 16», «Nro» no: solo el ordinal del numero.
        if matches!(*p, "N.0" | "N.o" | "N°" | "N.º" | "n.0" | "n.o") {
            s.push_str(if p.starts_with('n') { "n.°" } else { "N.°" });
            continue;
        }
        let (cuerpo, cola) = p.split_at(p.trim_end_matches(|c: char| !c.is_alphanumeric()).len());
        let (cabeza, cuerpo) = cuerpo.split_at(
            cuerpo.len()
                - cuerpo
                    .trim_start_matches(|c: char| !c.is_alphanumeric())
                    .len(),
        );
        let bajo = cuerpo.to_lowercase();
        match SIN_TILDE.iter().find(|(sin, _)| *sin == bajo) {
            Some((_, con)) => {
                let mut bueno: String =
                    if cuerpo.chars().all(|c| !c.is_lowercase()) && cuerpo.chars().count() > 1 {
                        con.to_uppercase()
                    } else {
                        con.to_string()
                    };
                if cuerpo.chars().next().is_some_and(char::is_uppercase)
                    && !bueno.chars().all(|c| !c.is_lowercase())
                {
                    let mut cs = bueno.chars();
                    bueno = cs
                        .next()
                        .map(|c| c.to_uppercase().collect::<String>() + cs.as_str())
                        .unwrap_or_default();
                }
                s.push_str(cabeza);
                s.push_str(&bueno);
                s.push_str(cola);
            }
            None => s.push_str(p),
        }
    }
    s
}

/// `#rrggbb`.
pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

struct Imagen<'a> {
    px: &'a [u8],
    w: usize,
    _h: usize,
}

impl Imagen<'_> {
    fn rgb(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.w + x) * 4;
        [self.px[i], self.px[i + 1], self.px[i + 2]]
    }
    fn luz(&self, x: usize, y: usize) -> f64 {
        let [r, g, b] = self.rgb(x, y);
        0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64
    }
}

fn mediana(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    Some(v[v.len() / 2])
}

fn mediana_rgb(v: &[[u8; 3]]) -> Option<[u8; 3]> {
    if v.is_empty() {
        return None;
    }
    let canal = |k: usize| mediana(v.iter().map(|c| c[k] as f64).collect()).unwrap_or(0.0) as u8;
    Some([canal(0), canal(1), canal(2)])
}

fn distancia(a: [u8; 3], b: [u8; 3]) -> f64 {
    (0..3)
        .map(|k| (a[k] as f64 - b[k] as f64).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// Las filas (o columnas) seguidas que cumplen, juntas: (centro, grosor).
fn juntar(marcadas: &[usize]) -> Vec<(f64, f64)> {
    let mut v: Vec<(usize, usize)> = Vec::new();
    for &i in marcadas {
        match v.last_mut() {
            Some((_, fin)) if i <= *fin + 1 => *fin = i,
            _ => v.push((i, i)),
        }
    }
    v.into_iter()
        .map(|(a, b)| ((a + b) as f64 / 2.0, (b - a + 1) as f64))
        .collect()
}

/// El tramo oscuro mas largo de una lista de si/no.
fn tramo_mas_largo(oscuros: impl Iterator<Item = bool>) -> (usize, usize, usize) {
    let (mut mejor, mut desde_mejor, mut actual, mut desde) = (0usize, 0usize, 0usize, 0usize);
    for (i, o) in oscuros.enumerate() {
        if o {
            if actual == 0 {
                desde = i;
            }
            actual += 1;
            if actual > mejor {
                mejor = actual;
                desde_mejor = desde;
            }
        } else {
            actual = 0;
        }
    }
    (mejor, desde_mejor, desde_mejor + mejor)
}

/// **Mide el aspecto** de la tabla de una imagen RGBA con las palabras que
/// leyo el OCR (en sus mismos pixeles). `None` si no hay palabras. Una foto
/// torcida (un escaneo) se endereza antes ([`inclinacion`]): torcidas, las
/// rayas se rompen y no hay rejilla. Con rejilla, [`Medida::celdas`] trae lo
/// que dice cada celda (solo lo de dentro: sin el texto de la pagina).
pub fn medir(rgba: &[u8], ancho: u32, alto: u32, palabras: &[Palabra]) -> Option<Medida> {
    let m = medir_derecha(rgba, ancho, alto, palabras)?;
    if !m.columnas.is_empty() {
        return Some(con_celdas(m, palabras));
    }
    let Some(grados) = inclinacion(rgba, ancho, alto) else {
        return Some(m);
    };
    let (s, c) = grados.to_radians().sin_cos();
    let (cx, cy) = (ancho as f64 / 2.0, alto as f64 / 2.0);
    let derecho = enderezar(rgba, ancho, alto, grados);
    // Las palabras, por el mismo giro (su centro).
    let ps: Vec<Palabra> = palabras
        .iter()
        .map(|p| {
            let (dx, dy) = (p.x + p.ancho / 2.0 - cx, p.y + p.alto / 2.0 - cy);
            let (x, y) = (c * dx + s * dy + cx, -s * dx + c * dy + cy);
            Palabra {
                x: x - p.ancho / 2.0,
                y: y - p.alto / 2.0,
                ..p.clone()
            }
        })
        .collect();
    let mut m2 = medir_derecha(&derecho, ancho, alto, &ps)?;
    m2.inclinacion = grados;
    Some(if m2.columnas.is_empty() {
        m2
    } else {
        con_celdas(m2, &ps)
    })
}

/// Lo que dice cada celda de la rejilla: las palabras cuyo centro cae dentro,
/// en orden de lectura.
fn con_celdas(mut m: Medida, palabras: &[Palabra]) -> Medida {
    let (nf, nc) = (
        m.filas.len().saturating_sub(1),
        m.columnas.len().saturating_sub(1),
    );
    let mut celdas: Vec<Vec<Vec<&Palabra>>> = vec![vec![Vec::new(); nc]; nf];
    let mut fuera: Vec<&Palabra> = Vec::new();
    for p in palabras {
        let (x, y) = (p.x + p.ancho / 2.0, p.y + p.alto / 2.0);
        let f = (0..nf).find(|&f| y >= m.filas[f] && y < m.filas[f + 1]);
        let c = (0..nc).find(|&c| x >= m.columnas[c] && x < m.columnas[c + 1]);
        if let (Some(f), Some(c)) = (f, c) {
            celdas[f][c].push(p);
        } else {
            fuera.push(p);
        }
    }
    // Lo de fuera de la tabla (un titulo, una nota al pie), por renglones:
    // palabras seguidas en la misma altura y sin un hueco de mas de dos
    // letras entre ellas (dos columnas de texto no se juntan; un guion que el OCR se come deja un hueco de dos).
    // Primero las bandas de altura (un renglon de la hoja) y, dentro, de
    // izquierda a derecha, partiendo donde hay un hueco grande.
    fuera.sort_by(|a, b| (a.y + a.alto / 2.0).total_cmp(&(b.y + b.alto / 2.0)));
    let mut bandas: Vec<Vec<&Palabra>> = Vec::new();
    for p in fuera {
        let centro = p.y + p.alto / 2.0;
        match bandas.last_mut() {
            Some(b)
                if (centro
                    - mediana(b.iter().map(|q| q.y + q.alto / 2.0).collect())
                        .unwrap_or(centro))
                .abs()
                    < p.alto.max(b[0].alto) * 0.5 =>
            {
                b.push(p)
            }
            _ => bandas.push(vec![p]),
        }
    }
    let mut renglones: Vec<Vec<&Palabra>> = Vec::new();
    for mut b in bandas {
        b.sort_by(|a, c| a.x.total_cmp(&c.x));
        let mut actual: Vec<&Palabra> = Vec::new();
        for p in b {
            if let Some(u) = actual.last()
                && p.x - (u.x + u.ancho) > 3.0 * u.alto.max(p.alto)
            {
                renglones.push(std::mem::take(&mut actual));
            }
            actual.push(p);
        }
        if !actual.is_empty() {
            renglones.push(actual);
        }
    }
    // La letra: la de casi todos los renglones (una palabra con letras altas
    // no es un renglon mas grande); un titulo de verdad, la suya.
    let comun = mediana(renglones.iter().flatten().map(|p| p.alto).collect()).unwrap_or(10.0);
    m.fuera = renglones
        .into_iter()
        .map(|r| {
            let x = r.iter().map(|p| p.x).fold(f64::MAX, f64::min);
            let y = r.iter().map(|p| p.y).fold(f64::MAX, f64::min);
            let alto = mediana(r.iter().map(|p| p.alto).collect()).unwrap_or(comun);
            let alto = if (alto - comun).abs() < comun * 0.25 {
                comun
            } else {
                alto
            };
            Renglon {
                texto: corregir_ocr(
                    &r.iter()
                        .map(|p| p.texto.trim())
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
                x,
                y,
                tam: alto * ALTO_A_TAM,
            }
        })
        .collect();
    m.celdas = celdas
        .into_iter()
        .map(|fila| {
            fila.into_iter()
                .map(|mut ps| {
                    // Por renglones (centros a menos de media letra) y, en
                    // cada uno, de izquierda a derecha: «(días)» es mas alta
                    // que «Edad» y no por eso va antes.
                    ps.sort_by(|a, b| (a.y + a.alto / 2.0).total_cmp(&(b.y + b.alto / 2.0)));
                    let mut renglones: Vec<Vec<&Palabra>> = Vec::new();
                    for p in ps {
                        let centro = p.y + p.alto / 2.0;
                        match renglones.last_mut() {
                            Some(r)
                                if (centro - (r[0].y + r[0].alto / 2.0)).abs()
                                    < r[0].alto.max(p.alto) * 0.5 =>
                            {
                                r.push(p)
                            }
                            _ => renglones.push(vec![p]),
                        }
                    }
                    // (Una lista dentro de la celda, «1. … 2. …» o «• …»: un
                    // renglon por punto, como en la foto.)
                    let lineas: Vec<String> = renglones
                        .into_iter()
                        .map(|mut r| {
                            r.sort_by(|a, b| a.x.total_cmp(&b.x));
                            r.into_iter()
                                .map(|p| p.texto.trim())
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .collect();
                    let es_punto = |l: &str| {
                        let l = l.trim_start();
                        l.starts_with(['•', '-', '–', '▪']) || {
                            let n = l.chars().take_while(|c| c.is_ascii_digit()).count();
                            n > 0 && n <= 2 && l[n..].starts_with(['.', ')'])
                        }
                    };
                    // Cada punto, con lo que le sigue hasta el siguiente.
                    let mut puntos: Vec<String> = Vec::new();
                    for l in lineas {
                        match puntos.last_mut() {
                            Some(u) if !es_punto(&l) => {
                                u.push(' ');
                                u.push_str(&l);
                            }
                            _ => puntos.push(l),
                        }
                    }
                    puntos
                        .iter()
                        .map(|p| corregir_ocr(p))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .collect()
        })
        .collect();
    m
}

/// **Cuanto esta torcida la hoja**, en grados (lo que gira el escaner), o
/// `None` si esta derecha (menos de 0,15°). Por la proyeccion de lo oscuro:
/// girada lo justo, las rayas y los renglones caen en muy pocas filas y la
/// suma de los cuadrados del histograma es la mayor.
pub fn inclinacion(rgba: &[u8], ancho: u32, alto: u32) -> Option<f64> {
    let (w, h) = (ancho as usize, alto as usize);
    if w < 8 || h < 8 || rgba.len() < w * h * 4 {
        return None;
    }
    let img = Imagen { px: rgba, w, _h: h };
    let mut luces: Vec<f64> = (0..w * h)
        .step_by(((w * h) / 20_000).max(1))
        .map(|i| img.luz(i % w, i / w))
        .collect();
    luces.sort_by(|a, b| a.total_cmp(b));
    let papel = luces[luces.len() / 2];
    let paso = ((w * h) / 400_000).max(1);
    let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
    let oscuros: Vec<(f64, f64)> = (0..w * h)
        .step_by(paso)
        .filter(|&i| img.luz(i % w, i / w) < papel * 0.72)
        .map(|i| ((i % w) as f64 - cx, (i / w) as f64 - cy))
        .collect();
    if oscuros.len() < 200 {
        return None;
    }
    let puntuar = |grados: f64| {
        let (s, c) = grados.to_radians().sin_cos();
        let mut hist = vec![0u32; w + h + 2];
        for &(dx, dy) in &oscuros {
            let y = (-s * dx + c * dy + (w + h) as f64 / 2.0) as usize;
            if let Some(b) = hist.get_mut(y) {
                *b += 1;
            }
        }
        hist.iter().map(|&n| (n as f64).powi(2)).sum::<f64>()
    };
    let (mut mejor, mut nota) = (0.0, puntuar(0.0));
    for k in -50..=50 {
        let g = k as f64 / 10.0;
        let n = puntuar(g);
        if n > nota {
            (mejor, nota) = (g, n);
        }
    }
    // Afinar a la centesima alrededor del mejor.
    for k in -9..=9 {
        let g = mejor + k as f64 / 100.0;
        let n = puntuar(g);
        if n > nota {
            (mejor, nota) = (g, n);
        }
    }
    (mejor.abs() >= 0.15).then_some(mejor)
}

/// La imagen girada para deshacer `grados` de [`inclinacion`] (lo de fuera,
/// el papel de la esquina).
pub fn enderezar(rgba: &[u8], ancho: u32, alto: u32, grados: f64) -> Vec<u8> {
    let (w, h) = (ancho as i64, alto as i64);
    let (s, c) = grados.to_radians().sin_cos();
    let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
    let fondo = [rgba[0], rgba[1], rgba[2]];
    let mut px = vec![255u8; rgba.len()];
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f64 - cx, y as f64 - cy);
            // El punto de la foto que cae aqui al enderezar.
            let (sx, sy) = (c * dx - s * dy + cx, s * dx + c * dy + cy);
            let (x0, y0) = (sx.floor() as i64, sy.floor() as i64);
            let (fx, fy) = (sx - x0 as f64, sy - y0 as f64);
            let i = ((y * w + x) * 4) as usize;
            for k in 0..3 {
                let p = |xx: i64, yy: i64| {
                    if xx < 0 || yy < 0 || xx >= w || yy >= h {
                        fondo[k] as f64
                    } else {
                        rgba[((yy * w + xx) * 4) as usize + k] as f64
                    }
                };
                let v = p(x0, y0) * (1.0 - fx) * (1.0 - fy)
                    + p(x0 + 1, y0) * fx * (1.0 - fy)
                    + p(x0, y0 + 1) * (1.0 - fx) * fy
                    + p(x0 + 1, y0 + 1) * fx * fy;
                px[i + k] = v.round().clamp(0.0, 255.0) as u8;
            }
            px[i + 3] = 255;
        }
    }
    px
}

fn medir_derecha(rgba: &[u8], ancho: u32, alto: u32, palabras: &[Palabra]) -> Option<Medida> {
    let (w, h) = (ancho as usize, alto as usize);
    if w < 8 || h < 8 || rgba.len() < w * h * 4 || palabras.is_empty() {
        return None;
    }
    let img = Imagen { px: rgba, w, _h: h };
    // El papel.
    let mut muestras = Vec::new();
    let paso = ((w * h) / 40_000).max(1);
    for i in (0..w * h).step_by(paso) {
        muestras.push(img.rgb(i % w, i / w));
    }
    let papel = mediana_rgb(&muestras)?;
    let luz_papel = 0.299 * papel[0] as f64 + 0.587 * papel[1] as f64 + 0.114 * papel[2] as f64;
    let oscuro = |x: usize, y: usize| img.luz(x, y) < luz_papel * 0.72;
    // Las rayas horizontales.
    let largo_min = (w / 4).max(40);
    let mut horizontales: Vec<(usize, usize, usize)> = Vec::new();
    for y in 0..h {
        let (n, a, b) = tramo_mas_largo((0..w).map(|x| oscuro(x, y)));
        if n >= largo_min {
            horizontales.push((y, a, b));
        }
    }
    let mut m = Medida {
        tam_letra: 0.0,
        texto: [20, 20, 20],
        ..Default::default()
    };
    let alto_letra = mediana(
        palabras
            .iter()
            .map(|p| p.alto)
            .filter(|a| *a > 0.0)
            .collect(),
    )?;
    m.tam_letra = alto_letra * ALTO_A_TAM;
    let filas_raya = juntar(&horizontales.iter().map(|t| t.0).collect::<Vec<_>>());
    if filas_raya.len() >= 2 {
        let x0 = horizontales.iter().map(|t| t.1).min().unwrap_or(0);
        let x1 = horizontales.iter().map(|t| t.2).max().unwrap_or(w);
        let (arriba, abajo) = (
            filas_raya[0].0 as usize,
            filas_raya[filas_raya.len() - 1].0 as usize,
        );
        let alto_tabla = abajo.saturating_sub(arriba);
        let mut verticales = Vec::new();
        if alto_tabla > 10 {
            for x in x0.saturating_sub(3)..(x1 + 3).min(w) {
                // Cuantos de la columna son tinta (no un tramo seguido: en
                // un escaneo el grano y el desenfoque la cortan), un poco mas
                // claro que para las horizontales.
                let n = (arriba..=abajo.min(h - 1))
                    .filter(|&y| img.luz(x, y) < luz_papel * 0.80)
                    .count();
                if n as f64 >= alto_tabla as f64 * 0.85 {
                    verticales.push(x);
                }
            }
        }
        let cols = juntar(&verticales);
        if cols.len() >= 2 {
            m.columnas = cols.iter().map(|c| c.0).collect();
            m.filas = filas_raya.iter().map(|f| f.0).collect();
            let grosor =
                mediana(filas_raya.iter().chain(cols.iter()).map(|c| c.1).collect()).unwrap_or(1.0);
            // El color del borde: el centro de las rayas.
            let mut en_raya = Vec::new();
            for &(y, a, b) in horizontales.iter().step_by(1) {
                for x in (a..b).step_by(7) {
                    if oscuro(x, y) {
                        en_raya.push(img.rgb(x, y));
                    }
                }
            }
            m.borde = mediana_rgb(&en_raya).map(|c| (c, grosor));
            // Los fondos y el color del texto, celda a celda.
            let margen = grosor.ceil() as usize + 2;
            let mut tinta = Vec::new();
            for f in 0..m.filas.len() - 1 {
                let mut fila = Vec::new();
                for c in 0..m.columnas.len() - 1 {
                    let (cx0, cx1) = (
                        m.columnas[c] as usize + margen,
                        (m.columnas[c + 1] as usize).saturating_sub(margen),
                    );
                    let (cy0, cy1) = (
                        m.filas[f] as usize + margen,
                        (m.filas[f + 1] as usize).saturating_sub(margen),
                    );
                    let mut claros = Vec::new();
                    if cx1 > cx0 && cy1 > cy0 {
                        let paso = ((cx1 - cx0) * (cy1 - cy0) / 3000).max(1);
                        for i in (0..(cx1 - cx0) * (cy1 - cy0)).step_by(paso) {
                            let (x, y) = (cx0 + i % (cx1 - cx0), cy0 + i / (cx1 - cx0));
                            if oscuro(x, y) {
                                tinta.push(img.rgb(x, y));
                            } else {
                                claros.push(img.rgb(x, y));
                            }
                        }
                    }
                    fila.push(mediana_rgb(&claros));
                }
                m.fondos.push(fila);
            }
            // Una celda tiene fondo si se aparta del de la mayoria de las
            // celdas (no del papel entero: en un escaneo el papel se oscurece
            // hacia un lado y eso no es un fondo), o del papel por color.
            let comun = mediana_rgb(
                &m.fondos
                    .iter()
                    .flatten()
                    .flatten()
                    .copied()
                    .collect::<Vec<_>>(),
            )
            .unwrap_or(papel);
            let tono = |c: [u8; 3]| (c.iter().max().unwrap() - c.iter().min().unwrap()) as f64;
            for f in m.fondos.iter_mut().flatten() {
                *f = f.filter(|c| {
                    distancia(*c, comun) > 14.0
                        && (tono(*c) > tono(comun) + 8.0 || distancia(*c, comun) > 30.0)
                });
            }
            // El color de la letra: el cuarto mas oscuro de su tinta (los
            // bordes de cada letra, suavizados, la aclaran).
            tinta.sort_by_key(|c| c[0] as u32 * 299 + c[1] as u32 * 587 + c[2] as u32 * 114);
            tinta.truncate((tinta.len() / 4).max(1));
            if let Some(t) = mediana_rgb(&tinta) {
                m.texto = t;
            }
            // El aire: de la raya de la izquierda a la primera palabra de
            // cada celda (la mas baja de las que se ven: alineado a la izquierda).
            let mut aires = Vec::new();
            for p in palabras {
                let Some(c) = (0..m.columnas.len() - 1)
                    .find(|&c| p.x >= m.columnas[c] && p.x < m.columnas[c + 1])
                else {
                    continue;
                };
                aires.push(p.x - m.columnas[c]);
            }
            aires.sort_by(|a, b| a.total_cmp(b));
            m.aire = aires.get(aires.len() / 4).copied();
        }
    }
    Some(m)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una tabla de 2 x 3 pintada: rayas grises de 2 px, cabecera azul claro.
    fn pintada() -> (Vec<u8>, u32, u32) {
        let (w, h) = (400u32, 200u32);
        let mut px = vec![255u8; (w * h * 4) as usize];
        let mut poner = |x: u32, y: u32, c: [u8; 3]| {
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 3].copy_from_slice(&c);
        };
        // Cabecera azul claro entre y 20 y 60.
        for y in 20..60 {
            for x in 20..380 {
                poner(x, y, [224, 233, 245]);
            }
        }
        let gris = [90, 90, 96];
        for &y in &[20u32, 60, 100] {
            for x in 20..382 {
                poner(x, y, gris);
                poner(x, y + 1, gris);
            }
        }
        for &x in &[20u32, 140, 260, 380] {
            for y in 20..102 {
                poner(x, y, gris);
                poner(x + 1, y, gris);
            }
        }
        (px, w, h)
    }

    #[test]
    fn la_rejilla_los_fondos_y_el_borde_salen_de_los_pixeles() {
        let (px, w, h) = pintada();
        let palabras = vec![
            Palabra::nueva("Material", 28.0, 30.0, 60.0, 18.0),
            Palabra::nueva("350", 148.0, 70.0, 30.0, 18.0),
        ];
        let m = medir(&px, w, h, &palabras).unwrap();
        assert!(m.cuadra(2, 3), "{m:?}");
        let anchos = m.anchos();
        assert!(anchos.iter().all(|a| (a - 120.0).abs() < 2.0), "{anchos:?}");
        // La cabecera azul, el resto sin fondo.
        assert!(
            m.fondos[0]
                .iter()
                .all(|f| f.is_some_and(|c| c[2] > c[0] + 10)),
            "{:?}",
            m.fondos
        );
        assert!(m.fondos[1].iter().all(Option::is_none));
        let (borde, grosor) = m.borde.unwrap();
        assert!(
            distancia(borde, [90, 90, 96]) < 20.0 && (grosor - 2.0).abs() < 0.6,
            "{borde:?} {grosor}"
        );
        assert!((m.aire.unwrap() - 8.0).abs() < 2.0);
        assert!((m.tam_letra - 18.0 * ALTO_A_TAM).abs() < 0.1);
    }

    #[test]
    fn la_pasada_de_correccion_pone_tildes_y_el_ordinal() {
        assert_eq!(corregir_ocr("Ferreteria El Sol"), "Ferretería El Sol");
        assert_eq!(corregir_ocr("Alambre negro N.0 16"), "Alambre negro N.° 16");
        assert_eq!(corregir_ocr("Edad (dias)"), "Edad (días)");
        assert_eq!(corregir_ocr("GUIA DE PRACTICA"), "GUÍA DE PRÁCTICA");
        // Negativo: lo que ya esta bien, o no esta en la lista, no se toca.
        assert_eq!(
            corregir_ocr("Cemento Portland tipo I 350"),
            "Cemento Portland tipo I 350"
        );
        assert_eq!(corregir_ocr("esta solo el area"), "esta solo el area");
    }

    #[test]
    fn el_titulo_de_fuera_de_la_tabla_sale_como_renglon() {
        let (px, w, h) = pintada();
        let palabras = vec![
            Palabra::nueva("Almacen", 20.0, 2.0, 60.0, 14.0),
            Palabra::nueva("de", 84.0, 2.0, 16.0, 14.0),
            Palabra::nueva("obra", 104.0, 2.0, 30.0, 14.0),
            Palabra::nueva("350", 148.0, 70.0, 30.0, 18.0),
        ];
        let m = medir(&px, w, h, &palabras).unwrap();
        assert_eq!(m.fuera.len(), 1, "{:?}", m.fuera);
        assert_eq!(m.fuera[0].texto, "Almacén de obra");
        assert!((m.fuera[0].x - 20.0).abs() < 0.1 && (m.fuera[0].y - 2.0).abs() < 0.1);
        assert_eq!(m.celdas[1][1], "350");
    }

    #[test]
    fn sin_rayas_no_hay_rejilla_pero_si_letra() {
        let (w, h) = (300u32, 100u32);
        let px = vec![250u8; (w * h * 4) as usize];
        let m = medir(&px, w, h, &[Palabra::nueva("a", 10.0, 10.0, 8.0, 20.0)]).unwrap();
        assert!(m.columnas.is_empty() && m.fondos.is_empty() && m.borde.is_none());
        assert!(m.tam_letra > 0.0);
        // Negativo: sin palabras no se mide nada.
        assert!(medir(&px, w, h, &[]).is_none());
    }
}
