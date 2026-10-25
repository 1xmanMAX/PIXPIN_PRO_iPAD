//! **Una hoja (foto, escaneo o pagina de PDF) al lienzo, interpretada.**
//!
//! Pedido del usuario (14-oct-2026, diseno acordado): pasar al lienzo una
//! hoja con tabla, formulas, encabezados, varios parrafos, dos columnas con
//! una raya en medio y una foto, y que cada cosa llegue como lo que es —texto
//! del lienzo, tabla dibujada, ecuacion compuesta, foto pegada, rectas— en su
//! sitio y en el orden de lectura. Reglas de la casa: el mismo motor del
//! lienzo (todo sale por `lienzo::Fabrica` y las piezas de `pixpin_motor2d`),
//! un componente con variables ([`Opciones`]) y **la minima latencia**.
//!
//! **Solo con la nube** (decision del usuario, 14-oct): con el modelo local
//! la herramienta ni se ofrece y, si se pide, se contesta [`SOLO_NUBE`] sin
//! intentar nada ([`solo_nube`]). Lo que no necesita modelo (casi todo)
//! corre igual aqui, en Rust.
//!
//! ## El proceso
//!
//! 1. **Seccionar** ([`seccion::seccionar`], sin modelo, decenas de ms): de
//!    las palabras con caja del OCR (o de la capa de texto del PDF) y de un
//!    analisis de la imagen ([`imagen`]): rayas largas y finas (separadores
//!    de columnas, bordes de tabla, ejes), fotos (zonas con medios tonos),
//!    dibujos y graficas (tinta sin texto, con ejes o sin ellos), renglones,
//!    y el **orden de lectura** con un corte XY que respeta las columnas.
//!    Luego cada bloque se clasifica: encabezado (letra mayor o negrita),
//!    parrafo (interlineado y sangria), tabla (rejilla o columnas alineadas,
//!    con `tabla_escaneada`), formula (simbolos, renglones centrados). Lo
//!    dudoso se marca [`Tipo::Dudosa`] y se le pregunta al modelo SOLO con su
//!    recorte ([`vision`]).
//! 2. **Mapear** ([`Mapa`]): una escala comun pagina→lienzo (el ancho de la
//!    hoja a [`Opciones::ancho`], A4 a 1000 px por omision); cada zona con su
//!    caja destino.
//! 3. **Trabajar** ([`montaje`]): cada zona a sus elementos. Lo que no usa
//!    modelo sale ya; las formulas (a LaTeX), las zonas dudosas y las
//!    graficas de funciones van al modelo con vision en paralelo (lo hace la
//!    app con un pool, `apps/pixpin/src/asistente/hoja_a_lienzo.rs`).
//! 4. **Comprobar** ([`comparar`]): el lienzo pintado contra el original,
//!    zona a zona; la que difiere mucho se rehace una vez por otro camino
//!    ([`montaje::Via`]).
//!
//! ## Enganches para otros agentes
//!
//! - **Calcado** (`ia-calcar-suave`): [`montaje`] llama a `calcar::calcar` y
//!   recoloca sus trazos con `Calcado::escala`/`desplazamiento`; si el calco
//!   cambia de forma (Bezier), solo hay que tocar `montaje::calco_en_caja`.
//! - **Graficas de funciones** (`ia-estudiantes`): `montaje::Trabajo::Grafica`
//!   con [`vision::LeidoGrafica`] usa `funciones::piezas` en la caja destino.
//! - **Edicion del lienzo** (`ia-edicion-lienzo`): las fotos van como
//!   elementos `image` con su `fileId` (`pc<hex>`) y sus pixeles aparte
//!   ([`montaje::ImagenNueva`]), igual que una imagen pegada.

pub mod comparar;
pub mod imagen;
pub mod montaje;
pub mod paneles;
pub mod seccion;
pub mod vision;

#[cfg(test)]
mod pruebas;

use serde_json::Value;

use crate::motor::{Fin, Mensaje, Respuesta, Rol};

