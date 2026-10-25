//! **Cronogramas** (`cronograma`): un diagrama de Gantt limpio en el lienzo
//! —escala de tiempo, una fila por tarea con su barra, hitos como rombos y
//! flechas de dependencia— hecho de figuras normales (rectangulos, rombos,
//! lineas, flechas y textos) en un grupo: se mueve de una vez, se edita
//! pieza a pieza y viaja al movil como cualquier dibujo.
//!
//! No es el elemento `pixpin-gantt` del movil: ese se quito del lienzo del
//! PC por malo (ver `elemento.rs`). Aqui el cronograma es dibujo, no un
//! objeto con datos, y por eso se ve igual en todas partes.
//!
//! Las fechas las cuenta Rust: el modelo da inicio y fin (dias, semanas o
//! fechas `AAAA-MM-DD`) y aqui se comprueba que no esten al reves, que las
//! dependencias existan y no hagan un circulo.

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use crate::herramientas::{ErrorHerramienta, Herramienta};
use crate::lienzo::Fabrica;

pub const NOMBRE: &str = "cronograma";
/// Cuantas tareas como mucho.
pub const MAX_TAREAS: usize = 40;
/// Lo mas largo que puede ser un cronograma (en dias o periodos).
pub const MAX_PERIODOS: i64 = 3700;

/// La paleta de las barras (la del lienzo).
const COLORES: [&str; 8] = ["#1971c2", "#2f9e44", "#f08c00", "#6741d9", "#0c8599", "#c2255c", "#e03131", "#868e96"];
const GRIS: &str = "#868e96";
const NEGRO: &str = "#1e1e1e";

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

pub fn herramienta() -> Herramienta {
    Herramienta {
        nombre: NOMBRE.into(),
        descripcion: concat!(
            "Dibuja en el lienzo un cronograma (diagrama de Gantt): escala de tiempo, una barra por tarea, hitos y flechas de dependencia. ",
            "«inicio» y «fin» como fechas AAAA-MM-DD o como numeros de dia/semana; un hito lleva «hito»:true (o fin igual a inicio). ",
            "Ejemplo: {\"titulo\":\"Obra\",\"tareas\":[{\"nombre\":\"Excavacion\",\"inicio\":\"2026-11-02\",\"fin\":\"2026-11-06\"},",
            "{\"nombre\":\"Cimientos\",\"inicio\":\"2026-11-09\",\"fin\":\"2026-11-20\",\"depende_de\":[\"Excavacion\"]},",
            "{\"nombre\":\"Fin de cimientos\",\"inicio\":\"2026-11-20\",\"hito\":true,\"depende_de\":[\"Cimientos\"]}]}"
        )
        .into(),
        parametros: json!({
            "type": "object",
            "required": ["tareas"],
            "properties": {
                "titulo": {"type": "string"},
                "unidad": {"type": "string", "enum": ["dias", "semanas", "meses"], "description": "si inicio y fin son numeros"},
                "tareas": {"type": "array", "maxItems": MAX_TAREAS, "items": {
                    "type": "object",
                    "required": ["nombre", "inicio"],
                    "properties": {
                        "nombre": {"type": "string"},
                        "inicio": {"type": ["string", "number"]},
                        "fin": {"type": ["string", "number"]},
                        "duracion": {"type": "number"},
                        "depende_de": {"type": "array", "items": {"type": ["string", "number"]}},
                        "hito": {"type": "boolean"},
                        "color": {"type": "string"}
                    }
                }}
            }
        }),
    }
}

/// Una tarea, en periodos (dias desde 1970 si eran fechas): `[inicio, fin)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Tarea {
    pub nombre: String,
    pub inicio: i64,
    pub fin: i64,
    pub hito: bool,
    /// Indices de las tareas de las que depende.
    pub depende: Vec<usize>,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cronograma {
    pub titulo: String,
    pub tareas: Vec<Tarea>,
    /// Si los periodos son dias de calendario (para rotular fechas).
    pub fechas: bool,
    /// `dias`, `semanas` o `meses` (si no son fechas).
    pub unidad: &'static str,
}

/// Dias desde 1970-01-01 (Howard Hinnant, `days_from_civil`).
pub fn dias_de(a: i64, m: i64, d: i64) -> i64 {
    let a = if m <= 2 { a - 1 } else { a };
    let era = if a >= 0 { a } else { a - 399 } / 400;
    let ae = a - era * 400;
    let dm = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let de = ae * 365 + ae / 4 - ae / 100 + dm;
    era * 146_097 + de - 719_468
}

