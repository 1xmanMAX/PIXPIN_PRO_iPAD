//! **Un Excel del chat es un libro de tablas**, traido de Windows
//! (`apps/pixpin/src/hoja_pin/libro.rs`, igual). Es
//! `guardados/LibroDelChat.kt` del movil.
//!
//! Cada hoja del libro, al editarla, queda como una tabla del proyecto con
//! una referencia **sacada del mensaje** (`libro-<mensaje>-<hoja>`, la misma
//! que el movil): la proxima vez que se toque el archivo se editan esas
//! tablas, con lo que se haya cambiado en ellas, en vez de volver a leer el
//! libro encima. El `.xlsx` original no se toca nunca. Las hojas que el
//! movil guardaba solo en `tablas/libro-*.json` antes del 13-oct
//! (`vista::tablas_viejas_del_libro`) pasan tambien a mensajes al guardar.

use pixpin_proyecto::cuaderno::{Clase, Mensaje, Sello};
use pixpin_proyecto::importar_hojas;

/// Si el mensaje es un libro de hojas (o un `.xls`, que se reconoce para
/// decir que no se lee).
pub fn es_libro(m: &Mensaje) -> bool {
    m.clase == Some(Clase::Archivo)
        && m.ruta.as_deref().is_some_and(|r| !r.is_empty())
        && (importar_hojas::es_libro(&m.nombre) || m.ruta.as_deref().is_some_and(importar_hojas::es_libro))
}

/// La extension con que se reconoce el libro: la del nombre o, si no trae,
/// la de la ruta.
pub fn extension(m: &Mensaje) -> String {
    let e = importar_hojas::extension(&m.nombre);
    if e.is_empty() { m.ruta.as_deref().map(importar_hojas::extension).unwrap_or_default() } else { e }
}

/// La referencia de la tabla de la hoja `hoja` del libro `m`
/// (`LibroDelChat.idDeHoja`).
pub fn id_de_hoja(m: &Mensaje, hoja: usize) -> String {
    format!("libro-{}-{hoja}", m.id)
}

/// Las tablas que ya salieron de este libro, en el orden de sus hojas
/// (posiciones en `mensajes`).
pub fn tablas_hechas(mensajes: &[Mensaje], libro: &Mensaje) -> Vec<usize> {
    let prefijo = format!("libro-{}-", libro.id);
    let mut hechas: Vec<(usize, usize)> = mensajes
        .iter()
        .enumerate()
        .filter(|(_, m)| m.miniapp.as_deref() == Some(pixpin_proyecto::tabla::MINIAPP))
        .filter_map(|(i, m)| {
            let hoja = m.referencia.as_deref()?.strip_prefix(&prefijo)?.parse().ok()?;
            Some((hoja, i))
        })
        .collect();
    hechas.sort();
    hechas.into_iter().map(|(_, i)| i).collect()
}

/// Una hoja ya escrita en JSON, lista para ser mensaje.
pub struct HojaLista {
    pub nombre: String,
    pub documento: String,
}

/// Los mensajes de las tablas de un libro, uno por hoja.
pub fn mensajes_de_hojas(libro: &Mensaje, hojas: &[HojaLista], sello: &Sello) -> Vec<Mensaje> {
    hojas
        .iter()
        .enumerate()
        .map(|(i, h)| {
            // El id es la hora: con varias hojas en el mismo milisegundo se
            // pisarian, asi que cada una va un milisegundo detras.
            let sello = Sello { cuando: sello.cuando + i as i64, numero: sello.numero + i as i64, ..sello.clone() };
            let mut m = Mensaje::miniapp(pixpin_proyecto::tabla::MINIAPP, &h.nombre, &h.documento, &sello);
            m.referencia = Some(id_de_hoja(libro, i));
            m
        })
        .collect()
}

/// **Si una tabla se cambio despues de nacer**: su `tocado` es posterior a
/// la hora de su mensaje. Al leer el libro las dos son la misma (o la tabla
/// un poco antes); guardar desde el pin o desde el movil pone `tocado` a la
/// hora de guardar. Se lee solo ese campo, sin montar las celdas.
#[cfg(test)]
pub fn editada(m: &Mensaje) -> bool {
    #[derive(serde::Deserialize)]
    struct Solo {
        #[serde(default)]
        tocado: i64,
    }
    // Dos segundos de holgura: el reloj del movil y el del PC no van igual.
    serde_json::from_str::<Solo>(&m.texto).is_ok_and(|s| s.tocado > m.cuando + 2_000)
}

