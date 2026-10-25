//! **El asistente**, visto desde Swift.
//!
//! El orden de motores lo decide la app (`Asistente/` en Swift):
//!
//! 1. **Foundation Models** (macOS 26, el modelo del sistema). No pasa por
//!    Rust: Swift declara las herramientas con el JSON de
//!    [`ia_catalogo_para`] y, cuando el modelo llama una, pasa el nombre y
//!    los argumentos a [`ia_llamar`] (validar con las reglas de siempre y,
//!    si dibuja, los elementos de Excalidraw). Una llamada por herramienta,
//!    no una por figura (§4bis del plan).
//! 2. **Un `llama-server` del usuario** escuchando en `127.0.0.1` (por
//!    omision el 8080, [`PUERTO_LLAMA`]): [`llama_local_activo`] lo mira y
//!    [`llama_local_preguntar`] hace un turno entero con el bucle de
//!    `pixpin_ia::agente` y el cliente HTTP de `pixpin_ia::servidor`. La
//!    respuesta llega entera, o **en vivo** con
//!    [`llama_local_preguntar_en_vivo`]: el texto va llegando a Swift por un
//!    [`OyenteIa`] (un interfaz que implementa Swift) en tandas de como
//!    mucho [`CADA_TANDA_MS`] (no una llamada por palabra: §4bis.1), y se
//!    corta con [`CorteIa::cancelar`] (se cierra el enchufe y
//!    `llama-server` deja de generar).
//! 3. Si no hay ninguno, Swift dice que no hay motor.
//!
//! **Con los ajustes `[asistente]`** (D6) el turno va por [`turno::ia_turno`]:
//! la nube (Claude o DeepSeek, por el HTTPS de Swift: [`transporte`]) con el
//! local de respaldo, el modelo bajado con su `llama-server` ([`local`]) o
//! el del usuario; pensar, la hoja de trabajo, el camino rapido y las
//! acciones de la app como en Windows. Las funciones `llama_local_*` de
//! aqui son el turno suelto de antes (sin ajustes), que siguen usando las
//! pruebas y quien no tiene carpeta de datos.
//!
//! Todo es bloqueante y sin hilos propios: Swift lo llama desde una `Task`
//! con prioridad `.utility`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use pixpin_ia::agente::{self, Ejecutor};
use pixpin_ia::herramientas::Llamada;
use pixpin_ia::motor::{Mensaje, Motor, Peticion, Respuesta, Rol, Seguir, Trozo};
use pixpin_ia::{ErrorIa, Evento};
use serde_json::Value;

use crate::ErrorNucleo;

pub mod al_lienzo;
pub mod chat;
pub mod conexiones;
pub mod de_fondo;
pub mod documentos;
pub mod documentos_acciones;
pub mod hoja_a_lienzo;
pub mod letras;
pub mod en_la_conversacion;
pub mod local;
pub mod memoria;
pub mod ojos;
pub mod recetas;
pub mod transporte;
pub mod turno;

/// El puerto de fabrica de `llama-server` (`--port 8080`).
pub const PUERTO_LLAMA: u16 = 8080;

/// Un mensaje de la conversacion que guarda Swift, para la vuelta
/// siguiente. Solo texto: las llamadas a herramientas de turnos pasados no
/// se vuelven a mandar (un modelo pequeno se lia con ellas).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MensajeIa {
    /// `true` si lo escribio el usuario; `false`, el asistente.
    pub del_usuario: bool,
    pub texto: String,
}

/// Una herramienta que se uso en el turno.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HerramientaUsada {
    pub nombre: String,
    pub ok: bool,
    /// Lo que se le devolvio al modelo («dibujadas 4 figuras», el motivo).
    pub resumen: String,
}

