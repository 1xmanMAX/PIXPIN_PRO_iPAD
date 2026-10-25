//! **Las plantillas de pagina entera**: planificacion, academico, creativo y
//! diario. Una hoja A4 (794 × 1123, o apaisada) con su banda de color, el
//! titulo en Fraunces y las secciones con su rotulo y su raya de acento.
//!
//! Todas salen de las mismas piezas ([`Pag`]: seccion, renglones, tabla,
//! casillas, caja...) con el color de su categoria ([`Tema`]): cambiar el
//! aire de todas es tocar aqui una vez.
//!
//! **Las que se personalizan** ([`PERSONALIZABLES`], 25-oct-2026): su hoja
//! va por bloques (cabecera, lo fijo y la zona de secciones) y lo de la
//! zona sale de una lista de [`Seccion`] (ver `secciones.rs`). Cada una
//! dice en su comentario que bloques tiene y que se puede cambiar; su
//! lista de fabrica (`secciones_<plantilla>`) dibuja lo de siempre.

use super::secciones::{
    AnchoSeccion::Lateral,
    AnchoSeccion::{Completo, Derecha, Izquierda, Mitad},
    Hueco, Propio, Seccion, SeccionPedida,
    TipoSeccion::{Casillas, Libre, Numerados, Propia, Recuadro, Renglones},
    Zona, colocar, normalizar,
};
use super::{Al, CAVEAT, FRAUNCES, Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, EstiloTrazo};

/// A4 vertical a 96 ppp.
pub(super) const W: f32 = 794.0;
pub(super) const H: f32 = 1123.0;
/// El margen.
pub(super) const M: f32 = 48.0;
/// Donde empieza lo de debajo del titulo.
const Y0: f32 = 128.0;

pub(super) const TINTA: ColorRgba = hex(0x22304a);
pub(super) const ROTULO: ColorRgba = hex(0x5d6b82);
pub(super) const RAYA: ColorRgba = hex(0xcfd7e3);
pub(super) const BORDE: ColorRgba = hex(0xb4c0d3);
pub(super) const BLANCO: ColorRgba = hex(0xffffff);

/// El color de una categoria: el acento (rotulos, banda, rayas fuertes) y
/// su version suave (cabeceras, fondos).
#[derive(Clone, Copy)]
pub(super) struct Tema {
    pub categoria: &'static str,
    pub acento: ColorRgba,
    pub suave: ColorRgba,
    pub titulo: &'static str,
}

pub(super) const PLANIFICACION: Tema = Tema {
    categoria: "Planificación",
    acento: hex(0x3f6db3),
    suave: hex(0xe8eff9),
    titulo: FRAUNCES,
};
pub(super) const ACADEMICO: Tema = Tema {
    categoria: "Académico",
    acento: hex(0x2c8a7f),
    suave: hex(0xe3f3f0),
    titulo: FRAUNCES,
};
pub(super) const CREATIVO: Tema = Tema {
    categoria: "Creativo",
    acento: hex(0xc0692b),
    suave: hex(0xfbeee3),
    titulo: FRAUNCES,
};
pub(super) const DIARIO: Tema = Tema {
    categoria: "Diario",
    acento: hex(0xbf5878),
    suave: hex(0xfbe9ef),
    titulo: CAVEAT,
};

const DIAS: [&str; 7] = ["LUN", "MAR", "MIÉ", "JUE", "VIE", "SÁB", "DOM"];
const LETRAS: [&str; 7] = ["L", "M", "X", "J", "V", "S", "D"];

/// Una hoja a medio hacer.
pub(super) struct Pag {
    pub l: Lapiz,
    pub t: Tema,
    pub w: f32,
    pub h: f32,
    /// Donde empieza cada campo de la derecha, en el orden en que se dieron.
    campos: Vec<f32>,
}

impl Pag {
    /// La hoja: papel blanco, banda de color arriba, el titulo y los campos
    /// de la derecha («Semana», «Mes»...), y la raya de acento debajo.
    pub fn nueva(t: Tema, titulo: &str, campos: &[&str]) -> Pag {
        Pag::de_tamano(t, W, H, titulo, campos)
    }

    pub fn apaisada(t: Tema, titulo: &str, campos: &[&str]) -> Pag {
        Pag::de_tamano(t, H, W, titulo, campos)
    }

    /// Vertical y, si lo de dentro no cabe en un A4, mas larga.
    pub fn alta(t: Tema, titulo: &str, campos: &[&str], h: f32) -> Pag {
        Pag::de_tamano(t, W, h.max(H), titulo, campos)
    }

    pub fn de_tamano(t: Tema, w: f32, h: f32, titulo: &str, campos: &[&str]) -> Pag {
        let mut p = Pag {
            l: Lapiz::default(),
            t,
            w,
            h,
            campos: vec![0.0; campos.len()],
        };
        p.l.rect(
            0.0,
            0.0,
            w,
            h,
            Some(hex(0xdde3ec)),
            Some(BLANCO),
            1.0,
            false,
        );
        if t.titulo == CAVEAT {
            p.adornos();
        }
        p.l.rect(0.0, 0.0, w, 10.0, None, Some(t.acento), 1.0, false);
        if t.titulo == CAVEAT {
            p.l.texto(M, 34.0, titulo, 50.0, CAVEAT, t.acento, true, Al::Izq);
        } else {
            p.l.texto(M, 42.0, titulo, 34.0, t.titulo, TINTA, true, Al::Izq);
        }
        let mut x = w - M;
        for (i, c) in campos.iter().enumerate().rev() {
            p.l.texto(
                x - CAMPO,
                62.0,
                &c.to_uppercase(),
                10.0,
                WORK_SANS,
                ROTULO,
                true,
                Al::Izq,
            );
            p.l.linea(x - CAMPO, 82.0, x, 82.0, BORDE, 1.0);
            p.campos[i] = x - CAMPO;
            x -= CAMPO + 22.0;
        }
        p.l.linea(M, 104.0, w - M, 104.0, t.acento, 2.0);
        p
    }

    /// Las manchas suaves de las hojas del diario, detras de todo.
    fn adornos(&mut self) {
        let s = self.t.suave;
        let (w, h) = (self.w, self.h);
        // Dentro del papel: una mancha que se sale agranda la figura.
        self.l.circulo(w - 130.0, 200.0, 84.0, None, Some(s), 1.0);
        self.l.circulo(w - 62.0, 300.0, 34.0, None, Some(s), 1.0);
        self.l.circulo(132.0, h - 132.0, 100.0, None, Some(s), 1.0);
        self.l.circulo(262.0, h - 56.0, 40.0, None, Some(s), 1.0);
    }

    /// El rotulo de una seccion y su raya de acento. Devuelve donde empieza
    /// lo de debajo.
    pub fn seccion(&mut self, x: f32, y: f32, w: f32, s: &str) -> f32 {
        self.l.texto(
            x,
            y,
            &s.to_uppercase(),
            11.0,
            WORK_SANS,
            self.t.acento,
            true,
            Al::Izq,
        );
        self.l
            .linea(x, y + 20.0, x + w, y + 20.0, self.t.acento, 1.25);
        y + 20.0
    }

    /// `n` renglones cada `paso`, empezando un paso por debajo de `y`.
    pub fn renglones(&mut self, x: f32, y: f32, w: f32, n: usize, paso: f32) -> f32 {
        for i in 1..=n {
            let yy = y + paso * i as f32;
            self.l.linea(x, yy, x + w, yy, RAYA, 1.0);
        }
        y + paso * n as f32
    }

    /// `01`, `02`... con su renglon, empezando en `desde`.
    pub fn numerados_desde(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        desde: u32,
        n: usize,
        paso: f32,
    ) -> f32 {
        for i in 0..n {
            let yy = y + paso * (i + 1) as f32;
            self.l.texto(
                x + 2.0,
                yy - 17.0,
                &numero(desde + i as u32),
                10.0,
                WORK_SANS,
                self.t.acento,
                true,
                Al::Izq,
            );
            self.l.linea(x + 30.0, yy, x + w, yy, RAYA, 1.0);
        }
        y + paso * n as f32
    }

    /// Lo ya sabido de un campo de la cabecera («Mes»: noviembre), escrito
    /// a la derecha de su raya.
    pub fn valor(&mut self, campo: usize, s: &str) {
        if let Some(&x) = self.campos.get(campo) {
            self.l
                .texto(x + CAMPO, 60.0, s, 13.0, WORK_SANS, TINTA, false, Al::Der);
        }
    }

    /// Casillas para marcar, cada una con su renglon (y su texto si lo hay).
    pub fn casillas(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        n: usize,
        paso: f32,
        textos: &[&str],
    ) -> f32 {
        for i in 1..=n {
            let yy = y + paso * i as f32;
            self.l.rect(
                x + 2.0,
                yy - 15.0,
                12.0,
                12.0,
                Some(BORDE),
                Some(BLANCO),
                1.25,
                true,
            );
            if let Some(s) = textos.get(i - 1) {
                self.l.texto(
                    x + 22.0,
                    yy - 17.0,
                    s,
                    11.0,
                    WORK_SANS,
                    ROTULO,
                    false,
                    Al::Izq,
                );
            }
            self.l.linea(x + 22.0, yy, x + w, yy, RAYA, 1.0);
        }
        y + paso * n as f32
    }

    /// Un recuadro redondeado con su rotulo dentro, arriba a la izquierda.
    pub fn caja(&mut self, x: f32, y: f32, w: f32, h: f32, s: &str) {
        self.l.rect(x, y, w, h, Some(BORDE), None, 1.25, true);
        if !s.is_empty() {
            self.l.texto(
                x + 12.0,
                y + 10.0,
                &s.to_uppercase(),
                10.0,
                WORK_SANS,
                ROTULO,
                true,
                Al::Izq,
            );
        }
    }

    /// Una caja con el fondo suave.
    pub fn caja_suave(&mut self, x: f32, y: f32, w: f32, h: f32, s: &str) {
        self.l.rect(x, y, w, h, None, Some(self.t.suave), 1.0, true);
        if !s.is_empty() {
            self.l.texto(
                x + 12.0,
                y + 10.0,
                &s.to_uppercase(),
                10.0,
                WORK_SANS,
                self.t.acento,
                true,
                Al::Izq,
            );
        }
    }

    /// **Una tabla**: cabecera con fondo suave, columnas por fraccion del
    /// ancho, filas alternas apenas sombreadas y el borde de fuera. Devuelve
    /// donde acaba.
    pub fn tabla(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        cols: &[(&str, f32)],
        filas: usize,
        fh: f32,
    ) -> f32 {
        let total: f32 = cols.iter().map(|(_, f)| f).sum();
        let cab = if cols.iter().any(|(s, _)| !s.is_empty()) {
            fh.max(26.0)
        } else {
            0.0
        };
        let alto = cab + fh * filas as f32;
        if cab > 0.0 {
            self.l
                .rect(x, y, w, cab, None, Some(self.t.suave), 1.0, false);
        }
        for f in (1..filas).step_by(2) {
            self.l.rect(
                x,
                y + cab + fh * f as f32,
                w,
                fh,
                None,
                Some(hex(0xf7f9fc)),
                1.0,
                false,
            );
        }
        let mut cx = x;
        for (i, (s, f)) in cols.iter().enumerate() {
            let cw = w * f / total;
            if !s.is_empty() {
                self.l.texto(
                    cx + cw / 2.0,
                    y + (cab - 14.0) / 2.0,
                    &s.to_uppercase(),
                    9.5,
                    WORK_SANS,
                    self.t.acento,
                    true,
                    Al::Centro,
                );
            }
            if i > 0 {
                self.l.linea(cx, y, cx, y + alto, RAYA, 1.0);
            }
            cx += cw;
        }
        for f in 0..filas {
            let yy = y + cab + fh * f as f32;
            if f > 0 || cab > 0.0 {
                self.l.linea(x, yy, x + w, yy, RAYA, 1.0);
            }
        }
        self.l.rect(x, y, w, alto, Some(BORDE), None, 1.25, false);
        y + alto
    }

    /// Una lista de «Etiqueta: ____» (datos del vuelo, del curso...).
    pub fn datos(&mut self, x: f32, y: f32, w: f32, etiquetas: &[&str], paso: f32) -> f32 {
        for (i, s) in etiquetas.iter().enumerate() {
            let yy = y + paso * (i + 1) as f32;
            self.l.texto(
                x + 2.0,
                yy - 18.0,
                s,
                11.0,
                WORK_SANS,
                ROTULO,
                false,
                Al::Izq,
            );
            self.l.linea(x, yy, x + w, yy, RAYA, 1.0);
        }
        y + paso * etiquetas.len() as f32
    }

    /// Los circulitos de los dias (L M X J V S D), para marcar el de hoy.
    pub fn dias_en_circulos(&mut self, xd: f32, y: f32) {
        for (i, d) in LETRAS.iter().enumerate() {
            let cx = xd - (6 - i) as f32 * 22.0;
            self.l.circulo(cx, y, 9.0, Some(BORDE), None, 1.0);
            self.l
                .texto(cx, y - 6.5, d, 9.0, WORK_SANS, ROTULO, true, Al::Centro);
        }
    }

