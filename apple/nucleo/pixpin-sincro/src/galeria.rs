//! **La galeria de capturas, la misma en todos los aparatos del grupo**
//! (Android v0.107.0, 8-oct-2026). El usuario: «quiero que se sincronice la
//! galeria con sus tiempos de desaparicion y todo completo».
//!
//! Puerto directo de `capture/GaleriaCompartida.kt` (las reglas, sin
//! aparato) y de `sincro/GaleriaQueViaja.kt` (los pasos que dan los dos
//! lados), con la guia que Android escribio para el PC
//! (`docs/galeria-que-viaja.md`). Lo del cable —las cuatro peticiones
//! `galeria`, `galeriajunta`, `damecaptura` y `poncaptura`— vive en
//! `protocolo`, junto al resto de la conversacion.
//!
//! Cada captura tiene una [`Entrada`] que viaja al sincronizar: cuando se
//! hizo, **cuando se va** (la fecha exacta, no «a los N dias»: asi se va a la
//! vez en todos aunque llegara a cada uno otro dia), si se conserva y si se
//! borro. Lo cambiado mas tarde gana ([`juntar`]).
//!
//! Las tres decisiones de Android, iguales aqui:
//!
//! - **Caducar no es borrar.** Cada aparato barre lo suyo con la misma fecha
//!   (el registro de caducidad con `fijadas`), asi que no hace falta avisar a
//!   nadie. Solo lo que el usuario quita **antes de tiempo** deja una marca de
//!   borrada que se lleva la captura de los demas (a su papelera).
//! - **Lo que no esta porque no se pudo ver no cuenta.** Solo se marca como
//!   borrada una captura que este aparato llego a tener ([`Estado::tenia`]) y
//!   ya no esta; si no se pudo listar (`None`), no se toca nada.
//! - **El registro de siempre manda en la pantalla.** Lo acordado se escribe
//!   en el registro de caducidad ([`aplicar`]): la galeria y el barrendero no
//!   necesitan saber nada de esto.
//!
//! El protocolo NO sube de version (sigue en 4): un aparato que no sabe de
//! galerias contesta «No sé qué es «galeria»» y la vuelta sigue sin ella.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::protocolo::Resultado;

/// El fichero del estado compartido, en la raiz de los datos.
pub const FICHERO: &str = "galeria-compartida.json";
/// Un dia en milisegundos.
pub const DIA_MS: i64 = 86_400_000;
/// Lo que da «Dar 7 dias mas».
pub const DIAS_DE_PRORROGA: i64 = 7;

/// Lo que se pide para empezar.
pub const PETICION: &str = "galeria";
/// Lo juntado, para que el otro lo guarde.
pub const JUNTA: &str = "galeriajunta";
/// «Dame esta captura».
pub const DAME: &str = "damecaptura";
/// «Toma esta captura».
pub const PON: &str = "poncaptura";

/// Si una peticion es de la galeria.
pub fn es_de_galeria(t: &str) -> bool {
    matches!(t, PETICION | JUNTA | DAME | PON)
}

// ------------------------------------------------------------ el registro

/// **El registro de caducidad de las capturas** (`capturas-caducidad.json`),
/// el mismo fichero y la misma forma en el PC y en Android
/// (`CaducidadDeCapturas.Registro`). Vive aqui y no en la aplicacion porque
/// las reglas de la galeria compartida lo leen y lo cambian; la aplicacion
/// lo lee y lo escribe en su sitio (`caducidad_capturas`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caducidad {
    /// La primera vez que se miro la caducidad, en ms UTC. Nada caduca
    /// antes de `desde + dias`.
    pub desde: i64,
    /// Los nombres de fichero de las capturas que no se van.
    #[serde(default)]
    pub conservadas: BTreeSet<String>,
    /// Las que se dejaron estar mas («Dar 7 dias mas»): nombre y la fecha
    /// nueva en que se van, en ms UTC. Si es anterior a la de la regla, gana
    /// la regla: prorrogar nunca acorta.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub prorrogadas: BTreeMap<String, i64>,
    /// **La fecha exacta en que se va, acordada con el grupo** (la galeria
    /// que viaja). Manda sobre la regla y las prorrogas: asi una captura se
    /// va a la vez en todos los aparatos aunque llegara a cada uno otro dia.
    /// Opcional: un registro sin el se lee igual, y vacio no se escribe.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fijadas: BTreeMap<String, i64>,
}

