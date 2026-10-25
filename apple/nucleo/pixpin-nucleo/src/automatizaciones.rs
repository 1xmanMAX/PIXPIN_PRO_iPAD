//! **Las automatizaciones del asistente** (tanda E6): lo mismo que
//! `apps/pixpin/src/automatizaciones.rs` y `automatizaciones/motor.rs` de
//! Windows, con el mismo fichero, para que viajen entre los aparatos propios
//! y se entiendan igual en los tres sistemas.
//!
//! Una automatizacion es un disparador ([`Disparador`]: al capturar, a una
//! hora, al llegar algo por la sincro, a mano) y una lista de pasos
//! ([`Paso`]): una accion de la tabla `pixpin_ia::acciones` (el mismo pedido
//! que armaria el asistente) o un pedido al asistente. Los pasos se encadenan
//! con variables (`{captura}`, `{resultado}`, `{fecha}`, `{hora}`, `{sincro}`,
//! `{nombre}`): lo que contesto un paso es el `{resultado}` del siguiente.
//!
//! - **Donde se guardan**: JSON en camelCase en la carpeta del chat
//!   «Asistente», `android/guardados/asistente/automatizaciones.json`, que
//!   viaja como `pixpin:files/guardados/asistente/automatizaciones.json`
//!   porque lo senala un mensaje ARCHIVO fijo del chat ([`ID_MENSAJE`],
//!   `auto-automatizaciones`): solo va a los aparatos propios, como un
//!   `.leccion`, sin tocar el protocolo.
//! - **Donde corren**: cada una lleva el `aparato` que la creo; las de otro
//!   aparato se listan pero no corren aqui (si no, «cada dia a las 8»
//!   saldria dos veces). Sin `aparato`, en todos.
//! - **Lo que ya corrio** (la ultima vez de cada una) es de este Mac y no
//!   viaja: `<raiz>/asistente/automatizaciones-estado.json`, como Windows.
//!   El historial de vueltas (solo del Mac, tampoco viaja) va al lado,
//!   `automatizaciones-historial.json`.
//! - **Nunca algo destructivo sin preguntar**: cada accion pasa por
//!   `pixpin_ia::tareas::hacer`, que pregunta antes de lo que borra o sale
//!   del Mac (Swift: [`ManosAutomatizacion::confirmar`]).
//! - **Sin bucles**: una que salta al capturar no puede capturar, ni una que
//!   salta con la sincro sincronizar, ni ninguna crear automatizaciones (se
//!   rechaza al crearla y se bloquea al correr; dentro de un paso del
//!   asistente lo bloquea `HacerPedidos` con [`automatizacion_bloqueada`]); y
//!   aun asi, cada una tiene un tope de veces por hora ([`Guardia`]).
//!
//! Lo que en Windows es el hilo del motor (`motor::bucle`) aqui lo lleva
//! Swift (`Automatizaciones/MotorAutomatizaciones.swift`): mira la hora cada
//! 30 s, vigila la carpeta de capturas y la sincronizacion, y llama a
//! [`Nucleo::automatizaciones_disparar`] fuera del hilo principal, una vuelta
//! a la vez. Lo puro ([`elegir`], [`toca_a_la_hora`], [`ejecutar`]) es de
//! aqui y se prueba con un reloj de mentira.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pixpin_ia::acciones::{self, AccionApp};
use pixpin_ia::fechas::{self, Momento};
use pixpin_ia::tareas::{Chat, FuenteTareas, VistaLista, VistaTarea};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::{ErrorNucleo, Nucleo};

/// La version del fichero.
pub const VERSION: u32 = 1;
/// El id del mensaje del chat «Asistente» que senala el fichero.
pub const ID_MENSAJE: &str = "auto-automatizaciones";
/// La ruta con que viaja (y la del mensaje).
pub const RUTA_PORTATIL: &str = "pixpin:files/guardados/asistente/automatizaciones.json";
/// Dentro de la carpeta del chat.
pub const RUTA_EN_EL_CHAT: &str = "android/guardados/asistente/automatizaciones.json";
/// Lo que dice el mensaje que la hace viajar (el de Windows).
const TEXTO_DEL_MENSAJE: &str = "⚙️ Las automatizaciones del asistente (se cambian en Ajustes → Asistente)";
/// Cuantos pasos como mucho.
pub const MAX_PASOS: usize = 10;

/// Cuanto despues de su hora se recupera una que no corrio (el Mac estaba
/// apagado a las 8 y se enciende a las 9: corre; a las 20, ya no).
pub const RECUPERAR_HASTA_MS: i64 = 6 * 3_600_000;
/// Veces por hora como mucho, cada una.
pub const POR_HORA: usize = 30;
/// Lo minimo entre dos vueltas de la misma.
pub const HUECO_MINIMO_MS: i64 = 3_000;
/// Cuantas vueltas guarda el historial del Mac.
pub const HISTORIAL: usize = 50;

/// **Cuando salta.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Disparador {
    /// Al guardarse una captura nueva en la carpeta de capturas
    /// (`{captura}` es su ruta).
    AlCapturar,
    /// A una hora local (`"08:00"`), los `dias` de la semana (1 lunes … 7
    /// domingo; vacio: todos).
    ALaHora {
        hora: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        dias: Vec<u8>,
    },
    /// Al acabar una vuelta de la sincronizacion que trajo algo (`{sincro}`
    /// es lo que dijo la vuelta).
    AlSincronizar,
    /// Solo cuando se pide (la ventana, el pedido `automatizacion`).
    AMano,
}

/// El tipo del disparador, sin sus datos (para la guardia de bucles).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tipo {
    AlCapturar,
    ALaHora,
    AlSincronizar,
    AMano,
}

impl Tipo {
    /// Como va en el JSON (`al_capturar`…): lo que se le dice a Swift.
    pub fn nombre(self) -> &'static str {
        match self {
            Tipo::AlCapturar => "al_capturar",
            Tipo::ALaHora => "a_la_hora",
            Tipo::AlSincronizar => "al_sincronizar",
            Tipo::AMano => "a_mano",
        }
    }

    pub fn de_nombre(s: &str) -> Option<Tipo> {
        Some(match s {
            "al_capturar" => Tipo::AlCapturar,
            "a_la_hora" => Tipo::ALaHora,
            "al_sincronizar" => Tipo::AlSincronizar,
            "a_mano" => Tipo::AMano,
            _ => return None,
        })
    }
}

impl Disparador {
    pub fn tipo(&self) -> Tipo {
        match self {
            Disparador::AlCapturar => Tipo::AlCapturar,
            Disparador::ALaHora { .. } => Tipo::ALaHora,
            Disparador::AlSincronizar => Tipo::AlSincronizar,
            Disparador::AMano => Tipo::AMano,
        }
    }

    /// En palabras, para la lista.
    pub fn en_palabras(&self) -> String {
        match self {
            Disparador::AlCapturar => "Al capturar".into(),
            Disparador::ALaHora { hora, dias } if dias.is_empty() => format!("Cada día a las {hora}"),
            Disparador::ALaHora { hora, dias } => {
                const NOMBRES: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];
                let d: Vec<&str> = dias
                    .iter()
                    .filter_map(|d| NOMBRES.get(usize::from(*d).wrapping_sub(1)).copied())
                    .collect();
                format!("{} a las {hora}", d.join(", "))
            }
            Disparador::AlSincronizar => "Al llegar algo por la sincronización".into(),
            Disparador::AMano => "A mano".into(),
        }
    }
}

/// **Un paso.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Paso {
    /// Una accion de la tabla del asistente (`pixpin_ia::acciones`), por el
    /// nombre de su herramienta, con sus argumentos. `proyecto` (nombre o id
    /// del chat) e `imagenes` van al pedido si la accion los entiende.
    Accion {
        herramienta: String,
        #[serde(default, skip_serializing_if = "Map::is_empty")]
        args: Map<String, Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proyecto: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        imagenes: Vec<String>,
    },
    /// Un pedido al asistente, como escrito en su chat. `imagenes` van
    /// adjuntas (al capturar, sin decir nada, la captura).
    Asistente {
        pedido: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proyecto: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        imagenes: Vec<String>,
    },
}

