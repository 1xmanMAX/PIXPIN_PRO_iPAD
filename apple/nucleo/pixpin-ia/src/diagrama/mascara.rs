//! **Las mascaras del diagrama**: el papel, la tinta, sus colores, lo que es
//! relleno y lo que es trazo, el esqueleto de los trazos y sus cadenas de
//! pixeles. Todo a la resolucion del recorte (sin reducir: una raya de un
//! pixel tiene que seguir siendo una raya).

/// Un punto en pixeles del recorte (continuo: el pixel (x, y) ocupa
/// `[x, x+1] x [y, y+1]`).
pub type P = (f64, f64);

/// La imagen en RGB sobre blanco (lo transparente es papel).
pub struct Img {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 3]>,
}

impl Img {
    pub fn de_rgba(rgba: &[u8], w: usize, h: usize) -> Img {
        let mut px = vec![[255f32; 3]; w * h];
        for (i, p) in px.iter_mut().enumerate() {
            let Some(q) = rgba.get(i * 4..i * 4 + 4) else {
                break;
            };
            let a = q[3] as f32 / 255.0;
            for c in 0..3 {
                p[c] = q[c] as f32 * a + 255.0 * (1.0 - a);
            }
        }
        Img { w, h, px }
    }
    pub fn en(&self, x: i64, y: i64) -> Option<[f32; 3]> {
        (x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h)
            .then(|| self.px[y as usize * self.w + x as usize])
    }
}

pub fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

pub fn dist_color(a: [f32; 3], b: [f32; 3]) -> f32 {
    d2(a, b).sqrt()
}

/// **El color del papel**: el mas comun (cuantizado) del borde del recorte.
pub fn fondo(img: &Img) -> [f32; 3] {
    type Cuenta = std::collections::HashMap<(u8, u8, u8), (usize, [f64; 3])>;
    fn ver(cuenta: &mut Cuenta, p: [f32; 3]) {
        let k = ((p[0] as u8) >> 4, (p[1] as u8) >> 4, (p[2] as u8) >> 4);
        let e = cuenta.entry(k).or_insert((0, [0.0; 3]));
        e.0 += 1;
        for (suma, v) in e.1.iter_mut().zip(p) {
            *suma += v as f64;
        }
    }
    let mut cuenta: Cuenta = Default::default();
    let (w, h) = (img.w, img.h);
    if w == 0 || h == 0 {
        return [255.0; 3];
    }
    for x in 0..w {
        ver(&mut cuenta, img.px[x]);
        ver(&mut cuenta, img.px[(h - 1) * w + x]);
    }
    for y in 0..h {
        ver(&mut cuenta, img.px[y * w]);
        ver(&mut cuenta, img.px[y * w + w - 1]);
    }
    // Con poco borde (o borde con tinta), tambien el interior cuenta.
    if cuenta.values().map(|e| e.0).max().unwrap_or(0) < (w + h) / 2 {
        for i in (0..w * h).step_by(7) {
            ver(&mut cuenta, img.px[i]);
        }
    }
    let (n, s) = cuenta
        .values()
        .max_by_key(|e| e.0)
        .copied()
        .unwrap_or((1, [255.0; 3]));
    [
        (s[0] / n as f64) as f32,
        (s[1] / n as f64) as f32,
        (s[2] / n as f64) as f32,
    ]
}

