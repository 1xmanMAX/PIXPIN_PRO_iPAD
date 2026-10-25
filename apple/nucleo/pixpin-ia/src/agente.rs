//! **Un turno del asistente**: pedir al modelo, validar lo que pide, hacerlo
//! y devolverle el resultado, hasta que conteste con texto.
//!
//! ```text
//! usuario ─▶ modelo ─▶ ¿llamadas? ──no──▶ texto final
//!                         │ si
//!                         ▼
//!                     validar ──mal──▶ error al modelo (reintento, max. 2)
//!                         │ bien
//!                         ▼
//!                  Ejecutor (la UI) ──▶ resultado al modelo ──▶ otra vuelta
//! ```
//!
//! El bucle no sabe dibujar ni leer PDFs: eso lo hace el [`Ejecutor`], que
//! implementa el chat con el lienzo y el OCR de verdad. Asi el mismo bucle
//! sirve para las pruebas (con un ejecutor de mentira) y, portado, para
//! Android.

use crate::ErrorIa;
use crate::contexto;
use crate::herramientas::{ErrorHerramienta, Herramienta, Llamada, colores_del_pedido, validar};
use crate::hoja_trabajo::HojaDeTrabajo;
use crate::motor::{Fin, Mensaje, Motor, Peticion, Respuesta, Rol, Seguir, Trozo};
use crate::pensar::{Cronometro, Plan, Separador, cortar, instrucciones_del_plan, separar};

/// Cuantas veces se deja al modelo corregir una llamada mal hecha en un
/// mismo turno. Con dos, en las medidas el 2B siempre acabo bien; mas solo
/// alarga la espera cuando el modelo no sabe hacerlo.
pub const REINTENTOS: usize = 2;

/// Cuantas vueltas modelo→herramienta→modelo como mucho en un turno (un
/// modelo pequeno puede quedarse llamando en bucle).
pub const VUELTAS: usize = 4;

/// Lo que pasa durante un turno, para pintarlo en vivo.
#[derive(Debug, Clone, PartialEq)]
pub enum Evento {
    /// Texto nuevo de la respuesta.
    Texto(String),
    /// El modelo empezo a pedir una herramienta.
    Llamando(String),
    /// Una llamada validada que se va a ejecutar, con sus avisos cosmeticos.
    Herramienta { llamada: Llamada, avisos: Vec<String> },
    /// Lo que devolvio el ejecutor.
    Resultado { nombre: String, ok: bool, texto: String },
    /// La llamada estaba mal; se le pide al modelo que la corrija.
    Reintento { motivo: String },
    /// Un trozo de lo que esta pensando el modelo (se pinta en vivo en la
    /// seccion plegable «Pensando…»).
    Pensando(String),
    /// Acabo de pensar, tras tantos segundos («Penso 12 s»).
    PensoListo { segundos: f32 },
    /// La peticion mejorada, antes de actuar.
    Peticion(Plan),
    /// Se aligero el contexto al pasar del 70 % (tokens estimados).
    Reinicio { antes: u32, despues: u32 },
}

/// **El punto de enganche de la UI**: hace de verdad cada herramienta.
pub trait Ejecutor {
    /// Ejecuta la llamada. `Ok` lleva un resumen corto para el modelo
    /// («dibujadas 4 figuras», o el texto del OCR); `Err`, el motivo.
    fn ejecutar(&mut self, llamada: &Llamada) -> Result<String, String>;

    /// **Varias llamadas de una misma respuesta**, con sus resultados en el
    /// mismo orden. Por omision, una tras otra; un ejecutor puede hacer a la
    /// vez las que no dependen entre si (el OCR de dos PDF) para que el
    /// modelo espere solo a la mas lenta.
    fn ejecutar_varias(&mut self, llamadas: &[Llamada]) -> Vec<Result<String, String>> {
        llamadas.iter().map(|l| self.ejecutar(l)).collect()
    }

    /// **Pinta el lienzo** (o la hoja anotada) para que el modelo lo vea
    /// (`ver_lienzo` y la revision, `ojos.rs`). Por omision no hay nada que
    /// mirar: el turno sigue sin vista.
    fn ver(&mut self, _mirada: &crate::ojos::Mirada) -> Result<crate::ojos::Vista, String> {
        Err("aqui no hay lienzo que mirar".into())
    }

    /// Acabo un turno que cambio el lienzo: lo que hizo (y lo que corrigio al
    /// revisar) queda en un solo paso de deshacer si nadie lo toco entre
    /// medias. Por omision, nada.
    fn cerrar_turno(&mut self) {}
}

/// Como acabo el turno.
#[derive(Debug, Clone, PartialEq)]
pub struct Final {
    pub texto: String,
    pub herramientas_hechas: usize,
    pub reintentos: usize,
    pub cortado: bool,
}

/// **Lo que el turno sabe hacer ademas de contestar** (pedido del 13-oct:
/// ver lo que piensa, la peticion mejorada, la hoja de trabajo y no quedarse
/// sin contexto). Por omision todo apagado: el turno de siempre.
#[derive(Default)]
pub struct Opciones<'a> {
    /// Antes de actuar, una vuelta corta en la que el modelo escribe que
    /// entendio, que hara y con que herramientas ([`Plan`]).
    pub planificar: bool,
    /// Que piense en esa vuelta (el modo pensar de Qwen; Claude y DeepSeek
    /// piensan por su cuenta). Con el 0.8B se puede apagar si va lento.
    pub pensar: bool,
    /// La hoja de trabajo del chat: va en las instrucciones de sistema.
    pub hoja: Option<&'a HojaDeTrabajo>,
    /// Los tokens de la ventana del motor. 0: no se mide.
    pub ventana: u32,
    /// Buscar en la conversacion guardada: con esto el modelo tiene la
    /// herramienta `buscar_historial`.
    pub buscar: Option<&'a dyn Fn(&str) -> String>,
    /// Todas las herramientas que podria usar (el catalogo entero). La
    /// vuelta de planear elige entre ellas, y las que nombre se anaden a las
    /// del turno aunque la seleccion por palabras no las diera.
    pub ampliar: Option<&'a [Herramienta]>,
    /// **El camino rapido** (14-oct): con el «ahora» local, las ordenes
    /// simples («abre la galería», «apunta comprar pan mañana») se hacen sin
    /// preguntar al modelo ([`crate::rapido`]). `None`: siempre el modelo.
    pub rapido: Option<crate::fechas::Momento>,
    /// **Los ojos** (15-oct, `ojos.rs`): ofrecer `ver_lienzo` si el motor ve
    /// imagenes.
    pub ojos: bool,
    /// Rondas de revision tras un turno que dibujo o anoto (0: ninguna; como
    /// mucho [`REVISIONES_MAXIMAS`]). Solo con `ojos` y un motor que ve.
    pub revisar: usize,
}

/// Lo mas largo que se devuelve al modelo de un resultado de herramienta: lo
/// demas se recorta diciendolo (cada letra se vuelve a leer en cada vuelta).
pub const RESULTADO_MAXIMO: usize = 4000;

/// Recorta un resultado para el modelo, avisando de que hay mas.
pub fn recortar_resultado(texto: String) -> String {
    if texto.chars().count() <= RESULTADO_MAXIMO {
        return texto;
    }
    let mut s: String = texto.chars().take(RESULTADO_MAXIMO).collect();
    s.push_str("\n[...recortado: el usuario tiene el resultado entero]");
    s
}

