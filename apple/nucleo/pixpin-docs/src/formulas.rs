//! **Las formulas de las notas**: viven en el motor del lienzo
//! (`pixpin_motor2d::formulas`) desde el 15-oct-2026, porque el texto del
//! lienzo tambien las compone en linea (`pixpin_motor2d::texto_rico`). Un
//! solo lector para notas, lienzo y asistente; aqui solo se reexporta para
//! no cambiar a quien ya lo llamaba por este camino.

pub use pixpin_motor2d::formulas::*;
