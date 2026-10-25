//! Los proyectos que hay en este equipo: la lista de la ventana de chat.
//!
//! Un indice pequeno (`proyectos/indice.json`) con lo justo para pintar la
//! lista sin abrir nada: nombre, ultima linea, cuando se toco y los tres
//! codigos. El contenido de cada proyecto sigue en su `.pixpin`.
//!
//! Aqui vive tambien la regla de recibir de PixPin Android: **solo se pone
//! al dia un proyecto si coinciden sus tres codigos**; si no, entra como uno
//! nuevo con codigo unico nuevo. Ante la duda, duplicar y no pisar.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::codigos;

/// La ficha de un proyecto en la lista.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ficha {
    pub id: String,
    pub nombre: String,
    /// Los tres codigos (ver `codigos`).
    pub uid: Option<String>,
    pub creado: i64,
    pub aparato: Option<String>,
    /// Ultima vez que se toco, en milisegundos desde 1970. Ordena la lista.
    pub tocado: i64,
    /// Cuantas hojas tiene: lo que se ensena bajo el nombre mientras no
    /// haya mensajes. El texto se compone arriba, que es quien traduce.
    pub hojas: u32,
    /// La ultima linea, si ya hay uno.
    pub resumen: String,
    pub sin_leer: u32,
    /// El fichero del que salio, si vino de uno.
    pub paquete: Option<String>,
    /// La carpeta que eligio el usuario para este proyecto, si eligio una
    /// (`ubicacion`). La verdad es la union de `proyectos/<id>`; esto es para
    /// ensenarla y para rehacer la union si falta. `None` es la zona
    /// habitual, y no se escribe para que los indices de siempre no cambien.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ubicacion: Option<String>,
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

impl Ficha {
    /// Un proyecto creado AQUI, vacio y con sus tres codigos recien puestos.
    ///
    /// Se le pone `aparato` y `uid` propios, no los de nadie: es lo que hace
    /// que, cuando este proyecto llegue al movil, se reconozca como el mismo
    /// si vuelve, en vez de duplicarse en cada viaje (ver `recibir`).
    ///
    /// No crea la carpeta: eso lo hace quien escriba el primer mensaje, y
    /// asi un proyecto que se crea y se descarta no deja nada en el disco.
    pub fn nueva(nombre: &str, cuando: i64, aparato: &str) -> Ficha {
        Ficha {
            id: codigos::nuevo(),
            nombre: nombre.to_string(),
            uid: Some(codigos::nuevo()),
            creado: cuando,
            aparato: Some(aparato.to_string()),
            tocado: cuando,
            hojas: 0,
            resumen: String::new(),
            sin_leer: 0,
            paquete: None,
            ubicacion: None,
            resto: Default::default(),
        }
    }

    /// La ficha de un proyecto recien abierto, con sus tres codigos tal como
    /// vienen: son los que deciden si esto ya estaba aqui.
    pub fn de_proyecto(p: &crate::Proyecto, paquete: Option<&std::path::Path>) -> Ficha {
        Ficha {
            id: p.id.clone(),
            nombre: p.nombre.clone(),
            uid: p.uid.clone(),
            creado: p.creado,
            aparato: p.aparato.clone(),
            tocado: p.tocado,
            hojas: p.hojas.len() as u32,
            resumen: String::new(),
            sin_leer: 0,
            paquete: paquete.map(|r| r.display().to_string()),
            ubicacion: None,
            resto: Default::default(),
        }
    }

    pub fn codigo_unico(&self) -> String {
        codigos::unico(self.uid.as_deref(), "p:", &self.id)
    }

    /// Los tres codigos iguales: es la misma cosa, se puede poner al dia.
    pub fn misma_que(&self, otra: &Ficha) -> bool {
        self.codigo_unico() == otra.codigo_unico()
            && self.creado == otra.creado
            && self.aparato == otra.aparato
    }
}

/// Que paso al recibir un proyecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recibido {
    /// Ya estaba: se puso al dia el que habia.
    Actualizado,
    /// No estaba (o no es el mismo): entro como proyecto nuevo.
    Creado,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Indice {
    pub proyectos: Vec<Ficha>,
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

pub fn ruta(raiz: &Path) -> PathBuf {
    raiz.join("proyectos").join("indice.json")
}

/// La carpeta de un proyecto, donde vive su cuaderno (`guardados.jsonl`) y
/// lo que traiga consigo. Por `id` y no por codigo unico porque el codigo
/// puede cambiar al recibir, y una carpeta que se renombra sola pierde lo
/// que hubiera dentro.
pub fn carpeta(raiz: &Path, id: &str) -> PathBuf {
    raiz.join("proyectos").join(id)
}

/// El fichero de un lienzo del proyecto, con el mismo nombre que dentro del
/// `.pixpin` (`lienzos/<dibujo>.excalidraw`): asi un proyecto abierto y uno
/// empaquetado se leen igual.
pub fn lienzo(raiz: &Path, id: &str, dibujo: &str) -> PathBuf {
    carpeta(raiz, id)
        .join("lienzos")
        .join(format!("{dibujo}.excalidraw"))
}

/// Deja un nombre de fichero en algo que se pueda escribir en cualquier
/// disco: sin separadores, sin los signos que Windows prohibe y sin los
/// nombres reservados. Un nombre que venga de fuera no puede decidir donde
/// se escribe.
///
/// Y siempre en **NFC** («ñ» como un solo signo): el Finder del Mac entrega
/// los nombres en NFD («n» + «~»), Windows y Android en NFC, y la
/// sincronizacion compara rutas byte a byte. Sin esto, la misma foto
/// arrastrada en el Mac y en el PC serian dos ficheros distintos.
pub fn nombre_seguro(nombre: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    const PROHIBIDOS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let limpio: String = nombre
        .nfc()
        .map(|c| {
            if PROHIBIDOS.contains(&c) || (c as u32) < 0x20 {
                '_'
            } else {
                c
            }
        })
        .collect();
    // Windows tampoco admite que acabe en punto o espacio.
    let limpio = limpio.trim().trim_end_matches('.').trim();
    if limpio.is_empty() {
        return "archivo".to_string();
    }
    // Los nombres reservados de DOS siguen vivos y no se pueden crear.
    const RESERVADOS: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let raiz = limpio.split('.').next().unwrap_or(limpio);
    if RESERVADOS.iter().any(|r| raiz.eq_ignore_ascii_case(r)) {
        return format!("_{limpio}");
    }
    limpio.to_string()
}

/// Guarda un fichero dentro del proyecto y devuelve su ruta **relativa a la
/// carpeta del proyecto**, que es lo que se apunta en el mensaje.
///
/// Se copia en vez de apuntar al original a proposito: el original se mueve,
/// se renombra o se borra, y un proyecto que viaja no puede depender de una
/// ruta del escritorio de nadie.
///
/// Si ya hay uno con ese nombre se le pone un numero, en vez de pisarlo: dos
/// `captura.png` de dos sitios distintos son dos ficheros distintos.
pub fn guardar_adjunto(
    raiz: &Path,
    id: &str,
    nombre: &str,
    bytes: &[u8],
) -> std::io::Result<String> {
    let carpeta = carpeta(raiz, id).join("archivos");
    std::fs::create_dir_all(&carpeta)?;
    let seguro = nombre_seguro(nombre);
    let (tronco, extension) = match seguro.rsplit_once('.') {
        Some((t, e)) if !t.is_empty() => (t.to_string(), format!(".{e}")),
        _ => (seguro.clone(), String::new()),
    };
    let mut intento = seguro.clone();
    let mut n = 1;
    while carpeta.join(&intento).exists() {
        intento = format!("{tronco} ({n}){extension}");
        n += 1;
        if n > 9999 {
            return Err(std::io::Error::other("demasiados ficheros con ese nombre"));
        }
    }
    std::fs::write(carpeta.join(&intento), bytes)?;
    Ok(format!("archivos/{intento}"))
}

impl Indice {
    /// Lee el indice. **Solo un indice que NO EXISTE da la lista vacia**;
    /// un fichero que esta pero no se puede leer o no se entiende es un
    /// error, y quien va a reescribir la lista entera tiene que verlo.
    ///
    /// Confundir «no hay lista» con «no la pude leer» es perder proyectos:
    /// quien lee, anade una ficha y guarda, deja en el disco una lista de UNA
    /// ficha en lugar de las que habia, y el indice viejo ya no vuelve. Un
    /// disco ocupado, un permiso o un antivirus a media escritura bastan.
    pub fn leer_si_esta(raiz: &Path) -> std::io::Result<Indice> {
        let texto = match std::fs::read_to_string(ruta(raiz)) {
            Ok(t) => t,
            // Un almacen recien nacido: aqui la lista vacia es la verdad.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Indice::default()),
            Err(e) => return Err(e),
        };
        serde_json::from_str(&texto).map_err(std::io::Error::other)
    }

    /// Lo mismo, para quien solo va a ENSENAR la lista: un indice roto da la
    /// lista vacia y no impide abrir la ventana. Quien vaya a guardar encima
    /// tiene que usar `leer_si_esta` y parar si falla.
    pub fn leer(raiz: &Path) -> Indice {
        Indice::leer_si_esta(raiz).unwrap_or_default()
    }

    /// Temporal y renombrado, como el resto del almacen: un corte a mitad no
    /// deja la lista a medias.
    pub fn guardar(&self, raiz: &Path) -> std::io::Result<()> {
        let fichero = ruta(raiz);
        let carpeta = fichero.parent().unwrap_or(raiz);
        std::fs::create_dir_all(carpeta)?;
        let tmp = carpeta.join("indice.json.tmp");
        let texto = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, texto)?;
        std::fs::rename(&tmp, &fichero)
    }

    /// Las fichas en el orden de la lista: lo ultimo tocado arriba, y a
    /// igualdad de fecha, por nombre, para que no bailen entre arranques.
    pub fn ordenadas(&self) -> Vec<&Ficha> {
        let mut v: Vec<&Ficha> = self.proyectos.iter().collect();
        v.sort_by(|a, b| {
            // «Mensajes guardados» primero, pase lo que pase con las fechas:
            // es el unico chat que tiene que estar siempre en el mismo sitio.
            b.es_guardados()
                .cmp(&a.es_guardados())
                // Y el del asistente detras, tambien fijo (13-oct-2026).
                .then_with(|| b.es_asistente().cmp(&a.es_asistente()))
                .then_with(|| b.tocado.cmp(&a.tocado))
                .then_with(|| a.nombre.cmp(&b.nombre))
        });
        v
    }

    pub fn buscar(&self, id: &str) -> Option<&Ficha> {
        self.proyectos.iter().find(|f| f.id == id)
    }

    /// Lo de aqui que ES lo que llega: los tres codigos iguales.
    ///
    /// Es lo que hace falta ANTES de aceptar un envio, para poder preguntar
    /// «¿actualizar el que tengo o crear como nuevo?» como el movil: sin esto
    /// solo se sabria despues de haber escrito, que es tarde.
    pub fn misma(&self, llega: &Ficha) -> Option<&Ficha> {
        self.proyectos.iter().find(|f| f.misma_que(llega))
    }

    /// Mete lo que llega: pone al dia el que ya esta si coinciden los tres
    /// codigos, y si no lo anade como nuevo, con codigo unico nuevo para que
    /// los dos puedan convivir y volver a viajar sin pisarse.
    pub fn recibir(&mut self, llega: Ficha) -> (Recibido, String) {
        if let Some(sitio) = self.proyectos.iter().position(|f| f.misma_que(&llega)) {
            let id = self.proyectos[sitio].id.clone();
            let sin_leer = self.proyectos[sitio].sin_leer;
            // Donde lo guarda el usuario es cosa de este equipo: lo que llega
            // del movil no lo sabe, y perderlo dejaria la union sin su ruta.
            let ubicacion = self.proyectos[sitio].ubicacion.clone();
            // El id de aqui manda: lo de fuera puede venir con otro y hay
            // ficheros que ya lo usan.
            self.proyectos[sitio] = Ficha {
                id: id.clone(),
                sin_leer,
                ubicacion,
                ..llega
            };
            return (Recibido::Actualizado, id);
        }
        let mut nueva = llega;
        if self.proyectos.iter().any(|f| f.id == nueva.id) {
            // Mismo id pero otra cosa: se le da uno propio para no mezclar.
            nueva.id = format!("{}-{}", nueva.id, codigos::nuevo());
        }
        // El codigo que trae se respeta: es lo que hace que la proxima vez se
        // reconozca y se ponga al dia en vez de duplicarse. Solo se estrena
        // uno si aqui ya hay otro proyecto con ese mismo codigo, que si no
        // quedarian dos cosas distintas llamandose igual.
        let suyo = nueva.codigo_unico();
        if nueva.uid.is_none() || self.proyectos.iter().any(|f| f.codigo_unico() == suyo) {
            nueva.uid = Some(codigos::nuevo());
        }
        let id = nueva.id.clone();
        self.proyectos.push(nueva);
        (Recibido::Creado, id)
    }
}

