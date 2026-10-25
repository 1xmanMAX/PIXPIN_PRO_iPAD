//! El evaluador: recorre el arbol de una formula y da su valor.
//!
//! Aqui viven las cuentas (`+ - * / ^ & %` y las comparaciones), el reparto de
//! las funciones por su nombre y los trucos que comparten todas: convertir un
//! valor en numero, en texto o en verdadero/falso, y leer un rango.
//!
//! Las funciones en si estan en `mates`, `texto`, `busqueda`, `criterio`,
//! `fecha` e `info`. Todas devuelven `Option<Resultado>`: `None` es «esa
//! funcion no es mia», y el reparto prueba la siguiente.

use std::cmp::Ordering;
use std::rc::Rc;

use super::analisis::{Cmp, Nodo, Op};
use super::{
    ErrorFormula, Memoria, Valor, celdas_del_rango, evaluar_camino, general, numero_escrito,
};
use crate::tabla::{Ref, Tabla};

pub(super) type Resultado = Result<Valor, ErrorFormula>;

/// Un rectangulo de celdas ya con las esquinas en orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Rg {
    pub c1: u32,
    pub f1: u32,
    pub c2: u32,
    pub f2: u32,
}

impl Rg {
    pub fn nuevo(a: Ref, b: Ref) -> Rg {
        // Al reves (`B6:B1`) es el mismo rango.
        Rg {
            c1: a.columna.min(b.columna),
            f1: a.fila.min(b.fila),
            c2: a.columna.max(b.columna),
            f2: a.fila.max(b.fila),
        }
    }

    pub fn nfilas(&self) -> u64 {
        (self.f2 - self.f1) as u64 + 1
    }

    pub fn ncols(&self) -> u64 {
        (self.c2 - self.c1) as u64 + 1
    }

    /// El mismo sitio de arriba a la izquierda con otro tamano.
    pub fn con_tamano(&self, filas: u64, cols: u64) -> Rg {
        Rg {
            c1: self.c1,
            f1: self.f1,
            c2: (self.c1 as u64 + cols.max(1) - 1).min(u32::MAX as u64) as u32,
            f2: (self.f1 as u64 + filas.max(1) - 1).min(u32::MAX as u64) as u32,
        }
    }
}

/// Lo que da evaluar un trozo de formula.
pub(super) enum Ev {
    /// Un valor suelto: un numero escrito, el resultado de una cuenta.
    V(Valor),
    /// El valor de UNA celda citada. Se distingue porque dentro de `SUMA` un
    /// texto escrito a mano se intenta leer como numero y el de una celda se
    /// ignora (como Excel).
    C(Valor),
    R(Rg),
}

pub(super) struct Ctx<'t, 'm> {
    pub tabla: &'t Tabla,
    pub m: &'m mut Memoria<'t>,
}

impl Ctx<'_, '_> {
    pub fn celda(&mut self, r: Ref) -> Valor {
        evaluar_camino(self.tabla, r, self.m)
    }

    /// Hasta donde llegan las celdas escritas: (columnas, filas).
    pub fn extension(&mut self) -> (u32, u32) {
        if let Some(e) = self.m.extension {
            return e;
        }
        // Un rango de ida y vuelta construye el indice como efecto.
        let _ = celdas_del_rango(
            self.tabla,
            Ref { columna: 0, fila: 0 },
            Ref { columna: 0, fila: 0 },
            &mut self.m.indice,
        );
        let mut cols = 0;
        let mut filas = 0;
        for r in self.m.indice.iter().flatten() {
            cols = cols.max(r.columna + 1);
            filas = filas.max(r.fila + 1);
        }
        self.m.extension = Some((cols, filas));
        (cols, filas)
    }

