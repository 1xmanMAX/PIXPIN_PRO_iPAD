//! **El asistente con modelos de la nube**: Claude Haiku 5.5 o DeepSeek
//! V4.1 Flash, con la clave del usuario.
//!
//! Pedido del usuario (13-oct-2026): poder cambiar del modelo local a la API
//! de Claude o de DeepSeek, con los modelos ya elegidos (solo se le pide la
//! clave), con instrucciones optimizadas para cada uno, que puedan usar todo
//! lo de la app y que fallen poco gracias a buenas instrucciones y entorno.
//!
//! ## Como encaja
//!
//! [`MotorNube`] implementa el mismo [`Motor`] que llama.cpp: el bucle del
//! agente (`agente::turno`), el catalogo de herramientas, su validacion y el
//! chat no cambian. Cada proveedor tiene su **adaptador** ([`claude`],
//! [`deepseek`]) que traduce la peticion comun y lee su streaming, y las
//! instrucciones salen de un solo componente con variables
//! ([`instrucciones`]).
//!
//! ## Fallar poco y fallar bien
//!
//! - Herramientas estrictas en Claude; reglas + ejemplo en DeepSeek; y en
//!   los dos la validacion de `herramientas::validar` antes de ejecutar,
//!   con el reintento corto del agente si una llamada viene mal.
//! - 429, 5xx, «saturado» y cortes de red: hasta dos reintentos con espera
//!   (respetando `retry-after`), **solo si aun no se enseno nada** (no se
//!   duplica texto en la burbuja).
//! - Clave mala, sin saldo, peticion rechazada: un [`ErrorIa::Nube`] con un
//!   aviso en espanol que dice que hacer. [`MotorConRespaldo`] sigue con el
//!   modelo local si esta bajado y el usuario no lo apago.
//! - Una respuesta cortada por `max_tokens` con una herramienta a medias no
//!   se ejecuta: vuelve al agente como llamada rota (que pide repetirla mas
//!   corta). Un rechazo (`refusal`) no ejecuta nada y se dice.
//!
//! Las claves: [`claves`] (DPAPI, fichero propio, nunca en el TOML).

pub mod claude;
pub mod claves;
pub mod deepseek;
pub mod http;
pub mod instrucciones;

#[cfg(test)]
mod pruebas;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::ErrorIa;
use crate::motor::{Fin, Motor, Peticion, Respuesta, Seguir, Trozo};
use crate::sse::Acumulador;

/// A quien se le pregunta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Proveedor {
    Claude,
    DeepSeek,
}

impl Proveedor {
    pub const TODOS: [Proveedor; 2] = [Proveedor::Claude, Proveedor::DeepSeek];

    /// Como se guarda en los ajustes (`[asistente] motor`) y en el nombre
    /// del fichero de la clave.
    pub fn id(self) -> &'static str {
        match self {
            Proveedor::Claude => "claude",
            Proveedor::DeepSeek => "deepseek",
        }
    }

    pub fn por_id(id: &str) -> Option<Proveedor> {
        Proveedor::TODOS.into_iter().find(|p| p.id() == id.trim())
    }

    /// El nombre que ve el usuario.
    pub fn nombre(self) -> &'static str {
        match self {
            Proveedor::Claude => "Claude Haiku 5.5",
            Proveedor::DeepSeek => "DeepSeek V4.1 Flash",
        }
    }

    /// El id exacto del modelo en su API.
    pub fn modelo(self) -> &'static str {
        match self {
            Proveedor::Claude => claude::MODELO,
            Proveedor::DeepSeek => deepseek::MODELO,
        }
    }

    pub fn url(self) -> &'static str {
        match self {
            Proveedor::Claude => claude::URL,
            Proveedor::DeepSeek => deepseek::URL,
        }
    }

    /// Donde se saca la clave.
    pub fn pagina_de_claves(self) -> &'static str {
        match self {
            Proveedor::Claude => "https://platform.claude.com/settings/keys",
            Proveedor::DeepSeek => "https://platform.deepseek.com/api_keys",
        }
    }

    fn max_tokens(self) -> u32 {
        match self {
            Proveedor::Claude => claude::MAX_TOKENS,
            Proveedor::DeepSeek => deepseek::MAX_TOKENS,
        }
    }

    fn cabeceras(self, clave: &str) -> Vec<(&'static str, String)> {
        match self {
            Proveedor::Claude => claude::cabeceras(clave),
            Proveedor::DeepSeek => deepseek::cabeceras(clave),
        }
    }
}

