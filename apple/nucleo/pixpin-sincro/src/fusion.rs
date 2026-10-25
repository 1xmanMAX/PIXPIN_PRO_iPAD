//! Juntar dos versiones de lo mismo sin que ninguna mande (15-sep-2026).
//!
//! Puerto de `sincro/Fusion.kt`. Si en un aparato el lienzo es A+B y en el
//! otro A+C, al sincronizar tiene que quedar A+B+C, no «el del maestro». Es
//! la fusion a tres bandas de git —que cambio cada uno desde lo ultimo que
//! tuvieron en comun (la *base*) y sumar los dos cambios—, pero sobre el
//! JSON de las cosas: un lienzo se junta figura por figura (por su `id`),
//! una tabla celda por celda, una nota parrafo por parrafo y un croquis
//! trazo por trazo.
//!
//! De donde sale cada regla (las mismas que en Android):
//!
//! - Propiedad por propiedad (Figma): mover en un lado y cambiar el color en
//!   el otro deja movida y con el color nuevo. Solo choca la misma propiedad
//!   cambiada en los dos; entonces gana el ultimo.
//! - El ultimo, sin fiarse del reloj (Excalidraw `reconcile.ts`): primero la
//!   hora `updated` corregida con el desfase medido al conectar; a igualdad,
//!   `version` mayor y `versionNonce` menor.
//! - Lo modificado gana a lo borrado (OR-Set, add-wins): borrar quita lo que
//!   el que borro llego a ver; un cambio que no vio sobrevive.
//! - Los parrafos, con diff3: cada lado contra la base, y solo choca un tramo
//!   tocado en los dos.
//! - Solo viajan los cambios ([diferencia] y [aplicar]).
//!
//! Trabaja sobre [Json] y no sobre `serde_json::Value` porque lo que sale
//! de aqui se escribe en disco y se resume: ver `canonico`.

use std::collections::{HashMap, HashSet};

use crate::canonico::{Json, Objeto};

/// Quien gana cuando los dos cambiaron lo mismo.
///
/// `mio_mas_nuevo` decide cuando las cosas no llevan hora propia (el archivo
/// que se toco mas tarde). `desfase` es cuanto va adelantado el reloj del
/// otro: se le resta a sus horas antes de compararlas, o un telefono con la
/// hora adelantada ganaria siempre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Criterio {
    pub mio_mas_nuevo: bool,
    pub desfase: i64,
}

impl Default for Criterio {
    fn default() -> Self {
        Criterio {
            mio_mas_nuevo: true,
            desfase: 0,
        }
    }
}

/// Las claves de texto que se juntan por parrafos y no enteras.
pub const PARRAFOS: [&str; 3] = ["nota", "texto", "transcripcion"];

/// Por encima de esto una nota no se compara linea a linea: gana entera la
/// mas reciente.
const TOPE_DE_COMPARAR: u64 = 4_000_000;

/// Lo que cuenta de una fusion, para decirlo al acabar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cuenta {
    /// La misma cosa cambiada en los dos lados y resuelta por «gana el ultimo».
    pub choques: u32,
    /// Algo borrado en un lado que vuelve porque en el otro se cambio.
    pub rescatados: u32,
}

/// Un parche que no tiene la forma que escribe [diferencia].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("Parche mal formado")]
pub struct ParcheMalFormado;

/// La fusion a tres bandas. `base` es lo ultimo que tuvieron en comun (None
/// si nunca se sincronizo: entonces se junta todo, que perder algo es peor
/// que tener de mas). Devuelve None si el resultado es «no esta».
pub fn json(
    base: Option<&Json>,
    mio: Option<&Json>,
    suyo: Option<&Json>,
    criterio: &Criterio,
    cuenta: &mut Cuenta,
) -> Option<Json> {
    juntar(
        nulo(base),
        nulo(mio),
        nulo(suyo),
        criterio.mio_mas_nuevo,
        None,
        criterio,
        cuenta,
    )
}

/// Un `null` de JSON por arriba cuenta como «no esta».
fn nulo(e: Option<&Json>) -> Option<&Json> {
    match e {
        Some(Json::Nulo) => None,
        otro => otro,
    }
}

fn juntar(
    b: Option<&Json>,
    m: Option<&Json>,
    s: Option<&Json>,
    gana_mio: bool,
    clave: Option<&str>,
    c: &Criterio,
    cuenta: &mut Cuenta,
) -> Option<Json> {
    if m == s {
        return m.cloned();
    }
    if m == b {
        return s.cloned();
    }
    if s == b {
        return m.cloned();
    }
    // **Las marcas y los marcadores de una nota de voz**, uno por
    // milisegundo (25-oct-2026): se juntan por su minuto, a tres bandas. Sin
    // esto, un marcador puesto en el movil y otro en el PC antes de
    // sincronizar eran un choque de la lista entera y uno se perdia.
    if let Some(k) = clave.filter(|k| POR_MILISEGUNDO.contains(k))
        && let Some(l) = por_milisegundo(b, m, s, gana_mio, c, cuenta)
    {
        // `marcadores` sin ninguno no se escribe (`clase.rs`); `marcas` es
        // del esquema de Kotlin y va siempre, aunque sea `[]`.
        return (!l.is_empty() || k == "marcas").then_some(Json::Lista(l));
    }
    // Cambiado en los dos, y distinto.
    let (Some(m), Some(s)) = (m, s) else {
        // Borrado en uno y cambiado en el otro: se queda lo cambiado.
        return m.or(s).cloned();
    };
    if let (Json::Objeto(om), Json::Objeto(os)) = (m, s) {
        let ob = b.and_then(Json::como_objeto);
        return Some(Json::Objeto(objeto(ob, om, os, gana_mio, c, cuenta)));
    }
    if let (Json::Lista(lm), Json::Lista(ls)) = (m, s) {
        let bb = b.and_then(Json::como_lista);
        if con_ids(lm) && con_ids(ls) && bb.is_none_or(con_ids) {
            return Some(Json::Lista(lista_con_ids(bb, lm, ls, gana_mio, c, cuenta)));
        }
        if de_textos(lm) && de_textos(ls) && bb.is_none_or(de_textos) {
            return Some(Json::Lista(conjunto(bb, lm, ls)));
        }
    }
    // **Una tabla del chat se junta celda a celda**, no por parrafos
    // (`Fusion.tablaJunta` de Android, 13-oct-2026; aqui el 15-oct): la
    // mini-app `tabla` y las hojas editadas de un Excel llevan el JSON de la
    // tabla en el `texto` del mensaje. Por parrafos, un cambio en cada
    // aparato era un choque entero (el movil lo escribe en una linea) o, con
    // el JSON del PC (una celda por linea), podia salir un JSON roto.
    if clave == Some("texto")
        && let (Json::Cadena(tm), Json::Cadena(ts)) = (m, s)
        && let Some(junta) = tabla_junta(b, tm, ts, gana_mio, c, cuenta)
    {
        return Some(Json::Cadena(junta));
    }
    // **Una carpeta de paginas se junta pagina a pagina** (25-oct-2026): su
    // documento va en el `texto` de un mensaje `MINIAPP` «carpeta» en una
    // sola linea; por parrafos, anadir una pagina en cada aparato era un
    // choque y una de las dos se perdia.
    if clave == Some("texto")
        && let (Json::Cadena(tm), Json::Cadena(ts)) = (m, s)
        && let Some(junta) = carpeta_junta(b, tm, ts, gana_mio, c, cuenta)
    {
        return Some(Json::Cadena(junta));
    }
    if clave.is_some_and(|k| PARRAFOS.contains(&k)) {
        if let (Json::Cadena(tm), Json::Cadena(ts)) = (m, s) {
            // Una base que no sea texto cuenta como vacia, igual que en
            // Android.
            let bt = b.and_then(Json::como_cadena).unwrap_or("");
            return Some(Json::Cadena(parrafos(bt, tm, ts, gana_mio, cuenta)));
        }
    }
    cuenta.choques += 1;
    Some(if gana_mio { m.clone() } else { s.clone() })
}

