//! **La pizarra**: las piezas sueltas con las que se dibujan las graficas
//! de funciones, los diagramas de fisica y los problemas en limpio.
//!
//! No es otro dibujo: son `Elemento` del motor del lienzo
//! (`pixpin_motor2d`) —lineas, flechas, regiones, puntos, textos y
//! ecuaciones compuestas con las cajas de `ecuacion`— que luego se escriben
//! con su escritor de Excalidraw (`Fabrica::de_motor`). Asi lo que dibuja el
//! asistente para un estudiante se mueve, se recolorea y sale en el PDF y en
//! el movil como cualquier trazo. Todo liso (rugosidad 0): una pizarra de
//! clase se quiere limpia, no a mano alzada.
//!
//! Los ejes con **dos escalas** ([`Ejes`]) son para lo que el instrumento
//! (que tiene una sola unidad) no puede: una grafica x-t con metros arriba y
//! segundos al lado.

use pixpin_motor2d::ecuacion::{self, Estilo};
use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pixpin_motor2d::formas::TipoPunta;
use pixpin_motor2d::relleno::EstiloRelleno;
use pixpin_motor2d::vector::Punto2;

/// La letra de los rotulos: con serifa, como las ecuaciones.
pub const LETRA: &str = "Fraunces";

pub const TINTA: ColorRgba = ColorRgba::opaco(0.12, 0.12, 0.12);
pub const GRIS: ColorRgba = ColorRgba::opaco(0.45, 0.45, 0.48);
pub const ROJO: ColorRgba = ColorRgba::opaco(0.878, 0.192, 0.192);
pub const AZUL: ColorRgba = ColorRgba::opaco(0.098, 0.443, 0.761);
pub const VERDE: ColorRgba = ColorRgba::opaco(0.184, 0.62, 0.267);
pub const NARANJA: ColorRgba = ColorRgba::opaco(0.941, 0.549, 0.0);
pub const MORADO: ColorRgba = ColorRgba::opaco(0.612, 0.212, 0.71);
pub const TURQUESA: ColorRgba = ColorRgba::opaco(0.047, 0.522, 0.6);

/// Los colores de las series, en orden (los de la grafica del lienzo).
pub const SERIES: [ColorRgba; 6] = [ROJO, AZUL, VERDE, NARANJA, MORADO, TURQUESA];

pub fn color_de_serie(k: usize) -> ColorRgba {
    SERIES[k % SERIES.len()]
}

/// Como se coloca un texto respecto a su punto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alinear {
    Izquierda,
    Centro,
    Derecha,
}

/// Un numero escrito para una persona: como mucho tres decimales, sin ceros
/// colgando y sin «-0».
pub fn numero(v: f64) -> String {
    if !v.is_finite() {
        return "?".into();
    }
    let r = (v * 1000.0).round() / 1000.0;
    let r = if r == 0.0 { 0.0 } else { r };
    let mut s = format!("{r:.3}");
    while s.contains('.') && (s.ends_with('0') || s.ends_with('.')) {
        s.pop();
    }
    s
}

/// Si en `c[i]` empieza una palabra suelta `w` (sin letras pegadas delante
/// ni detras).
fn palabra_suelta(c: &[char], i: usize, w: &str) -> bool {
    let w: Vec<char> = w.chars().collect();
    c.len() >= i + w.len()
        && c[i..i + w.len()] == w[..]
        && (i == 0 || !c[i - 1].is_alphabetic())
        && c.get(i + w.len()).is_none_or(|d| !d.is_alphabetic())
}