/// Lo que se suma al tope de la respuesta para que el modelo pueda pensar.
pub const RESERVA_PENSAR: u32 = 8_192;

/// Cuanto se espera antes de cada reintento (si el servidor no dice otra
/// cosa con `retry-after`).
pub const ESPERAS: [Duration; 2] = [Duration::from_secs(1), Duration::from_secs(4)];

/// Lo mas que se espera por un `retry-after`: mas que esto y es mejor
/// decirselo al usuario.
const ESPERA_MAXIMA: Duration = Duration::from_secs(20);

/// Lo que se hace tras un intento fallido.
enum Intento {
    /// Algo pasajero (429, 5xx, red): otra vez tras esperar.
    Reintentar {
        error: ErrorIa,
        esperar: Option<Duration>,
    },
    /// Se cambio un ajuste de la peticion (sin pensar): otra
    /// vez ya.
    Cambiar(ErrorIa),
    /// No tiene arreglo aqui.
    Fallo(ErrorIa),
}

/// **El aviso para el usuario** de un codigo HTTP, y si vale la pena
/// reintentar.
pub fn clasificar(p: Proveedor, codigo: u32, cuerpo: &str) -> (String, bool) {
    let detalle = serde_json::from_str::<Value>(cuerpo)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| cuerpo.chars().take(160).collect());
    let bajo = detalle.to_lowercase();
    let n = p.nombre();
    let sin_saldo =
        codigo == 402 || bajo.contains("credit balance") || bajo.contains("insufficient balance");
    match codigo {
        _ if sin_saldo => (
            format!(
                "Tu cuenta de {n} no tiene saldo. Recargala en su web o vuelve al asistente local en Ajustes → Asistente."
            ),
            false,
        ),
        401 => (
            format!(
                "{n} no acepta la clave: esta mal copiada o se revoco. Pega otra en Ajustes → Asistente."
            ),
            false,
        ),
        403 => (
            format!(
                "La clave de {n} no tiene permiso para usar este modelo. Revisa tu cuenta o usa otra clave."
            ),
            false,
        ),
        404 => (
            format!("{n} ya no ofrece el modelo que usa PixPin. Actualiza PixPin."),
            false,
        ),
        413 => (
            format!("La peticion es demasiado grande para {n}; prueba con una foto mas pequena."),
            false,
        ),
        429 => (
            format!("Demasiadas peticiones seguidas a {n}; espera un momento y vuelve a pedirlo."),
            true,
        ),
        500 | 502 | 503 | 504 | 529 => (
            format!("{n} esta saturado o fallo; vuelve a intentarlo en un rato."),
            true,
        ),
        _ => {
            tracing::warn!(codigo, %detalle, "asistente: la nube rechazo la peticion");
            (
                format!(
                    "{n} no acepto la peticion. Prueba a pedirlo de otra forma o vuelve al asistente local en Ajustes → Asistente."
                ),
                false,
            )
        }
    }
}

/// **El motor de la nube.** Uno por proveedor y clave; se puede compartir
/// entre hilos (cada `completar` es una peticion HTTP independiente).
pub struct MotorNube {
    proveedor: Proveedor,
    clave: String,
    url: String,
    /// Lo que la API pide que se le devuelva y el `Mensaje` comun no lleva:
    /// los bloques de Claude (pensamiento firmado incluido) o el
    /// `reasoning_content` de DeepSeek, por id de la primera llamada.
    memoria: Mutex<HashMap<String, Value>>,
    /// Si piensa (Claude adaptativo, DeepSeek `thinking`). Lo apaga un turno
    /// simple ([`MotorNube::pensando`]) y, en DeepSeek, que pida un
    /// razonamiento que no hay.
    pensar: AtomicBool,
    esperas: Vec<Duration>,
    /// Sin pensar y con el menor esfuerzo: para preguntas cortas de una
    /// vuelta (el LaTeX de un recorte, `hoja::vision`), donde pensar solo
    /// anade segundos.
    ligero: bool,
    /// Otro modelo del mismo proveedor (el critico de respaldo,
    /// [`claude::MODELO_CRITICO`]); `None`: el de siempre.
    modelo: Option<&'static str>,
}

