//! **Las notas Markdown** (mensaje `NOTA` y hoja `nota` de un proyecto):
//! guardar el texto donde lo guarda Windows (`notas_md/guardar.rs`) y lo que
//! el editor necesita para ensenar el Markdown formateado mientras se
//! escribe (`pixpin_docs::md_vivo`, el puerto de `EditorVivo.kt` del movil)
//! y las formulas `$…$` (`pixpin_docs::formulas`).
//!
//! **El texto del editor ES el Markdown**, letra a letra: el formato se pone
//! encima (titulos, negrita, listas, casillas) y las marcas se esconden.
//! Guardar es leer el texto y ya. Todas las posiciones van en **UTF-16**,
//! que son las de `NSString`/`NSTextView`: Swift las usa tal cual como
//! `NSRange`.
//!
//! Rendimiento: analizar es una pasada lineal por el texto (una nota de
//! cien mil letras, unos milisegundos); Swift lo pide tras una pausa al
//! escribir, no por cada tecla, y solo restila lo que cambio.
//!
//! El editor que se abre ahora es el de Windows (`web`): su misma pagina en
//! un `WKWebView`. Lo de arriba queda para el editor de antes (AppKit).

mod adjuntos;
mod comentarios;
pub mod vista;
pub mod web;

use pixpin_docs::{formulas, md_vivo};
use pixpin_proyecto::{almacen, cuaderno};

use crate::chats::validar_chat;
use crate::{ErrorNucleo, MensajeChat, Nucleo};

/// **Que nota se edita** (`notas_md::Destino` de Windows): un mensaje
/// `NOTA` de un chat, por su id, o un `.md` suelto, por su ruta.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum DestinoNota {
    Mensaje { chat: String, mensaje: String },
    Fichero { ruta: String },
}

/// Que formato lleva un tramo (`md_vivo::Estilo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum EstiloMd {
    /// Una marca de Markdown (`#`, `**`, `[`, `](url)`): se esconde.
    Marca,
    /// El texto de un titulo, nivel 1 a 6.
    Titulo {
        nivel: u8,
    },
    Negrita,
    Cursiva,
    Tachado,
    Codigo,
    BloqueCodigo,
    Cita,
    Enlace,
    /// La casilla de una tarea (`[ ]` o `[x]`).
    Casilla {
        hecha: bool,
    },
    /// Lo escrito tras una casilla marcada: tachado y apagado.
    Hecha,
    Vineta,
    Numero,
    /// Una formula entre dolares, con sus dolares.
    Formula,
    Regla,
    Imagen,
    Cabecera,
}

impl From<md_vivo::Estilo> for EstiloMd {
    fn from(e: md_vivo::Estilo) -> Self {
        use md_vivo::Estilo as E;
        match e {
            E::Marca => EstiloMd::Marca,
            E::Titulo(n) => EstiloMd::Titulo { nivel: n },
            E::Negrita => EstiloMd::Negrita,
            E::Cursiva => EstiloMd::Cursiva,
            E::Tachado => EstiloMd::Tachado,
            E::Codigo => EstiloMd::Codigo,
            E::BloqueCodigo => EstiloMd::BloqueCodigo,
            E::Cita => EstiloMd::Cita,
            E::Enlace => EstiloMd::Enlace,
            E::Casilla { hecha } => EstiloMd::Casilla { hecha },
            E::Hecha => EstiloMd::Hecha,
            E::Vineta => EstiloMd::Vineta,
            E::Numero => EstiloMd::Numero,
            E::Formula => EstiloMd::Formula,
            E::Regla => EstiloMd::Regla,
            E::Imagen => EstiloMd::Imagen,
            E::Cabecera => EstiloMd::Cabecera,
        }
    }
}

/// Un tramo con formato, en posiciones UTF-16 del texto entero.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TramoMd {
    pub desde: u32,
    pub hasta: u32,
    pub estilo: EstiloMd,
    /// El renglon (desde 0).
    pub linea: u32,
    /// La direccion de un enlace o la ruta de una imagen.
    pub url: Option<String>,
}

