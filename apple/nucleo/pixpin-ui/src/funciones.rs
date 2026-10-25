//! **Funciones**: la segunda parte del interruptor de la barra del chat
//! (25-oct-2026), las cuentas sin pintar nada.
//!
//! Donde estaba «Figuras». Elegida, la columna de la lista de chats ensena
//! una rejilla de mosaicos grandes, uno por funcion fuerte de la aplicacion
//! (grabar una reunion, un lienzo nuevo, las tareas…), con su icono, su
//! titulo y una linea que dice que hace. Lo que vive aqui ya no esta en la
//! cabecera de la lista de chats: alli quedan solo los chats (el usuario:
//! «que solo aparezcan unas pocas funciones fuertes, y quitarlas de los
//! chats»). Lo mismo en el movil y en el Mac: ver
//! `docs/ui-funciones-y-chats.md`.

use pixpin_geom::{Punto, Rect};

/// Cada funcion de la rejilla, **en el orden en que sale**: de la que mas se
/// usa a la que menos. La posicion manda en la rejilla y en el teclado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Funcion {
    /// Grabar una clase o una reunion: microfono, sonido del equipo y
    /// capturas de una zona, y luego la transcripcion.
    Grabar,
    /// Un lienzo en blanco, en el chat abierto o en «Guardados».
    Lienzo,
    /// Las listas de tareas de todos los chats juntas.
    Tareas,
    /// Lo que pasa en el dia, minuto a minuto, con las lecciones dentro.
    Timeline,
    /// El horario de clases que llama antes de cada una.
    Horario,
    /// Todas las capturas guardadas.
    Galeria,
    /// La biblioteca de figuras guardadas (antes, la parte «Figuras» del
    /// interruptor).
    Figuras,
}

impl Funcion {
    /// Todas, en su orden.
    pub const TODAS: [Funcion; 7] = [
        Funcion::Grabar,
        Funcion::Lienzo,
        Funcion::Tareas,
        Funcion::Timeline,
        Funcion::Horario,
        Funcion::Galeria,
        Funcion::Figuras,
    ];

    /// El nombre estable, el mismo en todos los aparatos.
    pub fn nombre(self) -> &'static str {
        match self {
            Funcion::Grabar => "grabar",
            Funcion::Lienzo => "lienzo",
            Funcion::Tareas => "tareas",
            Funcion::Timeline => "timeline",
            Funcion::Horario => "horario",
            Funcion::Galeria => "galeria",
            Funcion::Figuras => "figuras",
        }
    }

    /// La clave del catalogo con su titulo.
    pub fn clave_titulo(self) -> &'static str {
        match self {
            Funcion::Grabar => "funciones-grabar",
            Funcion::Lienzo => "funciones-lienzo",
            Funcion::Tareas => "funciones-tareas",
            Funcion::Timeline => "funciones-timeline",
            Funcion::Horario => "funciones-horario",
            Funcion::Galeria => "funciones-galeria",
            Funcion::Figuras => "funciones-figuras",
        }
    }

    /// La clave de su linea de descripcion.
    pub fn clave_detalle(self) -> &'static str {
        match self {
            Funcion::Grabar => "funciones-grabar-detalle",
            Funcion::Lienzo => "funciones-lienzo-detalle",
            Funcion::Tareas => "funciones-tareas-detalle",
            Funcion::Timeline => "funciones-timeline-detalle",
            Funcion::Horario => "funciones-horario-detalle",
            Funcion::Galeria => "funciones-galeria-detalle",
            Funcion::Figuras => "funciones-figuras-detalle",
        }
    }
}

