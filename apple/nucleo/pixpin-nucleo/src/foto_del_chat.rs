//! **La foto del chat** del lado del Mac (Windows 22bf4dc, bc71b4f y
//! 838a37f, 25-oct-2026): lo que se ve en el circulo de cada chat (una
//! imagen, un emoji sobre un degradado o las iniciales sobre el color
//! elegido) y la hoja para elegirla.
//!
//! Es `ventana_chat/foto_del_chat.rs` y `ventana_chat/logo.rs` de Windows
//! sin pintar nada, sobre los mismos crates: `pixpin_proyecto::avatar` (el
//! campo `avatar` de la ficha que viaja, la imagen `<proyecto>/avatar-<ms>.png`
//! y la limpieza de las viejas) y `pixpin_ui::foto_del_chat` (los doce
//! fondos, los emojis con su buscador y el recorte que se mueve y se acerca).
//! Asi el Mac guarda la foto **exactamente donde y como Windows**: PNG
//! cuadrado, ya recortado, de 256 px de lado como mucho, con un nombre nuevo
//! cada vez; y pinta igual la que llegue de Windows o de Android.
//!
//! Swift pinta el circulo ([`FotoDelChat`] va en cada [`FichaChat`]) y la
//! hoja, y llama aqui para todo lo demas.

use std::path::{Path, PathBuf};

use pixpin_codec::ImagenRgba;
use pixpin_proyecto::almacen::{Ficha, Indice};
use pixpin_proyecto::avatar::{self, Avatar};
use pixpin_store::ajustes::PreferenciaIdioma;
use pixpin_store::idioma::{Catalogo, resolver_idioma};
use pixpin_ui::foto_del_chat::{self as ui, EMOJIS, FONDOS, Recorte};

use crate::{ErrorNucleo, Nucleo};

/// Lo mas grande que se trabaja de una foto elegida (`LADO_DE_TRABAJO` de
/// Windows): de sobra para acercar ocho veces sobre 256.
pub const LADO_DE_TRABAJO: u32 = 2048;

/// **La foto de un chat, para pintarla** (`logo::pintar` de Windows).
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct FotoDelChat {
    /// `imagen`, `emoji`, `iniciales` o lo que traiga otro aparato (que se
    /// pinta como `iniciales`).
    pub tipo: String,
    /// Solo con `emoji`, y no vacio.
    pub emoji: Option<String>,
    /// El degradado del circulo de arriba (`arriba`) abajo (`abajo`), en
    /// `0xRRGGBB`. Sin el, el color de siempre del chat.
    pub arriba: Option<u32>,
    pub abajo: Option<u32>,
    /// Solo con `imagen`: el fichero en este Mac. Puede no estar aun (no ha
    /// llegado por la sincronizacion): mientras, se pinta como siempre.
    pub imagen: Option<String>,
    /// Cuando se puso (ms). Informativo.
    pub cuando: i64,
}

impl FotoDelChat {
    /// La de una ficha (o su `logo.png` de antes), si tiene una que no sea
    /// la de siempre.
    pub fn de_ficha(raiz: &Path, f: &Ficha) -> Option<FotoDelChat> {
        let a = avatar::de_ficha_o_logo(raiz, f)?;
        if a.es_lo_de_siempre() {
            return None;
        }
        let fondo = a.fondo();
        Some(FotoDelChat {
            tipo: a.tipo.clone(),
            emoji: a.el_emoji().map(str::to_string),
            arriba: fondo.map(|f| f.0),
            abajo: fondo.map(|f| f.1),
            imagen: avatar::ruta_de_la_imagen(raiz, &f.id, &a)
                .map(|r| r.to_string_lossy().into_owned()),
            cuando: a.cuando,
        })
    }
}

/// Un fondo de la paleta (`pixpin_ui::foto_del_chat::FONDOS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Record)]
pub struct FondoDeLaFoto {
    pub arriba: u32,
    pub abajo: u32,
}

/// Los doce fondos que se ofrecen, en su orden.
#[uniffi::export]
pub fn fondos_de_la_foto() -> Vec<FondoDeLaFoto> {
    FONDOS
        .iter()
        .map(|f| FondoDeLaFoto {
            arriba: f.arriba,
            abajo: f.abajo,
        })
        .collect()
}

