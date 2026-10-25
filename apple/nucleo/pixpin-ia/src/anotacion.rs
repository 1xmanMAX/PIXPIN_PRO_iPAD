//! **Anotar un documento con cabeza** (15-oct-2026, pedido del usuario:
//! «todos los formatos tienen un canvas integrado con una capa superior; el
//! asistente deberia poder hacer anotaciones ahi de forma inteligente:
//! senalarme puntos importantes con un cuadradito y la explicacion al
//! costado, y cosas asi»).
//!
//! El modelo **no ve pixeles ni inventa coordenadas**: recibe el texto de la
//! pagina renglon a renglon (la capa de texto del PDF, la maquetacion de un
//! Word o un PowerPoint, el OCR con cajas de una foto o un escaneado, los
//! textos de un plano o las celdas de una hoja) y devuelve, una por linea,
//! anotaciones **ancladas a un trozo del texto** (`ancla`) o a una celda.
//! Aqui se encuentra ese trozo ([`ancla`]), se busca sitio en el margen
//! libre para la explicacion sin pisar otra ([`colocar`]) y se hacen las
//! figuras con el motor del lienzo ([`figuras`]): recuadro o resaltado sobre
//! lo senalado, la nota al costado y una linea guia que no tapa el texto.
//!
//! Un solo componente con variables para todos los formatos: lo unico que
//! cambia de un formato a otro es como se leen las [`Pagina`]s (en las
//! unidades de su capa) y donde se escriben las figuras, que lo hace la app
//! (`apps/pixpin/src/asistente/anotar_inteligente.rs`). Lo que se pide
//! —senalar lo importante, numerar pasos, marcar errores con su correccion,
//! resumir, un glosario, preguntas de repaso, explicar algo al costado— es
//! la [`Clase`] de cada anotacion, no otro motor.
//!
//! **Latencia**: la respuesta llega en vivo y [`flujo::Flujo`] suelta cada
//! anotacion en cuanto se cierra su objeto JSON: la primera se ve mientras
//! el modelo aun escribe las demas.

pub mod ancla;
pub mod colocar;
pub mod figuras;
pub mod flujo;

#[cfg(test)]
mod pruebas;

use std::collections::{HashMap, HashSet};

use pixpin_motor2d::Elemento;
use serde_json::Value;

use crate::acciones::normalizar;

/// El nombre de la herramienta.
pub const NOMBRE: &str = "anotar_inteligente";

/// Lo que llevan delante los grupos de lo que pone un turno: el grupo de
/// cada turno es `asistente-anotado:<turno>`, y con el se sabe que deshacer
/// junto (un Ctrl+Z por turno) y que borrar.
pub const GRUPO_TURNO: &str = "asistente-anotado:";

/// Lo que se le pide al modelo, como mucho, por pagina: mas notas no caben
/// en el margen sin taparse ni se leen.
pub const MAX_POR_PAGINA: usize = 7;

/// Cuanto texto del documento va al modelo (letras): unas 4-5 paginas.
pub const MAX_LETRAS: usize = 14_000;

/// Que es cada anotacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Clase {
    /// Lo importante: recuadro y la explicacion al costado.
    Importante,
    /// Un paso numerado.
    Paso,
    /// Un error marcado con su correccion.
    Error,
    /// El resumen de una pagina o una seccion, en el margen.
    Resumen,
    /// Un termino y su definicion (glosario).
    Definicion,
    /// Una pregunta de repaso sobre lo senalado.
    Pregunta,
    /// La explicacion de algo (una formula, una frase dificil).
    Explicacion,
}

impl Clase {
    pub const NOMBRES: [&'static str; 7] = [
        "importante",
        "paso",
        "error",
        "resumen",
        "definicion",
        "pregunta",
        "explicacion",
    ];

