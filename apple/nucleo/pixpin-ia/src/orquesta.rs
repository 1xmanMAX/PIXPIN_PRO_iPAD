//! **El jefe y los trabajadores** (23-oct-2026, idea del usuario).
//!
//! «Un modelo más potente como jefe: divide bien el problema, ve una forma
//! de hacerlo, manda a los agentes menos inteligentes (Haiku, Flash) que
//! hagan las tareas de forma detallada, y ve el resultado: es el orquestador
//! y el supervisor a la vez, supervisa cómo van avanzando y corrige.»
//!
//! El patrón es el de orquestador y trabajadores de Anthropic («Building
//! effective agents»): para tareas que no se pueden prever paso a paso.
//!
//! 1. **Planear**: el jefe (Sonnet o DeepSeek V4 Pro) lee el pedido, el mapa
//!    de la app (`manual.rs`, va en las instrucciones) y lo que hay (el
//!    «Lienzo destino», los adjuntos) y devuelve un plan: tareas concretas,
//!    cada una con su prueba de «hecho».
//! 2. **Hacer**: cada tarea la hace un trabajador (Haiku o Flash) con el
//!    turno de siempre (`agente::turno_con`) y un contexto limpio: el
//!    pedido, su tarea y lo hecho antes; nada de la conversacion entera.
//! 3. **Supervisar cada tarea**: el jefe lee el informe (lo que dijo, lo que
//!    devolvieron las herramientas, con sus `ERROR` y los avisos del lint) y
//!    decide: bien, corregir (vuelve al trabajador con instrucciones
//!    concretas, como mucho [`CORRECCIONES`] veces) o no se puede.
//! 4. **Revisar el conjunto**: con todo hecho, el jefe mira el resultado
//!    (la vista del lienzo si ve imagenes; si no, los datos) contra el
//!    pedido; lo que falte, mas tareas (una ronda). Y contesta al usuario.
//!
//! Se ve en la burbuja: el plan con cada tarea marcandose ([`Evento::Tareas`]).
//! Si el jefe no contesta o no se le entiende, el turno sigue como siempre
//! con el trabajador solo: nunca peor que sin jefe.
//!
//! **Cuando**: solo si la app pone un jefe (`Opciones::jefe`, con la nube) y
//! el pedido es complejo o de varias partes ([`conviene`]). Lo simple lo hace
//! el trabajador solo: llamar al jefe ahi seria mas lento y caro sin ganar
//! nada.

use serde_json::Value;

use crate::agente::{self, Ejecutor, Evento, Final, Opciones};
use crate::herramientas::Herramienta;
use crate::motor::{Mensaje, Motor, Parte, Peticion, Rol, Seguir};

/// Tareas como mucho en un plan (y en total con las de la revision final).
pub const MAX_TAREAS: usize = 8;
/// Rondas de tareas que puede anadir la revision final (25-oct-2026: era
/// una, y la segunda revision que aun veia cosas a medias acababa el turno
/// diciendo «ahora voy a limpiar el resto…» sin hacerlo).
pub const RONDAS_FINALES: usize = 2;
/// Veces que el jefe puede devolver una tarea para corregirla.
pub const CORRECCIONES: usize = 2;
/// Lo mas largo de cada resultado de herramienta en el informe al jefe.
const LARGO_RESULTADO: usize = 400;

/// **Si conviene el jefe** para este pedido: complejo (`complejidad`) o de
/// varias partes (`agente::pedido_compuesto`).
pub fn conviene(pedido: &str) -> bool {
    crate::complejidad::clasificar(pedido).is_some() || agente::pedido_compuesto(pedido)
}

/// Como va una tarea (lo que ve el usuario en la burbuja).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoTarea {
    Pendiente,
    EnCurso,
    Corrigiendo,
    Hecha,
    NoSePudo,
}

/// Una tarea del plan, como se ensena.
#[derive(Debug, Clone, PartialEq)]
pub struct TareaVisible {
    pub titulo: String,
    pub estado: EstadoTarea,
}

