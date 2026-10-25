//! **La hoja de trabajo del asistente**: un documento corto que el agente
//! reescribe en cada turno y que es su memoria de la conversacion.
//!
//! Pedido del usuario (13-oct-2026): que el asistente «guarde sus
//! pensamientos y su proceso» y que, si se le acaba la ventana de contexto,
//! «solo lea su mejora de la peticion»: algo iterativo en el que se cierra la
//! conversacion y se empieza de nuevo con lo que se descifro pensando.
//!
//! La hoja tiene las partes que pidio el usuario —**Objetivo** (la peticion
//! mejorada), **Hechos**, **Hecho/Falta** y **Decisiones**— y, por turno, el
//! pensamiento resumido. Cada parte tiene un tope de lineas y de letras: la
//! hoja tiene que caber siempre en el contexto, porque es justo lo que queda
//! cuando se reinicia (ver [`crate::contexto`]).
//!
//! Se guarda con cada respuesta del asistente en un [`Registro`] (ver
//! `apps/pixpin/src/asistente/pensado.rs`): el de la ultima respuesta es la
//! hoja del chat. Asi viaja con su nota como todo lo anotado y se borra con
//! ella, sin nada nuevo en la sincronizacion. Un registro roto no rompe nada:
//! se empieza una hoja nueva.

use serde_json::{Value, json};

use crate::pensar::{Plan, cadenas, cortar, resumir};

/// Topes: lineas de cada parte y letras de cada linea.
pub const MAX_HECHOS: usize = 8;
pub const MAX_HECHO: usize = 6;
pub const MAX_FALTA: usize = 4;
pub const MAX_DECISIONES: usize = 6;
pub const MAX_TURNOS: usize = 4;
pub const LARGO_LINEA: usize = 160;
/// Lo largo del pensamiento resumido de cada turno.
pub const LARGO_PENSADO: usize = 220;
/// Cuanto pensamiento se guarda entero en el registro de un turno.
pub const MAX_PENSAMIENTO: usize = 20_000;

/// Version del formato guardado.
pub const VERSION: u64 = 1;

/// Lo esencial de un turno ya hecho.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TurnoResumido {
    pub n: u32,
    /// La peticion mejorada de ese turno.
    pub peticion: String,
    /// Lo que penso, en una o dos frases.
    pub pensado: String,
    /// Como acabo, en una linea.
    pub resultado: String,
}

/// **La hoja.**
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HojaDeTrabajo {
    pub objetivo: String,
    pub hechos: Vec<String>,
    pub hecho: Vec<String>,
    pub falta: Vec<String>,
    pub decisiones: Vec<String>,
    pub turnos: Vec<TurnoResumido>,
    /// Desde que mensaje del chat se cuenta la conversacion tras el ultimo
    /// reinicio (su id). Lo de antes ya no se manda: lo resume la hoja y lo
    /// recupera `buscar_historial`.
    pub reinicio_desde: Option<String>,
    /// Cuantos reinicios lleva este chat.
    pub reinicios: u32,
}

/// Lo que paso en un turno, para reescribir la hoja.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResumenDelTurno {
    /// Las herramientas que se usaron bien, con su resumen corto.
    pub herramientas: Vec<String>,
    /// El texto final de la respuesta.
    pub respuesta: String,
    /// Si acabo mal: cortado, error o sin saber hacerlo.
    pub incompleto: bool,
    /// El pensamiento entero del turno.
    pub pensamiento: String,
}

impl HojaDeTrabajo {
    pub fn esta_vacia(&self) -> bool {
        self.objetivo.is_empty()
            && self.hechos.is_empty()
            && self.hecho.is_empty()
            && self.turnos.is_empty()
    }

    /// El numero del turno siguiente.
    pub fn siguiente_turno(&self) -> u32 {
        self.turnos.last().map_or(1, |t| t.n + 1)
    }