/// Lo que contesto el motor local en un turno.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RespuestaIa {
    pub texto: String,
    pub herramientas: Vec<HerramientaUsada>,
    /// Lo dibujado en el turno, como un `.excalidraw` entero (el formato
    /// que lee el lienzo), o nada si no se dibujo.
    pub lienzo: Option<String>,
    /// Lo que penso antes de contestar (vacio si no penso).
    pub pensamiento: String,
    /// Cuanto penso («Pensó 12 s»), si penso.
    pub segundos_pensando: Option<f32>,
    /// Quien contesto («Claude Haiku 5.5», «Qwen3.5 2B (recomendado)»).
    pub motor: String,
    /// La hoja de trabajo de la conversacion, reescrita (JSON), para el
    /// turno siguiente. Vacia si el turno no la lleva.
    pub hoja: String,
    /// Lo citado de los documentos del usuario (`buscar_en_mis_cosas`): las
    /// tarjetas que abren el documento.
    pub citas: Vec<CitaIa>,
}

/// Una cita de los documentos del usuario.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CitaIa {
    /// El `[n]` de la respuesta.
    pub numero: u32,
    /// «Tesis.pdf · pág. 12».
    pub titulo: String,
    /// El pedido `abrir` del protocolo que lo abre (JSON).
    pub pedido: String,
}

impl RespuestaIa {
    /// Una respuesta sin pensar ni hoja (los turnos sueltos).
    pub(crate) fn simple(texto: String, herramientas: Vec<HerramientaUsada>, lienzo: Option<String>, motor: &str) -> RespuestaIa {
        RespuestaIa {
            texto,
            herramientas,
            lienzo,
            pensamiento: String::new(),
            segundos_pensando: None,
            motor: motor.into(),
            hoja: String::new(),
            citas: Vec::new(),
        }
    }
}

/// Cada cuanto, como mucho, cruza el texto nuevo a Swift mientras llega.
/// Unas 16 tandas por segundo: se ve fluido y el puente casi no se nota.
pub const CADA_TANDA_MS: u64 = 60;

/// **Quien escucha la respuesta en vivo**: lo implementa Swift. Se le
/// llama desde el hilo del turno (no el principal): Swift pasa lo recibido
/// al hilo de la interfaz.
#[uniffi::export(foreign)]
pub trait OyenteIa: Send + Sync {
    /// Texto nuevo de la respuesta (una tanda; se pega detras de lo de antes).
    fn texto(&self, trozo: String);
    /// Se uso una herramienta (con su resultado).
    fn herramienta(&self, usada: HerramientaUsada);
    /// Un trozo de lo que piensa (la seccion plegable «Pensando…»).
    fn pensando(&self, trozo: String);
    /// El modelo empezo a pedir una herramienta («dibujando…»).
    fn llamando(&self, nombre: String);
}

/// **Para cortar un turno en vivo** desde otro hilo (el boton «Parar»).
#[derive(Debug, Default, uniffi::Object)]
pub struct CorteIa {
    cortado: AtomicBool,
}

#[uniffi::export]
impl CorteIa {
    #[uniffi::constructor]
    pub fn nuevo() -> Arc<CorteIa> {
        Arc::new(CorteIa::default())
    }

    /// Pide parar: el turno acaba en el siguiente trozo que llegue, con lo
    /// que hubiera hasta ahi.
    pub fn cancelar(&self) {
        self.cortado.store(true, Ordering::Relaxed);
    }

    pub fn cortado(&self) -> bool {
        self.cortado.load(Ordering::Relaxed)
    }
}

/// Junta el texto que llega y lo suelta en tandas.
pub(crate) struct Tanda {
    pendiente: String,
    ultima: Instant,
    cada: Duration,
}

impl Tanda {
    pub(crate) fn nueva(cada: Duration) -> Tanda {
        Tanda {
            pendiente: String::new(),
            ultima: Instant::now(),
            cada,
        }
    }

    /// Mete texto; devuelve la tanda si ya toca soltarla.
    pub(crate) fn meter(&mut self, texto: &str) -> Option<String> {
        self.pendiente.push_str(texto);
        if self.pendiente.is_empty() || self.ultima.elapsed() < self.cada {
            return None;
        }
        self.soltar()
    }

