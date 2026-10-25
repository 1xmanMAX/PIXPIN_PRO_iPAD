//! **Las figuras de UML** (24-oct-2026): las de los diagramas que mas se
//! usan (clases, casos de uso, secuencia, actividad y estados, y algo de
//! componentes y despliegue), con las relaciones como conectores de verdad.
//!
//! Las clases van por compartimentos («Nombre», «Atributos», «Métodos»),
//! cada uno su parte con su texto de ejemplo, para cambiar uno sin tocar los
//! otros. Las relaciones son flechas del lienzo con su punta UML (triangulo
//! hueco, rombo...), asi que se estiran y se enganchan como cualquier otra.

use super::Lapiz;
use super::diagramas::{
    BLANCO, G, RELLENO, RELLENO_FUERTE, TAM, TRAZO, caja, caja_redonda, conector, forma, punto,
    raya, raya_con, rotulo, rotulo_izq, rotulo_subrayado,
};
use super::tecnico::{Dibujo, poner};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::EstiloTrazo;
use crate::formas::TipoPunta;

pub(super) const CATEGORIA: &str = "UML";

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: &[Dibujo] = &[
        ("uml-clase", "Clase", clase),
        ("uml-interfaz", "Interfaz", interfaz),
        ("uml-enumeracion", "Enumeración", enumeracion),
        ("uml-objeto", "Objeto (instancia)", objeto),
        ("uml-actor", "Actor", actor),
        ("uml-caso-de-uso", "Caso de uso", caso_de_uso),
        ("uml-sistema", "Límite del sistema", sistema),
        (
            "uml-linea-de-vida",
            "Línea de vida con activación",
            linea_de_vida,
        ),
        ("uml-nota", "Nota", nota),
        ("uml-paquete", "Paquete", paquete),
        ("uml-componente", "Componente", componente),
        ("uml-nodo", "Nodo (despliegue)", nodo),
        ("uml-accion", "Acción (actividad)", accion),
        ("uml-estado", "Estado", estado),
        ("uml-estado-inicial", "Estado inicial", estado_inicial),
        ("uml-estado-final", "Estado final", estado_final),
        ("uml-decision", "Decisión / fusión", decision),
        ("uml-bifurcacion", "Bifurcación / unión (fork)", bifurcacion),
        ("uml-herencia", "Relación de herencia", |l| {
            relacion(
                l,
                "hereda de",
                TipoPunta::Ninguna,
                TipoPunta::TrianguloHueco,
                false,
            )
        }),
        ("uml-realizacion", "Relación de realización", |l| {
            relacion(
                l,
                "implementa",
                TipoPunta::Ninguna,
                TipoPunta::TrianguloHueco,
                true,
            )
        }),
        ("uml-asociacion", "Relación de asociación", |l| {
            relacion(l, "1      *", TipoPunta::Ninguna, TipoPunta::Flecha, false)
        }),
        ("uml-dependencia", "Relación de dependencia", |l| {
            relacion(l, "«usa»", TipoPunta::Ninguna, TipoPunta::Flecha, true)
        }),
        ("uml-agregacion", "Relación de agregación", |l| {
            relacion(l, "tiene", TipoPunta::RomboHueco, TipoPunta::Ninguna, false)
        }),
        ("uml-composicion", "Relación de composición", |l| {
            relacion(
                l,
                "se compone de",
                TipoPunta::Rombo,
                TipoPunta::Ninguna,
                false,
            )
        }),
        ("uml-mensaje", "Mensaje síncrono", |l| {
            relacion(
                l,
                "1: pedir()",
                TipoPunta::Ninguna,
                TipoPunta::Triangulo,
                false,
            )
        }),
        ("uml-respuesta", "Mensaje de respuesta", |l| {
            relacion(
                l,
                "2: respuesta",
                TipoPunta::Ninguna,
                TipoPunta::Flecha,
                true,
            )
        }),
    ];
    poner(v, CATEGORIA, lista);
}

/// Un compartimento de clase: su caja y su texto alineado a la izquierda.
fn compartimento(l: &mut Lapiz, y: f32, h: f32, s: &str) {
    caja(l, 0.0, y, 200.0, h);
    rotulo_izq(l, 10.0, y + 10.0, s, 14.0, false);
}