/// **Intenta el camino rapido**: si la orden es simple, la hace con el
/// ejecutor y devuelve el final del turno; `None` si no era simple o si no
/// salio (entonces contesta el modelo, sin que se haya dicho nada).
fn turno_rapido(
    historial: &mut Vec<Mensaje>,
    ejecutor: &mut dyn Ejecutor,
    ahora: crate::fechas::Momento,
    // Con la nube, tambien las ordenes de `acciones::SOLO_NUBE`.
    nube: bool,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Option<Final> {
    let ultimo = historial.last().filter(|m| m.rol == Rol::Usuario && !m.tiene_imagen())?;
    let orden = crate::rapido::interpretar_con(&ultimo.texto_entero(), ahora, nube)?;
    let cruda = crate::motor::LlamadaCruda {
        id: "rapido-1".into(),
        nombre: orden.herramienta.into(),
        argumentos: serde_json::Value::Object(orden.args).to_string(),
    };
    let v = validar(&cruda, &crate::herramientas::catalogo()).ok()?;
    // Lo que pregunta antes nunca va por aqui (no deberia llegar: el
    // interprete no entiende «borra»), por si acaso.
    if matches!(&v.llamada, Llamada::App(a) if a.confirmar.is_some()) {
        return None;
    }
    let texto = match ejecutor.ejecutar(&v.llamada) {
        Ok(t) => t,
        // La orden se entendio pero no casa con lo que hay (una tarea que no
        // esta, una fecha que no existe, una lista de otro chat): el motivo
        // ya es la respuesta, con lo que si hay. Preguntar al modelo solo lo
        // repetiria mas tarde (medido con el 2B: ademas lo apuntaba en otro
        // sitio sin decirlo).
        Err(motivo) if motivo.contains(crate::tareas::NO_HICE_NADA) => {
            al_evento(Evento::Texto(motivo.clone()));
            historial.push(Mensaje::texto(Rol::Asistente, motivo.clone()));
            return Some(Final {
                texto: motivo,
                herramientas_hechas: 0,
                reintentos: 0,
                cortado: false,
            });
        }
        Err(motivo) => {
            tracing::info!(%motivo, herramienta = %cruda.nombre, "asistente: el camino rapido no pudo, contesta el modelo");
            return None;
        }
    };
    al_evento(Evento::Herramienta {
        llamada: v.llamada.clone(),
        avisos: Vec::new(),
    });
    al_evento(Evento::Resultado {
        nombre: v.llamada.nombre().to_string(),
        ok: true,
        texto: texto.clone(),
    });
    let respuesta = if texto.trim().is_empty() || texto.starts_with("hecho") { HECHO.to_string() } else { texto };
    al_evento(Evento::Texto(respuesta.clone()));
    // La conversacion queda como si el modelo lo hubiera hecho.
    let mut m = Mensaje::texto(Rol::Asistente, "");
    m.llamadas = vec![cruda.clone()];
    historial.push(m);
    historial.push(Mensaje::resultado(&cruda.id, respuesta.clone()));
    historial.push(Mensaje::texto(Rol::Asistente, respuesta.clone()));
    Some(Final {
        texto: respuesta,
        herramientas_hechas: 1,
        reintentos: 0,
        cortado: false,
    })
}

/// Tope de la vuelta de planear: sin pensar, cinco lineas; pensando, unos
/// 40 s del 2B en la MX250 (~15 tok/s). Medido el 13-oct: con 1536 pensaba
/// 110-125 s en ingles y se quedaba sin sitio para el plan; si se acaba sin
/// plan, una vuelta corta sin pensar lo escribe con lo pensado.
pub const TOKENS_PLAN: u32 = 320;
pub const TOKENS_PLAN_PENSANDO: u32 = 640;

/// **Un turno.** `historial` ya trae el mensaje nuevo del usuario al final;
/// al acabar trae tambien todo lo que dijo el asistente (incluidas las
/// llamadas y sus resultados), listo para la vuelta siguiente.
///
/// Nunca acaba con la burbuja vacia: si el modelo no dice nada, se dice algo
/// por el ([`SIN_PALABRAS`], [`HECHO`]).
pub fn turno(
    motor: &dyn Motor,
    historial: &mut Vec<Mensaje>,
    herramientas: &[Herramienta],
    ejecutor: &mut dyn Ejecutor,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Result<Final, ErrorIa> {
    turno_con(motor, historial, herramientas, ejecutor, &Opciones::default(), al_evento)
}

/// **Un turno con pensar, plan, hoja y contexto medido** ([`Opciones`]).
pub fn turno_con(
    motor: &dyn Motor,
    historial: &mut Vec<Mensaje>,
    herramientas: &[Herramienta],
    ejecutor: &mut dyn Ejecutor,
    opciones: &Opciones,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Result<Final, ErrorIa> {
    let mut crono = Cronometro::default();
    let r = turno_dentro(motor, historial, herramientas, ejecutor, opciones, &mut crono, al_evento);
    // Penso y no llego a decir nada mas (cortado, o solo pensamiento).
    if let Some(segundos) = crono.otra_cosa() {
        al_evento(Evento::PensoListo { segundos });
    }
    r
}

/// Pasa un trozo del motor al turno: separa el pensamiento escrito entre
/// etiquetas, lo cronometra y lo convierte en eventos. Con `plan`, el texto
/// va ahi (la vuelta de planear: su texto es el plan, no la respuesta).
fn repartir(
    t: &Trozo,
    sep: &mut Separador,
    crono: &mut Cronometro,
    mut plan: Option<&mut String>,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Seguir {
    let mut seguir = Seguir::Si;
    for x in sep.meter(t) {
        let s = match x {
            Trozo::Pensando(s) => {
                crono.pensando(&s);
                // Un pensamiento en blanco no abre la seccion «Pensando…».
                if crono.penso() {
                    al_evento(Evento::Pensando(s))
                } else {
                    Seguir::Si
                }
            }
            otro => {
                if let Some(segundos) = crono.otra_cosa() {
                    al_evento(Evento::PensoListo { segundos });
                }
                match otro {
                    Trozo::Texto(s) => match plan.as_deref_mut() {
                        Some(destino) => {
                            destino.push_str(&s);
                            Seguir::Si
                        }
                        None => al_evento(Evento::Texto(s)),
                    },
                    Trozo::Llamando { nombre } => al_evento(Evento::Llamando(nombre)),
                    Trozo::Pensando(_) => Seguir::Si,
                }
            }
        };
        if s == Seguir::Parar {
            seguir = Seguir::Parar;
        }
    }
    seguir
}

/// **La vuelta de planear.** `Ok(None)`: el usuario paro.
fn planear(
    motor: &dyn Motor,
    historial: &[Mensaje],
    herramientas: &[Herramienta],
    opciones: &Opciones,
    crono: &mut Cronometro,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Result<Option<Plan>, ErrorIa> {
    let u = historial.iter().rposition(|m| m.rol == Rol::Usuario);
    let pedido = u.map(|u| historial[u].texto_entero()).unwrap_or_default();
    let mut texto = String::new();
    if let Some(h) = opciones.hoja.filter(|h| !h.esta_vacia()) {
        texto.push_str(h.a_texto().trim_end());
        texto.push_str("\n\n");
    }
    let antes: Vec<&Mensaje> = historial[..u.unwrap_or(0)]
        .iter()
        .filter(|m| matches!(m.rol, Rol::Usuario | Rol::Asistente) && !m.texto_entero().trim().is_empty())
        .collect();
    if !antes.is_empty() {
        texto.push_str("## Conversacion reciente\n");
        for m in &antes[antes.len().saturating_sub(2)..] {
            let quien = if m.rol == Rol::Usuario { "Usuario" } else { "Asistente" };
            texto.push_str(&format!("{quien}: {}\n", cortar(&m.texto_entero(), 400)));
        }
        texto.push('\n');
    }
    texto.push_str("## Mensaje nuevo\n");
    texto.push_str(&pedido);
    if u.is_some_and(|u| historial[u].tiene_imagen()) {
        texto.push_str("\n(adjunta una foto)");
    }
    let mut p = Peticion::nueva(vec![Mensaje::sistema(instrucciones_del_plan(herramientas)), Mensaje::usuario(texto)]);
    p.pensar = opciones.pensar;
    p.max_tokens = if opciones.pensar { TOKENS_PLAN_PENSANDO } else { TOKENS_PLAN };
    p.temperatura = 0.6;
    let mut sep = Separador::nuevo();
    let mut plan_texto = String::new();
    let r = motor.completar(&p, &mut |t| repartir(t, &mut sep, crono, Some(&mut plan_texto), al_evento));
    for t in sep.acabar() {
        repartir(&t, &mut Separador::nuevo(), crono, Some(&mut plan_texto), al_evento);
    }
    let r = match r {
        Ok(r) => r,
        // Sin plan no se deja de contestar: se sigue con el pedido tal cual
        // (si el motor esta caido, la vuelta de actuar lo dira).
        Err(e) => {
            tracing::warn!(%e, "asistente: la vuelta de planear fallo");
            let plan = Plan::por_omision(&pedido);
            al_evento(Evento::Peticion(plan.clone()));
            return Ok(Some(plan));
        }
    };
    // Un motor que no manda el pensamiento a trozos lo trae al final.
    if !crono.penso() && !r.pensamiento.trim().is_empty() {
        crono.pensando(&r.pensamiento);
        al_evento(Evento::Pensando(r.pensamiento.clone()));
    }
    if r.fin == Fin::Cortado {
        return Ok(None);
    }
    if plan_texto.trim().is_empty() {
        plan_texto = r.texto.clone();
    }
    // Un `</think>` suelto en el plan: lo de antes era pensamiento.
    if plan_texto.contains(CIERRE_PENSAR) {
        let (pensado, resto) = separar(&plan_texto);
        if !crono.penso() && !pensado.trim().is_empty() {
            crono.pensando(&pensado);
            al_evento(Evento::Pensando(pensado));
        }
        plan_texto = resto;
    }
    if let Some(segundos) = crono.otra_cosa() {
        al_evento(Evento::PensoListo { segundos });
    }
    let mut plan = Plan::leer(&plan_texto, herramientas);
    // Penso hasta agotar su tope sin llegar a escribir el plan (medido con
    // el 2B, 13-oct: 1536 tokens de «Thinking Process…» y ni una linea):
    // una vuelta corta SIN pensar, con lo que penso, que si lo escribe.
    if plan.is_none() && crono.penso() {
        let mut p2 = p.clone();
        p2.pensar = false;
        p2.max_tokens = TOKENS_PLAN;
        if let Some(m) = p2.mensajes.last_mut() {
            *m = Mensaje::usuario(format!(
                "{}\n\n## Lo que ya pensaste\n{}\n\nAhora escribe SOLO las cinco lineas.",
                m.texto_entero(),
                crate::pensar::resumir(&crono.texto, 900)
            ));
        }
        let mut otro = String::new();
        let r2 = motor.completar(&p2, &mut |t| match t {
            Trozo::Texto(s) => {
                otro.push_str(s);
                Seguir::Si
            }
            _ => Seguir::Si,
        });
        if let Ok(r2) = r2 {
            if otro.trim().is_empty() {
                otro = r2.texto;
            }
            plan = Plan::leer(&separar(&otro).1, herramientas);
        }
    }
    let plan = plan.unwrap_or_else(|| Plan::por_omision(&pedido));
    if al_evento(Evento::Peticion(plan.clone())) == Seguir::Parar {
        return Ok(None);
    }
    Ok(Some(plan))
}

/// Mide el historial y, si pasa del 70 %, poda los resultados viejos y, si
/// no basta, reinicia. Avisa si cambio.
fn vigilar_contexto(
    historial: &mut Vec<Mensaje>,
    herramientas: &[Herramienta],
    ventana: u32,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) {
    let antes = contexto::estimar(historial, herramientas);
    if !contexto::pasa_del_umbral(antes, contexto::RESERVA_RESPUESTA, ventana) {
        return;
    }
    contexto::podar_en_turno(historial);
    if contexto::pasa_del_umbral(contexto::estimar(historial, herramientas), contexto::RESERVA_RESPUESTA, ventana) {
        *historial = contexto::reiniciar(historial);
    }
    let despues = contexto::estimar(historial, herramientas);
    if despues < antes {
        al_evento(Evento::Reinicio { antes, despues });
    }
}

/// La marca de fin de pensamiento que a veces queda en el texto.
const CIERRE_PENSAR: &str = "</think>";

fn turno_dentro(
    motor: &dyn Motor,
    historial: &mut Vec<Mensaje>,
    herramientas_base: &[Herramienta],
    ejecutor: &mut dyn Ejecutor,
    opciones: &Opciones,
    crono: &mut Cronometro,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Result<Final, ErrorIa> {
    // Antes que nada (ni plan ni modelo): la orden simple, al instante.
    if let Some(ahora) = opciones.rapido
        && let Some(f) = turno_rapido(historial, ejecutor, ahora, motor.catalogo_completo(), al_evento)
    {
        return Ok(f);
    }
    // La hoja al lienzo solo con la nube (decision del usuario, 14-oct): el
    // local ni la ve, y si se le pide se contesta sin preguntarle.
    let local = !motor.catalogo_completo();
    if local && let Some(f) = crate::hoja::solo_nube(historial, al_evento) {
        return Ok(f);
    }
    let mut lista = herramientas_base.to_vec();
    if !local {
        // Lo que la nube sabe de mas (tabla, paralelogramo, que herramienta
        // va con cada pedido): el local, con su catalogo de siempre.
        crate::herramientas::para_la_nube(&mut lista);
    }
    if local {
        lista.retain(|h| h.nombre != crate::hoja::NOMBRE);
    }
    if opciones.buscar.is_some() && !lista.iter().any(|h| h.nombre == contexto::BUSCAR_HISTORIAL) {
        lista.push(contexto::herramienta_buscar());
    }
    // Los ojos (15-oct): `ver_lienzo` solo si el motor ve imagenes.
    let ojos = opciones.ojos && motor.ve_imagenes();
    lista.retain(|h| h.nombre != crate::ojos::NOMBRE);
    if ojos {
        lista.push(crate::ojos::herramienta());
    }
    let mut plan = None;
    if opciones.planificar {
        // El plan elige entre todas las que hay (`ampliar`), no solo entre
        // las que la seleccion por palabras dio a este turno: «ponle tres
        // ventanas» no dice «dibuja», pero el plan si lo entiende.
        let mut posibles = lista.clone();
        for h in opciones.ampliar.unwrap_or_default() {
            if !posibles.iter().any(|x| x.nombre == h.nombre) && !(local && h.nombre == crate::hoja::NOMBRE) {
                posibles.push(h.clone());
            }
        }
        match planear(motor, historial, &posibles, opciones, crono, al_evento)? {
            Some(p) => {
                for n in &p.herramientas {
                    if !lista.iter().any(|x| &x.nombre == n)
                        && let Some(h) = posibles.iter().find(|x| &x.nombre == n)
                    {
                        lista.push(h.clone());
                    }
                }
                plan = Some(p);
            }
            None => {
                return Ok(Final {
                    texto: String::new(),
                    herramientas_hechas: 0,
                    reintentos: 0,
                    cortado: true,
                });
            }
        }
    }
    let herramientas = &lista[..];
    if opciones.hoja.is_some() || plan.is_some() {
        let base = match historial.first() {
            Some(m) if m.rol == Rol::Sistema => m.texto_entero(),
            _ => instrucciones(),
        };
        let hoja = opciones.hoja.cloned().unwrap_or_default();
        contexto::poner_sistema(historial, contexto::sistema_con_hoja(&base, &hoja, plan.as_ref()));
    }
    let mut est = Estado::default();
    let r = vueltas(motor, historial, herramientas, ejecutor, opciones, crono, &mut est, al_evento);
    let dicho = match r {
        Ok(d) => d,
        Err(e) => {
            if est.cambios > 0 {
                ejecutor.cerrar_turno();
            }
            return Err(e);
        }
    };
    let texto = match dicho {
        Some(mut texto) => {
            // **La revision** (15-oct): el usuario ya ve lo dibujado; ahora
            // el modelo lo mira y corrige lo que este mal.
            if ojos && opciones.revisar > 0 && est.cambios > 0 && !est.cortado
                && let Some(extra) = revisar(motor, historial, herramientas, ejecutor, opciones, crono, &mut est, al_evento)
            {
                texto.push_str("\n\n");
                texto.push_str(&extra);
            }
            texto
        }
        // Se acabaron las vueltas o los reintentos (o se corto): se dice
        // claro en vez de dejar la burbuja vacia.
        None if est.cortado => String::new(),
        // Algo si salio (medido: el 2B dibuja bien y luego insiste en
        // anadir mas con datos rotos): no se dice que no se hizo nada.
        None if est.hechas > 0 => "Hecho en parte: lo demas no me salio con las herramientas.".to_string(),
        None => "No he sabido hacerlo con las herramientas; prueba a pedirlo de otra forma o por partes.".to_string(),
    };
    if est.cambios > 0 {
        // Todo lo que cambio el turno (y lo que corrigio), un solo Ctrl+Z.
        ejecutor.cerrar_turno();
    }
    Ok(Final {
        texto,
        herramientas_hechas: est.hechas,
        reintentos: est.reintentos,
        cortado: est.cortado,
    })
}

/// Lo que lleva la cuenta de un turno entre sus vueltas (y su revision).
struct Estado {
    hechas: usize,
    reintentos: usize,
    cortado: bool,
    /// Cuantos elementos puede llevar cada lista de una llamada: empieza en
    /// [`TOPE_LISTAS`] y baja a [`TOPE_LISTAS_REINTENTO`] si una salio
    /// cortada. Con la gramatica de llama.cpp el modelo NO PUEDE pasarse.
    tope: u64,
    /// `buscar_historial` vale UNA vuelta por turno: medido con el 2B
    /// (13-oct), con ella a mano la llamaba seis veces seguidas y no
    /// contestaba nunca. Tras usarla se quita, y el modelo contesta con lo
    /// que encontro.
    ya_busco: bool,
    /// Llamadas hechas que cambiaron lo que se ve (dibujar, editar, anotar).
    cambios: usize,
    /// Si lo ultimo que cambio fue un documento anotado: la revision mira
    /// la hoja con lo anotado, no el lienzo.
    documento: bool,
    /// Cuantas vistas pidio el modelo en este turno.
    vistas: usize,
}

impl Default for Estado {
    fn default() -> Self {
        Estado {
            hechas: 0,
            reintentos: 0,
            cortado: false,
            tope: TOPE_LISTAS,
            ya_busco: false,
            cambios: 0,
            documento: false,
            vistas: 0,
        }
    }
}

/// Cuantas veces puede pedir el modelo `ver_lienzo` en un turno (cada vista
/// son ~1000 tokens que se releen en cada vuelta).
pub const VISTAS_POR_TURNO: usize = 3;

/// Cuantas rondas de revision como mucho, pida lo que pida la app.
pub const REVISIONES_MAXIMAS: usize = 2;

/// **El pedido del usuario**: su ultimo mensaje que no sea una vista del
/// lienzo ni un aviso de reintento.
pub fn pedido_del_usuario(historial: &[Mensaje]) -> String {
    historial
        .iter()
        .rev()
        .filter(|m| m.rol == Rol::Usuario && !crate::ojos::es_vista(m))
        .map(Mensaje::texto_entero)
        .find(|t| !t.trim_start().starts_with("ERROR"))
        .unwrap_or_default()
}

/// **Las vueltas modelo → herramientas → modelo** hasta que conteste con
/// texto (`Some`) o se acaben las vueltas o los reintentos, o se corte
/// (`None`).
#[allow(clippy::too_many_arguments)]
fn vueltas(
    motor: &dyn Motor,
    historial: &mut Vec<Mensaje>,
    herramientas: &[Herramienta],
    ejecutor: &mut dyn Ejecutor,
    opciones: &Opciones,
    crono: &mut Cronometro,
    est: &mut Estado,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Result<Option<String>, ErrorIa> {
    let ojos = herramientas.iter().any(|h| h.nombre == crate::ojos::NOMBRE);
    // Los reintentos de ESTA tanda (la revision tiene los suyos).
    let mut reintentos = 0;
    let mut hechas_aqui = 0;
    for _ in 0..VUELTAS + REINTENTOS {
        if opciones.ventana > 0 {
            vigilar_contexto(historial, herramientas, opciones.ventana, al_evento);
        }
        let mut peticion = Peticion::nueva(historial.clone());
        let ofrecidas: Vec<Herramienta> = herramientas
            .iter()
            .filter(|h| !(est.ya_busco && h.nombre == contexto::BUSCAR_HISTORIAL))
            .cloned()
            .collect();
        peticion.herramientas = con_topes(&ofrecidas, est.tope);
        // La nube no tiene la ventana de 8192 del local: con su catalogo
        // entero (~13k tokens) la cuenta del local la dejaba SIEMPRE en
        // `MIN_TOKENS` (512) y un resumen o una llamada larga salian
        // cortados (banco de documentos, 15-oct).
        peticion.max_tokens = if motor.catalogo_completo() { MAX_TOKENS_NUBE } else { tope_de_tokens(&peticion) };
        let mut sep = Separador::nuevo();
        let respuesta = motor.completar(&peticion, &mut |t| repartir(t, &mut sep, crono, None, al_evento));
        for t in sep.acabar() {
            repartir(&t, &mut Separador::nuevo(), crono, None, al_evento);
        }
        let mut respuesta = match respuesta {
            Ok(r) => r,
            // `llama-server` no deja pasar una llamada cuyo JSON no cierra
            // (HTTP 500, «Failed to parse tool call arguments»): es una
            // llamada cortada por el tope de tokens. Se le pide que la repita
            // MAS CORTA, y la gramatica se lo impone (listas mas cortas).
            Err(ErrorIa::Protocolo(m)) if llamada_rota(&m) => {
                tracing::warn!(detalle = %m, tope = est.tope, "asistente: llamada a herramienta cortada");
                reintentos += 1;
                est.reintentos += 1;
                if reintentos > REINTENTOS {
                    break;
                }
                est.tope = TOPE_LISTAS_REINTENTO;
                let motivo = pedir_mas_corto();
                if al_evento(Evento::Reintento { motivo: motivo.clone() }) == Seguir::Parar {
                    est.cortado = true;
                    break;
                }
                historial.push(Mensaje::usuario(motivo));
                continue;
            }
            Err(e) => return Err(e),
        };
        if respuesta.texto.contains(CIERRE_PENSAR) {
            respuesta.texto = separar(&respuesta.texto).1;
        }
        historial.push(mensaje_del_asistente(&respuesta));
        if respuesta.fin == Fin::Cortado {
            est.cortado = true;
            break;
        }
        if respuesta.llamadas.is_empty() {
            let texto = if !respuesta.texto.trim().is_empty() {
                respuesta.texto
            } else if hechas_aqui > 0 {
                HECHO.to_string()
            } else {
                SIN_PALABRAS.to_string()
            };
            return Ok(Some(texto));
        }
        let mut alguna_mal = false;
        let mut busco_bien = false;
        // Lo que pidio el usuario, para los colores que el modelo olvide.
        let pedido = pedido_del_usuario(historial);
        // Primero se valida todo; luego se hacen juntas las buenas
        // (`Ejecutor::ejecutar_varias`: las independientes, a la vez); y los
        // resultados vuelven al modelo en el orden de sus llamadas.
        let mut huecos: Vec<(String, Option<String>)> = Vec::new();
        let mut buenas: Vec<(usize, crate::herramientas::Validada)> = Vec::new();
        // Las vistas pedidas: se pintan DESPUES de lo demas de la misma
        // respuesta («dibuja y mira como quedo»).
        let mut miradas: Vec<(usize, crate::ojos::Mirada)> = Vec::new();
        for cruda in &respuesta.llamadas {
            // `buscar_historial` no es del chat: lo contesta el propio turno.
            if let Some(buscar) = opciones.buscar.filter(|_| cruda.nombre.trim() == contexto::BUSCAR_HISTORIAL) {
                let texto = match contexto::consulta_de(&cruda.argumentos) {
                    _ if est.ya_busco => "Ya buscaste en este turno: contesta ahora con lo que encontraste.".into(),
                    Some(c) => buscar(&c),
                    None => "ERROR: falta «consulta». Llama a buscar_historial con {\"consulta\": \"que buscar\"}."
                        .into(),
                };
                let ok = !texto.starts_with("ERROR");
                busco_bien |= ok;
                alguna_mal |= !ok;
                al_evento(Evento::Resultado {
                    nombre: contexto::BUSCAR_HISTORIAL.to_string(),
                    ok,
                    texto: cortar(&texto, 300),
                });
                huecos.push((cruda.id.clone(), Some(recortar_resultado(texto))));
                continue;
            }
            // `ver_lienzo` tampoco: la imagen la pinta el ejecutor y va en
            // un mensaje aparte (`ojos.rs`).
            if ojos && cruda.nombre.trim() == crate::ojos::NOMBRE {
                match crate::ojos::leer(&cruda.argumentos) {
                    Ok(m) => {
                        miradas.push((huecos.len(), m));
                        huecos.push((cruda.id.clone(), None));
                    }
                    Err(e) => {
                        alguna_mal = true;
                        let motivo = format!("ERROR: {e}");
                        if al_evento(Evento::Reintento { motivo: motivo.clone() }) == Seguir::Parar {
                            est.cortado = true;
                        }
                        huecos.push((cruda.id.clone(), Some(motivo)));
                    }
                }
                continue;
            }
            match validar(&sanear(cruda), herramientas) {
                Ok(mut v) => {
                    match &mut v.llamada {
                        Llamada::Dibujar(figuras) => colores_del_pedido(figuras, &pedido),
                        Llamada::Calcar(p) => crate::calcar::ajustar_al_pedido(p, &pedido),
                        Llamada::Editar(e) => crate::edicion::ajustar_al_pedido(e, &pedido),
                        _ => {}
                    }
                    al_evento(Evento::Herramienta {
                        llamada: v.llamada.clone(),
                        avisos: v.avisos.clone(),
                    });
                    buenas.push((huecos.len(), v));
                    huecos.push((cruda.id.clone(), None));
                }
                Err(e) => {
                    alguna_mal = true;
                    let mut motivo = e.para_el_modelo(herramientas);
                    // Unos argumentos que no cierran casi siempre son una
                    // llamada que se quedo sin tokens: la siguiente, corta.
                    // La siguiente, corta: una llamada larga que sale mal es
                    // la que mas tarda en repetirse.
                    est.tope = TOPE_LISTAS_REINTENTO;
                    if matches!(e, ErrorHerramienta::JsonRoto) || respuesta.fin == Fin::Longitud {
                        motivo = pedir_mas_corto();
                    }
                    tracing::warn!(herramienta = %cruda.nombre, %motivo, "asistente: llamada mal hecha");
                    if al_evento(Evento::Reintento { motivo: motivo.clone() }) == Seguir::Parar {
                        est.cortado = true;
                    }
                    huecos.push((cruda.id.clone(), Some(motivo)));
                }
            }
        }
        if !buenas.is_empty() {
            let llamadas: Vec<Llamada> = buenas.iter().map(|(_, v)| v.llamada.clone()).collect();
            let mut resultados = ejecutor.ejecutar_varias(&llamadas);
            resultados.resize_with(llamadas.len(), || Err("no se hizo".into()));
            for ((i, v), r) in buenas.into_iter().zip(resultados) {
                let (ok, texto) = match r {
                    Ok(t) => (true, t),
                    Err(t) => (false, format!("ERROR: {t}")),
                };
                est.hechas += usize::from(ok);
                hechas_aqui += usize::from(ok);
                if ok && crate::ojos::cambia_lo_que_se_ve(&v.llamada) {
                    est.cambios += 1;
                    est.documento = matches!(&v.llamada, Llamada::App(a) if a.herramienta == crate::acciones_documentos::ANOTAR);
                }
                al_evento(Evento::Resultado {
                    nombre: v.llamada.nombre().to_string(),
                    ok,
                    texto: texto.clone(),
                });
                let mut para_el_modelo = recortar_resultado(texto);
                if !v.avisos.is_empty() {
                    para_el_modelo.push_str(&format!(" (avisos: {})", v.avisos.join("; ")));
                }
                huecos[i].1 = Some(para_el_modelo);
            }
        }
        let mut vistas = Vec::new();
        for (i, m) in miradas {
            let texto = if est.vistas >= VISTAS_POR_TURNO {
                format!("Ya miraste {VISTAS_POR_TURNO} veces en este turno: sigue con lo que viste.")
            } else {
                est.vistas += 1;
                al_evento(Evento::Llamando(crate::ojos::NOMBRE.into()));
                match ejecutor.ver(&m) {
                    Ok(v) => {
                        let t = format!("hecho: la imagen va en el mensaje siguiente ({}x{} px)", v.ancho, v.alto);
                        vistas.push(v);
                        t
                    }
                    Err(e) => format!("ERROR: {e}"),
                }
            };
            al_evento(Evento::Resultado {
                nombre: crate::ojos::NOMBRE.to_string(),
                ok: !texto.starts_with("ERROR"),
                texto: texto.clone(),
            });
            huecos[i].1 = Some(texto);
        }
        for (id, texto) in huecos {
            historial.push(Mensaje::resultado(&id, texto.unwrap_or_default()));
        }
        // Detras de los resultados, como mensaje del usuario: lo unico que
        // admiten igual Claude, DeepSeek y llama.cpp.
        for v in &vistas {
            historial.push(crate::ojos::mensaje_de_vista(v, ""));
        }
        est.ya_busco |= busco_bien;
        if est.cortado {
            break;
        }
        if alguna_mal {
            reintentos += 1;
            est.reintentos += 1;
            if reintentos > REINTENTOS {
                break;
            }
        }
    }
    Ok(None)
}

/// **La revision** (15-oct): se le ensena al modelo como quedo lo que
/// dibujo o anoto, una o dos veces, y corrige lo que vea mal. Devuelve lo
/// que dijo de lo corregido (`None`: estaba bien, no se pudo mirar o no
/// cambio nada). Mientras, su texto no va a la burbuja: un «TODO_BIEN» no
/// le dice nada al usuario.
#[allow(clippy::too_many_arguments)]
fn revisar(
    motor: &dyn Motor,
    historial: &mut Vec<Mensaje>,
    herramientas: &[Herramienta],
    ejecutor: &mut dyn Ejecutor,
    opciones: &Opciones,
    crono: &mut Cronometro,
    est: &mut Estado,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Option<String> {
    use crate::ojos::{Mirada, Zona};
    let pedido = pedido_del_usuario(historial);
    let mut dicho: Vec<String> = Vec::new();
    for ronda in 1..=opciones.revisar.min(REVISIONES_MAXIMAS) {
        if al_evento(Evento::Llamando(crate::ojos::NOMBRE.into())) == Seguir::Parar {
            est.cortado = true;
            break;
        }
        let mirada = Mirada::de(if est.documento { Zona::Documento } else { Zona::Turno });
        let vista = match ejecutor.ver(&mirada) {
            Ok(v) => v,
            Err(e) => {
                tracing::info!(%e, "asistente: no hay vista para revisar; se queda como esta");
                break;
            }
        };
        let marca = historial.len();
        historial.push(crate::ojos::mensaje_de_vista(&vista, &crate::ojos::pedido_de_revision(&pedido, ronda)));
        let cambios_antes = est.cambios;
        let mut dice = String::new();
        let r = {
            let mut filtro = |e: Evento| -> Seguir {
                match e {
                    // Lo que dice se guarda; mientras, la burbuja dice
                    // «mirando el lienzo» (y se sigue mirando si paro).
                    Evento::Texto(t) => {
                        dice.push_str(&t);
                        al_evento(Evento::Llamando(crate::ojos::NOMBRE.into()))
                    }
                    Evento::Pensando(_) | Evento::PensoListo { .. } | Evento::Peticion(_) => Seguir::Si,
                    otro => al_evento(otro),
                }
            };
            vueltas(motor, historial, herramientas, ejecutor, opciones, crono, est, &mut filtro)
        };
        let corrigio = est.cambios > cambios_antes;
        match r {
            Err(e) => {
                tracing::warn!(%e, "asistente: la revision no pudo acabar; se queda lo de antes");
                if !corrigio {
                    historial.truncate(marca);
                }
                break;
            }
            Ok(t) => {
                if !corrigio {
                    // Bien a la primera (o no supo que cambiar): ni se dice
                    // ni se queda la imagen en la conversacion.
                    historial.truncate(marca);
                    break;
                }
                let t = t.unwrap_or(dice);
                let frase = if crate::ojos::es_todo_bien(&t) || t.trim().is_empty() {
                    "Lo revisé y corregí.".to_string()
                } else {
                    t.trim().to_string()
                };
                al_evento(Evento::Texto(format!("\n\n{frase}")));
                dicho.push(frase);
            }
        }
        if est.cortado {
            break;
        }
    }
    (!dicho.is_empty()).then(|| dicho.join("\n\n"))
}

/// Lo que se dice si el modelo acaba sin una palabra y sin hacer nada:
/// mejor una pregunta que una burbuja vacia.
pub const SIN_PALABRAS: &str = "No sé qué decir sobre eso. ¿Qué quieres que haga?";
/// Y si hizo algo pero no lo conto.
pub const HECHO: &str = "Hecho.";

/// Tope de tokens de cada vuelta del agente. Un dibujo de [`TOPE_LISTAS`]
/// figuras completas son unos 1500 tokens de JSON (medido con el 2B: ~1,7
/// letras por token); con el doble de margen es la gramatica, y no el tope,
/// quien corta.
pub const MAX_TOKENS_CON_HERRAMIENTAS: u32 = 3072;
/// Tope de cada vuelta con la nube (Claude, DeepSeek): un resumen largo o
/// una llamada de 16 figuras con textos ricos caben de sobra; se paga lo que
/// sale, no el tope.
pub const MAX_TOKENS_NUBE: u32 = 8192;
/// Lo minimo que se le deja contestar aunque el contexto vaya lleno.
pub const MIN_TOKENS: u32 = 512;
/// Cuantas figuras puede llevar una llamada (las listas de objetos). Las
/// de datos sueltos (numeros, etiquetas) el doble: pesan poco. 16 figuras
/// completas son ~1600 tokens: en la CPU de este PC, unos 4 minutos; con
/// 24 y textos largos se fueron a 7 (medido, 13-oct).
pub const TOPE_LISTAS: u64 = 16;
/// Y tras una llamada cortada.
pub const TOPE_LISTAS_REINTENTO: u64 = 10;
/// Lo mas largo que puede ser un texto suelto dentro de una llamada.
pub const TOPE_TEXTO: u64 = 120;

/// **Cuanta respuesta se pide**: [`MAX_TOKENS_CON_HERRAMIENTAS`], o lo que
/// quepa en el contexto con lo que ya lleva la conversacion (estimado por
/// lo alto: 3 letras por token y ~1000 por foto).
pub fn tope_de_tokens(p: &Peticion) -> u32 {
    let letras: usize = p
        .mensajes
        .iter()
        .map(|m| m.texto_entero().chars().count() + m.llamadas.iter().map(|l| l.argumentos.len()).sum::<usize>())
        .sum::<usize>()
        + p.herramientas.iter().map(|h| h.a_json().to_string().len()).sum::<usize>();
    let fotos = p.mensajes.iter().filter(|m| m.tiene_imagen()).count() as u32;
    let entrada = (letras / 3) as u32 + 1000 * fotos + 200;
    crate::servidor::CONTEXTO
        .saturating_sub(entrada)
        .clamp(MIN_TOKENS, MAX_TOKENS_CON_HERRAMIENTAS)
}

/// **Las herramientas con sus listas y textos acotados.** `llama-server`
/// convierte el esquema de cada herramienta en una gramatica que restringe
/// lo que el modelo puede escribir (comprobado con b11580 y el 2B: con
/// `maxItems: 3` y pidiendo 15 figuras salen 3; con 100, salen las 15). Asi
/// una llamada no puede crecer hasta cortarse a medias por el tope de
/// tokens, que es lo que dejaba el JSON roto («missing closing quote») y el
/// lienzo vacio. Solo se recorta lo de la peticion: el catalogo y su
/// validacion no cambian.
pub fn con_topes(herramientas: &[Herramienta], tope: u64) -> Vec<Herramienta> {
    fn acotar(v: &mut serde_json::Value, tope: u64) {
        let Some(o) = v.as_object_mut() else { return };
        match o.get("type").and_then(|t| t.as_str()) {
            Some("array") => {
                let de_objetos = o.get("items").and_then(|i| i.get("type")).and_then(|t| t.as_str()) == Some("object");
                let tope = if de_objetos { tope } else { 2 * tope };
                let max = o.get("maxItems").and_then(|m| m.as_u64()).map_or(tope, |m| m.min(tope));
                o.insert("maxItems".into(), max.into());
            }
            // Un texto sin tope propio es una etiqueta o un titulo: [`TOPE_TEXTO`].
            // Los largos por naturaleza (un SVG, una nota, los pasos de una
            // automatizacion) traen el suyo en su esquema y se respeta: con 120
            // para todos, un SVG llegaba cortado a 120 letras (14-oct).
            Some("string") if !o.contains_key("enum") && !o.contains_key("maxLength") => {
                o.insert("maxLength".into(), TOPE_TEXTO.into());
            }
            _ => {}
        }
        for (_, hijo) in o.iter_mut() {
            match hijo {
                serde_json::Value::Object(_) => acotar(hijo, tope),
                serde_json::Value::Array(lista) => lista.iter_mut().for_each(|x| acotar(x, tope)),
                _ => {}
            }
        }
    }
    herramientas
        .iter()
        .map(|h| {
            let mut h = h.clone();
            acotar(&mut h.parametros, tope);
            h
        })
        .collect()
}

/// **Lleva al borde las coordenadas que se salen un poco**, antes de
/// validar. El 2B escribe a veces `x: 2010` en la figura 12 de 16, y la
/// validacion (con razon) rechaza la llamada entera: eran 7 minutos de CPU
/// tirados por un numero (medido, 13-oct). Un punto pegado al borde se ve
/// igual; lo que no es numero o no casa, lo sigue diciendo la validacion.
pub fn sanear(cruda: &crate::motor::LlamadaCruda) -> crate::motor::LlamadaCruda {
    use crate::herramientas::{COORDENADA_MAXIMA, COORDENADA_MINIMA};
    fn llevar(v: &mut serde_json::Value, cambios: &mut usize) {
        match v {
            serde_json::Value::Object(o) => {
                for (k, x) in o.iter_mut() {
                    let rango = match k.as_str() {
                        "x" | "y" | "x2" | "y2" => Some((COORDENADA_MINIMA, COORDENADA_MAXIMA)),
                        "ancho" | "alto" => Some((-COORDENADA_MAXIMA, COORDENADA_MAXIMA)),
                        _ => None,
                    };
                    match (rango, x.as_f64()) {
                        (Some((a, b)), Some(n)) if n.is_finite() && (n < a || n > b) => {
                            *x = serde_json::json!(n.clamp(a, b));
                            *cambios += 1;
                        }
                        _ => llevar(x, cambios),
                    }
                }
            }
            serde_json::Value::Array(l) => l.iter_mut().for_each(|x| llevar(x, cambios)),
            _ => {}
        }
    }
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&cruda.argumentos) else {
        return cruda.clone();
    };
    let mut cambios = 0;
    llevar(&mut v, &mut cambios);
    if cambios == 0 {
        return cruda.clone();
    }
    tracing::info!(cambios, herramienta = %cruda.nombre, "asistente: coordenadas llevadas al borde");
    crate::motor::LlamadaCruda {
        argumentos: v.to_string(),
        ..cruda.clone()
    }
}

/// Lo que se le dice al modelo tras una llamada cortada: mas corta, y lo
/// grande, en varias llamadas.
fn pedir_mas_corto() -> String {
    format!(
        "ERROR: tu llamada a la herramienta salio cortada porque era demasiado larga. \
         Vuelve a llamarla con {TOPE_LISTAS_REINTENTO} elementos como mucho, solo las formas principales; \
         si hace falta mas, haz varias llamadas seguidas."
    )
}

/// Si el error del motor es una llamada a herramienta con el JSON roto (lo
/// que contesta `llama-server` cuando no la puede leer).
fn llamada_rota(m: &str) -> bool {
    let m = m.to_lowercase();
    m.contains("tool call") && (m.contains("json") || m.contains("parse"))
}

/// **El fallo en palabras del usuario.** Lo tecnico (codigos HTTP, el JSON
/// del servidor) va al registro, nunca a la burbuja.
pub fn para_el_usuario(e: &ErrorIa) -> String {
    match e {
        ErrorIa::Falta(_) => "Falta bajar el asistente: abre el chat «Asistente» para bajarlo.".into(),
        ErrorIa::Arranque(_) => "No pude arrancar el asistente. Prueba otra vez en un momento.".into(),
        ErrorIa::Conexion(_) => "El asistente dejó de responder. Prueba otra vez.".into(),
        ErrorIa::Protocolo(_) => "Esta vez no me salió la respuesta. Prueba otra vez o pídelo más corto.".into(),
        ErrorIa::Cancelado => "Cancelado.".into(),
        // La nube ya lo dice en palabras del usuario (sin codigos ni JSON:
        // ver `nube::clasificar`).
        ErrorIa::Nube(aviso) => aviso.clone(),
        ErrorIa::Descarga { .. } | ErrorIa::HuellaDistinta { .. } => e.to_string(),
        ErrorIa::Io(_) => "No pude contestar por un fallo del equipo. Prueba otra vez.".into(),
    }
}

fn mensaje_del_asistente(r: &Respuesta) -> Mensaje {
    let mut m = Mensaje::texto(Rol::Asistente, r.texto.clone());
    m.llamadas = r.llamadas.clone();
    m
}

/// Las instrucciones de sistema del asistente. Cortas: cada token cuenta en
/// cada vuelta, y los modelos pequenos siguen mejor pocas reglas claras.
pub fn instrucciones() -> String {
    "Eres el asistente de PixPin, una app de capturas, notas y lienzo. \
     Contesta en el idioma del usuario, breve y claro. \
     Si pide dibujar algo, usa dibujar_en_lienzo, \
     poniendo el color de cada figura. \
     Si pide dibujar, copiar o calcar al lienzo lo de una imagen adjunta (este perro, la foto), usa calcar_imagen. \
     Para cambiar lo que YA esta en el lienzo (el «Lienzo destino» trae sus #id): transformar_elementos (mover, borrar, girar...), cambiar_estilo, \
     trazar_a_mano (subrayar o resaltar algo, lapiz, grafito), geometria_precisa (rectas, circulos, arcos exactos), cortar_linea, extender_linea, \
     imagen_en_lienzo (pegar o recortar una foto); cada una devuelve los #id para la siguiente. \
     Si da datos para graficar, usa grafica; si da una funcion, graficar_funcion; una ecuacion, escribir_ecuacion; un cronograma, cronograma; una ilustracion con curvas, dibujar_svg. \
     Para estudiantes: un problema (de una foto o escrito) que pasar en limpio o resolver, problema_en_limpio; un diagrama de fisica (fuerzas, plano inclinado, tiro, circuito, MRU), diagrama_fisica. Las cuentas las hace la app, no tu. \
     Si quiere el texto de un documento escaneado, usa ocr. \
     Si pide hacer algo en la app (notas, tareas, ventanas, capturas, abrir ficheros), usa su herramienta. \
     Con sus documentos y planos: una tabla de una foto o un PDF, tabla_de_imagen; resaltar, anotar o encerrar algo, anotar_documento; \
     «¿donde dice...?», buscar_en_documento; abrir uno por su nombre, abrir_por_nombre; otro formato, exportar_documento. \
     Nunca pidas rutas ni codigos: nombra el archivo con las palabras del usuario; si te dicen que hay varios, pregunta cual con esa lista corta. \
     Si pregunta donde esta algo o como se hace en PixPin, usa ayuda_app y contesta con lo que diga, sin inventar. \
     Despues de usar una herramienta, di en una frase lo que hiciste. "
        .to_string()
        + crate::conocimiento::RESUMEN
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::herramientas::catalogo;
    use crate::motor::MotorGuionado;

    /// Un ejecutor que apunta lo que le piden.
    #[derive(Default)]
    struct Apunta {
        hechas: Vec<Llamada>,
        fallar: bool,
    }

    impl Ejecutor for Apunta {
        fn ejecutar(&mut self, llamada: &Llamada) -> Result<String, String> {
            self.hechas.push(llamada.clone());
            if self.fallar {
                Err("el lienzo esta cerrado".into())
            } else {
                Ok("hecho".into())
            }
        }
    }

    fn historial() -> Vec<Mensaje> {
        vec![Mensaje::sistema(instrucciones()), Mensaje::usuario("dibuja una casa")]
    }

    fn correr(guion: Vec<Respuesta>, ejecutor: &mut Apunta) -> (Final, Vec<Evento>, Vec<Mensaje>, MotorGuionado) {
        let motor = MotorGuionado::nuevo(guion);
        let mut h = historial();
        let mut eventos = Vec::new();
        let f = turno(&motor, &mut h, &catalogo(), ejecutor, &mut |e| {
            eventos.push(e);
            Seguir::Si
        })
        .unwrap();
        (f, eventos, h, motor)
    }

    /// Un motor «de la nube»: el guionado con el catalogo entero.
    struct Nube(MotorGuionado);

    impl Motor for Nube {
        fn completar(&self, p: &Peticion, t: &mut dyn FnMut(&Trozo) -> Seguir) -> Result<Respuesta, ErrorIa> {
            self.0.completar(p, t)
        }
        fn catalogo_completo(&self) -> bool {
            true
        }
    }

    #[test]
    fn la_nube_no_hereda_el_tope_de_la_ventana_del_local() {
        // Con el catalogo entero, la cuenta del local daria MIN_TOKENS.
        let nube = Nube(MotorGuionado::nuevo(vec![Respuesta::de_texto("Resumen largo")]));
        let mut h = historial();
        let mut ej = Apunta::default();
        turno(&nube, &mut h, &catalogo(), &mut ej, &mut |_| Seguir::Si).unwrap();
        assert_eq!(nube.0.recibido.lock().unwrap()[0].max_tokens, MAX_TOKENS_NUBE);
        // El local sigue con su cuenta (negativo: no cambia).
        let local = MotorGuionado::nuevo(vec![Respuesta::de_texto("Hola")]);
        let mut h = historial();
        turno(&local, &mut h, &catalogo(), &mut ej, &mut |_| Seguir::Si).unwrap();
        let p = &local.recibido.lock().unwrap()[0];
        assert_eq!(p.max_tokens, tope_de_tokens(p));
        assert!(p.max_tokens < MAX_TOKENS_NUBE);
    }

    #[test]
    fn solo_texto_acaba_en_una_vuelta() {
        let mut ej = Apunta::default();
        let (f, eventos, h, _) = correr(vec![Respuesta::de_texto("Hola que tal")], &mut ej);
        assert_eq!(f.texto, "Hola que tal");
        assert_eq!(f.herramientas_hechas, 0);
        assert!(matches!(&eventos[0], Evento::Texto(t) if t == "Hola "));
        assert_eq!(h.last().unwrap().rol, Rol::Asistente);
    }

    #[test]
    fn llamada_buena_se_ejecuta_y_el_resultado_vuelve_al_modelo() {
        let mut ej = Apunta::default();
        let (f, eventos, h, motor) = correr(
            vec![
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"rectangulo","x":10,"y":10}]}"#),
                Respuesta::de_texto("Listo, dibuje un rectangulo."),
            ],
            &mut ej,
        );
        assert_eq!(f.herramientas_hechas, 1);
        assert_eq!(f.texto, "Listo, dibuje un rectangulo.");
        assert_eq!(ej.hechas.len(), 1);
        assert!(eventos.iter().any(|e| matches!(e, Evento::Resultado { ok: true, .. })));
        // La segunda peticion lleva la llamada y su resultado.
        let pedidas = motor.recibido.lock().unwrap();
        let segunda = &pedidas[1].mensajes;
        assert_eq!(segunda[segunda.len() - 2].llamadas.len(), 1);
        assert_eq!(segunda.last().unwrap().rol, Rol::Herramienta);
        assert_eq!(segunda.last().unwrap().texto_entero(), "hecho");
        assert!(!pedidas[0].herramientas.is_empty());
        assert_eq!(h.len(), 5);
    }

    #[test]
    fn llamada_mala_se_reintenta_con_el_motivo() {
        let mut ej = Apunta::default();
        let (f, eventos, _, motor) = correr(
            vec![
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"dragon","x":1,"y":1}]}"#),
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"texto","x":1,"y":1,"texto":"CASA"}]}"#),
                Respuesta::de_texto("Hecho."),
            ],
            &mut ej,
        );
        assert_eq!(f.reintentos, 1);
        assert_eq!(f.herramientas_hechas, 1);
        assert_eq!(ej.hechas.len(), 1, "la mala no se ejecuta");
        assert!(eventos.iter().any(|e| matches!(e, Evento::Reintento { motivo } if motivo.contains("dragon"))));
        let pedidas = motor.recibido.lock().unwrap();
        assert!(pedidas[1].mensajes.last().unwrap().texto_entero().starts_with("ERROR:"));
    }

    #[test]
    fn herramienta_inventada_tambien_se_reintenta() {
        let mut ej = Apunta::default();
        let (f, _, _, _) = correr(
            vec![Respuesta::de_llamada("pintar_cielo", "{}"), Respuesta::de_texto("No puedo hacer eso.")],
            &mut ej,
        );
        assert_eq!(f.reintentos, 1);
        assert_eq!(f.texto, "No puedo hacer eso.");
        assert!(ej.hechas.is_empty());
    }

    #[test]
    fn si_se_equivoca_siempre_se_rinde_con_un_mensaje() {
        let mut ej = Apunta::default();
        let malas = (0..10).map(|_| Respuesta::de_llamada("grafica", "{roto")).collect();
        let (f, _, _, motor) = correr(malas, &mut ej);
        assert_eq!(f.reintentos, REINTENTOS + 1);
        assert!(f.texto.contains("No he sabido"));
        assert_eq!(motor.recibido.lock().unwrap().len(), REINTENTOS + 1);
    }

    #[test]
    fn el_fallo_del_ejecutor_se_le_cuenta_al_modelo() {
        let mut ej = Apunta {
            fallar: true,
            ..Default::default()
        };
        let (f, eventos, _, motor) = correr(
            vec![
                Respuesta::de_llamada("ocr", "{}"),
                Respuesta::de_texto("No pude: el lienzo esta cerrado."),
            ],
            &mut ej,
        );
        assert_eq!(f.herramientas_hechas, 0);
        assert!(eventos.iter().any(|e| matches!(e, Evento::Resultado { ok: false, .. })));
        let pedidas = motor.recibido.lock().unwrap();
        assert!(pedidas[1].mensajes.last().unwrap().texto_entero().contains("lienzo esta cerrado"));
    }

    #[test]
    fn un_modelo_que_llama_en_bucle_se_corta() {
        let mut ej = Apunta::default();
        let bucle = (0..20)
            .map(|_| Respuesta::de_llamada("ocr", "{}"))
            .collect();
        let (f, _, _, motor) = correr(bucle, &mut ej);
        // Las llamadas si se hicieron: no se dice que no se hizo nada.
        assert!(f.texto.contains("Hecho en parte"), "{}", f.texto);
        assert_eq!(motor.recibido.lock().unwrap().len(), VUELTAS + REINTENTOS);
    }

    /// Falla la primera vez como `llama-server` con una llamada cortada.
    struct CortaUnaVez(std::sync::atomic::AtomicBool, MotorGuionado);

    impl Motor for CortaUnaVez {
        fn completar(
            &self,
            p: &Peticion,
            t: &mut dyn FnMut(&Trozo) -> Seguir,
        ) -> Result<Respuesta, ErrorIa> {
            // El reintento pide lo mismo que cabe (el catalogo entero ya se
            // come parte de la ventana de 8192: 14-oct).
            assert_eq!(p.max_tokens, tope_de_tokens(p));
            if !self.0.swap(true, std::sync::atomic::Ordering::SeqCst) {
                return Err(ErrorIa::Protocolo(
                    "HTTP 500: Failed to parse tool call arguments as JSON: unexpected end of input".into(),
                ));
            }
            self.1.completar(p, t)
        }
    }

    #[test]
    fn una_llamada_cortada_por_el_servidor_se_reintenta() {
        let motor = CortaUnaVez(
            Default::default(),
            MotorGuionado::nuevo(vec![
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"rombo","x":1,"y":1}]}"#),
                Respuesta::de_texto("Listo."),
            ]),
        );
        // Un catalogo fijo (solo dibujar): con el entero, cada fila nueva de
        // la tabla de acciones bajaba el tope de tokens y rompia la prueba.
        let fijo: Vec<_> = crate::herramientas::catalogo_base()
            .into_iter()
            .filter(|h| h.nombre == crate::herramientas::DIBUJAR)
            .collect();
        assert_eq!(fijo.len(), 1);
        let mut h = historial();
        let mut ej = Apunta::default();
        let f = turno(&motor, &mut h, &fijo, &mut ej, &mut |_| Seguir::Si).unwrap();
        assert_eq!(f.reintentos, 1);
        assert_eq!(f.herramientas_hechas, 1);
        assert!(h.iter().any(|m| m.texto_entero().contains("salio cortada")));
        // Otro error del motor no se reintenta.
        let vacio = MotorGuionado::nuevo(vec![]);
        assert!(turno(&vacio, &mut historial(), &fijo, &mut ej, &mut |_| Seguir::Si).is_err());
    }

    // -----------------------------------------------------------------------
    // Pensar, plan, hoja y contexto (13-oct)

    fn plan_del_modelo(pensamiento: &str, texto: &str) -> Respuesta {
        Respuesta {
            pensamiento: pensamiento.into(),
            ..Respuesta::de_texto(texto)
        }
    }

    const PLAN_CASA: &str = "ENTIENDO: Dibujar una casa roja en el lienzo.\nHARE: llamar a dibujar_en_lienzo con figuras rojas.\nHERRAMIENTAS: dibujar_en_lienzo\nDATOS: color rojo\nDECIDIDO: nada";

    fn con_opciones(
        guion: Vec<Respuesta>,
        h: &mut Vec<Mensaje>,
        o: &Opciones,
    ) -> (Final, Vec<Evento>, MotorGuionado) {
        let motor = MotorGuionado::nuevo(guion);
        let mut eventos = Vec::new();
        let f = turno_con(&motor, h, &catalogo(), &mut Apunta::default(), o, &mut |e| {
            eventos.push(e);
            Seguir::Si
        })
        .unwrap();
        (f, eventos, motor)
    }

    #[test]
    fn el_pensamiento_llega_a_trozos_se_pliega_y_luego_va_el_plan() {
        let mut h = historial();
        let o = Opciones {
            planificar: true,
            pensar: true,
            ..Default::default()
        };
        let (f, eventos, motor) = con_opciones(
            vec![
                plan_del_modelo("El usuario quiere una casa. La hare roja.", PLAN_CASA),
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"rectangulo","x":1,"y":1}]}"#),
                Respuesta::de_texto("Listo, una casa roja."),
            ],
            &mut h,
            &o,
        );
        assert_eq!(f.texto, "Listo, una casa roja.");
        let pensados: Vec<&str> = eventos
            .iter()
            .filter_map(|e| if let Evento::Pensando(s) = e { Some(s.as_str()) } else { None })
            .collect();
        assert!(pensados.len() > 3, "a trozos: {pensados:?}");
        assert_eq!(pensados.concat(), "El usuario quiere una casa. La hare roja.");
        // Se pliega una vez, despues de pensar y antes del plan.
        let i_listo: Vec<usize> = eventos
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e, Evento::PensoListo { .. }))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(i_listo.len(), 1);
        let i_plan = eventos.iter().position(|e| matches!(e, Evento::Peticion(_))).unwrap();
        let i_ultimo_pensado = eventos.iter().rposition(|e| matches!(e, Evento::Pensando(_))).unwrap();
        let i_texto = eventos.iter().position(|e| matches!(e, Evento::Texto(_))).unwrap();
        assert!(i_ultimo_pensado < i_listo[0] && i_listo[0] < i_plan && i_plan < i_texto);
        let Evento::Peticion(plan) = &eventos[i_plan] else { unreachable!() };
        assert_eq!(plan.entiendo, "Dibujar una casa roja en el lienzo.");
        assert!(plan.del_modelo);
        // El texto del plan no sale como respuesta.
        assert!(!eventos.iter().any(|e| matches!(e, Evento::Texto(t) if t.contains("ENTIENDO"))));
        let pedidas = motor.recibido.lock().unwrap();
        assert!(pedidas[0].pensar && pedidas[0].herramientas.is_empty(), "planear piensa y no usa herramientas");
        assert_eq!(pedidas[0].max_tokens, TOKENS_PLAN_PENSANDO);
        assert!(!pedidas[1].pensar);
        let sistema = pedidas[1].mensajes[0].texto_entero();
        assert!(sistema.contains("Peticion mejorada") && sistema.contains("casa roja"), "{sistema}");
    }

    #[test]
    fn un_modelo_que_no_piensa_no_abre_la_seccion_pero_si_planea() {
        let mut h = historial();
        let o = Opciones {
            planificar: true,
            pensar: true,
            ..Default::default()
        };
        let (_, eventos, _) = con_opciones(
            vec![plan_del_modelo("", PLAN_CASA), Respuesta::de_texto("Vale.")],
            &mut h,
            &o,
        );
        assert!(!eventos.iter().any(|e| matches!(e, Evento::Pensando(_) | Evento::PensoListo { .. })));
        assert!(eventos.iter().any(|e| matches!(e, Evento::Peticion(_))));
    }

    #[test]
    fn un_pensamiento_en_blanco_tampoco() {
        let mut h = historial();
        let o = Opciones {
            planificar: true,
            pensar: true,
            ..Default::default()
        };
        let (_, eventos, _) = con_opciones(
            vec![plan_del_modelo("   \n  ", PLAN_CASA), Respuesta::de_texto("Vale.")],
            &mut h,
            &o,
        );
        assert!(!eventos.iter().any(|e| matches!(e, Evento::Pensando(_) | Evento::PensoListo { .. })));
    }

    #[test]
    fn si_el_plan_no_se_entiende_se_usa_el_pedido() {
        let mut h = historial();
        let o = Opciones {
            planificar: true,
            ..Default::default()
        };
        let (f, eventos, motor) = con_opciones(
            vec![Respuesta::de_texto("¡Claro! Aqui tienes tu casa."), Respuesta::de_texto("Hecho.")],
            &mut h,
            &o,
        );
        assert_eq!(f.texto, "Hecho.");
        let plan = eventos
            .iter()
            .find_map(|e| if let Evento::Peticion(p) = e { Some(p.clone()) } else { None })
            .unwrap();
        assert_eq!(plan.entiendo, "dibuja una casa");
        assert!(!plan.del_modelo);
        assert_eq!(motor.recibido.lock().unwrap()[0].max_tokens, TOKENS_PLAN);
    }

    #[test]
    fn el_pensamiento_entre_etiquetas_no_sale_en_la_respuesta() {
        let mut h = historial();
        let (f, eventos, _) = con_opciones(
            vec![Respuesta::de_texto("<think>Es un saludo. </think>Hola, que tal")],
            &mut h,
            &Opciones::default(),
        );
        assert_eq!(f.texto, "Hola, que tal");
        let texto: String = eventos
            .iter()
            .filter_map(|e| if let Evento::Texto(s) = e { Some(s.as_str()) } else { None })
            .collect();
        assert_eq!(texto, "Hola, que tal");
        assert!(eventos.iter().any(|e| matches!(e, Evento::Pensando(s) if s.contains("saludo"))));
        assert!(eventos.iter().any(|e| matches!(e, Evento::PensoListo { .. })));
        assert_eq!(h.last().unwrap().texto_entero(), "Hola, que tal");
    }

    #[test]
    fn buscar_historial_contesta_desde_el_turno() {
        let fuentes = vec![
            contexto::Fuente { etiqueta: "#3 usuario".into(), texto: "Las ventas de enero fueron 120.".into() },
            contexto::Fuente { etiqueta: "#4 asistente".into(), texto: "Apuntado.".into() },
        ];
        let buscar = |c: &str| contexto::buscar(&fuentes, c, 3);
        let o = Opciones {
            buscar: Some(&buscar),
            ..Default::default()
        };
        let mut h = historial();
        let (f, eventos, motor) = con_opciones(
            vec![
                Respuesta::de_llamada(contexto::BUSCAR_HISTORIAL, r#"{}"#),
                Respuesta::de_llamada(contexto::BUSCAR_HISTORIAL, r#"{"consulta":"ventas de enero"}"#),
                Respuesta::de_llamada(contexto::BUSCAR_HISTORIAL, r#"{"consulta":"otra vez"}"#),
                Respuesta::de_texto("Fueron 120."),
            ],
            &mut h,
            &o,
        );
        assert_eq!(f.texto, "Fueron 120.");
        assert_eq!(f.herramientas_hechas, 0, "no es una herramienta del chat");
        assert_eq!(f.reintentos, 1, "la llamada sin consulta cuenta como mal hecha");
        let pedidas = motor.recibido.lock().unwrap();
        assert!(pedidas[0].herramientas.iter().any(|x| x.nombre == contexto::BUSCAR_HISTORIAL));
        assert!(pedidas[1].mensajes.last().unwrap().texto_entero().starts_with("ERROR"));
        assert!(pedidas[2].mensajes.last().unwrap().texto_entero().contains("[#3 usuario] Las ventas de enero fueron 120."));
        // Tras una busqueda buena ya no se ofrece, y si insiste se le dice que conteste.
        assert!(!pedidas[2].herramientas.iter().any(|x| x.nombre == contexto::BUSCAR_HISTORIAL));
        assert!(pedidas[3].mensajes.last().unwrap().texto_entero().starts_with("Ya buscaste"));
        assert!(eventos.iter().any(|e| matches!(e, Evento::Resultado { ok: true, nombre, .. } if nombre == contexto::BUSCAR_HISTORIAL)));
        // Sin `buscar`, la herramienta no se ofrece y es una inventada.
        let mut h = historial();
        let (f, _, motor) = con_opciones(
            vec![Respuesta::de_llamada(contexto::BUSCAR_HISTORIAL, r#"{"consulta":"x"}"#), Respuesta::de_texto("No.")],
            &mut h,
            &Opciones::default(),
        );
        assert_eq!(f.reintentos, 1);
        assert!(!motor.recibido.lock().unwrap()[0].herramientas.iter().any(|x| x.nombre == contexto::BUSCAR_HISTORIAL));
    }

    #[test]
    fn al_pasar_del_70_se_reinicia_sin_perder_el_objetivo() {
        let hoja = HojaDeTrabajo {
            objetivo: "Hacer el plano de la casa de Ana con 3 ventanas".into(),
            reinicios: 0,
            ..Default::default()
        };
        let mut h = vec![Mensaje::sistema(instrucciones())];
        for i in 0..30 {
            h.push(Mensaje::usuario(format!("mensaje viejo {i} {}", "bla ".repeat(60))));
            h.push(Mensaje::texto(Rol::Asistente, format!("respuesta vieja {i} {}", "ble ".repeat(60))));
        }
        h.push(Mensaje::usuario("y ahora la puerta"));
        let antes = h.len();
        let o = Opciones {
            planificar: true,
            hoja: Some(&hoja),
            ventana: 8192,
            ..Default::default()
        };
        // Con las herramientas de un turno (la seleccion), no el catalogo
        // entero: ese solo ya no cabe en una ventana pequena.
        let motor = MotorGuionado::nuevo(vec![
            Respuesta::de_texto("ENTIENDO: Anadir la puerta al plano de la casa de Ana.\nHARE: dibujarla."),
            Respuesta::de_texto("Puerta puesta."),
        ]);
        let mut eventos = Vec::new();
        turno_con(&motor, &mut h, &crate::seleccion::para_el_pedido("dibuja la puerta en el plano", false), &mut Apunta::default(), &o, &mut |e| {
            eventos.push(e);
            Seguir::Si
        })
        .unwrap();
        let reinicio = eventos.iter().find_map(|e| match e {
            Evento::Reinicio { antes, despues } => Some((*antes, *despues)),
            _ => None,
        });
        let (t_antes, t_despues) = reinicio.expect("tenia que reiniciar");
        assert!(t_despues < t_antes);
        assert!(!contexto::pasa_del_umbral(t_despues, contexto::RESERVA_RESPUESTA, 8192));
        let pedidas = motor.recibido.lock().unwrap();
        // La vuelta de planear ve la hoja; la de actuar, solo lo justo.
        assert!(pedidas[0].mensajes[1].texto_entero().contains("casa de Ana"));
        let actuar = &pedidas[1].mensajes;
        assert!(actuar.len() <= 4, "sistema + 2 + el nuevo: {}", actuar.len());
        assert!(actuar.len() < antes);
        let sistema = actuar[0].texto_entero();
        assert!(sistema.contains("Objetivo: Hacer el plano de la casa de Ana"), "{sistema}");
        assert!(sistema.contains("Anadir la puerta"), "{sistema}");
        assert_eq!(actuar.last().unwrap().texto_entero(), "y ahora la puerta");
    }

    #[test]
    fn si_piensa_hasta_el_tope_sin_plan_otra_vuelta_corta_lo_escribe() {
        let mut h = historial();
        let o = Opciones {
            planificar: true,
            pensar: true,
            ..Default::default()
        };
        let (_, eventos, motor) = con_opciones(
            vec![
                plan_del_modelo("Thinking Process: 1. The user wants a red house.", ""),
                Respuesta::de_texto(PLAN_CASA),
                Respuesta::de_texto("Hecho."),
            ],
            &mut h,
            &o,
        );
        let plan = eventos
            .iter()
            .find_map(|e| if let Evento::Peticion(p) = e { Some(p.clone()) } else { None })
            .unwrap();
        assert!(plan.del_modelo, "el plan salio de la segunda vuelta");
        assert_eq!(plan.entiendo, "Dibujar una casa roja en el lienzo.");
        let pedidas = motor.recibido.lock().unwrap();
        assert_eq!(pedidas.len(), 3);
        assert!(!pedidas[1].pensar && pedidas[1].max_tokens == TOKENS_PLAN);
        assert!(pedidas[1].mensajes[1].texto_entero().contains("red house"));
    }

    #[test]
    fn el_plan_anade_las_herramientas_que_la_seleccion_no_dio() {
        let todas = catalogo();
        let solo_ocr: Vec<Herramienta> = todas.iter().filter(|h| h.nombre == "ocr").cloned().collect();
        let o = Opciones {
            planificar: true,
            ampliar: Some(&todas),
            ..Default::default()
        };
        let motor = MotorGuionado::nuevo(vec![Respuesta::de_texto(PLAN_CASA), Respuesta::de_texto("Vale.")]);
        let mut h = historial();
        turno_con(&motor, &mut h, &solo_ocr, &mut Apunta::default(), &o, &mut |_| Seguir::Si).unwrap();
        let pedidas = motor.recibido.lock().unwrap();
        // Planear ve todos los nombres; actuar, la seleccion mas lo del plan.
        assert!(pedidas[0].mensajes[0].texto_entero().contains("dibujar_en_lienzo"));
        let nombres: Vec<&str> = pedidas[1].herramientas.iter().map(|x| x.nombre.as_str()).collect();
        assert_eq!(nombres, vec!["ocr", "dibujar_en_lienzo"]);
    }

    #[test]
    fn parar_mientras_piensa_corta_sin_actuar() {
        let motor = MotorGuionado::nuevo(vec![plan_del_modelo("uno dos tres cuatro cinco", PLAN_CASA)]);
        let mut h = historial();
        let mut n = 0;
        let o = Opciones {
            planificar: true,
            pensar: true,
            ..Default::default()
        };
        let mut eventos = Vec::new();
        let f = turno_con(&motor, &mut h, &catalogo(), &mut Apunta::default(), &o, &mut |e| {
            eventos.push(e);
            n += 1;
            if n >= 2 { Seguir::Parar } else { Seguir::Si }
        })
        .unwrap();
        assert!(f.cortado);
        assert_eq!(motor.recibido.lock().unwrap().len(), 1, "no llego a actuar");
        assert!(matches!(eventos.last(), Some(Evento::PensoListo { .. })), "se pliega al parar");
    }

    #[test]
    fn parar_corta_el_turno() {
        let motor = MotorGuionado::nuevo(vec![Respuesta::de_texto("uno dos tres cuatro")]);
        let mut h = historial();
        let mut n = 0;
        let f = turno(&motor, &mut h, &catalogo(), &mut Apunta::default(), &mut |_| {
            n += 1;
            if n >= 2 { Seguir::Parar } else { Seguir::Si }
        })
        .unwrap();
        assert!(f.cortado);
    }

    #[test]
    fn una_respuesta_vacia_igual_dice_algo() {
        // Lo que hacia el 2B con una foto sola: ni texto ni herramienta.
        let (f, _, _, _) = correr(vec![Respuesta::de_texto("  ")], &mut Apunta::default());
        assert_eq!(f.texto, SIN_PALABRAS);
        // Y si hizo algo y luego no dijo nada, se dice que esta hecho.
        let (f, _, _, _) = correr(
            vec![
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"rectangulo","x":1,"y":1}]}"#),
                Respuesta::de_texto(""),
            ],
            &mut Apunta::default(),
        );
        assert_eq!(f.texto, HECHO);
        assert_eq!(f.herramientas_hechas, 1);
    }

    fn max_items_de_elementos(h: &[Herramienta]) -> u64 {
        let d = h.iter().find(|h| h.nombre == crate::herramientas::DIBUJAR).unwrap();
        d.parametros["properties"]["elementos"]["maxItems"].as_u64().unwrap()
    }

    #[test]
    fn las_listas_van_acotadas_y_tras_un_json_roto_mas_aun() {
        let mut ej = Apunta::default();
        let (_, _, _, motor) = correr(
            vec![
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"linea","x":1,"y":1},{"tipo":"li"#),
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"linea","x":1,"y":1,"x2":5,"y2":5}]}"#),
                Respuesta::de_texto("Listo."),
            ],
            &mut ej,
        );
        let pedidas = motor.recibido.lock().unwrap();
        assert_eq!(max_items_de_elementos(&pedidas[0].herramientas), TOPE_LISTAS);
        assert_eq!(max_items_de_elementos(&pedidas[1].herramientas), TOPE_LISTAS_REINTENTO);
        // Los textos sueltos tambien llevan tope; los de una lista cerrada no.
        let props = &pedidas[0].herramientas[0].parametros["properties"]["elementos"]["items"]["properties"];
        assert_eq!(props["texto"]["maxLength"].as_u64(), Some(TOPE_TEXTO));
        assert!(props["tipo"].get("maxLength").is_none());
        // Al modelo se le pide mas corto y en varias llamadas.
        assert!(pedidas[1].mensajes.last().unwrap().texto_entero().contains("varias llamadas"));
        // El catalogo de verdad (el que valida) no cambia.
        assert_eq!(max_items_de_elementos(&catalogo()), crate::herramientas::MAX_ELEMENTOS as u64);
        assert_eq!(ej.hechas.len(), 1);
    }

    #[test]
    fn con_topes_nunca_amplia_un_tope_mas_pequeno() {
        let h = vec![Herramienta {
            nombre: "x".into(),
            descripcion: String::new(),
            parametros: serde_json::json!({"type":"object","properties":{
                "a":{"type":"array","maxItems":3,"items":{"type":"string","maxLength":5}},
                "b":{"type":"array","items":{"type":"number"}}}}),
        }];
        let c = con_topes(&h, 10);
        assert_eq!(c[0].parametros["properties"]["a"]["maxItems"], 3);
        assert_eq!(c[0].parametros["properties"]["a"]["items"]["maxLength"], 5);
        // Una lista de numeros pesa poco: el doble.
        assert_eq!(c[0].parametros["properties"]["b"]["maxItems"], 20);
    }

    #[test]
    fn una_coordenada_que_se_sale_un_poco_se_lleva_al_borde_y_no_tira_la_llamada() {
        let mut ej = Apunta::default();
        let (f, eventos, _, _) = correr(
            vec![
                Respuesta::de_llamada(
                    "dibujar_en_lienzo",
                    r#"{"elementos":[{"tipo":"linea","x":10,"y":10,"x2":2010,"y2":-350}]}"#,
                ),
                Respuesta::de_texto("Listo."),
            ],
            &mut ej,
        );
        assert_eq!(f.reintentos, 0);
        assert_eq!(ej.hechas.len(), 1);
        assert!(!eventos.iter().any(|e| matches!(e, Evento::Reintento { .. })));
        let Llamada::Dibujar(figs) = &ej.hechas[0] else { panic!() };
        assert_eq!((figs[0].x2, figs[0].y2), (crate::herramientas::COORDENADA_MAXIMA, crate::herramientas::COORDENADA_MINIMA));
        // Lo que no es JSON se deja tal cual (lo dice la validacion).
        let rota = crate::motor::LlamadaCruda { id: "1".into(), nombre: "x".into(), argumentos: "{roto".into() };
        assert_eq!(sanear(&rota), rota);
    }

    /// Siempre falla como `llama-server` con una llamada cortada.
    struct SiempreCortada(std::sync::atomic::AtomicUsize);

    impl Motor for SiempreCortada {
        fn completar(&self, _: &Peticion, _: &mut dyn FnMut(&Trozo) -> Seguir) -> Result<Respuesta, ErrorIa> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(ErrorIa::Protocolo(
                "HTTP 500: {\"error\":{\"code\":500,\"message\":\"Failed to parse tool call arguments as JSON: \
                 [json.exception.parse_error.101] parse error at line 1, column 3514: missing closing quote\"}}"
                    .into(),
            ))
        }
    }

    #[test]
    fn una_llamada_siempre_cortada_acaba_en_palabras_y_no_en_error() {
        let motor = SiempreCortada(Default::default());
        let mut eventos = Vec::new();
        let f = turno(&motor, &mut historial(), &catalogo(), &mut Apunta::default(), &mut |e| {
            eventos.push(e);
            Seguir::Si
        })
        .expect("no es un error para el usuario");
        assert!(f.texto.contains("No he sabido"), "{}", f.texto);
        assert!(!f.texto.contains("HTTP") && !f.texto.contains('{'));
        assert_eq!(motor.0.load(std::sync::atomic::Ordering::SeqCst), REINTENTOS + 1);
        assert_eq!(eventos.iter().filter(|e| matches!(e, Evento::Reintento { .. })).count(), REINTENTOS);
    }

    #[test]
    fn el_fallo_para_el_usuario_no_lleva_nada_tecnico() {
        for e in [
            ErrorIa::Protocolo("HTTP 500: {\"error\":{\"code\":500}}".into()),
            ErrorIa::Conexion("os error 10054".into()),
            ErrorIa::Arranque("el motor se cerro (exit code: 0xc0000005)".into()),
            ErrorIa::Falta("C:\\x\\llama-server.exe".into()),
        ] {
            let t = para_el_usuario(&e);
            assert!(!t.contains("HTTP") && !t.contains('{') && !t.contains("0x") && !t.contains("C:\\"), "{t}");
            assert!(!t.is_empty());
        }
    }

    #[test]
    fn el_tope_de_tokens_cabe_en_el_contexto() {
        let corta = Peticion::nueva(historial());
        assert_eq!(tope_de_tokens(&corta), MAX_TOKENS_CON_HERRAMIENTAS);
        // Una conversacion que llena el contexto: menos respuesta, nunca
        // menos del minimo.
        let mut larga = historial();
        larga.push(Mensaje::usuario("x".repeat(15_000)));
        let t = tope_de_tokens(&Peticion::nueva(larga.clone()));
        assert!(t < MAX_TOKENS_CON_HERRAMIENTAS && t >= MIN_TOKENS, "{t}");
        larga.push(Mensaje::usuario("x".repeat(60_000)));
        assert_eq!(tope_de_tokens(&Peticion::nueva(larga)), MIN_TOKENS);
    }
}

