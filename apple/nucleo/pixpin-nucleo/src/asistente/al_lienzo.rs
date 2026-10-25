//! **Lo que dibuja o edita el asistente, al lienzo** (E1: el
//! `asistente/al_lienzo.rs` de Windows y su `trabajar_en_el_lienzo_del_chat`).
//!
//! En Windows el lienzo es de su ventana y el turno se lo pide por una cola.
//! Aqui la escena de un lienzo abierto es de Rust ([`EditorLienzo`]) y el
//! turno la toca directamente con su cerrojo; la ventana se entera por su
//! `version` y repinta. Adonde va:
//!
//! - **Hay un lienzo abierto** (el que estuvo al frente el ultimo, o el
//!   nombrado con `@lienzo:` si es ese): lo dibujado cae **centrado en lo que
//!   se ve** ([`meter_ids`]) en un solo paso de deshacer, y se guarda como lo
//!   que hace el raton.
//! - **No hay**: el lienzo nombrado en la pregunta (`@lienzo:<nombre>`) o, si
//!   no, el **lienzo propio del asistente** en esa conversacion (uno suyo,
//!   `aparato = "IA"`, creado la primera vez); se abre sin ventana, lo nuevo
//!   va **a la derecha de lo que ya habia** ([`meter_al_lado_ids`]) y se
//!   guarda por el camino de siempre ([`EditorLienzo::guardar`]: lapidas,
//!   fotos, lo ajeno intacto), que es lo que mira la sincronizacion.

use std::path::PathBuf;
use std::sync::Arc;

use pixpin_ia::edicion::{Anfitrion, Contexto, Edicion};
use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::Elemento;
use pixpin_proyecto::cuaderno::{Cuaderno, Mensaje as Nota};

use super::chat as ia_chat;
use crate::Nucleo;
use crate::editor::{self, EditorLienzo};
use crate::lienzo::ImagenDelLienzo;

/// El nombre del lienzo propio del asistente, al crearlo.
pub const NOMBRE_LIENZO_PROPIO: &str = "Lienzo del asistente";

/// Hueco entre lo que habia y lo nuevo.
const HUECO: f32 = 80.0;

/// Que se pide al lienzo (ademas de dibujar, editar lo que hay y resumirlo
/// para el modelo; `pixpin_ia::edicion`).
#[derive(Debug, Clone)]
pub enum Trabajo {
    /// Meter un `.excalidraw` hecho por `pixpin_ia::lienzo::documento`.
    Dibujar(String),
    /// Una hoja pasada al lienzo (`hoja_a_lienzo`): su documento y las
    /// fotos recortadas que nombra (id, PNG), todo en un paso.
    DibujarHoja { documento: String, imagenes: Vec<(u64, Vec<u8>)> },
    /// Editar lo que ya hay, con lo que el turno sabe del lienzo.
    Editar { edicion: Box<Edicion>, ctx: Contexto },
    /// El resumen del lienzo destino, al empezar el turno.
    Resumir,
    /// Copiar lo que hay (para «guarda esto como figura» sin nada elegido).
    Elegido,
    /// **Mirarlo** (`ojos`): se copia la zona con sus fotos.
    Ver { mirada: pixpin_ia::ojos::Mirada, ctx: Contexto },
}

/// Lo que contesta el lienzo: el texto para el modelo, los ids que toco
/// (pasan a ser «lo ultimo») y, al resumir, el contexto del turno.
#[derive(Debug, Clone, Default)]
pub struct Respuesta {
    pub texto: String,
    pub ids: Vec<u64>,
    pub ctx: Option<Contexto>,
    /// Con [`Trabajo::Elegido`]: lo elegido en el lienzo abierto.
    pub elegido: Vec<Elemento>,
    /// Con [`Trabajo::Ver`]: lo copiado para pintarlo.
    pub vista: Option<Box<super::ojos::Encargo>>,
}

/// Los elementos de un documento del asistente.
pub fn elementos_de(documento: &str) -> Result<Vec<Elemento>, String> {
    let lienzo = pixpin_motor2d::excalidraw::leer(documento).map_err(|e| format!("el lienzo no lo entiende: {e}"))?;
    let els = lienzo.elementos();
    if els.is_empty() {
        return Err("no hay nada que dibujar".into());
    }
    Ok(els)
}

