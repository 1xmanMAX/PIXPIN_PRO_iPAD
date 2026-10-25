//! **Google Calendar, solo para ver** (18-oct-2026). Pedido del usuario:
//! «una integracion con Google Calendar», y de lo que se le ofrecio eligio
//! *ver mi Calendar en PixPin*: los eventos salen en la rejilla del horario
//! semanal, avisan con la llamada de las clases y el asistente contesta
//! «¿que tengo hoy?». Nada aqui cambia el calendario del usuario: el permiso
//! es `calendar.readonly`.
//!
//! Va sobre lo mismo que Drive ([`super::Conexion`]: el permiso de Google, el
//! token renovado y los fallos ya dichos); el ID de cliente de Google es el
//! mismo para los dos ([`super::guardar_cliente_en`] lo copia).

use serde_json::Value;

use super::{Conexion, Servicio};

pub const GOOGLE_CALENDAR: Servicio = Servicio {
    id: "google_calendar",
    nombre: "Google Calendar",
    autorizar: "https://accounts.google.com/o/oauth2/v2/auth",
    token: "https://oauth2.googleapis.com/token",
    revocar: "https://oauth2.googleapis.com/revoke",
    api: "https://www.googleapis.com/calendar/v3",
    subida: "",
    alcances: &["https://www.googleapis.com/auth/calendar.readonly"],
    extra_autorizar: &[("access_type", "offline"), ("prompt", "consent")],
    consola: "console.cloud.google.com",
};

const LIMITE_JSON: usize = 8 * 1024 * 1024;
/// Lo mas que se piden de un calendario de una vez.
const POR_CALENDARIO: usize = 250;

/// **Un evento**, lo que hace falta para pintarlo, avisar y contarlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evento {
    pub id: String,
    /// El nombre del calendario («Personal», «Clases»…).
    pub calendario: String,
    pub titulo: String,
    /// En milisegundos UTC. En un evento de todo el dia, la medianoche UTC
    /// de su fecha (es una fecha, no una hora: quien lo pinte no le suma la
    /// zona horaria).
    pub inicio_ms: i64,
    pub fin_ms: i64,
    pub todo_el_dia: bool,
    pub lugar: String,
    /// `#rrggbb`, el del calendario.
    pub color: String,
    /// Para abrirlo en el navegador.
    pub enlace: String,
}

/// El correo de la cuenta (el id del calendario principal).
pub fn cuenta(cx: &Conexion) -> Result<String, String> {
    let v = pedir_json(
        cx,
        &format!("{}/calendars/primary?fields=id", cx.extremos.api),
    )?;
    Ok(v["id"].as_str().unwrap_or_default().to_string())
}

