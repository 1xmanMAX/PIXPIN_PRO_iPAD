//! **Las plantillas de serie en el lienzo** (Windows,
//! `asistente/al_lienzo.rs`, 22–25-oct): rehacerlas con otro intervalo
//! («damela del 1 al 40», «de 10 en 10»), personalizar sus secciones, de
//! noche o de dia, y la guia de lo que se les puede cambiar para el modelo.
//! Todo sobre la escena: vale igual para el lienzo abierto que para uno
//! cerrado ([`super::en_el_chat`]).

use pixpin_motor2d::Escena;
use pixpin_motor2d::biblioteca::FiguraGuardada;
use pixpin_motor2d::elemento::Elemento;

use super::{Respuesta, caja};

const ENTRE_HOJAS: f32 = 60.0;

/// **«Damela del 1 al 40», «de 10 en 10»**: la plantilla de serie elegida
/// (o la de `nombre`) rehecha con ese intervalo, con el mismo diseño
/// (`figuras_de_serie::alargar`). Con `intervalo.secciones`, tambien
/// **personalizada** (25-oct: quitar, reordenar, ensanchar secciones), con
/// el mismo codigo que la dibuja. Con `intervalo.noche`, **de noche** (o de
/// dia): sin decirlo, la que se rehace se queda en su modo, y la de
/// «nombre» sale como se llame («Cronograma (noche)» de noche).
///
/// Si lo elegido es esa plantilla, las hojas nuevas **ocupan su sitio**: la
/// primera donde estaba y las demas en fila a su derecha; la vieja se borra
/// en el mismo paso, asi que Ctrl+Z la devuelve. Si no, van centradas en
/// `centro` (lo que se ve) o, con el lienzo cerrado, a la derecha de lo que
/// hubiera. Las hojas quedan elegidas, listas para «Guardar en figuras».
pub fn poner_plantilla(
    escena: &mut Escena,
    elegidos: &[u64],
    nombre: Option<&str>,
    intervalo: &pixpin_motor2d::figuras_de_serie::Intervalo,
    centro: Option<(f32, f32)>,
) -> Result<Respuesta, String> {
    use pixpin_motor2d::figuras_de_serie as serie;
    let mut elegidas: Vec<Elemento> = escena
        .visibles()
        .filter(|e| elegidos.contains(&e.id))
        .cloned()
        .collect();
    let mut la_elegida = serie::cual_es(&elegidas);
    // Solo el modo («ponlo en modo noche»): vale cualquier plantilla de
    // pagina, tambien las que no se alargan ni se personalizan.
    let solo_modo = intervalo.noche.is_some()
        && intervalo.secciones.is_none()
        && intervalo.desde.is_none()
        && intervalo.hasta.is_none()
        && intervalo.de_a.is_none()
        && intervalo.semanas.is_none()
        && intervalo.mes.is_none()
        && intervalo.meses.is_none();
    // **Sin elegirla** (25-oct-2026, el usuario abrio el cronograma y pidio
    // agrandarlo sin seleccionarlo): si en el lienzo hay una sola plantilla
    // que se alarga (y es la pedida, si se nombro), es esa, y se rehace en
    // su sitio con su mismo diseño.
    if la_elegida.is_none() {
        let pedida = nombre.and_then(|n| {
            let catalogo = serie::catalogo();
            pixpin_ia::figuras::buscar(&catalogo, n)
                .and_then(serie::clave_de)
                .map(str::to_string)
        });
        if let Some((c, v)) = plantilla_en_la_escena(escena, pedida.as_deref(), solo_modo) {
            la_elegida = Some(c);
            elegidas = v;
        }
    }
    // Pedida por su nombre de noche («cronograma (noche)»): de noche.
    let mut nombrada_de_noche = false;
    let clave = match nombre {
        Some(n) => {
            let catalogo = serie::catalogo();
            let f = pixpin_ia::figuras::buscar(&catalogo, n).ok_or_else(|| {
                format!(
                    "no hay ninguna plantilla llamada «{n}»; se alargan: {}",
                    serie::las_que_se_alargan()
                )
            })?;
            nombrada_de_noche = serie::es_la_de_noche(f);
            serie::clave_de(f).unwrap_or_default().to_string()
        }
        None => la_elegida.clone().ok_or_else(|| {
            if elegidas.is_empty() {
                "no hay nada elegido: elige en el lienzo la plantilla (o di cual quieres)"
                    .to_string()
            } else {
                "lo elegido no es una plantilla de serie (se reconocen por su titulo)".to_string()
            }
        })?,
    };
    // **Lo que no se dice se queda como esta** (25-oct-2026: «redúcelo a 10
    // semanas» volvia a las 18 actividades de fabrica): rehaciendo la que hay,
    // las filas y las semanas que no se piden salen de las suyas.
    // Lo mismo con el modo (25-oct): la de noche se rehace de noche salvo
    // que se pida el de dia, y el mes de un mensual o unos habitos se queda.
    let completo;
    let intervalo = if la_elegida.as_deref() == Some(clave.as_str()) {
        let actual = tramo_actual(&elegidas);
        let mut iv = intervalo.clone();
        let sin_numeros =
            iv.desde.is_none() && iv.hasta.is_none() && iv.mes.is_none() && iv.de_a.is_none();
        let por_meses = matches!(clave.as_str(), "planificador-mensual" | "habitos");
        if sin_numeros && por_meses && actual.mes.is_some() {
            iv.mes = actual.mes;
        } else if sin_numeros && serie::se_alarga(&clave) && clave != "planificador-mensual" {
            if let Some((a, b)) = actual.filas {
                iv.desde = Some(a);
                iv.hasta = Some(b);
            }
        }
        if iv.semanas.is_none() && clave == "cronograma" {
            iv.semanas = actual.semanas;
        }
        if iv.noche.is_none() && (actual.noche || nombrada_de_noche) {
            iv.noche = Some(true);
        }
        completo = iv;
        &completo
    } else if intervalo.noche.is_none() && nombrada_de_noche {
        completo = pixpin_motor2d::figuras_de_serie::Intervalo {
            noche: Some(true),
            ..intervalo.clone()
        };
        &completo
    } else {
        intervalo
    };
    let hojas = serie::alargar(&clave, intervalo)?;
    let (ancho, alto) = medida_de_las_hojas(&hojas);
    let reemplaza = la_elegida.as_deref() == Some(clave.as_str());
    // La esquina de la primera hoja.
    let (x0, y0) = match (reemplaza, centro) {
        (true, _) => caja(&elegidas).map(|c| (c.0, c.1)).unwrap_or_default(),
        (false, Some((cx, cy))) => (cx - ancho / 2.0, cy - alto / 2.0),
        (false, None) => match caja(&escena.visibles().cloned().collect::<Vec<_>>()) {
            Some(c) => (c.2 + HUECO_PLANTILLA, c.1),
            None => (0.0, 0.0),
        },
    };
    escena.abrir_paso();
    if reemplaza {
        for e in &elegidas {
            escena.borrar_apuntando(e.id);
        }
    }
    let ids = colocar_hojas(escena, &hojas, (x0, y0));
    escena.cerrar_paso();
    let que = serie::nombre_de(&clave);
    let texto = if solo_modo {
        format!(
            "listo: «{que}» rehecha {} con el mismo diseño ({})",
            if intervalo.noche == Some(true) {
                "en modo noche"
            } else {
                "de día"
            },
            hojas[0].nombre
        )
    } else if intervalo.secciones.is_some() && hojas.len() == 1 {
        format!(
            "listo: «{que}» rehecha con las secciones pedidas ({})",
            hojas[0].nombre
        )
    } else if hojas.len() == 1 {
        format!("listo: «{que}» rehecha en una hoja ({})", hojas[0].nombre)
    } else {
        format!(
            "listo: «{que}» en {} hojas, una al lado de otra",
            hojas.len()
        )
    };
    let texto = format!(
        "{texto}{}. Quedan elegidas: el usuario puede guardarlas con «Guardar en figuras» del panel lateral, o pedirtelo.",
        if reemplaza {
            ", en el sitio de la que estaba elegida (Ctrl+Z la devuelve)"
        } else {
            ""
        }
    );
    Ok(Respuesta {
        texto,
        ids,
        ..Default::default()
    })
}

