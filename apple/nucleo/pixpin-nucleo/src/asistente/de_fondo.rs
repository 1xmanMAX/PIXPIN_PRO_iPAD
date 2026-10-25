//! **El asistente sigue aunque se cierre el chat, y retoma lo cortado al
//! arrancar** (Windows, `ventana_chat/asistente/de_fondo.rs`, 24-oct-2026).
//! El usuario: «cuando cierro la aplicación por accidente, el asistente como
//! que se olvida de la tarea … y al final no hace nada».
//!
//! En el Mac el turno no vive en la ventana del chat: lo lleva
//! `AsistenteEnElChat` (uno para toda la app) en un hilo de GCD, y escribe
//! su respuesta en el cuaderno, asi que cerrar la ventana no lo para. Lo que
//! faltaba es lo de cerrar **PixPin entera** (Salir, o se cae):
//!
//! - Cada turno se **apunta en el disco** al empezar ([`FICHERO`], el mismo
//!   nombre y formato que Windows; uno por equipo, no viaja) y se borra al
//!   acabar; la primera herramienta que hace algo lo marca ([`marcar_que_actuo`]).
//! - Lo que siga apuntado al arrancar se corto a medias
//!   ([`Nucleo::ia_retomar_al_arrancar`]): si aun no habia hecho nada con
//!   sus herramientas, **se vuelve a lanzar solo** (una respuesta nueva
//!   debajo); si ya habia hecho algo, no se repite (apuntaria o dibujaria
//!   dos veces) y la burbuja cortada lo dice con claridad, con como seguir.
//!   Un pedido que ya se retomo una vez no se retoma otra.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno;
use serde::{Deserialize, Serialize};

use super::chat as ia_chat;
use super::en_la_conversacion::InicioIa;
use super::turno::AjustesIa;
use crate::{MensajeChat, Nucleo};

/// El fichero, en la raiz de los datos (uno por equipo, no viaja con la
/// sincro: lo que esta a medias es de este Mac).
pub(crate) const FICHERO: &str = "asistente-en-marcha.json";
const VERSION: u32 = 1;
/// Mas viejo que esto, no se retoma solo: el usuario ya no lo espera.
pub(crate) const RETOMAR_HASTA_MS: i64 = 24 * 60 * 60 * 1000;

/// **Un turno en marcha, tal como se apunta** (los mismos campos que Windows).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EnMarcha {
    pub proyecto: String,
    /// La nota de la respuesta.
    pub nota: String,
    /// La nota de la pregunta (de ahi sale el pedido al retomarlo).
    pub pregunta: String,
    /// Si es el chat «Asistente» (todo lo escrito es para el) o otro, donde
    /// hay que nombrarlo con `@asistente`.
    #[serde(default)]
    pub asistente: bool,
    /// Cuando empezo (ms UTC).
    pub desde: i64,
    /// Alguna herramienta ya hizo algo.
    #[serde(default)]
    pub actuo: bool,
    /// Las veces que ya se retomo.
    #[serde(default)]
    pub intentos: u32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Apuntados {
    version: u32,
    #[serde(default)]
    turnos: Vec<EnMarcha>,
}

/// El fichero lo tocan los hilos de los turnos y el de retomar.
static CANDADO: Mutex<()> = Mutex::new(());

/// Los retomados que aun no han empezado: cuantas veces se retomaron (la
/// respuesta nueva, por su nota). Lo lee [`apuntar`] al empezar su turno.
static RETOMADOS: Mutex<Option<HashMap<String, u32>>> = Mutex::new(None);

fn leer(raiz: &Path) -> Vec<EnMarcha> {
    std::fs::read_to_string(raiz.join(FICHERO))
        .ok()
        .and_then(|t| serde_json::from_str::<Apuntados>(&t).ok())
        .map(|a| a.turnos)
        .unwrap_or_default()
}

