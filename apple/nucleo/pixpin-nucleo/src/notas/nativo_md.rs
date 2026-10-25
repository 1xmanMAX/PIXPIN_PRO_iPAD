//! **Lo que el editor nativo de notas del iPad pide al Markdown** (el
//! `UITextView`, cuyo texto ES el Markdown): el menu de la barra `/`
//! (`pixpin_docs::md_comandos`, el de Windows y el movil), las tablas de
//! barras para editarlas como rejilla (`pixpin_docs::md_tabla`) y las fotos
//! de la nota con su renglon (`pixpin_docs::md_imagen`). Todas las
//! posiciones van en UTF-16 (`NSRange`).

use pixpin_docs::md_comandos::{self as mc, Bloque};
use pixpin_docs::md_imagen;
use pixpin_docs::md_tabla::{self, Alineacion, Tabla};

use super::TextoElegido;

/// Lo que se esta escribiendo tras una barra al principio del renglon.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ConsultaBarraMd {
    /// Donde esta la barra.
    pub barra: u32,
    /// Lo tecleado detras (lo que filtra el menu).
    pub escrito: String,
}

/// Una entrada del menu de la barra.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ComandoBarraMd {
    /// `titulo1`, `lista`, `tabla`, `imagen`...: lo que se pide al elegirla.
    pub clave: String,
    pub nombre: String,
    /// El atajo de Markdown (en gris, a la derecha).
    pub atajo: String,
}

/// Lo que el iPad sabe poner (los de proyecto, chat, documento y audio los
/// pone el editor de Windows con sus ventanas).
const DEL_IPAD: [(Bloque, &str, &str, &str); 12] = [
    (Bloque::Titulo1, "titulo1", "Título 1", "#"),
    (Bloque::Titulo2, "titulo2", "Título 2", "##"),
    (Bloque::Titulo3, "titulo3", "Título 3", "###"),
    (Bloque::Lista, "lista", "Lista con viñetas", "-"),
    (Bloque::Numerada, "numerada", "Lista numerada", "1."),
    (Bloque::Casillas, "casillas", "Casillas", "[]"),
    (Bloque::Cita, "cita", "Cita", ">"),
    (Bloque::Codigo, "codigo", "Código", "```"),
    (Bloque::Tabla, "tabla", "Tabla", ""),
    (Bloque::Imagen, "imagen", "Imagen", ""),
    (Bloque::Fecha, "fecha", "Fecha de hoy", ""),
    (Bloque::Separador, "separador", "Separador", "---"),
];

fn nombre(b: Bloque) -> String {
    DEL_IPAD
        .iter()
        .find(|x| x.0 == b)
        .map(|x| x.2.to_string())
        .unwrap_or_default()
}

/// Si el cursor esta escribiendo un comando de la barra (`/` al principio
/// del renglon y letras detras).
#[uniffi::export]
pub fn md_barra_consulta(texto: String, cursor: u32) -> Option<ConsultaBarraMd> {
    mc::consulta(&texto, cursor as usize).map(|(barra, escrito)| ConsultaBarraMd {
        barra: barra as u32,
        escrito,
    })
}

/// Las entradas que encajan con lo tecleado, en el orden de Windows.
#[uniffi::export]
pub fn md_barra_comandos(escrito: String) -> Vec<ComandoBarraMd> {
    mc::buscar(&escrito, &nombre)
        .into_iter()
        .filter_map(|b| DEL_IPAD.iter().find(|x| x.0 == b))
        .map(|(_, clave, nombre, atajo)| ComandoBarraMd {
            clave: clave.to_string(),
            nombre: nombre.to_string(),
            atajo: atajo.to_string(),
        })
        .collect()
}

const MESES: [&str; 12] = [
    "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic",
];

