//! **Copias de seguridad y proyectos borrados** (`sincronizar/copias_ui.rs`
//! de Windows, copia de `sincro/CopiasActivity.kt` del movil).
//!
//! Las copias se hacen solas antes de recibir algo por Wi-Fi y antes de cada
//! vuelta de sincronizar (`pixpin_sincro::copias`, el mismo crate que
//! Windows): viven en `copias/`, fuera de lo que viaja. Aqui se listan y se
//! vuelve a cualquiera. **Volver no borra lo hecho despues** y se puede
//! deshacer, porque antes de volver se guarda otra copia.
//!
//! Y **«Proyectos borrados»**: lo que esta en la papelera de PixPin
//! (`pixpin_proyecto::almacen::en_papelera`) se recupera entero —nombre,
//! hojas en su orden, PDF y chat— y deja de estar borrado para sincronizar.
//!
//! Como en Windows, en el PC (y en el Mac) un lienzo vive dentro de la
//! carpeta de su proyecto: no hay «lienzos sin proyecto» que recoger.

use pixpin_proyecto::almacen::{self, EnPapelera};
use pixpin_proyecto::vista::DiscoPc;
use pixpin_sincro::copias::{self, Copia};

use crate::{ErrorNucleo, Nucleo};

/// Una copia de seguridad de un chat, la mas nueva primero en la lista.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CopiaDeSeguridad {
    /// Su id dentro de su chat; con `chat` dice cual es.
    pub id: String,
    /// El chat de sincronizar del que es.
    pub chat: String,
    /// Cuando se hizo, en milisegundos UTC.
    pub cuando: i64,
    /// Por que se hizo («Antes de sincronizar con Tableta»).
    pub motivo: String,
    /// El nombre del proyecto entonces.
    pub nombre: String,
    pub hojas: u32,
    pub mensajes: u32,
    /// Si algun archivo (el PDF de mas de 40 MB) se quedo sin copiar: de
    /// ese no se puede volver atras.
    pub sin_grandes: bool,
}

impl From<&Copia> for CopiaDeSeguridad {
    fn from(c: &Copia) -> Self {
        CopiaDeSeguridad {
            id: c.id.clone(),
            chat: c.chat.clone(),
            cuando: c.cuando,
            motivo: c.motivo.clone(),
            nombre: c.nombre.clone(),
            hojas: c.hojas as u32,
            mensajes: c.mensajes.len() as u32,
            sin_grandes: !c.sin_copiar.is_empty(),
        }
    }
}

/// Un proyecto en la papelera de PixPin.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ProyectoBorrado {
    /// El nombre de su carpeta en la papelera (`<id>-<ms>`): con el se
    /// recupera.
    pub carpeta: String,
    pub id: String,
    pub nombre: String,
    pub hojas: u32,
    pub mensajes: u32,
    /// Cuando se borro, en milisegundos UTC.
    pub borrado: i64,
}

impl From<&EnPapelera> for ProyectoBorrado {
    fn from(e: &EnPapelera) -> Self {
        ProyectoBorrado {
            carpeta: e
                .carpeta
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            id: e.ficha.id.clone(),
            nombre: e.ficha.nombre.clone(),
            hojas: e.ficha.hojas,
            mensajes: e.mensajes as u32,
            borrado: e.borrado,
        }
    }
}

/// Como acabo «Recuperar todos».
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct Recuperados {
    pub n: u32,
    pub total: u32,
}

#[uniffi::export]
impl Nucleo {
    /// **Las copias de seguridad de todos los chats**, la mas nueva primero
    /// (`Copias.todas`). Se guardan las 30 ultimas de cada uno.
    pub fn copias_de_seguridad(&self) -> Vec<CopiaDeSeguridad> {
        copias::todas(&DiscoPc::nuevo(self.ruta()))
            .iter()
            .map(CopiaDeSeguridad::from)
            .collect()
    }

    /// **Vuelve a la copia `id` del chat `chat`** (`Copias.restaurar`):
    /// antes guarda como esta ahora (asi se deshace), pone los archivos y los
    /// mensajes de entonces y **conserva los lienzos de despues**. Devuelve
    /// el nombre del proyecto. Una copia que ya no esta (la lista se releyo)
    /// no restaura otra cualquiera: da `NoExiste`.
    pub fn volver_a_copia(&self, chat: String, id: String) -> Result<String, ErrorNucleo> {
        let disco = DiscoPc::nuevo(self.ruta());
        let copia = copias::lista(&disco, &chat)
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| ErrorNucleo::NoExiste {
                que: format!("la copia {id}"),
            })?;
        copias::restaurar(&disco, &copia, crate::ahora_ms())?;
        self.vaciar_caches();
        Ok(copia.nombre)
    }

    /// **Los proyectos de la papelera**, el borrado mas reciente primero.
    pub fn proyectos_borrados(&self) -> Vec<ProyectoBorrado> {
        almacen::en_papelera(self.ruta())
            .iter()
            .map(ProyectoBorrado::from)
            .collect()
    }

    /// **Recupera un proyecto de la papelera** por el nombre de su carpeta
    /// ([`ProyectoBorrado::carpeta`]): vuelve a la lista entero y deja de
    /// estar borrado para sincronizar (se quita su lapida). Devuelve su
    /// nombre. Sin dialogo, como en el movil: recuperar no pisa ni borra.
    pub fn recuperar_proyecto(&self, carpeta: String) -> Result<String, ErrorNucleo> {
        let en = almacen::en_papelera(self.ruta())
            .into_iter()
            .find(|e| e.carpeta.file_name().is_some_and(|n| n == carpeta.as_str()))
            .ok_or_else(|| ErrorNucleo::NoExiste {
                que: format!("«{carpeta}» en la papelera"),
            })?;
        let ficha = almacen::recuperar(self.ruta(), &en, crate::ahora_ms())?;
        self.vaciar_caches();
        Ok(ficha.nombre)
    }

    /// **«Recuperar todos»**: cuantos volvieron de cuantos habia.
    pub fn recuperar_todos(&self) -> Recuperados {
        let todos = almacen::en_papelera(self.ruta());
        let ahora = crate::ahora_ms();
        let n = todos
            .iter()
            .filter(|en| almacen::recuperar(self.ruta(), en, ahora).is_ok())
            .count();
        self.vaciar_caches();
        Recuperados {
            n: n as u32,
            total: todos.len() as u32,
        }
    }
}
