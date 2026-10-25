//! **Medir el texto del lienzo con las letras de verdad** (Core Text en el
//! iPad, DirectWrite en Windows: `dibujo::pintar::medir_para_el_motor`).
//!
//! El motor es puro y, sin medidor, mide «a ojo» (un ancho medio por
//! caracter): la caja de un texto en Caveat o en Lilita One no era la de lo
//! escrito, y el cursor, la seleccion y el centrado caian fuera. Swift
//! instala aqui su medidor (el mismo `MedidorLetras` del lector de Office)
//! una vez al arrancar y el motor lo usa en todos los lienzos.
//!
//! Lo que se mide es lo que se pinta: cada renglon es la suma de los avances
//! de sus letras, y el alto, los renglones por el interlineado fijo de la
//! familia (el de `texto::interlineado_de`, el mismo con que se pintan) o,
//! en las del sistema, por el de su fuente.

use std::sync::OnceLock;

use pixpin_motor2d::texto::EstiloDeTexto;
use pixpin_office::{Letra, Medidor};

use crate::office::{MedidorConCache, MedidorLetras};

static MEDIDOR: OnceLock<MedidorConCache> = OnceLock::new();

/// **Instala el medidor de letras del lienzo** (una vez; las demas no hacen
/// nada) y se lo da al motor.
#[uniffi::export]
pub fn instalar_medidor_del_lienzo(medidor: Box<dyn MedidorLetras>) {
    if MEDIDOR.set(MedidorConCache::nuevo(medidor)).is_ok() {
        pixpin_motor2d::texto::instalar_medidor(medir_para_el_motor);
    }
}

/// Ancho y alto de `texto` a `tam` con un medidor (`None` si no hay letra).
fn medir_con(m: &dyn Medidor, texto: &str, tam: f32, familia: &str, estilo: EstiloDeTexto) -> Option<(f32, f32)> {
    if !(tam.is_finite() && tam > 0.0) {
        return None;
    }
    let letra = Letra {
        familia: std::sync::Arc::from(familia),
        tam,
        negrita: estilo.negrita,
        cursiva: estilo.cursiva,
    };
    let mut avances = Vec::new();
    let mut ancho: f32 = 0.0;
    let mut renglones = 0usize;
    for r in texto.split('\n') {
        avances.clear();
        m.avances(r, &letra, &mut avances);
        ancho = ancho.max(avances.iter().sum());
        renglones += 1;
    }
    let paso = match pixpin_motor2d::texto::interlineado_de(familia) {
        Some(k) => k * tam,
        None => {
            let me = m.metricas(&letra);
            (me.ascenso + me.descenso + me.hueco) * tam
        }
    };
    let alto = paso * renglones.max(1) as f32;
    (ancho.is_finite() && alto.is_finite()).then_some((ancho, alto))
}

fn medir_para_el_motor(texto: &str, tam: f32, familia: &str, estilo: EstiloDeTexto) -> Option<(f32, f32)> {
    let m = MEDIDOR.get()?;
    medir_con(m, texto, tam, familia, estilo)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn mide_por_renglones_con_el_interlineado_de_la_familia() {
        let m = pixpin_office::MedidorFijo::default();
        let (a1, h1) = medir_con(&m, "hola", 20.0, "Nunito", EstiloDeTexto::default()).unwrap();
        let (a2, h2) = medir_con(&m, "hola\nhola mundo", 20.0, "Nunito", EstiloDeTexto::default()).unwrap();
        assert!(a2 > a1);
        assert!((h2 - 2.0 * h1).abs() < 1e-3);
        assert!(medir_con(&m, "x", 0.0, "Nunito", EstiloDeTexto::default()).is_none());
    }
}
