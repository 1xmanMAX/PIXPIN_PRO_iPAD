//! **Buscar en todo**: los chats por su nombre, los mensajes (notas, nombres
//! de ficheros, transcripciones), las tareas y los momentos del timeline.
//!
//! La ventana «Buscar en PixPin» (D8) usa [`Nucleo::buscar_en_todo`]
//! (`buscar/todo.rs`), que ademas busca dentro de los documentos
//! (`buscar/textos.rs`), en el texto leido de las capturas, en las
//! lecciones, ordena por lo que vale cada coincidencia como el plugin de
//! Windows (`buscar/puntos.rs`) y recuerda lo reciente
//! (`buscar/recientes.rs`). [`Nucleo::buscar`] sigue para Atajos y Siri.
//!
//! El buscador de Windows (`buscar_todo.rs`) usa el del plugin de Flow
//! Launcher (`pixpin-lanzador`), que aun no compila fuera de Windows (esta
//! en `por-adaptar/`). Hasta que pase a `nucleo/`, aqui hay un recorrido
//! sencillo con las mismas reglas que el resto de buscadores de PixPin
//! (`pixpin_timeline::buscar`): sin mayusculas ni tildes, y **cada palabra**
//! de la consulta tiene que estar, en cualquier orden.
//!
//! # Rendimiento
//!
//! - Se recorre chat a chat (el mas reciente primero) y se para en cuanto
//!   hay lo pedido: la segunda pagina vuelve a recorrer, pero solo hasta
//!   donde llega ella, y nunca mas alla de [`TOPE_DE_RESULTADOS`].
//! - Se puede **cancelar** desde otro hilo ([`Busqueda::cancelar`]): Swift
//!   cancela la consulta anterior al escribir otra letra y Rust lo mira en
//!   cada linea, asi que no sigue leyendo cuadernos que nadie espera.
//! - Cada resultado de un mensaje lleva su **posicion** en el chat (la de
//!   [`Nucleo::mensajes`]) para abrir la pagina donde esta.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Mensaje};
use pixpin_proyecto::mini;
use pixpin_timeline::buscar::normalizar;

use crate::chats::{MensajeChat, validar_chat};
use crate::tareas::{es_lista, nombre_de_lista};
use crate::{ErrorNucleo, Nucleo};

pub mod puntos;
pub mod recientes;
pub mod textos;
pub mod todo;

pub use todo::{DocumentoSinTexto, OpcionesBusqueda, RecientesBusqueda, ResultadosEnTodo};

/// Lo mas lejos que llega una busqueda (`desde + cuantos`): pasado esto,
/// hay que afinar la consulta.
pub const TOPE_DE_RESULTADOS: u32 = 1000;

/// Lo largo del trozo de texto que se ensena de cada resultado, en letras.
const LARGO_DEL_TROZO: usize = 140;

/// Una busqueda en curso, para poder cancelarla desde otro hilo.
#[derive(Debug, Default, uniffi::Object)]
pub struct Busqueda {
    cancelada: AtomicBool,
}

#[uniffi::export]
impl Busqueda {
    #[uniffi::constructor]
    pub fn nueva() -> Arc<Busqueda> {
        Arc::new(Busqueda::default())
    }

    /// Que pare en cuanto pueda (devuelve lo que llevara, marcado).
    pub fn cancelar(&self) {
        self.cancelada.store(true, Ordering::Relaxed);
    }

    pub fn cancelada(&self) -> bool {
        self.cancelada.load(Ordering::Relaxed)
    }
}

/// Que se encontro.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, uniffi::Enum,
)]
pub enum ClaseDeResultado {
    /// Un chat por su nombre.
    Chat,
    /// Un mensaje de un chat (una nota, un texto, una mini-app).
    Mensaje,
    /// Una tarea de una lista.
    Tarea,
    /// Un momento del timeline.
    Momento,
    /// Un fichero de un chat (documento, foto, audio, lienzo u hoja), por
    /// su nombre o por lo que pone dentro.
    Fichero,
    /// Una captura de la galeria, por su nombre, su fecha o su texto.
    Captura,
    /// Una leccion aprendida.
    Leccion,
}

