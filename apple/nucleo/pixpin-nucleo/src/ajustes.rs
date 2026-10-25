//! **Los ajustes que valen en los dos sistemas**, en el mismo
//! `pixpinmax.toml` que Windows (`pixpin_store::ajustes`), en la raiz de la
//! carpeta de datos.
//!
//! Cruzan las claves que significan lo mismo en Mac y en Windows (las de la
//! ventana de Ajustes de Windows, `ventana_ajustes.rs`): el idioma, la
//! captura (retardo, formato del color, tope del scroll), el GIF, los dias
//! que aguanta una captura, la sincronizacion (`[sincro]` presencia,
//! automatica y «Lo mio manda»), la voz (`[voz]`) y el asistente
//! (`[asistente]`). Lo que es de cada aparato (los atajos, el tema, abrir al
//! iniciar sesion) vive en Swift (UserDefaults, `SMAppService`): un atajo de
//! Carbon no es un atajo de Windows y no tiene sentido que viaje.
//!
//! Los valores con nombre (idioma, formato del color, modo de la voz…) cruzan
//! como el texto del TOML (`"es"`, `"hsl"`, `"cada_uno"`): asi Swift no
//! necesita un enum por cada uno y el fichero dice lo mismo que la pantalla.
//!
//! Se guarda con `guardar_conservando`: el fichero conserva sus
//! comentarios, su orden y las claves de Windows que el Mac no toca. Si el
//! fichero que hay no se entiende, **no se guarda** (se dice): reescribirlo
//! con los valores de fabrica le borraria al usuario todo lo de Windows.

use pixpin_store::ajustes::{FormatoColor, ModoDeIdiomas, PreferenciaIdioma};
use pixpin_store::{Ubicacion, ajustes};

use crate::{ErrorNucleo, Nucleo};

/// Lo de `pixpinmax.toml` que se toca desde el Mac.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AjustesMac {
    /// Dias que aguanta una captura antes de irse sola (0 = nunca).
    pub dias_caducidad: u32,
    /// Sincronizar solo con los aparatos del grupo que contesten.
    pub sincro_automatica: bool,
    /// Dejarse encontrar en la red local (escuchar y anunciarse).
    pub sincro_presencia: bool,
    /// «Lo mio manda» de fabrica: al sincronizar, lo de este equipo se
    /// impone (`[sincro] lo_mio_manda`).
    pub sincro_lo_mio_manda: bool,
    /// `sistema`, `es` o `en` (`idioma`).
    pub idioma: String,
    /// Segundos de la captura con retardo (`retardo_captura_s`, 0 a 30).
    pub retardo_captura_s: u32,
    /// `hex`, `rgb` o `hsl` (`formato_color`, el del cuentagotas).
    pub formato_color: String,
    /// Tope de alto de la captura con scroll (`limite_scroll_px`).
    pub limite_scroll_px: u32,
    /// Fotogramas por segundo del GIF (`[gif] por_segundo`, 5 a 30).
    pub gif_por_segundo: u32,
    /// Segundos de cortesia antes del primer fotograma (`[gif] retardo_s`).
    pub gif_retardo_s: u32,
    /// Segundo idioma de las notas de voz: vacio, `en`, `es` o `pt`.
    pub voz_segundo_idioma: String,
    /// `cada_uno` o `todo_en_uno` (`[voz] modo_de_idiomas`).
    pub voz_modo_de_idiomas: String,
    /// Quien contesta: vacio (local), `claude` o `deepseek`.
    pub asistente_motor: String,
    /// El modelo local: vacio (el 2B) o `qwen3.5-0.8b`.
    pub asistente_modelo: String,
    /// Pensar antes de contestar: `None` automatico, o siempre/nunca.
    pub asistente_pensar: Option<bool>,
    /// Si la nube falla, seguir con el modelo local.
    pub asistente_respaldo_local: bool,
    /// Gemma 4 E2B para mirar fotos.
    pub asistente_mejor_vista: bool,
    /// Los documentos propios a la nube: vacio (preguntar), `si` o `no`.
    pub asistente_documentos_a_la_nube: String,
    /// Lo dicho al microfono se deja en la caja para repasarlo.
    pub asistente_revisar_lo_dicho: bool,
}

