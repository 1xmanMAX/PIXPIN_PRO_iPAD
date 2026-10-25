//! Juntar dos versiones de un proyecto sin preguntar y sin que mande nadie.
//!
//! Puerto de `sincro/Mezcla.kt`. Un proyecto es un indice: que hojas tiene,
//! en que orden, como se llama. Si en el telefono se anade una hoja y en la
//! tableta otra, lo que uno espera es tener las dos, no elegir entre dos
//! listas.
//!
//! Hasta el 15-sep-2026 un aparato podia ponerse de maestro y ganar los
//! empates; ese dia la sincronizacion tomo un lienzo que faltaba por un
//! borrado y lo quito en los dos. Desde entonces:
//!
//! - Las hojas se suman. Una que falta en un lado solo se quita si alli se
//!   quito a mano (la marca `quitadas`), y ni asi si en el otro lado se
//!   cambio: lo modificado gana a lo borrado.
//! - Cada hoja y los datos del proyecto se juntan campo a campo con
//!   `fusion`: una nota por parrafos; el nombre cambiado en un solo lado
//!   pasa; cambiado en los dos, gana el proyecto tocado mas tarde.
//! - El orden: el del mio con lo nuevo del otro detras de su vecina.
//!
//! Aqui el proyecto es su JSON portatil, el mismo que viaja y que Android
//! escribe con `Proyectos.json` (`encodeDefaults = true`, nulos explicitos):
//! este crate no conoce la estructura `Proyecto` (vive una capa mas arriba)
//! y no le hace falta, porque todo se decide por claves. Quien llame tiene
//! que dar los dos lados codificados de la misma manera, o «iguales» y
//! «distintos» no significarian lo mismo que en el movil.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::canonico::{Json, Objeto};
use crate::fusion::{self, Criterio, Cuenta};

/// Cuantas marcas de hojas quitadas se recuerdan por proyecto
/// (`Proyectos.MARCAS_DE_QUITADAS`).
pub const MARCAS_DE_QUITADAS: usize = 400;

/// Quien decide cuando los dos lados no dicen lo mismo.
///
/// Puerto de `Sesion.loMioManda` (`sincro/Protocolo.kt:404-412`, del
/// 21-sep-2026). Existe para un caso raro que la fusion normal empeora: el
/// usuario vacio el portatil creyendo que el telefono lo volveria a llenar,
/// y como los borrados del portatil eran lo mas reciente, juntar los hizo
/// ganar. Con [Mando::LoMioManda] aqui no se borra ni se cambia nada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mando {
    /// Lo de siempre: las hojas se suman y los empates los decide la hora.
    #[default]
    Acordar,
    /// Una sola direccion: si aqui hay proyecto, ese queda tal cual.
    LoMioManda,
}

/// El proyecto juntado, o None si no hay ninguno.
///
/// `cambiadas_aqui` son las hojas (por id) y croquis cuyos archivos cambiaron
/// en este aparato desde lo acordado (ver [cambiadas]): una hoja quitada
/// alli y cambiada aqui vuelve. `cambiadas_alli`, lo mismo del otro.
/// `desfase` es cuanto va adelantado el reloj del otro.
pub fn proyecto(
    mio: Option<&Json>,
    suyo: Option<&Json>,
    base: Option<&Json>,
    cambiadas_aqui: &HashSet<String>,
    cambiadas_alli: &HashSet<String>,
    desfase: i64,
) -> Option<Json> {
    proyecto_con(
        mio,
        suyo,
        base,
        cambiadas_aqui,
        cambiadas_alli,
        desfase,
        Mando::Acordar,
    )
}

