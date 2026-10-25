//! **Las figuras del lienzo** (F12, F14 de Windows): la grafica de una
//! funcion, la tabla en blanco o pegada, los instrumentos de graficar (la
//! recta, el plano y el espacio) y la biblioteca de figuras guardadas.
//!
//! Copia de lo que en Windows es `ventana_editor/figuras.rs` sin la ventana
//! (el menu y los cajetines son de Swift) y de `biblioteca_figuras.rs`: la
//! geometria es del motor (`grafica`, `tabla_dibujada`, `instrumento`,
//! `biblioteca`). Todo entra en medio de la vista, agrupado, en un paso de
//! deshacer y elegido, para llevarlo a su sitio de un tiron.
//!
//! # Donde vive la biblioteca
//!
//! Donde la dejan Windows y el movil: `figuras.json` en la carpeta del chat
//! «Asistente» (`android/pins/draw/figuras.json`, que en el movil es
//! `pixpin:files/pins/draw/figuras.json`), senalada por un mensaje ARCHIVO
//! fijo (`auto-figuras`) para que la sincronizacion la lleve a los aparatos
//! propios. El formato es el del movil (`biblioteca::a_json`).

use std::path::{Path, PathBuf};

use pixpin_motor2d::biblioteca::{self, FiguraGuardada};
use pixpin_motor2d::ecuacion::Estilo;
use pixpin_motor2d::elemento::Elemento;
use pixpin_motor2d::gesto::Gesto;
use pixpin_motor2d::grafica::{self, Fallo, Peticion};
use pixpin_motor2d::vector::Punto2;

use crate::editor::EditorLienzo;
use crate::{ErrorNucleo, Nucleo};

/// El id del mensaje que senala la biblioteca (`biblioteca_figuras.rs`).
pub const ID_MENSAJE: &str = "auto-figuras";
/// Donde la deja el movil.
pub const RUTA_PORTATIL: &str = "pixpin:files/pins/draw/figuras.json";
/// Dentro de la carpeta del chat «Asistente».
pub const RUTA_EN_EL_CHAT: &str = "android/pins/draw/figuras.json";
/// El texto del mensaje que la senala.
const TEXTO_DEL_MENSAJE: &str =
    "🧩 Las figuras guardadas del lienzo (se usan desde el menu de figuras)";

/// Una figura de la biblioteca, para el menu.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FiguraDeBiblioteca {
    pub id: String,
    pub nombre: String,
}

/// **El pincel de ahora como estilo de figura** (`estilo_del_pincel` de
/// Windows): el color, la letra y la opacidad que el panel dejo como
/// «actuales».
pub(crate) fn estilo_del_pincel(g: &Gesto) -> Estilo {
    let tam = if g.estilo.tamano_letra > 0.0 {
        g.estilo.tamano_letra
    } else {
        pixpin_motor2d::texto::TAM_POR_DEFECTO
    };
    Estilo {
        color: g.estilo.trazo,
        tam,
        opacidad: g.estilo.opacidad,
        familia: pixpin_motor2d::texto::nombre_de_familia(g.estilo.familia).to_string(),
    }
}

/// Lo que mide un texto con la letra `familia`: la cuenta del motor (en el
/// Mac no hay medidor de DirectWrite).
pub(crate) fn medir(familia: &str) -> impl Fn(&str, f32) -> (f32, f32) + '_ {
    move |texto, tam| pixpin_motor2d::texto::medida(texto, tam, familia, Default::default())
}

/// **El instrumento de fabrica con el pincel de ahora** (`instrumento_del_pincel`).
fn instrumento_del_pincel(g: &Gesto, ejes: u8) -> Elemento {
    let mut e = pixpin_motor2d::instrumento::elemento_de_fabrica(
        ejes,
        g.estilo.trazo,
        g.estilo.grosor.de_forma(),
        g.estilo.opacidad,
    );
    e.material = g.estilo.material;
    e.extras.familia = g
        .estilo
        .familia
        .map(|f| pixpin_motor2d::texto::nombre_de_familia(Some(f)).to_string());
    e
}

