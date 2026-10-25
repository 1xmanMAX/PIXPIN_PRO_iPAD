//! **La galeria de capturas**: las capturas de `capturas/`, la mas nueva
//! primero, con la fecha en que se van, y lo que se hace con una:
//! conservar, «dar 7 dias mas» y borrar (con deshacer).
//!
//! No se inventa nada: es lo mismo que la galeria de Windows
//! (`galeria_capturas.rs` y `caducidad_capturas.rs` de la app), con los
//! mismos ficheros, para que un Mac sea un aparato mas del grupo:
//!
//! - las capturas viven en `<datos>/capturas/` con las mismas extensiones
//!   (`png`, `jpg`, `jpeg`, `bmp`, `gif`, `webp`, `mp4`);
//! - la caducidad es el registro `<datos>/capturas-caducidad.json`
//!   (`pixpin_sincro::galeria::Caducidad`, el de Windows y Android): se crea
//!   la primera vez con `desde = ahora`, se escribe por un temporal
//!   `capturas-caducidad.json.tmp` y uno que no se entiende se aparta como
//!   `capturas-caducidad.json.roto-<ms>`;
//! - cuando se va una captura lo dice `pixpin_sincro::galeria::se_va_el`
//!   (conservada, fecha acordada con el grupo, regla y prorroga), y «7 dias
//!   mas» es `pixpin_sincro::galeria::prorrogar`;
//! - **conservar** es mandarla a «Mensajes guardados» como una foto mas
//!   (el mismo camino que adjuntar) y apuntarla en `conservadas`;
//! - **borrar** la lleva a `<datos>/papelera/capturas/` (con la hora delante
//!   si ya habia una con ese nombre) y olvida si estaba conservada o
//!   prorrogada. Lo que la galeria que viaja hace despues (marcarla como
//!   borrada para los demas) lo decide al sincronizar con `tenia`, igual
//!   que en Windows: aqui solo se mueve el fichero.
//!
//! **Barrer** (lo que hace `caducidad_capturas::barrer` en Windows) va
//! partido en dos: aqui se decide que capturas ya tocan
//! ([`Nucleo::capturas_caducadas`], las mismas reglas que Windows) y Swift
//! las lleva a la **Papelera del Mac** con `FileManager.trashItem` (lo que
//! en Windows es la papelera de reciclaje: se restaura desde ahi). Despues,
//! [`Nucleo::olvidar_capturas_ausentes`] limpia del registro las
//! conservadas y prorrogadas cuyo fichero ya no esta, como Windows.
//!
//! **Buscar** (B6) es el buscador de la v2 de Windows: el nombre, la fecha
//! y el texto de dentro de la captura, que Swift lee con Vision en segundo
//! plano y apunta aqui ([`Nucleo::galeria_poner_texto`]) en
//! `cache/galeria/`, junto a donde Windows guarda el suyo.
//!
//! # Rendimiento
//!
//! La lista se lee **por paginas** ([`Nucleo::galeria`]). La carpeta se
//! relee solo si cambio su fecha de modificacion (como Windows): la lista
//! ordenada se guarda en una cache pequena (unos 60 bytes por captura) y se
//! suelta con [`Nucleo::vaciar_caches`].

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use pixpin_sincro::galeria::{self as reglas, Caducidad};

use crate::{ErrorNucleo, MensajeChat, Nucleo};

/// Las extensiones que se listan, las de Windows. El MP4 tambien: no tiene
/// miniatura, pero se abre y se borra como las demas.
pub const EXTENSIONES: [&str; 7] = ["png", "jpg", "jpeg", "bmp", "gif", "webp", "mp4"];

/// Lo que aguanta una captura antes de irse sola, de fabrica (Windows:
/// `caducidad_capturas::DIAS`). Lo que vale lo pasa Swift (su ajuste).
pub const DIAS_POR_OMISION: i64 = 7;

/// Lo mas que se devuelve en una pagina de la galeria.
pub const TOPE_DE_PAGINA_GALERIA: u32 = 500;

/// El fichero del registro de caducidad, en la raiz de los datos.
const FICHERO_CADUCIDAD: &str = "capturas-caducidad.json";

/// Leer, cambiar y escribir el registro de uno en uno (como el `CERROJO`
/// de `caducidad_capturas.rs`).
static CERROJO: Mutex<()> = Mutex::new(());

/// Una captura de la galeria.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CapturaGaleria {
    /// El nombre del fichero: la clave de la captura en todos los aparatos.
    pub nombre: String,
    /// Su ruta absoluta en este aparato.
    pub ruta: String,
    /// Cuando se hizo (su fecha de modificacion), en ms desde 1970.
    pub cuando: i64,
    pub bytes: u64,
    /// Cuando se va, en ms desde 1970. `None`: no se va (conservada, o con
    /// la caducidad apagada).
    pub se_va: Option<i64>,
    /// Los dias de calendario que le quedan (el numero rojo de su chapita):
    /// 0 es que se va hoy. `None` si no se va.
    pub dias_que_quedan: Option<i64>,
    pub conservada: bool,
    /// Un video (`mp4`): sin miniatura.
    pub es_video: bool,
}