/// El JSON de una tabla (`pixpin_proyecto::tabla::Tabla`) en un texto, o
/// `None` si no lo es (`Fusion.comoTabla`).
fn como_tabla(texto: &str) -> Option<Objeto> {
    let t = texto.trim_start();
    if !t.starts_with('{') || !t.contains("\"celdas\"") {
        return None;
    }
    match Json::analizar(texto).ok()? {
        Json::Objeto(o) if matches!(o.obtener("celdas"), Some(Json::Objeto(_))) => Some(o),
        _ => None,
    }
}

/// Dos textos-tabla juntos celda a celda, en JSON compacto
/// (`Fusion.tablaJunta`). `None` si alguno no es una tabla: entonces va por
/// parrafos, como cualquier texto.
fn tabla_junta(
    b: Option<&Json>,
    mio: &str,
    suyo: &str,
    gana_mio: bool,
    c: &Criterio,
    cuenta: &mut Cuenta,
) -> Option<String> {
    let m = como_tabla(mio)?;
    let s = como_tabla(suyo)?;
    let base = b.and_then(Json::como_cadena).and_then(como_tabla);
    Some(Json::Objeto(objeto(base.as_ref(), &m, &s, gana_mio, c, cuenta)).a_texto())
}

/// El documento de una carpeta de paginas
/// (`pixpin_proyecto::carpeta_de_paginas::Carpeta`) en un texto, o `None` si
/// no lo es: un objeto con `formato` y la lista `paginas`, cada una con su
/// `id`.
fn como_carpeta(texto: &str) -> Option<Objeto> {
    let t = texto.trim_start();
    if !t.starts_with('{') || !t.contains("\"paginas\"") || !t.contains("\"formato\"") {
        return None;
    }
    match Json::analizar(texto).ok()? {
        Json::Objeto(o)
            if matches!(o.obtener("formato"), Some(Json::Literal(_)))
                && matches!(o.obtener("paginas"), Some(Json::Lista(l)) if con_ids(l)) =>
        {
            Some(o)
        }
        _ => None,
    }
}

/// Dos carpetas de paginas juntas pagina a pagina (por su `id`), en JSON
/// compacto. `None` si alguna no es una carpeta: entonces va por parrafos.
fn carpeta_junta(
    b: Option<&Json>,
    mio: &str,
    suyo: &str,
    gana_mio: bool,
    c: &Criterio,
    cuenta: &mut Cuenta,
) -> Option<String> {
    let m = como_carpeta(mio)?;
    let s = como_carpeta(suyo)?;
    // Una carpeta recien creada nace con el texto vacio o `{}`: es una base
    // sin paginas, no «nunca se acordo nada».
    let base = b.and_then(Json::como_cadena).map(|t| {
        como_carpeta(t).unwrap_or_else(|| {
            let mut o = Objeto::nuevo();
            o.poner("paginas", Json::Lista(Vec::new()));
            o
        })
    });
    Some(Json::Objeto(objeto(base.as_ref(), &m, &s, gana_mio, c, cuenta)).a_texto())
}

/// Las claves de un mensaje que son listas de cosas puestas en un
/// milisegundo del audio: `marcas` (`[ms]`, del esquema de Kotlin) y
/// `marcadores` (`[{ms, texto}]`, de `pixpin_proyecto::clase`).
pub const POR_MILISEGUNDO: [&str; 2] = ["marcas", "marcadores"];

/// El milisegundo de una marca (`1500`) o de un marcador (`{"ms":1500,…}`).
fn ms_de(e: &Json) -> Option<i64> {
    match e {
        Json::Literal(_) => numero(Some(e)),
        Json::Objeto(o) => numero(o.obtener("ms")),
        _ => None,
    }
}

/// Una lista de marcas o marcadores por milisegundo, a tres bandas. Lo que
/// falta cuenta como una lista vacia (asi se quitan todos los marcadores).
/// `None` si algo no tiene la forma (entonces decide la regla general).
fn por_milisegundo(
    b: Option<&Json>,
    m: Option<&Json>,
    s: Option<&Json>,
    gana: bool,
    c: &Criterio,
    cuenta: &mut Cuenta,
) -> Option<Vec<Json>> {
    use std::collections::BTreeMap;
    let mapa = |l: Option<&Json>| -> Option<BTreeMap<i64, Json>> {
        let Some(l) = l else {
            return Some(BTreeMap::new());
        };
        let mut salida = BTreeMap::new();
        for e in l.como_lista()? {
            salida.insert(ms_de(e)?, e.clone());
        }
        Some(salida)
    };
    let (en_b, en_m, en_s) = (mapa(b)?, mapa(m)?, mapa(s)?);
    let sin_base = b.is_none();
    let claves: std::collections::BTreeSet<i64> = en_m.keys().chain(en_s.keys()).copied().collect();
    let mut salida = Vec::new();
    for k in claves {
        let (eb, em, es) = (en_b.get(&k), en_m.get(&k), en_s.get(&k));
        // Sin base no se sabe que se quito: se queda todo lo de los dos.
        let junto = if sin_base && (em.is_none() || es.is_none()) {
            em.or(es).cloned()
        } else {
            juntar(eb, em, es, gana, None, c, cuenta)
        };
        salida.extend(junto);
    }
    Some(salida)
}

/// **La biblioteca de figuras** (`pins/draw/figuras.json`, una lista de
/// `{id, nombre, elementos, propia}`) cambiada en los dos aparatos: se juntan
/// figura a figura por su `id`, y dentro de una figura cambiada en los dos,
/// pieza a pieza. No hay base (es un archivo, no un texto de la
/// sincronizacion), asi que lo que solo esta en un lado se queda: una
/// figura guardada en el movil y otra en el PC antes de sincronizar no se
/// pierden. `None` si alguna no es una lista de figuras con `id`.
pub fn biblioteca(mio: &str, suyo: &str, c: &Criterio) -> Option<String> {
    let lista = |t: &str| match Json::analizar(t).ok()? {
        Json::Lista(l) if con_ids(&l) => Some(l),
        _ => None,
    };
    let (m, s) = (lista(mio)?, lista(suyo)?);
    let mut cuenta = Cuenta::default();
    let l = lista_con_ids(None, &m, &s, c.mio_mas_nuevo, c, &mut cuenta);
    Some(Json::Lista(l).a_texto())
}

/// Un objeto, clave por clave. Si lleva su propia hora (`updated` en una
/// figura, `tocado` en una tabla), esa decide los choques de dentro; si no,
/// lo que venga de fuera.
fn objeto(
    b: Option<&Objeto>,
    m: &Objeto,
    s: &Objeto,
    gana_de_fuera: bool,
    c: &Criterio,
    cuenta: &mut Cuenta,
) -> Objeto {
    let gana = desempate(m, s, c).unwrap_or(gana_de_fuera);
    let mut salida = Objeto::nuevo();
    let claves = m.claves().chain(s.claves().filter(|k| !m.contiene(k)));
    for k in claves {
        let v = juntar(
            b.and_then(|b| b.obtener(k)),
            m.obtener(k),
            s.obtener(k),
            gana,
            Some(k),
            c,
            cuenta,
        );
        if let Some(v) = v {
            salida.poner(k, v);
        }
    }
    // Una figura hecha de las dos no es ninguna de las dos: sube de version,
    // como si se hubiera editado, para que nadie la tome por la vieja. En
    // Android `m["version"] is JsonPrimitive` incluye un `null`: aqui igual.
    if salida != *m
        && salida != *s
        && m.contiene("id")
        && m.obtener("version").is_some_and(Json::es_primitivo)
    {
        let v = numero(m.obtener("version"))
            .unwrap_or(0)
            .max(numero(s.obtener("version")).unwrap_or(0))
            .wrapping_add(1);
        salida.poner("version", Json::numero(v));
        let hora = numero(m.obtener("updated")).unwrap_or(0).max(
            numero(s.obtener("updated"))
                .unwrap_or(0)
                .wrapping_sub(c.desfase),
        );
        if m.contiene("updated") || s.contiene("updated") {
            salida.poner("updated", Json::numero(hora));
        }
    }
    salida
}

