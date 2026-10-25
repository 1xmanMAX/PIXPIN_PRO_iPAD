//! **De las palabras de una foto o un PDF, con su caja, a una tabla.**
//!
//! Pedido del usuario (14-oct-2026, «integracion completa con la app»): con
//! la foto o el PDF de una tabla, sacar una tabla del chat o un Excel. El
//! modelo NO lee las celdas (se inventaria numeros y tardaria): solo decide
//! «tabla o Excel», el nombre y si la primera fila es cabecera. Las celdas
//! salen de aqui, un algoritmo clasico sobre lo que da el OCR de Windows
//! (`pixpin_ocr::Palabra`: texto y caja en pixeles) o la capa de texto del
//! PDF (`pixpin_pdf::texto::TrozoDeTexto`, en puntos). Las unidades dan
//! igual: todo se mide en altos de letra.
//!
//! ## Como
//!
//! 1. **Filas**: las palabras se ordenan por su centro vertical y se juntan
//!    mientras se solapen en vertical con la fila (mas de la mitad del alto
//!    menor). Asi una fila torcida un poco en la foto sigue siendo una.
//! 2. **Celdas**: dentro de una fila, palabras separadas menos de
//!    [`HUECO_EN_CELDA`] altos de letra son la misma celda («Jr. Antisuyo»).
//! 3. **Columnas**: los intervalos horizontales de las celdas, de la mas
//!    estrecha a la mas ancha. Una celda que cae en una sola columna la
//!    ensancha; una que pisa dos o mas ya hechas es una **celda combinada**
//!    (un titulo que abarca varias) y no las junta. Por eso se empieza por
//!    las estrechas: son las que dibujan las columnas de verdad.
//! 4. Cada celda va a la primera columna que pisa y, si pisa varias, se
//!    apunta como combinada ([`Combinada`]). La tabla del chat no combina
//!    celdas (ni la de Android): el texto queda en la primera y el resto
//!    vacio, que es lo que se ve al abrir un Excel con celdas combinadas.

/// Una palabra (o un trozo de texto) con su caja. `x`, `y` es la esquina de
/// arriba a la izquierda, en las unidades que sean.
#[derive(Debug, Clone, PartialEq)]
pub struct Palabra {
    pub texto: String,
    pub x: f64,
    pub y: f64,
    pub ancho: f64,
    pub alto: f64,
}

impl Palabra {
    pub fn nueva(texto: &str, x: f64, y: f64, ancho: f64, alto: f64) -> Palabra {
        Palabra {
            texto: texto.to_string(),
            x,
            y,
            ancho,
            alto,
        }
    }
    fn fin(&self) -> f64 {
        self.x + self.ancho
    }
    fn centro_y(&self) -> f64 {
        self.y + self.alto / 2.0
    }
}

/// Una celda que abarca varias columnas: fila, primera columna y cuantas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Combinada {
    pub fila: usize,
    pub columna: usize,
    pub columnas: usize,
}

/// La tabla reconstruida: `filas[f][c]`, todas del mismo largo.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TablaLeida {
    pub filas: Vec<Vec<String>>,
    pub combinadas: Vec<Combinada>,
}

impl TablaLeida {
    pub fn columnas(&self) -> usize {
        self.filas.first().map_or(0, Vec::len)
    }
    /// Si la primera fila parece cabecera: ningun numero y alguna letra,
    /// con numeros en las de abajo. Lo decide el modelo; esto es lo que se
    /// usa si no dice nada.
    pub fn parece_cabecera(&self) -> bool {
        let Some(primera) = self.filas.first() else { return false };
        let es_numero = |s: &str| {
            let t: String = s.chars().filter(|c| !matches!(c, ' ' | ',' | '.' | '%' | '$' | '-' | '/')).collect();
            !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())
        };
        let sin_numeros = primera.iter().all(|c| !es_numero(c)) && primera.iter().any(|c| !c.trim().is_empty());
        let abajo = self.filas.iter().skip(1).flatten().any(|c| es_numero(c));
        sin_numeros && (abajo || self.filas.len() > 1)
    }
    /// En TSV (lo que se pega en una hoja), para el modelo y el portapapeles.
    pub fn a_tsv(&self) -> String {
        self.filas.iter().map(|f| f.join("\t")).collect::<Vec<_>>().join("\n")
    }
}