/// El nombre de la herramienta (su fila esta en `acciones_documentos`).
pub const NOMBRE: &str = "hoja_a_lienzo";

/// Lo que se contesta si se pide con el modelo local (decision del usuario).
pub const SOLO_NUBE: &str =
    "Para pasar una hoja al lienzo necesitas Claude o DeepSeek: actívalo en Ajustes → Asistente.";

/// Una caja en pixeles (de la pagina o del lienzo): esquina de arriba a la
/// izquierda, ancho y alto.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Caja {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Caja {
    pub fn nueva(x: f64, y: f64, w: f64, h: f64) -> Caja {
        Caja { x, y, w: w.max(0.0), h: h.max(0.0) }
    }
    pub fn de_esquinas(x0: f64, y0: f64, x1: f64, y1: f64) -> Caja {
        Caja::nueva(x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs())
    }
    pub fn x1(&self) -> f64 {
        self.x + self.w
    }
    pub fn y1(&self) -> f64 {
        self.y + self.h
    }
    pub fn cx(&self) -> f64 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f64 {
        self.y + self.h / 2.0
    }
    pub fn area(&self) -> f64 {
        self.w * self.h
    }
    pub fn union(&self, o: &Caja) -> Caja {
        Caja::de_esquinas(self.x.min(o.x), self.y.min(o.y), self.x1().max(o.x1()), self.y1().max(o.y1()))
    }
    /// Lo que se pisan (area).
    pub fn comun(&self, o: &Caja) -> f64 {
        let w = (self.x1().min(o.x1()) - self.x.max(o.x)).max(0.0);
        let h = (self.y1().min(o.y1()) - self.y.max(o.y)).max(0.0);
        w * h
    }
    /// Que parte de esta caja cae dentro de `o` (0-1).
    pub fn dentro_de(&self, o: &Caja) -> f64 {
        if self.area() <= 0.0 {
            return if o.contiene(self.cx(), self.cy()) { 1.0 } else { 0.0 };
        }
        self.comun(o) / self.area()
    }
    pub fn contiene(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x1() && y >= self.y && y <= self.y1()
    }
    /// Lo que se solapan en vertical (pixeles).
    pub fn solape_y(&self, o: &Caja) -> f64 {
        (self.y1().min(o.y1()) - self.y.max(o.y)).max(0.0)
    }
    pub fn solape_x(&self, o: &Caja) -> f64 {
        (self.x1().min(o.x1()) - self.x.max(o.x)).max(0.0)
    }
    /// Agrandada `m` por cada lado.
    pub fn crecer(&self, m: f64) -> Caja {
        Caja::nueva(self.x - m, self.y - m, self.w + 2.0 * m, self.h + 2.0 * m)
    }
    /// Recortada a `0..ancho, 0..alto`.
    pub fn dentro(&self, ancho: f64, alto: f64) -> Caja {
        Caja::de_esquinas(self.x.clamp(0.0, ancho), self.y.clamp(0.0, alto), self.x1().clamp(0.0, ancho), self.y1().clamp(0.0, alto))
    }
}

/// Una palabra (o un trozo de texto del PDF) con su caja en pixeles de la
/// imagen de la pagina.
#[derive(Debug, Clone, PartialEq)]
pub struct PalabraHoja {
    pub texto: String,
    pub caja: Caja,
    /// La capa de texto del PDF lo sabe; el OCR no (se estima por la tinta).
    pub negrita: bool,
    /// El color de su tinta (rojo, morado, el azul de un enlace), medido en
    /// la imagen. `None`: no se sabe (negro).
    pub color: Option<[u8; 3]>,
}

impl PalabraHoja {
    pub fn nueva(texto: &str, x: f64, y: f64, w: f64, h: f64) -> PalabraHoja {
        PalabraHoja { texto: texto.to_string(), caja: Caja::nueva(x, y, w, h), negrita: false, color: None }
    }
}

