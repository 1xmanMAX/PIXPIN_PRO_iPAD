//! **Las secciones de una plantilla, como datos** (25-oct-2026).
//!
//! El usuario: «tengo un planificador mensual con las casillas y abajo
//! objetivos del mes, tareas, prioridades. Le digo quita prioridades, amplia
//! los objetivos del mes a 5 o 6 lineas, las tareas reducelas; objetivos y
//! tareas subelos arriba, prioridades que ocupe todo el ancho… y que lo
//! reordene». El asistente lo tiene que hacer con el MISMO codigo que dibuja
//! la plantilla, no redibujandola a mano.
//!
//! Por eso cada hoja se parte en **bloques**: la cabecera (titulo y campos),
//! lo fijo de esa plantilla (el calendario del mensual, el horario del
//! semanal...) y una **zona de secciones**. Lo de la zona ya no esta escrito
//! a mano en cada plantilla: es una lista de [`Seccion`] (titulo, tipo,
//! lineas y ancho) que coloca el maquetador ([`colocar`]) con las mismas
//! piezas de siempre (`Pag::seccion`, `renglones`, `numerados`, `casillas`).
//! La lista de fabrica de cada plantilla dibuja lo mismo que antes; otra
//! lista (la que manda el asistente) da la hoja personalizada.
//!
//! **Como se colocan**, en el orden de la lista:
//! - `Completo`: una fila entera, debajo de todo lo anterior.
//! - `Izquierda` / `Derecha`: media columna; cada columna apila las suyas.
//! - `Mitad`: de dos en dos y alineadas arriba: la primera abre una fila
//!   nueva a la izquierda (debajo de todo) y la siguiente va a su derecha.
//! - `Lateral`: en una columna a la derecha de lo fijo de la hoja (en el
//!   cronograma, junto a la tabla); la hoja se ensancha lo justo. Solo en
//!   las plantillas que lo tienen ([`Zona::lateral`]).
//!
//! Cada seccion mide su rotulo ([`TITULO`]) mas `lineas × paso` (o lo que
//! mida su bloque propio) y entre una y otra va el `hueco` de la plantilla.
//! Si no cabe, la hoja crece (`Pag::alta`); si sobra sitio, un recuadro
//! libre que no tenga nada debajo se estira hasta el pie (las «Notas»).

use super::plantillas::Pag;

/// Lo que ocupa el rotulo de una seccion con su raya (`Pag::seccion`).
pub(super) const TITULO: f32 = 20.0;
/// Cuantas secciones como mucho en una hoja.
pub const MAX_SECCIONES: usize = 12;
/// Cuantas lineas como mucho en una seccion pedida.
pub const MAX_LINEAS: u32 = 40;
/// El largo maximo de un titulo.
const MAX_TITULO: usize = 60;

/// **Que lleva una seccion** debajo de su rotulo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoSeccion {
    /// Renglones para escribir.
    Renglones,
    /// `01`, `02`... con su renglon (prioridades, pasos).
    Numerados,
    /// Una casilla para marcar en cada renglon (tareas, listas).
    Casillas,
    /// Un recuadro de fondo suave, para lo que se destaca (la meta).
    Recuadro,
    /// Un recuadro en blanco con borde, para notas o dibujos; si no tiene
    /// nada debajo, se estira hasta el pie de la hoja.
    Libre,
    /// Una zona gris clara para anotar (el fondo suave del tema, sin rayas):
    /// «abajo una zona gris donde anotar».
    Notas,
    /// Un bloque propio de esa plantilla (una tabla, unos datos, la rejilla
    /// de habitos...): solo con el titulo de uno de los suyos, y sus
    /// `lineas` son sus filas.
    Propia,
}

impl TipoSeccion {
    pub const TODOS: [TipoSeccion; 7] = [
        TipoSeccion::Renglones,
        TipoSeccion::Numerados,
        TipoSeccion::Casillas,
        TipoSeccion::Recuadro,
        TipoSeccion::Libre,
        TipoSeccion::Notas,
        TipoSeccion::Propia,
    ];

