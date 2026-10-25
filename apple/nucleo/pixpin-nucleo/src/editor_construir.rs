//! **Las cuatro herramientas de construir** en el editor de Swift: el bote
//! de relleno, recortar, extender y el punto etiquetado.
//!
//! Copia de `por-traducir/pixpin/src/dibujo/construir.rs` de Windows (la
//! ventana del editor): la geometria esta en el motor (`regiones`,
//! `recorte`, `puntos_etiquetados`, `angulos`); aqui solo esta lo que el
//! motor no sabe: **en que orden queda la escena y que cuenta como un paso
//! de deshacer**. Tres de ellas no dibujan nada (`deja_rastro` dice que no)
//! y trabajan sobre lo que ya hay; el punto nace del gesto sin letra y aqui
//! se remata. Todo cuelga del `Pulsar`, despues de `Gesto::evento`, igual
//! que en `dibujo::mano::al_motor`.
//!
//! Soldar vertices (`Nudo`) va por la misma puerta, como en Windows, aunque
//! la barra de Mac todavia no la ofrece.

use pixpin_motor2d::elemento::Elemento;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{Gesto, Herramienta};
use pixpin_motor2d::nudos;
use pixpin_motor2d::pintado::Orden;
use pixpin_motor2d::puntos_etiquetados::{self, RADIO_DE_AGARRE, RADIO_PARA_PUNTOS, SerieDePunto};
use pixpin_motor2d::recorte::{self, ALCANCE_EXTENDER};
use pixpin_motor2d::regiones::{self, AjustesRelleno};
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::{ColorRgba, angulos};

/// Si `h` es una de las de construir (para la barra y las pruebas).
pub fn es_de_construir(h: Herramienta) -> bool {
    matches!(
        h,
        Herramienta::Relleno | Herramienta::Recortar | Herramienta::Extender | Herramienta::Punto
    )
}

/// **La puerta unica.** `true` si la escena cambio y hay que repintar.
/// `zoom`: los radios de agarre son de pixeles de pantalla.
pub fn al_pulsar(escena: &mut Escena, gesto: &Gesto, p: Punto2, zoom: f32) -> bool {
    match gesto.herramienta {
        Herramienta::Relleno => bote(escena, gesto, p),
        Herramienta::Recortar => recortar(escena, p, zoom),
        Herramienta::Extender => extender(escena, p, zoom),
        Herramienta::Nudo => nudos::soldar(escena, p, nudos::RADIO_DEL_CLAVO / zoom.max(0.0001)),
        Herramienta::Punto => match gesto.elemento_en_curso() {
            Some((id, _)) => rematar_punto(escena, id, p, zoom, gesto.serie_de_punto),
            None => false,
        },
        _ => false,
    }
}

/// El elemento del que copian el estilo las figuras que nacen aqui: el
/// «actual» del panel, con el grafito si es lo que se tenia en la mano.
fn plantilla(gesto: &Gesto) -> Elemento {
    Elemento {
        trazo: gesto.estilo.trazo,
        relleno: gesto.estilo.relleno,
        estilo_relleno: gesto.estilo.estilo_relleno,
        grosor: gesto.estilo.grosor.de_forma(),
        estilo: gesto.estilo.estilo,
        rugosidad: gesto.estilo.rugosidad,
        opacidad: gesto.estilo.opacidad,
        material: if gesto.grafito_en_la_mano {
            pixpin_motor2d::tinta::MaterialTinta::Cuadritos
        } else {
            gesto.estilo.material
        },
        extras: pixpin_motor2d::elemento::Extras {
            referencia: gesto.modo_guia(),
            dureza: if gesto.grafito_en_la_mano {
                gesto.estilo.dureza.para_guardar()
            } else {
                None
            },
            ..Default::default()
        },
        ..Default::default()
    }
}

/// **El bote de pintura**: quita el relleno que ya hubiera en ese hueco,
/// hace nacer el nuevo y lo baja por debajo de la primera pared que cruza,
/// todo en un paso de deshacer. Dentro de una sola figura cerrada le pone
/// su propio fondo.
fn bote(escena: &mut Escena, gesto: &Gesto, p: Punto2) -> bool {
    let elementos: Vec<Elemento> = escena.elementos.clone();
    let color = gesto.estilo.trazo;
    if !gesto.grafito_en_la_mano
        && let Some(id) = regiones::figura_que_se_rellena_sola(&elementos, p)
    {
        escena.abrir_paso();
        for r in regiones::rellenos_en(&elementos, p) {
            escena.borrar_apuntando(r);
        }
        escena.apuntar_edicion(id);
        if let Some(e) = escena.buscar_mut(id) {
            e.relleno = Some(color);
            e.estilo_relleno = gesto.estilo.estilo_relleno;
            e.tocar();
        }
        escena.cerrar_paso();
        return true;
    }
    // `None` es «este recinto esta abierto»: la escena no se toca.
    let Some(region) = regiones::region_en(&elementos, p, &AjustesRelleno::default()) else {
        return false;
    };
    let mut molde = plantilla(gesto);
    molde.relleno = Some(color);
    let nuevo = regiones::nueva_region(region, &molde);

    escena.abrir_paso();
    for id in regiones::rellenos_en(&elementos, p) {
        escena.borrar_apuntando(id);
    }
    let sitio = regiones::sitio_del_relleno(&escena.elementos, &nuevo);
    let id = escena.anadir(nuevo);
    if sitio < escena.elementos.len() - 1 {
        escena.apuntar_reordenamiento();
        if let Some(i) = escena.elementos.iter().position(|e| e.id == id) {
            let e = escena.elementos.remove(i);
            escena.elementos.insert(sitio, e);
        }
    }
    escena.cerrar_paso();
    true
}

