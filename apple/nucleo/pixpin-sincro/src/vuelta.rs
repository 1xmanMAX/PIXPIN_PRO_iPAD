//! Una vuelta con otro aparato, sin pantalla.
//!
//! Lo que hace `SincronizarActivity.unaVuelta` entre que se conecta y que se
//! despide: resolver los proyectos borrados, elegir (o no) que chats, y para
//! cada uno los dos pasos de la [Sesion] contando lo que va pasando. La
//! pantalla de cada aparato solo pone la cara: lo que dice y como se decide
//! es esto, igual en el movil y en el PC.

use std::collections::{BTreeSet, HashMap};
use std::io::{Read, Write};

use crate::disco::Disco;
use crate::mensajes::{Aparato, Chat, LapidaDeChat};
use crate::mezcla::Mando;
use crate::protocolo::{Hecho, Resultado, Sesion};

/// Un chat en la lista de elegir: donde esta y cuando se toco en cada lado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fila {
    pub id: String,
    pub nombre: String,
    pub aqui: Option<Chat>,
    pub alli: Option<Chat>,
    pub marcado: bool,
    /// Borrado en el otro y aqui sin tocar desde entonces: si se deja
    /// marcado, la vuelta lo borra AQUI (con copia y a la papelera).
    pub se_borra_aqui: bool,
}

impl Fila {
    /// Lo que se ensena debajo del nombre en la pestana de un lado
    /// (`esMio`): si esta en el otro y cual es mas reciente.
    pub fn estado(&self, es_mio: bool, otro: &str) -> (String, bool) {
        let (suyo, del_otro) = if es_mio {
            (&self.aqui, &self.alli)
        } else {
            (&self.alli, &self.aqui)
        };
        let t = suyo.as_ref().map_or(0, |c| c.tocado);
        match del_otro {
            None => (format!("Solo aquí · se copiará a {otro}"), false),
            Some(o) if t > o.tocado => ("Más reciente".into(), true),
            Some(o) if t < o.tocado => ("Anterior".into(), false),
            Some(_) => ("Igual en los dos".into(), false),
        }
    }
}

/// Lo que se ensena mientras trabaja (`Fase.Trabajando`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trabajo {
    pub texto: String,
    pub hechos: u64,
    pub total: u64,
    pub velocidad: String,
}

impl Trabajo {
    pub fn solo(texto: String) -> Trabajo {
        Trabajo {
            texto,
            ..Default::default()
        }
    }
}

/// Lo que salio de una vuelta (`Vuelta`).
#[derive(Debug, Clone, Default)]
pub struct Vuelta {
    pub nombre: String,
    pub bytes: u64,
    pub segundos: f64,
    pub avisos: Vec<String>,
    pub cancelada: bool,
}

/// Lo que se le pregunta al usuario en «¿Que sincronizar?».
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pregunta {
    pub filas: Vec<Fila>,
    /// Los nombres de lo que se borraria en el otro porque se borro aqui
    /// (sin casilla: se borro aqui a proposito, y la lista lo avisa en rojo).
    pub se_borran_alli: Vec<String>,
    /// Como empieza el interruptor «Juntar / Lo mio manda».
    pub lo_mio_manda: bool,
}

/// Lo que contesta el usuario: que chats y si manda lo de aqui.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Eleccion {
    pub elegidos: BTreeSet<String>,
    pub lo_mio_manda: bool,
}

/// Que chats: con `Some`, se pregunta (la pantalla de «¿Que sincronizar?»);
/// devuelve None si se cancela. Sin ella se va con lo elegido la ultima vez
/// con ese aparato, o con todo la primera vez, y **no se borra ningun
/// proyecto** (ver [decidir_borrados]).
pub type Elegir<'a> = &'a mut dyn FnMut(&Aparato, Pregunta) -> Option<Eleccion>;

/// Lo que hay que hacer con los proyectos borrados, **sin hacerlo todavia**
/// (`Borrados` de `SincronizarActivity.kt`, v0.79).
///
/// Un proyecto borrado en un aparato tiene lapida y el otro no lo sabe: si
/// no se mira esto primero, la vuelta lo devuelve entero. Con la lapida hay
/// tres casos, y el tercero es el que evita perder trabajo:
///
/// - Borrado aqui y alli **no se ha tocado desde entonces** → `alla`.
/// - Borrado alli y aqui sin tocar desde entonces → `aqui`.
/// - Borrado aqui pero **tocado alli despues** → `levantar`: vuelve.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanDeBorrados {
    pub alla: Vec<String>,
    pub aqui: Vec<String>,
    pub levantar: Vec<String>,
    /// Borrados en los dos, o aqui y que el otro ya no tiene: no hay nada
    /// que sincronizar.
    pub ya_fuera: BTreeSet<String>,
    /// Todos los que tienen lapida aqui y el otro conserva.
    pub mios_con_lapida: Vec<String>,
}

