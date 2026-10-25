//! **Seccionar una hoja**: de las palabras con caja y de lo que se ve en la
//! imagen, sus zonas con su tipo, su caja y su orden de lectura. Sin
//! modelo: es geometria y unas cuentas, y en una hoja entera tarda decenas
//! de ms (prueba `seccionar_es_rapido`).
//!
//! ## Como
//!
//! 1. **Lo que no es texto** ([`super::imagen`]): fotos (medios tonos),
//!    rayas largas y finas. Las rayas que se cruzan forman **tablas
//!    rayadas** (o tres o mas horizontales del mismo largo, una tabla «de
//!    libro»); una horizontal y una vertical en L son **ejes**: una grafica.
//!    Las verticales largas que quedan **separan columnas**.
//! 2. **Renglones**: las palabras por filas (se solapan en vertical) y cada
//!    fila partida donde hay un hueco grande o una raya: asi el renglon de la
//!    columna izquierda no se pega al de la derecha.
//! 3. **La tinta que queda** sin palabras ni rayas son manchas: grandes,
//!    dibujos (o fotos, si tienen medios tonos); pequenas, trozos de una
//!    formula que el OCR no leyo (la raya de una fraccion, una raiz).
//! 4. **Orden de lectura**: corte XY recursivo. Primero se busca un pasillo
//!    vertical que cruce todo el bloque (o una raya separadora); si no, se
//!    parte en franjas horizontales y se juntan las franjas seguidas que
//!    comparten un pasillo (las dos columnas bajo un titulo a todo lo
//!    ancho). Un bloque partido en columnas de trozos cortos y alineados no
//!    son columnas de texto: es una **tabla sin rayas**.
//! 5. **Cada bloque, renglon a renglon**: formula (simbolos, `=`, pocas
//!    palabras, centrada), encabezado (letra mayor o negrita), o parrafo
//!    (se corta en un hueco mayor que el interlineado, una sangria o un
//!    renglon corto que acaba en punto).

use super::imagen::{self, Analisis, Mancha, Raya};
use super::{Alineacion, Caja, PalabraHoja, Renglon, Seccionado, Tipo, Zona, mediana};
use crate::tabla_escaneada;

/// Un hueco mayor que esto (en altos de letra) parte un renglon.
pub const HUECO_RENGLON: f64 = 1.3;
/// Un pasillo entre columnas mide al menos esto (en altos de letra).
pub const PASILLO: f64 = 1.2;

/// **Secciona la hoja.** `rgba` es la imagen de la pagina y `palabras` lo
/// que leyo el OCR (o la capa de texto del PDF), en pixeles de esa imagen.
/// Nunca falla: una hoja en blanco da cero zonas.
pub fn seccionar(rgba: &[u8], ancho: u32, alto: u32, mut palabras: Vec<PalabraHoja>) -> Seccionado {
    let a = imagen::analizar(rgba, ancho, alto);
    for p in &mut palabras {
        if p.color.is_none() {
            p.color = imagen::color_de_tinta(rgba, ancho, alto, &p.caja, a.umbral);
        }
    }
    seccionar_con(&a, ancho, alto, palabras)
}

