//! **El asistente en la conversacion** (E1): lo que en Windows hacen
//! `ventana_chat/asistente.rs` y `asistente/pensado.rs`, sin la ventana.
//!
//! - En el chat fijo «Asistente» todo lo que se escribe es para el; en
//!   cualquier otro, `@asistente` al principio o en medio
//!   ([`super::chat::pregunta_de`]).
//! - **Su respuesta es una nota normal del cuaderno**, con `aparato = "IA"`
//!   (la chapa dice `#48·IA`): se guarda, se busca y **viaja** como cualquier
//!   otra, sin un formato nuevo que Windows o el movil no sepan leer. Nace
//!   en cuanto se envia la pregunta ([`Nucleo::ia_al_enviar`], con «💭
//!   Pensando…») y se va reescribiendo en el disco cada [`GUARDAR_CADA`]
//!   mientras el modelo escribe ([`ia_turno_en_chat`]); la burbuja de Swift
//!   lo ve al momento por un [`OyenteChatIa`].
//! - En otro chat la respuesta cuelga de la pregunta (`respondeA`).
//! - Lo dibujado va a un lienzo del chat ([`super::al_lienzo`]) y la
//!   respuesta lo apunta en `lienzoIa` (su «Abrir lienzo»); lo citado de los
//!   documentos, en `fuentesIa`.
//! - **Lo que penso**, cuanto, la peticion mejorada y la hoja de trabajo van
//!   en `pins/draw/anot-<codigo>.asistente.json` con el codigo unico de la
//!   nota (`pixpin_ia::hoja_trabajo::Registro`): viajan con su nota y se
//!   borran con ella. La hoja del chat es la de la ultima respuesta que la
//!   tenga ([`hoja_del_chat`]).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pixpin_ia::agente::Evento;
use pixpin_ia::contexto::Fuente;
use pixpin_ia::hoja_trabajo::{HojaDeTrabajo, Registro};
use pixpin_ia::motor::Seguir;
use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Cuaderno, Mensaje as Nota};
use pixpin_sincro::anotado as an;
use pixpin_sincro::disco::Disco;

use super::chat::{self as ia_chat, Contestacion, EnVivo};
use super::turno::{AjustesIa, AppIa, EjecutorDelMac, EnChat, Entorno, Extra, FotoIa, PeticionIa, RangoPaginas, VENTANA_NUBE};
use super::{CorteIa, MensajeIa};
use crate::chats::validar_chat;
use crate::{ErrorNucleo, FichaChat, MensajeChat, Nucleo};

/// Cada cuanto se escribe en el disco lo que va contestando (en la burbuja
/// se ve al momento). Un turno de minuto y medio no son cien escrituras.
pub const GUARDAR_CADA: Duration = Duration::from_secs(2);

/// Cada cuanto, como mucho, cruza la burbuja a Swift mientras crece.
const CRUZAR_CADA: Duration = Duration::from_millis(super::CADA_TANDA_MS);

/// El lado mayor de la foto que se le ensena (Windows, `foto::LADO`).
pub const LADO_FOTO: u32 = 768;

/// **Lo que nace al enviar**: la respuesta (aun sin texto) y la pregunta.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct InicioIa {
    pub respuesta: MensajeChat,
    /// El id del mensaje que se le pregunta.
    pub pregunta_id: String,
}

/// **La burbuja en vivo**, como se ve ahora.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct VivoIa {
    /// El id de la nota de la respuesta.
    pub nota: String,
    /// El texto tal como se ve (con «✏️ dibujando…» o el cursor).
    pub texto: String,
    pub pensado: PensadoIa,
    /// La pregunta de «¿seguro?» que espera el «Sí» o el «No».
    pub pregunta: Option<String>,
    pub acabado: bool,
}

/// **Lo que se pinta encima de una respuesta** (`pensado::Seccion` de
/// Windows): lo que penso, la peticion mejorada y si se renovo el contexto.
#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct PensadoIa {
    pub pensamiento: String,
    /// Cuanto penso; `None` mientras piensa (o si no penso).
    pub segundos: Option<f32>,
    /// Pensando ahora mismo (la seccion va abierta y crece).
    pub pensando: bool,
    /// La peticion mejorada («📝 Entiendo: …»), vacia si no hay.
    pub entiendo: String,
    /// Lo que hara, en una frase.
    pub hare: String,
    /// Este turno empezo con el contexto renovado.
    pub reinicio: bool,
}

