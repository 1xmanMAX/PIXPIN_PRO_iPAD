//! **Exportar un plano** (`plano_cad::exportar` de Windows, 13-oct-2026):
//! lo que el nucleo hace para el «Exportar…» del visor (⌘E).
//!
//! - **HTML**: la pagina que se mueve y acerca como el visor, con las capas
//!   que se encienden y apagan, los grosores, las imagenes, las cotas, los
//!   marcos y lo anotado con el motor del lienzo (`pixpin_cad::web`, la
//!   misma pagina que Windows). El plano por capas sale de la cache
//!   ([`super::cache`]): ya se leyo al abrirlo.
//! - **DXF** (de un DWG): lo escribe `opencadcodec` (`leer::a_dxf`).
//!
//! El PDF, el PNG y el JPG los pinta Swift con el pintor de imprimir (los
//! buferes que ya estan en la tarjeta, en vectores), como el PNG de Windows
//! sale de su `imprimir::dibujar`.

use std::path::{Path, PathBuf};

use pixpin_cad::anotado::{GRUPO_COTA, GRUPO_MARCO};

use super::{AnotadoPlano, Plano, PuntoPlano, cache, leer_escena};
use crate::ErrorNucleo;

fn es_dwg(ruta: &Path) -> bool {
    ruta.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("dwg"))
}

/// Las imagenes del plano que se encuentran (su ruta tal cual, junto al
/// plano o por su nombre en su carpeta), en PNG o JPEG (`imagenes` de
/// Windows).
fn imagenes(
    plano: &Path,
    lista: &[pixpin_cad::convertir::ImagenDelPlano],
    origen: [f64; 2],
) -> Vec<pixpin_cad::web::ImagenWeb> {
    let carpeta = plano.parent().unwrap_or(Path::new("."));
    lista
        .iter()
        .filter_map(|i| {
            // La ruta de Windows que trae el plano (`C:\...\foto.png`) no es
            // una ruta aqui: vale su nombre.
            let r = PathBuf::from(i.ruta.replace('\\', "/"));
            let nombre = r.file_name()?.to_owned();
            let ruta = [r.clone(), carpeta.join(&r), carpeta.join(&nombre)]
                .into_iter()
                .find(|x| x.is_file())?;
            let ext = ruta
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            let (mime, bytes) = match ext.as_str() {
                "png" => ("image/png", std::fs::read(&ruta).ok()?),
                "jpg" | "jpeg" => ("image/jpeg", std::fs::read(&ruta).ok()?),
                _ => (
                    "image/png",
                    pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::cargar(&ruta).ok()?)
                        .ok()?,
                ),
            };
            let rel = |p: [f64; 2]| [p[0] - origen[0], p[1] - origen[1]];
            Some(pixpin_cad::web::ImagenWeb {
                capa: i.capa.clone(),
                mime,
                bytes,
                esquina: rel(i.esquina),
                u: i.u,
                v: i.v,
            })
        })
        .collect()
}

