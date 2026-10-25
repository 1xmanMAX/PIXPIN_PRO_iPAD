//! **La maqueta del movil** (K16): donde cae cada bloque de un Word o un
//! libro, con las cuentas de la pagina que ensena el `WebView` de Android.
//! Es `visor/maqueta_movil.rs` y `visor/tablas.rs` de Windows, y el partir
//! renglones de `pixpin_render::lectura`, sin ventana: lo que alli pregunta
//! a DirectWrite (lo que avanza cada letra) aqui lo contesta Core Text desde
//! Swift ([`super::MedidorReflujo`]).
//!
//! # Por que
//!
//! Lo anotado encima de un documento va en pixeles de su pagina. Si dos
//! aparatos colocan el texto de otra forma, el trazo que en el movil
//! subrayaba una palabra cae en otro sobre otra: medido con el Word del
//! usuario, el movil lo ponia en 12 185 px de alto y el PC (antes de K16) en
//! 19 016. Por eso Windows y el Mac colocan **igual que el movil**: la misma
//! hoja de estilo (`DocxAHtml.ESTILO` para un Word, `EpubAHtml.ESTILO` para
//! un libro, y encima `Lectura.estilo`), la misma letra (Noto Serif y
//! Roboto, incrustadas: [`super::letras_de_lectura`]), los mismos renglones
//! de `line-height` exacto ([`partir`]) y las mismas reglas de CSS: margenes
//! que se solapan, relleno de `main`, sangria de las listas, tablas de ancho
//! automatico con sus celdas `padding:4px 8px` y su raya de 1 px, imagenes a
//! su alto. Con la columna fijada (`anot-<uid>.maqueta`) los tres parten los
//! renglones en los mismos sitios y la tinta cae en la misma palabra; el
//! aumento y la ventana solo escalan y desplazan.
//!
//! # Las unidades
//!
//! Las del lector: pixeles logicos a escala 1 con el cero en el borde de la
//! columna (el `body` del movil, de `columna` px de ancho). Dentro va `main`
//! (como mucho 46 em, centrado) con su relleno, y el texto empieza en su
//! caja de contenido.

use pixpin_docs::documento::{
    Alineacion as AlineaDoc, Bloque, Clase, Documento, MARCA_IMAGEN, Trozo, texto_y_tramos,
};
use pixpin_docs::lectura;
use pixpin_docs::tabla::Celda;

// ---------------------------------------------------------------------------
// Lo que se coloca

/// Negrita, cursiva o codigo de un tramo (`pixpin_render::EstiloTexto`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct EstiloTexto {
    pub(crate) negrita: bool,
    pub(crate) cursiva: bool,
    pub(crate) mono: bool,
}

/// Un tramo de un parrafo con otro estilo, en unidades UTF-16.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tramo {
    pub(crate) inicio: u32,
    pub(crate) longitud: u32,
    pub(crate) estilo: EstiloTexto,
}

/// `text-align` del bloque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Alineacion {
    #[default]
    Izquierda,
    Centro,
    Derecha,
}

/// Lo que ocupa un parrafo a un ancho: el renglon mas ancho, el alto de
/// todos, el ancho minimo (el trozo sin corte mas ancho) y donde empieza y
/// acaba cada renglon (en unidades UTF-16, sin los espacios del final).
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Medida {
    pub(crate) ancho: f32,
    pub(crate) alto: f32,
    pub(crate) minimo: f32,
    pub(crate) renglones: Vec<(u32, u32)>,
}

/// De que color va un bloque (los de la hoja del movil; Swift los pone del
/// tono del lector).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tinte {
    Texto,
    Apagado,
    Raya,
}

/// Las letras del movil (`Lectura.LETRAS`: serif, sans-serif, monospace,
/// cursive): las dos primeras son las de Android incrustadas; Courier New
/// avanza lo mismo que Droid Sans Mono (0,6 em); la cursiva de Android
/// (Dancing Script) no viene, y Caveat es la letra a mano de la aplicacion.
pub(crate) const FAMILIAS: [&str; lectura::TIPOS] =
    ["Noto Serif", "Roboto", "Courier New", "Caveat"];

/// La letra de ancho fijo de un tramo de codigo: Courier New avanza 0,6 em
/// por letra, lo mismo que la «monospace» de Android (Droid Sans Mono).
pub(crate) const LETRA_FIJA: &str = "Courier New";

/// Con que letra va un bloque.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Letra {
    pub(crate) familia: &'static str,
    pub(crate) peso: u16,
    pub(crate) interlineado: f32,
    pub(crate) alineacion: Alineacion,
}

impl Letra {
    /// La letra del cuerpo de un documento con estos ajustes.
    pub(crate) fn del_cuerpo(hoja: Hoja, a: &lectura::Ajustes) -> Letra {
        Letra {
            familia: FAMILIAS[usize::from(a.tipo).min(FAMILIAS.len() - 1)],
            peso: lectura::PESOS[usize::from(a.grosor).min(lectura::GROSORES - 1)],
            interlineado: hoja.css().interlineado,
            alineacion: Alineacion::Izquierda,
        }
    }
}

/// Donde cae una celda de una tabla: su caja entera (con sus rayas) y
/// cuales lleva (`visor/tablas.rs`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Caja {
    pub(crate) x: f32,
    pub(crate) ancho: f32,
    pub(crate) y: f32,
    pub(crate) alto: f32,
    /// Sin raya arriba: es la continuacion de una celda unida hacia abajo.
    pub(crate) arriba: bool,
    /// Sin raya abajo: la sigue su continuacion.
    pub(crate) abajo: bool,
    pub(crate) relleno: bool,
}