/// Lo mismo, con la imagen ya analizada.
pub fn seccionar_con(a: &Analisis, ancho: u32, alto: u32, palabras: Vec<PalabraHoja>) -> Seccionado {
    let (pw, ph) = (ancho as f64, alto as f64);
    let mut palabras: Vec<PalabraHoja> = palabras
        .into_iter()
        .filter(|p| !p.texto.trim().is_empty() && p.caja.w > 0.0 && p.caja.h > 0.0)
        .map(|mut p| {
            p.caja = p.caja.dentro(pw, ph);
            p
        })
        .collect();
    let hm = mediana(palabras.iter().map(|p| p.caja.h).collect()).unwrap_or(ph * 0.012).max(4.0);

    // 1. Fotos, y lo que el OCR leyo dentro de ellas fuera.
    let mut fotos = imagen::fotos(a, 4.0 * hm);
    // Un fondo de color casi sin texto (un logo en su recuadro gris) tambien
    // es una imagen; con texto, o cruzado por rayas (la cabecera gris de una
    // tabla), no.
    let brutas = imagen::rayas(a, 0.05);
    for f in imagen::fondos(a, 4.0 * hm) {
        let texto: f64 = palabras.iter().filter(|p| f.contiene(p.caja.cx(), p.caja.cy())).map(|p| p.caja.area()).sum();
        let rayado = brutas.iter().any(|r| r.caja().comun(&f.crecer(2.0)) > 0.0);
        if texto < f.area() * 0.25 && !rayado && !fotos.iter().any(|x| x.comun(&f) > 0.0) {
            fotos.push(f);
        }
    }
    palabras.retain(|p| !fotos.iter().any(|f| p.caja.dentro_de(f) > 0.5));
    // Negrita de las palabras del OCR: mas tinta por area que las demas.
    let densidades: Vec<f64> = palabras.iter().map(|p| a.densidad(&p.caja)).collect();
    let dens_media = mediana(densidades.clone()).unwrap_or(0.0);
    if dens_media > 0.0 {
        for (p, d) in palabras.iter_mut().zip(&densidades) {
            if !p.negrita && *d > dens_media * 1.38  {
                p.negrita = true;
            }
        }
    }

    // 2. Rayas, sin las de dentro de las fotos ni las que tachan texto.
    let mut rayas: Vec<Raya> = brutas
        .into_iter()
        .filter(|r| !fotos.iter().any(|f| r.caja().crecer(1.0).dentro_de(f) > 0.5))
        .filter(|r| {
            let c = r.caja();
            // Una «raya» que cae dentro de las palabras es la propia letra (los
            // trazos de una palabra larga se tocan al reducir la hoja), o un
            // tachado: no es una raya del dibujo.
            let largo = if r.vertical { c.h } else { c.w };
            let dentro: f64 = palabras
                .iter()
                .map(|p| {
                    let q = p.caja.crecer(p.caja.h * 0.15);
                    if r.vertical { c.solape_y(&q) * (c.solape_x(&q) > 0.0) as u8 as f64 } else { c.solape_x(&q) * (c.solape_y(&q) > 0.0) as u8 as f64 }
                })
                .sum();
            dentro < largo * 0.4
        })
        .collect();
    rayas.sort_by(|a, b| a.y0.partial_cmp(&b.y0).unwrap_or(std::cmp::Ordering::Equal));
    // El adorno de un separador («———※———»): el OCR lo lee como «x» o «•x»;
    // es un simbolo entre dos rayas, no texto (sale como imagen).
    palabras.retain(|p| {
        let corto = p.texto.trim().chars().count() <= 2;
        let entre = |izq: bool| {
            rayas.iter().any(|r| {
                !r.vertical && (r.y0 - p.caja.cy()).abs() < hm && if izq { (p.caja.x - r.x1).abs() < hm * 2.0 } else { (r.x0 - p.caja.x1()).abs() < hm * 2.0 }
            })
        };
        !(corto && entre(true) && entre(false))
    });

    // 3. Tablas rayadas y graficas (ejes) de las rayas que se tocan.
    let (tablas_r, graficas, sueltas) = agrupar_rayas(&rayas, &palabras, hm);

    // Lo que ocupan las tablas y graficas: sus palabras van dentro.
    let mut ocupado: Vec<(Caja, usize)> = Vec::new(); // (caja, indice en figuras)
    let mut figuras: Vec<Figura> = Vec::new();
    for f in &fotos {
        figuras.push(Figura { tipo: Tipo::Foto, caja: *f, confianza: 0.85, rayas: Vec::new(), palabras: Vec::new() });
    }
    for g in &graficas {
        figuras.push(Figura { tipo: Tipo::Grafica, caja: g.0, confianza: 0.7, rayas: g.1.clone(), palabras: Vec::new() });
    }
    let mut tablas: Vec<TablaR> = tablas_r.iter().map(|t| TablaR { caja: t.0, rayas: t.1.clone(), palabras: Vec::new(), iconos: Vec::new() }).collect();
    // Una foto dentro de una tabla es un icono de una celda.
    figuras.retain(|f| match tablas.iter_mut().find(|t| t.caja.contiene(f.caja.cx(), f.caja.cy())) {
        Some(t) => {
            t.iconos.push(f.caja);
            false
        }
        None => true,
    });
    for (i, f) in figuras.iter().enumerate() {
        ocupado.push((f.caja, i));
    }
    let mut libres: Vec<PalabraHoja> = Vec::new();
    for p in palabras {
        if let Some(t) = tablas.iter_mut().find(|t| t.caja.crecer(hm * 0.3).contiene(p.caja.cx(), p.caja.cy())) {
            t.palabras.push(p);
        } else if ocupado.iter().any(|(c, _)| c.crecer(hm).contiene(p.caja.cx(), p.caja.cy())) {
            // Los rotulos de una grafica van con ella.
        } else {
            libres.push(p);
        }
    }

    // 4. Manchas de tinta sin texto.
    let mut quitar: Vec<Caja> = libres.iter().map(|p| p.caja.crecer(hm * 0.15)).collect();
    quitar.extend(fotos.iter().copied());
    quitar.extend(tablas.iter().flat_map(|t| t.palabras.iter().map(|p| p.caja.crecer(hm * 0.15))));
    quitar.extend(figuras.iter().filter(|f| f.tipo == Tipo::Grafica).map(|f| f.caja));
    let manchas = imagen::manchas(a, &quitar, &rayas, hm * 0.8);
    let mut restos: Vec<Caja> = Vec::new();
    for m in manchas {
        let c = m.caja;
        if c.w < hm * 0.5 && c.h < hm * 0.5 {
            continue; // grano, un punto
        }
        // Dentro de una tabla: un icono de una celda (o grano).
        if let Some(t) = tablas.iter_mut().find(|t| t.caja.contiene(c.cx(), c.cy())) {
            if c.w >= hm * 0.8 && c.h >= hm * 0.8 {
                t.iconos.push(c);
            }
            continue;
        }
        if c.w >= hm * 5.0 && c.h >= hm * 3.5 {
            figuras.push(figura_de_mancha(&m, hm));
        } else {
            restos.push(c);
        }
    }
    // Las rayas sueltas cortas (una raya de fraccion grande, un subrayado):
    // como restos; las largas, lineas o separadores.
    let mut lineas: Vec<Raya> = Vec::new();
    for r in sueltas {
        let largo_rel = if r.vertical { r.largo() / ph } else { r.largo() / pw };
        if largo_rel >= 0.12 {
            lineas.push(r);
        } else {
            restos.push(r.caja());
        }
    }
    unir_figuras(&mut figuras, hm);
    // Los restos dentro de una figura son de ella.
    restos.retain(|r| !figuras.iter().any(|f| r.dentro_de(&f.caja.crecer(hm * 0.5)) > 0.6));
    // Los rotulos cortos dentro de un dibujo (A, B, 2a, 45°) son suyos: van
    // con el (sus letras, que un diagrama pone junto a sus puntos), no
    // sueltos como parrafos de una letra (15-oct, la lamina de geometria).
    for f in figuras.iter_mut().filter(|f| f.tipo == Tipo::Dibujo) {
        let caja = f.caja.crecer(hm * 0.3);
        let (suyas, otras): (Vec<PalabraHoja>, Vec<PalabraHoja>) =
            libres.drain(..).partition(|p| caja.contiene(p.caja.cx(), p.caja.cy()) && p.texto.chars().count() <= 6);
        libres = otras;
        for p in &suyas {
            f.caja = f.caja.union(&p.caja);
        }
        f.palabras.extend(suyas);
    }

    // 5. Renglones.
    let separadores: Vec<Raya> = lineas.iter().filter(|r| r.vertical).copied().collect();
    let renglones = renglones(&libres, &separadores, hm);

    // 6. Orden de lectura.
    let mut atomos: Vec<Atomo> = Vec::new();
    for (i, r) in renglones.iter().enumerate() {
        atomos.push(Atomo { caja: r.caja, clase: Clase::Texto(i) });
    }
    for (i, f) in figuras.iter().enumerate() {
        atomos.push(Atomo { caja: f.caja, clase: Clase::Figura(i) });
    }
    for (i, t) in tablas.iter().enumerate() {
        atomos.push(Atomo { caja: t.caja, clase: Clase::Tabla(i) });
    }
    for (i, r) in restos.iter().enumerate() {
        atomos.push(Atomo { caja: *r, clase: Clase::Resto(i) });
    }
    let ids: Vec<usize> = (0..atomos.len()).collect();
    let mut hojas: Vec<Hoja> = Vec::new();
    let cx = Corte { atomos: &atomos, renglones: &renglones, separadores: &separadores, hm };
    cx.cortar(&ids, 0, 0, &mut hojas);

    // 7. Cada bloque a sus zonas.
    // El alto de letra del cuerpo: el de la palabra mediana (los renglones
    // de los titulos, aunque sean muchos, no lo mueven).
    let alto_cuerpo = hm;
    let n_columnas = hojas.iter().map(|h| h.columna + 1).max().unwrap_or(1);
    let mut zonas: Vec<Zona> = Vec::new();
    for hoja in &hojas {
        let columna = hoja.columna;
        let bloque = Bloque { atomos: &atomos, renglones: &renglones, figuras: &figuras, tablas: &tablas, restos: &restos, hm: alto_cuerpo };
        for mut z in bloque.zonas(hoja) {
            z.columna = columna;
            zonas.push(z);
        }
    }
    for l in &lineas {
        let c = l.caja();
        zonas.push(Zona {
            id: 0,
            tipo: Tipo::Linea,
            caja: c,
            orden: 0,
            columna: 0,
            confianza: 0.95,
            renglones: Vec::new(),
            alineacion: Alineacion::Izquierda,
            tabla: None,
            rayas: vec![[l.x0, l.y0, l.x1, l.y1, l.grosor]],
            dentro: Vec::new(),
            panel: None,
        });
    }
    for (i, z) in zonas.iter_mut().enumerate() {
        z.id = i;
        z.orden = i;
    }
    Seccionado {
        ancho,
        alto,
        zonas,
        columnas: n_columnas,
        alto_letra: alto_cuerpo,
        paneles: Vec::new(),
    }
}

/// Una figura (foto, grafica, dibujo o dudosa) antes de ser zona.
#[derive(Debug, Clone)]
struct Figura {
    tipo: Tipo,
    caja: Caja,
    confianza: f32,
    rayas: Vec<[f64; 5]>,
    /// Sus rotulos (las letras de un dibujo).
    palabras: Vec<PalabraHoja>,
}

