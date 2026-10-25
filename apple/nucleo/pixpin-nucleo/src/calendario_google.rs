//! **Tu Google Calendar en PixPin** (Windows, 18-oct), solo para ver: los
//! eventos avisan con la misma llamada que las clases (`horario`) y el
//! asistente contesta «¿que tengo hoy?» (`ver_calendario`).
//!
//! Lo leido se guarda en `calendario-google.json` junto a los datos (el
//! mismo fichero que Windows): quien lo lee nunca espera a Google. En el
//! Mac la lectura la lanza Swift cada 15 minutos ([`Nucleo::calendario_leer`]),
//! en su hilo de fondo.

use std::path::{Path, PathBuf};

use pixpin_ia::conexiones::calendario::{self, Evento, GOOGLE_CALENDAR};
use pixpin_ia::conexiones::guardado;

use crate::{ErrorNucleo, Nucleo};

/// Lo que se trae: de hace dos dias a tres semanas.
const ATRAS_MS: i64 = 2 * 86_400_000;
const ADELANTE_MS: i64 = 21 * 86_400_000;
const DIA: i64 = 86_400_000;

fn fichero(raiz: &Path) -> PathBuf {
    raiz.join("calendario-google.json")
}

/// Si hay permiso guardado de Google Calendar.
pub fn conectado() -> bool {
    guardado::carpeta().is_some_and(|c| guardado::leer_de(&c, &GOOGLE_CALENDAR).conectado())
}

/// Lo guardado: los eventos y cuando se leyeron (0: nunca).
fn guardados(raiz: &Path) -> (Vec<Evento>, i64) {
    let Some(v) = std::fs::read(fichero(raiz)).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
    else {
        return (Vec::new(), 0);
    };
    (calendario::de_json(&v["eventos"]), v["leido"].as_i64().unwrap_or(0))
}

/// Los eventos que tocan `[desde, hasta)` (ms UTC), de lo guardado.
pub fn eventos_entre(raiz: &Path, desde_utc: i64, hasta_utc: i64) -> Vec<Evento> {
    guardados(raiz)
        .0
        .into_iter()
        .filter(|e| e.inicio_ms < hasta_utc && e.fin_ms > desde_utc)
        .collect()
}

/// Uno por su id (el aviso que suena).
pub fn evento(raiz: &Path, id: &str) -> Option<Evento> {
    guardados(raiz).0.into_iter().find(|e| e.id == id)
}

/// `16:00–17:30` (o «todo el día») en hora local.
pub fn tramo_legible(e: &Evento, desfase_ms: i64) -> String {
    if e.todo_el_dia {
        return "todo el día".into();
    }
    let hm = |utc: i64| {
        let l = (utc + desfase_ms).rem_euclid(DIA) / 60_000;
        format!("{}:{:02}", l / 60, l % 60)
    };
    format!("{}–{}", hm(e.inicio_ms), hm(e.fin_ms))
}

/// **Lo que se le cuenta al asistente** de los eventos de `[desde, hasta)`
/// (ms locales).
pub fn para_el_modelo(raiz: &Path, desde_local: i64, hasta_local: i64, desfase_ms: i64, conectado: bool) -> String {
    if !conectado {
        return "Google Calendar no esta conectado: el usuario lo conecta en Ajustes → Asistente → Conexiones (Conectar Google Calendar).".into();
    }
    let evs = eventos_entre(raiz, desde_local - desfase_ms, hasta_local - desfase_ms);
    let leido = guardados(raiz).1;
    let hace = if leido == 0 {
        "aun no se ha leido (se lee en unos segundos)".to_string()
    } else {
        format!("leido hace {} min", (crate::ahora_ms() - leido).max(0) / 60_000)
    };
    if evs.is_empty() {
        return format!("No hay eventos en Google Calendar en ese tramo ({hace}).");
    }
    let mut out = format!("{} eventos de Google Calendar ({hace}):\n", evs.len());
    for e in &evs {
        let dia = if e.todo_el_dia { e.inicio_ms } else { e.inicio_ms + desfase_ms }.div_euclid(DIA);
        let (a, m, d) = calendario::civil(dia);
        let semana = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"][(dia + 3).rem_euclid(7) as usize];
        out.push_str(&format!("- {semana} {d:02}/{m:02}/{a} {} «{}»", tramo_legible(e, desfase_ms), e.titulo));
        if !e.lugar.is_empty() {
            out.push_str(&format!(" en {}", e.lugar));
        }
        out.push_str(&format!(" ({})\n", e.calendario));
    }
    out
}

