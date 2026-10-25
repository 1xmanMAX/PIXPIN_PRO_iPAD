//! **Lo anotado sobre un adjunto del chat, nombrado por su mensaje**
//! (`sincro/AnotacionesDelAdjunto.kt` de PixPin Android v0.96, 28-sep-2026).
//!
//! La tinta de un PDF suelto, la de un Word o un libro, sus marcadores y los
//! espacios para anotar se guardaban con la ruta del documento, que es de un
//! aparato: en el otro el mismo PDF vive en otra. Ahora van en `pins/draw/`
//! con el **codigo unico del mensaje** (`Codigos.unico`, [`crate::kotlin::unico`]),
//! que es el mismo en los dos aparatos, y `alcance` los manda con su mensaje:
//!
//! - `anot-<uid>-p<n>.excalidraw.gz`: la tinta de la pagina `n` (desde 0) de un PDF.
//! - `anot-<uid>.excalidraw.gz`: la tinta de un Word o un libro.
//! - `anot-<uid>-texto…`: lo de un PDF leido como texto (otro dibujo, otra columna).
//! - `anot-<uid>.marcas`: los marcadores, el mismo texto de siempre.
//! - `anot-<uid>.espacios`: los espacios del PDF, `0`..`3`.
//! - `anot-<uid>.maqueta`: la columna fijada de un Word o un libro
//!   (`columna,izq,der,tamano,grosor,tipo`).
//! - `anot-<uid>.voz`: el marcador verde de la voz (`parrafo:fraccion`).
//! - `anot-<uid>.sitio`: por donde se iba leyendo (fraccion del alto).
//! - `anot-<uid>-p<n>.hoja`, `anot-<uid>.hoja`: **el marco de la tinta**
//!   (29-sep), donde esta la hoja en las unidades en que se escribieron los
//!   puntos de la tinta hermana, con la huella de esa tinta ([`huella_de_tinta`]).
//!   Ver [`MarcoDeLaHoja`].
//! - `anot-<uid>.comentarios.json`: **los comentarios de una nota Markdown**
//!   (30-sep): de una nota del chat (`NOTA`), de una hoja `nota` del proyecto
//!   (con el codigo de la hoja) o de un `.md` adjunto. El formato es el de
//!   `pixpin_docs::md_comentarios`. Viajan con su nota y se van con ella
//!   ([`lleva_anotado`], [`hojas_quitadas`]).
//! - `anot-<uid>.asistente.json`: lo que penso el asistente al escribir esa
//!   nota y su hoja de trabajo (13-oct, [`ASISTENTE`]).
//!
//! El PDF de un proyecto lleva su tinta en las hojas (ya viajan); sus
//! marcadores y espacios van aqui con el codigo **del proyecto**. Y los
//! marcadores de un lienzo van junto a su dibujo, `<dibujo>.marcas`, con la
//! vista de cada uno en `<dibujo>.vistas.json` ([`vistas_junto_a`]).
//!
//! Aqui solo va lo que es igual en los dos aparatos (los nombres y el texto
//! de la maqueta); donde cae cada fichero en el disco lo dice cada `Disco`.

use std::collections::BTreeMap;

use crate::canonico::Json;
use crate::kotlin;

/// Donde viven, en rutas portatiles.
pub const CARPETA: &str = "pins/draw";
pub const PREFIJO: &str = "anot-";

/// Las terminaciones de lo anotado (`TERMINACIONES`): ni temporales ni otra cosa.
pub const TERMINACIONES: [&str; 11] = [
    ".excalidraw.gz",
    ".marcas",
    ".espacios",
    ".maqueta",
    ".voz",
    ".sitio",
    ".hoja",
    COMENTARIOS,
    ASISTENTE,
    MARCOS,
    VISTAS,
];

/// Los marcos numerados de un documento (`anot-<uid>.marcos.json`, 20-oct,
/// `marcos_doc`): la lista `[{caja, parte, vista}]` en las coordenadas del
/// documento. Ver `docs/sincronizar-marcos-vistas-y-tinta.md`.
pub const MARCOS: &str = ".marcos.json";

/// Lo que guarda cada marcador ademas de su sitio (`anot-<uid>.vistas.json`,
/// 20-oct): el zoom, o la camara y el estado de un plano o un modelo 3D.
pub const VISTAS: &str = ".vistas.json";

/// La terminacion del marco de la tinta ([`MarcoDeLaHoja`]).
pub const HOJA: &str = ".hoja";

/// La terminacion de los comentarios de una nota (`anot-<uid>.comentarios.json`).
pub const COMENTARIOS: &str = ".comentarios.json";

/// La terminacion de lo que penso el asistente al escribir una respuesta y
/// su hoja de trabajo (`anot-<uid>.asistente.json`, 13-oct, solo PC por
/// ahora: Android aun no tiene asistente). Viaja con su nota como los
/// comentarios.
pub const ASISTENTE: &str = ".asistente.json";