fn figura_de_mancha(m: &Mancha, _hm: f64) -> Figura {
    let e = m.est;
    if e.parece_foto() {
        Figura { tipo: Tipo::Foto, caja: m.caja, confianza: (0.55 + e.medios as f32).min(0.9), rayas: Vec::new(), palabras: Vec::new() }
    } else if e.tinta < 0.01 {
        // Sin tinta de verdad, solo grises claros (una marca de agua, un
        // sombreado): calcarlo lo oscureceria; va como imagen.
        Figura { tipo: Tipo::Foto, caja: m.caja, confianza: 0.6, rayas: Vec::new(), palabras: Vec::new() }
    } else if e.tinta < 0.22 && e.medios < 0.12 {
        // Trazos finos sobre papel: un dibujo de lineas.
        Figura { tipo: Tipo::Dibujo, caja: m.caja, confianza: 0.6, rayas: Vec::new(), palabras: Vec::new() }
    } else {
        Figura { tipo: Tipo::Dudosa, caja: m.caja, confianza: 0.35, rayas: Vec::new(), palabras: Vec::new() }
    }
}

/// Junta las figuras que se pisan (un dibujo y su grafica, dos trozos de una
/// foto). Gana el tipo de la mas segura.
fn unir_figuras(f: &mut Vec<Figura>, hm: f64) {
    loop {
        let mut junto = false;
        'fuera: for i in 0..f.len() {
            for j in i + 1..f.len() {
                if f[i].caja.crecer(hm * 0.3).comun(&f[j].caja) > 0.0 {
                    let b = f.remove(j);
                    let a = &mut f[i];
                    a.caja = a.caja.union(&b.caja);
                    a.rayas.extend(b.rayas);
                    a.palabras.extend(b.palabras);
                    if b.tipo == Tipo::Grafica || (a.tipo != Tipo::Grafica && b.confianza > a.confianza) {
                        a.tipo = b.tipo;
                        a.confianza = b.confianza;
                    }
                    junto = true;
                    break 'fuera;
                }
            }
        }
        if !junto {
            break;
        }
    }
}

type ConRayas = (Caja, Vec<[f64; 5]>);

/// Las rayas que se tocan, en grupos: tablas rayadas, ejes de graficas y
/// las sueltas.
fn agrupar_rayas(rayas: &[Raya], palabras: &[PalabraHoja], hm: f64) -> (Vec<ConRayas>, Vec<ConRayas>, Vec<Raya>) {
    let n = rayas.len();
    let tol = hm * 0.4;
    let tocan = |a: &Raya, b: &Raya| a.caja().crecer(tol).comun(&b.caja().crecer(tol)) > 0.0;
    // Union-find.
    let mut padre: Vec<usize> = (0..n).collect();
    fn raiz(p: &mut Vec<usize>, i: usize) -> usize {
        let mut i = i;
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for i in 0..n {
        for j in i + 1..n {
            if rayas[i].vertical != rayas[j].vertical && tocan(&rayas[i], &rayas[j]) {
                let (a, b) = (raiz(&mut padre, i), raiz(&mut padre, j));
                padre[a] = b;
            }
        }
    }
    // Tablas «de libro»: horizontales del mismo largo, una bajo otra, sin
    // verticales.
    // Las que ya tocan una vertical son de una tabla rayada (o un recuadro):
    // esas no se juntan por su largo.
    let con_vertical: Vec<bool> = (0..n).map(|i| !rayas[i].vertical && (0..n).any(|j| rayas[j].vertical && tocan(&rayas[i], &rayas[j]))).collect();
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (&rayas[i], &rayas[j]);
            if a.vertical || b.vertical || con_vertical[i] || con_vertical[j] {
                continue;
            }
            let mismo = (a.x0 - b.x0).abs() < hm * 1.5 && (a.x1 - b.x1).abs() < hm * 1.5;
            let cerca = (a.y0 - b.y0).abs() < hm * 14.0;
            if mismo && cerca {
                let (x, y) = (raiz(&mut padre, i), raiz(&mut padre, j));
                padre[x] = y;
            }
        }
    }
    let mut grupos: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for i in 0..n {
        let r = raiz(&mut padre, i);
        grupos.entry(r).or_default().push(i);
    }
    let (mut tablas, mut graficas, mut sueltas) = (Vec::new(), Vec::new(), Vec::new());
    for (_, g) in grupos {
        let h: Vec<&Raya> = g.iter().map(|&i| &rayas[i]).filter(|r| !r.vertical).collect();
        let v: Vec<&Raya> = g.iter().map(|&i| &rayas[i]).filter(|r| r.vertical).collect();
        let caja = g.iter().map(|&i| rayas[i].caja()).reduce(|a, b| a.union(&b)).unwrap_or_default();
        let lista: Vec<[f64; 5]> = g.iter().map(|&i| rayas[i]).map(|r| [r.x0, r.y0, r.x1, r.y1, r.grosor]).collect();
        let dentro = palabras.iter().filter(|p| caja.contiene(p.caja.cx(), p.caja.cy())).count();
        if h.len() == 1 && v.len() == 1 && es_esquina(h[0], v[0], hm) {
            // Ejes: lo que esta a su izquierda y debajo (los numeros) va con
            // la grafica.
            let ejes = Caja::de_esquinas(caja.x - hm * 3.0, caja.y - hm, caja.x1() + hm, caja.y1() + hm * 2.5);
            graficas.push((ejes, lista));
        } else if (h.len() >= 2 && v.len() >= 2 || h.len() >= 3) && dentro >= 2 && caja.h > hm * 1.5 {
            tablas.push((caja, lista));
        } else if h.len() >= 2 && v.len() >= 2 && dentro < 2 && caja.w > hm * 4.0 && caja.h > hm * 4.0 {
            // Una rejilla sin texto: papel cuadriculado de una grafica o un
            // dibujo; mejor calcarla.
            graficas.push((caja, lista));
        } else {
            sueltas.extend(g.iter().map(|&i| rayas[i]));
        }
    }
    (tablas, graficas, sueltas)
}

/// Si una horizontal y una vertical forman una L (los ejes de una grafica):
/// la vertical baja hasta el extremo izquierdo de la horizontal, o la cruza
/// cerca de el.
fn es_esquina(h: &Raya, v: &Raya, hm: f64) -> bool {
    let cerca_x = v.x0 >= h.x0 - hm && v.x0 <= h.x0 + (h.x1 - h.x0) * 0.25;
    let cerca_y = h.y0 >= v.y0 + (v.y1 - v.y0) * 0.6 && h.y0 <= v.y1 + hm;
    cerca_x && cerca_y && h.largo() > hm * 4.0 && v.largo() > hm * 4.0
}

