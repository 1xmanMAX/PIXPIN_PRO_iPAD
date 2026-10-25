//! Del texto de una formula a su arbol.
//!
//! Primero se trocea (`trocear`) y luego se arma el arbol (`Nodo`) segun la
//! prioridad de Excel, de menos a mas: comparaciones, `&`, `+ -`, `* /`, `^`,
//! el signo de delante y el `%` de detras. Que `-2^2` sea 4 y no -4 es de
//! Excel, no un descuido: el signo se pega antes que la potencia.
//!
//! Se construye el arbol entero ANTES de calcular nada para que `SI` pueda
//! callar la rama que no toca (una division por cero en la rama descartada no
//! es un error) y para que una formula a medias sea siempre `#¡VALOR!` sin
//! haber tocado ninguna celda.

use super::ErrorFormula;
use crate::tabla::Ref;

/// Cada cosa suelta de una formula, ya separada del texto.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Pieza {
    Numero(f64),
    Texto(String),
    Celda(Ref),
    /// Tiene forma de celda pero no lo es («A0»): se guarda aparte para poder
    /// decir «referencia mala» en vez de un «no se entiende» generico.
    CeldaMala,
    /// En mayusculas. Con `(` detras es una funcion; sin ella, solo vale
    /// `VERDADERO` y `FALSO`.
    Nombre(String, bool),
    Mas,
    Menos,
    Por,
    Entre,
    Potencia,
    Concat,
    Porciento,
    Op(Cmp),
    Abre,
    Cierra,
    DosPuntos,
    Separador,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cmp {
    Igual,
    Distinto,
    Menor,
    Mayor,
    MenorIgual,
    MayorIgual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Op {
    Suma,
    Resta,
    Por,
    Entre,
    Potencia,
    Concat,
    Cmp(Cmp),
}

/// La formula ya armada.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Nodo {
    Num(f64),
    Txt(String),
    Bool(bool),
    Celda(Ref),
    Rango(Ref, Ref),
    /// Un error que ya se sabe al leer (`A0`, `A0:A3`).
    Error(ErrorFormula),
    Neg(Box<Nodo>),
    Pct(Box<Nodo>),
    Bin(Op, Box<Nodo>, Box<Nodo>),
    Llamada(String, Vec<Nodo>),
}

/// Cuantos parentesis (o funciones) se admiten uno dentro de otro. El
/// evaluador es recursivo: una formula de diez mil `(` tumbaria la pila.
const ANIDADO_MAXIMO: usize = 64;

/// Una celda («B7», «ab12») sin construir ningun texto por el camino. Igual
/// que `ref_de`, que reserva memoria para cada letra y se llama por cada
/// referencia de cada formula.
pub(super) fn celda_de(s: &str) -> Option<Ref> {
    let b = s.as_bytes();
    let letras = b.iter().take_while(|c| c.is_ascii_alphabetic()).count();
    let numeros = &b[letras..];
    if letras == 0 || letras > 7 || numeros.is_empty() || !numeros.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut columna: u32 = 0;
    for c in &b[..letras] {
        let valor = (c.to_ascii_uppercase() - b'A' + 1) as u32;
        columna = columna.checked_mul(26)?.checked_add(valor)?;
    }
    let fila: u32 = s[letras..].parse().ok()?;
    Some(Ref {
        columna: columna.checked_sub(1)?,
        fila: fila.checked_sub(1)?,
    })
}

