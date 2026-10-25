//! **Un lienzo del PC, en la forma que el movil sabe leer.**
//!
//! El movil lee cada lienzo con `ExcalidrawJson.decodeFromString<Scene>`
//! (`motor/ExcalidrawStore.kt::cargar`, kotlinx con `ignoreUnknownKeys` y
//! SIN `coerceInputValues`): un solo campo que no encaje tira la escena
//! ENTERA y el lienzo se abre en blanco. Los lienzos nacidos en el PC llevan
//! los puntos como `[x, y]` (la forma de Excalidraw) y el `Pt` del movil los
//! quiere como `{"x":…,"y":…}`; con eso solo ya no se abria nada de lo que
//! el PC anotaba y mandaba al sincronizar (queja del usuario del 27-sep: «de
//! la PC no se enviaron las anotaciones que tengo en el PC»).
//!
//! Aqui vive la regla, una sola vez, para los dos caminos que mandan
//! lienzos: el envio por Wi-Fi (`recibir/lienzo_suelto.rs` de la app) y la
//! sincronizacion (`vista::DiscoPc::a_portatil`). Las reglas estan copiadas
//! de `motor/Element.kt` y `motor/Scene.kt` (main de 2026-09-26).
//!
//! **Solo se toca lo que el movil no leeria.** Un lienzo que ya esta bien
//! (el que vino del movil, o uno del PC sin trazos) se devuelve tal cual,
//! byte a byte: reescribirlo cambiaria sus decimales y su resumen, y la
//! sincronizacion lo tomaria por cambiado en cada vuelta.

use serde_json::{Map, Value};

/// Los `type` que el movil sabe leer (`ElementType` de `motor/Element.kt`).
/// Un elemento con otro tira la escena entera alla.
pub const TIPOS_DEL_MOVIL: &[&str] = &[
    "rectangle",
    "diamond",
    "ellipse",
    "arrow",
    "line",
    "freedraw",
    "text",
    "image",
    "pixpin-mosaic",
    "pixpin-spotlight",
    "pixpin-lupa",
    "pixpin-serial",
    "pixpin-measure",
    "pixpin-arc",
    "pixpin-region",
    "pixpin-scalebar",
    "frame",
    "pixpin-point",
    "pixpin-axes",
    "pixpin-number-line",
    "pixpin-space",
    "pixpin-solid",
    "pixpin-gantt",
];

/// Los campos de enumeracion del `Element` del movil y las palabras que
/// admite cada uno. Con otra palabra kotlinx rechaza la escena; sin el campo,
/// el movil pone el suyo de fabrica. Por eso lo que no encaja se QUITA.
const ENUMERADOS: &[(&str, &[&str])] = &[
    (
        "fillStyle",
        &["hachure", "cross-hatch", "solid", "zigzag", "pixpin-lines"],
    ),
    ("strokeStyle", &["solid", "dashed", "dotted"]),
    ("textAlign", &["left", "center", "right"]),
    ("verticalAlign", &["top", "middle", "bottom"]),
    ("startArrowhead", PUNTAS),
    ("endArrowhead", PUNTAS),
    (
        "material",
        &[
            "lisa",
            "luz",
            "hdr",
            "rayado",
            "cruzado",
            "puntos",
            "tiza",
            "lapiz2b",
            "seco",
            "trama",
            "cuadritos",
        ],
    ),
    ("dureza", &["4H", "2H", "HB", "2B", "4B", "6B", "8B"]),
    ("papel", &["a4", "a5", "carta", "cuadrada", "apaisada"]),
    ("pauta", &["lisa", "rayada", "cuadros", "puntos"]),
    (
        "formaSolida",
        &[
            "caja",
            "cuna",
            "cilindro",
            "prisma",
            "revolucion",
            "extrusion",
        ],
    ),
];

const PUNTAS: &[&str] = &[
    "arrow",
    "bar",
    "circle",
    "circle_outline",
    "triangle",
    "triangle_outline",
    "diamond",
    "diamond_outline",
];

/// Los campos `Int` del `Element` del movil: un `1.0` o un numero que no
/// cabe en 32 bits hace fallar la lectura.
const ENTEROS: &[&str] = &[
    "roughness",
    "opacity",
    "seed",
    "version",
    "versionNonce",
    "fontFamily",
    "periodos",
    "oscurecer",
];

