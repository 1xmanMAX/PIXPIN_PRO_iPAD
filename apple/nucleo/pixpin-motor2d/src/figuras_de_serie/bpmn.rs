//! **Las figuras de BPMN 2.0** (24-oct-2026): el puñado de elementos que
//! cubre casi todos los diagramas de procesos de verdad (eventos de inicio,
//! intermedio y fin, tareas, subproceso, compuertas, pool y carriles,
//! objeto de datos, anotacion y grupo) y los tres flujos como conectores.
//!
//! Los eventos llevan su nombre debajo y las compuertas su pregunta encima,
//! cada cosa su parte («Evento», «Nombre», «Compuerta», «Marca»,
//! «Pregunta»...), para cambiar el texto sin tocar el simbolo.

use super::Lapiz;
use super::diagramas::{
    BLANCO, G, RELLENO, RELLENO_FUERTE, TAM, TRAZO, caja_redonda, conector, forma, rotulo,
    rotulo_izq,
};
use super::electronica::arco_pts;
use super::tecnico::{Dibujo, poner};
use super::uml::hoja_doblada;
use crate::biblioteca::FiguraGuardada;
use crate::elemento::EstiloTrazo;
use crate::formas::TipoPunta;

pub(super) const CATEGORIA: &str = "BPMN";

/// El radio de los eventos (50 de diametro).
const R: f32 = 25.0;

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: &[Dibujo] = &[
        ("bpmn-inicio", "Evento de inicio", |l| {
            evento(l, Evento::Inicio, Icono::Ninguno, "Inicio")
        }),
        ("bpmn-inicio-mensaje", "Evento de inicio de mensaje", |l| {
            evento(l, Evento::Inicio, Icono::Sobre, "Pedido recibido")
        }),
        ("bpmn-intermedio", "Evento intermedio", |l| {
            evento(l, Evento::Intermedio, Icono::Ninguno, "Hito")
        }),
        ("bpmn-temporizador", "Evento de temporizador", |l| {
            evento(l, Evento::Intermedio, Icono::Reloj, "Esperar 2 días")
        }),
        ("bpmn-fin", "Evento de fin", |l| {
            evento(l, Evento::Fin, Icono::Ninguno, "Fin")
        }),
        ("bpmn-fin-mensaje", "Evento de fin de mensaje", |l| {
            evento(l, Evento::Fin, Icono::SobreLleno, "Aviso enviado")
        }),
        ("bpmn-tarea", "Tarea", |l| tarea(l, Tarea::Simple, "Tarea")),
        ("bpmn-tarea-usuario", "Tarea de usuario", |l| {
            tarea(l, Tarea::Usuario, "Revisar\nsolicitud")
        }),
        ("bpmn-tarea-servicio", "Tarea de servicio", |l| {
            tarea(l, Tarea::Servicio, "Enviar\ncorreo")
        }),
        ("bpmn-subproceso", "Subproceso", |l| {
            tarea(l, Tarea::Subproceso, "Subproceso")
        }),
        ("bpmn-exclusiva", "Compuerta exclusiva (XOR)", |l| {
            compuerta(l, Marca::Aspa, "¿Aprobado?")
        }),
        ("bpmn-paralela", "Compuerta paralela (AND)", |l| {
            compuerta(l, Marca::Mas, "")
        }),
        ("bpmn-inclusiva", "Compuerta inclusiva (OR)", |l| {
            compuerta(l, Marca::Circulo, "¿Qué hace falta?")
        }),
        (
            "bpmn-basada-en-eventos",
            "Compuerta basada en eventos",
            |l| compuerta(l, Marca::Pentagono, "¿Qué llega antes?"),
        ),
        ("bpmn-pool", "Pool con carriles", pool),
        ("bpmn-objeto-de-datos", "Objeto de datos", objeto_de_datos),
        ("bpmn-anotacion", "Anotación de texto", anotacion),
        ("bpmn-grupo", "Grupo", grupo),
        ("bpmn-flujo-secuencia", "Flujo de secuencia", |l| {
            flujo(
                l,
                "sí",
                TipoPunta::Ninguna,
                TipoPunta::Triangulo,
                EstiloTrazo::Solido,
            )
        }),
        ("bpmn-flujo-mensaje", "Flujo de mensaje", |l| {
            flujo(
                l,
                "pedido",
                TipoPunta::CirculoHueco,
                TipoPunta::TrianguloHueco,
                EstiloTrazo::Discontinuo,
            )
        }),
        ("bpmn-asociacion", "Asociación", |l| {
            flujo(
                l,
                "asociación",
                TipoPunta::Ninguna,
                TipoPunta::Ninguna,
                EstiloTrazo::Punteado,
            )
        }),
    ];
    poner(v, CATEGORIA, lista);
}

