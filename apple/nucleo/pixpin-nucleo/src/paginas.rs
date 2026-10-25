//! **Clip → «Una pagina de un proyecto»** (`guardarPagina` y el dialogo
//! `paginasDe` de `MensajesActivity.kt:2154-2211, 5286-5341`).
//!
//! El movil pregunta primero el proyecto y luego la hoja («Pagina N»), y
//! adjunta:
//!
//! - una hoja de PDF como mensaje `Pagina`, con el numero en el nombre
//!   («Plano · Pagina 7») porque es lo que viaja: la cita, la lista, el
//!   nombre del fichero al compartir;
//! - una hoja de lienzo como `Dibujo`, y si es **de otro proyecto con su
//!   propia copia**: compartir el mismo dibujo haria que dibujar aqui tocara
//!   tambien el del otro (14-sep-2026).
//!
//! ## Lo que cambia en Windows
//!
//! Aqui una pagina se pinta con el PDF **del proyecto en el que esta el
//! mensaje** (`pagina_del_pdf`), no con una ruta absoluta como en el movil.
//! Una pagina del PDF de OTRO proyecto apuntaria entonces al documento
//! equivocado, asi que se lleva **pintada**: la pagina hecha imagen y
//! guardada en `archivos/` de este proyecto, como las paginas que se
//! extraian antes. Se ve, se abre y se dibuja encima como cualquier foto.
//!
//! Lo puro (que hojas se ofrecen y que mensaje sale) va separado de lo que
//! toca el disco, para comprobarlo sin carpetas.
//!
//! ## En el Mac (18-oct)
//!
//! Lo mismo. La pagina de OTRO proyecto la pinta Swift con PDFKit
//! ([`Nucleo::pagina_por_pintar`] dice que PDF y que hoja) y la pasa en PNG
//! a [`Nucleo::adjuntar_pagina`].

use std::path::Path;

use pixpin_proyecto::{Proyecto, almacen, cuaderno};

use crate::chats::{hora_unica, validar_chat};
use crate::{ErrorNucleo, MensajeChat, Nucleo};

/// Una hoja que se puede adjuntar, con su numero para el rotulo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elegible {
    pub hoja: String,
    /// «Pagina N»: la del PDF si la tiene, y si no su puesto en el proyecto,
    /// como el movil (`hoja.pagina?.let { it + 1 } ?: indexOf(hoja) + 1`).
    pub n: u32,
    pub pagina: Option<u32>,
    pub dibujo: Option<String>,
}

/// El `proyecto.json` de un proyecto, si lo tiene y se entiende.
pub fn leer(raiz: &Path, id: &str) -> Option<Proyecto> {
    let texto = std::fs::read_to_string(almacen::carpeta(raiz, id).join("proyecto.json")).ok()?;
    serde_json::from_str(&texto).ok()
}

/// Las hojas que se ofrecen, en el orden del proyecto.
///
/// Solo las que son pagina de PDF o lienzo: una nota o un croquis no tienen
/// nada que ver como pagina, y el movil las deja caer sin decir nada
/// (`val dibujo = hoja.dibujo ?: return`). Mejor no ofrecerlas.
pub fn elegibles(p: &Proyecto) -> Vec<Elegible> {
    p.hojas
        .iter()
        .enumerate()
        .filter(|(_, h)| h.pagina.is_some() || h.dibujo.as_deref().is_some_and(|d| !d.is_empty()))
        .map(|(n, h)| Elegible {
            hoja: h.id.clone(),
            n: h.pagina.map_or(n as u32 + 1, |p| p + 1),
            pagina: h.pagina,
            dibujo: h.dibujo.clone().filter(|d| !d.is_empty()),
        })
        .collect()
}

