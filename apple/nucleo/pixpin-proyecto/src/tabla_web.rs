//! **La tabla dentro de la pagina web** (J3): las dos piezas que la pagina
//! necesita de una [`Tabla`], como `motor/VisorTabla.kt` del movil
//! (`estatica` y `json`). El resto —el motor que recalcula en el navegador,
//! la rejilla que se edita y el guardar— es JavaScript copiado del movil y
//! vive con la pagina (`pixpin_motor2d::exportar_html`).
//!
//! La tabla sale **ya calculada** en el HTML: quien la abra en un visor que no
//! ejecuta guiones —muchos de los que se abren desde un chat— ve los totales.
//! Con guion, el visor la rehace desde el JSON y a partir de ahi es suya: lo
//! que ensena lo calcula el motor del movil, no el del PC.

use crate::formula::{self, Valor};
use crate::tabla::{Ref, Tabla, ref_a, ref_de};

/// El ancho de una columna sin tocar, en pixeles (`ANCHO` del visor).
const ANCHO: u32 = 96;
const MAX_FILAS: u32 = 100_000;
const MAX_COLS: u32 = 702;

/// **Que parte de la tabla se comparte** (`VisorTabla.marco`): lo escrito y
/// una fila y una columna de margen por cada lado, no una rejilla infinita.
/// Arriba y a la izquierda el margen no pasa de A1. `(fila0, col0, fila1,
/// col1)`, todo incluido.
pub fn marco(t: &Tabla) -> (u32, u32, u32, u32) {
    let refs: Vec<Ref> = t.celdas.keys().filter_map(|k| ref_de(k)).collect();
    let (Some(f0), Some(c0), Some(f1), Some(c1)) = (
        refs.iter().map(|r| r.fila).min(),
        refs.iter().map(|r| r.columna).min(),
        refs.iter().map(|r| r.fila).max(),
        refs.iter().map(|r| r.columna).max(),
    ) else {
        return (0, 0, 1, 1);
    };
    (
        f0.saturating_sub(1),
        c0.saturating_sub(1),
        (f1 + 1).min(MAX_FILAS - 1),
        (c1 + 1).min(MAX_COLS - 1),
    )
}

fn letras(columna: u32) -> String {
    ref_a(Ref { columna, fila: 0 })
        .trim_end_matches('1')
        .to_string()
}

fn escapar(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn es_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())
}

/// **La tabla ya calculada**, para leerse sin guion y para buscar dentro con
/// la lupa del navegador (`VisorTabla.estatica`). Como mucho `max_filas` por
/// `max_cols`: una tabla de cien mil filas no cabe en una pagina que se
/// manda por el chat, y el guion la pinta entera desde el JSON.
pub fn estatica(t: &Tabla, max_filas: u32, max_cols: u32, decimal: char) -> String {
    let (f0, c0, f1, c1) = marco(t);
    let f1 = f1.min(f0 + max_filas.max(1) - 1);
    let c1 = c1.min(c0 + max_cols.max(1) - 1);
    let valores = formula::evaluar_todo(t);
    let mut s = String::with_capacity(((f1 - f0 + 1) * (c1 - c0 + 1) * 24 + 200) as usize);
    s.push_str("<table class=\"calc\"><thead><tr><th class=\"esq\"></th>");
    for c in c0..=c1 {
        let l = letras(c);
        let ancho = t.anchos.get(&l).copied().unwrap_or(ANCHO);
        s.push_str(&format!("<th style=\"width:{ancho}px\">{l}</th>"));
    }
    s.push_str("</tr></thead><tbody>");
    for f in f0..=f1 {
        s.push_str(&format!("<tr><th>{}</th>", f + 1));
        for c in c0..=c1 {
            let dir = ref_a(Ref {
                columna: c,
                fila: f,
            });
            let crudo = t.celdas.get(&dir).map_or("", String::as_str);
            let valor = valores.get(&dir);
            // Como `texto` del motor del movil: lo escrito a mano se ensena tal
            // cual (sin el apostrofo), y lo calculado con el separador decimal
            // del usuario, el mismo que ensenara el guion al recalcular.
            let texto = if Tabla::es_formula(crudo) {
                valor.map(|v| v.mostrar(decimal)).unwrap_or_default()
            } else {
                crudo.strip_prefix('\'').unwrap_or(crudo).to_string()
            };
            let e = t.estilos.get(&dir);
            let alineacion = e.and_then(|e| e.a.as_deref()).unwrap_or(match valor {
                Some(Valor::Numero(_)) => "d",
                Some(Valor::Error(_)) => "c",
                _ => "i",
            });
            let mut clases = Vec::new();
            match alineacion {
                "d" => clases.push("d"),
                "c" => clases.push("c"),
                _ => {}
            }
            if e.is_some_and(|e| e.n) {
                clases.push("n");
            }
            if matches!(valor, Some(Valor::Error(_))) {
                clases.push("err");
            }
            if Tabla::es_formula(crudo) {
                clases.push("f");
            }
            if protegida(t) && e.is_some_and(|e| e.e) {
                clases.push("ed");
            }
            s.push_str("<td");
            if !clases.is_empty() {
                s.push_str(&format!(" class=\"{}\"", clases.join(" ")));
            }
            if let Some(fondo) = e.and_then(|e| e.f.as_deref()).filter(|f| es_color(f)) {
                s.push_str(&format!(" style=\"background:{fondo}\""));
            }
            s.push('>');
            s.push_str(&escapar(&texto));
            s.push_str("</td>");
        }
        s.push_str("</tr>");
    }
    s.push_str("</tbody></table>");
    s
}