/// Mete un `.pixpin` en el almacen como un proyecto mas de la lista.
///
/// Hasta ahora un paquete solo se abria como pines sueltos en la pantalla
/// (`Pines::abrir_paquete`), y lo que llegaba del movil no aparecia en
/// ninguna conversacion. Pero **cada chat ES un proyecto**: un `.pixpin` es
/// una conversacion entera, no un monton de hojas.
///
/// Se copia TODO lo que trae el paquete, tambien lo que aqui no se entiende
/// (croquis, imagenes de una version futura): abrir y volver a guardar no
/// puede perder nada.
///
/// Los tres codigos del proyecto se conservan (uid, creado y aparato): son
/// los que hacen que, cuando vuelva del movil, se reconozca como el mismo y
/// no se duplique. Si el paquete no los trae, se le ponen los de aqui.
pub fn importar_paquete(
    raiz: &Path,
    paquete: &crate::Paquete,
    aparato: &str,
) -> std::io::Result<Ficha> {
    use crate::cuaderno;
    let p = &paquete.proyecto;
    let cuando = if p.creado > 0 { p.creado } else { p.tocado };
    let mut ficha = Ficha::nueva(&p.nombre, cuando.max(1), aparato);
    // Los codigos del paquete mandan sobre los recien inventados.
    if let Some(uid) = p.uid.clone() {
        ficha.uid = Some(uid);
    }
    if let Some(a) = p.aparato.clone() {
        ficha.aparato = Some(a);
    }
    ficha.tocado = p.tocado.max(cuando);
    ficha.hojas = p.hojas.len() as u32;

    let destino = carpeta(raiz, &ficha.id);
    std::fs::create_dir_all(&destino)?;
    for nombre in paquete.nombres() {
        let Some(bytes) = paquete.entrada(nombre) else {
            continue;
        };
        // Un nombre con `..` dentro no puede escribir fuera de la carpeta
        // del proyecto: es un fichero que viene de otro aparato.
        if nombre.split(['/', '\\']).any(|t| t == "..") {
            continue;
        }
        let ruta = destino.join(nombre.replace('\\', "/"));
        if let Some(padre) = ruta.parent() {
            std::fs::create_dir_all(padre)?;
        }
        std::fs::write(&ruta, bytes)?;
    }

    // Una hoja por mensaje, para que la conversacion se vea. Si el paquete
    // ya trae su cuaderno, ese manda: es el del movil, con sus fechas y sus
    // codigos, y rehacerlo aqui seria inventar otra verdad.
    if paquete.entrada("guardados.jsonl").is_none() {
        // Las paginas del PDF no: como en el movil, del PDF el chat solo
        // lleva su mensaje (ver `paginas_fuera_del_chat`). Se ven en la
        // tarjeta de Proyectos, que las pide con `hojas_para_ensenar`.
        for (n, hoja) in p.hojas.iter().filter(|h| h.pagina.is_none()).enumerate() {
            let numero = n as i64 + 1;
            let sello = cuaderno::Sello {
                cuando: ficha.creado + numero,
                numero,
                aparato: ficha.aparato.clone().unwrap_or_else(|| aparato.to_string()),
                proyecto: ficha.id.clone(),
            };
            let mut mensaje = match (&hoja.dibujo, &hoja.nota) {
                (Some(dibujo), _) => {
                    let nombre = format!("{dibujo}.excalidraw");
                    let bytes = paquete
                        .entrada(&format!("lienzos/{nombre}"))
                        .map(|b| b.len())
                        .unwrap_or(0);
                    let mut m = cuaderno::Mensaje::adjunto(
                        cuaderno::Clase::Dibujo,
                        if hoja.nombre.is_empty() {
                            &nombre
                        } else {
                            &hoja.nombre
                        },
                        &format!("lienzos/{nombre}"),
                        bytes as i64,
                        &sello,
                    );
                    m.referencia = Some(dibujo.clone());
                    m
                }
                (None, Some(texto)) => cuaderno::Mensaje::nota(texto, &sello),
                // Una sin dibujo, nota ni pagina (un croquis) se apunta por
                // su nombre: mejor una linea que decir que esta que perderla
                // porque aqui no se sabe pintarla.
                (None, None) => cuaderno::Mensaje::nota(&hoja.nombre, &sello),
            };
            mensaje.uid = hoja.uid.clone();
            mensaje.pagina = hoja.pagina;
            // De quien cuelga: las «zonas» de una pagina son hojas hijas, y
            // sin esto se pierde de que pagina es cada recorte.
            mensaje.responde_a = hoja
                .resto
                .get("padre")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            cuaderno::anadir(&destino, &mensaje)?;
        }
    }

    // `leer_si_esta` y no `leer`: aqui se REESCRIBE la lista entera, y una
    // lista vacia por no haberla podido leer se llevaria por delante todos
    // los proyectos que ya habia.
    let mut indice = Indice::leer_si_esta(raiz)?;
    indice.proyectos.push(ficha.clone());
    indice.guardar(raiz)?;
    Ok(ficha)
}

/// **Crear como nuevo**: el mismo contenido con los tres codigos estrenados.
///
/// Puerto de `Recepcion.renovado` de Android. Quien recibe eligio que esto
/// que llega, aunque sea lo mismo que ya tiene, entre aparte: desde ahora es
/// otra cosa, y para que las dos puedan convivir y volver a viajar sin
/// pisarse necesita codigos propios. Si se quedara con los del original, el
/// siguiente envio de cualquiera de las dos pondria al dia a la otra.
///
/// Se renuevan: el codigo unico del proyecto, su fecha de creacion, el
/// aparato donde nacio (ninguno: nace aqui), el codigo unico de cada hoja y
/// los tres codigos de cada mensaje de su cuaderno. Los ficheros —lienzos,
/// adjuntos, el PDF— no se tocan: son el contenido, no la identidad.
pub fn renovar(paquete: &crate::Paquete, ahora: i64) -> crate::Paquete {
    let mut proyecto = paquete.proyecto.clone();
    proyecto.uid = Some(codigos::nuevo());
    proyecto.creado = ahora;
    proyecto.aparato = None;
    // `origen` dice de que proyecto de otro aparato es copia: una cosa nueva
    // no viene de ninguna.
    proyecto.resto.remove("origen");
    for h in &mut proyecto.hojas {
        h.uid = Some(codigos::nuevo());
        h.resto.remove("origen");
    }
    let mut nuevo = crate::Paquete::nuevo(paquete.manifiesto.clone(), proyecto);
    // Lo anotado de un adjunto (tinta, marcos, marcadores con su vista) va
    // por el codigo de su mensaje (`anot-<uid>…`): el mensaje que lo tiene
    // estrena su codigo ya aqui y sus ficheros se llaman con el nuevo, o la
    // copia se quedaria sin nada de lo anotado.
    let anotados: std::collections::HashSet<String> =
        paquete.nombres().filter_map(|n| uid_de_lo_anotado(n).map(str::to_string)).collect();
    let mut cambiados = std::collections::HashMap::new();
    if let Some(bytes) = paquete.entrada("guardados.jsonl") {
        nuevo.poner_entrada("guardados.jsonl", renovar_cuaderno(bytes, ahora, &anotados, &mut cambiados).into_bytes());
    }
    for nombre in paquete.nombres() {
        if nombre == "manifest.json" || nombre == "proyecto.json" || nombre == "guardados.jsonl" {
            continue;
        }
        let Some(bytes) = paquete.entrada(nombre) else {
            continue;
        };
        let puesto = match uid_de_lo_anotado(nombre).and_then(|u| cambiados.get(u).map(|n: &String| (u, n))) {
            Some((viejo, estrenado)) => {
                let (carpeta, fichero) = nombre.rsplit_once('/').unwrap_or(("", nombre));
                let fichero = fichero.replacen(viejo, estrenado, 1);
                if carpeta.is_empty() { fichero } else { format!("{carpeta}/{fichero}") }
            }
            None => nombre.to_string(),
        };
        nuevo.poner_entrada(&puesto, bytes.to_vec());
    }
    nuevo
}

/// El codigo de mensaje de un fichero de lo anotado del paquete
/// (`lienzos/anot-<uid>….excalidraw`, `android/pins/draw/anot-<uid>…`): lo
/// que sigue a `anot-` hasta el primer `-` o `.`, si tiene el largo de un
/// codigo.
fn uid_de_lo_anotado(nombre: &str) -> Option<&str> {
    let fichero = nombre.rsplit('/').next()?;
    let resto = fichero.strip_prefix(pixpin_sincro::anotado::PREFIJO)?;
    let fin = resto.find(['-', '.'])?;
    let uid = &resto[..fin];
    (uid.chars().count() == pixpin_sincro::codigo::LARGO).then_some(uid)
}

/// Los mensajes de un cuaderno con los tres codigos estrenados.
///
/// Como en Android (`Codigos.renovar` mas `copy(id, cuando, uid = null)`):
/// id y hora nuevos —la hora los mantiene en el mismo orden—, y sin codigo
/// unico, numero, letra, aparato ni origen, que los pone quien los guarde.
/// Una linea que no se entienda se copia tal cual: reescribir el cuaderno no
/// puede ser la forma de perder lo que escriba una version mas nueva.
///
/// Salvo el codigo unico de los que tienen algo anotado (`anotados`): esos
/// estrenan uno ya, apuntado en `cambiados` (viejo → nuevo).
fn renovar_cuaderno(
    bytes: &[u8],
    ahora: i64,
    anotados: &std::collections::HashSet<String>,
    cambiados: &mut std::collections::HashMap<String, String>,
) -> String {
    use crate::cuaderno::Mensaje;
    let texto = String::from_utf8_lossy(bytes);
    let mut salida = String::with_capacity(texto.len());
    for (i, linea) in texto.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        match serde_json::from_str::<Mensaje>(linea) {
            Ok(m) => {
                let uid = m.uid.as_ref().filter(|u| anotados.contains(*u)).map(|viejo| {
                    cambiados.entry(viejo.clone()).or_insert_with(codigos::nuevo).clone()
                });
                let puesto = Mensaje {
                    id: (ahora + i as i64).to_string(),
                    cuando: ahora + i as i64,
                    uid,
                    numero: 0,
                    letra: None,
                    aparato: None,
                    origen: None,
                    ..m
                };
                match serde_json::to_string(&puesto) {
                    Ok(t) => salida.push_str(&t),
                    Err(_) => salida.push_str(linea),
                }
            }
            Err(_) => salida.push_str(linea),
        }
        salida.push('\n');
    }
    salida
}

/// **Actualizar el que tengo**: lo que llega, escrito sobre el proyecto `id`.
///
/// Es la otra mitad de la pregunta del movil (`Recepcion.guardarProyecto`
/// cuando `antes != null`). El proyecto de aqui **conserva su `id`**, y con
/// el su carpeta, sus accesos y lo que el chat ya senalaba; lo que trae el
/// paquete se escribe encima fichero a fichero.
///
/// **Recibir no quita nada** (Android, 15-sep-2026): lo que solo esta aqui
/// —un lienzo que el otro aparato no tenia, un mensaje escrito despues— se
/// queda. Por eso el cuaderno se funde por codigo unico en vez de sustituirse
/// y los ficheros que el paquete no trae no se borran. El usuario perdio
/// lienzos justamente por lo contrario.
///
/// Quien llama tiene que haber hecho antes la copia de seguridad
/// (`pixpin_sincro::copias::hacer`): aqui se escribe encima.
pub fn actualizar_paquete(
    raiz: &Path,
    id: &str,
    paquete: &crate::Paquete,
) -> std::io::Result<Ficha> {
    let p = &paquete.proyecto;
    let destino = carpeta(raiz, id);
    std::fs::create_dir_all(&destino)?;
    for nombre in paquete.nombres() {
        let Some(bytes) = paquete.entrada(nombre) else {
            continue;
        };
        // Un nombre con `..` dentro no puede escribir fuera de la carpeta del
        // proyecto: viene de otro aparato.
        if nombre.split(['/', '\\']).any(|t| t == "..") {
            continue;
        }
        // `proyecto.json` sale de la estructura, ya con el id de aqui.
        if nombre == "proyecto.json" {
            continue;
        }
        if nombre == "guardados.jsonl" {
            // Temporal y renombrado, como el indice: un corte en mitad de
            // esta escritura deja la conversacion partida por la mitad, y lo
            // que se pierde son mensajes que solo tenia este equipo.
            let fundido = fundir_cuaderno(&destino.join("guardados.jsonl"), bytes)?;
            pixpin_sincro::disco::escribir_atomico(
                &destino.join("guardados.jsonl"),
                fundido.as_bytes(),
            )?;
            continue;
        }
        let ruta = destino.join(nombre.replace('\\', "/"));
        if let Some(padre) = ruta.parent() {
            std::fs::create_dir_all(padre)?;
        }
        std::fs::write(&ruta, bytes)?;
    }
    // El `proyecto.json` de aqui: las hojas que llegan mas las que solo tenia
    // este equipo, detras y sin repetir, y **el id que ya tenia**, que es con
    // el que este proyecto se sincroniza (`vista::chat_de_ficha` lo saca de
    // aqui). Cambiarlo partiria en dos la conversacion.
    let mut puesto = p.clone();
    if let Ok(texto) = std::fs::read_to_string(destino.join("proyecto.json"))
        && let Ok(antes) = serde_json::from_str::<crate::Proyecto>(&texto)
    {
        if !antes.id.is_empty() {
            puesto.id = antes.id.clone();
        }
        let llegan: std::collections::BTreeSet<String> =
            puesto.hojas.iter().map(|h| h.codigo_unico()).collect();
        for h in antes.hojas {
            if !llegan.contains(&h.codigo_unico()) {
                puesto.hojas.push(h);
            }
        }
        puesto.archivado = antes.archivado;
    }
    std::fs::write(
        destino.join("proyecto.json"),
        serde_json::to_vec_pretty(&puesto).map_err(std::io::Error::other)?,
    )?;

    let mut indice = Indice::leer_si_esta(raiz)?;
    let Some(sitio) = indice.proyectos.iter().position(|f| f.id == id) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "ese proyecto ya no está en la lista",
        ));
    };
    let antes = indice.proyectos[sitio].clone();
    let ficha = Ficha {
        nombre: if p.nombre.is_empty() {
            antes.nombre.clone()
        } else {
            p.nombre.clone()
        },
        hojas: puesto.hojas.len() as u32,
        tocado: p.tocado.max(antes.tocado),
        ..antes
    };
    indice.proyectos[sitio] = ficha.clone();
    indice.guardar(raiz)?;
    Ok(ficha)
}

