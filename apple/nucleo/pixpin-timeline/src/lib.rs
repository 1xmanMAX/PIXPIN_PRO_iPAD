//! **El timeline** (5-oct-2026): lo que pasa en el dia, apuntado en el
//! momento, con la voz, a mano o con una foto, y visto como una linea de
//! tiempo vertical.
//!
//! Lo pidio el usuario asi: «que me permita registrar todo lo que pasa dia a
//! dia minuto a minuto de forma rapida y sencilla […] como un chat que se
//! llena super rapido, con una UI diferente al chat; solo voz, texto e
//! imagenes; que se muestren solo las ultimas 24 horas, lo demas se archive y
//! se pueda buscar viendo el calendario; y exportar las ultimas 24 horas o
//! los dias que elija en HTML o PDF, con el mismo formato de timeline».
//!
//! En PixPin Android no hay nada igual (buscado en el Kotlin actual el
//! 5-oct): esto es nuevo, y por eso vive aparte del chat y no ensucia el
//! cuaderno que se sincroniza.
//!
//! Lo que no depende de ventanas:
//!
//! - [`momento`]: un momento y su JSON (una linea por momento).
//! - [`almacen`]: el fichero `timeline/momentos.jsonl` y sus fotos y audios.
//! - [`frases`]: partir lo dicho en titulo y descripcion por las frases
//!   clave («con esto paso…», «y asi te lo cuento…»).
//! - [`dias`]: las ultimas 24 horas, los dias con algo y la rejilla del mes.
//! - [`html`]: la pagina que se exporta (y que se imprime a PDF).
//! - [`buscar`]: el buscador de la barra de arriba, en todo el timeline.
//! - [`emoticonos`]: sacar un emoticono entero (con ZWJ, variantes y
//!   banderas) para el circulo de cada dia de «Estado».
//! - [`resumen`]: lo que mas se repitio en un mes («Sobre todo «X»»).
//! - [`estilo`]: los degradados y la fecha de las historias.
//! - [`archivo`]: lo pasado por ano, mes y dia (pestanas «Momentos» y
//!   «Estado», copiadas de WeChat).

#![forbid(unsafe_code)]

pub mod almacen;
pub mod archivo;
pub mod buscar;
pub mod dias;
pub mod emoticonos;
pub mod estilo;
pub mod frases;
pub mod html;
pub mod momento;
pub mod resumen;

pub use momento::Momento;
