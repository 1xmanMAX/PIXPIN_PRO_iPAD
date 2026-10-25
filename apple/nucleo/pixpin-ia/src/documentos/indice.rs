//! **El indice: trozos de texto con su origen, y BM25 para buscarlos.**
//!
//! ## Que guarda
//!
//! Una [`Fuente`] es algo que se lee entero de una vez: un PDF, un Word, una
//! nota, **el chat de un proyecto** (todos sus mensajes)... Leerla da
//! [`Unidad`]es (una pagina, una diapositiva, una hoja de Excel, un
//! mensaje), y cada unidad se parte en **trozos** de unas
//! [`PALABRAS_POR_TROZO`] palabras que se solapan un poco: lo que se
//! devuelve al modelo es un trozo, no un documento de 200 paginas.
//!
//! ## Incremental, como el indice del buscador del movil
//!
//! Cada fuente lleva su [`Firma`] (tamano y fecha del fichero). Ponerse al
//! dia ([`Indice::pendientes`]) es comparar firmas: lo que no cambio no se
//! vuelve a leer, lo que ya no esta se quita y lo nuevo se lee. Es la misma
//! idea que `IndiceDelBuscador.kt` de Android («solo se escribe lo que
//! cambio: se guarda la firma de cada documento»). Un fichero roto se apunta
//! con su firma y sin trozos: no se reintenta hasta que cambie.
//!
//! ## En el disco
//!
//! En la cache de la app (no se sincroniza): `fuentes.json` con la lista y
//! las firmas, y un `f/<huella>.json` por fuente con sus trozos. Cambiar un
//! chat reescribe su fichero y la lista, no el indice entero. Las listas de
//! palabras (el indice invertido) no se guardan: se rehacen al cargar, que
//! es contar palabras y cuesta menos que leer un fichero el doble de grande.
//!
//! ## BM25
//!
//! El de siempre (k1 = 1,2; b = 0,75) sobre las palabras de `texto.rs`, con
//! un empujon si la palabra esta en el nombre de la fuente («la tesis»
//! encuentra `Tesis.pdf` aunque el trozo no diga «tesis»).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::texto;

/// Lo que mide un trozo, en palabras.
pub const PALABRAS_POR_TROZO: usize = 160;
/// Lo que se solapan dos trozos seguidos, para no partir una idea.
pub const SOLAPE: usize = 30;
/// Lo mas que se guarda de una fuente (un PDF de 2000 paginas no se come la
/// cache entera: lo primero suele ser lo que se busca).
pub const MAX_TROZOS_POR_FUENTE: usize = 4000;
/// La version del formato del disco: si cambia, se rehace todo.
pub const VERSION: u64 = 1;

const K1: f32 = 1.2;
const B: f32 = 0.75;
/// Cuanto vale una palabra de la pregunta que esta en el nombre de la fuente.
const EMPUJON_NOMBRE: f32 = 0.5;
/// Lo minimo que tiene que puntuar un trozo, respecto al mejor, para salir.
const CORTE_RELATIVO: f32 = 0.45;

/// De que es una fuente (lo que se le dice al modelo y al usuario).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tipo {
    Pdf,
    Word,
    Excel,
    PowerPoint,
    /// Una nota Markdown del proyecto.
    Nota,
    /// Un texto llano (`.txt`, `.csv`), p. ej. lo que leyo el OCR.
    Texto,
    /// Los mensajes de un chat (notas, voz pasada a texto, listas y los
    /// nombres de los adjuntos).
    Chat,
}

impl Tipo {
    pub fn clave(self) -> &'static str {
        match self {
            Tipo::Pdf => "pdf",
            Tipo::Word => "word",
            Tipo::Excel => "excel",
            Tipo::PowerPoint => "powerpoint",
            Tipo::Nota => "nota",
            Tipo::Texto => "texto",
            Tipo::Chat => "chat",
        }
    }

    pub fn de_clave(s: &str) -> Option<Tipo> {
        Some(match s {
            "pdf" => Tipo::Pdf,
            "word" => Tipo::Word,
            "excel" => Tipo::Excel,
            "powerpoint" => Tipo::PowerPoint,
            "nota" => Tipo::Nota,
            "texto" => Tipo::Texto,
            "chat" => Tipo::Chat,
            _ => return None,
        })
    }

    /// Como se nombra la unidad: «pág.», «diapositiva»...
    pub fn nombre_de_unidad(self) -> &'static str {
        match self {
            Tipo::PowerPoint => "diapositiva",
            _ => "pág.",
        }
    }
}

