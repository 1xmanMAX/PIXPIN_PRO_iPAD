//! **Lo que piensa el modelo, a la vista, y la peticion mejorada.**
//!
//! Pedido del usuario (13-oct-2026): «que pueda ver lo que esta pensando
//! como en Claude; que guarde sus pensamientos y su proceso, resuma y mejore
//! las peticiones». Aqui va la parte que no sabe de ventanas:
//!
//! - [`Separador`]: el razonamiento llega por dos caminos. `llama-server`
//!   (b11580, `--reasoning-format auto`) lo manda aparte en
//!   `reasoning_content`, que [`crate::sse`] ya convierte en
//!   [`Trozo::Pensando`]; Claude (thinking resumido) y DeepSeek
//!   (`reasoning_content`) llegan igual por su motor. Pero un servidor con
//!   `--reasoning-format none`, o una plantilla rara, lo deja dentro del texto
//!   entre `<think>` y `</think>`: el separador lo saca de ahi, aunque la
//!   etiqueta venga partida entre dos trozos. (Un `</think>` suelto, sin
//!   `<think>`, solo se reconoce al vuelo si nada de texto salio antes: lo
//!   ya ensenado no se puede retirar, y la etiqueta se deja en el texto para
//!   que [`separar`] lo arregle al acabar.)
//! - [`Cronometro`]: cuanto penso («Penso 12 s»).
//! - [`resumir`]: el pensamiento en una o dos frases, para la hoja de trabajo
//!   (sin otra llamada al modelo: se toma su primera y su ultima frase, que
//!   es donde un modelo pequeno dice que entendio y que decidio).
//! - [`Plan`]: la **peticion mejorada**. Antes de actuar, el modelo escribe
//!   en pocas lineas que entendio, que hara y con que herramientas. Con eso
//!   se rellena el objetivo de la hoja de trabajo y es lo que sobrevive
//!   cuando se reinicia el contexto.

use std::time::Instant;

use crate::herramientas::Herramienta;
use crate::motor::Trozo;

const ABRE: &str = "<think>";
const CIERRA: &str = "</think>";

/// **Saca el pensamiento escrito entre etiquetas** de los trozos de texto.
#[derive(Debug, Default)]
pub struct Separador {
    dentro: bool,
    /// Lo que podria ser el principio de una etiqueta partida.
    cola: String,
    /// Si ya salio texto de verdad (un `</think>` sin abrir despues de eso
    /// ya no puede convertir en pensamiento lo enviado).
    hubo_texto: bool,
}

impl Separador {
    pub fn nuevo() -> Separador {
        Separador::default()
    }

    /// Un trozo del motor, ya repartido entre pensamiento y texto.
    pub fn meter(&mut self, t: &Trozo) -> Vec<Trozo> {
        let Trozo::Texto(s) = t else {
            return vec![t.clone()];
        };
        let mut buf = std::mem::take(&mut self.cola);
        buf.push_str(s);
        let mut salida = Vec::new();
        loop {
            if self.dentro {
                if let Some(i) = buf.find(CIERRA) {
                    empujar(&mut salida, Trozo::Pensando(buf[..i].to_string()));
                    buf = buf[i + CIERRA.len()..].trim_start().to_string();
                    self.dentro = false;
                    continue;
                }
                let guardar = prefijo_al_final(&buf, CIERRA);
                let corte = buf.len() - guardar;
                empujar(&mut salida, Trozo::Pensando(buf[..corte].to_string()));
                self.cola = buf[corte..].to_string();
                break;
            }
            let abre = buf.find(ABRE);
            let cierra = buf.find(CIERRA);
            // Un `</think>` sin abrir: la plantilla de Qwen ya escribio el
            // `<think>` por su cuenta y solo llega el cierre.
            if let Some(c) = cierra.filter(|c| abre.is_none_or(|a| *c < a)) {
                if self.hubo_texto {
                    // Lo de antes ya salio como texto: la etiqueta se deja
                    // en el texto para que [`separar`] lo arregle al final.
                    empujar(
                        &mut salida,
                        Trozo::Texto(buf[..c + CIERRA.len()].to_string()),
                    );
                    buf = buf[c + CIERRA.len()..].to_string();
                    continue;
                }
                empujar(&mut salida, Trozo::Pensando(buf[..c].to_string()));
                buf = buf[c + CIERRA.len()..].trim_start().to_string();
                continue;
            }
            if let Some(a) = abre {
                self.texto(&mut salida, &buf[..a]);
                buf = buf[a + ABRE.len()..].to_string();
                self.dentro = true;
                continue;
            }
            let guardar = prefijo_al_final(&buf, ABRE).max(prefijo_al_final(&buf, CIERRA));
            let corte = buf.len() - guardar;
            self.texto(&mut salida, &buf[..corte]);
            self.cola = buf[corte..].to_string();
            break;
        }
        salida
    }

