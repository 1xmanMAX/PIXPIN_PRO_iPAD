//! Los criterios de `CONTAR.SI`, `SUMAR.SI`, `PROMEDIO.SI` y sus «.CONJUNTO».
//!
//! Un criterio es un valor («5», «hola») o un operador seguido de un valor
//! («>5», «<>0», «=a*»). El texto admite `*` (cualquier cosa), `?` (una letra)
//! y `~` para escribirlos tal cual.

use super::analisis::Cmp;
use super::motor::{Ctx, Ev, Resultado, Rg, es_vacio, mismo_numero};
use super::{ErrorFormula, Valor, numero_escrito};
use crate::tabla::Ref;

#[derive(Debug)]
pub(super) struct Criterio {
    op: Cmp,
    objetivo: Objetivo,
}

#[derive(Debug)]
enum Objetivo {
    Numero(f64),
    Logico(bool),
    /// En minusculas: como texto (para comparar rapido) y en letras (para los
    /// comodines).
    Texto(String, Vec<char>),
    /// «=» o «»: las celdas vacias.
    Vacio,
}

impl Criterio {
    pub fn de(v: &Valor) -> Criterio {
        match v {
            Valor::Numero(n) => Criterio {
                op: Cmp::Igual,
                objetivo: Objetivo::Numero(*n),
            },
            Valor::Logico(b) => Criterio {
                op: Cmp::Igual,
                objetivo: Objetivo::Logico(*b),
            },
            Valor::Error(_) => Criterio {
                op: Cmp::Igual,
                objetivo: Objetivo::Vacio,
            },
            Valor::Texto(t) => {
                // Los de dos letras antes que los de una.
                let (op, resto) = [
                    ("<>", Cmp::Distinto),
                    ("<=", Cmp::MenorIgual),
                    (">=", Cmp::MayorIgual),
                    ("=", Cmp::Igual),
                    ("<", Cmp::Menor),
                    (">", Cmp::Mayor),
                ]
                .iter()
                .find_map(|(p, op)| t.strip_prefix(p).map(|r| (*op, r)))
                .unwrap_or((Cmp::Igual, t.as_str()));
                let objetivo = if resto.is_empty() {
                    Objetivo::Vacio
                } else if let Some(n) = numero_escrito(resto) {
                    Objetivo::Numero(n)
                } else {
                    match resto.to_uppercase().as_str() {
                        "VERDADERO" | "TRUE" => Objetivo::Logico(true),
                        "FALSO" | "FALSE" => Objetivo::Logico(false),
                        _ => {
                            let bajo = resto.to_lowercase();
                            let letras = bajo.chars().collect();
                            Objetivo::Texto(bajo, letras)
                        }
                    }
                };
                Criterio { op, objetivo }
            }
        }
    }

    pub fn cumple(&self, v: &Valor) -> bool {
        if matches!(v, Valor::Error(_)) {
            return false;
        }
        let ord = |o: std::cmp::Ordering| match self.op {
            Cmp::Igual => o.is_eq(),
            Cmp::Distinto => o.is_ne(),
            Cmp::Menor => o.is_lt(),
            Cmp::Mayor => o.is_gt(),
            Cmp::MenorIgual => o.is_le(),
            Cmp::MayorIgual => o.is_ge(),
        };
        // Lo que no es del tipo del criterio solo cumple «distinto de».
        let otro_tipo = self.op == Cmp::Distinto;
        match &self.objetivo {
            Objetivo::Vacio => match self.op {
                Cmp::Distinto => !es_vacio(v),
                _ => es_vacio(v),
            },
            Objetivo::Numero(n) => match v {
                Valor::Numero(x) => {
                    if mismo_numero(*x, *n) {
                        ord(std::cmp::Ordering::Equal)
                    } else {
                        ord(x.partial_cmp(n).unwrap_or(std::cmp::Ordering::Equal))
                    }
                }
                _ => otro_tipo,
            },
            Objetivo::Logico(b) => match v {
                Valor::Logico(x) => ord(x.cmp(b)),
                _ => otro_tipo,
            },
            Objetivo::Texto(plano, patron) => match v {
                Valor::Texto(t) if !t.is_empty() => {
                    let con_comodines = patron.iter().any(|c| matches!(c, '*' | '?' | '~'));
                    match self.op {
                        Cmp::Igual | Cmp::Distinto => {
                            let casa = if con_comodines {
                                let t: Vec<char> = t.to_lowercase().chars().collect();
                                comodin(patron, &t, true)
                            } else {
                                // Sin comodines no hace falta construir nada.
                                if t.is_ascii() && plano.is_ascii() {
                                    t.eq_ignore_ascii_case(plano)
                                } else {
                                    t.to_lowercase() == *plano
                                }
                            };
                            casa == (self.op == Cmp::Igual)
                        }
                        _ => {
                            let t: Vec<char> = t.to_lowercase().chars().collect();
                            ord(t.cmp(&patron.to_vec()))
                        }
                    }
                }
                _ => otro_tipo,
            },
        }
    }
}

