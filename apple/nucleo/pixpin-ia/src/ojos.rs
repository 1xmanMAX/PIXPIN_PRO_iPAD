//! **Los ojos del asistente sobre el lienzo** (15-oct-2026).
//!
//! Pedido del usuario: «dale los ojos al canvas: que [las IAs] puedan ver el
//! canvas, para que puedan revisar y dar un mejor resultado, corregir sus
//! anotaciones». Hasta aqui el modelo solo leia el resumen en texto del
//! lienzo (`edicion::resumen`): cajas y colores, pero no si un texto se sale
//! de su caja, si una flecha apunta a donde debe o si dos cosas se tapan.
//!
//! ## Las piezas (lo de aqui es Rust puro; pintar lo hace la app)
//!
//! - La herramienta [`NOMBRE`] (`ver_lienzo`): una [`Mirada`] con su zona
//!   (todo, una caja, lo elegido, lo ultimo, unos elementos o el documento
//!   anotado). La contesta el ejecutor ([`crate::agente::Ejecutor::ver`]),
//!   que pinta con **el mismo motor del lienzo** a un PNG de
//!   [`LADO_MAXIMO`] px como mucho, con una rejilla ligera numerada en las
//!   coordenadas del modelo y una etiqueta `#id` en cada elemento. La imagen
//!   va al modelo en un mensaje del usuario detras de los resultados de las
//!   herramientas ([`mensaje_de_vista`]): es lo unico que admiten igual
//!   Claude, DeepSeek y llama.cpp (DeepSeek no acepta imagenes en un
//!   resultado de herramienta).
//! - **La revision** ([`crate::agente::Opciones::revisar`]): tras un turno
//!   que dibujo o anoto, el agente le ensena al modelo el resultado una vez
//!   ([`pedido_de_revision`]) y le deja corregir; si contesta [`TODO_BIEN`],
//!   no se dice nada mas.
//! - **Mirar al empezar** ([`pide_mirar`]): «corrige», «mejora», «¿que
//!   falta?», «alinea»… dependen de lo que se ve, asi que la app pega la
//!   vista al pedido ademas del resumen en texto.
//!
//! Las coordenadas son las del modelo (las del resumen, medidas desde
//! [`Contexto::marco`]): la rejilla dice esos numeros, para que lo que vea lo
//! pueda escribir tal cual en `transformar_elementos`.

use serde_json::{Value, json};

use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::Elemento;

use crate::edicion::Contexto;
use crate::herramientas::Herramienta;
use crate::motor::Mensaje;

pub const NOMBRE: &str = "ver_lienzo";

/// Con que empieza el mensaje que lleva la vista: el agente lo reconoce y no
/// lo toma por el pedido del usuario.
pub const MARCA: &str = "[Vista del lienzo]";

/// El lado mayor de la imagen como mucho. ~1000 tokens de vision en Claude
/// (lado×lado/750) y una lectura de pocos segundos en el local.
pub const LADO_MAXIMO: u32 = 1024;

/// Lo que mas se amplia una zona pequena (una etiqueta sola): mas, y la
/// rejilla miente sobre el tamano real.
pub const AUMENTO_MAXIMO: f32 = 2.0;

/// Cuantas etiquetas `#id` como mucho (con mas, el dibujo no se ve).
pub const MAX_ETIQUETAS: usize = 40;

/// Margen alrededor de lo mirado, en unidades del lienzo.
pub const MARGEN: f32 = 30.0;

/// Lo que contesta el modelo en la revision si no hay nada que corregir.
pub const TODO_BIEN: &str = "TODO_BIEN";

/// Que mirar.
#[derive(Debug, Clone, PartialEq)]
pub enum Zona {
    /// Todo lo que hay.
    Todo,
    /// Una caja `[x, y, ancho, alto]` en las coordenadas del modelo.
    Caja([f32; 4]),
    /// Lo elegido por el usuario en el lienzo abierto.
    Elegido,
    /// Lo que creo o toco la ultima llamada de este turno.
    Ultimo,
    /// Unos elementos dichos como en las herramientas de editar («#12»,
    /// «el circulo rojo»), con lo de alrededor.
    Elementos(String),
    /// Todo lo que creo o toco este turno, con lo de alrededor (la
    /// revision). La app pone esos ids en `Contexto::ultimos`.
    Turno,
    /// La hoja del documento (PDF, foto) que se acaba de anotar, con las
    /// anotaciones encima.
    Documento,
}

