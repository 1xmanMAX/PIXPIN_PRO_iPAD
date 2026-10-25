//! **Google Drive** (API v3): las variables del servicio y su [`Almacen`].
//!
//! - Alcances: `drive.readonly` (buscar y leer todo lo del usuario) y
//!   `drive.file` (subir: solo toca lo que sube PixPin). Nada de `drive`
//!   entero: PixPin no puede borrar ni cambiar lo que ya hay.
//! - Buscar: `files.list` con `name contains` / `fullText contains` (sin
//!   orden, que Drive no ordena una busqueda de texto); sin texto, lo
//!   ultimo modificado.
//! - Leer: un Doc o una presentacion se exportan a texto plano y una hoja
//!   a CSV; lo demas se baja y el texto lo saca el lector de la app (PDF,
//!   Word...), con el OCR si hace falta.
//! - Bajar: los de Google salen en su formato de Office (o PNG un dibujo).
//! - Subir: hasta 5 MB en una peticion (`multipart`); mas, con la subida
//!   reanudable (dos peticiones).

use serde_json::{Value, json};

use super::url::{escapar, formulario};
use super::{Almacen, Archivo, Conexion, Contenido, LIMITE_ARCHIVO, Servicio};

pub const GOOGLE_DRIVE: Servicio = Servicio {
    id: "google_drive",
    nombre: "Google Drive",
    autorizar: "https://accounts.google.com/o/oauth2/v2/auth",
    token: "https://oauth2.googleapis.com/token",
    revocar: "https://oauth2.googleapis.com/revoke",
    api: "https://www.googleapis.com/drive/v3",
    subida: "https://www.googleapis.com/upload/drive/v3",
    alcances: &[
        "https://www.googleapis.com/auth/drive.readonly",
        "https://www.googleapis.com/auth/drive.file",
    ],
    // `offline` + `consent`: que de token de refresco (si no, habria que
    // volver al navegador cada hora).
    extra_autorizar: &[("access_type", "offline"), ("prompt", "consent")],
    consola: "console.cloud.google.com",
};

const CAMPOS: &str = "id,name,mimeType,size,modifiedTime,webViewLink";
const CARPETA: &str = "application/vnd.google-apps.folder";
/// Lo que Drive deja exportar de una vez.
const LIMITE_EXPORTAR: usize = 20 * 1024 * 1024;
const LIMITE_JSON: usize = 4 * 1024 * 1024;
/// Hasta aqui, la subida en una sola peticion.
const MULTIPART_MAX: usize = 5 * 1024 * 1024;

pub struct Drive<'a> {
    pub cx: &'a Conexion,
}

/// El tipo de un archivo dicho para el modelo.
pub fn tipo_leible(mime: &str) -> &'static str {
    match mime {
        "application/vnd.google-apps.document" => "Documento de Google",
        "application/vnd.google-apps.spreadsheet" => "Hoja de calculo de Google",
        "application/vnd.google-apps.presentation" => "Presentacion de Google",
        "application/vnd.google-apps.drawing" => "Dibujo de Google",
        CARPETA => "carpeta",
        "application/pdf" => "PDF",
        m if m.contains("wordprocessingml") || m == "application/msword" => "Word",
        m if m.contains("spreadsheetml") || m == "application/vnd.ms-excel" => "Excel",
        m if m.contains("presentationml") || m == "application/vnd.ms-powerpoint" => "PowerPoint",
        m if m.starts_with("image/") => "imagen",
        m if m.starts_with("audio/") => "audio",
        m if m.starts_with("video/") => "video",
        m if m.starts_with("text/") => "texto",
        m if m.starts_with("application/vnd.google-apps.") => "archivo de Google",
        _ => "archivo",
    }
}

/// Como se exporta un archivo de Google para LEERLO: (MIME pedido).
fn exportar_para_leer(mime: &str) -> Option<&'static str> {
    match mime {
        "application/vnd.google-apps.document" | "application/vnd.google-apps.presentation" => {
            Some("text/plain")
        }
        "application/vnd.google-apps.spreadsheet" => Some("text/csv"),
        _ => None,
    }
}