impl PensadoIa {
    /// Si hay algo que ensenar.
    pub fn algo(&self) -> bool {
        !self.pensamiento.trim().is_empty() || !self.entiendo.is_empty() || self.reinicio
    }

    fn de_vivo(v: &EnVivo) -> PensadoIa {
        PensadoIa {
            pensamiento: v.pensamiento.clone(),
            segundos: v.segundos,
            pensando: v.pensando,
            entiendo: v.plan.as_ref().map(|p| p.entiendo.clone()).unwrap_or_default(),
            hare: v.plan.as_ref().map(|p| p.hare.clone()).unwrap_or_default(),
            reinicio: v.reiniciado,
        }
    }

    fn de_registro(r: &Registro) -> PensadoIa {
        PensadoIa {
            pensamiento: r.pensamiento.clone(),
            segundos: r.segundos,
            pensando: false,
            entiendo: r.plan.as_ref().map(|p| p.entiendo.clone()).unwrap_or_default(),
            hare: r.plan.as_ref().map(|p| p.hare.clone()).unwrap_or_default(),
            reinicio: r.reinicio,
        }
    }
}

/// **Como acaba el turno en el chat.**
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FinChatIa {
    /// La respuesta como quedo en el cuaderno.
    pub respuesta: MensajeChat,
    /// El lienzo que se pidio abrir al terminar («…y ábrelo»), su
    /// referencia.
    pub abrir_lienzo: Option<String>,
}

/// Una opcion de la lista de `@` mientras se escribe.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct OpcionMencionIa {
    /// Lo que se ve en la lista («🎨 Plano casa»).
    pub rotulo: String,
    /// Lo que se escribe al elegirla (`@lienzo:Plano_casa`), sin espacio.
    pub mencion: String,
}

/// **Quien ve la burbuja en vivo**: lo implementa Swift. Se le llama desde
/// el hilo del turno, como mucho cada ~60 ms y siempre que cambia algo que
/// no es texto (una herramienta, una pregunta, el final).
#[uniffi::export(foreign)]
pub trait OyenteChatIa: Send + Sync {
    fn vivo(&self, v: VivoIa);
}

// ---------------------------------------------------------------------------
// Donde va lo pensado

/// Donde va el registro de una respuesta (`pensado::ruta_del_registro`).
pub fn ruta_del_registro(raiz: &Path, ficha: &str, codigo_unico: &str) -> PathBuf {
    let rel = an::rel(&format!("{}{codigo_unico}", an::PREFIJO), an::ASISTENTE);
    let chat = pixpin_proyecto::vista::chat_de_ficha(raiz, ficha).unwrap_or_else(|| ficha.to_string());
    pixpin_proyecto::vista::DiscoPc::nuevo(raiz).ruta(&chat, &rel)
}

/// El registro de una nota (si no hay, o esta roto, nada).
fn registro_de(raiz: &Path, ficha: &str, nota: &Nota) -> Option<Registro> {
    Registro::leer(&ruta_del_registro(raiz, ficha, &nota.codigo_unico())).ok().flatten()
}

/// Cuantas respuestas hacia atras se mira para encontrar la hoja.
const BUSCAR_HOJA_EN: usize = 30;

/// **La hoja del chat**: la del registro de la ultima respuesta que lo
/// tenga. Si esta rota, la anterior; si no hay ninguna, una nueva.
pub fn hoja_del_chat(raiz: &Path, ficha: &str, notas: &[Nota]) -> HojaDeTrabajo {
    notas
        .iter()
        .rev()
        .filter(|n| ia_chat::es_del_asistente(n))
        .take(BUSCAR_HOJA_EN)
        .find_map(|n| registro_de(raiz, ficha, n))
        .map(|r| r.hoja)
        .unwrap_or_default()
}

/// **Donde busca `buscar_historial`**: las notas del chat y lo pensado en
/// cada turno de la hoja.
pub fn fuentes(notas: &[Nota], hoja: &HojaDeTrabajo) -> Vec<Fuente> {
    let mut v: Vec<Fuente> = ia_chat::conversacion_con_ids(notas)
        .into_iter()
        .map(|(_, n, del_usuario, texto)| Fuente {
            etiqueta: format!("#{n} {}", if del_usuario { "usuario" } else { "asistente" }),
            texto,
        })
        .collect();
    for t in &hoja.turnos {
        if !t.pensado.is_empty() {
            v.push(Fuente {
                etiqueta: format!("T{} pensado", t.n),
                texto: format!("{} — {}", t.peticion, t.pensado),
            });
        }
    }
    v
}

