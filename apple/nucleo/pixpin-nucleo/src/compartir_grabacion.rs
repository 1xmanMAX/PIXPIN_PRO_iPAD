//! **Compartir una grabacion** (Windows 2921a1d, `compartir/clase.rs`; el
//! usuario: «en las grabaciones, que el menu de compartir sea el de toda la
//! app, con los mismos formatos; sobre todo el HTML, que se vea claro e
//! intuitivo»).
//!
//! Una nota de voz sola (una clase, una reunion o una nota cualquiera), o
//! la carpeta de una grabacion, entra en la hoja de compartir de siempre
//! (`PixPinKit/Compartir/`) con lo de siempre —sus paginas para el PDF y las
//! imagenes, los originales y el `.pixpin`— y con lo suyo:
//!
//! - **HTML** ([`grabacion_web`]): la pagina de
//!   [`pixpin_proyecto::clase_exportar::html`], con el reproductor, la
//!   transcripcion que salta y se resalta, las capturas en su minuto y los
//!   marcadores. El audio va dentro hasta [`AUDIO_DENTRO`]; mas grande, al
//!   lado, como el video (siempre al lado: pesa demasiado para base64).
//! - **PDF** (y PNG, JPG, SVG de una pagina): las hojas A4 de
//!   [`grabacion_hojas`] (el titulo, los marcadores y la transcripcion con
//!   las capturas en su minuto), las mismas ordenes que Windows, que Swift
//!   pinta con el pintor del lienzo (`PintorDeHojas`) como un dibujo.
//! - **Word** ([`grabacion_word`]), **Markdown** ([`grabacion_markdown`],
//!   con la carpeta de las capturas, en un ZIP) y **Texto**
//!   ([`grabacion_texto`], la transcripcion con sus minutos).
//! - **Anki** ([`grabacion_anki`]): el fichero para «Archivo → Importar»,
//!   una tarjeta por captura, como el «Enviar a Anki» del menu.
//! - **Originales**: el audio, el video, las capturas y lo adjunto
//!   ([`GrabacionParaCompartir::originales`]).
//!
//! Lo que se lee de la grabacion es [`GrabacionParaCompartir`] (la
//! `ClaseParaExportar` del nucleo comun con sus rutas en texto), que Swift
//! guarda y devuelve a cada formato. Los rotulos son los de
//! `Rotulos::default()`, los mismos que `grabacion-*` en castellano.

use std::path::{Path, PathBuf};

use pixpin_motor2d::ColorRgba;
use pixpin_motor2d::exportar::{self as ex, Hoja};
use pixpin_motor2d::exportar_svg::{self, Incrustada, OpcionesSvg};
use pixpin_motor2d::pintado::Orden;
use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::carpeta_de_paginas::Carpeta;
use pixpin_proyecto::clase::Marcador;
use pixpin_proyecto::clase_exportar::{self as ce, ClaseParaExportar, Fila, Medio, Rotulos};
use pixpin_proyecto::cuaderno::{self, Mensaje};

use crate::chats::validar_chat;
use crate::compartir::HojaDeDibujo;
use crate::{ErrorNucleo, Nucleo};

/// El audio mas grande que va dentro de la pagina (en base64 pesa un
/// tercio mas, y un navegador abre bien hasta unas decenas de megas).
pub const AUDIO_DENTRO: u64 = 30 * 1024 * 1024;
/// Una captura que pesa mas que esto se vuelve a guardar en JPEG para la
/// pagina: cuarenta capturas de pantalla en PNG son cien megas.
const CAPTURA_LIGERA: u64 = 400 * 1024;
/// El lado mas largo de una captura dentro de la pagina.
const LADO_CAPTURA: u32 = 2000;

/// Una captura de la grabacion: su minuto y su fichero.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CapturaDeGrabacion {
    pub ms: i64,
    pub ruta: String,
}

/// Un trozo de la transcripcion: su minuto (si lo lleva) y lo que dice.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LineaDeGrabacion {
    pub ms: Option<i64>,
    pub texto: String,
}

/// Un marcador o una banderita (sin nota) de la grabacion.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MarcadorDeGrabacion {
    pub ms: i64,
    pub texto: String,
}

/// **Una grabacion lista para compartir** (`Grabacion` de Windows): lo que
/// hace falta de ella con sus ficheros por ruta absoluta.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct GrabacionParaCompartir {
    pub titulo: String,
    /// La fecha ya escrita («25/10/2026 18:05»). La pone Swift con la hora
    /// de este Mac a partir de [`Self::cuando`].
    pub fecha: String,
    /// Cuando se grabo, en milisegundos UTC (0 si no se sabe).
    pub cuando: i64,
    pub duracion_ms: i64,
    pub audio: Option<String>,
    pub video: Option<String>,
    pub capturas: Vec<CapturaDeGrabacion>,
    pub lineas: Vec<LineaDeGrabacion>,
    pub marcadores: Vec<MarcadorDeGrabacion>,
    pub adjuntos: Vec<String>,
    /// Todos sus ficheros que hay en este Mac, sin repetir: «Original» u
    /// «Originales».
    pub originales: Vec<String>,
    /// El mazo de Anki (`PixPin::<proyecto>`).
    pub mazo: String,
}

/// Las fotos de las hojas: el `id_objeto` de cada imagen y su fichero.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FotoDeGrabacion {
    pub id_objeto: u64,
    pub ruta: String,
}