pub(super) fn trocear(texto: &str) -> Result<Vec<Pieza>, ErrorFormula> {
    let b = texto.as_bytes();
    let mut piezas = Vec::with_capacity(b.len() / 3 + 1);
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' {
            i += 1;
            continue;
        }
        // Los espacios que trae un texto pegado de fuera (el duro, el fino).
        if c >= 0x80 {
            if let Some(x) = texto[i..].chars().next().filter(|x| x.is_whitespace()) {
                i += x.len_utf8();
                continue;
            }
        }
        let siguiente = b.get(i + 1).copied();
        let doble = match (c, siguiente) {
            (b'<', Some(b'>')) => Some(Cmp::Distinto),
            (b'<', Some(b'=')) => Some(Cmp::MenorIgual),
            (b'>', Some(b'=')) => Some(Cmp::MayorIgual),
            _ => None,
        };
        if let Some(op) = doble {
            piezas.push(Pieza::Op(op));
            i += 2;
            continue;
        }
        let simple = match c {
            b'+' => Some(Pieza::Mas),
            b'-' => Some(Pieza::Menos),
            b'*' => Some(Pieza::Por),
            b'/' => Some(Pieza::Entre),
            b'^' => Some(Pieza::Potencia),
            b'&' => Some(Pieza::Concat),
            b'%' => Some(Pieza::Porciento),
            b'=' => Some(Pieza::Op(Cmp::Igual)),
            b'<' => Some(Pieza::Op(Cmp::Menor)),
            b'>' => Some(Pieza::Op(Cmp::Mayor)),
            b'(' => Some(Pieza::Abre),
            b')' => Some(Pieza::Cierra),
            b':' => Some(Pieza::DosPuntos),
            // El punto y coma tambien separa: es el que pone Windows en
            // espanol, y una formula copiada de fuera lo trae.
            b',' | b';' => Some(Pieza::Separador),
            _ => None,
        };
        if let Some(p) = simple {
            piezas.push(p);
            i += 1;
            continue;
        }

        if c == b'"' {
            // Una comilla doble dentro del texto se escribe doble: "a""b".
            let mut texto_literal = String::new();
            i += 1;
            let mut desde = i;
            loop {
                match b.get(i) {
                    None => return Err(ErrorFormula::Sintaxis),
                    Some(b'"') if b.get(i + 1) == Some(&b'"') => {
                        texto_literal.push_str(&texto[desde..=i]);
                        i += 2;
                        desde = i;
                    }
                    Some(b'"') => {
                        texto_literal.push_str(&texto[desde..i]);
                        i += 1;
                        break;
                    }
                    Some(_) => i += 1,
                }
            }
            piezas.push(Pieza::Texto(texto_literal));
            continue;
        }

        if c.is_ascii_digit() || c == b'.' {
            let empieza = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            // El exponente (`1E3`, `2.5e-4`) solo si de verdad lleva cifras
            // detras: `2E` o `2EA` no son un numero.
            if matches!(b.get(i), Some(b'e' | b'E')) {
                let mut j = i + 1;
                if matches!(b.get(j), Some(b'+' | b'-')) {
                    j += 1;
                }
                if b.get(j).is_some_and(u8::is_ascii_digit) {
                    while b.get(j).is_some_and(u8::is_ascii_digit) {
                        j += 1;
                    }
                    i = j;
                }
            }
            // Aqui caen los «1.2.3»: mejor un error que quedarse con el 1.2.
            let n: f64 = texto[empieza..i].parse().map_err(|_| ErrorFormula::Sintaxis)?;
            piezas.push(Pieza::Numero(n));
            continue;
        }

        // El `$` de `$A$1` solo fija la celda al copiar la formula; al
        // calcular no cambia nada, y un libro de Excel lo trae a cada paso.
        let empieza_nombre = if c < 0x80 {
            c.is_ascii_alphabetic()
                || c == b'_'
                || (c == b'$' && b.get(i + 1).is_some_and(u8::is_ascii_alphabetic))
        } else {
            texto[i..].chars().next().is_some_and(char::is_alphabetic)
        };
        if empieza_nombre {
            let empieza = i;
            while i < b.len() {
                let x = b[i];
                let sigue = if x < 0x80 {
                    x.is_ascii_alphanumeric()
                        // El punto va dentro de los nombres (`SI.ERROR`).
                        || x == b'_'
                        || x == b'.'
                        || (x == b'$' && b.get(i + 1).is_some_and(u8::is_ascii_alphanumeric))
                } else {
                    texto[i..].chars().next().is_some_and(char::is_alphanumeric)
                };
                if !sigue {
                    break;
                }
                i += if x < 0x80 {
                    1
                } else {
                    texto[i..].chars().next().map_or(1, char::len_utf8)
                };
            }
            let crudo = &texto[empieza..i];
            let con_dolar = crudo.contains('$');
            let con_parentesis = b[i..]
                .iter()
                .find(|x| !x.is_ascii_whitespace())
                .is_some_and(|x| *x == b'(');
            if con_parentesis && !con_dolar {
                piezas.push(Pieza::Nombre(crudo.to_uppercase(), true));
                continue;
            }
            let limpio = if con_dolar {
                std::borrow::Cow::Owned(crudo.replace('$', ""))
            } else {
                std::borrow::Cow::Borrowed(crudo)
            };
            piezas.push(match celda_de(&limpio) {
                Some(r) => Pieza::Celda(r),
                // Con dolar o con numeros dentro solo puede querer ser una
                // celda; sin ellos es un nombre.
                None if con_dolar || limpio.bytes().any(|c| c.is_ascii_digit()) => {
                    Pieza::CeldaMala
                }
                None => Pieza::Nombre(limpio.to_uppercase(), false),
            });
            continue;
        }

        return Err(ErrorFormula::Sintaxis);
    }
    Ok(piezas)
}