/// La conversacion para el turno, desde el ultimo reinicio de la hoja:
/// los ids (para apuntar el siguiente reinicio) y los mensajes.
pub fn conversacion_desde(hoja: &HojaDeTrabajo, anteriores: &[Nota]) -> (Vec<String>, Vec<MensajeIa>) {
    let mut conv = ia_chat::conversacion_con_ids(anteriores);
    if let Some(desde) = hoja.reinicio_desde.as_deref()
        && let Some(i) = conv.iter().position(|(id, _, _, _)| id == desde)
    {
        conv.drain(..i);
    }
    conv.into_iter()
        .map(|(id, _, del_usuario, texto)| (id, MensajeIa { del_usuario, texto }))
        .unzip()
}

// ---------------------------------------------------------------------------
// La foto

/// Las medidas reducidas para que el lado mayor no pase de `lado` (nunca se
/// amplia).
pub fn medidas_reducidas(ancho: u32, alto: u32, lado: u32) -> (u32, u32) {
    let mayor = ancho.max(alto);
    if mayor <= lado || mayor == 0 {
        return (ancho, alto);
    }
    let f = lado as f64 / mayor as f64;
    (((ancho as f64 * f).round() as u32).max(1), ((alto as f64 * f).round() as u32).max(1))
}

/// **La foto que se le ensena, reducida** a ~768 px y en JPEG (Windows,
/// `foto::preparar_fichero`). `None` si no se puede abrir: mejor contestar
/// sin verla que no contestar.
pub fn foto_de(ruta: &Path) -> Option<FotoIa> {
    let img = pixpin_codec::imagen::cargar(ruta).ok()?;
    let (a, h) = medidas_reducidas(img.ancho, img.alto, LADO_FOTO);
    let img = if (a, h) == (img.ancho, img.alto) {
        img
    } else {
        pixpin_codec::imagen::redimensionar(img, a, h).ok()?
    };
    let bytes = pixpin_codec::imagen::codificar_jpg(&img, 85).ok()?;
    Some(FotoIa {
        bytes,
        mime: "image/jpeg".into(),
    })
}

// ---------------------------------------------------------------------------
// La burbuja en vivo

/// La respuesta que se escribe: lo que se ve, cuando se dijo a Swift y
/// cuando se guardo.
struct Burbuja {
    nucleo: Arc<Nucleo>,
    chat: String,
    nota: String,
    oyente: Arc<dyn OyenteChatIa>,
    estado: Mutex<Estado>,
    cruzar_cada: Duration,
}

struct Estado {
    vivo: EnVivo,
    cruzado: Instant,
    guardado: Instant,
}

impl Burbuja {
    fn tomar(&self) -> std::sync::MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Un evento del turno: a la burbuja y, si toca, a Swift y al disco.
    fn aplicar(&self, e: &Evento) {
        let mut st = self.tomar();
        st.vivo.aplicar(e);
        let ya = !matches!(e, Evento::Texto(_) | Evento::Pensando(_));
        self.contar(&mut st, ya);
    }

    /// Lo que borra o envia pregunta en la burbuja.
    fn preguntar(&self, pregunta: &str) {
        let mut st = self.tomar();
        st.vivo.preguntar(pregunta);
        // Que se vea ya en el disco tambien.
        st.guardado = Instant::now() - GUARDAR_CADA;
        self.contar(&mut st, true);
    }

    /// La pregunta contestada queda escrita en la burbuja.
    fn contestar(&self, c: Contestacion) {
        let mut st = self.tomar();
        if st.vivo.contestar(c) {
            st.guardado = Instant::now() - GUARDAR_CADA;
            self.contar(&mut st, true);
        }
    }

    fn vivo_ia(&self, v: &EnVivo) -> VivoIa {
        VivoIa {
            nota: self.nota.clone(),
            texto: v.mostrar(),
            pensado: PensadoIa::de_vivo(v),
            pregunta: v.pendiente.clone(),
            acabado: v.acabado,
        }
    }

    /// A Swift (cada ~60 ms, o `ya`) y al disco (cada [`GUARDAR_CADA`]).
    fn contar(&self, st: &mut Estado, ya: bool) {
        if ya || st.cruzado.elapsed() >= self.cruzar_cada {
            st.cruzado = Instant::now();
            self.oyente.vivo(self.vivo_ia(&st.vivo));
        }
        if st.guardado.elapsed() >= GUARDAR_CADA {
            st.guardado = Instant::now();
            let texto = st.vivo.mostrar();
            let _ = self.nucleo.cambiar_nota(&self.chat, &self.nota, |m| {
                if m.texto == texto {
                    return false;
                }
                m.texto = texto.clone();
                true
            });
        }
    }
}