/// Que hacer al pulsar Intro en un renglon (`md_vivo::Continuar`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ContinuarMd {
    /// Un salto normal.
    Nada,
    /// Seguir la lista: el renglon nuevo empieza con esto.
    Con { prefijo: String },
    /// El renglon solo tenia la marca: se quita y se sale de la lista.
    Cortar,
}

/// Que se hace con un trozo de una formula (`formulas::Toque`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ToqueFormula {
    /// Pequeno y subido (exponente, numerador).
    Arriba,
    /// Pequeno y bajado (subindice, denominador).
    Abajo,
    /// Una marca que no se ve (`^`, `_`, llaves).
    Oculto,
}

/// Un toque de una formula, en UTF-16 dentro de la formula sin dolares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct TramoFormula {
    pub desde: u32,
    pub hasta: u32,
    pub toque: ToqueFormula,
}

fn u32_de(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// **Todos los tramos con formato** de una nota (`md_vivo::analizar`).
#[uniffi::export]
pub fn md_tramos(texto: String) -> Vec<TramoMd> {
    md_vivo::analizar(&texto)
        .into_iter()
        .map(|t| TramoMd {
            desde: u32_de(t.desde),
            hasta: u32_de(t.hasta),
            estilo: t.estilo.into(),
            linea: u32_de(t.linea),
            url: t.url,
        })
        .collect()
}

/// Con que sigue el renglon de debajo al pulsar Intro en `renglon`.
#[uniffi::export]
pub fn md_continuar(renglon: String) -> ContinuarMd {
    match md_vivo::continuar(&renglon) {
        md_vivo::Continuar::Nada => ContinuarMd::Nada,
        md_vivo::Continuar::Con(prefijo) => ContinuarMd::Con { prefijo },
        md_vivo::Continuar::Cortar => ContinuarMd::Cortar,
    }
}

/// El renglon cuya casilla esta en la posicion `pos` (UTF-16), si hay una.
#[uniffi::export]
pub fn md_casilla_en(texto: String, pos: u32) -> Option<u32> {
    md_vivo::casilla_en(&texto, pos as usize).map(u32_de)
}

/// El texto con la casilla del renglon `linea` marcada o desmarcada.
#[uniffi::export]
pub fn md_alternar_casilla(texto: String, linea: u32) -> Option<String> {
    md_vivo::alternar_casilla(&texto, linea as usize)
}

/// Envuelve lo elegido (`desde..hasta`, UTF-16) con `marca` (`**`, `_`,
/// `~~`, `` ` ``) o se la quita si ya la tenia. Devuelve el texto nuevo y lo
/// elegido despues: `[texto]` y las dos posiciones en `elegido`.
#[uniffi::export]
pub fn md_envolver(texto: String, desde: u32, hasta: u32, marca: String) -> TextoElegido {
    let (t, a, b) = md_vivo::envolver(&texto, desde as usize, hasta as usize, &marca);
    TextoElegido {
        texto: t,
        desde: u32_de(a),
        hasta: u32_de(b),
    }
}

/// Un texto y lo elegido en el.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TextoElegido {
    pub texto: String,
    pub desde: u32,
    pub hasta: u32,
}

/// El titulo de una nota (su primer renglon con algo, sin marcas).
#[uniffi::export]
pub fn md_titulo(texto: String) -> String {
    md_vivo::titulo(&texto)
}

/// **Los toques de una formula** (sin sus dolares): que subir, que bajar y
/// que esconder para que `x^2` se vea x² fuera del renglon del cursor.
#[uniffi::export]
pub fn formula_toques(formula: String) -> Vec<TramoFormula> {
    formulas::toques(&formula)
        .into_iter()
        .map(|(desde, hasta, t)| TramoFormula {
            desde: u32_de(desde),
            hasta: u32_de(hasta),
            toque: match t {
                formulas::Toque::Arriba => ToqueFormula::Arriba,
                formulas::Toque::Abajo => ToqueFormula::Abajo,
                formulas::Toque::Oculto => ToqueFormula::Oculto,
            },
        })
        .collect()
}

