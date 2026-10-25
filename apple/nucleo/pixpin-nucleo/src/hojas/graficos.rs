//! **Los graficos de un libro de Excel** (`pixpin-hojas`, el lector del
//! visor de hojas de Windows): su titulo, su clase, las categorias y los
//! valores guardados en cache por Excel, y su ancla a las celdas, para que
//! Swift los pinte encima de la rejilla en las mismas celdas que Excel.
//!
//! No se calcula nada: se ensena lo ultimo que Excel guardo (como el visor de
//! Windows, que pinta «algo parecido o al menos un recuadro con el titulo»).

use std::path::Path;

use pixpin_hojas::{Ancla, ContenidoDibujo, Libro, Regional, TipoHoja};

/// Una serie de un grafico.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SerieGrafico {
    pub nombre: String,
    /// Un valor por categoria; `NaN` donde Excel no guardo ninguno.
    pub valores: Vec<f64>,
    /// `#rrggbb`, si el libro lo dice (o el del tema).
    pub color: Option<String>,
}

/// **Un grafico anclado a una hoja.**
///
/// El sitio va en celdas: arriba a la izquierda en la celda
/// (`columna`, `fila`) mas `dx`/`dy` pixeles de Excel (96 ppp) dentro de
/// ella; abajo a la derecha en (`hasta_columna`, `hasta_fila`) mas
/// `hasta_dx`/`hasta_dy` si va de una celda a otra, o con `ancho` x `alto`
/// pixeles si no (`hasta_*` en `None`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct GraficoHoja {
    /// El nombre de la hoja (la pestana de Excel).
    pub hoja: String,
    /// Su orden en el libro (desde 0, contando todas las hojas).
    pub orden: u32,
    pub titulo: String,
    /// La clase de Excel: `barChart` (columnas), `barHChart` (barras),
    /// `lineChart`, `pieChart`, `doughnutChart`, `areaChart`,
    /// `scatterChart`... o vacio si no se entiende.
    pub tipo: String,
    pub categorias: Vec<String>,
    pub series: Vec<SerieGrafico>,
    pub columna: u32,
    pub fila: u32,
    pub dx: f64,
    pub dy: f64,
    pub hasta_columna: Option<u32>,
    pub hasta_fila: Option<u32>,
    pub hasta_dx: f64,
    pub hasta_dy: f64,
    pub ancho: f64,
    pub alto: f64,
}

fn hex(c: pixpin_hojas::Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// **Los graficos de todas las hojas de `ruta`** (un `.xlsx`/`.xlsm`; un
/// `.xls` antiguo o un CSV no tienen). Bloqueante: lee cada hoja entera
/// (el ancla va al final de su XML), asi que Swift lo llama fuera del hilo
/// principal. Un libro que no se lee da la lista vacia.
#[uniffi::export]
pub fn graficos_del_libro(ruta: String) -> Vec<GraficoHoja> {
    let Ok(libro) = Libro::abrir(Path::new(&ruta), Regional::es_pe()) else {
        return Vec::new();
    };
    let mut v = Vec::new();
    for (i, info) in libro.hojas.iter().enumerate() {
        if info.tipo != TipoHoja::Hoja {
            continue;
        }
        let mut h = libro.abrir_hoja(i);
        h.cargar_todo();
        for d in &h.cola.dibujos {
            let ContenidoDibujo::Grafico {
                titulo,
                tipo,
                categorias,
                series,
            } = &d.contenido
            else {
                continue;
            };
            let mut g = GraficoHoja {
                hoja: info.nombre.clone(),
                orden: i as u32,
                titulo: titulo.clone(),
                tipo: tipo.clone(),
                categorias: categorias.clone(),
                series: series
                    .iter()
                    .map(|s| SerieGrafico {
                        nombre: s.nombre.clone(),
                        valores: s.valores.clone(),
                        color: s.color.map(hex),
                    })
                    .collect(),
                columna: 0,
                fila: 0,
                dx: 0.0,
                dy: 0.0,
                hasta_columna: None,
                hasta_fila: None,
                hasta_dx: 0.0,
                hasta_dy: 0.0,
                ancho: 0.0,
                alto: 0.0,
            };
            match d.ancla {
                Ancla::DosCeldas { desde, hasta } => {
                    (g.columna, g.fila, g.dx, g.dy) = (desde.col, desde.fila, desde.dx, desde.dy);
                    g.hasta_columna = Some(hasta.col);
                    g.hasta_fila = Some(hasta.fila);
                    (g.hasta_dx, g.hasta_dy) = (hasta.dx, hasta.dy);
                }
                Ancla::UnaCelda { desde, ancho, alto } => {
                    (g.columna, g.fila, g.dx, g.dy) = (desde.col, desde.fila, desde.dx, desde.dy);
                    (g.ancho, g.alto) = (ancho, alto);
                }
                Ancla::Absoluta { x, y, ancho, alto } => {
                    (g.dx, g.dy, g.ancho, g.alto) = (x, y, ancho, alto);
                }
            }
            v.push(g);
        }
    }
    v
}

/// **La rejilla de la tinta de una hoja** (`anotar_inteligente::rejilla_de_la_capa`
/// de Windows): donde empieza cada columna y cada fila en pixeles de Excel
/// al 100 % (96 ppp). La tinta de un libro (`Nucleo::ruta_de_tinta_de_hoja`)
/// va en estas unidades, y cada visor lleva cada punto a la misma fraccion
/// de la misma celda de su rejilla: asi cae en su celda en todos.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RejillaTinta {
    /// `columnas + 1` bordes (el primero, 0).
    pub xs: Vec<f64>,
    /// `filas + 1` bordes (el primero, 0).
    pub ys: Vec<f64>,
    /// Lo que mide una columna o una fila mas alla de las de arriba.
    pub ancho_defecto: f64,
    pub alto_defecto: f64,
}