/// **Los emojis que encuentra `consulta`** en la rejilla (sin nada, todos):
/// en castellano o en ingles, sin tildes y por el principio de las palabras.
#[uniffi::export]
pub fn emojis_de_la_foto(consulta: String) -> Vec<String> {
    ui::buscar(&consulta)
        .into_iter()
        .map(|i| EMOJIS[i].0.to_string())
        .collect()
}

// --- El recorte ---------------------------------------------------------------

/// Que cuadrado de la foto preparada entra en el circulo
/// (`pixpin_ui::foto_del_chat::Recorte`).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct RecorteDeLaFoto {
    /// 1 = el cuadrado mas grande que cabe; hasta 8.
    pub aumento: f32,
    /// El centro del cuadrado, en pixeles de la foto.
    pub cx: f32,
    pub cy: f32,
}

impl From<Recorte> for RecorteDeLaFoto {
    fn from(r: Recorte) -> Self {
        RecorteDeLaFoto {
            aumento: r.aumento,
            cx: r.cx,
            cy: r.cy,
        }
    }
}

impl From<RecorteDeLaFoto> for Recorte {
    fn from(r: RecorteDeLaFoto) -> Self {
        Recorte {
            aumento: r.aumento,
            cx: r.cx,
            cy: r.cy,
        }
    }
}

/// Donde se pinta la foto ENTERA para que su cuadrado llene la caja.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct DestinoDeLaFoto {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

/// El cuadrado mas grande, en el centro: como se abre una foto.
#[uniffi::export]
pub fn recorte_centrado(ancho: u32, alto: u32) -> RecorteDeLaFoto {
    Recorte::centrado(ancho, alto).into()
}

/// Arrastrar la foto `(dx, dy)` puntos en un circulo de `lado`: la foto va
/// con el raton y el cuadrado nunca se sale de ella.
#[uniffi::export]
pub fn recorte_movido(
    recorte: RecorteDeLaFoto,
    ancho: u32,
    alto: u32,
    lado: f32,
    dx: f32,
    dy: f32,
) -> RecorteDeLaFoto {
    let mut r = Recorte::from(recorte);
    r.mover(ancho, alto, lado, dx, dy);
    r.into()
}

/// Acercar (`factor` > 1) o alejar hacia `(px, py)`, en fraccion del
/// circulo (0..1): lo que hay bajo el raton se queda bajo el.
#[uniffi::export]
pub fn recorte_acercado(
    recorte: RecorteDeLaFoto,
    ancho: u32,
    alto: u32,
    factor: f32,
    px: f32,
    py: f32,
) -> RecorteDeLaFoto {
    let mut r = Recorte::from(recorte);
    r.acercar(ancho, alto, factor, (px, py));
    r.into()
}

/// Un giro de rueda (`delta` en unidades de 120 de Windows; positivo acerca)
/// hacia `(px, py)`.
#[uniffi::export]
pub fn recorte_con_rueda(
    recorte: RecorteDeLaFoto,
    ancho: u32,
    alto: u32,
    delta: i32,
    px: f32,
    py: f32,
) -> RecorteDeLaFoto {
    let mut r = Recorte::from(recorte);
    r.rueda(ancho, alto, delta, (px, py));
    r.into()
}

/// Donde pintar la foto entera para que su cuadrado caiga en un circulo de
/// `lado` con su esquina en el cero.
#[uniffi::export]
pub fn destino_del_recorte(
    recorte: RecorteDeLaFoto,
    ancho: u32,
    alto: u32,
    lado: f32,
) -> DestinoDeLaFoto {
    let (x, y, w, h) = Recorte::from(recorte).destino((0.0, 0.0, lado), ancho, alto);
    DestinoDeLaFoto {
        x,
        y,
        ancho: w,
        alto: h,
    }
}

// --- La imagen --------------------------------------------------------------

/// Una foto elegida, lista para recortarla: un PNG entero (reducido a
/// [`LADO_DE_TRABAJO`]) y sus medidas, que son las del recorte.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FotoPreparada {
    pub ruta: String,
    pub ancho: u32,
    pub alto: u32,
}

