//! **Que no se acabe la ventana de contexto**: medir, reiniciar a tiempo y
//! buscar lo viejo en vez de tenerlo cargado.
//!
//! Pedido del usuario (13-oct-2026): «para que no se le acabe el contexto,
//! cierra esa conversacion y empieza de nuevo con lo que descifro pensando».
//! Con el 2B en 8192 tokens, el catalogo y una foto ya se comen ~1600 y una
//! respuesta con dibujo pide hasta 2048: una conversacion de una docena de
//! mensajes largos llega al limite, y ahi `llama-server` corta o falla.
//!
//! Lo que se hace:
//!
//! 1. **Medir** ([`estimar`]): una cuenta por letras (el tokenizador de Qwen
//!    da ~3,5 letras por token en espanol; se cuenta algo por encima) mas
//!    las imagenes y el catalogo. No hace falta exactitud: se mira contra el
//!    70 %, no contra el borde.
//! 2. **Reiniciar al 70 %** ([`reiniciar`]), sin esperar al limite: el turno
//!    arranca con las instrucciones + la hoja de trabajo (que ya lleva el
//!    objetivo mejorado) + los 2 ultimos mensajes + el nuevo. Dentro de un
//!    turno largo (muchas herramientas), [`podar_en_turno`] recorta los
//!    resultados viejos de herramientas, que son lo que mas pesa (un OCR).
//!    Desde el 22-oct, en tres escalones: desde el 50 %, los resultados de
//!    herramientas viejos se despejan ([`despejar_resultados`], quedan los
//!    3 ultimos); al 70 %, antes de reiniciar, el modelo guarda lo duradero
//!    en la memoria de largo plazo (`memoria::guardar_lo_duradero`); los
//!    mensajes crudos siguen en el chat, buscables.
//! 3. **Buscar lo viejo** ([`buscar`], herramienta `buscar_historial`): el
//!    modelo pide «lo que dije de las ventas» y recibe los trozos que mas se
//!    parecen de la conversacion guardada.

use serde_json::json;

use crate::herramientas::Herramienta;
use crate::hoja_trabajo::HojaDeTrabajo;
use crate::motor::{Mensaje, Parte, Rol};
use crate::pensar::{Plan, cortar, sin_tildes};

/// Al pasar de esta fraccion de la ventana, se reinicia.
pub const UMBRAL: f32 = 0.70;

/// Letras por token, por lo bajo (asi se cuentan tokens de mas, no de menos).
const LETRAS_POR_TOKEN: f32 = 3.2;
/// Lo que se le cuenta a una imagen (Qwen3.5 a ~768 px: ~300-450 tokens).
const TOKENS_IMAGEN: u32 = 450;
/// Lo que anade la plantilla por mensaje (rol, separadores).
const TOKENS_POR_MENSAJE: u32 = 6;

/// Lo que se cuenta para la respuesta al medir contra el 70 %: una
/// respuesta normal. No el tope entero de una vuelta con herramientas
/// (`agente::MAX_TOKENS_CON_HERRAMIENTAS`): ese ya lo encoge
/// `agente::tope_de_tokens` a lo que quepa, y contarlo entero reiniciaria
/// casi en cada turno.
pub const RESERVA_RESPUESTA: u32 = 1536;

/// Cuantos mensajes de antes se quedan al reiniciar.
pub const QUEDAN_AL_REINICIAR: usize = 2;

/// La marca de un resultado de herramienta ya recortado.
const RECORTADO: &str =
    "(resultado recortado para ahorrar contexto; usa buscar_historial si lo necesitas)";

pub fn tokens_de_texto(s: &str) -> u32 {
    (s.chars().count() as f32 / LETRAS_POR_TOKEN).ceil() as u32
}