/// **Las manos de la app, con la pregunta en la burbuja**: antes de lo que
/// borra o envia, la burbuja ensena la pregunta (y la deja escrita al
/// contestarla); lo demas, a las manos de Swift tal cual.
struct ManosEnLaBurbuja {
    dentro: Arc<dyn AppIa>,
    burbuja: Arc<Burbuja>,
}

impl AppIa for ManosEnLaBurbuja {
    fn confirmar(&self, pregunta: String) -> bool {
        self.burbuja.preguntar(&pregunta);
        let si = self.dentro.confirmar(pregunta);
        self.burbuja.contestar(if si { Contestacion::Si } else { Contestacion::No });
        si
    }

    fn pedido(&self, json: String) -> Option<String> {
        self.dentro.pedido(json)
    }

    fn leer_texto(&self, ruta: String, paginas: Vec<RangoPaginas>) -> Result<String, ErrorNucleo> {
        self.dentro.leer_texto(ruta, paginas)
    }

    fn pintar_vista(&self, vista: super::ojos::VistaParaPintar) -> Option<Vec<u8>> {
        self.dentro.pintar_vista(vista)
    }

    fn paginas_de_pdf(&self, ruta: String) -> Vec<super::turno::MedidaPagina> {
        self.dentro.paginas_de_pdf(ruta)
    }

    fn pintar_pagina(&self, ruta: String, pagina: u32, ancho: u32) -> Option<Vec<u8>> {
        self.dentro.pintar_pagina(ruta, pagina, ancho)
    }

    fn palabras_de_imagen(&self, imagen: Vec<u8>) -> Vec<super::turno::PalabraLeida> {
        self.dentro.palabras_de_imagen(imagen)
    }

    fn exportar(&self, ruta: String, formato: String, carpeta: String) -> Result<Vec<String>, ErrorNucleo> {
        self.dentro.exportar(ruta, formato, carpeta)
    }
}

// ---------------------------------------------------------------------------
// El cuaderno

impl Nucleo {
    /// Cambia una nota del cuaderno de `chat` (con su cerrojo) y olvida la
    /// cache: la siguiente pagina la lee como quedo.
    pub(crate) fn cambiar_nota(
        &self,
        chat: &str,
        id: &str,
        cambio: impl FnOnce(&mut Nota) -> bool,
    ) -> Result<Option<Nota>, ErrorNucleo> {
        let carpeta = almacen::carpeta(self.ruta(), chat);
        let fichero = carpeta.join("guardados.jsonl");
        let mut cache = self.cache_tomada();
        let hecho = cuaderno::cambiar(&carpeta, id, cambio)?;
        cache.quitar(&fichero);
        Ok(hecho)
    }

    /// Los mensajes de un chat (vacio si aun no tiene cuaderno).
    pub(crate) fn notas_de(&self, chat: &str) -> Vec<Nota> {
        Cuaderno::leer_de(&almacen::carpeta(self.ruta(), chat))
            .map(|c| c.mensajes)
            .unwrap_or_default()
    }

    /// Si `chat` es el chat fijo del asistente.
    fn es_chat_del_asistente(&self, chat: &str) -> bool {
        Indice::leer_si_esta(self.ruta())
            .ok()
            .and_then(|i| i.buscar(chat).map(|f| f.es_asistente()))
            .unwrap_or(false)
    }

    /// El chat sube en la lista con la respuesta (como al escribir).
    fn subir_en_la_lista(&self, chat: &str, resumen: &str) {
        let Ok(mut indice) = Indice::leer_si_esta(self.ruta()) else { return };
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == chat) {
            f.tocado = crate::chats::hora_unica();
            f.resumen = resumen.chars().take(120).collect();
            let _ = indice.guardar(self.ruta());
        }
    }

    /// **El texto entero de lo leido, al chat**: una nota con el texto (se
    /// lee y se copia ahi mismo) y el mismo texto como `.txt` adjunto
    /// (Windows, `dejar_texto_leido`).
    fn dejar_texto_leido(&self, chat: &str, de: &super::turno::AdjuntoIa, texto: &str) -> Result<(), ErrorNucleo> {
        let base = Path::new(&de.nombre)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "texto".into());
        let ia = |m: &mut Nota| m.aparato = Some(ia_chat::APARATO.into());
        self.escribir_nota_con(chat.to_string(), format!("📄 Texto de «{}»:\n\n{}", de.nombre, texto), None, ia)?;
        let carpeta = std::env::temp_dir().join(format!("pixpin-leido-{}-{}", std::process::id(), crate::ahora_ms()));
        std::fs::create_dir_all(&carpeta)?;
        let fichero = carpeta.join(format!("{base} (texto).txt"));
        std::fs::write(&fichero, texto)?;
        let r = self.adjuntar_con(chat.to_string(), fichero.display().to_string(), None, ia);
        let _ = std::fs::remove_dir_all(&carpeta);
        r.map(|_| ())
    }
}