/// Lo que lleva ahora una plantilla puesta (ver [`tramo_actual`]).
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Actual {
    /// Sus filas: la numeracion «01», «02»… seguida mas larga.
    pub filas: Option<(u32, u32)>,
    /// En el cronograma, sus semanas («S1»…).
    pub semanas: Option<(u32, u32)>,
    /// El mes de un mensual o unos habitos con el calendario de verdad.
    pub mes: Option<(i32, u32)>,
    /// Si es de noche (su papel es oscuro).
    pub noche: bool,
}

/// **Lo que lleva ahora una plantilla puesta**: sus filas, sus semanas, su
/// mes y su modo, para rehacerla sin perder nada de eso.
pub(crate) fn tramo_actual(piezas: &[Elemento]) -> Actual {
    let textos: Vec<&str> = piezas
        .iter()
        .filter_map(|e| match &e.figura {
            pixpin_motor2d::Figura::Texto { texto, .. } => Some(texto.trim()),
            _ => None,
        })
        .collect();
    let mut numeros: Vec<u32> = textos
        .iter()
        .filter(|t| t.len() >= 2 && t.chars().all(|c| c.is_ascii_digit()))
        .filter_map(|t| t.parse().ok())
        .collect();
    numeros.sort_unstable();
    numeros.dedup();
    // La tirada seguida mas larga: la de las filas, no un año o un dia suelto.
    let mut mejor: Option<(u32, u32)> = None;
    let mut i = 0;
    while i < numeros.len() {
        let mut j = i;
        while j + 1 < numeros.len() && numeros[j + 1] == numeros[j] + 1 {
            j += 1;
        }
        if j > i && mejor.is_none_or(|(a, b)| numeros[j] - numeros[i] > b - a) {
            mejor = Some((numeros[i], numeros[j]));
        }
        i = j + 1;
    }
    let semanas: Vec<u32> = textos
        .iter()
        .filter_map(|t| t.strip_prefix('S')?.parse::<u32>().ok())
        .collect();
    let semanas = semanas
        .iter()
        .min()
        .zip(semanas.iter().max())
        .map(|(a, b)| (*a, *b));
    Actual {
        filas: mejor,
        semanas,
        mes: pixpin_motor2d::figuras_de_serie::mes_de_la_hoja(piezas),
        noche: pixpin_motor2d::figuras_de_serie::es_de_noche(piezas),
    }
}