/// **Como se abre lo encontrado**: un pedido `abrir` del protocolo
/// (`docs/protocolo-pedidos.md`), el mismo que manda Flow Launcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Abrir {
    /// Un fichero en su lector, en esa pagina (desde 1) si la hay.
    Fichero { ruta: PathBuf, pagina: Option<u32> },
    /// El chat en ese mensaje, resaltado. `proyecto: None` es «Mensajes
    /// guardados».
    Mensaje { proyecto: Option<String>, codigo: String },
}

impl Abrir {
    /// El pedido JSON, listo para mandar.
    pub fn pedido(&self) -> Value {
        match self {
            Abrir::Fichero { ruta, pagina } => {
                let mut que = json!({"tipo": "fichero", "ruta": ruta.display().to_string()});
                if let Some(p) = pagina {
                    que["pagina"] = json!(p);
                }
                json!({"pixpin": 1, "accion": "abrir", "que": que})
            }
            Abrir::Mensaje { proyecto, codigo } => json!({"pixpin": 1, "accion": "abrir",
                "que": {"tipo": "mensaje", "proyecto": proyecto, "codigo": codigo}}),
        }
    }

    fn a_json(&self) -> Value {
        match self {
            Abrir::Fichero { ruta, pagina } => json!({"ruta": ruta.display().to_string(), "pagina": pagina}),
            Abrir::Mensaje { proyecto, codigo } => json!({"proyecto": proyecto, "codigo": codigo}),
        }
    }

    fn de_json(v: &Value) -> Option<Abrir> {
        if let Some(r) = v.get("ruta").and_then(Value::as_str) {
            return Some(Abrir::Fichero {
                ruta: PathBuf::from(r),
                pagina: v.get("pagina").and_then(Value::as_u64).map(|p| p as u32),
            });
        }
        Some(Abrir::Mensaje {
            proyecto: v.get("proyecto").and_then(Value::as_str).map(str::to_string),
            codigo: v.get("codigo")?.as_str()?.to_string(),
        })
    }

    /// La misma, en otra pagina.
    pub fn en_pagina(&self, pagina: Option<u32>) -> Abrir {
        match self {
            Abrir::Fichero { ruta, .. } => Abrir::Fichero {
                ruta: ruta.clone(),
                pagina,
            },
            otro => otro.clone(),
        }
    }
}

/// Lo que dice si una fuente cambio: tamano y fecha de su fichero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Firma {
    pub bytes: u64,
    /// Milisegundos desde 1970.
    pub modificado: i64,
}

impl Firma {
    /// La del fichero, o `None` si no esta.
    pub fn de(ruta: &Path) -> Option<Firma> {
        let m = std::fs::metadata(ruta).ok()?;
        let modificado = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        Some(Firma {
            bytes: m.len(),
            modificado,
        })
    }
}

/// **Una fuente que indexar**, como la describe quien conoce los datos (la
/// app). Lo que hay que leer lo lee un [`Lector`].
#[derive(Debug, Clone, PartialEq)]
pub struct Fuente {
    /// Unica y estable: `f:<ruta>` para un fichero, `chat:<proyecto>`...
    pub clave: String,
    /// El proyecto (la carpeta `proyectos/<id>`) del que es.
    pub proyecto: String,
    /// Como se llama ese proyecto (para decir «en el chat «Obra»»).
    pub proyecto_nombre: String,
    /// Lo que ve el usuario: `Tesis.pdf`, `Chat de Obra`...
    pub nombre: String,
    pub tipo: Tipo,
    /// El fichero que se lee (y cuya firma manda).
    pub ruta: PathBuf,
    pub firma: Firma,
    /// Como se abre (sin pagina: la pone cada unidad).
    pub abrir: Abrir,
}

/// **Un pedazo de lo leido**: una pagina, una diapositiva, una hoja de
/// Excel, un mensaje.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Unidad {
    pub texto: String,
    /// Desde 1.
    pub pagina: Option<u32>,
    /// Un titulo que ayuda a situarlo: la hoja de Excel, el apartado del
    /// Word, «adjunto»...
    pub seccion: Option<String>,
    /// Si se abre distinto que la fuente (un mensaje de un chat).
    pub abrir: Option<Abrir>,
    /// Cuando (ms desde 1970), si se sabe: un mensaje.
    pub cuando: Option<i64>,
}

