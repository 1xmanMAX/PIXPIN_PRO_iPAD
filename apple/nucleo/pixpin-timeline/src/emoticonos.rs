//! **Los emoticonos**, un solo modulo para toda la app (5-oct-2026).
//!
//! Lo usan el timeline (el circulo de cada dia de «Estado», lo que mas se
//! repitio en un mes, los titulos con emoticonos en color) y las tareas (sus
//! etiquetas son los emoticonos de su texto, `tareas::emoticonos`, que solo
//! llama aqui). Antes eran dos parsers que discrepaban (⌚ era emoticono en
//! tareas y no en el timeline); un revisor lo vio y se dejo uno, con el
//! criterio de tareas, que es el de Unicode (`Emoji_Presentation`): ⌚ ⏰ ☕
//! son emoticonos aunque no lleven el selector.
//!
//! Hay que sacar cada emoticono **entero**: 👨‍💻 son tres letras unidas por el
//! «pegamento» invisible (ZWJ), ❤️ es el corazon mas un selector de variante
//! y 🇵🇪 son dos letras regionales. Sin crates de Unicode a proposito: basta
//! con los bloques de pictogramas y las reglas de como se juntan. Lo de
//! fuera (flechas, ©, ™) solo cuenta si lleva detras el selector VS16, que
//! es como lo escribe el teclado de emoticonos. Un emoticono raro que se
//! escape se queda en el texto: nunca se pierde nada.
//!
//! Tambien vive aqui como se pintan **como los estados de WeChat** (el
//! usuario, 5-oct): un circulo pastel por emoticono, siempre del mismo tono
//! ([`tono_de`]), apilados ([`pila_de_estados`]).

/// Une dos pictogramas en uno (👨‍💻).
const ZWJ: char = '\u{200D}';
/// Pide la forma de emoticono (❤️) o la de texto.
const VS16: char = '\u{FE0F}';
const VS15: char = '\u{FE0E}';
/// El marco de tecla de `1️⃣`.
const TECLA: char = '\u{20E3}';

/// Los bloques donde todo es pictograma.
fn es_pictograma(c: char) -> bool {
    matches!(c as u32,
        0x1F000..=0x1FAFF   // cartas, fichas, simbolos y pictogramas, caras, transporte...
        | 0x2600..=0x27BF   // simbolos varios y dingbats: ☀ ✅ ❤ ✈
        | 0x231A..=0x231B   // ⌚ ⌛
        | 0x23E9..=0x23FA   // ⏩ ⏰ ⏳
        | 0x2B05..=0x2B07
        | 0x2B1B..=0x2B1C
        | 0x2B50
        | 0x2B55
        | 0x3030
        | 0x303D
        | 0x3297
        | 0x3299)
        // Las letras regionales van por pares (banderas), aparte.
        && !es_regional(c)
}

fn es_regional(c: char) -> bool {
    (0x1F1E6..=0x1F1FF).contains(&(c as u32))
}

fn es_tono_de_piel(c: char) -> bool {
    (0x1F3FB..=0x1F3FF).contains(&(c as u32))
}

/// Las letras de etiqueta (U+E0020..E007F) de las banderas de regiones.
fn es_etiqueta(c: char) -> bool {
    (0xE0020..=0xE007F).contains(&(c as u32))
}

/// Lo que se pega detras de un pictograma sin hacer otro.
fn modifica(c: char) -> bool {
    c == VS16 || c == VS15 || c == TECLA || es_tono_de_piel(c) || es_etiqueta(c)
}

