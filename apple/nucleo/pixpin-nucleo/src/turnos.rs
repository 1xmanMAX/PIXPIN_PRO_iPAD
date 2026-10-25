//! **Quien hablo cuando**: la conversacion por turnos (tanda E5).
//!
//! Es `Mensaje.turnos` del movil (`TurnoDeVoz(quien, desdeMs, hastaMs)`,
//! `ConversacionActivity.terminar`) y `pixpin_voz::turnos` de Windows, que
//! sigue en `por-adaptar/`: lo poco que hace falta esta copiado aqui **al
//! caracter** (el campo viaja por la sincronizacion). Con los turnos
//! guardados, pasar la nota a texto saca **cada turno con su nombre
//! delante**, `[0:12] **Pepe:** lo que dijo`.
//!
//! Lo propio del ordenador: se graba **sin parar** y las teclas 1..9 dicen
//! quien habla desde ese momento; el 0 es «no habla nadie»
//! ([`de_los_cambios`]). La ventana es de Swift
//! (`VozVentanas/Conversacion.swift`).
//!
//! **Lo que cambia en el Mac**: Windows pasa a texto cada turno por
//! separado (Whisper sobre su trozo de audio); aqui el reconocedor de Apple
//! pasa la nota entera y cada trozo reconocido va al turno en el que
//! empieza ([`repartir`]). Lo dicho cuando no hablaba nadie no sale, igual
//! que alli.

use pixpin_proyecto::cuaderno::Mensaje;

use crate::voz::{SegmentoVoz, marca_de_tiempo};

/// El nombre del campo en el JSONL: el del movil, no se traduce.
pub const CAMPO: &str = "turnos";

/// Un turno mas corto que esto es un dedo que se equivoco de tecla
/// (`conversacion::TURNO_MINIMO_MS`).
pub const TURNO_MINIMO_MS: i64 = 400;

/// Un turno: quien hablo y de que milisegundo a cual del audio entero.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TurnoVoz {
    pub quien: String,
    pub desde_ms: i64,
    pub hasta_ms: i64,
}

impl TurnoVoz {
    pub fn nuevo(quien: impl Into<String>, desde_ms: i64, hasta_ms: i64) -> TurnoVoz {
        TurnoVoz {
            quien: quien.into(),
            desde_ms,
            hasta_ms,
        }
    }

    /// Lo que dura. Un turno al reves (un JSON tocado a mano) dura cero.
    pub fn dura_ms(&self) -> i64 {
        (self.hasta_ms - self.desde_ms).max(0)
    }
}

/// Una tecla de la conversacion: desde que milisegundo de la grabacion habla
/// `quien` (vacio: nadie).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CambioDeTurno {
    pub ms: i64,
    pub quien: String,
}

/// **Los turnos de una grabacion continua**: `cambios` en el orden en que
/// se pulsaron y `fin_ms` lo que dura. Cada turno va de su cambio al
/// siguiente; pulsar la misma persona dos veces seguidas no parte su turno,
/// y un turno de menos de `minimo_ms` se le suma al anterior. Lo grabado
/// antes del primer cambio no es de nadie y no sale.
pub fn de_los_cambios(cambios: &[(i64, String)], fin_ms: i64, minimo_ms: i64) -> Vec<TurnoVoz> {
    let mut v: Vec<TurnoVoz> = Vec::new();
    for (n, (desde, quien)) in cambios.iter().enumerate() {
        let hasta = cambios
            .get(n + 1)
            .map(|c| c.0)
            .unwrap_or(fin_ms)
            .min(fin_ms);
        if hasta <= *desde {
            continue;
        }
        match v.last_mut() {
            Some(u) if u.quien == *quien => u.hasta_ms = hasta,
            _ => v.push(TurnoVoz::nuevo(quien.clone(), *desde, hasta)),
        }
    }
    // Los turnos cortisimos se funden con el de antes (o con el de despues
    // si son el primero): son tropiezos, no turnos.
    let mut limpio: Vec<TurnoVoz> = Vec::new();
    for t in v {
        if t.dura_ms() < minimo_ms
            && let Some(u) = limpio.last_mut()
        {
            u.hasta_ms = t.hasta_ms;
            continue;
        }
        match limpio.last_mut() {
            Some(u) if u.quien == t.quien => u.hasta_ms = t.hasta_ms,
            _ => limpio.push(t),
        }
    }
    if limpio.len() > 1 && limpio[0].dura_ms() < minimo_ms {
        let primero = limpio.remove(0);
        limpio[0].desde_ms = primero.desde_ms;
    }
    limpio
}

