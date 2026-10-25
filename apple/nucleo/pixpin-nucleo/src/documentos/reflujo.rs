//! **El visor reflujado de Word y libros** (tanda E7): lo que en Windows
//! hacen `visor.rs` (sin su ventana), `visor/maqueta_movil.rs` y la parte de
//! Word y libro de `anotado_del_adjunto.rs`. La ventana, el dibujo y los
//! mandos son de Swift (`Documentos/VisorReflujo.swift`); aqui se decide
//! **donde cae cada bloque** (la maqueta del movil, [`maqueta`]) y **donde
//! va lo anotado y en que unidades**, con los nombres de Windows y Android
//! para que viaje:
//!
//! | Que | Adjunto del chat | Otro documento |
//! |---|---|---|
//! | tinta | `anot-<uid>` + su marco `.hoja` (la columna) | `<doc>.pixpin-anotado/capa.excalidraw` |
//! | marcadores | `anot-<uid>.marcas` (`id:fraccion:emoji`) | `<doc>.pixpin-lectura` |
//! | columna, letra y espacios | `anot-<uid>.maqueta` (`columna,izq,der,tamano,grosor,tipo`) | `<doc>.pixpin-lectura` |
//! | por donde iba | `anot-<uid>.sitio` | `<doc>.pixpin-lectura` |
//! | el verde de la voz | `anot-<uid>.voz` (`parrafo:fraccion`) | `<doc>.pixpin-lectura` |
//! | aumento | `<doc>.pixpin-lectura` (de esta pantalla, no viaja) | igual |
//!
//! # Las unidades de la tinta
//!
//! Las del lector: pixeles de lectura a escala 1 con el cero en el borde de
//! la columna (ver [`maqueta`]). La primera vez que se anota se fija la
//! columna y con ella la letra (`columnaDeAnotar` del movil): desde que hay
//! tinta el texto no puede partirse en otros renglones. El movil cuenta
//! desde el borde de su pagina (la columna empieza en `izq`); su marco
//! (`anot-<uid>.hoja`) dice donde cae la columna en el fichero, y sin marco
//! vale la regla de antes: la columna corrida su `izq` (`corrida`). Al
//! guardar se escribe en las del lector con el marco de la columna,
//! `0,0,columna,columna`, como hace el lector de hojas con la suya: quien lo
//! lee (Windows, Android desde v0.98) encaja la columna del fichero en la
//! suya y la tinta cae en la misma palabra.
//!
//! # Rendimiento
//!
//! Abrir lee el documento una vez ([`DocumentoReflujo`]); colocar pide a
//! Swift lo que avanza cada letra **una vez por parrafo**
//! ([`MedidorReflujo`]) y se repite solo si cambian la columna o la letra.
//! Las imagenes no se descodifican aqui: su hueco sale de su cabecera.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pixpin_docs::documento::{Clase, Documento};
use pixpin_docs::{lectura, vista, voz_alta};
use pixpin_proyecto::anotado::{self as an, Base};
use pixpin_sincro::anotado::{Maqueta, MarcoDeLaHoja, margen_de};

use super::{DondeAnota, leer_marco};
use crate::editor::{EditorLienzo, HojaDoc};
use crate::unidades::Unidades;
use crate::{ErrorNucleo, Nucleo};

mod maqueta;
use maqueta::{Alineacion, Colocado, Hoja, Letra, Medida, Tinte, Tramo};

// ---------------------------------------------------------------------------
// Las letras del movil

/// Una letra incrustada del lector: la familia, si es la negrita o la
/// cursiva, y su fichero (woff2, que Core Text abre en memoria).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct LetraDeLectura {
    pub familia: String,
    pub negrita: bool,
    pub cursiva: bool,
    pub datos: Vec<u8>,
}