/// **Recortar**: quita el trozo de raya que se toca, hasta donde la cruzan
/// las demas. Si estaba en medio, la raya se parte y la segunda mitad nace
/// con id propio.
fn recortar(escena: &mut Escena, p: Punto2, zoom: f32) -> bool {
    let radio = RADIO_DE_AGARRE / zoom.max(0.0001);
    let Some(id) = mas_cercano(escena, p, radio, recorte::se_recorta) else {
        return false;
    };
    let elementos: Vec<Elemento> = escena.elementos.clone();
    let Some(victima) = elementos.iter().find(|e| e.id == id).cloned() else {
        return false;
    };
    let otros: Vec<Elemento> = elementos.into_iter().filter(|e| e.id != id).collect();
    let Some(r) = recorte::recortar_en(&victima, &otros, p) else {
        return false;
    };
    escena.abrir_paso();
    match r.queda {
        Some(queda) => {
            escena.apuntar_edicion(id);
            if let Some(e) = escena.buscar_mut(id) {
                let suyo = e.id;
                *e = queda;
                e.id = suyo;
            }
        }
        None => {
            escena.borrar_apuntando(id);
        }
    }
    if let Some(nace) = r.nace {
        escena.anadir(nace);
    }
    escena.cerrar_paso();
    true
}

/// **Extender**: estira la punta que se queda corta hasta lo primero que
/// topa. Sin nada en su camino no pasa nada.
fn extender(escena: &mut Escena, p: Punto2, zoom: f32) -> bool {
    let radio = RADIO_DE_AGARRE / zoom.max(0.0001);
    let Some(id) = mas_cercano(escena, p, radio, recorte::es_lineal) else {
        return false;
    };
    let elementos: Vec<Elemento> = escena.elementos.clone();
    let Some(victima) = elementos.iter().find(|e| e.id == id).cloned() else {
        return false;
    };
    let otros: Vec<Elemento> = elementos.into_iter().filter(|e| e.id != id).collect();
    let Some(estirada) = recorte::extender_en(&victima, &otros, p, ALCANCE_EXTENDER) else {
        return false;
    };
    escena.abrir_paso();
    escena.apuntar_edicion(id);
    if let Some(e) = escena.buscar_mut(id) {
        let suyo = e.id;
        *e = estirada;
        e.id = suyo;
    }
    escena.cerrar_paso();
    true
}

/// **Remata el punto recien nacido**: lo pega a un sitio notable y le pone
/// su letra; sin sitio notable cerca, se quita.
fn rematar_punto(escena: &mut Escena, id: u64, p: Punto2, zoom: f32, serie: SerieDePunto) -> bool {
    let radio = RADIO_PARA_PUNTOS / zoom.max(0.0001);
    let otros: Vec<Elemento> = escena
        .elementos
        .iter()
        .filter(|e| e.id != id)
        .cloned()
        .collect();
    let Some(donde) = puntos_etiquetados::sitio_para_punto(&otros, p, radio) else {
        escena.borrar_apuntando(id);
        return true;
    };
    let molde = escena.buscar(id).cloned().unwrap_or_default();
    let rematado = puntos_etiquetados::nuevo_punto(donde, &otros, serie, &molde);
    if let Some(e) = escena.buscar_mut(id) {
        let suyo = e.id;
        *e = rematado;
        e.id = suyo;
    }
    true
}

/// El elemento tocable mas cercano a `p` que cumple `vale`, por su
/// recorrido y no por su caja.
fn mas_cercano(escena: &Escena, p: Punto2, radio: f32, vale: fn(&Elemento) -> bool) -> Option<u64> {
    let mut mejor: Option<(f32, u64)> = None;
    for e in escena.visibles() {
        if e.bloqueado || !vale(e) {
            continue;
        }
        let mut d = f32::MAX;
        for (a, b) in pixpin_motor2d::segmentos_de(e, pixpin_motor2d::PASO_PERIMETRO) {
            d = d.min(pixpin_motor2d::distancia_a_segmento(p, a, b));
        }
        if d <= radio && mejor.is_none_or(|(md, _)| d < md) {
            mejor = Some((d, e.id));
        }
    }
    mejor.map(|(_, id)| id)
}

/// El azul de las guias del editor.
const COLOR_DEL_ROTULO: ColorRgba = ColorRgba {
    r: 0.20,
    g: 0.45,
    b: 0.90,
    a: 0.95,
};