#[cfg(test)]
mod pruebas_rapidez {
    use super::*;
    use crate::herramientas::catalogo;
    use crate::motor::MotorGuionado;

    /// Apunta lo que le piden; falla si se le dice.
    struct Apunta(Vec<String>, Option<String>);

    impl Ejecutor for Apunta {
        fn ejecutar(&mut self, l: &Llamada) -> Result<String, String> {
            self.0.push(l.nombre().to_string());
            match &self.1 {
                Some(e) => Err(e.clone()),
                None => Ok("Apuntada «comprar pan».".into()),
            }
        }
    }

    fn con_rapido(pedido: &str, ej: &mut Apunta, guion: Vec<Respuesta>) -> (Final, Vec<Evento>, usize) {
        let motor = MotorGuionado::nuevo(guion);
        let mut h = vec![Mensaje::sistema(instrucciones()), Mensaje::usuario(pedido)];
        let o = Opciones {
            planificar: true,
            rapido: Some(crate::tareas::pruebas::ahora()),
            ..Default::default()
        };
        let mut eventos = Vec::new();
        let f = turno_con(&motor, &mut h, &catalogo(), ej, &o, &mut |e| {
            eventos.push(e);
            Seguir::Si
        })
        .unwrap();
        let pedidas = motor.recibido.lock().unwrap().len();
        (f, eventos, pedidas)
    }