/// Lo mismo, diciendo quien manda. Ver [Mando].
///
/// Con [Mando::LoMioManda] no se junta nada: el proyecto de aqui queda tal
/// cual, con sus hojas, sus croquis, su nombre y sus marcas de quitadas, y
/// lo del otro no entra ni siquiera para ganar un empate por la hora. Es lo
/// que hace Android, que ni llama a la mezcla en ese modo
/// (`Protocolo.kt:563-575`: `var junto = mio ?: suyo` y luego solo junta
/// `if (... && !loMioManda)`). Lo que solo tiene el otro si viaja, porque un
/// proyecto que aqui no existe no puede pisar a nadie.
pub fn proyecto_con(
    mio: Option<&Json>,
    suyo: Option<&Json>,
    base: Option<&Json>,
    cambiadas_aqui: &HashSet<String>,
    cambiadas_alli: &HashSet<String>,
    desfase: i64,
    mando: Mando,
) -> Option<Json> {
    if mando == Mando::LoMioManda {
        return mio.or(suyo).cloned();
    }
    let (Some(mio), Some(suyo)) = (mio, suyo) else {
        return mio.or(suyo).cloned();
    };
    if mio == suyo {
        return Some(mio.clone());
    }
    let tocado_mio = tocado(mio);
    let tocado_suyo = tocado(suyo);
    let criterio = Criterio {
        mio_mas_nuevo: tocado_mio >= tocado_suyo.wrapping_sub(desfase),
        desfase,
    };
    let mas_nuevo = || if criterio.mio_mas_nuevo { mio } else { suyo };
    let (Some(om), Some(os)) = (mio.como_objeto(), suyo.como_objeto()) else {
        // Sin forma de proyecto no hay nada que juntar campo a campo.
        return Some(mas_nuevo().clone());
    };
    let ob = base.and_then(Json::como_objeto);

    // Los datos sueltos del proyecto (nombre, archivado, PDF, codigos),
    // campo a campo. En Android el resultado se decodifica a `Proyecto` y si
    // no cuadra se toma el mas nuevo: aqui, si no es un objeto con id y
    // nombre.
    let campos_b = ob.map(campos);
    let datos = fusion::json(
        campos_b.as_ref(),
        Some(&campos(om)),
        Some(&campos(os)),
        &criterio,
        &mut Cuenta::default(),
    );
    let mut datos = match datos {
        Some(Json::Objeto(o)) if es_proyecto(&o) => o,
        _ => mas_nuevo().como_objeto().cloned().unwrap_or_default(),
    };

    let hm = unicas(hojas_de(om));
    let hs = unicas(hojas_de(os));
    let hb = ob.map(|b| unicas(hojas_de(b))).unwrap_or_default();
    let indice =
        |h: &[(String, Objeto)]| -> HashMap<String, Objeto> { h.iter().cloned().collect() };
    let (en_m, en_s, en_b) = (indice(&hm), indice(&hs), indice(&hb));
    let quitadas_aqui: HashSet<String> = textos_de(om, "quitadas").into_iter().collect();
    let quitadas_alli: HashSet<String> = textos_de(os, "quitadas").into_iter().collect();
    let ids =
        |h: &[(String, Objeto)]| -> Vec<String> { h.iter().map(|(id, _)| id.clone()).collect() };
    // La base va con sus hojas tal cual, repetidas incluidas, como en Android.
    let ids_b: Option<Vec<String>> =
        ob.map(|b| hojas_de(b).into_iter().map(|(id, _)| id).collect());
    let orden = fusion::orden_junto(&ids(&hm), &ids(&hs), ids_b.as_deref(), |k| {
        let m = en_m.get(k);
        let s = en_s.get(k);
        let b = en_b.get(k);
        match (m, s) {
            (Some(_), Some(_)) => true,
            // Falta en el suyo: se queda salvo que alli se quitara a mano y
            // aqui nadie la tocara.
            (Some(m), None) => {
                !quitadas_alli.contains(k)
                    || cambiadas_aqui.contains(k)
                    || b.is_some_and(|b| m != b)
            }
            (None, Some(s)) => {
                !quitadas_aqui.contains(k)
                    || cambiadas_alli.contains(k)
                    || b.is_some_and(|b| s != b)
            }
            (None, None) => false,
        }
    });
    let hojas: Vec<Json> = orden
        .iter()
        .filter_map(|k| hoja(en_b.get(k), en_m.get(k), en_s.get(k), &criterio))
        .map(Json::Objeto)
        .collect();

    let croquis_m = textos_de(om, "croquis");
    let croquis_s = textos_de(os, "croquis");
    let croquis_b = ob.map(|b| textos_de(b, "croquis"));
    let croquis = fusion::orden_junto(
        &distintos(&croquis_m),
        &distintos(&croquis_s),
        croquis_b.as_deref().map(distintos).as_deref(),
        |c| {
            let m = croquis_m.iter().any(|x| x == c);
            let s = croquis_s.iter().any(|x| x == c);
            let en_base = croquis_b.as_ref().is_some_and(|b| b.iter().any(|x| x == c));
            match (m, s) {
                (true, true) => true,
                _ if !en_base => true,
                (true, _) => cambiadas_aqui.contains(c),
                _ => cambiadas_alli.contains(c),
            }
        },
    );

    let quedan: HashSet<&String> = orden.iter().collect();
    let mut quitadas: Vec<String> = Vec::new();
    for q in textos_de(om, "quitadas")
        .into_iter()
        .chain(textos_de(os, "quitadas"))
    {
        if !quitadas.contains(&q) && !quedan.contains(&q) {
            quitadas.push(q);
        }
    }
    let desde = quitadas.len().saturating_sub(MARCAS_DE_QUITADAS);
    let quitadas: Vec<Json> = quitadas.drain(desde..).map(Json::Cadena).collect();

    // `datos.copy(...)`: el id es el mio, y lo demas lo recien juntado.
    datos.poner("id", om.obtener("id").cloned().unwrap_or(Json::Nulo));
    datos.poner("hojas", Json::Lista(hojas));
    datos.poner(
        "croquis",
        Json::Lista(croquis.into_iter().map(Json::Cadena).collect()),
    );
    datos.poner("tocado", Json::numero(tocado_mio.max(tocado_suyo)));
    datos.poner("quitadas", Json::Lista(quitadas));
    Some(Json::Objeto(datos))
}