#[uniffi::export]
impl Nucleo {
    /// **El chat «Asistente»**, creandolo si no esta (como «Mensajes
    /// guardados»: fijo, detras de el en la lista).
    pub fn chat_asistente(&self) -> Result<FichaChat, ErrorNucleo> {
        let aparato = self.identidad()?.yo.codigo();
        let f = almacen::asegurar_asistente(self.ruta(), crate::ahora_ms(), &aparato)?;
        Ok(FichaChat::from(&f))
    }

    /// **Despues de escribir un mensaje** (`mensaje`, una nota o una foto):
    /// si es para el asistente (el chat «Asistente», o `@asistente` en otro)
    /// nace su respuesta, con «💭 Pensando…», y se devuelve para lanzar el
    /// turno ([`ia_turno_en_chat`]). `None` si no era para el.
    pub fn ia_al_enviar(&self, chat: String, mensaje: String) -> Result<Option<InicioIa>, ErrorNucleo> {
        validar_chat(&chat)?;
        let notas = self.notas_de(&chat);
        let Some(m) = notas.iter().rev().find(|m| m.id == mensaje) else {
            return Ok(None);
        };
        let del_asistente = self.es_chat_del_asistente(&chat);
        if ia_chat::pregunta_de(m, del_asistente).is_none() {
            return Ok(None);
        }
        let vivo = EnVivo {
            estado: Some(ia_chat::ESTADO_INICIAL.into()),
            ..Default::default()
        };
        // En otro chat la respuesta cuelga de la pregunta, para que se sepa a
        // que contesta entre los mensajes de los demas.
        let responde_a = (!del_asistente).then(|| m.id.clone());
        let nota = self.escribir_nota_con(chat, vivo.mostrar(), responde_a, |n| {
            n.aparato = Some(ia_chat::APARATO.into());
        })?;
        Ok(Some(InicioIa {
            respuesta: MensajeChat::from(nota),
            pregunta_id: mensaje,
        }))
    }

    /// **«Conversación nueva»** en el chat `chat`: una nota del asistente que
    /// corta el contexto que se le manda (lo de antes sigue a la vista y
    /// viaja como siempre; Windows, 17-oct).
    pub fn ia_conversacion_nueva(&self, chat: String) -> Result<MensajeChat, ErrorNucleo> {
        validar_chat(&chat)?;
        let nota = self.escribir_nota_con(chat, ia_chat::TEXTO_NUEVA_CONVERSACION.into(), None, |n| {
            n.aparato = Some(ia_chat::APARATO.into());
            n.resto.insert(ia_chat::NUEVA_CONVERSACION.into(), serde_json::Value::Bool(true));
        })?;
        Ok(MensajeChat::from(nota))
    }

    /// **Lo que se pinta encima de una respuesta guardada**: lo que penso,
    /// la peticion mejorada y si se renovo el contexto. `None` si no es del
    /// asistente o no hay nada que ensenar.
    pub fn ia_pensado(&self, chat: String, nota: String) -> Option<PensadoIa> {
        validar_chat(&chat).ok()?;
        let n = self.notas_de(&chat).into_iter().rev().find(|m| m.id == nota)?;
        if !ia_chat::es_del_asistente(&n) {
            return None;
        }
        let p = PensadoIa::de_registro(&registro_de(self.ruta(), &chat, &n)?);
        p.algo().then_some(p)
    }