/// Los campos que son un `Pt` (`{"x","y"}`) o listas de ellos.
const PUNTO_SUELTO: &[&str] = &["lastCommittedPoint", "foco"];
const LISTA_DE_PUNTOS: &[&str] = &["points", "planta", "forma"];

/// Un elemento como lo lee el `Element` del movil, o `None` si no hay forma
/// (un `type` que alla no existe, o sin `id`).
pub fn elemento(v: Value) -> Option<Value> {
    let Value::Object(mut e) = v else {
        return None;
    };
    let tipo = e.get("type").and_then(Value::as_str)?;
    if !TIPOS_DEL_MOVIL.contains(&tipo) {
        return None;
    }
    let id = e.get("id").and_then(Value::as_str)?.to_string();
    for clave in ["x", "y", "width", "height"] {
        if !e.get(clave).is_some_and(Value::is_number) {
            e.insert(clave.into(), Value::from(0));
        }
    }
    // `seed` no tiene valor por omision alla: sin el, no se lee.
    if !e.get("seed").is_some_and(Value::is_number) {
        e.insert("seed".into(), Value::from(semilla_de(&id)));
    }
    for clave in ENTEROS {
        if let Some(n) = e.get(*clave).and_then(Value::as_f64) {
            e.insert((*clave).into(), Value::from(entero_de_32(n)));
        }
    }
    if let Some(n) = e.get("updated").and_then(Value::as_f64) {
        e.insert("updated".into(), Value::from(n.round() as i64));
    }
    for (clave, palabras) in ENUMERADOS {
        let vale = match e.get(*clave) {
            None | Some(Value::Null) => true,
            Some(Value::String(s)) => palabras.contains(&s.as_str()),
            Some(_) => false,
        };
        if !vale {
            e.remove(*clave);
        }
    }
    for clave in PUNTO_SUELTO {
        if let Some(p) = e.get_mut(*clave)
            && !a_punto(p)
        {
            e.remove(*clave);
        }
    }
    for clave in LISTA_DE_PUNTOS {
        if let Some(l) = e.get_mut(*clave)
            && !a_puntos(l)
        {
            e.remove(*clave);
        }
    }
    if let Some(Value::Array(huecos)) = e.get_mut("huecos") {
        huecos.retain_mut(a_puntos);
    }
    if let Some((w, h)) = caja_que_falta(&e) {
        e.insert("width".into(), Value::from(w));
        e.insert("height".into(), Value::from(h));
    }
    // Cada atadura con su `id` y un `type` que alla exista.
    if let Some(Value::Array(atados)) = e.get_mut("boundElements") {
        atados.retain(|a| {
            a.get("id").is_some_and(Value::is_string)
                && a.get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|t| TIPOS_DEL_MOVIL.contains(&t))
        });
    }
    for clave in ["startBinding", "endBinding"] {
        let vale = match e.get(clave) {
            None | Some(Value::Null) => true,
            Some(b) => {
                b.get("elementId").is_some_and(Value::is_string)
                    && b.get("mode")
                        .and_then(Value::as_str)
                        .is_none_or(|m| m == "orbit" || m == "inside")
            }
        };
        if !vale {
            e.remove(clave);
        }
    }
    if let Some(r) = e.get("roundness")
        && !r.is_null()
    {
        match r.get("type").and_then(Value::as_f64) {
            Some(t) => {
                e.insert(
                    "roundness".into(),
                    serde_json::json!({ "type": entero_de_32(t) }),
                );
            }
            None => {
                e.insert("roundness".into(), Value::Null);
            }
        }
    }
    if let Some(c) = e.get("crop")
        && !c.is_null()
        && !["x", "y", "width", "height", "naturalWidth", "naturalHeight"]
            .iter()
            .all(|k| c.get(*k).is_some_and(Value::is_number))
    {
        e.remove("crop");
    }
    Some(Value::Object(e))
}

/// `[x, y]` a `{"x": x, "y": y}`. Devuelve si quedo un punto.
pub fn a_punto(v: &mut Value) -> bool {
    match v {
        Value::Null => true,
        Value::Object(m) => {
            m.get("x").is_some_and(Value::is_number) && m.get("y").is_some_and(Value::is_number)
        }
        Value::Array(a) if a.len() >= 2 && a[0].is_number() && a[1].is_number() => {
            *v = serde_json::json!({ "x": a[0], "y": a[1] });
            true
        }
        _ => false,
    }
}

