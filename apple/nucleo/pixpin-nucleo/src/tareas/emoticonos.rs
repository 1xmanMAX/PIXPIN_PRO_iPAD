//! **Las etiquetas de una tarea son sus emoticonos** (tareas-v4).
//!
//! El usuario (5-oct): «sobre eso haya el texto y algunas etiquetas que sean
//! emoticones, solo emoticones». No hay un campo de etiquetas en el
//! cuaderno (ni en el movil): las etiquetas son los emoticonos que ya se
//! escriben en la tarea. La vista de tarjetas los saca del texto y los
//! pinta aparte, como chapitas; el texto guardado no se toca.
//!
//! El parser, los tonos pastel y la pila de circulos viven en
//! `pixpin_timeline::emoticonos` (con sus pruebas), compartidos con el
//! timeline: antes habia dos copias que discrepaban (⌚ si aqui, no alli).
//! Aqui solo se reexporta lo que usan las tareas.

#![forbid(unsafe_code)]

pub use pixpin_timeline::emoticonos::emoticonos_de;
