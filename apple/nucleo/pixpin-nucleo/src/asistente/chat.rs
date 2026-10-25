//! **El asistente dentro de una conversacion** (E1, el `asistente/chat.rs`
//! de Windows tal cual): lo que no depende de la ventana (que se le manda al
//! modelo, que foto y que adjuntos ve, y como se escribe la burbuja en
//! vivo). El turno en el chat lo engancha [`super::en_la_conversacion`].
//!
//! - En el chat fijo «Asistente» todo lo que se escribe es para el.
//! - En cualquier otro chat, `@asistente` al principio o en medio le
//!   pregunta sobre lo que hay ahi (ve los ultimos mensajes del chat).
//! - Su respuesta es una nota normal del cuaderno, con `aparato = "IA"`
//!   (la chapa dice `#48·IA`): se guarda, se busca y viaja como cualquier
//!   otra, sin un formato nuevo que el movil no sepa leer.

use std::path::PathBuf;

use pixpin_ia::agente::{Evento, Final};
use pixpin_ia::herramientas::{AYUDA, CALCAR, DIBUJAR, GRAFICA, OCR};
use pixpin_proyecto::cuaderno::{Clase, Mensaje as Nota};

use super::turno::AdjuntoIa;

/// El «aparato» de las notas del asistente.
pub const APARATO: &str = "IA";

/// La mencion que lo llama desde otro chat.
pub const MENCION: &str = "@asistente";

pub fn es_del_asistente(m: &Nota) -> bool {
    m.aparato.as_deref() == Some(APARATO)
}

fn es_nota(m: &Nota) -> bool {
    matches!(m.clase, None | Some(Clase::Nota))
}

/// Si el texto lo menciona, la pregunta sin la mencion. Sin distinguir
/// mayusculas; la mencion tiene que ser palabra entera (`@asistentes` no).
pub fn mencion(texto: &str) -> Option<String> {
    let bajo = texto.to_lowercase();
    let mut desde = 0;
    while let Some(i) = bajo[desde..].find(MENCION) {
        let i = desde + i;
        let fin = i + MENCION.len();
        let antes_ok = i == 0 || !bajo[..i].chars().next_back().is_some_and(char::is_alphanumeric);
        let despues_ok = !bajo[fin..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_');
        if antes_ok && despues_ok {
            let resto = format!("{}{}", &texto[..i], &texto[fin..]);
            let limpio = resto
                .trim()
                .trim_start_matches([',', ':'])
                .trim()
                .to_string();
            return Some(limpio);
        }
        desde = fin;
    }
    None
}

// ---------------------------------------------------------------------------
// «Nueva conversación» (Windows, 17-oct-2026)

/// En `resto` de la nota que marca una **conversacion nueva**: lo de antes
/// sigue en la vista (y se sincroniza como siempre), pero ya no se le manda
/// al modelo. Es una nota normal del asistente (el movil la ve como una
/// respuesta mas y conserva `resto` tal cual), no un formato nuevo.
pub const NUEVA_CONVERSACION: &str = "nuevaConversacionIa";

/// Lo que dice esa nota en la burbuja.
pub const TEXTO_NUEVA_CONVERSACION: &str =
    "🧹 Conversación nueva: lo de arriba ya no se le manda al asistente.";

/// Si es la marca de una conversacion nueva.
pub fn es_corte(m: &Nota) -> bool {
    es_del_asistente(m) && m.resto.get(NUEVA_CONVERSACION).and_then(|v| v.as_bool()) == Some(true)
}

/// **Lo que el asistente ve de la conversacion**: lo de despues de la ultima
/// «Conversación nueva» (todo, si no hay ninguna).
pub fn desde_el_corte(mensajes: &[Nota]) -> &[Nota] {
    match mensajes.iter().rposition(es_corte) {
        Some(i) => &mensajes[i + 1..],
        None => mensajes,
    }
}

/// La conversacion de antes, para el modelo: `(es_del_usuario, texto)` de
/// las notas (las fotos y ficheros van aparte).
pub fn conversacion(anteriores: &[Nota]) -> Vec<(bool, String)> {
    desde_el_corte(anteriores)
        .iter()
        .filter(|m| es_nota(m) && !m.texto.trim().is_empty())
        .map(|m| (!es_del_asistente(m), m.texto.clone()))
        .collect()
}

/// Lo mismo con el id y el numero de cada nota (`(id, numero, del_usuario,
/// texto)`): la hoja de trabajo apunta desde que nota se reinicio, y
/// `buscar_historial` dice de que mensaje sale cada trozo.
pub fn conversacion_con_ids(anteriores: &[Nota]) -> Vec<(String, i64, bool, String)> {
    desde_el_corte(anteriores)
        .iter()
        .filter(|m| es_nota(m) && !m.texto.trim().is_empty())
        .map(|m| (m.id.clone(), m.numero, !es_del_asistente(m), m.texto.clone()))
        .collect()
}

/// Cuantos mensajes hacia atras se busca la foto de la que se habla.
pub const FOTO_ATRAS: usize = 4;

/// La foto que se le enseña: la ultima imagen de los ultimos
/// [`FOTO_ATRAS`] mensajes (contando el nuevo), sea cual sea la respuesta
/// que hubo en medio.
///
/// Antes se cortaba en la ultima respuesta del asistente, y eso rompia lo
/// mas natural: mandar la foto (el asistente la describe) y luego pedir
/// «dibuja eso»; el modelo ya no la veia y dibujaba lineas a ciegas hasta
/// quedarse sin tokens (13-oct).
pub fn foto_reciente(mensajes: &[Nota]) -> Option<&Nota> {
    mensajes
        .iter()
        .rev()
        .take(FOTO_ATRAS)
        .find(|m| m.clase == Some(Clase::Imagen) && !es_del_asistente(m))
}

/// Lo que se le pregunta al modelo cuando llega una foto sin texto: que
/// conteste siempre, aunque el usuario no haya escrito nada.
pub const PREGUNTA_SOLO_FOTO: &str =
    "Te mando esta imagen. Descríbela en una o dos frases y pregúntame qué quiero hacer con ella.";

/// Si el mensaje nuevo es para el asistente, la pregunta (sin la mencion).
/// En el chat «Asistente» todo lo es: una foto sola tambien, con
/// [`PREGUNTA_SOLO_FOTO`]. En otro chat, solo lo que lo menciona (en una
/// foto, en su pie).
pub fn pregunta_de(m: &Nota, chat_del_asistente: bool) -> Option<String> {
    if es_del_asistente(m) {
        return None;
    }
    let foto = m.clase == Some(Clase::Imagen);
    if !(es_nota(m) || foto) {
        return None;
    }
    let pregunta = if chat_del_asistente {
        m.texto.trim().to_string()
    } else {
        mencion(&m.texto)?
    };
    if pregunta.is_empty() {
        return foto.then(|| PREGUNTA_SOLO_FOTO.to_string());
    }
    Some(pregunta)
}

// ---------------------------------------------------------------------------
// Lienzos: el propio, los mencionados y «Abrir lienzo»

/// En la nota de una respuesta, el lienzo donde dibujo (su referencia).
/// Va en `resto`, que el cuaderno guarda tal cual y el movil respeta.
pub const LIENZO_DE_LA_RESPUESTA: &str = "lienzoIa";

/// El prefijo de la mencion de un lienzo: `@lienzo:<nombre>`.
pub const MENCION_LIENZO: &str = "@lienzo:";

/// Si es un lienzo (o una hoja de PDF con dibujo) en el que se puede dibujar.
pub fn es_lienzo(m: &Nota) -> bool {
    matches!(m.clase, Some(Clase::Dibujo) | Some(Clase::Pagina)) && m.referencia.as_deref().is_some_and(|r| !r.is_empty())
}

/// Como se le llama a un lienzo en la lista y en la mencion.
pub fn nombre_de_lienzo(m: &Nota) -> String {
    // Un lienzo sin nombre lleva el de su fichero (`<id>.excalidraw`), que
    // no le dice nada a nadie.
    let n = m.nombre.trim();
    if !n.is_empty() && !n.ends_with(".excalidraw") {
        n.to_string()
    } else if m.clase == Some(Clase::Pagina) {
        format!("Hoja {}", m.pagina.map_or(m.numero, |p| p as i64 + 1))
    } else {
        format!("Lienzo {}", m.numero)
    }
}

/// La mencion que se escribe para un lienzo: sin espacios (el `_` los
/// hace), para que acabe donde acaba la palabra.
pub fn mencion_de_lienzo(m: &Nota) -> String {
    format!("{MENCION_LIENZO}{}", nombre_de_lienzo(m).replace(char::is_whitespace, "_"))
}

fn sin_tildes(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            '_' => ' ',
            c => c,
        })
        .collect()
}

