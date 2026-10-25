//! Los ajustes de la aplicacion, en TOML.
//!
//! Dos reglas gobiernan el formato, y las dos existen para que el fichero se
//! pueda editar a mano sin miedo:
//!
//! - **Todo campo que falte se rellena con su valor por defecto.** Un usuario
//!   que solo quiere cambiar un atajo escribe dos lineas, no el fichero entero.
//! - **Las claves desconocidas se ignoran.** Un fichero escrito por una version
//!   mas nueva no impide arrancar a una mas vieja.

use std::fs;
use std::path::PathBuf;

use pixpin_sistema::Atajo;
use serde::{Deserialize, Serialize};

use crate::rutas::Ubicacion;

#[derive(Debug, thiserror::Error)]
pub enum ErrorAjustes {
    #[error("no se pudo leer {ruta}: {fuente}")]
    Lectura {
        ruta: PathBuf,
        #[source]
        fuente: std::io::Error,
    },
    #[error("no se pudo escribir {ruta}: {fuente}")]
    Escritura {
        ruta: PathBuf,
        #[source]
        fuente: std::io::Error,
    },
    #[error("el fichero de ajustes tiene un error: {0}")]
    Formato(#[from] toml::de::Error),
    #[error("no se pudieron serializar los ajustes: {0}")]
    Serializacion(#[from] toml::ser::Error),
    /// La del guardado que conserva comentarios, que usa otro escritor.
    #[error("no se pudieron serializar los ajustes: {0}")]
    SerializacionConservando(#[from] toml_edit::ser::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PreferenciaIdioma {
    /// Se toma del idioma de Windows.
    #[default]
    Sistema,
    #[serde(rename = "es")]
    Espanol,
    #[serde(rename = "en")]
    Ingles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PreferenciaNivel {
    /// La aplicacion mide el equipo al arrancar y decide.
    #[default]
    Auto,
    Completo,
    Ligero,
}

/// Seccion `[rendimiento]` del fichero de ajustes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rendimiento {
    /// `auto` deja decidir a los hechos; `completo` y `ligero` fuerzan el
    /// nivel. Forzar `ligero` en una maquina potente es legitimo y util:
    /// es como se prueba la ruta ligera sin tener hardware flojo delante.
    pub nivel: PreferenciaNivel,
    /// Registrar cada 60 fotogramas del editor cuanto se tarda en vaciar la
    /// cola, pintar, presentar y esperar (D129). Es para diagnosticar en el
    /// equipo del usuario sin entrada sintetizada; apagado no registra nada.
    pub medir_fotogramas: bool,
    /// Empezar el fotograma JUSTO ANTES del plazo de DWM en vez de nada mas
    /// poder (B1 de la investigacion del 2026-09-19). Los puntos del lapiz
    /// entran mas frescos y la punta va menos retrasada, a cambio de que un
    /// calculo fallado pierda un refresco.
    ///
    /// Apagado de fabrica: es un cambio de ritmo del bucle y una regresion
    /// se apaga con esta linea, sin volver a compilar.
    pub ritmo: bool,
    /// Desplazar y acercar moviendo el visual de la escena en el compositor
    /// en vez de repintarla (A3). Encendido de fabrica: es la diferencia
    /// entre panear a 15-83 ms por fotograma y no pintar nada hasta parar.
    /// A `false` vuelve el repintado por fotograma de siempre.
    pub paneo_por_composicion: bool,
}

impl Default for Rendimiento {
    fn default() -> Self {
        Self {
            nivel: PreferenciaNivel::default(),
            medir_fotogramas: false,
            ritmo: false,
            paneo_por_composicion: true,
        }
    }
}

/// Como se suaviza el trazo a mano (`[tinta] suavizado`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Suavizado {
    /// Lo de siempre: solo el `streamline` de perfect-freehand, que es lo
    /// que hace Excalidraw. Los ficheros salen identicos a los de antes.
    #[default]
    Excalidraw,
    /// Ademas, el filtro de 1 euro delante (`pixpin-tinta`). Quita el
    /// temblor de la mano parada sin retrasar la punta cuando se va deprisa,
    /// que es donde `streamline` se siente como una goma. Cambia un poco la
    /// forma del trazo: por eso no es el de por defecto.
    Natural,
}

/// Seccion `[tinta]` del fichero de ajustes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Tinta {
    pub suavizado: Suavizado,
    /// `mincutoff` del filtro de 1 euro, en hercios: el corte con el lapiz
    /// quieto. Bajarlo quita mas temblor y retrasa mas al arrancar.
    /// `None` = el valor conservador del propio motor.
    pub corte_minimo: Option<f32>,
    /// `beta`: cuanto sube el corte con la velocidad. Subirlo pega mas la
    /// punta al cursor a costa de dejar pasar mas temblor.
    pub beta: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FormatoColor {
    #[default]
    Hex,
    Rgb,
    Hsl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Atajos {
    /// Capturar region y mostrar la barra de resultado. Es el unico atajo
    /// de fabrica (D140): lleva a la seleccion y alli se elige que hacer.
    pub region: Option<Atajo>,
    /// Capturar region y copiar directo al portapapeles, sin confirmacion.
    pub copiar: Option<Atajo>,
    /// Captura larga con scroll.
    pub scroll: Option<Atajo>,
    /// Cuentagotas global. Sin atajo por defecto (D81).
    pub cuentagotas: Option<Atajo>,
    /// Recortar y dejar flotando como pin (S2).
    pub pin: Option<Atajo>,
    /// Pinear el contenido del portapapeles (S2-B).
    pub portapapeles: Option<Atajo>,
    /// Anotar sobre la pantalla con la capa viva (S3-C).
    pub anotar: Option<Atajo>,
    /// Anotar sobre una captura estatica de la pantalla (S3-C, D56). Sin
    /// atajo por defecto (D81).
    pub anotar_congelada: Option<Atajo>,
}

impl Default for Atajos {
    fn default() -> Self {
        // `expect` es correcto aqui: si una constante del propio codigo no
        // parsea, es un fallo de programacion y debe verse en el primer test.
        Self {
            // El unico atajo de fabrica (D140). Los demas nacen sin atajo:
            // lo rapido va por los gestos con Alt, y quien quiera alguno lo
            // escribe en el TOML.
            region: Some("Ctrl+Alt+X".parse().expect("atajo por defecto valido")),
            copiar: None,
            scroll: None,
            cuentagotas: None,
            pin: None,
            portapapeles: None,
            anotar: None,
            anotar_congelada: None,
        }
    }
}

/// Lo que se recuerda de una grabacion a la siguiente.
///
/// Ajustar el ritmo cada vez seria un impuesto: quien graba interfaces
/// suele quedarse en el mismo numero durante meses. El retardo esta por
/// lo mismo que en el original: da tiempo a poner el raton donde toca
/// antes de que empiece a contar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Gif {
    /// Fotogramas por segundo. Se guarda el numero y no el indice de la
    /// lista para que el fichero siga significando lo mismo si algun dia
    /// se ofrecen otros ritmos.
    pub por_segundo: u32,
    /// Segundos de cortesia entre pulsar «Grabar» y el primer fotograma.
    pub retardo_s: u32,
}

impl Default for Gif {
    fn default() -> Self {
        Self {
            por_segundo: 10,
            retardo_s: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ajustes {
    pub idioma: PreferenciaIdioma,
    /// La tabla vieja, de cuando cada atajo era un campo. Se sigue leyendo
    /// para no romper el fichero de nadie, pero lo que manda es `comandos`.
    pub atajos: Atajos,
    /// Nombre de comando -> atajo, la tabla nueva (ver `comandos.rs`). Lo
    /// que no se nombra conserva su atajo por defecto; una cadena vacia deja
    /// el comando sin atajo.
    pub comandos: std::collections::BTreeMap<String, String>,
    /// Si es `None` se usa la carpeta Imagenes del usuario.
    pub carpeta_capturas: Option<PathBuf>,
    pub formato_color: FormatoColor,
    pub rendimiento: Rendimiento,
    /// Como se siente el lapiz (`[tinta]`). Vacio = como siempre.
    pub tinta: Tinta,
    pub arranque_con_windows: bool,
    /// M1: el tema Cosmos del movil fuera del universo: el cielo estrellado
    /// detras de la lista de proyectos del chat y sus colores de noche. En
    /// el movil es una opcion del «Modo noche» (y la de fabrica); aqui, que
    /// no hay modo noche propio, una casilla, apagada de fabrica para no
    /// cambiarle el aspecto a nadie sin pedirlo.
    pub tema_cosmos: bool,
    /// Tope de altura de la captura con scroll. Sin el, una pagina infinita
    /// capturaria hasta agotar la memoria.
    pub limite_scroll_px: u32,
    /// Lo que se recuerda de la grabacion (P5b).
    pub gif: Gif,
    /// Programas delante de los cuales los atajos no actuan (P1.8).
    ///
    /// Se nombran por ejecutable, con o sin `.exe`, sin distinguir
    /// mayusculas. Por defecto esta vacia: nadie ha pedido que PixPin
    /// deje de responder, y una lista con algo dentro de fabrica seria
    /// una sorpresa muy dificil de averiguar.
    pub ignorar_programas: Vec<String>,
    /// Segundos que espera la captura con retardo antes de abrirse
    /// (P2.1). Tres es lo justo para desplegar un menu y soltar el raton.
    pub retardo_captura_s: u32,
    /// Zonas guardadas con nombre, cada una con su atajo (P2.3).
    ///
    /// Es una tabla repetible (`[[regiones]]`) y por eso va DESPUES de
    /// las claves sueltas en el fichero, como cualquier seccion.
    pub regiones: Vec<crate::regiones::Region>,
    /// Salir en «Abrir con» de Windows para imagenes y videos.
    ///
    /// Es lo UNICO que escribe en el registro estando en modo portable, y
    /// por eso tiene interruptor: el modo portable promete no dejar rastro
    /// en el equipo. Apagarlo borra lo escrito, no solo deja de escribir.
    pub abrir_con: bool,
    /// **Vista rapida**: el espacio en el Explorador (o el escritorio) abre
    /// en PixPin lo elegido que sabe abrir (10-oct-2026).
    pub vista_rapida: bool,
    /// A que se pega el cursor al dibujar (fase B.6).
    ///
    /// El tipo vive en el motor y no aqui, aunque eso ate `pixpin-store` a
    /// `pixpin-motor2d`: la alternativa era una copia del struct con los
    /// mismos campos y una traduccion, o sea dos sitios que hay que
    /// acordarse de tocar a la vez -y un campo que se olvide en la copia es
    /// un ajuste que se guarda y no se lee-. La regla de capas lo permite:
    /// store es capa 2 y motor2d es capa 1.
    pub enganche: pixpin_motor2d::enganche::Ajustes,
    /// Sincronizar con el movil.
    ///
    /// Es una tabla (`[sincro]`) y por eso va al final, detras de las claves
    /// sueltas: en TOML, lo que sigue a una cabecera de tabla es suyo.
    pub sincro: Sincro,
    /// La pila de capturas (`[capturas]`). Tabla, asi que tambien al final.
    pub capturas: Capturas,
    /// Aligerar los PDF que entran al chat (`[pdf]`). Tabla: al final.
    pub pdf: Pdf,
    /// Que herramientas de dibujo salen en el lienzo, el lector, los pines
    /// y el anotador de pantalla (`[herramientas]`). Tabla: al final.
    pub herramientas: crate::herramientas::Herramientas,
    /// Pasar notas de voz a texto con dos idiomas (`[voz]`). Tabla: al final.
    pub voz: Voz,
    /// Las palabras magicas propias (`[pinear]`). Tabla: al final.
    pub pinear: Pinear,
    /// El asistente local (`[asistente]`). Tabla: al final.
    pub asistente: Asistente,
    /// Las palabras de cada urgencia de las tareas (`[tareas]`). Tabla: al
    /// final.
    pub tareas: Tareas,
}

/// Seccion `[pinear]`: las palabras magicas que pone el usuario.
///
/// En el movil es una cadena que puede no existir (`palabrasMagicas`, null =
/// las de fabrica). Aqui son dos claves porque el guardado que conserva los
/// comentarios no borra lo que ya no esta: con una sola opcional, volver a
/// las de fabrica dejaria escrita la lista vieja.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Pinear {
    /// Si mandan las de aqui. Apagado: las de fabrica, diga lo que diga la
    /// lista.
    pub palabras_propias: bool,
    /// Una por linea, `palabra=APP`, el formato del movil
    /// (`MagicWord.escribir`): «reloj=TIMER». Vacia con `palabras_propias`
    /// es «ninguna».
    pub palabras: String,
}

impl Pinear {
    /// Lo guardado como lo lee `magia::leer`: `None` es «las de fabrica».
    pub fn guardadas(&self) -> Option<&str> {
        self.palabras_propias.then_some(self.palabras.as_str())
    }
}

/// Cuanto se aprieta un PDF: los cuatro perfiles de pdfsqueeze, con los
/// nombres del movil (`ComprimirPdf.NIVELES`). En el TOML se aceptan tambien
/// los de pdfsqueeze (`lossless`, `balanced`, `small`, `extreme`), que son
/// los que alguien copiaria de su documentacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum NivelPdf {
    /// Ni un pixel cambia: solo se ordena y se aprieta por dentro.
    #[serde(alias = "lossless")]
    SinPerdida,
    /// No se nota a la vista: fotos a 200 ppp, cada una comprobada. El del
    /// movil de fabrica (`NIVEL_POR_DEFECTO = EQUILIBRADO`).
    #[default]
    #[serde(alias = "balanced")]
    Equilibrado,
    /// Para leer en pantalla: 150 ppp, y en los escaneos el texto se separa
    /// del papel y sigue nitido.
    #[serde(alias = "small")]
    Pequeno,
    /// Lo minimo que se sigue leyendo bien.
    #[serde(alias = "extreme")]
    Extremo,
}

/// Seccion `[pdf]`: aligerar los PDF al meterlos al chat.
///
/// En el movil es un solo ajuste con un «No comprimir» entre los niveles;
/// aqui son dos porque la fila de opciones de la ventana de ajustes no tiene
/// sitio para cinco, y porque asi apagarlo no olvida el nivel elegido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pdf {
    /// Al entrar un PDF al chat, se aligera despues, en segundo plano.
    /// Encendido de fabrica, como en el movil.
    pub aligerar_al_entrar: bool,
    /// Con que nivel. Al entrar solo se usan Exacto y Medio: si aqui dice
    /// Chico o Max, al entrar se usa Medio y esos dos quedan para «Aligerar el
    /// PDF» pedido a mano (ver `apps/pixpin/src/aligerar.rs`).
    pub nivel: NivelPdf,
}

impl Default for Pdf {
    fn default() -> Self {
        Self {
            aligerar_al_entrar: true,
            nivel: NivelPdf::default(),
        }
    }
}

/// En que esquina del monitor aparece el icono de la pila de capturas.
///
/// Los nombres van en kebab-case en el TOML (`esquina = "abajo-derecha"`) y
/// un nombre desconocido da error en vez de caer al de por defecto: si
/// alguien escribe `"abajo derecha"` sin el guion, es mejor que PixPin lo
/// diga a que el icono aparezca en otro sitio sin explicacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum EsquinaPila {
    ArribaIzquierda,
    ArribaDerecha,
    AbajoIzquierda,
    #[default]
    AbajoDerecha,
}

/// Seccion `[capturas]`: la pila de capturas.
///
/// El usuario la pidio asi: varias capturas seguidas se juntan en un
/// montoncito y un solo Ctrl+V las pega todas. Se apaga entera poniendo
/// `apilar_segundos = 0`, y entonces todo se comporta como antes de que esto
/// existiera: una captura, una imagen en el portapapeles y ninguna ventana en
/// la esquina.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Capturas {
    /// **Cero apaga la pila entera**; cualquier otro valor la enciende.
    ///
    /// Nacio como un plazo (segundos desde la ultima captura para que la
    /// siguiente se apilara) y el 2-oct el usuario lo cambio por un
    /// interruptor: un clic en el recuadro de la esquina lo arma y agrupa
    /// sin plazo hasta otro clic. El numero ya no mide nada; se conserva el
    /// nombre para no romper los TOML que ya lo llevan.
    pub apilar_segundos: u32,
    /// Donde se planta el icono, dentro del monitor donde se hizo la captura.
    pub esquina: EsquinaPila,
    /// Segundos que el recuadro SIN ARMAR sigue ahi despues de la ultima
    /// captura. Cero lo deja hasta que el usuario lo quite. Armado no se va
    /// solo nunca.
    ///
    /// Ocho porque son los justos para ver que la captura se hizo y decidir
    /// si se abre el montoncito, sin quedarse tapando la esquina.
    pub icono_segundos: u32,
    /// **Dias que aguanta una captura antes de irse sola** a la papelera de
    /// Windows (`apps/pixpin/src/caducidad_capturas.rs`), salvo las
    /// conservadas. Cero = no se va ninguna. Siete de fabrica: lo que pidio
    /// el usuario el 3-oct («por ejemplo 1 semana»). Un fichero de antes,
    /// sin esta clave, sigue en siete.
    #[serde(default = "dias_caducidad_de_fabrica")]
    pub dias_caducidad: u32,
}

