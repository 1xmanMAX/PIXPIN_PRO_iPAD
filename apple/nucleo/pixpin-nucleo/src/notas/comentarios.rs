//! **Los comentarios de la nota** (`notas_md/comentarios.rs` y
//! `comentarios_web.rs` de Windows): en el fichero hermano
//! `anot-<codigo>.comentarios.json`, junto a lo anotado del chat, que es lo
//! que los hace viajar con la nota (Windows y Android leen el mismo). Las
//! cuentas son las de `pixpin_docs::md_comentarios` (el ancla es una cita
//! con su contexto; al cambiar el texto se vuelve a buscar; al guardar se
//! junta con lo que haya llegado al disco). La pagina solo los pinta y pide
//! cambios.

use std::path::{Path, PathBuf};

use pixpin_docs::md_comentarios::{self as mc, Comentarios, Quien};
use pixpin_proyecto::comentarios_de_notas as sitio;
use serde_json::json;

use super::DestinoNota;

/// El fichero de comentarios de la nota. `codigo`: el codigo unico del
/// mensaje `NOTA` (el que no cambia aunque cambie el titulo).
pub fn ruta(raiz: &Path, destino: &DestinoNota, codigo: &str) -> Option<PathBuf> {
    match destino {
        DestinoNota::Mensaje { chat, .. } => sitio::de_la_nota(raiz, chat, codigo),
        DestinoNota::Fichero { ruta } => Some(sitio::del_md(raiz, Path::new(ruta))),
    }
}

pub struct Comentarista {
    ruta: Option<PathBuf>,
    /// Lo que habia en el disco al leer (la base de la fusion).
    base: Comentarios,
    /// Lo de aqui.
    mio: Comentarios,
    quien: Quien,
    /// El fichero no se entiende: no se escribe encima (seria perderlo).
    roto: bool,
}

fn utf16(t: &str) -> Vec<u16> {
    t.encode_utf16().collect()
}

impl Comentarista {
    pub fn abrir(ruta: Option<PathBuf>, quien: Quien) -> Self {
        let (c, roto) = match ruta.as_ref().map(|r| sitio::leer(r)) {
            Some(Some(t)) => match mc::leer(&t) {
                Ok(c) => (c, false),
                Err(_) => (Comentarios::default(), true),
            },
            // Esta pero no se lee: tampoco se pisa.
            Some(None) => (Comentarios::default(), true),
            None => (Comentarios::default(), false),
        };
        Comentarista {
            ruta,
            base: c.clone(),
            mio: c,
            quien,
            roto,
        }
    }

