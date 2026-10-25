//! **El motor de video de la clase, sin nada del Mac** (Windows bea497b,
//! 32973c1 y cdad692, `pixpin-motor-video`): los perfiles, con que se
//! codifica, cuando se escribe un fotograma, en que instante del MP4 va y
//! que hacer cuando la pantalla se va. Lo de Windows al numero, para que un
//! video de una reunion pese y se vea igual en los dos.
//!
//! Lo que habla con el Mac es de Swift (`Grabar/VideoDeClase.swift`):
//! ScreenCaptureKit, que ya encoge la pantalla en la GPU y solo da un
//! fotograma cuando algo cambia, y `AVAssetWriter` con el H.264 de
//! VideoToolbox (la grafica; si no, por software), de modo que la imagen no
//! baja nunca a la CPU. Aqui:
//!
//! - [`PerfilVideo`]: «Ligero» (720p/10), «Normal» (1080p/15) y «Nitido»
//!   (la pantalla tal cual/30), con sus bits (`perfil.rs`).
//! - [`caminos_del_video`]: la cadena de caidas elegida al empezar
//!   (`cadena.rs`): la grafica, luego a 1080p y 720p, el procesador y por
//!   ultimo el modo sencillo; [`video_con`] dice cual quedo.
//! - [`RitmoDelVideo`]: el ritmo variable (`ritmo.rs`: un fotograma nuevo en
//!   cuanto llega y paso el minimo, el ultimo repetido cada segundo), el
//!   reloj (`reloj.rs`: manda el sonido; un fotograma va en su ancla mas lo
//!   que paso), la pista de sonido que no se pisa (`audio.rs`), el atasco
//!   (`cola.rs`) y volver a abrir la pantalla que se fue (`reapertura.rs` y
//!   `recuperacion.rs`).
//!
//! Todas las horas van en unidades de 100 ns (las de Windows): Swift pasa la
//! hora del sistema (`CMClockGetHostTimeClock`) a esa unidad.

use std::sync::Mutex;

/// 100 ns por segundo.
pub const UNIDADES_POR_SEGUNDO: i64 = 10_000_000;
/// El tope duro de «la pantalla tal cual»: 4K (`geometria::TOPE_NATIVO`).
pub const TOPE_NATIVO: (u32, u32) = (3840, 2160);
/// Bits por segundo del AAC del video: los 96 kbps de la nota de voz
/// (`BYTES_POR_SEGUNDO_AAC`, 12 000 bytes).
pub const BITS_AUDIO: u32 = 96_000;

// --- Los perfiles (`perfil.rs`) -------------------------------------------

/// Con que calidad se graba (`pixpin_motor_video::Perfil`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, uniffi::Enum)]
pub enum PerfilVideo {
    /// 720p a 10 fps: lo minimo para leer una diapositiva.
    Ligero,
    /// 1080p a 15 fps: el de salida.
    #[default]
    Normal,
    /// La pantalla tal cual (hasta 4K) a 30 fps.
    Nitido,
}

/// Lo que un perfil le pide al codificador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ajustes {
    /// El tamano mas grande; `None`, el de la pantalla (con [`TOPE_NATIVO`]).
    pub tope: Option<(u32, u32)>,
    /// Fotogramas por segundo como mucho.
    pub por_segundo: u32,
    /// Bits por segundo de media y de pico para [`Ajustes::referencia`].
    pub media: u32,
    pub pico: u32,
    pub referencia: (u32, u32),
}

impl PerfilVideo {
    /// En el orden en que se ofrecen.
    pub const TODOS: [PerfilVideo; 3] = [PerfilVideo::Ligero, PerfilVideo::Normal, PerfilVideo::Nitido];

    pub fn ajustes(self) -> Ajustes {
        match self {
            PerfilVideo::Ligero => Ajustes {
                tope: Some((1280, 720)),
                por_segundo: 10,
                media: 300_000,
                pico: 1_500_000,
                referencia: (1280, 720),
            },
            PerfilVideo::Normal => Ajustes {
                tope: Some((1920, 1080)),
                por_segundo: 15,
                media: 550_000,
                pico: 3_000_000,
                referencia: (1920, 1080),
            },
            PerfilVideo::Nitido => Ajustes {
                tope: None,
                por_segundo: 30,
                media: 1_200_000,
                pico: 6_000_000,
                referencia: (1920, 1080),
            },
        }
    }

    /// El siguiente al pulsar (vuelve al primero tras el ultimo).
    pub fn siguiente(self) -> PerfilVideo {
        match self {
            PerfilVideo::Ligero => PerfilVideo::Normal,
            PerfilVideo::Normal => PerfilVideo::Nitido,
            PerfilVideo::Nitido => PerfilVideo::Ligero,
        }
    }