    pub fn fin(self, clave: &str, nombre: &str) -> FiguraGuardada {
        let cat = self.t.categoria;
        self.l.figura(clave, cat, nombre)
    }

    /// La hoja de un tramo: si es una de varias, con su pie («Hoja 2 de 4 ·
    /// 11–20») abajo a la derecha, en el margen.
    pub fn fin_de(mut self, clave: &str, nombre: &str, tr: &Tramo) -> FiguraGuardada {
        if let Some((i, n)) = tr.hoja {
            let s = format!("HOJA {i} DE {n}  ·  {}", tr.rotulo());
            let (w, h) = (self.w, self.h);
            self.l
                .texto(w - M, h - 32.0, &s, 9.0, WORK_SANS, ROTULO, true, Al::Der);
        }
        self.fin(clave, nombre)
    }
}

/// El ancho de un campo de la cabecera.
const CAMPO: f32 = 150.0;

/// Como se escribe un numero de una serie: con dos cifras como poco, para
/// que la columna no baile entre el 9 y el 10.
pub(super) fn numero(i: u32) -> String {
    format!("{i:02}")
}

/// **Que trozo de su serie lleva una hoja** (22-oct): el usuario elige una
/// plantilla y pide «del 1 al 40» (una hoja mas larga) o «de 10 en 10»
/// (cuatro hojas). La serie es lo que cuenta cada plantilla: filas del
/// cronograma, horas del diario, dias de los habitos...
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Tramo {
    pub desde: u32,
    pub hasta: u32,
    /// Las semanas del cronograma (su segunda serie).
    pub semanas: (u32, u32),
    /// `(esta, de cuantas)` si es una de varias hojas.
    pub hoja: Option<(u32, u32)>,
    /// `(ano, mes 1-12)`: el calendario de verdad (mensual y habitos).
    pub mes: Option<(i32, u32)>,
}

impl Tramo {
    pub const fn de(desde: u32, hasta: u32) -> Tramo {
        Tramo {
            desde,
            hasta,
            semanas: (1, 12),
            hoja: None,
            mes: None,
        }
    }

    pub fn n(&self) -> usize {
        (self.hasta.saturating_sub(self.desde) + 1) as usize
    }

    pub fn numeros(&self) -> std::ops::RangeInclusive<u32> {
        self.desde..=self.hasta
    }

    /// «11–20», o el mes.
    pub fn rotulo(&self) -> String {
        match self.mes {
            Some((a, m)) => format!("{} {a}", MESES[m as usize - 1]),
            None => format!("{}–{}", self.desde, self.hasta),
        }
    }
}

pub(super) const MESES: [&str; 12] = [
    "Enero",
    "Febrero",
    "Marzo",
    "Abril",
    "Mayo",
    "Junio",
    "Julio",
    "Agosto",
    "Septiembre",
    "Octubre",
    "Noviembre",
    "Diciembre",
];

/// Cuantos dias tiene un mes.
pub(super) fn dias_del_mes(a: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (a % 4 == 0 && a % 100 != 0) || a % 400 == 0 => 29,
        _ => 28,
    }
}

/// El dia de la semana, 0 el lunes (Sakamoto).
pub(super) fn dia_de_la_semana(a: i32, m: u32, d: u32) -> usize {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let a = if m < 3 { a - 1 } else { a };
    let domingo0 = (a + a / 4 - a / 100 + a / 400 + T[m as usize - 1] + d as i32).rem_euclid(7);
    ((domingo0 + 6) % 7) as usize
}

/// **Las plantillas que se alargan**: su clave, lo que cuenta su serie y su
/// tramo de serie (el del menu de figuras).
pub(super) struct Ampliable {
    pub clave: &'static str,
    pub cuenta: &'static str,
    pub defecto: (u32, u32),
    pub minimo: u32,
    pub maximo: u32,
}

pub(super) const AMPLIABLES: [Ampliable; 13] = [
    Ampliable {
        clave: "cronograma",
        cuenta: "filas",
        defecto: (1, 18),
        minimo: 1,
        maximo: 999,
    },
    Ampliable {
        clave: "planificador-diario",
        cuenta: "horas",
        defecto: (6, 22),
        minimo: 0,
        maximo: 24,
    },
    Ampliable {
        clave: "planificador-mensual",
        cuenta: "meses",
        defecto: (1, 1),
        minimo: 1,
        maximo: 1,
    },
    Ampliable {
        clave: "lista-de-tareas",
        cuenta: "tareas",
        defecto: (1, 42),
        minimo: 1,
        maximo: 999,
    },
    Ampliable {
        clave: "metas",
        cuenta: "pasos",
        defecto: (1, 12),
        minimo: 1,
        maximo: 999,
    },
    Ampliable {
        clave: "habitos",
        cuenta: "días",
        defecto: (1, 31),
        minimo: 1,
        maximo: 366,
    },
    Ampliable {
        clave: "itinerario",
        cuenta: "días",
        defecto: (1, 3),
        minimo: 1,
        maximo: 99,
    },
    Ampliable {
        clave: "lista-numerada",
        cuenta: "números",
        defecto: (1, 30),
        minimo: 0,
        maximo: 9999,
    },
    Ampliable {
        clave: "semestre",
        cuenta: "horas",
        defecto: (7, 22),
        minimo: 0,
        maximo: 24,
    },
    Ampliable {
        clave: "asistencia",
        cuenta: "alumnos",
        defecto: (1, 30),
        minimo: 1,
        maximo: 999,
    },
    Ampliable {
        clave: "storyboard-4",
        cuenta: "viñetas",
        defecto: (1, 4),
        minimo: 1,
        maximo: 999,
    },
    Ampliable {
        clave: "storyboard-6",
        cuenta: "viñetas",
        defecto: (1, 6),
        minimo: 1,
        maximo: 999,
    },
    Ampliable {
        clave: "receta",
        cuenta: "pasos",
        defecto: (1, 16),
        minimo: 1,
        maximo: 999,
    },
];

/// **Una plantilla que se alarga, con ese tramo.** `None` si la clave no es
/// de ninguna.
pub(super) fn hacer(clave: &str, tr: &Tramo) -> Option<FiguraGuardada> {
    hacer_con(clave, tr, None)
}

/// **Una plantilla con ese tramo y, si se dan, esas secciones** (las que se
/// personalizan; a las demas no les cambia nada). Tambien las que no se
/// alargan pero se personalizan (el semanal, el curso...): a esas el tramo
/// no les toca. `None` si la clave no es de ninguna de las dos tablas.
pub(super) fn hacer_con(
    clave: &str,
    tr: &Tramo,
    secs: Option<&[Seccion]>,
) -> Option<FiguraGuardada> {
    Some(match clave {
        "cronograma" => cronograma(tr, secs),
        "planificador-diario" => diario(tr, secs),
        "planificador-mensual" => mensual(tr, secs),
        "lista-de-tareas" => tareas(tr),
        "metas" => metas(tr, secs),
        "habitos" => habitos(tr),
        "itinerario" => itinerario(tr),
        "lista-numerada" => lista_numerada(tr),
        "semestre" => semestre(tr),
        "asistencia" => asistencia(tr),
        "storyboard-4" => storyboard(4, tr),
        "storyboard-6" => storyboard(6, tr),
        "receta" => receta(tr, secs),
        "planificador-semanal" => semanal(secs),
        "sesion-de-estudio" => estudio(secs),
        "curso" => curso(secs),
        "planificador-de-tareas" => entrega(secs),
        "viaje" => viaje(secs),
        "finanzas" => finanzas(secs),
        _ => return None,
    })
}

/// La de serie, con su tramo de siempre.
fn de_serie(clave: &str) -> FiguraGuardada {
    let a = AMPLIABLES
        .iter()
        .find(|a| a.clave == clave)
        .expect("es de la tabla");
    hacer(clave, &Tramo::de(a.defecto.0, a.defecto.1)).expect("es de la tabla")
}

/// **Las plantillas que se personalizan por secciones**: su clave, sus
/// bloques en palabras (para el modelo), su lista de fabrica, sus bloques
/// propios y, si una seccion cuenta la serie del tramo, cual.
pub(super) struct Personalizable {
    pub clave: &'static str,
    pub bloques: &'static str,
    pub fabrica: fn(&Tramo) -> Vec<Seccion>,
    pub propios: &'static [Propio],
    /// La seccion que lleva los numeros del tramo («Pasos» en las metas):
    /// si se pide un intervalo, sus lineas son las del tramo.
    pub serie: Option<&'static str>,
    /// Si tiene columna lateral (`ancho: lateral`).
    pub lateral: bool,
}

pub(super) const PERSONALIZABLES: [Personalizable; 11] = [
    Personalizable {
        clave: "planificador-mensual",
        bloques: "cabecera (título, Mes y Año) · calendario fijo de 7 columnas (va por meses con «mes») · debajo, la zona de secciones a dos columnas",
        fabrica: secciones_mensual,
        propios: &[],
        serie: None,
        lateral: false,
    },
    Personalizable {
        clave: "planificador-semanal",
        bloques: "cabecera (Semana, Mes) · horario fijo de los 7 días a la izquierda · a la derecha, la zona de secciones (una columna de 310 de ancho)",
        fabrica: secciones_semanal,
        propios: &PROPIOS_SEMANAL,
        serie: None,
        lateral: false,
    },
    Personalizable {
        clave: "planificador-diario",
        bloques: "cabecera (Fecha y los días L–D) · horario fijo a la izquierda (una franja por hora; las horas van con desde/hasta) · a la derecha, la zona de secciones (una columna)",
        fabrica: secciones_diario,
        propios: &[],
        serie: None,
        lateral: false,
    },
    Personalizable {
        clave: "metas",
        bloques: "cabecera (Fecha límite) · zona de secciones a dos columnas · pie fijo «Progreso» (10 casillas del 10 al 100 %). Los «Pasos» llevan los números del intervalo",
        fabrica: secciones_metas,
        propios: &PROPIOS_METAS,
        serie: Some("Pasos"),
        lateral: false,
    },
    Personalizable {
        clave: "sesion-de-estudio",
        bloques: "cabecera (Fecha) · zona de secciones a dos columnas, toda la hoja",
        fabrica: secciones_estudio,
        propios: &PROPIOS_ESTUDIO,
        serie: None,
        lateral: false,
    },
    Personalizable {
        clave: "curso",
        bloques: "cabecera (Ciclo) · zona de secciones a dos columnas, toda la hoja",
        fabrica: secciones_curso,
        propios: &PROPIOS_CURSO,
        serie: None,
        lateral: false,
    },
    Personalizable {
        clave: "planificador-de-tareas",
        bloques: "cabecera (Curso) · zona de secciones con la columna izquierda estrecha (300) y la derecha ancha (370)",
        fabrica: secciones_entrega,
        propios: &PROPIOS_ENTREGA,
        serie: None,
        lateral: false,
    },
    Personalizable {
        clave: "viaje",
        bloques: "cabecera (Destino, Fechas) · zona de secciones a dos columnas, toda la hoja",
        fabrica: secciones_viaje,
        propios: &PROPIOS_VIAJE,
        serie: None,
        lateral: false,
    },
    Personalizable {
        clave: "finanzas",
        bloques: "cabecera (Mes) · zona de secciones a dos columnas: tablas con su fila de TOTAL",
        fabrica: secciones_finanzas,
        propios: &PROPIOS_FINANZAS,
        serie: None,
        lateral: false,
    },
    Personalizable {
        clave: "receta",
        bloques: "cabecera (Categoría) · arriba, fijo: la foto a la izquierda y el nombre y los detalles a la derecha · debajo, la zona de secciones a dos columnas. La «Preparación» lleva los números del intervalo",
        fabrica: secciones_receta,
        propios: &[],
        serie: Some("Preparación"),
        lateral: false,
    },
    Personalizable {
        clave: "cronograma",
        bloques: "cabecera (Proyecto, Inicio) · tabla fija de actividades × semanas (filas con desde/hasta, semanas con semanas_desde/semanas_hasta) con su leyenda · de serie sin secciones: se pueden poner debajo de la tabla (completo/mitad) o en el lateral a su derecha (ancho «lateral»)",
        fabrica: secciones_cronograma,
        propios: &[],
        serie: None,
        lateral: true,
    },
];

/// La ficha de una que se personaliza.
pub(super) fn personalizable(clave: &str) -> Option<&'static Personalizable> {
    PERSONALIZABLES.iter().find(|p| p.clave == clave)
}

