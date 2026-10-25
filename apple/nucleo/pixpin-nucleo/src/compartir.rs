//! **Compartir y exportar comun** (D1): lo que la hoja de compartir del Mac
//! (`PixPinKit/Compartir/`) le pide al nucleo, lo mismo que hace
//! `apps/pixpin/src/compartir.rs` de Windows.
//!
//! En Windows todo lo que se comparte se convierte en hojas del motor y de
//! hojas salen la pagina web, el PDF, el SVG y el PNG. En el Mac el PDF, el
//! PNG y el JPG los pinta Swift con el pintor de siempre (`PintorDeHojas`,
//! como el lienzo), asi que aqui solo esta lo que no se puede pintar:
//!
//! - **las hojas de un dibujo del chat** ([`Nucleo::hojas_de_dibujo`]): el
//!   lienzo entero y cada marco (`anadir_lienzo` de Windows), con sus
//!   ordenes para pintarlas y su SVG, el del motor, con las fotos dentro;
//! - **el `.excalidraw` limpio** de un dibujo, para el «Editable» y para que
//!   viaje dentro de la pagina web ([`Nucleo::excalidraw_de_dibujo`]);
//! - **la pagina web** ([`pagina_web_compartida`]): la del movil
//!   (`exportar_html`), con sus mandos, de las hojas que Swift ya tiene en
//!   SVG;
//! - **el `.pixpin` de un proyecto** ([`Nucleo::paquete_de_proyecto`]),
//!   para abrirlo en otro PixPin o en el movil.

use std::collections::HashMap;
use std::path::PathBuf;

use pixpin_motor2d::excalidraw;
use pixpin_motor2d::exportar::{self as ex, Alcance};
use pixpin_motor2d::exportar_html::{self, HojaWeb};
use pixpin_motor2d::exportar_svg::{self, Incrustada, OpcionesSvg};
use pixpin_motor2d::{Escena, marco};

use crate::chats::validar_chat;
use crate::editor_exportar::{QueExportar, hojas as hojas_con_ordenes, mime_de};
use crate::lienzo::LienzoPlano;
use crate::{ErrorNucleo, Nucleo};

/// **Una hoja de un dibujo para compartir**: la pagina de la hoja (el lienzo
/// entero o un marco), sus ordenes para pintarla y su SVG.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct HojaDeDibujo {
    /// El nombre del marco; vacio en el lienzo entero.
    pub nombre: String,
    /// Si es un marco (va sangrado debajo del lienzo entero).
    pub es_marco: bool,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub plano: LienzoPlano,
    /// La hoja como documento SVG del motor, con el papel del lienzo de
    /// fondo y las fotos dentro.
    pub svg: String,
    /// El papel, `#rrggbb` (lo que la pagina web pone detras).
    pub fondo: String,
}

/// Una hoja de la pagina web, ya en SVG: la hace Swift (una foto, una
/// pagina de texto, una pagina de PDF) o sale de [`HojaDeDibujo::svg`].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HojaParaWeb {
    pub nombre: String,
    pub svg: String,
    /// `#rrggbb`.
    pub fondo: String,
}

/// **La pagina web de un solo archivo** con estas hojas: la del movil y la
/// de Windows (`exportar_html::paginas`), con todos sus mandos (lapiz,
/// medir, presentar, imprimir y guardar lo anotado). `excalidraw` viaja
/// dentro para volver a editar el dibujo (solo cuando lo compartido es UN
/// lienzo). `None` sin hojas.
#[uniffi::export]
pub fn pagina_web_compartida(
    titulo: String,
    hojas: Vec<HojaParaWeb>,
    excalidraw: Option<String>,
) -> Option<String> {
    let hojas: Vec<HojaWeb> = hojas
        .into_iter()
        .map(|h| HojaWeb {
            nombre: h.nombre,
            svg: h.svg,
            fondo: h.fondo,
        })
        .collect();
    exportar_html::paginas(
        &hojas,
        &titulo,
        &exportar_html::nombre_de_fichero(&titulo),
        exportar_html::Opciones::default(),
        excalidraw.as_deref(),
    )
}

/// El nombre de fichero de un titulo, el mismo que Windows
/// (`compartir::nombre_de_fichero`): sin los signos que un disco no admite.
#[uniffi::export]
pub fn nombre_de_fichero_compartido(titulo: String) -> String {
    exportar_html::nombre_de_fichero(titulo.trim_end_matches(['.', ' ']))
}

/// Un dibujo del chat leido: su escena y el fichero de cada foto.
fn escena_de_dibujo(
    n: &Nucleo,
    chat: &str,
    dibujo: &str,
) -> Result<(Escena, HashMap<u64, PathBuf>), ErrorNucleo> {
    let lienzo = n.abrir_lienzo(chat.to_string(), dibujo.to_string())?;
    let fotos = n
        .imagenes_del_lienzo(chat.to_string(), lienzo.clone())?
        .into_iter()
        .map(|i| (i.id_objeto, PathBuf::from(i.ruta)))
        .collect();
    Ok((lienzo.escena().clone(), fotos))
}