/// **Las letras del movil**, incrustadas como en Windows (`pixpin-render`):
/// Noto Serif y Roboto (las «serif» y «sans-serif» de Android, rectas y
/// cursivas, normal y negrita) y Caveat (la letra a mano). Courier New ya
/// viene con el Mac. Ver `letras/LICENCIAS.txt`.
#[uniffi::export]
pub fn letras_de_lectura() -> Vec<LetraDeLectura> {
    let l = |familia: &str, negrita: bool, cursiva: bool, datos: &[u8]| LetraDeLectura {
        familia: familia.into(),
        negrita,
        cursiva,
        datos: datos.to_vec(),
    };
    vec![
        l(
            "Noto Serif",
            false,
            false,
            include_bytes!("../../letras/noto-serif-400-normal.woff2"),
        ),
        l(
            "Noto Serif",
            false,
            true,
            include_bytes!("../../letras/noto-serif-400-italic.woff2"),
        ),
        l(
            "Noto Serif",
            true,
            false,
            include_bytes!("../../letras/noto-serif-700-normal.woff2"),
        ),
        l(
            "Noto Serif",
            true,
            true,
            include_bytes!("../../letras/noto-serif-700-italic.woff2"),
        ),
        l(
            "Roboto",
            false,
            false,
            include_bytes!("../../letras/roboto-400-normal.woff2"),
        ),
        l(
            "Roboto",
            false,
            true,
            include_bytes!("../../letras/roboto-400-italic.woff2"),
        ),
        l(
            "Roboto",
            true,
            false,
            include_bytes!("../../letras/roboto-700-normal.woff2"),
        ),
        l(
            "Roboto",
            true,
            true,
            include_bytes!("../../letras/roboto-700-italic.woff2"),
        ),
        l(
            "Caveat",
            false,
            false,
            include_bytes!("../../letras/caveat-500.woff2"),
        ),
        l(
            "Caveat",
            true,
            false,
            include_bytes!("../../letras/caveat-700.woff2"),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Medir (Core Text desde Swift)

/// Un tramo de un parrafo con otro estilo, en unidades UTF-16.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct TramoReflujo {
    pub inicio: u32,
    pub longitud: u32,
    pub negrita: bool,
    pub cursiva: bool,
    /// De ancho fijo (Courier New).
    pub mono: bool,
}

/// **Quien mide los parrafos**: Core Text desde Swift, con las mismas
/// letras con las que luego se pinta.
#[uniffi::export(callback_interface)]
pub trait MedidorReflujo: Send + Sync {
    /// Lo que avanza cada unidad UTF-16 de `texto` (el ancho de su racimo en
    /// la primera y cero en las demas), en puntos, con `familia` a `tam` y
    /// el peso de CSS `peso` (un tramo en negrita va a `max(peso, 700)`),
    /// con su interletrado de siempre (el `kerning` de la letra) y los
    /// tramos de `tramos` encima. Uno por unidad UTF-16.
    fn avances(
        &self,
        texto: String,
        familia: String,
        peso: u16,
        tam: f32,
        tramos: Vec<TramoReflujo>,
    ) -> Vec<f32>;
}

fn tramo_a_swift(t: &Tramo) -> TramoReflujo {
    TramoReflujo {
        inicio: t.inicio,
        longitud: t.longitud,
        negrita: t.estilo.negrita,
        cursiva: t.estilo.cursiva,
        mono: t.estilo.mono,
    }
}

// ---------------------------------------------------------------------------
// Lo colocado, como lo ve Swift

/// Que es un bloque colocado (`pixpin_docs::documento::Clase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ClaseReflujo {
    Parrafo,
    Titulo { nivel: u8 },
    Lista,
    Cita,
    Codigo,
    Regla,
    Fila,
    Nota,
    Capitulo,
}

impl From<Clase> for ClaseReflujo {
    fn from(c: Clase) -> Self {
        match c {
            Clase::Parrafo => ClaseReflujo::Parrafo,
            Clase::Titulo(nivel) => ClaseReflujo::Titulo { nivel },
            Clase::Lista => ClaseReflujo::Lista,
            Clase::Cita => ClaseReflujo::Cita,
            Clase::Codigo => ClaseReflujo::Codigo,
            Clase::Regla => ClaseReflujo::Regla,
            Clase::Fila => ClaseReflujo::Fila,
            Clase::Nota => ClaseReflujo::Nota,
            Clase::Capitulo => ClaseReflujo::Capitulo,
        }
    }
}

/// De que color va (los de la hoja del movil; Swift los pone del tono).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TinteReflujo {
    Texto,
    Apagado,
    Raya,
}

/// `text-align` del bloque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AlineacionReflujo {
    Izquierda,
    Centro,
    Derecha,
}

/// La caja de una celda de una tabla, con sus rayas.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct CajaReflujo {
    pub x: f32,
    pub ancho: f32,
    pub y: f32,
    pub alto: f32,
    /// Raya arriba (sin ella, es la continuacion de una celda unida).
    pub arriba: bool,
    /// Raya abajo (sin ella, la sigue su continuacion).
    pub abajo: bool,
    /// Con fondo (`w:shd` de la celda).
    pub relleno: bool,
}

/// **Un bloque colocado**, en unidades del lector (ver la doc del modulo).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ColocadoReflujo {
    pub texto: String,
    pub tramos: Vec<TramoReflujo>,
    pub clase: ClaseReflujo,
    pub tam: f32,
    /// Donde empieza su caja de texto desde el borde de la columna.
    pub sangria: f32,
    pub ancho: f32,
    pub y: f32,
    pub alto: f32,
    pub tinte: TinteReflujo,
    /// El bloque del documento del que sale (la cabecera de un libro no
    /// tiene).
    pub bloque: Option<u32>,
    pub caja: Option<CajaReflujo>,
    /// La letra con la que se midio: con otra, el texto se partiria en
    /// otros renglones.
    pub familia: String,
    pub peso: u16,
    pub interlineado: f32,
    pub alineacion: AlineacionReflujo,
    /// El hueco de una imagen (con su recuadro si no se puede ensenar).
    pub recuadro: bool,
    /// Que imagen del documento va en ese hueco ([`DocumentoReflujo::imagen`]).
    pub imagen: Option<u32>,
    /// Donde empieza y acaba cada renglon (UTF-16), de dos en dos: se pinta
    /// por ahi, no por donde partiria Core Text.
    pub renglones: Vec<u32>,
}

/// **El documento colocado** en una columna.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MaquetaReflujo {
    pub colocados: Vec<ColocadoReflujo>,
    /// El alto de la pagina entera.
    pub alto: f32,
    pub columna: f32,
    /// Los colocados que se leen en voz alta, en orden (cada uno un
    /// «parrafo» de la voz).
    pub para_leer: Vec<u32>,
}