/// `CaducidadDeCapturas.seVaEl`: cuando se va la captura `nombre` hecha en
/// `cuando` (ms UTC). `None`: no se va. Con `dias` <= 0 no se va ninguna.
/// Primero lo conservado, luego lo acordado con el grupo y por ultimo la
/// regla con su prorroga.
pub fn se_va_el(r: &Caducidad, nombre: &str, cuando: i64, dias: i64) -> Option<i64> {
    if dias <= 0 || r.conservadas.contains(nombre) {
        return None;
    }
    if let Some(&t) = r.fijadas.get(nombre) {
        return Some(t);
    }
    let regla = cuando.max(r.desde) + dias * DIA_MS;
    Some(match r.prorrogadas.get(nombre) {
        Some(&t) => regla.max(t),
        None => regla,
    })
}

/// La fecha nueva de «Dar 7 dias mas»: siete dias despues de la que tenia, o
/// de ahora si ya habia pasado. `None` si no se va.
pub fn fecha_prorrogada(
    r: &Caducidad,
    nombre: &str,
    cuando: i64,
    ahora: i64,
    dias: i64,
) -> Option<i64> {
    se_va_el(r, nombre, cuando, dias).map(|t| t.max(ahora) + DIAS_DE_PRORROGA * DIA_MS)
}

/// `CaducidadDeCapturas.prorrogada`: apunta la prorroga. Con fecha acordada
/// se mueve esa, que es la que viaja; si no, va a `prorrogadas`.
pub fn prorrogar(r: &mut Caducidad, nombre: &str, cuando: i64, ahora: i64, dias: i64) {
    let Some(t) = fecha_prorrogada(r, nombre, cuando, ahora, dias) else {
        return;
    };
    if let Some(f) = r.fijadas.get_mut(nombre) {
        *f = t;
    } else {
        r.prorrogadas.insert(nombre.to_string(), t);
    }
}

// ------------------------------------------------------------ lo que viaja

fn es_cero(n: &i64) -> bool {
    *n == 0
}

fn es_falso(b: &bool) -> bool {
    !*b
}

fn png() -> String {
    "image/png".into()
}

fn es_png(m: &String) -> bool {
    m == "image/png"
}

/// Una captura tal como viaja (`GaleriaCompartida.Entrada`). Los campos con
/// su valor por omision no se escriben, como `encodeDefaults = false` alli.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entrada {
    /// El nombre del fichero; es la clave. Sin rutas.
    pub nombre: String,
    /// Cuando se hizo, en ms UTC (en el aparato donde se hizo).
    pub cuando: i64,
    #[serde(default, skip_serializing_if = "es_cero")]
    pub bytes: i64,
    #[serde(default = "png", skip_serializing_if = "es_png")]
    pub mime: String,
    /// Cuando se va, en ms UTC. `None` y no conservada: no se va (alli no
    /// caducaba nada).
    #[serde(rename = "seVa", default, skip_serializing_if = "Option::is_none")]
    pub se_va: Option<i64>,
    #[serde(default, skip_serializing_if = "es_falso")]
    pub conservada: bool,
    /// Quitada a mano antes de tiempo: se quita en todos.
    #[serde(default, skip_serializing_if = "es_falso")]
    pub borrada: bool,
    /// Cuando cambio por ultima vez algo de lo de arriba: lo mas nuevo gana
    /// al juntar.
    #[serde(default, skip_serializing_if = "es_cero")]
    pub cambiado: i64,
    /// El aparato donde se hizo, solo para ensenarlo.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub de: String,
    /// **Lo que pone dentro**: el texto reconocido de la captura (el OCR de
    /// Windows en el PC), con el que la galeria la encuentra al buscar.
    /// Viaja con la entrada para que el otro aparato la encuentre por su
    /// texto sin volver a leerla. Vacio: sin leer o sin texto, y entonces
    /// no se escribe. Android aun no lo tiene: lo ignora al leer
    /// (`ignoreUnknownKeys`) y lo pierde al juntar, por eso aqui nunca se
    /// olvida un texto que ya se tenia ([`juntar`], [`aplicar_juntas`]).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub texto: String,
}

impl Entrada {
    /// Una entrada con lo minimo, como `Entrada(nombre, cuando)` en Kotlin.
    pub fn nueva(nombre: &str, cuando: i64) -> Entrada {
        Entrada {
            nombre: nombre.to_string(),
            cuando,
            bytes: 0,
            mime: png(),
            se_va: None,
            conservada: false,
            borrada: false,
            cambiado: 0,
            de: String::new(),
            texto: String::new(),
        }
    }