/// Un resultado.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(default)]
pub struct ResultadoBusqueda {
    pub clase: ClaseDeResultado,
    /// El chat (no en los momentos).
    pub chat: Option<String>,
    pub nombre_chat: String,
    /// El id del mensaje (en mensajes y tareas).
    pub mensaje: Option<String>,
    /// La posicion del mensaje en su chat (0 el mas antiguo), la de
    /// [`Nucleo::mensajes`].
    pub posicion: Option<u64>,
    /// El numero de la tarea en su lista.
    pub tarea: Option<u32>,
    pub hecha: bool,
    /// El id del momento.
    pub momento: Option<String>,
    /// La clase del mensaje (`NOTA`, `IMAGEN`...) o vacio.
    pub tipo: String,
    /// La linea principal: el nombre del chat, el del fichero, el titulo
    /// de la lista o del momento.
    pub titulo: String,
    /// Un trozo del texto donde esta lo buscado.
    pub fragmento: String,
    /// Milisegundos desde 1970.
    pub cuando: i64,
    /// Lo que lo identifica entre todos (`captura/<nombre>`,
    /// `mensaje/<chat>/<id>`...), como la `recordKey` de Windows.
    pub clave: String,
    /// La ruta absoluta del fichero en este aparato (un fichero, una
    /// captura, la primera foto de un momento), si se resuelve.
    pub ruta: Option<String>,
    /// El id de la leccion.
    pub leccion: Option<String>,
    /// Cuanto vale la coincidencia (`puntos::puntuar`); 0 sin consulta.
    pub puntos: u32,
    /// Las letras de `titulo` que coinciden, en UTF-16 (`NSString`).
    pub resaltado: Vec<u32>,
    /// Se encontro dentro (el texto del documento o el leido de la
    /// captura), no en el nombre.
    pub dentro: bool,
    /// El tamano del fichero o la captura.
    pub bytes: u64,
    /// Una captura conservada (no caduca).
    pub conservada: bool,
    /// Los dias que le quedan a una captura (0: hoy); `None` si no se va.
    pub dias_que_quedan: Option<i64>,
    /// El texto de la tarea tal cual esta en su lista (lo que pide
    /// [`Nucleo::marcar_tarea`]); vacio en lo demas.
    pub crudo: String,
}

impl Default for ResultadoBusqueda {
    fn default() -> Self {
        ResultadoBusqueda::de(ClaseDeResultado::Mensaje, "")
    }
}

impl ResultadoBusqueda {
    /// Uno vacio de esa clase con ese titulo (lo demas se rellena luego).
    pub fn de(clase: ClaseDeResultado, titulo: &str) -> ResultadoBusqueda {
        ResultadoBusqueda {
            clase,
            chat: None,
            nombre_chat: String::new(),
            mensaje: None,
            posicion: None,
            tarea: None,
            hecha: false,
            momento: None,
            tipo: String::new(),
            titulo: titulo.to_string(),
            fragmento: String::new(),
            cuando: 0,
            clave: String::new(),
            ruta: None,
            leccion: None,
            puntos: 0,
            resaltado: Vec::new(),
            dentro: false,
            bytes: 0,
            conservada: false,
            dias_que_quedan: None,
            crudo: String::new(),
        }
    }
}

/// Una pagina de resultados.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaBusqueda {
    pub resultados: Vec<ResultadoBusqueda>,
    pub desde: u32,
    /// Hay mas detras de esta pagina.
    pub hay_mas: bool,
    /// Se cancelo a mitad: lo que hay es lo que dio tiempo.
    pub cancelada: bool,
    /// Se llego a [`TOPE_DE_RESULTADOS`]: habria mas.
    pub recortada: bool,
}

/// Las palabras de la consulta, ya normalizadas.
fn palabras(consulta: &str) -> Vec<String> {
    normalizar(consulta)
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

fn encaja(palabras: &[String], texto: &str) -> bool {
    let t = normalizar(texto);
    palabras.iter().all(|p| t.contains(p.as_str()))
}

/// La linea de `texto` donde esta la primera palabra, recortada a
/// [`LARGO_DEL_TROZO`] letras alrededor (con «…» si se corto).
fn trozo(texto: &str, palabras: &[String]) -> String {
    let primera = palabras.first().map(String::as_str).unwrap_or("");
    let linea = texto
        .lines()
        .find(|l| normalizar(l).contains(primera))
        .unwrap_or_else(|| texto.lines().next().unwrap_or(""))
        .trim();
    let letras: Vec<char> = linea.chars().collect();
    if letras.len() <= LARGO_DEL_TROZO {
        return linea.to_string();
    }
    // Donde cae la palabra (aproximado: normalizar casi siempre deja una
    // letra por letra).
    let en = normalizar(linea)
        .find(primera)
        .map(|b| normalizar(linea)[..b].chars().count())
        .unwrap_or(0);
    let inicio = en
        .saturating_sub(LARGO_DEL_TROZO / 3)
        .min(letras.len() - LARGO_DEL_TROZO);
    let fin = inicio + LARGO_DEL_TROZO;
    let mut s = String::new();
    if inicio > 0 {
        s.push('…');
    }
    s.extend(&letras[inicio..fin]);
    if fin < letras.len() {
        s.push('…');
    }
    s
}

/// Lo minimo de una linea para contar posiciones como el indice de
/// [`Nucleo::mensajes`] (si el mensaje entero no se entiende).
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct Cabeza {
    id: String,
}

/// Lo que va juntando la busqueda, y cuando parar.
struct Recogida<'a> {
    hace_falta: usize,
    busqueda: Option<&'a Busqueda>,
    hallados: Vec<ResultadoBusqueda>,
}