fn a_swift(c: &Colocado) -> ColocadoReflujo {
    ColocadoReflujo {
        texto: c.texto.clone(),
        tramos: c.tramos.iter().map(tramo_a_swift).collect(),
        clase: c.clase.into(),
        tam: c.tam,
        sangria: c.sangria,
        ancho: c.ancho,
        y: c.y,
        alto: c.alto,
        tinte: match c.tinte {
            Tinte::Texto => TinteReflujo::Texto,
            Tinte::Apagado => TinteReflujo::Apagado,
            Tinte::Raya => TinteReflujo::Raya,
        },
        bloque: c.bloque.map(|b| b as u32),
        caja: c.caja.map(|k| CajaReflujo {
            x: k.x,
            ancho: k.ancho,
            y: k.y,
            alto: k.alto,
            arriba: k.arriba,
            abajo: k.abajo,
            relleno: k.relleno,
        }),
        familia: c.letra.familia.to_string(),
        peso: c.letra.peso,
        interlineado: c.letra.interlineado,
        alineacion: match c.letra.alineacion {
            Alineacion::Izquierda => AlineacionReflujo::Izquierda,
            Alineacion::Centro => AlineacionReflujo::Centro,
            Alineacion::Derecha => AlineacionReflujo::Derecha,
        },
        recuadro: c.recuadro,
        imagen: c.imagen.map(|n| n as u32),
        renglones: c.renglones.iter().flat_map(|(a, b)| [*a, *b]).collect(),
    }
}

/// Una entrada del indice (I): el titulo y el bloque al que lleva.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct EntradaIndiceReflujo {
    pub nivel: u8,
    pub titulo: String,
    pub bloque: u32,
}

/// Una coincidencia de buscar: en que colocado y que letras (UTF-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct CoincidenciaReflujo {
    pub colocado: u32,
    pub inicio: u32,
    pub largo: u32,
}

/// **Si el visor reflujado sabe abrir esto** (`visor::se_abre`): Word,
/// libros, paginas, Markdown y texto.
#[uniffi::export]
pub fn se_abre_en_el_visor(nombre: String) -> bool {
    pixpin_docs::formato_de(&nombre).is_some()
}

/// **Un documento abierto en el visor reflujado.**
#[derive(uniffi::Object)]
pub struct DocumentoReflujo {
    doc: Documento,
    hoja: Hoja,
    /// Lo ultimo colocado: buscar busca en lo que se pinta.
    colocados: Mutex<Vec<Colocado>>,
}

#[uniffi::export]
impl DocumentoReflujo {
    /// Lee el documento (bloqueante: Swift lo llama fuera del hilo principal).
    #[uniffi::constructor]
    pub fn abrir(ruta: String) -> Result<Arc<DocumentoReflujo>, ErrorNucleo> {
        let doc = pixpin_docs::abrir(Path::new(&ruta)).map_err(|e| ErrorNucleo::Argumento {
            detalle: format!("no se abre en el visor: {e}"),
        })?;
        Ok(Arc::new(DocumentoReflujo {
            hoja: Hoja::de(&pixpin_docs::nombre(Path::new(&ruta))),
            doc,
            colocados: Mutex::new(Vec::new()),
        }))
    }

    /// El titulo del documento (el de un libro; si no, vacio).
    pub fn titulo(&self) -> String {
        self.doc.titulo.trim().to_string()
    }

    /// Si se coloca con la hoja del libro (`EpubAHtml.ESTILO`).
    pub fn es_libro(&self) -> bool {
        self.hoja == Hoja::Libro
    }

    /// El indice (los titulos y los capitulos), en el orden del documento.
    pub fn indice(&self) -> Vec<EntradaIndiceReflujo> {
        pixpin_docs::indice::de(&self.doc)
            .into_iter()
            .map(|e| EntradaIndiceReflujo {
                nivel: e.nivel,
                titulo: e.titulo,
                bloque: e.bloque as u32,
            })
            .collect()
    }

    /// Las palabras del documento (la pagina que sube sola).
    pub fn palabras(&self) -> u64 {
        self.doc
            .bloques
            .iter()
            .map(|b| pixpin_docs::auto_desplazar::contar_palabras(&b.texto()) as u64)
            .sum()
    }

    /// Los bytes de la imagen `n` del documento (se descodifican al verse).
    pub fn imagen(&self, n: u32) -> Option<Vec<u8>> {
        self.doc.imagenes.get(n as usize).map(|i| i.datos.clone())
    }

    /// **Coloca el documento como el movil** en una columna de `columna`
    /// pixeles de lectura, con la letra `tamano` (tanto por ciento), `tipo`
    /// y `grosor` (los cuatro del movil). Bloqueante: fuera del hilo
    /// principal.
    pub fn colocar(
        &self,
        columna: f32,
        tamano: u32,
        tipo: u8,
        grosor: u8,
        medidor: Box<dyn MedidorReflujo>,
    ) -> MaquetaReflujo {
        let columna = if columna.is_finite() {
            columna.max(1.0)
        } else {
            1.0
        };
        let ajustes = lectura::Ajustes {
            tamano: lectura::tamano_valido(tamano),
            tipo: tipo.min(lectura::TIPOS as u8 - 1),
            grosor: grosor.min(lectura::GROSORES as u8 - 1),
            columna: columna as u32,
            ..lectura::Ajustes::default()
        };
        let mide = |texto: &str, tam: f32, ancho: f32, tramos: &[Tramo], letra: &Letra| -> Medida {
            let avances = medidor.avances(
                texto.to_string(),
                letra.familia.to_string(),
                letra.peso,
                tam,
                tramos.iter().map(tramo_a_swift).collect(),
            );
            maqueta::medir_con_avances(texto, &avances, tam, ancho, letra)
        };
        let (colocados, alto) = maqueta::colocar(&self.doc, &ajustes, columna, self.hoja, &mide);
        let salida = MaquetaReflujo {
            colocados: colocados.iter().map(a_swift).collect(),
            alto,
            columna,
            para_leer: maqueta::para_leer(&colocados)
                .into_iter()
                .map(|i| i as u32)
                .collect(),
        };
        *self.colocados.lock().unwrap_or_else(|e| e.into_inner()) = colocados;
        salida
    }