/// La caja que ocupan unos elementos: (x0, y0, x1, y1).
pub fn caja(els: &[Elemento]) -> Option<(f32, f32, f32, f32)> {
    els.iter()
        .map(Elemento::caja)
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
        .filter(|c| [c.0, c.1, c.2, c.3].iter().all(|v| v.is_finite()))
}

/// **Mete lo dibujado con su centro en `centro`** (el de la vista), en un
/// solo paso de deshacer. Devuelve los ids de lo que entro.
pub fn meter_ids(escena: &mut Escena, documento: &str, centro: (f32, f32)) -> Result<Vec<u64>, String> {
    let els = elementos_de(documento)?;
    let c = caja(&els).ok_or("las figuras no tienen medida")?;
    let dx = centro.0 - (c.0 + c.2) / 2.0;
    let dy = centro.1 - (c.1 + c.3) / 2.0;
    Ok(pixpin_motor2d::portapapeles::pegar(escena, &els, dx, dy))
}

/// **Mete lo dibujado a la derecha de lo que ya hay** (con un lienzo
/// cerrado), alineado arriba; en un lienzo vacio, donde lo puso el modelo.
pub fn meter_al_lado(escena: &mut Escena, documento: &str) -> Result<usize, String> {
    meter_al_lado_ids(escena, documento).map(|v| v.len())
}

/// Lo mismo, devolviendo los ids de lo que entro.
pub fn meter_al_lado_ids(escena: &mut Escena, documento: &str) -> Result<Vec<u64>, String> {
    let els = elementos_de(documento)?;
    let nueva = caja(&els).ok_or("las figuras no tienen medida")?;
    let habia = escena
        .visibles()
        .map(Elemento::caja)
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
        .filter(|c| [c.0, c.1, c.2, c.3].iter().all(|v| v.is_finite()));
    let (dx, dy) = match habia {
        Some(h) => (h.2 + HUECO - nueva.0, h.1 - nueva.1),
        None => (0.0, 0.0),
    };
    Ok(pixpin_motor2d::portapapeles::pegar(escena, &els, dx, dy))
}

/// El resumen que se le devuelve al modelo.
pub fn resumen(n: usize) -> String {
    format!("hecho: {n} elementos anadidos al lienzo")
}

/// El «hecho» de lo dibujado con sus `#id` (en el marco del modelo, si el
/// turno tiene uno), para poder editarlo en la llamada siguiente.
pub fn resumen_con_ids(escena: &Escena, ids: Vec<u64>, marco: Option<(f32, f32)>) -> Respuesta {
    let ctx = Contexto {
        marco: marco.unwrap_or_else(|| pixpin_ia::edicion::marco_de(escena)),
        ..Default::default()
    };
    Respuesta {
        texto: pixpin_ia::edicion::resumen_de_lo_anadido(escena, &ids, &ctx),
        ids,
        ..Default::default()
    }
}

/// Las imagenes que el asistente pega: aun no en el Mac (lo dice al
/// modelo, que lo cuenta).
struct SinImagenes;

impl Anfitrion for SinImagenes {
    fn imagen_nueva(&mut self, _fuente: &str) -> Result<Elemento, String> {
        Err("pegar imagenes en el lienzo desde el asistente aun no se puede en PixPin para Mac".into())
    }
}

