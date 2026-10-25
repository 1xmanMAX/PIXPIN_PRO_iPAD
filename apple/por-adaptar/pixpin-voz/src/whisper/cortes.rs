//! **Por donde se parte una nota para Whisper**: en frases, y las frases
//! en tandas.
//!
//! Puerto de `Transcriptor.cortes`, `puntoMasCallado` y `agrupar`
//! (`Transcriptor.kt:269-280, 343-423`), con los mismos numeros. El movil
//! trabaja en **bytes** de un fichero PCM; aqui el audio ya esta en memoria
//! y se cuenta en **muestras** (un byte par del movil es media muestra), asi
//! que cada constante esta pasada a muestras.
//!
//! Por que partir: Whisper oye 30 s como mucho y **se inventa frases** en
//! los silencios largos. Cortando en los silencios y juntando frases
//! seguidas hasta 20 s, cada tanda es voz con poco hueco, y cada una sale
//! con el minuto en que empieza para escribir `[m:ss]`.
//!
//! Aritmetica pura: se prueba entero sin modelo.

use std::ops::Range;

use crate::HERCIOS;

const MUESTRAS_POR_MS: usize = HERCIOS as usize / 1000;
/// `VENTANA_MS`: se mide la energia cada 20 ms.
const VENTANA: usize = 20 * MUESTRAS_POR_MS;
/// `SILENCIO_MS`: un silencio de un cuarto de segundo ya separa frases.
const SILENCIO_MS: usize = 250;
/// `MINIMO_MS`: lo que no llega a 0,3 s es un golpe, no una frase.
const MINIMO: usize = 300 * MUESTRAS_POR_MS;
/// `TROZO_SEGUNDOS`: una frase de mas de 8 s se parte por su punto mas
/// callado, buscado en los 4 s de antes del tope (`BUSQUEDA_SEGUNDOS`).
const TROZO: usize = 8 * HERCIOS as usize;
const BUSQUEDA: usize = 4 * HERCIOS as usize;
/// `PARTE_DE_SILENCIO` y `UMBRAL_MINIMO`: silencio es menos de un octavo de
/// la energia tipica de la nota, y nunca menos de 120 (ruido de fondo).
const PARTE_DE_SILENCIO: f64 = 0.12;
const UMBRAL_MINIMO: f64 = 120.0;

/// **Las frases de una nota**, como rangos de muestras.
///
/// Se mide la energia (RMS) cada 20 ms; silencio es lo que queda por debajo
/// de una fraccion de la energia tipica (el percentil 60); se corta en
/// mitad de cada silencio de mas de un cuarto de segundo. Los trozos de mas
/// de 8 s se parten en su punto mas callado; lo que no llega a 0,3 s se
/// tira.
pub fn cortes(pcm: &[i16]) -> Vec<Range<usize>> {
    let total = pcm.len();
    if total == 0 {
        return Vec::new();
    }
    if total < VENTANA {
        // Una lista con un tramo, no un rango de numeros mal escrito.
        #[allow(clippy::single_range_in_vec_init)]
        return vec![0..total];
    }
    let mut puntos: Vec<usize> = silencios(pcm).iter().map(mitad).collect();
    puntos.push(total);

    let mut salida = Vec::new();
    let mut desde = 0usize;
    for fin in puntos {
        if fin <= desde {
            continue;
        }
        while fin - desde > TROZO {
            let tope = desde + TROZO;
            let en_medio = punto_mas_callado(pcm, tope.saturating_sub(BUSQUEDA), tope);
            if en_medio - desde >= MINIMO {
                salida.push(desde..en_medio);
            }
            desde = en_medio;
        }
        if fin - desde >= MINIMO {
            salida.push(desde..fin);
        }
        desde = fin;
    }
    salida.retain(|r| !r.is_empty());
    salida
}

/// **Los silencios de una nota** de mas de un cuarto de segundo, en
/// muestras: lo que queda por debajo de una fraccion de la energia tipica
/// (el percentil 60), medido cada 20 ms. Es lo que usa [`cortes`] para
/// partir, y [`pausas_largas`] para saber donde cambia de idioma.
fn silencios(pcm: &[i16]) -> Vec<Range<usize>> {
    let energias: Vec<f64> = pcm.chunks_exact(VENTANA).map(rms).collect();
    if energias.is_empty() {
        return Vec::new();
    }
    let mut ordenadas = energias.clone();
    ordenadas.sort_by(f64::total_cmp);
    let tipica = ordenadas[((ordenadas.len() as f64 * 0.6) as usize).min(ordenadas.len() - 1)];
    let umbral = UMBRAL_MINIMO.max(tipica * PARTE_DE_SILENCIO);
    let minimo_de_silencio = SILENCIO_MS / 20;
    let mut v = Vec::new();
    let mut calladas_desde: Option<usize> = None;
    for (k, &e) in energias.iter().enumerate() {
        let callada = e < umbral;
        match (callada, calladas_desde) {
            (true, None) => calladas_desde = Some(k),
            (false, Some(desde)) => {
                if k - desde >= minimo_de_silencio {
                    v.push(desde * VENTANA..k * VENTANA);
                }
                calladas_desde = None;
            }
            _ => {}
        }
    }
    v
}