/// **La pregunta con sus menciones de lienzo resueltas**: el texto para el
/// modelo (la mencion cambiada por «nombre») y la referencia del lienzo
/// nombrado, si existe en la conversacion. Una mencion que no casa con
/// ningun lienzo se deja tal cual y no cambia el destino.
pub fn resolver_lienzo(pregunta: &str, mensajes: &[Nota]) -> (String, Option<String>) {
    let mut texto = String::new();
    let mut destino = None;
    let mut resto = pregunta;
    while let Some(i) = resto.to_lowercase().find(MENCION_LIENZO) {
        texto.push_str(&resto[..i]);
        let tras = &resto[i + MENCION_LIENZO.len()..];
        let fin = tras.find(char::is_whitespace).unwrap_or(tras.len());
        let nombre = tras[..fin].trim_end_matches([',', '.', ';', ':', '?', '!']);
        let buscado = sin_tildes(nombre);
        let hallado = mensajes
            .iter()
            .rev()
            .filter(|m| es_lienzo(m))
            .find(|m| sin_tildes(&nombre_de_lienzo(m)) == buscado);
        match hallado {
            Some(m) => {
                texto.push_str(&format!("el lienzo «{}»", nombre_de_lienzo(m)));
                destino = m.referencia.clone();
            }
            None => texto.push_str(&resto[i..i + MENCION_LIENZO.len() + nombre.len()]),
        }
        resto = &tras[nombre.len()..];
    }
    texto.push_str(resto);
    (texto, destino)
}

/// **El lienzo del chat que se llama asi** (`usar_lienzo`, Windows 17-oct):
/// el nombre entero sin mayusculas ni tildes o, si no hay, el ultimo que lo
/// contiene.
pub fn lienzo_por_nombre(mensajes: &[Nota], nombre: &str) -> Option<usize> {
    let buscado = sin_tildes(nombre.trim());
    if buscado.trim().is_empty() {
        return None;
    }
    let lienzos = || mensajes.iter().enumerate().rev().filter(|(_, m)| es_lienzo(m));
    lienzos()
        .find(|(_, m)| sin_tildes(&nombre_de_lienzo(m)) == buscado)
        .or_else(|| lienzos().find(|(_, m)| sin_tildes(&nombre_de_lienzo(m)).contains(buscado.trim())))
        .map(|(i, _)| i)
}

/// El lienzo propio del asistente en esta conversacion: el ultimo que creo
/// el (su `aparato` es [`APARATO`]).
pub fn lienzo_propio(mensajes: &[Nota]) -> Option<usize> {
    mensajes
        .iter()
        .rposition(|m| m.clase == Some(Clase::Dibujo) && es_del_asistente(m) && es_lienzo(m))
}