/// Un bloque ya medido: lo que se pinta y cuanto ocupa, en unidades del
/// documento (pixeles logicos a escala 1, con el cero en el borde de la
/// columna).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Colocado {
    pub(crate) texto: String,
    pub(crate) tramos: Vec<Tramo>,
    pub(crate) clase: Clase,
    pub(crate) tam: f32,
    /// Sangria a la izquierda respecto del borde de la columna.
    pub(crate) sangria: f32,
    /// El ancho de la caja del texto: la columna menos la sangria, o el
    /// hueco de dentro de su celda en una tabla.
    pub(crate) ancho: f32,
    pub(crate) y: f32,
    pub(crate) alto: f32,
    pub(crate) tinte: Tinte,
    /// El bloque del documento del que sale (la cabecera no tiene).
    pub(crate) bloque: Option<usize>,
    /// En una tabla, la caja de su celda y sus rayas.
    pub(crate) caja: Option<Caja>,
    /// La letra del bloque, la del movil: con otra, el texto se parte en
    /// otros renglones que los que se midieron.
    pub(crate) letra: Letra,
    /// Una imagen: su hueco con un recuadro y lo que es.
    pub(crate) recuadro: bool,
    /// Que imagen del documento (`doc.imagenes`) va en ese hueco, si su
    /// cabecera dijo lo que mide.
    pub(crate) imagen: Option<usize>,
    /// Donde empieza y acaba cada renglon, como se partio al medir (las
    /// reglas del navegador, [`partir`]): se pinta por ahi.
    pub(crate) renglones: Vec<(u32, u32)>,
}

/// Lo que mide un parrafo a un ancho con su letra: el lector lo pregunta a
/// Core Text ([`super::MedidorReflujo`]) y las pruebas a una cuenta de
/// mentira.
pub(crate) type Mide<'a> = dyn Fn(&str, f32, f32, &[Tramo], &Letra) -> Medida + 'a;

// ---------------------------------------------------------------------------
// Donde se puede partir (`pixpin_render::lectura`)

fn es_espacio(c: u16) -> bool {
    // El espacio duro (U+00A0) no parte: es el `&nbsp;` de un parrafo vacio.
    matches!(c, 0x20 | 0x09 | 0x2000..=0x200A | 0x3000) && c != 0x2007
}

fn es_letra_o_cifra(c: u16) -> bool {
    char::from_u32(u32::from(c)).is_some_and(|c| c.is_alphanumeric())
}

fn es_ideograma(c: u16) -> bool {
    matches!(c, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF)
}

/// **Si el navegador puede partir entre `antes` y `despues`** (el ultimo
/// caracter de un renglon y el primero del siguiente): tras un espacio (el
/// espacio se queda colgando al final), tras un guion entre letras o cifras,
/// tras una raya corta, antes y despues de una raya larga, tras un espacio
/// de ancho cero y entre ideogramas. No tras la barra ni el punto de un
/// enlace, ni entre una letra y el signo de grado: asi lo hace el `WebView`.
pub(crate) fn se_parte_tras(antes: u16, despues: u16) -> bool {
    if es_espacio(despues) {
        return false;
    }
    if es_espacio(antes) || antes == 0x200B {
        return true;
    }
    match antes {
        // Guion y guion de Unicode: entre dos letras o cifras.
        0x2D | 0x2010 => es_letra_o_cifra(despues),
        // Raya corta: detras.
        0x2013 => true,
        // Raya larga: detras, salvo pegada a otra.
        0x2014 => despues != 0x2014,
        _ => despues == 0x2014 && antes != 0x2014 || es_ideograma(antes) || es_ideograma(despues),
    }
}

/// **Parte en renglones** un texto de `anchos` (lo que avanza cada unidad
/// UTF-16: el ancho de su racimo en la primera y cero en las demas) a un
/// ancho maximo. Devuelve los renglones (sin sus espacios del final), el mas
/// ancho y el trozo sin corte mas ancho (el «min-content»).
///
/// Un salto de linea (`\n`, U+2028) parte siempre. Una palabra que no cabe
/// ni sola se parte por donde llegue (`overflow-wrap:break-word`).
pub(crate) fn partir(texto: &[u16], anchos: &[f32], ancho_max: f32) -> (Vec<(u32, u32)>, f32, f32) {
    let n = texto.len().min(anchos.len());
    let texto = &texto[..n];
    let mut renglones = Vec::new();
    let mut mas_ancho = 0.0f32;
    let mut minimo = 0.0f32;
    let fin_sin_espacios = |a: usize, b: usize| -> usize {
        let mut b = b;
        while b > a && es_espacio(texto[b - 1]) {
            b -= 1;
        }
        b
    };
    // Sin los espacios del final: cuelgan fuera del renglon.
    let ancho_de = |a: usize, b: usize| -> f32 { anchos[a..fin_sin_espacios(a, b)].iter().sum() };
    let salto = |c: u16| c == 0x0A || c == 0x2028;

    // El min-content: cada trozo entre dos sitios donde se puede partir.
    let mut desde = 0usize;
    for i in 1..=n {
        let corte = i == n || salto(texto[i - 1]) || se_parte_tras(texto[i - 1], texto[i]);
        if corte {
            let hasta = if i > desde && salto(texto[i - 1]) {
                i - 1
            } else {
                i
            };
            minimo = minimo.max(ancho_de(desde, hasta));
            desde = i;
        }
    }

    let mut inicio = 0usize;
    while inicio < n || renglones.is_empty() {
        if inicio >= n {
            renglones.push((n as u32, n as u32));
            break;
        }
        let mut suma = 0.0f32;
        // El ultimo sitio donde se podia partir.
        let mut ultimo: Option<usize> = None;
        let mut i = inicio;
        let mut fin = n;
        let mut siguiente = n;
        while i < n {
            let c = texto[i];
            if salto(c) {
                fin = i;
                siguiente = i + 1;
                break;
            }
            if i > inicio && se_parte_tras(texto[i - 1], c) {
                ultimo = Some(i);
            }
            let ancho = anchos[i];
            if !es_espacio(c) && suma + ancho > ancho_max + 0.01 && suma > 0.0 {
                match ultimo {
                    Some(u) => {
                        fin = u;
                        siguiente = u;
                    }
                    None => {
                        // Ni la palabra sola cabe: se parte por donde llega.
                        fin = i;
                        siguiente = i;
                    }
                }
                break;
            }
            suma += ancho;
            i += 1;
        }
        if i >= n && fin == n {
            siguiente = n;
        }
        let b = fin_sin_espacios(inicio, fin);
        mas_ancho = mas_ancho.max(ancho_de(inicio, fin));
        renglones.push((inicio as u32, b as u32));
        if siguiente <= inicio {
            // Nunca un renglon vacio que no avanza.
            siguiente = inicio + 1;
        }
        // Un salto al final del parrafo no abre otro renglon (un `<br>` al
        // final de un `<p>` tampoco).
        inicio = siguiente;
    }
    (renglones, mas_ancho, minimo)
}