    #[test]
    fn una_orden_simple_no_pregunta_al_modelo() {
        let mut ej = Apunta(Vec::new(), None);
        let t0 = std::time::Instant::now();
        let (f, eventos, pedidas) = con_rapido("apunta comprar pan", &mut ej, vec![]);
        assert!(t0.elapsed() < std::time::Duration::from_millis(100), "{:?}", t0.elapsed());
        assert_eq!(pedidas, 0, "ni plan ni modelo");
        assert_eq!(ej.0, vec!["anadir_tarea"]);
        assert_eq!(f.texto, "Apuntada «comprar pan».");
        assert!(matches!(eventos.last(), Some(Evento::Texto(t)) if t == "Apuntada «comprar pan»."));
    }

    #[test]
    fn si_el_camino_rapido_no_puede_contesta_el_modelo() {
        // Un fallo de la app (no contesta): se pregunta al modelo (con su
        // plan), sin que la burbuja haya dicho nada antes.
        let mut ej = Apunta(Vec::new(), Some("PixPin no contesto".into()));
        let (f, eventos, pedidas) = con_rapido(
            "tacha pintar la luna",
            &mut ej,
            vec![Respuesta::de_texto("ENTIENDO: x\nHARE: y\nHERRAMIENTAS: ninguna\nDATOS: -\nDECIDIDO: -"), Respuesta::de_texto("No la encuentro.")],
        );
        assert_eq!(pedidas, 2);
        assert_eq!(f.texto, "No la encuentro.");
        assert!(!eventos.iter().any(|e| matches!(e, Evento::Herramienta { .. })));
        // Y lo que no es simple va directo al modelo.
        // La tarea no esta: el motivo es la respuesta, sin modelo.
        let mut ej = Apunta(Vec::new(), Some("No encuentro ninguna tarea «pintar la luna». No hice nada.".into()));
        let (f, _, pedidas) = con_rapido("tacha pintar la luna", &mut ej, vec![]);
        assert_eq!(pedidas, 0);
        assert!(f.texto.starts_with("No encuentro") && f.herramientas_hechas == 0);
        let mut ej = Apunta(Vec::new(), None);
        let (_, _, pedidas) = con_rapido("Apunta comprar yeso y dibuja un círculo", &mut ej, vec![
            Respuesta::de_texto("ENTIENDO: x\nHARE: y\nHERRAMIENTAS: ninguna\nDATOS: -\nDECIDIDO: -"),
            Respuesta::de_texto("Vale."),
        ]);
        assert_eq!(pedidas, 2);
        assert!(ej.0.is_empty());
    }

