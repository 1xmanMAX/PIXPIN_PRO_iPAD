//! **Las figuras de una anotacion**, con el motor del lienzo: la marca sobre
//! lo senalado (recuadro, resaltado, subrayado de error o el numero de un
//! paso), la nota del margen (papel del color de su clase, titulo y texto
//! partido a su ancho) y la **linea guia** de la nota a lo senalado.
//!
//! La linea guia **no tapa el texto**: se prueba primero la recta; si cruza
//! alguna palabra, se va por el borde de la columna y se entra por el hueco
//! entre renglones (encima o debajo de lo senalado); y si tampoco hay hueco,
//! se queda en el borde de la columna, a la altura de lo senalado, con un
//! punto. Todo en las unidades de la capa de la pagina.

use pixpin_motor2d::relleno::EstiloRelleno;
use pixpin_motor2d::{ColorRgba, Elemento, Figura, Punto2};

use super::ancla::Anclaje;
use super::colocar::{Colocada, Lado};
use super::{Clase, Pagina, Pedida};

/// La letra de las notas: la «Normal» del catalogo (Nunito), la que viaja
/// tal cual al movil y a Excalidraw (una que no esta en el catalogo, como
/// Segoe UI, se lee alli como la de a mano).
const FAMILIA: &str = "Nunito";

/// Parte `texto` en renglones que caben en `ancho` midiendo con la letra
/// de las notas (con DirectWrite si la app puso su medidor; si no, a ojo
/// y algo de mas, que es lo seguro).
fn partir(texto: &str, tam: f32, ancho: f32, negrita: bool) -> Vec<String> {
    let estilo = pixpin_motor2d::texto::EstiloDeTexto {
        negrita,
        ..Default::default()
    };
    let mide = |s: &str| pixpin_motor2d::texto::medida(s, tam, FAMILIA, estilo).0;
    let mut v = Vec::new();
    for parrafo in texto.split('\n') {
        let mut l = String::new();
        for w in parrafo.split_whitespace() {
            let prueba = if l.is_empty() {
                w.to_string()
            } else {
                format!("{l} {w}")
            };
            if !l.is_empty() && mide(&prueba) > ancho {
                v.push(std::mem::replace(&mut l, w.to_string()));
            } else {
                l = prueba;
            }
        }
        v.push(l);
    }
    v
}
/// El texto de las notas.
const TINTA: (u8, u8, u8) = (0x21, 0x25, 0x29);

fn color((r, g, b): (u8, u8, u8)) -> ColorRgba {
    ColorRgba::opaco(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

/// El color mezclado con blanco (`k` de blanco): el papel de la nota.
fn palido((r, g, b): (u8, u8, u8), k: f32) -> ColorRgba {
    let m = |c: u8| (c as f32 / 255.0) * (1.0 - k) + k;
    ColorRgba::opaco(m(r), m(g), m(b))
}

fn interlineado() -> f32 {
    pixpin_motor2d::texto::interlineado_de(FAMILIA).unwrap_or(pixpin_motor2d::texto::INTERLINEADO)
}

/// Lo que dice una nota y a que letra.
#[derive(Debug, Clone, PartialEq)]
pub struct Nota {
    pub titulo: String,
    pub cuerpo: String,
    pub letra: f32,
}

impl Nota {
    pub fn de(p: &Pedida, letra: f32) -> Nota {
        let titulo = match (p.clase, p.numero) {
            (Clase::Paso, Some(n)) => format!("Paso {n}"),
            (c, _) => c.titulo().to_string(),
        };
        let mut cuerpo = p.nota.trim().to_string();
        if let Some(c) = p
            .correccion
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
            if !cuerpo.is_empty() {
                cuerpo.push('\n');
            }
            cuerpo.push_str(&format!("→ {c}"));
        }
        Nota {
            titulo,
            cuerpo,
            letra: letra.max(1.0),
        }
    }

    fn tam_titulo(&self) -> f32 {
        self.letra * 0.82
    }

    fn relleno(&self) -> f32 {
        self.letra * 0.5
    }

    /// Mas ancha no se lee bien (unas 36 letras por renglon).
    pub fn ancho_maximo(&self) -> f32 {
        self.letra * 18.0
    }

    /// El titulo y el cuerpo partidos a una nota de `ancho`.
    pub fn renglones(&self, ancho: f32) -> (Vec<String>, Vec<String>) {
        // Un pelo menos que el hueco, por si la letra sale algo mas ancha.
        let util = ((ancho - 2.0 * self.relleno()) * 0.96).max(self.letra * 3.0);
        let t = partir(&self.titulo, self.tam_titulo(), util, true);
        let c = if self.cuerpo.is_empty() {
            Vec::new()
        } else {
            partir(&self.cuerpo, self.letra, util, false)
        };
        (t, c)
    }

    /// El alto de la nota partida a `ancho`.
    pub fn alto_con(&self, ancho: f32) -> f32 {
        let (t, c) = self.renglones(ancho);
        let il = interlineado();
        let mut alto = 2.0 * self.relleno() + t.len() as f32 * self.tam_titulo() * il;
        if !c.is_empty() {
            alto += self.letra * 0.2 + c.len() as f32 * self.letra * il;
        }
        alto
    }
}

fn base(grupos: &[String], trazo: ColorRgba, grosor: f32) -> Elemento {
    Elemento {
        trazo,
        grosor: grosor.max(0.5),
        rugosidad: 0.0,
        grupos: grupos.to_vec(),
        ..Default::default()
    }
}

fn rect(c: [f32; 4], b: &Elemento) -> Elemento {
    Elemento {
        figura: Figura::Rectangulo,
        x: c[0],
        y: c[1],
        ancho: (c[2] - c[0]).max(0.5),
        alto: (c[3] - c[1]).max(0.5),
        ..b.clone()
    }
}

fn linea(puntos: &[(f32, f32)], b: &Elemento) -> Elemento {
    let pts: Vec<Punto2> = puntos.iter().map(|(x, y)| Punto2::nuevo(*x, *y)).collect();
    let (x0, y0, x1, y1) = pts
        .iter()
        .fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |a, p| {
            (a.0.min(p.x), a.1.min(p.y), a.2.max(p.x), a.3.max(p.y))
        });
    Elemento {
        figura: Figura::Linea { puntos: pts },
        x: x0,
        y: y0,
        ancho: x1 - x0,
        alto: y1 - y0,
        ..b.clone()
    }
}