impl Recogida<'_> {
    fn llena(&self) -> bool {
        self.hallados.len() >= self.hace_falta
    }

    fn cancelada(&self) -> bool {
        self.busqueda.is_some_and(Busqueda::cancelada)
    }
}

/// Los resultados de un cuaderno, del mensaje mas nuevo al mas viejo.
fn buscar_en_cuaderno(
    texto: &str,
    ficha: &almacen::Ficha,
    p: &[String],
    r: &Recogida<'_>,
) -> (Vec<ResultadoBusqueda>, bool) {
    let mut v = Vec::new();
    let mut posicion: u64 = 0;
    for linea in texto.lines() {
        if r.cancelada() {
            return (v, true);
        }
        if linea.trim().is_empty() {
            continue;
        }
        let Ok(m) = serde_json::from_str::<Mensaje>(linea) else {
            // Que no se entienda entero no la saca de la cuenta si el
            // indice de paginas la cuenta.
            if serde_json::from_str::<Cabeza>(linea)
                .is_ok_and(|c| !c.id.starts_with(cuaderno::PREFIJO_DE_SISTEMA))
            {
                posicion += 1;
            }
            continue;
        };
        if m.id.starts_with(cuaderno::PREFIJO_DE_SISTEMA) {
            continue;
        }
        let aqui = posicion;
        posicion += 1;
        if es_lista(&m) {
            let titulo = nombre_de_lista(&m);
            for (i, t) in mini::leer_tareas(&m.texto).into_iter().enumerate() {
                let (visible, _) = mini::partir(&t.texto);
                let (limpio, _) = mini::imagenes_de(visible);
                if encaja(p, &limpio) {
                    v.push(ResultadoBusqueda {
                        clase: ClaseDeResultado::Tarea,
                        chat: Some(ficha.id.clone()),
                        nombre_chat: ficha.nombre.clone(),
                        mensaje: Some(m.id.clone()),
                        posicion: Some(aqui),
                        tarea: Some(i as u32),
                        hecha: t.hecha,
                        momento: None,
                        tipo: "MINIAPP".into(),
                        titulo: titulo.clone(),
                        fragmento: limpio,
                        cuando: m.cuando,
                        clave: format!("tarea/{}/{}/{i}", ficha.id, m.id),
                        crudo: t.texto.clone(),
                        ..ResultadoBusqueda::default()
                    });
                }
            }
            continue;
        }
        let donde = format!(
            "{}\n{}\n{}",
            m.nombre,
            m.texto,
            m.transcripcion.as_deref().unwrap_or("")
        );
        if !encaja(p, &donde) {
            continue;
        }
        let fragmento = trozo(&donde, p);
        let m = MensajeChat::from(m);
        let clave = format!("mensaje/{}/{}", ficha.id, m.id);
        v.push(ResultadoBusqueda {
            clase: ClaseDeResultado::Mensaje,
            chat: Some(ficha.id.clone()),
            nombre_chat: ficha.nombre.clone(),
            mensaje: Some(m.id),
            posicion: Some(aqui),
            tarea: None,
            hecha: false,
            momento: None,
            titulo: if m.nombre.is_empty() {
                m.resumen
            } else {
                m.nombre
            },
            clave,
            tipo: m.clase,
            fragmento,
            cuando: m.cuando,
            ..ResultadoBusqueda::default()
        });
    }
    // Lo mas nuevo primero (las tareas de una lista, en su orden).
    v.sort_by(|a, b| b.posicion.cmp(&a.posicion).then(a.tarea.cmp(&b.tarea)));
    (v, false)
}

