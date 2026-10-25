//! **La foto del chat**: lo que se ve en el circulo de cada chat (25-oct-2026,
//! el usuario: «permitir poner un fondo en las esferas de los chats para que
//! parezca mas una app de chat: dame la opcion de poner emojis como su
//! perfil, o poner una imagen yo»).
//!
//! Es la evolucion del logo de imagen del 4-oct (`logo.png` en la carpeta del
//! proyecto, solo del PC). Ahora **viaja**: vive en la ficha del proyecto
//! (`resto["avatar"]` del indice) y sale en el `Proyecto` que va por la red
//! como un campo mas, `avatar`, que el movil conserva aunque no lo entienda
//! (regla 1 de sincronizar). Su imagen, si tiene, es un fichero del proyecto
//! nombrado con su ruta portatil, asi que `alcance` la lleva con el.
//!
//! El formato, el mismo en todos los aparatos:
//!
//! ```json
//! "avatar": {"tipo": "imagen",    "ruta": "pixpin:files/guardados/pc/<chat>/avatar-1761400000000.png", "cuando": 1761400000000}
//! "avatar": {"tipo": "emoji",     "emoji": "🏠", "color": "#FF885E", "color2": "#FF516A", "cuando": 1761400000000}
//! "avatar": {"tipo": "iniciales", "color": "#72D5FD", "color2": "#2A9EF1", "cuando": 1761400000000}
//! "avatar": {"tipo": "iniciales", "cuando": 1761400000000}
//! ```
//!
//! - `color` y `color2` son el degradado de arriba abajo del circulo (detras
//!   del emoji o de las iniciales). Sin `color2`, liso; sin ninguno, el color
//!   de siempre, el que sale del codigo unico del proyecto.
//! - La imagen ya va **cuadrada, recortada y pequena** (256 px de lado, PNG):
//!   quien la pinta solo la mete en el circulo.
//! - **Volver a lo de siempre se escribe**, no se borra: `{"tipo":"iniciales"}`
//!   sin colores. Asi un aparato que aun tenga el `logo.png` de antes no lo
//!   resucita, y la fusion ve un cambio y no una ausencia.
//! - Un `tipo` que no se conozca se pinta como las iniciales y **se conserva
//!   tal cual**: la ficha guarda el JSON crudo, y es ese el que sale.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::almacen::{self, Ficha, Indice};

/// La clave en la ficha y en el `Proyecto` que viaja.
pub const CLAVE: &str = "avatar";
/// El logo de antes (4-oct-2026), solo del PC: si un proyecto no tiene
/// `avatar` y si este fichero, es su foto.
pub const LOGO_DE_ANTES: &str = "logo.png";
/// El lado con que se guarda la imagen: el avatar mas grande (la isla, 56 px)
/// al 300 % y algo de sobra.
pub const LADO: u32 = 256;

pub const IMAGEN: &str = "imagen";
pub const EMOJI: &str = "emoji";
pub const INICIALES: &str = "iniciales";

/// La foto del chat leida. Solo para pintar y decidir: lo que se guarda y
/// viaja es el JSON crudo de la ficha.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Avatar {
    pub tipo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    /// `#RRGGBB`: arriba del degradado.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// `#RRGGBB`: abajo del degradado. Sin el, liso.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color2: Option<String>,
    /// Relativa a la carpeta del proyecto si nacio aqui (`avatar-<ms>.png`),
    /// o portatil (`pixpin:files/…`) si llego de otro aparato.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruta: Option<String>,
    /// Cuando se puso, en milisegundos desde 1970.
    #[serde(skip_serializing_if = "es_cero")]
    pub cuando: i64,
}

fn es_cero(v: &i64) -> bool {
    *v == 0
}

impl Avatar {
    /// Una imagen ya guardada en la carpeta del proyecto (`guardar_imagen`).
    pub fn imagen(ruta: &str, cuando: i64) -> Avatar {
        Avatar {
            tipo: IMAGEN.into(),
            ruta: Some(ruta.into()),
            cuando,
            ..Default::default()
        }
    }

    /// Un emoji sobre su fondo.
    pub fn emoji(emoji: &str, fondo: Option<(&str, &str)>, cuando: i64) -> Avatar {
        let (color, color2) = colores(fondo);
        Avatar {
            tipo: EMOJI.into(),
            emoji: Some(emoji.into()),
            color,
            color2,
            cuando,
            ..Default::default()
        }
    }