fn dias_caducidad_de_fabrica() -> u32 {
    7
}

impl Default for Capturas {
    fn default() -> Self {
        Self {
            apilar_segundos: 10,
            esquina: EsquinaPila::default(),
            icono_segundos: 8,
            dias_caducidad: dias_caducidad_de_fabrica(),
        }
    }
}

/// Lo de sincronizar que se puede tocar a mano.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sincro {
    /// Dejarse encontrar por el grupo mientras PixPin esta abierto.
    ///
    /// Encendido, PixPin escucha en el puerto de sincronizar y se anuncia en
    /// la red local desde que arranca, como hace `Presencia` en el movil: es
    /// lo que hace que el movil vea el ordenador sin esperar uno o dos
    /// minutos. Apagado, no se abre ninguna puerta ni se anuncia nada —desde
    /// aqui se puede seguir llamando al movil, pero el movil no podra llamar.
    ///
    /// Viene encendido porque es lo que el usuario espera de «Sincronizar»;
    /// tiene interruptor porque una puerta abierta en la red local es algo
    /// que cada uno tiene derecho a no querer.
    pub presencia: bool,
    /// «Lo mio manda»: al sincronizar, lo de ESTE equipo se impone.
    ///
    /// Con el encendido, un proyecto que ya esta aqui se queda tal cual y del
    /// otro lado solo llega lo que aqui no existe. Es el `loMioManda` del
    /// movil (v0.79), y nacio de un accidente real: alguien vacio un portatil
    /// creyendo que el telefono lo volveria a llenar, y al juntar los dos
    /// aparatos los borrados eran lo mas reciente y se llevaron por delante
    /// lo del telefono.
    ///
    /// **Apagado de fabrica**: sincronizar es ponerse de acuerdo, y que un
    /// lado mande siempre es la excepcion que se pide a proposito, no lo que
    /// alguien espera sin haberlo tocado.
    pub lo_mio_manda: bool,
    /// **Sincronizar solo, con la aplicacion abierta** (8-oct-2026). Con
    /// los aparatos del grupo que contesten: al aparecer en la red, al
    /// cambiar algo aqui (tras 30 s sin mas cambios) y cada diez minutos.
    /// Sin globos salvo si llega algo o falla varias veces seguidas. Pide la
    /// presencia encendida: sin sonda no se sabe quien contesta.
    ///
    /// Tambien lleva **las tareas al instante** (10-oct-2026): lo que se
    /// tacha, apunta o quita sale en cuanto se deja de tocar (600 ms) solo
    /// con su chat. Apagada esta, tampoco.
    pub automatica: bool,
}

