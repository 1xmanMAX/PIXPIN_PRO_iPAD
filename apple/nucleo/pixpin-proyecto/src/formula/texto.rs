//! Funciones de texto.
//!
//! Las posiciones y los largos van en letras («caracteres»), no en bytes: la
//! `ñ` y la `é` son una sola.

use super::criterio::comodin;
use super::motor::{Ctx, Ev, Resultado, a_texto};
use super::{ErrorFormula, Valor, formato, numero_escrito};

pub(super) fn llamar(c: &mut Ctx, canon: &str, args: &[Ev]) -> Option<Resultado> {
    Some(match canon {
        "CONCATENAR" => concatenar(c, args, false),
        "CONCAT" => concatenar(c, args, true),
        "UNIRCADENAS" => unir(c, args),
        "IZQUIERDA" => lado(c, args, true),
        "DERECHA" => lado(c, args, false),
        "EXTRAE" => (|| {
            let t = c.texto(&args[0])?;
            let (ini, n) = (c.entero(&args[1])?, c.entero(&args[2])?);
            if ini < 1 || n < 0 {
                return Err(ErrorFormula::Sintaxis);
            }
            let s: String = t.chars().skip(ini as usize - 1).take(n as usize).collect();
            Ok(Valor::Texto(s))
        })(),
        "LARGO" => c
            .texto(&args[0])
            .map(|t| Valor::Numero(t.chars().count() as f64)),
        "MAYUSC" => c.texto(&args[0]).map(|t| Valor::Texto(t.to_uppercase())),
        "MINUSC" => c.texto(&args[0]).map(|t| Valor::Texto(t.to_lowercase())),
        "NOMPROPIO" => c.texto(&args[0]).map(|t| Valor::Texto(nom_propio(&t))),
        "ESPACIOS" => c.texto(&args[0]).map(|t| {
            // Solo el espacio de siempre, como Excel: los saltos de linea se
            // quedan.
            let palabras: Vec<&str> = t.split(' ').filter(|p| !p.is_empty()).collect();
            Valor::Texto(palabras.join(" "))
        }),
        "SUSTITUIR" => sustituir(c, args),
        "ENCONTRAR" => buscar(c, args, true),
        "HALLAR" => buscar(c, args, false),
        "TEXTO" => (|| {
            let v = c.valor_de(&args[0]);
            let f = c.texto(&args[1])?;
            formato::dar_formato(&v, &f).map(Valor::Texto)
        })(),
        "VALOR" => {
            let v = c.valor_de(&args[0]);
            match v {
                Valor::Numero(_) => Ok(v),
                Valor::Logico(_) => Err(ErrorFormula::Sintaxis),
                Valor::Texto(t) if t.trim().is_empty() => Ok(Valor::Numero(0.0)),
                Valor::Texto(t) => numero_escrito(&t)
                    .or_else(|| super::fecha::serie_de_texto(&t))
                    .map(Valor::Numero)
                    .ok_or(ErrorFormula::Sintaxis),
                Valor::Error(e) => Err(e),
            }
        }
        _ => return None,
    })
}

/// `CONCATENAR` solo admite valores sueltos; `CONCAT` tambien rangos, que
/// recorre por filas.
fn concatenar(c: &mut Ctx, args: &[Ev], rangos: bool) -> Resultado {
    let mut s = String::new();
    for a in args {
        match a {
            Ev::R(rg) if rangos => {
                let mut celdas = c.rala(*rg)?.to_vec();
                celdas.sort_by_key(|(r, _)| (r.fila, r.columna));
                for (_, v) in celdas {
                    s.push_str(&a_texto(&v)?);
                }
            }
            _ => s.push_str(&c.texto(a)?),
        }
    }
    Ok(Valor::Texto(s))
}