/// **Cuantos tokens ocupan** estos mensajes con este catalogo.
pub fn estimar(mensajes: &[Mensaje], herramientas: &[Herramienta]) -> u32 {
    let mut n: u32 = herramientas
        .iter()
        .map(|h| tokens_de_texto(&h.a_json().to_string()))
        .sum();
    for m in mensajes {
        n += TOKENS_POR_MENSAJE;
        for p in &m.partes {
            n += match p {
                Parte::Texto(t) => tokens_de_texto(t),
                Parte::Imagen { .. } => TOKENS_IMAGEN,
            };
        }
        for l in &m.llamadas {
            n += tokens_de_texto(&l.nombre) + tokens_de_texto(&l.argumentos) + 8;
        }
    }
    n
}

/// Si con `tokens` mas lo que se reserva para la respuesta se pasa del 70 %.
pub fn pasa_del_umbral(tokens: u32, reserva_respuesta: u32, ventana: u32) -> bool {
    ventana > 0 && (tokens + reserva_respuesta) as f32 > ventana as f32 * UMBRAL
}

/// Lo que [`sistema_con_hoja`] anade debajo de las instrucciones, en el
/// orden en que lo pone: de aqui en adelante cambia cada turno.
const PARTES_QUE_CAMBIAN: &[&str] = &[
    "\n\n# Hoja de trabajo\n",
    "\n\n# Peticion mejorada",
    "\n\nLa conversacion de antes ya no esta cargada",
];

/// **Parte el sistema** en lo fijo (las instrucciones) y lo que cambia cada
/// turno (la hoja, la peticion mejorada), para que lo fijo sea un prefijo
/// que se cachea (`nube::instrucciones::en_dos`).
pub fn partir_sistema(s: &str) -> (&str, &str) {
    match PARTES_QUE_CAMBIAN.iter().filter_map(|m| s.find(m)).min() {
        Some(i) => s.split_at(i),
        None => (s, ""),
    }
}

/// **Las instrucciones con la hoja y la peticion mejorada** debajo.
pub fn sistema_con_hoja(instrucciones: &str, hoja: &HojaDeTrabajo, plan: Option<&Plan>) -> String {
    let mut s = instrucciones.trim_end().to_string();
    if !hoja.esta_vacia() {
        s.push_str("\n\n");
        s.push_str(hoja.a_texto().trim_end());
        s.push_str("\nSi lo que te piden ya esta en la hoja de trabajo, contesta directamente con ello, sin herramientas.");
    }
    if let Some(p) = plan {
        s.push_str("\n\n# Peticion mejorada (lo que entendiste antes de actuar)\n");
        s.push_str(&p.lineas());
    }
    if hoja.reinicios > 0 || hoja.reinicio_desde.is_some() {
        s.push_str(
            "\n\nLa conversacion de antes ya no esta cargada: si necesitas algo de ella, usa buscar_historial.",
        );
    }
    s
}

/// Pone (o cambia) las instrucciones de sistema al principio del historial.
pub fn poner_sistema(historial: &mut Vec<Mensaje>, texto: String) {
    match historial.first_mut() {
        Some(m) if m.rol == Rol::Sistema => *m = Mensaje::sistema(texto),
        _ => historial.insert(0, Mensaje::sistema(texto)),
    }
}

/// **El historial reiniciado**: el sistema (que ya debe llevar la hoja), los
/// [`QUEDAN_AL_REINICIAR`] ultimos mensajes de antes del ultimo del usuario
/// y ese ultimo con lo que venga detras. Ningun resultado de herramienta
/// queda huerfano de su llamada.
pub fn reiniciar(historial: &[Mensaje]) -> Vec<Mensaje> {
    let mut salida = Vec::new();
    let resto = match historial.first() {
        Some(m) if m.rol == Rol::Sistema => {
            salida.push(m.clone());
            &historial[1..]
        }
        _ => historial,
    };
    let Some(u) = resto.iter().rposition(|m| m.rol == Rol::Usuario) else {
        salida.extend(resto.iter().cloned());
        return salida;
    };
    // De lo de antes, solo texto: usuario y asistente sin llamadas.
    let antes: Vec<&Mensaje> = resto[..u]
        .iter()
        .filter(|m| matches!(m.rol, Rol::Usuario | Rol::Asistente) && m.llamadas.is_empty())
        .filter(|m| !m.texto_entero().trim().is_empty())
        .collect();
    let desde = antes.len().saturating_sub(QUEDAN_AL_REINICIAR);
    salida.extend(antes[desde..].iter().map(|m| (*m).clone()));
    salida.extend(resto[u..].iter().cloned());
    salida
}