/// Y al reves: (ano, mes, dia).
pub fn fecha_de(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let de = z - era * 146_097;
    let ae = (de - de / 1460 + de / 36_524 - de / 146_096) / 365;
    let a = ae + era * 400;
    let da = de - (365 * ae + ae / 4 - ae / 100);
    let mp = (5 * da + 2) / 153;
    let d = da - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { a + 1 } else { a }, m, d)
}

/// `2026-11-02`, `2026/11/02` o `02/11/2026`.
fn fecha(t: &str) -> Option<i64> {
    let p: Vec<i64> = t.trim().split(['-', '/', '.']).map(|s| s.trim().parse().ok()).collect::<Option<_>>()?;
    let (a, m, d) = match p.as_slice() {
        [a, m, d] if *a > 999 => (*a, *m, *d),
        [d, m, a] if *a > 999 => (*a, *m, *d),
        _ => return None,
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || !(1900..=2200).contains(&a) {
        return None;
    }
    let dias = dias_de(a, m, d);
    // 31 de febrero: no existe.
    (fecha_de(dias) == (a, m, d)).then_some(dias)
}

enum Momento {
    Fecha(i64),
    Numero(i64),
}

fn momento(v: &Value, donde: &str, campo: &str) -> Result<Option<Momento>, ErrorHerramienta> {
    match v {
        Value::Null => Ok(None),
        Value::Number(n) => {
            let x = n.as_f64().filter(|x| x.is_finite() && x.abs() <= 1e5).ok_or_else(|| mal(format!("{donde}: «{campo}» fuera de rango")))?;
            Ok(Some(Momento::Numero(x.round() as i64)))
        }
        Value::String(s) => {
            if let Ok(x) = s.trim().parse::<f64>() {
                return momento(&json!(x), donde, campo);
            }
            fecha(s)
                .map(|d| Some(Momento::Fecha(d)))
                .ok_or_else(|| mal(format!("{donde}: «{campo}» = «{s}» no es una fecha (AAAA-MM-DD) ni un numero")))
        }
        _ => Err(mal(format!("{donde}: «{campo}» tiene que ser una fecha o un numero"))),
    }
}

pub fn validar(args: &Map<String, Value>, avisos: &mut Vec<String>) -> Result<Cronograma, ErrorHerramienta> {
    let lista = match args.get("tareas") {
        Some(Value::Array(l)) => l,
        _ => return Err(mal("falta «tareas» (una lista con nombre, inicio y fin)")),
    };
    if lista.is_empty() {
        return Err(mal("«tareas» esta vacia"));
    }
    if lista.len() > MAX_TAREAS {
        return Err(mal(format!("como mucho {MAX_TAREAS} tareas; agrupa las pequenas")));
    }
    let mut fechas: Option<bool> = None;
    let mut tareas = Vec::new();
    let mut deps_crudas: Vec<Vec<Value>> = Vec::new();
    for (i, v) in lista.iter().enumerate() {
        let donde = format!("tarea {}", i + 1);
        let Value::Object(o) = v else { return Err(mal(format!("{donde}: tiene que ser un objeto"))) };
        let nombre: String = ["nombre", "tarea", "texto", "name"]
            .iter()
            .find_map(|k| o.get(*k).and_then(Value::as_str))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| mal(format!("{donde}: falta «nombre»")))?
            .chars()
            .take(80)
            .collect();
        let inicio = momento(o.get("inicio").unwrap_or(&Value::Null), &donde, "inicio")?
            .ok_or_else(|| mal(format!("{donde} («{nombre}»): falta «inicio»")))?;
        let fin = momento(o.get("fin").unwrap_or(&Value::Null), &donde, "fin")?;
        let es_fecha = matches!(inicio, Momento::Fecha(_));
        if fin.as_ref().is_some_and(|f| matches!(f, Momento::Fecha(_)) != es_fecha) {
            return Err(mal(format!("{donde}: «inicio» y «fin» tienen que ser los dos fechas o los dos numeros")));
        }
        match fechas {
            None => fechas = Some(es_fecha),
            Some(f) if f != es_fecha => {
                return Err(mal("todas las tareas tienen que usar fechas, o todas numeros".to_string()));
            }
            _ => {}
        }
        let a = match inicio {
            Momento::Fecha(d) | Momento::Numero(d) => d,
        };
        let hito_pedido = o.get("hito").and_then(Value::as_bool).unwrap_or(false);
        let duracion = o.get("duracion").and_then(Value::as_f64).filter(|d| d.is_finite());
        // Las fechas cuentan el ultimo dia entero: del 2 al 6 son 5 dias.
        let b = match (fin, duracion) {
            (Some(Momento::Fecha(d)), _) => d + 1,
            (Some(Momento::Numero(d)), _) => d,
            (None, Some(d)) if d >= 0.0 && d <= MAX_PERIODOS as f64 => a + d.round() as i64,
            (None, Some(d)) => return Err(mal(format!("{donde}: «duracion» = {d} no vale"))),
            (None, None) if hito_pedido => a,
            (None, None) => return Err(mal(format!("{donde} («{nombre}»): falta «fin» (o «duracion», o «hito»:true)"))),
        };
        let (b, hito) = if hito_pedido {
            (a, true)
        } else if b < a || (es_fecha && b <= a) {
            return Err(mal(format!(
                "{donde} («{nombre}»): las fechas estan al reves: termina antes de empezar"
            )));
        } else if b == a {
            avisos.push(format!("{donde} («{nombre}»): dura cero, se pone como hito"));
            (a, true)
        } else {
            (b, false)
        };
        let color = o
            .get("color")
            .and_then(Value::as_str)
            .and_then(crate::herramientas::color)
            .map_or_else(|| COLORES[i % COLORES.len()].to_string(), |c| c.0);
        deps_crudas.push(match o.get("depende_de").or(o.get("dependencias")).or(o.get("despues_de")) {
            Some(Value::Array(l)) => l.clone(),
            Some(Value::Null) | None => Vec::new(),
            Some(otro) => vec![otro.clone()],
        });
        tareas.push(Tarea { nombre, inicio: a, fin: b, hito, depende: Vec::new(), color });
    }
    // Las dependencias, por nombre (sin mayusculas) o por numero (de 1).
    let por_nombre: HashMap<String, usize> = tareas.iter().enumerate().map(|(i, t)| (t.nombre.to_lowercase(), i)).collect();
    for (i, deps) in deps_crudas.iter().enumerate() {
        for d in deps {
            let j = match d {
                Value::Number(n) => n.as_u64().map(|n| n as usize).filter(|n| (1..=tareas.len()).contains(n)).map(|n| n - 1),
                Value::String(s) => por_nombre.get(&s.trim().to_lowercase()).copied().or_else(|| {
                    s.trim().parse::<usize>().ok().filter(|n| (1..=tareas.len()).contains(n)).map(|n| n - 1)
                }),
                _ => None,
            }
            .ok_or_else(|| mal(format!("tarea {} («{}»): depende de {d}, que no es ninguna tarea", i + 1, tareas[i].nombre)))?;
            if j == i {
                return Err(mal(format!("tarea {} («{}») depende de si misma", i + 1, tareas[i].nombre)));
            }
            if !tareas[i].depende.contains(&j) {
                tareas[i].depende.push(j);
            }
        }
    }
    // Sin circulos (Kahn).
    let n = tareas.len();
    let mut entrantes: Vec<usize> = tareas.iter().map(|t| t.depende.len()).collect();
    let mut listas: Vec<usize> = (0..n).filter(|i| entrantes[*i] == 0).collect();
    let mut vistas = 0;
    while let Some(k) = listas.pop() {
        vistas += 1;
        for (i, t) in tareas.iter().enumerate() {
            if t.depende.contains(&k) {
                entrantes[i] -= 1;
                if entrantes[i] == 0 {
                    listas.push(i);
                }
            }
        }
    }
    if vistas < n {
        return Err(mal("las dependencias hacen un circulo (A depende de B y B de A)"));
    }
    for t in &tareas {
        for &j in &t.depende {
            if t.inicio < tareas[j].fin && !tareas[j].hito || t.inicio < tareas[j].inicio {
                avisos.push(format!("«{}» empieza antes de que acabe «{}», de la que depende", t.nombre, tareas[j].nombre));
            }
        }
    }
    let desde = tareas.iter().map(|t| t.inicio).min().unwrap_or(0);
    let hasta = tareas.iter().map(|t| t.fin).max().unwrap_or(0);
    if hasta - desde > MAX_PERIODOS {
        return Err(mal(format!("el cronograma dura {} periodos; como mucho {MAX_PERIODOS}", hasta - desde)));
    }
    let unidad = match args.get("unidad").and_then(Value::as_str).map(crate::acciones::normalizar).as_deref() {
        Some("semanas" | "semana") => "semanas",
        Some("meses" | "mes") => "meses",
        _ => "dias",
    };
    let titulo = args.get("titulo").and_then(Value::as_str).unwrap_or("").trim().chars().take(120).collect();
    Ok(Cronograma { titulo, tareas, fechas: fechas.unwrap_or(false), unidad })
}

