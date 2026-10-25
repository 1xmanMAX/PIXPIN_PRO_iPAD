//! **Recordatorios**: «a las diez, recuerdame esto» (tanda A4), lo de
//! `recordatorios.rs` de Windows sin el hilo.
//!
//! ## Donde vive el dato: en el mensaje, y en ninguna parte mas
//!
//! El movil lo guarda en UN campo, `Mensaje.recuerdaEn: Long?`
//! (`guardados/Mensajes.kt:283`), milisegundos UTC desde 1970. **No hay
//! fichero de alarmas**, ni alli ni en Windows ni aqui: por eso un
//! recordatorio puesto en el movil sale puesto en el Mac en cuanto se
//! sincroniza. El `Mensaje` de Rust no declara ese campo, asi que viaja en
//! `resto` (`#[serde(flatten)]`) y se lee y escribe con [`hora_de`],
//! [`poner_hora`] y [`quitar_hora`]: el nombre esta escrito en un unico
//! sitio, [`CAMPO`].
//!
//! Las tareas llevan su hora dentro del texto (`⏰ AAAA-MM-DD HH:MM`, hora
//! local: [`crate::tareas::RELOJ`]), y su id de agenda es
//! `tarea:<lista>:<hora local>` (el de Windows).
//!
//! ## Quien vigila
//!
//! En Windows, un hilo dormido hasta la hora (`Vigia`). Aqui **Rust no
//! arranca hilos** (ver `lib.rs`): el vigia es de Swift
//! (`Recordatorios/Vigia.swift`, temporizadores con la hora de la pared y
//! notificaciones del sistema para cuando la app esta cerrada). Rust dice
//! **que** hay puesto ([`Nucleo::recordatorios`], al arrancar y tras
//! sincronizar, el `releer` de Windows) y **escribe** lo que pasa al sonar:
//! [`Nucleo::olvidar_recordatorio`] (Android hace `recuerdaEn = null` nada
//! mas avisar, `RecordatorioReceiver.kt:58`; si no, sonaria otra vez al
//! siguiente arranque), «+10 min» y «Hecha».
//!
//! ## Rendimiento
//!
//! Hay que mirar todos los cuadernos, pero una linea que no nombra
//! `recuerdaEn` ni es una lista con `⏰` se salta sin leerla como JSON.

use std::path::Path;

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Mensaje};
use pixpin_proyecto::mini;

use crate::chats::validar_chat;
use crate::tareas::{
    RELOJ, crudo_con_hora, de_id_de_recordatorio, es_lista, hora_de_tarea,
    id_de_recordatorio, puede_ser_lista, sin_marca,
};
use crate::{ErrorNucleo, Nucleo};

/// El nombre del campo en el JSONL. Es el de Android y **no se traduce ni se
/// renombra**: es un dato guardado que ya viaja entre aparatos.
pub const CAMPO: &str = "recuerdaEn";

/// Lo que se pospone con «+10 min» (`notificaciones::POSPONER_MIN`).
pub const POSPONER_MIN: u32 = 10;

/// La tarea de un recordatorio: su lista y su numero, para «Hecha» y
/// «+10 min» de la notificacion (`tareas::tarea_de_recordatorio`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TareaRecordada {
    /// El id del mensaje de la lista.
    pub lista: String,
    /// Su numero en el documento (desde 0).
    pub indice: u32,
}

/// Un recordatorio puesto, de un mensaje o de una tarea.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RecordatorioMac {
    /// El chat (la carpeta del proyecto).
    pub chat: String,
    pub nombre_chat: String,
    /// El id de agenda: el del mensaje, o `tarea:<lista>:<hora local>`.
    pub id: String,
    /// Cuando suena, en ms **UTC** desde 1970.
    pub cuando_utc_ms: i64,
    /// Lo que se ensena al sonar (`Mensaje::resumen`, o `☑ <tarea>`).
    pub texto: String,
    /// Si es de una tarea, cual.
    pub tarea: Option<TareaRecordada>,
}