/// Lo que queda para el modelo al empezar un turno.
#[derive(Debug, Clone, PartialEq)]
pub struct Preparado {
    pub historial: Vec<Mensaje>,
    /// Si hubo que reiniciar: la conversacion de antes ya no va entera.
    pub reiniciado: bool,
    /// Los tokens estimados de lo que va.
    pub tokens: u32,
    /// El primero de `anteriores` que va (el chat lo apunta en la hoja como
    /// el nuevo principio de la conversacion).
    pub primero: usize,
}

/// **El historial de un turno nuevo**: el sistema (con la hoja), los mensajes
/// de antes (`(es_del_usuario, texto)`, ya desde el ultimo reinicio) y el
/// nuevo. Si todo junto pasa del 70 % de la `ventana` con lo que se reserva
/// para contestar, se queda con los [`QUEDAN_AL_REINICIAR`] ultimos.
pub fn preparar(
    sistema: String,
    anteriores: &[(bool, String)],
    nuevo: Mensaje,
    herramientas: &[Herramienta],
    ventana: u32,
    reserva_respuesta: u32,
) -> Preparado {
    let montar = |desde: usize| -> Vec<Mensaje> {
        let mut h = vec![Mensaje::sistema(sistema.clone())];
        for (del_usuario, texto) in &anteriores[desde..] {
            if texto.trim().is_empty() {
                continue;
            }
            h.push(if *del_usuario {
                Mensaje::usuario(texto.clone())
            } else {
                Mensaje::texto(Rol::Asistente, texto.clone())
            });
        }
        h.push(nuevo.clone());
        h
    };
    let entero = montar(0);
    let tokens = estimar(&entero, herramientas);
    if !pasa_del_umbral(tokens, reserva_respuesta, ventana) {
        return Preparado {
            historial: entero,
            reiniciado: false,
            tokens,
            primero: 0,
        };
    }
    let con_texto: Vec<usize> = (0..anteriores.len())
        .filter(|&i| !anteriores[i].1.trim().is_empty())
        .collect();
    let primero = con_texto
        .get(con_texto.len().saturating_sub(QUEDAN_AL_REINICIAR))
        .copied()
        .unwrap_or(anteriores.len());
    let historial = montar(primero);
    Preparado {
        tokens: estimar(&historial, herramientas),
        historial,
        reiniciado: true,
        primero,
    }
}

/// **Dentro de un turno**: recorta los resultados de herramientas viejos
/// (todos menos los de la ultima llamada) a una linea. Devuelve si cambio
/// algo.
pub fn podar_en_turno(historial: &mut [Mensaje]) -> bool {
    let Some(ultima) = historial.iter().rposition(|m| !m.llamadas.is_empty()) else {
        return false;
    };
    let mut cambio = false;
    for m in historial[..ultima]
        .iter_mut()
        .filter(|m| m.rol == Rol::Herramienta)
    {
        let t = m.texto_entero();
        if t.chars().count() > 200 && !t.contains(RECORTADO) {
            let corto = format!("{} {RECORTADO}", cortar(&t, 160));
            m.partes = vec![Parte::Texto(corto)];
            cambio = true;
        }
    }
    cambio
}

// ---------------------------------------------------------------------------
// El primer escalon: despejar resultados viejos (22-oct)

/// **Desde esta fraccion** se quitan los resultados de herramientas viejos,
/// antes de llegar al reinicio del 70 %. Es lo mas barato (Anthropic,
/// `clear_tool_uses`): no pregunta a ningun modelo y lo que se quita se
/// puede volver a pedir.
pub const UMBRAL_DESPEJAR: f32 = 0.50;