/// **Los renglones**: filas por solape vertical, partidas por huecos grandes
/// y por las rayas separadoras.
pub fn renglones(palabras: &[PalabraHoja], separadores: &[Raya], hm: f64) -> Vec<Renglon> {
    let mut ps: Vec<&PalabraHoja> = palabras.iter().collect();
    ps.sort_by(|a, b| a.caja.cy().partial_cmp(&b.caja.cy()).unwrap_or(std::cmp::Ordering::Equal));
    let mut filas: Vec<Vec<&PalabraHoja>> = Vec::new();
    for p in ps {
        let mut puesta = false;
        // Contra la ultima fila y la anterior (dos columnas con renglones
        // algo desfasados).
        for f in filas.iter_mut().rev().take(3) {
            let (y0, y1) = f.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.caja.y), b.max(q.caja.y1())));
            let alto_f = mediana(f.iter().map(|q| q.caja.h).collect()).unwrap_or(hm);
            let solape = (p.caja.y1().min(y1) - p.caja.y.max(y0)).max(0.0);
            // Y que no choque en horizontal con otra de la fila.
            let choca = f.iter().any(|q| q.caja.solape_x(&p.caja) > p.caja.w.min(q.caja.w) * 0.5);
            if !choca && solape > p.caja.h.min(alto_f) * 0.5 {
                f.push(p);
                puesta = true;
                break;
            }
        }
        if !puesta {
            filas.push(vec![p]);
        }
    }
    let mut out = Vec::new();
    for mut f in filas {
        f.sort_by(|a, b| a.caja.x.partial_cmp(&b.caja.x).unwrap_or(std::cmp::Ordering::Equal));
        let alto_f = mediana(f.iter().map(|q| q.caja.h).collect()).unwrap_or(hm).max(hm * 0.6);
        let mut trozo: Vec<&PalabraHoja> = Vec::new();
        for p in f {
            if let Some(u) = trozo.last() {
                let hueco = p.caja.x - u.caja.x1();
                let raya_en_medio = separadores.iter().any(|r| r.x0 > u.caja.x1() - 1.0 && r.x0 < p.caja.x + 1.0 && r.y0 <= p.caja.y1() && r.y1 >= p.caja.y);
                if hueco > alto_f * HUECO_RENGLON || raya_en_medio {
                    out.push(renglon_de(&trozo));
                    trozo.clear();
                }
            }
            trozo.push(p);
        }
        if !trozo.is_empty() {
            out.push(renglon_de(&trozo));
        }
    }
    out
}

fn renglon_de(ps: &[&PalabraHoja]) -> Renglon {
    let caja = ps.iter().map(|p| p.caja).reduce(|a, b| a.union(&b)).unwrap_or_default();
    let negritas = ps.iter().filter(|p| p.negrita).count();
    Renglon {
        texto: ps.iter().map(|p| p.texto.trim()).collect::<Vec<_>>().join(" "),
        caja,
        palabras: ps.iter().map(|p| (*p).clone()).collect(),
        negrita: negritas * 2 > ps.len(),
        color: color_mayoritario(ps.iter().map(|p| (p.color, p.texto.chars().count()))),
    }
}

/// El color de la mayoria de las letras (por numero de letras).
pub fn color_mayoritario(v: impl Iterator<Item = (Option<[u8; 3]>, usize)>) -> Option<[u8; 3]> {
    let mut grupos: Vec<([u8; 3], usize)> = Vec::new();
    for (c, n) in v {
        let Some(c) = c else { continue };
        match grupos.iter_mut().find(|(g, _)| distancia_color(*g, c) < 60.0) {
            Some(g) => g.1 += n,
            None => grupos.push((c, n)),
        }
    }
    grupos.into_iter().max_by_key(|g| g.1).map(|g| g.0)
}

/// Lo que se parecen dos colores (distancia en RGB). Sin color es negro.
pub fn distancia_color(a: [u8; 3], b: [u8; 3]) -> f64 {
    let d = |i: usize| a[i] as f64 - b[i] as f64;
    (d(0) * d(0) + d(1) * d(1) + d(2) * d(2)).sqrt()
}

// ---------------------------------------------------------------------------
// Orden de lectura: corte XY

#[derive(Debug, Clone, Copy, PartialEq)]
enum Clase {
    Texto(usize),
    Figura(usize),
    Tabla(usize),
    Resto(usize),
}

#[derive(Debug, Clone, Copy)]
struct Atomo {
    caja: Caja,
    clase: Clase,
}

/// Un bloque hoja del corte: sus atomos, y si es una tabla sin rayas.
#[derive(Debug, Clone)]
struct Hoja {
    ids: Vec<usize>,
    tabla: bool,
    /// La columna (0 la de la izquierda; lo que va a todo lo ancho, la del
    /// bloque que lo contiene).
    columna: usize,
}

struct Corte<'a> {
    atomos: &'a [Atomo],
    renglones: &'a [Renglon],
    separadores: &'a [Raya],
    hm: f64,
}

/// Une intervalos `[a, b]` ordenados.
fn unir(mut v: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut o: Vec<(f64, f64)> = Vec::new();
    for (a, b) in v {
        match o.last_mut() {
            Some(u) if a <= u.1 => u.1 = u.1.max(b),
            _ => o.push((a, b)),
        }
    }
    o
}

/// Los huecos de `[x0, x1]` que no cubren los intervalos.
fn huecos(cubiertos: &[(f64, f64)], x0: f64, x1: f64) -> Vec<(f64, f64)> {
    let mut v = Vec::new();
    let mut desde = x0;
    for &(a, b) in cubiertos {
        if a > desde {
            v.push((desde, a));
        }
        desde = desde.max(b);
    }
    if x1 > desde {
        v.push((desde, x1));
    }
    v
}

