//! **«Buscar en PixPin»** (D8): todo lo de la ventana de buscar de Windows
//! (`buscar_todo.rs`, que pregunta al plugin de Flow) con lo que hay en el
//! Mac: los chats por su nombre; los mensajes y las notas; los ficheros por
//! su nombre y por lo que pone dentro (`textos.rs`); las tareas; las
//! lecciones; las capturas por su nombre, su fecha y su texto (el OCR de la
//! galeria); y los momentos del timeline.
//!
//! Cada resultado lleva lo que vale la coincidencia (`puntos.rs`, las reglas
//! del plugin: el principio del nombre gana a una palabra, que gana a un
//! trozo, que gana a lo de dentro) y las letras del titulo a resaltar. La
//! lista sale ordenada por puntos y, entre iguales, lo mas nuevo primero;
//! Swift la reparte en pestanas y grupos (`Buscar/EstadoBuscar.swift`).
//!
//! **Sin consulta** cada clase da lo ultimo: las tareas pendientes, las
//! lecciones mas tocadas, las ultimas capturas, los chats y ficheros
//! recientes, los ultimos momentos (lo que ensena cada pestana con la caja
//! vacia, como `Pestana::consulta_vacia` de Windows). Y «Abierto hace
//! poco» ([`Nucleo::buscar_inicio`]) es lo elegido aqui antes, la ultima
//! captura y los chats recientes.
//!
//! # Rendimiento
//!
//! Cada clase se para en `tope` (uno mas para saber si habia mas: entonces
//! sale en `recortadas` y Swift pide con un tope mayor al llegar al final).
//! Los chats se recorren del mas reciente al mas viejo y se dejan de leer
//! cuando las clases de los chats estan llenas. Se cancela desde otro hilo
//! con [`Busqueda`]. Los documentos no se leen al buscar: solo se mira el
//! texto ya guardado.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Mensaje};
use pixpin_proyecto::mini;

use super::puntos::{self, normalizar, puntuar};
use super::recientes::Recientes;
use super::textos;
use super::{
    Busqueda, Cabeza, ClaseDeResultado, ResultadoBusqueda, TOPE_DE_RESULTADOS, palabras, trozo,
};
use crate::chats::validar_chat;
use crate::galeria::CapturaGaleria;
use crate::lecciones::{FichaLeccion, FiltroLecciones};
use crate::tareas::{es_lista, nombre_de_lista};
use crate::{ErrorNucleo, Nucleo};

/// Puntos de mas para un chat que se llama asi (`EXTRA_PROYECTO` del
/// plugin): «Obra» tiene que dar antes el chat que una nota que lo nombra.
pub const EXTRA_CHAT: u32 = 20;

/// Cuantos «abiertos hace poco» se ensenan (`ABIERTOS_A_LA_VISTA`).
pub const ABIERTOS_A_LA_VISTA: usize = 5;

/// Lo largo del titulo de un mensaje sin nombre (su primera linea).
const LARGO_DEL_TITULO: usize = 120;

/// Que se busca.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct OpcionesBusqueda {
    pub consulta: String,
    /// En que clases; vacio es en todas.
    pub clases: Vec<ClaseDeResultado>,
    /// Cuantos de cada clase como mucho (hasta [`TOPE_DE_RESULTADOS`]).
    pub tope: u32,
    /// Los dias que dura una captura (el ajuste de la galeria).
    pub dias_caducidad: i64,
    /// Lo que se suma a UTC para la hora de aqui (los dias de las capturas).
    pub desfase_ms: i64,
}

/// Lo encontrado.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ResultadosEnTodo {
    /// Por puntos y, entre iguales, lo mas nuevo primero.
    pub resultados: Vec<ResultadoBusqueda>,
    /// Las clases que se cortaron en el tope: habria mas.
    pub recortadas: Vec<ClaseDeResultado>,
    /// Se cancelo a mitad: lo que hay es lo que dio tiempo.
    pub cancelada: bool,
}

/// Las busquedas recientes y lo abierto hace poco.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RecientesBusqueda {
    pub busquedas: Vec<String>,
    pub abiertos: Vec<ResultadoBusqueda>,
}

/// Un PDF cuyo texto falta (lo lee Swift con PDFKit).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DocumentoSinTexto {
    pub ruta: String,
    pub nombre: String,
}