    /// De lo que escribe el modelo (con sus sinonimos habituales).
    pub fn de(s: &str) -> Option<Clase> {
        let s = normalizar(s.trim());
        Some(match s.as_str() {
            "importante" | "clave" | "destacado" | "idea" | "idea clave" => Clase::Importante,
            "paso" | "pasos" | "numero" | "numerar" => Clase::Paso,
            "error" | "errores" | "correccion" | "falta" => Clase::Error,
            "resumen" | "resumir" | "sintesis" => Clase::Resumen,
            "definicion" | "glosario" | "termino" | "concepto" => Clase::Definicion,
            "pregunta" | "preguntas" | "repaso" => Clase::Pregunta,
            "explicacion" | "explicar" | "explica" | "nota" | "formula" => Clase::Explicacion,
            _ => return None,
        })
    }

    pub fn nombre(self) -> &'static str {
        Clase::NOMBRES[self as usize]
    }

    /// El color de la anotacion (recuadro, linea y titulo de la nota).
    pub fn color(self) -> (u8, u8, u8) {
        match self {
            Clase::Importante => (0xe8, 0x59, 0x0c),
            Clase::Paso => (0x1c, 0x7e, 0xd6),
            Clase::Error => (0xe0, 0x31, 0x31),
            Clase::Resumen => (0x5f, 0x3d, 0xc4),
            Clase::Definicion => (0x2b, 0x8a, 0x3e),
            Clase::Pregunta => (0x0c, 0x85, 0x99),
            Clase::Explicacion => (0x86, 0x5d, 0x0b),
        }
    }

    /// El titulo de la nota.
    pub fn titulo(self) -> &'static str {
        match self {
            Clase::Importante => "Importante",
            Clase::Paso => "Paso",
            Clase::Error => "Error",
            Clase::Resumen => "Resumen",
            Clase::Definicion => "Definición",
            Clase::Pregunta => "Pregunta",
            Clase::Explicacion => "Explicación",
        }
    }
}

// ---------------------------------------------------------------------------
// El documento, con donde cae cada palabra

/// Una palabra con su caja `[x0, y0, x1, y1]` en las unidades de la capa
/// (y hacia abajo).
#[derive(Debug, Clone, PartialEq)]
pub struct Palabra {
    pub texto: String,
    pub caja: [f32; 4],
}

impl Palabra {
    pub fn nueva(texto: &str, caja: [f32; 4]) -> Palabra {
        Palabra {
            texto: texto.to_string(),
            caja,
        }
    }
}

/// Un renglon: sus palabras en orden de lectura. En una hoja de calculo
/// cada celda es un renglon con su `etiqueta` («B4»).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Renglon {
    pub palabras: Vec<Palabra>,
    pub etiqueta: Option<String>,
}