impl Corte<'_> {
    fn caja(&self, ids: &[usize]) -> Caja {
        ids.iter().map(|&i| self.atomos[i].caja).reduce(|a, b| a.union(&b)).unwrap_or_default()
    }

    fn cortar(&self, ids: &[usize], nivel: usize, col: usize, out: &mut Vec<Hoja>) {
        if ids.len() <= 1 || nivel > 24 {
            if !ids.is_empty() {
                out.push(Hoja { ids: ids.to_vec(), tabla: false, columna: col });
            }
            return;
        }
        if let Some(partes) = self.en_columnas(ids) {
            if self.parece_tabla(&partes) {
                let mut todos: Vec<usize> = partes.concat();
                todos.sort_unstable();
                out.push(Hoja { ids: todos, tabla: true, columna: col });
                return;
            }
            for (k, p) in partes.into_iter().enumerate() {
                self.cortar(&p, nivel + 1, col + k, out);
            }
            return;
        }
        let grupos = self.en_franjas(ids);
        if grupos.len() <= 1 {
            out.push(Hoja { ids: ids.to_vec(), tabla: false, columna: col });
            return;
        }
        for (g, con_pasillo) in grupos {
            if con_pasillo && g.len() < ids.len() {
                self.cortar(&g, nivel + 1, col, out);
            } else {
                out.push(Hoja { ids: g, tabla: false, columna: col });
            }
        }
    }

    /// Partido por pasillos verticales (o rayas separadoras) que cruzan todo
    /// el bloque, de izquierda a derecha. `None` si no hay.
    fn en_columnas(&self, ids: &[usize]) -> Option<Vec<Vec<usize>>> {
        let c = self.caja(ids);
        let cubiertos = unir(ids.iter().map(|&i| (self.atomos[i].caja.x, self.atomos[i].caja.x1())).collect());
        let mut cortes: Vec<f64> = huecos(&cubiertos, c.x, c.x1())
            .into_iter()
            .filter(|(a, b)| *a > c.x && *b < c.x1() && b - a >= self.hm * PASILLO)
            .map(|(a, b)| (a + b) / 2.0)
            .collect();
        for r in self.separadores {
            let cubre = (r.y1.min(c.y1()) - r.y0.max(c.y)).max(0.0);
            if r.x0 > c.x && r.x0 < c.x1() && cubre >= c.h * 0.5 {
                let cruza = ids.iter().any(|&i| {
                    let a = self.atomos[i].caja;
                    a.x < r.x0 - 1.0 && a.x1() > r.x0 + 1.0 && a.y1() > r.y0 && a.y < r.y1
                });
                if !cruza && !cortes.iter().any(|x| (x - r.x0).abs() < self.hm * PASILLO) {
                    cortes.push(r.x0);
                }
            }
        }
        if cortes.is_empty() {
            return None;
        }
        cortes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut partes: Vec<Vec<usize>> = vec![Vec::new(); cortes.len() + 1];
        for &i in ids {
            let x = self.atomos[i].caja.cx();
            let k = cortes.iter().filter(|&&cx| x > cx).count();
            partes[k].push(i);
        }
        partes.retain(|p| !p.is_empty());
        // «q =» a la izquierda de su fraccion: un trozo de formula suelto no
        // es una columna.
        let trozo_de_formula = partes.iter().any(|p| {
            p.len() == 1 && matches!(self.atomos[p[0]].clase, Clase::Texto(r) if { let t = self.renglones[r].texto.trim(); t.ends_with('=') || t.starts_with('=') })
        });
        (partes.len() >= 2 && !trozo_de_formula).then_some(partes)
    }

    /// Columnas de trozos cortos y alineados en filas: una tabla sin rayas,
    /// no columnas de texto.
    fn parece_tabla(&self, partes: &[Vec<usize>]) -> bool {
        if partes.len() < 2 {
            return false;
        }
        let mut textos: Vec<Vec<&Renglon>> = Vec::new();
        for p in partes {
            let mut v = Vec::new();
            for &i in p {
                match self.atomos[i].clase {
                    Clase::Texto(r) => v.push(&self.renglones[r]),
                    Clase::Resto(_) => {}
                    _ => return false,
                }
            }
            if v.len() < 2 {
                return false;
            }
            textos.push(v);
        }
        let cortos = textos.iter().all(|v| mediana(v.iter().map(|r| r.n_palabras() as f64).collect()).unwrap_or(9.0) <= 3.0);
        if !cortos {
            return false;
        }
        // Cada trozo con otro a su altura en otra columna.
        let (mut con, mut total) = (0, 0);
        for (k, v) in textos.iter().enumerate() {
            for r in v {
                total += 1;
                let par = textos.iter().enumerate().any(|(j, w)| j != k && w.iter().any(|s| s.caja.solape_y(&r.caja) > r.caja.h.min(s.caja.h) * 0.5));
                if par {
                    con += 1;
                }
            }
        }
        total > 0 && con * 10 >= total * 6
    }

    /// Franjas horizontales, juntando las seguidas que comparten un pasillo
    /// interior. `(atomos, si comparten pasillo)`.
    fn en_franjas(&self, ids: &[usize]) -> Vec<(Vec<usize>, bool)> {
        let c = self.caja(ids);
        let mut orden: Vec<usize> = ids.to_vec();
        orden.sort_by(|&a, &b| self.atomos[a].caja.y.partial_cmp(&self.atomos[b].caja.y).unwrap_or(std::cmp::Ordering::Equal));
        // Franjas: se cortan donde ningun atomo cruza.
        let mut franjas: Vec<(Vec<usize>, f64)> = Vec::new(); // (ids, y1)
        for i in orden {
            let a = self.atomos[i].caja;
            match franjas.last_mut() {
                Some((f, y1)) if a.y < *y1 - self.hm * 0.1 => {
                    f.push(i);
                    *y1 = y1.max(a.y1());
                }
                _ => franjas.push((vec![i], a.y1())),
            }
        }
        if franjas.len() <= 1 {
            return vec![(ids.to_vec(), false)];
        }
        let libres = |f: &[usize]| -> Vec<(f64, f64)> {
            let cub = unir(f.iter().map(|&i| (self.atomos[i].caja.x, self.atomos[i].caja.x1())).collect());
            huecos(&cub, c.x, c.x1()).into_iter().filter(|(a, b)| b - a >= self.hm * PASILLO).collect()
        };
        let interior = |v: &[(f64, f64)]| v.iter().any(|(a, b)| *a > c.x + 0.5 && *b < c.x1() - 0.5);
        let cruzar = |p: &[(f64, f64)], q: &[(f64, f64)]| -> Vec<(f64, f64)> {
            let mut o = Vec::new();
            for &(a, b) in p {
                for &(x, y) in q {
                    let (s, e) = (a.max(x), b.min(y));
                    if e - s >= self.hm * PASILLO {
                        o.push((s, e));
                    }
                }
            }
            o
        };
        let mut grupos: Vec<(Vec<usize>, bool)> = Vec::new();
        let mut actual: Vec<usize> = Vec::new();
        let mut comun: Vec<(f64, f64)> = Vec::new();
        let mut n_franjas = 0;
        let cerrar = |actual: &mut Vec<usize>, n: usize, grupos: &mut Vec<(Vec<usize>, bool)>| {
            if !actual.is_empty() {
                let con = n >= 2;
                // Las franjas sueltas seguidas sin pasillo, juntas (un parrafo
                // a todo lo ancho es un bloque).
                match grupos.last_mut() {
                    Some((g, false)) if !con => g.append(actual),
                    _ => grupos.push((std::mem::take(actual), con)),
                }
                actual.clear();
            }
        };
        for (f, _) in &franjas {
            let l = libres(f);
            if actual.is_empty() {
                if interior(&l) {
                    comun = l;
                    actual.extend(f);
                    n_franjas = 1;
                } else {
                    let mut solo = f.clone();
                    cerrar(&mut solo, 1, &mut grupos);
                }
                continue;
            }
            let x = cruzar(&comun, &l);
            if interior(&x) {
                comun = x;
                actual.extend(f);
                n_franjas += 1;
            } else {
                cerrar(&mut actual, n_franjas, &mut grupos);
                if interior(&l) {
                    comun = l;
                    actual.extend(f);
                    n_franjas = 1;
                } else {
                    let mut solo = f.clone();
                    cerrar(&mut solo, 1, &mut grupos);
                    n_franjas = 0;
                }
            }
        }
        cerrar(&mut actual, n_franjas, &mut grupos);
        grupos
    }
}


// ---------------------------------------------------------------------------
// Cada bloque a sus zonas

struct Bloque<'a> {
    atomos: &'a [Atomo],
    renglones: &'a [Renglon],
    figuras: &'a [Figura],
    tablas: &'a [TablaR],
    restos: &'a [Caja],
    hm: f64,
}

/// Lo que parece cada renglon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parece {
    Texto,
    Titulo,
    Formula,
}

/// Los signos de una formula.
const MATES: &str = "=+−-×÷*/^_√∑∏∫∂∆πθλμσΔΩαβγδεφωρτ≤≥≠≈±∞·′<>";

