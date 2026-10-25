//! **Varias paginas del PDF de un proyecto, juntas en un lienzo nuevo**
//! (`fusionar_paginas.rs` de Windows, `FusionDePaginas.kt` y
//! `FusionarPaginas.kt` del movil): «la 4, la 5 y la 6 en un mismo lienzo,
//! separadas a una distancia razonable», como dos planos en la mesa.
//!
//! Las cuentas (cuantos pixeles por pagina, en fila o en columna, donde va
//! cada una) son las de Windows. Pintar las paginas lo hace Swift (PDFKit):
//! el nucleo del iPad no pinta PDF. Swift pide los anchos
//! ([`anchos_para_fusionar`]), pinta cada pagina a su ancho en JPEG y se la
//! pasa a [`Nucleo::fusionar_paginas`], que escribe el `.excalidraw` (un
//! marco con su rotulo y la foto de la pagina por cada una, las dos con
//! candado), su mensaje `DIBUJO` y su hoja en `proyecto.json`.

use pixpin_proyecto::{Hoja, Proyecto, almacen, vista};

use crate::{ErrorNucleo, MensajeChat, Nucleo};

/// Mas de esto no se fusiona: son mapas de bits, y no cabrian.
pub const TOPE_DE_PAGINAS: usize = 12;
/// Los pixeles de todas las paginas juntas.
pub const PIXELES_EN_TOTAL: f64 = 24_000_000.0;
pub const ANCHO_MAXIMO: f64 = 2400.0;
pub const ANCHO_MINIMO: f64 = 700.0;
/// La separacion entre paginas, en tanto por uno del lado mas largo.
pub const SEPARACION: f64 = 0.14;

/// Lo que mide una pagina del PDF, en puntos.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct MedidaDePagina {
    pub ancho: f64,
    pub alto: f64,
}

/// Una pagina ya pintada por Swift: cual (desde 0), su JPEG y sus pixeles.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaParaFusionar {
    pub pagina: u32,
    pub jpg: Vec<u8>,
    pub ancho: u32,
    pub alto: u32,
}