impl Default for Sincro {
    fn default() -> Self {
        Self {
            presencia: true,
            lo_mio_manda: false,
            automatica: true,
        }
    }
}

// --- Voz (B6): dos idiomas en una nota -----------------------------------

/// Que hace Whisper con dos idiomas mezclados. Las palabras del TOML son las
/// del movil (`MODO_CADA_IDIOMA = "cada_uno"`, `MODO_TODO_EN_UNO`), para que
/// signifiquen lo mismo en los dos aparatos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ModoDeIdiomas {
    /// Cada trozo en el idioma en que se dijo.
    #[default]
    #[serde(rename = "cada_uno")]
    CadaUno,
    /// Todo en el primer idioma (lo del otro, traducido).
    #[serde(rename = "todo_en_uno")]
    TodoEnUno,
}

/// Seccion `[voz]`: pasar notas de voz a texto.
///
/// `segundoIdiomaDeVoz` y `modoDeIdiomas` del movil (`SettingsRepository`):
/// el primer idioma es el de la interfaz (ver `apps/pixpin/src/voz.rs`), y
/// el segundo, solo para Whisper, vacio = ninguno.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Voz {
    /// Codigo corto (`en`, `pt`…) o vacio.
    pub segundo_idioma: String,
    pub modo_de_idiomas: ModoDeIdiomas,
}

// --- Tareas (18-oct-2026) -------------------------------------------------

/// Seccion `[tareas]`: las palabras que, escritas en una tarea, la hacen
/// urgente o muy urgente (`pixpin_frase::tarea`). Sin ninguna, todo es
/// normal. El plugin de Flow las lee de aqui con `Palabras::de_toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tareas {
    pub urgente: Vec<String>,
    pub muy_urgente: Vec<String>,
}

impl Default for Tareas {
    fn default() -> Self {
        let p = pixpin_frase::tarea::Palabras::preestablecidas();
        Tareas {
            urgente: p.urgente,
            muy_urgente: p.muy_urgente,
        }
    }
}

impl Tareas {
    /// Las palabras como las usa el parser.
    pub fn palabras(&self) -> pixpin_frase::tarea::Palabras {
        pixpin_frase::tarea::Palabras {
            urgente: self.urgente.clone(),
            muy_urgente: self.muy_urgente.clone(),
        }
    }
}

// --- Asistente local (13-oct-2026) ---------------------------------------