/// Cuanto mide (en bytes) el emoticono que empieza en `s`, si empieza uno.
fn emoticono_al_principio(s: &str) -> Option<usize> {
    let mut it = s.char_indices().peekable();
    let (_, c) = it.next()?;
    let siguiente = it.peek().map(|&(_, c)| c);
    let mut fin = c.len_utf8();
    if es_regional(c) {
        // Una bandera son dos letras regionales; una suelta tambien se
        // dibuja como letra-emoticono, y se toma igual.
        if let Some(d) = siguiente.filter(|&d| es_regional(d)) {
            fin += d.len_utf8();
        }
        return Some(fin);
    }
    // `1️⃣`, `#️⃣`: un digito con el marco de tecla (el VS16 es opcional).
    if c.is_ascii_digit() || c == '#' || c == '*' {
        let resto = &s[fin..];
        let resto = resto.strip_prefix(VS16).unwrap_or(resto);
        return resto
            .starts_with(TECLA)
            .then(|| s.len() - resto.len() + TECLA.len_utf8());
    }
    if !es_pictograma(c) && siguiente != Some(VS16) {
        return None;
    }
    // Lo que lo modifica, y tras un ZWJ el pictograma que se une (con lo
    // suyo), una y otra vez: 👨‍👩‍👧‍👦 es uno solo.
    loop {
        let resto = &s[fin..];
        let mut cs = resto.chars();
        match cs.next() {
            Some(m) if modifica(m) => fin += m.len_utf8(),
            Some(ZWJ) => match cs.next() {
                Some(d) if es_pictograma(d) || es_regional(d) => {
                    fin += ZWJ.len_utf8() + d.len_utf8();
                }
                _ => break,
            },
            _ => break,
        }
    }
    Some(fin)
}