/// Lo ya troceado, hecho arbol. `Err` es siempre `#¡VALOR!`.
pub(super) fn analizar(piezas: Vec<Pieza>) -> Result<Nodo, ErrorFormula> {
    let mut p = Parser {
        piezas,
        i: 0,
        hondo: 0,
    };
    let nodo = p.comparacion()?;
    // Lo que sobra tras la expresion es una formula a medias («=1 2»), y
    // quedarse con el 1 callando seria peor que decir que no se entiende.
    if p.i != p.piezas.len() {
        return Err(ErrorFormula::Sintaxis);
    }
    Ok(nodo)
}

struct Parser {
    piezas: Vec<Pieza>,
    i: usize,
    hondo: usize,
}

impl Parser {
    fn mira(&self) -> Option<&Pieza> {
        self.piezas.get(self.i)
    }

    fn traga(&mut self, pieza: &Pieza) -> bool {
        if self.mira() == Some(pieza) {
            self.i += 1;
            return true;
        }
        false
    }

    fn comparacion(&mut self) -> Result<Nodo, ErrorFormula> {
        let mut izq = self.concatenacion()?;
        while let Some(&Pieza::Op(c)) = self.mira() {
            self.i += 1;
            let der = self.concatenacion()?;
            izq = Nodo::Bin(Op::Cmp(c), Box::new(izq), Box::new(der));
        }
        Ok(izq)
    }

    fn concatenacion(&mut self) -> Result<Nodo, ErrorFormula> {
        let mut izq = self.suma()?;
        while self.traga(&Pieza::Concat) {
            let der = self.suma()?;
            izq = Nodo::Bin(Op::Concat, Box::new(izq), Box::new(der));
        }
        Ok(izq)
    }

    /// Sumas y restas: de izquierda a derecha, `2-3-4` es -5.
    fn suma(&mut self) -> Result<Nodo, ErrorFormula> {
        let mut izq = self.producto()?;
        loop {
            let op = if self.traga(&Pieza::Mas) {
                Op::Suma
            } else if self.traga(&Pieza::Menos) {
                Op::Resta
            } else {
                return Ok(izq);
            };
            let der = self.producto()?;
            izq = Nodo::Bin(op, Box::new(izq), Box::new(der));
        }
    }

    /// Debajo de las sumas justo para que `2+3*4` sean 14 y no 20.
    fn producto(&mut self) -> Result<Nodo, ErrorFormula> {
        let mut izq = self.potencia()?;
        loop {
            let op = if self.traga(&Pieza::Por) {
                Op::Por
            } else if self.traga(&Pieza::Entre) {
                Op::Entre
            } else {
                return Ok(izq);
            };
            let der = self.potencia()?;
            izq = Nodo::Bin(op, Box::new(izq), Box::new(der));
        }
    }

    /// `2^3^2` es 64 en Excel (de izquierda a derecha), no 512.
    fn potencia(&mut self) -> Result<Nodo, ErrorFormula> {
        let mut izq = self.unario()?;
        while self.traga(&Pieza::Potencia) {
            let der = self.unario()?;
            izq = Nodo::Bin(Op::Potencia, Box::new(izq), Box::new(der));
        }
        Ok(izq)
    }

