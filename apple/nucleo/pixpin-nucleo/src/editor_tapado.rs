//! **Lo que el mosaico y la lupa ensenan dentro**: el pixelado de lo que
//! tapa un mosaico y lo agrandado que mira una lupa.
//!
//! En Windows son dos pasadas del editor (`ventana_editor/tapar.rs` y
//! `lupas.rs`) que pintan con Direct2D despues de la escena. Aqui el que
//! pinta es Swift con el pintor de siempre, asi que se hace con lo que ese
//! pintor ya sabe hacer: **una imagen**. En el plano de la vista, justo
//! despues de las ordenes del mosaico (su banda maciza) o de la lupa (su
//! montura), va una `Orden::Imagen` con un `id_objeto` que no es de ninguna
//! foto ([`id_de_mosaico`], [`id_de_lupa`]); Swift la resuelve pidiendo
//! [`EditorLienzo::contenido_de_tapado`] y pintando ese plano a un mapa
//! (en cuadros para el mosaico, agrandado y recortado al cristal para la
//! lupa).
//!
//! Va en su sitio del orden de pintado, como en el movil: el mosaico tapa lo
//! que hay DEBAJO, y una flecha dibujada despues, senalando el dato tapado,
//! se sigue viendo encima. Y si la imagen no llegara a pintarse, debajo
//! sigue la banda maciza de `pintado.rs`: tapa igual.

use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::lupa_elemento as lupa;
use pixpin_motor2d::pintado::{self, Orden};

use crate::editor::EditorLienzo;
use crate::lienzo::{LienzoPlano, empaquetar};

/// El bit de los `id_objeto` de un mosaico: las fotos van por debajo de
/// 2^48 y los ids de la escena por debajo de 2^62 (`ids_nuevos_desde`).
const BIT_MOSAICO: u64 = 1 << 63;
/// El de una lupa.
const BIT_LUPA: u64 = 1 << 62;

/// El `id_objeto` de la imagen que tapa el mosaico `id`.
pub fn id_de_mosaico(id: u64) -> u64 {
    BIT_MOSAICO | id
}

/// El de la imagen de dentro de la lupa `id`.
pub fn id_de_lupa(id: u64) -> u64 {
    BIT_LUPA | id
}

/// Que elemento y de que clase nombra un `id_objeto`, si es de estos.
fn de_id(id_objeto: u64) -> Option<(u64, bool)> {
    if id_objeto & BIT_MOSAICO != 0 {
        Some((id_objeto & !BIT_MOSAICO, true))
    } else if id_objeto & BIT_LUPA != 0 {
        Some((id_objeto & !BIT_LUPA, false))
    } else {
        None
    }
}

/// **Lo que va despues de las ordenes de `e`** en el plano de la vista: la
/// imagen del pixelado o de lo agrandado y, en la lupa, la montura otra vez
/// encima (la imagen se come su mitad de dentro). Vacio si `e` no es uno de
/// los dos. El mosaico girado se queda con su banda maciza: tapa igual.
pub fn ordenes_tras(e: &Elemento) -> Vec<Orden> {
    if e.borrado {
        return Vec::new();
    }
    match &e.figura {
        Figura::Mosaico { .. } if e.angulo == 0.0 && e.ancho > 0.0 && e.alto > 0.0 => {
            let (x0, y0, x1, y1) = pixpin_motor2d::mosaico::caja_girada(e);
            vec![Orden::Imagen {
                id_objeto: id_de_mosaico(e.id),
                x: x0,
                y: y0,
                ancho: x1 - x0,
                alto: y1 - y0,
                opacidad: 1.0,
                recorte: None,
                angulo: 0.0,
            }]
        }
        Figura::Lupa { .. } => {
            let (x, y, ancho, alto) = lupa::caja_de(e);
            let mut v = vec![Orden::Imagen {
                id_objeto: id_de_lupa(e.id),
                x,
                y,
                ancho,
                alto,
                opacidad: 1.0,
                recorte: None,
                angulo: 0.0,
            }];
            v.extend(pintado::ordenes(e).into_iter().next());
            v
        }
        _ => Vec::new(),
    }
}