/// Si en la pregunta se pide abrir el lienzo al terminar («dibuja… y
/// ábrelo»). Sin pedirlo, se dibuja en segundo plano y la respuesta lleva
/// «Abrir lienzo».
pub fn pide_abrir(pregunta: &str) -> bool {
    let t = sin_tildes(pregunta);
    [
        "abrelo",
        "abremelo",
        "abre el lienzo",
        "abrir el lienzo",
        "abre el dibujo",
        "abrelo al terminar",
        "muestramelo",
        "ensenamelo",
        "y abre",
        "y abrir",
    ]
    .iter()
    .any(|p| t.contains(p))
}

/// El lienzo al que apunta una respuesta, por su posicion en la
/// conversacion.
pub fn lienzo_de_la_respuesta(m: &Nota, mensajes: &[Nota]) -> Option<usize> {
    if !es_del_asistente(m) {
        return None;
    }
    let r = m.resto.get(LIENZO_DE_LA_RESPUESTA)?.as_str()?;
    mensajes.iter().rposition(|x| es_lienzo(x) && x.referencia.as_deref() == Some(r))
}

// ---------------------------------------------------------------------------
// Las fuentes citadas (`buscar_en_mis_cosas`)

/// En la nota de una respuesta, lo que cito de los documentos del usuario:
/// `[{n, titulo, proyecto, pedido}]` (`documentos::herramienta::cita_a_json`).
/// Va en `resto`, como `lienzoIa`.
pub const FUENTES_DE_LA_RESPUESTA: &str = "fuentesIa";

/// Lo que se apunta en la respuesta al acabar: las citas que el modelo
/// nombro (o la primera, si no nombro ninguna). `None` si no busco nada.
pub fn fuentes_para_la_nota(citas: &[pixpin_ia::documentos::Cita], respuesta: &str) -> Option<serde_json::Value> {
    if citas.is_empty() {
        return None;
    }
    let c = pixpin_ia::documentos::Citas { lista: citas.to_vec(), desfase_ms: None };
    let citadas: Vec<serde_json::Value> = c
        .citadas(respuesta)
        .into_iter()
        .map(pixpin_ia::documentos::herramienta::cita_a_json)
        .collect();
    Some(serde_json::Value::Array(citadas))
}

/// Una fuente citada en una respuesta: como se ensena y el pedido que la
/// abre.
#[derive(Debug, Clone, PartialEq)]
pub struct FuenteCitada {
    pub titulo: String,
    pub proyecto: String,
    pub pedido: serde_json::Value,
    /// Cuantas mas cito ademas de esta.
    pub mas: usize,
}

/// La primera fuente citada de una respuesta del asistente, si la hay: de
/// ahi sale su tarjeta «Abrir fuente».
pub fn fuente_de_la_respuesta(m: &Nota) -> Option<FuenteCitada> {
    if !es_del_asistente(m) {
        return None;
    }
    let lista = m.resto.get(FUENTES_DE_LA_RESPUESTA)?.as_array()?;
    let f = lista.first()?;
    Some(FuenteCitada {
        titulo: f.get("titulo")?.as_str()?.to_string(),
        proyecto: f.get("proyecto").and_then(|p| p.as_str()).unwrap_or_default().to_string(),
        pedido: f.get("pedido")?.clone(),
        mas: lista.len() - 1,
    })
}

// ---------------------------------------------------------------------------
// La lista de `@` mientras se escribe

/// Lo que se puede elegir en la lista de `@`.
#[derive(Debug, Clone, PartialEq)]
pub struct Opcion {
    /// Lo que se ve en la lista.
    pub rotulo: String,
    /// Lo que se escribe en la caja al elegirla (con un espacio detras).
    pub mencion: String,
}

/// Cuantas opciones se ensenan como mucho.
pub const OPCIONES_MAX: usize = 6;

/// **Si se esta escribiendo una mencion**: donde empieza su `@` (en bytes)
/// y lo escrito detras, hasta la rayita. Solo si el `@` empieza palabra y
/// no hay un espacio entre el y la rayita.
pub fn consulta_de_mencion(texto: &str, cursor: usize) -> Option<(usize, String)> {
    let antes = texto.get(..cursor)?;
    let arroba = antes.rfind('@')?;
    let escrito = &antes[arroba + 1..];
    if escrito.chars().any(char::is_whitespace) {
        return None;
    }
    let empieza = antes[..arroba].chars().next_back().is_none_or(char::is_whitespace);
    empieza.then(|| (arroba, escrito.to_string()))
}

/// Las opciones para lo escrito tras el `@`: el asistente (fuera de su
/// chat) y los lienzos y hojas de la conversacion, del mas nuevo al mas
/// viejo.
pub fn opciones_de_mencion(mensajes: &[Nota], escrito: &str, chat_del_asistente: bool) -> Vec<Opcion> {
    let buscado = sin_tildes(escrito);
    let casa = |o: &Opcion| {
        buscado.is_empty()
            || sin_tildes(&o.rotulo).contains(&buscado)
            || sin_tildes(o.mencion.trim_start_matches('@')).starts_with(&buscado)
    };
    let mut v = Vec::new();
    if !chat_del_asistente {
        v.push(Opcion {
            rotulo: "🤖 Asistente".into(),
            mencion: MENCION.into(),
        });
    }
    let mut vistas = std::collections::HashSet::new();
    for m in mensajes.iter().rev().filter(|m| es_lienzo(m)) {
        // Una hoja repetida (la misma referencia dos veces) sale una vez.
        if !vistas.insert(m.referencia.clone()) {
            continue;
        }
        let icono = if m.clase == Some(Clase::Pagina) { "📄" } else { "🎨" };
        v.push(Opcion {
            rotulo: format!("{icono} {}", nombre_de_lienzo(m)),
            mencion: mencion_de_lienzo(m),
        });
    }
    v.retain(casa);
    v.truncate(OPCIONES_MAX);
    v
}

