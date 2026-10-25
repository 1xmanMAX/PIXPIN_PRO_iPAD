//! **`hoja_a_lienzo` en el Mac** (`asistente/hoja_a_lienzo.rs` de Windows):
//! una foto o una pagina de PDF al lienzo del chat, zona a zona, con
//! `pixpin_ia::hoja` (el mismo seccionado y el mismo montaje que Windows).
//!
//! Lo que cambia en el Mac:
//! - **Leer la hoja**: la foto la carga `pixpin-codec`; la pagina de un PDF
//!   la pinta PDFKit (`AppIa::pintar_pagina`) y sus palabras salen de su
//!   capa de texto o de Vision (`AppIa::palabras_de_imagen`).
//! - **Una pasada, en orden**, sin el planificador ni la comparacion final
//!   de Windows: cada zona se monta y solo lo que necesita modelo (formulas,
//!   zonas dudosas, graficas, dibujos que quiza son formulas) se pregunta a
//!   la nube con su recorte.
//! - **Un solo paso de deshacer**: todo entra a la vez en el lienzo del chat
//!   (`al_lienzo::Trabajo::DibujarHoja`), con las fotos recortadas.
//!
//! Solo con la nube, como en Windows (`hoja::SOLO_NUBE`).
//!
//! **El supervisor** (`supervisor.rs`, el mismo de Windows del 18-oct): al
//! final se pinta lo hecho (con PDFKit, `AppIa::pintar_pagina`) y las zonas
//! que peor se parecen al papel se le ensenan al modelo lado a lado; se
//! rehacen con lo que dice si la prueba de los pixeles lo deja.

pub(crate) mod supervisor;

use std::path::Path;

use pixpin_ia::hoja::montaje::{self, Entrada, Hecho, Leido, Via};
use pixpin_ia::hoja::{self, Caja, Mapa, Opciones, PalabraHoja, Seccionado, Tipo, comparar, imagen, vision};
use pixpin_ia::motor::Motor;

use super::turno::AppIa;

/// El ancho al que se pinta una pagina de PDF (~200 ppp en A4).
pub const ANCHO_PAGINA: u32 = 1700;
/// Una foto mas grande se reduce antes de leerla.
const LADO_MAXIMO: u32 = 2600;
/// Lo mas grande que se manda un recorte al modelo.
const LADO_AL_MODELO: u32 = 1024;

/// La pagina, lista para seccionar.
pub struct Pagina {
    pub rgba: Vec<u8>,
    pub ancho: u32,
    pub alto: u32,
    pub palabras: Vec<PalabraHoja>,
}

fn es_imagen(ruta: &Path) -> bool {
    super::turno::es_imagen(&ruta.display().to_string())
}