/// **Un numero escrito como en un papel**, en la forma que entiende la hoja
/// (`1250.5`): con miles con espacio, punto o coma («1 250», «1.250,50»,
/// «1,250.50»), signo y moneda delante. `None` si no es un numero (un
/// codigo como «R-1» o una fecha se quedan como texto). Con un solo
/// separador seguido de 3 cifras, son miles («1.250» = 1250), como lo leeria
/// una persona en una tabla de costos.
pub fn como_numero(s: &str) -> Option<String> {
    let t = s.trim().trim_start_matches(['$', '€', 'S', '/', '.']).trim();
    let t: String = t.chars().filter(|c| !c.is_whitespace() && *c != '\u{a0}').collect();
    let (signo, t) = match t.strip_prefix('-') {
        Some(r) => ("-", r.to_string()),
        None => ("", t),
    };
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') || !t.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let ultimo = t.rfind(['.', ',']);
    let limpio = match ultimo {
        None => t.clone(),
        Some(i) => {
            let sep = t.as_bytes()[i] as char;
            let otros = t.matches(['.', ',']).count();
            let decimales = t.len() - i - 1;
            let mezcla = t.contains('.') && t.contains(',');
            if mezcla || (otros == 1 && decimales != 3) {
                // El ultimo es el decimal; los demas, miles.
                let (ent, dec) = t.split_at(i);
                let ent: String = ent.chars().filter(char::is_ascii_digit).collect();
                format!("{ent}.{}", &dec[1..])
            } else if t.matches(sep).count() == otros {
                // Uno o varios del mismo con grupos de 3: miles.
                let grupos: Vec<&str> = t.split(sep).collect();
                if grupos[1..].iter().all(|g| g.len() == 3) {
                    grupos.concat()
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
    };
    limpio.parse::<f64>().ok().filter(|n| n.is_finite())?;
    let limpio = limpio.trim_start_matches('0');
    let limpio = if limpio.is_empty() || limpio.starts_with('.') { format!("0{limpio}") } else { limpio.to_string() };
    Some(format!("{signo}{limpio}"))
}

/// Dos palabras con un hueco menor que esto (en altos de letra) son la
/// misma celda. Un espacio normal es ~0.3; entre columnas suele haber 1.5+.
pub const HUECO_EN_CELDA: f64 = 0.9;

/// **Reconstruye la tabla.** Sin palabras, tabla vacia.
pub fn reconstruir(palabras: &[Palabra]) -> TablaLeida {
    let mut ps: Vec<&Palabra> = palabras
        .iter()
        .filter(|p| !p.texto.trim().is_empty() && p.alto > 0.0 && p.ancho >= 0.0)
        .collect();
    if ps.is_empty() {
        return TablaLeida::default();
    }
    // 1. Filas.
    ps.sort_by(|a, b| a.centro_y().total_cmp(&b.centro_y()));
    let mut filas: Vec<Vec<&Palabra>> = Vec::new();
    let mut banda = (0.0f64, 0.0f64);
    for p in ps {
        let solapa = match filas.last() {
            Some(_) => {
                let arriba = banda.0.max(p.y);
                let abajo = banda.1.min(p.y + p.alto);
                let menor = (banda.1 - banda.0).min(p.alto);
                abajo - arriba > menor * 0.5
            }
            None => false,
        };
        if solapa {
            filas.last_mut().unwrap().push(p);
            // La banda es la media de la fila: no crece sin fin.
            let f = filas.last().unwrap();
            let y0 = f.iter().map(|p| p.y).sum::<f64>() / f.len() as f64;
            let alto = f.iter().map(|p| p.alto).sum::<f64>() / f.len() as f64;
            banda = (y0, y0 + alto);
        } else {
            banda = (p.y, p.y + p.alto);
            filas.push(vec![p]);
        }
    }
    // 2. Celdas de cada fila: (inicio, fin, texto).
    let mut celdas: Vec<Vec<(f64, f64, String)>> = Vec::with_capacity(filas.len());
    for mut f in filas {
        f.sort_by(|a, b| a.x.total_cmp(&b.x));
        let mut cs: Vec<(f64, f64, String, f64)> = Vec::new();
        for p in f {
            if let Some(ultima) = cs.last_mut() {
                let alto = ultima.3.max(p.alto);
                if p.x - ultima.1 < HUECO_EN_CELDA * alto {
                    ultima.1 = ultima.1.max(p.fin());
                    ultima.2.push(' ');
                    ultima.2.push_str(p.texto.trim());
                    continue;
                }
            }
            cs.push((p.x, p.fin(), p.texto.trim().to_string(), p.alto));
        }
        celdas.push(cs.into_iter().map(|(a, b, t, _)| (a, b, t)).collect());
    }
    // 3. Columnas, de las celdas estrechas a las anchas.
    let mut todas: Vec<(f64, f64)> = celdas.iter().flatten().map(|c| (c.0, c.1)).collect();
    todas.sort_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)));
    let mut columnas: Vec<(f64, f64)> = Vec::new();
    for (a, b) in todas {
        let pisa: Vec<usize> = (0..columnas.len()).filter(|&i| se_pisan((a, b), columnas[i])).collect();
        match pisa.as_slice() {
            [] => columnas.push((a, b)),
            [i] => {
                columnas[*i].0 = columnas[*i].0.min(a);
                columnas[*i].1 = columnas[*i].1.max(b);
            }
            _ => {} // combinada: no junta columnas
        }
    }
    columnas.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Al ensancharse, dos columnas pueden haber acabado pisandose: se unen.
    let mut unidas: Vec<(f64, f64)> = Vec::new();
    for c in columnas {
        match unidas.last_mut() {
            Some(u) if c.0 < u.1 => u.1 = u.1.max(c.1),
            _ => unidas.push(c),
        }
    }
    // 4. Cada celda a su columna.
    let n = unidas.len().max(1);
    let mut tabla = TablaLeida::default();
    for (f, cs) in celdas.into_iter().enumerate() {
        let mut fila = vec![String::new(); n];
        for (a, b, texto) in cs {
            let pisa: Vec<usize> = (0..unidas.len()).filter(|&i| se_pisan((a, b), unidas[i])).collect();
            let primera = pisa.first().copied().unwrap_or_else(|| mas_cercana(&unidas, (a + b) / 2.0));
            if pisa.len() > 1 {
                tabla.combinadas.push(Combinada {
                    fila: f,
                    columna: primera,
                    columnas: pisa.len(),
                });
            }
            let c = &mut fila[primera];
            if !c.is_empty() {
                c.push(' ');
            }
            c.push_str(&texto);
        }
        tabla.filas.push(fila);
    }
    tabla.sin_prosa_alrededor();
    tabla
}