/// Escribe la opcion elegida en lugar de lo que se llevaba escrito desde el
/// `@`. Devuelve donde queda la rayita (en bytes).
pub fn poner_mencion(texto: &mut String, arroba: usize, cursor: usize, o: &Opcion) -> usize {
    let cursor = cursor.min(texto.len()).max(arroba);
    let puesta = format!("{} ", o.mencion);
    texto.replace_range(arroba..cursor, &puesta);
    arroba + puesta.len()
}

/// Los adjuntos que el OCR puede leer (fotos y PDF), del mas viejo al mas
/// nuevo. `ruta` resuelve donde esta el fichero de cada mensaje.
pub fn adjuntos(mensajes: &[Nota], ruta: impl Fn(&Nota) -> Option<PathBuf>) -> Vec<AdjuntoIa> {
    mensajes
        .iter()
        .filter(|m| matches!(m.clase, Some(Clase::Imagen) | Some(Clase::Archivo)))
        .filter_map(|m| {
            let r = ruta(m)?;
            let texto = r.to_string_lossy();
            (super::turno::es_imagen(&texto) || es_pdf(&r)).then(|| AdjuntoIa {
                nombre: if m.nombre.is_empty() {
                    r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                } else {
                    m.nombre.clone()
                },
                ruta: texto.into_owned(),
            })
        })
        .collect()
}

fn es_pdf(r: &std::path::Path) -> bool {
    r.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Lo que se ve en la burbuja de la respuesta desde el primer instante,
/// antes de que llegue nada del modelo (`asistente::rapido` de Windows).
pub const ESTADO_INICIAL: &str = "💭 Pensando…";

/// **La burbuja que se escribe en vivo.**
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnVivo {
    pub texto: String,
    /// «Dibujando…» mientras el modelo usa una herramienta.
    pub estado: Option<String>,
    /// Lo que fue mal por el camino (se dice al final, en una linea).
    pub fallos: Vec<String>,
    pub acabado: bool,
    /// La pregunta de una accion que borra o envia, mientras espera el «Sí»
    /// o el «No» de la tarjeta de la burbuja. Contestada, pasa al texto.
    pub pendiente: Option<String>,
    /// Lo que va pensando (la seccion plegable «Pensando…»).
    pub pensamiento: String,
    /// Pensando ahora mismo.
    pub pensando: bool,
    /// Cuanto penso, al acabar de pensar.
    pub segundos: Option<f32>,
    /// La peticion mejorada.
    pub plan: Option<pixpin_ia::pensar::Plan>,
    /// El turno empezo con el contexto renovado.
    pub reiniciado: bool,
    /// El plan del jefe o la lista de pasos del turno y como va cada tarea
    /// (`pixpin_ia::orquesta`, `pixpin_ia::pasos`).
    pub tareas: Vec<pixpin_ia::orquesta::TareaVisible>,
}

/// Como acabo una pregunta de confirmar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contestacion {
    Si,
    No,
    /// Nadie contesto: se cerro el chat, se paro o paso el tiempo.
    Caducada,
}

/// Cuanto espera una pregunta antes de darse por no contestada.
pub const CADUCA_LA_PREGUNTA: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Lo que se ensena mientras se usa cada herramienta.
pub fn rotulo_de(nombre: &str) -> String {
    // La hoja al lienzo cuenta sus zonas: `hoja_a_lienzo:3/9`.
    if let Some(resto) = nombre.strip_prefix(pixpin_ia::hoja::NOMBRE) {
        return match resto.strip_prefix(':') {
            Some(cuenta) => format!("📄 pasando la hoja al lienzo… {cuenta} zonas"),
            None => "📄 pasando la hoja al lienzo…".to_string(),
        };
    }
    match nombre {
        "repasar" => "🔎 repasando el pedido…",
        "jefe:planear" => "🧭 el jefe está planeando…",
        "jefe:supervisar" => "🧐 el jefe revisa la tarea…",
        "jefe:revisar" => "🧐 el jefe revisa el resultado…",
        DIBUJAR => "✏️ dibujando en el lienzo…",
        GRAFICA => "📊 haciendo la gráfica…",
        OCR => "🔎 leyendo el documento…",
        pixpin_ia::ojos::NOMBRE => "👀 mirando el lienzo…",
        pixpin_ia::contexto::BUSCAR_HISTORIAL => "🗂️ buscando en la conversación…",
        CALCAR => "🖼️ calcando la imagen…",
        AYUDA => "📖 mirando la ayuda…",
        pixpin_ia::nube::claude::BUSCAR_EN_LA_WEB => "🌐 buscando en la web…",
        pixpin_ia::nube::claude::LEER_LA_WEB => "🌐 leyendo la página…",
        _ => "⚙️ trabajando…",
    }
    .to_string()
}

impl EnVivo {
    pub fn aplicar(&mut self, e: &Evento) {
        match e {
            Evento::Texto(t) => {
                self.texto.push_str(t);
                self.estado = None;
                self.pensando = false;
            }
            Evento::Pensando(t) => {
                self.pensamiento.push_str(t);
                self.pensando = true;
            }
            Evento::PensoListo { segundos } => {
                self.pensando = false;
                // Con la nube piensa en cada vuelta: se suma.
                *self.segundos.get_or_insert(0.0) += *segundos;
            }
            Evento::Peticion(p) => self.plan = Some(p.clone()),
            Evento::Reinicio { .. } => self.reiniciado = true,
            Evento::Llamando(n) => self.estado = Some(rotulo_de(n)),
            Evento::Herramienta { llamada, .. } => {
                // Lo que diga despues va en su linea, no pegado a lo de antes.
                if !self.texto.is_empty() && !self.texto.ends_with('\n') {
                    self.texto.push('\n');
                }
                self.estado = Some(rotulo_de(llamada.nombre()));
            }
            Evento::Resultado { ok, texto, .. } => {
                // Un «No» en la tarjeta no es un fallo: ya lo dice su linea.
                if !ok && !texto.contains("dijo que no") {
                    self.fallos.push(texto.trim_start_matches("ERROR: ").to_string());
                }
                self.estado = Some("💭 …".into());
            }
            Evento::Reintento { .. } => self.estado = Some("🔁 corrigiendo…".into()),
            Evento::Tareas(t) => self.tareas = t.clone(),
        }
    }

