//! **Un diagrama o una figura geometrica, a figuras EXACTAS del lienzo.**
//!
//! Queja del usuario (15-oct-2026, con captura): pego en el lienzo una
//! lamina de geometria (circunferencia, cuadrante relleno, cuerdas, arcos de
//! color, puntos A B C D O P, marcas de angulo, rectas punteadas con flecha,
//! recuadros, formulas) y pidio «calca esto». El calco por manchas de color
//! (`calcar`) la dejo en borrones: sin rayas finas, el titulo en garabatos,
//! la circunferencia perdida. Pidio «dibujalo usando las herramientas
//! correctas, no solo lapiz».
//!
//! Aqui la geometria **la mide Rust**, sin modelo y en decenas de ms:
//!
//! 1. **Mascaras** ([`mascara`]): el papel, la tinta, sus colores (k-medias
//!    con el antialias atribuido a su color), lo que es **relleno** (lo que
//!    sobrevive a una erosion: regiones, puntos, puntas de flecha) y lo que
//!    es **trazo** (lo fino). El texto que leyo el OCR se aparta antes.
//! 2. **Esqueleto** de los trazos (Zhang-Suen) y sus **cadenas** de cruce a
//!    cruce, troceadas en tramos rectos y de arco ([`ajuste`]: recta de
//!    minimo error perpendicular; circunferencia de Kasa refinada con
//!    Gauss-Newton).
//! 3. **Juntar** ([`geometria`]): los tramos colineales en un segmento (con
//!    su grosor y color medidos, y discontinuo o punteado si el perfil de
//!    tinta tiene huecos regulares); los arcos de la misma circunferencia en
//!    una (entera si da la vuelta) o en arcos con sus extremos, partidos
//!    donde cambia el color (el arco verde de una circunferencia negra).
//! 4. **Puntos** (circulos rellenos pequenos), **puntas de flecha**,
//!    **marcas de angulo** y **regiones** de color (con su color medio; un
//!    degradado queda en bandas), **recuadros** (cuatro segmentos en
//!    rectangulo) y los **rotulos**: el texto del OCR y las letras sueltas
//!    que el OCR no vio, leidas aparte ([`Lector`]).
//! 5. **Topologia**: los extremos de los segmentos se pegan a los puntos, a
//!    la circunferencia y a los cruces con otras rectas.
//!
//! La nube (con vision) solo interpreta lo dudoso —las etiquetas que el OCR
//! no leyo bien— despues, sin frenar lo primero que se ve
//! (`hoja::vision::pedir_rotulos`).
//!
//! Lo usan `hoja::montaje` (zonas de dibujo que resultan diagrama) y el
//! calco (`calcar_imagen` con una lamina con texto, en la app).

pub mod ajuste;
mod geometria;
pub mod mascara;
pub mod restricciones;
mod salida;

#[cfg(test)]
mod pruebas;

use std::time::Instant;

use crate::hoja::{Caja, PalabraHoja};
use mascara::{Img, P};

pub use salida::a_elementos;
pub(crate) use salida::elementos;

/// Como va un trazo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Estilo {
    #[default]
    Solido,
    Discontinuo,
    Punteado,
}

/// Un segmento recto (o flecha).
#[derive(Debug, Clone, PartialEq)]
pub struct Segmento {
    pub a: P,
    pub b: P,
    pub grosor: f64,
    pub color: [u8; 3],
    pub estilo: Estilo,
    pub flecha_a: bool,
    pub flecha_b: bool,
}

impl Segmento {
    pub fn largo(&self) -> f64 {
        ajuste::dist(self.a, self.b)
    }
}

/// Una circunferencia o un arco suyo. `inicio` y `barrido` en radianes de
/// la pantalla (la y baja: un barrido positivo va en el sentido del reloj);
/// `barrido >= 2 pi` es la circunferencia entera.
#[derive(Debug, Clone, PartialEq)]
pub struct Arco {
    pub c: P,
    pub r: f64,
    pub inicio: f64,
    pub barrido: f64,
    pub grosor: f64,
    pub color: [u8; 3],
    pub estilo: Estilo,
}