/// **Los colores de la tinta**: k-medias sobre una muestra de los pixeles de
/// tinta (los mas alejados del papel pesan mas: el antialias no crea colores
/// propios), con los centros parecidos juntados. Devuelve los centros.
pub fn colores(img: &Img, tinta: &[bool], papel: [f32; 3], k: usize) -> Vec<[f32; 3]> {
    let idx: Vec<usize> = (0..tinta.len()).filter(|&i| tinta[i]).collect();
    if idx.is_empty() {
        return Vec::new();
    }
    let paso = (idx.len() / 30_000).max(1);
    // Solo los «de verdad»: los pixeles cuyo color esta lejos del papel o
    // que son el centro de su trazo (mas lejos del papel que sus vecinos).
    let muestra: Vec<[f32; 3]> = idx
        .iter()
        .step_by(paso)
        .map(|&i| img.px[i])
        .filter(|&c| dist_color(c, papel) > 70.0)
        .collect();
    if muestra.is_empty() {
        return vec![media(idx.iter().map(|&i| img.px[i]))];
    }
    // Inicio: el mas lejano del papel y luego el mas lejano de los elegidos.
    let mut centros: Vec<[f32; 3]> = Vec::new();
    let primero = muestra
        .iter()
        .copied()
        .max_by(|a, b| dist_color(*a, papel).total_cmp(&dist_color(*b, papel)))
        .unwrap();
    centros.push(primero);
    while centros.len() < k {
        let (mejor, d) = muestra
            .iter()
            .map(|&c| {
                (
                    c,
                    centros.iter().map(|&q| d2(c, q)).fold(f32::MAX, f32::min),
                )
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        if d < 30.0 * 30.0 {
            break;
        }
        centros.push(mejor);
    }
    for _ in 0..8 {
        let mut suma = vec![[0f64; 3]; centros.len()];
        let mut n = vec![0usize; centros.len()];
        for &c in &muestra {
            let j = cercano(&centros, c);
            for q in 0..3 {
                suma[j][q] += c[q] as f64;
            }
            n[j] += 1;
        }
        for j in 0..centros.len() {
            if n[j] > 0 {
                centros[j] = [
                    (suma[j][0] / n[j] as f64) as f32,
                    (suma[j][1] / n[j] as f64) as f32,
                    (suma[j][2] / n[j] as f64) as f32,
                ];
            }
        }
    }
    // Juntar los parecidos.
    let mut i = 0;
    while i < centros.len() {
        let mut j = i + 1;
        while j < centros.len() {
            if dist_color(centros[i], centros[j]) < 40.0 {
                let c = centros.remove(j);
                for q in 0..3 {
                    centros[i][q] = (centros[i][q] + c[q]) / 2.0;
                }
            } else {
                j += 1;
            }
        }
        i += 1;
    }
    centros
}

pub fn media(v: impl Iterator<Item = [f32; 3]>) -> [f32; 3] {
    let (mut s, mut n) = ([0f64; 3], 0usize);
    for c in v {
        for q in 0..3 {
            s[q] += c[q] as f64;
        }
        n += 1;
    }
    if n == 0 {
        return [0.0; 3];
    }
    [
        (s[0] / n as f64) as f32,
        (s[1] / n as f64) as f32,
        (s[2] / n as f64) as f32,
    ]
}

pub fn cercano(centros: &[[f32; 3]], c: [f32; 3]) -> usize {
    let mut mejor = 0;
    let mut d = f32::MAX;
    for (j, q) in centros.iter().enumerate() {
        let e = d2(c, *q);
        if e < d {
            d = e;
            mejor = j;
        }
    }
    mejor
}

/// **La clase de color de un pixel de tinta**: el centro al que se parece,
/// sin contar el papel que se le mezcla en el borde (el antialias de una
/// raya azul es un azul claro: se compara su direccion desde el papel).
pub fn clase(centros: &[[f32; 3]], papel: [f32; 3], c: [f32; 3]) -> usize {
    let mut mejor = 0;
    let mut d = f32::MAX;
    for (j, q) in centros.iter().enumerate() {
        // c = papel + t (q - papel): el t que mejor lo explica.
        let v = [q[0] - papel[0], q[1] - papel[1], q[2] - papel[2]];
        let u = [c[0] - papel[0], c[1] - papel[1], c[2] - papel[2]];
        let vv = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
        let t = if vv > 1.0 {
            ((u[0] * v[0] + u[1] * v[1] + u[2] * v[2]) / vv).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let r = [
            papel[0] + t * v[0],
            papel[1] + t * v[1],
            papel[2] + t * v[2],
        ];
        // Lo que se aparta de la mezcla, y un poco lo que se aparta del
        // propio color (que una mezcla muy clara no sea de cualquiera).
        let e = d2(c, r) + 0.15 * d2(c, *q);
        if e < d {
            d = e;
            mejor = j;
        }
    }
    mejor
}

/// **Distancia de cada pixel de la mascara a lo de fuera** (chamfer 3-4,
/// en pixeles). Fuera de la mascara, 0.
pub fn distancia(m: &[bool], w: usize, h: usize) -> Vec<f32> {
    let inf = 1e6f32;
    let mut d: Vec<f32> = m.iter().map(|&b| if b { inf } else { 0.0 }).collect();
    let en = |d: &Vec<f32>, x: isize, y: isize| -> f32 {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            0.0
        } else {
            d[y as usize * w + x as usize]
        }
    };
    for y in 0..h as isize {
        for x in 0..w as isize {
            let i = y as usize * w + x as usize;
            if d[i] == 0.0 {
                continue;
            }
            let v = d[i]
                .min(en(&d, x - 1, y) + 3.0)
                .min(en(&d, x, y - 1) + 3.0)
                .min(en(&d, x - 1, y - 1) + 4.0)
                .min(en(&d, x + 1, y - 1) + 4.0);
            d[i] = v;
        }
    }
    for y in (0..h as isize).rev() {
        for x in (0..w as isize).rev() {
            let i = y as usize * w + x as usize;
            if d[i] == 0.0 {
                continue;
            }
            let v = d[i]
                .min(en(&d, x + 1, y) + 3.0)
                .min(en(&d, x, y + 1) + 3.0)
                .min(en(&d, x + 1, y + 1) + 4.0)
                .min(en(&d, x - 1, y + 1) + 4.0);
            d[i] = v;
        }
    }
    d.iter().map(|v| v / 3.0).collect()
}

/// Lo que esta a menos de `r` de la mascara (dilatacion redonda).
pub fn dilatar(m: &[bool], w: usize, h: usize, r: f32) -> Vec<bool> {
    let fuera: Vec<bool> = m.iter().map(|b| !b).collect();
    distancia(&fuera, w, h).iter().map(|&d| d <= r).collect()
}

/// Lo que esta a mas de `r` del borde (erosion redonda).
pub fn erosionar(m: &[bool], w: usize, h: usize, r: f32) -> Vec<bool> {
    distancia(m, w, h).iter().map(|&d| d > r).collect()
}

/// Componentes conexas (8 vecinos): los indices de sus pixeles.
pub fn componentes(m: &[bool], w: usize, h: usize) -> Vec<Vec<usize>> {
    let mut visto = vec![false; w * h];
    let mut salida = Vec::new();
    let mut pila = Vec::new();
    for i in 0..w * h {
        if !m[i] || visto[i] {
            continue;
        }
        let mut c = Vec::new();
        visto[i] = true;
        pila.push(i);
        while let Some(j) = pila.pop() {
            c.push(j);
            let (x, y) = ((j % w) as isize, (j / w) as isize);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (xx, yy) = (x + dx, y + dy);
                    if xx < 0 || yy < 0 || xx >= w as isize || yy >= h as isize {
                        continue;
                    }
                    let k = yy as usize * w + xx as usize;
                    if m[k] && !visto[k] {
                        visto[k] = true;
                        pila.push(k);
                    }
                }
            }
        }
        salida.push(c);
    }
    salida
}