/// Lo que queda por leer de los documentos.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PendientesDeTexto {
    /// Los PDF sin leer (como mucho los pedidos).
    pub pdfs: Vec<DocumentoSinTexto>,
    /// Cuantos leyo Rust en esta vuelta (Word, PowerPoint...).
    pub leidos: u32,
}

/// Lo que se va juntando, por clase.
struct Cesta {
    tope: usize,
    quiere: Vec<ClaseDeResultado>,
    por_clase: HashMap<ClaseDeResultado, Vec<ResultadoBusqueda>>,
}

impl Cesta {
    fn quiere(&self, c: ClaseDeResultado) -> bool {
        self.quiere.contains(&c)
    }

    /// Ya tiene uno mas del tope (con eso se sabe que hay mas).
    fn llena(&self, c: ClaseDeResultado) -> bool {
        self.por_clase.get(&c).is_some_and(|v| v.len() > self.tope)
    }

    /// Si aun cabe uno de esa clase.
    fn cabe(&self, c: ClaseDeResultado) -> bool {
        self.quiere(c) && !self.llena(c)
    }

    fn meter(&mut self, r: ResultadoBusqueda) {
        if self.cabe(r.clase) {
            self.por_clase.entry(r.clase).or_default().push(r);
        }
    }
}

/// La consulta ya preparada.
struct Consulta {
    /// Plegada y sin blancos a los lados (`puntuar`).
    q: String,
    /// Como la escribio el usuario (para resaltar).
    cruda: String,
    /// Sus palabras (para el trozo).
    palabras: Vec<String>,
}

impl Consulta {
    fn vacia(&self) -> bool {
        self.q.is_empty()
    }

    /// Los puntos de `nombre` y `dentro` (sin plegar). Sin consulta, todo
    /// vale 0 y entra.
    fn puntos(&self, nombre: &str, dentro: &str) -> u32 {
        puntuar(&self.q, &normalizar(nombre), &normalizar(dentro))
    }

    fn resaltado(&self, titulo: &str) -> Vec<u32> {
        puntos::resaltado(titulo, &self.cruda)
            .into_iter()
            .map(|i| i as u32)
            .collect()
    }
}

