//! **La nota rica del chat** (Windows 974d5ba, `ventana_chat/nota_rica.rs`):
//! una `NOTA` cuyo `texto` es Markdown se ensena entera en su burbuja, con
//! su texto con formato y sus fotos `![…](pixpin:files/…)` en su sitio; las
//! seguidas, en album (filas de dos o tres), como un mensaje de Telegram.
//! El formato no cambia (ver `docs/nota-rica-del-chat.md` de Windows).
//!
//! Los bloques y el reparto del album son los de `pixpin_ui::nota_rica`, el
//! mismo crate que Windows: asi el Mac ensena exactamente lo mismo. Aqui se
//! les pone lo que solo sabe este aparato —el fichero de cada foto (la misma
//! resolucion que los adjuntos, [`Nucleo::ruta_absoluta`]) y su proporcion
//! (solo su cabecera)— y Swift los pinta.
//!
//! Rendimiento: una llamada por nota (todos sus bloques y fotos juntos); la
//! ruta de cada enlace se resuelve una vez y la medida de cada foto se lee
//! una vez por fecha del fichero, como en Windows (`RUTAS`, `MEDIDAS`).
//!
//! Tambien lo de escribirla: una foto pegada con texto ya escrito va a
//! `notas/` del proyecto y su renglon `![](notas/…)` a lo escrito; al
//! enviar, esas rutas pasan a la portatil ([`Nucleo::escribir_nota`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use pixpin_proyecto::{almacen, vista};
use pixpin_ui::nota_rica as nr;

use crate::chats::{hora_unica, validar_chat};
use crate::{ErrorNucleo, Nucleo};

/// Que es un bloque de texto: decide su letra, su sangria y su marca.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ClaseDeBloque {
    Parrafo,
    /// `#` a `######`.
    Titulo { nivel: u8 },
    /// `- x`, con su nivel de sangria (cero el de fuera).
    Vineta { nivel: u8 },
    /// `1. x`, con su numero tal cual se escribio.
    Numero { nivel: u8, numero: String },
    /// `- [ ] x` / `- [x] x`.
    Casilla { nivel: u8, hecha: bool },
    /// `> x`.
    Cita,
}

/// Un trozo con estilo propio, en unidades UTF-16 (las de `NSString`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct MarcaDeNota {
    pub inicio: u32,
    pub largo: u32,
    pub negrita: bool,
    pub cursiva: bool,
    pub codigo: bool,
    pub tachado: bool,
}

/// Un enlace del texto (`[texto](url)`): donde esta (UTF-16) y adonde lleva.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct EnlaceDeNota {
    pub inicio: u32,
    pub largo: u32,
    pub url: String,
}

/// Una foto de la nota, ya con lo de este aparato.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FotoDeNota {
    /// Lo que dice de ella (sin el ancho del editor, `|300`).
    pub alt: String,
    /// Su enlace tal cual (`pixpin:files/…`).
    pub enlace: String,
    /// Su fichero en este aparato, solo si esta. Sin el, un recuadro con el
    /// icono de imagen (nunca la ruta como texto).
    pub ruta: Option<String>,
    /// Ancho / alto, o cero si no se sabe (el album pone un 4:3).
    pub proporcion: f32,
    /// Cual es de la nota, en orden (`nota_rica::imagenes`).
    pub indice: u32,
}

/// Un bloque de la nota, de arriba abajo.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BloqueDeNota {
    Texto {
        clase: ClaseDeBloque,
        texto: String,
        marcas: Vec<MarcaDeNota>,
        enlaces: Vec<EnlaceDeNota>,
    },
    /// Un bloque de codigo (o una tabla, que en monoespaciada se lee).
    Codigo { texto: String },
    /// `---`.
    Regla,
    /// Una o varias fotos seguidas: un album.
    Album { fotos: Vec<FotoDeNota> },
    /// Un fichero de la nota que no es una foto (un PDF, un audio): se
    /// nombra con 📎 y no se pinta.
    Adjunto { nombre: String },
}

/// Una nota con algo que ensenar con formato.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct NotaRica {
    pub bloques: Vec<BloqueDeNota>,
}

/// Una caja del album, desde su esquina de arriba a la izquierda.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct CajaDeAlbum {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

/// Donde va cada foto de un album y lo que mide entero.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AlbumDeNota {
    pub cajas: Vec<CajaDeAlbum>,
    pub alto: f32,
}