/// Lo que es cada zona.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tipo {
    Encabezado,
    Parrafo,
    Tabla,
    Formula,
    Foto,
    Grafica,
    Dibujo,
    /// Un dibujo hecho de figuras exactas (una figura geometrica, un
    /// esquema): rectas, circunferencias, puntos, flechas (`diagrama`).
    Diagrama,
    /// Una raya separadora (entre columnas, bajo un titulo).
    Linea,
    /// No esta claro: se le pregunta al modelo con su recorte.
    Dudosa,
    /// **Un recuadro** (17-oct): una tarjeta con su fondo o una caja con su
    /// borde ([`paneles`]). Va como un rectangulo relleno, debajo de lo suyo,
    /// que se secciona aparte.
    Recuadro,
}

impl Tipo {
    pub fn nombre(self) -> &'static str {
        match self {
            Tipo::Encabezado => "encabezado",
            Tipo::Parrafo => "parrafo",
            Tipo::Tabla => "tabla",
            Tipo::Formula => "formula",
            Tipo::Foto => "foto",
            Tipo::Grafica => "grafica",
            Tipo::Dibujo => "dibujo",
            Tipo::Diagrama => "diagrama",
            Tipo::Linea => "linea",
            Tipo::Dudosa => "dudosa",
            Tipo::Recuadro => "recuadro",
        }
    }
    /// Si su trabajo necesita el modelo con vision.
    pub fn usa_modelo(self) -> bool {
        matches!(self, Tipo::Formula | Tipo::Dudosa | Tipo::Grafica | Tipo::Diagrama)
    }
}

/// Como van los renglones de un bloque de texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alineacion {
    #[default]
    Izquierda,
    Centro,
    Derecha,
    Justificada,
}

/// Un renglon de texto (un trozo de una fila, sin cruzar columnas).
#[derive(Debug, Clone, PartialEq)]
pub struct Renglon {
    pub texto: String,
    pub caja: Caja,
    /// Las cajas de sus palabras (para colocarlas tal cual al rehacer).
    pub palabras: Vec<PalabraHoja>,
    pub negrita: bool,
    /// El color de su tinta (el de la mayoria de sus letras).
    pub color: Option<[u8; 3]>,
}

impl Renglon {
    pub fn alto(&self) -> f64 {
        // El alto mediano de sus palabras: una sola con descendente o un
        // signo alto no cambia el del renglon.
        mediana(self.palabras.iter().map(|p| p.caja.h).collect()).unwrap_or(self.caja.h)
    }
    pub fn n_palabras(&self) -> usize {
        self.texto.split_whitespace().count()
    }
}

/// **Una zona de la hoja**, con lo que el trabajador necesita.
#[derive(Debug, Clone, PartialEq)]
pub struct Zona {
    /// Su numero (el orden de lectura empieza en 0).
    pub id: usize,
    pub tipo: Tipo,
    /// En pixeles de la imagen de la pagina.
    pub caja: Caja,
    /// Orden de lectura (0, 1, 2...).
    pub orden: usize,
    /// Columna (0 = la de la izquierda; una zona a todo lo ancho, 0).
    pub columna: usize,
    /// Lo seguro que se esta del tipo (0-1). Por debajo de
    /// [`Opciones::duda`], si hay modelo, se le pregunta.
    pub confianza: f32,
    /// Sus renglones (texto, encabezado, formula, tabla).
    pub renglones: Vec<Renglon>,
    pub alineacion: Alineacion,
    /// La tabla leida (`tabla_escaneada`), en las tablas.
    pub tabla: Option<crate::tabla_escaneada::TablaLeida>,
    /// Las rayas de dentro (de una tabla rayada; o la propia raya), en
    /// pixeles de la pagina: `(x0, y0, x1, y1, grosor)`.
    pub rayas: Vec<[f64; 5]>,
    /// Imagenes dentro de la zona (los iconos de las celdas de una tabla),
    /// en pixeles de la pagina: van recortadas encima.
    pub dentro: Vec<Caja>,
    /// Su relleno y su borde, en un [`Tipo::Recuadro`].
    pub panel: Option<paneles::Estilo>,
}