/// **Un titulo o rotulo escrito por el modelo, como en la pizarra**: `x^3`
/// → x³, `x^-1` → x⁻¹, `pi` → π, `theta` → θ, `sqrt` → √, `<=` → ≤, `*` → ·.
/// `polar`: la `t` suelta es el angulo θ. Para textos (un titulo mezcla
/// palabras y formula); una formula sola se compone con `caja_de_ecuacion`.
pub fn a_pizarra(s: &str, polar: bool) -> String {
    const ARRIBA: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] == '^' {
            // `^{12}`, `^12`, `^-1`, `^n`.
            let (mut j, llaves) = if c.get(i + 1) == Some(&'{') {
                (i + 2, true)
            } else {
                (i + 1, false)
            };
            let mut sub = String::new();
            let mut ok = true;
            while j < c.len()
                && (llaves && c[j] != '}'
                    || !llaves && (c[j].is_ascii_digit() || (c[j] == '-' && sub.is_empty())))
            {
                match c[j] {
                    d @ '0'..='9' => sub.push(ARRIBA[d as usize - '0' as usize]),
                    '-' => sub.push('⁻'),
                    'n' => sub.push('ⁿ'),
                    _ => ok = false,
                }
                j += 1;
            }
            if !llaves && sub.is_empty() && c.get(i + 1) == Some(&'n') {
                sub.push('ⁿ');
                j = i + 2;
            }
            if ok && !sub.is_empty() {
                out.push_str(&sub);
                i = if llaves { j + 1 } else { j };
                continue;
            }
        }
        let mut cambio = None;
        for (w, por) in [
            ("pi", "π"),
            ("theta", "θ"),
            ("sqrt", "√"),
            ("alpha", "α"),
            ("phi", "φ"),
            ("mu", "μ"),
            ("omega", "ω"),
            ("lambda", "λ"),
            ("Delta", "Δ"),
            ("infinito", "∞"),
            ("inf", "∞"),
        ] {
            let con_barra = c[i] == '\\' && palabra_suelta(&c, i + 1, w);
            if palabra_suelta(&c, i, w) || con_barra {
                cambio = Some((por, w.chars().count() + usize::from(con_barra)));
                break;
            }
        }
        if let Some((por, n)) = cambio {
            out.push_str(por);
            i += n;
            continue;
        }
        if polar && palabra_suelta(&c, i, "t") {
            out.push('θ');
            i += 1;
            continue;
        }
        match (c[i], c.get(i + 1)) {
            ('<', Some('=')) => {
                out.push('≤');
                i += 2;
                continue;
            }
            ('>', Some('=')) => {
                out.push('≥');
                i += 2;
                continue;
            }
            ('*', _) => out.push('·'),
            (ch, _) => out.push(ch),
        }
        i += 1;
    }
    out
}

/// **Una formula de la grafica, lista para componer** en la leyenda: sin el
/// `*` de la cuenta entre un numero y lo que multiplica (`2*sin(3*t)` →
/// `2sin(3t)`) y, en una polar, con θ por la `t`.
pub fn para_ver(f: &str, polar: bool) -> String {
    let c: Vec<char> = f.chars().collect();
    let mut out = String::with_capacity(f.len());
    for (i, &ch) in c.iter().enumerate() {
        if ch == '*' {
            let antes = c[..i].iter().rev().find(|x| !x.is_whitespace());
            let despues = c[i + 1..].iter().find(|x| !x.is_whitespace());
            if antes.is_some_and(|a| a.is_ascii_digit() || *a == ')')
                && despues.is_some_and(|d| d.is_alphabetic() || *d == '(')
            {
                continue;
            }
        }
        if polar
            && ch == 't'
            && (i == 0 || !c[i - 1].is_alphabetic())
            && c.get(i + 1).is_none_or(|d| !d.is_alphabetic())
        {
            out.push('θ');
            continue;
        }
        out.push(ch);
    }
    out
}

/// Lo que mide un texto en la letra de la pizarra.
pub fn medir(t: &str, tam: f32) -> (f32, f32) {
    crate::lienzo::medir_con(t, tam, LETRA)
}

/// Los elementos que se van dibujando.
#[derive(Default)]
pub struct Pizarra {
    pub el: Vec<Elemento>,
    /// Los rotulos de las flechas, aun sin poner: se colocan todos juntos
    /// al final ([`Pizarra::colocar_rotulos`]), cuando ya se sabe que hay
    /// dibujado y no se pisan ni con el bloque ni con otras flechas.
    pendientes: Vec<Pendiente>,
}

/// Un rotulo por poner: su ancla, el sitio de siempre, el texto y su color.
#[derive(Debug, Clone)]
struct Pendiente {
    ancla: (f64, f64),
    preferida: (f64, f64),
    texto: String,
    tam: f32,
    color: ColorRgba,
}

fn liso(mut e: Elemento) -> Elemento {
    e.rugosidad = 0.0;
    e
}

fn caja_de(puntos: &[Punto2]) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in puntos {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    (x0, y0, x1, y1)
}

fn p(x: f64, y: f64) -> Punto2 {
    Punto2::nuevo(x as f32, y as f32)
}

impl Pizarra {
    pub fn nueva() -> Pizarra {
        Pizarra::default()
    }