    /// **Reescribe la hoja** con lo que entendio el modelo y lo que paso.
    pub fn reescribir(&mut self, plan: &Plan, turno: &ResumenDelTurno) {
        let n = self.siguiente_turno();
        if !plan.entiendo.trim().is_empty() {
            self.objetivo = cortar(plan.entiendo.trim(), 2 * LARGO_LINEA);
        }
        for d in &plan.datos {
            anadir_sin_repetir(&mut self.hechos, d, MAX_HECHOS);
        }
        for d in &plan.decidido {
            anadir_sin_repetir(&mut self.decisiones, d, MAX_DECISIONES);
        }
        let primera = turno
            .respuesta
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("")
            .to_string();
        let mut linea = format!("T{n}:");
        if !turno.herramientas.is_empty() {
            linea.push_str(&format!(" [{}]", turno.herramientas.join(", ")));
        }
        if !primera.is_empty() {
            linea.push(' ');
            linea.push_str(&primera);
        }
        if turno.incompleto {
            linea.push_str(" (sin acabar)");
        }
        empujar_con_tope(&mut self.hecho, cortar(&linea, LARGO_LINEA), MAX_HECHO);
        if turno.incompleto {
            let falta = if plan.hare.is_empty() {
                plan.entiendo.clone()
            } else {
                plan.hare.clone()
            };
            if !falta.trim().is_empty() {
                anadir_sin_repetir(&mut self.falta, &falta, MAX_FALTA);
            }
        } else {
            // Acabo bien: lo que faltaba de este objetivo ya no falta.
            self.falta.clear();
        }
        empujar_con_tope(
            &mut self.turnos,
            TurnoResumido {
                n,
                peticion: cortar(&plan.entiendo, LARGO_LINEA),
                pensado: resumir(&turno.pensamiento, LARGO_PENSADO),
                resultado: cortar(&primera, LARGO_LINEA),
            },
            MAX_TURNOS,
        );
    }

    /// **La hoja para el modelo**, en Markdown corto. Sin las partes vacias.
    pub fn a_texto(&self) -> String {
        let mut s = String::from("# Hoja de trabajo\n");
        if !self.objetivo.is_empty() {
            s.push_str(&format!("Objetivo: {}\n", self.objetivo));
        }
        for (titulo, v) in [
            ("Hechos", &self.hechos),
            ("Hecho", &self.hecho),
            ("Falta", &self.falta),
            ("Decisiones", &self.decisiones),
        ] {
            if v.is_empty() {
                continue;
            }
            s.push_str(&format!("{titulo}:\n"));
            for x in v {
                s.push_str(&format!("- {x}\n"));
            }
        }
        if let Some(t) = self.turnos.iter().rev().find(|t| !t.pensado.is_empty()) {
            s.push_str(&format!("Lo ultimo que pense (T{}): {}\n", t.n, t.pensado));
        }
        s
    }

    pub fn a_json(&self) -> Value {
        json!({
            "objetivo": self.objetivo,
            "hechos": self.hechos,
            "hecho": self.hecho,
            "falta": self.falta,
            "decisiones": self.decisiones,
            "turnos": self.turnos.iter().map(|t| json!({
                "n": t.n, "peticion": t.peticion, "pensado": t.pensado, "resultado": t.resultado
            })).collect::<Vec<_>>(),
            "reinicio_desde": self.reinicio_desde,
            "reinicios": self.reinicios,
        })
    }