    /// **Escribe la respuesta acabada** que no salio de [`ia_turno_en_chat`]
    /// (Apple Intelligence, o un aviso): el texto y, si dibujo, `lienzo` (un
    /// `.excalidraw`) al lienzo del chat, apuntado para su «Abrir lienzo».
    pub fn ia_responder(&self, chat: String, nota: String, texto: String, lienzo: Option<String>) -> Result<MensajeChat, ErrorNucleo> {
        validar_chat(&chat)?;
        let mut referencia = None;
        let mut texto = texto;
        if let Some(doc) = lienzo {
            match super::al_lienzo::en_el_chat(self, &chat, None, &super::al_lienzo::Trabajo::Dibujar(doc), None) {
                Ok((_, r)) => referencia = r,
                Err(e) => texto.push_str(&format!("\n⚠️ {e}")),
            }
        }
        let hecho = self.cambiar_nota(&chat, &nota, |m| {
            m.texto = texto.clone();
            if let Some(r) = &referencia {
                m.resto.insert(ia_chat::LIENZO_DE_LA_RESPUESTA.into(), serde_json::Value::String(r.clone()));
            }
            true
        })?;
        self.subir_en_la_lista(&chat, &texto);
        hecho.map(MensajeChat::from).ok_or(ErrorNucleo::NoExiste { que: nota })
    }

    /// **La lista de `@`** para lo escrito tras el `@`: el asistente (fuera
    /// de su chat) y los lienzos y hojas de la conversacion, del mas nuevo
    /// al mas viejo.
    pub fn ia_opciones_mencion(&self, chat: String, escrito: String) -> Vec<OpcionMencionIa> {
        if validar_chat(&chat).is_err() {
            return Vec::new();
        }
        let notas = self.notas_de(&chat);
        ia_chat::opciones_de_mencion(&notas, &escrito, self.es_chat_del_asistente(&chat))
            .into_iter()
            .map(|o| OpcionMencionIa {
                rotulo: o.rotulo,
                mencion: o.mencion,
            })
            .collect()
    }
}

/// **Si se esta escribiendo una mencion**: lo escrito tras el `@` hasta la
/// rayita (`antes`: el texto de la caja hasta la rayita). Solo si el `@`
/// empieza palabra y no hay un espacio entre el y la rayita.
#[uniffi::export]
pub fn ia_consulta_de_mencion(antes: String) -> Option<String> {
    ia_chat::consulta_de_mencion(&antes, antes.len()).map(|(_, escrito)| escrito)
}

/// Una respuesta del asistente como se pinta en su burbuja: las listas
/// como listas y las negritas aparte (en unidades UTF-16, las de
/// `NSString`). Lo guardado no cambia.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FormatoRespuesta {
    pub texto: String,
    /// `(inicio, largo)` de cada tramo en negrita, en UTF-16.
    pub negritas: Vec<TramoNegrita>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct TramoNegrita {
    pub inicio: u32,
    pub largo: u32,
}

/// El texto de una respuesta para su burbuja (Windows, 17-oct).
#[uniffi::export]
pub fn ia_formato_de_respuesta(texto: String) -> FormatoRespuesta {
    let (texto, n) = ia_chat::formato_de_respuesta(&texto);
    FormatoRespuesta {
        texto,
        negritas: n.into_iter().map(|(inicio, largo)| TramoNegrita { inicio, largo }).collect(),
    }
}

/// Si un texto que se va a enviar lleva a `@asistente` (para precargar el
/// modelo o avisar mientras se escribe).
#[uniffi::export]
pub fn ia_menciona_al_asistente(texto: String) -> bool {
    ia_chat::mencion(&texto).is_some()
}

// ---------------------------------------------------------------------------
// El turno