fn es_pdf(ruta: &Path) -> bool {
    ruta.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Las palabras de unos pixeles por Vision.
fn palabras_por_vision(app: &dyn AppIa, img: &pixpin_codec::ImagenRgba) -> Result<Vec<PalabraHoja>, String> {
    let png = pixpin_codec::imagen::codificar_png(img).map_err(|e| format!("{e:?}"))?;
    Ok(app
        .palabras_de_imagen(png)
        .into_iter()
        .map(|p| PalabraHoja::nueva(&p.texto, p.x as f64, p.y as f64, p.ancho as f64, p.alto as f64))
        .collect())
}

/// Los trozos de la capa de texto del PDF (en puntos, `y` la linea base),
/// partidos en palabras y en pixeles (Windows, `palabras_de_trozos`).
pub fn palabras_de_trozos(trozos: &[pixpin_pdf::texto::TrozoDeTexto], esc: f64) -> Vec<PalabraHoja> {
    let mut v = Vec::new();
    for t in trozos {
        let total = t.texto.chars().count().max(1) as f64;
        let paso = (t.fin - t.x).max(0.0) as f64 / total;
        let (y, alto) = ((t.y - t.alto * 0.8) as f64, t.alto as f64);
        let mut i = 0usize;
        for trozo in t.texto.split(' ') {
            let n = trozo.chars().count();
            if !trozo.trim().is_empty() {
                let x = t.x as f64 + i as f64 * paso;
                let mut p = PalabraHoja::nueva(trozo, x * esc, y * esc, n as f64 * paso * esc, alto * esc);
                p.negrita = t.negrita;
                v.push(p);
            }
            i += n + 1;
        }
    }
    v
}

/// **Lee la hoja**: una foto, o la pagina `pagina` (desde 0) de un PDF.
pub fn leer(app: &dyn AppIa, ruta: &Path, pagina: Option<usize>) -> Result<Pagina, String> {
    if es_imagen(ruta) {
        let mut img = pixpin_codec::imagen::cargar(ruta).map_err(|e| format!("no se pudo abrir la imagen: {e}"))?;
        let mayor = img.ancho.max(img.alto);
        if mayor > LADO_MAXIMO {
            let k = LADO_MAXIMO as f64 / mayor as f64;
            let (w, h) = (((img.ancho as f64 * k) as u32).max(1), ((img.alto as f64 * k) as u32).max(1));
            img = pixpin_codec::imagen::redimensionar(img, w, h).map_err(|e| format!("no se pudo reducir la imagen: {e:?}"))?;
        }
        let palabras = palabras_por_vision(app, &img)?;
        return Ok(Pagina { rgba: img.pixeles, ancho: img.ancho, alto: img.alto, palabras });
    }
    if !es_pdf(ruta) {
        return Err("solo se pasa al lienzo una foto (PNG, JPG...) o un PDF".into());
    }
    let medidas = app.paginas_de_pdf(ruta.display().to_string());
    let p = pagina.unwrap_or(0);
    let m = medidas.get(p).ok_or_else(|| format!("el PDF tiene {} paginas", medidas.len()))?;
    let png = app.pintar_pagina(ruta.display().to_string(), p as u32, ANCHO_PAGINA).ok_or("no se pudo pintar la pagina")?;
    let tmp = std::env::temp_dir().join(format!("pixpin-hoja-{}-{}.png", std::process::id(), crate::ahora_ms()));
    std::fs::write(&tmp, &png).map_err(|e| e.to_string())?;
    let img = pixpin_codec::imagen::cargar(&tmp).map_err(|e| format!("no se pudo leer la pagina pintada: {e}"));
    let _ = std::fs::remove_file(&tmp);
    let img = img?;
    let trozos = pixpin_pdf::texto::trozos_de_fichero(ruta).ok().and_then(|t| t.get(p).cloned()).unwrap_or_default();
    let palabras = if trozos.len() >= 4 && m.ancho > 0.0 {
        palabras_de_trozos(&trozos, img.ancho as f64 / m.ancho as f64)
    } else {
        palabras_por_vision(app, &img)?
    };
    Ok(Pagina { rgba: img.pixeles, ancho: img.ancho, alto: img.alto, palabras })
}

/// Lo que se le pregunta al modelo de una zona.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pregunta {
    Latex,
    Tipo,
    Grafica,
}

/// Que pregunta (si alguna) lleva una zona (Windows, sin lo de las laminas).
fn pregunta(z: &hoja::Zona, o: &Opciones, alto_letra: f64) -> Option<Pregunta> {
    match z.tipo {
        Tipo::Dudosa if z.caja.w < alto_letra * 3.0 && z.caja.h < alto_letra * 3.0 => None,
        Tipo::Formula => Some(Pregunta::Latex),
        Tipo::Dudosa => Some(Pregunta::Tipo),
        Tipo::Grafica => Some(Pregunta::Grafica),
        Tipo::Dibujo if z.caja.w >= alto_letra * 1.5 || z.caja.h >= alto_letra * 1.5 => Some(Pregunta::Tipo),
        Tipo::Foto | Tipo::Dibujo if z.confianza < o.duda => Some(Pregunta::Tipo),
        _ => None,
    }
}

