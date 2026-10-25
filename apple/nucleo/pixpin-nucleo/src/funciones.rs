//! **La pestana Funciones** de la ventana de los chats (Windows 2e0da60,
//! b313007, 768b73f y 9620ac8, 25-oct-2026), del lado del Mac: la segunda
//! parte del interruptor Chat · Funciones (antes «Figuras»), que cambia la
//! columna de la lista por una rejilla de mosaicos grandes. Es
//! `ventana_chat/funciones.rs` de Windows sin pintar nada, sobre
//! `pixpin_ui::funciones`, que es el mismo crate: el orden, las claves, donde
//! cae cada mosaico y las flechas son los de Windows. Los textos salen del
//! mismo catalogo (`funciones-*` de `pixpin-store`), y el icono y el color
//! de cada mosaico son los de la tabla de `docs/ui-funciones-y-chats.md`.
//!
//! Swift pinta, apunta el raton y hace lo pulsado (`FuncionesDeLaLista`).

use pixpin_geom::{Punto, Rect};
use pixpin_store::ajustes::PreferenciaIdioma;
use pixpin_store::idioma::{Catalogo, resolver_idioma};
use pixpin_ui::funciones::{self as ui, Funcion, Paso};

/// Cada funcion de la rejilla (`pixpin_ui::funciones::Funcion`), en su orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum FuncionDeLaLista {
    Grabar,
    Lienzo,
    Tareas,
    Timeline,
    /// Abre Tareas en su vista Horario (Windows 9620ac8).
    Horario,
    Galeria,
    Figuras,
}

impl From<Funcion> for FuncionDeLaLista {
    fn from(f: Funcion) -> Self {
        match f {
            Funcion::Grabar => FuncionDeLaLista::Grabar,
            Funcion::Lienzo => FuncionDeLaLista::Lienzo,
            Funcion::Tareas => FuncionDeLaLista::Tareas,
            Funcion::Timeline => FuncionDeLaLista::Timeline,
            Funcion::Horario => FuncionDeLaLista::Horario,
            Funcion::Galeria => FuncionDeLaLista::Galeria,
            Funcion::Figuras => FuncionDeLaLista::Figuras,
        }
    }
}

/// El icono (Material, el nombre de `TrazadosMaterial`) y el color del
/// circulo de cada funcion: el mismo de dia y de noche, con el icono en
/// blanco encima (`icono` y `acento` de Windows).
fn icono_y_color(f: Funcion) -> (&'static str, u32) {
    match f {
        Funcion::Grabar => ("mic", 0xE5484D),
        Funcion::Lienzo => ("draw", 0x8E4EC6),
        Funcion::Tareas => ("checklist", 0x1F7A3A),
        Funcion::Timeline => ("timeline", 0x0090FF),
        Funcion::Horario => ("calendar_month", 0xF76B15),
        Funcion::Galeria => ("photo_library", 0x12A594),
        Funcion::Figuras => ("layers", 0x3E63DD),
    }
}

/// Un mosaico, con lo que lleva escrito.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MosaicoDeFunciones {
    pub funcion: FuncionDeLaLista,
    /// El nombre estable (`grabar`, `lienzo`…), el mismo en todos los aparatos.
    pub nombre: String,
    pub titulo: String,
    /// La linea de debajo del titulo.
    pub detalle: String,
    /// El icono Material del circulo.
    pub icono: String,
    /// El color del circulo, `0xRRGGBB`.
    pub color: u32,
}

/// Los textos de la pestana y del interruptor, en el idioma de la app.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TextosDeFunciones {
    /// Las dos partes del interruptor de la barra.
    pub interruptor_chat: String,
    pub interruptor_funciones: String,
    /// «Funciones», encima de la rejilla.
    pub titulo: String,
    /// Un mosaico por funcion, en el orden en que salen.
    pub mosaicos: Vec<MosaicoDeFunciones>,
}

/// **Los textos de Funciones** para `locale` (`es-ES`, `en_US`…; cualquiera
/// que no sea espanol, en ingles, como `resolver_idioma`).
#[uniffi::export]
pub fn textos_de_funciones(locale: String) -> TextosDeFunciones {
    let textos = Catalogo::nuevo(resolver_idioma(&locale, PreferenciaIdioma::Sistema));
    TextosDeFunciones {
        interruptor_chat: textos.t("proyectos-interruptor-chat"),
        interruptor_funciones: textos.t("proyectos-interruptor-funciones"),
        titulo: textos.t("funciones-titulo"),
        mosaicos: Funcion::TODAS
            .iter()
            .map(|f| {
                let (icono, color) = icono_y_color(*f);
                MosaicoDeFunciones {
                    funcion: (*f).into(),
                    nombre: f.nombre().to_string(),
                    titulo: textos.t(f.clave_titulo()),
                    detalle: textos.t(f.clave_detalle()),
                    icono: icono.to_string(),
                    color,
                }
            })
            .collect(),
    }
}

