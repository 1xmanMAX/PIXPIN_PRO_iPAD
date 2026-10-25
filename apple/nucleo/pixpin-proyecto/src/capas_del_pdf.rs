//! **Donde va lo anotado en el lector de PDF**: puerto de `CapasDelPdf` de
//! `pdf/LectorPdfActivity.kt` del movil (v0.69-v0.70).
//!
//! En el movil cada hoja del lector tiene su dibujo, y cual es depende de si
//! el PDF es de un proyecto (`CapasDelPdf.idDe`):
//!
//! - **Si es el documento de un proyecto** (su `pdfOrigen` o su `pdfLimpio`),
//!   el dibujo de la hoja `i` es el de **la hoja del proyecto con esa
//!   `pagina`**; si aun no tenia, se le pone `pdf-<huella>-p<i>` al escribir
//!   (`Proyectos.conDibujo`). Lo anotado en el lector es lo mismo que se
//!   edita desde proyectos, y **viaja al sincronizar**: el `alcance` de
//!   `sincro/Disco.kt` manda el dibujo de cada hoja.
//! - Si no, es un dibujo suelto del propio PDF, que no es de ningun chat ni
//!   de ningun proyecto y **no viaja** (en el movil tampoco).
//!
//! El PC guardaba las dos cosas como la segunda, en la carpeta hermana
//! `<pdf>.pixpin-anotado/hoja-<n>.excalidraw` (ver `lector_tinta` de la
//! app): lo anotado sobre el PDF de un proyecto no llegaba nunca al movil
//! (queja del usuario del 27-sep). Aqui se decide como alli, y lo que ya
//! estaba en la carpeta hermana se **adopta** en la hoja del proyecto, como
//! hace `CapasDelPdf.pasarAProyecto` con los dibujos sueltos.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::almacen::{self, Indice};
use crate::{Proyecto, vista};

/// El PDF que se lee es de un proyecto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelProyecto {
    /// La ficha (la carpeta) del proyecto.
    pub ficha: String,
    /// Su copia limpia, si esta en este equipo: es la que se ensena, porque
    /// el documento de un proyecto que viene del movil lleva lo anotado
    /// cocido dentro y aqui se pinta ademas encima («Se ensena la copia
    /// limpia», `LectorPdfActivity`).
    pub limpio: Option<PathBuf>,
}

/// La huella con que el movil nombra los dibujos de un PDF:
/// `ruta.hashCode().toUInt().toString(16)` de Kotlin (por unidad UTF-16).
pub fn huella(ruta: &str) -> String {
    let mut h: i32 = 0;
    for u in ruta.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(u as i32);
    }
    format!("{:x}", h as u32)
}

/// El nombre del dibujo suelto de la hoja `i` (`pdf-<huella>-p<i>`).
pub fn suelto(huella: &str, i: u32) -> String {
    format!("pdf-{huella}-p{i}")
}