    /// Hace las llamadas de una respuesta de una vez y dice en que orden.
    struct Juntas(Vec<usize>);

    impl Ejecutor for Juntas {
        fn ejecutar(&mut self, _: &Llamada) -> Result<String, String> {
            unreachable!("van todas por ejecutar_varias")
        }
        fn ejecutar_varias(&mut self, l: &[Llamada]) -> Vec<Result<String, String>> {
            self.0.push(l.len());
            (0..l.len()).map(|i| if i == 1 { Err("roto".into()) } else { Ok(format!("r{i}")) }).collect()
        }
    }

    #[test]
    fn varias_llamadas_se_hacen_juntas_y_vuelven_en_orden() {
        let motor = MotorGuionado::nuevo(vec![
            Respuesta {
                llamadas: vec![
                    crate::motor::LlamadaCruda { id: "a".into(), nombre: "ocr".into(), argumentos: "{}".into() },
                    crate::motor::LlamadaCruda { id: "b".into(), nombre: "ocr".into(), argumentos: "{}".into() },
                    crate::motor::LlamadaCruda { id: "c".into(), nombre: "inventada".into(), argumentos: "{}".into() },
                    crate::motor::LlamadaCruda { id: "d".into(), nombre: "ayuda_app".into(), argumentos: r#"{"pregunta":"x"}"#.into() },
                ],
                ..Respuesta::de_llamada("ocr", "{}")
            },
            Respuesta::de_texto("Listo."),
        ]);
        let mut h = vec![Mensaje::sistema(instrucciones()), Mensaje::usuario("lee los dos")];
        let mut ej = Juntas(Vec::new());
        turno(&motor, &mut h, &catalogo(), &mut ej, &mut |_| Seguir::Si).unwrap();
        assert_eq!(ej.0, vec![3], "las tres buenas, de una vez");
        let resultados: Vec<(String, String)> = h
            .iter()
            .filter(|m| m.rol == Rol::Herramienta)
            .map(|m| (m.id_llamada.clone().unwrap_or_default(), m.texto_entero()))
            .collect();
        let ids: Vec<&str> = resultados.iter().map(|(i, _)| i.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c", "d"]);
        assert_eq!(resultados[0].1, "r0");
        assert_eq!(resultados[1].1, "ERROR: roto");
        assert!(resultados[2].1.contains("inventada"));
        assert_eq!(resultados[3].1, "r2");
    }

    #[test]
    fn un_resultado_largo_llega_recortado_al_modelo() {
        let corto = recortar_resultado("hola".into());
        assert_eq!(corto, "hola");
        let largo = recortar_resultado("x".repeat(RESULTADO_MAXIMO + 50));
        assert!(largo.chars().count() < RESULTADO_MAXIMO + 80 && largo.contains("recortado"));
    }
}

#[cfg(test)]
mod pruebas_topes {
    use super::*;
    use crate::herramientas::catalogo;

    fn esquema<'a>(hs: &'a [Herramienta], nombre: &str) -> &'a serde_json::Value {
        &hs.iter().find(|h| h.nombre == nombre).unwrap_or_else(|| panic!("falta {nombre}")).parametros["properties"]
    }