/// El aire alrededor de la rejilla.
pub const MARGEN: u32 = 14;
/// El hueco entre dos mosaicos.
pub const HUECO: u32 = 10;
/// Lo alto del titulo «Funciones», encima de la rejilla.
pub const TITULO_ALTO: u32 = 44;
/// La letra del titulo.
pub const TITULO_TAM: f32 = 17.0;
/// Lo mas estrecho que puede ser un mosaico con su texto; por debajo, la
/// rejilla pasa a solo iconos.
pub const MOSAICO_MIN: u32 = 140;
/// Nunca mas columnas que estas: un mosaico grande se pulsa sin apuntar.
pub const COLUMNAS_MAX: usize = 3;
/// Lo alto de un mosaico con su descripcion.
pub const MOSAICO_ALTO: u32 = 104;
/// Lo menos alto que se encoge en una ventana baja; por debajo de
/// [`ALTO_CON_DETALLE`] se queda sin la linea de descripcion.
pub const MOSAICO_ALTO_MIN: u32 = 60;
pub const ALTO_CON_DETALLE: u32 = 88;
/// El radio de las esquinas de un mosaico.
pub const RADIO: f32 = 12.0;
/// El lado del circulo del icono y la letra del titulo y la descripcion.
pub const ICONO_LADO: u32 = 34;
pub const TAM_TITULO: f32 = 13.0;
pub const TAM_DETALLE: f32 = 11.0;
/// El relleno de dentro de un mosaico.
pub const RELLENO: u32 = 12;

/// Donde va cada cosa de la pantalla de Funciones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejilla {
    /// La franja del titulo (vacia si solo caben iconos).
    pub titulo: Rect,
    /// Un mosaico por funcion, en el orden de [`Funcion::TODAS`].
    pub mosaicos: Vec<Rect>,
    pub columnas: usize,
    /// Tan estrecha que solo caben los iconos (la lista plegada).
    pub solo_iconos: bool,
    /// Los mosaicos llevan su linea de descripcion.
    pub con_detalle: bool,
}

fn e(v: u32, escala: u32) -> u32 {
    v * escala / 100
}

/// Reparte `cuantos` mosaicos en `zona` (la columna de la lista).
///
/// Las columnas salen del ancho (de una a [`COLUMNAS_MAX`]), y el alto de
/// cada mosaico se encoge en una ventana baja hasta [`MOSAICO_ALTO_MIN`];
/// mas baja aun, la rejilla se sale por abajo y quien pinta la recorta.
pub fn rejilla(zona: Rect, cuantos: usize, escala: u32) -> Rejilla {
    let margen = e(MARGEN, escala);
    let hueco = e(HUECO, escala);
    let util = zona.ancho.saturating_sub(2 * margen);
    let minimo = e(MOSAICO_MIN, escala).max(1);
    let solo_iconos = util < minimo;
    // Solo iconos: una columna de cuadrados del ancho que haya, sin titulo.
    if solo_iconos {
        let lado = zona.ancho.saturating_sub(2 * e(6, escala)).max(1);
        let x = zona.x + (zona.ancho as i32 - lado as i32) / 2;
        let mosaicos = (0..cuantos)
            .map(|i| Rect {
                x,
                y: zona.y + (e(6, escala) + i as u32 * (lado + e(6, escala))) as i32,
                ancho: lado,
                alto: lado,
            })
            .collect();
        return Rejilla {
            titulo: Rect {
                ancho: 0,
                alto: 0,
                ..zona
            },
            mosaicos,
            columnas: 1,
            solo_iconos,
            con_detalle: false,
        };
    }
    let columnas = ((util + hueco) / (minimo + hueco)).clamp(1, COLUMNAS_MAX as u32) as usize;
    let filas = cuantos.div_ceil(columnas).max(1) as u32;
    let ancho = (util - hueco * (columnas as u32 - 1)) / columnas as u32;
    let titulo = Rect {
        x: zona.x + margen as i32,
        y: zona.y,
        ancho: util,
        alto: e(TITULO_ALTO, escala).min(zona.alto),
    };
    let arriba = titulo.abajo();
    let sitio = ((zona.abajo() - arriba).max(0) as u32).saturating_sub(margen);
    let por_fila = sitio.saturating_sub(hueco * (filas - 1)) / filas;
    let alto = por_fila.clamp(e(MOSAICO_ALTO_MIN, escala), e(MOSAICO_ALTO, escala));
    let mosaicos = (0..cuantos)
        .map(|i| {
            let (f, c) = ((i / columnas) as u32, (i % columnas) as u32);
            Rect {
                x: titulo.x + (c * (ancho + hueco)) as i32,
                y: arriba + (f * (alto + hueco)) as i32,
                ancho,
                alto,
            }
        })
        .collect();
    Rejilla {
        titulo,
        mosaicos,
        columnas,
        solo_iconos,
        con_detalle: alto >= e(ALTO_CON_DETALLE, escala),
    }
}