    /// Un nombre fijo (`Perfil::clave`).
    pub fn clave(self) -> &'static str {
        match self {
            PerfilVideo::Ligero => "ligero",
            PerfilVideo::Normal => "normal",
            PerfilVideo::Nitido => "nitido",
        }
    }

    /// El de una clave; cualquier otra cosa, «Normal».
    pub fn de_clave(clave: &str) -> PerfilVideo {
        PerfilVideo::TODOS
            .into_iter()
            .find(|p| p.clave() == clave)
            .unwrap_or_default()
    }

    /// Las medidas del video para una pantalla de `ancho`x`alto` pixeles.
    pub fn medidas(self, ancho: u32, alto: u32) -> (u32, u32) {
        let (tw, th) = self.ajustes().tope.unwrap_or(TOPE_NATIVO);
        crate::clase::medida_de_salida(ancho, alto, tw, th)
    }
}

/// **Bits por segundo de media y de pico** para un video de `ancho`x`alto`:
/// proporcionales a los pixeles respecto a la referencia, entre la mitad y
/// cuatro veces (`perfil::bits_por_segundo`).
pub fn bits_por_segundo(a: &Ajustes, ancho: u32, alto: u32) -> (u32, u32) {
    let px = u64::from(ancho.max(2)) * u64::from(alto.max(2));
    let referencia = u64::from(a.referencia.0.max(2)) * u64::from(a.referencia.1.max(2));
    let factor = (px * 1000 / referencia).clamp(500, 4000);
    let escalar = |v: u32| (u64::from(v) * factor / 1000).min(u64::from(u32::MAX)) as u32;
    (escalar(a.media), escalar(a.pico))
}

/// Fotogramas entre dos fotogramas clave: dos segundos al ritmo maximo.
pub fn grupo_de_imagenes(a: &Ajustes) -> u32 {
    a.por_segundo.max(1) * 2
}

/// Megas por hora en el peor caso: el video a su media todo el rato y el
/// sonido AAC.
pub fn megas_por_hora(media_video: u32, con_sonido: bool) -> u64 {
    let video = u64::from(media_video) / 8 * 3600;
    let sonido = if con_sonido { u64::from(BITS_AUDIO / 8) * 3600 } else { 0 };
    (video + sonido) / 1_000_000
}

/// El siguiente perfil al pulsar la ficha de la calidad.
#[uniffi::export]
pub fn perfil_del_video_siguiente(perfil: PerfilVideo) -> PerfilVideo {
    perfil.siguiente()
}

/// La clave de un perfil (`ligero`, `normal`, `nitido`).
#[uniffi::export]
pub fn clave_del_perfil(perfil: PerfilVideo) -> String {
    perfil.clave().to_string()
}

/// El perfil de una clave guardada; lo raro, «Normal».
#[uniffi::export]
pub fn perfil_de_clave(clave: String) -> PerfilVideo {
    PerfilVideo::de_clave(&clave)
}

// --- Los textos (`clase-calidad-*` y `clase-video-con`) -------------------

/// El rotulo de la ficha de la calidad (`clase-calidad-ligero`...).
#[uniffi::export]
pub fn rotulo_de_calidad(perfil: PerfilVideo) -> String {
    match perfil {
        PerfilVideo::Ligero => "Ligero",
        PerfilVideo::Normal => "Normal",
        PerfilVideo::Nitido => "Nítido",
    }
    .to_string()
}

/// Lo que pesa cada calidad, para la pista con el raton encima
/// (`clase-calidad-*-pista`).
#[uniffi::export]
pub fn pista_de_calidad(perfil: PerfilVideo) -> String {
    match perfil {
        PerfilVideo::Ligero => "Vídeo a 720p y 10 fps: lo que menos pesa (unos 180 MB por hora)",
        PerfilVideo::Normal => "Vídeo a 1080p y 15 fps: unos 300 MB por hora como mucho",
        PerfilVideo::Nitido => "La pantalla tal cual a 30 fps: el texto pequeño se lee, pesa más",
    }
    .to_string()
}

/// «Video con la grafica…», lo que dice la barra al empezar
/// (`clase-video-con` con `clase-codificador-*`).
#[uniffi::export]
pub fn video_con(codificador: CodificadorVideo) -> String {
    let c = match codificador {
        CodificadorVideo::Hardware => "la gráfica (casi sin gastar procesador)",
        CodificadorVideo::Software => "el procesador",
        CodificadorVideo::Cpu => "el procesador, en modo sencillo",
    };
    format!("Vídeo con {c}")
}

// --- La cadena de caidas (`cadena.rs`) ------------------------------------

/// Con que se codifica (`pixpin_motor_video::Codificador`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum CodificadorVideo {
    /// El H.264 de la grafica (VideoToolbox por hardware).
    Hardware,
    /// El H.264 por software, con la imagen aun encogida en la GPU.
    Software,
    /// El modo sencillo: por software y en «Ligero» (el ultimo recurso).
    Cpu,
}

impl CodificadorVideo {
    /// Un nombre fijo, para el registro.
    pub fn clave(self) -> &'static str {
        match self {
            CodificadorVideo::Hardware => "grafica",
            CodificadorVideo::Software => "software",
            CodificadorVideo::Cpu => "cpu",
        }
    }
}