/// **Una automatizacion.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Automatizacion {
    pub id: String,
    pub nombre: String,
    #[serde(default = "si")]
    pub activa: bool,
    pub disparador: Disparador,
    pub pasos: Vec<Paso>,
    /// El codigo del aparato donde corre (el que la creo). `None`: en todos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aparato: Option<String>,
    #[serde(default)]
    pub creada: i64,
    #[serde(default)]
    pub tocada: i64,
    /// Lo que no se entiende (lo que anada el movil o Windows), tal cual.
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

fn si() -> bool {
    true
}

impl Automatizacion {
    /// Si corre en este aparato.
    pub fn corre_aqui(&self, aparato: &str) -> bool {
        self.aparato.as_deref().is_none_or(|a| a.is_empty() || a == aparato)
    }

    /// Los pasos en una linea, para la lista.
    pub fn pasos_en_palabras(&self) -> String {
        self.pasos
            .iter()
            .map(|p| match p {
                Paso::Accion { herramienta, proyecto, .. } => match proyecto {
                    Some(pr) => format!("{herramienta} en «{pr}»"),
                    None => herramienta.clone(),
                },
                Paso::Asistente { pedido, .. } => format!("«{}»", recortar(pedido, 40)),
            })
            .collect::<Vec<_>>()
            .join(" → ")
    }
}

/// Lo que hay en el fichero, en su orden: las que se entienden y las que no
/// (de una version mas nueva, o rotas), que se guardan tal cual.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lista {
    pub entradas: Vec<Entrada>,
    /// Lo de mas del fichero, tal cual.
    pub resto: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entrada {
    Buena(Automatizacion),
    Rota { valor: Value, motivo: String },
}

impl Lista {
    /// Lee el JSON. Un fichero que no es JSON es un error (no se pisa); una
    /// automatizacion que no se entiende se queda como [`Entrada::Rota`].
    pub fn de_json(texto: &str) -> Result<Lista, String> {
        let v: Value = serde_json::from_str(texto).map_err(|e| format!("no es JSON: {e}"))?;
        let Value::Object(mut o) = v else {
            return Err("no es un objeto JSON".into());
        };
        let items = match o.remove("automatizaciones") {
            Some(Value::Array(a)) => a,
            None => Vec::new(),
            Some(_) => return Err("«automatizaciones» no es una lista".into()),
        };
        o.remove("version");
        let entradas = items
            .into_iter()
            .map(|valor| match serde_json::from_value::<Automatizacion>(valor.clone()) {
                Ok(a) => match validar(&a) {
                    Ok(()) => Entrada::Buena(a),
                    Err(motivo) => Entrada::Rota { valor, motivo },
                },
                Err(e) => Entrada::Rota {
                    valor,
                    motivo: motivo_legible(&e.to_string()),
                },
            })
            .collect();
        Ok(Lista { entradas, resto: o })
    }

    pub fn a_json(&self) -> String {
        let mut o = self.resto.clone();
        o.insert("version".into(), json!(VERSION));
        let items: Vec<Value> = self
            .entradas
            .iter()
            .map(|e| match e {
                Entrada::Buena(a) => serde_json::to_value(a).unwrap_or(Value::Null),
                Entrada::Rota { valor, .. } => valor.clone(),
            })
            .collect();
        o.insert("automatizaciones".into(), Value::Array(items));
        serde_json::to_string_pretty(&Value::Object(o)).unwrap_or_default()
    }

    pub fn buenas(&self) -> impl Iterator<Item = &Automatizacion> {
        self.entradas.iter().filter_map(|e| match e {
            Entrada::Buena(a) => Some(a),
            Entrada::Rota { .. } => None,
        })
    }

    pub fn buscar(&self, id: &str) -> Option<&Automatizacion> {
        self.buenas().find(|a| a.id == id)
    }

    fn buscar_mut(&mut self, id: &str) -> Option<&mut Automatizacion> {
        self.entradas.iter_mut().find_map(|e| match e {
            Entrada::Buena(a) if a.id == id => Some(a),
            _ => None,
        })
    }

    /// Enciende o apaga. `false` si no esta.
    pub fn activar(&mut self, id: &str, activa: bool, ahora: i64) -> bool {
        match self.buscar_mut(id) {
            Some(a) => {
                a.activa = activa;
                a.tocada = ahora;
                true
            }
            None => false,
        }
    }

    /// Quita una (buena o rota, por su id). `false` si no estaba.
    pub fn borrar(&mut self, id: &str) -> bool {
        let antes = self.entradas.len();
        self.entradas.retain(|e| match e {
            Entrada::Buena(a) => a.id != id,
            Entrada::Rota { valor, .. } => valor.get("id").and_then(Value::as_str) != Some(id),
        });
        self.entradas.len() != antes
    }

    /// Anade una nueva, ya validada.
    pub fn anadir(&mut self, a: Automatizacion) -> Result<(), String> {
        validar(&a)?;
        if self.buscar(&a.id).is_some() {
            return Err(format!("ya hay una automatización «{}»", a.id));
        }
        self.entradas.push(Entrada::Buena(a));
        Ok(())
    }

    /// **Cambia una que ya estaba** (el editor del Mac), en su sitio,
    /// conservando lo que no se toca: `resto`, `creada` y `aparato`. Si no
    /// estaba, se anade al final.
    pub fn poner(&mut self, mut a: Automatizacion, ahora: i64) -> Result<(), String> {
        validar(&a)?;
        a.tocada = ahora;
        match self.buscar_mut(&a.id) {
            Some(vieja) => {
                a.resto = std::mem::take(&mut vieja.resto);
                a.creada = vieja.creada;
                a.aparato = vieja.aparato.take();
                *vieja = a;
                Ok(())
            }
            None => {
                if a.creada == 0 {
                    a.creada = ahora;
                }
                self.entradas.push(Entrada::Buena(a));
                Ok(())
            }
        }
    }
}

/// Lo que dice serde, en palabras: «unknown variant `x`» es algo de una
/// version mas nueva (el movil, mas adelante).
fn motivo_legible(e: &str) -> String {
    if let Some(resto) = e.split("unknown variant `").nth(1)
        && let Some(x) = resto.split('`').next()
    {
        return format!("«{x}» es de una versión más nueva");
    }
    if let Some(resto) = e.split("missing field `").nth(1)
        && let Some(x) = resto.split('`').next()
    {
        return format!("le falta «{x}»");
    }
    e.to_string()
}

// ---------------------------------------------------------------------------
// Validar

/// Las acciones que harian saltar a cada disparador (un bucle).
fn salta_con(accion: &str) -> Option<Tipo> {
    match accion {
        "capturar" => Some(Tipo::AlCapturar),
        "sincronizar" => Some(Tipo::AlSincronizar),
        _ => None,
    }
}

/// **Si un pedido no se puede hacer desde una automatizacion de este
/// disparador**: el motivo. Vale para los pasos y para lo que pida el
/// asistente dentro de un paso.
pub fn bloqueado(tipo: Option<Tipo>, pedido: &Value) -> Option<String> {
    let accion = pedido.get("accion").and_then(Value::as_str).unwrap_or_default();
    if tipo.is_some() && accion == "automatizacion" {
        return Some("una automatización no puede crear ni tocar automatizaciones".into());
    }
    match (tipo, salta_con(accion)) {
        (Some(t), Some(s)) if t == s => Some(format!(
            "«{accion}» haría saltar otra vez esta automatización: sería un bucle"
        )),
        _ => None,
    }
}

/// Valores de ejemplo de las variables, para validar los pasos al crearlos.
fn vars_de_ejemplo() -> Vars {
    Vars {
        captura: Some(PathBuf::from("/capturas/captura.png")),
        resultado: "texto".into(),
        fecha: "2026-10-13".into(),
        hora: "08:00".into(),
        sincro: "Al día".into(),
        nombre: "automatización".into(),
    }
}

