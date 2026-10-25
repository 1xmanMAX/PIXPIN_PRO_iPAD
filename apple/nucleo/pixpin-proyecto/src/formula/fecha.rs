//! Fechas con el numero de serie de Excel (sistema 1900).
//!
//! El 1 de enero de 1900 es el 1. Excel cree que 1900 fue bisiesto (hereda el
//! error de Lotus 1-2-3): el 29 de febrero de 1900 es el 60, que no existio. Se
//! respeta igual para que un libro de Excel y esta hoja digan lo mismo desde
//! el 1 de marzo de 1900 (61) en adelante.

use super::motor::{Ctx, Ev, Resultado};
use super::{ErrorFormula, Valor};

pub(super) fn llamar(c: &mut Ctx, canon: &str, args: &[Ev]) -> Option<Resultado> {
    Some(match canon {
        // El dia entero, sin hora.
        "HOY" => Ok(Valor::Numero(c.ahora().floor())),
        "AHORA" => Ok(Valor::Numero(c.ahora())),
        "FECHA" => (|| {
            let (a, m, d) = (c.num(&args[0])?, c.num(&args[1])?, c.num(&args[2])?);
            fecha(a, m, d).map(Valor::Numero)
        })(),
        "AÑO" => parte(c, &args[0], |(a, _, _)| a as f64),
        "MES" => parte(c, &args[0], |(_, m, _)| m as f64),
        "DIA" => parte(c, &args[0], |(_, _, d)| d as f64),
        "DIAS" => (|| {
            let (fin, ini) = (serie(c, &args[0])?, serie(c, &args[1])?);
            Ok(Valor::Numero(fin.floor() - ini.floor()))
        })(),
        "DIASEM" => (|| {
            let s = serie(c, &args[0])?;
            let tipo = match args.get(1) {
                Some(t) => c.entero(t)?,
                None => 1,
            };
            // 0 = domingo, tal como cae el 1 de enero de 1900 en Excel.
            let d = (s.floor() as i64 - 1).rem_euclid(7);
            let n = match tipo {
                1 | 17 => d + 1,
                2 | 11 => (d + 6) % 7 + 1,
                3 => (d + 6) % 7,
                _ => return Err(ErrorFormula::Numero),
            };
            Ok(Valor::Numero(n as f64))
        })(),
        _ => return None,
    })
}

fn parte(c: &mut Ctx, e: &Ev, f: fn((i64, i64, i64)) -> f64) -> Resultado {
    let s = serie(c, e)?;
    Ok(Valor::Numero(f(desde_serie(s.floor() as i64)?)))
}

/// El numero de serie de un argumento: un numero, o un texto con forma de
/// fecha. Un negativo no es fecha.
fn serie(c: &mut Ctx, e: &Ev) -> Result<f64, ErrorFormula> {
    let v = c.valor_de(e);
    let s = match v {
        Valor::Numero(n) => n,
        Valor::Logico(b) => b as u8 as f64,
        Valor::Texto(t) if t.is_empty() => 0.0,
        Valor::Texto(t) => serie_de_texto(&t).ok_or(ErrorFormula::Sintaxis)?,
        Valor::Error(e) => return Err(e),
    };
    if s < 0.0 || !s.is_finite() || s > 2_958_465.0 {
        return Err(ErrorFormula::Numero);
    }
    Ok(s)
}

/// Dias desde 1970-01-01 de una fecha del calendario gregoriano.
fn dias_civiles(a: i64, m: i64, d: i64) -> i64 {
    let a = if m <= 2 { a - 1 } else { a };
    let era = a.div_euclid(400);
    let yoe = a.rem_euclid(400);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Lo inverso: (anio, mes, dia) de los dias desde 1970.
fn civil_de_dias(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let a = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { a + 1 } else { a }, m, d)
}

/// 1899-12-30 en dias desde 1970: el cero de los seriales posteriores al
/// 28 de febrero de 1900.
const BASE: i64 = -25_569;

/// (anio, mes, dia) de un serial entero.
pub(super) fn desde_serie(s: i64) -> Result<(i64, i64, i64), ErrorFormula> {
    match s {
        i64::MIN..=-1 => Err(ErrorFormula::Numero),
        0 => Ok((1900, 1, 0)),
        // Los dias antes del bisiesto inventado van desplazados en uno.
        1..=59 => Ok(civil_de_dias(BASE + s + 1)),
        60 => Ok((1900, 2, 29)),
        _ => Ok(civil_de_dias(BASE + s)),
    }
}