    /// Ya paso su fecha: cada aparato la barre solo, no hace falta pasarla.
    pub fn caducada(&self, ahora: i64) -> bool {
        !self.borrada && !self.conservada && self.se_va.is_some_and(|t| t <= ahora)
    }

    /// Viva y con sentido tenerla: hay que traerla o mandarla si falta.
    pub fn hay_que_tenerla(&self, ahora: i64) -> bool {
        !self.borrada && !self.caducada(ahora)
    }
}

/// Lo de este aparato: las entradas (las que viajan) y lo que habia aqui la
/// ultima vez. Es `<datos>/galeria-compartida.json`, local.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Estado {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entradas: Vec<Entrada>,
    /// Las capturas que habia en este aparato al ponerse al dia: para saber
    /// que se quito.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub tenia: BTreeSet<String>,
}

impl Estado {
    pub fn por_nombre(&self) -> HashMap<&str, &Entrada> {
        self.entradas
            .iter()
            .map(|e| (e.nombre.as_str(), e))
            .collect()
    }
}

/// Una captura que hay en este aparato ahora.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    pub nombre: String,
    pub cuando: i64,
    pub bytes: i64,
    pub mime: String,
    /// El texto que ya se leyo de ella (vacio: aun no, o no tiene).
    pub texto: String,
}

// ------------------------------------------------------------ reglas

/// **Lo de este aparato, al dia** (`alDia`): entradas nuevas para las
/// capturas que aun no tenian, lo que el usuario cambio aqui desde la ultima
/// vez (conservar, «7 dias mas») y las que se quitaron a mano. `locales`
/// `None` = no se pudo listar: se devuelve igual.
pub fn al_dia(
    e: &Estado,
    locales: Option<&[Local]>,
    r: &Caducidad,
    dias: i64,
    ahora: i64,
    aparato: &str,
) -> Estado {
    let Some(locales) = locales else {
        return e.clone();
    };
    let hay: BTreeSet<String> = locales.iter().map(|c| c.nombre.clone()).collect();
    // En el orden de antes y las nuevas detras, como el `LinkedHashMap` de
    // Kotlin: el fichero no baila de una vuelta a otra.
    let mut nuevas: Vec<Entrada> = Vec::new();
    let mut donde: HashMap<String, usize> = HashMap::new();
    for x in &e.entradas {
        match donde.get(&x.nombre) {
            Some(&i) => nuevas[i] = x.clone(),
            None => {
                donde.insert(x.nombre.clone(), nuevas.len());
                nuevas.push(x.clone());
            }
        }
    }
    for c in locales {
        let conservada = r.conservadas.contains(&c.nombre);
        let Some(&i) = donde.get(&c.nombre) else {
            donde.insert(c.nombre.clone(), nuevas.len());
            nuevas.push(Entrada {
                nombre: c.nombre.clone(),
                cuando: c.cuando,
                bytes: c.bytes,
                mime: c.mime.clone(),
                se_va: if conservada {
                    None
                } else {
                    se_va_el(r, &c.nombre, c.cuando, dias)
                },
                conservada,
                borrada: false,
                cambiado: c.cuando,
                de: aparato.to_string(),
                texto: c.texto.clone(),
            });
            continue;
        };
        // El texto leido aqui despues de apuntarla (la galeria lo lee en
        // segundo plano): se anade sin tocar `cambiado`, que no es un cambio
        // del usuario sino algo mas que se sabe de la misma imagen.
        if nuevas[i].texto.is_empty() && !c.texto.is_empty() {
            nuevas[i].texto = c.texto.clone();
        }
        let vieja = &nuevas[i];
        if vieja.borrada {
            // Se quito en otro: la quitara `a_tirar`.
            continue;
        }
        // La fecha que rige aqui: la acordada (`fijadas`) o, si aun no se
        // acordo, la regla con sus prorrogas. Sin caducidad aqui (dias <= 0)
        // no se dice nada: eso es de este aparato.
        let efectiva = if dias <= 0 {
            vieja.se_va
        } else {
            se_va_el(r, &c.nombre, c.cuando, dias)
        };
        if conservada && !vieja.conservada {
            // Se conservo aqui.
            let v = &mut nuevas[i];
            v.conservada = true;
            v.se_va = None;
            v.cambiado = ahora;
        } else if !vieja.conservada && !conservada && efectiva.is_some() && efectiva != vieja.se_va
        {
            // «7 dias mas» aqui: la fecha se movio.
            let v = &mut nuevas[i];
            v.se_va = efectiva;
            v.cambiado = ahora;
        }
    }
    // Las que tenia y ya no estan: quitadas a mano, si no les tocaba irse.
    for n in &e.tenia {
        if hay.contains(n) {
            continue;
        }
        let Some(&i) = donde.get(n) else { continue };
        let v = &mut nuevas[i];
        if v.borrada || v.caducada(ahora) {
            continue;
        }
        v.borrada = true;
        v.cambiado = ahora;
    }
    Estado {
        entradas: nuevas,
        tenia: hay,
    }
}