    /// **Busca** en lo ultimo colocado (con la vineta de las listas y todo),
    /// sin mirar mayusculas ni tildes, en orden de lectura.
    pub fn buscar(&self, consulta: String) -> Vec<CoincidenciaReflujo> {
        let c = self.colocados.lock().unwrap_or_else(|e| e.into_inner());
        pixpin_docs::buscar::buscar(c.iter().map(|x| x.texto.as_str()), &consulta)
            .into_iter()
            .filter_map(|k| {
                let texto = &c.get(k.texto)?.texto;
                let inicio = pixpin_docs::buscar::a_utf16(texto, k.inicio);
                let fin = pixpin_docs::buscar::a_utf16(texto, k.inicio + k.largo);
                Some(CoincidenciaReflujo {
                    colocado: k.texto as u32,
                    inicio: inicio as u32,
                    largo: (fin - inicio) as u32,
                })
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Las cuentas del lector

/// La columna de lectura: ni tan ancha que se pierda el renglon ni tan
/// estrecha que se corte. Las 46 «emes» de la hoja del movil.
const COLUMNA_MAXIMA: f32 = 860.0;
const COLUMNA_MINIMA: f32 = 240.0;

/// **La columna que cabe** en una ventana de `ancho` puntos (`visor::
/// columna_que_cabe`): la que se usa mientras no haya tinta.
#[uniffi::export]
pub fn columna_de_lectura_que_cabe(ancho: f32) -> f32 {
    if !ancho.is_finite() {
        return COLUMNA_MAXIMA;
    }
    (ancho - 96.0).clamp(COLUMNA_MINIMA, COLUMNA_MAXIMA)
}

/// **El espacio en blanco de cada lado de la columna** (`visor::lados`):
/// el puesto con los mandos (`izq`, `der` de la maqueta) o, si nunca se
/// toco, los dos tercios de siempre. `[izq, der]`.
#[uniffi::export]
pub fn lados_de_la_columna(lectura: LecturaReflujo, columna: f32) -> Vec<f32> {
    match lectura.lados {
        Some(l) if lectura.columna > 0 => vec![l.izq as f32, l.der as f32],
        _ => {
            let m = vista::margen_de(columna);
            vec![m, m]
        }
    }
}

/// **Un paso mas o uno menos de espacio a un lado** (`anadirEspacio` del
/// movil): un tercio de la columna, dentro de lo que cabe. Devuelve la
/// lectura nueva, o `None` si ya estaba en el tope (o en cero). Sin
/// columna fijada se fija aqui, sin espacio a ningun lado salvo el pedido.
#[uniffi::export]
pub fn otro_espacio_de_lado(
    lectura: LecturaReflujo,
    columna_medida: f32,
    izquierda: bool,
    mas: bool,
) -> Option<LecturaReflujo> {
    let mut l = lectura;
    if l.columna == 0 {
        l.columna = columna_medida.round().max(1.0) as u32;
        l.lados = Some(LadosReflujo { izq: 0, der: 0 });
    }
    let c = l.columna;
    let m = c * 2 / 3;
    let (i, d) = l.lados.map(|x| (x.izq, x.der)).unwrap_or((m, m));
    let paso = i64::from(vista::paso_de_espacio(c)) * if mas { 1 } else { -1 };
    let antes = if izquierda { i } else { d };
    let ahora = vista::espacio_valido(i64::from(antes) + paso, c);
    if ahora == antes {
        return None;
    }
    l.lados = Some(if izquierda {
        LadosReflujo { izq: ahora, der: d }
    } else {
        LadosReflujo { izq: i, der: ahora }
    });
    Some(l)
}

/// El siguiente tamano de la letra (+ / −), de la barra de puntos del movil.
#[uniffi::export]
pub fn tamano_de_letra_vecino(actual: u32, mas: bool) -> u32 {
    lectura::tamano_vecino(actual, mas)
}

/// Lo mas que se aleja (`vista::zoom_minimo`): hasta ver entero lo que mide
/// `ancho_doc` en `vista` puntos, y nunca por encima de 1.
#[uniffi::export]
pub fn aumento_minimo_de_lectura(ancho_doc: f32, vista: f32) -> f32 {
    vista::zoom_minimo(ancho_doc, vista, 1.0)
}

/// El aumento mas grande del lector.
#[uniffi::export]
pub fn aumento_maximo_de_lectura() -> f32 {
    vista::ZOOM_MAXIMO
}

/// Un marcador mas, **en el orden del documento** (`lectura::con_marcador`:
/// el mismo emoticono muy cerca se reemplaza; mas de 24, fuera el mas
/// viejo).
#[uniffi::export]
pub fn con_marcador_de_lectura(
    marcadores: Vec<MarcadorReflujo>,
    fraccion: f32,
    emoji: String,
    ahora: u64,
) -> Vec<MarcadorReflujo> {
    let l: Vec<lectura::Marcador> = marcadores.into_iter().map(marcador_de_swift).collect();
    lectura::con_marcador(&l, fraccion, &emoji, ahora)
        .into_iter()
        .map(marcador_a_swift)
        .collect()
}

/// **Por que parrafo se sigue escuchando** (`voz_alta::parrafo_de_la_marca`):
/// el del numero del verde si su fraccion cae cerca (el mismo documento
/// contado igual); si no, el que empieza mas cerca de esa fraccion.
/// `fracciones`: donde empieza cada parrafo de la voz, de 0 a 1.
#[uniffi::export]
pub fn parrafo_del_verde(fracciones: Vec<f32>, verde: VozReflujo) -> Option<u32> {
    voz_alta::parrafo_de_la_marca(&fracciones, (verde.parrafo as usize, verde.fraccion))
        .map(|p| p as u32)
}

// ---------------------------------------------------------------------------
// Lo que se recuerda de un documento (`anotado_del_adjunto::leer/escribir`)

/// El espacio en blanco de cada lado de la columna, en pixeles de lectura.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct LadosReflujo {
    pub izq: u32,
    pub der: u32,
}

/// El marcador verde de la voz: el parrafo y la fraccion donde se dejo.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct VozReflujo {
    pub parrafo: u32,
    pub fraccion: f32,
}

/// Un marcador de un Word o un libro: la fraccion del documento.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MarcadorReflujo {
    pub id: u64,
    pub fraccion: f32,
    pub emoji: String,
}

fn marcador_de_swift(m: MarcadorReflujo) -> lectura::Marcador {
    lectura::Marcador {
        id: m.id,
        fraccion: if m.fraccion.is_finite() {
            m.fraccion.clamp(0.0, 1.0)
        } else {
            0.0
        },
        emoji: m.emoji.replace(['|', ':', '\n', '\r'], ""),
    }
}

fn marcador_a_swift(m: lectura::Marcador) -> MarcadorReflujo {
    MarcadorReflujo {
        id: m.id,
        fraccion: m.fraccion,
        emoji: m.emoji,
    }
}

/// **Lo que se recuerda de un Word o un libro**: lo que viaja (columna,
/// letra, espacios, marcadores, por donde iba y el verde) y lo de esta
/// pantalla (el aumento).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct LecturaReflujo {
    /// La letra en tanto por ciento (70..250).
    pub tamano: u32,
    /// Una de las cuatro letras del movil (0 serif, 1 sans, 2 ancho fijo, 3 a mano).
    pub tipo: u8,
    /// Uno de los cuatro grosores (0 fina, 1 normal, 2 gruesa, 3 negra).
    pub grosor: u8,
    /// **La columna fijada al anotar**; 0 si nunca se anoto (la letra se
    /// puede cambiar).
    pub columna: u32,
    /// Los espacios de cada lado (solo con la columna fijada); `None`: los
    /// dos tercios de siempre.
    pub lados: Option<LadosReflujo>,
    /// El candado de los lados: la vista solo sube y baja.
    pub sin_lado: bool,
    /// Por donde se iba, de 0 a 1.
    pub sitio: f32,
    /// El aumento propio (1 = la columna a su tamano).
    pub zoom: f32,
    pub marcadores: Vec<MarcadorReflujo>,
    pub voz: Option<VozReflujo>,
    pub donde: DondeAnota,
}