/// **La lista pedida, completa y comprobada** para esa plantilla.
pub(super) fn secciones_pedidas(
    clave: &str,
    nombre: &str,
    pedidas: &[SeccionPedida],
) -> Result<Vec<Seccion>, String> {
    let p = personalizable(clave)
        .ok_or_else(|| format!("«{nombre}» no tiene secciones que cambiar"))?;
    let fabrica = (p.fabrica)(&tramo_de_fabrica(clave));
    super::secciones::completar(nombre, &fabrica, p.propios, p.lateral, pedidas)
}

/// Con un intervalo pedido, la seccion de la serie lleva sus numeros.
pub(super) fn con_la_serie(clave: &str, secs: &[Seccion], tr: &Tramo) -> Vec<Seccion> {
    let serie = personalizable(clave).and_then(|p| p.serie);
    secs.iter()
        .cloned()
        .map(|mut s| {
            if serie.is_some_and(|t| normalizar(t) == normalizar(&s.titulo)) {
                s.lineas = tr.n() as u32;
            }
            s
        })
        .collect()
}

/// El tramo de la de serie (a las que no se alargan les da igual).
pub(super) fn tramo_de_fabrica(clave: &str) -> Tramo {
    AMPLIABLES
        .iter()
        .find(|a| a.clave == clave)
        .map_or(Tramo::de(1, 1), |a| Tramo::de(a.defecto.0, a.defecto.1))
}

/// La lista que se usa: la pedida o la de fabrica.
fn las_secciones(
    secs: Option<&[Seccion]>,
    fabrica: fn(&Tramo) -> Vec<Seccion>,
    tr: &Tramo,
) -> Vec<Seccion> {
    secs.map_or_else(|| fabrica(tr), <[Seccion]>::to_vec)
}

// ---------------------------------------------------------------------------

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let desde = v.len();
    v.extend([
        de_serie("cronograma"),
        de_serie("planificador-diario"),
        semanal(None),
        de_serie("planificador-mensual"),
        de_serie("lista-de-tareas"),
        de_serie("lista-numerada"),
        de_serie("metas"),
        de_serie("habitos"),
        comidas(),
        finanzas(None),
        de_serie("itinerario"),
        viaje(None),
        diario_de_viaje(),
        de_serie("semestre"),
        de_serie("asistencia"),
        curso(None),
        estudio(None),
        entrega(None),
        mapa_mental(),
        de_serie("storyboard-4"),
        de_serie("storyboard-6"),
        de_serie("receta"),
        pentagramas(),
        diario_libre(),
        autocuidado(),
        gratitud(),
        metas_del_dia(),
        sueno(),
    ]);
    // Y cada una, de noche (`noche.rs`): misma hoja, otros colores.
    let noches: Vec<FiguraGuardada> = v[desde..]
        .iter()
        .cloned()
        .map(super::noche::gemela)
        .collect();
    v.extend(noches);
}

const ANCHO: f32 = W - 2.0 * M;

// ------------------------------------------------------------- planificacion

/// **El cronograma** (el que le hizo falta al usuario para un boceto
/// preliminar): actividades, responsable y doce semanas en tres meses. Sus
/// bloques:
/// 1. Cabecera: «Proyecto» e «Inicio».
/// 2. Tabla (fija): una fila por actividad del tramo (`desde`/`hasta`) y
///    una columna por semana (`semanas`), con los meses encima y la leyenda
///    debajo.
/// 3. Zona de secciones (de serie, vacia): debajo de la leyenda, a lo
///    ancho de la tabla, y/o en el **lateral**, una columna de
///    [`LATERAL_DEL_CRONOGRAMA`] a la derecha de la tabla (la hoja se
///    ensancha lo justo). «Abajo una zona gris donde anotar» es una seccion
///    de tipo `Notas` a todo el ancho.
fn secciones_cronograma(_: &Tramo) -> Vec<Seccion> {
    Vec::new()
}

/// El ancho de la columna lateral del cronograma.
pub(super) const LATERAL_DEL_CRONOGRAMA: f32 = 300.0;

fn cronograma(tr: &Tramo, secs: Option<&[Seccion]>) -> FiguraGuardada {
    let secs = las_secciones(secs, secciones_cronograma, tr);
    let (s0, s1) = tr.semanas;
    let k = (s1.saturating_sub(s0) + 1) as usize;
    // Con pocas filas (una hoja de un tramo corto) se estiran hasta llenar el papel.
    let fh = ((W - 132.0 - 28.0 - 48.0 - M) / tr.n() as f32).clamp(28.0, 44.0);
    let y = 132.0;
    // Con mas de doce semanas la hoja se ensancha con columnas del mismo
    // ancho; con menos, las que hay se reparten el A4 apaisado.
    let fijo = 7.4;
    let unidad = (H - 2.0 * M) / (fijo + 0.9 * 12.0);
    let ancho = (unidad * (fijo + 0.9 * k as f32) + 2.0 * M).max(H);
    let (x, w) = (M, ancho - 2.0 * M);
    // Con lateral, la hoja se ensancha lo justo y la tabla no cambia.
    let hay_lateral = secs.iter().any(|s| s.ancho == Lateral);
    let ancho = if hay_lateral {
        ancho + 28.0 + LATERAL_DEL_CRONOGRAMA
    } else {
        ancho
    };
    // Las secciones de abajo empiezan bajo la leyenda (18 + 12) y su aire.
    let fin_tabla = y + fh + fh * tr.n() as f32;
    let z = Zona {
        lateral: Some((x + w + 28.0, y - 26.0, LATERAL_DEL_CRONOGRAMA)),
        fondo_lateral: fin_tabla,
        ..Zona::de_dos(x, fin_tabla + 18.0 + 12.0 + 30.0, w, 30.0, 24.0)
    };
    let m = colocar(&z, &secs);
    let alto = (fin_tabla + 48.0 + M).max(W).max(m.fin + M);
    let mut p = Pag::de_tamano(
        PLANIFICACION,
        ancho,
        alto,
        "Cronograma",
        &["Proyecto", "Inicio"],
    );
    // La tabla con sus meses y su leyenda: el bloque fijo.
    p.l.parte("Tabla");
    let fijas = [("#", 0.6), ("Actividad", 5.2), ("Resp.", 1.6)];
    let semanas: Vec<String> = (s0..=s1).map(|s| format!("S{s}")).collect();
    let mut cols: Vec<(&str, f32)> = fijas.to_vec();
    cols.extend(semanas.iter().map(|s| (s.as_str(), 0.9)));
    let total: f32 = cols.iter().map(|(_, f)| f).sum();
    let ancho_fijo = w * fijo / total;
    // Los meses encima de las semanas: cuatro semanas cada uno, contadas
    // desde la primera del proyecto (la 13 es del mes 4).
    let ancho_semana = w * 0.9 / total;
    let mut j = 0;
    while j < k {
        let mes = (s0 + j as u32 - 1) / 4;
        let cuantas = (j..k)
            .take_while(|&i| (s0 + i as u32 - 1) / 4 == mes)
            .count();
        let mx = x + ancho_fijo + ancho_semana * j as f32;
        let mw = ancho_semana * cuantas as f32;
        p.l.rect(
            mx + 1.0,
            y - 26.0,
            mw - 2.0,
            24.0,
            None,
            Some(p.t.acento),
            1.0,
            false,
        );
        let rotulo = if cuantas >= 2 {
            format!("MES {}", mes + 1)
        } else {
            format!("M{}", mes + 1)
        };
        p.l.texto(
            mx + mw / 2.0,
            y - 22.0,
            &rotulo,
            10.0,
            WORK_SANS,
            BLANCO,
            true,
            Al::Centro,
        );
        j += cuantas;
    }
    let fin = p.tabla(x, y, w, &cols, tr.n(), fh);
    for (f, i) in tr.numeros().enumerate() {
        let yy = y + fh + fh * f as f32;
        p.l.texto(
            x + w * 0.3 / total,
            yy + (fh - 14.0) / 2.0,
            &numero(i),
            10.0,
            WORK_SANS,
            ROTULO,
            false,
            Al::Centro,
        );
    }
    // La leyenda.
    let ly = fin + 18.0;
    p.l.rect(x, ly, 34.0, 12.0, None, Some(p.t.acento), 1.0, true);
    p.l.texto(
        x + 42.0,
        ly - 2.0,
        "Avance",
        11.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Izq,
    );
    p.l.rect(x + 120.0, ly, 34.0, 12.0, None, Some(p.t.suave), 1.0, true);
    p.l.texto(
        x + 162.0,
        ly - 2.0,
        "Planeado",
        11.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Izq,
    );
    let r = x + 260.0;
    p.l.poli(
        &[
            (r + 7.0, ly - 1.0),
            (r + 14.0, ly + 6.0),
            (r + 7.0, ly + 13.0),
            (r, ly + 6.0),
            (r + 7.0, ly - 1.0),
        ],
        hex(0xd9822b),
        1.5,
        Some(hex(0xf0a04b)),
    );
    p.l.texto(
        r + 22.0,
        ly - 2.0,
        "Hito",
        11.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Izq,
    );
    let fondo = p.h - M;
    p.poner_secciones(&z, &secs, m, fondo);
    p.fin_de("cronograma", "Cronograma", tr)
}

/// **El planificador diario.** Sus bloques, de arriba abajo y de izquierda
/// a derecha:
/// 1. Cabecera: «Fecha» y los circulitos L–D para marcar el dia.
/// 2. Horario (fijo, a la izquierda, 330 de ancho): una franja de 54 por
///    hora del tramo (`desde`/`hasta`; de 6 a 22 de serie).
/// 3. Zona de secciones (a la derecha, una sola columna): de fabrica la
///    meta del dia (recuadro), las prioridades, otras tareas y las notas
///    (libre: baja hasta el pie). Se pueden quitar, reordenar, cambiar de
///    lineas o anadir otras; «mitad» parte esa columna en dos.
///
/// Si lo de la derecha no cabe junto al horario, la hoja crece.
fn secciones_diario(_: &Tramo) -> Vec<Seccion> {
    vec![
        Seccion::nueva("Meta del día", Recuadro, 2, Completo),
        Seccion::nueva("Prioridades", Numerados, 3, Completo),
        Seccion::nueva("Otras tareas", Casillas, 10, Completo),
        Seccion::nueva("Notas", Libre, 4, Completo),
    ]
}

fn diario(tr: &Tramo, secs: Option<&[Seccion]>) -> FiguraGuardada {
    let secs = las_secciones(secs, secciones_diario, tr);
    let izq = 330.0;
    let x2 = M + izq + 30.0;
    let z = Zona::de_dos(x2, Y0, W - M - x2, 32.0, 26.0);
    let m = colocar(&z, &secs);
    let alto = (Y0 + 30.0 + 54.0 * tr.n() as f32 + M).max(m.fin + M);
    let mut p = Pag::alta(PLANIFICACION, "Planificador diario", &["Fecha"], alto);
    p.dias_en_circulos(W - M - 9.0 - 172.0, 72.0);
    // El horario: el bloque fijo de la izquierda.
    p.l.parte("Horario");
    let y = p.seccion(M, Y0, izq, "Horario");
    for (i, h) in tr.numeros().enumerate() {
        let yy = y + 10.0 + i as f32 * 54.0;
        p.l.texto(
            M,
            yy + 4.0,
            &format!("{h:02}"),
            11.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Izq,
        );
        p.l.linea(M + 30.0, yy + 18.0, M + izq, yy + 18.0, RAYA, 1.0);
        p.l.linea_con(
            M + 30.0,
            yy + 45.0,
            M + izq,
            yy + 45.0,
            RAYA,
            1.0,
            EstiloTrazo::Punteado,
        );
    }
    let fondo = p.h - M;
    p.poner_secciones(&z, &secs, m, fondo);
    p.fin_de("planificador-diario", "Planificador diario", tr)
}

/// **El planificador semanal.** Sus bloques:
/// 1. Cabecera: «Semana» y «Mes».
/// 2. Horario (fijo, a la izquierda, 360 de ancho): los siete dias, cada uno
///    con su pastilla y tres renglones, repartidos en todo el alto.
/// 3. Zona de secciones (a la derecha, una columna de 310): de fabrica el
///    objetivo de la semana (recuadro), las prioridades, otras tareas y la
///    rejilla de habitos (bloque propio: un renglon por habito y un circulo
///    por dia). Todo se quita, se reordena o cambia de lineas.
fn secciones_semanal(_: &Tramo) -> Vec<Seccion> {
    vec![
        Seccion::nueva("Objetivo de la semana", Recuadro, 3, Completo),
        Seccion::nueva("Prioridades", Numerados, 4, Completo),
        Seccion::nueva("Otras tareas", Casillas, 9, Completo),
        Seccion::nueva("Hábitos", Propia, 6, Completo),
    ]
}

const PROPIOS_SEMANAL: [Propio; 1] = [Propio {
    titulo: "Hábitos",
    que_es: "rejilla de hábitos: un renglón por hábito y un círculo por día (L a D); líneas = hábitos",
    alto: |n| 26.0 + 28.0 * n as f32,
    dibujar: habitos_de_la_semana,
    estira: false,
}];