#[uniffi::export]
impl Nucleo {
    /// **Busca en todo**: primero los chats por su nombre, luego los
    /// mensajes y tareas de cada chat (el chat mas reciente primero y,
    /// dentro, el mensaje mas nuevo primero) y al final los momentos del
    /// timeline. Por paginas: `desde` y `cuantos`, hasta
    /// [`TOPE_DE_RESULTADOS`]. Con `busqueda`, se puede cancelar desde otro
    /// hilo. Una consulta sin palabras da una pagina vacia.
    pub fn buscar(
        &self,
        consulta: String,
        desde: u32,
        cuantos: u32,
        busqueda: Option<Arc<Busqueda>>,
    ) -> Result<PaginaBusqueda, ErrorNucleo> {
        let p = palabras(&consulta);
        let desde = desde.min(TOPE_DE_RESULTADOS);
        let cuantos = cuantos.min(TOPE_DE_RESULTADOS - desde);
        let vacia = PaginaBusqueda {
            resultados: Vec::new(),
            desde,
            hay_mas: false,
            cancelada: false,
            recortada: false,
        };
        if p.is_empty() || cuantos == 0 {
            return Ok(vacia);
        }
        // Uno de mas para saber si hay otra pagina.
        let mut r = Recogida {
            hace_falta: (desde + cuantos) as usize + 1,
            busqueda: busqueda.as_deref(),
            hallados: Vec::new(),
        };
        let indice = Indice::leer_si_esta(self.ruta())?;
        let fichas: Vec<&almacen::Ficha> = indice
            .ordenadas()
            .into_iter()
            .filter(|f| validar_chat(&f.id).is_ok())
            .collect();
        let mut cancelada = false;

        // 1. Los chats por su nombre.
        for f in &fichas {
            if encaja(&p, &f.nombre) {
                r.hallados.push(ResultadoBusqueda {
                    clase: ClaseDeResultado::Chat,
                    chat: Some(f.id.clone()),
                    nombre_chat: f.nombre.clone(),
                    mensaje: None,
                    posicion: None,
                    tarea: None,
                    hecha: false,
                    momento: None,
                    tipo: String::new(),
                    titulo: f.nombre.clone(),
                    fragmento: f.resumen.clone(),
                    cuando: f.tocado,
                    clave: format!("chat/{}", f.id),
                    ..ResultadoBusqueda::default()
                });
            }
        }

        // 2. Los mensajes y las tareas, chat a chat.
        for f in &fichas {
            if r.llena() {
                break;
            }
            if r.cancelada() {
                cancelada = true;
                break;
            }
            let fichero = almacen::carpeta(self.ruta(), &f.id).join("guardados.jsonl");
            let Ok(texto) = std::fs::read_to_string(&fichero) else {
                continue;
            };
            let (v, parada) = buscar_en_cuaderno(&texto, f, &p, &r);
            r.hallados.extend(v);
            if parada {
                cancelada = true;
                break;
            }
        }

        // 3. Los momentos del timeline.
        if !r.llena() && !cancelada {
            let momentos = self.momentos();
            for m in pixpin_timeline::buscar::buscar(&momentos, &consulta) {
                if r.llena() {
                    break;
                }
                r.hallados.push(ResultadoBusqueda {
                    clase: ClaseDeResultado::Momento,
                    chat: None,
                    nombre_chat: String::new(),
                    mensaje: None,
                    posicion: None,
                    tarea: None,
                    hecha: false,
                    momento: Some(m.id.clone()),
                    tipo: String::new(),
                    titulo: m.titulo.clone(),
                    fragmento: trozo(&m.descripcion, &p),
                    cuando: m.cuando,
                    clave: format!("momento/{}", m.id),
                    ..ResultadoBusqueda::default()
                });
            }
        }

        let hay = r.hallados.len();
        let recortada =
            !cancelada && hay > (desde + cuantos) as usize && desde + cuantos >= TOPE_DE_RESULTADOS;
        let hay_mas = !cancelada && hay > (desde + cuantos) as usize && !recortada;
        let resultados: Vec<ResultadoBusqueda> = r
            .hallados
            .into_iter()
            .skip(desde as usize)
            .take(cuantos as usize)
            .collect();
        Ok(PaginaBusqueda {
            resultados,
            desde,
            hay_mas,
            cancelada,
            recortada,
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_palabra_sin_tildes_ni_mayusculas() {
        let p = palabras("Reunión  JUAN");
        assert!(encaja(&p, "la reunion con juan"));
        assert!(encaja(&p, "Juan: REUNIÓN"));
        // Caso negativo: falta una palabra.
        assert!(!encaja(&p, "reunion con pedro"));
        assert!(palabras("   ").is_empty());
    }

    #[test]
    fn el_trozo_es_la_linea_donde_esta_y_se_recorta() {
        let p = palabras("yeso");
        assert_eq!(trozo("comprar arena\ncomprar yeso", &p), "comprar yeso");
        let largo = format!("{} yeso {}", "a".repeat(200), "b".repeat(200));
        let t = trozo(&largo, &p);
        assert!(t.contains("yeso"), "{t}");
        assert!(t.starts_with('…') && t.ends_with('…'));
        assert!(t.chars().count() <= LARGO_DEL_TROZO + 2);
    }

    #[test]
    fn una_busqueda_cancelada_lo_dice() {
        let b = Busqueda::nueva();
        assert!(!b.cancelada());
        b.cancelar();
        assert!(b.cancelada());
    }
}
