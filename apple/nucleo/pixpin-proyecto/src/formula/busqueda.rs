//! Funciones de busqueda: `BUSCARV`, `BUSCARH`, `INDICE`, `COINCIDIR`,
//! `BUSCARX`.
//!
//! Se miran solo las celdas escritas del rango: una columna de mil filas con
//! veinte datos se recorre en veinte pasos. La posicion de cada una se saca de
//! su fila (o columna), asi que los huecos cuentan para `COINCIDIR` e `INDICE`
//! aunque no se visiten.
//!
//! **Coincidencia aproximada.** Excel hace una busqueda binaria y con la
//! columna desordenada devuelve lo que sale. Aqui se recorre en orden y se
//! para en el primer valor mayor (menor, si va de mayor a menor): con la
//! columna ordenada da lo mismo, con la columna desordenada da un resultado
//! mas previsible. Solo se comparan valores del mismo tipo (numero con
//! numero, texto con texto).

use std::cmp::Ordering;

use super::criterio::{comodin, tiene_comodines};
use super::motor::{Ctx, Ev, Resultado, Rg, mismo_numero, ordenar};
use super::{ErrorFormula, Valor};
use crate::tabla::Ref;

pub(super) fn llamar(c: &mut Ctx, canon: &str, args: &[Ev]) -> Option<Resultado> {
    Some(match canon {
        "BUSCARV" => buscar_vh(c, args, true),
        "BUSCARH" => buscar_vh(c, args, false),
        "INDICE" => indice(c, args),
        "COINCIDIR" => coincidir(c, args),
        "BUSCARX" => buscarx(c, args),
        _ => return None,
    })
}

/// Orden solo entre cosas del mismo tipo; `None` si no se pueden comparar.
fn orden_del_mismo_tipo(a: &Valor, b: &Valor) -> Option<Ordering> {
    match (a, b) {
        (Valor::Numero(_), Valor::Numero(_))
        | (Valor::Logico(_), Valor::Logico(_))
        | (Valor::Texto(_), Valor::Texto(_)) => Some(ordenar(a, b)),
        _ => None,
    }
}

/// Si la celda es lo buscado: igual, y con comodines si se busca un texto
/// que los lleva y `comodines` lo permite.
fn es_igual(buscado: &Valor, celda: &Valor, comodines: bool) -> bool {
    match (buscado, celda) {
        (Valor::Numero(a), Valor::Numero(b)) => mismo_numero(*a, *b),
        (Valor::Logico(a), Valor::Logico(b)) => a == b,
        (Valor::Texto(a), Valor::Texto(b)) => {
            if comodines && tiene_comodines(a) {
                let p: Vec<char> = a.to_lowercase().chars().collect();
                let t: Vec<char> = b.to_lowercase().chars().collect();
                comodin(&p, &t, true)
            } else if a.is_ascii() && b.is_ascii() {
                a.eq_ignore_ascii_case(b)
            } else {
                a.to_lowercase() == b.to_lowercase()
            }
        }
        _ => false,
    }
}

/// Una celda suelta tambien es una matriz, de una celda.
enum Matriz {
    Rango(Rg),
    Una(Valor),
}

impl Matriz {
    fn de(e: &Ev) -> Matriz {
        match e {
            Ev::R(rg) => Matriz::Rango(*rg),
            Ev::C(v) | Ev::V(v) => Matriz::Una(v.clone()),
        }
    }

    fn dim(&self) -> (u64, u64) {
        match self {
            Matriz::Rango(rg) => (rg.nfilas(), rg.ncols()),
            Matriz::Una(_) => (1, 1),
        }
    }

    /// El valor en (fila, columna) relativas.
    fn en(&self, c: &mut Ctx, fila: u64, col: u64) -> Valor {
        match self {
            Matriz::Rango(rg) => c.celda(Ref {
                columna: (rg.c1 as u64 + col) as u32,
                fila: (rg.f1 as u64 + fila) as u32,
            }),
            Matriz::Una(v) => v.clone(),
        }
    }

