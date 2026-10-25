//! **Buscar dentro de todos los chats** desde el buscador de la lista
//! (Windows 97e7f88, d106969 y 02b3124, 25-oct-2026, el usuario: «que el
//! buscador busque tambien dentro de todos los chats»). Como en Telegram:
//! debajo de los chats que casan por su nombre, la seccion «Mensajes · N»
//! con los mensajes de cualquier chat que llevan lo escrito, del mas nuevo
//! al mas viejo; pulsar uno abre su chat llevado a el y resaltado.
//!
//! Es `ventana_chat/buscar_en_chats.rs` de Windows sin pintar ni hilo: lo
//! puro (aplanar, buscar con tope y cuenta, el trozo) es
//! `pixpin_ui::buscar_en_chats`, el mismo crate. Aqui, **el indice**: los
//! mensajes de todos los chats ya aplanados, hecho la primera vez que se
//! busca y despues solo releyendo el `guardados.jsonl` del chat que cambio
//! (por su fecha y su tamano). Swift lo llama desde una `Task` de fondo
//! con la espera de 180 ms ([`MedidasDeBuscarEnChats::espera_ms`]) y tira
//! lo que conteste a una busqueda vieja.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use fluent_bundle::FluentArgs;
use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{self, Mensaje};
use pixpin_store::ajustes::PreferenciaIdioma;
use pixpin_store::idioma::{Catalogo, resolver_idioma};
use pixpin_ui::buscar_en_chats as b;

use crate::Nucleo;

/// Cuantos caracteres del mensaje se ensenan antes de lo encontrado, y
/// cuantos como mucho en la linea (`ANTES` y `LARGO` de Windows).
pub const ANTES: usize = 22;
pub const LARGO: usize = 160;

/// Un trozo de lo encontrado dentro del texto, en unidades UTF-16 (las de
/// `NSString`): va en negrita y con el fondo de enviar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct TramoEncontrado {
    pub inicio: u32,
    pub largo: u32,
}

/// **Un mensaje encontrado**, listo para pintar su fila.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MensajeEncontrado {
    /// El `id` de la ficha de su chat.
    pub chat: String,
    /// El `id` del mensaje (para marcarlo al saltar).
    pub mensaje: String,
    /// Su posicion en el chat (0 el mas antiguo, la de [`Nucleo::mensajes`]):
    /// la pagina que se carga al saltar.
    pub posicion: u64,
    /// Su codigo unico.
    pub codigo: String,
    pub cuando: i64,
    /// El trozo en una linea, con «…» si se corta.
    pub texto: String,
    pub tramos: Vec<TramoEncontrado>,
}

/// Lo que contesta una busqueda: los primeros `tope` y cuantos hay en total.
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
pub struct EncontradoEnChats {
    pub mensajes: Vec<MensajeEncontrado>,
    pub total: u64,
}

/// Las medidas y los tiempos de la seccion, los de Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct MedidasDeBuscarEnChats {
    /// Cuantos se ensenan de primeras, y cuantos mas con cada «Ver más».
    pub de_primeras: u32,
    /// La espera desde la ultima tecla antes de buscar.
    pub espera_ms: u32,
    /// El alto de la cabecera «Mensajes · N» y de la fila «Ver N más».
    pub cabecera: u32,
    pub ver_mas: u32,
    /// El alto de cada fila: el de una fila de chat.
    pub fila: u32,
}

#[uniffi::export]
pub fn medidas_de_buscar_en_chats() -> MedidasDeBuscarEnChats {
    MedidasDeBuscarEnChats {
        de_primeras: b::DE_PRIMERAS as u32,
        espera_ms: b::ESPERA_MS as u32,
        cabecera: b::CABECERA_SECCION,
        ver_mas: b::VER_MAS,
        fila: pixpin_ui::chat::FILA,
    }
}