/// **Si lo anotado con el codigo de un mensaje es suyo**: viaja con el y se
/// borra con el. Un adjunto (tiene `ruta`: su tinta, sus marcadores...) y,
/// desde el 30-sep, una nota escrita (`NOTA`: sus comentarios).
pub fn lleva_anotado(m: &Json) -> bool {
    kotlin::cadena(m, "ruta").is_some() || kotlin::cadena(m, "clase") == Some("NOTA")
}

/// **Los codigos de las hojas que ya no estan**: las de `antes` que faltan
/// en `despues` (dos versiones del mismo proyecto portatil). Lo anotado con
/// esos codigos (los comentarios de una hoja `nota`) se va con la hoja.
pub fn hojas_quitadas(antes: Option<&Json>, despues: &Json) -> Vec<String> {
    let codigos = |p: &Json| -> Vec<String> {
        p.como_objeto()
            .and_then(|o| o.obtener("hojas"))
            .and_then(Json::como_lista)
            .map(|l| l.iter().map(kotlin::unico_de_hoja).collect())
            .unwrap_or_default()
    };
    let quedan = codigos(despues);
    antes
        .map(codigos)
        .unwrap_or_default()
        .into_iter()
        .filter(|c| !quedan.contains(c))
        .collect()
}

/// La base de la tinta de la pagina `pagina` (desde 0) de un PDF.
pub fn de_pagina(uid: &str, pagina: u32) -> String {
    format!("{PREFIJO}{uid}-p{pagina}")
}

/// La base de lo de un PDF (marcadores y espacios).
pub fn del_pdf(uid: &str) -> String {
    format!("{PREFIJO}{uid}")
}

/// La base de un Word, un libro, o un PDF leido como texto (`delDocumento`).
pub fn del_documento(uid: &str, nombre_del_original: &str) -> String {
    if nombre_del_original.to_ascii_lowercase().ends_with(".pdf") {
        format!("{PREFIJO}{uid}-texto")
    } else {
        format!("{PREFIJO}{uid}")
    }
}

/// La ruta portatil de un fichero de lo anotado: `pins/draw/<base><terminacion>`.
pub fn rel(base: &str, terminacion: &str) -> String {
    format!("{CARPETA}/{base}{terminacion}")
}

/// `marcasJuntoA`: `pins/draw/X.excalidraw.gz` → `pins/draw/X.marcas`;
/// `None` si no es un dibujo de esa carpeta.
pub fn marcas_junto_a(rel: &str) -> Option<String> {
    let resto = rel.strip_prefix(CARPETA)?.strip_prefix('/')?;
    let base = resto.strip_suffix(".excalidraw.gz")?;
    (!resto.contains('/')).then(|| format!("{CARPETA}/{base}.marcas"))
}

/// **La vista de cada marcador de un lienzo** (25-oct-2026, como las vistas
/// con nombre de AutoCAD): `pins/draw/X.excalidraw.gz` →
/// `pins/draw/X.vistas.json`, el objeto id de la marca → `"x y ancho alto"`
/// (`pixpin_motor2d::marcas::VistaDeMarca`). Va con su dibujo como
/// `X.marcas`; `None` si no es un dibujo de esa carpeta.
pub fn vistas_junto_a(rel: &str) -> Option<String> {
    let marcas = marcas_junto_a(rel)?;
    Some(format!("{}{VISTAS}", marcas.strip_suffix(".marcas")?))
}

/// El codigo de mensaje de un nombre de fichero (`anot-<uid>…`), si lo es:
/// empieza por el prefijo, acaba en una terminacion conocida y el codigo
/// tiene su largo entero (`porUid`).
pub fn uid_del_nombre(nombre: &str) -> Option<&str> {
    let resto = nombre.strip_prefix(PREFIJO)?;
    if !TERMINACIONES.iter().any(|t| nombre.ends_with(t)) {
        return None;
    }
    let fin = resto.find(['-', '.']).unwrap_or(resto.len());
    let uid = &resto[..fin];
    (uid.chars().count() == crate::codigo::LARGO).then_some(uid)
}

/// `porUid`: lo anotado de una lista de rutas portatiles, por codigo de
/// mensaje. Lo que no es de un mensaje se deja fuera.
pub fn por_uid<'a>(rels: impl IntoIterator<Item = &'a str>) -> BTreeMap<String, Vec<String>> {
    let mut salida: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in rels {
        let Some(nombre) = r.strip_prefix(CARPETA).and_then(|x| x.strip_prefix('/')) else {
            continue;
        };
        if nombre.contains('/') {
            continue;
        }
        if let Some(uid) = uid_del_nombre(nombre) {
            salida
                .entry(uid.to_string())
                .or_default()
                .push(r.to_string());
        }
    }
    salida
}

/// **La columna fijada de un Word o un libro** y la letra con la que se
/// fijo (`AnotacionesDelAdjunto.Maqueta`). Los numeros son los del movil:
/// `grosor` es el indice de sus cuatro pesos (300, 400, 600, 800) y `tipo`
/// el de sus cuatro letras (serif, sans, mono, cursiva).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Maqueta {
    pub columna: i32,
    pub izq: i32,
    pub der: i32,
    pub tamano: i32,
    pub grosor: i32,
    pub tipo: i32,
}