#[derive(Clone, Copy, PartialEq)]
enum Evento {
    Inicio,
    Intermedio,
    Fin,
}

#[derive(Clone, Copy, PartialEq)]
enum Icono {
    Ninguno,
    Sobre,
    SobreLleno,
    Reloj,
}

/// El sobre de los mensajes, centrado en `(cx, cy)`; relleno si se lanza.
fn sobre(l: &mut Lapiz, cx: f32, cy: f32, lleno: bool) {
    let (x0, y0, x1) = (cx - 13.0, cy - 9.0, cx + 13.0);
    let (trazo, fondo) = if lleno {
        (BLANCO, TRAZO)
    } else {
        (TRAZO, BLANCO)
    };
    l.rect(x0, y0, 26.0, 18.0, Some(TRAZO), Some(fondo), 1.5, false);
    l.poli(&[(x0, y0), (cx, cy + 1.0), (x1, y0)], trazo, 1.5, None);
}

/// La esfera del reloj del temporizador.
fn reloj(l: &mut Lapiz, cx: f32, cy: f32) {
    l.circulo(cx, cy, 15.0, Some(TRAZO), Some(BLANCO), 1.5);
    for i in 0..12 {
        let a = (i as f32 * 30.0).to_radians();
        let (s, c) = a.sin_cos();
        l.linea(
            cx + c * 11.0,
            cy + s * 11.0,
            cx + c * 14.0,
            cy + s * 14.0,
            TRAZO,
            1.0,
        );
    }
    l.poli(
        &[(cx + 6.0, cy - 8.0), (cx, cy), (cx + 8.0, cy + 3.0)],
        TRAZO,
        1.5,
        None,
    );
}

fn evento(l: &mut Lapiz, tipo: Evento, icono: Icono, nombre: &str) {
    let (cx, cy) = (50.0, R);
    // Parte «Evento»: el circulo (fino al inicio, doble en medio, grueso
    // al final).
    l.parte("Evento");
    let g = if tipo == Evento::Fin { 5.0 } else { G };
    l.circulo(cx, cy, R, Some(TRAZO), Some(RELLENO), g);
    if tipo == Evento::Intermedio {
        l.circulo(cx, cy, R - 5.0, Some(TRAZO), None, G);
    }
    // Parte «Icono»: lo que dispara el evento (mensaje, tiempo...).
    match icono {
        Icono::Ninguno => {}
        Icono::Sobre | Icono::SobreLleno => {
            l.parte("Icono");
            sobre(l, cx, cy, icono == Icono::SobreLleno);
        }
        Icono::Reloj => {
            l.parte("Icono");
            reloj(l, cx, cy);
        }
    }
    // Parte «Nombre»: lo que pasa, debajo del circulo.
    l.parte("Nombre");
    rotulo(l, cx, 2.0 * R + 16.0, nombre, 14.0, false);
}

#[derive(Clone, Copy, PartialEq)]
enum Tarea {
    Simple,
    Usuario,
    Servicio,
    Subproceso,
}

/// El monigote de busto de las tareas de usuario, arriba a la izquierda.
fn busto(l: &mut Lapiz, x: f32, y: f32) {
    l.circulo(x + 9.0, y + 6.0, 5.0, Some(TRAZO), Some(BLANCO), 1.5);
    let mut p = arco_pts(x + 9.0, y + 22.0, 9.0, 9.0, 180.0, 360.0);
    p.push((x + 18.0, y + 22.0));
    p.push((x, y + 22.0));
    l.poli(&p, TRAZO, 1.5, Some(BLANCO));
}

