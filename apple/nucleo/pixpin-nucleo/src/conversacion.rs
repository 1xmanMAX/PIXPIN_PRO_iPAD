//! **Un chat compartido como conversacion** (Windows b89c058 y e3f1d60,
//! 25-oct-2026), del lado del Mac: lo mio a la derecha, lo de cada otra
//! persona a la izquierda con su nombre encima de su tanda, y la chapa de
//! compartido en la lista. Es `compartidos_ui::en_el_chat` y
//! `ventana_chat::lados_del_chat_compartido` de Windows sin la ventana, sobre
//! `pixpin_ui::conversacion` y `pixpin_ui::historial::colocar`, que son los
//! mismos crates: quien es quien sale de los mismos campos (el `aparato` de
//! cada mensaje, los aparatos de mi grupo y la lista de `compartidos.json`).
//!
//! Un chat normal no cambia: [`Nucleo::conversacion_del_chat`] devuelve
//! `None` y se ve como siempre, un cuaderno propio.
//!
//! Sin ningun chat compartido no se lee la identidad (leerla la crea), como
//! Windows cad5e20.

use std::collections::HashSet;

use pixpin_geom::Rect;
use pixpin_proyecto::cuaderno::Mensaje;
use pixpin_proyecto::vista::DiscoPc;
use pixpin_sincro::compartidos::{self, ChatCompartido, CompartidosEnDisco, Persona};
use pixpin_ui::conversacion::{self as cv, Lados, Quien};
use pixpin_ui::historial;

use crate::{ErrorNucleo, Nucleo};

/// Los colores de las personas (`COLORES_AVATAR` de Windows, los de
/// Telegram): `conversacion::tono` elige uno por el nombre, el mismo en
/// todos los aparatos.
pub const COLORES_DE_PERSONA: [u32; 7] = [
    0xe17076, 0xfaa774, 0xa695e7, 0x7bc862, 0x6ec9cb, 0x65aadd, 0xee7aae,
];

/// Un chat compartido en el que estoy, para la chapa de la lista.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChapaDeCompartido {
    /// La ficha de este aparato (`FichaChat.id`).
    pub ficha: String,
    /// Las personas que estan dentro, yo incluido, cada una una vez.
    pub personas: u32,
}

/// De un mensaje, lo que hace falta para saber de quien es.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AutoriaDeMensaje {
    pub id: String,
    pub cuando: i64,
    /// `MensajeChat.aparato`: el codigo corto del aparato donde nacio.
    pub aparato: Option<String>,
}

/// A que lado va un mensaje de un chat compartido y que lleva encima.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LadoDeMensaje {
    pub id: String,
    /// Mio (de cualquiera de mis aparatos): a la derecha, con mi color.
    pub mio: bool,
    /// El nombre de quien lo escribio, solo en la primera burbuja de cada
    /// tanda de otra persona (como un grupo de Telegram).
    pub nombre: Option<String>,
    /// La inicial de su circulo y el color de los dos (`0xRRGGBB`).
    pub inicial: String,
    pub color: u32,
    /// Empieza tanda: va algo mas separado del de arriba.
    pub abre_tanda: bool,
}

/// Un chat compartido visto desde el chat: cuantas personas y el lado de
/// cada mensaje, en el orden en que se pidieron.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ConversacionCompartida {
    pub personas: u32,
    pub lados: Vec<LadoDeMensaje>,
}

/// Lo que deja el «+» de «Chat compartido».
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChatNuevo {
    pub ficha: crate::chats::FichaChat,
    /// Nacio ya compartido (si no, quedo como chat normal).
    pub compartido: bool,
}

/// El codigo corto (`K7Q2`) del aparato donde nacio un mensaje
/// (`aparato_corto` de Windows). Unos se guardaron con el id entero del
/// aparato: de ese sale el corto por el mismo camino que en el movil.
pub(crate) fn aparato_corto(m: &Mensaje) -> Option<String> {
    let a = m.aparato.as_deref()?.trim();
    if a.chars().count() > 6 {
        Some(pixpin_proyecto::codigos::de_aparato(a))
    } else {
        Some(a.to_string())
    }
}