/// Lo escribe de una vez (al lado y renombrando); sin nada, lo borra.
fn escribir(raiz: &Path, turnos: &[EnMarcha]) -> std::io::Result<()> {
    let ruta = raiz.join(FICHERO);
    if turnos.is_empty() {
        return match std::fs::remove_file(&ruta) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let a = Apuntados {
        version: VERSION,
        turnos: turnos.to_vec(),
    };
    let texto = serde_json::to_string_pretty(&a).map_err(std::io::Error::other)?;
    let parte = ruta.with_extension("parte");
    std::fs::write(&parte, texto)?;
    std::fs::rename(&parte, &ruta)
}

fn cambiar(raiz: &Path, f: impl FnOnce(&mut Vec<EnMarcha>)) {
    let _g = CANDADO.lock().unwrap_or_else(|e| e.into_inner());
    let mut v = leer(raiz);
    f(&mut v);
    // Si no se puede apuntar, el turno sigue igual: solo no se retomaria.
    let _ = escribir(raiz, &v);
}

/// Apunta un turno que empieza (uno anterior de la misma nota se cambia).
/// Si es uno retomado, con sus intentos.
pub(crate) fn apuntar(raiz: &Path, mut e: EnMarcha) {
    if let Some(n) = RETOMADOS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()
        .and_then(|m| m.remove(&e.nota))
    {
        e.intentos = e.intentos.max(n);
    }
    cambiar(raiz, |v| {
        v.retain(|x| x.nota != e.nota);
        v.push(e);
    });
}

/// El turno acabo (bien, mal o parado): ya no esta a medias.
pub(crate) fn quitar(raiz: &Path, nota: &str) {
    cambiar(raiz, |v| v.retain(|x| x.nota != nota));
}

/// Una herramienta del turno ya hizo algo.
pub(crate) fn marcar_que_actuo(raiz: &Path, nota: &str) {
    cambiar(raiz, |v| v.iter_mut().filter(|x| x.nota == nota).for_each(|x| x.actuo = true));
}

/// Lo apuntado, vaciando el fichero: lo coge quien lo va a retomar.
fn tomar_apuntados(raiz: &Path) -> Vec<EnMarcha> {
    let _g = CANDADO.lock().unwrap_or_else(|e| e.into_inner());
    let v = leer(raiz);
    let _ = escribir(raiz, &[]);
    v
}

// ---------------------------------------------------------------------------
// Que hacer con lo cortado (puro, como Windows)

/// Por que no se retoma solo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Motivo {
    /// Ya hizo algo con sus herramientas: repetirlo lo haria dos veces.
    YaActuo,
    /// Ya se retomo una vez y volvio a cortarse.
    YaSeRetomo,
    /// Es de hace demasiado.
    Viejo,
    /// No se encuentra la pregunta (se borro).
    SinPregunta,
    /// No se pudo volver a lanzar (sin modelo, sin clave…): el aviso.
    NoSePudo(String),
}

/// Lo que se hace con un turno que quedo a medias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Retomar {
    /// Se vuelve a lanzar, desde el principio.
    OtraVez,
    /// Solo se dice en la burbuja (y como seguir).
    Avisar(Motivo),
}

/// **La regla**: se retoma solo lo que no hizo nada todavia, no se habia
/// retomado ya, es reciente y conserva su pregunta.
pub(crate) fn que_hacer(e: &EnMarcha, ahora: i64, hay_pregunta: bool) -> Retomar {
    if !hay_pregunta {
        Retomar::Avisar(Motivo::SinPregunta)
    } else if e.actuo {
        Retomar::Avisar(Motivo::YaActuo)
    } else if e.intentos > 0 {
        Retomar::Avisar(Motivo::YaSeRetomo)
    } else if ahora.saturating_sub(e.desde) > RETOMAR_HASTA_MS {
        Retomar::Avisar(Motivo::Viejo)
    } else {
        Retomar::OtraVez
    }
}

/// Lo que quedo escrito, sin lo que solo tiene sentido en vivo: el cursor,
/// el «💭 Pensando…» de antes de llegar nada y la linea de estado de una
/// herramienta («✏️ dibujando…»), que ya no esta pasando.
pub(crate) fn sin_lo_vivo(texto: &str) -> String {
    let t = texto.trim_end();
    let t = t.strip_suffix('▍').unwrap_or(t).trim_end();
    let mut lineas: Vec<&str> = t.lines().collect();
    if let Some(ultima) = lineas.last().map(|l| l.trim()) {
        let estado = ultima == ia_chat::ESTADO_INICIAL
            || (ultima.ends_with('…') && ultima.chars().count() <= 60 && ultima.chars().next().is_some_and(|c| !c.is_alphanumeric()));
        if estado {
            lineas.pop();
        }
    }
    lineas.join("\n").trim().to_string()
}

