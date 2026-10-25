//! El **interruptor Chat · Figuras** de la barra de titulo del chat: las
//! cuentas, sin pintar nada.
//!
//! Hasta el 25-oct-2026 tenia tres partes (Chat · Proyectos · Figuras). La
//! pantalla de Proyectos se quito (el usuario: «los proyectos no sirven»);
//! Figuras se queda y abre su ventana aparte.

use pixpin_geom::Rect;

/// El interruptor de la barra de titulo, con sus dos partes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interruptor {
    pub caja: Rect,
    pub chat: Rect,
    /// La galeria de las figuras guardadas (24-oct-2026).
    pub figuras: Rect,
}

/// Lo que mide cada parte del interruptor.
pub const INTERRUPTOR_MITAD: u32 = 76;
/// Lo que se le quita al alto de la barra, repartido arriba y abajo.
pub const INTERRUPTOR_AIRE_Y: u32 = 3;
/// La letra de sus dos palabras.
pub const INTERRUPTOR_TAM: f32 = 11.0;

fn e(v: u32, escala: u32) -> u32 {
    v * escala / 100
}

/// El interruptor, centrado en la barra de titulo y sin pisar sus botones:
/// en una ventana estrecha se corre a la izquierda de ellos. `hasta` es donde
/// empiezan los botones de minimizar, maximizar y cerrar; `desde`, donde acaba
/// el nombre del programa.
pub fn interruptor(barra: Rect, desde: i32, hasta: i32, escala: u32) -> Interruptor {
    let aire = e(INTERRUPTOR_AIRE_Y, escala);
    // En una ventana estrecha las dos partes se encogen para caber entre el
    // nombre del programa y los botones de la ventana.
    let sitio = (hasta - desde - aire as i32).max(0) as u32 / 2;
    let mitad = e(INTERRUPTOR_MITAD, escala).min(sitio.max(1));
    let ancho = 2 * mitad;
    let alto = barra.alto.saturating_sub(2 * aire).max(1);
    let centrado = barra.x + (barra.ancho as i32 - ancho as i32) / 2;
    let x = centrado.min(hasta - ancho as i32 - aire as i32).max(desde);
    let caja = Rect {
        x,
        y: barra.y + aire as i32,
        ancho,
        alto,
    };
    Interruptor {
        caja,
        chat: Rect {
            ancho: mitad,
            ..caja
        },
        figuras: Rect {
            x: caja.x + mitad as i32,
            ancho: mitad,
            ..caja
        },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn r(x: i32, y: i32, ancho: u32, alto: u32) -> Rect {
        Rect { x, y, ancho, alto }
    }

    #[test]
    fn el_interruptor_va_centrado_y_no_pisa_los_botones_de_la_ventana() {
        let barra = r(0, 0, 1024, 24);
        let i = interruptor(barra, 80, 1024 - 108, 100);
        assert_eq!(i.caja.x, (1024 - 2 * 76) / 2);
        assert_eq!(i.chat.derecha(), i.figuras.x);
        assert_eq!(i.figuras.derecha(), i.caja.derecha());
        assert!(i.caja.alto < 24);
        // Estrecha: se corre a la izquierda de los botones, pero nunca encima
        // del nombre del programa.
        let estrecha = interruptor(r(0, 0, 380, 24), 80, 380 - 108, 100);
        assert!(estrecha.caja.derecha() <= 380 - 108);
        assert!(estrecha.caja.x >= 80);
    }
}