/// Los rangos de la ventana de Windows (`ventana_ajustes.rs`): lo de fuera
/// solo puede ser un error de quien llama.
pub const RETARDO_MAXIMO: u32 = 30;
pub const SCROLL_MINIMO: u32 = 2_000;
pub const SCROLL_MAXIMO: u32 = 100_000;
pub const GIF_RITMO_MINIMO: u32 = 5;
pub const GIF_RITMO_MAXIMO: u32 = 30;
pub const GIF_RETARDO_MAXIMO: u32 = 10;

fn texto_idioma(i: PreferenciaIdioma) -> &'static str {
    match i {
        PreferenciaIdioma::Sistema => "sistema",
        PreferenciaIdioma::Espanol => "es",
        PreferenciaIdioma::Ingles => "en",
    }
}

fn idioma_de(t: &str) -> Option<PreferenciaIdioma> {
    Some(match t {
        "sistema" => PreferenciaIdioma::Sistema,
        "es" => PreferenciaIdioma::Espanol,
        "en" => PreferenciaIdioma::Ingles,
        _ => return None,
    })
}

fn texto_color(f: FormatoColor) -> &'static str {
    match f {
        FormatoColor::Hex => "hex",
        FormatoColor::Rgb => "rgb",
        FormatoColor::Hsl => "hsl",
    }
}

fn color_de(t: &str) -> Option<FormatoColor> {
    Some(match t {
        "hex" => FormatoColor::Hex,
        "rgb" => FormatoColor::Rgb,
        "hsl" => FormatoColor::Hsl,
        _ => return None,
    })
}

fn texto_modo(m: ModoDeIdiomas) -> &'static str {
    match m {
        ModoDeIdiomas::CadaUno => "cada_uno",
        ModoDeIdiomas::TodoEnUno => "todo_en_uno",
    }
}

fn modo_de(t: &str) -> Option<ModoDeIdiomas> {
    Some(match t {
        "cada_uno" => ModoDeIdiomas::CadaUno,
        "todo_en_uno" => ModoDeIdiomas::TodoEnUno,
        _ => return None,
    })
}

impl From<&ajustes::Ajustes> for AjustesMac {
    fn from(a: &ajustes::Ajustes) -> Self {
        AjustesMac {
            dias_caducidad: a.capturas.dias_caducidad,
            sincro_automatica: a.sincro.automatica,
            sincro_presencia: a.sincro.presencia,
            sincro_lo_mio_manda: a.sincro.lo_mio_manda,
            idioma: texto_idioma(a.idioma).into(),
            retardo_captura_s: a.retardo_captura_s,
            formato_color: texto_color(a.formato_color).into(),
            limite_scroll_px: a.limite_scroll_px,
            gif_por_segundo: a.gif.por_segundo,
            gif_retardo_s: a.gif.retardo_s,
            voz_segundo_idioma: a.voz.segundo_idioma.clone(),
            voz_modo_de_idiomas: texto_modo(a.voz.modo_de_idiomas).into(),
            asistente_motor: a.asistente.motor.clone(),
            asistente_modelo: a.asistente.modelo.clone(),
            asistente_pensar: a.asistente.pensar,
            asistente_respaldo_local: a.asistente.respaldo_local,
            asistente_mejor_vista: a.asistente.mejor_vista,
            asistente_documentos_a_la_nube: a.asistente.documentos_a_la_nube.clone(),
            asistente_revisar_lo_dicho: a.asistente.revisar_lo_dicho,
        }
    }
}

fn fuera(que: &str, valor: impl std::fmt::Debug) -> ErrorNucleo {
    ErrorNucleo::Argumento {
        detalle: format!("{que} fuera de rango: {valor:?}"),
    }
}