#[allow(clippy::too_many_arguments)]
fn texto(
    t: &str,
    x: f32,
    y: f32,
    tam: f32,
    ancho: f32,
    renglones: usize,
    b: &Elemento,
    negrita: bool,
) -> Elemento {
    let mut e = Elemento {
        figura: Figura::Texto {
            texto: t.to_string(),
            tam,
            familia: FAMILIA.into(),
        },
        x,
        y,
        ancho,
        alto: renglones.max(1) as f32 * tam * interlineado(),
        grosor: 1.0,
        ..b.clone()
    };
    e.extras.negrita = negrita;
    e
}

/// **Las figuras de una anotacion**: la marca sobre `a`, la nota en
/// `colocada` y la linea guia entre las dos, todas en `grupos`.
pub fn de(
    clase: Clase,
    a: &Anclaje,
    colocada: &Colocada,
    nota: &Nota,
    pag: &Pagina,
    grupos: Vec<String>,
) -> Vec<Elemento> {
    let l = nota.letra;
    let c = clase.color();
    let tinta = color(c);
    let b = base(&grupos, tinta, l * 0.1);
    let mut v = Vec::new();
    let u = a.union();
    let aire = l * 0.2;
    let marca = [u[0] - aire, u[1] - aire, u[2] + aire, u[3] + aire];

    // 1. Lo senalado.
    match clase {
        Clase::Definicion => {
            for caja in &a.cajas {
                let mut r = rect(
                    [
                        caja[0] - aire * 0.5,
                        caja[1] - aire * 0.3,
                        caja[2] + aire * 0.5,
                        caja[3] + aire * 0.3,
                    ],
                    &b,
                );
                r.trazo = ColorRgba { a: 0.0, ..tinta };
                r.relleno = Some(tinta);
                r.estilo_relleno = EstiloRelleno::Solido;
                r.opacidad = 0.22;
                v.push(r);
                v.push(linea(
                    &[
                        (caja[0], caja[3] + aire * 0.4),
                        (caja[2], caja[3] + aire * 0.4),
                    ],
                    &b,
                ));
            }
        }
        Clase::Error => {
            for caja in &a.cajas {
                let mut r = rect(
                    [
                        caja[0] - aire * 0.5,
                        caja[1] - aire * 0.3,
                        caja[2] + aire * 0.5,
                        caja[3] + aire * 0.3,
                    ],
                    &b,
                );
                r.trazo = ColorRgba { a: 0.0, ..tinta };
                r.relleno = Some(tinta);
                r.estilo_relleno = EstiloRelleno::Solido;
                r.opacidad = 0.14;
                v.push(r);
                // El subrayado del error, en zigzag.
                let y = caja[3] + aire * 0.5;
                let paso = l * 0.3;
                let mut pts = Vec::new();
                let mut x = caja[0];
                let mut arriba = false;
                while x < caja[2] {
                    pts.push((x, if arriba { y - paso * 0.5 } else { y }));
                    arriba = !arriba;
                    x += paso;
                }
                pts.push((caja[2], y));
                if pts.len() >= 2 {
                    v.push(linea(&pts, &b));
                }
            }
        }
        Clase::Resumen => {
            // Una raya al borde de la columna, a la altura de lo que resume y
            // del lado de la nota: fuera del texto, nunca encima.
            let col = pag.columna().unwrap_or(marca);
            let x = match colocada.franja.lado {
                Lado::Derecha => col[2].max(marca[2]) + aire * 2.0,
                Lado::Izquierda => col[0].min(marca[0]) - aire * 2.0,
            };
            let mut r = linea(&[(x, marca[1]), (x, marca[3])], &b);
            r.grosor = l * 0.18;
            v.push(r);
        }
        _ => {
            let mut r = rect(marca, &b);
            r.redondo = true;
            r.grosor = l * 0.11;
            v.push(r);
        }
    }
    if clase == Clase::Paso {
        // El numero, en un circulo antes de lo senalado.
        let n = nota.titulo.trim_start_matches("Paso ").to_string();
        let d = l * 1.25;
        let (cx, cy) = (marca[0] - d * 0.6, (a.cajas[0][1] + a.cajas[0][3]) / 2.0);
        let mut circulo = Elemento {
            figura: Figura::Elipse,
            x: cx - d / 2.0,
            y: cy - d / 2.0,
            ancho: d,
            alto: d,
            ..b.clone()
        };
        circulo.relleno = Some(tinta);
        circulo.estilo_relleno = EstiloRelleno::Solido;
        v.push(circulo);
        let tam = l * 0.8;
        let ancho = pixpin_motor2d::exportar::ancho_de_texto(&n, tam);
        let mut t = texto(
            &n,
            cx - ancho / 2.0,
            cy - tam * interlineado() / 2.0,
            tam,
            ancho,
            1,
            &b,
            true,
        );
        t.trazo = ColorRgba::opaco(1.0, 1.0, 1.0);
        v.push(t);
    }

    // 2. La linea guia.
    let caja = colocada.caja;
    v.extend(guia(a, marca, caja, colocada.franja.lado, pag, &b, l));

    // 3. La nota.
    let mut papel = rect(caja, &b);
    papel.relleno = Some(palido(c, 0.9));
    papel.estilo_relleno = EstiloRelleno::Solido;
    papel.trazo = ColorRgba { a: 0.55, ..tinta };
    papel.grosor = l * 0.06;
    papel.redondo = true;
    v.push(papel);
    let (t, cu) = nota.renglones(caja[2] - caja[0]);
    let p = nota.relleno();
    let ancho = caja[2] - caja[0] - 2.0 * p;
    let tt = nota.tam_titulo();
    v.push(texto(
        &t.join("\n"),
        caja[0] + p,
        caja[1] + p,
        tt,
        ancho,
        t.len(),
        &b,
        true,
    ));
    if !cu.is_empty() {
        let y = caja[1] + p + t.len() as f32 * tt * interlineado() + l * 0.2;
        let mut e = texto(
            &cu.join("\n"),
            caja[0] + p,
            y,
            l,
            ancho,
            cu.len(),
            &b,
            false,
        );
        e.trazo = color(TINTA);
        v.push(e);
    }
    v
}