    /// **El plan del jefe** (o la lista de pasos) como lista que se va
    /// marcando, encima de la respuesta: «📋 Plan» y una linea por tarea.
    pub fn plan_visible(&self) -> String {
        use pixpin_ia::orquesta::EstadoTarea as E;
        if self.tareas.is_empty() {
            return String::new();
        }
        let mut s = String::from("📋 Plan\n");
        for (i, t) in self.tareas.iter().enumerate() {
            let marca = match t.estado {
                E::Pendiente => "⬜",
                E::EnCurso => "⏳",
                E::Corrigiendo => "🔁",
                E::Hecha => "✅",
                E::NoSePudo => "✖️",
            };
            s.push_str(&format!("{marca} {}. {}\n", i + 1, t.titulo));
        }
        s
    }

    /// **Una duda con opciones** (`preguntar_usuario`, 22-oct): la tarjeta la
    /// pinta el chat; aqui solo el estado mientras espera.
    pub fn preguntar_con_opciones(&mut self) {
        self.estado = Some("✋ elige una opción o escribe la tuya…".into());
    }

    /// **La duda contestada queda en el historial**, en su linea:
    /// «❓ ¿Traduzco todo o solo la figura? → ✅ Solo la figura». `respuesta`
    /// `None`: nadie contesto y el turno sigue con la recomendada.
    pub fn contestada_con_opciones(&mut self, pregunta: &str, respuesta: Option<&str>, recomendada: &str) {
        if !self.texto.is_empty() && !self.texto.ends_with('\n') {
            self.texto.push('\n');
        }
        let fin = match respuesta {
            Some(r) => format!("✅ {}", r.trim()),
            None => format!("⌛ sin respuesta: sigo con «{}»", recomendada.trim()),
        };
        self.texto.push_str(&format!("❓ {} → {fin}\n", pregunta.trim()));
        self.estado = (!self.acabado).then(|| "💭 …".to_string());
    }

    /// Llega una pregunta: la burbuja ensena su tarjeta con «Sí» y «No».
    pub fn preguntar(&mut self, pregunta: &str) {
        // Una anterior sin contestar (no deberia: el turno espera) caduca.
        self.contestar(Contestacion::Caducada);
        self.pendiente = Some(pregunta.trim().to_string());
        self.estado = Some("✋ esperando tu respuesta…".into());
    }

    /// **La pregunta contestada queda en el historial**, en su linea:
    /// «❓ ¿Borrar la lista «Obra»? → ✅ Sí». `false` si no habia ninguna.
    pub fn contestar(&mut self, c: Contestacion) -> bool {
        let Some(p) = self.pendiente.take() else { return false };
        if !self.texto.is_empty() && !self.texto.ends_with('\n') {
            self.texto.push('\n');
        }
        let fin = match c {
            Contestacion::Si => "✅ Sí",
            Contestacion::No => "✖️ No",
            Contestacion::Caducada => "⌛ sin respuesta: no se hizo nada",
        };
        self.texto.push_str(&format!("❓ {p} → {fin}\n"));
        self.estado = (!self.acabado).then(|| "💭 …".to_string());
        true
    }

    /// El final del turno.
    pub fn acabar(&mut self, r: &Result<Final, String>) {
        self.contestar(Contestacion::Caducada);
        self.acabado = true;
        self.estado = None;
        self.pensando = false;
        // Un `</think>` suelto que se colo en el texto (ver `pensar::Separador`).
        if self.texto.contains("</think>") {
            let (pensado, texto) = pixpin_ia::pensar::separar(&self.texto);
            if self.pensamiento.trim().is_empty() {
                self.pensamiento = pensado;
            }
            self.texto = texto;
        }
        match r {
            Ok(f) => {
                if self.texto.trim().is_empty() {
                    self.texto = f.texto.clone();
                }
                if f.cortado {
                    if !self.texto.is_empty() {
                        self.texto.push(' ');
                    }
                    self.texto.push_str("⏹ (parado)");
                }
            }
            Err(e) => {
                // `e` ya viene en palabras del usuario
                // (`agente::para_el_usuario`): lo tecnico esta en el registro.
                if !self.texto.is_empty() {
                    self.texto.push_str("\n\n");
                }
                self.texto.push_str(&format!("⚠️ {e}"));
            }
        }
    }

    /// El texto de la burbuja tal como se ve ahora.
    pub fn mostrar(&self) -> String {
        let mut s = format!("{}{}", self.plan_visible(), self.texto.trim_end());
        if let Some(e) = &self.estado {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(e);
        } else if !self.acabado {
            s.push_str(" ▍");
        }
        // Con jefe, lo que fallo por el camino ya lo corrigio o lo dice su
        // respuesta: no se repite abajo.
        if self.acabado && !self.fallos.is_empty() && self.tareas.is_empty() {
            s.push_str(&format!("\n⚠️ {}", self.fallos.join(" · ")));
        }
        s.trim_start().to_string()
    }
}

#[cfg(test)]
mod pruebas {
    //! Las de `asistente/pruebas{,_pensado}.rs` de Windows que no necesitan
    //! ni ventana ni modelo.