/// Una tarea del plan del jefe.
#[derive(Debug, Clone, PartialEq)]
pub struct Tarea {
    pub titulo: String,
    pub instrucciones: String,
    pub hecho_si: String,
}

/// El plan del jefe.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlanDelJefe {
    pub entendido: String,
    pub tareas: Vec<Tarea>,
}

/// Lo que decide el jefe de una tarea hecha.
#[derive(Debug, Clone, PartialEq)]
pub enum Veredicto {
    Bien,
    Corregir(String),
    NoSePuede(String),
}

/// La revision final del jefe.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cierre {
    pub faltan: Vec<Tarea>,
    pub respuesta: String,
}

// ---------------------------------------------------------------------------
// Lo que se le pide al jefe

/// Quien es el jefe y como contesta (va delante de las instrucciones de
/// siempre, que traen el mapa de la app).
pub const SISTEMA_JEFE: &str = "Eres el JEFE del asistente de PixPin: planeas, repartes y supervisas; no usas herramientas. \
Unos trabajadores (modelos más pequeños, con todas las herramientas de la app) hacen cada tarea que les mandes y te informan. \
Divide el pedido en pocas tareas concretas (como mucho 8), cada una de un solo sitio de la app, con instrucciones detalladas \
(qué leer, qué herramientas, qué valores, dónde) y una prueba de «hecho» que se pueda comprobar con los resultados de las herramientas. \
Lo que el modelo sabe hacer solo (traducir, redactar, calcular) va dentro de la tarea que lo escribe. \
Contesta SOLO con el JSON que se te pida, sin texto alrededor.\n\n";

/// El pedido del plan.
pub const PEDIR_PLAN: &str = "Haz el plan para el último pedido del usuario. JSON: \
{\"entendido\":\"una frase\",\"tareas\":[{\"titulo\":\"corto\",\"instrucciones\":\"detalladas\",\"hecho_si\":\"comprobable\"}]}";

/// El informe de una tarea y lo que se pide al jefe.
pub fn pedir_veredicto(pedido: &str, tarea: &Tarea, informe: &str) -> String {
    format!(
        "Pedido del usuario: {pedido}\n\nTarea: {}\nInstrucciones: {}\nSe da por hecha si: {}\n\nInforme del trabajador:\n{informe}\n\n\
Decide con los resultados (no con lo que dice el trabajador): ¿está hecha y bien (sin ERROR sin resolver, sin avisos de solapes, \
textos que se salen o cosas torcidas)? JSON: {{\"veredicto\":\"bien\"|\"corregir\"|\"no_se_puede\",\"motivo\":\"corto\",\
\"correccion\":\"si es corregir: qué cambiar exactamente\"}}",
        tarea.titulo, tarea.instrucciones, tarea.hecho_si
    )
}

/// La revision final.
pub fn pedir_cierre(pedido: &str, informes: &str, con_vista: bool) -> String {
    format!(
        "Pedido del usuario: {pedido}\n\nLo hecho, tarea a tarea:\n{informes}\n\n{}\
Repasa el pedido punto por punto: ¿falta algo o algo quedó mal (desalineado, encimado, sin traducir, incompleto)? \
JSON: {{\"faltan\":[{{\"titulo\":\"\",\"instrucciones\":\"\",\"hecho_si\":\"\"}}],\
\"respuesta\":\"al usuario, en su idioma, una o dos frases de lo hecho (y lo que no se pudo, si algo)\"}}. \
Si está todo, \"faltan\":[].",
        if con_vista {
            "La imagen adjunta es cómo quedó el lienzo ahora.\n\n"
        } else {
            ""
        }
    )
}

// ---------------------------------------------------------------------------
// Leer lo que contesta el jefe

/// El primer objeto JSON del texto (el jefe a veces lo envuelve en ```).
pub fn json_de(texto: &str) -> Option<Value> {
    let t = texto.trim();
    let ini = t.find('{')?;
    let fin = t.rfind('}')?;
    (fin > ini)
        .then(|| serde_json::from_str(&t[ini..=fin]).ok())
        .flatten()
}