/// Seccion `[asistente]`: el modelo de lenguaje local del chat
/// (`crates/pixpin-ia`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Asistente {
    /// El id del modelo (`qwen3.5-2b`, `qwen3.5-0.8b`). Vacio: el 2B, el
    /// recomendado.
    pub modelo: String,
    /// Si piensa antes de contestar (lo que piensa se ve, plegable, en la
    /// burbuja del chat). `None`: automatico, solo en las tareas complejas
    /// (`pixpin_ia::complejidad`, 15-oct), el local y la nube igual.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pensar: Option<bool>,
    /// Quien contesta (13-oct-2026): vacio o `local` (el modelo de este PC),
    /// `claude` (Claude Haiku 5.5) o `deepseek` (DeepSeek V4.1 Flash). Las
    /// claves de la nube NO van aqui: van cifradas en su fichero propio
    /// (`pixpin_ia::nube::claves`).
    pub motor: String,
    /// Si la nube falla (red, clave, saldo), seguir con el modelo local si
    /// esta bajado.
    pub respaldo_local: bool,
    /// **Mejor vista con fotos** (13-oct-2026): con el modelo local, usar
    /// Gemma 4 E2B (opcional, ~4,1 GB que se bajan aparte) cuando se pide
    /// MIRAR una foto: describirla, leer lo que pone. Apagado de fabrica.
    pub mejor_vista: bool,
    /// Si con un motor de la nube se le pueden mandar trozos de los
    /// documentos propios (`buscar_en_mis_cosas`): vacio = preguntar la
    /// primera vez, `si` o `no` (13-oct-2026).
    pub documentos_a_la_nube: String,
    /// Voz (13-oct-2026): lo dicho al microfono del asistente se deja en la
    /// caja para repasarlo antes de enviarlo, en vez de enviarse solo.
    pub revisar_lo_dicho: bool,
    // `leer_respuestas` (leer cada respuesta sola al acabar) se quito el
    // 17-oct-2026 a peticion del usuario: el TOML viejo que lo traiga se
    // sigue leyendo (sin `deny_unknown_fields`) y al guardar desaparece.
}

impl Default for Asistente {
    fn default() -> Self {
        Self {
            modelo: String::new(),
            motor: String::new(),
            respaldo_local: true,
            mejor_vista: false,
            documentos_a_la_nube: String::new(),
            pensar: None,
            revisar_lo_dicho: false,
        }
    }
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            idioma: PreferenciaIdioma::default(),
            atajos: Atajos::default(),
            comandos: std::collections::BTreeMap::new(),
            carpeta_capturas: None,
            formato_color: FormatoColor::default(),
            rendimiento: Rendimiento::default(),
            tinta: Tinta::default(),
            // Encendido de fabrica: el usuario lo pidio con estas palabras,
            // «que apenas prendo se encienda la app». Vive en la bandeja y no
            // roba nada al arrancar, y quien no lo quiera lo apaga en los
            // ajustes o pone `arranque_con_windows = false` en el TOML. En
            // modo portable no toca el registro, se quiera o no.
            arranque_con_windows: true,
            tema_cosmos: false,
            limite_scroll_px: 30_000,
            gif: Gif::default(),
            ignorar_programas: Vec::new(),
            retardo_captura_s: 3,
            regiones: Vec::new(),
            abrir_con: true,
            vista_rapida: true,
            enganche: pixpin_motor2d::enganche::Ajustes::default(),
            sincro: Sincro::default(),
            capturas: Capturas::default(),
            pdf: Pdf::default(),
            herramientas: crate::herramientas::Herramientas::default(),
            voz: Voz::default(),
            pinear: Pinear::default(),
            asistente: Asistente::default(),
            tareas: Tareas::default(),
        }
    }
}

/// Lee los ajustes. Si el fichero no existe, devuelve los valores por defecto.
pub fn cargar(ubicacion: &Ubicacion) -> Result<Ajustes, ErrorAjustes> {
    let ruta = ubicacion.fichero_ajustes();
    let texto = match fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Ajustes::default()),
        Err(fuente) => return Err(ErrorAjustes::Lectura { ruta, fuente }),
    };
    Ok(toml::from_str(&texto)?)
}

/// Escribe los ajustes, creando el directorio si hace falta.
pub fn guardar(ubicacion: &Ubicacion, ajustes: &Ajustes) -> Result<(), ErrorAjustes> {
    let ruta = ubicacion.fichero_ajustes();
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre).map_err(|fuente| ErrorAjustes::Escritura {
            ruta: padre.to_path_buf(),
            fuente,
        })?;
    }
    let texto = toml::to_string_pretty(ajustes)?;
    fs::write(&ruta, texto).map_err(|fuente| ErrorAjustes::Escritura { ruta, fuente })
}

/// Guarda los ajustes SIN perder los comentarios ni el orden del
/// fichero (P6).
///
/// `guardar` serializa la estructura entera y reescribe el fichero de
/// cero: sirve para crearlo, pero usarlo desde la ventana de ajustes le
/// borraria al usuario todas las explicaciones que tiene escritas dentro,
/// que en este fichero son la mitad del contenido. Esto lee lo que hay,
/// cambia solo los VALORES, y devuelve el resto tal cual: comentarios,
/// orden, lineas en blanco y hasta las claves que no conocemos.
///
/// La fusion es generica y no campo a campo a proposito. Escribir a mano
/// «pon idioma, pon comandos, pon gif...» significa que el dia que se
/// anada un ajuste, alguien se olvidara de anadirlo aqui y ese ajuste
/// dejara de guardarse sin que falle nada. Lo vigila ademas la prueba
/// `ningun_ajuste_se_queda_sin_guardar`.
pub fn guardar_conservando(ubicacion: &Ubicacion, ajustes: &Ajustes) -> Result<(), ErrorAjustes> {
    let ruta = ubicacion.fichero_ajustes();
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre).map_err(|fuente| ErrorAjustes::Escritura {
            ruta: padre.to_path_buf(),
            fuente,
        })?;
    }
    // Si no hay fichero, o el que hay no se puede leer como TOML, se
    // empieza de uno vacio. No se aborta: quedarse sin poder guardar
    // porque el fichero de antes estaba roto seria dejar al usuario
    // atrapado, y lo que va a escribirse es valido de todas formas.
    let existente = fs::read_to_string(&ruta).unwrap_or_default();
    let mut documento = existente
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_default();
    let nuevo = toml_edit::ser::to_document(ajustes)?;
    fusionar(documento.as_table_mut(), nuevo.as_table());
    fs::write(&ruta, documento.to_string())
        .map_err(|fuente| ErrorAjustes::Escritura { ruta, fuente })
}

/// Copia los valores de `origen` sobre `destino`, entrando en las tablas
/// en vez de reemplazarlas.
///
/// Reemplazar la tabla entera se llevaria por delante los comentarios de
/// cada linea de dentro, que es justo lo que hay que conservar. Y lo que
/// esta en `destino` y no en `origen` se QUEDA: si el usuario escribio una
/// clave que no conocemos, no es asunto nuestro borrarsela.
fn fusionar(destino: &mut toml_edit::Table, origen: &toml_edit::Table) {
    for (clave, valor) in origen.iter() {
        // Si los dos lados son una tabla, se entra. Ojo: el serializador
        // produce tablas EN LINEA (`{ a = 1 }`) donde el fichero escrito a
        // mano tiene secciones (`[a]`), asi que hay que reconocer las dos
        // formas. Sin esto, `[comandos]` se sustituia entera por una tabla
        // en linea y se perdian de golpe el comentario de la seccion y el
        // de cada atajo de dentro.
        if let (Some(toml_edit::Item::Table(d)), Some(o)) =
            (destino.get_mut(clave), como_tabla(valor))
        {
            fusionar(d, &o);
            continue;
        }
        // El adorno de la clave (sus comentarios y su sangria) va pegado a
        // la clave y no al valor, asi que asignar el valor los conserva
        // sin hacer nada mas.
        destino[clave] = valor.clone();
    }
}