/// Una pagina de la galeria, de la mas nueva a la mas vieja.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaGaleria {
    pub capturas: Vec<CapturaGaleria>,
    /// La posicion de la primera de la pagina (0 es la mas nueva).
    pub desde: u64,
    /// Cuantas capturas hay en total.
    pub total: u64,
}

/// Una captura llevada a la papelera de PixPin, con lo que hace falta para
/// devolverla tal cual estaba ([`Nucleo::devolver_capturas`]).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CapturaBorrada {
    pub nombre: String,
    /// Donde quedo, dentro de `papelera/capturas/`.
    pub destino: String,
    pub conservada: bool,
    /// La fecha de su «7 dias mas», si la tenia.
    pub prorroga: Option<i64>,
}

/// Una captura que ya caduco: Swift la lleva a la Papelera del Mac.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CapturaCaducada {
    pub nombre: String,
    /// Su ruta absoluta en este aparato.
    pub ruta: String,
    /// Cuando tocaba irse, en ms desde 1970.
    pub se_va: i64,
}

/// Una captura de la carpeta (el `Entrada` de Windows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entrada {
    pub(crate) nombre: String,
    pub(crate) cuando: i64,
    pub(crate) bytes: u64,
}

/// La lista ordenada de la ultima lectura, valida mientras la carpeta
/// tenga la misma fecha.
#[derive(Debug, Default)]
pub struct CacheGaleria {
    lista: Option<(Option<SystemTime>, Arc<Vec<Entrada>>)>,
    /// El texto leido de cada captura, ya plegado, por la ruta de su
    /// fichero en `cache/galeria/` (que cambia si cambia la captura).
    textos: std::collections::HashMap<String, String>,
}

impl CacheGaleria {
    pub(crate) fn vaciar(&mut self) {
        self.lista = None;
        self.textos.clear();
    }
}

/// Si un nombre es de los que lista la galeria (por su extension).
pub fn es_captura(nombre: &str) -> bool {
    Path::new(nombre)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONES.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

fn es_video(nombre: &str) -> bool {
    Path::new(nombre)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
}

fn ms_de(t: SystemTime) -> i64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Las capturas de `carpeta`, la mas nueva primero (a igual hora, por
/// nombre al reves), como `galeria_capturas::listar`. Una carpeta que no
/// existe es una lista vacia. Un fichero vacio es una captura escribiendose:
/// no sale.
pub(crate) fn listar(carpeta: &Path) -> Vec<Entrada> {
    let Ok(lista) = std::fs::read_dir(carpeta) else {
        return Vec::new();
    };
    let mut v: Vec<Entrada> = lista
        .flatten()
        .filter_map(|e| {
            let nombre = e.file_name().to_string_lossy().into_owned();
            let meta = e.metadata().ok()?;
            (meta.is_file() && meta.len() > 0 && es_captura(&nombre)).then(|| Entrada {
                cuando: meta.modified().map(ms_de).unwrap_or(0),
                bytes: meta.len(),
                nombre,
            })
        })
        .collect();
    v.sort_by(|a, b| {
        b.cuando
            .cmp(&a.cuando)
            .then_with(|| b.nombre.cmp(&a.nombre))
    });
    v
}

// ---------------------------------------------------------------- registro

fn ruta_del_registro(raiz: &Path) -> PathBuf {
    raiz.join(FICHERO_CADUCIDAD)
}

/// Lee el registro; si no esta (la primera vez) lo crea con `desde = ahora`
/// y lo escribe en el acto. Quien llama tiene el cerrojo.
fn leer_sin_cerrojo(raiz: &Path, ahora: i64) -> Caducidad {
    let ruta = ruta_del_registro(raiz);
    if let Ok(texto) = std::fs::read_to_string(&ruta)
        && let Ok(r) = serde_json::from_str::<Caducidad>(&texto)
    {
        return r;
    }
    let nuevo = Caducidad {
        desde: ahora,
        ..Default::default()
    };
    if ruta.exists() {
        // Estaba pero no se entiende: se aparta, no se pisa a ciegas.
        let _ = std::fs::rename(
            &ruta,
            raiz.join(format!("{FICHERO_CADUCIDAD}.roto-{ahora}")),
        );
    }
    let _ = escribir(raiz, &nuevo);
    nuevo
}

fn escribir(raiz: &Path, r: &Caducidad) -> std::io::Result<()> {
    let ruta = ruta_del_registro(raiz);
    let tmp = ruta.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(r).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(&tmp, &ruta)
}

/// El registro de caducidad (creandolo si no esta).
pub(crate) fn leer_registro(raiz: &Path, ahora: i64) -> Caducidad {
    let _c = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    leer_sin_cerrojo(raiz, ahora)
}

/// Cambia el registro con el cerrojo cogido y lo escribe.
pub(crate) fn cambiar_registro(
    raiz: &Path,
    ahora: i64,
    f: impl FnOnce(&mut Caducidad),
) -> std::io::Result<Caducidad> {
    let _c = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = leer_sin_cerrojo(raiz, ahora);
    f(&mut r);
    escribir(raiz, &r)?;
    Ok(r)
}

/// Los dias de calendario (en el huso de quien mira) que faltan de `ahora`
/// a `se_va`, nunca negativos (`logica::dias_que_quedan` de Windows).
pub fn dias_que_quedan(se_va: Option<i64>, ahora: i64, desfase_ms: i64) -> Option<i64> {
    const DIA: i64 = reglas::DIA_MS;
    se_va.map(|t| ((t + desfase_ms).div_euclid(DIA) - (ahora + desfase_ms).div_euclid(DIA)).max(0))
}

/// **Las que ya tocan** (`caducidad_capturas::caducadas` de Windows): las
/// de `lista` cuya fecha de irse (conservada, fijada con el grupo, regla y
/// prorroga, por `pixpin_sincro::galeria::se_va_el`) ya llego en `ahora`.
/// Con `dias` cero o menos no se va ninguna. Cada una con su fecha.
pub(crate) fn caducadas<'a>(
    registro: &Caducidad,
    lista: &'a [Entrada],
    ahora: i64,
    dias: i64,
) -> Vec<(&'a Entrada, i64)> {
    lista
        .iter()
        .filter_map(|e| {
            let t = reglas::se_va_el(registro, &e.nombre, e.cuando, dias)?;
            (t <= ahora).then_some((e, t))
        })
        .collect()
}

/// Con cuanto de antelacion se avisa de que una captura se va
/// (`caducidad_capturas::AVISAR_ANTES_MS` de Windows): un dia.
pub const AVISAR_ANTES_MS: i64 = reglas::DIA_MS;

/// **Las que se van en las proximas 24 h** (`caducidad_capturas::por_avisar`
/// de Windows), con su fecha de irse. Las ya caducadas no: se van en este
/// barrido. Las ya avisadas las quita quien avisa (Swift, una vez por
/// sesion, como Windows).
pub(crate) fn por_avisar<'a>(
    registro: &Caducidad,
    lista: &'a [Entrada],
    ahora: i64,
    dias: i64,
) -> Vec<(&'a Entrada, i64)> {
    lista
        .iter()
        .filter_map(|e| {
            let t = reglas::se_va_el(registro, &e.nombre, e.cuando, dias)?;
            (t > ahora && t <= ahora + AVISAR_ANTES_MS).then_some((e, t))
        })
        .collect()
}

