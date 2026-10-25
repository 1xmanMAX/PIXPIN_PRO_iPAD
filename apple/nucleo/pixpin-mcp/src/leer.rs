//! **Lo que se lee de la carpeta de datos**, sin la app y sin escribir nada:
//! las tareas, los chats y el indice de documentos del asistente.
//!
//! - **Tareas**: las listas son mensajes `MINIAPP` «tareas» de cada chat
//!   (`pixpin_proyecto::mini`), y la consulta es la del asistente
//!   (`pixpin_ia::tareas::consultar`, la misma frase de respuesta). Lo unico
//!   propio es [`FuenteDelDisco`], el adaptador que en la app es
//!   `asistente::acciones::FuenteDeLaApp` (que no se puede usar desde aqui:
//!   vive en el ejecutable).
//! - **Buscar**: el indice BM25 que la app tiene al dia en
//!   `<raiz>\cache\documentos` (`pixpin_ia::documentos::Indice::cargar`, solo
//!   lee). Si aun no existe (el asistente no se ha usado nunca), se busca en
//!   los chats con un indice en memoria, que no se guarda.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use pixpin_ia::documentos::{Abrir, Alcance, Cita, Fuente, Firma, Hallazgo, Indice, Lector, Tipo, Unidad};
use pixpin_ia::fechas::Momento;
use pixpin_ia::tareas::{Chat, FuenteTareas, VistaLista, VistaTarea};
use pixpin_proyecto::cuaderno::{Clase, Cuaderno};
use pixpin_proyecto::{almacen, mini};

use crate::herramientas::{Contenido, Resultado};

/// Otra raiz de datos (pruebas). Sin ella, la de PixPin: la carpeta de este
/// exe si a su lado hay un `pixpinmax.toml` (portable), si no
/// `%APPDATA%\PixPinMax`, igual que decide la app.
pub const VAR_RAIZ: &str = "PIXPIN_RAIZ";

/// **Donde estan los datos.**
pub fn raiz() -> PathBuf {
    if let Some(r) = std::env::var_os(VAR_RAIZ).filter(|r| !r.is_empty()) {
        return PathBuf::from(r);
    }
    // La de la app del Mac (`carpetaDeDatos`): Application Support/PixPinMax.
    let casa = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    casa.join("Library").join("Application Support").join("PixPinMax")
}

// ---------------------------------------------------------------------------
// Tareas

/// Las listas de tareas de todos los chats, como las ve el asistente.
pub struct FuenteDelDisco {
    pub raiz: PathBuf,
}

/// Una tarea como la ve el asistente: sin fecha de creacion, sin imagenes y
/// sin su marca ⏰ (que va aparte, en `vence`).
fn vista_de_tarea(indice: usize, t: &mini::Tarea) -> VistaTarea {
    let (visible, creada) = mini::partir(&t.texto);
    let (texto, _) = mini::imagenes_de(visible);
    let vence = Momento::de_marca(&texto);
    let texto = match (vence, texto.rfind('⏰')) {
        (Some(_), Some(i)) => texto[..i].trim_end().to_string(),
        _ => texto,
    };
    VistaTarea {
        indice,
        texto: texto.trim().to_string(),
        hecha: t.hecha,
        creada: creada.map(|d| d.dias()),
        vence,
    }
}

impl FuenteTareas for FuenteDelDisco {
    fn listas(&self) -> Vec<VistaLista> {
        let mut v = Vec::new();
        for ficha in almacen::Indice::leer(&self.raiz).proyectos {
            let Ok(c) = Cuaderno::leer_de(&almacen::carpeta(&self.raiz, &ficha.id)) else { continue };
            for m in &c.mensajes {
                if m.clase != Some(Clase::MiniApp) || m.miniapp.as_deref() != Some(mini::TAREAS) {
                    continue;
                }
                let titulo = match mini::titulo(&m.texto) {
                    t if t.is_empty() => m.nombre.clone(),
                    t => t,
                };
                v.push(VistaLista {
                    proyecto: ficha.id.clone(),
                    chat: ficha.nombre.clone(),
                    guardados: ficha.es_guardados(),
                    codigo: m.id.clone(),
                    titulo,
                    cuando: m.cuando,
                    tareas: mini::leer_tareas(&m.texto).iter().enumerate().map(|(i, t)| vista_de_tarea(i, t)).collect(),
                });
            }
        }
        // El Inbox primero, luego lo mas nuevo (como la ventana de tareas).
        v.sort_by(|a, b| b.es_inbox().cmp(&a.es_inbox()).then(b.cuando.cmp(&a.cuando)));
        v
    }