impl Plano {
    /// El titulo del plano: el nombre de su fichero.
    fn titulo(&self) -> String {
        self.ruta
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Lo anotado con el lapiz (sin cotas ni marcos, que van aparte) como
    /// SVG del motor, con su encaje respecto a `referencia`.
    fn anotado_en_svg(&self) -> Option<pixpin_cad::web::Anotado> {
        let (_, mut escena) = leer_escena(&self.rutas.tinta)?;
        escena.elementos.retain(|e| {
            !e.borrado && !e.grupos.iter().any(|g| g == GRUPO_COTA || g == GRUPO_MARCO)
        });
        if escena.elementos.is_empty() {
            return None;
        }
        let hoja = pixpin_motor2d::exportar::hojas(
            &escena,
            pixpin_motor2d::exportar::Alcance::Todo,
            &[],
            None,
        )
        .into_iter()
        .next()?;
        let svg = pixpin_motor2d::exportar_svg::svg(
            &hoja,
            pixpin_motor2d::exportar_svg::OpcionesSvg {
                fondo: None,
                marcos: false,
            },
            &|_| None,
        );
        let c = hoja.caja;
        let e = self.encaje_del_fichero();
        Some(pixpin_cad::web::Anotado {
            svg,
            caja: [c.0 as f64, c.1 as f64, c.2 as f64, c.3 as f64],
            u: e.u,
            cero: [e.cero_x, e.cero_y],
        })
    }
}

#[uniffi::export]
impl Plano {
    /// **La pagina web del plano** (`html` de Windows), con las cotas y los
    /// marcos de `anotado` (los del visor ahora) y las capas como se ven
    /// ahora. `claro`: con que fondo abre. Bloquea si el plano por capas ya
    /// no esta en la cache (lo vuelve a leer).
    pub fn pagina_web(&self, anotado: AnotadoPlano, claro: bool) -> Result<String, ErrorNucleo> {
        let mut p = cache::leer_por_capas(&self.raiz, &self.ruta)
            .map_err(|detalle| ErrorNucleo::Argumento { detalle })?;
        // Las capas, encendidas o apagadas como en el visor.
        let enc = self.encendidos_tomados().clone();
        for g in &mut p.grupos {
            if self.fichas.iter().any(|f| f.capa == g.capa) {
                g.encendida = self
                    .fichas
                    .iter()
                    .zip(&enc)
                    .any(|(f, e)| f.capa == g.capa && *e);
            }
        }
        let r = self.referencia;
        let rel = |p: &PuntoPlano| [p.x - r[0], p.y - r[1]];
        let resumen = self.resumen();
        let u = resumen.caja_util;
        let o = pixpin_cad::web::OpcionesWeb {
            titulo: self.titulo(),
            origen: r,
            vista: [u.x0 - r[0], u.y0 - r[1], u.x1 - r[0], u.y1 - r[1]],
            unidades: self.unidades,
            cotas: anotado
                .cotas
                .iter()
                .filter(|c| c.puntos.len() >= 2)
                .map(|c| c.puntos.iter().map(rel).collect())
                .collect(),
            marcos: anotado
                .marcos
                .iter()
                .map(|m| {
                    [
                        m.x0.min(m.x1) - r[0],
                        m.y0.min(m.y1) - r[1],
                        m.x0.max(m.x1) - r[0],
                        m.y0.max(m.y1) - r[1],
                    ]
                })
                .collect(),
            anotado: self.anotado_en_svg(),
            imagenes: imagenes(&self.ruta, &p.imagenes, r),
            claro,
            textos: None,
        };
        Ok(pixpin_cad::web::pagina(&p.grupos, &o))
    }

    /// Si el plano es un DWG (se ofrece «DXF» al exportar).
    pub fn es_dwg(&self) -> bool {
        es_dwg(&self.ruta)
    }

    /// **Escribe el plano en DXF** en `salida` (solo de un DWG; un DXF ya lo
    /// es: se exporta el original).
    pub fn exportar_dxf(&self, salida: String) -> Result<(), ErrorNucleo> {
        if !self.es_dwg() {
            return Err(ErrorNucleo::Argumento {
                detalle: "el plano ya es un DXF".into(),
            });
        }
        pixpin_cad::leer::a_dxf(&self.ruta, Path::new(&salida))
            .map_err(|detalle| ErrorNucleo::Argumento { detalle })
    }

    /// El fichero del plano.
    pub fn ruta(&self) -> String {
        self.ruta.display().to_string()
    }
}

#[cfg(test)]
mod pruebas {
    use super::super::pruebas::abrir;
    use super::super::{CajaPlano, CotaPlano};
    use super::*;
    use pixpin_motor2d::{ColorRgba, Elemento, Figura, Punto2, excalidraw};

    /// La cota y el marco de las pruebas de Windows: lo ancho de abajo y el
    /// plano entero.
    fn anotado(p: &Plano) -> AnotadoPlano {
        let u = p.resumen().caja_util;
        AnotadoPlano {
            cotas: vec![CotaPlano {
                puntos: vec![
                    PuntoPlano { x: u.x0, y: u.y0 },
                    PuntoPlano { x: u.x1, y: u.y0 },
                ],
            }],
            marcos: vec![u],
        }
    }