/// Una lista de puntos. Los que no son punto se quitan.
pub fn a_puntos(v: &mut Value) -> bool {
    match v {
        Value::Null => true,
        Value::Array(l) => {
            l.retain_mut(|p| !p.is_null() && a_punto(p));
            true
        }
        _ => false,
    }
}

/// Los tipos que el movil pinta como una raya de su primer punto al ultimo
/// cuando su caja mide menos de dos pixeles (`Renderer.sePierdeDePequeno`,
/// solo los que llevan puntos).
const DE_PUNTOS: &[&str] = &["freedraw", "line", "arrow"];

/// La caja de los puntos de un elemento (en cualquiera de las dos formas),
/// o `None` si no tiene ninguno.
fn caja_de_los_puntos(e: &Map<String, Value>) -> Option<(f64, f64)> {
    let mut caja: Option<(f64, f64, f64, f64)> = None;
    for p in e.get("points")?.as_array()? {
        let (x, y) = match p {
            Value::Array(a) if a.len() >= 2 => (a[0].as_f64()?, a[1].as_f64()?),
            Value::Object(o) => (o.get("x")?.as_f64()?, o.get("y")?.as_f64()?),
            _ => continue,
        };
        caja = Some(match caja {
            None => (x, y, x, y),
            Some((x1, y1, x2, y2)) => (x1.min(x), y1.min(y), x2.max(x), y2.max(y)),
        });
    }
    caja.map(|(x1, y1, x2, y2)| (x2 - x1, y2 - y1))
}

/// **La caja que le falta a un trazo, una linea o una flecha del PC.**
///
/// El PC los escribia con `width` y `height` a 0 y el movil pinta lo que
/// mide menos de dos pixeles como la cuerda de su primer punto al ultimo
/// (`pintarComoRaya`): lo anotado aqui llegaba como rayas rectas (queja del
/// 28-sep-2026). Alli la caja es la de los puntos (`Element.withPoint`).
/// Solo se toca la caja **a cero** de algo que si se extiende: el movil
/// nunca la escribe asi, y una suya que no sea la exacta (la deja al mover
/// un punto suelto) no es asunto de esto.
fn caja_que_falta(e: &Map<String, Value>) -> Option<(f64, f64)> {
    let tipo = e.get("type").and_then(Value::as_str)?;
    if !DE_PUNTOS.contains(&tipo) {
        return None;
    }
    let cero = |k: &str| e.get(k).and_then(Value::as_f64).is_none_or(|n| n == 0.0);
    if !(cero("width") && cero("height")) {
        return None;
    }
    caja_de_los_puntos(e).filter(|(w, h)| w + h > 0.0)
}

fn entero_de_32(n: f64) -> i64 {
    (n.round() as i64).clamp(i32::MIN as i64, i32::MAX as i64)
}

/// Una semilla que no cambia para el mismo id (FNV-1a), en positivo.
fn semilla_de(id: &str) -> i64 {
    let mut h: u32 = 0x811c_9dc5;
    for b in id.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    (h & 0x7fff_ffff) as i64
}

/// Si el `Element` del movil rechazaria este elemento, y con el la escena
/// entera: un `type` que alla no existe, puntos que no son `Pt`, un entero
/// con decimales o una palabra fuera de su enumeracion.
fn lo_tiraria(v: &Value) -> bool {
    let Value::Object(e) = v else {
        return true;
    };
    if !e.get("id").is_some_and(Value::is_string)
        || !e
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| TIPOS_DEL_MOVIL.contains(&t))
    {
        return true;
    }
    let es_pt = |p: &Value| {
        p.is_null()
            || p.get("x").is_some_and(Value::is_number) && p.get("y").is_some_and(Value::is_number)
    };
    let lista_de_pt = |l: &Value| l.is_null() || l.as_array().is_some_and(|l| l.iter().all(es_pt));
    if LISTA_DE_PUNTOS
        .iter()
        .any(|k| e.get(*k).is_some_and(|l| !lista_de_pt(l)))
        || PUNTO_SUELTO
            .iter()
            .any(|k| e.get(*k).is_some_and(|p| !es_pt(p)))
        || e.get("huecos").is_some_and(|h| {
            !h.is_null() && !h.as_array().is_some_and(|l| l.iter().all(lista_de_pt))
        })
    {
        return true;
    }
    let entero = |x: &Value| x.is_null() || x.as_i64().is_some_and(|n| i32::try_from(n).is_ok());
    if ENTEROS
        .iter()
        .any(|k| e.get(*k).is_some_and(|x| !entero(x)))
    {
        return true;
    }
    ENUMERADOS.iter().any(|(k, palabras)| match e.get(*k) {
        None | Some(Value::Null) => false,
        Some(Value::String(s)) => !palabras.contains(&s.as_str()),
        Some(_) => true,
    })
}