/// Como se exporta un archivo de Google para BAJARLO: (MIME, extension).
fn exportar_para_bajar(mime: &str) -> Option<(&'static str, &'static str)> {
    match mime {
        "application/vnd.google-apps.document" => Some((
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "docx",
        )),
        "application/vnd.google-apps.spreadsheet" => Some((
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "xlsx",
        )),
        "application/vnd.google-apps.presentation" => Some((
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "pptx",
        )),
        "application/vnd.google-apps.drawing" => Some(("image/png", "png")),
        _ => None,
    }
}

/// Si lo bajado ya es texto (no hace falta el lector).
fn es_texto(mime: &str) -> bool {
    mime.starts_with("text/")
        || matches!(
            mime,
            "application/json" | "application/xml" | "application/x-yaml"
        )
}

/// El MIME por la extension, para subir.
pub fn tipo_de_nombre(nombre: &str) -> &'static str {
    let ext = nombre
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "csv" => "text/csv",
        "json" => "application/json",
        "html" | "htm" => "text/html",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "zip" => "application/zip",
        "dwg" => "image/vnd.dwg",
        "dxf" => "image/vnd.dxf",
        _ => "application/octet-stream",
    }
}

/// Un texto dentro de una consulta de Drive (`'...'`).
fn literal(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

/// **La consulta `q` de Drive** para un texto y un tipo.
pub fn consulta(texto: &str, tipo: &str) -> String {
    let mut partes = vec!["trashed = false".to_string()];
    let t = texto.trim();
    if !t.is_empty() {
        let l = literal(t);
        partes.push(format!("(name contains '{l}' or fullText contains '{l}')"));
    }
    let mime = match tipo {
        "documento" => Some(
            "mimeType = 'application/vnd.google-apps.document' or mimeType = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'",
        ),
        "hoja" => Some(
            "mimeType = 'application/vnd.google-apps.spreadsheet' or mimeType = 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'",
        ),
        "presentacion" => Some(
            "mimeType = 'application/vnd.google-apps.presentation' or mimeType = 'application/vnd.openxmlformats-officedocument.presentationml.presentation'",
        ),
        "pdf" => Some("mimeType = 'application/pdf'"),
        "imagen" => Some("mimeType contains 'image/'"),
        "carpeta" => Some("mimeType = 'application/vnd.google-apps.folder'"),
        _ => None,
    };
    if let Some(m) = mime {
        partes.push(format!("({m})"));
    }
    partes.join(" and ")
}

fn archivo_de(v: &Value) -> Archivo {
    let s = |k: &str| {
        v.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let tipo = s("mimeType");
    Archivo {
        id: s("id"),
        nombre: s("name"),
        carpeta: tipo == CARPETA,
        tamano: v.get("size").and_then(|x| {
            x.as_str()
                .and_then(|t| t.parse().ok())
                .or_else(|| x.as_u64())
        }),
        modificado: s("modifiedTime"),
        enlace: s("webViewLink"),
        tipo,
    }
}

fn json_de(bytes: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(bytes)
        .map_err(|_| "Google Drive contesto algo que no se entiende".to_string())
}

/// El nombre con la extension de su formato de Office (`Informe` → `Informe.docx`).
fn con_extension(nombre: &str, ext: &str) -> String {
    if nombre.to_ascii_lowercase().ends_with(&format!(".{ext}")) {
        nombre.to_string()
    } else {
        format!("{nombre}.{ext}")
    }
}

impl Drive<'_> {
    fn get(&self, ruta_y_consulta: &str, limite: usize) -> Result<Vec<u8>, String> {
        let url = format!("{}{ruta_y_consulta}", self.cx.extremos.api);
        Ok(self.cx.pedir("GET", &url, &[], &[], limite)?.cuerpo)
    }
}

impl Almacen for Drive<'_> {
    fn buscar(&self, texto: &str, tipo: &str, maximo: usize) -> Result<Vec<Archivo>, String> {
        let q = consulta(texto, tipo);
        let n = maximo.clamp(1, 50).to_string();
        let campos = format!("files({CAMPOS})");
        let mut pares = vec![
            ("q", q.as_str()),
            ("pageSize", n.as_str()),
            ("fields", campos.as_str()),
            ("supportsAllDrives", "true"),
            ("includeItemsFromAllDrives", "true"),
        ];
        // Drive no ordena una busqueda con `fullText`.
        if texto.trim().is_empty() {
            pares.push(("orderBy", "modifiedTime desc"));
        }
        let v = json_de(&self.get(&format!("/files?{}", formulario(&pares)), LIMITE_JSON)?)?;
        Ok(v.get("files")
            .and_then(Value::as_array)
            .map(|l| l.iter().map(archivo_de).collect())
            .unwrap_or_default())
    }

    fn archivo(&self, id: &str) -> Result<Archivo, String> {
        let v = json_de(&self.get(
            &format!(
                "/files/{}?{}",
                escapar(id),
                formulario(&[("fields", CAMPOS), ("supportsAllDrives", "true")])
            ),
            LIMITE_JSON,
        )?)?;
        Ok(archivo_de(&v))
    }

    fn leer(&self, a: &Archivo) -> Result<Contenido, String> {
        if a.carpeta {
            return Err(format!(
                "«{}» es una carpeta; busca dentro con su nombre",
                a.nombre
            ));
        }
        if let Some(m) = exportar_para_leer(&a.tipo) {
            let b = self.get(
                &format!(
                    "/files/{}/export?{}",
                    escapar(&a.id),
                    formulario(&[("mimeType", m)])
                ),
                LIMITE_EXPORTAR,
            )?;
            let t = String::from_utf8_lossy(&b);
            return Ok(Contenido::Texto(
                t.trim_start_matches('\u{feff}').to_string(),
            ));
        }
        if a.tipo.starts_with("application/vnd.google-apps.") {
            return Err(format!(
                "«{}» es un {} y no tiene texto que leer; puedes bajarlo al chat",
                a.nombre,
                tipo_leible(&a.tipo)
            ));
        }
        let (_, bytes) = self.bajar(a)?;
        if es_texto(&a.tipo) {
            return Ok(Contenido::Texto(
                String::from_utf8_lossy(&bytes).into_owned(),
            ));
        }
        Ok(Contenido::Fichero {
            nombre: a.nombre.clone(),
            bytes,
        })
    }

    fn bajar(&self, a: &Archivo) -> Result<(String, Vec<u8>), String> {
        if a.carpeta {
            return Err(format!(
                "«{}» es una carpeta: baja los archivos de dentro de uno en uno",
                a.nombre
            ));
        }
        if let Some((m, ext)) = exportar_para_bajar(&a.tipo) {
            let b = self.get(
                &format!(
                    "/files/{}/export?{}",
                    escapar(&a.id),
                    formulario(&[("mimeType", m)])
                ),
                LIMITE_EXPORTAR,
            )?;
            return Ok((con_extension(&a.nombre, ext), b));
        }
        if a.tipo.starts_with("application/vnd.google-apps.") {
            return Err(format!(
                "«{}» ({}) no se puede bajar como archivo",
                a.nombre,
                tipo_leible(&a.tipo)
            ));
        }
        if a.tamano.is_some_and(|t| t as usize > LIMITE_ARCHIVO) {
            return Err(format!(
                "«{}» pesa mas de {} MB; abrelo en Drive: {}",
                a.nombre,
                LIMITE_ARCHIVO / (1024 * 1024),
                a.enlace
            ));
        }
        let b = self.get(
            &format!(
                "/files/{}?{}",
                escapar(&a.id),
                formulario(&[("alt", "media"), ("supportsAllDrives", "true")])
            ),
            LIMITE_ARCHIVO,
        )?;
        Ok((a.nombre.clone(), b))
    }

    fn subir(&self, nombre: &str, bytes: &[u8], carpeta: Option<&str>) -> Result<Archivo, String> {
        if bytes.len() > LIMITE_ARCHIVO {
            return Err(format!(
                "«{nombre}» pesa mas de {} MB",
                LIMITE_ARCHIVO / (1024 * 1024)
            ));
        }
        let tipo = tipo_de_nombre(nombre);
        let mut meta = json!({"name": nombre});
        if let Some(c) = carpeta.filter(|c| !c.trim().is_empty()) {
            meta["parents"] = json!([c.trim()]);
        }
        let consulta_comun = formulario(&[("fields", CAMPOS), ("supportsAllDrives", "true")]);
        let base = &self.cx.extremos.subida;
        let r = if bytes.len() <= MULTIPART_MAX {
            let frontera = format!(
                "pixpin{}",
                super::oauth::base64_url(&super::oauth::aleatorio(12)).replace(['-', '_'], "x")
            );
            let mut cuerpo = Vec::with_capacity(bytes.len() + 512);
            cuerpo.extend_from_slice(
                format!("--{frontera}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{meta}\r\n--{frontera}\r\nContent-Type: {tipo}\r\n\r\n").as_bytes(),
            );
            cuerpo.extend_from_slice(bytes);
            cuerpo.extend_from_slice(format!("\r\n--{frontera}--\r\n").as_bytes());
            self.cx.pedir(
                "POST",
                &format!("{base}/files?uploadType=multipart&{consulta_comun}"),
                &[(
                    "Content-Type",
                    format!("multipart/related; boundary={frontera}"),
                )],
                &cuerpo,
                LIMITE_JSON,
            )?
        } else {
            let inicio = self.cx.pedir(
                "POST",
                &format!("{base}/files?uploadType=resumable&{consulta_comun}"),
                &[
                    ("Content-Type", "application/json; charset=UTF-8".into()),
                    ("X-Upload-Content-Type", tipo.into()),
                    ("X-Upload-Content-Length", bytes.len().to_string()),
                ],
                meta.to_string().as_bytes(),
                LIMITE_JSON,
            )?;
            let destino = inicio
                .ubicacion
                .filter(|u| !u.is_empty())
                .ok_or("Google Drive no dio donde subir")?;
            self.cx.pedir(
                "PUT",
                &destino,
                &[("Content-Type", tipo.into())],
                bytes,
                LIMITE_JSON,
            )?
        };
        Ok(archivo_de(&json_de(&r.cuerpo)?))
    }

    fn cuenta(&self) -> Result<String, String> {
        let v = json_de(&self.get("/about?fields=user(emailAddress)", LIMITE_JSON)?)?;
        Ok(v.pointer("/user/emailAddress")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_consulta_escapa_las_comillas_y_filtra_el_tipo() {
        assert_eq!(consulta("", "cualquiera"), "trashed = false");
        let q = consulta("informe d'obra", "pdf");
        assert_eq!(
            q,
            "trashed = false and (name contains 'informe d\\'obra' or fullText contains 'informe d\\'obra') and (mimeType = 'application/pdf')"
        );
        assert!(consulta("x", "carpeta").contains(CARPETA));
    }

    #[test]
    fn los_de_google_se_leen_y_bajan_en_su_formato() {
        assert_eq!(
            exportar_para_leer("application/vnd.google-apps.spreadsheet"),
            Some("text/csv")
        );
        assert_eq!(
            exportar_para_bajar("application/vnd.google-apps.document")
                .unwrap()
                .1,
            "docx"
        );
        assert_eq!(exportar_para_leer("application/pdf"), None);
        assert_eq!(con_extension("Informe", "docx"), "Informe.docx");
        assert_eq!(con_extension("Informe.DOCX", "docx"), "Informe.DOCX");
        assert_eq!(tipo_de_nombre("Plano.PDF"), "application/pdf");
        assert_eq!(tipo_de_nombre("sin"), "application/octet-stream");
    }
}