impl Arco {
    pub fn entera(&self) -> bool {
        self.barrido >= std::f64::consts::TAU - 1e-6
    }
    pub fn extremos(&self) -> (P, P) {
        let a = (
            self.c.0 + self.r * self.inicio.cos(),
            self.c.1 + self.r * self.inicio.sin(),
        );
        let f = self.inicio + self.barrido;
        (
            a,
            (self.c.0 + self.r * f.cos(), self.c.1 + self.r * f.sin()),
        )
    }
}

/// Un punto (circulito relleno).
#[derive(Debug, Clone, PartialEq)]
pub struct Punto {
    pub c: P,
    pub r: f64,
    pub color: [u8; 3],
}

/// Una region rellena (con agujeros).
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    pub contorno: Vec<P>,
    pub huecos: Vec<Vec<P>>,
    pub color: [u8; 3],
    pub area: f64,
}

/// **Un ovalo** de ejes horizontales: centro, semiejes, su trazo y su relleno.
#[derive(Debug, Clone, PartialEq)]
pub struct Ovalo {
    pub c: P,
    pub rx: f64,
    pub ry: f64,
    pub grosor: f64,
    pub color: [u8; 3],
    pub relleno: Option<[u8; 3]>,
    /// Un rombo (la decision de un diagrama de flujo) con esa caja, no un ovalo.
    pub rombo: bool,
}

/// **Una marca de angulo**: el sector relleno de vertice `v` entre sus dos
/// rayas (de `inicio`, barriendo `barrido`, radianes de pantalla).
#[derive(Debug, Clone, PartialEq)]
pub struct Marca {
    pub v: P,
    pub r: f64,
    pub inicio: f64,
    pub barrido: f64,
    pub color: [u8; 3],
}

/// Un rectangulo de cuatro segmentos.
#[derive(Debug, Clone, PartialEq)]
pub struct Recuadro {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub grosor: f64,
    pub color: [u8; 3],
    /// Su relleno (el azul claro de una caja de un diagrama de flujo).
    pub relleno: Option<[u8; 3]>,
}

/// Un texto del diagrama (del OCR o una letra suelta leida aparte).
#[derive(Debug, Clone, PartialEq)]
pub struct Rotulo {
    pub texto: String,
    pub caja: Caja,
    pub color: Option<[u8; 3]>,
    pub negrita: bool,
    /// Una letra suelta (su caja es el alto de la mayuscula, no el renglon).
    pub letra_suelta: bool,
}

/// Una mancha pequena que parece una letra que el OCR no leyo: se lee
/// aparte; si nada la lee, sus trazos (`trazos`) van como lineas.
#[derive(Debug, Clone, PartialEq)]
pub struct Glifo {
    pub caja: Caja,
    pub color: [u8; 3],
    pub trazos: Vec<Vec<P>>,
    pub grosor: f64,
    /// Sus pixeles (x, y) en el recorte: solo ellos van a la hoja de letras.
    pub pixeles: Vec<(u32, u32)>,
}

/// **El diagrama medido**, en pixeles del recorte.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Diagrama {
    pub ancho: usize,
    pub alto: usize,
    pub segmentos: Vec<Segmento>,
    pub arcos: Vec<Arco>,
    pub puntos: Vec<Punto>,
    pub regiones: Vec<Region>,
    pub recuadros: Vec<Recuadro>,
    /// Las puntas de flecha ABIERTAS (dos rayas en V, no un triangulo).
    pub puntas_abiertas: Vec<P>,
    /// Los ovalos (el «Inicio» y el «Fin» de un diagrama de flujo).
    pub ovalos: Vec<Ovalo>,
    /// Las marcas de angulo (sectores rellenos en un vertice).
    pub marcas: Vec<Marca>,
    pub rotulos: Vec<Rotulo>,
    pub glifos: Vec<Glifo>,
    /// Que parte de la tinta (sin el texto) explican las figuras (0-1): un
    /// dibujo a mano o una foto se explican poco y van al calco.
    pub explica: f64,
    /// Cuanta tinta habia (pixeles, sin el texto).
    pub tinta: usize,
    /// Que parte de la tinta es relleno (regiones).
    pub de_relleno: f64,
    /// El largo mediano de los trazos (segmentos y arcos), en pixeles.
    pub largo_mediano: f64,
    /// Que parte del recorte es papel, y lo que se aparta del papel lo que
    /// lo es (el grano): una foto no tiene papel liso.
    pub papel: f64,
    pub grano: f64,
    pub ms: u128,
}