/// **Cuanto parece formula un renglon** (0-1+). `ancho`: el del bloque.
pub fn puntuar_formula(texto: &str, caja: &Caja, bloque: &Caja) -> f64 {
    let t = texto.trim();
    let n = t.chars().filter(|c| !c.is_whitespace()).count().max(1) as f64;
    let mates = t.chars().filter(|c| MATES.contains(*c)).count() as f64;
    let largas = t
        .split_whitespace()
        .filter(|w| w.chars().filter(|c| c.is_alphabetic()).count() >= 4 && w.chars().all(|c| c.is_alphabetic() || ",.;:()".contains(c)))
        .count();
    let mut s = 0.0;
    if t.contains('=') || t.contains('≤') || t.contains('≥') || t.contains('≈') {
        s += 0.35;
    }
    s += (mates / n * 2.0).min(0.5);
    match largas {
        0 | 1 => s += 0.2,
        2 | 3 => {}
        _ => s -= 0.5,
    }
    if t.chars().any(|c| "√∑∏∫∂".contains(c)) {
        s += 0.3;
    }
    let (izq, der) = (caja.x - bloque.x, bloque.x1() - caja.x1());
    if izq > bloque.w * 0.12 && der > bloque.w * 0.12 && (izq - der).abs() < bloque.w * 0.25 {
        s += 0.2;
    }
    s
}

impl Bloque<'_> {
    fn zonas(&self, h: &Hoja) -> Vec<Zona> {
        let mut ids = h.ids.clone();
        ids.sort_by(|&a, &b| {
            let (ca, cb) = (self.atomos[a].caja, self.atomos[b].caja);
            ca.y.partial_cmp(&cb.y).unwrap_or(std::cmp::Ordering::Equal).then(ca.x.partial_cmp(&cb.x).unwrap_or(std::cmp::Ordering::Equal))
        });
        let textos: Vec<&Renglon> = ids
            .iter()
            .filter_map(|&i| match self.atomos[i].clase {
                Clase::Texto(r) => Some(&self.renglones[r]),
                _ => None,
            })
            .collect();
        if h.tabla {
            let palabras: Vec<tabla_escaneada::Palabra> = textos
                .iter()
                .flat_map(|r| r.palabras.iter())
                .map(|p| tabla_escaneada::Palabra::nueva(&p.texto, p.caja.x, p.caja.y, p.caja.w, p.caja.h))
                .collect();
            let caja = textos.iter().map(|r| r.caja).reduce(|a, b| a.union(&b)).unwrap_or_default();
            return vec![zona(Tipo::Tabla, caja, 0.65, textos.iter().map(|r| (*r).clone()).collect(), Some(tabla_escaneada::reconstruir(&palabras)), Vec::new())];
        }
        let extension = textos.iter().map(|r| r.caja).reduce(|a, b| a.union(&b)).unwrap_or_default();
        // El interlineado tipico del bloque (de arriba a arriba).
        let paso = {
            let mut v = Vec::new();
            for w in textos.windows(2) {
                let d = w[1].caja.y - w[0].caja.y;
                if d > 0.0 && w[0].caja.solape_x(&w[1].caja) > 0.0 {
                    v.push(d);
                }
            }
            mediana(v).unwrap_or(self.hm * 1.5)
        };
        let mut out: Vec<Zona> = Vec::new();
        let mut actual: Vec<Renglon> = Vec::new();
        let mut tipo_actual = Parece::Texto;
        let mut restos_actual: Vec<Caja> = Vec::new();
        let cerrar = |actual: &mut Vec<Renglon>, restos: &mut Vec<Caja>, tipo: Parece, out: &mut Vec<Zona>| {
            if actual.is_empty() {
                // Restos sueltos: si tienen tamano, al modelo.
                for r in restos.drain(..) {
                    if r.w >= self.hm * 1.2 && r.h >= self.hm * 0.9 {
                        out.push(zona(Tipo::Dudosa, r, 0.3, Vec::new(), None, Vec::new()));
                    }
                }
                return;
            }
            let mut caja = actual.iter().map(|r| r.caja).reduce(|a, b| a.union(&b)).unwrap_or_default();
            for r in restos.drain(..) {
                caja = caja.union(&r);
            }
            let (t, conf) = match tipo {
                Parece::Formula => (Tipo::Formula, 0.7),
                Parece::Titulo => (Tipo::Encabezado, 0.8),
                Parece::Texto => (Tipo::Parrafo, 0.85),
            };
            let mut z = zona(t, caja, conf, std::mem::take(actual), None, Vec::new());
            z.alineacion = alineacion(&z.renglones, &extension, self.hm);
            out.push(z);
        };
        let mut ultimo: Option<Renglon> = None;
        for &i in &ids {
            match self.atomos[i].clase {
                Clase::Figura(f) => {
                    cerrar(&mut actual, &mut restos_actual, tipo_actual, &mut out);
                    let fi = &self.figuras[f];
                    out.push(zona(fi.tipo, fi.caja, fi.confianza, renglones(&fi.palabras, &[], self.hm), None, fi.rayas.clone()));
                    ultimo = None;
                }
                Clase::Tabla(t) => {
                    cerrar(&mut actual, &mut restos_actual, tipo_actual, &mut out);
                    let TablaR { caja, rayas, palabras, iconos } = &self.tablas[t];
                    let ps: Vec<tabla_escaneada::Palabra> = palabras
                        .iter()
                        .map(|p| tabla_escaneada::Palabra::nueva(&p.texto, p.caja.x, p.caja.y, p.caja.w, p.caja.h))
                        .collect();
                    let rs = renglones(palabras, &[], self.hm);
                    let leida = tabla_escaneada::reconstruir(&ps);
                    if leida.filas.len() <= 1 && iconos.is_empty() {
                        // Un recuadro con un renglon dentro: texto con su marco.
                        let mut z = zona(Tipo::Parrafo, *caja, 0.8, rs, None, rayas.clone());
                        z.alineacion = alineacion(&z.renglones, caja, self.hm);
                        out.push(z);
                    } else {
                        let mut z = zona(Tipo::Tabla, *caja, 0.85, rs, Some(leida), rayas.clone());
                        z.dentro = iconos.clone();
                        out.push(z);
                    }
                    ultimo = None;
                }
                Clase::Resto(k) => {
                    let r = self.restos[k];
                    // Con la formula de arriba si la toca.
                    let toca = actual.last().is_some_and(|u| r.y < u.caja.y1() + self.hm * 1.2);
                    // O con los restos que esperan (dos trozos de una raiz).
                    let junto = actual.is_empty() && restos_actual.iter().any(|c| c.crecer(self.hm).comun(&r) > 0.0);
                    // Un renglon corto justo encima de una raya (el numerador
                    // de una fraccion): la formula empieza en el.
                    let numerador = toca && actual.len() == 1 && actual[0].n_palabras() <= 4 && r.w >= actual[0].caja.w * 0.6;
                    if numerador {
                        tipo_actual = Parece::Formula;
                    }
                    if (tipo_actual == Parece::Formula && toca) || junto {
                        restos_actual.push(r);
                    } else {
                        cerrar(&mut actual, &mut restos_actual, tipo_actual, &mut out);
                        restos_actual.push(r);
                        tipo_actual = Parece::Formula;
                    }
                }
                Clase::Texto(k) => {
                    let r = &self.renglones[k];
                    let parece = self.parece(r, &extension);
                    let sigue = match (ultimo.as_ref(), actual.last()) {
                        (Some(u), Some(_)) => self.continua(u, r, parece, tipo_actual, paso, &extension),
                        _ => false,
                    };
                    // Unos restos que esperaban (la raya de una fraccion
                    // encima): si este renglon es formula, van con el.
                    if actual.is_empty() && !restos_actual.is_empty() {
                        let cerca = restos_actual.iter().any(|c| (r.caja.y - c.y1()).abs() < self.hm * 1.5 || c.solape_y(&r.caja) > 0.0);
                        let corto = r.n_palabras() <= 4;
                        if (parece == Parece::Formula || corto) && cerca {
                            actual.push(r.clone());
                            tipo_actual = Parece::Formula;
                            ultimo = Some(r.clone());
                            continue;
                        }
                        cerrar(&mut actual, &mut restos_actual, tipo_actual, &mut out);
                    }
                    let tipo_r = if tipo_actual == Parece::Formula && sigue { Parece::Formula } else { parece };
                    if sigue && tipo_r == tipo_actual {
                        actual.push(r.clone());
                    } else {
                        cerrar(&mut actual, &mut restos_actual, tipo_actual, &mut out);
                        actual.push(r.clone());
                        tipo_actual = tipo_r;
                    }
                    ultimo = Some(r.clone());
                }
            }
        }
        cerrar(&mut actual, &mut restos_actual, tipo_actual, &mut out);
        juntar_formulas(&mut out, self.hm);
        out
    }

    fn parece(&self, r: &Renglon, bloque: &Caja) -> Parece {
        if puntuar_formula(&r.texto, &r.caja, bloque) >= 0.55 {
            return Parece::Formula;
        }
        let alto = r.alto();
        let corto = r.n_palabras() <= 14 && !r.texto.trim_end().ends_with('.');
        if alto >= self.hm * 1.3 && r.n_palabras() <= 20 || r.negrita && corto {
            Parece::Titulo
        } else {
            Parece::Texto
        }
    }

    /// Si `r` sigue el bloque que acaba en `u`.
    fn continua(&self, u: &Renglon, r: &Renglon, parece: Parece, tipo: Parece, paso: f64, bloque: &Caja) -> bool {
        // Otro color (un renglon rojo tras uno negro) es otro bloque.
        let negro = [30, 30, 30];
        if distancia_color(u.color.unwrap_or(negro), r.color.unwrap_or(negro)) > 70.0 {
            return false;
        }
        let hueco = r.caja.y - u.caja.y1();
        let de_arriba = r.caja.y - u.caja.y;
        match tipo {
            Parece::Formula => {
                // Un renglon corto y centrado cerca de una formula (el
                // denominador de una fraccion) sigue en ella.
                let corto = r.n_palabras() <= 4 && puntuar_formula(&r.texto, &r.caja, bloque) >= 0.3;
                hueco < self.hm * 1.2 && (parece == Parece::Formula || corto)
            }
            Parece::Titulo => parece == Parece::Titulo && hueco < u.alto().max(r.alto()) * 0.9 && (r.alto() / u.alto() - 1.0).abs() < 0.25,
            Parece::Texto => {
                if parece != Parece::Texto {
                    return false;
                }
                let alto_parecido = (r.alto() / u.alto().max(1.0) - 1.0).abs() < 0.3;
                let sin_hueco = de_arriba < paso * 1.45 && hueco < self.hm * 1.2;
                // Sangria de primera linea: empieza un parrafo.
                let sangria = r.caja.x - bloque.x > self.hm * 1.2 && u.caja.x - bloque.x < self.hm * 0.6;
                // Un renglon anterior corto que acaba en punto cierra el parrafo.
                let cerro = u.caja.x1() < bloque.x1() - self.hm * 3.0 && u.texto.trim_end().ends_with(['.', ':', '!', '?']);
                alto_parecido && sin_hueco && !sangria && !cerro
            }
        }
    }
}