/// La hora que tiene puesta un mensaje, si tiene alguna.
///
/// Se descarta lo que no sea un entero: el campo lo escribe otro aparato y
/// una cadena o un `null` ahi significan «no hay hora», no un fallo.
pub fn hora_de(m: &Mensaje) -> Option<i64> {
    m.resto.get(CAMPO)?.as_i64()
}

/// Le pone (o le cambia) la hora a un mensaje. No escribe en disco.
pub fn poner_hora(m: &mut Mensaje, cuando_utc_ms: i64) {
    m.resto
        .insert(CAMPO.to_string(), serde_json::Value::from(cuando_utc_ms));
}

/// Le quita la hora.
///
/// Se **borra la clave** en vez de dejarla a `null` para que el mensaje
/// vuelva a ser byte a byte lo que era: el resumen de sincronizacion es un
/// hash del JSON canonico, y un `"recuerdaEn": null` colgando haria viajar
/// el mensaje entero en cada vuelta.
pub fn quitar_hora(m: &mut Mensaje) -> bool {
    m.resto.remove(CAMPO).is_some()
}

/// El recordatorio de un mensaje, si lo tiene (sin el chat).
fn de_un_mensaje(m: &Mensaje) -> Option<(String, i64, String)> {
    Some((m.id.clone(), hora_de(m)?, m.resumen()))
}

/// Los recordatorios de las tareas pendientes de una lista
/// (`tareas::avisos_del_cuaderno`): id, hora UTC, texto y numero.
fn de_una_lista(m: &Mensaje, desfase_local_ms: i64) -> Vec<(String, i64, String, u32)> {
    mini::leer_tareas(&m.texto)
        .into_iter()
        .enumerate()
        .filter(|(_, t)| !t.hecha)
        .filter_map(|(i, t)| {
            let local = hora_de_tarea(&t.texto)?;
            let (visible, _) = mini::partir(&t.texto);
            let (texto, _) = mini::imagenes_de(visible);
            Some((
                id_de_recordatorio(&m.id, local),
                local - desfase_local_ms,
                format!("☑ {}", sin_marca(&texto).trim()),
                i as u32,
            ))
        })
        .collect()
}

/// Una linea que puede llevar un recordatorio. Lo que no, se salta sin
/// leerla como JSON.
fn puede_recordar(linea: &str) -> bool {
    linea.contains(CAMPO) || (puede_ser_lista(linea) && linea.contains(RELOJ))
}

/// Los recordatorios de un cuaderno, en el orden del fichero.
fn del_cuaderno(
    fichero: &Path,
    ficha: &almacen::Ficha,
    desfase_local_ms: i64,
) -> std::io::Result<Vec<RecordatorioMac>> {
    let texto = match std::fs::read_to_string(fichero) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut v = Vec::new();
    for linea in texto.lines().filter(|l| puede_recordar(l)) {
        let Ok(m) = serde_json::from_str::<Mensaje>(linea) else {
            continue;
        };
        // Lo que esta en el buzon aun no es del chat (ni suena).
        if m.en_buzon {
            continue;
        }
        if let Some((id, cuando, texto)) = de_un_mensaje(&m) {
            v.push(RecordatorioMac {
                chat: ficha.id.clone(),
                nombre_chat: ficha.nombre.clone(),
                id,
                cuando_utc_ms: cuando,
                texto,
                tarea: None,
            });
        }
        if es_lista(&m) {
            for (id, cuando, texto, indice) in de_una_lista(&m, desfase_local_ms) {
                v.push(RecordatorioMac {
                    chat: ficha.id.clone(),
                    nombre_chat: ficha.nombre.clone(),
                    id,
                    cuando_utc_ms: cuando,
                    texto,
                    tarea: Some(TareaRecordada {
                        lista: m.id.clone(),
                        indice,
                    }),
                });
            }
        }
    }
    Ok(v)
}

