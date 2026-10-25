//! **Quien hablo cuando**: la conversacion por turnos (B10).
//!
//! Es `Mensaje.turnos` del movil (`TurnoDeVoz(quien, desdeMs, hastaMs)`,
//! `ConversacionActivity.terminar`): con los turnos guardados, pasar la nota
//! a texto (la primera vez o «otra vez») saca **cada turno por su cuenta y
//! con su nombre delante**, `[0:12] **Pepe:** lo que dijo`. Asi se sabe
//! quien dijo cada cosa sin adivinarlo por la voz, que es lo que ningun
//! reconocedor gratuito hace bien.
//!
//! Aritmetica y texto, sin Windows: se prueba entero.

use crate::tiempos::{EstadoDelTexto, Segmento, con_tiempos};

/// Un turno: quien hablo y de que milisegundo a cual del audio entero.
/// Los nombres de los campos son los del JSON del movil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turno {
    pub quien: String,
    pub desde_ms: i64,
    pub hasta_ms: i64,
}

impl Turno {
    pub fn nuevo(quien: impl Into<String>, desde_ms: i64, hasta_ms: i64) -> Turno {
        Turno {
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

/// Las muestras (a `hercios`) de cada turno dentro de un audio de `total`
/// muestras, recortadas al audio. Un turno que queda vacio no sale.
pub fn rangos(
    turnos: &[Turno],
    total: usize,
    hercios: u32,
) -> Vec<(usize, std::ops::Range<usize>)> {
    let muestra = |ms: i64| ((ms.max(0) as u128 * hercios as u128 / 1000) as usize).min(total);
    turnos
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let (a, b) = (muestra(t.desde_ms), muestra(t.hasta_ms));
            (b > a).then_some((i, a..b))
        })
        .collect()
}

/// Lo de un turno ya pasado a texto: sus trozos con el minuto **del audio
/// entero** (ya corridos por quien llama).
pub struct Oido<'a> {
    pub turno: &'a Turno,
    pub segmentos: Vec<Segmento>,
}