/// Los nombres apuntados (conservadas y prorrogadas) cuyo fichero ya no
/// esta en `presentes`.
fn ausentes(registro: &Caducidad, presentes: &[Entrada]) -> Vec<String> {
    let hay = |n: &String| presentes.iter().any(|e| &e.nombre == n);
    registro
        .conservadas
        .iter()
        .chain(registro.prorrogadas.keys())
        .filter(|n| !hay(n))
        .cloned()
        .collect()
}

/// Lleva `ruta` a `papelera/capturas/`; si ya hay una con ese nombre, la
/// nueva lleva la hora delante (`galeria_capturas::a_la_papelera`).
pub(crate) fn a_la_papelera(raiz: &Path, ruta: &Path, ahora: i64) -> std::io::Result<PathBuf> {
    let dir = raiz.join("papelera").join("capturas");
    std::fs::create_dir_all(&dir)?;
    let nombre = ruta
        .file_name()
        .ok_or_else(|| std::io::Error::other("sin nombre"))?;
    let mut destino = dir.join(nombre);
    if destino.exists() {
        destino = dir.join(format!("{ahora}-{}", nombre.to_string_lossy()));
    }
    std::fs::rename(ruta, &destino)?;
    Ok(destino)
}

impl Nucleo {
    fn carpeta_de_capturas(&self) -> PathBuf {
        self.ruta().join("capturas")
    }

    /// La lista ordenada, de la cache si la carpeta no cambio.
    fn lista_de_capturas(&self) -> Arc<Vec<Entrada>> {
        let carpeta = self.carpeta_de_capturas();
        let fecha = std::fs::metadata(&carpeta)
            .ok()
            .and_then(|m| m.modified().ok());
        let mut cache = self.galeria_tomada();
        if let Some((f, lista)) = &cache.lista
            && fecha.is_some()
            && *f == fecha
        {
            return Arc::clone(lista);
        }
        let lista = Arc::new(listar(&carpeta));
        cache.lista = Some((fecha, Arc::clone(&lista)));
        lista
    }

    /// El fichero de una captura por su nombre, comprobado: un nombre con
    /// rutas no puede salir de `capturas/`.
    fn fichero_de_captura(&self, nombre: &str) -> Result<PathBuf, ErrorNucleo> {
        let nombre = reglas::nombre_valido(Some(nombre))
            .map_err(|detalle| ErrorNucleo::Argumento { detalle })?;
        if !es_captura(&nombre) {
            return Err(ErrorNucleo::Argumento {
                detalle: format!("no es una captura: {nombre:?}"),
            });
        }
        let ruta = self.carpeta_de_capturas().join(&nombre);
        if !ruta.is_file() {
            return Err(ErrorNucleo::NoExiste { que: nombre });
        }
        Ok(ruta)
    }

    /// [`Nucleo::capturas_caducadas`] a una hora dada (las pruebas).
    pub(crate) fn caducadas_en(&self, dias: i64, ahora: i64) -> Vec<CapturaCaducada> {
        let lista = self.lista_de_capturas();
        let registro = leer_registro(self.ruta(), ahora);
        let carpeta = self.carpeta_de_capturas();
        caducadas(&registro, &lista, ahora, dias)
            .into_iter()
            .map(|(e, se_va)| CapturaCaducada {
                nombre: e.nombre.clone(),
                ruta: carpeta.join(&e.nombre).display().to_string(),
                se_va,
            })
            .collect()
    }