    /// Una polilinea recta por esos puntos.
    pub fn linea(&mut self, puntos: &[(f64, f64)], color: ColorRgba, grosor: f32) {
        if puntos.len() < 2 {
            return;
        }
        let estilo = Estilo {
            color,
            tam: 16.0,
            opacidad: 1.0,
            familia: LETRA.into(),
        };
        let pts: Vec<Punto2> = puntos.iter().map(|&(x, y)| p(x, y)).collect();
        self.el
            .push(liso(ecuacion::elemento_linea(pts, grosor, &estilo)));
    }

    /// Lo mismo, a trazos (asintotas, componentes, guias).
    pub fn discontinua(&mut self, puntos: &[(f64, f64)], color: ColorRgba, grosor: f32) {
        self.linea(puntos, color, grosor);
        if let Some(e) = self.el.last_mut() {
            e.estilo = EstiloTrazo::Discontinuo;
        }
    }

    /// Una flecha de `a` a `b` (un vector, una fuerza).
    pub fn flecha(&mut self, a: (f64, f64), b: (f64, f64), color: ColorRgba, grosor: f32) {
        let pts = vec![p(a.0, a.1), p(b.0, b.1)];
        let (x0, y0, x1, y1) = caja_de(&pts);
        self.el.push(Elemento {
            figura: Figura::Flecha {
                puntos: pts,
                punta_inicio: TipoPunta::Ninguna,
                punta_fin: TipoPunta::Flecha,
                codos: false,
            },
            x: a.0 as f32,
            y: a.1 as f32,
            ancho: x1 - x0,
            alto: y1 - y0,
            trazo: color,
            grosor,
            rugosidad: 0.0,
            ..Default::default()
        });
    }

    /// Una flecha a trazos (las componentes de una fuerza).
    pub fn flecha_discontinua(
        &mut self,
        a: (f64, f64),
        b: (f64, f64),
        color: ColorRgba,
        grosor: f32,
    ) {
        self.flecha(a, b, color, grosor);
        if let Some(e) = self.el.last_mut() {
            e.estilo = EstiloTrazo::Discontinuo;
        }
    }

