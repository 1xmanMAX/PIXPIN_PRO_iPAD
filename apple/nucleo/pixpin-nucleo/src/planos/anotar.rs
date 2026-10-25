//! **Anotar sobre el plano con el motor del lienzo** (`plano_cad::anotar`
//! de Windows, A): la misma barra, el mismo panel y las mismas teclas que
//! el lienzo, sobre la capa del plano (`anot-<uid>` si es de un chat, la de
//! la cache si no), como PixPin Android v0.114: «anotar en el visor de
//! planos usa las herramientas del motor del lienzo, sin herramientas
//! propias».
//!
//! **Las unidades**: Windows lleva la capa en memoria en la unidad de la
//! letra del plano y la devuelve al guardar a la del fichero. Aqui el
//! editor la lleva **tal como esta en el fichero** (la de su `.hoja`; sin
//! ella, la de antes si ya habia capa, o la de la letra si es nueva, lo
//! mismo que lee `encaje_del_fichero`): una capa nueva queda igual que en
//! Windows y una del movil no se reescribe en otra unidad. Swift pone la
//! vista del lienzo con ese encaje encima del plano.
//!
//! **La `.hoja` va despues de la capa**, como en Windows: si el plano es de
//! un chat, el propio editor la escribe tras cada guardado (`HojaDoc`, la
//! de los lectores); si no, [`Plano::poner_hoja`] cuando Swift ve que se
//! guardo. Un plano en el que no se anota nada no deja ficheros.

use std::sync::Arc;

use pixpin_cad::anotado::Encaje;
use pixpin_sincro::anotado::MarcoDeLaHoja;

use super::{EncajePlano, Plano, escribir_hoja};
use crate::ErrorNucleo;
use crate::editor::{EditorLienzo, HojaDoc};

impl Plano {
    /// La `.hoja` de este encaje (la caja del plano en unidades de la capa).
    fn hoja_de(&self, e: &Encaje) -> [f64; 4] {
        e.hoja(self.caja_ref)
    }
}

#[uniffi::export]
impl Plano {
    /// **La capa del plano para dibujar en ella** con el editor del lienzo
    /// (sus herramientas, deshacer, guardar solo). Las cotas y los marcos
    /// tambien estan (son figuras de la capa): borrarlos o moverlos aqui
    /// cambia los del visor, como en Windows. Al terminar, Swift vuelve a
    /// leer [`Plano::anotado`].
    pub fn anotar(&self) -> Result<Arc<EditorLienzo>, ErrorNucleo> {
        let h = self.hoja_de(&self.encaje_del_fichero());
        let marco = MarcoDeLaHoja::nuevo(h[0] as f32, h[1] as f32, h[2] as f32, h[3] as f32);
        let ed = EditorLienzo::de_hoja(
            self.rutas.tinta.clone(),
            crate::unidades::Unidades::DEL_PC,
            HojaDoc {
                marco: self.rutas.base.clone().map(|b| (b, marco)),
                registrar: None,
            },
        )?;
        Ok(Arc::new(ed))
    }

    /// **Escribe la `.hoja` de la capa con `encaje`** (el que se leyo al
    /// empezar a anotar), si ya hay capa. Solo hace falta en un plano que no
    /// es de ningun chat; en los demas no cambia nada (ya la escribio el
    /// editor). Dice si la escribio o ya estaba.
    pub fn poner_hoja(&self, encaje: EncajePlano) -> Result<bool, ErrorNucleo> {
        if !self.rutas.tinta.is_file() || !(encaje.u.is_finite() && encaje.u > 0.0) {
            return Ok(false);
        }
        let e = Encaje {
            u: encaje.u,
            cero_x: encaje.cero_x - self.referencia[0],
            cero_y: encaje.cero_y - self.referencia[1],
        };
        escribir_hoja(&self.rutas.hoja, self.hoja_de(&e))?;
        Ok(true)
    }
}

#[cfg(test)]
mod pruebas {
    use super::super::leer_hoja;
    use super::super::pruebas::abrir;

    #[test]
    fn lo_anotado_con_el_lienzo_cae_en_la_capa_del_plano_con_su_hoja() {
        let (d, p) = abrir("anotar");
        let antes = p.encaje();
        let ed = p.anotar().unwrap();
        // Sin dibujar nada no hay ficheros.
        assert!(!p.rutas.tinta.exists());
        assert!(!p.poner_hoja(antes).unwrap());
        // Una raya con el lapiz.
        ed.herramienta("lapiz".into()).unwrap();
        use crate::editor::FasePuntero::{Mover, Pulsar, Soltar};
        ed.puntero(Pulsar, 0.0, 0.0, None, 0, 1.0);
        ed.puntero(Mover, 30.0, 10.0, None, 0, 1.0);
        ed.puntero(Soltar, 40.0, 20.0, None, 0, 1.0);
        assert!(ed.guardar().unwrap());
        assert!(p.rutas.tinta.is_file());
        assert!(p.poner_hoja(antes).unwrap());
        // La hoja dice el encaje de cuando se empezo: se lee el mismo.
        assert!(leer_hoja(&p.rutas.hoja).is_some());
        let despues = p.encaje();
        assert!((despues.u - antes.u).abs() < 1e-9);
        assert!(
            (despues.cero_x - antes.cero_x).abs() < 1e-6
                && (despues.cero_y - antes.cero_y).abs() < 1e-6
        );
        // Lo dibujado no es cota ni marco.
        let a = p.anotado();
        assert!(a.cotas.is_empty() && a.marcos.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