impl AjustesMac {
    /// Pasa lo del Mac a los ajustes leidos, comprobando cada valor. Con un
    /// valor malo no se toca nada.
    fn aplicar(&self, a: &mut ajustes::Ajustes) -> Result<(), ErrorNucleo> {
        if self.dias_caducidad > DIAS_MAXIMOS {
            return Err(fuera("dias de caducidad", self.dias_caducidad));
        }
        if self.retardo_captura_s > RETARDO_MAXIMO {
            return Err(fuera("retardo de la captura", self.retardo_captura_s));
        }
        if !(SCROLL_MINIMO..=SCROLL_MAXIMO).contains(&self.limite_scroll_px) {
            return Err(fuera("alto maximo con scroll", self.limite_scroll_px));
        }
        if !(GIF_RITMO_MINIMO..=GIF_RITMO_MAXIMO).contains(&self.gif_por_segundo) {
            return Err(fuera("fotogramas por segundo", self.gif_por_segundo));
        }
        if self.gif_retardo_s > GIF_RETARDO_MAXIMO {
            return Err(fuera("cortesia del GIF", self.gif_retardo_s));
        }
        let idioma = idioma_de(&self.idioma).ok_or_else(|| fuera("idioma", &self.idioma))?;
        let color = color_de(&self.formato_color)
            .ok_or_else(|| fuera("formato del color", &self.formato_color))?;
        let modo = modo_de(&self.voz_modo_de_idiomas)
            .ok_or_else(|| fuera("modo de idiomas", &self.voz_modo_de_idiomas))?;
        if !["", "en", "es", "pt"].contains(&self.voz_segundo_idioma.as_str()) {
            return Err(fuera("segundo idioma", &self.voz_segundo_idioma));
        }
        if !["", "local", "claude", "deepseek"].contains(&self.asistente_motor.as_str()) {
            return Err(fuera("motor del asistente", &self.asistente_motor));
        }
        if !["", "si", "no"].contains(&self.asistente_documentos_a_la_nube.as_str()) {
            return Err(fuera(
                "documentos a la nube",
                &self.asistente_documentos_a_la_nube,
            ));
        }
        a.capturas.dias_caducidad = self.dias_caducidad;
        a.sincro.automatica = self.sincro_automatica;
        a.sincro.presencia = self.sincro_presencia;
        a.sincro.lo_mio_manda = self.sincro_lo_mio_manda;
        a.idioma = idioma;
        a.retardo_captura_s = self.retardo_captura_s;
        a.formato_color = color;
        a.limite_scroll_px = self.limite_scroll_px;
        a.gif.por_segundo = self.gif_por_segundo;
        a.gif.retardo_s = self.gif_retardo_s;
        a.voz.segundo_idioma = self.voz_segundo_idioma.clone();
        a.voz.modo_de_idiomas = modo;
        a.asistente.motor = self.asistente_motor.clone();
        a.asistente.modelo = self.asistente_modelo.clone();
        a.asistente.pensar = self.asistente_pensar;
        a.asistente.respaldo_local = self.asistente_respaldo_local;
        a.asistente.mejor_vista = self.asistente_mejor_vista;
        a.asistente.documentos_a_la_nube = self.asistente_documentos_a_la_nube.clone();
        a.asistente.revisar_lo_dicho = self.asistente_revisar_lo_dicho;
        Ok(())
    }
}

impl From<ajustes::ErrorAjustes> for ErrorNucleo {
    fn from(e: ajustes::ErrorAjustes) -> Self {
        ErrorNucleo::Ajustes {
            detalle: e.to_string(),
        }
    }
}

/// El tope de dias: un ano y pico. Mas no tiene sentido y un numero enorme
/// solo puede ser un error de escritura.
pub const DIAS_MAXIMOS: u32 = 400;

#[uniffi::export]
impl Nucleo {
    /// Los ajustes de `pixpinmax.toml` (los de fabrica si no hay fichero).
    pub fn ajustes(&self) -> Result<AjustesMac, ErrorNucleo> {
        Ok(AjustesMac::from(&ajustes::cargar(&self.ubicacion())?))
    }