    /// Los hilos con donde estan ahora en `texto` (unidades UTF-16, las de
    /// la pagina), para pintarlos.
    pub fn json(&mut self, texto: &str) -> serde_json::Value {
        let sitios = self.mio.reanclar(&utf16(texto));
        let hilos: Vec<serde_json::Value> = self
            .mio
            .comentarios
            .iter()
            .zip(sitios)
            .map(|(h, s)| {
                json!({
                    "id": h.id,
                    "desde": s.map(|s| s.0),
                    "hasta": s.map(|s| s.1),
                    "cita": h.ancla.cita,
                    "autor": h.autor,
                    "aparato": h.aparato,
                    "cuando": h.cuando,
                    "editado": h.editado,
                    "texto": h.texto,
                    "resuelto": h.resuelto,
                    "respuestas": h.respuestas.iter().map(|r| json!({
                        "id": r.id, "autor": r.autor, "aparato": r.aparato,
                        "cuando": r.cuando, "editado": r.editado, "texto": r.texto,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        json!({"tipo": "comentarios", "hilos": hilos, "abiertos": self.mio.abiertos(), "yo": self.quien.aparato})
    }

    /// Los hilos con donde estan ahora en `texto` (UTF-16), sin pasar por
    /// JSON (el editor nativo del iPad).
    pub fn hilos(&mut self, texto: &str) -> Vec<(mc::Hilo, Option<(usize, usize)>)> {
        let sitios = self.mio.reanclar(&utf16(texto));
        self.mio.comentarios.iter().cloned().zip(sitios).collect()
    }

    /// Cuantos hilos siguen abiertos.
    pub fn abiertos(&self) -> usize {
        self.mio.abiertos()
    }

    /// El aparato de quien comenta aqui (para saber que es «mio»).
    pub fn aparato(&self) -> String {
        self.quien.aparato.clone()
    }

    pub fn nuevo(
        &mut self,
        texto: &str,
        desde: usize,
        hasta: usize,
        cuerpo: &str,
        ahora: i64,
    ) -> Option<String> {
        let t = utf16(texto);
        let (desde, hasta) = (desde.min(t.len()), hasta.min(t.len()));
        let (desde, hasta) = if desde == hasta {
            mc::palabra_en(&t, desde)?
        } else {
            (desde.min(hasta), desde.max(hasta))
        };
        let ancla = mc::ancla_de(&t, desde, hasta)?;
        let id = self.mio.nuevo(ancla, &self.quien, ahora, cuerpo)?;
        self.guardar();
        Some(id)
    }

    pub fn responder(&mut self, hilo: &str, cuerpo: &str, ahora: i64) {
        if self
            .mio
            .responder(hilo, &self.quien, ahora, cuerpo)
            .is_some()
        {
            self.guardar();
        }
    }

    pub fn editar(&mut self, id: &str, cuerpo: &str, ahora: i64) {
        if self.mio.editar(id, cuerpo, ahora) {
            self.guardar();
        }
    }

    pub fn borrar(&mut self, id: &str) {
        if self.mio.borrar(id) {
            self.guardar();
        }
    }

    pub fn resolver(&mut self, hilo: &str, si: bool, ahora: i64) {
        if self.mio.resolver(hilo, si, &self.quien, ahora) {
            self.guardar();
        }
    }

    /// Escribe junto con lo que haya llegado al disco mientras tanto.
    fn guardar(&mut self) {
        let Some(ruta) = self.ruta.clone() else {
            return;
        };
        if self.roto {
            return;
        }
        let disco = sitio::leer(&ruta)
            .map(|t| mc::leer(&t))
            .unwrap_or_else(|| Ok(Comentarios::default()));
        let Ok(disco) = disco else {
            // Se rompio por fuera: no se pisa.
            return;
        };
        let junto = mc::fusionar(&self.base, &self.mio, &disco);
        if sitio::escribir(&ruta, &mc::escribir(&junto)).is_ok() {
            self.base = junto.clone();
            self.mio = junto;
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_comentario_se_guarda_en_su_fichero_y_vuelve_con_su_sitio() {
        let d = std::env::temp_dir().join(format!("pixpin-com-web-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let md = d.join("nota.md");
        std::fs::write(&md, "Hola mundo bonito").unwrap();
        let destino = DestinoNota::Fichero {
            ruta: md.display().to_string(),
        };
        let r = ruta(&d, &destino, "");
        // Un `.md` suelto los lleva a su lado.
        assert_eq!(r, Some(d.join("nota.comentarios.json")));
        let quien = Quien {
            autor: "Max".into(),
            aparato: "MAC1".into(),
        };
        let mut c = Comentarista::abrir(r.clone(), quien.clone());
        let id = c.nuevo("Hola mundo bonito", 5, 10, "¿seguro?", 1).unwrap();
        // Otra vez desde el disco: esta, y con el texto movido se reencuentra.
        let mut c2 = Comentarista::abrir(r.clone(), quien);
        let j = c2.json("Ya: Hola mundo bonito");
        assert_eq!(j["hilos"][0]["id"], id);
        assert_eq!(j["hilos"][0]["desde"], 9);
        assert_eq!(j["abiertos"], 1);
        c2.resolver(&id, true, 2);
        assert_eq!(c2.json("Hola mundo bonito")["abiertos"], 0);
        // Caso negativo: un comentario vacio no se crea.
        assert!(c2.nuevo("Hola mundo bonito", 0, 4, "   ", 3).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_fichero_roto_no_se_pisa() {
        let d = std::env::temp_dir().join(format!("pixpin-com-roto-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("x.comentarios.json");
        std::fs::write(&f, "esto no es json").unwrap();
        let quien = Quien {
            autor: "Max".into(),
            aparato: "MAC1".into(),
        };
        let mut c = Comentarista::abrir(Some(f.clone()), quien);
        let _ = c.nuevo("Hola mundo", 0, 4, "algo", 1);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "esto no es json");
        let _ = std::fs::remove_dir_all(&d);
    }
}