    /// Las celdas escritas de una linea (fila o columna) con su posicion
    /// desde cero.
    fn linea(
        &self,
        c: &mut Ctx,
        vertical: bool,
        otra: u64,
    ) -> Result<Vec<(u64, Valor)>, ErrorFormula> {
        match self {
            Matriz::Una(v) => Ok(vec![(0, v.clone())]),
            Matriz::Rango(rg) => {
                let sub = if vertical {
                    Rg {
                        c1: rg.c1 + otra as u32,
                        c2: rg.c1 + otra as u32,
                        ..*rg
                    }
                } else {
                    Rg {
                        f1: rg.f1 + otra as u32,
                        f2: rg.f1 + otra as u32,
                        ..*rg
                    }
                };
                Ok(c.rala(sub)?
                    .iter()
                    .map(|(r, v)| {
                        let pos = if vertical {
                            r.fila - rg.f1
                        } else {
                            r.columna - rg.c1
                        };
                        (pos as u64, v.clone())
                    })
                    .collect())
            }
        }
    }
}

fn no_disponible() -> Resultado {
    Err(ErrorFormula::NoDisponible)
}

/// `BUSCARV(buscado; tabla; columna; [aproximado])` y su version horizontal.
fn buscar_vh(c: &mut Ctx, args: &[Ev], vertical: bool) -> Resultado {
    let buscado = c.valor_de(&args[0]);
    let m = Matriz::de(&args[1]);
    let n = c.entero(&args[2])?;
    let aprox = match args.get(3) {
        Some(a) => c.logico(a)?,
        None => true,
    };
    let (filas, cols) = m.dim();
    if n < 1 {
        return Err(ErrorFormula::Sintaxis);
    }
    let a_lo_largo = if vertical { cols } else { filas };
    if n as u64 > a_lo_largo {
        return Err(ErrorFormula::Referencia);
    }
    let celdas = m.linea(c, vertical, 0)?;
    let pos = if aprox {
        ultimo_menor_o_igual(&buscado, &celdas)
    } else {
        celdas
            .iter()
            .find(|(_, v)| es_igual(&buscado, v, true))
            .map(|(p, _)| *p)
    };
    let Some(pos) = pos else {
        return no_disponible();
    };
    let (f, k) = if vertical {
        (pos, n as u64 - 1)
    } else {
        (n as u64 - 1, pos)
    };
    Ok(m.en(c, f, k))
}

/// La posicion del ultimo valor que no pasa de lo buscado, parando en el
/// primero que se pasa.
fn ultimo_menor_o_igual(buscado: &Valor, celdas: &[(u64, Valor)]) -> Option<u64> {
    let mut mejor = None;
    for (p, v) in celdas {
        match orden_del_mismo_tipo(v, buscado) {
            Some(Ordering::Greater) => break,
            Some(_) => mejor = Some(*p),
            None => {}
        }
    }
    mejor
}

fn indice(c: &mut Ctx, args: &[Ev]) -> Resultado {
    let m = Matriz::de(&args[0]);
    let (filas, cols) = m.dim();
    let a = c.entero(&args[1])?;
    let (f, k) = match args.get(2) {
        Some(b) => (a, c.entero(b)?),
        // Sin columna: en una columna manda la fila, en una fila la columna.
        None if cols == 1 => (a, 1),
        None if filas == 1 => (1, a),
        // Devolver una fila entera no cabe en una celda.
        None => return Err(ErrorFormula::Sintaxis),
    };
    if f < 0 || k < 0 {
        return Err(ErrorFormula::Sintaxis);
    }
    // El cero pide «toda la fila» o «toda la columna»: tampoco cabe.
    if f == 0 || k == 0 {
        return Err(ErrorFormula::Sintaxis);
    }
    if f as u64 > filas || k as u64 > cols {
        return Err(ErrorFormula::Referencia);
    }
    Ok(m.en(c, f as u64 - 1, k as u64 - 1))
}

/// La linea, si es una columna y sus valores con su posicion.
type LineaUnica = (Matriz, bool, Vec<(u64, Valor)>);

/// Una linea (columna o fila) para `COINCIDIR` y `BUSCARX`.
fn linea_unica(c: &mut Ctx, e: &Ev) -> Result<LineaUnica, ErrorFormula> {
    let m = Matriz::de(e);
    let (filas, cols) = m.dim();
    if filas != 1 && cols != 1 {
        return Err(ErrorFormula::NoDisponible);
    }
    let vertical = cols == 1;
    let celdas = m.linea(c, vertical, 0)?;
    Ok((m, vertical, celdas))
}

