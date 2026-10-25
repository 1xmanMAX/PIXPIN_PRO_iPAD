//! **Cambiar las tareas desde la ventana de Tareas** (G1 y el Kanban de
//! Windows, 17-oct): corregir el texto (F2), borrar con deshacer y mover a
//! otra lista (arrastrar una tarjeta a otra columna, «Mover a…»).
//!
//! Como al marcar: cada operacion lleva el `crudo` que se ve en pantalla y,
//! si la tarea numero `indice` ya no es esa (la lista cambio en el chat o
//! llego cambiada del movil), no se toca nada y se devuelve `false`. Todo
//! reescribe el documento de la mini-app `tareas` con `mini`, que es lo que
//! viaja al movil al sincronizar.

use pixpin_proyecto::{cuaderno, mini};

use super::{es_lista, frase};
use crate::{ErrorNucleo, Nucleo};

/// Que paso al intentar un cambio en una lista.
enum Hecho {
    /// Se cambio (o no hacia falta).
    Si,
    /// La tarea ya no es la que se ve: hay que releer.
    Cambio,
}

impl Nucleo {
    /// Cambia las tareas de la lista `lista` del chat `chat` con `f`, que
    /// devuelve `None` si la tarea esperada ya no esta, o `Some(cambio)`.
    fn con_tareas(
        &self,
        chat: &str,
        lista: &str,
        mut f: impl FnMut(&mut Vec<mini::Tarea>) -> Option<bool>,
    ) -> Result<Hecho, ErrorNucleo> {
        let carpeta = self.carpeta_de_chat(chat)?;
        let mut era_lista = true;
        let mut hecho = Hecho::Cambio;
        let r = cuaderno::cambiar(&carpeta, lista, |m| {
            if !es_lista(m) {
                era_lista = false;
                return false;
            }
            let mut doc = mini::leer_tareas(&m.texto);
            match f(&mut doc) {
                None => false,
                Some(cambia) => {
                    hecho = Hecho::Si;
                    if cambia {
                        m.texto = mini::escribir_tareas(&mini::titulo(&m.texto), &doc);
                    }
                    cambia
                }
            }
        });
        match r {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ErrorNucleo::NoExiste { que: lista.to_string() });
            }
            otro => {
                if otro?.is_none() {
                    return Err(ErrorNucleo::NoExiste { que: lista.to_string() });
                }
            }
        }
        if !era_lista {
            return Err(ErrorNucleo::Argumento {
                detalle: format!("no es una lista de tareas: {lista}"),
            });
        }
        self.soltar_cuaderno(&carpeta);
        Ok(hecho)
    }
}

/// La tarea `indice` si su texto es `crudo`.
fn la_que_se_ve<'a>(doc: &'a mut [mini::Tarea], indice: u32, crudo: &str) -> Option<&'a mut mini::Tarea> {
    doc.get_mut(indice as usize).filter(|t| t.texto == crudo)
}