fn misma_ruta(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

fn leer_proyecto(raiz: &Path, ficha: &str) -> Option<Proyecto> {
    let t = std::fs::read_to_string(almacen::carpeta(raiz, ficha).join("proyecto.json")).ok()?;
    serde_json::from_str(&t).ok()
}

fn guardar_proyecto(raiz: &Path, ficha: &str, p: &Proyecto) -> io::Result<()> {
    let texto = serde_json::to_string(p).map_err(io::Error::other)?;
    pixpin_sincro::disco::escribir_atomico(
        &almacen::carpeta(raiz, ficha).join("proyecto.json"),
        texto.as_bytes(),
    )
}

/// **De que proyecto es este PDF** (`CapasDelPdf.proyecto`: el que lo tiene
/// de `pdfOrigen` o de `pdfLimpio`). `None` si de ninguno.
pub fn de_este_pdf(raiz: &Path, pdf: &Path) -> Option<DelProyecto> {
    for f in Indice::leer(raiz).proyectos {
        let Some(p) = leer_proyecto(raiz, &f.id) else {
            continue;
        };
        let real = |r: &Option<String>| {
            r.as_deref()
                .and_then(|r| vista::ruta_real(raiz, &f.id, r))
                .filter(|r| r.is_file())
        };
        let (origen, limpio) = (real(&p.pdf_origen), real(&p.pdf_limpio));
        if [&origen, &limpio]
            .into_iter()
            .flatten()
            .any(|r| misma_ruta(r, pdf))
        {
            return Some(DelProyecto {
                ficha: f.id,
                limpio,
            });
        }
    }
    None
}

/// **El dibujo de la hoja `i`** (`CapasDelPdf.idDe`): el de la hoja del
/// proyecto con esa pagina; si no tenia, `pdf-<huella>-p<i>`, que con
/// `escribir` se le apunta en el `proyecto.json` (como `conDibujo`, subiendo
/// su `tocado`: es lo que hace que el proyecto viaje con el). `None` si
/// ninguna hoja del proyecto es esa pagina: entonces lo anotado es suelto.
pub fn dibujo_de_la_pagina(
    raiz: &Path,
    ficha: &str,
    i: u32,
    huella: &str,
    escribir: bool,
    ahora: i64,
) -> Option<String> {
    let mut p = leer_proyecto(raiz, ficha)?;
    let hoja = p.hojas.iter_mut().find(|h| h.pagina == Some(i))?;
    if let Some(d) = hoja.dibujo.clone().filter(|d| !d.is_empty()) {
        return Some(d);
    }
    let d = suelto(huella, i);
    if escribir {
        hoja.dibujo = Some(d.clone());
        p.tocado = p.tocado.max(ahora);
        // Sin apuntarlo, lo que se guarde no seria de la hoja: mejor nada.
        guardar_proyecto(raiz, ficha, &p).ok()?;
    }
    Some(d)
}

/// El fichero del dibujo `d` de ese proyecto.
pub fn fichero(raiz: &Path, ficha: &str, dibujo: &str) -> PathBuf {
    almacen::lienzo(raiz, ficha, dibujo)
}

/// Si un lienzo tiene algo a la vista (algun elemento sin borrar).
fn con_algo(v: &Value) -> bool {
    v.get("elements")
        .and_then(Value::as_array)
        .is_some_and(|l| {
            l.iter()
                .any(|e| e.get("isDeleted") != Some(&Value::Bool(true)))
        })
}

/// **Adopta lo anotado antes en la carpeta hermana** (`suelta`, la
/// `hoja-<n>.excalidraw` de la pagina `i`) en la hoja del proyecto, como
/// `pasarAProyecto` con los dibujos sueltos: si la hoja no tenia dibujo, el
/// suelto pasa a serlo; si ya tenia, se le anade lo que no estuviera (por
/// id de elemento, y sus fotos). La suelta se renombra a `.adoptada`: no se
/// borra nada del usuario, pero tampoco se adopta dos veces.
///
/// Devuelve si adopto algo. `Ok(false)` si no hay suelta, si esta vacia o
/// ilegible, o si ninguna hoja del proyecto es esa pagina.
pub fn adoptar(
    raiz: &Path,
    ficha: &str,
    i: u32,
    huella: &str,
    suelta: &Path,
    ahora: i64,
) -> io::Result<bool> {
    let Ok(texto) = std::fs::read_to_string(suelta) else {
        return Ok(false);
    };
    let Ok(vieja) = serde_json::from_str::<Value>(&texto) else {
        return Ok(false);
    };
    if !con_algo(&vieja) {
        return Ok(false);
    }
    let Some(d) = dibujo_de_la_pagina(raiz, ficha, i, huella, true, ahora) else {
        return Ok(false);
    };
    let destino = fichero(raiz, ficha, &d);
    let actual = std::fs::read_to_string(&destino)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let salida = match actual {
        None => texto,
        Some(mut a) => {
            juntar(&mut a, &vieja);
            serde_json::to_string_pretty(&a).map_err(io::Error::other)?
        }
    };
    pixpin_sincro::disco::escribir_atomico(&destino, salida.as_bytes())?;
    let mut hecha = suelta.as_os_str().to_owned();
    hecha.push(".adoptada");
    std::fs::rename(suelta, PathBuf::from(hecha))?;
    Ok(true)
}

/// **«Al proyecto»** (`CapasDelPdf.pasarAProyecto` + `ProyectosRepository
/// .deEstePdf`): el PDF que se lee pasa a ser un proyecto nuevo, una hoja
/// por pagina, y lo anotado en el lector (`sueltas(i)`, la capa suelta de la
/// hoja `i` si la hay) se queda en sus hojas. Solo cuando se pide: anotar no
/// crea ningun proyecto.
///
/// Como en el movil, el documento del proyecto (`pdfOrigen`) es **el mismo
/// fichero** que se estaba leyendo si esta en algun chat (en portatil, que es
/// lo que el movil sabe traducir: alli el lector de ese adjunto escribe
/// tambien en el proyecto), y la copia limpia una copia nueva
/// (`archivos/limpio-<t>.pdf`). Si el PDF no esta en ningun chat (se abrio
/// desde el Explorador) se copia tambien como `archivos/doc-<t>.pdf`. Su chat
/// empieza con el PDF como primer mensaje, ya unido.
///
/// Si el PDF ya es de un proyecto, devuelve ese sin crear otro.
pub fn al_proyecto(
    raiz: &Path,
    pdf: &Path,
    nombre: &str,
    paginas: u32,
    ahora: i64,
    sueltas: &dyn Fn(u32) -> PathBuf,
) -> io::Result<String> {
    if let Some(d) = de_este_pdf(raiz, pdf) {
        return Ok(d.ficha);
    }
    if paginas == 0 || !pdf.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no es un PDF con hojas",
        ));
    }
    let identidad = crate::identidad::Identidad::leer_o_crear(raiz, "PixPin Max")?;
    let aparato = identidad.yo.codigo();
    let mut indice = Indice::leer_si_esta(raiz)?;
    let ficha = almacen::Ficha::nueva(nombre, ahora, &aparato);
    let carpeta = almacen::carpeta(raiz, &ficha.id);
    std::fs::create_dir_all(carpeta.join("archivos"))?;
    // La copia limpia, antes de nada: es la base intacta del documento.
    let limpio = format!("archivos/limpio-{ahora}.pdf");
    std::fs::copy(pdf, carpeta.join(&limpio))?;
    let origen = match vista::portatil_de_ruta(raiz, pdf) {
        Some(p) => p,
        None => {
            let doc = format!("archivos/doc-{ahora}.pdf");
            std::fs::copy(pdf, carpeta.join(&doc))?;
            doc
        }
    };
    let hojas = (0..paginas)
        .map(|i| {
            let mut h = crate::Hoja {
                id: format!("h-{ahora}-{i}"),
                pagina: Some(i),
                ..Default::default()
            };
            h.uid = Some(h.codigo_unico());
            h
        })
        .collect();
    let p = Proyecto {
        id: ficha.id.clone(),
        nombre: nombre.to_string(),
        hojas,
        tocado: ahora,
        pdf_origen: Some(origen),
        pdf_limpio: Some(limpio.clone()),
        uid: ficha.uid.clone(),
        creado: ahora,
        aparato: Some(aparato.clone()),
        ..Default::default()
    };
    guardar_proyecto(raiz, &ficha.id, &p)?;
    let mut f = ficha.clone();
    f.hojas = paginas;
    indice.proyectos.push(f);
    indice.guardar(raiz)?;
    // «Todo pasa por el chat»: el PDF es su primer mensaje, ya unido.
    let archivo = if nombre.to_lowercase().ends_with(".pdf") {
        nombre.to_string()
    } else {
        format!("{nombre}.pdf")
    };
    let bytes = std::fs::metadata(carpeta.join(&limpio)).map_or(0, |m| m.len() as i64);
    let mut m = crate::cuaderno::Mensaje::adjunto(
        crate::cuaderno::Clase::Archivo,
        &archivo,
        &limpio,
        bytes,
        &crate::cuaderno::Sello {
            cuando: ahora,
            numero: 1,
            aparato,
            proyecto: ficha.id.clone(),
        },
    );
    m.resto.insert("unido".into(), Value::Bool(true));
    crate::cuaderno::anadir(&carpeta, &m)?;
    // Y lo anotado ya, a sus hojas.
    let huella = huella(&pdf.to_string_lossy());
    for i in 0..paginas {
        adoptar(raiz, &ficha.id, i, &huella, &sueltas(i), ahora)?;
    }
    Ok(ficha.id)
}

