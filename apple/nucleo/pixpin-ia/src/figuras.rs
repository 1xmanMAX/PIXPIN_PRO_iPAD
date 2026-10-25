//! **La biblioteca de figuras desde el asistente** (`figuras_guardadas`):
//! «guarda esto como arbol isometrico», «pon el arbol isometrico», «¿que
//! figuras tengo?».
//!
//! Es la MISMA biblioteca del panel de figuras del lienzo
//! (`pixpin_motor2d::biblioteca`, fichero `figuras.json` en el formato del
//! movil): lo que guarda el asistente sale en el menu del lienzo y al
//! reves. Esta herramienta solo dice que se pide; la ejecuta la app, que
//! sabe donde vive el fichero y que hay elegido en el lienzo abierto.
//!
//! **Personalizar una plantilla** (25-oct-2026, «quita prioridades, amplia
//! los objetivos a 6 lineas, prioridades a todo el ancho»): la accion
//! «personalizar» manda la lista de secciones en su orden nuevo y la app
//! rehace la plantilla con su mismo codigo (`figuras_de_serie`), en su
//! sitio. Sin lista, devuelve las secciones que tiene para leerlas primero.
//!
//! **El modo noche** (25-oct): «noche»: true/false en alargar o
//! personalizar rehace la plantilla en oscuro (o de dia) en su sitio; sin
//! decirlo, la que se rehace se queda en el modo que tenga.

use serde_json::{Map, Value, json};

use pixpin_motor2d::biblioteca::FiguraGuardada;
use pixpin_motor2d::figuras_de_serie::{
    self as serie, AnchoSeccion, MAX_LINEAS, MAX_SECCIONES, SeccionPedida, TipoSeccion,
};

use crate::herramientas::{ErrorHerramienta, Herramienta};

pub const NOMBRE: &str = "figuras_guardadas";

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

