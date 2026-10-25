//! **La rubrica de la revision** (22-oct-2026).
//!
//! Pedido del usuario: que el asistente saque una captura de su trabajo y se
//! ponga nota, para que siempre quede bonito, ordenado, logico y con las
//! tildes bien, y que se corrija en bucle.
//!
//! Por que una rubrica y no «¿esta bien?»: en CITL (arXiv 2604.05839) una
//! rubrica de varios ejes con bandas definidas coincidio con las personas el
//! 69,5 % de las veces, frente al 48,5 % de una nota unica; y el feedback
//! concreto rinde mucho mas que el generico (Self-Refine, 43,2 frente a
//! 31,2). Por eso el critico:
//!
//! - va en una **peticion aparte**, sin el razonamiento del que dibujo (los
//!   modelos puntuan mas alto lo suyo, arXiv 2404.13076);
//! - puntua **5 ejes de 1 a 10** ([`EJES`]) con las bandas de CITL;
//! - nombra **como mucho [`MAX_PROBLEMAS`] problemas**, cada uno con los
//!   `#id` de las etiquetas de la captura (Set-of-Mark): el que corrige toca
//!   solo esos elementos en vez de «senalar».
//!
//! Lo de aqui es texto y numeros (el prompt, leer la respuesta, la nota); el
//! bucle que la usa es `agente::revisar`.

use serde_json::Value;

/// Los ejes, en el orden de [`Nota::ejes`].
pub const EJES: [&str; 5] = ["completo", "texto", "orden", "coherencia", "legibilidad"];

/// La nota de cada eje que ya no merece otra ronda.
pub const NOTA_BUENA: f32 = 8.0;

/// Lo que tiene que subir la media de una ronda a otra para seguir: menos,
/// y otra ronda cuesta mas de lo que da (CITL: del ciclo 2 al 3, +0,5 %).
pub const MEJORA_MINIMA: f32 = 0.3;

/// Cuantos problemas como mucho: los mas importantes, no una lista larga
/// que el modelo arregle a medias.
pub const MAX_PROBLEMAS: usize = 5;

/// Con que empieza el mensaje que le pide al modelo corregir lo que vio el
/// critico: el agente lo reconoce y no lo toma por el pedido del usuario.
pub const MARCA_CORRECCION: &str = "[Revision del lienzo]";

/// Un problema concreto que vio el critico.
#[derive(Debug, Clone, PartialEq)]
pub struct Problema {
    /// Los `#id` que toca (vacio: algo pedido que falta del todo).
    pub ids: Vec<u64>,
    /// De que eje es (uno de [`EJES`]; vacio si no lo dijo).
    pub eje: String,
    /// Que esta mal.
    pub que: String,
    /// Como arreglarlo.
    pub arreglo: String,
}

/// **La nota de una captura**: un numero de 1 a 10 por eje y lo que hay que
/// arreglar.
#[derive(Debug, Clone, PartialEq)]
pub struct Nota {
    pub ejes: [f32; 5],
    pub problemas: Vec<Problema>,
}

impl Nota {
    /// La de «no hay nada que corregir» (un critico que contesto solo
    /// `TODO_BIEN`).
    pub fn perfecta() -> Nota {
        Nota {
            ejes: [10.0; 5],
            problemas: Vec::new(),
        }
    }

    pub fn media(&self) -> f32 {
        self.ejes.iter().sum::<f32>() / self.ejes.len() as f32
    }

    pub fn minima(&self) -> f32 {
        self.ejes.iter().copied().fold(f32::INFINITY, f32::min)
    }

    /// Si todos los ejes llegan a [`NOTA_BUENA`].
    pub fn aprobada(&self) -> bool {
        self.minima() >= NOTA_BUENA
    }

    /// `completo 9, texto 6, orden 7, coherencia 9, legibilidad 8 (media 7.8)`.
    pub fn resumen(&self) -> String {
        let ejes: Vec<String> = EJES
            .iter()
            .zip(self.ejes)
            .map(|(n, v)| format!("{n} {}", redondo(v)))
            .collect();
        format!("{} (media {:.1})", ejes.join(", "), self.media())
    }
}

fn redondo(v: f32) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