/// **El album** (`nota_rica::album`): las cajas de las fotos dadas sus
/// proporciones (cero o menos si no se sabe: un 4:3), el ancho que hay, el
/// tope de una foto sola y el de cada fila, y el hueco entre fotos. Una
/// sola guarda su proporcion; en una fila de varias todas toman el mismo
/// alto y cada una se recorta por el centro.
#[uniffi::export]
pub fn album_de_nota(
    proporciones: Vec<f32>,
    ancho: f32,
    alto_solo: f32,
    alto_fila: f32,
    hueco: f32,
) -> AlbumDeNota {
    let (cajas, alto) = nr::album(&proporciones, ancho, alto_solo, alto_fila, hueco);
    AlbumDeNota {
        cajas: cajas
            .into_iter()
            .map(|c| CajaDeAlbum {
                x: c.x,
                y: c.y,
                ancho: c.ancho,
                alto: c.alto,
            })
            .collect(),
        alto,
    }
}

/// Si un fichero es una foto, por su extension (la regla de la nota rica:
/// pegar fotos con texto escrito las mete en lo escrito).
#[uniffi::export]
pub fn es_foto_de_nota(nombre: String) -> bool {
    nr::es_foto(&nombre)
}

/// Lo que hay que escribir con la rayita entre `antes` y `despues` para que
/// `renglones` queden cada uno en su renglon (`nota_rica::en_su_renglon`):
/// lo que hace la caja de escribir al pegar una foto con texto.
#[uniffi::export]
pub fn renglones_en_su_sitio(antes: String, despues: String, renglones: Vec<String>) -> String {
    nr::en_su_renglon(&antes, &despues, &renglones)
}

/// La nota rica de `texto` sin mirar el disco (las fotos sin fichero ni
/// proporcion), o `None` si es una nota llana: Swift decide con ella, al
/// pintar, si la burbuja va con formato, y la pinta ya mientras
/// [`Nucleo::nota_rica`] busca las fotos fuera del hilo principal.
#[uniffi::export]
pub fn nota_rica_sin_fotos(texto: String) -> Option<NotaRica> {
    bloques_de_nota(&texto).map(|bloques| NotaRica { bloques })
}

/// Los bloques de `texto` sin nada de este aparato (sin fichero ni
/// proporcion), o `None` si es una nota llana que se pinta como siempre.
pub fn bloques_de_nota(texto: &str) -> Option<Vec<BloqueDeNota>> {
    let texto = texto.trim();
    if texto.is_empty() {
        return None;
    }
    let bloques = nr::bloques(texto);
    if !nr::tiene_formato(&bloques, texto) {
        return None;
    }
    let mut foto = 0u32;
    Some(
        bloques
            .into_iter()
            .map(|b| match b {
                nr::Bloque::Texto(clase, l) => BloqueDeNota::Texto {
                    clase: clase_de(clase),
                    texto: l.texto,
                    marcas: l
                        .marcas
                        .iter()
                        .map(|m| MarcaDeNota {
                            inicio: m.inicio,
                            largo: m.largo,
                            negrita: m.negrita,
                            cursiva: m.cursiva,
                            codigo: m.codigo,
                            tachado: m.tachado,
                        })
                        .collect(),
                    enlaces: l
                        .enlaces
                        .into_iter()
                        .map(|e| EnlaceDeNota {
                            inicio: e.inicio,
                            largo: e.largo,
                            url: e.url,
                        })
                        .collect(),
                },
                nr::Bloque::Codigo(texto) => BloqueDeNota::Codigo { texto },
                nr::Bloque::Regla => BloqueDeNota::Regla,
                nr::Bloque::Imagenes(v) => BloqueDeNota::Album {
                    fotos: v
                        .into_iter()
                        .map(|i| {
                            foto += 1;
                            FotoDeNota {
                                alt: i.alt,
                                enlace: i.enlace,
                                ruta: None,
                                proporcion: 0.0,
                                indice: foto - 1,
                            }
                        })
                        .collect(),
                },
                // Su nombre, que es lo que se reconoce (el de Windows).
                nr::Bloque::Adjunto(i) => BloqueDeNota::Adjunto {
                    nombre: if i.alt.is_empty() {
                        i.enlace.rsplit('/').next().unwrap_or_default().to_string()
                    } else {
                        i.alt
                    },
                },
            })
            .collect(),
    )
}