fn zona(tipo: Tipo, caja: Caja, confianza: f32, renglones: Vec<Renglon>, tabla: Option<tabla_escaneada::TablaLeida>, rayas: Vec<[f64; 5]>) -> Zona {
    Zona { id: 0, tipo, caja, orden: 0, columna: 0, confianza, renglones, alineacion: Alineacion::Izquierda, tabla, rayas, dentro: Vec::new(), panel: None }
}

/// Como van los renglones dentro del bloque.
fn alineacion(rs: &[Renglon], bloque: &Caja, hm: f64) -> Alineacion {
    if rs.is_empty() {
        return Alineacion::Izquierda;
    }
    let izq: Vec<f64> = rs.iter().map(|r| r.caja.x - bloque.x).collect();
    let der: Vec<f64> = rs.iter().map(|r| bloque.x1() - r.caja.x1()).collect();
    let tol = hm * 0.8;
    let centrado = izq.iter().zip(&der).all(|(a, b)| (a - b).abs() < hm * 1.5) && izq.iter().any(|a| *a > hm * 2.0);
    if centrado {
        return Alineacion::Centro;
    }
    if der.iter().all(|d| *d < tol) && izq.iter().any(|a| *a > hm * 2.0) {
        return Alineacion::Derecha;
    }
    if rs.len() >= 3 && der[..der.len() - 1].iter().all(|d| *d < tol) {
        return Alineacion::Justificada;
    }
    Alineacion::Izquierda
}

/// Una tabla rayada antes de ser zona: su caja, sus rayas, sus palabras y
/// las imagenes de sus celdas (iconos).
#[derive(Debug, Clone)]
struct TablaR {
    caja: Caja,
    rayas: Vec<[f64; 5]>,
    palabras: Vec<PalabraHoja>,
    iconos: Vec<Caja>,
}

/// **Los trozos de una misma formula, juntos**: el numerador que parecio un
/// titulo, el «q =» que el OCR no leyo (una mancha) y el denominador acaban
/// a veces en zonas distintas. Una formula se come lo que la toca si es
/// otra formula, una zona dudosa o un renglon corto.
fn juntar_formulas(zonas: &mut Vec<Zona>, hm: f64) {
    let pequena = |z: &Zona| match z.tipo {
        Tipo::Formula | Tipo::Dudosa => true,
        Tipo::Encabezado | Tipo::Parrafo => z.renglones.len() == 1 && z.renglones[0].n_palabras() <= 4,
        _ => false,
    };
    loop {
        let mut junto = false;
        'fuera: for i in 0..zonas.len() {
            for j in 0..zonas.len() {
                if i == j || zonas[i].tipo != Tipo::Formula || !pequena(&zonas[j]) {
                    continue;
                }
                if zonas[i].caja.crecer(hm * 1.6).comun(&zonas[j].caja) <= 0.0 {
                    continue;
                }
                let b = zonas.remove(j);
                let i = if j < i { i - 1 } else { i };
                let a = &mut zonas[i];
                a.caja = a.caja.union(&b.caja);
                a.renglones.extend(b.renglones);
                a.renglones.sort_by(|x, y| x.caja.y.partial_cmp(&y.caja.y).unwrap_or(std::cmp::Ordering::Equal));
                a.rayas.extend(b.rayas);
                a.confianza = a.confianza.max(b.confianza);
                junto = true;
                break 'fuera;
            }
        }
        if !junto {
            break;
        }
    }
}