/// **La web, apagada** en este proceso: la cuenta de Claude contesto que no
/// la tiene activada (400 «Web search is not enabled for this
/// organization»). Desde entonces las peticiones van sin ella.
static WEB_APAGADA: AtomicBool = AtomicBool::new(false);

/// Si el asistente puede usar la web (la cuenta no la rechazo).
pub fn web_disponible() -> bool {
    !WEB_APAGADA.load(Ordering::Relaxed)
}

/// Quita del cuerpo de Claude las herramientas de la web (y `tools` si se
/// queda vacio, que la API no lo admite).
fn sin_la_web(c: &mut Value) {
    let Some(tools) = c.get_mut("tools").and_then(Value::as_array_mut) else {
        return;
    };
    tools.retain(|t| {
        !t.get("type")
            .and_then(Value::as_str)
            .is_some_and(|x| x.starts_with("web_"))
    });
    if tools.is_empty()
        && let Some(m) = c.as_object_mut()
    {
        m.remove("tools");
        m.remove("tool_choice");
    }
}

impl std::fmt::Debug for MotorNube {
    // A mano: la clave nunca sale por `{:?}` (ni a un registro).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MotorNube")
            .field("proveedor", &self.proveedor)
            .field("clave", &claves::enmascarar(&self.clave))
            .field("url", &self.url)
            .finish()
    }
}

impl MotorNube {
    pub fn nuevo(proveedor: Proveedor, clave: impl Into<String>) -> MotorNube {
        MotorNube {
            proveedor,
            clave: clave.into().trim().to_string(),
            url: proveedor.url().to_string(),
            memoria: Mutex::new(HashMap::new()),
            pensar: AtomicBool::new(true),
            esperas: ESPERAS.to_vec(),
            ligero: false,
            modelo: None,
        }
    }

    /// Otra direccion (el servidor falso de las pruebas).
    pub fn con_url(mut self, url: impl Into<String>) -> MotorNube {
        self.url = url.into();
        self
    }

    /// Otras esperas entre reintentos (las pruebas no esperan segundos).
    pub fn con_esperas(mut self, esperas: Vec<Duration>) -> MotorNube {
        self.esperas = esperas;
        self
    }

    /// **Sin pensar** este turno (`crate::complejidad`: la tarea es simple),
    /// con el esfuerzo de siempre. `pensar: false` es eso; `true` lo deja
    /// como esta.
    pub fn pensando(self, pensar: bool) -> MotorNube {
        if !pensar {
            self.pensar.store(false, Ordering::Relaxed);
        }
        self
    }

    /// **Ligero**: sin pensar y con el menor esfuerzo (ver `ligero`).
    pub fn ligero(mut self) -> MotorNube {
        self.ligero = true;
        self.pensar = AtomicBool::new(false);
        self
    }

    /// **Otro modelo** del mismo proveedor y con la misma clave (el critico
    /// de respaldo de la revision: [`claude::MODELO_CRITICO`]).
    pub fn con_modelo(mut self, modelo: &'static str) -> MotorNube {
        self.modelo = Some(modelo);
        self
    }

    pub fn proveedor(&self) -> Proveedor {
        self.proveedor
    }

    fn cuerpo(&self, p: &Peticion) -> Value {
        let memoria = self.memoria.lock().map(|m| m.clone()).unwrap_or_default();
        // El tope del agente (`agente::tope_de_tokens`) es para la RESPUESTA;
        // en la nube el pensamiento cuenta dentro del mismo `max_tokens`, asi
        // que se le suma su reserva. Las listas acotadas (`agente::con_topes`)
        // llegan ya en las herramientas de la peticion.
        // Sin pensar no hace falta sitio para el pensamiento.
        let pensar = self.pensar.load(Ordering::Relaxed);
        let reserva = if self.ligero || !pensar {
            0
        } else {
            RESERVA_PENSAR
        };
        let max = p
            .max_tokens
            .saturating_add(reserva)
            .min(self.proveedor.max_tokens());
        let mut c = match self.proveedor {
            Proveedor::Claude if self.ligero => {
                let mut c = claude::cuerpo(p, &memoria, max);
                if let Some(m) = c.as_object_mut() {
                    m.remove("thinking");
                }
                c["output_config"] = serde_json::json!({"effort": "low"});
                c
            }
            Proveedor::Claude if !pensar => {
                let mut c = claude::cuerpo(p, &memoria, max);
                c["thinking"] = serde_json::json!({"type": "disabled"});
                c
            }
            Proveedor::Claude => claude::cuerpo(p, &memoria, max),
            Proveedor::DeepSeek => deepseek::cuerpo(p, &memoria, pensar, max),
        };
        if self.proveedor == Proveedor::Claude && !web_disponible() {
            sin_la_web(&mut c);
        }
        if let Some(m) = self.modelo {
            c["model"] = Value::String(m.into());
        }
        c
    }