/// **Hace un trabajo de editar o resumir sobre una escena** (la de un
/// lienzo abierto o la de uno cerrado). `elegidos`: lo elegido ahora;
/// `marco`: desde donde mide el modelo si es un resumen nuevo.
pub fn trabajar(escena: &mut Escena, trabajo: &Trabajo, elegidos: &[u64], marco: (f32, f32), fotos: &[ImagenDelLienzo]) -> Result<Respuesta, String> {
    match trabajo {
        Trabajo::Ver { mirada, ctx } => {
            let mut ctx = ctx.clone();
            ctx.elegidos = elegidos.to_vec();
            let ruta = |id: u64| fotos.iter().find(|f| f.id_objeto == id).map(|f| f.ruta.clone());
            let enc = super::ojos::encargo_de(escena, mirada, &ctx, ruta)?;
            Ok(Respuesta {
                vista: Some(Box::new(enc)),
                ..Default::default()
            })
        }
        Trabajo::Resumir => {
            let ctx = Contexto {
                marco,
                elegidos: elegidos.to_vec(),
                ultimos: Vec::new(),
            };
            Ok(Respuesta {
                texto: pixpin_ia::edicion::resumen(escena, &ctx),
                ctx: Some(ctx),
                ..Default::default()
            })
        }
        Trabajo::Editar { edicion, ctx } => {
            let mut ctx = ctx.clone();
            // Lo elegido es lo de ahora, no lo de cuando empezo el turno.
            ctx.elegidos = elegidos.to_vec();
            let h = pixpin_ia::edicion::aplicar(escena, edicion, &ctx, &mut SinImagenes)?;
            Ok(Respuesta {
                texto: h.texto,
                ids: h.ids,
                ..Default::default()
            })
        }
        Trabajo::Elegido => Ok(Respuesta {
            elegido: escena.visibles().filter(|e| elegidos.contains(&e.id)).cloned().collect(),
            ..Default::default()
        }),
        Trabajo::Dibujar(_) | Trabajo::DibujarHoja { .. } => Err("eso no es un trabajo de editar".into()),
    }
}

/// Lo que se dice cuando aun no hay lienzo y solo se queria mirarlo.
pub const DESTINO_VACIO: &str = "Lienzo destino: vacio (aun no hay lienzo del asistente en este chat).";

/// **El trabajo en un lienzo abierto**: centrado en lo que se ve.
fn en_el_abierto(editor: &EditorLienzo, trabajo: &Trabajo, marco: Option<(f32, f32)>) -> Result<Respuesta, String> {
    registrar_fotos(editor, trabajo)?;
    let elegidos = editor.elegidos();
    let fotos = editor.imagenes();
    editor.con_la_escena(|escena, vista| {
        let vista = vista.unwrap_or_else(|| {
            let c = escena.caja().unwrap_or((0.0, 0.0, 800.0, 600.0));
            (c.0, c.1, c.0 + 800.0, c.1 + 600.0)
        });
        match trabajo {
            Trabajo::Dibujar(doc) | Trabajo::DibujarHoja { documento: doc, .. } => {
                let centro = ((vista.0 + vista.2) / 2.0, (vista.1 + vista.3) / 2.0);
                let ids = meter_ids(escena, doc, centro)?;
                Ok(resumen_con_ids(escena, ids, marco))
            }
            otro => trabajar(escena, otro, &elegidos, (vista.0.floor(), vista.1.floor()), &fotos),
        }
    })
}

/// Las fotos de una hoja, al editor antes de meter su documento.
fn registrar_fotos(editor: &EditorLienzo, trabajo: &Trabajo) -> Result<(), String> {
    if let Trabajo::DibujarHoja { imagenes, .. } = trabajo {
        for (id, png) in imagenes {
            editor.registrar_png(*id, png).map_err(|e| format!("no se pudo guardar una foto de la hoja: {e}"))?;
        }
    }
    Ok(())
}

/// El lienzo de un mensaje del chat, su fichero y sus fotos.
fn fichero_del_lienzo(nucleo: &Nucleo, chat: &str, m: &Nota) -> Option<(PathBuf, Vec<ImagenDelLienzo>)> {
    let referencia = m.referencia.as_deref()?;
    let ruta = pixpin_proyecto::almacen::lienzo(nucleo.ruta(), chat, referencia);
    let texto = std::fs::read_to_string(&ruta).ok()?;
    let leido = pixpin_motor2d::excalidraw::leer(&texto).ok()?;
    let imagenes = pixpin_motor2d::excalidraw::ficheros(&leido)
        .into_iter()
        .filter_map(|(id_objeto, rel)| {
            let r = nucleo.resolver(chat, &rel)?;
            Some(ImagenDelLienzo {
                id_objeto,
                ruta: r.display().to_string(),
            })
        })
        .collect();
    Some((ruta, imagenes))
}

/// Si el lienzo `ruta` esta abierto, su editor.
fn abierto(ruta: &std::path::Path) -> Option<Arc<EditorLienzo>> {
    editor::abierto_de(ruta)
}