/// **Los angulos que se ven mientras se mueve algo**: no se guardan, se van
/// al soltar. Solo se calculan si hay algo en marcha (la pregunta barata).
pub fn angulos_en_vivo(escena: &Escena, gesto: &Gesto, zoom: f32) -> Vec<Orden> {
    if gesto.en_reposo() {
        return Vec::new();
    }
    let moviendose: Vec<u64> = match gesto.elemento_en_curso() {
        Some((id, _)) => vec![id],
        None if !gesto.seleccion.ids().is_empty() => gesto.seleccion.ids().to_vec(),
        None => return Vec::new(),
    };
    let angulos = angulos::angulos_internos(&escena.elementos, &moviendose, angulos::JUNTA);
    angulos::ordenes_de_angulos(&angulos, zoom, COLOR_DEL_ROTULO)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::elemento::Figura;
    use pixpin_motor2d::gesto::EventoGesto;

    fn raya(a: (f32, f32), b: (f32, f32)) -> Elemento {
        Elemento {
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
            },
            x: a.0.min(b.0),
            y: a.1.min(b.1),
            ancho: (b.0 - a.0).abs(),
            alto: (b.1 - a.1).abs(),
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            grosor: 2.0,
            ..Default::default()
        }
    }

    fn recinto() -> Escena {
        let mut e = Escena::nueva();
        e.anadir(raya((0.0, 0.0), (200.0, 0.0)));
        e.anadir(raya((200.0, 0.0), (200.0, 200.0)));
        e.anadir(raya((200.0, 200.0), (0.0, 200.0)));
        e.anadir(raya((0.0, 200.0), (0.0, 0.0)));
        e
    }

    fn con(h: Herramienta) -> Gesto {
        let mut g = Gesto::nuevo();
        g.herramienta = h;
        g
    }

    #[test]
    fn el_bote_rellena_un_recinto_cerrado_en_un_paso() {
        let mut escena = recinto();
        let pasos = escena.pasos_cerrados();
        assert!(al_pulsar(
            &mut escena,
            &con(Herramienta::Relleno),
            Punto2::nuevo(100.0, 100.0),
            1.0
        ));
        assert!(
            escena
                .visibles()
                .any(|e| matches!(e.figura, Figura::Region { .. }))
        );
        assert_eq!(escena.pasos_cerrados(), pasos + 1);
    }

    #[test]
    fn el_bote_en_un_recinto_abierto_no_toca_nada() {
        let mut escena = Escena::nueva();
        escena.anadir(raya((0.0, 0.0), (200.0, 0.0)));
        let pasos = escena.pasos_cerrados();
        assert!(!al_pulsar(
            &mut escena,
            &con(Herramienta::Relleno),
            Punto2::nuevo(100.0, 100.0),
            1.0
        ));
        assert_eq!(escena.pasos_cerrados(), pasos);
    }

    #[test]
    fn recortar_parte_la_raya_y_deshacer_la_devuelve_entera() {
        let mut escena = Escena::nueva();
        escena.anadir(raya((0.0, 50.0), (300.0, 50.0)));
        escena.anadir(raya((100.0, 0.0), (100.0, 100.0)));
        escena.anadir(raya((200.0, 0.0), (200.0, 100.0)));
        assert!(al_pulsar(
            &mut escena,
            &con(Herramienta::Recortar),
            Punto2::nuevo(150.0, 50.0),
            1.0
        ));
        assert_eq!(escena.visibles().count(), 4);
        assert!(escena.deshacer());
        assert_eq!(escena.visibles().count(), 3);
    }

    #[test]
    fn extender_llega_a_la_pared() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(raya((0.0, 50.0), (50.0, 50.0)));
        escena.anadir(raya((100.0, 0.0), (100.0, 100.0)));
        assert!(al_pulsar(
            &mut escena,
            &con(Herramienta::Extender),
            Punto2::nuevo(50.0, 50.0),
            1.0
        ));
        let p = escena.buscar(id).unwrap().puntos().unwrap().to_vec();
        assert!((p[1].x - 100.0).abs() < 0.01);
    }

    #[test]
    fn el_punto_se_pega_al_cruce_con_su_letra() {
        let mut escena = Escena::nueva();
        escena.anadir(raya((0.0, 0.0), (200.0, 0.0)));
        escena.anadir(raya((80.0, -50.0), (80.0, 50.0)));
        let mut gesto = con(Herramienta::Punto);
        let p = Punto2::nuevo(82.0, 1.0);
        gesto.evento(
            EventoGesto::Pulsar {
                p,
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        assert!(al_pulsar(&mut escena, &gesto, p, 1.0));
        gesto.evento(EventoGesto::Soltar { p }, &mut escena, 1.0);
        let (letra, x, y) = escena
            .visibles()
            .find_map(|e| match &e.figura {
                Figura::Punto { letra, .. } => Some((letra.clone(), e.x, e.y)),
                _ => None,
            })
            .expect("nacio el punto");
        assert_eq!(letra, "A");
        assert_eq!((x, y), (80.0, 0.0));
    }
}