impl Diagrama {
    /// Cuantas figuras salieron (sin los rotulos).
    pub fn figuras(&self) -> usize {
        self.segmentos.len()
            + self.arcos.len()
            + self.puntos.len()
            + self.regiones.len()
            + self.recuadros.len()
            + self.marcas.len()
            + self.ovalos.len()
    }
    /// **Si esto es de verdad un diagrama** (y no una ilustracion o una
    /// foto): la geometria explica casi toda la tinta, y hay geometria.
    pub fn es_diagrama(&self) -> bool {
        let lineas = self.segmentos.len() + self.arcos.len() + self.recuadros.len();
        // Una foto tambien «se explica» con sus manchas de color y mil
        // rayitas de ruido: un diagrama tiene trazos largos y no es todo relleno.
        let largo_min = 12f64.max(0.02 * self.ancho.max(self.alto) as f64);
        lineas >= 2
            && self.explica >= UMBRAL_DIAGRAMA
            && self.largo_mediano >= largo_min
            && self.de_relleno <= 0.9
            && self.papel >= 0.3
            && self.grano <= 10.0
    }
    /// Lo que salio, en corto.
    pub fn resumen(&self) -> String {
        format!(
            "{} segmentos, {} arcos/circunferencias, {} puntos, {} regiones, {} recuadros, {} rotulos (explica {:.0} %, relleno {:.0} %, largo mediano {:.0} px, papel {:.0} % grano {:.1})",
            self.segmentos.len(),
            self.arcos.len(),
            self.puntos.len(),
            self.regiones.len(),
            self.recuadros.len(),
            self.rotulos.len() + self.glifos.len(),
            self.explica * 100.0,
            self.de_relleno * 100.0,
            self.largo_mediano,
            self.papel * 100.0,
            self.grano
        )
    }
}

/// Lo que tienen que explicar las figuras para que sea diagrama.
pub const UMBRAL_DIAGRAMA: f64 = 0.72;

/// **Quien lee las letras sueltas**: recibe una imagen RGBA (ancho, alto) y
/// devuelve las palabras con su caja en esa imagen (el OCR de Windows en la
/// app; uno de mentira en las pruebas).
pub type Lector = dyn Fn(&[u8], u32, u32) -> Vec<PalabraHoja> + Sync;

/// Las variables.
#[derive(Debug, Clone, PartialEq)]
pub struct Opciones {
    /// El alto de letra del texto de alrededor (pixeles del recorte).
    pub alto_letra: f64,
    /// Mas lejos del papel que esto es tinta (distancia de color RGB).
    pub umbral_tinta: f32,
    /// Texto de alrededor que no es del diagrama (de otras zonas de la hoja
    /// que caen en el recorte): no se vectoriza ni va como rotulo.
    pub tapar: Vec<Caja>,
    /// **El texto de la hoja** (renglones del OCR de alrededor, lo que dijo
    /// la nube): de aqui salen los hechos ([`restricciones::Hecho`]) que
    /// hacen exacta la geometria («AB es diametro», «cuadrante»).
    pub pistas: Vec<String>,
    /// Aplicar la geometria exacta ([`restricciones::aplicar`]).
    pub exacta: bool,
}

impl Default for Opciones {
    fn default() -> Self {
        Opciones {
            alto_letra: 14.0,
            umbral_tinta: 34.0,
            tapar: Vec::new(),
            pistas: Vec::new(),
            exacta: true,
        }
    }
}

