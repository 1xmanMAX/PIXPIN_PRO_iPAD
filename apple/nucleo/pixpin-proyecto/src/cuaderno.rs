//! El cuaderno del movil: un chat contigo mismo.
//!
//! Fotos, ficheros, notas de voz que se pasan a texto, dibujos y paginas de
//! plano, todo en una conversacion. Es la pata que en Windows no existe.
//!
//! Se guarda en `guardados.jsonl`, **una linea de JSON por mensaje**, y se
//! escribe anadiendo al final. Esa forma no es casual y conviene entenderla
//! antes de tocarla:
//!
//! - Guardar un mensaje es escribir una linea. No hay que releer ni
//!   reescribir el fichero entero, asi que guardar cuesta lo mismo con diez
//!   mensajes que con diez mil.
//! - **Una linea rota no invalida el resto.** Si se corta la luz a mitad de
//!   escribir, se pierde ese mensaje y no el cuaderno. Con un JSON unico —un
//!   array de mensajes— un corte a mitad deja un fichero que no abre, y se
//!   pierde todo.
//!
//! Este lector respeta esa promesa: una linea que no se entienda se salta y
//! se cuenta, nunca tumba la lectura.

use serde::{Deserialize, Serialize};

/// De que es cada mensaje.
///
/// Se guardan en mayusculas porque es como las escribe el Kotlin del movil.
/// Lo que no se reconozca cae en `Otra`, con su palabra dentro: una clase
/// nueva del movil tiene que poder leerse aqui, aunque sea para decir que no
/// se sabe ensenarla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Clase {
    #[serde(rename = "NOTA")]
    Nota,
    #[serde(rename = "IMAGEN")]
    Imagen,
    #[serde(rename = "ARCHIVO")]
    Archivo,
    #[serde(rename = "VOZ")]
    Voz,
    #[serde(rename = "DIBUJO")]
    Dibujo,
    #[serde(rename = "PAGINA")]
    Pagina,
    /// Un proyecto entero, como acceso directo. No copia nada.
    #[serde(rename = "PROYECTO")]
    Proyecto,
    /// Una mini-aplicacion: una lista de tareas, unos gastos. El documento
    /// entero va en `texto`.
    #[serde(rename = "MINIAPP")]
    MiniApp,
    #[serde(untagged)]
    Otra(String),
}

/// Las secciones del cuaderno, las mismas que `Seccion` en PixPin Android.
///
/// Viven aqui y no en la interfaz porque son del dominio: que una nota de
/// voz sea «voz» no depende de como se pinte. La interfaz solo decide en
/// que orden se ensenan y con que aspecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seccion {
    /// La galeria del proyecto: **sus hojas**, en cuadricula y con su vista
    /// previa. Es la pantalla de proyectos de Android (`ui/Proyectos.kt`),
    /// que recorre `proyecto.hojas` y no la carpeta del disco.
    ///
    /// Un PDF importado deja UNA HOJA POR PAGINA (`motor/Proyectos.kt`,
    /// `dePdf`), asi que aqui salen todas sus paginas aunque no se haya
    /// dibujado en ellas. Lo que no es hoja —un `.html` suelto, una nota de
    /// voz, un archivo del chat— NO sale: el usuario lo pidio asi, «en la
    /// galeria solo aparezca lo mismo que aparece en la seccion de proyectos
    /// de Android».
    ///
    /// Es la primera porque es la que se abre al pulsar el titulo: lo que se
    /// quiere al entrar es VER lo que hay, no leer una lista.
    Galeria,
    Todo,
    Fotos,
    /// Los videos, aparte de las fotos (25-oct-2026, la ficha del chat al
    /// estilo de Telegram: «chat, fotos, videos, archivos, enlaces»). Son
    /// los ficheros con extension de video, lleguen como foto o como archivo.
    Videos,
    Archivos,
    /// Lo que lleva una direccion web en su texto (`http://`, `https://` o
    /// `www.`), como «Enlaces» de Telegram.
    Enlaces,
    Voz,
    Dibujos,
    Fijados,
    Buzon,
}

/// Las extensiones de video: las mismas que la familia «video» de los
/// colores por extension (`pixpin_ui::color_de_extension`).
const EXTENSIONES_DE_VIDEO: [&str; 9] = [
    "mp4", "mov", "avi", "mkv", "webm", "3gp", "m4v", "wmv", "flv",
];

/// Si un mensaje es un video: una foto o un archivo con extension de video.
pub fn es_video(m: &Mensaje) -> bool {
    matches!(m.clase, Some(Clase::Imagen | Clase::Archivo))
        && std::path::Path::new(&m.nombre)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| {
                EXTENSIONES_DE_VIDEO
                    .iter()
                    .any(|v| v.eq_ignore_ascii_case(e))
            })
}

/// Si el texto lleva una direccion web: una palabra que empieza por
/// `http://`, `https://` o `www.` y tiene un punto en el servidor. Lo justo
/// para la seccion «Enlaces»; sacar cada enlace con su sitio es cosa de quien
/// los pinta.
pub fn tiene_enlace(texto: &str) -> bool {
    texto.split_whitespace().any(|palabra| {
        let p = palabra
            .trim_start_matches(['(', '[', '<', '«', '"', '\''])
            .to_ascii_lowercase();
        let sin = ["https://", "http://", "www."]
            .iter()
            .find_map(|i| p.strip_prefix(i));
        sin.is_some_and(|resto| {
            let servidor = resto.split(['/', '?', '#']).next().unwrap_or("");
            let servidor = servidor.trim_end_matches(['.', ',', ';', ':', ')', ']', '>', '»', '"']);
            servidor.contains('.') && !servidor.starts_with('.')
        })
    })
}

impl Seccion {
    pub const TODAS: [Seccion; 10] = [
        Seccion::Galeria,
        Seccion::Todo,
        Seccion::Fotos,
        Seccion::Videos,
        Seccion::Archivos,
        Seccion::Enlaces,
        Seccion::Voz,
        Seccion::Dibujos,
        Seccion::Fijados,
        Seccion::Buzon,
    ];

    /// La clave de su nombre traducido.
    pub fn clave(self) -> &'static str {
        match self {
            Seccion::Galeria => "info-galeria",
            Seccion::Todo => "info-todo",
            Seccion::Fotos => "info-fotos",
            Seccion::Videos => "info-videos",
            Seccion::Archivos => "info-archivos",
            Seccion::Enlaces => "info-enlaces",
            Seccion::Voz => "info-voz",
            Seccion::Dibujos => "info-dibujos",
            Seccion::Fijados => "info-fijados",
            Seccion::Buzon => "info-buzon",
        }
    }

    /// Si se ensena como cuadricula. Lo que se mira va en cuadricula; lo que
    /// se lee, en filas.
    pub fn es_cuadricula(self) -> bool {
        matches!(
            self,
            Seccion::Galeria | Seccion::Fotos | Seccion::Videos | Seccion::Dibujos
        )
    }
}