/// **Mide un parrafo** con lo que avanza cada unidad UTF-16 (`avances`): sus
/// renglones, partidos como en el movil, y su alto de renglones exactos
/// (`DWRITE_LINE_SPACING_METHOD_UNIFORM`, el `line-height` de CSS).
pub(crate) fn medir_con_avances(
    texto: &str,
    avances: &[f32],
    tam: f32,
    ancho_max: f32,
    letra: &Letra,
) -> Medida {
    let unidades: Vec<u16> = texto.encode_utf16().collect();
    let mut a: Vec<f32> = avances
        .iter()
        .map(|x| if x.is_finite() { x.max(0.0) } else { 0.0 })
        .collect();
    // Lo que no contesto el medidor, a media eme.
    a.resize(unidades.len(), tam * 0.5);
    let (renglones, ancho, minimo) = partir(&unidades, &a, ancho_max);
    Medida {
        ancho,
        alto: renglones.len() as f32 * tam * letra.interlineado,
        minimo,
        renglones,
    }
}

// ---------------------------------------------------------------------------
// La hoja de estilo del movil

/// **Que hoja de estilo usa el movil** para este documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Hoja {
    /// `DocxAHtml.ESTILO` (y lo que no es un libro: pagina, Markdown).
    #[default]
    Word,
    /// `EpubAHtml.ESTILO`.
    Libro,
}

impl Hoja {
    pub(crate) fn de(nombre: &str) -> Hoja {
        match pixpin_docs::formato_de(nombre) {
            Some(pixpin_docs::Formato::Epub) => Hoja::Libro,
            _ => Hoja::Word,
        }
    }

    fn css(self) -> &'static Css {
        match self {
            Hoja::Word => &WORD,
            Hoja::Libro => &LIBRO,
        }
    }
}

/// Los numeros de una hoja de estilo del movil. Lo que va en «em» se
/// multiplica por la letra del cuerpo (o la del bloque, en los titulos).
struct Css {
    /// `body{font:16px/1.55 …}`.
    letra: f32,
    interlineado: f32,
    /// `main{max-width:46em;padding:18px 16px 48px}`.
    ancho_max_em: f32,
    arriba: f32,
    lado: f32,
    abajo: f32,
    /// `p{margin:.55em 0}`.
    parrafo_em: f32,
    /// `h1..h6{font-size}` en em del cuerpo.
    titulos: [f32; 6],
    /// `h1..h6{line-height:1.25;margin:1.2em 0 .5em}`, en em del titulo.
    titulo_interlineado: f32,
    titulo_arriba_em: f32,
    titulo_abajo_em: f32,
    /// La lista del Word es `p.lista{margin:.25em 0 .25em 1.2em}`; la del
    /// libro, `ul{margin:1em 0;padding-left:40px}` con `li` sin margen.
    lista_em: f32,
    lista_sangria_em: f32,
    lista_sangria_px: f32,
    /// `.tabla{margin:.8em 0}` del Word; la tabla del libro no lleva.
    tabla_em: f32,
    /// `td p{margin:.2em 0}` del Word; en el libro `p{margin:.7em 0}` tambien
    /// dentro de una celda.
    celda_parrafo_em: f32,
    /// `blockquote{margin:1em 40px}` de fabrica; `1em 1.4em` en el libro.
    cita_em: f32,
    cita_lado_em: f32,
    cita_lado_px: f32,
    /// `hr`: de fabrica `.5em auto` y 2 px de raya; en el libro `1.5em 0` y 1 px.
    regla_em: f32,
    regla_alto: f32,
    /// `section.capitulo{border-top:1px;margin-top:2.5em;padding-top:2em}`.
    capitulo: Option<(f32, f32)>,
    /// `header.libro{margin:2em 0 1em}` con `h1{font-size:1.9em;line-height:1.2;
    /// margin:0 0 .35em}` y el autor en cursiva sin margen.
    cabecera: bool,
}

const WORD: Css = Css {
    letra: 16.0,
    interlineado: 1.55,
    ancho_max_em: 46.0,
    arriba: 18.0,
    lado: 16.0,
    abajo: 48.0,
    parrafo_em: 0.55,
    titulos: [1.7, 1.4, 1.2, 1.05, 1.05, 1.05],
    titulo_interlineado: 1.25,
    titulo_arriba_em: 1.2,
    titulo_abajo_em: 0.5,
    lista_em: 0.25,
    lista_sangria_em: 1.2,
    lista_sangria_px: 0.0,
    tabla_em: 0.8,
    celda_parrafo_em: 0.2,
    cita_em: 1.0,
    cita_lado_em: 0.0,
    cita_lado_px: 40.0,
    regla_em: 0.5,
    regla_alto: 2.0,
    capitulo: None,
    cabecera: false,
};

const LIBRO: Css = Css {
    letra: 17.0,
    interlineado: 1.6,
    ancho_max_em: 42.0,
    arriba: 18.0,
    lado: 18.0,
    abajo: 64.0,
    parrafo_em: 0.7,
    titulos: [1.6, 1.35, 1.15, 1.0, 1.0, 1.0],
    titulo_interlineado: 1.25,
    titulo_arriba_em: 1.3,
    titulo_abajo_em: 0.6,
    lista_em: 0.0,
    lista_sangria_em: 0.0,
    lista_sangria_px: 40.0,
    tabla_em: 0.0,
    celda_parrafo_em: 0.7,
    cita_em: 1.0,
    cita_lado_em: 1.4,
    cita_lado_px: 0.0,
    regla_em: 1.5,
    regla_alto: 1.0,
    capitulo: Some((2.5, 2.0)),
    cabecera: true,
};

/// `td{padding:4px 8px}` y la raya de 1 px de `border-collapse`.
const CELDA_ALTO: f32 = 4.0;
const CELDA_ANCHO: f32 = 8.0;
const RAYA: f32 = 1.0;

/// La letra del cuerpo, en pixeles: la de la hoja por el tamano elegido
/// (el `textZoom` del `WebView`, que escala tambien lo que va en em).
pub(crate) fn letra_del_cuerpo(hoja: Hoja, a: &lectura::Ajustes) -> f32 {
    hoja.css().letra * a.tamano as f32 / 100.0
}

