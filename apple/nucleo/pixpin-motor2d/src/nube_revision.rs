//! **La nube de revision dinamica** (24-oct-2026): la de Bluebeam y Acrobat,
//! la que rodea en un plano lo que ha cambiado.
//!
//! La de antes era una pieza fija: una linea con los festones ya calculados
//! para 220 x 130. Al estirarla se estiraban los arcos con ella, y una nube
//! ancha y baja salia con unos festones aplastados que ya no parecian nube.
//! Lo que el usuario pidio es lo que hacen esos programas: **los festones no
//! cambian de tamano ni de forma; al crecer el contorno caben mas**.
//!
//! # Que es una nube aqui
//!
//! No es una figura nueva: es **un estilo de borde** sobre el rectangulo o la
//! elipse, como el `/BE /S /C` (borde nube) de las anotaciones PDF. El
//! elemento sigue siendo su caja (`x/y/ancho/alto`) y lleva aparte el radio
//! del feston (`Extras::nube`), que **no depende de la caja**. Asi todo lo
//! que ya sabe tratar un rectangulo —los tiradores, el giro, el relleno, el
//! grosor, el color, el trazo discontinuo, la opacidad, el rotulo de dentro,
//! las flechas atadas, el lazo— vale tal cual, y solo cambia por donde pasa
//! la raya. Al redimensionar solo cambia la caja; el contorno se recalcula al
//! pintar.
//!
//! # Como se reparte
//!
//! La caja es el borde de FUERA: la punta de cada feston la toca, y lo que se
//! selecciona y se exporta es exactamente lo que se ve. Por dentro va la base
//! (la caja metida la panza del feston) y sobre cada lado de la base se ponen
//! `n = largo / diametro` festones, redondeado y al menos uno, todos iguales.
//! En las esquinas se juntan el ultimo de un lado y el primero del siguiente
//! en el vertice de la base: esquinas limpias y el contorno cerrado. La base
//! elipse se reparte igual, por su perimetro.
//!
//! Cada feston es un arco de circunferencia con la panza fija ([`PANZA`] del
//! radio): como la cuerda solo cambia lo que da el redondeo, todos salen casi
//! del mismo tamano en cualquier caja.
//!
//! # Como viaja
//!
//! Al movil y a Excalidraw, que no la conocen, sale como **una linea cerrada
//! con los festones ya calculados** (se ve igual, y se rellena) y con dos
//! claves suyas: `nubeRadio` y `nubeBase`. El PC, al leerla, la recupera
//! parametrica con la caja de la linea (ver `excalidraw.rs`).

use crate::azar::Azar;
use crate::elemento::{ColorRgba, Elemento, Extras, Figura};
use crate::relleno::EstiloRelleno;
use crate::rough;
use crate::vector::Punto2;

/// El radio del feston que se pone si no se dice otro, en pixeles del
/// documento: un diametro de 24, el de las nubes de Bluebeam a escala 1.
pub const RADIO_DE_FABRICA: f32 = 12.0;
/// Por debajo de esto no se distingue un feston de un temblor.
pub const RADIO_MINIMO: f32 = 2.0;
/// Y por encima, una nube de tres festones no es una nube.
pub const RADIO_MAXIMO: f32 = 400.0;

/// **Cuanto sale cada feston de su cuerda**, en radios. Con 0,8 y la cuerda
/// de un diametro el arco barre unos 155 grados y entre dos festones queda
/// un pico de unos 25: la nube clasica. Con 1 serian semicirculos tangentes,
/// sin pico, y con menos la nube se aplana hasta parecer una ola.
pub const PANZA: f32 = 0.8;

/// La figura sobre la que va la nube.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    Rectangulo,
    Elipse,
}

impl Base {
    /// La palabra de `nubeBase` en el fichero.
    pub fn palabra(self) -> &'static str {
        match self {
            Base::Rectangulo => "rectangulo",
            Base::Elipse => "elipse",
        }
    }

    /// Y al reves. Sin palabra, o con una que no se conoce, rectangulo: es la
    /// nube de siempre.
    pub fn desde_palabra(p: Option<&str>) -> Base {
        match p {
            Some("elipse") => Base::Elipse,
            _ => Base::Rectangulo,
        }
    }

    /// La figura del elemento que la lleva.
    pub fn figura(self) -> Figura {
        match self {
            Base::Rectangulo => Figura::Rectangulo,
            Base::Elipse => Figura::Elipse,
        }
    }
}