/// **Las hojas A4 de una grabacion** para el PDF, el PNG, el JPG y el SVG,
/// con las fotos que pintan. El SVG de cada una no va aqui (lleva las
/// capturas dentro y pesaria demasiado): se pide al elegirlo con
/// [`grabacion_svg_de_hoja`].
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct HojasDeGrabacion {
    pub hojas: Vec<HojaDeDibujo>,
    pub fotos: Vec<FotoDeGrabacion>,
}

fn texto_de(r: &Path) -> String {
    r.display().to_string()
}

impl GrabacionParaCompartir {
    /// De lo del nucleo comun.
    pub fn de(c: &ClaseParaExportar, cuando: i64, mazo: String) -> Self {
        GrabacionParaCompartir {
            titulo: c.titulo.clone(),
            fecha: c.fecha.clone(),
            cuando,
            duracion_ms: c.duracion_ms,
            audio: c.audio.as_deref().map(texto_de),
            video: c.video.as_deref().map(texto_de),
            capturas: c
                .capturas
                .iter()
                .map(|(ms, r)| CapturaDeGrabacion {
                    ms: *ms,
                    ruta: texto_de(r),
                })
                .collect(),
            lineas: c
                .lineas
                .iter()
                .map(|(ms, t)| LineaDeGrabacion {
                    ms: *ms,
                    texto: t.clone(),
                })
                .collect(),
            marcadores: c
                .marcadores
                .iter()
                .map(|m| MarcadorDeGrabacion {
                    ms: m.ms,
                    texto: m.texto.clone(),
                })
                .collect(),
            adjuntos: c.adjuntos.iter().map(|r| texto_de(r)).collect(),
            originales: c
                .ficheros()
                .into_iter()
                .filter(|r| r.is_file())
                .map(|r| texto_de(&r))
                .collect(),
            mazo,
        }
    }

    /// Lo del nucleo comun, para sacar cada formato.
    pub fn datos(&self) -> ClaseParaExportar {
        ClaseParaExportar {
            titulo: self.titulo.clone(),
            fecha: self.fecha.clone(),
            duracion_ms: self.duracion_ms,
            audio: self.audio.as_ref().map(PathBuf::from),
            video: self.video.as_ref().map(PathBuf::from),
            capturas: self
                .capturas
                .iter()
                .map(|c| (c.ms, PathBuf::from(&c.ruta)))
                .collect(),
            lineas: self
                .lineas
                .iter()
                .map(|l| (l.ms, l.texto.clone()))
                .collect(),
            marcadores: self
                .marcadores
                .iter()
                .map(|m| Marcador {
                    ms: m.ms,
                    texto: m.texto.clone(),
                })
                .collect(),
            adjuntos: self.adjuntos.iter().map(PathBuf::from).collect(),
        }
    }
}

/// Los trozos de la transcripcion, como los lee la pantalla de la letra.
fn lineas_de(voz: &Mensaje) -> Vec<(Option<i64>, String)> {
    voz.transcripcion
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(crate::anki::trozos)
        .unwrap_or_default()
        .into_iter()
        .map(|(ms, t)| (ms, crate::anki::sin_marcas(&t)))
        .filter(|(_, t)| !t.is_empty())
        .collect()
}

/// **La nota de voz que se comparte**: la misma si `m` es una, o la de su
/// grabacion si `m` es la carpeta de una grabacion. `None` si no hay.
pub fn voz_de<'a>(m: &'a Mensaje, mensajes: &'a [Mensaje]) -> Option<&'a Mensaje> {
    if ce::es_grabacion(m) {
        return Some(m);
    }
    let c = Carpeta::de_mensaje(m).filter(Carpeta::es_grabacion)?;
    mensajes
        .iter()
        .find(|v| !c.voz.is_empty() && v.id == c.voz)
        .filter(|v| ce::es_grabacion(v))
}

// ---------------------------------------------------------------------------
// Las hojas (PDF, PNG, JPG, SVG), las de Windows (`hojas_de`)

/// La hoja A4 a 96 ppp, como las de texto de Windows.
const ANCHO_A4: f32 = 794.0;
const ALTO_A4: f32 = 1123.0;
const MARGEN_A4: f32 = 64.0;
const BLANCO: ColorRgba = ColorRgba::opaco(1.0, 1.0, 1.0);
const TINTA_TEXTO: ColorRgba = ColorRgba::opaco(0.106, 0.106, 0.122);
const TINTA_SUAVE: ColorRgba = ColorRgba::opaco(0.42, 0.43, 0.46);
const ACENTO: ColorRgba = ColorRgba::opaco(0.18, 0.44, 0.87);
const MARCA: ColorRgba = ColorRgba::opaco(0.72, 0.43, 0.0);
const LETRA: f32 = 12.0;
const LETRA_TITULO: f32 = 22.0;
const LETRA_SECCION: f32 = 13.0;
/// Lo que se aparta el texto de su minuto.
const SANGRIA: f32 = 58.0;
/// Lo mas alta que sale una captura en el A4.
const ALTO_CAPTURA: f32 = 430.0;