    fn chats(&self) -> Vec<Chat> {
        almacen::Indice::leer(&self.raiz)
            .proyectos
            .iter()
            .map(|f| Chat {
                id: f.id.clone(),
                nombre: f.nombre.clone(),
                guardados: f.es_guardados(),
            })
            .collect()
    }

    fn ahora(&self) -> Momento {
        Momento::de_ms_locales(crate::hora::ahora_local_ms())
    }
}

/// **`consultar_tareas`**: la fila de la tabla, validada con la tabla y
/// contestada con la consulta del asistente.
pub fn consultar_tareas(args: &Map<String, Value>) -> Resultado {
    let fila = pixpin_ia::acciones::por_nombre("consultar_tareas").expect("esta en la tabla");
    let a = pixpin_ia::acciones::validar(fila, args).map_err(|e| e.to_string())?;
    let texto = pixpin_ia::tareas::consultar(&a.por_resolver, &FuenteDelDisco { raiz: raiz() })?;
    Ok(vec![Contenido::Texto(texto)])
}

/// **`listar_chats`**.
pub fn listar_chats(_: &Map<String, Value>) -> Resultado {
    let raiz = raiz();
    let indice = almacen::Indice::leer(&raiz);
    if indice.proyectos.is_empty() {
        return Ok(vec![Contenido::Texto(format!("No hay chats en {}.", raiz.display()))]);
    }
    let listas = FuenteDelDisco { raiz: raiz.clone() }.listas();
    let mut s = String::new();
    for f in indice.ordenadas() {
        let nombre = if f.es_guardados() { "Mensajes guardados".to_string() } else { f.nombre.clone() };
        s.push_str(&format!("- {nombre} (id {})", f.id));
        if let Some(fecha) = pixpin_ia::documentos::herramienta::fecha_corta(f.tocado) {
            s.push_str(&format!(", tocado el {fecha}"));
        }
        let suyas: Vec<String> = listas
            .iter()
            .filter(|l| l.proyecto == f.id)
            .map(|l| format!("«{}» ({} pendientes)", l.titulo, l.tareas.iter().filter(|t| !t.hecha).count()))
            .collect();
        if !suyas.is_empty() {
            s.push_str(&format!("; listas: {}", suyas.join(", ")));
        }
        s.push('\n');
    }
    Ok(vec![Contenido::Texto(s.trim_end().to_string())])
}

// ---------------------------------------------------------------------------
// Buscar

/// Como se llama un chat al decirlo.
fn nombre_de(f: &almacen::Ficha) -> String {
    if f.es_guardados() { "Mensajes guardados".into() } else { f.nombre.clone() }
}

/// Lee los chats para el indice en memoria (sin el de la app).
struct LectorDeChats;

impl Lector for LectorDeChats {
    fn leer(&self, f: &Fuente) -> Result<Vec<Unidad>, String> {
        let texto = std::fs::read_to_string(&f.ruta).map_err(|e| e.to_string())?;
        let proyecto = match &f.abrir {
            Abrir::Mensaje { proyecto, .. } => proyecto.clone(),
            _ => None,
        };
        Ok(Cuaderno::leer(&texto)
            .mensajes
            .into_iter()
            .filter(|m| !m.en_buzon && !m.id.is_empty())
            .filter_map(|m| {
                let t = match m.clase {
                    Some(Clase::Voz) => m.transcripcion.clone().unwrap_or_default(),
                    Some(Clase::Archivo) | Some(Clase::Imagen) | Some(Clase::Dibujo) | Some(Clase::Pagina) => {
                        format!("{} {}", m.nombre, m.texto)
                    }
                    _ => m.texto.clone(),
                };
                (!t.trim().is_empty()).then(|| Unidad {
                    texto: t,
                    abrir: Some(Abrir::Mensaje {
                        proyecto: proyecto.clone(),
                        codigo: m.id.clone(),
                    }),
                    cuando: Some(m.cuando).filter(|c| *c > 0),
                    ..Default::default()
                })
            })
            .collect())
    }
}