/// Cuantos resultados de herramientas se quedan enteros al despejar (los
/// ultimos; el valor por omision de Anthropic).
pub const QUEDAN_RESULTADOS: usize = 3;

/// La marca de un resultado despejado.
const DESPEJADO: &str = "resultado quitado para ahorrar contexto";

/// Si con `tokens` mas la reserva se pasa de `fraccion` de la ventana.
pub fn pasa_de(tokens: u32, reserva_respuesta: u32, ventana: u32, fraccion: f32) -> bool {
    ventana > 0 && (tokens + reserva_respuesta) as f32 > ventana as f32 * fraccion
}

/// **Despeja los resultados de herramientas viejos**: todos menos los
/// `quedan` ultimos se quedan en una linea que dice de que herramienta era
/// (asi el modelo sabe que paso y puede repetir la llamada). Los cortos no
/// se tocan: no ahorran nada. Devuelve si cambio algo.
pub fn despejar_resultados(historial: &mut [Mensaje], quedan: usize) -> bool {
    let resultados: Vec<usize> = (0..historial.len())
        .filter(|&i| historial[i].rol == Rol::Herramienta)
        .collect();
    let viejos = &resultados[..resultados.len().saturating_sub(quedan)];
    let mut cambio = false;
    for &i in viejos {
        let t = historial[i].texto_entero();
        if t.chars().count() <= 200 || t.contains(DESPEJADO) || t.contains(RECORTADO) {
            continue;
        }
        let id = historial[i].id_llamada.clone();
        let nombre = historial[..i]
            .iter()
            .rev()
            .flat_map(|m| m.llamadas.iter())
            .find(|l| Some(&l.id) == id.as_ref())
            .map(|l| l.nombre.clone())
            .unwrap_or_else(|| "herramienta".into());
        historial[i].partes = vec![Parte::Texto(format!(
            "({nombre}: {DESPEJADO}; empezaba «{}». Repite la llamada si lo necesitas entero.)",
            cortar(&t, 80)
        ))];
        cambio = true;
    }
    cambio
}

// ---------------------------------------------------------------------------
// buscar_historial

pub const BUSCAR_HISTORIAL: &str = "buscar_historial";

/// La herramienta tal como se le describe al modelo.
pub fn herramienta_buscar() -> Herramienta {
    Herramienta {
        nombre: BUSCAR_HISTORIAL.into(),
        descripcion: "Busca en la conversacion guardada algo que ya no tienes delante (un dato, lo que se dijo o se hizo antes). Devuelve los trozos mas parecidos.".into(),
        parametros: json!({
            "type": "object",
            "required": ["consulta"],
            "properties": {"consulta": {"type": "string", "description": "Que buscar, en pocas palabras"}}
        }),
    }
}

/// Un trozo de la conversacion donde buscar.
#[derive(Debug, Clone, PartialEq)]
pub struct Fuente {
    /// «#12 usuario», «#13 asistente», «T3 pensado»...
    pub etiqueta: String,
    pub texto: String,
}

/// Los argumentos de una llamada a `buscar_historial`: la consulta.
pub fn consulta_de(argumentos: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(argumentos.trim()).ok()?;
    let c = v
        .get("consulta")
        .or_else(|| v.get("query"))?
        .as_str()?
        .trim();
    (!c.is_empty()).then(|| c.to_string())
}

fn palabras(s: &str) -> Vec<String> {
    sin_tildes(&s.to_lowercase())
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| {
            p.chars().count() >= 3 || p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty()
        })
        .map(str::to_string)
        .collect()
}