/// El orden de `ganadora`: lo cambiado mas tarde; a la misma hora, borrada
/// antes que viva, conservada antes que no, y la fecha mas lejana (sin
/// fecha es lo mas lejano). Igual en los dos sentidos.
fn clave(e: &Entrada) -> (i64, bool, bool, i64, i64) {
    (
        e.cambiado,
        e.borrada,
        e.conservada,
        e.se_va.unwrap_or(i64::MAX),
        e.bytes,
    )
}

/// **Junta lo de dos aparatos** (`juntar`), igual en los dos lados: por cada
/// captura, la entrada que gana; por nombre.
pub fn juntar(a: &[Entrada], b: &[Entrada]) -> Vec<Entrada> {
    let pa: HashMap<&str, &Entrada> = a.iter().map(|e| (e.nombre.as_str(), e)).collect();
    let pb: HashMap<&str, &Entrada> = b.iter().map(|e| (e.nombre.as_str(), e)).collect();
    let todas: BTreeSet<&str> = pa.keys().chain(pb.keys()).copied().collect();
    todas
        .into_iter()
        .map(|n| match (pa.get(n), pb.get(n)) {
            (Some(x), Some(y)) => {
                // `ganadora`: a igual clave, la del primero, como alli.
                let (mut g, otra) = if clave(x) >= clave(y) {
                    ((*x).clone(), *y)
                } else {
                    ((*y).clone(), *x)
                };
                // El texto es de la imagen, no de lo que se decidio sobre
                // ella: si la que gana no lo trae (Android no lo conoce), se
                // queda el de la otra.
                if g.texto.is_empty() {
                    g.texto = otra.texto.clone();
                }
                g
            }
            (Some(x), None) => (*x).clone(),
            (None, Some(y)) => (*y).clone(),
            (None, None) => unreachable!("el nombre sale de uno de los dos"),
        })
        .collect()
}

/// **Lo acordado, en el registro de caducidad** (`aplicar`): conservadas y
/// fechas fijadas. Las borradas se olvidan (su nombre queda libre).
pub fn aplicar(r: &Caducidad, entradas: &[Entrada]) -> Caducidad {
    let borradas: BTreeSet<&str> = entradas
        .iter()
        .filter(|e| e.borrada)
        .map(|e| e.nombre.as_str())
        .collect();
    let mut n = r.clone();
    n.conservadas.retain(|x| !borradas.contains(x.as_str()));
    n.prorrogadas.retain(|x, _| !borradas.contains(x.as_str()));
    n.fijadas.retain(|x, _| !borradas.contains(x.as_str()));
    for e in entradas.iter().filter(|e| !e.borrada) {
        if e.conservada {
            n.conservadas.insert(e.nombre.clone());
        } else if let Some(t) = e.se_va {
            n.fijadas.insert(e.nombre.clone(), t);
        }
    }
    n
}

/// Las capturas de aqui que se quitaron en otro aparato: a la papelera.
pub fn a_tirar(entradas: &[Entrada], aqui: &BTreeSet<String>) -> Vec<String> {
    entradas
        .iter()
        .filter(|e| e.borrada && aqui.contains(&e.nombre))
        .map(|e| e.nombre.clone())
        .collect()
}

/// Las que me faltan y el otro tiene: hay que traerlas.
pub fn que_traer(
    entradas: &[Entrada],
    mias: &BTreeSet<String>,
    suyas: &BTreeSet<String>,
    ahora: i64,
) -> Vec<Entrada> {
    entradas
        .iter()
        .filter(|e| {
            e.hay_que_tenerla(ahora) && !mias.contains(&e.nombre) && suyas.contains(&e.nombre)
        })
        .cloned()
        .collect()
}