/// **El texto de la burbuja cortada**: lo que ya habia escrito y, debajo,
/// lo que paso y como seguir (los textos de Windows).
pub(crate) fn texto_cortado(parcial: &str, r: &Retomar, asistente: bool) -> String {
    let sigue = if asistente { "«continúa»" } else { "«@asistente continúa»" };
    let aviso = match r {
        Retomar::OtraVez => "↻ PixPin se cerró antes de que acabara. Lo retomo desde el principio en la respuesta de abajo.".to_string(),
        Retomar::Avisar(m) => {
            let porque = match m {
                Motivo::YaActuo => format!(
                    "Algunos pasos ya estaban hechos, así que no lo repito solo (se harían dos veces). Escribe {sigue} y sigo desde aquí."
                ),
                Motivo::YaSeRetomo => {
                    format!("Ya lo había retomado una vez y volvió a cortarse, así que no insisto. Escribe {sigue} si quieres que lo intente otra vez.")
                }
                Motivo::Viejo => format!("Ha pasado demasiado para retomarlo solo. Escribe {sigue} si aún lo quieres."),
                Motivo::SinPregunta => "No encuentro tu pedido (¿se borró?), así que no puedo retomarlo.".to_string(),
                Motivo::NoSePudo(aviso) => format!("No he podido retomarlo: {aviso}"),
            };
            format!("⚠️ PixPin se cerró antes de que acabara esto. {porque}")
        }
    };
    let parcial = sin_lo_vivo(parcial);
    if parcial.is_empty() { aviso } else { format!("{parcial}\n\n{aviso}") }
}

// ---------------------------------------------------------------------------
// Al arrancar

/// Lo que se retoma al arrancar: las respuestas cortadas (ya reescritas) y
/// los turnos que hay que volver a lanzar.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RetomadoIa {
    /// El chat.
    pub chat: String,
    /// La burbuja cortada, como quedo (para cambiarla en lo cargado).
    pub cortada: MensajeChat,
    /// La respuesta nueva que nace debajo, si se vuelve a lanzar
    /// (`ia_turno_en_chat` con ella).
    pub otra_vez: Option<InicioIa>,
}

#[uniffi::export]
impl Nucleo {
    /// **Lo que quedo a medias la ultima vez** que se cerro PixPin con el
    /// asistente trabajando: cada burbuja cortada dice que paso y, lo que se
    /// puede, nace otra vez debajo para que Swift lo lance. Se llama una vez
    /// al arrancar, fuera del hilo principal (mira el motor de `ajustes`).
    pub fn ia_retomar_al_arrancar(&self, ajustes: AjustesIa) -> Vec<RetomadoIa> {
        let cortados = tomar_apuntados(self.ruta());
        let ahora = crate::ahora_ms();
        cortados.iter().filter_map(|e| self.retomar_uno(&ajustes, e, ahora)).collect()
    }
}