/// La base de lo anotado de `doc` si es un adjunto del chat (y no un PDF).
fn base_de(raiz: &Path, doc: &Path) -> Option<Base> {
    if super::es_pdf(doc) {
        return None;
    }
    an::base_del_documento(raiz, doc)
}

/// El grosor y la letra del movil, los mismos del PC desde K16; lo que no
/// existe, al extremo mas cercano.
fn grosor_del_movil(g: i32) -> u8 {
    g.clamp(0, lectura::GROSORES as i32 - 1) as u8
}

fn tipo_del_movil(t: i32) -> u8 {
    t.clamp(0, lectura::TIPOS as i32 - 1) as u8
}

/// La maqueta que corresponde a lo fijado aqui, conservando del movil lo
/// que el Mac no sabe decir si la columna no cambio. `None` si lo que hay
/// ya dice eso mismo (no se reescribe).
fn maqueta_para(a: &lectura::Ajustes, actual: Option<Maqueta>) -> Option<Maqueta> {
    let columna = a.columna as i32;
    if let Some(m) = actual
        && m.columna == columna
        && lectura::tamano_valido(m.tamano.max(0) as u32) == a.tamano
        && grosor_del_movil(m.grosor) == a.grosor
        && tipo_del_movil(m.tipo) == a.tipo
        && a.lados
            .is_none_or(|(i, d)| (i as i32, d as i32) == (m.izq, m.der))
    {
        return None;
    }
    // Los espacios de cada lado: los puestos con los mandos; si no, los del
    // movil con esta misma columna, o los dos tercios de siempre.
    let (izq, der) = match (a.lados, actual) {
        (Some((i, d)), _) => (i as i32, d as i32),
        (None, Some(m)) if m.columna == columna => (m.izq, m.der),
        _ => (margen_de(columna), margen_de(columna)),
    };
    Some(Maqueta {
        columna,
        izq,
        der,
        tamano: a.tamano as i32,
        grosor: i32::from(a.grosor.min(lectura::GROSORES as u8 - 1)),
        tipo: i32::from(a.tipo.min(lectura::TIPOS as u8 - 1)),
    })
}

/// Los marcadores como los escribe `Lectura.aTexto`.
fn marcadores_a_texto(a: &lectura::Ajustes) -> String {
    a.marcadores
        .iter()
        .map(|m| format!("{}:{}:{}", m.id, m.fraccion, m.emoji))
        .collect::<Vec<_>>()
        .join("|")
}

/// Lo de antes a los ficheros del mensaje, si aun no los tiene (`migrar`).
fn migrar(b: &Base, a: &lectura::Ajustes) {
    let marcas = (!a.marcadores.is_empty()).then(|| marcadores_a_texto(a));
    an::poner_si_falta(&b.fichero(".marcas"), marcas.as_deref());
    let maqueta = (a.columna > 0)
        .then(|| maqueta_para(a, None).map(|m| m.a_texto()))
        .flatten();
    an::poner_si_falta(&b.fichero(".maqueta"), maqueta.as_deref());
    an::poner_si_falta(
        &b.fichero(".sitio"),
        (a.sitio > 0.0).then(|| a.sitio.to_string()).as_deref(),
    );
    an::poner_si_falta(
        &b.fichero(".voz"),
        a.voz.map(|(p, f)| voz_alta::voz_a_texto(p, f)).as_deref(),
    );
}