    /// Su nombre para el modelo (el que se escribe en el JSON).
    pub fn nombre(self) -> &'static str {
        match self {
            TipoSeccion::Renglones => "renglones",
            TipoSeccion::Numerados => "numerados",
            TipoSeccion::Casillas => "casillas",
            TipoSeccion::Recuadro => "recuadro",
            TipoSeccion::Libre => "libre",
            TipoSeccion::Notas => "notas",
            TipoSeccion::Propia => "propia",
        }
    }

    /// El tipo por su palabra, con las que el modelo suele usar.
    pub fn de_palabra(s: &str) -> Option<TipoSeccion> {
        let s = normalizar(s);
        Some(match s.as_str() {
            "renglones" | "renglon" | "lineas" | "linea" | "texto" => TipoSeccion::Renglones,
            "numerados" | "numerado" | "numeros" | "lista numerada" | "numerada" => {
                TipoSeccion::Numerados
            }
            "casillas" | "casilla" | "checklist" | "checkbox" | "marcar" | "tareas" => {
                TipoSeccion::Casillas
            }
            "recuadro" | "destacado" | "caja suave" | "fondo" => TipoSeccion::Recuadro,
            "libre" | "caja" | "espacio" | "en blanco" | "blanco" => TipoSeccion::Libre,
            "notas" | "nota" | "zona gris" | "gris" | "anotar" | "anotaciones" => {
                TipoSeccion::Notas
            }
            "propia" | "propio" | "tabla" | "bloque" | "fija" | "fijo" => TipoSeccion::Propia,
            _ => return None,
        })
    }
}

/// **Cuanto ancho ocupa** y en que columna.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchoSeccion {
    /// Todo el ancho de la zona, debajo de todo lo anterior.
    Completo,
    /// Media columna, de dos en dos alineadas arriba.
    Mitad,
    /// Media columna, apilada en la de la izquierda.
    Izquierda,
    /// Media columna, apilada en la de la derecha.
    Derecha,
    /// En la columna lateral, a la derecha de lo fijo de la hoja.
    Lateral,
}

impl AnchoSeccion {
    pub fn nombre(self) -> &'static str {
        match self {
            AnchoSeccion::Completo => "completo",
            AnchoSeccion::Mitad => "mitad",
            AnchoSeccion::Izquierda => "izquierda",
            AnchoSeccion::Derecha => "derecha",
            AnchoSeccion::Lateral => "lateral",
        }
    }

    pub fn de_palabra(s: &str) -> Option<AnchoSeccion> {
        let s = normalizar(s);
        Some(match s.as_str() {
            "completo" | "completa" | "todo" | "todo el ancho" | "entero" | "entera" | "ancho"
            | "full" | "100%" => AnchoSeccion::Completo,
            "mitad" | "media" | "medio" | "half" | "50%" => AnchoSeccion::Mitad,
            "izquierda" | "izq" | "left" => AnchoSeccion::Izquierda,
            "derecha" | "der" | "right" => AnchoSeccion::Derecha,
            "lateral" | "al lado" | "a un lado" | "costado" | "side" | "sidebar" => {
                AnchoSeccion::Lateral
            }
            _ => return None,
        })
    }
}

/// **Una seccion de la zona**: su rotulo, lo que lleva, cuantas lineas (o
/// filas) y su ancho.
#[derive(Clone, Debug, PartialEq)]
pub struct Seccion {
    pub titulo: String,
    pub tipo: TipoSeccion,
    pub lineas: u32,
    pub ancho: AnchoSeccion,
}

impl Seccion {
    pub fn nueva(titulo: &str, tipo: TipoSeccion, lineas: u32, ancho: AnchoSeccion) -> Seccion {
        Seccion {
            titulo: titulo.to_string(),
            tipo,
            lineas,
            ancho,
        }
    }
}

