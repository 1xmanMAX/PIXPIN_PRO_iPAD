//! **Las tareas del usuario, por su nombre**, para el asistente (14-oct).
//!
//! Las listas son las de siempre (la mini-app `tareas` de un mensaje del
//! chat, que se sincroniza con el movil) y se cambian con los pedidos de
//! siempre (`anadir_tarea`, `marcar_tarea`, `mover_tarea`, `corregir_tarea`,
//! `quitar_tarea`, `borrar_lista`), que piden ids y numeros. El modelo, y el
//! usuario, hablan de «la tarea comprar pan de la lista Compras» y de
//! «mañana a las 10». Aqui se hace el cambio, **sin tocar nada**: la app da
//! las listas con [`FuenteTareas`] y recibe los pedidos ya completos.
//!
//! - [`resolver`]: de una [`AccionApp`] con nombres a sus pedidos, la
//!   pregunta de confirmar con los nombres de verdad y la frase de «hecho».
//! - [`consultar`]: «¿que tengo pendiente?», «¿que vencio esta semana?»,
//!   como una lista clara, agrupada por lista.
//! - [`hacer`]: todo junto, con la confirmacion y el envio de quien llama.
//!
//! Lo que no casa no se adivina: una tarea que no esta, dos que se parecen
//! igual, una lista de otro chat o una fecha imposible son un `Err` con el
//! motivo y lo que si hay, para que el modelo pregunte o elija.
//!
//! La fecha de una tarea es su marca `⏰ AAAA-MM-DD HH:MM` (hora local) al
//! final de lo que se ve, la misma del recordatorio de la ventana de tareas
//! y del movil (`Tareas.marcaDeHora`): ver [`crate::fechas`].

use serde_json::{Map, Value, json};

use crate::acciones::{AccionApp, normalizar, rellenar};
use crate::fechas::{self, Momento};

/// Una tarea como la ve el asistente.
#[derive(Debug, Clone, PartialEq)]
pub struct VistaTarea {
    /// Su numero en el documento (desde 0): el `indice` de los pedidos.
    pub indice: usize,
    /// Lo que se lee: sin fecha de creacion, sin imagenes y sin la marca ⏰.
    pub texto: String,
    pub hecha: bool,
    /// El dia en que se creo (dias desde 1970), si lo lleva.
    pub creada: Option<i64>,
    /// Su fecha (la marca ⏰), si la lleva.
    pub vence: Option<Momento>,
}

/// Una lista de tareas de un chat.
#[derive(Debug, Clone, PartialEq)]
pub struct VistaLista {
    /// El id del chat (la carpeta del proyecto).
    pub proyecto: String,
    /// Como se llama el chat.
    pub chat: String,
    /// Si el chat es «Mensajes guardados».
    pub guardados: bool,
    /// El id del mensaje de la lista.
    pub codigo: String,
    pub titulo: String,
    /// Cuando se creo (ms): de dos con el mismo nombre, la nueva es la ultima.
    pub cuando: i64,
    pub tareas: Vec<VistaTarea>,
}

impl VistaLista {
    /// El Inbox: la lista «Inbox» de «Mensajes guardados», adonde va todo lo
    /// que se apunta sin decir donde.
    pub fn es_inbox(&self) -> bool {
        self.guardados && normalizar(self.titulo.trim()) == "inbox"
    }

    /// «Compras» o «Compras (Obra)» si es de otro chat que el de guardados.
    pub fn nombre(&self) -> String {
        if self.guardados {
            self.titulo.clone()
        } else {
            format!("{} ({})", self.titulo, self.chat)
        }
    }
}

/// Un chat (para «la lista Compras del chat Obra» o «crea la lista en Obra»).
#[derive(Debug, Clone, PartialEq)]
pub struct Chat {
    pub id: String,
    pub nombre: String,
    pub guardados: bool,
}

/// **Lo que la app ensena al asistente**: sus listas, sus chats y la hora.
pub trait FuenteTareas {
    fn listas(&self) -> Vec<VistaLista>;
    fn chats(&self) -> Vec<Chat>;
    /// Ahora, en hora local.
    fn ahora(&self) -> Momento;
}

/// Unas listas fijas (pruebas, banco).
#[derive(Debug, Clone, Default)]
pub struct Fijas {
    pub listas: Vec<VistaLista>,
    pub chats: Vec<Chat>,
    pub ahora: Option<Momento>,
}

impl FuenteTareas for Fijas {
    fn listas(&self) -> Vec<VistaLista> {
        self.listas.clone()
    }
    fn chats(&self) -> Vec<Chat> {
        self.chats.clone()
    }
    fn ahora(&self) -> Momento {
        self.ahora.unwrap_or(Momento { dia: 0, minuto: 0 })
    }
}

/// Lo que llevan todos los motivos de [`resolver`] cuando no se toco nada
/// porque no casa lo que se pidio (una tarea que no esta, una fecha que no
/// existe...): es una respuesta para el usuario, no un fallo tecnico, y el
/// camino rapido la dice tal cual (`agente`).
pub const NO_HICE_NADA: &str = "No hice nada";

// ------------------------------------------------------------ parecerse