/// Quien sabe leer las fuentes (la app: PDF, Office, el cuaderno...).
pub trait Lector {
    /// El texto de la fuente en unidades. `Err`: no se pudo leer (roto,
    /// cifrado...), con el motivo.
    fn leer(&self, fuente: &Fuente) -> Result<Vec<Unidad>, String>;
}

/// Un trozo indexado.
#[derive(Debug, Clone)]
struct Trozo {
    fuente: u32,
    texto: String,
    pagina: Option<u32>,
    seccion: Option<String>,
    abrir: Option<Abrir>,
    cuando: Option<i64>,
    /// Cuantas palabras cuenta (para BM25).
    largo: u32,
    vivo: bool,
}

/// Una fuente ya indexada.
#[derive(Debug, Clone)]
struct Guardada {
    fuente: Fuente,
    /// Lo que fallo al leerla, si fallo.
    fallo: Option<String>,
    /// Sus trozos (indices en `Indice::trozos`).
    trozos: Vec<u32>,
    /// Las palabras de su nombre.
    nombre: HashSet<String>,
    viva: bool,
}

/// **A quien se busca**: todo, o solo unos proyectos (desde un chat
/// compartido, solo ese: nunca se mezcla lo de proyectos que la otra
/// persona no ve).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Alcance {
    #[default]
    Todo,
    Proyectos(HashSet<String>),
}

impl Alcance {
    pub fn solo(proyecto: &str) -> Alcance {
        Alcance::Proyectos(HashSet::from([proyecto.to_string()]))
    }

    pub fn deja(&self, proyecto: &str) -> bool {
        match self {
            Alcance::Todo => true,
            Alcance::Proyectos(p) => p.contains(proyecto),
        }
    }
}

/// **Lo encontrado**: un trozo con su origen.
#[derive(Debug, Clone, PartialEq)]
pub struct Hallazgo {
    pub nombre: String,
    pub tipo: Tipo,
    pub proyecto: String,
    pub proyecto_nombre: String,
    pub pagina: Option<u32>,
    pub seccion: Option<String>,
    pub cuando: Option<i64>,
    /// Lo que dice, recortado alrededor de lo buscado.
    pub fragmento: String,
    pub abrir: Abrir,
    pub puntos: f32,
}

/// Lo que hizo una puesta al dia.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Informe {
    pub leidas: usize,
    pub quitadas: usize,
    pub sin_cambios: usize,
    /// Las que no se pudieron leer, con el motivo.
    pub rotas: Vec<(String, String)>,
}

/// **El indice.**
#[derive(Debug, Default)]
pub struct Indice {
    /// Donde vive en el disco (`None`: solo en memoria, para pruebas).
    dir: Option<PathBuf>,
    fuentes: Vec<Guardada>,
    por_clave: HashMap<String, u32>,
    trozos: Vec<Trozo>,
    terminos: HashMap<String, Vec<(u32, u16)>>,
    /// Trozos vivos y la suma de sus largos.
    vivos: u64,
    suma_largos: u64,
    /// Cambiaron cosas que no estan en `fuentes.json`.
    sucio: bool,
}

impl Indice {
    /// Uno vacio que vive en `dir` (no lee nada: ver [`Indice::cargar`]).
    pub fn nuevo(dir: Option<PathBuf>) -> Indice {
        Indice {
            dir,
            ..Default::default()
        }
    }

