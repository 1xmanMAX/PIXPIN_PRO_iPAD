//! De donde salen las tarjetas. Puro: se prueba con textos.

/// Una imagen que va dentro de una tarjeta (en Anki, en su carpeta de
/// medios y con `<img src>` en el campo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imagen {
    /// El nombre con que se guarda en Anki: unico, sin carpetas.
    pub nombre: String,
    pub bytes: Vec<u8>,
}

/// Una tarjeta basica de Anki: delante y detras, en HTML.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tarjeta {
    pub frente: String,
    pub reverso: String,
    pub etiquetas: Vec<String>,
    /// Las imagenes que nombran `frente` o `reverso`.
    pub imagenes: Vec<Imagen>,
}

/// El texto a salvo dentro de HTML, con los saltos de linea como `<br>`.
pub fn html(texto: &str) -> String {
    tal_cual(texto.trim())
}

/// [`html`] sin quitar los blancos de los bordes (para trozos de una linea).
fn tal_cual(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            '"' => s.push_str("&quot;"),
            '\n' => s.push_str("<br>"),
            '\r' => {}
            c => s.push(c),
        }
    }
    s
}

/// Una etiqueta de Anki: sin espacios (alli separan etiquetas).
pub fn etiqueta(texto: &str) -> String {
    texto
        .trim()
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect()
}

/// **Las tarjetas de una nota.** Se entienden:
///
/// - `pregunta :: respuesta` (o `;;`), una por linea;
/// - `P: …` / `R: …` (y `Q:` / `A:`), la respuesta en las lineas que siguen
///   hasta la siguiente pregunta o una linea en blanco;
/// - una linea con huecos de Anki, `{{c1::Lima}} es la capital`: delante con
///   el hueco `[…]` y detras entera, lo tapado en negrita.
///
/// Lo demas se deja: un parrafo suelto no es una pregunta.
pub fn de_la_nota(texto: &str) -> Vec<Tarjeta> {
    let mut v = Vec::new();
    let mut abierta: Option<(String, String)> = None;
    let cerrar = |abierta: &mut Option<(String, String)>, v: &mut Vec<Tarjeta>| {
        if let Some((p, r)) = abierta.take()
            && !p.trim().is_empty()
            && !r.trim().is_empty()
        {
            v.push(Tarjeta {
                frente: html(&p),
                reverso: html(&r),
                ..Tarjeta::default()
            });
        }
    };
    for linea in texto.replace("\r\n", "\n").lines() {
        let l = linea.trim();
        if l.is_empty() {
            cerrar(&mut abierta, &mut v);
            continue;
        }
        if let Some(p) = quitar_prefijo(l, &["P:", "Q:", "Pregunta:", "Question:"]) {
            cerrar(&mut abierta, &mut v);
            abierta = Some((p.to_string(), String::new()));
            continue;
        }
        if let Some(r) = quitar_prefijo(l, &["R:", "A:", "Respuesta:", "Answer:"]) {
            if let Some((_, resp)) = abierta.as_mut() {
                if !resp.is_empty() {
                    resp.push('\n');
                }
                resp.push_str(r);
            }
            continue;
        }
        if let Some((_, resp)) = abierta.as_mut()
            && !resp.is_empty()
        {
            resp.push('\n');
            resp.push_str(l);
            continue;
        }
        let l = l.trim_start_matches(['-', '*', '•']).trim();
        if let Some((p, r)) = l.split_once("::").or_else(|| l.split_once(";;"))
            && !p.contains("{{")
            && !p.trim().is_empty()
            && !r.trim().is_empty()
        {
            v.push(Tarjeta {
                frente: html(p),
                reverso: html(r),
                ..Tarjeta::default()
            });
            continue;
        }
        if let Some(t) = de_huecos(l) {
            v.push(t);
        }
    }
    cerrar(&mut abierta, &mut v);
    v
}

fn quitar_prefijo<'a>(l: &'a str, prefijos: &[&str]) -> Option<&'a str> {
    prefijos.iter().find_map(|p| {
        let cabeza = l.get(..p.len())?;
        cabeza.eq_ignore_ascii_case(p).then(|| l[p.len()..].trim())
    })
}

/// Una linea con huecos `{{c1::texto}}` (o `{{c1::texto::pista}}`).
fn de_huecos(l: &str) -> Option<Tarjeta> {
    let mut frente = String::new();
    let mut reverso = String::new();
    let mut resto = l;
    let mut alguno = false;
    while let Some(i) = resto.find("{{c") {
        let Some(fin) = resto[i..].find("}}") else {
            break;
        };
        let dentro = &resto[i + 2..i + fin];
        let Some((_, cuerpo)) = dentro.split_once("::") else {
            break;
        };
        let (tapado, pista) = match cuerpo.split_once("::") {
            Some((t, p)) => (t, Some(p)),
            None => (cuerpo, None),
        };
        frente.push_str(&tal_cual(&resto[..i]));
        reverso.push_str(&tal_cual(&resto[..i]));
        frente.push_str(&match pista {
            Some(p) => format!("[{}]", tal_cual(p)),
            None => "[…]".to_string(),
        });
        reverso.push_str(&format!("<b>{}</b>", tal_cual(tapado)));
        resto = &resto[i + fin + 2..];
        alguno = true;
    }
    if !alguno {
        return None;
    }
    frente.push_str(&tal_cual(resto));
    reverso.push_str(&tal_cual(resto));
    Some(Tarjeta {
        frente,
        reverso,
        ..Tarjeta::default()
    })
}