/// Si un punto cae dentro de una caja (algo encogida: rozar no es tapar).
fn dentro(c: &[f32; 4], (x, y): (f32, f32), margen: f32) -> bool {
    x > c[0] + margen && x < c[2] - margen && y > c[1] + margen && y < c[3] - margen
}

/// Si el camino pasa por encima de alguna palabra que no sea de lo
/// senalado (se mira cada tercio de letra).
fn tapa(camino: &[(f32, f32)], palabras: &[[f32; 4]], paso: f32) -> bool {
    for w in camino.windows(2) {
        let (a, b) = (w[0], w[1]);
        let largo = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let n = (largo / paso.max(0.1)).ceil().max(1.0) as usize;
        for k in 0..=n {
            let t = k as f32 / n as f32;
            let q = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            if palabras.iter().any(|c| dentro(c, q, paso * 0.15)) {
                return true;
            }
        }
    }
    false
}

/// **La linea guia** de la nota (`caja`) a lo senalado (`marca`): el
/// primer camino que no tapa texto, y un punto donde llega.
fn guia(
    a: &Anclaje,
    marca: [f32; 4],
    caja: [f32; 4],
    lado: Lado,
    pag: &Pagina,
    b: &Elemento,
    l: f32,
) -> Vec<Elemento> {
    // En una hoja de calculo la «columna» es todo lo usado: lo de fuera del
    // rango leido tambien son celdas con numeros que la guia no puede tachar.
    let col = if pag.solo_derecha {
        pag.caja
    } else {
        pag.columna().unwrap_or(marca)
    };
    // Las palabras que no son de lo senalado.
    let palabras: Vec<[f32; 4]> = pag
        .renglones
        .iter()
        .flat_map(|r| r.palabras.iter().map(|p| p.caja))
        .filter(|c| {
            !(c[0] >= marca[0] - 0.5
                && c[2] <= marca[2] + 0.5
                && c[1] >= marca[1] - 0.5
                && c[3] <= marca[3] + 0.5)
        })
        .collect();
    let primera = a.cajas[0];
    let ay = (primera[1] + primera[3]) / 2.0;
    let g = l * 0.5;
    let derecha = lado == Lado::Derecha;
    // Donde sale de la nota: su canto de dentro, a la altura de lo senalado
    // si cae dentro de la nota.
    let sy = ay.clamp(
        caja[1] + l * 0.6,
        (caja[3] - l * 0.6).max(caja[1] + l * 0.6),
    );
    let s = (if derecha { caja[0] } else { caja[2] }, sy);
    // El borde de lo senalado del lado de la nota, y el de la columna.
    let (borde, borde_col) = if derecha {
        (marca[2], col[2].max(marca[2]) + g)
    } else {
        (marca[0], col[0].min(marca[0]) - g)
    };
    // El hueco entre renglones encima y debajo de lo senalado: lo que queda
    // libre entre la marca y la palabra mas cercana que la solapa a lo ancho.
    let (xa, xb) = if derecha {
        (marca[0], borde_col)
    } else {
        (borde_col, marca[2])
    };
    let solapa = |c: &[f32; 4]| c[2] > xa && c[0] < xb;
    let techo = palabras
        .iter()
        .filter(|c| solapa(c) && c[3] <= marca[1] + 0.5)
        .map(|c| c[3])
        .fold(f32::MIN, f32::max);
    let suelo = palabras
        .iter()
        .filter(|c| solapa(c) && c[1] >= marca[3] - 0.5)
        .map(|c| c[1])
        .fold(f32::MAX, f32::min);
    let hueco_arriba = if techo == f32::MIN {
        Some(marca[1] - l * 0.4)
    } else {
        (marca[1] - techo >= l * 0.12).then(|| (marca[1] + techo) / 2.0)
    };
    let hueco_abajo = if suelo == f32::MAX {
        Some(marca[3] + l * 0.4)
    } else {
        (suelo - marca[3] >= l * 0.12).then(|| (marca[3] + suelo) / 2.0)
    };
    let entrada_x = if derecha {
        borde - l * 0.6
    } else {
        borde + l * 0.6
    };
    let mut caminos: Vec<Vec<(f32, f32)>> = if pag.solo_derecha {
        Vec::new()
    } else {
        vec![vec![s, (borde, ay)]]
    };
    if !pag.solo_derecha {
        caminos.push(vec![s, (borde_col, sy), (borde_col, ay), (borde, ay)]);
    }
    if let Some(y) = hueco_arriba {
        caminos.push(vec![
            s,
            (borde_col, sy),
            (borde_col, y),
            (entrada_x, y),
            (entrada_x, marca[1]),
        ]);
    }
    if let Some(y) = hueco_abajo {
        caminos.push(vec![
            s,
            (borde_col, sy),
            (borde_col, y),
            (entrada_x, y),
            (entrada_x, marca[3]),
        ]);
    }
    if pag.solo_derecha {
        // Hasta el borde de lo usado, a la altura de su celda (abajo).
        caminos.clear();
    }
    let paso = l / 3.0;
    let (camino, llega) = caminos
        .into_iter()
        .find(|c| !tapa(c, &palabras, paso))
        .map(|c| {
            let fin = *c.last().unwrap_or(&s);
            (c, fin)
        })
        // Sin camino libre: hasta el borde de la columna, a la altura de lo
        // senalado.
        .unwrap_or_else(|| (vec![s, (borde_col, sy), (borde_col, ay)], (borde_col, ay)));
    let camino: Vec<(f32, f32)> = camino.into_iter().fold(Vec::new(), |mut v, p| {
        if v.last()
            .is_none_or(|q: &(f32, f32)| (q.0 - p.0).abs() + (q.1 - p.1).abs() > 0.01)
        {
            v.push(p);
        }
        v
    });
    let mut v = Vec::new();
    if camino.len() >= 2 {
        let mut e = linea(&camino, b);
        e.grosor = (l * 0.07).max(0.6);
        v.push(e);
    }
    let r = l * 0.18;
    let mut punto = Elemento {
        figura: Figura::Elipse,
        x: llega.0 - r,
        y: llega.1 - r,
        ancho: 2.0 * r,
        alto: 2.0 * r,
        ..b.clone()
    };
    punto.relleno = Some(b.trazo);
    punto.estilo_relleno = EstiloRelleno::Solido;
    v.push(punto);
    v
}