/// El engranaje de las tareas de servicio, centrado en `(cx, cy)`.
fn engranaje(l: &mut Lapiz, cx: f32, cy: f32) {
    let n = 8;
    let mut p = Vec::new();
    for i in 0..n {
        let a0 = i as f32 * 360.0 / n as f32;
        for (da, r) in [(-14.0, 7.0), (-8.0, 10.0), (8.0, 10.0), (14.0, 7.0)] {
            let a = (a0 + da).to_radians();
            p.push((cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    p.push(p[0]);
    l.poli(&p, TRAZO, 1.5, Some(BLANCO));
    l.circulo(cx, cy, 3.0, Some(TRAZO), Some(RELLENO), 1.5);
}

fn tarea(l: &mut Lapiz, tipo: Tarea, texto: &str) {
    // Parte «Forma»: el rectangulo redondeado de la actividad.
    l.parte("Forma");
    caja_redonda(l, 0.0, 0.0, 160.0, 80.0);
    match tipo {
        Tarea::Simple => {}
        Tarea::Usuario => {
            // Parte «Icono»: la persona que la hace.
            l.parte("Icono");
            busto(l, 8.0, 6.0);
        }
        Tarea::Servicio => {
            // Parte «Icono»: el engranaje de lo que hace un sistema.
            l.parte("Icono");
            engranaje(l, 18.0, 18.0);
        }
        Tarea::Subproceso => {
            // Parte «Marcador»: el [+] de abajo, que tiene pasos dentro.
            l.parte("Marcador");
            l.rect(
                72.0,
                62.0,
                16.0,
                16.0,
                Some(TRAZO),
                Some(BLANCO),
                1.5,
                false,
            );
            l.linea(75.0, 70.0, 85.0, 70.0, TRAZO, 1.5);
            l.linea(80.0, 65.0, 80.0, 75.0, TRAZO, 1.5);
        }
    }
    // Parte «Texto»: lo que se hace, con un verbo.
    l.parte("Texto");
    let cy = if tipo == Tarea::Simple || tipo == Tarea::Subproceso {
        40.0
    } else {
        44.0
    };
    rotulo(l, 80.0, cy, texto, 15.0, false);
}

#[derive(Clone, Copy)]
enum Marca {
    Aspa,
    Mas,
    Circulo,
    Pentagono,
}

fn compuerta(l: &mut Lapiz, marca: Marca, pregunta: &str) {
    // Las de pregunta la llevan encima; el rombo empieza en y = 30.
    let (cx, cy) = (40.0, 70.0);
    if !pregunta.is_empty() {
        // Parte «Pregunta»: lo que se decide, encima del rombo.
        l.parte("Pregunta");
        rotulo(l, cx, 12.0, pregunta, 14.0, false);
    }
    // Parte «Compuerta»: el rombo.
    l.parte("Compuerta");
    forma(
        l,
        &[
            (cx, cy - 40.0),
            (cx + 40.0, cy),
            (cx, cy + 40.0),
            (cx - 40.0, cy),
        ],
    );
    // Parte «Marca»: lo que dice que clase de compuerta es.
    l.parte("Marca");
    match marca {
        Marca::Aspa => {
            l.linea(cx - 11.0, cy - 11.0, cx + 11.0, cy + 11.0, TRAZO, 5.0);
            l.linea(cx - 11.0, cy + 11.0, cx + 11.0, cy - 11.0, TRAZO, 5.0);
        }
        Marca::Mas => {
            l.linea(cx - 15.0, cy, cx + 15.0, cy, TRAZO, 5.0);
            l.linea(cx, cy - 15.0, cx, cy + 15.0, TRAZO, 5.0);
        }
        Marca::Circulo => l.circulo(cx, cy, 14.0, Some(TRAZO), None, 4.0),
        Marca::Pentagono => {
            l.circulo(cx, cy, 17.0, Some(TRAZO), None, 1.5);
            l.circulo(cx, cy, 13.0, Some(TRAZO), None, 1.5);
            let p: Vec<(f32, f32)> = (0..=5)
                .map(|i| {
                    let a = (-90.0 + 72.0 * i as f32).to_radians();
                    (cx + 8.0 * a.cos(), cy + 1.0 + 8.0 * a.sin())
                })
                .collect();
            l.poli(&p, TRAZO, 1.5, None);
        }
    }
}

/// El pool en vertical: la cabecera con el participante y tres carriles
/// en columna, cada uno con su nombre arriba.
fn pool(l: &mut Lapiz) {
    // Parte «Pool»: el marco y la cabecera con el participante.
    l.parte("Pool");
    l.rect(0.0, 0.0, 600.0, 440.0, Some(TRAZO), Some(BLANCO), G, false);
    l.rect(
        0.0,
        0.0,
        600.0,
        40.0,
        Some(TRAZO),
        Some(RELLENO_FUERTE),
        G,
        false,
    );
    rotulo(l, 300.0, 20.0, "Empresa", TAM, true);
    for (i, nombre) in ["Cliente", "Ventas", "Almacén"].into_iter().enumerate() {
        let x = 200.0 * i as f32;
        // Parte «Carril N»: una columna con su nombre arriba.
        l.parte(&format!("Carril {}", i + 1));
        l.rect(x, 40.0, 200.0, 400.0, Some(TRAZO), None, G, false);
        l.rect(x, 40.0, 200.0, 36.0, Some(TRAZO), Some(RELLENO), G, false);
        rotulo(l, x + 100.0, 58.0, nombre, 15.0, false);
    }
}

fn objeto_de_datos(l: &mut Lapiz) {
    // Parte «Forma»: la hoja con la esquina doblada.
    l.parte("Forma");
    hoja_doblada(l, 20.0, 0.0, 50.0, 70.0, 14.0);
    // Parte «Nombre»: el documento o dato, debajo.
    l.parte("Nombre");
    rotulo(l, 45.0, 88.0, "Factura", 14.0, false);
}

fn anotacion(l: &mut Lapiz) {
    // Parte «Corchete»: el corchete abierto de la anotacion.
    l.parte("Corchete");
    l.poli(
        &[(20.0, 0.0), (0.0, 0.0), (0.0, 70.0), (20.0, 70.0)],
        TRAZO,
        G,
        None,
    );
    // Parte «Texto»: la aclaracion.
    l.parte("Texto");
    rotulo_izq(l, 12.0, 12.0, "Anotación:\nse revisa en 24 h", 14.0, false);
}

fn grupo(l: &mut Lapiz) {
    // Parte «Marco»: el rectangulo de raya y punto que agrupa.
    l.parte("Marco");
    l.rect(0.0, 0.0, 320.0, 200.0, Some(TRAZO), None, G, true);
    if let Some(e) = l.v.last_mut() {
        e.estilo = EstiloTrazo::RayaPunto;
    }
    // Parte «Nombre»: de que va el grupo.
    l.parte("Nombre");
    rotulo(l, 160.0, 20.0, "Grupo", 14.0, true);
}

fn flujo(l: &mut Lapiz, etiqueta: &str, inicio: TipoPunta, fin: TipoPunta, e: EstiloTrazo) {
    if !etiqueta.is_empty() {
        // Parte «Etiqueta»: la condicion o lo que viaja.
        l.parte("Etiqueta");
        rotulo(l, 100.0, 10.0, etiqueta, 14.0, false);
    }
    // Parte «Conector»: la flecha.
    l.parte("Conector");
    conector(l, &[(0.0, 30.0), (200.0, 30.0)], inicio, fin, e);
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::figuras_de_serie::diagramas::pruebas::{comprobar, nombres, partes_estampada};

    #[test]
    fn estan_las_de_bpmn_y_cada_pieza_tiene_su_parte() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(v.len(), 21);
        comprobar(&v, "serie-bpmn-");
    }

    #[test]
    fn la_compuerta_y_el_pool_estampados_conservan_sus_partes() {
        let mut v = Vec::new();
        todas(&mut v);
        assert_eq!(
            partes_estampada(&v, "serie-bpmn-exclusiva"),
            nombres(&["Pregunta", "Compuerta", "Marca"])
        );
        assert_eq!(
            partes_estampada(&v, "serie-bpmn-pool"),
            nombres(&["Pool", "Carril 1", "Carril 2", "Carril 3"])
        );
    }
}