const MESES: [&str; 12] = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"];

/// Las marcas de la escala: (periodo, rotulo).
fn marcas(c: &Cronograma, desde: i64, hasta: i64) -> Vec<(i64, String)> {
    let largo = (hasta - desde).max(1);
    if c.fechas {
        let mut v = Vec::new();
        if largo <= 21 {
            for d in desde..=hasta {
                let (_, m, dd) = fecha_de(d);
                v.push((d, format!("{dd}/{m}")));
            }
        } else if largo <= 120 {
            // Los lunes (el 1970-01-01 fue jueves).
            let mut d = desde + (7 - (desde + 3).rem_euclid(7)) % 7;
            while d <= hasta {
                let (_, m, dd) = fecha_de(d);
                v.push((d, format!("{dd}/{m}")));
                d += 7;
            }
        } else {
            let (mut a, mut m, _) = fecha_de(desde);
            loop {
                let d = dias_de(a, m, 1);
                if d > hasta {
                    break;
                }
                if d >= desde {
                    let rot = if m == 1 || v.is_empty() { format!("{} {a}", MESES[(m - 1) as usize]) } else { MESES[(m - 1) as usize].to_string() };
                    v.push((d, rot));
                }
                m += 1;
                if m > 12 {
                    m = 1;
                    a += 1;
                }
            }
        }
        v
    } else {
        let paso = pixpin_motor2d::grafica::paso_bonito(largo as f64).max(1.0) as i64;
        let letra = match c.unidad {
            "semanas" => "S",
            "meses" => "M",
            _ => "D",
        };
        let mut p = desde - desde.rem_euclid(paso);
        let mut v = Vec::new();
        while p <= hasta {
            if p >= desde {
                v.push((p, format!("{letra}{p}")));
            }
            p += paso;
        }
        v
    }
}

