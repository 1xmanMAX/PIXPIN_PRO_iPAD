//! **Las carpetas de paginas del chat** («Dia 1, Dia 2…», Windows
//! `ventana_chat/carpetas.rs`, 26-oct-2026).
//!
//! El formato es el de Windows y viaja igual (`pixpin_proyecto::
//! carpeta_de_paginas`, copiado tal cual): un mensaje `MINIAPP` con la
//! palabra `carpeta` y la lista ordenada de los `id` de sus paginas en
//! `texto`. Las paginas siguen siendo mensajes normales del chat.
//!
//! Aqui va lo que Windows hace sin ventana: crear una carpeta (con nombre
//! libre, «Cuaderno», «Cuaderno 2»…, y si se pide con una primera pagina),
//! cambiarla en el disco con el cerrojo del cuaderno (anadir, quitar, mover,
//! renombrar; lo borrado se poda de paso) y la **pagina siguiente**: un
//! lienzo nuevo con la plantilla de la ultima pagina, vacia, y el titulo
//! siguiente («Dia 3» tras «Dia 2»).
//!
//! # La plantilla de la pagina siguiente
//!
//! Windows la encuentra con `hoja_de_plantilla` (que conoce las plantillas
//! de serie, `figuras_de_serie`, y su `bloques`). Esos dos modulos no estan
//! en el nucleo de Apple (son 25 000 lineas de figuras de serie); aqui se
//! hace lo mismo con la regla de `bloques::leer` que importa: una plantilla
//! es un grupo estampado (el grupo mas exterior) cuya **pagina** —el
//! rectangulo mas grande que encierra casi todo el grupo— mide al menos lo
//! que pide Windows a una propia (300 x 300). Lo escrito encima no es del
//! grupo y no se copia. Lo que se tecleo dentro de los textos de la
//! plantilla si se copia (Windows lo limpia volviendo a la de serie).

use pixpin_motor2d::biblioteca::{self, FiguraGuardada};
use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::excalidraw;
use pixpin_motor2d::vector::Punto2;
use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::carpeta_de_paginas::{self as carpetas, Carpeta};
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje, Sello};

use crate::chats::{MensajeChat, hora_unica, validar_chat};
use crate::{ErrorNucleo, Nucleo};

/// Una pagina de una carpeta: el `id` de su mensaje y su titulo en ella
/// (vacio: se ensena el nombre del mensaje).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaDeCarpeta {
    pub id: String,
    pub titulo: String,
}

/// Lo que se le hace a una carpeta. Las paginas van por el `id` de su
/// mensaje (no por su puesto: el chat puede haber cambiado entre medias).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum CambioCarpeta {
    /// Al final, con el titulo que le toca (`titulo_al_anadir`): el de la
    /// serie («Dia 3»), o `nombre_de_la_hoja`, o `por_defecto` y su puesto.
    Anadir {
        pagina: String,
        nombre_de_la_hoja: String,
        por_defecto: String,
    },
    /// La quita de la carpeta (el mensaje sigue en el chat).
    Quitar { pagina: String },
    /// La lleva al puesto `hasta` (desde 0; de mas, al final).
    Mover { pagina: String, hasta: u32 },
    Renombrar { pagina: String, titulo: String },
}

/// Como quedo una carpeta tras un cambio.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CarpetaCambiada {
    /// El mensaje de la carpeta como quedo (para la tarjeta del chat).
    pub mensaje: MensajeChat,
    pub paginas: Vec<PaginaDeCarpeta>,
    /// Si cambio algo.
    pub cambio: bool,
    /// Al anadir: si ya estaba (entonces no se toco nada).
    pub ya_estaba: bool,
    /// Al anadir: el titulo con que entro (o el que ya tenia).
    pub titulo: String,
}

/// Lo que deja la pagina siguiente.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaNuevaDeCarpeta {
    /// El lienzo nuevo, ya en el chat.
    pub lienzo: MensajeChat,
    /// La carpeta con el dentro.
    pub carpeta: MensajeChat,
    pub titulo: String,
    /// Si lleva la plantilla de la ultima pagina (si no, en blanco).
    pub con_plantilla: bool,
}