/// **Lo de dentro de un mosaico o de una lupa**, para pintarlo en un mapa.
///
/// La imagen cubre la caja `(x, y, ancho, alto)` del mundo. Dentro, un
/// punto `q` del dibujo cae en `centro + (q - foco) * aumento`, con
/// `centro` el de la caja (en el mosaico, `foco` es ese mismo centro y
/// `aumento` 1). `contorno` es el recorte (x, y, x, y... del mundo; vacio en
/// el mosaico) y `grano` el lado del cuadro del pixelado en unidades del
/// mundo (0 en la lupa).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ContenidoTapado {
    pub plano: LienzoPlano,
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
    pub foco_x: f32,
    pub foco_y: f32,
    pub aumento: f32,
    pub contorno: Vec<f32>,
    pub grano: f32,
    /// `mosaicBlur` del movil: con el puesto, mancha en vez de cuadros.
    pub desenfoque: bool,
}

/// Si dos cajas `(x0, y0, x1, y1)` se tocan.
fn se_cortan(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    a.0 <= b.2 && a.2 >= b.0 && a.1 <= b.3 && a.3 >= b.1
}

/// Lo que se pinta dentro: lo que no es otra ventana (lupas, focos y
/// mosaicos, que se pintarian a si mismos) y cae en `mira`.
fn se_ve_dentro(e: &Elemento, mira: (f32, f32, f32, f32)) -> bool {
    if e.borrado
        || matches!(
            e.figura,
            Figura::Lupa { .. } | Figura::Foco { .. } | Figura::Mosaico { .. }
        )
    {
        return false;
    }
    let (x0, y0, x1, y1) = e.caja();
    // Con holgura por el grueso del trazo, que se sale de la caja.
    let h = e.grosor * 2.0;
    se_cortan((x0 - h, y0 - h, x1 + h, y1 + h), mira)
}

/// Las ordenes de `e` con sus numeros de cota, si los tiene.
fn ordenes_enteras(escena: &Escena, e: &Elemento) -> Vec<Orden> {
    let mut v = pintado::ordenes(e);
    v.extend(pintado::ordenes_medibles(
        e,
        escena.escala.as_ref(),
        ',',
        escena.fondo,
    ));
    v
}

/// Lo de dentro del elemento que nombra `id_objeto` (ver [`ContenidoTapado`]).
pub fn contenido(escena: &Escena, id_objeto: u64) -> Option<ContenidoTapado> {
    let (id, es_mosaico) = de_id(id_objeto)?;
    let indice = escena
        .elementos
        .iter()
        .position(|e| e.id == id && !e.borrado)?;
    let e = &escena.elementos[indice];
    if es_mosaico {
        let t = pixpin_motor2d::mosaico::tapado_de(e)?;
        let (x0, y0, x1, y1) = t.caja;
        // Lo de DEBAJO: lo que se pinta antes que el.
        let ordenes: Vec<Orden> = escena.elementos[..indice]
            .iter()
            .filter(|o| se_ve_dentro(o, t.caja))
            .filter(|o| escena.referencias_visibles || !o.extras.referencia)
            .flat_map(|o| ordenes_enteras(escena, o))
            .collect();
        let centro = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        return Some(ContenidoTapado {
            plano: empaquetar(&ordenes, escena.fondo, None),
            x: x0,
            y: y0,
            ancho: x1 - x0,
            alto: y1 - y0,
            foco_x: centro.0,
            foco_y: centro.1,
            aumento: 1.0,
            contorno: Vec::new(),
            grano: t.grano,
            desenfoque: t.desenfoque,
        });
    }
    let cr = lupa::de(e)?;
    let caja = lupa::caja_de(e);
    let aumento = lupa::aumento_de(cr, caja);
    let foco = lupa::foco_de(cr, caja);
    // Lo que cabe en el cristal: centrado en el foco y del tamano del
    // cristal partido por el aumento (`lupas::mirado` de Windows).
    let (mw, mh) = (caja.2.abs() / aumento / 2.0, caja.3.abs() / aumento / 2.0);
    let mira = (foco.x - mw, foco.y - mh, foco.x + mw, foco.y + mh);
    let ordenes: Vec<Orden> = escena
        .elementos
        .iter()
        .filter(|o| se_ve_dentro(o, mira))
        .filter(|o| escena.referencias_visibles || !o.extras.referencia)
        .flat_map(|o| ordenes_enteras(escena, o))
        .collect();
    let contorno = lupa::puntos_del_cristal(cr, caja)
        .into_iter()
        .flat_map(|p| [p.x, p.y])
        .collect();
    Some(ContenidoTapado {
        plano: empaquetar(&ordenes, escena.fondo, None),
        x: caja.0,
        y: caja.1,
        ancho: caja.2,
        alto: caja.3,
        foco_x: foco.x,
        foco_y: foco.y,
        aumento,
        contorno,
        grano: 0.0,
        desenfoque: false,
    })
}