/// Las que tengo y al otro le faltan: hay que mandarselas.
pub fn que_mandar(
    entradas: &[Entrada],
    mias: &BTreeSet<String>,
    suyas: &BTreeSet<String>,
    ahora: i64,
) -> Vec<Entrada> {
    entradas
        .iter()
        .filter(|e| {
            e.hay_que_tenerla(ahora) && mias.contains(&e.nombre) && !suyas.contains(&e.nombre)
        })
        .cloned()
        .collect()
}

/// El nombre que viene del otro, comprobado (`nombreValido`): nada de
/// rutas, ni vacio, ni `.` o `..`. Lo que el saneado cambiaria tampoco vale:
/// no es un nombre que este aparato hubiera escrito.
pub fn nombre_valido(n: Option<&str>) -> Result<String, String> {
    match n {
        Some(n)
            if !n.trim().is_empty()
                && n == crate::envio::nombre_sano(n)
                && n != "."
                && n != ".." =>
        {
            Ok(n.to_string())
        }
        _ => Err("Nombre de captura no válido".into()),
    }
}

// ------------------------------------------------------------ fichero

/// Las entradas como viajan: una cadena con JSON dentro.
pub fn entradas_a_texto(l: &[Entrada]) -> String {
    serde_json::to_string(l).unwrap_or_else(|_| "[]".into())
}

pub fn entradas_de_texto(t: &str) -> serde_json::Result<Vec<Entrada>> {
    serde_json::from_str(t)
}

/// Leer, cambiar y escribir el estado, de uno en uno: lo tocan la vuelta
/// que dirige y la que responde, cada una en su hilo.
static CERROJO: Mutex<()> = Mutex::new(());

fn ruta_del_estado(raiz: &Path) -> PathBuf {
    raiz.join(FICHERO)
}

fn leer_sin_cerrojo(raiz: &Path) -> Estado {
    let ruta = ruta_del_estado(raiz);
    let Ok(texto) = std::fs::read_to_string(&ruta) else {
        return Estado::default();
    };
    match serde_json::from_str(&texto) {
        Ok(e) => e,
        Err(_) => {
            // No se entiende: se aparta y se empieza de nuevo (lo vivo
            // vuelve a apuntarse solo).
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let _ = std::fs::rename(&ruta, raiz.join(format!("{FICHERO}.roto-{ms}")));
            Estado::default()
        }
    }
}

/// El estado de `raiz`; vacio si aun no hay.
pub fn leer(raiz: &Path) -> Estado {
    let _c = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    leer_sin_cerrojo(raiz)
}