/// **Comprueba** una automatizacion: nombre, disparador, pasos conocidos
/// con sus argumentos, y que no se dispare a si misma.
pub fn validar(a: &Automatizacion) -> Result<(), String> {
    if a.id.trim().is_empty() {
        return Err("le falta el id".into());
    }
    if a.nombre.trim().is_empty() {
        return Err("le falta el nombre".into());
    }
    if let Disparador::ALaHora { hora, dias } = &a.disparador {
        minuto_del_dia(hora).ok_or_else(|| format!("la hora «{hora}» no se entiende (HH:MM)"))?;
        if dias.iter().any(|d| !(1..=7).contains(d)) {
            return Err("los días van del 1 (lunes) al 7 (domingo)".into());
        }
    }
    if a.pasos.is_empty() {
        return Err("no tiene pasos".into());
    }
    if a.pasos.len() > MAX_PASOS {
        return Err(format!("tiene {} pasos; como mucho {MAX_PASOS}", a.pasos.len()));
    }
    let vars = vars_de_ejemplo();
    for (i, p) in a.pasos.iter().enumerate() {
        let n = i + 1;
        match p {
            Paso::Accion { .. } => {
                let hecha = armar_accion(p, &vars, &|_| Some("pr-1".into())).map_err(|e| format!("paso {n}: {e}"))?;
                if let Some(m) = bloqueado(Some(a.disparador.tipo()), &hecha.pedido) {
                    return Err(format!("paso {n}: {m}"));
                }
            }
            Paso::Asistente { pedido, .. } => {
                if pedido.trim().is_empty() {
                    return Err(format!("paso {n}: el pedido al asistente está vacío"));
                }
            }
        }
    }
    Ok(())
}

/// `"8:05"` → 485. `None` si no es una hora.
pub fn minuto_del_dia(hora: &str) -> Option<u32> {
    let (h, m) = hora.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

// ---------------------------------------------------------------------------
// Las variables

/// Lo que se sabe al correr una automatizacion.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Vars {
    pub captura: Option<PathBuf>,
    /// Lo que contesto el paso anterior.
    pub resultado: String,
    pub fecha: String,
    pub hora: String,
    pub sincro: String,
    pub nombre: String,
}

impl Vars {
    /// Fecha y hora de ahora (hora local en ms).
    pub fn con_hora(mut self, local_ms: i64) -> Vars {
        let (a, m, d) = fecha_civil(local_ms.div_euclid(86_400_000));
        let min = local_ms.rem_euclid(86_400_000) / 60_000;
        self.fecha = format!("{a:04}-{m:02}-{d:02}");
        self.hora = format!("{:02}:{:02}", min / 60, min % 60);
        self
    }

    /// Cambia las `{variables}` de un texto. Una que no se conoce se deja.
    pub fn sustituir(&self, texto: &str) -> String {
        let captura = self
            .captura
            .as_ref()
            .map(|r| r.display().to_string())
            .unwrap_or_default();
        texto
            .replace("{captura}", &captura)
            .replace("{resultado}", &self.resultado)
            .replace("{fecha}", &self.fecha)
            .replace("{hora}", &self.hora)
            .replace("{sincro}", &self.sincro)
            .replace("{nombre}", &self.nombre)
    }

    fn sustituir_valor(&self, v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(self.sustituir(s)),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.sustituir_valor(x)).collect()),
            Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), self.sustituir_valor(x))).collect()),
            otro => otro.clone(),
        }
    }

    /// Las rutas de `imagenes` ya sustituidas (las vacias fuera).
    pub fn rutas(&self, imagenes: &[String]) -> Vec<PathBuf> {
        imagenes
            .iter()
            .map(|i| self.sustituir(i))
            .filter(|i| !i.trim().is_empty())
            .map(PathBuf::from)
            .collect()
    }
}

/// Dias desde 1970 a (año, mes, dia) (Howard Hinnant, `civil_from_days`).
pub fn fecha_civil(dias: i64) -> (i64, i64, i64) {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// El dia de la semana (1 lunes … 7 domingo) de una hora local en ms.
pub fn dia_de_la_semana(local_ms: i64) -> u8 {
    // El 1-ene-1970 fue jueves (4).
    ((local_ms.div_euclid(86_400_000) + 3).rem_euclid(7) + 1) as u8
}

// ---------------------------------------------------------------------------
// Armar un paso

/// Las acciones cuyo pedido entiende `proyecto` (`docs/protocolo-pedidos.md`).
const CON_PROYECTO: [&str; 9] = [
    "chat",
    "nota_nueva",
    "lienzo_nuevo",
    "grabar",
    "lista_nueva",
    "anadir_tarea",
    "soltar",
    "leccion_nueva",
    "lecciones",
];
/// Y las que entienden `imagenes`.
const CON_IMAGENES: [&str; 5] = ["chat", "nota_nueva", "anadir_tarea", "leccion_nueva", "enviar_al_movil"];

/// Pone `proyecto` en un pedido que lo entienda y no lo traiga (ni una lista
/// concreta: `anadir_tarea` con `codigo` ya sabe donde va).
pub fn con_proyecto(pedido: &mut Value, proyecto: &str) {
    let accion = pedido.get("accion").and_then(Value::as_str).unwrap_or_default();
    if !CON_PROYECTO.contains(&accion) {
        return;
    }
    if pedido.get("proyecto").is_some_and(|p| !p.is_null()) || pedido.get("codigo").is_some_and(|c| !c.is_null()) {
        return;
    }
    pedido["proyecto"] = json!(proyecto);
}

/// **El pedido de un paso de accion**, con las variables puestas y
/// validado por la tabla. `resolver` pasa el nombre (o el id) de un chat a su
/// id.
pub fn armar_accion(paso: &Paso, vars: &Vars, resolver: &dyn Fn(&str) -> Option<String>) -> Result<AccionApp, String> {
    let Paso::Accion {
        herramienta,
        args,
        proyecto,
        imagenes,
    } = paso
    else {
        return Err("no es un paso de acción".into());
    };
    let fila = acciones::por_nombre(herramienta).ok_or_else(|| format!("no hay ninguna acción «{herramienta}»"))?;
    let args: Map<String, Value> = args.iter().map(|(k, v)| (k.clone(), vars.sustituir_valor(v))).collect();
    let mut hecha = acciones::validar(fila, &args).map_err(|e| e.to_string())?;
    if let Some(p) = proyecto.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        if !CON_PROYECTO.contains(&fila.accion) {
            return Err(format!("«{herramienta}» no va a un chat concreto"));
        }
        let id = resolver(p).ok_or_else(|| format!("no hay ningún chat «{p}»"))?;
        con_proyecto(&mut hecha.pedido, &id);
    }
    let rutas = vars.rutas(imagenes);
    if !rutas.is_empty() {
        if !CON_IMAGENES.contains(&fila.accion) {
            return Err(format!("«{herramienta}» no lleva imágenes"));
        }
        let mut todas: Vec<Value> = hecha.pedido["imagenes"].as_array().cloned().unwrap_or_default();
        todas.extend(rutas.iter().map(|r| json!(r.display().to_string())));
        hecha.pedido["imagenes"] = Value::Array(todas);
    }
    Ok(hecha)
}

// ---------------------------------------------------------------------------
// Correr

/// Lo que hace de verdad cada paso (la app, o las pruebas).
pub trait Hacedor {
    /// Una accion de la app ya armada; pregunta antes si lleva `confirmar`.
    fn accion(&mut self, a: &AccionApp) -> Result<String, String>;
    /// Un pedido al asistente: lo que contesto.
    fn asistente(&mut self, pedido: &str, proyecto: Option<&str>, imagenes: &[PathBuf], bloquear: Tipo) -> Result<String, String>;
    /// El id de un chat por su nombre o su id.
    fn proyecto(&self, nombre: &str) -> Option<String>;
}

/// Como fue cada paso.
#[derive(Debug, Clone, PartialEq)]
pub struct Informe {
    pub pasos: Vec<Result<String, String>>,
}

impl Informe {
    pub fn bien(&self) -> bool {
        !self.pasos.is_empty() && self.pasos.iter().all(Result::is_ok)
    }

    /// Lo que contesto el ultimo paso que salio bien.
    pub fn ultimo(&self) -> Option<&str> {
        self.pasos.iter().rev().find_map(|r| r.as_ref().ok().map(String::as_str))
    }

    /// En una o dos lineas, para el aviso.
    pub fn resumen(&self, total: usize) -> String {
        match self.pasos.iter().position(Result::is_err) {
            Some(i) => format!(
                "⚠️ Paso {} de {total}: {}",
                i + 1,
                self.pasos[i].as_ref().err().map(String::as_str).unwrap_or_default()
            ),
            None => match self.ultimo() {
                Some(t) if !t.trim().is_empty() => recortar(t, 220),
                _ => format!("Hecho ({total} pasos)"),
            },
        }
    }
}