/// **Pone el bloque `clave`** en lugar de la barra y lo tecleado
/// (`desde..hasta`): el texto nuevo y el cursor (en `desde` y `hasta`, que
/// son iguales). `ahora_local_ms`: la hora de este equipo (para la fecha).
/// `None` para `imagen` (la pone el editor, con su selector de fotos) o una
/// clave que no se conoce.
#[uniffi::export]
pub fn md_barra_poner(
    texto: String,
    desde: u32,
    hasta: u32,
    clave: String,
    ahora_local_ms: i64,
) -> Option<TextoElegido> {
    let b = DEL_IPAD.iter().find(|x| x.1 == clave)?.0;
    let (antes, despues): (String, String) = match mc::plantilla(b) {
        Some((a, d)) => (a.into(), d.into()),
        None => match b {
            Bloque::Tabla => (format!("{}\n", md_tabla::a_gfm(&Tabla::nueva(3, 3))), String::new()),
            Bloque::Fecha => {
                let (d, m, y) = mc::dia_de(ahora_local_ms);
                let meses: Vec<String> = MESES.iter().map(|s| s.to_string()).collect();
                (mc::fecha(d, m, y, &meses), String::new())
            }
            _ => return None,
        },
    };
    let (nuevo, cursor) = mc::poner(&texto, desde as usize, hasta as usize, &antes, &despues);
    Some(TextoElegido {
        texto: nuevo,
        desde: cursor as u32,
        hasta: cursor as u32,
    })
}

/// **Una tabla de barras** de la nota, con donde esta.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TablaMd {
    /// De donde a donde va (UTF-16, sin el ultimo salto).
    pub desde: u32,
    pub hasta: u32,
    /// Las filas (la primera es la cabecera) con el texto de cada celda.
    pub filas: Vec<Vec<String>>,
    /// `i`, `c` o `d` por columna.
    pub alineaciones: Vec<String>,
}

fn letra_de(a: Alineacion) -> String {
    match a {
        Alineacion::Izquierda => "i",
        Alineacion::Centro => "c",
        Alineacion::Derecha => "d",
    }
    .into()
}

/// Las tablas de barras de `texto` (una fila de barras con la de guiones
/// debajo), fuera de los bloques de codigo.
#[uniffi::export]
pub fn md_tablas(texto: String) -> Vec<TablaMd> {
    let mut v = Vec::new();
    let mut renglones: Vec<(usize, &str)> = Vec::new();
    let mut pos = 0usize;
    for r in texto.split('\n') {
        renglones.push((pos, r));
        pos += r.encode_utf16().count() + 1;
    }
    let mut en_codigo = false;
    let mut i = 0;
    while i < renglones.len() {
        let t = renglones[i].1.trim();
        if t.starts_with("```") {
            en_codigo = !en_codigo;
            i += 1;
            continue;
        }
        if !en_codigo
            && t.starts_with('|')
            && renglones
                .get(i + 1)
                .is_some_and(|r| md_tabla::es_separadora(r.1))
        {
            let mut j = i + 2;
            while j < renglones.len() && renglones[j].1.trim().starts_with('|') {
                j += 1;
            }
            let desde = renglones[i].0;
            let ultimo = renglones[j - 1];
            let hasta = ultimo.0 + ultimo.1.trim_end_matches('\r').encode_utf16().count();
            let bloque: Vec<&str> = renglones[i..j].iter().map(|r| r.1).collect();
            if let Some(t) = md_tabla::leer_gfm(&bloque.join("\n")) {
                let n = t.columnas();
                v.push(TablaMd {
                    desde: desde as u32,
                    hasta: hasta as u32,
                    filas: t
                        .filas
                        .iter()
                        .map(|f| (0..n).map(|c| f.get(c).cloned().unwrap_or_default()).collect())
                        .collect(),
                    alineaciones: (0..n).map(|c| letra_de(t.alineacion(c))).collect(),
                });
            }
            i = j;
            continue;
        }
        i += 1;
    }
    v
}