/// Los turnos de la conversacion a partir de las teclas, sin los ratos en
/// que no hablaba nadie (el 0): `conversacion::turnos_de` de Windows.
pub fn de_la_conversacion(cambios: &[(i64, String)], fin_ms: i64) -> Vec<TurnoVoz> {
    de_los_cambios(cambios, fin_ms, TURNO_MINIMO_MS)
        .into_iter()
        .filter(|t| !t.quien.is_empty())
        .collect()
}

/// Los turnos en JSON, como los escribe el movil (`quien`, `desdeMs`,
/// `hastaMs`), para `Mensaje.resto["turnos"]`.
pub fn a_json(turnos: &[TurnoVoz]) -> serde_json::Value {
    serde_json::Value::Array(
        turnos
            .iter()
            .map(|t| {
                serde_json::json!({
                    "quien": t.quien,
                    "desdeMs": t.desde_ms,
                    "hastaMs": t.hasta_ms,
                })
            })
            .collect(),
    )
}

/// Y de vuelta. Lo que no tenga los tres campos bien se salta: viene de
/// otro aparato y no puede tumbar la transcripcion.
pub fn de_json(v: &serde_json::Value) -> Vec<TurnoVoz> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|t| {
                    Some(TurnoVoz::nuevo(
                        t.get("quien")?.as_str()?,
                        t.get("desdeMs")?.as_i64()?,
                        t.get("hastaMs")?.as_i64()?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Los turnos de un mensaje (`voz::turnos_de` de Windows): vacio si no es
/// una conversacion.
pub fn de_un_mensaje(m: &Mensaje) -> Vec<TurnoVoz> {
    m.resto.get(CAMPO).map(de_json).unwrap_or_default()
}

/// El texto con tiempos de un turno: `con_tiempos` de `pixpin_voz::tiempos`
/// con el nombre delante de cada parrafo.
fn con_tiempos(segmentos: &[SegmentoVoz], prefijo: &str) -> String {
    let mut lineas: Vec<String> = Vec::new();
    let mut desde: i64 = -1;
    let mut trozo = String::new();
    for sg in segmentos {
        if desde >= 0 && sg.desde_ms - desde >= crate::voz::PARRAFO_MS {
            lineas.push(format!(
                "[{}] {prefijo}{}",
                marca_de_tiempo(desde),
                trozo.trim()
            ));
            trozo.clear();
            desde = -1;
        }
        if desde < 0 {
            desde = sg.desde_ms;
        }
        trozo.push_str(&sg.texto);
        trozo.push(' ');
    }
    if !trozo.trim().is_empty() {
        lineas.push(format!(
            "[{}] {prefijo}{}",
            marca_de_tiempo(desde.max(0)),
            trozo.trim()
        ));
    }
    lineas.join("\n\n")
}

/// **Reparte lo reconocido de la nota entera entre sus turnos**: cada trozo
/// va al turno en el que empieza (el ultimo se queda tambien lo que cae
/// justo en su final). Lo que cae fuera de todos (nadie hablaba) se tira.
pub fn repartir(turnos: &[TurnoVoz], segmentos: &[SegmentoVoz]) -> Vec<Vec<SegmentoVoz>> {
    let mut v = vec![Vec::new(); turnos.len()];
    for sg in segmentos.iter().filter(|s| !s.texto.trim().is_empty()) {
        let dentro = turnos
            .iter()
            .position(|t| sg.desde_ms >= t.desde_ms && sg.desde_ms < t.hasta_ms.max(t.desde_ms));
        let i = dentro.or_else(|| {
            turnos
                .iter()
                .rposition(|t| t.dura_ms() > 0 && sg.desde_ms == t.hasta_ms)
        });
        if let Some(i) = i {
            v[i].push(sg.clone());
        }
    }
    v
}

/// **El dialogo**: cada turno con su nombre en negrita delante de cada
/// parrafo, separados por renglon en blanco. Un turno sin nada entendido no
/// deja linea.
pub fn dialogo(turnos: &[TurnoVoz], oidos: &[Vec<SegmentoVoz>]) -> String {
    turnos
        .iter()
        .zip(oidos)
        .filter(|(_, s)| !s.is_empty())
        .map(|(t, s)| con_tiempos(s, &format!("**{}:** ", t.quien)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Como queda el texto (`estadoDelTexto` del movil): nada entendido, `mal`;
/// menos turnos con texto que turnos, `aviso`; todos, `bien`.
pub fn estado(con_texto: usize, turnos: usize) -> &'static str {
    if con_texto == 0 {
        "mal"
    } else if con_texto < turnos {
        "aviso"
    } else {
        "bien"
    }
}

/// El texto y su estado de una conversacion pasada a texto entera.
pub fn texto_por_turnos(turnos: &[TurnoVoz], segmentos: &[SegmentoVoz]) -> (String, &'static str) {
    let oidos = repartir(turnos, segmentos);
    let con_texto = oidos.iter().filter(|s| !s.is_empty()).count();
    (dialogo(turnos, &oidos), estado(con_texto, turnos.len()))
}

/// **Los turnos de la conversacion** a partir de sus teclas: lo que Swift
/// guarda al terminar (sin los ratos de nadie, con el tope de los tropiezos).
#[uniffi::export]
pub fn turnos_de_la_conversacion(cambios: Vec<CambioDeTurno>, fin_ms: i64) -> Vec<TurnoVoz> {
    let pares: Vec<(i64, String)> = cambios.into_iter().map(|c| (c.ms, c.quien)).collect();
    de_la_conversacion(&pares, fin_ms)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn sg(ms: i64, t: &str) -> SegmentoVoz {
        SegmentoVoz {
            desde_ms: ms,
            texto: t.into(),
        }
    }

    #[test]
    fn cada_turno_sale_con_su_nombre_delante_y_su_minuto_del_audio_entero() {
        let t = [
            TurnoVoz::nuevo("Pepe", 0, 4_000),
            TurnoVoz::nuevo("Juan", 4_000, 30_000),
        ];
        let (texto, estado) = texto_por_turnos(&t, &[sg(0, "hola"), sg(4_000, "que tal")]);
        assert_eq!(texto, "[0:00] **Pepe:** hola\n\n[0:04] **Juan:** que tal");
        assert_eq!(estado, "bien");
    }

    #[test]
    fn un_turno_sin_nada_entendido_no_deja_linea_y_el_estado_lo_dice() {
        let t = [
            TurnoVoz::nuevo("Pepe", 0, 4_000),
            TurnoVoz::nuevo("Juan", 4_000, 8_000),
        ];
        let (texto, e) = texto_por_turnos(&t, &[sg(4_000, "si")]);
        assert_eq!(texto, "[0:04] **Juan:** si");
        assert_eq!(e, "aviso");
        assert_eq!(estado(2, 2), "bien");
        assert_eq!(estado(0, 2), "mal");
    }

    #[test]
    fn lo_dicho_cuando_no_hablaba_nadie_no_sale() {
        let t = [
            TurnoVoz::nuevo("Ana", 0, 3_000),
            TurnoVoz::nuevo("Luis", 6_000, 9_000),
        ];
        let o = repartir(
            &t,
            &[sg(1_000, "uno"), sg(4_000, "ruido"), sg(9_000, "fin")],
        );
        assert_eq!(o[0], vec![sg(1_000, "uno")]);
        assert_eq!(
            o[1],
            vec![sg(9_000, "fin")],
            "lo que cae en el final es del ultimo"
        );
        // Caso negativo: sin turnos, nada es de nadie.
        assert!(repartir(&[], &[sg(0, "x")]).is_empty());
    }

    #[test]
    fn las_teclas_parten_la_grabacion_en_turnos_de_quien_hablaba() {
        let c = vec![
            (500, "Ana".to_string()),
            (4_000, "Luis".to_string()),
            (9_000, "Ana".to_string()),
        ];
        assert_eq!(
            de_los_cambios(&c, 12_000, 300),
            vec![
                TurnoVoz::nuevo("Ana", 500, 4_000),
                TurnoVoz::nuevo("Luis", 4_000, 9_000),
                TurnoVoz::nuevo("Ana", 9_000, 12_000),
            ]
        );
    }

    #[test]
    fn la_misma_persona_dos_veces_seguidas_no_parte_su_turno() {
        let c = vec![(0, "Ana".to_string()), (3_000, "Ana".to_string())];
        assert_eq!(
            de_los_cambios(&c, 6_000, 300),
            vec![TurnoVoz::nuevo("Ana", 0, 6_000)]
        );
    }

    #[test]
    fn una_tecla_equivocada_y_corregida_en_el_acto_no_es_un_turno() {
        let c = vec![
            (0, "Ana".to_string()),
            (4_000, "Luis".to_string()),
            (4_100, "Eva".to_string()),
        ];
        assert_eq!(
            de_los_cambios(&c, 8_000, 300),
            vec![
                TurnoVoz::nuevo("Ana", 0, 4_100),
                TurnoVoz::nuevo("Eva", 4_100, 8_000)
            ]
        );
    }

    #[test]
    fn sin_ninguna_tecla_no_hay_turnos() {
        assert!(de_los_cambios(&[], 8_000, 300).is_empty());
    }

    #[test]
    fn lo_que_se_graba_con_el_cero_no_es_de_nadie_y_no_sale() {
        let c = vec![
            (0, "Ana".to_string()),
            (3_000, String::new()),
            (6_000, "Luis".to_string()),
        ];
        assert_eq!(
            de_la_conversacion(&c, 9_000),
            vec![
                TurnoVoz::nuevo("Ana", 0, 3_000),
                TurnoVoz::nuevo("Luis", 6_000, 9_000)
            ]
        );
        let por_swift = turnos_de_la_conversacion(
            vec![
                CambioDeTurno {
                    ms: 0,
                    quien: "Ana".into(),
                },
                CambioDeTurno {
                    ms: 3_000,
                    quien: String::new(),
                },
            ],
            9_000,
        );
        assert_eq!(por_swift, vec![TurnoVoz::nuevo("Ana", 0, 3_000)]);
    }

    #[test]
    fn los_turnos_van_y_vuelven_con_los_nombres_del_movil_y_lo_roto_se_salta() {
        let t = vec![TurnoVoz::nuevo("Pepe", 0, 1_200)];
        let j = a_json(&t);
        assert_eq!(j[0]["desdeMs"], 0);
        assert_eq!(j[0]["hastaMs"], 1_200);
        assert_eq!(de_json(&j), t);
        let roto = serde_json::json!([{"quien": "X"}, {"quien": "Y", "desdeMs": 1, "hastaMs": 2}]);
        assert_eq!(de_json(&roto), vec![TurnoVoz::nuevo("Y", 1, 2)]);
        assert!(de_json(&serde_json::json!("nada")).is_empty());
    }

    #[test]
    fn los_turnos_de_una_conversacion_se_leen_del_mensaje() {
        let mut m = Mensaje::default();
        assert!(de_un_mensaje(&m).is_empty());
        m.resto.insert(
            CAMPO.into(),
            serde_json::json!([{"quien": "Ana", "desdeMs": 0, "hastaMs": 900}]),
        );
        assert_eq!(de_un_mensaje(&m), vec![TurnoVoz::nuevo("Ana", 0, 900)]);
    }
}