/// **Si un chat pasa el filtro de la lista** (02b3124): lo escrito en
/// cualquier sitio de su nombre o de su ultima linea, sin mirar mayusculas
/// ni tildes (como la seccion «Mensajes»). Sin nada escrito, todos.
#[uniffi::export]
pub fn chat_pasa_el_filtro(nombre: String, resumen: String, aguja: String) -> bool {
    let a = aguja.trim();
    a.is_empty()
        || pixpin_ui::resaltado::hay_coincidencia(&nombre, a)
        || pixpin_ui::resaltado::hay_coincidencia(&resumen, a)
}

/// Los rotulos de la seccion: «Mensajes · N» y «Ver N más».
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RotulosDeBuscarEnChats {
    pub mensajes: String,
    pub ver_mas: String,
}

/// Los rotulos para `total` encontrados y `mas` por ensenar
/// (`chat-buscar-mensajes` y `chat-buscar-ver-mas` de `pixpin-store`).
#[uniffi::export]
pub fn rotulos_de_buscar_en_chats(locale: String, total: u64, mas: u64) -> RotulosDeBuscarEnChats {
    let t = Catalogo::nuevo(resolver_idioma(&locale, PreferenciaIdioma::Sistema));
    let con = |clave: &str, n: u64| {
        let mut a = FluentArgs::new();
        a.set("n", n);
        t.t_args(clave, &a)
    };
    RotulosDeBuscarEnChats {
        mensajes: con("chat-buscar-mensajes", total),
        ver_mas: con("chat-buscar-ver-mas", mas),
    }
}

// --- El indice -------------------------------------------------------------

/// Lo que se busca de un mensaje: su texto (la nota rica sin las rutas de
/// sus fotos, que no son de nadie), su transcripcion y el nombre de su
/// fichero. El pie de una foto es su texto (`entrada_de` de Windows).
pub fn entrada_de(chat: &str, m: &Mensaje) -> b::Entrada {
    let texto = cuaderno::sin_rutas_de_fotos(&m.texto);
    b::Entrada::nueva(
        chat,
        &m.codigo_unico(),
        m.cuando,
        [
            texto.as_str(),
            m.transcripcion.as_deref().unwrap_or_default(),
            m.nombre.as_str(),
        ],
    )
}

/// La marca de las paginas sueltas de un PDF unido (`pdf_en_chat::SOLO_GALERIA`
/// de Windows): solo salen en la galeria.
const SOLO_GALERIA: &str = "_soloGaleria";

/// Si el mensaje tiene burbuja en su chat: lo que no la tiene (las paginas
/// de un PDF unido, las lecciones, lo del sistema) no se encuentra, porque
/// pulsarlo no llevaria a ninguna parte.
pub fn se_ve_en_el_chat(m: &Mensaje) -> bool {
    let solo_galeria = m.resto.get(SOLO_GALERIA).and_then(|v| v.as_bool()) == Some(true);
    let de_leccion = crate::lecciones::es_leccion(m)
        || m.responde_a
            .as_deref()
            .is_some_and(|r| r.starts_with(pixpin_lecciones::leccion::PREFIJO));
    !solo_galeria && !de_leccion && !cuaderno::es_de_sistema(m)
}

/// Por lo que se sabe si un cuaderno cambio: su fecha y su tamano.
type Firma = Option<(SystemTime, u64)>;