/// Cambia el estado con el cerrojo cogido y lo escribe.
pub fn cambiar(raiz: &Path, f: impl FnOnce(Estado) -> Estado) -> io::Result<Estado> {
    let _c = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    let nuevo = f(leer_sin_cerrojo(raiz));
    std::fs::create_dir_all(raiz)?;
    let ruta = ruta_del_estado(raiz);
    let tmp = raiz.join(format!("{FICHERO}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec(&nuevo).map_err(io::Error::other)?)?;
    std::fs::rename(&tmp, &ruta)?;
    Ok(nuevo)
}

// ------------------------------------------------------------ el aparato

/// **Las capturas de este aparato**, para que la galeria viaje sin que la
/// sincronizacion sepa donde viven (`CapturasDelAparato` de Android): en el
/// PC, la carpeta `capturas/` de los datos; en las pruebas, una cualquiera.
pub trait CapturasDelAparato {
    /// Donde vive `galeria-compartida.json` (la raiz de los datos).
    fn raiz(&self) -> PathBuf;
    /// Las que hay ahora. `None` si no se pudo mirar (no es «ninguna»).
    fn listar(&self) -> Option<Vec<Local>>;
    /// El fichero de una, si esta.
    fn abrir(&self, nombre: &str) -> Option<PathBuf>;
    /// Guarda una que llega, con lo que dice su entrada (su fecha es
    /// `cuando`). `escribir` vuelca los trozos y dice si llego entera.
    /// Devuelve si quedo entera; sin entera, no queda nada.
    fn guardar(
        &self,
        e: &Entrada,
        escribir: &mut dyn FnMut(&mut dyn Write) -> Resultado<bool>,
    ) -> Resultado<bool>;
    /// A la papelera.
    fn tirar(&self, nombres: &[String]);
    /// «Dias hasta borrar» de aqui (0 = nunca).
    fn dias(&self) -> i64;
    /// El registro de caducidad de aqui.
    fn caducidad(&self, ahora: i64) -> Caducidad;
    /// Cambia el registro de caducidad y lo escribe.
    fn cambiar_caducidad(&self, ahora: i64, f: &mut dyn FnMut(&mut Caducidad)) -> io::Result<()>;
    /// Algo de la galeria cambio (para refrescar lo que este abierto).
    fn avisar(&self) {}
    /// El texto de una captura que ya esta aqui, llegado de otro aparato:
    /// a donde lo busca la galeria (en el PC, la cache del OCR). Las que
    /// llegan enteras traen el suyo en la entrada de [`guardar`](Self::guardar).
    fn apuntar_texto(&self, _nombre: &str, _texto: &str) {}
}

/// **Lo de aqui, al dia y escrito** (`ponerAlDia`), con el registro de
/// caducidad ya con lo acordado.
pub fn poner_al_dia(c: &dyn CapturasDelAparato, aparato: &str, ahora: i64) -> io::Result<Estado> {
    let locales = c.listar();
    let dias = c.dias();
    let r = c.caducidad(ahora);
    let e = cambiar(&c.raiz(), |viejo| {
        al_dia(&viejo, locales.as_deref(), &r, dias, ahora, aparato)
    })?;
    c.cambiar_caducidad(ahora, &mut |r| *r = aplicar(r, &e.entradas))?;
    Ok(e)
}

/// **Lo juntado, aqui** (`aplicarJuntas`): se guardan las entradas, se
/// apunta lo acordado en el registro y lo quitado en otro aparato va a la
/// papelera. Devuelve cuantas se tiraron.
pub fn aplicar_juntas(
    c: &dyn CapturasDelAparato,
    juntas: &[Entrada],
    ahora: i64,
) -> io::Result<usize> {
    let locales = c.listar().unwrap_or_default();
    let aqui: BTreeSet<String> = locales.iter().map(|x| x.nombre.clone()).collect();
    let tirar = a_tirar(juntas, &aqui);
    if !tirar.is_empty() {
        c.tirar(&tirar);
    }
    // Lo juntado por quien no sabe de textos (Android) llega sin ellos: se
    // completan con lo que ya habia aqui, en el estado o leido de la
    // captura, para no olvidarlos.
    let texto_local: HashMap<&str, &str> = locales
        .iter()
        .filter(|l| !l.texto.is_empty())
        .map(|l| (l.nombre.as_str(), l.texto.as_str()))
        .collect();
    let mut juntas = juntas.to_vec();
    let e = cambiar(&c.raiz(), |e| {
        let texto_viejo: HashMap<&str, &str> = e
            .entradas
            .iter()
            .filter(|x| !x.texto.is_empty())
            .map(|x| (x.nombre.as_str(), x.texto.as_str()))
            .collect();
        for j in juntas.iter_mut().filter(|j| j.texto.is_empty()) {
            if let Some(t) = texto_viejo
                .get(j.nombre.as_str())
                .or_else(|| texto_local.get(j.nombre.as_str()))
            {
                j.texto = t.to_string();
            }
        }
        Estado {
            entradas: juntas.clone(),
            tenia: e.tenia.into_iter().filter(|n| !tirar.contains(n)).collect(),
        }
    })?;
    // Y al reves: el texto que trae el otro de una captura que ya esta aqui
    // pero aun no se leyo, a donde lo busca la galeria.
    for j in e
        .entradas
        .iter()
        .filter(|j| !j.borrada && !j.texto.is_empty())
    {
        if aqui.contains(&j.nombre) && !texto_local.contains_key(j.nombre.as_str()) {
            c.apuntar_texto(&j.nombre, &j.texto);
        }
    }
    c.cambiar_caducidad(ahora, &mut |r| *r = aplicar(r, &e.entradas))?;
    Ok(tirar.len())
}

/// Una que acaba de llegar (`apuntarQueLlego`): desde ahora es de las que
/// hay aqui (si se quita, se quita en todos).
pub fn apuntar_que_llego(c: &dyn CapturasDelAparato, nombre: &str) -> io::Result<()> {
    cambiar(&c.raiz(), |mut e| {
        e.tenia.insert(nombre.to_string());
        e
    })
    .map(|_| ())
}

#[cfg(test)]
mod pruebas;
