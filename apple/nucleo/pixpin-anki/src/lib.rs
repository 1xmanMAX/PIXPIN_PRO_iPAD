//! **Anki desde PixPin** (24-oct-2026, el usuario: «quiero que des una
//! integracion con Anki … ayuda mucho a estudiantes a personas para
//! recordar»).
//!
//! Tres piezas, sin ventanas:
//!
//! - [`tarjetas`]: de donde salen. De una **nota** («pregunta :: respuesta»,
//!   «P: … / R: …» y los huecos `{{c1::…}}` de Anki) y de una **clase
//!   grabada**: cada captura es una tarjeta con la imagen delante y, detras,
//!   lo que se dijo en ese tramo de la transcripcion.
//! - [`conectar`]: **AnkiConnect**, el complemento de Anki que escucha en
//!   `127.0.0.1:8765`. Con Anki abierto las tarjetas entran directas en su
//!   mazo, con sus imagenes.
//! - [`fichero`]: sin Anki abierto, un `.txt` que Anki importa tal cual
//!   («Archivo → Importar»), con el mazo y las etiquetas en su cabecera.

pub mod conectar;
pub mod fichero;
pub mod tarjetas;

pub use tarjetas::{Imagen, Tarjeta};
