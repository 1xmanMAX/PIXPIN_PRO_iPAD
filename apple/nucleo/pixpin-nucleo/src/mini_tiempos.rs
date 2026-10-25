//! **Los relojes de las mini-apps** (cronometro, temporizador y alarma) para
//! los pines de herramienta del Mac.
//!
//! Como en [`crate::miniapps`], no hay formato nuevo: el documento es el de
//! `pixpin_proyecto::mini::tiempos` (lineas `- clave: valor`, el mismo texto
//! que escriben Windows y el movil) y cada toque es **documento entra,
//! documento sale**. Lo que se ve y lo que hace cada boton copia el panel de
//! Windows (`apps/pixpin/src/mini_panel.rs`: `de_cronometro`,
//! `temporizador`, `alarma`, `duracion_tecleada`, `hora_tecleada`,
//! `cuando_avisa`), que a su vez copia `MiniActivity.kt` del movil.
//!
//! ## El reloj entra por parametro
//!
//! Lo que se guarda es **el instante** (cuando arranco, cuando acaba), no el
//! numero que se ve: el numero se calcula al mirarlo con `ahora_ms`, y por
//! eso es correcto aunque el Mac se haya dormido entre medias. Swift pasa la
//! hora de la pared (`Date()`), nunca una cuenta de ticks.
//!
//! ## Los avisos
//!
//! Un temporizador en marcha o una alarma encendida tienen que sonar aunque
//! su pin este cerrado: [`mini_cuando_avisa`] dice cuando (lo de Windows al
//! tocarla) y [`Nucleo::avisos_miniapps`] los de todos los chats (lo de
//! Windows al arrancar y tras sincronizar, `recordatorios::agenda_de`).

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_proyecto::mini::{self, tiempos};

use crate::{ErrorNucleo, Nucleo};

const DIA: i64 = 86_400_000;

// ---- Lo que se ve -------------------------------------------------------------

/// Un cronometro mirado en un instante.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CronometroMini {
    pub corriendo: bool,
    /// Lo que lleva en ese instante, en milisegundos.
    pub transcurrido: i64,
    /// El numero grande, ya escrito (`1:05,3`, `12:04:05`).
    pub tablero: String,
    /// Las vueltas en el orden del documento (la primera, la primera),
    /// ya escritas. El pin las ensena del reves, la ultima arriba.
    pub vueltas: Vec<String>,
    /// Si «Reiniciar» tiene algo que reiniciar.
    pub se_reinicia: bool,
}

/// Un temporizador mirado en un instante.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TemporizadorMini {
    pub corriendo: bool,
    /// Lo que dura, en milisegundos.
    pub duracion: i64,
    /// Lo que falta (cero si ya paso su hora).
    pub restante: i64,
    /// Cuando suena, en milisegundos UTC, si esta en marcha.
    pub fin_en: Option<i64>,
    /// La cuenta atras sin decimas (`4:59`).
    pub tablero: String,
    /// Llego a cero estando en marcha: el tablero en rojo.
    pub vencido: bool,
}

/// Una alarma.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AlarmaMini {
    pub hora: u8,
    pub minuto: u8,
    pub activa: bool,
    /// `7:05`: la hora sin rellenar y el minuto con dos cifras, como el movil.
    pub tablero: String,
}

/// Una hora tecleada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct HoraMini {
    pub hora: u8,
    pub minuto: u8,
}

/// Lo que se le hace a un cronometro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccionCronometro {
    /// El boton grande.
    ArrancarOParar,
    Vuelta,
    Reiniciar,
}

/// Lo que se le hace a un temporizador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccionTemporizador {
    /// El boton grande.
    ArrancarOParar,
    /// Sumar o restar minutos a lo que dura. En marcha no hace nada.
    Minutos { minutos: i64 },
    /// Lo que dura, tecleado de una vez. En marcha no hace nada.
    Duracion { ms: i64 },
    /// Ponerlo en esos minutos y en marcha desde ya, este como este (la
    /// «pegatina de tiempo» de Windows, `arrancar_temporizador`).
    EnMarchaCon { minutos: i64 },
}