    /// **Lee el indice de `dir`.** Lo que no se entienda se tira (es una
    /// cache: se rehara la proxima vez); nunca falla.
    pub fn cargar(dir: &Path) -> Indice {
        let mut ix = Indice::nuevo(Some(dir.to_path_buf()));
        let Some(lista) = std::fs::read_to_string(dir.join("fuentes.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        else {
            return ix;
        };
        if lista.get("version").and_then(Value::as_u64) != Some(VERSION) {
            return ix;
        }
        for f in lista.get("fuentes").and_then(Value::as_array).into_iter().flatten() {
            let Some(fuente) = fuente_de_json(f) else { continue };
            let fallo = f.get("fallo").and_then(Value::as_str).map(str::to_string);
            let trozos = if fallo.is_some() {
                Some(Vec::new())
            } else {
                std::fs::read_to_string(dir.join("f").join(archivo_de(&fuente.clave)))
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .and_then(|v| v.get("trozos").and_then(Value::as_array).cloned())
                    .map(|l| l.iter().filter_map(trozo_de_json).collect::<Vec<_>>())
            };
            // Sin su fichero de trozos no se sabe que tenia: se volvera a leer.
            let Some(trozos) = trozos else { continue };
            ix.meter(fuente, fallo, trozos);
        }
        ix.sucio = false;
        ix
    }

    /// Cuantas fuentes y trozos vivos hay.
    pub fn tamano(&self) -> (usize, usize) {
        (self.fuentes.iter().filter(|f| f.viva).count(), self.vivos as usize)
    }

    /// Si ya esta esa fuente con esa firma.
    pub fn al_dia(&self, f: &Fuente) -> bool {
        self.por_clave
            .get(&f.clave)
            .map(|&i| &self.fuentes[i as usize])
            .is_some_and(|g| g.viva && g.fuente.firma == f.firma && g.fuente.abrir == f.abrir && g.fuente.nombre == f.nombre && g.fuente.proyecto_nombre == f.proyecto_nombre)
    }

    /// **Que hay que hacer** para que el indice sea `fuentes`: las que leer
    /// (nuevas o cambiadas) y las claves que quitar (ya no estan).
    pub fn pendientes(&self, fuentes: &[Fuente]) -> (Vec<Fuente>, Vec<String>) {
        let nuevas: HashSet<&str> = fuentes.iter().map(|f| f.clave.as_str()).collect();
        let leer = fuentes.iter().filter(|f| !self.al_dia(f)).cloned().collect();
        let quitar = self
            .fuentes
            .iter()
            .filter(|g| g.viva && !nuevas.contains(g.fuente.clave.as_str()))
            .map(|g| g.fuente.clave.clone())
            .collect();
        (leer, quitar)
    }

    /// **Mete lo leido de una fuente** (sustituye lo que hubiera).
    pub fn aplicar(&mut self, fuente: Fuente, leido: Result<Vec<Unidad>, String>) {
        self.quitar(&fuente.clave);
        let (fallo, trozos) = match leido {
            Ok(u) => (None, trocear(&u)),
            Err(e) => (Some(e), Vec::new()),
        };
        if let Some(dir) = &self.dir {
            let arch = dir.join("f").join(archivo_de(&fuente.clave));
            if fallo.is_none() {
                let v = json!({"clave": fuente.clave, "trozos": trozos.iter().map(trozo_a_json).collect::<Vec<_>>()});
                if let Err(e) = escribir(&arch, &v.to_string()) {
                    tracing::warn!(%e, "documentos: no se pudo guardar una fuente del indice");
                }
            } else {
                let _ = std::fs::remove_file(arch);
            }
        }
        self.meter(fuente, fallo, trozos);
        self.sucio = true;
    }

    /// Quita una fuente (si estaba).
    pub fn quitar(&mut self, clave: &str) {
        let Some(&i) = self.por_clave.get(clave) else { return };
        let g = &mut self.fuentes[i as usize];
        if !g.viva {
            return;
        }
        g.viva = false;
        for &t in &g.trozos {
            let t = &mut self.trozos[t as usize];
            if t.vivo {
                t.vivo = false;
                self.vivos -= 1;
                self.suma_largos -= u64::from(t.largo);
            }
        }
        if let Some(dir) = &self.dir {
            let _ = std::fs::remove_file(dir.join("f").join(archivo_de(clave)));
        }
        self.por_clave.remove(clave);
        self.sucio = true;
        // Si lo muerto ya pesa mas que lo vivo, se rehace en limpio.
        if self.trozos.len() as u64 > 2 * self.vivos + 1000 {
            self.compactar();
        }
    }

    /// **Pone al dia** con un lector, todo de una vez (pruebas y quien no
    /// tenga prisa; la app lo hace por partes para no tener el indice
    /// cogido mientras lee un PDF largo).
    pub fn poner_al_dia(&mut self, fuentes: &[Fuente], lector: &dyn Lector) -> Informe {
        let (leer, quitar) = self.pendientes(fuentes);
        let mut inf = Informe {
            sin_cambios: fuentes.len() - leer.len(),
            quitadas: quitar.len(),
            ..Default::default()
        };
        for c in quitar {
            self.quitar(&c);
        }
        for f in leer {
            let r = lector.leer(&f);
            if let Err(e) = &r {
                inf.rotas.push((f.nombre.clone(), e.clone()));
            }
            inf.leidas += 1;
            self.aplicar(f, r);
        }
        self.guardar();
        inf
    }

    /// Escribe la lista de fuentes si cambio (los trozos ya se escribieron
    /// al aplicar cada una).
    pub fn guardar(&mut self) {
        if !self.sucio {
            return;
        }
        let Some(dir) = &self.dir else {
            self.sucio = false;
            return;
        };
        let lista: Vec<Value> = self
            .fuentes
            .iter()
            .filter(|g| g.viva)
            .map(|g| {
                let mut v = fuente_a_json(&g.fuente);
                if let Some(f) = &g.fallo {
                    v["fallo"] = json!(f);
                }
                v
            })
            .collect();
        let v = json!({"version": VERSION, "fuentes": lista});
        match escribir(&dir.join("fuentes.json"), &v.to_string()) {
            Ok(()) => self.sucio = false,
            Err(e) => tracing::warn!(%e, "documentos: no se pudo guardar la lista del indice"),
        }
    }

    fn meter(&mut self, fuente: Fuente, fallo: Option<String>, trozos: Vec<Trozo>) {
        let i = self.fuentes.len() as u32;
        let mut ids = Vec::with_capacity(trozos.len());
        for mut t in trozos {
            t.fuente = i;
            let id = self.trozos.len() as u32;
            let mut cuenta: HashMap<String, u16> = HashMap::new();
            let mut largo = 0u32;
            for p in texto::palabras(&t.texto) {
                *cuenta.entry(p).or_default() += 1;
                largo += 1;
            }
            // Lo que situa el trozo (la hoja de Excel, el adjunto) tambien
            // se busca.
            if let Some(s) = &t.seccion {
                for p in texto::palabras(s) {
                    *cuenta.entry(p).or_default() += 1;
                    largo += 1;
                }
            }
            for (p, n) in cuenta {
                self.terminos.entry(p).or_default().push((id, n));
            }
            t.largo = largo;
            t.vivo = true;
            self.vivos += 1;
            self.suma_largos += u64::from(largo);
            self.trozos.push(t);
            ids.push(id);
        }
        let nombre = texto::palabras(&fuente.nombre).into_iter().collect();
        self.por_clave.insert(fuente.clave.clone(), i);
        self.fuentes.push(Guardada {
            fuente,
            fallo,
            trozos: ids,
            nombre,
            viva: true,
        });
    }

    /// Rehace todo sin lo muerto.
    fn compactar(&mut self) {
        let viejas = std::mem::take(&mut self.fuentes);
        let trozos = std::mem::take(&mut self.trozos);
        self.por_clave.clear();
        self.terminos.clear();
        self.vivos = 0;
        self.suma_largos = 0;
        for g in viejas.into_iter().filter(|g| g.viva) {
            let ts = g.trozos.iter().map(|&t| trozos[t as usize].clone()).collect();
            self.meter(g.fuente, g.fallo, ts);
        }
    }

    /// **Busca.** Los `max` mejores trozos de lo que deja `alcance`, como
    /// mucho [`POR_FUENTE`] de cada fuente (que un PDF largo no tape lo
    /// demas). Una pregunta sin palabras que buscar no da nada.
    pub fn buscar(&self, consulta: &str, alcance: &Alcance, max: usize) -> Vec<Hallazgo> {
        let q = texto::de_la_consulta(consulta);
        if q.is_empty() || self.vivos == 0 {
            return Vec::new();
        }
        let n = self.vivos as f32;
        let medio = (self.suma_largos as f32 / n).max(1.0);
        let deja = |t: &Trozo| t.vivo && alcance.deja(&self.fuentes[t.fuente as usize].fuente.proyecto);
        // Lo raro que es cada palabra (una que no esta en ningun texto, como
        // el nombre de un plano, es la mas rara de todas).
        let idf = |p: &str| -> f32 {
            let df = self
                .terminos
                .get(p)
                .map_or(0, |l| l.iter().filter(|(t, _)| self.trozos[*t as usize].vivo).count()) as f32;
            ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
        };
        let mut puntos: HashMap<u32, f32> = HashMap::new();
        for p in &q {
            let Some(lista) = self.terminos.get(p) else { continue };
            let idf = idf(p);
            for &(t, tf) in lista {
                let tr = &self.trozos[t as usize];
                if !deja(tr) {
                    continue;
                }
                let tf = f32::from(tf);
                let s = idf * tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * tr.largo as f32 / medio));
                *puntos.entry(t).or_default() += s;
            }
        }
        // El empujon del nombre: «la tesis» encuentra Tesis.pdf. Suma a los
        // trozos de la fuente que ya casaban con algo y, si ninguno casaba,
        // a su primero (preguntar por un plano por su nombre). Vale algo
        // menos que la misma palabra en el texto: lo que el documento DICE
        // pesa mas que como se llama.
        let mut por_nombre: HashMap<u32, f32> = HashMap::new();
        for (i, g) in self.fuentes.iter().enumerate() {
            if !g.viva || !alcance.deja(&g.fuente.proyecto) {
                continue;
            }
            let e: f32 = q.iter().filter(|p| g.nombre.contains(*p)).map(|p| EMPUJON_NOMBRE * idf(p)).sum();
            if e > 0.0 {
                por_nombre.insert(i as u32, e);
            }
        }
        let mut con_texto: HashSet<u32> = HashSet::new();
        for (t, s) in puntos.iter_mut() {
            let f = self.trozos[*t as usize].fuente;
            con_texto.insert(f);
            if let Some(e) = por_nombre.get(&f) {
                *s += e;
            }
        }
        for (&f, &e) in &por_nombre {
            if con_texto.contains(&f) {
                continue;
            }
            if let Some(&t) = self.fuentes[f as usize].trozos.first() {
                puntos.insert(t, e);
            }
        }
        let mut orden: Vec<(u32, f32)> = puntos.into_iter().collect();
        orden.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        // Lo que puntua mucho menos que lo mejor es ruido: a un modelo de 2B
        // tres trozos flojos le tapan el bueno (medido el 14-oct con «¿donde
        // mande el plano de cunetas?»).
        let tope = orden.first().map_or(0.0, |o| o.1 * CORTE_RELATIVO);
        orden.retain(|o| o.1 >= tope);
        let mut cuantos: HashMap<u32, usize> = HashMap::new();
        let mut fuera = Vec::new();
        for (t, s) in orden {
            let tr = &self.trozos[t as usize];
            let c = cuantos.entry(tr.fuente).or_default();
            if *c >= POR_FUENTE {
                continue;
            }
            *c += 1;
            let g = &self.fuentes[tr.fuente as usize];
            fuera.push(Hallazgo {
                nombre: g.fuente.nombre.clone(),
                tipo: g.fuente.tipo,
                proyecto: g.fuente.proyecto.clone(),
                proyecto_nombre: g.fuente.proyecto_nombre.clone(),
                pagina: tr.pagina,
                seccion: tr.seccion.clone(),
                cuando: tr.cuando,
                fragmento: fragmento(&tr.texto, &q, LARGO_FRAGMENTO),
                abrir: tr.abrir.clone().unwrap_or_else(|| g.fuente.abrir.en_pagina(tr.pagina)),
                puntos: s,
            });
            if fuera.len() >= max {
                break;
            }
        }
        fuera
    }
}