/// El orden de la agenda: del mas proximo al mas lejano; a la misma hora,
/// por chat e id (`Agenda::programar` de Windows).
fn ordenar(v: &mut [RecordatorioMac]) {
    v.sort_by(|a, b| {
        a.cuando_utc_ms
            .cmp(&b.cuando_utc_ms)
            .then_with(|| a.chat.cmp(&b.chat))
            .then_with(|| a.id.cmp(&b.id))
    });
}

/// Quita del texto de la tarea pendiente de `lista` la marca de la hora
/// `local` (`tareas::olvidar_hora`). `false` si la lista o la marca ya no
/// estan.
fn olvidar_hora_de_tarea(carpeta: &Path, lista: &str, local: i64) -> std::io::Result<bool> {
    let mut quitada = false;
    let r = cuaderno::cambiar(carpeta, lista, |m| {
        if !es_lista(m) {
            return false;
        }
        let mut tareas_doc = mini::leer_tareas(&m.texto);
        for t in tareas_doc.iter_mut() {
            if hora_de_tarea(&t.texto) == Some(local) {
                t.texto = crudo_con_hora(&t.texto, None);
                quitada = true;
            }
        }
        if quitada {
            m.texto = mini::escribir_tareas(&mini::titulo(&m.texto), &tareas_doc);
        }
        quitada
    });
    match r {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
        Ok(_) => Ok(quitada),
    }
}

impl Nucleo {
    pub(crate) fn carpeta_de_chat(&self, chat: &str) -> Result<std::path::PathBuf, ErrorNucleo> {
        validar_chat(chat)?;
        Ok(almacen::carpeta(self.ruta(), chat))
    }

    /// Suelta el indice de paginas de un cuaderno reescrito (puede quedar
    /// del mismo tamano y la fecha no bastar).
    pub(crate) fn soltar_cuaderno(&self, carpeta: &Path) {
        self.cache_tomada().quitar(&carpeta.join("guardados.jsonl"));
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Todo lo que hay puesto para recordar**, en todos los chats: los
    /// mensajes con `recuerdaEn` y las tareas pendientes con `⏰`, del mas
    /// proximo al mas lejano (tambien lo que ya paso: suena al arrancar,
    /// como en Windows, «llegar tarde es mejor que no llegar»).
    /// `desfase_local_ms` es el huso de este Mac (las horas de las tareas
    /// son locales). Un cuaderno que no se puede leer no deja sin los demas.
    pub fn recordatorios(&self, desfase_local_ms: i64) -> Result<Vec<RecordatorioMac>, ErrorNucleo> {
        let indice = Indice::leer_si_esta(self.ruta())?;
        let mut v = Vec::new();
        for ficha in &indice.proyectos {
            if validar_chat(&ficha.id).is_err() {
                continue;
            }
            let fichero = almacen::carpeta(self.ruta(), &ficha.id).join("guardados.jsonl");
            if let Ok(suyos) = del_cuaderno(&fichero, ficha, desfase_local_ms) {
                v.extend(suyos);
            }
        }
        ordenar(&mut v);
        Ok(v)
    }

    /// **Le quita la hora** al recordatorio `id` del chat `chat` (al sonar,
    /// o «Quitar el recordatorio»): `recuerdaEn` fuera del mensaje, o la
    /// marca `⏰` fuera del texto de la tarea. `false` si ya no estaba.
    pub fn olvidar_recordatorio(&self, chat: String, id: String) -> Result<bool, ErrorNucleo> {
        let carpeta = self.carpeta_de_chat(&chat)?;
        let quitada = match de_id_de_recordatorio(&id) {
            Some((lista, local)) => olvidar_hora_de_tarea(&carpeta, lista, local)?,
            None => {
                let mut habia = false;
                match cuaderno::cambiar(&carpeta, &id, |m| {
                    habia = quitar_hora(m);
                    habia
                }) {
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    otro => {
                        otro?;
                    }
                }
                habia
            }
        };
        if quitada {
            self.soltar_cuaderno(&carpeta);
        }
        Ok(quitada)
    }

    /// **Pone (o cambia) la hora de un mensaje** a `cuando_utc_ms`: lo de
    /// «+10 min» y «Volver a llamar en» (`volver_a_poner` de Windows). Se
    /// escribe en el cuaderno, que es lo que viaja y sobrevive a cerrar la
    /// app. `false` si el mensaje ya no esta.
    pub fn volver_a_recordar(
        &self,
        chat: String,
        mensaje: String,
        cuando_utc_ms: i64,
    ) -> Result<bool, ErrorNucleo> {
        let carpeta = self.carpeta_de_chat(&chat)?;
        let r = match cuaderno::cambiar(&carpeta, &mensaje, |m| {
            if hora_de(m) == Some(cuando_utc_ms) {
                return false;
            }
            poner_hora(m, cuando_utc_ms);
            true
        }) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            otro => otro?,
        };
        self.soltar_cuaderno(&carpeta);
        Ok(r.is_some())
    }