impl Zona {
    /// Todo su texto, renglon a renglon.
    pub fn texto(&self) -> String {
        self.renglones.iter().map(|r| r.texto.as_str()).collect::<Vec<_>>().join("\n")
    }
}

/// Lo que sale de seccionar una hoja.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Seccionado {
    pub ancho: u32,
    pub alto: u32,
    /// En orden de lectura.
    pub zonas: Vec<Zona>,
    /// Cuantas columnas de texto se vieron (1 si ninguna separacion).
    pub columnas: usize,
    /// El alto de letra del cuerpo (mediana de los renglones), en pixeles.
    pub alto_letra: f64,
    /// Las cajas de los recuadros: lo de un recuadro no se junta con lo de
    /// otro (`bloques_de_texto` de la app).
    pub paneles: Vec<Caja>,
}

impl Seccionado {
    pub fn cuantas(&self, t: Tipo) -> usize {
        self.zonas.iter().filter(|z| z.tipo == t).count()
    }
}

/// Las variables del componente.
#[derive(Debug, Clone, PartialEq)]
pub struct Opciones {
    /// El ancho de la hoja en el lienzo (A4 a 1000 px por omision).
    pub ancho: f64,
    /// La letra del texto corrido.
    pub familia: String,
    /// La letra de los encabezados.
    pub familia_titulos: String,
    /// Por debajo de esta confianza una zona se le pregunta al modelo.
    pub duda: f32,
    /// Cuantas peticiones a la nube a la vez.
    pub concurrencia: usize,
    /// **Texto rico de la nube** (15-oct): los bloques de texto y formulas
    /// (la columna «RESOLUCION» de una lamina) se le piden a la nube en texto
    /// rico, con sus formulas en linea, colores y cursiva. Para las laminas
    /// que se calcan; una hoja de texto corriente se queda con el OCR.
    pub rico: bool,
    /// Por debajo de este parecido (0-1) una zona se rehace una vez.
    pub rehacer_bajo: f64,
    /// Comprobar al final (pintar y comparar).
    pub comprobar: bool,
    /// Lo mas que se espera a la nube: lo que no llegue sale sin modelo (una
    /// formula como imagen). Una red colgada no deja la hoja a medias.
    pub plazo_nube: std::time::Duration,
}

impl Default for Opciones {
    fn default() -> Self {
        Opciones {
            ancho: 1000.0,
            familia: "Work Sans".into(),
            familia_titulos: "Work Sans".into(),
            duda: 0.55,
            concurrencia: 4,
            rico: false,
            rehacer_bajo: 0.35,
            comprobar: true,
            plazo_nube: std::time::Duration::from_secs(40),
        }
    }
}

/// **La escala comun pagina → lienzo.** Todo lo de la hoja cae en
/// `origen + caja * escala`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mapa {
    pub escala: f64,
    pub origen: (f64, f64),
}

impl Mapa {
    pub fn para(s: &Seccionado, o: &Opciones) -> Mapa {
        Mapa { escala: o.ancho / s.ancho.max(1) as f64, origen: (0.0, 0.0) }
    }
    pub fn caja(&self, c: &Caja) -> Caja {
        Caja::nueva(self.origen.0 + c.x * self.escala, self.origen.1 + c.y * self.escala, c.w * self.escala, c.h * self.escala)
    }
    pub fn punto(&self, x: f64, y: f64) -> (f64, f64) {
        (self.origen.0 + x * self.escala, self.origen.1 + y * self.escala)
    }
    /// El tamano de la hoja entera en el lienzo.
    pub fn hoja(&self, s: &Seccionado) -> (f64, f64) {
        (s.ancho as f64 * self.escala, s.alto as f64 * self.escala)
    }
}

/// La mediana (sin `NaN`).
pub fn mediana(mut v: Vec<f64>) -> Option<f64> {
    v.retain(|x| x.is_finite());
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(v[v.len() / 2])
}

// ---------------------------------------------------------------------------
// Solo la nube