    use super::*;
    use pixpin_ia::agente::Final;
    use pixpin_ia::pensar::Plan;
    use pixpin_proyecto::cuaderno::Mensaje as Nota;

    #[test]
    fn la_mencion_se_reconoce_y_se_quita() {
        assert_eq!(mencion("@asistente dibuja una casa").as_deref(), Some("dibuja una casa"));
        assert_eq!(mencion("oye @Asistente, que es esto?").as_deref(), Some("oye , que es esto?"));
        assert_eq!(mencion("@asistente: hola").as_deref(), Some("hola"));
        assert_eq!(mencion("hola a todos"), None);
        assert_eq!(mencion("@asistentes hola"), None);
        assert_eq!(mencion("correo@asistente.com"), None);
    }

    fn nota(t: &str, ap: &str) -> Nota {
        Nota {
            id: t.into(),
            texto: t.into(),
            aparato: Some(ap.into()),
            ..Nota::default()
        }
    }

    fn lienzo_de_prueba(nombre: &str, referencia: &str, aparato: &str) -> Nota {
        Nota {
            id: referencia.into(),
            clase: Some(Clase::Dibujo),
            nombre: nombre.into(),
            referencia: Some(referencia.into()),
            aparato: Some(aparato.into()),
            numero: 7,
            ..Default::default()
        }
    }

    #[test]
    fn la_conversacion_separa_usuario_y_asistente_y_la_foto_es_la_reciente() {
        let foto = Nota {
            id: "f".into(),
            clase: Some(Clase::Imagen),
            ruta: Some("imagenes/f.png".into()),
            ..Nota::default()
        };
        let ms = vec![nota("hola", "K7Q2"), nota("que tal", APARATO), foto.clone()];
        let c = conversacion(&ms);
        assert_eq!(c, vec![(true, "hola".to_string()), (false, "que tal".to_string())]);
        assert_eq!(foto_reciente(&ms).map(|m| m.id.as_str()), Some("f"));
        // La foto de la que se hablo antes sigue a la vista: «dibuja eso»
        // tras la descripcion tiene que verla (13-oct).
        let ms2 = vec![foto.clone(), nota("es un plano", APARATO), nota("dibuja eso", "K7Q2")];
        assert_eq!(foto_reciente(&ms2).map(|m| m.id.as_str()), Some("f"));
        // Pero no una de hace mucho.
        let mut ms3 = vec![foto];
        ms3.extend((0..FOTO_ATRAS).map(|i| nota(&format!("m{i}"), "K7Q2")));
        assert!(foto_reciente(&ms3).is_none());
    }

    #[test]
    fn una_foto_sola_al_asistente_es_una_pregunta() {
        let foto = Nota {
            clase: Some(Clase::Imagen),
            aparato: Some("K7Q2".into()),
            ..Default::default()
        };
        // En su chat, sin pie: se le pide que la describa y pregunte.
        assert_eq!(pregunta_de(&foto, true).as_deref(), Some(PREGUNTA_SOLO_FOTO));
        // Con pie, el pie es la pregunta.
        let con_pie = Nota {
            texto: "¿qué es?".into(),
            ..foto.clone()
        };
        assert_eq!(pregunta_de(&con_pie, true).as_deref(), Some("¿qué es?"));
        // En otro chat, solo si el pie lo menciona.
        assert_eq!(pregunta_de(&foto, false), None);
        let con_mencion = Nota {
            texto: "@asistente ¿qué es?".into(),
            ..foto.clone()
        };
        assert_eq!(pregunta_de(&con_mencion, false).as_deref(), Some("¿qué es?"));
        // Lo suyo no se contesta a si mismo, y un PDF suelto no es pregunta.
        assert_eq!(pregunta_de(&nota("hola", APARATO), true), None);
        let pdf = Nota {
            clase: Some(Clase::Archivo),
            ..foto
        };
        assert_eq!(pregunta_de(&pdf, true), None);
    }

    #[test]
    fn la_mencion_de_un_lienzo_se_resuelve_por_su_nombre() {
        let ms = vec![
            lienzo_de_prueba("Plano casa", "ref-plano", "K7Q2"),
            lienzo_de_prueba("", "ref-sin", "K7Q2"),
            lienzo_de_prueba("Lienzo del asistente", "ref-ia", APARATO),
        ];
        assert_eq!(mencion_de_lienzo(&ms[0]), "@lienzo:Plano_casa");
        let (t, d) = resolver_lienzo("dibuja una puerta en @lienzo:Plano_casa, por favor", &ms);
        assert_eq!(t, "dibuja una puerta en el lienzo «Plano casa», por favor");
        assert_eq!(d.as_deref(), Some("ref-plano"));
        // Sin tildes ni mayusculas.
        let (_, d) = resolver_lienzo("en @LIENZO:plano_CASA", &ms);
        assert_eq!(d.as_deref(), Some("ref-plano"));
        // Un lienzo sin nombre se llama por su numero, no por su fichero.
        assert_eq!(nombre_de_lienzo(&lienzo_de_prueba("x.excalidraw", "r", "K")), "Lienzo 7");
        // Una mencion que no casa no cambia nada.
        let (t, d) = resolver_lienzo("en @lienzo:Fachada ya", &ms);
        assert_eq!(t, "en @lienzo:Fachada ya");
        assert_eq!(d, None);
        // El propio del asistente es el suyo, no el ultimo.
        assert_eq!(lienzo_propio(&ms), Some(2));
        assert_eq!(lienzo_propio(&ms[..2]), None);
    }

    #[test]
    fn pedir_que_lo_abra() {
        assert!(pide_abrir("dibuja una casa y ábrelo"));
        assert!(pide_abrir("Dibújalo y abre el lienzo al terminar"));
        assert!(!pide_abrir("dibuja una casa"));
        assert!(!pide_abrir("¿qué es esto?"));
    }

