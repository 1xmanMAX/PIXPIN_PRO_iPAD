//! **Editar una tabla ya dibujada** (`ventana_editor/figuras.rs`,
//! `editar_tabla` de Windows): de lo elegido se leen otra vez las filas, las
//! columnas y lo escrito en cada celda (`tabla_dibujada::leer`), el cajetin
//! de Swift las cambia, y se vuelve a dibujar en el mismo sitio y grupo,
//! con la letra y el color que ya tenia y sus figuras encajadas en su celda.

use pixpin_motor2d::elemento::Elemento;
use pixpin_motor2d::tabla_dibujada::{Celda, leer};

use crate::editor::EditorLienzo;

/// Una tabla leida de lo elegido, para el cajetin.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TablaDelLienzo {
    /// Lo escrito en cada celda, fila a fila (todas del mismo ancho).
    pub celdas: Vec<Vec<String>>,
    /// Si la primera fila lleva el fondo de cabecera.
    pub cabecera: bool,
}

#[uniffi::export]
impl EditorLienzo {
    /// **La tabla elegida**, si lo elegido es una tabla dibujada (con o sin
    /// figuras metidas). `None` si no.
    pub fn tabla_elegida(&self) -> Option<TablaDelLienzo> {
        let st = self.tomar();
        let elegidos: Vec<&Elemento> =
            st.escena.visibles().filter(|e| st.gesto.seleccion.contiene(e.id)).collect();
        let t = leer::leer_tabla(&elegidos)?;
        Some(TablaDelLienzo {
            celdas: t.celdas.iter().map(|f| f.iter().map(|c| c.texto.clone()).collect()).collect(),
            cabecera: t.cabecera,
        })
    }

    /// **Vuelve a dibujar la tabla elegida con estas celdas** (una fila mas,
    /// una columna menos, otro texto), en un paso de deshacer, y deja elegida
    /// la nueva. `false` si no hay tabla elegida, si no cambia nada o si las
    /// celdas vienen vacias.
    pub fn reemplazar_tabla(&self, celdas: Vec<Vec<String>>, cabecera: bool) -> bool {
        let ancho = celdas.iter().map(Vec::len).max().unwrap_or(0);
        if celdas.is_empty() || ancho == 0 {
            return false;
        }
        // Todas las filas del mismo ancho, como las deja el cajetin.
        let textos: Vec<Vec<String>> = celdas
            .iter()
            .map(|f| (0..ancho).map(|i| f.get(i).map_or(String::new(), |c| c.trim().to_string())).collect())
            .collect();
        let hecho = {
            let mut guarda = self.tomar();
            let st = &mut *guarda;
            if !st.gesto.en_reposo() {
                return false;
            }
            st.gesto.cerrar_texto(&mut st.escena);
            let vieja = {
                let elegidos: Vec<&Elemento> =
                    st.escena.visibles().filter(|e| st.gesto.seleccion.contiene(e.id)).collect();
                match leer::leer_tabla(&elegidos) {
                    Some(t) => t,
                    None => return false,
                }
            };
            let antes: Vec<Vec<String>> =
                vieja.celdas.iter().map(|f| f.iter().map(|c| c.texto.clone()).collect()).collect();
            if antes == textos && cabecera == vieja.cabecera {
                return false;
            }
            // Con la letra y el color que ya tenia la tabla, no con los del
            // pincel de ahora.
            let mut estilo = crate::editor_figuras::estilo_del_pincel(&st.gesto);
            if let Some(tam) = vieja.tam {
                estilo.tam = tam;
            }
            if let Some(familia) = &vieja.familia {
                estilo.familia = familia.clone();
            }
            if let Some(marco) = vieja.de_la_tabla.first().and_then(|id| st.escena.buscar(*id)) {
                estilo.color = marco.trazo;
                estilo.opacidad = marco.opacidad;
            }
            // La negrita de cada celda se queda con su celda si sigue.
            let nuevas: Vec<Vec<Celda>> = textos
                .iter()
                .enumerate()
                .map(|(i, fila)| {
                    fila.iter()
                        .enumerate()
                        .map(|(j, texto)| Celda {
                            negrita: vieja.celdas.get(i).and_then(|f| f.get(j)).is_some_and(|c| c.negrita),
                            ..Celda::de(texto.clone())
                        })
                        .collect()
                })
                .collect();
            let medir = crate::editor_figuras::medir(&estilo.familia);
            let ids = leer::rehacer_en_escena(&mut st.escena, &vieja, &nuevas, cabecera, &estilo, &medir);
            if ids.is_empty() {
                false
            } else {
                st.gesto.seleccion.limpiar();
                st.gesto.seleccion.poner_todos(ids);
                true
            }
        };
        if hecho {
            self.subir_cambios();
            self.subir_version();
        }
        hecho
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn editor(n: &str) -> std::sync::Arc<EditorLienzo> {
        let d = std::env::temp_dir().join(format!("pixpin-nucleo-tabla-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        EditorLienzo::de_fichero(d.join("t.excalidraw").display().to_string(), true).unwrap()
    }

    #[test]
    fn se_lee_y_se_le_anade_una_fila() {
        let e = editor("fila");
        assert_eq!(e.tabla_elegida(), None);
        let celdas = vec![vec!["a".to_string(), "b".to_string()], vec!["1".to_string(), "2".to_string()]];
        assert!(e.estampar_tabla(celdas.clone(), true, 0.0, 0.0));
        let t = e.tabla_elegida().expect("la tabla recien puesta queda elegida");
        assert_eq!(t.celdas, celdas);
        assert!(t.cabecera);
        // Lo mismo no cambia nada.
        assert!(!e.reemplazar_tabla(celdas.clone(), true));
        let mut mas = celdas.clone();
        mas.push(vec!["3".into(), "4".into()]);
        assert!(e.reemplazar_tabla(mas.clone(), true));
        assert_eq!(e.tabla_elegida().unwrap().celdas, mas);
        // Se deshace de una vez.
        assert!(e.deshacer());
        assert_eq!(e.tabla_elegida().map(|t| t.celdas.len()).unwrap_or(2), 2);
    }
}