    /// Lo que quede, toque o no.
    pub(crate) fn soltar(&mut self) -> Option<String> {
        self.ultima = Instant::now();
        (!self.pendiente.is_empty()).then(|| std::mem::take(&mut self.pendiente))
    }
}

/// Las instrucciones de sistema del asistente (las mismas que en Windows).
#[uniffi::export]
pub fn ia_instrucciones() -> String {
    agente::instrucciones()
}

/// Todas las herramientas, como la lista `tools` de OpenAI (JSON).
#[uniffi::export]
pub fn ia_catalogo() -> String {
    pixpin_ia::puente::catalogo()
}

/// Las pocas herramientas que vienen a cuento para `pedido` (JSON, lista
/// `tools` de OpenAI): un modelo pequeno se pierde con el catalogo entero.
#[uniffi::export]
pub fn ia_catalogo_para(pedido: String, con_imagen: bool) -> String {
    pixpin_ia::puente::catalogo_para(&pedido, con_imagen)
}

/// Una llamada del modelo, de cualquier motor (ver
/// `pixpin_ia::puente::llamar`): devuelve el objeto JSON con `ok`,
/// `elementos`, `argumentos`... o `para_el_modelo` si estaba mal.
#[uniffi::export]
pub fn ia_llamar(
    nombre: String,
    argumentos: String,
    origen_x: f64,
    origen_y: f64,
    semilla: u64,
) -> String {
    pixpin_ia::puente::llamar(&nombre, &argumentos, (origen_x, origen_y), semilla)
}

/// Un `.excalidraw` entero con los `elementos` (texto JSON de una lista,
/// los que van juntando las llamadas de [`ia_llamar`]). Lo que no es una
/// lista da un lienzo vacio.
#[uniffi::export]
pub fn ia_lienzo_de(elementos: String) -> String {
    let lista = match serde_json::from_str::<Value>(&elementos) {
        Ok(Value::Array(v)) => v,
        _ => Vec::new(),
    };
    pixpin_ia::lienzo::documento(lista)
}

/// Si hay un `llama-server` con el modelo cargado en `127.0.0.1:puerto`
/// (`GET /health` da 200). Tarda como mucho unos 300 ms si no hay nadie.
#[uniffi::export]
pub fn llama_local_activo(puerto: u16) -> bool {
    pixpin_ia::servidor::salud(puerto)
}

/// **Un turno con el `llama-server` del usuario**: `historial` es lo dicho
/// antes (del mas antiguo al mas nuevo) y `pedido` lo nuevo. Usa las
/// herramientas que vienen a cuento; las que dibujan se juntan en
/// [`RespuestaIa::lienzo`] y las que la app del Mac aun no sabe hacer
/// vuelven al modelo como fallo, para que lo diga.
#[uniffi::export]
pub fn llama_local_preguntar(
    puerto: u16,
    historial: Vec<MensajeIa>,
    pedido: String,
) -> Result<RespuestaIa, ErrorNucleo> {
    let pedido = pedido.trim().to_string();
    if pedido.is_empty() {
        return Err(ErrorNucleo::Argumento {
            detalle: "el pedido esta vacio".into(),
        });
    }
    let motor = LlamaDelUsuario { puerto };
    turno_con_motor(&motor, historial, &pedido)
}

/// **Un turno en vivo con el `llama-server` del usuario**: como
/// [`llama_local_preguntar`], pero el texto va llegando a `oyente` mientras
/// el modelo escribe (en tandas) y `corte` lo para. Devuelve la respuesta
/// entera al final (cortada, si se paro: con lo que hubiera).
#[uniffi::export]
pub fn llama_local_preguntar_en_vivo(
    puerto: u16,
    historial: Vec<MensajeIa>,
    pedido: String,
    oyente: Arc<dyn OyenteIa>,
    corte: Arc<CorteIa>,
) -> Result<RespuestaIa, ErrorNucleo> {
    let pedido = pedido.trim().to_string();
    if pedido.is_empty() {
        return Err(ErrorNucleo::Argumento {
            detalle: "el pedido esta vacio".into(),
        });
    }
    let motor = LlamaDelUsuario { puerto };
    turno_en_vivo(
        &motor,
        historial,
        &pedido,
        oyente.as_ref(),
        &corte,
        Duration::from_millis(CADA_TANDA_MS),
    )
}