/// El serial de una fecha ya normalizada.
pub(super) fn a_serie(a: i64, m: i64, d: i64) -> i64 {
    let dias = dias_civiles(a, m, d) - BASE;
    // Antes del 1 de marzo, Excel va uno por detras: cuenta el 29 de febrero.
    if dias < 61 {
        dias - 1
    } else {
        dias
    }
}

/// `FECHA(a; m; d)`: los meses y los dias que se pasan o faltan se reparten
/// (el mes 14 es febrero del ano siguiente, el dia 0 es el ultimo del mes
/// anterior). Un ano de 0 a 1899 se suma a 1900.
fn fecha(a: f64, m: f64, d: f64) -> Result<f64, ErrorFormula> {
    let (mut a, m, d) = (a.trunc(), m.trunc(), d.trunc());
    if !(0.0..10000.0).contains(&a) {
        return Err(ErrorFormula::Numero);
    }
    if a < 1900.0 {
        a += 1900.0;
    }
    if m.abs() > 1e6 || d.abs() > 1e7 {
        return Err(ErrorFormula::Numero);
    }
    let (a, m, d) = (a as i64, m as i64, d as i64);
    let total = a * 12 + (m - 1);
    let (a, m) = (total.div_euclid(12), total.rem_euclid(12) + 1);
    // El 29 de febrero de 1900 no existio, pero Excel lo escribe: es el 60.
    let s = if (a, m, d) == (1900, 2, 29) {
        60
    } else {
        let dias = dias_civiles(a, m, 1) + (d - 1) - BASE;
        if dias < 61 { dias - 1 } else { dias }
    };
    if !(0..=2_958_465).contains(&s) {
        return Err(ErrorFormula::Numero);
    }
    Ok(s as f64)
}

/// «2026-10-11», «11/10/2026» (dia primero, como en espanol) o «11-10-2026».
pub(super) fn serie_de_texto(t: &str) -> Option<f64> {
    let t = t.trim();
    let partes: Vec<&str> = t.split(['/', '-', '.']).collect();
    if partes.len() != 3 || partes.iter().any(|p| p.is_empty() || p.len() > 4) {
        return None;
    }
    let n: Vec<i64> = partes
        .iter()
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let (a, m, d) = if partes[0].len() == 4 {
        (n[0], n[1], n[2])
    } else if partes[2].len() == 4 {
        (n[2], n[1], n[0])
    } else {
        return None;
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || !(1900..=9999).contains(&a) {
        return None;
    }
    // Que el dia exista en ese mes: el 31 de abril no es fecha.
    let s = a_serie(a, m, d);
    (desde_serie(s).ok()? == (a, m, d)).then_some(s as f64)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn seriales_conocidos_de_excel() {
        assert_eq!(a_serie(1900, 1, 1), 1);
        assert_eq!(a_serie(1900, 2, 28), 59);
        assert_eq!(a_serie(1900, 3, 1), 61);
        assert_eq!(a_serie(2000, 1, 1), 36526);
        assert_eq!(a_serie(2026, 10, 11), 46306);
        assert_eq!(desde_serie(60), Ok((1900, 2, 29)));
        assert_eq!(desde_serie(36526), Ok((2000, 1, 1)));
        assert_eq!(desde_serie(1), Ok((1900, 1, 1)));
        assert_eq!(desde_serie(-1), Err(ErrorFormula::Numero));
    }

    #[test]
    fn ida_y_vuelta_de_muchos_dias() {
        for s in (61..80_000).step_by(37) {
            let (a, m, d) = desde_serie(s).unwrap();
            assert_eq!(a_serie(a, m, d), s);
        }
    }

    #[test]
    fn texto_con_forma_de_fecha() {
        assert_eq!(serie_de_texto("2000-01-01"), Some(36526.0));
        assert_eq!(serie_de_texto("1/1/2000"), Some(36526.0));
        assert_eq!(serie_de_texto("31/04/2026"), None, "abril no tiene 31");
        assert_eq!(serie_de_texto("hola"), None);
        assert_eq!(serie_de_texto("1/2/26"), None, "el ano va con cuatro cifras");
    }
}