impl Nucleo {
    /// Uno: reescribe su burbuja y, si toca, nace la respuesta nueva.
    fn retomar_uno(&self, ajustes: &AjustesIa, e: &EnMarcha, ahora: i64) -> Option<RetomadoIa> {
        let carpeta = almacen::carpeta(self.ruta(), &e.proyecto);
        let mut mensajes = cuaderno::Cuaderno::leer_de(&carpeta).ok()?.mensajes;
        mensajes.sort_by_key(|m| m.cuando);
        // Sin su burbuja (el usuario la borro) no hay nada que retomar.
        let parcial = mensajes.iter().find(|m| m.id == e.nota)?.texto.clone();
        let pregunta = mensajes.iter().find(|m| m.id == e.pregunta).and_then(|m| ia_chat::pregunta_de(m, e.asistente));
        let mut decision = que_hacer(e, ahora, pregunta.is_some());
        if decision == Retomar::OtraVez
            && let Err(err) = super::turno::motor_del_turno(ajustes, pregunta.as_deref().unwrap_or_default(), false)
        {
            decision = Retomar::Avisar(Motivo::NoSePudo(match err {
                crate::ErrorNucleo::Asistente { detalle } => detalle,
                otro => otro.to_string(),
            }));
        }
        let texto = texto_cortado(&parcial, &decision, e.asistente);
        let cortada = self
            .cambiar_nota(&e.proyecto, &e.nota, |m| {
                m.texto = texto.clone();
                true
            })
            .ok()??;
        let mut otra_vez = None;
        if decision == Retomar::OtraVez {
            // La respuesta nueva, debajo (en otro chat, colgada de la pregunta).
            let vivo = ia_chat::EnVivo {
                estado: Some(ia_chat::ESTADO_INICIAL.into()),
                ..Default::default()
            };
            let responde_a = (!e.asistente).then(|| e.pregunta.clone());
            if let Ok(n) = self.escribir_nota_con(e.proyecto.clone(), vivo.mostrar(), responde_a, |n| {
                n.aparato = Some(ia_chat::APARATO.into());
            }) {
                RETOMADOS
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get_or_insert_with(HashMap::new)
                    .insert(n.id.clone(), e.intentos + 1);
                otra_vez = Some(InicioIa {
                    respuesta: MensajeChat::from(n),
                    pregunta_id: e.pregunta.clone(),
                });
            }
        }
        Some(RetomadoIa {
            chat: e.proyecto.clone(),
            cortada: MensajeChat::from(cortada),
            otra_vez,
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn en_marcha(nota: &str) -> EnMarcha {
        EnMarcha {
            proyecto: "p1".into(),
            nota: nota.into(),
            pregunta: "q1".into(),
            asistente: true,
            desde: 1_000,
            actuo: false,
            intentos: 0,
        }
    }

    /// Se apunta al empezar, se marca al actuar y se borra al acabar; lo que
    /// queda se coge una vez (el fichero se vacia) y sin nada no hay fichero.
    #[test]
    fn el_fichero_apunta_marca_y_se_vacia() {
        let raiz = std::env::temp_dir().join(format!("pixpin-de-fondo-mac-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        apuntar(&raiz, en_marcha("a"));
        apuntar(&raiz, en_marcha("b"));
        // La misma nota otra vez no se duplica.
        apuntar(&raiz, en_marcha("b"));
        marcar_que_actuo(&raiz, "b");
        let v = leer(&raiz);
        assert_eq!(v.len(), 2);
        assert!(!v[0].actuo && v[1].actuo, "{v:?}");
        quitar(&raiz, "a");
        assert_eq!(leer(&raiz).iter().map(|e| e.nota.as_str()).collect::<Vec<_>>(), ["b"]);
        assert_eq!(tomar_apuntados(&raiz).len(), 1);
        assert!(tomar_apuntados(&raiz).is_empty());
        assert!(!raiz.join(FICHERO).exists(), "sin nada a medias no queda fichero");
        // Uno roto no tumba nada: se ignora.
        std::fs::write(raiz.join(FICHERO), "{roto").unwrap();
        assert!(leer(&raiz).is_empty());
        // El formato de Windows se lee tal cual.
        std::fs::write(
            raiz.join(FICHERO),
            r#"{"version":1,"turnos":[{"proyecto":"p","nota":"n","pregunta":"q","desde":5}]}"#,
        )
        .unwrap();
        assert_eq!(leer(&raiz)[0].nota, "n");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Solo se repite solo lo que no hizo nada, no se habia retomado, es
    /// reciente y conserva su pregunta.
    #[test]
    fn la_regla_de_retomar() {
        let e = en_marcha("a");
        assert_eq!(que_hacer(&e, 2_000, true), Retomar::OtraVez);
        assert_eq!(que_hacer(&e, 2_000, false), Retomar::Avisar(Motivo::SinPregunta));
        assert_eq!(que_hacer(&EnMarcha { actuo: true, ..e.clone() }, 2_000, true), Retomar::Avisar(Motivo::YaActuo));
        assert_eq!(que_hacer(&EnMarcha { intentos: 1, ..e.clone() }, 2_000, true), Retomar::Avisar(Motivo::YaSeRetomo));
        assert_eq!(que_hacer(&e, 1_000 + RETOMAR_HASTA_MS + 1, true), Retomar::Avisar(Motivo::Viejo));
    }

    /// La burbuja cortada guarda lo escrito sin lo de en vivo y dice que
    /// paso y como seguir (con `@asistente` fuera de su chat).
    #[test]
    fn la_burbuja_cortada_dice_que_paso() {
        assert_eq!(sin_lo_vivo("💭 Pensando…"), "");
        assert_eq!(sin_lo_vivo("Hola, voy a ▍"), "Hola, voy a");
        assert_eq!(sin_lo_vivo("Primero esto.\n✏️ dibujando en el lienzo…"), "Primero esto.");
        // Un texto suyo que acaba en puntos suspensivos se queda.
        assert_eq!(sin_lo_vivo("Y entonces…"), "Y entonces…");
        let t = texto_cortado("💭 Pensando…", &Retomar::OtraVez, true);
        assert!(t.starts_with("↻ PixPin se cerró"), "{t}");
        let t = texto_cortado("Ya apunte dos tareas ▍", &Retomar::Avisar(Motivo::YaActuo), true);
        assert!(t.starts_with("Ya apunte dos tareas\n\n⚠️ PixPin se cerró"), "{t}");
        assert!(t.contains("«continúa»") && !t.contains("@asistente"), "{t}");
        let t = texto_cortado("", &Retomar::Avisar(Motivo::Viejo), false);
        assert!(t.contains("«@asistente continúa»"), "{t}");
        let t = texto_cortado("", &Retomar::Avisar(Motivo::NoSePudo("falta la clave".into())), true);
        assert!(t.ends_with("No he podido retomarlo: falta la clave"), "{t}");
    }

    /// Al arrancar, lo cortado se reescribe en su burbuja: lo que ya actuo
    /// no se repite, y sin motor se dice por que no se retoma. El fichero
    /// queda vacio.
    #[test]
    fn al_arrancar_las_burbujas_cortadas_dicen_que_paso() {
        let d = std::env::temp_dir().join(format!("pixpin-retomar-mac-{}-{}", std::process::id(), crate::ahora_ms()));
        std::fs::create_dir_all(&d).unwrap();
        let n = Nucleo::abrir(d.display().to_string(), "Mac de pruebas".into()).unwrap();
        let chat = n.chat_asistente().unwrap().id;
        let apuntar_uno = |pedido: &str, actuo: bool| {
            let p = n.escribir_nota(chat.clone(), pedido.into(), None).unwrap();
            let r = n.ia_al_enviar(chat.clone(), p.id.clone()).unwrap().unwrap().respuesta;
            n.cambiar_nota(&chat, &r.id, |m| {
                m.texto = "Voy apuntando ▍".into();
                true
            })
            .unwrap();
            apuntar(
                n.ruta(),
                EnMarcha {
                    proyecto: chat.clone(),
                    nota: r.id.clone(),
                    pregunta: p.id,
                    asistente: true,
                    desde: crate::ahora_ms(),
                    actuo,
                    intentos: 0,
                },
            );
            r.id
        };
        let hecho = apuntar_uno("apunta pan", true);
        let sin_motor = apuntar_uno("dibuja una casa", false);
        let ajustes = AjustesIa {
            motor: String::new(),
            modelo: String::new(),
            pensar: None,
            respaldo_local: false,
            mejor_vista: false,
            documentos_a_la_nube: String::new(),
            carpeta_modelos: String::new(),
            puerto_usuario: 0,
        };
        let r = n.ia_retomar_al_arrancar(ajustes.clone());
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|x| x.otra_vez.is_none()), "nada se relanza");
        let de = |id: &str| r.iter().find(|x| x.cortada.id == id).unwrap().cortada.texto.clone();
        let t = de(&hecho);
        assert!(t.starts_with("Voy apuntando\n\n⚠️ PixPin se cerró") && t.contains("no lo repito"), "{t}");
        assert!(de(&sin_motor).contains("No he podido retomarlo"), "{}", de(&sin_motor));
        // Lo de disco ya se cogio: la segunda vez no hay nada.
        assert!(n.ia_retomar_al_arrancar(ajustes).is_empty());
        let _ = std::fs::remove_dir_all(d);
    }
}