/// Los nombres de todas las hojas del libro, en su orden (las ocultas
/// tambien): para saber cual es la pestana que se ve.
#[uniffi::export]
pub fn hojas_del_libro(ruta: String) -> Vec<String> {
    Libro::abrir(Path::new(&ruta), Regional::es_pe())
        .map(|l| l.hojas.iter().map(|h| h.nombre.clone()).collect())
        .unwrap_or_default()
}

/// La rejilla de la hoja `hoja` (desde 0, en el orden de
/// [`hojas_del_libro`]) con sus `columnas` y `filas` primeras (con tope de
/// 4096 y 200 000). `None` si el libro no se lee. Bloqueante.
#[uniffi::export]
pub fn rejilla_del_libro(ruta: String, hoja: u32, columnas: u32, filas: u32) -> Option<RejillaTinta> {
    let libro = Libro::abrir(Path::new(&ruta), Regional::es_pe()).ok()?;
    if hoja as usize >= libro.hojas.len() {
        return None;
    }
    let (columnas, filas) = (columnas.min(4096), filas.min(200_000));
    let mut h = libro.abrir_hoja(hoja as usize);
    h.cargar_hasta_fila(filas);
    let r = pixpin_hojas::Rejilla::nueva(&h, &libro.estilos.fuente_normal);
    Some(RejillaTinta {
        xs: (0..=columnas).map(|c| r.x(c)).collect(),
        ys: (0..=filas).map(|f| r.y(f)).collect(),
        ancho_defecto: r.ancho_defecto,
        alto_defecto: r.alto_defecto,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_libro_sin_graficos_o_que_no_se_lee_da_la_lista_vacia() {
        let d = std::env::temp_dir().join(format!("pixpin-graficos-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let t = pixpin_proyecto::tabla::Tabla::default();
        let bytes = pixpin_proyecto::tabla_xlsx::a_xlsx(
            &[pixpin_proyecto::tabla_xlsx::HojaParaVer {
                nombre: "Hoja1",
                tabla: &t,
                valores: None,
            }],
            0,
        )
        .unwrap();
        let libro = d.join("sin.xlsx");
        std::fs::write(&libro, bytes).unwrap();
        assert!(graficos_del_libro(libro.display().to_string()).is_empty());
        assert_eq!(hojas_del_libro(libro.display().to_string()), vec!["Hoja1".to_string()]);
        let r = rejilla_del_libro(libro.display().to_string(), 0, 3, 2).unwrap();
        assert_eq!(r.xs.len(), 4);
        assert_eq!(r.ys.len(), 3);
        assert!(r.xs[1] > 0.0 && r.ys[2] > r.ys[1]);
        assert!(rejilla_del_libro(libro.display().to_string(), 5, 3, 2).is_none());
        // Caso negativo: lo que no es un libro.
        let roto = d.join("roto.xlsx");
        std::fs::write(&roto, "no es un zip").unwrap();
        assert!(graficos_del_libro(roto.display().to_string()).is_empty());
        assert!(rejilla_del_libro(roto.display().to_string(), 0, 1, 1).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