    /// [`Nucleo::capturas_por_avisar`] a una hora dada (las pruebas).
    pub(crate) fn por_avisar_en(&self, dias: i64, ahora: i64) -> Vec<CapturaCaducada> {
        let lista = self.lista_de_capturas();
        let registro = leer_registro(self.ruta(), ahora);
        let carpeta = self.carpeta_de_capturas();
        por_avisar(&registro, &lista, ahora, dias)
            .into_iter()
            .map(|(e, se_va)| CapturaCaducada {
                nombre: e.nombre.clone(),
                ruta: carpeta.join(&e.nombre).display().to_string(),
                se_va,
            })
            .collect()
    }

    fn cuando_de_captura(ruta: &Path) -> i64 {
        std::fs::metadata(ruta)
            .and_then(|m| m.modified())
            .map(ms_de)
            .unwrap_or(0)
    }
}

#[uniffi::export]
impl Nucleo {
    /// La carpeta de las capturas (`<datos>/capturas`), donde Swift guarda
    /// las nuevas y vigila con FSEvents.
    pub fn carpeta_capturas(&self) -> String {
        self.carpeta_de_capturas().display().to_string()
    }

    /// **Una pagina de la galeria**, de la mas nueva a la mas vieja: `desde`
    /// es la posicion de la primera (0 la mas nueva) y `cuantos` el largo
    /// (hasta [`TOPE_DE_PAGINA_GALERIA`]). `dias` es el plazo del ajuste
    /// («dias hasta borrar», 0 = nunca) y `desfase_ms` el del huso de quien
    /// mira (para contar los dias que quedan por dias de calendario).
    pub fn galeria(
        &self,
        desde: u64,
        cuantos: u32,
        dias: i64,
        desfase_ms: i64,
    ) -> Result<PaginaGaleria, ErrorNucleo> {
        let lista = self.lista_de_capturas();
        let ahora = crate::ahora_ms();
        let registro = leer_registro(self.ruta(), ahora);
        let total = lista.len();
        let desde = (desde.min(total as u64)) as usize;
        let hasta = (desde + cuantos.min(TOPE_DE_PAGINA_GALERIA) as usize).min(total);
        let capturas = lista[desde..hasta]
            .iter()
            .map(|e| self.ficha_de_captura(e, &registro, dias, ahora, desfase_ms))
            .collect();
        Ok(PaginaGaleria {
            capturas,
            desde: desde as u64,
            total: total as u64,
        })
    }

    /// **Conserva una captura**: entra en «Mensajes guardados» como una foto
    /// mas (se copia, como al adjuntar; de ahi viaja al movil) y se apunta
    /// en `conservadas` para que no caduque. Devuelve el mensaje nuevo. Una
    /// ya conservada da error de argumento («ya estaba conservada»), como el
    /// aviso de Windows: no se manda dos veces al chat.
    pub fn conservar_captura(&self, nombre: String) -> Result<MensajeChat, ErrorNucleo> {
        let ruta = self.fichero_de_captura(&nombre)?;
        let ahora = crate::ahora_ms();
        if leer_registro(self.ruta(), ahora)
            .conservadas
            .contains(&nombre)
        {
            return Err(ErrorNucleo::Argumento {
                detalle: format!("ya estaba conservada: {nombre}"),
            });
        }
        let guardados = self.chat_guardados()?;
        let m = self.adjuntar(
            guardados.id,
            ruta.display().to_string(),
            Some(nombre.clone()),
        )?;
        cambiar_registro(self.ruta(), ahora, |r| {
            r.conservadas.insert(nombre);
        })?;
        Ok(m)
    }

    /// **«Dar 7 dias mas»**: siete dias despues de la fecha que tenia (o de
    /// ahora, si ya habia pasado). Con fecha acordada con el grupo se mueve
    /// esa; si no, va a `prorrogadas`. Devuelve la fecha nueva, o `None` si
    /// no se va (conservada o caducidad apagada).
    pub fn prorrogar_captura(&self, nombre: String, dias: i64) -> Result<Option<i64>, ErrorNucleo> {
        let ruta = self.fichero_de_captura(&nombre)?;
        let cuando = Nucleo::cuando_de_captura(&ruta);
        let ahora = crate::ahora_ms();
        let r = cambiar_registro(self.ruta(), ahora, |r| {
            reglas::prorrogar(r, &nombre, cuando, ahora, dias);
        })?;
        Ok(reglas::se_va_el(&r, &nombre, cuando, dias))
    }