    /// **El tope de 120 es para etiquetas**; lo largo por naturaleza lleva
    /// el suyo, que sale del esquema de cada herramienta (hallado el 14-oct:
    /// un SVG llegaba al modelo local acotado a 120 letras).
    #[test]
    fn los_textos_largos_llevan_su_tope_y_las_etiquetas_el_corto() {
        let hs = con_topes(&catalogo(), TOPE_LISTAS);
        let largo = |h: &str, campo: &str| esquema(&hs, h)[campo]["maxLength"].as_u64().unwrap();
        assert_eq!(largo(crate::svg::NOMBRE, "svg"), crate::svg::MAX_BYTES as u64);
        assert_eq!(largo("nota_nueva", "texto"), crate::acciones::LARGO_LARGO as u64);
        assert_eq!(largo("escribir_en_chat", "texto"), crate::acciones::LARGO_LARGO as u64);
        assert_eq!(largo("crear_automatizacion", "pasos"), crate::acciones::LARGO_LARGO as u64);
        assert_eq!(largo(crate::ecuacion::NOMBRE, "ecuacion"), crate::ecuacion::MAX_LARGO as u64);
        // Las etiquetas de las figuras, cortas.
        let fig = &esquema(&hs, DIBUJAR_)["elementos"]["items"]["properties"];
        assert_eq!(fig["texto"]["maxLength"].as_u64(), Some(TOPE_TEXTO));
        // Un SVG de 5 KB cabe entero; una etiqueta de 500 no.
        let svg = format!("<svg>{}</svg>", "<rect width=\"1\" height=\"1\"/>".repeat(180));
        assert!(svg.len() > 5000 && (svg.len() as u64) < largo(crate::svg::NOMBRE, "svg"));
        assert!(500 > fig["texto"]["maxLength"].as_u64().unwrap());
        // Y la validacion de la tabla deja pasar una nota de 5000 letras y
        // corta una de 9000 a su tope.
        let a = crate::acciones::por_nombre("nota_nueva").unwrap();
        let args = |n: usize| serde_json::json!({"texto": "x".repeat(n)}).as_object().unwrap().clone();
        let v = crate::acciones::validar(a, &args(5000)).unwrap();
        assert_eq!(v.pedido["texto"].as_str().unwrap().len(), 5000);
        let v = crate::acciones::validar(a, &args(9000)).unwrap();
        assert_eq!(v.pedido["texto"].as_str().unwrap().len(), crate::acciones::LARGO_LARGO);
    }

    const DIBUJAR_: &str = crate::herramientas::DIBUJAR;
}