/// El minuto de una marca: `12:34` o `1:02:03`.
pub fn minuto(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Lo mas que se pone detras de una tarjeta de clase: mas no se repasa.
pub const MAXIMO_REVERSO: usize = 700;

/// **Las tarjetas de una clase grabada**: una por captura. Delante la imagen
/// con su minuto y la pregunta; detras lo que se dijo desde esa captura
/// hasta la siguiente (`trozos` son los de la transcripcion, `(ms, texto)`).
/// Una captura sin nada dicho detras se queda con su minuto: la imagen ya
/// es lo que se repasa.
pub fn de_la_clase(
    titulo: &str,
    pregunta: &str,
    trozos: &[(i64, String)],
    capturas: Vec<(i64, Imagen)>,
) -> Vec<Tarjeta> {
    let mut capturas = capturas;
    capturas.sort_by_key(|(ms, _)| *ms);
    let tiempos: Vec<i64> = capturas.iter().map(|(ms, _)| *ms).collect();
    capturas
        .into_iter()
        .enumerate()
        .map(|(n, (ms, imagen))| {
            let hasta = tiempos.get(n + 1).copied().unwrap_or(i64::MAX);
            // Lo dicho en el tramo: desde el trozo que sonaba al hacer la
            // captura (empezo un poco antes) hasta la siguiente.
            let primero = trozos.iter().rposition(|(t, _)| *t <= ms).unwrap_or(0);
            let mut dicho = String::new();
            for (t, texto) in trozos.iter().skip(primero) {
                if *t >= hasta && !dicho.is_empty() {
                    break;
                }
                if !dicho.is_empty() {
                    dicho.push(' ');
                }
                dicho.push_str(texto.trim());
                if dicho.chars().count() > MAXIMO_REVERSO {
                    dicho = dicho.chars().take(MAXIMO_REVERSO).collect::<String>() + "…";
                    break;
                }
            }
            let cabeza = format!("{} · ⏱ {}", titulo.trim(), minuto(ms));
            Tarjeta {
                frente: format!(
                    "<img src=\"{}\"><br><small>{}</small><br>{}",
                    imagen.nombre,
                    html(&cabeza),
                    html(pregunta)
                ),
                reverso: if dicho.is_empty() {
                    html(&cabeza)
                } else {
                    format!("{}<br><small>{}</small>", html(&dicho), html(&cabeza))
                },
                etiquetas: Vec::new(),
                imagenes: vec![imagen],
            }
        })
        .collect()
}

/// **Una foto sola**: delante la imagen; detras su descripcion.
pub fn de_la_foto(imagen: Imagen, descripcion: &str, titulo: &str) -> Tarjeta {
    let detras = if descripcion.trim().is_empty() {
        titulo
    } else {
        descripcion
    };
    Tarjeta {
        frente: format!("<img src=\"{}\">", imagen.nombre),
        reverso: html(detras),
        etiquetas: Vec::new(),
        imagenes: vec![imagen],
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_nota_con_dos_puntos_dobles_da_una_tarjeta_por_linea() {
        let t = de_la_nota("Capital de Peru :: Lima\n- 2 + 2 ;; 4\nUn parrafo suelto.");
        assert_eq!(t.len(), 2);
        assert_eq!(
            (t[0].frente.as_str(), t[0].reverso.as_str()),
            ("Capital de Peru", "Lima")
        );
        assert_eq!(
            (t[1].frente.as_str(), t[1].reverso.as_str()),
            ("2 + 2", "4")
        );
    }

    #[test]
    fn pregunta_y_respuesta_en_varias_lineas() {
        let t = de_la_nota(
            "P: ¿Qué es la mitosis?\nR: División celular\nque da dos células\n\nQ: <b>?\nA: x & y",
        );
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].reverso, "División celular<br>que da dos células");
        // El HTML de la nota no se cuela: se escapa.
        assert_eq!(t[1].frente, "&lt;b&gt;?");
        assert_eq!(t[1].reverso, "x &amp; y");
        // Caso negativo: una pregunta sin respuesta no es tarjeta.
        assert!(de_la_nota("P: ¿sola?").is_empty());
    }

    #[test]
    fn los_huecos_de_anki_tapan_delante_y_se_ven_detras() {
        let t = de_la_nota("{{c1::Lima}} es la capital de {{c2::Perú::país}}");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].frente, "[…] es la capital de [país]");
        assert_eq!(t[0].reverso, "<b>Lima</b> es la capital de <b>Perú</b>");
    }

    #[test]
    fn una_clase_da_una_tarjeta_por_captura_con_lo_dicho_en_su_tramo() {
        let img = |n: &str| Imagen {
            nombre: n.into(),
            bytes: vec![1],
        };
        let trozos = vec![
            (0, "Hola.".to_string()),
            (10_000, "La celula.".to_string()),
            (20_000, "El nucleo.".to_string()),
            (40_000, "Las mitocondrias.".to_string()),
        ];
        let t = de_la_clase(
            "Biología",
            "¿Qué se explicó aquí?",
            &trozos,
            vec![(39_000, img("b.png")), (12_000, img("a.png"))],
        );
        assert_eq!(t.len(), 2);
        assert!(t[0].frente.contains("a.png") && t[0].frente.contains("0:12"));
        assert!(
            t[0].reverso.starts_with("La celula. El nucleo."),
            "{}",
            t[0].reverso
        );
        // Caso negativo: lo de la captura siguiente no se mete en esta.
        assert!(!t[0].reverso.contains("mitocondrias"));
        assert!(t[1].reverso.contains("Las mitocondrias."));
        assert_eq!(t[1].imagenes[0].nombre, "b.png");
    }

    #[test]
    fn las_etiquetas_no_llevan_espacios() {
        assert_eq!(etiqueta(" Clase de física "), "Clase_de_física");
    }
}
