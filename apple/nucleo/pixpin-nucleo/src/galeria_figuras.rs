//! **La ventana de las figuras** (Windows `ventana_chat/figuras.rs`,
//! 24-oct-2026): la galeria de las figuras guardadas, agrupadas, dibujadas
//! de verdad y con su previsualizador (lo que mide, de que piezas esta
//! hecha, los textos que lleva).
//!
//! Aqui va lo que no es ventana: las fichas de cada figura (su grupo, sus
//! piezas, sus textos), el lienzo de una para pintarla (Swift la pinta como
//! la miniatura de un lienzo del chat), **«Insertar»** (un lienzo nuevo en el
//! chat con la figura dentro, `adjuntar_figura` de Windows) y **«Guardar en
//! mis figuras»** de un lienzo del chat (`guardar_en_figuras`).
//!
//! La biblioteca es la de siempre (`editor_figuras.rs`): `figuras.json` del
//! chat «Asistente», que viaja con la sincronizacion. Las figuras de serie de
//! Windows (`figuras_de_serie`, planificadores, diarios, simbolos) llegan por
//! ahi, con su categoria en el nombre («Diario · Gratitud») y su id
//! `serie-…`: no hace falta su catalogo para ensenarlas.

use std::sync::Arc;

use pixpin_motor2d::Figura;
use pixpin_motor2d::biblioteca::{self, FiguraGuardada};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::excalidraw;
use pixpin_motor2d::vector::Punto2;

use crate::chats::{MensajeChat, validar_chat};
use crate::lienzo::Lienzo;
use crate::{ErrorNucleo, Nucleo};

/// Entre la categoria y el nombre (`figuras_de_serie::SEPARADOR`).
pub const SEPARADOR: &str = " · ";
/// El principio del id de las figuras de serie (`figuras_de_serie::PREFIJO_ID`).
pub const PREFIJO_ID: &str = "serie-";

/// Las categorias de serie, en el orden del menu de Windows
/// (`figuras_de_serie::CATEGORIAS`). Una inventada por el usuario va detras.
pub const CATEGORIAS: [&str; 35] = [
    "Planificación",
    "Académico",
    "Creativo",
    "Diario",
    "Blocs",
    "Storyboard",
    "Arquitectura",
    "Vegetación",
    "Mobiliario",
    "Escala humana",
    "Planta: estructura",
    "Planta: puertas y ventanas",
    "Planta: cocina y baño",
    "Planta: muebles",
    "Planta: exterior",
    "Personas",
    "Personas en planta",
    "Ingeniería civil",
    "Derecho",
    "Electrónica",
    "Diagrama de flujo",
    "Marcas y sellos",
    "UML",
    "BPMN",
    "Entidad-relación",
    "Redes y TI",
    "Mapas mentales y organigramas",
    "Estrategia y negocio",
    "Formas y llamadas",
    "Física",
    "Química",
    "Matemáticas",
    "Biología",
    "Medicina",
    "Mecánica",
];

/// De que esta hecha una figura, para el previsualizador.
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
pub struct PiezasDeFigura {
    pub total: u32,
    pub trazos: u32,
    pub lineas: u32,
    pub formas: u32,
    pub textos: u32,
    pub otras: u32,
}

/// Una figura de la galeria.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FichaDeFigura {
    pub id: String,
    /// El nombre sin la categoria delante.
    pub nombre: String,
    /// La categoria; vacia en las del usuario («Mis figuras»).
    pub grupo: String,
    pub de_serie: bool,
    pub ancho: f32,
    pub alto: f32,
    pub piezas: PiezasDeFigura,
    /// Los textos que lleva, en orden de lectura (arriba abajo).
    pub letras: Vec<String>,
}

/// **La categoria de una figura** por su nombre («Diario · Gratitud» →
/// («Diario», «Gratitud»)), o nada si no lleva.
pub fn categoria_de(nombre: &str) -> Option<(&str, &str)> {
    let (c, n) = nombre.split_once(SEPARADOR)?;
    let (c, n) = (c.trim(), n.trim());
    (!c.is_empty() && !n.is_empty()).then_some((c, n))
}