/// Un intento de abrir el codificador: con que, a que tamano y con que bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct CaminoDelVideo {
    pub codificador: CodificadorVideo,
    /// El tamano del MP4, pares.
    pub ancho: u32,
    pub alto: u32,
    pub por_segundo: u32,
    pub bits_media: u32,
    pub bits_pico: u32,
    /// Fotogramas entre dos fotogramas clave.
    pub grupo: u32,
    pub bits_audio: u32,
}

fn camino(codificador: CodificadorVideo, a: &Ajustes, (ancho, alto): (u32, u32)) -> CaminoDelVideo {
    let (bits_media, bits_pico) = bits_por_segundo(a, ancho, alto);
    CaminoDelVideo {
        codificador,
        ancho,
        alto,
        por_segundo: a.por_segundo,
        bits_media,
        bits_pico,
        grupo: grupo_de_imagenes(a),
        bits_audio: BITS_AUDIO,
    }
}

/// Lo siguiente a probar si la grafica no admite estas medidas: 1080p y
/// luego 720p, en vertical si la pantalla lo es (`reintento_mas_pequeno`).
pub fn reintento_mas_pequeno((w, h): (u32, u32)) -> Option<(u32, u32)> {
    for tope in [(1920, 1080), (1280, 720)] {
        let tope = if h > w { (tope.1, tope.0) } else { tope };
        if w > tope.0 || h > tope.1 {
            return Some(crate::clase::medida_de_salida(w, h, tope.0, tope.1));
        }
    }
    None
}

/// **Lo que se intenta, en orden**, para grabar una pantalla de
/// `ancho`x`alto` pixeles con `perfil`: la grafica (si `hay_hardware`) a
/// las medidas del perfil y, si no arranca, a 1080p y 720p; el procesador a
/// las medidas del perfil; y el modo sencillo («Ligero» por software). El
/// primero que abre es el que graba.
#[uniffi::export]
pub fn caminos_del_video(
    perfil: PerfilVideo,
    ancho_pantalla: u32,
    alto_pantalla: u32,
    hay_hardware: bool,
) -> Vec<CaminoDelVideo> {
    let a = perfil.ajustes();
    let medidas = perfil.medidas(ancho_pantalla, alto_pantalla);
    let mut v = Vec::with_capacity(5);
    if hay_hardware {
        v.push(camino(CodificadorVideo::Hardware, &a, medidas));
        let mut m = medidas;
        while let Some(menor) = reintento_mas_pequeno(m) {
            v.push(camino(CodificadorVideo::Hardware, &a, menor));
            m = menor;
        }
    }
    v.push(camino(CodificadorVideo::Software, &a, medidas));
    let ligero = PerfilVideo::Ligero;
    v.push(camino(
        CodificadorVideo::Cpu,
        &ligero.ajustes(),
        ligero.medidas(ancho_pantalla, alto_pantalla),
    ));
    v
}

// --- El ritmo (`ritmo.rs`) ------------------------------------------------

/// Lo que se espera como mucho sin escribir ningun fotograma.
pub const LATIDO: i64 = UNIDADES_POR_SEGUNDO;
/// Lo que se espera a la primera imagen antes de empezar en negro.
pub const ESPERA_PRIMERO: i64 = 5 * UNIDADES_POR_SEGUNDO;

/// El minimo entre fotogramas, con un 5 % de holgura para el monitor.
pub fn minimo_entre(por_segundo: u32) -> i64 {
    UNIDADES_POR_SEGUNDO / i64::from(por_segundo.max(1)) * 95 / 100
}

/// Que hacer con el video ahora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccionDelVideo {
    /// Escribir el fotograma nuevo que llego de la pantalla.
    Nuevo,
    /// Repetir el ultimo (o uno negro si aun no hay ninguno).
    Repetir,
    /// Nada todavia.
    Esperar,
}

fn decidir(ahora: i64, inicio: i64, ultimo: Option<i64>, hay_nuevo: bool, minimo: i64) -> AccionDelVideo {
    match ultimo {
        None if hay_nuevo => AccionDelVideo::Nuevo,
        None if ahora - inicio >= ESPERA_PRIMERO => AccionDelVideo::Repetir,
        None => AccionDelVideo::Esperar,
        Some(u) => {
            let pasado = ahora - u;
            if hay_nuevo && pasado >= minimo {
                AccionDelVideo::Nuevo
            } else if pasado >= LATIDO {
                AccionDelVideo::Repetir
            } else {
                AccionDelVideo::Esperar
            }
        }
    }
}

fn espera(ahora: i64, inicio: i64, ultimo: Option<i64>, hay_nuevo: bool, minimo: i64) -> i64 {
    let hasta = match ultimo {
        None if hay_nuevo => ahora,
        None => inicio + ESPERA_PRIMERO,
        Some(u) if hay_nuevo => u + minimo,
        Some(u) => u + LATIDO,
    };
    (hasta - ahora).max(0)
}

// --- El reloj (`reloj.rs`) ------------------------------------------------

