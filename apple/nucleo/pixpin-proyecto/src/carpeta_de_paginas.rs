//! **Las carpetas de paginas del chat** (26-oct-2026): un cuaderno con
//! nombre dentro de la conversacion, «como si fuera un PDF».
//!
//! El usuario: «alguien usa la plantilla de diario: el dia 1 la llena en un
//! lienzo, el dia 2 en otro, y en medio escribe mas cosas en el chat. Con los
//! dias las hojas quedan dispersas». La carpeta las junta **sin moverlas**:
//! al abrirla se ven sus paginas en orden —Dia 1, Dia 2, Dia 3…—, y al
//! exportarla sale un PDF con una pagina por cada una.
//!
//! ## Como viaja
//!
//! Una carpeta es **un mensaje mas**, de clase `MINIAPP` con la palabra
//! [`MINIAPP`] (`"carpeta"`) y el documento entero en `texto`, igual que la
//! tabla de calculo del PC (`tabla.rs`). Es el hueco que dejo PixPin Android
//! para lo que no conoce: ante una palabra de `miniapp` que no reconoce
//! ensena el texto tal cual, sin romperse. Una clase nueva (`"CARPETA"`)
//! habria sido mas limpia aqui, pero **el movil no la sabe leer**: su
//! `enum class Clase` tiene diez palabras (`pixpin_sincro::kotlin::CLASES`)
//! y kotlinx tumba el mensaje que trae otra (el cable de sincronizar la
//! cambiaria por la de por omision, y la carpeta llegaria vacia de sentido).
//! La palabra de `miniapp` en cambio es texto libre, y la tabla viaja asi
//! desde hace semanas.
//!
//! Las paginas **siguen siendo mensajes normales** del chat: la carpeta solo
//! guarda la lista ordenada de sus `id` y el titulo de cada pagina. Asi un
//! lienzo sigue en la conversacion, se abre, se reenvia y se borra como
//! siempre, y puede estar en varias carpetas a la vez. Si se borra, la
//! carpeta no se rompe: la pagina que ya no esta se salta al leerla
//! ([`Carpeta::vivas`]) y se quita al guardar ([`Carpeta::podar`]).
//!
//! El nombre de la carpeta es el `nombre` del mensaje (lo que ensenan la
//! lista, el buscador y el movil); el documento solo lleva las paginas. Lo
//! que no se entiende del documento —un campo de una version mas nueva— se
//! conserva al reescribirlo.
//!
//! ```json
//! {"formato":1,"paginas":[{"id":"1761400000000","titulo":"Día 1"},
//!                          {"id":"1761486400000","titulo":"Día 2"}]}
//! ```
//!
//! ## La carpeta de una grabacion
//!
//! **Una clase o reunion grabada entra en el chat dentro de su carpeta**
//! (25-oct-2026, el usuario: «una grabacion completa siempre deberia ir
//! dentro de una carpeta, para que en el chat no salgan varias imagenes y un
//! audio, que ocupa mucho espacio»). Es una carpeta como las otras, con tres
//! campos mas que solo se escriben en ella (los demas aparatos, y un PC de
//! antes, los guardan tal cual en `resto` y la siguen leyendo como una
//! carpeta normal):
//!
//! - `tipo: "grabacion"` ([`TIPO_GRABACION`]): lo que dice que es una
//!   grabacion. Sin `tipo` (o vacio), una carpeta de paginas de siempre.
//! - `voz`: el `id` de su nota de voz (vacio si la nota no se pudo cerrar y
//!   solo entraron el video y las capturas).
//! - `video`: el `id` de su video, si se grabo (si no, no va).
//! - `duracionMs`: lo que duro, para la tarjeta sin leer la nota.
//!
//! Y cada pagina puede llevar `ms`, el minuto de la grabacion en que se
//! hizo (una captura; la voz y el video, `0`). Las paginas van en el orden
//! de la grabacion: la voz, el video y las capturas por su minuto.
//!
//! **Lo adjuntado a la clase** (un PDF del profesor, una foto de la
//! pizarra, una nota) tambien va en la carpeta, pero no es de la grabacion:
//! su pagina lleva `adjunto: true` y se ensena aparte, con otro color
//! ([`Carpeta::de_la_grabacion`] y [`Carpeta::adjuntos`]). Sin el campo (o
//! `false`), la pieza es de la grabacion.
//!
//! Los enlaces de siempre entre la voz y lo suyo (`capturas`, `video`,
//! `enGrabacion` en `resto`) no cambian: «Estudiar la clase» sigue yendo
//! por ellos.
//!
//! ## Lo de dentro no sale suelto en el chat
//!
//! (25-oct-2026, el usuario: «las paginas de un cuaderno tienen que estar
//! solo dentro de la carpeta».) **Lo que esta en una carpeta, de cualquier
//! tipo, no se ensena como burbuja en el chat** ([`dentro_de_carpetas`]): la
//! tarjeta de la carpeta lo representa. Sigue siendo un mensaje del chat
//! (viaja, se busca y sale en la galeria) y al quitarlo de la carpeta vuelve
//! a verse. Es una regla de lo que se ensena, no del formato: un aparato que
//! no la sepa lo ensena suelto, sin perder nada.
//!
//! ```json
//! {"formato":1,"tipo":"grabacion","voz":"1761400000001",
//!  "video":"1761400000002","duracionMs":5400000,
//!  "paginas":[{"id":"1761400000001","titulo":"","ms":0},
//!             {"id":"1761400000002","titulo":"","ms":0},
//!             {"id":"1761400000003","titulo":"⏱ 01:05","ms":65000}]}
//! ```