/// **Una seccion como la pide el asistente**: lo que no diga se toma de la
/// seccion de fabrica con ese titulo (asi «Prioridades a todo el ancho» no
/// tiene que repetir su tipo ni sus lineas).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeccionPedida {
    pub titulo: String,
    pub tipo: Option<TipoSeccion>,
    pub lineas: Option<u32>,
    pub ancho: Option<AnchoSeccion>,
}

/// Sin tildes, en minusculas y sin signos: «¿Por qué?» es «por que».
pub(super) fn normalizar(s: &str) -> String {
    let s: String = s
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            c if c.is_alphanumeric() || c == '%' => c,
            _ => ' ',
        })
        .collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Los bloques propios de cada plantilla
// ---------------------------------------------------------------------------

/// El sitio de lo de debajo de un rotulo: `y` es donde acaba la raya.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Hueco {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub alto: f32,
}

/// **Un bloque que solo tiene una plantilla** (la tabla de hitos de las
/// metas, los datos del curso, la rejilla de habitos del semanal...). Se
/// puede quitar, mover, ensanchar o cambiar de filas como cualquier
/// seccion; lo que dibuja lo sabe la plantilla.
pub(super) struct Propio {
    pub titulo: &'static str,
    /// Que es, para el modelo.
    pub que_es: &'static str,
    /// Lo que mide debajo del rotulo con esas filas.
    pub alto: fn(u32) -> f32,
    pub dibujar: fn(&mut Pag, Hueco, u32),
    /// Si se estira hasta el pie cuando no tiene nada debajo.
    pub estira: bool,
}

/// **La zona de secciones de una hoja**: donde empieza, su ancho, el de la
/// columna izquierda (la derecha es lo que queda tras `entre`), el paso de
/// los renglones y el hueco entre secciones.
pub(super) struct Zona<'a> {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub izquierda: f32,
    pub entre: f32,
    pub paso: f32,
    pub hueco: f32,
    /// La seccion numerada que cuenta la serie de la plantilla (los pasos
    /// de las metas) y desde que numero.
    pub serie: Option<(&'static str, u32)>,
    pub propios: &'a [Propio],
    /// La columna lateral `(x, y, ancho)`, si la hoja la tiene: va a la
    /// derecha de lo fijo y no se mezcla con las columnas de la zona.
    pub lateral: Option<(f32, f32, f32)>,
    /// Hasta donde baja la ultima zona de notas (o libre) del lateral: el
    /// pie de lo fijo de al lado (la tabla del cronograma), para que no se
    /// quede a media altura (24-oct-2026).
    pub fondo_lateral: f32,
}

impl<'a> Zona<'a> {
    /// Dos columnas iguales con 28 entre medias (las de siempre).
    pub fn de_dos(x: f32, y: f32, ancho: f32, paso: f32, hueco: f32) -> Zona<'a> {
        Zona {
            x,
            y,
            ancho,
            izquierda: ancho / 2.0 - 14.0,
            entre: 28.0,
            paso,
            hueco,
            serie: None,
            propios: &[],
            lateral: None,
            fondo_lateral: 0.0,
        }
    }

    pub fn con_propios(mut self, propios: &'a [Propio]) -> Zona<'a> {
        self.propios = propios;
        self
    }

    pub fn con_serie(mut self, titulo: &'static str, desde: u32) -> Zona<'a> {
        self.serie = Some((titulo, desde));
        self
    }

    fn propio(&self, titulo: &str) -> Option<&Propio> {
        let t = normalizar(titulo);
        self.propios.iter().find(|p| normalizar(p.titulo) == t)
    }

    /// Lo que mide una seccion debajo de su rotulo.
    fn contenido(&self, s: &Seccion) -> f32 {
        match (s.tipo, self.propio(&s.titulo)) {
            (TipoSeccion::Propia, Some(p)) => (p.alto)(s.lineas),
            _ => self.paso * s.lineas as f32,
        }
    }

    fn estira(&self, s: &Seccion) -> bool {
        match s.tipo {
            TipoSeccion::Libre => true,
            TipoSeccion::Propia => self.propio(&s.titulo).is_some_and(|p| p.estira),
            _ => false,
        }
    }
}

/// Donde va cada seccion: `y` es la de su rotulo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Sitio {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub contenido: f32,
}

