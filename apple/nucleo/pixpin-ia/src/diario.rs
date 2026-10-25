//! **El diario del asistente** (23-oct-2026, fase 1 del plan: medir).
//!
//! Una linea JSON por turno con lo que hace falta para saber si el
//! asistente mejora: si se rindio, cuantas herramientas fallaron, si el
//! repaso o el jefe corrigieron algo, cuantas tareas planeo el jefe y
//! cuanto tardo. Sin el texto entero del pedido (los primeros caracteres,
//! para reconocerlo) y sin respuestas: solo para medir, en el equipo del
//! usuario, nunca se manda a ningun sitio.
//!
//! [`resumen`] lo cuenta: con el, «el jefe sube los pedidos bien hechos del
//! 60 al 85 %» es un numero y no una impresion.

use std::io::Write;
use std::path::Path;

use serde_json::{Value, json};

/// Lo mas grande que se deja crecer el diario; al pasar, se queda la
/// segunda mitad (lo reciente).
pub const TOPE_BYTES: u64 = 2 * 1024 * 1024;
/// Lo que se guarda del pedido.
pub const LARGO_PEDIDO: usize = 120;

/// Un turno.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Entrada {
    /// Hora UTC en milisegundos.
    pub cuando: i64,
    /// «Claude», «DeepSeek», «Local».
    pub motor: String,
    /// Lo hizo con jefe y trabajadores.
    pub jefe: bool,
    pub pedido: String,
    pub herramientas_ok: usize,
    pub herramientas_error: usize,
    /// Dijo «no puedo»/«no tengo herramienta» (antes del repaso).
    pub se_rindio: bool,
    /// Vueltas de repaso.
    pub repasos: usize,
    /// Tareas del plan del jefe y cuantas acabaron hechas.
    pub tareas: usize,
    pub tareas_hechas: usize,
    /// Veces que el jefe devolvio una tarea para corregirla.
    pub correcciones: usize,
    pub segundos: f32,
    /// El turno acabo con error o lo paro el usuario.
    pub fallo: bool,
    pub cortado: bool,
}

impl Entrada {
    /// La linea del diario.
    pub fn a_json(&self) -> Value {
        json!({
            "cuando": self.cuando, "motor": self.motor, "jefe": self.jefe, "pedido": self.pedido,
            "herramientas_ok": self.herramientas_ok, "herramientas_error": self.herramientas_error,
            "se_rindio": self.se_rindio, "repasos": self.repasos, "tareas": self.tareas,
            "tareas_hechas": self.tareas_hechas, "correcciones": self.correcciones,
            "segundos": self.segundos, "fallo": self.fallo, "cortado": self.cortado,
        })
    }

    /// Una linea leida (lo que falte, por omision).
    pub fn de_json(v: &Value) -> Option<Entrada> {
        let o = v.as_object()?;
        let n = |k: &str| o.get(k).and_then(Value::as_u64).unwrap_or(0) as usize;
        let b = |k: &str| o.get(k).and_then(Value::as_bool).unwrap_or(false);
        let t = |k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        Some(Entrada {
            cuando: o.get("cuando").and_then(Value::as_i64).unwrap_or(0),
            motor: t("motor"),
            jefe: b("jefe"),
            pedido: t("pedido"),
            herramientas_ok: n("herramientas_ok"),
            herramientas_error: n("herramientas_error"),
            se_rindio: b("se_rindio"),
            repasos: n("repasos"),
            tareas: n("tareas"),
            tareas_hechas: n("tareas_hechas"),
            correcciones: n("correcciones"),
            segundos: o.get("segundos").and_then(Value::as_f64).unwrap_or(0.0) as f32,
            fallo: b("fallo"),
            cortado: b("cortado"),
        })
    }

    /// **Bien hecho**, a falta de que lo diga el usuario: acabo, sin
    /// rendirse, sin errores sin corregir y con todas las tareas hechas.
    pub fn bien(&self) -> bool {
        !self.fallo
            && !self.cortado
            && !self.se_rindio
            && self.tareas_hechas >= self.tareas
            && (self.herramientas_error == 0 || self.repasos > 0 || self.correcciones > 0)
    }
}

/// El pedido, recortado para el diario.
pub fn recortar(pedido: &str) -> String {
    let t = pedido.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= LARGO_PEDIDO {
        t
    } else {
        format!("{}…", t.chars().take(LARGO_PEDIDO).collect::<String>())
    }
}