/// El texto partido en trozos de texto normal (`false`) y emoticonos
/// (`true`), en orden y sin perder nada: quien pinta dibuja los emoticonos
/// con la letra en color y lo demas con la suya (la negrita no los pinta en
/// color).
pub fn trozos(texto: &str) -> Vec<(&str, bool)> {
    let mut v = Vec::new();
    let mut desde = 0;
    let mut i = 0;
    while i < texto.len() {
        if let Some(n) = emoticono_al_principio(&texto[i..]) {
            if desde < i {
                v.push((&texto[desde..i], false));
            }
            v.push((&texto[i..i + n], true));
            i += n;
            desde = i;
        } else {
            i += texto[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    if desde < texto.len() {
        v.push((&texto[desde..], false));
    }
    v
}

/// Los emoticonos de `texto`, enteros y en orden (repetidos incluidos).
pub fn emoticonos(texto: &str) -> Vec<&str> {
    trozos(texto)
        .into_iter()
        .filter_map(|(t, e)| e.then_some(t))
        .collect()
}

/// El primer emoticono de `texto`, entero.
pub fn primer_emoticono(texto: &str) -> Option<&str> {
    trozos(texto).into_iter().find_map(|(t, e)| e.then_some(t))
}

/// La clave para contar: sin selectores, para que ❤ y ❤️ cuenten igual.
pub fn clave(e: &str) -> String {
    e.chars().filter(|c| *c != VS16 && *c != VS15).collect()
}

/// Saca los emoticonos de `texto`: devuelve el texto sin ellos (con los
/// blancos que dejan recogidos) y los emoticonos en el orden en que salen,
/// cada uno una vez. Son las etiquetas de una tarea.
pub fn emoticonos_de(texto: &str) -> (String, Vec<String>) {
    let mut limpio = String::with_capacity(texto.len());
    let mut emos: Vec<String> = Vec::new();
    for (t, e) in trozos(texto) {
        if e {
            if !emos.iter().any(|x| x == t) {
                emos.push(t.to_string());
            }
            // Donde estaba, un blanco: «a😀b» no se junta en «ab».
            limpio.push(' ');
        } else {
            limpio.push_str(t);
        }
    }
    let limpio = limpio.split_whitespace().collect::<Vec<_>>().join(" ");
    (limpio, emos)
}

// ------------------------------------------------- como estados de WeChat

/// Los fondos pastel de los circulos (el usuario, 5-oct: «implementa lo de
/// los emoticones como los estados de WeChat»): azul, rosa, verde, arena,
/// lila, menta, melocoton y orquidea. Suaves para que el emoticono, en
/// color, sea lo que se lee.
///
/// POR QUE estos (6-oct): los de antes eran grisaceos y sobre el fondo
/// oscuro los circulos parecian apagados. Estos son mas luminosos y ninguno
/// es gris: un circulo gris parecia deshabilitado.
pub const TONOS: [u32; 8] = [
    0xBFD7F0, 0xF7C6D6, 0xC4E8BE, 0xFFE3A3, 0xD9C8F5, 0xB6EADF, 0xFFD0B5, 0xF2C1E8,
];

/// El fondo de un emoticono (o de cualquier texto): siempre el mismo para
/// el mismo (en todas las tarjetas y en cada arranque), asi 🔥 se reconoce
/// de un vistazo. FNV-1a sobre sus bytes y no el `DefaultHasher`, que no
/// promete dar lo mismo de una version de Rust a otra.
pub fn tono_de(emo: &str) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in emo.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    TONOS[(h % TONOS.len() as u32) as usize]
}

/// Cuantos circulos se ven a lo sumo; los demas van en «+N».
pub const VISIBLES: usize = 3;

/// Los circulos de una pila de `n` emoticonos de diametro `d`, con el
/// primero a la izquierda en `x`: el centro (x) de los que se ven, del de
/// delante al de mas atras, con cuanto oscurece cada uno (0 el de delante),
/// y cuantos quedan para el «+N». Como en WeChat, el siguiente asoma
/// detras, corrido a la derecha y mas oscuro.
pub fn pila_de_estados(x: f32, d: f32, n: usize) -> (Vec<(f32, f32)>, usize) {
    let visibles = n.min(VISIBLES);
    let paso = d * 0.55;
    let v = (0..visibles)
        .map(|k| (x + d / 2.0 + k as f32 * paso, 0.18 * k as f32))
        .collect();
    (v, n - visibles)
}

/// Lo que ocupa a lo ancho la pila de `n` (sin el «+N»).
pub fn ancho_de_pila(d: f32, n: usize) -> f32 {
    match n.min(VISIBLES) {
        0 => 0.0,
        v => d + (v - 1) as f32 * d * 0.55,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    // ---- las del timeline

    #[test]
    fn saca_el_emoticono_entero_con_zwj_variante_y_bandera() {
        assert_eq!(primer_emoticono("hoy 👨‍💻 programando"), Some("👨‍💻"));
        assert_eq!(primer_emoticono("te quiero ❤️ mucho"), Some("❤️"));
        assert_eq!(primer_emoticono("Lima 🇵🇪!"), Some("🇵🇪"));
        assert_eq!(primer_emoticono("👍🏽 vale"), Some("👍🏽"));
        assert_eq!(primer_emoticono("👨‍👩‍👧‍👦"), Some("👨‍👩‍👧‍👦"));
        assert_eq!(primer_emoticono("1️⃣ primero"), Some("1️⃣"));
    }

    #[test]
    fn todos_en_orden() {
        assert_eq!(emoticonos("a😀b🎉c❤d"), ["😀", "🎉", "❤"]);
        assert_eq!(emoticonos("🇵🇪🇨🇱"), ["🇵🇪", "🇨🇱"]);
    }

    #[test]
    fn el_criterio_es_el_de_unicode_reloj_y_cafe_sin_selector_si_cuentan() {
        // Lo que discrepaba entre tareas y el timeline: ahora uno solo.
        assert_eq!(primer_emoticono("a las ⌚ 8"), Some("⌚"));
        assert_eq!(primer_emoticono("⏰ despertar"), Some("⏰"));
        assert_eq!(primer_emoticono("Café ☕"), Some("☕"));
    }

    #[test]
    fn caso_negativo_letras_con_tilde_signos_y_numeros_no_son_emoticonos() {
        assert_eq!(primer_emoticono("¿Qué tal? ¡Bien! Ñandú, árbol"), None);
        assert_eq!(primer_emoticono("a las 10:40 → casa © 2026 #1"), None);
        assert_eq!(primer_emoticono(""), None);
    }

    #[test]
    fn los_trozos_separan_emoticonos_sin_perder_nada() {
        let t = "Hola 👨‍💻 mundo❤️!";
        let v = trozos(t);
        assert_eq!(
            v,
            [("Hola ", false), ("👨‍💻", true), (" mundo", false), ("❤️", true), ("!", false)]
        );
        assert_eq!(v.iter().map(|(x, _)| *x).collect::<String>(), t);
        // Caso negativo: sin emoticonos, un solo trozo de texto.
        assert_eq!(trozos("¿Qué tal?"), [("¿Qué tal?", false)]);
        assert!(trozos("").is_empty());
    }

    #[test]
    fn el_corazon_con_y_sin_selector_es_la_misma_clave() {
        assert_eq!(clave("❤️"), clave("❤"));
        assert_ne!(clave("❤"), clave("💙"));
    }

    // ---- las de tareas (antes en `tareas::emoticonos`)

    #[test]
    fn el_mismo_emoticono_tiene_siempre_el_mismo_tono_pastel() {
        assert_eq!(tono_de("🔥"), tono_de("🔥"));
        assert!(TONOS.contains(&tono_de("👨‍💻")));
        let muestra = ["🔥", "❤️", "✅", "⭐", "📌", "🏗️", "🇵🇪", "👨‍💻", "🛒", "📞", "💡", "🎓"];
        let distintos: std::collections::HashSet<u32> = muestra.iter().map(|e| tono_de(e)).collect();
        assert!(distintos.len() >= 4, "{distintos:?}");
        // Caso negativo: el tono no depende de lo que se pidio antes.
        let a = tono_de("🔥");
        let _ = tono_de("✅");
        assert_eq!(tono_de("🔥"), a);
    }

    #[test]
    fn la_pila_de_estados_ensena_tres_y_el_resto_en_mas_n() {
        let (v, mas) = pila_de_estados(10.0, 40.0, 2);
        assert_eq!(mas, 0);
        assert_eq!(v, vec![(30.0, 0.0), (52.0, 0.18)]);
        assert_eq!(ancho_de_pila(40.0, 2), 62.0);
        let (v, mas) = pila_de_estados(0.0, 40.0, 5);
        assert_eq!((v.len(), mas), (3, 2));
        assert!(v[2].1 > v[1].1, "el de mas atras, mas oscuro");
        // Caso negativo: sin emoticonos no hay circulos ni «+N», ni ancho.
        assert_eq!(pila_de_estados(0.0, 40.0, 0), (vec![], 0));
        assert_eq!(ancho_de_pila(40.0, 0), 0.0);
    }

    #[test]
    fn saca_los_emoticonos_y_deja_el_texto_limpio() {
        let (t, e) = emoticonos_de("🔥 revisar la obra 🏗️ hoy");
        assert_eq!(t, "revisar la obra hoy");
        assert_eq!(e, ["🔥", "🏗️"]);
        let (t, e) = emoticonos_de("pan😀leche");
        assert_eq!((t.as_str(), e.len()), ("pan leche", 1));
        let (_, e) = emoticonos_de("⭐ uno ⭐ dos ✅");
        assert_eq!(e, ["⭐", "✅"]);
    }

    #[test]
    fn los_emoticonos_compuestos_salen_enteros() {
        let (t, e) = emoticonos_de("👨‍💻 programar ❤️ y 🇵🇪 👍🏽 1️⃣");
        assert_eq!(t, "programar y");
        assert_eq!(e, ["👨‍💻", "❤️", "🇵🇪", "👍🏽", "1️⃣"]);
        let (_, e) = emoticonos_de("👨‍👩‍👧‍👦");
        assert_eq!(e, ["👨‍👩‍👧‍👦"]);
        let (t, e) = emoticonos_de("↔️");
        assert_eq!((t.as_str(), e), ("", vec!["↔️".to_string()]));
    }

    #[test]
    fn caso_negativo_tildes_signos_y_numeros_no_son_etiquetas() {
        for s in [
            "¿Qué tal? ¡Año ñandú! cigüeña",
            "pagar 3 # de 20 * 2",
            "a → b © 2026",
            "[img 01] comprar «pan»",
        ] {
            let (t, e) = emoticonos_de(s);
            assert!(e.is_empty(), "{s}: {e:?}");
            assert_eq!(t, s.split_whitespace().collect::<Vec<_>>().join(" "));
        }
        // Un ZWJ suelto al final no se lleva nada mas.
        let (t, e) = emoticonos_de("ok 🙂\u{200D}");
        assert_eq!(e, ["🙂"]);
        assert_eq!(t, "ok \u{200D}");
    }
}