/// El turno en vivo con cualquier motor (las pruebas usan uno guionado).
pub(crate) fn turno_en_vivo(
    motor: &dyn Motor,
    historial: Vec<MensajeIa>,
    pedido: &str,
    oyente: &dyn OyenteIa,
    corte: &CorteIa,
    cada: Duration,
) -> Result<RespuestaIa, ErrorNucleo> {
    if corte.cortado() {
        return Ok(RespuestaIa::simple(String::new(), Vec::new(), None, ""));
    }
    let mut tanda = Tanda::nueva(cada);
    let r = turno_con_eventos(motor, historial, pedido, &mut |e| {
        match e {
            Evento::Texto(t) => {
                if let Some(t) = tanda.meter(t) {
                    oyente.texto(t);
                }
            }
            Evento::Resultado { nombre, ok, texto } => {
                // Lo escrito antes de la herramienta va primero.
                if let Some(t) = tanda.soltar() {
                    oyente.texto(t);
                }
                oyente.herramienta(HerramientaUsada {
                    nombre: nombre.clone(),
                    ok: *ok,
                    resumen: texto.clone(),
                });
            }
            _ => {}
        }
        if corte.cortado() {
            Seguir::Parar
        } else {
            Seguir::Si
        }
    });
    if let Some(t) = tanda.soltar() {
        oyente.texto(t);
    }
    r
}

/// El turno con cualquier motor (las pruebas usan uno guionado).
pub(crate) fn turno_con_motor(
    motor: &dyn Motor,
    historial: Vec<MensajeIa>,
    pedido: &str,
) -> Result<RespuestaIa, ErrorNucleo> {
    turno_con_eventos(motor, historial, pedido, &mut |_| Seguir::Si)
}

/// El turno, avisando de cada evento a `al_evento` (que puede parar).
fn turno_con_eventos(
    motor: &dyn Motor,
    historial: Vec<MensajeIa>,
    pedido: &str,
    al_evento_fuera: &mut dyn FnMut(&Evento) -> Seguir,
) -> Result<RespuestaIa, ErrorNucleo> {
    let mut mensajes = Vec::with_capacity(historial.len() + 2);
    mensajes.push(Mensaje::sistema(agente::instrucciones()));
    for m in historial {
        if m.texto.trim().is_empty() {
            continue;
        }
        let rol = if m.del_usuario {
            Rol::Usuario
        } else {
            Rol::Asistente
        };
        mensajes.push(Mensaje::texto(rol, m.texto));
    }
    mensajes.push(Mensaje::usuario(pedido));
    let herramientas = pixpin_ia::seleccion::para_el_pedido(pedido, false);
    let mut ejecutor = EjecutorMac::default();
    let mut usadas: Vec<HerramientaUsada> = Vec::new();
    let mut al_evento = |e: Evento| {
        let seguir = al_evento_fuera(&e);
        if let Evento::Resultado { nombre, ok, texto } = e {
            usadas.push(HerramientaUsada {
                nombre,
                ok,
                resumen: texto,
            });
        }
        seguir
    };
    let fin = agente::turno(
        motor,
        &mut mensajes,
        &herramientas,
        &mut ejecutor,
        &mut al_evento,
    )
    .map_err(|e| ErrorNucleo::Asistente {
        detalle: agente::para_el_usuario(&e),
    })?;
    let lienzo = (!ejecutor.elementos.is_empty())
        .then(|| pixpin_ia::lienzo::documento(std::mem::take(&mut ejecutor.elementos)));
    Ok(RespuestaIa::simple(fin.texto, usadas, lienzo, ""))
}