fn png_del_recorte(p: &Pagina, c: &Caja) -> Result<Vec<u8>, String> {
    let (rgba, w, h) = imagen::recortar(&p.rgba, p.ancho, p.alto, &c.crecer(6.0));
    let (rgba, w, h) = montaje::reducir(&rgba, w, h, LADO_AL_MODELO);
    pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba { ancho: w, alto: h, pixeles: rgba }).map_err(|e| format!("{e:?}"))
}

fn preguntar(motor: &dyn Motor, png: Vec<u8>, q: Pregunta) -> Result<Option<Leido>, String> {
    match q {
        Pregunta::Latex => vision::pedir_latex(motor, png).map(Some),
        Pregunta::Tipo => vision::pedir_tipo(motor, png).map(Some),
        Pregunta::Grafica => vision::pedir_grafica(motor, png),
    }
}

/// Lo que salio de una hoja.
pub struct Pasada {
    pub seccionado: Seccionado,
    pub hechos: Vec<Hecho>,
    /// Cuantas zonas se preguntaron a la nube.
    pub preguntadas: usize,
    /// Cuantas rehizo el supervisor.
    pub rehechas: usize,
}

/// Lo que pinta lo hecho (`hechos`, en una hoja de `tam`) a `ancho` pixeles.
pub type Pintor<'a> = &'a dyn Fn(&[Hecho], (f32, f32), u32) -> Option<(Vec<u8>, u32, u32)>;

/// **Lo hecho, pintado** como queda en el lienzo: un PDF de la hoja a su
/// medida (`pixpin_pdf`) que pinta PDFKit (Windows lo pinta su lector).
pub fn pintar(app: &dyn AppIa, hechos: &[Hecho], tam: (f32, f32), ancho: u32) -> Option<(Vec<u8>, u32, u32)> {
    use std::collections::HashMap;
    let elementos: Vec<serde_json::Value> = hechos.iter().flat_map(|h| h.elementos.iter().cloned()).collect();
    let imagenes: HashMap<u64, &montaje::ImagenNueva> = hechos.iter().flat_map(|h| h.imagenes.iter()).map(|i| (i.id, i)).collect();
    let alto = ((ancho as f32 * tam.1 / tam.0.max(1.0)) as u32).max(1);
    let mut escena = pixpin_motor2d::Escena::nueva();
    if !elementos.is_empty() {
        let l = pixpin_motor2d::excalidraw::leer(&pixpin_ia::lienzo::documento(elementos)).ok()?;
        for e in l.elementos() {
            escena.anadir(e);
        }
    }
    let mut hojas = pixpin_motor2d::exportar::hojas(&escena, pixpin_motor2d::exportar::Alcance::Todo, &[], None);
    if hojas.is_empty() {
        return Some((vec![255; (ancho * alto * 4) as usize], ancho, alto));
    }
    hojas.truncate(1);
    hojas[0].caja = (0.0, 0.0, tam.0, tam.1);
    let resolver = |id: u64| imagenes.get(&id).map(|i| pixpin_pdf::escribir::Pixeles { ancho: i.ancho, alto: i.alto, rgba: i.rgba.clone() });
    let pdf = pixpin_pdf::escribir::de_hojas_a_medida(&hojas, &[tam], &resolver, None)?;
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let ruta = std::env::temp_dir().join(format!("pixpin-hoja-{}-{n}.pdf", std::process::id()));
    std::fs::write(&ruta, pdf).ok()?;
    let png = app.pintar_pagina(ruta.display().to_string(), 0, ancho);
    let _ = std::fs::remove_file(&ruta);
    let png = png?;
    let tmp = std::env::temp_dir().join(format!("pixpin-hoja-{}-{n}.png", std::process::id()));
    std::fs::write(&tmp, png).ok()?;
    let img = pixpin_codec::imagen::cargar(&tmp);
    let _ = std::fs::remove_file(&tmp);
    let img = img.ok()?;
    Some((img.pixeles, img.ancho, img.alto))
}