/// Un pedido de vista.
#[derive(Debug, Clone, PartialEq)]
pub struct Mirada {
    pub zona: Zona,
    /// Rejilla numerada y etiquetas `#id` (por omision, si).
    pub marcas: bool,
}

impl Default for Mirada {
    fn default() -> Self {
        Mirada { zona: Zona::Todo, marcas: true }
    }
}

impl Mirada {
    pub fn de(zona: Zona) -> Mirada {
        Mirada { zona, marcas: true }
    }
}

/// Lo que devuelve el ejecutor: la imagen y lo que hay que saber para leerla.
#[derive(Debug, Clone, PartialEq)]
pub struct Vista {
    pub png: Vec<u8>,
    pub ancho: u32,
    pub alto: u32,
    /// La leyenda para el modelo ([`leyenda`]).
    pub texto: String,
    /// Lo que tardo en pintarse (registro y banco).
    pub ms: u128,
}

/// **La herramienta** para el catalogo del turno.
pub fn herramienta() -> Herramienta {
    Herramienta {
        nombre: NOMBRE.into(),
        descripcion: concat!(
            "Mira el lienzo: devuelve una imagen de lo que hay (o de una zona), con una rejilla numerada en tus coordenadas ",
            "y la etiqueta #id de cada elemento. Usala para comprobar lo dibujado o anotado (textos cortados o encima de otros, ",
            "flechas que no apuntan, cosas fuera de sitio) y corregirlo con las herramientas de editar, o para entender un ",
            "lienzo antes de cambiarlo. «documento»: la hoja del PDF o la foto que acabas de anotar."
        )
        .into(),
        parametros: json!({
            "type": "object",
            "properties": {
                "zona": {"type": "string", "enum": ["todo", "caja", "elegido", "ultimo", "elementos", "documento"],
                         "description": "que mirar (omitir: todo)"},
                "caja": {"type": "array", "items": {"type": "number"}, "maxItems": 4, "description": "con zona=caja: [x, y, ancho, alto]"},
                "que": {"type": "string", "description": "con zona=elementos: «#12», «el circulo rojo»..."},
            }
        }),
    }
}

/// **Lee los argumentos** del modelo. Lo que falte se entiende como «todo»;
/// solo es error lo que no se puede entender.
pub fn leer(argumentos: &str) -> Result<Mirada, String> {
    let v = if argumentos.trim().is_empty() {
        json!({})
    } else {
        crate::herramientas::reparar_json(argumentos).ok_or("los argumentos de ver_lienzo no son JSON")?
    };
    let texto = |k: &str| v.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    let caja = v.get("caja").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).map(|n| n as f32).collect::<Vec<_>>());
    let zona = match texto("zona").map(crate::acciones::normalizar).as_deref() {
        None | Some("todo") | Some("todo el lienzo") => match (&caja, texto("que")) {
            (Some(c), _) if c.len() == 4 => Zona::Caja([c[0], c[1], c[2], c[3]]),
            (_, Some(q)) => Zona::Elementos(q.to_string()),
            _ => Zona::Todo,
        },
        Some("caja") => match caja {
            Some(c) if c.len() == 4 && c[2] > 0.0 && c[3] > 0.0 => Zona::Caja([c[0], c[1], c[2], c[3]]),
            _ => return Err("con zona=caja hace falta «caja»: [x, y, ancho, alto] con ancho y alto positivos".into()),
        },
        Some("elegido") | Some("seleccion") => Zona::Elegido,
        Some("ultimo") | Some("lo ultimo") => Zona::Ultimo,
        Some("elementos") | Some("elemento") => Zona::Elementos(texto("que").ok_or("con zona=elementos hace falta «que» (#id o descripcion)")?.to_string()),
        Some("documento") | Some("pagina") | Some("foto") | Some("plano") => Zona::Documento,
        Some(otra) => return Err(format!("zona «{otra}» no existe: usa todo, caja, elegido, ultimo, elementos o documento")),
    };
    Ok(Mirada::de(zona))
}