/// **Quien es quien** en un chat compartido (`en_el_chat` de Windows): `mios`
/// son los ids de todos mis aparatos. Una persona de la lista cuyo aparato
/// es mio (mi movil, que entro solo al sincronizar el grupo) es mia: si no,
/// lo que escribo desde el movil saldria a la izquierda con mi propio
/// nombre. Devuelve tambien cuantas personas hay dentro.
pub(crate) fn en_el_chat(c: &ChatCompartido, mios: &HashSet<String>) -> (u32, Lados) {
    let codigos_mios = mios
        .iter()
        .map(|id| pixpin_sincro::grupo::codigo_de_aparato(id))
        .chain(
            c.personas
                .iter()
                .filter(|p| mios.contains(&p.id))
                .map(|p| p.codigo.clone()),
        );
    // Por orden de entrada, como `compartidos::autores`: si un codigo
    // saliera dos veces, gana el nombre mas reciente.
    let mut personas: Vec<&Persona> = c.personas.iter().collect();
    personas.sort_by_key(|p| p.desde);
    let lados = Lados::nuevo(
        codigos_mios,
        personas
            .into_iter()
            .map(|p| (p.codigo.clone(), p.nombre.clone())),
    );
    (compartidos::nombres_dentro(c).len() as u32, lados)
}

/// **El lado de cada mensaje** (`lados_del_chat_compartido` de Windows): de
/// quien es, su numero de autor y, con `historial::colocar`, en cual va el
/// nombre encima. `desfase_ms` es local menos UTC (el dia cambia de tanda).
pub(crate) fn lados_de(
    lados: &Lados,
    mensajes: &[AutoriaDeMensaje],
    desfase_ms: i64,
) -> Vec<LadoDeMensaje> {
    let aparato = |m: &AutoriaDeMensaje| m.aparato.as_deref().map(str::trim).map(str::to_string);
    let quienes: Vec<Quien> = mensajes
        .iter()
        .map(|m| lados.quien(aparato(m).as_deref()))
        .collect();
    let autores = cv::numerar(&quienes);
    let del_asistente =
        |m: &AutoriaDeMensaje| aparato(m).as_deref() == Some(crate::asistente::chat::APARATO);
    // Solo importa quien abre tanda y quien lleva nombre: las medidas dan
    // igual, se colocan todas de uno por uno.
    let entradas: Vec<historial::Entrada> = mensajes
        .iter()
        .enumerate()
        .map(|(k, m)| {
            let asistente = del_asistente(m);
            historial::Entrada {
                alto: 1,
                ancho: 1,
                // El asistente no es una persona de la lista: a la
                // izquierda, como en su chat, y sin nombre encima.
                mio: !asistente && quienes[k].es_mio(),
                cuando: m.cuando,
                dia: (m.cuando + desfase_ms).div_euclid(86_400_000),
                autor: if asistente { u32::MAX } else { autores[k] },
                alto_nombre: u32::from(!asistente && quienes[k].nombre().is_some()),
            }
        })
        .collect();
    let area = Rect {
        x: 0,
        y: 0,
        ancho: 1000,
        alto: 1000,
    };
    let (puestos, _) = historial::colocar(area, &entradas, 100);
    let mut anterior: Option<&historial::Puesto> = None;
    mensajes
        .iter()
        .zip(&entradas)
        .zip(&puestos)
        .enumerate()
        .map(|(k, ((m, e), p))| {
            // Abre tanda lo que va tras una pildora, lleva nombre o queda
            // separado del de arriba por el hueco grande.
            let hueco = anterior.map(|a| p.arriba() - (a.burbuja.y + a.burbuja.alto as i32));
            let abre_tanda = p.separador.is_some()
                || p.nombre.is_some()
                || hueco.is_none_or(|h| h >= historial::HUECO_GRUPO as i32);
            anterior = Some(p);
            let nombre = quienes[k].nombre().unwrap_or_default();
            LadoDeMensaje {
                id: m.id.clone(),
                mio: e.mio,
                nombre: p.nombre.map(|_| nombre.to_string()),
                inicial: cv::inicial(nombre),
                color: COLORES_DE_PERSONA[cv::tono(nombre, COLORES_DE_PERSONA.len())],
                abre_tanda,
            }
        })
        .collect()
}

