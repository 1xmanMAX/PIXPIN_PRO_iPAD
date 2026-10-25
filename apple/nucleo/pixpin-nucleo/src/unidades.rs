//! **Las unidades de la tinta de una hoja** (la de un PDF o un Word del
//! chat): copia de `Unidades`, `llevar` y `con_resaltadores` de
//! `por-traducir/pixpin/src/lector_tinta.rs`, sin ventana.
//!
//! El lector cuenta cada hoja con `pixpin_docs::vista::ANCHO_HOJA` (1400) de
//! ancho desde su esquina. El fichero `anot-<uid>-p<n>` puede venir en otras
//! (las de la capa del movil, segun sus espacios) y entonces su marco
//! (`anot-<uid>-p<n>.hoja`) dice cuales; sin marco, la regla de antes
//! (`pixpin_docs::vista::capa_del_movil`). Al abrir se trae todo a las del
//! lector; al guardar se escribe en las del lector con el marco que lo dice,
//! `0,0,1400,alto`, que es lo que hacen Windows y Android desde v0.98.0.

use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::vector::Punto2;
use pixpin_sincro::anotado::MarcoDeLaHoja;

/// El grosor del resaltador del lector: cubre un renglon de texto (ver
/// `GROSOR_RESALTADOR` de `lector_tinta.rs`). Tambien es lo que lo distingue
/// de un lapiz al leer el fichero.
pub const GROSOR_RESALTADOR: f32 = 8.0;
/// Lo que pesaba con la tinta vieja de grosor constante.
const GROSOR_RESALTADOR_VIEJO: f32 = 16.0;

/// `fichero = lector * ex + dx` a lo ancho y `fichero = lector * ey + dy` a
/// lo alto (ver `lector_tinta::Unidades`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unidades {
    pub ex: f32,
    pub ey: f32,
    pub dx: f32,
    pub dy: f32,
}

impl Unidades {
    /// Las mismas: lo que solo escribe y lee el lector.
    pub const DEL_PC: Unidades = Unidades {
        ex: 1.0,
        ey: 1.0,
        dx: 0.0,
        dy: 0.0,
    };

    /// La misma escala en los dos ejes y el cero corrido a lo ancho.
    pub fn uniformes(escala: f32, dx: f32) -> Unidades {
        Unidades {
            ex: escala,
            ey: escala,
            dx,
            dy: 0.0,
        }
    }

    /// La de una hoja de PDF del movil con sus espacios puestos.
    pub fn de_la_capa_del_movil(espacios: u8) -> Unidades {
        let (escala, dx) = pixpin_docs::vista::capa_del_movil(espacios);
        Unidades::uniformes(escala, dx)
    }

    /// Las que dice el marco del fichero: llevan `propia` (la hoja en
    /// unidades del lector) a `del_fichero`. `None` si alguno no sirve.
    pub fn del_marco(propia: &MarcoDeLaHoja, del_fichero: &MarcoDeLaHoja) -> Option<Unidades> {
        if !propia.valido() || !del_fichero.valido() {
            return None;
        }
        let (ex, ey, dx, dy) = del_fichero.desde(propia);
        Some(Unidades { ex, ey, dx, dy })
    }

    /// Donde cae la hoja `propia` en estas unidades: el marco que hay que
    /// escribir junto a la tinta.
    pub fn marco_de(&self, propia: &MarcoDeLaHoja) -> MarcoDeLaHoja {
        MarcoDeLaHoja::nuevo(
            propia.x0 * self.ex + self.dx,
            propia.y0 * self.ey + self.dy,
            propia.x1 * self.ex + self.dx,
            propia.y1 * self.ey + self.dy,
        )
    }

    /// La escala de lo que no tiene eje (grosor, letra).
    pub fn media(&self) -> f32 {
        (self.ex * self.ey).abs().sqrt()
    }

    /// Las que deshacen estas.
    pub fn inversas(&self) -> Unidades {
        Unidades {
            ex: 1.0 / self.ex,
            ey: 1.0 / self.ey,
            dx: -self.dx / self.ex,
            dy: -self.dy / self.ey,
        }
    }