/// La clase del mensaje como la escribe el cuaderno (`NOTA`, `ARCHIVO`...).
fn clase_de(m: &Mensaje) -> String {
    m.clase
        .as_ref()
        .and_then(|c| serde_json::to_value(c).ok())
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Si el mensaje es un fichero (va a «Archivos»).
fn es_fichero(clase: &str) -> bool {
    matches!(
        clase,
        "ARCHIVO" | "IMAGEN" | "VOZ" | "VIDEO" | "DIBUJO" | "PAGINA"
    )
}

/// Si es una leccion o una foto o audio suyo: no salen como mensajes, salen
/// como lecciones (`es_de_leccion` del plugin).
fn es_de_leccion(m: &Mensaje) -> bool {
    crate::lecciones::es_leccion(m)
        || m.responde_a
            .as_deref()
            .is_some_and(|r| r.starts_with(pixpin_lecciones::leccion::PREFIJO))
}

/// La primera linea con algo, recortada.
fn primera_linea(texto: &str, largo: usize) -> String {
    let l = texto
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let mut s: String = l.chars().take(largo).collect();
    if l.chars().count() > largo {
        s.push('…');
    }
    s
}

/// El nombre de fichero de una ruta de mensaje (de cualquier aparato).
fn nombre_de_ruta(ruta: &str) -> &str {
    ruta.rsplit(['/', '\\']).next().unwrap_or("")
}

impl Nucleo {
    /// Los resultados de un cuaderno (el mas nuevo primero).
    #[allow(clippy::too_many_arguments)]
    fn en_cuaderno(
        &self,
        texto: &str,
        ficha: &almacen::Ficha,
        c: &Consulta,
        cesta: &mut Cesta,
        busqueda: Option<&Busqueda>,
    ) -> bool {
        let mut hallados: Vec<ResultadoBusqueda> = Vec::new();
        let mut posicion: u64 = 0;
        for linea in texto.lines() {
            if busqueda.is_some_and(Busqueda::cancelada) {
                return true;
            }
            if linea.trim().is_empty() {
                continue;
            }
            let Ok(m) = serde_json::from_str::<Mensaje>(linea) else {
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
            if es_de_leccion(&m) {
                continue;
            }
            let base = |clase, titulo: &str| {
                let mut r = ResultadoBusqueda::de(clase, titulo);
                r.chat = Some(ficha.id.clone());
                r.nombre_chat = ficha.nombre.clone();
                r.mensaje = Some(m.id.clone());
                r.posicion = Some(aqui);
                r.cuando = m.cuando;
                r
            };
            if es_lista(&m) {
                if !cesta.quiere(ClaseDeResultado::Tarea) {
                    continue;
                }
                let lista = nombre_de_lista(&m);
                for (i, t) in mini::leer_tareas(&m.texto).into_iter().enumerate() {
                    let (visible, _) = mini::partir(&t.texto);
                    let (limpio, _) = mini::imagenes_de(visible);
                    let puntos = if c.vacia() {
                        // Sin consulta, solo lo pendiente.
                        if t.hecha {
                            continue;
                        }
                        0
                    } else {
                        let p = c.puntos(&limpio, &lista);
                        if p == 0 {
                            continue;
                        }
                        p
                    };
                    let mut r = base(ClaseDeResultado::Tarea, &limpio);
                    r.tarea = Some(i as u32);
                    r.hecha = t.hecha;
                    r.tipo = "MINIAPP".into();
                    r.fragmento = lista.clone();
                    r.crudo = t.texto.clone();
                    r.puntos = puntos;
                    r.resaltado = c.resaltado(&limpio);
                    r.clave = format!("tarea/{}/{}/{i}", ficha.id, m.id);
                    hallados.push(r);
                }
                continue;
            }
            let clase = clase_de(&m);
            let fichero = es_fichero(&clase);
            let cual = if fichero {
                ClaseDeResultado::Fichero
            } else {
                ClaseDeResultado::Mensaje
            };
            if !cesta.quiere(cual) {
                continue;
            }
            let de_ruta = m.ruta.as_deref().map(nombre_de_ruta).unwrap_or("");
            let titulo = if !m.nombre.trim().is_empty() {
                m.nombre.trim().to_string()
            } else if fichero && !de_ruta.is_empty() && clase != "DIBUJO" && clase != "PAGINA" {
                de_ruta.to_string()
            } else {
                let t = primera_linea(&m.resumen(), LARGO_DEL_TITULO);
                if t.is_empty() { clase.clone() } else { t }
            };
            // El fichero en este aparato (solo los que tienen ruta).
            let ruta = if fichero && !matches!(clase.as_str(), "DIBUJO" | "PAGINA") {
                m.ruta
                    .as_deref()
                    .and_then(|r| self.resolver(&ficha.id, r))
                    .filter(|r| r.is_file())
            } else {
                None
            };
            let transcripcion = m.transcripcion.as_deref().unwrap_or("");
            let dentro_mensaje = format!("{}\n{transcripcion}", m.texto);
            let (puntos, dentro, fragmento) = if c.vacia() {
                // Sin consulta solo salen los ficheros (los recientes).
                if !fichero {
                    continue;
                }
                (0, false, primera_linea(&dentro_mensaje, LARGO_DEL_TITULO))
            } else {
                let p = c.puntos(&titulo, &dentro_mensaje);
                if p > 0 {
                    let donde = format!("{}\n{dentro_mensaje}", m.nombre);
                    (p, false, trozo(&donde, &c.palabras))
                } else {
                    // Lo que pone dentro del documento.
                    let texto = ruta
                        .as_deref()
                        .filter(|r| textos::lector_de(&pixpin_docs::nombre(r)).is_some())
                        .and_then(|r| textos::guardado(self.ruta(), r));
                    let Some(t) = texto else {
                        continue;
                    };
                    let p = puntuar(&c.q, &normalizar(&titulo), &t.plegado);
                    if p == 0 {
                        continue;
                    }
                    (p, true, trozo(&t.original, &c.palabras))
                }
            };
            let mut r = base(cual, &titulo);
            r.tipo = clase;
            r.fragmento = fragmento;
            r.puntos = puntos;
            r.dentro = dentro;
            r.resaltado = c.resaltado(&titulo);
            r.bytes = m.bytes.max(0) as u64;
            r.ruta = ruta.map(|r| r.display().to_string());
            r.clave = format!(
                "{}/{}/{}",
                if fichero { "fichero" } else { "mensaje" },
                ficha.id,
                m.id
            );
            hallados.push(r);
        }
        // Lo mas nuevo primero (las tareas de una lista, en su orden).
        hallados.sort_by(|a, b| b.posicion.cmp(&a.posicion).then(a.tarea.cmp(&b.tarea)));
        for r in hallados {
            cesta.meter(r);
        }
        false
    }

    /// Una captura de la galeria como resultado.
    fn resultado_de_captura(&self, k: &CapturaGaleria, c: Option<&Consulta>) -> ResultadoBusqueda {
        let mut r = ResultadoBusqueda::de(ClaseDeResultado::Captura, &k.nombre);
        r.ruta = Some(k.ruta.clone());
        r.cuando = k.cuando;
        r.bytes = k.bytes;
        r.conservada = k.conservada;
        r.dias_que_quedan = k.dias_que_quedan;
        r.tipo = if k.es_video { "VIDEO" } else { "IMAGEN" }.into();
        r.clave = format!("captura/{}", k.nombre);
        if let Some(c) = c.filter(|c| !c.vacia()) {
            let leido = self.texto_leido_de_captura(k);
            let p = c.puntos(&k.nombre, &leido);
            // La galeria tambien la encuentra por su fecha («viernes»,
            // «3 oct»): eso vale como lo de dentro.
            r.puntos = if p == 0 { puntos::DENTRO_TROZOS } else { p };
            r.dentro = p > 0 && p <= puntos::DENTRO && p != puntos::SALTEADO;
            if r.dentro {
                r.fragmento = trozo(&leido, &c.palabras);
            }
            r.resaltado = c.resaltado(&k.nombre);
        }
        r
    }

    /// Una leccion como resultado.
    fn resultado_de_leccion(
        f: &FichaLeccion,
        nombres: &HashMap<String, String>,
        c: &Consulta,
    ) -> ResultadoBusqueda {
        let titulo = if f.titulo.trim().is_empty() {
            primera_linea(&f.resumen, LARGO_DEL_TITULO)
        } else {
            f.titulo.trim().to_string()
        };
        let mut r = ResultadoBusqueda::de(ClaseDeResultado::Leccion, &titulo);
        r.leccion = Some(f.id.clone());
        if !f.chat.is_empty() {
            r.chat = Some(f.chat.clone());
            r.nombre_chat = nombres.get(&f.chat).cloned().unwrap_or_default();
        }
        r.tipo = f.tipo.clone();
        r.cuando = f.tocada;
        r.clave = format!("leccion/{}", f.id);
        let dentro = format!(
            "{}\n{}\n{}\n{}",
            f.que_paso,
            f.por_que,
            f.proxima,
            f.etiquetas.join(" ")
        );
        if c.vacia() {
            r.fragmento = primera_linea(&f.que_paso, LARGO_DEL_TITULO);
        } else {
            let p = c.puntos(&titulo, &dentro);
            // Su buscador perdona erratas: lo que encontro el y no las
            // reglas de aqui va al final.
            r.puntos = if p == 0 { puntos::SALTEADO } else { p };
            r.fragmento = if p > 0 && p <= puntos::DENTRO {
                trozo(&dentro, &c.palabras)
            } else {
                primera_linea(&f.que_paso, LARGO_DEL_TITULO)
            };
            r.resaltado = c.resaltado(&titulo);
        }
        r
    }

    fn resultado_de_chat(f: &almacen::Ficha, c: &Consulta) -> ResultadoBusqueda {
        let mut r = ResultadoBusqueda::de(ClaseDeResultado::Chat, &f.nombre);
        r.chat = Some(f.id.clone());
        r.nombre_chat = f.nombre.clone();
        r.fragmento = f.resumen.clone();
        r.cuando = f.tocado;
        r.clave = format!("chat/{}", f.id);
        if !c.vacia() {
            r.resaltado = c.resaltado(&f.nombre);
        }
        r
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Busca en todo** lo de [`OpcionesBusqueda`]: ver el modulo. Con
    /// `busqueda`, se puede cancelar desde otro hilo.
    pub fn buscar_en_todo(
        &self,
        opciones: OpcionesBusqueda,
        busqueda: Option<Arc<Busqueda>>,
    ) -> Result<ResultadosEnTodo, ErrorNucleo> {
        let busqueda = busqueda.as_deref();
        let consulta = opciones.consulta.trim();
        let c = Consulta {
            q: normalizar(consulta),
            cruda: consulta.to_string(),
            palabras: palabras(consulta),
        };
        let todas = [
            ClaseDeResultado::Chat,
            ClaseDeResultado::Mensaje,
            ClaseDeResultado::Fichero,
            ClaseDeResultado::Tarea,
            ClaseDeResultado::Leccion,
            ClaseDeResultado::Captura,
            ClaseDeResultado::Momento,
        ];
        let mut cesta = Cesta {
            tope: opciones.tope.clamp(1, TOPE_DE_RESULTADOS) as usize,
            quiere: if opciones.clases.is_empty() {
                todas.to_vec()
            } else {
                opciones.clases.clone()
            },
            por_clase: HashMap::new(),
        };
        let mut cancelada = false;
        let indice = Indice::leer_si_esta(self.ruta())?;
        let fichas: Vec<&almacen::Ficha> = indice
            .ordenadas()
            .into_iter()
            .filter(|f| validar_chat(&f.id).is_ok())
            .collect();

        // 1. Los chats por su nombre (sin consulta, los recientes).
        if cesta.quiere(ClaseDeResultado::Chat) {
            let mut v: Vec<ResultadoBusqueda> = fichas
                .iter()
                .filter_map(|f| {
                    let mut r = Self::resultado_de_chat(f, &c);
                    if !c.vacia() {
                        let p = c.puntos(&f.nombre, "");
                        if p == 0 {
                            return None;
                        }
                        r.puntos = p + EXTRA_CHAT;
                    }
                    Some(r)
                })
                .collect();
            if c.vacia() {
                v.sort_by_key(|r| std::cmp::Reverse(r.cuando));
            }
            v.into_iter().for_each(|r| cesta.meter(r));
        }

        // 2. Las capturas (nombre, fecha y texto leido), de la galeria.
        if cesta.quiere(ClaseDeResultado::Captura) {
            let pagina = self.galeria_buscar(
                consulta.to_string(),
                0,
                cesta.tope as u32 + 1,
                opciones.dias_caducidad,
                opciones.desfase_ms,
            )?;
            for k in &pagina.capturas {
                let r = self.resultado_de_captura(k, Some(&c));
                cesta.meter(r);
            }
        }

        // 3. Las lecciones, con su buscador (perdona erratas).
        if cesta.quiere(ClaseDeResultado::Leccion) {
            let filtro = FiltroLecciones {
                texto: consulta.to_string(),
                ..FiltroLecciones::default()
            };
            if let Ok(p) = self.lecciones(filtro, 0, cesta.tope as u32 + 1) {
                let nombres: HashMap<String, String> = fichas
                    .iter()
                    .map(|f| (f.id.clone(), f.nombre.clone()))
                    .collect();
                for f in &p.lecciones {
                    cesta.meter(Self::resultado_de_leccion(f, &nombres, &c));
                }
            }
        }

        // 4. Los mensajes, ficheros y tareas, chat a chat.
        let de_chats = [
            ClaseDeResultado::Mensaje,
            ClaseDeResultado::Fichero,
            ClaseDeResultado::Tarea,
        ];
        for f in &fichas {
            if !de_chats.iter().any(|k| cesta.cabe(*k)) {
                break;
            }
            if busqueda.is_some_and(Busqueda::cancelada) {
                cancelada = true;
                break;
            }
            let fichero = almacen::carpeta(self.ruta(), &f.id).join("guardados.jsonl");
            let Ok(texto) = std::fs::read_to_string(&fichero) else {
                continue;
            };
            if self.en_cuaderno(&texto, f, &c, &mut cesta, busqueda) {
                cancelada = true;
                break;
            }
        }

        // 5. Los momentos del timeline.
        if !cancelada && cesta.quiere(ClaseDeResultado::Momento) {
            let momentos = self.momentos();
            let a = pixpin_timeline::almacen::Almacen::en(self.ruta());
            let mut v: Vec<&pixpin_timeline::Momento> = if c.vacia() {
                let mut v: Vec<_> = momentos.iter().collect();
                v.sort_by_key(|m| std::cmp::Reverse(m.cuando));
                v
            } else {
                pixpin_timeline::buscar::buscar(&momentos, consulta)
            };
            v.truncate(cesta.tope + 1);
            for m in v {
                let mut r = ResultadoBusqueda::de(ClaseDeResultado::Momento, &m.titulo);
                r.momento = Some(m.id.clone());
                r.cuando = m.cuando;
                r.clave = format!("momento/{}", m.id);
                r.ruta = m
                    .fotos
                    .iter()
                    .find(|f| !f.is_empty() && !f.contains(['/', '\\', '\0']) && *f != "..")
                    .map(|f| a.ruta(f).display().to_string());
                if c.vacia() {
                    r.fragmento = primera_linea(&m.descripcion, LARGO_DEL_TITULO);
                } else {
                    let p = c.puntos(&m.titulo, &m.descripcion);
                    r.puntos = p.max(puntos::DENTRO_TROZOS);
                    r.fragmento = trozo(&m.descripcion, &c.palabras);
                    r.resaltado = c.resaltado(&m.titulo);
                }
                cesta.meter(r);
            }
        }

        let mut recortadas = Vec::new();
        let mut resultados = Vec::new();
        for k in todas {
            if let Some(mut v) = cesta.por_clase.remove(&k) {
                if v.len() > cesta.tope {
                    v.truncate(cesta.tope);
                    recortadas.push(k);
                }
                resultados.extend(v);
            }
        }
        // Por puntos y, entre iguales, lo mas nuevo (el orden es estable:
        // las tareas de una lista siguen en el suyo).
        resultados.sort_by(|a, b| b.puntos.cmp(&a.puntos).then(b.cuando.cmp(&a.cuando)));
        Ok(ResultadosEnTodo {
            resultados,
            recortadas,
            cancelada,
        })
    }

    /// **«Abierto hace poco»** (la caja vacia en «Todo»): lo elegido aqui
    /// antes que siga existiendo y, si es poco, la ultima captura y los
    /// chats recientes, sin repetir, como mucho [`ABIERTOS_A_LA_VISTA`].
    pub fn buscar_inicio(&self, dias_caducidad: i64, desfase_ms: i64) -> Vec<ResultadoBusqueda> {
        let mut v: Vec<ResultadoBusqueda> = Recientes::leer(self.ruta())
            .abiertos
            .into_iter()
            .filter(|r| r.ruta.as_deref().is_none_or(|p| Path::new(p).exists()))
            .collect();
        if v.len() < ABIERTOS_A_LA_VISTA {
            if let Ok(p) = self.galeria(0, 1, dias_caducidad, desfase_ms)
                && let Some(k) = p.capturas.first()
            {
                v.push(self.resultado_de_captura(k, None));
            }
            let c = Consulta {
                q: String::new(),
                cruda: String::new(),
                palabras: Vec::new(),
            };
            if let Ok(indice) = Indice::leer_si_esta(self.ruta()) {
                let mut fichas: Vec<&almacen::Ficha> = indice
                    .proyectos
                    .iter()
                    .filter(|f| validar_chat(&f.id).is_ok())
                    .collect();
                fichas.sort_by_key(|f| std::cmp::Reverse(f.tocado));
                v.extend(fichas.iter().map(|f| Self::resultado_de_chat(f, &c)));
            }
        }
        let mut vistas = HashSet::new();
        v.retain(|r| vistas.insert(r.clave.clone()));
        v.truncate(ABIERTOS_A_LA_VISTA);
        v
    }

    /// Las busquedas recientes y lo abierto hace poco, tal cual.
    pub fn buscar_recientes(&self) -> RecientesBusqueda {
        let r = Recientes::leer(self.ruta());
        RecientesBusqueda {
            busquedas: r.busquedas,
            abiertos: r.abiertos,
        }
    }

    /// Apunta lo elegido (Intro sobre un resultado): la busqueda en las
    /// recientes y el resultado en «Abierto hace poco».
    pub fn buscar_apuntar(
        &self,
        consulta: String,
        abierto: Option<ResultadoBusqueda>,
    ) -> Result<(), ErrorNucleo> {
        let mut r = Recientes::leer(self.ruta());
        r.apuntar_busqueda(&consulta);
        if let Some(a) = abierto {
            r.apuntar_abierto(&a);
        }
        r.guardar(self.ruta())?;
        Ok(())
    }

    /// Quita una busqueda reciente (su ✕).
    pub fn buscar_quitar_busqueda(&self, indice: u32) -> Result<(), ErrorNucleo> {
        let mut r = Recientes::leer(self.ruta());
        r.quitar_busqueda(indice as usize);
        r.guardar(self.ruta())?;
        Ok(())
    }

    /// «Borrar todas» las busquedas recientes.
    pub fn buscar_borrar_busquedas(&self) -> Result<(), ErrorNucleo> {
        let mut r = Recientes::leer(self.ruta());
        r.busquedas.clear();
        r.guardar(self.ruta())?;
        Ok(())
    }

    /// **Lee el texto de los documentos de los chats que aun no se leyo**:
    /// los que sabe leer Rust los lee y guarda aqui (como mucho `cuantos`);
    /// los PDF los devuelve (como mucho `cuantos`) para que Swift los lea
    /// con PDFKit y los apunte con [`Nucleo::poner_texto_de_documento`].
    /// Swift lo llama en segundo plano hasta que no quede nada.
    pub fn documentos_sin_texto(&self, cuantos: u32) -> Result<PendientesDeTexto, ErrorNucleo> {
        let cuantos = cuantos.max(1) as usize;
        let mut pdfs = Vec::new();
        let mut leidos = 0usize;
        let mut vistos = HashSet::new();
        let indice = Indice::leer_si_esta(self.ruta())?;
        'chats: for f in indice.ordenadas() {
            if validar_chat(&f.id).is_err() {
                continue;
            }
            let fichero = almacen::carpeta(self.ruta(), &f.id).join("guardados.jsonl");
            let Ok(texto) = std::fs::read_to_string(&fichero) else {
                continue;
            };
            // Del mas nuevo al mas viejo: lo reciente se busca antes.
            for linea in texto.lines().rev() {
                if pdfs.len() >= cuantos && leidos >= cuantos {
                    break 'chats;
                }
                let Ok(m) = serde_json::from_str::<Mensaje>(linea) else {
                    continue;
                };
                if clase_de(&m) != "ARCHIVO" || es_de_leccion(&m) {
                    continue;
                }
                let Some(ruta) = m
                    .ruta
                    .as_deref()
                    .and_then(|r| self.resolver(&f.id, r))
                    .filter(|r| r.is_file())
                else {
                    continue;
                };
                let nombre = pixpin_docs::nombre(&ruta);
                let Some(lector) = textos::lector_de(&nombre) else {
                    continue;
                };
                if !vistos.insert(ruta.clone()) || textos::esta_leido(self.ruta(), &ruta) {
                    continue;
                }
                match lector {
                    textos::Lector::Pdf => {
                        if pdfs.len() < cuantos {
                            pdfs.push(DocumentoSinTexto {
                                ruta: ruta.display().to_string(),
                                nombre: if m.nombre.trim().is_empty() {
                                    nombre
                                } else {
                                    m.nombre.clone()
                                },
                            });
                        }
                    }
                    _ if leidos < cuantos => {
                        let t = textos::extraer(&ruta).unwrap_or_default();
                        textos::guardar(self.ruta(), &ruta, &t)?;
                        leidos += 1;
                    }
                    _ => {}
                }
            }
        }
        Ok(PendientesDeTexto {
            pdfs,
            leidos: leidos as u32,
        })
    }

    /// Apunta el texto de un documento leido por Swift (vacio tambien vale:
    /// «no tiene texto», y no se vuelve a pedir). Solo de dentro de los
    /// datos de PixPin.
    pub fn poner_texto_de_documento(&self, ruta: String, texto: String) -> Result<(), ErrorNucleo> {
        let ruta = Path::new(&ruta);
        if !ruta.starts_with(self.ruta()) || ruta.components().any(|c| c.as_os_str() == "..") {
            return Err(ErrorNucleo::Argumento {
                detalle: format!("documento fuera de los datos: {}", ruta.display()),
            });
        }
        textos::guardar(self.ruta(), ruta, &texto)?;
        Ok(())
    }
}

#[cfg(test)]
mod pruebas;