/// El cuaderno de aqui con lo que llega escrito encima, por codigo unico.
///
/// Lo que ya estaba con el mismo codigo se sustituye **en su sitio** (para
/// que la conversacion no se reordene) y lo que no estaba se anade al final.
/// Lo que solo tiene este equipo se queda: recibir no borra mensajes.
fn fundir_cuaderno(fichero: &Path, llegan: &[u8]) -> std::io::Result<String> {
    use crate::cuaderno::Mensaje;
    // **No `unwrap_or_default`**: un cuaderno que esta pero no se deja leer
    // daria la fusion con SOLO lo que trae el movil, y lo escrito aqui y no
    // sincronizado se perderia sin que nadie lo notara. Solo cuenta como
    // vacio el cuaderno que todavia no existe.
    let mias = match std::fs::read_to_string(fichero) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let texto = String::from_utf8_lossy(llegan);
    let mut nuevas: Vec<(String, &str)> = Vec::new();
    for linea in texto.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str::<Mensaje>(linea) {
            Ok(m) => nuevas.push((m.codigo_unico(), linea)),
            // Sin poder leerla no hay con que compararla: entra al final.
            Err(_) => nuevas.push((String::new(), linea)),
        }
    }
    let mut salida = String::with_capacity(mias.len() + texto.len());
    let mut puestas = std::collections::BTreeSet::new();
    for linea in mias.lines().filter(|l| !l.trim().is_empty()) {
        let clave = serde_json::from_str::<Mensaje>(linea)
            .map(|m| m.codigo_unico())
            .unwrap_or_default();
        let sustituta = (!clave.is_empty())
            .then(|| nuevas.iter().find(|(c, _)| *c == clave))
            .flatten();
        match sustituta {
            Some((c, l)) => {
                salida.push_str(l);
                puestas.insert(c.clone());
            }
            None => salida.push_str(linea),
        }
        salida.push('\n');
    }
    for (clave, linea) in &nuevas {
        if !clave.is_empty() && puestas.contains(clave) {
            continue;
        }
        salida.push_str(linea);
        salida.push('\n');
    }
    Ok(salida)
}

/// Lo contrario de `importar_paquete`: un proyecto de la lista, entero, como
/// un `.pixpin` para mandarlo a otro aparato.
///
/// Va todo lo que hay en su carpeta —cuaderno, adjuntos, lienzos y lo que
/// trajo del movil y aqui no se entiende—, porque lo que no viaja se pierde
/// al otro lado. El `proyecto.json` sale con los tres codigos de la FICHA:
/// son los que este equipo conoce, y con ellos el otro sabra que es la misma
/// cosa si ya la tenia. Un proyecto nacido aqui no tiene `proyecto.json`: se
/// le hace uno con lo que dice la ficha.
pub fn empaquetar(raiz: &Path, id: &str, cuando: i64) -> std::io::Result<Vec<u8>> {
    let ficha = Indice::leer(raiz)
        .buscar(id)
        .cloned()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "ese proyecto no está"))?;
    let dir = carpeta(raiz, id);
    let mut proyecto: crate::Proyecto = std::fs::read(dir.join("proyecto.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| crate::Proyecto {
            id: ficha.id.clone(),
            tocado: ficha.tocado,
            ..Default::default()
        });
    proyecto.nombre = ficha.nombre.clone();
    proyecto.uid = ficha.uid.clone().or(proyecto.uid);
    if ficha.creado > 0 {
        proyecto.creado = ficha.creado;
    }
    proyecto.aparato = ficha.aparato.clone().or(proyecto.aparato);
    proyecto.tocado = proyecto.tocado.max(ficha.tocado);
    proyecto.sellar(ficha.aparato.as_deref());
    let manifiesto = crate::Manifiesto {
        escrito: cuando,
        proyecto: ficha.nombre.clone(),
        ..Default::default()
    };
    let mut paquete = crate::Paquete::nuevo(manifiesto, proyecto);
    let mut pendientes = vec![dir.clone()];
    while let Some(d) = pendientes.pop() {
        let Ok(leidas) = std::fs::read_dir(&d) else {
            continue;
        };
        for entrada in leidas.flatten() {
            let ruta = entrada.path();
            if ruta.is_dir() {
                pendientes.push(ruta);
                continue;
            }
            let Ok(relativa) = ruta.strip_prefix(&dir) else {
                continue;
            };
            let nombre = relativa.to_string_lossy().replace('\\', "/");
            // Estos dos los escribe el paquete desde su estructura.
            if nombre == "proyecto.json" || nombre == "manifest.json" {
                continue;
            }
            paquete.poner_entrada(&nombre, std::fs::read(&ruta)?);
        }
    }
    // El documento de un PDF unido aqui vive en `archivos/doc-<t>.pdf`, y el
    // `.pixpin` lo lleva como `documento.pdf`: es donde lo busca el movil
    // (sus hojas `pagina` son paginas de ESE fichero).
    if paquete.entrada("documento.pdf").is_none()
        && let Some(doc) = crate::vista::documento_del_proyecto(raiz, id)
    {
        paquete.poner_entrada("documento.pdf", std::fs::read(&doc)?);
    }
    paquete.a_bytes().map_err(std::io::Error::other)
}

/// Anade al cuaderno las hojas del `proyecto.json` que todavia no estan.
///
/// Un proyecto que entro con una version anterior se quedo sin las paginas
/// del PDF que no llevaban dibujo y sin saber de que pagina cuelga cada
/// «zona». Reimportarlo obligaria a volver a pedirselo al movil, asi que se
/// completa en el sitio: se compara por el codigo unico de cada hoja, que es
/// lo unico que no cambia, y lo que ya esta no se toca.
///
/// Devuelve cuantas se anadieron.
pub fn completar_hojas(raiz: &Path, id: &str, aparato: &str) -> std::io::Result<usize> {
    use crate::cuaderno;
    // Un chat que ya se sincroniza tiene sus mensajes como los tiene el otro
    // aparato: el movil no pone uno por pagina de PDF, y los que se
    // inventaran aqui le llegarian como nuevos en la vuelta siguiente.
    if Indice::leer(raiz)
        .buscar(id)
        .is_some_and(|f| f.resto.get(crate::vista::MARCA_SINCRO).is_some())
    {
        return Ok(0);
    }
    let carpeta = carpeta(raiz, id);
    let texto = match std::fs::read_to_string(carpeta.join("proyecto.json")) {
        Ok(t) => t,
        // Un proyecto nacido aqui no tiene `proyecto.json`: no hay nada que
        // completar y no es un error.
        Err(_) => return Ok(0),
    };
    let Ok(p) = serde_json::from_str::<crate::Proyecto>(&texto) else {
        return Ok(0);
    };
    let previos = cuaderno::Cuaderno::leer_de(&carpeta).unwrap_or_default();
    let ya: std::collections::BTreeSet<String> = previos
        .mensajes
        .iter()
        .filter_map(|m| m.uid.clone())
        .collect();
    let numero = previos.mensajes.iter().map(|m| m.numero).max().unwrap_or(0);
    let cuando = previos
        .mensajes
        .iter()
        .map(|m| m.cuando)
        .max()
        .unwrap_or(p.tocado);
    // Las hojas que salieron de un mensaje (`deMensaje`: las paginas de un
    // PDF unido desde el chat) ya tienen su mensaje: escribirles otro llenaria
    // la conversacion de paginas, que es justo lo que unir no debe hacer. Se
    // ensenan en la galeria sin tocar el cuaderno (`hojas_para_ensenar`).
    let de_un_mensaje: std::collections::BTreeSet<String> = p
        .hojas
        .iter()
        .filter(|h| h.resto.get("deMensaje").is_some_and(|v| v.is_string()))
        .filter_map(|h| h.uid.clone())
        .collect();
    // Y las paginas del PDF, vengan de donde vengan (anotadas o no): como
    // en `RegistroDelChat.queFalta` del movil, «sus paginas no, que van
    // dentro del PDF y no son lienzos sueltos». Un PDF de cuarenta hojas no
    // son cuarenta mensajes; se ven en la tarjeta de Proyectos.
    let paginas = paginas_fuera_del_chat(&p);
    let mut hechas = 0;
    for m in hojas_que_faltan(
        &p,
        &carpeta,
        &ya,
        &referencias_de(&previos.mensajes),
        numero,
        cuando,
        aparato,
        id,
    ) {
        if m.uid
            .as_ref()
            .is_some_and(|u| de_un_mensaje.contains(u) || paginas.contains_key(u))
        {
            continue;
        }
        cuaderno::anadir(&carpeta, &m)?;
        hechas += 1;
    }
    Ok(hechas)
}