/// Lo que un fotograma puede ir por delante del ultimo sonido recibido.
pub const ADELANTO_MAXIMO: i64 = UNIDADES_POR_SEGUNDO / 4;
/// Lo que puede ir por detras del ultimo ancla.
pub const RETRASO_MAXIMO: i64 = UNIDADES_POR_SEGUNDO;
/// Lo minimo entre dos fotogramas del MP4: un milisegundo.
pub const SEPARACION_MINIMA: i64 = UNIDADES_POR_SEGUNDO / 1000;

#[derive(Debug, Clone, Copy)]
struct Ancla {
    tiempo: i64,
    hora: i64,
}

#[derive(Debug, Clone)]
struct Reloj {
    con_sonido: Option<u32>,
    inicio: i64,
    ancla: Option<Ancla>,
    pausa_desde: Option<i64>,
    pausado: i64,
    ultimo: Option<i64>,
    fin_sonido: i64,
}

impl Reloj {
    fn nuevo(muestreo: Option<u32>, ahora: i64) -> Reloj {
        Reloj {
            con_sonido: muestreo,
            inicio: ahora,
            ancla: None,
            pausa_desde: None,
            pausado: 0,
            ultimo: None,
            fin_sonido: 0,
        }
    }

    fn sonido(&mut self, fin: u64, ahora: i64) {
        let Some(muestreo) = self.con_sonido else {
            return;
        };
        let tiempo = crate::clase::tiempo_de_muestras(fin, muestreo);
        self.fin_sonido = self.fin_sonido.max(tiempo);
        if self.pausa_desde.is_none() {
            self.ancla = Some(Ancla {
                tiempo: self.fin_sonido,
                hora: ahora,
            });
        }
    }

    fn pausar(&mut self, ahora: i64) {
        if self.pausa_desde.is_none() {
            self.pausa_desde = Some(ahora);
            self.ancla = None;
        }
    }

    fn seguir(&mut self, ahora: i64) {
        if let Some(desde) = self.pausa_desde.take() {
            self.pausado += (ahora - desde).max(0);
        }
    }

    fn en_pausa(&self) -> bool {
        self.pausa_desde.is_some()
    }

    /// El instante del MP4 de un fotograma capturado a la hora `hora`, o
    /// `None` si no va (el primero, siempre en el cero).
    fn fotograma(&mut self, hora: i64) -> Option<i64> {
        if self.en_pausa() {
            return None;
        }
        let Some(ultimo) = self.ultimo else {
            self.ultimo = Some(0);
            return Some(0);
        };
        let (t, tope) = match self.con_sonido {
            Some(_) => {
                let a = self.ancla?;
                (
                    a.tiempo + (hora - a.hora).clamp(-RETRASO_MAXIMO, ADELANTO_MAXIMO),
                    a.tiempo + ADELANTO_MAXIMO,
                )
            }
            None => (hora - self.inicio - self.pausado, i64::MAX),
        };
        let t = t.max(ultimo + SEPARACION_MINIMA);
        if t > tope {
            return None;
        }
        self.ultimo = Some(t);
        Some(t)
    }

    fn final_(&self, ahora: i64) -> i64 {
        match self.con_sonido {
            Some(_) => self.fin_sonido,
            None => {
                let en_pausa = self.pausa_desde.map_or(0, |d| (ahora - d).max(0));
                (ahora - self.inicio - self.pausado - en_pausa).max(0)
            }
        }
    }
}

// --- Lo que se hace con lo que llega ---------------------------------------

/// Lo que se aguanta sin poder dar ningun fotograma al codificador.
pub const ATASCO: i64 = 10 * UNIDADES_POR_SEGUNDO;
/// Entre dos intentos de volver a abrir la pantalla (`reapertura::CADA`).
pub const REABRIR_CADA: i64 = 3 * UNIDADES_POR_SEGUNDO;
/// Vueltas que se permiten en [`VENTANA_DE_VUELTAS`] (`recuperacion`).
pub const VUELTAS: usize = 3;
pub const VENTANA_DE_VUELTAS: i64 = 600 * UNIDADES_POR_SEGUNDO;

/// Lo que se escribe de un bloque de sonido: se saltan `salto` muestras del
/// principio (pisaban lo ya escrito) y lo demas va en `desde`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct TramoDeSonido {
    pub salto: u64,
    pub desde: u64,
}

/// Que hacer con el video ahora: la accion, el instante del MP4 en que va
/// (100 ns) y lo que dura un fotograma al ritmo maximo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct PasoDelVideo {
    pub accion: AccionDelVideo,
    pub instante: i64,
    pub duracion: i64,
}

#[derive(Debug)]
struct Estado {
    reloj: Reloj,
    /// Donde acaba el sonido ya escrito (`audio::Pista`).
    fin_sonido: u64,
    inicio: i64,
    minimo: i64,
    duracion: i64,
    ultimo_intento: Option<i64>,
    atasco_desde: Option<i64>,
    proximo_reintento: Option<i64>,
    vueltas: Vec<i64>,
}