fn paginas_de(c: &Carpeta) -> Vec<PaginaDeCarpeta> {
    c.paginas
        .iter()
        .map(|p| PaginaDeCarpeta {
            id: p.id.clone(),
            titulo: p.titulo.clone(),
        })
        .collect()
}

/// La palabra de `miniapp` de una carpeta (`"carpeta"`).
#[uniffi::export]
pub fn carpeta_miniapp() -> String {
    carpetas::MINIAPP.to_string()
}

/// **Las paginas de una carpeta** por el documento de su mensaje (su
/// `texto`), en orden. `None` si no se entiende (no se reescribe).
#[uniffi::export]
pub fn carpeta_paginas(documento: String) -> Option<Vec<PaginaDeCarpeta>> {
    Carpeta::leer(&documento).ok().map(|c| paginas_de(&c))
}

/// El titulo que sigue a `titulo` («Dia 09» → «Dia 10»), o nada si no lleva
/// numero.
#[uniffi::export]
pub fn carpeta_siguiente_titulo(titulo: String) -> Option<String> {
    carpetas::siguiente_titulo(&titulo)
}

/// Un nombre que no repite el de otra carpeta: `base`, o `base 2`, `base 3`…
pub(crate) fn nombre_libre(base: &str, mensajes: &[Mensaje]) -> String {
    let usados: Vec<&str> = carpetas::carpetas_de(mensajes)
        .into_iter()
        .map(|i| mensajes[i].nombre.trim())
        .collect();
    if !usados.contains(&base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|n| !usados.contains(&n.as_str()))
        .unwrap_or_else(|| base.to_string())
}

fn area(c: (f32, f32, f32, f32)) -> f32 {
    (c.2 - c.0).max(0.0) * (c.3 - c.1).max(0.0)
}

fn dentro(p: (f32, f32), c: (f32, f32, f32, f32)) -> bool {
    p.0 >= c.0 && p.0 <= c.2 && p.1 >= c.1 && p.1 <= c.3
}

fn centro(c: (f32, f32, f32, f32)) -> (f32, f32) {
    ((c.0 + c.2) / 2.0, (c.1 + c.3) / 2.0)
}

/// Lo minimo que mide la pagina de una plantilla (el `AREA_MINIMA` de
/// Windows): un simbolo agrupado con su recuadro no pasa por hoja.
const AREA_MINIMA: f32 = 300.0 * 300.0;

/// La pagina de unas piezas (`bloques::leer` de Windows): el rectangulo
/// mas grande que tiene dentro el centro de ocho de cada diez de las demas.
fn pagina_de(piezas: &[Elemento]) -> Option<(f32, f32, f32, f32)> {
    let vivos: Vec<&Elemento> = piezas.iter().filter(|e| !e.borrado).collect();
    if vivos.len() < 2 {
        return None;
    }
    vivos
        .iter()
        .filter(|e| matches!(e.figura, Figura::Rectangulo))
        .filter(|p| {
            let c = p.caja();
            let n = vivos
                .iter()
                .filter(|o| o.id != p.id && dentro(centro(o.caja()), c))
                .count();
            n * 10 >= (vivos.len() - 1) * 8
        })
        .map(|p| p.caja())
        .max_by(|a, b| area(*a).total_cmp(&area(*b)))
}