/// Si el patron (con `*`, `?` y `~`) casa con el texto: entero si `exacto`, o
/// desde el principio del texto hasta donde sea (para `HALLAR`).
pub(super) fn comodin(patron: &[char], texto: &[char], exacto: bool) -> bool {
    match patron.split_first() {
        None => !exacto || texto.is_empty(),
        Some(('*', resto)) => (0..=texto.len()).any(|i| comodin(resto, &texto[i..], exacto)),
        Some(('?', resto)) => !texto.is_empty() && comodin(resto, &texto[1..], exacto),
        Some(('~', resto)) if matches!(resto.first(), Some('*' | '?' | '~')) => {
            texto.first() == resto.first() && comodin(&resto[1..], &texto[1..], exacto)
        }
        Some((c, resto)) => texto.first() == Some(c) && comodin(resto, &texto[1..], exacto),
    }
}

pub(super) fn tiene_comodines(t: &str) -> bool {
    t.contains(['*', '?', '~'])
}

pub(super) fn llamar(c: &mut Ctx, canon: &str, args: &[Ev]) -> Option<Resultado> {
    Some(match canon {
        "CONTAR.SI" => una(c, args, None, Que::Contar),
        "SUMAR.SI" => una(c, args, args.get(2), Que::Sumar),
        "PROMEDIO.SI" => una(c, args, args.get(2), Que::Promediar),
        "CONTAR.SI.CONJUNTO" => varias(c, args, None, Que::Contar),
        "SUMAR.SI.CONJUNTO" => varias(c, &args[1..], args.first(), Que::Sumar),
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Que {
    Contar,
    Sumar,
    Promediar,
}

fn rango_de(a: &Ev) -> Result<Rg, ErrorFormula> {
    match a {
        Ev::R(rg) => Ok(*rg),
        // Una celda suelta es un rango de una celda.
        Ev::C(_) | Ev::V(_) => Err(ErrorFormula::Sintaxis),
    }
}

fn una(c: &mut Ctx, args: &[Ev], suma: Option<&Ev>, que: Que) -> Resultado {
    let crit = c.valor_de(&args[1]);
    if let Valor::Error(e) = crit {
        return Err(e);
    }
    let rg = rango_de(&args[0])?;
    let suma = match suma {
        Some(s) => Some(rango_de(s)?),
        None => None,
    };
    let clave = format!("{que:?}|{rg:?}|{suma:?}|{crit}");
    resolver(c, &[(rg, Criterio::de(&crit))], suma, que, clave)
}

/// `args` son pares rango/criterio; `suma` el rango a sumar (si lo hay).
fn varias(c: &mut Ctx, args: &[Ev], suma: Option<&Ev>, que: Que) -> Resultado {
    if args.len() % 2 != 0 {
        return Err(ErrorFormula::Sintaxis);
    }
    let mut pares = Vec::new();
    let mut clave = format!("{que:?}");
    for par in args.chunks(2) {
        let crit = c.valor_de(&par[1]);
        if let Valor::Error(e) = crit {
            return Err(e);
        }
        let rg = rango_de(&par[0])?;
        clave.push_str(&format!("|{rg:?}|{crit}"));
        pares.push((rg, Criterio::de(&crit)));
    }
    let suma = match suma {
        Some(s) => {
            let s = rango_de(s)?;
            // Aqui el rango de suma debe medir lo mismo que los de criterio.
            if s.nfilas() != pares[0].0.nfilas() || s.ncols() != pares[0].0.ncols() {
                return Err(ErrorFormula::Sintaxis);
            }
            Some(s)
        }
        None => None,
    };
    clave.push_str(&format!("|{suma:?}"));
    resolver(c, &pares, suma, que, clave)
}

/// Cuenta, suma o promedia lo que cumple todos los criterios.
///
/// Los rangos deben tener el mismo tamano (`#¡VALOR!` si no). Solo se
/// recorre el rectangulo que llega hasta la ultima celda escrita; lo que
/// queda fuera de el esta vacio en todos los rangos, y vale por igual: o
/// cumple los criterios todo ello o ninguno (`CONTAR.SI(A1:A1000;"")` cuenta
/// los huecos sin recorrerlos).
fn resolver(
    c: &mut Ctx,
    pares: &[(Rg, Criterio)],
    suma: Option<Rg>,
    que: Que,
    clave: String,
) -> Resultado {
    // Lo mismo ya preguntado (los mismos rangos y criterios) no se vuelve a
    // contar: en una hoja el mismo total por categoria se repite en cada fila.
    // Solo se guardan los resultados buenos: un error puede depender de por
    // donde se entro (ver `evaluar_camino`).
    if let Some(hecho) = c.m.criterios.get(&clave) {
        return Ok(hecho.clone());
    }
    let r = resolver_de_verdad(c, pares, suma, que)?;
    c.m.criterios.insert(clave, r.clone());
    Ok(r)
}

fn resolver_de_verdad(
    c: &mut Ctx,
    pares: &[(Rg, Criterio)],
    suma: Option<Rg>,
    que: Que,
) -> Resultado {
    let (filas, cols) = (pares[0].0.nfilas(), pares[0].0.ncols());
    for (rg, _) in pares {
        if rg.nfilas() != filas || rg.ncols() != cols {
            return Err(ErrorFormula::Sintaxis);
        }
    }
    // El rango de suma manda por su esquina y toma el tamano del primero.
    let suma = suma.map(|s| s.con_tamano(filas, cols));
    let (ext_c, ext_f) = c.extension();
    let recortado = |rg: &Rg| -> (u64, u64) {
        let f = (ext_f as u64).saturating_sub(rg.f1 as u64).min(filas);
        let k = (ext_c as u64).saturating_sub(rg.c1 as u64).min(cols);
        (f, k)
    };
    let (mut caja_f, mut caja_c) = (0, 0);
    for rg in pares.iter().map(|(r, _)| r).chain(suma.iter()) {
        let (f, k) = recortado(rg);
        caja_f = caja_f.max(f);
        caja_c = caja_c.max(k);
    }

    // Lo leido de cada rango se presta, no se copia: son centenares de valores
    // por llamada, y una llamada por celda.
    let mut leidos = Vec::new();
    for (rg, _) in pares {
        leidos.push(c.rala(*rg)?);
    }
    let leido_suma = match suma {
        Some(s) => Some(c.rala(s)?),
        None => None,
    };
    let datos: Vec<Vec<&Valor>> = pares
        .iter()
        .zip(&leidos)
        .map(|((rg, _), l)| cuadricula(l, *rg, caja_f, caja_c))
        .collect();
    let datos_suma = match (suma, &leido_suma) {
        (Some(s), Some(l)) => Some(cuadricula(l, s, caja_f, caja_c)),
        _ => None,
    };
    let mut cuenta = 0u64;
    let mut total = 0.0;
    let mut con_numero = 0u64;
    for k in 0..(caja_f * caja_c) as usize {
        if !pares
            .iter()
            .zip(&datos)
            .all(|((_, crit), d)| crit.cumple(d[k]))
        {
            continue;
        }
        cuenta += 1;
        if que == Que::Contar {
            continue;
        }
        match datos_suma.as_ref().map_or(datos[0][k], |d| d[k]) {
            Valor::Numero(n) => {
                total += n;
                con_numero += 1;
            }
            Valor::Error(e) => return Err(*e),
            _ => {}
        }
    }
    // Lo de fuera de la caja, todo vacio: cumple todo o nada. Sumar vacios no
    // cambia nada, y promediar solo cuenta numeros, asi que solo importa al
    // contar.
    if que == Que::Contar {
        let fuera = filas * cols - caja_f * caja_c;
        if fuera > 0
            && pares
                .iter()
                .all(|(_, crit)| crit.cumple(&Valor::Texto(String::new())))
        {
            cuenta += fuera;
        }
    }
    match que {
        Que::Contar => Ok(Valor::Numero(cuenta as f64)),
        Que::Sumar => Ok(Valor::Numero(total)),
        Que::Promediar => {
            if con_numero == 0 {
                Err(ErrorFormula::DivisionPorCero)
            } else {
                Ok(Valor::Numero(total / con_numero as f64))
            }
        }
    }
}

/// Lo que esta escrito en un rango, puesto en una cuadricula de `filas` x
/// `cols` (por filas); lo que no esta escrito es vacio.
fn cuadricula(leido: &[(Ref, Valor)], rg: Rg, filas: u64, cols: u64) -> Vec<&Valor> {
    static VACIO: Valor = Valor::Texto(String::new());
    let mut cuadricula = vec![&VACIO; (filas * cols) as usize];
    for (r, v) in leido {
        let (i, j) = ((r.fila - rg.f1) as u64, (r.columna - rg.c1) as u64);
        if i < filas && j < cols {
            cuadricula[(i * cols + j) as usize] = v;
        }
    }
    cuadricula
}