/// Las palabras que cuentan de un texto (sin tildes, sin relleno).
fn palabras(s: &str) -> Vec<String> {
    const RELLENO: &[&str] = &[
        "el", "la", "los", "las", "de", "del", "a", "al", "que", "en", "y", "un", "una", "unos", "unas", "para", "por",
        "mi", "mis", "lo", "tarea", "tareas", "lista", "con",
    ];
    normalizar(s)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty() && !RELLENO.contains(p))
        .map(str::to_string)
        .collect()
}

/// **Cuanto se parece** lo que dijo el usuario a un texto: 100 igual, 85 lo
/// contiene entero, 70 esta contenido, hasta 60 si comparten casi todas las
/// palabras, 0 nada.
pub fn parecido(consulta: &str, texto: &str) -> u32 {
    let q = palabras(consulta);
    let t = palabras(texto);
    if q.is_empty() || t.is_empty() {
        return 0;
    }
    if q == t {
        return 100;
    }
    let contiene = |a: &[String], b: &[String]| b.len() <= a.len() && a.windows(b.len()).any(|w| w == b);
    if contiene(&t, &q) {
        return 85;
    }
    if contiene(&q, &t) {
        return 70;
    }
    // Palabras en comun (una palabra vale si empieza igual: «llamar»/«llama»).
    let igual = |a: &str, b: &str| a == b || (a.len() >= 4 && b.len() >= 4 && (a.starts_with(b) || b.starts_with(a)));
    let comunes = q.iter().filter(|p| t.iter().any(|x| igual(p, x))).count();
    let ratio = comunes as f32 / q.len() as f32;
    if comunes > 0 && ratio >= 0.6 {
        (40.0 + 20.0 * ratio) as u32
    } else {
        0
    }
}

fn es_guardados(nombre: &str) -> bool {
    matches!(normalizar(nombre.trim()).as_str(), "mensajes guardados" | "guardados" | "mis mensajes")
}

/// El chat que dijo el usuario.
pub fn buscar_chat<'a>(chats: &'a [Chat], nombre: &str) -> Result<&'a Chat, String> {
    if es_guardados(nombre)
        && let Some(c) = chats.iter().find(|c| c.guardados)
    {
        return Ok(c);
    }
    let mut v: Vec<(u32, &Chat)> = chats.iter().map(|c| (parecido(nombre, &c.nombre), c)).filter(|x| x.0 > 0).collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    match v.as_slice() {
        [] => Err(format!(
            "No hay ningun chat «{nombre}». Chats: {}. No hice nada.",
            chats.iter().map(|c| format!("«{}»", c.nombre)).collect::<Vec<_>>().join(", ")
        )),
        [(a, x), (b, _), ..] if a == b => Err(format!(
            "Hay varios chats que se parecen a «{nombre}» (p. ej. «{}»). No hice nada: di cual.",
            x.nombre
        )),
        [(_, c), ..] => Ok(c),
    }
}

/// **La lista que dijo el usuario**, en el chat que dijo (si lo dijo).
pub fn buscar_lista<'a>(
    listas: &'a [VistaLista],
    nombre: &str,
    chat: Option<&str>,
    chats: &[Chat],
) -> Result<&'a VistaLista, String> {
    let q = normalizar(nombre.trim());
    // Un chat que no existe no filtra: los modelos pequeños se inventan uno
    // («PixPin», medido con el 2B). Uno que existe, si (y si la lista esta
    // en otro, se dice).
    let chat = chat.filter(|c| !c.trim().is_empty() && buscar_chat(chats, c).is_ok());
    let en_chat: Option<String> = chat.and_then(|c| buscar_chat(chats, c).ok()).map(|c| c.id.clone());
    let candidatas: Vec<(u32, &VistaLista)> = listas
        .iter()
        .map(|l| {
            let s = if matches!(q.as_str(), "inbox" | "bandeja" | "bandeja de entrada") && l.es_inbox() {
                100
            } else {
                parecido(nombre, &l.titulo)
            };
            (s, l)
        })
        .filter(|x| x.0 > 0)
        .collect();
    let en: Vec<(u32, &VistaLista)> = candidatas
        .iter()
        .filter(|(_, l)| en_chat.as_ref().is_none_or(|c| &l.proyecto == c))
        .cloned()
        .collect();
    if en.is_empty() {
        if let (Some(c), Some((_, otra))) = (chat, candidatas.first()) {
            return Err(format!(
                "La lista «{}» esta en el chat «{}», no en «{c}». No hice nada: di si la quieres alli o si creo una nueva en «{c}».",
                otra.titulo, otra.chat
            ));
        }
        let todas: Vec<String> = listas.iter().take(12).map(|l| format!("«{}»", l.nombre())).collect();
        return Err(format!(
            "No hay ninguna lista «{nombre}»{}. Tus listas: {}. No hice nada.",
            chat.map(|c| format!(" en «{c}»")).unwrap_or_default(),
            if todas.is_empty() { "ninguna".to_string() } else { todas.join(", ") }
        ));
    }
    let mejor = en.iter().map(|x| x.0).max().unwrap_or(0);
    let top: Vec<&VistaLista> = en.iter().filter(|x| x.0 == mejor).map(|x| x.1).collect();
    if top.len() > 1 {
        return Err(format!(
            "Hay varias listas «{nombre}»: {}. No hice nada: di de que chat.",
            top.iter().map(|l| format!("«{}»", l.nombre())).collect::<Vec<_>>().join(", ")
        ));
    }
    Ok(top[0])
}