    /// Guarda los ajustes del Mac en `pixpinmax.toml` sin tocar lo demas.
    pub fn guardar_ajustes(&self, nuevos: AjustesMac) -> Result<(), ErrorNucleo> {
        let u = self.ubicacion();
        // Si el fichero no se entiende, aqui sale el error y no se escribe.
        let mut a = ajustes::cargar(&u)?;
        nuevos.aplicar(&mut a)?;
        ajustes::guardar_conservando(&u, &a)?;
        Ok(())
    }

    /// Los de fabrica, los de Windows (para el punto azul de «cambiado» y
    /// «Restablecer» de la pantalla de Ajustes).
    pub fn ajustes_de_fabrica(&self) -> AjustesMac {
        AjustesMac::from(&ajustes::Ajustes::default())
    }

    /// La ruta de `pixpinmax.toml` (para ensenarla).
    pub fn ruta_ajustes(&self) -> String {
        self.ubicacion().fichero_ajustes().display().to_string()
    }
}

impl Nucleo {
    pub(crate) fn ubicacion(&self) -> Ubicacion {
        Ubicacion::Instalado {
            raiz: self.ruta().to_path_buf(),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(n: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("pixpin-nucleo-ajustes-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn sin_fichero_salen_los_de_fabrica_y_se_guardan() {
        let d = temporal("fabrica");
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        let a = n.ajustes().unwrap();
        assert_eq!(a, n.ajustes_de_fabrica());
        assert_eq!(a.dias_caducidad, 7);
        assert!(a.sincro_automatica);
        assert!(!a.sincro_lo_mio_manda);
        assert_eq!(a.idioma, "sistema");
        assert_eq!(a.retardo_captura_s, 3);
        assert_eq!(a.formato_color, "hex");
        assert_eq!(a.limite_scroll_px, 30_000);
        assert_eq!((a.gif_por_segundo, a.gif_retardo_s), (10, 0));
        assert_eq!(a.voz_modo_de_idiomas, "cada_uno");
        assert!(a.asistente_respaldo_local);
        assert_eq!(a.asistente_pensar, None);
        n.guardar_ajustes(AjustesMac {
            dias_caducidad: 30,
            sincro_automatica: false,
            ..a
        })
        .unwrap();
        let b = n.ajustes().unwrap();
        assert_eq!(b.dias_caducidad, 30);
        assert!(!b.sincro_automatica);
        assert!(n.ruta_ajustes().ends_with("pixpinmax.toml"));
    }

    #[test]
    fn todo_lo_de_windows_va_y_vuelve_con_sus_nombres() {
        let d = temporal("todo");
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        let nuevos = AjustesMac {
            sincro_lo_mio_manda: true,
            sincro_presencia: false,
            idioma: "en".into(),
            retardo_captura_s: 5,
            formato_color: "hsl".into(),
            limite_scroll_px: 40_000,
            gif_por_segundo: 15,
            gif_retardo_s: 2,
            voz_segundo_idioma: "pt".into(),
            voz_modo_de_idiomas: "todo_en_uno".into(),
            asistente_motor: "claude".into(),
            asistente_modelo: "qwen3.5-0.8b".into(),
            asistente_pensar: Some(false),
            asistente_respaldo_local: false,
            asistente_mejor_vista: true,
            asistente_documentos_a_la_nube: "no".into(),
            asistente_revisar_lo_dicho: true,
            ..n.ajustes_de_fabrica()
        };
        n.guardar_ajustes(nuevos.clone()).unwrap();
        assert_eq!(n.ajustes().unwrap(), nuevos);
        // Lo escrito es lo que lee Windows (`pixpin_store::ajustes`).
        let texto = std::fs::read_to_string(d.join("pixpinmax.toml")).unwrap();
        for linea in [
            "idioma = \"en\"",
            "retardo_captura_s = 5",
            "formato_color = \"hsl\"",
            "limite_scroll_px = 40000",
            "lo_mio_manda = true",
            "modo_de_idiomas = \"todo_en_uno\"",
            "segundo_idioma = \"pt\"",
            "motor = \"claude\"",
            "pensar = false",
        ] {
            assert!(texto.contains(linea), "falta {linea:?} en:\n{texto}");
        }
        let w = ajustes::cargar(&n.ubicacion()).unwrap();
        assert_eq!(w.gif.por_segundo, 15);
        assert_eq!(w.idioma, PreferenciaIdioma::Ingles);
        assert!(w.sincro.lo_mio_manda);
        // Volver a «automatico» quita la clave `pensar`.
        n.guardar_ajustes(AjustesMac {
            asistente_pensar: None,
            ..nuevos
        })
        .unwrap();
        assert_eq!(n.ajustes().unwrap().asistente_pensar, None);
    }

    #[test]
    fn se_conservan_los_comentarios_y_lo_de_windows() {
        let d = temporal("conservar");
        let toml = "# mis ajustes\narranque_con_windows = false\nclave_rara = 5\n\n[capturas]\n# una semana\ndias_caducidad = 7\n";
        std::fs::write(d.join("pixpinmax.toml"), toml).unwrap();
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        n.guardar_ajustes(AjustesMac {
            dias_caducidad: 14,
            sincro_presencia: false,
            ..n.ajustes().unwrap()
        })
        .unwrap();
        let texto = std::fs::read_to_string(d.join("pixpinmax.toml")).unwrap();
        assert!(texto.contains("# mis ajustes"));
        assert!(texto.contains("# una semana"));
        assert!(texto.contains("clave_rara = 5"));
        assert!(texto.contains("arranque_con_windows = false"));
        assert!(texto.contains("dias_caducidad = 14"));
        assert!(!n.ajustes().unwrap().sincro_presencia);
    }

    #[test]
    fn un_fichero_roto_no_se_pisa() {
        let d = temporal("roto");
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        let fabrica = n.ajustes_de_fabrica();
        std::fs::write(d.join("pixpinmax.toml"), "esto = = no es toml").unwrap();
        assert!(matches!(n.ajustes(), Err(ErrorNucleo::Ajustes { .. })));
        let e = n.guardar_ajustes(fabrica.clone());
        assert!(matches!(e, Err(ErrorNucleo::Ajustes { .. })));
        assert_eq!(
            std::fs::read_to_string(d.join("pixpinmax.toml")).unwrap(),
            "esto = = no es toml"
        );
    }

    #[test]
    fn lo_fuera_de_rango_no_se_guarda() {
        let d = temporal("rango");
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        let f = n.ajustes_de_fabrica();
        let malos = [
            AjustesMac {
                dias_caducidad: 9999,
                ..f.clone()
            },
            AjustesMac {
                retardo_captura_s: 31,
                ..f.clone()
            },
            AjustesMac {
                limite_scroll_px: 100,
                ..f.clone()
            },
            AjustesMac {
                gif_por_segundo: 60,
                ..f.clone()
            },
            AjustesMac {
                gif_retardo_s: 11,
                ..f.clone()
            },
            AjustesMac {
                idioma: "fr".into(),
                ..f.clone()
            },
            AjustesMac {
                formato_color: "cmyk".into(),
                ..f.clone()
            },
            AjustesMac {
                voz_modo_de_idiomas: "otro".into(),
                ..f.clone()
            },
            AjustesMac {
                voz_segundo_idioma: "de".into(),
                ..f.clone()
            },
            AjustesMac {
                asistente_motor: "openai".into(),
                ..f.clone()
            },
            AjustesMac {
                asistente_documentos_a_la_nube: "quiza".into(),
                ..f.clone()
            },
        ];
        for m in malos {
            assert!(
                matches!(
                    n.guardar_ajustes(m.clone()),
                    Err(ErrorNucleo::Argumento { .. })
                ),
                "se guardo {m:?}"
            );
        }
        assert!(!d.join("pixpinmax.toml").exists());
    }
}