pub(crate) fn piezas_de(f: &FiguraGuardada) -> PiezasDeFigura {
    let mut p = PiezasDeFigura::default();
    for e in f.elementos.iter().filter(|e| !e.borrado) {
        p.total += 1;
        match &e.figura {
            Figura::Lapiz { .. } | Figura::Resaltador { .. } => p.trazos += 1,
            Figura::Linea { .. } | Figura::Flecha { .. } | Figura::Arco { .. } | Figura::Cota { .. } => {
                p.lineas += 1
            }
            Figura::Rectangulo | Figura::Rombo | Figura::Elipse | Figura::Region { .. } => {
                p.formas += 1
            }
            Figura::Texto { .. } => p.textos += 1,
            _ => p.otras += 1,
        }
    }
    p
}

pub(crate) fn letras_de(f: &FiguraGuardada) -> Vec<String> {
    let mut v: Vec<(f32, f32, String)> = f
        .elementos
        .iter()
        .filter(|e| !e.borrado)
        .filter_map(|e| match &e.figura {
            Figura::Texto { texto, .. } if !texto.trim().is_empty() => {
                Some((e.y, e.x, texto.trim().replace('\n', " ")))
            }
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    v.into_iter().map(|(_, _, t)| t).collect()
}

fn ficha_de(f: &FiguraGuardada) -> FichaDeFigura {
    let (grupo, nombre) = match categoria_de(&f.nombre) {
        Some((c, n)) => (c.to_string(), n.to_string()),
        None => (String::new(), f.nombre.clone()),
    };
    let (ancho, alto) = f.tamano();
    FichaDeFigura {
        id: f.id.clone(),
        nombre,
        grupo,
        de_serie: f.id.starts_with(PREFIJO_ID),
        ancho,
        alto,
        piezas: piezas_de(f),
        letras: letras_de(f),
    }
}

/// La escena de un lienzo con la figura dentro (`figuras::escena_con`).
pub(crate) fn escena_con(f: &FiguraGuardada) -> Escena {
    let mut escena = Escena::nueva();
    let (w, h) = f.tamano();
    biblioteca::estampar_en_escena(&mut escena, f, Punto2::nuevo(w / 2.0, h / 2.0));
    escena
}

/// Las categorias de serie, en su orden (para ordenar los grupos).
#[uniffi::export]
pub fn figuras_categorias() -> Vec<String> {
    CATEGORIAS.iter().map(|c| c.to_string()).collect()
}

impl Nucleo {
    fn figura(&self, id: &str) -> Result<Option<FiguraGuardada>, ErrorNucleo> {
        Ok(self.biblioteca_cargar()?.into_iter().find(|f| f.id == id))
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Las figuras de la galeria**, en el orden de la biblioteca (las
    /// nuevas arriba), con su grupo, lo que miden, sus piezas y sus textos.
    pub fn galeria_de_figuras(&self) -> Result<Vec<FichaDeFigura>, ErrorNucleo> {
        Ok(self.biblioteca_cargar()?.iter().map(ficha_de).collect())
    }

    /// **El lienzo de la figura `id`**, para pintarla (sus ordenes con
    /// `plano()` y su caja). Nada si ya no esta.
    pub fn lienzo_de_figura(&self, id: String) -> Result<Option<Arc<Lienzo>>, ErrorNucleo> {
        Ok(self
            .figura(&id)?
            .map(|f| Arc::new(Lienzo::de_escena(escena_con(&f)))))
    }

    /// **«Insertar»**: un lienzo nuevo al final del chat con la figura
    /// dentro y su nombre (sin la categoria), como `adjuntar_figura` de
    /// Windows.
    pub fn adjuntar_figura(&self, chat: String, id: String) -> Result<MensajeChat, ErrorNucleo> {
        validar_chat(&chat)?;
        let f = self.figura(&id)?.ok_or(ErrorNucleo::NoExiste { que: id })?;
        let nombre = categoria_de(&f.nombre).map_or(f.nombre.clone(), |(_, n)| n.to_string());
        let m = self.crear_lienzo_con(chat.clone(), Some(nombre), |_| {})?;
        if let Some(r) = m.referencia.as_deref() {
            let lienzo = excalidraw::con_escena(&excalidraw::Lienzo::vacio(), &escena_con(&f));
            std::fs::write(
                pixpin_proyecto::almacen::lienzo(self.ruta(), &chat, r),
                excalidraw::escribir(&lienzo),
            )?;
        }
        Ok(MensajeChat::from(m))
    }

    /// **«Guardar en mis figuras»** de un lienzo del chat (la `referencia`
    /// de un `DIBUJO`): lo que se ve en el, con `nombre` (la del mismo
    /// nombre se sustituye). `false` si el lienzo esta vacio.
    pub fn guardar_lienzo_como_figura(
        &self,
        chat: String,
        dibujo: String,
        nombre: String,
    ) -> Result<bool, ErrorNucleo> {
        let lienzo = self.abrir_lienzo(chat, dibujo)?;
        let piezas: Vec<_> = lienzo.escena().visibles().cloned().collect();
        if piezas.is_empty() {
            return Ok(false);
        }
        let nombre: String = nombre.trim().chars().take(80).collect();
        let nombre = if nombre.is_empty() { "Figura".to_string() } else { nombre };
        self.biblioteca_anadir(FiguraGuardada {
            id: biblioteca::id_nuevo(),
            nombre,
            elementos: biblioteca::normalizar(&piezas),
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::elemento::Elemento;

    fn pieza(id: u64, figura: Figura, x: f32, y: f32) -> Elemento {
        Elemento {
            id,
            figura,
            x,
            y,
            ancho: 10.0,
            alto: 10.0,
            semilla: 7,
            ..Default::default()
        }
    }

    #[test]
    fn el_grupo_sale_del_nombre_y_se_cuentan_las_piezas() {
        assert_eq!(categoria_de("Diario · Gratitud"), Some(("Diario", "Gratitud")));
        // Casos negativos: sin separador, o con una parte vacia.
        assert_eq!(categoria_de("Flecha"), None);
        assert_eq!(categoria_de(" · Algo"), None);
        let f = FiguraGuardada {
            id: "serie-gratitud".into(),
            nombre: "Diario · Gratitud".into(),
            elementos: vec![
                pieza(1, Figura::Rectangulo, 0.0, 0.0),
                pieza(2, Figura::Elipse, 20.0, 0.0),
            ],
        };
        let ficha = ficha_de(&f);
        assert_eq!((ficha.grupo.as_str(), ficha.nombre.as_str()), ("Diario", "Gratitud"));
        assert!(ficha.de_serie);
        assert_eq!(ficha.piezas.total, 2);
        assert_eq!(ficha.piezas.formas, 2);
        assert!(ficha.letras.is_empty());
        let escena = escena_con(&f);
        assert_eq!(escena.visibles().count(), 2);
        assert_eq!(figuras_categorias().len(), 35);
    }

    #[test]
    fn un_lienzo_del_chat_se_guarda_como_figura_y_se_inserta_otra_vez() {
        let d = std::env::temp_dir().join(format!("pixpin-nucleo-galeria-fig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let n = Nucleo::abrir(d.display().to_string(), "iPad".into()).unwrap();
        let chat = n.crear_chat("Obra".into()).unwrap().id;
        let m = n.crear_lienzo(chat.clone(), None).unwrap();
        let r = m.referencia.clone().unwrap();
        // Caso negativo: un lienzo vacio no se guarda.
        assert!(!n.guardar_lienzo_como_figura(chat.clone(), r.clone(), "Nada".into()).unwrap());
        let f = FiguraGuardada {
            id: "x".into(),
            nombre: "x".into(),
            elementos: vec![pieza(1, Figura::Rectangulo, 0.0, 0.0)],
        };
        let lienzo = excalidraw::con_escena(&excalidraw::Lienzo::vacio(), &escena_con(&f));
        std::fs::write(
            pixpin_proyecto::almacen::lienzo(&d, &chat, &r),
            excalidraw::escribir(&lienzo),
        )
        .unwrap();
        assert!(n.guardar_lienzo_como_figura(chat.clone(), r, "Caja".into()).unwrap());
        let galeria = n.galeria_de_figuras().unwrap();
        let caja = galeria.iter().find(|g| g.nombre == "Caja").unwrap();
        assert_eq!(caja.grupo, "");
        assert!(!caja.de_serie);
        let l = n.lienzo_de_figura(caja.id.clone()).unwrap().unwrap();
        assert_eq!(l.cuantos_elementos(), 1);
        let puesto = n.adjuntar_figura(chat.clone(), caja.id.clone()).unwrap();
        assert_eq!(puesto.nombre, "Caja");
        assert_eq!(puesto.clase, "DIBUJO");
        // Casos negativos: una figura que no esta.
        assert!(n.lienzo_de_figura("nadie".into()).unwrap().is_none());
        assert!(n.adjuntar_figura(chat, "nadie".into()).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
