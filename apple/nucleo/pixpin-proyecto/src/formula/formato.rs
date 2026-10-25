//! `TEXTO(valor; formato)`: los formatos basicos de Excel.
//!
//! Numeros: `0`, `0.00`, `#,##0`, `#,##0.00`, `0%`, `0.0%`, con texto fijo
//! alrededor (`"$"0.00`, `0 "kg"`, `$#,##0`). En el formato la coma agrupa
//! miles y el punto separa decimales (el orden de Excel en ingles); el
//! resultado sale igual, con coma de miles y punto decimal, porque `TEXTO`
//! devuelve un texto y no un numero que se muestre con la coma del usuario.
//! Solo se mira la primera seccion del formato (lo que va tras un `;`).
//!
//! Fechas: `dd/mm/aaaa`, `dd/mm/yyyy`, `yyyy-mm-dd`, `d/m/yy`, `mmmm`, `mmm`,
//! `dddd`, `ddd`, `hh:mm`, `hh:mm:ss`, `AM/PM`. Los nombres de meses y dias
//! salen en espanol. `aaaa`/`aa` y `yyyy`/`yy` valen igual.
//!
//! Lo que no se entiende (notacion cientifica, condiciones entre corchetes)
//! es `#¡VALOR!` en vez de un resultado que parezca bueno.

use super::{ErrorFormula, Valor, fecha, general};

pub(super) fn dar_formato(v: &Valor, formato: &str) -> Result<String, ErrorFormula> {
    let formato = formato.split(';').next().unwrap_or("");
    let n = match v {
        Valor::Error(e) => return Err(*e),
        Valor::Numero(n) => *n,
        Valor::Logico(b) => return Ok(if *b { "VERDADERO" } else { "FALSO" }.into()),
        // El texto pasa tal cual (y lo vacio, vacio); uno con forma de numero
        // se formatea como numero, como hace Excel.
        Valor::Texto(t) => match super::numero_escrito(t) {
            Some(n) if !t.is_empty() => n,
            _ => match fecha::serie_de_texto(t) {
                Some(n) => n,
                None => return Ok(t.clone()),
            },
        },
    };
    if !n.is_finite() {
        return Err(ErrorFormula::Numero);
    }
    if formato.contains(['[', 'E', 'e']) && !formato.contains('"') && tiene_cientifica(formato) {
        return Err(ErrorFormula::Sintaxis);
    }
    if formato.eq_ignore_ascii_case("general") || formato.is_empty() {
        return Ok(general(n));
    }
    let marcadores = formato.chars().any(|c| matches!(c, '0' | '#'));
    if !marcadores && es_fecha(formato) {
        return fecha_con_formato(n, formato);
    }
    Ok(numero_con_formato(n, formato))
}

fn tiene_cientifica(f: &str) -> bool {
    f.contains("E+") || f.contains("E-") || f.contains("e+") || f.contains("e-") || f.contains('[')
}

fn es_fecha(f: &str) -> bool {
    let mut entre_comillas = false;
    for c in f.chars() {
        if c == '"' {
            entre_comillas = !entre_comillas;
        } else if !entre_comillas
            && matches!(c.to_ascii_lowercase(), 'd' | 'm' | 'y' | 'a' | 'h' | 's')
        {
            return true;
        }
    }
    false
}

// ---- Numeros ----------------------------------------------------------