/// Lo que dice de que hoja es cada mensaje: su `referencia` (el dibujo) y su
/// `id` (lo que una hoja guarda en `deMensaje`). Ver [`hojas_que_faltan`].
fn referencias_de(mensajes: &[crate::cuaderno::Mensaje]) -> std::collections::BTreeSet<String> {
    mensajes
        .iter()
        .flat_map(|m| {
            m.referencia
                .iter()
                .cloned()
                .chain(std::iter::once(m.id.clone()))
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Los mensajes que representan las hojas del `proyecto.json` que el
/// cuaderno todavia no tiene. **No escribe nada.**
///
/// Va aparte de `completar_hojas` porque hay dos usos con reglas distintas:
///
/// - **Escribirlos** (`completar_hojas`) solo vale para un proyecto que NO se
///   sincroniza. En uno que si, el movil no pone un mensaje por pagina de
///   PDF, y los que se inventaran aqui le llegarian como mensajes nuevos.
/// - **Ensenarlos** vale siempre: el usuario quiere ver el proyecto entero,
///   con sus palabras, «si hay PDFs ahi tienen que aparecer todas sus hojas
///   y asi como en Android». Para eso se piden aqui y se usan solo en
///   pantalla, sin tocar el cuaderno.
///
/// `ya` son los codigos unicos que el cuaderno ya tiene y `ya_ref` lo que
/// dice de que hoja es cada mensaje (su `referencia` y su `id`): una hoja
/// cuyo dibujo ya es la `referencia` de un mensaje, o cuyo `deMensaje` es un
/// mensaje del chat, ya esta en el, aunque su codigo unico no coincida
/// (`RegistroDelChat.queFalta` del movil compara asi; por el codigo solo,
/// los lienzos del movil salian dos veces, queja del 3-oct-2026); `desde_numero` y
/// `desde_cuando` son el ultimo numero y la ultima hora usados, para que lo
/// que salga vaya detras y en orden.
#[allow(clippy::too_many_arguments)] // es el sello del cuaderno, que va junto
pub fn hojas_que_faltan(
    p: &crate::Proyecto,
    carpeta: &Path,
    ya: &std::collections::BTreeSet<String>,
    ya_ref: &std::collections::BTreeSet<String>,
    desde_numero: i64,
    desde_cuando: i64,
    aparato: &str,
    id: &str,
) -> Vec<crate::cuaderno::Mensaje> {
    use crate::cuaderno;
    let mut numero = desde_numero;
    let mut salida = Vec::new();
    for hoja in &p.hojas {
        let Some(uid) = hoja.uid.clone() else {
            continue;
        };
        // Las paginas de un PDF guardan en `deMensaje` el mensaje del PDF,
        // no uno suyo: ese no las representa, y la tarjeta las ensena todas.
        let de_mensaje = hoja
            .resto
            .get("deMensaje")
            .and_then(|v| v.as_str())
            .filter(|_| hoja.pagina.is_none());
        if ya.contains(&uid)
            || hoja.dibujo.as_ref().is_some_and(|d| ya_ref.contains(d))
            || de_mensaje.is_some_and(|d| ya_ref.contains(d))
        {
            continue;
        }
        numero += 1;
        let sello = cuaderno::Sello {
            cuando: desde_cuando + numero,
            numero,
            aparato: aparato.to_string(),
            proyecto: id.to_string(),
        };
        let mut m = match (&hoja.dibujo, &hoja.nota) {
            (Some(dibujo), _) => {
                let nombre = format!("{dibujo}.excalidraw");
                let ruta = format!("lienzos/{nombre}");
                let bytes = std::fs::metadata(carpeta.join(&ruta))
                    .map(|m| m.len() as i64)
                    .unwrap_or(0);
                let mut m = cuaderno::Mensaje::adjunto(
                    cuaderno::Clase::Dibujo,
                    if hoja.nombre.is_empty() {
                        &nombre
                    } else {
                        &hoja.nombre
                    },
                    &ruta,
                    bytes,
                    &sello,
                );
                m.referencia = Some(dibujo.clone());
                m
            }
            (None, Some(texto)) => cuaderno::Mensaje::nota(texto, &sello),
            (None, None) if hoja.pagina.is_some() => {
                cuaderno::Mensaje::adjunto(cuaderno::Clase::Pagina, &hoja.nombre, "", 0, &sello)
            }
            (None, None) => cuaderno::Mensaje::nota(&hoja.nombre, &sello),
        };
        m.uid = Some(uid);
        m.pagina = hoja.pagina;
        m.responde_a = hoja
            .resto
            .get("padre")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        salida.push(m);
    }
    salida
}

/// **Las paginas del PDF de un proyecto, que no van al chat**: codigo unico
/// de cada hoja con `pagina` (anotada o no) -> el mensaje del que salio
/// (`deMensaje`), si salio de uno.
///
/// Es la regla del movil (`AvisosDelProyecto.novedades` y
/// `RegistroDelChat.queFalta` se saltan `h.pagina != null`): del PDF, en el
/// chat solo esta su mensaje; las paginas son hojas del proyecto y se ven en
/// la tarjeta de Proyectos y al abrir el PDF. El usuario lo pidio igual
/// (27-sep-2026): «esto podria llenar el chat absurdamente si hubiera un PDF
/// largo». El mensaje de origen va aparte porque una pagina que el usuario
/// SI mando al chat (y luego unio) tiene como codigo el de su mensaje: ese
/// mensaje es suyo y se queda.
pub fn paginas_fuera_del_chat(
    p: &crate::Proyecto,
) -> std::collections::BTreeMap<String, Option<String>> {
    p.hojas
        .iter()
        .filter(|h| h.pagina.is_some())
        .filter_map(|h| {
            let de = h
                .resto
                .get("deMensaje")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            Some((h.uid.clone()?, de))
        })
        .collect()
}

/// Las hojas del proyecto `id` que su cuaderno no tiene, listas para
/// ENSENAR. Nunca escribe; `Vec` vacio si el proyecto no tiene
/// `proyecto.json` (los nacidos aqui) o si no se puede leer.
///
/// Es lo que hace que un proyecto sincronizado del movil se vea entero: las
/// paginas de sus PDF son hojas, y el movil no las anota como mensajes.
pub fn hojas_para_ensenar(raiz: &Path, id: &str, aparato: &str) -> Vec<crate::cuaderno::Mensaje> {
    let carpeta = carpeta(raiz, id);
    let Ok(texto) = std::fs::read_to_string(carpeta.join("proyecto.json")) else {
        return Vec::new();
    };
    let Ok(p) = serde_json::from_str::<crate::Proyecto>(&texto) else {
        return Vec::new();
    };
    let previos = crate::cuaderno::Cuaderno::leer_de(&carpeta).unwrap_or_default();
    let ya: std::collections::BTreeSet<String> = previos
        .mensajes
        .iter()
        .filter_map(|m| m.uid.clone())
        .collect();
    let numero = previos.mensajes.iter().map(|m| m.numero).max().unwrap_or(0);
    let cuando = previos
        .mensajes
        .iter()
        .map(|m| m.cuando)
        .max()
        .unwrap_or(p.tocado);
    // SOLO las hojas. Los ficheros sueltos de la carpeta (un `.html`, una
    // imagen) NO entran: la pantalla de proyectos de Android tampoco los
    // ensena —recorre `proyecto.hojas` y no el disco (`ui/Proyectos.kt`)— y
    // el usuario lo pidio igual: «en la galeria solo aparezca lo mismo que
    // aparece en la seccion de proyectos de Android», y «los html no tienen
    // que aparecer ya que estos no se pueden agregar ahi».
    hojas_que_faltan(
        &p,
        &carpeta,
        &ya,
        &referencias_de(&previos.mensajes),
        numero,
        cuando,
        aparato,
        id,
    )
}

/// Los codigos unicos de las hojas del proyecto.
///
/// Con ellos se sabe QUE mensajes son hojas, que es justo lo que la galeria
/// ensena. Vacio si el proyecto no tiene `proyecto.json` (los nacidos aqui):
/// entonces no hay hojas y la galeria sale vacia, como en el movil.
pub fn uids_de_hojas(raiz: &Path, id: &str) -> std::collections::BTreeSet<String> {
    let Ok(texto) = std::fs::read_to_string(carpeta(raiz, id).join("proyecto.json")) else {
        return Default::default();
    };
    let Ok(p) = serde_json::from_str::<crate::Proyecto>(&texto) else {
        return Default::default();
    };
    p.hojas.iter().filter_map(|h| h.uid.clone()).collect()
}

/// Como se llama el chat de lo suelto, igual que en el movil.
pub const NOMBRE_GUARDADOS: &str = "Mensajes guardados";
/// Como se llamo aqui antes de tener su nombre de verdad.
const NOMBRE_VIEJO_DE_GUARDADOS: &str = "Del movil";
/// La marca que lo distingue en el indice. Una marca y no el nombre: si el
/// usuario lo renombra sigue siendo el, y un proyecto suyo que se llame igual
/// no lo suplanta.
const MARCA_GUARDADOS: &str = "guardados";

impl Ficha {
    /// Si es el chat de «Mensajes guardados»: el que existe siempre, va
    /// arriba y no se borra.
    pub fn es_guardados(&self) -> bool {
        self.resto.get(MARCA_GUARDADOS).and_then(|v| v.as_bool()) == Some(true)
    }
}

/// El chat de «Mensajes guardados», creandolo si no esta.
///
/// Es adonde va todo lo suelto -un archivo que llega del movil, por ejemplo-,
/// asi que tiene que existir ANTES de que llegue nada: el usuario lo busco y
/// no estaba. El «Del movil» de las primeras versiones se convierte en el, con
/// lo que ya tenia dentro.
pub fn asegurar_guardados(raiz: &Path, cuando: i64, aparato: &str) -> std::io::Result<Ficha> {
    let mut indice = Indice::leer_si_esta(raiz)?;
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.es_guardados()) {
        // Se llama SIEMPRE asi (el usuario: «siempre tiene que ser solo
        // Mensajes guardados, que no se pueda cambiar de nombre»). Uno que se
        // renombro antes de esta regla, o que llega renombrado de otro
        // aparato, recupera aqui su nombre.
        if f.nombre == NOMBRE_GUARDADOS {
            return Ok(f.clone());
        }
        f.nombre = NOMBRE_GUARDADOS.to_string();
        let f = f.clone();
        indice.guardar(raiz)?;
        return Ok(f);
    }
    let marca = (MARCA_GUARDADOS.to_string(), serde_json::Value::Bool(true));
    let ficha = match indice
        .proyectos
        .iter_mut()
        .find(|f| f.nombre == NOMBRE_VIEJO_DE_GUARDADOS)
    {
        Some(vieja) => {
            vieja.nombre = NOMBRE_GUARDADOS.to_string();
            vieja.resto.insert(marca.0, marca.1);
            vieja.clone()
        }
        None => {
            let mut nueva = Ficha::nueva(NOMBRE_GUARDADOS, cuando, aparato);
            nueva.resto.insert(marca.0, marca.1);
            indice.proyectos.push(nueva.clone());
            nueva
        }
    };
    indice.guardar(raiz)?;
    Ok(ficha)
}

/// Como se llama el chat del asistente local (`crates/pixpin-ia`).
pub const NOMBRE_ASISTENTE: &str = "Asistente";
/// Su marca en el indice, como la de «Mensajes guardados»: renombrado sigue
/// siendo el, y un chat del usuario que se llame igual no lo suplanta.
const MARCA_ASISTENTE: &str = "asistente";

impl Ficha {
    /// Si es el chat fijo del asistente: todo lo que se escribe en el se le
    /// pregunta al modelo local.
    pub fn es_asistente(&self) -> bool {
        self.resto.get(MARCA_ASISTENTE).and_then(|v| v.as_bool()) == Some(true)
    }
}

/// El chat «Asistente», creandolo si no esta. Nace sin nada dentro: la
/// tarjeta de bajar el modelo la pinta el chat, no es un mensaje.
pub fn asegurar_asistente(raiz: &Path, cuando: i64, aparato: &str) -> std::io::Result<Ficha> {
    let mut indice = Indice::leer_si_esta(raiz)?;
    if let Some(f) = indice.proyectos.iter().find(|f| f.es_asistente()) {
        return Ok(f.clone());
    }
    let mut nueva = Ficha::nueva(NOMBRE_ASISTENTE, cuando, aparato);
    nueva
        .resto
        .insert(MARCA_ASISTENTE.to_string(), serde_json::Value::Bool(true));
    indice.proyectos.push(nueva.clone());
    indice.guardar(raiz)?;
    Ok(nueva)
}

/// Donde van a parar los proyectos borrados.
pub fn papelera(raiz: &Path) -> PathBuf {
    raiz.join("papelera")
}

/// Quita proyectos de la lista y se lleva sus carpetas a la papelera.
/// Devuelve cuantos se quitaron y las carpetas que no se pudieron mover, para
/// que quien llama lo cuente: este crate no escribe en el registro.
///
/// A la papelera y no borrados de verdad: un proyecto son horas de trabajo y
/// el boton esta a un clic de «abrir». La carpeta se va con la hora en el
/// nombre para que borrar dos veces un proyecto con el mismo `id` -se importa,
/// se borra, se vuelve a importar, se vuelve a borrar- no choque.
///
/// Primero la lista y despues las carpetas: si mover una falla -la tiene
/// abierta otro programa-, el proyecto ya no sale en la lista y su carpeta
/// sigue entera donde estaba, que es el lado bueno del que caerse.
pub fn borrar_proyectos(
    raiz: &Path,
    ids: &[String],
    cuando: i64,
) -> std::io::Result<(usize, Vec<PathBuf>)> {
    // Un `id` es un nombre de carpeta. Uno con barras o puntos sacaria de
    // `proyectos/` lo que se mueve; los de verdad son letras y cifras.
    let validos: Vec<&String> = ids
        .iter()
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .collect();
    let mut indice = Indice::leer_si_esta(raiz)?;
    let antes = indice.proyectos.len();
    // Los chats que se van, antes de que dejen de estar en el indice: cada
    // uno deja su lapida (`LapidaDeChat`). Sin ella, la siguiente vuelta de
    // sincronizar lo traeria entero otra vez del otro aparato.
    // Y la ficha entera de cada uno: con ella la papelera lo devuelve con su
    // nombre, sus codigos y su chat (`recuperar`), no como uno a medias.
    let se_van: Vec<(Ficha, Option<String>)> = indice
        .proyectos
        .iter()
        .filter(|f| !f.es_guardados() && validos.contains(&&f.id))
        .map(|f| (f.clone(), crate::vista::chat_de_ficha(raiz, &f.id)))
        .collect();
    let chats: Vec<String> = se_van.iter().filter_map(|(_, c)| c.clone()).collect();
    // «Mensajes guardados» no se borra: es donde cae lo que llega, y sin el
    // lo siguiente que llegara no tendria adonde ir.
    indice
        .proyectos
        .retain(|f| f.es_guardados() || !validos.contains(&&f.id));
    for chat in &chats {
        pixpin_sincro::disco::anotar_lapida_en(&raiz.join("sincro"), chat, cuando, "")?;
    }
    let validos: Vec<&String> = validos
        .into_iter()
        .filter(|id| indice.buscar(id).is_none())
        .collect();
    let quitados = antes - indice.proyectos.len();
    if quitados > 0 {
        indice.guardar(raiz)?;
    }
    let destino = papelera(raiz);
    let mut sin_mover = Vec::new();
    for id in validos {
        let origen = carpeta(raiz, id);
        // Un proyecto guardado en otra carpeta es una union: a la papelera va
        // la union (renombrar mueve el enlace, no lo que hay detras), y la
        // carpeta del usuario no se toca. Tambien si esta rota —el disco
        // desenchufado—: `is_dir` la da por ausente y se quedaria colgando.
        if !origen.is_dir() && !pixpin_sistema::union::es_union(&origen) {
            continue;
        }
        std::fs::create_dir_all(&destino)?;
        let nombre = format!("{id}-{cuando}");
        if std::fs::rename(&origen, destino.join(&nombre)).is_err() {
            sin_mover.push(origen);
            continue;
        }
        // La etiqueta va AL LADO de la carpeta y no dentro: dentro se
        // colaria en el proyecto al recuperarlo. Si no se puede escribir, la
        // papelera rehace la ficha con `proyecto.json`: se pierde el orden de
        // la lista, no el proyecto, y por eso no se corta el borrado.
        if let Some((f, chat)) = se_van.iter().find(|(f, _)| &f.id == id) {
            let etiqueta = Etiqueta {
                ficha: f.clone(),
                chat: chat.clone(),
                borrado: cuando,
            };
            if let Ok(t) = serde_json::to_vec_pretty(&etiqueta) {
                let _ = std::fs::write(destino.join(format!("{nombre}.json")), t);
            }
        }
    }
    Ok((quitados, sin_mover))
}

/// Lo que se apunta al lado de cada carpeta de la papelera.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Etiqueta {
    ficha: Ficha,
    chat: Option<String>,
    borrado: i64,
}

/// Un proyecto en la papelera, listo para ensenar y recuperar
/// (`Copias.proyectosBorrados` del movil, v0.79).
#[derive(Debug, Clone, PartialEq)]
pub struct EnPapelera {
    /// Su carpeta dentro de la papelera (`<id>-<ms>`).
    pub carpeta: PathBuf,
    /// Como estaba en la lista cuando se borro.
    pub ficha: Ficha,
    /// Su chat de sincronizar, el de su lapida, si se supo al borrarlo.
    pub chat: Option<String>,
    /// Cuando se borro, en milisegundos UTC.
    pub borrado: i64,
    /// Cuantos mensajes tiene su chat.
    pub mensajes: usize,
}

/// `<id>-<ms>` → (id, ms). El id puede llevar guiones; la hora no.
fn partir_nombre(nombre: &str) -> Option<(&str, i64)> {
    let (id, ms) = nombre.rsplit_once('-')?;
    let valido = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    (valido && !ms.is_empty() && ms.chars().all(|c| c.is_ascii_digit()))
        .then(|| ms.parse().ok().map(|ms| (id, ms)))
        .flatten()
}

/// Los proyectos que se pueden recuperar enteros, el borrado mas reciente
/// primero.
///
/// De cada proyecto solo sale su ultimo borrado: se importa, se borra, se
/// vuelve a importar y se vuelve a borrar deja dos carpetas, y recuperar la
/// vieja con la nueva tambien en la papelera seria devolver lo de antes. Y
/// no sale lo que ya vuelve a estar en la lista (por su `id` o por su chat,
/// que llego otra vez del movil): recuperarlo lo duplicaria.
pub fn en_papelera(raiz: &Path) -> Vec<EnPapelera> {
    let dir = papelera(raiz);
    let Ok(lista) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let indice = Indice::leer(raiz);
    let vivos = crate::vista::chats_vivos(raiz);
    let mut salida: Vec<EnPapelera> = Vec::new();
    for e in lista.flatten().filter(|e| e.path().is_dir()) {
        let nombre = e.file_name().to_string_lossy().to_string();
        let Some((id, ms)) = partir_nombre(&nombre) else {
            continue;
        };
        let carpeta = e.path();
        let etiqueta: Option<Etiqueta> = std::fs::read(dir.join(format!("{nombre}.json")))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let (ficha, chat, borrado) = match etiqueta {
            Some(et) if et.ficha.id == id => (et.ficha, et.chat, et.borrado.max(ms)),
            // Borrado antes de que hubiera etiqueta: la ficha se rehace con
            // lo que tenga dentro.
            _ => (ficha_de_carpeta(&carpeta, id), None, ms),
        };
        if indice.buscar(id).is_some()
            || chat.as_ref().is_some_and(|c| vivos.contains(c))
            || carpeta_de_proyecto_ocupada(raiz, id)
        {
            continue;
        }
        let mensajes = crate::cuaderno::Cuaderno::leer_de(&carpeta)
            .map(|c| c.mensajes.len())
            .unwrap_or(0);
        let nuevo = EnPapelera {
            carpeta,
            ficha,
            chat,
            borrado,
            mensajes,
        };
        match salida.iter_mut().find(|x| x.ficha.id == id) {
            Some(x) if x.borrado < nuevo.borrado => *x = nuevo,
            Some(_) => {}
            None => salida.push(nuevo),
        }
    }
    salida.sort_by_key(|x| std::cmp::Reverse(x.borrado));
    salida
}

fn carpeta_de_proyecto_ocupada(raiz: &Path, id: &str) -> bool {
    // Sin seguir enlaces: una union rota (su disco desenchufado) no «existe»
    // para `exists`, pero ocupa el nombre y no se puede renombrar encima.
    std::fs::symlink_metadata(carpeta(raiz, id)).is_ok()
}

/// La ficha de una carpeta sin etiqueta: la de su `proyecto.json` si lo
/// tiene (lo que vino del movil), o una con el id por nombre.
fn ficha_de_carpeta(carpeta: &Path, id: &str) -> Ficha {
    let de_proyecto = std::fs::read_to_string(carpeta.join("proyecto.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<crate::Proyecto>(&t).ok())
        .map(|p| Ficha::de_proyecto(&p, None));
    let mut f = de_proyecto.unwrap_or_else(|| Ficha {
        nombre: id.to_string(),
        ..Default::default()
    });
    // El id es el de la carpeta: es donde viven sus mensajes y lienzos.
    f.id = id.to_string();
    if f.nombre.trim().is_empty() {
        f.nombre = id.to_string();
    }
    f
}

/// **Recupera un proyecto entero** de la papelera: su carpeta vuelve a
/// `proyectos/` y su ficha a la lista, con su nombre, sus codigos, sus hojas,
/// su PDF y su chat. Devuelve la ficha puesta.
///
/// Y deja de estar borrado para sincronizar (`Copias.restaurar` del movil,
/// v0.79): se quita su lapida, porque con ella la vuelta siguiente lo
/// volveria a borrar nada mas recuperarlo; y se olvida lo acordado de su
/// chat, porque con eso las marcas que el otro puso al borrarlo pasarian por
/// cambios suyos y le quitarian los mensajes uno a uno.
///
/// Su hora pasa a ser `ahora` por lo mismo: si volviera con la de antes de
/// borrarlo, el otro aparato lo veria «sin tocar desde que se borro» y la
/// lista lo propondria para borrar otra vez.
///
/// Falla sin tocar nada si su sitio esta ocupado (otro proyecto con ese id o
/// con su chat): recuperar no puede pisar ni duplicar.
pub fn recuperar(raiz: &Path, en: &EnPapelera, ahora: i64) -> std::io::Result<Ficha> {
    use std::io::{Error, ErrorKind};
    let id = en.ficha.id.clone();
    let nombre = en
        .carpeta
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    // Solo lo que esta en la papelera y con su nombre de papelera: una ruta
    // que venga de fuera no puede mover nada mas.
    if en.carpeta.parent() != Some(papelera(raiz).as_path())
        || partir_nombre(&nombre).map(|(i, _)| i) != Some(id.as_str())
        || !en.carpeta.is_dir()
    {
        return Err(Error::new(ErrorKind::NotFound, "no esta en la papelera"));
    }
    let mut indice = Indice::leer_si_esta(raiz)?;
    if indice.buscar(&id).is_some() || carpeta_de_proyecto_ocupada(raiz, &id) {
        return Err(Error::new(
            ErrorKind::AlreadyExists,
            "ya hay un proyecto en su sitio",
        ));
    }
    if en
        .chat
        .as_ref()
        .is_some_and(|c| crate::vista::chats_vivos(raiz).contains(c))
    {
        return Err(Error::new(
            ErrorKind::AlreadyExists,
            "ese proyecto ya esta en la lista",
        ));
    }
    std::fs::create_dir_all(raiz.join("proyectos"))?;
    std::fs::rename(&en.carpeta, carpeta(raiz, &id))?;
    let mut ficha = en.ficha.clone();
    ficha.tocado = ficha.tocado.max(ahora);
    indice.proyectos.push(ficha.clone());
    if let Err(e) = indice.guardar(raiz) {
        // Sin la ficha en la lista, la carpeta vuelve a la papelera: mejor
        // seguir borrado que perdido en `proyectos/` sin que nada lo ensene.
        let _ = std::fs::rename(carpeta(raiz, &id), &en.carpeta);
        return Err(e);
    }
    let sincro = raiz.join("sincro");
    let chat = en
        .chat
        .clone()
        .or_else(|| crate::vista::chat_de_ficha(raiz, &id));
    if let Some(c) = &chat {
        pixpin_sincro::disco::quitar_lapida_en(&sincro, c)?;
        pixpin_sincro::disco::olvidar_bases_en(&sincro, c)?;
    }
    let _ = std::fs::remove_file(papelera(raiz).join(format!("{nombre}.json")));
    Ok(ficha)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ficha(id: &str, nombre: &str, tocado: i64) -> Ficha {
        Ficha {
            id: id.into(),
            nombre: nombre.into(),
            uid: Some("VVT587BFCA".into()),
            creado: 1_757_939_357_123,
            aparato: Some("K7Q2".into()),
            tocado,
            ..Default::default()
        }
    }

    fn carpeta_temporal(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-proyectos-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn un_proyecto_empaquetado_se_lleva_su_carpeta_y_sus_tres_codigos() {
        let raiz = carpeta_temporal("empaquetar");
        let mut i = Indice::default();
        i.proyectos.push(ficha("p1", "Casa", 10));
        i.guardar(&raiz).unwrap();
        let dir = carpeta(&raiz, "p1");
        std::fs::create_dir_all(dir.join("archivos")).unwrap();
        std::fs::write(dir.join("guardados.jsonl"), b"{}\n").unwrap();
        std::fs::write(dir.join("archivos").join("plano.png"), b"png").unwrap();

        let bytes = empaquetar(&raiz, "p1", 99).unwrap();
        let p = crate::Paquete::desde_bytes(&bytes).expect("es un .pixpin legible");
        assert_eq!(p.proyecto.nombre, "Casa");
        assert_eq!(p.proyecto.uid.as_deref(), Some("VVT587BFCA"));
        assert_eq!(p.proyecto.creado, 1_757_939_357_123);
        assert_eq!(p.proyecto.aparato.as_deref(), Some("K7Q2"));
        assert_eq!(p.entrada("archivos/plano.png"), Some(&b"png"[..]));
        assert_eq!(p.entrada("guardados.jsonl"), Some(&b"{}\n"[..]));
        assert_eq!(p.manifiesto.escrito, 99);

        // Y al otro lado se reconoce como la misma cosa, no como otra.
        let otra = carpeta_temporal("empaquetar-otro");
        let llega = importar_paquete(&otra, &p, "ZZZZ").unwrap();
        assert!(llega.misma_que(&ficha("x", "Casa", 0)));
    }

    /// Un paquete como el que manda el movil: un proyecto con dos hojas, sus
    /// lienzos y su cuaderno.
    fn paquete(nombre: &str, uid: &str, creado: i64) -> crate::Paquete {
        let hoja = |id: &str, uid: &str, dibujo: &str| crate::Hoja {
            id: id.into(),
            nombre: id.into(),
            dibujo: Some(dibujo.into()),
            uid: Some(uid.into()),
            ..Default::default()
        };
        let proyecto = crate::Proyecto {
            id: "p-movil".into(),
            nombre: nombre.into(),
            hojas: vec![
                hoja("h1", "AAAAAAAAAA", "d1"),
                hoja("h2", "BBBBBBBBBB", "d2"),
            ],
            tocado: creado + 5,
            uid: Some(uid.into()),
            creado,
            aparato: Some("K7Q2".into()),
            ..Default::default()
        };
        let mut p = crate::Paquete::nuevo(crate::Manifiesto::default(), proyecto);
        p.poner_entrada("lienzos/d1.excalidraw", b"{\"uno\":1}".to_vec());
        p.poner_entrada("lienzos/d2.excalidraw", b"{\"dos\":2}".to_vec());
        p.poner_entrada(
            "guardados.jsonl",
            concat!(
                "{\"id\":\"m1\",\"cuando\":100,\"uid\":\"MMMMMMMMMM\",\"numero\":1,\"aparato\":\"K7Q2\",\"texto\":\"del movil\"}\n",
                "{\"id\":\"m2\",\"cuando\":200,\"uid\":\"NNNNNNNNNN\",\"numero\":2,\"aparato\":\"K7Q2\",\"texto\":\"otro\"}\n",
            )
            .as_bytes()
            .to_vec(),
        );
        // Ida y vuelta por el ZIP, como el que llega de verdad: asi
        // `proyecto.json` y el manifiesto estan escritos dentro y no vacios.
        crate::Paquete::desde_bytes(&p.a_bytes().unwrap()).unwrap()
    }

    fn lineas(raiz: &Path, id: &str) -> Vec<crate::cuaderno::Mensaje> {
        crate::cuaderno::Cuaderno::leer_de(&carpeta(raiz, id))
            .unwrap()
            .mensajes
    }

    #[test]
    fn lo_que_vuelve_con_los_tres_codigos_se_reconoce_y_lo_que_cambia_uno_no() {
        let raiz = carpeta_temporal("misma");
        let p = paquete("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let puesta = importar_paquete(&raiz, &p, "ZZZZ").unwrap();
        let indice = Indice::leer(&raiz);
        let llega = Ficha::de_proyecto(&p.proyecto, None);
        assert_eq!(indice.misma(&llega).map(|f| f.id.clone()), Some(puesta.id));
        // Casos negativos: con otra fecha de creacion, otro aparato o otro
        // codigo unico ya no es la misma cosa y no se puede pisar.
        for otra in [
            Ficha {
                creado: 1,
                ..llega.clone()
            },
            Ficha {
                aparato: Some("OTRO".into()),
                ..llega.clone()
            },
            Ficha {
                uid: Some("2222222222".into()),
                ..llega.clone()
            },
        ] {
            assert!(indice.misma(&otra).is_none(), "{otra:?} no es la misma");
        }
    }

    #[test]
    fn un_proyecto_sincronizado_ensena_sus_hojas_aunque_no_las_escriba() {
        // Lo que el usuario reporto: al sincronizar, las paginas de un PDF
        // son hojas del proyecto y el movil no las anota como mensajes, asi
        // que el chat de aqui ensenaba menos cosas que el telefono.
        let raiz = carpeta_temporal("hojas-de-sincro");
        let p = paquete("Examen", "VVT587BFCA", 1_757_939_357_123);
        let ficha = importar_paquete(&raiz, &p, "ZZZZ").unwrap();
        let dir = carpeta(&raiz, &ficha.id);
        // Una pagina de PDF sin dibujo, como las que manda el movil.
        let mut proyecto: crate::Proyecto =
            serde_json::from_str(&std::fs::read_to_string(dir.join("proyecto.json")).unwrap())
                .unwrap();
        proyecto.hojas.push(crate::Hoja {
            id: "h-pag-7".into(),
            nombre: "Página 7".into(),
            pagina: Some(7),
            uid: Some("PAGPAGPAG7".into()),
            ..Default::default()
        });
        std::fs::write(
            dir.join("proyecto.json"),
            serde_json::to_vec(&proyecto).unwrap(),
        )
        .unwrap();

        let antes = lineas(&raiz, &ficha.id).len();
        let ensenar = hojas_para_ensenar(&raiz, &ficha.id, "ZZZZ");
        // Las tres hojas del proyecto: sus dos lienzos, que el cuaderno del
        // movil tampoco anota, y la pagina de PDF recien anadida.
        assert_eq!(ensenar.len(), 3);
        let pagina = ensenar
            .iter()
            .find(|m| m.uid.as_deref() == Some("PAGPAGPAG7"))
            .expect("la pagina que falta");
        assert_eq!(pagina.pagina, Some(7));
        assert_eq!(pagina.clase, Some(crate::cuaderno::Clase::Pagina));
        assert!(
            ensenar
                .iter()
                .any(|m| m.clase == Some(crate::cuaderno::Clase::Dibujo)),
            "y los lienzos tambien"
        );
        // Y lo importante: NO ha tocado el cuaderno. Escribirlas las mandaria
        // de vuelta al movil como mensajes nuevos.
        assert_eq!(lineas(&raiz, &ficha.id).len(), antes, "no escribe nada");

        // Caso negativo: pedirlas dos veces no las duplica, y una hoja que
        // el cuaderno YA tiene deja de salir.
        assert_eq!(hojas_para_ensenar(&raiz, &ficha.id, "ZZZZ").len(), 3);
        crate::cuaderno::anadir(&dir, pagina).unwrap();
        let despues = hojas_para_ensenar(&raiz, &ficha.id, "ZZZZ");
        assert_eq!(despues.len(), 2, "la pagina ya esta en el cuaderno");
        assert!(
            despues
                .iter()
                .all(|m| m.uid.as_deref() != Some("PAGPAGPAG7"))
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_lienzo_con_su_mensaje_no_sale_otra_vez_aunque_su_codigo_unico_no_coincida() {
        // Lo que pasaba con los lienzos del movil (3-oct-2026): el mensaje y
        // la hoja apuntan al mismo dibujo pero llevan codigos unicos
        // distintos, y el chat los ensenaba dos veces.
        let p = crate::Proyecto {
            hojas: vec![
                crate::Hoja {
                    id: "h-1".into(),
                    dibujo: Some("dib-1".into()),
                    uid: Some("HOJAHOJA01".into()),
                    ..Default::default()
                },
                crate::Hoja {
                    id: "h-2".into(),
                    dibujo: Some("dib-2".into()),
                    uid: Some("HOJAHOJA02".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mensaje = crate::cuaderno::Mensaje {
            id: "m-1".into(),
            clase: Some(crate::cuaderno::Clase::Dibujo),
            referencia: Some("dib-1".into()),
            uid: Some("OTROCODIGO".into()),
            ..Default::default()
        };
        let ya: std::collections::BTreeSet<String> = mensaje.uid.iter().cloned().collect();
        let faltan = hojas_que_faltan(
            &p,
            Path::new("."),
            &ya,
            &referencias_de(std::slice::from_ref(&mensaje)),
            0,
            0,
            "ZZZZ",
            "p1",
        );
        // Caso negativo: la hoja sin mensaje (dib-2) sigue saliendo.
        let refs: Vec<_> = faltan
            .iter()
            .filter_map(|m| m.referencia.as_deref())
            .collect();
        assert_eq!(refs, ["dib-2"]);
    }

    #[test]
    fn un_proyecto_nacido_aqui_no_tiene_hojas_que_ensenar() {
        // Caso negativo: sin `proyecto.json` no hay nada que completar, y
        // leerlo no puede ser un error.
        let raiz = carpeta_temporal("hojas-sin-proyecto-json");
        assert!(hojas_para_ensenar(&raiz, "NOEXISTE00", "ZZZZ").is_empty());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn actualizar_escribe_encima_y_no_se_lleva_por_delante_lo_que_solo_hay_aqui() {
        let raiz = carpeta_temporal("actualizar");
        let p = paquete("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let ficha = importar_paquete(&raiz, &p, "ZZZZ").unwrap();
        let dir = carpeta(&raiz, &ficha.id);
        // Lo que este equipo tiene y el movil no: un lienzo mas y un mensaje.
        let mut mio: crate::Proyecto =
            serde_json::from_str(&std::fs::read_to_string(dir.join("proyecto.json")).unwrap())
                .unwrap();
        mio.hojas.push(crate::Hoja {
            id: "h-mia".into(),
            nombre: "solo aqui".into(),
            dibujo: Some("d-mia".into()),
            uid: Some("CCCCCCCCCC".into()),
            ..Default::default()
        });
        std::fs::write(dir.join("proyecto.json"), serde_json::to_vec(&mio).unwrap()).unwrap();
        crate::cuaderno::anadir(
            &dir,
            &crate::cuaderno::Mensaje {
                id: "m-mio".into(),
                cuando: 300,
                uid: Some("PPPPPPPPPP".into()),
                texto: "escrito aqui".into(),
                ..Default::default()
            },
        )
        .unwrap();

        // Y ahora vuelve del movil con un lienzo cambiado y un mensaje nuevo.
        let mut vuelve = paquete("Tesis al día", "VVT587BFCA", 1_757_939_357_123);
        vuelve.poner_entrada("lienzos/d1.excalidraw", b"{\"uno\":99}".to_vec());
        vuelve.poner_entrada(
            "guardados.jsonl",
            concat!(
                "{\"id\":\"m1\",\"cuando\":100,\"uid\":\"MMMMMMMMMM\",\"numero\":1,\"aparato\":\"K7Q2\",\"texto\":\"corregido\"}\n",
                "{\"id\":\"m3\",\"cuando\":400,\"uid\":\"QQQQQQQQQQ\",\"numero\":3,\"aparato\":\"K7Q2\",\"texto\":\"nuevo\"}\n",
            )
            .as_bytes()
            .to_vec(),
        );
        let puesta = actualizar_paquete(&raiz, &ficha.id, &vuelve).unwrap();

        assert_eq!(puesta.id, ficha.id, "el id de aqui manda");
        assert_eq!(puesta.nombre, "Tesis al día");
        // El id con el que se sincroniza no cambia: si cambiara, la
        // conversacion se partiria en dos.
        assert_eq!(
            crate::vista::chat_de_ficha(&raiz, &ficha.id).as_deref(),
            Some("p-movil")
        );
        assert_eq!(
            std::fs::read(dir.join("lienzos/d1.excalidraw")).unwrap(),
            b"{\"uno\":99}"
        );
        let p2: crate::Proyecto =
            serde_json::from_str(&std::fs::read_to_string(dir.join("proyecto.json")).unwrap())
                .unwrap();
        assert_eq!(p2.id, "p-movil");
        assert!(
            p2.hojas.iter().any(|h| h.id == "h-mia"),
            "recibir no quita lienzos: {:?}",
            p2.hojas.iter().map(|h| &h.id).collect::<Vec<_>>()
        );
        assert_eq!(p2.hojas.len(), 3, "y no los duplica");
        let ms = lineas(&raiz, &ficha.id);
        let textos: Vec<&str> = ms.iter().map(|m| m.texto.as_str()).collect();
        assert!(textos.contains(&"corregido"), "{textos:?}");
        assert!(textos.contains(&"escrito aqui"), "{textos:?}");
        assert!(textos.contains(&"nuevo"), "{textos:?}");
        assert!(!textos.contains(&"del movil"), "se puso al dia: {textos:?}");
        assert_eq!(puesta.hojas, 3);
    }

    #[test]
    fn actualizar_un_proyecto_que_no_esta_en_la_lista_falla_y_no_lo_inventa() {
        // Caso negativo: sin ficha no hay a que ponerse al dia, y crear una a
        // escondidas dejaria dos proyectos con la misma carpeta.
        let raiz = carpeta_temporal("actualizar-nada");
        let p = paquete("Tesis", "VVT587BFCA", 1);
        let e = actualizar_paquete(&raiz, "no-existe", &p).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn crear_como_nuevo_estrena_los_tres_codigos_y_deja_de_ser_la_misma_cosa() {
        let raiz = carpeta_temporal("renovar");
        let p = paquete("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let vieja = importar_paquete(&raiz, &p, "ZZZZ").unwrap();

        let nuevo = renovar(&p, 5_000_000);
        assert_ne!(nuevo.proyecto.uid, p.proyecto.uid);
        assert_eq!(nuevo.proyecto.creado, 5_000_000);
        assert_eq!(nuevo.proyecto.aparato, None);
        for (a, b) in nuevo.proyecto.hojas.iter().zip(&p.proyecto.hojas) {
            assert_ne!(a.uid, b.uid, "cada hoja estrena el suyo");
        }
        // El contenido es el mismo: solo cambia quien dice que es.
        assert_eq!(
            nuevo.entrada("lienzos/d1.excalidraw"),
            p.entrada("lienzos/d1.excalidraw")
        );

        let otra = importar_paquete(&raiz, &nuevo, "ZZZZ").unwrap();
        assert_ne!(otra.id, vieja.id);
        assert!(!otra.misma_que(&vieja), "conviven sin pisarse");
        // Y sus mensajes tampoco son los de antes, o el siguiente envio de uno
        // pondria al dia al otro.
        let ms = lineas(&raiz, &otra.id);
        assert_eq!(ms.len(), 2);
        assert!(ms.iter().all(|m| m.uid.is_none() && m.numero == 0));
        let viejos: Vec<String> = lineas(&raiz, &vieja.id)
            .iter()
            .map(|m| m.codigo_unico())
            .collect();
        for m in &ms {
            assert!(
                !viejos.contains(&m.codigo_unico()),
                "{:?}",
                m.codigo_unico()
            );
        }
        assert_eq!(ms[0].texto, "del movil", "el contenido se conserva");
    }

    #[test]
    fn el_pixpin_lleva_todo_lo_anotado_y_crear_como_nuevo_no_lo_suelta() {
        // Un chat con un PDF anotado: tinta de una pagina, marcos, vistas de
        // los marcadores y lo anotado en un marcador del 3D.
        let raiz = carpeta_temporal("todo-lo-anotado");
        let p = paquete("Obra", "VVT587BFCA", 1_757_939_357_123);
        let ficha = importar_paquete(&raiz, &p, "ZZZZ").unwrap();
        let dir = carpeta(&raiz, &ficha.id);
        let lado = dir.join("android").join("pins").join("draw");
        std::fs::create_dir_all(&lado).unwrap();
        let ficheros = [
            ("lienzos/anot-MMMMMMMMMM-p0.excalidraw", "tinta"),
            ("android/pins/draw/anot-MMMMMMMMMM.marcos.json", "[{\"n\":1}]"),
            ("android/pins/draw/anot-MMMMMMMMMM.vistas.json", "{\"m1\":\"2 0 0\"}"),
            ("android/pins/draw/anot-MMMMMMMMMM-mm1.hoja", "marco"),
        ];
        for (rel, texto) in ficheros {
            std::fs::write(dir.join(rel), texto).unwrap();
        }

        let bytes = empaquetar(&raiz, &ficha.id, 9).unwrap();
        let llegado = crate::Paquete::desde_bytes(&bytes).unwrap();
        for (rel, texto) in ficheros {
            assert_eq!(llegado.entrada(rel), Some(texto.as_bytes()), "{rel}");
        }
        let otra_raiz = carpeta_temporal("todo-lo-anotado-otro-pc");
        let puesta = importar_paquete(&otra_raiz, &llegado, "YYYY").unwrap();
        for (rel, texto) in ficheros {
            assert_eq!(std::fs::read_to_string(carpeta(&otra_raiz, &puesta.id).join(rel)).unwrap(), texto, "{rel}");
        }

        // Crear como nuevo: el mensaje anotado estrena codigo y lo suyo se
        // llama con el; el que no tenia nada se queda sin codigo (Android).
        let nuevo = renovar(&llegado, 5_000_000);
        let copia = importar_paquete(&otra_raiz, &nuevo, "YYYY").unwrap();
        let ms = lineas(&otra_raiz, &copia.id);
        let anotado = ms.iter().find(|m| m.texto == "del movil").unwrap();
        let uid = anotado.uid.clone().expect("el anotado estrena codigo");
        assert_ne!(uid, "MMMMMMMMMM");
        assert!(ms.iter().find(|m| m.texto == "otro").unwrap().uid.is_none());
        for (rel, texto) in ficheros {
            let rel = rel.replace("MMMMMMMMMM", &uid);
            assert_eq!(std::fs::read_to_string(carpeta(&otra_raiz, &copia.id).join(&rel)).unwrap(), texto, "{rel}");
        }
        assert!(nuevo.nombres().all(|n| !n.contains("MMMMMMMMMM")), "nada con el codigo viejo");
    }

    #[test]
    fn empaquetar_un_proyecto_que_no_esta_falla_y_no_inventa_uno() {
        let raiz = carpeta_temporal("empaquetar-nada");
        let e = empaquetar(&raiz, "no-existe", 1).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn la_lista_ensena_arriba_lo_ultimo_tocado() {
        let mut i = Indice::default();
        i.proyectos.push(ficha("a", "Casa", 10));
        i.proyectos.push(ficha("b", "Obra", 30));
        i.proyectos.push(ficha("c", "Taller", 20));
        let nombres: Vec<&str> = i.ordenadas().iter().map(|f| f.nombre.as_str()).collect();
        assert_eq!(nombres, ["Obra", "Taller", "Casa"]);
    }

    #[test]
    fn un_proyecto_creado_aqui_nace_con_sus_tres_codigos_y_no_se_duplica() {
        let f = Ficha::nueva("Obra nueva", 1_000, "K7Q2");
        assert_eq!(f.nombre, "Obra nueva");
        assert_eq!(f.aparato.as_deref(), Some("K7Q2"));
        assert_eq!((f.creado, f.tocado), (1_000, 1_000));
        assert!(f.uid.is_some(), "sin uid no se reconoceria al volver");
        assert_eq!(f.paquete, None, "no salio de ningun fichero");

        // Dos creados seguidos son cosas distintas: si compartieran codigos,
        // el segundo pisaria al primero al viajar.
        let otro = Ficha::nueva("Obra nueva", 1_000, "K7Q2");
        assert_ne!(f.id, otro.id);
        assert_ne!(f.uid, otro.uid);
        assert!(!f.misma_que(&otro), "mismo nombre no es la misma cosa");

        // Y el que vuelve del movil se reconoce en vez de duplicarse.
        let mut i = Indice::default();
        i.proyectos.push(f.clone());
        let (que, _) = i.recibir(f);
        assert_eq!(que, Recibido::Actualizado);
        assert_eq!(i.proyectos.len(), 1);
    }

    #[test]
    fn recibir_lo_mismo_lo_pone_al_dia_en_vez_de_duplicarlo() {
        let mut i = Indice::default();
        i.recibir(ficha("a", "Casa", 10));
        let mut otra_vez = ficha("id-de-otro-aparato", "Casa (con la cocina)", 50);
        otra_vez.resumen = "3 hojas".into();
        let (que, id) = i.recibir(otra_vez);
        assert_eq!(que, Recibido::Actualizado);
        assert_eq!(id, "a", "conserva el id de aqui");
        assert_eq!(i.proyectos.len(), 1);
        assert_eq!(i.buscar("a").unwrap().nombre, "Casa (con la cocina)");
        assert_eq!(i.buscar("a").unwrap().resumen, "3 hojas");
    }

    #[test]
    fn si_no_coinciden_los_tres_codigos_entra_como_nuevo_y_no_pisa() {
        let mut i = Indice::default();
        i.recibir(ficha("a", "Casa", 10));
        // Mismo codigo unico y misma fecha, pero otro aparato: no es lo mismo.
        let mut otra = ficha("a", "Casa de otro", 20);
        otra.aparato = Some("9FMQ".into());
        let (que, id) = i.recibir(otra);
        assert_eq!(que, Recibido::Creado);
        assert_ne!(id, "a", "no puede compartir id con el que ya estaba");
        assert_eq!(i.proyectos.len(), 2);
        assert_eq!(i.buscar("a").unwrap().nombre, "Casa", "el de aqui, intacto");
        // Y el nuevo estrena codigo unico, para poder viajar sin pisar.
        let nuevo = i.buscar(&id).unwrap();
        assert_ne!(nuevo.uid.as_deref(), Some("VVT587BFCA"));
        assert_eq!(nuevo.uid.as_deref().map(str::len), Some(codigos::LARGO));
    }

    #[test]
    fn el_indice_va_y_vuelve_del_disco_sin_perder_campos() {
        let raiz = carpeta_temporal("disco");
        let mut i = Indice::default();
        let mut f = ficha("a", "Casa", 10);
        f.resto
            .insert("futuro".into(), serde_json::Value::Bool(true));
        i.proyectos.push(f);
        i.guardar(&raiz).unwrap();
        let vuelta = Indice::leer(&raiz);
        assert_eq!(vuelta, i);
        assert!(vuelta.proyectos[0].resto.contains_key("futuro"));
        // Caso negativo: sin fichero, lista vacia y ningun error.
        let _ = std::fs::remove_dir_all(&raiz);
        assert_eq!(Indice::leer(&raiz), Indice::default());
    }

    #[test]
    fn un_nombre_de_fuera_no_puede_decidir_donde_se_escribe() {
        // Lo importante: nada que salga de la carpeta.
        for malo in ["../../pasa.txt", r"c:\windows\system32\a.dll", "a/b/c.png"] {
            let s = nombre_seguro(malo);
            assert!(!s.contains('/') && !s.contains('\\'), "{malo} -> {s}");
            assert!(!s.contains(':'), "{malo} -> {s}");
        }
        // Los nombres que Windows no deja crear.
        assert_eq!(nombre_seguro("CON"), "_CON");
        assert_eq!(nombre_seguro("nul.txt"), "_nul.txt");
        assert_eq!(nombre_seguro("fin."), "fin");
        assert_eq!(nombre_seguro("   "), "archivo");
        // Y uno normal se queda como esta, acentos incluidos.
        assert_eq!(nombre_seguro("Plano fachada ñ.pdf"), "Plano fachada ñ.pdf");
    }

    #[test]
    fn un_nombre_del_finder_en_nfd_queda_igual_que_el_de_windows() {
        // «Diseño ó.pdf» escrito como lo da el Finder: n + virgulilla
        // combinable, o + tilde combinable.
        let nfd = "Disen\u{303}o o\u{301}.pdf";
        let nfc = "Diseño ó.pdf";
        assert_ne!(nfd.as_bytes(), nfc.as_bytes());
        assert_eq!(nombre_seguro(nfd), nfc);
        assert_eq!(nombre_seguro(nfc), nfc);
        // Caso negativo: lo que ya era ASCII no cambia ni un byte.
        assert_eq!(nombre_seguro("plano-2026_v3.dwg"), "plano-2026_v3.dwg");
    }

    #[test]
    fn dos_ficheros_con_el_mismo_nombre_no_se_pisan() {
        let raiz = carpeta_temporal("adjuntos");
        let a = guardar_adjunto(&raiz, "pr-1", "captura.png", b"primero").unwrap();
        let b = guardar_adjunto(&raiz, "pr-1", "captura.png", b"segundo").unwrap();
        assert_eq!(a, "archivos/captura.png");
        assert_eq!(
            b, "archivos/captura (1).png",
            "el segundo no pisa al primero"
        );
        let base = carpeta(&raiz, "pr-1");
        assert_eq!(std::fs::read(base.join(&a)).unwrap(), b"primero");
        assert_eq!(std::fs::read(base.join(&b)).unwrap(), b"segundo");
        // Y cada proyecto tiene los suyos.
        let c = guardar_adjunto(&raiz, "pr-2", "captura.png", b"de otro").unwrap();
        assert_eq!(c, "archivos/captura.png");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_nombre_sin_extension_tambien_se_numera_bien() {
        let raiz = carpeta_temporal("sin-extension");
        assert_eq!(
            guardar_adjunto(&raiz, "p", "LEEME", b"1").unwrap(),
            "archivos/LEEME"
        );
        assert_eq!(
            guardar_adjunto(&raiz, "p", "LEEME", b"2").unwrap(),
            "archivos/LEEME (1)"
        );
        // Caso negativo: uno que empieza por punto es todo extension.
        assert_eq!(
            guardar_adjunto(&raiz, "p", ".gitignore", b"3").unwrap(),
            "archivos/.gitignore"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn borrar_un_proyecto_lo_quita_de_la_lista_y_lo_deja_en_la_papelera() {
        let raiz = carpeta_temporal("borrar");
        let mut i = Indice::default();
        i.proyectos.push(ficha("a", "Casa", 10));
        i.proyectos.push(ficha("b", "Obra", 30));
        i.guardar(&raiz).unwrap();
        std::fs::create_dir_all(carpeta(&raiz, "a")).unwrap();
        std::fs::write(carpeta(&raiz, "a").join("guardados.jsonl"), "{}").unwrap();

        let (n, sin_mover) = borrar_proyectos(&raiz, &["a".to_string()], 99).unwrap();
        assert!(sin_mover.is_empty());

        assert_eq!(n, 1);
        let queda = Indice::leer(&raiz);
        assert_eq!(queda.proyectos.len(), 1);
        assert_eq!(queda.proyectos[0].id, "b");
        assert!(!carpeta(&raiz, "a").exists());
        assert!(
            papelera(&raiz)
                .join("a-99")
                .join("guardados.jsonl")
                .is_file()
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    // ------------------------------------------------------------ papelera

    /// Dos proyectos, `a` con mensajes y una base acordada con un movil, y
    /// `a` borrado desde la lista a las 99.
    fn con_a_en_la_papelera(etiqueta: &str) -> PathBuf {
        let raiz = carpeta_temporal(etiqueta);
        let mut i = Indice::default();
        let mut a = ficha("a", "Casa", 10);
        a.hojas = 3;
        i.proyectos.push(a);
        i.proyectos.push(ficha("b", "Obra", 30));
        i.guardar(&raiz).unwrap();
        std::fs::create_dir_all(carpeta(&raiz, "a")).unwrap();
        std::fs::write(carpeta(&raiz, "a").join("guardados.jsonl"), "").unwrap();
        let base = raiz.join("sincro").join("base").join("id-movil");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("a.json"), "{}").unwrap();
        borrar_proyectos(&raiz, &["a".to_string()], 99).unwrap();
        raiz
    }

    fn lapidas(raiz: &Path) -> String {
        std::fs::read_to_string(raiz.join("sincro").join("chatsborrados.jsonl")).unwrap_or_default()
    }

    #[test]
    fn la_papelera_ensena_lo_borrado_con_su_nombre_y_no_lo_que_sigue_en_la_lista() {
        let raiz = con_a_en_la_papelera("papelera-lista");
        let v = en_papelera(&raiz);
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].ficha.nombre, "Casa");
        assert_eq!(v[0].ficha.hojas, 3, "la ficha entera, de su etiqueta");
        assert_eq!(v[0].borrado, 99);
        assert_eq!(v[0].chat.as_deref(), Some("a"));
        // Caso negativo: una carpeta suelta que no es `<id>-<ms>` no sale.
        std::fs::create_dir_all(papelera(&raiz).join("cosas")).unwrap();
        assert_eq!(en_papelera(&raiz).len(), 1);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn recuperar_devuelve_el_proyecto_entero_y_deja_de_estar_borrado_para_sincronizar() {
        let raiz = con_a_en_la_papelera("papelera-recuperar");
        assert!(lapidas(&raiz).contains("\"a\""), "borrar dejo lapida");
        let en = en_papelera(&raiz).remove(0);
        let f = recuperar(&raiz, &en, 5_000).unwrap();
        assert_eq!(f.nombre, "Casa");
        let i = Indice::leer(&raiz);
        let puesta = i.buscar("a").expect("vuelve a la lista");
        assert_eq!(puesta.uid.as_deref(), Some("VVT587BFCA"), "con sus codigos");
        assert_eq!(
            puesta.tocado, 5_000,
            "tocado ahora, o se propondria borrarlo otra vez"
        );
        assert!(carpeta(&raiz, "a").join("guardados.jsonl").is_file());
        assert!(
            !lapidas(&raiz).contains("\"a\""),
            "sin lapida: no se vuelve a borrar"
        );
        assert!(
            !raiz.join("sincro/base/id-movil/a.json").exists(),
            "lo acordado se olvida: sus mensajes ganan a las marcas del otro"
        );
        assert!(en_papelera(&raiz).is_empty(), "ya no esta en la papelera");
        assert!(
            !papelera(&raiz).join("a-99.json").exists(),
            "ni su etiqueta"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn recuperar_no_pisa_un_proyecto_que_ya_ocupa_su_sitio() {
        let raiz = con_a_en_la_papelera("papelera-ocupado");
        let en = en_papelera(&raiz).remove(0);
        // Mientras tanto volvio a entrar un «a» (se importo otra vez).
        let mut i = Indice::leer(&raiz);
        i.proyectos.push(ficha("a", "Casa nueva", 50));
        i.guardar(&raiz).unwrap();
        let e = recuperar(&raiz, &en, 5_000).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(en.carpeta.is_dir(), "la de la papelera sigue donde estaba");
        assert_eq!(Indice::leer(&raiz).proyectos.len(), 2, "no se duplica");
        assert!(en_papelera(&raiz).is_empty(), "y no se ofrece");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn recuperar_no_mueve_nada_que_no_este_en_la_papelera() {
        let raiz = con_a_en_la_papelera("papelera-fuera");
        let mut en = en_papelera(&raiz).remove(0);
        en.carpeta = carpeta(&raiz, "b");
        std::fs::create_dir_all(&en.carpeta).unwrap();
        assert!(recuperar(&raiz, &en, 5_000).is_err());
        assert!(carpeta(&raiz, "b").is_dir());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn de_un_proyecto_borrado_dos_veces_sale_el_ultimo_borrado() {
        let raiz = con_a_en_la_papelera("papelera-dos-veces");
        let mut i = Indice::leer(&raiz);
        i.proyectos.push(ficha("a", "Casa otra vez", 200));
        i.guardar(&raiz).unwrap();
        std::fs::create_dir_all(carpeta(&raiz, "a")).unwrap();
        borrar_proyectos(&raiz, &["a".to_string()], 300).unwrap();
        let v = en_papelera(&raiz);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].borrado, 300);
        assert_eq!(v[0].ficha.nombre, "Casa otra vez");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn lo_borrado_antes_de_las_etiquetas_se_recupera_con_el_nombre_de_su_carpeta() {
        let raiz = carpeta_temporal("papelera-sin-etiqueta");
        let vieja = papelera(&raiz).join("c-7");
        std::fs::create_dir_all(&vieja).unwrap();
        std::fs::write(vieja.join("guardados.jsonl"), "").unwrap();
        let en = en_papelera(&raiz).remove(0);
        assert_eq!(en.ficha.id, "c");
        assert_eq!(en.chat, None);
        let f = recuperar(&raiz, &en, 9).unwrap();
        assert_eq!(f.nombre, "c");
        assert!(Indice::leer(&raiz).buscar("c").is_some());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_nombre_de_la_papelera_se_parte_por_el_ultimo_guion() {
        assert_eq!(partir_nombre("pr-1-99"), Some(("pr-1", 99)));
        assert_eq!(partir_nombre("abc"), None);
        assert_eq!(partir_nombre("abc-"), None);
        assert_eq!(partir_nombre("-99"), None);
        assert_eq!(partir_nombre("a.b-99"), None, "nada de puntos");
    }

    #[test]
    fn mensajes_guardados_se_crea_una_vez_va_arriba_y_no_se_borra() {
        let raiz = carpeta_temporal("guardados");
        let mut i = Indice::default();
        i.proyectos.push(ficha("a", "Casa", 9_999));
        i.guardar(&raiz).unwrap();

        let g = asegurar_guardados(&raiz, 10, "K7Q2").unwrap();
        let otra_vez = asegurar_guardados(&raiz, 20, "K7Q2").unwrap();
        assert_eq!(g.id, otra_vez.id);

        let indice = Indice::leer(&raiz);
        assert_eq!(indice.proyectos.len(), 2);
        // «Casa» se toco mucho despues y aun asi va debajo.
        assert_eq!(indice.ordenadas()[0].nombre, NOMBRE_GUARDADOS);

        let (n, _) = borrar_proyectos(&raiz, std::slice::from_ref(&g.id), 1).unwrap();
        assert_eq!(n, 0);
        assert!(Indice::leer(&raiz).buscar(&g.id).is_some());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn mensajes_guardados_renombrado_recupera_su_nombre() {
        let raiz = carpeta_temporal("guardados-renombrado");
        let g = asegurar_guardados(&raiz, 10, "K7Q2").unwrap();
        let mut i = Indice::leer(&raiz);
        i.proyectos[0].nombre = "Mensajes guardadospkjhal".into();
        i.guardar(&raiz).unwrap();

        let otra_vez = asegurar_guardados(&raiz, 20, "K7Q2").unwrap();
        assert_eq!(otra_vez.id, g.id);
        assert_eq!(otra_vez.nombre, NOMBRE_GUARDADOS);
        assert_eq!(Indice::leer(&raiz).proyectos[0].nombre, NOMBRE_GUARDADOS);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_del_movil_de_antes_se_convierte_en_mensajes_guardados_con_su_id() {
        let raiz = carpeta_temporal("guardados-viejo");
        let mut i = Indice::default();
        i.proyectos.push(ficha("buzon", "Del movil", 5));
        i.guardar(&raiz).unwrap();

        let g = asegurar_guardados(&raiz, 10, "K7Q2").unwrap();

        // El mismo id: su carpeta, con lo recibido, sigue siendo la suya.
        assert_eq!(g.id, "buzon");
        assert_eq!(g.nombre, NOMBRE_GUARDADOS);
        assert_eq!(Indice::leer(&raiz).proyectos.len(), 1);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_proyecto_que_solo_se_llama_igual_no_es_mensajes_guardados() {
        assert!(!ficha("a", NOMBRE_GUARDADOS, 1).es_guardados());
    }

    #[test]
    fn un_id_con_barras_no_saca_nada_de_la_carpeta_de_proyectos() {
        let raiz = carpeta_temporal("borrar-malo");
        let mut i = Indice::default();
        i.proyectos.push(ficha("a", "Casa", 10));
        i.guardar(&raiz).unwrap();
        std::fs::create_dir_all(raiz.join("ajustes")).unwrap();

        let (n, _) =
            borrar_proyectos(&raiz, &["../ajustes".to_string(), String::new()], 1).unwrap();

        assert_eq!(n, 0);
        assert!(raiz.join("ajustes").is_dir());
        assert_eq!(Indice::leer(&raiz).proyectos.len(), 1);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Un proyecto nacido de un PDF en el movil: tres paginas (una anotada,
    /// con su dibujo), un lienzo suelto y ningun cuaderno.
    fn paquete_de_pdf() -> crate::Paquete {
        let pagina = |n: u32, dibujo: Option<&str>| crate::Hoja {
            id: format!("h-pag-{n}"),
            nombre: format!("Pagina {}", n + 1),
            pagina: Some(n),
            dibujo: dibujo.map(str::to_string),
            uid: Some(format!("PAGINA000{n}")),
            ..Default::default()
        };
        let proyecto = crate::Proyecto {
            id: "p-plano".into(),
            nombre: "Plano".into(),
            hojas: vec![
                pagina(0, None),
                pagina(1, Some("dpag1")),
                pagina(2, None),
                crate::Hoja {
                    id: "h-lienzo".into(),
                    nombre: "Croquis".into(),
                    dibujo: Some("dlienzo".into()),
                    uid: Some("LIENZO0001".into()),
                    ..Default::default()
                },
            ],
            pdf_origen: Some("/storage/emulated/0/Download/plano.pdf".into()),
            tocado: 10,
            creado: 1_757_939_357_123,
            ..Default::default()
        };
        let mut p = crate::Paquete::nuevo(crate::Manifiesto::default(), proyecto);
        p.poner_entrada("lienzos/dpag1.excalidraw", b"{}".to_vec());
        p.poner_entrada("lienzos/dlienzo.excalidraw", b"{}".to_vec());
        crate::Paquete::desde_bytes(&p.a_bytes().unwrap()).unwrap()
    }

    #[test]
    fn importar_un_proyecto_de_pdf_no_escribe_sus_paginas_en_el_chat() {
        // `RegistroDelChat.queFalta` del movil: «sus paginas no, que van
        // dentro del PDF y no son lienzos sueltos».
        let raiz = carpeta_temporal("importar-pdf");
        let ficha = importar_paquete(&raiz, &paquete_de_pdf(), "ZZZZ").unwrap();
        let escritos = lineas(&raiz, &ficha.id);
        assert_eq!(escritos.len(), 1, "solo el lienzo suelto: {escritos:?}");
        assert_eq!(escritos[0].uid.as_deref(), Some("LIENZO0001"));
        assert!(escritos.iter().all(|m| m.pagina.is_none()));
        // Las paginas siguen siendo hojas: la tarjeta de Proyectos las pide.
        let ensenar = hojas_para_ensenar(&raiz, &ficha.id, "ZZZZ");
        assert_eq!(ensenar.len(), 3, "las tres paginas, anotada incluida");
        assert!(ensenar.iter().all(|m| m.pagina.is_some()));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn completar_las_hojas_no_escribe_las_paginas_del_pdf_pero_si_los_lienzos() {
        let raiz = carpeta_temporal("completar-pdf");
        let ficha = importar_paquete(&raiz, &paquete_de_pdf(), "ZZZZ").unwrap();
        // Un cuaderno que se quedo sin nada: completar es el `reparar` del movil.
        std::fs::remove_file(carpeta(&raiz, &ficha.id).join("guardados.jsonl")).unwrap();
        assert_eq!(completar_hojas(&raiz, &ficha.id, "ZZZZ").unwrap(), 1);
        let escritos = lineas(&raiz, &ficha.id);
        assert_eq!(escritos.len(), 1);
        assert_eq!(
            escritos[0].uid.as_deref(),
            Some("LIENZO0001"),
            "el lienzo suelto si"
        );
        // Y otra vez no hace nada.
        assert_eq!(completar_hojas(&raiz, &ficha.id, "ZZZZ").unwrap(), 0);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn las_paginas_fuera_del_chat_son_las_hojas_con_pagina_y_su_mensaje_de_origen() {
        let mut p = paquete_de_pdf().proyecto;
        p.hojas[2].resto.insert(
            "deMensaje".into(),
            serde_json::Value::String("m-pdf".into()),
        );
        let fuera = paginas_fuera_del_chat(&p);
        assert_eq!(fuera.len(), 3);
        assert_eq!(fuera.get("PAGINA0000"), Some(&None));
        assert_eq!(fuera.get("PAGINA0002"), Some(&Some("m-pdf".to_string())));
        // Caso negativo: un lienzo suelto no es una pagina.
        assert!(!fuera.contains_key("LIENZO0001"));
    }
}