/// **El nucleo del video de una grabacion** (`nucleo.rs` de Windows): lo
/// que decide en cada vuelta, sin tocar la pantalla ni el codificador.
/// Swift le cuenta lo que pasa y hace lo que dice.
#[derive(Debug, uniffi::Object)]
pub struct RitmoDelVideo {
    estado: Mutex<Estado>,
}

impl RitmoDelVideo {
    fn estado(&self) -> std::sync::MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[uniffi::export]
impl RitmoDelVideo {
    /// Un video que empieza a la hora `ahora`, con sonido a `muestreo` (cero
    /// si va mudo: entonces manda el reloj del sistema sin pausas), a
    /// `por_segundo` como mucho.
    #[uniffi::constructor]
    pub fn nuevo(muestreo: u32, por_segundo: u32, ahora: i64) -> std::sync::Arc<RitmoDelVideo> {
        std::sync::Arc::new(RitmoDelVideo {
            estado: Mutex::new(Estado {
                reloj: Reloj::nuevo((muestreo > 0).then_some(muestreo), ahora),
                fin_sonido: 0,
                inicio: ahora,
                minimo: minimo_entre(por_segundo),
                duracion: UNIDADES_POR_SEGUNDO / i64::from(por_segundo.max(1)),
                ultimo_intento: None,
                atasco_desde: None,
                proximo_reintento: None,
                vueltas: Vec::new(),
            }),
        })
    }

    /// **Un bloque de sonido** de `marcos` muestras que empieza en
    /// `posicion`, llegado a la hora `ahora`: lo que se escribe de el
    /// (`None` si ya estaba todo escrito). Ancla el reloj.
    pub fn sonido(&self, posicion: u64, marcos: u64, ahora: i64) -> Option<TramoDeSonido> {
        let mut e = self.estado();
        let fin = posicion.saturating_add(marcos);
        if marcos == 0 || fin <= e.fin_sonido {
            return None;
        }
        let salto = e.fin_sonido.saturating_sub(posicion);
        e.fin_sonido = fin;
        e.reloj.sonido(fin, ahora);
        Some(TramoDeSonido {
            salto,
            desde: posicion + salto,
        })
    }

    /// Pausa o sigue a la hora `ahora`. Al seguir, el atasco se cuenta de
    /// cero (una pausa larga no es un codificador atascado).
    pub fn pausar(&self, pausa: bool, ahora: i64) {
        let mut e = self.estado();
        if pausa {
            e.reloj.pausar(ahora);
        } else {
            e.reloj.seguir(ahora);
            e.atasco_desde = None;
        }
    }

    pub fn en_pausa(&self) -> bool {
        self.estado().reloj.en_pausa()
    }

    /// **Que hacer a la hora `ahora`**, con un fotograma nuevo de la
    /// pantalla capturado a `hora_retenida` esperando (o ninguno). Si dice
    /// `Nuevo` o `Repetir`, se escribe en `instante`; si el reloj aun no lo
    /// puede fechar (esperando al sonido tras la pausa), dice `Esperar` y
    /// el fotograma se queda retenido: es la imagen mas nueva.
    pub fn paso(&self, ahora: i64, hora_retenida: Option<i64>) -> PasoDelVideo {
        let mut e = self.estado();
        let duracion = e.duracion;
        let esperar = PasoDelVideo {
            accion: AccionDelVideo::Esperar,
            instante: 0,
            duracion,
        };
        if e.reloj.en_pausa() {
            return esperar;
        }
        let accion = decidir(ahora, e.inicio, e.ultimo_intento, hora_retenida.is_some(), e.minimo);
        let instante = match accion {
            AccionDelVideo::Esperar => return esperar,
            AccionDelVideo::Nuevo => {
                e.ultimo_intento = Some(ahora);
                hora_retenida.and_then(|h| e.reloj.fotograma(h))
            }
            AccionDelVideo::Repetir => {
                e.ultimo_intento = Some(ahora);
                e.reloj.fotograma(ahora)
            }
        };
        match instante {
            Some(instante) => PasoDelVideo {
                accion,
                instante,
                duracion,
            },
            None => esperar,
        }
    }

    /// **Lo que se puede dormir**, en 100 ns, hasta que [`RitmoDelVideo::paso`]
    /// cambie de opinion: entre 1 y 250 ms (en pausa, 250).
    pub fn espera(&self, ahora: i64, hay_retenido: bool) -> i64 {
        let e = self.estado();
        if e.reloj.en_pausa() {
            return UNIDADES_POR_SEGUNDO / 4;
        }
        espera(ahora, e.inicio, e.ultimo_intento, hay_retenido, e.minimo)
            .clamp(10_000, UNIDADES_POR_SEGUNDO / 4)
    }

    /// **Si el fotograma de verdad que llega a `t_nuevo` ocupa el sitio del
    /// repetido que aun espera en `t_pendiente`** (`ocupa_el_sitio_del_repetido`):
    /// el repetido se tira y el de verdad va en su instante, sin dejar un
    /// fotograma de un milisegundo.
    pub fn sustituye_al_repetido(&self, pendiente_es_repetido: bool, t_pendiente: i64, t_nuevo: i64) -> bool {
        let minimo = self.estado().minimo;
        pendiente_es_repetido && t_nuevo >= t_pendiente && t_nuevo - t_pendiente < minimo
    }