/// **La tarea que dijo el usuario.** `lista`: donde buscar (si la dijo).
/// `prefiere_hecha`: de dos que se parecen igual, la que esta en ese estado
/// (para marcar como hecha se busca entre las pendientes).
pub fn buscar_tarea<'a>(
    listas: &'a [VistaLista],
    consulta: &str,
    lista: Option<&'a VistaLista>,
    prefiere_hecha: Option<bool>,
) -> Result<(&'a VistaLista, &'a VistaTarea), String> {
    let donde: Vec<&VistaLista> = match lista {
        Some(l) => vec![l],
        None => listas.iter().collect(),
    };
    // «la 2», «#2», «2» dentro de una lista dicha: el numero que ve el usuario.
    let q = consulta.trim().trim_start_matches('#');
    let q = q.strip_prefix("la ").unwrap_or(q);
    if let (Some(l), Ok(n)) = (lista, q.trim().parse::<usize>()) {
        return match l.tareas.get(n.wrapping_sub(1)) {
            Some(t) => Ok((l, t)),
            None => Err(format!("La lista «{}» no tiene tarea {n} (tiene {}). No hice nada.", l.nombre(), l.tareas.len())),
        };
    }
    let mut v: Vec<(u32, &VistaLista, &VistaTarea)> = Vec::new();
    for l in &donde {
        for t in &l.tareas {
            let s = parecido(consulta, &t.texto);
            if s > 0 {
                let estado = u32::from(prefiere_hecha.is_some_and(|h| h == t.hecha));
                v.push((s * 2 + estado, l, t));
            }
        }
    }
    v.sort_by(|a, b| b.0.cmp(&a.0));
    match v.as_slice() {
        [] => {
            let pendientes: Vec<String> = donde
                .iter()
                .flat_map(|l| l.tareas.iter().filter(|t| !t.hecha))
                .take(6)
                .map(|t| format!("«{}»", t.texto))
                .collect();
            Err(format!(
                "No encuentro ninguna tarea «{consulta}»{}. No hice nada. {}",
                lista.map(|l| format!(" en la lista «{}»", l.nombre())).unwrap_or_default(),
                if pendientes.is_empty() {
                    "No hay tareas pendientes.".to_string()
                } else {
                    format!("Pendientes: {}.", pendientes.join(", "))
                }
            ))
        }
        [(a, la, ta), (b, lb, tb), ..] if a == b && !(la.codigo == lb.codigo && ta.indice == tb.indice) => {
            let iguales: Vec<String> = v
                .iter()
                .filter(|x| x.0 == *a)
                .take(5)
                .map(|(_, l, t)| format!("«{}» ({})", t.texto, l.nombre()))
                .collect();
            Err(format!(
                "Hay varias tareas que se parecen a «{consulta}»: {}. No hice nada: di cual (o de que lista).",
                iguales.join(", ")
            ))
        }
        [(_, l, t), ..] => Ok((l, t)),
    }
}

// ------------------------------------------------------------ resolver

/// Lo que hay que hacer de verdad para una [`AccionApp`].
#[derive(Debug, Clone, PartialEq)]
pub struct Pasos {
    /// Los pedidos, en orden. Uno marcado con `__en_lista_nueva` se completa
    /// tras crear esa lista ([`hacer`]).
    pub pedidos: Vec<Value>,
    /// La pregunta antes de hacerlo, con los nombres de verdad.
    pub confirmar: Option<String>,
    /// La frase de «hecho» para la burbuja y el modelo.
    pub hecho: String,
}