    /// Un texto de una linea; devuelve lo que mide.
    pub fn texto(
        &mut self,
        t: &str,
        x: f64,
        y: f64,
        tam: f32,
        color: ColorRgba,
        alinear: Alinear,
    ) -> (f32, f32) {
        self.texto_con(t, x, y, tam, color, alinear, false)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn texto_con(
        &mut self,
        t: &str,
        x: f64,
        y: f64,
        tam: f32,
        color: ColorRgba,
        alinear: Alinear,
        cursiva: bool,
    ) -> (f32, f32) {
        if t.is_empty() {
            return (0.0, 0.0);
        }
        let (w, h) = medir(t, tam);
        let x = match alinear {
            Alinear::Izquierda => x as f32,
            Alinear::Centro => x as f32 - w / 2.0,
            Alinear::Derecha => x as f32 - w,
        };
        let estilo = Estilo {
            color,
            tam,
            opacidad: 1.0,
            familia: LETRA.into(),
        };
        self.el.push(ecuacion::elemento_texto(
            t,
            x,
            y as f32,
            (w, h),
            tam,
            cursiva,
            &estilo,
        ));
        (w, h)
    }

    /// Un parrafo partido en renglones de `ancho` como mucho; devuelve el
    /// alto que ocupa.
    pub fn parrafo(
        &mut self,
        t: &str,
        x: f64,
        y: f64,
        ancho: f64,
        tam: f32,
        color: ColorRgba,
    ) -> f64 {
        let mut alto = 0.0;
        for r in renglones(t, ancho, tam) {
            let (_, h) = self.texto(&r, x, y + alto, tam, color, Alinear::Izquierda);
            alto += h.max(tam * 1.2) as f64;
        }
        alto
    }

    /// Un rectangulo, con relleno liso si se da (y su opacidad).
    pub fn rectangulo(
        &mut self,
        [x, y, w, h]: [f64; 4],
        color: ColorRgba,
        relleno: Option<ColorRgba>,
        grosor: f32,
    ) {
        self.el.push(Elemento {
            figura: Figura::Rectangulo,
            x: x as f32,
            y: y as f32,
            ancho: w as f32,
            alto: h as f32,
            trazo: color,
            relleno,
            estilo_relleno: EstiloRelleno::Solido,
            grosor,
            rugosidad: 0.0,
            ..Default::default()
        });
    }

    pub fn elipse(
        &mut self,
        [x, y, w, h]: [f64; 4],
        color: ColorRgba,
        relleno: Option<ColorRgba>,
        grosor: f32,
    ) {
        self.el.push(Elemento {
            figura: Figura::Elipse,
            x: x as f32,
            y: y as f32,
            ancho: w as f32,
            alto: h as f32,
            trazo: color,
            relleno,
            estilo_relleno: EstiloRelleno::Solido,
            grosor,
            rugosidad: 0.0,
            ..Default::default()
        });
    }

    /// Una mancha rellena (el area entre dos curvas), translucida.
    pub fn region(&mut self, contorno: &[(f64, f64)], color: ColorRgba, opacidad: f32) {
        if contorno.len() < 3 {
            return;
        }
        let pts: Vec<Punto2> = contorno.iter().map(|&(x, y)| p(x, y)).collect();
        let (x0, y0, x1, y1) = caja_de(&pts);
        self.el.push(Elemento {
            figura: Figura::Region {
                contorno: pts,
                huecos: Vec::new(),
            },
            x: x0,
            y: y0,
            ancho: x1 - x0,
            alto: y1 - y0,
            trazo: color,
            relleno: Some(color),
            estilo_relleno: EstiloRelleno::Solido,
            grosor: 0.5,
            opacidad,
            rugosidad: 0.0,
            ..Default::default()
        });
    }

    /// Un poligono cerrado sin relleno (un bloque girado, un plano).
    pub fn poligono(&mut self, puntos: &[(f64, f64)], color: ColorRgba, grosor: f32) {
        let mut v = puntos.to_vec();
        if let Some(&p0) = puntos.first() {
            v.push(p0);
        }
        self.linea(&v, color, grosor);
    }

    /// Un punto marcado con su rotulo al lado (una raiz, un maximo).
    pub fn punto(&mut self, x: f64, y: f64, rotulo: &str, color: ColorRgba) {
        self.punto_con(x, y, rotulo, color, -std::f32::consts::FRAC_PI_4);
    }

    /// Lo mismo con el rotulo hacia `angulo` (radianes de pantalla: 0 a la
    /// derecha, -π/2 arriba).
    pub fn punto_con(&mut self, x: f64, y: f64, rotulo: &str, color: ColorRgba, angulo: f32) {
        self.el.push(Elemento {
            figura: Figura::Punto {
                letra: rotulo.to_string(),
                angulo,
                radio: 12.0,
            },
            x: x as f32,
            y: y as f32,
            trazo: color,
            relleno: Some(color),
            grosor: 2.0,
            rugosidad: 0.0,
            ..Default::default()
        });
    }

    /// Una ecuacion compuesta (LaTeX o escrita normal) con su esquina en
    /// `(x, y)`; devuelve lo que mide.
    pub fn ecuacion(
        &mut self,
        texto: &str,
        x: f64,
        y: f64,
        tam: f32,
        color: ColorRgba,
    ) -> (f32, f32) {
        let (els, medida) = caja_de_ecuacion(texto, x, y, tam, color);
        self.el.extend(els);
        medida
    }

    /// Un arco de circunferencia (el de un angulo), con los grados como en
    /// clase: desde el eje x, en sentido antihorario.
    pub fn arco(
        &mut self,
        (cx, cy): (f64, f64),
        radio: f64,
        desde: f64,
        hasta: f64,
        color: ColorRgba,
        grosor: f32,
    ) {
        let n = 24;
        let pts: Vec<(f64, f64)> = (0..=n)
            .map(|k| (desde + (hasta - desde) * k as f64 / n as f64).to_radians())
            .map(|a| (cx + radio * a.cos(), cy - radio * a.sin()))
            .collect();
        self.linea(&pts, color, grosor);
    }

    /// Una flecha que sale de `origen` con ese `largo` y ese angulo (en
    /// grados, como en clase: 0 a la derecha, 90 arriba), con su rotulo en
    /// la punta. Devuelve la punta.
    pub fn vector(
        &mut self,
        origen: (f64, f64),
        largo: f64,
        angulo: f64,
        rotulo: &str,
        color: ColorRgba,
        discontinua: bool,
    ) -> (f64, f64) {
        let a = angulo.to_radians();
        let punta = (origen.0 + largo * a.cos(), origen.1 - largo * a.sin());
        if discontinua {
            self.flecha_discontinua(origen, punta, color, 2.0);
        } else {
            self.flecha(origen, punta, color, 2.5);
        }
        self.rotulo_en(punta, a, rotulo, color);
        punta
    }

    /// Un rotulo junto a una punta que apunta hacia `a` (radianes): fuera
    /// de la flecha, para no pisarla.
    pub fn rotulo_en(&mut self, punta: (f64, f64), a: f64, rotulo: &str, color: ColorRgba) {
        if rotulo.is_empty() {
            return;
        }
        let tam = 15.0;
        let (w, h) = medir(rotulo, tam);
        let (dx, dy) = (a.cos(), -a.sin());
        let x = punta.0
            + dx * 10.0
            + if dx < -0.3 {
                -(w as f64)
            } else if dx > 0.3 {
                0.0
            } else {
                -(w as f64) / 2.0
            };
        let y = punta.1
            + dy * 10.0
            + if dy < -0.3 {
                -(h as f64)
            } else if dy > 0.3 {
                0.0
            } else {
                -(h as f64) / 2.0
            };
        self.pendientes.push(Pendiente {
            ancla: punta,
            preferida: (x, y),
            texto: rotulo.to_string(),
            tam,
            color,
        });
    }

    /// **Pone los rotulos pendientes sin que se pisen** (`rotulos::colocar`)
    /// con nada de lo dibujado: lineas, flechas, cajas, otros textos. Cada
    /// uno prueba primero su sitio de siempre (fuera de la punta).
    pub fn colocar_rotulos(&mut self) {
        if self.pendientes.is_empty() {
            return;
        }
        let mut ocupado: Vec<crate::rotulos::Caja> = Vec::new();
        for e in &self.el {
            match &e.figura {
                Figura::Linea { puntos } | Figura::Flecha { puntos, .. } => {
                    let v: Vec<(f64, f64)> =
                        puntos.iter().map(|q| (q.x as f64, q.y as f64)).collect();
                    ocupado.extend(crate::rotulos::cajas_de_linea(&v, 6.0));
                }
                _ => {
                    let (x0, y0, x1, y1) = e.caja();
                    if [x0, y0, x1, y1].iter().all(|v| v.is_finite()) {
                        ocupado.push([x0 as f64, y0 as f64, (x1 - x0) as f64, (y1 - y0) as f64]);
                    }
                }
            }
        }
        let pendientes = std::mem::take(&mut self.pendientes);
        let pedidos: Vec<crate::rotulos::Rotulo> = pendientes
            .iter()
            .map(|p| {
                let (w, h) = medir(&p.texto, p.tam);
                crate::rotulos::Rotulo {
                    ancla: p.ancla,
                    ancho: w as f64,
                    alto: h as f64,
                    preferida: Some(p.preferida),
                }
            })
            .collect();
        for (p, (x, y)) in pendientes
            .iter()
            .zip(crate::rotulos::colocar(&pedidos, &ocupado, None))
        {
            self.texto(&p.texto, x, y, p.tam, p.color, Alinear::Izquierda);
        }
    }

    /// Mueve todo lo dibujado desde `desde`.
    pub fn mover_desde(&mut self, desde: usize, dx: f64, dy: f64) {
        for e in self.el.iter_mut().skip(desde) {
            e.mover(dx as f32, dy as f32);
        }
    }
}

/// Las piezas de una ecuacion en `(x, y)` y lo que mide.
pub fn caja_de_ecuacion(
    texto: &str,
    x: f64,
    y: f64,
    tam: f32,
    color: ColorRgba,
) -> (Vec<Elemento>, (f32, f32)) {
    let medir_ec = |t: &str, tam: f32| crate::lienzo::medir_con(t, tam, crate::ecuacion::FAMILIA);
    let estilo = Estilo {
        color,
        tam,
        opacidad: 1.0,
        familia: crate::ecuacion::FAMILIA.into(),
    };
    let lineas: Vec<String> = crate::ecuacion::partir_lineas(texto);
    let cajas: Vec<ecuacion::Caja> = lineas
        .iter()
        .map(|l| crate::ecuacion::componer(&pixpin_notas::formulas::leer(l), tam, &medir_ec))
        .collect();
    let col = ecuacion::columna(cajas, tam * 0.5);
    (
        col.elementos(x as f32, y as f32, &estilo),
        (col.ancho, col.alto),
    )
}

/// Un texto partido en renglones que caben en `ancho` (por palabras; los
/// saltos de linea del texto se respetan).
pub fn renglones(t: &str, ancho: f64, tam: f32) -> Vec<String> {
    let mut v = Vec::new();
    for parrafo in t.split('\n') {
        let mut actual = String::new();
        for palabra in parrafo.split_whitespace() {
            let prueba = if actual.is_empty() {
                palabra.to_string()
            } else {
                format!("{actual} {palabra}")
            };
            if !actual.is_empty() && medir(&prueba, tam).0 as f64 > ancho {
                v.push(std::mem::take(&mut actual));
                actual = palabra.to_string();
            } else {
                actual = prueba;
            }
        }
        v.push(actual);
    }
    while v.last().is_some_and(String::is_empty) {
        v.pop();
    }
    v
}

/// **Unos ejes con una escala por eje**, para graficas de fisica (x-t,
/// v-t...): la caja en el lienzo y la ventana de valores. Las marcas, a un
/// paso «bonito» (`grafica::paso_bonito`), como la grafica del lienzo.
#[derive(Debug, Clone, Copy)]
pub struct Ejes {
    /// La caja del trazado en el lienzo: x, y, ancho, alto.
    pub caja: [f64; 4],
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
}

impl Ejes {
    /// Ventana con los dos ceros dentro si caen cerca, y un poco de margen
    /// arriba y abajo.
    pub fn nuevos(caja: [f64; 4], (x0, x1): (f64, f64), (y0, y1): (f64, f64)) -> Ejes {
        let (mut y0, mut y1) = (y0.min(0.0), y1.max(0.0));
        if (y1 - y0).abs() < 1e-9 {
            y0 -= 1.0;
            y1 += 1.0;
        }
        let m = (y1 - y0) * 0.08;
        if y0 < 0.0 {
            y0 -= m;
        }
        y1 += m;
        Ejes {
            caja,
            x0,
            x1: if x1 > x0 { x1 } else { x0 + 1.0 },
            y0,
            y1,
        }
    }