/// El `llama-server` que arranco el usuario (no PixPin): no se lanza ni se
/// mata, solo se le habla.
pub(crate) struct LlamaDelUsuario {
    pub(crate) puerto: u16,
}

impl Motor for LlamaDelUsuario {
    fn completar(
        &self,
        peticion: &Peticion,
        al_trozo: &mut dyn FnMut(&Trozo) -> Seguir,
    ) -> Result<Respuesta, ErrorIa> {
        // Sin `--api-key` el servidor no mira la clave.
        pixpin_ia::servidor::pedir(self.puerto, "pixpin", peticion, al_trozo)
    }
}

/// Lo que el Mac ya sabe hacer de verdad: dibujar (juntando los elementos
/// para el lienzo) y contestar sobre la app. Lo demas vuelve al modelo como
/// fallo.
#[derive(Default)]
struct EjecutorMac {
    elementos: Vec<Value>,
    semilla: u64,
}

impl Ejecutor for EjecutorMac {
    fn ejecutar(&mut self, llamada: &Llamada) -> Result<String, String> {
        if llamada.dibuja() {
            self.semilla += 1;
            // Cada dibujo, debajo del anterior, para que no se pisen.
            let y = self
                .elementos
                .iter()
                .filter_map(|e| Some(e["y"].as_f64()? + e["height"].as_f64().unwrap_or(0.0)))
                .fold(0.0_f64, f64::max);
            let origen = (
                0.0,
                if self.elementos.is_empty() {
                    0.0
                } else {
                    y + 40.0
                },
            );
            let nuevos = pixpin_ia::lienzo::elementos(llamada, origen, self.semilla);
            let n = nuevos.len();
            self.elementos.extend(nuevos);
            return Ok(format!("dibujadas {n} figuras"));
        }
        match llamada {
            Llamada::Ayuda(pregunta) => Ok(pixpin_ia::conocimiento::responder(pregunta)),
            otra => Err(format!(
                "{} aun no se puede hacer en PixPin para Mac",
                otra.nombre()
            )),
        }
    }
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;
    use pixpin_ia::herramientas;
    use pixpin_ia::motor::MotorGuionado;

    #[test]
    fn el_catalogo_y_la_llamada_cruzan_como_texto_json() {
        let v: Value =
            serde_json::from_str(&ia_catalogo_para("dibuja un circulo".into(), false)).unwrap();
        assert!(!v.as_array().unwrap().is_empty());
        let r: Value = serde_json::from_str(&ia_llamar(
            herramientas::DIBUJAR.into(),
            r#"{"elementos":[{"tipo":"elipse","x":0,"y":0,"ancho":50,"alto":50,"color":"azul"}]}"#
                .into(),
            0.0,
            0.0,
            3,
        ))
        .unwrap();
        assert_eq!(r["ok"], true, "{r}");
        assert!(!ia_instrucciones().is_empty());
        assert!(
            serde_json::from_str::<Value>(&ia_catalogo())
                .unwrap()
                .is_array()
        );
        let doc = ia_lienzo_de(r["elementos"].to_string());
        assert!(pixpin_motor2d::excalidraw::leer(&doc).is_ok());
        assert!(pixpin_motor2d::excalidraw::leer(&ia_lienzo_de("roto".into())).is_ok());
    }

    #[test]
    fn sin_servidor_no_hay_motor_local() {
        // Un puerto libre que nadie escucha.
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = l.local_addr().unwrap().port();
        drop(l);
        assert!(!llama_local_activo(puerto));
        let e = llama_local_preguntar(puerto, Vec::new(), "hola".into()).unwrap_err();
        assert!(matches!(e, ErrorNucleo::Asistente { .. }), "{e:?}");
        assert!(matches!(
            llama_local_preguntar(puerto, Vec::new(), "  ".into()),
            Err(ErrorNucleo::Argumento { .. })
        ));
    }