/// **Un dibujo es una zona** (16-oct, el diagrama de flujo del banco de
/// documentos): el seccionado partia un dibujo en trozos —el ovalo de «Fin»
/// en dos mitades, los rectangulos como «tablas», el paralelogramo en una
/// dudosa y una formula— y cada trozo se vectorizaba por su cuenta, sin las
/// flechas que los unen. Aqui, la tinta que va seguida de una pieza a otra
/// (las flechas, los lados) junta en UNA zona de dibujo todas las zonas que
/// toca, si alguna era ya dibujo: el texto de dentro va con ella (sus
/// rotulos). Un parrafo suelto, una foto o una tabla que no toca un dibujo
/// se quedan como estaban.
pub fn unir_dibujos(s: &mut Seccionado, rgba: &[u8]) {
    let (w, h) = (s.ancho as usize, s.alto as usize);
    // Que zonas son dibujo: las de dibujo y las dudosas que son un diagrama (las barras
    // de una grafica, que el seccionado no supo que eran).
    let (w0, h0) = (s.ancho, s.alto);
    let es_dibujo: Vec<bool> = s
        .zonas
        .iter()
        .map(|z| {
            matches!(z.tipo, Tipo::Dibujo | Tipo::Grafica)
                || (z.tipo == Tipo::Dudosa && z.caja.w >= 60.0 && z.caja.h >= 60.0 && {
                    let (sub, sw, sh) = imagen::recortar(rgba, w0, h0, &z.caja);
                    let o = crate::diagrama::Opciones { alto_letra: s.alto_letra, exacta: false, ..Default::default() };
                    crate::diagrama::vectorizar(&sub, sw, sh, &[], &o, None).es_diagrama()
                })
        })
        .collect();
    if w < 8 || h < 8 || rgba.len() < w * h * 4 || !es_dibujo.iter().any(|&b| b) {
        return;
    }
    // A mitad de tamano (la tinta de 1 px se queda: se mira el mas oscuro de cada 2x2).
    let (mw, mh) = (w / 2, h / 2);
    let img = crate::diagrama::mascara::Img::de_rgba(rgba, w, h);
    let papel = crate::diagrama::mascara::fondo(&img);
    let mut tinta = vec![false; mw * mh];
    for y in 0..mh {
        for x in 0..mw {
            tinta[y * mw + x] = (0..4).any(|k| {
                let (xx, yy) = (2 * x + k % 2, 2 * y + k / 2);
                crate::diagrama::mascara::dist_color(img.px[yy * w + xx], papel) > 60.0
            });
        }
    }
    // Lo que es texto no une (un rotulo pegado a una flecha no arrastra el parrafo).
    for z in &s.zonas {
        for r in &z.renglones {
            for p in &r.palabras {
                let c = p.caja.crecer(1.0);
                for y in (c.y / 2.0).floor().max(0.0) as usize..((c.y1() / 2.0).ceil() as usize).min(mh) {
                    for x in (c.x / 2.0).floor().max(0.0) as usize..((c.x1() / 2.0).ceil() as usize).min(mw) {
                        tinta[y * mw + x] = false;
                    }
                }
            }
        }
    }
    let comps = crate::diagrama::mascara::componentes(&tinta, mw, mh);
    let unibles = |t: Tipo| matches!(t, Tipo::Dibujo | Tipo::Grafica | Tipo::Tabla | Tipo::Dudosa | Tipo::Formula | Tipo::Linea | Tipo::Diagrama);
    let mut grupos: Vec<Vec<usize>> = Vec::new();
    for comp in comps.iter().filter(|c| c.len() >= 40) {
        // Las zonas que toca la mancha (con al menos 15 de sus pixeles dentro).
        let mut cuenta = vec![0usize; s.zonas.len()];
        for &i in comp {
            let (x, y) = ((i % mw) as f64 * 2.0 + 1.0, (i / mw) as f64 * 2.0 + 1.0);
            for (k, z) in s.zonas.iter().enumerate() {
                if z.caja.contiene(x, y) {
                    cuenta[k] += 1;
                }
            }
        }
        let mut tocadas: Vec<usize> = (0..s.zonas.len()).filter(|&k| cuenta[k] >= 15 && unibles(s.zonas[k].tipo) && s.zonas[k].tipo != Tipo::Formula).collect();
        // Una «formula» solo si cae dentro de lo demas (el «Leer N» de su
        // paralelogramo, si; un titulo subrayado que toca la figura, no).
        if let Some(union) = tocadas.iter().map(|&k| s.zonas[k].caja).reduce(|a, b| Caja::de_esquinas(a.x.min(b.x), a.y.min(b.y), a.x1().max(b.x1()), a.y1().max(b.y1()))) {
            tocadas.extend((0..s.zonas.len()).filter(|&k| cuenta[k] >= 15 && s.zonas[k].tipo == Tipo::Formula && s.zonas[k].caja.dentro_de(&union) >= 0.6));
        }
        if tocadas.len() >= 2 && tocadas.iter().any(|&k| es_dibujo[k]) {
            grupos.push(tocadas);
        }
    }
    // Los grupos que comparten zonas, uno.
    let mut i = 0;
    while i < grupos.len() {
        let mut j = i + 1;
        let mut junto = false;
        while j < grupos.len() {
            if grupos[j].iter().any(|k| grupos[i].contains(k)) {
                let g = grupos.remove(j);
                for k in g {
                    if !grupos[i].contains(&k) {
                        grupos[i].push(k);
                    }
                }
                junto = true;
            } else {
                j += 1;
            }
        }
        if !junto {
            i += 1;
        }
    }
    if grupos.is_empty() {
        return;
    }
    let mut quitar: Vec<usize> = Vec::new();
    for g in &grupos {
        let base = *g.iter().min().unwrap();
        let mut caja = s.zonas[base].caja;
        for &k in g {
            let c = s.zonas[k].caja;
            caja = Caja::de_esquinas(caja.x.min(c.x), caja.y.min(c.y), caja.x1().max(c.x1()), caja.y1().max(c.y1()));
        }
        // Y el texto que cae dentro del dibujo (el «Fin» del ovalo, «Si» y «No»).
        // (Y los rotulos de alrededor: palabras cortas pegadas al dibujo, como los
        // meses bajo las barras o los valores del eje.)
        let a = s.alto_letra.max(6.0);
        let rotulos = |z: &Zona| z.renglones.iter().flat_map(|r| r.palabras.iter()).all(|p| p.texto.chars().count() <= 6);
        let dentro: Vec<usize> = (0..s.zonas.len())
            .filter(|&k| !g.contains(&k) && matches!(s.zonas[k].tipo, Tipo::Parrafo | Tipo::Encabezado))
            .filter(|&k| s.zonas[k].caja.dentro_de(&caja) >= 0.8 || (rotulos(&s.zonas[k]) && s.zonas[k].caja.dentro_de(&caja.crecer(2.5 * a)) >= 0.8))
            .collect();
        let mut renglones: Vec<Renglon> = g.iter().chain(&dentro).flat_map(|&k| s.zonas[k].renglones.iter().cloned()).collect();
        renglones.sort_by(|a, b| a.caja.y.total_cmp(&b.caja.y));
        let z = &mut s.zonas[base];
        z.tipo = Tipo::Dibujo;
        z.caja = caja;
        z.renglones = renglones;
        z.rayas.clear();
        z.tabla = None;
        z.dentro.clear();
        quitar.extend(g.iter().chain(&dentro).copied().filter(|&k| k != base));
    }
    quitar.sort_unstable();
    quitar.dedup();
    for &k in quitar.iter().rev() {
        s.zonas.remove(k);
    }
    for (k, z) in s.zonas.iter_mut().enumerate() {
        z.id = k;
    }
}