/// **Busca** en las fuentes: puntua por palabras de la consulta que aparecen
/// (tambien por su raiz de 5 letras: «ventas» encuentra «venta») y devuelve
/// los `max` mejores, cada uno recortado alrededor de lo encontrado.
pub fn buscar(fuentes: &[Fuente], consulta: &str, max: usize) -> String {
    let buscadas = palabras(consulta);
    if buscadas.is_empty() {
        return "ERROR: la consulta esta vacia.".into();
    }
    let raiz = |p: &str| p.chars().take(5).collect::<String>();
    let mut puntuadas: Vec<(f32, usize, &Fuente)> = fuentes
        .iter()
        .enumerate()
        .filter_map(|(i, f)| {
            let tiene = palabras(&f.texto);
            let mut puntos = 0.0;
            for b in &buscadas {
                if tiene.iter().any(|t| t == b) {
                    puntos += 1.0;
                } else if tiene.iter().any(|t| raiz(t) == raiz(b)) {
                    puntos += 0.6;
                }
            }
            (puntos > 0.0).then_some((puntos, i, f))
        })
        .collect();
    if puntuadas.is_empty() {
        return format!("No hay nada sobre «{consulta}» en la conversacion guardada.");
    }
    // Mas puntos primero; a igualdad, lo mas reciente.
    puntuadas.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut s = String::new();
    for (_, _, f) in puntuadas.into_iter().take(max.max(1)) {
        s.push_str(&format!(
            "[{}] {}\n",
            f.etiqueta,
            alrededor(&f.texto, &buscadas, 300)
        ));
    }
    s.trim_end().to_string()
}