    /// Las iniciales, sobre el fondo elegido o (sin fondo) el de siempre:
    /// esto ultimo es «volver a lo de siempre».
    pub fn iniciales(fondo: Option<(&str, &str)>, cuando: i64) -> Avatar {
        let (color, color2) = colores(fondo);
        Avatar {
            tipo: INICIALES.into(),
            color,
            color2,
            cuando,
            ..Default::default()
        }
    }

    pub fn es_imagen(&self) -> bool {
        self.tipo == IMAGEN && self.ruta.as_deref().is_some_and(|r| !r.is_empty())
    }

    /// El emoji, si es de emoji y trae uno.
    pub fn el_emoji(&self) -> Option<&str> {
        (self.tipo == EMOJI)
            .then_some(self.emoji.as_deref())
            .flatten()
            .filter(|e| !e.trim().is_empty())
    }

    /// Si se pinta como siempre: iniciales (o un tipo que no se conoce, o un
    /// emoji vacio) sin fondo propio.
    pub fn es_lo_de_siempre(&self) -> bool {
        !self.es_imagen() && self.el_emoji().is_none() && self.fondo().is_none()
    }

    /// El degradado del fondo `(arriba, abajo)` en `0xRRGGBB`, si se eligio
    /// uno valido. Con un solo color, liso.
    pub fn fondo(&self) -> Option<(u32, u32)> {
        let arriba = self.color.as_deref().and_then(color_de_texto)?;
        let abajo = self
            .color2
            .as_deref()
            .and_then(color_de_texto)
            .unwrap_or(arriba);
        Some((arriba, abajo))
    }

    /// En JSON, para la ficha.
    pub fn a_valor(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

fn colores(fondo: Option<(&str, &str)>) -> (Option<String>, Option<String>) {
    match fondo {
        Some((a, b)) if a == b => (Some(a.into()), None),
        Some((a, b)) => (Some(a.into()), Some(b.into())),
        None => (None, None),
    }
}

/// `#RRGGBB` (o `RRGGBB`) a `0xRRGGBB`. Otra cosa, `None`.
pub fn color_de_texto(t: &str) -> Option<u32> {
    let h = t.trim().strip_prefix('#').unwrap_or(t.trim());
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(h, 16).ok()
}

/// `0xRRGGBB` a `#RRGGBB`, como se guarda.
pub fn texto_de_color(c: u32) -> String {
    format!("#{:06X}", c & 0x00ff_ffff)
}

/// El JSON crudo de la foto de una ficha, si tiene.
pub fn valor_de(f: &Ficha) -> Option<&serde_json::Value> {
    f.resto.get(CLAVE).filter(|v| v.is_object())
}

/// La foto de una ficha, leida. Un JSON que no se entiende es «ninguna».
pub fn de_ficha(f: &Ficha) -> Option<Avatar> {
    serde_json::from_value(valor_de(f)?.clone()).ok()
}

/// Lo mismo, y si no tiene, el `logo.png` de antes si esta en su carpeta.
/// Asi el logo que ya habia puesto el usuario sigue saliendo y, ademas,
/// empieza a viajar sin hacer nada.
pub fn de_ficha_o_logo(raiz: &Path, f: &Ficha) -> Option<Avatar> {
    if valor_de(f).is_some() {
        return de_ficha(f);
    }
    almacen::carpeta(raiz, &f.id)
        .join(LOGO_DE_ANTES)
        .is_file()
        .then(|| Avatar::imagen(LOGO_DE_ANTES, 0))
}

/// Donde esta en este equipo la imagen de una foto `imagen`.
pub fn ruta_de_la_imagen(raiz: &Path, ficha_id: &str, a: &Avatar) -> Option<PathBuf> {
    if !a.es_imagen() {
        return None;
    }
    crate::vista::ruta_real(raiz, ficha_id, a.ruta.as_deref()?)
}

/// **Lo que llego de otro aparato**: la foto de `p` (el `Proyecto` que se
/// guarda) pasa a la ficha, o se quita de ella si no trae. Devuelve si
/// cambio, para limpiar las imagenes que ya no se usan
/// ([`limpiar_tras_llegar`]).
pub fn al_llegar(f: &mut Ficha, p: Option<&serde_json::Value>) -> bool {
    let nuevo = p.filter(|v| v.is_object()).cloned();
    if f.resto.get(CLAVE) == nuevo.as_ref() {
        return false;
    }
    match nuevo {
        Some(v) => f.resto.insert(CLAVE.into(), v),
        None => f.resto.remove(CLAVE),
    };
    true
}

/// Tras cambiar la foto por lo que llego: se borran las imagenes de foto de
/// la carpeta que ya no son la suya.
pub fn limpiar_tras_llegar(raiz: &Path, f: &Ficha) {
    let carpeta = almacen::carpeta(raiz, &f.id);
    let queda = de_ficha(f)
        .and_then(|a| ruta_de_la_imagen(raiz, &f.id, &a))
        .filter(|r| r.parent() == Some(carpeta.as_path()))
        .and_then(|r| r.file_name().map(|n| n.to_string_lossy().to_string()));
    // El logo de antes solo se toca si la ficha ya dice algo: sin `avatar`
    // sigue siendo su foto.
    if valor_de(f).is_some() {
        limpiar_imagenes(raiz, &f.id, queda.as_deref());
    }
}

/// **Pone la foto de un proyecto** en su ficha del indice. Si se deja de
/// usar una imagen nacida aqui, se borra (y el `logo.png` de antes con ella).
pub fn poner(raiz: &Path, id: &str, a: &Avatar) -> std::io::Result<()> {
    let mut indice = Indice::leer_si_esta(raiz)?;
    let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == id) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no hay ese proyecto",
        ));
    };
    f.resto.insert(CLAVE.into(), a.a_valor());
    indice.guardar(raiz)?;
    limpiar_imagenes(raiz, id, a.ruta.as_deref().filter(|_| a.es_imagen()));
    Ok(())
}