/// La rejilla de habitos del semanal: las letras de los dias arriba a la
/// derecha y, por habito, su renglon y siete circulos.
fn habitos_de_la_semana(p: &mut Pag, h: Hueco, n: u32) {
    let celda = 22.0;
    let x0 = h.x + h.w - 7.0 * celda;
    for (i, d) in LETRAS.iter().enumerate() {
        p.l.texto(
            x0 + celda * i as f32 + celda / 2.0,
            h.y + 6.0,
            d,
            9.0,
            WORK_SANS,
            ROTULO,
            true,
            Al::Centro,
        );
    }
    for f in 0..n {
        let yy = h.y + 26.0 + f as f32 * 28.0;
        p.l.linea(h.x, yy + 20.0, x0 - 8.0, yy + 20.0, RAYA, 1.0);
        for i in 0..7 {
            p.l.circulo(
                x0 + celda * i as f32 + celda / 2.0,
                yy + 11.0,
                7.5,
                Some(BORDE),
                None,
                1.0,
            );
        }
    }
}

fn semanal(secs: Option<&[Seccion]>) -> FiguraGuardada {
    let secs = las_secciones(secs, secciones_semanal, &Tramo::de(1, 1));
    let izq = 360.0;
    let x2 = M + izq + 28.0;
    let z = Zona::de_dos(x2, Y0, W - M - x2, 30.0, 24.0).con_propios(&PROPIOS_SEMANAL);
    let m = colocar(&z, &secs);
    let mut p = Pag::alta(
        PLANIFICACION,
        "Planificador semanal",
        &["Semana", "Mes"],
        m.fin + M,
    );
    // El horario: el bloque fijo de la izquierda, en todo el alto.
    p.l.parte("Horario");
    let y = p.seccion(M, Y0, izq, "Horario");
    let alto = (p.h - M - y - 12.0) / 7.0;
    for (i, d) in DIAS.iter().enumerate() {
        let yy = y + 12.0 + alto * i as f32;
        p.l.rect(M, yy, 38.0, alto - 8.0, None, Some(p.t.suave), 1.0, true);
        p.l.texto(
            M + 19.0,
            yy + 8.0,
            d,
            10.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Centro,
        );
        p.l.rect(M, yy, izq, alto - 8.0, Some(BORDE), None, 1.25, true);
        p.renglones(M + 50.0, yy + 2.0, izq - 62.0, 3, (alto - 14.0) / 3.4);
    }
    let fondo = p.h - M;
    p.poner_secciones(&z, &secs, m, fondo);
    p.fin("planificador-semanal", "Planificador semanal")
}

/// **El planificador mensual**: en blanco, o con el mes de verdad (cada dia
/// en su dia de la semana, el mes y el ano escritos en la cabecera). Sus
/// bloques, de arriba abajo:
/// 1. Cabecera: «Mes» y «Año» (con mes, ya escritos).
/// 2. Calendario (fijo): la fila de los dias y seis semanas de casillas
///    (con mes, las que tenga, de 4 a 6, en el mismo alto). Va por meses
///    con `tr.mes`.
/// 3. Zona de secciones (debajo, a todo el ancho y a dos columnas): de
///    fabrica el objetivo del mes y las prioridades a la izquierda y las
///    tareas a la derecha. Se quitan, se reordenan, cambian de lineas, se
///    ponen a todo el ancho o se anaden otras («Objetivos del mes» a 6
///    lineas y a todo el ancho, «Tareas» a 4, sin «Prioridades»...).
///
/// Si las secciones no caben debajo del calendario, la hoja crece.
fn secciones_mensual(_: &Tramo) -> Vec<Seccion> {
    vec![
        Seccion::nueva("Objetivo del mes", Renglones, 2, Izquierda),
        Seccion::nueva("Prioridades", Numerados, 4, Izquierda),
        Seccion::nueva("Tareas", Casillas, 7, Derecha),
    ]
}

fn mensual(tr: &Tramo, secs: Option<&[Seccion]>) -> FiguraGuardada {
    let secs = las_secciones(secs, secciones_mensual, tr);
    let y = Y0;
    // El calendario: 28 de la fila de los dias, 8 de aire y seis filas de 96.
    let y1 = y + 36.0;
    let z = Zona::de_dos(M, y1 + 96.0 * 6.0 + 22.0, ANCHO, 30.0, 20.0);
    let m = colocar(&z, &secs);
    let mut p = Pag::alta(
        PLANIFICACION,
        "Planificador mensual",
        &["Mes", "Año"],
        m.fin + M,
    );
    // El calendario: un bloque fijo con nombre.
    p.l.parte("Calendario");
    let cw = ANCHO / 7.0;
    p.l.rect(M, y, ANCHO, 28.0, None, Some(p.t.acento), 1.0, true);
    for (i, d) in DIAS.iter().enumerate() {
        p.l.texto(
            M + cw * i as f32 + cw / 2.0,
            y + 7.0,
            d,
            10.0,
            WORK_SANS,
            BLANCO,
            true,
            Al::Centro,
        );
    }
    // Con mes: las semanas que tenga (4 a 6) se reparten el alto de seis.
    let (primero, dias) = match tr.mes {
        Some((a, m)) => {
            // Lo de la cabecera no es del calendario.
            p.l.parte("");
            p.valor(0, MESES[m as usize - 1]);
            p.valor(1, &a.to_string());
            p.l.parte("Calendario");
            (dia_de_la_semana(a, m, 1), dias_del_mes(a, m) as usize)
        }
        None => (0, 0),
    };
    let filas = if dias > 0 {
        (primero + dias).div_ceil(7)
    } else {
        6
    };
    let fh = 96.0 * 6.0 / filas as f32;
    for f in 0..filas {
        for c in 0..7 {
            let (cx, cy) = (M + cw * c as f32, y1 + fh * f as f32);
            let dia = (f * 7 + c + 1)
                .checked_sub(primero)
                .filter(|&d| d >= 1 && d <= dias);
            let fuera = dias > 0 && dia.is_none();
            let fondo = if fuera {
                Some(hex(0xf3f5f9))
            } else if c >= 5 {
                Some(hex(0xf6f8fc))
            } else {
                None
            };
            p.l.rect(
                cx + 2.0,
                cy + 2.0,
                cw - 4.0,
                fh - 4.0,
                Some(RAYA),
                fondo,
                1.0,
                true,
            );
            if fuera {
                continue;
            }
            match dia {
                Some(d) => {
                    p.l.circulo(cx + cw - 19.0, cy + 18.0, 10.0, None, Some(p.t.suave), 1.0);
                    p.l.texto(
                        cx + cw - 19.0,
                        cy + 11.0,
                        &d.to_string(),
                        10.0,
                        WORK_SANS,
                        p.t.acento,
                        true,
                        Al::Centro,
                    );
                }
                None => {
                    p.l.circulo(cx + cw - 19.0, cy + 18.0, 8.0, Some(RAYA), None, 1.0)
                }
            }
        }
    }
    let fondo = p.h - M;
    p.poner_secciones(&z, &secs, m, fondo);
    p.fin_de("planificador-mensual", "Planificador mensual", tr)
}

/// Las tareas van numeradas en dos columnas (la primera mitad a la
/// izquierda), cada una con su casilla y su fecha.
fn tareas(tr: &Tramo) -> FiguraGuardada {
    let filas = tr.n().div_ceil(2);
    let paso = ((H - Y0 - 20.0 - 150.0 - 20.0 - M) / filas as f32).clamp(34.0, 52.0);
    let alto = Y0 + 20.0 + 150.0 + 20.0 + paso * filas as f32 + M;
    let mut p = Pag::alta(PLANIFICACION, "Lista de tareas", &["Fecha"], alto);
    let y = p.seccion(M, Y0, ANCHO, "Lo más importante");
    p.caja_suave(M, y + 10.0, ANCHO, 116.0, "");
    // Lo mas importante va con un punto y no con numeros: los numeros son
    // los de la lista de abajo.
    for i in 1..=3 {
        let yy = y + 12.0 + 34.0 * i as f32;
        p.l.circulo(M + 24.0, yy - 9.0, 5.0, None, Some(p.t.acento), 1.0);
        p.l.linea(M + 40.0, yy, M + ANCHO - 14.0, yy, RAYA, 1.0);
    }
    let y = y + 150.0;
    let mitad = ANCHO / 2.0 - 14.0;
    let mut numeros = tr.numeros();
    for (k, x) in [M, M + mitad + 28.0].into_iter().enumerate() {
        let ys = p.seccion(
            x,
            y,
            mitad,
            if k == 0 { "Pendientes" } else { "Más tareas" },
        );
        for (f, i) in numeros.by_ref().take(filas).enumerate() {
            let yy = ys + paso * (f + 1) as f32;
            p.l.texto(
                x + 2.0,
                yy - 17.0,
                &numero(i),
                10.0,
                WORK_SANS,
                p.t.acento,
                true,
                Al::Izq,
            );
            p.l.rect(
                x + 32.0,
                yy - 17.0,
                14.0,
                14.0,
                Some(BORDE),
                None,
                1.25,
                true,
            );
            p.l.linea(x + 56.0, yy, x + mitad - 60.0, yy, RAYA, 1.0);
            p.l.texto(
                x + mitad - 26.0,
                yy - 17.0,
                "/",
                11.0,
                WORK_SANS,
                RAYA,
                false,
                Al::Centro,
            );
            p.l.linea(x + mitad - 50.0, yy, x + mitad, yy, RAYA, 1.0);
        }
    }
    p.fin_de("lista-de-tareas", "Lista de tareas", tr)
}

/// **La lista numerada** (22-oct): la mas sencilla, para cualquier cosa que
/// se cuente («del 1 al 50»): el numero en su pastilla, el renglon y una
/// casilla al final.
fn lista_numerada(tr: &Tramo) -> FiguraGuardada {
    let paso = ((H - Y0 - 20.0 - M) / tr.n() as f32).clamp(32.0, 80.0);
    let alto = Y0 + 20.0 + paso * tr.n() as f32 + M;
    let mut p = Pag::alta(PLANIFICACION, "Lista numerada", &["Tema", "Fecha"], alto);
    let y = Y0 + 8.0;
    for (f, i) in tr.numeros().enumerate() {
        let yy = y + paso * f as f32;
        if f % 2 == 1 {
            p.l.rect(M, yy, ANCHO, paso, None, Some(hex(0xf7f9fc)), 1.0, false);
        }
        p.l.rect(
            M + 4.0,
            yy + paso - 26.0,
            40.0,
            20.0,
            None,
            Some(p.t.suave),
            1.0,
            true,
        );
        p.l.texto(
            M + 24.0,
            yy + paso - 23.0,
            &numero(i),
            10.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Centro,
        );
        p.l.linea(
            M + 56.0,
            yy + paso - 8.0,
            M + ANCHO - 34.0,
            yy + paso - 8.0,
            RAYA,
            1.0,
        );
        p.l.rect(
            M + ANCHO - 20.0,
            yy + paso - 23.0,
            14.0,
            14.0,
            Some(BORDE),
            Some(BLANCO),
            1.25,
            true,
        );
    }
    p.fin_de("lista-numerada", "Lista numerada", tr)
}

/// **Las metas.** Sus bloques:
/// 1. Cabecera: «Fecha límite».
/// 2. Zona de secciones a dos columnas: de fabrica, a la izquierda la meta
///    (recuadro), el porque y la motivacion; a la derecha los pasos (con los
///    numeros del tramo: «del 1 al 30» da 30 pasos) y la tabla de hitos
///    (bloque propio: hito, fecha y ✓).
/// 3. Pie (fijo): «Progreso», diez casillas del 10 al 100 %, siempre abajo.
///
/// Si la zona no cabe, la hoja crece y el progreso baja con ella.
fn secciones_metas(tr: &Tramo) -> Vec<Seccion> {
    vec![
        Seccion::nueva("Mi meta", Recuadro, 6, Izquierda),
        Seccion::nueva("¿Por qué?", Renglones, 6, Izquierda),
        Seccion::nueva("Motivación", Renglones, 6, Izquierda),
        Seccion::nueva("Pasos", Numerados, tr.n() as u32, Derecha),
        Seccion::nueva("Hitos", Propia, 6, Derecha),
    ]
}

const PROPIOS_METAS: [Propio; 1] = [Propio {
    titulo: "Hitos",
    que_es: "tabla de hitos (hito, fecha y ✓); líneas = filas",
    alto: |n| 10.0 + 28.0 + 28.0 * n as f32,
    dibujar: |p, h, n| {
        p.tabla(
            h.x,
            h.y + 10.0,
            h.w,
            &[("Hito", 4.0), ("Fecha", 1.6), ("✓", 0.7)],
            n as usize,
            28.0,
        );
    },
    estira: false,
}];

/// Lo que ocupa el pie del progreso, con su aire.
const PIE_DE_METAS: f32 = 30.0 + 60.0;