use serde::{Deserialize, Serialize};

use crate::cuaderno::{Clase, Mensaje, Sello};

/// La palabra que va en `Mensaje.miniapp`. Es un dato guardado: cambiarla
/// dejaria de reconocer las carpetas ya escritas.
pub const MINIAPP: &str = "carpeta";

/// La version del documento que se escribe.
pub const FORMATO: u32 = 1;

/// Lo mas largo de un titulo de pagina, como el nombre de un lienzo.
pub const TOPE_TITULO: usize = 80;

/// El `tipo` de la carpeta de una clase o reunion grabada. Es un dato
/// guardado.
pub const TIPO_GRABACION: &str = "grabacion";

/// Una pagina de la carpeta: que mensaje es y como se llama dentro de ella.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pagina {
    /// El `id` del mensaje (`Mensaje::id`), el mismo con el que se responde
    /// a un mensaje (`respondeA`).
    pub id: String,
    /// «Dia 2». Vacio: se ensena el nombre del mensaje.
    pub titulo: String,
    /// En una grabacion, el minuto (ms desde que empezo) en que se hizo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<i64>,
    /// En una grabacion, que se adjunto a la clase y no es de lo grabado.
    #[serde(skip_serializing_if = "es_falso")]
    pub adjunto: bool,
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

/// El documento de una carpeta: sus paginas, en orden.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Carpeta {
    pub formato: u32,
    /// Vacio: una carpeta de paginas. [`TIPO_GRABACION`]: una grabacion.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub tipo: String,
    /// En una grabacion, el `id` de su nota de voz (vacio si no entro).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub voz: String,
    /// En una grabacion, el `id` de su video (vacio si no lo hay).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub video: String,
    /// En una grabacion, lo que duro.
    #[serde(rename = "duracionMs", skip_serializing_if = "es_cero")]
    pub duracion_ms: i64,
    pub paginas: Vec<Pagina>,
    /// Lo que no se entiende, tal cual.
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

impl Default for Carpeta {
    fn default() -> Self {
        Carpeta {
            formato: FORMATO,
            tipo: String::new(),
            voz: String::new(),
            video: String::new(),
            duracion_ms: 0,
            paginas: Vec::new(),
            resto: serde_json::Map::new(),
        }
    }
}

fn es_cero(n: &i64) -> bool {
    *n == 0
}

fn es_falso(b: &bool) -> bool {
    !*b
}

/// Si el mensaje es una carpeta.
pub fn es_carpeta(m: &Mensaje) -> bool {
    m.clase == Some(Clase::MiniApp) && m.miniapp.as_deref() == Some(MINIAPP)
}

/// Las carpetas de una conversacion, por su posicion en la lista.
pub fn carpetas_de(mensajes: &[Mensaje]) -> Vec<usize> {
    mensajes
        .iter()
        .enumerate()
        .filter(|(_, m)| es_carpeta(m))
        .map(|(i, _)| i)
        .collect()
}

/// **Un mensaje de carpeta nuevo**, vacio, con su nombre.
pub fn mensaje_nuevo(nombre: &str, sello: &Sello) -> Mensaje {
    let nombre: String = nombre.trim().chars().take(TOPE_TITULO).collect();
    Mensaje::miniapp(MINIAPP, &nombre, &Carpeta::default().escribir(), sello)
}

/// **La carpeta de una grabacion**, con su nombre («Clase 25-10 10h58») y
/// lo que lleva dentro ya puesto (ver [`Carpeta::grabacion`]).
pub fn mensaje_de_grabacion(nombre: &str, carpeta: &Carpeta, sello: &Sello) -> Mensaje {
    let nombre: String = limpio(nombre);
    Mensaje::miniapp(MINIAPP, &nombre, &carpeta.escribir(), sello)
}