    fn recordar(&self, id: &str, v: Value) {
        if let Ok(mut m) = self.memoria.lock() {
            // Una conversacion larga no hace crecer esto sin fin: lo viejo ya
            // no se reenvia (el chat solo manda texto de los turnos de antes).
            if m.len() > 256 {
                m.clear();
            }
            m.insert(id.to_string(), v);
        }
    }

    fn nube(&self, m: impl Into<String>) -> ErrorIa {
        ErrorIa::Nube(m.into())
    }

    /// Un intento: una peticion HTTP entera.
    fn una_vez(
        &self,
        p: &Peticion,
        al_trozo: &mut dyn FnMut(&Trozo) -> Seguir,
        emitido: &mut bool,
    ) -> Result<Respuesta, Intento> {
        let cuerpo = serde_json::to_vec(&self.cuerpo(p))
            .map_err(|e| Intento::Fallo(ErrorIa::Protocolo(e.to_string())))?;
        let mut grabacion = Grabacion::nueva(self.proveedor, &cuerpo);
        let pedido = http::Pedido {
            url: &self.url,
            cabeceras: self.proveedor.cabeceras(&self.clave),
            cuerpo: &cuerpo,
        };
        let n = self.proveedor.nombre();
        match self.proveedor {
            Proveedor::Claude => {
                let mut lector = claude::Lector::nuevo();
                let mut roto: Option<ErrorIa> = None;
                let r = http::enviar(&pedido, &mut |b| match lector.meter(grabacion.pasar(b)) {
                    Ok(trozos) => {
                        for t in trozos {
                            *emitido = true;
                            if al_trozo(&t) == Seguir::Parar {
                                return Seguir::Parar;
                            }
                        }
                        Seguir::Si
                    }
                    Err(e) => {
                        roto = Some(e);
                        Seguir::Parar
                    }
                });
                let cortado = self.resultado_http(r)?;
                if let Some(e) = roto {
                    return Err(Intento::Fallo(e));
                }
                if let Some(e) = lector.error.clone() {
                    // `overloaded_error` y `api_error` a mitad del flujo son
                    // pasajeros; lo demas, no.
                    let pasajero = matches!(
                        e.tipo.as_str(),
                        "overloaded_error" | "api_error" | "rate_limit_error"
                    );
                    tracing::warn!(tipo = %e.tipo, mensaje = %e.mensaje, "asistente: error a mitad del flujo");
                    let error = self.nube(format!(
                        "{n} fallo a mitad de la respuesta; vuelve a pedirlo."
                    ));
                    return Err(if pasajero {
                        Intento::Reintentar {
                            error,
                            esperar: None,
                        }
                    } else {
                        Intento::Fallo(error)
                    });
                }
                if !cortado && !lector.acabado {
                    return Err(Intento::Reintentar {
                        error: self.nube(format!(
                            "La respuesta de {n} llego cortada; vuelve a pedirlo."
                        )),
                        esperar: None,
                    });
                }
                let salida = lector.acabar(cortado);
                let mut r = salida.respuesta;
                match salida.motivo.as_deref() {
                    Some("refusal") => {
                        // Nada de lo que pidio se ejecuta: un rechazo puede
                        // cortar una llamada a medias.
                        let aviso = format!(
                            "{}{n} no quiso hacer esto{}. Prueba a pedirlo de otra forma.",
                            if r.texto.is_empty() { "" } else { "\n\n" },
                            salida
                                .categoria
                                .map(|_| " (su filtro de seguridad lo paro)".to_string())
                                .unwrap_or_default()
                        );
                        let _ = al_trozo(&Trozo::Texto(aviso.clone()));
                        r.texto.push_str(&aviso);
                        r.llamadas.clear();
                        r.fin = Fin::Natural;
                        return Ok(r);
                    }
                    Some("max_tokens") if !r.llamadas.is_empty() => {
                        // El agente lo trata como llamada rota y pide repetirla
                        // mas corta (ver `agente::llamada_rota`).
                        return Err(Intento::Fallo(ErrorIa::Protocolo(
                            "la tool call salio cortada por max_tokens: JSON incompleto".into(),
                        )));
                    }
                    _ => {}
                }
                if r.fin != Fin::Cortado
                    && let Some(primera) = r.llamadas.first()
                {
                    self.recordar(&primera.id, Value::Array(salida.bloques));
                }
                Ok(r)
            }
            Proveedor::DeepSeek => {
                let mut acum = Acumulador::nuevo();
                let mut roto: Option<ErrorIa> = None;
                let r = http::enviar(&pedido, &mut |b| match acum.meter(grabacion.pasar(b)) {
                    Ok(trozos) => {
                        for t in trozos {
                            *emitido = true;
                            if al_trozo(&t) == Seguir::Parar {
                                return Seguir::Parar;
                            }
                        }
                        Seguir::Si
                    }
                    Err(e) => {
                        roto = Some(e);
                        Seguir::Parar
                    }
                });
                let cortado = self.resultado_http(r)?;
                if let Some(e) = roto {
                    tracing::warn!(%e, "asistente: error a mitad del flujo");
                    let error = self.nube(format!(
                        "{n} fallo a mitad de la respuesta; vuelve a pedirlo."
                    ));
                    return Err(Intento::Reintentar {
                        error,
                        esperar: None,
                    });
                }
                if !cortado && !acum.acabado() && !*emitido {
                    return Err(Intento::Reintentar {
                        error: self.nube(format!(
                            "La respuesta de {n} llego vacia; vuelve a pedirlo."
                        )),
                        esperar: None,
                    });
                }
                let r = acum.respuesta(cortado);
                if r.fin != Fin::Cortado
                    && self.pensar.load(Ordering::Relaxed)
                    && let Some(primera) = r.llamadas.first()
                {
                    self.recordar(&primera.id, Value::String(r.pensamiento.clone()));
                }
                Ok(r)
            }
        }
    }