    /// Las celdas escritas de un rango con su valor, por columnas y de arriba
    /// abajo. Un ciclo dentro del rango sube: el rango no se puede leer, y
    /// seguir callando daria un resultado creible y falso.
    pub fn rala(&mut self, rg: Rg) -> Result<Rc<Vec<(Ref, Valor)>>, ErrorFormula> {
        // Un rango que se lee veinte veces (`BUSCARV` sobre la misma tabla en
        // veinte filas) se lee una. Un ciclo no llega aqui guardado: sale
        // como error antes, y depende de por donde se entro.
        if let Some(hecho) = self.m.rangos.get(&rg) {
            return Ok(hecho.clone());
        }
        let refs = celdas_del_rango(
            self.tabla,
            Ref {
                columna: rg.c1,
                fila: rg.f1,
            },
            Ref {
                columna: rg.c2,
                fila: rg.f2,
            },
            &mut self.m.indice,
        );
        let mut salida = Vec::with_capacity(refs.len());
        for r in refs {
            let v = evaluar_camino(self.tabla, r, self.m);
            if v == Valor::Error(ErrorFormula::Ciclo) {
                return Err(ErrorFormula::Ciclo);
            }
            salida.push((r, v));
        }
        let salida = Rc::new(salida);
        self.m.rangos.insert(rg, salida.clone());
        Ok(salida)
    }

    /// El serial de Excel de este instante, en hora local.
    pub fn ahora(&mut self) -> f64 {
        let ms = *self
            .m
            .ahora_ms
            .get_or_insert_with(pixpin_sistema::entorno::ahora_local_ms);
        // 25569 es el serial de 1970-01-01.
        25569.0 + ms as f64 / 86_400_000.0
    }

