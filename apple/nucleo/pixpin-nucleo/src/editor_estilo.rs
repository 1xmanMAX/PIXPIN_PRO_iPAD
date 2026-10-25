//! **El estilo de la barra** del editor de Swift: color del trazo, fondo,
//! como se rellena, grosor, linea continua o a trazos, temblor y opacidad.
//!
//! Es el panel lateral de Windows (`panel_dibujo::aplicar` con
//! `AccionPanel::Estilo`): el cambio queda SIEMPRE como el «actual» con el
//! que nace lo proximo (`Gesto::estilo`, y el grosor del lapiz en
//! `grosor_tinta`), y si hay algo elegido se le aplica en un paso de deshacer
//! (`estilo::aplicar_a`), solo a lo que admite ese mando
//! (`estilo::propiedades_de`, la tabla del movil).

use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo};
use pixpin_motor2d::estilo::{self, CambioEstilo, NivelGrosor, Propiedad};
use pixpin_motor2d::relleno::EstiloRelleno;
use pixpin_motor2d::seleccion::Seleccion;

use crate::editor::EditorLienzo;

/// Un color con sus cuatro canales en `0..1`.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct ColorEstilo {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// Los tres grosores de Excalidraw (`strokeWidth` 1, 2 y 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GrosorEstilo {
    Fino,
    Medio,
    Grueso,
}

/// Continua, a trazos o de puntos (`strokeStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LineaEstilo {
    Continua,
    Discontinua,
    Punteada,
    /// Raya y punto, la de los ejes de los planos (Windows, 17-oct).
    RayaPunto,
}

/// Como se pinta el fondo (`fillStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RellenoEstilo {
    Solido,
    Rayado,
    Cruzado,
    Zigzag,
    /// El rayado limpio propio de PixPin (`lineaspixpin`).
    Lineas,
}

/// Lo que ensena la barra: el estilo de lo elegido (el primero) o, sin
/// nada elegido, el «actual» con el que nace lo proximo.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct EstiloLienzo {
    pub trazo: ColorEstilo,
    /// `None`: transparente.
    pub fondo: Option<ColorEstilo>,
    pub relleno: RellenoEstilo,
    pub grosor: GrosorEstilo,
    pub linea: LineaEstilo,
    /// 0 arquitecto, 1 artista, 2 caricaturista.
    pub rugosidad: f32,
    /// 0..1.
    pub opacidad: f32,
}

/// Un mando de la barra pulsado.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum CambioDeEstilo {
    Trazo { color: ColorEstilo },
    Fondo { color: Option<ColorEstilo> },
    Relleno { relleno: RellenoEstilo },
    Grosor { grosor: GrosorEstilo },
    Linea { linea: LineaEstilo },
    Rugosidad { valor: f32 },
    Opacidad { valor: f32 },
}