    #[test]
    fn un_turno_que_dibuja_devuelve_el_lienzo_y_el_texto() {
        let motor = MotorGuionado::nuevo(vec![
            Respuesta::de_llamada(
                herramientas::DIBUJAR,
                r#"{"elementos":[{"tipo":"rectangulo","x":0,"y":0,"ancho":80,"alto":40,"color":"rojo"}]}"#,
            ),
            Respuesta::de_texto("Listo, dibuje un rectangulo rojo."),
        ]);
        let historial = vec![
            MensajeIa {
                del_usuario: true,
                texto: "hola".into(),
            },
            MensajeIa {
                del_usuario: false,
                texto: "Hola, ¿que dibujo?".into(),
            },
        ];
        let r = turno_con_motor(&motor, historial, "dibuja un rectangulo rojo").unwrap();
        assert_eq!(r.texto, "Listo, dibuje un rectangulo rojo.");
        assert_eq!(r.herramientas.len(), 1);
        assert!(r.herramientas[0].ok);
        let doc = r.lienzo.expect("lo dibujado");
        assert!(pixpin_motor2d::excalidraw::leer(&doc).is_ok());
        // El historial llego al motor: sistema + 2 + el pedido.
        let recibido = motor.recibido.lock().unwrap();
        assert!(recibido[0].mensajes.len() >= 4);
    }

    /// Lo que oye Swift, apuntado.
    #[derive(Default)]
    pub(crate) struct Apunta {
        pub(crate) tandas: std::sync::Mutex<Vec<String>>,
        pub(crate) herramientas: std::sync::Mutex<Vec<HerramientaUsada>>,
        pub(crate) pensado: std::sync::Mutex<String>,
        pub(crate) llamadas: std::sync::Mutex<Vec<String>>,
        /// Si se pone, corta al llegar la primera tanda.
        cortar: Option<Arc<CorteIa>>,
    }

    impl OyenteIa for Apunta {
        fn texto(&self, trozo: String) {
            self.tandas.lock().unwrap().push(trozo);
            if let Some(c) = &self.cortar {
                c.cancelar();
            }
        }
        fn herramienta(&self, usada: HerramientaUsada) {
            self.herramientas.lock().unwrap().push(usada);
        }
        fn pensando(&self, trozo: String) {
            self.pensado.lock().unwrap().push_str(&trozo);
        }
        fn llamando(&self, nombre: String) {
            self.llamadas.lock().unwrap().push(nombre);
        }
    }

    #[test]
    fn en_vivo_el_texto_llega_por_tandas_y_entero() {
        let motor = MotorGuionado::nuevo(vec![
            Respuesta::de_llamada(
                herramientas::DIBUJAR,
                r#"{"elementos":[{"tipo":"elipse","x":0,"y":0,"ancho":50,"alto":50,"color":"azul"}]}"#,
            ),
            Respuesta::de_texto("Listo, ahi tienes un circulo azul."),
        ]);
        let oyente = Apunta::default();
        let corte = CorteIa::nuevo();
        // Sin espera entre tandas: cada trozo cruza en cuanto llega.
        let r = turno_en_vivo(
            &motor,
            Vec::new(),
            "dibuja un circulo azul",
            &oyente,
            &corte,
            Duration::ZERO,
        )
        .unwrap();
        let tandas = oyente.tandas.lock().unwrap();
        assert!(tandas.len() > 1, "llego en varias tandas: {tandas:?}");
        assert_eq!(
            tandas.concat(),
            r.texto,
            "lo de las tandas es la respuesta entera"
        );
        let usadas = oyente.herramientas.lock().unwrap();
        assert_eq!(usadas.len(), 1);
        assert!(usadas[0].ok);
        assert!(r.lienzo.is_some());
    }

    #[test]
    fn en_vivo_con_espera_larga_se_junta_en_una_tanda() {
        let motor = MotorGuionado::nuevo(vec![Respuesta::de_texto("uno dos tres cuatro cinco")]);
        let oyente = Apunta::default();
        let r = turno_en_vivo(
            &motor,
            Vec::new(),
            "cuenta",
            &oyente,
            &CorteIa::nuevo(),
            Duration::from_secs(3600),
        )
        .unwrap();
        // Caso negativo: no se cruza el puente por cada palabra.
        assert_eq!(*oyente.tandas.lock().unwrap(), vec![r.texto.clone()]);
    }