/// Un feston: un arco de circunferencia de `desde` a `hasta`, con la panza
/// hacia fuera. Los angulos en radianes y en pantalla (la y baja); el
/// `barrido` lleva signo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Feston {
    pub desde: Punto2,
    pub hasta: Punto2,
    pub centro: Punto2,
    pub radio: f32,
    pub angulo: f32,
    pub barrido: f32,
}

impl Feston {
    /// El punto a `t` (de 0 a 1) del arco.
    pub fn punto(&self, t: f32) -> Punto2 {
        let a = self.angulo + self.barrido * t;
        Punto2::nuevo(
            self.centro.x + self.radio * a.cos(),
            self.centro.y + self.radio * a.sin(),
        )
    }

    /// Lo que mide la cuerda: el «diametro» real de este feston.
    pub fn cuerda(&self) -> f32 {
        self.desde.distancia(self.hasta)
    }

    /// Cuantos tramos rectos para que no se vean facetas: uno cada 2 px de
    /// arco, entre 4 y 32.
    fn pasos(&self) -> usize {
        ((self.barrido.abs() * self.radio / 2.0).ceil() as usize).clamp(4, 32)
    }

    /// El arco en dos cubicas (cada una de menos de 90 grados, que es donde
    /// la aproximacion de siempre no se aparta ni una milesima del radio).
    fn cubicas(&self) -> [(Punto2, Punto2, Punto2, Punto2); 2] {
        let medio = self.barrido / 2.0;
        let k = 4.0 / 3.0 * (medio / 4.0).tan() * self.radio;
        let tangente = |a: f32| Punto2::nuevo(-a.sin(), a.cos());
        let tramo = |a0: f32| {
            let a1 = a0 + medio;
            let p0 = Punto2::nuevo(
                self.centro.x + self.radio * a0.cos(),
                self.centro.y + self.radio * a0.sin(),
            );
            let p3 = Punto2::nuevo(
                self.centro.x + self.radio * a1.cos(),
                self.centro.y + self.radio * a1.sin(),
            );
            (
                p0,
                p0.proyectar(tangente(a0), k),
                p3.proyectar(tangente(a1), -k),
                p3,
            )
        };
        let mut t = [tramo(self.angulo), tramo(self.angulo + medio)];
        // Los extremos, los exactos: asi empalman sin costura con el feston
        // de al lado aunque el seno y el coseno redondeen distinto.
        t[0].0 = self.desde;
        t[1].3 = self.hasta;
        t
    }
}

/// El radio que se usa de verdad: lo que diga el elemento, dentro de lo
/// razonable. Un numero roto (NaN, infinito) no puede romper el dibujo: vale
/// el de fabrica.
pub fn radio_util(radio: f32) -> f32 {
    if radio.is_finite() && radio > 0.0 {
        radio.clamp(RADIO_MINIMO, RADIO_MAXIMO)
    } else {
        RADIO_DE_FABRICA
    }
}

/// Sobre que va la nube de `e`, si lleva: solo el rectangulo y la elipse.
pub fn base_de(e: &Elemento) -> Option<Base> {
    e.extras.nube?;
    match e.figura {
        Figura::Rectangulo => Some(Base::Rectangulo),
        Figura::Elipse => Some(Base::Elipse),
        _ => None,
    }
}

