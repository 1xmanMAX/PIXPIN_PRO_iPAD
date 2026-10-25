//! **La zona, al chat del proyecto, y el salto por su enlace** (F8;
//! `zona_al_chat.rs` y `salto_por_enlace.rs` de Windows, `mandarLaZona` y
//! `abrirSublienzo` del movil).
//!
//! Con «Zona al chat», la foto de la zona va al chat del proyecto como
//! **sublienzo**, paso a paso como Windows:
//!
//! 1. el lienzo de origen tiene que ser una hoja del proyecto; si no lo era,
//!    lo pasa a ser;
//! 2. la foto entra en el chat como adjunto (`archivos/…`);
//! 3. el sublienzo es un lienzo propio (`foto-<id del mensaje>`) con la foto
//!    clavada;
//! 4. su hoja va **detras de su lienzo** (y de los sublienzos que ya
//!    tuviera), con `padre` y `deMensaje`, y se llama «↳ Zona de…»;
//! 5. el mensaje es una IMAGEN con `referencia` al sublienzo, `unido` y
//!    `vieneDe` («Lienzo «Planta»» o «PDF «Plano» → página 3»).
//!
//! La marca con enlace en el origen la pone el editor
//! ([`EditorLienzo::marcar_zona`], `pixpin_motor2d::zona::marcar`), en un
//! paso de deshacer. Tocar su icono lleva al sublienzo
//! ([`EditorLienzo::enlace_en`] y [`Nucleo::dibujo_del_enlace`]).

use std::path::Path;

use pixpin_motor2d::vector::Punto2;
use pixpin_proyecto::cuaderno::{self, Mensaje};
use pixpin_proyecto::{Hoja, Proyecto, almacen};
use serde_json::{Value, json};

use crate::editor::EditorLienzo;
use crate::{ErrorNucleo, MensajeChat, Nucleo};

/// Lo que quedo hecho al mandar la zona.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ZonaMandada {
    /// El dibujo del sublienzo: el `enlace` de la marca.
    pub dibujo: String,
    /// El nombre del proyecto, para decirlo («Zona mandada al chat de…»).
    pub proyecto: String,
    /// El mensaje IMAGEN nuevo del chat.
    pub mensaje: MensajeChat,
}

/// Un icono de enlace a la vista: donde esta (la esquina de arriba a la
/// derecha de su marca, en el mundo) y a que dibujo lleva.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct EnlaceDelLienzo {
    pub dibujo: String,
    pub x: f32,
    pub y: f32,
}

/// El radio del icono de enlace en puntos de pantalla (lo pinta Swift).
#[uniffi::export]
pub fn radio_del_icono_de_enlace() -> f32 {
    pixpin_motor2d::zona::RADIO_DEL_ICONO_DE_ENLACE
}

fn argumento(detalle: impl ToString) -> ErrorNucleo {
    ErrorNucleo::Argumento {
        detalle: detalle.to_string(),
    }
}