/// **Mide el diagrama** de un recorte RGBA. `palabras`: lo que leyo el OCR
/// dentro (en pixeles del recorte), que va como rotulo y no se vectoriza.
/// `lector`: para las letras sueltas que el OCR no vio (sin el, van como
/// trazos). Con `o.exacta`, la geometria sale exacta figura a figura, con
/// los hechos de `o.pistas` y de sus propios rotulos.
pub fn vectorizar(
    rgba: &[u8],
    ancho: u32,
    alto: u32,
    palabras: &[PalabraHoja],
    o: &Opciones,
    lector: Option<&Lector>,
) -> Diagrama {
    let t0 = Instant::now();
    let (w, h) = (ancho as usize, alto as usize);
    if w < 4 || h < 4 || rgba.len() < w * h * 4 {
        return Diagrama {
            ancho: w,
            alto: h,
            ..Default::default()
        };
    }
    let img = Img::de_rgba(rgba, w, h);
    let mut d = geometria::medir(&img, palabras, o);
    if let Some(l) = lector {
        geometria::leer_glifos(&mut d, &img, l, o);
    }
    if o.exacta {
        exacta(&mut d, &o.pistas, o.alto_letra);
        geometria::marcas_por_color(&img, &mut d, o.alto_letra.max(6.0));
    }
    d.ms = t0.elapsed().as_millis();
    d
}

/// **La geometria exacta** ([`restricciones`]): los hechos de `pistas` y de
/// los rotulos del diagrama, figura a figura.
pub fn exacta(d: &mut Diagrama, pistas: &[String], alto_letra: f64) {
    let mut textos: Vec<String> = pistas.to_vec();
    textos.extend(d.rotulos.iter().map(|r| r.texto.clone()));
    let hechos = restricciones::hechos_de_texto(&textos);
    restricciones::aplicar(d, &hechos, alto_letra.max(6.0));
    quitar_arquitos_sueltos(d, alto_letra.max(6.0));
    pulir_rotulos(d);
    texto_de_recuadros(d);
}

/// Los rotulos que dijo la nube (texto y su centro en fracciones 0-1 del
/// recorte) puestos en el diagrama: corrigen al OCR o a la letra suelta
/// que caen cerca, y lo que no cae cerca de nada se anade (si no pisa), con
/// el color de la tinta de alli si se pasa el recorte (`rgba`, ancho, alto).
pub fn corregir_rotulos(
    d: &mut Diagrama,
    de_la_nube: &[(String, f64, f64)],
    alto_letra: f64,
    recorte: Option<(&[u8], u32, u32)>,
) {
    let img = recorte
        .filter(|r| r.0.len() >= (r.1 * r.2 * 4) as usize)
        .map(|r| Img::de_rgba(r.0, r.1 as usize, r.2 as usize));
    geometria::corregir_rotulos(d, de_la_nube, alto_letra, img.as_ref());
}

/// **Los rotulos, limpios y de su tamano** (15-oct): un «∢» o un «<» solo no
/// es un rotulo (lo que el OCR saco de un signo dentro de «θ=45°»), y uno
/// cortito mas pequeno que los demas («45°», la O del centro, que el OCR
/// leyo en minuscula) se pone del alto de las otras letras.
fn pulir_rotulos(d: &mut Diagrama) {
    d.rotulos
        .retain(|r| r.texto.chars().any(|c| c.is_alphanumeric()));
    // «θ=<45°»: el signo de angulo que el OCR ve en el trazo del 4 sobra.
    for r in d.rotulos.iter_mut() {
        for (de, a) in [("=∢", "="), ("=<", "="), ("= ∢", "= "), ("= <", "= ")] {
            r.texto = r.texto.replace(de, a);
        }
    }
    // El alto de mayuscula de cada rotulo corto.
    let mayuscula = |r: &Rotulo| {
        if r.letra_suelta {
            r.caja.h
        } else {
            r.caja.h * 0.72
        }
    };
    let mut altos: Vec<f64> = d
        .rotulos
        .iter()
        .filter(|r| r.texto.chars().count() <= 4)
        .map(mayuscula)
        .collect();
    if altos.len() < 3 {
        return;
    }
    altos.sort_by(f64::total_cmp);
    let m = altos[altos.len() / 2];
    for r in d
        .rotulos
        .iter_mut()
        .filter(|r| r.texto.chars().count() <= 4)
    {
        let c = mayuscula(r);
        if c > 0.0 && c < 0.75 * m {
            let k = m / c;
            let (cx, cy) = (r.caja.cx(), r.caja.cy());
            r.caja = Caja::nueva(
                cx - r.caja.w * k / 2.0,
                cy - r.caja.h * k / 2.0,
                r.caja.w * k,
                r.caja.h * k,
            );
        }
    }
}

