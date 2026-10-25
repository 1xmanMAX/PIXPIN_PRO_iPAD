//! **Las figuras de serie** (21-oct-2026): plantillas de pagina entera
//! (planificadores, blocs, diarios, storyboard) y simbolos sueltos de
//! storyboard, para la biblioteca de figuras del lienzo.
//!
//! El usuario las pidio viendo las de otras aplicaciones («crealas y
//! insertalas en las figuras para poder anadirlo a cualquier parte rapido»,
//! «agrupalas por categoria», «haz que luzcan bien, muy atractivo»). Aqui se
//! dibujan desde cero, en espanol, con piezas normales del lienzo (lineas,
//! rectangulos, elipses, flechas y textos) a tiralineas: se estampan, se
//! mueven de una vez y se editan pieza a pieza como cualquier dibujo, y
//! viajan al movil en `figuras.json` como las figuras propias.
//!
//! **La categoria va en el nombre** («Planificacion · Semanal»,
//! [`SEPARADOR`]): el movil conserva el nombre tal cual y no sabe de un campo
//! nuevo, y el menu del PC agrupa por lo que va antes del separador
//! ([`categoria_de`]). Asi una figura del usuario tambien puede ir en una
//! categoria con solo llamarla asi.
//!
//! Los ids son fijos (`serie-<clave>`, [`PREFIJO_ID`]): volver a instalarlas
//! sustituye las de serie y no toca las del usuario.

use crate::biblioteca::{self, FiguraGuardada};
use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use crate::formas::TipoPunta;
use crate::relleno::EstiloRelleno;
use crate::texto::{self, AlineacionTexto, EstiloDeTexto};
use crate::vector::Punto2;

mod arquitectura;
mod basicas;
mod blocs;
mod bpmn;
mod ciencias;
mod civil;
mod derecho;
mod diagramas;
mod electronica;
mod er;
mod flujo;
mod intervalo;
mod mapas;
mod marcas;
mod mobiliario;
mod negocio;
mod personas;
mod planta;
mod plantillas;
mod redes;
mod secciones;
mod storyboard;
mod tecnico;
mod uml;
mod vegetacion;

pub use intervalo::{
    Intervalo, MAX_HOJAS, alargar, clave_de, cual_es, cuenta_de, es_la_de_noche, guia_de,
    las_que_se_alargan, las_que_se_personalizan, mes_de_la_hoja, nombre_de, se_alarga,
    se_personaliza, secciones_de, tramo_de_serie,
};
pub use secciones::{AnchoSeccion, MAX_LINEAS, MAX_SECCIONES, Seccion, SeccionPedida, TipoSeccion};

/// Entre la categoria y el nombre.
pub const SEPARADOR: &str = " · ";
/// El principio del id de las figuras de serie.
pub const PREFIJO_ID: &str = "serie-";
/// Sube cuando cambia el catalogo: la app lo vuelve a instalar.
pub const VERSION: u32 = 7;

/// Las categorias, en el orden en que salen en el menu.
pub const CATEGORIAS: [&str; 35] = [
    "Planificación",
    "Académico",
    "Creativo",
    "Diario",
    "Blocs",
    "Storyboard",
    "Arquitectura",
    "Vegetación",
    "Mobiliario",
    "Escala humana",
    "Planta: estructura",
    "Planta: puertas y ventanas",
    "Planta: cocina y baño",
    "Planta: muebles",
    "Planta: exterior",
    "Personas",
    "Personas en planta",
    "Ingeniería civil",
    "Derecho",
    "Electrónica",
    "Diagrama de flujo",
    "Marcas y sellos",
    "UML",
    "BPMN",
    "Entidad-relación",
    "Redes y TI",
    "Mapas mentales y organigramas",
    "Estrategia y negocio",
    "Formas y llamadas",
    "Física",
    "Química",
    "Matemáticas",
    "Biología",
    "Medicina",
    "Mecánica",
];

/// **La categoria de una figura** por su nombre («Diario · Gratitud» →
/// «Diario»), o `None` si no lleva.
pub fn categoria_de(nombre: &str) -> Option<(&str, &str)> {
    let (c, n) = nombre.split_once(SEPARADOR)?;
    let (c, n) = (c.trim(), n.trim());
    (!c.is_empty() && !n.is_empty()).then_some((c, n))
}

/// Si es de serie (por su id).
pub fn es_de_serie(f: &FiguraGuardada) -> bool {
    f.id.starts_with(PREFIJO_ID)
}

/// **El catalogo entero**, ya normalizado al origen.
pub fn catalogo() -> Vec<FiguraGuardada> {
    let mut v = Vec::new();
    plantillas::todas(&mut v);
    blocs::todas(&mut v);
    storyboard::todas(&mut v);
    arquitectura::todas(&mut v);
    vegetacion::todas(&mut v);
    mobiliario::todas(&mut v);
    planta::todas(&mut v);
    personas::todas(&mut v);
    civil::todas(&mut v);
    derecho::todas(&mut v);
    ciencias::todas(&mut v);
    electronica::todas(&mut v);
    flujo::todas(&mut v);
    marcas::todas(&mut v);
    uml::todas(&mut v);
    bpmn::todas(&mut v);
    er::todas(&mut v);
    redes::todas(&mut v);
    mapas::todas(&mut v);
    negocio::todas(&mut v);
    basicas::todas(&mut v);
    v
}