pub fn herramienta() -> Herramienta {
    Herramienta {
        nombre: NOMBRE.into(),
        descripcion: format!(
            "{}{}{}{}{}{}{}{}",
            "La biblioteca de figuras del lienzo (las del usuario y las plantillas de serie). «accion»: guardar (lo elegido en el lienzo, o si no ",
            "lo ultimo que dibujaste, con el «nombre» que diga el usuario o uno corto que le pongas tu), poner (estampa la figura de ese «nombre») ",
            "o lista (las que hay). «alargar»: rehace la PLANTILLA que hay en el lienzo (elegida o no; o la de «nombre») con otro intervalo, mas grande o mas pequeño (tambien para reducir: «déjalo en 10 semanas» es semanas_hasta 10), con el mismo diseño y en su mismo sitio. ",
            "Para AGRANDAR, AMPLIAR o EXTENDER una plantilla (mas semanas, mas filas o actividades, mas horas) usa SIEMPRE «alargar»: nunca la borres ni la redibujes con otras herramientas. ",
            "«30 actividades» (o filas o tareas) es desde 1 hasta 30; «30 semanas» es semanas_desde 1 y semanas_hasta 30: ",
            format_args!(
                "«desde»/«hasta» (del 1 al 40: una hoja mas larga), «de_a» (de 10 en 10: una hoja por cada 10), «semanas_desde»/«semanas_hasta» (cronograma), \
                 «mes» AAAA-MM y «meses» (planificador mensual y habitos con el calendario de verdad). Se alargan: {}. \
                 Ejemplos: {{\"accion\":\"poner\",\"nombre\":\"arbol isometrico\"}}, {{\"accion\":\"alargar\",\"desde\":1,\"hasta\":40,\"de_a\":10}}, \
                 {{\"accion\":\"alargar\",\"nombre\":\"cronograma\",\"desde\":1,\"hasta\":30,\"semanas_desde\":1,\"semanas_hasta\":30}}",
                pixpin_motor2d::figuras_de_serie::las_que_se_alargan()
            ),
            format_args!(
                " «personalizar»: cambia las SECCIONES de una plantilla con su mismo código (quitar una, cambiar sus líneas, reordenarlas, ponerla a todo el ancho, añadir otra); manda solo lo que cambia (si las semanas o las filas se quedan como están, no las pongas), \
                 rehecha en su sitio (Ctrl+Z la devuelve). Sin «secciones» te devuelve las que tiene (léelas primero); con ellas, manda la lista ENTERA en el orden nuevo: \
                 «secciones»: [{{titulo, tipo, lineas, ancho}}] (de una que ya existe basta el titulo y lo que cambie; lo que quites no lo pongas). \
                 tipo: renglones, numerados, casillas, recuadro, libre, notas (zona gris para anotar) o propia (un bloque de esa plantilla); \
                 ancho: completo (fila entera debajo de todo), mitad (de dos en dos, alineadas arriba), izquierda o derecha (apiladas en esa columna), lateral (solo el cronograma: a la derecha de la tabla). \
                 Líneas de 1 a {MAX_LINEAS}, hasta {MAX_SECCIONES} secciones; se puede juntar con desde/hasta/semanas/mes. Se personalizan: {}. \
                 Ejemplo (mensual: quita Prioridades, Objetivo del mes a 6 líneas a todo el ancho arriba, Tareas a 4): \
                 {{\"accion\":\"personalizar\",\"nombre\":\"planificador mensual\",\"secciones\":[{{\"titulo\":\"Objetivo del mes\",\"lineas\":6,\"ancho\":\"completo\"}},{{\"titulo\":\"Tareas\",\"lineas\":4}}]}}; \
                 cronograma de 10 semanas con una zona gris abajo para anotar: \
                 {{\"accion\":\"personalizar\",\"nombre\":\"cronograma\",\"semanas_desde\":1,\"semanas_hasta\":10,\"secciones\":[{{\"titulo\":\"Notas\",\"tipo\":\"notas\",\"lineas\":6,\"ancho\":\"completo\"}}]}} (o \"ancho\":\"lateral\" para tenerla al lado)",
                serie::las_que_se_personalizan()
            ),
            " MODO NOCHE: «noche»:true en alargar o personalizar rehace la plantilla en oscuro en su sitio, con el mismo diseño (\"ponlo en modo noche\":              {\"accion\":\"alargar\",\"noche\":true}, sin más), y «noche»:false la vuelve de día; sin «noche», la que se rehace se queda como está.              Las de serie también vienen de noche: «Cronograma (noche)»."
        ),
        parametros: json!({
            "type": "object",
            "required": ["accion"],
            "properties": {
                "accion": {"type": "string", "enum": ["guardar", "poner", "lista", "alargar", "personalizar"]},
                "nombre": {"type": "string"},
                "desde": {"type": "integer"},
                "hasta": {"type": "integer"},
                "de_a": {"type": "integer"},
                "semanas_desde": {"type": "integer"},
                "semanas_hasta": {"type": "integer"},
                "mes": {"type": "string"},
                "meses": {"type": "integer"},
                "noche": {"type": "boolean"},
                "secciones": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["titulo"],
                        "properties": {
                            "titulo": {"type": "string"},
                            "tipo": {"type": "string", "enum": ["renglones", "numerados", "casillas", "recuadro", "libre", "notas", "propia"]},
                            "lineas": {"type": "integer"},
                            "ancho": {"type": "string", "enum": ["completo", "mitad", "izquierda", "derecha", "lateral"]}
                        }
                    }
                }
            }
        }),
    }
}

/// Lo que se pide a la biblioteca.
#[derive(Debug, Clone, PartialEq)]
pub enum PedidoFiguras {
    Guardar {
        nombre: String,
    },
    Poner {
        nombre: String,
    },
    Lista,
    /// **Una plantilla con otro intervalo** (22-oct): la elegida en el
    /// lienzo, o la de `nombre`.
    Alargar {
        nombre: Option<String>,
        intervalo: pixpin_motor2d::figuras_de_serie::Intervalo,
    },
    /// **Las secciones que tiene una plantilla** (25-oct), para leerlas
    /// antes de personalizarla: la elegida en el lienzo, o la de `nombre`.
    Secciones {
        nombre: Option<String>,
    },
}

/// Un entero del pedido (el modelo a veces lo manda como texto).
fn entero(args: &Map<String, Value>, k: &str) -> Result<Option<u32>, ErrorHerramienta> {
    match args.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .or_else(|| {
                v.as_f64()
                    .filter(|f| *f >= 0.0 && f.fract() == 0.0)
                    .map(|f| f as u64)
            })
            .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| mal(format!("«{k}» tiene que ser un numero entero"))),
    }
}