/// La formula en texto plano (`(a/b)`, `x^2`, `sqrt(x)`): para copiar,
/// leer en voz alta o la ayuda.
#[uniffi::export]
pub fn formula_plano(formula: String) -> String {
    formulas::plano(&formulas::leer(&formula))
}

fn escribir_atomico(ruta: &std::path::Path, datos: &[u8]) -> std::io::Result<()> {
    pixpin_sincro::disco::escribir_atomico(ruta, datos)
}

#[uniffi::export]
impl Nucleo {
    /// **Guarda el texto de una nota** (`mensaje`: el id de un `NOTA`) donde
    /// la tenga, como `notas_md::guardar` de Windows: en su linea del
    /// cuaderno (lo demas de la linea queda como estaba) y, si el proyecto
    /// tiene una hoja `nota` con el mismo codigo unico (una nota del movil),
    /// tambien en ella y en su copia `notas/<id>.md`. Sube el `tocado` del
    /// chat. Devuelve el mensaje como quedo. Bloqueante.
    pub fn editar_nota(
        &self,
        chat: String,
        mensaje: String,
        texto: String,
    ) -> Result<MensajeChat, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta();
        let carpeta = almacen::carpeta(raiz, &chat);
        let fichero = carpeta.join("guardados.jsonl");
        if !fichero.exists() {
            return Err(ErrorNucleo::NoExiste { que: chat });
        }
        let mut no_es_nota = false;
        let mut cache = self.cache_tomada();
        let hecho = cuaderno::cambiar(&carpeta, &mensaje, |m| {
            if m.clase != Some(cuaderno::Clase::Nota) {
                no_es_nota = true;
                return false;
            }
            if m.texto == texto {
                return false;
            }
            m.texto = texto.clone();
            true
        })?;
        cache.quitar(&fichero);
        drop(cache);
        let m = match hecho {
            None => return Err(ErrorNucleo::NoExiste { que: mensaje }),
            Some(_) if no_es_nota => {
                return Err(ErrorNucleo::Argumento {
                    detalle: format!("no es una nota: {mensaje}"),
                });
            }
            Some(m) => m,
        };
        let ahora = crate::ahora_ms();
        // La hoja `nota` del proyecto con el mismo codigo (las dos copias del
        // movil: la de dentro del proyecto y la de `notas/`).
        let codigo = m.codigo_unico();
        let pj = carpeta.join("proyecto.json");
        if let Ok(t) = std::fs::read_to_string(&pj)
            && let Ok(mut p) = serde_json::from_str::<pixpin_proyecto::Proyecto>(&t)
            && let Some(h) = p
                .hojas
                .iter_mut()
                .find(|h| h.codigo_unico() == codigo && h.nota.is_some())
            && h.nota.as_deref() != Some(texto.as_str())
        {
            h.nota = Some(texto.clone());
            let id = h.id.clone();
            p.tocado = p.tocado.max(ahora);
            escribir_atomico(
                &carpeta.join("notas").join(format!("{id}.md")),
                texto.as_bytes(),
            )?;
            let json = serde_json::to_vec_pretty(&p).map_err(|e| ErrorNucleo::Disco {
                detalle: e.to_string(),
            })?;
            escribir_atomico(&pj, &json)?;
        }
        let mut indice = almacen::Indice::leer(raiz);
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == chat) {
            f.tocado = f.tocado.max(ahora);
            indice.guardar(raiz)?;
        }
        Ok(MensajeChat::from(m))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_tramos_van_en_utf16_como_nsstring() {
        let t = md_tramos("# Hola 😀\n- [x] **hecho**".into());
        assert!(
            t.iter()
                .any(|x| x.estilo == EstiloMd::Titulo { nivel: 1 } && x.linea == 0)
        );
        assert!(
            t.iter()
                .any(|x| x.estilo == EstiloMd::Casilla { hecha: true } && x.linea == 1)
        );
        // El emoji cuenta dos: el renglon 0 mide 9 en UTF-16.
        let titulo = t
            .iter()
            .find(|x| matches!(x.estilo, EstiloMd::Titulo { .. }))
            .unwrap();
        assert_eq!(titulo.hasta, 9);
        assert!(t.iter().any(|x| x.estilo == EstiloMd::Negrita));
    }

    #[test]
    fn la_lista_sigue_y_la_casilla_se_marca() {
        assert_eq!(
            md_continuar("- [x] algo".into()),
            ContinuarMd::Con {
                prefijo: "- [ ] ".into()
            }
        );
        assert_eq!(md_continuar("- ".into()), ContinuarMd::Cortar);
        assert_eq!(md_continuar("texto".into()), ContinuarMd::Nada);
        let t = "uno\n- [ ] dos".to_string();
        let pos = md_casilla_en(t.clone(), 7).expect("la casilla del renglon 1");
        assert_eq!(pos, 1);
        assert_eq!(md_alternar_casilla(t, 1).unwrap(), "uno\n- [x] dos");
        // Caso negativo: un renglon sin casilla no cambia.
        assert_eq!(md_alternar_casilla("uno".into(), 0), None);
    }

    #[test]
    fn las_formulas_suben_y_bajan() {
        let t = formula_toques("x^2".into());
        assert!(t.contains(&TramoFormula {
            desde: 1,
            hasta: 2,
            toque: ToqueFormula::Oculto
        }));
        assert!(t.contains(&TramoFormula {
            desde: 2,
            hasta: 3,
            toque: ToqueFormula::Arriba
        }));
        assert_eq!(formula_plano("\\frac{a}{b}".into()), "(a/b)");
    }

    #[test]
    fn editar_una_nota_la_cambia_en_el_cuaderno_y_conserva_lo_demas() {
        let d = std::env::temp_dir().join(format!("pixpin-nucleo-notas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        let chat = n.crear_chat("Notas".into()).unwrap();
        let m = n
            .escribir_nota(chat.id.clone(), "# Lista\n- [ ] pan".into(), None)
            .unwrap();
        // Una clave que solo entiende el movil, en la linea de la nota.
        let carpeta = almacen::carpeta(&d, &chat.id);
        cuaderno::cambiar(&carpeta, &m.id, |x| {
            x.resto
                .insert("colorDelMovil".into(), serde_json::Value::from(7));
            true
        })
        .unwrap();
        let editada = n
            .editar_nota(chat.id.clone(), m.id.clone(), "# Lista\n- [x] pan".into())
            .unwrap();
        assert_eq!(editada.texto, "# Lista\n- [x] pan");
        let pagina = n.ultimos_mensajes(chat.id.clone(), 10).unwrap();
        assert_eq!(pagina.mensajes.last().unwrap().texto, "# Lista\n- [x] pan");
        let linea = std::fs::read_to_string(carpeta.join("guardados.jsonl")).unwrap();
        assert!(
            linea.contains("colorDelMovil"),
            "lo del movil sigue: {linea}"
        );
        // Casos negativos: un mensaje que no esta, y uno que no es nota.
        assert!(matches!(
            n.editar_nota(chat.id.clone(), "nada".into(), "x".into()),
            Err(ErrorNucleo::NoExiste { .. })
        ));
        let lienzo = n.crear_lienzo(chat.id.clone(), None).unwrap();
        assert!(matches!(
            n.editar_nota(chat.id, lienzo.id, "x".into()),
            Err(ErrorNucleo::Argumento { .. })
        ));
    }
}