/// **Corre los pasos en orden.** El primero que falla para los demas (el
/// siguiente podria depender de el: «ponlo en el chat» y luego «hazle OCR»).
pub fn ejecutar(a: &Automatizacion, mut vars: Vars, h: &mut dyn Hacedor) -> Informe {
    vars.nombre = a.nombre.clone();
    let tipo = a.disparador.tipo();
    let mut pasos = Vec::new();
    for p in &a.pasos {
        let r = match p {
            Paso::Accion { .. } => armar_accion(p, &vars, &|n| h.proyecto(n)).and_then(|hecha| {
                match bloqueado(Some(tipo), &hecha.pedido) {
                    Some(m) => Err(m),
                    None => h.accion(&hecha),
                }
            }),
            Paso::Asistente {
                pedido,
                proyecto,
                imagenes,
            } => {
                let mut rutas = vars.rutas(imagenes);
                // Al capturar, la captura va con el pedido aunque no se diga.
                if rutas.is_empty()
                    && tipo == Tipo::AlCapturar
                    && let Some(c) = &vars.captura
                {
                    rutas.push(c.clone());
                }
                let proyecto = match proyecto.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
                    Some(p) => h.proyecto(p).ok_or_else(|| format!("no hay ningún chat «{p}»")),
                    None => Ok(String::new()),
                };
                proyecto.and_then(|pr| {
                    h.asistente(
                        &vars.sustituir(pedido),
                        Some(pr.as_str()).filter(|p| !p.is_empty()),
                        &rutas,
                        tipo,
                    )
                })
            }
        };
        let fallo = r.is_err();
        if let Ok(t) = &r {
            vars.resultado = t.clone();
        }
        pasos.push(r);
        if fallo {
            break;
        }
    }
    Informe { pasos }
}

// ---------------------------------------------------------------------------
// Crear desde un pedido (el asistente, «cada vez que…»)

/// Lo que llega en `receta` del pedido `automatizacion` (lo arma la fila
/// `crear_automatizacion` de la tabla): `nombre`, `disparador.tipo`,
/// `disparador.hora?`, `disparador.dias?` y `pasos` (una lista, o un texto
/// con una lista JSON; cada paso es un texto —un pedido al asistente— o un
/// objeto `{herramienta, args?, proyecto?, imagenes?}` o
/// `{asistente: "…", proyecto?, imagenes?}`).
pub fn de_receta(receta: &Value, id: String, aparato: &str, ahora: i64) -> Result<Automatizacion, String> {
    let nombre = receta
        .get("nombre")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .ok_or("falta el nombre")?
        .to_string();
    let d = receta.get("disparador").cloned().unwrap_or(Value::Null);
    let tipo = d
        .get("tipo")
        .and_then(Value::as_str)
        .or_else(|| d.as_str())
        .unwrap_or_default();
    let disparador = match acciones::normalizar(tipo.trim()).replace([' ', '-'], "_").as_str() {
        "al_capturar" => Disparador::AlCapturar,
        "al_sincronizar" => Disparador::AlSincronizar,
        "a_mano" => Disparador::AMano,
        "a_la_hora" => Disparador::ALaHora {
            hora: d.get("hora").and_then(Value::as_str).unwrap_or_default().trim().to_string(),
            dias: d
                .get("dias")
                .and_then(Value::as_array)
                .map(|l| l.iter().filter_map(|x| x.as_u64().map(|n| n as u8)).collect())
                .unwrap_or_default(),
        },
        otro => return Err(format!("disparador desconocido «{otro}»")),
    };
    let pasos_v = match receta.get("pasos") {
        Some(Value::String(s)) => match serde_json::from_str::<Value>(s) {
            Ok(v @ Value::Array(_)) => v,
            // Un texto que no es una lista: un pedido al asistente.
            _ if !s.trim().is_empty() => json!([s.trim()]),
            _ => return Err("faltan los pasos".into()),
        },
        Some(v @ Value::Array(_)) => v.clone(),
        _ => return Err("faltan los pasos".into()),
    };
    let mut pasos = Vec::new();
    for (i, p) in pasos_v.as_array().into_iter().flatten().enumerate() {
        pasos.push(paso_de(p).map_err(|e| format!("paso {}: {e}", i + 1))?);
    }
    let a = Automatizacion {
        id,
        nombre,
        activa: true,
        disparador,
        pasos,
        aparato: Some(aparato.to_string()).filter(|a| !a.is_empty()),
        creada: ahora,
        tocada: ahora,
        resto: Map::new(),
    };
    validar(&a)?;
    Ok(a)
}

/// Antes de mandar el pedido de crear una: si la receta no vale, el motivo
/// (para el modelo, que la arregla). Otro pedido pasa.
pub fn revisar_pedido(pedido: &Value) -> Result<(), String> {
    let es_crear = pedido.get("accion").and_then(Value::as_str) == Some("automatizacion")
        && pedido.get("operacion").and_then(Value::as_str) == Some("crear");
    if !es_crear {
        return Ok(());
    }
    let receta = pedido.get("receta").ok_or("falta la receta")?;
    de_receta(receta, "revisar".into(), "", 0)
        .map(|_| ())
        .map_err(|e| format!("la automatización no vale: {e}"))
}

fn paso_de(v: &Value) -> Result<Paso, String> {
    let cadena = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let imagenes: Vec<String> = v
        .get("imagenes")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    match v {
        Value::String(s) if !s.trim().is_empty() => Ok(Paso::Asistente {
            pedido: s.trim().to_string(),
            proyecto: None,
            imagenes: Vec::new(),
        }),
        Value::Object(o) => {
            if let Ok(p) = serde_json::from_value::<Paso>(v.clone()) {
                return Ok(p);
            }
            if let Some(pedido) = cadena("asistente").or_else(|| cadena("pedido")) {
                return Ok(Paso::Asistente {
                    pedido,
                    proyecto: cadena("proyecto"),
                    imagenes,
                });
            }
            let herramienta = cadena("herramienta")
                .or_else(|| cadena("accion"))
                .ok_or("ni «herramienta» ni «asistente»")?;
            let args = match o.get("args") {
                Some(Value::Object(a)) => a.clone(),
                _ => Map::new(),
            };
            Ok(Paso::Accion {
                herramienta,
                args,
                proyecto: cadena("proyecto"),
                imagenes,
            })
        }
        _ => Err("un paso es un texto o un objeto".into()),
    }
}

// ---------------------------------------------------------------------------
// Los disparos (el `motor.rs` de Windows)

/// Lo que hace saltar.
#[derive(Debug, Clone, PartialEq)]
pub enum Disparo {
    Captura(PathBuf),
    Sincro(String),
    AMano(String),
    /// Mirar la hora.
    Reloj,
}

/// **Si a una `a_la_hora` le toca ahora.** `ahora` y `ultima` son horas
/// locales en ms. Le toca si hoy es uno de sus dias, ya paso su hora (hace
/// menos de [`RECUPERAR_HASTA_MS`]) y no corrio desde entonces.
pub fn toca_a_la_hora(d: &Disparador, ahora: i64, ultima: Option<i64>) -> bool {
    let Disparador::ALaHora { hora, dias } = d else {
        return false;
    };
    let Some(minuto) = minuto_del_dia(hora) else {
        return false;
    };
    if !dias.is_empty() && !dias.contains(&dia_de_la_semana(ahora)) {
        return false;
    }
    let hoy = ahora.div_euclid(86_400_000) * 86_400_000;
    let su_hora = hoy + i64::from(minuto) * 60_000;
    ahora >= su_hora && ahora - su_hora < RECUPERAR_HASTA_MS && ultima.is_none_or(|u| u < su_hora)
}

/// **Lo que corre por un disparo**: las activas de este aparato con ese
/// disparador, cada una con sus variables. A mano, la pedida aunque este
/// apagada, sea de otro disparador o de otro aparato (se pidio a proposito,
/// y aqui).
pub fn elegir(
    lista: &Lista,
    disparo: &Disparo,
    aparato: &str,
    ahora_local: i64,
    ultimas: &BTreeMap<String, i64>,
) -> Vec<(Automatizacion, Vars)> {
    let base = Vars::default().con_hora(ahora_local);
    lista
        .buenas()
        .filter_map(|a| {
            let vars = match (disparo, &a.disparador) {
                // A mano corre aqui aunque sea de otro aparato: se pidio aqui.
                (Disparo::AMano(id), _) if *id == a.id => base.clone(),
                (Disparo::AMano(_), _) => return None,
                _ if !a.activa || !a.corre_aqui(aparato) => return None,
                (Disparo::Captura(r), Disparador::AlCapturar) => Vars {
                    captura: Some(r.clone()),
                    ..base.clone()
                },
                (Disparo::Sincro(t), Disparador::AlSincronizar) => Vars {
                    sincro: t.clone(),
                    ..base.clone()
                },
                (Disparo::Reloj, d @ Disparador::ALaHora { .. })
                    if toca_a_la_hora(d, ahora_local, ultimas.get(&a.id).copied()) =>
                {
                    base.clone()
                }
                _ => return None,
            };
            Some((a.clone(), vars))
        })
        .collect()
}