/// **La caja del texto** dentro de la columna: `main` mide la columna o,
/// si es mas ancha, sus 46 em centrados, y el texto va dentro de su relleno.
/// Devuelve `(x, ancho)` en unidades del lector.
pub(crate) fn caja_de_texto(hoja: Hoja, a: &lectura::Ajustes, columna: f32) -> (f32, f32) {
    let css = hoja.css();
    // `max-width` es de la caja de contenido: el relleno va por fuera.
    let texto = (columna - 2.0 * css.lado)
        .min(css.ancho_max_em * letra_del_cuerpo(hoja, a))
        .max(1.0);
    let x = (columna - texto - 2.0 * css.lado) / 2.0 + css.lado;
    (x, texto)
}

/// Los margenes verticales de CSS: el de abajo de un bloque y el de arriba
/// del siguiente **se solapan** (cuenta el mayor), salvo que haya un relleno
/// o una raya en medio.
struct Flujo {
    y: f32,
    margen: f32,
}

impl Flujo {
    /// Un bloque de `alto` con sus dos margenes: devuelve donde empieza.
    fn bloque(&mut self, arriba: f32, alto: f32, abajo: f32) -> f32 {
        let y = self.y + self.margen.max(arriba);
        self.y = y + alto;
        self.margen = abajo;
        y
    }
}

/// El texto de un bloque y sus tramos, como los pinta el `WebView`: la
/// negrita, la cursiva y el codigo de cada trozo; el enlace solo cambia de
/// color (`.enlace`), no de letra; el tabulador de Word es `&emsp;`, y los
/// espacios seguidos son uno y los del principio y el final no cuentan
/// (`white-space:normal` de HTML).
pub(crate) fn texto_de(trozos: &[Trozo]) -> (String, Vec<Tramo>) {
    let mut limpios: Vec<Trozo> = Vec::with_capacity(trozos.len());
    let mut tras_espacio = true;
    for t in trozos {
        let mut s = String::with_capacity(t.texto.len());
        for ch in t.texto.chars() {
            match ch {
                ' ' | '\r' => {
                    if !tras_espacio {
                        s.push(' ');
                        tras_espacio = true;
                    }
                }
                '\n' => {
                    if s.ends_with(' ') {
                        s.pop();
                    }
                    s.push('\n');
                    tras_espacio = true;
                }
                '\t' => {
                    s.push('\u{2003}');
                    tras_espacio = false;
                }
                c => {
                    s.push(c);
                    tras_espacio = false;
                }
            }
        }
        limpios.push(Trozo {
            texto: s,
            estilo: t.estilo,
        });
    }
    if let Some(ultimo) = limpios.iter_mut().rev().find(|t| !t.texto.is_empty())
        && ultimo.texto.ends_with(' ')
    {
        ultimo.texto.pop();
    }
    let b = Bloque::nuevo(Clase::Parrafo, limpios);
    let (texto, lista) = texto_y_tramos(&b);
    let tramos = lista
        .iter()
        .map(|t| Tramo {
            inicio: t.inicio,
            longitud: t.longitud,
            estilo: EstiloTexto {
                negrita: t.estilo.negrita,
                cursiva: t.estilo.cursiva,
                mono: t.estilo.mono,
            },
        })
        .filter(|t| t.estilo != EstiloTexto::default())
        .collect();
    (texto, tramos)
}

fn alineacion(a: AlineaDoc) -> Alineacion {
    match a {
        AlineaDoc::Izquierda => Alineacion::Izquierda,
        AlineaDoc::Centro => Alineacion::Centro,
        AlineaDoc::Derecha => Alineacion::Derecha,
    }
}

/// Todo lo que hace falta para colocar, junto.
struct Obra<'a, 'm> {
    css: &'static Css,
    base: f32,
    cuerpo: Letra,
    x: f32,
    ancho: f32,
    mide: &'a Mide<'m>,
    flujo: Flujo,
    salida: Vec<Colocado>,
}

impl Obra<'_, '_> {
    /// Un parrafo de texto: su alto son sus renglones (uno al menos: el
    /// `<p>&nbsp;</p>` de un parrafo vacio de Word ocupa su renglon).
    #[allow(clippy::too_many_arguments)] // el bloque entero: texto, letra, sitio y margenes
    fn parrafo(
        &mut self,
        texto: String,
        tramos: Vec<Tramo>,
        clase: Clase,
        tam: f32,
        letra: Letra,
        (x, ancho): (f32, f32),
        (arriba, abajo): (f32, f32),
        tinte: Tinte,
        bloque: Option<usize>,
    ) {
        let renglon = tam * letra.interlineado;
        let vacio = texto.trim().is_empty();
        let (alto, renglones) = if vacio {
            (renglon, Vec::new())
        } else {
            let m = (self.mide)(&texto, tam, ancho, &tramos, &letra);
            (m.alto.max(renglon), m.renglones)
        };
        let y = self.flujo.bloque(arriba, alto, abajo);
        self.salida.push(Colocado {
            texto: if vacio { String::new() } else { texto },
            tramos,
            clase,
            tam,
            sangria: x,
            ancho,
            y,
            alto,
            tinte,
            bloque,
            caja: None,
            letra,
            recuadro: false,
            imagen: None,
            renglones,
        });
    }
}