    /// Lo que quedo guardado por si era una etiqueta, al acabar.
    pub fn acabar(&mut self) -> Vec<Trozo> {
        let cola = std::mem::take(&mut self.cola);
        if cola.is_empty() {
            return Vec::new();
        }
        if self.dentro {
            vec![Trozo::Pensando(cola)]
        } else {
            vec![Trozo::Texto(cola)]
        }
    }

    fn texto(&mut self, salida: &mut Vec<Trozo>, s: &str) {
        if !s.is_empty() {
            self.hubo_texto |= !s.trim().is_empty();
            empujar(salida, Trozo::Texto(s.to_string()));
        }
    }
}

fn empujar(salida: &mut Vec<Trozo>, t: Trozo) {
    match &t {
        Trozo::Texto(s) | Trozo::Pensando(s) if s.is_empty() => {}
        _ => salida.push(t),
    }
}

/// Cuantos bytes del final de `s` son el principio de `etiqueta`.
fn prefijo_al_final(s: &str, etiqueta: &str) -> usize {
    (1..etiqueta.len().min(s.len() + 1))
        .rev()
        .find(|&n| s.is_char_boundary(s.len() - n) && s.ends_with(&etiqueta[..n]))
        .unwrap_or(0)
}

/// Quita el pensamiento de un texto ya entero: `(pensamiento, texto)`.
pub fn separar(texto: &str) -> (String, String) {
    let mut s = Separador::nuevo();
    let mut trozos = s.meter(&Trozo::Texto(texto.to_string()));
    trozos.extend(s.acabar());
    let (mut p, mut t) = (String::new(), String::new());
    for x in trozos {
        match x {
            Trozo::Pensando(x) => p.push_str(&x),
            Trozo::Texto(x) => t.push_str(&x),
            Trozo::Llamando { .. } => {}
        }
    }
    (p, t)
}

/// **Cuanto penso.** Empieza con el primer trozo de pensamiento y acaba con
/// el primer trozo de otra cosa (o al final de la respuesta).
#[derive(Debug, Default)]
pub struct Cronometro {
    empezo: Option<Instant>,
    acabado: bool,
    pub texto: String,
}

impl Cronometro {
    /// Un trozo de pensamiento. Uno vacio no cuenta: un modelo que «piensa»
    /// en blanco no ha pensado.
    pub fn pensando(&mut self, s: &str) {
        if s.is_empty() || (self.texto.is_empty() && s.trim().is_empty()) {
            return;
        }
        if self.acabado {
            // Volvio a pensar (otra vuelta del agente): sigue sumando.
            self.acabado = false;
            self.texto.push_str("\n\n");
        }
        self.empezo.get_or_insert_with(Instant::now);
        self.texto.push_str(s);
    }

    /// Llego otra cosa. Devuelve los segundos si con esto acaba de pensar.
    pub fn otra_cosa(&mut self) -> Option<f32> {
        if self.acabado || self.texto.trim().is_empty() {
            return None;
        }
        self.acabado = true;
        self.empezo.map(|e| e.elapsed().as_secs_f32())
    }

    pub fn penso(&self) -> bool {
        !self.texto.trim().is_empty()
    }

    pub fn segundos(&self) -> f32 {
        self.empezo.map_or(0.0, |e| e.elapsed().as_secs_f32())
    }
}

/// **El pensamiento en pocas palabras**, para la hoja: su primera frase y la
/// ultima, como mucho `max` letras.
pub fn resumir(pensamiento: &str, max: usize) -> String {
    let limpio = pensamiento.split_whitespace().collect::<Vec<_>>().join(" ");
    if limpio.chars().count() <= max {
        return limpio;
    }
    let frases: Vec<&str> = limpio
        .split_inclusive(['.', '?', '!'])
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .collect();
    let primera = frases.first().copied().unwrap_or("");
    let ultima = frases.last().copied().unwrap_or("");
    let junto = if frases.len() > 1 {
        format!("{primera} … {ultima}")
    } else {
        primera.to_string()
    };
    cortar(&junto, max)
}