    /// **Pone, cambia o quita la hora de una tarea** (`cuando_local_ms`, la
    /// hora del reloj de la pared; `None` la quita): la marca `⏰` en su
    /// texto, como «Recordarmelo» de la ventana de Tareas de Windows y el
    /// «+10 min» de su notificacion. Con `crudo` (el que se ve en pantalla,
    /// [`crate::Tarea::crudo`]) se comprueba antes que la tarea numero
    /// `indice` sigue siendo esa; sin el (la notificacion, que ya le quito
    /// la hora al sonar), basta el numero, como el pedido de Windows.
    /// `false` si la lista o la tarea ya no estan: hay que releer.
    pub fn poner_hora_tarea(
        &self,
        chat: String,
        lista: String,
        indice: u32,
        crudo: Option<String>,
        cuando_local_ms: Option<i64>,
    ) -> Result<bool, ErrorNucleo> {
        let carpeta = self.carpeta_de_chat(&chat)?;
        let mut sigue = false;
        let r = cuaderno::cambiar(&carpeta, &lista, |m| {
            if !es_lista(m) {
                return false;
            }
            let mut doc = mini::leer_tareas(&m.texto);
            let Some(t) = doc.get_mut(indice as usize) else {
                return false;
            };
            if crudo.as_ref().is_some_and(|c| *c != t.texto) {
                return false;
            }
            sigue = true;
            let nuevo = crudo_con_hora(&t.texto, cuando_local_ms);
            if nuevo == t.texto {
                return false;
            }
            t.texto = nuevo;
            m.texto = mini::escribir_tareas(&mini::titulo(&m.texto), &doc);
            true
        });
        match r {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            otro => {
                otro?;
            }
        }
        self.soltar_cuaderno(&carpeta);
        Ok(sigue)
    }

    /// **«Hecha»** de la notificacion de una tarea: la tacha por su numero
    /// (el pedido `marcar_tarea` de Windows), sin comprobar su texto, que
    /// cambio al quitarle la hora. `false` si ya no esta o ya estaba hecha.
    pub fn tachar_tarea_recordada(
        &self,
        chat: String,
        lista: String,
        indice: u32,
    ) -> Result<bool, ErrorNucleo> {
        let carpeta = self.carpeta_de_chat(&chat)?;
        let mut tachada = false;
        let r = cuaderno::cambiar(&carpeta, &lista, |m| {
            if !es_lista(m) {
                return false;
            }
            match mini::leer_tareas(&m.texto).get(indice as usize) {
                Some(t) if !t.hecha => {
                    m.texto = mini::alternar(&m.texto, indice as usize);
                    tachada = true;
                    true
                }
                _ => false,
            }
        });
        match r {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            otro => {
                otro?;
            }
        }
        if tachada {
            self.soltar_cuaderno(&carpeta);
        }
        Ok(tachada)
    }