/// Que se adjunta con una hoja elegida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Una pagina del PDF de este mismo proyecto: se apunta a ella.
    Pagina { pagina: u32, dibujo: Option<String> },
    /// Una pagina del PDF de otro proyecto: se lleva pintada.
    PaginaPintada { pagina: u32 },
    /// Un lienzo de este mismo proyecto: se apunta a el.
    Dibujo { dibujo: String },
    /// Un lienzo de otro proyecto: se copia con un id nuevo.
    DibujoCopiado { dibujo: String },
}

/// Decide que se adjunta. `mismo` es si la hoja es del proyecto abierto.
pub fn plan(e: &Elegible, mismo: bool) -> Option<Plan> {
    match (e.pagina, &e.dibujo, mismo) {
        (Some(pagina), dibujo, true) => Some(Plan::Pagina {
            pagina,
            dibujo: dibujo.clone(),
        }),
        (Some(pagina), _, false) => Some(Plan::PaginaPintada { pagina }),
        (None, Some(d), true) => Some(Plan::Dibujo { dibujo: d.clone() }),
        (None, Some(d), false) => Some(Plan::DibujoCopiado { dibujo: d.clone() }),
        (None, None, _) => None,
    }
}

/// Una hoja que se ofrece en el menu.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaElegible {
    pub hoja: String,
    /// «Pagina N».
    pub numero: u32,
    /// Si es un lienzo (si no, una pagina del PDF).
    pub es_lienzo: bool,
}

/// La pagina de otro proyecto que hay que pintar antes de adjuntarla.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaPorPintar {
    pub pdf: String,
    /// Desde 0.
    pub pagina: u32,
}

/// Adjunta la hoja `hoja` del proyecto `desde` a la conversacion `hacia`
/// (Windows). Una pagina de otro proyecto pide su `png` ya pintado.
pub fn adjuntar(
    raiz: &Path,
    desde: &str,
    hacia: &str,
    hoja: &str,
    rotulo: impl Fn(u32) -> String,
    sello: &cuaderno::Sello,
) -> Result<cuaderno::Mensaje, String> {
    adjuntar_con(raiz, desde, hacia, hoja, rotulo, sello, None)
}

pub fn adjuntar_con(
    raiz: &Path,
    desde: &str,
    hacia: &str,
    hoja: &str,
    rotulo: impl Fn(u32) -> String,
    sello: &cuaderno::Sello,
    png: Option<Vec<u8>>,
) -> Result<cuaderno::Mensaje, String> {
    let p = leer(raiz, desde).ok_or("el proyecto no tiene hojas")?;
    let e = elegibles(&p).into_iter().find(|e| e.hoja == hoja).ok_or("esa hoja ya no esta")?;
    let nombre = format!("{} · {}", p.nombre, rotulo(e.n));
    let plan = plan(&e, desde == hacia).ok_or("esa hoja no es una pagina")?;
    let mut m = match plan {
        Plan::Pagina { pagina, dibujo } => {
            let mut m = cuaderno::Mensaje::adjunto(cuaderno::Clase::Pagina, &nombre, "", 0, sello);
            m.ruta = None;
            m.pagina = Some(pagina);
            m.referencia = dibujo;
            m
        }
        Plan::PaginaPintada { .. } => {
            let png = png.ok_or("esa pagina hay que pintarla antes (pagina_por_pintar)")?;
            let ruta = almacen::guardar_adjunto(raiz, hacia, &format!("{nombre}.png"), &png).map_err(|e| e.to_string())?;
            cuaderno::Mensaje::adjunto(cuaderno::Clase::Imagen, &nombre, &ruta, png.len() as i64, sello)
        }
        Plan::Dibujo { dibujo } => lienzo(raiz, hacia, &nombre, dibujo, sello),
        Plan::DibujoCopiado { dibujo } => {
            let nuevo = format!("dib-{}", sello.cuando);
            copiar_lienzo(raiz, desde, hacia, &dibujo, &nuevo)?;
            lienzo(raiz, hacia, &nombre, nuevo, sello)
        }
    };
    m.proyecto = Some(hacia.to_string());
    cuaderno::anadir(&almacen::carpeta(raiz, hacia), &m).map_err(|e| e.to_string())?;
    Ok(m)
}