/// **Apunta un turno** al final del diario (y lo poda si se pasa).
pub fn apuntar(ruta: &Path, e: &Entrada) -> std::io::Result<()> {
    if let Some(dir) = ruta.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if std::fs::metadata(ruta).map(|m| m.len()).unwrap_or(0) > TOPE_BYTES {
        let todo = std::fs::read_to_string(ruta).unwrap_or_default();
        let lineas: Vec<&str> = todo.lines().collect();
        let resto = lineas[lineas.len() / 2..].join("\n");
        std::fs::write(ruta, format!("{resto}\n"))?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(ruta)?;
    writeln!(f, "{}", e.a_json())
}

/// Lo leido del diario (las lineas que no se entienden se saltan).
pub fn leer(ruta: &Path) -> Vec<Entrada> {
    std::fs::read_to_string(ruta)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| Entrada::de_json(&v))
        .collect()
}

/// Las cuentas de un grupo de turnos.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cuentas {
    pub turnos: usize,
    pub bien: usize,
    pub rendidos: usize,
    pub con_errores: usize,
    pub segundos_medios: f32,
}

impl Cuentas {
    fn de<'a>(e: impl Iterator<Item = &'a Entrada>) -> Cuentas {
        let mut c = Cuentas::default();
        let mut s = 0.0;
        for x in e {
            c.turnos += 1;
            c.bien += usize::from(x.bien());
            c.rendidos += usize::from(x.se_rindio);
            c.con_errores += usize::from(x.herramientas_error > 0);
            s += x.segundos;
        }
        if c.turnos > 0 {
            c.segundos_medios = s / c.turnos as f32;
        }
        c
    }

    /// El porcentaje bien hecho (0 sin turnos).
    pub fn porcentaje_bien(&self) -> f32 {
        if self.turnos == 0 {
            0.0
        } else {
            100.0 * self.bien as f32 / self.turnos as f32
        }
    }
}

/// **El resumen**: todo, solo el trabajador y con jefe.
pub fn resumen(entradas: &[Entrada]) -> (Cuentas, Cuentas, Cuentas) {
    (
        Cuentas::de(entradas.iter()),
        Cuentas::de(entradas.iter().filter(|e| !e.jefe)),
        Cuentas::de(entradas.iter().filter(|e| e.jefe)),
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_apunta_se_lee_y_se_cuenta() {
        let dir = std::env::temp_dir().join(format!("pixpin-diario-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ruta = dir.join("diario.jsonl");
        let bien = Entrada {
            motor: "Claude".into(),
            herramientas_ok: 3,
            segundos: 4.0,
            ..Default::default()
        };
        let rendido = Entrada {
            se_rindio: true,
            segundos: 2.0,
            ..Default::default()
        };
        let con_jefe = Entrada {
            jefe: true,
            tareas: 3,
            tareas_hechas: 2,
            segundos: 30.0,
            ..Default::default()
        };
        for e in [&bien, &rendido, &con_jefe] {
            apuntar(&ruta, e).unwrap();
        }
        std::fs::write(
            &ruta,
            format!("{}no es json\n", std::fs::read_to_string(&ruta).unwrap()),
        )
        .unwrap();
        let leidas = leer(&ruta);
        assert_eq!(leidas, vec![bien, rendido, con_jefe]);
        let (todo, solo, jefe) = resumen(&leidas);
        assert_eq!((todo.turnos, todo.bien, todo.rendidos), (3, 1, 1));
        assert_eq!((solo.turnos, jefe.turnos), (2, 1));
        assert!((todo.segundos_medios - 12.0).abs() < 1e-3);
        assert!((todo.porcentaje_bien() - 100.0 / 3.0).abs() < 1e-3);
        // Un error corregido por el repaso cuenta como bien; sin corregir, no.
        assert!(
            Entrada {
                herramientas_error: 1,
                repasos: 1,
                ..Default::default()
            }
            .bien()
        );
        assert!(
            !Entrada {
                herramientas_error: 1,
                ..Default::default()
            }
            .bien()
        );
        assert_eq!(
            recortar(&"palabra ".repeat(40)).chars().count(),
            LARGO_PEDIDO + 1
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