impl PlanDeBorrados {
    /// Si el chat sale en la lista de elegir. Lo borrado aqui no sale (salvo
    /// lo que vuelve), porque sincronizarlo lo resucitaria por sus mensajes.
    pub fn es_candidato(&self, id: &str) -> bool {
        self.levantar.iter().any(|c| c == id)
            || !(self.ya_fuera.contains(id) || self.mios_con_lapida.iter().any(|c| c == id))
    }
}

/// De cada chat, la ultima lapida, en el sitio de la primera
/// (`associateBy`): un proyecto borrado, recuperado y vuelto a borrar tiene
/// dos lineas y manda la mas nueva.
fn ultima_de_cada(v: &[LapidaDeChat]) -> Vec<LapidaDeChat> {
    let mut salida: Vec<LapidaDeChat> = Vec::new();
    for l in v {
        match salida.iter_mut().find(|x| x.chat == l.chat) {
            Some(x) => *x = l.clone(),
            None => salida.push(l.clone()),
        }
    }
    salida
}

/// **Primero se decide, luego se pregunta y solo entonces se borra**
/// (`planDeBorrados`, v0.79). Antes se borraba aqui mismo, antes de ensenar
/// la lista: un usuario vacio su portatil pensando que el telefono lo
/// volveria a llenar, abrio sincronizar y se le borro todo el telefono sin
/// que nadie le preguntara. Esto solo hace la cuenta; no toca nada.
///
/// El reloj del otro puede ir desviado: sus horas se pasan a la de aqui
/// restando `desfase` antes de comparar.
pub fn plan_de_borrados(
    mis: &[LapidaDeChat],
    sus: &[LapidaDeChat],
    mios: &HashMap<String, Chat>,
    suyos: &HashMap<String, Chat>,
    desfase: i64,
) -> PlanDeBorrados {
    let mis = ultima_de_cada(mis);
    let sus = ultima_de_cada(sus);
    let mut plan = PlanDeBorrados::default();
    for l in &mis {
        let Some(suyo) = suyos.get(&l.chat) else {
            plan.ya_fuera.insert(l.chat.clone());
            continue;
        };
        plan.mios_con_lapida.push(l.chat.clone());
        if suyo.tocado - desfase > l.cuando {
            plan.levantar.push(l.chat.clone());
        } else {
            plan.alla.push(l.chat.clone());
        }
    }
    for l in &sus {
        if mis.iter().any(|m| m.chat == l.chat) {
            plan.ya_fuera.insert(l.chat.clone());
            continue;
        }
        let Some(mio) = mios.get(&l.chat) else {
            continue;
        };
        if mio.tocado > l.cuando - desfase {
            continue;
        }
        plan.aqui.push(l.chat.clone());
    }
    plan
}

/// Lo que se hace de verdad con el plan, segun lo contestado.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Decision {
    pub borrar_alla: Vec<String>,
    pub borrar_aqui: Vec<String>,
    /// Lapidas de aqui que se quitan: el otro lo toco despues de borrarlo.
    pub levantar: Vec<String>,
    /// Se iba a borrar aqui y no se borra: ni se toca ni se sincroniza en
    /// esta vuelta (sincronizarlo borraria sus mensajes uno a uno).
    pub retenidos_aqui: Vec<String>,
    /// Se iba a borrar en el otro y no se borra en esta vuelta.
    pub retenidos_alla: Vec<String>,
    /// Los que NO se sincronizan: borrados, o pendientes de decidir.
    pub fuera: BTreeSet<String>,
}