/// **Coloca el documento como el movil** en una columna de `columna`
/// unidades. Devuelve los bloques colocados (en orden de lectura) y el alto
/// de la pagina entera.
pub(crate) fn colocar(
    doc: &Documento,
    ajustes: &lectura::Ajustes,
    columna: f32,
    hoja: Hoja,
    mide: &Mide<'_>,
) -> (Vec<Colocado>, f32) {
    let css = hoja.css();
    let base = letra_del_cuerpo(hoja, ajustes);
    let cuerpo = Letra::del_cuerpo(hoja, ajustes);
    let fuerte = cuerpo.peso.max(700);
    let (x, ancho) = caja_de_texto(hoja, ajustes, columna);
    let mut o = Obra {
        css,
        base,
        cuerpo,
        x,
        ancho,
        mide,
        // El relleno de arriba de `main`: el primer margen no lo cruza.
        flujo: Flujo {
            y: css.arriba,
            margen: 0.0,
        },
        salida: Vec::with_capacity(doc.bloques.len() + 2),
    };

    // La cabecera del libro: su titulo centrado y el autor en cursiva.
    if css.cabecera && !doc.titulo.trim().is_empty() {
        let tam = base * 1.9;
        let letra = Letra {
            peso: fuerte,
            interlineado: 1.2,
            alineacion: Alineacion::Centro,
            ..cuerpo
        };
        o.parrafo(
            doc.titulo.trim().to_string(),
            Vec::new(),
            Clase::Titulo(1),
            tam,
            letra,
            (x, ancho),
            (base * 2.0, tam * 0.35),
            Tinte::Texto,
            None,
        );
        if !doc.autor.trim().is_empty() {
            let texto = doc.autor.trim().to_string();
            let largo = texto.encode_utf16().count() as u32;
            let tramos = vec![Tramo {
                inicio: 0,
                longitud: largo,
                estilo: EstiloTexto {
                    cursiva: true,
                    ..EstiloTexto::default()
                },
            }];
            let letra = Letra {
                alineacion: Alineacion::Centro,
                ..cuerpo
            };
            o.parrafo(
                texto,
                tramos,
                Clase::Nota,
                base,
                letra,
                (x, ancho),
                (0.0, 0.0),
                Tinte::Apagado,
                None,
            );
        }
        // El margen de abajo de la cabecera.
        o.flujo.margen = o.flujo.margen.max(base);
    }

    let mut imagen = 0usize;
    let mut i = 0usize;
    while i < doc.bloques.len() {
        let b = &doc.bloques[i];
        match b.clase {
            Clase::Fila => {
                let mut hasta = i + 1;
                while hasta < doc.bloques.len()
                    && pixpin_docs::tabla::misma_tabla(&doc.bloques[hasta - 1], &doc.bloques[hasta])
                {
                    hasta += 1;
                }
                tabla(&mut o, doc, i, hasta, &mut imagen);
                i = hasta;
                continue;
            }
            Clase::Capitulo if css.capitulo.is_some() => {
                // La raya de arriba del capitulo y su relleno.
                let (margen, relleno) = css.capitulo.unwrap_or((2.5, 2.0));
                let y = o.flujo.bloque(base * margen, RAYA + base * relleno, 0.0);
                o.salida
                    .push(raya(Clase::Capitulo, x, ancho, y, Some(i), cuerpo));
            }
            Clase::Regla | Clase::Capitulo => {
                let y = o
                    .flujo
                    .bloque(base * css.regla_em, css.regla_alto, base * css.regla_em);
                o.salida.push(raya(b.clase, x, ancho, y, Some(i), cuerpo));
            }
            Clase::Nota if b.texto() == MARCA_IMAGEN => {
                let datos = doc.imagenes.get(imagen);
                colocar_imagen(&mut o, datos, i, imagen);
                imagen += 1;
            }
            Clase::Titulo(n) => {
                let tam = base * css.titulos[usize::from(n.clamp(1, 6)) - 1];
                let letra = Letra {
                    peso: fuerte,
                    interlineado: css.titulo_interlineado,
                    alineacion: alineacion(b.alineacion),
                    ..cuerpo
                };
                let (texto, tramos) = texto_de(&b.trozos);
                let margenes = (tam * css.titulo_arriba_em, tam * css.titulo_abajo_em);
                o.parrafo(
                    texto,
                    tramos,
                    b.clase,
                    tam,
                    letra,
                    (x, ancho),
                    margenes,
                    Tinte::Texto,
                    Some(i),
                );
            }
            Clase::Lista => {
                let (texto, tramos) = texto_de(&b.trozos);
                // La vineta va delante, en el renglon: corre los tramos.
                let texto = format!("• {texto}");
                let tramos = tramos
                    .into_iter()
                    .map(|t| Tramo {
                        inicio: t.inicio + 2,
                        ..t
                    })
                    .collect();
                let sangria = base * css.lista_sangria_em + css.lista_sangria_px;
                // En el libro los puntos seguidos son una lista: su margen
                // va antes del primero y despues del ultimo.
                let (arriba, abajo) = if css.lista_em > 0.0 {
                    (base * css.lista_em, base * css.lista_em)
                } else {
                    let antes = i > 0 && doc.bloques[i - 1].clase == Clase::Lista;
                    let despues = doc
                        .bloques
                        .get(i + 1)
                        .is_some_and(|s| s.clase == Clase::Lista);
                    (
                        if antes { 0.0 } else { base },
                        if despues { 0.0 } else { base },
                    )
                };
                let letra = Letra {
                    alineacion: alineacion(b.alineacion),
                    ..cuerpo
                };
                o.parrafo(
                    texto,
                    tramos,
                    b.clase,
                    base,
                    letra,
                    (x + sangria, (ancho - sangria).max(1.0)),
                    (arriba, abajo),
                    Tinte::Texto,
                    Some(i),
                );
            }
            Clase::Cita => {
                let lado = base * css.cita_lado_em + css.cita_lado_px;
                let (texto, tramos) = texto_de(&b.trozos);
                o.parrafo(
                    texto,
                    tramos,
                    b.clase,
                    base,
                    cuerpo,
                    (x + lado, (ancho - 2.0 * lado).max(1.0)),
                    (base * css.cita_em, base * css.cita_em),
                    Tinte::Apagado,
                    Some(i),
                );
            }
            Clase::Codigo => {
                // `pre` no esta en la lista de `Lectura.estilo`: se queda en
                // la de ancho fijo del navegador, a 13 px (por el tamano).
                let tam = 13.0 * ajustes.tamano as f32 / 100.0;
                let letra = Letra {
                    familia: LETRA_FIJA,
                    peso: 400,
                    ..cuerpo
                };
                let (texto, tramos) = texto_de(&b.trozos);
                o.parrafo(
                    texto,
                    tramos,
                    b.clase,
                    tam,
                    letra,
                    (x, ancho),
                    (tam, tam),
                    Tinte::Texto,
                    Some(i),
                );
            }
            Clase::Parrafo | Clase::Nota => {
                let (texto, tramos) = texto_de(&b.trozos);
                let tinte = if b.clase == Clase::Nota {
                    Tinte::Apagado
                } else {
                    Tinte::Texto
                };
                let letra = Letra {
                    alineacion: alineacion(b.alineacion),
                    ..cuerpo
                };
                let m = base * css.parrafo_em;
                o.parrafo(
                    texto,
                    tramos,
                    b.clase,
                    base,
                    letra,
                    (x, ancho),
                    (m, m),
                    tinte,
                    Some(i),
                );
            }
        }
        i += 1;
    }
    // El ultimo margen no cruza el relleno de abajo de `main`.
    let alto = o.flujo.y + o.flujo.margen + o.css.abajo;
    (o.salida, alto)
}