    /// Una raya roja gorda en la capa, como la deja el motor.
    fn anotar_con_el_lapiz(p: &Plano) {
        let mut escena = pixpin_motor2d::Escena::nueva();
        escena.anadir(Elemento {
            x: 0.0,
            y: 0.0,
            ancho: 40.0,
            alto: 20.0,
            trazo: ColorRgba::opaco(0.9, 0.1, 0.1),
            grosor: 3.0,
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(40.0, 20.0)],
            },
            ..Default::default()
        });
        let l = excalidraw::con_escena(&excalidraw::Lienzo::vacio(), &escena);
        std::fs::create_dir_all(p.rutas.tinta.parent().unwrap()).unwrap();
        std::fs::write(&p.rutas.tinta, excalidraw::escribir(&l)).unwrap();
    }

    #[test]
    fn el_html_lleva_las_capas_los_grosores_las_cotas_y_lo_anotado() {
        let (d, p) = abrir("html");
        anotar_con_el_lapiz(&p);
        let h = p.pagina_web(anotado(&p), true).unwrap();
        assert!(h.starts_with("<!DOCTYPE html>"));
        assert!(h.contains("<title>planta</title>"));
        // Las dos capas, la apagada apagada; el grosor de 0,50 mm.
        assert!(h.contains(r#"{"n":"Muros","on":true"#), "sin la capa Muros");
        assert!(
            h.contains(r#"{"n":"Ejes","on":false"#),
            "sin la capa Ejes apagada"
        );
        assert!(h.contains(r#""g":50"#), "sin el grosor");
        // La cota, el marco y lo anotado (el SVG del motor y su encaje).
        assert!(h.contains(r#""cotas":[["#) && h.contains(r#""marcos":[["#));
        assert!(h.contains("<svg") && !h.contains(r#""anotado":null"#));
        // La capa encendida en el visor sale encendida.
        assert!(p.poner_capa("Ejes".into(), true));
        let h = p
            .pagina_web(
                AnotadoPlano {
                    cotas: vec![],
                    marcos: vec![],
                },
                false,
            )
            .unwrap();
        assert!(h.contains(r#"{"n":"Ejes","on":true"#));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sin_lapiz_la_pagina_no_lleva_anotado_y_un_marco_al_reves_se_endereza() {
        let (d, p) = abrir("html-limpio");
        let u = p.resumen().caja_util;
        let al_reves = AnotadoPlano {
            cotas: vec![CotaPlano {
                puntos: vec![PuntoPlano { x: u.x0, y: u.y0 }],
            }],
            marcos: vec![CajaPlano {
                x0: u.x1,
                y0: u.y1,
                x1: u.x0,
                y1: u.y0,
            }],
        };
        let h = p.pagina_web(al_reves, true).unwrap();
        assert!(h.contains(r#""anotado":null"#));
        // Una cota de un solo punto no es cota.
        assert!(h.contains(r#""cotas":[]"#));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_dxf_no_se_exporta_a_dxf_y_un_dwg_si_lo_intenta() {
        let (d, p) = abrir("dxf");
        assert!(!p.es_dwg());
        assert!(
            p.exportar_dxf(d.join("copia.dxf").display().to_string())
                .is_err()
        );
        // El DXF del plano se vuelve a leer (la prueba de Windows).
        let r = d.join("copia.dxf");
        pixpin_cad::leer::a_dxf(&p.ruta, &r).unwrap();
        let (m, _) = pixpin_cad::convertir::convertir_fichero(&r).unwrap();
        assert!(!m.vacio());
        // Caso negativo: un destino donde no se puede escribir.
        assert!(pixpin_cad::leer::a_dxf(&p.ruta, &d.join("no-existe").join("x.dxf")).is_err());
        assert!(es_dwg(Path::new("/a/Planta.DWG")) && !es_dwg(Path::new("/a/planta.dxf")));
        let _ = std::fs::remove_dir_all(&d);
    }
}