    /// **Borra capturas**: a la papelera de PixPin (`papelera/capturas/`),
    /// sin preguntar (se deshace con [`Nucleo::devolver_capturas`]), y se
    /// olvida si estaban conservadas o prorrogadas (su nombre queda libre).
    /// Lo que se pudo mover se devuelve aunque alguna fallara; si fallo
    /// alguna y no se movio ninguna, es un error.
    pub fn borrar_capturas(
        &self,
        nombres: Vec<String>,
    ) -> Result<Vec<CapturaBorrada>, ErrorNucleo> {
        let ahora = crate::ahora_ms();
        let antes = leer_registro(self.ruta(), ahora);
        let mut borradas = Vec::new();
        let mut fallo = None;
        for nombre in nombres {
            let ruta = match self.fichero_de_captura(&nombre) {
                Ok(r) => r,
                Err(e) => {
                    fallo = Some(e);
                    continue;
                }
            };
            match a_la_papelera(self.ruta(), &ruta, ahora) {
                Ok(destino) => borradas.push(CapturaBorrada {
                    conservada: antes.conservadas.contains(&nombre),
                    prorroga: antes.prorrogadas.get(&nombre).copied(),
                    destino: destino.display().to_string(),
                    nombre,
                }),
                Err(e) => fallo = Some(e.into()),
            }
        }
        if borradas
            .iter()
            .any(|b| b.conservada || b.prorroga.is_some())
        {
            cambiar_registro(self.ruta(), ahora, |r| {
                for b in &borradas {
                    r.conservadas.remove(&b.nombre);
                    r.prorrogadas.remove(&b.nombre);
                }
            })?;
        }
        self.galeria_tomada().vaciar();
        match fallo {
            Some(e) if borradas.is_empty() => Err(e),
            _ => Ok(borradas),
        }
    }

    /// **Las capturas que ya caducaron** con el plazo `dias` (el ajuste
    /// `[capturas] dias_caducidad`; 0 = nunca): las mismas reglas que el
    /// barrendero de Windows. No mueve nada: Swift las lleva a la Papelera
    /// del Mac (`FileManager.trashItem`) y luego llama a
    /// [`Nucleo::olvidar_capturas_ausentes`]. La carpeta se lee de la cache
    /// si no cambio.
    pub fn capturas_caducadas(&self, dias: i64) -> Vec<CapturaCaducada> {
        self.caducadas_en(dias, crate::ahora_ms())
    }

    /// **Las capturas que se van en las proximas 24 h** con el plazo `dias`
    /// (0 = nunca), con la fecha en que se van: el aviso «se van en 24 h»
    /// de Windows (`caducidad_capturas::vigilar_avisos`). Las ya caducadas
    /// no salen (las barre [`Nucleo::capturas_caducadas`]); las conservadas
    /// tampoco. Cada una se avisa una vez por sesion: eso lo lleva Swift.
    pub fn capturas_por_avisar(&self, dias: i64) -> Vec<CapturaCaducada> {
        self.por_avisar_en(dias, crate::ahora_ms())
    }

    /// **Tras barrer**: olvida las conservadas y prorrogadas cuyo fichero
    /// ya no esta (el numero de `captura-NNNN` se reutiliza: una nueva con
    /// el nombre de una conservada que se fue no tiene por que quedarse
    /// para siempre), como el final de `caducidad_capturas::barrer`. Suelta
    /// la lista de la cache. Devuelve cuantos nombres se olvidaron; si no
    /// sobra ninguno, no se escribe el registro.
    pub fn olvidar_capturas_ausentes(&self) -> Result<u32, ErrorNucleo> {
        self.galeria_tomada().vaciar();
        let lista = self.lista_de_capturas();
        let ahora = crate::ahora_ms();
        let sobran = ausentes(&leer_registro(self.ruta(), ahora), &lista);
        if sobran.is_empty() {
            return Ok(0);
        }
        cambiar_registro(self.ruta(), ahora, |r| {
            for n in &sobran {
                r.conservadas.remove(n);
                r.prorrogadas.remove(n);
            }
        })?;
        Ok(sobran.len() as u32)
    }

    /// **Deshace un borrado**: cada una vuelve a `capturas/` (si su nombre
    /// sigue libre) con lo que tenia apuntado. Devuelve cuantas volvieron.
    pub fn devolver_capturas(&self, borradas: Vec<CapturaBorrada>) -> Result<u32, ErrorNucleo> {
        let carpeta = self.carpeta_de_capturas();
        let papelera = self.ruta().join("papelera").join("capturas");
        let mut vueltas: Vec<&CapturaBorrada> = Vec::new();
        for b in &borradas {
            let Ok(nombre) = reglas::nombre_valido(Some(&b.nombre)) else {
                continue;
            };
            let destino = PathBuf::from(&b.destino);
            // Solo lo que esta en la papelera de capturas: el destino viene
            // de Swift y no puede mover un fichero cualquiera.
            if destino.parent() != Some(papelera.as_path()) {
                continue;
            }
            let original = carpeta.join(&nombre);
            if original.exists() {
                continue;
            }
            std::fs::create_dir_all(&carpeta)?;
            if std::fs::rename(&destino, &original).is_ok() {
                vueltas.push(b);
            }
        }
        if vueltas.iter().any(|b| b.conservada || b.prorroga.is_some()) {
            cambiar_registro(self.ruta(), crate::ahora_ms(), |r| {
                for b in &vueltas {
                    if b.conservada {
                        r.conservadas.insert(b.nombre.clone());
                    }
                    if let Some(t) = b.prorroga {
                        r.prorrogadas.insert(b.nombre.clone(), t);
                    }
                }
            })?;
        }
        self.galeria_tomada().vaciar();
        Ok(vueltas.len() as u32)
    }
}