fn coincidir(c: &mut Ctx, args: &[Ev]) -> Resultado {
    let buscado = c.valor_de(&args[0]);
    let (_, _, celdas) = linea_unica(c, &args[1])?;
    let tipo = match args.get(2) {
        Some(t) => c.num(t)?,
        None => 1.0,
    };
    let pos = if tipo == 0.0 {
        celdas
            .iter()
            .find(|(_, v)| es_igual(&buscado, v, true))
            .map(|(p, _)| *p)
    } else if tipo > 0.0 {
        ultimo_menor_o_igual(&buscado, &celdas)
    } else {
        // De mayor a menor: el ultimo que no baja de lo buscado.
        let mut mejor = None;
        for (p, v) in &celdas {
            match orden_del_mismo_tipo(v, &buscado) {
                Some(Ordering::Less) => break,
                Some(_) => mejor = Some(*p),
                None => {}
            }
        }
        mejor
    };
    match pos {
        Some(p) => Ok(Valor::Numero(p as f64 + 1.0)),
        None => no_disponible(),
    }
}

/// `BUSCARX(buscado; donde; devolver; [si_no_esta]; [modo]; [sentido])`, lo
/// basico: `donde` y `devolver` son lineas del mismo largo. Modo: 0 exacto,
/// -1 exacto o el menor siguiente, 1 exacto o el mayor siguiente, 2 con
/// comodines. Sentido: 1 de primero a ultimo, -1 de ultimo a primero (2 y -2,
/// que en Excel piden busqueda binaria, se leen como 1 y -1).
fn buscarx(c: &mut Ctx, args: &[Ev]) -> Resultado {
    let buscado = c.valor_de(&args[0]);
    let (donde, _, mut celdas) = linea_unica(c, &args[1])?;
    let devolver = Matriz::de(&args[2]);
    let (df, dc) = devolver.dim();
    if df != 1 && dc != 1 {
        // Devolver una fila entera de una tabla no cabe en una celda.
        return Err(ErrorFormula::Sintaxis);
    }
    let (filas, cols) = donde.dim();
    if (filas.max(cols)) != df.max(dc) {
        return Err(ErrorFormula::Sintaxis);
    }
    let modo = match args.get(4) {
        Some(m) => c.entero(m)?,
        None => 0,
    };
    let sentido = match args.get(5) {
        Some(s) => c.entero(s)?,
        None => 1,
    };
    if !matches!(modo, -1..=2) || !matches!(sentido, -2 | -1 | 1 | 2) {
        return Err(ErrorFormula::Sintaxis);
    }
    if sentido < 0 {
        celdas.reverse();
    }
    let exacto = celdas
        .iter()
        .find(|(_, v)| es_igual(&buscado, v, modo == 2))
        .map(|(p, _)| *p);
    let pos = exacto.or_else(|| {
        if modo != -1 && modo != 1 {
            return None;
        }
        // El vecino mas cercano del lado pedido; el primero en el sentido de
        // la busqueda si hay empate.
        let mut mejor: Option<(u64, &Valor)> = None;
        for (p, v) in &celdas {
            let Some(o) = orden_del_mismo_tipo(v, &buscado) else {
                continue;
            };
            let vale = if modo == -1 {
                o == Ordering::Less
            } else {
                o == Ordering::Greater
            };
            if !vale {
                continue;
            }
            let mejora = match mejor {
                None => true,
                Some((_, m)) => {
                    let o = ordenar(v, m);
                    if modo == -1 {
                        o == Ordering::Greater
                    } else {
                        o == Ordering::Less
                    }
                }
            };
            if mejora {
                mejor = Some((*p, v));
            }
        }
        mejor.map(|(p, _)| p)
    });
    match pos {
        Some(p) => {
            let (f, k) = if df == 1 { (0, p) } else { (p, 0) };
            Ok(devolver.en(c, f, k))
        }
        None => match args.get(3) {
            Some(otro) => Ok(c.valor_de(otro)),
            None => no_disponible(),
        },
    }
}