/// La caja de unos pixeles: (x0, y0, x1, y1) inclusiva.
pub fn caja_de(px: &[usize], w: usize) -> (usize, usize, usize, usize) {
    let mut c = (usize::MAX, usize::MAX, 0, 0);
    for &i in px {
        let (x, y) = (i % w, i / w);
        c.0 = c.0.min(x);
        c.1 = c.1.min(y);
        c.2 = c.2.max(x);
        c.3 = c.3.max(y);
    }
    c
}

/// **Adelgazamiento de Zhang-Suen**: el esqueleto de un pixel de ancho.
pub fn esqueleto(m: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut s = m.to_vec();
    // Sin tinta en el borde del recorte (los vecinos se leen sin mirar).
    for x in 0..w {
        s[x] = false;
        s[(h - 1) * w + x] = false;
    }
    for y in 0..h {
        s[y * w] = false;
        s[y * w + w - 1] = false;
    }
    let mut cambio = true;
    let mut quitar = Vec::new();
    while cambio {
        cambio = false;
        for paso in 0..2 {
            quitar.clear();
            for y in 1..h.saturating_sub(1) {
                for x in 1..w.saturating_sub(1) {
                    let i = y * w + x;
                    if !s[i] {
                        continue;
                    }
                    // p2..p9: N, NE, E, SE, S, SO, O, NO.
                    let v = [
                        s[i - w],
                        s[i - w + 1],
                        s[i + 1],
                        s[i + w + 1],
                        s[i + w],
                        s[i + w - 1],
                        s[i - 1],
                        s[i - w - 1],
                    ];
                    let b = v.iter().filter(|&&q| q).count();
                    if !(2..=6).contains(&b) {
                        continue;
                    }
                    let a = (0..8).filter(|&k| !v[k] && v[(k + 1) % 8]).count();
                    if a != 1 {
                        continue;
                    }
                    let (p2, p4, p6, p8) = (v[0], v[2], v[4], v[6]);
                    let ok = if paso == 0 {
                        !(p2 && p4 && p6) && !(p4 && p6 && p8)
                    } else {
                        !(p2 && p4 && p8) && !(p2 && p6 && p8)
                    };
                    if ok {
                        quitar.push(i);
                    }
                }
            }
            if !quitar.is_empty() {
                cambio = true;
                for &i in &quitar {
                    s[i] = false;
                }
            }
        }
    }
    // Las escaleras: un pixel con dos vecinos en L que se tocan entre si
    // sobra (deja el esqueleto de 8-conexion limpia, sin falsos cruces).
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let i = y * w + x;
            if !s[i] {
                continue;
            }
            let (n, e, so, o) = (s[i - w], s[i + 1], s[i + w], s[i - 1]);
            let esquina = (n && e && !s[i - w + 1])
                || (e && so && !s[i + w + 1])
                || (so && o && !s[i + w - 1])
                || (o && n && !s[i - w - 1]);
            if esquina {
                let vecinos = [
                    s[i - w],
                    s[i - w + 1],
                    s[i + 1],
                    s[i + w + 1],
                    s[i + w],
                    s[i + w - 1],
                    s[i - 1],
                    s[i - w - 1],
                ]
                .iter()
                .filter(|&&q| q)
                .count();
                if vecinos == 2 {
                    s[i] = false;
                }
            }
        }
    }
    s
}