/// **La plantilla de serie que hay en el lienzo** (la que se alarga y, si se
/// dice, la de `clave`), con sus piezas: las de un mismo grupo exterior, que
/// es el que pone estampar. `None` si no hay ninguna o hay varias (entonces
/// hay que elegirla). Con `cualquiera`, tambien las que no se alargan ni se
/// personalizan (para cambiarles solo el modo, de dia o de noche).
pub(crate) fn plantilla_en_la_escena(
    escena: &Escena,
    clave: Option<&str>,
    cualquiera: bool,
) -> Option<(String, Vec<Elemento>)> {
    use pixpin_motor2d::figuras_de_serie as serie;
    let mut grupos: Vec<(String, Vec<Elemento>)> = Vec::new();
    for e in escena.visibles() {
        let Some(g) = e.grupos.last() else {
            continue;
        };
        match grupos.iter_mut().find(|(n, _)| n == g) {
            Some((_, v)) => v.push(e.clone()),
            None => grupos.push((g.clone(), vec![e.clone()])),
        }
    }
    let mut halladas: Vec<(String, Vec<Elemento>)> = grupos
        .into_iter()
        .filter_map(|(_, v)| serie::cual_es(&v).map(|c| (c, v)))
        .filter(|(c, _)| {
            (cualquiera || serie::se_alarga(c) || serie::se_personaliza(c))
                && clave.is_none_or(|k| k == c)
        })
        .collect();
    if halladas.len() == 1 {
        return halladas.pop();
    }
    if !halladas.is_empty() {
        return None;
    }
    // **Sin grupo** (desagrupada, o venida de otro sitio): por su titulo, y
    // sus piezas son lo que cae dentro de la hoja que lo encierra.
    let vivos: Vec<Elemento> = escena.visibles().cloned().collect();
    let c = serie::cual_es(&vivos)
        .filter(|c| (cualquiera || serie::se_alarga(c)) && clave.is_none_or(|k| k == c))?;
    let hojas: Vec<&Elemento> = vivos
        .iter()
        .filter(|e| matches!(e.figura, pixpin_motor2d::Figura::Rectangulo))
        .filter(|r| {
            let caja = r.caja();
            let dentro: Vec<Elemento> = vivos
                .iter()
                .filter(|o| {
                    let oc = o.caja();
                    let (cx, cy) = ((oc.0 + oc.2) / 2.0, (oc.1 + oc.3) / 2.0);
                    cx >= caja.0 && cx <= caja.2 && cy >= caja.1 && cy <= caja.3
                })
                .cloned()
                .collect();
            dentro.len() >= 8 && serie::cual_es(&dentro).as_deref() == Some(c.as_str())
        })
        .collect();
    // La hoja: el rectangulo mas grande que la contiene, una sola vez.
    let hoja = hojas.iter().max_by(|a, b| {
        let area = |e: &Elemento| {
            let c = e.caja();
            (c.2 - c.0) * (c.3 - c.1)
        };
        area(a).total_cmp(&area(b))
    })?;
    let caja = hoja.caja();
    let piezas: Vec<Elemento> = vivos
        .into_iter()
        .filter(|o| {
            let oc = o.caja();
            let (cx, cy) = ((oc.0 + oc.2) / 2.0, (oc.1 + oc.3) / 2.0);
            cx >= caja.0 - 0.5 && cx <= caja.2 + 0.5 && cy >= caja.1 - 0.5 && cy <= caja.3 + 0.5
        })
        .collect();
    Some((c, piezas))
}