/// La cabecera de una clase: la caja mas oscura y el nombre centrado (con
/// su estereotipo encima si lo lleva).
fn cabecera(l: &mut Lapiz, h: f32, estereotipo: Option<&str>, nombre: &str) {
    l.rect(
        0.0,
        0.0,
        200.0,
        h,
        Some(TRAZO),
        Some(RELLENO_FUERTE),
        G,
        false,
    );
    match estereotipo {
        Some(e) => {
            rotulo(l, 100.0, 16.0, e, 13.0, false);
            rotulo(l, 100.0, 36.0, nombre, TAM, true);
        }
        None => rotulo(l, 100.0, h / 2.0, nombre, TAM, true),
    }
}

fn clase(l: &mut Lapiz) {
    // Parte «Nombre»: la cabecera con el nombre de la clase.
    l.parte("Nombre");
    cabecera(l, 40.0, None, "Clase");
    // Parte «Atributos»: el compartimento de los atributos, uno por renglon.
    l.parte("Atributos");
    compartimento(l, 40.0, 70.0, "- atributo: Tipo\n- otro: Tipo");
    // Parte «Métodos»: el de las operaciones.
    l.parte("Métodos");
    compartimento(l, 110.0, 70.0, "+ método(): Tipo\n+ otro(x: Tipo)");
}

fn interfaz(l: &mut Lapiz) {
    // Parte «Nombre»: la cabecera con «interfaz» y el nombre.
    l.parte("Nombre");
    cabecera(l, 50.0, Some("«interfaz»"), "Interfaz");
    // Parte «Métodos»: las operaciones que obliga a tener.
    l.parte("Métodos");
    compartimento(l, 50.0, 70.0, "+ operación(): Tipo\n+ otra(x: Tipo)");
}

fn enumeracion(l: &mut Lapiz) {
    // Parte «Nombre»: la cabecera con «enumeración» y el nombre.
    l.parte("Nombre");
    cabecera(l, 50.0, Some("«enumeración»"), "Color");
    // Parte «Valores»: los valores posibles, uno por renglon.
    l.parte("Valores");
    compartimento(l, 50.0, 80.0, "ROJO\nVERDE\nAZUL");
}

fn objeto(l: &mut Lapiz) {
    // Parte «Nombre»: «objeto: Clase», subrayado como manda UML.
    l.parte("Nombre");
    l.rect(
        0.0,
        0.0,
        200.0,
        40.0,
        Some(TRAZO),
        Some(RELLENO_FUERTE),
        G,
        false,
    );
    rotulo_subrayado(l, 100.0, 20.0, "objeto: Clase", TAM, true);
    // Parte «Valores»: los atributos con su valor.
    l.parte("Valores");
    compartimento(l, 40.0, 60.0, "atributo = valor\notro = 42");
}

/// El monigote: cabeza, cuerpo, brazos y piernas, de 60 de ancho.
fn monigote(l: &mut Lapiz) {
    l.circulo(30.0, 14.0, 14.0, Some(TRAZO), Some(RELLENO), G);
    raya(l, (30.0, 28.0), (30.0, 70.0));
    raya(l, (4.0, 44.0), (56.0, 44.0));
    raya(l, (30.0, 70.0), (6.0, 100.0));
    raya(l, (30.0, 70.0), (54.0, 100.0));
}

fn actor(l: &mut Lapiz) {
    // Parte «Figura»: el monigote del actor.
    l.parte("Figura");
    monigote(l);
    // Parte «Nombre»: quien es, debajo.
    l.parte("Nombre");
    rotulo(l, 30.0, 116.0, "Actor", TAM, false);
}

fn caso_de_uso(l: &mut Lapiz) {
    // Parte «Forma»: la elipse del caso de uso.
    l.parte("Forma");
    l.elipse(0.0, 0.0, 180.0, 80.0, Some(TRAZO), Some(RELLENO), G);
    // Parte «Texto»: lo que hace el actor, con un verbo.
    l.parte("Texto");
    rotulo(l, 90.0, 40.0, "Caso de uso", TAM, false);
}

fn sistema(l: &mut Lapiz) {
    // Parte «Límite»: el rectangulo que encierra los casos de uso.
    l.parte("Límite");
    l.rect(0.0, 0.0, 300.0, 380.0, Some(TRAZO), Some(BLANCO), G, false);
    // Parte «Nombre»: el nombre del sistema, arriba.
    l.parte("Nombre");
    rotulo(l, 150.0, 24.0, "Sistema", TAM, true);
}