fn texto_de(m: &Map<String, Value>, k: &str) -> Option<String> {
    m.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// La marca de fecha leida con el reloj de la fuente.
fn fecha(texto: &str, ahora: Momento) -> Result<Momento, String> {
    fechas::interpretar(texto, ahora).map_err(|e| {
        let mut m = e.to_string();
        m.replace_range(..1, &m[..1].to_uppercase());
        format!("{m}. No hice nada.")
    })
}

/// **De nombres a pedidos.** `Err`: el motivo, en una frase para el modelo
/// (o la burbuja), con lo que si hay.
pub fn resolver(a: &AccionApp, f: &dyn FuenteTareas) -> Result<Pasos, String> {
    let fila = a.fila();
    let mut valores = a.valores();
    let poner_valor = |valores: &mut Vec<(String, String)>, k: &str, v: String| {
        valores.retain(|(x, _)| x != k);
        valores.push((k.to_string(), v));
    };
    if a.por_resolver.is_empty() {
        return Ok(Pasos {
            pedidos: vec![a.pedido.clone()],
            confirmar: a.confirmar.clone(),
            hecho: rellenar(fila.hecho, &valores),
        });
    }
    let r = &a.por_resolver;
    let listas = f.listas();
    let chats = f.chats();
    let ahora = f.ahora();
    let mut pedido = a.pedido.clone();
    let chat = texto_de(r, "chat");
    let accion = fila.accion;

    // La lista de origen (si la dijo).
    // Con una tarea, una «lista» que no es ninguna lista suele ser un trozo
    // de la tarea mal puesto por el modelo (el 2B: tarea «tachalo de mis
    // pendientes», lista «encofrado»): se usa como pista para buscarla.
    let mut pista: Option<String> = None;
    let lista = match texto_de(r, "lista") {
        Some(n) if fila.herramienta != "anadir_tareas" || texto_de(r, "lista_nueva").is_none() => {
            match buscar_lista(&listas, &n, chat.as_deref(), &chats) {
                Ok(l) => Some(l),
                Err(e) if texto_de(r, "tarea").is_some() && e.starts_with("No hay ninguna lista") => {
                    pista = Some(n);
                    None
                }
                Err(e) => return Err(e),
            }
        }
        _ => None,
    };
    if let Some(l) = lista {
        pedido["codigo"] = json!(l.codigo);
        pedido["proyecto"] = json!(l.proyecto);
        poner_valor(&mut valores, "lista", l.nombre());
    } else if let Some(c) = &chat {
        // Sin lista pero con chat: crear o apuntar en ese chat.
        let c = buscar_chat(&chats, c)?;
        pedido["proyecto"] = json!(c.id);
        poner_valor(&mut valores, "chat", c.nombre.clone());
    }

    // La tarea (marcar, mover, editar, quitar).
    let mut tarea: Option<(&VistaLista, &VistaTarea)> = None;
    if let Some(t) = texto_de(r, "tarea") {
        let prefiere = match accion {
            "marcar_tarea" => Some(!pedido["hecha"].as_bool().unwrap_or(true)),
            _ => Some(false),
        };
        let (l, x) = match buscar_tarea(&listas, &t, lista, prefiere) {
            Ok(x) => x,
            Err(e) => match pista.as_deref().map(|p| buscar_tarea(&listas, p, None, prefiere)) {
                Some(Ok(x)) => x,
                _ => return Err(e),
            },
        };
        pedido["codigo"] = json!(l.codigo);
        pedido["proyecto"] = json!(l.proyecto);
        pedido["indice"] = json!(x.indice);
        poner_valor(&mut valores, "tarea", x.texto.clone());
        poner_valor(&mut valores, "lista", l.nombre());
        tarea = Some((l, x));
        if accion == "marcar_tarea" {
            let hecha = pedido["hecha"].as_bool().unwrap_or(true);
            poner_valor(&mut valores, "estado", if hecha { "hecha" } else { "pendiente" }.into());
        }
    }

    // La lista de destino (mover).
    if let Some(n) = texto_de(r, "a_lista") {
        let destino = buscar_lista(&listas, &n, None, &chats)?;
        if let Some((l, _)) = tarea
            && l.codigo == destino.codigo
        {
            return Err(format!("La tarea ya esta en la lista «{}». No hice nada.", destino.nombre()));
        }
        pedido["a_codigo"] = json!(destino.codigo);
        pedido["a_proyecto"] = json!(destino.proyecto);
        poner_valor(&mut valores, "a_lista", destino.nombre());
    }

    // La fecha: la marca ⏰ al final del texto.
    let cuando = match texto_de(r, "fecha") {
        Some(t) => Some(fecha(&t, ahora)?),
        None => None,
    };
    if let Some(m) = cuando {
        poner_valor(&mut valores, "fecha", m.legible(ahora));
    }
    if accion == "corregir_tarea" {
        let (_, x) = tarea.expect("editar_tarea pide la tarea");
        let base = pedido["texto"].as_str().map(str::to_string).unwrap_or_else(|| x.texto.clone());
        if pedido["texto"].is_null() && cuando.is_none() {
            return Err("Para cambiar una tarea dime el texto nuevo o la fecha nueva. No hice nada.".into());
        }
        // Sin fecha nueva, la de antes se queda.
        let marca = cuando.or(x.vence).map(|m| format!(" {}", m.marca())).unwrap_or_default();
        pedido["texto"] = json!(format!("{}{marca}", base.trim()));
        poner_valor(&mut valores, "texto", base.trim().to_string());
        if cuando.is_none()
            && let Some(v) = x.vence
        {
            poner_valor(&mut valores, "fecha", v.legible(ahora));
        }
    } else if let Some(m) = cuando
        && let Some(t) = pedido["texto"].as_str().map(str::to_string)
    {
        pedido["texto"] = json!(format!("{} {}", t.trim(), m.marca()));
    }

    let mut pedidos = vec![pedido.clone()];

    // Varias tareas de una vez, en una lista que existe o en una nueva.
    if let Some(Value::Array(items)) = r.get("tareas") {
        pedidos.clear();
        let base = {
            let mut b = pedido.clone();
            b["accion"] = json!("anadir_tarea");
            b
        };
        let mut nombre_lista = lista.map(VistaLista::nombre).unwrap_or_else(|| "Inbox".to_string());
        if let Some(t) = texto_de(r, "lista_nueva") {
            let mut nueva = json!({"pixpin": 1, "accion": "lista_nueva", "titulo": t});
            if let Some(p) = base.get("proyecto") {
                nueva["proyecto"] = p.clone();
            }
            pedidos.push(nueva);
            nombre_lista = t.clone();
        }
        for it in items {
            let Some(texto) = it.as_str().map(str::trim).filter(|s| !s.is_empty()) else { continue };
            let mut p = base.clone();
            // Cada una puede traer su fecha al final («revisar planos el viernes»).
            let (texto, cuando) = fechas::separar(texto, ahora).unwrap_or((texto.to_string(), None));
            p["texto"] = json!(match cuando {
                Some(m) => format!("{texto} {}", m.marca()),
                None => texto,
            });
            if let Some(t) = texto_de(r, "lista_nueva") {
                p["__en_lista_nueva"] = json!(t);
            }
            pedidos.push(p);
        }
        poner_valor(&mut valores, "n", (pedidos.len() - usize::from(texto_de(r, "lista_nueva").is_some())).to_string());
        poner_valor(&mut valores, "lista", nombre_lista);
    }

    let confirmar = fila.confirmar.map(|q| rellenar(q, &valores));
    Ok(Pasos {
        pedidos,
        confirmar,
        hecho: rellenar(fila.hecho, &valores),
    })
}

/// Completa un pedido que va a una lista recien creada: su codigo y su chat,
/// leidos de nuevo. La mas nueva con ese titulo.
pub fn en_lista_nueva(pedido: &mut Value, f: &dyn FuenteTareas) -> Result<(), String> {
    let Some(titulo) = pedido.get("__en_lista_nueva").and_then(Value::as_str).map(str::to_string) else {
        return Ok(());
    };
    let proyecto = pedido.get("proyecto").and_then(Value::as_str).map(str::to_string);
    let l = f
        .listas()
        .into_iter()
        .filter(|l| l.titulo.trim() == titulo.trim() && proyecto.as_ref().is_none_or(|p| &l.proyecto == p))
        .max_by_key(|l| l.cuando)
        .ok_or_else(|| format!("No encuentro la lista nueva «{titulo}» recien creada."))?;
    pedido["codigo"] = json!(l.codigo);
    pedido["proyecto"] = json!(l.proyecto);
    if let Some(o) = pedido.as_object_mut() {
        o.remove("__en_lista_nueva");
    }
    Ok(())
}

/// **Hace la accion entera**: consulta, o resuelve, pregunta si hace falta y
/// manda los pedidos en orden. `Ok`: la frase de hecho (o la consulta).
pub fn hacer(
    a: &AccionApp,
    f: &dyn FuenteTareas,
    confirmar: &dyn Fn(&str) -> bool,
    mandar: &mut dyn FnMut(&Value) -> Result<(), String>,
) -> Result<String, String> {
    if a.es_consulta() {
        return consultar(&a.por_resolver, f);
    }
    let pasos = resolver(a, f)?;
    if let Some(q) = &pasos.confirmar
        && !confirmar(q)
    {
        return Err("el usuario dijo que no; no se hizo nada".into());
    }
    for mut p in pasos.pedidos {
        en_lista_nueva(&mut p, f)?;
        mandar(&p)?;
    }
    Ok(pasos.hecho)
}

// ------------------------------------------------------------ consultar

/// Cuantas tareas como mucho en una respuesta (el resto se cuenta).
pub const MAXIMO_EN_CONSULTA: usize = 30;

/// **«¿Que tengo pendiente?»**: las tareas que piden `filtro` (los campos
/// de `consultar_tareas`), agrupadas por lista, con su fecha.
pub fn consultar(filtro: &Map<String, Value>, f: &dyn FuenteTareas) -> Result<String, String> {
    let ahora = f.ahora();
    let listas = f.listas();
    let chats = f.chats();
    let estado = texto_de(filtro, "estado").unwrap_or_else(|| "pendientes".into());
    let periodo = texto_de(filtro, "periodo").unwrap_or_else(|| "todo".into());
    let chat = texto_de(filtro, "chat");
    // Un modelo pequeño repite el chat o la lista en «buscar» (medido con el
    // 2B: chat y buscar «Casa Lima», y ninguna tarea dice «Casa Lima»).
    let buscar = texto_de(filtro, "buscar").filter(|b| {
        [chat.as_deref(), texto_de(filtro, "lista").as_deref()]
            .into_iter()
            .flatten()
            .all(|x| parecido(b, x) < 70)
    });
    let elegidas: Vec<&VistaLista> = match texto_de(filtro, "lista") {
        Some(n) => vec![buscar_lista(&listas, &n, chat.as_deref(), &chats)?],
        None => match &chat {
            Some(c) => {
                let id = &buscar_chat(&chats, c)?.id;
                listas.iter().filter(|l| &l.proyecto == id).collect()
            }
            None => listas.iter().collect(),
        },
    };
    // El periodo, en dias locales: [desde, hasta].
    let lunes = ahora.dia - fechas::dia_de_la_semana(ahora.dia);
    let (y, m, _) = fechas::fecha_de(ahora.dia);
    let fin_de_mes = if m == 12 { fechas::dias_de(y + 1, 1, 1) } else { fechas::dias_de(y, m + 1, 1) } - 1;
    let rango = match periodo.as_str() {
        "hoy" => Some((ahora.dia, ahora.dia)),
        "semana" => Some((lunes, lunes + 6)),
        "mes" => Some((fechas::dias_de(y, m, 1), fin_de_mes)),
        _ => None,
    };
    let vale = |t: &VistaTarea| {
        let por_estado = match estado.as_str() {
            "hechas" => t.hecha,
            "vencidas" => !t.hecha && t.vence.is_some_and(|v| v < ahora),
            "todas" => true,
            _ => !t.hecha,
        };
        let por_fecha = rango.is_none_or(|(a, b)| t.vence.is_some_and(|v| v.dia >= a && v.dia <= b));
        let por_texto = buscar.as_deref().is_none_or(|q| parecido(q, &t.texto) > 0);
        por_estado && por_fecha && por_texto
    };
    let mut lineas = Vec::new();
    let mut cuantas = 0;
    let mut puestas = 0;
    for l in &elegidas {
        let mut ts: Vec<&VistaTarea> = l.tareas.iter().filter(|t| vale(t)).collect();
        if ts.is_empty() {
            continue;
        }
        // Lo que tiene fecha primero, la mas cercana arriba.
        ts.sort_by_key(|t| (t.vence.is_none(), t.vence, t.indice));
        cuantas += ts.len();
        if puestas >= MAXIMO_EN_CONSULTA {
            continue;
        }
        lineas.push(format!("{}:", l.nombre()));
        for t in ts {
            if puestas >= MAXIMO_EN_CONSULTA {
                break;
            }
            puestas += 1;
            let casilla = if t.hecha { "☑" } else { "☐" };
            let cuando = match t.vence {
                Some(v) if !t.hecha && v < ahora => format!(" — venció {}", v.legible(ahora)),
                Some(v) => format!(" — {}", v.legible(ahora)),
                None => String::new(),
            };
            lineas.push(format!("  {casilla} {}{cuando}", t.texto));
        }
    }
    let que = match estado.as_str() {
        "hechas" => "hechas",
        "vencidas" => "vencidas",
        "todas" => "",
        _ => "pendientes",
    };
    let cuando = match periodo.as_str() {
        "hoy" => " para hoy",
        "semana" => " de esta semana",
        "mes" => " de este mes",
        _ => "",
    };
    if cuantas == 0 {
        return Ok(format!("No tienes tareas {que}{cuando}.").replace("  ", " "));
    }
    let mut s = format!("Tareas {que}{cuando} ({cuantas}):\n").replace("  ", " ");
    s.push_str(&lineas.join("\n"));
    if cuantas > puestas {
        s.push_str(&format!("\n…y {} más.", cuantas - puestas));
    }
    Ok(s)
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;
    use crate::acciones::{por_nombre, validar};

    pub fn ahora() -> Momento {
        // Miercoles 14-oct-2026, 10:30.
        Momento {
            dia: fechas::dias_de(2026, 10, 14),
            minuto: 630,
        }
    }

    fn t(indice: usize, texto: &str, hecha: bool, vence: Option<(i64, u32)>) -> VistaTarea {
        VistaTarea {
            indice,
            texto: texto.into(),
            hecha,
            creada: None,
            vence: vence.map(|(d, m)| Momento { dia: ahora().dia + d, minuto: m }),
        }
    }

    /// Inbox (guardados), Obra (chat Casa Lima), Compras (chat Obra Norte).
    pub fn fuente() -> Fijas {
        let lista = |proyecto: &str, chat: &str, g: bool, codigo: &str, titulo: &str, tareas| VistaLista {
            proyecto: proyecto.into(),
            chat: chat.into(),
            guardados: g,
            codigo: codigo.into(),
            titulo: titulo.into(),
            cuando: 0,
            tareas,
        };
        Fijas {
            listas: vec![
                lista("g", "Mensajes guardados", true, "inbox", "Inbox", vec![
                    t(0, "comprar pan", false, None),
                    t(1, "llamar al banco", false, Some((-1, 600))),
                    t(2, "pagar la luz", true, None),
                ]),
                lista("casa", "Casa Lima", false, "obra", "Obra", vec![
                    t(0, "revisar encofrado", false, Some((1, 540))),
                    t(1, "pedir cemento", true, None),
                ]),
                lista("norte", "Obra Norte", false, "compras", "Compras", vec![t(0, "comprar clavos", false, Some((20, 540)))]),
            ],
            chats: vec![
                Chat { id: "g".into(), nombre: "Mensajes guardados".into(), guardados: true },
                Chat { id: "casa".into(), nombre: "Casa Lima".into(), guardados: false },
                Chat { id: "norte".into(), nombre: "Obra Norte".into(), guardados: false },
            ],
            ahora: Some(ahora()),
        }
    }

    fn accion(nombre: &str, args: &str) -> AccionApp {
        validar(por_nombre(nombre).unwrap(), &serde_json::from_str(args).unwrap()).unwrap()
    }

    fn pasos(nombre: &str, args: &str) -> Result<Pasos, String> {
        resolver(&accion(nombre, args), &fuente())
    }

    #[test]
    fn se_parecen_como_se_espera() {
        assert_eq!(parecido("comprar pan", "Comprar pan"), 100);
        assert_eq!(parecido("pan", "comprar pan"), 85);
        assert_eq!(parecido("la tarea de revisar el encofrado ya", "revisar encofrado"), 70);
        assert!(parecido("llama al banco", "llamar al banco") > 0);
        assert_eq!(parecido("pintar la luna", "comprar pan"), 0);
    }

    #[test]
    fn anadir_con_lista_fecha_y_chat() {
        let p = pasos("anadir_tarea", r#"{"texto":"revisar planos","fecha":"mañana a las 10"}"#).unwrap();
        assert_eq!(p.pedidos[0]["texto"], "revisar planos ⏰ 2026-10-15 10:00");
        assert!(p.pedidos[0].get("codigo").is_none(), "sin lista: al Inbox");
        assert_eq!(p.hecho, "Apuntada «revisar planos» para mañana 10:00.");
        let p = pasos("anadir_tarea", r#"{"texto":"tornillos","lista":"compras"}"#).unwrap();
        assert_eq!((&p.pedidos[0]["codigo"], &p.pedidos[0]["proyecto"]), (&json!("compras"), &json!("norte")));
        assert_eq!(p.hecho, "Apuntada «tornillos» en «Compras (Obra Norte)».");
        let p = pasos("lista_nueva", r#"{"titulo":"Acabados","chat":"casa lima"}"#).unwrap();
        assert_eq!(p.pedidos[0]["proyecto"], "casa");
    }

    #[test]
    fn marcar_mover_editar_y_quitar_por_su_nombre() {
        let p = pasos("marcar_tarea", r#"{"tarea":"revisar el encofrado"}"#).unwrap();
        assert_eq!(p.pedidos[0], json!({"pixpin":1,"accion":"marcar_tarea","hecha":true,"codigo":"obra","proyecto":"casa","indice":0}));
        assert_eq!(p.hecho, "«revisar encofrado» marcada como hecha.");
        let p = pasos("marcar_tarea", r#"{"tarea":"pedir cemento","hecha":false}"#).unwrap();
        assert_eq!(p.pedidos[0]["indice"], 1);
        assert!(p.hecho.ends_with("pendiente."));
        let p = pasos("mover_tarea", r#"{"tarea":"comprar pan","a_lista":"Compras"}"#).unwrap();
        assert_eq!(p.pedidos[0]["a_codigo"], "compras");
        assert_eq!(p.pedidos[0]["a_proyecto"], "norte");
        let p = pasos("editar_tarea", r#"{"tarea":"llamar al banco","texto":"llamar al banco y al seguro"}"#).unwrap();
        assert_eq!(p.pedidos[0]["accion"], "corregir_tarea");
        assert_eq!(p.pedidos[0]["texto"], "llamar al banco y al seguro ⏰ 2026-10-13 10:00", "conserva su fecha");
        let p = pasos("editar_tarea", r#"{"tarea":"comprar pan","fecha":"el viernes"}"#).unwrap();
        assert_eq!(p.pedidos[0]["texto"], "comprar pan ⏰ 2026-10-16 09:00");
        let p = pasos("quitar_tarea", r#"{"tarea":"pan"}"#).unwrap();
        assert_eq!(p.confirmar.as_deref(), Some("¿Quito la tarea «comprar pan» de «Inbox»?"));
        let p = pasos("borrar_lista", r#"{"lista":"obra"}"#).unwrap();
        assert!(p.confirmar.unwrap().contains("«Obra (Casa Lima)»"));
    }

    #[test]
    fn varias_tareas_de_un_acta_en_una_lista_nueva() {
        let p = pasos(
            "anadir_tareas",
            r#"{"tareas":["Enviar presupuesto el viernes","Revisar planos"," "],"lista_nueva":"Acta 14-oct","chat":"Casa Lima"}"#,
        )
        .unwrap();
        assert_eq!(p.pedidos.len(), 3);
        assert_eq!(p.pedidos[0], json!({"pixpin":1,"accion":"lista_nueva","titulo":"Acta 14-oct","proyecto":"casa"}));
        assert_eq!(p.pedidos[1]["texto"], "Enviar presupuesto ⏰ 2026-10-16 09:00");
        assert_eq!(p.pedidos[2]["__en_lista_nueva"], "Acta 14-oct");
        assert_eq!(p.hecho, "Apuntadas 2 tareas en «Acta 14-oct».");
        // Al hacerla, cada tarea va a la lista recien creada.
        let mut f = fuente();
        let mut mandados = Vec::new();
        let a = accion("anadir_tareas", r#"{"tareas":["a","b"],"lista_nueva":"Acta"}"#);
        let mut primero = true;
        let r = hacer(&a, &f.clone(), &|_| true, &mut |p| {
            mandados.push(p.clone());
            if primero {
                primero = false;
            }
            Ok(())
        });
        // La fuente fija no crea la lista: se dice, y no se manda lo demas.
        assert!(r.unwrap_err().contains("lista nueva «Acta»"));
        assert_eq!(mandados.len(), 1);
        f.listas.push(VistaLista {
            proyecto: "g".into(),
            chat: "Mensajes guardados".into(),
            guardados: true,
            codigo: "acta".into(),
            titulo: "Acta".into(),
            cuando: 5,
            tareas: vec![],
        });
        mandados.clear();
        hacer(&a, &f, &|_| true, &mut |p| {
            mandados.push(p.clone());
            Ok(())
        })
        .unwrap();
        assert_eq!(mandados[1]["codigo"], "acta");
        assert!(mandados[1].get("__en_lista_nueva").is_none());
    }

    #[test]
    fn lo_que_no_casa_vuelve_con_el_motivo() {
        let e = pasos("marcar_tarea", r#"{"tarea":"pintar la luna"}"#).unwrap_err();
        assert!(e.contains("No encuentro") && e.contains("comprar pan"), "{e}");
        let e = pasos("anadir_tarea", r#"{"texto":"pagar alquiler","fecha":"el 31 de febrero"}"#).unwrap_err();
        assert!(e.contains("no existe"), "{e}");
        let e = pasos("anadir_tarea", r#"{"texto":"x","lista":"Compras","chat":"Casa Lima"}"#).unwrap_err();
        assert!(e.contains("esta en el chat «Obra Norte»"), "{e}");
        let e = pasos("anadir_tarea", r#"{"texto":"x","lista":"Ferreteria"}"#).unwrap_err();
        assert!(e.contains("No hay ninguna lista") && e.contains("Compras"), "{e}");
        let e = pasos("mover_tarea", r#"{"tarea":"comprar clavos","a_lista":"compras"}"#).unwrap_err();
        assert!(e.contains("ya esta"), "{e}");
        // «comprar» se parece igual a dos tareas: se pregunta cual.
        let e = pasos("quitar_tarea", r#"{"tarea":"comprar"}"#).unwrap_err();
        assert!(e.contains("varias"), "{e}");
        let e = pasos("editar_tarea", r#"{"tarea":"comprar pan"}"#).unwrap_err();
        assert!(e.contains("texto nuevo"), "{e}");
    }

    /// Lo que el 2B pone mal (medido, 14-oct) se entiende si no deja dudas.
    #[test]
    fn un_chat_inventado_o_una_lista_que_es_la_tarea_se_entienden() {
        let p = pasos("borrar_lista", r#"{"lista":"Compras","chat":"PixPin"}"#).unwrap();
        assert!(p.confirmar.unwrap().contains("«Compras (Obra Norte)»"));
        let p = pasos("marcar_tarea", r#"{"tarea":"táchalo de mis pendientes","lista":"encofrado"}"#).unwrap();
        assert_eq!((&p.pedidos[0]["codigo"], &p.pedidos[0]["indice"]), (&json!("obra"), &json!(0)));
        // Pero un chat que existe si manda, y una lista que no esta sin tarea es error.
        assert!(pasos("anadir_tarea", r#"{"texto":"x","lista":"Compras","chat":"Casa Lima"}"#).is_err());
        assert!(pasos("borrar_lista", r#"{"lista":"encofrado"}"#).is_err());
    }

    #[test]
    fn lo_que_borra_no_sale_sin_el_si() {
        let a = accion("quitar_tarea", r#"{"tarea":"comprar pan"}"#);
        let mut n = 0;
        let r = hacer(&a, &fuente(), &|_| false, &mut |_| {
            n += 1;
            Ok(())
        });
        assert!(r.is_err() && n == 0);
    }

    #[test]
    fn la_consulta_es_una_lista_clara() {
        let f = fuente();
        let c = |args: &str| consultar(&accion("consultar_tareas", args).por_resolver, &f).unwrap();
        let s = c("{}");
        assert!(s.starts_with("Tareas pendientes (4):"), "{s}");
        assert!(s.contains("Inbox:\n  ☐ llamar al banco — venció ayer 10:00\n  ☐ comprar pan"), "{s}");
        assert!(s.contains("Obra (Casa Lima):\n  ☐ revisar encofrado — mañana 09:00"), "{s}");
        let s = c(r#"{"estado":"vencidas","periodo":"semana"}"#);
        assert_eq!(s, "Tareas vencidas de esta semana (1):\nInbox:\n  ☐ llamar al banco — venció ayer 10:00");
        let s = c(r#"{"estado":"hechas","chat":"Casa Lima"}"#);
        assert!(s.contains("pedir cemento") && !s.contains("pagar la luz"), "{s}");
        assert_eq!(c(r#"{"periodo":"hoy"}"#), "No tienes tareas pendientes para hoy.");
        // «buscar» repitiendo el chat no deja la consulta vacia.
        assert!(c(r#"{"chat":"Casa Lima","buscar":"Casa Lima"}"#).contains("revisar encofrado"));
        assert!(c(r#"{"periodo":"mes"}"#).contains("revisar encofrado"));
        assert!(!c(r#"{"periodo":"mes"}"#).contains("clavos"), "el 3-nov no es de este mes");
        assert!(consultar(&accion("consultar_tareas", r#"{"lista":"cocina"}"#).por_resolver, &f).is_err());
    }
}