/// **El tope de vueltas.**
#[derive(Debug, Default)]
pub struct Guardia {
    veces: HashMap<String, VecDeque<i64>>,
}

impl Guardia {
    /// Si puede correr ahora (`ahora` en ms) y, si si, lo apunta.
    pub fn puede(&mut self, id: &str, ahora: i64) -> Result<(), String> {
        let v = self.veces.entry(id.to_string()).or_default();
        while v.front().is_some_and(|t| ahora - t >= 3_600_000) {
            v.pop_front();
        }
        if v.back().is_some_and(|t| ahora - t < HUECO_MINIMO_MS) {
            return Err("acaba de correr: se salta (¿un bucle?)".into());
        }
        if v.len() >= POR_HORA {
            return Err(format!("ya corrió {POR_HORA} veces en la última hora: se para (¿un bucle?)"));
        }
        v.push_back(ahora);
        Ok(())
    }
}

/// La guardia de este proceso (la del hilo del motor de Windows).
static GUARDIA: Mutex<Option<Guardia>> = Mutex::new(None);
/// Una vuelta a la vez (el hilo unico de Windows).
static CORRIENDO: Mutex<()> = Mutex::new(());
/// Un solo cambio a la vez en el fichero (la ventana, el pedido y el motor).
static CERROJO: Mutex<()> = Mutex::new(());

// ---------------------------------------------------------------------------
// El estado de este Mac (no viaja)

fn fichero_de_estado(raiz: &Path) -> PathBuf {
    raiz.join("asistente").join("automatizaciones-estado.json")
}

/// La ultima vez que corrio cada una (hora local en ms).
pub fn leer_estado(raiz: &Path) -> BTreeMap<String, i64> {
    std::fs::read_to_string(fichero_de_estado(raiz))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| serde_json::from_value(v.get("ultimas")?.clone()).ok())
        .unwrap_or_default()
}

pub fn guardar_estado(raiz: &Path, ultimas: &BTreeMap<String, i64>) {
    let ruta = fichero_de_estado(raiz);
    if let Some(d) = ruta.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(&ruta, json!({ "ultimas": ultimas }).to_string());
}

/// Una vuelta del historial (solo del Mac).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Vuelta {
    id: String,
    nombre: String,
    /// UTC en ms.
    cuando: i64,
    disparo: String,
    bien: bool,
    resumen: String,
}

fn fichero_de_historial(raiz: &Path) -> PathBuf {
    raiz.join("asistente").join("automatizaciones-historial.json")
}

fn leer_historial(raiz: &Path) -> Vec<Vuelta> {
    std::fs::read_to_string(fichero_de_historial(raiz))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| serde_json::from_value(v.get("vueltas")?.clone()).ok())
        .unwrap_or_default()
}

/// Apunta una vuelta (las nuevas delante) y deja las [`HISTORIAL`] ultimas.
fn apuntar_vuelta(raiz: &Path, v: Vuelta) {
    let mut todas = leer_historial(raiz);
    todas.insert(0, v);
    todas.truncate(HISTORIAL);
    let ruta = fichero_de_historial(raiz);
    if let Some(d) = ruta.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let tmp = ruta.with_extension("json.tmp");
    if std::fs::write(&tmp, json!({ "version": 1, "vueltas": todas }).to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, &ruta);
    }
}

// ---------------------------------------------------------------------------
// El disco

/// El fichero, en la carpeta de un chat.
pub fn fichero_en(carpeta_del_chat: &Path) -> PathBuf {
    carpeta_del_chat.join(RUTA_EN_EL_CHAT)
}

/// Lee la lista del chat. Sin fichero, vacia.
pub fn leer_de(carpeta_del_chat: &Path) -> Result<Lista, String> {
    match std::fs::read_to_string(fichero_en(carpeta_del_chat)) {
        Ok(t) => Lista::de_json(&t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Lista::default()),
        Err(e) => Err(e.to_string()),
    }
}

/// El chat «Asistente» (creandolo si no esta): su id y su carpeta.
pub fn chat_del_asistente(raiz: &Path, aparato: &str, ahora: i64) -> std::io::Result<(String, PathBuf)> {
    let f = pixpin_proyecto::almacen::asegurar_asistente(raiz, ahora, aparato)?;
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &f.id);
    Ok((f.id, carpeta))
}

/// La carpeta del chat «Asistente» si ya existe (sin crearlo solo por
/// mirar, como Windows).
fn carpeta_si_hay(raiz: &Path) -> Option<PathBuf> {
    let indice = pixpin_proyecto::almacen::Indice::leer_si_esta(raiz).ok()?;
    let f = indice.proyectos.iter().find(|f| f.es_asistente())?;
    Some(pixpin_proyecto::almacen::carpeta(raiz, &f.id))
}

/// La lista, si el chat «Asistente» ya existe (si no, vacia).
pub fn leer_si_hay(raiz: &Path) -> Result<Lista, String> {
    match carpeta_si_hay(raiz) {
        Some(c) => leer_de(&c),
        None => Ok(Lista::default()),
    }
}

/// **Cambia la lista** y la guarda (escritura atomica) y se asegura de que
/// el mensaje que la hace viajar este en el chat. `cambio` devuelve lo que se
/// le contesta al que pidio; si devuelve `Err`, no se escribe nada.
pub fn cambiar<R>(
    raiz: &Path,
    aparato: &str,
    ahora: i64,
    cambio: impl FnOnce(&mut Lista) -> Result<R, String>,
) -> Result<R, String> {
    let _g = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    let (proyecto, carpeta) = chat_del_asistente(raiz, aparato, ahora).map_err(|e| e.to_string())?;
    let mut lista = leer_de(&carpeta)?;
    let r = cambio(&mut lista)?;
    let ruta = fichero_en(&carpeta);
    if let Some(dir) = ruta.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = ruta.with_extension("json.tmp");
    std::fs::write(&tmp, lista.a_json()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &ruta).map_err(|e| e.to_string())?;
    senalar(&carpeta, &proyecto, aparato, ahora).map_err(|e| e.to_string())?;
    Ok(r)
}

/// El mensaje ARCHIVO del chat «Asistente» que senala el fichero (asi viaja
/// con la sincronizacion, como un `.leccion`). Si ya esta, no se toca
/// (`senalar_fichero` de Windows).
fn senalar(carpeta: &Path, proyecto: &str, aparato: &str, ahora: i64) -> std::io::Result<()> {
    use pixpin_proyecto::cuaderno;
    let c = match cuaderno::Cuaderno::leer_de(carpeta) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => return Err(e),
    };
    if c.mensajes.iter().any(|m| m.id == ID_MENSAJE) {
        return Ok(());
    }
    let numero = c.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let bytes = std::fs::metadata(carpeta.join(RUTA_EN_EL_CHAT))
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    let mut m = cuaderno::Mensaje::adjunto(
        cuaderno::Clase::Archivo,
        "automatizaciones.json",
        RUTA_PORTATIL,
        bytes,
        &cuaderno::Sello {
            cuando: ahora,
            numero,
            aparato: aparato.to_string(),
            proyecto: proyecto.to_string(),
        },
    );
    m.id = ID_MENSAJE.into();
    m.texto = TEXTO_DEL_MENSAJE.into();
    cuaderno::anadir(carpeta, &m)
}

/// Un id nuevo.
pub fn id_nuevo(ahora: i64) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    format!("auto-{ahora}-{}", N.fetch_add(1, Ordering::Relaxed))
}

/// Corta a `n` letras con «…».
pub fn recortar(t: &str, n: usize) -> String {
    let t = t.trim();
    if t.chars().count() <= n {
        return t.to_string();
    }
    let mut s: String = t.chars().take(n).collect();
    s.push('…');
    s
}