/// **«¿Que tengo hoy?»** (`ver_calendario`): los dias pedidos (`desde`:
/// hoy, mañana, un dia de la semana, «semana» o AAAA-MM-DD; `dias`).
pub fn ver_calendario(raiz: &Path, args: &serde_json::Value, desfase_ms: i64) -> String {
    let hoy = (crate::ahora_ms() + desfase_ms).div_euclid(DIA);
    let desde = args["desde"].as_str().unwrap_or("hoy").trim().to_lowercase();
    let mut dias = args["dias"].as_i64().unwrap_or(1).clamp(1, 31);
    let semana_de = |d: i64| (d + 3).rem_euclid(7);
    let nombres = ["lunes", "martes", "miercoles", "jueves", "viernes", "sabado", "domingo"];
    let sin_tildes = desde.replace('á', "a").replace('é', "e");
    let primero = match sin_tildes.as_str() {
        "" | "hoy" => hoy,
        "mañana" | "manana" => hoy + 1,
        "pasado mañana" | "pasado manana" | "pasado" => hoy + 2,
        "semana" | "esta semana" => {
            dias = dias.max(7);
            hoy - semana_de(hoy)
        }
        "semana que viene" | "proxima semana" | "la semana que viene" => {
            dias = dias.max(7);
            hoy - semana_de(hoy) + 7
        }
        otro => {
            let sin = otro.replace("sábado", "sabado").replace("miércoles", "miercoles");
            if let Some(k) = nombres.iter().position(|n| sin.contains(n)) {
                hoy + (k as i64 - semana_de(hoy)).rem_euclid(7)
            } else {
                let p: Vec<i64> = otro.split(['-', '/']).filter_map(|x| x.parse().ok()).collect();
                match p.as_slice() {
                    [a, m, d] if *a > 1900 => calendario::dias_desde_1970(*a, *m as u32, *d as u32),
                    [d, m, a] if *a > 1900 => calendario::dias_desde_1970(*a, *m as u32, *d as u32),
                    _ => hoy,
                }
            }
        }
    };
    para_el_modelo(raiz, primero * DIA, (primero + dias) * DIA, desfase_ms, conectado())
}

#[uniffi::export]
impl Nucleo {
    /// **Lee Google Calendar** (de hace dos dias a tres semanas) y lo deja
    /// en `calendario-google.json`. Bloquea (red): Swift lo llama en un hilo
    /// de fondo cada 15 min y al conectar. Devuelve si cambio algo (para
    /// releer los avisos). Sin conectar no hace nada.
    pub fn calendario_leer(&self) -> Result<bool, ErrorNucleo> {
        if !conectado() {
            return Ok(false);
        }
        let mal = |detalle: String| ErrorNucleo::Asistente { detalle };
        let cx = pixpin_ia::conexiones::Conexion::abrir(&GOOGLE_CALENDAR).map_err(mal)?;
        let ahora = crate::ahora_ms();
        let eventos = calendario::eventos(&cx, ahora - ATRAS_MS, ahora + ADELANTE_MS).map_err(mal)?;
        let raiz = self.ruta();
        let cambiaron = guardados(raiz).0 != eventos;
        let v = serde_json::json!({"leido": ahora, "eventos": calendario::a_json(&eventos)});
        let tmp = fichero(raiz).with_extension("json.parte");
        std::fs::write(&tmp, v.to_string())?;
        std::fs::rename(&tmp, fichero(raiz))?;
        Ok(cambiaron)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn evento(id: &str, inicio: i64, fin: i64) -> Evento {
        let v = serde_json::json!([{"id": id, "calendario": "Personal", "titulo": "Dentista", "inicio": inicio, "fin": fin,
                                    "todo_el_dia": false, "lugar": "Calle 1"}]);
        calendario::de_json(&v).into_iter().next().expect("el evento se lee")
    }

    #[test]
    fn lo_guardado_se_filtra_por_tramo_y_se_cuenta_al_modelo() {
        let raiz = std::env::temp_dir().join(format!("pixpin-gcal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        // 3-oct-2026 14:00–15:00 UTC.
        let ini = 1_790_985_600_000 + 14 * 3_600_000;
        let e = evento("e1", ini, ini + 3_600_000);
        let v = serde_json::json!({"leido": 1, "eventos": calendario::a_json(std::slice::from_ref(&e))});
        std::fs::write(fichero(&raiz), v.to_string()).unwrap();
        assert_eq!(eventos_entre(&raiz, ini - 1, ini + 1).len(), 1);
        assert!(eventos_entre(&raiz, ini + 3_600_000, ini + 7_200_000).is_empty(), "despues no");
        assert_eq!(tramo_legible(&e, -5 * 3_600_000), "9:00–10:00");
        let t = para_el_modelo(&raiz, ini - DIA, ini + DIA, 0, true);
        assert!(t.contains("«Dentista» en Calle 1 (Personal)"), "{t}");
        // Caso negativo: sin conectar se dice como conectarlo.
        assert!(para_el_modelo(&raiz, 0, 1, 0, false).contains("no esta conectado"));
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