const VECINOS: [(isize, isize); 8] = [
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, 0),
    (1, -1),
    (1, 1),
    (-1, 1),
    (-1, -1),
];

fn vecinos_de(s: &[bool], w: usize, h: usize, i: usize) -> Vec<usize> {
    let (x, y) = ((i % w) as isize, (i / w) as isize);
    let mut v = Vec::with_capacity(8);
    for (dx, dy) in VECINOS {
        let (xx, yy) = (x + dx, y + dy);
        if xx < 0 || yy < 0 || xx >= w as isize || yy >= h as isize {
            continue;
        }
        let k = yy as usize * w + xx as usize;
        if s[k] {
            v.push(k);
        }
    }
    v
}

/// **Las cadenas del esqueleto**: de cruce (o punta) a cruce (o punta), y
/// los anillos sueltos. Cada cadena en centros de pixel. Los cruces (3 o mas
/// vecinos) se reparten: cada cadena que llega a uno lo incluye como punta.
pub fn cadenas(s: &[bool], w: usize, h: usize) -> Vec<Vec<P>> {
    let n_vec: Vec<usize> = (0..w * h)
        .map(|i| {
            if s[i] {
                vecinos_de(s, w, h, i).len()
            } else {
                0
            }
        })
        .collect();
    // Nodo: cruce (3 o mas ramas al dar la vuelta) o punta (1). Contar
    // vecinos no vale: en una escalera un pixel tiene 3 y no es cruce.
    let cruces: Vec<u8> = (0..w * h)
        .map(|i| if s[i] { ramas(s, w, h, i) } else { 0 })
        .collect();
    let es_nodo = |i: usize| n_vec[i] != 2 && (cruces[i] != 2 || n_vec[i] < 2);
    let mut visto = vec![false; w * h];
    let centro = |i: usize| ((i % w) as f64 + 0.5, (i / w) as f64 + 0.5);
    let mut salida: Vec<Vec<P>> = Vec::new();
    // Recorre desde el nodo `a` por su vecino `b`.
    let recorrer = |a: usize, b: usize, visto: &mut Vec<bool>| -> Option<Vec<P>> {
        if visto[b] && !es_nodo(b) {
            return None;
        }
        let mut camino = vec![centro(a)];
        let (mut prev, mut cur) = (a, b);
        loop {
            camino.push(centro(cur));
            if es_nodo(cur) {
                break;
            }
            visto[cur] = true;
            // El siguiente: un vecino sin ver que no sea de donde viene;
            // primero los de 4-conexion.
            let sig = vecinos_de(s, w, h, cur)
                .into_iter()
                .filter(|&k| k != prev && (es_nodo(k) || !visto[k]))
                .filter(|&k| k != a || camino.len() > 3)
                .min_by_key(|&k| {
                    let (dx, dy) = (
                        (k % w) as isize - (cur % w) as isize,
                        (k / w) as isize - (cur / w) as isize,
                    );
                    (dx.abs() + dy.abs()) as usize
                });
            match sig {
                Some(k) => {
                    prev = cur;
                    cur = k;
                }
                None => break,
            }
        }
        Some(camino)
    };
    // Una cadena de un nodo a un nodo vecino directo se recorreria dos
    // veces (una desde cada uno): se apuntan los pares hechos.
    let mut pares: std::collections::HashSet<(usize, usize)> = Default::default();
    for i in 0..w * h {
        if !s[i] || !es_nodo(i) || n_vec[i] == 0 {
            continue;
        }
        for b in vecinos_de(s, w, h, i) {
            if es_nodo(b) {
                let par = (i.min(b), i.max(b));
                if !pares.insert(par) {
                    continue;
                }
                salida.push(vec![centro(i), centro(b)]);
                continue;
            }
            if visto[b] {
                continue;
            }
            if let Some(c) = recorrer(i, b, &mut visto) {
                salida.push(c);
            }
        }
    }
    // Anillos sin nodos.
    for i in 0..w * h {
        if s[i] && !visto[i] && !es_nodo(i) {
            visto[i] = true;
            let v = vecinos_de(s, w, h, i);
            if let Some(&b) = v.first()
                && let Some(mut c) = recorrer(i, b, &mut visto)
            {
                // Cerrarlo.
                if let Some(&p) = c.first() {
                    c.push(p);
                }
                salida.push(c);
            }
        }
    }
    salida
}

/// Cuantas ramas salen de un pixel del esqueleto: los pasos de vacio a
/// lleno al dar la vuelta por sus 8 vecinos (el «crossing number»).
fn ramas(s: &[bool], w: usize, h: usize, i: usize) -> u8 {
    let (x, y) = ((i % w) as isize, (i / w) as isize);
    const ANILLO: [(isize, isize); 8] = [
        (0, -1),
        (1, -1),
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
    ];
    let en = |k: usize| {
        let (xx, yy) = (x + ANILLO[k].0, y + ANILLO[k].1);
        xx >= 0 && yy >= 0 && xx < w as isize && yy < h as isize && s[yy as usize * w + xx as usize]
    };
    (0..8).filter(|&k| !en(k) && en((k + 1) % 8)).count() as u8
}