/// Si la tabla esta protegida (`TablaDeCalculo.protegida` del movil; aqui
/// viaja entre lo que no se entiende).
fn protegida(t: &Tabla) -> bool {
    t.resto
        .get("protegida")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// **El JSON de la tabla, sin un solo `<`** (`VisorTabla.json`): asi nunca
/// cierra el `script` que lo lleva. Compacto: la pagina se manda por el chat.
pub fn json(t: &Tabla) -> String {
    serde_json::to_string(t)
        .unwrap_or_else(|_| "{}".into())
        .replace('<', "\\u003c")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::tabla::EstiloDeCelda;

    fn tabla(celdas: &[(&str, &str)]) -> Tabla {
        let mut t = Tabla {
            nombre: "Gastos".into(),
            ..Default::default()
        };
        for (d, v) in celdas {
            t.poner(ref_de(d).unwrap(), v);
        }
        t
    }

    #[test]
    fn el_marco_es_lo_escrito_con_una_fila_y_una_columna_de_margen() {
        assert_eq!(marco(&Tabla::default()), (0, 0, 1, 1), "vacia: A1:B2");
        assert_eq!(
            marco(&tabla(&[("A1", "1")])),
            (0, 0, 1, 1),
            "el margen no pasa de A1"
        );
        assert_eq!(marco(&tabla(&[("C3", "1"), ("D5", "2")])), (1, 1, 5, 4));
    }

    #[test]
    fn la_tabla_estatica_ya_trae_los_totales_con_sus_clases() {
        let mut t = tabla(&[
            ("A1", "Pan"),
            ("B1", "2,5"),
            ("B2", "=B1*2"),
            ("B3", "=1/0"),
            ("A3", "<b>"),
        ]);
        t.anchos.insert("A".into(), 150);
        t.estilos.insert(
            "A1".into(),
            EstiloDeCelda {
                n: true,
                f: Some("#fff2cc".into()),
                ..Default::default()
            },
        );
        let html = estatica(&t, 2000, 200, '.');
        assert!(html.starts_with("<table class=\"calc\"><thead><tr><th class=\"esq\"></th><th style=\"width:150px\">A</th><th style=\"width:96px\">B</th><th style=\"width:96px\">C</th></tr>"));
        assert!(html.contains("<tr><th>1</th><td class=\"n\" style=\"background:#fff2cc\">Pan</td><td class=\"d\">2,5</td>"));
        assert!(
            html.contains("<td class=\"d f\">5</td>"),
            "la formula ya calculada, a la derecha"
        );
        assert!(html.contains("<td class=\"c err f\">#¡DIV/0!</td>"));
        assert!(
            html.contains("<td>&lt;b&gt;</td>"),
            "lo escrito no se cuela como HTML"
        );
        // Hasta la fila 4: una de margen.
        assert!(html.contains("<tr><th>4</th>"));
        assert!(!html.contains("<tr><th>5</th>"));
    }

    #[test]
    fn lo_calculado_sale_con_el_separador_decimal_del_usuario_como_lo_escrito() {
        // La captura del 24-sep: «12,5» y «7» escritos y el total en «19.5».
        let t = tabla(&[
            ("B1", "12,5"),
            ("B2", "7"),
            ("B3", "=SUMA(B1:B2)"),
            ("B4", "'007"),
        ]);
        let es = estatica(&t, 100, 100, ',');
        assert!(es.contains("<td class=\"d\">12,5</td>"));
        assert!(es.contains("<td class=\"d f\">19,5</td>"), "es-ES: coma");
        let en = estatica(&t, 100, 100, '.');
        assert!(en.contains("<td class=\"d f\">19.5</td>"), "en-US: punto");
        // Caso negativo: lo escrito a mano no se toca (sale como se escribio,
        // sin el apostrofo), ni con coma ni con punto.
        assert!(en.contains("<td class=\"d\">12,5</td>"));
        assert!(es.contains("<td>007</td>"));
    }

    #[test]
    fn una_tabla_enorme_se_corta_en_la_estatica() {
        let t = tabla(&[("A1", "1"), ("A5000", "2")]);
        let html = estatica(&t, 10, 3, '.');
        assert!(html.contains("<tr><th>10</th>"));
        assert!(
            !html.contains("<tr><th>11</th>"),
            "caso negativo: no mas de diez filas"
        );
    }

    #[test]
    fn el_json_no_lleva_ningun_menor_que() {
        let t = tabla(&[("A1", "</script><script>alert(1)"), ("A2", "=A1")]);
        let j = json(&t);
        assert!(!j.contains('<'));
        // Y se vuelve a leer igual.
        let vuelta = Tabla::leer(&j.replace("\\u003c", "<")).unwrap();
        assert_eq!(vuelta.celdas, t.celdas);
        assert_eq!(
            Tabla::leer(&j).unwrap().celdas,
            t.celdas,
            "el \\u003c es JSON valido"
        );
    }

    /// Una pagina de muestra para mirarla en un navegador: `PIXPIN_MUESTRA`
    /// dice en que carpeta. No es una prueba automatica (hace falta ojos).
    #[test]
    #[ignore]
    fn muestra_de_pagina_con_tabla() {
        let Ok(carpeta) = std::env::var("PIXPIN_MUESTRA") else {
            return;
        };
        let mut t = tabla(&[
            ("A1", "Concepto"),
            ("B1", "Importe"),
            ("A2", "Cemento"),
            ("B2", "12,5"),
            ("A3", "Arena"),
            ("B3", "7"),
            ("A4", "Total"),
            ("B4", "=SUMA(B2:B3)"),
            ("C4", "=SI(B4>10;\"caro\";\"barato\")"),
        ]);
        t.estilos.insert(
            "A4".into(),
            EstiloDeCelda {
                n: true,
                f: Some("#fff2cc".into()),
                ..Default::default()
            },
        );
        for (decimal, fichero) in [(',', "tabla.html"), ('.', "tabla-en.html")] {
            let hoja = pixpin_motor2d::exportar_html::HojaTabla {
                nombre: t.nombre.clone(),
                fondo: "#ffffff".into(),
                json: json(&t),
                estatica: estatica(&t, 2000, 200, decimal),
                decimal,
            };
            let html = pixpin_motor2d::exportar_html::paginas_mixtas(
                &[pixpin_motor2d::exportar_html::HojaDeLaPagina::Tabla(&hoja)],
                "Gastos",
                "Gastos",
                pixpin_motor2d::exportar_html::Opciones::default(),
                None,
            )
            .unwrap();
            std::fs::write(std::path::Path::new(&carpeta).join(fichero), html).unwrap();
        }
    }

    #[test]
    fn una_celda_protegida_editable_se_marca() {
        let mut t = tabla(&[("A1", "1")]);
        t.resto
            .insert("protegida".into(), serde_json::Value::Bool(true));
        t.estilos.insert(
            "A1".into(),
            EstiloDeCelda {
                e: true,
                ..Default::default()
            },
        );
        assert!(estatica(&t, 10, 10, '.').contains("<td class=\"d ed\">1</td>"));
        // Caso negativo: sin proteger, la marca no sale.
        t.resto.clear();
        assert!(!estatica(&t, 10, 10, '.').contains(" ed"));
    }
}