/// **El turno del asistente en un chat**, en el hilo de quien lo llama (Swift
/// lo lanza en uno de GCD). Lee la conversacion, elige el motor de los
/// ajustes, contesta en la nota de `inicio` (en vivo a `oyente`, al disco
/// cada [`GUARDAR_CADA`]) y al acabar guarda lo pensado, apunta el lienzo y
/// las fuentes, y deja en el chat el texto entero de lo leido. `corte` lo
/// para; `app` hace las acciones (y pregunta «¿seguro?»). `desfase_ms`: el
/// huso de quien pregunta («mañana a las 10»).
#[uniffi::export]
#[allow(clippy::too_many_arguments)]
pub fn ia_turno_en_chat(
    ajustes: AjustesIa,
    nucleo: Arc<Nucleo>,
    chat: String,
    inicio: InicioIa,
    desfase_ms: i64,
    app: Arc<dyn AppIa>,
    oyente: Arc<dyn OyenteChatIa>,
    corte: Arc<CorteIa>,
) -> Result<FinChatIa, ErrorNucleo> {
    validar_chat(&chat)?;
    let pregunta = {
        let notas = nucleo.notas_de(&chat);
        let m = notas
            .iter()
            .find(|m| m.id == inicio.pregunta_id)
            .ok_or_else(|| ErrorNucleo::NoExiste { que: inicio.pregunta_id.clone() })?;
        ia_chat::pregunta_de(m, nucleo.es_chat_del_asistente(&chat)).unwrap_or_default()
    };
    let con_foto = {
        let notas = nucleo.notas_de(&chat);
        let hasta = notas.iter().position(|m| m.id == inicio.pregunta_id).map_or(notas.len(), |i| i + 1);
        ia_chat::foto_reciente(&notas[..hasta]).is_some()
    };
    let motor = super::turno::motor_del_turno(&ajustes, &pregunta, con_foto);
    let pensar = pixpin_ia::complejidad::decidir(ajustes.pensar, &pregunta);
    let preparado = motor.map(|(motor, nombre, nube)| {
        let entorno = Entorno {
            nombre,
            nube,
            pensar,
            ventana: if nube { VENTANA_NUBE } else { pixpin_ia::servidor::CONTEXTO },
            cada: Duration::from_millis(super::CADA_TANDA_MS),
        };
        (motor, entorno)
    });
    let (motor, entorno) = match preparado {
        Ok((m, e)) => (Some(m), Some(e)),
        Err(e) => {
            // Sin motor no hay turno: la respuesta lo dice.
            let burbuja = Burbuja {
                nucleo: nucleo.clone(),
                chat: chat.clone(),
                nota: inicio.respuesta.id.clone(),
                oyente: oyente.clone(),
                estado: Mutex::new(Estado {
                    vivo: EnVivo::default(),
                    cruzado: Instant::now(),
                    guardado: Instant::now(),
                }),
                cruzar_cada: CRUZAR_CADA,
            };
            let detalle = match e {
                ErrorNucleo::Asistente { detalle } => detalle,
                otro => otro.to_string(),
            };
            return acabar(&nucleo, &chat, &burbuja, Err(detalle), None, None, Vec::new(), Vec::new(), false);
        }
    };
    turno_en_chat_con(
        motor.expect("hay motor").as_ref(),
        &entorno.expect("hay entorno"),
        nucleo,
        chat,
        inicio,
        desfase_ms,
        app,
        oyente,
        &corte,
        CRUZAR_CADA,
    )
}

/// El turno en el chat con cualquier motor (las pruebas usan uno guionado).
#[allow(clippy::too_many_arguments)]
pub(crate) fn turno_en_chat_con(
    motor: &dyn pixpin_ia::motor::Motor,
    entorno: &Entorno,
    nucleo: Arc<Nucleo>,
    chat: String,
    inicio: InicioIa,
    desfase_ms: i64,
    app: Arc<dyn AppIa>,
    oyente: Arc<dyn OyenteChatIa>,
    corte: &CorteIa,
    cruzar_cada: Duration,
) -> Result<FinChatIa, ErrorNucleo> {
    let raiz = nucleo.ruta().to_path_buf();
    let notas = nucleo.notas_de(&chat);
    let del_asistente = nucleo.es_chat_del_asistente(&chat);
    let i = notas
        .iter()
        .position(|m| m.id == inicio.pregunta_id)
        .ok_or_else(|| ErrorNucleo::NoExiste { que: inicio.pregunta_id.clone() })?;
    let todos = &notas[..=i];
    let anteriores = &notas[..i];
    let pregunta = ia_chat::pregunta_de(&notas[i], del_asistente).unwrap_or_default();
    // La foto puede ser la misma que se acaba de mandar.
    let foto = ia_chat::foto_reciente(todos)
        .and_then(|m| nucleo.resolver(&chat, m.ruta.as_deref()?))
        .and_then(|r| foto_de(&r));
    let adjuntos = ia_chat::adjuntos(todos, |m| nucleo.resolver(&chat, m.ruta.as_deref()?).filter(|r| r.is_file()));
    // `@lienzo:<nombre>`: donde dibujar.
    let (para_el_modelo, destino) = ia_chat::resolver_lienzo(&pregunta, todos);
    let abrir = ia_chat::pide_abrir(&pregunta);
    let hoja = hoja_del_chat(&raiz, &chat, anteriores);
    let fuentes = fuentes(anteriores, &hoja);
    let (ids, historial) = conversacion_desde(&hoja, anteriores);

    let burbuja = Arc::new(Burbuja {
        nucleo: nucleo.clone(),
        chat: chat.clone(),
        nota: inicio.respuesta.id.clone(),
        oyente,
        estado: Mutex::new(Estado {
            vivo: EnVivo {
                estado: Some(ia_chat::ESTADO_INICIAL.into()),
                ..Default::default()
            },
            cruzado: Instant::now() - cruzar_cada,
            guardado: Instant::now(),
        }),
        cruzar_cada,
    });
    let manos: Arc<dyn AppIa> = Arc::new(ManosEnLaBurbuja {
        dentro: app,
        burbuja: burbuja.clone(),
    });
    let peticion = PeticionIa {
        historial,
        pedido: para_el_modelo,
        foto,
        adjuntos: adjuntos.clone(),
        hoja: hoja.a_json().to_string(),
        desfase_ms,
    };
    let mut ejecutor = EjecutorDelMac::nuevo(adjuntos, peticion.desfase_ms)
        .con_app(manos)
        .con_nucleo(Some(nucleo.clone()))
        .en_la_nube(entorno.nube)
        .en_el_chat(EnChat::nuevo(nucleo.clone(), chat.clone(), destino));
    let extra = Extra {
        fuentes: Some(fuentes),
        cortado: corte.cortado(),
    };
    let r = super::turno::turno_nucleo(motor, entorno, peticion, &mut ejecutor, extra, &mut |e| {
        burbuja.aplicar(e);
        if corte.cortado() { Seguir::Parar } else { Seguir::Si }
    });
    let (citas, leidos, lienzo) = match ejecutor.chat.take() {
        Some(c) => (ejecutor.citas.lista.clone(), c.leidos, c.lienzo),
        None => (Vec::new(), Vec::new(), None),
    };
    match r {
        Ok((_, mut detalle)) => {
            // Desde que nota sigue la conversacion si se reinicio.
            if let Some(k) = detalle.primero {
                detalle.registro.hoja.reinicio_desde = ids.get(k).cloned();
            }
            acabar(&nucleo, &chat, &burbuja, detalle.fin, Some(detalle.registro), lienzo.clone(), citas, leidos, abrir)
        }
        Err(e) => {
            let detalle = match e {
                ErrorNucleo::Asistente { detalle } => detalle,
                otro => otro.to_string(),
            };
            acabar(&nucleo, &chat, &burbuja, Err(detalle), None, lienzo, citas, leidos, abrir)
        }
    }
}