    /// Lo que dijo la capa HTTP, ya como intento.
    fn resultado_http(&self, r: Result<bool, http::Fallo>) -> Result<bool, Intento> {
        let n = self.proveedor.nombre();
        match r {
            Ok(cortado) => Ok(cortado),
            Err(http::Fallo::Red(d)) => Err(Intento::Reintentar {
                error: {
                    tracing::warn!(%d, "asistente: sin conexion con la nube");
                    self.nube(format!("No hay conexion con {n}. Mira la red o vuelve al asistente local en Ajustes → Asistente."))
                },
                esperar: None,
            }),
            Err(http::Fallo::Http {
                codigo,
                cuerpo,
                esperar,
            }) => {
                let bajo = cuerpo.to_lowercase();
                if codigo == 400
                    && self.proveedor == Proveedor::Claude
                    && (bajo.contains("web search") || bajo.contains("web fetch"))
                    && !WEB_APAGADA.swap(true, Ordering::Relaxed)
                {
                    // La cuenta no tiene la web activada (Console →
                    // Privacidad): se sigue sin ella, sin romper el chat.
                    tracing::warn!(
                        "asistente: la cuenta de {n} no tiene la web activada; se sigue sin la web"
                    );
                    return Err(Intento::Cambiar(
                        self.nube(clasificar(self.proveedor, codigo, &cuerpo).0),
                    ));
                }
                if codigo == 400
                    && self.proveedor == Proveedor::DeepSeek
                    && bajo.contains("reasoning")
                    && self.pensar.swap(false, Ordering::Relaxed)
                {
                    tracing::warn!(
                        "asistente: {n} pidio un razonamiento que no habia; se sigue sin pensar"
                    );
                    return Err(Intento::Cambiar(
                        self.nube(clasificar(self.proveedor, codigo, &cuerpo).0),
                    ));
                }
                let (aviso, pasajero) = clasificar(self.proveedor, codigo, &cuerpo);
                tracing::warn!(codigo, "asistente: {n} contesto con error");
                let error = self.nube(aviso);
                Err(if pasajero {
                    Intento::Reintentar { error, esperar }
                } else {
                    Intento::Fallo(error)
                })
            }
        }
    }
}

impl Motor for MotorNube {
    fn catalogo_completo(&self) -> bool {
        true
    }

    fn a_la_vez(&self) -> usize {
        match self.proveedor() {
            Proveedor::Claude => 8,
            Proveedor::DeepSeek => 3,
        }
    }

