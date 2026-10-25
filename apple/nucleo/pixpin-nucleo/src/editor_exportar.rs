//! **Exportar, compartir e imprimir el lienzo**: las hojas que salen
//! (todo, lo elegido o cada marco) y el SVG.
//!
//! Como en Windows (`ventana_editor/exportar.rs`), aqui no hay formatos de
//! pagina ni dialogos: las hojas las decide el motor
//! (`pixpin_motor2d::exportar::hojas`) y el SVG lo escribe el motor
//! (`exportar_svg`). El PNG, el PDF y el papel los pinta Swift con el pintor
//! de siempre sobre un `CGContext` (un mapa de bits, un PDF vectorial o la
//! impresora), asi que aqui solo se le dan las ordenes de cada hoja.
//!
//! Las ordenes de una hoja para Swift son las de la vista (las de cada
//! elemento, los numeros de las cotas, el papel de los marcos) con el
//! pixelado de los mosaicos y lo de dentro de las lupas
//! (`editor_tapado`): un mosaico tambien tapa en lo exportado (F18 de
//! Windows). El grafito va con sus ordenes de siempre.

use pixpin_motor2d::elemento::Elemento;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::exportar::{self as ex, Alcance};
use pixpin_motor2d::exportar_svg::{self, Incrustada, OpcionesSvg};
use pixpin_motor2d::marco;
use pixpin_motor2d::pintado::{self, Orden};

use crate::editor::EditorLienzo;
use crate::lienzo::{LienzoPlano, empaquetar};

/// Que se saca del lienzo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum QueExportar {
    /// Todo el lienzo en una hoja.
    Todo,
    /// Solo lo elegido (y lo que encierran los marcos elegidos).
    Seleccion,
    /// Cada marco en su hoja (todo el lienzo si no hay marcos).
    Marcos,
}

impl QueExportar {
    fn alcance(self) -> Alcance {
        match self {
            QueExportar::Todo => Alcance::Todo,
            QueExportar::Seleccion => Alcance::Seleccion,
            QueExportar::Marcos => Alcance::Marcos,
        }
    }
}

/// Una hoja que sale: su nombre (el del marco, o vacio), su caja en el
/// mundo y lo que se pinta en ella (en el mundo; Swift recorta a la caja).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct HojaExportada {
    pub nombre: String,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub plano: LienzoPlano,
}

/// Las ordenes de `e` al exportar: el papel si es un marco; si no, las suyas
/// con sus numeros de cota y su tapado.
fn ordenes_de(escena: &Escena, e: &Elemento) -> Vec<Orden> {
    if marco::es_marco(e) {
        return marco::ordenes_del_papel(e);
    }
    let mut v = pintado::ordenes(e);
    v.extend(pintado::ordenes_medibles(
        e,
        escena.escala.as_ref(),
        ',',
        escena.fondo,
    ));
    v.extend(crate::editor_tapado::ordenes_tras(e));
    v
}

/// **Las hojas de `que`** en `escena` con `seleccion` elegida.
/// `papel` es el tamano del fondo (un pin, una pagina) si lo hay: entra en
/// la hoja entera, como en Windows. Su imagen no va en el plano: la pinta
/// Swift debajo (`FondoLienzo`).
pub fn hojas(
    escena: &Escena,
    que: QueExportar,
    seleccion: &[u64],
    papel: Option<(f32, f32)>,
) -> Vec<HojaExportada> {
    let elegidos = (que == QueExportar::Seleccion)
        .then(|| marco::con_contenidos(&escena.elementos, seleccion));
    ex::hojas(escena, que.alcance(), seleccion, papel)
        .into_iter()
        .map(|h| {
            let (x0, y0, x1, y1) = h.caja;
            let ordenes: Vec<Orden> = escena
                .a_la_vista()
                .filter(|e| elegidos.as_ref().is_none_or(|ids| ids.contains(&e.id)))
                .filter(|e| {
                    let (a, b, c, d) = e.caja();
                    let m = e.grosor * 2.0 + 40.0;
                    a - m <= x1 && c + m >= x0 && b - m <= y1 && d + m >= y0
                })
                .flat_map(|e| ordenes_de(escena, e))
                .collect();
            HojaExportada {
                nombre: h.nombre,
                x0,
                y0,
                x1,
                y1,
                plano: empaquetar(&ordenes, escena.fondo, Some(h.caja)),
            }
        })
        .collect()
}