/// Ve un elemento como tabla, venga como seccion o como tabla en linea.
fn como_tabla(item: &toml_edit::Item) -> Option<toml_edit::Table> {
    match item {
        toml_edit::Item::Table(t) => Some(t.clone()),
        toml_edit::Item::Value(toml_edit::Value::InlineTable(t)) => Some(t.clone().into_table()),
        _ => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    fn temporal(etiqueta: &str) -> Ubicacion {
        let dir = std::env::temp_dir().join(format!("pixpin-ajustes-{etiqueta}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Ubicacion::Instalado { raiz: dir }
    }

    /// Las palabras de urgencia de las tareas (18-oct-2026): de fabrica las
    /// del parser, se guardan en `[tareas]` y el plugin de Flow las lee del
    /// fichero tal como lo escriben los dos guardados.
    #[test]
    fn las_palabras_de_urgencia_se_guardan_y_las_lee_flow() {
        let d = Ajustes::default();
        assert_eq!(
            d.tareas.palabras(),
            pixpin_frase::tarea::Palabras::preestablecidas()
        );
        let u = temporal("tareas-urgencia");
        let mut a = Ajustes::default();
        a.tareas.urgente = vec!["pronto".into(), "con \"comillas\"".into()];
        a.tareas.muy_urgente = vec![];
        guardar(&u, &a).unwrap();
        let texto = fs::read_to_string(u.fichero_ajustes()).unwrap();
        assert_eq!(
            pixpin_frase::tarea::Palabras::de_toml(&texto),
            a.tareas.palabras()
        );
        assert_eq!(cargar(&u).unwrap().tareas, a.tareas);
        a.tareas.muy_urgente = vec!["ya".into()];
        guardar_conservando(&u, &a).unwrap();
        let texto = fs::read_to_string(u.fichero_ajustes()).unwrap();
        assert_eq!(
            pixpin_frase::tarea::Palabras::de_toml(&texto),
            a.tareas.palabras()
        );
        // Un TOML de antes, sin `[tareas]`: las de fabrica.
        let viejo: Ajustes = toml::from_str("idioma = \"es\"\n").unwrap();
        assert_eq!(viejo.tareas, Tareas::default());
    }

    /// «Leer siempre las respuestas» se quito (17-oct-2026): un TOML que
    /// lo trae encendido (el del usuario) se sigue leyendo entero, y al
    /// guardar desaparece.
    #[test]
    fn el_viejo_leer_respuestas_se_lee_y_se_olvida() {
        let u = temporal("leer-respuestas");
        fs::write(
            u.fichero_ajustes(),
            "[asistente]
motor = \"claude\"
revisar_lo_dicho = true
leer_respuestas = true
",
        )
        .unwrap();
        let a = cargar(&u).unwrap();
        assert_eq!(a.asistente.motor, "claude");
        assert!(a.asistente.revisar_lo_dicho, "lo demas de la seccion sigue");
        guardar(&u, &a).unwrap();
        let texto = fs::read_to_string(u.fichero_ajustes()).unwrap();
        assert!(!texto.contains("leer_respuestas"), "{texto}");
    }

    /// Unos ajustes con TODO cambiado respecto a los de fabrica.
    ///
    /// Que no quede ni un campo en su valor por defecto es lo que hace
    /// util la prueba de ida y vuelta: si un campo se quedara sin
    /// guardar, volveria a su valor por defecto y la comparacion no lo
    /// notaria si ese ya era su valor.
    fn todo_cambiado() -> Ajustes {
        let atajos = Atajos {
            copiar: Some("Ctrl+Shift+F9".parse().unwrap()),
            ..Atajos::default()
        };
        let mut comandos = std::collections::BTreeMap::new();
        comandos.insert("grabar-gif".to_string(), "Ctrl+Alt+G".to_string());
        Ajustes {
            idioma: PreferenciaIdioma::Ingles,
            atajos,
            comandos,
            carpeta_capturas: Some(PathBuf::from("C:/capturas")),
            formato_color: FormatoColor::Hsl,
            arranque_con_windows: true,
            limite_scroll_px: 12_345,
            gif: Gif {
                por_segundo: 25,
                retardo_s: 4,
            },
            ignorar_programas: vec!["juego.exe".into()],
            retardo_captura_s: 7,
            regiones: vec![crate::regiones::Region {
                nombre: "panel".into(),
                x: 1,
                y: 2,
                ancho: 300,
                alto: 200,
                atajo: Some("Ctrl+Alt+1".into()),
            }],
            enganche: pixpin_motor2d::enganche::Ajustes {
                activo: false,
                esquinas: false,
                medios: false,
                centros: false,
                radio_px: 20.0,
            },
            rendimiento: Rendimiento {
                nivel: PreferenciaNivel::Ligero,
                medir_fotogramas: true,
                ..Rendimiento::default()
            },
            capturas: Capturas {
                apilar_segundos: 25,
                esquina: EsquinaPila::ArribaIzquierda,
                icono_segundos: 3,
                dias_caducidad: 30,
            },
            pdf: Pdf {
                aligerar_al_entrar: false,
                nivel: NivelPdf::Extremo,
            },
            herramientas: crate::herramientas::Herramientas {
                apagadas: vec!["lazo".into(), "mosaico".into()],
                ..Default::default()
            },
            pinear: Pinear {
                palabras_propias: true,
                palabras: "reloj=TIMER
sorteito=RULETA"
                    .into(),
            },
            ..Ajustes::default()
        }
    }

    /// Las palabras propias sobreviven a cerrar y abrir PixPin, y volver a
    /// las de fabrica tambien, aunque el guardado conserve lo que habia.
    #[test]
    fn las_palabras_propias_sobreviven_a_reiniciar_y_volver_a_fabrica_tambien() {
        let u = temporal("palabras");
        let mut a = Ajustes::default();
        assert_eq!(a.pinear.guardadas(), None, "de fabrica, las de fabrica");
        a.pinear = Pinear {
            palabras_propias: true,
            palabras: "reloj=TIMER".into(),
        };
        guardar_conservando(&u, &a).unwrap();
        let leidos = cargar(&u).unwrap();
        assert_eq!(leidos.pinear.guardadas(), Some("reloj=TIMER"));
        // «Ninguna» (vacia) es distinto de «las de fabrica».
        a.pinear.palabras.clear();
        guardar_conservando(&u, &a).unwrap();
        assert_eq!(cargar(&u).unwrap().pinear.guardadas(), Some(""));
        // De fabrica otra vez: la lista vieja puede quedarse escrita, pero
        // ya no manda.
        a.pinear = Pinear::default();
        guardar_conservando(&u, &a).unwrap();
        assert_eq!(cargar(&u).unwrap().pinear.guardadas(), None);
    }

    #[test]
    fn ningun_ajuste_se_queda_sin_guardar() {
        // La red de seguridad de `guardar_conservando`: como la fusion es
        // generica, anadir un ajuste nuevo NO deberia pedir tocarla. Esta
        // prueba lo comprueba de verdad, con todos los campos cambiados y
        // una vuelta completa por el disco.
        let u = temporal("conservando-todo");
        let esperado = todo_cambiado();
        guardar_conservando(&u, &esperado).unwrap();
        assert_eq!(cargar(&u).unwrap(), esperado);
    }

    #[test]
    fn guardar_no_se_lleva_por_delante_los_comentarios() {
        // Es la razon de ser de esta funcion. `guardar` a secas reescribe
        // el fichero de cero y borraria todo esto, que en el fichero real
        // del usuario es la mitad del contenido.
        let u = temporal("conservando-comentarios");
        let antes = "# Lo que explica el fichero entero.
\n                     limite_scroll_px = 100

\n                     # Lo que explica los comandos.
\n                     [comandos]
\n                     # Este captura y copia.
\n                     capturar-y-copiar = \"Ctrl+Alt+C\"
\n                     # Una clave que no conocemos, escrita a mano.
\n                     invento-del-usuario = \"Ctrl+F12\"
";
        fs::write(u.fichero_ajustes(), antes).unwrap();

        let mut a = cargar(&u).unwrap();
        a.limite_scroll_px = 999;
        guardar_conservando(&u, &a).unwrap();

        let despues = fs::read_to_string(u.fichero_ajustes()).unwrap();
        assert!(despues.contains("# Lo que explica el fichero entero."));
        assert!(despues.contains("# Lo que explica los comandos."));
        assert!(despues.contains("# Este captura y copia."));
        // Lo que el usuario escribio y nosotros no entendemos SE QUEDA:
        // borrarselo no es asunto nuestro.
        assert!(
            despues.contains("invento-del-usuario"),
            "se perdio una clave ajena:
{despues}"
        );
        // Y el valor si cambio.
        assert_eq!(cargar(&u).unwrap().limite_scroll_px, 999);
    }

    #[test]
    fn un_fichero_roto_no_impide_guardar() {
        // Caso negativo: quedarse sin poder guardar porque lo que habia
        // antes estaba mal escrito dejaria al usuario atrapado, sin manera
        // de arreglarlo desde el programa. Se empieza de cero y se guarda.
        let u = temporal("conservando-roto");
        fs::write(u.fichero_ajustes(), "esto no [[[ es TOML").unwrap();
        let a = Ajustes {
            limite_scroll_px: 42,
            ..Ajustes::default()
        };
        guardar_conservando(&u, &a).unwrap();
        assert_eq!(cargar(&u).unwrap().limite_scroll_px, 42);
    }

    #[test]
    fn una_clave_suelta_tras_una_seccion_no_es_del_nivel_de_arriba() {
        // Esto no prueba nuestro codigo, prueba TOML â y esta aqui porque
        // ya me costo un fallo en el fichero del usuario: puse
        // `ignorar_programas` DEBAJO de `[gif]`, y TOML lo leyo como
        // `gif.ignorar_programas`. Serde ignora lo que no conoce, asi que
        // el ajuste desaparecio sin una sola queja.
        //
        // Una clave de primer nivel tiene que ir ANTES de la primera
        // seccion. Si algun dia se genera este fichero desde codigo, esta
        // prueba dice por que el orden importa.
        let mal = "[gif]
por_segundo = 20
ignorar_programas = ['x.exe']
";
        let a: Ajustes = toml::from_str(mal).expect("carga igual, y ese es el problema");
        assert_eq!(a.gif.por_segundo, 20);
        assert!(
            a.ignorar_programas.is_empty(),
            "la clave se perdio dentro de [gif], como se esperaba"
        );
        // Puesta donde toca, si llega.
        let bien = "ignorar_programas = ['x.exe']
[gif]
por_segundo = 20
";
        let a: Ajustes = toml::from_str(bien).unwrap();
        assert_eq!(a.ignorar_programas, vec!["x.exe".to_string()]);
        assert_eq!(a.gif.por_segundo, 20);
    }

    #[test]
    fn lo_que_se_elige_al_grabar_vuelve_igual() {
        let u = temporal("gif");
        let mut a = Ajustes::default();
        assert_eq!(a.gif.por_segundo, 10);
        assert_eq!(a.gif.retardo_s, 0);
        a.gif.por_segundo = 25;
        a.gif.retardo_s = 3;
        guardar(&u, &a).unwrap();
        assert_eq!(cargar(&u).unwrap().gif, a.gif);
    }

    #[test]
    fn un_fichero_sin_la_seccion_de_grabar_sigue_valiendo() {
        // Caso negativo del que se rompe al anadir un ajuste: el fichero
        // de quien ya tenia la aplicacion no lleva la seccion nueva, y no
        // puede dejar de cargar por eso. Lo que falta toma su valor por
        // defecto y el resto se respeta.
        let u = temporal("gif-viejo");
        fs::write(
            u.fichero_ajustes(),
            "limite_scroll_px = 1234
arranque_con_windows = true
",
        )
        .unwrap();
        let a = cargar(&u).unwrap();
        assert_eq!(a.limite_scroll_px, 1234);
        assert!(a.arranque_con_windows);
        assert_eq!(a.gif, Gif::default());
    }

    #[test]
    fn los_valores_por_defecto_son_los_del_diseno() {
        let a = Ajustes::default();
        // Un solo atajo general (D140); lo demas va por gesto (Alt + boton)
        // y por la bandeja.
        assert_eq!(
            a.atajos.region.map(|r| r.to_string()).as_deref(),
            Some("Ctrl+Alt+X")
        );
        for otro in [
            a.atajos.copiar,
            a.atajos.scroll,
            a.atajos.cuentagotas,
            a.atajos.pin,
            a.atajos.portapapeles,
            a.atajos.anotar,
            a.atajos.anotar_congelada,
        ] {
            assert_eq!(otro, None, "ningun otro atajo de fabrica");
        }
        assert_eq!(a.idioma, PreferenciaIdioma::Sistema);
        assert_eq!(a.formato_color, FormatoColor::Hex);
        // Arranca con Windows de fabrica: lo pidio el usuario («que apenas
        // prendo se encienda la app»). Vive en la bandeja, asi que no se
        // pone delante de nadie; quien no lo quiera lo apaga en los ajustes.
        assert!(a.arranque_con_windows);
    }

    #[test]
    fn la_pila_de_capturas_nace_encendida_con_diez_segundos_abajo_a_la_derecha() {
        let a = Ajustes::default();
        assert_eq!(a.capturas.apilar_segundos, 10);
        assert_eq!(a.capturas.esquina, EsquinaPila::AbajoDerecha);
        assert_eq!(a.capturas.icono_segundos, 8);
    }

    #[test]
    fn la_pila_se_lee_del_toml_y_se_apaga_entera_con_un_cero() {
        let a: Ajustes =
            toml::from_str("[capturas]\napilar_segundos = 25\nesquina = \"arriba-izquierda\"")
                .unwrap();
        assert_eq!(a.capturas.apilar_segundos, 25);
        assert_eq!(a.capturas.esquina, EsquinaPila::ArribaIzquierda);
        // Lo que no se nombra conserva su valor de fabrica, como el resto.
        assert_eq!(a.capturas.icono_segundos, 8);

        // El interruptor de apagado: `0` y no un booleano aparte, porque «no
        // apilar» es exactamente «la ventana de apilado dura cero».
        let apagada: Ajustes = toml::from_str("[capturas]\napilar_segundos = 0").unwrap();
        assert_eq!(apagada.capturas.apilar_segundos, 0);

        // Y un fichero de antes de que esto existiera sigue abriendo con la
        // pila encendida: es la funcion nueva, no una migracion.
        let viejo: Ajustes = toml::from_str("limite_scroll_px = 100").unwrap();
        assert_eq!(viejo.capturas, Capturas::default());
    }

    #[test]
    fn las_capturas_caducan_a_los_siete_dias_y_un_fichero_viejo_sigue_igual() {
        assert_eq!(Ajustes::default().capturas.dias_caducidad, 7);
        // Un fichero de antes, con `[capturas]` pero sin la clave nueva.
        let viejo: Ajustes = toml::from_str("[capturas]\napilar_segundos = 25").unwrap();
        assert_eq!(viejo.capturas.dias_caducidad, 7);
        let a: Ajustes = toml::from_str("[capturas]\ndias_caducidad = 30").unwrap();
        assert_eq!(a.capturas.dias_caducidad, 30);
        // Caso negativo: cero no cae al de fabrica, es «no se va ninguna».
        let nunca: Ajustes = toml::from_str("[capturas]\ndias_caducidad = 0").unwrap();
        assert_eq!(nunca.capturas.dias_caducidad, 0);
    }

    #[test]
    fn una_esquina_desconocida_da_error_en_vez_de_adivinar() {
        // Caso negativo: sin el guion no es ninguna de las cuatro, y caer a
        // la de por defecto dejaria el icono en otro sitio sin decir nada.
        assert!(toml::from_str::<Ajustes>("[capturas]\nesquina = \"abajo derecha\"").is_err());
        assert!(toml::from_str::<Ajustes>("[capturas]\nesquina = \"centro\"").is_err());
    }

    #[test]
    fn el_tema_cosmos_viene_apagado_se_guarda_y_un_fichero_viejo_lo_deja_apagado() {
        assert!(!Ajustes::default().tema_cosmos);
        let a = Ajustes {
            tema_cosmos: true,
            ..Default::default()
        };
        let texto = toml::to_string_pretty(&a).unwrap();
        assert!(toml::from_str::<Ajustes>(&texto).unwrap().tema_cosmos);
        // Caso negativo: un TOML de antes, sin la clave.
        let viejo: Ajustes = toml::from_str("arranque_con_windows = true\n").unwrap();
        assert!(!viejo.tema_cosmos);
    }

    #[test]
    fn sobrevive_la_ida_y_vuelta_por_toml() {
        let original = Ajustes {
            idioma: PreferenciaIdioma::Ingles,
            arranque_con_windows: true,
            atajos: Atajos {
                region: Some("Ctrl+Shift+F1".parse().unwrap()),
                ..Default::default()
            },
            ..Default::default()
        };

        let texto = toml::to_string_pretty(&original).unwrap();
        let vuelta: Ajustes = toml::from_str(&texto).unwrap();

        assert_eq!(original, vuelta);
    }

    #[test]
    fn si_no_hay_fichero_se_usan_los_valores_por_defecto() {
        let u = temporal("sin-fichero");
        let a = cargar(&u).unwrap();
        assert_eq!(a, Ajustes::default());
    }

    #[test]
    fn un_fichero_a_medias_completa_con_los_valores_por_defecto() {
        // Es el caso real de un usuario que edita el TOML a mano y solo
        // escribe lo que quiere cambiar. No debe romper nada.
        let u = temporal("parcial");
        fs::write(u.fichero_ajustes(), "arranque_con_windows = true\n").unwrap();

        let a = cargar(&u).unwrap();

        assert!(a.arranque_con_windows);
        assert_eq!(a.atajos, Atajos::default());
    }

    #[test]
    fn las_claves_desconocidas_se_ignoran() {
        // Compatibilidad hacia atras: un fichero escrito por una version mas
        // nueva no debe impedir que arranque una version mas vieja.
        let u = temporal("desconocidas");
        fs::write(
            u.fichero_ajustes(),
            "arranque_con_windows = true\nfuncion_del_futuro = 42\n",
        )
        .unwrap();

        let a = cargar(&u).unwrap();

        assert!(a.arranque_con_windows);
    }

    #[test]
    fn un_atajo_invalido_da_error_con_mensaje_util() {
        let u = temporal("atajo-malo");
        fs::write(u.fichero_ajustes(), "[atajos]\nregion = \"NoEsUnAtajo\"\n").unwrap();

        let e = cargar(&u).unwrap_err();

        assert!(
            e.to_string().contains("NoEsUnAtajo"),
            "el error debe decir que valor concreto esta mal, dijo: {e}"
        );
    }

    #[test]
    fn el_nivel_de_rendimiento_se_lee_y_por_defecto_es_auto() {
        let ajustes: Ajustes = toml::from_str("[rendimiento]\nnivel = \"ligero\"").unwrap();
        assert_eq!(ajustes.rendimiento.nivel, PreferenciaNivel::Ligero);
        // Un fichero sin la seccion conserva el valor por defecto: la regla
        // de "todo campo que falte se rellena" tambien vale para secciones.
        let vacios: Ajustes = toml::from_str("").unwrap();
        assert_eq!(vacios.rendimiento.nivel, PreferenciaNivel::Auto);
    }

    #[test]
    fn la_tinta_nace_como_siempre_y_el_toml_la_puede_poner_natural() {
        // Lo que no se escribe no cambia el trazo: un fichero sin `[tinta]`
        // tiene que dar exactamente el lapiz de antes.
        let vacios: Ajustes = toml::from_str("").unwrap();
        assert_eq!(vacios.tinta.suavizado, Suavizado::Excalidraw);
        assert_eq!(vacios.tinta.corte_minimo, None);

        let a: Ajustes = toml::from_str("[tinta]\nsuavizado = \"natural\"\nbeta = 0.05").unwrap();
        assert_eq!(a.tinta.suavizado, Suavizado::Natural);
        assert_eq!(a.tinta.beta, Some(0.05));
        // Lo que no se nombra sigue en `None`, que significa «el valor
        // conservador del motor», no cero.
        assert_eq!(a.tinta.corte_minimo, None);
    }

    #[test]
    fn un_suavizado_desconocido_da_error_en_vez_de_adivinar() {
        // Caso negativo: si "suabe" pasara por bueno, el usuario creeria
        // haber encendido algo que no esta pasando.
        assert!(toml::from_str::<Ajustes>("[tinta]\nsuavizado = \"suabe\"").is_err());
    }

    #[test]
    fn un_nivel_desconocido_da_error_en_vez_de_adivinar() {
        // Caso negativo: "turbo" no existe. Adivinar seria peor que fallar,
        // porque el usuario cree haber forzado algo que no esta pasando.
        let resultado = toml::from_str::<Ajustes>("[rendimiento]\nnivel = \"turbo\"");
        assert!(resultado.is_err());
    }

    #[test]
    fn medir_fotogramas_se_lee_y_por_defecto_esta_apagado() {
        let a: Ajustes = toml::from_str("[rendimiento]\nmedir_fotogramas = true").unwrap();
        assert!(a.rendimiento.medir_fotogramas);
        assert_eq!(
            a.rendimiento.nivel,
            PreferenciaNivel::Auto,
            "lo que no se nombra conserva su valor"
        );
        // Caso negativo: un fichero de antes no tiene la clave y no mide nada.
        let viejo: Ajustes = toml::from_str("[rendimiento]\nnivel = \"ligero\"").unwrap();
        assert!(!viejo.rendimiento.medir_fotogramas);
    }

    #[test]
    fn el_ritmo_y_el_paneo_por_composicion_se_leen_del_toml() {
        // Las dos palancas de la tarea A3/B1: una regresion se apaga con una
        // linea del fichero, sin volver a compilar.
        let a: Ajustes =
            toml::from_str("[rendimiento]\nritmo = true\npaneo_por_composicion = false").unwrap();
        assert!(a.rendimiento.ritmo);
        assert!(!a.rendimiento.paneo_por_composicion);
    }

    #[test]
    fn un_fichero_viejo_no_enciende_el_ritmo_pero_si_el_paneo_por_composicion() {
        // Caso negativo: el ritmo cambia CUANDO se pinta, asi que nace
        // apagado; el paneo por composicion solo cambia COMO se mueve lo ya
        // pintado y es la mejora que se viene a dar, asi que nace encendido.
        let viejo: Ajustes = toml::from_str("[rendimiento]\nnivel = \"ligero\"").unwrap();
        assert!(!viejo.rendimiento.ritmo);
        assert!(viejo.rendimiento.paneo_por_composicion);
    }

    #[test]
    fn guardar_crea_el_directorio_si_no_existe() {
        let dir = std::env::temp_dir().join("pixpin-ajustes-crear/anidado");
        let _ = fs::remove_dir_all(dir.parent().unwrap());
        let u = Ubicacion::Instalado { raiz: dir.clone() };

        guardar(&u, &Ajustes::default()).unwrap();

        assert!(u.fichero_ajustes().is_file());
        assert_eq!(cargar(&u).unwrap(), Ajustes::default());
    }

    #[test]
    fn los_ajustes_del_iman_van_y_vuelven() {
        let mut a = Ajustes::default();
        a.enganche.centros = false;
        a.enganche.radio_px = 20.0;

        let texto = toml::to_string(&a).expect("serializa");
        let vuelta: Ajustes = toml::from_str(&texto).expect("deserializa");

        assert!(!vuelta.enganche.centros);
        assert_eq!(vuelta.enganche.radio_px, 20.0);
        assert!(vuelta.enganche.activo, "lo no tocado conserva su valor");
    }

    #[test]
    fn un_fichero_viejo_sin_iman_sigue_abriendo() {
        // Nadie tiene la seccion `[enganche]` en su fichero todavia. Si
        // faltar la rompiera, la actualizacion le borraria los ajustes.
        // El TOML de abajo se parece a un fichero real de hoy -claves
        // sueltas, `[atajos]`, `[comandos]`, `[gif]`, `[rendimiento]` y una
        // tabla repetible `[[regiones]]`- y no a un minimo artificial que
        // no ejercite el caso.
        let viejo = "arranque_con_windows = true
limite_scroll_px = 20000
retardo_captura_s = 5
abrir_con = true

[atajos]
copiar = \"Ctrl+Alt+C\"
scroll = \"Ctrl+Alt+S\"
pin = \"Ctrl+Alt+F\"
portapapeles = \"Ctrl+Alt+V\"
anotar = \"Ctrl+Alt+A\"

[comandos]
capturar-y-copiar = \"Ctrl+Alt+C\"

[gif]
por_segundo = 15
retardo_s = 2

[rendimiento]
nivel = \"auto\"

[[regiones]]
nombre = \"panel\"
x = 1
y = 2
ancho = 300
alto = 200
";
        let a: Ajustes = toml::from_str(viejo).expect("un fichero viejo tiene que abrir");
        assert!(a.enganche.activo, "el iman nace encendido");
        assert_eq!(a.enganche.radio_px, 14.0);
        // Y el resto del fichero viejo se sigue leyendo tal cual.
        assert_eq!(a.limite_scroll_px, 20000);
        assert_eq!(a.regiones.len(), 1);
    }

    #[test]
    fn el_iman_sobrevive_la_ida_y_vuelta_con_regiones_de_por_medio() {
        // Esta prueba fija un supuesto del que dependemos sin haberlo
        // escrito en ningun sitio: que `toml` coloca las TABLAS (una
        // seccion `[tabla]`, o una tabla repetible `[[tabla]]`) despues de
        // las claves sueltas del fichero de salida, sin importar en que
        // orden aparecen los campos en el struct de Rust. Por eso
        // `abrir_con` (booleano suelto) y `enganche` (tabla) pueden
        // convivir con `regiones` (tabla repetible) sin que el fichero
        // salga invalido, aunque `abrir_con` y `enganche` esten declarados
        // DESPUES de `regiones` en el struct `Ajustes`.
        //
        // Las otras pruebas de ida-y-vuelta no lo cubren: las de `toml`
        // puro (`sobrevive_la_ida_y_vuelta_por_toml`,
        // `los_ajustes_del_iman_van_y_vuelven`) parten de `regiones`
        // vacio, y la que si tiene `regiones` poblado
        // (`ningun_ajuste_se_queda_sin_guardar`) pasa por
        // `guardar_conservando` -> `toml_edit` -> `fusionar`, que asigna
        // por nombre de clave y no tiene la restriccion de orden que aqui
        // se comprueba. Esta prueba usa `toml::to_string_pretty` sobre el
        // struct entero, que es lo que hace `guardar()`.
        let original = Ajustes {
            regiones: vec![crate::regiones::Region {
                nombre: "panel".into(),
                x: 1,
                y: 2,
                ancho: 300,
                alto: 200,
                atajo: Some("Ctrl+Alt+1".into()),
            }],
            enganche: pixpin_motor2d::enganche::Ajustes {
                activo: false,
                esquinas: false,
                medios: false,
                centros: false,
                radio_px: 20.0,
            },
            ..Ajustes::default()
        };

        let texto = toml::to_string_pretty(&original).expect("serializa");
        let vuelta: Ajustes = toml::from_str(&texto).expect("un fichero valido tiene que releerse");

        assert_eq!(
            vuelta.regiones, original.regiones,
            "las regiones sobreviven a la ida y vuelta:\n{texto}"
        );
        assert_eq!(
            vuelta.enganche, original.enganche,
            "el iman sobrevive a la ida y vuelta con regiones de por medio:\n{texto}"
        );
    }

    #[test]
    fn la_voz_viene_sin_segundo_idioma_y_cada_trozo_como_se_dijo() {
        let a: Ajustes = toml::from_str("").unwrap();
        assert_eq!(a.voz.segundo_idioma, "");
        assert_eq!(a.voz.modo_de_idiomas, ModoDeIdiomas::CadaUno);
    }

    #[test]
    fn el_modo_de_idiomas_se_escribe_con_las_palabras_del_movil() {
        let a: Ajustes =
            toml::from_str("[voz]\nsegundo_idioma = \"en\"\nmodo_de_idiomas = \"todo_en_uno\"\n")
                .unwrap();
        assert_eq!(a.voz.segundo_idioma, "en");
        assert_eq!(a.voz.modo_de_idiomas, ModoDeIdiomas::TodoEnUno);
        let texto = toml::to_string(&a).unwrap();
        assert!(
            texto.contains("modo_de_idiomas = \"todo_en_uno\""),
            "{texto}"
        );
        // Caso negativo: una palabra que no es del movil no se acepta callada.
        assert!(toml::from_str::<Ajustes>("[voz]\nmodo_de_idiomas = \"mezcla\"\n").is_err());
    }

    #[test]
    fn aligerar_los_pdf_viene_encendido_y_en_equilibrado_como_en_el_movil() {
        let a: Ajustes = toml::from_str("").unwrap();
        assert!(a.pdf.aligerar_al_entrar);
        assert_eq!(a.pdf.nivel, NivelPdf::Equilibrado);
    }

    #[test]
    fn el_nivel_del_pdf_se_lee_con_su_nombre_o_con_el_de_pdfsqueeze() {
        let a: Ajustes = toml::from_str("[pdf]\nnivel = \"pequeno\"\n").unwrap();
        assert_eq!(a.pdf.nivel, NivelPdf::Pequeno);
        assert!(
            a.pdf.aligerar_al_entrar,
            "lo que no se escribe sigue de fabrica"
        );
        let b: Ajustes =
            toml::from_str("[pdf]\nnivel = \"lossless\"\naligerar_al_entrar = false\n").unwrap();
        assert_eq!(b.pdf.nivel, NivelPdf::SinPerdida);
        assert!(!b.pdf.aligerar_al_entrar);
        // Caso negativo: un nivel que no existe es un error, no un nivel
        // cualquiera en silencio.
        assert!(toml::from_str::<Ajustes>("[pdf]\nnivel = \"maximo\"\n").is_err());
    }

    #[test]
    fn el_ajuste_del_pdf_va_y_vuelve_por_el_fichero() {
        let donde = temporal("pdf");
        let a = Ajustes {
            pdf: Pdf {
                aligerar_al_entrar: false,
                nivel: NivelPdf::SinPerdida,
            },
            ..Ajustes::default()
        };
        guardar_conservando(&donde, &a).unwrap();
        let texto = fs::read_to_string(donde.fichero_ajustes()).unwrap();
        assert!(texto.contains("sin-perdida"), "{texto}");
        assert_eq!(cargar(&donde).unwrap().pdf, a.pdf);
    }
}