/// `max` letras del texto alrededor de la primera palabra encontrada.
fn alrededor(texto: &str, buscadas: &[String], max: usize) -> String {
    let limpio = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    let letras: Vec<char> = limpio.chars().collect();
    if letras.len() <= max {
        return limpio;
    }
    let bajo: Vec<char> = sin_tildes(&limpio.to_lowercase()).chars().collect();
    let bajo_s: String = bajo.iter().collect();
    let pos_byte = buscadas
        .iter()
        .filter_map(|b| bajo_s.find(b.as_str()))
        .min()
        .unwrap_or(0);
    let pos = bajo_s[..pos_byte].chars().count();
    let inicio = pos.saturating_sub(max / 3);
    let fin = (inicio + max).min(letras.len());
    let mut s: String = letras[inicio..fin].iter().collect();
    if inicio > 0 {
        s.insert(0, '…');
    }
    if fin < letras.len() {
        s.push('…');
    }
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::herramientas::catalogo;
    use crate::motor::LlamadaCruda;

    #[test]
    fn la_cuenta_crece_con_el_texto_las_imagenes_y_el_catalogo() {
        let a = estimar(&[Mensaje::usuario("hola")], &[]);
        let b = estimar(&[Mensaje::usuario("hola ".repeat(400))], &[]);
        let c = estimar(
            &[Mensaje::usuario_con_imagen("hola", vec![1], "image/png")],
            &[],
        );
        let d = estimar(&[Mensaje::usuario("hola")], &catalogo());
        assert!(
            b > a + 500 && c >= a + TOKENS_IMAGEN && d > a + 300,
            "{a} {b} {c} {d}"
        );
        assert!(!pasa_del_umbral(1000, 500, 8192));
        assert!(pasa_del_umbral(5000, 900, 8192));
        assert!(
            !pasa_del_umbral(99_999, 0, 0),
            "sin ventana conocida no se reinicia"
        );
    }

    fn largo(i: usize) -> String {
        format!("mensaje {i} {}", "bla ".repeat(50))
    }

    #[test]
    fn reiniciar_deja_el_sistema_dos_de_antes_y_el_nuevo() {
        let mut h = vec![Mensaje::sistema("SIS + hoja")];
        for i in 0..10 {
            h.push(Mensaje::usuario(largo(i)));
            h.push(Mensaje::texto(Rol::Asistente, largo(i)));
        }
        h.push(Mensaje::usuario("el nuevo"));
        let r = reiniciar(&h);
        assert_eq!(r.len(), 4);
        assert_eq!(r[0].texto_entero(), "SIS + hoja");
        assert!(r[1].texto_entero().starts_with("mensaje 9"));
        assert_eq!(r[2].rol, Rol::Asistente);
        assert_eq!(r[3].texto_entero(), "el nuevo");
        assert!(estimar(&r, &[]) < estimar(&h, &[]) / 3);
    }

    #[test]
    fn reiniciar_no_deja_resultados_huerfanos() {
        let mut llamada = Mensaje::texto(Rol::Asistente, "");
        llamada.llamadas = vec![LlamadaCruda {
            id: "a".into(),
            nombre: "ocr".into(),
            argumentos: "{}".into(),
        }];
        let h = vec![
            Mensaje::sistema("S"),
            Mensaje::usuario("lee esto"),
            llamada.clone(),
            Mensaje::resultado("a", "texto del ocr"),
            Mensaje::texto(Rol::Asistente, "leido"),
            Mensaje::usuario("y ahora resumelo"),
            llamada,
            Mensaje::resultado("a", "otra vez"),
        ];
        let r = reiniciar(&h);
        // Antes del ultimo usuario: solo texto, sin la llamada ni su resultado.
        assert_eq!(r[1].texto_entero(), "lee esto");
        assert_eq!(r[2].texto_entero(), "leido");
        assert_eq!(r[3].texto_entero(), "y ahora resumelo");
        // Lo de este turno se queda entero.
        assert_eq!(r[4].llamadas.len(), 1);
        assert_eq!(r[5].rol, Rol::Herramienta);
        // Sin sistema ni usuario no se rompe.
        assert!(reiniciar(&[]).is_empty());
    }

    #[test]
    fn podar_recorta_los_resultados_viejos_y_no_el_ultimo() {
        let mut llamada = Mensaje::texto(Rol::Asistente, "");
        llamada.llamadas = vec![LlamadaCruda {
            id: "a".into(),
            nombre: "ocr".into(),
            argumentos: "{}".into(),
        }];
        let mut h = vec![
            Mensaje::usuario("lee"),
            llamada.clone(),
            Mensaje::resultado("a", "x".repeat(3000)),
            llamada,
            Mensaje::resultado("a", "y".repeat(3000)),
        ];
        assert!(podar_en_turno(&mut h));
        assert!(h[2].texto_entero().len() < 400);
        assert!(h[2].texto_entero().contains("buscar_historial"));
        assert_eq!(h[4].texto_entero().len(), 3000);
        assert!(!podar_en_turno(&mut h), "ya no hay nada que podar");
    }

    #[test]
    fn despejar_deja_los_tres_ultimos_y_dice_de_que_herramienta_era() {
        let mut h = vec![Mensaje::usuario("lee los cinco")];
        for k in 0..5 {
            let mut llamada = Mensaje::texto(Rol::Asistente, "");
            llamada.llamadas = vec![LlamadaCruda {
                id: format!("c{k}"),
                nombre: format!("ocr{k}"),
                argumentos: "{}".into(),
            }];
            h.push(llamada);
            h.push(Mensaje::resultado(
                &format!("c{k}"),
                format!("texto {k} {}", "z".repeat(2000)),
            ));
        }
        h.push(Mensaje::resultado("corto", "ok"));
        let antes = estimar(&h, &[]);
        assert!(despejar_resultados(&mut h, QUEDAN_RESULTADOS));
        assert!(estimar(&h, &[]) < antes / 2);
        // Los tres primeros, despejados con su nombre (el corto del final
        // cuenta entre los tres ultimos); los dos siguientes, enteros.
        assert!(
            h[2].texto_entero().starts_with("(ocr0: resultado quitado"),
            "{}",
            h[2].texto_entero()
        );
        assert!(h[4].texto_entero().starts_with("(ocr1:"));
        assert!(h[6].texto_entero().starts_with("(ocr2:"));
        assert!(h[8].texto_entero().len() > 2000 && h[10].texto_entero().len() > 2000);
        assert!(
            !despejar_resultados(&mut h, QUEDAN_RESULTADOS),
            "ya no hay nada que despejar"
        );
        assert!(pasa_de(5000, 0, 8192, UMBRAL_DESPEJAR) && !pasa_de(5000, 0, 8192, UMBRAL));
        assert!(!pasa_de(5000, 0, 0, UMBRAL_DESPEJAR));
    }

    #[test]
    fn buscar_encuentra_lo_mas_parecido_y_lo_mas_reciente() {
        let f = vec![
            Fuente {
                etiqueta: "#1 usuario".into(),
                texto: "Las ventas de enero fueron 120 y las de febrero 95.".into(),
            },
            Fuente {
                etiqueta: "#2 asistente".into(),
                texto: "Dibuje una casa roja.".into(),
            },
            Fuente {
                etiqueta: "#9 usuario".into(),
                texto: "La venta de marzo: 140.".into(),
            },
        ];
        let r = buscar(&f, "¿cuáles eran las ventas?", 2);
        let lineas: Vec<&str> = r.lines().collect();
        assert_eq!(lineas.len(), 2, "{r}");
        assert!(lineas[0].starts_with("[#1 usuario]"), "{r}");
        assert!(
            lineas[1].starts_with("[#9 usuario]"),
            "la raiz «venta» tambien vale: {r}"
        );
        assert!(buscar(&f, "avion", 3).starts_with("No hay nada"));
        assert!(buscar(&f, "  ", 3).starts_with("ERROR"));
        assert_eq!(
            consulta_de(r#"{"consulta":" ventas "}"#).as_deref(),
            Some("ventas")
        );
        assert_eq!(consulta_de("{roto"), None);
        assert_eq!(consulta_de(r#"{"consulta":""}"#), None);
    }

    #[test]
    fn buscar_recorta_alrededor_de_lo_encontrado() {
        let texto = format!(
            "{} el codigo secreto es 4477 {}",
            "relleno ".repeat(100),
            "cola ".repeat(100)
        );
        let f = vec![Fuente {
            etiqueta: "#3".into(),
            texto,
        }];
        let r = buscar(&f, "codigo secreto", 1);
        assert!(r.contains("4477"), "{r}");
        assert!(r.chars().count() < 330);
    }

    #[test]
    fn preparar_reinicia_solo_al_pasar_del_70() {
        let anteriores: Vec<(bool, String)> = (0..12).map(|i| (i % 2 == 0, largo(i))).collect();
        let p = preparar(
            "S".into(),
            &anteriores,
            Mensaje::usuario("nuevo"),
            &[],
            8192,
            2048,
        );
        assert!(!p.reiniciado);
        assert_eq!(p.historial.len(), 14);
        // Con una ventana pequena, se queda con los dos ultimos.
        let p = preparar(
            "S".into(),
            &anteriores,
            Mensaje::usuario("nuevo"),
            &[],
            1500,
            300,
        );
        assert!(p.reiniciado);
        assert_eq!(p.primero, 10);
        assert_eq!(p.historial.len(), 4);
        assert!(p.historial[1].texto_entero().starts_with("mensaje 10"));
        assert!(
            !pasa_del_umbral(p.tokens, 300, 1500),
            "cabe tras reiniciar: {}",
            p.tokens
        );
        // Sin nada antes no hay que reiniciar nada.
        let p = preparar("S".into(), &[], Mensaje::usuario("x"), &[], 10, 0);
        assert_eq!(p.historial.len(), 2);
    }

    #[test]
    fn el_sistema_lleva_la_hoja_y_el_plan() {
        let mut hoja = HojaDeTrabajo {
            objetivo: "una casa".into(),
            ..Default::default()
        };
        let plan = Plan {
            entiendo: "casa roja".into(),
            ..Default::default()
        };
        let s = sistema_con_hoja("Eres X.", &hoja, Some(&plan));
        assert!(s.contains("Objetivo: una casa") && s.contains("Entiendo: casa roja"));
        assert!(!s.contains("buscar_historial"));
        hoja.reinicios = 1;
        assert!(sistema_con_hoja("Eres X.", &hoja, None).contains("buscar_historial"));
        let mut h = vec![Mensaje::usuario("hola")];
        poner_sistema(&mut h, "S1".into());
        poner_sistema(&mut h, "S2".into());
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].texto_entero(), "S2");
    }
}