    /// Un numero de [0, 1) (xorshift). No hace falta mas: es para `ALEATORIO`.
    pub fn azar(&mut self) -> f64 {
        let mut x = *self.m.semilla.get_or_insert_with(|| {
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x9E37_79B9_7F4A_7C15);
            t | 1
        });
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.m.semilla = Some(x);
        (x >> 11) as f64 / (1u64 << 53) as f64
    }

    // ---- Evaluar -------------------------------------------------------

    pub fn eval(&mut self, n: &Nodo) -> Ev {
        match n {
            Nodo::Num(x) => Ev::V(Valor::Numero(*x)),
            Nodo::Txt(t) => Ev::V(Valor::Texto(t.clone())),
            Nodo::Bool(b) => Ev::V(Valor::Logico(*b)),
            Nodo::Celda(r) => Ev::C(self.celda(*r)),
            Nodo::Rango(a, b) => Ev::R(Rg::nuevo(*a, *b)),
            Nodo::Error(e) => Ev::V(Valor::Error(*e)),
            Nodo::Neg(x) => {
                let v = self.valor(x);
                Ev::V(match a_numero(&v) {
                    Ok(n) => Valor::Numero(-n),
                    Err(e) => Valor::Error(e),
                })
            }
            Nodo::Pct(x) => {
                let v = self.valor(x);
                Ev::V(match a_numero(&v) {
                    Ok(n) => Valor::Numero(n / 100.0),
                    Err(e) => Valor::Error(e),
                })
            }
            Nodo::Bin(op, a, b) => {
                let a = self.valor(a);
                let b = self.valor(b);
                Ev::V(operar(*op, a, b))
            }
            Nodo::Llamada(nombre, args) => self.llamar(nombre, args),
        }
    }

    /// El valor de un trozo de formula donde solo cabe un valor: un rango de
    /// varias celdas no vale («=B1:B6*2» no significa nada, y elegir por el
    /// usuario cual de las seis queria seria adivinar).
    pub fn valor(&mut self, n: &Nodo) -> Valor {
        let e = self.eval(n);
        self.escalar(e)
    }

    pub fn escalar(&mut self, e: Ev) -> Valor {
        match e {
            Ev::V(v) | Ev::C(v) => v,
            Ev::R(rg) if rg.nfilas() == 1 && rg.ncols() == 1 => self.celda(Ref {
                columna: rg.c1,
                fila: rg.f1,
            }),
            Ev::R(_) => Valor::Error(ErrorFormula::Sintaxis),
        }
    }

    fn escalar_ref(&mut self, e: &Ev) -> Valor {
        match e {
            Ev::V(v) | Ev::C(v) => v.clone(),
            Ev::R(rg) => self.escalar(Ev::R(*rg)),
        }
    }

    pub fn num(&mut self, e: &Ev) -> Result<f64, ErrorFormula> {
        let v = self.escalar_ref(e);
        a_numero(&v)
    }

    pub fn entero(&mut self, e: &Ev) -> Result<i64, ErrorFormula> {
        let n = self.num(e)?;
        if !n.is_finite() || n.abs() > 9e15 {
            return Err(ErrorFormula::Numero);
        }
        Ok(n.trunc() as i64)
    }

    pub fn texto(&mut self, e: &Ev) -> Result<String, ErrorFormula> {
        let v = self.escalar_ref(e);
        a_texto(&v)
    }

    pub fn logico(&mut self, e: &Ev) -> Result<bool, ErrorFormula> {
        let v = self.escalar_ref(e);
        a_logico(&v)
    }

    pub fn valor_de(&mut self, e: &Ev) -> Valor {
        self.escalar_ref(e)
    }

    /// Los numeros que aportan unos argumentos.
    ///
    /// El texto y las celdas vacias no aportan nada —ni siquiera un cero— y
    /// por eso `PROMEDIO` y `CONTAR` salen bien sin saber nada de ellos. Un
    /// error dentro sube: una suma que se salta la celda rota daria un total
    /// creible y falso, que es lo peor que puede pasar aqui.
    pub fn numeros(&mut self, args: &[Ev]) -> Result<Vec<f64>, ErrorFormula> {
        let mut numeros = Vec::new();
        for a in args {
            match a {
                Ev::V(v) => match v {
                    Valor::Numero(n) => numeros.push(*n),
                    Valor::Logico(b) => numeros.push(*b as u8 as f64),
                    // Escrito a mano, «"5"» se lee como cinco; texto que no es
                    // numero se calla, como siempre en las funciones.
                    Valor::Texto(t) => {
                        if let Some(n) = numero_escrito(t) {
                            numeros.push(n);
                        }
                    }
                    Valor::Error(e) => return Err(*e),
                },
                Ev::C(v) => match v {
                    Valor::Numero(n) => numeros.push(*n),
                    Valor::Error(e) => return Err(*e),
                    _ => {}
                },
                Ev::R(rg) => {
                    for (_, v) in self.rala(*rg)?.iter() {
                        match v {
                            Valor::Numero(n) => numeros.push(*n),
                            Valor::Error(e) => return Err(*e),
                            _ => {}
                        }
                    }
                }
            }
        }
        Ok(numeros)
    }

    // ---- Llamar --------------------------------------------------------

    fn llamar(&mut self, nombre: &str, args: &[Nodo]) -> Ev {
        let Some((canon, min, max)) = catalogo(nombre) else {
            return Ev::V(Valor::Error(ErrorFormula::Nombre));
        };
        if args.len() < min || args.len() > max {
            return Ev::V(Valor::Error(ErrorFormula::Sintaxis));
        }
        match canon {
            "SI" => {
                let c = self.eval(&args[0]);
                let cond = match self.logico(&c) {
                    Ok(b) => b,
                    Err(e) => return Ev::V(Valor::Error(e)),
                };
                if cond {
                    return self.eval(&args[1]);
                }
                match args.get(2) {
                    Some(n) => self.eval(n),
                    None => Ev::V(Valor::Logico(false)),
                }
            }
            "SI.ERROR" | "SI.ND" => {
                let e = self.eval(&args[0]);
                let v = match &e {
                    Ev::R(_) => return e,
                    Ev::V(v) | Ev::C(v) => v.clone(),
                };
                match v {
                    // Un ciclo no se tapa: no es un dato malo, es la hoja mal
                    // escrita.
                    Valor::Error(ErrorFormula::Ciclo) => Ev::V(v),
                    Valor::Error(x) if canon == "SI.ERROR" || x == ErrorFormula::NoDisponible => {
                        self.eval(&args[1])
                    }
                    _ => e,
                }
            }
            "SI.CONJUNTO" => {
                if args.len() % 2 != 0 {
                    return Ev::V(Valor::Error(ErrorFormula::Sintaxis));
                }
                for par in args.chunks(2) {
                    let c = self.eval(&par[0]);
                    match self.logico(&c) {
                        Ok(true) => return self.eval(&par[1]),
                        Ok(false) => {}
                        Err(e) => return Ev::V(Valor::Error(e)),
                    }
                }
                Ev::V(Valor::Error(ErrorFormula::NoDisponible))
            }
            _ => {
                let evs: Vec<Ev> = args.iter().map(|a| self.eval(a)).collect();
                // Los errores de los argumentos suben, salvo en las funciones
                // cuyo oficio es mirarlos.
                if (!canon.starts_with("ES") || canon == "ESPACIOS") && canon != "CONTARA" && canon != "CONTAR.BLANCO" {
                    for e in &evs {
                        if let Ev::V(Valor::Error(x)) | Ev::C(Valor::Error(x)) = e {
                            return Ev::V(Valor::Error(*x));
                        }
                    }
                }
                let r = super::mates::llamar(self, canon, &evs)
                    .or_else(|| super::criterio::llamar(self, canon, &evs))
                    .or_else(|| super::busqueda::llamar(self, canon, &evs))
                    .or_else(|| super::texto::llamar(self, canon, &evs))
                    .or_else(|| super::fecha::llamar(self, canon, &evs))
                    .or_else(|| super::info::llamar(self, canon, &evs))
                    .unwrap_or(Err(ErrorFormula::Nombre));
                Ev::V(r.unwrap_or_else(Valor::Error))
            }
        }
    }
}

