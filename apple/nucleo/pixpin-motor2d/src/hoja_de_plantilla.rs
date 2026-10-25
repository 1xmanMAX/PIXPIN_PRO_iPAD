//! **La hoja de una plantilla dentro de un lienzo** (26-oct-2026, las
//! carpetas de paginas). El usuario: «al exportar, de cada lienzo solo lo
//! que va sobre la plantilla (la hoja) o sobre el marco que puso; si no hay
//! ninguno de los dos, todo lo dibujado». Y «Nueva pagina»: la misma
//! plantilla otra vez, vacia, en un lienzo nuevo.
//!
//! Una plantilla de serie (un diario, un planificador, un bloc) se estampa
//! **agrupada** (`biblioteca::estampar`: el grupo de la estampacion va el
//! ultimo de `grupos`) y lleva **su pagina**: el rectangulo grande que lo
//! encierra todo (`bloques::leer` → `Hoja.pagina`). Con eso basta para
//! encontrarla en un lienzo lleno de cosas escritas encima: se agrupa por el
//! grupo mas exterior y se pregunta a `bloques` por la pagina de cada grupo.
//! Lo escrito encima no es del grupo (se dibujo despues), y por eso se puede
//! separar la plantilla de lo escrito sin adivinar nada.
//!
//! Todo es puro: recibe la escena y devuelve cajas y piezas.

use crate::biblioteca::{self, FiguraGuardada};
use crate::bloques;
use crate::elemento::Elemento;
use crate::escena::Escena;
use crate::figuras_de_serie;

/// Lo minimo que mide de area la pagina de una plantilla que no es de serie
/// (una propia del usuario): asi un simbolo agrupado con su recuadro (una
/// viñeta de storyboard, un rotulo) no pasa por hoja. Un A4 de serie mide
/// 794 x 1123.
const AREA_MINIMA: f32 = 300.0 * 300.0;

/// Una plantilla encontrada en el lienzo.
#[derive(Debug, Clone, PartialEq)]
pub struct Plantilla {
    /// La caja de su pagina, `(x0, y0, x1, y1)`.
    pub caja: (f32, f32, f32, f32),
    /// Las piezas de la plantilla (las del grupo), sin lo escrito encima.
    pub ids: Vec<u64>,
    /// El grupo con que se estampo, o vacio si no iba agrupada.
    pub grupo: String,
    /// Que plantilla de serie es (`figuras_de_serie::cual_es`), si lo es.
    pub clave: Option<String>,
}

fn area(c: (f32, f32, f32, f32)) -> f32 {
    (c.2 - c.0).max(0.0) * (c.3 - c.1).max(0.0)
}

/// Si estas piezas son una plantilla: tienen pagina y o son de serie o la
/// pagina es grande.
fn plantilla_de(piezas: &[Elemento], grupo: &str) -> Option<Plantilla> {
    if piezas.len() < 2 {
        return None;
    }
    let (_, caja) = bloques::leer(piezas).pagina?;
    let clave = figuras_de_serie::cual_es(piezas);
    if clave.is_none() && area(caja) < AREA_MINIMA {
        return None;
    }
    Some(Plantilla {
        caja,
        ids: piezas.iter().map(|e| e.id).collect(),
        grupo: grupo.to_string(),
        clave,
    })
}

/// **Las plantillas del lienzo**, en el orden en que se leen (de arriba
/// abajo y, en la misma altura, de izquierda a derecha).
///
/// Primero por grupos: cada grupo exterior con pagina es una plantilla. Si
/// no hay ninguna agrupada (el usuario la desagrupo), se mira el lienzo
/// entero, y cuenta solo si es de serie: sin el grupo, un lienzo con un
/// rectangulo grande no tiene por que ser una hoja.
pub fn plantillas(escena: &Escena) -> Vec<Plantilla> {
    let vivos: Vec<&Elemento> = escena.a_la_vista().collect();
    let mut grupos: Vec<(String, Vec<Elemento>)> = Vec::new();
    for e in &vivos {
        let Some(g) = e.grupos.last() else { continue };
        match grupos.iter_mut().find(|(n, _)| n == g) {
            Some((_, v)) => v.push((*e).clone()),
            None => grupos.push((g.clone(), vec![(*e).clone()])),
        }
    }
    let mut v: Vec<Plantilla> = grupos
        .iter()
        .filter_map(|(g, piezas)| plantilla_de(piezas, g))
        .collect();
    if v.is_empty() {
        let todas: Vec<Elemento> = vivos.iter().map(|e| (*e).clone()).collect();
        if let Some(p) = plantilla_de(&todas, "").filter(|p| p.clave.is_some()) {
            v.push(p);
        }
    }
    v.sort_by(|a, b| {
        a.caja
            .1
            .total_cmp(&b.caja.1)
            .then(a.caja.0.total_cmp(&b.caja.0))
    });
    v
}