fn texto(texto: String, x: f32, y: f32, tam: f32, color: ColorRgba, negrita: bool) -> Orden {
    Orden::Texto {
        texto,
        x,
        y,
        tam,
        familia: "Segoe UI".into(),
        color,
        ancho_max: (ANCHO_A4 - x - MARGEN_A4) * 1.5,
        negrita,
        cursiva: false,
    }
}

fn hoja_en_blanco(ordenes: Vec<Orden>) -> Hoja {
    Hoja {
        nombre: String::new(),
        caja: (0.0, 0.0, ANCHO_A4, ALTO_A4),
        ordenes,
        marcos: Vec::new(),
        granos: Vec::new(),
        grafitos: Vec::new(),
    }
}

/// Las hojas A4 que se van llenando de arriba abajo.
struct Paginas {
    hojas: Vec<Hoja>,
    ordenes: Vec<Orden>,
    y: f32,
}

impl Paginas {
    fn cabe(&mut self, alto: f32) {
        if self.y + alto > ALTO_A4 - MARGEN_A4 && !self.ordenes.is_empty() {
            self.hojas
                .push(hoja_en_blanco(std::mem::take(&mut self.ordenes)));
            self.y = MARGEN_A4;
        }
    }

    /// Un parrafo partido en renglones, con su minuto delante si lo lleva.
    fn parrafo(&mut self, minuto: Option<(String, ColorRgba)>, cuerpo: &str, color: ColorRgba) {
        let x = MARGEN_A4 + if minuto.is_some() { SANGRIA } else { 0.0 };
        let ancho = ANCHO_A4 - MARGEN_A4 - x;
        let alto = LETRA * ex::INTERLINEA;
        let renglones = ex::partir_texto(cuerpo, LETRA, ancho);
        // El minuto no se queda solo al pie de una hoja.
        self.cabe(alto * renglones.len().clamp(1, 2) as f32);
        if let Some((m, c)) = minuto {
            self.ordenes
                .push(texto(m, MARGEN_A4, self.y, LETRA, c, true));
        }
        for r in renglones {
            self.cabe(alto);
            if !r.trim().is_empty() {
                self.ordenes.push(texto(r, x, self.y, LETRA, color, false));
            }
            self.y += alto;
        }
        self.y += LETRA * 0.5;
    }

    fn seccion(&mut self, nombre: &str) {
        self.y += LETRA_SECCION * 0.6;
        self.cabe(LETRA_SECCION * ex::INTERLINEA * 3.0);
        self.ordenes.push(texto(
            nombre.to_uppercase(),
            MARGEN_A4,
            self.y,
            LETRA_SECCION,
            TINTA_SUAVE,
            true,
        ));
        self.y += LETRA_SECCION * ex::INTERLINEA + 6.0;
    }
}

/// **La grabacion en hojas A4**: el titulo con la fecha y la duracion, los
/// marcadores, y la transcripcion con las capturas en su minuto. La
/// captura `i` va con el `id_objeto` `i + 1` (el cero es «sin imagen»).
fn hojas_de(c: &ClaseParaExportar, r: &Rotulos) -> Vec<Hoja> {
    let ancho = ANCHO_A4 - 2.0 * MARGEN_A4;
    let mut h = Paginas {
        hojas: Vec::new(),
        ordenes: Vec::new(),
        y: MARGEN_A4,
    };
    for l in ex::partir_texto(c.titulo.trim(), LETRA_TITULO, ancho) {
        h.ordenes
            .push(texto(l, MARGEN_A4, h.y, LETRA_TITULO, TINTA_TEXTO, true));
        h.y += LETRA_TITULO * ex::INTERLINEA;
    }
    let mut meta = Vec::new();
    if !c.fecha.trim().is_empty() {
        meta.push(c.fecha.trim().to_string());
    }
    if c.duracion() > 0 {
        meta.push(format!("{} {}", r.duracion, ce::minuto(c.duracion())));
    }
    if !meta.is_empty() {
        h.y += 2.0;
        h.ordenes.push(texto(
            meta.join("  ·  "),
            MARGEN_A4,
            h.y,
            LETRA,
            TINTA_SUAVE,
            false,
        ));
        h.y += LETRA * ex::INTERLINEA;
    }
    h.y += LETRA;
    if !c.marcadores.is_empty() {
        h.seccion(&r.marcadores);
        for m in &c.marcadores {
            let nota = if m.texto.trim().is_empty() {
                r.marcador.clone()
            } else {
                m.texto.trim().to_string()
            };
            h.parrafo(Some((ce::minuto(m.ms), MARCA)), &nota, TINTA_TEXTO);
        }
    }
    h.seccion(&r.transcripcion);
    if c.lineas.is_empty() {
        h.parrafo(None, &r.sin_texto, TINTA_SUAVE);
    }
    for f in ce::filas(c) {
        match f {
            Fila::Linea(i) => {
                let (ms, dicho) = &c.lineas[i];
                h.parrafo(ms.map(|ms| (ce::minuto(ms), ACENTO)), dicho, TINTA_TEXTO);
            }
            Fila::Marcador(i) => {
                let m = &c.marcadores[i];
                let nota = if m.texto.trim().is_empty() {
                    r.marcador.clone()
                } else {
                    format!("{}: {}", r.marcador, m.texto.trim())
                };
                h.parrafo(Some((ce::minuto(m.ms), MARCA)), &nota, MARCA);
            }
            Fila::Captura(i) => {
                let (ms, ruta) = &c.capturas[i];
                let Ok((w, alto)) = pixpin_codec::imagen::medidas(ruta) else {
                    continue;
                };
                let (w, alto) = (w.max(1) as f32, alto.max(1) as f32);
                let k = (ancho / w).min(ALTO_CAPTURA / alto);
                let (iw, ih) = (w * k, alto * k);
                let pie = LETRA * ex::INTERLINEA;
                h.y += 4.0;
                h.cabe(ih + pie + 8.0);
                h.ordenes.push(Orden::Imagen {
                    id_objeto: i as u64 + 1,
                    x: MARGEN_A4 + (ancho - iw) / 2.0,
                    y: h.y,
                    ancho: iw,
                    alto: ih,
                    opacidad: 1.0,
                    recorte: None,
                    angulo: 0.0,
                });
                h.y += ih + 4.0;
                h.ordenes.push(texto(
                    format!("{} {} · {}", r.captura, i + 1, ce::minuto(*ms)),
                    MARGEN_A4 + (ancho - iw) / 2.0,
                    h.y,
                    LETRA * 0.9,
                    TINTA_SUAVE,
                    false,
                ));
                h.y += pie + LETRA;
            }
        }
    }
    if !h.ordenes.is_empty() || h.hojas.is_empty() {
        h.hojas.push(hoja_en_blanco(std::mem::take(&mut h.ordenes)));
    }
    h.hojas
}