/// Si el mensaje es la carpeta de una grabacion.
pub fn es_grabacion(m: &Mensaje) -> bool {
    Carpeta::de_mensaje(m).is_some_and(|c| c.es_grabacion())
}

/// **Los `id` de lo que esta dentro de alguna carpeta** de la conversacion
/// (de paginas o de una grabacion): lo que el chat no ensena suelto, porque
/// lo representa la tarjeta de su carpeta.
pub fn dentro_de_carpetas(mensajes: &[Mensaje]) -> std::collections::HashSet<String> {
    mensajes
        .iter()
        .filter_map(Carpeta::de_mensaje)
        .flat_map(|c| c.paginas.into_iter().map(|p| p.id))
        .collect()
}

/// Que la pagina de un mensaje puede ir en una carpeta: lo que se ve como
/// una hoja (un lienzo, una pagina de un PDF, una foto) y tambien una nota
/// o un archivo, que salen como su texto o su primera pagina. Una carpeta
/// dentro de otra, no: una carpeta no es una pagina.
pub fn puede_ir(m: &Mensaje) -> bool {
    !es_carpeta(m) && !crate::cuaderno::es_de_sistema(m) && !m.id.is_empty()
}

impl Carpeta {
    /// Lee el documento de una carpeta. Un texto vacio es una carpeta vacia
    /// (asi nace); uno que no se entiende es un error, y quien llama no lo
    /// sobreescribe.
    pub fn leer(texto: &str) -> Result<Carpeta, serde_json::Error> {
        if texto.trim().is_empty() {
            return Ok(Carpeta::default());
        }
        serde_json::from_str(texto)
    }

    /// La carpeta de un mensaje, o `None` si no es una carpeta o su
    /// documento no se entiende.
    pub fn de_mensaje(m: &Mensaje) -> Option<Carpeta> {
        es_carpeta(m)
            .then(|| Carpeta::leer(&m.texto).ok())
            .flatten()
    }

    /// **Una carpeta de grabacion vacia**: de la nota `voz` (vacia si no
    /// entro) y que duro `duracion_ms`. Lo de dentro, con
    /// [`Carpeta::anadir_al_minuto`].
    pub fn grabacion(voz: &str, duracion_ms: i64) -> Carpeta {
        Carpeta {
            tipo: TIPO_GRABACION.into(),
            voz: voz.to_string(),
            duracion_ms: duracion_ms.max(0),
            ..Carpeta::default()
        }
    }

    /// Si es la carpeta de una grabacion.
    pub fn es_grabacion(&self) -> bool {
        self.tipo == TIPO_GRABACION
    }

    /// **Las capturas de una grabacion**: lo grabado que no es ni su nota
    /// ni su video, en su orden.
    pub fn capturas(&self) -> impl Iterator<Item = &Pagina> {
        self.de_la_grabacion()
            .filter(|p| p.id != self.voz && p.id != self.video)
    }

    /// **Lo que es de la grabacion**: la nota, el video y las capturas, en
    /// su orden. En una carpeta de paginas, todas.
    pub fn de_la_grabacion(&self) -> impl Iterator<Item = &Pagina> {
        self.paginas.iter().filter(|p| !p.adjunto)
    }

    /// **Lo adjuntado a la clase** (que no es de lo grabado), en su orden.
    pub fn adjuntos(&self) -> impl Iterator<Item = &Pagina> {
        self.paginas.iter().filter(|p| p.adjunto)
    }

    /// **Adjunta a la clase** el mensaje `id`, al final y marcado como
    /// adjunto. `false` si ya estaba.
    pub fn adjuntar(&mut self, id: &str, titulo: &str) -> bool {
        if !self.anadir(id, titulo) {
            return false;
        }
        if let Some(p) = self.paginas.last_mut() {
            p.adjunto = true;
        }
        true
    }

    /// Anade al final una pieza de la grabacion, con su minuto. Como
    /// [`Carpeta::anadir`]: `false` si ya estaba.
    pub fn anadir_al_minuto(&mut self, id: &str, titulo: &str, ms: i64) -> bool {
        if !self.anadir(id, titulo) {
            return false;
        }
        if let Some(p) = self.paginas.last_mut() {
            p.ms = Some(ms.max(0));
        }
        true
    }

    /// El documento, para ir en `Mensaje::texto`.
    pub fn escribir(&self) -> String {
        let mut c = self.clone();
        c.formato = c.formato.max(FORMATO);
        serde_json::to_string(&c).unwrap_or_else(|_| "{}".into())
    }