/// **Pone el catalogo en una lista**: quita las de serie de antes y mete
/// las de ahora al final, sin tocar las del usuario (ni su orden). Si el
/// usuario tiene una figura con el mismo nombre que una de serie, gana la
/// suya y la de serie no entra.
pub fn instalar_en(lista: &mut Vec<FiguraGuardada>) {
    lista.retain(|f| !es_de_serie(f));
    let propias: Vec<String> = lista.iter().map(|f| f.nombre.to_lowercase()).collect();
    lista.extend(
        catalogo()
            .into_iter()
            .filter(|f| !propias.contains(&f.nombre.to_lowercase())),
    );
}

// ---------------------------------------------------------------------------
// El lapiz: lo que usan las plantillas y los simbolos
// ---------------------------------------------------------------------------

/// Un color por su `#rrggbb`.
pub(crate) const fn hex(c: u32) -> ColorRgba {
    ColorRgba::opaco(
        ((c >> 16) & 0xff) as f32 / 255.0,
        ((c >> 8) & 0xff) as f32 / 255.0,
        (c & 0xff) as f32 / 255.0,
    )
}

const TRANSPARENTE: ColorRgba = ColorRgba {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

pub(crate) const FRAUNCES: &str = "Fraunces";
pub(crate) const WORK_SANS: &str = "Work Sans";
pub(crate) const CAVEAT: &str = "Caveat";

/// Como va un texto.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Al {
    Izq,
    Centro,
    Der,
}

/// Junta las piezas de una figura.
#[derive(Default)]
pub(crate) struct Lapiz {
    pub v: Vec<Elemento>,
    /// La parte abierta (su nombre y desde que pieza), ver [`Lapiz::parte`].
    abierta: Option<(String, usize)>,
}

/// El principio del subgrupo de una parte con nombre («parte:Tareas»).
pub use crate::biblioteca::PREFIJO_PARTE;

fn base(trazo: ColorRgba, grosor: f32) -> Elemento {
    Elemento {
        trazo,
        grosor,
        rugosidad: 0.0,
        estilo_relleno: EstiloRelleno::Solido,
        ..Default::default()
    }
}

fn caja_de_puntos(p: &[Punto2]) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for q in p {
        x0 = x0.min(q.x);
        y0 = y0.min(q.y);
        x1 = x1.max(q.x);
        y1 = y1.max(q.y);
    }
    (x0, y0, x1, y1)
}

impl Lapiz {
    /// **Lo que se dibuje a continuacion es la parte `nombre`** (una
    /// seccion de la hoja, su calendario...): sus piezas llevan el
    /// subgrupo `parte:<nombre>` dentro del grupo de la figura, para que
    /// `bloques::leer` la tome como un bloque exacto y el asistente sepa
    /// que es cada cosa. Cierra la anterior; con `""` no abre otra.
    pub(crate) fn parte(&mut self, nombre: &str) {
        if let Some((n, desde)) = self.abierta.take() {
            let g = format!("{PREFIJO_PARTE}{n}");
            for e in &mut self.v[desde..] {
                e.grupos.push(g.clone());
            }
        }
        if !nombre.trim().is_empty() {
            self.abierta = Some((nombre.trim().to_string(), self.v.len()));
        }
    }

    pub fn linea(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, c: ColorRgba, g: f32) {
        self.poli(&[(x1, y1), (x2, y2)], c, g, None);
    }

    /// Una linea a trazos o a puntos.
    #[allow(clippy::too_many_arguments)]
    pub fn linea_con(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        c: ColorRgba,
        g: f32,
        estilo: EstiloTrazo,
    ) {
        self.linea(x1, y1, x2, y2, c, g);
        if let Some(e) = self.v.last_mut() {
            e.estilo = estilo;
        }
    }

    /// Una linea por varios puntos; cerrada y con `relleno`, una forma.
    pub fn poli(&mut self, pts: &[(f32, f32)], c: ColorRgba, g: f32, relleno: Option<ColorRgba>) {
        if pts.len() < 2 {
            return;
        }
        let puntos: Vec<Punto2> = pts.iter().map(|(x, y)| Punto2::nuevo(*x, *y)).collect();
        let (x0, y0, x1, y1) = caja_de_puntos(&puntos);
        let mut e = base(c, g);
        e.x = puntos[0].x;
        e.y = puntos[0].y;
        e.ancho = (x1 - x0).max(0.0);
        e.alto = (y1 - y0).max(0.0);
        e.relleno = relleno;
        e.figura = Figura::Linea { puntos };
        self.v.push(e);
    }

