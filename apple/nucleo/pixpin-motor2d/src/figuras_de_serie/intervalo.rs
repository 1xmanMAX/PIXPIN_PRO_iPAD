//! **Alargar una plantilla** (22-oct-2026). El usuario elige en el lienzo
//! una plantilla de serie y pide «damela del 1 al 40»: sale la misma hoja,
//! mas larga, con los numeros del 1 al 40. Si ademas dice «de 10 en 10»,
//! salen cuatro hojas iguales (1–10, 11–20...) con su pie «Hoja 2 de 4».
//!
//! Sus palabras: «no necesariamente cree una nueva lista o elementos con la
//! continuacion, puedo desde el principio pedirle seleccionando el templete
//! y decirle damelo con intervalos de 1 al 40 y que lo genere pero mas
//! grande en la misma hoja; pero si le digo que lo divida de 10 en 10 que me
//! de 4 hojas».
//!
//! Lo que cuenta cada plantilla (filas, horas, dias, viñetas...) lo dice
//! `plantillas::AMPLIABLES`; aqui se comprueba lo pedido, se parte en hojas
//! y se reconoce que plantilla es la elegida (por su titulo).
//!
//! **Y personalizarla** (25-oct): con `secciones`, la misma hoja sale con
//! otra lista de secciones (quitar, reordenar, cambiar lineas, a todo el
//! ancho; ver `secciones.rs`), sola o junto con el intervalo.

use std::sync::OnceLock;

use super::noche::{self, SUFIJO_NOCHE};
use super::plantillas::{self, AMPLIABLES, MESES, PERSONALIZABLES, Tramo, VINETA_DE_4};
use super::secciones::{self, Seccion, SeccionPedida};
use super::{PREFIJO_ID, SEPARADOR};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{Elemento, Figura};

/// Cuantas hojas como mucho de una vez.
pub const MAX_HOJAS: usize = 60;

/// **Lo que se pide.** Lo que falta se queda como en la de serie.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Intervalo {
    pub desde: Option<u32>,
    pub hasta: Option<u32>,
    /// «De 10 en 10»: cuantos por hoja.
    pub de_a: Option<u32>,
    /// Las semanas del cronograma (su segunda serie).
    pub semanas: Option<(u32, u32)>,
    /// `(ano, mes 1-12)`: el planificador mensual y los habitos, con el
    /// calendario de verdad.
    pub mes: Option<(i32, u32)>,
    /// Cuantos meses seguidos desde `mes`, una hoja cada uno.
    pub meses: Option<u32>,
    /// **Las secciones**, en su orden (las que se personalizan): lo que no
    /// diga cada una se toma de la de fabrica con su titulo.
    pub secciones: Option<Vec<SeccionPedida>>,
    /// **El modo noche** (25-oct, `noche.rs`): `Some(true)` la hoja sale en
    /// oscuro, `Some(false)` de dia, `None` como este la que se rehace (la
    /// app lo completa con [`es_de_noche`](super::es_de_noche)) o de dia si
    /// es nueva. Solo con el modo («ponlo en modo noche») sale la misma
    /// hoja de serie en el otro modo.
    pub noche: Option<bool>,
}

impl Intervalo {
    /// Si pide numeros (y no solo secciones).
    fn pide_numeros(&self) -> bool {
        self.desde.is_some()
            || self.hasta.is_some()
            || self.de_a.is_some()
            || self.semanas.is_some()
            || self.mes.is_some()
            || self.meses.is_some()
    }
}

/// Lo que se sabe de cada plantilla de pagina: su clave, su titulo (lo que
/// se lee en la hoja) y su nombre en el menu.
struct Ficha {
    clave: String,
    titulo: String,
    nombre: String,
}

fn fichas() -> &'static [Ficha] {
    static F: OnceLock<Vec<Ficha>> = OnceLock::new();
    F.get_or_init(|| {
        super::catalogo()
            .into_iter()
            .filter_map(|f| {
                // La gemela de noche es la misma plantilla: una ficha basta.
                if f.id.ends_with(SUFIJO_NOCHE) {
                    return None;
                }
                let titulo = titulo_de(&f.elementos)?;
                let clave = f.id.strip_prefix(PREFIJO_ID)?.to_string();
                let nombre =
                    super::categoria_de(&f.nombre).map_or(f.nombre.clone(), |(_, n)| n.to_string());
                Some(Ficha {
                    clave,
                    titulo: titulo.to_string(),
                    nombre,
                })
            })
            .collect()
    })
}

