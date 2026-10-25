//! **Lo que comparten las familias de diagramas** (24-oct-2026): UML,
//! BPMN, entidad-relacion, redes, mapas y organigramas, negocio y formas
//! basicas. Mismo estilo que el diagrama de flujo (trazo azul y relleno
//! claro), textos de ejemplo en espanol y medidas en multiplos de 10 para
//! que las flechas enganchen en los bordes.
//!
//! Aqui no hay figuras: solo los colores y los trazos de siempre (cajas,
//! rotulos, rayas, conectores con su punta) para que todas salgan parejas.

use super::electronica::{region, texto_en};
use super::{Al, Lapiz, WORK_SANS, hex};
use crate::elemento::{ColorRgba, EstiloTrazo, Figura};
use crate::formas::TipoPunta;
use crate::texto::{self, EstiloDeTexto};

/// El azul del trazo, el de `flujo.rs`.
pub(super) const TRAZO: ColorRgba = hex(0x2f5f9e);
/// El relleno suave.
pub(super) const RELLENO: ColorRgba = hex(0xeaf1fb);
/// Un azul algo mas fuerte, para cabeceras y temas centrales.
pub(super) const RELLENO_FUERTE: ColorRgba = hex(0xc9dbf3);
/// La tinta de los textos.
pub(super) const TINTA: ColorRgba = hex(0x1f2d44);
pub(super) const BLANCO: ColorRgba = hex(0xffffff);
/// El grosor normal.
pub(super) const G: f32 = 2.0;
/// El tamano normal del texto.
pub(super) const TAM: f32 = 16.0;

/// El trazo y el relleno de una zona de color (DAFO, iconos, notas...).
#[derive(Clone, Copy)]
pub(super) struct Tono {
    pub trazo: ColorRgba,
    pub relleno: ColorRgba,
}

pub(super) const AZUL: Tono = Tono {
    trazo: TRAZO,
    relleno: RELLENO,
};
pub(super) const VERDE: Tono = Tono {
    trazo: hex(0x2e8b57),
    relleno: hex(0xe5f4ea),
};
pub(super) const AMBAR: Tono = Tono {
    trazo: hex(0xb7791f),
    relleno: hex(0xfdf1dc),
};
pub(super) const ROJO: Tono = Tono {
    trazo: hex(0xc53030),
    relleno: hex(0xfdeaea),
};
pub(super) const MORADO: Tono = Tono {
    trazo: hex(0x6b46c1),
    relleno: hex(0xefe9fb),
};
pub(super) const AMARILLO: Tono = Tono {
    trazo: hex(0xc9a227),
    relleno: hex(0xfff3a8),
};

/// Una caja con el trazo y el relleno de siempre.
pub(super) fn caja(l: &mut Lapiz, x: f32, y: f32, w: f32, h: f32) {
    l.rect(x, y, w, h, Some(TRAZO), Some(RELLENO), G, false);
}

/// Una caja con las esquinas redondeadas.
pub(super) fn caja_redonda(l: &mut Lapiz, x: f32, y: f32, w: f32, h: f32) {
    l.rect(x, y, w, h, Some(TRAZO), Some(RELLENO), G, true);
}

/// Una caja de un tono.
pub(super) fn caja_de(l: &mut Lapiz, x: f32, y: f32, w: f32, h: f32, t: Tono, redonda: bool) {
    l.rect(x, y, w, h, Some(t.trazo), Some(t.relleno), G, redonda);
}

/// Una forma cerrada y rellena con el trazo y el relleno de siempre.
pub(super) fn forma(l: &mut Lapiz, pts: &[(f32, f32)]) {
    region(l, pts, TRAZO, RELLENO, G);
}

/// Una forma cerrada de un tono.
pub(super) fn forma_de(l: &mut Lapiz, pts: &[(f32, f32)], t: Tono) {
    region(l, pts, t.trazo, t.relleno, G);
}

/// Un texto centrado en `(cx, cy)` con la tinta de siempre.
pub(super) fn rotulo(l: &mut Lapiz, cx: f32, cy: f32, s: &str, tam: f32, negrita: bool) {
    texto_en(l, cx, cy, s, tam, TINTA, negrita);
}

/// Un texto con su esquina de arriba a la izquierda en `(x, y)`.
pub(super) fn rotulo_izq(l: &mut Lapiz, x: f32, y: f32, s: &str, tam: f32, negrita: bool) {
    l.texto(x, y, s, tam, WORK_SANS, TINTA, negrita, Al::Izq);
}

