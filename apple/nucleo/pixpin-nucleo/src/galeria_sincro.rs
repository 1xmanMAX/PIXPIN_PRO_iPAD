//! **La galeria de capturas que viaja**, del lado del Mac (Android
//! v0.107.0): lo que `pixpin_sincro::galeria` necesita saber de las capturas
//! de este aparato (`CapturasDelAparato`). Es `sincronizar/galeria.rs` de
//! Windows (`CapturasDelPc`) linea a linea, sobre los mismos ficheros:
//!
//! - las capturas viven en `<datos>/capturas/` y su fecha es la de
//!   modificacion del fichero (por ella las agrupa la galeria por dia);
//! - el registro de caducidad es `capturas-caducidad.json` (el de
//!   [`crate::galeria`], con su cerrojo);
//! - lo que se quita en otro aparato va a la papelera de PixPin
//!   (`papelera/capturas/`), como el «Borrar» de la galeria;
//! - «dias hasta borrar» es `[capturas] dias_caducidad` de `pixpinmax.toml`.
//!
//! Las reglas (que se junta, que se trae, que se tira) son las de Android y
//! viven en `pixpin_sincro::galeria`; aqui solo se lee y se escribe.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use pixpin_sincro::galeria::{Caducidad, CapturasDelAparato, Entrada, Local};
use pixpin_sincro::protocolo::Resultado;
use pixpin_store::{Ubicacion, ajustes};

use crate::galeria::{self, DIAS_POR_OMISION};

/// Las capturas de este aparato, sobre la raiz de sus datos.
pub(crate) struct CapturasDelMac {
    raiz: PathBuf,
}

impl CapturasDelMac {
    pub(crate) fn nuevas(raiz: &Path) -> CapturasDelMac {
        CapturasDelMac {
            raiz: raiz.to_path_buf(),
        }
    }

    fn carpeta(&self) -> PathBuf {
        self.raiz.join("capturas")
    }
}

/// Donde se escribe una captura mientras llega: fuera de lo que lista la
/// galeria (su extension no es de captura), para que no se vea a medias.
fn ruta_llegando(carpeta: &Path, nombre: &str) -> PathBuf {
    carpeta.join(format!(".{nombre}.llegando"))
}

impl CapturasDelAparato for CapturasDelMac {
    fn raiz(&self) -> PathBuf {
        self.raiz.clone()
    }

    fn listar(&self) -> Option<Vec<Local>> {
        let carpeta = self.carpeta();
        // Sin carpeta aun no se capturo nada: «ninguna». Pero si esta y no se
        // puede leer, NO es «ninguna»: dar la lista vacia marcaria como
        // quitadas a mano todas las que habia y se irian de los demas.
        if carpeta.exists() && std::fs::read_dir(&carpeta).is_err() {
            return None;
        }
        Some(
            galeria::listar(&carpeta)
                .into_iter()
                .map(|e| Local {
                    texto: galeria::texto_guardado(&self.raiz, &carpeta.join(&e.nombre)),
                    cuando: e.cuando,
                    bytes: e.bytes as i64,
                    mime: pixpin_sincro::al_lienzo::mime_de(&e.nombre).to_string(),
                    nombre: e.nombre,
                })
                .collect(),
        )
    }

    fn abrir(&self, nombre: &str) -> Option<PathBuf> {
        Some(self.carpeta().join(nombre)).filter(|r| r.is_file())
    }