/// Lo del mensaje encima de lo de aqui: manda lo que viaja.
fn poner_lo_del_mensaje(b: &Base, a: &mut lectura::Ajustes) {
    let leer = |t: &str| an::leer(&b.fichero(t));
    if let Some(t) = leer(".marcas") {
        a.marcadores = lectura::marcadores_de_texto(t.trim());
    }
    if let Some(t) = leer(".maqueta") {
        match Maqueta::de_texto(&t) {
            Some(m) => {
                a.columna = m.columna.clamp(0, 20_000) as u32;
                a.tamano = lectura::tamano_valido(m.tamano.max(0) as u32);
                a.grosor = grosor_del_movil(m.grosor);
                a.tipo = tipo_del_movil(m.tipo);
                // Los espacios de cada lado, dentro de lo que cabe
                // (`Lectura.espacioValido` al leer la maqueta).
                a.lados = Some((
                    vista::espacio_valido(i64::from(m.izq), a.columna),
                    vista::espacio_valido(i64::from(m.der), a.columna),
                ));
            }
            // Vacia: la columna se solto (al quitar toda la tinta).
            None => {
                a.columna = 0;
                a.lados = None;
            }
        }
    }
    // El marcador verde de la voz: vacio es sin verde.
    if let Some(t) = leer(".voz") {
        a.voz = voz_alta::voz_de_texto(&t);
    }
    if let Some(s) = leer(".sitio")
        .and_then(|t| t.trim().parse::<f32>().ok())
        .filter(|s| s.is_finite())
    {
        a.sitio = s.clamp(0.0, 1.0);
    }
}

/// **Lo que se recuerda de un documento**: lo de junto al documento y, si
/// es un adjunto del chat, lo que viaja encima (la primera vez, lo de aqui
/// pasa al mensaje).
pub(crate) fn leer_ajustes(raiz: &Path, doc: &Path) -> lectura::Ajustes {
    let mut a = lectura::leer(doc);
    if let Some(b) = base_de(raiz, doc) {
        migrar(&b, &a);
        poner_lo_del_mensaje(&b, &mut a);
    }
    a
}

/// **Guarda lo que se recuerda**: junto al documento, como siempre, y lo
/// que viaja en los ficheros de su mensaje. Solo se escribe lo que cambia
/// (cada fecha nueva es algo que mandar).
pub(crate) fn escribir_ajustes(
    raiz: &Path,
    doc: &Path,
    a: &lectura::Ajustes,
) -> std::io::Result<()> {
    let junto = lectura::escribir(doc, a);
    if let Some(b) = base_de(raiz, doc) {
        escribir_lo_del_mensaje(&b, a)?;
    }
    junto
}

fn escribir_lo_del_mensaje(b: &Base, a: &lectura::Ajustes) -> std::io::Result<()> {
    // Lo que no esta y no dice nada no se crea: un fichero vacio mas en el
    // otro aparato por cada documento que solo se abrio.
    let poner = |t: &str, texto: String, nada: bool| -> std::io::Result<()> {
        let f = b.fichero(t);
        if nada && !f.exists() {
            return Ok(());
        }
        an::escribir(&f, &texto)
    };
    let marcas = marcadores_a_texto(a);
    let f = b.fichero(".marcas");
    if an::leer(&f).is_none_or(|t| t.trim() != marcas) {
        poner(".marcas", marcas.clone(), marcas.is_empty())?;
    }
    let f = b.fichero(".maqueta");
    let actual = an::leer(&f).and_then(|t| Maqueta::de_texto(&t));
    if a.columna == 0 {
        // Sin columna, vacia y no borrada: borrada seria «no se sabe».
        if actual.is_some() {
            an::escribir(&f, "")?;
        }
    } else if let Some(m) = maqueta_para(a, actual) {
        an::escribir(&f, &m.a_texto())?;
    }
    let f = b.fichero(".sitio");
    let igual = an::leer(&f).and_then(|t| t.trim().parse::<f32>().ok()) == Some(a.sitio);
    if !igual {
        poner(".sitio", a.sitio.to_string(), a.sitio <= 0.0)?;
    }
    // El verde: lo de ahora, o vacio si se quito (no se borra, como el movil).
    let f = b.fichero(".voz");
    let ahora = a
        .voz
        .map(|(p, fr)| voz_alta::voz_a_texto(p, fr))
        .unwrap_or_default();
    if an::leer(&f).and_then(|t| voz_alta::voz_de_texto(&t)) != a.voz {
        poner(".voz", ahora, a.voz.is_none())?;
    }
    Ok(())
}

fn lectura_de(a: &lectura::Ajustes, donde: DondeAnota) -> LecturaReflujo {
    LecturaReflujo {
        tamano: lectura::tamano_valido(a.tamano),
        tipo: a.tipo.min(lectura::TIPOS as u8 - 1),
        grosor: a.grosor.min(lectura::GROSORES as u8 - 1),
        columna: a.columna,
        lados: a.lados.map(|(izq, der)| LadosReflujo { izq, der }),
        sin_lado: a.sin_lado,
        sitio: if a.sitio.is_finite() {
            a.sitio.clamp(0.0, 1.0)
        } else {
            0.0
        },
        zoom: if a.zoom.is_finite() && a.zoom > 0.0 {
            a.zoom
        } else {
            1.0
        },
        marcadores: a.marcadores.iter().cloned().map(marcador_a_swift).collect(),
        voz: a.voz.map(|(p, f)| VozReflujo {
            parrafo: p as u32,
            fraccion: f,
        }),
        donde,
    }
}