/// Una hoja juntada campo a campo; el id se queda el mio. Si lo juntado no
/// tiene forma de hoja, la del proyecto mas nuevo.
fn hoja(
    b: Option<&Objeto>,
    m: Option<&Objeto>,
    s: Option<&Objeto>,
    criterio: &Criterio,
) -> Option<Objeto> {
    let Some(m) = m else {
        return s.cloned();
    };
    let Some(s) = s else {
        return Some(m.clone());
    };
    if m == s {
        return Some(m.clone());
    }
    let j = |h: Option<&Objeto>| h.map(|h| Json::Objeto(h.clone()));
    let junto = fusion::json(
        j(b).as_ref(),
        j(Some(m)).as_ref(),
        j(Some(s)).as_ref(),
        criterio,
        &mut Cuenta::default(),
    );
    match junto {
        Some(Json::Objeto(mut o))
            if o.obtener("id").is_some_and(|id| id.como_cadena().is_some()) =>
        {
            o.poner("id", m.obtener("id").cloned().unwrap_or(Json::Nulo));
            Some(o)
        }
        _ => Some(if criterio.mio_mas_nuevo { m } else { s }.clone()),
    }
}

/// Que hojas y croquis cambiaron en un lado desde lo acordado, por sus
/// archivos: `archivos` es ruta → resumen de ese lado y `acordado` lo de la
/// base. Un archivo que no estaba en la base tambien cuenta como cambiado.
pub fn cambiadas(
    p: Option<&Json>,
    archivos: &BTreeMap<String, String>,
    acordado: &BTreeMap<String, String>,
) -> HashSet<String> {
    let mut salida = HashSet::new();
    let Some(p) = p.and_then(Json::como_objeto) else {
        return salida;
    };
    let cambio = |rel: Option<String>| -> bool {
        rel.is_some_and(|rel| {
            archivos
                .get(&rel)
                .is_some_and(|r| acordado.get(&rel) != Some(r))
        })
    };
    for (id, h) in hojas_de(p) {
        let texto = |k: &str| h.obtener(k).and_then(Json::como_cadena).map(str::to_string);
        if cambio(texto("dibujo").map(|d| format!("pins/draw/{d}.excalidraw.gz")))
            || cambio(texto("croquis").map(|c| format!("croquis3d/{c}.croquis.gz")))
            || cambio(texto("tabla").map(|t| format!("tablas/{}.json", limpio(&t))))
        {
            salida.insert(id);
        }
    }
    for c in textos_de(p, "croquis") {
        if cambio(Some(format!("croquis3d/{c}.croquis.gz"))) {
            salida.insert(c);
        }
    }
    salida
}