fn rotulo(n: u32) -> String {
    format!("Página {n}")
}

#[uniffi::export]
impl Nucleo {
    /// Las hojas de un proyecto que se pueden adjuntar (paginas de su PDF y
    /// lienzos), en su orden. Vacio si no tiene.
    pub fn paginas_de_proyecto(&self, proyecto: String) -> Vec<PaginaElegible> {
        if validar_chat(&proyecto).is_err() {
            return Vec::new();
        }
        leer(self.ruta(), &proyecto)
            .map(|p| elegibles(&p))
            .unwrap_or_default()
            .into_iter()
            .map(|e| PaginaElegible { es_lienzo: e.pagina.is_none(), hoja: e.hoja, numero: e.n })
            .collect()
    }

    /// Si adjuntar esa hoja en `hacia` pide pintarla antes (una pagina del
    /// PDF de otro proyecto): que PDF y que hoja. `None`: no hace falta.
    pub fn pagina_por_pintar(&self, hacia: String, desde: String, hoja: String) -> Option<PaginaPorPintar> {
        validar_chat(&desde).ok()?;
        let p = leer(self.ruta(), &desde)?;
        let e = elegibles(&p).into_iter().find(|e| e.hoja == hoja)?;
        match plan(&e, desde == hacia)? {
            Plan::PaginaPintada { pagina } => Some(PaginaPorPintar {
                pdf: pixpin_proyecto::vista::documento_del_proyecto(self.ruta(), &desde)?.display().to_string(),
                pagina,
            }),
            _ => None,
        }
    }

    /// **Adjunta la hoja `hoja` del proyecto `desde` al chat `hacia`**
    /// (Clip → «Página de un proyecto»). `png`: la pagina ya pintada, si
    /// [`Nucleo::pagina_por_pintar`] lo pidio.
    pub fn adjuntar_pagina(
        &self,
        hacia: String,
        desde: String,
        hoja: String,
        png: Option<Vec<u8>>,
    ) -> Result<MensajeChat, ErrorNucleo> {
        validar_chat(&hacia)?;
        validar_chat(&desde)?;
        let raiz = self.ruta();
        let mal = |detalle: &str| ErrorNucleo::Argumento { detalle: detalle.into() };
        let carpeta = almacen::carpeta(raiz, &hacia);
        let fichero = carpeta.join("guardados.jsonl");
        let aparato = self.identidad()?.yo.codigo();
        let mut cache = self.cache_tomada();
        let numero = cache.indice(&fichero)?.map_or(0, |i| i.mayor_numero) + 1;
        let sello = cuaderno::Sello { cuando: hora_unica(), numero, aparato, proyecto: hacia.clone() };
        let m = adjuntar_con(raiz, &desde, &hacia, &hoja, rotulo, &sello, png).map_err(|d| mal(&d))?;
        cache.quitar(&fichero);
        Ok(MensajeChat::from(m))
    }
}

/// El mensaje de un lienzo, como lo escribe `hojas_que_faltan`.
fn lienzo(
    raiz: &Path,
    hacia: &str,
    nombre: &str,
    dibujo: String,
    sello: &cuaderno::Sello,
) -> cuaderno::Mensaje {
    let ruta = format!("lienzos/{dibujo}.excalidraw");
    let bytes = std::fs::metadata(almacen::lienzo(raiz, hacia, &dibujo))
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    let mut m = cuaderno::Mensaje::adjunto(cuaderno::Clase::Dibujo, nombre, &ruta, bytes, sello);
    m.referencia = Some(dibujo);
    m
}