    /// Se le dio un fotograma al codificador a la hora `ahora` y `entro`
    /// (o no: estaba atascado). Devuelve si ya es un atasco de los que no
    /// se pasan (diez segundos sin que entre ninguno): se corta el video.
    pub fn escrito(&self, entro: bool, ahora: i64) -> bool {
        let mut e = self.estado();
        if entro {
            e.atasco_desde = None;
            return false;
        }
        let desde = *e.atasco_desde.get_or_insert(ahora);
        ahora - desde >= ATASCO
    }

    /// **Donde acaba el video** a la hora `ahora`: donde acaba el sonido
    /// (o lo corrido sin pausas si va mudo). Ahi se cierra el ultimo
    /// fotograma, para que las dos pistas duren lo mismo.
    pub fn final_del_video(&self, ahora: i64) -> i64 {
        let e = self.estado();
        e.reloj.final_(ahora).max(e.duracion)
    }

    /// **La pantalla se fue** (un monitor que se desconecta, el Mac que se
    /// duerme): si toca intentar abrirla otra vez a la hora `ahora` (cada
    /// tres segundos, el primero enseguida). Mientras, se repite la ultima
    /// imagen y el sonido no se corta.
    pub fn reabrir_toca(&self, ahora: i64) -> bool {
        let mut e = self.estado();
        match e.proximo_reintento {
            Some(p) if ahora < p => false,
            _ => {
                e.proximo_reintento = Some(ahora + REABRIR_CADA);
                true
            }
        }
    }

    /// La pantalla volvio: el proximo fallo se intenta enseguida.
    pub fn reabierta(&self) {
        self.estado().proximo_reintento = None;
    }