fn linea_de_vida(l: &mut Lapiz) {
    // Parte «Cabecera»: la caja con el participante.
    l.parte("Cabecera");
    caja(l, 0.0, 0.0, 140.0, 50.0);
    rotulo(l, 70.0, 25.0, ":Objeto", TAM, false);
    // Parte «Línea»: la linea de vida a trazos.
    l.parte("Línea");
    raya_con(l, (70.0, 50.0), (70.0, 320.0), EstiloTrazo::Discontinuo);
    // Parte «Activación»: el rectangulo estrecho mientras trabaja.
    l.parte("Activación");
    l.rect(
        60.0,
        100.0,
        20.0,
        140.0,
        Some(TRAZO),
        Some(BLANCO),
        G,
        false,
    );
}

/// La hoja con la esquina doblada (notas de UML, objetos de datos).
pub(super) fn hoja_doblada(l: &mut Lapiz, x: f32, y: f32, w: f32, h: f32, pico: f32) {
    forma(
        l,
        &[
            (x, y),
            (x + w - pico, y),
            (x + w, y + pico),
            (x + w, y + h),
            (x, y + h),
        ],
    );
    l.poli(
        &[
            (x + w - pico, y),
            (x + w - pico, y + pico),
            (x + w, y + pico),
        ],
        TRAZO,
        G,
        None,
    );
}

fn nota(l: &mut Lapiz) {
    // Parte «Forma»: la hoja con la esquina doblada.
    l.parte("Forma");
    hoja_doblada(l, 0.0, 0.0, 180.0, 100.0, 20.0);
    // Parte «Texto»: el comentario.
    l.parte("Texto");
    rotulo_izq(l, 12.0, 14.0, "Nota o\nrestricción", 14.0, false);
}

fn paquete(l: &mut Lapiz) {
    // Parte «Pestaña»: la lengüeta de arriba, con el nombre.
    l.parte("Pestaña");
    l.rect(
        0.0,
        0.0,
        100.0,
        30.0,
        Some(TRAZO),
        Some(RELLENO_FUERTE),
        G,
        false,
    );
    rotulo(l, 50.0, 15.0, "paquete", 14.0, true);
    // Parte «Cuerpo»: donde van las clases del paquete.
    l.parte("Cuerpo");
    caja(l, 0.0, 30.0, 240.0, 150.0);
}

fn componente(l: &mut Lapiz) {
    // Parte «Cuerpo»: la caja del componente.
    l.parte("Cuerpo");
    caja(l, 0.0, 0.0, 200.0, 100.0);
    // Parte «Icono»: el simbolo de componente, arriba a la derecha.
    l.parte("Icono");
    l.rect(
        166.0,
        10.0,
        24.0,
        28.0,
        Some(TRAZO),
        Some(BLANCO),
        1.5,
        false,
    );
    l.rect(
        160.0,
        15.0,
        12.0,
        6.0,
        Some(TRAZO),
        Some(BLANCO),
        1.5,
        false,
    );
    l.rect(
        160.0,
        27.0,
        12.0,
        6.0,
        Some(TRAZO),
        Some(BLANCO),
        1.5,
        false,
    );
    // Parte «Nombre»: el estereotipo y el nombre.
    l.parte("Nombre");
    rotulo(l, 90.0, 40.0, "«componente»", 13.0, false);
    rotulo(l, 90.0, 62.0, "Componente", TAM, true);
}

fn nodo(l: &mut Lapiz) {
    // Parte «Caras»: la tapa y el costado que le dan volumen.
    l.parte("Caras");
    forma(l, &[(0.0, 20.0), (20.0, 0.0), (220.0, 0.0), (200.0, 20.0)]);
    forma(
        l,
        &[(200.0, 20.0), (220.0, 0.0), (220.0, 120.0), (200.0, 140.0)],
    );
    // Parte «Cuerpo»: la cara de delante.
    l.parte("Cuerpo");
    caja(l, 0.0, 20.0, 200.0, 120.0);
    // Parte «Nombre»: el tipo de nodo y su nombre.
    l.parte("Nombre");
    rotulo(l, 100.0, 66.0, "«dispositivo»", 13.0, false);
    rotulo(l, 100.0, 88.0, "Servidor", TAM, true);
}