// ------------------------------------------------------------- buscar (B6)

/// Donde se guarda el texto ya leido de una captura (el OCR de Vision):
/// `<datos>/cache/galeria/<nombre>-<segundos>-<bytes>.txt`, el mismo nombre
/// que la miniatura de Windows (`ruta_en_cache`): si la captura cambia,
/// cambia el nombre y se vuelve a leer.
pub(crate) fn ruta_del_texto(raiz: &Path, e: &Entrada) -> PathBuf {
    let base = Path::new(&e.nombre)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    raiz.join("cache")
        .join("galeria")
        .join(format!("{base}-{}-{}.txt", e.cuando.div_euclid(1000), e.bytes))
}

/// El texto ya leido de la captura `ruta` (vacio si no se leyo): lo que
/// viaja en su entrada de la galeria (`Entrada::texto` de la sincro).
pub(crate) fn texto_guardado(raiz: &Path, ruta: &Path) -> String {
    entrada_de_fichero(ruta)
        .and_then(|e| std::fs::read_to_string(ruta_del_texto(raiz, &e)).ok())
        .unwrap_or_default()
}

/// Apunta el texto de la captura `ruta`, llegado de otro aparato, donde lo
/// busca la galeria (como si lo hubiera leido Vision aqui).
pub(crate) fn guardar_texto(raiz: &Path, ruta: &Path, texto: &str) -> std::io::Result<PathBuf> {
    let Some(e) = entrada_de_fichero(ruta) else {
        return Err(std::io::Error::other("sin la captura"));
    };
    let destino = ruta_del_texto(raiz, &e);
    if let Some(p) = destino.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(&destino, texto)?;
    Ok(destino)
}

fn entrada_de_fichero(ruta: &Path) -> Option<Entrada> {
    let meta = std::fs::metadata(ruta).ok()?;
    Some(Entrada {
        cuando: meta.modified().map(ms_de).unwrap_or(0),
        bytes: meta.len(),
        nombre: ruta.file_name()?.to_string_lossy().into_owned(),
    })
}

/// En minusculas y sin tildes, como `logica::plegar` de Windows.
pub fn plegar(s: &str) -> String {
    pixpin_timeline::buscar::normalizar(s)
}

/// Si TODAS las palabras de la consulta (ya plegada) estan en alguno de los
/// campos (ya plegados). Vacia encaja con todo (`logica::encaja`).
pub fn encaja(consulta_plegada: &str, campos: &[&str]) -> bool {
    consulta_plegada
        .split_whitespace()
        .all(|p| campos.iter().any(|c| c.contains(p)))
}

/// Lo que se busca de la fecha de una captura, plegado: «viernes 3 de
/// octubre de 2026 3 oct», y «hoy» o «ayer» si toca.
fn fecha_para_buscar(cuando: i64, ahora: i64, desfase_ms: i64) -> String {
    use pixpin_timeline::dias::{self, Dia};
    let d = Dia::de_instante(cuando, desfase_ms);
    let hoy = Dia::de_instante(ahora, desfase_ms);
    let cerca = match hoy.numero() - d.numero() {
        0 => " hoy",
        1 => " ayer",
        _ => "",
    };
    plegar(&format!(
        "{} {} {}{cerca}",
        dias::nombre_del_dia(d, false),
        d.dia,
        pixpin_timeline::estilo::mes_corto(d.mes, false)
    ))
}

impl Nucleo {
    /// El texto leido de una captura tal cual (con sus mayusculas y
    /// tildes), para ensenar el trozo donde esta lo buscado en «Buscar en
    /// todo». Vacio si aun no se leyo.
    pub(crate) fn texto_leido_de_captura(&self, c: &CapturaGaleria) -> String {
        let e = Entrada {
            nombre: c.nombre.clone(),
            cuando: c.cuando,
            bytes: c.bytes,
        };
        std::fs::read_to_string(ruta_del_texto(self.ruta(), &e)).unwrap_or_default()
    }

    /// El texto leido de una captura, plegado (de la cache de memoria, o del
    /// disco la primera vez). Vacio si aun no se leyo.
    fn texto_de_captura(&self, e: &Entrada) -> String {
        let ruta = ruta_del_texto(self.ruta(), e);
        let clave = ruta.display().to_string();
        if let Some(t) = self.galeria_tomada().textos.get(&clave) {
            return t.clone();
        }
        let t = std::fs::read_to_string(&ruta)
            .map(|t| plegar(&t))
            .unwrap_or_default();
        if ruta.exists() {
            self.galeria_tomada().textos.insert(clave, t.clone());
        }
        t
    }

    fn ficha_de_captura(
        &self,
        e: &Entrada,
        registro: &Caducidad,
        dias: i64,
        ahora: i64,
        desfase_ms: i64,
    ) -> CapturaGaleria {
        let se_va = reglas::se_va_el(registro, &e.nombre, e.cuando, dias);
        CapturaGaleria {
            ruta: self
                .carpeta_de_capturas()
                .join(&e.nombre)
                .display()
                .to_string(),
            cuando: e.cuando,
            bytes: e.bytes,
            se_va,
            dias_que_quedan: dias_que_quedan(se_va, ahora, desfase_ms),
            conservada: registro.conservadas.contains(&e.nombre),
            es_video: es_video(&e.nombre),
            nombre: e.nombre.clone(),
        }
    }
}