impl Maqueta {
    pub fn a_texto(&self) -> String {
        format!(
            "{},{},{},{},{},{}",
            self.columna, self.izq, self.der, self.tamano, self.grosor, self.tipo
        )
    }

    /// `deTexto`: seis enteros con coma; una columna de cero o menos, o
    /// cualquier cosa que no sea un entero, no es maqueta.
    pub fn de_texto(t: &str) -> Option<Maqueta> {
        let n: Vec<i32> = t
            .trim()
            .split(',')
            .map(|x| x.trim().parse::<i32>().ok())
            .collect::<Option<Vec<_>>>()?;
        if n.len() != 6 || n[0] <= 0 {
            return None;
        }
        Some(Maqueta {
            columna: n[0],
            izq: n[1],
            der: n[2],
            tamano: n[3],
            grosor: n[4],
            tipo: n[5],
        })
    }
}

/// **El marco de la tinta** (idea del usuario, 29-sep-2026: «un marco
/// invisible [...] como puntos de referencia que siempre deben ir en algun
/// lugar, que la tinta se ajuste con respecto al marco»).
///
/// Cada aparato escribe la tinta de un documento en las unidades de su capa,
/// y esas unidades dependian de cosas que el otro tenia que adivinar (los
/// espacios del PDF, el margen izquierdo de un Word). El marco las dice: es
/// **donde esta la hoja** —la pagina de un PDF, la columna de un Word o un
/// libro— en las mismas unidades en que estan los puntos de su fichero de
/// tinta. Quien lee lleva ese rectangulo al de su hoja y la tinta cae encima
/// sin adivinar nada.
///
/// Va en un fichero hermano de la tinta (`anot-<uid>-p<n>.hoja`,
/// `anot-<uid>.hoja`), no dentro de la escena: la `Scene` del movil tira la
/// escena entera si trae un tipo que no conoce, y un elemento marco se
/// pintaria como un recuadro en un aparato viejo.
///
/// El texto son dos lineas, `x0,y0,x1,y1` con punto decimal y `v1`: **el
/// formato de Android** (`AnotacionesDelAdjunto.Marco`, v0.98.0), que
/// rechaza un marco de mas lineas. Al leer la segunda es opcional y aun se
/// acepta una tercera `tinta <huella>` (la que escribio el PC el 29-sep, ver
/// [`huella_de_tinta`]); al escribir ya no se pone. Para un Word o un libro el marco es la
/// banda de la columna desde lo alto del documento, **tan alta como ancha**
/// (`izq,0,izq+columna,columna`): el alto del documento depende de quien lo
/// maqueta (el `WebView` suma su relleno), y el texto crece a la escala de
/// su columna, asi que la escala vertical tiene que ser la misma.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarcoDeLaHoja {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// La unica version que se entiende. Otra es un formato que aun no se sabe
/// leer: mejor la regla vieja que encajar con numeros que dicen otra cosa.
const VERSION_DEL_MARCO: &str = "v1";

/// Lo que va delante de la huella de la tinta en la tercera linea del marco.
const PREFIJO_DE_LA_HUELLA: &str = "tinta ";

/// Cuantas cifras hexadecimales de la huella escribe el PC: 64 bits bastan
/// para saber si es la misma tinta, y el fichero sigue siendo de una linea
/// corta. Quien lee acepta de 16 a 64 (un prefijo del resumen entero).
pub const CIFRAS_DE_LA_HUELLA: usize = 16;

/// **La huella de una tinta** (29-sep): las primeras cifras del resumen con
/// que la sincronizacion compara ese fichero en los dos aparatos
/// (`canonico::resumen`: SHA-256 del JSON canonico, sobre el texto
/// portatil y descomprimido). Es la unica cuenta que los dos lados ya hacen
/// igual byte a byte: los bytes del `.gz` no (cada uno comprime a su
/// manera), ni los del fichero del PC (sale al movil en su forma legible,
/// `para_el_movil`), pero el resumen canonico si, o cada vuelta mandaria de
/// nuevo todos los dibujos.
///
/// Servia para que el marco no mintiera: un movil que aun no sabia del
/// marco (v0.97) podia reescribir la tinta sin tocar su `.hoja`. **Ya no se
/// escribe** (30-sep): Android v0.98 hizo el marco de dos lineas y rechaza
/// uno de tres (caeria a la regla vieja), y como escribe el marco cada vez
/// que guarda la tinta, la huella sobra. Se sigue leyendo en los marcos que
/// el PC escribio con ella: si no coincide, el marco no vale.
pub fn huella_de_tinta(texto_portatil: &str) -> String {
    crate::canonico::resumen(texto_portatil)[..CIFRAS_DE_LA_HUELLA].to_string()
}

/// Si la huella escrita en un marco (`de_16_a_64` cifras) es la del resumen
/// entero `resumen` de la tinta de ahora.
pub fn huella_coincide(escrita: &str, resumen: &str) -> bool {
    resumen.len() >= escrita.len() && resumen[..escrita.len()].eq_ignore_ascii_case(escrita)
}

