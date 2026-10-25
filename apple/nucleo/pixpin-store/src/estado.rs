//! Lo que el programa recuerda solo, aparte de lo que tu escribes.
//!
//! Hay dos clases de cosas guardadas y no pueden vivir en el mismo fichero.
//! `pixpinmax.toml` es TUYO: se edita con el bloc de notas y esta lleno de
//! comentarios que explican cada linea. `estado.toml` lo escribe el
//! programa cada vez que eliges algo en pantalla, y por eso nadie deberia
//! molestarse en comentarlo.
//!
//! Mezclarlos costaria los comentarios: guardar los ajustes serializa la
//! estructura entera y reescribe el fichero, asi que la primera vez que
//! cambiaras los fotogramas por segundo desde la barra de grabar, tu
//! `pixpinmax.toml` volveria del reves sin una sola explicacion dentro.
//!
//! Lo de aqui SIEMPRE es opcional. Si el fichero no esta, esta roto o trae
//! un valor que ya no significa nada, se ignora y manda lo que digan los
//! ajustes. Perder esto no puede costarle nada a nadie.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::rutas::Ubicacion;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Estado {
    /// Los fotogramas por segundo que se eligieron la ultima vez en la
    /// barra de grabar. `None` significa que nunca se toco, y entonces vale
    /// lo que diga `[gif] por_segundo` en los ajustes.
    pub gif_por_segundo: Option<u32>,
    /// Donde y de que tamano quedo la ventana de chat: (x, y, ancho, alto)
    /// en pixeles fisicos. `None` la primera vez.
    pub chat_ventana: Option<[i32; 4]>,
    /// La escala de la interfaz del chat, en por ciento, como la de
    /// Telegram: cuanto agrandar o encoger sobre lo que ya manda el DPI del
    /// monitor. `None` es «lo que diga el monitor».
    ///
    /// Relativa y no absoluta a proposito: quien la sube porque no ve bien
    /// quiere verlo igual de grande en el portatil y en el monitor de casa,
    /// y cada uno tiene su DPI.
    pub escala_interfaz: Option<u32>,
    /// **El modo oscuro de los visores** (el boton luna/sol de la barra de
    /// los pines-lector): uno para todos. `None` es «nunca se toco», y
    /// entonces manda el tema de Windows, como en Android manda el del
    /// telefono.
    pub lectores_oscuros: Option<bool>,
    /// **La velocidad de la pagina que sube sola** en los lectores, en
    /// palabras por minuto (`auto:ppm` del movil): es del equipo, no del
    /// documento. `None` es la de fabrica (230).
    pub palabras_por_minuto: Option<u32>,
    /// **El lienzo, a pantalla completa o en ventana** (la ultima eleccion,
    /// F11 o el boton de la barra): uno para todos los lienzos. `None` es
    /// «nunca se toco», y entonces a pantalla completa, como siempre.
    pub lienzo_pantalla_completa: Option<bool>,
    /// Donde y de que tamano quedo el lienzo en ventana: (x, y, ancho, alto)
    /// en pixeles fisicos. `None` la primera vez.
    pub lienzo_ventana: Option<[i32; 4]>,
}

fn fichero(ubicacion: &Ubicacion) -> PathBuf {
    ubicacion.raiz().join("estado.toml")
}

/// Lee lo recordado. Cualquier problema devuelve el estado vacio: esto no
/// es motivo para que el programa se queje, y menos para que no arranque.
pub fn cargar(ubicacion: &Ubicacion) -> Estado {
    fs::read_to_string(fichero(ubicacion))
        .ok()
        .and_then(|t| toml::from_str(&t).ok())
        .unwrap_or_default()
}

/// Guarda lo recordado. Devuelve el error para poder anotarlo en el
/// registro, pero quien llama puede seguir adelante sin mas: lo unico que
/// se pierde es una comodidad.
pub fn guardar(ubicacion: &Ubicacion, estado: &Estado) -> std::io::Result<()> {
    let ruta = fichero(ubicacion);
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre)?;
    }
    let texto = toml::to_string_pretty(estado)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(ruta, texto)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(etiqueta: &str) -> Ubicacion {
        let dir = std::env::temp_dir().join(format!("pixpin-estado-{etiqueta}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Ubicacion::Instalado { raiz: dir }
    }

    #[test]
    fn lo_recordado_vuelve_igual() {
        let u = temporal("ida-y-vuelta");
        assert_eq!(cargar(&u), Estado::default());
        let e = Estado {
            gif_por_segundo: Some(25),
            chat_ventana: Some([10, 20, 900, 700]),
            escala_interfaz: Some(125),
            lectores_oscuros: Some(true),
            palabras_por_minuto: Some(260),
            lienzo_pantalla_completa: Some(false),
            lienzo_ventana: Some([-1200, 40, 900, 640]),
        };
        guardar(&u, &e).unwrap();
        assert_eq!(cargar(&u), e);
    }

    #[test]
    fn un_fichero_roto_no_rompe_nada() {
        // Caso negativo: esto lo escribe el programa, no una persona, asi
        // que un fichero ilegible solo puede venir de un apagon a media
        // escritura. La respuesta correcta es olvidarlo, no fallar.
        let u = temporal("roto");
        fs::write(fichero(&u), "esto no es TOML = = [[[").unwrap();
        assert_eq!(cargar(&u), Estado::default());
    }

    #[test]
    fn sin_fichero_no_hay_nada_recordado() {
        let u = temporal("vacio");
        assert_eq!(cargar(&u).gif_por_segundo, None);
    }
}