/// **Las hojas A4 de la grabacion**, con sus ordenes para pintarlas y las
/// fotos que usan. Siempre una al menos. Bloqueante (mide las capturas).
#[uniffi::export]
pub fn grabacion_hojas(g: GrabacionParaCompartir) -> HojasDeGrabacion {
    let c = g.datos();
    let hojas = hojas_de(&c, &Rotulos::default())
        .into_iter()
        .map(|h| {
            let (x0, y0, x1, y1) = h.caja;
            HojaDeDibujo {
                nombre: String::new(),
                es_marco: false,
                x0,
                y0,
                x1,
                y1,
                plano: crate::lienzo::empaquetar(&h.ordenes, BLANCO, Some(h.caja)),
                svg: String::new(),
                fondo: ex::hex(BLANCO),
            }
        })
        .collect();
    let fotos = g
        .capturas
        .iter()
        .enumerate()
        .map(|(i, c)| FotoDeGrabacion {
            id_objeto: i as u64 + 1,
            ruta: c.ruta.clone(),
        })
        .collect();
    HojasDeGrabacion { hojas, fotos }
}

/// **El SVG de la hoja `indice`** de [`grabacion_hojas`], con las capturas
/// dentro (ligeras, como en la pagina web). `None` si no hay esa hoja.
#[uniffi::export]
pub fn grabacion_svg_de_hoja(g: GrabacionParaCompartir, indice: u32) -> Option<String> {
    let c = g.datos();
    let hoja = hojas_de(&c, &Rotulos::default())
        .into_iter()
        .nth(indice as usize)?;
    let incrustada = |id: u64| -> Option<Incrustada> {
        let (_, ruta) = c.capturas.get((id as usize).checked_sub(1)?)?;
        match captura_ligera(ruta)? {
            Medio::Incrustado { bytes, .. } => Some(Incrustada {
                mime: crate::editor_exportar::mime_de(&bytes)?,
                bytes,
            }),
            Medio::AlLado(_) => None,
        }
    };
    Some(exportar_svg::svg(
        &hoja,
        OpcionesSvg {
            fondo: Some(BLANCO),
            marcos: false,
        },
        &incrustada,
    ))
}

// ---------------------------------------------------------------------------
// Los formatos

fn escribir(ruta: &Path, bytes: &[u8]) -> Result<(), ErrorNucleo> {
    std::fs::write(ruta, bytes).map_err(|e| ErrorNucleo::Disco {
        detalle: format!("no se pudo escribir {}: {e}", ruta.display()),
    })
}

/// Copia (o enlaza, que es instantaneo) un fichero grande junto a la pagina.
fn al_lado(origen: &Path, destino: &Path) -> std::io::Result<()> {
    if destino.is_file() {
        let _ = std::fs::remove_file(destino);
    }
    if std::fs::hard_link(origen, destino).is_err() {
        std::fs::copy(origen, destino)?;
    }
    Ok(())
}

/// Una captura para dentro de la pagina: tal cual si es ligera; si no, en
/// JPEG y sin pasar de [`LADO_CAPTURA`].
fn captura_ligera(ruta: &Path) -> Option<Medio> {
    let tal_cual = || {
        Some(Medio::Incrustado {
            mime: ce::mime_de(ruta).to_string(),
            bytes: std::fs::read(ruta).ok()?,
        })
    };
    if std::fs::metadata(ruta).ok()?.len() <= CAPTURA_LIGERA {
        return tal_cual();
    }
    let Ok(mut img) = pixpin_codec::cargar(ruta) else {
        return tal_cual();
    };
    let lado = img.ancho.max(img.alto);
    if lado > LADO_CAPTURA {
        let k = LADO_CAPTURA as f32 / lado as f32;
        let (w, h) = (
            ((img.ancho as f32 * k).round() as u32).max(1),
            ((img.alto as f32 * k).round() as u32).max(1),
        );
        if let Ok(r) = pixpin_codec::imagen::redimensionar(img.clone(), w, h) {
            img = r;
        }
    }
    let opaca = img.pixeles.chunks_exact(4).all(|p| p[3] == 255);
    let bytes = if opaca {
        pixpin_codec::imagen::codificar_jpg(&img, 85).ok()
    } else {
        pixpin_codec::codificar_png(&img).ok()
    };
    match bytes {
        Some(b) => Some(Medio::Incrustado {
            mime: if opaca { "image/jpeg" } else { "image/png" }.into(),
            bytes: b,
        }),
        None => tal_cual(),
    }
}