/// El nombre de la imagen que se guarda en un momento: uno nuevo cada vez,
/// para que el otro aparato la vea como un fichero nuevo (y no como el mismo
/// cambiado, que pierde contra el suyo si su reloj va adelantado).
pub fn nombre_de_imagen(cuando: i64) -> String {
    format!("avatar-{cuando}.png")
}

/// Guarda los bytes PNG de una foto en la carpeta del proyecto y devuelve su
/// ruta relativa, la que va en `ruta`. Entero o nada.
pub fn guardar_imagen(raiz: &Path, id: &str, png: &[u8], cuando: i64) -> std::io::Result<String> {
    let rel = nombre_de_imagen(cuando);
    pixpin_sincro::disco::escribir_atomico(&almacen::carpeta(raiz, id).join(&rel), png)?;
    Ok(rel)
}

/// Si `nombre` es una imagen de foto guardada aqui (`avatar-<cifras>.png`).
pub fn es_imagen_de_avatar(nombre: &str) -> bool {
    nombre
        .strip_prefix("avatar-")
        .and_then(|x| x.strip_suffix(".png"))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Borra de la carpeta del proyecto las imagenes de foto que ya no se usan
/// (todas menos `queda`) y el `logo.png` de antes. Lo que no se pueda
/// borrar se queda: solo ocupa sitio.
fn limpiar_imagenes(raiz: &Path, id: &str, queda: Option<&str>) {
    let carpeta = almacen::carpeta(raiz, id);
    let Ok(lista) = std::fs::read_dir(&carpeta) else {
        return;
    };
    for e in lista.flatten() {
        let nombre = e.file_name().to_string_lossy().to_string();
        let sobra = (es_imagen_de_avatar(&nombre) || nombre == LOGO_DE_ANTES)
            && Some(nombre.as_str()) != queda;
        if sobra {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn raiz(etiqueta: &str) -> PathBuf {
        let r =
            std::env::temp_dir().join(format!("pixpin-avatar-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        r
    }

    #[test]
    fn los_colores_se_leen_y_se_escriben_como_hex() {
        assert_eq!(color_de_texto("#FF885E"), Some(0xff885e));
        assert_eq!(color_de_texto("ff885e"), Some(0xff885e));
        assert_eq!(texto_de_color(0x2a9ef1), "#2A9EF1");
        // Caso negativo: lo que no es un color no se inventa.
        assert_eq!(color_de_texto("#FFF"), None);
        assert_eq!(color_de_texto("rojo!!"), None);
    }

    #[test]
    fn el_formato_es_el_del_manual_y_sin_campos_vacios() {
        let a = Avatar::emoji("🏠", Some(("#FF885E", "#FF516A")), 7);
        assert_eq!(
            a.a_valor(),
            serde_json::json!({"tipo":"emoji","emoji":"🏠","color":"#FF885E","color2":"#FF516A","cuando":7})
        );
        let liso = Avatar::iniciales(Some(("#72D5FD", "#72D5FD")), 8);
        assert_eq!(
            liso.a_valor(),
            serde_json::json!({"tipo":"iniciales","color":"#72D5FD","cuando":8})
        );
        assert_eq!(liso.fondo(), Some((0x72d5fd, 0x72d5fd)));
        let siempre = Avatar::iniciales(None, 9);
        assert_eq!(
            siempre.a_valor(),
            serde_json::json!({"tipo":"iniciales","cuando":9})
        );
        assert!(siempre.es_lo_de_siempre());
        assert!(!a.es_lo_de_siempre());
        assert!(Avatar::imagen("avatar-1.png", 1).es_imagen());
    }

    #[test]
    fn un_tipo_que_no_se_conoce_se_pinta_como_siempre_y_se_conserva() {
        let mut f = Ficha::nueva("Casa", 1, "PC01");
        let raro = serde_json::json!({"tipo":"animado","gif":"x.gif","cuando":3});
        f.resto.insert(CLAVE.into(), raro.clone());
        let a = de_ficha(&f).expect("se lee");
        assert_eq!(a.tipo, "animado");
        assert!(a.es_lo_de_siempre());
        // Lo que se guarda es lo crudo, con su `gif`.
        assert_eq!(valor_de(&f), Some(&raro));
        // Un emoji vacio tampoco es un emoji.
        f.resto.insert(
            CLAVE.into(),
            serde_json::json!({"tipo":"emoji","emoji":" "}),
        );
        assert!(de_ficha(&f).unwrap().el_emoji().is_none());
        // Y algo que no es un objeto no es una foto.
        f.resto.insert(CLAVE.into(), serde_json::json!("hola"));
        assert!(de_ficha(&f).is_none());
    }

    #[test]
    fn poner_una_imagen_la_deja_en_la_ficha_y_quita_las_viejas_y_el_logo() {
        let r = raiz("poner");
        let f = Ficha::nueva("Casa", 1, "PC01");
        Indice {
            proyectos: vec![f.clone()],
            ..Default::default()
        }
        .guardar(&r)
        .unwrap();
        let carpeta = almacen::carpeta(&r, &f.id);
        std::fs::create_dir_all(&carpeta).unwrap();
        std::fs::write(carpeta.join(LOGO_DE_ANTES), b"viejo").unwrap();
        // Sin `avatar`, el logo de antes es su foto.
        let leida = de_ficha_o_logo(&r, &f).unwrap();
        assert_eq!(leida.ruta.as_deref(), Some(LOGO_DE_ANTES));
        assert_eq!(
            ruta_de_la_imagen(&r, &f.id, &leida),
            Some(carpeta.join(LOGO_DE_ANTES))
        );

        let uno = guardar_imagen(&r, &f.id, b"png 1", 100).unwrap();
        poner(&r, &f.id, &Avatar::imagen(&uno, 100)).unwrap();
        let dos = guardar_imagen(&r, &f.id, b"png 2", 200).unwrap();
        poner(&r, &f.id, &Avatar::imagen(&dos, 200)).unwrap();
        assert!(!carpeta.join(LOGO_DE_ANTES).exists(), "el logo se va");
        assert!(!carpeta.join(&uno).exists(), "la vieja se va");
        assert_eq!(std::fs::read(carpeta.join(&dos)).unwrap(), b"png 2");
        let ficha = Indice::leer(&r).buscar(&f.id).cloned().unwrap();
        assert_eq!(
            de_ficha(&ficha).unwrap().ruta.as_deref(),
            Some("avatar-200.png")
        );

        // Volver a lo de siempre se escribe y se lleva la imagen.
        poner(&r, &f.id, &Avatar::iniciales(None, 300)).unwrap();
        assert!(!carpeta.join(&dos).exists());
        let ficha = Indice::leer(&r).buscar(&f.id).cloned().unwrap();
        assert!(de_ficha_o_logo(&r, &ficha).unwrap().es_lo_de_siempre());

        // Caso negativo: un proyecto que no esta no se inventa.
        assert!(poner(&r, "no-esta", &Avatar::iniciales(None, 1)).is_err());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn solo_se_reconocen_las_imagenes_de_foto_nombradas_por_su_hora() {
        assert!(es_imagen_de_avatar("avatar-1761400000000.png"));
        assert!(!es_imagen_de_avatar("avatar-.png"));
        assert!(!es_imagen_de_avatar("avatar-casa.png"));
        assert!(!es_imagen_de_avatar("captura.png"));
    }
}