/// Un rectangulo en puntos de la columna, con el cero en su esquina.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct CajaDeMosaico {
    pub x: i32,
    pub y: i32,
    pub ancho: u32,
    pub alto: u32,
}

impl From<Rect> for CajaDeMosaico {
    fn from(r: Rect) -> Self {
        CajaDeMosaico {
            x: r.x,
            y: r.y,
            ancho: r.ancho,
            alto: r.alto,
        }
    }
}

/// Donde va cada cosa de la pestana (`pixpin_ui::funciones::Rejilla`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RejillaDeFunciones {
    /// La franja del titulo (vacia si solo caben iconos).
    pub titulo: CajaDeMosaico,
    /// Uno por funcion, en el orden de [`textos_de_funciones`].
    pub mosaicos: Vec<CajaDeMosaico>,
    pub columnas: u32,
    /// Tan estrecha que solo caben los iconos, en una columna de cuadrados.
    pub solo_iconos: bool,
    /// Los mosaicos llevan su linea de descripcion; si no, el icono va a la
    /// izquierda del titulo.
    pub con_detalle: bool,
}

/// **La rejilla en una columna de `ancho` x `alto` puntos** (la de la
/// lista): las mismas cuentas que Windows a escala 100, con el radio, los
/// lados y las letras de `pixpin_ui::funciones` (ver [`medidas_de_funciones`]).
#[uniffi::export]
pub fn rejilla_de_funciones(ancho: u32, alto: u32) -> RejillaDeFunciones {
    let zona = Rect {
        x: 0,
        y: 0,
        ancho,
        alto,
    };
    let g = ui::rejilla(zona, Funcion::TODAS.len(), 100);
    RejillaDeFunciones {
        titulo: g.titulo.into(),
        mosaicos: g.mosaicos.iter().map(|r| (*r).into()).collect(),
        columnas: g.columnas as u32,
        solo_iconos: g.solo_iconos,
        con_detalle: g.con_detalle,
    }
}

/// El mosaico bajo el punto `x, y` de la columna, si cae en alguno.
#[uniffi::export]
pub fn funcion_en_la_rejilla(r: RejillaDeFunciones, x: i32, y: i32) -> Option<u32> {
    let p = Punto { x, y };
    r.mosaicos
        .iter()
        .position(|c| {
            Rect {
                x: c.x,
                y: c.y,
                ancho: c.ancho,
                alto: c.alto,
            }
            .contiene(p)
        })
        .map(|i| i as u32)
}

/// Las medidas con que se pinta un mosaico (`pixpin_ui::funciones`).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct MedidasDeFunciones {
    pub radio: f32,
    pub icono_lado: f32,
    pub relleno: f32,
    pub tam_titulo: f32,
    pub tam_detalle: f32,
    /// La letra del titulo «Funciones».
    pub tam_titulo_rejilla: f32,
}

#[uniffi::export]
pub fn medidas_de_funciones() -> MedidasDeFunciones {
    MedidasDeFunciones {
        radio: ui::RADIO,
        icono_lado: ui::ICONO_LADO as f32,
        relleno: ui::RELLENO as f32,
        tam_titulo: ui::TAM_TITULO,
        tam_detalle: ui::TAM_DETALLE,
        tam_titulo_rejilla: ui::TITULO_TAM,
    }
}

/// Una tecla con el teclado en la rejilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TeclaDeFunciones {
    Izquierda,
    Derecha,
    Arriba,
    Abajo,
    Inicio,
    Fin,
    Tab,
    /// Intro o Espacio.
    Hacer,
    Escape,
}

/// Lo que contesta la rejilla a una tecla (`Respuesta` de Windows).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RespuestaDeFunciones {
    /// El mosaico elegido despues de la tecla.
    pub elegida: Option<u32>,
    /// La funcion que hay que hacer (Intro o Espacio con uno elegido).
    pub hacer: Option<FuncionDeLaLista>,
    /// Escape: de vuelta al chat.
    pub volver: bool,
}