/// El tipo de una imagen por sus primeros bytes (lo que admite un `data:`).
pub(crate) fn mime_de(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF8") {
        Some("image/gif")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

#[uniffi::export]
impl EditorLienzo {
    /// **Las hojas que salen** al exportar, compartir o imprimir `que`. Vacio
    /// si no hay nada (un lienzo en blanco, nada elegido).
    pub fn hojas_para_exportar(&self, que: QueExportar) -> Vec<HojaExportada> {
        let st = self.tomar();
        let papel = st.papel.as_ref().map(|(t, _)| *t);
        hojas(&st.escena, que, st.gesto.seleccion.ids(), papel)
    }

    /// **El papel de fondo** del lienzo (un pin, una foto o una pagina de
    /// PDF, `fondo_lienzo` de Windows): mide `ancho` x `alto` en el mundo
    /// desde (0, 0) y entra en lo exportado; `png` es su imagen para el SVG.
    /// Con `ancho` o `alto` a 0 se quita.
    pub fn poner_papel(&self, ancho: f32, alto: f32, png: Option<Vec<u8>>) {
        let mut st = self.tomar();
        st.papel = (ancho > 0.0 && alto > 0.0 && ancho.is_finite() && alto.is_finite())
            .then_some(((ancho, alto), png));
    }

    /// **Cada hoja de `que` como documento SVG** (el que escribe el motor,
    /// el mismo de Windows y del movil), con las fotos dentro. Sin
    /// `transparente`, con el papel del lienzo de fondo.
    pub fn svg_para_exportar(&self, que: QueExportar, transparente: bool) -> Vec<String> {
        // Las fotos: las que traia el lienzo y las pegadas en esta sesion.
        let rutas: std::collections::HashMap<u64, std::path::PathBuf> = {
            let st = self.tomar();
            self.imagenes()
                .into_iter()
                .map(|i| (i.id_objeto, std::path::PathBuf::from(i.ruta)))
                .chain(st.pegadas.iter().map(|p| (p.id_objeto, p.temporal.clone())))
                .collect()
        };
        let incrustada = |id: u64| -> Option<Incrustada> {
            let bytes = std::fs::read(rutas.get(&id)?).ok()?;
            Some(Incrustada {
                mime: mime_de(&bytes)?,
                bytes,
            })
        };
        let st = self.tomar();
        let papel_png = st.papel.as_ref().and_then(|(_, png)| png.clone());
        let incrustada = |id: u64| -> Option<Incrustada> {
            if id == ex::ID_PAPEL {
                return papel_png.clone().map(|bytes| Incrustada {
                    mime: "image/png",
                    bytes,
                });
            }
            incrustada(id)
        };
        let fondo = (!transparente).then_some(st.escena.fondo);
        let papel = st.papel.as_ref().map(|(t, _)| *t);
        ex::hojas(&st.escena, que.alcance(), st.gesto.seleccion.ids(), papel)
            .iter()
            .map(|h| {
                exportar_svg::svg(
                    h,
                    OpcionesSvg {
                        fondo,
                        marcos: false,
                    },
                    &incrustada,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::editor::FasePuntero;

    #[test]
    fn todo_lo_elegido_y_cada_marco_salen_en_sus_hojas() {
        let dir = std::env::temp_dir().join(format!("pixpin-exportar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ed =
            EditorLienzo::de_fichero(dir.join("e.excalidraw").display().to_string(), true).unwrap();
        // Caso negativo: un lienzo en blanco no tiene nada que sacar.
        assert!(ed.hojas_para_exportar(QueExportar::Todo).is_empty());
        let trazar = |h: &str, a: (f32, f32), b: (f32, f32)| {
            ed.herramienta(h.into()).unwrap();
            ed.puntero(FasePuntero::Pulsar, a.0, a.1, None, 0, 1.0);
            ed.puntero(FasePuntero::Mover, b.0, b.1, None, 0, 1.0);
            ed.puntero(FasePuntero::Soltar, b.0, b.1, None, 0, 1.0);
        };
        trazar("rectangulo", (0.0, 0.0), (100.0, 50.0));
        trazar("elipse", (500.0, 500.0), (560.0, 540.0));
        let todo = ed.hojas_para_exportar(QueExportar::Todo);
        assert_eq!(todo.len(), 1);
        assert!(todo[0].x0 < 0.0 && todo[0].x1 > 560.0, "con su margen");
        assert!(!todo[0].plano.datos.is_empty());
        // Sin marcos, «cada marco» es todo.
        assert_eq!(ed.hojas_para_exportar(QueExportar::Marcos).len(), 1);
        // Lo elegido: la elipse recien trazada... nada, al soltar no queda
        // elegida; se elige todo y sale todo.
        ed.herramienta("seleccion".into()).unwrap();
        ed.seleccionar_todo();
        assert_eq!(ed.hojas_para_exportar(QueExportar::Seleccion).len(), 1);
        // Dos marcos: dos hojas.
        trazar("marco", (-20.0, -20.0), (150.0, 80.0));
        trazar("marco", (480.0, 480.0), (600.0, 600.0));
        let marcos = ed.hojas_para_exportar(QueExportar::Marcos);
        assert_eq!(marcos.len(), 2);
        let svg = ed.svg_para_exportar(QueExportar::Marcos, false);
        assert_eq!(svg.len(), 2);
        assert!(svg[0].starts_with("<?xml") && svg[0].contains("<svg"));
        // Con papel de fondo (un pin, una pagina), la hoja entera lo abarca
        // y el SVG lo lleva dentro.
        let png = pixpin_motor2d::exportar::png_rgba(1, 1, &[255, 255, 255, 255]);
        ed.poner_papel(1000.0, 800.0, Some(png));
        let h = &ed.hojas_para_exportar(QueExportar::Todo)[0];
        assert!(h.x0 <= 0.0 && h.y0 <= 0.0 && h.x1 >= 1000.0 && h.y1 >= 800.0);
        assert!(ed.svg_para_exportar(QueExportar::Todo, false)[0].contains("data:image/png"));
        ed.poner_papel(0.0, 0.0, None);
        assert!(
            ed.hojas_para_exportar(QueExportar::Todo)[0].x1 < 1000.0,
            "quitado"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn el_tipo_de_una_foto_se_sabe_por_sus_bytes() {
        assert_eq!(mime_de(b"\x89PNG\r\n"), Some("image/png"));
        assert_eq!(mime_de(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
        assert_eq!(mime_de(b"hola"), None);
    }
}