/// Une cajas `(x0, y0, x1, y1)`.
fn unir(cajas: impl Iterator<Item = (f32, f32, f32, f32)>) -> Option<(f32, f32, f32, f32)> {
    cajas
        .filter(|c| [c.0, c.1, c.2, c.3].iter().all(|v| v.is_finite()))
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
}

/// **La caja que se mira**, en coordenadas de la escena y con
/// [`MARGEN`]. `Err` con el motivo para el modelo (lienzo vacio, nada
/// elegido, referencia que no encaja). [`Zona::Documento`] no es de la
/// escena: la resuelve la app.
pub fn caja_de(escena: &Escena, m: &Mirada, ctx: &Contexto) -> Result<(f32, f32, f32, f32), String> {
    let vivos = || escena.a_la_vista();
    let de_ids = |ids: &[u64]| unir(vivos().filter(|e| ids.contains(&e.id)).map(Elemento::caja));
    let caja = match &m.zona {
        Zona::Todo | Zona::Documento => unir(vivos().map(Elemento::caja)).ok_or("el lienzo esta vacio: no hay nada que ver")?,
        Zona::Caja([x, y, w, h]) => {
            let (x0, y0) = (x + ctx.marco.0, y + ctx.marco.1);
            return Ok((x0, y0, x0 + w.abs().max(1.0), y0 + h.abs().max(1.0)));
        }
        Zona::Elegido => de_ids(&ctx.elegidos).ok_or("el usuario no tiene nada elegido en el lienzo")?,
        Zona::Ultimo => de_ids(&ctx.ultimos).ok_or("aun no has dibujado ni tocado nada en este turno")?,
        Zona::Turno => {
            let c = de_ids(&ctx.ultimos).ok_or("aun no has dibujado ni tocado nada en este turno")?;
            // Si todo el lienzo no es mucho mayor, todo: lo nuevo suele ir
            // con lo que ya habia (etiquetas de figuras, flechas a ellas).
            let todo = unir(vivos().map(Elemento::caja)).unwrap_or(c);
            let (tw, th) = (todo.2 - todo.0, todo.3 - todo.1);
            if tw.max(th) <= 1600.0 || (tw <= 3.0 * (c.2 - c.0).max(200.0) && th <= 3.0 * (c.3 - c.1).max(200.0)) {
                todo
            } else {
                let aire = 0.25 * (c.2 - c.0).max(c.3 - c.1).max(80.0);
                (c.0 - aire, c.1 - aire, c.2 + aire, c.3 + aire)
            }
        }
        Zona::Elementos(q) => {
            let e = crate::edicion::referencia::resolver(escena, q, ctx, None)?;
            let c = de_ids(&e.ids).ok_or_else(|| format!("no hay nada que encaje con «{q}»"))?;
            // Con lo de alrededor: para ver si tapa o lo tapan.
            let aire = 0.5 * (c.2 - c.0).max(c.3 - c.1).max(60.0);
            (c.0 - aire, c.1 - aire, c.2 + aire, c.3 + aire)
        }
    };
    Ok((caja.0 - MARGEN, caja.1 - MARGEN, caja.2 + MARGEN, caja.3 + MARGEN))
}

/// **El tamano de la imagen y su escala** para una caja: el lado mayor a
/// `lado` como mucho, sin ampliar mas de [`AUMENTO_MAXIMO`].
pub fn tamano(caja: (f32, f32, f32, f32), lado: u32) -> (u32, u32, f32) {
    let (w, h) = ((caja.2 - caja.0).max(1.0), (caja.3 - caja.1).max(1.0));
    let escala = (lado as f32 / w.max(h)).min(AUMENTO_MAXIMO);
    (((w * escala).round() as u32).max(16), ((h * escala).round() as u32).max(16), escala)
}