fn clase_de(c: nr::Clase) -> ClaseDeBloque {
    match c {
        nr::Clase::Parrafo => ClaseDeBloque::Parrafo,
        nr::Clase::Titulo(nivel) => ClaseDeBloque::Titulo { nivel },
        nr::Clase::Vineta(nivel) => ClaseDeBloque::Vineta { nivel },
        nr::Clase::Numero(nivel, numero) => ClaseDeBloque::Numero { nivel, numero },
        nr::Clase::Casilla(nivel, hecha) => ClaseDeBloque::Casilla { nivel, hecha },
        nr::Clase::Cita => ClaseDeBloque::Cita,
    }
}

/// Cada enlace de foto ya resuelto a su ruta, este o no el fichero:
/// resolver una portatil lee el indice de proyectos (Windows, `RUTAS`).
type Rutas = HashMap<(PathBuf, String, String), Option<PathBuf>>;
fn rutas() -> &'static Mutex<Rutas> {
    static R: OnceLock<Mutex<Rutas>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Lo que mide cada foto, por fichero y con su fecha: se lee la cabecera
/// una vez y no en cada burbuja (Windows, `MEDIDAS`).
type Medidas = HashMap<PathBuf, (Option<SystemTime>, Option<(u32, u32)>)>;
fn medidas() -> &'static Mutex<Medidas> {
    static M: OnceLock<Mutex<Medidas>> = OnceLock::new();
    M.get_or_init(Default::default)
}

/// Ancho / alto de una foto, o cero si no se sabe.
fn proporcion(ruta: &Path) -> f32 {
    let fecha = std::fs::metadata(ruta).and_then(|m| m.modified()).ok();
    let mut m = medidas().lock().unwrap_or_else(|e| e.into_inner());
    let medida = match m.get(ruta) {
        Some((f, medida)) if *f == fecha => *medida,
        _ => {
            let medida = pixpin_codec::imagen::medidas(ruta).ok();
            m.insert(ruta.to_path_buf(), (fecha, medida));
            medida
        }
    };
    match medida {
        Some((w, h)) if w > 0 && h > 0 => w as f32 / h as f32,
        _ => 0.0,
    }
}

impl Nucleo {
    /// El fichero de este aparato al que lleva el enlace de una foto de la
    /// nota, solo si esta. La resolucion es la de los adjuntos.
    fn ruta_de_foto(&self, chat: &str, enlace: &str) -> Option<PathBuf> {
        let enlace = enlace.trim();
        let clave = (self.ruta().to_path_buf(), chat.to_string(), enlace.to_string());
        let r = {
            let mut rutas = rutas().lock().unwrap_or_else(|e| e.into_inner());
            match rutas.get(&clave) {
                Some(r) => r.clone(),
                None => {
                    let r = self.resolver(chat, enlace);
                    rutas.insert(clave, r.clone());
                    r
                }
            }
        };
        r.filter(|p| p.is_file())
    }
}

#[uniffi::export]
impl Nucleo {
    /// **La nota rica de `texto`** (una `NOTA` de `chat`): sus bloques con
    /// cada foto resuelta y medida, o `None` si es una nota llana (se pinta
    /// como siempre y no cambia ni un pixel). Una llamada por nota.
    pub fn nota_rica(&self, chat: String, texto: String) -> Result<Option<NotaRica>, ErrorNucleo> {
        validar_chat(&chat)?;
        let Some(mut bloques) = bloques_de_nota(&texto) else {
            return Ok(None);
        };
        for b in &mut bloques {
            if let BloqueDeNota::Album { fotos } = b {
                for f in fotos {
                    if let Some(r) = self.ruta_de_foto(&chat, &f.enlace) {
                        f.proporcion = proporcion(&r);
                        f.ruta = Some(r.display().to_string());
                    }
                }
            }
        }
        Ok(Some(NotaRica { bloques }))
    }

    /// **Una foto pegada con texto ya escrito va dentro de lo escrito**
    /// (`fotos_al_borrador` de Windows): se guarda en `notas/<ms>-<nombre>`
    /// del proyecto (donde guarda las suyas el editor de notas) y devuelve
    /// su renglon `![](notas/…)`, que Swift mete donde esta la rayita. Al
    /// enviar, [`Nucleo::escribir_nota`] lo pasa a la ruta portatil.
    pub fn foto_a_la_nota(
        &self,
        chat: String,
        nombre: String,
        datos: Vec<u8>,
    ) -> Result<String, ErrorNucleo> {
        validar_chat(&chat)?;
        let carpeta = almacen::carpeta(self.ruta(), &chat).join("notas");
        std::fs::create_dir_all(&carpeta)?;
        // Una imagen del portapapeles ya trae la hora en su nombre
        // (`pegado-<ms>.png`): con la de delante basta.
        let nombre = if nombre.trim().is_empty() || nombre.starts_with("pegado-") {
            "pegado.png".to_string()
        } else {
            nombre
        };
        // Sin repetir milisegundo: dos fotos pegadas juntas no se pisan.
        let fichero = format!(
            "{}-{}",
            hora_unica(),
            crate::notas::adjuntos::nombre_limpio(&nombre)
        );
        pixpin_sincro::disco::escribir_atomico(&carpeta.join(&fichero), &datos)?;
        Ok(nr::renglon_de_foto(&fichero))
    }
}