/// La peticion de una grafica, o el aviso de lo que falla (los textos de
/// `grafica-*` de Windows).
// `!(b > a)` es a proposito: tambien rechaza los NaN.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn peticion_de_grafica(
    formulas: &[String],
    x_desde: f64,
    x_hasta: f64,
    y_desde: f64,
    y_hasta: f64,
    escala: f64,
) -> Result<Peticion, String> {
    let p = Peticion {
        formulas: formulas
            .iter()
            .flat_map(|f| f.lines())
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        x_desde,
        x_hasta,
        y_desde,
        y_hasta,
        escala,
    };
    let limites =
        "Los límites no tienen sentido: «desde» tiene que ser menor que «hasta»".to_string();
    match grafica::compilar(&p) {
        Err(Fallo::SinFormula) => Err("Escribe una fórmula".into()),
        Err(Fallo::NoSeEntiende(s)) => Err(format!("No se entiende: {s}")),
        Err(Fallo::OtraVariable(s)) => Err(format!("Solo puede usar la x: {s}")),
        Err(Fallo::Limites) => Err(limites),
        Ok(_) if !(x_hasta > x_desde) || !(y_hasta > y_desde) || !(escala > 0.0) => Err(limites),
        Ok(_) => Ok(p),
    }
}

/// **Las celdas de una tabla pegada** (lo que trae el portapapeles copiando
/// unas celdas de Excel o de Sheets: renglones y tabuladores). `None` si el
/// texto no es una tabla.
#[uniffi::export]
pub fn celdas_de_texto(texto: String) -> Option<Vec<Vec<String>>> {
    pixpin_motor2d::tabla_dibujada::rejilla_de_texto(&texto)
}

/// **Si una grafica se puede dibujar asi**: `None` si si; el aviso de lo
/// que falla si no (para el cajetin, mientras se teclea).
#[uniffi::export]
pub fn aviso_de_grafica(
    formulas: Vec<String>,
    x_desde: f64,
    x_hasta: f64,
    y_desde: f64,
    y_hasta: f64,
    escala: f64,
) -> Option<String> {
    peticion_de_grafica(&formulas, x_desde, x_hasta, y_desde, y_hasta, escala).err()
}

impl EditorLienzo {
    /// **Estampa unos elementos en `centro`**, agrupados, en un paso de
    /// deshacer, y los deja elegidos con la mano puesta
    /// (`estampar_en_la_vista` de Windows).
    fn estampar(&self, nombre: &str, elementos: &[Elemento], centro: Punto2) -> bool {
        if elementos.is_empty() {
            return false;
        }
        let figura = FiguraGuardada {
            id: biblioteca::id_nuevo(),
            nombre: nombre.to_string(),
            elementos: biblioteca::normalizar(elementos),
        };
        self.estampar_figura(&figura, centro)
    }

    fn estampar_figura(&self, figura: &FiguraGuardada, centro: Punto2) -> bool {
        let mut guarda = self.tomar();
        let st = &mut *guarda;
        if !st.gesto.en_reposo() {
            return false;
        }
        st.gesto.cerrar_texto(&mut st.escena);
        let ids = biblioteca::estampar_en_escena(&mut st.escena, figura, centro);
        if ids.is_empty() {
            return false;
        }
        st.gesto
            .tomar_herramienta(pixpin_motor2d::gesto::Herramienta::Mano);
        st.gesto.seleccion.poner_todos(ids);
        drop(guarda);
        self.subir_cambios();
        self.subir_version();
        true
    }
}

#[uniffi::export]
impl EditorLienzo {
    /// **La grafica de una funcion** (una o varias curvas en `formulas`, una
    /// por renglon, en funcion de `x`), con sus ejes y su ecuacion, en la
    /// ventana `x_desde..x_hasta` x `y_desde..y_hasta` a `escala` pixeles la
    /// unidad; con el pincel de ahora, centrada en `(centro_x, centro_y)`.
    // Los numeros sueltos y no un registro: es lo que teclea el cajetin.
    #[allow(clippy::too_many_arguments)]
    pub fn estampar_grafica(
        &self,
        formulas: Vec<String>,
        x_desde: f64,
        x_hasta: f64,
        y_desde: f64,
        y_hasta: f64,
        escala: f64,
        centro_x: f32,
        centro_y: f32,
    ) -> Result<bool, ErrorNucleo> {
        let p = peticion_de_grafica(&formulas, x_desde, x_hasta, y_desde, y_hasta, escala)
            .map_err(|detalle| ErrorNucleo::Argumento { detalle })?;
        let estilo = estilo_del_pincel(&self.tomar().gesto);
        let medidor = medir(&estilo.familia);
        let v = grafica::elementos(&p, &estilo, &medidor).map_err(|e| ErrorNucleo::Argumento {
            detalle: format!("{e:?}"),
        })?;
        Ok(self.estampar("grafica", &v, Punto2::nuevo(centro_x, centro_y)))
    }