/// Cuantos trozos de una misma fuente como mucho en una busqueda.
pub const POR_FUENTE: usize = 2;
/// Lo largo de lo que se ensena de cada trozo, en letras.
pub const LARGO_FRAGMENTO: usize = 420;

/// **Parte las unidades en trozos** de [`PALABRAS_POR_TROZO`] palabras con
/// [`SOLAPE`]. Una unidad nunca se mezcla con otra (cada trozo es de una
/// pagina, y asi su enlace lleva a ella).
fn trocear(unidades: &[Unidad]) -> Vec<Trozo> {
    let mut v = Vec::new();
    for u in unidades {
        let palabras: Vec<&str> = u.texto.split_whitespace().collect();
        if palabras.is_empty() {
            continue;
        }
        let mut desde = 0;
        loop {
            let hasta = (desde + PALABRAS_POR_TROZO).min(palabras.len());
            v.push(Trozo {
                fuente: 0,
                texto: palabras[desde..hasta].join(" "),
                pagina: u.pagina,
                seccion: u.seccion.clone(),
                abrir: u.abrir.clone(),
                cuando: u.cuando,
                largo: 0,
                vivo: true,
            });
            if hasta == palabras.len() || v.len() >= MAX_TROZOS_POR_FUENTE {
                break;
            }
            desde = hasta - SOLAPE;
        }
        if v.len() >= MAX_TROZOS_POR_FUENTE {
            break;
        }
    }
    v
}