/// En fila (verticales) o en columna (apaisadas), por la pagina media.
pub fn en_fila(medidas: &[(f64, f64)]) -> bool {
    if medidas.is_empty() {
        return true;
    }
    let mediana = |f: fn(&(f64, f64)) -> f64| {
        let mut v: Vec<f64> = medidas.iter().map(f).collect();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    mediana(|m| m.1) >= mediana(|m| m.0)
}

/// Donde cae cada pagina: en fila alineadas por arriba, en columna por la
/// izquierda, con la separacion de la mayor.
pub fn sitios(tamanos: &[(f64, f64)], fila: bool) -> Vec<(f64, f64, f64, f64)> {
    let hueco = tamanos.iter().map(|t| t.0.max(t.1)).fold(0.0, f64::max) * SEPARACION;
    let (mut x, mut y) = (0.0, 0.0);
    tamanos
        .iter()
        .map(|&(an, al)| {
            let s = (x, y, an, al);
            if fila {
                x += an + hueco;
            } else {
                y += al + hueco;
            }
            s
        })
        .collect()
}

/// **A cuantos pixeles de ancho se pinta cada pagina**, repartiendo
/// [`PIXELES_EN_TOTAL`] en proporcion a su papel, con los topes de ancho en
/// la escala (no en cada pagina).
#[uniffi::export]
pub fn anchos_para_fusionar(medidas: Vec<MedidaDePagina>) -> Vec<u32> {
    if medidas.is_empty() {
        return Vec::new();
    }
    let area: f64 = medidas.iter().map(|m| m.ancho.max(1.0) * m.alto.max(1.0)).sum();
    let mut k = if area > 0.0 { (PIXELES_EN_TOTAL / area).sqrt() } else { 1.0 };
    let mas_ancha = medidas.iter().map(|m| m.ancho.max(1.0)).fold(0.0, f64::max);
    let mas_estrecha = medidas.iter().map(|m| m.ancho.max(1.0)).fold(f64::MAX, f64::min);
    k = k.min(ANCHO_MAXIMO / mas_ancha);
    k = k.max(ANCHO_MINIMO / mas_estrecha);
    medidas.iter().map(|m| ((m.ancho.max(1.0) * k) as u32).max(1)).collect()
}

/// Cuantas paginas se pueden fusionar como mucho.
#[uniffi::export]
pub fn tope_de_paginas_para_fusionar() -> u32 {
    TOPE_DE_PAGINAS as u32
}

/// «3 a 5» si van seguidas; «3, 5 y 8» si no (desde 0 → desde 1).
fn cuales(paginas: &[u32]) -> String {
    let n: Vec<u32> = paginas.iter().map(|p| p + 1).collect();
    match n.as_slice() {
        [] => String::new(),
        [uno] => uno.to_string(),
        _ if n.windows(2).all(|w| w[1] == w[0] + 1) => format!("{} a {}", n[0], n[n.len() - 1]),
        [antes @ .., ultimo] => format!(
            "{} y {ultimo}",
            antes.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// **El nombre de la hoja fusionada**: que paginas son.
#[uniffi::export]
pub fn nombre_de_paginas_fusionadas(paginas: Vec<u32>) -> String {
    match paginas.as_slice() {
        [] => "Páginas".into(),
        [una] => format!("Página {}", una + 1),
        _ => format!("Páginas {}", cuales(&paginas)),
    }
}

/// El rotulo del marco de una pagina: «3 a 5 · pág. 4».
fn rotulo(paginas: &[u32], pagina: u32) -> String {
    format!("{} · pág. {}", cuales(paginas), pagina + 1)
}

/// El `.excalidraw` de la fusion: por cada pagina, su marco con su rotulo y
/// la foto de la pagina dentro, las dos con candado.
fn lienzo(paginas: &[u32], hechas: &[(u32, String, f64, f64)], ahora: i64) -> String {
    let tamanos: Vec<(f64, f64)> = hechas.iter().map(|h| (h.2, h.3)).collect();
    let sitios = sitios(&tamanos, en_fila(&tamanos));
    let mut elementos = Vec::new();
    let mut ficheros = serde_json::Map::new();
    for (i, ((pagina, foto, _, _), s)) in hechas.iter().zip(&sitios).enumerate() {
        let comun = |id: String, tipo: &str, semilla: i64| {
            serde_json::json!({
                "id": id, "type": tipo,
                "x": s.0, "y": s.1, "width": s.2, "height": s.3,
                "angle": 0, "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
                "fillStyle": "solid", "strokeWidth": 1, "strokeStyle": "solid",
                "roughness": 0, "opacity": 100, "groupIds": [], "frameId": null,
                "seed": semilla, "version": 1, "versionNonce": 1, "isDeleted": false,
                "boundElements": null, "locked": true
            })
        };
        let mut marco = comun(format!("marco-fus-{ahora}-{i}"), "frame", 1);
        marco["name"] = serde_json::Value::String(rotulo(paginas, *pagina));
        let mut imagen = comun(format!("pag-fus-{ahora}-{i}"), "image", 2);
        imagen["fileId"] = serde_json::Value::String(foto.clone());
        imagen["status"] = "saved".into();
        imagen["scale"] = serde_json::json!([1, 1]);
        imagen["strokeColor"] = "transparent".into();
        elementos.push(marco);
        elementos.push(imagen);
        ficheros.insert(
            foto.clone(),
            serde_json::json!({"id": foto, "mimeType": "image/jpeg", "path": format!("imagenes/{foto}"), "created": ahora}),
        );
    }
    serde_json::json!({
        "type": "excalidraw",
        "version": 2,
        "source": "pixpin-max",
        "elements": elementos,
        "appState": {"viewBackgroundColor": "#ffffff"},
        "files": ficheros
    })
    .to_string()
}

#[uniffi::export]
impl Nucleo {
    /// El PDF del proyecto `chat` (`documento.pdf`, o el de origen si esta
    /// aqui), si lo hay: el de las paginas que se pueden fusionar.
    pub fn pdf_del_proyecto(&self, chat: String) -> Option<String> {
        crate::chats::validar_chat(&chat).ok()?;
        vista::documento_del_proyecto(self.ruta(), &chat)
            .filter(|p| p.is_file())
            .map(|p| p.display().to_string())
    }

    /// **Junta las paginas ya pintadas en un lienzo nuevo** del chat: sus
    /// JPEG a `imagenes/`, el `.excalidraw` con un marco por pagina, el
    /// mensaje `DIBUJO` («Páginas 4 a 6») y su hoja en `proyecto.json` (con
    /// el mismo codigo unico, para que Windows y el movil no la ensenen dos
    /// veces). Devuelve el mensaje, para abrirlo. Bloqueante.
    pub fn fusionar_paginas(
        &self,
        chat: String,
        paginas: Vec<PaginaParaFusionar>,
    ) -> Result<MensajeChat, ErrorNucleo> {
        crate::chats::validar_chat(&chat)?;
        let mut paginas: Vec<PaginaParaFusionar> = paginas.into_iter().filter(|p| !p.jpg.is_empty()).collect();
        paginas.sort_by_key(|p| p.pagina);
        paginas.dedup_by_key(|p| p.pagina);
        if paginas.is_empty() || paginas.len() > TOPE_DE_PAGINAS {
            return Err(ErrorNucleo::Argumento {
                detalle: format!("se fusionan de 1 a {TOPE_DE_PAGINAS} paginas"),
            });
        }
        let raiz = self.ruta();
        let ahora = crate::ahora_ms();
        let carpeta = almacen::carpeta(raiz, &chat);
        std::fs::create_dir_all(carpeta.join("imagenes"))?;
        let numeros: Vec<u32> = paginas.iter().map(|p| p.pagina).collect();
        let mut hechas = Vec::new();
        for (i, p) in paginas.iter().enumerate() {
            let foto = format!("fus{ahora}x{i}");
            std::fs::write(carpeta.join("imagenes").join(&foto), &p.jpg)?;
            hechas.push((p.pagina, foto, p.ancho.max(1) as f64, p.alto.max(1) as f64));
        }
        let nombre = nombre_de_paginas_fusionadas(numeros.clone());
        let m = self.crear_lienzo_con(chat.clone(), Some(nombre.clone()), |_| {})?;
        let dibujo = m.referencia.clone().unwrap_or_default();
        let texto = lienzo(&numeros, &hechas, ahora);
        let ruta = almacen::lienzo(raiz, &chat, &dibujo);
        let temporal = ruta.with_extension("excalidraw.tmp");
        std::fs::write(&temporal, &texto)?;
        std::fs::rename(&temporal, &ruta)?;
        // Su hoja en el proyecto, con el codigo de su mensaje.
        let ruta_p = carpeta.join("proyecto.json");
        let mut p: Proyecto = std::fs::read_to_string(&ruta_p)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if p.id.is_empty() {
            p.id = chat.clone();
        }
        p.hojas.push(Hoja {
            id: format!("hoja-fus-{ahora}"),
            nombre,
            dibujo: Some(dibujo),
            uid: Some(m.codigo_unico()),
            ..Default::default()
        });
        p.tocado = ahora;
        let tmp = carpeta.join("proyecto.json.fus.tmp");
        std::fs::write(&tmp, serde_json::to_string(&p).map_err(|e| ErrorNucleo::Argumento { detalle: e.to_string() })?)?;
        std::fs::rename(&tmp, &ruta_p)?;
        Ok(MensajeChat::from(m))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_cuentas_de_windows() {
        assert_eq!(nombre_de_paginas_fusionadas(vec![3, 4, 5]), "Páginas 4 a 6");
        assert_eq!(nombre_de_paginas_fusionadas(vec![2, 4, 7]), "Páginas 3, 5 y 8");
        assert_eq!(nombre_de_paginas_fusionadas(vec![0]), "Página 1");
        assert_eq!(rotulo(&[3, 4, 5], 4), "4 a 6 · pág. 5");
        let a4 = MedidaDePagina { ancho: 595.0, alto: 842.0 };
        let v = anchos_para_fusionar(vec![a4, a4]);
        assert_eq!(v.len(), 2);
        assert!(v[0] <= 2400 && v[0] >= 700);
        assert!(en_fila(&[(595.0, 842.0), (595.0, 842.0)]));
        assert!(!en_fila(&[(842.0, 595.0), (842.0, 595.0)]));
        let s = sitios(&[(100.0, 200.0), (100.0, 200.0)], true);
        assert_eq!(s[1].0, 100.0 + 200.0 * SEPARACION);
    }

    #[test]
    fn el_lienzo_nuevo_entra_en_el_chat_y_en_el_proyecto() {
        let d = std::env::temp_dir().join(format!("pixpin-nucleo-fusion-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let n = Nucleo::abrir(d.display().to_string(), "iPad".into()).unwrap();
        let chat = n.crear_chat("Plano".into()).unwrap().id;
        assert_eq!(n.pdf_del_proyecto(chat.clone()), None);
        let pag = |i| PaginaParaFusionar { pagina: i, jpg: vec![0xff, 0xd8, 1], ancho: 700, alto: 990 };
        let m = n.fusionar_paginas(chat.clone(), vec![pag(5), pag(3), pag(4)]).unwrap();
        assert_eq!(m.clase, "DIBUJO");
        assert_eq!(m.nombre, "Páginas 4 a 6");
        let e = n.editar_lienzo(chat.clone(), m.referencia.clone().unwrap()).unwrap();
        assert_eq!(e.imagenes().len(), 3);
        let carpeta = almacen::carpeta(n.ruta(), &chat);
        let p: Proyecto = serde_json::from_str(&std::fs::read_to_string(carpeta.join("proyecto.json")).unwrap()).unwrap();
        assert_eq!(p.hojas.last().unwrap().dibujo, m.referencia);
        assert!(n.fusionar_paginas(chat, Vec::new()).is_err());
    }
}