/// `[^A-Za-z0-9._-]` → `_`, por unidades UTF-16 como el `Regex` de Kotlin:
/// un emoji son dos guiones bajos, no uno.
fn limpio(s: &str) -> String {
    s.encode_utf16()
        .map(|u| match char::from_u32(u as u32) {
            Some(c) if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') => c,
            _ => '_',
        })
        .collect()
}

fn tocado(p: &Json) -> i64 {
    fusion::numero(p.como_objeto().and_then(|o| o.obtener("tocado"))).unwrap_or(0)
}

/// `p.copy(hojas = emptyList(), croquis = emptyList(), quitadas = emptyList(), tocado = 0L)`.
fn campos(p: &Objeto) -> Json {
    let mut o = p.clone();
    o.poner("hojas", Json::Lista(vec![]));
    o.poner("croquis", Json::Lista(vec![]));
    o.poner("quitadas", Json::Lista(vec![]));
    o.poner("tocado", Json::numero(0));
    Json::Objeto(o)
}

/// Lo minimo para que `Proyecto.serializer()` lo decodificara: los dos
/// campos sin valor por omision.
fn es_proyecto(o: &Objeto) -> bool {
    ["id", "nombre"]
        .iter()
        .all(|k| o.obtener(k).is_some_and(|v| v.como_cadena().is_some()))
}