fn rutas(v: Vec<PathBuf>) -> Vec<String> {
    v.iter().map(|r| texto_de(r)).collect()
}

/// **La pagina web de la grabacion** en `carpeta`: la pagina (la primera)
/// y, al lado, lo que no cabe dentro (el video, un audio muy largo, los
/// adjuntos). `base` es el nombre de fichero de lo compartido.
#[uniffi::export]
pub fn grabacion_web(
    g: GrabacionParaCompartir,
    base: String,
    carpeta: String,
) -> Result<Vec<String>, ErrorNucleo> {
    let c = g.datos();
    let r = Rotulos::default();
    let carpeta = Path::new(&carpeta);
    let mut ficheros = Vec::new();
    let mut al_lado_de: Vec<(PathBuf, String)> = Vec::new();
    let mut poner_al_lado = |origen: &Path, que: &str| -> Option<String> {
        let ext = origen
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        let nombre = format!("{base} - {que}{ext}");
        let destino = carpeta.join(&nombre);
        al_lado(origen, &destino).ok()?;
        ficheros.push(destino);
        al_lado_de.push((origen.to_path_buf(), nombre.clone()));
        Some(nombre)
    };
    let nombre = |s: &str| crate::compartir::nombre_de_fichero_compartido(s.to_string());
    if let Some(a) = &c.audio
        && std::fs::metadata(a).is_ok_and(|m| m.len() > AUDIO_DENTRO)
    {
        poner_al_lado(a, &nombre(&r.audio.to_lowercase()));
    }
    if let Some(v) = &c.video {
        poner_al_lado(v, &nombre(&r.video.to_lowercase()));
    }
    for a in &c.adjuntos {
        let tronco = a
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        poner_al_lado(a, &nombre(&tronco));
    }
    let medio = |ruta: &Path| -> Option<Medio> {
        if let Some((_, n)) = al_lado_de.iter().find(|(o, _)| o == ruta) {
            return Some(Medio::AlLado(n.clone()));
        }
        if c.capturas.iter().any(|(_, r)| r == ruta) {
            return captura_ligera(ruta);
        }
        Some(Medio::Incrustado {
            mime: ce::mime_de(ruta).to_string(),
            bytes: std::fs::read(ruta).ok()?,
        })
    };
    let pagina = ce::html(&c, &r, &medio);
    let ruta = carpeta.join(format!("{base}.html"));
    escribir(&ruta, pagina.as_bytes())?;
    ficheros.insert(0, ruta);
    Ok(rutas(ficheros))
}

/// **Markdown**: solo el `.md` si no hay capturas; con ellas, el ZIP con
/// la carpeta de las capturas al lado.
#[uniffi::export]
pub fn grabacion_markdown(
    g: GrabacionParaCompartir,
    base: String,
    carpeta: String,
) -> Result<Vec<String>, ErrorNucleo> {
    let c = g.datos();
    let r = Rotulos::default();
    let carpeta = Path::new(&carpeta);
    if c.capturas.is_empty() {
        let ruta = carpeta.join(format!("{base}.md"));
        escribir(&ruta, ce::markdown(&c, &r, &|_: &Path| None).as_bytes())?;
        return Ok(rutas(vec![ruta]));
    }
    let bytes =
        ce::paquete_markdown(&c, &r, &base, &|r: &Path| std::fs::read(r).ok()).map_err(|e| {
            ErrorNucleo::Disco {
                detalle: e.to_string(),
            }
        })?;
    let ruta = carpeta.join(format!("{base} (Markdown).zip"));
    escribir(&ruta, &bytes)?;
    Ok(rutas(vec![ruta]))
}

/// **Texto**: la transcripcion con sus minutos.
#[uniffi::export]
pub fn grabacion_texto(
    g: GrabacionParaCompartir,
    base: String,
    carpeta: String,
) -> Result<Vec<String>, ErrorNucleo> {
    let ruta = Path::new(&carpeta).join(format!("{base}.txt"));
    escribir(&ruta, ce::texto(&g.datos(), &Rotulos::default()).as_bytes())?;
    Ok(rutas(vec![ruta]))
}

/// Los bytes de una foto como los quiere Word (`pixpin_notas::word::
/// bytes_de_foto` de Windows): tal cual si es PNG, JPEG, GIF o BMP; si no,
/// pasada a PNG.
fn bytes_de_foto(ruta: &Path) -> Option<Vec<u8>> {
    let bytes = std::fs::read(ruta).ok()?;
    if pixpin_docs::exportar_docx::imagen::leer(&bytes).is_some() {
        return Some(bytes);
    }
    let rgba = pixpin_codec::cargar(ruta).ok()?;
    pixpin_codec::codificar_png(&rgba).ok()
}