fn accion(l: &mut Lapiz) {
    // Parte «Forma»: el rectangulo redondeado de la accion.
    l.parte("Forma");
    caja_redonda(l, 0.0, 0.0, 160.0, 60.0);
    // Parte «Texto»: lo que se hace.
    l.parte("Texto");
    rotulo(l, 80.0, 30.0, "Acción", TAM, false);
}

fn estado(l: &mut Lapiz) {
    // Parte «Forma»: el rectangulo redondeado y su raya.
    l.parte("Forma");
    caja_redonda(l, 0.0, 0.0, 160.0, 90.0);
    raya(l, (0.0, 40.0), (160.0, 40.0));
    // Parte «Nombre»: el nombre del estado.
    l.parte("Nombre");
    rotulo(l, 80.0, 20.0, "Estado", TAM, true);
    // Parte «Actividades»: lo que hace al entrar y mientras esta.
    l.parte("Actividades");
    rotulo_izq(
        l,
        12.0,
        48.0,
        "entrada / acción\nhacer / actividad",
        13.0,
        false,
    );
}

fn estado_inicial(l: &mut Lapiz) {
    // Parte «Forma»: el circulo relleno del principio.
    l.parte("Forma");
    punto(l, 15.0, 15.0, 15.0, TRAZO);
}

fn estado_final(l: &mut Lapiz) {
    // Parte «Forma»: la diana del final, aro y circulo.
    l.parte("Forma");
    l.circulo(18.0, 18.0, 17.0, Some(TRAZO), Some(BLANCO), G);
    punto(l, 18.0, 18.0, 11.0, TRAZO);
}

fn decision(l: &mut Lapiz) {
    // Parte «Forma»: el rombo pequeno de decidir o juntar caminos.
    l.parte("Forma");
    forma(l, &[(30.0, 0.0), (60.0, 30.0), (30.0, 60.0), (0.0, 30.0)]);
}

fn bifurcacion(l: &mut Lapiz) {
    // Parte «Entrada»: el camino que llega.
    l.parte("Entrada");
    conector(
        l,
        &[(100.0, 0.0), (100.0, 34.0)],
        TipoPunta::Ninguna,
        TipoPunta::Flecha,
        EstiloTrazo::Solido,
    );
    // Parte «Barra»: la barra gruesa donde se reparte (o se junta).
    l.parte("Barra");
    l.rect(0.0, 36.0, 200.0, 8.0, Some(TRAZO), Some(TRAZO), 1.0, false);
    // Parte «Salidas»: los caminos que siguen en paralelo.
    l.parte("Salidas");
    for x in [40.0, 160.0] {
        conector(
            l,
            &[(x, 44.0), (x, 80.0)],
            TipoPunta::Ninguna,
            TipoPunta::Flecha,
            EstiloTrazo::Solido,
        );
    }
}

/// Una relacion: el conector de 200 con su punta y su etiqueta encima.
fn relacion(l: &mut Lapiz, etiqueta: &str, inicio: TipoPunta, fin: TipoPunta, trazos: bool) {
    // Parte «Etiqueta»: lo que dice la relacion (multiplicidad, verbo...).
    l.parte("Etiqueta");
    rotulo(l, 100.0, 10.0, etiqueta, 14.0, false);
    // Parte «Conector»: la flecha con la punta de UML.
    l.parte("Conector");
    let estilo = if trazos {
        EstiloTrazo::Discontinuo
    } else {
        EstiloTrazo::Solido
    };
    conector(l, &[(0.0, 30.0), (200.0, 30.0)], inicio, fin, estilo);
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::figuras_de_serie::diagramas::pruebas::{comprobar, partes_estampada};

    #[test]
    fn estan_las_de_uml_y_cada_pieza_tiene_su_parte() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(v.len(), 26);
        comprobar(&v, "serie-uml-");
    }

    #[test]
    fn la_clase_estampada_conserva_sus_compartimentos() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(
            partes_estampada(&v, "serie-uml-clase"),
            ["Nombre", "Atributos", "Métodos"]
                .map(String::from)
                .into_iter()
                .collect()
        );
    }
}