    pub fn len(&self) -> usize {
        self.paginas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paginas.is_empty()
    }

    /// Donde esta el mensaje `id`, si esta.
    pub fn posicion(&self, id: &str) -> Option<usize> {
        self.paginas.iter().position(|p| p.id == id)
    }

    pub fn contiene(&self, id: &str) -> bool {
        self.posicion(id).is_some()
    }

    /// **Anade una pagina al final.** Devuelve `false` si ya estaba: la
    /// misma hoja dos veces en un cuaderno solo confunde.
    pub fn anadir(&mut self, id: &str, titulo: &str) -> bool {
        if id.is_empty() || self.contiene(id) {
            return false;
        }
        self.paginas.push(Pagina {
            id: id.to_string(),
            titulo: limpio(titulo),
            ..Pagina::default()
        });
        true
    }

    /// Quita la pagina del mensaje `id`. El mensaje no se toca.
    pub fn quitar(&mut self, id: &str) -> bool {
        match self.posicion(id) {
            Some(i) => {
                self.paginas.remove(i);
                true
            }
            None => false,
        }
    }

    /// **Lleva la pagina `desde` al puesto `hasta`** (los dos desde 0; un
    /// `hasta` de mas cae al final). Devuelve si se movio.
    pub fn mover(&mut self, desde: usize, hasta: usize) -> bool {
        if desde >= self.paginas.len() {
            return false;
        }
        let hasta = hasta.min(self.paginas.len() - 1);
        if desde == hasta {
            return false;
        }
        let p = self.paginas.remove(desde);
        self.paginas.insert(hasta, p);
        true
    }

    /// Sube una pagina un puesto.
    pub fn subir(&mut self, i: usize) -> bool {
        i > 0 && self.mover(i, i - 1)
    }

    /// Baja una pagina un puesto.
    pub fn bajar(&mut self, i: usize) -> bool {
        self.mover(i, i + 1)
    }

    /// Le pone otro titulo a la pagina `i`. Devuelve si cambio.
    pub fn renombrar(&mut self, i: usize, titulo: &str) -> bool {
        let titulo = limpio(titulo);
        match self.paginas.get_mut(i) {
            Some(p) if p.titulo != titulo => {
                p.titulo = titulo;
                true
            }
            _ => false,
        }
    }

    /// **Las paginas que siguen en el chat**, en su orden y con su posicion
    /// en `mensajes`. Una pagina cuyo mensaje se borro se salta sin mas: la
    /// carpeta no se rompe porque falte una hoja.
    pub fn vivas<'a>(&'a self, mensajes: &[Mensaje]) -> Vec<(usize, &'a Pagina)> {
        self.paginas
            .iter()
            .filter_map(|p| mensajes.iter().position(|m| m.id == p.id).map(|i| (i, p)))
            .collect()
    }

    /// Quita las paginas cuyo mensaje ya no existe. Devuelve cuantas quito.
    pub fn podar(&mut self, existe: impl Fn(&str) -> bool) -> usize {
        let antes = self.paginas.len();
        self.paginas.retain(|p| existe(&p.id));
        antes - self.paginas.len()
    }

    /// **El titulo de la pagina que viene**: el de la ultima con su numero
    /// uno mas («Dia 3» tras «Dia 2»). Sin paginas, o si la ultima no lleva
    /// numero, `por_defecto` con el puesto que le toca («Pagina 4»).
    pub fn titulo_siguiente(&self, por_defecto: &str) -> String {
        self.paginas
            .iter()
            .rev()
            .find(|p| !p.titulo.trim().is_empty())
            .and_then(|p| siguiente_titulo(&p.titulo))
            .unwrap_or_else(|| format!("{} {}", por_defecto.trim(), self.paginas.len() + 1))
    }

    /// **El titulo con el que entra una hoja que ya existia** («Añadir a
    /// carpeta…»): el siguiente de la serie si la carpeta lleva una («Dia
    /// 3»); si no, el nombre que ya tenia la hoja; si no tenia, el de serie.
    pub fn titulo_al_anadir(&self, nombre_de_la_hoja: &str, por_defecto: &str) -> String {
        let serie = self
            .paginas
            .iter()
            .rev()
            .find(|p| !p.titulo.trim().is_empty())
            .and_then(|p| siguiente_titulo(&p.titulo));
        match serie {
            Some(t) => t,
            None if !nombre_de_la_hoja.trim().is_empty() => limpio(nombre_de_la_hoja),
            None => self.titulo_siguiente(por_defecto),
        }
    }
}