impl TablaLeida {
    /// **Quita el texto de la pagina que no es tabla** (banco de documentos,
    /// 15-oct: de un PDF con un titulo, una linea de docente y una
    /// observacion al pie, el Excel salia con esas filas pegadas en la
    /// columna A). Arriba y abajo, mientras la fila tenga una sola celda con
    /// algo y sea larga (una frase: un titulo corto combinado se queda), en tablas de 3
    /// columnas o mas. Siempre quedan 2 filas.
    pub fn sin_prosa_alrededor(&mut self) {
        if self.columnas() < 3 {
            return;
        }
        let es_prosa = |t: &TablaLeida, f: usize| {
            let llenas: Vec<&String> = t.filas[f].iter().filter(|c| !c.trim().is_empty()).collect();
            llenas.len() == 1 && llenas[0].chars().count() >= 25
        };
        while self.filas.len() > 2 && es_prosa(self, 0) {
            self.filas.remove(0);
            self.combinadas.retain(|c| c.fila != 0);
            for c in &mut self.combinadas {
                c.fila -= 1;
            }
        }
        while self.filas.len() > 2 && es_prosa(self, self.filas.len() - 1) {
            let f = self.filas.len() - 1;
            self.filas.pop();
            self.combinadas.retain(|c| c.fila != f);
        }
    }
}