/// El id de un chat por su id o su nombre (sin mayusculas ni tildes; si no
/// hay uno igual, el unico que lo contenga).
pub fn proyecto_por_nombre(raiz: &Path, nombre: &str) -> Option<String> {
    let indice = pixpin_proyecto::almacen::Indice::leer(raiz);
    let q = acciones::normalizar(nombre.trim());
    if q.is_empty() {
        return None;
    }
    if let Some(f) = indice.proyectos.iter().find(|f| f.id == nombre.trim()) {
        return Some(f.id.clone());
    }
    let nombre_de = |f: &pixpin_proyecto::almacen::Ficha| {
        if f.es_guardados() {
            "mensajes guardados".to_string()
        } else {
            acciones::normalizar(&f.nombre)
        }
    };
    if let Some(f) = indice.proyectos.iter().find(|f| nombre_de(f) == q) {
        return Some(f.id.clone());
    }
    let parecidos: Vec<_> = indice.proyectos.iter().filter(|f| nombre_de(f).contains(&q)).collect();
    match parecidos.as_slice() {
        [uno] => Some(uno.id.clone()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Lo que ve Swift

/// **Las manos de las automatizaciones en la app**: lo implementa Swift. Se
/// le llama desde el hilo de la vuelta (nunca el principal) y puede esperar.
#[uniffi::export(foreign)]
pub trait ManosAutomatizacion: Send + Sync {
    /// Empieza una (el aviso sale ya, no al acabar: si un paso pregunta, se
    /// ve en el).
    fn empieza(&self, id: String, nombre: String);
    /// Pregunta al usuario y espera. Sin respuesta es un no.
    fn confirmar(&self, pregunta: String) -> bool;
    /// Hace un pedido del protocolo (`{"pixpin":1,"accion":...}`), como si
    /// llegara de `pixpinmax://`. `None`: hecho; si no, el motivo.
    fn pedido(&self, json: String) -> Option<String>;
    /// Un turno del asistente sin ventana. `proyecto` es el id del chat
    /// sobre el que se pide; `bloquear`, el disparador (`al_capturar`…): lo
    /// que lo haria saltar no se hace dentro del turno.
    fn asistente(&self, pedido: String, proyecto: Option<String>, imagenes: Vec<String>, bloquear: String) -> RespuestaPaso;
    /// Acabo una: lo que paso, para el aviso.
    fn acabo(&self, informe: InformeAutomatizacion);
}

/// Lo que contesto un paso hecho en Swift.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RespuestaPaso {
    pub bien: bool,
    /// La respuesta, o el motivo si fallo.
    pub texto: String,
}

/// **Lo que hace saltar**, desde Swift.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum DisparoAutomatizacion {
    /// Una captura nueva en la carpeta de capturas.
    Captura { ruta: String },
    /// Una vuelta de la sincronizacion que trajo algo (lo que dijo).
    Sincro { texto: String },
    /// Correr esa ya.
    AMano { id: String },
    /// Mirar la hora (cada 30 s).
    Reloj,
}

impl DisparoAutomatizacion {
    fn interno(&self) -> Disparo {
        match self {
            DisparoAutomatizacion::Captura { ruta } => Disparo::Captura(PathBuf::from(ruta)),
            DisparoAutomatizacion::Sincro { texto } => Disparo::Sincro(texto.clone()),
            DisparoAutomatizacion::AMano { id } => Disparo::AMano(id.clone()),
            DisparoAutomatizacion::Reloj => Disparo::Reloj,
        }
    }

    fn nombre(&self) -> &'static str {
        match self {
            DisparoAutomatizacion::Captura { .. } => "al_capturar",
            DisparoAutomatizacion::Sincro { .. } => "al_sincronizar",
            DisparoAutomatizacion::AMano { .. } => "a_mano",
            DisparoAutomatizacion::Reloj => "a_la_hora",
        }
    }
}

/// Como fue una vuelta.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct InformeAutomatizacion {
    pub id: String,
    pub nombre: String,
    pub bien: bool,
    /// En una o dos lineas, para el aviso.
    pub resumen: String,
    /// Cada paso hecho: bien y lo que dijo (el que fallo es el ultimo).
    pub pasos: Vec<RespuestaPaso>,
    /// Si no corrio (la guardia de bucles): por que.
    pub saltada: Option<String>,
}

/// Un argumento de un paso de accion, en texto (`valor` es el texto, o el
/// JSON si no lo era: una lista, un numero).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ArgAutomatizacion {
    pub nombre: String,
    pub valor: String,
}

/// Un paso, para el editor.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PasoAutomatizacion {
    /// `true`: un pedido al asistente; `false`: una accion de la tabla.
    pub es_asistente: bool,
    pub herramienta: String,
    pub args: Vec<ArgAutomatizacion>,
    pub pedido: String,
    pub proyecto: Option<String>,
    pub imagenes: Vec<String>,
}

/// **Una automatizacion, para la lista y el editor.**
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FichaAutomatizacion {
    /// Vacio en una nueva (al guardar nace su id).
    pub id: String,
    pub nombre: String,
    pub activa: bool,
    /// `al_capturar`, `a_la_hora`, `al_sincronizar` o `a_mano`.
    pub disparador: String,
    /// `HH:MM`, en `a_la_hora`.
    pub hora: String,
    /// 1 lunes … 7 domingo; vacio, todos.
    pub dias: Vec<u8>,
    pub pasos: Vec<PasoAutomatizacion>,
    /// «Al capturar · escribir_en_chat en «Obra» → «Hazle OCR»».
    pub detalle: String,
    /// `None`: corre aqui; si no, el aparato donde corre.
    pub otro_aparato: Option<String>,
    /// Si no se entiende, el motivo (solo se puede borrar).
    pub rota: Option<String>,
    /// La ultima vez que corrio aqui (hora local en ms).
    pub ultima_vez: Option<i64>,
}

/// Una vuelta del historial.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct VueltaAutomatizacion {
    pub id: String,
    pub nombre: String,
    /// UTC en ms.
    pub cuando: i64,
    /// Lo que la hizo saltar (`al_capturar`…).
    pub disparo: String,
    pub bien: bool,
    pub resumen: String,
}

/// Un parametro de una accion, para el editor.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ParamAutomatizacion {
    pub nombre: String,
    pub descripcion: String,
    pub obligatorio: bool,
    /// Las que valen, si es de opciones.
    pub opciones: Vec<String>,
}

/// Una accion de la tabla, para elegirla en el editor.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct HerramientaAutomatizacion {
    pub nombre: String,
    pub descripcion: String,
    pub params: Vec<ParamAutomatizacion>,
    /// Si el pedido entiende `proyecto` (un chat concreto).
    pub con_proyecto: bool,
    /// Si el pedido lleva `imagenes`.
    pub con_imagenes: bool,
}

/// Un valor JSON en texto, para el editor.
fn a_texto(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        otro => otro.to_string(),
    }
}

/// Lo que se escribio en el editor, a JSON: una lista escrita como tal
/// (`["a","b"]`) es una lista; lo demas, texto (la tabla lo convierte).
fn de_texto(s: &str) -> Value {
    let t = s.trim();
    if t.starts_with('[')
        && let Ok(v @ Value::Array(_)) = serde_json::from_str::<Value>(t)
    {
        return v;
    }
    Value::String(s.to_string())
}

fn paso_a_ficha(p: &Paso) -> PasoAutomatizacion {
    match p {
        Paso::Accion {
            herramienta,
            args,
            proyecto,
            imagenes,
        } => PasoAutomatizacion {
            es_asistente: false,
            herramienta: herramienta.clone(),
            args: args
                .iter()
                .map(|(k, v)| ArgAutomatizacion {
                    nombre: k.clone(),
                    valor: a_texto(v),
                })
                .collect(),
            pedido: String::new(),
            proyecto: proyecto.clone(),
            imagenes: imagenes.clone(),
        },
        Paso::Asistente {
            pedido,
            proyecto,
            imagenes,
        } => PasoAutomatizacion {
            es_asistente: true,
            herramienta: String::new(),
            args: Vec::new(),
            pedido: pedido.clone(),
            proyecto: proyecto.clone(),
            imagenes: imagenes.clone(),
        },
    }
}