/// «2026-11», «11/2026» o «noviembre 2026».
pub fn mes_de(s: &str) -> Option<(i32, u32)> {
    const MESES: [&str; 12] = [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
    ];
    let s = crate::acciones::normalizar(s.trim());
    let partes: Vec<&str> = s
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty() && *p != "de" && *p != "del")
        .collect();
    let [a, b] = partes[..] else { return None };
    let numero = |p: &str| p.parse::<i64>().ok();
    let mes = |p: &str| {
        numero(p)
            .filter(|m| (1..=12).contains(m))
            .map(|m| m as u32)
            .or_else(|| {
                MESES
                    .iter()
                    .position(|m| p.starts_with(m))
                    .map(|i| i as u32 + 1)
            })
    };
    match (numero(a), numero(b)) {
        (Some(x), _) if x > 31 => Some((x as i32, mes(b)?)),
        _ => Some((numero(b).filter(|x| *x > 31)? as i32, mes(a)?)),
    }
}

fn intervalo(
    args: &Map<String, Value>,
) -> Result<pixpin_motor2d::figuras_de_serie::Intervalo, ErrorHerramienta> {
    let semanas = match (
        entero(args, "semanas_desde")?,
        entero(args, "semanas_hasta")?,
    ) {
        (None, None) => None,
        (d, h) => Some((d.unwrap_or(1), h.or(d).unwrap_or(12))),
    };
    let mes = match args
        .get("mes")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
    {
        Some(s) => Some(
            mes_de(s)
                .ok_or_else(|| mal(format!("«mes» = «{s}»: escribelo como AAAA-MM (2026-11)")))?,
        ),
        None => None,
    };
    Ok(pixpin_motor2d::figuras_de_serie::Intervalo {
        desde: entero(args, "desde")?,
        hasta: entero(args, "hasta")?,
        de_a: entero(args, "de_a")?,
        semanas,
        mes,
        meses: entero(args, "meses")?,
        secciones: secciones(args)?,
        noche: noche(args)?,
    })
}

/// **El modo pedido** («noche»: true/false). El modelo a veces lo manda
/// como texto («noche», «día», «oscuro»...).
fn noche(args: &Map<String, Value>) -> Result<Option<bool>, ErrorHerramienta> {
    match args.get("noche").or_else(|| args.get("modo")) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(Value::String(s)) => match crate::acciones::normalizar(s.trim()).as_str() {
            "true" | "si" | "noche" | "oscuro" | "oscura" | "dark" => Ok(Some(true)),
            "false" | "no" | "dia" | "claro" | "clara" | "light" => Ok(Some(false)),
            otro => Err(mal(format!(
                "«noche» = «{otro}»: tiene que ser true o false"
            ))),
        },
        Some(_) => Err(mal("«noche» tiene que ser true o false")),
    }
}