/// Copia un lienzo de un proyecto a otro con id nuevo, y las fotos que
/// lleva dentro (`imagenes/…`): sin ellas, alli seria una hoja con huecos.
fn copiar_lienzo(
    raiz: &Path,
    desde: &str,
    hacia: &str,
    id: &str,
    nuevo: &str,
) -> Result<(), String> {
    let origen = almacen::lienzo(raiz, desde, id);
    let texto = std::fs::read_to_string(&origen).map_err(|e| e.to_string())?;
    let destino = almacen::lienzo(raiz, hacia, nuevo);
    if let Some(padre) = destino.parent() {
        std::fs::create_dir_all(padre).map_err(|e| e.to_string())?;
    }
    std::fs::write(&destino, &texto).map_err(|e| e.to_string())?;
    let (de, a) = (almacen::carpeta(raiz, desde), almacen::carpeta(raiz, hacia));
    for rel in rutas_de_fotos(&texto) {
        let (o, d) = (de.join(&rel), a.join(&rel));
        if o.is_file() && !d.exists() {
            if let Some(padre) = d.parent() {
                let _ = std::fs::create_dir_all(padre);
            }
            if let Err(e) = std::fs::copy(&o, &d) {
                let _ = e;
            }
        }
    }
    Ok(())
}

/// Las rutas relativas de las fotos de un `.excalidraw` (`files.*.path`).
/// Solo las que se quedan dentro de la carpeta del proyecto: una ruta con
/// `..` o absoluta copiaria algo de fuera.
fn rutas_de_fotos(texto: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(texto) else {
        return Vec::new();
    };
    v.get("files")
        .and_then(|f| f.as_object())
        .map(|f| {
            f.values()
                .filter_map(|x| x.get("path")?.as_str())
                .filter(|r| {
                    !r.is_empty()
                        && !r.contains("..")
                        && !Path::new(r).is_absolute()
                        && !r.contains(':')
                })
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::Hoja;

    fn hoja(id: &str, pagina: Option<u32>, dibujo: Option<&str>, nota: Option<&str>) -> Hoja {
        Hoja {
            id: id.into(),
            pagina,
            dibujo: dibujo.map(str::to_string),
            nota: nota.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn se_ofrecen_las_paginas_y_los_lienzos_y_no_las_notas() {
        let p = Proyecto {
            hojas: vec![
                hoja("a", Some(6), Some("d1"), None),
                hoja("b", None, None, Some("una nota")),
                hoja("c", None, Some("d2"), None),
                hoja("d", None, Some(""), None),
            ],
            ..Default::default()
        };
        let v = elegibles(&p);
        assert_eq!(v.len(), 2, "la nota y el dibujo vacio no son paginas");
        assert_eq!(v[0].n, 7, "la de PDF lleva su numero de pagina");
        assert_eq!(v[1].n, 3, "el lienzo, su puesto en el proyecto");
        assert_eq!(v[1].dibujo.as_deref(), Some("d2"));
    }

    #[test]
    fn de_otro_proyecto_la_pagina_va_pintada_y_el_lienzo_copiado() {
        let pagina = Elegible {
            hoja: "a".into(),
            n: 3,
            pagina: Some(2),
            dibujo: Some("d".into()),
        };
        assert_eq!(
            plan(&pagina, true),
            Some(Plan::Pagina {
                pagina: 2,
                dibujo: Some("d".into())
            })
        );
        assert_eq!(
            plan(&pagina, false),
            Some(Plan::PaginaPintada { pagina: 2 })
        );
        let lienzo = Elegible {
            hoja: "b".into(),
            n: 1,
            pagina: None,
            dibujo: Some("d".into()),
        };
        assert_eq!(
            plan(&lienzo, true),
            Some(Plan::Dibujo { dibujo: "d".into() })
        );
        assert_eq!(
            plan(&lienzo, false),
            Some(Plan::DibujoCopiado { dibujo: "d".into() })
        );
        // Caso negativo: sin pagina ni dibujo no hay nada que adjuntar.
        let nada = Elegible {
            hoja: "c".into(),
            n: 1,
            pagina: None,
            dibujo: None,
        };
        assert_eq!(plan(&nada, true), None);
    }

    #[test]
    fn un_lienzo_de_otro_proyecto_se_copia_con_sus_fotos_y_con_id_nuevo() {
        let raiz = std::env::temp_dir().join(format!("pixpin-paginas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let desde = almacen::carpeta(&raiz, "p1");
        std::fs::create_dir_all(desde.join("lienzos")).unwrap();
        std::fs::create_dir_all(desde.join("imagenes")).unwrap();
        std::fs::write(desde.join("imagenes").join("f1"), b"png").unwrap();
        let lienzo = r#"{"type":"excalidraw","elements":[],"files":{"f1":{"path":"imagenes/f1"},"f2":{"path":"../fuera"}}}"#;
        std::fs::write(almacen::lienzo(&raiz, "p1", "d1"), lienzo).unwrap();
        let p = Proyecto {
            id: "p1".into(),
            nombre: "Plano".into(),
            hojas: vec![hoja("h1", None, Some("d1"), None)],
            ..Default::default()
        };
        std::fs::write(
            desde.join("proyecto.json"),
            serde_json::to_string(&p).unwrap(),
        )
        .unwrap();
        std::fs::create_dir_all(almacen::carpeta(&raiz, "p2")).unwrap();
        let sello = cuaderno::Sello {
            cuando: 42,
            numero: 1,
            aparato: "K7Q2".into(),
            proyecto: "p2".into(),
        };
        let m = adjuntar(&raiz, "p1", "p2", "h1", |n| format!("Pagina {n}"), &sello).unwrap();
        assert_eq!(m.clase, Some(cuaderno::Clase::Dibujo));
        assert_eq!(m.nombre, "Plano · Pagina 1");
        assert_eq!(
            m.referencia.as_deref(),
            Some("dib-42"),
            "id nuevo: no comparte dibujo"
        );
        assert!(almacen::lienzo(&raiz, "p2", "dib-42").is_file());
        assert!(
            almacen::carpeta(&raiz, "p2")
                .join("imagenes")
                .join("f1")
                .is_file()
        );
        // Caso negativo: la ruta que sale de la carpeta no se sigue.
        assert!(!raiz.join("proyectos").join("fuera").exists());
        let alli = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&raiz, "p2")).unwrap();
        assert_eq!(alli.mensajes.len(), 1);
        // Y una hoja que no existe no adjunta nada.
        assert!(adjuntar(&raiz, "p1", "p2", "no-esta", |n| n.to_string(), &sello).is_err());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn una_pagina_del_mismo_proyecto_apunta_a_su_pdf_y_a_su_dibujo() {
        let raiz = std::env::temp_dir().join(format!("pixpin-paginas-m-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let carpeta = almacen::carpeta(&raiz, "p1");
        std::fs::create_dir_all(&carpeta).unwrap();
        let p = Proyecto {
            id: "p1".into(),
            nombre: "Plano".into(),
            hojas: vec![hoja("h1", Some(4), Some("d9"), None)],
            ..Default::default()
        };
        std::fs::write(
            carpeta.join("proyecto.json"),
            serde_json::to_string(&p).unwrap(),
        )
        .unwrap();
        let sello = cuaderno::Sello {
            cuando: 7,
            numero: 3,
            aparato: "K7Q2".into(),
            proyecto: "p1".into(),
        };
        let m = adjuntar(&raiz, "p1", "p1", "h1", |n| format!("Pagina {n}"), &sello).unwrap();
        assert_eq!(m.clase, Some(cuaderno::Clase::Pagina));
        assert_eq!(m.nombre, "Plano · Pagina 5");
        assert_eq!(m.pagina, Some(4));
        assert_eq!(m.referencia.as_deref(), Some("d9"));
        assert_eq!(m.ruta, None, "no apunta a ningun fichero del movil");
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