impl Nucleo {
    /// Los chats compartidos donde estoy dentro y quien soy yo (este aparato
    /// y todos los de mi grupo). Sin ninguno no se lee la identidad.
    fn compartidos_donde_estoy(
        &self,
    ) -> Result<(Vec<ChatCompartido>, HashSet<String>), ErrorNucleo> {
        let lista = CompartidosEnDisco::nuevo(self.ruta()).leer();
        if lista.is_empty() {
            return Ok((Vec::new(), HashSet::new()));
        }
        let i = self.identidad()?;
        let mut mios: HashSet<String> = i.miembros.iter().map(|m| m.id.clone()).collect();
        mios.insert(i.yo.id.clone());
        let dentro = lista
            .into_iter()
            .filter(|c| compartidos::dentro(c, &i.yo.id))
            .collect();
        Ok((dentro, mios))
    }
}

#[uniffi::export]
impl Nucleo {
    /// **La chapa de la lista**: los chats compartidos donde estoy, por su
    /// ficha, con cuantas personas son (`personas_del_chat` de Windows). Los
    /// que aun no tienen proyecto aqui no salen.
    pub fn chapas_de_compartidos(&self) -> Result<Vec<ChapaDeCompartido>, ErrorNucleo> {
        let (lista, mios) = self.compartidos_donde_estoy()?;
        if lista.is_empty() {
            return Ok(Vec::new());
        }
        let disco = DiscoPc::nuevo(self.ruta());
        Ok(lista
            .iter()
            .filter_map(|c| {
                let f = disco.ficha_de(&c.chat)?;
                Some(ChapaDeCompartido {
                    ficha: f.id,
                    personas: en_el_chat(c, &mios).0,
                })
            })
            .collect())
    }

    /// **El chat de la ficha `ficha` como conversacion**, si es compartido y
    /// estoy dentro: el lado de cada uno de `mensajes` (los que se ven, del
    /// mas viejo al mas nuevo). `None` es un chat normal.
    pub fn conversacion_del_chat(
        &self,
        ficha: String,
        mensajes: Vec<AutoriaDeMensaje>,
        desfase_ms: i64,
    ) -> Result<Option<ConversacionCompartida>, ErrorNucleo> {
        let (lista, mios) = self.compartidos_donde_estoy()?;
        if lista.is_empty() {
            return Ok(None);
        }
        let chat = pixpin_proyecto::vista::chat_de_ficha(self.ruta(), &ficha).unwrap_or(ficha);
        let Some(c) = lista.iter().find(|c| c.chat == chat) else {
            return Ok(None);
        };
        let (personas, lados) = en_el_chat(c, &mios);
        Ok(Some(ConversacionCompartida {
            personas,
            lados: lados_de(&lados, &mensajes, desfase_ms),
        }))
    }