fn texto_de(v: &Value, clave: &str) -> String {
    v.get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn tareas_de(v: Option<&Value>) -> Vec<Tarea> {
    v.and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(|t| {
                    let tarea = Tarea {
                        titulo: texto_de(t, "titulo"),
                        instrucciones: texto_de(t, "instrucciones"),
                        hecho_si: texto_de(t, "hecho_si"),
                    };
                    (!tarea.titulo.is_empty() || !tarea.instrucciones.is_empty()).then_some(tarea)
                })
                .take(MAX_TAREAS)
                .collect()
        })
        .unwrap_or_default()
}

/// **El plan**, si se entiende y trae al menos una tarea.
pub fn leer_plan(texto: &str) -> Option<PlanDelJefe> {
    let v = json_de(texto)?;
    let plan = PlanDelJefe {
        entendido: texto_de(&v, "entendido"),
        tareas: tareas_de(v.get("tareas")),
    };
    (!plan.tareas.is_empty()).then_some(plan)
}

/// **El veredicto**; lo que no se entiende cuenta como «bien» (no se
/// atasca una tarea por una respuesta rara del jefe).
pub fn leer_veredicto(texto: &str) -> Veredicto {
    let Some(v) = json_de(texto) else {
        return Veredicto::Bien;
    };
    let motivo = texto_de(&v, "motivo");
    match texto_de(&v, "veredicto").to_lowercase().as_str() {
        "corregir" => {
            let c = texto_de(&v, "correccion");
            Veredicto::Corregir(if c.is_empty() { motivo } else { c })
        }
        "no_se_puede" | "no se puede" => Veredicto::NoSePuede(motivo),
        _ => Veredicto::Bien,
    }
}

/// **La revision final.**
pub fn leer_cierre(texto: &str) -> Cierre {
    match json_de(texto) {
        Some(v) => Cierre {
            faltan: tareas_de(v.get("faltan")),
            respuesta: texto_de(&v, "respuesta"),
        },
        None => Cierre::default(),
    }
}

// ---------------------------------------------------------------------------
// El turno con jefe

fn cortar(t: &str, n: usize) -> String {
    if t.chars().count() <= n {
        t.to_string()
    } else {
        format!("{}…", t.chars().take(n).collect::<String>())
    }
}

/// Lo que se le manda al jefe: sus instrucciones, las de siempre (el mapa
/// de la app) y la conversacion sin el sistema de antes.
fn conversacion_para_el_jefe(historial: &[Mensaje]) -> Vec<Mensaje> {
    let mut v = vec![Mensaje::sistema(format!(
        "{SISTEMA_JEFE}{}",
        agente::instrucciones()
    ))];
    v.extend(
        historial
            .iter()
            .filter(|m| m.rol != Rol::Sistema && m.rol != Rol::Herramienta)
            .cloned(),
    );
    v
}

/// Pregunta al jefe (sin herramientas) y devuelve su texto.
fn preguntar_al_jefe(jefe: &dyn Motor, mensajes: Vec<Mensaje>, max_tokens: u32) -> Option<String> {
    let mut p = Peticion::nueva(mensajes);
    p.max_tokens = max_tokens;
    p.temperatura = 0.3;
    match jefe.completar(&p, &mut |_| Seguir::Si) {
        Ok(r) => Some(r.texto),
        Err(e) => {
            tracing::warn!(%e, "asistente: el jefe no contesto");
            None
        }
    }
}

/// Lo que se juntó de un trabajador.
#[derive(Default)]
struct Informe {
    dicho: String,
    resultados: Vec<(String, bool, String)>,
}

impl Informe {
    fn para_el_jefe(&self) -> String {
        let mut s = String::new();
        for (n, ok, t) in &self.resultados {
            s.push_str(&format!(
                "- {n}: {}{}\n",
                if *ok { "" } else { "ERROR " },
                cortar(t, LARGO_RESULTADO)
            ));
        }
        if s.is_empty() {
            s.push_str("- (no usó ninguna herramienta)\n");
        }
        s.push_str(&format!("Dijo: {}", cortar(self.dicho.trim(), 600)));
        s
    }
}