/// `Some(true)` si gana el mio, `Some(false)` si el suyo, None si estos
/// objetos no dicen nada de su hora.
pub fn desempate(m: &Objeto, s: &Objeto, c: &Criterio) -> Option<bool> {
    for campo in ["updated", "tocado"] {
        let (Some(hm), Some(hs)) = (numero(m.obtener(campo)), numero(s.obtener(campo))) else {
            continue;
        };
        // **Dos horas a 0 no dicen nada** (4-oct-2026). Los datos de un
        // proyecto se juntan con su `tocado` puesto a 0 en los dos lados; con
        // el desfase restado, 0 contra «0 - desfase» decidia por el signo del
        // desfase entre relojes, y el nombre provisional de un proyecto recien
        // llegado del movil (su id) le ganaba al de verdad. Igual en Android
        // (`Fusion.kt`).
        if hm == 0 && hs == 0 {
            continue;
        }
        let suya = hs.wrapping_sub(c.desfase);
        if hm != suya {
            return Some(hm > suya);
        }
    }
    let vm = numero(m.obtener("version"));
    let vs = numero(s.obtener("version"));
    if let (Some(vm), Some(vs)) = (vm, vs) {
        if vm != vs {
            return Some(vm > vs);
        }
        let nm = numero(m.obtener("versionNonce"));
        let ns = numero(s.obtener("versionNonce"));
        if let (Some(nm), Some(ns)) = (nm, ns) {
            if nm != ns {
                // La regla de Excalidraw: a igual version, el nonce menor.
                return Some(nm < ns);
            }
        }
    }
    None
}

/// El numero de un literal, como lo lee Android: entero si cabe, y si no
/// como `Double` truncado. Una cadena no es un numero aunque lleve cifras.
pub(crate) fn numero(e: Option<&Json>) -> Option<i64> {
    let Some(Json::Literal(t)) = e else {
        return None;
    };
    t.parse::<i64>()
        .ok()
        .or_else(|| t.parse::<f64>().ok().map(|d| d as i64))
}

fn id_de(e: &Json) -> Option<&str> {
    e.como_objeto()?.obtener("id")?.como_cadena()
}

/// Una lista de cosas con `id` distinto cada una: figuras, trazos, hojas.
pub fn con_ids(a: &[Json]) -> bool {
    let mut vistos = HashSet::new();
    a.iter()
        .all(|e| id_de(e).is_some_and(|id| vistos.insert(id)))
}

fn de_textos(a: &[Json]) -> bool {
    a.iter().all(|e| matches!(e, Json::Cadena(_)))
}

/// Figuras, trazos: por su id. Lo anadido en cualquier lado entra; lo
/// quitado en un lado se quita si el otro no lo toco, y se queda si el otro
/// lo cambio. El orden (que figura va encima) es el del mio con lo nuevo del
/// otro colocado detras de su vecina.
fn lista_con_ids(
    b: Option<&[Json]>,
    m: &[Json],
    s: &[Json],
    gana: bool,
    c: &Criterio,
    cuenta: &mut Cuenta,
) -> Vec<Json> {
    let indice = |l: &[Json]| -> HashMap<String, Json> {
        l.iter()
            .filter_map(|e| id_de(e).map(|id| (id.to_string(), e.clone())))
            .collect()
    };
    let ids = |l: &[Json]| -> Vec<String> {
        l.iter()
            .filter_map(|e| id_de(e).map(str::to_string))
            .collect()
    };
    let en_m = indice(m);
    let en_s = indice(s);
    let en_b = b.map(indice).unwrap_or_default();
    let ids_b = b.map(ids);
    let orden = orden_junto(&ids(m), &ids(s), ids_b.as_deref(), |k| {
        let em = en_m.get(k);
        let es = en_s.get(k);
        let eb = en_b.get(k);
        match (em, es) {
            (Some(_), Some(_)) => true,
            _ if b.is_none() => true,
            // Solo en uno: si no estaba en la base, alguien lo anadio.
            _ if eb.is_none() => true,
            // Estaba y en un lado ya no: se borro alli. Se queda solo si
            // aqui se cambio.
            (Some(em), None) => {
                let vuelve = Some(em) != eb;
                if vuelve {
                    cuenta.rescatados += 1;
                }
                vuelve
            }
            (None, Some(es)) => {
                let vuelve = Some(es) != eb;
                if vuelve {
                    cuenta.rescatados += 1;
                }
                vuelve
            }
            (None, None) => false,
        }
    });
    orden
        .iter()
        .filter_map(|k| juntar(en_b.get(k), en_m.get(k), en_s.get(k), gana, None, c, cuenta))
        .collect()
}

/// Una lista de textos (grupos de una figura, croquis de un proyecto) como
/// un conjunto a tres bandas.
fn conjunto(b: Option<&[Json]>, m: &[Json], s: &[Json]) -> Vec<Json> {
    let textos = |l: &[Json]| -> Vec<String> {
        l.iter()
            .filter_map(|e| e.como_cadena().map(str::to_string))
            .collect()
    };
    let base: Option<HashSet<String>> = b.map(|b| textos(b).into_iter().collect());
    // `toSet().toList()`: sin repetidos, en el orden de la primera vez.
    let base_lista: Option<Vec<String>> = b.map(|b| {
        let mut vistos = HashSet::new();
        textos(b)
            .into_iter()
            .filter(|t| vistos.insert(t.clone()))
            .collect()
    });
    let cm = textos(m);
    let cs = textos(s);
    orden_junto(&cm, &cs, base_lista.as_deref(), |k| {
        let em = cm.iter().any(|x| x == k);
        let es = cs.iter().any(|x| x == k);
        match &base {
            _ if em && es => true,
            None => true,
            Some(base) => !base.contains(k),
        }
    })
    .into_iter()
    .map(Json::Cadena)
    .collect()
}