/// Una cuenta entre dos valores, arrastrando el primer error que aparezca.
fn operar(op: Op, a: Valor, b: Valor) -> Valor {
    let r = (|| -> Resultado {
        match op {
            Op::Concat => {
                let (a, b) = (a_texto(&a), a_texto(&b));
                Ok(Valor::Texto(format!("{}{}", a?, b?)))
            }
            Op::Cmp(c) => {
                for v in [&a, &b] {
                    if let Valor::Error(e) = v {
                        return Err(*e);
                    }
                }
                let o = ordenar(&a, &b);
                Ok(Valor::Logico(match c {
                    Cmp::Igual => o == Ordering::Equal,
                    Cmp::Distinto => o != Ordering::Equal,
                    Cmp::Menor => o == Ordering::Less,
                    Cmp::Mayor => o == Ordering::Greater,
                    Cmp::MenorIgual => o != Ordering::Greater,
                    Cmp::MayorIgual => o != Ordering::Less,
                }))
            }
            _ => {
                let (x, y) = (a_numero(&a), a_numero(&b));
                let (x, y) = (x?, y?);
                let n = match op {
                    Op::Suma => x + y,
                    Op::Resta => x - y,
                    Op::Por => x * y,
                    Op::Entre => {
                        if y == 0.0 {
                            // Dividir entre cero da infinito en Rust, y un
                            // infinito pintado en una celda no le dice nada
                            // a nadie.
                            return Err(ErrorFormula::DivisionPorCero);
                        }
                        x / y
                    }
                    _ => potencia(x, y)?,
                };
                if !n.is_finite() {
                    return Err(ErrorFormula::Numero);
                }
                Ok(Valor::Numero(n))
            }
        }
    })();
    r.unwrap_or_else(Valor::Error)
}

/// `x^y` con los errores de Excel: `0^0` y la raiz de un negativo son
/// `#¡NUM!`, y `0^-1` es `#¡DIV/0!`.
pub(super) fn potencia(x: f64, y: f64) -> Result<f64, ErrorFormula> {
    if x == 0.0 && y == 0.0 {
        return Err(ErrorFormula::Numero);
    }
    if x == 0.0 && y < 0.0 {
        return Err(ErrorFormula::DivisionPorCero);
    }
    if x < 0.0 && y.fract() != 0.0 {
        return Err(ErrorFormula::Numero);
    }
    let r = x.powf(y);
    if r.is_finite() {
        Ok(r)
    } else {
        Err(ErrorFormula::Numero)
    }
}

/// El numero de un valor para operar con el.
pub(super) fn a_numero(v: &Valor) -> Result<f64, ErrorFormula> {
    match v {
        Valor::Numero(n) => Ok(*n),
        Valor::Logico(b) => Ok(*b as u8 as f64),
        // La celda vacia vale cero: `=B1+B2` con una sin rellenar tiene que
        // dar la otra, que es lo unico que el usuario puede esperar.
        Valor::Texto(t) if t.is_empty() => Ok(0.0),
        // El texto de verdad, no, salvo que sea un numero escrito entre
        // comillas. Ver la cabecera del fichero: dentro de una funcion se
        // ignora, pero multiplicar «Importe» por dos no significa nada y dar
        // cero seria mentir con cara de resultado.
        Valor::Texto(t) => numero_escrito(t).ok_or(ErrorFormula::Sintaxis),
        Valor::Error(e) => Err(*e),
    }
}