/// El titulo de una hoja: su texto grande.
fn titulo_de(elementos: &[Elemento]) -> Option<&str> {
    elementos
        .iter()
        .filter(|e| !e.borrado)
        .filter_map(|e| match &e.figura {
            Figura::Texto { texto, tam, .. } if *tam >= 30.0 => Some((texto.as_str(), *tam)),
            _ => None,
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(t, _)| t)
}

/// **Que plantilla de serie es** lo elegido (su clave), por el titulo de la
/// hoja. Los dos storyboard se titulan igual: el de 4 tiene las viñetas de
/// [`VINETA_DE_4`] de ancho. La de noche es la misma (su titulo es el
/// mismo); si es de noche lo dice [`es_de_noche`](super::es_de_noche).
pub fn cual_es(elementos: &[Elemento]) -> Option<String> {
    let titulo = titulo_de(elementos)?;
    let f = fichas().iter().find(|f| f.titulo == titulo)?;
    if f.clave.starts_with("storyboard-") {
        let de_4 = elementos
            .iter()
            .any(|e| matches!(e.figura, Figura::Rectangulo) && (e.ancho - VINETA_DE_4).abs() < 0.5);
        return Some(if de_4 { "storyboard-4" } else { "storyboard-6" }.to_string());
    }
    Some(f.clave.clone())
}

/// El nombre de una plantilla en el menu («Lista de tareas»).
pub fn nombre_de(clave: &str) -> String {
    fichas()
        .iter()
        .find(|f| f.clave == clave)
        .map_or_else(|| clave.to_string(), |f| f.nombre.clone())
}

/// La clave de una plantilla de serie por su id (`serie-<clave>`, o
/// `serie-<clave>-noche` la de noche: la misma clave).
pub fn clave_de(f: &FiguraGuardada) -> Option<&str> {
    let c = f.id.strip_prefix(PREFIJO_ID)?;
    Some(c.strip_suffix(SUFIJO_NOCHE).unwrap_or(c))
}

/// Si es la gemela de noche de una plantilla de serie (por su id).
pub fn es_la_de_noche(f: &FiguraGuardada) -> bool {
    f.id.starts_with(PREFIJO_ID) && f.id.ends_with(SUFIJO_NOCHE)
}

/// **El mes de una hoja que va por meses** (el mensual y los habitos con
/// el calendario de verdad), por lo que lleva escrito: «Noviembre» y
/// «2026» en la cabecera, o «Noviembre 2026». `None` si no dice ninguno.
pub fn mes_de_la_hoja(elementos: &[Elemento]) -> Option<(i32, u32)> {
    let textos: Vec<&str> = elementos
        .iter()
        .filter(|e| !e.borrado)
        .filter_map(|e| match &e.figura {
            Figura::Texto { texto, .. } => Some(texto.trim()),
            _ => None,
        })
        .collect();
    let mes = |t: &str| MESES.iter().position(|m| *m == t).map(|i| i as u32 + 1);
    let ano = |t: &str| {
        t.parse::<i32>()
            .ok()
            .filter(|a| t.len() == 4 && (1900..=2200).contains(a))
    };
    // «Noviembre 2026» en un solo texto (los habitos).
    for t in &textos {
        if let Some((m, a)) = t.split_once(' ')
            && let (Some(m), Some(a)) = (mes(m), ano(a))
        {
            return Some((a, m));
        }
    }
    let m = textos.iter().find_map(|t| mes(t))?;
    let a = textos.iter().find_map(|t| ano(t))?;
    Some((a, m))
}

/// Si esa plantilla se puede alargar.
pub fn se_alarga(clave: &str) -> bool {
    AMPLIABLES.iter().any(|a| a.clave == clave)
}

/// Las que se alargan y lo que cuenta cada una, para el modelo y los avisos:
/// «Cronograma (filas, y semanas), Planificador diario (horas)...».
pub fn las_que_se_alargan() -> String {
    AMPLIABLES
        .iter()
        .map(|a| {
            let extra = match a.clave {
                "cronograma" => ", y semanas",
                "habitos" => ", o un mes",
                _ => "",
            };
            let cuenta = if a.clave == "planificador-mensual" {
                "un mes y cuántos"
            } else {
                a.cuenta
            };
            format!("{} ({cuenta}{extra})", nombre_de(a.clave))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// **Las hojas pedidas**, cada una normalizada al origen y con el tramo en
/// el nombre («Planificación · Lista de tareas (1–40)»). Error, en palabras
/// para el usuario, si la plantilla no se alarga o lo pedido no cabe. Con
/// `noche`, las hojas salen de noche (y «· noche» en el nombre).
pub fn alargar(clave: &str, iv: &Intervalo) -> Result<Vec<FiguraGuardada>, String> {
    let mut hojas = match iv.noche {
        // Solo el modo: la misma hoja de serie, de dia o de noche.
        Some(_) if !iv.pide_numeros() && iv.secciones.is_none() => vec![de_serie(clave)?],
        _ => de_dia(clave, iv)?,
    };
    if iv.noche == Some(true) {
        for h in &mut hojas {
            noche::pintar(&mut h.elementos);
            h.id.push_str(SUFIJO_NOCHE);
            h.nombre = match h.nombre.strip_suffix(')') {
                Some(n) => format!("{n} · noche)"),
                None => format!("{} (noche)", h.nombre),
            };
        }
    }
    Ok(hojas)
}

/// La hoja de serie de cualquier plantilla de pagina, con «(de serie)» en
/// el nombre (la de noche la pinta [`alargar`]).
fn de_serie(clave: &str) -> Result<FiguraGuardada, String> {
    let f = match plantillas::hacer(clave, &plantillas::tramo_de_fabrica(clave)) {
        Some(f) => f,
        None => super::catalogo()
            .into_iter()
            .find(|f| f.id == format!("{PREFIJO_ID}{clave}") && titulo_de(&f.elementos).is_some())
            .ok_or_else(|| format!("«{}» no es una plantilla de página", nombre_de(clave)))?,
    };
    Ok(con_nombre(f, "de serie"))
}

/// Las hojas pedidas, de dia.
fn de_dia(clave: &str, iv: &Intervalo) -> Result<Vec<FiguraGuardada>, String> {
    let secs = match &iv.secciones {
        Some(pedidas) => {
            if !se_personaliza(clave) {
                return Err(format!(
                    "«{}» no tiene secciones que cambiar. Se personalizan: {}",
                    nombre_de(clave),
                    las_que_se_personalizan()
                ));
            }
            Some(plantillas::secciones_pedidas(
                clave,
                &nombre_de(clave),
                pedidas,
            )?)
        }
        None => None,
    };
    let secs = secs.as_deref();
    // Solo secciones: una hoja como la de serie, con esas secciones.
    if secs.is_some() && !iv.pide_numeros() {
        let tr = plantillas::tramo_de_fabrica(clave);
        return Ok(vec![con_nombre(
            plantillas::hacer_con(clave, &tr, secs).expect("es de la tabla"),
            "a medida",
        )]);
    }
    let Some(a) = AMPLIABLES.iter().find(|a| a.clave == clave) else {
        let extra = if se_personaliza(clave) {
            " (de esta solo se cambian las secciones: manda solo «secciones»)"
        } else {
            ""
        };
        return Err(format!(
            "«{}» no lleva números que alargar{extra}. Se alargan: {}",
            nombre_de(clave),
            las_que_se_alargan()
        ));
    };
    if let Some(mes) = iv.mes {
        return por_meses(clave, mes, iv.meses.unwrap_or(1), secs);
    }
    if clave == "planificador-mensual" {
        return Err(
            "el planificador mensual va por meses: di el mes y el año (y cuántos meses seguidos)"
                .into(),
        );
    }
    if iv.semanas.is_some() && clave != "cronograma" {
        return Err("solo el cronograma lleva semanas".into());
    }
    let desde = iv.desde.unwrap_or(a.defecto.0);
    let hasta = iv.hasta.unwrap_or(a.defecto.1.max(desde));
    if hasta < desde {
        return Err(format!("el intervalo va al revés: del {desde} al {hasta}"));
    }
    if desde < a.minimo || hasta > a.maximo {
        return Err(format!(
            "«{}» va del {} al {} como mucho ({})",
            nombre_de(clave),
            a.minimo,
            a.maximo,
            a.cuenta
        ));
    }
    let semanas = match iv.semanas {
        Some((s0, s1)) if s0 == 0 || s1 < s0 || s1 - s0 >= 260 => {
            return Err(format!(
                "las semanas van al revés o son demasiadas: de la {s0} a la {s1}"
            ));
        }
        Some(s) => s,
        None => (1, 12),
    };
    // Lo que se parte: las filas, salvo que solo se hayan pedido semanas.
    let partir_semanas = iv.semanas.is_some() && iv.desde.is_none() && iv.hasta.is_none();
    let (lo, hi) = if partir_semanas {
        semanas
    } else {
        (desde, hasta)
    };
    let trozos = trozos(lo, hi, iv.de_a)?;
    let n = trozos.len() as u32;
    Ok(trozos
        .into_iter()
        .enumerate()
        .map(|(i, (t0, t1))| {
            let mut tr = Tramo::de(desde, hasta);
            tr.semanas = semanas;
            if partir_semanas {
                tr.semanas = (t0, t1);
            } else {
                (tr.desde, tr.hasta) = (t0, t1);
            }
            tr.hoja = (n > 1).then_some((i as u32 + 1, n));
            let rotulo = if partir_semanas {
                format!("semanas {t0}–{t1}")
            } else {
                tr.rotulo()
            };
            // Con secciones y filas pedidas, la de la serie lleva las del tramo.
            let propias = secs.map(|s| {
                if iv.desde.is_some() || iv.hasta.is_some() || iv.de_a.is_some() {
                    plantillas::con_la_serie(clave, s, &tr)
                } else {
                    s.to_vec()
                }
            });
            con_nombre(
                plantillas::hacer_con(clave, &tr, propias.as_deref()).expect("es de la tabla"),
                &a_medida(&rotulo, secs),
            )
        })
        .collect())
}

/// `lo..=hi` en trozos de `de_a` (el ultimo, lo que quede).
fn trozos(lo: u32, hi: u32, de_a: Option<u32>) -> Result<Vec<(u32, u32)>, String> {
    let Some(k) = de_a.filter(|&k| k > 0 && k <= hi - lo) else {
        return Ok(vec![(lo, hi)]);
    };
    let v: Vec<(u32, u32)> = (lo..=hi)
        .step_by(k as usize)
        .map(|a| (a, (a + k - 1).min(hi)))
        .collect();
    if v.len() > MAX_HOJAS {
        return Err(format!(
            "serían {} hojas; como mucho {MAX_HOJAS} de una vez",
            v.len()
        ));
    }
    Ok(v)
}

/// Una hoja por mes, desde `mes`: el mensual con sus dias en su sitio, o
/// los habitos con tantas columnas como dias tenga el mes.
fn por_meses(
    clave: &str,
    (ano, mes): (i32, u32),
    cuantos: u32,
    secs: Option<&[Seccion]>,
) -> Result<Vec<FiguraGuardada>, String> {
    if !matches!(clave, "planificador-mensual" | "habitos") {
        return Err(format!(
            "«{}» no va por meses; por meses van el planificador mensual y los hábitos",
            nombre_de(clave)
        ));
    }
    if !(1..=12).contains(&mes) || !(1900..=2200).contains(&ano) {
        return Err(format!("ese mes no existe: {mes}/{ano}"));
    }
    let cuantos = cuantos.max(1);
    if cuantos as usize > MAX_HOJAS {
        return Err(format!(
            "serían {cuantos} hojas; como mucho {MAX_HOJAS} de una vez"
        ));
    }
    Ok((0..cuantos)
        .map(|i| {
            let k = mes - 1 + i;
            let (a, m) = (ano + (k / 12) as i32, k % 12 + 1);
            let mut tr = Tramo::de(
                1,
                if clave == "habitos" {
                    plantillas::dias_del_mes(a, m)
                } else {
                    1
                },
            );
            tr.mes = Some((a, m));
            tr.hoja = (cuantos > 1).then_some((i + 1, cuantos));
            let rotulo = format!("{} {a}", MESES[m as usize - 1]);
            con_nombre(
                plantillas::hacer_con(clave, &tr, secs).expect("es de la tabla"),
                &a_medida(&rotulo, secs),
            )
        })
        .collect())
}

/// «1–40 · a medida» si lleva secciones pedidas.
fn a_medida(rotulo: &str, secs: Option<&[Seccion]>) -> String {
    match secs {
        Some(_) => format!("{rotulo} · a medida"),
        None => rotulo.to_string(),
    }
}

/// El tramo en el nombre: la hoja guardada en figuras se distingue de la de
/// serie y dice que lleva.
fn con_nombre(mut f: FiguraGuardada, rotulo: &str) -> FiguraGuardada {
    f.nombre = format!("{} ({rotulo})", f.nombre);
    debug_assert!(f.nombre.contains(SEPARADOR));
    f
}

/// **Que cuenta la serie de una plantilla** que se alarga («filas»,
/// «horas», «meses»...); `None` si no se alarga.
pub fn cuenta_de(clave: &str) -> Option<&'static str> {
    AMPLIABLES
        .iter()
        .find(|a| a.clave == clave)
        .map(|a| a.cuenta)
}

/// El tramo de la de serie de una plantilla que se alarga («1–42»).
pub fn tramo_de_serie(clave: &str) -> Option<(u32, u32)> {
    AMPLIABLES
        .iter()
        .find(|a| a.clave == clave)
        .map(|a| a.defecto)
}

// ---------------------------------------------------------------------------
// Las secciones, para el modelo
// ---------------------------------------------------------------------------

/// Si esa plantilla se personaliza por secciones.
pub fn se_personaliza(clave: &str) -> bool {
    plantillas::personalizable(clave).is_some()
}

/// Las que se personalizan, por su nombre: «Planificador mensual,
/// Planificador semanal...».
pub fn las_que_se_personalizan() -> String {
    PERSONALIZABLES
        .iter()
        .map(|p| nombre_de(p.clave))
        .collect::<Vec<_>>()
        .join(", ")
}

/// **Las secciones de fabrica de una plantilla**, en su orden (lo que dibuja
/// la de serie). `None` si no se personaliza.
pub fn secciones_de(clave: &str) -> Option<Vec<Seccion>> {
    let p = plantillas::personalizable(clave)?;
    Some((p.fabrica)(&plantillas::tramo_de_fabrica(clave)))
}

/// **La guia de una plantilla, por bloques**, para el modelo: que bloques
/// tiene (los fijos y la zona de secciones), que es cada seccion con su
/// tipo, lineas y ancho, sus bloques propios, como se colocan y que se le
/// puede cambiar, y sus secciones en JSON para devolverlas cambiadas con
/// `figuras_guardadas` «personalizar». En el lienzo, cada bloque lleva el
/// subgrupo `parte:<nombre>`. `None` si no se personaliza.
pub fn guia_de(clave: &str) -> Option<String> {
    let p = plantillas::personalizable(clave)?;
    let secs = secciones_de(clave)?;
    let tipo_en_palabras = |s: &Seccion| -> String {
        use secciones::TipoSeccion as T;
        match s.tipo {
            T::Renglones => "renglones para escribir".into(),
            T::Numerados => "renglones numerados 01, 02…".into(),
            T::Casillas => "casillas para marcar".into(),
            T::Recuadro => "recuadro de fondo suave para destacar".into(),
            T::Libre => "recuadro en blanco (baja hasta el pie si no tiene nada debajo)".into(),
            T::Notas => "zona gris para anotar".into(),
            T::Propia => p
                .propios
                .iter()
                .find(|b| secciones::normalizar(b.titulo) == secciones::normalizar(&s.titulo))
                .map_or("bloque propio".into(), |b| {
                    format!("bloque propio: {}", b.que_es)
                }),
        }
    };
    let lista = if secs.is_empty() {
        "  (ninguna de serie)".to_string()
    } else {
        secs.iter()
            .map(|s| {
                format!(
                    "  - «{}»: {} · tipo {}, {} líneas, ancho {}",
                    s.titulo,
                    tipo_en_palabras(s),
                    s.tipo.nombre(),
                    s.lineas,
                    s.ancho.nombre()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let propios = if p.propios.is_empty() {
        String::new()
    } else {
        format!(
            "\nBloques propios (tipo «propia», solo con su título; sus líneas son sus filas): {}.",
            p.propios
                .iter()
                .map(|b| format!("«{}» ({})", b.titulo, b.que_es))
                .collect::<Vec<_>>()
                .join("; ")
        )
    };
    let lateral = if p.lateral {
        "\nTiene columna lateral: «ancho»:\"lateral\" pone la sección a la derecha de lo fijo (la hoja se ensancha lo justo)."
    } else {
        ""
    };
    Some(format!(
        "Plantilla «{}». Bloques: {}.\nSecciones de serie, en su orden:\n{lista}{propios}{lateral}\n\
         Se puede: quitar una (no la pongas), cambiar sus líneas, reordenarlas, cambiar su ancho o su tipo, o añadir otras. \
         Tipos: renglones, numerados, casillas, recuadro (fondo suave), libre (recuadro en blanco), notas (zona gris para anotar), propia. \
         Ancho: completo (fila entera debajo de todo), mitad (de dos en dos alineadas arriba), izquierda / derecha (apiladas en esa columna). \
         Líneas de 1 a {}, como mucho {} secciones; si no caben, la hoja crece.\n\
         Para cambiarla, figuras_guardadas «personalizar» con la lista ENTERA en el orden nuevo (de una que ya existe basta el «titulo» y lo que cambie): {}",
        nombre_de(p.clave),
        p.bloques,
        secciones::MAX_LINEAS,
        secciones::MAX_SECCIONES,
        secciones::en_json(&secs),
    ))
}