/// Con el lienzo cerrado, a cuanto de lo que habia.
const HUECO_PLANTILLA: f32 = 120.0;

/// **Lo que la plantilla tiene AHORA** (24-oct-2026, banco con Haiku): la
/// guia trae las secciones de serie, y con una ya personalizada el modelo
/// volvia a mandarlas y deshacia lo de antes. Sus bloques con nombre (las
/// partes `parte:<nombre>`), en orden de arriba abajo.
fn ahora_tiene(piezas: &[Elemento]) -> String {
    let mut vistos: Vec<(String, f32, f32)> = Vec::new();
    for e in piezas {
        let Some(n) = e
            .grupos
            .iter()
            .find_map(|g| pixpin_motor2d::biblioteca::nombre_de_parte(g))
        else {
            continue;
        };
        let c = e.caja();
        match vistos.iter_mut().find(|(v, _, _)| v == n) {
            Some(v) => {
                v.1 = v.1.min(c.1);
                v.2 = v.2.min(c.0);
            }
            None => vistos.push((n.to_string(), c.1, c.0)),
        }
    }
    if vistos.is_empty() {
        return String::new();
    }
    vistos.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)));
    let nombres: Vec<String> = vistos.iter().map(|(n, _, _)| format!("«{n}»")).collect();
    format!(
        "\nLo que tiene AHORA en el lienzo (de arriba abajo): {}. Si difiere de lo de serie es que ya se personalizó: \
parte de esto (no vuelvas a poner lo que se quitó y conserva lo que se añadió), cambiando solo lo pedido.",
        nombres.join(", ")
    )
}

/// Como mucho cuantas guias de plantilla van en el resumen (son largas).
const MAX_GUIAS: usize = 2;

/// **La guia de las plantillas que hay en el lienzo** (25-oct): por cada
/// plantilla de serie que se personaliza, sus bloques y lo que se le puede
/// cambiar (`figuras_de_serie::guia_de`), para que el modelo la entienda
/// sin tener que pedirla. Vacio si no hay ninguna.
pub fn guias_del_lienzo(escena: &Escena) -> String {
    use pixpin_motor2d::figuras_de_serie as serie;
    let mut grupos: Vec<(String, Vec<Elemento>)> = Vec::new();
    for e in escena.visibles() {
        let Some(g) = e.grupos.last() else {
            continue;
        };
        match grupos.iter_mut().find(|(n, _)| n == g) {
            Some((_, v)) => v.push(e.clone()),
            None => grupos.push((g.clone(), vec![e.clone()])),
        }
    }
    let mut claves: Vec<(String, String)> = Vec::new();
    for (_, v) in &grupos {
        if let Some(c) = serie::cual_es(v)
            && serie::se_personaliza(&c)
            && !claves.iter().any(|(k, _)| *k == c)
        {
            claves.push((c, ahora_tiene(v)));
        }
    }
    claves
        .iter()
        .take(MAX_GUIAS)
        .filter_map(|(c, ahora)| serie::guia_de(c).map(|g| format!("\n\n{g}{ahora}")))
        .collect()
}

/// **Las secciones de la plantilla del lienzo**, en palabras para el
/// modelo (`figuras_de_serie::guia_de`): la elegida o, sin
/// elegir, la unica plantilla que hay. Error en palabras si no hay ninguna
/// o no se personaliza.
pub fn secciones_de_la_plantilla(escena: &Escena, elegidos: &[u64]) -> Result<String, String> {
    use pixpin_motor2d::figuras_de_serie as serie;
    let elegidas: Vec<Elemento> = escena
        .visibles()
        .filter(|e| elegidos.contains(&e.id))
        .cloned()
        .collect();
    let (clave, piezas) = serie::cual_es(&elegidas)
        .map(|c| (c, elegidas.clone()))
        .or_else(|| plantilla_en_la_escena(escena, None, false))
        .ok_or_else(|| {
            format!(
                "no encuentro la plantilla en el lienzo: elígela o di su «nombre» (se personalizan: {})",
                serie::las_que_se_personalizan()
            )
        })?;
    let ahora = ahora_tiene(&piezas);
    serie::guia_de(&clave)
        .map(|g| format!("{g}{ahora}"))
        .ok_or_else(|| {
            format!(
                "«{}» no tiene secciones que cambiar; se personalizan: {}",
                serie::nombre_de(&clave),
                serie::las_que_se_personalizan()
            )
        })
}