fn metas(tr: &Tramo, secs: Option<&[Seccion]>) -> FiguraGuardada {
    let secs = las_secciones(secs, secciones_metas, tr);
    let z = Zona::de_dos(M, Y0, ANCHO, 30.0, 26.0)
        .con_propios(&PROPIOS_METAS)
        .con_serie("Pasos", tr.desde);
    let m = colocar(&z, &secs);
    let mut p = Pag::alta(
        PLANIFICACION,
        "Metas",
        &["Fecha límite"],
        m.fin + PIE_DE_METAS + M,
    );
    let fondo = p.h - M - PIE_DE_METAS;
    p.poner_secciones(&z, &secs, m, fondo);
    // El pie: el progreso, siempre abajo.
    p.l.parte("Progreso");
    let y = p.h - M - 60.0;
    let y = p.seccion(M, y, ANCHO, "Progreso");
    let paso = ANCHO / 10.0;
    for i in 0..10 {
        p.l.rect(
            M + paso * i as f32 + 2.0,
            y + 12.0,
            paso - 4.0,
            22.0,
            Some(BORDE),
            None,
            1.0,
            true,
        );
        p.l.texto(
            M + paso * (i as f32 + 0.5),
            y + 16.0,
            &format!("{}%", (i + 1) * 10),
            9.0,
            WORK_SANS,
            ROTULO,
            false,
            Al::Centro,
        );
    }
    p.fin_de("metas", "Metas", tr)
}

/// Los habitos: una columna por dia del tramo. Con mes, cada numero lleva
/// debajo la letra de su dia de la semana y el mes va en la cabecera.
fn habitos(tr: &Tramo) -> FiguraGuardada {
    let nombre = 168.0;
    let n = tr.n();
    // Hasta 31 dias se reparten el A4; con mas, la hoja se ensancha.
    let minima = (ANCHO - nombre) / 31.0;
    let ancho = (2.0 * M + nombre + minima * n as f32).max(W);
    let mut p = Pag::de_tamano(PLANIFICACION, ancho, H, "Hábitos del mes", &["Mes"]);
    if let Some((a, m)) = tr.mes {
        p.valor(0, &format!("{} {a}", MESES[m as usize - 1]));
    }
    let y = Y0 + 8.0;
    let celda = (ancho - 2.0 * M - nombre) / n as f32;
    let lado = (celda * 0.62).clamp(13.0, 20.0);
    let cab = if tr.mes.is_some() { 40.0 } else { 28.0 };
    p.l.texto(
        M,
        y + 4.0,
        "HÁBITO",
        10.0,
        WORK_SANS,
        p.t.acento,
        true,
        Al::Izq,
    );
    for (d, i) in tr.numeros().enumerate() {
        let cx = M + nombre + celda * (d as f32 + 0.5);
        p.l.texto(
            cx,
            y + 5.0,
            &i.to_string(),
            8.5,
            WORK_SANS,
            ROTULO,
            true,
            Al::Centro,
        );
        if let Some((a, m)) = tr.mes {
            let s = dia_de_la_semana(a, m, i);
            let color = if s >= 5 { p.t.acento } else { BORDE };
            p.l.texto(
                cx,
                y + 18.0,
                LETRAS[s],
                7.5,
                WORK_SANS,
                color,
                true,
                Al::Centro,
            );
        }
    }
    let fh = 34.0;
    let ancho_util = ancho - 2.0 * M;
    for f in 0..22 {
        let yy = y + cab + fh * f as f32;
        if f % 2 == 0 {
            p.l.rect(M, yy, ancho_util, fh, None, Some(p.t.suave), 1.0, true);
        }
        for d in 0..n {
            let cx = M + nombre + celda * (d as f32 + 0.5);
            p.l.rect(
                cx - lado / 2.0,
                yy + (fh - lado) / 2.0,
                lado,
                lado,
                Some(BORDE),
                Some(BLANCO),
                1.0,
                true,
            );
        }
    }
    let y = y + cab + fh * 22.0 + 24.0;
    let y = p.seccion(M, y, ancho_util, "Notas del mes");
    p.renglones(M, y, ancho_util, if tr.mes.is_some() { 2 } else { 3 }, 30.0);
    p.fin_de("habitos", "Hábitos del mes", tr)
}

fn comidas() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Comidas de la semana", &["Semana"]);
    let cols = [
        ("", 0.9),
        ("Desayuno", 2.0),
        ("Almuerzo", 2.0),
        ("Cena", 2.0),
        ("Snack", 1.4),
    ];
    let fh = 80.0;
    let fin = p.tabla(M, Y0, ANCHO, &cols, 7, fh);
    let total: f32 = cols.iter().map(|(_, f)| f).sum();
    for (i, d) in DIAS.iter().enumerate() {
        let yy = Y0 + 26.0 + fh * i as f32;
        p.l.rect(
            M,
            yy,
            ANCHO * 0.9 / total,
            fh,
            None,
            Some(p.t.suave),
            1.0,
            false,
        );
        p.l.texto(
            M + ANCHO * 0.45 / total,
            yy + fh / 2.0 - 8.0,
            d,
            11.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Centro,
        );
    }
    let y = fin + 26.0;
    let tercio = (ANCHO - 56.0) / 3.0;
    let ys = p.seccion(M, y, tercio * 2.0 + 28.0, "Lista de compras");
    p.casillas(M, ys, tercio, 7, 30.0, &[]);
    p.casillas(M + tercio + 28.0, ys, tercio, 7, 30.0, &[]);
    let x3 = M + 2.0 * (tercio + 28.0);
    let yn = p.seccion(x3, y, tercio, "Notas");
    p.caja(x3, yn + 10.0, tercio, H - M - yn - 10.0, "");
    p.fin("comidas", "Comidas de la semana")
}

/// **Las finanzas del mes.** Sus bloques:
/// 1. Cabecera: «Mes».
/// 2. Zona de secciones a dos columnas, todas bloques propios: los gastos
///    fijos y los variables a la izquierda; los ingresos, el ahorro y el
///    resumen a la derecha. Cada tabla lleva su fila de TOTAL; sus
///    `lineas` son sus filas. Se quitan, se reordenan, cambian de filas o
///    se ponen a todo el ancho, y se pueden anadir renglones o casillas.
fn secciones_finanzas(_: &Tramo) -> Vec<Seccion> {
    vec![
        Seccion::nueva("Gastos fijos", Propia, 10, Izquierda),
        Seccion::nueva("Gastos variables", Propia, 16, Izquierda),
        Seccion::nueva("Ingresos", Propia, 6, Derecha),
        Seccion::nueva("Ahorro", Propia, 4, Derecha),
        Seccion::nueva("Resumen", Propia, 5, Derecha),
    ]
}

/// Las columnas de los movimientos.
const MOVIMIENTOS: [(&str, f32); 3] = [("Fecha", 1.0), ("Concepto", 2.6), ("Monto", 1.3)];
/// Lo que mide una tabla de 27 con su cabecera y su total.
const fn alto_con_total(n: u32) -> f32 {
    10.0 + 27.0 + 27.0 * n as f32 + 28.0
}

const PROPIOS_FINANZAS: [Propio; 5] = [
    Propio {
        titulo: "Gastos fijos",
        que_es: "tabla fecha, concepto y monto con su TOTAL; líneas = filas",
        alto: alto_con_total,
        dibujar: |p, h, n| movimientos(p, h, &MOVIMIENTOS, n),
        estira: false,
    },
    Propio {
        titulo: "Gastos variables",
        que_es: "tabla fecha, concepto y monto con su TOTAL; líneas = filas",
        alto: alto_con_total,
        dibujar: |p, h, n| movimientos(p, h, &MOVIMIENTOS, n),
        estira: false,
    },
    Propio {
        titulo: "Ingresos",
        que_es: "tabla fecha, concepto y monto con su TOTAL; líneas = filas",
        alto: alto_con_total,
        dibujar: |p, h, n| movimientos(p, h, &MOVIMIENTOS, n),
        estira: false,
    },
    Propio {
        titulo: "Ahorro",
        que_es: "tabla para qué y monto con su TOTAL; líneas = filas",
        alto: alto_con_total,
        dibujar: |p, h, n| movimientos(p, h, &[("Para", 2.6), ("Monto", 1.3)], n),
        estira: false,
    },
    Propio {
        titulo: "Resumen",
        que_es: "tabla de ingresos, gastos fijos, gastos variables, ahorro y saldo; líneas = filas",
        alto: |n| 10.0 + 30.0 + 30.0 * n as f32,
        dibujar: resumen_de_finanzas,
        estira: false,
    },
];

/// Una tabla de movimientos y, debajo, su fila de TOTAL.
fn movimientos(p: &mut Pag, h: Hueco, cols: &[(&str, f32)], n: u32) {
    let y = p.tabla(h.x, h.y + 10.0, h.w, cols, n as usize, 27.0);
    p.l.rect(h.x, y, h.w, 28.0, None, Some(p.t.suave), 1.0, false);
    p.l.texto(
        h.x + 12.0,
        y + 7.0,
        "TOTAL",
        10.0,
        WORK_SANS,
        p.t.acento,
        true,
        Al::Izq,
    );
}

fn resumen_de_finanzas(p: &mut Pag, h: Hueco, n: u32) {
    let filas = [
        "Ingresos",
        "Gastos fijos",
        "Gastos variables",
        "Ahorro",
        "Saldo",
    ];
    let fin = p.tabla(
        h.x,
        h.y + 10.0,
        h.w,
        &[("", 2.6), ("Monto", 1.3)],
        n as usize,
        30.0,
    );
    for (i, s) in filas.iter().take(n as usize).enumerate() {
        let yy = fin - 30.0 * (n as usize - i) as f32;
        p.l.texto(
            h.x + 12.0,
            yy + 8.0,
            s,
            11.0,
            WORK_SANS,
            if i == 4 { p.t.acento } else { ROTULO },
            i == 4,
            Al::Izq,
        );
    }
}

fn finanzas(secs: Option<&[Seccion]>) -> FiguraGuardada {
    let secs = las_secciones(secs, secciones_finanzas, &Tramo::de(1, 1));
    let z = Zona::de_dos(M, Y0, ANCHO, 30.0, 24.0).con_propios(&PROPIOS_FINANZAS);
    let m = colocar(&z, &secs);
    let mut p = Pag::alta(PLANIFICACION, "Finanzas del mes", &["Mes"], m.fin + M);
    let fondo = p.h - M;
    p.poner_secciones(&z, &secs, m, fondo);
    p.fin("finanzas", "Finanzas del mes")
}

/// Un bloque por dia del tramo: «DIA 4», la raya para su fecha y su tabla.
fn itinerario(tr: &Tramo) -> FiguraGuardada {
    // Cada dia: rotulo 28, cabecera 30 y siete filas de 30, y 30 de aire.
    let bloque = 28.0 + 30.0 + 7.0 * 30.0 + 30.0;
    let alto = Y0 + bloque * tr.n() as f32 - 30.0 + M;
    let mut p = Pag::alta(PLANIFICACION, "Itinerario", &["Destino"], alto);
    let mut y = Y0;
    for d in tr.numeros() {
        let s = format!("DÍA {d}");
        p.l.texto(M, y, &s, 11.0, WORK_SANS, p.t.acento, true, Al::Izq);
        let tras =
            M + crate::texto::medida(
                &s,
                11.0,
                WORK_SANS,
                crate::texto::EstiloDeTexto {
                    negrita: true,
                    ..Default::default()
                },
            )
            .0 + 12.0;
        p.l.linea(tras, y + 16.0, tras + 170.0, y + 16.0, BORDE, 1.0);
        y = p.tabla(
            M,
            y + 28.0,
            ANCHO,
            &[("Hora", 1.0), ("Lugar", 2.4), ("Detalles", 3.2)],
            7,
            30.0,
        ) + 30.0;
    }
    p.fin_de("itinerario", "Itinerario", tr)
}

/// **El planificador de viaje.** Sus bloques:
/// 1. Cabecera: «Destino» y «Fechas».
/// 2. Zona de secciones a dos columnas: de fabrica la ficha del vuelo a la
///    izquierda y las del alojamiento y el transporte a la derecha (bloques
///    propios: etiqueta y hueco por fila); debajo, en una fila, el equipaje
///    y lo de antes de salir (casillas, de dos en dos).
fn secciones_viaje(_: &Tramo) -> Vec<Seccion> {
    vec![
        Seccion::nueva("Vuelo", Propia, 8, Izquierda),
        Seccion::nueva("Alojamiento", Propia, 4, Derecha),
        Seccion::nueva("Transporte", Propia, 3, Derecha),
        Seccion::nueva("Equipaje", Casillas, 16, Mitad),
        Seccion::nueva("Antes de salir", Casillas, 16, Mitad),
    ]
}