/// **La plantilla vacia de la ultima hoja del lienzo**, lista para estampar
/// en uno nuevo: sus piezas, en el origen.
///
/// Si es una de serie y tiene las mismas piezas que la de serie, sale la de
/// serie **tal cual** (sin nada de lo que se haya tecleado dentro de sus
/// textos); si no (una alargada, una propia, una retocada), sus piezas como
/// estan, sin lo escrito encima. `None` si el lienzo no tiene plantilla.
pub fn vacia(escena: &Escena) -> Option<FiguraGuardada> {
    let p = plantillas(escena).pop()?;
    let piezas: Vec<Elemento> = escena
        .a_la_vista()
        .filter(|e| p.ids.contains(&e.id))
        .cloned()
        .map(|mut e| {
            // El grupo de la estampacion se quita: el nuevo lienzo pone el
            // suyo al estampar. Los de dentro de la plantilla se quedan.
            if !p.grupo.is_empty() && e.grupos.last() == Some(&p.grupo) {
                e.grupos.pop();
            }
            e
        })
        .collect();
    let de_serie = p.clave.as_deref().and_then(|clave| {
        let id = format!("{}{clave}", figuras_de_serie::PREFIJO_ID);
        figuras_de_serie::catalogo()
            .into_iter()
            .find(|f| f.id == id)
            .filter(|f| f.elementos.iter().filter(|e| !e.borrado).count() == piezas.len())
    });
    Some(match de_serie {
        Some(f) => f,
        None => FiguraGuardada {
            id: biblioteca::id_nuevo(),
            nombre: p.clave.unwrap_or_default(),
            elementos: biblioteca::normalizar(&piezas),
        },
    })
}