    /// **Una tabla dibujada** con estas `celdas` (filas de textos; en blanco
    /// tambien: una tabla en blanco es donde escribir), la primera fila de
    /// cabecera si `cabecera`, con la letra del pincel.
    pub fn estampar_tabla(
        &self,
        celdas: Vec<Vec<String>>,
        cabecera: bool,
        centro_x: f32,
        centro_y: f32,
    ) -> bool {
        if celdas.is_empty() || celdas.iter().all(Vec::is_empty) {
            return false;
        }
        let estilo = estilo_del_pincel(&self.tomar().gesto);
        let medidor = medir(&estilo.familia);
        // Celda a celda y no por `elementos_de_tabla`, que quita las filas
        // en blanco del final: una tabla en blanco es donde escribir
        // (`tabla_de_celdas` de Windows). Las filas, todas del mismo ancho.
        use pixpin_motor2d::tabla_dibujada::{Celda, elementos_de_tabla_con_juntas};
        let ancho = celdas.iter().map(Vec::len).max().unwrap_or(0);
        let rejilla: Vec<Vec<Celda>> = celdas
            .iter()
            .map(|f| {
                (0..ancho)
                    .map(|i| Celda::de(f.get(i).map_or(String::new(), |c| c.trim().to_string())))
                    .collect()
            })
            .collect();
        let v = elementos_de_tabla_con_juntas(
            &rejilla,
            &estilo,
            Punto2::nuevo(0.0, 0.0),
            &medidor,
            cabecera,
        );
        self.estampar("tabla", &v, Punto2::nuevo(centro_x, centro_y))
    }

    /// **El instrumento de graficar** con `ejes` ejes: 1 la recta de
    /// funciones, 2 el plano y 3 el espacio xyz.
    pub fn estampar_instrumento(&self, ejes: u8, centro_x: f32, centro_y: f32) -> bool {
        if !(1..=3).contains(&ejes) {
            return false;
        }
        let e = instrumento_del_pincel(&self.tomar().gesto, ejes);
        self.estampar("instrumento", &[e], Punto2::nuevo(centro_x, centro_y))
    }
}

// ---------------------------------------------------------------------------
// La biblioteca
// ---------------------------------------------------------------------------

/// La ruta de la biblioteca en `raiz`, creando el chat «Asistente» si no
/// estaba (`biblioteca_figuras::ruta_en`), con la carpeta y el id del chat.
fn ruta_en(raiz: &Path, aparato: &str, ahora: i64) -> std::io::Result<(PathBuf, String, PathBuf)> {
    let f = pixpin_proyecto::almacen::asegurar_asistente(raiz, ahora, aparato)?;
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &f.id);
    Ok((carpeta.join(RUTA_EN_EL_CHAT), f.id, carpeta))
}

/// Pone (una vez) el mensaje ARCHIVO del chat que hace viajar la biblioteca
/// (`automatizaciones::senalar_fichero` de Windows).
fn senalar(carpeta: &Path, proyecto: &str, aparato: &str, ahora: i64) -> std::io::Result<()> {
    use pixpin_proyecto::cuaderno;
    let c = match cuaderno::Cuaderno::leer_de(carpeta) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => return Err(e),
    };
    if c.mensajes.iter().any(|m| m.id == ID_MENSAJE) {
        return Ok(());
    }
    let numero = c.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let bytes = std::fs::metadata(carpeta.join(RUTA_EN_EL_CHAT))
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    let mut m = cuaderno::Mensaje::adjunto(
        cuaderno::Clase::Archivo,
        "figuras.json",
        RUTA_PORTATIL,
        bytes,
        &cuaderno::Sello {
            cuando: ahora,
            numero,
            aparato: aparato.to_string(),
            proyecto: proyecto.to_string(),
        },
    );
    m.id = ID_MENSAJE.into();
    m.texto = TEXTO_DEL_MENSAJE.into();
    cuaderno::anadir(carpeta, &m)
}

impl Nucleo {
    /// Las figuras de la biblioteca (el asistente: `figuras_guardadas`).
    pub(crate) fn biblioteca_cargar(&self) -> Result<Vec<FiguraGuardada>, ErrorNucleo> {
        let (ruta, ..) = self.biblioteca()?;
        Ok(biblioteca::cargar(&ruta))
    }