impl Rejilla {
    /// El mosaico bajo el punto.
    pub fn en(&self, p: Punto) -> Option<usize> {
        self.mosaicos.iter().position(|r| r.contiene(p))
    }
}

/// Una tecla de moverse por la rejilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paso {
    Izquierda,
    Derecha,
    Arriba,
    Abajo,
    Inicio,
    Fin,
}

/// A que mosaico lleva `paso` desde `desde` (sin ninguno elegido, al
/// primero). Las flechas no dan la vuelta: en el borde se queda donde esta,
/// como en el Explorador; a los lados si pasa a la fila de al lado, que es
/// lo que se espera de una rejilla que se lee como un texto.
pub fn mover(desde: Option<usize>, paso: Paso, columnas: usize, cuantos: usize) -> Option<usize> {
    if cuantos == 0 {
        return None;
    }
    let Some(i) = desde.filter(|i| *i < cuantos) else {
        return Some(match paso {
            Paso::Fin => cuantos - 1,
            _ => 0,
        });
    };
    let columnas = columnas.max(1);
    Some(match paso {
        Paso::Izquierda => i.saturating_sub(1),
        Paso::Derecha => (i + 1).min(cuantos - 1),
        Paso::Arriba => i.checked_sub(columnas).unwrap_or(i),
        Paso::Abajo if i + columnas < cuantos => i + columnas,
        Paso::Abajo => i,
        Paso::Inicio => 0,
        Paso::Fin => cuantos - 1,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn r(x: i32, y: i32, ancho: u32, alto: u32) -> Rect {
        Rect { x, y, ancho, alto }
    }

    fn centro(r: Rect) -> Punto {
        Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        }
    }

    #[test]
    fn las_funciones_van_en_su_orden_y_cada_una_con_sus_claves() {
        assert_eq!(Funcion::TODAS[0], Funcion::Grabar);
        assert_eq!(Funcion::TODAS[1], Funcion::Lienzo);
        let mut nombres: Vec<_> = Funcion::TODAS.iter().map(|f| f.nombre()).collect();
        nombres.sort();
        nombres.dedup();
        assert_eq!(nombres.len(), Funcion::TODAS.len(), "nombres distintos");
        for f in Funcion::TODAS {
            assert!(f.clave_titulo().starts_with("funciones-"));
            assert_eq!(f.clave_detalle(), format!("{}-detalle", f.clave_titulo()));
        }
    }

    #[test]
    fn en_la_lista_de_siempre_salen_dos_columnas_sin_tocarse_ni_salirse() {
        let zona = r(0, 40, 320, 700);
        let g = rejilla(zona, 7, 100);
        assert_eq!(g.columnas, 2);
        assert!(!g.solo_iconos);
        assert!(g.con_detalle);
        assert_eq!(g.mosaicos.len(), 7);
        for (i, a) in g.mosaicos.iter().enumerate() {
            assert!(a.x >= zona.x && a.derecha() <= zona.derecha());
            assert!(a.y >= g.titulo.abajo() && a.abajo() <= zona.abajo());
            assert_eq!(a.alto, MOSAICO_ALTO);
            for b in &g.mosaicos[i + 1..] {
                assert!(a.interseccion(*b).is_none(), "{a:?} pisa {b:?}");
            }
        }
        // Se leen como un texto: el segundo a la derecha del primero, el
        // tercero debajo del primero.
        assert_eq!(g.mosaicos[1].y, g.mosaicos[0].y);
        assert!(g.mosaicos[1].x > g.mosaicos[0].x);
        assert_eq!(g.mosaicos[2].x, g.mosaicos[0].x);
        assert!(g.mosaicos[2].y > g.mosaicos[0].y);
    }

    #[test]
    fn ancha_nunca_pasa_de_tres_columnas() {
        let g = rejilla(r(0, 0, 2000, 900), 7, 100);
        assert_eq!(g.columnas, COLUMNAS_MAX);
    }

    #[test]
    fn baja_encoge_los_mosaicos_y_les_quita_la_descripcion() {
        let g = rejilla(r(0, 0, 320, 330), 7, 100);
        let alto = g.mosaicos[0].alto;
        assert!((MOSAICO_ALTO_MIN..ALTO_CON_DETALLE).contains(&alto));
        assert!(!g.con_detalle);
        // Caso negativo: mas baja que el minimo no encoge mas.
        let g = rejilla(r(0, 0, 320, 120), 7, 100);
        assert_eq!(g.mosaicos[0].alto, MOSAICO_ALTO_MIN);
    }

    #[test]
    fn la_lista_plegada_deja_solo_los_iconos_en_una_columna() {
        let g = rejilla(r(0, 40, 72, 700), 7, 100);
        assert!(g.solo_iconos);
        assert_eq!(g.columnas, 1);
        assert_eq!(g.titulo.ancho, 0);
        for m in &g.mosaicos {
            assert_eq!(m.ancho, m.alto, "cuadrados");
            assert!(m.x >= 0 && m.derecha() <= 72);
        }
    }

    #[test]
    fn la_escala_lo_agranda_todo() {
        let g = rejilla(r(0, 0, 640, 1400), 7, 200);
        assert_eq!(g.columnas, 2);
        assert_eq!(g.mosaicos[0].alto, 2 * MOSAICO_ALTO);
    }

    #[test]
    fn el_clic_cae_en_su_mosaico_y_fuera_en_ninguno() {
        let g = rejilla(r(0, 40, 320, 700), 7, 100);
        for (i, m) in g.mosaicos.iter().enumerate() {
            assert_eq!(g.en(centro(*m)), Some(i));
        }
        assert_eq!(g.en(Punto { x: 1, y: 41 }), None, "en el margen");
        assert_eq!(g.en(centro(g.titulo)), None, "en el titulo");
    }

    #[test]
    fn las_flechas_se_mueven_por_la_rejilla_sin_salirse() {
        // Dos columnas, siete mosaicos: 0 1 / 2 3 / 4 5 / 6.
        assert_eq!(mover(None, Paso::Abajo, 2, 7), Some(0));
        assert_eq!(mover(None, Paso::Fin, 2, 7), Some(6));
        assert_eq!(mover(Some(0), Paso::Derecha, 2, 7), Some(1));
        assert_eq!(
            mover(Some(1), Paso::Derecha, 2, 7),
            Some(2),
            "a la fila de abajo"
        );
        assert_eq!(mover(Some(0), Paso::Izquierda, 2, 7), Some(0));
        assert_eq!(mover(Some(1), Paso::Abajo, 2, 7), Some(3));
        assert_eq!(mover(Some(5), Paso::Abajo, 2, 7), Some(5), "no hay debajo");
        assert_eq!(mover(Some(4), Paso::Abajo, 2, 7), Some(6));
        assert_eq!(mover(Some(1), Paso::Arriba, 2, 7), Some(1));
        assert_eq!(mover(Some(6), Paso::Arriba, 2, 7), Some(4));
        assert_eq!(mover(Some(6), Paso::Derecha, 2, 7), Some(6));
        assert_eq!(mover(Some(3), Paso::Inicio, 2, 7), Some(0));
        // Caso negativo: sin mosaicos no hay nada que elegir.
        assert_eq!(mover(Some(0), Paso::Abajo, 2, 0), None);
    }
}
