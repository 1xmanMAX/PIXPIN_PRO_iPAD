//! pixpin-model — ver docs/superpowers/specs/2026-08-09-pixpin-pc-master-design.md
//!
//! Este crate no puede contener `unsafe`. Si alguna vez lo necesitara, seria
//! señal de que la frontera de capas se ha roto y hay que arreglar el diseño,
//! no relajar la regla.
#![forbid(unsafe_code)]
