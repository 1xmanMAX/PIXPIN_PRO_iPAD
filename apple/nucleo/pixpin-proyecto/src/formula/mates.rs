//! Funciones logicas, matematicas y de recuento.

use super::motor::{Ctx, Ev, Resultado, es_vacio, potencia};
use super::{ErrorFormula, Valor};

pub(super) fn llamar(c: &mut Ctx, canon: &str, args: &[Ev]) -> Option<Resultado> {
    Some(match canon {
        "VERDADERO" => Ok(Valor::Logico(true)),
        "FALSO" => Ok(Valor::Logico(false)),
        "Y" | "O" => logica(c, canon == "Y", args),
        "NO" => c.logico(&args[0]).map(|b| Valor::Logico(!b)),

        "SUMA" => c.numeros(args).map(|n| Valor::Numero(n.iter().sum())),
        "CONTAR" => c.numeros(args).map(|n| Valor::Numero(n.len() as f64)),
        "PROMEDIO" => c.numeros(args).and_then(|n| {
            if n.is_empty() {
                // Promediar tres celdas vacias no es cero: no hay nada que
                // promediar, y decir cero seria inventarse un dato.
                return Err(ErrorFormula::DivisionPorCero);
            }
            Ok(Valor::Numero(n.iter().sum::<f64>() / n.len() as f64))
        }),
        // Se parte del primero y no de cero: una columna de numeros negativos
        // tiene su maximo en el menos pequeno, no en un cero que no escribio
        // nadie. Sin numeros si valen cero, como en cualquier hoja: es el
        // hueco de una columna aun sin rellenar, no un error.
        "MIN" => c.numeros(args).map(|n| Valor::Numero(extremo(&n, f64::min))),
        "MAX" => c.numeros(args).map(|n| Valor::Numero(extremo(&n, f64::max))),
        "PRODUCTO" => c.numeros(args).map(|n| {
            if n.is_empty() {
                Valor::Numero(0.0)
            } else {
                Valor::Numero(n.iter().product())
            }
        }),
        "MEDIANA" => c.numeros(args).and_then(|mut n| {
            if n.is_empty() {
                return Err(ErrorFormula::Numero);
            }
            n.sort_by(|a, b| a.total_cmp(b));
            let m = n.len() / 2;
            Ok(Valor::Numero(if n.len() % 2 == 1 {
                n[m]
            } else {
                (n[m - 1] + n[m]) / 2.0
            }))
        }),

        "REDONDEAR" => redondear(c, args, Modo::Cerca),
        "REDONDEAR.MAS" => redondear(c, args, Modo::Lejos),
        "REDONDEAR.MENOS" => redondear(c, args, Modo::Cero),
        "ENTERO" => c.num(&args[0]).map(|n| Valor::Numero(n.floor())),
        "ABS" => c.num(&args[0]).map(|n| Valor::Numero(n.abs())),
        "RESIDUO" => (|| {
            let (n, d) = (c.num(&args[0])?, c.num(&args[1])?);
            if d == 0.0 {
                return Err(ErrorFormula::DivisionPorCero);
            }
            // Con el signo del divisor, como Excel: RESIDUO(-3;2) es 1.
            Ok(Valor::Numero(n - d * (n / d).floor()))
        })(),
        "POTENCIA" => (|| {
            let (a, b) = (c.num(&args[0])?, c.num(&args[1])?);
            potencia(a, b).map(Valor::Numero)
        })(),
        "RAIZ" => c.num(&args[0]).and_then(|n| {
            if n < 0.0 {
                Err(ErrorFormula::Numero)
            } else {
                Ok(Valor::Numero(n.sqrt()))
            }
        }),
        "PI" => Ok(Valor::Numero(std::f64::consts::PI)),
        "ALEATORIO" => Ok(Valor::Numero(c.azar())),
        "ALEATORIO.ENTRE" => (|| {
            let (a, b) = (c.num(&args[0])?.ceil(), c.num(&args[1])?.floor());
            if a > b || !a.is_finite() || !b.is_finite() {
                return Err(ErrorFormula::Numero);
            }
            Ok(Valor::Numero((a + (c.azar() * (b - a + 1.0)).floor()).min(b)))
        })(),

        "CONTARA" => contara(c, args),
        "CONTAR.BLANCO" => contar_blanco(c, &args[0]),
        _ => return None,
    })
}