/// **Las instrucciones del critico** (su mensaje de sistema).
pub fn instrucciones() -> String {
    format!(
        "Eres el revisor de un lienzo de PixPin. Tú no hiciste este trabajo: júzgalo con ojos frescos y sin indulgencia.\n\
         Recibes lo que pidió el usuario, una imagen del lienzo (cada elemento lleva su #id en una etiqueta amarilla; la rejilla \
         gris y las etiquetas no son del dibujo) y la lista de elementos con su #id, su caja [x, y, ancho, alto] y sus textos.\n\
         Puntúa estos 5 ejes de 1 a 10:\n\
         - completo: está todo lo que pidió el usuario (repasa cada requisito: sí o no).\n\
         - texto: el idioma del pedido, la ortografía y las tildes (en español: «está», «función», «¿…?»), nada de texto cortado.\n\
         - orden: alineado, sin solapes, sin textos que se salen de su figura, espaciado parejo, flechas que llegan a lo que señalan.\n\
         - coherencia: tiene lógica con el tema (el orden de los pasos, cada etiqueta en su sitio, colores con sentido).\n\
         - legibilidad: se lee de un vistazo (tamaño de letra, contraste, nada diminuto ni amontonado).\n\
         Bandas: 1-2 fallos graves; 3-4 carencias importantes; 5-6 problemas menores que se ven; 7-8 faltan detalles pequeños; \
         9-10 cumplido del todo.\n\
         Da como mucho {MAX_PROBLEMAS} problemas, los más importantes, cada uno con los #id que toca y un arreglo concreto \
         (mover, cambiar el texto, agrandar, alinear con otro #id...). Si falta algo pedido, «ids» vacío y di qué añadir. \
         Sin problemas, «problemas» vacío.\n\
         Contesta SOLO con este JSON, sin nada más:\n\
         {{\"notas\":{{\"completo\":9,\"texto\":7,\"orden\":6,\"coherencia\":9,\"legibilidad\":8}},\
         \"problemas\":[{{\"ids\":[3],\"eje\":\"texto\",\"que\":\"«Funcion» sin tilde\",\"arreglo\":\"cambiar el texto de #3 a «Función»\"}}]}}"
    )
}

/// **Lo que se le ensena al critico** junto a la imagen: el pedido, la
/// leyenda de la vista (con la lista de elementos) y lo que fallan las
/// comprobaciones automaticas, si las hay.
pub fn pedido_al_critico(pedido: &str, leyenda: &str, duros: &[String]) -> String {
    let pedido: String = pedido.chars().take(600).collect();
    let mut s = format!("Pedido del usuario: «{}»\n\n{leyenda}", pedido.trim());
    if !duros.is_empty() {
        s.push_str("\n\nComprobaciones automáticas que fallan (son seguras: cuéntalas en su eje):");
        for d in duros {
            s.push_str(&format!("\n- {d}"));
        }
    }
    s
}

/// **Lo que se le pide al modelo que dibujo** tras la nota: solo los
/// problemas, con sus `#id`, para que toque esos y no lo rehaga todo.
pub fn pedido_de_correccion(nota: &Nota, duros: &[String], ronda: usize) -> String {
    let mut s = format!(
        "{MARCA_CORRECCION} Ronda {ronda}. Un revisor miró la imagen de tu trabajo y le puso: {}. Arregla solo esto con las \
         herramientas de editar, usando los #id (mueve o cambia; no lo vuelvas a dibujar entero):",
        nota.resumen()
    );
    let mut n = 0;
    for d in duros {
        n += 1;
        s.push_str(&format!("\n{n}. (comprobación) {d}"));
    }
    for p in &nota.problemas {
        n += 1;
        let ids: Vec<String> = p.ids.iter().map(|i| format!("#{i}")).collect();
        let quien = if ids.is_empty() {
            "falta".to_string()
        } else {
            ids.join(", ")
        };
        let eje = if p.eje.is_empty() {
            String::new()
        } else {
            format!(" ({})", p.eje)
        };
        let arreglo = if p.arreglo.is_empty() {
            String::new()
        } else {
            format!(" → {}", p.arreglo)
        };
        s.push_str(&format!("\n{n}. {quien}{eje}: {}{arreglo}", p.que));
    }
    s.push_str("\nLuego di en una frase corta qué corregiste.");
    s
}

/// El eje que quiso decir el critico (acepta sinonimos y tildes).
fn eje_de(nombre: &str) -> Option<usize> {
    let n = crate::acciones::normalizar(nombre.trim());
    let i = match n.as_str() {
        "completo" | "completitud" | "completa" | "requisitos" => 0,
        "texto" | "textos" | "ortografia" | "idioma" | "tildes" | "acentos" => 1,
        "orden" | "ordenado" | "alineacion" | "composicion" | "disposicion" => 2,
        "coherencia" | "logica" | "coherente" | "tema" => 3,
        "legibilidad" | "legible" | "lectura" => 4,
        _ => return None,
    };
    Some(i)
}

/// Un numero de una nota: `8`, `"8"`, `"8/10"` o `{"nota": 8}`.
fn numero(v: &Value) -> Option<f32> {
    let n = match v {
        Value::Number(n) => n.as_f64()? as f32,
        Value::String(s) => s.split('/').next()?.trim().replace(',', ".").parse().ok()?,
        Value::Object(o) => {
            return ["nota", "puntuacion", "valor", "score"]
                .iter()
                .find_map(|k| o.get(*k).and_then(numero));
        }
        _ => return None,
    };
    n.is_finite().then(|| n.clamp(1.0, 10.0))
}

