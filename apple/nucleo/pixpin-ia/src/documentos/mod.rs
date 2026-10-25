//! **Preguntarle al asistente por los documentos propios, sin internet**
//! (encargo del 13-oct-2026): «¿que dice la tesis sobre…?», «busca en mis
//! notas…», «¿donde mande el plano de cunetas?».
//!
//! ## Lo que se decidio y por que
//!
//! - **Solo BM25, sin vectores.** Se midio un modelo de *embeddings* pequeno
//!   y multilingue (multilingual-e5-small Q8_0, 132 MB) en el mismo
//!   `llama-server` b11580 de la app, en la CPU de este PC (13-oct): 14 s en
//!   cargar, **506 MB de RAM** de pico, y **0,58 s por trozo** de ~200
//!   tokens (con 4 hilos, otros agentes compilando a la vez). Una tesis de
//!   400 trozos son 4 minutos; un disco con 20 000 trozos, horas de CPU. Y
//!   hace falta un segundo proceso (el servidor del chat lleva el Qwen y no
//!   sirve *embeddings* a la vez) y otra descarga. Para preguntas sobre los
//!   documentos propios —que nombran lo que buscan: «cunetas», «tesis»,
//!   «presupuesto»— BM25 con raices acierta, y cuesta milisegundos y unos
//!   pocos MB. No compensa: solo BM25.
//! - **El indice es una cache** (`<raiz>\cache\documentos`): no se
//!   sincroniza y se puede borrar sin perder nada. Se pone al dia por
//!   partes (firma por fuente, como `IndiceDelBuscador.kt` del movil).
//! - **El origen manda**: cada trozo sabe de que fichero y pagina (o de que
//!   mensaje) es, y su [`indice::Abrir`] es el pedido `abrir` del protocolo
//!   que lo ensena (el mismo de Flow Launcher).
//! - **Los chats compartidos no se mezclan** ([`indice::Alcance`]): desde un
//!   chat compartido solo se busca en el. Lo filtra el propio indice, no el
//!   modelo.
//!
//! ## Las piezas
//!
//! - [`texto`]: las palabras (minusculas, sin tildes, sin vacias, raiz
//!   ligera). Igual en la pregunta y en el texto.
//! - [`indice`]: fuentes, trozos, BM25, la puesta al dia y el disco.
//! - [`herramienta`]: `buscar_en_mis_cosas`, su validacion y las citas.
//!
//! Que hay que indexar y como se lee cada formato lo sabe la app
//! (`apps/pixpin/src/asistente/documentos.rs`): aqui no se lee ni un PDF.
//!
//! ## Android, igual
//!
//! Todo lo de este modulo es texto y cuentas, sin nada de Windows: el movil
//! lo porta tal cual a Kotlin (las mismas reglas de palabras de `texto.rs`,
//! los mismos trozos de 160 palabras con 30 de solape, el mismo BM25 y el
//! mismo JSON en `cacheDir/documentos`). Lo que cambia es quien lee: alli
//! el texto del PDF lo da `PdfRenderer` + el lector de texto que ya usa
//! `PdfAHtml.kt`, los Office sus lectores de `motor/`, y la puesta al dia va
//! en un `WorkManager` con `setRequiresCharging`/prioridad baja en vez de un
//! hilo con `THREAD_MODE_BACKGROUND`. La herramienta es la misma del
//! catalogo comun (`catalogo_json`) y el resultado abre con su `pixpin://`.

pub mod herramienta;
pub mod indice;
pub mod texto;

pub use herramienta::{BUSCAR, Cita, Citas, PeticionBusqueda};
pub use indice::{Abrir, Alcance, Firma, Fuente, Hallazgo, Indice, Informe, Lector, Tipo, Unidad};

#[cfg(test)]
mod pruebas;