/// `UNIRCADENAS(separador; ignorar_vacios; textos...)`.
fn unir(c: &mut Ctx, args: &[Ev]) -> Resultado {
    let sep = c.texto(&args[0])?;
    let ignorar = c.logico(&args[1])?;
    let mut trozos: Vec<String> = Vec::new();
    for a in &args[2..] {
        match a {
            Ev::R(rg) => {
                // Con los vacios hay que verlos: cada hueco del rango pone su
                // separador. Se recorre hasta la ultima celda escrita; mas
                // alla todo son huecos finales y sobran.
                let dado = c.rala(*rg)?;
                let mapa: std::collections::HashMap<(u32, u32), Valor> = dado
                    .iter()
                    .map(|(r, v)| ((r.fila, r.columna), v.clone()))
                    .collect();
                let (ec, ef) = c.extension();
                let f2 = rg.f2.min(ef.saturating_sub(1));
                let c2 = rg.c2.min(ec.saturating_sub(1));
                if rg.f1 <= f2 && rg.c1 <= c2 {
                    for f in rg.f1..=f2 {
                        for k in rg.c1..=c2 {
                            let v = mapa
                                .get(&(f, k))
                                .cloned()
                                .unwrap_or(Valor::Texto(String::new()));
                            trozos.push(a_texto(&v)?);
                        }
                    }
                }
            }
            otro => {
                let t = c.texto(otro)?;
                trozos.push(t);
            }
        }
    }
    if ignorar {
        trozos.retain(|t| !t.is_empty());
    }
    Ok(Valor::Texto(trozos.join(&sep)))
}

fn lado(c: &mut Ctx, args: &[Ev], izquierda: bool) -> Resultado {
    let t = c.texto(&args[0])?;
    let n = match args.get(1) {
        Some(n) => c.entero(n)?,
        None => 1,
    };
    if n < 0 {
        return Err(ErrorFormula::Sintaxis);
    }
    let largo = t.chars().count();
    let n = (n as usize).min(largo);
    let s: String = if izquierda {
        t.chars().take(n).collect()
    } else {
        t.chars().skip(largo - n).collect()
    };
    Ok(Valor::Texto(s))
}

/// Mayuscula la primera letra de cada palabra y minuscula el resto; lo que no
/// es una letra corta la palabra (`o'neil` es `O'Neil`, como en Excel).
fn nom_propio(t: &str) -> String {
    let mut sale = String::with_capacity(t.len());
    let mut en_palabra = false;
    for ch in t.chars() {
        if ch.is_alphabetic() {
            if en_palabra {
                sale.extend(ch.to_lowercase());
            } else {
                sale.extend(ch.to_uppercase());
            }
            en_palabra = true;
        } else {
            sale.push(ch);
            en_palabra = false;
        }
    }
    sale
}

/// `SUSTITUIR(texto; viejo; nuevo; [cual])`: distingue mayusculas.
fn sustituir(c: &mut Ctx, args: &[Ev]) -> Resultado {
    let t = c.texto(&args[0])?;
    let viejo = c.texto(&args[1])?;
    let nuevo = c.texto(&args[2])?;
    let cual = match args.get(3) {
        Some(n) => {
            let n = c.entero(n)?;
            if n < 1 {
                return Err(ErrorFormula::Sintaxis);
            }
            Some(n as usize)
        }
        None => None,
    };
    if viejo.is_empty() {
        return Ok(Valor::Texto(t));
    }
    let mut sale = String::with_capacity(t.len());
    let mut resto = t.as_str();
    let mut visto = 0;
    while let Some(i) = resto.find(&viejo) {
        visto += 1;
        sale.push_str(&resto[..i]);
        if cual.is_none_or(|n| n == visto) {
            sale.push_str(&nuevo);
        } else {
            sale.push_str(&viejo);
        }
        resto = &resto[i + viejo.len()..];
    }
    sale.push_str(resto);
    Ok(Valor::Texto(sale))
}

/// `ENCONTRAR` (distingue mayusculas, sin comodines) y `HALLAR` (no distingue
/// y admite `*`, `?` y `~`). Dan la posicion desde 1, o `#¡VALOR!`.
fn buscar(c: &mut Ctx, args: &[Ev], exacto: bool) -> Resultado {
    let quiero = c.texto(&args[0])?;
    let en = c.texto(&args[1])?;
    let desde = match args.get(2) {
        Some(n) => c.entero(n)?,
        None => 1,
    };
    let largo = en.chars().count();
    if desde < 1 || desde as usize > largo.max(1) {
        return Err(ErrorFormula::Sintaxis);
    }
    let desde = desde as usize - 1;
    let (q, e): (Vec<char>, Vec<char>) = if exacto {
        (quiero.chars().collect(), en.chars().collect())
    } else {
        (
            quiero.to_lowercase().chars().collect(),
            en.to_lowercase().chars().collect(),
        )
    };
    if q.is_empty() {
        return Ok(Valor::Numero(desde as f64 + 1.0));
    }
    for i in desde..e.len() {
        let casa = if exacto {
            e[i..].starts_with(&q)
        } else {
            comodin(&q, &e[i..], false)
        };
        if casa {
            return Ok(Valor::Numero(i as f64 + 1.0));
        }
    }
    Err(ErrorFormula::Sintaxis)
}