/// **El dialogo**: cada turno con su nombre en negrita delante de cada
/// parrafo, separados por renglon en blanco. Un turno sin nada entendido no
/// deja linea.
pub fn dialogo(oidos: &[Oido<'_>]) -> String {
    oidos
        .iter()
        .filter(|o| !o.segmentos.is_empty())
        .map(|o| con_tiempos(&o.segmentos, &format!("**{}:** ", o.turno.quien)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Como queda el texto (`estadoDelTexto` del movil): nada entendido, mal;
/// menos turnos con texto que turnos, aviso; todos, bien.
pub fn estado(con_texto: usize, turnos: usize) -> EstadoDelTexto {
    if con_texto == 0 {
        EstadoDelTexto::Mal
    } else if con_texto < turnos {
        EstadoDelTexto::Aviso
    } else {
        EstadoDelTexto::Bien
    }
}

/// **Los turnos de una grabacion continua** (lo propio del PC): se graba
/// sin parar y cada tecla 1..9 dice quien habla desde ese momento.
///
/// `cambios` son `(milisegundo, quien)` en el orden en que se pulsaron;
/// `fin_ms` lo que dura la grabacion. Cada turno va de su cambio al
/// siguiente; pulsar la misma persona dos veces seguidas no parte su turno,
/// y un turno de menos de `minimo_ms` (un dedo que se equivoco de tecla y
/// corrigio en el acto) se le suma al anterior. Lo grabado antes del primer
/// cambio no es de nadie y no sale.
pub fn de_los_cambios(cambios: &[(i64, String)], fin_ms: i64, minimo_ms: i64) -> Vec<Turno> {
    let mut v: Vec<Turno> = Vec::new();
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
            _ => v.push(Turno::nuevo(quien.clone(), *desde, hasta)),
        }
    }
    // Los turnos cortisimos se funden con el de antes (o con el de despues
    // si son el primero): son tropiezos, no turnos.
    let mut limpio: Vec<Turno> = Vec::new();
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

/// Los turnos en JSON, como los escribe el movil (`quien`, `desdeMs`,
/// `hastaMs`), para `Mensaje.resto["turnos"]`.
pub fn a_json(turnos: &[Turno]) -> serde_json::Value {
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
pub fn de_json(v: &serde_json::Value) -> Vec<Turno> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|t| {
                    Some(Turno::nuevo(
                        t.get("quien")?.as_str()?,
                        t.get("desdeMs")?.as_i64()?,
                        t.get("hastaMs")?.as_i64()?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_turno_sale_con_su_nombre_delante_y_su_minuto_del_audio_entero() {
        let pepe = Turno::nuevo("Pepe", 0, 4_000);
        let juan = Turno::nuevo("Juan", 4_000, 30_000);
        let oidos = [
            Oido {
                turno: &pepe,
                segmentos: vec![Segmento::nuevo(0, "hola")],
            },
            Oido {
                turno: &juan,
                segmentos: vec![Segmento::nuevo(4_000, "que tal")],
            },
        ];
        assert_eq!(
            dialogo(&oidos),
            "[0:00] **Pepe:** hola\n\n[0:04] **Juan:** que tal"
        );
    }

    #[test]
    fn un_turno_sin_nada_entendido_no_deja_linea_y_el_estado_lo_dice() {
        let pepe = Turno::nuevo("Pepe", 0, 4_000);
        let juan = Turno::nuevo("Juan", 4_000, 8_000);
        let oidos = [
            Oido {
                turno: &pepe,
                segmentos: vec![],
            },
            Oido {
                turno: &juan,
                segmentos: vec![Segmento::nuevo(4_000, "si")],
            },
        ];
        assert_eq!(dialogo(&oidos), "[0:04] **Juan:** si");
        assert_eq!(estado(1, 2), EstadoDelTexto::Aviso);
        assert_eq!(estado(2, 2), EstadoDelTexto::Bien);
        assert_eq!(estado(0, 2), EstadoDelTexto::Mal);
    }

    #[test]
    fn los_rangos_se_recortan_al_audio_y_los_vacios_no_salen() {
        let t = [
            Turno::nuevo("A", 0, 1_000),
            Turno::nuevo("B", 1_000, 1_000),
            Turno::nuevo("C", 1_500, 99_000),
            Turno::nuevo("D", 5_000, 2_000),
        ];
        let r = rangos(&t, 32_000, 16_000);
        assert_eq!(r, vec![(0, 0..16_000), (2, 24_000..32_000)]);
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
                Turno::nuevo("Ana", 500, 4_000),
                Turno::nuevo("Luis", 4_000, 9_000),
                Turno::nuevo("Ana", 9_000, 12_000),
            ]
        );
    }

    #[test]
    fn la_misma_persona_dos_veces_seguidas_no_parte_su_turno() {
        let c = vec![(0, "Ana".to_string()), (3_000, "Ana".to_string())];
        assert_eq!(
            de_los_cambios(&c, 6_000, 300),
            vec![Turno::nuevo("Ana", 0, 6_000)]
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
                Turno::nuevo("Ana", 0, 4_100),
                Turno::nuevo("Eva", 4_100, 8_000)
            ]
        );
    }

    #[test]
    fn sin_ninguna_tecla_no_hay_turnos() {
        assert!(de_los_cambios(&[], 8_000, 300).is_empty());
    }

    #[test]
    fn los_turnos_van_y_vuelven_con_los_nombres_del_movil_y_lo_roto_se_salta() {
        let t = vec![Turno::nuevo("Pepe", 0, 1_200)];
        let j = a_json(&t);
        assert_eq!(j[0]["desdeMs"], 0);
        assert_eq!(j[0]["hastaMs"], 1_200);
        assert_eq!(de_json(&j), t);
        let roto = serde_json::json!([{"quien": "X"}, {"quien": "Y", "desdeMs": 1, "hastaMs": 2}]);
        assert_eq!(de_json(&roto), vec![Turno::nuevo("Y", 1, 2)]);
        assert!(de_json(&serde_json::json!("nada")).is_empty());
    }
}