    fn guardar(
        &self,
        e: &Entrada,
        escribir: &mut dyn FnMut(&mut dyn Write) -> Resultado<bool>,
    ) -> Resultado<bool> {
        let carpeta = self.carpeta();
        std::fs::create_dir_all(&carpeta)?;
        let tmp = ruta_llegando(&carpeta, &e.nombre);
        let hecho = (|| -> Resultado<bool> {
            let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
            if !escribir(&mut f)? {
                return Ok(false);
            }
            let f = f.into_inner().map_err(|e| e.into_error())?;
            // **Con la hora de cuando se hizo**, no la de ahora: la galeria
            // agrupa por dia con la fecha del fichero.
            if e.cuando > 0 {
                f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_millis(e.cuando as u64))?;
            }
            drop(f);
            let ruta = carpeta.join(&e.nombre);
            std::fs::rename(&tmp, &ruta)?;
            // Su texto, si lo trae (Windows lo lee con su OCR): asi se
            // encuentra al buscar sin volver a leerla.
            if !e.texto.is_empty() {
                let _ = galeria::guardar_texto(&self.raiz, &ruta, &e.texto);
            }
            Ok(true)
        })();
        if !matches!(hecho, Ok(true)) {
            let _ = std::fs::remove_file(&tmp);
        }
        hecho
    }

    fn apuntar_texto(&self, nombre: &str, texto: &str) {
        let ruta = self.carpeta().join(nombre);
        if ruta.is_file() {
            let _ = galeria::guardar_texto(&self.raiz, &ruta, texto);
        }
    }

    fn tirar(&self, nombres: &[String]) {
        let ahora = crate::ahora_ms();
        for n in nombres {
            let ruta = self.carpeta().join(n);
            if ruta.is_file() {
                // Como el «Borrar» de la galeria: a la papelera de PixPin.
                let _ = galeria::a_la_papelera(&self.raiz, &ruta, ahora);
            }
        }
    }

    fn dias(&self) -> i64 {
        ajustes::cargar(&Ubicacion::Instalado {
            raiz: self.raiz.clone(),
        })
        .map(|a| i64::from(a.capturas.dias_caducidad))
        .unwrap_or(DIAS_POR_OMISION)
    }

    fn caducidad(&self, ahora: i64) -> Caducidad {
        galeria::leer_registro(&self.raiz, ahora)
    }

    fn cambiar_caducidad(
        &self,
        ahora: i64,
        f: &mut dyn FnMut(&mut Caducidad),
    ) -> std::io::Result<()> {
        galeria::cambiar_registro(&self.raiz, ahora, |r| f(r)).map(|_| ())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-galeria-mac-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn una_captura_que_llega_se_guarda_con_la_fecha_de_cuando_se_hizo() {
        let raiz = temporal("llega");
        let c = CapturasDelMac::nuevas(&raiz);
        assert_eq!(c.listar(), Some(Vec::new()), "sin carpeta: ninguna");
        let mut e = Entrada::nueva("PixPin_20261005_101500.png", 1_791_000_000_000);
        e.mime = "image/png".into();
        let entera = c
            .guardar(&e, &mut |w| {
                w.write_all(b"\x89PNG datos").unwrap();
                Ok(true)
            })
            .unwrap();
        assert!(entera);
        let l = c.listar().unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].nombre, "PixPin_20261005_101500.png");
        assert_eq!(l[0].cuando, 1_791_000_000_000);
        assert_eq!(l[0].mime, "image/png");
        assert!(c.abrir("PixPin_20261005_101500.png").is_some());
        assert_eq!(c.dias(), DIAS_POR_OMISION);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn caso_negativo_una_que_no_llego_entera_no_deja_nada() {
        let raiz = temporal("rota");
        let c = CapturasDelMac::nuevas(&raiz);
        let e = Entrada::nueva("a.png", 5);
        let entera = c
            .guardar(&e, &mut |w| {
                w.write_all(b"medio").unwrap();
                Ok(false)
            })
            .unwrap();
        assert!(!entera);
        assert!(c.listar().unwrap().is_empty());
        let carpeta = raiz.join("capturas");
        assert_eq!(std::fs::read_dir(&carpeta).unwrap().count(), 0);
        assert!(
            c.guardar(&e, &mut |_| Err(
                pixpin_sincro::protocolo::ErrorSincro::Protocolo("cortado".into())
            ))
            .is_err()
        );
        assert_eq!(std::fs::read_dir(&carpeta).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn lo_quitado_en_otro_va_a_la_papelera_de_pixpin_y_lo_acordado_al_registro() {
        let raiz = temporal("tirar");
        let c = CapturasDelMac::nuevas(&raiz);
        let carpeta = raiz.join("capturas");
        std::fs::create_dir_all(&carpeta).unwrap();
        std::fs::write(carpeta.join("a.png"), b"x").unwrap();
        c.tirar(&["a.png".to_string()]);
        assert!(!carpeta.join("a.png").exists());
        assert!(
            raiz.join("papelera")
                .join("capturas")
                .join("a.png")
                .exists()
        );
        c.cambiar_caducidad(7, &mut |r| {
            r.fijadas.insert("b.png".into(), 99);
        })
        .unwrap();
        assert_eq!(c.caducidad(7).fijadas.get("b.png"), Some(&99));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_texto_de_una_captura_viaja_al_llegar_y_al_listar() {
        let raiz = temporal("texto");
        let c = CapturasDelMac::nuevas(&raiz);
        let mut e = Entrada::nueva("PixPin_20261017_090000.png", 1_791_000_000_000);
        e.texto = "Factura 123".into();
        c.guardar(&e, &mut |w| {
            w.write_all(b"\x89PNG").unwrap();
            Ok(true)
        })
        .unwrap();
        // Llego con su texto (el OCR de Windows): ya se sabe aqui.
        assert_eq!(c.listar().unwrap()[0].texto, "Factura 123");
        // Y el que llega despues para una que ya estaba, se apunta.
        let mut b = Entrada::nueva("b.png", 1_791_000_000_000);
        b.mime = "image/png".into();
        c.guardar(&b, &mut |w| {
            w.write_all(b"\x89PNG b").unwrap();
            Ok(true)
        })
        .unwrap();
        let sin = c.listar().unwrap();
        assert!(sin.iter().find(|l| l.nombre == "b.png").unwrap().texto.is_empty());
        c.apuntar_texto("b.png", "hola");
        let con = c.listar().unwrap();
        assert_eq!(con.iter().find(|l| l.nombre == "b.png").unwrap().texto, "hola");
        // Caso negativo: una que no esta no deja texto suelto.
        c.apuntar_texto("no-esta.png", "x");
        assert!(!raiz.join("cache/galeria").join("no-esta").exists());
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