fn medida_de_las_hojas(hojas: &[FiguraGuardada]) -> (f32, f32) {
    let ancho = hojas.iter().map(|f| f.tamano().0).sum::<f32>()
        + ENTRE_HOJAS * hojas.len().saturating_sub(1) as f32;
    (
        ancho,
        hojas.iter().map(|f| f.tamano().1).fold(0.0, f32::max),
    )
}

/// **Unas hojas en fila** desde `(x0, y0)`, cada una su grupo, en el paso
/// de deshacer que este abierto. Devuelve los ids.
fn colocar_hojas(escena: &mut Escena, hojas: &[FiguraGuardada], (x0, y0): (f32, f32)) -> Vec<u64> {
    let mut ids = Vec::new();
    let mut x = x0;
    for f in hojas {
        let (w, h) = f.tamano();
        let primero = escena.siguiente_id.max(1);
        let grupo = format!("g{primero}");
        let centro = pixpin_motor2d::vector::Punto2::nuevo(x + w / 2.0, y0 + h / 2.0);
        for e in pixpin_motor2d::biblioteca::estampar(f, centro, primero, &grupo) {
            ids.push(escena.anadir(e));
        }
        x += w + ENTRE_HOJAS;
    }
    ids
}

/// Las hojas de una plantilla alargada en medio de `centro` (la ventana de
/// las figuras), en un solo paso de deshacer.
pub fn colocar_hojas_en_medio(
    escena: &mut Escena,
    hojas: &[FiguraGuardada],
    centro: (f32, f32),
) -> Vec<u64> {
    let (ancho, alto) = medida_de_las_hojas(hojas);
    escena.abrir_paso();
    let ids = colocar_hojas(
        escena,
        hojas,
        (centro.0 - ancho / 2.0, centro.1 - alto / 2.0),
    );
    escena.cerrar_paso();
    ids
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::biblioteca;
    use pixpin_motor2d::figuras_de_serie::{self as serie, Intervalo};

    /// Un lienzo con la lista de tareas de serie estampada y elegida.
    fn con_la_lista() -> (Escena, Vec<u64>) {
        let mut escena = Escena::nueva();
        let f = serie::catalogo()
            .into_iter()
            .find(|f| f.id == "serie-lista-de-tareas")
            .unwrap();
        let ids = biblioteca::estampar_en_escena(
            &mut escena,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(500.0, 600.0),
        );
        (escena, ids)
    }

    #[test]
    fn la_plantilla_elegida_se_rehace_en_su_sitio_en_hojas_y_se_deshace_de_una() {
        let (mut escena, ids) = con_la_lista();
        let caja_vieja = caja(&escena.visibles().cloned().collect::<Vec<_>>()).unwrap();
        let iv = Intervalo {
            desde: Some(1),
            hasta: Some(40),
            de_a: Some(10),
            ..Default::default()
        };
        let r = poner_plantilla(&mut escena, &ids, None, &iv, Some((0.0, 0.0))).unwrap();
        assert!(r.texto.contains("4 hojas"), "{}", r.texto);
        assert!(
            ids.iter()
                .all(|id| escena.buscar(*id).is_some_and(|e| e.borrado)),
            "la vieja se va"
        );
        let nuevas: Vec<Elemento> = escena.visibles().cloned().collect();
        let grupos: std::collections::HashSet<&String> =
            nuevas.iter().filter_map(|e| e.grupos.last()).collect();
        assert_eq!(grupos.len(), 4, "una hoja, un grupo");
        let c = caja(&nuevas).unwrap();
        assert!(
            (c.0 - caja_vieja.0).abs() < 0.5 && (c.1 - caja_vieja.1).abs() < 0.5,
            "empieza donde estaba la vieja"
        );
        assert_eq!(r.ids.len(), nuevas.len());
        // Un Ctrl+Z la devuelve tal cual.
        escena.deshacer();
        assert_eq!(escena.visibles().count(), ids.len());
        assert!(
            ids.iter()
                .all(|id| escena.buscar(*id).is_some_and(|e| !e.borrado))
        );
    }

    /// 25-oct-2026: el mensual en el lienzo, sin elegir, y «quita
    /// prioridades, objetivos a 6 lineas y a todo el ancho, tareas a 4»: se
    /// rehace en su sitio con sus secciones, y Ctrl+Z lo devuelve.
    #[test]
    fn el_mensual_se_personaliza_en_su_sitio() {
        use pixpin_motor2d::figuras_de_serie::{AnchoSeccion, SeccionPedida};
        let mut escena = Escena::nueva();
        let f = serie::catalogo()
            .into_iter()
            .find(|f| f.id == "serie-planificador-mensual")
            .unwrap();
        let ids = biblioteca::estampar_en_escena(
            &mut escena,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(200.0, 100.0),
        );
        let caja_vieja = caja(&escena.visibles().cloned().collect::<Vec<_>>()).unwrap();
        // Primero se leen sus secciones; el resumen ya trae su guia.
        let t = secciones_de_la_plantilla(&escena, &[]).unwrap();
        assert!(t.contains("Prioridades"), "{t}");
        assert!(
            guias_del_lienzo(&escena).contains("Calendario")
                || guias_del_lienzo(&escena).contains("calendario")
        );
        assert!(guias_del_lienzo(&Escena::nueva()).is_empty());
        let iv = Intervalo {
            secciones: Some(vec![
                SeccionPedida {
                    titulo: "Objetivo del mes".into(),
                    lineas: Some(6),
                    ancho: Some(AnchoSeccion::Completo),
                    ..Default::default()
                },
                SeccionPedida {
                    titulo: "Tareas".into(),
                    lineas: Some(4),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };
        let r = poner_plantilla(&mut escena, &[], None, &iv, None).unwrap();
        assert!(r.texto.contains("secciones pedidas"), "{}", r.texto);
        let nuevas: Vec<Elemento> = escena.visibles().cloned().collect();
        let textos: Vec<&str> = nuevas
            .iter()
            .filter_map(|e| match &e.figura {
                pixpin_motor2d::elemento::Figura::Texto { texto, .. } => Some(texto.as_str()),
                _ => None,
            })
            .collect();
        assert!(!textos.contains(&"PRIORIDADES") && textos.contains(&"TAREAS"));
        // La guia dice lo que tiene AHORA (sin Prioridades), no solo lo de serie.
        let t = secciones_de_la_plantilla(&escena, &[]).unwrap();
        let ahora = &t[t.find("AHORA").expect("dice lo de ahora")..];
        assert!(
            ahora.contains("«Tareas»") && !ahora.contains("«Prioridades»"),
            "{ahora}"
        );
        let c = caja(&nuevas).unwrap();
        assert!((c.0 - caja_vieja.0).abs() < 0.5 && (c.1 - caja_vieja.1).abs() < 0.5);
        escena.deshacer();
        assert!(
            ids.iter()
                .all(|id| escena.buscar(*id).is_some_and(|e| !e.borrado))
        );
        // Caso negativo: un lienzo vacio no tiene plantilla que leer.
        assert!(secciones_de_la_plantilla(&Escena::nueva(), &[]).is_err());
    }

    /// 24-oct-2026 (banco con Haiku): «lo de los habitos no lo uso, pon un
    /// espacio en blanco grande». Sus «01–04» son las prioridades, no filas
    /// que alargar: personalizar el semanal no debe pedir numeros.
    #[test]
    fn el_semanal_cambia_habitos_por_un_espacio_libre() {
        use pixpin_motor2d::figuras_de_serie::{SeccionPedida, TipoSeccion};
        let mut escena = Escena::nueva();
        let f = serie::catalogo()
            .into_iter()
            .find(|f| f.id == "serie-planificador-semanal")
            .unwrap();
        biblioteca::estampar_en_escena(
            &mut escena,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(300.0, 400.0),
        );
        let titulo = |t: &str| SeccionPedida {
            titulo: t.into(),
            ..Default::default()
        };
        let iv = Intervalo {
            secciones: Some(vec![
                titulo("Objetivo de la semana"),
                titulo("Prioridades"),
                titulo("Otras tareas"),
                SeccionPedida {
                    titulo: "Espacio libre".into(),
                    tipo: Some(TipoSeccion::Libre),
                    lineas: Some(8),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };
        let r = poner_plantilla(&mut escena, &[], None, &iv, None).unwrap();
        let textos: Vec<String> = escena
            .visibles()
            .filter_map(|e| match &e.figura {
                pixpin_motor2d::elemento::Figura::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect();
        assert!(
            textos.iter().any(|t| t == "ESPACIO LIBRE") && !textos.iter().any(|t| t == "HÁBITOS"),
            "{}: {textos:?}",
            r.texto
        );
        // Caso negativo: con numeros de verdad (el cronograma) si se conservan.
        assert_eq!(
            serie::cuenta_de("cronograma"),
            Some("filas"),
            "el cronograma sigue contando sus filas"
        );
        assert_eq!(serie::cuenta_de("planificador-semanal"), None);
    }

    /// 25-oct-2026: el cronograma abierto, SIN elegirlo, y «que sean 30
    /// semanas y 30 actividades»: se rehace en su sitio con su diseño.
    #[test]
    fn el_cronograma_sin_elegir_se_agranda_en_su_sitio() {
        let mut escena = Escena::nueva();
        let f = serie::catalogo()
            .into_iter()
            .find(|f| f.id == "serie-cronograma")
            .unwrap();
        let ids = biblioteca::estampar_en_escena(
            &mut escena,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(500.0, 600.0),
        );
        let caja_vieja = caja(&escena.visibles().cloned().collect::<Vec<_>>()).unwrap();
        let iv = Intervalo {
            desde: Some(1),
            hasta: Some(30),
            semanas: Some((1, 30)),
            ..Default::default()
        };
        let r = poner_plantilla(&mut escena, &[], Some("cronograma"), &iv, None).unwrap();
        assert!(r.texto.contains("una hoja"), "{}", r.texto);
        assert!(
            ids.iter()
                .all(|id| escena.buscar(*id).is_some_and(|e| e.borrado)),
            "la vieja se va: no quedan dos cronogramas"
        );
        let nuevas: Vec<Elemento> = escena.visibles().cloned().collect();
        let c = caja(&nuevas).unwrap();
        assert!(
            (c.0 - caja_vieja.0).abs() < 0.5 && (c.1 - caja_vieja.1).abs() < 0.5,
            "empieza donde estaba"
        );
        assert!(
            c.2 - c.0 > caja_vieja.2 - caja_vieja.0 && c.3 - c.1 > caja_vieja.3 - caja_vieja.1,
            "mas ancha (semanas) y mas alta (actividades)"
        );
        // Caso negativo: con dos cronogramas en el lienzo no se adivina.
        let mut dos = Escena::nueva();
        for x in [0.0, 3000.0] {
            biblioteca::estampar_en_escena(
                &mut dos,
                &f,
                pixpin_motor2d::vector::Punto2::nuevo(x, 0.0),
            );
        }
        // «Redúcelo a 10 semanas» deja las 30 actividades que tenia.
        let mut e2 = Escena::nueva();
        biblioteca::estampar_en_escena(
            &mut e2,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(0.0, 0.0),
        );
        let iv30 = Intervalo {
            desde: Some(1),
            hasta: Some(30),
            semanas: Some((1, 30)),
            ..Default::default()
        };
        poner_plantilla(&mut e2, &[], Some("cronograma"), &iv30, None).unwrap();
        let ancho_30 = caja(&e2.visibles().cloned().collect::<Vec<_>>())
            .map(|c| c.2 - c.0)
            .unwrap();
        let solo_semanas = Intervalo {
            semanas: Some((1, 10)),
            ..Default::default()
        };
        poner_plantilla(&mut e2, &[], None, &solo_semanas, None).unwrap();
        let vivos: Vec<Elemento> = e2.visibles().cloned().collect();
        let actual = tramo_actual(&vivos);
        assert_eq!(actual.filas, Some((1, 30)), "las actividades se quedan");
        assert_eq!(actual.semanas, Some((1, 10)));
        assert!(!actual.noche);
        let ancho_10 = caja(&vivos).map(|c| c.2 - c.0).unwrap();
        assert!(
            ancho_10 < ancho_30,
            "la hoja se reajusta, no solo borra columnas"
        );
        assert!(plantilla_en_la_escena(&dos, Some("cronograma"), false).is_none());
        // Desagrupada: se reconoce igual, por su titulo y su hoja.
        let mut suelta = Escena::nueva();
        let ids = biblioteca::estampar_en_escena(
            &mut suelta,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(0.0, 0.0),
        );
        for id in &ids {
            if let Some(x) = suelta.buscar_mut(*id) {
                x.grupos.clear();
            }
        }
        let (c, piezas) = plantilla_en_la_escena(&suelta, None, false).expect("sin grupo tambien");
        assert_eq!(c, "cronograma");
        assert_eq!(piezas.len(), ids.len(), "todas sus piezas y nada mas");
    }

    #[test]
    fn por_su_nombre_va_en_medio_y_sin_nada_elegido_se_dice() {
        let mut escena = Escena::nueva();
        let iv = Intervalo {
            desde: Some(7),
            hasta: Some(18),
            ..Default::default()
        };
        let r = poner_plantilla(
            &mut escena,
            &[],
            Some("storyboard de 6 viñetas"),
            &iv,
            Some((1000.0, 1000.0)),
        )
        .unwrap();
        let c = caja(&escena.visibles().cloned().collect::<Vec<_>>()).unwrap();
        assert!(((c.0 + c.2) / 2.0 - 1000.0).abs() < 1.0, "{c:?}");
        assert!(!r.texto.contains("Ctrl+Z"));
        let e = poner_plantilla(&mut Escena::nueva(), &[], None, &iv, None).unwrap_err();
        assert!(e.contains("elige"), "{e}");
        let (mut escena, ids) = con_la_lista();
        let e = poner_plantilla(&mut escena, &ids, Some("curso"), &iv, None).unwrap_err();
        assert!(e.contains("no lleva números"), "{e}");
    }

    /// 25-oct-2026: «ponlo en modo noche» la rehace en su sitio en oscuro;
    /// alargarla despues sigue de noche, el mes no se pierde y «de día» la
    /// devuelve.
    #[test]
    fn el_modo_noche_se_pide_y_se_queda() {
        let vivos = |e: &Escena| e.visibles().cloned().collect::<Vec<_>>();
        let solo = |noche| Intervalo {
            noche: Some(noche),
            ..Default::default()
        };
        // El cronograma de 30 actividades, en el lienzo sin elegir.
        let mut escena = Escena::nueva();
        let iv30 = Intervalo {
            desde: Some(1),
            hasta: Some(30),
            ..Default::default()
        };
        poner_plantilla(&mut escena, &[], Some("cronograma"), &iv30, None).unwrap();
        let antes = caja(&vivos(&escena)).unwrap();
        let r = poner_plantilla(&mut escena, &[], None, &solo(true), None).unwrap();
        assert!(r.texto.contains("modo noche"), "{}", r.texto);
        let ahora = tramo_actual(&vivos(&escena));
        assert!(ahora.noche);
        assert_eq!(ahora.filas, Some((1, 30)), "no pierde filas");
        let despues = caja(&vivos(&escena)).unwrap();
        assert!(
            [
                despues.0 - antes.0,
                despues.1 - antes.1,
                despues.2 - antes.2,
                despues.3 - antes.3
            ]
            .iter()
            .all(|d| d.abs() < 0.5),
            "en su sitio, igual de grande: {antes:?} {despues:?}"
        );
        // Alargarla sin decir el modo: sigue de noche.
        let iv40 = Intervalo {
            hasta: Some(40),
            ..Default::default()
        };
        poner_plantilla(&mut escena, &[], None, &iv40, None).unwrap();
        let ahora = tramo_actual(&vivos(&escena));
        assert!(ahora.noche && ahora.filas == Some((1, 40)), "{ahora:?}");
        // De dia otra vez.
        poner_plantilla(&mut escena, &[], None, &solo(false), None).unwrap();
        assert!(!tramo_actual(&vivos(&escena)).noche);
        // El mensual de noviembre: de noche sigue siendo noviembre.
        let mut m = Escena::nueva();
        let nov = Intervalo {
            mes: Some((2026, 11)),
            ..Default::default()
        };
        poner_plantilla(&mut m, &[], Some("planificador mensual"), &nov, None).unwrap();
        poner_plantilla(&mut m, &[], None, &solo(true), None).unwrap();
        let ahora = tramo_actual(&vivos(&m));
        assert!(ahora.noche);
        assert_eq!(ahora.mes, Some((2026, 11)));
        // Una que no se alarga ni se personaliza (la gratitud) tambien.
        let mut g = Escena::nueva();
        poner_plantilla(&mut g, &[], Some("gratitud"), &solo(false), None).unwrap();
        poner_plantilla(&mut g, &[], None, &solo(true), None).unwrap();
        let v = vivos(&g);
        assert!(serie::es_de_noche(&v));
        assert_eq!(serie::cual_es(&v).as_deref(), Some("gratitud"));
        // Pedida por su nombre de noche, sale de noche.
        let mut n = Escena::nueva();
        poner_plantilla(&mut n, &[], Some("cronograma (noche)"), &iv30, None).unwrap();
        assert!(tramo_actual(&vivos(&n)).noche);
        // Caso negativo: sin plantilla en el lienzo, el modo solo no adivina.
        let e = poner_plantilla(&mut Escena::nueva(), &[], None, &solo(true), None).unwrap_err();
        assert!(e.contains("elige"), "{e}");
    }
}