    pub fn son_las_del_pc(&self) -> bool {
        *self == Unidades::DEL_PC
    }
}

/// La hoja `alto` en unidades del lector: 1400 de ancho desde su esquina.
pub fn hoja_propia(alto: f32) -> MarcoDeLaHoja {
    MarcoDeLaHoja::nuevo(0.0, 0.0, pixpin_docs::vista::ANCHO_HOJA, alto)
}

/// Corre un elemento sin subir su version (correr al leer no es cambiarlo).
fn correr(e: &mut Elemento, dx: f32, dy: f32) {
    let version = e.version;
    e.mover(dx, dy);
    e.version = version;
}

/// **Lleva un elemento a las unidades `u`**, con su grosor y su letra a la
/// escala media. Sin subir la version, como Windows.
pub fn llevar(e: &mut Elemento, u: Unidades) {
    let Unidades { ex, ey, dx, dy } = u;
    if ex == 1.0 && ey == 1.0 {
        correr(e, dx, dy);
        return;
    }
    let k = u.media();
    let version = e.version;
    let mapa = |q: &mut Punto2| {
        q.x = q.x * ex + dx;
        q.y = q.y * ey + dy;
    };
    match &mut e.figura {
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. }
        | Figura::Cota { puntos } => puntos.iter_mut().for_each(mapa),
        Figura::Region { contorno, huecos } => contorno
            .iter_mut()
            .chain(huecos.iter_mut().flatten())
            .for_each(mapa),
        Figura::Texto { tam, .. } => *tam *= k,
        _ => {}
    }
    e.x = e.x * ex + dx;
    e.y = e.y * ey + dy;
    e.ancho *= ex;
    e.alto *= ey;
    e.grosor *= k;
    if let Some(t) = e.extras.tam_letra.as_mut() {
        *t *= k;
    }
    e.version = version;
}

/// **El trazo gordo y translucido vuelve a ser resaltador** al leer: el
/// fichero lo guarda como un trazo mas, con su opacidad.
pub fn con_resaltadores(escena: &mut Escena) {
    let alfa = pixpin_motor2d::tinta::OPACIDAD_DEL_RESALTADOR;
    for e in escena.elementos.iter_mut() {
        let es_resaltador = e.opacidad <= alfa + 0.01 && e.grosor >= GROSOR_RESALTADOR * 0.5;
        if let (true, Figura::Lapiz { puntos, .. }) = (es_resaltador, &mut e.figura) {
            let puntos = std::mem::take(puntos);
            e.figura = Figura::Resaltador { puntos };
            e.opacidad = (e.opacidad / alfa).min(1.0);
            if e.grosor >= GROSOR_RESALTADOR_VIEJO * 0.9 {
                e.grosor = GROSOR_RESALTADOR;
            }
        }
    }
}

/// Trae toda la escena de las unidades del fichero `u` a las del lector.
pub fn a_las_del_lector(escena: &mut Escena, u: Unidades) {
    if u.son_las_del_pc() {
        return;
    }
    let inv = u.inversas();
    for e in escena.elementos.iter_mut() {
        llevar(e, inv);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_inversas_deshacen() {
        let u = Unidades::de_la_capa_del_movil(0);
        let v = u.inversas();
        let x = 700.0 * u.ex + u.dx;
        assert!((x * v.ex + v.dx - 700.0).abs() < 1e-3);
        // Sin espacios, el movil reparte 2,5 veces la hoja.
        assert!((u.ex - 2.5).abs() < 1e-5);
        assert_eq!(Unidades::de_la_capa_del_movil(3), Unidades::DEL_PC);
    }

    #[test]
    fn el_marco_de_la_hoja_propia_es_el_del_lector() {
        let m = hoja_propia(1980.0);
        assert_eq!(m.a_texto(), "0,0,1400,1980\nv1\n");
        let u = Unidades::del_marco(&m, &m).unwrap();
        assert!(u.son_las_del_pc());
        // Caso negativo: un marco sin area no sirve.
        assert!(Unidades::del_marco(&m, &MarcoDeLaHoja::nuevo(0.0, 0.0, 0.0, 5.0)).is_none());
    }
}