fn raya(clase: Clase, x: f32, ancho: f32, y: f32, bloque: Option<usize>, letra: Letra) -> Colocado {
    Colocado {
        texto: String::new(),
        tramos: Vec::new(),
        clase,
        tam: 0.0,
        sangria: x,
        ancho,
        y,
        alto: RAYA,
        tinte: Tinte::Raya,
        bloque,
        caja: None,
        letra,
        recuadro: false,
        imagen: None,
        renglones: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Imagenes

/// **Lo que mide una imagen** por su cabecera, sin descodificarla: PNG, GIF,
/// BMP, JPEG y WebP, que son las que pinta el `WebView`.
pub(crate) fn medidas_de_imagen(d: &[u8]) -> Option<(u32, u32)> {
    let u32be = |i: usize| {
        d.get(i..i + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    let u16be = |i: usize| {
        d.get(i..i + 2)
            .map(|b| u32::from(u16::from_be_bytes([b[0], b[1]])))
    };
    let u16le = |i: usize| {
        d.get(i..i + 2)
            .map(|b| u32::from(u16::from_le_bytes([b[0], b[1]])))
    };
    let i32le = |i: usize| {
        d.get(i..i + 4)
            .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]).unsigned_abs())
    };
    let medidas = if d.starts_with(b"\x89PNG") {
        (u32be(16)?, u32be(20)?)
    } else if d.starts_with(b"GIF8") {
        (u16le(6)?, u16le(8)?)
    } else if d.starts_with(b"BM") {
        (i32le(18)?, i32le(22)?)
    } else if d.starts_with(b"RIFF") && d.get(8..12) == Some(b"WEBP") {
        match d.get(12..16)? {
            b"VP8X" => {
                let tres = |i: usize| {
                    d.get(i..i + 3)
                        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], 0]) + 1)
                };
                (tres(24)?, tres(27)?)
            }
            b"VP8L" => {
                let b = d.get(21..25)?;
                let v = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                ((v & 0x3FFF) + 1, ((v >> 14) & 0x3FFF) + 1)
            }
            _ => (u16le(26)? & 0x3FFF, u16le(28)? & 0x3FFF),
        }
    } else if d.starts_with(&[0xFF, 0xD8]) {
        // Los segmentos del JPEG hasta el que dice su tamano (SOFn).
        let mut i = 2usize;
        loop {
            if *d.get(i)? != 0xFF {
                return None;
            }
            let marca = *d.get(i + 1)?;
            if marca == 0xFF {
                i += 1;
                continue;
            }
            let largo = u16be(i + 2)? as usize;
            if matches!(marca, 0xC0..=0xCF) && !matches!(marca, 0xC4 | 0xC8 | 0xCC) {
                break (u16be(i + 7)?, u16be(i + 5)?);
            }
            i += 2 + largo;
        }
    } else {
        return None;
    };
    (medidas.0 > 0 && medidas.1 > 0).then_some(medidas)
}

/// Una imagen del documento: en el movil va dentro de su `<p>`, a su tamano
/// y como mucho del ancho del texto (`img{max-width:100%;height:auto}`),
/// sentada en la linea base del renglon, asi que debajo queda lo que baja la
/// letra. Aqui va su hueco a ese tamano y el visor pinta la imagen dentro,
/// descodificada solo cuando se ve. El hueco sale de la cabecera, sin
/// descodificar: medir sigue siendo instantaneo y, como el movil la pone
/// del mismo alto, la tinta de debajo cae en la misma palabra.
///
/// Si su cabecera no se deja leer, queda un renglon con un recuadro que lo
/// dice y sin `imagen`. `n` es su numero en `doc.imagenes`.
fn colocar_imagen(
    o: &mut Obra,
    datos: Option<&pixpin_docs::documento::Imagen>,
    i: usize,
    n: usize,
) {
    let base = o.base;
    let renglon = base * o.cuerpo.interlineado;
    let medida = datos.and_then(|d| medidas_de_imagen(&d.datos));
    let (w, h) = medida
        .map(|(w, h)| {
            let w = w as f32;
            let ancho = w.min(o.ancho);
            (ancho, h as f32 * ancho / w)
        })
        .unwrap_or((o.ancho, renglon));
    // Lo que queda del renglon por debajo de la linea base: medio
    // interlineado y la bajada de la letra (una quinta parte del cuerpo).
    let debajo = (renglon - base) / 2.0 + base * 0.2;
    let alto = (h + debajo).max(renglon);
    let m = base * o.css.parrafo_em;
    let y = o.flujo.bloque(m, alto, m);
    let peso = datos.map(|d| d.datos.len() / 1024).unwrap_or(0);
    o.salida.push(Colocado {
        // Con la imagen pintada encima no hace falta decir nada; sin ella,
        // que hay una y que no se puede ensenar.
        texto: if medida.is_some() {
            String::new()
        } else {
            format!("🖼 imagen ({peso} kB) — no se puede mostrar")
        },
        tramos: Vec::new(),
        clase: Clase::Nota,
        tam: base * 0.85,
        sangria: o.x,
        ancho: w,
        y,
        alto: h.max(1.0),
        tinte: Tinte::Apagado,
        bloque: Some(i),
        caja: None,
        letra: o.cuerpo,
        recuadro: true,
        imagen: medida.map(|_| n),
        renglones: Vec::new(),
    });
}

// ---------------------------------------------------------------------------
// Tablas

/// Los parrafos de una celda: el movil pone cada parrafo de Word en su `<p>`
/// (y uno vacio, `<p>&nbsp;</p>`, ocupa su renglon); la continuacion de una
/// celda unida hacia abajo es un `<td></td>` sin nada. Con cada parrafo,
/// cuantas imagenes lleva (para seguir contandolas en `doc.imagenes`).
fn parrafos_de_celda(c: &Celda) -> Vec<(String, Vec<Tramo>, usize)> {
    if c.sigue {
        return Vec::new();
    }
    let mut parrafos: Vec<Vec<Trozo>> = vec![Vec::new()];
    for t in &c.trozos {
        let mut partes = t.texto.split('\n');
        if let Some(primera) = partes.next()
            && !primera.is_empty()
            && let Some(p) = parrafos.last_mut()
        {
            p.push(Trozo {
                texto: primera.to_string(),
                estilo: t.estilo,
            });
        }
        for parte in partes {
            let mut p = Vec::new();
            if !parte.is_empty() {
                p.push(Trozo {
                    texto: parte.to_string(),
                    estilo: t.estilo,
                });
            }
            parrafos.push(p);
        }
    }
    parrafos
        .iter()
        .map(|p| {
            let (texto, tramos) = texto_de(p);
            let imagenes = p.iter().filter(|t| t.texto == MARCA_IMAGEN).count();
            (texto, tramos, imagenes)
        })
        .collect()
}