/// **Word**: el mismo Markdown por el escritor de `.docx` de las notas, con
/// las capturas dentro (`clase::word` de Windows, por
/// `pixpin_notas::word::exportar`).
#[uniffi::export]
pub fn grabacion_word(
    g: GrabacionParaCompartir,
    base: String,
    carpeta: String,
) -> Result<Vec<String>, ErrorNucleo> {
    let c = g.datos();
    let nombres: Vec<(PathBuf, String)> = c
        .capturas
        .iter()
        .enumerate()
        .map(|(i, (ms, r))| (r.clone(), ce::nombre_de_captura(i, *ms, r)))
        .collect();
    let md = ce::markdown(&c, &Rotulos::default(), &|ruta: &Path| {
        nombres
            .iter()
            .find(|(r, _)| r == ruta)
            .map(|(_, n)| n.clone())
    });
    let fotos = |r: &str| {
        nombres
            .iter()
            .find(|(_, n)| n == r)
            .and_then(|(ruta, _)| bytes_de_foto(ruta))
    };
    let titulo = match pixpin_docs::md_vivo::titulo(&md) {
        t if t.trim().is_empty() => c.titulo.clone(),
        t => t,
    };
    let bytes = pixpin_docs::exportar_docx::exportar(&pixpin_docs::exportar_docx::Nota {
        markdown: &md,
        titulo: &titulo,
        letra: pixpin_docs::exportar_docx::Letra::default(),
        comentarios: None,
        imagen: &fotos,
        autor: "",
        ahora_ms: crate::ahora_ms(),
    })
    .map_err(|e| ErrorNucleo::Disco {
        detalle: format!("no se pudo hacer el Word: {e}"),
    })?;
    let ruta = Path::new(&carpeta).join(format!("{base}.docx"));
    escribir(&ruta, &bytes)?;
    Ok(rutas(vec![ruta]))
}

/// Anki, con la carpeta de medios dada (en las pruebas, ninguna).
fn anki_en(
    g: &GrabacionParaCompartir,
    base: &str,
    carpeta: &Path,
    pregunta: &str,
    medios: Option<&Path>,
) -> Result<Vec<PathBuf>, ErrorNucleo> {
    let c = g.datos();
    let trozos: Vec<(i64, String)> = c
        .lineas
        .iter()
        .filter_map(|(ms, t)| Some(((*ms)?, t.clone())))
        .collect();
    let tronco = crate::compartir::nombre_de_fichero_compartido(c.titulo.clone()).replace(' ', "-");
    let capturas: Vec<(i64, pixpin_anki::Imagen)> = c
        .capturas
        .iter()
        .filter_map(|(ms, r)| {
            let ext = r
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_else(|| "png".into());
            Some((
                *ms,
                pixpin_anki::Imagen {
                    nombre: format!("pixpin-{tronco}-{ms}.{ext}"),
                    bytes: std::fs::read(r).ok()?,
                },
            ))
        })
        .collect();
    let mut tarjetas = pixpin_anki::tarjetas::de_la_clase(&c.titulo, pregunta, &trozos, capturas);
    if tarjetas.is_empty() {
        return Err(ErrorNucleo::NoExiste {
            que: "tarjetas".into(),
        });
    }
    for t in &mut tarjetas {
        t.etiquetas = vec!["pixpin".to_string()];
    }
    let ruta = carpeta.join(format!("{base} (Anki).txt"));
    escribir(
        &ruta,
        pixpin_anki::fichero::texto(&g.mazo, &tarjetas).as_bytes(),
    )?;
    let mut v = vec![ruta];
    for i in tarjetas.iter().flat_map(|t| &t.imagenes) {
        let r = carpeta.join(&i.nombre);
        escribir(&r, &i.bytes)?;
        v.push(r);
        if let Some(m) = medios {
            let _ = std::fs::write(m.join(&i.nombre), &i.bytes);
        }
    }
    Ok(v)
}

/// **Anki**: las tarjetas de la grabacion (una por captura, con lo dicho en
/// su tramo) en el fichero de «Archivo → Importar», y sus imagenes al lado.
/// Como el «Enviar a Anki» sin Anki abierto, las imagenes se dejan tambien
/// en la carpeta de medios de Anki, si esta: son las mismas con el mismo
/// nombre, asi que repetirlo no ensucia. `pregunta` es
/// `anki-pregunta-clase`.
#[uniffi::export]
pub fn grabacion_anki(
    g: GrabacionParaCompartir,
    base: String,
    carpeta: String,
    pregunta: String,
) -> Result<Vec<String>, ErrorNucleo> {
    let medios = crate::anki::medios_de_anki();
    anki_en(&g, &base, Path::new(&carpeta), &pregunta, medios.as_deref()).map(rutas)
}