    /// Guarda `nueva` arriba de la lista (la del mismo nombre se sustituye).
    pub(crate) fn biblioteca_anadir(&self, nueva: FiguraGuardada) -> Result<bool, ErrorNucleo> {
        let (ruta, ..) = self.biblioteca()?;
        let mut lista = biblioteca::cargar(&ruta);
        lista.retain(|f| !f.nombre.eq_ignore_ascii_case(&nueva.nombre));
        lista.insert(0, nueva);
        self.guardar_biblioteca(&lista)
    }

    fn biblioteca(&self) -> Result<(PathBuf, String, PathBuf, String), ErrorNucleo> {
        let aparato = self.identidad()?.yo.codigo();
        let (ruta, proyecto, carpeta) = ruta_en(self.ruta(), &aparato, crate::ahora_ms())?;
        Ok((ruta, proyecto, carpeta, aparato))
    }

    fn guardar_biblioteca(&self, figuras: &[FiguraGuardada]) -> Result<bool, ErrorNucleo> {
        let (ruta, proyecto, carpeta, aparato) = self.biblioteca()?;
        if !biblioteca::guardar(&ruta, figuras) {
            return Ok(false);
        }
        senalar(&carpeta, &proyecto, &aparato, crate::ahora_ms())?;
        Ok(true)
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Las figuras guardadas**, las nuevas arriba.
    pub fn figuras_guardadas(&self) -> Result<Vec<FiguraDeBiblioteca>, ErrorNucleo> {
        let (ruta, ..) = self.biblioteca()?;
        Ok(biblioteca::cargar(&ruta)
            .into_iter()
            .map(|f| FiguraDeBiblioteca {
                id: f.id,
                nombre: f.nombre,
            })
            .collect())
    }

    /// **Guarda lo elegido en `editor` como figura** con ese nombre (la del
    /// mismo nombre se sustituye), arriba de la lista. `false` si no hay
    /// nada elegido.
    pub fn guardar_figura(
        &self,
        editor: Arc<EditorLienzo>,
        nombre: String,
    ) -> Result<bool, ErrorNucleo> {
        let nombre = nombre.trim();
        let nombre = if nombre.is_empty() { "Figura" } else { nombre };
        let nueva = {
            let st = editor.tomar();
            biblioteca::de_la_seleccion(&st.escena.elementos, st.gesto.seleccion.ids(), nombre)
        };
        let Some(nueva) = nueva else { return Ok(false) };
        let (ruta, ..) = self.biblioteca()?;
        let mut lista = biblioteca::cargar(&ruta);
        lista.retain(|f| !f.nombre.eq_ignore_ascii_case(&nueva.nombre));
        lista.insert(0, nueva);
        self.guardar_biblioteca(&lista)
    }

    /// Quita la figura `id` de la biblioteca.
    pub fn quitar_figura(&self, id: String) -> Result<bool, ErrorNucleo> {
        let (ruta, ..) = self.biblioteca()?;
        let mut lista = biblioteca::cargar(&ruta);
        let antes = lista.len();
        lista.retain(|f| f.id != id);
        if lista.len() == antes {
            return Ok(false);
        }
        self.guardar_biblioteca(&lista)
    }

    /// **Estampa la figura `id`** en `editor`, centrada en `(centro_x,
    /// centro_y)` y elegida.
    pub fn estampar_figura(
        &self,
        editor: Arc<EditorLienzo>,
        id: String,
        centro_x: f32,
        centro_y: f32,
    ) -> Result<bool, ErrorNucleo> {
        let (ruta, ..) = self.biblioteca()?;
        let Some(f) = biblioteca::cargar(&ruta).into_iter().find(|f| f.id == id) else {
            return Ok(false);
        };
        Ok(editor.estampar_figura(&f, Punto2::nuevo(centro_x, centro_y)))
    }
}

use std::sync::Arc;

#[cfg(test)]
mod pruebas {
    use super::*;