/// Se pisan si comparten mas de un cuarto del menor de los dos.
fn se_pisan(a: (f64, f64), b: (f64, f64)) -> bool {
    let comun = a.1.min(b.1) - a.0.max(b.0);
    let menor = (a.1 - a.0).min(b.1 - b.0).max(1e-9);
    comun > 0.0 && comun / menor > 0.25
}

fn mas_cercana(columnas: &[(f64, f64)], x: f64) -> usize {
    (0..columnas.len())
        .min_by(|&i, &j| {
            let d = |k: usize| ((columnas[k].0 + columnas[k].1) / 2.0 - x).abs();
            d(i).total_cmp(&d(j))
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una tabla escrita «a maquina»: cada celda en su (columna, fila), con
    /// letras de 10 de alto y 6 de ancho, columnas cada `paso`.
    fn escribir(celdas: &[(usize, usize, &str)], pasos: &[f64]) -> Vec<Palabra> {
        let mut v = Vec::new();
        for &(c, f, t) in celdas {
            let mut x = pasos[..c].iter().sum::<f64>() + 5.0;
            let y = 20.0 * f as f64 + 3.0;
            for w in t.split(' ') {
                let ancho = 6.0 * w.chars().count() as f64;
                v.push(Palabra::nueva(w, x, y, ancho, 10.0));
                x += ancho + 3.0; // un espacio
            }
        }
        v
    }

    #[test]
    fn una_tabla_sencilla_sale_igual() {
        let pal = escribir(
            &[
                (0, 0, "Calle"),
                (1, 0, "Senales"),
                (2, 0, "Costo"),
                (0, 1, "Jr. Antisuyo"),
                (1, 1, "12"),
                (2, 1, "1 250.00"),
                (0, 2, "Av. Tahuantinsuyo"),
                (1, 2, "7"),
                (2, 2, "830.50"),
            ],
            &[140.0, 80.0, 80.0],
        );
        let t = reconstruir(&pal);
        assert_eq!(
            t.filas,
            vec![
                vec!["Calle", "Senales", "Costo"],
                vec!["Jr. Antisuyo", "12", "1 250.00"],
                vec!["Av. Tahuantinsuyo", "7", "830.50"],
            ]
        );
        assert!(t.combinadas.is_empty());
        assert!(t.parece_cabecera());
    }

    #[test]
    fn las_celdas_vacias_y_el_desorden_no_la_rompen() {
        let mut pal = escribir(
            &[(0, 0, "A"), (2, 0, "C"), (0, 1, "a1"), (1, 1, "b1"), (2, 1, "c1"), (1, 2, "b2")],
            &[60.0, 60.0, 60.0],
        );
        pal.reverse();
        // Una palabra torcida (2 px mas abajo) sigue en su fila.
        pal[0].y += 2.0;
        let t = reconstruir(&pal);
        assert_eq!(t.filas, vec![vec!["A", "", "C"], vec!["a1", "b1", "c1"], vec!["", "b2", ""]]);
    }

    #[test]
    fn el_texto_de_la_pagina_alrededor_no_entra_en_la_tabla() {
        let pasos = [80.0, 80.0, 80.0];
        let mut pal = escribir(&[(0, 1, "x"), (1, 1, "y"), (2, 1, "z"), (0, 2, "1"), (1, 2, "2"), (2, 2, "3")], &pasos);
        // Un parrafo encima y otro debajo, en la columna A.
        pal.extend(escribir(&[(0, 0, "Registro de notas del curso de Resistencia")], &pasos));
        pal.extend(escribir(&[(0, 3, "Observacion: el promedio es la media de las notas")], &pasos));
        let t = reconstruir(&pal);
        assert_eq!(t.filas.len(), 2, "{t:?}");
        assert_eq!(t.filas[0][0], "x");
        // Negativo: una tabla de dos columnas no se toca (podria ser
        // «concepto | importe» con conceptos largos).
        let mut dos = escribir(&[(0, 1, "a"), (1, 1, "b")], &pasos);
        dos.extend(escribir(&[(0, 0, "Una frase larga que va encima de la tabla")], &pasos));
        assert_eq!(reconstruir(&dos).filas.len(), 2);
    }

    #[test]
    fn un_titulo_que_abarca_varias_es_combinada_y_no_junta_columnas() {
        let mut pal = escribir(
            &[(0, 1, "x"), (1, 1, "y"), (2, 1, "z"), (0, 2, "1"), (1, 2, "2"), (2, 2, "3")],
            &[60.0, 60.0, 60.0],
        );
        // Titulo de la fila 0, de x=5 a x=170: pisa las tres columnas.
        pal.push(Palabra::nueva("RESUMEN", 5.0, 3.0, 80.0, 10.0));
        pal.push(Palabra::nueva("DE", 88.0, 3.0, 12.0, 10.0));
        pal.push(Palabra::nueva("COSTOS", 103.0, 3.0, 67.0, 10.0));
        let t = reconstruir(&pal);
        assert_eq!(t.columnas(), 3, "{t:?}");
        assert_eq!(t.filas[0], vec!["RESUMEN DE COSTOS", "", ""]);
        assert_eq!(t.combinadas, vec![Combinada { fila: 0, columna: 0, columnas: 3 }]);
        assert_eq!(t.filas[2], vec!["1", "2", "3"]);
    }

    #[test]
    fn los_numeros_del_papel_pasan_a_numeros_de_la_hoja() {
        assert_eq!(como_numero("1 250.00").as_deref(), Some("1250.00"));
        assert_eq!(como_numero("1.250,50").as_deref(), Some("1250.50"));
        assert_eq!(como_numero("1,250.50").as_deref(), Some("1250.50"));
        assert_eq!(como_numero("1.250").as_deref(), Some("1250"));
        assert_eq!(como_numero("830,5").as_deref(), Some("830.5"));
        assert_eq!(como_numero("S/. 12").as_deref(), Some("12"));
        assert_eq!(como_numero("-7").as_deref(), Some("-7"));
        assert_eq!(como_numero("0.75").as_deref(), Some("0.75"));
        // Lo que no es un numero se queda como texto.
        for no in ["R-1", "12/10/2026", "Jr. Antisuyo", "", "1,2,3"] {
            assert_eq!(como_numero(no), None, "{no}");
        }
    }

    #[test]
    fn sin_palabras_no_hay_tabla_y_sin_numeros_no_hay_cabecera() {
        assert_eq!(reconstruir(&[]), TablaLeida::default());
        assert_eq!(reconstruir(&[Palabra::nueva("  ", 0.0, 0.0, 5.0, 10.0)]).filas.len(), 0);
        let t = reconstruir(&escribir(&[(0, 0, "12"), (1, 0, "13"), (0, 1, "4"), (1, 1, "5")], &[50.0, 50.0]));
        assert!(!t.parece_cabecera());
        assert_eq!(t.a_tsv(), "12\t13\n4\t5");
    }
}