impl Nucleo {
    /// Lo escrito tal como se guarda: las fotos pegadas en la caja
    /// (`notas/…`) con su ruta portatil, para que viajen con la nota
    /// (`para_guardar` de Windows).
    pub(crate) fn nota_para_guardar(&self, chat: &str, texto: &str) -> String {
        if !texto.contains(&format!("]({}", nr::NOTAS)) {
            return texto.to_string();
        }
        let carpeta = almacen::carpeta(self.ruta(), chat).join("notas");
        let portatil = vista::portatil_de_ruta(self.ruta(), &carpeta);
        nr::enlaces_portatiles(texto, portatil.as_deref())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_nota_llana_no_es_rica() {
        assert_eq!(nota_rica_sin_fotos("hola, que tal".into()), None);
        assert!(nota_rica_sin_fotos("**hola**".into()).is_some());
        assert_eq!(bloques_de_nota("dos\nrenglones"), None);
        assert_eq!(bloques_de_nota("   "), None);
    }

    #[test]
    fn los_bloques_son_los_de_windows() {
        let b = bloques_de_nota(
            "# Obra\n**Pendiente:** la escalera\n- [x] medir\n![a](pixpin:files/x/notas/1-a.png)\n![b|300](pixpin:files/x/notas/2-b.jpg)\n![](pixpin:files/x/notas/3-plano.pdf)",
        )
        .unwrap();
        assert_eq!(
            b[0],
            BloqueDeNota::Texto {
                clase: ClaseDeBloque::Titulo { nivel: 1 },
                texto: "Obra".into(),
                marcas: vec![],
                enlaces: vec![],
            }
        );
        let BloqueDeNota::Texto { texto, marcas, .. } = &b[1] else {
            panic!("{b:?}")
        };
        assert_eq!(texto, "Pendiente: la escalera");
        assert!(marcas[0].negrita && marcas[0].inicio == 0);
        assert!(matches!(
            &b[2],
            BloqueDeNota::Texto { clase: ClaseDeBloque::Casilla { nivel: 0, hecha: true }, .. }
        ));
        let BloqueDeNota::Album { fotos } = &b[3] else {
            panic!("{b:?}")
        };
        // Dos fotos seguidas: un album, con su orden y sin el ancho.
        assert_eq!(fotos.len(), 2);
        assert_eq!((fotos[0].indice, fotos[1].indice), (0, 1));
        assert_eq!(fotos[1].alt, "b");
        assert_eq!(
            b[4],
            BloqueDeNota::Adjunto {
                nombre: "3-plano.pdf".into()
            }
        );
    }

    #[test]
    fn el_album_reparte_como_windows() {
        let a = album_de_nota(vec![1.0, 0.0, 2.0], 300.0, 400.0, 200.0, 2.0);
        assert_eq!(a.cajas.len(), 3);
        // Tres: la primera sola arriba y dos debajo.
        assert_eq!(a.cajas[0].ancho, 300.0);
        assert_eq!(a.cajas[1].y, a.cajas[2].y);
        assert!((a.cajas[2].x + a.cajas[2].ancho - 300.0).abs() < 0.01);
        // Una sola guarda su proporcion.
        let una = album_de_nota(vec![2.0], 300.0, 400.0, 200.0, 2.0);
        assert_eq!((una.cajas[0].ancho, una.alto), (300.0, 150.0));
    }

    #[test]
    fn los_renglones_de_una_foto_pegada() {
        assert!(es_foto_de_nota("Foto.JPG".into()));
        assert!(!es_foto_de_nota("informe.pdf".into()));
        assert_eq!(
            renglones_en_su_sitio("hola".into(), "".into(), vec!["![](notas/1-a.png)".into()]),
            "\n![](notas/1-a.png)\n"
        );
        assert_eq!(
            renglones_en_su_sitio("hola\n".into(), "\nfin".into(), vec!["x".into()]),
            "x"
        );
    }
}