/// **Las hojas de una escena para compartir**: la entera y, si tiene marcos
/// con algo dentro, cada marco (`anadir_lienzo` de Windows). Sin marcos,
/// «cada marco» devuelve el lienzo entero otra vez, y no se repite.
pub(crate) fn hojas_de_escena(escena: &Escena, fotos: &HashMap<u64, PathBuf>) -> Vec<HojaDeDibujo> {
    let incrustada = |id: u64| -> Option<Incrustada> {
        let bytes = std::fs::read(fotos.get(&id)?).ok()?;
        Some(Incrustada {
            mime: mime_de(&bytes)?,
            bytes,
        })
    };
    let svg_de = |h: &ex::Hoja| {
        exportar_svg::svg(
            h,
            OpcionesSvg {
                fondo: Some(escena.fondo),
                marcos: false,
            },
            &incrustada,
        )
    };
    let fondo = ex::hex(escena.fondo);
    let Some(entera) = ex::hojas(escena, Alcance::Todo, &[], None)
        .into_iter()
        .next()
    else {
        return Vec::new();
    };
    let Some(ordenes_entera) = hojas_con_ordenes(escena, QueExportar::Todo, &[], None)
        .into_iter()
        .next()
    else {
        return Vec::new();
    };
    let hay_marcos = !marco::hojas_en_orden(&escena.elementos).is_empty();
    let mut marcos = if hay_marcos {
        ex::hojas(escena, Alcance::Marcos, &[], None)
    } else {
        Vec::new()
    };
    if marcos.len() == 1 && marcos[0].caja == entera.caja && marcos[0].nombre.is_empty() {
        marcos.clear();
    }
    let ordenes_marcos = if marcos.is_empty() {
        Vec::new()
    } else {
        hojas_con_ordenes(escena, QueExportar::Marcos, &[], None)
    };
    let mut v = vec![HojaDeDibujo {
        nombre: String::new(),
        es_marco: false,
        x0: ordenes_entera.x0,
        y0: ordenes_entera.y0,
        x1: ordenes_entera.x1,
        y1: ordenes_entera.y1,
        plano: ordenes_entera.plano,
        svg: svg_de(&entera),
        fondo: fondo.clone(),
    }];
    for (h, o) in marcos.iter().zip(ordenes_marcos) {
        v.push(HojaDeDibujo {
            nombre: h.nombre.clone(),
            es_marco: true,
            x0: o.x0,
            y0: o.y0,
            x1: o.x1,
            y1: o.y1,
            plano: o.plano,
            svg: svg_de(h),
            fondo: fondo.clone(),
        });
    }
    v
}

#[uniffi::export]
impl Nucleo {
    /// **Las hojas de un dibujo del chat** (la `referencia` de un `DIBUJO`)
    /// para la hoja de compartir: el lienzo entero primero y cada marco
    /// detras. Vacio si no hay nada dibujado. Bloqueante (lee las fotos).
    pub fn hojas_de_dibujo(
        &self,
        chat: String,
        dibujo: String,
    ) -> Result<Vec<HojaDeDibujo>, ErrorNucleo> {
        validar_chat(&chat)?;
        let (escena, fotos) = escena_de_dibujo(self, &chat, &dibujo)?;
        Ok(hojas_de_escena(&escena, &fotos))
    }

    /// **El dibujo como `.excalidraw` limpio** (`excalidraw_de` de Windows):
    /// el «Editable» de la hoja y lo que viaja dentro de la pagina web.
    pub fn excalidraw_de_dibujo(
        &self,
        chat: String,
        dibujo: String,
    ) -> Result<String, ErrorNucleo> {
        validar_chat(&chat)?;
        let lienzo = self.abrir_lienzo(chat, dibujo)?;
        Ok(excalidraw::escribir(&excalidraw::con_escena(
            &excalidraw::Lienzo::vacio(),
            lienzo.escena(),
        )))
    }