const VUELO: [&str; 8] = [
    "Aerolínea",
    "Reserva",
    "N.º de vuelo",
    "Salida",
    "Llegada",
    "N.º de vuelo",
    "Salida",
    "Llegada",
];
const ALOJAMIENTO: [&str; 4] = ["Nombre", "Dirección", "Fechas", "Entrada / salida"];
const TRANSPORTE: [&str; 3] = ["Empresa", "Reserva", "Recoger / devolver"];

const PROPIOS_VIAJE: [Propio; 3] = [
    Propio {
        titulo: "Vuelo",
        que_es: "ficha del vuelo (aerolínea, reserva, vuelos de ida y vuelta); líneas = filas",
        alto: |n| 10.0 + 30.0 * n as f32,
        dibujar: |p, h, n| ficha(p, h, &VUELO, n),
        estira: false,
    },
    Propio {
        titulo: "Alojamiento",
        que_es: "ficha del alojamiento (nombre, dirección, fechas, entrada y salida); líneas = filas",
        alto: |n| 10.0 + 30.0 * n as f32,
        dibujar: |p, h, n| ficha(p, h, &ALOJAMIENTO, n),
        estira: false,
    },
    Propio {
        titulo: "Transporte",
        que_es: "ficha del transporte (empresa, reserva, recoger y devolver); líneas = filas",
        alto: |n| 10.0 + 30.0 * n as f32,
        dibujar: |p, h, n| ficha(p, h, &TRANSPORTE, n),
        estira: false,
    },
];

/// Una ficha: por fila, su etiqueta sobre fondo suave y el hueco para
/// escribir. Con mas filas que etiquetas, las de mas van en blanco.
fn ficha(p: &mut Pag, h: Hueco, etiquetas: &[&str], n: u32) {
    let (x, y, w) = (h.x, h.y + 10.0, h.w);
    let fin = p.tabla(x, y, w, &[("", 1.4), ("", 2.2)], n as usize, 30.0);
    for i in 0..n as usize {
        let yy = y + 30.0 * i as f32;
        p.l.rect(
            x,
            yy,
            w * 1.4 / 3.6,
            30.0,
            None,
            Some(p.t.suave),
            1.0,
            false,
        );
        if let Some(s) = etiquetas.get(i) {
            p.l.texto(
                x + 10.0,
                yy + 9.0,
                &s.to_uppercase(),
                9.5,
                WORK_SANS,
                p.t.acento,
                true,
                Al::Izq,
            );
        }
    }
    p.l.rect(x, y, w, fin - y, Some(BORDE), None, 1.25, false);
}

fn viaje(secs: Option<&[Seccion]>) -> FiguraGuardada {
    let secs = las_secciones(secs, secciones_viaje, &Tramo::de(1, 1));
    let z = Zona::de_dos(M, Y0, ANCHO, 30.0, 24.0).con_propios(&PROPIOS_VIAJE);
    let m = colocar(&z, &secs);
    let mut p = Pag::alta(
        PLANIFICACION,
        "Planificador de viaje",
        &["Destino", "Fechas"],
        m.fin + M,
    );
    let fondo = p.h - M;
    p.poner_secciones(&z, &secs, m, fondo);
    p.fin("viaje", "Planificador de viaje")
}

fn diario_de_viaje() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Diario de viaje", &["Lugar", "Fechas"]);
    let alto_foto = 380.0;
    p.caja_suave(M, Y0, ANCHO, alto_foto, "Foto o dibujo");
    let y = Y0 + alto_foto + 20.0;
    let paso = 22.0;
    let (cols, filas) = ((ANCHO / paso) as usize, ((H - M - y) / paso) as usize);
    for c in 0..=cols {
        let x = M + paso * c as f32;
        p.l.linea(x, y, x, y + paso * filas as f32, hex(0xe1e7f0), 1.0);
    }
    for f in 0..=filas {
        let yy = y + paso * f as f32;
        p.l.linea(M, yy, M + paso * cols as f32, yy, hex(0xe1e7f0), 1.0);
    }
    p.fin("diario-de-viaje", "Diario de viaje")
}

// ----------------------------------------------------------------- academico

/// El horario: una fila por hora del tramo y, debajo, los cursos.
fn semestre(tr: &Tramo) -> FiguraGuardada {
    let fh = 38.0;
    let horas = tr.n();
    let alto = Y0 + 26.0 + fh * horas as f32 + 26.0 + 20.0 + 12.0 + 62.0 * 3.0 + M;
    let mut p = Pag::alta(ACADEMICO, "Horario del semestre", &["Inicio", "Fin"], alto);
    let mut cols = vec![("", 0.55)];
    cols.extend(DIAS.iter().map(|d| (*d, 1.0)));
    let fin = p.tabla(M, Y0, ANCHO, &cols, horas, fh);
    for (f, h) in tr.numeros().enumerate() {
        let yy = Y0 + 26.0 + fh * f as f32;
        p.l.texto(
            M + 18.0,
            yy + (fh - 14.0) / 2.0,
            &numero(h),
            10.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Centro,
        );
    }
    let y = p.seccion(M, fin + 26.0, ANCHO, "Mis cursos");
    let mitad = ANCHO / 2.0 - 12.0;
    for i in 0..6 {
        let (x, yy) = (
            M + (mitad + 24.0) * (i % 2) as f32,
            y + 12.0 + 62.0 * (i / 2) as f32,
        );
        p.l.rect(x, yy, mitad, 52.0, Some(BORDE), None, 1.25, true);
        p.l.rect(x, yy, 8.0, 52.0, None, Some(p.t.acento), 1.0, false);
        p.l.texto(
            x + 18.0,
            yy + 6.0,
            &format!("Curso {}:", i + 1),
            11.0,
            WORK_SANS,
            ROTULO,
            true,
            Al::Izq,
        );
        p.l.linea(x + 18.0, yy + 26.0, x + mitad - 12.0, yy + 26.0, RAYA, 1.0);
        p.l.texto(
            x + 18.0,
            yy + 30.0,
            "Aula:",
            10.0,
            WORK_SANS,
            ROTULO,
            false,
            Al::Izq,
        );
    }
    p.fin_de("semestre", "Horario del semestre", tr)
}

/// **La lista de asistencia** (22-oct): numero, nombre y doce columnas para
/// las fechas, con la clave de lo que se marca debajo.
fn asistencia(tr: &Tramo) -> FiguraGuardada {
    let fh = ((H - Y0 - 26.0 - 50.0 - M) / tr.n() as f32).clamp(26.0, 44.0);
    let alto = Y0 + 26.0 + fh * tr.n() as f32 + 50.0 + M;
    let mut p = Pag::alta(ACADEMICO, "Lista de asistencia", &["Curso", "Mes"], alto);
    let mut cols = vec![("N.º", 0.7), ("Nombre", 4.4)];
    cols.extend(std::iter::repeat_n(("", 0.62), 12));
    let total: f32 = cols.iter().map(|(_, f)| f).sum();
    let fin = p.tabla(M, Y0, ANCHO, &cols, tr.n(), fh);
    for (f, i) in tr.numeros().enumerate() {
        let yy = Y0 + 26.0 + fh * f as f32;
        p.l.texto(
            M + ANCHO * 0.35 / total,
            yy + (fh - 14.0) / 2.0,
            &numero(i),
            10.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Centro,
        );
    }
    let y = fin + 18.0;
    let mut x = M;
    for (marca, que) in [
        ("✓", "Presente"),
        ("F", "Falta"),
        ("T", "Tarde"),
        ("J", "Justificada"),
    ] {
        p.l.rect(x, y, 20.0, 20.0, Some(BORDE), Some(p.t.suave), 1.0, true);
        p.l.texto(
            x + 10.0,
            y + 3.0,
            marca,
            10.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Centro,
        );
        p.l.texto(
            x + 28.0,
            y + 3.0,
            que,
            10.5,
            WORK_SANS,
            ROTULO,
            false,
            Al::Izq,
        );
        x += 130.0;
    }
    p.fin_de("asistencia", "Lista de asistencia", tr)
}

/// **La ficha de un curso.** Sus bloques:
/// 1. Cabecera: «Ciclo».
/// 2. Zona de secciones a dos columnas: de fabrica, a la izquierda los
///    datos (curso, docente, horario...), los libros y las fechas
///    importantes; a la derecha la tabla de entregas y las notas (libre:
///    baja hasta el pie). Las tablas y los datos son bloques propios (sus
///    `lineas` son sus filas).
fn secciones_curso(_: &Tramo) -> Vec<Seccion> {
    vec![
        Seccion::nueva("Datos", Propia, 6, Izquierda),
        Seccion::nueva("Libros y materiales", Propia, 7, Izquierda),
        Seccion::nueva("Fechas importantes", Propia, 8, Izquierda),
        Seccion::nueva("Entregas", Propia, 18, Derecha),
        Seccion::nueva("Notas", Libre, 4, Derecha),
    ]
}

const DATOS_DEL_CURSO: [&str; 6] = [
    "Curso:",
    "Docente:",
    "Horario:",
    "Aula:",
    "Atención:",
    "Contacto:",
];

const PROPIOS_CURSO: [Propio; 4] = [
    Propio {
        titulo: "Datos",
        que_es: "curso, docente, horario, aula, atención y contacto, cada uno con su renglón; líneas = filas",
        alto: |n| 32.0 * n as f32,
        dibujar: |p, h, n| {
            p.datos(h.x, h.y, h.w, &rotulos(&DATOS_DEL_CURSO, n), 32.0);
        },
        estira: false,
    },
    Propio {
        titulo: "Libros y materiales",
        que_es: "tabla de dos columnas sin cabecera (qué y ✓); líneas = filas",
        alto: |n| 6.0 + 30.0 * n as f32,
        dibujar: |p, h, n| {
            p.tabla(
                h.x,
                h.y + 6.0,
                h.w,
                &[("", 5.0), ("", 1.0)],
                n as usize,
                30.0,
            );
        },
        estira: false,
    },
    Propio {
        titulo: "Fechas importantes",
        que_es: "tabla fecha y qué; líneas = filas",
        alto: |n| 6.0 + 30.0 + 30.0 * n as f32,
        dibujar: |p, h, n| {
            p.tabla(
                h.x,
                h.y + 6.0,
                h.w,
                &[("Fecha", 1.0), ("Qué", 3.0)],
                n as usize,
                30.0,
            );
        },
        estira: false,
    },
    Propio {
        titulo: "Entregas",
        que_es: "tabla fecha, trabajo y ✓; líneas = filas",
        alto: |n| 6.0 + 30.0 + 30.0 * n as f32,
        dibujar: |p, h, n| {
            p.tabla(
                h.x,
                h.y + 6.0,
                h.w,
                &[("Fecha", 1.0), ("Trabajo", 3.2), ("✓", 0.6)],
                n as usize,
                30.0,
            );
        },
        estira: false,
    },
];

/// Las `n` primeras etiquetas y, si se piden mas, en blanco.
fn rotulos<'a>(lista: &[&'a str], n: u32) -> Vec<&'a str> {
    (0..n as usize)
        .map(|i| lista.get(i).copied().unwrap_or(""))
        .collect()
}

fn curso(secs: Option<&[Seccion]>) -> FiguraGuardada {
    let secs = las_secciones(secs, secciones_curso, &Tramo::de(1, 1));
    let z = Zona::de_dos(M, Y0, ANCHO, 30.0, 24.0).con_propios(&PROPIOS_CURSO);
    let m = colocar(&z, &secs);
    let mut p = Pag::alta(ACADEMICO, "Curso", &["Ciclo"], m.fin + M);
    let fondo = p.h - M;
    p.poner_secciones(&z, &secs, m, fondo);
    p.fin("curso", "Curso")
}

/// **La sesion de estudio.** Sus bloques:
/// 1. Cabecera: «Fecha».
/// 2. Zona de secciones a dos columnas: de fabrica, a la izquierda el
///    tiempo (inicio, fin, total), los objetivos, los temas y las pausas; a
///    la derecha lo de antes de empezar (casillas ya escritas), los bloques
///    de tiempo (tabla de 15 en 15 minutos) y las notas con «Mi premio»
///    (baja hasta el pie). Los tres de la derecha y el tiempo son bloques
///    propios; sus `lineas` son sus filas.
fn secciones_estudio(_: &Tramo) -> Vec<Seccion> {
    vec![
        Seccion::nueva("Tiempo", Propia, 3, Izquierda),
        Seccion::nueva("Objetivos", Numerados, 4, Izquierda),
        Seccion::nueva("Temas / capítulos", Renglones, 10, Izquierda),
        Seccion::nueva("Pausas", Renglones, 4, Izquierda),
        Seccion::nueva("Antes de empezar", Propia, 6, Derecha),
        Seccion::nueva("Bloques de tiempo", Propia, 8, Derecha),
        Seccion::nueva("Notas", Propia, 4, Derecha),
    ]
}

const ANTES_DE_EMPEZAR: [&str; 6] = [
    "Materiales listos",
    "Lugar tranquilo",
    "Sin distracciones",
    "Música de fondo",
    "Agua y algo de comer",
    "Meta clara",
];

