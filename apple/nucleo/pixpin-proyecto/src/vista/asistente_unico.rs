//! **Un solo chat «Asistente» en todos los aparatos** (22-oct-2026).
//!
//! Cada aparato creaba el suyo y el de los otros le llegaba como un chat
//! normal que se llamaba igual: en el PC del usuario habia tres (el suyo,
//! el del movil y el de otro PC) y la conversacion quedaba partida en
//! trozos, «el asistente solo se acuerda de 2 o 3 mensajes». El movil ya
//! marcaba el suyo (`"asistente": true` en el proyecto, `Proyecto.asistente`
//! de Android); el PC no mandaba esa marca ni la leia.
//!
//! Ahora la marca va y viene ([`DiscoPc::proyecto_portatil`],
//! [`DiscoPc::guardar_proyecto`]) y, si aun asi hay varios, se juntan en
//! **el mas antiguo** (por `creado` y luego por id: todos los PC eligen el
//! mismo sin hablarse): sus mensajes pasan a el con sus adjuntos y los
//! otros se borran como cualquier chat (copia en la papelera, marcas y
//! lapida), y ese borrado llega a los demas aparatos, que se quedan con el
//! mismo. El movil elige el que lleva la marca (`ChatDelAsistente.buscar`).

use std::collections::HashSet;
use std::io;
use std::path::Path;

use pixpin_sincro::canonico::Json;
use pixpin_sincro::disco::{Disco, limpio};
use pixpin_sincro::kotlin;

use super::{DiscoPc, GENERAL, ahora};
use crate::almacen::{self, Ficha, NOMBRE_ASISTENTE};

/// Los ids fijos de los mensajes que el PC deja en el chat del asistente
/// (figuras, automatizaciones, letras de los planos).
fn es_de_sistema_del_asistente(id: &str) -> bool {
    id.starts_with("auto-")
}

impl DiscoPc {
    /// Si un chat es «el del asistente» de algun aparato: con la marca (aqui
    /// o en su proyecto), o llamado «Asistente» (el de otro aparato llega
    /// sin marca si es de un PC de antes).
    fn es_chat_del_asistente(&self, chat: &str, f: &Ficha) -> bool {
        if chat == GENERAL || f.es_guardados() {
            return false;
        }
        if f.es_asistente() {
            return true;
        }
        let marcado =
            std::fs::read_to_string(almacen::carpeta(&self.raiz, &f.id).join("proyecto.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|v| v.get("asistente")?.as_bool())
                == Some(true);
        marcado || f.nombre.trim().eq_ignore_ascii_case(NOMBRE_ASISTENTE)
    }

    /// **Junta los chats del asistente en uno.** Devuelve cuantos se
    /// juntaron (0 si ya habia uno solo o ninguno). El que queda lleva la
    /// marca, aunque hubiera llegado sin ella.
    pub fn unificar_asistentes(&self) -> io::Result<usize> {
        let mut candidatos: Vec<(String, Ficha)> = self
            .mapa()
            .into_iter()
            .filter(|(c, f)| self.es_chat_del_asistente(c, f))
            .collect();
        if candidatos.is_empty() {
            return Ok(0);
        }
        candidatos.sort_by(|(ca, a), (cb, b)| {
            a.creado
                .max(0)
                .cmp(&b.creado.max(0))
                .then_with(|| ca.cmp(cb))
        });
        let (destino, ficha) = candidatos.remove(0);
        if !ficha.es_asistente() {
            let mut indice = almacen::Indice::leer(&self.raiz);
            for f in indice.proyectos.iter_mut() {
                almacen::quitar_marca_asistente(f);
                if f.id == ficha.id {
                    almacen::poner_marca_asistente(f);
                }
            }
            indice.guardar(&self.raiz)?;
            self.olvidar_mapa();
        }
        let aparato = crate::codigos::de_aparato(&self.identidad()?.yo.id);
        let mut juntados = 0;
        for (origen, f) in candidatos {
            self.pasar_mensajes(&origen, &f, &destino)?;
            self.borrar_chat(
                &origen,
                "unificado con el chat del asistente",
                ahora(),
                &aparato,
            )?;
            juntados += 1;
        }
        if juntados > 0 {
            self.avisar(pixpin_sincro::disco::Cambio::Proyectos);
        }
        Ok(juntados)
    }

    /// Los mensajes de `origen` (y sus ficheros) a `destino`, en orden de
    /// fecha y numerados detras de los suyos. Lo que ya esta (mismo codigo
    /// unico, o un mensaje fijo del PC que ya tiene) no se repite.
    fn pasar_mensajes(&self, origen: &str, f: &Ficha, destino: &str) -> io::Result<()> {
        let (de_origen, _) = self.lineas(origen)?;
        if de_origen.is_empty() {
            return Ok(());
        }
        copiar_sin_pisar(
            &almacen::carpeta(&self.raiz, &f.id),
            &self.carpeta_de(destino),
        )?;
        let _cerrojo = crate::cuaderno::cerrojo();
        let (mut buenas, crudas) = self.lineas(destino)?;
        let ya: HashSet<String> = buenas.iter().map(kotlin::unico).collect();
        let mut numero = buenas
            .iter()
            .filter_map(|m| kotlin::numero(m, "numero"))
            .max()
            .unwrap_or(0);
        let mut nuevas: Vec<Json> = de_origen
            .into_iter()
            .filter(|m| {
                let id = kotlin::cadena(m, "id").unwrap_or_default();
                !ya.contains(&kotlin::unico(m))
                    && !(es_de_sistema_del_asistente(id) && ya.contains(id))
            })
            .collect();
        nuevas.sort_by_key(|m| kotlin::numero(m, "cuando").unwrap_or(0));
        let (de, a) = (
            format!("guardados/pc/{}/", limpio(origen)),
            format!("guardados/pc/{}/", limpio(destino)),
        );
        for m in &mut nuevas {
            numero += 1;
            kotlin::poner(m, "numero", Json::numero(numero));
            kotlin::poner(m, "proyecto", Json::cadena(destino));
            if let Some(r) = kotlin::cadena(m, "ruta")
                .filter(|r| r.contains(&de))
                .map(|r| r.replace(&de, &a))
            {
                kotlin::poner(m, "ruta", Json::cadena(r));
            }
        }
        buenas.extend(nuevas);
        // Todo en orden de fecha: la conversacion se lee como paso.
        buenas.sort_by_key(|m| kotlin::numero(m, "cuando").unwrap_or(0));
        self.escribir_lineas(destino, &buenas, &crudas)
    }
}

/// Copia lo de una carpeta de chat en otra (adjuntos, lienzos, `android/`)
/// sin pisar nada de lo que ya hay. El cuaderno y el proyecto no.
fn copiar_sin_pisar(de: &Path, a: &Path) -> io::Result<()> {
    fn recorrer(de: &Path, a: &Path, raiz: bool) -> io::Result<()> {
        let Ok(l) = std::fs::read_dir(de) else {
            return Ok(());
        };
        for e in l.flatten() {
            let nombre = e.file_name();
            if raiz && (nombre == "guardados.jsonl" || nombre == "proyecto.json") {
                continue;
            }
            let (origen, destino) = (e.path(), a.join(&nombre));
            if origen.is_dir() {
                recorrer(&origen, &destino, false)?;
            } else if !destino.exists() {
                std::fs::create_dir_all(a)?;
                std::fs::copy(&origen, &destino)?;
            }
        }
        Ok(())
    }
    recorrer(de, a, true)
}

#[cfg(test)]
#[path = "asistente_unico/pruebas.rs"]
mod pruebas;
