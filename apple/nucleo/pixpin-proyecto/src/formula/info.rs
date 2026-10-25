//! Funciones que preguntan por la clase de un valor: `ESERROR`, `ESBLANCO`,
//! `ESNUMERO`, `ESND`, `ESTEXTO`.
//!
//! Son las unicas (con `SI.ERROR`) a las que un error en el argumento no
//! hace subir el error: su oficio es mirarlo.

use super::motor::{Ctx, Ev, Resultado, es_vacio};
use super::{ErrorFormula, Valor};

pub(super) fn llamar(c: &mut Ctx, canon: &str, args: &[Ev]) -> Option<Resultado> {
    if !canon.starts_with("ES") {
        return None;
    }
    let v = c.valor_de(&args[0]);
    // Un ciclo no se esconde detras de un «es error».
    if v == Valor::Error(ErrorFormula::Ciclo) {
        return Some(Err(ErrorFormula::Ciclo));
    }
    // Una celda de verdad vacia y un texto vacio devuelto por una formula
    // se ven igual desde aqui; los dos son blanco.
    let es = match canon {
        "ESERROR" => matches!(v, Valor::Error(_)),
        "ESND" => v == Valor::Error(ErrorFormula::NoDisponible),
        "ESBLANCO" => es_vacio(&v),
        "ESNUMERO" => matches!(v, Valor::Numero(_)),
        "ESTEXTO" => matches!(&v, Valor::Texto(t) if !t.is_empty()),
        _ => return None,
    };
    Some(Ok(Valor::Logico(es)))
}