#[uniffi::export]
impl Nucleo {
    /// **El buscador de la galeria** (`coincide` de Windows): las capturas
    /// donde esta cada palabra de `consulta`, sin mayusculas ni tildes, en
    /// su nombre, su fecha («viernes», «3 oct», «hoy») o el texto de dentro
    /// ya leido ([`Nucleo::galeria_poner_texto`]). De la mas nueva a la mas
    /// vieja, por paginas como [`Nucleo::galeria`]; `total` es cuantas
    /// encajan.
    pub fn galeria_buscar(
        &self,
        consulta: String,
        desde: u64,
        cuantos: u32,
        dias: i64,
        desfase_ms: i64,
    ) -> Result<PaginaGaleria, ErrorNucleo> {
        let q = plegar(consulta.trim());
        let lista = self.lista_de_capturas();
        let ahora = crate::ahora_ms();
        let encajan: Vec<&Entrada> = lista
            .iter()
            .filter(|e| {
                q.is_empty() || {
                    let nombre = plegar(&e.nombre);
                    let fecha = fecha_para_buscar(e.cuando, ahora, desfase_ms);
                    // El texto solo se mira si el nombre y la fecha no bastan.
                    encaja(&q, &[&nombre, &fecha]) || {
                        let dentro = self.texto_de_captura(e);
                        encaja(&q, &[&nombre, &fecha, &dentro])
                    }
                }
            })
            .collect();
        let registro = leer_registro(self.ruta(), ahora);
        let total = encajan.len();
        let desde = (desde.min(total as u64)) as usize;
        let hasta = (desde + cuantos.min(TOPE_DE_PAGINA_GALERIA) as usize).min(total);
        Ok(PaginaGaleria {
            capturas: encajan[desde..hasta]
                .iter()
                .map(|e| self.ficha_de_captura(e, &registro, dias, ahora, desfase_ms))
                .collect(),
            desde: desde as u64,
            total: total as u64,
        })
    }

    /// **Las capturas cuyo texto aun no se leyo** (las imagenes, no los
    /// videos), de la mas nueva a la mas vieja, como mucho `cuantos`: Swift
    /// las pasa por Vision en segundo plano y apunta lo leido con
    /// [`Nucleo::galeria_poner_texto`].
    pub fn galeria_sin_texto(&self, cuantos: u32) -> Vec<CapturaGaleria> {
        let lista = self.lista_de_capturas();
        let ahora = crate::ahora_ms();
        let registro = leer_registro(self.ruta(), ahora);
        lista
            .iter()
            .filter(|e| !es_video(&e.nombre) && !ruta_del_texto(self.ruta(), e).exists())
            .take(cuantos as usize)
            .map(|e| self.ficha_de_captura(e, &registro, 0, ahora, 0))
            .collect()
    }

    /// Apunta el texto leido de una captura (vacio tambien vale: «no tiene
    /// texto», y no se vuelve a leer). Una captura que ya no esta no es un
    /// error: no se apunta nada.
    pub fn galeria_poner_texto(&self, nombre: String, texto: String) -> Result<(), ErrorNucleo> {
        let Ok(ruta) = self.fichero_de_captura(&nombre) else {
            return Ok(());
        };
        if !ruta.is_file() {
            return Ok(());
        }
        let destino = guardar_texto(self.ruta(), &ruta, &texto)?;
        self.galeria_tomada()
            .textos
            .insert(destino.display().to_string(), plegar(&texto));
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const DIA: i64 = reglas::DIA_MS;

    #[test]
    fn los_dias_que_quedan_son_de_calendario_y_nunca_negativos() {
        // Se va manana a las 00:30 y son las 23:00 de hoy: queda 1 dia.
        assert_eq!(
            dias_que_quedan(Some(10 * DIA + DIA / 48), 10 * DIA - DIA / 24, 0),
            Some(1)
        );
        assert_eq!(dias_que_quedan(Some(10 * DIA), 10 * DIA, 0), Some(0));
        // Caso negativo: ya paso y no sale -3.
        assert_eq!(dias_que_quedan(Some(7 * DIA), 10 * DIA, 0), Some(0));
        assert_eq!(dias_que_quedan(None, 0, 0), None);
        // Con el huso de Lima (-5 h) el dia cambia a otra hora.
        let lima = -5 * 3_600_000;
        assert_eq!(
            dias_que_quedan(Some(11 * DIA + 3_600_000), 10 * DIA + 3_600_000, lima),
            Some(1)
        );
    }

    fn entrada(nombre: &str, cuando: i64) -> Entrada {
        Entrada {
            nombre: nombre.into(),
            cuando,
            bytes: 1,
        }
    }

    #[test]
    fn se_avisa_de_las_que_se_van_en_un_dia() {
        // `se_avisa_una_vez_de_las_que_se_van_en_un_dia` de Windows (lo de
        // «una vez» lo lleva Swift).
        let r = Caducidad {
            desde: 0,
            ..Default::default()
        };
        let ahora = 100 * DIA;
        let lista = [
            // Se va en 12 h: se avisa.
            entrada("pronto.png", ahora - 7 * DIA + DIA / 2),
            // Se va en tres dias: todavia no.
            entrada("luego.png", ahora - 7 * DIA + 3 * DIA),
            // Ya caducada: la barre el barrendero, no se avisa.
            entrada("ida.png", ahora - 7 * DIA - 1),
        ];
        let v = por_avisar(&r, &lista, ahora, 7);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0.nombre, "pronto.png");
        assert_eq!(v[0].1, ahora + DIA / 2);
        // Caso negativo: conservada, no se va; con el ajuste a cero, nada.
        let mut c = r.clone();
        c.conservadas.insert("pronto.png".into());
        assert!(por_avisar(&c, &lista, ahora, 7).is_empty());
        assert!(por_avisar(&r, &lista, ahora, 0).is_empty());
    }