/// Si el pedido es pasar una hoja (una foto de un papel, un escaneo, una
/// pagina) al lienzo: «pasa esta hoja al lienzo», «digitaliza esta foto»,
/// «esta pagina al lienzo».
pub fn pide_hoja_a_lienzo(pedido: &str) -> bool {
    let q = crate::acciones::normalizar(pedido);
    let palabras: Vec<&str> = q.split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty()).collect();
    let empieza = |raices: &[&str]| palabras.iter().any(|p| raices.iter().any(|r| p.starts_with(r)));
    if empieza(&["digitaliz"]) {
        return true;
    }
    let hoja = empieza(&["hoja", "pagina", "escaneo", "escanead", "apunte", "folio", "documento", "papel", "foto"]);
    let al_lienzo = q.contains("al lienzo") || q.contains("en el lienzo") || q.contains("a un lienzo") || q.contains("en un lienzo");
    let verbo = empieza(&["pas", "pon", "conviert", "lleva", "mete", "transcrib", "copia", "haz"]);
    hoja && al_lienzo && verbo
}

/// **Con el modelo local no se intenta**: si el ultimo mensaje del usuario
/// pide pasar una hoja al lienzo, la respuesta es [`SOLO_NUBE`] y el turno
/// acaba sin preguntar al modelo. `None`: no era eso.
pub fn solo_nube(historial: &mut Vec<Mensaje>, al_evento: &mut dyn FnMut(crate::agente::Evento) -> crate::motor::Seguir) -> Option<crate::agente::Final> {
    let ultimo = historial.last().filter(|m| m.rol == Rol::Usuario)?;
    if !pide_hoja_a_lienzo(&ultimo.texto_entero()) {
        return None;
    }
    al_evento(crate::agente::Evento::Texto(SOLO_NUBE.to_string()));
    historial.push(Mensaje::texto(Rol::Asistente, SOLO_NUBE));
    Some(crate::agente::Final { texto: SOLO_NUBE.into(), herramientas_hechas: 0, reintentos: 0, cortado: false })
}

/// Una respuesta vacia (para los motores de prueba).
pub fn respuesta_de_texto(t: &str) -> Respuesta {
    let mut r = Respuesta::de_texto(t);
    r.fin = Fin::Natural;
    r
}

/// El resumen que se le cuenta al modelo al acabar.
pub fn resumen(s: &Seccionado, hechas: usize, rehechas: usize, ms: u128) -> String {
    let mut por_tipo: Vec<String> = Vec::new();
    for t in [
        Tipo::Encabezado,
        Tipo::Parrafo,
        Tipo::Tabla,
        Tipo::Formula,
        Tipo::Foto,
        Tipo::Grafica,
        Tipo::Dibujo,
        Tipo::Diagrama,
        Tipo::Linea,
    ] {
        let n = s.cuantas(t);
        if n > 0 {
            por_tipo.push(format!("{n} {}", t.nombre()));
        }
    }
    format!(
        "hecho: la hoja esta en el lienzo, {hechas} zonas ({}), {} columna(s){}, en {:.1} s",
        por_tipo.join(", "),
        s.columnas.max(1),
        if rehechas > 0 { format!(", {rehechas} rehecha(s) tras comprobar") } else { String::new() },
        ms as f64 / 1000.0
    )
}

/// Los argumentos de la llamada, leidos (`archivo`, `pagina` desde 1,
/// `ancho`, `modo`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pedido {
    pub archivo: Option<String>,
    /// Desde 0.
    pub pagina: Option<usize>,
    pub ancho: Option<f64>,
    /// `editable` (lo normal) o `fiel` (cada renglon en su sitio exacto).
    pub fiel: bool,
}

impl Pedido {
    pub fn de(args: &Value) -> Pedido {
        let texto = |k: &str| args.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
        Pedido {
            archivo: texto("archivo"),
            pagina: args.get("pagina").and_then(Value::as_u64).filter(|p| *p >= 1).map(|p| p as usize - 1),
            ancho: args.get("ancho").and_then(Value::as_f64).filter(|a| (200.0..=4000.0).contains(a)),
            fiel: texto("modo").as_deref() == Some("fiel"),
        }
    }
}