/// **El Gantt** en el lienzo de 800x600.
pub(crate) fn elementos(f: &mut Fabrica, c: &Cronograma) {
    let antes = f.grupo.clone();
    f.grupo = Some(f.nombre_de_grupo("cronograma"));
    let desde = c.tareas.iter().map(|t| t.inicio).min().unwrap_or(0);
    let hasta = c.tareas.iter().map(|t| t.fin + i64::from(t.hito && c.fechas)).max().unwrap_or(1).max(desde + 1);
    let (x0, x1) = (210.0, 780.0);
    let arriba = if c.titulo.is_empty() { 20.0 } else { 64.0 };
    let y_escala = arriba;
    let y_filas = arriba + 34.0;
    let n = c.tareas.len() as f64;
    let fila = ((580.0 - y_filas) / n).clamp(18.0, 40.0);
    let x_de = |p: i64| x0 + (p - desde) as f64 / (hasta - desde) as f64 * (x1 - x0);
    let liso = |f: &mut Fabrica| {
        if let Some(e) = f.salida.last_mut() {
            e["roughness"] = json!(0);
            if e["type"] == "text" {
                e["fontFamily"] = json!(2);
            }
        }
    };
    if !c.titulo.is_empty() {
        f.texto(400.0, 14.0, &c.titulo, 26.0, NEGRO, "center");
        liso(f);
    }
    let y_fin = y_filas + fila * n;
    // Las bandas de las filas, una si y otra no.
    for i in (0..c.tareas.len()).step_by(2) {
        f.caja("rectangle", [20.0, y_filas + fila * i as f64, x1 - 20.0, fila], "#f1f3f5", Some("#f1f3f5"));
        liso(f);
        if let Some(e) = f.salida.last_mut() {
            e["strokeWidth"] = json!(1);
        }
    }
    // La escala: rayas finas y sus rotulos.
    let ms = marcas(c, desde, hasta);
    let cada = ((ms.len() as f64 * 46.0 / (x1 - x0)).ceil() as usize).max(1);
    for (k, (p, rot)) in ms.iter().enumerate() {
        let x = x_de(*p);
        f.trazo("line", &[(x, y_escala + 22.0), (x, y_fin)], "#ced4da", None);
        liso(f);
        if let Some(e) = f.salida.last_mut() {
            e["strokeWidth"] = json!(1);
        }
        if k % cada == 0 {
            f.texto(x, y_escala, rot, 12.0, GRIS, "center");
            liso(f);
        }
    }
    f.trazo("line", &[(x0, y_escala + 22.0), (x1, y_escala + 22.0)], GRIS, None);
    liso(f);
    // Las tareas: su nombre a la izquierda y su barra (o su rombo).
    let mut extremos: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for (i, t) in c.tareas.iter().enumerate() {
        let y = y_filas + fila * i as f64;
        let medio = y + fila / 2.0;
        let tam = (fila * 0.42).clamp(10.0, 15.0);
        let maximo = ((x0 - 34.0) / (tam * 0.6)) as usize;
        let rotulo: String = if t.nombre.chars().count() > maximo {
            t.nombre.chars().take(maximo.saturating_sub(1)).collect::<String>() + "…"
        } else {
            t.nombre.clone()
        };
        f.texto(28.0, medio - tam * 0.62, &rotulo, tam, NEGRO, "left");
        liso(f);
        if t.hito {
            // Un hito con fecha cae al final de su dia (cuando se entrega).
            let x = x_de(t.inicio + i64::from(c.fechas));
            let lado = (fila * 0.6).min(18.0);
            f.caja("diamond", [x - lado / 2.0, medio - lado / 2.0, lado, lado], NEGRO, Some(NEGRO));
            liso(f);
            extremos.push(((x - lado / 2.0, medio), (x + lado / 2.0, medio)));
        } else {
            let (xa, xb) = (x_de(t.inicio), x_de(t.fin));
            let alto = fila * 0.56;
            f.caja("rectangle", [xa, medio - alto / 2.0, (xb - xa).max(3.0), alto], &t.color, Some(&t.color));
            liso(f);
            if let Some(e) = f.salida.last_mut() {
                e["roundness"] = json!({"type": 3});
                e["strokeWidth"] = json!(1);
            }
            extremos.push(((xa, medio), (xb, medio)));
        }
    }
    // Las dependencias: del final de una al principio de la otra, en codo.
    for (i, t) in c.tareas.iter().enumerate() {
        for &j in &t.depende {
            let (_, a) = extremos[j];
            let (b, _) = extremos[i];
            let codo = a.0 + 8.0;
            let puntos = if t.hito && (b.0 - a.0).abs() < 24.0 {
                // Un hito justo al acabar lo anterior: una flecha que baja.
                let x = (extremos[i].0.0 + extremos[i].1.0) / 2.0;
                vec![(x, a.1 + fila * 0.3), (x, b.1 - fila * 0.3)]
            } else if b.0 - a.0 >= 16.0 {
                vec![a, (codo, a.1), (codo, b.1), (b.0 - 1.0, b.1)]
            } else {
                // La siguiente empieza antes: se baja por detras.
                let y_pasa = (a.1 + b.1) / 2.0;
                vec![a, (a.0 + 8.0, a.1), (a.0 + 8.0, y_pasa), (b.0 - 10.0, y_pasa), (b.0 - 10.0, b.1), (b.0 - 1.0, b.1)]
            };
            f.trazo("arrow", &puntos, "#495057", None);
            liso(f);
            if let Some(e) = f.salida.last_mut() {
                e["strokeWidth"] = json!(1.5);
                e["endArrowhead"] = json!("triangle");
            }
        }
    }
    f.grupo = antes;
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn validar_(v: Value) -> Result<Cronograma, ErrorHerramienta> {
        let Value::Object(m) = v else { panic!() };
        validar(&m, &mut Vec::new())
    }

    #[test]
    fn las_fechas_van_y_vuelven() {
        assert_eq!(dias_de(1970, 1, 1), 0);
        assert_eq!(fecha_de(dias_de(2026, 11, 2)), (2026, 11, 2));
        assert_eq!(fecha("2026-02-31"), None);
        assert_eq!(fecha("02/11/2026"), fecha("2026-11-02"));
    }

    #[test]
    fn un_gantt_de_obra_sale_y_lo_lee_el_lienzo() {
        let c = validar_(json!({"titulo": "Obra", "tareas": [
            {"nombre": "Excavacion", "inicio": "2026-11-02", "fin": "2026-11-06"},
            {"nombre": "Cimientos", "inicio": "2026-11-09", "fin": "2026-11-20", "depende_de": ["excavacion"]},
            {"nombre": "Muros", "inicio": "2026-11-23", "duracion": 10, "depende_de": [2]},
            {"nombre": "Fin de cimientos", "inicio": "2026-11-20", "hito": true, "depende_de": ["Cimientos"]}]}))
        .unwrap();
        assert!(c.fechas);
        assert_eq!(c.tareas[0].fin - c.tareas[0].inicio, 5, "del 2 al 6, cinco dias");
        assert_eq!(c.tareas[2].depende, vec![1]);
        assert!(c.tareas[3].hito);
        let mut f = Fabrica::nueva((0.0, 0.0), 4);
        elementos(&mut f, &c);
        let l = pixpin_motor2d::excalidraw::leer(&crate::lienzo::documento(f.salida.clone())).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0);
        assert_eq!(f.salida.iter().filter(|e| e["type"] == "diamond").count(), 1);
        assert_eq!(f.salida.iter().filter(|e| e["type"] == "arrow").count(), 3);
        let textos: Vec<&str> = f.salida.iter().filter_map(|e| e["text"].as_str()).collect();
        assert!(textos.contains(&"Obra") && textos.contains(&"Cimientos"));
        // Todo dentro del lienzo.
        assert!(f.salida.iter().all(|e| e["x"].as_f64().unwrap() >= 0.0 && e["x"].as_f64().unwrap() <= 800.0));
    }

    #[test]
    fn con_numeros_de_semana() {
        let c = validar_(json!({"unidad": "semanas", "tareas": [
            {"nombre": "Diseno", "inicio": 1, "fin": 4}, {"nombre": "Licencia", "inicio": 4, "fin": 9, "depende_de": ["Diseno"]}]}))
        .unwrap();
        assert!(!c.fechas);
        let ms = marcas(&c, 1, 9);
        assert!(ms.iter().all(|(_, r)| r.starts_with('S')));
    }

    #[test]
    fn lo_mal_puesto_se_rechaza() {
        for (v, motivo) in [
            (json!({"tareas": [{"nombre": "A", "inicio": "2026-11-10", "fin": "2026-11-02"}]}), "al reves"),
            (json!({"tareas": [{"nombre": "A", "inicio": 5, "fin": 2}]}), "al reves"),
            (json!({"tareas": [{"nombre": "A", "inicio": "ayer", "fin": 2}]}), "no es una fecha"),
            (json!({"tareas": [{"nombre": "A", "inicio": "2026-01-01", "fin": 3}]}), "los dos fechas"),
            (json!({"tareas": [{"nombre": "A", "inicio": 1, "fin": 3}, {"nombre": "B", "inicio": "2026-01-01", "fin": "2026-01-03"}]}), "todas"),
            (json!({"tareas": [{"nombre": "A", "inicio": 1, "fin": 3, "depende_de": ["Z"]}]}), "ninguna tarea"),
            (json!({"tareas": [{"nombre": "A", "inicio": 1, "fin": 3, "depende_de": ["A"]}]}), "si misma"),
            (json!({"tareas": [{"nombre": "A", "inicio": 1, "fin": 3, "depende_de": ["B"]}, {"nombre": "B", "inicio": 3, "fin": 5, "depende_de": ["A"]}]}), "circulo"),
            (json!({"tareas": [{"nombre": "A", "inicio": 1}]}), "falta «fin»"),
            (json!({"tareas": [{"inicio": 1, "fin": 2}]}), "falta «nombre»"),
            (json!({"tareas": []}), "vacia"),
            (json!({}), "falta «tareas»"),
            (json!({"tareas": [{"nombre": "A", "inicio": 0, "fin": 9000}]}), "como mucho"),
        ] {
            let e = validar_(v.clone()).unwrap_err().to_string();
            assert!(e.contains(motivo), "{v}: {e}");
        }
        let muchas: Vec<Value> = (0..=MAX_TAREAS).map(|i| json!({"nombre": format!("t{i}"), "inicio": i, "fin": i + 1})).collect();
        assert!(validar_(json!({"tareas": muchas})).is_err());
    }
}
