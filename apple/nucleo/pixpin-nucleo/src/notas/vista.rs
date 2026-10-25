//! **La letra y el tamano con que se ve una nota** (H12, 30-sep): una
//! preferencia de VISTA, no del texto. El `.md` no la lleva —el movil no
//! tendria donde leerla y la nota se veria distinta en cada aparato sin que
//! nadie la cambiara—; se guarda en un fichero de ajustes local, una
//! general y, si se pide, una para una nota concreta.
//!
//! Las letras son las ocho del lienzo (las de Excalidraw y las de la
//! aplicacion de citas), las mismas que el usuario ya conoce de sus dibujos.
//!
//! El fichero es texto, un renglon por vista: `clave\tcuerpo\ttitulos\tpx`,
//! con `*` como clave de la general. Lo que no se entiende se salta.
//!
//! Copia de `pixpin-notas/src/vista.rs` de Windows (el mismo
//! `vista-de-notas.txt`, al lado del almacen y sin viajar).

/// Las letras que se pueden elegir: las del lienzo.
pub const LETRAS: [&str; 8] = [
    "Excalifont",
    "Nunito",
    "Lilita One",
    "Comic Shanns",
    "Work Sans",
    "Fraunces",
    "Courier New",
    "Caveat",
];

/// Los tamanos con nombre, en pixeles a 96 ppp: pequena, normal, grande y
/// muy grande. Con Ctrl+rueda se llega a cualquier otro entre los topes.
pub const TAMANOS: [u32; 4] = [14, 16, 18, 21];
pub const TAMANO_MINIMO: u32 = 10;
pub const TAMANO_MAXIMO: u32 = 32;

/// La clave de la vista general en el fichero.
const GENERAL: &str = "*";

/// Como se ve una nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vista {
    /// La familia del cuerpo (una de [`LETRAS`]).
    pub cuerpo: String,
    /// La de los titulos.
    pub titulos: String,
    /// El tamano del cuerpo, en pixeles a 96 ppp (16 es el de siempre).
    pub px: u32,
}

impl Default for Vista {
    /// La de siempre: titulos en serif (Fraunces) y cuerpo sans (Work
    /// Sans), como el editor de Claude.
    fn default() -> Self {
        Vista {
            cuerpo: "Work Sans".into(),
            titulos: "Fraunces".into(),
            px: 16,
        }
    }
}

impl Vista {
    /// Cuanto se agranda todo respecto al tamano de siempre.
    pub fn factor(&self) -> f32 {
        self.px as f32 / 16.0
    }

    /// El tamano un paso mas grande (`mas`) o mas pequeno, dentro de los topes.
    pub fn otro_tamano(&self, mas: bool) -> u32 {
        if mas {
            (self.px + 1).min(TAMANO_MAXIMO)
        } else {
            self.px.saturating_sub(1).max(TAMANO_MINIMO)
        }
    }
}

fn limpia(clave: &str) -> String {
    clave.replace(['\t', '\n', '\r'], " ")
}

fn de_renglon(r: &str) -> Option<(String, Vista)> {
    let mut c = r.split('\t');
    let clave = c.next()?.to_string();
    let cuerpo = c.next()?;
    let titulos = c.next()?;
    let px: u32 = c.next()?.trim().parse().ok()?;
    let valida = |f: &str| {
        LETRAS
            .iter()
            .find(|l| l.eq_ignore_ascii_case(f.trim()))
            .map(|l| l.to_string())
    };
    Some((
        clave,
        Vista {
            cuerpo: valida(cuerpo)?,
            titulos: valida(titulos)?,
            px: px.clamp(TAMANO_MINIMO, TAMANO_MAXIMO),
        },
    ))
}

/// La vista de una nota: la suya si la tiene (y `true`), si no la general,
/// y si tampoco, la de siempre.
pub fn leer(contenido: &str, nota: Option<&str>) -> (Vista, bool) {
    let todas: Vec<(String, Vista)> = contenido.lines().filter_map(de_renglon).collect();
    if let Some(n) = nota.map(limpia)
        && let Some((_, v)) = todas.iter().find(|(c, _)| *c == n)
    {
        return (v.clone(), true);
    }
    let general = todas
        .into_iter()
        .find(|(c, _)| c == GENERAL)
        .map(|(_, v)| v);
    (general.unwrap_or_default(), false)
}