/// Pone en `a` lo que dice `l` (lo demas, como estaba).
fn poner_lectura(a: &mut lectura::Ajustes, l: &LecturaReflujo) {
    a.tamano = lectura::tamano_valido(l.tamano);
    a.tipo = l.tipo.min(lectura::TIPOS as u8 - 1);
    a.grosor = l.grosor.min(lectura::GROSORES as u8 - 1);
    a.columna = l.columna.min(20_000);
    a.lados = if a.columna > 0 {
        l.lados.map(|x| {
            (
                vista::espacio_valido(i64::from(x.izq), a.columna),
                vista::espacio_valido(i64::from(x.der), a.columna),
            )
        })
    } else {
        None
    };
    a.sin_lado = l.sin_lado;
    if l.sitio.is_finite() {
        a.sitio = l.sitio.clamp(0.0, 1.0);
    }
    if l.zoom.is_finite() && l.zoom > 0.0 {
        a.zoom = l.zoom;
    }
    let mut marcadores: Vec<lectura::Marcador> = l
        .marcadores
        .iter()
        .cloned()
        .map(marcador_de_swift)
        .collect();
    // El verde no es un marcador: va en `.voz`.
    marcadores.retain(|m| m.emoji != voz_alta::EMOJI_DE_VOZ);
    marcadores.truncate(lectura::MARCADORES);
    a.marcadores = marcadores;
    a.voz = l
        .voz
        .filter(|v| v.fraccion.is_finite())
        .map(|v| (v.parrafo as usize, v.fraccion.clamp(0.0, 1.0)));
}

// ---------------------------------------------------------------------------
// La tinta

/// `<doc>.pixpin-anotado/capa.excalidraw` (`lector_tinta::ruta_de_capa`).
fn capa_junto_al_documento(doc: &Path) -> PathBuf {
    let mut nombre = doc
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "documento".into());
    nombre.push_str(".pixpin-anotado");
    doc.with_file_name(nombre).join("capa.excalidraw")
}

/// Cuanto se corre la tinta del movil sin marco: su margen izquierdo
/// (`izq`), o el que se le pondria a esta columna si aun no hay maqueta.
fn corrida(b: &Base, a: &lectura::Ajustes) -> f32 {
    an::leer(&b.fichero(".maqueta"))
        .and_then(|t| Maqueta::de_texto(&t))
        .map(|m| m.izq)
        .unwrap_or_else(|| margen_de(a.columna as i32)) as f32
}

/// **La columna como marco** (`MarcoDeLaHoja`): la banda de la columna desde
/// lo alto del documento, tan alta como ancha, en unidades del lector.
/// `None` sin columna fijada: sin ella no hay hoja con que encajar.
fn columna_propia(a: &lectura::Ajustes) -> Option<MarcoDeLaHoja> {
    let c = a.columna as f32;
    (c > 0.0).then(|| MarcoDeLaHoja::nuevo(0.0, 0.0, c, c))
}

/// **Abre la tinta** de un Word o un libro: la del mensaje si es un adjunto
/// del chat (en las unidades de su marco o, sin el, la columna corrida su
/// `izq`), y si no la de junto al documento. A la del chat sin marco se le
/// apunta el de la regla de antes (`marco_de_la_tinta`): asi un `izq` nuevo
/// no la corre.
pub(crate) fn abrir_tinta(raiz: &Path, doc: &Path) -> Result<EditorLienzo, ErrorNucleo> {
    let a = leer_ajustes(raiz, doc);
    let Some(propia) = columna_propia(&a) else {
        return Err(ErrorNucleo::Argumento {
            detalle: "sin columna fijada no se anota (se fija al empezar a anotar)".into(),
        });
    };
    let Some(b) = base_de(raiz, doc) else {
        return EditorLienzo::de_hoja(
            capa_junto_al_documento(doc),
            Unidades::DEL_PC,
            HojaDoc {
                marco: None,
                registrar: None,
            },
        );
    };
    let del_marco = leer_marco(&b).and_then(|m| Unidades::del_marco(&propia, &m));
    let con_marco = del_marco.is_some();
    let u = del_marco.unwrap_or_else(|| Unidades::uniformes(1.0, corrida(&b, &a)));
    let ed = EditorLienzo::de_hoja(
        b.tinta(),
        u,
        HojaDoc {
            marco: Some((b.clone(), propia)),
            registrar: None,
        },
    )?;
    if !con_marco
        && ed.cuantos_elementos() > 0
        && an::leer(&b.marco()).is_none_or(|t| MarcoDeLaHoja::de_texto_con_huella(&t).is_some())
    {
        b.escribir_marco(&u.marco_de(&propia))?;
    }
    Ok(ed)
}

#[uniffi::export]
impl Nucleo {
    /// **Lo que se recuerda de un Word o un libro** (`documento`: su ruta en
    /// este Mac) y adonde va lo anotado.
    pub fn lectura_de_reflujo(&self, documento: String) -> LecturaReflujo {
        let doc = PathBuf::from(&documento);
        let a = leer_ajustes(self.ruta(), &doc);
        let donde = if base_de(self.ruta(), &doc).is_some() {
            DondeAnota::Chat
        } else {
            DondeAnota::JuntoAlDocumento
        };
        lectura_de(&a, donde)
    }

