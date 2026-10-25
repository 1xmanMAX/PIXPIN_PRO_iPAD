//! **Sin Anki abierto**: un `.txt` que Anki importa tal cual («Archivo →
//! Importar»). Las cabeceras `#…` son las de Anki 2.1.55 en adelante: el
//! separador, que los campos son HTML, el mazo y las etiquetas.

use crate::Tarjeta;

/// El texto del fichero de importar.
pub fn texto(mazo: &str, tarjetas: &[Tarjeta]) -> String {
    let mut s = String::from("#separator:tab\n#html:true\n");
    s.push_str(&format!("#deck:{}\n", limpio(mazo)));
    s.push_str("#tags column:3\n");
    for t in tarjetas {
        s.push_str(&limpio(&t.frente));
        s.push('\t');
        s.push_str(&limpio(&t.reverso));
        s.push('\t');
        s.push_str(&t.etiquetas.join(" "));
        s.push('\n');
    }
    s
}

/// Sin tabuladores ni saltos de linea: romperian las columnas.
fn limpio(t: &str) -> String {
    t.replace(['\t', '\n', '\r'], " ")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_fichero_lleva_su_mazo_y_una_tarjeta_por_linea() {
        let t = vec![Tarjeta {
            frente: "a\tb".into(),
            reverso: "c\nd".into(),
            etiquetas: vec!["pixpin".into(), "clase".into()],
            imagenes: Vec::new(),
        }];
        let s = texto("PixPin::Física", &t);
        assert!(s.starts_with("#separator:tab\n#html:true\n#deck:PixPin::Física\n"));
        assert!(s.ends_with("a b\tc d\tpixpin clase\n"), "{s}");
    }
}