/// El fichero con `vista` puesta como la de la nota `nota` (o como la
/// general si es `None`). Lo demas se queda como estaba.
pub fn escribir(contenido: &str, nota: Option<&str>, vista: &Vista) -> String {
    let clave = nota.map(limpia).unwrap_or_else(|| GENERAL.to_string());
    let renglon = format!("{clave}\t{}\t{}\t{}", vista.cuerpo, vista.titulos, vista.px);
    let mut fuera: Vec<String> = contenido
        .lines()
        .filter(|r| de_renglon(r).is_none_or(|(c, _)| c != clave))
        .map(String::from)
        .collect();
    fuera.push(renglon);
    fuera.join("\n") + "\n"
}

/// El fichero sin la vista propia de la nota `nota`: vuelve a la general.
pub fn olvidar(contenido: &str, nota: &str) -> String {
    let clave = limpia(nota);
    let fuera: Vec<&str> = contenido
        .lines()
        .filter(|r| de_renglon(r).is_none_or(|(c, _)| c != clave))
        .collect();
    if fuera.is_empty() {
        String::new()
    } else {
        fuera.join("\n") + "\n"
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_fichero_la_vista_es_la_de_siempre() {
        assert_eq!(leer("", Some("nota-1")), (Vista::default(), false));
        assert_eq!(Vista::default().factor(), 1.0);
    }

    #[test]
    fn la_vista_de_una_nota_manda_sobre_la_general_y_las_demas_usan_la_general() {
        let grande = Vista {
            cuerpo: "Nunito".into(),
            titulos: "Lilita One".into(),
            px: 21,
        };
        let chica = Vista {
            cuerpo: "Caveat".into(),
            titulos: "Caveat".into(),
            px: 14,
        };
        let f = escribir("", None, &grande);
        let f = escribir(&f, Some("pins/notas/a.md"), &chica);
        assert_eq!(leer(&f, Some("pins/notas/a.md")), (chica.clone(), true));
        assert_eq!(leer(&f, Some("otra")), (grande.clone(), false));
        // Escribir otra vez la misma clave la cambia, no la repite.
        let f = escribir(&f, None, &Vista::default());
        assert_eq!(f.lines().count(), 2);
        assert_eq!(leer(&f, Some("otra")).0, Vista::default());
        // Olvidarla vuelve a la general.
        let f = olvidar(&f, "pins/notas/a.md");
        assert_eq!(leer(&f, Some("pins/notas/a.md")), (Vista::default(), false));
    }

    #[test]
    fn un_renglon_roto_o_con_una_letra_que_no_es_del_lienzo_se_salta() {
        let f = "*\tComic Sans MS\tFraunces\t16\nbasura\n*\tNunito\tFraunces\tmucho\n";
        assert_eq!(leer(f, None), (Vista::default(), false));
        // Un tamano fuera de los topes se queda en el tope.
        let f = "*\tNunito\tFraunces\t200\n";
        assert_eq!(leer(f, None).0.px, TAMANO_MAXIMO);
    }

    #[test]
    fn el_tamano_sube_y_baja_de_uno_en_uno_sin_pasar_los_topes() {
        let mut v = Vista::default();
        assert_eq!(v.otro_tamano(true), 17);
        v.px = TAMANO_MAXIMO;
        assert_eq!(v.otro_tamano(true), TAMANO_MAXIMO);
        v.px = TAMANO_MINIMO;
        assert_eq!(v.otro_tamano(false), TAMANO_MINIMO);
    }

    #[test]
    fn una_clave_con_tabuladores_no_rompe_el_fichero() {
        let v = Vista {
            px: 18,
            ..Default::default()
        };
        let f = escribir("", Some("a\tb\nc"), &v);
        assert_eq!(f.lines().count(), 1);
        assert_eq!(leer(&f, Some("a\tb\nc")), (v, true));
    }
}