/// Si un mensaje es un PDF anadido al proyecto.
///
/// Por la extension del nombre y no por la clase: un PDF llega como
/// `Clase::Archivo`, igual que un .docx o un .zip, y lo unico que lo
/// distingue es como se llama.
pub fn es_pdf(m: &Mensaje) -> bool {
    m.clase == Some(Clase::Archivo)
        && std::path::Path::new(&m.nombre)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Un mensaje del cuaderno.
///
/// Todo opcional salvo lo que identifica: el movil anade campos con el
/// tiempo, y un cuaderno escrito por una version mas nueva tiene que abrir
/// igual. Es la misma regla que los ajustes, el indice y el proyecto.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Mensaje {
    pub id: String,
    /// Milisegundos desde 1970.
    pub cuando: i64,
    pub clase: Option<Clase>,
    pub texto: String,
    /// Ruta en el APARATO de origen. En Windows no significa nada por si
    /// sola; sirve para saber que fichero acompanaba al mensaje.
    pub ruta: Option<String>,
    pub nombre: String,
    pub bytes: i64,
    pub referencia: Option<String>,
    /// Pagina dentro de un PDF, cuando la clase es `Pagina`.
    pub pagina: Option<u32>,
    /// Duracion del audio en milisegundos, en una nota de voz.
    #[serde(rename = "duracionMs")]
    pub duracion_ms: i64,
    /// De que conversacion es. `None` es la general — y `None` y no cadena
    /// vacia porque es lo que ya tenian escrito los mensajes de antes de que
    /// existieran los proyectos.
    pub proyecto: Option<String>,
    /// La etiqueta que se le puso: un emoji, o nada.
    pub emoji: Option<String>,
    pub fijado: bool,
    #[serde(rename = "enBuzon")]
    pub en_buzon: bool,
    /// El texto de una nota de voz, si ya se paso a texto.
    pub transcripcion: Option<String>,
    /// De que tipo es una mini-aplicacion. Se guarda la palabra y no un
    /// numero: un numero cambia de significado en cuanto alguien reordena la
    /// lista, y lo guardado no se puede reordenar.
    pub miniapp: Option<String>,
    #[serde(rename = "respondeA")]
    pub responde_a: Option<String>,
    /// Los tres codigos (Android v0.50): el numero en su conversacion, la
    /// letra del aparato de antes, el codigo unico, el aparato donde nacio y
    /// de donde se copio.
    pub numero: i64,
    pub letra: Option<String>,
    pub uid: Option<String>,
    pub aparato: Option<String>,
    pub origen: Option<String>,
    /// Lo que no se entiende, tal cual: guardar no puede perder lo que
    /// anada una version de Android.
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

impl Mensaje {
    pub fn codigo_unico(&self) -> String {
        crate::codigos::unico(self.uid.as_deref(), "m:", &self.id)
    }

    /// `47·K7Q2`, `47a` o nada.
    pub fn codigo_chat(&self) -> Option<String> {
        crate::codigos::de_chat(self.numero, self.aparato.as_deref(), self.letra.as_deref())
    }

    /// Si dos mensajes son la misma cosa: los tres codigos iguales.
    pub fn mismo_que(&self, otro: &Mensaje) -> bool {
        self.codigo_unico() == otro.codigo_unico()
            && self.codigo_chat().is_some()
            && self.codigo_chat() == otro.codigo_chat()
            && self.cuando == otro.cuando
    }

    /// Lo que se ensena de un mensaje en una linea.
    ///
    /// Para una nota de voz se prefiere su transcripcion: el texto de un
    /// mensaje de voz suele estar vacio, y ensenar el nombre del fichero de
    /// audio no le dice nada a nadie.
    pub fn resumen(&self) -> String {
        if let Some(t) = self.transcripcion.as_ref().filter(|t| !t.trim().is_empty()) {
            return t.clone();
        }
        if !self.texto.trim().is_empty() {
            return sin_rutas_de_fotos(&self.texto);
        }
        if !self.nombre.trim().is_empty() {
            return self.nombre.clone();
        }
        String::new()
    }
}

/// El texto de una nota con sus fotos (`![alt](pixpin:files/…)`, la nota
/// rica del chat) dichas como «🖼 alt»: en una linea la ruta no le dice
/// nada a nadie y se comeria todo el sitio. Lo demas, tal cual.
pub fn sin_rutas_de_fotos(texto: &str) -> String {
    if !texto.contains("![") {
        return texto.to_string();
    }
    let mut salida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(i) = resto.find("![") {
        salida.push_str(&resto[..i]);
        let tras = &resto[i + 2..];
        let cierre = tras
            .find("](")
            .filter(|&c| !tras[..c].contains('\n'))
            .and_then(|c| Some((c, tras[c + 2..].find(')')?)));
        match cierre {
            Some((c, f)) => {
                let alt = tras[..c].split('|').next().unwrap_or_default().trim();
                salida.push_str(if alt.is_empty() { "🖼" } else { "🖼 " });
                salida.push_str(alt);
                resto = &tras[c + 2 + f + 1..];
            }
            None => {
                salida.push_str("![");
                resto = tras;
            }
        }
    }
    salida.push_str(resto);
    salida
}

/// Quien escribe, cuando y en que conversacion: lo que sella un mensaje
/// nacido en este equipo. Van juntos porque siempre viajan juntos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sello {
    /// Milisegundos desde 1970.
    pub cuando: i64,
    /// El siguiente de su conversacion (ver `Cuaderno::siguiente_numero`).
    pub numero: i64,
    /// El codigo de este equipo (`K7Q2`).
    pub aparato: String,
    pub proyecto: String,
}

impl Mensaje {
    /// Una nota escrita aqui, con sus tres codigos ya puestos.
    ///
    /// `numero` es el siguiente de su conversacion (ver
    /// `Cuaderno::siguiente_numero`) y `aparato` el codigo de este equipo:
    /// juntos hacen el codigo de chat (`47·K7Q2`) que ensena PixPin Android.
    pub fn nota(texto: &str, sello: &Sello) -> Mensaje {
        Mensaje {
            // El id lleva la hora, como en el movil: ordena solo y no choca
            // con los que ya hay.
            id: format!("{}", sello.cuando),
            cuando: sello.cuando,
            clase: Some(Clase::Nota),
            texto: texto.to_string(),
            numero: sello.numero,
            uid: Some(crate::codigos::nuevo()),
            aparato: Some(sello.aparato.clone()),
            proyecto: Some(sello.proyecto.clone()),
            ..Default::default()
        }
    }

    /// Una mini-aplicacion: el documento entero va dentro del mensaje.
    ///
    /// Es el hueco que dejo PixPin Android para lo que no conoce: ante una
    /// palabra de `miniapp` que no reconoce ensena el texto tal cual, asi que
    /// una hoja de calculo hecha aqui llega al movil sin romperle nada. Por
    /// eso el documento va en `texto` y no en un fichero aparte: un fichero
    /// que el movil no sabe abrir seria un mensaje vacio.
    pub fn miniapp(cual: &str, nombre: &str, documento: &str, sello: &Sello) -> Mensaje {
        Mensaje {
            id: format!("{}", sello.cuando),
            cuando: sello.cuando,
            clase: Some(Clase::MiniApp),
            miniapp: Some(cual.to_string()),
            nombre: nombre.to_string(),
            texto: documento.to_string(),
            numero: sello.numero,
            uid: Some(crate::codigos::nuevo()),
            aparato: Some(sello.aparato.clone()),
            proyecto: Some(sello.proyecto.clone()),
            ..Default::default()
        }
    }

    /// Un mensaje con un fichero dentro del proyecto.
    ///
    /// `ruta` es relativa a la carpeta del proyecto (`archivos/foto.jpg`), no
    /// del escritorio: una ruta absoluta de este equipo no significa nada en
    /// el movil, y el proyecto tiene que poder viajar con sus ficheros.
    pub fn adjunto(clase: Clase, nombre: &str, ruta: &str, bytes: i64, sello: &Sello) -> Mensaje {
        Mensaje {
            id: format!("{}", sello.cuando),
            cuando: sello.cuando,
            clase: Some(clase),
            nombre: nombre.to_string(),
            ruta: Some(ruta.to_string()),
            bytes,
            numero: sello.numero,
            uid: Some(crate::codigos::nuevo()),
            aparato: Some(sello.aparato.clone()),
            proyecto: Some(sello.proyecto.clone()),
            ..Default::default()
        }
    }
}