    fn unario(&mut self) -> Result<Nodo, ErrorFormula> {
        if self.traga(&Pieza::Menos) {
            self.entra()?;
            let dentro = self.unario();
            self.hondo -= 1;
            return Ok(Nodo::Neg(Box::new(dentro?)));
        }
        if self.traga(&Pieza::Mas) {
            self.entra()?;
            let dentro = self.unario();
            self.hondo -= 1;
            return dentro;
        }
        let mut nodo = self.primario()?;
        while self.traga(&Pieza::Porciento) {
            nodo = Nodo::Pct(Box::new(nodo));
        }
        Ok(nodo)
    }

    fn entra(&mut self) -> Result<(), ErrorFormula> {
        self.hondo += 1;
        if self.hondo > ANIDADO_MAXIMO {
            return Err(ErrorFormula::Sintaxis);
        }
        Ok(())
    }

    fn primario(&mut self) -> Result<Nodo, ErrorFormula> {
        if self.i >= self.piezas.len() {
            // Se acabo la formula donde tenia que haber algo: «=1+».
            return Err(ErrorFormula::Sintaxis);
        }
        // Se toma la pieza sin copiarla: ya no se vuelve a mirar.
        let pieza = std::mem::replace(&mut self.piezas[self.i], Pieza::Separador);
        self.i += 1;
        match pieza {
            Pieza::Numero(n) => Ok(Nodo::Num(n)),
            Pieza::Texto(t) => Ok(Nodo::Txt(t)),
            Pieza::CeldaMala => {
                // Si era el principio de un rango, se traga el resto.
                if self.traga(&Pieza::DosPuntos) {
                    self.i += 1;
                }
                Ok(Nodo::Error(ErrorFormula::Referencia))
            }
            Pieza::Celda(a) => {
                if self.traga(&Pieza::DosPuntos) {
                    return match self.piezas.get(self.i).cloned() {
                        Some(Pieza::Celda(b)) => {
                            self.i += 1;
                            Ok(Nodo::Rango(a, b))
                        }
                        // Los dos puntos ya dicen que esto queria ser un
                        // rango; que un extremo no exista («A1:A0») es un
                        // fallo de referencia, y llamarlo sintaxis mandaria a
                        // buscar el parentesis que falta.
                        Some(Pieza::CeldaMala) => {
                            self.i += 1;
                            Ok(Nodo::Error(ErrorFormula::Referencia))
                        }
                        _ => Err(ErrorFormula::Referencia),
                    };
                }
                Ok(Nodo::Celda(a))
            }
            Pieza::Abre => {
                self.entra()?;
                let dentro = self.comparacion();
                self.hondo -= 1;
                let dentro = dentro?;
                if !self.traga(&Pieza::Cierra) {
                    return Err(ErrorFormula::Sintaxis);
                }
                Ok(dentro)
            }
            Pieza::Nombre(nombre, true) => {
                self.i += 1; // el `(`
                self.entra()?;
                let r = self.argumentos();
                self.hondo -= 1;
                Ok(Nodo::Llamada(nombre, r?))
            }
            Pieza::Nombre(nombre, false) => match nombre.as_str() {
                "VERDADERO" | "TRUE" => Ok(Nodo::Bool(true)),
                "FALSO" | "FALSE" => Ok(Nodo::Bool(false)),
                _ => Err(ErrorFormula::Sintaxis),
            },
            _ => Err(ErrorFormula::Sintaxis),
        }
    }

    fn argumentos(&mut self) -> Result<Vec<Nodo>, ErrorFormula> {
        let mut args = Vec::new();
        if self.traga(&Pieza::Cierra) {
            return Ok(args);
        }
        loop {
            args.push(self.comparacion()?);
            if self.traga(&Pieza::Separador) {
                continue;
            }
            if self.traga(&Pieza::Cierra) {
                return Ok(args);
            }
            return Err(ErrorFormula::Sintaxis);
        }
    }
}