    /// Claude Haiku 5.5, Sonnet y DeepSeek V4.1 Flash ven imagenes;
    /// DeepSeek V4 Pro (el jefe con DeepSeek) no.
    fn ve_imagenes(&self) -> bool {
        self.modelo != Some(deepseek::MODELO_PRO)
    }

    fn completar(
        &self,
        p: &Peticion,
        al_trozo: &mut dyn FnMut(&Trozo) -> Seguir,
    ) -> Result<Respuesta, ErrorIa> {
        if self.clave.is_empty() {
            return Err(ErrorIa::Nube(format!(
                "Falta la clave de {}: pegala en Ajustes → Asistente.",
                self.proveedor.nombre()
            )));
        }
        let mut reintentos = 0;
        let mut cambios = 0;
        loop {
            let mut emitido = false;
            match self.una_vez(p, al_trozo, &mut emitido) {
                Ok(r) => return Ok(r),
                Err(Intento::Fallo(e)) => return Err(e),
                Err(Intento::Cambiar(e)) => {
                    if emitido || cambios >= 2 {
                        return Err(e);
                    }
                    cambios += 1;
                }
                Err(Intento::Reintentar { error, esperar }) => {
                    // Con algo ya en la burbuja no se repite: saldria doble.
                    if emitido || reintentos >= self.esperas.len() {
                        return Err(error);
                    }
                    let espera = esperar.unwrap_or(self.esperas[reintentos]);
                    if espera > ESPERA_MAXIMA {
                        return Err(error);
                    }
                    reintentos += 1;
                    std::thread::sleep(espera);
                }
            }
        }
    }
}

/// **Grabar lo que va y viene**, para diagnosticar: con `PIXPIN_IA_GRABAR=<carpeta>`
/// cada peticion deja su cuerpo JSON (`*-peticion.json`) y la respuesta SSE
/// cruda (`*-respuesta.sse`). Las cabeceras (la clave) NUNCA se graban.
/// Sin la variable no hace nada.
struct Grabacion {
    respuesta: Option<std::fs::File>,
}

impl Grabacion {
    fn nueva(p: Proveedor, cuerpo: &[u8]) -> Grabacion {
        let Some(dir) = std::env::var_os("PIXPIN_IA_GRABAR").map(std::path::PathBuf::from) else {
            return Grabacion { respuesta: None };
        };
        let _ = std::fs::create_dir_all(&dir);
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let base = dir.join(format!("{}-{}-{n:03}", p.id(), std::process::id()));
        let _ = std::fs::write(base.with_extension("peticion.json"), cuerpo);
        Grabacion {
            respuesta: std::fs::File::create(base.with_extension("respuesta.sse")).ok(),
        }
    }

    /// Apunta `b` y lo devuelve, para leerlo a la vez.
    fn pasar<'a>(&mut self, b: &'a [u8]) -> &'a [u8] {
        if let Some(f) = &mut self.respuesta {
            use std::io::Write;
            let _ = f.write_all(b);
        }
        b
    }
}

/// **«Probar»**: una llamada minima con la clave. `Ok` con una frase para
/// ensenar; `Err` con el aviso de que paso.
pub fn probar(p: Proveedor, clave: &str) -> Result<String, String> {
    probar_en(p, clave, p.url())
}

pub fn probar_en(p: Proveedor, clave: &str, url: &str) -> Result<String, String> {
    let clave = clave.trim();
    if clave.is_empty() {
        return Err("escribe la clave primero".into());
    }
    let cuerpo = match p {
        Proveedor::Claude => claude::cuerpo_de_prueba(),
        Proveedor::DeepSeek => deepseek::cuerpo_de_prueba(),
    };
    let cuerpo = serde_json::to_vec(&cuerpo).map_err(|e| e.to_string())?;
    let pedido = http::Pedido {
        url,
        cabeceras: p.cabeceras(clave),
        cuerpo: &cuerpo,
    };
    let mut respuesta = Vec::new();
    match http::enviar(&pedido, &mut |b| {
        respuesta.extend_from_slice(b);
        if respuesta.len() > 64 * 1024 {
            Seguir::Parar
        } else {
            Seguir::Si
        }
    }) {
        Ok(_) => {
            let v: Value = serde_json::from_slice(&respuesta)
                .map_err(|_| format!("{} contesto algo que no se entiende", p.nombre()))?;
            let modelo = v.get("model").and_then(Value::as_str).unwrap_or(p.modelo());
            Ok(format!(
                "la clave funciona: {} ({modelo}) contesto.",
                p.nombre()
            ))
        }
        Err(http::Fallo::Red(d)) => {
            tracing::warn!(%d, "asistente: sin conexion al probar la clave");
            Err(format!("no hay conexion con {}", p.nombre()))
        }
        Err(http::Fallo::Http { codigo, cuerpo, .. }) => Err(clasificar(p, codigo, &cuerpo).0),
    }
}