/// De que clase es un fichero segun su extension. Lo que no se reconoce es
/// un archivo y ya: fingir que un `.xyz` es una imagen solo lleva a que el
/// visor falle al abrirlo.
pub fn clase_de_nombre(nombre: &str) -> Clase {
    let extension = nombre.rsplit_once('.').map(|(_, e)| e.to_lowercase());
    match extension.as_deref() {
        Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tif" | "tiff" | "avif") => {
            Clase::Imagen
        }
        Some("m4a" | "mp3" | "ogg" | "opus" | "wav" | "aac" | "flac") => Clase::Voz,
        _ => Clase::Archivo,
    }
}

/// El cerrojo de los cuadernos: todo lo que escribe en un `guardados.jsonl`
/// lo toma mientras escribe.
///
/// Hace falta porque reescribir (leer entero, cambiar, renombrar encima)
/// y anadir al final se hacen desde hilos distintos —el chat, el editor de
/// notas, los recordatorios, lo que llega del movil—: una linea anadida
/// entre la lectura y el renombrado de otro hilo desaparecia sin aviso. Uno
/// solo para todos los cuadernos, porque escribir es raro y corto y asi no
/// hay que llevar la cuenta de carpetas. Un hilo que murio con el cerrojo
/// tomado no deja a los demas sin poder guardar: se sigue con el.
pub fn cerrojo() -> std::sync::MutexGuard<'static, ()> {
    static CERROJO: std::sync::Mutex<()> = std::sync::Mutex::new(());
    CERROJO.lock().unwrap_or_else(|e| e.into_inner())
}

/// Anade un mensaje al cuaderno de una carpeta: **una linea al final**, sin
/// releer ni reescribir nada. Guardar cuesta lo mismo con diez mensajes que
/// con diez mil, y un corte a mitad se lleva ese mensaje y no el cuaderno.
pub fn anadir(carpeta: &std::path::Path, m: &Mensaje) -> std::io::Result<()> {
    use std::io::Write;
    let _cerrojo = cerrojo();
    std::fs::create_dir_all(carpeta)?;
    let linea = serde_json::to_string(m).map_err(std::io::Error::other)?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(carpeta.join("guardados.jsonl"))?;
    // El salto va DESPUES de la linea: si el fichero se corta, lo roto es lo
    // ultimo y lo anterior sigue entero.
    writeln!(f, "{linea}")
}

/// La linea que [`anadir`] escribiria para `m`, sin escribirla: quien tiene
/// mensajes enormes (una tabla de un Excel) la prepara en otro hilo y luego
/// solo la pega con [`anadir_lineas`].
pub fn linea_de(m: &Mensaje) -> std::io::Result<String> {
    serde_json::to_string(m).map_err(std::io::Error::other)
}