    pub fn a_lienzo(&self, x: f64, y: f64) -> (f64, f64) {
        let [cx, cy, w, h] = self.caja;
        (
            cx + (x - self.x0) / (self.x1 - self.x0) * w,
            cy + h - (y - self.y0) / (self.y1 - self.y0) * h,
        )
    }

    /// Los ejes con flechas, marcas, numeros y el nombre de cada uno.
    pub fn dibujar(&self, pz: &mut Pizarra, nombre_x: &str, nombre_y: &str) {
        let [cx, cy, w, h] = self.caja;
        let (_, y_cero) = self.a_lienzo(self.x0, 0.0_f64.clamp(self.y0, self.y1));
        let (x_cero, _) = self.a_lienzo(0.0_f64.clamp(self.x0, self.x1), self.y0);
        pz.flecha((cx, y_cero), (cx + w + 10.0, y_cero), TINTA, 1.5);
        pz.flecha((x_cero, cy + h), (x_cero, cy - 10.0), TINTA, 1.5);
        let tam = 12.0;
        // Un paso «bonito» que deje al menos 24 px entre numero y numero.
        let paso_que_cabe = |rango: f64, pixeles: f64| {
            let mut paso = pixpin_motor2d::grafica::paso_bonito(rango);
            while paso / rango * pixeles < 24.0 && paso < rango {
                paso *= 2.0;
            }
            paso
        };
        let paso_x = paso_que_cabe(self.x1 - self.x0, w);
        let paso_y = paso_que_cabe(self.y1 - self.y0, h);
        let mut x = (self.x0 / paso_x).ceil() * paso_x;
        while x <= self.x1 + 1e-9 {
            if x.abs() > 1e-9 {
                let (px, _) = self.a_lienzo(x, 0.0);
                pz.linea(&[(px, y_cero - 4.0), (px, y_cero + 4.0)], TINTA, 1.0);
                pz.texto(&numero(x), px, y_cero + 6.0, tam, GRIS, Alinear::Centro);
            }
            x += paso_x;
        }
        let mut y = (self.y0 / paso_y).ceil() * paso_y;
        while y <= self.y1 + 1e-9 {
            if y.abs() > 1e-9 {
                let (_, py) = self.a_lienzo(0.0, y);
                pz.linea(&[(x_cero - 4.0, py), (x_cero + 4.0, py)], TINTA, 1.0);
                pz.texto(
                    &numero(y),
                    x_cero - 6.0,
                    py - tam as f64 * 0.65,
                    tam,
                    GRIS,
                    Alinear::Derecha,
                );
            }
            y += paso_y;
        }
        pz.texto_con(
            nombre_x,
            cx + w + 14.0,
            y_cero - 9.0,
            15.0,
            TINTA,
            Alinear::Izquierda,
            true,
        );
        pz.texto_con(
            nombre_y,
            x_cero + 6.0,
            cy - 24.0,
            15.0,
            TINTA,
            Alinear::Izquierda,
            true,
        );
    }

