//! **La herramienta `buscar_en_mis_cosas`**: lo que ve el modelo, como se
//! valida lo que pide y como se le devuelve lo encontrado.
//!
//! Lo encontrado va numerado (`[1]`, `[2]`...) con su origen, y se le pide
//! al modelo que cite asi. La numeracion sigue de una llamada a otra en el
//! mismo turno ([`Citas`]): si busca dos veces, la segunda empieza en `[3]`
//! y no hay dos `[1]` que lleven a sitios distintos. La respuesta guarda las
//! citas y el chat ensena la primera que el modelo nombro como tarjeta que
//! abre el documento en esa pagina (o el chat en ese mensaje).

use serde_json::{Map, Value, json};

use super::indice::{Hallazgo, Tipo};
use crate::herramientas::{ErrorHerramienta, Herramienta};

pub const BUSCAR: &str = "buscar_en_mis_cosas";

/// Cuantos trozos se le dan al modelo por busqueda: el contexto del 2B es
/// de 8192 tokens y cada trozo son unos 120.
pub const RESULTADOS: usize = 5;

/// Lo mas larga que puede ser una consulta.
pub const MAX_CONSULTA: usize = 300;

/// La herramienta tal como se le describe al modelo.
pub fn herramienta() -> Herramienta {
    Herramienta {
        nombre: BUSCAR.into(),
        descripcion: concat!(
            "Busca en los PDF, Word, Excel, PowerPoint, notas y chats del usuario (y en los nombres de lo que mando). ",
            "Usala si pregunta que dice algo suyo o donde mando o apunto algo. Cita la fuente con su numero: [1]."
        )
        .into(),
        parametros: json!({
            "type": "object",
            "required": ["consulta"],
            "properties": {
                "consulta": {"type": "string", "description": "palabras clave, p. ej. «cunetas pendiente»"},
                "proyecto": {"type": "string", "description": "nombre del proyecto; omitir: todos"}
            }
        }),
    }
}

/// Lo que pidio el modelo, ya validado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeticionBusqueda {
    pub consulta: String,
    /// El nombre del proyecto donde buscar, tal como lo escribio el modelo.
    pub proyecto: Option<String>,
}