    /// Lee una hoja. Las partes que falten quedan vacias, y lo que exceda los
    /// topes se recorta (una hoja escrita a mano o por otra version no puede
    /// llenar el contexto). Solo falla si no es un objeto.
    pub fn de_json(v: &Value) -> Result<HojaDeTrabajo, String> {
        let o = v.as_object().ok_or("la hoja no es un objeto")?;
        let cadena = |k: &str| {
            o.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let recortar = |v: Vec<String>, n: usize| -> Vec<String> {
            let desde = v.len().saturating_sub(n);
            v[desde..].iter().map(|x| cortar(x, LARGO_LINEA)).collect()
        };
        let turnos: Vec<TurnoResumido> = o
            .get("turnos")
            .and_then(Value::as_array)
            .map(|l| {
                l.iter()
                    .filter_map(Value::as_object)
                    .map(|t| TurnoResumido {
                        n: t.get("n").and_then(Value::as_u64).unwrap_or(0) as u32,
                        peticion: t
                            .get("peticion")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        pensado: cortar(
                            t.get("pensado").and_then(Value::as_str).unwrap_or_default(),
                            LARGO_PENSADO,
                        ),
                        resultado: t
                            .get("resultado")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let desde = turnos.len().saturating_sub(MAX_TURNOS);
        Ok(HojaDeTrabajo {
            objetivo: cortar(&cadena("objetivo"), 2 * LARGO_LINEA),
            hechos: recortar(cadenas(o.get("hechos")), MAX_HECHOS),
            hecho: recortar(cadenas(o.get("hecho")), MAX_HECHO),
            falta: recortar(cadenas(o.get("falta")), MAX_FALTA),
            decisiones: recortar(cadenas(o.get("decisiones")), MAX_DECISIONES),
            turnos: turnos[desde..].to_vec(),
            reinicio_desde: o
                .get("reinicio_desde")
                .and_then(Value::as_str)
                .map(str::to_string),
            reinicios: o.get("reinicios").and_then(Value::as_u64).unwrap_or(0) as u32,
        })
    }
}

fn empujar_con_tope<T>(v: &mut Vec<T>, x: T, tope: usize) {
    v.push(x);
    if v.len() > tope {
        v.drain(..v.len() - tope);
    }
}

fn anadir_sin_repetir(v: &mut Vec<String>, x: &str, tope: usize) {
    let x = cortar(x.trim(), LARGO_LINEA);
    if x.is_empty() {
        return;
    }
    let bajo = x.to_lowercase();
    v.retain(|y| y.to_lowercase() != bajo);
    empujar_con_tope(v, x, tope);
}

// ---------------------------------------------------------------------------
// El registro de un turno

/// **Lo que se guarda con cada respuesta del asistente**: lo que penso, cuanto,
/// la peticion mejorada y la hoja tal como quedo tras ese turno.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Registro {
    pub pensamiento: String,
    /// Cuanto penso. `None` si no penso.
    pub segundos: Option<f32>,
    pub plan: Option<Plan>,
    pub hoja: HojaDeTrabajo,
    /// Si este turno empezo con el contexto reiniciado.
    pub reinicio: bool,
}

impl Registro {
    pub fn a_json(&self) -> Value {
        let pensamiento = if self.pensamiento.chars().count() > MAX_PENSAMIENTO {
            cortar(&self.pensamiento, MAX_PENSAMIENTO)
        } else {
            self.pensamiento.clone()
        };
        json!({
            "version": VERSION,
            "pensamiento": pensamiento,
            "segundos": self.segundos,
            "plan": self.plan.as_ref().map(Plan::a_json),
            "hoja": self.hoja.a_json(),
            "reinicio": self.reinicio,
        })
    }

    pub fn de_json(v: &Value) -> Result<Registro, String> {
        let o = v.as_object().ok_or("el registro no es un objeto")?;
        let version = o.get("version").and_then(Value::as_u64).unwrap_or(0);
        if version == 0 || version > VERSION {
            return Err(format!("version desconocida: {version}"));
        }
        Ok(Registro {
            pensamiento: o
                .get("pensamiento")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            segundos: o.get("segundos").and_then(Value::as_f64).map(|s| s as f32),
            plan: o.get("plan").and_then(Plan::de_json),
            hoja: match o.get("hoja") {
                Some(h) => HojaDeTrabajo::de_json(h)?,
                None => HojaDeTrabajo::default(),
            },
            reinicio: o.get("reinicio").and_then(Value::as_bool).unwrap_or(false),
        })
    }

    /// Lee un registro del disco. `Ok(None)` si no hay; `Err` si esta roto
    /// (quien llama decide: el chat lo apunta y sigue con una hoja nueva).
    pub fn leer(ruta: &std::path::Path) -> Result<Option<Registro>, String> {
        let bytes = match std::fs::read(ruta) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let v: Value = serde_json::from_slice(&bytes).map_err(|e| format!("JSON roto: {e}"))?;
        Registro::de_json(&v).map(Some)
    }

    /// Lo escribe entero o nada (a un temporal y luego encima).
    pub fn escribir(&self, ruta: &std::path::Path) -> std::io::Result<()> {
        if let Some(d) = ruta.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = ruta.with_extension("json.tmp");
        let texto = serde_json::to_string_pretty(&self.a_json()).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, texto)?;
        std::fs::rename(&tmp, ruta).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn plan(entiendo: &str) -> Plan {
        Plan {
            entiendo: entiendo.into(),
            hare: "dibujar".into(),
            herramientas: vec!["dibujar_en_lienzo".into()],
            datos: vec!["color rojo".into()],
            decidido: vec!["sin texto".into()],
            del_modelo: true,
        }
    }

    #[test]
    fn la_hoja_se_reescribe_cada_turno_sin_crecer_sin_fin() {
        let mut h = HojaDeTrabajo::default();
        for i in 0..20 {
            h.reescribir(
                &Plan {
                    datos: vec![format!("dato {i}"), "color rojo".into()],
                    ..plan(&format!("objetivo {i}"))
                },
                &ResumenDelTurno {
                    herramientas: vec!["dibujar_en_lienzo".into()],
                    respuesta: format!("Hecho {i}.\nmas"),
                    incompleto: false,
                    pensamiento: "Pienso. Mucho.".into(),
                },
            );
        }
        assert_eq!(h.objetivo, "objetivo 19");
        assert_eq!(h.hechos.len(), MAX_HECHOS);
        assert_eq!(
            h.hechos.iter().filter(|x| *x == "color rojo").count(),
            1,
            "sin repetir"
        );
        assert_eq!(h.hecho.len(), MAX_HECHO);
        assert!(
            h.hecho
                .last()
                .unwrap()
                .starts_with("T20: [dibujar_en_lienzo] Hecho 19.")
        );
        assert_eq!(h.turnos.len(), MAX_TURNOS);
        assert_eq!(h.decisiones, vec!["sin texto"]);
        let t = h.a_texto();
        assert!(t.contains("Objetivo: objetivo 19"));
        assert!(t.contains("Lo ultimo que pense (T20): Pienso. Mucho."));
        assert!(t.len() < 2500, "la hoja es corta: {}", t.len());
    }

    #[test]
    fn lo_que_falta_se_apunta_y_se_borra_al_acabar_bien() {
        let mut h = HojaDeTrabajo::default();
        h.reescribir(
            &plan("casa"),
            &ResumenDelTurno {
                incompleto: true,
                ..Default::default()
            },
        );
        assert_eq!(h.falta, vec!["dibujar"]);
        assert!(h.hecho[0].ends_with("(sin acabar)"));
        h.reescribir(&plan("casa"), &ResumenDelTurno::default());
        assert!(h.falta.is_empty());
    }

    #[test]
    fn el_registro_va_y_vuelve_por_el_disco() {
        let dir = std::env::temp_dir().join(format!("pixpin-ia-registro-{}", std::process::id()));
        let ruta = dir.join("anot-X.asistente.json");
        assert_eq!(Registro::leer(&ruta), Ok(None));
        let mut r = Registro {
            pensamiento: "a".repeat(MAX_PENSAMIENTO + 50),
            segundos: Some(12.5),
            plan: Some(plan("casa")),
            reinicio: true,
            ..Default::default()
        };
        r.hoja
            .reescribir(&plan("casa"), &ResumenDelTurno::default());
        r.hoja.reinicio_desde = Some("m7".into());
        r.escribir(&ruta).unwrap();
        let leido = Registro::leer(&ruta).unwrap().unwrap();
        assert_eq!(leido.pensamiento.chars().count(), MAX_PENSAMIENTO);
        assert_eq!(leido.segundos, Some(12.5));
        assert_eq!(leido.plan, r.plan);
        assert_eq!(leido.hoja, r.hoja);
        assert!(leido.reinicio);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_hoja_corrupta_no_rompe_nada() {
        let dir = std::env::temp_dir().join(format!("pixpin-ia-rota-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("rota.json");
        std::fs::write(&ruta, b"{\"version\":1, \"hoja\": {\"objetivo\": ").unwrap();
        assert!(Registro::leer(&ruta).is_err());
        std::fs::write(&ruta, b"{\"version\":99}").unwrap();
        assert!(
            Registro::leer(&ruta).is_err(),
            "una version del futuro no se lee a medias"
        );
        std::fs::write(&ruta, b"{\"version\":1,\"hoja\":7}").unwrap();
        assert!(Registro::leer(&ruta).is_err());
        // Una hoja enorme (escrita a mano) se recorta a los topes.
        let mucho: Vec<String> = (0..100)
            .map(|i| format!("{i} {}", "x".repeat(400)))
            .collect();
        let h = HojaDeTrabajo::de_json(&json!({"hechos": mucho, "turnos": "no es lista"})).unwrap();
        assert_eq!(h.hechos.len(), MAX_HECHOS);
        assert!(h.hechos.iter().all(|x| x.chars().count() <= LARGO_LINEA));
        assert!(h.turnos.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