fn firma_de(carpeta: &Path) -> Firma {
    let m = std::fs::metadata(carpeta.join("guardados.jsonl")).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// Un mensaje del indice: lo que se busca y por donde se le encuentra.
struct Lugar {
    entrada: b::Entrada,
    id: String,
    posicion: u64,
}

/// Los mensajes de un chat, ya aplanados.
struct Chat {
    id: String,
    firma: Firma,
    lugares: Vec<Lugar>,
}

/// Lee los mensajes de un chat con su posicion, la misma que da
/// [`Nucleo::mensajes`]: los que no son de sistema, por su hora (estable).
fn leer_chat(raiz: &Path, id: &str, firma: Firma) -> Chat {
    let carpeta = almacen::carpeta(raiz, id);
    let texto = std::fs::read_to_string(carpeta.join("guardados.jsonl")).unwrap_or_default();
    let mut visibles: Vec<Mensaje> = texto
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Mensaje>(l).ok())
        .filter(|m| !m.id.starts_with(cuaderno::PREFIJO_DE_SISTEMA))
        .collect();
    visibles.sort_by_key(|m| m.cuando);
    let lugares = visibles
        .iter()
        .enumerate()
        .filter(|(_, m)| se_ve_en_el_chat(m))
        .map(|(i, m)| Lugar {
            entrada: entrada_de(id, m),
            id: m.id.clone(),
            posicion: i as u64,
        })
        .collect();
    Chat {
        id: id.to_string(),
        firma,
        lugares,
    }
}

/// **El indice**: los mensajes de todos los chats y en que orden se buscan
/// (lo mas nuevo primero), como pares (chat, mensaje).
#[derive(Default)]
struct Indice {
    chats: Vec<Chat>,
    orden: Vec<(u32, u32)>,
}

impl Indice {
    /// Relee los chats nuevos o cambiados y suelta los que ya no estan.
    fn poner_al_dia(&mut self, raiz: &Path) {
        let fichas = almacen::Indice::leer(raiz).proyectos;
        let antes: Vec<String> = self.chats.iter().map(|c| c.id.clone()).collect();
        let mut viejos: HashMap<String, Chat> = std::mem::take(&mut self.chats)
            .into_iter()
            .map(|c| (c.id.clone(), c))
            .collect();
        let mut cambio = false;
        for f in &fichas {
            let firma = firma_de(&almacen::carpeta(raiz, &f.id));
            let chat = match viejos.remove(&f.id) {
                Some(c) if c.firma == firma => c,
                _ => {
                    cambio = true;
                    leer_chat(raiz, &f.id, firma)
                }
            };
            self.chats.push(chat);
        }
        cambio |= self.chats.iter().map(|c| &c.id).ne(antes.iter());
        if cambio {
            self.ordenar();
        }
    }

    fn ordenar(&mut self) {
        let mut orden: Vec<(u32, u32)> = self
            .chats
            .iter()
            .enumerate()
            .flat_map(|(c, chat)| (0..chat.lugares.len()).map(move |i| (c as u32, i as u32)))
            .collect();
        let chats = &self.chats;
        orden.sort_by(|x, y| {
            b::comparar(
                &chats[x.0 as usize].lugares[x.1 as usize].entrada,
                &chats[y.0 as usize].lugares[y.1 as usize].entrada,
            )
        });
        self.orden = orden;
    }

    fn lugar(&self, (c, i): (u32, u32)) -> &Lugar {
        &self.chats[c as usize].lugares[i as usize]
    }

    fn buscar(&self, aguja: &str, tope: usize) -> EncontradoEnChats {
        let hallado = b::buscar(
            self.orden.iter().map(|&par| &self.lugar(par).entrada),
            aguja,
            tope,
        );
        let mensajes = hallado
            .aciertos
            .iter()
            .map(|a| {
                let l = self.lugar(self.orden[a.entrada]);
                let e = &l.entrada;
                let f = b::fragmento(&e.piezas[a.pieza].texto, aguja, ANTES, LARGO);
                let tramos = f
                    .tramos
                    .iter()
                    .map(|t| {
                        let (inicio, largo) = t.en_utf16(&f.texto);
                        TramoEncontrado { inicio, largo }
                    })
                    .collect();
                MensajeEncontrado {
                    chat: e.chat.clone(),
                    mensaje: l.id.clone(),
                    posicion: l.posicion,
                    codigo: e.codigo.clone(),
                    cuando: e.cuando,
                    texto: f.texto,
                    tramos,
                }
            })
            .collect();
        EncontradoEnChats {
            mensajes,
            total: hallado.total as u64,
        }
    }
}