/// **La plantilla vacia de la ultima hoja de `escena`**, lista para
/// estampar (`hoja_de_plantilla::vacia` de Windows, sin las de serie): las
/// piezas del ultimo grupo exterior con pagina, en orden de lectura.
pub(crate) fn plantilla_de(escena: &Escena) -> Option<FiguraGuardada> {
    let mut grupos: Vec<(String, Vec<Elemento>)> = Vec::new();
    for e in escena.a_la_vista() {
        let Some(g) = e.grupos.last() else { continue };
        match grupos.iter_mut().find(|(n, _)| n == g) {
            Some((_, v)) => v.push(e.clone()),
            None => grupos.push((g.clone(), vec![e.clone()])),
        }
    }
    let mut halladas: Vec<((f32, f32, f32, f32), String, Vec<Elemento>)> = grupos
        .into_iter()
        .filter_map(|(g, piezas)| {
            let caja = pagina_de(&piezas)?;
            (area(caja) >= AREA_MINIMA).then_some((caja, g, piezas))
        })
        .collect();
    halladas.sort_by(|a, b| a.0.1.total_cmp(&b.0.1).then(a.0.0.total_cmp(&b.0.0)));
    let (_, grupo, piezas) = halladas.pop()?;
    let piezas: Vec<Elemento> = piezas
        .into_iter()
        .map(|mut e| {
            // El grupo de la estampacion se quita: el lienzo nuevo pone el
            // suyo al estampar. Los de dentro de la plantilla se quedan.
            if e.grupos.last() == Some(&grupo) {
                e.grupos.pop();
            }
            e
        })
        .collect();
    Some(FiguraGuardada {
        id: biblioteca::id_nuevo(),
        nombre: String::new(),
        elementos: biblioteca::normalizar(&piezas),
    })
}

/// **Un lienzo nuevo con la plantilla vacia de `escena`**, con el mismo
/// papel (`hoja_de_plantilla::lienzo_siguiente`). `None` sin plantilla.
pub(crate) fn lienzo_siguiente(escena: &Escena) -> Option<Escena> {
    let f = plantilla_de(escena)?;
    let mut nueva = Escena::nueva();
    nueva.fondo = escena.fondo;
    let (w, h) = f.tamano();
    biblioteca::estampar_en_escena(&mut nueva, &f, Punto2::nuevo(w / 2.0, h / 2.0));
    Some(nueva)
}

impl Nucleo {
    fn mensajes_de(&self, chat: &str) -> Result<Vec<Mensaje>, ErrorNucleo> {
        let carpeta = almacen::carpeta(self.ruta(), chat);
        match cuaderno::Cuaderno::leer_de(&carpeta) {
            Ok(c) => Ok(c.mensajes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.into()),
        }
    }

    /// `cambiar_en_disco` de Windows, quedandose con el mensaje como quedo.
    fn cambiar_carpeta_en_disco(
        &self,
        chat: &str,
        id: &str,
        cambio: impl FnOnce(&mut Carpeta) -> bool,
    ) -> Result<Option<(Mensaje, Carpeta, bool)>, ErrorNucleo> {
        let dir = almacen::carpeta(self.ruta(), chat);
        let existen: std::collections::HashSet<String> =
            self.mensajes_de(chat)?.into_iter().map(|m| m.id).collect();
        let mut leida: Option<(Carpeta, bool)> = None;
        let r = cuaderno::cambiar(&dir, id, |m| {
            let Some(mut c) = Carpeta::de_mensaje(m) else {
                return false;
            };
            let podadas = c.podar(|p| existen.contains(p));
            let cambiada = cambio(&mut c);
            let escribir = cambiada || podadas > 0;
            if escribir {
                m.texto = c.escribir();
            }
            leida = Some((c, cambiada));
            escribir
        });
        let m = match r {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            otro => otro?,
        };
        self.cache_tomada().quitar(&dir.join("guardados.jsonl"));
        Ok(match (m, leida) {
            (Some(m), Some((c, cambiada))) => Some((m, c, cambiada)),
            _ => None,
        })
    }