/// Un titulo sin saltos de linea, sin espacios de mas y con su tope.
fn limpio(t: &str) -> String {
    t.replace(['\n', '\r'], " ")
        .trim()
        .chars()
        .take(TOPE_TITULO)
        .collect()
}

/// **El titulo siguiente a `t`**: el ultimo numero que lleva, uno mas, con
/// los mismos ceros delante («Dia 09» → «Dia 10», «Semana 4» → «Semana 5»,
/// «Pagina 7 · lunes» → «Pagina 8 · lunes»). `None` si no lleva numero.
pub fn siguiente_titulo(t: &str) -> Option<String> {
    let t = t.trim();
    let bytes = t.as_bytes();
    let fin = bytes.iter().rposition(u8::is_ascii_digit)? + 1;
    let inicio = bytes[..fin]
        .iter()
        .rposition(|b| !b.is_ascii_digit())
        .map_or(0, |i| i + 1);
    let cifras = &t[inicio..fin];
    // Un numero enorme (una fecha pegada, un codigo) no se cuenta.
    if cifras.len() > 9 {
        return None;
    }
    let n: u64 = cifras.parse().ok()?;
    let siguiente = format!("{:0ancho$}", n + 1, ancho = cifras.len());
    Some(format!("{}{siguiente}{}", &t[..inicio], &t[fin..]))
}