/// El orden de una lista juntada: la del mio, con lo que solo tiene el otro
/// metido detras de la que tenia delante en la suya. Asi lo anadido en medio
/// en la tableta cae en medio tambien aqui, y si los dos anadieron al final,
/// va primero lo mio y luego lo suyo, sin intercalar.
///
/// `sigue` dice si una clave entra; se le pregunta como mucho una vez por
/// clave y solo cuando hace falta (cuenta los rescatados, y Android tampoco
/// la llama de mas).
pub fn orden_junto(
    mio: &[String],
    suyo: &[String],
    base: Option<&[String]>,
    mut sigue: impl FnMut(&str) -> bool,
) -> Vec<String> {
    let en_mio: HashSet<&str> = mio.iter().map(String::as_str).collect();
    let en_suyo: HashSet<&str> = suyo.iter().map(String::as_str).collect();
    let en_base: HashSet<&str> = base
        .map(|b| b.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let mut salida: Vec<String> = Vec::new();
    let mut puestos: HashSet<&str> = HashSet::new();
    for k in mio {
        if !puestos.contains(k.as_str()) && sigue(k) {
            puestos.insert(k);
            salida.push(k.clone());
        }
    }
    let mut anterior: Option<&String> = None;
    for k in suyo {
        if !en_mio.contains(k.as_str()) && !salida.contains(k) && sigue(k) {
            let mut donde = anterior
                .and_then(|a| salida.iter().position(|x| x == a))
                .map_or(0, |i| i + 1);
            while donde < salida.len()
                && !en_suyo.contains(salida[donde].as_str())
                && !en_base.contains(salida[donde].as_str())
            {
                donde += 1;
            }
            salida.insert(donde.min(salida.len()), k.clone());
        }
        if salida.contains(k) {
            anterior = Some(k);
        }
    }
    salida
}

// ------------------------------------------------------------------ parrafos

/// Una nota juntada parrafo por parrafo, con diff3: se buscan los parrafos
/// que siguen iguales en los tres, y entre ellos cada tramo se toma del lado
/// que lo cambio. Si los dos cambiaron el mismo tramo, gana el mas reciente.
pub fn parrafos(base: &str, mio: &str, suyo: &str, gana_mio: bool, cuenta: &mut Cuenta) -> String {
    if mio == suyo {
        return mio.to_string();
    }
    if mio == base {
        return suyo.to_string();
    }
    if suyo == base {
        return mio.to_string();
    }
    let o: Vec<&str> = base.split('\n').collect();
    let a: Vec<&str> = mio.split('\n').collect();
    let b: Vec<&str> = suyo.split('\n').collect();
    // Una nota enorme no se compara linea a linea: gana entera la mas
    // reciente.
    if (o.len() as u64) * (a.len().max(b.len()) as u64) > TOPE_DE_COMPARAR {
        cuenta.choques += 1;
        return if gana_mio { mio } else { suyo }.to_string();
    }
    let ma = parejas_lcs(&o, &a);
    let mb = parejas_lcs(&o, &b);
    let mut salida: Vec<&str> = Vec::new();
    let (mut i, mut j, mut k) = (0usize, 0usize, 0usize);
    loop {
        while i < o.len() && ma[i] == j as isize && mb[i] == k as isize {
            salida.push(o[i]);
            i += 1;
            j += 1;
            k += 1;
        }
        if i >= o.len() && j >= a.len() && k >= b.len() {
            break;
        }
        let mut ancla = i;
        while ancla < o.len() && !(ma[ancla] >= j as isize && mb[ancla] >= k as isize) {
            ancla += 1;
        }
        if ancla >= o.len() {
            tramo(&o[i..], &a[j..], &b[k..], gana_mio, &mut salida, cuenta);
            break;
        }
        let ja = ma[ancla] as usize;
        let kb = mb[ancla] as usize;
        tramo(
            &o[i..ancla],
            &a[j..ja],
            &b[k..kb],
            gana_mio,
            &mut salida,
            cuenta,
        );
        i = ancla;
        j = ja;
        k = kb;
    }
    salida.join("\n")
}

fn tramo<'a>(
    ao: &[&'a str],
    aa: &[&'a str],
    ab: &[&'a str],
    gana_mio: bool,
    salida: &mut Vec<&'a str>,
    cuenta: &mut Cuenta,
) {
    if aa == ao {
        salida.extend_from_slice(ab);
    } else if ab == ao || aa == ab {
        salida.extend_from_slice(aa);
    } else {
        cuenta.choques += 1;
        salida.extend_from_slice(if gana_mio { aa } else { ab });
    }
}

/// Para cada linea de `o`, con que linea de `x` se empareja en la
/// subsecuencia comun mas larga (-1 si con ninguna).
fn parejas_lcs(o: &[&str], x: &[&str]) -> Vec<isize> {
    let n = o.len();
    let m = x.len();
    let ancho = m + 1;
    let mut t = vec![0u32; (n + 1) * ancho];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i * ancho + j] = if o[i] == x[j] {
                t[(i + 1) * ancho + j + 1] + 1
            } else {
                t[(i + 1) * ancho + j].max(t[i * ancho + j + 1])
            };
        }
    }
    let mut r = vec![-1isize; n];
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if o[i] == x[j] {
            r[i] = j as isize;
            i += 1;
            j += 1;
        } else if t[(i + 1) * ancho + j] >= t[i * ancho + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    r
}

// ------------------------------------------------------------------ parches

/// Que cambio de `antes` a `despues`, para mandar solo eso. En una lista de
/// figuras: las nuevas enteras, las cambiadas solo en lo que cambio, las
/// quitadas por su id, y el orden solo si se movio algo. None si son
/// iguales.
///
/// Formato: `{"v": valor}` sustituye; `{"o": {clave: parche}, "d": [claves
/// quitadas]}` cambia un objeto; `{"l": {"c": {id: parche}, "n": [figuras
/// nuevas], "d": [ids], "i": [orden]}}` cambia una lista con ids; `{"x":
/// true}` quita.
pub fn diferencia(antes: Option<&Json>, despues: Option<&Json>) -> Option<Json> {
    let a = nulo(antes);
    let d = nulo(despues);
    if a == d {
        return None;
    }
    let Some(d) = d else {
        return Some(Json::Objeto(Objeto::de(vec![(
            "x".into(),
            Json::booleano(true),
        )])));
    };
    if let (Some(Json::Objeto(oa)), Json::Objeto(od)) = (a, d) {
        let mut cambios = Objeto::nuevo();
        for (k, v) in od.iter() {
            if let Some(p) = diferencia(oa.obtener(k), Some(v)) {
                cambios.poner(k, p);
            }
        }
        let quitadas: Vec<Json> = oa
            .claves()
            .filter(|k| !od.contiene(k))
            .map(Json::cadena)
            .collect();
        let mut p = Objeto::nuevo();
        if !cambios.is_empty() {
            p.poner("o", Json::Objeto(cambios));
        }
        if !quitadas.is_empty() {
            p.poner("d", Json::Lista(quitadas));
        }
        return Some(Json::Objeto(p));
    }
    if let (Some(Json::Lista(la)), Json::Lista(ld)) = (a, d) {
        if !la.is_empty() && con_ids(la) && con_ids(ld) {
            let en_a: HashMap<&str, &Json> =
                la.iter().filter_map(|e| Some((id_de(e)?, e))).collect();
            let ids_d: Vec<&str> = ld.iter().filter_map(id_de).collect();
            let en_d: HashSet<&str> = ids_d.iter().copied().collect();
            let mut cambiadas = Objeto::nuevo();
            let mut nuevas: Vec<Json> = Vec::new();
            for e in ld {
                let Some(id) = id_de(e) else { continue };
                match en_a.get(id) {
                    None => nuevas.push(e.clone()),
                    Some(viejo) => {
                        if let Some(p) = diferencia(Some(viejo), Some(e)) {
                            cambiadas.poner(id, p);
                        }
                    }
                }
            }
            let ids_a: Vec<&str> = la.iter().filter_map(id_de).collect();
            let quitadas: Vec<Json> = ids_a
                .iter()
                .filter(|id| !en_d.contains(*id))
                .map(|id| Json::cadena(*id))
                .collect();
            let mut l = Objeto::nuevo();
            if !cambiadas.is_empty() {
                l.poner("c", Json::Objeto(cambiadas));
            }
            if !nuevas.is_empty() {
                l.poner("n", Json::Lista(nuevas.clone()));
            }
            if !quitadas.is_empty() {
                l.poner("d", Json::Lista(quitadas));
            }
            // El orden viaja solo si no es el que sale de quitar lo quitado
            // y anadir lo nuevo al final.
            let supuesto: Vec<&str> = ids_a
                .iter()
                .copied()
                .filter(|id| en_d.contains(id))
                .chain(nuevas.iter().filter_map(id_de))
                .collect();
            if supuesto != ids_d {
                l.poner(
                    "i",
                    Json::Lista(ids_d.iter().map(|id| Json::cadena(*id)).collect()),
                );
            }
            return Some(Json::Objeto(Objeto::de(vec![(
                "l".into(),
                Json::Objeto(l),
            )])));
        }
    }
    Some(Json::Objeto(Objeto::de(vec![("v".into(), d.clone())])))
}

/// Lo contrario de [diferencia]: `antes` con el `parche` puesto. Falla si el
/// parche no tiene la forma esperada (Android lanza ahi y lo recoge quien
/// aplica, que entonces pide el archivo entero).
pub fn aplicar(
    antes: Option<&Json>,
    parche: Option<&Json>,
) -> Result<Option<Json>, ParcheMalFormado> {
    let Some(parche) = parche else {
        return Ok(antes.cloned());
    };
    if matches!(parche, Json::Nulo) {
        return Ok(antes.cloned());
    }
    let Json::Objeto(p) = parche else {
        return Err(ParcheMalFormado);
    };
    // La sola presencia de la clave manda, valga lo que valga.
    if p.contiene("x") {
        return Ok(None);
    }
    if let Some(v) = p.obtener("v") {
        return Ok(Some(v.clone()));
    }
    if let Some(l) = p.obtener("l") {
        let Json::Objeto(l) = l else {
            return Err(ParcheMalFormado);
        };
        let a: &[Json] = antes.and_then(Json::como_lista).unwrap_or(&[]);
        let quitadas: HashSet<&str> = match l.obtener("d") {
            Some(Json::Lista(d)) => d
                .iter()
                .map(|e| e.contenido().ok_or(ParcheMalFormado))
                .collect::<Result<_, _>>()?,
            _ => HashSet::new(),
        };
        let cambios = l.obtener("c").and_then(Json::como_objeto);
        let mut por_id = Objeto::nuevo();
        for e in a {
            let Some(id) = id_de(e) else { continue };
            if quitadas.contains(id) {
                continue;
            }
            // Si el parche de una figura la deja en «no esta», Android se
            // queda con la de antes (`?: e`): se conserva tal cual.
            let v = match cambios.and_then(|c| c.obtener(id)) {
                Some(sub) => aplicar(Some(e), Some(sub))?.unwrap_or_else(|| e.clone()),
                None => e.clone(),
            };
            por_id.poner(id, v);
        }
        if let Some(Json::Lista(n)) = l.obtener("n") {
            for e in n {
                if let Some(id) = id_de(e) {
                    por_id.poner(id, e.clone());
                }
            }
        }
        let orden: Vec<String> = match l.obtener("i") {
            Some(Json::Lista(i)) => i
                .iter()
                .map(|e| e.contenido().map(str::to_string).ok_or(ParcheMalFormado))
                .collect::<Result<_, _>>()?,
            _ => por_id.claves().map(str::to_string).collect(),
        };
        return Ok(Some(Json::Lista(
            orden
                .iter()
                .filter_map(|k| por_id.obtener(k).cloned())
                .collect(),
        )));
    }
    let vacio = Objeto::nuevo();
    let a = antes.and_then(Json::como_objeto).unwrap_or(&vacio);
    let mut salida = a.clone();
    if let Some(Json::Lista(d)) = p.obtener("d") {
        for e in d {
            salida.quitar(e.contenido().ok_or(ParcheMalFormado)?);
        }
    }
    if let Some(Json::Objeto(o)) = p.obtener("o") {
        for (k, sub) in o.iter() {
            // Se mira `a`, el de antes, no lo ya quitado: como Android.
            match aplicar(a.obtener(k), Some(sub))? {
                None => {
                    salida.quitar(k);
                }
                Some(v) => salida.poner(k, v),
            }
        }
    }
    Ok(Some(Json::Objeto(salida)))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::canonico;

    fn j(s: &str) -> Json {
        Json::analizar(s).unwrap()
    }

    fn junto(base: Option<&Json>, mio: &Json, suyo: &Json) -> Json {
        json(
            base,
            Some(mio),
            Some(suyo),
            &Criterio::default(),
            &mut Cuenta::default(),
        )
        .unwrap()
    }

    fn ids(e: &Json) -> Vec<String> {
        e.como_objeto()
            .unwrap()
            .obtener("elements")
            .unwrap()
            .como_lista()
            .unwrap()
            .iter()
            .map(|f| {
                f.como_objeto()
                    .unwrap()
                    .obtener("id")
                    .unwrap()
                    .como_cadena()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    fn figura<'a>(e: &'a Json, id: &str) -> &'a Objeto {
        e.como_objeto()
            .unwrap()
            .obtener("elements")
            .unwrap()
            .como_lista()
            .unwrap()
            .iter()
            .map(|f| f.como_objeto().unwrap())
            .find(|f| f.obtener("id").unwrap().como_cadena() == Some(id))
            .unwrap()
    }

    fn campo<'a>(o: &'a Objeto, k: &str) -> &'a str {
        o.obtener(k).unwrap().contenido().unwrap()
    }

    fn fig(id: &str, x: i64, color: &str, v: i64, updated: i64, nonce: i64) -> String {
        format!(
            r#"{{"id":"{id}","type":"rectangle","x":{x},"strokeColor":"{color}","version":{v},"versionNonce":{nonce},"updated":{updated}}}"#
        )
    }

    fn f(id: &str) -> String {
        fig(id, 0, "#000", 1, 1, 5)
    }

    fn lienzo(figuras: &[String]) -> Json {
        j(&format!(
            r##"{{"elements":[{}],"backgroundColor":"#fff"}}"##,
            figuras.join(",")
        ))
    }

    /// Las claves de un mensaje juntado, en texto.
    fn en(e: &Json, k: &str) -> String {
        e.como_objeto()
            .unwrap()
            .obtener(k)
            .map_or("-".into(), Json::a_texto)
    }

    #[test]
    fn un_marcador_en_cada_aparato_quedan_los_dos_en_orden_y_quitar_uno_viaja() {
        // La nota de voz de una clase: el PC pone uno al minuto 1 y el movil
        // otro al 3 desde la misma base (el del segundo 10).
        let base = j(r#"{"id":"v","marcadores":[{"ms":10000,"texto":"a"}],"marcas":[5]}"#);
        let mio = j(
            r#"{"id":"v","marcadores":[{"ms":10000,"texto":"a"},{"ms":60000,"texto":"pc"}],"marcas":[5,7]}"#,
        );
        let suyo = j(
            r#"{"id":"v","marcadores":[{"ms":10000,"texto":"a"},{"ms":180000,"texto":"movil"}],"marcas":[5,9]}"#,
        );
        let r = junto(Some(&base), &mio, &suyo);
        assert_eq!(
            en(&r, "marcadores"),
            r#"[{"ms":10000,"texto":"a"},{"ms":60000,"texto":"pc"},{"ms":180000,"texto":"movil"}]"#
        );
        assert_eq!(en(&r, "marcas"), "[5,7,9]");
        // Quitado en uno (el del segundo 10) y anadido otro en el otro: el
        // quitado se va, el anadido entra.
        let mio = j(r#"{"id":"v","marcadores":[],"marcas":[]}"#);
        let r = junto(Some(&base), &mio, &suyo);
        assert_eq!(en(&r, "marcadores"), r#"[{"ms":180000,"texto":"movil"}]"#);
        assert_eq!(en(&r, "marcas"), "[9]");
        // Caso negativo: quitarlos todos en los dos lados quita la clave
        // `marcadores` (como `clase.rs`), y `marcas` se queda en `[]`.
        let r = junto(
            Some(&base),
            &j(r#"{"id":"v","marcas":[],"x":1}"#),
            &j(r#"{"id":"v","marcas":[],"x":2}"#),
        );
        assert_eq!(en(&r, "marcadores"), "-");
        assert_eq!(en(&r, "marcas"), "[]");
    }

    #[test]
    fn la_misma_nota_del_mismo_minuto_cambiada_en_los_dos_gana_una_sin_duplicar() {
        let base = j(r#"{"marcadores":[{"ms":1,"texto":"a"}]}"#);
        let mio = j(r#"{"marcadores":[{"ms":1,"texto":"pc"}]}"#);
        let suyo = j(r#"{"marcadores":[{"ms":1,"texto":"movil"}]}"#);
        let r = junto(Some(&base), &mio, &suyo);
        assert_eq!(en(&r, "marcadores"), r#"[{"ms":1,"texto":"pc"}]"#);
        // Caso negativo: una lista que no es de marcadores (sin `ms`) sigue
        // la regla general, entera.
        let r = junto(
            None,
            &j(r#"{"marcadores":[{"x":1}]}"#),
            &j(r#"{"marcadores":[{"x":2}]}"#),
        );
        assert_eq!(en(&r, "marcadores"), r#"[{"x":1}]"#);
    }

    #[test]
    fn una_pagina_anadida_a_la_carpeta_en_cada_aparato_quedan_las_dos() {
        let carpeta = |paginas: &[&str]| {
            let p: Vec<String> = paginas
                .iter()
                .map(|id| format!(r#"{{"id":"{id}","titulo":"Día {id}"}}"#))
                .collect();
            Json::cadena(format!(r#"{{"formato":1,"paginas":[{}]}}"#, p.join(",")))
        };
        let mensaje = |texto: Json| {
            let mut o = Objeto::nuevo();
            o.poner("miniapp", Json::cadena("carpeta"));
            o.poner("texto", texto);
            Json::Objeto(o)
        };
        let base = mensaje(carpeta(&["1"]));
        let r = junto(
            Some(&base),
            &mensaje(carpeta(&["1", "2"])),
            &mensaje(carpeta(&["1", "3"])),
        );
        let texto = r.como_objeto().unwrap().obtener("texto").unwrap();
        let doc = j(texto.como_cadena().unwrap());
        let ids: Vec<String> = doc
            .como_objeto()
            .unwrap()
            .obtener("paginas")
            .unwrap()
            .como_lista()
            .unwrap()
            .iter()
            .map(|p| id_de(p).unwrap().to_string())
            .collect();
        assert_eq!(ids, ["1", "2", "3"]);
        // La carpeta recien creada (texto vacio) cuenta como sin paginas.
        let r = junto(
            Some(&mensaje(Json::cadena(""))),
            &mensaje(carpeta(&["2"])),
            &mensaje(carpeta(&["3"])),
        );
        let t = r.como_objeto().unwrap().obtener("texto").unwrap().a_texto();
        assert!(t.contains(r#"\"2\""#) && t.contains(r#"\"3\""#), "{t}");
        // Caso negativo: un texto que no es una carpeta sigue por parrafos.
        let r = junto(
            Some(&mensaje(Json::cadena("a"))),
            &mensaje(Json::cadena("a\nb")),
            &mensaje(Json::cadena("c\na")),
        );
        assert_eq!(
            r.como_objeto()
                .unwrap()
                .obtener("texto")
                .unwrap()
                .como_cadena(),
            Some("c\na\nb")
        );
    }

    #[test]
    fn la_biblioteca_de_figuras_guardada_en_los_dos_no_pierde_ninguna() {
        let fig = |id: &str, nombre: &str| {
            format!(
                r#"{{"id":"{id}","nombre":"{nombre}","elementos":[{}],"propia":true}}"#,
                f(&format!("{id}-pieza"))
            )
        };
        let aqui = format!("[{},{}]", fig("serie-1", "Norte"), fig("pc", "Sello PC"));
        let alli = format!(
            "[{},{}]",
            fig("serie-1", "Norte"),
            fig("movil", "Sello movil")
        );
        let junta = biblioteca(&aqui, &alli, &Criterio::default()).unwrap();
        let l = j(&junta);
        let ids: Vec<&str> = l.como_lista().unwrap().iter().filter_map(id_de).collect();
        assert_eq!(ids, ["serie-1", "pc", "movil"]);
        // Caso negativo: algo que no es una lista de figuras no se junta.
        assert_eq!(biblioteca("{}", &alli, &Criterio::default()), None);
        assert_eq!(biblioteca(&aqui, "no es json", &Criterio::default()), None);
    }

    #[test]
    fn a_mas_b_y_a_mas_c_dan_a_mas_b_mas_c() {
        let base = lienzo(&[f("a")]);
        let mio = lienzo(&[f("a"), f("b")]);
        let suyo = lienzo(&[f("a"), f("c")]);
        assert_eq!(ids(&junto(Some(&base), &mio, &suyo)), vec!["a", "b", "c"]);
    }

    #[test]
    fn movida_en_uno_y_con_otro_color_en_el_otro_queda_movida_y_con_el_color_nuevo() {
        let base = lienzo(&[fig("a", 0, "#000", 1, 10, 5)]);
        let mio = lienzo(&[fig("a", 50, "#000", 2, 20, 5)]);
        let suyo = lienzo(&[fig("a", 0, "#f00", 2, 30, 5)]);
        let r = junto(Some(&base), &mio, &suyo);
        let a = figura(&r, "a");
        assert_eq!(campo(a, "x"), "50");
        assert_eq!(campo(a, "strokeColor"), "#f00");
        // Hecha de las dos: sube de version por encima de las dos.
        assert_eq!(campo(a, "version"), "3");
        // Y la hora es la mayor de las dos.
        assert_eq!(campo(a, "updated"), "30");
    }

    #[test]
    fn la_misma_propiedad_cambiada_en_los_dos_la_gana_el_ultimo_cambio() {
        let base = lienzo(&[fig("a", 0, "#000", 1, 10, 5)]);
        let mio = lienzo(&[fig("a", 50, "#000", 2, 40, 5)]);
        let suyo = lienzo(&[fig("a", 90, "#000", 2, 30, 5)]);
        let mut cuenta = Cuenta::default();
        let r = json(
            Some(&base),
            Some(&mio),
            Some(&suyo),
            &Criterio::default(),
            &mut cuenta,
        )
        .unwrap();
        assert_eq!(campo(figura(&r, "a"), "x"), "50");
        assert!(cuenta.choques > 0);
    }

    #[test]
    fn un_reloj_adelantado_no_gana_por_ir_adelantado() {
        let base = lienzo(&[fig("a", 0, "#000", 1, 10, 5)]);
        let mio = lienzo(&[fig("a", 50, "#000", 2, 1_000, 5)]);
        // El otro va una hora adelantado: su 1.500 es mi 1.500 - 3.600.000.
        let suyo = lienzo(&[fig("a", 90, "#000", 2, 1_500 + 3_600_000, 5)]);
        let criterio = Criterio {
            mio_mas_nuevo: true,
            desfase: 3_600_000,
        };
        let r = json(
            Some(&base),
            Some(&mio),
            Some(&suyo),
            &criterio,
            &mut Cuenta::default(),
        )
        .unwrap();
        assert_eq!(campo(figura(&r, "a"), "x"), "90");
        let mio2 = lienzo(&[fig("a", 50, "#000", 2, 2_000, 5)]);
        let sin_corregir = json(
            Some(&base),
            Some(&mio2),
            Some(&suyo),
            &criterio,
            &mut Cuenta::default(),
        )
        .unwrap();
        assert_eq!(campo(figura(&sin_corregir, "a"), "x"), "50");
    }

    #[test]
    fn borrada_en_uno_y_cambiada_en_el_otro_vuelve_a_aparecer() {
        let base = lienzo(&[f("a"), f("b")]);
        let mio = lienzo(&[f("a")]);
        let suyo = lienzo(&[f("a"), fig("b", 0, "#0f0", 2, 1, 5)]);
        let mut cuenta = Cuenta::default();
        let r = json(
            Some(&base),
            Some(&mio),
            Some(&suyo),
            &Criterio::default(),
            &mut cuenta,
        )
        .unwrap();
        assert_eq!(ids(&r), vec!["a", "b"]);
        assert_eq!(campo(figura(&r, "b"), "strokeColor"), "#0f0");
        assert_eq!(cuenta.rescatados, 1);
    }

    #[test]
    fn borrada_en_uno_y_sin_tocar_en_el_otro_se_borra() {
        let base = lienzo(&[f("a"), f("b")]);
        assert_eq!(
            ids(&junto(Some(&base), &lienzo(&[f("a")]), &base)),
            vec!["a"]
        );
        assert_eq!(
            ids(&junto(Some(&base), &base, &lienzo(&[f("a")]))),
            vec!["a"]
        );
    }

    #[test]
    fn sin_base_se_junta_todo_y_no_se_pierde_nada() {
        let mio = lienzo(&[f("a"), f("b")]);
        let suyo = lienzo(&[f("a"), f("c")]);
        assert_eq!(ids(&junto(None, &mio, &suyo)), vec!["a", "b", "c"]);
    }

    #[test]
    fn lo_nuevo_del_otro_cae_junto_a_su_vecina_no_al_final() {
        let base = lienzo(&[f("a"), f("z")]);
        let mio = lienzo(&[f("a"), f("z"), f("m")]);
        let suyo = lienzo(&[f("a"), f("c"), f("z")]);
        assert_eq!(
            ids(&junto(Some(&base), &mio, &suyo)),
            vec!["a", "c", "z", "m"]
        );
    }

    #[test]
    fn una_tabla_se_junta_celda_por_celda() {
        let base = j(r#"{"celdas":{"A1":"1","B1":"2"},"tocado":10}"#);
        let mio = j(r#"{"celdas":{"A1":"uno","B1":"2"},"tocado":20}"#);
        let suyo = j(r#"{"celdas":{"A1":"1","B1":"2","C1":"=A1+B1"},"tocado":30}"#);
        let r = junto(Some(&base), &mio, &suyo);
        let celdas = r
            .como_objeto()
            .unwrap()
            .obtener("celdas")
            .unwrap()
            .como_objeto()
            .unwrap();
        assert_eq!(campo(celdas, "A1"), "uno");
        assert_eq!(campo(celdas, "C1"), "=A1+B1");
        // La misma celda en los dos: la tabla tocada mas tarde.
        let otra = junto(
            Some(&base),
            &j(r#"{"celdas":{"A1":"mia"},"tocado":20}"#),
            &j(r#"{"celdas":{"A1":"suya"},"tocado":30}"#),
        );
        let celdas = otra
            .como_objeto()
            .unwrap()
            .obtener("celdas")
            .unwrap()
            .como_objeto()
            .unwrap();
        assert_eq!(campo(celdas, "A1"), "suya");
    }

    /// **La tabla en el texto de un mensaje** (la mini-app `tabla`, las hojas
    /// editadas de un Excel) tambien se junta celda a celda, venga escrita
    /// por el PC (una celda por linea) o por el movil (en una linea).
    #[test]
    fn la_tabla_del_texto_de_un_mensaje_se_junta_celda_a_celda() {
        let msj = |texto: &str| {
            Json::Objeto(Objeto::de(vec![
                ("id".into(), Json::cadena("1")),
                ("texto".into(), Json::cadena(texto)),
            ]))
        };
        let base = "{\n  \"nombre\": \"T\",\n  \"celdas\": {\n    \"A1\": \"1\",\n    \"B1\": \"2\"\n  },\n  \"tocado\": 10,\n  \"protegida\": true\n}";
        // El PC cambia A1 y anade A2 (en su JSON con sangria); el movil anade
        // C1 (compacto). Por parrafos esto era un choque o un JSON roto.
        let pc = "{\n  \"nombre\": \"T\",\n  \"celdas\": {\n    \"A1\": \"uno\",\n    \"A2\": \"x\",\n    \"B1\": \"2\"\n  },\n  \"tocado\": 20,\n  \"protegida\": true\n}";
        let movil = r#"{"nombre":"T","celdas":{"A1":"1","B1":"2","C1":"=A1+B1"},"tocado":30,"protegida":true}"#;
        let mut cuenta = Cuenta::default();
        let r = json(
            Some(&msj(base)),
            Some(&msj(pc)),
            Some(&msj(movil)),
            &Criterio::default(),
            &mut cuenta,
        )
        .unwrap();
        let texto = r
            .como_objeto()
            .unwrap()
            .obtener("texto")
            .unwrap()
            .como_cadena()
            .unwrap()
            .to_string();
        let t = j(&texto);
        let celdas = t
            .como_objeto()
            .unwrap()
            .obtener("celdas")
            .unwrap()
            .como_objeto()
            .unwrap();
        assert_eq!(campo(celdas, "A1"), "uno");
        assert_eq!(campo(celdas, "A2"), "x");
        assert_eq!(campo(celdas, "C1"), "=A1+B1");
        assert_eq!(
            t.como_objeto().unwrap().obtener("protegida"),
            Some(&Json::Literal("true".into())),
            "lo que el PC no conoce sigue"
        );
        // Caso negativo: un texto que no es una tabla sigue por parrafos.
        let nota = json(
            Some(&msj("a\n\nb")),
            Some(&msj("a2\n\nb")),
            Some(&msj("a\n\nb2")),
            &Criterio::default(),
            &mut Cuenta::default(),
        )
        .unwrap();
        assert_eq!(
            nota.como_objeto()
                .unwrap()
                .obtener("texto")
                .unwrap()
                .como_cadena(),
            Some("a2\n\nb2")
        );
        // Ni una tabla rota se toma por tabla.
        assert!(como_tabla("{\"celdas\": {").is_none());
        assert!(como_tabla("{\"celdas\": [1]}").is_none());
    }

    #[test]
    fn una_nota_se_junta_parrafo_por_parrafo() {
        let base = "Título\n\nPrimero.\n\nSegundo.\n\nTercero.";
        let mio = "Título\n\nPrimero, corregido.\n\nSegundo.\n\nTercero.";
        let suyo = "Título\n\nPrimero.\n\nSegundo.\n\nTercero.\n\nCuarto nuevo.";
        assert_eq!(
            parrafos(base, mio, suyo, true, &mut Cuenta::default()),
            "Título\n\nPrimero, corregido.\n\nSegundo.\n\nTercero.\n\nCuarto nuevo."
        );
        // Dentro de un objeto, por su clave.
        let nota = |t: &str| Json::Objeto(Objeto::de(vec![("nota".into(), Json::cadena(t))]));
        let r = junto(Some(&nota(base)), &nota(mio), &nota(suyo));
        let texto = campo(r.como_objeto().unwrap(), "nota");
        assert!(texto.contains("corregido"));
        assert!(texto.contains("Cuarto"));
        // Caso negativo: con otra clave el texto no se junta, choca entero.
        let otra = |t: &str| Json::Objeto(Objeto::de(vec![("titulo".into(), Json::cadena(t))]));
        let mut cuenta = Cuenta::default();
        let r = json(
            Some(&otra(base)),
            Some(&otra(mio)),
            Some(&otra(suyo)),
            &Criterio::default(),
            &mut cuenta,
        )
        .unwrap();
        assert_eq!(campo(r.como_objeto().unwrap(), "titulo"), mio);
        assert_eq!(cuenta.choques, 1);
    }

    #[test]
    fn el_mismo_parrafo_cambiado_en_los_dos_lo_gana_el_mas_reciente() {
        let mut cuenta = Cuenta::default();
        assert_eq!(
            parrafos("a\nb\nc", "a\nB mío\nc", "a\nB suyo\nc", false, &mut cuenta),
            "a\nB suyo\nc"
        );
        assert_eq!(cuenta.choques, 1);
    }

    #[test]
    fn los_grupos_de_una_figura_se_juntan_como_conjunto() {
        let r = junto(
            Some(&j(r#"{"g":["x"]}"#)),
            &j(r#"{"g":["x","y"]}"#),
            &j(r#"{"g":[]}"#),
        );
        let g: Vec<&str> = r
            .como_objeto()
            .unwrap()
            .obtener("g")
            .unwrap()
            .como_lista()
            .unwrap()
            .iter()
            .map(|e| e.como_cadena().unwrap())
            .collect();
        assert_eq!(g, vec!["y"]);
    }

    #[test]
    fn el_parche_lleva_solo_lo_cambiado_y_al_aplicarlo_sale_lo_mismo() {
        let muchas: Vec<String> = (1..=300)
            .map(|i| fig(&format!("f{i}"), i, "#000", 1, 1, 5))
            .collect();
        let antes = lienzo(&muchas);
        let mut cambiadas: Vec<String> = muchas[1..]
            .iter()
            .map(|s| {
                if s.contains("\"f7\"") {
                    fig("f7", 999, "#000", 2, 1, 5)
                } else {
                    s.clone()
                }
            })
            .collect();
        cambiadas.push(f("nueva"));
        let despues = lienzo(&cambiadas);
        let parche = diferencia(Some(&antes), Some(&despues)).unwrap();
        let largo_parche = parche.a_texto().len();
        let largo_lienzo = despues.a_texto().len();
        assert!(
            largo_parche * 10 < largo_lienzo,
            "el parche ocupa {largo_parche} y el lienzo {largo_lienzo}"
        );
        let puesto = aplicar(Some(&antes), Some(&parche)).unwrap().unwrap();
        assert_eq!(
            canonico::de(&despues.a_texto()),
            canonico::de(&puesto.a_texto())
        );
    }

    #[test]
    fn el_parche_de_un_orden_cambiado_lo_reordena() {
        let antes = lienzo(&[f("a"), f("b"), f("c")]);
        let despues = lienzo(&[f("c"), f("a"), fig("b", 3, "#000", 1, 1, 5)]);
        let parche = diferencia(Some(&antes), Some(&despues));
        assert_eq!(
            aplicar(Some(&antes), parche.as_ref()).unwrap(),
            Some(despues)
        );
    }

    #[test]
    fn sin_cambios_no_hay_parche() {
        let a = lienzo(&[f("a")]);
        assert_eq!(diferencia(Some(&a), Some(&a)), None);
        assert_eq!(aplicar(Some(&a), None).unwrap(), Some(a.clone()));
        assert_eq!(aplicar(Some(&a), Some(&Json::Nulo)).unwrap(), Some(a));
    }

    #[test]
    fn quitar_una_clave_viaja_en_el_parche() {
        let antes = j(r#"{"a":1,"b":{"c":2,"d":3}}"#);
        let despues = j(r#"{"a":1,"b":{"c":2}}"#);
        let parche = diferencia(Some(&antes), Some(&despues));
        assert_eq!(
            parche.as_ref().map(Json::a_texto).as_deref(),
            Some(r#"{"o":{"b":{"d":["d"]}}}"#)
        );
        assert_eq!(
            aplicar(Some(&antes), parche.as_ref()).unwrap(),
            Some(despues)
        );
        let vacia = Json::Lista(vec![]);
        let uno = j("[1]");
        let parche = diferencia(Some(&uno), Some(&vacia));
        assert_eq!(aplicar(Some(&uno), parche.as_ref()).unwrap(), Some(vacia));
    }

    #[test]
    fn quitar_del_todo_viaja_como_x_y_un_parche_roto_se_rechaza() {
        let a = j(r#"{"a":1}"#);
        let parche = diferencia(Some(&a), None).unwrap();
        assert_eq!(parche.a_texto(), r#"{"x":true}"#);
        assert_eq!(aplicar(Some(&a), Some(&parche)).unwrap(), None);
        // Caso negativo: un parche que no es un objeto no se aplica.
        assert_eq!(aplicar(Some(&a), Some(&j("[1]"))), Err(ParcheMalFormado));
        assert_eq!(
            aplicar(Some(&a), Some(&j(r#"{"l":{"d":[{"no":1}]}}"#))),
            Err(ParcheMalFormado)
        );
    }

    #[test]
    fn el_desempate_mira_la_hora_luego_la_version_y_luego_el_nonce() {
        let c = Criterio::default();
        let o = |s: &str| j(s).como_objeto().unwrap().clone();
        assert_eq!(
            desempate(&o(r#"{"updated":5}"#), &o(r#"{"updated":9}"#), &c),
            Some(false)
        );
        assert_eq!(
            desempate(
                &o(r#"{"updated":5,"version":3}"#),
                &o(r#"{"updated":5,"version":2}"#),
                &c
            ),
            Some(true)
        );
        assert_eq!(
            desempate(
                &o(r#"{"version":2,"versionNonce":1}"#),
                &o(r#"{"version":2,"versionNonce":9}"#),
                &c
            ),
            Some(true)
        );
        assert_eq!(
            desempate(&o(r#"{"version":2}"#), &o(r#"{"version":2}"#), &c),
            None
        );
        assert_eq!(desempate(&o(r#"{"x":1}"#), &o(r#"{"x":2}"#), &c), None);
        // Una hora escrita como cadena no cuenta: solo los literales.
        assert_eq!(
            desempate(&o(r#"{"updated":"9"}"#), &o(r#"{"updated":5}"#), &c),
            None
        );
        // Y una con decimales se trunca, como `toDouble().toLong()`.
        assert_eq!(numero(Some(&Json::Literal("9.9".into()))), Some(9));
        assert_eq!(numero(Some(&Json::Literal("1e3".into()))), Some(1000));
    }

    #[test]
    fn juntar_listas_conserva_el_orden_y_lo_de_cada_lado() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let base = l(&["a", "b", "c"]);
        let mio = l(&["a", "b", "y"]);
        let suyo = l(&["a", "x", "b", "c"]);
        // «c» se quito en el mio; «x» se anadio en el suyo detras de «a»;
        // «y» en el mio.
        let r = orden_junto(&mio, &suyo, Some(&base), |k| {
            (mio.iter().any(|m| m == k) && suyo.iter().any(|s| s == k))
                || !base.iter().any(|b| b == k)
        });
        assert_eq!(r, l(&["a", "x", "b", "y"]));
    }

    #[test]
    fn una_figura_con_version_nula_sube_de_version_igual_que_en_android() {
        // `m["version"] is JsonPrimitive` es cierto para un null en kotlinx.
        let base = j(r#"{"id":"a","version":null,"x":0,"y":0}"#);
        let mio = j(r#"{"id":"a","version":null,"x":1,"y":0}"#);
        let suyo = j(r#"{"id":"a","version":null,"x":0,"y":1}"#);
        let r = junto(Some(&base), &mio, &suyo);
        assert_eq!(r.a_texto(), r#"{"id":"a","version":1,"x":1,"y":1}"#);
    }
}