    #[test]
    fn la_lista_de_arroba() {
        let ms = vec![
            lienzo_de_prueba("Plano casa", "ref-plano", "K7Q2"),
            lienzo_de_prueba("Fachada", "ref-fachada", "K7Q2"),
        ];
        // Se abre con un `@` que empieza palabra, hasta el primer espacio.
        let t = "hola @pla";
        assert_eq!(consulta_de_mencion(t, t.len()), Some((5, "pla".into())));
        assert_eq!(consulta_de_mencion("correo@pla", 10), None);
        assert_eq!(consulta_de_mencion("@pla casa", 9), None);
        assert_eq!(consulta_de_mencion("sin nada", 8), None);
        // Fuera del chat del asistente sale tambien el asistente.
        let todas = opciones_de_mencion(&ms, "", false);
        assert_eq!(todas.len(), 3);
        assert_eq!(todas[0].mencion, "@asistente");
        // Del mas nuevo al mas viejo, y filtradas por lo escrito.
        assert_eq!(todas[1].mencion, "@lienzo:Fachada");
        let pla = opciones_de_mencion(&ms, "pla", true);
        assert_eq!(pla.len(), 1);
        assert_eq!(pla[0].mencion, "@lienzo:Plano_casa");
        assert!(opciones_de_mencion(&ms, "lienzo:fa", true)[0].mencion.ends_with("Fachada"));
        assert!(opciones_de_mencion(&ms, "zzz", true).is_empty());
        // Elegir escribe la mencion y deja la rayita detras del espacio.
        let mut texto = "hola @pla y mas".to_string();
        let fin = poner_mencion(&mut texto, 5, 9, &pla[0]);
        assert_eq!(texto, "hola @lienzo:Plano_casa  y mas");
        assert_eq!(&texto[..fin], "hola @lienzo:Plano_casa ");
    }

    fn final_de(texto: &str) -> Final {
        Final {
            texto: texto.into(),
            herramientas_hechas: 0,
            reintentos: 0,
            cortado: false,
        }
    }

    #[test]
    fn la_burbuja_en_vivo() {
        let mut v = EnVivo::default();
        v.aplicar(&Evento::Texto("Voy ".into()));
        assert_eq!(v.mostrar(), "Voy ▍");
        v.aplicar(&Evento::Llamando("dibujar_en_lienzo".into()));
        assert!(v.mostrar().ends_with("dibujando en el lienzo…"));
        v.aplicar(&Evento::Resultado {
            nombre: "dibujar_en_lienzo".into(),
            ok: false,
            texto: "ERROR: el lienzo esta cerrado".into(),
        });
        v.aplicar(&Evento::Texto("No pude.".into()));
        v.acabar(&Ok(final_de("No pude.")));
        assert_eq!(v.mostrar(), "Voy No pude.\n⚠️ el lienzo esta cerrado");
    }

    #[test]
    fn la_burbuja_piensa_en_vivo_se_pliega_y_guarda_la_peticion() {
        let mut v = EnVivo::default();
        v.aplicar(&Evento::Pensando("Quiere ".into()));
        v.aplicar(&Evento::Pensando("una casa.".into()));
        assert!(v.pensando);
        v.aplicar(&Evento::PensoListo { segundos: 12.4 });
        v.aplicar(&Evento::Peticion(Plan::por_omision("casa")));
        v.aplicar(&Evento::Texto("Hecho.".into()));
        assert!(!v.pensando);
        assert_eq!(v.pensamiento, "Quiere una casa.");
        assert_eq!(v.segundos, Some(12.4));
        assert_eq!(v.plan.as_ref().unwrap().entiendo, "casa");
        // El texto de la burbuja es solo la respuesta.
        assert_eq!(v.mostrar(), "Hecho. ▍");
    }

    #[test]
    fn un_cierre_de_pensar_colado_en_el_texto_se_quita_al_acabar() {
        let mut v = EnVivo::default();
        v.aplicar(&Evento::Texto("Pienso que si.".into()));
        v.aplicar(&Evento::Texto("</think>Es azul.".into()));
        v.acabar(&Ok(final_de("Es azul.")));
        assert_eq!(v.mostrar(), "Es azul.");
        assert_eq!(v.pensamiento, "Pienso que si.");
    }

    #[test]
    fn la_pregunta_contestada_queda_en_su_linea() {
        let mut v = EnVivo::default();
        v.aplicar(&Evento::Texto("Voy a borrarla.".into()));
        v.preguntar("¿Borrar la lista «Obra»?");
        assert!(v.mostrar().ends_with("esperando tu respuesta…"));
        assert!(v.contestar(Contestacion::Si));
        // Dos veces no (caso negativo).
        assert!(!v.contestar(Contestacion::No));
        v.preguntar("¿Mandar el correo?");
        v.acabar(&Ok(final_de("Hecho.")));
        let t = v.mostrar();
        assert!(t.contains("❓ ¿Borrar la lista «Obra»? → ✅ Sí"), "{t}");
        assert!(t.contains("❓ ¿Mandar el correo? → ⌛"), "una sin contestar caduca al acabar: {t}");
    }
}

// ---------------------------------------------------------------------------
// Las listas de la respuesta, como listas (17-oct-2026)