fn limpio(s: &Option<String>) -> Option<String> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn paso_de_ficha(p: &PasoAutomatizacion) -> Paso {
    let imagenes: Vec<String> = p
        .imagenes
        .iter()
        .map(|i| i.trim().to_string())
        .filter(|i| !i.is_empty())
        .collect();
    if p.es_asistente {
        Paso::Asistente {
            pedido: p.pedido.trim().to_string(),
            proyecto: limpio(&p.proyecto),
            imagenes,
        }
    } else {
        Paso::Accion {
            herramienta: p.herramienta.trim().to_string(),
            args: p
                .args
                .iter()
                .filter(|a| !a.valor.trim().is_empty())
                .map(|a| (a.nombre.clone(), de_texto(&a.valor)))
                .collect(),
            proyecto: limpio(&p.proyecto),
            imagenes,
        }
    }
}

/// La ficha del editor, a una automatizacion (sin validar).
fn de_ficha(f: &FichaAutomatizacion, id: String) -> Result<Automatizacion, String> {
    let disparador = match f.disparador.as_str() {
        "al_capturar" => Disparador::AlCapturar,
        "al_sincronizar" => Disparador::AlSincronizar,
        "a_mano" => Disparador::AMano,
        "a_la_hora" => {
            let mut dias = f.dias.clone();
            dias.sort_unstable();
            dias.dedup();
            Disparador::ALaHora {
                hora: f.hora.trim().to_string(),
                dias,
            }
        }
        otro => return Err(format!("disparador desconocido «{otro}»")),
    };
    Ok(Automatizacion {
        id,
        nombre: f.nombre.trim().to_string(),
        activa: f.activa,
        disparador,
        pasos: f.pasos.iter().map(paso_de_ficha).collect(),
        aparato: None,
        creada: 0,
        tocada: 0,
        resto: Map::new(),
    })
}

/// Las fichas de la lista, en su orden.
pub fn fichas(lista: &Lista, aparato: &str, ultimas: &BTreeMap<String, i64>) -> Vec<FichaAutomatizacion> {
    lista
        .entradas
        .iter()
        .map(|e| match e {
            Entrada::Buena(a) => {
                let (hora, dias) = match &a.disparador {
                    Disparador::ALaHora { hora, dias } => (hora.clone(), dias.clone()),
                    _ => (String::new(), Vec::new()),
                };
                FichaAutomatizacion {
                    id: a.id.clone(),
                    nombre: a.nombre.clone(),
                    activa: a.activa,
                    disparador: a.disparador.tipo().nombre().into(),
                    hora,
                    dias,
                    pasos: a.pasos.iter().map(paso_a_ficha).collect(),
                    detalle: format!("{} · {}", a.disparador.en_palabras(), a.pasos_en_palabras()),
                    otro_aparato: (!a.corre_aqui(aparato)).then(|| a.aparato.clone().unwrap_or_default()),
                    rota: None,
                    ultima_vez: ultimas.get(&a.id).copied(),
                }
            }
            Entrada::Rota { valor, motivo } => FichaAutomatizacion {
                id: valor.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
                nombre: valor
                    .get("nombre")
                    .and_then(Value::as_str)
                    .unwrap_or("(sin nombre)")
                    .to_string(),
                activa: false,
                disparador: valor
                    .pointer("/disparador/tipo")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                hora: String::new(),
                dias: Vec::new(),
                pasos: Vec::new(),
                detalle: String::new(),
                otro_aparato: None,
                rota: Some(motivo.clone()),
                ultima_vez: None,
            },
        })
        .collect()
}

/// **Si un pedido no se puede hacer ahora** porque lo pide el asistente
/// dentro de un paso de una automatizacion de ese disparador (`al_capturar`,
/// …; `None` si no corre ninguna): el motivo. Lo usa `HacerPedidos`.
#[uniffi::export]
pub fn automatizacion_bloqueada(disparador: Option<String>, accion: String) -> Option<String> {
    let tipo = disparador.as_deref().and_then(Tipo::de_nombre);
    bloqueado(tipo, &json!({ "accion": accion }))
}

/// **Las tareas del usuario, para los pasos de tareas** (`tareas::hacer`
/// las busca por su nombre): las mismas que ve el asistente.
struct FuenteDelMac<'a> {
    nucleo: &'a Nucleo,
    desfase_ms: i64,
    ahora: Momento,
}