/// **Decide que se borra**, sin tocar nada (`ejecutarBorrados` sin el disco).
///
/// - `elegidos = None` es que nadie ha preguntado («Sincronizar con todos»):
///   **no se borra ningun proyecto en ningun lado**. El movil si lo hace en
///   su ronda; el PC no, porque un borrado sin pantalla delante es justo lo
///   que vacio el telefono del usuario. Lo pendiente se queda fuera de la
///   vuelta y se avisa: se decide sincronizando con ese aparato solo.
/// - Preguntando y juntando: lo borrado alli se borra aqui **solo si esta
///   marcado** en la lista; desmarcado no se borra ni se sincroniza.
/// - Preguntando con «Lo mio manda»: aqui no se borra nada (lo borrado alli
///   se le vuelve a mandar entero) y lo borrado aqui se borra alli aunque
///   alli lo tocaran despues.
pub fn decidir_borrados(
    plan: &PlanDeBorrados,
    elegidos: Option<&BTreeSet<String>>,
    lo_mio_manda: bool,
) -> Decision {
    let mut d = Decision {
        fuera: plan.ya_fuera.clone(),
        ..Default::default()
    };
    match (elegidos, lo_mio_manda) {
        (None, false) => {
            // Volver es no perder nada: lo tocado alli despues de borrarlo
            // aqui se levanta tambien sin preguntar.
            d.levantar = plan.levantar.clone();
            d.retenidos_alla = plan.alla.clone();
            d.retenidos_aqui = plan.aqui.clone();
            d.fuera.extend(plan.alla.iter().cloned());
            d.fuera.extend(plan.aqui.iter().cloned());
        }
        (None, true) => {
            // Con lo mio manda no se levanta nada (manda lo de aqui, y aqui
            // esta borrado) ni se borra alli sin preguntar: lo borrado aqui
            // se queda entero fuera de la vuelta. Lo borrado alli y vivo aqui
            // si viaja: se le vuelve a mandar, y eso aqui no borra nada.
            d.retenidos_alla = plan.mios_con_lapida.clone();
            d.fuera.extend(plan.mios_con_lapida.iter().cloned());
        }
        (Some(e), false) => {
            d.levantar = plan.levantar.clone();
            d.borrar_alla = plan.alla.clone();
            for c in &plan.aqui {
                if e.contains(c) {
                    d.borrar_aqui.push(c.clone());
                } else {
                    d.retenidos_aqui.push(c.clone());
                }
            }
            d.fuera.extend(plan.alla.iter().cloned());
            d.fuera.extend(plan.aqui.iter().cloned());
        }
        (Some(_), true) => {
            d.borrar_alla = plan.mios_con_lapida.clone();
            d.fuera.extend(plan.mios_con_lapida.iter().cloned());
        }
    }
    d
}

/// Las filas marcadas que se borrarian AQUI al pulsar «Sincronizar»: lo que
/// la lista tiene que decir en rojo antes. Con «Lo mio manda», ninguna.
pub fn se_borrarian_aqui<'f>(
    filas: &'f [Fila],
    marcados: &[bool],
    lo_mio_manda: bool,
) -> Vec<&'f Fila> {
    if lo_mio_manda {
        return Vec::new();
    }
    filas
        .iter()
        .zip(marcados)
        .filter(|(f, m)| f.se_borra_aqui && **m)
        .map(|(f, _)| f)
        .collect()
}