/// Corta a `max` letras con puntos suspensivos.
pub fn cortar(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
    t = t.trim_end().to_string();
    t.push('…');
    t
}

// ---------------------------------------------------------------------------
// La peticion mejorada

/// **Lo que entendio el modelo antes de actuar.**
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    /// Que quiere el usuario, con las referencias («eso», «otra vez»)
    /// resueltas con la hoja de trabajo. Es el objetivo de la hoja.
    pub entiendo: String,
    /// Que hara, en una frase.
    pub hare: String,
    /// Con que herramientas (solo nombres que existen).
    pub herramientas: Vec<String>,
    /// Datos concretos que hay que recordar (cifras, nombres, colores).
    pub datos: Vec<String>,
    /// Lo que el usuario decidio o prefiere.
    pub decidido: Vec<String>,
    /// Si salio del modelo o se puso a falta de uno (el modelo no lo
    /// escribio bien).
    pub del_modelo: bool,
}

impl Plan {
    /// A falta de algo mejor: el propio pedido.
    pub fn por_omision(pedido: &str) -> Plan {
        Plan {
            entiendo: cortar(
                pedido
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .as_str(),
                240,
            ),
            ..Plan::default()
        }
    }

    /// **Lee lo que escribio el modelo.** Sin exigir: mayusculas, tildes,
    /// guiones o asteriscos de Markdown dan igual. `None` si no hay ni una
    /// linea que se entienda (el modelo contesto otra cosa).
    pub fn leer(texto: &str, conocidas: &[Herramienta]) -> Option<Plan> {
        let mut p = Plan {
            del_modelo: true,
            ..Plan::default()
        };
        let mut alguna = false;
        for linea in texto.lines() {
            let linea = linea
                .trim()
                .trim_start_matches(['-', '*', '#', ' ', '•'])
                .trim();
            let Some((clave, valor)) = linea.split_once(':') else {
                continue;
            };
            let clave = sin_tildes(&clave.replace('*', "").trim().to_lowercase());
            let valor = valor.replace("**", "").trim().to_string();
            let vacio = es_nada(&valor);
            match clave.as_str() {
                "entiendo" | "entendi" | "objetivo" | "peticion" => {
                    alguna = true;
                    if !vacio {
                        p.entiendo = cortar(&valor, 300);
                    }
                }
                "hare" | "pasos" | "plan" | "hago" => {
                    alguna = true;
                    if !vacio {
                        p.hare = cortar(&valor, 300);
                    }
                }
                "herramientas" | "herramienta" | "con" => {
                    alguna = true;
                    if !vacio {
                        let bajo = valor.to_lowercase();
                        p.herramientas = conocidas
                            .iter()
                            .filter(|h| bajo.contains(&h.nombre.to_lowercase()))
                            .map(|h| h.nombre.clone())
                            .collect();
                    }
                }
                "datos" | "hechos" => {
                    alguna = true;
                    if !vacio {
                        p.datos = lista(&valor);
                    }
                }
                "decidido" | "decisiones" | "preferencias" => {
                    alguna = true;
                    if !vacio {
                        p.decidido = lista(&valor);
                    }
                }
                _ => {}
            }
        }
        (alguna && !p.entiendo.is_empty()).then_some(p)
    }

    /// Lo que se ensena encima de la respuesta: de dos a cuatro lineas.
    pub fn lineas(&self) -> String {
        let mut v = vec![format!("Entiendo: {}", self.entiendo)];
        if !self.hare.is_empty() {
            v.push(format!("Haré: {}", self.hare));
        }
        if !self.herramientas.is_empty() {
            v.push(format!("Con: {}", self.herramientas.join(", ")));
        }
        if !self.datos.is_empty() {
            v.push(format!("Datos: {}", self.datos.join("; ")));
        }
        v.join("\n")
    }

    pub fn a_json(&self) -> serde_json::Value {
        serde_json::json!({
            "entiendo": self.entiendo,
            "hare": self.hare,
            "herramientas": self.herramientas,
            "datos": self.datos,
            "decidido": self.decidido,
            "del_modelo": self.del_modelo,
        })
    }