impl FuenteTareas for FuenteDelMac<'_> {
    fn listas(&self) -> Vec<VistaLista> {
        let Ok(listas) = self.nucleo.tareas(self.desfase_ms) else {
            return Vec::new();
        };
        listas
            .into_iter()
            .map(|l| VistaLista {
                proyecto: l.chat,
                chat: l.nombre_chat,
                guardados: l.guardados,
                codigo: l.mensaje,
                titulo: l.titulo,
                cuando: l.cuando,
                tareas: l
                    .tareas
                    .into_iter()
                    .map(|t| VistaTarea {
                        indice: t.indice as usize,
                        texto: t.texto.trim().to_string(),
                        hecha: t.hecha,
                        creada: t.creada.as_deref().and_then(dia_de),
                        vence: t.recuerda.map(Momento::de_ms_locales),
                    })
                    .collect(),
            })
            .collect()
    }

    fn chats(&self) -> Vec<Chat> {
        pixpin_proyecto::almacen::Indice::leer_si_esta(self.nucleo.ruta())
            .map(|i| {
                i.proyectos
                    .iter()
                    .map(|f| Chat {
                        id: f.id.clone(),
                        nombre: f.nombre.clone(),
                        guardados: f.es_guardados(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn ahora(&self) -> Momento {
        self.ahora
    }
}

/// `AAAA-MM-DD` en dias desde 1970.
fn dia_de(s: &str) -> Option<i64> {
    let mut p = s.trim().splitn(3, '-').map(|x| x.parse::<i64>().ok());
    let (y, m, d) = (p.next()??, p.next()??, p.next()??);
    fechas::existe(y, m, d).then(|| fechas::dias_de(y, m, d))
}

/// Los pasos de verdad: las acciones por `tareas::hacer` (que pregunta antes
/// de lo que borra) y el resto, Swift.
struct DeVerdad<'a> {
    nucleo: &'a Nucleo,
    manos: &'a Arc<dyn ManosAutomatizacion>,
    desfase_ms: i64,
}

impl Hacedor for DeVerdad<'_> {
    fn accion(&mut self, a: &AccionApp) -> Result<String, String> {
        let manos = self.manos.clone();
        let confirmar = |q: &str| manos.confirmar(q.to_string());
        let mut mandar = |p: &Value| -> Result<(), String> { self.manos.pedido(p.to_string()).map_or(Ok(()), Err) };
        let fuente = FuenteDelMac {
            nucleo: self.nucleo,
            desfase_ms: self.desfase_ms,
            ahora: Momento::de_ms_locales(crate::ahora_ms() + self.desfase_ms),
        };
        pixpin_ia::tareas::hacer(a, &fuente, &confirmar, &mut mandar)
    }

    fn asistente(&mut self, pedido: &str, proyecto: Option<&str>, imagenes: &[PathBuf], bloquear: Tipo) -> Result<String, String> {
        let r = self.manos.asistente(
            pedido.to_string(),
            proyecto.map(str::to_string),
            imagenes.iter().map(|r| r.display().to_string()).collect(),
            bloquear.nombre().to_string(),
        );
        if r.bien { Ok(r.texto) } else { Err(r.texto) }
    }

    fn proyecto(&self, nombre: &str) -> Option<String> {
        proyecto_por_nombre(self.nucleo.ruta(), nombre)
    }
}

fn argumento(detalle: impl Into<String>) -> ErrorNucleo {
    ErrorNucleo::Argumento { detalle: detalle.into() }
}

impl Nucleo {
    fn codigo_del_aparato(&self) -> Result<String, ErrorNucleo> {
        Ok(self.identidad()?.yo.codigo())
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Las automatizaciones** del chat «Asistente», en su orden (las rotas
    /// tambien, con su motivo). Sin chat «Asistente», ninguna (no se crea
    /// solo por mirar).
    pub fn automatizaciones(&self) -> Result<Vec<FichaAutomatizacion>, ErrorNucleo> {
        let aparato = self.codigo_del_aparato()?;
        let lista = leer_si_hay(self.ruta()).map_err(argumento)?;
        Ok(fichas(&lista, &aparato, &leer_estado(self.ruta())))
    }

    /// **Guarda una del editor**: nueva (sin `id`: nace con este Mac como
    /// su aparato) o cambiada (conserva su aparato, lo que no se entiende y
    /// cuando nacio). Si no vale, el motivo y no se escribe nada. Devuelve
    /// su id.
    pub fn automatizacion_guardar(&self, ficha: FichaAutomatizacion) -> Result<String, ErrorNucleo> {
        let aparato = self.codigo_del_aparato()?;
        let ahora = crate::ahora_ms();
        let nueva = ficha.id.trim().is_empty();
        let id = if nueva { id_nuevo(ahora) } else { ficha.id.clone() };
        let mut a = de_ficha(&ficha, id.clone()).map_err(argumento)?;
        if nueva {
            a.aparato = Some(aparato.clone());
            a.creada = ahora;
        }
        cambiar(self.ruta(), &aparato, ahora, |l| l.poner(a, ahora)).map_err(argumento)?;
        Ok(id)
    }

    /// Si la del editor vale: `None`, o el motivo.
    pub fn automatizacion_revisar(&self, ficha: FichaAutomatizacion) -> Option<String> {
        let id = if ficha.id.trim().is_empty() { "nueva".to_string() } else { ficha.id.clone() };
        de_ficha(&ficha, id).and_then(|a| validar(&a)).err()
    }

    /// **Crea una desde una receta** (el pedido `automatizacion` con
    /// `operacion: "crear"`, que manda el asistente tras preguntar). Devuelve
    /// su nombre; si no vale, el motivo y no se guarda.
    pub fn automatizacion_de_receta(&self, receta_json: String) -> Result<String, ErrorNucleo> {
        let receta: Value = serde_json::from_str(&receta_json).map_err(|_| argumento("falta la receta"))?;
        let aparato = self.codigo_del_aparato()?;
        let ahora = crate::ahora_ms();
        cambiar(self.ruta(), &aparato, ahora, |l| {
            let a = de_receta(&receta, id_nuevo(ahora), &aparato, ahora)?;
            let nombre = a.nombre.clone();
            l.anadir(a)?;
            Ok(nombre)
        })
        .map_err(argumento)
    }

    /// Enciende o apaga una. Su nombre, o `None` si no esta.
    pub fn automatizacion_activar(&self, id: String, activa: bool) -> Result<Option<String>, ErrorNucleo> {
        let aparato = self.codigo_del_aparato()?;
        let ahora = crate::ahora_ms();
        cambiar(self.ruta(), &aparato, ahora, |l| {
            let n = l.buscar(&id).map(|a| a.nombre.clone());
            l.activar(&id, activa, ahora);
            Ok(n)
        })
        .map_err(argumento)
    }

    /// Borra una (buena o rota). Su nombre, o `None` si no estaba. No
    /// pregunta: quien lo pide ya pregunto.
    pub fn automatizacion_borrar(&self, id: String) -> Result<Option<String>, ErrorNucleo> {
        let aparato = self.codigo_del_aparato()?;
        let ahora = crate::ahora_ms();
        cambiar(self.ruta(), &aparato, ahora, |l| {
            let n = l.buscar(&id).map(|a| a.nombre.clone());
            Ok(if l.borrar(&id) { Some(n.unwrap_or_else(|| id.clone())) } else { None })
        })
        .map_err(argumento)
    }

    /// **Una vuelta del motor**: elige lo que corre por `disparo` (las
    /// activas de este Mac con ese disparador; a la hora, las que les toca;
    /// a mano, la pedida), pasa la guardia de bucles y las corre en orden,
    /// diciendoselo a `manos` al empezar y al acabar cada una. Espera: se
    /// llama fuera del hilo principal. Una vuelta a la vez.
    pub fn automatizaciones_disparar(
        &self,
        disparo: DisparoAutomatizacion,
        desfase_ms: i64,
        manos: Arc<dyn ManosAutomatizacion>,
    ) -> Result<Vec<InformeAutomatizacion>, ErrorNucleo> {
        let _uno = CORRIENDO.lock().unwrap_or_else(|e| e.into_inner());
        let raiz = self.ruta();
        let lista = leer_si_hay(raiz).map_err(argumento)?;
        if lista.buenas().next().is_none() {
            return Ok(Vec::new());
        }
        let aparato = self.codigo_del_aparato()?;
        let interno = disparo.interno();
        let ahora_local = crate::ahora_ms() + desfase_ms;
        let mut ultimas = leer_estado(raiz);
        let elegidas = elegir(&lista, &interno, &aparato, ahora_local, &ultimas);
        let mut informes = Vec::new();
        for (a, vars) in elegidas {
            let puede = GUARDIA
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_or_insert_with(Guardia::default)
                .puede(&a.id, crate::ahora_ms());
            if let Err(m) = puede {
                informes.push(InformeAutomatizacion {
                    id: a.id.clone(),
                    nombre: a.nombre.clone(),
                    bien: false,
                    resumen: m.clone(),
                    pasos: Vec::new(),
                    saltada: Some(m),
                });
                continue;
            }
            if matches!(a.disparador, Disparador::ALaHora { .. }) || matches!(interno, Disparo::AMano(_)) {
                ultimas.insert(a.id.clone(), ahora_local);
                guardar_estado(raiz, &ultimas);
            }
            manos.empieza(a.id.clone(), a.nombre.clone());
            let mut h = DeVerdad {
                nucleo: self,
                manos: &manos,
                desfase_ms,
            };
            let inf = ejecutar(&a, vars, &mut h);
            let informe = InformeAutomatizacion {
                id: a.id.clone(),
                nombre: a.nombre.clone(),
                bien: inf.bien(),
                resumen: inf.resumen(a.pasos.len()),
                pasos: inf
                    .pasos
                    .iter()
                    .map(|r| match r {
                        Ok(t) => RespuestaPaso { bien: true, texto: t.clone() },
                        Err(t) => RespuestaPaso { bien: false, texto: t.clone() },
                    })
                    .collect(),
                saltada: None,
            };
            apuntar_vuelta(
                raiz,
                Vuelta {
                    id: a.id.clone(),
                    nombre: a.nombre.clone(),
                    cuando: crate::ahora_ms(),
                    disparo: disparo.nombre().into(),
                    bien: informe.bien,
                    resumen: informe.resumen.clone(),
                },
            );
            manos.acabo(informe.clone());
            informes.push(informe);
        }
        Ok(informes)
    }

    /// **El historial** de este Mac: las ultimas vueltas, la mas nueva
    /// primero (no viaja).
    pub fn automatizaciones_historial(&self) -> Vec<VueltaAutomatizacion> {
        leer_historial(self.ruta())
            .into_iter()
            .map(|v| VueltaAutomatizacion {
                id: v.id,
                nombre: v.nombre,
                cuando: v.cuando,
                disparo: v.disparo,
                bien: v.bien,
                resumen: v.resumen,
            })
            .collect()
    }
}

/// **Las acciones que se pueden poner en un paso** (la tabla del asistente,
/// sin crear automatizaciones ni lo que el Mac aun no hace: lo del chat y
/// los documentos).
#[uniffi::export]
pub fn automatizacion_herramientas() -> Vec<HerramientaAutomatizacion> {
    acciones::todas()
        .filter(|a| {
            a.accion != "automatizacion"
                && !acciones::es_en_el_chat(a.herramienta)
                && !pixpin_ia::acciones_documentos::es_de_documentos(a.herramienta)
        })
        .map(|a| HerramientaAutomatizacion {
            nombre: a.herramienta.into(),
            descripcion: a.descripcion.into(),
            params: a
                .params
                .iter()
                .map(|p| ParamAutomatizacion {
                    nombre: p.nombre.into(),
                    descripcion: p.descripcion.into(),
                    obligatorio: p.obligatorio,
                    opciones: match p.tipo {
                        acciones::Tipo::Opciones(o) => o.iter().map(|x| x.to_string()).collect(),
                        _ => Vec::new(),
                    },
                })
                .collect(),
            con_proyecto: CON_PROYECTO.contains(&a.accion),
            con_imagenes: CON_IMAGENES.contains(&a.accion),
        })
        .collect()
}

#[cfg(test)]
#[path = "automatizaciones_pruebas.rs"]
mod pruebas;