/// La peticion para el modelo local cuando hace de respaldo: con las pocas
/// herramientas que vienen a cuento (`seleccion`), como si lo hubiera
/// elegido el usuario; con el catalogo entero un 2B se pierde.
pub fn para_el_local(p: &Peticion) -> Peticion {
    let mut q = p.clone();
    if q.herramientas.len() > crate::seleccion::MAXIMO {
        let ultimo = p
            .mensajes
            .iter()
            .rev()
            .find(|m| m.rol == crate::motor::Rol::Usuario);
        let pedido = ultimo
            .map(crate::motor::Mensaje::texto_entero)
            .unwrap_or_default();
        let con_imagen = p.mensajes.iter().any(crate::motor::Mensaje::tiene_imagen);
        let elegidas = crate::seleccion::para_el_pedido(&pedido, con_imagen);
        q.herramientas
            .retain(|h| elegidas.iter().any(|e| e.nombre == h.nombre));
    }
    q
}

/// **La nube con red de seguridad**: si falla antes de decir nada (sin red,
/// clave mala, sin saldo, saturado) y hay modelo local, sigue con el local
/// avisandolo en la burbuja. Tras un fallo, los siguientes minutos van
/// directos al local (no se repite el aviso en cada vuelta del turno).
pub struct MotorConRespaldo {
    pub nube: Arc<dyn Motor>,
    pub local: Option<Arc<dyn Motor>>,
    pub nombre: String,
    caido_hasta: Mutex<Option<Instant>>,
}

/// Cuanto se tira del local tras un fallo de la nube.
pub const PAUSA_TRAS_FALLO: Duration = Duration::from_secs(3 * 60);

impl MotorConRespaldo {
    pub fn nuevo(
        nube: Arc<dyn Motor>,
        local: Option<Arc<dyn Motor>>,
        nombre: impl Into<String>,
    ) -> MotorConRespaldo {
        MotorConRespaldo {
            nube,
            local,
            nombre: nombre.into(),
            caido_hasta: Mutex::new(None),
        }
    }

    fn caido(&self) -> bool {
        self.caido_hasta
            .lock()
            .ok()
            .and_then(|g| *g)
            .is_some_and(|t| Instant::now() < t)
    }
}

impl Motor for MotorConRespaldo {
    fn completar(
        &self,
        p: &Peticion,
        al_trozo: &mut dyn FnMut(&Trozo) -> Seguir,
    ) -> Result<Respuesta, ErrorIa> {
        if let Some(local) = &self.local
            && self.caido()
        {
            return local.completar(&para_el_local(p), al_trozo);
        }
        let mut emitido = false;
        let r = self.nube.completar(p, &mut |t| {
            emitido = true;
            al_trozo(t)
        });
        match (r, &self.local) {
            (Err(ErrorIa::Nube(aviso)), Some(local)) if !emitido => {
                if let Ok(mut g) = self.caido_hasta.lock() {
                    *g = Some(Instant::now() + PAUSA_TRAS_FALLO);
                }
                let nota = format!(
                    "⚠️ {}: {aviso}\nSigo con el asistente local.\n\n",
                    self.nombre
                );
                if al_trozo(&Trozo::Texto(nota)) == Seguir::Parar {
                    return Ok(Respuesta {
                        fin: Fin::Cortado,
                        ..Respuesta::de_texto("")
                    });
                }
                local.completar(&para_el_local(p), al_trozo)
            }
            (r, _) => r,
        }
    }

    fn catalogo_completo(&self) -> bool {
        self.nube.catalogo_completo()
    }

    fn a_la_vez(&self) -> usize {
        self.nube.a_la_vez()
    }

    fn ve_imagenes(&self) -> bool {
        self.nube.ve_imagenes()
    }

    fn soltar(&self) {
        self.nube.soltar();
        if let Some(l) = &self.local {
            l.soltar();
        }
    }
}