const PROPIOS_ESTUDIO: [Propio; 4] = [
    Propio {
        titulo: "Tiempo",
        que_es: "inicio, fin y total, cada uno con su renglón; líneas = filas",
        alto: |n| 30.0 * n as f32,
        dibujar: |p, h, n| {
            p.datos(
                h.x,
                h.y,
                h.w,
                &rotulos(&["Inicio:", "Fin:", "Total:"], n),
                30.0,
            );
        },
        estira: false,
    },
    Propio {
        titulo: "Antes de empezar",
        que_es: "casillas ya escritas (materiales listos, lugar tranquilo...); líneas = casillas",
        alto: |n| 28.0 * n as f32,
        dibujar: |p, h, n| {
            p.casillas(h.x, h.y, h.w, n as usize, 28.0, &ANTES_DE_EMPEZAR);
        },
        estira: false,
    },
    Propio {
        titulo: "Bloques de tiempo",
        que_es: "tabla de horas en bloques de 15, 30, 45 y 60 minutos; líneas = horas",
        alto: |n| 8.0 + 28.0 + 28.0 * n as f32,
        dibujar: bloques_de_tiempo,
        estira: false,
    },
    Propio {
        titulo: "Notas",
        que_es: "recuadro de notas con el círculo «Mi premio» abajo; baja hasta el pie si no tiene nada debajo",
        alto: |n| 30.0 * n as f32,
        dibujar: notas_con_premio,
        estira: true,
    },
];

fn bloques_de_tiempo(p: &mut Pag, h: Hueco, n: u32) {
    p.tabla(
        h.x,
        h.y + 8.0,
        h.w,
        &[
            ("", 1.3),
            ("15", 1.0),
            ("30", 1.0),
            ("45", 1.0),
            ("60", 1.0),
        ],
        n as usize,
        28.0,
    );
    for i in 0..n {
        p.l.texto(
            h.x + 10.0,
            h.y + 8.0 + 26.0 + 28.0 * i as f32 + 7.0,
            &format!("Hora {}", i + 1),
            10.0,
            WORK_SANS,
            ROTULO,
            false,
            Al::Izq,
        );
    }
}

fn notas_con_premio(p: &mut Pag, h: Hueco, _: u32) {
    let alto = h.alto - 10.0;
    p.caja(h.x, h.y + 10.0, h.w, alto, "");
    let (cx, cy) = (h.x + h.w - 56.0, h.y + 10.0 + alto - 56.0);
    p.l.circulo(cx, cy, 44.0, Some(p.t.acento), Some(p.t.suave), 1.5);
    p.l.texto(
        cx,
        cy - 8.0,
        "MI PREMIO",
        10.0,
        WORK_SANS,
        p.t.acento,
        true,
        Al::Centro,
    );
}

fn estudio(secs: Option<&[Seccion]>) -> FiguraGuardada {
    let secs = las_secciones(secs, secciones_estudio, &Tramo::de(1, 1));
    let z = Zona::de_dos(M, Y0, ANCHO, 30.0, 24.0).con_propios(&PROPIOS_ESTUDIO);
    let m = colocar(&z, &secs);
    let mut p = Pag::alta(ACADEMICO, "Sesión de estudio", &["Fecha"], m.fin + M);
    let fondo = p.h - M;
    p.poner_secciones(&z, &secs, m, fondo);
    p.fin("sesion-de-estudio", "Sesión de estudio")
}

/// **El planificador de tareas** (de una entrega). Sus bloques:
/// 1. Cabecera: «Curso».
/// 2. Zona de secciones con la columna izquierda estrecha (300) y la
///    derecha ancha (370): de fabrica, a la izquierda los detalles (tarea,
///    inicio, entrega), los recursos y los pasos; a la derecha el espacio
///    libre punteado (bloque propio: baja hasta el pie).
fn secciones_entrega(_: &Tramo) -> Vec<Seccion> {
    vec![
        Seccion::nueva("Detalles", Propia, 3, Izquierda),
        Seccion::nueva("Recursos", Renglones, 5, Izquierda),
        Seccion::nueva("Pasos", Casillas, 15, Izquierda),
        Seccion::nueva("Espacio libre (bocetos, notas, ideas)", Propia, 10, Derecha),
    ]
}

const PROPIOS_ENTREGA: [Propio; 2] = [
    Propio {
        titulo: "Detalles",
        que_es: "tarea, inicio y entrega, cada uno con su renglón; líneas = filas",
        alto: |n| 32.0 * n as f32,
        dibujar: |p, h, n| {
            p.datos(
                h.x,
                h.y,
                h.w,
                &rotulos(&["Tarea:", "Inicio:", "Entrega:"], n),
                32.0,
            );
        },
        estira: false,
    },
    Propio {
        titulo: "Espacio libre (bocetos, notas, ideas)",
        que_es: "recuadro punteado para bocetos; baja hasta el pie si no tiene nada debajo",
        alto: |n| 30.0 * n as f32,
        dibujar: espacio_punteado,
        estira: true,
    },
];

/// Un recuadro con una trama de puntos centrada.
fn espacio_punteado(p: &mut Pag, h: Hueco, _: u32) {
    let (x2, w2, alto) = (h.x, h.w, h.alto - 10.0);
    let y = h.y;
    p.l.rect(x2, y + 10.0, w2, alto, Some(BORDE), None, 1.25, true);
    let paso = 24.0;
    let (nc, nf) = ((w2 / paso) as usize, (alto / paso) as usize);
    if nc == 0 || nf == 0 {
        return;
    }
    let (ox, oy) = (
        x2 + (w2 - paso * (nc - 1) as f32) / 2.0,
        y + 10.0 + (alto - paso * (nf - 1) as f32) / 2.0,
    );
    for c in 0..nc {
        for f in 0..nf {
            p.l.circulo(
                ox + paso * c as f32,
                oy + paso * f as f32,
                1.3,
                None,
                Some(hex(0xb7c2d2)),
                1.0,
            );
        }
    }
}

fn entrega(secs: Option<&[Seccion]>) -> FiguraGuardada {
    let secs = las_secciones(secs, secciones_entrega, &Tramo::de(1, 1));
    let z = Zona {
        izquierda: 300.0,
        ..Zona::de_dos(M, Y0, ANCHO, 30.0, 24.0)
    }
    .con_propios(&PROPIOS_ENTREGA);
    let m = colocar(&z, &secs);
    let mut p = Pag::alta(ACADEMICO, "Planificador de tareas", &["Curso"], m.fin + M);
    let fondo = p.h - M;
    p.poner_secciones(&z, &secs, m, fondo);
    p.fin("planificador-de-tareas", "Planificador de tareas")
}

fn mapa_mental() -> FiguraGuardada {
    let mut p = Pag::apaisada(ACADEMICO, "Mapa mental", &["Tema"]);
    let (cx, cy) = (H / 2.0, 120.0 + (W - 120.0) / 2.0);
    let acento = p.t.acento;
    // Las hojas de los bordes y las ramas.
    let tarjeta = |p: &mut Pag, x: f32, y: f32| {
        p.l.rect(x, y, 150.0, 86.0, Some(BORDE), Some(BLANCO), 1.25, true);
        p.l.rect(x, y, 150.0, 22.0, None, Some(p.t.suave), 1.0, true);
    };
    let sub = [
        (cx - 250.0, cy - 150.0),
        (cx + 100.0, cy - 150.0),
        (cx - 250.0, cy + 110.0),
        (cx + 100.0, cy + 110.0),
    ];
    let hojas = [
        [(M, 140.0), (M, 270.0)],
        [(H - M - 150.0, 140.0), (H - M - 150.0, 270.0)],
        [(M, W - M - 216.0), (M, W - M - 86.0)],
        [
            (H - M - 150.0, W - M - 216.0),
            (H - M - 150.0, W - M - 86.0),
        ],
    ];
    for (k, (sx, sy)) in sub.iter().enumerate() {
        let (scx, scy) = (sx + 75.0, sy + 20.0);
        p.l.poli(&[(cx, cy), (cx, scy), (scx, scy)], acento, 1.5, None);
        for (hx, hy) in hojas[k] {
            let (ex, ey) = if hx < cx {
                (hx + 150.0, hy + 43.0)
            } else {
                (hx, hy + 43.0)
            };
            let medio = if hx < cx { ex + 30.0 } else { ex - 30.0 };
            p.l.poli(
                &[
                    (if hx < cx { *sx } else { sx + 150.0 }, scy),
                    (medio, scy),
                    (medio, ey),
                    (ex, ey),
                ],
                BORDE,
                1.25,
                None,
            );
            tarjeta(&mut p, hx, hy);
        }
    }
    for (sx, sy) in sub {
        p.l.rect(
            sx,
            sy,
            150.0,
            40.0,
            Some(acento),
            Some(p.t.suave),
            1.5,
            true,
        );
        p.l.texto(
            sx + 75.0,
            sy + 12.0,
            "SUBTEMA",
            10.0,
            WORK_SANS,
            acento,
            true,
            Al::Centro,
        );
    }
    p.l.rect(
        cx - 110.0,
        cy - 34.0,
        220.0,
        68.0,
        None,
        Some(acento),
        1.0,
        true,
    );
    p.l.texto(
        cx,
        cy - 9.0,
        "TEMA CENTRAL",
        13.0,
        WORK_SANS,
        BLANCO,
        true,
        Al::Centro,
    );
    p.fin("mapa-mental", "Mapa mental")
}

// ------------------------------------------------------------------ creativo

/// El ancho de las viñetas del storyboard de 4 (por el se sabe cual de los
/// dos es el que esta elegido: los dos se titulan igual).
pub(super) const VINETA_DE_4: f32 = 340.0;

fn storyboard(n: usize, tr: &Tramo) -> FiguraGuardada {
    let acento = CREATIVO.acento;
    let fw6 = (ANCHO - 28.0) / 2.0;
    let fh6 = fw6 * 9.0 / 16.0;
    let alto = if n == 4 {
        Y0 + 8.0 + tr.n() as f32 * (191.0 + 34.0) - 34.0 + M
    } else {
        Y0 + 8.0 + tr.n().div_ceil(2) as f32 * (fh6 + 110.0) - 26.0 + M
    };
    let mut p = Pag::alta(CREATIVO, "Storyboard", &["Proyecto", "Página"], alto);
    if let Some((i, _)) = tr.hoja {
        p.valor(1, &i.to_string());
    }
    let numero = |p: &mut Pag, x: f32, y: f32, i: u32| {
        let r = if i >= 100 { 15.0 } else { 12.0 };
        p.l.circulo(x + 16.0, y + 16.0, r, None, Some(acento), 1.0);
        p.l.texto(
            x + 16.0,
            y + 8.5,
            &format!("{i}"),
            11.0,
            WORK_SANS,
            BLANCO,
            true,
            Al::Centro,
        );
    };
    if n == 4 {
        let (fw, fh) = (VINETA_DE_4, 191.0);
        for (i, v) in tr.numeros().enumerate() {
            let y = Y0 + 8.0 + i as f32 * (fh + 34.0);
            p.l.rect(M, y, fw, fh, Some(BORDE), Some(hex(0xfcfcfd)), 1.5, false);
            numero(&mut p, M + 8.0, y + 8.0, v);
            let x2 = M + fw + 26.0;
            let w2 = W - M - x2;
            p.l.texto(x2, y, "ESCENA", 10.0, WORK_SANS, acento, true, Al::Izq);
            p.l.linea(x2 + 52.0, y + 15.0, x2 + 120.0, y + 15.0, BORDE, 1.0);
            p.l.texto(
                x2 + 140.0,
                y,
                "TOMA",
                10.0,
                WORK_SANS,
                acento,
                true,
                Al::Izq,
            );
            p.l.linea(x2 + 180.0, y + 15.0, x2 + w2, y + 15.0, BORDE, 1.0);
            p.renglones(x2, y + 18.0, w2, 6, 28.0);
        }
    } else {
        let (fw, fh) = (fw6, fh6);
        for (i, v) in tr.numeros().enumerate() {
            let (x, y) = (
                M + (fw + 28.0) * (i % 2) as f32,
                Y0 + 8.0 + (fh + 110.0) * (i / 2) as f32,
            );
            p.l.rect(x, y, fw, fh, Some(BORDE), Some(hex(0xfcfcfd)), 1.5, false);
            numero(&mut p, x + 8.0, y + 8.0, v);
            p.renglones(x, y + fh, fw, 3, 28.0);
        }
    }
    p.fin_de(
        &format!("storyboard-{n}"),
        &format!("Storyboard de {n} viñetas"),
        tr,
    )
}