/// Y / O: todos los argumentos se leen (un error en cualquiera sube, como en
/// Excel). Los textos y las celdas vacias de un rango no cuentan; un texto
/// suelto que no es verdadero ni falso es `#¡VALOR!`. Sin ningun logico,
/// `#¡VALOR!`.
fn logica(c: &mut Ctx, y: bool, args: &[Ev]) -> Resultado {
    let mut hay = false;
    let mut resultado = y;
    for a in args {
        let valores: Vec<Valor> = match a {
            Ev::R(rg) => c.rala(*rg)?.iter().map(|(_, v)| v.clone()).collect(),
            Ev::C(v) => vec![v.clone()],
            Ev::V(v) => vec![v.clone()],
        };
        let directo = !matches!(a, Ev::R(_));
        for v in valores {
            let b = match &v {
                Valor::Error(e) => return Err(*e),
                Valor::Numero(n) => *n != 0.0,
                Valor::Logico(b) => *b,
                // Dentro de un rango o una celda el texto se ignora.
                Valor::Texto(_) if !directo || matches!(a, Ev::C(_)) => continue,
                Valor::Texto(_) => super::motor::a_logico(&v)?,
            };
            hay = true;
            resultado = if y { resultado && b } else { resultado || b };
        }
    }
    if !hay {
        return Err(ErrorFormula::Sintaxis);
    }
    Ok(Valor::Logico(resultado))
}

/// El mayor o el menor de unos numeros; cero si no hay ninguno.
fn extremo(numeros: &[f64], cual: fn(f64, f64) -> f64) -> f64 {
    match numeros.split_first() {
        Some((primero, resto)) => resto.iter().copied().fold(*primero, cual),
        None => 0.0,
    }
}

#[derive(Clone, Copy)]
enum Modo {
    /// Al mas cercano; la mitad se aleja del cero.
    Cerca,
    /// Siempre alejandose del cero.
    Lejos,
    /// Siempre hacia el cero.
    Cero,
}

/// `REDONDEAR` y hermanas. Los numeros de una hoja son decimales que el
/// ordenador guarda en binario: `2.675*100` es `267.49999999999994`, y
/// redondearlo tal cual daria 2.67 donde Excel (y cualquiera) dice 2.68. Por
/// eso antes de redondear se ajusta lo que esta a una milmillonesima de un
/// entero o de una mitad.
fn redondear(c: &mut Ctx, args: &[Ev], modo: Modo) -> Resultado {
    let n = c.num(&args[0])?;
    let d = c.entero(&args[1])?.clamp(-400, 400) as i32;
    let r = redondear_numero(n, d, modo);
    if r.is_finite() {
        Ok(Valor::Numero(r))
    } else {
        Err(ErrorFormula::Numero)
    }
}

pub(super) fn redondear_a(n: f64, decimales: i32) -> f64 {
    redondear_numero(n, decimales, Modo::Cerca)
}

fn redondear_numero(n: f64, d: i32, modo: Modo) -> f64 {
    if n == 0.0 || !n.is_finite() {
        return n;
    }
    let (x, deshacer) = if d >= 0 {
        let f = 10f64.powi(d.min(300));
        (n * f, Escala::Mult(f))
    } else {
        let f = 10f64.powi((-d).min(300));
        (n / f, Escala::Div(f))
    };
    if !x.is_finite() {
        return n;
    }
    let a = x.abs();
    // Si ya esta a un suspiro de un entero, es ese entero.
    let entero = a.round();
    let a = if (a - entero).abs() <= 1e-9 * a.max(1.0) {
        entero
    } else {
        a
    };
    let r = match modo {
        Modo::Cerca => {
            // Y a un suspiro de una mitad, es esa mitad.
            let doble = a * 2.0;
            let a = if (doble - doble.round()).abs() <= 1e-9 * doble.max(1.0) {
                doble.round() / 2.0
            } else {
                a
            };
            a.round()
        }
        Modo::Lejos => a.ceil(),
        Modo::Cero => a.floor(),
    };
    let r = r.copysign(n);
    match deshacer {
        Escala::Mult(f) => r / f,
        Escala::Div(f) => r * f,
    }
}

enum Escala {
    Mult(f64),
    Div(f64),
}

/// `CONTARA`: lo que no esta vacio, errores incluidos. Un ciclo, en cambio,
/// sube: el rango no se pudo leer.
fn contara(c: &mut Ctx, args: &[Ev]) -> Resultado {
    let mut cuenta = 0usize;
    for a in args {
        match a {
            Ev::V(_) => cuenta += 1,
            Ev::C(v) => cuenta += !es_vacio(v) as usize,
            Ev::R(rg) => {
                cuenta += c.rala(*rg)?.iter().filter(|(_, v)| !es_vacio(v)).count();
            }
        }
    }
    Ok(Valor::Numero(cuenta as f64))
}

/// `CONTAR.BLANCO`: las celdas del rango (todas, escritas o no) menos las
/// que tienen algo. Un texto vacio devuelto por una formula tambien es blanco.
fn contar_blanco(c: &mut Ctx, a: &Ev) -> Resultado {
    match a {
        Ev::R(rg) => {
            let total = rg.nfilas() * rg.ncols();
            let llenas = c.rala(*rg)?.iter().filter(|(_, v)| !es_vacio(v)).count() as u64;
            Ok(Valor::Numero((total - llenas) as f64))
        }
        Ev::C(v) | Ev::V(v) => Ok(Valor::Numero(es_vacio(v) as u8 as f64)),
    }
}