/// Lo que se le hace a una alarma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccionAlarma {
    /// Encenderla o apagarla.
    Activar,
    /// Mover la hora (de las 0 hacia atras se va a las 23).
    Horas { horas: i64 },
    /// Mover el minuto (sin tocar la hora, como el movil).
    Minutos { minutos: i64 },
    /// La hora tecleada: se pone y **se enciende**.
    Hora { hora: u8, minuto: u8 },
}

// ---- Cronometro ---------------------------------------------------------------

#[uniffi::export]
pub fn mini_cronometro_leer(documento: String, ahora_ms: i64) -> CronometroMini {
    let c = tiempos::leer_cronometro(&documento);
    CronometroMini {
        corriendo: c.corriendo(),
        transcurrido: c.transcurrido(ahora_ms),
        tablero: tiempos::como_se_lee(c.transcurrido(ahora_ms)),
        vueltas: c.vueltas.iter().map(|v| tiempos::como_se_lee(*v)).collect(),
        se_reinicia: c.llevado > 0 || c.corriendo() || !c.vueltas.is_empty(),
    }
}

#[uniffi::export]
pub fn mini_cronometro_cambiar(
    documento: String,
    accion: AccionCronometro,
    ahora_ms: i64,
) -> String {
    let c = tiempos::leer_cronometro(&documento);
    let nuevo = match accion {
        AccionCronometro::ArrancarOParar if c.corriendo() => tiempos::parar(&c, ahora_ms),
        AccionCronometro::ArrancarOParar => tiempos::arrancar(&c, ahora_ms),
        AccionCronometro::Vuelta => tiempos::vuelta(&c, ahora_ms),
        AccionCronometro::Reiniciar => tiempos::reiniciar(&c),
    };
    tiempos::escribir_cronometro(&mini::titulo(&documento), &nuevo)
}

// ---- Temporizador -------------------------------------------------------------

#[uniffi::export]
pub fn mini_temporizador_leer(documento: String, ahora_ms: i64) -> TemporizadorMini {
    let t = tiempos::leer_temporizador(&documento);
    TemporizadorMini {
        corriendo: t.corriendo(),
        duracion: t.duracion,
        restante: t.restante(ahora_ms),
        fin_en: t.fin_en,
        // Sin decimas, como el movil (`comoSeLeeCorto`).
        tablero: tiempos::como_se_lee_corto(t.restante(ahora_ms)),
        vencido: t.vencido(ahora_ms),
    }
}

/// El documento tras un toque. Con el temporizador en marcha no se cambia lo
/// que dura (Windows y el movil apagan esos botones): a mitad de cuenta
/// nadie sabe si «+5» adelanta el final o lo aplaza.
#[uniffi::export]
pub fn mini_temporizador_cambiar(
    documento: String,
    accion: AccionTemporizador,
    ahora_ms: i64,
) -> String {
    let t = tiempos::leer_temporizador(&documento);
    let nuevo = match accion {
        AccionTemporizador::ArrancarOParar if t.corriendo() => tiempos::detener(&t),
        AccionTemporizador::ArrancarOParar => tiempos::lanzar(&t, ahora_ms),
        AccionTemporizador::Minutos { .. } | AccionTemporizador::Duracion { .. }
            if t.corriendo() =>
        {
            return documento;
        }
        AccionTemporizador::Minutos { minutos } => tiempos::con_duracion(
            &t,
            t.duracion.saturating_add(minutos.saturating_mul(60_000)),
            ahora_ms,
        ),
        AccionTemporizador::Duracion { ms } => tiempos::con_duracion(&t, ms, ahora_ms),
        AccionTemporizador::EnMarchaCon { minutos } => tiempos::lanzar(
            &tiempos::con_duracion(
                &tiempos::detener(&t),
                minutos.saturating_mul(60_000),
                ahora_ms,
            ),
            ahora_ms,
        ),
    };
    tiempos::escribir_temporizador(&mini::titulo(&documento), &nuevo)
}