    fn editor(nombre: &str) -> (Arc<EditorLienzo>, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("pixpin-figuras-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ed =
            EditorLienzo::de_fichero(dir.join("f.excalidraw").display().to_string(), true).unwrap();
        (ed, dir)
    }

    #[test]
    fn la_grafica_la_tabla_y_el_plano_entran_agrupados_y_elegidos_en_un_paso() {
        let (ed, dir) = editor("estampar");
        assert!(
            ed.estampar_grafica(
                vec!["sin(x)".into()],
                -5.0,
                5.0,
                -3.0,
                3.0,
                40.0,
                100.0,
                100.0
            )
            .unwrap()
        );
        let n = ed.cuantos_elementos();
        assert!(n > 3, "ejes, curva y ecuacion: {n}");
        assert_eq!(ed.cuantos_elegidos(), n);
        assert_eq!(ed.herramienta_actual(), "seleccion");
        assert!(ed.deshacer());
        assert_eq!(ed.cuantos_elementos(), 0, "un solo paso de deshacer");

        let celdas = celdas_de_texto("a\tb\n1\t2\n".into()).expect("es una tabla");
        assert_eq!(
            celdas,
            vec![
                vec!["a".to_string(), "b".into()],
                vec!["1".into(), "2".into()]
            ]
        );
        assert!(ed.estampar_tabla(celdas, true, 0.0, 0.0));
        assert!(
            ed.estampar_tabla(vec![vec![String::new(); 3]; 4], true, 0.0, 0.0),
            "en blanco"
        );
        assert!(ed.cuantos_elementos() > 0);
        assert!(ed.estampar_instrumento(2, 0.0, 0.0));
        // Casos negativos: sin formula, limites al reves, ejes que no hay.
        assert!(
            ed.estampar_grafica(vec![" ".into()], -5.0, 5.0, -3.0, 3.0, 40.0, 0.0, 0.0)
                .is_err()
        );
        assert!(aviso_de_grafica(vec!["x".into()], 5.0, -5.0, -3.0, 3.0, 40.0).is_some());
        assert!(aviso_de_grafica(vec!["y*2".into()], -5.0, 5.0, -3.0, 3.0, 40.0).is_some());
        assert!(aviso_de_grafica(vec!["x^2".into()], -5.0, 5.0, -3.0, 3.0, 40.0).is_none());
        assert!(!ed.estampar_instrumento(4, 0.0, 0.0));
        assert!(celdas_de_texto("solo un parrafo".into()).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn la_biblioteca_vive_en_el_chat_del_asistente_y_viaja() {
        let raiz = std::env::temp_dir().join(format!("pixpin-biblio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let nucleo = Nucleo::abrir(raiz.display().to_string(), "Mac".into()).unwrap();
        let (ed, dir) = editor("biblio");
        ed.herramienta("rectangulo".into()).unwrap();
        ed.puntero(crate::editor::FasePuntero::Pulsar, 0.0, 0.0, None, 0, 1.0);
        ed.puntero(crate::editor::FasePuntero::Mover, 50.0, 40.0, None, 0, 1.0);
        ed.puntero(crate::editor::FasePuntero::Soltar, 50.0, 40.0, None, 0, 1.0);
        // Sin nada elegido no se guarda nada.
        ed.herramienta("seleccion".into()).unwrap();
        assert!(!nucleo.guardar_figura(ed.clone(), "Caja".into()).unwrap());
        ed.seleccionar_todo();
        assert!(nucleo.guardar_figura(ed.clone(), "Caja".into()).unwrap());
        assert!(
            nucleo.guardar_figura(ed.clone(), "caja".into()).unwrap(),
            "la del mismo nombre se sustituye"
        );
        let lista = nucleo.figuras_guardadas().unwrap();
        assert_eq!(lista.len(), 1);
        // Donde la busca el movil, y senalada para que viaje.
        let (ruta, _, carpeta) =
            ruta_en(&raiz, &nucleo.identidad().unwrap().yo.codigo(), 1).unwrap();
        assert!(ruta.ends_with(RUTA_EN_EL_CHAT) && ruta.is_file());
        let c = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        assert_eq!(c.mensajes.iter().filter(|m| m.id == ID_MENSAJE).count(), 1);

        assert!(
            nucleo
                .estampar_figura(ed.clone(), lista[0].id.clone(), 300.0, 300.0)
                .unwrap()
        );
        assert_eq!(ed.cuantos_elementos(), 2);
        assert!(
            !nucleo
                .estampar_figura(ed.clone(), "no-esta".into(), 0.0, 0.0)
                .unwrap()
        );
        assert!(nucleo.quitar_figura(lista[0].id.clone()).unwrap());
        assert!(nucleo.figuras_guardadas().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(raiz);
    }
}