/// Una foto grande, reducida sin deformarla (`reducida` de Windows).
pub fn reducida(imagen: ImagenRgba) -> ImagenRgba {
    let mayor = imagen.ancho.max(imagen.alto);
    if mayor <= LADO_DE_TRABAJO {
        return imagen;
    }
    let k = LADO_DE_TRABAJO as f32 / mayor as f32;
    let (w, h) = (
        ((imagen.ancho as f32 * k).round() as u32).max(1),
        ((imagen.alto as f32 * k).round() as u32).max(1),
    );
    pixpin_codec::redimensionar(imagen.clone(), w, h).unwrap_or(imagen)
}

/// El cuadrado `(x, y, lado)` de una imagen (`logo::recortar` de Windows).
/// `None` si se sale de ella o la imagen no es coherente.
pub fn recortar(imagen: &ImagenRgba, (x, y, lado): (u32, u32, u32)) -> Option<ImagenRgba> {
    if imagen.ancho == 0
        || imagen.alto == 0
        || lado == 0
        || imagen.pixeles.len() != imagen.ancho as usize * imagen.alto as usize * 4
        || x + lado > imagen.ancho
        || y + lado > imagen.alto
    {
        return None;
    }
    let (fila, trozo) = (imagen.ancho as usize * 4, lado as usize * 4);
    let mut pixeles = Vec::with_capacity(trozo * lado as usize);
    for f in y..y + lado {
        let inicio = f as usize * fila + x as usize * 4;
        pixeles.extend_from_slice(&imagen.pixeles[inicio..inicio + trozo]);
    }
    Some(ImagenRgba {
        ancho: lado,
        alto: lado,
        pixeles,
    })
}

/// **Guarda como foto del chat `id`** el cuadrado ya recortado: reducido a
/// 256 si es mayor (una pequena no se agranda), en PNG, como
/// `<proyecto>/avatar-<cuando>.png`, y apuntado en su ficha. La de antes se
/// borra. Es `logo::guardar_imagen` de Windows.
pub fn guardar_cuadrada(
    raiz: &Path,
    id: &str,
    cuadrada: ImagenRgba,
    cuando: i64,
) -> Result<String, ErrorNucleo> {
    let cuadrada = if cuadrada.ancho > avatar::LADO {
        pixpin_codec::redimensionar(cuadrada, avatar::LADO, avatar::LADO).map_err(error_de_imagen)?
    } else {
        cuadrada
    };
    let png = pixpin_codec::codificar_png(&cuadrada).map_err(error_de_imagen)?;
    let rel = avatar::guardar_imagen(raiz, id, &png, cuando)?;
    avatar::poner(raiz, id, &Avatar::imagen(&rel, cuando))?;
    Ok(rel)
}

fn error_de_imagen(e: pixpin_codec::ErrorCodec) -> ErrorNucleo {
    ErrorNucleo::Argumento {
        detalle: format!("imagen: {e}"),
    }
}

/// Los colores de un fondo como se guardan (`#RRGGBB`).
fn fondo_en_texto(fondo: Option<FondoDeLaFoto>) -> Option<(String, String)> {
    fondo.map(|f| (avatar::texto_de_color(f.arriba), avatar::texto_de_color(f.abajo)))
}

/// Donde se deja la foto preparada: una al lado de la otra en la carpeta
/// temporal, con un nombre nuevo cada vez.
fn ruta_preparada(cuando: i64) -> PathBuf {
    std::env::temp_dir().join(format!("pixpin-foto-del-chat-{}-{cuando}.png", std::process::id()))
}