fn huella_valida(h: &str) -> bool {
    (CIFRAS_DE_LA_HUELLA..=64).contains(&h.len()) && h.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Un numero del marco: hasta tres decimales, sin ceros de cola ni `-0`.
/// Tres bastan (un milesimo de unidad) y un numero corto se lee igual en
/// Kotlin (`toFloatOrNull`) y en Rust.
fn numero(v: f32) -> String {
    let t = format!("{v:.3}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t == "-0" { "0".into() } else { t.to_string() }
}

impl MarcoDeLaHoja {
    pub fn nuevo(x0: f32, y0: f32, x1: f32, y1: f32) -> MarcoDeLaHoja {
        MarcoDeLaHoja { x0, y0, x1, y1 }
    }

    pub fn ancho(&self) -> f32 {
        self.x1 - self.x0
    }

    pub fn alto(&self) -> f32 {
        self.y1 - self.y0
    }

    /// Un marco con que encajar: numeros de verdad y con area.
    pub fn valido(&self) -> bool {
        [self.x0, self.y0, self.x1, self.y1]
            .iter()
            .all(|v| v.is_finite())
            && self.ancho() > 1e-3
            && self.alto() > 1e-3
    }

    /// **El marco como lo escribe Android** (`aTexto`): `x0,y0,x1,y1` y
    /// `v1`, dos lineas. Una tercera haria que Android lo ignorase.
    pub fn a_texto(&self) -> String {
        format!(
            "{},{},{},{}\n{VERSION_DEL_MARCO}\n",
            numero(self.x0),
            numero(self.y0),
            numero(self.x1),
            numero(self.y1)
        )
    }

    /// `casiIgual` de Android: las cuatro esquinas a menos de una centesima.
    /// Escribir un marco que ya dice esto no toca el fichero: una ida y
    /// vuelta sin cambios no provoca otro envio.
    pub fn casi_igual(&self, o: &MarcoDeLaHoja) -> bool {
        [
            self.x0 - o.x0,
            self.y0 - o.y0,
            self.x1 - o.x1,
            self.y1 - o.y1,
        ]
        .iter()
        .all(|d| d.abs() < 0.01)
    }

    /// **La hoja de un PDF tal como la mide el editor** (`Marco.deHoja`):
    /// `ancho` de ancho desde el cero y de alto lo que da su `proporcion`
    /// (ancho entre alto). Es el marco con que Android escribe **siempre**
    /// la tinta de un PDF desde v0.98.0, pongan o quiten espacios, y el que
    /// escribe ahora el PC: una sola convencion.
    pub fn de_hoja(ancho: f32, proporcion: f32) -> MarcoDeLaHoja {
        MarcoDeLaHoja::nuevo(0.0, 0.0, ancho, ancho / proporcion.max(0.01))
    }

    /// **La regla de antes del marco** (`Marco.delLectorViejo`), para la
    /// tinta de un PDF suelto que llega sin el: el lector del movil escribia
    /// en unidades que dependian de los espacios puestos (1 = izquierda,
    /// 2 = derecha). Es la cuenta de `pixpin_docs::vista::capa_del_movil`:
    /// sin espacios `-1050,0,2450,…`; con los dos, la hoja tal cual.
    pub fn del_lector_viejo(
        espacios: u8,
        ancho: f32,
        margen: f32,
        proporcion: f32,
    ) -> MarcoDeLaHoja {
        let izq = if espacios & 1 != 0 { 1.0 } else { 0.0 };
        let der = if espacios & 2 != 0 { 1.0 } else { 0.0 };
        let k = (1.0 + 2.0 * margen) / (1.0 + margen * (izq + der));
        let x0 = ancho * margen * izq * k - ancho * margen;
        MarcoDeLaHoja::nuevo(x0, 0.0, x0 + ancho * k, ancho / proporcion.max(0.01) * k)
    }

    /// `deTexto`: cuatro numeros con coma en la primera linea y, si hay
    /// segunda, que sea `v1`. Lo que no, `None`: se ignora y se lee con la
    /// regla de antes. La huella, si la hay, no se mira aqui (ver
    /// [`MarcoDeLaHoja::de_texto_con_huella`]).
    pub fn de_texto(t: &str) -> Option<MarcoDeLaHoja> {
        MarcoDeLaHoja::de_texto_con_huella(t).map(|(m, _)| m)
    }

    /// El marco y la huella de su tinta, si la trae. La tercera linea solo
    /// puede ir detras de `v1` y ser `tinta <16 a 64 cifras hexadecimales>`;
    /// cualquier otra cosa hace el marco entero no valido (un formato que
    /// no se sabe leer no se encaja a medias).
    pub fn de_texto_con_huella(t: &str) -> Option<(MarcoDeLaHoja, Option<String>)> {
        let mut lineas = t.lines().map(str::trim).filter(|l| !l.is_empty());
        let n: Vec<f32> = lineas
            .next()?
            .split(',')
            .map(|x| x.trim().parse::<f32>().ok())
            .collect::<Option<Vec<_>>>()?;
        if let Some(v) = lineas.next()
            && v != VERSION_DEL_MARCO
        {
            return None;
        }
        let huella = match lineas.next() {
            None => None,
            Some(l) => {
                let h = l.strip_prefix(PREFIJO_DE_LA_HUELLA)?.trim();
                if !huella_valida(h) {
                    return None;
                }
                Some(h.to_ascii_lowercase())
            }
        };
        if n.len() != 4 || lineas.next().is_some() {
            return None;
        }
        let m = MarcoDeLaHoja::nuevo(n[0], n[1], n[2], n[3]);
        m.valido().then_some((m, huella))
    }

    /// **El mismo marco sin la huella**, en las dos lineas que entiende
    /// Android: la primera tal cual (con los decimales de quien la escribio:
    /// reescribirla con los de aqui la cambiaria sin decir nada nuevo) y
    /// `v1`. `None` si el texto no es un marco que se entienda (no se toca).
    pub fn sin_huella(texto: &str) -> Option<String> {
        MarcoDeLaHoja::de_texto_con_huella(texto)?;
        let primera = texto.lines().map(str::trim).find(|l| !l.is_empty())?;
        Some(format!("{primera}\n{VERSION_DEL_MARCO}\n"))
    }

    /// **Como se lleva un punto de `propio` a este marco**, eje por eje:
    /// `(ex, ey, dx, dy)` con `aqui = propio * e + d`. Si las proporciones no
    /// coinciden, cada eje con la suya (se encaja al marco, que es lo que
    /// manda); quien la use lo dice en el registro.
    pub fn desde(&self, propio: &MarcoDeLaHoja) -> (f32, f32, f32, f32) {
        let ex = self.ancho() / propio.ancho();
        let ey = self.alto() / propio.alto();
        (ex, ey, self.x0 - propio.x0 * ex, self.y0 - propio.y0 * ey)
    }
}

/// `Lectura.margenDe` en enteros, como lo guarda el movil: dos tercios de la columna.
pub fn margen_de(columna: i32) -> i32 {
    columna * 2 / 3
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_nombres_son_los_del_movil() {
        assert_eq!(de_pagina("ABCDE23456", 2), "anot-ABCDE23456-p2");
        assert_eq!(del_pdf("ABCDE23456"), "anot-ABCDE23456");
        assert_eq!(
            del_documento("ABCDE23456", "2_informe.docx"),
            "anot-ABCDE23456"
        );
        assert_eq!(
            del_documento("ABCDE23456", "Plano.PDF"),
            "anot-ABCDE23456-texto"
        );
        assert_eq!(
            rel("anot-ABCDE23456", ".marcas"),
            "pins/draw/anot-ABCDE23456.marcas"
        );
    }

    #[test]
    fn solo_lo_de_un_mensaje_con_su_codigo_entero_y_sin_temporales() {
        let rels = [
            "pins/draw/anot-ABCDE23456-p0.excalidraw.gz",
            "pins/draw/anot-ABCDE23456.marcas",
            "pins/draw/anot-ABCDE23456.marcas.tmp",
            "pins/draw/anot-CORTO.marcas",
            "pins/draw/d1.excalidraw.gz",
            "pins/draw/files/anot-ABCDE23456.marcas",
            "guardados/anot-ABCDE23456.marcas",
        ];
        let m = por_uid(rels);
        assert_eq!(m.len(), 1);
        assert_eq!(
            m["ABCDE23456"],
            vec![
                "pins/draw/anot-ABCDE23456-p0.excalidraw.gz".to_string(),
                "pins/draw/anot-ABCDE23456.marcas".into()
            ]
        );
        assert_eq!(
            uid_del_nombre("anot-ABCDE23456-texto.maqueta"),
            Some("ABCDE23456")
        );
        assert_eq!(uid_del_nombre("anot-ABCDE23456.pdf"), None);
    }

    #[test]
    fn las_marcas_de_un_lienzo_van_junto_a_su_dibujo() {
        assert_eq!(
            marcas_junto_a("pins/draw/d1.excalidraw.gz").as_deref(),
            Some("pins/draw/d1.marcas")
        );
        // Caso negativo: ni lo de otra carpeta ni lo que no es un dibujo.
        assert_eq!(marcas_junto_a("pins/draw/files/x.excalidraw.gz"), None);
        assert_eq!(marcas_junto_a("croquis3d/c.croquis.gz"), None);
        assert_eq!(marcas_junto_a("pins/drawx/d.excalidraw.gz"), None);
    }

    #[test]
    fn las_vistas_de_las_marcas_de_un_lienzo_van_junto_a_su_dibujo() {
        assert_eq!(
            vistas_junto_a("pins/draw/d1.excalidraw.gz").as_deref(),
            Some("pins/draw/d1.vistas.json")
        );
        assert_eq!(
            vistas_junto_a("pins/draw/hoja-pr-1-0.excalidraw.gz").as_deref(),
            Some("pins/draw/hoja-pr-1-0.vistas.json")
        );
        // Caso negativo: lo mismo que no tiene marcas tampoco tiene vistas.
        assert_eq!(vistas_junto_a("pins/draw/files/x.excalidraw.gz"), None);
        assert_eq!(vistas_junto_a("croquis3d/c.croquis.gz"), None);
        assert_eq!(vistas_junto_a("pins/draw/d1.marcas"), None);
        // Y no se confunde con lo de un adjunto: no lleva codigo de mensaje.
        assert_eq!(uid_del_nombre("d1.vistas.json"), None);
    }

    #[test]
    fn la_maqueta_se_lee_y_se_escribe_como_en_el_movil() {
        let m = Maqueta::de_texto("420,280,280,100,1,0").unwrap();
        assert_eq!(
            m,
            Maqueta {
                columna: 420,
                izq: 280,
                der: 280,
                tamano: 100,
                grosor: 1,
                tipo: 0
            }
        );
        assert_eq!(m.a_texto(), "420,280,280,100,1,0");
        assert_eq!(
            Maqueta::de_texto(" 384, 256,256,100,1,0\n").map(|m| m.columna),
            Some(384)
        );
        // Casos negativos del movil.
        assert_eq!(Maqueta::de_texto("0,1,2,3,4,5"), None);
        assert_eq!(Maqueta::de_texto("420,x,2,3,4,5"), None);
        assert_eq!(Maqueta::de_texto("420,1,2"), None);
        assert_eq!(Maqueta::de_texto(""), None);
        assert_eq!(margen_de(384), 256);
    }

    #[test]
    fn el_marco_de_la_hoja_viaja_con_su_tinta_y_se_borra_con_su_mensaje() {
        let rels = [
            "pins/draw/anot-ABCDE23456-p0.excalidraw.gz",
            "pins/draw/anot-ABCDE23456-p0.hoja",
            "pins/draw/anot-ABCDE23456.hoja",
            "pins/draw/anot-ABCDE23456-p0.hoja.tmp",
            "pins/draw/pdf-1a2b-p0.hoja",
        ];
        let m = por_uid(rels);
        assert_eq!(m["ABCDE23456"].len(), 3, "{m:?}");
        assert_eq!(
            uid_del_nombre("anot-ABCDE23456-p12.hoja"),
            Some("ABCDE23456")
        );
        // Caso negativo: el temporal no viaja.
        assert_eq!(uid_del_nombre("anot-ABCDE23456-p0.hoja.tmp"), None);
        assert_eq!(
            rel(&de_pagina("ABCDE23456", 3), HOJA),
            "pins/draw/anot-ABCDE23456-p3.hoja"
        );
    }

    #[test]
    fn el_marco_se_escribe_corto_y_se_lee_igual() {
        let m = MarcoDeLaHoja::nuevo(-1050.0, 0.0, 2450.0, 4950.0);
        assert_eq!(m.a_texto(), "-1050,0,2450,4950\nv1\n");
        assert_eq!(MarcoDeLaHoja::de_texto(&m.a_texto()), Some(m));
        let raro = MarcoDeLaHoja::nuevo(0.12345, -0.0001, 1400.5, 1979.99);
        assert_eq!(raro.a_texto(), "0.123,0,1400.5,1979.99\nv1\n");
        // Sin version, con espacios y con el `1400.0` de Kotlin tambien vale.
        assert_eq!(
            MarcoDeLaHoja::de_texto(" 0.0, 0.0 ,1400.0,1980.0 \r\n"),
            Some(MarcoDeLaHoja::nuevo(0.0, 0.0, 1400.0, 1980.0))
        );
    }

    #[test]
    fn un_marco_mal_formado_no_es_marco() {
        for t in [
            "",
            "1,2,3",
            "1,2,3,4,5",
            "0,0,x,10",
            "0,0,1400,1980\nv2",
            "0,0,1400,1980\nv1\nmas",
            "10,0,10,1980",
            "0,50,1400,20",
            "0,0,NaN,1980",
            "0,0,inf,1980",
            "0;0;1400;1980",
        ] {
            assert_eq!(MarcoDeLaHoja::de_texto(t), None, "{t:?}");
        }
    }

    #[test]
    fn la_huella_que_el_pc_escribio_el_29_se_sigue_leyendo() {
        let m = MarcoDeLaHoja::nuevo(-1050.0, 0.0, 2450.0, 4950.0);
        let tinta = r#"{"elements":[{"id":"a","x":1.0}],"files":{}}"#;
        let h = huella_de_tinta(tinta);
        assert_eq!(h.len(), CIFRAS_DE_LA_HUELLA);
        // Es el principio del resumen con que la sincronizacion compara el
        // fichero: el mismo JSON escrito de otra manera da la misma.
        assert!(crate::canonico::resumen(tinta).starts_with(&h));
        assert_eq!(
            huella_de_tinta("{ \"files\":{},\"elements\":[{\"x\":1.0,\"id\":\"a\"}]}"),
            h
        );
        // Lo que escribio el PC el 29-sep se sigue leyendo.
        let t = format!("-1050,0,2450,4950\nv1\ntinta {h}\n");
        assert_eq!(
            MarcoDeLaHoja::de_texto_con_huella(&t),
            Some((m, Some(h.clone())))
        );
        assert_eq!(MarcoDeLaHoja::de_texto(&t), Some(m));
        // Sin huella (lo que el PC escribio antes) sigue valiendo.
        assert_eq!(
            MarcoDeLaHoja::de_texto_con_huella(&m.a_texto()),
            Some((m, None))
        );
        // Una huella entera (64) de Kotlin, en mayusculas, tambien.
        let entera = crate::canonico::resumen(tinta).to_ascii_uppercase();
        let (_, leida) =
            MarcoDeLaHoja::de_texto_con_huella(&format!("0,0,1,1\nv1\ntinta {entera}")).unwrap();
        assert!(huella_coincide(
            &leida.unwrap(),
            &crate::canonico::resumen(tinta)
        ));
        // Caso negativo: la huella de otra tinta no coincide.
        assert!(!huella_coincide(
            &h,
            &crate::canonico::resumen(r#"{"elements":[]}"#)
        ));
        assert!(!huella_coincide("abcdef0123456789", "abcd"));
    }

    #[test]
    fn una_tercera_linea_que_no_es_una_huella_hace_el_marco_no_valido() {
        for t in [
            "0,0,1400,1980\nv1\ntinta xyz0123456789abcd",
            "0,0,1400,1980\nv1\ntinta 0123",
            "0,0,1400,1980\nv1\ntinta ",
            "0,0,1400,1980\nv1\nhuella 0123456789abcdef",
            "0,0,1400,1980\nv1\ntinta 0123456789abcdef\notra",
            "0,0,1400,1980\nv2\ntinta 0123456789abcdef",
        ] {
            assert_eq!(MarcoDeLaHoja::de_texto_con_huella(t), None, "{t:?}");
        }
        // Quitar la huella deja la primera linea tal cual y `v1`.
        assert_eq!(
            MarcoDeLaHoja::sin_huella(
                " 500.0,300.0,4700.0,6240.0\r\nv1\r\ntinta 0123456789abcdef\r\n"
            )
            .as_deref(),
            Some("500.0,300.0,4700.0,6240.0\nv1\n")
        );
        assert_eq!(
            MarcoDeLaHoja::sin_huella("1,2,3,4").as_deref(),
            Some("1,2,3,4\nv1\n")
        );
        assert_eq!(
            MarcoDeLaHoja::sin_huella("1,2,3,4\nv2\n"),
            None,
            "lo que no se entiende no se toca"
        );
    }

    /// Lo que escribe Android v0.98.0 (`MarcoDeLaHojaTest`): dos lineas, y
    /// rechaza cualquier marco de mas. El PC escribe exactamente eso.
    #[test]
    fn el_marco_se_escribe_en_las_dos_lineas_que_lee_android() {
        let m = MarcoDeLaHoja::nuevo(-1050.0, 0.0, 2450.0, 4950.0);
        let t = m.a_texto();
        assert_eq!(t, "-1050,0,2450,4950\nv1\n");
        assert_eq!(
            t.lines().count(),
            2,
            "una tercera linea haria que Android lo ignorase"
        );
        assert_eq!(
            MarcoDeLaHoja::nuevo(0.5, -0.0, 1414.2857, 2.0).a_texto(),
            "0.5,0,1414.286,2\nv1\n"
        );
        // Lo que escribe Android: `deHoja` de un A4 y los decimales de Kotlin.
        for android in [
            "0,0,1400,1979.899\nv1\n",
            "1400.0,0.0,2800.0,1980.0",
            "-1050,0,2450,4950\nv1\n",
        ] {
            assert!(MarcoDeLaHoja::de_texto(android).is_some(), "{android:?}");
        }
    }

    #[test]
    fn la_regla_del_lector_de_antes_es_la_misma_que_en_android() {
        let cerca = |a: MarcoDeLaHoja, b: MarcoDeLaHoja| {
            assert!(
                [a.x0 - b.x0, a.y0 - b.y0, a.x1 - b.x1, a.y1 - b.y1]
                    .iter()
                    .all(|d| d.abs() < 1e-2),
                "{a:?} frente a {b:?}"
            )
        };
        let p = 1400.0 / 1980.0;
        cerca(
            MarcoDeLaHoja::del_lector_viejo(0, 1400.0, 0.75, p),
            MarcoDeLaHoja::nuevo(-1050.0, 0.0, 2450.0, 4950.0),
        );
        cerca(
            MarcoDeLaHoja::del_lector_viejo(3, 1400.0, 0.75, p),
            MarcoDeLaHoja::nuevo(0.0, 0.0, 1400.0, 1980.0),
        );
        let k = 2.5 / 1.75;
        cerca(
            MarcoDeLaHoja::del_lector_viejo(2, 1400.0, 0.75, p),
            MarcoDeLaHoja::nuevo(-1050.0, 0.0, -1050.0 + 1400.0 * k, 1980.0 * k),
        );
        cerca(
            MarcoDeLaHoja::del_lector_viejo(1, 1400.0, 0.75, p),
            MarcoDeLaHoja::nuevo(
                1050.0 * k - 1050.0,
                0.0,
                1050.0 * k - 1050.0 + 1400.0 * k,
                1980.0 * k,
            ),
        );
        cerca(
            MarcoDeLaHoja::de_hoja(1400.0, p),
            MarcoDeLaHoja::nuevo(0.0, 0.0, 1400.0, 1980.0),
        );
        // Es la cuenta de `capa_del_movil` del PC: ancho `1400·k`, cero en `dx`.
        for e in 0..4u8 {
            let viejo = MarcoDeLaHoja::del_lector_viejo(e, 1400.0, 0.75, p);
            let (k, dx) = pixpin_docs_capa_del_movil(e);
            cerca(
                viejo,
                MarcoDeLaHoja::nuevo(dx, 0.0, dx + 1400.0 * k, 1980.0 * k),
            );
        }
    }

    /// `capa_del_movil` de `pixpin_docs::vista` (este crate no depende de
    /// el): la escala y el cero de la hoja en la capa del movil de antes.
    fn pixpin_docs_capa_del_movil(espacios: u8) -> (f32, f32) {
        let (izq, der) = (f32::from(espacios & 1), f32::from((espacios >> 1) & 1));
        let k = 2.5 / (1.0 + 0.75 * (izq + der));
        (k, 1050.0 * izq * k - 1050.0)
    }

    #[test]
    fn casi_igual_tolera_el_redondeo_y_no_mas() {
        let m = MarcoDeLaHoja::nuevo(0.0, 0.0, 1400.0, 1980.0);
        assert!(m.casi_igual(&MarcoDeLaHoja::nuevo(0.0, 0.0, 1400.0005, 1980.0)));
        // Caso negativo: diez unidades de alto ya es otro marco.
        assert!(!m.casi_igual(&MarcoDeLaHoja::nuevo(0.0, 0.0, 1400.0, 1990.0)));
    }

    #[test]
    fn el_encaje_lleva_la_hoja_propia_exactamente_al_marco() {
        let propio = MarcoDeLaHoja::nuevo(0.0, 0.0, 1400.0, 1980.0);
        let fichero = MarcoDeLaHoja::nuevo(-1050.0, 0.0, 2450.0, 4950.0);
        let (ex, ey, dx, dy) = fichero.desde(&propio);
        assert_eq!((ex, ey, dx, dy), (2.5, 2.5, -1050.0, 0.0));
        // Proporciones distintas: cada eje con la suya.
        let otro = MarcoDeLaHoja::nuevo(100.0, 50.0, 800.0, 1050.0);
        let (ex, ey, dx, dy) = otro.desde(&propio);
        assert!((ex - 0.5).abs() < 1e-6 && (ey - 1000.0 / 1980.0).abs() < 1e-6);
        assert_eq!((dx, dy), (100.0, 50.0));
    }

    #[test]
    fn los_comentarios_de_una_nota_son_de_lo_anotado_y_su_temporal_no() {
        let rels = [
            "pins/draw/anot-ABCDE23456.comentarios.json",
            "pins/draw/anot-ABCDE23456.comentarios.json.tmp",
            "pins/draw/anot-ABC.comentarios.json",
            "pins/draw/nota.comentarios.json",
        ];
        let m = por_uid(rels);
        assert_eq!(m.len(), 1);
        assert_eq!(
            m["ABCDE23456"],
            ["pins/draw/anot-ABCDE23456.comentarios.json"]
        );
        assert_eq!(rel(&del_pdf("ABCDE23456"), COMENTARIOS), rels[0]);
    }

    fn j(v: serde_json::Value) -> Json {
        Json::de_valor(&v)
    }

    #[test]
    fn lo_anotado_va_con_los_adjuntos_y_las_notas_y_no_con_lo_demas() {
        assert!(lleva_anotado(&j(
            serde_json::json!({"id": "a", "clase": "ARCHIVO", "ruta": "x"})
        )));
        assert!(lleva_anotado(&j(
            serde_json::json!({"id": "n", "clase": "NOTA", "texto": "# Hola"})
        )));
        assert!(!lleva_anotado(&j(
            serde_json::json!({"id": "d", "clase": "DIBUJO", "referencia": "d1"})
        )));
        assert!(!lleva_anotado(&j(
            serde_json::json!({"id": "t", "texto": "hola"})
        )));
    }

    #[test]
    fn las_hojas_quitadas_son_las_que_faltan_y_solo_esas() {
        let antes = j(serde_json::json!({"id": "p", "hojas": [
            {"id": "n-1", "uid": "AAAAAAAAAA", "nota": "x"},
            {"id": "n-2", "uid": "BBBBBBBBBB", "nota": "y"},
            {"id": "d1", "dibujo": "d1"}
        ]}));
        let despues = j(serde_json::json!({"id": "p", "hojas": [
            {"id": "n-2", "uid": "BBBBBBBBBB", "nota": "y cambiada"},
            {"id": "d1", "dibujo": "d1"}
        ]}));
        assert_eq!(hojas_quitadas(Some(&antes), &despues), ["AAAAAAAAAA"]);
        // Caso negativo: un proyecto nuevo o sin cambios de hojas no quita nada.
        assert!(hojas_quitadas(None, &despues).is_empty());
        assert!(hojas_quitadas(Some(&despues), &antes).is_empty());
    }
}