/// **Una imagen dentro de una celda**: el parrafo que es solo su marca, con
/// su numero en `doc.imagenes` y lo que mide por su cabecera.
fn imagen_de_parrafo(
    doc: &Documento,
    texto: &str,
    cuantas: usize,
    n: usize,
) -> Option<(usize, (f32, f32))> {
    if cuantas != 1 || texto != MARCA_IMAGEN {
        return None;
    }
    let (w, h) = medidas_de_imagen(&doc.imagenes.get(n)?.datos)?;
    Some((n, (w as f32, h as f32)))
}

/// **El reparto de columnas de una tabla automatica**, como el del
/// navegador (LayoutNG): cada columna tiene su minimo (su palabra mas
/// ancha) y su maximo (su parrafo en un renglon). La tabla mide su maximo
/// si cabe; si no, lo que haya, pero nunca menos que su minimo. Lo que haya
/// entre el minimo y el ancho se reparte en proporcion a lo que a cada
/// columna le falta para su maximo.
pub(crate) fn repartir_columnas(minimo: &[f32], maximo: &[f32], disponible: f32) -> Vec<f32> {
    let suma_min: f32 = minimo.iter().sum();
    let suma_max: f32 = maximo.iter().zip(minimo).map(|(a, b)| a.max(*b)).sum();
    if suma_max <= disponible {
        return maximo.iter().zip(minimo).map(|(a, b)| a.max(*b)).collect();
    }
    if suma_min >= disponible || suma_max <= suma_min {
        return minimo.to_vec();
    }
    let sobra = disponible - suma_min;
    let falta = suma_max - suma_min;
    minimo
        .iter()
        .zip(maximo)
        .map(|(mi, ma)| mi + (ma.max(*mi) - mi) * sobra / falta)
        .collect()
}

/// Un parrafo de una celda, ya con su imagen si es una.
type ParrafoDeCelda = (String, Vec<Tramo>, Option<(usize, (f32, f32))>);
/// Un parrafo medido: texto, tramos, alto, renglones e imagen (n, ancho, alto).
type Medido = (
    String,
    Vec<Tramo>,
    f32,
    Vec<(u32, u32)>,
    Option<(usize, f32, f32)>,
);