/// El ancho de un texto.
pub(super) fn ancho_de(s: &str, tam: f32, negrita: bool) -> f32 {
    let estilo = EstiloDeTexto {
        negrita,
        ..Default::default()
    };
    texto::medida(s, tam, WORK_SANS, estilo).0
}

/// Un texto centrado en `(cx, cy)` y subrayado (claves de ER, objetos UML).
pub(super) fn rotulo_subrayado(l: &mut Lapiz, cx: f32, cy: f32, s: &str, tam: f32, negrita: bool) {
    rotulo(l, cx, cy, s, tam, negrita);
    let w = ancho_de(s, tam, negrita);
    let y = cy + tam * 0.62;
    l.linea(cx - w / 2.0, y, cx + w / 2.0, y, TINTA, 1.5);
}

/// Una raya con el trazo de siempre.
pub(super) fn raya(l: &mut Lapiz, a: (f32, f32), b: (f32, f32)) {
    l.linea(a.0, a.1, b.0, b.1, TRAZO, G);
}

/// Una raya a trazos o a puntos.
pub(super) fn raya_con(l: &mut Lapiz, a: (f32, f32), b: (f32, f32), estilo: EstiloTrazo) {
    l.linea_con(a.0, a.1, b.0, b.1, TRAZO, G, estilo);
}

/// **Un conector de verdad** (una flecha del lienzo) por esos puntos, con
/// sus puntas: se engancha y se estira como cualquier flecha.
pub(super) fn conector(
    l: &mut Lapiz,
    pts: &[(f32, f32)],
    inicio: TipoPunta,
    fin: TipoPunta,
    estilo: EstiloTrazo,
) {
    l.flecha(pts, TRAZO, G, false);
    if let Some(e) = l.v.last_mut() {
        e.estilo = estilo;
        if let Figura::Flecha {
            punta_inicio,
            punta_fin,
            ..
        } = &mut e.figura
        {
            *punta_inicio = inicio;
            *punta_fin = fin;
        }
    }
}

/// Un circulo relleno de un color (puntos, estados iniciales).
pub(super) fn punto(l: &mut Lapiz, cx: f32, cy: f32, r: f32, c: ColorRgba) {
    l.circulo(cx, cy, r, Some(c), Some(c), 1.0);
}

#[cfg(test)]
pub(super) mod pruebas {
    use crate::biblioteca::{self, FiguraGuardada, nombre_de_parte};
    use crate::vector::Punto2;
    use std::collections::HashSet;

    /// Lo de cada familia: ids con su prefijo y unicos, y cada pieza en
    /// exactamente una parte con nombre.
    pub(crate) fn comprobar(v: &[FiguraGuardada], prefijo: &str) {
        let mut ids = HashSet::new();
        for f in v {
            assert!(f.id.starts_with(prefijo), "{}", f.id);
            assert!(ids.insert(f.id.clone()), "id repetido: {}", f.id);
            assert!(!f.elementos.is_empty(), "{}", f.nombre);
            for e in &f.elementos {
                let n = e.grupos.iter().filter_map(|g| nombre_de_parte(g)).count();
                assert_eq!(n, 1, "{}: pieza sin parte (o con dos)", f.nombre);
            }
        }
    }

    /// Las partes de la figura `id` despues de estamparla, comprobando que
    /// cada pieza lleva su parte dentro y el grupo de la figura por fuera.
    pub(crate) fn partes_estampada(v: &[FiguraGuardada], id: &str) -> HashSet<String> {
        let f = v.iter().find(|f| f.id == id).expect(id);
        let e = biblioteca::estampar(f, Punto2::nuevo(300.0, 200.0), 1, "g7");
        for p in &e {
            assert_eq!(p.grupos.len(), 2, "{id}");
            assert!(p.grupos[0].starts_with("parte:") && p.grupos[0].ends_with("@g7"));
            assert_eq!(p.grupos[1], "g7");
        }
        e.iter()
            .flat_map(|p| p.grupos.iter())
            .filter_map(|g| nombre_de_parte(g).map(str::to_string))
            .collect()
    }

    /// Un conjunto de nombres.
    pub(crate) fn nombres(n: &[&str]) -> HashSet<String> {
        n.iter().map(|s| s.to_string()).collect()
    }
}