/// Lo que dura un temporizador tecleado, en milisegundos
/// (`mini_panel::duracion_tecleada` de Windows).
///
/// Un numero solo son **minutos** («7», «2,5»); con dos puntos es `m:ss`
/// («1:30») y con dos pares `h:mm:ss`. Nada si no es un tiempo (letras,
/// «1:75», cero): la caja se queda con lo escrito para corregirlo.
#[uniffi::export]
pub fn mini_duracion_tecleada(texto: String) -> Option<i64> {
    let limpio = texto.trim();
    if limpio.is_empty() || !limpio.is_ascii() {
        return None;
    }
    let ms = if limpio.contains(':') {
        let partes: Vec<&str> = limpio.split(':').map(str::trim).collect();
        if partes.len() > 3
            || partes
                .iter()
                .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        let n: Vec<i64> = partes
            .iter()
            .map(|p| p.parse::<i64>().ok())
            .collect::<Option<_>>()?;
        // Lo de detras del primero son minutos o segundos: mas de 59 es un
        // error de tecleo, no una forma rara de escribir.
        if n.iter().skip(1).any(|v| *v > 59) {
            return None;
        }
        n.iter()
            .fold(0i64, |acc, v| acc.saturating_mul(60).saturating_add(*v))
            .saturating_mul(1000)
    } else {
        // Minutos, con decimales si se quiere («2,5» = dos y medio).
        let (enteros, fraccion) = limpio.split_once([',', '.']).unwrap_or((limpio, ""));
        if enteros.is_empty() && fraccion.is_empty() {
            return None;
        }
        let cifras = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
        if !cifras(enteros) || !cifras(fraccion) || enteros.len() > 6 {
            return None;
        }
        let e: i64 = if enteros.is_empty() {
            0
        } else {
            enteros.parse().ok()?
        };
        // Hasta la decima de minuto (seis segundos), como Windows.
        let decima = fraccion.bytes().next().map_or(0, |b| (b - b'0') as i64);
        e * 60_000 + decima * 6_000
    };
    (ms > 0).then_some(ms.min(tiempos::TOPE_DE_DURACION))
}

// ---- Alarma -------------------------------------------------------------------

#[uniffi::export]
pub fn mini_alarma_leer(documento: String) -> AlarmaMini {
    let a = tiempos::leer_alarma(&documento);
    AlarmaMini {
        hora: a.hora,
        minuto: a.minuto,
        activa: a.activa,
        tablero: format!("{}:{:02}", a.hora, a.minuto),
    }
}

#[uniffi::export]
pub fn mini_alarma_cambiar(documento: String, accion: AccionAlarma) -> String {
    let mut a = tiempos::leer_alarma(&documento);
    match accion {
        AccionAlarma::Activar => a.activa = !a.activa,
        // Teclear una hora es pedir que suene a esa hora: se enciende. Con los
        // botones no, porque alli se va de paso por horas que nadie pidio.
        AccionAlarma::Hora { hora, minuto } if hora < 24 && minuto < 60 => {
            a.hora = hora;
            a.minuto = minuto;
            a.activa = true;
        }
        AccionAlarma::Hora { .. } => return documento,
        // Aritmetica modular, como el movil (`MiniActivity.kt:544-555`).
        AccionAlarma::Horas { horas } => a.hora = (a.hora as i64 + horas).rem_euclid(24) as u8,
        AccionAlarma::Minutos { minutos } => {
            a.minuto = (a.minuto as i64 + minutos).rem_euclid(60) as u8
        }
    }
    tiempos::escribir_alarma(&mini::titulo(&documento), &a)
}