impl Renglon {
    pub fn texto(&self) -> String {
        self.palabras
            .iter()
            .map(|p| p.texto.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn caja(&self) -> Option<[f32; 4]> {
        union(self.palabras.iter().map(|p| p.caja))
    }
}

/// La caja que abarca todas (o `None` si no hay ninguna).
pub fn union(cajas: impl IntoIterator<Item = [f32; 4]>) -> Option<[f32; 4]> {
    cajas.into_iter().fold(None, |u: Option<[f32; 4]>, c| {
        Some(match u {
            None => c,
            Some(u) => [
                u[0].min(c[0]),
                u[1].min(c[1]),
                u[2].max(c[2]),
                u[3].max(c[3]),
            ],
        })
    })
}

/// **Una pagina para anotar**, en las unidades de su capa: la hoja de un
/// PDF o un Word (1400 de ancho), la foto (sus pixeles), el plano (la capa
/// del plano) o la hoja de calculo (sus pixeles al 100 %).
#[derive(Debug, Clone, PartialEq)]
pub struct Pagina {
    /// Desde 0 (en un plano, una foto o una hoja de calculo, 0).
    pub numero: usize,
    /// La hoja entera: lo que es «la pagina».
    pub caja: [f32; 4],
    pub renglones: Vec<Renglon>,
    /// El alto de la letra de las notas: legible al zoom normal.
    pub letra: f32,
    /// Cuanto se puede salir de la hoja para las notas, a cada lado (el
    /// espacio para anotar del lector, el lienzo alrededor de una foto o de
    /// un plano); 0, nada.
    pub fuera: f32,
    /// Una hoja de calculo: las notas solo a la derecha de lo usado (a la
    /// izquierda de la columna A no hay nada, y dentro todo son celdas),
    /// y pueden bajar mas alla de la ultima fila.
    pub solo_derecha: bool,
}

impl Pagina {
    pub fn vacia(&self) -> bool {
        self.renglones.iter().all(|r| r.palabras.is_empty())
    }

    /// **La columna del texto**: donde empieza y acaba la mayoria de los
    /// renglones (sin el numero de pagina ni una nota al margen, que no
    /// mueven el borde). `None` sin texto.
    pub fn columna(&self) -> Option<[f32; 4]> {
        let cajas: Vec<[f32; 4]> = self.renglones.iter().filter_map(Renglon::caja).collect();
        if cajas.is_empty() {
            return None;
        }
        let mut x0: Vec<f32> = cajas.iter().map(|c| c[0]).collect();
        let mut x1: Vec<f32> = cajas.iter().map(|c| c[2]).collect();
        x0.sort_by(f32::total_cmp);
        x1.sort_by(f32::total_cmp);
        // El 10 % mas a la izquierda y el 10 % mas a la derecha no cuentan.
        let k = (cajas.len() as f32 * 0.1).floor() as usize;
        let izq = x0[k.min(x0.len() - 1)];
        let der = x1[x1.len() - 1 - k.min(x1.len() - 1)];
        let y0 = cajas.iter().map(|c| c[1]).fold(f32::MAX, f32::min);
        let y1 = cajas.iter().map(|c| c[3]).fold(f32::MIN, f32::max);
        Some([izq, y0, der.max(izq + 1.0), y1])
    }

    /// **Donde pueden ir las notas**: los margenes de la hoja que tengan
    /// sitio para una nota legible y, si se puede salir de la hoja, el
    /// espacio de fuera. Los de dentro primero.
    pub fn franjas(&self) -> Vec<colocar::Franja> {
        let l = self.letra.max(1.0);
        let aire = l * 0.6;
        let minimo = l * colocar::LETRAS_MINIMAS;
        let col =
            self.columna()
                .unwrap_or([self.caja[0], self.caja[1], self.caja[0], self.caja[1]]);
        let (y0, y1) = (self.caja[1] + aire, self.caja[3] - aire);
        let mut v = Vec::new();
        let izq = [self.caja[0] + aire, col[0] - aire];
        let der = [col[2] + aire, self.caja[2] - aire];
        // En una hoja de calculo no hay margenes: lo que no se leyo (fuera del
        // rango pedido) tambien son celdas.
        if !self.solo_derecha && izq[1] - izq[0] >= minimo {
            v.push(colocar::Franja {
                x0: izq[0],
                x1: izq[1],
                y0,
                y1,
                lado: colocar::Lado::Izquierda,
                fuera: false,
            });
        }
        if !self.solo_derecha && der[1] - der[0] >= minimo {
            v.push(colocar::Franja {
                x0: der[0],
                x1: der[1],
                y0,
                y1,
                lado: colocar::Lado::Derecha,
                fuera: false,
            });
        }
        if self.fuera >= minimo {
            let ancho = self.fuera - aire;
            // En una hoja de calculo se puede bajar mas alla de lo usado: una
            // de tres filas no deja sitio ni para una nota (en un PDF no: alli
            // abajo empieza la hoja siguiente).
            let y1 = if self.solo_derecha {
                y1.max(y0 + l * 120.0)
            } else {
                y1
            };
            v.push(colocar::Franja {
                x0: self.caja[2] + aire,
                x1: self.caja[2] + aire + ancho,
                y0,
                y1,
                lado: colocar::Lado::Derecha,
                fuera: true,
            });
            if !self.solo_derecha {
                v.push(colocar::Franja {
                    x0: self.caja[0] - aire - ancho,
                    x1: self.caja[0] - aire,
                    y0,
                    y1,
                    lado: colocar::Lado::Izquierda,
                    fuera: true,
                });
            }
        }
        v
    }
}

// ---------------------------------------------------------------------------
// Lo que se le pide al modelo

/// Las instrucciones del anotador (el mensaje de sistema).
pub fn instrucciones(hoja_de_calculo: bool) -> String {
    let ancla = if hoja_de_calculo {
        "la celda o el rango al que se refiere, tal cual (\"B4\", \"B4:D9\")"
    } else {
        "el trozo EXACTO del documento al que se refiere, copiado letra por letra del renglon (de 2 a 12 palabras; una palabra sola si es un termino)"
    };
    format!(
        "Eres el anotador de documentos de PixPin. Recibes el texto de una o varias paginas renglon a renglon \
         (cada renglon con su numero «L12»{celdas}) y el pedido del usuario. Contestas SOLO con anotaciones, \
         una por linea, cada una un objeto JSON en UNA sola linea; nada de texto antes ni despues, ni ``` ni listas.\n\
         Campos:\n\
         - \"tipo\": importante | paso | error | resumen | definicion | pregunta | explicacion\n\
         - \"pag\": el numero de la pagina (el de «=== Pagina N ===»)\n\
         - \"ancla\": {ancla}\n\
         - \"l\": el numero del renglon donde empieza el ancla{l_celdas}\n\
         - \"nota\": lo que va al margen, en el idioma del pedido del usuario (aunque el documento este en otro), claro y breve (como mucho 25 palabras). \
         En «resumen», 1 a 3 frases; en «pregunta», la pregunta de repaso; en «definicion», «termino: definicion»; \
         en «explicacion», que significa o como se lee (una formula: que es cada simbolo).\n\
         - \"corr\": solo en «error»: como deberia decir\n\
         - \"n\": solo en «paso»: su numero (1, 2, 3...)\n\
         Reglas: el ancla SIEMPRE copiada del texto, nunca inventada ni resumida; como mucho {max} anotaciones \
         por pagina; la mas util primero; no anotes dos veces el mismo trozo; nada obvio ni de relleno. \
         Un «resumen» de la pagina entera lleva de ancla el primer renglon de la pagina (o de la seccion que resume). \
         Si no hay nada que anotar, una sola linea {{\"tipo\":\"nada\",\"nota\":\"por que\"}}.",
        celdas = if hoja_de_calculo {
            " o su celda «B4»"
        } else {
            ""
        },
        l_celdas = if hoja_de_calculo {
            " (en una hoja de calculo, omitelo)"
        } else {
            ""
        },
        max = MAX_POR_PAGINA,
    )
}

/// **El texto que ve el modelo**: el pedido y las paginas, renglon a
/// renglon, recortado a `max_letras`.
pub fn mensaje(pedido: &str, paginas: &[Pagina], max_letras: usize) -> String {
    let mut s = format!("Pedido del usuario: {}\n\n", pedido.trim());
    let mut recortado = false;
    'paginas: for p in paginas {
        s.push_str(&format!("=== Pagina {} ===\n", p.numero + 1));
        for (i, r) in p.renglones.iter().enumerate() {
            if r.palabras.is_empty() {
                continue;
            }
            let linea = match &r.etiqueta {
                Some(e) => format!("{e}: {}\n", r.texto()),
                None => format!("L{}: {}\n", i + 1, r.texto()),
            };
            if s.chars().count() + linea.chars().count() > max_letras {
                recortado = true;
                break 'paginas;
            }
            s.push_str(&linea);
        }
        s.push('\n');
    }
    if recortado {
        s.push_str("[...el resto no cabe: anota solo lo que ves aqui]\n");
    }
    s
}

// ---------------------------------------------------------------------------
// Lo que contesta el modelo

/// Una anotacion pedida por el modelo, ya leida.
#[derive(Debug, Clone, PartialEq)]
pub struct Pedida {
    pub clase: Clase,
    /// La pagina que dijo (desde 0).
    pub pagina: Option<usize>,
    pub ancla: String,
    /// El renglon que dijo (desde 1, como se le dio).
    pub linea: Option<usize>,
    pub nota: String,
    pub correccion: Option<String>,
    pub numero: Option<u32>,
}

/// Lo que dice un objeto del modelo: una anotacion, «nada que anotar» (con
/// el motivo) o algo que no se entiende.
#[derive(Debug, Clone, PartialEq)]
pub enum Leida {
    Anotacion(Pedida),
    Nada(String),
    Mal(String),
}

fn texto_de(v: &Value, campos: &[&str]) -> Option<String> {
    campos.iter().find_map(|c| match v.get(*c) {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    })
}

fn numero_de(v: &Value, campos: &[&str]) -> Option<u64> {
    campos.iter().find_map(|c| match v.get(*c) {
        Some(Value::Number(n)) => n
            .as_u64()
            .or_else(|| n.as_f64().filter(|x| *x >= 0.0).map(|x| x as u64)),
        Some(Value::String(s)) => s.trim().trim_start_matches(['L', 'l']).parse().ok(),
        _ => None,
    })
}

impl Leida {
    pub fn de(v: &Value) -> Leida {
        let Some(tipo) = texto_de(v, &["tipo", "type"]) else {
            return Leida::Mal("sin «tipo»".into());
        };
        if normalizar(&tipo) == "nada" {
            return Leida::Nada(texto_de(v, &["nota"]).unwrap_or_default());
        }
        let Some(clase) = Clase::de(&tipo) else {
            return Leida::Mal(format!("tipo «{tipo}» desconocido"));
        };
        let ancla = texto_de(v, &["ancla", "texto", "anchor"]).unwrap_or_default();
        let nota = texto_de(v, &["nota", "explicacion", "note"]).unwrap_or_default();
        if ancla.is_empty() && clase != Clase::Resumen {
            return Leida::Mal("sin «ancla»".into());
        }
        if nota.is_empty() && !matches!(clase, Clase::Paso | Clase::Error) {
            return Leida::Mal("sin «nota»".into());
        }
        Leida::Anotacion(Pedida {
            clase,
            pagina: numero_de(v, &["pag", "pagina", "page"])
                .filter(|p| *p >= 1)
                .map(|p| p as usize - 1),
            ancla,
            linea: numero_de(v, &["l", "linea", "renglon"])
                .filter(|l| *l >= 1)
                .map(|l| l as usize),
            nota,
            correccion: texto_de(v, &["corr", "correccion", "correcto"]),
            numero: numero_de(v, &["n", "numero", "paso"]).map(|n| n as u32),
        })
    }
}

// ---------------------------------------------------------------------------
// De lo pedido a las figuras

/// Lo que quedo puesto (para contarselo al modelo y al banco).
#[derive(Debug, Clone, PartialEq)]
pub struct Puesta {
    /// Que pagina de [`Anotador::paginas`] (su indice en la lista).
    pub en: usize,
    pub clase: Clase,
    pub ancla: String,
    /// Si la nota salio de la hoja (al espacio para anotar), y a que lado.
    pub fuera: bool,
    pub lado: colocar::Lado,
    pub elementos: Vec<Elemento>,
}

/// **El anotador de un turno**: las paginas leidas, donde ya hay notas en
/// cada una y lo que ya se puso. Cada [`Pedida`] que llega se ancla, se
/// coloca y sale hecha figuras, en el orden en que llegan.
pub struct Anotador {
    pub paginas: Vec<Pagina>,
    colocadores: HashMap<usize, colocar::Colocador>,
    turno: String,
    puestas: usize,
    vistas: HashSet<(usize, String)>,
    pub por_clase: HashMap<Clase, usize>,
    pub no_encontradas: Vec<String>,
    pub sin_sitio: usize,
}

impl Anotador {
    /// `turno`: lo que distingue lo de este turno (va en el grupo).
    pub fn nuevo(paginas: Vec<Pagina>, turno: &str) -> Anotador {
        let colocadores = paginas
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mut c = colocar::Colocador::nuevo(p.franjas(), p.letra * 0.5);
                // Lo escrito fuera de la columna (el numero de pagina, una
                // nota al margen que ya estaba) no se tapa.
                if let Some(col) = p.columna() {
                    for r in &p.renglones {
                        for w in &r.palabras {
                            if w.caja[2] < col[0] || w.caja[0] > col[2] {
                                c.ocupar(w.caja);
                            }
                        }
                    }
                }
                (i, c)
            })
            .collect();
        Anotador {
            paginas,
            colocadores,
            turno: turno.to_string(),
            puestas: 0,
            vistas: HashSet::new(),
            por_clase: HashMap::new(),
            no_encontradas: Vec::new(),
            sin_sitio: 0,
        }
    }

    /// El grupo de este turno.
    pub fn grupo(&self) -> String {
        format!("{GRUPO_TURNO}{}", self.turno)
    }

    pub fn cuantas(&self) -> usize {
        self.puestas
    }

    /// **Pone una anotacion**: la ancla en su pagina (o en otra si alli no
    /// esta), le busca sitio en el margen y hace sus figuras. `Err` con el
    /// motivo si no se encontro el ancla o no queda sitio.
    pub fn poner(&mut self, p: &Pedida) -> Result<Puesta, String> {
        if self.paginas.is_empty() {
            return Err("no hay paginas".into());
        }
        // La pagina que dijo primero; si alli no esta, las demas.
        let dicha = p
            .pagina
            .and_then(|n| self.paginas.iter().position(|x| x.numero == n));
        let mut orden: Vec<usize> = dicha.into_iter().collect();
        orden.extend((0..self.paginas.len()).filter(|i| Some(*i) != dicha));
        let mut hallado = None;
        for i in orden {
            let pag = &self.paginas[i];
            let a = if p.ancla.trim().is_empty() && p.clase == Clase::Resumen {
                ancla::primer_renglon(pag)
            } else {
                // La linea dicha solo vale en la pagina dicha.
                let linea = if Some(i) == dicha || dicha.is_none() {
                    p.linea
                } else {
                    None
                };
                ancla::anclar(pag, &p.ancla, linea)
            };
            if let Some(a) = a {
                hallado = Some((i, a));
                break;
            }
        }
        let Some((i, a)) = hallado else {
            self.no_encontradas.push(p.ancla.clone());
            return Err(format!("«{}» no esta en el texto", p.ancla));
        };
        let clave = (i, normalizar(&p.ancla));
        if !self.vistas.insert(clave) {
            return Err(format!("«{}» ya estaba anotado", p.ancla));
        }
        let pag = &self.paginas[i];
        let k = self.puestas + 1;
        let nota = figuras::Nota::de(p, pag.letra);
        let colocador = self
            .colocadores
            .get_mut(&i)
            .expect("un colocador por pagina");
        let colocada = colocador.colocar(
            a.union(),
            &|ancho| nota.alto_con(ancho),
            nota.ancho_maximo(),
        );
        let Some(colocada) = colocada else {
            self.sin_sitio += 1;
            return Err("no queda sitio en el margen de esa pagina".into());
        };
        let grupos = vec![format!("{}#{k}", self.grupo()), self.grupo()];
        let elementos = figuras::de(p.clase, &a, &colocada, &nota, pag, grupos);
        self.puestas = k;
        *self.por_clase.entry(p.clase).or_default() += 1;
        Ok(Puesta {
            en: i,
            clase: p.clase,
            ancla: p.ancla.clone(),
            fuera: colocada.franja.fuera,
            lado: colocada.franja.lado,
            elementos,
        })
    }

    /// Lo hecho, contado para el modelo: «5 anotaciones (3 importante, 2
    /// pregunta)».
    pub fn resumen(&self) -> String {
        let mut partes: Vec<(usize, &str)> = self
            .por_clase
            .iter()
            .map(|(c, n)| (*n, c.nombre()))
            .collect();
        partes.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        let detalle: Vec<String> = partes.iter().map(|(n, c)| format!("{n} {c}")).collect();
        let mut s = format!("{} anotaciones ({})", self.puestas, detalle.join(", "));
        if !self.no_encontradas.is_empty() {
            let lista: Vec<String> = self
                .no_encontradas
                .iter()
                .take(4)
                .map(|a| format!("«{a}»"))
                .collect();
            s.push_str(&format!(
                "; no se encontraron en el texto: {}",
                lista.join(", ")
            ));
        }
        if self.sin_sitio > 0 {
            s.push_str(&format!("; {} no cupieron en el margen", self.sin_sitio));
        }
        s
    }
}