/// La zona ya colocada: un sitio por seccion y donde acaba lo mas bajo.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Maqueta {
    pub sitios: Vec<Sitio>,
    pub fin: f32,
}

/// **El maquetador**: coloca las secciones en su orden (ver el principio
/// del modulo). Es puro: no dibuja, para que la plantilla sepa antes de
/// empezar si la hoja tiene que crecer.
pub(super) fn colocar(z: &Zona, secciones: &[Seccion]) -> Maqueta {
    let derecha = (z.x + z.izquierda + z.entre, z.ancho - z.izquierda - z.entre);
    let izquierda = (z.x, z.izquierda);
    // Donde va el siguiente rotulo de cada columna.
    let mut cols = [z.y, z.y];
    // La fila de «Mitad» abierta a la izquierda (su `y`), si la hay.
    let mut fila: Option<f32> = None;
    // Lo mas bajo de lo colocado (en el lateral puede no haber nada debajo).
    let mut fin = f32::MIN;
    // Donde va el siguiente rotulo del lateral.
    let mut lat = z.lateral.map_or(z.y, |l| l.1);
    let mut sitios = Vec::with_capacity(secciones.len());
    // Una columna sin nadie en la otra dejaria medio ancho vacio (quitar el
    // objetivo del mensual dejaba Tareas sola a la derecha, 24-oct-2026): a
    // todo el ancho.
    let hay = |a: AnchoSeccion| secciones.iter().any(|s| s.ancho == a);
    let sola = !(hay(AnchoSeccion::Izquierda) && hay(AnchoSeccion::Derecha));
    for s in secciones {
        let contenido = z.contenido(s);
        let alto = TITULO + contenido;
        let abajo = cols[0].max(cols[1]);
        let ancho = match s.ancho {
            AnchoSeccion::Izquierda | AnchoSeccion::Derecha if sola => AnchoSeccion::Completo,
            a => a,
        };
        let (x, y, w) = match (ancho, z.lateral) {
            (AnchoSeccion::Lateral, Some((lx, _, lw))) => {
                let y = lat;
                lat = y + alto + z.hueco;
                (lx, y, lw)
            }
            // Sin lateral (no pasa: se valida antes), a todo el ancho.
            (AnchoSeccion::Completo | AnchoSeccion::Lateral, _) => {
                fila = None;
                cols = [abajo + alto + z.hueco; 2];
                (z.x, abajo, z.ancho)
            }
            (AnchoSeccion::Mitad, _) => match fila.take() {
                Some(y) => {
                    cols[1] = y + alto + z.hueco;
                    (derecha.0, y, derecha.1)
                }
                None => {
                    fila = Some(abajo);
                    cols = [abajo + alto + z.hueco, abajo];
                    (izquierda.0, abajo, izquierda.1)
                }
            },
            (AnchoSeccion::Izquierda | AnchoSeccion::Derecha, _) => {
                fila = None;
                let i = usize::from(ancho == AnchoSeccion::Derecha);
                let y = cols[i];
                cols[i] = y + alto + z.hueco;
                let (x, w) = if i == 0 { izquierda } else { derecha };
                (x, y, w)
            }
        };
        fin = fin.max(y + alto);
        sitios.push(Sitio { x, y, w, contenido });
    }
    // La ultima de notas o libre del lateral baja hasta el pie de lo fijo.
    if let Some((i, _)) = secciones
        .iter()
        .enumerate()
        .rfind(|(_, s)| s.ancho == AnchoSeccion::Lateral)
        .filter(|(_, s)| matches!(s.tipo, TipoSeccion::Notas | TipoSeccion::Libre))
    {
        let st = &mut sitios[i];
        st.contenido = st.contenido.max(z.fondo_lateral - st.y - TITULO);
        fin = fin.max(st.y + TITULO + st.contenido);
    }
    // Sin secciones no ocupa nada: la hoja queda como si no hubiera zona.
    if sitios.is_empty() {
        fin = 0.0;
    }
    Maqueta { sitios, fin }
}

