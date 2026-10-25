//! **Las marcas de revision y los sellos** (24-oct-2026), los de corregir
//! planos y PDF: flecha, nubes de revision, rectangulo y triangulo de
//! marca, y los sellos de estado en espanol (APROBADO, RECHAZADO...).
//!
//! Los sellos son un rectangulo redondeado con doble marco y su texto en
//! mayusculas y negrita, del color de lo que dicen: verde lo aprobado, rojo
//! lo rechazado o urgente, azul lo que pide algo y gris lo que ya no vale.
//! Cada uno lleva sus partes con nombre: «Marco» y «Texto» (y «Firma» el de
//! revisado por), para cambiar el texto o el color sin rehacerlo.

use super::electronica::{region, texto_en};
use super::tecnico::{Dibujo, poner};
use super::{Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::ColorRgba;
use crate::nube_revision::{self, Base};
use crate::texto::{self, EstiloDeTexto};

pub(super) const CATEGORIA: &str = "Marcas y sellos";

/// El rojo de corregir.
const ROJO: ColorRgba = hex(0xd1342f);
const BLANCO: ColorRgba = hex(0xffffff);

/// La tinta y el fondo de un sello.
#[derive(Clone, Copy)]
struct Tono {
    tinta: ColorRgba,
    fondo: ColorRgba,
}

const VERDE: Tono = Tono {
    tinta: hex(0x2e8b47),
    fondo: hex(0xeaf6ec),
};
const ROJIZO: Tono = Tono {
    tinta: hex(0xc8352b),
    fondo: hex(0xfdeceb),
};
const AZUL: Tono = Tono {
    tinta: hex(0x2f5f9e),
    fondo: hex(0xe9f0fa),
};
const GRIS: Tono = Tono {
    tinta: hex(0x5f6673),
    fondo: hex(0xf1f2f4),
};

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: &[Dibujo] = &[
        ("marca-flecha", "Flecha de marca", flecha),
        ("marca-nube-1", "Nube de revisión 1", nube_rectangular),
        ("marca-nube-2", "Nube de revisión 2", nube_redonda),
        ("marca-rectangulo", "Rectángulo de marca", rectangulo),
        (
            "marca-triangulo",
            "Triángulo de marca (revisión)",
            triangulo,
        ),
        ("marca-sello-aprobado", "Sello Aprobado", |l| {
            sello(l, "APROBADO", VERDE)
        }),
        (
            "marca-sello-aprobado-con-comentarios",
            "Sello Aprobado con comentarios",
            |l| sello(l, "APROBADO CON COMENTARIOS", VERDE),
        ),
        ("marca-sello-completado", "Sello Completado", |l| {
            sello(l, "COMPLETADO", VERDE)
        }),
        ("marca-sello-rechazado", "Sello Rechazado", |l| {
            sello(l, "RECHAZADO", ROJIZO)
        }),
        ("marca-sello-anulado", "Sello Anulado", |l| {
            sello(l, "ANULADO", GRIS)
        }),
        ("marca-sello-sustituido", "Sello Sustituido", |l| {
            sello(l, "SUSTITUIDO", GRIS)
        }),
        (
            "marca-sello-revisado-por",
            "Sello Revisado por",
            revisado_por,
        ),
        (
            "marca-sello-ver-comentarios",
            "Sello Ver comentarios",
            |l| sello(l, "VER COMENTARIOS", AZUL),
        ),
        (
            "marca-sello-para-comentarios",
            "Sello Para comentarios",
            |l| sello(l, "PARA COMENTARIOS", AZUL),
        ),
        ("marca-sello-urgente", "Sello Urgente", |l| {
            sello(l, "URGENTE", ROJIZO)
        }),
        ("marca-sello-borrador", "Sello Borrador", |l| {
            sello(l, "BORRADOR", GRIS)
        }),
        (
            "marca-sello-para-informacion",
            "Sello Para información",
            |l| sello(l, "PARA INFORMACIÓN", AZUL),
        ),
        ("marca-sello-no-publicar", "Sello No publicar", |l| {
            sello(l, "NO PUBLICAR", GRIS)
        }),
        (
            "marca-sello-para-publicacion",
            "Sello Para publicación",
            |l| sello(l, "PARA PUBLICACIÓN", AZUL),
        ),
    ];
    poner(v, CATEGORIA, lista);
}