/// **Los eventos de todos los calendarios visibles** entre `desde_ms` y
/// `hasta_ms` (UTC), ordenados por su inicio. Un calendario que falla no
/// tumba a los demas (se apunta en el registro).
pub fn eventos(cx: &Conexion, desde_ms: i64, hasta_ms: i64) -> Result<Vec<Evento>, String> {
    let lista = pedir_json(
        cx,
        &format!(
            "{}/users/me/calendarList?minAccessRole=reader&fields=items(id,summary,summaryOverride,backgroundColor,selected,hidden)",
            cx.extremos.api
        ),
    )?;
    let mut todos = Vec::new();
    for c in lista["items"].as_array().into_iter().flatten() {
        // Los que el usuario tiene ocultos o sin marcar en Google, tampoco.
        if c["hidden"].as_bool() == Some(true) || c["selected"].as_bool() == Some(false) {
            continue;
        }
        let Some(id) = c["id"].as_str() else { continue };
        let nombre = c["summaryOverride"]
            .as_str()
            .or_else(|| c["summary"].as_str())
            .unwrap_or(id)
            .to_string();
        let color = c["backgroundColor"]
            .as_str()
            .unwrap_or("#4285f4")
            .to_string();
        let url = format!(
            "{}/calendars/{}/events?timeMin={}&timeMax={}&singleEvents=true&orderBy=startTime&maxResults={POR_CALENDARIO}&fields=items(id,summary,start,end,location,htmlLink,status)",
            cx.extremos.api,
            super::url::escapar(id),
            super::url::escapar(&rfc3339(desde_ms)),
            super::url::escapar(&rfc3339(hasta_ms)),
        );
        match pedir_json(cx, &url) {
            Ok(v) => todos.extend(
                v["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| evento_de(e, &nombre, &color)),
            ),
            Err(e) => tracing::warn!("calendario: no se pudo leer «{nombre}»: {e}"),
        }
    }
    todos.sort_by(|a, b| a.inicio_ms.cmp(&b.inicio_ms).then(a.titulo.cmp(&b.titulo)));
    Ok(todos)
}

fn pedir_json(cx: &Conexion, url: &str) -> Result<Value, String> {
    let r = cx.pedir("GET", url, &[], &[], LIMITE_JSON)?;
    serde_json::from_slice(&r.cuerpo)
        .map_err(|e| format!("Google Calendar contesto algo que no se entiende: {e}"))
}

/// Un evento de la API (`None`: cancelado o sin fechas).
pub fn evento_de(e: &Value, calendario: &str, color: &str) -> Option<Evento> {
    if e["status"].as_str() == Some("cancelled") {
        return None;
    }
    let (inicio_ms, todo) = momento(&e["start"])?;
    let (fin_ms, _) = momento(&e["end"]).unwrap_or((inicio_ms + 3_600_000, todo));
    Some(Evento {
        id: e["id"].as_str().unwrap_or_default().to_string(),
        calendario: calendario.to_string(),
        titulo: e["summary"]
            .as_str()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or("(sin título)")
            .to_string(),
        inicio_ms,
        fin_ms: fin_ms.max(inicio_ms),
        todo_el_dia: todo,
        lugar: e["location"].as_str().unwrap_or_default().to_string(),
        color: color.to_string(),
        enlace: e["htmlLink"].as_str().unwrap_or_default().to_string(),
    })
}

/// `{"dateTime": "2026-10-19T09:00:00-05:00"}` o `{"date": "2026-10-19"}`.
fn momento(v: &Value) -> Option<(i64, bool)> {
    if let Some(t) = v["dateTime"].as_str() {
        return Some((ms_de_rfc3339(t)?, false));
    }
    let d = v["date"].as_str()?;
    let (a, m, dd) = fecha(d)?;
    Some((dias_desde_1970(a, m, dd) * 86_400_000, true))
}

fn fecha(s: &str) -> Option<(i64, u32, u32)> {
    let mut p = s.get(..10)?.split('-');
    let a = p.next()?.parse().ok()?;
    let m = p.next()?.parse().ok()?;
    let d = p.next()?.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((a, m, d))
}

/// `2026-10-19T09:00:00-05:00`, `…Z`, `…T09:00:00.000+01:00` → ms UTC.
pub fn ms_de_rfc3339(s: &str) -> Option<i64> {
    let (a, m, d) = fecha(s)?;
    let t = s.get(11..)?;
    let num = |x: &str| x.parse::<i64>().ok();
    let (h, mi, se) = (num(t.get(0..2)?)?, num(t.get(3..5)?)?, num(t.get(6..8)?)?);
    let resto = &t[8..];
    // Las fracciones de segundo no cuentan.
    let zona = resto.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let desfase_min = match zona {
        "" | "Z" | "z" => 0,
        z => {
            let signo = if z.starts_with('-') { -1 } else { 1 };
            let z = &z[1..];
            signo * (num(z.get(0..2)?)? * 60 + num(z.get(3..5).unwrap_or("00"))?)
        }
    };
    let local = dias_desde_1970(a, m, d) * 86_400 + h * 3600 + mi * 60 + se;
    Some((local - desfase_min * 60) * 1000)
}

/// ms UTC → `2026-10-19T05:00:00Z`.
pub fn rfc3339(ms: i64) -> String {
    let s = ms.div_euclid(1000);
    let (dias, resto) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    let (a, m, d) = civil(dias);
    format!(
        "{a:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        resto / 3600,
        resto % 3600 / 60,
        resto % 60
    )
}

/// Dias desde el 1-1-1970 de una fecha (calendario gregoriano).
pub fn dias_desde_1970(a: i64, m: u32, d: u32) -> i64 {
    let (a, m) = if m <= 2 { (a - 1, m + 9) } else { (a, m - 3) };
    let era = a.div_euclid(400);
    let ae = a - era * 400;
    let dia_del_ano = (153 * m as i64 + 2) / 5 + d as i64 - 1;
    let de_la_era = ae * 365 + ae / 4 - ae / 100 + dia_del_ano;
    era * 146_097 + de_la_era - 719_468
}

/// Y al reves.
pub fn civil(dias: i64) -> (i64, u32, u32) {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let de_la_era = z - era * 146_097;
    let ae = (de_la_era - de_la_era / 1460 + de_la_era / 36_524 - de_la_era / 146_096) / 365;
    let dia_del_ano = de_la_era - (365 * ae + ae / 4 - ae / 100);
    let mp = (5 * dia_del_ano + 2) / 153;
    let d = (dia_del_ano - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (era * 400 + ae + i64::from(m <= 2), m, d)
}

/// Los eventos en JSON (la cache de la app).
pub fn a_json(eventos: &[Evento]) -> Value {
    Value::Array(
        eventos
            .iter()
            .map(|e| {
                serde_json::json!({"id": e.id, "calendario": e.calendario, "titulo": e.titulo, "inicio": e.inicio_ms,
                    "fin": e.fin_ms, "todo_el_dia": e.todo_el_dia, "lugar": e.lugar, "color": e.color, "enlace": e.enlace})
            })
            .collect(),
    )
}

/// Y al reves (lo que no se entiende, fuera).
pub fn de_json(v: &Value) -> Vec<Evento> {
    let t = |e: &Value, k: &str| e[k].as_str().unwrap_or_default().to_string();
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            Some(Evento {
                id: t(e, "id"),
                calendario: t(e, "calendario"),
                titulo: t(e, "titulo"),
                inicio_ms: e["inicio"].as_i64()?,
                fin_ms: e["fin"].as_i64()?,
                todo_el_dia: e["todo_el_dia"].as_bool().unwrap_or(false),
                lugar: t(e, "lugar"),
                color: t(e, "color"),
                enlace: t(e, "enlace"),
            })
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    #[test]
    fn las_horas_de_google_pasan_a_utc() {
        // 9:00 en Lima (UTC-5) son las 14:00 UTC.
        let a = ms_de_rfc3339("2026-10-19T09:00:00-05:00").unwrap();
        assert_eq!(rfc3339(a), "2026-10-19T14:00:00Z");
        assert_eq!(ms_de_rfc3339("2026-10-19T14:00:00Z"), Some(a));
        assert_eq!(ms_de_rfc3339("2026-10-19T15:00:00.000+01:00"), Some(a));
        assert_eq!(civil(dias_desde_1970(2024, 2, 29)), (2024, 2, 29));
        assert_eq!(dias_desde_1970(1970, 1, 1), 0);
    }

    #[test]
    fn un_evento_de_la_api_y_uno_de_todo_el_dia() {
        let e = evento_de(
            &json!({"id": "a1", "summary": "Reunión con el asesor", "location": "Aula 12",
                    "start": {"dateTime": "2026-10-20T16:00:00-05:00"}, "end": {"dateTime": "2026-10-20T17:30:00-05:00"}}),
            "Personal",
            "#0b8043",
        )
        .unwrap();
        assert_eq!(
            (e.titulo.as_str(), e.lugar.as_str(), e.todo_el_dia),
            ("Reunión con el asesor", "Aula 12", false)
        );
        assert_eq!(e.fin_ms - e.inicio_ms, 90 * 60_000);
        let f = evento_de(
            &json!({"id": "b", "start": {"date": "2026-10-21"}, "end": {"date": "2026-10-22"}}),
            "Feriados",
            "#fff",
        )
        .unwrap();
        assert!(f.todo_el_dia);
        assert_eq!(f.titulo, "(sin título)");
        assert_eq!(rfc3339(f.inicio_ms), "2026-10-21T00:00:00Z");
        assert!(
            evento_de(
                &json!({"id": "c", "status": "cancelled", "start": {"date": "2026-10-21"}}),
                "x",
                "#fff"
            )
            .is_none()
        );
    }
}