    /// **Nace un chat compartido** (el «+» de la lista, «Chat compartido»,
    /// `compartir_chat_nuevo` de Windows): un proyecto nuevo y su chat en
    /// `compartidos.json` con una sola persona, yo, que lo creo. A los demas
    /// se les invita despues, como siempre. Si no se puede compartir, el
    /// proyecto queda como chat normal (`compartido` a `false`: «No se pudo
    /// compartir el chat nuevo: queda como chat normal»).
    pub fn crear_chat_compartido(&self, nombre: String) -> Result<ChatNuevo, ErrorNucleo> {
        let ficha = self.crear_chat(nombre)?;
        let compartido = match self.compartir_chat(ficha.id.clone()) {
            Ok(_) => true,
            Err(e) => {
                eprintln!("PixPin: no se pudo compartir el chat nuevo: {e:?}");
                false
            }
        };
        Ok(ChatNuevo { ficha, compartido })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn yo(id: &str, nombre: &str) -> compartidos::Yo {
        compartidos::Yo {
            id: id.into(),
            nombre: nombre.into(),
            codigo: pixpin_sincro::grupo::codigo_de_aparato(id),
            llaves: compartidos::Llaves::nuevas(),
        }
    }

    fn autoria(id: &str, cuando: i64, aparato: Option<&str>) -> AutoriaDeMensaje {
        AutoriaDeMensaje {
            id: id.into(),
            cuando,
            aparato: aparato.map(str::to_string),
        }
    }

    /// Max con su PC y su movil, y Ana: el chat de la prueba de Windows.
    fn chat_de_max_y_ana() -> (
        ChatCompartido,
        compartidos::Yo,
        compartidos::Yo,
        compartidos::Yo,
    ) {
        let pc = yo("pc-de-max", "Max");
        let movil = yo("movil-de-max", "Max");
        let ana = yo("movil-de-ana", "Ana");
        let c = compartidos::crear("tesis", "Tesis", &pc, 1);
        let c = compartidos::invitar(&c, &pc, &movil.ficha(0), 2).expect("mi movil entra");
        let c = compartidos::invitar(&c, &pc, &ana.ficha(0), 3).expect("Ana entra");
        (c, pc, movil, ana)
    }

    #[test]
    fn lo_de_mi_movil_es_mio_y_lo_de_ana_de_ana() {
        let (c, pc, movil, ana) = chat_de_max_y_ana();
        let mios: HashSet<String> = ["pc-de-max".to_string(), "movil-de-max".to_string()].into();
        let (personas, l) = en_el_chat(&c, &mios);
        assert_eq!(l.quien(Some(&pc.codigo)), Quien::Yo);
        assert_eq!(l.quien(Some(&movil.codigo)), Quien::Yo);
        assert_eq!(l.quien(Some(&ana.codigo)), Quien::Otro("Ana".into()));
        // Lo de antes de compartir (sin codigo) sigue siendo mio.
        assert_eq!(l.quien(None), Quien::Yo);
        assert_eq!(personas, 2, "Max (dos aparatos) y Ana");
    }

    #[test]
    fn el_nombre_va_solo_en_la_primera_de_cada_tanda_de_otra_persona() {
        let (c, pc, _, ana) = chat_de_max_y_ana();
        let mios: HashSet<String> = ["pc-de-max".to_string()].into();
        let (_, l) = en_el_chat(&c, &mios);
        let min = 60_000;
        let v = lados_de(
            &l,
            &[
                autoria("1", min, Some(&pc.codigo)),
                autoria("2", 2 * min, Some(&ana.codigo)),
                autoria("3", 3 * min, Some(&ana.codigo)),
                autoria("4", 4 * min, Some(&pc.codigo)),
                // Pasado el rato, Ana abre tanda otra vez y lleva nombre.
                autoria("5", 20 * min, Some(&ana.codigo)),
                autoria("6", 21 * min, Some("IA")),
            ],
            0,
        );
        let mios: Vec<bool> = v.iter().map(|x| x.mio).collect();
        assert_eq!(mios, [true, false, false, true, false, false]);
        let nombres: Vec<Option<&str>> = v.iter().map(|x| x.nombre.as_deref()).collect();
        assert_eq!(nombres, [None, Some("Ana"), None, None, Some("Ana"), None]);
        let tandas: Vec<bool> = v.iter().map(|x| x.abre_tanda).collect();
        assert_eq!(tandas, [true, true, false, true, true, true]);
        assert_eq!(v[1].inicial, "A");
        assert_eq!(
            v[1].color,
            COLORES_DE_PERSONA[cv::tono("Ana", COLORES_DE_PERSONA.len())]
        );
    }

    #[test]
    fn dos_personas_seguidas_no_se_agrupan_aunque_vayan_al_mismo_lado() {
        let pc = yo("pc-de-max", "Max");
        let ana = yo("movil-de-ana", "Ana");
        let luis = yo("pc-de-luis", "Luis");
        let c = compartidos::crear("tesis", "Tesis", &pc, 1);
        let c = compartidos::invitar(&c, &pc, &ana.ficha(0), 2).unwrap();
        let c = compartidos::invitar(&c, &pc, &luis.ficha(0), 3).unwrap();
        let (personas, l) = en_el_chat(&c, &["pc-de-max".to_string()].into());
        assert_eq!(personas, 3);
        let v = lados_de(
            &l,
            &[
                autoria("1", 1000, Some(&ana.codigo)),
                autoria("2", 2000, Some(&luis.codigo)),
            ],
            0,
        );
        assert_eq!(v[0].nombre.as_deref(), Some("Ana"));
        assert_eq!(v[1].nombre.as_deref(), Some("Luis"));
    }

    #[test]
    fn el_aparato_largo_se_acorta_como_en_el_movil() {
        let m = Mensaje {
            aparato: Some("un-id-de-aparato-largo".into()),
            ..Default::default()
        };
        assert_eq!(
            aparato_corto(&m),
            Some(pixpin_proyecto::codigos::de_aparato(
                "un-id-de-aparato-largo"
            ))
        );
        let corto = Mensaje {
            aparato: Some(" K7Q2 ".into()),
            ..Default::default()
        };
        assert_eq!(aparato_corto(&corto).as_deref(), Some("K7Q2"));
        assert_eq!(aparato_corto(&Mensaje::default()), None);
    }
}