#[uniffi::export]
impl EditorLienzo {
    /// **Lo que ensena dentro un mosaico o una lupa** cuyo `id_objeto` salio
    /// en el plano de la vista (ver la doc del modulo). `None` si ese
    /// `id_objeto` es de una foto de verdad o el elemento ya no esta.
    pub fn contenido_de_tapado(&self, id_objeto: u64) -> Option<ContenidoTapado> {
        contenido(&self.tomar().escena, id_objeto)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::elemento::ColorRgba;

    fn rect(x: f32, color: ColorRgba) -> Elemento {
        Elemento {
            figura: Figura::Rectangulo,
            x,
            y: 0.0,
            ancho: 40.0,
            alto: 40.0,
            trazo: color,
            relleno: Some(color),
            grosor: 2.0,
            rugosidad: 0.0,
            ..Default::default()
        }
    }

    fn mosaico(x: f32) -> Elemento {
        Elemento {
            figura: Figura::Mosaico { desenfoque: false },
            x,
            y: -10.0,
            ancho: 60.0,
            alto: 60.0,
            grosor: 2.0,
            ..Default::default()
        }
    }

    #[test]
    fn el_mosaico_lleva_su_imagen_y_pinta_solo_lo_de_debajo() {
        let mut escena = Escena::nueva();
        let rojo = ColorRgba::opaco(1.0, 0.0, 0.0);
        let azul = ColorRgba::opaco(0.0, 0.0, 1.0);
        escena.anadir(rect(0.0, rojo));
        let m = escena.anadir(mosaico(-10.0));
        // Encima del mosaico: no se pixela.
        escena.anadir(rect(5.0, azul));
        let e = escena.buscar(m).unwrap().clone();
        let tras = ordenes_tras(&e);
        let [Orden::Imagen { id_objeto, .. }] = tras.as_slice() else {
            panic!("{tras:?}");
        };
        let c = contenido(&escena, *id_objeto).expect("contenido");
        assert!(c.grano > 0.0 && c.contorno.is_empty());
        assert_eq!((c.ancho, c.alto), (60.0, 60.0));
        // Solo lo de debajo: el rectangulo rojo, no el azul de encima.
        let otra = Escena::nueva();
        let solo_rojo = {
            let mut s = otra;
            s.anadir(rect(0.0, rojo));
            empaquetar(
                &ordenes_enteras(&s, s.elementos.first().unwrap()),
                s.fondo,
                None,
            )
        };
        assert_eq!(c.plano.datos.len(), solo_rojo.datos.len());
        // Caso negativo: un id de foto no es un tapado.
        assert!(contenido(&escena, 1234).is_none());
    }

    #[test]
    fn la_lupa_mira_su_foco_con_su_aumento_y_recorta_a_su_cristal() {
        let mut escena = Escena::nueva();
        let circulo = escena.anadir(Elemento {
            figura: Figura::Elipse,
            ancho: 100.0,
            alto: 100.0,
            grosor: 2.0,
            ..Default::default()
        });
        escena.anadir(rect(20.0, ColorRgba::opaco(0.0, 0.5, 0.0)));
        let mut gesto = pixpin_motor2d::gesto::Gesto::nuevo();
        let id = crate::editor_herramientas::convertir_en_lupa(&mut escena, &mut gesto, circulo)
            .expect("lupa");
        let e = escena.buscar(id).unwrap().clone();
        let tras = ordenes_tras(&e);
        assert!(matches!(tras[0], Orden::Imagen { .. }));
        assert!(tras.len() == 2, "y la montura otra vez encima");
        let c = contenido(&escena, id_de_lupa(id)).expect("contenido");
        assert!(c.aumento > 1.0);
        assert!(c.contorno.len() >= 16, "el cristal tiene forma");
        assert!(!c.plano.datos.is_empty());
        assert_eq!(c.grano, 0.0);
        // Borrada la lupa, no hay nada que pintar dentro.
        escena.borrar_apuntando(id);
        assert!(contenido(&escena, id_de_lupa(id)).is_none());
    }
}