/// **Una tabla escrita en Markdown** como la escribe el movil (sin salto al
/// final). Las filas cortas se completan.
#[uniffi::export]
pub fn md_tabla_texto(filas: Vec<Vec<String>>, alineaciones: Vec<String>) -> String {
    let n = filas.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let t = Tabla {
        filas: if filas.is_empty() {
            vec![vec![String::new(); n]]
        } else {
            filas
        },
        alineaciones: (0..n)
            .map(|c| match alineaciones.get(c).map(String::as_str) {
                Some("c") => Alineacion::Centro,
                Some("d") => Alineacion::Derecha,
                _ => Alineacion::Izquierda,
            })
            .collect(),
        formato: Vec::new(),
    };
    md_tabla::a_gfm(&t)
}

/// Una foto de la nota con su renglon.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FotoMd {
    /// El renglon (desde 0).
    pub linea: u32,
    pub alt: String,
    /// El ancho pedido (pixeles a 96 ppp) o nada (a lo ancho de la columna).
    pub ancho: Option<u32>,
    /// La ruta tal cual va en el Markdown (`Nucleo::ruta_en_la_nota`).
    pub ruta: String,
}

/// Las fotos `![…](…)` de la nota, una por renglon (las de un bloque de
/// codigo no cuentan).
#[uniffi::export]
pub fn md_fotos(texto: String) -> Vec<FotoMd> {
    md_imagen::fotos(&texto)
        .into_iter()
        .map(|(linea, f)| FotoMd {
            linea: linea as u32,
            alt: f.alt,
            ancho: f.ancho,
            ruta: f.ruta,
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_barra_filtra_y_pone_el_bloque() {
        let c = md_barra_consulta("hola\n/tab".into(), 9).unwrap();
        assert_eq!((c.barra, c.escrito.as_str()), (5, "tab"));
        assert_eq!(md_barra_comandos("tab".into())[0].clave, "tabla");
        assert_eq!(md_barra_comandos("".into()).len(), 12);
        let p = md_barra_poner("hola\n/t1".into(), 5, 8, "titulo1".into(), 0).unwrap();
        assert_eq!(p.texto, "hola\n# ");
        assert_eq!(p.desde, 7);
        let t = md_barra_poner("/".into(), 0, 1, "tabla".into(), 0).unwrap();
        assert!(t.texto.starts_with("|  |  |  |\n|:---|:---|:---|"), "{}", t.texto);
        // La fecha de un dia conocido (1 de enero de 1970).
        assert_eq!(md_barra_poner("/".into(), 0, 1, "fecha".into(), 0).unwrap().texto, "1 ene 1970");
        // Casos negativos: en mitad de una frase no hay menu; la imagen la
        // pone el editor.
        assert!(md_barra_consulta("a/b".into(), 3).is_none());
        assert!(md_barra_poner("/".into(), 0, 1, "imagen".into(), 0).is_none());
    }

    #[test]
    fn las_tablas_se_encuentran_y_se_vuelven_a_escribir() {
        let texto = "Antes\n| A | B |\n|:---|---:|\n| 1 | 2 |\nDespues\n```\n| x |\n|---|\n```";
        let t = md_tablas(texto.into());
        assert_eq!(t.len(), 1, "la del bloque de codigo no cuenta");
        assert_eq!(t[0].desde, 6);
        assert_eq!(&texto[t[0].desde as usize..t[0].hasta as usize], "| A | B |\n|:---|---:|\n| 1 | 2 |");
        assert_eq!(t[0].filas, vec![vec!["A", "B"], vec!["1", "2"]]);
        assert_eq!(t[0].alineaciones, vec!["i", "d"]);
        let escrita = md_tabla_texto(vec![vec!["A".into(), "B".into()], vec!["x|y".into()]], t[0].alineaciones.clone());
        assert_eq!(escrita, "| A | B |\n|:---|---:|\n| x\\|y |  |");
    }

    #[test]
    fn las_fotos_salen_con_su_renglon() {
        let f = md_fotos("Hola\n![planta|320](notas/1-a.png)\n```\n![no](x.png)\n```".into());
        assert_eq!(f.len(), 1);
        assert_eq!((f[0].linea, f[0].ancho, f[0].ruta.as_str()), (1, Some(320), "notas/1-a.png"));
    }
}