fn leer_proyecto(carpeta: &Path) -> Proyecto {
    std::fs::read_to_string(carpeta.join("proyecto.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Al lado y luego de un tiron: un corte a mitad no deja el fichero roto.
fn escribir_atomico(ruta: &Path, texto: &str) -> std::io::Result<()> {
    if let Some(dir) = ruta.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut temporal = ruta.as_os_str().to_owned();
    temporal.push(".zona.tmp");
    let temporal = std::path::PathBuf::from(temporal);
    std::fs::write(&temporal, texto)?;
    std::fs::rename(&temporal, ruta)
}

/// El `.excalidraw` del sublienzo: la foto a su tamano en pixeles, en el
/// origen y bloqueada, como `hojaDeLaFotoDelChat` del movil.
fn lienzo_con_foto(foto: &str, ancho: u32, alto: u32, ahora: i64) -> String {
    json!({
        "type": "excalidraw",
        "version": 2,
        "source": "pixpin-max",
        "elements": [{
            "id": format!("foto-{ahora}-0"),
            "type": "image",
            "x": 0, "y": 0,
            "width": ancho.max(1) as f64,
            "height": alto.max(1) as f64,
            "angle": 0,
            "strokeColor": "transparent",
            "backgroundColor": "transparent",
            "fillStyle": "solid",
            "strokeWidth": 1,
            "strokeStyle": "solid",
            "roughness": 0,
            "opacity": 100,
            "groupIds": [],
            "seed": 1,
            "version": 1,
            "versionNonce": 1,
            "isDeleted": false,
            "boundElements": null,
            "locked": true,
            "fileId": foto,
            "status": "saved",
            "scale": [1, 1]
        }],
        "appState": {"viewBackgroundColor": "#ffffff"},
        "files": {
            foto: {"id": foto, "mimeType": "image/png", "path": format!("imagenes/{foto}"), "created": ahora}
        }
    })
    .to_string()
}

/// Si `s` es el `-<hora>` que pone el movil al importar (13 cifras).
fn es_sufijo_de_hora(s: &str) -> bool {
    s.strip_prefix('-')
        .is_some_and(|n| n.len() >= 10 && n.chars().all(|c| c.is_ascii_digit()))
}

/// Un enlace que puede ser un id de dibujo (sin rutas ni extension).
fn normalizar(enlace: &str) -> Option<String> {
    let e = enlace.trim();
    let e = e.strip_suffix(".excalidraw").unwrap_or(e);
    let e = e.rsplit('/').next().unwrap_or(e);
    (!e.is_empty() && e != "." && e != ".." && !e.contains(['\\', '\0'])).then(|| e.to_string())
}

/// **El dibujo al que lleva `enlace`** en el proyecto, si existe su lienzo
/// (`dibujo_del_enlace` de Windows): tal cual; si no, el de la hoja que
/// salio de ese mensaje (`deMensaje`) o el del mensaje con ese id; y si no,
/// el mismo nombre con el sufijo de importacion del movil.
pub(crate) fn dibujo_del_enlace(raiz: &Path, proyecto: &str, enlace: &str) -> Option<String> {
    let e = normalizar(enlace)?;
    let existe = |d: &str| almacen::lienzo(raiz, proyecto, d).is_file();
    if existe(&e) {
        return Some(e);
    }
    let carpeta = almacen::carpeta(raiz, proyecto);
    if let Some(id) = e.strip_prefix("foto-").filter(|id| !id.is_empty()) {
        let p = leer_proyecto(&carpeta);
        let por_hoja = p.hojas.iter().find_map(|h| {
            (h.resto.get("deMensaje").and_then(|v| v.as_str()) == Some(id))
                .then(|| h.dibujo.clone())
                .flatten()
        });
        if let Some(d) = por_hoja.filter(|d| existe(d)) {
            return Some(d);
        }
        let por_mensaje = cuaderno::Cuaderno::leer_de(&carpeta)
            .ok()
            .and_then(|c| c.mensajes.into_iter().find(|m| m.id == id))
            .and_then(|m| m.referencia);
        if let Some(d) = por_mensaje.filter(|d| existe(d)) {
            return Some(d);
        }
    }
    if let Some(sufijo) = proyecto.strip_prefix("pr").filter(|s| es_sufijo_de_hora(s)) {
        let d = format!("{e}{sufijo}");
        if existe(&d) {
            return Some(d);
        }
    }
    // Con otra hora: solo si no hay duda de cual es.
    let candidatos: Vec<String> = std::fs::read_dir(carpeta.join("lienzos"))
        .ok()?
        .filter_map(|f| f.ok())
        .filter_map(|f| {
            let n = f.file_name().to_string_lossy().to_string();
            let tronco = n.strip_suffix(".excalidraw")?.to_string();
            tronco
                .strip_prefix(e.as_str())
                .is_some_and(es_sufijo_de_hora)
                .then_some(tronco)
        })
        .collect();
    match candidatos.as_slice() {
        [uno] => Some(uno.clone()),
        _ => None,
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Manda la zona al chat del proyecto** `chat` (`mandarLaZona`):
    /// `dibujo` es el lienzo de origen (`lienzos/<dibujo>.excalidraw`),
    /// `pagina` la del PDF del proyecto si es una hoja-pagina, `nombre` como
    /// se llama en el chat y `png` la foto de la zona (`ancho` x `alto`
    /// pixeles). Despues hay que poner la marca con
    /// [`EditorLienzo::marcar_zona`] y el `dibujo` que devuelve. Bloqueante.
    #[allow(clippy::too_many_arguments)]
    pub fn zona_al_chat(
        &self,
        chat: String,
        dibujo: String,
        pagina: Option<u32>,
        nombre: String,
        png: Vec<u8>,
        ancho: u32,
        alto: u32,
    ) -> Result<ZonaMandada, ErrorNucleo> {
        crate::chats::validar_chat(&chat)?;
        let dibujo = normalizar(&dibujo).ok_or_else(|| argumento("dibujo no valido"))?;
        if png.is_empty() {
            return Err(argumento("zona sin foto"));
        }
        let raiz = self.ruta();
        let ahora = crate::ahora_ms();
        let carpeta = almacen::carpeta(raiz, &chat);
        let mut indice = almacen::Indice::leer_si_esta(raiz)?;
        let mut p = leer_proyecto(&carpeta);
        if p.id.is_empty() {
            // Uno nacido aqui no tiene `proyecto.json` todavia.
            p.id = chat.clone();
            p.nombre = indice.buscar(&chat).map(|f| f.nombre.clone()).unwrap_or_default();
        }
        let es_pdf = pagina.is_some();
        let nombre = nombre.strip_suffix(".excalidraw").unwrap_or(&nombre).trim().to_string();

        // 1. El lienzo de origen, hoja del proyecto.
        let origen = match p
            .hojas
            .iter()
            .position(|x| x.dibujo.as_deref() == Some(dibujo.as_str()) || (es_pdf && x.pagina == pagina))
        {
            Some(i) => p.hojas[i].clone(),
            None => {
                let nueva = Hoja {
                    id: format!("h-{ahora}"),
                    nombre: if nombre.is_empty() { "Lienzo".into() } else { nombre.clone() },
                    dibujo: Some(dibujo.clone()),
                    pagina,
                    ..Default::default()
                };
                p.hojas.push(nueva.clone());
                nueva
            }
        };

        // Los textos del movil: de donde viene y como se llama.
        let (de_donde, titulo) = match pagina {
            Some(n) => (
                format!("PDF «{}» → página {}", p.nombre, n + 1),
                format!("Zona de la página {}", n + 1),
            ),
            None => {
                let de = if origen.nombre.trim().is_empty() { p.nombre.clone() } else { origen.nombre.clone() };
                let suyo = if origen.nombre.trim().is_empty() { "lienzo".to_string() } else { origen.nombre.clone() };
                (format!("Lienzo «{de}»"), format!("Zona de {suyo}"))
            }
        };
        let fichero = format!("{titulo}.png");

        // 2. La foto, adjunta al chat.
        let aparato = self.identidad()?.yo.codigo();
        let fichero_cuaderno = carpeta.join("guardados.jsonl");
        let mut cache = self.cache_tomada();
        let numero = cache.indice(&fichero_cuaderno)?.map_or(0, |i| i.mayor_numero) + 1;
        let ruta = almacen::guardar_adjunto(raiz, &chat, &fichero, &png)?;
        let mut m = Mensaje::adjunto(
            cuaderno::Clase::Imagen,
            &fichero,
            &ruta,
            png.len() as i64,
            &cuaderno::Sello {
                cuando: crate::chats::hora_unica(),
                numero,
                aparato,
                proyecto: chat.clone(),
            },
        );
        let sublienzo = format!("foto-{}", m.id);

        // 3. El sublienzo: la foto clavada en su propio lienzo.
        let foto = format!("zona{ahora}");
        std::fs::create_dir_all(carpeta.join("imagenes"))?;
        std::fs::write(carpeta.join("imagenes").join(&foto), &png)?;
        let lienzo = almacen::lienzo(raiz, &chat, &sublienzo);
        if !lienzo.exists() {
            escribir_atomico(&lienzo, &lienzo_con_foto(&foto, ancho, alto, ahora))?;
        }

        // 4. Su hoja, detras de su lienzo y de sus otros sublienzos.
        let mut hoja = Hoja {
            id: format!("hoja-{ahora}"),
            nombre: format!("↳ {titulo}"),
            dibujo: Some(sublienzo.clone()),
            uid: Some(m.codigo_unico()),
            ..Default::default()
        };
        hoja.resto.insert("padre".into(), Value::String(origen.id.clone()));
        hoja.resto.insert("deMensaje".into(), Value::String(m.id.clone()));
        let donde = p
            .hojas
            .iter()
            .rposition(|x| x.id == origen.id || x.resto.get("padre").and_then(|v| v.as_str()) == Some(origen.id.as_str()))
            .map_or(p.hojas.len(), |i| i + 1);
        p.hojas.insert(donde.min(p.hojas.len()), hoja);
        p.tocado = ahora;
        escribir_atomico(&carpeta.join("proyecto.json"), &serde_json::to_string(&p).map_err(argumento)?)?;

        // 5. El mensaje, con los campos del movil.
        m.referencia = Some(sublienzo.clone());
        m.resto.insert("unido".into(), Value::Bool(true));
        let mut viene = serde_json::Map::new();
        viene.insert("texto".into(), Value::String(de_donde));
        viene.insert("dibujo".into(), Value::String(dibujo.clone()));
        if es_pdf && let Some(pdf) = &p.pdf_origen {
            viene.insert("pdf".into(), Value::String(pdf.clone()));
        }
        if let Some(n) = pagina {
            viene.insert("pagina".into(), json!(n));
        }
        viene.insert("proyecto".into(), Value::String(p.id.clone()));
        m.resto.insert("vieneDe".into(), Value::Object(viene));
        cuaderno::anadir(&carpeta, &m)?;
        cache.quitar(&fichero_cuaderno);
        drop(cache);

        // Y el proyecto sube en la lista, como con cualquier adjunto.
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == chat) {
            f.tocado = ahora;
            f.resumen = fichero.clone();
            let _ = indice.guardar(raiz);
        }
        Ok(ZonaMandada {
            dibujo: sublienzo,
            proyecto: p.nombre,
            mensaje: MensajeChat::from(m),
        })
    }

    /// **El dibujo al que lleva un enlace** (el de la marca de una zona) en
    /// el chat `chat`, si existe su lienzo. `None`: el enlace no lleva a
    /// ningun lienzo de este proyecto.
    pub fn dibujo_del_enlace(&self, chat: String, enlace: String) -> Option<String> {
        crate::chats::validar_chat(&chat).ok()?;
        dibujo_del_enlace(self.ruta(), &chat, &enlace)
    }

    /// **El mensaje del chat cuya hoja es `dibujo`** (el `DIBUJO` o la
    /// `IMAGEN` con esa `referencia`, el ultimo), para abrirlo. `None` si no
    /// hay ninguno.
    pub fn mensaje_del_dibujo(&self, chat: String, dibujo: String) -> Option<MensajeChat> {
        crate::chats::validar_chat(&chat).ok()?;
        cuaderno::Cuaderno::leer_de(&almacen::carpeta(self.ruta(), &chat))
            .ok()?
            .mensajes
            .into_iter()
            .rev()
            .find(|m| m.referencia.as_deref() == Some(dibujo.as_str()))
            .map(MensajeChat::from)
    }
}

#[uniffi::export]
impl EditorLienzo {
    /// **La marca con enlace** de una zona mandada al chat: un rectangulo
    /// azul a trazos con la caja de la zona, que lleva a `dibujo`. Un paso de
    /// deshacer. Devuelve si se puso.
    pub fn marcar_zona(&self, x0: f32, y0: f32, x1: f32, y1: f32, dibujo: String) -> bool {
        if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) || dibujo.trim().is_empty() {
            return false;
        }
        let caja = (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
        {
            let mut st = self.tomar();
            pixpin_motor2d::zona::marcar(&mut st.escena, caja, dibujo.trim());
        }
        self.subir_cambios();
        self.subir_version();
        true
    }

    /// **El enlace cuyo icono esta bajo el punto** `(x, y)` del mundo, con
    /// `zoom` pixeles por unidad (el icono mide lo mismo en pantalla a
    /// cualquier aumento). `None` si no hay ninguno.
    pub fn enlace_en(&self, x: f32, y: f32, zoom: f32) -> Option<String> {
        let st = self.tomar();
        pixpin_motor2d::zona::enlace_bajo_el_puntero(&st.escena.elementos, Punto2::nuevo(x, y), zoom)
            .map(str::to_string)
    }

    /// Los iconos de enlace que caen en la caja `(x0, y0, x1, y1)` del mundo,
    /// para pintarlos encima.
    pub fn enlaces_en_la_vista(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<EnlaceDelLienzo> {
        let st = self.tomar();
        st.escena
            .elementos
            .iter()
            .filter_map(|e| {
                let c = pixpin_motor2d::zona::icono_de_enlace(e)?;
                let dentro = c.x >= x0 - 40.0 && c.x <= x1 + 40.0 && c.y >= y0 - 40.0 && c.y <= y1 + 40.0;
                dentro.then(|| EnlaceDelLienzo {
                    dibujo: e.enlace.clone().unwrap_or_default(),
                    x: c.x,
                    y: c.y,
                })
            })
            .collect()
    }

    /// **El iman** de Ajustes → Lienzo (`[enganche]`): a que se pega el
    /// cursor al dibujar y a cuantos pixeles de pantalla agarra.
    pub fn poner_iman(&self, activo: bool, esquinas: bool, medios: bool, centros: bool, radio_px: f32) {
        let mut st = self.tomar();
        st.gesto.enganche = pixpin_motor2d::enganche::Ajustes {
            activo,
            esquinas,
            medios,
            centros,
            radio_px: if radio_px.is_finite() { radio_px.clamp(1.0, 200.0) } else { 14.0 },
        };
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_zona_va_al_chat_como_sublienzo_y_su_enlace_lleva_a_el() {
        let d = std::env::temp_dir().join(format!("pixpin-nucleo-zona-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let n = Nucleo::abrir(d.display().to_string(), "iPad".into()).unwrap();
        let chat = n.crear_chat("Planta".into()).unwrap().id;
        let lienzo = n.crear_lienzo(chat.clone(), Some("Planta baja".into())).unwrap();
        let dibujo = lienzo.referencia.clone().unwrap();
        let png = vec![0x89, b'P', b'N', b'G', 1, 2, 3];
        let z = n
            .zona_al_chat(chat.clone(), dibujo.clone(), None, "Planta baja".into(), png, 10, 8)
            .unwrap();
        assert!(z.dibujo.starts_with("foto-"));
        assert_eq!(z.mensaje.clase, "IMAGEN");
        assert_eq!(z.mensaje.referencia.as_deref(), Some(z.dibujo.as_str()));
        assert_eq!(n.dibujo_del_enlace(chat.clone(), z.dibujo.clone()).as_deref(), Some(z.dibujo.as_str()));
        assert_eq!(n.dibujo_del_enlace(chat.clone(), "nada".into()), None);
        assert_eq!(n.mensaje_del_dibujo(chat.clone(), z.dibujo.clone()).map(|m| m.id), Some(z.mensaje.id.clone()));
        let p = leer_proyecto(&almacen::carpeta(n.ruta(), &chat));
        let i = p.hojas.iter().position(|h| h.dibujo.as_deref() == Some(dibujo.as_str())).unwrap();
        assert_eq!(p.hojas[i + 1].nombre, "↳ Zona de Planta baja");
        // La marca en el origen y el enlace bajo su icono.
        let e = n.editar_lienzo(chat, dibujo).unwrap();
        assert!(e.marcar_zona(0.0, 0.0, 100.0, 50.0, z.dibujo.clone()));
        assert_eq!(e.enlace_en(100.0, 0.0, 1.0).as_deref(), Some(z.dibujo.as_str()));
        assert_eq!(e.enlace_en(300.0, 300.0, 1.0), None);
        assert_eq!(e.enlaces_en_la_vista(0.0, 0.0, 200.0, 200.0).len(), 1);
        assert!(e.deshacer());
        assert_eq!(e.enlace_en(100.0, 0.0, 1.0), None);
    }
}