// ---------------------------------------------------------------- marcas

fn flecha(l: &mut Lapiz) {
    let (a, b) = ((0.0, 60.0), (140.0, 0.0));
    // Parte «Asta»: la linea de la flecha.
    l.parte("Asta");
    l.linea(a.0, a.1, b.0 - 4.0, b.1 + 1.7, ROJO, 4.0);
    // Parte «Punta»: la cabeza abierta, en el extremo que senala.
    l.parte("Punta");
    let ang: f32 = (b.1 - a.1).atan2(b.0 - a.0);
    let ala = |giro: f32| {
        let t = ang + std::f32::consts::PI + giro;
        (b.0 + 22.0 * t.cos(), b.1 + 22.0 * t.sin())
    };
    l.poli(&[ala(0.45), b, ala(-0.45)], ROJO, 4.0, None);
}

/// El radio del feston de las nubes de la galeria: un diametro de 26, el
/// de las nubes que habia antes, para que se vean igual al estamparlas.
const RADIO_NUBE: f32 = 13.0;

/// **Una nube de revision dinamica** (`nube_revision`): su caja y el radio
/// del feston. Al estirarla no se estiran los festones: caben mas.
fn nube(l: &mut Lapiz, base: Base, ancho: f32, alto: f32) {
    // Parte «Nube»: el contorno en festones que rodea lo revisado.
    l.parte("Nube");
    let mut e = nube_revision::nube(0.0, 0.0, ancho, alto, RADIO_NUBE, ROJO, 2.5);
    e.figura = base.figura();
    l.v.push(e);
}

fn nube_rectangular(l: &mut Lapiz) {
    nube(l, Base::Rectangulo, 220.0, 130.0);
}

fn nube_redonda(l: &mut Lapiz) {
    nube(l, Base::Elipse, 240.0, 150.0);
}

fn rectangulo(l: &mut Lapiz) {
    // Parte «Marco»: el rectangulo rojo sin relleno, para encerrar algo.
    l.parte("Marco");
    l.rect(0.0, 0.0, 200.0, 120.0, Some(ROJO), None, 3.0, false);
}

/// El delta de revision con su numero.
fn triangulo(l: &mut Lapiz) {
    // Parte «Triángulo»: el delta.
    l.parte("Triángulo");
    region(
        l,
        &[(40.0, 0.0), (80.0, 70.0), (0.0, 70.0)],
        ROJO,
        BLANCO,
        2.5,
    );
    // Parte «Número»: la revision a la que se refiere.
    l.parte("Número");
    texto_en(l, 40.0, 46.0, "1", 24.0, ROJO, true);
}

// ----------------------------------------------------------------- sellos

const TAM: f32 = 22.0;

/// Lo ancho que sale `s` en el texto de los sellos.
fn ancho_de(s: &str) -> f32 {
    let estilo = EstiloDeTexto {
        negrita: true,
        ..Default::default()
    };
    texto::medida(s, TAM, WORK_SANS, estilo).0
}

/// El doble marco de un sello de `w × h`.
fn marco(l: &mut Lapiz, w: f32, h: f32, t: Tono) {
    // Parte «Marco»: el rectangulo de fuera (con el fondo) y el fino de
    // dentro.
    l.parte("Marco");
    l.rect(0.0, 0.0, w, h, Some(t.tinta), Some(t.fondo), 3.0, true);
    l.rect(6.0, 6.0, w - 12.0, h - 12.0, Some(t.tinta), None, 1.2, true);
}