/// Los `#id` de un problema: de «ids» (numeros o «#3»), de «id», o los que
/// salgan escritos en su texto.
fn ids_de(p: &serde_json::Map<String, Value>, texto: &str) -> Vec<u64> {
    let de = |v: &Value| -> Option<u64> {
        match v {
            Value::Number(n) => n.as_u64(),
            Value::String(s) => s.trim().trim_start_matches('#').parse().ok(),
            _ => None,
        }
    };
    let mut ids: Vec<u64> = match (p.get("ids"), p.get("id")) {
        (Some(Value::Array(a)), _) => a.iter().filter_map(de).collect(),
        (Some(v), _) | (None, Some(v)) => de(v).into_iter().collect(),
        _ => Vec::new(),
    };
    if ids.is_empty() {
        ids = ids_en(texto);
    }
    ids.dedup();
    ids
}

/// Los `#12` escritos en un texto.
pub fn ids_en(texto: &str) -> Vec<u64> {
    let mut ids = Vec::new();
    for trozo in texto.split('#').skip(1) {
        let cifras: String = trozo.chars().take_while(char::is_ascii_digit).collect();
        if let Ok(id) = cifras.parse::<u64>()
            && !ids.contains(&id)
        {
            ids.push(id);
        }
    }
    ids
}

/// **Lee la respuesta del critico**. Aguanta JSON con texto alrededor o
/// mal cerrado (`reparar_json`), las notas sueltas o dentro de «notas», y
/// sinonimos de los ejes. Si faltan ejes (como mucho dos), toman la media de
/// los que dijo; con menos de tres, `None`: no es una nota. Un `TODO_BIEN`
/// sin JSON es la nota perfecta.
pub fn leer(texto: &str) -> Option<Nota> {
    let t = texto.trim();
    let Some(v) = crate::herramientas::reparar_json(t).filter(Value::is_object) else {
        return (!t.is_empty() && super::es_todo_bien(t)).then(Nota::perfecta);
    };
    let notas = match v
        .get("notas")
        .or_else(|| v.get("ejes"))
        .or_else(|| v.get("puntuaciones"))
    {
        Some(Value::Object(o)) => o.clone(),
        _ => v.as_object().cloned().unwrap_or_default(),
    };
    let mut ejes: [Option<f32>; 5] = [None; 5];
    for (k, val) in &notas {
        if let (Some(i), Some(n)) = (eje_de(k), numero(val)) {
            ejes[i] = Some(n);
        }
    }
    let dichos: Vec<f32> = ejes.iter().flatten().copied().collect();
    if dichos.len() < 3 {
        return None;
    }
    let media = dichos.iter().sum::<f32>() / dichos.len() as f32;
    let ejes = ejes.map(|e| e.unwrap_or(media));
    let problemas = match v.get("problemas") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|p| match p {
                Value::Object(o) => {
                    let campo = |k: &str| {
                        o.get(k)
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .trim()
                            .to_string()
                    };
                    let que = ["que", "problema", "descripcion"]
                        .iter()
                        .map(|k| campo(k))
                        .find(|s| !s.is_empty())
                        .unwrap_or_default();
                    let arreglo = ["arreglo", "accion", "solucion"]
                        .iter()
                        .map(|k| campo(k))
                        .find(|s| !s.is_empty())
                        .unwrap_or_default();
                    if que.is_empty() && arreglo.is_empty() {
                        return None;
                    }
                    let eje = eje_de(&campo("eje"))
                        .map(|i| EJES[i].to_string())
                        .unwrap_or_default();
                    Some(Problema {
                        ids: ids_de(o, &format!("{que} {arreglo}")),
                        eje,
                        que,
                        arreglo,
                    })
                }
                Value::String(s) if !s.trim().is_empty() => Some(Problema {
                    ids: ids_en(s),
                    eje: String::new(),
                    que: s.trim().to_string(),
                    arreglo: String::new(),
                }),
                _ => None,
            })
            .take(MAX_PROBLEMAS)
            .collect(),
        _ => Vec::new(),
    };
    Some(Nota { ejes, problemas })
}