/// Las hojas de un proyecto con su id, en orden. Una sin id de texto no es
/// una hoja que Android pudiera leer, y se salta.
fn hojas_de(p: &Objeto) -> Vec<(String, Objeto)> {
    p.obtener("hojas")
        .and_then(Json::como_lista)
        .map(|l| {
            l.iter()
                .filter_map(|h| {
                    let o = h.como_objeto()?;
                    let id = o.obtener("id")?.como_cadena()?;
                    Some((id.to_string(), o.clone()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Nunca dos hojas con el mismo id: se queda la primera.
fn unicas(h: Vec<(String, Objeto)>) -> Vec<(String, Objeto)> {
    let mut vistas = HashSet::new();
    h.into_iter()
        .filter(|(id, _)| vistas.insert(id.clone()))
        .collect()
}

fn textos_de(p: &Objeto, clave: &str) -> Vec<String> {
    p.obtener(clave)
        .and_then(Json::como_lista)
        .map(|l| {
            l.iter()
                .filter_map(|e| e.como_cadena().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn distintos(l: &[String]) -> Vec<String> {
    let mut vistos = HashSet::new();
    l.iter()
        .filter(|s| vistos.insert(s.as_str()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una hoja como la escribe `Proyectos.json`: todos los campos, los
    /// nulos explicitos, en el orden de la clase.
    fn hoja_json(id: &str, nombre: &str, dibujo: Option<&str>, nota: Option<&str>) -> String {
        let c =
            |v: Option<&str>| v.map_or("null".to_string(), |s| serde_json::to_string(s).unwrap());
        format!(
            r#"{{"id":"{id}","nombre":"{nombre}","dibujo":{},"marco":null,"nota":{},"pagina":null,"deMensaje":null,"croquis":null,"vista":null,"tabla":null,"origen":null,"padre":null,"uid":null}}"#,
            c(dibujo),
            c(nota)
        )
    }

    fn proyecto_json(
        id: &str,
        nombre: &str,
        hojas: &[String],
        tocado: i64,
        croquis: &[&str],
        quitadas: &[&str],
    ) -> Json {
        let lista = |v: &[&str]| {
            v.iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(",")
        };
        Json::analizar(&format!(
            r#"{{"id":"{id}","nombre":"{nombre}","hojas":[{}],"archivado":false,"tocado":{tocado},"pdfOrigen":null,"pdfLimpio":null,"croquis":[{}],"origen":null,"uid":null,"creado":0,"aparato":null,"quitadas":[{}]}}"#,
            hojas.join(","),
            lista(croquis),
            lista(quitadas)
        ))
        .unwrap()
    }

    fn con(p: &Json, clave: &str, valor: Json) -> Json {
        let mut o = p.como_objeto().unwrap().clone();
        o.poner(clave, valor);
        Json::Objeto(o)
    }

    fn nombre_de(p: &Json) -> String {
        p.como_objeto()
            .unwrap()
            .obtener("nombre")
            .unwrap()
            .como_cadena()
            .unwrap()
            .to_string()
    }

    fn ids_de_hojas(p: &Json) -> Vec<String> {
        hojas_de(p.como_objeto().unwrap())
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    }

    fn nada() -> HashSet<String> {
        HashSet::new()
    }

    fn junto(mio: &Json, suyo: &Json, base: &Json) -> Json {
        proyecto(Some(mio), Some(suyo), Some(base), &nada(), &nada(), 0).unwrap()
    }

    #[test]
    fn sin_ninguno_no_hay_proyecto_y_con_uno_solo_es_ese() {
        assert_eq!(proyecto(None, None, None, &nada(), &nada(), 0), None);
        let p = proyecto_json("p", "obra", &[], 1, &[], &[]);
        assert_eq!(
            proyecto(Some(&p), None, None, &nada(), &nada(), 0),
            Some(p.clone())
        );
        assert_eq!(
            proyecto(None, Some(&p), None, &nada(), &nada(), 0),
            Some(p.clone())
        );
        // Iguales: el mio, sin tocar nada.
        assert_eq!(
            proyecto(Some(&p), Some(&p), None, &nada(), &nada(), 0),
            Some(p)
        );
    }

    /// 4-oct-2026: con el desfase entre relojes, el nombre lo ganaba el lado
    /// que dijera el signo del desfase, no el mas reciente. Asi el nombre
    /// provisional (el id) de un proyecto recien llegado del movil se quedaba.
    #[test]
    fn sin_base_y_con_desfase_el_nombre_lo_gana_el_mas_reciente() {
        let hoja = hoja_json("h1", "planta", None, None);
        let provisional = proyecto_json("pr-movil", "pr-movil", &[], 0, &[], &[]);
        let de_verdad = proyecto_json("pr-movil", "Reforma", &[hoja], 1_791_000_000_000, &[], &[]);
        for desfase in [-3, 0, 3, 5_000] {
            let r = proyecto(
                Some(&provisional),
                Some(&de_verdad),
                None,
                &nada(),
                &nada(),
                desfase,
            )
            .unwrap();
            assert_eq!(nombre_de(&r), "Reforma", "desfase {desfase}");
            let r = proyecto(
                Some(&de_verdad),
                Some(&provisional),
                None,
                &nada(),
                &nada(),
                desfase,
            )
            .unwrap();
            assert_eq!(nombre_de(&r), "Reforma", "desfase {desfase}, al reves");
        }
    }

    #[test]
    fn sin_maestro_el_nombre_cambiado_en_los_dos_lo_gana_el_ultimo() {
        let hoja = hoja_json("h1", "planta", None, None);
        let base = proyecto_json("p", "obra", &[hoja], 100, &[], &[]);
        let mio = con(
            &con(&base, "nombre", Json::cadena("obra vieja")),
            "tocado",
            Json::numero(200),
        );
        let suyo = con(
            &con(&base, "nombre", Json::cadena("obra nueva")),
            "tocado",
            Json::numero(300),
        );
        assert_eq!(nombre_de(&junto(&mio, &suyo, &base)), "obra nueva");
        let mio = con(&mio, "tocado", Json::numero(400));
        assert_eq!(nombre_de(&junto(&mio, &suyo, &base)), "obra vieja");
        // Cambiado en uno solo, pasa aunque el otro sea mas reciente.
        let mio = con(&base, "tocado", Json::numero(900));
        let r = junto(&mio, &suyo, &base);
        assert_eq!(nombre_de(&r), "obra nueva");
        // El tocado es el mayor de los dos y el id el mio.
        assert_eq!(
            r.como_objeto()
                .unwrap()
                .obtener("tocado")
                .unwrap()
                .a_texto(),
            "900"
        );
        assert_eq!(
            r.como_objeto()
                .unwrap()
                .obtener("id")
                .unwrap()
                .como_cadena(),
            Some("p")
        );
    }

    #[test]
    fn las_hojas_nuevas_de_los_dos_se_suman_y_una_nota_se_junta_por_parrafos() {
        let a = |nota: &str| hoja_json("h1", "", None, Some(nota));
        let base = proyecto_json("p", "obra", &[a("uno\ndos")], 100, &[], &[]);
        let mio = proyecto_json(
            "p",
            "obra",
            &[a("UNO\ndos"), hoja_json("h2", "", None, None)],
            200,
            &[],
            &[],
        );
        let suyo = proyecto_json(
            "p",
            "obra",
            &[a("uno\ndos\ntres"), hoja_json("h3", "", None, None)],
            300,
            &[],
            &[],
        );
        let r = junto(&mio, &suyo, &base);
        assert_eq!(ids_de_hojas(&r), vec!["h1", "h2", "h3"]);
        let hojas = hojas_de(r.como_objeto().unwrap());
        assert_eq!(
            hojas[0].1.obtener("nota").unwrap().como_cadena(),
            Some("UNO\ndos\ntres")
        );
    }

    #[test]
    fn una_hoja_quitada_a_mano_en_un_lado_y_cambiada_en_el_otro_se_queda() {
        let h = hoja_json("h1", "", Some("d1"), None);
        let base = proyecto_json("p", "obra", &[h], 100, &[], &[]);
        // `Proyectos.sinHoja(base, "h1", 200)`.
        let mio = proyecto_json("p", "obra", &[], 200, &[], &["h1"]);
        assert!(ids_de_hojas(&junto(&mio, &base, &base)).is_empty());
        let suyo = con(&base, "tocado", Json::numero(300));
        let alli: HashSet<String> = ["h1".to_string()].into_iter().collect();
        let r = proyecto(Some(&mio), Some(&suyo), Some(&base), &nada(), &alli, 0).unwrap();
        assert_eq!(ids_de_hojas(&r), vec!["h1"]);
        assert!(
            textos_de(r.como_objeto().unwrap(), "quitadas").is_empty(),
            "vuelve y pierde la marca"
        );
        // Y la que falta sin marca vuelve sola.
        let sin_marca = proyecto_json("p", "obra", &[], 200, &[], &[]);
        assert_eq!(ids_de_hojas(&junto(&sin_marca, &base, &base)), vec!["h1"]);
    }

    #[test]
    fn el_caso_de_tesis_sin_maestro_no_se_pierde_ningun_lienzo() {
        // El telefono quedo con el mismo lienzo tres veces y tocado despues;
        // la tableta anadio uno suyo. Nadie manda: quedan los tres.
        let h1 = hoja_json("h1", "Uno", Some("d1"), None);
        let h2 = hoja_json("h2", "Dos", Some("d2"), None);
        let h3 = hoja_json("h3", "Tres", Some("d3"), None);
        let base = proyecto_json("tesis", "Tesis", &[h1.clone(), h2.clone()], 5, &[], &[]);
        let tableta = proyecto_json("tesis", "Tesis", &[h1, h2.clone(), h3], 6, &[], &[]);
        let telefono = proyecto_json("tesis", "Tesis", &[h2.clone(), h2.clone(), h2], 9, &[], &[]);
        let r = junto(&telefono, &tableta, &base);
        let mut ids = ids_de_hojas(&r);
        ids.sort();
        assert_eq!(ids, vec!["h1", "h2", "h3"]);
    }

    #[test]
    fn los_croquis_se_juntan_como_conjunto_y_las_marcas_de_quitadas_se_acotan() {
        let base = proyecto_json("p", "obra", &[], 1, &["c1", "c2"], &[]);
        // Aqui se quito c2 sin tocarlo; alli se anadio c3.
        let mio = proyecto_json("p", "obra", &[], 2, &["c1"], &[]);
        let suyo = proyecto_json("p", "obra", &[], 3, &["c1", "c2", "c3"], &[]);
        let r = junto(&mio, &suyo, &base);
        assert_eq!(
            textos_de(r.como_objeto().unwrap(), "croquis"),
            vec!["c1", "c3"]
        );
        // Pero si aqui c2 se cambio, se queda.
        let aqui: HashSet<String> = ["c2".to_string()].into_iter().collect();
        let r = proyecto(Some(&suyo), Some(&mio), Some(&base), &aqui, &nada(), 0).unwrap();
        assert_eq!(
            textos_de(r.como_objeto().unwrap(), "croquis"),
            vec!["c1", "c2", "c3"]
        );
        // Las marcas se suman sin repetir y solo se guardan las ultimas 400.
        let muchas: Vec<String> = (0..450).map(|i| format!("q{i}")).collect();
        let refs: Vec<&str> = muchas.iter().map(String::as_str).collect();
        let mio = proyecto_json("p", "obra", &[], 2, &[], &refs[..300]);
        let suyo = proyecto_json("p", "obra", &[], 3, &[], &refs[200..]);
        let q = textos_de(junto(&mio, &suyo, &base).como_objeto().unwrap(), "quitadas");
        assert_eq!(q.len(), MARCAS_DE_QUITADAS);
        assert_eq!(q.first().map(String::as_str), Some("q50"));
        assert_eq!(q.last().map(String::as_str), Some("q449"));
    }

    /// Los dos lados del caso del portatil vaciado: aqui quedo una hoja y se
    /// quito otra a mano; alli estan las dos y ademas una suya, y alli se
    /// toco despues.
    fn el_portatil_y_el_telefono() -> (Json, Json, Json) {
        let h1 = hoja_json("h1", "Uno", Some("d1"), None);
        let h2 = hoja_json("h2", "Dos", Some("d2"), None);
        let base = proyecto_json("p", "obra", &[h1.clone(), h2.clone()], 100, &["c1"], &[]);
        let mio = proyecto_json("p", "obra mia", &[h1], 200, &["c1"], &["h2"]);
        let suyo = proyecto_json(
            "p",
            "obra suya",
            &[
                hoja_json("h1", "Uno", Some("d1"), None),
                h2,
                hoja_json("h3", "Tres", Some("d3"), None),
            ],
            900,
            &["c1", "c2"],
            &[],
        );
        (base, mio, suyo)
    }

    fn manda_lo_mio(mio: Option<&Json>, suyo: Option<&Json>, base: Option<&Json>) -> Option<Json> {
        proyecto_con(mio, suyo, base, &nada(), &nada(), 0, Mando::LoMioManda)
    }

    #[test]
    fn con_lo_mio_manda_el_proyecto_de_aqui_queda_tal_cual_y_lo_del_otro_no_se_cuela() {
        let (base, mio, suyo) = el_portatil_y_el_telefono();
        let r = manda_lo_mio(Some(&mio), Some(&suyo), Some(&base)).unwrap();
        // Ni una hoja suya, ni su croquis, ni su nombre, ni su hora: el mio
        // entero, byte a byte, que es lo que Android guarda en ese modo.
        assert_eq!(r, mio);
        assert_eq!(ids_de_hojas(&r), vec!["h1"]);
        assert_eq!(nombre_de(&r), "obra mia");
        assert_eq!(textos_de(r.como_objeto().unwrap(), "croquis"), vec!["c1"]);
    }

    #[test]
    fn con_lo_mio_manda_la_hoja_que_yo_quite_sigue_quitada_aunque_alli_la_tocaran_despues() {
        let (base, mio, suyo) = el_portatil_y_el_telefono();
        let alli: HashSet<String> = ["h2".to_string()].into_iter().collect();
        // Acordar la rescata, porque lo modificado gana a lo borrado.
        let acordando = proyecto(Some(&mio), Some(&suyo), Some(&base), &nada(), &alli, 0).unwrap();
        assert_eq!(ids_de_hojas(&acordando), vec!["h1", "h2", "h3"]);
        // Lo mio manda no la rescata, y la marca de quitada se queda para que
        // la proxima vuelta tampoco la vuelva a traer.
        let mandando = proyecto_con(
            Some(&mio),
            Some(&suyo),
            Some(&base),
            &nada(),
            &alli,
            0,
            Mando::LoMioManda,
        )
        .unwrap();
        assert_eq!(ids_de_hojas(&mandando), vec!["h1"]);
        assert_eq!(
            textos_de(mandando.como_objeto().unwrap(), "quitadas"),
            vec!["h2"]
        );
    }

    #[test]
    fn con_lo_mio_manda_lo_que_solo_tiene_el_otro_llega_entero() {
        let (_, mio, suyo) = el_portatil_y_el_telefono();
        // Un proyecto que aqui no existe no pisa nada, asi que viaja.
        assert_eq!(manda_lo_mio(None, Some(&suyo), None), Some(suyo));
        assert_eq!(manda_lo_mio(Some(&mio), None, None), Some(mio));
        assert_eq!(manda_lo_mio(None, None, None), None);
    }

    #[test]
    fn acordar_es_exactamente_lo_de_siempre_y_no_lo_cambia_el_mando() {
        let (base, mio, suyo) = el_portatil_y_el_telefono();
        let aqui: HashSet<String> = ["c1".to_string()].into_iter().collect();
        let alli: HashSet<String> = ["h2".to_string()].into_iter().collect();
        let viejo = proyecto(Some(&mio), Some(&suyo), Some(&base), &aqui, &alli, 7);
        let nuevo = proyecto_con(
            Some(&mio),
            Some(&suyo),
            Some(&base),
            &aqui,
            &alli,
            7,
            Mando::Acordar,
        );
        assert_eq!(viejo, nuevo);
        assert_eq!(Mando::default(), Mando::Acordar);
        // Y sigue juntando: estan las hojas de los dos y el croquis que solo
        // anadio el otro.
        let r = nuevo.unwrap();
        assert_eq!(ids_de_hojas(&r), vec!["h1", "h2", "h3"]);
        assert_eq!(
            textos_de(r.como_objeto().unwrap(), "croquis"),
            vec!["c1", "c2"]
        );
    }

    #[test]
    fn cambiadas_mira_los_archivos_de_cada_hoja_y_croquis() {
        let p = Json::analizar(
            r#"{"id":"p","nombre":"x","hojas":[{"id":"h1","dibujo":"d1"},{"id":"h2","tabla":"t/1"},{"id":"h3","croquis":"c9"}],"croquis":["c1","c2"]}"#,
        )
        .unwrap();
        let m = |pares: &[(&str, &str)]| -> BTreeMap<String, String> {
            pares
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        let archivos = m(&[
            ("pins/draw/d1.excalidraw.gz", "nuevo"),
            ("tablas/t_1.json", "igual"),
            ("croquis3d/c1.croquis.gz", "sin base"),
            ("croquis3d/c2.croquis.gz", "igual"),
        ]);
        let acordado = m(&[
            ("pins/draw/d1.excalidraw.gz", "viejo"),
            ("tablas/t_1.json", "igual"),
            ("croquis3d/c2.croquis.gz", "igual"),
        ]);
        let mut r: Vec<String> = cambiadas(Some(&p), &archivos, &acordado)
            .into_iter()
            .collect();
        r.sort();
        // h2 esta igual; h3 senala un croquis que aqui no existe; c1 no
        // estaba en la base y por eso cuenta.
        assert_eq!(r, vec!["c1", "h1"]);
        assert!(cambiadas(None, &archivos, &acordado).is_empty());
        assert_eq!(limpio("a b/c😀"), "a_b_c__");
    }
}