/// **Una tecla en la rejilla** desde `elegida`, con `columnas` (las de la
/// rejilla que se ve): flechas, Inicio y Fin recorren sin dar la vuelta
/// (`pixpin_ui::funciones::mover`), Tab si da la vuelta, Intro o Espacio
/// hacen el elegido (sin ninguno, eligen el primero) y Escape vuelve al chat.
#[uniffi::export]
pub fn tecla_en_funciones(
    elegida: Option<u32>,
    tecla: TeclaDeFunciones,
    columnas: u32,
) -> RespuestaDeFunciones {
    let cuantos = Funcion::TODAS.len();
    let desde = elegida.map(|i| i as usize);
    let mover = |paso| ui::mover(desde, paso, columnas as usize, cuantos).map(|i| i as u32);
    let sigue = |elegida| RespuestaDeFunciones {
        elegida,
        hacer: None,
        volver: false,
    };
    match tecla {
        TeclaDeFunciones::Izquierda => sigue(mover(Paso::Izquierda)),
        TeclaDeFunciones::Derecha => sigue(mover(Paso::Derecha)),
        TeclaDeFunciones::Arriba => sigue(mover(Paso::Arriba)),
        TeclaDeFunciones::Abajo => sigue(mover(Paso::Abajo)),
        TeclaDeFunciones::Inicio => sigue(mover(Paso::Inicio)),
        TeclaDeFunciones::Fin => sigue(mover(Paso::Fin)),
        // Tab da la vuelta, como en cualquier grupo de botones.
        TeclaDeFunciones::Tab => sigue(Some(desde.map_or(0, |i| (i + 1) % cuantos) as u32)),
        TeclaDeFunciones::Hacer => match desde.and_then(|i| Funcion::TODAS.get(i)) {
            Some(f) => RespuestaDeFunciones {
                elegida,
                hacer: Some((*f).into()),
                volver: false,
            },
            None => sigue(Some(0)),
        },
        TeclaDeFunciones::Escape => RespuestaDeFunciones {
            elegida,
            hacer: None,
            volver: true,
        },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_textos_van_en_el_orden_de_windows_en_los_dos_idiomas() {
        let es = textos_de_funciones("es-ES".into());
        assert_eq!(es.interruptor_chat, "Chat");
        assert_eq!(es.interruptor_funciones, "Funciones");
        assert_eq!(es.titulo, "Funciones");
        let nombres: Vec<_> = es.mosaicos.iter().map(|m| m.nombre.as_str()).collect();
        assert_eq!(
            nombres,
            [
                "grabar", "lienzo", "tareas", "timeline", "horario", "galeria", "figuras"
            ]
        );
        assert_eq!(es.mosaicos[0].titulo, "Grabar reunión");
        assert_eq!(
            es.mosaicos[0].detalle,
            "Clase o reunión, con capturas y transcripción"
        );
        assert_eq!(es.mosaicos[5].titulo, "Capturas");
        assert_eq!(es.mosaicos[4].funcion, FuncionDeLaLista::Horario);
        assert_eq!(es.mosaicos[4].icono, "calendar_month");
        assert_eq!(es.mosaicos[4].color, 0xF76B15);
        let en = textos_de_funciones("en_US".into());
        assert_eq!(en.interruptor_funciones, "Functions");
        assert_eq!(en.mosaicos[4].titulo, "Timetable");
        assert_eq!(en.mosaicos[6].titulo, "Shapes");
        // Caso negativo: ningun texto se queda en su clave.
        for m in es.mosaicos.iter().chain(&en.mosaicos) {
            assert!(!m.titulo.starts_with("funciones-"), "{m:?}");
            assert!(!m.detalle.starts_with("funciones-"), "{m:?}");
        }
    }

    #[test]
    fn la_rejilla_de_la_lista_de_siempre_es_la_de_windows() {
        let g = rejilla_de_funciones(320, 700);
        assert_eq!(g.columnas, 2);
        assert!(g.con_detalle && !g.solo_iconos);
        assert_eq!(g.mosaicos.len(), 7);
        assert_eq!(g.titulo.alto, ui::TITULO_ALTO);
        assert_eq!(g.mosaicos[0].x, ui::MARGEN as i32);
        assert_eq!(g.mosaicos[0].alto, ui::MOSAICO_ALTO);
        let c = g.mosaicos[2];
        assert_eq!(
            funcion_en_la_rejilla(g.clone(), c.x + 5, c.y + 5),
            Some(2),
            "el tercero"
        );
        // Caso negativo: en el titulo no hay mosaico.
        assert_eq!(funcion_en_la_rejilla(g.clone(), 20, 10), None);
        // Estrecha: solo iconos.
        assert!(rejilla_de_funciones(72, 700).solo_iconos);
    }

    #[test]
    fn el_teclado_hace_lo_mismo_que_en_windows() {
        use TeclaDeFunciones as T;
        // Sin elegido, la primera flecha elige el primero y Intro lo hace.
        let r = tecla_en_funciones(None, T::Abajo, 2);
        assert_eq!(r.elegida, Some(0));
        let r = tecla_en_funciones(r.elegida, T::Hacer, 2);
        assert_eq!(r.hacer, Some(FuncionDeLaLista::Grabar));
        let r = tecla_en_funciones(Some(0), T::Derecha, 2);
        assert_eq!(r.elegida, Some(1));
        assert_eq!(
            tecla_en_funciones(Some(1), T::Abajo, 2).elegida,
            Some(3),
            "Timeline"
        );
        // Tab da la vuelta; las flechas no.
        assert_eq!(tecla_en_funciones(Some(6), T::Tab, 2).elegida, Some(0));
        assert_eq!(tecla_en_funciones(Some(6), T::Derecha, 2).elegida, Some(6));
        // Intro sin elegido elige el primero sin hacer nada.
        let r = tecla_en_funciones(None, T::Hacer, 2);
        assert_eq!((r.elegida, r.hacer), (Some(0), None));
        assert!(tecla_en_funciones(Some(3), T::Escape, 2).volver);
        assert!(!tecla_en_funciones(Some(3), T::Fin, 2).volver);
    }
}