/// **Pasa la hoja**: seccionar (por recuadros, sin la marca de agua) y
/// montar cada zona en orden; `vision` contesta lo que necesita modelo (si
/// falla, la zona sale por el camino sin modelo). `parar` corta.
#[allow(clippy::too_many_arguments)] // lo mismo que pide la pasada de Windows
pub fn pasar(p: &Pagina, vision: Option<&dyn Motor>, pintor: Option<Pintor<'_>>, o: &Opciones, fiel: bool, semilla: u64, parar: &dyn Fn() -> bool, progreso: &mut dyn FnMut(usize, usize)) -> Result<Pasada, String> {
    let (s, limpia) = hoja::paneles::seccionar_limpia(&p.rgba, p.ancho, p.alto, p.palabras.clone(), false);
    let rgba = limpia.unwrap_or_else(|| p.rgba.clone());
    let pagina = Pagina { rgba, ancho: p.ancho, alto: p.alto, palabras: Vec::new() };
    if s.zonas.is_empty() {
        return Err("la hoja esta en blanco: no hay nada que pasar al lienzo".into());
    }
    let mapa = Mapa::para(&s, o);
    let ajenas: Vec<Caja> = s.zonas.iter().flat_map(|z| z.renglones.iter().flat_map(|r| r.palabras.iter().map(|p| p.caja))).collect();
    let pistas: Vec<String> = s.zonas.iter().flat_map(|z| z.renglones.iter().map(|r| r.texto.clone())).collect();
    let via = |z: &hoja::Zona| if fiel && matches!(z.tipo, Tipo::Parrafo | Tipo::Encabezado | Tipo::Tabla) { Via::Fiel } else { Via::Editable };
    let mut hechos = Vec::with_capacity(s.zonas.len());
    let mut preguntadas = 0;
    let total = s.zonas.len();
    for (i, z) in s.zonas.iter().enumerate() {
        if parar() {
            return Err("el usuario lo paro".into());
        }
        progreso(i, total);
        let e = Entrada {
            zona: z,
            mapa,
            opciones: o,
            pagina: (&pagina.rgba, pagina.ancho, pagina.alto),
            alto_letra: s.alto_letra,
            semilla,
            lector: None,
            ajenas: &ajenas,
            pistas: &pistas,
        };
        let leido = match (vision, pregunta(z, o, s.alto_letra)) {
            (Some(m), Some(q)) => {
                preguntadas += 1;
                png_del_recorte(&pagina, &z.caja).ok().and_then(|png| preguntar(m, png, q).ok().flatten())
            }
            _ => None,
        };
        hechos.push(montaje::hacer(&e, leido.as_ref(), via(z)));
    }
    let entrada = |z: usize| Entrada {
        zona: &s.zonas[z],
        mapa,
        opciones: o,
        pagina: (&pagina.rgba, pagina.ancho, pagina.alto),
        alto_letra: s.alto_letra,
        semilla,
        lector: None,
        ajenas: &ajenas,
        pistas: &pistas,
    };
    let mut rehechas = 0;
    if let (Some(m), Some(pintor)) = (vision, pintor)
        && o.rico
        && o.comprobar
        && !parar()
        && std::env::var_os("PIXPIN_SIN_SUPERVISOR").is_none()
    {
        rehechas = supervisar(p, &s, m, pintor, mapa, &mut hechos, &entrada);
    }
    progreso(total, total);
    Ok(Pasada { seccionado: s, hechos, preguntadas, rehechas })
}