pub(super) fn a_texto(v: &Valor) -> Result<String, ErrorFormula> {
    match v {
        Valor::Numero(n) => Ok(general(*n)),
        Valor::Texto(t) => Ok(t.clone()),
        Valor::Logico(b) => Ok(if *b { "VERDADERO" } else { "FALSO" }.into()),
        Valor::Error(e) => Err(*e),
    }
}

pub(super) fn a_logico(v: &Valor) -> Result<bool, ErrorFormula> {
    match v {
        Valor::Numero(n) => Ok(*n != 0.0),
        Valor::Logico(b) => Ok(*b),
        Valor::Texto(t) => {
            let t = t.trim().to_uppercase();
            match t.as_str() {
                "" => Ok(false),
                "VERDADERO" | "TRUE" => Ok(true),
                "FALSO" | "FALSE" => Ok(false),
                _ => Err(ErrorFormula::Sintaxis),
            }
        }
        Valor::Error(e) => Err(*e),
    }
}

/// Si dos numeros son el mismo para una hoja de calculo: 0.1+0.2 y 0.3 lo son.
pub(super) fn mismo_numero(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-14 * a.abs().max(b.abs())
}

/// Si es una celda sin nada (o texto vacio, que aqui es lo mismo).
pub(super) fn es_vacio(v: &Valor) -> bool {
    matches!(v, Valor::Texto(t) if t.is_empty())
}