/// **La receta.** Sus bloques:
/// 1. Cabecera: «Categoría».
/// 2. Arriba (fijo): la foto a la izquierda; el nombre y los detalles
///    (preparacion, coccion, porciones) a la derecha.
/// 3. Zona de secciones a dos columnas: de fabrica los ingredientes a la
///    izquierda (tantos como pasos, 16 como poco), la preparacion a la
///    derecha (con los numeros del tramo) y las notas a todo el ancho
///    (libre: baja hasta el pie).
fn secciones_receta(tr: &Tramo) -> Vec<Seccion> {
    vec![
        Seccion::nueva("Ingredientes", Casillas, tr.n().max(16) as u32, Izquierda),
        Seccion::nueva("Preparación", Numerados, tr.n() as u32, Derecha),
        Seccion::nueva("Notas", Libre, 4, Completo),
    ]
}

fn receta(tr: &Tramo, secs: Option<&[Seccion]>) -> FiguraGuardada {
    let secs = las_secciones(secs, secciones_receta, tr);
    let z = Zona::de_dos(M, Y0 + 260.0, ANCHO, 30.0, 26.0).con_serie("Preparación", tr.desde);
    let m = colocar(&z, &secs);
    let mut p = Pag::alta(CREATIVO, "Receta", &["Categoría"], m.fin + M);
    let mitad = ANCHO / 2.0 - 14.0;
    let x2 = M + mitad + 28.0;
    // Lo de arriba, fijo: la foto, el nombre y los detalles.
    p.l.parte("Foto y detalles");
    p.caja_suave(M, Y0, mitad, 230.0, "Foto");
    let y = p.datos(x2, Y0 - 6.0, mitad, &["Nombre:"], 34.0);
    let y = p.seccion(x2, y + 22.0, mitad, "Detalles");
    let fin = p.tabla(x2, y + 8.0, mitad, &[("", 1.4), ("", 2.0)], 3, 32.0);
    for (i, s) in ["Preparación", "Cocción", "Porciones"].iter().enumerate() {
        let yy = y + 8.0 + 32.0 * i as f32;
        p.l.rect(
            x2,
            yy,
            mitad * 1.4 / 3.4,
            32.0,
            None,
            Some(p.t.suave),
            1.0,
            false,
        );
        p.l.texto(
            x2 + 10.0,
            yy + 10.0,
            &s.to_uppercase(),
            9.5,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Izq,
        );
    }
    p.l.rect(
        x2,
        y + 8.0,
        mitad,
        fin - y - 8.0,
        Some(BORDE),
        None,
        1.25,
        false,
    );
    let fondo = p.h - M;
    p.poner_secciones(&z, &secs, m, fondo);
    p.fin_de("receta", "Receta", tr)
}

fn pentagramas() -> FiguraGuardada {
    let mut p = Pag::nueva(CREATIVO, "Partitura", &["Título", "Autor"]);
    let mut y = Y0 + 20.0;
    while y + 40.0 < H - M {
        for k in 0..5 {
            let yy = y + 9.0 * k as f32;
            p.l.linea(M, yy, W - M, yy, hex(0x8f9bb0), 1.0);
        }
        p.l.linea(M, y, M, y + 36.0, hex(0x8f9bb0), 1.5);
        p.l.linea(W - M, y, W - M, y + 36.0, hex(0x8f9bb0), 1.5);
        y += 82.0;
    }
    p.fin("pentagramas", "Partitura")
}

// -------------------------------------------------------------------- diario

/// La fila del animo: cinco caritas para marcar una.
fn animo(p: &mut Pag, x: f32, y: f32) {
    for i in 0..5 {
        let cx = x + i as f32 * 30.0;
        p.l.circulo(cx, y, 11.0, Some(p.t.acento), None, 1.25);
        p.l.circulo(cx - 4.0, y - 3.0, 1.2, None, Some(p.t.acento), 1.0);
        p.l.circulo(cx + 4.0, y - 3.0, 1.2, None, Some(p.t.acento), 1.0);
        let curva = [-4.0, -2.0, 0.0, 2.0, 4.0][i];
        p.l.poli(
            &[
                (cx - 5.0, y + 4.0),
                (cx, y + 4.0 + curva * 0.8),
                (cx + 5.0, y + 4.0),
            ],
            p.t.acento,
            1.25,
            None,
        );
    }
}

fn diario_libre() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Diario", &["Fecha"]);
    p.l.texto(
        M,
        Y0,
        "¿CÓMO ME SIENTO?",
        10.0,
        WORK_SANS,
        p.t.acento,
        true,
        Al::Izq,
    );
    animo(&mut p, M + 150.0, Y0 + 7.0);
    p.renglones(M, Y0 + 24.0, ANCHO, 30, 31.0);
    p.fin("diario", "Diario")
}

fn autocuidado() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Autocuidado", &["Fecha"]);
    let acento = p.t.acento;
    let cuarto = (ANCHO - 36.0) / 4.0;
    for (i, s) in ["Ánimo", "Clima", "Comidas", "Agua"].iter().enumerate() {
        let x = M + (cuarto + 12.0) * i as f32;
        p.caja_suave(x, Y0, cuarto, 74.0, s);
        let y = Y0 + 50.0;
        match i {
            0 => animo(&mut p, x + 24.0, y),
            1 => {
                p.l.circulo(x + 26.0, y, 9.0, Some(acento), None, 1.25);
                for k in 0..8 {
                    let a = (k as f32 * 45.0).to_radians();
                    p.l.linea(
                        x + 26.0 + 12.0 * a.cos(),
                        y + 12.0 * a.sin(),
                        x + 26.0 + 16.0 * a.cos(),
                        y + 16.0 * a.sin(),
                        acento,
                        1.25,
                    );
                }
                p.l.arco(x + 70.0, y + 2.0, 9.0, 180.0, 360.0, acento, 1.25);
                p.l.arco(x + 82.0, y - 2.0, 11.0, 200.0, 360.0, acento, 1.25);
                p.l.linea(x + 61.0, y + 2.0, x + 93.0, y + 2.0, acento, 1.25);
                for k in 0..3 {
                    let dx = x + 118.0 + k as f32 * 8.0;
                    p.l.linea(dx, y - 6.0, dx - 4.0, y + 6.0, acento, 1.25);
                }
            }
            2 => {
                for k in 0..3 {
                    p.l.circulo(
                        x + 26.0 + k as f32 * 34.0,
                        y,
                        11.0,
                        Some(acento),
                        None,
                        1.25,
                    );
                }
            }
            _ => {
                for k in 0..8 {
                    let dx = x + 18.0 + k as f32 * 18.0;
                    p.l.poli(
                        &[
                            (dx, y - 10.0),
                            (dx + 6.0, y),
                            (dx + 4.0, y + 6.0),
                            (dx, y + 8.0),
                            (dx - 4.0, y + 6.0),
                            (dx - 6.0, y),
                            (dx, y - 10.0),
                        ],
                        acento,
                        1.25,
                        None,
                    );
                }
            }
        }
    }
    let mut y = Y0 + 104.0;
    let preguntas: [(&str, usize); 9] = [
        ("¿Hablé con alguien hoy?", 0),
        ("¿Cuánta energía social tengo?", 0),
        ("¿Cuánto tiempo pasé con el móvil?", 0),
        ("¿Hice ejercicio? ¿Cuánto?", 0),
        ("¿Dormí una siesta?", 0),
        ("¿Comí bien hoy?", 0),
        ("¿Qué comí?", 2),
        ("¿Cómo se siente mi cuerpo?", 3),
        ("¿Tomé mis medicinas?", 0),
    ];
    for (s, extra) in preguntas {
        p.l.circulo(M + 5.0, y + 8.0, 4.0, None, Some(acento), 1.0);
        p.l.texto(
            M + 16.0,
            y,
            s,
            12.0,
            WORK_SANS,
            hex(0x4a3f48),
            true,
            Al::Izq,
        );
        p.l.texto(
            W - M - 70.0,
            y + 1.0,
            "SÍ    NO",
            10.0,
            WORK_SANS,
            acento,
            true,
            Al::Izq,
        );
        p.l.linea(M + 16.0, y + 26.0, W - M - 90.0, y + 26.0, RAYA, 1.0);
        y += 40.0;
        if extra > 0 {
            y = p.renglones(M + 16.0, y - 14.0, ANCHO - 16.0, extra, 30.0) + 18.0;
        }
    }
    let y = p.seccion(M, y + 6.0, ANCHO, "Medicinas");
    p.tabla(M, y + 10.0, ANCHO, &[("", 1.0); 8], 2, 34.0);
    p.fin("autocuidado", "Autocuidado")
}

fn gratitud() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Gratitud", &["Fecha"]);
    let izq = 400.0;
    let x2 = M + izq + 24.0;
    let w2 = W - M - x2;
    let alto = (H - M - Y0 - 2.0 * 20.0) / 3.0;
    for (i, (s, d)) in [
        ("Agradezco", "Dibuja lo que te hace feliz"),
        ("Mis logros", "Dibuja un logro"),
        ("Lo mejor del día", "Dibuja algo que te hizo sonreír"),
    ]
    .iter()
    .enumerate()
    {
        let y = Y0 + (alto + 20.0) * i as f32;
        let ys = p.seccion(M, y, izq, s);
        p.renglones(M, ys, izq, ((alto - 30.0) / 29.0) as usize, 29.0);
        p.caja(x2, y, w2, alto, "");
        p.l.texto(
            x2 + w2 / 2.0,
            y + 10.0,
            d,
            15.0,
            CAVEAT,
            p.t.acento,
            true,
            Al::Centro,
        );
    }
    p.fin("gratitud", "Gratitud")
}

fn metas_del_dia() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Metas del día", &["Fecha"]);
    let alto = (H - M - Y0 - 2.0 * 18.0) / 3.0;
    for (i, s) in [
        "Intención de la mañana",
        "Afirmaciones del mediodía",
        "Reflexión de la noche",
    ]
    .iter()
    .enumerate()
    {
        let y = Y0 + (alto + 18.0) * i as f32;
        p.l.rect(M, y, ANCHO, alto, Some(BORDE), Some(BLANCO), 1.25, true);
        p.l.rect(M, y, ANCHO, 34.0, None, Some(p.t.suave), 1.0, true);
        p.l.circulo(M + 20.0, y + 17.0, 7.0, None, Some(p.t.acento), 1.0);
        p.l.texto(
            M + 36.0,
            y + 9.0,
            &s.to_uppercase(),
            11.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Izq,
        );
    }
    p.fin("metas-del-dia", "Metas del día")
}

fn sueno() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Sueño", &["Fecha"]);
    let acento = p.t.acento;
    let izq = ANCHO - 90.0;
    let y = p.datos(M, Y0 - 6.0, izq / 2.0 - 12.0, &["Me acosté:"], 32.0);
    p.datos(
        M + izq / 2.0 + 12.0,
        Y0 - 6.0,
        izq / 2.0 - 12.0,
        &["Me levanté:"],
        32.0,
    );
    let mut y = y + 20.0;
    let preguntas: [(&str, usize); 9] = [
        ("¿Seguí mi horario de siempre?", 0),
        ("¿Cómo dormí?", 0),
        ("¿Cómo fue el lugar? (ruido, luz, temperatura)", 2),
        ("¿Miré el móvil antes de dormir?", 0),
        ("¿Comí algo antes de acostarme?", 0),
        ("¿Me desperté en la noche? ¿Cuántas veces?", 0),
        ("¿Soñé? ¿Con qué?", 4),
        ("¿Me sentí descansado al despertar?", 0),
        ("Notas", 4),
    ];
    for (s, extra) in preguntas {
        p.l.poli(
            &[
                (M + 5.0, y + 3.0),
                (M + 9.0, y + 8.0),
                (M + 5.0, y + 13.0),
                (M + 1.0, y + 8.0),
                (M + 5.0, y + 3.0),
            ],
            acento,
            1.0,
            Some(acento),
        );
        p.l.texto(
            M + 16.0,
            y,
            s,
            12.0,
            WORK_SANS,
            hex(0x4a3f48),
            true,
            Al::Izq,
        );
        p.l.linea(M + 16.0, y + 26.0, M + izq, y + 26.0, RAYA, 1.0);
        y += 40.0;
        if extra > 0 {
            y = p.renglones(M + 16.0, y - 14.0, izq - 16.0, extra, 30.0) + 18.0;
        }
    }
    // Las horas dormidas, en una columna a la derecha.
    let x = W - M - 40.0;
    p.l.rect(
        x - 10.0,
        Y0 - 6.0,
        60.0,
        500.0,
        None,
        Some(p.t.suave),
        1.0,
        true,
    );
    p.l.texto(
        x + 20.0,
        Y0 + 2.0,
        "HORAS",
        9.5,
        WORK_SANS,
        acento,
        true,
        Al::Centro,
    );
    for h in 0..12 {
        let cy = Y0 + 38.0 + h as f32 * 37.0;
        p.l.circulo(x + 20.0, cy, 12.0, Some(acento), Some(BLANCO), 1.25);
        p.l.texto(
            x + 20.0,
            cy - 7.0,
            &format!("{}", h + 1),
            10.0,
            WORK_SANS,
            acento,
            true,
            Al::Centro,
        );
    }
    p.fin("sueno", "Sueño")
}