/// Los recuadros libres que no tienen nada debajo bajan hasta `fondo`.
fn estirar(z: &Zona, secciones: &[Seccion], m: &mut Maqueta, fondo: f32) {
    let sitios = m.sitios.clone();
    for (i, (s, sitio)) in secciones.iter().zip(m.sitios.iter_mut()).enumerate() {
        if !z.estira(s) {
            continue;
        }
        let tapado = sitios.iter().enumerate().any(|(j, o)| {
            j != i && o.y > sitio.y && o.x < sitio.x + sitio.w && sitio.x < o.x + o.w
        });
        if !tapado {
            sitio.contenido = sitio.contenido.max(fondo - sitio.y - TITULO);
        }
    }
}

impl Pag {
    /// **Dibuja la zona de secciones** ya colocada (`m`, de [`colocar`]),
    /// con los recuadros libres estirados hasta `fondo`.
    pub(super) fn poner_secciones(
        &mut self,
        z: &Zona,
        secciones: &[Seccion],
        mut m: Maqueta,
        fondo: f32,
    ) {
        estirar(z, secciones, &mut m, fondo);
        for (s, st) in secciones.iter().zip(&m.sitios) {
            // Cada seccion, su subgrupo con nombre: un bloque exacto.
            self.l.parte(&s.titulo);
            let y = self.seccion(st.x, st.y, st.w, &s.titulo);
            let n = s.lineas as usize;
            match (s.tipo, z.propio(&s.titulo)) {
                (TipoSeccion::Propia, Some(p)) => {
                    let h = Hueco {
                        x: st.x,
                        y,
                        w: st.w,
                        alto: st.contenido,
                    };
                    (p.dibujar)(self, h, s.lineas);
                }
                (TipoSeccion::Renglones, _) => {
                    self.renglones(st.x, y, st.w, n, z.paso);
                }
                (TipoSeccion::Numerados, _) => {
                    let desde = z
                        .serie
                        .filter(|(t, _)| normalizar(t) == normalizar(&s.titulo))
                        .map_or(1, |(_, d)| d);
                    self.numerados_desde(st.x, y, st.w, desde, n, z.paso);
                }
                (TipoSeccion::Casillas, _) => {
                    self.casillas(st.x, y, st.w, n, z.paso, &[]);
                }
                (TipoSeccion::Recuadro, _) => {
                    self.caja_suave(st.x, y + 10.0, st.w, st.contenido - 10.0, "");
                }
                (TipoSeccion::Notas, _) => {
                    // Gris claro y sin borde ni rayas: para anotar a mano.
                    let gris = super::hex(0xf1f3f6);
                    self.l.rect(
                        st.x,
                        y + 10.0,
                        st.w,
                        st.contenido - 10.0,
                        None,
                        Some(gris),
                        1.0,
                        true,
                    );
                }
                // Una propia sin bloque (no pasa: se valida antes) es libre.
                (TipoSeccion::Libre | TipoSeccion::Propia, _) => {
                    self.caja(st.x, y + 10.0, st.w, st.contenido - 10.0, "");
                }
            }
        }
        self.l.parte("");
    }
}

// ---------------------------------------------------------------------------
// Lo pedido: completar y validar
// ---------------------------------------------------------------------------

