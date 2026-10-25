//! Regiones guardadas con nombre (P2.3).
//!
//! Para capturar siempre el mismo trozo de pantalla sin volver a marcarlo:
//! un panel de un programa, una ventana que siempre esta en el mismo sitio,
//! la mitad izquierda de un monitor. Se declaran en el TOML, cada una con
//! su atajo, y capturan directamente sin abrir el overlay.
//!
//! **Los identificadores viven aparte de los comandos.** `Comando::id()` va
//! del uno al tamano del catalogo, asi que las regiones empiezan en mil. Si
//! compartieran espacio, anadir un comando nuevo cambiaria el significado
//! de los atajos de las regiones ya registradas, y eso pasaria sin un solo
//! aviso.

use serde::{Deserialize, Serialize};

use pixpin_sistema::Atajo;

/// El primer identificador de region. Muy por encima del ultimo comando,
/// para que anadir comandos nunca alcance a las regiones.
pub const PRIMER_ID: u32 = 1000;

/// Una region guardada, tal cual se escribe en el TOML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    /// Como la llama el usuario. Sale en el registro y, mas adelante, en la
    /// ventana de ajustes.
    pub nombre: String,
    pub x: i32,
    pub y: i32,
    pub ancho: u32,
    pub alto: u32,
    /// Su atajo. Sin el, la region queda declarada pero todavia no hay
    /// forma de dispararla: la lista en el menu de bandeja llega con P6.
    #[serde(default)]
    pub atajo: Option<String>,
}

impl Region {
    /// Si la region tiene medidas que sirven para algo.
    ///
    /// Una de lado cero no es un descuido que haya que perdonar en
    /// silencio: capturaria una imagen vacia y el usuario creeria que la
    /// captura fallo. Vale mas descartarla y decirlo.
    pub fn es_util(&self) -> bool {
        self.ancho > 0 && self.alto > 0
    }
}

/// El identificador de la region que ocupa el sitio `indice`.
pub fn id_de(indice: usize) -> u32 {
    PRIMER_ID + indice as u32
}

/// El sitio en la lista al que corresponde un identificador, si es de una
/// region y no de un comando.
pub fn desde_id(id: u32) -> Option<usize> {
    (id >= PRIMER_ID).then(|| (id - PRIMER_ID) as usize)
}

/// Las peticiones de atajo de todas las regiones que tengan uno valido.
///
/// Lo que no se entiende se descarta con su motivo, en vez de tumbar el
/// arranque: un atajo mal escrito en una region no puede dejar a nadie sin
/// PixPin.
pub fn registrables(regiones: &[Region]) -> (Vec<(u32, Atajo)>, Vec<String>) {
    let mut peticiones = Vec::new();
    let mut avisos = Vec::new();
    for (indice, region) in regiones.iter().enumerate() {
        if !region.es_util() {
            avisos.push(format!(
                "la region «{}» mide {}x{} y no se puede capturar",
                region.nombre, region.ancho, region.alto
            ));
            continue;
        }
        let Some(texto) = region.atajo.as_deref().filter(|t| !t.is_empty()) else {
            continue;
        };
        match texto.parse::<Atajo>() {
            Ok(atajo) => peticiones.push((id_de(indice), atajo)),
            Err(_) => avisos.push(format!(
                "el atajo «{texto}» de la region «{}» no se entiende",
                region.nombre
            )),
        }
    }
    (peticiones, avisos)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn region(nombre: &str, atajo: Option<&str>) -> Region {
        Region {
            nombre: nombre.into(),
            x: 10,
            y: 20,
            ancho: 300,
            alto: 200,
            atajo: atajo.map(|a| a.into()),
        }
    }

    #[test]
    fn los_identificadores_no_pisan_a_los_comandos() {
        // Es la razon de ser del espacio aparte: si se solaparan, anadir un
        // comando cambiaria en silencio que hace el atajo de una region.
        let ultimo_comando = crate::comandos::CATALOGO.len() as u32;
        assert!(
            PRIMER_ID > ultimo_comando,
            "el catalogo tiene {ultimo_comando} comandos y las regiones empiezan en {PRIMER_ID}"
        );
        assert_eq!(desde_id(ultimo_comando), None);
        assert_eq!(desde_id(id_de(0)), Some(0));
        assert_eq!(desde_id(id_de(7)), Some(7));
    }

    #[test]
    fn solo_se_registran_las_que_tienen_atajo() {
        let lista = vec![
            region("con atajo", Some("Ctrl+Alt+1")),
            region("sin atajo", None),
            region("atajo vacio", Some("")),
        ];
        let (peticiones, avisos) = registrables(&lista);
        assert_eq!(peticiones.len(), 1);
        assert_eq!(peticiones[0].0, id_de(0));
        assert!(avisos.is_empty(), "{avisos:?}");
    }

    #[test]
    fn un_atajo_ilegible_se_descarta_con_su_motivo() {
        // Caso negativo: no puede tumbar el arranque ni desplazar los
        // identificadores de las que vienen detras.
        let lista = vec![
            region("mal escrita", Some("Ctrl+Alt+")),
            region("buena", Some("Ctrl+Alt+2")),
        ];
        let (peticiones, avisos) = registrables(&lista);
        assert_eq!(peticiones.len(), 1);
        assert_eq!(
            peticiones[0].0,
            id_de(1),
            "la buena conserva SU sitio, el uno"
        );
        assert_eq!(avisos.len(), 1);
        assert!(avisos[0].contains("mal escrita"), "{}", avisos[0]);
    }

    #[test]
    fn una_region_de_lado_cero_no_se_registra() {
        // Caso negativo: capturaria una imagen vacia y pareceria que la
        // captura fallo, que es de lo mas dificil de achacar al TOML.
        let mut r = region("plana", Some("Ctrl+Alt+3"));
        r.alto = 0;
        assert!(!r.es_util());
        let (peticiones, avisos) = registrables(&[r]);
        assert!(peticiones.is_empty());
        assert_eq!(avisos.len(), 1);
        assert!(avisos[0].contains("plana"), "{}", avisos[0]);
    }

    #[test]
    fn una_lista_vacia_no_pide_nada() {
        let (peticiones, avisos) = registrables(&[]);
        assert!(peticiones.is_empty());
        assert!(avisos.is_empty());
    }
}