/// Si una entrada de `files` tiene con que leerse alli: su `path` como
/// texto. Es lo que le falta a una foto de la web de Excalidraw (solo trae
/// `dataURL`); el `id` y el `mimeType` los pone siempre quien escribe aqui y
/// alli, y exigirlos tambien quitaria fotos que el movil si ensena.
fn foto_legible(v: &Value) -> bool {
    v.get("path").is_some_and(Value::is_string)
}

/// **El lienzo `texto` como lo lee el movil, solo si hace falta.**
///
/// `None` si ya se lee alli tal cual (o si no es un lienzo que se entienda:
/// entonces no se toca, que no es cosa de esto decidir que se hace con el).
/// `Some` con el lienzo arreglado si algun elemento, alguna foto o el papel
/// lo tirarian: los elementos que el movil no conoce se quedan fuera, los
/// puntos pasan a objeto, los enteros a enteros, y las fotos sin ruta (un
/// `dataURL` de la web de Excalidraw) se quitan de `files`, que alla una
/// entrada sin `path` tira la escena y una foto que no esta solo no se ve.
pub fn lienzo_legible(texto: &str) -> Option<String> {
    let mut raiz: Value = serde_json::from_str(texto).ok()?;
    let mapa = raiz.as_object_mut()?;
    let mut cambio = false;
    let Some(Value::Array(elementos)) = mapa.get_mut("elements") else {
        return None;
    };
    // Solo si algo lo TIRARIA alla se pasa todo por `elemento`: un trazo del
    // movil sin algun campo que su `Element` rellena solo no es motivo, y
    // tocarlo haria que el mismo fichero diera otro resumen aqui que alla.
    // Y lo que alli se pintaria como una raya recta (la caja a cero de un
    // trazo del PC) tambien.
    let sin_caja = |v: &Value| v.as_object().is_some_and(|e| caja_que_falta(e).is_some());
    if elementos.iter().any(|e| lo_tiraria(e) || sin_caja(e)) {
        cambio = true;
        let antes = std::mem::take(elementos);
        *elementos = antes.into_iter().filter_map(elemento).collect();
    }
    // El papel: el `Scene` lo quiere como texto arriba.
    if mapa.get("backgroundColor").is_some_and(|b| !b.is_string()) {
        mapa.remove("backgroundColor");
        cambio = true;
    }
    if let Some(Value::Object(files)) = mapa.get_mut("files") {
        let malas: Vec<String> = files
            .iter()
            .filter(|(_, v)| !foto_legible(v))
            .map(|(k, _)| k.clone())
            .collect();
        for k in &malas {
            files.remove(k);
        }
        cambio |= !malas.is_empty();
    } else if mapa.get("files").is_some_and(|f| !f.is_null()) {
        mapa.insert("files".into(), Value::Object(Map::new()));
        cambio = true;
    }
    if !cambio {
        return None;
    }
    serde_json::to_string(&raiz).ok()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn entero32(v: &Value) -> bool {
        v.as_i64().is_some_and(|n| i32::try_from(n).is_ok())
    }

    fn es_pt(v: &Value) -> bool {
        v.get("x").is_some_and(Value::is_number) && v.get("y").is_some_and(Value::is_number)
    }

    /// Lo que haria `ExcalidrawJson.decodeFromString<Scene>` del movil con lo
    /// que mira esta prueba: el primer motivo por el que tiraria la escena.
    fn como_lo_lee_el_movil(json: &str) -> Result<(), String> {
        let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let elementos = v
            .get("elements")
            .and_then(Value::as_array)
            .ok_or("sin elements")?;
        for e in elementos {
            let id = e.get("id").and_then(Value::as_str).ok_or("sin id")?;
            let tipo = e.get("type").and_then(Value::as_str).ok_or("sin type")?;
            if !TIPOS_DEL_MOVIL.contains(&tipo) {
                return Err(format!("{id}: type {tipo}"));
            }
            if !e.get("seed").is_some_and(entero32) {
                return Err(format!("{id}: seed"));
            }
            for k in ["opacity", "roughness", "version"] {
                if let Some(x) = e.get(k).filter(|x| !x.is_null())
                    && !entero32(x)
                {
                    return Err(format!("{id}: {k}"));
                }
            }
            if let Some(p) = e.get("points").filter(|p| !p.is_null())
                && !p.as_array().is_some_and(|l| l.iter().all(es_pt))
            {
                return Err(format!("{id}: points no son Pt"));
            }
            // `Renderer.sePierdeDePequeno`: lo que mide menos de dos
            // pixeles se pinta como la raya del primer punto al ultimo.
            // Un trazo que se extiende no puede llegar con la caja a cero.
            let num = |k: &str| e.get(k).and_then(Value::as_f64).unwrap_or(0.0);
            if DE_PUNTOS.contains(&tipo)
                && let Some((w, h)) = caja_de_los_puntos(e.as_object().unwrap())
                && w + h >= 2.0
                && num("width") + num("height") < 2.0
            {
                return Err(format!("{id}: se pintaria como una raya recta"));
            }
        }
        if let Some(Value::Object(f)) = v.get("files") {
            for (k, x) in f {
                if !foto_legible(x) {
                    return Err(format!("files.{k}"));
                }
            }
        }
        Ok(())
    }

    /// Un lienzo como lo escribe el PC: puntos como listas, una foto con su
    /// ruta y otra de la web solo con `dataURL`, y un tipo que alla no hay.
    const DEL_PC: &str = r##"{"type":"excalidraw","elements":[
        {"id":"t1","type":"freedraw","x":10,"y":20,"width":5,"height":5,"seed":7,
         "opacity":100.0,"points":[[0,0],[2.5,3],[5,5]],"strokeColor":"#e03131"},
        {"id":"f1","type":"image","x":0,"y":0,"width":100,"height":80,"seed":1,"fileId":"img"},
        {"id":"raro","type":"pixpin-tabla-del-futuro","x":0,"y":0,"width":1,"height":1,"seed":1}
      ],
      "files":{"img":{"id":"img","mimeType":"image/png","path":"imagenes/img"},
               "web":{"id":"web","mimeType":"image/png","dataURL":"data:image/png;base64,AAAA"}}}"##;

    #[test]
    fn un_lienzo_del_pc_con_puntos_en_lista_sale_legible_para_el_movil() {
        assert!(
            como_lo_lee_el_movil(DEL_PC).is_err(),
            "sin arreglar, el movil lo tira"
        );
        let arreglado = lienzo_legible(DEL_PC).expect("habia que arreglarlo");
        como_lo_lee_el_movil(&arreglado).expect("el movil lo lee");
        let v: Value = serde_json::from_str(&arreglado).unwrap();
        let t = &v["elements"][0];
        assert_eq!(t["points"][1], serde_json::json!({"x": 2.5, "y": 3}));
        assert_eq!(t["opacity"], 100);
        // Lo dibujado sigue ahi, con su color; lo desconocido se queda fuera.
        assert_eq!(t["strokeColor"], "#e03131");
        assert_eq!(v["elements"].as_array().unwrap().len(), 2);
        // La foto con ruta sigue; la que solo traia `dataURL` se va.
        assert!(v["files"].get("img").is_some());
        assert!(v["files"].get("web").is_none());
    }

    #[test]
    fn un_lienzo_que_el_movil_ya_lee_no_se_toca_ni_un_byte() {
        // Caso negativo: el del movil (puntos como objetos, decimales suyos)
        // no se reescribe, o su resumen cambiaria en cada vuelta.
        let del_movil = r#"{"type":"excalidraw","hoja":{"padre":"x"},"elements":[
          {"id":"a","type":"freedraw","x":1.0000000001,"y":2,"width":3,"height":4,"seed":9,
           "version":3,"points":[{"x":0.0,"y":0.0},{"x":1.5E-7,"y":2.0}]}],
          "files":{"f":{"id":"f","mimeType":"image/jpeg","path":"pixpin:files/pins/draw/files/f"}}}"#;
        assert_eq!(lienzo_legible(del_movil), None);
        // Ni uno al que le falte lo que el `Element` de alla rellena solo.
        let sin_semilla = r#"{"elements":[{"id":"A","type":"rectangle","x":0,"version":1}]}"#;
        assert_eq!(lienzo_legible(sin_semilla), None);
        // Y sin trazos tampoco hay nada que arreglar.
        assert_eq!(
            lienzo_legible(r#"{"type":"excalidraw","elements":[]}"#),
            None
        );
    }

    /// Un trazo como lo escribia el lector del PC (`anot-52U5AB5GUF-p0` de
    /// los datos del usuario): caja a 0, puntos relativos y separados.
    fn trazo_del_pc(puntos_como_objetos: bool) -> String {
        let pts: Vec<(f64, f64)> = vec![
            (0.0, 0.0),
            (-16.8, -8.4),
            (-50.4, -14.9),
            (-113.9, -14.9),
            (-191.3, 4.7),
            (-261.3, 36.4),
            (-393.9, 110.1),
        ];
        let puntos: Vec<String> = pts
            .iter()
            .map(|(x, y)| {
                if puntos_como_objetos {
                    format!(r#"{{"x":{x},"y":{y}}}"#)
                } else {
                    format!("[{x},{y}]")
                }
            })
            .collect();
        format!(
            r#"{{"type":"excalidraw","elements":[{{"id":"pc1","type":"freedraw","x":475.07,"y":1130.93,"width":0.0,"height":0.0,
            "seed":1,"version":87,"opacity":100,"roughness":1,"pressures":[],"simulatePressure":true,
            "strokeOptions":{{"streamline":0.15,"variability":"variable"}},"points":[{}]}}]}}"#,
            puntos.join(",")
        )
    }

    #[test]
    fn un_trazo_del_pc_con_la_caja_a_cero_llega_con_la_de_sus_puntos_y_todos_ellos() {
        for objetos in [false, true] {
            let del_pc = trazo_del_pc(objetos);
            let error = como_lo_lee_el_movil(&del_pc).unwrap_err();
            assert!(objetos || error.contains("Pt"), "{error}");
            let arreglado = lienzo_legible(&del_pc).expect("habia que arreglarlo");
            como_lo_lee_el_movil(&arreglado).expect("el movil lo pinta entero");
            let v: Value = serde_json::from_str(&arreglado).unwrap();
            let e = &v["elements"][0];
            assert_eq!(e["width"].as_f64(), Some(393.9));
            assert!((e["height"].as_f64().unwrap() - 125.0).abs() < 1e-9, "{e}");
            // Los 7 puntos, en su orden y relativos a x/y: nada se junta ni se pierde.
            let p = e["points"].as_array().unwrap();
            assert_eq!(p.len(), 7);
            let xy = |q: &Value| (q["x"].as_f64().unwrap(), q["y"].as_f64().unwrap());
            assert_eq!(xy(&p[0]), (0.0, 0.0));
            assert_eq!(xy(&p[6]), (-393.9, 110.1));
            assert_eq!(
                (e["x"].as_f64(), e["y"].as_f64()),
                (Some(475.07), Some(1130.93))
            );
            assert_eq!(
                lienzo_legible(&arreglado),
                None,
                "una vez arreglado ya no se toca"
            );
        }
    }

    #[test]
    fn la_caja_del_movil_y_la_de_un_punto_suelto_no_se_tocan() {
        // Caso negativo: una caja del movil que no es la exacta (la deja asi
        // al mover un punto suelto) es suya.
        let del_movil = r#"{"elements":[{"id":"a","type":"line","x":0,"y":0,"width":3,"height":0,"seed":2,
          "points":[{"x":0.0,"y":0.0},{"x":40.0,"y":10.0}]}]}"#;
        assert_eq!(lienzo_legible(del_movil), None);
        // Ni un toque (un solo punto) ni un rectangulo de caja cero.
        let punto = r#"{"elements":[{"id":"b","type":"freedraw","x":5,"y":5,"width":0,"height":0,"seed":2,
          "points":[{"x":0.0,"y":0.0}]}]}"#;
        assert_eq!(lienzo_legible(punto), None);
        let rect = r#"{"elements":[{"id":"c","type":"rectangle","x":5,"y":5,"width":0,"height":0,"seed":2}]}"#;
        assert_eq!(lienzo_legible(rect), None);
    }

    #[test]
    fn arreglar_dos_veces_da_lo_mismo_que_una() {
        let una = lienzo_legible(DEL_PC).unwrap();
        assert_eq!(lienzo_legible(&una), None, "lo arreglado ya no se toca");
    }

    #[test]
    fn lo_que_no_es_un_lienzo_se_deja_como_esta() {
        // Casos negativos: ni JSON, ni objeto, ni con `elements`.
        assert_eq!(lienzo_legible("no es json"), None);
        assert_eq!(lienzo_legible("[1,2,3]"), None);
        assert_eq!(lienzo_legible(r#"{"type":"excalidraw"}"#), None);
    }
}