/// **El buscador en todos los chats** de una ventana: su indice vive aqui
/// mientras Swift lo tenga.
#[derive(uniffi::Object)]
pub struct BuscadorEnChats {
    raiz: PathBuf,
    indice: Mutex<Indice>,
}

#[uniffi::export]
impl BuscadorEnChats {
    /// **Busca `aguja`** en todos los chats: pone al dia el indice (solo lo
    /// que cambio) y devuelve los `tope` primeros, del mas nuevo al mas
    /// viejo, con la cuenta de todos. Bloquea: desde una `Task` de fondo.
    pub fn buscar(&self, aguja: String, tope: u32) -> EncontradoEnChats {
        if b::aguja_plana(&aguja).is_none() {
            return EncontradoEnChats::default();
        }
        let mut ind = self.indice.lock().unwrap_or_else(|e| e.into_inner());
        ind.poner_al_dia(&self.raiz);
        ind.buscar(aguja.trim(), tope as usize)
    }
}

#[uniffi::export]
impl Nucleo {
    /// Un buscador en todos los chats nuevo (su indice nace con la primera
    /// busqueda).
    pub fn buscador_en_chats(&self) -> Arc<BuscadorEnChats> {
        Arc::new(BuscadorEnChats {
            raiz: self.ruta().to_path_buf(),
            indice: Mutex::new(Indice::default()),
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn raiz(etiqueta: &str) -> (PathBuf, Arc<Nucleo>) {
        let r = std::env::temp_dir().join(format!(
            "pixpin-nucleo-buscar-chats-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        let n = Nucleo::abrir(r.to_string_lossy().into_owned(), "Mac".into()).unwrap();
        (r, n)
    }

    fn escribir(n: &Nucleo, chat: &str, mensajes: &[Mensaje]) {
        let carpeta = almacen::carpeta(n.ruta(), chat);
        std::fs::create_dir_all(&carpeta).unwrap();
        let texto: String = mensajes
            .iter()
            .map(|m| serde_json::to_string(m).unwrap() + "\n")
            .collect();
        std::fs::write(carpeta.join("guardados.jsonl"), texto).unwrap();
    }

    fn nota(id: &str, cuando: i64, texto: &str) -> Mensaje {
        Mensaje {
            id: id.into(),
            cuando,
            texto: texto.into(),
            ..Default::default()
        }
    }

    #[test]
    fn de_la_nota_rica_se_busca_el_texto_y_no_las_rutas_de_sus_fotos() {
        let m = Mensaje {
            id: "m1".into(),
            cuando: 7,
            texto: "Fachada norte ![grieta](pixpin:files/fotos/IMG_0042.jpg) revisar".into(),
            transcripcion: Some("lo dicho en la obra".into()),
            nombre: "plano.pdf".into(),
            ..Default::default()
        };
        let e = entrada_de("chat", &m);
        assert_eq!(b::buscar([&e], "img_0042", 5).total, 0, "la ruta no cuenta");
        assert_eq!(b::buscar([&e], "grieta", 5).total, 1, "su pie si");
        assert_eq!(b::buscar([&e], "LO DICHO", 5).total, 1);
        assert_eq!(b::buscar([&e], "plano", 5).total, 1);
        assert_eq!(e.codigo, m.codigo_unico());
    }

    #[test]
    fn lo_que_no_tiene_burbuja_no_se_encuentra() {
        let sistema = Mensaje {
            id: format!("{}x", cuaderno::PREFIJO_DE_SISTEMA),
            ..Default::default()
        };
        assert!(!se_ve_en_el_chat(&sistema));
        let mut suelta = nota("p1", 1, "pagina");
        suelta.resto.insert(SOLO_GALERIA.into(), serde_json::json!(true));
        assert!(!se_ve_en_el_chat(&suelta));
        assert!(se_ve_en_el_chat(&nota("normal", 1, "hola")));
    }

    #[test]
    fn busca_en_todos_del_mas_nuevo_al_mas_viejo_con_su_posicion() {
        let (r, n) = raiz("todos");
        let obra = n.crear_chat("Obra".into()).unwrap();
        let casa = n.crear_chat("Casa".into()).unwrap();
        escribir(
            &n,
            &obra.id,
            &[
                nota("a2", 20, "El cemento llega el martes"),
                nota("a1", 10, "Pedir CEMENTO y arena"),
                nota(&format!("{}s", cuaderno::PREFIJO_DE_SISTEMA), 15, "cemento del sistema"),
            ],
        );
        escribir(&n, &casa.id, &[nota("b1", 30, "Sin cemento no hay cimentación")]);
        let buscador = n.buscador_en_chats();
        let h = buscador.buscar("cemento".into(), 20);
        assert_eq!(h.total, 3, "el de sistema no cuenta");
        let ids: Vec<&str> = h.mensajes.iter().map(|m| m.mensaje.as_str()).collect();
        assert_eq!(ids, ["b1", "a2", "a1"]);
        // La posicion es la del chat por su hora: «a1» es el primero.
        assert_eq!(h.mensajes[2].posicion, 0);
        assert_eq!(h.mensajes[1].posicion, 1);
        assert_eq!(h.mensajes[0].chat, casa.id);
        // Lo encontrado va resaltado en UTF-16.
        let m = &h.mensajes[0];
        assert_eq!(m.texto, "Sin cemento no hay cimentación");
        assert_eq!(m.tramos, vec![TramoEncontrado { inicio: 4, largo: 7 }]);

        // Sin tildes, y el tope corta lo devuelto pero no la cuenta.
        let h = buscador.buscar("CIMENTACION".into(), 20);
        assert_eq!(h.total, 1);
        let h = buscador.buscar("cemento".into(), 1);
        assert_eq!((h.mensajes.len(), h.total), (1, 3));

        // Un chat que cambia se relee: lo nuevo se encuentra.
        std::thread::sleep(std::time::Duration::from_millis(20));
        escribir(&n, &casa.id, &[nota("b1", 30, "Sin cemento"), nota("b2", 40, "Mas cemento, sin cal")]);
        let h = buscador.buscar("cemento".into(), 20);
        assert_eq!(h.total, 4);
        assert_eq!(h.mensajes[0].mensaje, "b2");

        // Caso negativo: nada escrito (o solo espacios) no busca nada.
        assert_eq!(buscador.buscar("   ".into(), 20), EncontradoEnChats::default());
        assert_eq!(buscador.buscar("zzzz".into(), 20).total, 0);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn el_filtro_de_la_lista_no_mira_tildes_y_los_rotulos_son_los_de_windows() {
        assert!(chat_pasa_el_filtro("Cimentación".into(), String::new(), "cimentacion".into()));
        assert!(chat_pasa_el_filtro("Casa de playa".into(), String::new(), "PLAYA".into()));
        assert!(chat_pasa_el_filtro("Casa".into(), "la ultima línea".into(), "linea".into()));
        assert!(chat_pasa_el_filtro("Casa".into(), String::new(), "  ".into()));
        // Caso negativo.
        assert!(!chat_pasa_el_filtro("Casa".into(), "algo".into(), "obra".into()));

        let r = rotulos_de_buscar_en_chats("es-ES".into(), 34, 14);
        assert_eq!(r.mensajes, "Mensajes · 34");
        assert_eq!(r.ver_mas, "Ver 14 más");
        let r = rotulos_de_buscar_en_chats("en-US".into(), 1, 20);
        assert_eq!(r.mensajes, "Messages · 1");
        assert_eq!(r.ver_mas, "Show 20 more");
        let m = medidas_de_buscar_en_chats();
        assert_eq!((m.de_primeras, m.espera_ms, m.cabecera, m.ver_mas, m.fila), (20, 180, 30, 40, 62));
    }
}