/// Anade varias lineas ya hechas con [`linea_de`], de una sola escritura.
pub fn anadir_lineas(carpeta: &std::path::Path, lineas: &[String]) -> std::io::Result<()> {
    use std::io::Write;
    let _cerrojo = cerrojo();
    std::fs::create_dir_all(carpeta)?;
    let mut todo = String::with_capacity(lineas.iter().map(|l| l.len() + 1).sum());
    for l in lineas {
        todo.push_str(l);
        todo.push('\n');
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(carpeta.join("guardados.jsonl"))?;
    f.write_all(todo.as_bytes())
}

/// Cambia un mensaje que ya estaba escrito, dejando el resto del fichero como
/// estaba. Devuelve si lo encontro.
///
/// Es lo contrario de [`anadir`] y cuesta lo que el cuaderno entero, asi que
/// solo se usa para lo que de verdad se edita en el sitio: el documento de
/// una mini-aplicacion —una tabla, por ejemplo—, que ES el texto del mensaje.
/// Un mensaje nuevo por cada tecla convertiria la conversacion en un diario
/// de pulsaciones.
///
/// **Las lineas que no se entienden se copian tal cual**, sin tocarlas: una
/// version mas nueva del movil puede escribir cosas que aqui no se leen, y
/// reescribir el fichero no puede ser la forma de perderlas. Se escribe a un
/// temporal y se renombra: un corte a mitad deja el cuaderno anterior entero.
pub fn reemplazar(carpeta: &std::path::Path, m: &Mensaje) -> std::io::Result<bool> {
    let _cerrojo = cerrojo();
    let fichero = carpeta.join("guardados.jsonl");
    let texto = std::fs::read_to_string(&fichero)?;
    reescribir_con_el(&fichero, &texto, m)
}

/// Lee el mensaje `id`, deja que `cambio` lo toque y, si dice que cambio,
/// lo reescribe: **todo con el cerrojo tomado**, para que nadie escriba en
/// medio. Es lo que necesita quien edita un mensaje desde fuera del chat
/// (la lista de tareas sacada a la pantalla): leer, cambiar y guardar por
/// separado perderia lo que el chat guardase entre medias.
///
/// Devuelve el mensaje como quedo, o `None` si ya no esta (se borro). Un
/// `cambio` que devuelve `false` no reescribe nada. `cambio` no puede tocar
/// el cuaderno: el cerrojo no es reentrante.
pub fn cambiar(
    carpeta: &std::path::Path,
    id: &str,
    cambio: impl FnOnce(&mut Mensaje) -> bool,
) -> std::io::Result<Option<Mensaje>> {
    let _cerrojo = cerrojo();
    let fichero = carpeta.join("guardados.jsonl");
    let texto = std::fs::read_to_string(&fichero)?;
    let Some(mut m) = texto
        .lines()
        .filter_map(|l| serde_json::from_str::<Mensaje>(l).ok())
        .find(|m| m.id == id)
    else {
        return Ok(None);
    };
    if cambio(&mut m) {
        reescribir_con_el(&fichero, &texto, &m)?;
    }
    Ok(Some(m))
}

/// Quita del cuaderno los mensajes cuyo `id` este en `ids` y devuelve los
/// que quito, tal como estaban: quien borra tiene que dejar su marca de
/// borrado con ellos (`vista::anotar_borrados`), o la siguiente vuelta de
/// sincronizar los traeria otra vez del otro aparato.
///
/// Es lo mismo que hace el chat al borrar (`quitar_del_cuaderno`) y la ficha
/// de una leccion: lo que no se entiende se copia tal cual —borrar no puede
/// ser la forma de perder lo que escribio una version mas nueva del movil—
/// y se escribe a un temporal que se renombra, con el cerrojo tomado.
pub fn quitar(
    carpeta: &std::path::Path,
    ids: &std::collections::BTreeSet<String>,
) -> std::io::Result<Vec<Mensaje>> {
    let _cerrojo = cerrojo();
    let fichero = carpeta.join("guardados.jsonl");
    let texto = std::fs::read_to_string(&fichero)?;
    let mut salida = String::with_capacity(texto.len());
    let mut quitados = Vec::new();
    for linea in texto.lines() {
        match serde_json::from_str::<Mensaje>(linea) {
            Ok(m) if ids.contains(&m.id) => quitados.push(m),
            _ => {
                salida.push_str(linea);
                salida.push('\n');
            }
        }
    }
    if !quitados.is_empty() {
        let temporal = fichero.with_extension("jsonl.tmp");
        std::fs::write(&temporal, salida)?;
        std::fs::rename(&temporal, &fichero)?;
    }
    Ok(quitados)
}

/// El cuaderno `texto` con la linea de `m` cambiada por la nueva, escrito en
/// `fichero`. Quien llama tiene el cerrojo.
fn reescribir_con_el(fichero: &std::path::Path, texto: &str, m: &Mensaje) -> std::io::Result<bool> {
    let nueva = serde_json::to_string(m).map_err(std::io::Error::other)?;
    let mut salida = String::with_capacity(texto.len() + nueva.len());
    let mut encontrado = false;
    for linea in texto.lines() {
        let suya = serde_json::from_str::<Mensaje>(linea).is_ok_and(|otro| otro.id == m.id);
        if suya {
            salida.push_str(&nueva);
            encontrado = true;
        } else {
            salida.push_str(linea);
        }
        salida.push('\n');
    }
    if !encontrado {
        return Ok(false);
    }
    let temporal = fichero.with_extension("jsonl.tmp");
    std::fs::write(&temporal, salida)?;
    std::fs::rename(&temporal, fichero)?;
    Ok(true)
}

/// Un cuaderno leido.
#[derive(Debug, Clone, Default)]
pub struct Cuaderno {
    pub mensajes: Vec<Mensaje>,
    /// Cuantas lineas no se pudieron entender.
    ///
    /// Se cuentan y se ensenan en vez de callarlas: si un cuaderno de mil
    /// mensajes ensena novecientos, el usuario tiene que enterarse de que
    /// faltan cien y no creer que nunca existieron.
    pub lineas_rotas: usize,
}

impl Cuaderno {
    /// Lee un cuaderno de su texto.
    ///
    /// Nunca falla: un cuaderno es una lista de lineas independientes, y la
    /// unica respuesta razonable a una linea rota es saltarsela.
    pub fn leer(texto: &str) -> Cuaderno {
        let mut mensajes = Vec::new();
        let mut lineas_rotas = 0;
        for linea in texto.lines() {
            if linea.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Mensaje>(linea) {
                Ok(m) => mensajes.push(m),
                Err(_) => lineas_rotas += 1,
            }
        }
        Cuaderno {
            mensajes,
            lineas_rotas,
        }
    }

    /// Lee el `guardados.jsonl` de una carpeta.
    pub fn leer_de(carpeta: &std::path::Path) -> std::io::Result<Cuaderno> {
        let texto = std::fs::read_to_string(carpeta.join("guardados.jsonl"))?;
        Ok(Cuaderno::leer(&texto))
    }

    /// El numero que le toca al siguiente mensaje de una conversacion.
    ///
    /// Se mira el mayor y se suma uno, en vez de contar cuantos hay: un
    /// mensaje borrado dejaria hueco y dos mensajes distintos acabarian con
    /// el mismo numero, que es justo lo que los tres codigos evitan.
    pub fn siguiente_numero(&self, proyecto: Option<&str>) -> i64 {
        self.de_conversacion(proyecto)
            .iter()
            .map(|m| m.numero)
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Los mensajes de una seccion, en el orden en que estan.
    ///
    /// Las secciones son las de PixPin Android. Dos criterios que no son
    /// evidentes: el BUZON es un aparte y no sale en «todo» (es donde caducan
    /// cosas a los siete dias), y una PAGINA de plano cuenta como archivo,
    /// porque quien busca un plano lo busca ahi y no le importa de que clase
    /// es por dentro.
    pub fn de_seccion(&self, seccion: Seccion) -> Vec<&Mensaje> {
        indices_de_seccion(&self.mensajes, seccion)
            .into_iter()
            .map(|n| &self.mensajes[n])
            .collect()
    }

    /// Los mensajes de una conversacion. `None` es la general.
    pub fn de_conversacion(&self, proyecto: Option<&str>) -> Vec<&Mensaje> {
        self.mensajes
            .iter()
            .filter(|m| m.proyecto.as_deref() == proyecto)
            .collect()
    }
}

/// El principio del id de los **mensajes de sistema** (14-oct-2026): los
/// ARCHIVO fijos del chat «Asistente» que hacen viajar ficheros entre los
/// aparatos propios (`auto-letra-*`, `auto-figuras`,
/// `auto-automatizaciones`). Existen y se sincronizan como cualquier otro,
/// pero no son conversacion: no salen como burbujas, ni en las secciones,
/// ni en la vista previa de la lista.
pub const PREFIJO_DE_SISTEMA: &str = "auto-";

/// **El filtro unico de los mensajes de sistema**: por su id, nunca por lo
/// que dicen (una nota cuyo texto empieza por «auto-» es una nota).
pub fn es_de_sistema(m: &Mensaje) -> bool {
    m.id.starts_with(PREFIJO_DE_SISTEMA)
}

/// QUE mensajes son de una seccion, por su posicion en la lista.
///
/// Va aparte de `Cuaderno::de_seccion` porque quien pinta necesita el numero
/// y no solo el mensaje: con el llega a lo que guarda en paralelo —el lienzo
/// ya leido de cada dibujo, por ejemplo— sin tener que buscarlo.
///
/// Es una funcion libre y no un metodo para poder llamarla con un
/// `&[Mensaje]` prestado: el panel la usa en cada fotograma, y montar un
/// `Cuaderno` solo para preguntar obliga a clonar la conversacion entera.
pub fn indices_de_seccion(mensajes: &[Mensaje], seccion: Seccion) -> Vec<usize> {
    mensajes
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            if es_de_sistema(m) {
                return false;
            }
            if m.en_buzon {
                return seccion == Seccion::Buzon;
            }
            match seccion {
                // La galeria son las HOJAS del proyecto, como la pantalla de
                // proyectos de Android. Quien tiene el `proyecto.json` a mano
                // filtra ademas por el codigo unico de cada hoja, que es
                // exacto; aqui se hace lo que se puede sin el: las clases que
                // una hoja produce. Un archivo suelto o una nota de voz no
                // son hojas y no salen.
                Seccion::Galeria => matches!(m.clase, Some(Clase::Dibujo | Clase::Pagina)),
                Seccion::Todo => true,
                Seccion::Buzon => false,
                Seccion::Fijados => m.fijado,
                Seccion::Fotos => m.clase == Some(Clase::Imagen) && !es_video(m),
                Seccion::Videos => es_video(m),
                Seccion::Enlaces => tiene_enlace(&m.texto),
                // Las cuatro que pone Android en esta seccion
                // (`Seccion.ARCHIVOS` en `Mensajes.kt`): lo que se abre
                // aparte. Una pagina de plano, un acceso a otro proyecto y
                // una mini-aplicacion se buscan aqui, y quien las busca no
                // piensa en de que clase son por dentro.
                Seccion::Archivos => {
                    matches!(
                        m.clase,
                        Some(Clase::Archivo | Clase::Pagina | Clase::Proyecto | Clase::MiniApp)
                    ) && !es_video(m)
                }
                Seccion::Voz => m.clase == Some(Clase::Voz),
                Seccion::Dibujos => m.clase == Some(Clase::Dibujo),
            }
        })
        .map(|(n, _)| n)
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta_temporal(etiqueta: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("pixpin-cuaderno-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn anadir_mientras_otro_hilo_reemplaza_no_pierde_ningun_mensaje() {
        let d = carpeta_temporal("a-la-vez");
        let mut fijo = Mensaje::nota("se edita", &sello(1, 1));
        fijo.id = "fijo".into();
        anadir(&d, &fijo).unwrap();
        let hecho = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let editor = {
            let (d, hecho, mut fijo) = (d.clone(), hecho.clone(), fijo.clone());
            std::thread::spawn(move || {
                let mut n = 0;
                while !hecho.load(std::sync::atomic::Ordering::Relaxed) {
                    n += 1;
                    fijo.texto = format!("vuelta {n}");
                    reemplazar(&d, &fijo).unwrap();
                }
            })
        };
        for n in 0..60 {
            let mut m = Mensaje::nota("nuevo", &sello(2, 2));
            m.id = format!("n{n}");
            anadir(&d, &m).unwrap();
        }
        hecho.store(true, std::sync::atomic::Ordering::Relaxed);
        editor.join().unwrap();
        let leido = Cuaderno::leer(&std::fs::read_to_string(d.join("guardados.jsonl")).unwrap());
        assert_eq!(leido.mensajes.len(), 61, "se perdieron mensajes");
        assert_eq!(leido.lineas_rotas, 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn reemplazar_cambia_solo_su_linea_y_conserva_lo_que_no_se_entiende() {
        let d = carpeta_temporal("reemplazar");
        let mut uno = Mensaje::nota("primero", &sello(1, 1));
        uno.id = "m1".into();
        let mut dos = Mensaje::nota("segundo", &sello(2, 2));
        dos.id = "m2".into();
        anadir(&d, &uno).unwrap();
        // Una linea de una version mas nueva, que aqui no se entiende.
        std::fs::OpenOptions::new()
            .append(true)
            .open(d.join("guardados.jsonl"))
            .map(|mut f| std::io::Write::write_all(&mut f, b"{lo que venga del futuro}\n"))
            .unwrap()
            .unwrap();
        anadir(&d, &dos).unwrap();

        dos.texto = "segundo, corregido".into();
        assert!(reemplazar(&d, &dos).unwrap());

        let c = Cuaderno::leer_de(&d).unwrap();
        assert_eq!(c.mensajes.len(), 2);
        assert_eq!(c.mensajes[0].texto, "primero", "el otro no se toca");
        assert_eq!(c.mensajes[1].texto, "segundo, corregido");
        assert_eq!(c.lineas_rotas, 1, "la linea rara sigue ahi");

        // Caso negativo: un mensaje que no esta no escribe nada.
        let mut ajeno = Mensaje::nota("de otro sitio", &sello(3, 3));
        ajeno.id = "m9".into();
        assert!(!reemplazar(&d, &ajeno).unwrap());
        assert_eq!(Cuaderno::leer_de(&d).unwrap().mensajes.len(), 2);
    }

    #[test]
    fn quitar_saca_solo_los_pedidos_y_conserva_lo_que_no_se_entiende() {
        let d = carpeta_temporal("quitar");
        let mut uno = Mensaje::nota("primero", &sello(1, 1));
        uno.id = "m1".into();
        let mut dos = Mensaje::nota("segundo", &sello(2, 2));
        dos.id = "m2".into();
        anadir(&d, &uno).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(d.join("guardados.jsonl"))
            .map(|mut f| std::io::Write::write_all(&mut f, b"{lo que venga del futuro}\n"))
            .unwrap()
            .unwrap();
        anadir(&d, &dos).unwrap();

        let ids = std::collections::BTreeSet::from(["m2".to_string()]);
        let idos = quitar(&d, &ids).unwrap();
        assert_eq!(idos.len(), 1);
        assert_eq!(idos[0].texto, "segundo", "devuelve el quitado entero");
        let c = Cuaderno::leer_de(&d).unwrap();
        assert_eq!(c.mensajes.len(), 1);
        assert_eq!(c.mensajes[0].id, "m1");
        assert_eq!(c.lineas_rotas, 1, "la linea rara sigue ahi");
        // Caso negativo: lo que ya no esta no quita ni reescribe nada.
        assert!(quitar(&d, &ids).unwrap().is_empty());
        assert_eq!(Cuaderno::leer_de(&d).unwrap().mensajes.len(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn cambiar_lee_toca_y_guarda_de_una_vez_y_solo_si_cambio() {
        let d = carpeta_temporal("cambiar");
        let mut lista = Mensaje::nota("# L\n\n- [ ] pan", &sello(1, 1));
        lista.id = "l1".into();
        anadir(&d, &lista).unwrap();
        let hecho = cambiar(&d, "l1", |m| {
            m.texto.push_str("\n- [ ] sal");
            true
        })
        .unwrap()
        .unwrap();
        assert_eq!(hecho.texto, "# L\n\n- [ ] pan\n- [ ] sal");
        assert_eq!(
            Cuaderno::leer_de(&d).unwrap().mensajes[0].texto,
            hecho.texto
        );
        // Sin cambio no se reescribe: el fichero sigue siendo el mismo.
        let antes = std::fs::read_to_string(d.join("guardados.jsonl")).unwrap();
        let igual = cambiar(&d, "l1", |_| false).unwrap().unwrap();
        assert_eq!(igual.texto, hecho.texto);
        assert_eq!(
            std::fs::read_to_string(d.join("guardados.jsonl")).unwrap(),
            antes
        );
        // Caso negativo: uno que ya no esta no se inventa ni escribe nada.
        assert!(cambiar(&d, "borrado", |_| true).unwrap().is_none());
        assert_eq!(
            std::fs::read_to_string(d.join("guardados.jsonl")).unwrap(),
            antes
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    fn sello(cuando: i64, numero: i64) -> Sello {
        Sello {
            cuando,
            numero,
            aparato: "K7Q2".into(),
            proyecto: "pr-1".into(),
        }
    }

    #[test]
    fn un_mensaje_con_sus_tres_codigos_se_reconoce_y_no_pierde_campos() {
        let original = r#"{"id":"1757939357123","cuando":1757939357123,"texto":"hola","numero":47,"uid":"RVK5YHKCX7","aparato":"K7Q2","recibidoDe":"Max phone · K7Q2","vieneDe":{"x":1}}"#;
        let m: Mensaje = serde_json::from_str(original).unwrap();
        assert_eq!(m.codigo_chat().as_deref(), Some("47·K7Q2"));
        assert_eq!(m.codigo_unico(), "RVK5YHKCX7");
        let mut otro = m.clone();
        assert!(m.mismo_que(&otro));
        otro.cuando += 1;
        assert!(!m.mismo_que(&otro), "otra fecha: no es el mismo");
        let vuelta = serde_json::to_value(&m).unwrap();
        for campo in ["recibidoDe", "vieneDe", "numero", "uid", "aparato"] {
            assert!(vuelta.get(campo).is_some(), "se perdio {campo}");
        }
    }

    fn cuaderno_de_prueba() -> &'static str {
        concat!(
            r#"{"id":"m1","cuando":1725500000000,"clase":"NOTA","texto":"comprar cemento"}"#,
            "\n",
            r#"{"id":"m2","cuando":1725500001000,"clase":"VOZ","nombre":"voz-1.m4a","duracionMs":4200,"transcripcion":"llamar al aparejador"}"#,
            "\n",
            r#"{"id":"m3","cuando":1725500002000,"clase":"IMAGEN","nombre":"fachada.jpg","bytes":204800,"proyecto":"pr-1","emoji":"P","fijado":true}"#,
            "\n",
            r#"{"id":"m4","cuando":1725500003000,"clase":"COSA_NUEVA","texto":"de una version futura"}"#,
            "\n"
        )
    }

    #[test]
    fn se_leen_los_mensajes_con_sus_campos() {
        let c = Cuaderno::leer(cuaderno_de_prueba());
        assert_eq!(c.mensajes.len(), 4);
        assert_eq!(c.lineas_rotas, 0);
        assert_eq!(c.mensajes[0].clase, Some(Clase::Nota));
        assert_eq!(c.mensajes[0].texto, "comprar cemento");
        assert_eq!(c.mensajes[2].emoji.as_deref(), Some("P"));
        assert!(c.mensajes[2].fijado);
        assert_eq!(c.mensajes[2].bytes, 204_800);
    }

    #[test]
    fn una_linea_rota_no_se_lleva_el_cuaderno() {
        // Es la razon de ser del formato: si un corte de luz a mitad de
        // escribir tumbara la lectura entera, el usuario perderia todo el
        // cuaderno en vez de un mensaje.
        let texto = concat!(
            r#"{"id":"a","cuando":1,"clase":"NOTA","texto":"antes"}"#,
            "\n",
            r#"{"id":"b","cuando":2,"clase":"NOT"#,
            "\n",
            r#"{"id":"c","cuando":3,"clase":"NOTA","texto":"despues"}"#,
            "\n"
        );
        let c = Cuaderno::leer(texto);
        assert_eq!(c.mensajes.len(), 2, "se perdieron los buenos");
        assert_eq!(c.lineas_rotas, 1);
        assert_eq!(c.mensajes[1].texto, "despues");
    }

    #[test]
    fn las_lineas_rotas_se_cuentan_y_no_se_callan() {
        // Si un cuaderno de mil ensena novecientos, el usuario tiene que
        // enterarse de que faltan cien y no creer que nunca existieron.
        let c = Cuaderno::leer("{roto\n{tambien roto\n");
        assert!(c.mensajes.is_empty());
        assert_eq!(c.lineas_rotas, 2);
    }

    #[test]
    fn una_clase_que_no_conocemos_se_lee_igual() {
        // El movil anade clases con el tiempo. Rechazar la linea entera por
        // una palabra desconocida perderia su texto, que si se entiende.
        let c = Cuaderno::leer(cuaderno_de_prueba());
        assert_eq!(
            c.mensajes[3].clase,
            Some(Clase::Otra("COSA_NUEVA".into())),
            "una clase futura tiene que llegar con su nombre"
        );
        assert_eq!(c.mensajes[3].texto, "de una version futura");
    }

    #[test]
    fn de_una_nota_de_voz_se_ensena_lo_que_dijo() {
        // El texto de un mensaje de voz suele venir vacio, y ensenar
        // «voz-1.m4a» no le dice nada a nadie.
        let c = Cuaderno::leer(cuaderno_de_prueba());
        assert_eq!(c.mensajes[1].resumen(), "llamar al aparejador");
        // Y sin transcripcion, al menos el nombre.
        let sin = Cuaderno::leer(r#"{"id":"z","clase":"VOZ","nombre":"voz-9.m4a"}"#);
        assert_eq!(sin.mensajes[0].resumen(), "voz-9.m4a");
        // Un mensaje sin nada no inventa texto.
        let vacio = Cuaderno::leer(r#"{"id":"z"}"#);
        assert_eq!(vacio.mensajes[0].resumen(), "");
    }

    #[test]
    fn de_una_nota_con_fotos_no_se_ensena_la_ruta() {
        let m = Mensaje {
            texto: "hola\n![](pixpin:files/guardados/pc/c/notas/1-a.png)\n![planta|300](notas/2.png) fin".into(),
            ..Default::default()
        };
        assert_eq!(m.resumen(), "hola\n🖼\n🖼 planta fin");
        // Caso negativo: un `![` sin cerrar se queda como se escribio.
        assert_eq!(sin_rutas_de_fotos("mira ![esto"), "mira ![esto");
    }

    #[test]
    fn cada_conversacion_tiene_lo_suyo() {
        let c = Cuaderno::leer(cuaderno_de_prueba());
        // La general son los que no llevan proyecto, y `None` no es lo mismo
        // que una cadena vacia: los mensajes de antes de que existieran los
        // proyectos siguen siendo de la general.
        assert_eq!(c.de_conversacion(None).len(), 3);
        assert_eq!(c.de_conversacion(Some("pr-1")).len(), 1);
        assert!(c.de_conversacion(Some("no-existe")).is_empty());
    }

    #[test]
    fn un_cuaderno_vacio_es_un_cuaderno() {
        // Caso negativo: cero mensajes no es un error, es un cuaderno recien
        // estrenado.
        let c = Cuaderno::leer("");
        assert!(c.mensajes.is_empty());
        assert_eq!(c.lineas_rotas, 0);
        // Y las lineas en blanco no cuentan como rotas.
        let d = Cuaderno::leer("\n\n   \n");
        assert_eq!(d.lineas_rotas, 0);
    }

    #[test]
    fn un_mensaje_al_que_le_faltan_campos_abre_igual() {
        // La misma regla que los ajustes: lo que falta toma su valor por
        // defecto en vez de tumbar la linea.
        let c = Cuaderno::leer(r#"{"id":"solo-id"}"#);
        assert_eq!(c.mensajes.len(), 1);
        assert_eq!(c.mensajes[0].id, "solo-id");
        assert_eq!(c.mensajes[0].cuando, 0);
        assert!(c.mensajes[0].clase.is_none());
        assert!(!c.mensajes[0].fijado);
    }

    fn carpeta(etiqueta: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("pixpin-cuaderno-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn escribir_una_nota_anade_una_linea_y_no_toca_las_de_antes() {
        let d = carpeta("anadir");
        let vieja = Mensaje::nota("lo de ayer", &sello(1_725_500_000_000, 1));
        anadir(&d, &vieja).unwrap();
        let antes = std::fs::read_to_string(d.join("guardados.jsonl")).unwrap();
        let nueva = Mensaje::nota("lo de hoy", &sello(1_725_600_000_000, 2));
        anadir(&d, &nueva).unwrap();
        let texto = std::fs::read_to_string(d.join("guardados.jsonl")).unwrap();
        assert!(texto.starts_with(&antes), "la linea de antes, intacta");
        assert_eq!(texto.lines().count(), 2);

        let c = Cuaderno::leer(&texto);
        assert_eq!(c.lineas_rotas, 0);
        assert_eq!(c.mensajes[1].texto, "lo de hoy");
        assert_eq!(c.mensajes[1].clase, Some(Clase::Nota));
        assert_eq!(c.mensajes[1].codigo_chat().as_deref(), Some("2·K7Q2"));
        assert_eq!(c.mensajes[1].codigo_unico().len(), crate::codigos::LARGO);
        // Dos notas seguidas no comparten codigo unico.
        assert_ne!(c.mensajes[0].codigo_unico(), c.mensajes[1].codigo_unico());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn el_numero_siguiente_es_el_mayor_mas_uno_y_va_por_conversacion() {
        let c = Cuaderno::leer(concat!(
            r#"{"id":"a","numero":7,"proyecto":"pr-1"}"#,
            "\n",
            r#"{"id":"b","numero":3,"proyecto":"pr-1"}"#,
            "\n",
            r#"{"id":"c","numero":41,"proyecto":"otro"}"#,
            "\n",
            r#"{"id":"d","numero":2}"#,
            "\n"
        ));
        assert_eq!(
            c.siguiente_numero(Some("pr-1")),
            8,
            "el mayor, no cuantos hay"
        );
        assert_eq!(c.siguiente_numero(Some("otro")), 42);
        assert_eq!(c.siguiente_numero(None), 3, "la general va aparte");
        // Una conversacion sin nada empieza por el uno.
        assert_eq!(c.siguiente_numero(Some("nueva")), 1);
    }

    #[test]
    fn una_linea_rota_al_final_no_se_lleva_las_buenas() {
        let d = carpeta("rota");
        anadir(&d, &Mensaje::nota("buena", &sello(1, 1))).unwrap();
        // Como si se hubiera cortado la luz a mitad de escribir la segunda.
        std::fs::write(
            d.join("guardados.jsonl"),
            format!(
                "{}{}",
                std::fs::read_to_string(d.join("guardados.jsonl")).unwrap(),
                r#"{"id":"a","cuando":"#
            ),
        )
        .unwrap();
        let c = Cuaderno::leer_de(&d).unwrap();
        assert_eq!(c.mensajes.len(), 1, "la buena se lee");
        assert_eq!(c.lineas_rotas, 1, "y la rota se cuenta, no se calla");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_clase_sale_de_la_extension_y_lo_raro_es_un_archivo() {
        assert_eq!(clase_de_nombre("fachada.JPG"), Clase::Imagen);
        assert_eq!(clase_de_nombre("plano.png"), Clase::Imagen);
        assert_eq!(clase_de_nombre("voz-1.m4a"), Clase::Voz);
        assert_eq!(clase_de_nombre("presupuesto.pdf"), Clase::Archivo);
        // Caso negativo: lo desconocido y lo que no tiene extension no se
        // fingen imagenes.
        assert_eq!(clase_de_nombre("cosa.xyz"), Clase::Archivo);
        assert_eq!(clase_de_nombre("LEEME"), Clase::Archivo);
    }

    #[test]
    fn un_adjunto_guarda_la_ruta_de_dentro_del_proyecto_no_la_del_escritorio() {
        let m = Mensaje::adjunto(
            Clase::Imagen,
            "fachada.jpg",
            "archivos/fachada.jpg",
            204_800,
            &sello(1_725_500_000_000, 4),
        );
        assert_eq!(m.ruta.as_deref(), Some("archivos/fachada.jpg"));
        assert!(!m.ruta.as_deref().unwrap().contains(':'), "nada absoluto");
        assert_eq!(m.nombre, "fachada.jpg");
        assert_eq!(m.bytes, 204_800);
        assert_eq!(m.codigo_chat().as_deref(), Some("4·K7Q2"));
        // Lo que se ensena de el es su nombre: no tiene texto.
        assert_eq!(m.resumen(), "fachada.jpg");
    }

    #[test]
    fn cada_seccion_se_queda_con_lo_suyo() {
        let c = Cuaderno::leer(concat!(
            r#"{"id":"n","clase":"NOTA","texto":"hola"}"#,
            "\n",
            r#"{"id":"i","clase":"IMAGEN","nombre":"a.jpg"}"#,
            "\n",
            r#"{"id":"a","clase":"ARCHIVO","nombre":"plano.pdf"}"#,
            "\n",
            r#"{"id":"v","clase":"VOZ","nombre":"v.m4a"}"#,
            "\n",
            r#"{"id":"d","clase":"DIBUJO","nombre":"croquis"}"#,
            "\n",
            r#"{"id":"p","clase":"PAGINA","nombre":"plano.pdf","fijado":true}"#,
            "\n",
            r#"{"id":"b","clase":"NOTA","texto":"caduca","enBuzon":true}"#,
            "\n"
        ));
        let ids =
            |s: Seccion| -> Vec<&str> { c.de_seccion(s).iter().map(|m| m.id.as_str()).collect() };
        // «Todo» es todo lo que no esta en el buzon: el buzon es un aparte,
        // no una etiqueta mas.
        assert_eq!(ids(Seccion::Todo), ["n", "i", "a", "v", "d", "p"]);
        assert_eq!(ids(Seccion::Fotos), ["i"]);
        // Una pagina de plano es un archivo para quien lo busca: lo que
        // importa es que se abre fuera, no de que clase es por dentro.
        assert_eq!(ids(Seccion::Archivos), ["a", "p"]);
        assert_eq!(ids(Seccion::Voz), ["v"]);
        assert_eq!(ids(Seccion::Dibujos), ["d"]);
        assert_eq!(ids(Seccion::Fijados), ["p"]);
        assert_eq!(ids(Seccion::Buzon), ["b"]);
    }

    #[test]
    fn los_videos_y_los_enlaces_tienen_su_seccion() {
        let c = Cuaderno::leer(concat!(
            r#"{"id":"f","clase":"IMAGEN","nombre":"a.jpg"}"#,
            "\n",
            r#"{"id":"v1","clase":"IMAGEN","nombre":"clip.MP4"}"#,
            "\n",
            r#"{"id":"v2","clase":"ARCHIVO","nombre":"obra.mov"}"#,
            "\n",
            r#"{"id":"a","clase":"ARCHIVO","nombre":"plano.pdf"}"#,
            "\n",
            r#"{"id":"e","clase":"NOTA","texto":"mira (https://ejemplo.com/x)"}"#,
            "\n",
            r#"{"id":"w","clase":"NOTA","texto":"en www.pixpin.app hoy"}"#,
            "\n",
            r#"{"id":"n","clase":"NOTA","texto":"sin enlace: http:// ni www."}"#,
            "\n"
        ));
        let ids =
            |s: Seccion| -> Vec<&str> { c.de_seccion(s).iter().map(|m| m.id.as_str()).collect() };
        assert_eq!(ids(Seccion::Videos), ["v1", "v2"]);
        // Caso negativo: un video no es una foto ni un archivo mas.
        assert_eq!(ids(Seccion::Fotos), ["f"]);
        assert_eq!(ids(Seccion::Archivos), ["a"]);
        assert_eq!(ids(Seccion::Enlaces), ["e", "w"]);
        assert!(Seccion::Videos.es_cuadricula());
        assert!(!Seccion::Enlaces.es_cuadricula());
    }

    #[test]
    fn una_mini_aplicacion_lleva_su_documento_dentro_del_mensaje() {
        let m = Mensaje::miniapp("tabla", "Mediciones", r#"{"nombre":"x"}"#, &sello(10, 1));
        assert_eq!(m.clase, Some(Clase::MiniApp));
        assert_eq!(m.miniapp.as_deref(), Some("tabla"));
        assert_eq!(m.texto, r#"{"nombre":"x"}"#);
        // El documento va en `texto`, NO en un fichero: un fichero que el
        // movil no sabe abrir seria un mensaje vacio alli.
        assert_eq!(m.ruta, None);
        assert_eq!(m.referencia, None);
        // Y sale en «archivos», que es donde Android pone las mini-apps.
        let c = Cuaderno {
            mensajes: vec![m],
            lineas_rotas: 0,
        };
        assert_eq!(c.de_seccion(Seccion::Archivos).len(), 1);
        assert!(c.de_seccion(Seccion::Fotos).is_empty());
    }

    #[test]
    fn la_galeria_ensena_el_proyecto_entero() {
        // El usuario lo corrigio: «quiero que se vea TODO lo que esta en el
        // proyecto en el celular Android». Nada se filtra; lo que cambia
        // respecto a `Todo` es que se mira en cuadricula.
        let c = Cuaderno::leer(concat!(
            r#"{"id":"d1","clase":"DIBUJO","nombre":"Tesis base"}"#,
            "\n",
            r#"{"id":"d2","clase":"DIBUJO","nombre":"↳ Zona de la página 1"}"#,
            "\n",
            r#"{"id":"p1","clase":"PAGINA","nombre":"GE_Sem16 · 5","pagina":5}"#,
            "\n",
            r#"{"id":"a1","clase":"ARCHIVO","nombre":"GE_Sem16.pdf"}"#,
            "\n",
            r#"{"id":"a2","clase":"ARCHIVO","nombre":"notas.docx"}"#,
            "\n",
            r#"{"id":"i1","clase":"IMAGEN","nombre":"foto.jpg"}"#,
            "\n",
            r#"{"id":"n1","clase":"NOTA","texto":"hola"}"#,
            "\n"
        ));
        let ids: Vec<&str> = c
            .de_seccion(Seccion::Galeria)
            .iter()
            .map(|m| m.id.as_str())
            .collect();
        // Lo que una hoja produce: lienzos, sub-lienzos y paginas de PDF.
        assert_eq!(ids, ["d1", "d2", "p1"]);
        // Caso negativo, y es lo que el usuario corrigio: un PDF suelto, un
        // .docx, una foto o una nota NO son hojas del proyecto, y la pantalla
        // de proyectos de Android tampoco los ensena.
        for fuera in ["a1", "a2", "i1", "n1"] {
            assert!(!ids.contains(&fuera), "no deberia estar {fuera}");
        }
        // Y va en cuadricula, que es lo que la hace una galeria.
        assert!(Seccion::Galeria.es_cuadricula());
        assert!(!Seccion::Todo.es_cuadricula());
    }

    #[test]
    fn la_galeria_es_la_primera_pestana_y_lo_del_buzon_no_se_cuela() {
        // Es la que se abre al pulsar el titulo del proyecto.
        assert_eq!(Seccion::TODAS[0], Seccion::Galeria);
        // Caso negativo: un lienzo en el buzon esta ahi para decidir si se
        // queda, y ensenarlo ya en la galeria seria darlo por aceptado.
        let c = Cuaderno::leer(r#"{"id":"d","clase":"DIBUJO","enBuzon":true}"#);
        assert!(c.de_seccion(Seccion::Galeria).is_empty());
        assert_eq!(c.de_seccion(Seccion::Buzon).len(), 1);
    }

    #[test]
    fn los_mensajes_de_sistema_no_salen_en_ninguna_seccion() {
        let c = Cuaderno::leer(concat!(
            r#"{"id":"auto-letra-romans.shx","clase":"ARCHIVO","nombre":"romans.shx","ruta":"pixpin:files/guardados/fuentes-shx/romans.shx"}"#,
            "\n",
            r#"{"id":"auto-figuras","clase":"ARCHIVO","nombre":"figuras.json","fijado":true}"#,
            "\n",
            r#"{"id":"n1","clase":"NOTA","texto":"auto-guardado de la obra"}"#,
            "\n",
            r#"{"id":"a1","clase":"ARCHIVO","nombre":"auto-plano.dwg"}"#,
            "\n"
        ));
        assert!(es_de_sistema(&c.mensajes[0]) && es_de_sistema(&c.mensajes[1]));
        // Caso negativo: por el id, nunca por lo que dice o como se llama.
        assert!(!es_de_sistema(&c.mensajes[2]) && !es_de_sistema(&c.mensajes[3]));
        let ids =
            |s: Seccion| -> Vec<&str> { c.de_seccion(s).iter().map(|m| m.id.as_str()).collect() };
        assert_eq!(ids(Seccion::Todo), ["n1", "a1"]);
        assert_eq!(ids(Seccion::Archivos), ["a1"]);
        assert!(ids(Seccion::Fijados).is_empty());
    }

    #[test]
    fn un_pdf_se_reconoce_por_su_nombre_con_mayusculas_o_sin_ellas() {
        let pdf = |nombre: &str| Mensaje {
            clase: Some(Clase::Archivo),
            nombre: nombre.into(),
            ..Default::default()
        };
        assert!(es_pdf(&pdf("informe.pdf")));
        assert!(es_pdf(&pdf("INFORME.PDF")));
        // Casos negativos: sin extension, con el punto dentro del nombre y
        // una clase que no es un archivo.
        assert!(!es_pdf(&pdf("pdf")));
        assert!(!es_pdf(&pdf("mi.pdf.zip")));
        assert!(!es_pdf(&Mensaje {
            clase: Some(Clase::Dibujo),
            nombre: "croquis.pdf".into(),
            ..Default::default()
        }));
    }

    #[test]
    fn los_indices_de_una_seccion_apuntan_a_sus_mismos_mensajes() {
        let c = Cuaderno::leer(concat!(
            r#"{"id":"n","clase":"NOTA","texto":"hola"}"#,
            "\n",
            r#"{"id":"i","clase":"IMAGEN","nombre":"a.jpg"}"#,
            "\n",
            r#"{"id":"d","clase":"DIBUJO","nombre":"croquis"}"#,
            "\n"
        ));
        // Son posiciones en la lista ENTERA, no en la seccion: en eso esta
        // toda la gracia, porque quien pinta guarda cosas en paralelo a esa
        // lista y las alcanza con este numero.
        assert_eq!(indices_de_seccion(&c.mensajes, Seccion::Dibujos), [2]);
        for s in Seccion::TODAS {
            let por_indice: Vec<&str> = indices_de_seccion(&c.mensajes, s)
                .into_iter()
                .map(|n| c.mensajes[n].id.as_str())
                .collect();
            let directos: Vec<&str> = c.de_seccion(s).iter().map(|m| m.id.as_str()).collect();
            assert_eq!(por_indice, directos, "{s:?}");
        }
        // Caso negativo: sin mensajes no hay indices que devolver, y no se
        // inventa el cero.
        assert!(indices_de_seccion(&[], Seccion::Todo).is_empty());
    }

    #[test]
    fn una_clase_que_no_conocemos_solo_sale_en_todo() {
        let c =
            Cuaderno::leer(r#"{"id":"x","clase":"COSA_NUEVA","texto":"de una version futura"}"#);
        assert_eq!(c.de_seccion(Seccion::Todo).len(), 1, "no se pierde");
        // Y no se cuela en ninguna de las demas fingiendo ser algo que no es.
        for s in [
            Seccion::Fotos,
            Seccion::Archivos,
            Seccion::Voz,
            Seccion::Dibujos,
            Seccion::Fijados,
            Seccion::Buzon,
        ] {
            assert!(c.de_seccion(s).is_empty(), "{s:?}");
        }
    }
}

#[cfg(test)]
mod pruebas_lineas {
    use super::*;

    #[test]
    fn las_lineas_hechas_fuera_quedan_igual_que_anadir_una_a_una() {
        let d = std::env::temp_dir().join(format!("pixpin-lineas-{}", std::process::id()));
        let (a, b) = (d.join("a"), d.join("b"));
        let m1 = Mensaje::nota(
            "uno",
            &Sello {
                cuando: 1,
                numero: 1,
                aparato: "K".into(),
                proyecto: "p".into(),
            },
        );
        let m2 = Mensaje::nota(
            "dos",
            &Sello {
                cuando: 2,
                numero: 2,
                aparato: "K".into(),
                proyecto: "p".into(),
            },
        );
        anadir(&a, &m1).unwrap();
        anadir(&a, &m2).unwrap();
        anadir_lineas(&b, &[linea_de(&m1).unwrap(), linea_de(&m2).unwrap()]).unwrap();
        let leer =
            |c: &std::path::Path| std::fs::read_to_string(c.join("guardados.jsonl")).unwrap();
        assert_eq!(leer(&a), leer(&b));
        // Caso negativo: ninguna linea no escribe nada raro.
        anadir_lineas(&b, &[]).unwrap();
        assert_eq!(leer(&a), leer(&b));
        let _ = std::fs::remove_dir_all(d);
    }
}