    /// **Un proyecto entero como `.pixpin`** (`Entero::Proyectos` de
    /// Windows): todo lo de su carpeta, para abrirlo en otro PixPin o en el
    /// movil. Bloqueante: puede pesar mucho.
    pub fn paquete_de_proyecto(&self, chat: String) -> Result<Vec<u8>, ErrorNucleo> {
        validar_chat(&chat)?;
        match pixpin_proyecto::almacen::empaquetar(self.ruta(), &chat, crate::ahora_ms()) {
            Ok(b) => Ok(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(ErrorNucleo::NoExiste { que: chat })
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un lienzo con un rectangulo y, si `con_marco`, un marco alrededor de
    /// otro.
    fn lienzo(con_marco: bool) -> String {
        let marco = if con_marco {
            r##",{"id":"m","type":"frame","x":400,"y":0,"width":200,"height":150,"angle":0,
              "strokeColor":"#bbb","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":1,
              "strokeStyle":"solid","roughness":0,"opacity":100,"name":"Portada","isDeleted":false},
              {"id":"e","type":"ellipse","x":450,"y":30,"width":60,"height":40,"angle":0,
              "strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":2,
              "strokeStyle":"solid","roughness":0,"opacity":100,"frameId":"m","isDeleted":false}"##
        } else {
            ""
        };
        format!(
            r##"{{"type":"excalidraw","version":2,"source":"pruebas","elements":[
              {{"id":"r","type":"rectangle","x":0,"y":0,"width":100,"height":50,"angle":0,
              "strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":2,
              "strokeStyle":"solid","roughness":0,"opacity":100,"isDeleted":false}}{marco}],
              "appState":{{"viewBackgroundColor":"#ffffff"}},"files":{{}}}}"##
        )
    }

    fn nucleo(nombre: &str) -> (std::sync::Arc<Nucleo>, std::path::PathBuf, String) {
        let d =
            std::env::temp_dir().join(format!("pixpin-compartir-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        let chat = n.crear_chat("Compartir".into()).unwrap();
        (n, d, chat.id)
    }

    #[test]
    fn un_dibujo_sale_entero_y_cada_marco_detras() {
        let (n, d, chat) = nucleo("dibujo");
        let m = n.crear_lienzo(chat.clone(), None).unwrap();
        let referencia = m.referencia.unwrap();
        let ruta = pixpin_proyecto::almacen::lienzo(&d, &chat, &referencia);
        // Caso negativo: un lienzo en blanco no tiene hojas.
        assert!(
            n.hojas_de_dibujo(chat.clone(), referencia.clone())
                .unwrap()
                .is_empty()
        );

        std::fs::write(&ruta, lienzo(false)).unwrap();
        let sin = n.hojas_de_dibujo(chat.clone(), referencia.clone()).unwrap();
        assert_eq!(sin.len(), 1, "sin marcos, solo el entero (no se repite)");
        assert!(!sin[0].es_marco && sin[0].svg.starts_with("<?xml") && sin[0].svg.contains("<svg"));
        assert!(!sin[0].plano.datos.is_empty() && sin[0].x1 > sin[0].x0);
        assert_eq!(sin[0].fondo, "#ffffff");

        std::fs::write(&ruta, lienzo(true)).unwrap();
        let con = n.hojas_de_dibujo(chat.clone(), referencia.clone()).unwrap();
        assert_eq!(con.len(), 2, "el entero y el marco");
        assert!(!con[0].es_marco && con[1].es_marco);
        assert_eq!(con[1].nombre, "Portada");
        assert!(
            con[1].x1 - con[1].x0 < con[0].x1 - con[0].x0,
            "el marco es mas pequeno"
        );

        // El editable se vuelve a leer.
        let json = n
            .excalidraw_de_dibujo(chat.clone(), referencia.clone())
            .unwrap();
        assert!(excalidraw::leer(&json).is_ok());
        // Casos negativos: un dibujo que no esta y un chat que no vale.
        assert!(n.hojas_de_dibujo(chat.clone(), "nada".into()).is_err());
        assert!(n.hojas_de_dibujo("../x".into(), referencia).is_err());
    }

    #[test]
    fn la_pagina_web_lleva_las_hojas_y_el_editable() {
        let hoja = HojaParaWeb {
            nombre: "Hoja <1>".into(),
            svg: "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\"><rect width=\"10\" height=\"10\"/></svg>".into(),
            fondo: "#ffffff".into(),
        };
        let web = pagina_web_compartida(
            "Viaje".into(),
            vec![hoja],
            Some("{\"type\":\"excalidraw\"}".into()),
        )
        .unwrap();
        assert!(web.starts_with("<!DOCTYPE html>"));
        assert!(web.contains("<rect width=\"10\""));
        assert!(web.contains("excalidraw"));
        assert!(!web.contains("Hoja <1>"), "los nombres van escapados");
        // Caso negativo: sin hojas no hay pagina.
        assert!(pagina_web_compartida("Viaje".into(), vec![], None).is_none());
    }

    #[test]
    fn el_nombre_de_fichero_quita_lo_que_no_vale() {
        assert_eq!(nombre_de_fichero_compartido("a/b: c?.".into()), "a-b- c-");
        assert_eq!(nombre_de_fichero_compartido("  ".into()), "dibujo");
    }

    #[test]
    fn un_proyecto_sale_como_pixpin() {
        let (n, _d, chat) = nucleo("paquete");
        n.escribir_nota(chat.clone(), "hola".into(), None).unwrap();
        let bytes = n.paquete_de_proyecto(chat).unwrap();
        assert!(bytes.starts_with(b"PK"), "un .pixpin es un zip");
        // Caso negativo: uno que no esta.
        assert!(matches!(
            n.paquete_de_proyecto("noesta".into()),
            Err(ErrorNucleo::NoExiste { .. })
        ));
    }
}