    /// Un mensaje nuevo al final del cuaderno, con su numero, y el chat
    /// arriba en la lista.
    fn anadir_al_chat(&self, chat: &str, m: &Mensaje) -> Result<(), ErrorNucleo> {
        let raiz = self.ruta();
        let mut indice = Indice::leer_si_esta(raiz)?;
        let carpeta = almacen::carpeta(raiz, chat);
        cuaderno::anadir(&carpeta, m)?;
        self.cache_tomada().quitar(&carpeta.join("guardados.jsonl"));
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == chat) {
            f.tocado = m.cuando;
            f.resumen = m.resumen();
        }
        indice.guardar(raiz)?;
        Ok(())
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Una carpeta nueva al final del chat** (`carpetas::crear` de
    /// Windows). Sin `nombre`, `base` sin repetir el de otra («Cuaderno»,
    /// «Cuaderno 2»…). Con `primera`, la lleva dentro con el titulo que le
    /// toca («Nueva carpeta con esta pagina»).
    pub fn crear_carpeta(
        &self,
        chat: String,
        nombre: Option<String>,
        base: String,
        primera: Option<String>,
        nombre_de_la_hoja: String,
        por_defecto: String,
    ) -> Result<MensajeChat, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta();
        if Indice::leer_si_esta(raiz)?.buscar(&chat).is_none() {
            return Err(ErrorNucleo::NoExiste { que: chat });
        }
        let mensajes = self.mensajes_de(&chat)?;
        let nombre = match nombre.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => n.to_string(),
            None => nombre_libre(base.trim(), &mensajes),
        };
        let numero = mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
        // El id es el milisegundo: que no sea el de otro mensaje.
        let mut cuando = hora_unica();
        while mensajes.iter().any(|m| m.id == cuando.to_string()) {
            cuando = hora_unica();
        }
        let mut m = carpetas::mensaje_nuevo(
            &nombre,
            &Sello {
                cuando,
                numero,
                aparato: self.identidad()?.yo.codigo(),
                proyecto: chat.clone(),
            },
        );
        if let Some(p) = primera
            .as_deref()
            .and_then(|id| mensajes.iter().find(|x| x.id == id))
            .filter(|p| carpetas::puede_ir(p))
        {
            let mut c = Carpeta::default();
            let titulo = c.titulo_al_anadir(&nombre_de_la_hoja, &por_defecto);
            c.anadir(&p.id, &titulo);
            m.texto = c.escribir();
        }
        self.anadir_al_chat(&chat, &m)?;
        Ok(MensajeChat::from(m))
    }

    /// **Cambia la carpeta `carpeta`** (el `id` de su mensaje) en el disco,
    /// con el cerrojo del cuaderno. Lo borrado del chat se quita de paso.
    /// `NoExiste` si la carpeta ya no esta o no se entiende.
    pub fn cambiar_carpeta(
        &self,
        chat: String,
        carpeta: String,
        cambio: CambioCarpeta,
    ) -> Result<CarpetaCambiada, ErrorNucleo> {
        validar_chat(&chat)?;
        let mut ya_estaba = false;
        let mut titulo = String::new();
        let hecho = self.cambiar_carpeta_en_disco(&chat, &carpeta, |c| match &cambio {
            CambioCarpeta::Anadir {
                pagina,
                nombre_de_la_hoja,
                por_defecto,
            } => {
                if let Some(i) = c.posicion(pagina) {
                    ya_estaba = true;
                    titulo = c.paginas[i].titulo.clone();
                    return false;
                }
                titulo = c.titulo_al_anadir(nombre_de_la_hoja, por_defecto);
                c.anadir(pagina, &titulo)
            }
            CambioCarpeta::Quitar { pagina } => c.quitar(pagina),
            CambioCarpeta::Mover { pagina, hasta } => match c.posicion(pagina) {
                Some(i) => c.mover(i, *hasta as usize),
                None => false,
            },
            CambioCarpeta::Renombrar { pagina, titulo } => {
                c.posicion(pagina).is_some_and(|i| c.renombrar(i, titulo))
            }
        })?;
        let Some((m, c, cambio)) = hecho else {
            return Err(ErrorNucleo::NoExiste { que: carpeta });
        };
        Ok(CarpetaCambiada {
            mensaje: MensajeChat::from(m),
            paginas: paginas_de(&c),
            cambio,
            ya_estaba,
            titulo,
        })
    }

    /// **«Nueva pagina»** de una carpeta (`pagina_siguiente` de Windows):
    /// un lienzo nuevo en el chat con la plantilla de la ultima pagina que
    /// sea un lienzo, vacia, o en blanco si no tiene; con el titulo
    /// siguiente («Dia 3» tras «Dia 2»; sin serie, `por_defecto` y su
    /// puesto), y metido al final de la carpeta.
    pub fn pagina_siguiente_de_carpeta(
        &self,
        chat: String,
        carpeta: String,
        por_defecto: String,
    ) -> Result<PaginaNuevaDeCarpeta, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta().to_path_buf();
        let mensajes = self.mensajes_de(&chat)?;
        let mut c = mensajes
            .iter()
            .find(|m| m.id == carpeta)
            .and_then(Carpeta::de_mensaje)
            .ok_or_else(|| ErrorNucleo::NoExiste {
                que: carpeta.clone(),
            })?;
        c.podar(|id| mensajes.iter().any(|m| m.id == id));
        let plantilla = c
            .vivas(&mensajes)
            .into_iter()
            .rev()
            .map(|(i, _)| &mensajes[i])
            .find(|m| m.clase == Some(Clase::Dibujo))
            .and_then(|m| m.referencia.clone())
            .and_then(|r| {
                let texto = std::fs::read_to_string(almacen::lienzo(&raiz, &chat, &r)).ok()?;
                let lienzo = excalidraw::leer(&texto).ok()?;
                lienzo_siguiente(&excalidraw::a_escena(&lienzo))
            });
        let titulo = c.titulo_siguiente(&por_defecto);
        let m = self.crear_lienzo_con(chat.clone(), Some(titulo.clone()), |_| {})?;
        if let (Some(escena), Some(r)) = (&plantilla, m.referencia.as_deref()) {
            let lienzo = excalidraw::con_escena(&excalidraw::Lienzo::vacio(), escena);
            std::fs::write(
                almacen::lienzo(&raiz, &chat, r),
                excalidraw::escribir(&lienzo),
            )?;
        }
        let id = m.id.clone();
        let t = titulo.clone();
        let hecho = self.cambiar_carpeta_en_disco(&chat, &carpeta, |c| c.anadir(&id, &t))?;
        let Some((cm, ..)) = hecho else {
            return Err(ErrorNucleo::NoExiste { que: carpeta });
        };
        Ok(PaginaNuevaDeCarpeta {
            lienzo: MensajeChat::from(m),
            carpeta: MensajeChat::from(cm),
            titulo,
            con_plantilla: plantilla.is_some(),
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::elemento::Elemento;

    fn rect(id: u64, x: f32, y: f32, w: f32, h: f32, grupo: &str) -> Elemento {
        let mut e = Elemento {
            id,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho: w,
            alto: h,
            semilla: 7,
            ..Default::default()
        };
        if !grupo.is_empty() {
            e.grupos = vec![grupo.to_string()];
        }
        e
    }

    #[test]
    fn la_plantilla_es_el_grupo_con_pagina_sin_lo_escrito_encima() {
        let mut escena = Escena::nueva();
        // La hoja: un A4 con dos recuadros, agrupados.
        escena.anadir(rect(1, 0.0, 0.0, 794.0, 1123.0, "g1"));
        escena.anadir(rect(2, 40.0, 40.0, 300.0, 100.0, "g1"));
        escena.anadir(rect(3, 40.0, 200.0, 300.0, 100.0, "g1"));
        // Lo escrito encima, sin grupo.
        escena.anadir(rect(4, 60.0, 60.0, 20.0, 20.0, ""));
        let f = plantilla_de(&escena).unwrap();
        assert_eq!(f.elementos.len(), 3, "sin lo de encima");
        assert!(f.elementos.iter().all(|e| e.grupos.is_empty()), "sin el grupo de fuera");
        let nueva = lienzo_siguiente(&escena).unwrap();
        assert_eq!(nueva.a_la_vista().count(), 3);
        // Caso negativo: un simbolo pequeno agrupado no es una hoja.
        let mut chica = Escena::nueva();
        chica.anadir(rect(1, 0.0, 0.0, 100.0, 100.0, "g"));
        chica.anadir(rect(2, 10.0, 10.0, 20.0, 20.0, "g"));
        assert!(plantilla_de(&chica).is_none());
        assert!(plantilla_de(&Escena::nueva()).is_none());
    }

    #[test]
    fn crear_anadir_mover_y_la_pagina_siguiente_en_el_disco() {
        let d = std::env::temp_dir().join(format!("pixpin-nucleo-carpetas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let n = Nucleo::abrir(d.display().to_string(), "iPad".into()).unwrap();
        let chat = n.crear_chat("Diario".into()).unwrap().id;
        let a = n.crear_lienzo(chat.clone(), None).unwrap();
        let c = n
            .crear_carpeta(chat.clone(), None, "Cuaderno".into(), Some(a.id.clone()), "".into(), "Página".into())
            .unwrap();
        assert_eq!(c.nombre, "Cuaderno");
        assert_eq!(c.clase, "MINIAPP");
        assert_eq!(c.miniapp.as_deref(), Some("carpeta"));
        let p = carpeta_paginas(c.texto.clone()).unwrap();
        assert_eq!(p[0].titulo, "Página 1");
        let otra = n
            .crear_carpeta(chat.clone(), None, "Cuaderno".into(), None, "".into(), "Página".into())
            .unwrap();
        assert_eq!(otra.nombre, "Cuaderno 2", "no repite el nombre");

        let nueva = n
            .pagina_siguiente_de_carpeta(chat.clone(), c.id.clone(), "Página".into())
            .unwrap();
        assert_eq!(nueva.titulo, "Página 2");
        assert!(!nueva.con_plantilla, "un lienzo en blanco no tiene plantilla");
        assert_eq!(nueva.lienzo.nombre, "Página 2");
        let ids: Vec<String> = carpeta_paginas(nueva.carpeta.texto.clone())
            .unwrap()
            .into_iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(ids, [a.id.clone(), nueva.lienzo.id.clone()]);

        let r = n
            .cambiar_carpeta(chat.clone(), c.id.clone(), CambioCarpeta::Mover { pagina: nueva.lienzo.id.clone(), hasta: 0 })
            .unwrap();
        assert!(r.cambio);
        assert_eq!(r.paginas[0].id, nueva.lienzo.id);
        let r = n
            .cambiar_carpeta(
                chat.clone(),
                c.id.clone(),
                CambioCarpeta::Anadir { pagina: a.id.clone(), nombre_de_la_hoja: "".into(), por_defecto: "Página".into() },
            )
            .unwrap();
        assert!(r.ya_estaba && !r.cambio);
        let r = n
            .cambiar_carpeta(chat.clone(), c.id.clone(), CambioCarpeta::Renombrar { pagina: a.id.clone(), titulo: "Día 1".into() })
            .unwrap();
        assert_eq!(r.paginas[1].titulo, "Día 1");
        let r = n
            .cambiar_carpeta(chat.clone(), c.id.clone(), CambioCarpeta::Quitar { pagina: a.id.clone() })
            .unwrap();
        assert_eq!(r.paginas.len(), 1);
        // Casos negativos: lo que no es carpeta, o no esta.
        assert!(n.cambiar_carpeta(chat.clone(), a.id.clone(), CambioCarpeta::Quitar { pagina: "x".into() }).is_err());
        assert!(n.cambiar_carpeta(chat.clone(), "nadie".into(), CambioCarpeta::Quitar { pagina: "x".into() }).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn el_nombre_de_una_carpeta_nueva_no_se_repite() {
        let s = |n| Sello {
            cuando: 1_761_400_000_000 + n,
            numero: n,
            aparato: "K7Q2".into(),
            proyecto: "p".into(),
        };
        let mensajes = vec![
            carpetas::mensaje_nuevo("Cuaderno", &s(1)),
            carpetas::mensaje_nuevo("Cuaderno 2", &s(2)),
        ];
        assert_eq!(nombre_libre("Cuaderno", &mensajes), "Cuaderno 3");
        assert_eq!(nombre_libre("Diario", &mensajes), "Diario");
        let p = carpeta_paginas(r#"{"formato":1,"paginas":[{"id":"a","titulo":"Día 1"}]}"#.into()).unwrap();
        assert_eq!(p, [PaginaDeCarpeta { id: "a".into(), titulo: "Día 1".into() }]);
        assert!(carpeta_paginas("no es json".into()).is_none());
        assert_eq!(carpeta_siguiente_titulo("Día 09".into()).as_deref(), Some("Día 10"));
    }
}