/// El texto nuevo de una tarea corregida: lo escrito, entendido como al
/// apuntar (prioridad, #etiquetas, hora), con la fecha de creacion, las
/// imagenes y la nota de antes.
fn corregido(crudo: &str, nuevo: &str, desfase_ms: i64) -> String {
    let (visible, creada) = mini::partir(crudo);
    let (_, imagenes) = mini::imagenes_de(visible);
    let antes = frase::marcas(crudo);
    let f = frase::leer(nuevo, frase::ahora(desfase_ms));
    let mut texto = if f.titulo.is_empty() { nuevo.trim().to_string() } else { f.texto() };
    // Una hora que ya tenia y no se ha vuelto a decir, se queda.
    if f.hora.is_none()
        && let Some(h) = antes.hora
    {
        texto = super::con_hora(&texto, frase::ms_de(h));
    }
    let mut s = mini::con_imagenes(&texto, &imagenes);
    if let Some(n) = antes.nota {
        s = frase::con_nota(&s, &n);
    }
    match creada {
        Some(fecha) => mini::con_fecha(&s, fecha),
        None => s,
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Corrige el texto de una tarea** (F2 de Windows). `false`: la lista
    /// cambio y hay que releer. Un texto vacio no se acepta (para eso,
    /// borrar).
    pub fn editar_tarea(
        &self,
        chat: String,
        lista: String,
        indice: u32,
        crudo: String,
        nuevo: String,
        desfase_ms: i64,
    ) -> Result<bool, ErrorNucleo> {
        if mini::saneado(&nuevo).is_empty() {
            return Err(ErrorNucleo::Argumento { detalle: "tarea vacia".into() });
        }
        let r = self.con_tareas(&chat, &lista, |doc| {
            let t = la_que_se_ve(doc, indice, &crudo)?;
            let otro = corregido(&t.texto, &nuevo, desfase_ms);
            let cambia = otro != t.texto;
            t.texto = otro;
            Some(cambia)
        })?;
        Ok(matches!(r, Hecho::Si))
    }

    /// **Borra una tarea** de su lista. `false`: la lista cambio. Para
    /// deshacerlo, [`Nucleo::devolver_tarea`] con lo mismo.
    pub fn borrar_tarea(
        &self,
        chat: String,
        lista: String,
        indice: u32,
        crudo: String,
    ) -> Result<bool, ErrorNucleo> {
        let r = self.con_tareas(&chat, &lista, |doc| {
            la_que_se_ve(doc, indice, &crudo)?;
            doc.remove(indice as usize);
            Some(true)
        })?;
        Ok(matches!(r, Hecho::Si))
    }

    /// **Deshace un borrado**: vuelve a poner la tarea en su sitio (o al
    /// final, si la lista ya es mas corta).
    pub fn devolver_tarea(
        &self,
        chat: String,
        lista: String,
        indice: u32,
        crudo: String,
        hecha: bool,
    ) -> Result<(), ErrorNucleo> {
        self.con_tareas(&chat, &lista, |doc| {
            let i = (indice as usize).min(doc.len());
            doc.insert(i, mini::Tarea { texto: crudo.clone(), hecha });
            Some(true)
        })?;
        Ok(())
    }

    /// **Pasa una tarea a otra lista** (el Kanban y «Mover a…» de Windows,
    /// `tareas::mover`): se anade al final de la de destino y se quita de la
    /// suya. `false`: la de origen cambio. A la misma lista no hace nada.
    pub fn mover_tarea(
        &self,
        chat: String,
        lista: String,
        indice: u32,
        crudo: String,
        a_chat: String,
        a_lista: String,
    ) -> Result<bool, ErrorNucleo> {
        if chat == a_chat && lista == a_lista {
            return Ok(true);
        }
        // Primero se mira que la de origen sigue igual, sin tocarla.
        let mut hecha = None;
        self.con_tareas(&chat, &lista, |doc| {
            hecha = Some(la_que_se_ve(doc, indice, &crudo)?.hecha);
            Some(false)
        })?;
        let Some(hecha) = hecha else { return Ok(false) };
        // Despues, al destino; si no es una lista, se para aqui.
        self.con_tareas(&a_chat, &a_lista, |doc| {
            doc.push(mini::Tarea { texto: crudo.clone(), hecha });
            Some(true)
        })?;
        // Y se quita del origen. Si entretanto cambio, se quita la copia
        // del destino para no dejarla dos veces.
        if self.borrar_tarea(chat, lista, indice, crudo.clone())? {
            return Ok(true);
        }
        let _ = self.con_tareas(&a_chat, &a_lista, |doc| {
            let i = doc.iter().rposition(|t| t.texto == crudo)?;
            doc.remove(i);
            Some(true)
        });
        Ok(false)
    }
}

/// Una tarea encontrada por su titulo (el pin de una tarea, F6).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TareaDelPin {
    pub indice: u32,
    pub crudo: String,
    pub hecha: bool,
}

#[uniffi::export]
impl Nucleo {
    /// **La tarea de un pin**, buscada por su titulo (sin marcas) y no por
    /// su numero: el chat o el movil pueden reordenar la lista mientras el
    /// pin esta en la pantalla (`pines::tarea` de Windows). `None` si ya no
    /// esta.
    pub fn tarea_por_titulo(
        &self,
        chat: String,
        lista: String,
        titulo: String,
    ) -> Result<Option<TareaDelPin>, ErrorNucleo> {
        let mut hallada = None;
        self.con_tareas(&chat, &lista, |doc| {
            hallada = doc
                .iter()
                .enumerate()
                .find(|(_, t)| frase::marcas(&t.texto).titulo == titulo)
                .map(|(i, t)| TareaDelPin { indice: i as u32, crudo: t.texto.clone(), hecha: t.hecha });
            Some(false)
        })?;
        Ok(hallada)
    }

    /// **Pone la nota** de por que no se esta haciendo (vacia: la quita) en
    /// la tarea de titulo `titulo`. Va dentro de la tarea, tras `💬`, asi
    /// que viaja al movil. Devuelve el texto crudo nuevo, o `None` si ya no
    /// esta.
    pub fn poner_nota_tarea(
        &self,
        chat: String,
        lista: String,
        titulo: String,
        nota: String,
    ) -> Result<Option<String>, ErrorNucleo> {
        let mut nueva = None;
        self.con_tareas(&chat, &lista, |doc| {
            let t = doc.iter_mut().find(|t| frase::marcas(&t.texto).titulo == titulo)?;
            let otro = frase::con_nota(&t.texto, &nota);
            let cambia = otro != t.texto;
            t.texto = otro.clone();
            nueva = Some(otro);
            Some(cambia)
        })?;
        Ok(nueva)
    }
}