/// Un indice de los chats, solo en memoria.
fn indice_de_chats(raiz: &Path) -> Indice {
    let fuentes: Vec<Fuente> = almacen::Indice::leer(raiz)
        .proyectos
        .iter()
        .filter(|f| !f.es_asistente())
        .filter_map(|f| {
            let ruta = almacen::carpeta(raiz, &f.id).join("guardados.jsonl");
            Some(Fuente {
                clave: format!("chat:{}", f.id),
                proyecto: f.id.clone(),
                proyecto_nombre: nombre_de(f),
                nombre: format!("Chat de {}", nombre_de(f)),
                tipo: Tipo::Chat,
                firma: Firma::de(&ruta)?,
                ruta,
                abrir: Abrir::Mensaje {
                    proyecto: (!f.es_guardados()).then(|| f.id.clone()),
                    codigo: String::new(),
                },
            })
        })
        .collect();
    let mut ix = Indice::nuevo(None);
    ix.poner_al_dia(&fuentes, &LectorDeChats);
    ix
}

/// Lo encontrado, para el modelo: numerado, con su origen, lo que dice y
/// como abrirlo con las herramientas de la app.
pub fn texto_de_hallazgos(consulta: &str, hallazgos: &[Hallazgo], aviso: Option<&str>) -> String {
    if hallazgos.is_empty() {
        let mut s = format!("No hay nada sobre «{consulta}» en los documentos y chats del usuario.");
        if let Some(a) = aviso {
            s.push(' ');
            s.push_str(a);
        }
        return s;
    }
    let mut s = String::new();
    for (i, h) in hallazgos.iter().enumerate() {
        let cita = Cita {
            numero: i + 1,
            hallazgo: h.clone(),
        };
        s.push_str(&format!("[{}] {} — chat «{}»", i + 1, cita.titulo(), h.proyecto_nombre));
        if let Some(f) = h.cuando.and_then(pixpin_ia::documentos::herramienta::fecha_corta) {
            s.push_str(&format!(", {f}"));
        }
        s.push_str(&format!("\n«{}»\n", h.fragmento.replace('\n', " ").trim()));
        match &h.abrir {
            Abrir::Fichero { ruta, pagina } => {
                s.push_str(&format!("abrir: abrir_fichero {}", json!({"ruta": ruta.display().to_string()})));
                if let Some(p) = pagina {
                    s.push_str(&format!(" (pág. {p})"));
                }
            }
            Abrir::Mensaje { codigo, .. } if !codigo.is_empty() => {
                s.push_str(&format!("abrir: abrir_mensaje {}", json!({"codigo": codigo})));
            }
            Abrir::Mensaje { .. } => s.push_str("abrir: abrir_chat"),
        }
        s.push_str("\n\n");
    }
    if let Some(a) = aviso {
        s.push_str(a);
    }
    s.trim_end().to_string()
}

/// Cuantos trozos se devuelven (el cliente tiene mas contexto que el 2B del
/// asistente, que se queda en 5).
pub const RESULTADOS: usize = 8;

/// **`buscar_en_mis_cosas`**, con la validacion del asistente.
pub fn buscar(args: &Map<String, Value>) -> Resultado {
    let p = pixpin_ia::documentos::herramienta::validar(args).map_err(|e| e.to_string())?;
    let raiz = raiz();
    let mut aviso = None;
    let alcance = match &p.proyecto {
        None => Alcance::Todo,
        Some(nombre) => {
            let buscado = pixpin_ia::documentos::texto::plano(nombre.trim());
            let ids: std::collections::HashSet<String> = almacen::Indice::leer(&raiz)
                .proyectos
                .iter()
                .filter(|f| {
                    let n = pixpin_ia::documentos::texto::plano(&nombre_de(f));
                    !buscado.is_empty() && (n == buscado || n.contains(&buscado))
                })
                .map(|f| f.id.clone())
                .collect();
            if ids.is_empty() {
                aviso = Some(format!("No hay ningun chat llamado «{nombre}»; se busco en todos."));
                Alcance::Todo
            } else {
                Alcance::Proyectos(ids)
            }
        }
    };
    let mut ix = Indice::cargar(&raiz.join("cache").join("documentos"));
    if ix.tamano().0 == 0 {
        ix = indice_de_chats(&raiz);
        aviso = Some(
            [aviso.unwrap_or_default(), "(Solo se busco en los chats: el indice de documentos de PixPin aun no esta hecho.)".into()]
                .join(" ")
                .trim()
                .to_string(),
        );
    }
    let hallazgos = ix.buscar(&p.consulta, &alcance, RESULTADOS);
    Ok(vec![Contenido::Texto(texto_de_hallazgos(&p.consulta, &hallazgos, aviso.as_deref()))])
}