    #[test]
    fn caducan_las_que_tocan_como_en_windows() {
        let mut r = Caducidad {
            desde: 0,
            ..Default::default()
        };
        r.conservadas.insert("guardada.png".into());
        r.prorrogadas.insert("alargada.png".into(), 30 * DIA);
        let lista = [
            entrada("vieja.png", 10 * DIA),
            entrada("guardada.png", 10 * DIA),
            entrada("alargada.png", 10 * DIA),
            entrada("nueva.png", 19 * DIA),
        ];
        // Dia 20 con 7 dias: solo la vieja (se iba el 17).
        let idas = caducadas(&r, &lista, 20 * DIA, 7);
        assert_eq!(idas.len(), 1);
        assert_eq!(idas[0].0.nombre, "vieja.png");
        assert_eq!(idas[0].1, 17 * DIA);
        // Justo un ms antes de tocar, ninguna.
        assert!(caducadas(&r, &lista, 17 * DIA - 1, 7).is_empty());
        // Caso negativo: con el ajuste a cero no se va ninguna.
        assert!(caducadas(&r, &lista, 1000 * DIA, 0).is_empty());
        // La prorrogada aguanta hasta su fecha; la conservada, siempre.
        let todas: Vec<_> = caducadas(&r, &lista, 1000 * DIA, 7)
            .into_iter()
            .map(|(e, _)| e.nombre.as_str())
            .collect();
        assert_eq!(todas, ["vieja.png", "alargada.png", "nueva.png"]);
    }

    #[test]
    fn las_de_antes_del_estreno_no_se_van_de_golpe() {
        let r = Caducidad {
            desde: 100 * DIA,
            ..Default::default()
        };
        let lista = [entrada("antigua.png", 3 * DIA)];
        assert!(caducadas(&r, &lista, 100 * DIA, 7).is_empty());
        assert_eq!(caducadas(&r, &lista, 107 * DIA, 7).len(), 1);
    }

    #[test]
    fn barrer_en_disco_decide_y_luego_olvida_las_que_no_estan() {
        let raiz =
            std::env::temp_dir().join(format!("pixpin-nucleo-caducadas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let n = Nucleo::abrir(raiz.display().to_string(), "Mac".into()).unwrap();
        let dir = raiz.join("capturas");
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["vieja.png", "guardada.png", "notas.txt"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        let ahora = crate::ahora_ms();
        let hace_ocho = SystemTime::now() - std::time::Duration::from_secs(8 * 86_400);
        for f in ["vieja.png", "guardada.png"] {
            std::fs::File::options()
                .write(true)
                .open(dir.join(f))
                .unwrap()
                .set_modified(hace_ocho)
                .unwrap();
        }
        cambiar_registro(&raiz, ahora, |r| {
            r.desde = 0;
            r.conservadas.insert("guardada.png".into());
            r.conservadas.insert("ya-no-esta.png".into());
            r.prorrogadas.insert("tampoco.png".into(), ahora);
        })
        .unwrap();
        let idas = n.caducadas_en(7, ahora);
        assert_eq!(idas.len(), 1, "{idas:?}");
        assert_eq!(idas[0].nombre, "vieja.png");
        assert!(idas[0].ruta.ends_with("vieja.png"));
        assert!(idas[0].se_va <= ahora);
        // Nada se movio: eso lo hace Swift (la Papelera del Mac).
        assert!(dir.join("vieja.png").exists());
        // Caso negativo: con el ajuste a cero no se va ninguna.
        assert!(n.caducadas_en(0, ahora).is_empty());
        std::fs::remove_file(dir.join("vieja.png")).unwrap();
        assert_eq!(n.olvidar_capturas_ausentes().unwrap(), 2);
        let r = leer_registro(&raiz, ahora);
        assert!(r.conservadas.contains("guardada.png"));
        assert!(!r.conservadas.contains("ya-no-esta.png"));
        assert!(r.prorrogadas.is_empty());
        // Otra vez no sobra nada.
        assert_eq!(n.olvidar_capturas_ausentes().unwrap(), 0);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn solo_se_listan_las_extensiones_de_windows() {
        assert!(es_captura("captura-20261015-101010.png"));
        assert!(es_captura("a.JPG"));
        assert!(es_captura("v.mp4"));
        assert!(!es_captura("notas.txt"));
        assert!(!es_captura("sin-extension"));
    }
}