    #[test]
    fn en_vivo_se_corta_y_devuelve_lo_que_habia() {
        let motor = MotorGuionado::nuevo(vec![Respuesta::de_texto(
            "una respuesta muy larga que nadie quiere leer entera",
        )]);
        let corte = CorteIa::nuevo();
        let oyente = Apunta {
            cortar: Some(corte.clone()),
            ..Default::default()
        };
        let r =
            turno_en_vivo(&motor, Vec::new(), "habla", &oyente, &corte, Duration::ZERO).unwrap();
        assert!(corte.cortado());
        let oido = oyente.tandas.lock().unwrap().concat();
        assert!(
            oido.len() < "una respuesta muy larga que nadie quiere leer entera".len(),
            "se paro antes del final: {oido:?}"
        );
        assert!(!r.texto.contains("entera"), "{:?}", r.texto);
        // Ya cortado antes de empezar: ni se llama al motor.
        let otro = MotorGuionado::nuevo(vec![Respuesta::de_texto("hola")]);
        let r = turno_en_vivo(
            &otro,
            Vec::new(),
            "hola",
            &Apunta::default(),
            &corte,
            Duration::ZERO,
        )
        .unwrap();
        assert!(r.texto.is_empty());
        assert!(otro.recibido.lock().unwrap().is_empty());
    }

    #[test]
    fn en_vivo_con_un_llama_server_falso_que_habla_por_sse() {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = l.local_addr().unwrap().port();
        let servidor = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            // Leer la peticion entera (cabecera y cuerpo): cerrar con bytes
            // sin leer haria que el sistema cortara la conexion de golpe.
            let mut buf = [0u8; 65536];
            let mut leido = Vec::new();
            loop {
                if let Some(fin) = leido.windows(4).position(|w| w == b"\r\n\r\n") {
                    let cabecera = String::from_utf8_lossy(&leido[..fin]).to_lowercase();
                    let largo: usize = cabecera
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0);
                    if leido.len() >= fin + 4 + largo {
                        break;
                    }
                }
                let n = s.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                leido.extend_from_slice(&buf[..n]);
            }
            let trozo = |t: &str| {
                format!(
                    "data: {{\"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"{t}\"}}}}]}}\n\n"
                )
            };
            let mut cuerpo = String::new();
            cuerpo.push_str(&trozo("Hola"));
            cuerpo.push_str(&trozo(", que tal"));
            cuerpo.push_str(
                "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            );
            cuerpo.push_str("data: [DONE]\n\n");
            let respuesta = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
                cuerpo.len()
            );
            s.write_all(respuesta.as_bytes()).unwrap();
        });
        let oyente = Arc::new(Apunta::default());
        let r = llama_local_preguntar_en_vivo(
            puerto,
            Vec::new(),
            "saluda".into(),
            oyente.clone(),
            CorteIa::nuevo(),
        )
        .unwrap();
        servidor.join().unwrap();
        assert_eq!(r.texto, "Hola, que tal");
        assert_eq!(oyente.tandas.lock().unwrap().concat(), "Hola, que tal");
    }

    #[test]
    fn lo_que_el_mac_no_sabe_hacer_vuelve_al_modelo_como_fallo() {
        let motor = MotorGuionado::nuevo(vec![
            Respuesta::de_llamada(herramientas::OCR, r#"{"idioma":"es"}"#),
            Respuesta::de_texto("No puedo leer ese PDF aqui todavia."),
        ]);
        let r = turno_con_motor(&motor, Vec::new(), "pasa por ocr el pdf escaneado").unwrap();
        assert!(r.lienzo.is_none());
        assert!(r.herramientas.iter().all(|h| !h.ok), "{:?}", r.herramientas);
        assert!(!r.texto.is_empty());
    }
}