/// **Valida** los argumentos: hace falta una consulta con algo que buscar.
pub fn validar(args: &Map<String, Value>) -> Result<PeticionBusqueda, ErrorHerramienta> {
    let texto = |k: &str| {
        args.get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    // Los modelos pequenos a veces lo llaman «query» o «pregunta».
    let consulta = texto("consulta")
        .or_else(|| texto("query"))
        .or_else(|| texto("pregunta"))
        .ok_or_else(|| {
            ErrorHerramienta::Parametro("falta «consulta» (las palabras que buscar)".into())
        })?;
    if super::texto::de_la_consulta(&consulta).is_empty() {
        return Err(ErrorHerramienta::Parametro(
            "«consulta» no tiene palabras que buscar; pon las palabras clave del tema".into(),
        ));
    }
    let consulta: String = consulta.chars().take(MAX_CONSULTA).collect();
    let proyecto = texto("proyecto").or_else(|| texto("chat"));
    // «todos», «todo», «*»: es no filtrar.
    let proyecto = proyecto.filter(|p| {
        !matches!(
            super::texto::plano(p).as_str(),
            "todos" | "todo" | "*" | "all" | "ninguno"
        )
    });
    Ok(PeticionBusqueda { consulta, proyecto })
}

/// Una cita numerada de un turno.
#[derive(Debug, Clone, PartialEq)]
pub struct Cita {
    pub numero: usize,
    pub hallazgo: Hallazgo,
}

impl Cita {
    /// «Tesis.pdf · pág. 12», «Chat de Obra · mensaje del 3-oct»...
    pub fn titulo(&self) -> String {
        let h = &self.hallazgo;
        // Un mensaje: lo que dice (o el archivo que se mando), no «Chat de…».
        if h.tipo == Tipo::Chat {
            let mut s: String = h
                .fragmento
                .trim()
                .trim_start_matches('…')
                .chars()
                .take(60)
                .collect();
            if h.fragmento.chars().count() > 60 {
                s.push('…');
            }
            if let Some(f) = h.cuando.and_then(fecha_corta) {
                s.push_str(&format!(" · {f}"));
            }
            return s;
        }
        let mut s = h.nombre.clone();
        if let Some(p) = h.pagina {
            s.push_str(&format!(" · {} {p}", h.tipo.nombre_de_unidad()));
        } else if let Some(sec) = h.seccion.as_ref().filter(|s| !s.is_empty()) {
            s.push_str(&format!(" · {sec}"));
        }
        s
    }

    /// El pedido que lo abre.
    pub fn pedido(&self) -> Value {
        self.hallazgo.abrir.pedido()
    }
}

/// **Las citas de un turno**, numeradas seguidas.
#[derive(Debug, Clone, Default)]
pub struct Citas {
    pub lista: Vec<Cita>,
    /// Con la nube (17-oct-2026): hora local menos UTC, y cada hallazgo
    /// lleva su fecha y hora al minuto (`fechas::fecha_y_hora`). `None` (el
    /// modelo local, congelado): la fecha corta de siempre.
    pub desfase_ms: Option<i64>,
}

impl Citas {
    /// Numera lo encontrado (lo que ya estaba citado conserva su numero) y
    /// devuelve el texto para el modelo.
    pub fn anadir(
        &mut self,
        consulta: &str,
        hallazgos: Vec<Hallazgo>,
        aviso: Option<&str>,
    ) -> String {
        if hallazgos.is_empty() {
            let mut s = format!(
                "No encontre nada sobre «{consulta}» en los documentos y chats del usuario. Diselo asi; no inventes."
            );
            if let Some(a) = aviso {
                s.push(' ');
                s.push_str(a);
            }
            return s;
        }
        let mut s = String::from(
            "Esto es lo que hay en los documentos y chats del usuario. Contesta SOLO con lo que dicen estos trozos, \
             en pocas frases, y cita cada dato con su numero entre corchetes, como [1]:\n",
        );
        for h in hallazgos {
            let numero = match self.lista.iter().find(|c| c.hallazgo.abrir == h.abrir) {
                Some(c) => c.numero,
                None => {
                    let n = self.lista.len() + 1;
                    self.lista.push(Cita {
                        numero: n,
                        hallazgo: h.clone(),
                    });
                    n
                }
            };
            if h.tipo == Tipo::Chat && h.seccion.as_deref() == Some("adjunto") {
                // Una frase entera: con «origen + nombre» el 2B contestaba
                // «no has mandado ningun plano» (medido el 14-oct).
                let cuando = self
                    .cuando(&h)
                    .map(|f| format!(" el {f}"))
                    .unwrap_or_default();
                s.push_str(&format!(
                    "\n[{numero}] El usuario SI mando el archivo «{}» al chat «{}»{cuando}.\n",
                    h.fragmento.trim(),
                    h.proyecto_nombre
                ));
            } else {
                s.push_str(&format!(
                    "\n[{numero}] {}\n«{}»\n",
                    origen(&h, self.cuando(&h)),
                    h.fragmento.replace('\n', " ")
                ));
            }
        }
        if let Some(a) = aviso {
            s.push('\n');
            s.push_str(a);
        }
        s
    }

    /// La fecha de un hallazgo como se le dice al modelo.
    fn cuando(&self, h: &Hallazgo) -> Option<String> {
        let ms = h.cuando?;
        match self.desfase_ms {
            Some(d) => crate::fechas::fecha_y_hora(ms, d),
            None => fecha_corta(ms),
        }
    }

    /// Las que el modelo nombro en su respuesta (`[2]`), en orden de numero;
    /// si no nombro ninguna, la primera (algo tiene que llevar al
    /// documento).
    pub fn citadas(&self, respuesta: &str) -> Vec<&Cita> {
        let nombradas: Vec<&Cita> = self
            .lista
            .iter()
            .filter(|c| respuesta.contains(&format!("[{}]", c.numero)))
            .collect();
        if nombradas.is_empty() {
            self.lista.first().into_iter().collect()
        } else {
            nombradas
        }
    }
}

/// De donde es un hallazgo, en una linea para el modelo.
fn origen(h: &Hallazgo, cuando: Option<String>) -> String {
    let que = match h.tipo {
        Tipo::Chat => match h.seccion.as_deref() {
            // Dicho para que un modelo pequeno entienda que SI lo mando (con
            // «adjunto en el chat» el 2B contesto «no has mandado ningun
            // plano», medido el 14-oct).
            Some("adjunto") => format!(
                "archivo que el usuario mando al chat «{}»",
                h.proyecto_nombre
            ),
            Some(s) if !s.is_empty() => format!("{s} en el chat «{}»", h.proyecto_nombre),
            _ => format!("mensaje del chat «{}»", h.proyecto_nombre),
        },
        _ => {
            let mut s = format!("«{}»", h.nombre);
            if let Some(p) = h.pagina {
                s.push_str(&format!(", {} {p}", h.tipo.nombre_de_unidad()));
            } else if let Some(sec) = h.seccion.as_ref().filter(|s| !s.is_empty()) {
                s.push_str(&format!(", {sec}"));
            }
            s.push_str(&format!(" (chat «{}»)", h.proyecto_nombre));
            s
        }
    };
    match cuando {
        Some(f) => format!("{que}, {f}"),
        None => que,
    }
}

/// «3-oct-2026» de unos milisegundos desde 1970.
pub fn fecha_corta(ms: i64) -> Option<String> {
    // Antes del 2001 no es una fecha de verdad (un mensaje sin hora).
    if ms < 978_307_200_000 {
        return None;
    }
    let dias = ms.div_euclid(86_400_000);
    // Hinnant, al reves.
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    const MESES: [&str; 12] = [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
    ];
    Some(format!("{d}-{}-{y}", MESES[(m - 1) as usize]))
}

/// La cita como se guarda en la nota de la respuesta (`resto.fuentesIa`),
/// que el movil conserva tal cual.
pub fn cita_a_json(c: &Cita) -> Value {
    json!({"n": c.numero, "titulo": c.titulo(), "proyecto": c.hallazgo.proyecto_nombre, "pedido": c.pedido()})
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn args(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn valida_la_consulta() {
        let p = validar(&args(
            json!({"consulta": " cunetas pendiente ", "proyecto": "Obra"}),
        ))
        .unwrap();
        assert_eq!(p.consulta, "cunetas pendiente");
        assert_eq!(p.proyecto.as_deref(), Some("Obra"));
        let p = validar(&args(json!({"query": "plano", "proyecto": "todos"}))).unwrap();
        assert_eq!(p.proyecto, None);
        assert!(validar(&args(json!({}))).is_err());
        assert!(validar(&args(json!({"consulta": "   "}))).is_err());
        // Solo palabras vacias o de preguntar: nada que buscar.
        assert!(validar(&args(json!({"consulta": "¿qué dice de la?"}))).is_err());
    }

    #[test]
    fn con_la_nube_cada_hallazgo_lleva_fecha_y_hora_al_minuto() {
        use crate::documentos::indice::{Abrir, Hallazgo};
        let h = Hallazgo {
            nombre: "Plano.pdf".into(),
            tipo: Tipo::Chat,
            proyecto: "p".into(),
            proyecto_nombre: "Obra".into(),
            pagina: None,
            seccion: Some("adjunto".into()),
            // 3-oct-2026 19:42 UTC
            cuando: Some(1_790_985_600_000 + (19 * 60 + 42) * 60_000),
            fragmento: "Plano.pdf".into(),
            abrir: Abrir::Mensaje {
                proyecto: Some("p".into()),
                codigo: "c".into(),
            },
            puntos: 1.0,
        };
        let mut nube = Citas {
            lista: Vec::new(),
            desfase_ms: Some(-5 * 3_600_000),
        };
        let t = nube.anadir("plano", vec![h.clone()], None);
        assert!(t.contains("el sáb 3-oct-2026 14:42"), "{t}");
        // Caso negativo: el local (congelado) sigue con la fecha corta.
        let mut local = Citas::default();
        let t = local.anadir("plano", vec![h], None);
        assert!(t.contains("el 3-oct-2026.") && !t.contains("14:42"), "{t}");
    }

    #[test]
    fn la_fecha_corta() {
        assert_eq!(fecha_corta(0), None);
        // 2026-10-03 00:00 UTC
        assert_eq!(
            fecha_corta(1_790_985_600_000).as_deref(),
            Some("3-oct-2026")
        );
        assert_eq!(
            fecha_corta(1_790_985_600_000 + 86_399_999).as_deref(),
            Some("3-oct-2026")
        );
    }
}