#[uniffi::export]
impl Nucleo {
    /// **Prepara una foto elegida** (el fichero de una imagen o el PNG de una
    /// pegada): la lee derecha, la reduce a [`LADO_DE_TRABAJO`] y la deja en
    /// un PNG temporal. Sus medidas son las del recorte.
    pub fn preparar_foto_del_chat(&self, origen: String) -> Result<FotoPreparada, ErrorNucleo> {
        let imagen = pixpin_codec::cargar(Path::new(&origen)).map_err(error_de_imagen)?;
        if imagen.ancho == 0 || imagen.alto == 0 {
            return Err(ErrorNucleo::Argumento {
                detalle: "imagen vacia".into(),
            });
        }
        let imagen = reducida(imagen);
        let ruta = ruta_preparada(crate::chats::hora_unica());
        let png = pixpin_codec::codificar_png(&imagen).map_err(error_de_imagen)?;
        pixpin_sincro::disco::escribir_atomico(&ruta, &png)?;
        Ok(FotoPreparada {
            ruta: ruta.to_string_lossy().into_owned(),
            ancho: imagen.ancho,
            alto: imagen.alto,
        })
    }

    /// **Guarda la foto preparada** recortada por `recorte` como foto del
    /// chat (Guardar en la hoja con una imagen nueva). Borra la preparada.
    pub fn poner_foto_imagen(
        &self,
        chat: String,
        preparada: String,
        recorte: RecorteDeLaFoto,
    ) -> Result<(), ErrorNucleo> {
        let imagen = pixpin_codec::cargar(Path::new(&preparada)).map_err(error_de_imagen)?;
        let cuadrado = Recorte::from(recorte).en_pixeles(imagen.ancho, imagen.alto);
        let c = recortar(&imagen, cuadrado).ok_or_else(|| ErrorNucleo::Argumento {
            detalle: "recorte fuera de la foto".into(),
        })?;
        guardar_cuadrada(self.ruta(), &chat, c, crate::ahora_ms())?;
        let _ = std::fs::remove_file(&preparada);
        Ok(())
    }

    /// Un emoji sobre `fondo` como foto del chat.
    pub fn poner_foto_emoji(
        &self,
        chat: String,
        emoji: String,
        fondo: Option<FondoDeLaFoto>,
    ) -> Result<(), ErrorNucleo> {
        let textos = fondo_en_texto(fondo);
        let fondo = textos.as_ref().map(|(a, b)| (a.as_str(), b.as_str()));
        avatar::poner(
            self.ruta(),
            &chat,
            &Avatar::emoji(&emoji, fondo, crate::ahora_ms()),
        )?;
        Ok(())
    }

    /// Las iniciales sobre `fondo`; sin fondo, **«Como siempre»** (se
    /// escribe `{"tipo":"iniciales"}`, no se borra: asi viaja).
    pub fn poner_foto_iniciales(
        &self,
        chat: String,
        fondo: Option<FondoDeLaFoto>,
    ) -> Result<(), ErrorNucleo> {
        let textos = fondo_en_texto(fondo);
        let fondo = textos.as_ref().map(|(a, b)| (a.as_str(), b.as_str()));
        avatar::poner(
            self.ruta(),
            &chat,
            &Avatar::iniciales(fondo, crate::ahora_ms()),
        )?;
        Ok(())
    }

    /// La foto de un chat como esta ahora (la hoja abre con ella).
    pub fn foto_del_chat(&self, chat: String) -> Result<Option<FotoDelChat>, ErrorNucleo> {
        let indice = Indice::leer_si_esta(self.ruta())?;
        Ok(indice
            .buscar(&chat)
            .and_then(|f| FotoDelChat::de_ficha(self.ruta(), f)))
    }

    /// El lugar en [`fondos_de_la_foto`] del fondo que se propone a un chat
    /// (uno fijo por su codigo): un emoji sin fondo elegido se lo lleva.
    pub fn fondo_propuesto_del_chat(&self, chat: String) -> Result<u32, ErrorNucleo> {
        let indice = Indice::leer_si_esta(self.ruta())?;
        let f = indice.buscar(&chat).ok_or_else(|| ErrorNucleo::NoExiste {
            que: format!("chat {chat}"),
        })?;
        Ok(ui::fondo_propuesto(&f.codigo_unico()) as u32)
    }
}

/// Los textos de la hoja (`foto-chat-*` de `pixpin-store`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TextosDeLaFoto {
    pub menu: String,
    pub titulo: String,
    pub imagen: String,
    pub pegar: String,
    pub quitar: String,
    pub buscar: String,
    pub sin_emojis: String,
    pub fondo: String,
    pub recorte: String,
    pub guardar: String,
    pub cancelar: String,
    pub sin_imagen: String,
    pub error: String,
}