/// El nombre de la pestana de una tabla de libro: «Cuentas · Gastos» ->
/// «Gastos». Una tabla suelta se queda con el suyo.
pub fn nombre_de_pestana(nombre: &str) -> &str {
    match nombre.rsplit_once(" · ") {
        Some((_, h)) if !h.trim().is_empty() => h,
        _ => nombre,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn libro() -> Mensaje {
        Mensaje {
            id: "1700".into(),
            clase: Some(Clase::Archivo),
            nombre: "Cuentas.xlsx".into(),
            ruta: Some("archivos/Cuentas.xlsx".into()),
            ..Default::default()
        }
    }

    #[test]
    fn un_excel_del_chat_es_un_libro_y_una_foto_no() {
        assert!(es_libro(&libro()));
        let mut csv = libro();
        csv.nombre = "datos.csv".into();
        assert!(es_libro(&csv));
        let mut sin_nombre = libro();
        sin_nombre.nombre = String::new();
        assert!(es_libro(&sin_nombre), "sin nombre, por la ruta");
        assert_eq!(extension(&sin_nombre), "xlsx");
        // Casos negativos: otra clase, sin fichero u otra extension.
        let mut nota = libro();
        nota.clase = Some(Clase::Nota);
        assert!(!es_libro(&nota));
        let mut sin_ruta = libro();
        sin_ruta.ruta = None;
        assert!(!es_libro(&sin_ruta));
        let mut pdf = libro();
        pdf.nombre = "a.pdf".into();
        pdf.ruta = Some("archivos/a.pdf".into());
        assert!(!es_libro(&pdf));
    }

    #[test]
    fn cada_hoja_es_una_tabla_con_la_referencia_del_libro_y_en_su_orden() {
        let l = libro();
        let doc = |t: &str, n: &str| importar_hojas::de_csv(t, n, false, 0).tabla.escribir().unwrap();
        let hojas = vec![
            HojaLista { nombre: "Cuentas · Gastos".into(), documento: doc("a;1", "Cuentas · Gastos") },
            HojaLista { nombre: "Cuentas · Resumen".into(), documento: doc("b;2", "Cuentas · Resumen") },
        ];
        let sello = Sello { cuando: 5000, numero: 9, aparato: "K7Q2".into(), proyecto: "p".into() };
        let nuevos = mensajes_de_hojas(&l, &hojas, &sello);
        assert_eq!(nuevos.len(), 2);
        assert_eq!(nuevos[0].referencia.as_deref(), Some("libro-1700-0"));
        assert_eq!(nuevos[1].referencia.as_deref(), Some("libro-1700-1"));
        assert_ne!(nuevos[0].id, nuevos[1].id, "dos hojas en el mismo milisegundo no se pisan");
        assert_eq!(nuevos[1].numero, 10);
        let t = pixpin_proyecto::tabla::Tabla::leer(&nuevos[1].texto).unwrap();
        assert_eq!(t.celda(pixpin_proyecto::tabla::ref_de("B1").unwrap()), "2");

        // Y la segunda vez se encuentran, en el orden de las hojas aunque
        // esten desordenadas en el chat.
        let chat = vec![l.clone(), nuevos[1].clone(), Mensaje::default(), nuevos[0].clone()];
        assert_eq!(tablas_hechas(&chat, &l), vec![3, 1]);
        // Caso negativo: las tablas de otro libro no son de este.
        let mut otro = l.clone();
        otro.id = "17".into();
        assert!(tablas_hechas(&chat, &otro).is_empty(), "libro-17- no es prefijo de libro-1700-");
    }

    #[test]
    fn una_tabla_recien_leida_no_esta_editada_y_una_guardada_si() {
        let mut m = Mensaje { cuando: 1_000_000, ..Default::default() };
        m.texto = r#"{"nombre":"x","celdas":{},"tocado":1000000}"#.into();
        assert!(!editada(&m));
        m.texto = r#"{"nombre":"x","celdas":{"A1":"1"},"tocado":1999999}"#.into();
        assert!(editada(&m));
        // Casos negativos: sin `tocado`, o un texto que no es una tabla.
        m.texto = r#"{"nombre":"x"}"#.into();
        assert!(!editada(&m));
        m.texto = "hola".into();
        assert!(!editada(&m));
    }

    #[test]
    fn la_pestana_de_una_tabla_de_libro_es_el_nombre_de_su_hoja() {
        assert_eq!(nombre_de_pestana("Cuentas · Gastos"), "Gastos");
        assert_eq!(nombre_de_pestana("Cuentas"), "Cuentas");
        // Caso negativo: un separador sin hoja detras no corta el nombre.
        assert_eq!(nombre_de_pestana("Cuentas · "), "Cuentas · ");
    }
}