fn color_de(c: ColorEstilo) -> ColorRgba {
    let f = |v: f32| {
        if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    ColorRgba {
        r: f(c.r),
        g: f(c.g),
        b: f(c.b),
        a: f(c.a),
    }
}

fn color_hacia(c: ColorRgba) -> ColorEstilo {
    ColorEstilo {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

fn grosor_de(g: GrosorEstilo) -> NivelGrosor {
    match g {
        GrosorEstilo::Fino => NivelGrosor::Fino,
        GrosorEstilo::Medio => NivelGrosor::Medio,
        GrosorEstilo::Grueso => NivelGrosor::Grueso,
    }
}

fn grosor_hacia(g: NivelGrosor) -> GrosorEstilo {
    match g {
        NivelGrosor::Fino => GrosorEstilo::Fino,
        NivelGrosor::Medio => GrosorEstilo::Medio,
        NivelGrosor::Grueso => GrosorEstilo::Grueso,
    }
}

fn linea_de(l: LineaEstilo) -> EstiloTrazo {
    match l {
        LineaEstilo::Continua => EstiloTrazo::Solido,
        LineaEstilo::Discontinua => EstiloTrazo::Discontinuo,
        LineaEstilo::Punteada => EstiloTrazo::Punteado,
        LineaEstilo::RayaPunto => EstiloTrazo::RayaPunto,
    }
}

fn linea_hacia(l: EstiloTrazo) -> LineaEstilo {
    match l {
        EstiloTrazo::Solido => LineaEstilo::Continua,
        EstiloTrazo::Discontinuo => LineaEstilo::Discontinua,
        EstiloTrazo::Punteado => LineaEstilo::Punteada,
        EstiloTrazo::RayaPunto => LineaEstilo::RayaPunto,
    }
}

fn relleno_de(r: RellenoEstilo) -> EstiloRelleno {
    match r {
        RellenoEstilo::Solido => EstiloRelleno::Solido,
        RellenoEstilo::Rayado => EstiloRelleno::Rayado,
        RellenoEstilo::Cruzado => EstiloRelleno::Cruzado,
        RellenoEstilo::Zigzag => EstiloRelleno::Zigzag,
        RellenoEstilo::Lineas => EstiloRelleno::LineasPixpin,
    }
}

fn relleno_hacia(r: EstiloRelleno) -> RellenoEstilo {
    match r {
        EstiloRelleno::Solido => RellenoEstilo::Solido,
        EstiloRelleno::Rayado => RellenoEstilo::Rayado,
        EstiloRelleno::Cruzado => RellenoEstilo::Cruzado,
        EstiloRelleno::Zigzag => RellenoEstilo::Zigzag,
        EstiloRelleno::LineasPixpin => RellenoEstilo::Lineas,
    }
}

/// El cambio del motor y la propiedad de la tabla que lo admite.
fn al_motor(c: CambioDeEstilo) -> (CambioEstilo, Propiedad) {
    match c {
        CambioDeEstilo::Trazo { color } => (CambioEstilo::Trazo(color_de(color)), Propiedad::Trazo),
        CambioDeEstilo::Fondo { color } => {
            (CambioEstilo::Relleno(color.map(color_de)), Propiedad::Fondo)
        }
        CambioDeEstilo::Relleno { relleno } => (
            CambioEstilo::EstiloRelleno(relleno_de(relleno)),
            Propiedad::Relleno,
        ),
        CambioDeEstilo::Grosor { grosor } => {
            (CambioEstilo::Grosor(grosor_de(grosor)), Propiedad::Grosor)
        }
        CambioDeEstilo::Linea { linea } => {
            (CambioEstilo::Estilo(linea_de(linea)), Propiedad::Linea)
        }
        CambioDeEstilo::Rugosidad { valor } => (
            CambioEstilo::Rugosidad(if valor.is_finite() { valor } else { 1.0 }),
            Propiedad::Rugosidad,
        ),
        CambioDeEstilo::Opacidad { valor } => (
            CambioEstilo::Opacidad(if valor.is_finite() { valor } else { 1.0 }),
            Propiedad::Opacidad,
        ),
    }
}

/// El estilo de un elemento como lo ensena la barra.
fn de_elemento(e: &Elemento) -> EstiloLienzo {
    EstiloLienzo {
        trazo: color_hacia(e.trazo),
        fondo: e.relleno.map(color_hacia),
        relleno: relleno_hacia(e.estilo_relleno),
        grosor: grosor_hacia(NivelGrosor::de_elemento(&e.figura, e.grosor)),
        linea: linea_hacia(e.estilo),
        rugosidad: e.rugosidad,
        opacidad: e.opacidad,
    }
}

#[uniffi::export]
impl EditorLienzo {
    /// **El estilo que ensena la barra**: el de lo elegido (el primero que
    /// siga vivo) o el «actual» con el que nace lo proximo.
    pub fn estilo(&self) -> EstiloLienzo {
        let st = self.tomar();
        if let Some(e) = st
            .gesto
            .seleccion
            .ids()
            .iter()
            .find_map(|id| st.escena.buscar(*id).filter(|e| !e.borrado))
        {
            return de_elemento(e);
        }
        let s = &st.gesto.estilo;
        EstiloLienzo {
            trazo: color_hacia(s.trazo),
            fondo: s.relleno.map(color_hacia),
            relleno: relleno_hacia(s.estilo_relleno),
            grosor: grosor_hacia(s.grosor),
            linea: linea_hacia(s.estilo),
            rugosidad: s.rugosidad,
            opacidad: s.opacidad,
        }
    }

    /// **Un mando de la barra**: queda como el estilo de lo proximo y, si
    /// hay algo elegido que lo admite, se le aplica en un paso de deshacer.
    /// Devuelve si cambio el dibujo (entonces hay que guardar).
    pub fn cambiar_estilo(&self, cambio: CambioDeEstilo) -> bool {
        let hecho = self.cambiar_estilo_sin_escala(cambio);
        self.poner_a_escala() || hecho
    }

    fn cambiar_estilo_sin_escala(&self, cambio: CambioDeEstilo) -> bool {
        let (c, propiedad) = al_motor(cambio);
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        st.gesto.estilo.aplicar(c);
        if let CambioEstilo::Grosor(n) = c {
            st.gesto.grosor_tinta = n.de_tinta();
        }
        let mut destino = Seleccion::nueva();
        destino.poner_todos(st.gesto.seleccion.ids().iter().copied().filter(|id| {
            st.escena
                .buscar(*id)
                .is_some_and(|e| estilo::propiedades_de(&e.figura).contains(&propiedad))
        }));
        let pasos = st.escena.pasos_cerrados();
        if !destino.esta_vacia() && st.gesto.en_reposo() {
            estilo::aplicar_a(&mut st.escena, &destino, c);
        }
        let cambio_dibujo = st.escena.pasos_cerrados() != pasos;
        drop(guarda);
        if cambio_dibujo {
            self.subir_cambios();
        }
        self.subir_version();
        cambio_dibujo
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_conversiones_van_y_vuelven() {
        for g in [
            GrosorEstilo::Fino,
            GrosorEstilo::Medio,
            GrosorEstilo::Grueso,
        ] {
            assert_eq!(grosor_hacia(grosor_de(g)), g);
        }
        for l in [
            LineaEstilo::Continua,
            LineaEstilo::Discontinua,
            LineaEstilo::Punteada,
            LineaEstilo::RayaPunto,
        ] {
            assert_eq!(linea_hacia(linea_de(l)), l);
        }
        for r in [
            RellenoEstilo::Solido,
            RellenoEstilo::Rayado,
            RellenoEstilo::Cruzado,
            RellenoEstilo::Zigzag,
            RellenoEstilo::Lineas,
        ] {
            assert_eq!(relleno_hacia(relleno_de(r)), r);
        }
    }

    #[test]
    fn un_color_fuera_de_rango_se_recorta() {
        let c = color_de(ColorEstilo {
            r: 2.0,
            g: -1.0,
            b: f32::NAN,
            a: 0.5,
        });
        assert_eq!((c.r, c.g, c.b, c.a), (1.0, 0.0, 0.0, 0.5));
    }
}