    pub fn de_json(v: &serde_json::Value) -> Option<Plan> {
        let o = v.as_object()?;
        let cadena = |k: &str| {
            o.get(k)
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .to_string()
        };
        let p = Plan {
            entiendo: cadena("entiendo"),
            hare: cadena("hare"),
            herramientas: cadenas(o.get("herramientas")),
            datos: cadenas(o.get("datos")),
            decidido: cadenas(o.get("decidido")),
            del_modelo: o
                .get("del_modelo")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
        };
        (!p.entiendo.is_empty()).then_some(p)
    }
}

pub(crate) fn cadenas(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(|x| x.as_array())
        .map(|l| {
            l.iter()
                .filter_map(|x| x.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn lista(valor: &str) -> Vec<String> {
    valor
        .split([';', '|'])
        .map(|x| x.trim().trim_end_matches('.').trim())
        .filter(|x| !x.is_empty() && !es_nada(x))
        .map(|x| cortar(x, 120))
        .take(6)
        .collect()
}

fn es_nada(v: &str) -> bool {
    let v = sin_tildes(&v.trim().trim_end_matches('.').to_lowercase());
    v.is_empty()
        || matches!(v.as_str(), "nada" | "ninguna" | "ninguno" | "-" | "no" | "none" | "n/a")
        // «No se ha decidido todavia…», «aun nada»: el 2B lo escribe asi
        // en vez de «nada» (medido, 13-oct).
        || ["no se ha", "aun no", "todavia no", "nada ", "ninguna ", "ninguno ", "sin decidir"]
            .iter()
            .any(|p| v.starts_with(p))
}

/// Para comparar sin que importen las tildes.
pub fn sin_tildes(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'Á' | 'À' | 'Ä' | 'Â' => 'A',
            'É' | 'È' | 'Ë' | 'Ê' => 'E',
            'Í' | 'Ì' | 'Ï' | 'Î' => 'I',
            'Ó' | 'Ò' | 'Ö' | 'Ô' => 'O',
            'Ú' | 'Ù' | 'Ü' | 'Û' => 'U',
            otra => otra,
        })
        .collect()
}

/// Las instrucciones de la vuelta de planear. Cortas y con el formato
/// exacto: un modelo de 2B sigue mejor una plantilla que una explicacion.
pub fn instrucciones_del_plan(herramientas: &[Herramienta]) -> String {
    let nombres: Vec<&str> = herramientas.iter().map(|h| h.nombre.as_str()).collect();
    format!(
        "Eres el planificador del asistente de PixPin. NO contestes al usuario ni hagas la tarea. \
         Lee la hoja de trabajo y el mensaje nuevo, y escribe SOLO estas cinco lineas:\n\
         ENTIENDO: que quiere el usuario, en una frase completa; si dice «eso», «otra vez» o «ahora», \
         di a que se refiere usando la hoja de trabajo.\n\
         HARE: los pasos, en una frase. Si es cambiar algo que ya existe (agrandar, reorganizar), se edita eso, no se hace otro.\n\
         HERRAMIENTAS: las que usaras de esta lista, separadas por comas, o ninguna: {}.\n\
         DATOS: cifras, nombres o colores concretos que DIJO el usuario (no los inventes: nada de actividades, \
         temas ni ejemplos que no dio), separados por punto y coma, o nada.\n\
         DECIDIDO: lo que el usuario eligio o prefiere, o nada.",
        if nombres.is_empty() {
            "ninguna".to_string()
        } else {
            nombres.join(", ")
        }
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::herramientas::catalogo;

    fn pasar(trozos: &[&str]) -> (String, String) {
        let mut s = Separador::nuevo();
        let mut todo = Vec::new();
        for t in trozos {
            todo.extend(s.meter(&Trozo::Texto(t.to_string())));
        }
        todo.extend(s.acabar());
        let (mut p, mut t) = (String::new(), String::new());
        for x in todo {
            match x {
                Trozo::Pensando(x) => p.push_str(&x),
                Trozo::Texto(x) => t.push_str(&x),
                _ => {}
            }
        }
        (p, t)
    }

    #[test]
    fn las_etiquetas_partidas_entre_trozos_se_separan() {
        let (p, t) = pasar(&[
            "<thi",
            "nk>Veamos: una ca",
            "sa roja.</th",
            "ink>\n\nHecho",
            ", la dibujé.",
        ]);
        assert_eq!(p, "Veamos: una casa roja.");
        assert_eq!(t, "Hecho, la dibujé.");
    }

    #[test]
    fn un_cierre_sin_apertura_es_pensamiento_si_va_al_principio() {
        // En un mismo trozo (o en un texto entero, ver `separar`): lo de
        // antes del cierre es pensamiento.
        let (p, t) = pasar(&["Pienso que es azul.</think>", "Es azul."]);
        assert_eq!(p, "Pienso que es azul.");
        assert_eq!(t, "Es azul.");
        // Ya enviado como texto no se puede retirar: la etiqueta se queda en
        // el texto y `separar` lo arregla al final.
        let (p, t) = pasar(&["Pienso.", "</think>", "Es azul."]);
        assert_eq!(p, "");
        assert_eq!(t, "Pienso.</think>Es azul.");
        assert_eq!(separar(&t), ("Pienso.".into(), "Es azul.".into()));
    }

    #[test]
    fn sin_etiquetas_todo_es_texto_y_los_menores_que_no_se_pierden() {
        let (p, t) = pasar(&["3 <", " 4 y a<b ", "<t"]);
        assert_eq!(p, "");
        assert_eq!(t, "3 < 4 y a<b <t");
    }

    #[test]
    fn los_trozos_de_pensamiento_y_llamadas_pasan_tal_cual() {
        let mut s = Separador::nuevo();
        assert_eq!(
            s.meter(&Trozo::Pensando("x".into())),
            vec![Trozo::Pensando("x".into())]
        );
        let l = Trozo::Llamando {
            nombre: "ocr".into(),
        };
        assert_eq!(s.meter(&l), vec![l]);
    }

    #[test]
    fn separar_un_texto_entero() {
        assert_eq!(separar("<think>a</think>b"), ("a".into(), "b".into()));
        assert_eq!(
            separar("<think>sin cerrar"),
            ("sin cerrar".into(), "".into())
        );
    }

    #[test]
    fn el_cronometro_ignora_el_pensamiento_vacio() {
        let mut c = Cronometro::default();
        c.pensando("");
        c.pensando("   \n");
        assert!(!c.penso());
        assert_eq!(c.otra_cosa(), None);
        c.pensando("hmm");
        assert!(c.penso());
        assert!(c.otra_cosa().is_some());
        assert_eq!(c.otra_cosa(), None, "solo acaba una vez");
    }

    #[test]
    fn resumir_toma_la_primera_y_la_ultima_frase() {
        let largo = "El usuario quiere una casa. Primero pienso en el tejado. Luego en las ventanas. \
                     Luego en la puerta. Al final usare dibujar_en_lienzo con rojo.";
        let r = resumir(largo, 120);
        assert!(r.starts_with("El usuario quiere una casa."), "{r}");
        assert!(r.ends_with("rojo."), "{r}");
        assert!(r.chars().count() <= 120);
        assert_eq!(resumir("  corto \n ya ", 50), "corto ya");
    }

    #[test]
    fn el_plan_se_lee_aunque_venga_con_markdown_y_tildes() {
        let t = "**ENTIENDO:** Dibujar la casa de antes en rojo.\n- HARÉ: llamar a dibujar con rojo.\n\
                 HERRAMIENTAS: dibujar_en_lienzo, volar\nDATOS: color rojo; 3 ventanas\nDECIDIDO: No se ha decidido todavía.";
        let p = Plan::leer(t, &catalogo()).unwrap();
        assert_eq!(p.entiendo, "Dibujar la casa de antes en rojo.");
        assert_eq!(p.hare, "llamar a dibujar con rojo.");
        assert_eq!(
            p.herramientas,
            vec!["dibujar_en_lienzo"],
            "las inventadas no"
        );
        assert_eq!(p.datos, vec!["color rojo", "3 ventanas"]);
        assert!(p.decidido.is_empty());
        assert!(p.lineas().lines().count() >= 2 && p.lineas().lines().count() <= 4);
        assert_eq!(Plan::de_json(&p.a_json()), Some(p));
    }

    #[test]
    fn un_plan_que_no_es_plan_no_se_lee() {
        assert_eq!(Plan::leer("Hola, ¿en qué te ayudo?", &catalogo()), None);
        assert_eq!(Plan::leer("ENTIENDO: nada\nHARE: x", &catalogo()), None);
        assert_eq!(Plan::de_json(&serde_json::json!([1, 2])), None);
        let p = Plan::por_omision("  dibuja\n una casa  ");
        assert_eq!(p.entiendo, "dibuja una casa");
        assert!(!p.del_modelo);
    }
}