#[uniffi::export]
impl Nucleo {
    /// **La grabacion del mensaje `mensaje`** de `chat` para compartirla:
    /// el de una nota de voz o el de la carpeta de una grabacion. `None` si
    /// no es ninguna de las dos. La `fecha` va vacia: la escribe Swift con
    /// la hora de este Mac. Bloqueante (lee el chat entero).
    pub fn grabacion_para_compartir(
        &self,
        chat: String,
        mensaje: String,
    ) -> Result<Option<GrabacionParaCompartir>, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta();
        let mensajes = cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, &chat))?.mensajes;
        let Some(m) = mensajes.iter().find(|m| m.id == mensaje) else {
            return Err(ErrorNucleo::NoExiste { que: mensaje });
        };
        let Some(voz) = voz_de(m, &mensajes) else {
            return Ok(None);
        };
        let ruta = |m: &Mensaje| -> Option<PathBuf> {
            let relativa = m.ruta.as_deref().filter(|r| !r.is_empty())?;
            self.resolver(&chat, relativa)
                .filter(|r| r.is_file())
                .map(|r| pixpin_proyecto::vista::con_la_extension_del_nombre(raiz, r, &m.nombre))
        };
        let datos =
            ClaseParaExportar::de_la_nota(voz, &mensajes, lineas_de(voz), String::new(), &ruta);
        let nombre = Indice::leer(raiz)
            .buscar(&chat)
            .map(|f| f.nombre.clone())
            .unwrap_or_default();
        Ok(Some(GrabacionParaCompartir::de(
            &datos,
            voz.cuando,
            crate::anki::mazo_de(&nombre),
        )))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-grabacion-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn png(ancho: u32, alto: u32) -> Vec<u8> {
        pixpin_codec::codificar_png(&pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles: vec![200; (ancho * alto * 4) as usize],
        })
        .unwrap()
    }

    /// La grabacion de Windows (`compartir/clase.rs`, `grabacion`).
    fn grabacion(dir: &Path) -> GrabacionParaCompartir {
        let captura = dir.join("c1.png");
        std::fs::write(&captura, png(8, 6)).unwrap();
        let audio = dir.join("clase.m4a");
        std::fs::write(&audio, b"aac").unwrap();
        let video = dir.join("clase.mp4");
        std::fs::write(&video, b"mp4").unwrap();
        let c = ClaseParaExportar {
            titulo: "Clase 1".into(),
            fecha: "hoy".into(),
            duracion_ms: 60_000,
            audio: Some(audio),
            video: Some(video),
            capturas: vec![(10_000, captura)],
            lineas: vec![(Some(0), "hola".into()), (Some(20_000), "adios".into())],
            marcadores: vec![Marcador {
                ms: 5_000,
                texto: "ojo".into(),
            }],
            adjuntos: vec![],
        };
        GrabacionParaCompartir::de(&c, 0, "PixPin::Fisica".into())
    }

    #[test]
    fn ida_y_vuelta_sin_perder_nada() {
        let dir = temporal("ida");
        let g = grabacion(&dir);
        assert_eq!(GrabacionParaCompartir::de(&g.datos(), 0, g.mazo.clone()), g);
        assert_eq!(
            g.originales.len(),
            3,
            "audio, video y captura: {:?}",
            g.originales
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_web_lleva_el_audio_dentro_y_el_video_al_lado() {
        let dir = temporal("web");
        let g = grabacion(&dir);
        let salida = dir.join("salida");
        std::fs::create_dir_all(&salida).unwrap();
        let s = salida.display().to_string();
        let v = grabacion_web(g, "Clase 1".into(), s).unwrap();
        assert_eq!(v[0], texto_de(&salida.join("Clase 1.html")));
        assert_eq!(v[1], texto_de(&salida.join("Clase 1 - vídeo.mp4")));
        let h = std::fs::read_to_string(&v[0]).unwrap();
        assert!(h.contains("<audio id=\"audio\" src=\"data:audio/mp4;base64,"));
        assert!(h.contains("src=\"Clase%201%20-%20v%C3%ADdeo.mp4\""));
        assert!(h.contains("data:image/png;base64,"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_markdown_va_en_zip_con_capturas_y_solo_sin_ellas() {
        let dir = temporal("md");
        let mut g = grabacion(&dir);
        let s = dir.display().to_string();
        let v = grabacion_markdown(g.clone(), "Clase 1".into(), s.clone()).unwrap();
        assert!(v[0].ends_with("Clase 1 (Markdown).zip"));
        assert!(std::fs::read(&v[0]).unwrap().starts_with(b"PK"));
        g.capturas.clear();
        let v = grabacion_markdown(g, "Clase 1".into(), s).unwrap();
        assert!(v[0].ends_with("Clase 1.md"));
        assert!(
            std::fs::read_to_string(&v[0])
                .unwrap()
                .starts_with("# Clase 1")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_texto_lleva_los_minutos() {
        let dir = temporal("txt");
        let v =
            grabacion_texto(grabacion(&dir), "Clase 1".into(), dir.display().to_string()).unwrap();
        let t = std::fs::read_to_string(&v[0]).unwrap();
        assert!(t.starts_with("Clase 1\n"));
        assert!(t.contains("[0:20] adios"), "{t}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_word_lleva_las_capturas_y_el_texto() {
        let dir = temporal("word");
        let v =
            grabacion_word(grabacion(&dir), "Clase 1".into(), dir.display().to_string()).unwrap();
        assert!(v[0].ends_with("Clase 1.docx"));
        let bytes = std::fs::read(&v[0]).unwrap();
        let mut p = pixpin_docs::Paquete::de_bytes(&bytes).unwrap();
        let nombres = p.nombres();
        assert!(
            nombres.iter().any(|n| n.starts_with("word/media/")),
            "{nombres:?}"
        );
        let d = pixpin_docs::docx::de_paquete(&mut p, "x").unwrap();
        let todo: String = d.bloques.iter().map(|b| b.texto()).collect();
        assert!(todo.contains("adios"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn anki_deja_el_fichero_y_sus_imagenes() {
        let dir = temporal("anki");
        let g = grabacion(&dir);
        let medios = dir.join("medios");
        std::fs::create_dir_all(&medios).unwrap();
        let v = anki_en(&g, "Clase 1", &dir, "¿Qué?", Some(&medios)).unwrap();
        assert_eq!(v.len(), 2);
        let t = std::fs::read_to_string(&v[0]).unwrap();
        assert!(t.contains("PixPin::Fisica"));
        assert!(v[1].is_file());
        assert!(
            medios.join(v[1].file_name().unwrap()).is_file(),
            "tambien en los medios de Anki"
        );
        // Caso negativo: sin capturas no hay tarjetas.
        let mut sin = g.clone();
        sin.capturas.clear();
        assert!(anki_en(&sin, "Clase 1", &dir, "¿Qué?", None).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn las_paginas_llevan_la_captura_y_el_texto() {
        let dir = temporal("hojas");
        let g = grabacion(&dir);
        let hojas = hojas_de(&g.datos(), &Rotulos::default());
        assert_eq!(hojas.len(), 1);
        let ordenes = &hojas[0].ordenes;
        assert!(
            ordenes
                .iter()
                .any(|o| matches!(o, Orden::Imagen { id_objeto: 1, .. }))
        );
        let textos: Vec<&str> = ordenes
            .iter()
            .filter_map(|o| match o {
                Orden::Texto { texto, .. } => Some(texto.as_str()),
                _ => None,
            })
            .collect();
        let hola = textos.iter().position(|t| *t == "hola").unwrap();
        let adios = textos.iter().position(|t| *t == "adios").unwrap();
        let pie = textos
            .iter()
            .position(|t| t.starts_with("Captura 1"))
            .unwrap();
        assert!(hola < pie && pie < adios);

        // Lo que ve Swift: la hoja A4 con su foto, y el SVG con la captura.
        let h = grabacion_hojas(g.clone());
        assert_eq!(h.hojas.len(), 1);
        assert_eq!((h.hojas[0].x1, h.hojas[0].y1), (ANCHO_A4, ALTO_A4));
        assert!(h.hojas[0].plano.imagenes.contains(&1));
        assert_eq!(
            h.fotos,
            vec![FotoDeGrabacion {
                id_objeto: 1,
                ruta: g.capturas[0].ruta.clone()
            }]
        );
        let svg = grabacion_svg_de_hoja(g.clone(), 0).unwrap();
        assert!(svg.contains("<svg") && svg.contains("data:image/png;base64,"));
        assert!(grabacion_svg_de_hoja(g, 1).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn muchas_lineas_dan_varias_hojas() {
        let dir = temporal("largas");
        let mut g = grabacion(&dir);
        g.lineas = (0..200)
            .map(|i| LineaDeGrabacion {
                ms: Some(i * 1000),
                texto: format!("renglon {i}"),
            })
            .collect();
        assert!(grabacion_hojas(g).hojas.len() > 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_nota_de_voz_y_su_carpeta_se_comparten_igual() {
        let d = temporal("nucleo");
        let n = Nucleo::abrir(d.join("raiz").display().to_string(), "Mac".into()).unwrap();
        let chat = n.crear_chat("Física".into()).unwrap().id;
        let raiz = d.join("raiz");
        let carpeta = almacen::carpeta(&raiz, &chat);
        let sello = |numero: i64| cuaderno::Sello {
            cuando: 1_000,
            numero,
            aparato: "MAC1".into(),
            proyecto: chat.clone(),
        };
        let mut voz = Mensaje::adjunto(cuaderno::Clase::Voz, "Clase 1.m4a", "", 0, &sello(1));
        voz.id = "v1".into();
        voz.transcripcion = Some("[0:00] hola\n\n[0:20] **adios**".into());
        let mut nota = Mensaje::nota("solo texto", &sello(2));
        nota.id = "n1".into();
        let mut carp = pixpin_proyecto::carpeta_de_paginas::mensaje_de_grabacion(
            "Clase 1",
            &Carpeta::grabacion("v1", 60_000),
            &sello(3),
        );
        carp.id = "c1".into();
        for m in [&voz, &nota, &carp] {
            cuaderno::anadir(&carpeta, m).unwrap();
        }

        let g = n
            .grabacion_para_compartir(chat.clone(), "v1".into())
            .unwrap()
            .unwrap();
        assert_eq!(g.titulo, "Clase 1");
        assert_eq!(g.cuando, 1_000);
        assert_eq!(g.mazo, "PixPin::Física");
        assert_eq!(
            g.lineas[1],
            LineaDeGrabacion {
                ms: Some(20_000),
                texto: "adios".into()
            }
        );
        assert!(g.fecha.is_empty(), "la fecha la pone Swift");
        let por_carpeta = n
            .grabacion_para_compartir(chat.clone(), "c1".into())
            .unwrap()
            .unwrap();
        assert_eq!(por_carpeta, g);
        // Casos negativos: una nota no es una grabacion, y un id que no esta.
        assert!(
            n.grabacion_para_compartir(chat.clone(), "n1".into())
                .unwrap()
                .is_none()
        );
        assert!(n.grabacion_para_compartir(chat, "nada".into()).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