/// Donde se corta un silencio: en su mitad, redondeada a la ventana.
fn mitad(s: &Range<usize>) -> usize {
    (s.start / VENTANA + s.end / VENTANA) / 2 * VENTANA
}

/// **Donde hay una pausa de `ms` o mas**: la mitad de cada silencio largo,
/// que es justo donde [`cortes`] parte. Con dos idiomas, ahi se cierra la
/// tanda (ver [`agrupar_por_frases`]).
pub fn pausas_largas(pcm: &[i16], ms: usize) -> Vec<usize> {
    silencios(pcm)
        .iter()
        .filter(|s| s.len() >= ms * MUESTRAS_POR_MS)
        .map(mitad)
        .collect()
}

fn rms(v: &[i16]) -> f64 {
    let suma: f64 = v.iter().map(|&x| (x as f64) * (x as f64)).sum();
    (suma / v.len().max(1) as f64).sqrt()
}

/// El principio de la ventana de 20 ms con menos energia entre `a` y `b`,
/// probando cada media ventana. Si no cabe ni una ventana, `b`.
fn punto_mas_callado(pcm: &[i16], a: usize, b: usize) -> usize {
    if b - a <= VENTANA {
        return b;
    }
    let mut mejor = b;
    let mut menor = f64::MAX;
    let mut i = a;
    while i + VENTANA <= b {
        let e: f64 = pcm[i..i + VENTANA]
            .iter()
            .map(|&x| (x as f64) * (x as f64))
            .sum();
        if e < menor {
            menor = e;
            mejor = i;
        }
        i += VENTANA / 2;
    }
    mejor
}

/// **Frases seguidas juntas en tandas de hasta `segundos`**: cada tanda es
/// un tramo contiguo, de donde empieza su primera frase a donde acaba la
/// ultima, con las pausas de en medio. Una frase que sola pase del tope va
/// sola. `Transcriptor.agrupar`.
pub fn agrupar(trozos: &[Range<usize>], segundos: usize) -> Vec<Range<usize>> {
    let tope = segundos * HERCIOS as usize;
    let mut salida = Vec::new();
    let mut tanda: Option<Range<usize>> = None;
    for t in trozos {
        tanda = match tanda {
            Some(r) if t.end - r.start > tope => {
                salida.push(r);
                Some(t.clone())
            }
            Some(r) => Some(r.start..t.end),
            None => Some(t.clone()),
        };
    }
    salida.extend(tanda);
    salida
}