/// **Las secciones pedidas**, comprobadas por su forma (lo que depende de
/// la plantilla, como sus bloques propios, lo mira el motor al rehacerla).
fn secciones(args: &Map<String, Value>) -> Result<Option<Vec<SeccionPedida>>, ErrorHerramienta> {
    let v = match args.get("secciones") {
        None | Some(Value::Null) => return Ok(None),
        // A veces llega como texto con el JSON dentro.
        Some(Value::String(s)) => serde_json::from_str::<Value>(s).map_err(|_| {
            mal("«secciones» tiene que ser una lista de {titulo, tipo, lineas, ancho}")
        })?,
        Some(v) => v.clone(),
    };
    let lista = v.as_array().ok_or_else(|| {
        mal("«secciones» tiene que ser una lista de {titulo, tipo, lineas, ancho}")
    })?;
    if lista.is_empty() {
        return Err(mal(
            "«secciones» está vacía: hace falta al menos una sección",
        ));
    }
    if lista.len() > MAX_SECCIONES {
        return Err(mal(format!(
            "son {} secciones; como mucho {MAX_SECCIONES} en una hoja",
            lista.len()
        )));
    }
    lista
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let o = s.as_object().ok_or_else(|| {
                mal(format!("la sección {} tiene que ser un objeto {{titulo, tipo, lineas, ancho}}", i + 1))
            })?;
            let titulo = ["titulo", "nombre", "title"]
                .iter()
                .find_map(|k| o.get(*k).and_then(Value::as_str))
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .ok_or_else(|| mal(format!("la sección {} no tiene «titulo»", i + 1)))?;
            let palabra = |k: &str| o.get(k).and_then(Value::as_str).filter(|p| !p.trim().is_empty());
            let tipo = match palabra("tipo") {
                Some(p) => Some(TipoSeccion::de_palabra(p).ok_or_else(|| {
                    mal(format!(
                        "«{titulo}»: el tipo «{p}» no existe; usa renglones, numerados, casillas, recuadro, libre, notas o propia"
                    ))
                })?),
                None => None,
            };
            let ancho = match palabra("ancho") {
                Some(p) => Some(AnchoSeccion::de_palabra(p).ok_or_else(|| {
                    mal(format!(
                        "«{titulo}»: el ancho «{p}» no existe; usa completo, mitad, izquierda, derecha o lateral"
                    ))
                })?),
                None => None,
            };
            let lineas = match entero(o, "lineas")? {
                Some(n) => Some(n),
                None => entero(o, "filas")?,
            };
            if let Some(n) = lineas.filter(|n| *n == 0 || *n > MAX_LINEAS) {
                return Err(mal(format!(
                    "«{titulo}»: las líneas van de 1 a {MAX_LINEAS} (pediste {n})"
                )));
            }
            Ok(SeccionPedida {
                titulo: titulo.chars().take(80).collect(),
                tipo,
                lineas,
                ancho,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// **Las secciones de la plantilla que se llama asi**, en palabras para el
/// modelo. Error si no hay ninguna con ese nombre o no se personaliza.
pub fn secciones_de_nombre(nombre: &str) -> Result<String, String> {
    let catalogo = serie::catalogo();
    let clave = buscar(&catalogo, nombre)
        .and_then(serie::clave_de)
        .ok_or_else(|| {
            format!(
                "no hay ninguna plantilla llamada «{nombre}»; se personalizan: {}",
                serie::las_que_se_personalizan()
            )
        })?;
    serie::guia_de(clave).ok_or_else(|| {
        format!(
            "«{}» no tiene secciones que cambiar; se personalizan: {}",
            serie::nombre_de(clave),
            serie::las_que_se_personalizan()
        )
    })
}

pub fn validar(args: &Map<String, Value>) -> Result<PedidoFiguras, ErrorHerramienta> {
    let nombre = ["nombre", "figura", "como"]
        .iter()
        .find_map(|k| args.get(*k).and_then(Value::as_str))
        .map(|s| {
            s.trim()
                .trim_matches(['«', '»', '"', '\''])
                .trim()
                .chars()
                .take(80)
                .collect::<String>()
        })
        .filter(|s| !s.is_empty());
    match args
        .get("accion")
        .and_then(Value::as_str)
        .map(crate::acciones::normalizar)
        .as_deref()
    {
        Some("guardar" | "guarda" | "salvar" | "save") => Ok(PedidoFiguras::Guardar {
            nombre: nombre
                .ok_or_else(|| mal("para guardar hace falta el «nombre» de la figura"))?,
        }),
        Some("poner" | "pon" | "insertar" | "estampar" | "usar" | "put" | "insert") => {
            Ok(PedidoFiguras::Poner {
                nombre: nombre.ok_or_else(|| mal("falta el «nombre» de la figura que poner"))?,
            })
        }
        Some("lista" | "listar" | "ver" | "cuales" | "list") => Ok(PedidoFiguras::Lista),
        Some(
            "alargar" | "intervalo" | "ampliar" | "extender" | "continuar" | "dividir" | "partir",
        ) => Ok(PedidoFiguras::Alargar {
            nombre,
            intervalo: intervalo(args)?,
        }),
        // Con la lista, se rehace; sin ella, se devuelven las que tiene.
        Some(
            "personalizar" | "personaliza" | "secciones" | "ver secciones" | "ver_secciones"
            | "reordenar" | "cambiar secciones" | "cambiar_secciones",
        ) => {
            let intervalo = intervalo(args)?;
            // Con el modo («ponlo en modo noche») tambien se rehace.
            Ok(
                if intervalo.secciones.is_some() || intervalo.noche.is_some() {
                    PedidoFiguras::Alargar { nombre, intervalo }
                } else {
                    PedidoFiguras::Secciones { nombre }
                },
            )
        }
        None if nombre.is_none() => Ok(PedidoFiguras::Lista),
        Some(otra) => Err(mal(format!(
            "«accion» = «{otra}» no existe; usa guardar, poner, lista, alargar o personalizar"
        ))),
        None => Err(mal(
            "falta «accion»: guardar, poner, lista, alargar o personalizar",
        )),
    }
}

/// **La figura que mejor encaja con el nombre pedido**: igual sin tildes ni
/// mayusculas; si no, la que contiene todas las palabras pedidas; si no, la
/// que mas comparte. `None` si no se parece ninguna.
pub fn buscar<'a>(figuras: &'a [FiguraGuardada], pedido: &str) -> Option<&'a FiguraGuardada> {
    let q = crate::acciones::normalizar(pedido.trim());
    let palabras = |s: &str| -> Vec<String> {
        crate::acciones::normalizar(s)
            .split(|c: char| !c.is_alphanumeric())
            .filter(|p| p.len() > 2 || p.chars().all(|c| c.is_ascii_digit()))
            // El plural no cuenta: «arboles» es «arbol».
            .map(|p| match p.strip_suffix("es").filter(|r| r.len() > 3) {
                Some(r) => r.to_string(),
                None => p.strip_suffix('s').unwrap_or(p).to_string(),
            })
            .filter(|p| !p.is_empty())
            .collect()
    };
    if let Some(f) = figuras
        .iter()
        .find(|f| crate::acciones::normalizar(&f.nombre) == q)
    {
        return Some(f);
    }
    let pq = palabras(pedido);
    if pq.is_empty() {
        return None;
    }
    let comun = |f: &FiguraGuardada| {
        let pf = palabras(&f.nombre);
        pq.iter().filter(|p| pf.contains(p)).count()
    };
    if let Some(f) = figuras.iter().find(|f| comun(f) == pq.len()) {
        return Some(f);
    }
    // A igual parecido, la de nombre mas corto y la primera: «cronograma
    // semanas» es el de dia y no su gemela «(noche)».
    figuras
        .iter()
        .filter(|f| comun(f) > 0)
        .min_by_key(|f| (std::cmp::Reverse(comun(f)), palabras(&f.nombre).len()))
}

/// La lista para el modelo (y el usuario).
pub fn lista(figuras: &[FiguraGuardada]) -> String {
    if figuras.is_empty() {
        return "no hay figuras guardadas todavia".into();
    }
    let nombres: Vec<&str> = figuras.iter().map(|f| f.nombre.as_str()).collect();
    format!(
        "{} figuras guardadas: {}",
        figuras.len(),
        nombres.join(", ")
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::Elemento;

    fn f(nombre: &str) -> FiguraGuardada {
        FiguraGuardada {
            id: nombre.into(),
            nombre: nombre.into(),
            elementos: vec![Elemento::default()],
        }
    }

    #[test]
    fn se_entienden_las_tres_acciones() {
        let v = |j: Value| validar(j.as_object().unwrap());
        assert_eq!(
            v(json!({"accion": "guardar", "nombre": "«Árbol isométrico»"})).unwrap(),
            PedidoFiguras::Guardar {
                nombre: "Árbol isométrico".into()
            }
        );
        assert_eq!(
            v(json!({"accion": "pon", "nombre": "arbol"})).unwrap(),
            PedidoFiguras::Poner {
                nombre: "arbol".into()
            }
        );
        assert_eq!(v(json!({"accion": "lista"})).unwrap(), PedidoFiguras::Lista);
        assert_eq!(v(json!({})).unwrap(), PedidoFiguras::Lista);
        // Casos negativos.
        assert!(v(json!({"accion": "guardar"})).is_err());
        assert!(v(json!({"accion": "poner", "nombre": "  "})).is_err());
        assert!(v(json!({"accion": "borrar_todo", "nombre": "x"})).is_err());
        assert!(v(json!({"nombre": "x"})).is_err());
    }

    #[test]
    fn busca_por_nombre_aunque_no_sea_exacto() {
        let l = vec![
            f("Árbol isométrico"),
            f("Casa"),
            f("Arbol de navidad"),
            f("Norte"),
        ];
        assert_eq!(
            buscar(&l, "arbol isometrico").unwrap().nombre,
            "Árbol isométrico"
        );
        assert_eq!(
            buscar(&l, "el árbol isométrico").unwrap().nombre,
            "Árbol isométrico"
        );
        assert_eq!(buscar(&l, "CASA").unwrap().nombre, "Casa");
        assert_eq!(
            buscar(&l, "arboles de navidad").unwrap().nombre,
            "Arbol de navidad"
        );
        assert!(buscar(&l, "perro").is_none());
        assert!(buscar(&[], "casa").is_none());
        assert!(lista(&l).contains("4 figuras"));
        assert!(lista(&[]).contains("no hay"));
    }

    #[test]
    fn se_entiende_alargar_una_plantilla() {
        use pixpin_motor2d::figuras_de_serie::Intervalo;
        let v = |j: Value| validar(j.as_object().unwrap());
        assert_eq!(
            v(json!({"accion": "alargar", "desde": 1, "hasta": "40", "de_a": 10.0})).unwrap(),
            PedidoFiguras::Alargar {
                nombre: None,
                intervalo: Intervalo {
                    desde: Some(1),
                    hasta: Some(40),
                    de_a: Some(10),
                    ..Default::default()
                }
            }
        );
        let PedidoFiguras::Alargar { nombre, intervalo } =
            v(json!({"accion": "alargar", "nombre": "cronograma", "semanas_desde": 13, "semanas_hasta": 24})).unwrap()
        else {
            panic!()
        };
        assert_eq!(nombre.as_deref(), Some("cronograma"));
        assert_eq!(intervalo.semanas, Some((13, 24)));
        for (s, esperado) in [
            ("2026-11", (2026, 11)),
            ("11/2026", (2026, 11)),
            ("noviembre 2026", (2026, 11)),
            ("Enero de 2027", (2027, 1)),
        ] {
            let PedidoFiguras::Alargar { intervalo, .. } =
                v(json!({"accion": "alargar", "mes": s, "meses": 3})).unwrap()
            else {
                panic!()
            };
            assert_eq!(intervalo.mes, Some(esperado), "{s}");
            assert_eq!(intervalo.meses, Some(3));
        }
        assert!(v(json!({"accion": "alargar", "mes": "pronto"})).is_err());
        assert!(v(json!({"accion": "alargar", "desde": -3})).is_err());
        assert!(herramienta().descripcion.contains("Cronograma"));
    }

    #[test]
    fn se_entiende_el_modo_noche() {
        let v = |j: Value| validar(j.as_object().unwrap());
        // «Ponlo en modo noche»: solo el modo, en alargar o en personalizar.
        for accion in ["alargar", "personalizar"] {
            let PedidoFiguras::Alargar { intervalo, .. } =
                v(json!({"accion": accion, "noche": true})).unwrap()
            else {
                panic!("{accion}")
            };
            assert_eq!(intervalo.noche, Some(true));
            assert!(intervalo.desde.is_none() && intervalo.secciones.is_none());
        }
        let PedidoFiguras::Alargar { intervalo, .. } =
            v(json!({"accion": "alargar", "desde": 1, "hasta": 30, "noche": "día"})).unwrap()
        else {
            panic!()
        };
        assert_eq!(intervalo.noche, Some(false));
        // Sin decirlo, como este.
        let PedidoFiguras::Alargar { intervalo, .. } =
            v(json!({"accion": "alargar", "hasta": 30})).unwrap()
        else {
            panic!()
        };
        assert_eq!(intervalo.noche, None);
        // Personalizar sin secciones ni modo sigue siendo ver las secciones.
        assert!(matches!(
            v(json!({"accion": "personalizar"})).unwrap(),
            PedidoFiguras::Secciones { .. }
        ));
        // Caso negativo.
        assert!(v(json!({"accion": "alargar", "noche": "a ratos"})).is_err());
        assert!(v(json!({"accion": "alargar", "noche": 3})).is_err());
        assert!(herramienta().descripcion.contains("MODO NOCHE"));
        // «cronograma» es el de dia; «cronograma noche», su gemela.
        let c = serie::catalogo();
        assert_eq!(buscar(&c, "cronograma").unwrap().id, "serie-cronograma");
        assert_eq!(
            buscar(&c, "el cronograma de las semanas").unwrap().id,
            "serie-cronograma"
        );
        assert_eq!(
            buscar(&c, "cronograma noche").unwrap().id,
            "serie-cronograma-noche"
        );
    }

    #[test]
    fn se_entiende_personalizar_y_ver_las_secciones() {
        let v = |j: Value| validar(j.as_object().unwrap());
        // El caso del usuario en el mensual.
        let PedidoFiguras::Alargar { nombre, intervalo } = v(json!({
            "accion": "personalizar",
            "nombre": "planificador mensual",
            "secciones": [
                {"titulo": "Objetivo del mes", "lineas": 6, "ancho": "completo"},
                {"titulo": "Tareas", "lineas": "4"}
            ]
        }))
        .unwrap() else {
            panic!()
        };
        assert_eq!(nombre.as_deref(), Some("planificador mensual"));
        let s = intervalo.secciones.unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].ancho, Some(AnchoSeccion::Completo));
        assert_eq!(s[1].lineas, Some(4));
        assert_eq!(s[1].tipo, None);
        // El cronograma con su zona gris, como texto con el JSON dentro.
        let PedidoFiguras::Alargar { intervalo, .. } = v(json!({
            "accion": "personalizar",
            "semanas_desde": 1, "semanas_hasta": 10,
            "secciones": "[{\"titulo\":\"Notas\",\"tipo\":\"notas\",\"lineas\":6,\"ancho\":\"lateral\"}]"
        }))
        .unwrap() else {
            panic!()
        };
        assert_eq!(intervalo.semanas, Some((1, 10)));
        let s = intervalo.secciones.unwrap();
        assert_eq!(s[0].tipo, Some(TipoSeccion::Notas));
        assert_eq!(s[0].ancho, Some(AnchoSeccion::Lateral));
        // Sin lista: ver las que tiene.
        assert_eq!(
            v(json!({"accion": "personalizar", "nombre": "metas"})).unwrap(),
            PedidoFiguras::Secciones {
                nombre: Some("metas".into())
            }
        );
        let t = secciones_de_nombre("planificador mensual").unwrap();
        assert!(t.contains("Prioridades") && t.contains("calendario"), "{t}");
        // Casos negativos, en palabras.
        let e = |j: Value| match v(j).unwrap_err() {
            ErrorHerramienta::Parametro(m) => m,
            otro => panic!("{otro:?}"),
        };
        assert!(e(json!({"accion": "personalizar", "secciones": []})).contains("vacía"));
        assert!(
            e(json!({"accion": "personalizar", "secciones": [{"lineas": 3}]})).contains("titulo")
        );
        assert!(
            e(json!({"accion": "personalizar", "secciones": [{"titulo": "A", "lineas": 0}]}))
                .contains("de 1 a")
        );
        assert!(
            e(json!({"accion": "personalizar", "secciones": [{"titulo": "A", "lineas": 99}]}))
                .contains("de 1 a")
        );
        assert!(
            e(json!({"accion": "personalizar", "secciones": [{"titulo": "A", "tipo": "tarta"}]}))
                .contains("no existe")
        );
        assert!(e(json!({"accion": "personalizar", "secciones": [{"titulo": "A", "ancho": "diagonal"}]})).contains("no existe"));
        let trece: Vec<Value> = (0..13)
            .map(|i| json!({"titulo": format!("S{i}")}))
            .collect();
        assert!(e(json!({"accion": "personalizar", "secciones": trece})).contains("como mucho"));
        assert!(e(json!({"accion": "personalizar", "secciones": "no es json"})).contains("lista"));
        assert!(secciones_de_nombre("lista de tareas").is_err());
        assert!(herramienta().descripcion.contains("personalizar"));
    }
}
