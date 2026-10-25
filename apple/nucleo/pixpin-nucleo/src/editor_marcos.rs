//! **Los marcos del lienzo** en el editor (Windows, 22-oct-2026,
//! `ventana_editor/marcas.rs::pintar_marcos`).
//!
//! El motor ya no los pinta (`pintado::ordenes` deja `Figura::Marco` sin
//! nada): son andamiaje para ordenar e imprimir laminas y lo impreso va sin
//! su raya, como los marcos de los visores. En el editor van **a trazos
//! azules de grosor fijo** (1,5 puntos de pantalla a cualquier aumento) con
//! su **numero en el cuadrito azul de la esquina** y el nombre encima, y van
//! con la escena (en el mismo plano que el dibujo), asi el numero se mueve
//! con el lienzo y no llega tarde.

use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::{ColorRgba, EstiloTrazo, Figura};
use pixpin_motor2d::marco;
use pixpin_motor2d::pintado::Orden;
use pixpin_motor2d::vector::Punto2;

/// El azul de los marcos de los visores (`marcos_doc::AZUL`, #0078D4).
pub(crate) const AZUL: ColorRgba = ColorRgba {
    r: 0.0,
    g: 0x78 as f32 / 255.0,
    b: 0xD4 as f32 / 255.0,
    a: 1.0,
};

const BLANCO: ColorRgba = ColorRgba {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

/// La letra de la insignia y del nombre (`marcos_doc::insignia`): 12 puntos.
const TAM: f32 = 12.0;
const FAMILIA: &str = "Segoe UI";

/// Las ordenes de los marcos para una vista a `zoom` (puntos por unidad):
/// todo lo que mide va en puntos de pantalla, pasado al mundo.
pub(crate) fn ordenes(escena: &Escena, zoom: f32) -> Vec<Orden> {
    let k = 1.0 / zoom.max(1e-6);
    let mut v = Vec::new();
    for (i, m) in marco::hojas_en_orden(&escena.elementos).iter().enumerate() {
        let (x0, y0) = (m.x.min(m.x + m.ancho), m.y.min(m.y + m.alto));
        let (x1, y1) = (m.x.max(m.x + m.ancho), m.y.max(m.y + m.alto));
        v.push(Orden::Polilinea {
            puntos: vec![
                Punto2::nuevo(x0, y0),
                Punto2::nuevo(x1, y0),
                Punto2::nuevo(x1, y1),
                Punto2::nuevo(x0, y1),
                Punto2::nuevo(x0, y0),
            ],
            color: AZUL,
            grosor: 1.5 * k,
            estilo: EstiloTrazo::Discontinuo,
        });
        // La insignia: el numero en blanco sobre su cuadrito azul, 20 de alto
        // y 6 de aire a cada lado.
        let n = (i + 1).to_string();
        let (w, _) = pixpin_motor2d::texto::medida_estimada(&n, TAM);
        let (ancho, alto) = ((w + 12.0) * k, 20.0 * k);
        v.push(Orden::Relleno {
            puntos: vec![
                Punto2::nuevo(x0, y0),
                Punto2::nuevo(x0 + ancho, y0),
                Punto2::nuevo(x0 + ancho, y0 + alto),
                Punto2::nuevo(x0, y0 + alto),
            ],
            color: AZUL,
        });
        v.push(texto(n, x0 + 6.0 * k, y0 + 3.0 * k, k, BLANCO));
        // El nombre, encima y fuera de la caja (dentro taparia el dibujo).
        if let Figura::Marco { nombre } = &m.figura
            && !nombre.is_empty()
        {
            let (_, h) = pixpin_motor2d::texto::medida_estimada(nombre, TAM);
            v.push(texto(nombre.clone(), x0, y0 - (h + 4.0) * k, k, AZUL));
        }
    }
    v
}

fn texto(texto: String, x: f32, y: f32, k: f32, color: ColorRgba) -> Orden {
    Orden::Texto {
        texto,
        x,
        y,
        tam: TAM * k,
        familia: FAMILIA.into(),
        color,
        ancho_max: pixpin_motor2d::texto::SIN_PARTIR,
        negrita: false,
        cursiva: false,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::elemento::Elemento;
    use pixpin_motor2d::relleno::EstiloRelleno;

    fn un_marco(escena: &mut Escena, x: f32, y: f32, nombre: &str) {
        escena.anadir(Elemento {
            id: 0,
            figura: Figura::Marco {
                nombre: nombre.into(),
            },
            x,
            y,
            ancho: 200.0,
            alto: 100.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            estilo_relleno: EstiloRelleno::Solido,
            grosor: 1.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        });
    }

    /// 22-oct-2026, «que a pesar de agrandar mantenga su grosor constante»:
    /// la raya y la insignia miden lo mismo en la pantalla a cualquier
    /// aumento, y cada marco lleva su numero en el orden de lectura.
    #[test]
    fn raya_azul_a_trazos_de_grosor_fijo_con_su_numero() {
        let mut escena = Escena::nueva();
        un_marco(&mut escena, 0.0, 300.0, "");
        un_marco(&mut escena, 0.0, 0.0, "Portada");
        let a1 = ordenes(&escena, 1.0);
        let a4 = ordenes(&escena, 4.0);
        let grosores = |o: &[Orden]| -> Vec<f32> {
            o.iter()
                .filter_map(|o| match o {
                    Orden::Polilinea {
                        grosor,
                        estilo: EstiloTrazo::Discontinuo,
                        color,
                        ..
                    } if *color == AZUL => Some(*grosor),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(grosores(&a1), vec![1.5, 1.5]);
        assert_eq!(grosores(&a4), vec![0.375, 0.375], "al acercar, la raya no engorda");
        let textos: Vec<&str> = a1
            .iter()
            .filter_map(|o| match o {
                Orden::Texto { texto, .. } => Some(texto.as_str()),
                _ => None,
            })
            .collect();
        // El de arriba es el 1 (con su nombre); el de abajo, el 2.
        assert_eq!(textos, vec!["1", "Portada", "2"]);
        // Caso negativo: sin marcos no se pinta nada, y lo borrado no cuenta.
        let mut vacia = Escena::nueva();
        assert!(ordenes(&vacia, 1.0).is_empty());
        un_marco(&mut vacia, 0.0, 0.0, "");
        vacia.elementos[0].borrado = true;
        assert!(ordenes(&vacia, 1.0).is_empty());
    }

    /// El motor ya no los pinta: lo impreso y exportado va sin su raya.
    #[test]
    fn el_motor_no_pinta_el_marco() {
        let mut escena = Escena::nueva();
        un_marco(&mut escena, 0.0, 0.0, "Hoja");
        assert!(pixpin_motor2d::pintado::ordenes(&escena.elementos[0]).is_empty());
    }
}