/// Anade a `a` los elementos y fotos de `b` que no tenga (por id).
fn juntar(a: &mut Value, b: &Value) {
    let Some(obj) = a.as_object_mut() else {
        return;
    };
    let ids: std::collections::HashSet<String> = obj
        .get("elements")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(|e| e.get("id")?.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let nuevos: Vec<Value> = b
        .get("elements")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter(|e| {
                    e.get("id")
                        .and_then(Value::as_str)
                        .is_some_and(|id| !ids.contains(id))
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    match obj.get_mut("elements") {
        Some(Value::Array(l)) => l.extend(nuevos),
        _ => {
            obj.insert("elements".into(), Value::Array(nuevos));
        }
    }
    if let Some(Value::Object(fb)) = b.get("files") {
        let fa = obj
            .entry("files")
            .or_insert_with(|| Value::Object(Default::default()));
        if !fa.is_object() {
            *fa = Value::Object(Default::default());
        }
        if let Value::Object(fa) = fa {
            for (k, v) in fb {
                fa.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::Hoja;
    use crate::almacen::Ficha;

    fn raiz(etiqueta: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!(
            "pixpin-capas-pdf-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    /// Un proyecto con su PDF (y copia limpia) y tres hojas-pagina: la 0 con
    /// dibujo, la 1 sin el; la 2 no existe (el proyecto tiene solo 0, 1 y 5).
    fn con_pdf(r: &Path) -> (Ficha, PathBuf, PathBuf) {
        let ficha = Ficha::nueva("Obra", 5, "PC01");
        let mut i = Indice::default();
        i.proyectos.push(ficha.clone());
        i.guardar(r).unwrap();
        let carpeta = almacen::carpeta(r, &ficha.id);
        std::fs::create_dir_all(carpeta.join("archivos")).unwrap();
        let doc = carpeta.join("archivos/doc-7.pdf");
        let limpio = carpeta.join("archivos/limpio-7.pdf");
        std::fs::write(&doc, b"%PDF-1.4 doc").unwrap();
        std::fs::write(&limpio, b"%PDF-1.4 limpio").unwrap();
        let hoja = |id: &str, pagina: u32, dibujo: Option<&str>| Hoja {
            id: id.into(),
            pagina: Some(pagina),
            dibujo: dibujo.map(str::to_string),
            ..Default::default()
        };
        let p = Proyecto {
            id: ficha.id.clone(),
            nombre: "Obra".into(),
            hojas: vec![
                hoja("h0", 0, Some("dib-1")),
                hoja("h1", 1, None),
                hoja("h5", 5, None),
            ],
            tocado: 10,
            pdf_origen: Some("archivos/doc-7.pdf".into()),
            pdf_limpio: Some("archivos/limpio-7.pdf".into()),
            ..Default::default()
        };
        guardar_proyecto(r, &ficha.id, &p).unwrap();
        (ficha, doc, limpio)
    }

    #[test]
    fn la_huella_es_el_hashcode_de_kotlin_en_hexadecimal_sin_signo() {
        // `"".hashCode()` es 0, `"a".hashCode()` es 97 y
        // `"polygenelubricants".hashCode()` es Integer.MIN_VALUE.
        assert_eq!(huella(""), "0");
        assert_eq!(huella("a"), "61");
        assert_eq!(huella("polygenelubricants"), "80000000");
        assert_eq!(suelto("61", 3), "pdf-61-p3");
    }

    #[test]
    fn el_documento_y_la_copia_limpia_de_un_proyecto_se_reconocen_como_suyos() {
        let r = raiz("de-quien");
        let (ficha, doc, limpio) = con_pdf(&r);
        let d = de_este_pdf(&r, &doc).expect("es su documento");
        assert_eq!(d.ficha, ficha.id);
        assert_eq!(d.limpio.as_deref(), Some(limpio.as_path()));
        assert_eq!(de_este_pdf(&r, &limpio).map(|d| d.ficha), Some(ficha.id));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_pdf_del_chat_que_no_es_de_ningun_proyecto_no_se_toma_por_suyo() {
        // Caso negativo: un adjunto de la conversacion, aunque este en la
        // carpeta del mismo proyecto, no es su documento.
        let r = raiz("de-nadie");
        let (ficha, _, _) = con_pdf(&r);
        let otro = almacen::carpeta(&r, &ficha.id).join("archivos/informe.pdf");
        std::fs::write(&otro, b"%PDF-1.4 otro").unwrap();
        assert_eq!(de_este_pdf(&r, &otro), None);
        assert_eq!(de_este_pdf(&r, Path::new("C:/no/existe.pdf")), None);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_hoja_con_dibujo_lo_usa_y_la_que_no_lo_recibe_solo_al_escribir() {
        let r = raiz("id-de");
        let (ficha, _, _) = con_pdf(&r);
        assert_eq!(
            dibujo_de_la_pagina(&r, &ficha.id, 0, "abc", true, 99).as_deref(),
            Some("dib-1")
        );
        // Solo mirar no cambia el proyecto.
        assert_eq!(
            dibujo_de_la_pagina(&r, &ficha.id, 1, "abc", false, 99).as_deref(),
            Some("pdf-abc-p1")
        );
        let p = leer_proyecto(&r, &ficha.id).unwrap();
        assert_eq!(p.hojas[1].dibujo, None);
        assert_eq!(p.tocado, 10);
        // Escribir se lo apunta, y el proyecto sube su `tocado` para viajar.
        dibujo_de_la_pagina(&r, &ficha.id, 1, "abc", true, 99);
        let p = leer_proyecto(&r, &ficha.id).unwrap();
        assert_eq!(p.hojas[1].dibujo.as_deref(), Some("pdf-abc-p1"));
        assert_eq!(p.tocado, 99);
        // Caso negativo: una pagina sin hoja en el proyecto es tinta suelta.
        assert_eq!(dibujo_de_la_pagina(&r, &ficha.id, 2, "abc", true, 99), None);
        let _ = std::fs::remove_dir_all(&r);
    }

    const TRAZO: &str = r#"{"type":"excalidraw","elements":[{"id":"t1","type":"freedraw","x":1,"y":2,"width":3,"height":4,"seed":1,"points":[[0,0],[3,4]]}],"files":{"f":{"id":"f","mimeType":"image/png","path":"imagenes/f"}}}"#;

    #[test]
    fn lo_anotado_antes_junto_al_pdf_pasa_a_la_hoja_del_proyecto() {
        let r = raiz("adoptar");
        let (ficha, doc, _) = con_pdf(&r);
        let hermana = doc.with_file_name("doc-7.pdf.pixpin-anotado");
        std::fs::create_dir_all(&hermana).unwrap();
        // Hoja 1 (sin dibujo): el suelto pasa a ser el suyo.
        let suelta1 = hermana.join("hoja-2.excalidraw");
        std::fs::write(&suelta1, TRAZO).unwrap();
        assert!(adoptar(&r, &ficha.id, 1, "abc", &suelta1, 50).unwrap());
        let d = fichero(&r, &ficha.id, "pdf-abc-p1");
        assert_eq!(std::fs::read_to_string(&d).unwrap(), TRAZO);
        assert!(!suelta1.exists() && hermana.join("hoja-2.excalidraw.adoptada").exists());
        // Hoja 0 (con dibujo): se le anade lo que no tenia, sin repetir.
        let d0 = fichero(&r, &ficha.id, "dib-1");
        std::fs::create_dir_all(d0.parent().unwrap()).unwrap();
        std::fs::write(
            &d0,
            r#"{"type":"excalidraw","hoja":{"padre":"x"},"elements":[{"id":"t1","type":"freedraw","x":9}]}"#,
        )
        .unwrap();
        let suelta0 = hermana.join("hoja-1.excalidraw");
        std::fs::write(
            &suelta0,
            TRAZO.replace(
                r#"}],"files""#,
                r#"},{"id":"t2","type":"line","x":5}],"files""#,
            ),
        )
        .unwrap();
        assert!(adoptar(&r, &ficha.id, 0, "abc", &suelta0, 60).unwrap());
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&d0).unwrap()).unwrap();
        let ids: Vec<&str> = v["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["t1", "t2"]);
        assert_eq!(v["elements"][0]["x"], 9, "lo de la hoja manda");
        assert!(v["hoja"].is_object(), "lo demas del lienzo se conserva");
        assert!(v["files"].get("f").is_some());
        let _ = std::fs::remove_dir_all(&r);
    }

    /// «Mensajes guardados» con un PDF adjunto de tres hojas, anotado en el
    /// lector en la 1 y la 3 (junto al PDF, como hasta ahora).
    fn pdf_del_chat(r: &Path) -> (almacen::Ficha, PathBuf) {
        let g = almacen::asegurar_guardados(r, 1, "PC01").unwrap();
        let carpeta = almacen::carpeta(r, &g.id);
        std::fs::create_dir_all(carpeta.join("archivos")).unwrap();
        let pdf = carpeta.join("archivos/informe.pdf");
        std::fs::write(&pdf, b"%PDF-1.4 informe").unwrap();
        let hermana = pdf.with_file_name("informe.pdf.pixpin-anotado");
        std::fs::create_dir_all(&hermana).unwrap();
        std::fs::write(hermana.join("hoja-1.excalidraw"), TRAZO).unwrap();
        std::fs::write(hermana.join("hoja-3.excalidraw"), TRAZO.replace("t1", "t3")).unwrap();
        (g, pdf)
    }

    fn suelta_de(pdf: &Path) -> impl Fn(u32) -> PathBuf + '_ {
        move |i| {
            pdf.with_file_name("informe.pdf.pixpin-anotado")
                .join(format!("hoja-{}.excalidraw", i + 1))
        }
    }

    #[test]
    fn al_proyecto_hace_del_pdf_del_chat_un_proyecto_que_viaja_con_lo_anotado() {
        use pixpin_sincro::disco::Disco;
        let r = raiz("al-proyecto");
        let (_, pdf) = pdf_del_chat(&r);
        let ficha = al_proyecto(&r, &pdf, "Informe", 3, 700, &suelta_de(&pdf)).unwrap();
        let p = leer_proyecto(&r, &ficha).unwrap();
        assert_eq!(p.hojas.len(), 3);
        assert_eq!(
            p.hojas.iter().map(|h| h.pagina).collect::<Vec<_>>(),
            [Some(0), Some(1), Some(2)]
        );
        // El documento es el mismo adjunto, en la forma que entiende el movil.
        assert_eq!(
            p.pdf_origen.as_deref(),
            Some("pixpin:files/guardados/pc/general/archivos/informe.pdf")
        );
        assert_eq!(p.pdf_limpio.as_deref(), Some("archivos/limpio-700.pdf"));
        // Y el lector lo reconoce desde ya como de ese proyecto.
        assert_eq!(de_este_pdf(&r, &pdf).map(|d| d.ficha), Some(ficha.clone()));
        // Lo anotado esta en las hojas 0 y 2; la 1 sigue sin dibujo.
        let h = huella(&pdf.to_string_lossy());
        assert_eq!(
            p.hojas[0].dibujo.as_deref(),
            Some(format!("pdf-{h}-p0").as_str())
        );
        assert_eq!(p.hojas[1].dibujo, None);
        assert_eq!(
            p.hojas[2].dibujo.as_deref(),
            Some(format!("pdf-{h}-p2").as_str())
        );
        // Su chat empieza con el PDF, ya unido.
        let chat = crate::cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, &ficha)).unwrap();
        assert_eq!(chat.mensajes.len(), 1);
        assert_eq!(
            chat.mensajes[0].ruta.as_deref(),
            Some("archivos/limpio-700.pdf")
        );
        assert_eq!(
            chat.mensajes[0].resto.get("unido"),
            Some(&Value::Bool(true))
        );
        // Lo que manda la sincronizacion con ese proyecto: las dos hojas
        // anotadas, legibles para el movil, y los dos PDF.
        let d = vista::DiscoPc::nuevo(&r);
        let c = vista::chat_de_ficha(&r, &ficha).unwrap();
        let rutas: Vec<String> = d.alcance(&c).unwrap().into_iter().map(|(x, _)| x).collect();
        for esperada in [
            format!("pins/draw/pdf-{h}-p0.excalidraw.gz"),
            format!("pins/draw/pdf-{h}-p2.excalidraw.gz"),
            "guardados/pc/general/archivos/informe.pdf".to_string(),
        ] {
            assert!(rutas.contains(&esperada), "{esperada} en {rutas:?}");
        }
        let hoja0 = d
            .texto_de(&c, &format!("pins/draw/pdf-{h}-p0.excalidraw.gz"))
            .unwrap();
        assert!(
            hoja0.contains(r#""x":0"#) && !hoja0.contains("[[0,0]"),
            "{hoja0}"
        );
        // Pedirlo otra vez no crea otro.
        assert_eq!(
            al_proyecto(&r, &pdf, "Informe", 3, 800, &suelta_de(&pdf)).unwrap(),
            ficha
        );
        assert_eq!(Indice::leer(&r).proyectos.len(), 2);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn al_proyecto_de_un_pdf_de_fuera_lo_copia_y_sin_hojas_no_crea_nada() {
        let r = raiz("al-proyecto-fuera");
        let fuera = r.join("descargas/plano.pdf");
        std::fs::create_dir_all(fuera.parent().unwrap()).unwrap();
        std::fs::write(&fuera, b"%PDF-1.4 plano").unwrap();
        let nada = |_| r.join("no-esta");
        let ficha = al_proyecto(&r, &fuera, "plano.pdf", 1, 5, &nada).unwrap();
        let p = leer_proyecto(&r, &ficha).unwrap();
        assert_eq!(p.pdf_origen.as_deref(), Some("archivos/doc-5.pdf"));
        assert!(
            almacen::carpeta(&r, &ficha)
                .join("archivos/doc-5.pdf")
                .is_file()
        );
        // Casos negativos: sin hojas, o un PDF que no esta, no crean nada.
        let r2 = raiz("al-proyecto-nada");
        assert!(al_proyecto(&r2, &fuera, "x", 0, 5, &nada).is_err());
        assert!(al_proyecto(&r2, &r2.join("no.pdf"), "x", 2, 5, &nada).is_err());
        assert!(Indice::leer(&r2).proyectos.is_empty());
        let _ = std::fs::remove_dir_all(&r);
        let _ = std::fs::remove_dir_all(&r2);
    }

    #[test]
    fn no_se_adopta_lo_vacio_ni_lo_que_no_tiene_hoja() {
        // Casos negativos: una capa vacia o con todo borrado, una ilegible y
        // una de una pagina que el proyecto no tiene se quedan donde estan.
        let r = raiz("no-adoptar");
        let (ficha, doc, _) = con_pdf(&r);
        let hermana = doc.with_file_name("doc-7.pdf.pixpin-anotado");
        std::fs::create_dir_all(&hermana).unwrap();
        let s = hermana.join("hoja-2.excalidraw");
        for texto in [
            r#"{"elements":[]}"#,
            r#"{"elements":[{"id":"a","type":"line","isDeleted":true}]}"#,
            "no es json",
        ] {
            std::fs::write(&s, texto).unwrap();
            assert!(!adoptar(&r, &ficha.id, 1, "abc", &s, 1).unwrap(), "{texto}");
            assert!(s.exists());
        }
        let s3 = hermana.join("hoja-3.excalidraw");
        std::fs::write(&s3, TRAZO).unwrap();
        assert!(!adoptar(&r, &ficha.id, 2, "abc", &s3, 1).unwrap());
        assert!(s3.exists());
        assert!(!adoptar(&r, &ficha.id, 1, "abc", &hermana.join("no-esta"), 1).unwrap());
        // Y el proyecto no se toco.
        assert_eq!(leer_proyecto(&r, &ficha.id).unwrap().hojas[1].dibujo, None);
        let _ = std::fs::remove_dir_all(&r);
    }
}
