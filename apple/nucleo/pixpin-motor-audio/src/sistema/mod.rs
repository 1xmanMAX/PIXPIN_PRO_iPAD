//! Lo que toca Windows: COM, WASAPI y Media Foundation, con su `unsafe`
//! acotado y comentado. Nada de aqui decide nada de sonido; eso esta en los
//! modulos puros del crate.

pub(crate) mod aparatos;
pub(crate) mod captura;
pub(crate) mod com;
pub(crate) mod m4a;
pub(crate) mod proceso;