/// La hora tecleada para una alarma: `7:30`, `730`, `7.30`, `7h30`, `19` (en
/// punto). Es la lectura de «Elegir la hora» de Windows
/// (`recordatorios::hora_escrita`): las dos ultimas cifras son los minutos
/// solo si hay tres o cuatro. Nada si no es una hora de verdad.
#[uniffi::export]
pub fn mini_hora_tecleada(texto: String) -> Option<HoraMini> {
    let limpio = texto.trim();
    if !limpio.is_ascii() {
        return None;
    }
    let (h, m) = match limpio.split_once([':', '.', 'h', ' ']) {
        Some((h, m)) => (h.trim(), m.trim()),
        None if limpio.len() >= 3 => limpio.split_at(limpio.len() - 2),
        None => (limpio, "00"),
    };
    let cifras = |t: &str| {
        (!t.is_empty() && t.len() <= 2 && t.bytes().all(|b| b.is_ascii_digit()))
            .then(|| t.parse::<u8>().ok())
            .flatten()
    };
    let hora = cifras(h)?;
    // `18:` a medio escribir es las 18 en punto.
    let minuto = if m.is_empty() { 0 } else { cifras(m)? };
    (hora < 24 && minuto < 60).then_some(HoraMini { hora, minuto })
}

// ---- Los avisos ---------------------------------------------------------------

/// Lo que va delante del id del mensaje en el aviso de una mini-app: el
/// mismo prefijo que el movil y Windows (`mini_panel::PREFIJO_AVISO`).
pub const PREFIJO_AVISO: &str = "mini:";

/// Cuando tiene que avisar esta mini-app, en milisegundos UTC, o nada
/// (`mini_panel::cuando_avisa` de Windows).
///
/// - Temporizador en marcha: a su `finEn`, si aun no ha pasado (uno que ya
///   vencio no vuelve a avisar al releerlo).
/// - Alarma encendida: la proxima vez que el reloj de la pared marque su
///   hora, hoy o manana. `desfase_local_ms` es lo que hay que sumar a UTC
///   para tener la hora local.
#[uniffi::export]
pub fn mini_cuando_avisa(
    miniapp: String,
    documento: String,
    ahora_ms: i64,
    desfase_local_ms: i64,
) -> Option<i64> {
    cuando_avisa(&miniapp, &documento, ahora_ms, desfase_local_ms)
}

fn cuando_avisa(cual: &str, documento: &str, ahora: i64, desfase: i64) -> Option<i64> {
    match cual {
        mini::TEMPORIZADOR => tiempos::leer_temporizador(documento)
            .fin_en
            .filter(|f| *f > ahora),
        mini::ALARMA => {
            let a = tiempos::leer_alarma(documento);
            if !a.activa {
                return None;
            }
            let local = ahora + desfase;
            let mut cuando =
                local.div_euclid(DIA) * DIA + a.hora as i64 * 3_600_000 + a.minuto as i64 * 60_000;
            if cuando <= local {
                cuando += DIA;
            }
            Some(cuando - desfase)
        }
        _ => None,
    }
}

/// Un aviso pendiente de una mini-app de algun chat.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AvisoMini {
    /// `mini:` + el id del mensaje: el identificador del aviso.
    pub id: String,
    pub chat: String,
    pub mensaje: String,
    pub miniapp: String,
    /// Lo que dice el aviso: el titulo del documento, o el nombre del
    /// mensaje si no lleva (vacio si tampoco: Swift pone el de la mini-app).
    pub texto: String,
    /// Cuando, en milisegundos UTC.
    pub cuando_ms: i64,
}

fn aviso_de(chat: &str, m: &Mensaje, ahora: i64, desfase: i64) -> Option<AvisoMini> {
    if m.clase != Some(Clase::MiniApp) {
        return None;
    }
    let cual = m.miniapp.as_deref()?;
    let cuando_ms = cuando_avisa(cual, &m.texto, ahora, desfase)?;
    let titulo = mini::titulo(&m.texto);
    Some(AvisoMini {
        id: format!("{PREFIJO_AVISO}{}", m.id),
        chat: chat.to_string(),
        mensaje: m.id.clone(),
        miniapp: cual.to_string(),
        texto: if titulo.is_empty() {
            m.nombre.clone()
        } else {
            titulo
        },
        cuando_ms,
    })
}