/// **Cambia la carpeta `id` en el cuaderno de `dir`**, con el cerrojo
/// tomado (`cuaderno::cambiar`): lee, deja que `cambio` la toque y, si dice
/// que cambio, la reescribe. Las paginas cuyo mensaje ya no esta se quitan
/// de paso. Devuelve la carpeta como quedo, o `None` si el mensaje ya no
/// esta o no es una carpeta que se entienda (entonces no se escribe nada:
/// reescribir lo que no se entiende seria perderlo).
pub fn cambiar_en_disco(
    dir: &std::path::Path,
    id: &str,
    cambio: impl FnOnce(&mut Carpeta) -> bool,
) -> std::io::Result<Option<Carpeta>> {
    // Los ids que hay, leidos antes de tomar el cerrojo (`cambiar` lo toma
    // y no es reentrante). Lo que se borre entre medias se poda la proxima.
    let existen: std::collections::HashSet<String> = crate::cuaderno::Cuaderno::leer_de(dir)?
        .mensajes
        .into_iter()
        .map(|m| m.id)
        .collect();
    let mut resultado = None;
    crate::cuaderno::cambiar(dir, id, |m| {
        let Some(mut c) = Carpeta::de_mensaje(m) else {
            return false;
        };
        let podadas = c.podar(|p| existen.contains(p));
        let cambiada = cambio(&mut c);
        if cambiada || podadas > 0 {
            m.texto = c.escribir();
        }
        resultado = Some(c);
        cambiada || podadas > 0
    })?;
    Ok(resultado)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cambiar_en_disco_guarda_y_poda_lo_borrado() {
        let dir = std::env::temp_dir().join(format!("pixpin-carpeta-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let carpeta = mensaje_nuevo("Diario", &sello(1));
        crate::cuaderno::anadir(&dir, &lienzo("a")).unwrap();
        crate::cuaderno::anadir(&dir, &carpeta).unwrap();
        let c = cambiar_en_disco(&dir, &carpeta.id, |c| {
            c.anadir("a", "Día 1") && c.anadir("fantasma", "Día 2")
        })
        .unwrap()
        .unwrap();
        // La que no existe se poda al leerla la proxima vez.
        assert_eq!(c.len(), 2);
        let c = cambiar_en_disco(&dir, &carpeta.id, |_| false)
            .unwrap()
            .unwrap();
        assert_eq!(c.len(), 1, "la pagina sin mensaje se fue");
        let leida = crate::cuaderno::Cuaderno::leer_de(&dir).unwrap();
        let m = leida.mensajes.iter().find(|m| m.id == carpeta.id).unwrap();
        assert_eq!(Carpeta::de_mensaje(m).unwrap().paginas[0].titulo, "Día 1");
        // Casos negativos: un mensaje que no es carpeta, o que no esta.
        assert!(cambiar_en_disco(&dir, "a", |_| true).unwrap().is_none());
        assert!(cambiar_en_disco(&dir, "nadie", |_| true).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn sello(n: i64) -> Sello {
        Sello {
            cuando: 1_761_400_000_000 + n,
            numero: n,
            aparato: "K7Q2".into(),
            proyecto: "p1".into(),
        }
    }

    fn lienzo(id: &str) -> Mensaje {
        Mensaje {
            id: id.into(),
            clase: Some(Clase::Dibujo),
            referencia: Some(format!("dib-{id}")),
            ..Default::default()
        }
    }

    #[test]
    fn una_carpeta_nueva_es_un_mensaje_que_el_movil_ya_sabe_leer() {
        let m = mensaje_nuevo("  Diario de campo ", &sello(1));
        assert!(es_carpeta(&m));
        assert_eq!(m.nombre, "Diario de campo");
        assert_eq!(m.clase, Some(Clase::MiniApp));
        assert_eq!(m.miniapp.as_deref(), Some(MINIAPP));
        // Viaja por el hueco de las mini-apps: clase y palabra en el JSON.
        let linea = crate::cuaderno::linea_de(&m).unwrap();
        assert!(linea.contains("\"clase\":\"MINIAPP\""), "{linea}");
        assert!(linea.contains("\"miniapp\":\"carpeta\""), "{linea}");
        // Y se vuelve a leer igual de una linea del cuaderno.
        let leido = crate::cuaderno::Cuaderno::leer(&format!("{linea}\n"));
        assert_eq!(leido.mensajes.len(), 1);
        let c = Carpeta::de_mensaje(&leido.mensajes[0]).unwrap();
        assert!(c.is_empty());
        assert_eq!(c.formato, FORMATO);
        // Caso negativo: una tabla o una nota no son carpetas.
        let mut tabla = m.clone();
        tabla.miniapp = Some(crate::tabla::MINIAPP.into());
        assert!(!es_carpeta(&tabla) && Carpeta::de_mensaje(&tabla).is_none());
        assert!(!es_carpeta(&Mensaje::nota("hola", &sello(2))));
    }

    #[test]
    fn anadir_quitar_reordenar_y_renombrar() {
        let mut c = Carpeta::default();
        assert!(c.anadir("a", "Día 1"));
        assert!(c.anadir("b", "Día 2"));
        assert!(c.anadir("c", "Día 3"));
        assert!(
            !c.anadir("b", "otra vez"),
            "la misma hoja no entra dos veces"
        );
        assert!(!c.anadir("", "sin id"));
        let ids = |c: &Carpeta| c.paginas.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&c), ["a", "b", "c"]);

        assert!(c.mover(2, 0));
        assert_eq!(ids(&c), ["c", "a", "b"]);
        assert!(c.mover(0, 99), "de mas, al final");
        assert_eq!(ids(&c), ["a", "b", "c"]);
        assert!(c.subir(1));
        assert_eq!(ids(&c), ["b", "a", "c"]);
        assert!(c.bajar(1));
        assert_eq!(ids(&c), ["b", "c", "a"]);
        // Casos negativos: subir la primera, bajar la ultima, mover fuera.
        assert!(!c.subir(0));
        assert!(!c.bajar(2));
        assert!(!c.mover(5, 0));

        assert!(c.renombrar(0, "  Día\ndos  "));
        assert_eq!(c.paginas[0].titulo, "Día dos");
        assert!(!c.renombrar(0, "Día dos"), "igual no cambia nada");
        assert!(!c.renombrar(9, "x"));

        assert!(c.quitar("c"));
        assert!(!c.quitar("c"));
        assert_eq!(ids(&c), ["b", "a"]);
    }

    #[test]
    fn el_titulo_siguiente_cuenta_el_ultimo_numero() {
        assert_eq!(siguiente_titulo("Día 2").as_deref(), Some("Día 3"));
        assert_eq!(siguiente_titulo("Semana 4").as_deref(), Some("Semana 5"));
        assert_eq!(siguiente_titulo("Página 7").as_deref(), Some("Página 8"));
        assert_eq!(siguiente_titulo("Día 09").as_deref(), Some("Día 10"));
        assert_eq!(siguiente_titulo("Día 9").as_deref(), Some("Día 10"));
        assert_eq!(
            siguiente_titulo("Día 2 · lunes").as_deref(),
            Some("Día 3 · lunes")
        );
        assert_eq!(siguiente_titulo("12").as_deref(), Some("13"));
        // Casos negativos: sin numero, o un numero que no es una cuenta.
        assert_eq!(siguiente_titulo("Prólogo"), None);
        assert_eq!(siguiente_titulo(""), None);
        assert_eq!(siguiente_titulo("Foto 20261024103000"), None);

        let mut c = Carpeta::default();
        assert_eq!(c.titulo_siguiente("Página"), "Página 1");
        c.anadir("a", "Día 1");
        c.anadir("b", "Día 2");
        assert_eq!(c.titulo_siguiente("Página"), "Día 3");
        c.anadir("c", "Epílogo");
        assert_eq!(
            c.titulo_siguiente("Página"),
            "Página 4",
            "la ultima sin numero: el de serie con su puesto"
        );
    }

    #[test]
    fn una_hoja_que_ya_existia_entra_con_la_serie_o_con_su_nombre() {
        let mut c = Carpeta::default();
        assert_eq!(
            c.titulo_al_anadir("Plano de la casa", "Página"),
            "Plano de la casa"
        );
        assert_eq!(c.titulo_al_anadir("  ", "Página"), "Página 1");
        c.anadir("a", "Día 1");
        assert_eq!(c.titulo_al_anadir("Lienzo", "Página"), "Día 2");
    }

    #[test]
    fn un_mensaje_borrado_desaparece_sin_romper_la_carpeta() {
        let mut c = Carpeta::default();
        c.anadir("a", "Día 1");
        c.anadir("b", "Día 2");
        c.anadir("c", "Día 3");
        // El chat sin la «b» (se borro), y con otras cosas en medio.
        let mensajes = vec![
            lienzo("c"),
            Mensaje::nota("en medio", &sello(5)),
            lienzo("a"),
        ];
        let vivas = c.vivas(&mensajes);
        let titulos: Vec<&str> = vivas.iter().map(|(_, p)| p.titulo.as_str()).collect();
        assert_eq!(titulos, ["Día 1", "Día 3"], "en el orden de la carpeta");
        assert_eq!(vivas[0].0, 2, "con su sitio en el chat");
        assert_eq!(vivas[1].0, 0);
        let quitadas = c.podar(|id| mensajes.iter().any(|m| m.id == id));
        assert_eq!(quitadas, 1);
        assert_eq!(c.len(), 2);
        assert_eq!(c.titulo_siguiente("Página"), "Día 4");
        // Caso negativo: sin nada borrado, podar no quita nada.
        assert_eq!(c.podar(|_| true), 0);
    }

    #[test]
    fn leer_y_escribir_conserva_lo_que_no_se_entiende() {
        let texto = r#"{"formato":2,"portada":"azul","paginas":[{"id":"a","titulo":"Día 1","color":"rojo"},{"id":"b"}]}"#;
        let mut c = Carpeta::leer(texto).unwrap();
        assert_eq!(c.formato, 2, "una version mas nueva se respeta");
        assert_eq!(c.len(), 2);
        assert_eq!(c.paginas[1].titulo, "");
        c.anadir("c", "Día 3");
        let escrito = c.escribir();
        let otra = Carpeta::leer(&escrito).unwrap();
        assert_eq!(otra, c);
        assert_eq!(otra.resto.get("portada"), Some(&serde_json::json!("azul")));
        assert_eq!(
            otra.paginas[0].resto.get("color"),
            Some(&serde_json::json!("rojo"))
        );
        assert_eq!(otra.formato, 2);
        // Vacio es una carpeta vacia; lo que no es JSON, un error.
        assert!(Carpeta::leer("  ").unwrap().is_empty());
        assert!(Carpeta::leer("no es json").is_err());
    }

    #[test]
    fn que_puede_ir_en_una_carpeta() {
        assert!(puede_ir(&lienzo("a")));
        assert!(puede_ir(&Mensaje::nota("una nota", &sello(1))));
        // Casos negativos: otra carpeta y un mensaje de sistema.
        assert!(!puede_ir(&mensaje_nuevo("Otra", &sello(2))));
        let mut sistema = lienzo("x");
        sistema.id = "auto-figuras".into();
        assert!(!puede_ir(&sistema));
    }

    #[test]
    fn las_carpetas_de_una_conversacion() {
        let mensajes = vec![
            lienzo("a"),
            mensaje_nuevo("Diario", &sello(1)),
            Mensaje::nota("x", &sello(2)),
            mensaje_nuevo("Obra", &sello(3)),
        ];
        assert_eq!(carpetas_de(&mensajes), [1, 3]);
        assert!(carpetas_de(&mensajes[..1]).is_empty());
    }

    #[test]
    fn la_carpeta_de_una_grabacion_lleva_su_voz_su_duracion_y_sus_minutos() {
        let mut c = Carpeta::grabacion("v", 5_400_000);
        c.video = "video".into();
        assert!(c.es_grabacion());
        assert!(c.anadir_al_minuto("v", "", 0));
        assert!(c.anadir_al_minuto("video", "", 0));
        assert!(c.anadir_al_minuto("f1", "⏱ 01:05", 65_000));
        assert!(
            !c.anadir_al_minuto("f1", "otra vez", 9),
            "no entra dos veces"
        );
        let m = mensaje_de_grabacion("  Clase 25-10 10h58 ", &c, &sello(1));
        assert!(es_carpeta(&m) && es_grabacion(&m));
        assert_eq!(m.nombre, "Clase 25-10 10h58");
        // Lo que se escribe: el tipo, la voz, la duracion y los minutos.
        let texto = &m.texto;
        assert!(texto.contains("\"tipo\":\"grabacion\""), "{texto}");
        assert!(texto.contains("\"voz\":\"v\""), "{texto}");
        assert!(texto.contains("\"duracionMs\":5400000"), "{texto}");
        assert!(texto.contains("\"ms\":65000"), "{texto}");
        assert!(texto.contains("\"video\":\"video\""), "{texto}");
        let capturas: Vec<&str> = c.capturas().map(|p| p.id.as_str()).collect();
        assert_eq!(capturas, ["f1"], "ni la nota ni el video");
        let leida = Carpeta::de_mensaje(&m).unwrap();
        assert_eq!(leida, c);
        assert_eq!(leida.paginas[2].ms, Some(65_000));
        // Caso negativo: una carpeta de paginas no escribe nada de eso.
        let normal = mensaje_nuevo("Diario", &sello(2));
        assert!(!es_grabacion(&normal));
        assert_eq!(normal.texto, r#"{"formato":1,"paginas":[]}"#);
    }

    #[test]
    fn un_pc_de_antes_conserva_lo_de_la_grabacion_y_la_lee_como_carpeta() {
        // Lo que escribe un PC de antes de las grabaciones: sin los campos.
        let viejo = r#"{"formato":1,"paginas":[{"id":"a","titulo":"Día 1"}]}"#;
        let c = Carpeta::leer(viejo).unwrap();
        assert!(!c.es_grabacion());
        assert_eq!(c.paginas[0].ms, None);
        assert!(!c.escribir().contains("\"ms\""), "sin minuto no se escribe");
        // Y lo que lee de una grabacion lo trata como una carpeta mas.
        let nuevo = r#"{"formato":1,"tipo":"grabacion","voz":"v","duracionMs":9,"paginas":[{"id":"v","titulo":"","ms":0}]}"#;
        let c = Carpeta::leer(nuevo).unwrap();
        assert!(c.es_grabacion());
        assert_eq!((c.voz.as_str(), c.duracion_ms), ("v", 9));
        assert_eq!(Carpeta::leer(&c.escribir()).unwrap(), c);
    }

    #[test]
    fn lo_de_dentro_de_cualquier_carpeta_no_sale_suelto() {
        let mut g = Carpeta::grabacion("v", 1000);
        g.anadir_al_minuto("v", "", 0);
        g.anadir_al_minuto("f1", "", 500);
        let mut diario = Carpeta::default();
        diario.anadir("a", "Día 1");
        let mensajes = vec![
            lienzo("a"),
            Mensaje::nota("v", &sello(1)),
            mensaje_de_grabacion("Clase", &g, &sello(2)),
            Mensaje::miniapp(MINIAPP, "Diario", &diario.escribir(), &sello(3)),
        ];
        let dentro = dentro_de_carpetas(&mensajes);
        assert!(dentro.contains("v") && dentro.contains("f1"));
        assert!(dentro.contains("a"), "la pagina de un cuaderno, tambien");
        assert_eq!(dentro.len(), 3);
        // Caso negativo: sin carpetas, nada esta dentro.
        assert!(dentro_de_carpetas(&mensajes[..2]).is_empty());
    }

    #[test]
    fn lo_adjuntado_a_la_clase_va_aparte_de_lo_grabado() {
        let mut c = Carpeta::grabacion("v", 1000);
        c.anadir_al_minuto("v", "", 0);
        c.anadir_al_minuto("f1", "⏱ 0:01", 1000);
        assert!(c.adjuntar("pdf", "Apuntes"));
        assert!(!c.adjuntar("f1", "otra vez"), "lo que ya estaba no entra");
        let ids = |v: Vec<&Pagina>| v.into_iter().map(|p| p.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(c.de_la_grabacion().collect()), ["v", "f1"]);
        assert_eq!(ids(c.adjuntos().collect()), ["pdf"]);
        assert_eq!(
            ids(c.capturas().collect()),
            ["f1"],
            "un adjunto no es captura"
        );
        let texto = c.escribir();
        assert!(texto.contains("\"adjunto\":true"), "{texto}");
        assert_eq!(texto.matches("adjunto").count(), 1, "solo en el adjunto");
        assert_eq!(Carpeta::leer(&texto).unwrap(), c);
    }
}