/// **El trozo recortado alrededor de lo buscado**: la ventana de `largo`
/// letras con mas palabras de la pregunta, cortada en palabras enteras.
pub fn fragmento(texto: &str, consulta: &[String], largo: usize) -> String {
    let letras: Vec<char> = texto.chars().collect();
    if letras.len() <= largo {
        return texto.to_string();
    }
    // Donde empieza cada palabra que casa.
    let mut aciertos: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < letras.len() {
        if letras[i].is_alphanumeric() {
            let ini = i;
            while i < letras.len() && letras[i].is_alphanumeric() {
                i += 1;
            }
            let palabra: String = letras[ini..i].iter().collect();
            if texto::palabras(&palabra).iter().any(|p| consulta.contains(p)) {
                aciertos.push(ini);
            }
        } else {
            i += 1;
        }
    }
    let mut mejor = (0usize, 0usize);
    for &a in &aciertos {
        let desde = a.saturating_sub(largo / 4);
        let n = aciertos.iter().filter(|&&b| b >= desde && b < desde + largo).count();
        if n > mejor.1 {
            mejor = (desde, n);
        }
    }
    let mut desde = mejor.0.min(letras.len().saturating_sub(largo));
    while desde > 0 && !letras[desde - 1].is_whitespace() {
        desde -= 1;
    }
    let mut hasta = (desde + largo).min(letras.len());
    while hasta < letras.len() && !letras[hasta].is_whitespace() && hasta - desde < largo + 30 {
        hasta += 1;
    }
    let mut s: String = letras[desde..hasta].iter().collect();
    if desde > 0 {
        s.insert(0, '…');
    }
    if hasta < letras.len() {
        s.push('…');
    }
    s
}