/// El orden de Excel: numeros, luego textos, luego verdadero/falso. El texto
/// no distingue mayusculas. Una celda vacia se parece a lo que tenga delante:
/// cero, texto vacio o falso.
pub(super) fn ordenar(a: &Valor, b: &Valor) -> Ordering {
    let (a, b) = match (es_vacio(a), es_vacio(b)) {
        (true, true) => return Ordering::Equal,
        (true, false) => (vacio_como(b), b.clone()),
        (false, true) => (a.clone(), vacio_como(a)),
        (false, false) => (a.clone(), b.clone()),
    };
    fn rango(v: &Valor) -> u8 {
        match v {
            Valor::Numero(_) => 0,
            Valor::Texto(_) => 1,
            Valor::Logico(_) => 2,
            Valor::Error(_) => 3,
        }
    }
    match (&a, &b) {
        (Valor::Numero(x), Valor::Numero(y)) => {
            if mismo_numero(*x, *y) {
                Ordering::Equal
            } else {
                x.partial_cmp(y).unwrap_or(Ordering::Equal)
            }
        }
        (Valor::Texto(x), Valor::Texto(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
        (Valor::Logico(x), Valor::Logico(y)) => x.cmp(y),
        _ => rango(&a).cmp(&rango(&b)),
    }
}

fn vacio_como(otro: &Valor) -> Valor {
    match otro {
        Valor::Numero(_) => Valor::Numero(0.0),
        Valor::Logico(_) => Valor::Logico(false),
        _ => Valor::Texto(String::new()),
    }
}

/// Las funciones que hay: (nombre, nombre en espanol, argumentos minimos y
/// maximos). El nombre en espanol es el que manda dentro del motor.
const CATALOGO: &[(&str, &str, usize, usize)] = &[
    // Logicas
    ("SI", "SI", 2, 3),
    ("IF", "SI", 2, 3),
    ("Y", "Y", 1, 255),
    ("AND", "Y", 1, 255),
    ("O", "O", 1, 255),
    ("OR", "O", 1, 255),
    ("NO", "NO", 1, 1),
    ("NOT", "NO", 1, 1),
    ("SI.ERROR", "SI.ERROR", 2, 2),
    ("IFERROR", "SI.ERROR", 2, 2),
    ("SI.ND", "SI.ND", 2, 2),
    ("IFNA", "SI.ND", 2, 2),
    ("SI.CONJUNTO", "SI.CONJUNTO", 2, 254),
    ("IFS", "SI.CONJUNTO", 2, 254),
    ("VERDADERO", "VERDADERO", 0, 0),
    ("TRUE", "VERDADERO", 0, 0),
    ("FALSO", "FALSO", 0, 0),
    ("FALSE", "FALSO", 0, 0),
    // Las de siempre. Sin argumentos valen (`SUMA()` es 0), como antes.
    ("SUMA", "SUMA", 0, 255),
    ("SUM", "SUMA", 0, 255),
    ("CONTAR", "CONTAR", 0, 255),
    ("COUNT", "CONTAR", 0, 255),
    ("PROMEDIO", "PROMEDIO", 0, 255),
    ("AVERAGE", "PROMEDIO", 0, 255),
    ("MIN", "MIN", 0, 255),
    ("MINIMO", "MIN", 0, 255),
    ("MAX", "MAX", 0, 255),
    ("MAXIMO", "MAX", 0, 255),
    // Matematicas
    ("REDONDEAR", "REDONDEAR", 2, 2),
    ("ROUND", "REDONDEAR", 2, 2),
    ("REDONDEAR.MAS", "REDONDEAR.MAS", 2, 2),
    ("ROUNDUP", "REDONDEAR.MAS", 2, 2),
    ("REDONDEAR.MENOS", "REDONDEAR.MENOS", 2, 2),
    ("ROUNDDOWN", "REDONDEAR.MENOS", 2, 2),
    ("ENTERO", "ENTERO", 1, 1),
    ("INT", "ENTERO", 1, 1),
    ("ABS", "ABS", 1, 1),
    ("RESIDUO", "RESIDUO", 2, 2),
    ("MOD", "RESIDUO", 2, 2),
    ("POTENCIA", "POTENCIA", 2, 2),
    ("POWER", "POTENCIA", 2, 2),
    ("RAIZ", "RAIZ", 1, 1),
    ("SQRT", "RAIZ", 1, 1),
    ("PRODUCTO", "PRODUCTO", 1, 255),
    ("PRODUCT", "PRODUCTO", 1, 255),
    ("MEDIANA", "MEDIANA", 1, 255),
    ("MEDIAN", "MEDIANA", 1, 255),
    ("PI", "PI", 0, 0),
    ("ALEATORIO", "ALEATORIO", 0, 0),
    ("RAND", "ALEATORIO", 0, 0),
    ("ALEATORIO.ENTRE", "ALEATORIO.ENTRE", 2, 2),
    ("RANDBETWEEN", "ALEATORIO.ENTRE", 2, 2),
    ("CONTARA", "CONTARA", 1, 255),
    ("COUNTA", "CONTARA", 1, 255),
    ("CONTAR.BLANCO", "CONTAR.BLANCO", 1, 1),
    ("COUNTBLANK", "CONTAR.BLANCO", 1, 1),
    // Con criterio
    ("SUMAR.SI", "SUMAR.SI", 2, 3),
    ("SUMIF", "SUMAR.SI", 2, 3),
    ("SUMAR.SI.CONJUNTO", "SUMAR.SI.CONJUNTO", 3, 255),
    ("SUMIFS", "SUMAR.SI.CONJUNTO", 3, 255),
    ("CONTAR.SI", "CONTAR.SI", 2, 2),
    ("COUNTIF", "CONTAR.SI", 2, 2),
    ("CONTAR.SI.CONJUNTO", "CONTAR.SI.CONJUNTO", 2, 254),
    ("COUNTIFS", "CONTAR.SI.CONJUNTO", 2, 254),
    ("PROMEDIO.SI", "PROMEDIO.SI", 2, 3),
    ("AVERAGEIF", "PROMEDIO.SI", 2, 3),
    // Busqueda
    ("BUSCARV", "BUSCARV", 3, 4),
    ("VLOOKUP", "BUSCARV", 3, 4),
    ("BUSCARH", "BUSCARH", 3, 4),
    ("HLOOKUP", "BUSCARH", 3, 4),
    ("INDICE", "INDICE", 2, 3),
    ("INDEX", "INDICE", 2, 3),
    ("COINCIDIR", "COINCIDIR", 2, 3),
    ("MATCH", "COINCIDIR", 2, 3),
    ("BUSCARX", "BUSCARX", 3, 6),
    ("XLOOKUP", "BUSCARX", 3, 6),
    // Texto
    ("CONCATENAR", "CONCATENAR", 1, 255),
    ("CONCATENATE", "CONCATENAR", 1, 255),
    ("CONCAT", "CONCAT", 1, 255),
    ("UNIRCADENAS", "UNIRCADENAS", 3, 255),
    ("TEXTJOIN", "UNIRCADENAS", 3, 255),
    ("IZQUIERDA", "IZQUIERDA", 1, 2),
    ("LEFT", "IZQUIERDA", 1, 2),
    ("DERECHA", "DERECHA", 1, 2),
    ("RIGHT", "DERECHA", 1, 2),
    ("EXTRAE", "EXTRAE", 3, 3),
    ("MID", "EXTRAE", 3, 3),
    ("LARGO", "LARGO", 1, 1),
    ("LEN", "LARGO", 1, 1),
    ("MAYUSC", "MAYUSC", 1, 1),
    ("UPPER", "MAYUSC", 1, 1),
    ("MINUSC", "MINUSC", 1, 1),
    ("LOWER", "MINUSC", 1, 1),
    ("NOMPROPIO", "NOMPROPIO", 1, 1),
    ("PROPER", "NOMPROPIO", 1, 1),
    ("ESPACIOS", "ESPACIOS", 1, 1),
    ("TRIM", "ESPACIOS", 1, 1),
    ("SUSTITUIR", "SUSTITUIR", 3, 4),
    ("SUBSTITUTE", "SUSTITUIR", 3, 4),
    ("ENCONTRAR", "ENCONTRAR", 2, 3),
    ("FIND", "ENCONTRAR", 2, 3),
    ("HALLAR", "HALLAR", 2, 3),
    ("SEARCH", "HALLAR", 2, 3),
    ("TEXTO", "TEXTO", 2, 2),
    ("TEXT", "TEXTO", 2, 2),
    ("VALOR", "VALOR", 1, 1),
    ("VALUE", "VALOR", 1, 1),
    // Fecha
    ("HOY", "HOY", 0, 0),
    ("TODAY", "HOY", 0, 0),
    ("AHORA", "AHORA", 0, 0),
    ("NOW", "AHORA", 0, 0),
    ("FECHA", "FECHA", 3, 3),
    ("DATE", "FECHA", 3, 3),
    ("AÑO", "AÑO", 1, 1),
    ("ANO", "AÑO", 1, 1),
    ("YEAR", "AÑO", 1, 1),
    ("MES", "MES", 1, 1),
    ("MONTH", "MES", 1, 1),
    ("DIA", "DIA", 1, 1),
    ("DAY", "DIA", 1, 1),
    ("DIAS", "DIAS", 2, 2),
    ("DAYS", "DIAS", 2, 2),
    ("DIASEM", "DIASEM", 1, 2),
    ("WEEKDAY", "DIASEM", 1, 2),
    // Informacion
    ("ESERROR", "ESERROR", 1, 1),
    ("ISERROR", "ESERROR", 1, 1),
    ("ESBLANCO", "ESBLANCO", 1, 1),
    ("ISBLANK", "ESBLANCO", 1, 1),
    ("ESNUMERO", "ESNUMERO", 1, 1),
    ("ISNUMBER", "ESNUMERO", 1, 1),
    ("ESND", "ESND", 1, 1),
    ("ISNA", "ESND", 1, 1),
    ("ESTEXTO", "ESTEXTO", 1, 1),
    ("ISTEXT", "ESTEXTO", 1, 1),
];

/// El nombre de dentro y cuantos argumentos admite, o `None` si no existe.
fn catalogo(nombre: &str) -> Option<(&'static str, usize, usize)> {
    // Buscar entre ciento cincuenta nombres uno por uno, en cada llamada de
    // cada celda, se notaba: un mapa hecho la primera vez.
    static MAPA: std::sync::OnceLock<std::collections::HashMap<&'static str, (&'static str, usize, usize)>> =
        std::sync::OnceLock::new();
    MAPA.get_or_init(|| {
        CATALOGO
            .iter()
            .map(|(alias, canon, min, max)| (*alias, (*canon, *min, *max)))
            .collect()
    })
    .get(nombre)
    .copied()
}

#[cfg(test)]
pub(super) fn existe(nombre: &str) -> bool {
    catalogo(nombre).is_some()
}

#[cfg(test)]
pub(super) fn todos_los_nombres() -> Vec<&'static str> {
    CATALOGO.iter().map(|(a, ..)| *a).collect()
}