/// **El supervisor** (paso 7 de la pasada de Windows): las zonas que peor
/// se parecen al papel, lado a lado al modelo; las que dice que estan mal se
/// rehacen con lo que dice, si la prueba de los pixeles lo deja. Devuelve
/// cuantas rehizo.
fn supervisar<'a>(p: &Pagina, s: &Seccionado, m: &dyn Motor, pintor: Pintor<'_>, mapa: Mapa, hechos: &mut [Hecho],
                  entrada: &dyn Fn(usize) -> Entrada<'a>) -> usize {
    let (hw, hh) = mapa.hoja(s);
    let tam = (hw as f32, hh as f32);
    let Some(pintado) = pintor(hechos, tam, p.ancho) else { return 0 };
    let aire = s.alto_letra * 0.6;
    let pares: Vec<(usize, f64)> = hechos
        .iter()
        .filter(|h| !h.elementos.is_empty())
        .map(|h| {
            let c = s.zonas[h.zona].caja;
            let a = supervisor::recorte(&p.rgba, p.ancho, p.alto, &c, aire);
            let b = supervisor::recorte(&pintado.0, pintado.1, pintado.2, &c, aire);
            (h.zona, comparar::parecido((&a.0, a.1, a.2), (&b.0, b.1, b.2)))
        })
        .collect();
    let cand = supervisor::candidatas(&pares, |z| hechos.iter().find(|h| h.zona == z).map(|h| h.tipo));
    let veredictos: Vec<(usize, Option<supervisor::Veredicto>)> = cand
        .iter()
        .map(|&z| {
            let c = s.zonas[z].caja;
            let a = supervisor::recorte(&p.rgba, p.ancho, p.alto, &c, aire);
            let b = supervisor::recorte(&pintado.0, pintado.1, pintado.2, &c, aire);
            let que_es = match s.zonas[z].tipo {
                Tipo::Formula => "una formula",
                Tipo::Tabla => "una tabla",
                Tipo::Dibujo | Tipo::Diagrama => "un dibujo o una figura",
                _ => "texto",
            };
            (z, supervisor::mirar(m, (&a.0, a.1, a.2), (&b.0, b.1, b.2), que_es))
        })
        .collect();
    let mut arregladas = 0;
    for (z, v) in veredictos {
        let zona = &s.zonas[z];
        let dice: String = match &v {
            Some(supervisor::Veredicto::Renglones(r, _)) => r.join(" "),
            _ => String::new(),
        };
        let nuevo = match v {
            Some(supervisor::Veredicto::Renglones(r, _)) if matches!(zona.tipo, Tipo::Parrafo | Tipo::Encabezado) && !zona.renglones.is_empty() => {
                let ocr: Vec<String> = zona.renglones.iter().map(|x| x.texto.clone()).collect();
                let json = serde_json::to_string(&r).unwrap_or_default();
                vision::leer_renglones(&json, &ocr)
                    .or_else(|| vision::renglones_libres(&json, &ocr))
                    .or_else(|| Some(supervisor::limpio(&r)))
                    .map(|bien| montaje::hacer(&entrada(z), Some(&Leido::Renglones(bien)), Via::Fiel))
            }
            Some(supervisor::Veredicto::Renglones(r, _))
                if zona.tipo == Tipo::Formula && r.len() == 1 && r[0].trim().starts_with('$') && r[0].trim().ends_with('$') && r[0].matches('$').count() == 2 =>
            {
                let l = r[0].trim().trim_matches('$').to_string();
                Some(montaje::hacer(&entrada(z), Some(&Leido::Latex(vec![l])), Via::Editable))
            }
            Some(supervisor::Veredicto::Renglones(r, _)) if !zona.renglones.is_empty() && matches!(zona.tipo, Tipo::Dibujo | Tipo::Dudosa | Tipo::Formula) => {
                let mut como_texto = zona.clone();
                como_texto.tipo = Tipo::Parrafo;
                let ocr: Vec<String> = zona.renglones.iter().map(|x| x.texto.clone()).collect();
                let json = serde_json::to_string(&r).unwrap_or_default();
                vision::leer_renglones(&json, &ocr).or_else(|| Some(supervisor::limpio(&r))).map(|bien| {
                    let e = Entrada { zona: &como_texto, ..entrada(z) };
                    let mut h = montaje::hacer(&e, Some(&Leido::Renglones(bien)), Via::Fiel);
                    h.tipo = Tipo::Parrafo;
                    h
                })
            }
            Some(supervisor::Veredicto::Recorte(_)) if !matches!(zona.tipo, Tipo::Parrafo | Tipo::Encabezado) => {
                Some(montaje::hacer(&entrada(z), None, Via::Imagen))
            }
            _ => None,
        };
        let Some(nuevo) = nuevo else { continue };
        if nuevo.elementos.is_empty() {
            continue;
        }
        // La prueba de los pixeles: la zona sola, antes y despues.
        let c = zona.caja;
        let a = supervisor::recorte(&p.rgba, p.ancho, p.alto, &c, aire);
        let cuanto = |h: &Hecho| {
            pintor(std::slice::from_ref(h), tam, p.ancho).map(|q| {
                let b = supervisor::recorte(&q.0, q.1, q.2, &c, aire);
                comparar::parecido((&a.0, a.1, a.2), (&b.0, b.1, b.2))
            })
        };
        let antes = hechos.iter().find(|h| h.zona == z).and_then(cuanto).unwrap_or(0.0);
        let despues = cuanto(&nuevo).unwrap_or(0.0);
        let ocr: String = zona.renglones.iter().map(|x| x.texto.as_str()).collect::<Vec<_>>().join(" ");
        let solo_forma = !dice.is_empty() && supervisor::mismas_letras(&dice, &ocr);
        let vale = if supervisor::repite(&dice, &ocr) {
            false
        } else if solo_forma || dice.is_empty() {
            despues + 0.01 >= antes
        } else {
            despues >= antes * 0.3
        };
        if vale && let Some(h) = hechos.iter_mut().find(|h| h.zona == z) {
            *h = nuevo;
            arregladas += 1;
        }
    }
    arregladas
}