/// Coloca las filas `desde..hasta` como una tabla del movil.
fn tabla(o: &mut Obra, doc: &Documento, desde: usize, hasta: usize, imagen: &mut usize) {
    let base = o.base;
    let cuerpo = o.cuerpo;
    let filas: Vec<Vec<Celda>> = doc.bloques[desde..hasta]
        .iter()
        .map(pixpin_docs::tabla::celdas_de)
        .collect();
    let n = pixpin_docs::tabla::columnas_de(&filas).max(1);
    // Cada parrafo con su imagen, si es una: las de las celdas tambien
    // estan en `doc.imagenes`, en orden, asi que se siguen contando aqui.
    let parrafos: Vec<Vec<Vec<ParrafoDeCelda>>> = filas
        .iter()
        .map(|f| {
            f.iter()
                .map(|c| {
                    parrafos_de_celda(c)
                        .into_iter()
                        .map(|(t, tramos, cuantas)| {
                            let img = imagen_de_parrafo(doc, &t, cuantas, *imagen);
                            *imagen += cuantas;
                            (t, tramos, img)
                        })
                        .collect()
                })
                .collect()
        })
        .collect();
    // Lo que queda del renglon por debajo de la imagen (`colocar_imagen`).
    let renglon_img = base * cuerpo.interlineado;
    let debajo = (renglon_img - base) / 2.0 + base * 0.2;

    // Minimo y maximo de cada columna, por sus celdas de una columna.
    let relleno = 2.0 * CELDA_ANCHO + RAYA;
    let mut minimo = vec![relleno; n];
    let mut maximo = vec![relleno; n];
    let mut anchas: Vec<(usize, usize, f32, f32)> = Vec::new();
    for (f, ps) in filas.iter().zip(&parrafos) {
        let mut g = 0usize;
        for (c, pc) in f.iter().zip(ps) {
            let span = usize::from(c.columnas.max(1));
            let (mut mi, mut ma) = (0.0f32, 0.0f32);
            for (t, tramos, img) in pc {
                if let Some((_, (w, _))) = img {
                    // `max-width:100%`: el navegador la deja encoger hasta
                    // nada (no pone minimo) y como mucho pide su ancho.
                    ma = ma.max(*w);
                    continue;
                }
                if t.trim().is_empty() {
                    continue;
                }
                let m = (o.mide)(t, base, 1.0e6, tramos, &cuerpo);
                mi = mi.max(m.minimo);
                ma = ma.max(m.ancho);
            }
            if g < n {
                if span == 1 {
                    minimo[g] = minimo[g].max(mi + relleno);
                    maximo[g] = maximo[g].max(ma + relleno);
                } else {
                    anchas.push((g, (g + span).min(n), mi + relleno, ma + relleno));
                }
            }
            g += span;
        }
    }
    // Una celda de varias columnas que no cabe en ellas las ensancha a partes iguales.
    for (a, b, mi, ma) in anchas {
        let k = (b - a).max(1) as f32;
        let (sm, sx): (f32, f32) = (minimo[a..b].iter().sum(), maximo[a..b].iter().sum());
        if mi > sm {
            minimo[a..b].iter_mut().for_each(|v| *v += (mi - sm) / k);
        }
        if ma > sx {
            maximo[a..b].iter_mut().for_each(|v| *v += (ma - sx) / k);
        }
    }
    for (mi, ma) in minimo.iter().zip(maximo.iter_mut()) {
        *ma = ma.max(*mi);
    }
    let anchos = repartir_columnas(&minimo, &maximo, o.ancho - RAYA);
    let mut xs = Vec::with_capacity(n + 1);
    xs.push(o.x);
    for a in &anchos {
        xs.push(xs.last().copied().unwrap_or(o.x) + a);
    }

    let arriba = base * o.css.tabla_em;
    let margen_p = base * o.css.celda_parrafo_em;
    let renglon = base * cuerpo.interlineado;
    // La tabla es un bloque: su margen se solapa con el de lo de antes.
    let y0 = o.flujo.bloque(arriba, 0.0, 0.0);
    // Con `border-collapse` la raya de arriba es media dentro de la tabla:
    // la primera fila empieza medio pixel mas abajo.
    let mut y = y0 + RAYA / 2.0;
    for (k, (f, ps)) in filas.iter().zip(&parrafos).enumerate() {
        let i = desde + k;
        let abajo_sigue: Vec<usize> = filas
            .get(k + 1)
            .map(|s| {
                let mut g = 0usize;
                s.iter()
                    .filter_map(|c| {
                        let r = (g, c.sigue);
                        g += usize::from(c.columnas.max(1));
                        r.1.then_some(r.0)
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Primero se mide cada celda a su ancho: la fila mide la mas alta.
        let mut celdas: Vec<(usize, usize, Vec<Medido>, f32)> = Vec::new();
        let mut g = 0usize;
        for (c, pc) in f.iter().zip(ps) {
            let span = usize::from(c.columnas.max(1));
            if g >= n {
                break;
            }
            let fin = (g + span).min(n);
            let ancho = (xs[fin] - xs[g] - relleno).max(1.0);
            let mut dentro: Vec<Medido> = Vec::with_capacity(pc.len());
            let mut alto = 0.0f32;
            for (t, tramos, img) in pc {
                if let Some((k, (w, h))) = *img {
                    // A su tamano y como mucho del ancho de la celda,
                    // sentada en la linea base como fuera de las tablas.
                    let aw = w.min(ancho);
                    let ih = h * aw / w;
                    let renglon_alto = (ih + debajo).max(renglon_img);
                    alto += renglon_alto;
                    dentro.push((
                        String::new(),
                        Vec::new(),
                        renglon_alto,
                        Vec::new(),
                        Some((k, aw, ih)),
                    ));
                    continue;
                }
                let (h, renglones) = if t.trim().is_empty() {
                    (renglon, Vec::new())
                } else {
                    let m = (o.mide)(t, base, ancho, tramos, &cuerpo);
                    (m.alto.max(renglon), m.renglones)
                };
                alto += h;
                dentro.push((t.clone(), tramos.clone(), h, renglones, None));
            }
            if !dentro.is_empty() {
                // Los margenes de sus parrafos: arriba, entre ellos y abajo
                // (la celda es su propio bloque: no se escapan de ella).
                alto += margen_p * (dentro.len() + 1) as f32;
            }
            celdas.push((g, fin, dentro, alto));
            g = fin;
        }
        let contenido = celdas.iter().map(|c| c.3).fold(0.0f32, f32::max);
        let alto_fila = contenido + 2.0 * CELDA_ALTO + RAYA;
        for ((g, fin, dentro, _), c) in celdas.into_iter().zip(f.iter()) {
            let (x, ancho) = (xs[g], xs[fin] - xs[g]);
            let caja = Caja {
                x,
                ancho: ancho + RAYA,
                y,
                alto: alto_fila + RAYA,
                arriba: !c.sigue,
                abajo: !abajo_sigue.contains(&g),
                relleno: c.relleno,
            };
            let mut ty = y + RAYA + CELDA_ALTO + margen_p;
            let texto_x = x + RAYA + CELDA_ANCHO;
            let texto_ancho = (ancho - relleno).max(1.0);
            if dentro.is_empty() {
                // Una celda sin nada: solo su caja.
                o.salida
                    .push(celda_vacia(caja, i, cuerpo, texto_x, texto_ancho, ty, base));
                continue;
            }
            for (n_p, (t, tramos, h, renglones, img)) in dentro.into_iter().enumerate() {
                if let Some((k, aw, ih)) = img {
                    o.salida.push(Colocado {
                        texto: String::new(),
                        tramos: Vec::new(),
                        clase: Clase::Fila,
                        tam: base * 0.85,
                        sangria: texto_x,
                        ancho: aw,
                        y: ty,
                        alto: ih.max(1.0),
                        tinte: Tinte::Apagado,
                        bloque: Some(i),
                        caja: (n_p == 0).then_some(caja),
                        letra: cuerpo,
                        recuadro: true,
                        imagen: Some(k),
                        renglones: Vec::new(),
                    });
                    ty += h + margen_p;
                    continue;
                }
                o.salida.push(Colocado {
                    texto: if t.trim().is_empty() {
                        String::new()
                    } else {
                        t
                    },
                    tramos,
                    clase: Clase::Fila,
                    tam: base,
                    sangria: texto_x,
                    ancho: texto_ancho,
                    y: ty,
                    alto: h,
                    tinte: Tinte::Texto,
                    bloque: Some(i),
                    caja: (n_p == 0).then_some(caja),
                    letra: cuerpo,
                    recuadro: false,
                    imagen: None,
                    renglones,
                });
                ty += h + margen_p;
            }
        }
        y += alto_fila;
    }
    let alto = y - y0;
    o.flujo.y = y0 + alto;
    o.flujo.margen = base * o.css.tabla_em;
}

fn celda_vacia(
    caja: Caja,
    i: usize,
    letra: Letra,
    x: f32,
    ancho: f32,
    y: f32,
    base: f32,
) -> Colocado {
    Colocado {
        texto: String::new(),
        tramos: Vec::new(),
        clase: Clase::Fila,
        tam: base,
        sangria: x,
        ancho,
        y,
        alto: 0.0,
        tinte: Tinte::Texto,
        bloque: Some(i),
        caja: Some(caja),
        letra,
        recuadro: false,
        imagen: None,
        renglones: Vec::new(),
    }
}

/// **Los colocados que se leen en voz alta**, en orden: los que tienen
/// texto (ni el hueco de una imagen ni una raya). Cada uno es un «parrafo»
/// de la voz, como cada bloque de texto sin otro dentro en el movil
/// (`visor::para_leer`).
pub(crate) fn para_leer(colocados: &[Colocado]) -> Vec<usize> {
    colocados
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            !c.recuadro
                && !matches!(c.clase, Clase::Regla | Clase::Capitulo)
                && !c.texto.trim().is_empty()
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
#[path = "maqueta_pruebas.rs"]
mod pruebas;