/// **Lo que pide el asistente a un lienzo**, desde la conversacion `chat`:
/// dibujar, editar lo que hay o resumirlo. Va al abierto que estuvo al
/// frente si no se nombro otro; si no, al lienzo nombrado (`destino`, su
/// referencia) o al propio del asistente (creandolo al dibujar). Devuelve la
/// respuesta y la referencia del lienzo del chat (`None` si fue a uno
/// abierto que no es de este chat, o si aun no hay).
pub fn en_el_chat(
    nucleo: &Nucleo,
    chat: &str,
    destino: Option<&str>,
    trabajo: &Trabajo,
    marco: Option<(f32, f32)>,
) -> Result<(Respuesta, Option<String>), String> {
    let carpeta = pixpin_proyecto::almacen::carpeta(nucleo.ruta(), chat);
    let mensajes = Cuaderno::leer_de(&carpeta).map(|c| c.mensajes).unwrap_or_default();
    let nombrado = destino.and_then(|r| {
        mensajes
            .iter()
            .rposition(|m| ia_chat::es_lienzo(m) && m.referencia.as_deref() == Some(r))
    });
    // Sin nombrar ninguno, el abierto que estuvo al frente el ultimo (como
    // en Windows: «con un lienzo abierto va al abierto y no al chat»).
    if nombrado.is_none()
        && let Some(ed) = editor::abierto_al_frente(nucleo.ruta())
    {
        let r = en_el_abierto(&ed, trabajo, marco)?;
        // Si es de este chat, la respuesta lo apunta para su «Abrir lienzo».
        let referencia = (ed.carpeta() == carpeta.as_path())
            .then(|| ed.fichero().file_stem().map(|s| s.to_string_lossy().into_owned()))
            .flatten();
        return Ok((con_nombre(r, None, false), referencia));
    }
    let elegido = nombrado.or_else(|| ia_chat::lienzo_propio(&mensajes));
    let (mensaje, nuevo) = match elegido {
        Some(i) => (mensajes[i].clone(), false),
        // Resumir o editar no estrenan un lienzo: si aun no hay, esta vacio.
        None => match trabajo {
            Trabajo::Resumir => {
                return Ok((
                    Respuesta {
                        texto: DESTINO_VACIO.into(),
                        ctx: Some(Default::default()),
                        ..Default::default()
                    },
                    None,
                ));
            }
            Trabajo::Elegido => return Ok((Respuesta::default(), None)),
            Trabajo::Ver { .. } => return Err("todavia no hay ningun lienzo en este chat: no hay nada que ver".into()),
            Trabajo::Editar { .. } => return Err("todavia no hay ningun lienzo en este chat: dibuja algo primero".into()),
            Trabajo::Dibujar(_) | Trabajo::DibujarHoja { .. } => {
                let m = nucleo
                    .crear_lienzo_con(chat.to_string(), Some(NOMBRE_LIENZO_PROPIO.into()), |m| {
                        m.aparato = Some(ia_chat::APARATO.into());
                    })
                    .map_err(|e| format!("no se pudo crear un lienzo: {e}"))?;
                (m, true)
            }
        },
    };
    let referencia = mensaje.referencia.clone().unwrap_or_default();
    let (ruta, imagenes) = fichero_del_lienzo(nucleo, chat, &mensaje).ok_or("no se pudo abrir el lienzo")?;
    // El nombrado, si esta abierto en una ventana: alli, a la vista.
    if let Some(ed) = abierto(&ruta) {
        let r = en_el_abierto(&ed, trabajo, marco)?;
        return Ok((con_nombre(r, Some(&mensaje), nuevo), Some(referencia)));
    }
    let ed = EditorLienzo::sin_ventana(ruta, carpeta, imagenes.clone()).map_err(|e| format!("no se pudo abrir el lienzo: {e}"))?;
    registrar_fotos(&ed, trabajo)?;
    let respuesta = ed.con_la_escena(|escena, _| match trabajo {
        Trabajo::Dibujar(doc) | Trabajo::DibujarHoja { documento: doc, .. } => {
            let ids = meter_al_lado_ids(escena, doc)?;
            Ok(resumen_con_ids(escena, ids, marco))
        }
        otro => {
            let marco = pixpin_ia::edicion::marco_de(escena);
            trabajar(escena, otro, &[], marco, &imagenes)
        }
    })?;
    match ed.guardar() {
        Ok(_) => {}
        Err(e) => return Err(format!("no se pudo guardar el lienzo: {e}")),
    }
    if matches!(trabajo, Trabajo::Resumir | Trabajo::Elegido | Trabajo::Ver { .. }) {
        return Ok((respuesta, Some(referencia)));
    }
    Ok((con_nombre(respuesta, Some(&mensaje), nuevo), Some(referencia)))
}