    /// **Una pantalla que se abrio y se volvio a caer**: si se le da otra
    /// vuelta (tres cada diez minutos) o ya no va a mejorar y se corta el
    /// video guardando lo que habia.
    pub fn otra_vuelta(&self, ahora: i64) -> bool {
        let mut e = self.estado();
        e.vueltas.retain(|&t| ahora - t < VENTANA_DE_VUELTAS);
        if e.vueltas.len() >= VUELTAS {
            return false;
        }
        e.vueltas.push(ahora);
        true
    }
}

/// **Que pantalla grabar al volver a abrirla**: la de antes si sigue
/// conectada, o la principal (`reapertura::elegir_monitor`: en una clase
/// es mejor seguir grabando otra pantalla que una imagen congelada).
#[uniffi::export]
pub fn pantalla_del_video(antes: u32, disponibles: Vec<u32>, principal: u32) -> u32 {
    if antes != 0 && disponibles.contains(&antes) {
        antes
    } else {
        principal
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const S: i64 = UNIDADES_POR_SEGUNDO;
    const MS: i64 = S / 1000;

    #[test]
    fn cada_perfil_tiene_su_tamano_y_su_ritmo() {
        assert_eq!(PerfilVideo::Ligero.medidas(1920, 1080), (1280, 720));
        assert_eq!(PerfilVideo::Normal.medidas(2560, 1440), (1920, 1080));
        assert_eq!(PerfilVideo::Nitido.medidas(2560, 1440), (2560, 1440));
        // Una Retina 16:10: manda el alto, y nunca pasa de 4K.
        assert_eq!(PerfilVideo::Normal.medidas(2880, 1800), (1728, 1080));
        assert_eq!(PerfilVideo::Nitido.medidas(6016, 3384), (3840, 2160));
        // Caso negativo: ningun perfil agranda una pantalla pequena.
        assert_eq!(PerfilVideo::Normal.medidas(1366, 768), (1366, 768));
        let fps: Vec<u32> = PerfilVideo::TODOS.iter().map(|p| p.ajustes().por_segundo).collect();
        assert_eq!(fps, vec![10, 15, 30]);
    }

    #[test]
    fn una_hora_de_normal_cabe_en_el_presupuesto() {
        let a = PerfilVideo::Normal.ajustes();
        let (media, _) = bits_por_segundo(&a, 1920, 1080);
        let mb = megas_por_hora(media, true);
        assert!((150..=300).contains(&mb), "{mb} MB por hora");
        let l = megas_por_hora(PerfilVideo::Ligero.ajustes().media, true);
        let n = megas_por_hora(PerfilVideo::Nitido.ajustes().media, true);
        assert!(l < mb && mb < n);
        // Caso negativo: una ventanita no se queda sin bits, ni una 8K se
        // dispara.
        assert_eq!(bits_por_segundo(&a, 320, 240), (a.media / 2, a.pico / 2));
        let x = PerfilVideo::Nitido.ajustes();
        assert_eq!(bits_por_segundo(&x, 7680, 4320).0, x.media * 4);
        assert_eq!(grupo_de_imagenes(&a), 30);
    }

    #[test]
    fn las_claves_van_y_vuelven_y_pulsar_recorre_los_tres() {
        for p in PerfilVideo::TODOS {
            assert_eq!(perfil_de_clave(clave_del_perfil(p)), p);
        }
        assert_eq!(perfil_de_clave("ultra".into()), PerfilVideo::Normal);
        let mut p = PerfilVideo::Ligero;
        for esperado in PerfilVideo::TODOS {
            assert_eq!(p, esperado);
            p = perfil_del_video_siguiente(p);
        }
        assert_eq!(p, PerfilVideo::Ligero);
        assert_eq!(PerfilVideo::default(), PerfilVideo::Normal);
    }

    #[test]
    fn los_textos_son_los_de_windows() {
        let ftl = include_str!("../../pixpin-store/i18n/es-ES/main.ftl");
        let clave = |k: &str| {
            ftl.lines()
                .find_map(|l| l.strip_prefix(&format!("{k} = ")))
                .unwrap_or_else(|| panic!("falta {k}"))
                .to_string()
        };
        for (p, k) in [
            (PerfilVideo::Ligero, "ligero"),
            (PerfilVideo::Normal, "normal"),
            (PerfilVideo::Nitido, "nitido"),
        ] {
            assert_eq!(rotulo_de_calidad(p), clave(&format!("clase-calidad-{k}")));
            assert_eq!(pista_de_calidad(p), clave(&format!("clase-calidad-{k}-pista")));
        }
        for (c, k) in [
            (CodificadorVideo::Hardware, "grafica"),
            (CodificadorVideo::Software, "software"),
            (CodificadorVideo::Cpu, "cpu"),
        ] {
            let esperado = clave("clase-video-con")
                .replace("{ $codificador }", &clave(&format!("clase-codificador-{k}")));
            assert_eq!(video_con(c), esperado);
            assert_eq!(c.clave(), k);
        }
    }

    #[test]
    fn la_cadena_empieza_por_la_grafica_y_acaba_en_el_modo_sencillo() {
        let v = caminos_del_video(PerfilVideo::Nitido, 3840, 2160, true);
        let pasos: Vec<(CodificadorVideo, u32, u32)> =
            v.iter().map(|c| (c.codificador, c.ancho, c.alto)).collect();
        assert_eq!(
            pasos,
            vec![
                (CodificadorVideo::Hardware, 3840, 2160),
                (CodificadorVideo::Hardware, 1920, 1080),
                (CodificadorVideo::Hardware, 1280, 720),
                (CodificadorVideo::Software, 3840, 2160),
                (CodificadorVideo::Cpu, 1280, 720),
            ]
        );
        assert_eq!(v[0].por_segundo, 30);
        assert_eq!(v[4].por_segundo, 10);
        assert!(v.iter().all(|c| c.bits_pico > c.bits_media && c.bits_audio == 96_000));
        // Caso negativo: sin grafica no se intenta, y en 720p no hay nada
        // mas pequeno que probar.
        let v = caminos_del_video(PerfilVideo::Ligero, 1280, 720, false);
        assert_eq!(v[0].codificador, CodificadorVideo::Software);
        assert_eq!(v.len(), 2);
        // Una pantalla vertical baja en vertical.
        assert_eq!(reintento_mas_pequeno((1440, 2560)), Some((1080, 1920)));
        assert_eq!(reintento_mas_pequeno((1280, 720)), None);
    }

    #[test]
    fn la_pantalla_quieta_da_un_fotograma_por_segundo() {
        let r = RitmoDelVideo::nuevo(0, 15, 0);
        // El primero, en cuanto llega, en el cero.
        let p = r.paso(0, Some(0));
        assert_eq!((p.accion, p.instante), (AccionDelVideo::Nuevo, 0));
        // Medio segundo sin nada: esperar; al segundo, el repetido.
        assert_eq!(r.paso(500 * MS, None).accion, AccionDelVideo::Esperar);
        let p = r.paso(S, None);
        assert_eq!((p.accion, p.instante), (AccionDelVideo::Repetir, S));
        // Uno nuevo antes del minimo espera; pasado, entra.
        assert_eq!(r.paso(S + 30 * MS, Some(S + 30 * MS)).accion, AccionDelVideo::Esperar);
        assert_eq!(r.paso(S + 70 * MS, Some(S + 30 * MS)).accion, AccionDelVideo::Nuevo);
        // La espera llega justo a lo siguiente, sin pasar de 250 ms.
        assert_eq!(r.espera(S + 70 * MS, false), S / 4);
        assert_eq!(r.espera(S + 80 * MS, true), minimo_entre(15) - 10 * MS);
    }

    #[test]
    fn sin_imagen_en_cinco_segundos_se_empieza_en_negro() {
        let r = RitmoDelVideo::nuevo(0, 10, 0);
        assert_eq!(r.paso(4 * S, None).accion, AccionDelVideo::Esperar);
        let p = r.paso(ESPERA_PRIMERO, None);
        assert_eq!((p.accion, p.instante), (AccionDelVideo::Repetir, 0));
    }

    #[test]
    fn con_sonido_manda_el_sonido() {
        let r = RitmoDelVideo::nuevo(48_000, 15, 0);
        assert_eq!(r.paso(0, Some(0)).instante, 0);
        // Llega medio segundo de sonido a los 600 ms de pared.
        r.sonido(0, 24_000, 600 * MS);
        // Un fotograma de 10 ms despues va 10 ms despues del sonido.
        let p = r.paso(610 * MS, Some(610 * MS));
        assert_eq!((p.accion, p.instante), (AccionDelVideo::Nuevo, S / 2 + 10 * MS));
        // Caso negativo: si el sonido se para, el video no se escapa mas de
        // un cuarto de segundo: llega a su tope y ahi se queda.
        let p = r.paso(3 * S, Some(3 * S));
        assert_eq!(p.instante, S / 2 + ADELANTO_MAXIMO);
        assert_eq!(r.paso(4 * S, Some(4 * S)).accion, AccionDelVideo::Esperar);
        assert_eq!(r.final_del_video(3 * S), S / 2);
    }

    #[test]
    fn la_pista_de_sonido_no_se_pisa() {
        let r = RitmoDelVideo::nuevo(48_000, 15, 0);
        assert_eq!(r.sonido(0, 960, 0), Some(TramoDeSonido { salto: 0, desde: 0 }));
        assert_eq!(r.sonido(860, 960, 0), Some(TramoDeSonido { salto: 100, desde: 960 }));
        // Un hueco se deja como esta.
        assert_eq!(r.sonido(5_000, 960, 0), Some(TramoDeSonido { salto: 0, desde: 5_000 }));
        // Caso negativo: un bloque repetido entero, o vacio, no entra.
        assert_eq!(r.sonido(5_000, 960, 0), None);
        assert_eq!(r.sonido(9_000, 0, 0), None);
    }

    #[test]
    fn en_pausa_no_se_escribe_nada() {
        let r = RitmoDelVideo::nuevo(0, 15, 0);
        r.paso(0, Some(0));
        r.pausar(true, S);
        assert!(r.en_pausa());
        assert_eq!(r.paso(10 * S, Some(10 * S)).accion, AccionDelVideo::Esperar);
        assert_eq!(r.espera(10 * S, true), S / 4);
        r.pausar(false, 11 * S);
        // Lo pausado no cuenta: el siguiente va a los dos segundos de video.
        let p = r.paso(12 * S, Some(12 * S));
        assert_eq!((p.accion, p.instante), (AccionDelVideo::Nuevo, 2 * S));
        assert_eq!(r.final_del_video(12 * S), 2 * S);
    }

    #[test]
    fn el_fotograma_pegado_al_latido_ocupa_su_sitio() {
        let r = RitmoDelVideo::nuevo(0, 15, 0);
        assert!(r.sustituye_al_repetido(true, S, S + 3 * MS));
        // Caso negativo: lo de verdad no se toca, ni lo que llega con su hueco.
        assert!(!r.sustituye_al_repetido(false, S, S + 3 * MS));
        assert!(!r.sustituye_al_repetido(true, S, S + minimo_entre(15)));
        assert!(!r.sustituye_al_repetido(true, S, S - MS));
    }

    #[test]
    fn un_atasco_tiene_que_durar_y_una_pausa_lo_olvida() {
        let r = RitmoDelVideo::nuevo(0, 15, 0);
        assert!(!r.escrito(false, 0));
        assert!(!r.escrito(false, 9 * S));
        assert!(r.escrito(false, 10 * S));
        // Caso negativo: con que entre uno, o tras una pausa, se cuenta de cero.
        assert!(!r.escrito(true, 11 * S));
        assert!(!r.escrito(false, 12 * S));
        r.pausar(true, 13 * S);
        r.pausar(false, 600 * S);
        assert!(!r.escrito(false, 601 * S));
    }

    #[test]
    fn la_pantalla_que_se_va_se_reabre_con_tope() {
        let r = RitmoDelVideo::nuevo(0, 15, 0);
        assert!(r.reabrir_toca(0));
        assert!(!r.reabrir_toca(2 * S), "cada tres segundos");
        assert!(r.reabrir_toca(3 * S));
        r.reabierta();
        assert!(r.reabrir_toca(3 * S + MS), "volvio: el siguiente fallo, enseguida");
        // Tres vueltas cada diez minutos; la cuarta, no.
        assert!(r.otra_vuelta(0) && r.otra_vuelta(S) && r.otra_vuelta(2 * S));
        assert!(!r.otra_vuelta(3 * S));
        assert!(r.otra_vuelta(VENTANA_DE_VUELTAS + MS));
        assert_eq!(pantalla_del_video(7, vec![1, 7], 1), 7);
        // Caso negativo: la que ya no esta pasa a la principal.
        assert_eq!(pantalla_del_video(7, vec![1], 1), 1);
        assert_eq!(pantalla_del_video(0, vec![0, 1], 1), 1);
    }
}