    /// **Guarda lo que se recuerda**: el aumento junto al documento (de esta
    /// pantalla) y lo demas tambien donde viaja, si es un adjunto del chat.
    /// Solo se escribe lo que cambia. Bloqueante (ficheros pequenos).
    pub fn guardar_lectura_de_reflujo(
        &self,
        documento: String,
        lectura: LecturaReflujo,
    ) -> Result<(), ErrorNucleo> {
        let doc = PathBuf::from(&documento);
        let mut a = leer_ajustes(self.ruta(), &doc);
        poner_lectura(&mut a, &lectura);
        // Ya se escribe en el formato de ahora.
        a.de_antes = false;
        escribir_ajustes(self.ruta(), &doc, &a)?;
        Ok(())
    }

    /// **Abre la tinta** de un Word o un libro para verla y anotarla, en las
    /// unidades del lector (ver la doc del modulo). Hace falta la columna
    /// fijada ([`LecturaReflujo::columna`], que Swift fija y guarda al
    /// empezar a anotar). Si no hay tinta, nace vacia y no se escribe nada
    /// hasta que se anote. Guardar con [`EditorLienzo::guardar`] (fuera del
    /// hilo principal) escribe la tinta y su marco.
    pub fn anotar_reflujo(&self, documento: String) -> Result<Arc<EditorLienzo>, ErrorNucleo> {
        Ok(Arc::new(abrir_tinta(self.ruta(), Path::new(&documento))?))
    }

    /// **Donde viaja lo de un adjunto** (Windows, 20-oct): el fichero
    /// `anot-<uid><terminacion>` de `documento` si es un adjunto de un chat
    /// (`.marcos.json` o `.vistas.json`), o `None` si no lo es (entonces se
    /// queda junto al fichero, sin viajar).
    pub fn ruta_anotada_del_adjunto(&self, documento: String, terminacion: String) -> Option<String> {
        if terminacion != pixpin_sincro::anotado::MARCOS && terminacion != pixpin_sincro::anotado::VISTAS {
            return None;
        }
        let b = an::base_del_documento(self.ruta(), Path::new(&documento))?;
        let f = b.fichero(&terminacion);
        if let Some(c) = f.parent() {
            let _ = std::fs::create_dir_all(c);
        }
        Some(f.display().to_string())
    }

    /// **La tinta de una pagina web o de una nota** (Windows,
    /// `tinta_de_la_pagina`): de un adjunto, `anot-<uid>` (la que viaja), en
    /// pixeles CSS del documento, sin marco ni la regla de columnas del Word;
    /// si no, `<doc>.pixpin-anotado/capa.excalidraw`.
    pub fn ruta_de_tinta_de_pagina(&self, documento: String) -> String {
        let doc = PathBuf::from(&documento);
        let r = match an::base_del_documento(self.ruta(), &doc).filter(|_| !super::es_pdf(&doc)) {
            Some(b) => b.tinta(),
            None => PathBuf::from(format!("{documento}.pixpin-anotado")).join("capa.excalidraw"),
        };
        if let Some(c) = r.parent() {
            let _ = std::fs::create_dir_all(c);
        }
        r.display().to_string()
    }

    /// **La tinta de la hoja `hoja` (desde 0) de un libro de calculo**
    /// (Windows, `tinta_de_la_hoja_del_libro`): `anot-<uid>-h<i>`, en
    /// pixeles de la hoja al 100 %, sin `.hoja`.
    pub fn ruta_de_tinta_de_hoja(&self, libro: String, hoja: u32) -> String {
        let doc = PathBuf::from(&libro);
        let r = match an::base_del_documento(self.ruta(), &doc) {
            Some(b) => Base { base: format!("{}-h{hoja}", b.base), ..b }.tinta(),
            None => PathBuf::from(format!("{libro}.pixpin-anotado")).join(format!("hoja-{}.excalidraw", hoja + 1)),
        };
        if let Some(c) = r.parent() {
            let _ = std::fs::create_dir_all(c);
        }
        r.display().to_string()
    }

    /// **La tinta de un marcador de un modelo 3D** (Windows,
    /// `plano_cad::capa_de_marcador`): `anot-<uid>-m<id>.excalidraw.gz` con su
    /// `.hoja` (`-2500,-2500,2500,2500`), en la pantalla desde el centro y en
    /// milesimas de su alto. Fuera de un chat, junto al modelo.
    pub fn ruta_de_tinta_de_marcador3d(&self, modelo: String, id: String) -> String {
        let doc = PathBuf::from(&modelo);
        let limpio: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
        let r = match an::base_del_documento(self.ruta(), &doc) {
            Some(b) => {
                let b = Base { base: format!("{}-m{limpio}", b.base), ..b };
                let hoja = b.marco();
                if !hoja.is_file() {
                    if let Some(c) = hoja.parent() {
                        let _ = std::fs::create_dir_all(c);
                    }
                    let _ = std::fs::write(&hoja, pixpin_sincro::anotado::MarcoDeLaHoja::nuevo(-2500.0, -2500.0, 2500.0, 2500.0).a_texto());
                }
                b.tinta()
            }
            None => PathBuf::from(format!("{modelo}.pixpin-anotado")).join(format!("marcador-{limpio}.excalidraw")),
        };
        if let Some(c) = r.parent() {
            let _ = std::fs::create_dir_all(c);
        }
        r.display().to_string()
    }

    /// Si `documento` ya tiene tinta (sin leerla: solo si su fichero esta).
    pub fn hay_tinta_de_reflujo(&self, documento: String) -> bool {
        let doc = PathBuf::from(&documento);
        match base_de(self.ruta(), &doc) {
            Some(b) => b.tinta().is_file(),
            None => capa_junto_al_documento(&doc).is_file(),
        }
    }
}

#[cfg(test)]
#[path = "reflujo/pruebas.rs"]
mod pruebas;