/// **Una nube nueva** de rectangulo, lista para el lienzo, sin rugosidad y
/// sin relleno. La usan la galeria (`figuras_de_serie::marcas`) y el
/// asistente (`pixpin-ia`, tipo «nube»).
pub fn nube(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    radio: f32,
    trazo: ColorRgba,
    grosor: f32,
) -> Elemento {
    Elemento {
        figura: Figura::Rectangulo,
        x,
        y,
        ancho,
        alto,
        trazo,
        grosor,
        rugosidad: 0.0,
        estilo_relleno: EstiloRelleno::Solido,
        extras: Extras {
            nube: Some(radio_util(radio)),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// **Los festones de una nube** con esa base, en esa caja (`x, y, ancho,
/// alto`, como la del elemento: puede venir con medidas negativas) y con ese
/// radio. En el sentido de las agujas del reloj en pantalla, empezando por la
/// esquina de arriba a la izquierda (o por arriba, en la elipse). Vacio si la
/// caja no mide nada.
///
/// En una caja pequena —menos de tres radios de lado— el feston se encoge
/// para que quepan: es lo unico que depende del tamano, y solo ahi.
pub fn festones(base: Base, (x, y, ancho, alto): (f32, f32, f32, f32), radio: f32) -> Vec<Feston> {
    let (x0, x1) = (x.min(x + ancho), x.max(x + ancho));
    let (y0, y1) = (y.min(y + alto), y.max(y + alto));
    let (w, h) = (x1 - x0, y1 - y0);
    if !(w.is_finite() && h.is_finite()) || w < 1.0 || h < 1.0 {
        return Vec::new();
    }
    let r = radio_util(radio).min(w.min(h) / 3.0);
    let panza = r * PANZA;
    let mut v = Vec::new();
    match base {
        Base::Rectangulo => {
            let p = Punto2::nuevo;
            let esquinas = [
                p(x0 + panza, y0 + panza),
                p(x1 - panza, y0 + panza),
                p(x1 - panza, y1 - panza),
                p(x0 + panza, y1 - panza),
            ];
            for i in 0..4 {
                let (a, b) = (esquinas[i], esquinas[(i + 1) % 4]);
                let n = cuantos(a.distancia(b), r, 1);
                for k in 0..n {
                    let desde = a.hacia(b, k as f32 / n as f32);
                    let hasta = if k + 1 == n {
                        b
                    } else {
                        a.hacia(b, (k + 1) as f32 / n as f32)
                    };
                    v.extend(feston(desde, hasta, panza));
                }
            }
        }
        Base::Elipse => {
            let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            let (rx, ry) = (w / 2.0 - panza, h / 2.0 - panza);
            // La elipse de la base muestreada fina, desde arriba y en el
            // sentido de las agujas del reloj, y su largo acumulado: los
            // festones van a distancias iguales por el perimetro, no a
            // angulos iguales (en una elipse alargada saldrian mas apretados
            // en las puntas).
            const MUESTRAS: usize = 720;
            let muestra: Vec<Punto2> = (0..=MUESTRAS)
                .map(|i| {
                    let t = -std::f32::consts::FRAC_PI_2
                        + std::f32::consts::TAU * i as f32 / MUESTRAS as f32;
                    Punto2::nuevo(c.x + rx * t.cos(), c.y + ry * t.sin())
                })
                .collect();
            let mut acumulado = vec![0.0f32];
            for par in muestra.windows(2) {
                let ultimo = *acumulado.last().unwrap_or(&0.0);
                acumulado.push(ultimo + par[0].distancia(par[1]));
            }
            let total = *acumulado.last().unwrap_or(&0.0);
            let n = cuantos(total, r, 3);
            let en = |s: f32| -> Punto2 {
                let i = acumulado.partition_point(|&a| a < s).clamp(1, MUESTRAS);
                let (a0, a1) = (acumulado[i - 1], acumulado[i]);
                let t = if a1 > a0 { (s - a0) / (a1 - a0) } else { 0.0 };
                muestra[i - 1].hacia(muestra[i], t)
            };
            let marcas: Vec<Punto2> = (0..n).map(|k| en(total * k as f32 / n as f32)).collect();
            for k in 0..n {
                v.extend(feston(marcas[k], marcas[(k + 1) % n], panza));
            }
        }
    }
    v
}

/// Cuantos festones caben en `largo`: largo entre diametro, redondeado, y al
/// menos `minimo`.
fn cuantos(largo: f32, radio: f32, minimo: usize) -> usize {
    ((largo / (2.0 * radio)).round() as usize).clamp(minimo, 100_000)
}

/// El feston de `a` a `b` con la panza hacia fuera: a la izquierda del
/// sentido de la marcha, que en un contorno que va como las agujas del reloj
/// en pantalla es fuera. La panza no pasa de media cuerda: asi el arco nunca
/// pasa de media vuelta y no se sale por los lados de su cuerda.
fn feston(a: Punto2, b: Punto2, panza: f32) -> Option<Feston> {
    let d = b.restar(a);
    let cuerda = d.longitud();
    if cuerda < 1e-3 {
        return None;
    }
    let fuera = Punto2::nuevo(d.y / cuerda, -d.x / cuerda);
    let s = panza.min(cuerda / 2.0).max(1e-3);
    let radio = (cuerda * cuerda / 4.0 + s * s) / (2.0 * s);
    let centro = a.hacia(b, 0.5).proyectar(fuera, s - radio);
    let angulo = (a.y - centro.y).atan2(a.x - centro.x);
    let apice = fuera.y.atan2(fuera.x);
    let mitad = (cuerda / 2.0 / radio).clamp(-1.0, 1.0).asin();
    // Hacia que lado se barre: el que pasa por la punta del feston.
    let mut giro = apice - angulo;
    while giro > std::f32::consts::PI {
        giro -= std::f32::consts::TAU;
    }
    while giro < -std::f32::consts::PI {
        giro += std::f32::consts::TAU;
    }
    let signo = if giro >= 0.0 { 1.0 } else { -1.0 };
    Some(Feston {
        desde: a,
        hasta: b,
        centro,
        radio,
        angulo,
        barrido: 2.0 * mitad * signo,
    })
}

/// **El contorno liso y cerrado de unos festones** (el ultimo punto es el
/// primero): el del relleno, el del trazo sin rugosidad, el de la linea que
/// viaja al movil y el que se toca con el raton.
pub fn contorno_de_festones(festones: &[Feston]) -> Vec<Punto2> {
    let mut salida = Vec::with_capacity(festones.len() * 16 + 1);
    for f in festones {
        let n = f.pasos();
        salida.push(f.desde);
        for i in 1..n {
            salida.push(f.punto(i as f32 / n as f32));
        }
    }
    if let Some(p) = salida.first().copied() {
        salida.push(p);
    }
    salida
}

/// El contorno de una nube con esa base, caja y radio. Una caja que no mide
/// nada da su caja: algo hay que ensenar.
pub fn contorno(base: Base, caja: (f32, f32, f32, f32), radio: f32) -> Vec<Punto2> {
    let f = festones(base, caja, radio);
    if f.is_empty() {
        let (x, y, w, h) = caja;
        return vec![
            Punto2::nuevo(x, y),
            Punto2::nuevo(x + w, y),
            Punto2::nuevo(x + w, y + h),
            Punto2::nuevo(x, y + h),
            Punto2::nuevo(x, y),
        ];
    }
    contorno_de_festones(&f)
}

/// El contorno de la nube de `e` (sin girar: el giro lo pone quien pinta),
/// o `None` si no es una nube.
pub fn contorno_de(e: &Elemento) -> Option<Vec<Punto2>> {
    let base = base_de(e)?;
    Some(contorno(
        base,
        (e.x, e.y, e.ancho, e.alto),
        e.extras.nube.unwrap_or(RADIO_DE_FABRICA),
    ))
}

/// **El trazo de la nube**: sin rugosidad, el contorno liso en una pasada;
/// con ella, cada feston con las cubicas de rough.js (`cubica_a`, con los
/// extremos quietos para que cierren), y las de una pasada encadenadas en una
/// sola raya —asi el discontinuo corre seguido por toda la nube en vez de
/// empezar de nuevo en cada feston—.
pub fn pasadas_a_mano(
    festones: &[Feston],
    o: rough::Opciones,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    let Some(primero) = festones.first() else {
        return Vec::new();
    };
    if o.rugosidad <= 0.0 {
        return vec![contorno_de_festones(festones)];
    }
    let o = rough::Opciones {
        preservar: true,
        ..o
    };
    let vueltas = if o.una_pasada { 1 } else { 2 };
    let pt = |p: Punto2| rough::Pt::nuevo(p.x as f64, p.y as f64);
    let mut r = rough::Rough::con_azar(o, azar.clone());
    let mut pasadas: Vec<Vec<rough::Op>> = vec![vec![rough::Op::Mover(pt(primero.desde))]; vueltas];
    for f in festones {
        for (p0, c1, c2, p3) in f.cubicas() {
            let ops = r.cubica_a(pt(p0), pt(c1), pt(c2), pt(p3));
            let cubicas = ops
                .into_iter()
                .filter(|op| matches!(op, rough::Op::Cubica(..)));
            for (pasada, op) in pasadas.iter_mut().zip(cubicas) {
                pasada.push(op);
            }
        }
    }
    *azar = r.azar();
    pasadas
        .iter()
        .flat_map(|ops| rough::a_pasadas(ops))
        .collect()
}

/// **Los trozos de la raya `a-b` que caen dentro del poligono** (par/impar).
/// Es el recorte del rayado de una nube, que no es convexa: con el recorte
/// del rombo las rayas cruzarian los picos entre festones.
pub fn recortar_a_poligono(a: Punto2, b: Punto2, poligono: &[Punto2]) -> Vec<(Punto2, Punto2)> {
    if poligono.len() < 3 {
        return Vec::new();
    }
    let d = b.restar(a);
    let mut cortes = vec![0.0f32, 1.0];
    let n = poligono.len();
    for i in 0..n {
        let (p, q) = (poligono[i], poligono[(i + 1) % n]);
        let e = q.restar(p);
        let den = d.x * e.y - d.y * e.x;
        if den.abs() < 1e-9 {
            continue;
        }
        let ap = p.restar(a);
        let t = (ap.x * e.y - ap.y * e.x) / den;
        let u = (ap.x * d.y - ap.y * d.x) / den;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            cortes.push(t);
        }
    }
    cortes.sort_by(|x, y| x.total_cmp(y));
    cortes.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
    cortes
        .windows(2)
        .filter(|w| crate::lazo::dentro_del_poligono(poligono, a.hacia(b, (w[0] + w[1]) / 2.0)))
        .map(|w| (a.hacia(b, w[0]), a.hacia(b, w[1])))
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const ROJO: ColorRgba = ColorRgba::opaco(0.8, 0.2, 0.2);

    fn caja_de(puntos: &[Punto2]) -> (f32, f32, f32, f32) {
        puntos.iter().fold(
            (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
            |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
        )
    }

    #[test]
    fn al_doblar_la_caja_el_feston_no_cambia_y_caben_el_doble() {
        let pequena = festones(Base::Rectangulo, (0.0, 0.0, 300.0, 120.0), 12.0);
        let grande = festones(Base::Rectangulo, (0.0, 0.0, 600.0, 240.0), 12.0);
        let (n1, n2) = (pequena.len() as f32, grande.len() as f32);
        assert!(
            (n2 / n1 - 2.0).abs() < 0.15,
            "{n1} festones y luego {n2}: tendrian que ser el doble"
        );
        // El feston es del mismo tamano en las dos: la cuerda, cerca del
        // diametro (lo que da el redondeo) y la panza, la misma.
        for f in pequena.iter().chain(&grande) {
            let c = f.cuerda();
            assert!((19.0..=29.0).contains(&c), "una cuerda de {c}");
        }
        let panza = |f: &Feston| {
            let fuera = Punto2::nuevo(
                (f.hasta.y - f.desde.y) / f.cuerda(),
                -(f.hasta.x - f.desde.x) / f.cuerda(),
            );
            f.punto(0.5)
                .restar(f.desde.hacia(f.hasta, 0.5))
                .producto(fuera)
        };
        for f in pequena.iter().chain(&grande) {
            assert!((panza(f) - 12.0 * PANZA).abs() < 0.01, "panza {}", panza(f));
        }
    }

    #[test]
    fn estirar_la_caja_no_deforma_los_festones_y_una_pieza_fija_si() {
        // Caso negativo: lo de antes, una pieza fija escalada, deja la
        // cuerda del feston 4 veces mas larga en horizontal que en vertical.
        // La dinamica la deja igual en los dos sentidos.
        let f = festones(Base::Rectangulo, (0.0, 0.0, 1000.0, 200.0), 12.0);
        let horizontal: Vec<f32> = f
            .iter()
            .filter(|f| (f.desde.y - f.hasta.y).abs() < 1e-3)
            .map(Feston::cuerda)
            .collect();
        let vertical: Vec<f32> = f
            .iter()
            .filter(|f| (f.desde.x - f.hasta.x).abs() < 1e-3)
            .map(Feston::cuerda)
            .collect();
        let media = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        assert!((media(&horizontal) / media(&vertical) - 1.0).abs() < 0.15);
        let fija = festones(Base::Rectangulo, (0.0, 0.0, 250.0, 200.0), 12.0);
        let estirada_h = fija
            .iter()
            .filter(|f| (f.desde.y - f.hasta.y).abs() < 1e-3)
            .map(|f| f.cuerda() * 4.0)
            .collect::<Vec<_>>();
        assert!(media(&estirada_h) / media(&vertical) > 3.0);
    }

    #[test]
    fn el_contorno_cierra_cabe_en_su_caja_y_las_esquinas_empalman() {
        for (w, h) in [
            (80.0, 40.0),
            (300.0, 120.0),
            (600.0, 600.0),
            (1000.0, 200.0),
        ] {
            for base in [Base::Rectangulo, Base::Elipse] {
                let f = festones(base, (10.0, 20.0, w, h), 12.0);
                assert!(!f.is_empty());
                // Cada feston empieza donde acaba el anterior, tambien en las
                // esquinas y al dar la vuelta.
                for i in 0..f.len() {
                    let (a, b) = (f[i], f[(i + 1) % f.len()]);
                    assert!(
                        a.hasta.distancia(b.desde) < 1e-3,
                        "{base:?} {w}x{h}: hueco en {i}"
                    );
                    assert!(a.punto(1.0).distancia(a.hasta) < 1e-2);
                }
                let c = contorno_de_festones(&f);
                assert_eq!(c.first(), c.last(), "cerrado");
                let (x0, y0, x1, y1) = caja_de(&c);
                assert!(x0 >= 10.0 - 0.01 && y0 >= 20.0 - 0.01, "{base:?} {w}x{h}");
                assert!(
                    x1 <= 10.0 + w + 0.01 && y1 <= 20.0 + h + 0.01,
                    "{base:?} {w}x{h}"
                );
                // El rectangulo llega a los cuatro bordes: lo que se ve es lo
                // que se elige.
                if base == Base::Rectangulo {
                    assert!((x0 - 10.0).abs() < 0.05 && (y0 - 20.0).abs() < 0.05);
                    assert!((x1 - 10.0 - w).abs() < 0.05 && (y1 - 20.0 - h).abs() < 0.05);
                }
            }
        }
    }

    #[test]
    fn una_caja_vuelta_del_reves_da_la_misma_nube() {
        let a = contorno(Base::Rectangulo, (0.0, 0.0, 200.0, 100.0), 12.0);
        let b = contorno(Base::Rectangulo, (200.0, 100.0, -200.0, -100.0), 12.0);
        assert_eq!(a, b);
        assert!(festones(Base::Rectangulo, (0.0, 0.0, 0.0, 50.0), 12.0).is_empty());
        assert_eq!(
            contorno(Base::Rectangulo, (0.0, 0.0, 0.0, 50.0), 12.0).len(),
            5
        );
        // Un radio roto no rompe: vale el de fabrica.
        assert_eq!(radio_util(f32::NAN), RADIO_DE_FABRICA);
        assert_eq!(radio_util(-3.0), RADIO_DE_FABRICA);
    }

    #[test]
    fn el_rayado_se_recorta_a_la_nube_y_no_a_su_caja() {
        let c = contorno(Base::Rectangulo, (0.0, 0.0, 200.0, 100.0), 12.0);
        // Una raya a la altura de los picos de arriba: dentro de la caja va
        // entera, dentro de la nube sale a trozos (entra y sale de cada
        // feston).
        let trozos = recortar_a_poligono(Punto2::nuevo(-10.0, 3.0), Punto2::nuevo(210.0, 3.0), &c);
        assert!(trozos.len() > 3, "{}", trozos.len());
        for (a, b) in &trozos {
            assert!(crate::lazo::dentro_del_poligono(&c, a.hacia(*b, 0.5)));
        }
        // Por el medio, una sola.
        assert_eq!(
            recortar_a_poligono(Punto2::nuevo(-10.0, 50.0), Punto2::nuevo(210.0, 50.0), &c).len(),
            1
        );
    }

    #[test]
    fn a_mano_cierra_y_con_una_pasada_es_una_sola_raya() {
        let f = festones(Base::Rectangulo, (0.0, 0.0, 200.0, 100.0), 12.0);
        let mut azar = Azar::nuevo(7);
        let dos = pasadas_a_mano(&f, rough::Opciones::nuevas(1.0, true, false), &mut azar);
        assert_eq!(dos.len(), 2);
        let una = pasadas_a_mano(
            &f,
            rough::Opciones::nuevas(1.0, true, true),
            &mut Azar::nuevo(7),
        );
        assert_eq!(una.len(), 1);
        for p in dos.iter().chain(&una) {
            assert!(p[0].distancia(*p.last().unwrap()) < 1e-3, "cierra");
        }
        let lisa = pasadas_a_mano(&f, rough::Opciones::nuevas(0.0, true, false), &mut azar);
        assert_eq!(lisa, vec![contorno_de_festones(&f)]);
    }

    #[test]
    fn ida_y_vuelta_por_excalidraw_sigue_siendo_nube_con_el_mismo_radio() {
        use crate::excalidraw::{elementos_a_json, elementos_de_json};
        for base in [Base::Rectangulo, Base::Elipse] {
            let mut e = nube(40.0, 30.0, 300.0, 120.0, 12.0, ROJO, 2.5);
            e.figura = base.figura();
            e.relleno = Some(ColorRgba::opaco(1.0, 1.0, 0.0));
            e.angulo = 0.3;
            let json = elementos_a_json(std::slice::from_ref(&e));
            // El movil ve una linea cerrada con los festones ya puestos, sin
            // redondeo y dentro de la caja.
            let v = &json[0];
            assert_eq!(v["type"], "line");
            assert_eq!(v["nubeRadio"], 12.0);
            assert_eq!(v["nubeBase"], base.palabra());
            assert!(v["roundness"].is_null());
            let puntos = v["points"].as_array().unwrap();
            assert!(puntos.len() > 40, "{}", puntos.len());
            assert_eq!(puntos.first(), puntos.last(), "cerrada");
            for p in puntos {
                let (x, y) = (p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
                assert!((-0.01..=300.01).contains(&x) && (-0.01..=120.01).contains(&y));
            }
            // Y el PC la recupera parametrica: la misma caja y el mismo radio.
            let vuelta = elementos_de_json(&json);
            assert_eq!(vuelta.len(), 1);
            let r = &vuelta[0];
            assert_eq!(base_de(r), Some(base));
            assert_eq!(r.extras.nube, Some(12.0));
            assert_eq!((r.x, r.y, r.ancho, r.alto), (40.0, 30.0, 300.0, 120.0));
            assert_eq!(r.angulo, 0.3);
            assert_eq!(r.relleno, e.relleno);
        }
    }

    #[test]
    fn la_estirada_en_el_movil_vuelve_con_mas_festones_y_el_mismo_radio() {
        use crate::excalidraw::{elementos_a_json, elementos_de_json};
        let e = nube(0.0, 0.0, 300.0, 120.0, 12.0, ROJO, 2.5);
        let mut json = elementos_a_json(std::slice::from_ref(&e));
        // El movil no sabe de nubes: estira la linea (sus puntos y su caja).
        json[0]["width"] = serde_json::json!(600.0);
        let r = &elementos_de_json(&json)[0];
        assert_eq!(r.ancho, 600.0);
        assert_eq!(r.extras.nube, Some(12.0));
        let antes = festones(Base::Rectangulo, (0.0, 0.0, 300.0, 120.0), 12.0).len();
        let ahora = contorno_de(r)
            .map(|_| festones(Base::Rectangulo, (0.0, 0.0, 600.0, 120.0), 12.0).len());
        assert!(ahora.unwrap() > antes + 20);
    }

    #[test]
    fn una_linea_sin_radio_valido_sigue_siendo_una_linea() {
        use crate::excalidraw::elementos_de_json;
        let linea = |extra: serde_json::Value| {
            let mut v = serde_json::json!({
                "type": "line", "id": "a", "x": 0, "y": 0, "width": 10, "height": 10,
                "points": [[0, 0], [10, 10]],
            });
            if let (Some(m), serde_json::Value::Object(e)) = (v.as_object_mut(), extra) {
                m.extend(e);
            }
            elementos_de_json(&[v]).remove(0)
        };
        for extra in [
            serde_json::json!({}),
            serde_json::json!({"nubeRadio": -4}),
            serde_json::json!({"nubeRadio": "doce"}),
            serde_json::json!({"nubeRadio": null}),
        ] {
            let e = linea(extra.clone());
            assert!(matches!(e.figura, Figura::Linea { .. }), "{extra}");
            assert_eq!(e.extras.nube, None);
        }
        // Un rectangulo con la clave tambien es nube (lo que escriba otro).
        let r = elementos_de_json(&[serde_json::json!({
            "type": "rectangle", "id": "b", "x": 0, "y": 0, "width": 100, "height": 60,
            "nubeRadio": 8,
        })]);
        assert_eq!(base_de(&r[0]), Some(Base::Rectangulo));
    }

    #[test]
    fn por_el_lienzo_entero_vuelve_nube_y_si_deja_de_serlo_vuelve_rectangulo() {
        use crate::excalidraw::{Lienzo, a_escena, con_escena, escribir, leer};
        let mut escena = crate::Escena::nueva();
        escena.anadir(nube(0.0, 0.0, 200.0, 100.0, 10.0, ROJO, 2.0));
        let texto = escribir(&con_escena(&Lienzo::vacio(), &escena));
        let lienzo = leer(&texto).unwrap();
        let e = &lienzo.elementos()[0];
        assert_eq!(e.extras.nube, Some(10.0));
        // Se le quita la nube aqui: sale rectangulo, sin las claves.
        let mut escena = a_escena(&lienzo);
        let id = escena.elementos[0].id;
        if let Some(e) = escena.elementos.iter_mut().find(|e| e.id == id) {
            e.extras.nube = None;
            e.version += 1;
        }
        let texto = escribir(&con_escena(&lienzo, &escena));
        let v: serde_json::Value = serde_json::from_str(&texto).unwrap();
        let el = &v["elements"][0];
        assert_eq!(el["type"], "rectangle", "{el}");
        assert!(el.get("nubeRadio").is_none() && el.get("points").is_none());
    }

    #[test]
    fn se_pinta_con_relleno_giro_discontinuo_y_opacidad_como_las_demas() {
        use crate::elemento::EstiloTrazo;
        use crate::pintado::{Orden, ordenes};
        let mut e = nube(0.0, 0.0, 200.0, 100.0, 12.0, ROJO, 3.0);
        e.relleno = Some(ColorRgba::opaco(1.0, 1.0, 0.0));
        e.estilo = EstiloTrazo::Discontinuo;
        e.opacidad = 0.5;
        let o = ordenes(&e);
        let contorno = contorno_de(&e).unwrap();
        // Primero el relleno, por los festones (no por la caja).
        assert!(
            matches!(&o[0], Orden::Relleno { puntos, color } if *puntos == contorno && color.a == 0.5)
        );
        assert!(o[1..].iter().all(|o| matches!(o,
            Orden::Polilinea { estilo: EstiloTrazo::Discontinuo, grosor, color, .. }
                if *grosor == 3.0 && color.a == 0.5)));
        // Rayado: rayas recortadas a la nube.
        e.estilo_relleno = EstiloRelleno::Rayado;
        e.estilo = EstiloTrazo::Solido;
        let rayas = ordenes(&e).len();
        assert!(rayas > 10);
        // Girada: los mismos puntos, girados alrededor del centro.
        e.estilo_relleno = EstiloRelleno::Solido;
        e.angulo = std::f32::consts::FRAC_PI_2;
        let girada = ordenes(&e);
        let Orden::Relleno { puntos, .. } = &girada[0] else {
            panic!("sin relleno");
        };
        let centro = Punto2::nuevo(100.0, 50.0);
        assert!(puntos[0].distancia(contorno[0].girar(centro, e.angulo)) < 1e-3);
        // Y se toca por los festones: en el centro de una sin relleno, no.
        e.relleno = None;
        e.angulo = 0.0;
        assert!(crate::impacto::toca(&e, contorno[3]));
        assert!(!crate::impacto::toca(&e, Punto2::nuevo(100.0, 50.0)));
    }

    #[test]
    fn el_svg_lleva_los_festones_y_no_un_rectangulo() {
        let mut escena = crate::Escena::nueva();
        escena.anadir(nube(0.0, 0.0, 300.0, 120.0, 12.0, ROJO, 2.5));
        let hoja = &crate::exportar::hojas(&escena, crate::exportar::Alcance::Todo, &[], None)[0];
        let svg = crate::exportar_svg::svg(hoja, Default::default(), &|_| None);
        let puntos = festones(Base::Rectangulo, (0.0, 0.0, 300.0, 120.0), 12.0).len();
        assert!(puntos > 25);
        // Una raya larga con cientos de vertices, no cuatro esquinas.
        let mayor = svg
            .split('<')
            .map(|t| t.matches(' ').count() + t.matches('L').count())
            .max()
            .unwrap_or(0);
        assert!(mayor > 200, "{svg}");
    }

    #[test]
    fn la_nube_de_fabrica_es_un_rectangulo_con_su_radio() {
        let e = nube(0.0, 0.0, 100.0, 50.0, 9.0, ROJO, 2.0);
        assert_eq!(base_de(&e), Some(Base::Rectangulo));
        assert_eq!(e.extras.nube, Some(9.0));
        // Sin la marca, un rectangulo cualquiera; y una linea no lleva nube.
        let mut sin = e.clone();
        sin.extras.nube = None;
        assert_eq!(base_de(&sin), None);
        let mut linea = e.clone();
        linea.figura = Figura::Linea { puntos: Vec::new() };
        assert_eq!(base_de(&linea), None);
    }
}