/// **Un sello** con su texto centrado; el ancho sale del texto.
fn sello(l: &mut Lapiz, s: &str, t: Tono) {
    let (w, h) = (ancho_de(s) + 68.0, 64.0);
    marco(l, w, h, t);
    // Parte «Texto»: lo que dice el sello; se cambia sin tocar el marco.
    l.parte("Texto");
    texto_en(l, w / 2.0, h / 2.0, s, TAM, t.tinta, true);
}

/// El de revisado por, con sitio para el nombre y la fecha.
fn revisado_por(l: &mut Lapiz) {
    let t = AZUL;
    let (w, h) = (280.0, 116.0);
    marco(l, w, h, t);
    // Parte «Texto»: el titulo del sello.
    l.parte("Texto");
    texto_en(l, w / 2.0, 32.0, "REVISADO POR", TAM, t.tinta, true);
    // Parte «Firma»: los renglones para el nombre y la fecha.
    l.parte("Firma");
    for (i, rotulo) in ["Nombre", "Fecha"].into_iter().enumerate() {
        let y = 66.0 + 26.0 * i as f32;
        l.texto(
            22.0,
            y - 9.0,
            rotulo,
            12.0,
            WORK_SANS,
            t.tinta,
            false,
            super::Al::Izq,
        );
        l.linea(78.0, y + 4.0, w - 22.0, y + 4.0, t.tinta, 1.0);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::biblioteca::nombre_de_parte;
    use crate::elemento::Figura;

    #[test]
    fn la_galeria_estampa_nubes_dinamicas_y_no_lineas_fijas() {
        let mut v = Vec::new();
        todas(&mut v);
        for (clave, base) in [
            ("marca-nube-1", Base::Rectangulo),
            ("marca-nube-2", Base::Elipse),
        ] {
            let f = v.iter().find(|f| f.id == format!("serie-{clave}")).unwrap();
            assert_eq!(f.elementos.len(), 1, "{clave}");
            let e = &f.elementos[0];
            assert_eq!(nube_revision::base_de(e), Some(base), "{clave}");
            assert_eq!(e.extras.nube, Some(RADIO_NUBE));
            // Caso negativo: no es la linea de festones fijos de antes.
            assert!(!matches!(e.figura, Figura::Linea { .. }));
            assert!(e.grupos.iter().any(|g| nombre_de_parte(g) == Some("Nube")));
        }
    }

    #[test]
    fn cada_sello_dice_lo_suyo_en_su_color_y_con_marco_y_texto() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(v.len(), 19);
        for (clave, dice, tinta) in [
            ("aprobado", "APROBADO", VERDE.tinta),
            ("rechazado", "RECHAZADO", ROJIZO.tinta),
            ("urgente", "URGENTE", ROJIZO.tinta),
            ("para-informacion", "PARA INFORMACIÓN", AZUL.tinta),
            ("anulado", "ANULADO", GRIS.tinta),
        ] {
            let f = v
                .iter()
                .find(|f| f.id == format!("serie-marca-sello-{clave}"))
                .unwrap();
            let texto = f
                .elementos
                .iter()
                .find(|e| matches!(&e.figura, Figura::Texto { texto, .. } if texto == dice))
                .unwrap_or_else(|| panic!("{clave} no dice {dice}"));
            assert_eq!(texto.trazo, tinta, "{clave}");
            assert!(
                texto
                    .grupos
                    .iter()
                    .any(|g| nombre_de_parte(g) == Some("Texto"))
            );
            // El texto cabe dentro del marco.
            let marco = &f.elementos[0];
            assert!(texto.x > marco.x && texto.x + texto.ancho < marco.x + marco.ancho);
            assert!(
                marco
                    .grupos
                    .iter()
                    .any(|g| nombre_de_parte(g) == Some("Marco"))
            );
        }
        for f in &v {
            for e in &f.elementos {
                assert_eq!(
                    e.grupos.iter().filter_map(|g| nombre_de_parte(g)).count(),
                    1,
                    "{}",
                    f.nombre
                );
            }
        }
    }
}