/// **La lista pedida, completa y comprobada**: lo que una seccion no diga
/// se toma de la de fabrica con su titulo (o, si es nueva, renglones, 3
/// lineas y media columna). Error en palabras para el usuario si algo no
/// vale.
pub(super) fn completar(
    nombre: &str,
    fabrica: &[Seccion],
    propios: &[Propio],
    con_lateral: bool,
    pedidas: &[SeccionPedida],
) -> Result<Vec<Seccion>, String> {
    if pedidas.is_empty() {
        return Err(if fabrica.is_empty() {
            format!("di qué secciones quieres en «{nombre}» (de serie no lleva ninguna)")
        } else {
            format!(
                "«{nombre}» necesita al menos una sección; las de ahora son: {}",
                titulos(fabrica)
            )
        });
    }
    if pedidas.len() > MAX_SECCIONES {
        return Err(format!(
            "son {} secciones; como mucho {MAX_SECCIONES} en una hoja",
            pedidas.len()
        ));
    }
    let mut v = Vec::with_capacity(pedidas.len());
    for p in pedidas {
        let titulo = p.titulo.trim();
        if titulo.is_empty() {
            return Err("hay una sección sin título: cada una necesita el suyo".into());
        }
        if titulo.chars().count() > MAX_TITULO {
            return Err(format!(
                "el título «{titulo}» es demasiado largo (como mucho {MAX_TITULO} letras)"
            ));
        }
        let t = normalizar(titulo);
        let base = fabrica.iter().find(|s| normalizar(&s.titulo) == t);
        let tipo = p
            .tipo
            .or(base.map(|s| s.tipo))
            .unwrap_or(TipoSeccion::Renglones);
        let es_propio = propios.iter().any(|b| normalizar(b.titulo) == t);
        if tipo == TipoSeccion::Propia && !es_propio {
            return Err(if propios.is_empty() {
                format!(
                    "«{titulo}» no puede ser «propia»: «{nombre}» no tiene bloques propios; usa renglones, numerados, casillas, recuadro o libre"
                )
            } else {
                format!(
                    "«{titulo}» no es un bloque propio de «{nombre}»; los suyos son: {}. Para una sección nueva usa renglones, numerados, casillas, recuadro o libre",
                    propios
                        .iter()
                        .map(|b| b.titulo)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            });
        }
        let lineas = match p.lineas {
            Some(n) if n == 0 || n > MAX_LINEAS => {
                return Err(format!(
                    "«{titulo}»: las líneas van de 1 a {MAX_LINEAS} (pediste {n})"
                ));
            }
            Some(n) => n,
            None => base.map_or(3, |s| s.lineas),
        };
        let ancho = p
            .ancho
            .or(base.map(|s| s.ancho))
            .unwrap_or(AnchoSeccion::Mitad);
        if ancho == AnchoSeccion::Lateral && !con_lateral {
            return Err(format!(
                "«{nombre}» no tiene columna lateral; usa completo, mitad, izquierda o derecha"
            ));
        }
        v.push(Seccion {
            // El titulo de fabrica si es el mismo: conserva sus tildes.
            titulo: base.map_or_else(|| titulo.to_string(), |s| s.titulo.clone()),
            tipo,
            lineas,
            ancho,
        });
    }
    Ok(v)
}

fn titulos(s: &[Seccion]) -> String {
    s.iter()
        .map(|s| s.titulo.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// **La lista en JSON**, tal como se la puede devolver el modelo cambiada.
pub(super) fn en_json(secciones: &[Seccion]) -> String {
    let v: Vec<String> = secciones
        .iter()
        .map(|s| {
            format!(
                "{{\"titulo\":\"{}\",\"tipo\":\"{}\",\"lineas\":{},\"ancho\":\"{}\"}}",
                s.titulo.replace('"', "'"),
                s.tipo.nombre(),
                s.lineas,
                s.ancho.nombre()
            )
        })
        .collect();
    format!("[{}]", v.join(","))
}