/// **Los textos de la foto del chat** para `locale`, como `textos_de_funciones`.
#[uniffi::export]
pub fn textos_de_la_foto(locale: String) -> TextosDeLaFoto {
    let t = Catalogo::nuevo(resolver_idioma(&locale, PreferenciaIdioma::Sistema));
    TextosDeLaFoto {
        menu: t.t("foto-chat-menu"),
        titulo: t.t("foto-chat-titulo"),
        imagen: t.t("foto-chat-imagen"),
        pegar: t.t("foto-chat-pegar"),
        quitar: t.t("foto-chat-quitar"),
        buscar: t.t("foto-chat-buscar"),
        sin_emojis: t.t("foto-chat-sin-emojis"),
        fondo: t.t("foto-chat-fondo"),
        recorte: t.t("foto-chat-recorte"),
        guardar: t.t("foto-chat-guardar"),
        cancelar: t.t("foto-chat-cancelar"),
        sin_imagen: t.t("foto-chat-sin-imagen"),
        error: t.t("foto-chat-error"),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::Arc;

    use pixpin_proyecto::almacen;
    use pixpin_proyecto::vista::{self, DiscoPc};
    use pixpin_sincro::disco::Disco;
    use pixpin_sincro::kotlin;

    fn montar(etiqueta: &str) -> (PathBuf, Arc<Nucleo>, Ficha) {
        let r = std::env::temp_dir().join(format!(
            "pixpin-nucleo-foto-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        let n = Nucleo::abrir(r.to_string_lossy().into_owned(), "Mac de prueba".into()).unwrap();
        let f = n.crear_chat("Obra".into()).unwrap();
        let ficha = Indice::leer(n.ruta()).buscar(&f.id).cloned().unwrap();
        (r, n, ficha)
    }

    /// Una foto de `w x h` con cada pixel de su columna y su fila.
    fn foto(w: u32, h: u32) -> ImagenRgba {
        let mut pixeles = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                pixeles.extend_from_slice(&[(x % 256) as u8, (y % 256) as u8, 7, 255]);
            }
        }
        ImagenRgba {
            ancho: w,
            alto: h,
            pixeles,
        }
    }

    #[test]
    fn la_imagen_se_guarda_como_la_guarda_windows_y_sale_portatil() {
        let (r, n, f) = montar("como-windows");
        let raiz = n.ruta().to_path_buf();
        // Una foto de 1200 x 800 que se elige desde un fichero.
        let origen = r.join("elegida.png");
        std::fs::write(&origen, pixpin_codec::codificar_png(&foto(1200, 800)).unwrap()).unwrap();
        let p = n
            .preparar_foto_del_chat(origen.to_string_lossy().into_owned())
            .unwrap();
        assert_eq!((p.ancho, p.alto), (1200, 800), "una de menos de 2048 no se toca");
        let recorte = recorte_centrado(p.ancho, p.alto);
        n.poner_foto_imagen(f.id.clone(), p.ruta.clone(), recorte)
            .unwrap();
        assert!(!Path::new(&p.ruta).exists(), "la preparada se borra");

        // En la ficha: `{"tipo":"imagen","ruta":"avatar-<ms>.png","cuando":<ms>}`,
        // como `logo::guardar_imagen` de Windows.
        let ficha = Indice::leer(&raiz).buscar(&f.id).cloned().unwrap();
        let v = avatar::valor_de(&ficha).unwrap().clone();
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 3, "sin campos de mas: {v}");
        assert_eq!(v["tipo"], "imagen");
        let cuando = v["cuando"].as_i64().unwrap();
        assert!(cuando > 0);
        let rel = v["ruta"].as_str().unwrap();
        assert_eq!(rel, avatar::nombre_de_imagen(cuando));

        // El fichero: PNG cuadrado de 256, en la carpeta del proyecto.
        let fichero = almacen::carpeta(&raiz, &f.id).join(rel);
        let bytes = std::fs::read(&fichero).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let guardada = pixpin_codec::cargar(&fichero).unwrap();
        assert_eq!((guardada.ancho, guardada.alto), (avatar::LADO, avatar::LADO));

        // Y por la red sale con ruta portatil, que lleva el fichero.
        let chat = vista::chat_de_ficha(&raiz, &f.id).unwrap();
        let disco = DiscoPc::nuevo(&raiz);
        let p = disco.proyecto_portatil(&chat).unwrap().expect("su proyecto");
        let a = p.como_objeto().unwrap().obtener("avatar").expect("con su foto");
        let portatil = kotlin::cadena(a, "ruta").unwrap().to_string();
        assert!(
            portatil.starts_with("pixpin:files/") && portatil.ends_with(rel),
            "{portatil}"
        );
        assert!(
            disco.alcance(&chat).unwrap().iter().any(|(r, _)| r.ends_with(rel)),
            "la imagen viaja con el proyecto"
        );

        // Lo que pinta Swift: la imagen en este Mac.
        let pinta = n.foto_del_chat(f.id.clone()).unwrap().unwrap();
        assert_eq!(pinta.tipo, "imagen");
        assert_eq!(pinta.imagen.as_deref(), Some(fichero.to_string_lossy().as_ref()));
        let lista = n.chats().unwrap();
        let en_lista = lista.iter().find(|c| c.id == f.id).unwrap();
        assert_eq!(en_lista.foto.as_ref(), Some(&pinta));

        // Otra foto: un nombre nuevo, y la vieja se va.
        std::thread::sleep(std::time::Duration::from_millis(2));
        let p = n
            .preparar_foto_del_chat(origen.to_string_lossy().into_owned())
            .unwrap();
        n.poner_foto_imagen(f.id.clone(), p.ruta, recorte_centrado(p.ancho, p.alto))
            .unwrap();
        assert!(!fichero.exists(), "la vieja se borra");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn una_foto_grande_se_reduce_y_una_pequena_no_se_agranda() {
        let r = reducida(foto(4096, 1024));
        assert_eq!((r.ancho, r.alto), (LADO_DE_TRABAJO, 512));
        let r = reducida(foto(300, 200));
        assert_eq!((r.ancho, r.alto), (300, 200));

        let (raiz, n, f) = montar("pequena");
        let rel = guardar_cuadrada(n.ruta(), &f.id, foto(100, 100), 5).unwrap();
        assert_eq!(rel, "avatar-5.png");
        let g = pixpin_codec::cargar(&almacen::carpeta(n.ruta(), &f.id).join(rel)).unwrap();
        assert_eq!((g.ancho, g.alto), (100, 100), "una pequena no se agranda");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn recortar_toma_su_cuadrado_y_nunca_fuera() {
        let c = recortar(&foto(6, 4), (2, 1, 3)).unwrap();
        assert_eq!((c.ancho, c.alto), (3, 3));
        assert_eq!(&c.pixeles[0..2], &[2, 1]);
        // Caso negativo: un cuadrado que se sale o vacio no se recorta.
        assert!(recortar(&foto(6, 4), (4, 0, 3)).is_none());
        assert!(recortar(&foto(6, 4), (0, 0, 0)).is_none());
    }

    #[test]
    fn emoji_iniciales_y_como_siempre_quedan_en_la_ficha_como_en_windows() {
        let (raiz, n, f) = montar("emoji");
        let fondos = fondos_de_la_foto();
        assert_eq!(fondos.len(), 12);
        n.poner_foto_emoji(f.id.clone(), "🏗️".into(), Some(fondos[1]))
            .unwrap();
        let ficha = Indice::leer(n.ruta()).buscar(&f.id).cloned().unwrap();
        let v = avatar::valor_de(&ficha).unwrap();
        assert_eq!(v["tipo"], "emoji");
        assert_eq!(v["emoji"], "🏗️");
        assert_eq!(v["color"], "#FFCD6A");
        assert_eq!(v["color2"], "#FFA85C");
        let pinta = n.foto_del_chat(f.id.clone()).unwrap().unwrap();
        assert_eq!(pinta.emoji.as_deref(), Some("🏗️"));
        assert_eq!((pinta.arriba, pinta.abajo), (Some(0xFFCD6A), Some(0xFFA85C)));

        // Un fondo liso (los dos iguales) se guarda sin `color2`.
        let liso = FondoDeLaFoto {
            arriba: 0x72D5FD,
            abajo: 0x72D5FD,
        };
        n.poner_foto_iniciales(f.id.clone(), Some(liso)).unwrap();
        let ficha = Indice::leer(n.ruta()).buscar(&f.id).cloned().unwrap();
        let v = avatar::valor_de(&ficha).unwrap();
        assert_eq!(v["tipo"], "iniciales");
        assert!(v.get("color2").is_none());

        // «Como siempre» se escribe y se pinta como siempre (sin foto).
        n.poner_foto_iniciales(f.id.clone(), None).unwrap();
        let ficha = Indice::leer(n.ruta()).buscar(&f.id).cloned().unwrap();
        let v = avatar::valor_de(&ficha).unwrap();
        assert_eq!(v["tipo"], "iniciales");
        assert!(v.get("color").is_none());
        assert_eq!(n.foto_del_chat(f.id.clone()).unwrap(), None);

        // Caso negativo: un chat que no esta no se inventa.
        assert!(n.poner_foto_iniciales("no-esta".into(), None).is_err());
        assert!(n.fondo_propuesto_del_chat("no-esta".into()).is_err());
        assert!(n.fondo_propuesto_del_chat(f.id.clone()).unwrap() < 12);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_que_llega_de_otro_aparato_se_pinta_y_un_tipo_raro_como_siempre() {
        let (raiz, n, f) = montar("llega");
        let mut indice = Indice::leer(n.ruta());
        let ficha = indice.proyectos.iter_mut().find(|x| x.id == f.id).unwrap();
        ficha.resto.insert(
            avatar::CLAVE.into(),
            serde_json::json!({"tipo":"imagen","ruta":"pixpin:files/guardados/pc/OBRA/avatar-9.png","cuando":9}),
        );
        indice.guardar(n.ruta()).unwrap();
        let pinta = n.foto_del_chat(f.id.clone()).unwrap().unwrap();
        assert_eq!(pinta.tipo, "imagen");
        // Su ruta en este Mac, aunque aun no haya llegado el fichero.
        assert!(pinta.imagen.unwrap().ends_with("avatar-9.png"));

        let mut indice = Indice::leer(n.ruta());
        let ficha = indice.proyectos.iter_mut().find(|x| x.id == f.id).unwrap();
        ficha.resto.insert(
            avatar::CLAVE.into(),
            serde_json::json!({"tipo":"animado","gif":"x","cuando":3}),
        );
        indice.guardar(n.ruta()).unwrap();
        assert_eq!(n.foto_del_chat(f.id.clone()).unwrap(), None);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_recorte_y_los_emojis_son_los_de_pixpin_ui() {
        let r = recorte_centrado(400, 300);
        assert_eq!(Recorte::from(r).en_pixeles(400, 300), (50, 0, 300));
        let r = recorte_movido(r, 400, 300, 150.0, 1000.0, 0.0);
        assert_eq!(Recorte::from(r).en_pixeles(400, 300), (0, 0, 300));
        let r = recorte_acercado(recorte_centrado(1000, 1000), 1000, 1000, 2.0, 0.5, 0.5);
        assert_eq!(Recorte::from(r).en_pixeles(1000, 1000), (250, 250, 500));
        let d = destino_del_recorte(recorte_centrado(400, 300), 400, 300, 150.0);
        assert_eq!((d.x, d.y, d.ancho, d.alto), (-25.0, 0.0, 200.0, 150.0));
        assert!(emojis_de_la_foto("casa".into()).contains(&"🏠".to_string()));
        assert_eq!(emojis_de_la_foto(String::new()).len(), EMOJIS.len());
        assert!(emojis_de_la_foto("zzzz".into()).is_empty());
        let t = textos_de_la_foto("es-ES".into());
        assert_eq!(t.menu, "Cambiar foto del chat…");
        assert_eq!(textos_de_la_foto("en-US".into()).guardar, "Save");
    }
}