/// **El texto de una respuesta del asistente como se pinta en su burbuja**
/// (el usuario, 17-oct-2026: «que las respuestas en lista tengan un formato
/// mas claro: lo da todo en texto y confunde»). El modelo escribe Markdown
/// ligero; la burbuja es texto con tramos, asi que aqui se pasa a algo que
/// se lee:
///
/// - `- x`, `* x`, `+ x` → `•  x` (con sangria por nivel: `◦` en el
///   segundo); `1. x` / `1) x` se queda numerada.
/// - `# Titulo` … `###### Titulo` → la linea en negrita, sin almohadillas.
/// - `**x**` y `__x__` → `x` en negrita; `` `x` `` → `x`.
/// - Las vallas de codigo (```` ``` ````) se quitan; lo de dentro, tal cual.
///
/// Devuelve el texto y los tramos en negrita como `(inicio, longitud)` en
/// unidades UTF-16 (las de DirectWrite). Lo guardado no cambia: copiar la
/// burbuja copia lo que escribio el modelo.
pub fn formato_de_respuesta(texto: &str) -> (String, Vec<(u32, u32)>) {
    let mut salida = String::with_capacity(texto.len());
    let mut negritas: Vec<(u32, u32)> = Vec::new();
    // Lo escrito, en UTF-16.
    let mut u16s: u32 = 0;
    let mut en_codigo = false;
    let mut primera = true;
    for linea in texto.lines() {
        let t = linea.trim_end();
        if t.trim_start().starts_with("```") {
            en_codigo = !en_codigo;
            continue;
        }
        if !primera {
            salida.push('\n');
            u16s += 1;
        }
        primera = false;
        let mut empujar = |s: &str, negrita: bool, salida: &mut String, u16s: &mut u32| {
            let n = s.encode_utf16().count() as u32;
            if negrita && n > 0 {
                negritas.push((*u16s, n));
            }
            salida.push_str(s);
            *u16s += n;
        };
        if en_codigo {
            empujar(t, false, &mut salida, &mut u16s);
            continue;
        }
        let sangria = t.len() - t.trim_start().len();
        let cuerpo = t.trim_start();
        // Titulo: toda la linea en negrita.
        let almohadillas = cuerpo.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&almohadillas) && cuerpo[almohadillas..].starts_with(' ') {
            let titulo = sin_marcas_en_linea(cuerpo[almohadillas..].trim());
            empujar(&titulo, true, &mut salida, &mut u16s);
            continue;
        }
        // Viñeta o numero.
        let nivel = sangria / 2;
        let (prefijo, resto) = if let Some(r) = ["- ", "* ", "+ "].iter().find_map(|m| cuerpo.strip_prefix(m)) {
            let vineta = if nivel == 0 { "•  " } else { "◦  " };
            (format!("{}{vineta}", "    ".repeat(nivel)), r)
        } else if let Some((n, r)) = numero_de_lista(cuerpo) {
            (format!("{}{n}. ", "    ".repeat(nivel)), r)
        } else {
            (" ".repeat(sangria), cuerpo)
        };
        empujar(&prefijo, false, &mut salida, &mut u16s);
        // `**negrita**` dentro de la linea.
        let mut resto = resto;
        loop {
            let marca = ["**", "__"].iter().filter_map(|m| resto.find(m).map(|i| (i, *m))).min_by_key(|(i, _)| *i);
            let Some((i, m)) = marca else { break };
            let tras = &resto[i + 2..];
            let Some(j) = tras.find(m) else { break };
            if j == 0 {
                break;
            }
            empujar(&sin_comillas_de_codigo(&resto[..i]), false, &mut salida, &mut u16s);
            empujar(&sin_comillas_de_codigo(&tras[..j]), true, &mut salida, &mut u16s);
            resto = &tras[j + 2..];
        }
        empujar(&sin_comillas_de_codigo(resto), false, &mut salida, &mut u16s);
    }
    (salida, negritas)
}

/// `12. resto` o `12) resto`: el numero y el resto.
fn numero_de_lista(s: &str) -> Option<(&str, &str)> {
    let digitos = s.chars().take_while(char::is_ascii_digit).count();
    if digitos == 0 || digitos > 3 {
        return None;
    }
    let tras = &s[digitos..];
    let r = tras.strip_prefix(". ").or_else(|| tras.strip_prefix(") "))?;
    Some((&s[..digitos], r))
}

fn sin_comillas_de_codigo(s: &str) -> String {
    s.replace('`', "")
}

fn sin_marcas_en_linea(s: &str) -> String {
    sin_comillas_de_codigo(&s.replace("**", "").replace("__", ""))
}

#[cfg(test)]
mod pruebas_formato {
    use super::*;

    fn negrita<'a>(t: &'a str, (i, n): (u32, u32)) -> String {
        let u: Vec<u16> = t.encode_utf16().collect();
        String::from_utf16(&u[i as usize..(i + n) as usize]).unwrap()
    }

    #[test]
    fn las_listas_se_ven_como_listas() {
        let (t, b) = formato_de_respuesta(
            "### Tus archivos\nTienes **3** archivos:\n- `plano.pdf` (vie 3-oct 14:05)\n  - revisión\n* informe.docx\n1. primero\n2) segundo",
        );
        assert_eq!(
            t,
            "Tus archivos\nTienes 3 archivos:\n•  plano.pdf (vie 3-oct 14:05)\n    ◦  revisión\n•  informe.docx\n1. primero\n2. segundo"
        );
        let nb: Vec<String> = b.iter().map(|x| negrita(&t, *x)).collect();
        assert_eq!(nb, vec!["Tus archivos", "3"]);
    }

    #[test]
    fn lo_que_no_es_markdown_no_se_toca() {
        // Casos negativos: un guion en medio, un numero sin punto, un
        // asterisco suelto, emojis (UTF-16 de dos unidades) y el codigo.
        let s = "a - b\n2026 fue bueno\n5 * 3 = 15\n✏️ dibujando…\n```\n- no es lista\n```";
        let (t, b) = formato_de_respuesta(s);
        assert_eq!(t, "a - b\n2026 fue bueno\n5 * 3 = 15\n✏️ dibujando…\n- no es lista");
        assert!(b.is_empty());
        let (t, b) = formato_de_respuesta("🎨 **Lienzo** listo");
        assert_eq!(negrita(&t, b[0]), "Lienzo");
    }
}