/// **El texto de un recuadro, uno**: dentro del marco verde de «θ=45°» el
/// OCR leia «e», «∢» y la nube anadia «θ=45°» al lado: queda el mas largo,
/// centrado en el recuadro y de su alto.
fn texto_de_recuadros(d: &mut Diagrama) {
    for q in &d.recuadros {
        let caja = Caja::de_esquinas(q.x0, q.y0, q.x1, q.y1);
        let dentro: Vec<usize> = (0..d.rotulos.len())
            .filter(|&k| {
                caja.crecer(2.0)
                    .contiene(d.rotulos[k].caja.cx(), d.rotulos[k].caja.cy())
            })
            .collect();
        // (Un recuadro que lleva letras sueltas de una figura, como el de «a» y
        // «b» en O, no: el texto tiene que llenarlo.)
        let lleno: f64 = dentro.iter().map(|&k| d.rotulos[k].caja.area()).sum();
        if dentro.is_empty() || lleno < 0.2 * caja.area() {
            continue;
        }
        let Some(&mejor) = dentro
            .iter()
            .max_by_key(|&&k| d.rotulos[k].texto.chars().count())
        else {
            continue;
        };
        let mut r = d.rotulos[mejor].clone();
        let h = (caja.h * 0.55).max(r.caja.h);
        let w = (h * 0.6 * r.texto.chars().count() as f64).min(caja.w * 0.9);
        r.caja = Caja::nueva(caja.cx() - w / 2.0, caja.cy() - h / 2.0, w, h);
        r.letra_suelta = false;
        let mut k = 0;
        d.rotulos.retain(|_| {
            k += 1;
            !dentro.contains(&(k - 1))
        });
        d.rotulos.push(r);
        // Y las rayitas de dentro (trozos de las letras del recuadro).
        let interior = caja.crecer(-2.0);
        d.segmentos
            .retain(|s| !(interior.contiene(s.a.0, s.a.1) && interior.contiene(s.b.0, s.b.1)));
        d.arcos.retain(|a| {
            let (p, q) = a.extremos();
            a.entera()
                || !(interior.contiene(p.0, p.1) && interior.contiene(q.0, q.1) && a.r < caja.h)
        });
    }
}

/// **Los arquitos sueltos** (15-oct): un arco pequeno que no toca nada por
/// ninguno de sus dos extremos es el trozo de una letra (la «a» y la «b»
/// rojas junto a O salian como dos parentesis), no geometria.
fn quitar_arquitos_sueltos(d: &mut Diagrama, hm: f64) {
    let toca = |p: P, d: &Diagrama, yo: usize| {
        d.segmentos
            .iter()
            .any(|s| ajuste::dist_a_segmento(p, s.a, s.b) <= 3.0 + s.grosor / 2.0)
            || d.puntos.iter().any(|q| ajuste::dist(p, q.c) <= q.r + 3.0)
            || d.arcos
                .iter()
                .enumerate()
                .any(|(j, a)| j != yo && (ajuste::dist(p, a.c) - a.r).abs() <= 3.0 + a.grosor / 2.0)
    };
    let mut quitar = Vec::new();
    for (i, a) in d.arcos.iter().enumerate() {
        if a.entera() || a.r > 1.5 * hm || a.barrido > 1.2 * std::f64::consts::PI {
            continue;
        }
        let (p, q) = a.extremos();
        if !toca(p, d, i) && !toca(q, d, i) {
            quitar.push(i);
        }
    }
    let mut k = 0;
    d.arcos.retain(|_| {
        k += 1;
        !quitar.contains(&(k - 1))
    });
}