/// El documento de todas las zonas y sus fotos (id, PNG), como van al
/// lienzo.
pub fn documento_de(hechos: &[Hecho]) -> Result<(String, Vec<(u64, Vec<u8>)>), String> {
    let elementos: Vec<serde_json::Value> = hechos.iter().flat_map(|h| h.elementos.iter().cloned()).collect();
    if elementos.is_empty() {
        return Err("no salio nada de la hoja".into());
    }
    let mut fotos = Vec::new();
    for i in hechos.iter().flat_map(|h| h.imagenes.iter()) {
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba { ancho: i.ancho, alto: i.alto, pixeles: i.rgba.clone() })
            .map_err(|e| format!("{e:?}"))?;
        fotos.push((i.id, png));
    }
    Ok((pixpin_ia::lienzo::documento(elementos), fotos))
}

/// Lo que se le cuenta al modelo.
pub fn contar(p: &Pasada) -> String {
    let mut r = hoja::resumen(&p.seccionado, p.hechos.len(), 0, 0);
    if p.preguntadas > 0 {
        r.push_str(&format!("; {} zona(s) leidas con la nube", p.preguntadas));
    }
    if p.rehechas > 0 {
        r.push_str(&format!("; el supervisor rehizo {} zona(s)", p.rehechas));
    }
    r
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una hoja blanca con dos renglones de «texto» (barras negras) y las
    /// palabras donde caen, como las daria Vision.
    fn hoja_de_prueba() -> Pagina {
        let (w, h) = (800u32, 600u32);
        let mut rgba = vec![255u8; (w * h * 4) as usize];
        let mut palabras = Vec::new();
        for (k, y0) in [100u32, 160].into_iter().enumerate() {
            for x0 in [80u32, 260, 440] {
                for y in y0..y0 + 24 {
                    for x in x0..x0 + 150 {
                        let i = ((y * w + x) * 4) as usize;
                        rgba[i..i + 3].copy_from_slice(&[20, 20, 20]);
                    }
                }
                palabras.push(PalabraHoja::nueva(&format!("palabra{k}{x0}"), x0 as f64, y0 as f64, 150.0, 24.0));
            }
        }
        Pagina { rgba, ancho: w, alto: h, palabras }
    }

    #[test]
    fn una_hoja_sale_al_lienzo_con_sus_fotos_sin_preguntar_a_la_nube() {
        let p = hoja_de_prueba();
        let mut vistos = Vec::new();
        let pasada = pasar(&p, None, None, &Opciones::default(), false, 7, &|| false, &mut |i, t| vistos.push((i, t))).unwrap();
        assert!(!pasada.hechos.is_empty());
        assert_eq!(pasada.preguntadas, 0);
        assert_eq!(vistos.last(), Some(&(pasada.hechos.len(), pasada.hechos.len())));
        let (doc, fotos) = documento_de(&pasada.hechos).unwrap();
        let leido = pixpin_motor2d::excalidraw::leer(&doc).unwrap();
        assert!(!leido.elementos().is_empty(), "el lienzo lo entiende");
        // Lo que no es texto sale como su foto recortada, con su PNG.
        let con_foto = pasada.hechos.iter().filter(|h| !h.imagenes.is_empty()).count();
        assert_eq!(fotos.len(), pasada.hechos.iter().map(|h| h.imagenes.len()).sum::<usize>());
        assert!(con_foto == 0 || fotos.iter().all(|(_, png)| png.starts_with(b"\x89PNG")));
        assert!(contar(&pasada).contains("zona"));
        // Caso negativo: parada al empezar.
        assert!(pasar(&p, None, None, &Opciones::default(), false, 7, &|| true, &mut |_, _| {}).is_err());
    }

    #[test]
    fn una_hoja_en_blanco_no_pasa() {
        let p = Pagina { rgba: vec![255u8; 200 * 200 * 4], ancho: 200, alto: 200, palabras: Vec::new() };
        assert!(pasar(&p, None, None, &Opciones::default(), false, 1, &|| false, &mut |_, _| {}).is_err());
    }
}