    /// Un arco de circunferencia como linea de muchos tramos (angulos en
    /// grados, 0 a la derecha y creciendo hacia abajo, como la pantalla).
    #[allow(clippy::too_many_arguments)]
    pub fn arco(&mut self, cx: f32, cy: f32, r: f32, desde: f32, hasta: f32, c: ColorRgba, g: f32) {
        self.arco_eliptico(cx, cy, r, r, desde, hasta, c, g);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn arco_eliptico(
        &mut self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        desde: f32,
        hasta: f32,
        c: ColorRgba,
        g: f32,
    ) {
        let tramos = (((hasta - desde).abs() / 7.5).ceil() as usize).max(2);
        let pts: Vec<(f32, f32)> = (0..=tramos)
            .map(|i| {
                let a = (desde + (hasta - desde) * i as f32 / tramos as f32).to_radians();
                (cx + rx * a.cos(), cy + ry * a.sin())
            })
            .collect();
        self.poli(&pts, c, g, None);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        trazo: Option<ColorRgba>,
        relleno: Option<ColorRgba>,
        g: f32,
        redondo: bool,
    ) {
        let mut e = base(trazo.unwrap_or(TRANSPARENTE), g);
        e.figura = Figura::Rectangulo;
        e.x = x;
        e.y = y;
        e.ancho = w;
        e.alto = h;
        e.relleno = relleno;
        e.redondo = redondo;
        self.v.push(e);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn elipse(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        trazo: Option<ColorRgba>,
        relleno: Option<ColorRgba>,
        g: f32,
    ) {
        let mut e = base(trazo.unwrap_or(TRANSPARENTE), g);
        e.figura = Figura::Elipse;
        e.x = x;
        e.y = y;
        e.ancho = w;
        e.alto = h;
        e.relleno = relleno;
        self.v.push(e);
    }

    /// Un circulo por su centro.
    pub fn circulo(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        trazo: Option<ColorRgba>,
        relleno: Option<ColorRgba>,
        g: f32,
    ) {
        self.elipse(cx - r, cy - r, 2.0 * r, 2.0 * r, trazo, relleno, g);
    }

    /// Una flecha por esos puntos, con punta al final (y al principio si
    /// `doble`).
    pub fn flecha(&mut self, pts: &[(f32, f32)], c: ColorRgba, g: f32, doble: bool) {
        self.poli(pts, c, g, None);
        if let Some(e) = self.v.last_mut()
            && let Figura::Linea { puntos } = std::mem::replace(&mut e.figura, Figura::Rectangulo)
        {
            e.figura = Figura::Flecha {
                puntos,
                punta_inicio: if doble {
                    TipoPunta::Flecha
                } else {
                    TipoPunta::Ninguna
                },
                punta_fin: TipoPunta::Flecha,
                codos: false,
            };
        }
    }

    /// Un texto con su caja medida. `(x, y)` es la esquina de arriba a la
    /// izquierda con `Al::Izq`, el centro de arriba con `Al::Centro` y la
    /// esquina de arriba a la derecha con `Al::Der`.
    #[allow(clippy::too_many_arguments)]
    pub fn texto(
        &mut self,
        x: f32,
        y: f32,
        s: &str,
        tam: f32,
        familia: &str,
        c: ColorRgba,
        negrita: bool,
        al: Al,
    ) {
        let estilo = EstiloDeTexto {
            negrita,
            ..Default::default()
        };
        let (w, h) = texto::medida(s, tam, familia, estilo);
        let x = match al {
            Al::Izq => x,
            Al::Centro => x - w / 2.0,
            Al::Der => x - w,
        };
        let mut e = base(c, 1.0);
        e.figura = Figura::Texto {
            texto: s.to_string(),
            tam,
            familia: familia.to_string(),
        };
        e.x = x;
        e.y = y;
        e.ancho = w;
        e.alto = h;
        e.extras.negrita = negrita;
        e.extras.alineacion = Some(match al {
            Al::Izq => AlineacionTexto::Izquierda,
            Al::Centro => AlineacionTexto::Centro,
            Al::Der => AlineacionTexto::Derecha,
        });
        self.v.push(e);
    }

    /// La figura, con su categoria en el nombre y normalizada al origen.
    pub fn figura(mut self, clave: &str, categoria: &str, nombre: &str) -> FiguraGuardada {
        self.parte("");
        FiguraGuardada {
            id: format!("{PREFIJO_ID}{clave}"),
            nombre: format!("{categoria}{SEPARADOR}{nombre}"),
            elementos: biblioteca::normalizar(&self.v),
        }
    }
}

/// El modo noche de las plantillas de pagina (aparte, para no tocar el
/// resto de la lista).
mod noche;
pub use noche::{SUFIJO_NOCHE, es_de_noche};

#[cfg(test)]
#[path = "figuras_de_serie/pruebas.rs"]
mod pruebas;

#[cfg(test)]
#[path = "figuras_de_serie/pruebas_secciones.rs"]
mod pruebas_secciones;