fn numero_con_formato(n: f64, formato: &str) -> String {
    let chars: Vec<char> = formato.chars().collect();
    // Trozo de marcadores: del primer `0 # . ,` al ultimo (fuera de comillas).
    let mut prefijo = String::new();
    let mut region = String::new();
    let mut sufijo = String::new();
    let mut porciento = false;
    let mut i = 0;
    let mut estado = 0; // 0 antes, 1 dentro, 2 despues
    while i < chars.len() {
        let c = chars[i];
        let literal: Option<String> = match c {
            '"' => {
                let fin = chars[i + 1..].iter().position(|x| *x == '"');
                let (txt, salto) = match fin {
                    Some(f) => (chars[i + 1..i + 1 + f].iter().collect(), f + 2),
                    None => (chars[i + 1..].iter().collect(), chars.len() - i),
                };
                i += salto;
                Some(txt)
            }
            '\\' | '_' => {
                // `\x` pone la x tal cual; `_x` deja un hueco del ancho de x.
                let t = chars.get(i + 1).map_or(String::new(), |x| {
                    if c == '_' { " ".into() } else { x.to_string() }
                });
                i += 2;
                Some(t)
            }
            '%' => {
                porciento = true;
                i += 1;
                Some("%".into())
            }
            '0' | '#' | '?' | '.' | ',' => None,
            otro => {
                i += 1;
                Some(otro.to_string())
            }
        };
        match literal {
            Some(t) => match estado {
                0 => prefijo.push_str(&t),
                _ => {
                    estado = 2;
                    sufijo.push_str(&t);
                }
            },
            None => {
                if estado == 2 {
                    // Un marcador despues de texto: se trata como texto.
                    sufijo.push(c);
                } else {
                    estado = 1;
                    region.push(c);
                }
                i += 1;
            }
        }
    }

    let (ent, dec) = match region.split_once('.') {
        Some((e, d)) => (e.to_string(), d.to_string()),
        None => (region.clone(), String::new()),
    };
    let miles = ent.contains(',');
    let min_enteros = ent.chars().filter(|c| *c == '0').count();
    let decimales = dec.chars().filter(|c| matches!(c, '0' | '#' | '?')).count();
    let min_decimales = dec.chars().filter(|c| *c == '0').count();

    let valor = if porciento { n * 100.0 } else { n };
    let redondeado = super::mates::redondear_a(valor, decimales as i32);
    let negativo = redondeado < 0.0;
    let cuerpo = format!("{:.*}", decimales, redondeado.abs());
    let (mut parte_ent, mut parte_dec) = match cuerpo.split_once('.') {
        Some((e, d)) => (e.to_string(), d.to_string()),
        None => (cuerpo.clone(), String::new()),
    };
    while parte_dec.len() > min_decimales && parte_dec.ends_with('0') {
        parte_dec.pop();
    }
    if min_enteros == 0 && parte_ent == "0" && (decimales > 0 || region.contains('#')) {
        parte_ent.clear();
    }
    while parte_ent.chars().count() < min_enteros {
        parte_ent.insert(0, '0');
    }
    if miles {
        parte_ent = con_miles(&parte_ent);
    }
    let mut s = String::new();
    if negativo {
        s.push('-');
    }
    s.push_str(&prefijo);
    s.push_str(&parte_ent);
    if !parte_dec.is_empty() {
        s.push('.');
        s.push_str(&parte_dec);
    }
    s.push_str(&sufijo);
    s
}

fn con_miles(entero: &str) -> String {
    let digitos: Vec<char> = entero.chars().collect();
    let mut s = String::new();
    for (i, c) in digitos.iter().enumerate() {
        if i > 0 && (digitos.len() - i) % 3 == 0 {
            s.push(',');
        }
        s.push(*c);
    }
    s
}

// ---- Fechas -----------------------------------------------------------

#[derive(Debug)]
enum Trozo {
    A(usize),
    M(usize),
    D(usize),
    H(usize),
    S(usize),
    Min(usize),
    AmPm,
    Lit(String),
}

const MESES: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];
const DIAS: [&str; 7] = [
    "domingo",
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
];