#[cfg(test)]
mod pruebas_lienzo {
    use super::*;
    use crate::Nucleo;

    #[test]
    fn la_hoja_entra_en_el_lienzo_del_chat_con_sus_fotos_guardadas() {
        let d = std::env::temp_dir().join(format!("pixpin-hoja-lienzo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        let chat = n.crear_chat("Obra".into()).unwrap().id;
        // Una «hoja» con una foto (un recuadro gris lleno: sale recortada).
        let (w, h) = (600u32, 400u32);
        let mut rgba = vec![255u8; (w * h * 4) as usize];
        for y in 80..300 {
            for x in 100..450 {
                let i = ((y * w + x) * 4) as usize;
                rgba[i..i + 3].copy_from_slice(&[(x % 255) as u8, 90, (y % 255) as u8]);
            }
        }
        let p = Pagina { rgba, ancho: w, alto: h, palabras: Vec::new() };
        let pasada = pasar(&p, None, None, &Opciones::default(), false, 11, &|| false, &mut |_, _| {}).unwrap();
        let (documento, imagenes) = documento_de(&pasada.hechos).unwrap();
        assert!(!imagenes.is_empty(), "la foto sale recortada");
        let trabajo = super::super::al_lienzo::Trabajo::DibujarHoja { documento, imagenes: imagenes.clone() };
        let (r, referencia) = super::super::al_lienzo::en_el_chat(&n, &chat, None, &trabajo, None).unwrap();
        assert!(!r.ids.is_empty(), "{}", r.texto);
        let referencia = referencia.expect("el lienzo del chat");
        let ruta = pixpin_proyecto::almacen::lienzo(n.ruta(), &chat, &referencia);
        let texto = std::fs::read_to_string(&ruta).unwrap();
        let clave = pixpin_motor2d::enlace::id_de_texto(imagenes[0].0);
        assert!(texto.contains(&format!("imagenes/{clave}")), "la foto esta en `files`");
        let foto = pixpin_proyecto::almacen::carpeta(n.ruta(), &chat).join("imagenes").join(&clave);
        assert!(foto.is_file(), "y su PNG en imagenes/");
        let _ = std::fs::remove_dir_all(&d);
    }
}