/// **El final del turno**: la burbuja acabada al disco (con el lienzo y las
/// fuentes), lo pensado a su registro, lo leido al chat y el chat arriba.
#[allow(clippy::too_many_arguments)]
fn acabar(
    nucleo: &Nucleo,
    chat: &str,
    burbuja: &Burbuja,
    fin: Result<pixpin_ia::agente::Final, String>,
    registro: Option<Registro>,
    lienzo: Option<String>,
    citas: Vec<pixpin_ia::documentos::Cita>,
    leidos: Vec<(super::turno::AdjuntoIa, String)>,
    abrir: bool,
) -> Result<FinChatIa, ErrorNucleo> {
    let (texto, vivo) = {
        let mut st = burbuja.tomar();
        st.vivo.acabar(&fin);
        (st.vivo.mostrar(), burbuja.vivo_ia(&st.vivo))
    };
    let fuentes = ia_chat::fuentes_para_la_nota(&citas, &texto);
    let hecho = nucleo.cambiar_nota(chat, &burbuja.nota, |m| {
        m.texto = texto.clone();
        if let Some(r) = &lienzo {
            m.resto.insert(ia_chat::LIENZO_DE_LA_RESPUESTA.into(), serde_json::Value::String(r.clone()));
        }
        if let Some(f) = &fuentes {
            m.resto.insert(ia_chat::FUENTES_DE_LA_RESPUESTA.into(), f.clone());
        }
        true
    })?;
    if let (Some(r), Some(m)) = (registro, hecho.as_ref()) {
        let ruta = ruta_del_registro(nucleo.ruta(), chat, &m.codigo_unico());
        let _ = r.escribir(&ruta);
    }
    for (de, entero) in &leidos {
        let _ = nucleo.dejar_texto_leido(chat, de, entero);
    }
    nucleo.subir_en_la_lista(chat, &texto);
    burbuja.oyente.vivo(vivo);
    let respuesta = hecho.ok_or_else(|| ErrorNucleo::NoExiste { que: burbuja.nota.clone() })?;
    Ok(FinChatIa {
        respuesta: MensajeChat::from(respuesta),
        abrir_lienzo: if abrir { lienzo } else { None },
    })
}

#[cfg(test)]
#[path = "en_la_conversacion/pruebas.rs"]
mod pruebas;