#[uniffi::export]
impl Nucleo {
    /// **Los avisos pendientes** de los temporizadores y alarmas de todos los
    /// chats, ordenados por hora: lo que hay que volver a poner al arrancar y
    /// tras sincronizar (un temporizador puesto en el movil llega como texto
    /// de un mensaje y, sin esto, no sonaria en el Mac).
    ///
    /// Solo se leen las lineas que pueden ser uno (llevan su palabra), y un
    /// cuaderno ilegible se salta en vez de tumbar la lista.
    pub fn avisos_miniapps(&self, desfase_local_ms: i64) -> Result<Vec<AvisoMini>, ErrorNucleo> {
        let raiz = self.ruta();
        let ahora = crate::ahora_ms();
        let mut avisos = Vec::new();
        for ficha in Indice::leer(raiz).proyectos {
            let fichero = almacen::carpeta(raiz, &ficha.id).join("guardados.jsonl");
            let Ok(texto) = std::fs::read_to_string(&fichero) else {
                continue;
            };
            avisos.extend(
                texto
                    .lines()
                    .filter(|l| l.contains(mini::TEMPORIZADOR) || l.contains(mini::ALARMA))
                    .filter_map(|l| serde_json::from_str::<Mensaje>(l).ok())
                    .filter_map(|m| aviso_de(&ficha.id, &m, ahora, desfase_local_ms)),
            );
        }
        avisos.sort_by_key(|a| a.cuando_ms);
        Ok(avisos)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::miniapps::mini_documento_nuevo;

    fn nueva(cual: &str) -> String {
        mini_documento_nuevo(cual.into(), "P".into(), "EUR".into()).unwrap()
    }

    #[test]
    fn el_cronometro_arranca_marca_vueltas_para_y_reinicia() {
        let mut d = nueva(mini::CRONOMETRO);
        let v = mini_cronometro_leer(d.clone(), 0);
        assert!(!v.corriendo && !v.se_reinicia);
        assert_eq!(v.tablero, "0:00,0");
        d = mini_cronometro_cambiar(d, AccionCronometro::ArrancarOParar, 1_000);
        // Parado no hay vueltas que marcar; en marcha si.
        d = mini_cronometro_cambiar(d, AccionCronometro::Vuelta, 11_250);
        let v = mini_cronometro_leer(d.clone(), 66_300);
        assert!(v.corriendo);
        assert_eq!(v.tablero, "1:05,3", "calculado al mirarlo, no guardado");
        assert_eq!(v.vueltas, vec!["0:10,2"]);
        d = mini_cronometro_cambiar(d, AccionCronometro::ArrancarOParar, 66_300);
        assert_eq!(
            d, "# P\n\n- llevado: 65300\n- vueltas: 10250",
            "el texto del movil"
        );
        // Parado, el tiempo no corre aunque pase una hora.
        assert_eq!(
            mini_cronometro_leer(d.clone(), 3_666_300).transcurrido,
            65_300
        );
        d = mini_cronometro_cambiar(d, AccionCronometro::Reiniciar, 0);
        assert_eq!(d, "# P\n\n- llevado: 0");
    }

    #[test]
    fn el_temporizador_no_se_ajusta_en_marcha_y_se_pone_en_rojo_a_cero() {
        let mut d = nueva(mini::TEMPORIZADOR);
        assert_eq!(mini_temporizador_leer(d.clone(), 0).tablero, "5:00");
        d = mini_temporizador_cambiar(d, AccionTemporizador::Minutos { minutos: -4 }, 0);
        assert_eq!(mini_temporizador_leer(d.clone(), 0).duracion, 60_000);
        d = mini_temporizador_cambiar(d, AccionTemporizador::ArrancarOParar, 1_000);
        assert_eq!(d, "# P\n\n- duracion: 60000\n- finEn: 61000", "finEn con F");
        let en_marcha = d.clone();
        d = mini_temporizador_cambiar(d, AccionTemporizador::Minutos { minutos: 5 }, 2_000);
        assert_eq!(d, en_marcha, "en marcha no se ajusta");
        d = mini_temporizador_cambiar(d, AccionTemporizador::Duracion { ms: 9 }, 2_000);
        assert_eq!(d, en_marcha);
        let v = mini_temporizador_leer(d.clone(), 31_000);
        assert_eq!((v.tablero.as_str(), v.vencido), ("0:30", false));
        let v = mini_temporizador_leer(d.clone(), 99_000);
        assert_eq!((v.restante, v.vencido), (0, true), "no baja de cero");
        d = mini_temporizador_cambiar(d, AccionTemporizador::ArrancarOParar, 99_000);
        assert_eq!(d, "# P\n\n- duracion: 60000");
        // La pegatina: en esos minutos y en marcha, este como este.
        let d = mini_temporizador_cambiar(
            en_marcha,
            AccionTemporizador::EnMarchaCon { minutos: 25 },
            5_000,
        );
        assert_eq!(d, "# P\n\n- duracion: 1500000\n- finEn: 1505000");
    }

    #[test]
    fn lo_que_dura_se_teclea_como_en_windows() {
        assert_eq!(mini_duracion_tecleada("7".into()), Some(7 * 60_000));
        assert_eq!(mini_duracion_tecleada("2,5".into()), Some(150_000));
        assert_eq!(mini_duracion_tecleada("1:30".into()), Some(90_000));
        assert_eq!(mini_duracion_tecleada("1:02:03".into()), Some(3_723_000));
        assert_eq!(
            mini_duracion_tecleada("100000".into()),
            Some(tiempos::TOPE_DE_DURACION)
        );
        for malo in ["", "0", "abc", "1:75", "1::2", "1:2:3:4", "−5", "ñ"] {
            assert_eq!(mini_duracion_tecleada(malo.into()), None, "{malo:?}");
        }
    }

    #[test]
    fn la_alarma_se_mueve_en_redondo_y_tecleada_se_enciende() {
        let mut d = nueva(mini::ALARMA);
        assert_eq!(mini_alarma_leer(d.clone()).tablero, "8:00");
        d = mini_alarma_cambiar(d, AccionAlarma::Horas { horas: -9 });
        d = mini_alarma_cambiar(d, AccionAlarma::Minutos { minutos: -5 });
        let a = mini_alarma_leer(d.clone());
        assert_eq!(
            (a.hora, a.minuto, a.activa),
            (23, 55, false),
            "con botones no se enciende"
        );
        d = mini_alarma_cambiar(d, AccionAlarma::Hora { hora: 7, minuto: 5 });
        assert_eq!(d, "# P\n\n- hora: 7:05\n- activa: sí", "con la tilde");
        assert_eq!(
            mini_alarma_cambiar(
                d.clone(),
                AccionAlarma::Hora {
                    hora: 24,
                    minuto: 0
                }
            ),
            d
        );
        d = mini_alarma_cambiar(d, AccionAlarma::Activar);
        assert!(!mini_alarma_leer(d).activa);
    }

    #[test]
    fn la_hora_se_teclea_como_en_windows() {
        let h = |t: &str| mini_hora_tecleada(t.into()).map(|h| (h.hora, h.minuto));
        assert_eq!(h("7:30"), Some((7, 30)));
        assert_eq!(h("730"), Some((7, 30)));
        assert_eq!(h("7.30"), Some((7, 30)));
        assert_eq!(h("19"), Some((19, 0)));
        assert_eq!(h("18:"), Some((18, 0)));
        assert_eq!(h("0:00"), Some((0, 0)));
        assert_eq!(h("23:59"), Some((23, 59)));
        assert_eq!(h("25:00"), None);
        assert_eq!(h("9:75"), None);
        assert_eq!(h("mañana"), None);
    }

    #[test]
    fn avisa_el_temporizador_a_su_hora_y_la_alarma_la_proxima_vez() {
        const H: i64 = 3_600_000;
        let t = |d: &str, ahora| mini_cuando_avisa(mini::TEMPORIZADOR.into(), d.into(), ahora, 0);
        assert_eq!(t("- duracion: 1000\n- finEn: 10000", 5_000), Some(10_000));
        assert_eq!(
            t("- duracion: 1000\n- finEn: 10000", 10_000),
            None,
            "ya aviso"
        );
        assert_eq!(t(&nueva(mini::TEMPORIZADOR), 0), None, "parado");
        let d = "# D\n\n- hora: 7:30\n- activa: sí";
        let desfase = 2 * H;
        let dia = 20_000 * DIA;
        let a = |ahora| mini_cuando_avisa(mini::ALARMA.into(), d.into(), ahora, desfase);
        assert_eq!(
            a(dia + 6 * H - desfase),
            Some(dia + 7 * H + H / 2 - desfase)
        );
        assert_eq!(
            a(dia + 8 * H - desfase),
            Some(dia + DIA + 7 * H + H / 2 - desfase),
            "ya paso: manana"
        );
        assert_eq!(
            mini_cuando_avisa(
                mini::ALARMA.into(),
                "- hora: 7:30\n- activa: no".into(),
                0,
                0
            ),
            None
        );
        assert_eq!(
            mini_cuando_avisa(mini::CONTADOR.into(), nueva(mini::CONTADOR), 0, 0),
            None
        );
    }

    #[test]
    fn los_avisos_de_todos_los_chats_salen_con_su_prefijo_y_en_orden() {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-mini-tiempos-{}",
            crate::chats::hora_unica()
        ));
        let n = Nucleo::abrir(dir.display().to_string(), "Mac".into()).unwrap();
        let chat = n.chat_guardados().unwrap().id;
        let alarma = n
            .crear_miniapp(
                chat.clone(),
                mini::ALARMA.into(),
                "Despertar".into(),
                "EUR".into(),
            )
            .unwrap();
        let temp = n
            .crear_miniapp(
                chat.clone(),
                mini::TEMPORIZADOR.into(),
                "".into(),
                "EUR".into(),
            )
            .unwrap();
        n.crear_miniapp(
            chat.clone(),
            mini::CRONOMETRO.into(),
            "".into(),
            "EUR".into(),
        )
        .unwrap();
        assert!(
            n.avisos_miniapps(0).unwrap().is_empty(),
            "apagados no avisan"
        );
        let d0 = n
            .documento_miniapp(chat.clone(), alarma.id.clone())
            .unwrap();
        let d1 = mini_alarma_cambiar(d0.clone(), AccionAlarma::Activar);
        assert!(
            n.guardar_miniapp(chat.clone(), alarma.id.clone(), d0, d1)
                .unwrap()
        );
        let ahora = crate::ahora_ms();
        let t0 = n.documento_miniapp(chat.clone(), temp.id.clone()).unwrap();
        let t1 = mini_temporizador_cambiar(
            t0.clone(),
            AccionTemporizador::EnMarchaCon { minutos: 1 },
            ahora,
        );
        assert!(
            n.guardar_miniapp(chat.clone(), temp.id.clone(), t0, t1)
                .unwrap()
        );
        let avisos = n.avisos_miniapps(0).unwrap();
        assert_eq!(avisos.len(), 2);
        let a = avisos.iter().find(|a| a.miniapp == mini::ALARMA).unwrap();
        assert_eq!(a.id, format!("mini:{}", alarma.id));
        assert_eq!(a.texto, "Despertar");
        assert_eq!(a.chat, chat);
        let t = avisos
            .iter()
            .find(|a| a.miniapp == mini::TEMPORIZADOR)
            .unwrap();
        assert_eq!(t.cuando_ms, ahora + 60_000);
        assert!(avisos.windows(2).all(|p| p[0].cuando_ms <= p[1].cuando_ms));
        let _ = std::fs::remove_dir_all(dir);
    }
}