/// **Un lienzo nuevo con la plantilla vacia de `escena`**, con el mismo
/// papel. `None` si `escena` no tiene plantilla: entonces la pagina nueva es
/// un lienzo en blanco, y eso lo decide quien llama.
pub fn lienzo_siguiente(escena: &Escena) -> Option<Escena> {
    let f = vacia(escena)?;
    let mut nueva = Escena::nueva();
    nueva.fondo = escena.fondo;
    let (w, h) = f.tamano();
    biblioteca::estampar_en_escena(
        &mut nueva,
        &f,
        crate::vector::Punto2::nuevo(w / 2.0, h / 2.0),
    );
    Some(nueva)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::Figura;

    fn de_serie(clave: &str) -> FiguraGuardada {
        let id = format!("{}{clave}", figuras_de_serie::PREFIJO_ID);
        figuras_de_serie::catalogo()
            .into_iter()
            .find(|f| f.id == id)
            .unwrap_or_else(|| panic!("no hay {clave}"))
    }

    /// Las piezas de serie llevan todas el id 0 (se numeran al estamparlas),
    /// asi que se prueban siempre estampadas, como estan en un lienzo.
    fn una_de_diario() -> FiguraGuardada {
        de_serie("diario")
    }

    fn trazo(x: f32, y: f32) -> Elemento {
        Elemento {
            x,
            y,
            ancho: 40.0,
            alto: 10.0,
            figura: Figura::Rectangulo,
            ..Elemento::default()
        }
    }

    #[test]
    fn la_plantilla_estampada_se_encuentra_con_lo_escrito_encima_aparte() {
        let f = una_de_diario();
        let mut e = Escena::nueva();
        let (w, h) = f.tamano();
        let ids = biblioteca::estampar_en_escena(
            &mut e,
            &f,
            crate::vector::Punto2::nuevo(w / 2.0, h / 2.0),
        );
        // Lo escrito encima, despues y sin grupo.
        let encima = e.anadir(trazo(100.0, 200.0));
        let v = plantillas(&e);
        assert_eq!(v.len(), 1, "una sola hoja");
        assert!(v[0].clave.is_some(), "es de serie");
        assert_eq!(v[0].ids.len(), ids.len());
        assert!(
            !v[0].ids.contains(&encima),
            "lo escrito no es de la plantilla"
        );
        let (x0, y0, x1, y1) = v[0].caja;
        assert!(x1 - x0 > 500.0 && y1 - y0 > 500.0, "{:?}", v[0].caja);
    }

    #[test]
    fn la_pagina_siguiente_es_la_plantilla_sin_lo_escrito() {
        let f = una_de_diario();
        let mut e = Escena::nueva();
        let (w, h) = f.tamano();
        biblioteca::estampar_en_escena(&mut e, &f, crate::vector::Punto2::nuevo(w / 2.0, h / 2.0));
        e.anadir(trazo(100.0, 200.0));
        e.anadir(trazo(150.0, 260.0));
        let nueva = lienzo_siguiente(&e).expect("tiene plantilla");
        assert_eq!(
            nueva.cuantos_visibles(),
            f.elementos.iter().filter(|x| !x.borrado).count(),
            "solo las piezas de la plantilla"
        );
        assert_eq!(plantillas(&nueva).len(), 1, "y se vuelve a reconocer");
    }

    #[test]
    fn un_texto_tecleado_dentro_de_la_plantilla_no_pasa_a_la_siguiente() {
        let f = una_de_diario();
        let mut e = Escena::nueva();
        let (w, h) = f.tamano();
        let ids = biblioteca::estampar_en_escena(
            &mut e,
            &f,
            crate::vector::Punto2::nuevo(w / 2.0, h / 2.0),
        );
        // El usuario escribio dentro de un texto pequeno de la plantilla (no
        // el titulo, que es por donde se reconoce).
        let pequeno = ids.iter().copied().find(
            |id| matches!(&e.buscar(*id).unwrap().figura, Figura::Texto { tam, .. } if *tam < 30.0),
        );
        if let Some(id) = pequeno
            && let Some(Figura::Texto { texto, .. }) = e.buscar_mut(id).map(|x| &mut x.figura)
        {
            *texto = "Lo que paso el lunes".into();
        }
        let vacia = vacia(&e).unwrap();
        let textos: Vec<String> = vacia
            .elementos
            .iter()
            .filter_map(|x| match &x.figura {
                Figura::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect();
        assert!(!textos.iter().any(|t| t.contains("lunes")), "{textos:?}");
    }

    #[test]
    fn varias_hojas_salen_en_orden_de_lectura() {
        let f = de_serie("planificador-semanal");
        let (w, h) = f.tamano();
        let mut e = Escena::nueva();
        // La segunda, debajo; la primera, arriba (se estampan al reves).
        biblioteca::estampar_en_escena(
            &mut e,
            &f,
            crate::vector::Punto2::nuevo(w / 2.0, h * 1.5 + 100.0),
        );
        biblioteca::estampar_en_escena(&mut e, &f, crate::vector::Punto2::nuevo(w / 2.0, h / 2.0));
        let v = plantillas(&e);
        assert_eq!(v.len(), 2);
        assert!(v[0].caja.1 < v[1].caja.1, "la de arriba primero");
    }

    #[test]
    fn sin_plantilla_no_hay_hoja_ni_pagina_siguiente() {
        let mut e = Escena::nueva();
        e.anadir(trazo(0.0, 0.0));
        e.anadir(trazo(30.0, 40.0));
        assert!(plantillas(&e).is_empty());
        assert!(lienzo_siguiente(&e).is_none());
        // Caso negativo: un recuadro pequeno agrupado con algo dentro no es
        // una hoja (un simbolo, un rotulo).
        let mut s = Escena::nueva();
        let mut caja = trazo(0.0, 0.0);
        caja.ancho = 100.0;
        caja.alto = 80.0;
        caja.grupos = vec!["g1".into()];
        let mut dentro = trazo(10.0, 10.0);
        dentro.grupos = vec!["g1".into()];
        s.anadir(caja);
        s.anadir(dentro);
        assert!(plantillas(&s).is_empty());
    }
}