// ---------------------------------------------------------------------------
// El disco

/// El nombre del fichero de una fuente: la huella de su clave.
fn archivo_de(clave: &str) -> String {
    use sha2::Digest;
    let h = sha2::Sha256::digest(clave.as_bytes());
    let hex: String = h.iter().take(12).map(|b| format!("{b:02x}")).collect();
    format!("{hex}.json")
}

/// Escribe por un temporal: a medias no queda nada que se lea mal.
fn escribir(ruta: &Path, texto: &str) -> std::io::Result<()> {
    if let Some(d) = ruta.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = ruta.with_extension("tmp");
    std::fs::write(&tmp, texto)?;
    std::fs::rename(&tmp, ruta)
}

fn fuente_a_json(f: &Fuente) -> Value {
    json!({
        "clave": f.clave, "proyecto": f.proyecto, "proyectoNombre": f.proyecto_nombre,
        "nombre": f.nombre, "tipo": f.tipo.clave(), "ruta": f.ruta.display().to_string(),
        "bytes": f.firma.bytes, "modificado": f.firma.modificado, "abrir": f.abrir.a_json(),
    })
}

fn fuente_de_json(v: &Value) -> Option<Fuente> {
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    Some(Fuente {
        clave: s("clave")?,
        proyecto: s("proyecto")?,
        proyecto_nombre: s("proyectoNombre").unwrap_or_default(),
        nombre: s("nombre")?,
        tipo: Tipo::de_clave(&s("tipo")?)?,
        ruta: PathBuf::from(s("ruta")?),
        firma: Firma {
            bytes: v.get("bytes")?.as_u64()?,
            modificado: v.get("modificado")?.as_i64()?,
        },
        abrir: Abrir::de_json(v.get("abrir")?)?,
    })
}

fn trozo_a_json(t: &Trozo) -> Value {
    let mut v = json!({"t": t.texto});
    if let Some(p) = t.pagina {
        v["p"] = json!(p);
    }
    if let Some(s) = &t.seccion {
        v["s"] = json!(s);
    }
    if let Some(a) = &t.abrir {
        v["a"] = a.a_json();
    }
    if let Some(c) = t.cuando {
        v["c"] = json!(c);
    }
    v
}

fn trozo_de_json(v: &Value) -> Option<Trozo> {
    Some(Trozo {
        fuente: 0,
        texto: v.get("t")?.as_str()?.to_string(),
        pagina: v.get("p").and_then(Value::as_u64).map(|p| p as u32),
        seccion: v.get("s").and_then(Value::as_str).map(str::to_string),
        abrir: v.get("a").and_then(Abrir::de_json),
        cuando: v.get("c").and_then(Value::as_i64),
        largo: 0,
        vivo: true,
    })
}