/// Como [`agrupar`], pero **una pausa larga tambien cierra la tanda** (B6).
///
/// Con dos idiomas, Whisper decide el idioma **por tanda**: una tanda que
/// junta el final de una frase en castellano con una frase entera en ingles
/// sale toda en uno de los dos, y la otra se pierde o se traduce (medido con
/// una nota sintetica castellano-ingles-castellano: sin esto, la frase en
/// ingles desaparecia). Quien cambia de idioma casi siempre lo hace entre
/// frases, tras una pausa; las comas y respiros de dentro de una frase son
/// mas cortos. `pausas` son los puntos de corte de las pausas largas
/// ([`pausas_largas`]): una tanda no cruza ninguno.
pub fn agrupar_por_frases(
    trozos: &[Range<usize>],
    segundos: usize,
    pausas: &[usize],
) -> Vec<Range<usize>> {
    let tope = segundos * HERCIOS as usize;
    let cruza =
        |r: &Range<usize>, t: &Range<usize>| pausas.iter().any(|&p| p > r.start && p <= t.start);
    let mut salida = Vec::new();
    let mut tanda: Option<Range<usize>> = None;
    for t in trozos {
        tanda = match tanda {
            Some(r) if t.end - r.start > tope || cruza(&r, t) => {
                salida.push(r);
                Some(t.clone())
            }
            Some(r) => Some(r.start..t.end),
            None => Some(t.clone()),
        };
    }
    salida.extend(tanda);
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn con_dos_idiomas_una_pausa_larga_empieza_tanda_y_una_coma_no() {
        let s = |a: f32, b: f32| (a * S as f32) as usize..(b * S as f32) as usize;
        // Frase 1 (partida por una coma en 3,1) y, tras una pausa larga
        // cortada en 7,7, la frase 2. Los trozos van seguidos: `cortes`
        // parte en mitad de cada silencio.
        let trozos = [s(0.0, 3.1), s(3.1, 7.7), s(7.7, 12.0)];
        let pausas = [s(7.7, 7.7).start];
        assert_eq!(
            agrupar_por_frases(&trozos, 20, &pausas),
            vec![s(0.0, 7.7), s(7.7, 12.0)]
        );
        // Sin la regla de la pausa, las tres irian juntas.
        assert_eq!(agrupar(&trozos, 20), vec![s(0.0, 12.0)]);
        // Y el tope de segundos sigue mandando.
        assert_eq!(
            agrupar_por_frases(&trozos, 5, &[]),
            vec![s(0.0, 3.1), s(3.1, 7.7), s(7.7, 12.0)]
        );
        assert!(agrupar_por_frases(&[], 20, &pausas).is_empty());
    }

    #[test]
    fn las_pausas_largas_son_las_de_mas_de_lo_pedido_y_caen_donde_corta() {
        // Frase, 0,4 s de silencio, frase, 1,5 s de silencio, frase.
        let mut pcm = tono(2.0);
        pcm.extend(silencio(0.4));
        pcm.extend(tono(2.0));
        pcm.extend(silencio(1.5));
        pcm.extend(tono(2.0));
        let p = pausas_largas(&pcm, 700);
        assert_eq!(p.len(), 1, "{p:?}");
        let cortes = cortes(&pcm);
        assert!(
            cortes.iter().any(|c| c.start == p[0]),
            "la pausa cae en un corte"
        );
        assert_eq!(pausas_largas(&pcm, 300).len(), 2);
        assert!(pausas_largas(&[], 700).is_empty());
    }

    const S: usize = HERCIOS as usize;

    fn tono(segundos: f32) -> Vec<i16> {
        let n = (segundos * S as f32) as usize;
        (0..n)
            .map(|i| ((i as f32 * 0.2).sin() * 8_000.0) as i16)
            .collect()
    }

    fn silencio(segundos: f32) -> Vec<i16> {
        vec![0; (segundos * S as f32) as usize]
    }

    #[test]
    fn dos_frases_con_medio_segundo_de_silencio_en_medio_son_dos_cortes() {
        let mut pcm = tono(1.0);
        pcm.extend(silencio(0.5));
        pcm.extend(tono(1.0));
        let c = cortes(&pcm);
        assert_eq!(c.len(), 2, "{c:?}");
        // El corte cae en mitad del silencio (1,25 s).
        assert!(
            (c[0].end as i64 - (1.25 * S as f32) as i64).abs() < 400,
            "{c:?}"
        );
        assert_eq!(c[1].end, pcm.len());
    }

    #[test]
    fn un_silencio_corto_no_parte_la_frase() {
        let mut pcm = tono(1.0);
        pcm.extend(silencio(0.1));
        pcm.extend(tono(1.0));
        assert_eq!(cortes(&pcm).len(), 1);
    }

    #[test]
    fn una_frase_de_veinte_segundos_sin_respirar_se_parte_en_trozos_de_ocho_como_mucho() {
        let pcm = tono(20.0);
        let c = cortes(&pcm);
        assert!(c.len() >= 3, "{c:?}");
        assert!(c.iter().all(|r| r.len() <= TROZO), "{c:?}");
        // Sin huecos: se cubre la nota entera.
        assert_eq!(c.first().unwrap().start, 0);
        assert_eq!(c.last().unwrap().end, pcm.len());
    }

    #[test]
    fn un_golpe_de_menos_de_un_tercio_de_segundo_no_es_una_frase() {
        let mut pcm = silencio(1.0);
        pcm.extend(tono(0.1));
        pcm.extend(silencio(1.0));
        let c = cortes(&pcm);
        // Quedan los tramos con el silencio alrededor, pero ninguno de
        // menos de 0,3 s.
        assert!(c.iter().all(|r| r.len() >= MINIMO), "{c:?}");
    }

    #[test]
    fn un_audio_vacio_no_tiene_cortes_y_uno_demasiado_corto_para_medir_va_entero() {
        assert!(cortes(&[]).is_empty());
        // Menos de una ventana de 20 ms: no hay energia que medir y va
        // entero, igual que en el movil (`if (energias.isEmpty())`).
        assert_eq!(cortes(&[100i16; 160]), vec![0..160]);
    }

    #[test]
    fn las_frases_se_juntan_hasta_veinte_segundos_y_la_que_pasa_empieza_otra_tanda() {
        let frases = [0..5 * S, 6 * S..12 * S, 13 * S..19 * S, 21 * S..25 * S];
        let t = agrupar(&frases, 20);
        assert_eq!(t, vec![0..19 * S, 21 * S..25 * S]);
    }

    #[test]
    fn una_frase_que_sola_pasa_del_tope_va_sola() {
        let frases = [0..S, 2 * S..40 * S, 41 * S..42 * S];
        let t = agrupar(&frases, 20);
        assert_eq!(t, vec![0..S, 2 * S..40 * S, 41 * S..42 * S]);
        assert!(agrupar(&[], 20).is_empty());
    }
}