    /// Una curva `y(x)` muestreada, cortada donde se sale de la ventana.
    pub fn curva(&self, pz: &mut Pizarra, f: &dyn Fn(f64) -> f64, color: ColorRgba, grosor: f32) {
        let n = 200;
        let mut rama: Vec<(f64, f64)> = Vec::new();
        for i in 0..=n {
            let x = self.x0 + (self.x1 - self.x0) * i as f64 / n as f64;
            let y = f(x);
            if !y.is_finite() || y < self.y0 - 1e-9 || y > self.y1 + 1e-9 {
                pz.linea(&std::mem::take(&mut rama), color, grosor);
                continue;
            }
            rama.push(self.a_lienzo(x, y));
        }
        pz.linea(&rama, color, grosor);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    #[allow(clippy::approx_constant)] // 1.41421 es un numero cualquiera con decimales
    fn los_numeros_se_escriben_para_personas() {
        assert_eq!(numero(2.0), "2");
        assert_eq!(numero(-0.0001), "0");
        assert_eq!(numero(1.41421), "1.414");
        assert_eq!(numero(40.8163), "40.816");
        assert_eq!(numero(f64::NAN), "?");
    }

    #[test]
    fn los_titulos_salen_como_en_la_pizarra() {
        assert_eq!(
            a_pizarra("f(x) = x^3 - 3x con tangente en x = 2", false),
            "f(x) = x³ - 3x con tangente en x = 2"
        );
        assert_eq!(
            a_pizarra("y = sin x en [0, pi]", false),
            "y = sin x en [0, π]"
        );
        assert_eq!(a_pizarra("r = 2 sin(3t)", true), "r = 2 sin(3θ)");
        assert_eq!(a_pizarra("x^{-1} y x^n, 2*x", false), "x⁻¹ y xⁿ, 2·x");
        // La t de una palabra no se toca, ni sin polar.
        assert_eq!(a_pizarra("tangente en t = 1", true), "tangente en θ = 1");
        assert_eq!(a_pizarra("tiempo t", false), "tiempo t");
        assert_eq!(a_pizarra("spin", false), "spin");
        assert_eq!(para_ver("r = 2*sin(3*t)", true), "r = 2sin(3θ)");
        assert_eq!(
            para_ver("x = cos(3*t), y = a*b", false),
            "x = cos(3t), y = a*b"
        );
    }

    #[test]
    fn los_renglones_caben_y_respetan_los_saltos() {
        let r = renglones(
            "uno dos tres cuatro cinco seis siete ocho nueve diez\notra",
            120.0,
            16.0,
        );
        assert!(r.len() >= 3, "{r:?}");
        assert_eq!(r.last().unwrap(), "otra");
        assert!(
            r.iter()
                .all(|l| medir(l, 16.0).0 <= 130.0 || !l.contains(' ')),
            "{r:?}"
        );
    }

    #[test]
    fn los_ejes_ponen_cada_valor_en_su_sitio() {
        let e = Ejes::nuevos([100.0, 100.0, 200.0, 100.0], (0.0, 10.0), (0.0, 50.0));
        let (x, _) = e.a_lienzo(0.0, 0.0);
        assert!((x - 100.0).abs() < 1e-9);
        let (x, y) = e.a_lienzo(10.0, e.y1);
        assert!((x - 300.0).abs() < 1e-9 && (y - 100.0).abs() < 1e-9);
        let mut pz = Pizarra::nueva();
        e.dibujar(&mut pz, "t (s)", "x (m)");
        e.curva(&mut pz, &|t| 5.0 * t, ROJO, 2.0);
        assert!(
            pz.el
                .iter()
                .any(|e| matches!(&e.figura, Figura::Linea { puntos } if puntos.len() > 100))
        );
    }
}