/// **El paso de la rejilla** en unidades del lienzo: un numero redondo que
/// deje entre 4 y 10 rayas en el lado mayor.
pub fn paso_de_rejilla(caja: (f32, f32, f32, f32)) -> f32 {
    let lado = (caja.2 - caja.0).max(caja.3 - caja.1).max(1.0);
    let crudo = lado / 8.0;
    let potencia = 10f32.powf(crudo.log10().floor());
    for f in [1.0, 2.0, 2.5, 5.0, 10.0] {
        if f * potencia >= crudo {
            return f * potencia;
        }
    }
    10.0 * potencia
}

/// **Donde va cada etiqueta `#id`**, en coordenadas de la escena: la esquina
/// de arriba a la izquierda de cada elemento que cae en la caja (los de
/// encima primero, hasta [`MAX_ETIQUETAS`]). Las guias no llevan.
pub fn etiquetas(escena: &Escena, caja: (f32, f32, f32, f32)) -> Vec<(u64, (f32, f32))> {
    let vivos: Vec<&Elemento> = escena
        .a_la_vista()
        .filter(|e| !e.extras.referencia)
        .filter(|e| {
            let (x0, y0, x1, y1) = e.caja();
            x1 >= caja.0 && x0 <= caja.2 && y1 >= caja.1 && y0 <= caja.3
        })
        .collect();
    let desde = vivos.len().saturating_sub(MAX_ETIQUETAS);
    vivos[desde..]
        .iter()
        .map(|e| {
            let (x0, y0, _, _) = e.caja();
            (e.id, (x0.max(caja.0), y0.max(caja.1)))
        })
        .collect()
}

/// **La leyenda de la imagen**: que zona es (en las coordenadas del modelo),
/// la escala y como leer la rejilla.
pub fn leyenda(caja: (f32, f32, f32, f32), marco: (f32, f32), ancho: u32, alto: u32, escala: f32, marcas: bool) -> String {
    let r = |v: f32| v.round() as i64;
    let mut s = format!(
        "{MARCA} Zona [{}, {}, {}, {}] en tus coordenadas, imagen de {ancho}x{alto} px (1 unidad = {:.2} px).",
        r(caja.0 - marco.0),
        r(caja.1 - marco.1),
        r(caja.2 - caja.0),
        r(caja.3 - caja.1),
        escala
    );
    if marcas {
        s.push_str(&format!(
            " Rejilla gris cada {} unidades con sus numeros en los bordes; cada elemento lleva su #id en una etiqueta amarilla arriba a la izquierda (las etiquetas y la rejilla no son del dibujo).",
            r(paso_de_rejilla(caja))
        ));
    }
    s
}

/// **El mensaje que lleva la vista** al modelo.
pub fn mensaje_de_vista(v: &Vista, extra: &str) -> Mensaje {
    let texto = if extra.is_empty() { v.texto.clone() } else { format!("{}\n{extra}", v.texto) };
    Mensaje::usuario_con_imagen(texto, v.png.clone(), "image/png")
}

/// Si un mensaje es una vista (no el pedido del usuario).
pub fn es_vista(m: &Mensaje) -> bool {
    m.texto_entero().trim_start().starts_with(MARCA)
}

/// **Lo que se le pide en la revision**, con el pedido original.
pub fn pedido_de_revision(pedido: &str, ronda: usize) -> String {
    let pedido: String = pedido.chars().take(600).collect();
    format!(
        "Revision {ronda}: esta es la imagen de como quedo el lienzo tras tus cambios. Comparala con lo que pidio el usuario: «{}». \
         Busca solo fallos que se vean: texto cortado, encima de otra cosa o fuera de su figura; flechas o lineas que no llegan a lo que senalan; \
         cosas solapadas, fuera de sitio o desalineadas; una anotacion que no cae sobre lo senalado; algo pedido que falta. \
         Si esta bien, contesta solo {TODO_BIEN}. Si no, corrigelo ya con las herramientas de editar (usa los #id de las etiquetas y las \
         coordenadas de la rejilla; mueve o cambia, no lo vuelvas a dibujar entero) y luego di en una frase corta que corregiste.",
        pedido.trim()
    )
}