fn fecha_con_formato(n: f64, formato: &str) -> Result<String, ErrorFormula> {
    if !(0.0..=2_958_465.999_999).contains(&n) {
        return Err(ErrorFormula::Numero);
    }
    let chars: Vec<char> = formato.chars().collect();
    let mut trozos: Vec<Trozo> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let bajo = c.to_ascii_lowercase();
        let resto: String = chars[i..].iter().collect::<String>().to_uppercase();
        if resto.starts_with("AM/PM") {
            trozos.push(Trozo::AmPm);
            i += 5;
            continue;
        }
        let corrida = chars[i..]
            .iter()
            .take_while(|x| x.to_ascii_lowercase() == bajo)
            .count();
        match bajo {
            'y' | 'a' => trozos.push(Trozo::A(corrida)),
            'm' => trozos.push(Trozo::M(corrida)),
            'd' => trozos.push(Trozo::D(corrida)),
            'h' => trozos.push(Trozo::H(corrida)),
            's' => trozos.push(Trozo::S(corrida)),
            _ => {
                if c == '"' {
                    let fin = chars[i + 1..].iter().position(|x| *x == '"');
                    let (txt, salto): (String, usize) = match fin {
                        Some(f) => (chars[i + 1..i + 1 + f].iter().collect(), f + 2),
                        None => (chars[i + 1..].iter().collect(), chars.len() - i),
                    };
                    trozos.push(Trozo::Lit(txt));
                    i += salto;
                } else if c == '\\' {
                    if let Some(x) = chars.get(i + 1) {
                        trozos.push(Trozo::Lit(x.to_string()));
                    }
                    i += 2;
                } else {
                    trozos.push(Trozo::Lit(c.to_string()));
                    i += 1;
                }
                continue;
            }
        }
        i += corrida;
    }
    // «mm» es minutos si va pegado a una hora o a unos segundos.
    for k in 0..trozos.len() {
        if let Trozo::M(n) = trozos[k] {
            let antes = trozos[..k]
                .iter()
                .rev()
                .find(|t| !matches!(t, Trozo::Lit(_)));
            let despues = trozos[k + 1..].iter().find(|t| !matches!(t, Trozo::Lit(_)));
            if matches!(antes, Some(Trozo::H(_))) || matches!(despues, Some(Trozo::S(_))) {
                trozos[k] = Trozo::Min(n);
            }
        }
    }

    let mut dia_entero = n.floor() as i64;
    let mut segundos = ((n - n.floor()) * 86_400.0).round() as i64;
    if segundos >= 86_400 {
        segundos -= 86_400;
        dia_entero += 1;
    }
    let (a, m, d) = fecha::desde_serie(dia_entero)?;
    let semana = (dia_entero - 1).rem_euclid(7) as usize;
    let (hh, mm, ss) = (segundos / 3600, segundos / 60 % 60, segundos % 60);
    let con_ampm = trozos.iter().any(|t| matches!(t, Trozo::AmPm));

    let mut s = String::new();
    for t in &trozos {
        match t {
            Trozo::A(n) => {
                if *n <= 2 {
                    s.push_str(&format!("{:02}", a % 100));
                } else {
                    s.push_str(&format!("{a:04}"));
                }
            }
            Trozo::M(n) => match n {
                1 => s.push_str(&m.to_string()),
                2 => s.push_str(&format!("{m:02}")),
                3 => s.push_str(&abreviar(MESES[(m - 1) as usize])),
                _ => s.push_str(MESES[(m - 1) as usize]),
            },
            Trozo::D(n) => match n {
                1 => s.push_str(&d.to_string()),
                2 => s.push_str(&format!("{d:02}")),
                3 => s.push_str(&abreviar(DIAS[semana])),
                _ => s.push_str(DIAS[semana]),
            },
            Trozo::H(n) => {
                let h = if con_ampm { (hh + 11) % 12 + 1 } else { hh };
                if *n >= 2 {
                    s.push_str(&format!("{h:02}"));
                } else {
                    s.push_str(&h.to_string());
                }
            }
            Trozo::Min(n) => {
                if *n >= 2 {
                    s.push_str(&format!("{mm:02}"));
                } else {
                    s.push_str(&mm.to_string());
                }
            }
            Trozo::S(n) => {
                if *n >= 2 {
                    s.push_str(&format!("{ss:02}"));
                } else {
                    s.push_str(&ss.to_string());
                }
            }
            Trozo::AmPm => s.push_str(if hh < 12 { "AM" } else { "PM" }),
            Trozo::Lit(t) => s.push_str(t),
        }
    }
    Ok(s)
}

/// «octubre» -> «oct», «miércoles» -> «mié».
fn abreviar(nombre: &str) -> String {
    nombre.chars().take(3).collect()
}