/// «hecho: ...» -> «hecho: ... «Lienzo del asistente»», con la lista de ids
/// detras.
fn con_nombre(mut r: Respuesta, mensaje: Option<&Nota>, nuevo: bool) -> Respuesta {
    let Some(m) = mensaje else { return r };
    if r.texto.is_empty() || r.ctx.is_some() {
        return r;
    }
    let nombre = ia_chat::nombre_de_lienzo(m);
    let (primera, resto) = r.texto.split_once('\n').unwrap_or((r.texto.as_str(), ""));
    r.texto = format!(
        "{primera} «{nombre}»{}{}{resto}",
        if nuevo { " (lienzo nuevo en este chat)" } else { "" },
        if resto.is_empty() { "" } else { "\n" }
    );
    r
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn con_el_lienzo_cerrado_lo_nuevo_va_a_la_derecha_de_lo_que_habia() {
        let doc = |x: f64| {
            pixpin_ia::lienzo::documento(vec![serde_json::json!({
                "id": format!("a{x}"), "type": "rectangle", "x": x, "y": 50.0, "width": 100.0, "height": 40.0,
                "strokeColor": "#e03131", "backgroundColor": "transparent", "version": 1, "isDeleted": false
            })])
        };
        let mut escena = pixpin_motor2d::Escena::nueva();
        // Vacio: donde lo puso el modelo.
        assert_eq!(meter_al_lado(&mut escena, &doc(10.0)).unwrap(), 1);
        let c1 = caja(&escena.visibles().cloned().collect::<Vec<_>>()).unwrap();
        assert!((c1.0 - 10.0).abs() < 3.0, "{c1:?}");
        // Con algo: a la derecha, sin taparlo, alineado arriba.
        assert_eq!(meter_al_lado(&mut escena, &doc(10.0)).unwrap(), 1);
        let els: Vec<_> = escena.visibles().cloned().collect();
        let segundo = els[1].caja();
        assert!(segundo.0 > c1.2 + 40.0, "{segundo:?} {c1:?}");
        assert!((segundo.1 - c1.1).abs() < 1.0);
        // Un documento que no es del lienzo no entra.
        assert!(meter_al_lado(&mut escena, "no es json").is_err());
        assert_eq!(escena.cuantos_visibles(), 2);
    }

    #[test]
    fn un_dibujo_llega_al_lienzo_abierto_centrado_y_en_un_paso() {
        let doc = pixpin_ia::lienzo::documento(vec![
            serde_json::json!({"id":"a","type":"rectangle","x":0.0,"y":0.0,"width":100.0,"height":100.0,"strokeColor":"#1e1e1e","version":1,"isDeleted":false}),
            serde_json::json!({"id":"b","type":"ellipse","x":200.0,"y":0.0,"width":100.0,"height":100.0,"strokeColor":"#1e1e1e","version":1,"isDeleted":false}),
        ]);
        let mut escena = pixpin_motor2d::Escena::nueva();
        let antes = escena.pasos_cerrados();
        let ids = meter_ids(&mut escena, &doc, (1000.0, 1000.0)).unwrap();
        assert_eq!(ids.len(), 2);
        let c = caja(&escena.visibles().cloned().collect::<Vec<_>>()).unwrap();
        assert!(((c.0 + c.2) / 2.0 - 1000.0).abs() < 2.0, "{c:?}");
        assert!(((c.1 + c.3) / 2.0 - 1000.0).abs() < 2.0, "{c:?}");
        // Un solo paso de deshacer.
        assert_eq!(escena.pasos_cerrados(), antes + 1);
        assert!(escena.deshacer());
        assert_eq!(escena.cuantos_visibles(), 0);
    }
}