    /// **La posicion de un mensaje en su chat** (0 el mas antiguo, la de
    /// [`Nucleo::mensajes`]), para «Abrir» un recordatorio en su sitio.
    /// `None` si ya no esta.
    pub fn posicion_de_mensaje(&self, chat: String, mensaje: String) -> Result<Option<u64>, ErrorNucleo> {
        /// Lo minimo de una linea para contar como el indice de paginas.
        #[derive(serde::Deserialize, Default)]
        #[serde(default)]
        struct Cabeza {
            id: String,
        }
        let carpeta = self.carpeta_de_chat(&chat)?;
        let texto = match std::fs::read_to_string(carpeta.join("guardados.jsonl")) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut posicion = 0u64;
        for linea in texto.lines().filter(|l| !l.trim().is_empty()) {
            let Ok(c) = serde_json::from_str::<Cabeza>(linea) else {
                continue;
            };
            if c.id.starts_with(cuaderno::PREFIJO_DE_SISTEMA) {
                continue;
            }
            if c.id == mensaje {
                return Ok(Some(posicion));
            }
            posicion += 1;
        }
        Ok(None)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn nota(id: &str, cuando: Option<i64>, texto: &str) -> Mensaje {
        let mut m = Mensaje {
            id: id.into(),
            texto: texto.into(),
            ..Default::default()
        };
        if let Some(t) = cuando {
            poner_hora(&mut m, t);
        }
        m
    }

    #[test]
    fn la_hora_viaja_en_el_campo_que_escribe_el_movil() {
        let m = nota("m1", Some(1_758_351_600_000), "llamar al arquitecto");
        let linea = serde_json::to_string(&m).unwrap();
        assert!(
            linea.contains("\"recuerdaEn\":1758351600000"),
            "el movil lee ese nombre exacto y un entero, no una cadena: {linea}"
        );
        let vuelta: Mensaje = serde_json::from_str(&linea).unwrap();
        assert_eq!(hora_de(&vuelta), Some(1_758_351_600_000));
    }

    #[test]
    fn quitar_la_hora_deja_el_mensaje_como_estaba_y_no_un_nulo_colgando() {
        let limpio = serde_json::to_string(&nota("m1", None, "hola")).unwrap();
        let mut m = nota("m1", Some(7), "hola");
        assert!(quitar_hora(&mut m));
        assert_eq!(serde_json::to_string(&m).unwrap(), limpio);
        assert!(!quitar_hora(&mut m), "ya no habia nada que quitar");
    }

    #[test]
    fn un_recuerdaen_que_no_es_un_numero_se_lee_como_si_no_hubiera_hora() {
        let mut m = nota("m1", None, "hola");
        m.resto
            .insert(CAMPO.into(), serde_json::Value::String("manana".into()));
        assert_eq!(hora_de(&m), None, "una cadena ahi no es una hora");
        assert!(de_un_mensaje(&m).is_none());
        m.resto.insert(CAMPO.into(), serde_json::Value::Null);
        assert_eq!(hora_de(&m), None, "un nulo tampoco");
    }

    #[test]
    fn solo_se_leen_las_lineas_que_pueden_recordar() {
        assert!(puede_recordar(r#"{"id":"1","recuerdaEn":5}"#));
        assert!(puede_recordar(
            r#"{"id":"1","miniapp":"tareas","texto":"- [ ] a ⏰ 2026-10-09 10:00"}"#
        ));
        assert!(!puede_recordar(
            r#"{"id":"1","miniapp":"tareas","texto":"- [ ] a"}"#
        ));
        assert!(!puede_recordar(r#"{"id":"1","texto":"⏰ suelto"}"#));
    }

    #[test]
    fn la_agenda_va_del_mas_proximo_al_mas_lejano() {
        let r = |chat: &str, id: &str, t| RecordatorioMac {
            chat: chat.into(),
            nombre_chat: String::new(),
            id: id.into(),
            cuando_utc_ms: t,
            texto: String::new(),
            tarea: None,
        };
        let mut v = vec![r("a", "tarde", 9_000), r("b", "pronto", 1_000), r("a", "otro", 9_000)];
        ordenar(&mut v);
        let ids: Vec<&str> = v.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["pronto", "otro", "tarde"]);
    }
}