/// El usuario: el pedido de verdad (con sus imagenes y el «Lienzo destino»).
fn ultimo_del_usuario(historial: &[Mensaje]) -> Option<&Mensaje> {
    historial.iter().rev().find(|m| m.rol == Rol::Usuario)
}

/// **Un trabajador hace una tarea**: el turno de siempre con un contexto
/// limpio (el pedido con lo suyo, su tarea, lo hecho antes y, si es una
/// correccion, lo que dijo el jefe).
#[allow(clippy::too_many_arguments)]
fn trabajar(
    trabajador: &dyn Motor,
    herramientas: &[Herramienta],
    ejecutor: &mut dyn Ejecutor,
    opciones: &Opciones,
    pedido: &Mensaje,
    tarea: &Tarea,
    (n, total): (usize, usize),
    antes: &str,
    correccion: Option<&str>,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> (Informe, Result<Final, crate::ErrorIa>) {
    let mut encargo = format!(
        "Eres un trabajador del asistente de PixPin. El jefe dividió el pedido del usuario en tareas; haz SOLO la tuya, \
con detalle y bien, usando las herramientas, y comprueba con sus resultados que quedó como se pide. \
Termina con una o dos frases de lo que hiciste.\n\nTu tarea ({n} de {total}): {}\n{}\nSe da por hecha si: {}",
        tarea.titulo, tarea.instrucciones, tarea.hecho_si
    );
    if !antes.trim().is_empty() {
        encargo.push_str(&format!("\n\nLo ya hecho por otros trabajadores:\n{antes}"));
    }
    if let Some(c) = correccion {
        encargo.push_str(&format!(
            "\n\nEl jefe revisó tu trabajo y pide corregir: {c}"
        ));
    }
    encargo.push_str(&format!(
        "\n\n{}\n{}",
        crate::guardas::MARCA_PEDIDO_ORIGINAL,
        pedido.texto_entero()
    ));
    // Las fotos del pedido tambien (calcar, leer una tabla...).
    let mut m = Mensaje::usuario(encargo);
    for p in &pedido.partes {
        if matches!(p, Parte::Imagen { .. }) {
            m.partes.insert(0, p.clone());
        }
    }
    let mut historial = vec![Mensaje::sistema(agente::instrucciones()), m];
    let o = Opciones {
        ojos: opciones.ojos,
        buscar: opciones.buscar,
        // El trabajador comprueba lo suyo con los resultados; la revision
        // de lo que se ve la hace el jefe al final.
        repasar: false,
        revisar: 0,
        ..Default::default()
    };
    let mut informe = Informe::default();
    let r = {
        let mut filtro = |e: Evento| -> Seguir {
            match e {
                // Lo que dice el trabajador va al jefe, no a la burbuja.
                Evento::Texto(t) => {
                    informe.dicho.push_str(&t);
                    Seguir::Si
                }
                Evento::Resultado { nombre, ok, texto } => {
                    informe.resultados.push((nombre.clone(), ok, texto.clone()));
                    al_evento(Evento::Resultado { nombre, ok, texto })
                }
                Evento::Pensando(_) | Evento::PensoListo { .. } | Evento::Peticion(_) => Seguir::Si,
                otro => al_evento(otro),
            }
        };
        agente::turno_con(
            trabajador,
            &mut historial,
            herramientas,
            ejecutor,
            &o,
            &mut filtro,
        )
    };
    if let Ok(f) = &r
        && informe.dicho.trim().is_empty()
    {
        informe.dicho = f.texto.clone();
    }
    (informe, r)
}

/// **Lo que falta segun la revision final**: sus tareas, o, si no trae
/// ninguna pero su respuesta anuncia trabajo o dice que quedo a medias
/// ([`crate::guardas::anuncia_pendiente`]), una tarea con eso: lo anunciado
/// se hace, no se promete.
pub fn faltan_del_cierre(cierre: &Cierre) -> Vec<Tarea> {
    if !cierre.faltan.is_empty() {
        return cierre.faltan.clone();
    }
    if !crate::guardas::anuncia_pendiente(&cierre.respuesta) {
        return Vec::new();
    }
    vec![Tarea {
        titulo: "Terminar lo que quedó pendiente".into(),
        instrucciones: format!(
            "La revisión final dijo: «{}». Hazlo ahora con las herramientas, editando lo que ya hay (sin dibujar otra versión al lado \
ni inventar contenido que el usuario no dio).",
            cortar(cierre.respuesta.trim(), 600)
        ),
        hecho_si: "lo que la revisión dijo que faltaba queda hecho y se ve en los resultados"
            .into(),
    }]
}

/// **La respuesta final sin promesas**: si ya no quedan rondas y aun asi
/// anuncia trabajo, se dice claro que no quedo terminado (el turno acaba
/// aqui: prometer «ahora lo hago» seria mentir).
pub fn sin_promesas(cierre: &Cierre) -> String {
    let r = cierre.respuesta.trim();
    if cierre.faltan.is_empty() && !crate::guardas::anuncia_pendiente(r) {
        return r.to_string();
    }
    let falta: Vec<&str> = cierre.faltan.iter().map(|t| t.titulo.as_str()).collect();
    if falta.is_empty() {
        format!("No llegué a terminarlo del todo. Lo último que vi: {r} Dime si sigo.")
    } else {
        format!(
            "{r}\n\nNo llegué a terminar: {}. Dime si sigo.",
            falta.join("; ")
        )
    }
}

fn visibles(plan: &[Tarea], estados: &[EstadoTarea]) -> Vec<TareaVisible> {
    plan.iter()
        .zip(estados)
        .map(|(t, e)| TareaVisible {
            titulo: t.titulo.clone(),
            estado: *e,
        })
        .collect()
}

/// **El turno con jefe.** `None` si el jefe no dio un plan que se entienda:
/// quien llama sigue con el turno de siempre.
pub fn turno(
    trabajador: &dyn Motor,
    jefe: &dyn Motor,
    historial: &mut Vec<Mensaje>,
    herramientas: &[Herramienta],
    ejecutor: &mut dyn Ejecutor,
    opciones: &Opciones,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Option<Result<Final, crate::ErrorIa>> {
    let pedido_msg = ultimo_del_usuario(historial)?.clone();
    let pedido = agente::pedido_del_usuario(historial);
    // 1. El plan.
    if al_evento(Evento::Llamando("jefe:planear".into())) == Seguir::Parar {
        return Some(Ok(Final {
            texto: String::new(),
            herramientas_hechas: 0,
            reintentos: 0,
            cortado: true,
        }));
    }
    let mut mensajes = conversacion_para_el_jefe(historial);
    mensajes.push(Mensaje::usuario(PEDIR_PLAN));
    let plan = leer_plan(&preguntar_al_jefe(jefe, mensajes, 4096)?)?;
    tracing::info!(tareas = plan.tareas.len(), entendido = %plan.entendido, "asistente: plan del jefe");
    let mut tareas = plan.tareas;
    let mut estados = vec![EstadoTarea::Pendiente; tareas.len()];
    let mut hechas = 0;
    let mut reintentos = 0;
    let mut hecho_antes = String::new();
    let mut cortado = false;
    let mut i = 0;
    let mut rondas_finales = 0;
    loop {
        // 2-3. Cada tarea, con su supervision.
        while i < tareas.len() && !cortado {
            estados[i] = EstadoTarea::EnCurso;
            if al_evento(Evento::Tareas(visibles(&tareas, &estados))) == Seguir::Parar {
                cortado = true;
                break;
            }
            let total = tareas.len();
            let mut correccion: Option<String> = None;
            let mut intentos = 0;
            loop {
                let (informe, r) = trabajar(
                    trabajador,
                    herramientas,
                    ejecutor,
                    opciones,
                    &pedido_msg,
                    &tareas[i],
                    (i + 1, total),
                    &hecho_antes,
                    correccion.as_deref(),
                    al_evento,
                );
                match &r {
                    Ok(f) => {
                        hechas += f.herramientas_hechas;
                        reintentos += f.reintentos;
                        cortado |= f.cortado;
                    }
                    Err(e) => tracing::warn!(%e, "asistente: el trabajador fallo"),
                }
                if cortado {
                    break;
                }
                let texto = informe.para_el_jefe();
                let veredicto = if r.is_err() {
                    Veredicto::Corregir(
                        "el trabajador no pudo acabar; inténtalo de nuevo con otra forma".into(),
                    )
                } else {
                    al_evento(Evento::Llamando("jefe:supervisar".into()));
                    let mut m = conversacion_para_el_jefe(&[]);
                    m.push(Mensaje::usuario(pedir_veredicto(
                        &pedido, &tareas[i], &texto,
                    )));
                    preguntar_al_jefe(jefe, m, 1024).map_or(Veredicto::Bien, |t| leer_veredicto(&t))
                };
                match veredicto {
                    Veredicto::Corregir(c) if intentos < CORRECCIONES => {
                        intentos += 1;
                        estados[i] = EstadoTarea::Corrigiendo;
                        al_evento(Evento::Tareas(visibles(&tareas, &estados)));
                        tracing::info!(tarea = %tareas[i].titulo, %c, "asistente: el jefe pide corregir");
                        correccion = Some(c);
                    }
                    Veredicto::NoSePuede(m) => {
                        estados[i] = EstadoTarea::NoSePudo;
                        hecho_antes
                            .push_str(&format!("- {}: no se pudo ({m})\n", tareas[i].titulo));
                        break;
                    }
                    _ => {
                        estados[i] = EstadoTarea::Hecha;
                        hecho_antes.push_str(&format!(
                            "- {}: {}\n",
                            tareas[i].titulo,
                            cortar(informe.dicho.trim(), 300)
                        ));
                        break;
                    }
                }
            }
            al_evento(Evento::Tareas(visibles(&tareas, &estados)));
            i += 1;
        }
        if cortado {
            break;
        }
        // 4. La revision del conjunto (con la vista si el jefe ve).
        al_evento(Evento::Llamando("jefe:revisar".into()));
        let vista = if jefe.ve_imagenes() && opciones.ojos {
            ejecutor
                .ver(&crate::ojos::Mirada::para_revisar(crate::ojos::Zona::Turno))
                .ok()
        } else {
            None
        };
        let texto = pedir_cierre(&pedido, &hecho_antes, vista.is_some());
        let mut m = conversacion_para_el_jefe(&[]);
        m.push(match &vista {
            Some(v) => Mensaje::usuario_con_imagen(texto, v.png.clone(), "image/png"),
            None => Mensaje::usuario(texto),
        });
        let cierre = preguntar_al_jefe(jefe, m, 2048)
            .map(|t| leer_cierre(&t))
            .unwrap_or_default();
        let faltan = faltan_del_cierre(&cierre);
        if rondas_finales < RONDAS_FINALES && !faltan.is_empty() && tareas.len() < MAX_TAREAS {
            rondas_finales += 1;
            for t in faltan.into_iter().take(MAX_TAREAS - tareas.len()) {
                tareas.push(t);
                estados.push(EstadoTarea::Pendiente);
            }
            continue;
        }
        let respuesta = if cierre.respuesta.is_empty() {
            let hechas_n = estados.iter().filter(|e| **e == EstadoTarea::Hecha).count();
            format!("Hecho: {hechas_n} de {} tareas.", estados.len())
        } else {
            sin_promesas(&cierre)
        };
        al_evento(Evento::Texto(respuesta.clone()));
        historial.push(Mensaje::texto(Rol::Asistente, respuesta.clone()));
        if hechas > 0 {
            ejecutor.cerrar_turno();
        }
        return Some(Ok(Final {
            texto: respuesta,
            herramientas_hechas: hechas,
            reintentos,
            cortado: false,
        }));
    }
    Some(Ok(Final {
        texto: String::new(),
        herramientas_hechas: hechas,
        reintentos,
        cortado: true,
    }))
}

#[cfg(test)]
mod pruebas;