/// Los avisos de lo que se quedo sin decidir, para leerlos al final.
pub fn avisos_de_borrados(
    d: &Decision,
    nombre: &dyn Fn(&str) -> String,
    otro: &str,
) -> Vec<String> {
    let lista = |v: &[String]| {
        v.iter()
            .map(|c| format!("«{}»", nombre(c)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut v = Vec::new();
    if !d.retenidos_aqui.is_empty() {
        v.push(format!(
            "No se ha borrado {} aquí aunque se borró en {otro}: sigue aquí entero y no se ha sincronizado. Sincroniza solo con {otro} para decidirlo.",
            lista(&d.retenidos_aqui)
        ));
    }
    if !d.retenidos_alla.is_empty() {
        v.push(format!(
            "{} se borró aquí y sigue en {otro}: no se ha borrado allí sin preguntar. Sincroniza solo con {otro} para decidirlo.",
            lista(&d.retenidos_alla)
        ));
    }
    v
}

/// Hace lo decidido: primero levanta, luego borra alli y despues aqui (con
/// copia y a la papelera, que es lo que hace `borrar_chat`).
fn ejecutar_borrados<D: Disco + ?Sized, F: Read + Write>(
    s: &mut Sesion<'_, D, F>,
    d: &Decision,
    mios: &HashMap<String, Chat>,
    suyos: &HashMap<String, Chat>,
    ahora: i64,
    progreso: &mut dyn FnMut(Trabajo),
) -> Resultado<()> {
    let disco = s.disco();
    for c in &d.levantar {
        disco.quitar_lapida(c)?;
    }
    for c in &d.borrar_alla {
        let nombre = suyos.get(c).map_or(c.as_str(), |x| x.nombre.as_str());
        progreso(Trabajo::solo(format!(
            "Borrando «{nombre}» en {}…",
            s.otro.nombre
        )));
        s.borrar_alla(c)?;
    }
    for c in &d.borrar_aqui {
        let nombre = mios.get(c).map_or(c.as_str(), |x| x.nombre.as_str());
        progreso(Trabajo::solo(format!(
            "Borrando «{nombre}», borrado en {}…",
            s.otro.nombre
        )));
        disco.borrar_chat(
            c,
            &format!("Antes de borrarlo, borrado en {}", s.otro.nombre),
            ahora,
            &s.otro.id,
        )?;
    }
    Ok(())
}

/// **Una vuelta con un aparato ya conectado.** `rotulo` va delante de lo que
/// se ensena mientras trabaja, para saber por donde va una ronda.
///
/// Con `elegir` se pregunta que chats y si manda lo de aqui; sin el, manda
/// lo que ya tenga puesto la sesion ([Sesion::quien_manda]).
pub fn una<D: Disco + ?Sized, F: Read + Write>(
    s: &mut Sesion<'_, D, F>,
    elegir: Option<Elegir<'_>>,
    hecho: &mut Hecho,
    rotulo: &str,
    ahora: &dyn Fn() -> i64,
    progreso: &mut dyn FnMut(Trabajo),
) -> Resultado<Vuelta> {
    con_alcance(s, elegir, None, hecho, rotulo, ahora, progreso)
}

/// **Solo estos chats, sin preguntar**: la vuelta de «tareas al instante»
/// (10-oct-2026, el usuario: «sincronizar las tareas de forma rapida a
/// todos los dispositivos»). Es la de «con todos» ([una] sin `elegir`)
/// recortada a `chats` (ids de chat como viajan, no fichas del PC):
///
/// - De `chats` solo entra lo que entraria en la vuelta entera: lo elegido
///   la ultima vez con ese aparato (o todo la primera) y nada con lapida
///   pendiente. Un chat desmarcado con ese aparato no se cuela por aqui.
/// - **No se borra ningun proyecto** (como en toda vuelta sin preguntar) y
///   solo se levantan las lapidas de `chats`.
/// - **Sin la galeria de capturas**: es lo que mas tarda y no tiene que ver.
///
/// En el cable no cambia nada: quien responde atiende peticion a peticion
/// (`catalogo`, `lapidas`, y por chat `inventario`… `base`) y nunca sabe ni
/// pregunta cuantos chats se van a juntar; en el movil igual
/// (`Respondedor.responder` de `Protocolo.kt`).
pub fn solo_estos<D: Disco + ?Sized, F: Read + Write>(
    s: &mut Sesion<'_, D, F>,
    chats: &BTreeSet<String>,
    hecho: &mut Hecho,
    rotulo: &str,
    ahora: &dyn Fn() -> i64,
    progreso: &mut dyn FnMut(Trabajo),
) -> Resultado<Vuelta> {
    con_alcance(s, None, Some(chats), hecho, rotulo, ahora, progreso)
}

/// Lo de [una] y [solo_estos]: con `solo`, la vuelta se recorta a esos
/// chats (y entonces nunca se pregunta).
fn con_alcance<D: Disco + ?Sized, F: Read + Write>(
    s: &mut Sesion<'_, D, F>,
    elegir: Option<Elegir<'_>>,
    solo: Option<&BTreeSet<String>>,
    hecho: &mut Hecho,
    rotulo: &str,
    ahora: &dyn Fn() -> i64,
    progreso: &mut dyn FnMut(Trabajo),
) -> Resultado<Vuelta> {
    let elegir = if solo.is_some() { None } else { elegir };
    let disco = s.disco();
    let otro = s.otro.clone();
    let lista_mia = disco.chats()?;
    let orden_mio: Vec<String> = lista_mia.iter().map(|c| c.id.clone()).collect();
    let mios: HashMap<String, Chat> = lista_mia.into_iter().map(|c| (c.id.clone(), c)).collect();
    let catalogo = s.catalogo()?;
    let suyos: HashMap<String, Chat> = catalogo.iter().map(|c| (c.id.clone(), c.clone())).collect();
    let antes = disco.elegidos(&otro.id);
    let sus_lapidas = s.lapidas()?;
    let plan = plan_de_borrados(&disco.lapidas(), &sus_lapidas, &mios, &suyos, s.desfase);
    let mut ids: Vec<String> = Vec::new();
    for id in orden_mio.iter().chain(catalogo.iter().map(|c| &c.id)) {
        if !ids.contains(id) && plan.es_candidato(id) {
            ids.push(id.clone());
        }
    }
    let filas: Vec<Fila> = ids
        .iter()
        .map(|id| {
            let m = mios.get(id).cloned();
            let su = suyos.get(id).cloned();
            let se_borra_aqui = plan.aqui.contains(id);
            Fila {
                id: id.clone(),
                nombre: m
                    .as_ref()
                    .or(su.as_ref())
                    .map(|c| c.nombre.clone())
                    .unwrap_or_default(),
                aqui: m,
                alli: su,
                // Lo que se borraria aqui empieza SIN marcar (el movil lo
                // marca como lo de la ultima vez): un clic de mas en
                // «Sincronizar» no puede borrar un proyecto entero.
                marcado: !se_borra_aqui && antes.as_ref().is_none_or(|a| a.contains(id)),
                se_borra_aqui,
            }
        })
        .collect();
    let (elegidos, lo_mio_manda, preguntado) = match elegir {
        Some(e) => {
            let pregunta = Pregunta {
                filas: filas.clone(),
                se_borran_alli: plan
                    .alla
                    .iter()
                    .map(|c| suyos.get(c).map_or(c.clone(), |x| x.nombre.clone()))
                    .collect(),
                lo_mio_manda: s.mando() == Mando::LoMioManda,
            };
            match e(&otro, pregunta) {
                None => {
                    s.adios();
                    return Ok(Vuelta {
                        nombre: otro.nombre,
                        cancelada: true,
                        ..Default::default()
                    });
                }
                Some(puesto) => {
                    // Lo mio manda es cosa de una vez: lo marcado entonces
                    // no se recuerda como lo de siempre.
                    if !puesto.lo_mio_manda {
                        disco.guardar_elegidos(&otro.id, &puesto.elegidos)?;
                    }
                    (puesto.elegidos, puesto.lo_mio_manda, true)
                }
            }
        }
        None => (
            antes.unwrap_or_else(|| ids.iter().cloned().collect()),
            s.mando() == Mando::LoMioManda,
            false,
        ),
    };
    s.quien_manda(if lo_mio_manda {
        Mando::LoMioManda
    } else {
        Mando::Acordar
    });
    let mut decision = decidir_borrados(&plan, preguntado.then_some(&elegidos), lo_mio_manda);
    if let Some(solo) = solo {
        // Recortada, solo se toca lo suyo: lo demas lo decide la vuelta
        // entera. Sin preguntar no se borra nada, asi que basta con esto.
        decision.levantar.retain(|c| solo.contains(c));
    }
    ejecutar_borrados(s, &decision, &mios, &suyos, ahora(), progreso)?;
    let nombre_de = |c: &str| {
        mios.get(c)
            .or(suyos.get(c))
            .map_or(c.to_string(), |x| x.nombre.clone())
    };
    // Recortada, los avisos de lo pendiente de borrar son de la entera.
    let mut avisos = if solo.is_some() {
        Vec::new()
    } else {
        avisos_de_borrados(&decision, &nombre_de, &otro.nombre)
    };

    let empezo = std::time::Instant::now();
    for chat in ids
        .iter()
        .filter(|c| elegidos.contains(*c) && !decision.fuera.contains(*c))
        .filter(|c| solo.is_none_or(|x| x.contains(*c)))
    {
        let nombre = filas
            .iter()
            .find(|f| f.id == *chat)
            .map(|f| f.nombre.clone())
            .unwrap_or_default();
        progreso(Trabajo::solo(format!(
            "{rotulo}Comparando «{nombre}» con {}…",
            otro.nombre
        )));
        let prep = s.preparar(chat)?;
        progreso(Trabajo::solo(format!("{rotulo}Juntando «{nombre}»…")));
        s.aplicar(&prep, hecho)?;
        let archivos = s.preparar_archivos(&prep)?;
        let total = archivos.bytes();
        let mut hechos = 0u64;
        let t0 = std::time::Instant::now();
        let mut ultimo = std::time::Instant::now() - std::time::Duration::from_secs(1);
        s.aplicar_archivos(&archivos, hecho, &mut |n| {
            hechos += n;
            if ultimo.elapsed().as_millis() > 150 {
                ultimo = std::time::Instant::now();
                let seg = t0.elapsed().as_secs_f64().max(0.001);
                progreso(Trabajo {
                    texto: format!("{rotulo}Pasando archivos de «{nombre}»"),
                    hechos,
                    total,
                    velocidad: format!("{}/s", tamano_legible((hechos as f64 / seg) as i64)),
                });
            }
        })?;
        progreso(Trabajo::solo(format!("{rotulo}Terminando «{nombre}»…")));
        s.cerrar(&prep, ahora())?;
    }
    // **La galeria de capturas**, con sus fechas de irse (Android v0.107.0).
    // Si el otro no sabe de galerias, no pasa nada.
    if solo.is_none() && s.tiene_galeria() {
        progreso(Trabajo::solo(format!(
            "{rotulo}Juntando la galería con {}…",
            otro.nombre
        )));
        let mut hechos = 0u64;
        let t0 = std::time::Instant::now();
        let mut ultimo = std::time::Instant::now() - std::time::Duration::from_secs(1);
        s.galeria(hecho, ahora(), &mut |n| {
            hechos += n;
            if ultimo.elapsed().as_millis() > 150 {
                ultimo = std::time::Instant::now();
                let seg = t0.elapsed().as_secs_f64().max(0.001);
                progreso(Trabajo::solo(format!(
                    "{rotulo}Pasando capturas de la galería: {} · {}/s",
                    tamano_legible(hechos as i64),
                    tamano_legible((hechos as f64 / seg) as i64)
                )));
            }
        })?;
    }
    // **Las listas de los chats compartidos** (Android 13-oct-2026): mi otro
    // aparato entra en los que comparto con otras personas. Un aparato de
    // antes contesta «No sé» y no pasa nada.
    if solo.is_none() {
        s.compartidos_del_grupo(ahora())?;
    }
    s.adios();
    if !hecho.saltados.is_empty() {
        let mut vistos = Vec::new();
        for n in &hecho.saltados {
            if !vistos.contains(n) {
                vistos.push(n.clone());
            }
        }
        let lista: Vec<String> = vistos.iter().map(|n| format!("«{n}»")).collect();
        avisos.push(format!(
            "No se pasó {} porque se estaba guardando en ese momento. Vuelve a sincronizar.",
            lista.join(", ")
        ));
    }
    if let Some(a) = aviso_del_reloj(&otro, s.desfase) {
        avisos.push(a);
    }
    Ok(Vuelta {
        nombre: otro.nombre,
        bytes: s.enviados() + s.recibidos(),
        segundos: empezo.elapsed().as_secs_f64(),
        avisos,
        cancelada: false,
    })
}

/// `avisoDelReloj`: con la hora mal, lo que llega se coloca en el chat donde
/// no toca.
pub fn aviso_del_reloj(otro: &Aparato, desfase: i64) -> Option<String> {
    let minutos = desfase.abs() / 60_000;
    if minutos < 2 {
        return None;
    }
    let sentido = if desfase > 0 {
        "adelantado"
    } else {
        "atrasado"
    };
    Some(format!(
        "El reloj de {} va {minutos} min {sentido}. Pon «fecha y hora automáticas» en los dos: con la hora mal, lo que llega se coloca en el chat donde no toca.",
        otro.nombre
    ))
}

/// `avisosDeLoHecho`: lo rescatado y lo que no hizo falta mandar.
pub fn avisos_de_lo_hecho(h: &Hecho) -> Vec<String> {
    let mut v = Vec::new();
    if h.rescatados > 0 {
        v.push(if h.rescatados == 1 {
            "1 cosa borrada en un aparato se quedó porque en el otro se había cambiado después."
                .to_string()
        } else {
            format!(
                "{} cosas borradas en un aparato se quedaron porque en el otro se había cambiado después.",
                h.rescatados
            )
        });
    }
    if h.ahorrados > 64_000 {
        v.push(format!(
            "Solo viajaron los cambios: {} que no hizo falta mandar.",
            tamano_legible(h.ahorrados)
        ));
    }
    v
}

/// `contarLoHecho`: lo que se le cuenta al usuario al acabar.
pub fn contar_lo_hecho(h: &Hecho, bytes: u64, segundos: f64) -> String {
    let mut partes = Vec::new();
    if h.traidos > 0 {
        partes.push(format!("{} traídos", h.traidos));
    }
    if h.enviados > 0 {
        partes.push(format!("{} enviados", h.enviados));
    }
    if h.borrados > 0 {
        partes.push(format!("{} borrados", h.borrados));
    }
    if h.archivos > 0 {
        partes.push(format!("{} archivos", h.archivos));
    }
    if h.fusionados > 0 {
        partes.push(format!("{} juntados de los dos", h.fusionados));
    }
    // La galeria, como `contarLoHecho` desde la v0.107.0 del movil.
    if h.capturas > 0 {
        partes.push(format!(
            "{} {} de la galería",
            h.capturas,
            if h.capturas == 1 {
                "captura"
            } else {
                "capturas"
            }
        ));
    }
    if h.capturas_tiradas > 0 {
        partes.push(format!("{} quitadas de la galería", h.capturas_tiradas));
    }
    let primera = if partes.is_empty() {
        "Ya estaban iguales.".to_string()
    } else {
        format!("{}.", partes.join(" · "))
    };
    let tam = tamano_legible(bytes as i64);
    format!(
        "{primera}\n{} en {} s.",
        if tam.is_empty() { "0 B".into() } else { tam },
        decimal(segundos)
    )
}

/// `tamanoLegible`: en kilobytes de mil, con coma decimal como el movil en
/// espanol.
pub fn tamano_legible(bytes: i64) -> String {
    match bytes {
        b if b <= 0 => String::new(),
        b if b < 1_000 => format!("{b} B"),
        b if b < 1_000_000 => format!("{} kB", b / 1_000),
        b if b < 1_000_000_000 => format!("{} MB", decimal(b as f64 / 1_000_000.0)),
        b => format!("{} GB", decimal(b as f64 / 1_000_000_000.0)),
    }
}

fn decimal(x: f64) -> String {
    format!("{x:.1}").replace('.', ",")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_tamano_se_escribe_como_en_el_movil() {
        assert_eq!(tamano_legible(0), "");
        assert_eq!(tamano_legible(999), "999 B");
        assert_eq!(tamano_legible(4_500), "4 kB");
        assert_eq!(tamano_legible(2_300_000), "2,3 MB");
    }

    #[test]
    fn lo_hecho_se_cuenta_como_en_el_movil() {
        let h = Hecho {
            traidos: 2,
            archivos: 1,
            ..Default::default()
        };
        assert_eq!(
            contar_lo_hecho(&h, 2_300_000, 1.25),
            "2 traídos · 1 archivos.\n2,3 MB en 1,2 s."
        );
        assert_eq!(
            contar_lo_hecho(&Hecho::default(), 0, 0.0),
            "Ya estaban iguales.\n0 B en 0,0 s."
        );
    }

    #[test]
    fn cada_fila_dice_cual_es_mas_reciente() {
        let c = |t| Chat {
            id: "p".into(),
            nombre: "P".into(),
            mensajes: 0,
            tocado: t,
        };
        let f = Fila {
            id: "p".into(),
            nombre: "P".into(),
            aqui: Some(c(5)),
            alli: Some(c(3)),
            marcado: true,
            se_borra_aqui: false,
        };
        assert_eq!(f.estado(true, "Móvil"), ("Más reciente".into(), true));
        assert_eq!(f.estado(false, "PC"), ("Anterior".into(), false));
        let solo = Fila { alli: None, ..f };
        assert_eq!(
            solo.estado(true, "Móvil").0,
            "Solo aquí · se copiará a Móvil"
        );
    }

    // ------------------------------------------- borrados: plan y decision

    fn chat(id: &str, tocado: i64) -> (String, Chat) {
        (
            id.to_string(),
            Chat {
                id: id.into(),
                nombre: id.to_uppercase(),
                mensajes: 1,
                tocado,
            },
        )
    }

    fn lapida(chat: &str, cuando: i64) -> LapidaDeChat {
        LapidaDeChat {
            chat: chat.into(),
            cuando,
            aparato: String::new(),
        }
    }

    fn todos(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Los cuatro casos a la vez: `a` borrado aqui y quieto alli, `b`
    /// borrado aqui y tocado alli despues, `c` borrado alli y quieto aqui,
    /// `d` borrado alli pero tocado aqui despues.
    fn plan_de_los_cuatro() -> PlanDeBorrados {
        let mios: HashMap<_, _> = [chat("c", 100), chat("d", 900)].into();
        let suyos: HashMap<_, _> = [chat("a", 100), chat("b", 900)].into();
        plan_de_borrados(
            &[lapida("a", 500), lapida("b", 500)],
            &[lapida("c", 500), lapida("d", 500)],
            &mios,
            &suyos,
            0,
        )
    }

    #[test]
    fn el_plan_separa_lo_que_se_borra_alli_aqui_y_lo_que_vuelve() {
        let p = plan_de_los_cuatro();
        assert_eq!(p.alla, ["a"]);
        assert_eq!(p.levantar, ["b"], "tocado alli despues: vuelve");
        assert_eq!(p.aqui, ["c"]);
        assert!(
            !p.aqui.contains(&"d".to_string()),
            "tocado aqui despues de borrarlo alli: no se borra"
        );
        assert!(p.es_candidato("b") && p.es_candidato("c") && p.es_candidato("d"));
        assert!(!p.es_candidato("a"), "lo borrado aqui no sale en la lista");
    }

    #[test]
    fn el_plan_cuenta_con_el_reloj_desviado_del_otro() {
        // El otro va 1000 ms adelantado: su «tocado 900» es un 900 - 1000 =
        // -100 de aqui, ANTES de la lapida. Sin restar, volveria.
        let suyos: HashMap<_, _> = [chat("b", 900)].into();
        let p = plan_de_borrados(&[lapida("b", 500)], &[], &HashMap::new(), &suyos, 1000);
        assert_eq!(p.alla, ["b"]);
        assert!(p.levantar.is_empty());
    }

    #[test]
    fn borrado_en_los_dos_o_ya_ido_no_se_sincroniza_ni_se_pregunta() {
        let mios: HashMap<_, _> = [chat("x", 100)].into();
        let p = plan_de_borrados(
            &[lapida("x", 500), lapida("y", 500)],
            &[lapida("x", 600)],
            &mios,
            &HashMap::new(),
            0,
        );
        assert_eq!(p.ya_fuera, todos(&["x", "y"]));
        assert!(p.alla.is_empty() && p.aqui.is_empty());
        let d = decidir_borrados(&p, Some(&todos(&["x", "y"])), false);
        assert!(d.borrar_aqui.is_empty() && d.borrar_alla.is_empty());
        assert!(d.fuera.contains("x"));
    }

    #[test]
    fn de_una_lapida_repetida_manda_la_ultima() {
        // Borrado, recuperado, tocado y vuelto a borrar: la vieja no cuenta.
        let suyos: HashMap<_, _> = [chat("a", 700)].into();
        let p = plan_de_borrados(
            &[lapida("a", 500), lapida("a", 800)],
            &[],
            &HashMap::new(),
            &suyos,
            0,
        );
        assert_eq!(p.alla, ["a"], "con la de 800, lo de alli (700) es de antes");
    }

    #[test]
    fn sin_preguntar_no_se_borra_nada_en_ningun_lado() {
        let p = plan_de_los_cuatro();
        let d = decidir_borrados(&p, None, false);
        assert!(d.borrar_aqui.is_empty(), "nada aqui sin preguntar");
        assert!(d.borrar_alla.is_empty(), "ni alli");
        assert_eq!(d.levantar, ["b"], "volver no borra nada: se hace");
        assert_eq!(d.retenidos_aqui, ["c"]);
        assert_eq!(d.retenidos_alla, ["a"]);
        assert!(d.fuera.contains("c"), "y no se sincroniza: lo borraria");
        assert!(!d.fuera.contains("b") && !d.fuera.contains("d"));
    }

    #[test]
    fn sin_preguntar_y_con_lo_mio_manda_tampoco_se_borra_y_lo_mio_viaja() {
        let p = plan_de_los_cuatro();
        let d = decidir_borrados(&p, None, true);
        assert!(d.borrar_aqui.is_empty() && d.borrar_alla.is_empty());
        assert!(d.levantar.is_empty());
        assert!(!d.fuera.contains("c"), "lo mio vuelve alla");
        assert!(d.fuera.contains("a") && d.fuera.contains("b"));
    }

    #[test]
    fn preguntando_solo_se_borra_aqui_lo_marcado() {
        let p = plan_de_los_cuatro();
        let d = decidir_borrados(&p, Some(&todos(&["c", "d"])), false);
        assert_eq!(d.borrar_aqui, ["c"]);
        assert_eq!(d.borrar_alla, ["a"]);
        assert_eq!(d.levantar, ["b"]);
        // Caso negativo: desmarcado no se borra, y tampoco se sincroniza.
        let d = decidir_borrados(&p, Some(&todos(&["d"])), false);
        assert!(d.borrar_aqui.is_empty());
        assert_eq!(d.retenidos_aqui, ["c"]);
        assert!(d.fuera.contains("c"));
    }

    #[test]
    fn preguntando_con_lo_mio_manda_aqui_no_se_borra_nada() {
        let p = plan_de_los_cuatro();
        let d = decidir_borrados(&p, Some(&todos(&["c", "d"])), true);
        assert!(d.borrar_aqui.is_empty(), "aunque este marcado");
        assert_eq!(d.borrar_alla, ["a", "b"], "lo borrado aqui se borra alli");
        assert!(d.levantar.is_empty());
        assert!(!d.fuera.contains("c"), "se le vuelve a mandar");
    }

    #[test]
    fn la_lista_dice_en_rojo_solo_lo_marcado_que_se_borraria_aqui() {
        let fila = |id: &str, se_borra_aqui: bool| Fila {
            id: id.into(),
            nombre: id.into(),
            aqui: None,
            alli: None,
            marcado: false,
            se_borra_aqui,
        };
        let filas = [fila("c", true), fila("x", false), fila("e", true)];
        let nombres = |v: Vec<&Fila>| v.iter().map(|f| f.id.clone()).collect::<Vec<_>>();
        assert_eq!(
            nombres(se_borrarian_aqui(&filas, &[true, true, false], false)),
            ["c"]
        );
        assert!(se_borrarian_aqui(&filas, &[false, true, false], false).is_empty());
        assert!(
            se_borrarian_aqui(&filas, &[true, true, true], true).is_empty(),
            "con lo mio manda no se borra nada aqui"
        );
    }

    #[test]
    fn los_avisos_nombran_lo_pendiente_y_sin_pendientes_no_hay_aviso() {
        let p = plan_de_los_cuatro();
        let d = decidir_borrados(&p, None, false);
        let v = avisos_de_borrados(&d, &|c: &str| c.to_uppercase(), "Móvil");
        assert_eq!(v.len(), 2);
        assert!(v[0].contains("«C»") && v[0].contains("Móvil"), "{v:?}");
        assert!(v[1].contains("«A»"), "{v:?}");
        let d = decidir_borrados(&p, Some(&todos(&["c"])), false);
        assert!(avisos_de_borrados(&d, &|c: &str| c.into(), "Móvil").is_empty());
    }
}