/// Si la respuesta de la revision dice que no hay nada que corregir.
pub fn es_todo_bien(texto: &str) -> bool {
    let t = crate::acciones::normalizar(texto.trim());
    t.is_empty() || t.contains(&TODO_BIEN.to_lowercase()) || t.contains("todo_bien") || t.trim_matches(|c: char| !c.is_alphanumeric()) == "ok"
}

/// **Si el pedido depende de lo que se ve** del lienzo: entonces conviene
/// pegarle la vista ademas del resumen en texto.
pub fn pide_mirar(pedido: &str) -> bool {
    let q = crate::acciones::normalizar(pedido);
    const CLAVES: &[&str] = &[
        "corrige", "corregir", "arregla", "mejora", "mejorar", "que falta", "falta algo", "alinea", "alinear", "ordena",
        "solap", "se tapa", "encima de", "revisa", "mira el lienzo", "mira lo", "como quedo", "que hay en el lienzo",
        "que ves", "acomoda", "reorganiza", "centra", "etiqueta a cada", "etiquetas a", "rotula", "ponle nombre",
        "nombra cada", "a cada figura", "a cada uno", "cada elemento", "distribuye", "espacia", "endereza",
    ];
    CLAVES.iter().any(|c| q.contains(c))
}

/// **Si una llamada cambio el lienzo o un documento** (lo que merece una
/// revision): dibujar, editar (menos elegir) o anotar un documento.
pub fn cambia_lo_que_se_ve(l: &crate::herramientas::Llamada) -> bool {
    use crate::herramientas::Llamada;
    match l {
        _ if l.dibuja() => true,
        Llamada::Editar(e) => e.herramienta != crate::edicion::SELECCIONAR,
        Llamada::App(a) => a.herramienta == crate::acciones_documentos::ANOTAR,
        _ => false,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn escena_con(cajas: &[(f32, f32, f32, f32)]) -> Escena {
        let mut e = Escena::nueva();
        for &(x, y, w, h) in cajas {
            e.anadir(Elemento { x, y, ancho: w, alto: h, ..Default::default() });
        }
        e
    }

    #[test]
    fn lee_las_zonas_y_lo_que_falta_es_todo() {
        assert_eq!(leer("").unwrap().zona, Zona::Todo);
        assert_eq!(leer("{}").unwrap().zona, Zona::Todo);
        assert_eq!(leer(r#"{"zona":"caja","caja":[1,2,3,4]}"#).unwrap().zona, Zona::Caja([1.0, 2.0, 3.0, 4.0]));
        assert_eq!(leer(r#"{"caja":[1,2,3,4]}"#).unwrap().zona, Zona::Caja([1.0, 2.0, 3.0, 4.0]));
        assert_eq!(leer(r##"{"zona":"elementos","que":"#3"}"##).unwrap().zona, Zona::Elementos("#3".into()));
        assert_eq!(leer(r#"{"zona":"documento"}"#).unwrap().zona, Zona::Documento);
        assert_eq!(leer(r#"{"zona":"último"}"#).unwrap().zona, Zona::Ultimo);
        // Casos negativos: lo que no se entiende se dice.
        assert!(leer(r#"{"zona":"caja"}"#).unwrap_err().contains("caja"));
        assert!(leer(r#"{"zona":"cielo"}"#).unwrap_err().contains("cielo"));
        assert!(leer(r#"{"zona":"elementos"}"#).is_err());
    }

    #[test]
    fn la_caja_de_cada_zona_en_la_escena() {
        let e = escena_con(&[(100.0, 100.0, 50.0, 50.0), (400.0, 300.0, 100.0, 20.0)]);
        let ctx = Contexto { marco: (60.0, 60.0), elegidos: vec![2], ultimos: vec![1] };
        let todo = caja_de(&e, &Mirada::default(), &ctx).unwrap();
        assert_eq!(todo, (100.0 - MARGEN, 100.0 - MARGEN, 500.0 + MARGEN, 320.0 + MARGEN));
        let c = caja_de(&e, &Mirada::de(Zona::Caja([0.0, 0.0, 10.0, 20.0])), &ctx).unwrap();
        assert_eq!(c, (60.0, 60.0, 70.0, 80.0));
        let el = caja_de(&e, &Mirada::de(Zona::Elegido), &ctx).unwrap();
        assert!(el.0 < 400.0 && el.2 > 500.0);
        let ul = caja_de(&e, &Mirada::de(Zona::Ultimo), &ctx).unwrap();
        assert!(ul.0 < 100.0 && ul.2 < 400.0);
        let un = caja_de(&e, &Mirada::de(Zona::Elementos("#1".into())), &ctx).unwrap();
        assert!(un.0 < 100.0 - MARGEN && un.2 > 150.0 + MARGEN);
        // Casos negativos.
        let vacia = Escena::nueva();
        assert!(caja_de(&vacia, &Mirada::default(), &ctx).unwrap_err().contains("vacio"));
        let sin = Contexto::default();
        assert!(caja_de(&e, &Mirada::de(Zona::Elegido), &sin).is_err());
        assert!(caja_de(&e, &Mirada::de(Zona::Ultimo), &sin).is_err());
    }

    #[test]
    fn el_tamano_no_pasa_del_lado_ni_amplia_de_mas() {
        let (w, h, s) = tamano((0.0, 0.0, 4000.0, 1000.0), LADO_MAXIMO);
        assert_eq!((w, h), (1024, 256));
        assert!((s - 0.256).abs() < 1e-3);
        let (w, h, s) = tamano((0.0, 0.0, 100.0, 50.0), LADO_MAXIMO);
        assert_eq!((w, h, s), (200, 100, AUMENTO_MAXIMO));
    }

    #[test]
    fn la_rejilla_es_redonda() {
        assert_eq!(paso_de_rejilla((0.0, 0.0, 800.0, 600.0)), 100.0);
        assert_eq!(paso_de_rejilla((0.0, 0.0, 160.0, 40.0)), 20.0);
        assert_eq!(paso_de_rejilla((0.0, 0.0, 3000.0, 600.0)), 500.0);
    }

    #[test]
    fn etiquetas_de_lo_que_cae_en_la_caja() {
        let e = escena_con(&[(100.0, 100.0, 50.0, 50.0), (900.0, 900.0, 10.0, 10.0)]);
        let v = etiquetas(&e, (0.0, 0.0, 500.0, 500.0));
        assert_eq!(v, vec![(1, (100.0, 100.0))]);
        assert!(etiquetas(&e, (2000.0, 2000.0, 2100.0, 2100.0)).is_empty());
    }

    #[test]
    fn la_revision_reconoce_el_todo_bien() {
        assert!(es_todo_bien("TODO_BIEN"));
        assert!(es_todo_bien("todo_bien."));
        assert!(es_todo_bien("OK"));
        assert!(!es_todo_bien("Movi la etiqueta #4 para que no tape el circulo."));
        assert!(pedido_de_revision("dibuja una casa", 1).contains("dibuja una casa"));
    }

    #[test]
    fn pedidos_que_dependen_de_lo_que_se_ve() {
        assert!(pide_mirar("Corrige lo que se solapa"));
        assert!(pide_mirar("¿qué falta en el diagrama?"));
        assert!(pide_mirar("pon una etiqueta a cada figura"));
        assert!(pide_mirar("alinea los rectángulos"));
        assert!(!pide_mirar("dibuja una casa"));
        assert!(!pide_mirar("apunta comprar pan mañana"));
    }

    #[test]
    fn la_vista_va_en_un_mensaje_marcado_con_su_imagen() {
        let v = Vista { png: vec![1, 2], ancho: 2, alto: 1, texto: leyenda((0.0, 0.0, 100.0, 50.0), (0.0, 0.0), 2, 1, 0.02, true), ms: 1 };
        let m = mensaje_de_vista(&v, "");
        assert!(m.tiene_imagen());
        assert!(es_vista(&m));
        assert!(!es_vista(&Mensaje::usuario("dibuja")));
        assert!(m.texto_entero().contains("#id"));
    }
}

#[cfg(test)]
mod pruebas_revision;