/// **Si unas comprobaciones duras traen un fallo nuevo** respecto a las de
/// la mejor version: una ronda asi no se acepta aunque el critico la puntue
/// mejor (la medida exacta manda sobre la vista).
pub fn rompe_algo(antes: &[String], ahora: &[String]) -> bool {
    ahora.iter().any(|d| !antes.contains(d))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lee_la_rubrica_entera() {
        let n = leer(
            r##"{"notas":{"completo":9,"texto":6,"orden":7,"coherencia":9,"legibilidad":8},
                "problemas":[{"ids":[3],"eje":"texto","que":"«Funcion» sin tilde","arreglo":"cambiar #3 a «Función»"},
                             {"ids":["#4","#5"],"eje":"orden","que":"#4 tapa a #5","arreglo":"mover #5 40 a la derecha"}]}"##,
        )
        .unwrap();
        assert_eq!(n.ejes, [9.0, 6.0, 7.0, 9.0, 8.0]);
        assert!((n.media() - 7.8).abs() < 1e-4);
        assert_eq!(n.minima(), 6.0);
        assert!(!n.aprobada());
        assert_eq!(n.problemas.len(), 2);
        assert_eq!(n.problemas[0].ids, vec![3]);
        assert_eq!(n.problemas[1].ids, vec![4, 5]);
        assert_eq!(n.problemas[1].eje, "orden");
        assert!(n.resumen().contains("texto 6") && n.resumen().contains("media 7.8"));
    }

    #[test]
    fn aguanta_texto_alrededor_sinonimos_y_numeros_raros() {
        let n = leer(
            "Aquí va mi nota:\n```json\n{\"completo\":\"9/10\",\"ortografía\":7,\"alineación\":{\"nota\":8},\"lógica\":10,\"legible\":12,\
             \"problemas\":[\"#2 se sale de su caja\"]}\n```",
        )
        .unwrap();
        assert_eq!(n.ejes, [9.0, 7.0, 8.0, 10.0, 10.0], "12 se queda en 10");
        assert_eq!(n.problemas[0].ids, vec![2]);
        // Los ids escritos en el texto si no vienen en «ids».
        let n = leer(r##"{"notas":{"completo":5,"texto":5,"orden":5},"problemas":[{"que":"#7 y #9 se tocan","arreglo":"separa"}]}"##).unwrap();
        assert_eq!(n.problemas[0].ids, vec![7, 9]);
        assert_eq!(
            n.ejes[3], 5.0,
            "un eje que falta toma la media de los dichos"
        );
    }

    #[test]
    fn mas_de_cinco_problemas_se_recortan() {
        let muchos: Vec<String> = (1..=9)
            .map(|i| format!(r##"{{"ids":[{i}],"que":"mal #{i}"}}"##))
            .collect();
        let t = format!(
            r#"{{"notas":{{"completo":5,"texto":5,"orden":5,"coherencia":5,"legibilidad":5}},"problemas":[{}]}}"#,
            muchos.join(",")
        );
        assert_eq!(leer(&t).unwrap().problemas.len(), MAX_PROBLEMAS);
    }

    #[test]
    fn lo_que_no_es_una_nota() {
        assert_eq!(leer("TODO_BIEN"), Some(Nota::perfecta()));
        assert!(Nota::perfecta().aprobada());
        // Casos negativos: texto libre, vacio, JSON sin ejes o con dos solos.
        assert_eq!(
            leer("Se ve bastante bien, aunque mejoraria el titulo."),
            None
        );
        assert_eq!(leer(""), None);
        assert_eq!(leer(r#"{"hola":1}"#), None);
        assert_eq!(leer(r#"{"notas":{"completo":9,"texto":9}}"#), None);
    }

    #[test]
    fn la_correccion_nombra_los_ids_y_las_comprobaciones() {
        let n = leer(r##"{"notas":{"completo":4,"texto":9,"orden":9,"coherencia":9,"legibilidad":9},"problemas":[{"ids":[],"eje":"completo","que":"falta el titulo","arreglo":"anade un titulo arriba"},{"ids":[2],"que":"#2 muy pequeno","arreglo":"agrandalo"}]}"##).unwrap();
        let s = pedido_de_correccion(&n, &["#5 se sale del lienzo".to_string()], 1);
        assert!(s.starts_with(MARCA_CORRECCION));
        assert!(s.contains("1. (comprobación) #5 se sale"));
        assert!(s.contains("2. falta (completo): falta el titulo → anade un titulo arriba"));
        assert!(s.contains("3. #2: #2 muy pequeno → agrandalo"));
        let c = pedido_al_critico(
            "dibuja un ciclo",
            "[Vista del lienzo] ...",
            &["#5 se sale".into()],
        );
        assert!(c.contains("«dibuja un ciclo»") && c.contains("- #5 se sale"));
        assert!(instrucciones().contains("\"legibilidad\":8"));
    }

    #[test]
    fn un_fallo_duro_nuevo_rompe_la_ronda() {
        let a = vec!["#1 tapa a #2".to_string()];
        assert!(!rompe_algo(&a, &[]));
        assert!(!rompe_algo(&a, &a));
        assert!(rompe_algo(&a, &["#3 se sale".to_string()]));
        assert!(rompe_algo(&[], &["#3 se sale".to_string()]));
    }
}
